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
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::profile::{CapabilityId, CapabilityState, EffectiveProfile};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::sql::{SqlBackend, SqlExec, SqlLogin, SqlTarget};

/// Capability that admits main-configuration writes for a platform profile.
pub const CAPABILITY_MAIN_WRITE: &str = "mssql.main.write";
/// The own exclusive `config apply` (`mssql-config-apply`): a stage that needs no
/// restructuring moved from `ConfigSave` into `Config` in one transaction.
pub const CAPABILITY_CONFIG_APPLY: &str = "mssql.config.apply";
/// The dynamic (online) `config apply` of the drop-in (`--dynamic=force`): a small delta stage
/// published as a generation while sessions are connected, with the writes the platform's own
/// `force` makes besides (`docs/apply/dropin-dynamic.md`).
pub const CAPABILITY_CONFIG_APPLY_DYNAMIC: &str = "mssql.config.apply.dynamic";
/// Capability that admits extension writes for a platform profile.
pub const CAPABILITY_EXTENSION_WRITE: &str = "mssql.extension.write";
/// Profile fingerprint key for `IBVersion`/`PlatformVersionReq`.
pub const FINGERPRINT_IB_VERSION: &str = "mssql.ibversion";
/// Profile fingerprint key for the canonical five-table schema digest.
pub const FINGERPRINT_CONFIG_SCHEMA: &str = "mssql.config-schema.sha256";

/// Native platform layouts that may be selected by MSSQL commands.
///
/// The enum keeps the command line closed; what each build may do is declared
/// by its bundled profile in `profiles/platform`, never by a match arm here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
pub enum MssqlNativePlatformProfile {
    #[value(name = "platform-8.3.27.1989")]
    Platform8_3_27_1989,
    #[value(name = "platform-8.3.27.2214")]
    Platform8_3_27_2214,
    #[value(name = "platform-8.5.1.1150")]
    Platform8_5_1_1150,
}

impl MssqlNativePlatformProfile {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Platform8_3_27_1989 => "platform-8.3.27.1989",
            Self::Platform8_3_27_2214 => "platform-8.3.27.2214",
            Self::Platform8_5_1_1150 => "platform-8.5.1.1150",
        }
    }

    /// Resolves the bundled declaration that owns this build's policy.
    pub fn effective_profile(self) -> Result<EffectiveProfile> {
        let registry = crate::profile_registry::load_bundled_profile_registry()?;
        let id = ProfileId::parse(self.id())
            .map_err(|error| anyhow!("invalid platform profile id `{}`: {error}", self.id()))?;
        registry
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("platform profile `{}` is not bundled", self.id()))
    }

    /// Admits an operation only when the profile declares the capability as
    /// `supported`. An undeclared capability fails closed exactly like an
    /// explicit `unsupported` declaration.
    fn require_capability(self, capability: &str) -> Result<()> {
        let profile = self.effective_profile()?;
        let id = CapabilityId::parse(capability)
            .map_err(|error| anyhow!("invalid capability id `{capability}`: {error}"))?;
        match profile.capabilities.get(&id).map(|entry| entry.value) {
            Some(CapabilityState::Supported) => Ok(()),
            Some(CapabilityState::Unsupported) => bail!(
                "capability `{capability}` is explicitly unsupported for platform profile `{}`",
                self.id()
            ),
            None => bail!(
                "capability `{capability}` is not declared for platform profile `{}`; declare it in profiles/platform/ only with native evidence",
                self.id()
            ),
        }
    }

    /// Main writes require an evidenced activation protocol for the build.
    pub fn require_main_write_supported(self) -> Result<()> {
        self.require_capability(CAPABILITY_MAIN_WRITE)
    }

    /// The own exclusive apply requires that its end state was compared with
    /// the native apply on this build.
    pub fn require_config_apply_supported(self) -> Result<()> {
        self.require_capability(CAPABILITY_CONFIG_APPLY)
    }

    /// The dynamic apply requires that its generation, its change registrations and
    /// `MobileVersions.dat` were compared with the native `--dynamic=force` on this build.
    /// 8.5.1.1150 admission is additionally restricted by the dynamic planner to
    /// its measured initial five-row CommonModule or module-only CommonForm cohort.
    pub fn require_config_apply_dynamic_supported(self) -> Result<()> {
        self.require_capability(CAPABILITY_CONFIG_APPLY_DYNAMIC)
    }

    /// Native 8.5.1.1150 re-stamps only the final block of its signed `root`
    /// payload even on a body-only import. This does not admit another root,
    /// unsigned/signed conversion, or a different payload layout. Callers must
    /// still bind both complete physical rows and judge the staged metadata.
    pub fn accepts_dynamic_root_restamp(self, stored: &[u8], staged: &[u8]) -> bool {
        if self != Self::Platform8_5_1_1150 {
            return false;
        }
        fn parts(bytes: &[u8]) -> Option<(Uuid, Vec<u8>)> {
            // Bound the complete inflated row before parsing; measured native
            // rows are 222 bytes and have no whitespace outside the braces.
            if bytes.len() > 300 {
                return None;
            }
            let text =
                std::str::from_utf8(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes)).ok()?;
            let inner = text.strip_prefix('{')?.strip_suffix('}')?;
            let mut fields = inner.splitn(3, ',');
            if fields.next()? != "2" {
                return None;
            }
            let identity = fields.next()?;
            let uuid = Uuid::parse_str(identity).ok()?;
            if uuid.hyphenated().to_string() != identity {
                return None;
            }
            let encoded = fields.next()?;
            // Bound before decoding: the measured 128-byte payload is 172
            // base64 characters; native CR/LF wrapping is the only extra data.
            if encoded.len() > 256
                || encoded.chars().any(|c| {
                    !(c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '\r' | '\n'))
                })
            {
                return None;
            }
            let payload = crate::module_blob::decode_base64_mime(encoded)?;
            let canonical: String = encoded
                .chars()
                .filter(|c| !matches!(c, '\r' | '\n'))
                .collect();
            (payload.len() == 128 && crate::module_blob::encode_base64(&payload) == canonical)
                .then_some((uuid, payload))
        }
        match (parts(stored), parts(staged)) {
            (Some((old_id, old)), Some((new_id, new))) => {
                old_id == new_id && old[..112] == new[..112]
            }
            _ => false,
        }
    }

    /// Extension mutation requires an evidenced CAS/registry protocol.
    pub fn require_extension_write_supported(self) -> Result<()> {
        self.require_capability(CAPABILITY_EXTENSION_WRITE)
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
    /// sqlcmd.exe for the schema probe (`--sqlcmd`); the built-in SQL client
    /// otherwise.
    pub sqlcmd: Option<&'a Path>,
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
    verify_mssql_native_profile_with_auth(claimed, options, None)
}

