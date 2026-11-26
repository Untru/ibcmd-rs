use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Result, bail};
use clap::Parser;

use super::*;
use crate::cli::{Cli, Commands};
use crate::sql::{Dbms, ScriptVariables, SqlClient, SqlParam, SqlRow};

const STOP: &str = "provided certificate-policy handle reached the SQL read";

struct ReadTrap {
    reads: Arc<AtomicUsize>,
    writes: Arc<AtomicUsize>,
}

impl SqlClient for ReadTrap {
    fn dbms(&self) -> Dbms {
        Dbms::SqlServer
    }
    fn max_connections(&self) -> usize {
        1
    }
    fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        bail!("unexpected stage write")
    }
    fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        bail!("unexpected stage write")
    }
    fn read_rows(
        &self,
        _: &str,
        _: &[SqlParam<'_>],
        _: &mut dyn FnMut(SqlRow) -> Result<()>,
    ) -> Result<()> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        bail!(STOP)
    }
    fn query_json(&self, _: &str) -> Result<Option<String>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        bail!(STOP)
    }
    fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        bail!("unexpected stage write")
    }
}

fn source_args(trust: bool, tools: bool) -> MssqlApplySourceChangeArgs {
    let mut argv = vec![
        "ibcmd-rs",
        "mssql-apply-source-change",
        "--platform-profile",
        "platform-8.3.27.2214",
        "--database",
        "certificate_policy_test",
        "--server",
        "must-not-connect",
        "--source-root",
        "unused-source",
        "--path",
        "CommonModules/Probe/Ext/Module.bsl",
        "--mode",
        "online",
        "--cluster-id",
        "11111111-1111-4111-8111-111111111111",
        "--infobase-id",
        "22222222-2222-4222-8222-222222222222",
    ];
    if trust {
        argv.push("--sqlcmd-trust-cert");
    }
    if tools {
        argv.extend(["--sqlcmd", "must-not-run-sqlcmd.exe"]);
    }
    match Cli::try_parse_from(argv).unwrap().command {
        Commands::MssqlApplySourceChange(args) => args,
        _ => unreachable!(),
    }
}

#[test]
fn source_certificate_policy_reaches_both_connection_backends() {
    for trust in [false, true] {
        for tools in [false, true] {
            let sql = main_read_sql(&source_args(trust, tools)).unwrap();
            assert_eq!(sql.trust_server_certificate(), trust);
            assert_eq!(sql.tools().is_some(), tools);
        }
    }
}

#[test]
fn source_certificate_policy_handle_is_used_by_export_stage_and_dry_run() {
    let temporary = TemporaryApplyRoot::new().unwrap();
    let root = temporary.path.join("source");
    fs::create_dir_all(root.join("CommonModules/Probe/Ext")).unwrap();
    fs::write(root.join("CommonModules/Probe.xml"), br#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20"><CommonModule uuid="11111111-1111-4111-8111-111111111111"><Properties><Name>Probe</Name><Global>false</Global><ClientManagedApplication>false</ClientManagedApplication><Server>true</Server><ExternalConnection>false</ExternalConnection><ClientOrdinaryApplication>false</ClientOrdinaryApplication><ServerCall>false</ServerCall><Privileged>false</Privileged><ReturnValuesReuse>DontUse</ReturnValuesReuse></Properties></CommonModule></MetaDataObject>"#).unwrap();
    fs::write(root.join("CommonModules/Probe/Ext/Module.bsl"), b"// probe").unwrap();

    for trust in [false, true] {
        for route in ["export", "stage", "dry-run"] {
            let reads = Arc::new(AtomicUsize::new(0));
            let writes = Arc::new(AtomicUsize::new(0));
            let requested = main_read_sql(&source_args(trust, false)).unwrap();
            let sql = SqlExec::with_client(
                requested.target().clone(),
                Box::new(ReadTrap {
                    reads: reads.clone(),
                    writes: writes.clone(),
                }),
            );
            assert_eq!(sql.trust_server_certificate(), trust);
            let command = match route {
                "export" => "mssql-dump-config",
                "stage" => "mssql-stage-source-objects",
                _ => "mssql-audit-source-parity",
            };
            let mut argv = vec![
                "ibcmd-rs".to_owned(),
                command.to_owned(),
                "--server".to_owned(),
                "must-not-connect".to_owned(),
                "--database".to_owned(),
                format!("cert_{}", Uuid::new_v4().simple()),
                "--sqlcmd".to_owned(),
                "must-not-run-sqlcmd.exe".to_owned(),
            ];
            if route == "export" {
                argv.extend([
                    "--output-dir".to_owned(),
                    temporary
                        .path
                        .join(format!("export-{trust}"))
                        .display()
                        .to_string(),
                ]);
            } else {
                argv.extend(["--source-root".to_owned(), root.display().to_string()]);
            }
            if route == "stage" {
                argv.extend(
                    ["--replace-config-save", "--allow-non-lab", "--per-row"].map(str::to_owned),
                );
            }
            let outcome = match Cli::try_parse_from(argv).unwrap().command {
                Commands::MssqlDumpConfig(args) => {
                    crate::mssql_dump::dump_config_with_sql(&args, Some(sql))
                        .map(|r| serde_json::to_value(r).unwrap())
                }
                Commands::MssqlStageSourceObjects(args) => {
                    crate::mssql::stage_source_objects_with_sql(&args, Some(sql))
                        .map(|r| serde_json::to_value(r).unwrap())
                }
                Commands::MssqlAuditSourceParity(args) => {
                    crate::mssql::audit_source_parity_with_sql(&args, Some(sql))
                        .map(|r| serde_json::to_value(r).unwrap())
                }
                _ => unreachable!(),
            };
            let diagnostic = match outcome {
                Ok(report) => report.to_string(),
                Err(error) => format!("{error:#}"),
            };
            assert!(reads.load(Ordering::SeqCst) > 0, "{route}: {diagnostic}");
            assert!(diagnostic.contains(STOP), "{route}: {diagnostic}");
            assert_eq!(
                writes.load(Ordering::SeqCst),
                0,
                "{route}: no preparation writes after refused read"
            );
        }
    }
}
