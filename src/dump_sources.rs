#![cfg(feature = "platform-oracle")]
//! Research-only source export through an installed ibcmd executable.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Serialize;
use serde_json::Value;
use walkdir::WalkDir;

use crate::cli::DumpSourcesArgs;

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

    let mut command = Command::new(&config.ibcmd);
    command
        .arg("infobase")
        .arg("config")
        .arg("export")
        .arg(format!("--dbms={}", config.dbms))
        .arg(format!("--db-server={}", config.db_server))
        .arg(format!("--db-name={}", config.db_name))
        .arg(format!("--data={}", config.data_dir.display()));
    if let Some(db_user) = &config.db_user {
        command
            .arg(format!("--db-user={db_user}"))
            .arg(format!("--db-pwd={}", config.db_pwd));
    }

    if let Some(user) = &config.infobase_user {
        command.arg(format!("--user={user}"));
    }
    if let Some(password) = &config.infobase_password {
        command.arg(format!("--password={password}"));
    }

    if let Some(extension) = &config.extension {
        command.arg(format!("--extension={extension}"));
    }

    command.arg("--force").arg(&temp_export_dir);

    let started = Instant::now();
    let output = run_with_timeout(command, config.timeout)?;
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
        ibcmd: config.ibcmd,
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
    })
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
}