/// Internal credentials created by the owned creator. This is neither a
/// serialized ownership proof nor a public endpoint-adoption switch.
pub(crate) struct ManagedRacReadAuth<'a> {
    pub agent_user: &'a str,
    pub agent_password: &'a str,
    pub cluster_user: &'a str,
    pub cluster_password: &'a str,
}

impl ManagedRacReadAuth<'_> {
    fn arguments(&self, agent: bool) -> Vec<String> {
        let (prefix, user, password) = if agent {
            ("agent", self.agent_user, self.agent_password)
        } else {
            ("cluster", self.cluster_user, self.cluster_password)
        };
        vec![
            format!("--{prefix}-user={user}"),
            format!("--{prefix}-pwd={password}"),
        ]
    }

    fn redact(&self, value: &str) -> String {
        redact_managed_secrets(value, &[self.agent_password, self.cluster_password])
    }
}

fn redact_managed_secrets(value: &str, secrets: &[&str]) -> String {
    let mut secrets: Vec<_> = secrets
        .iter()
        .copied()
        .filter(|secret| !secret.is_empty())
        .collect();
    // A shorter password may be a prefix of another credential.
    secrets.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    secrets
        .into_iter()
        .fold(value.to_owned(), |message, secret| {
            message.replace(secret, "[redacted]")
        })
}

pub(crate) fn verify_mssql_native_profile_managed_observation(
    claimed: MssqlNativePlatformProfile,
    options: MssqlNativeProfileVerificationOptions<'_>,
    authentication: &ManagedRacReadAuth<'_>,
    agent_version: &str,
    registration: &str,
) -> Result<MssqlNativeProfileVerification> {
    let secrets = [
        authentication.agent_password,
        authentication.cluster_password,
        options.infobase_pwd.unwrap_or_default(),
        options.sql_pwd.unwrap_or_default(),
    ];
    let result = (|| -> Result<_> {
        if options.sqlcmd.is_some() {
            bail!("managed profile requires the built-in SQL client");
        }
        let cluster_id = options
            .cluster_id
            .context("managed profile cluster UUID absent")?;
        let infobase_id = options
            .infobase_id
            .context("managed profile infobase UUID absent")?;
        require_registered_database(
            &parse_rac_blocks(registration),
            &infobase_id.to_string(),
            options.server,
            options.database,
        )?;
        let build = parse_rac_agent_build(agent_version)?;
        let probe = parse_probe(&run_probe(&options)?)?;
        verify_probe(
            claimed,
            &build,
            probe,
            RasDatabaseBinding {
                cluster_id,
                infobase_id,
            },
        )
    })();
    result
        // Native failure text can echo argv. Redact the complete error chain.
        .map_err(|error| {
            anyhow!(
                "{}",
                redact_managed_secrets(&format!("{error:#}"), &secrets)
            )
        })
}

#[cfg(test)]
mod managed_auth_tests {
    use super::*;

    #[test]
    fn managed_auth_uses_separate_agent_cluster_flags_and_redacts_all_credentials() {
        let auth = ManagedRacReadAuth {
            agent_user: "generated-agent",
            agent_password: "abc",
            cluster_user: "generated-cluster",
            cluster_password: "abcdef",
        };
        assert_eq!(
            auth.arguments(true),
            ["--agent-user=generated-agent", "--agent-pwd=abc"]
        );
        assert_eq!(
            auth.arguments(false),
            ["--cluster-user=generated-cluster", "--cluster-pwd=abcdef"]
        );
        assert_eq!(
            auth.redact("native echoed abcdef then abc"),
            "native echoed [redacted] then [redacted]"
        );
        assert_eq!(
            redact_managed_secrets(
                "abc/abcdef/IBpassword/SQLpassword",
                &["abc", "abcdef", "IBpassword", "SQLpassword", ""]
            ),
            "[redacted]/[redacted]/[redacted]/[redacted]"
        );
    }
}

