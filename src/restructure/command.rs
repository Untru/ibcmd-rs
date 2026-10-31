//! `ibcmd-rs mssql-restructure`: the research command of the prototype.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::restructure::exec::{ExecOptions, ExecReport, run as run_plan};
use crate::restructure::plan::{Inputs, Method, Plan, PlanOptions, indexing_word, plan};
use crate::restructure::reader::{database_name, read_inputs};
use crate::restructure::size_guard::{self, LimitSetting};
use crate::restructure::storage::STATUS_IDLE;
use crate::sql::mssql::TdsPool;
use crate::sql::{SqlLogin, SqlTarget};

#[derive(Debug, Clone, Args)]
pub struct MssqlRestructureArgs {
    /// SQL Server (`host`, `host\instance`, `host,port`); Windows login unless --sql-user is given.
    #[arg(long, default_value = "localhost")]
    pub server: String,
    #[arg(long)]
    pub sql_user: Option<String>,
    #[arg(long)]
    pub sql_pwd: Option<String>,
    #[arg(long, default_value = "IBCMD_DB_PSW")]
    pub sql_pwd_env: String,
    /// The infobase's database. Writes are refused for a database that is not a lab one
    /// (`ibcmd_rs_04_*` / `ibcmd_rs_05_*`) unless --allow-non-lab is given.
    #[arg(long)]
    pub database: String,
    /// Plan and print; write nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Run the whole restructure in its transaction, verify, and roll it back.
    #[arg(long)]
    pub trial: bool,
    /// Do not refuse when other sessions are connected to the database.
    #[arg(long)]
    pub skip_session_check: bool,
    #[arg(long)]
    pub allow_non_lab: bool,
    /// The guid of the new `DBNamesVersion-DBNames` (default: random).
    #[arg(long)]
    pub names_version: Option<String>,
    /// Research: add the column with `ALTER TABLE ... ADD` (it lands at the end of the physical
    /// table) instead of rebuilding the object's tables as the platform does.
    #[arg(long)]
    pub alter_add: bool,
    /// Leave the XDTO model cache (`Params` `*.si`) as it is, stale: a new attribute then is unknown to XDTO
    /// serialization until the platform rebuilds the cache.
    #[arg(long)]
    pub skip_xdto: bool,
    /// Leave the object registry (`Params` `1a621f0f-....si`) as it is, stale: the new attribute is not
    /// listed in the search information until the platform rebuilds it.
    #[arg(long)]
    pub skip_registry: bool,
    /// Write the JSON report here as well.
    #[arg(long)]
    pub report: Option<PathBuf>,
    /// Write the new DBSchema text, the new DBNames text and the statements into this folder.
    #[arg(long)]
    pub dump_plan: Option<PathBuf>,
    /// Write what is known of the extensions (how many, the tables of their schema, the objects they
    /// adopt) here as JSON, before the plan is made.
    #[arg(long)]
    pub extensions_report: Option<PathBuf>,
    /// Run as the structural gate of the own config apply (docs/apply/restructuring.md, section 12): one
    /// SERIALIZABLE transaction rebuilds the tables, publishes the schema and moves the staged rows.
    /// With --dry-run it plans and checks and writes nothing.
    #[arg(long)]
    pub through_apply: bool,
    /// With --through-apply: run the whole script and roll it back.
    #[arg(long)]
    pub rehearse: bool,
    /// With --through-apply: write the T-SQL of the transaction here.
    #[arg(long)]
    pub script_output: Option<PathBuf>,
    /// The most rows of tables the rebuild may copy in its transaction (the sum); above it the stage is
    /// refused and goes to the native apply. Default: IBCMD_RS_RESTRUCTURE_LIMIT_ROWS,
    /// `restructure-limit-rows` of ibcmd-rs.toml, the measured default.
    #[arg(long)]
    pub restructure_limit_rows: Option<u64>,
    /// The most bytes the rebuild may write to the log under full recovery (the data of the rebuilt tables
    /// twice, their other indexes once; the sum; `2GB`, `512MB`, a number of bytes). Default:
    /// IBCMD_RS_RESTRUCTURE_LIMIT_BYTES, `restructure-limit-bytes` of ibcmd-rs.toml, the measured default.
    #[arg(long)]
    pub restructure_limit_bytes: Option<String>,
    /// With --through-apply: the folder of the apply's recovery artifact.
    #[arg(long)]
    pub recovery_dir: Option<PathBuf>,
    /// With --through-apply: a structural apply drops the old tables in its transaction and is taken back
    /// from a backup only, so it refuses unless it has one. This takes a COPY_ONLY backup of the database
    /// into the file (a path on the SQL Server host) before the apply runs (recommended)...
    #[arg(long)]
    pub recovery_backup: Option<PathBuf>,
    /// ...and this says that the caller has taken one.
    #[arg(long)]
    pub i_have_a_backup: bool,
}

