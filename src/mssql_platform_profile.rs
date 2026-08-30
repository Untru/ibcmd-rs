//! Closed selection and write policy for native MSSQL storage layouts.
//!
//! This axis is deliberately independent from the XML source dialect: an XML
//! 2.20/2.21 document does not prove which native database layout is safe to
//! mutate.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use clap::ValueEnum;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Native platform layouts that may be selected by MSSQL commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
pub enum MssqlNativePlatformProfile {
    #[value(name = "platform-8.3.27.1989")]
    Platform8_3_27_1989,
    #[value(name = "platform-8.5.1.1150")]
    Platform8_5_1_1150,
}

impl MssqlNativePlatformProfile {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Platform8_3_27_1989 => "platform-8.3.27.1989",
            Self::Platform8_5_1_1150 => "platform-8.5.1.1150",
        }
    }

    /// Main writes stay closed until the complete activation protocol is
    /// evidenced.  On 8.5 the table shape matches 8.3, but native dynamic
    /// apply also rewrites generation-selection `.ui` rows in `Params`.
    pub fn require_main_write_supported(self) -> Result<()> {
        match self {
            Self::Platform8_3_27_1989 => Ok(()),
            Self::Platform8_5_1_1150 => bail!(
                "MSSQL main writes are not supported for platform profile `{}`; the 8.5 generation-selection Params protocol is not yet evidenced",
                self.id()
            ),
        }
    }

    /// Extension mutation still has evidence only for 8.3.27.
    pub fn require_extension_write_supported(self) -> Result<()> {
        match self {
            Self::Platform8_3_27_1989 => Ok(()),
            Self::Platform8_5_1_1150 => bail!(
                "MSSQL native writes are not supported for platform profile `{}`; 8.5 is currently read-only while storage-layout evidence is collected",
                self.id()
            ),
        }
    }
}

/// Actual database evidence bound to a claimed native platform profile.
#[derive(Debug, Clone)]
pub struct MssqlNativeProfileVerification {
    pub claimed_platform_profile: String,
    pub verified_platform_profile: String,
    pub storage_schema_sha256: String,
    pub ib_version: i32,
    pub platform_version_req: i32,
    pub verified_cluster_id: Uuid,
    pub verified_infobase_id: Uuid,
    pub identity_source: &'static str,
}

pub struct MssqlNativeProfileVerificationOptions<'a> {
    pub sqlcmd: &'a Path,
    pub rac: &'a Path,
    pub ras_endpoint: &'a str,
    pub server: &'a str,
    pub database: &'a str,
    pub cluster_id: Option<Uuid>,
    pub infobase_id: Option<Uuid>,
    pub infobase_user: Option<&'a str>,
    pub infobase_pwd: Option<&'a str>,
    pub sql_user: Option<&'a str>,
    pub sql_pwd: Option<&'a str>,
    pub sql_pwd_env: &'a str,
    pub sqlcmd_trust_cert: bool,
}

#[derive(Debug)]
struct NativeStorageProbe {
    ib_version: i32,
    platform_version_req: i32,
    columns: Vec<String>,
}

#[derive(Debug)]
struct RasDatabaseBinding {
    cluster_id: Uuid,
    infobase_id: Uuid,
}

/// Verify both the live native schema and the exact RAS agent build. SQL
/// storage alone does not expose the exact 1C executable build, and the five
/// configuration tables have the same seven-column shape on the observed 8.3
/// and 8.5 databases. Consequently a CLI enum alone never establishes build
/// identity.
pub fn verify_mssql_native_profile(
    claimed: MssqlNativePlatformProfile,
    options: MssqlNativeProfileVerificationOptions<'_>,
) -> Result<MssqlNativeProfileVerification> {
    let agent_build = read_rac_agent_build(options.rac, options.ras_endpoint)?;
    let binding = verify_ras_infobase_binding(
        options.rac,
        options.ras_endpoint,
        options.server,
        options.database,
        options.cluster_id,
        options.infobase_id,
        options.infobase_user,
        options.infobase_pwd,
    )?;
    let output = run_probe(&options)?;
    verify_probe(claimed, &agent_build, parse_probe(&output)?, binding)
}