fn verify_mssql_native_profile_with_auth(
    claimed: MssqlNativePlatformProfile,
    options: MssqlNativeProfileVerificationOptions<'_>,
    authentication: Option<&ManagedRacReadAuth<'_>>,
) -> Result<MssqlNativeProfileVerification> {
    let agent_build = if let Some(authentication) = authentication {
        let mut args = vec!["agent".to_owned(), "version".to_owned()];
        args.extend(authentication.arguments(true));
        args.push(options.ras_endpoint.to_owned());
        parse_rac_agent_build(&run_rac_bounded(options.rac, args)?)?
    } else {
        read_rac_agent_build(options.rac, options.ras_endpoint)?
    };
    let binding = verify_ras_infobase_binding(
        options.rac,
        options.ras_endpoint,
        options.server,
        options.database,
        options.cluster_id,
        options.infobase_id,
        options.infobase_user,
        options.infobase_pwd,
        authentication,
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

pub(crate) fn parse_rac_agent_build(output: &str) -> Result<String> {
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
    authentication: Option<&ManagedRacReadAuth<'_>>,
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
    if let Some(authentication) = authentication {
        args.extend(authentication.arguments(false));
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

/// A worker process of the cluster that holds an infobase only for the RAS
/// service's own connections: where the SQL sessions of the tool's own RAS
/// verification live ([`read_infobase_clients`]).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OwnRasProcess {
    /// The machine the process runs on (`host_name` of its SQL sessions).
    pub host: String,
    /// Its operating-system process id (`host_process_id` of its SQL sessions).
    pub pid: u32,
}

/// What the cluster says about who is connected to one infobase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasInfobaseClients {
    /// Connections and sessions of the infobase that are not the RAS service's
    /// own: a client, a background job, a web or COM connection, ...
    pub clients: Vec<String>,
    /// The worker processes that carry a RAS connection of the infobase and no
    /// other connection or session of it.
    pub ras_only_processes: BTreeSet<OwnRasProcess>,
}

/// Asks the cluster who is connected to the infobase: `rac connection list`,
/// `rac session list` and `rac process list`, without the infobase login (which
/// would itself open a RAS connection).
///
/// The tool's own RAS verification (`rac infobase info --infobase-user`) makes
/// the worker process load the infobase, and the process keeps two idle
/// `1CV83 Server` SQL sessions on the database for as long as its RAS
/// connection lives (#409 F-3: observed for 56 minutes after the command
/// ended). They are not users, but the SQL gate of an exclusive activation
/// counts every session.
pub fn read_infobase_clients(
    rac: &Path,
    ras_endpoint: &str,
    cluster_id: Uuid,
    infobase_id: Uuid,
) -> Result<RasInfobaseClients> {
    let list = |what: &str, extra: Vec<String>| -> Result<Vec<BTreeMap<String, String>>> {
        let mut args = vec![what.to_owned(), "list".to_owned()];
        args.push(format!("--cluster={cluster_id}"));
        args.extend(extra);
        args.push(ras_endpoint.to_owned());
        run_rac_bounded(rac, args)
            .map(|output| parse_rac_blocks(&output))
            .with_context(|| format!("rac {what} list failed"))
    };
    let infobase = format!("--infobase={infobase_id}");
    let connections = list("connection", vec![infobase.clone()])?;
    let sessions = list("session", vec![infobase])?;
    let processes = list("process", Vec::new())?;
    Ok(infobase_clients(&connections, &sessions, &processes))
}

fn infobase_clients(
    connections: &[BTreeMap<String, String>],
    sessions: &[BTreeMap<String, String>],
    processes: &[BTreeMap<String, String>],
) -> RasInfobaseClients {
    let field = |block: &BTreeMap<String, String>, key: &str| -> String {
        block.get(key).cloned().unwrap_or_default()
    };
    let mut clients = Vec::new();
    let mut ras_processes = BTreeSet::new();
    let mut client_processes = BTreeSet::new();
    for connection in connections {
        let process = field(connection, "process");
        let application = field(connection, "application");
        // An application the cluster does not name is not RAS: it counts.
        if application.eq_ignore_ascii_case("RAS") {
            ras_processes.insert(process);
        } else {
            client_processes.insert(process);
            clients.push(format!(
                "connection {} of application {application:?} on {}",
                field(connection, "conn-id"),
                field(connection, "host")
            ));
        }
    }
    for session in sessions {
        client_processes.insert(field(session, "process"));
        clients.push(format!(
            "session {} of user {:?} ({})",
            field(session, "session-id"),
            field(session, "user-name"),
            field(session, "app-id")
        ));
    }
    let ras_only_processes = processes
        .iter()
        .filter_map(|process| {
            let id = field(process, "process");
            if !ras_processes.contains(&id) || client_processes.contains(&id) {
                return None;
            }
            Some(OwnRasProcess {
                host: field(process, "host"),
                pid: field(process, "pid").parse().ok()?,
            })
        })
        .collect();
    RasInfobaseClients {
        clients,
        ras_only_processes,
    }
}

/// For an exclusive activation: refuses, before anything is written, an
/// infobase that has client connections or sessions, and returns the worker
/// processes whose idle SQL sessions the gate of the activation leaves out
/// ([`exclusive_session_gate`]) because they belong to the RAS service.
pub fn own_ras_processes_for_exclusive(
    rac: &Path,
    ras_endpoint: &str,
    cluster_id: Uuid,
    infobase_id: Uuid,
) -> Result<Vec<OwnRasProcess>> {
    let found = read_infobase_clients(rac, ras_endpoint, cluster_id, infobase_id)?;
    if !found.clients.is_empty() {
        bail!(
            "exclusive activation refused before any write: the cluster reports {} client connection(s) or session(s) on the infobase ({}); end them, or activate `online`",
            found.clients.len(),
            found
                .clients
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    Ok(found.ras_only_processes.into_iter().collect())
}

/// The statement of an exclusive activation that refuses when the database has
/// a session other than its own: `THROW code, message`.
///
/// `own` are the worker processes that hold the infobase only for RAS
/// connections ([`own_ras_processes_for_exclusive`]). Their `1CV83 Server` SQL
/// sessions are left out while they are idle -- `sleeping`, no open
/// transaction -- because the tool's own RAS verification made the process open
/// them; a session of that program that runs or holds a transaction, and every
/// session of another program or another process, still refuses. Without `own`
/// the statement counts every session, as it always did.
///
/// One case is not caught: a user who signs in through the same process in the
/// seconds between the cluster query and the transaction, while the sessions
/// stay idle.
pub fn exclusive_session_gate(code: u32, message: &str, own: &[OwnRasProcess]) -> String {
    let exempt = session_exemption(own);
    format!(
        "IF EXISTS (SELECT 1 FROM sys.dm_exec_sessions WHERE is_user_process=1 AND session_id<>@@SPID AND database_id=DB_ID(){exempt}) THROW {code}, '{}', 1;",
        message.replace('\'', "''")
    )
}

/// The sessions the exclusive gates leave out, as a condition to append to a
/// `WHERE` over `sys.dm_exec_sessions`: none when `own` is empty.
pub(crate) fn session_exemption(own: &[OwnRasProcess]) -> String {
    if own.is_empty() {
        String::new()
    } else {
        let mut by_host = BTreeMap::<&str, Vec<u32>>::new();
        for process in own {
            by_host.entry(&process.host).or_default().push(process.pid);
        }
        let alternatives = by_host
            .iter()
            .map(|(host, pids)| {
                format!(
                    "(ISNULL(host_name,N'')=N'{}' AND ISNULL(host_process_id,-1) IN ({}))",
                    host.replace('\'', "''"),
                    pids.iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect::<Vec<_>>()
            .join(" OR ");
        format!(
            " AND NOT (ISNULL(program_name,N'')=N'1CV83 Server' AND status=N'sleeping' AND open_transaction_count=0 AND ({alternatives}))"
        )
    }
}

/// The sessions the gate of an exclusive activation would count, as a query for
/// a connection that is not on the database: `session_id, login, host, program,
/// status`. Same condition as [`exclusive_session_gate`].
pub fn foreign_sessions_query(database: &str, own: &[OwnRasProcess]) -> String {
    format!(
        "SELECT session_id, ISNULL(login_name,N''), ISNULL(host_name,N''), ISNULL(program_name,N''), status FROM sys.dm_exec_sessions WHERE is_user_process=1 AND session_id<>@@SPID AND database_id=DB_ID(N'{}'){} ORDER BY session_id",
        database.replace('\'', "''"),
        session_exemption(own)
    )
}

/// For an exclusive activation: refuses, before anything is staged, a database
/// that has a session the gate of the activation would count.
///
/// The gate inside the transaction stays the last word; this only moves its
/// refusal before the stage, so that it leaves `ConfigSave` as it was (#409: an
/// apply refused by the gate used to leave the stage behind). A connection
/// through `--sqlcmd` cannot ask; it is left to the gate.
pub fn refuse_foreign_sessions(sql: &SqlExec, database: &str, own: &[OwnRasProcess]) -> Result<()> {
    let SqlBackend::Client(client) = sql.backend() else {
        return Ok(());
    };
    let rows = client
        .query_rows(&foreign_sessions_query(database, own), &[])
        .context("failed to look at the sessions of the database")?;
    if rows.is_empty() {
        return Ok(());
    }
    let mut listed = Vec::new();
    for row in rows.iter().take(5) {
        listed.push(format!(
            "session {} of {:?} on {:?} ({:?}, {})",
            row.i64(0)?,
            row.text(1)?,
            row.text(2)?,
            row.text(3)?,
            row.text(4)?
        ));
    }
    bail!(
        "exclusive activation refused before any write: the database has {} other session(s) ({}); end them, or activate `online`",
        rows.len(),
        listed.join("; ")
    );
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
    let sql_text = "SET NOCOUNT ON;\n\
         IF OBJECT_ID(N'dbo.IBVersion', N'U') IS NULL THROW 57320, 'IBVersion table is missing', 1;\n\
         SELECT CONCAT(N'IDENTITY|', IBVersion, N'|', PlatformVersionReq) FROM dbo.IBVersion;\n\
         SELECT CONCAT(N'COLUMN|', t.name, N'|', c.column_id, N'|', c.name, N'|', TYPE_NAME(c.user_type_id), N'|', c.max_length, N'|', c.precision, N'|', c.scale, N'|', CONVERT(int, c.is_nullable))\n\
         FROM sys.tables t JOIN sys.columns c ON c.object_id = t.object_id\n\
         WHERE t.name IN (N'Config', N'ConfigSave', N'Params', N'ConfigCAS', N'ConfigCASSave')\n\
         ORDER BY t.name, c.column_id;";
    let password = match options.sql_user {
        Some(_) => Some(
            options
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
                })?,
        ),
        None => {
            if options.sql_pwd.is_some_and(|value| !value.is_empty()) {
                bail!("--sql-pwd requires --sql-user");
            }
            None
        }
    };
    let Some(sqlcmd) = options.sqlcmd else {
        // The query builds each output line itself (`CONCAT`), one column a row.
        let sql = SqlExec::sql_server(SqlTarget {
            server: options.server.to_owned(),
            database: Some(options.database.to_owned()),
            login: SqlLogin::from_user(options.sql_user, password.as_deref()),
            trust_server_certificate: options.sqlcmd_trust_cert,
        })?;
        let client = sql
            .client()
            .ok_or_else(|| anyhow!("the built-in SQL client is not available"))?;
        let mut lines = String::new();
        client
            .read_rows(sql_text, &[], &mut |row| {
                lines.push_str(&row.value(0)?.to_text());
                lines.push('\n');
                if lines.len() > MAX_PROBE_OUTPUT_BYTES {
                    bail!(
                        "native profile verification output exceeds {MAX_PROBE_OUTPUT_BYTES} bytes"
                    );
                }
                Ok(())
            })
            .context("native profile verification query failed")?;
        return Ok(lines);
    };
    let mut command = Command::new(sqlcmd);
    command.arg("-S").arg(options.server);
    match (options.sql_user, password) {
        (Some(user), Some(password)) => {
            command.arg("-U").arg(user);
            command.env("SQLCMDPASSWORD", password);
        }
        _ => {
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
            .arg(sql_text),
    )
    .with_context(|| format!("failed to launch sqlcmd at {}", sqlcmd.display()))?;
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
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            bail!("native profile/RAS probe exceeded its 20-second deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
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

/// Compares one live observation with the value the profile declares. A profile
/// that declares no value for the key fails closed: a write must never rely on
/// an unevidenced storage shape.
fn require_declared_fingerprint(
    profile: &EffectiveProfile,
    key: &str,
    observed: &str,
) -> Result<()> {
    match profile.fingerprints.get(key) {
        Some(declared) if declared.value == observed => Ok(()),
        Some(declared) => bail!(
            "live `{key}` is `{observed}` but platform profile `{}` declares `{}`",
            profile.id.as_str(),
            declared.value
        ),
        None => bail!(
            "platform profile `{}` declares no `{key}` fingerprint; native writes stay closed until it is evidenced",
            profile.id.as_str()
        ),
    }
}

/// Compares a live storage probe with what the claimed profile declares:
/// the exact five-table column layout, then the profile's `IBVersion` and
/// schema fingerprints. Returns the schema fingerprint.
fn check_probe_against_profile(
    claimed: MssqlNativePlatformProfile,
    probe: &NativeStorageProbe,
) -> Result<String> {
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
    let observed_ib_version = format!("{}|{}", probe.ib_version, probe.platform_version_req);
    let profile = claimed.effective_profile()?;
    require_declared_fingerprint(&profile, FINGERPRINT_IB_VERSION, &observed_ib_version)?;
    require_declared_fingerprint(&profile, FINGERPRINT_CONFIG_SCHEMA, &observed_fingerprint)?;
    Ok(observed_fingerprint)
}

fn verify_probe(
    claimed: MssqlNativePlatformProfile,
    agent_build: &str,
    probe: NativeStorageProbe,
    binding: RasDatabaseBinding,
) -> Result<MssqlNativeProfileVerification> {
    let claimed_build = claimed
        .id()
        .strip_prefix("platform-")
        .expect("closed platform profile has a platform- prefix");
    let storage_schema_sha256 = check_probe_against_profile(claimed, &probe)?;
    if agent_build != claimed_build {
        bail!(
            "claimed platform profile `{}` does not match RAS agent build `{agent_build}`",
            claimed.id()
        );
    }
    Ok(MssqlNativeProfileVerification {
        claimed_platform_profile: claimed.id().to_owned(),
        verified_platform_profile: format!("platform-{agent_build}"),
        storage_schema_sha256,
        ib_version: probe.ib_version,
        platform_version_req: probe.platform_version_req,
        verified_cluster_id: binding.cluster_id,
        verified_infobase_id: binding.infobase_id,
        identity_source: "rac_agent_exact_build_plus_registered_infobase_plus_live_sql_schema",
    })
}

/// What a SQL-only verification of the storage layout established.
#[derive(Debug, Clone, Serialize)]
pub struct MssqlStorageVerification {
    pub claimed_platform_profile: String,
    pub storage_schema_sha256: String,
    pub ib_version: i32,
    pub platform_version_req: i32,
}

/// The storage half of [`verify_mssql_native_profile`] for commands that run
/// against the database alone (no cluster, no RAS): the live column layout of
/// the five configuration tables and `IBVersion` must equal what the claimed
/// profile declares, and the profile must admit the own exclusive apply
/// (`mssql.config.apply`).
/// The platform *build* is then the caller's claim, not an observation.
pub fn verify_mssql_storage_profile(
    claimed: MssqlNativePlatformProfile,
    client: &dyn crate::sql::SqlClient,
    database: &str,
) -> Result<MssqlStorageVerification> {
    claimed.require_config_apply_supported()?;
    let db = format!("[{}]", database.replace(']', "]]"));
    let sql_text = format!(
        "SET NOCOUNT ON;
         IF OBJECT_ID(N'{db}.dbo.IBVersion', N'U') IS NULL THROW 57320, 'IBVersion table is missing', 1;
         SELECT CONCAT(N'IDENTITY|', IBVersion, N'|', PlatformVersionReq) FROM {db}.dbo.IBVersion;
         SELECT CONCAT(N'COLUMN|', t.name, N'|', c.column_id, N'|', c.name, N'|', TYPE_NAME(c.user_type_id), N'|', c.max_length, N'|', c.precision, N'|', c.scale, N'|', CONVERT(int, c.is_nullable))
         FROM {db}.sys.tables t JOIN {db}.sys.columns c ON c.object_id = t.object_id
         WHERE t.name IN (N'Config', N'ConfigSave', N'Params', N'ConfigCAS', N'ConfigCASSave')
         ORDER BY t.name, c.column_id;"
    );
    let mut lines = String::new();
    client
        .read_rows(&sql_text, &[], &mut |row| {
            lines.push_str(&row.value(0)?.to_text());
            lines.push('\n');
            if lines.len() > MAX_PROBE_OUTPUT_BYTES {
                bail!("native profile verification output exceeds {MAX_PROBE_OUTPUT_BYTES} bytes");
            }
            Ok(())
        })
        .context("native profile verification query failed")?;
    let probe = parse_probe(&lines)?;
    let storage_schema_sha256 = check_probe_against_profile(claimed, &probe)?;
    Ok(MssqlStorageVerification {
        claimed_platform_profile: claimed.id().to_owned(),
        storage_schema_sha256,
        ib_version: probe.ib_version,
        platform_version_req: probe.platform_version_req,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_85_root_restamp_uses_native_bounded_rows_and_refuses_other_layouts() {
        let profile = MssqlNativePlatformProfile::Platform8_5_1_1150;
        let old = include_bytes!("../tests/fixtures/platform85-dynamic/root-active.native.bin");
        let new = include_bytes!("../tests/fixtures/platform85-dynamic/root-staged.native.bin");
        assert_ne!(old, new);
        assert!(profile.accepts_dynamic_root_restamp(old, new));
        assert!(
            !MssqlNativePlatformProfile::Platform8_3_27_2214.accepts_dynamic_root_restamp(old, new)
        );
        assert!(
            !MssqlNativePlatformProfile::Platform8_3_27_1989.accepts_dynamic_root_restamp(old, new)
        );
        let new = std::str::from_utf8(new).unwrap();
        for invalid in [
            new.replacen("{2,", "{3,", 1),
            new.replacen("66193438", "76193438", 1),
            new.replacen("66193438-abc5", "66193438-ABC5", 1),
            new.replacen("PiyN", "QiyN", 1),
            format!(
                "{{2,66193438-abc5-410b-a1f1-a204102d1a62,{}}}",
                crate::module_blob::encode_base64(&[0; 112])
            ),
            format!(
                "{{2,66193438-abc5-410b-a1f1-a204102d1a62,{}}}",
                crate::module_blob::encode_base64(&[0; 144])
            ),
            format!(
                "{{2,66193438-abc5-410b-a1f1-a204102d1a62,{}}}",
                "A".repeat(1024)
            ),
            "{2,66193438-abc5-410b-a1f1-a204102d1a62,}".to_owned(),
            format!("\u{2003}{new}"),
            format!("{new}\u{2003}"),
            format!("{}{new}", " ".repeat(4096)),
            format!("{new}{}", "\r\n".repeat(2048)),
        ] {
            assert!(
                !profile.accepts_dynamic_root_restamp(old, invalid.as_bytes()),
                "{invalid}"
            );
        }
        assert!(!profile.accepts_dynamic_root_restamp(b"invalid", new.as_bytes()));
    }

    // What `rac` printed on the lab cluster (8.3.27.2214) for the clone that
    // the tool's own verification had opened: one RAS connection, two worker
    // processes.
    const RAS_CONNECTIONS: &str = "connection     : d243edc4-d9a1-43b5-8b19-910671df20fd\nconn-id        : 104390\nhost           : DESKTOP-SMI5N4O\nprocess        : 68c054ba-34d3-4900-9c08-ead9756a9cf1\ninfobase       : 3855ed5b-ea8f-4960-a88e-6f746c8a5601\napplication    : \"RAS\"\nconnected-at   : 2026-09-30T01:28:35\nsession-number : 0\nblocked-by-ls  : 0\n";
    const RAS_PROCESSES: &str = "process              : 68c054ba-34d3-4900-9c08-ead9756a9cf1\nhost                 : DESKTOP-SMI5N4O\nport                 : 2560\npid                  : 22608\nturned-on            : yes\nrunning              : yes\nconnections          : 14\n\nprocess              : 5f8f3e05-2c7b-4ebf-8d6e-b0e6eed2b64a\nhost                 : DESKTOP-SMI5N4O\nport                 : 2564\npid                  : 110352\nturned-on            : yes\nrunning              : yes\nconnections          : 10\n";

    fn clients_of(connections: &str, sessions: &str, processes: &str) -> RasInfobaseClients {
        infobase_clients(
            &parse_rac_blocks(connections),
            &parse_rac_blocks(sessions),
            &parse_rac_blocks(processes),
        )
    }

    fn own(host: &str, pid: u32) -> OwnRasProcess {
        OwnRasProcess {
            host: host.to_owned(),
            pid,
        }
    }

    #[test]
    fn a_ras_connection_alone_is_no_client_and_names_its_process() {
        let found = clients_of(RAS_CONNECTIONS, "", RAS_PROCESSES);
        assert!(found.clients.is_empty(), "{:?}", found.clients);
        assert_eq!(
            found.ras_only_processes,
            BTreeSet::from([own("DESKTOP-SMI5N4O", 22608)])
        );
    }

    #[test]
    fn a_client_connection_or_session_is_counted_and_takes_its_process_out() {
        // The same process also carries a client of the infobase.
        let with_client = format!(
            "{RAS_CONNECTIONS}\nconnection     : 5d6d5f08-6c7c-4d4b-8f2e-000000000001\nconn-id        : 104391\nhost           : DESKTOP-SMI5N4O\nprocess        : 68c054ba-34d3-4900-9c08-ead9756a9cf1\ninfobase       : 3855ed5b-ea8f-4960-a88e-6f746c8a5601\napplication    : \"1CV8C\"\nconnected-at   : 2026-09-30T01:30:00\nsession-number : 3\nblocked-by-ls  : 0\n"
        );
        let found = clients_of(&with_client, "", RAS_PROCESSES);
        assert_eq!(found.clients.len(), 1);
        assert!(found.clients[0].contains("1CV8C"), "{:?}", found.clients);
        assert!(found.ras_only_processes.is_empty());

        // A session with no connection listed still counts.
        let found = clients_of(
            RAS_CONNECTIONS,
            "session        : 1\nsession-id     : 7\ninfobase       : 3855ed5b-ea8f-4960-a88e-6f746c8a5601\nconnection     : 5d6d5f08-6c7c-4d4b-8f2e-000000000001\nprocess        : 5f8f3e05-2c7b-4ebf-8d6e-b0e6eed2b64a\nuser-name      : Иванов\napp-id         : 1CV8C\n",
            RAS_PROCESSES,
        );
        assert_eq!(found.clients.len(), 1);
        assert_eq!(
            found.ras_only_processes,
            BTreeSet::from([own("DESKTOP-SMI5N4O", 22608)])
        );

        // No application name is not RAS.
        let unnamed = RAS_CONNECTIONS.replace("application    : \"RAS\"\n", "");
        let found = clients_of(&unnamed, "", RAS_PROCESSES);
        assert_eq!(found.clients.len(), 1);
        assert!(found.ras_only_processes.is_empty());
    }

    #[test]
    fn an_empty_cluster_answer_means_no_client_and_nothing_to_leave_out() {
        let found = clients_of("", "", RAS_PROCESSES);
        assert!(found.clients.is_empty());
        assert!(found.ras_only_processes.is_empty());
    }

    #[test]
    fn the_pre_stage_query_counts_what_the_gate_counts() {
        let own = [own("DESKTOP-SMI5N4O", 22608)];
        let gate = exclusive_session_gate(57209, "m", &own);
        let query = foreign_sessions_query("lab'db", &own);
        // The same condition, on the named database instead of the current one.
        let condition = gate
            .split_once("WHERE ")
            .and_then(|(_, rest)| rest.split_once(") THROW"))
            .map(|(condition, _)| condition.replace("DB_ID()", "DB_ID(N'lab''db')"))
            .unwrap();
        assert!(query.contains(&condition), "{query}\n{condition}");
        assert!(query.starts_with("SELECT session_id, ISNULL(login_name,N'')"));
        assert!(query.ends_with(" ORDER BY session_id"));
        // No process named: every session.
        let plain = foreign_sessions_query("db", &[]);
        assert!(plain.contains("database_id=DB_ID(N'db') ORDER BY session_id"));
        assert!(!plain.contains("1CV83 Server"));
    }

    #[test]
    fn the_exclusive_gate_counts_every_session_unless_told_which_are_the_rac_sessions() {
        assert_eq!(
            exclusive_session_gate(
                57209,
                "exclusive activation requires no other database sessions",
                &[]
            ),
            "IF EXISTS (SELECT 1 FROM sys.dm_exec_sessions WHERE is_user_process=1 AND session_id<>@@SPID AND database_id=DB_ID()) THROW 57209, 'exclusive activation requires no other database sessions', 1;"
        );
        let text = exclusive_session_gate(
            57209,
            "no other sessions",
            &[
                own("DESKTOP-SMI5N4O", 22608),
                own("DESKTOP-SMI5N4O", 110352),
                own("o'host", 7),
            ],
        );
        assert!(text.contains("AND NOT (ISNULL(program_name,N'')=N'1CV83 Server' AND status=N'sleeping' AND open_transaction_count=0 AND ("));
        assert!(text.contains(
            "(ISNULL(host_name,N'')=N'DESKTOP-SMI5N4O' AND ISNULL(host_process_id,-1) IN (22608,110352))"
        ));
        assert!(
            text.contains(
                "(ISNULL(host_name,N'')=N'o''host' AND ISNULL(host_process_id,-1) IN (7))"
            )
        );
        assert!(text.ends_with(") THROW 57209, 'no other sessions', 1;"));
    }

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
    fn write_policy_comes_from_the_bundled_profile_declaration() {
        MssqlNativePlatformProfile::Platform8_3_27_2214
            .require_extension_write_supported()
            .expect("8.3.27.2214 extension writes are evidenced");
        MssqlNativePlatformProfile::Platform8_3_27_2214
            .require_main_write_supported()
            .expect("8.3.27.2214 main writes are evidenced");

        // 8.3.27.1989 keeps its read evidence but declares no write capability.
        let undeclared = MssqlNativePlatformProfile::Platform8_3_27_1989
            .require_main_write_supported()
            .expect_err("an undeclared capability must fail closed");
        assert!(undeclared.to_string().contains("is not declared"));

        let explicit = MssqlNativePlatformProfile::Platform8_5_1_1150
            .require_main_write_supported()
            .expect_err("8.5 main activation must fail closed");
        assert!(explicit.to_string().contains("explicitly unsupported"));
        let explicit_extension = MssqlNativePlatformProfile::Platform8_5_1_1150
            .require_extension_write_supported()
            .expect_err("8.5 extension writes must fail closed");
        assert!(explicit_extension.to_string().contains("8.5.1.1150"));
    }

    #[test]
    fn the_own_apply_is_declared_per_build_and_apart_from_main_write() {
        // the own exclusive apply has a capability of its own: 8.5.1 leaves
        // main-configuration writes (the live generation switch) unsupported and
        // still admits the apply, once it was compared with the native one there
        MssqlNativePlatformProfile::Platform8_3_27_2214
            .require_config_apply_supported()
            .expect("8.3.27.2214 apply is compared with the native one");
        MssqlNativePlatformProfile::Platform8_5_1_1150
            .require_config_apply_supported()
            .expect("8.5.1.1150 apply is compared with the native one");
        // 8.3.27.1989 declares no write capability of any kind
        let undeclared = MssqlNativePlatformProfile::Platform8_3_27_1989
            .require_config_apply_supported()
            .expect_err("an undeclared capability must fail closed");
        assert!(undeclared.to_string().contains("is not declared"));
    }

    #[test]
    fn initial_85_dynamic_capability_does_not_enable_other_builds_or_activation_modes() {
        let exact = MssqlNativePlatformProfile::Platform8_5_1_1150;
        exact.require_config_apply_dynamic_supported().unwrap();
        // Main activation owns ONLINE/LIVE/WORKER; they remain closed even
        // though the separately guarded drop-in dynamic publication is admitted.
        assert!(exact.require_main_write_supported().is_err());
        assert!(exact.require_extension_write_supported().is_err());
        exact.require_config_apply_supported().unwrap(); // released 0.4 policy

        MssqlNativePlatformProfile::Platform8_3_27_2214
            .require_config_apply_dynamic_supported()
            .unwrap();
        assert!(
            MssqlNativePlatformProfile::Platform8_3_27_1989
                .require_config_apply_dynamic_supported()
                .is_err()
        );
        for unknown in ["platform-8.5.1.1529", "platform-8.5.1.1151", "platform-8.5"] {
            assert!(<MssqlNativePlatformProfile as ValueEnum>::from_str(unknown, false).is_err());
        }
        // The same schema never proves a different server executable build.
        assert!(verify_probe(exact, "8.5.1.1529", evidenced_probe(), test_binding()).is_err());
        assert!(verify_probe(exact, "8.3.27.2214", evidenced_probe(), test_binding()).is_err());
        assert!(verify_probe(exact, "8.5.1.1150", evidenced_probe(), test_binding()).is_ok());
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
            MssqlNativePlatformProfile::Platform8_3_27_2214,
            "8.3.27.2214",
            evidenced_probe(),
            test_binding(),
        )
        .unwrap();
        assert_eq!(verified.verified_platform_profile, "platform-8.3.27.2214");
        assert_eq!(
            verified.storage_schema_sha256,
            "49ab07a8ddb8c87ae1bdc47b1b5342dc2341dbf99cc777ede57ad8ec539bc0a3"
        );

        let build_error = verify_probe(
            MssqlNativePlatformProfile::Platform8_3_27_2214,
            "8.3.27.1989",
            evidenced_probe(),
            test_binding(),
        )
        .unwrap_err();
        assert!(build_error.to_string().contains("does not match RAS agent"));

        let mut wrong_schema = evidenced_probe();
        wrong_schema.columns.pop();
        let schema_error = verify_probe(
            MssqlNativePlatformProfile::Platform8_3_27_2214,
            "8.3.27.2214",
            wrong_schema,
            test_binding(),
        )
        .unwrap_err();
        assert!(schema_error.to_string().contains("does not exactly match"));

        let mut wrong_identity = evidenced_probe();
        wrong_identity.platform_version_req = 80327;
        let identity_error = verify_probe(
            MssqlNativePlatformProfile::Platform8_3_27_2214,
            "8.3.27.2214",
            wrong_identity,
            test_binding(),
        )
        .unwrap_err();
        assert!(
            identity_error
                .to_string()
                .contains("live `mssql.ibversion` is `7|80327`")
        );
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