#[derive(Debug, Serialize)]
pub struct AdditionReport {
    pub attribute: String,
    pub uuid: String,
    pub field: String,
    pub number: u64,
    pub position: usize,
}

#[derive(Debug, Serialize)]
pub struct RemovalReport {
    pub attribute: String,
    pub uuid: String,
    pub field: String,
    pub number: u64,
    pub indexes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct WideningReport {
    pub attribute: String,
    pub uuid: String,
    pub field: String,
    pub number: u64,
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Serialize)]
pub struct SwitchReport {
    pub attribute: String,
    pub uuid: String,
    pub field: String,
    pub from: String,
    pub to: String,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

/// A tabular section that is new or got new attributes.
#[derive(Debug, Serialize)]
pub struct SectionReport {
    pub section: String,
    pub uuid: String,
    /// The sub-table, `VT155`.
    pub table: String,
    /// A new section: the numbers of its `VT` and `LineNo` entries.
    pub table_number: Option<u64>,
    pub line_no: Option<u64>,
    pub attributes: Vec<AdditionReport>,
    pub indexes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct TableReport {
    pub table: String,
    pub columns: usize,
    pub indexes: Vec<String>,
    pub copy_columns: usize,
    /// The table is created (a new section's), not copied.
    pub created: bool,
}

#[derive(Debug, Serialize)]
pub struct StatementReport {
    pub phase: String,
    pub label: String,
    pub sql: String,
}

#[derive(Debug, Serialize)]
pub struct ObjectReport {
    pub kind: String,
    pub object: String,
    pub name: String,
    pub uuid: String,
    pub additions: Vec<AdditionReport>,
    pub removals: Vec<RemovalReport>,
    pub widenings: Vec<WideningReport>,
    pub switches: Vec<SwitchReport>,
    pub sections: Vec<SectionReport>,
    pub tables: Vec<TableReport>,
}

#[derive(Debug, Default, Serialize)]
pub struct RestructureReport {
    pub database: String,
    pub mode: String,
    pub summary: String,
    pub objects: Vec<ObjectReport>,
    pub caches: Vec<String>,
    pub old_schema_sha256: String,
    pub new_schema_sha256: String,
    pub new_names_sha256: String,
    pub statements: Vec<StatementReport>,
    pub execution: Option<ExecReport>,
    /// The size guard's verdict on the tables the plan rebuilds (S1-J): the limit, the totals, the largest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_check: Option<serde_json::Value>,
    /// `--through-apply`: the apply's report (its structure phase, gate verdict, timings).
    pub through_apply: Option<serde_json::Value>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_lab_database(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ["ibcmd_rs_04_", "ibcmd_rs_05_"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

pub fn run(args: &MssqlRestructureArgs) -> Result<RestructureReport> {
    let writes = !args.dry_run;
    if writes && !is_lab_database(&args.database) && !args.allow_non_lab {
        bail!(
            "{} is not a lab database (ibcmd_rs_04_* / ibcmd_rs_05_*): a restructure writes it; pass --allow-non-lab to continue",
            args.database
        );
    }
    let password = match &args.sql_user {
        Some(_) => args
            .sql_pwd
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var(&args.sql_pwd_env).ok()),
        None => None,
    };
    if args.sql_user.is_some() && password.is_none() {
        bail!(
            "SQL login {:?} needs a password: pass --sql-pwd or set {}",
            args.sql_user.as_deref().unwrap_or_default(),
            args.sql_pwd_env
        );
    }
    if args.through_apply {
        return run_through_apply(args, password.as_deref());
    }
    let target = SqlTarget {
        server: args.server.clone(),
        database: Some(args.database.clone()),
        login: SqlLogin::from_user(args.sql_user.as_deref(), password.as_deref()),
        trust_server_certificate: true,
    };
    let pool = TdsPool::new(target.clone(), 1)?;
    let mut connection = pool.dedicated()?;
    let connected = database_name(&mut connection)?;
    if !connected.eq_ignore_ascii_case(&args.database) {
        bail!("connected to {connected}, not {}", args.database);
    }

    let (mut inputs, storage) = read_inputs(&mut connection)?;
    if inputs.extensions.registered > 0 {
        let sql = crate::sql::SqlExec::sql_server(target)?;
        inputs.extensions.adoptions =
            crate::restructure::extensions::read_adoptions(&sql, &args.database)
                .context("the objects the extensions adopt")?;
        inputs.extensions.adoptions_read = true;
    }
    if let Some(path) = &args.extensions_report {
        std::fs::write(path, serde_json::to_string_pretty(&inputs.extensions)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    let main = storage
        .iter()
        .find(|row| row.schema_id == 0)
        .context("SchemaStorage has no main row")?;
    if !main.is_idle() {
        bail!(
            "SchemaStorage is not idle (status {}, generations {} / {} bytes): an interrupted restructure has to be finished first",
            main.status,
            main.new_gen_created.len(),
            main.new_gen_dropped.len()
        );
    }
    debug_assert_eq!(main.status, STATUS_IDLE);
    let plan = make_plan(&inputs, args)?;
    // S1-J: a rebuild copies the tables in one transaction; above the limit nothing is written.
    let size = if plan.method == Method::Rebuild {
        let tables: Vec<String> = plan
            .tables()
            .map(|table| table.table.name.clone())
            .collect();
        Some(size_guard::check_tables(
            &mut connection,
            &tables,
            &limit_of(args)?,
        )?)
    } else {
        None
    };

    let mode = if args.dry_run {
        "dry-run"
    } else if args.trial {
        "trial"
    } else {
        "apply"
    };
    let mut report = describe(&plan, &args.database, mode);
    report.size_check = size.as_ref().map(size_guard::SizeCheck::to_json);
    if let Some(reason) = size.as_ref().and_then(size_guard::SizeCheck::refusal) {
        if let Some(path) = &args.report {
            std::fs::write(path, serde_json::to_string_pretty(&report)?)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        bail!("S1: {reason}");
    }
    if let Some(folder) = &args.dump_plan {
        dump(&plan, folder)?;
    }
    if writes {
        report.execution = Some(run_plan(
            &mut connection,
            &plan,
            ExecOptions {
                trial: args.trial,
                skip_session_check: args.skip_session_check,
            },
        )?);
    }
    if let Some(path) = &args.report {
        std::fs::write(path, serde_json::to_string_pretty(&report)?)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(report)
}

/// The limit of the run: the flags, else the settings chain, else the default.
fn limit_of(args: &MssqlRestructureArgs) -> Result<LimitSetting> {
    size_guard::resolve_limit(
        &crate::settings::Settings::load(None)?,
        args.restructure_limit_rows,
        args.restructure_limit_bytes.as_deref(),
    )
}

fn plan_options(args: &MssqlRestructureArgs) -> PlanOptions {
    PlanOptions {
        names_version: args.names_version.clone(),
        skip_xdto: args.skip_xdto,
        skip_registry: args.skip_registry,
        method: if args.alter_add {
            Method::AlterAdd
        } else {
            Method::Rebuild
        },
    }
}

fn make_plan(inputs: &Inputs, args: &MssqlRestructureArgs) -> Result<Plan> {
    plan(inputs, &plan_options(args))
}

/// `--through-apply`: the own config apply with the S1 gate. The apply reads the stage, asks the gate,
/// and runs ONE script: locks, assertions, the structure phase, the promotion of the rows, the caches.
fn run_through_apply(
    args: &MssqlRestructureArgs,
    password: Option<&str>,
) -> Result<RestructureReport> {
    use crate::mssql_config_apply::{
        BackupPolicy, ConfigApplyOptions, Exclusivity, StructuralRefusal, apply_with_gate,
    };
    use crate::mssql_platform_profile::MssqlNativePlatformProfile;
    use crate::restructure::s1::S1Gate;
    use crate::sql::{SqlExec, SqlOptions};

    if args.alter_add {
        bail!("--alter-add is a research switch of the direct command, not of the apply");
    }
    let sql = SqlExec::from_options(SqlOptions {
        sqlcmd: None,
        bcp: None,
        server: &args.server,
        user: args.sql_user.as_deref(),
        password,
        password_env: &args.sql_pwd_env,
        trust_server_certificate: true,
    })?;
    let mut options = ConfigApplyOptions::new(
        args.database.clone(),
        MssqlNativePlatformProfile::Platform8_3_27_2214,
        crate::restructure::extensions::read_adoptions,
        &crate::mssql_dump::StoredActivationRows,
    );
    options.dry_run = args.dry_run;
    options.rehearse = args.rehearse;
    options.exclusivity = if args.skip_session_check {
        Exclusivity::Assumed
    } else {
        Exclusivity::SqlSessions
    };
    options.recovery_dir = args.recovery_dir.clone();
    options.script_output = args.script_output.clone();
    // The apply owns the backup policy: a structural apply that writes refuses unless it has a
    // `--recovery-backup` (it takes the COPY_ONLY backup first) or an `--i-have-a-backup`.
    options.backup = match (&args.recovery_backup, args.i_have_a_backup) {
        (Some(file), _) => BackupPolicy::File(file.clone()),
        (None, true) => BackupPolicy::Acknowledged,
        (None, false) => BackupPolicy::None,
    };
    options.restructure_limit = limit_of(args)?;
    let gate = S1Gate::new(
        &sql,
        options.conservative_gate(),
        plan_options(args),
        crate::restructure::extensions::read_adoptions,
    )
    .xml_version(Some("2.20"))
    .size_limit(options.restructure_limit.clone());
    let mode = if args.dry_run {
        "through-apply, dry-run"
    } else if args.rehearse {
        "through-apply, rehearsal"
    } else {
        "through-apply"
    };
    let write_report = |json: &str| -> Result<()> {
        if let Some(path) = &args.report {
            std::fs::write(path, json)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
        Ok(())
    };
    match apply_with_gate(&sql, &options, &gate) {
        Ok(applied) => {
            let report = RestructureReport {
                database: args.database.clone(),
                mode: mode.to_owned(),
                summary: applied
                    .structure
                    .as_ref()
                    .map(|phase| phase.objects.join("; "))
                    .unwrap_or_else(|| "no restructuring in the stage".to_owned()),
                caches: applied
                    .structure
                    .as_ref()
                    .map(|phase| phase.caches.clone())
                    .unwrap_or_default(),
                through_apply: Some(serde_json::to_value(&applied)?),
                ..RestructureReport::default()
            };
            write_report(&serde_json::to_string_pretty(&report)?)?;
            Ok(report)
        }
        Err(error) => {
            if let Some(refusal) = error.downcast_ref::<StructuralRefusal>() {
                let json = serde_json::to_string_pretty(&serde_json::json!({
                    "refused": "needs_native_apply",
                    "database": args.database,
                    "gate": refusal.verdict,
                }))?;
                write_report(&json)?;
                println!("{json}");
            }
            Err(error)
        }
    }
}

fn describe(plan: &Plan, database: &str, mode: &str) -> RestructureReport {
    RestructureReport {
        database: database.to_owned(),
        mode: mode.to_owned(),
        summary: plan.summary(),
        objects: plan
            .objects
            .iter()
            .map(|object| ObjectReport {
                kind: object.kind.label().to_owned(),
                object: object.object.clone(),
                name: object.object_name.clone(),
                uuid: object.object_uuid.clone(),
                additions: object
                    .additions
                    .iter()
                    .map(|addition| AdditionReport {
                        attribute: addition.name.clone(),
                        uuid: addition.uuid.clone(),
                        field: addition.field.name.clone(),
                        number: addition.number,
                        position: addition.position,
                    })
                    .collect(),
                removals: object
                    .removals
                    .iter()
                    .map(|removal| RemovalReport {
                        attribute: removal.name.clone(),
                        uuid: removal.uuid.clone(),
                        field: removal.field.name.clone(),
                        number: removal.number,
                        indexes: removal.indexes.clone(),
                    })
                    .collect(),
                widenings: object
                    .widenings
                    .iter()
                    .map(|widening| WideningReport {
                        attribute: widening.name.clone(),
                        uuid: widening.uuid.clone(),
                        field: widening.after.name.clone(),
                        number: widening.number,
                        from: widening.from,
                        to: widening.to,
                    })
                    .collect(),
                switches: object
                    .switches
                    .iter()
                    .map(|switch| SwitchReport {
                        attribute: switch.name.clone(),
                        uuid: switch.uuid.clone(),
                        field: switch.field.clone(),
                        from: indexing_word(switch.from).to_owned(),
                        to: indexing_word(switch.to).to_owned(),
                        added: switch.added.clone(),
                        removed: switch.removed.clone(),
                    })
                    .collect(),
                sections: object
                    .sections
                    .iter()
                    .map(|section| SectionReport {
                        section: section.name.clone(),
                        uuid: section.uuid.clone(),
                        table: section.table.clone(),
                        table_number: section.created.map(|created| created.table_number),
                        line_no: section.created.map(|created| created.line_no),
                        attributes: section
                            .additions
                            .iter()
                            .map(|addition| AdditionReport {
                                attribute: addition.name.clone(),
                                uuid: addition.uuid.clone(),
                                field: addition.field.name.clone(),
                                number: addition.number,
                                position: addition.position,
                            })
                            .collect(),
                        indexes: section.indexes.clone(),
                    })
                    .collect(),
                tables: object
                    .tables
                    .iter()
                    .map(|table| TableReport {
                        created: table.create,
                        table: table.table.name.clone(),
                        columns: table.table.columns.len(),
                        indexes: table
                            .table
                            .indexes
                            .iter()
                            .map(|index| {
                                format!(
                                    "{}{}{} ({})",
                                    if index.unique { "unique " } else { "" },
                                    if index.clustered { "clustered " } else { "" },
                                    if index.name.is_empty() {
                                        "primary key"
                                    } else {
                                        &index.name
                                    },
                                    index.columns.join(", ")
                                )
                            })
                            .collect(),
                        copy_columns: table.insert_columns.len(),
                    })
                    .collect(),
            })
            .collect(),
        caches: plan
            .caches
            .iter()
            .map(|cache| format!("{}: {}", cache.row_name, cache.what))
            .collect(),
        old_schema_sha256: plan.old_schema_sha256.clone(),
        new_schema_sha256: sha256_hex(&plan.new_schema),
        new_names_sha256: sha256_hex(&plan.new_names_text),
        statements: plan
            .statements()
            .into_iter()
            .map(|statement| StatementReport {
                phase: format!("{:?}", statement.phase).to_ascii_lowercase(),
                label: statement.label,
                sql: statement.sql.chars().take(400).collect(),
            })
            .collect(),
        execution: None,
        size_check: None,
        through_apply: None,
    }
}

fn dump(plan: &Plan, folder: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(folder)?;
    std::fs::write(folder.join("DBSchema.new.txt"), &plan.new_schema)?;
    std::fs::write(folder.join("DBNames.new.txt"), &plan.new_names_text)?;
    let mut script = String::new();
    for statement in plan.statements() {
        script.push_str(&format!(
            "-- {:?}: {}\n{}\n\n",
            statement.phase, statement.label, statement.sql
        ));
    }
    std::fs::write(folder.join("restructure.sql"), script)?;
    Ok(())
}