fn read_rac_agent_build(rac: &Path, ras_endpoint: &str) -> Result<String> {
    let mut command = Command::new(rac);
    command.arg("agent").arg("version").arg(ras_endpoint);
    let output = bounded_output(&mut command)
        .with_context(|| format!("failed to launch rac at {}", rac.display()))?;
    if !output.status.success() {
        bail!(
            "rac agent version failed: stdout={} stderr={}",
            output.stdout,
            output.stderr
        );
    }
    parse_rac_agent_build(&output.stdout)
}

fn parse_rac_agent_build(output: &str) -> Result<String> {
    let builds = output
        .split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .filter(|token| {
            token.split('.').count() == 4
                && token
                    .split('.')
                    .all(|component| !component.is_empty() && component.parse::<u32>().is_ok())
        })
        .collect::<BTreeSet<_>>();
    if builds.len() != 1 {
        bail!("rac agent version did not return exactly one four-part platform build");
    }
    Ok((*builds.first().expect("one build was checked")).to_owned())
}

fn verify_ras_infobase_binding(
    rac: &Path,
    ras_endpoint: &str,
    sql_server: &str,
    database: &str,
    cluster_id: Option<Uuid>,
    infobase_id: Option<Uuid>,
    infobase_user: Option<&str>,
    infobase_pwd: Option<&str>,
) -> Result<RasDatabaseBinding> {
    let cluster_id = cluster_id.ok_or_else(|| {
        anyhow!("native MSSQL write verification requires --cluster-id to bind RAS to SQL")
    })?;
    let infobase_id = infobase_id.ok_or_else(|| {
        anyhow!("native MSSQL write verification requires --infobase-id to bind RAS to SQL")
    })?;
    let mut args = vec![
        "infobase".to_owned(),
        "info".to_owned(),
        format!("--cluster={cluster_id}"),
        format!("--infobase={infobase_id}"),
    ];
    if let Some(user) = infobase_user {
        args.push(format!("--infobase-user={user}"));
        args.push(format!(
            "--infobase-pwd={}",
            infobase_pwd.unwrap_or_default()
        ));
    } else if infobase_pwd.is_some() {
        bail!("--infobase-pwd requires --infobase-user");
    }
    args.push(ras_endpoint.to_owned());
    let registration = run_rac_bounded(rac, args)?;
    require_registered_database(
        &parse_rac_blocks(&registration),
        &infobase_id.to_string(),
        sql_server,
        database,
    )?;
    Ok(RasDatabaseBinding {
        cluster_id,
        infobase_id,
    })
}

fn run_rac_bounded<I>(rac: &Path, args: I) -> Result<String>
where
    I: IntoIterator<Item = String>,
{
    let mut command = Command::new(rac);
    command.args(args);
    let output = bounded_output(&mut command)
        .with_context(|| format!("failed to launch rac at {}", rac.display()))?;
    if !output.status.success() {
        bail!(
            "rac database binding query failed: stdout={} stderr={}",
            output.stdout,
            output.stderr
        );
    }
    Ok(output.stdout)
}

fn parse_rac_blocks(text: &str) -> Vec<BTreeMap<String, String>> {
    let mut blocks = Vec::new();
    let mut current = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() {
            if !current.is_empty() {
                blocks.push(std::mem::take(&mut current));
            }
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            current.insert(
                key.trim().to_owned(),
                value.trim().trim_matches('"').to_owned(),
            );
        }
    }
    if !current.is_empty() {
        blocks.push(current);
    }
    blocks
}

fn require_registered_database(
    registrations: &[BTreeMap<String, String>],
    infobase_id: &str,
    sql_server: &str,
    database: &str,
) -> Result<()> {
    let expected_server = normalized_sql_server(sql_server);
    let matching = registrations
        .iter()
        .filter(|entry| {
            entry
                .get("infobase")
                .is_some_and(|value| value.eq_ignore_ascii_case(infobase_id))
                && entry
                    .get("db-name")
                    .is_some_and(|value| value.eq_ignore_ascii_case(database))
                && entry
                    .get("db-server")
                    .is_some_and(|value| normalized_sql_server(value) == expected_server)
                && entry
                    .get("dbms")
                    .is_some_and(|value| value == "MSSQLServer")
        })
        .count();
    if matching != 1 {
        bail!(
            "target SQL database `{sql_server}/{database}` must match exact MSSQL infobase registration `{infobase_id}` on the verified RAS endpoint; observed {matching}"
        );
    }
    Ok(())
}

