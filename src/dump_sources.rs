#![cfg(feature = "platform-oracle")]
//! Research-only source export through an installed ibcmd executable.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::cli::DumpSourcesArgs;
use crate::runtime_evidence_schema::{SanitizedRuntimeArgumentKind, SubprocessJournalSchema};

#[derive(Debug, Serialize)]
pub struct DumpSourcesReport {
    pub ibcmd: PathBuf,
    pub dbms: String,
    pub db_server: String,
    pub db_name: String,
    pub db_user: Option<String>,
    pub password_source: String,
    pub infobase_user: Option<String>,
    pub infobase_password_source: Option<String>,
    pub extension: Option<String>,
    pub output_dir: PathBuf,
    pub data_dir: PathBuf,
    pub temp_export_dir: PathBuf,
    pub duration_ms: u128,
    pub exit_code: Option<i32>,
    pub file_count: usize,
    pub stdout: String,
    pub stderr: String,
    pub runtime_call: SanitizedRuntimeCall,
}

#[derive(Clone, Debug, Serialize)]
pub struct SanitizedRuntimeCall {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub started_unix_ms: u128,
    pub ended_unix_ms: Option<u128>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub exception: Option<String>,
}

#[derive(Debug, Serialize)]
struct DumpSourcesRuntimeJournal {
    protocol_version: u32,
    runtime_call: SanitizedRuntimeCall,
}

struct DumpConfig {
    ibcmd: PathBuf,
    dbms: String,
    db_server: String,
    db_name: String,
    db_user: Option<String>,
    db_pwd: String,
    password_source: String,
    infobase_user: Option<String>,
    infobase_password: Option<String>,
    infobase_password_source: Option<String>,
    output_dir: PathBuf,
    extension: Option<String>,
    data_dir: PathBuf,
    timeout: Duration,
    overwrite: bool,
    normalize_taxi_old: bool,
}

