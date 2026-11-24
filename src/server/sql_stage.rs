//! SQL stage caches are process-wide and retain the first target's rows.
//! Compile each confirmed import in a fresh process with its own target.
use super::{ImportParams, Session};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Deserialize, Serialize)]
struct Request {
    server: String,
    database: String,
    user: Option<String>,
    password: Option<String>,
    platform: String,
    source_root: PathBuf,
    prefixes: Vec<String>,
    verify: bool,
    confirm: bool,
}

pub(super) fn stage(
    session: &Session,
    params: ImportParams,
    prefixes: Vec<String>,
) -> Result<Value> {
    let request = Request {
        server: session.server.clone(),
        database: session.database.clone(),
        user: session.login.user.clone(),
        password: session
            .login
            .password
            .as_ref()
            .map(|secret| secret.expose().to_owned()),
        platform: session.entry.platform.clone(),
        source_root: params.source_dir,
        prefixes,
        verify: params.verify.unwrap_or(true),
        confirm: params.confirm,
    };
    let bytes = serde_json::to_vec(&request)?;
    crate::cancel::check()?;
    let mut child = Command::new(std::env::current_exe()?)
        .arg("editor-sql-stage-worker")
        .env_remove(crate::settings::ENV_DB_PASSWORD)
        .env_remove("IBCMD_RS_BASE_ROWS_DIR")
        .env_remove("IBCMD_RS_STAGE_VERIFY")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("start SQL import worker")?;
    let sent = child.stdin.take().expect("piped stdin").write_all(&bytes);
    // Reap the worker even if the pipe closed early. Losing a reply must
    // never cause a retry or turn a possible commit into a cancellation.
    let output = child
        .wait_with_output()
        .context("SQL import outcome unknown; inspect ConfigSave")?;
    sent.context("could not send import request; inspect ConfigSave")?;
    ensure!(
        output.status.success(),
        "SQL import worker exited {}; inspect ConfigSave before retrying",
        output.status
    );
    let reply: Result<Value, String> = serde_json::from_slice(&output.stdout)
        .context("SQL import outcome unknown; inspect ConfigSave")?;
    reply.map_err(anyhow::Error::msg)
}

/// Private child entry point. Only the pipe carries the editor's password.
pub fn run_worker() -> Result<()> {
    let request: Request = serde_json::from_reader(std::io::stdin().lock())?;
    let reply = compile(request).map_err(|error| format!("{error:#}"));
    serde_json::to_writer(std::io::stdout().lock(), &reply)?;
    Ok(())
}

fn compile(request: Request) -> Result<crate::mssql::StageSourceObjectsReport> {
    ensure!(
        request.confirm && !request.prefixes.is_empty(),
        "selected object import requires confirmation"
    );
    let platform = crate::platform::parse_flag(&request.platform).map_err(anyhow::Error::msg)?;
    let sql = super::infobases::editor_sql(
        &request.server,
        request.user.as_deref(),
        request.password.as_deref(),
    )?;
    crate::settings::commands::check_tree(platform, &request.source_root, &request.database)?;
    let args = crate::cli::MssqlStageSourceObjectsArgs {
        server: request.server,
        database: request.database,
        sql_user: None,
        sql_pwd: None,
        sql_pwd_env: crate::settings::ENV_DB_PASSWORD.to_owned(),
        source_root: request.source_root,
        sqlcmd: None,
        replace_config_save: true,
        allow_non_lab: true,
        batch_size: None,
        platform: Some(platform),
        source_version: Some(platform.xml_version()),
        path_prefix: request.prefixes,
        files: Vec::new(),
        native_files: None,
        script_output: Some(
            std::env::temp_dir()
                .join("ibcmd-rs-serve")
                .join(format!("{}_import.sql", uuid::Uuid::new_v4())),
        ),
        script_only: false,
        bulk: false,
        per_row: false,
        bcp_executable: None,
        base_free: false,
        verify: request.verify,
    };
    crate::mssql::stage_source_objects_with_sql(&args, Some(sql))
}