fn normalized_sql_server(value: &str) -> String {
    let normalized = value
        .trim()
        .trim_matches('"')
        .strip_prefix("tcp:")
        .unwrap_or(value.trim().trim_matches('"'))
        .to_ascii_lowercase();
    if normalized.contains(['\\', ',']) {
        return normalized;
    }
    let computer_name = std::env::var("COMPUTERNAME")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "." | "(local)" | "localhost" | "127.0.0.1" | "::1"
    ) || (!computer_name.is_empty() && normalized == computer_name)
    {
        "local".to_owned()
    } else {
        normalized
    }
}

fn run_probe(options: &MssqlNativeProfileVerificationOptions<'_>) -> Result<String> {
    let sql = "SET NOCOUNT ON;\n\
         IF OBJECT_ID(N'dbo.IBVersion', N'U') IS NULL THROW 57320, 'IBVersion table is missing', 1;\n\
         SELECT CONCAT(N'IDENTITY|', IBVersion, N'|', PlatformVersionReq) FROM dbo.IBVersion;\n\
         SELECT CONCAT(N'COLUMN|', t.name, N'|', c.column_id, N'|', c.name, N'|', TYPE_NAME(c.user_type_id), N'|', c.max_length, N'|', c.precision, N'|', c.scale, N'|', CONVERT(int, c.is_nullable))\n\
         FROM sys.tables t JOIN sys.columns c ON c.object_id = t.object_id\n\
         WHERE t.name IN (N'Config', N'ConfigSave', N'Params', N'ConfigCAS', N'ConfigCASSave')\n\
         ORDER BY t.name, c.column_id;";
    let mut command = Command::new(options.sqlcmd);
    command.arg("-S").arg(options.server);
    match options.sql_user {
        Some(user) => {
            command.arg("-U").arg(user);
            let password = options
                .sql_pwd
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| {
                    std::env::var(options.sql_pwd_env)
                        .ok()
                        .filter(|value| !value.is_empty())
                })
                .ok_or_else(|| {
                    anyhow!("SQL login requires --sql-pwd or {}", options.sql_pwd_env)
                })?;
            command.env("SQLCMDPASSWORD", password);
        }
        None => {
            if options.sql_pwd.is_some_and(|value| !value.is_empty()) {
                bail!("--sql-pwd requires --sql-user");
            }
            command.arg("-E");
        }
    }
    if options.sqlcmd_trust_cert {
        command.arg("-C");
    }
    let output = bounded_output(
        command
            .arg("-d")
            .arg(options.database)
            .arg("-f")
            .arg("65001")
            .arg("-b")
            .arg("-h")
            .arg("-1")
            .arg("-W")
            .arg("-w")
            .arg("65535")
            .arg("-Q")
            .arg(sql),
    )
    .with_context(|| format!("failed to launch sqlcmd at {}", options.sqlcmd.display()))?;
    if !output.status.success() {
        bail!(
            "native profile verification query failed: stdout={} stderr={}",
            output.stdout,
            output.stderr
        );
    }
    Ok(output.stdout)
}

const MAX_PROBE_OUTPUT_BYTES: usize = 64 * 1024;

struct BoundedOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn bounded_output(command: &mut Command) -> Result<BoundedOutput> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().expect("piped stdout is present");
    let stderr = child.stderr.take().expect("piped stderr is present");
    let stdout_reader = std::thread::spawn(move || read_bounded(stdout));
    let stderr_reader = std::thread::spawn(move || read_bounded(stderr));
    let status = child.wait()?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow!("stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow!("stderr reader panicked"))??;
    Ok(BoundedOutput {
        status,
        stdout,
        stderr,
    })
}

fn read_bounded(mut reader: impl Read) -> Result<String> {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut exceeded = false;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = MAX_PROBE_OUTPUT_BYTES.saturating_sub(retained.len());
        retained.extend_from_slice(&buffer[..count.min(remaining)]);
        exceeded |= count > remaining;
    }
    if exceeded {
        bail!("probe output exceeded {MAX_PROBE_OUTPUT_BYTES} bytes");
    }
    Ok(String::from_utf8_lossy(&retained).to_string())
}