pub fn dump_sources(args: &DumpSourcesArgs) -> Result<DumpSourcesReport> {
    let config = resolve_config(args)?;
    let temp_export_dir = make_temp_dir("ibcmd-rs-export")?;

    let mut arguments = vec![
        "infobase".to_owned(),
        "config".to_owned(),
        "export".to_owned(),
        format!("--dbms={}", config.dbms),
        format!("--db-server={}", config.db_server),
        format!("--db-name={}", config.db_name),
        format!("--data={}", config.data_dir.display()),
    ];
    let mut sanitized_arguments = arguments.clone();
    if let Some(db_user) = &config.db_user {
        arguments.push(format!("--db-user={db_user}"));
        arguments.push(format!("--db-pwd={}", config.db_pwd));
        sanitized_arguments.push(format!("--db-user={db_user}"));
        sanitized_arguments.push("--db-pwd".to_owned());
        sanitized_arguments.push(password_source_marker(&config.password_source).to_owned());
    }

    if let Some(user) = &config.infobase_user {
        arguments.push(format!("--user={user}"));
        sanitized_arguments.push(format!("--user={user}"));
    }
    if let Some(password) = &config.infobase_password {
        arguments.push(format!("--password={password}"));
        sanitized_arguments.push("--password".to_owned());
        sanitized_arguments.push(
            password_source_marker(
                config
                    .infobase_password_source
                    .as_deref()
                    .unwrap_or("unknown"),
            )
            .to_owned(),
        );
    }

    if let Some(extension) = &config.extension {
        arguments.push(format!("--extension={extension}"));
        sanitized_arguments.push(format!("--extension={extension}"));
    }

    arguments.push("--force".to_owned());
    arguments.push(temp_export_dir.display().to_string());
    sanitized_arguments.push("--force".to_owned());
    sanitized_arguments.push(temp_export_dir.display().to_string());
    redact_password_values(
        &mut sanitized_arguments,
        &[
            (
                config.db_pwd.as_str(),
                password_source_marker(&config.password_source),
            ),
            (
                config.infobase_password.as_deref().unwrap_or_default(),
                password_source_marker(
                    config
                        .infobase_password_source
                        .as_deref()
                        .unwrap_or("unknown"),
                ),
            ),
        ],
    );

    let started = Instant::now();
    let mut runtime_call = SanitizedRuntimeCall {
        executable: config.ibcmd.clone(),
        arguments: sanitized_arguments,
        started_unix_ms: unix_time_ms(),
        ended_unix_ms: None,
        status: "running".to_owned(),
        exit_code: None,
        timed_out: false,
        exception: None,
    };
    persist_runtime_journal(args.runtime_journal.as_deref(), &runtime_call)?;

    let mut command = Command::new(&config.ibcmd);
    command.args(&arguments);
    let mut output = run_with_runtime_journal(
        command,
        config.timeout,
        args.runtime_journal.as_deref(),
        &mut runtime_call,
    )?;
    redact_runtime_output(
        &mut output.stdout,
        &[
            config.db_pwd.as_str(),
            config.infobase_password.as_deref().unwrap_or_default(),
        ],
    );
    redact_runtime_output(
        &mut output.stderr,
        &[
            config.db_pwd.as_str(),
            config.infobase_password.as_deref().unwrap_or_default(),
        ],
    );
    let duration_ms = started.elapsed().as_millis();

    if output.timed_out {
        bail!(
            "ibcmd export timed out after {} seconds\nibcmd: {}\ndatabase: {}\ninfobase_user: {}\ndata_dir: {}\ntemp_export_dir: {}",
            config.timeout.as_secs(),
            config.ibcmd.display(),
            config.db_name,
            config.infobase_user.as_deref().unwrap_or("<none>"),
            config.data_dir.display(),
            temp_export_dir.display()
        );
    }

    if !output.success {
        bail!(
            "ibcmd export failed with exit code {:?}\nibcmd: {}\ndatabase: {}\ninfobase_user: {}\ndata_dir: {}\ntemp_export_dir: {}\nstdout:\n{}\nstderr:\n{}",
            output.exit_code,
            config.ibcmd.display(),
            config.db_name,
            config.infobase_user.as_deref().unwrap_or("<none>"),
            config.data_dir.display(),
            temp_export_dir.display(),
            output.stdout,
            output.stderr
        );
    }

    if config.normalize_taxi_old {
        normalize_configuration_xml(&temp_export_dir)?;
    }

    mirror_export_to_output(&temp_export_dir, &config.output_dir, config.overwrite)?;
    let file_count = count_files(&config.output_dir)?;

    Ok(DumpSourcesReport {
        ibcmd: config.ibcmd.clone(),
        dbms: config.dbms,
        db_server: config.db_server,
        db_name: config.db_name,
        db_user: config.db_user,
        password_source: config.password_source,
        infobase_user: config.infobase_user,
        infobase_password_source: config.infobase_password_source,
        extension: config.extension,
        output_dir: config.output_dir,
        data_dir: config.data_dir,
        temp_export_dir,
        duration_ms,
        exit_code: output.exit_code,
        file_count,
        stdout: output.stdout,
        stderr: output.stderr,
        runtime_call,
    })
}

fn persist_runtime_journal(path: Option<&Path>, runtime_call: &SanitizedRuntimeCall) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    let journal = DumpSourcesRuntimeJournal {
        protocol_version: 1,
        runtime_call: runtime_call.clone(),
    };
    write_json_atomic(path, &journal)
        .with_context(|| format!("failed to persist runtime journal {}", path.display()))
}

fn run_with_runtime_journal(
    command: Command,
    timeout: Duration,
    journal_path: Option<&Path>,
    runtime_call: &mut SanitizedRuntimeCall,
) -> Result<ProcessOutput> {
    let output = match run_with_timeout(command, timeout) {
        Ok(output) => output,
        Err(error) => {
            runtime_call.ended_unix_ms = Some(unix_time_ms());
            runtime_call.status = "failed".to_owned();
            runtime_call.exception = Some(format!("failed to execute nested ibcmd: {error:#}"));
            persist_runtime_journal(journal_path, runtime_call)?;
            return Err(error).context("nested ibcmd execution failed");
        }
    };
    runtime_call.ended_unix_ms = Some(unix_time_ms());
    runtime_call.exit_code = output.exit_code;
    runtime_call.timed_out = output.timed_out;
    runtime_call.status = if output.success {
        "passed".to_owned()
    } else {
        "failed".to_owned()
    };
    runtime_call.exception = if output.timed_out {
        Some(format!(
            "nested ibcmd timed out after {} seconds",
            timeout.as_secs()
        ))
    } else if !output.success {
        Some(format!(
            "nested ibcmd exited with code {:?}",
            output.exit_code
        ))
    } else {
        None
    };
    persist_runtime_journal(journal_path, runtime_call)?;
    Ok(output)
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("journal path must have a parent: {}", path.display()))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("runtime-journal"),
        uuid::Uuid::new_v4().simple()
    ));
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    if let Err(error) = replace_file_atomic(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file_atomic(source: &Path, destination: &Path) -> Result<()> {
    fs::rename(source, destination)?;
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("journal path must have a parent: {}", destination.display()))?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn replace_file_atomic(source: &Path, destination: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        return Err(std::io::Error::last_os_error()).context("MoveFileExW failed");
    }
    Ok(())
}

fn unix_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn password_source_marker(source: &str) -> &'static str {
    if source == "integrated" {
        "<password-source:none>"
    } else if source == "--db-pwd" || source == "--password" {
        "<password-source:inline>"
    } else if source == "settings" {
        "<password-source:settings>"
    } else {
        "<password-source:environment>"
    }
}

fn redact_password_values(arguments: &mut [String], values: &[(&str, &str)]) {
    let mut values = values
        .iter()
        .copied()
        .filter(|(value, _)| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.0.len().cmp(&left.0.len()).then(left.0.cmp(right.0)));
    for (value, marker) in values {
        for argument in arguments.iter_mut() {
            if SubprocessJournalSchema::argument_kind(argument)
                == SanitizedRuntimeArgumentKind::Ordinary
                && argument.contains(value)
            {
                // Preserve only schema-recognized evidence verbatim.  An
                // ordinary argument containing a secret collapses to the
                // standalone marker, preventing prefix/suffix or mixed-marker
                // strings from acquiring evidence semantics in the journal.
                *argument = marker.to_owned();
            }
        }
    }
}

fn redact_runtime_output(text: &mut String, values: &[&str]) {
    let mut values = values
        .iter()
        .copied()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort_by(|left, right| right.len().cmp(&left.len()).then(left.cmp(right)));
    for value in values {
        *text = text.replace(value, "<redacted-sensitive-runtime-value>");
    }
}

fn resolve_config(args: &DumpSourcesArgs) -> Result<DumpConfig> {
    let settings = match &args.settings {
        Some(path) => Some(read_settings(path)?),
        None => None,
    };

    let dbms = first_value(args.dbms.as_deref(), settings_value(&settings, "dbms-type"))
        .unwrap_or_else(|| "MSSQLServer".to_string());
    let db_server = first_value(
        args.db_server.as_deref(),
        settings_value(&settings, "dbms-server"),
    )
    .unwrap_or_else(|| "localhost".to_string());
    let db_name = first_value(
        args.db_name.as_deref(),
        settings_value(&settings, "dbms-base"),
    )
    .ok_or_else(|| anyhow!("database name is required: pass --db-name or --settings"))?;
    let db_user = first_value(
        args.db_user.as_deref(),
        settings_value(&settings, "dbms-user"),
    )
    .or_else(|| env::var("IBCMD_DB_USR").ok());

    let (db_pwd, password_source) = if db_user.is_none() {
        (String::new(), "integrated".to_string())
    } else {
        match &args.db_pwd {
            Some(value) => (value.clone(), "--db-pwd".to_string()),
            None => {
                if let Ok(value) = env::var(&args.db_pwd_env) {
                    (value, "environment (redacted)".to_string())
                } else if let Some(value) = settings_value(&settings, "dbms-pwd") {
                    (value, "settings".to_string())
                } else {
                    bail!(
                        "database password is required when --db-user is set: pass --db-pwd, set {}, or use --settings",
                        args.db_pwd_env
                    );
                }
            }
        }
    };
    let infobase_user = first_value(
        args.user.as_deref(),
        settings_value_any(&settings, &["ib-user", "user", "usr"]),
    )
    .or_else(|| env::var("IBCMD_USR").ok());
    let (infobase_password, infobase_password_source) = resolve_optional_infobase_password(
        infobase_user.as_ref(),
        args.password.as_deref(),
        &settings,
        &args.password_env,
    )?;

    let ibcmd = resolve_ibcmd(args.ibcmd.as_deref())?;
    let output_dir = absolute_path(&args.output_dir)?;
    let data_dir = match &args.data_dir {
        Some(path) => absolute_path(path)?,
        None => make_temp_dir("ibcmd-rs-data")?,
    };

    Ok(DumpConfig {
        ibcmd,
        dbms,
        db_server,
        db_name,
        db_user,
        db_pwd,
        password_source,
        infobase_user,
        infobase_password,
        infobase_password_source,
        output_dir,
        extension: args.extension.clone(),
        data_dir,
        timeout: Duration::from_secs(args.timeout_sec),
        overwrite: args.overwrite,
        normalize_taxi_old: args.normalize_taxi_old,
    })
}