fn parse_probe(output: &str) -> Result<NativeStorageProbe> {
    let mut identity = None;
    let mut columns = Vec::new();
    for line in output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(value) = line.strip_prefix("IDENTITY|") {
            if identity.is_some() {
                bail!("IBVersion must contain exactly one row");
            }
            let fields = value.split('|').collect::<Vec<_>>();
            if fields.len() != 2 {
                bail!("native profile identity row has an unexpected shape");
            }
            identity = Some((fields[0].parse::<i32>()?, fields[1].parse::<i32>()?));
        } else if line.starts_with("COLUMN|") {
            if line.split('|').count() != 9 {
                bail!("native storage column row has an unexpected shape");
            }
            columns.push(line.to_owned());
        } else {
            bail!("native profile verification returned unexpected output: {line}");
        }
    }
    let (ib_version, platform_version_req) =
        identity.ok_or_else(|| anyhow!("IBVersion returned no identity row"))?;
    Ok(NativeStorageProbe {
        ib_version,
        platform_version_req,
        columns,
    })
}

fn verify_probe(
    claimed: MssqlNativePlatformProfile,
    agent_build: &str,
    probe: NativeStorageProbe,
    binding: RasDatabaseBinding,
) -> Result<MssqlNativeProfileVerification> {
    let required_tables = [
        "Config",
        "ConfigCAS",
        "ConfigCASSave",
        "ConfigSave",
        "Params",
    ];
    let expected_columns = [
        (1, "FileName", "nvarchar", 256, 0, 0, 0),
        (2, "Creation", "datetime2", 6, 19, 0, 0),
        (3, "Modified", "datetime2", 6, 19, 0, 0),
        (4, "Attributes", "smallint", 2, 5, 0, 0),
        (5, "DataSize", "bigint", 8, 19, 0, 0),
        (6, "BinaryData", "varbinary", -1, 0, 0, 0),
        (7, "PartNo", "int", 4, 10, 0, 0),
    ];
    let expected = required_tables
        .iter()
        .flat_map(|table| {
            expected_columns.iter().map(move |column| {
                format!(
                    "COLUMN|{table}|{}|{}|{}|{}|{}|{}|{}",
                    column.0, column.1, column.2, column.3, column.4, column.5, column.6
                )
            })
        })
        .collect::<Vec<_>>();
    if probe.columns != expected {
        bail!("native storage schema does not exactly match the evidenced five-table fingerprint");
    }
    if probe.ib_version != 7 || probe.platform_version_req != 80313 {
        bail!(
            "unsupported IBVersion identity: IBVersion={}, PlatformVersionReq={}",
            probe.ib_version,
            probe.platform_version_req
        );
    }
    let claimed_build = claimed
        .id()
        .strip_prefix("platform-")
        .expect("closed platform profile has a platform- prefix");
    if agent_build != claimed_build {
        bail!(
            "claimed platform profile `{}` does not match RAS agent build `{agent_build}`",
            claimed.id()
        );
    }
    let mut digest = Sha256::new();
    digest.update(format!(
        "IBVersion|{}|{}\n",
        probe.ib_version, probe.platform_version_req
    ));
    for line in &probe.columns {
        digest.update(line.as_bytes());
        digest.update(b"\n");
    }
    let observed_fingerprint = format!("{:x}", digest.finalize());
    Ok(MssqlNativeProfileVerification {
        claimed_platform_profile: claimed.id().to_owned(),
        verified_platform_profile: format!("platform-{agent_build}"),
        storage_schema_sha256: observed_fingerprint,
        ib_version: probe.ib_version,
        platform_version_req: probe.platform_version_req,
        verified_cluster_id: binding.cluster_id,
        verified_infobase_id: binding.infobase_id,
        identity_source: "rac_agent_exact_build_plus_registered_infobase_plus_live_sql_schema",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_binding() -> RasDatabaseBinding {
        RasDatabaseBinding {
            cluster_id: Uuid::nil(),
            infobase_id: Uuid::nil(),
        }
    }

    fn evidenced_probe() -> NativeStorageProbe {
        let mut columns = Vec::new();
        let shapes = [
            (1, "FileName", "nvarchar", 256, 0, 0, 0),
            (2, "Creation", "datetime2", 6, 19, 0, 0),
            (3, "Modified", "datetime2", 6, 19, 0, 0),
            (4, "Attributes", "smallint", 2, 5, 0, 0),
            (5, "DataSize", "bigint", 8, 19, 0, 0),
            (6, "BinaryData", "varbinary", -1, 0, 0, 0),
            (7, "PartNo", "int", 4, 10, 0, 0),
        ];
        for table in [
            "Config",
            "ConfigCAS",
            "ConfigCASSave",
            "ConfigSave",
            "Params",
        ] {
            for shape in shapes {
                columns.push(format!(
                    "COLUMN|{table}|{}|{}|{}|{}|{}|{}|{}",
                    shape.0, shape.1, shape.2, shape.3, shape.4, shape.5, shape.6
                ));
            }
        }
        NativeStorageProbe {
            ib_version: 7,
            platform_version_req: 80313,
            columns,
        }
    }

    #[test]
    fn only_evidenced_8_3_profile_allows_native_writes() {
        MssqlNativePlatformProfile::Platform8_3_27_1989
            .require_extension_write_supported()
            .expect("8.3.27 write profile is evidenced");
        let error = MssqlNativePlatformProfile::Platform8_5_1_1150
            .require_extension_write_supported()
            .expect_err("8.5 writes must fail closed");
        assert!(error.to_string().contains("currently read-only"));
        let main_error = MssqlNativePlatformProfile::Platform8_5_1_1150
            .require_main_write_supported()
            .expect_err("8.5 main activation must fail closed");
        assert!(main_error.to_string().contains("Params protocol"));
    }

    #[test]
    fn rac_agent_build_parser_is_exact_and_unambiguous() {
        assert_eq!(
            parse_rac_agent_build("version : 8.5.1.1150\r\n").unwrap(),
            "8.5.1.1150"
        );
        assert!(parse_rac_agent_build("version: unknown").is_err());
        assert!(parse_rac_agent_build("8.3.27.1989 8.5.1.1150").is_err());
    }

    #[test]
    fn claimed_profile_must_match_agent_and_live_storage_shape() {
        let verified = verify_probe(
            MssqlNativePlatformProfile::Platform8_3_27_1989,
            "8.3.27.1989",
            evidenced_probe(),
            test_binding(),
        )
        .unwrap();
        assert_eq!(verified.verified_platform_profile, "platform-8.3.27.1989");
        assert_eq!(verified.storage_schema_sha256.len(), 64);

        let build_error = verify_probe(
            MssqlNativePlatformProfile::Platform8_3_27_1989,
            "8.5.1.1150",
            evidenced_probe(),
            test_binding(),
        )
        .unwrap_err();
        assert!(build_error.to_string().contains("does not match RAS agent"));

        let mut wrong_schema = evidenced_probe();
        wrong_schema.columns.pop();
        let schema_error = verify_probe(
            MssqlNativePlatformProfile::Platform8_3_27_1989,
            "8.3.27.1989",
            wrong_schema,
            test_binding(),
        )
        .unwrap_err();
        assert!(schema_error.to_string().contains("does not exactly match"));
    }

    #[test]
    fn registered_infobase_binding_is_exact_and_accepts_local_aliases() {
        let registrations = parse_rac_blocks(
            "infobase : a\ndbms : MSSQLServer\ndb-server : localhost\ndb-name : bsp\n\ninfobase : b\ndbms : MSSQLServer\ndb-server : remote\ndb-name : bsp\n",
        );
        assert!(require_registered_database(&registrations, "a", "127.0.0.1", "BSP").is_ok());
        assert!(require_registered_database(&registrations, "b", "remote", "bsp").is_ok());
        assert!(require_registered_database(&registrations, "a", "other", "bsp").is_err());
        assert!(require_registered_database(&registrations, "a", "localhost", "missing").is_err());

        let wrong_dbms = parse_rac_blocks(
            "infobase : a\ndbms : PostgreSQL\ndb-server : localhost\ndb-name : bsp\n",
        );
        assert!(require_registered_database(&wrong_dbms, "a", "localhost", "bsp").is_err());
    }
}