fn read_settings(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read settings {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn settings_value(settings: &Option<Value>, name: &str) -> Option<String> {
    settings
        .as_ref()?
        .get("vrunner")?
        .get(name)?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn settings_value_any(settings: &Option<Value>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        settings_value(settings, name).or_else(|| settings_string_at(settings, &["ibcmd-rs", name]))
    })
}

fn settings_string_at(settings: &Option<Value>, path: &[&str]) -> Option<String> {
    let mut current = settings.as_ref()?;
    for segment in path {
        current = current.get(*segment)?;
    }
    current
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn resolve_optional_infobase_password(
    user: Option<&String>,
    cli_password: Option<&str>,
    settings: &Option<Value>,
    password_env: &str,
) -> Result<(Option<String>, Option<String>)> {
    if user.is_none() {
        return Ok((None, None));
    }
    if let Some(value) = cli_password.filter(|value| !value.is_empty()) {
        return Ok((Some(value.to_string()), Some("--password".to_string())));
    }
    if let Ok(value) = env::var(password_env) {
        return Ok((Some(value), Some("environment (redacted)".to_string())));
    }
    if let Some(value) = settings_value_any(settings, &["ib-pwd", "password", "pwd"]) {
        return Ok((Some(value), Some("settings".to_string())));
    }
    bail!(
        "infobase password is required when --user is set: pass --password, set {password_env}, or use --settings"
    )
}

fn first_value(cli: Option<&str>, settings: Option<String>) -> Option<String> {
    cli.filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
        .or(settings)
}

pub(crate) fn resolve_ibcmd(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        if path.is_file() {
            return absolute_path(path);
        }
        bail!("ibcmd executable not found: {}", path.display());
    }

    if let Ok(path) = env::var("IBCMD_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return absolute_path(&path);
        }
    }

    let preferred = PathBuf::from(r"C:\Program Files\1cv8\8.3.27.1989\bin\ibcmd.exe");
    if preferred.is_file() {
        return Ok(preferred);
    }

    let mut candidates = Vec::new();
    for root in common_1c_roots() {
        if !root.is_dir() {
            continue;
        }
        for entry in
            fs::read_dir(&root).with_context(|| format!("failed to read {}", root.display()))?
        {
            let entry = entry?;
            let version_dir = entry.path();
            let candidate = version_dir.join(r"bin\ibcmd.exe");
            if candidate.is_file()
                && version_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("8.3."))
            {
                candidates.push(candidate);
            }
        }
    }

    candidates.sort_by(|left, right| version_key(right).cmp(&version_key(left)));
    if let Some(candidate) = candidates.into_iter().next() {
        return Ok(candidate);
    }

    Ok(PathBuf::from("ibcmd"))
}

fn common_1c_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(program_files) = env::var_os("ProgramFiles") {
        roots.push(PathBuf::from(program_files).join("1cv8"));
    }
    if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(program_files_x86).join("1cv8"));
    }
    roots
}

fn version_key(path: &Path) -> Vec<u32> {
    path.parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .split('.')
        .map(|part| part.parse::<u32>().unwrap_or(0))
        .collect()
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return fs::canonicalize(path)
            .with_context(|| format!("failed to resolve {}", path.display()));
    }

    let base = if path.is_absolute() {
        PathBuf::new()
    } else {
        env::current_dir()?
    };
    Ok(base.join(path))
}

fn make_temp_dir(prefix: &str) -> Result<PathBuf> {
    let now_ms = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let path = env::temp_dir().join(format!("{prefix}-{}-{now_ms}", std::process::id()));
    fs::create_dir_all(&path).with_context(|| format!("failed to create {}", path.display()))?;
    Ok(path)
}

pub(crate) struct ProcessOutput {
    pub(crate) success: bool,
    pub(crate) timed_out: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<ProcessOutput> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().context("failed to start ibcmd")?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("failed to capture stderr"))?;

    let stdout_thread = thread::spawn(move || read_pipe(stdout));
    let stderr_thread = thread::spawn(move || read_pipe(stderr));

    let started = Instant::now();
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= timeout {
            timed_out = true;
            let _ = child.kill();
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(100));
    };

    let stdout = join_reader(stdout_thread)?;
    let stderr = join_reader(stderr_thread)?;

    Ok(ProcessOutput {
        success: status.success() && !timed_out,
        timed_out,
        exit_code: status.code(),
        stdout,
        stderr,
    })
}

fn read_pipe<R: Read>(mut reader: R) -> Result<String> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn join_reader(handle: thread::JoinHandle<Result<String>>) -> Result<String> {
    handle
        .join()
        .map_err(|_| anyhow!("reader thread panicked"))?
}

fn mirror_export_to_output(source: &Path, target: &Path, overwrite: bool) -> Result<()> {
    if target.exists() {
        if !target.is_dir() {
            bail!(
                "output path exists and is not a directory: {}",
                target.display()
            );
        }
        if fs::read_dir(target)?.next().is_some() && !overwrite {
            bail!(
                "output directory is not empty: {}. Pass --overwrite to replace it",
                target.display()
            );
        }
        if overwrite {
            clear_directory(target)?;
        }
    } else {
        fs::create_dir_all(target)
            .with_context(|| format!("failed to create {}", target.display()))?;
    }

    for entry in WalkDir::new(source).min_depth(1) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(source)?;
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination)?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &destination).with_context(|| {
                format!(
                    "failed to copy {} to {}",
                    entry.path().display(),
                    destination.display()
                )
            })?;
        }
    }

    Ok(())
}

fn clear_directory(path: &Path) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            fs::remove_dir_all(entry.path())?;
        } else {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn normalize_configuration_xml(root: &Path) -> Result<()> {
    let path = root.join("Configuration.xml");
    if !path.is_file() {
        return Ok(());
    }
    let text = fs::read_to_string(&path)?;
    let updated = text.replace("TaxiEnableVersion8_2", "TaxiEnableOld");
    if updated != text {
        fs::write(path, updated)?;
    }
    Ok(())
}

fn count_files(root: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in WalkDir::new(root) {
        let entry = entry?;
        if entry.file_type().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(windows)]
fn _command_args_for_tests(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_autumn_properties_settings() {
        let path = env::temp_dir().join(format!(
            "ibcmd-rs-settings-test-{}.json",
            std::process::id()
        ));
        fs::write(
            &path,
            r#"{"vrunner":{"dbms-type":"MSSQLServer","dbms-server":"localhost","dbms-base":"OstrovokEmpty","dbms-user":"test-sql-user","dbms-pwd":"dummy-value-for-settings-test"}}"#,
        )
        .unwrap();

        let settings = Some(read_settings(&path).unwrap());
        assert_eq!(
            settings_value(&settings, "dbms-base"),
            Some("OstrovokEmpty".to_string())
        );
        assert_eq!(
            settings_value(&settings, "dbms-pwd"),
            Some("dummy-value-for-settings-test".to_string())
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn version_key_reads_1c_version_directory() {
        let path = PathBuf::from(r"C:\Program Files\1cv8\8.3.27.1989\bin\ibcmd.exe");
        assert_eq!(version_key(&path), vec![8, 3, 27, 1989]);
    }

    #[test]
    fn runtime_password_markers_disclose_only_the_source_class() {
        assert_eq!(
            password_source_marker("integrated"),
            "<password-source:none>"
        );
        assert_eq!(
            password_source_marker("--db-pwd"),
            "<password-source:inline>"
        );
        assert_eq!(
            password_source_marker("settings"),
            "<password-source:settings>"
        );
        assert_eq!(
            password_source_marker("IBCMD_DB_PSW"),
            "<password-source:environment>"
        );
    }

    #[test]
    fn runtime_password_values_are_redacted_longest_first() {
        let mut arguments = vec![
            "--db-user=alpha-long".to_owned(),
            "--db-pwd=alpha-long".to_owned(),
            "--password=alpha".to_owned(),
        ];
        redact_password_values(
            &mut arguments,
            &[
                ("alpha", "<password-source:environment>"),
                ("alpha-long", "<password-source:settings>"),
            ],
        );
        let joined = arguments.join(" ");
        assert!(!joined.contains("alpha"));
        assert!(joined.contains("<password-source:settings>"));
        assert!(joined.contains("<password-source:environment>"));
    }

    #[test]
    fn runtime_journal_accepts_only_exact_evidence_and_redacts_hostile_marker_shapes() {
        let raw_password = "top-secret-password";
        let exact_password_markers = [
            "<password-source:none>".to_owned(),
            "<password-source:inline>".to_owned(),
            "<password-source:settings>".to_owned(),
            "<password-source:environment>".to_owned(),
        ];
        let replacement_marker = exact_password_markers[3].clone();
        let exact_query_marker =
            SubprocessJournalSchema::query_digest_marker("SELECT top-secret-query");
        let mut arguments = exact_password_markers.to_vec();
        arguments.extend([
            exact_query_marker.clone(),
            format!("<password-source:environment>{raw_password}"),
            format!("prefix<password-source:settings>{raw_password}"),
            format!("<password-source:<query-sha256:{raw_password}"),
            format!("<query-sha256:{raw_password}>"),
            format!("<query-sha256:{}>{raw_password}", "a".repeat(63)),
            format!("<query-sha256:{}>{raw_password}", "A".repeat(64)),
            format!(
                "prefix<query-sha256:{}>suffix-{raw_password}",
                "a".repeat(64)
            ),
        ]);

        redact_password_values(
            &mut arguments,
            &[(raw_password, replacement_marker.as_str())],
        );

        assert_eq!(
            &arguments[..exact_password_markers.len()],
            &exact_password_markers
        );
        assert_eq!(arguments[exact_password_markers.len()], exact_query_marker);
        assert!(
            arguments
                .iter()
                .skip(exact_password_markers.len() + 1)
                .all(|argument| {
                    SubprocessJournalSchema::argument_kind(argument)
                        == SanitizedRuntimeArgumentKind::PasswordSourceMarker
                })
        );

        let runtime_call = SanitizedRuntimeCall {
            executable: PathBuf::from("ibcmd"),
            arguments,
            started_unix_ms: 1,
            ended_unix_ms: Some(2),
            status: "failed".to_owned(),
            exit_code: Some(1),
            timed_out: false,
            exception: Some("nested ibcmd exited without exposing arguments".to_owned()),
        };
        let serialized = serde_json::to_string(&runtime_call).unwrap();
        let journal_path = env::temp_dir().join(format!(
            "ibcmd-rs-native-runtime-hostile-marker-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        persist_runtime_journal(Some(&journal_path), &runtime_call).unwrap();
        let persisted = fs::read_to_string(&journal_path).unwrap();
        assert!(!serialized.contains(raw_password));
        assert!(!persisted.contains(raw_password));
        assert!(!serialized.contains("SELECT top-secret-query"));
        assert!(!persisted.contains("SELECT top-secret-query"));
        for marker in exact_password_markers {
            assert!(serialized.contains(&marker));
            assert!(persisted.contains(&marker));
        }
        assert!(serialized.contains(&exact_query_marker));
        assert!(persisted.contains(&exact_query_marker));
        assert!(!serialized.contains("prefix<password-source:"));
        assert!(!serialized.contains("prefix<query-sha256:"));

        let mut child_output = format!("command echoed password={raw_password}");
        redact_runtime_output(&mut child_output, &[raw_password]);
        assert!(!child_output.contains(raw_password));
        let _ = fs::remove_file(journal_path);
    }

    #[test]
    fn runtime_journal_survives_actual_spawn_failure() {
        let journal_path = env::temp_dir().join(format!(
            "ibcmd-rs-native-runtime-failure-{}.json",
            uuid::Uuid::new_v4().simple()
        ));
        let mut call = SanitizedRuntimeCall {
            executable: PathBuf::from("ibcmd-rs-command-that-does-not-exist"),
            arguments: vec![
                "infobase".to_owned(),
                "config".to_owned(),
                "export".to_owned(),
            ],
            started_unix_ms: unix_time_ms(),
            ended_unix_ms: None,
            status: "running".to_owned(),
            exit_code: None,
            timed_out: false,
            exception: None,
        };
        persist_runtime_journal(Some(&journal_path), &call).unwrap();
        let command = Command::new(&call.executable);

        assert!(
            run_with_runtime_journal(
                command,
                Duration::from_secs(1),
                Some(&journal_path),
                &mut call,
            )
            .is_err()
        );
        let journal: Value =
            serde_json::from_str(&fs::read_to_string(&journal_path).unwrap()).unwrap();
        assert_eq!(journal["runtime_call"]["status"], "failed");
        assert!(journal["runtime_call"]["ended_unix_ms"].as_u64().is_some());
        assert!(journal["runtime_call"]["exception"].is_string());
        let _ = fs::remove_file(journal_path);
    }
}
