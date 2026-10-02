//! `ibcmd-rs serve --stdio`: the server a code editor (VS Code, Cursor,
//! VSCodium) runs, one per window, to work with the configuration of an
//! infobase -- its tree, the export and load of selected objects, their
//! comparison with a source folder -- each step in seconds, without reading
//! the whole database again.
//!
//! JSON-RPC 2.0 over stdin/stdout in `Content-Length` frames, as the Language
//! Server Protocol frames it ([`rpc`]); no network, no port. The protocol --
//! methods, notifications, errors, the version rule of `initialize` -- is
//! `openspec/changes/add-editor-server/design.md`.
//!
//! The server has no logic of its own: every method calls the library
//! functions the command line calls (`stored_objects`, the export of
//! `mssql-dump-config`, the stage of `mssql-stage-source-objects`, the check
//! of `mssql-apply-check`), so whatever the editor does a command can do.
//!
//! Requests are answered in the order they come, by one worker thread (the
//! export's state is process-wide, see `stored_objects`); the reader
//! thread meanwhile takes `$/cancelRequest`, which stops a running request at
//! its next step (`crate::cancel`) and a queued one before it starts. stdout
//! carries the protocol only; the engine's own diagnostics go to stderr.

pub mod infobases;
pub mod rpc;

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::settings::{ENV_CONFIG, ENV_DB_PASSWORD, Settings, SettingsSources};
use crate::stored_objects::{self as objects, ConfigSource, UnknownObject};
use infobases::{Entry, EntryParams, Login, Secret, Session};
use rpc::{RpcError, codes};

/// The protocol version this server speaks, `MAJOR.MINOR`. A client is
/// served when its major is this one and its minor is not above this one
/// (see `initialize`); the editor extension and the executable are released
/// together, so the check catches a mismatched pair.
pub const PROTOCOL_VERSION: (u32, u32) = (1, 0);

/// The methods a client may call.
pub const METHODS: &[&str] = &[
    "initialize",
    "shutdown",
    "infobases/list",
    "infobases/connect",
    "tree/children",
    "objects/status",
    "objects/export",
    "objects/import",
    "source/read",
    "config/pending",
];

/// The stack of the worker thread: the export walks deeply nested sources
/// (form item trees, brace bodies), as `main` explains for the command
/// thread.
const WORKER_STACK_BYTES: usize = 256 * 1024 * 1024;

/// Where the frames go: the whole frame in one write, so that nothing else
/// written to the same stream lands inside it.
#[derive(Clone)]
pub struct Output(Arc<Mutex<Box<dyn Write + Send>>>);

impl Output {
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self(Arc::new(Mutex::new(writer)))
    }

    fn send(&self, value: &Value) {
        let mut frame = Vec::new();
        if rpc::write_frame(&mut frame, value).is_err() {
            return;
        }
        let mut writer = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Err(error) = writer.write_all(&frame).and_then(|()| writer.flush()) {
            eprintln!("ibcmd-rs serve: failed to write to the client: {error}");
        }
    }

    fn notify(&self, method: &str, params: Value) {
        self.send(&rpc::notification(method, params));
    }

    /// A `log` notification.
    fn log(&self, level: &str, message: impl Into<String>) {
        self.notify("log", json!({"level": level, "message": message.into()}));
    }
}

/// A request id as a map key (`1` and `"1"` are different ids).
fn id_key(id: &Value) -> String {
    id.to_string()
}

struct Job {
    id: Value,
    method: String,
    params: Value,
    cancel: Arc<AtomicBool>,
}

type InFlight = Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>;

/// Serves stdin/stdout until `exit` or the end of stdin; the process exit
/// code (0 after `shutdown`, 1 otherwise, as LSP's `exit` defines it).
pub fn run_stdio() -> i32 {
    let stdin = std::io::stdin();
    serve(&mut stdin.lock(), Output::new(Box::new(std::io::stdout())))
}

/// Serves one client: frames from `input`, answers to `output`.
pub fn serve(input: &mut dyn BufRead, output: Output) -> i32 {
    let in_flight: InFlight = Arc::default();
    let shutdown = Arc::new(AtomicBool::new(false));
    let (jobs, queue) = mpsc::channel::<Job>();
    let worker = {
        let output = output.clone();
        let in_flight = in_flight.clone();
        let shutdown = shutdown.clone();
        std::thread::Builder::new()
            .name("ibcmd-rs-serve".to_string())
            .stack_size(WORKER_STACK_BYTES)
            .spawn(move || work(queue, output, in_flight, shutdown))
    };
    let worker = match worker {
        Ok(worker) => worker,
        Err(error) => {
            eprintln!("ibcmd-rs serve: failed to start the worker thread: {error}");
            return 1;
        }
    };

    let mut broken = false;
    loop {
        let body = match rpc::read_frame(input) {
            Ok(Some(body)) => body,
            Ok(None) => break,
            Err(error) => {
                eprintln!("ibcmd-rs serve: unreadable input, stopping: {error}");
                broken = true;
                break;
            }
        };
        let message = match serde_json::from_slice::<Value>(&body) {
            Ok(message) => message,
            Err(error) => {
                output.send(&rpc::response(
                    &Value::Null,
                    Err(RpcError::new(
                        codes::PARSE_ERROR,
                        format!("the message is not JSON: {error}"),
                    )),
                ));
                continue;
            }
        };
        let valid_id = message
            .get("id")
            .is_none_or(|id| matches!(id, Value::Null | Value::String(_) | Value::Number(_)));
        if !message.is_object()
            || message.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || !valid_id
        {
            let id = if valid_id {
                message.get("id").unwrap_or(&Value::Null)
            } else {
                &Value::Null
            };
            output.send(&rpc::response(
                id,
                Err(RpcError::new(
                    codes::INVALID_REQUEST,
                    "a request needs jsonrpc: 2.0 and a string, number or null id",
                )),
            ));
            continue;
        }
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            // A response to a request of ours (we send none) or garbage.
            if message.get("result").is_none() && message.get("error").is_none() {
                output.send(&rpc::response(
                    message.get("id").unwrap_or(&Value::Null),
                    Err(RpcError::new(
                        codes::INVALID_REQUEST,
                        "a request needs a method (batches are not supported)",
                    )),
                ));
            }
            continue;
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match (method, message.get("id")) {
            ("$/cancelRequest", _) => {
                if let Some(id) = params.get("id") {
                    let in_flight = in_flight.lock().unwrap_or_else(PoisonError::into_inner);
                    if let Some(flag) = in_flight.get(&id_key(id)) {
                        flag.store(true, Ordering::SeqCst);
                    }
                }
            }
            ("exit", _) => break,
            // Other notifications (`initialized`, `$/setTrace`) need nothing.
            (_, None) => {}
            (method, Some(id)) => {
                let cancel = Arc::new(AtomicBool::new(false));
                in_flight
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(id_key(id), cancel.clone());
                let job = Job {
                    id: id.clone(),
                    method: method.to_string(),
                    params,
                    cancel,
                };
                if jobs.send(job).is_err() {
                    broken = true;
                    break;
                }
            }
        }
    }
    // Whatever still runs or waits is cancelled; the worker ends once the
    // queue is drained.
    for flag in in_flight
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .values()
    {
        flag.store(true, Ordering::SeqCst);
    }
    drop(jobs);
    let _ = worker.join();
    if !broken && shutdown.load(Ordering::SeqCst) {
        0
    } else {
        1
    }
}

fn work(
    queue: mpsc::Receiver<Job>,
    output: Output,
    in_flight: InFlight,
    shutdown: Arc<AtomicBool>,
) {
    let mut state = State {
        output: output.clone(),
        initialized: false,
        shutdown,
        settings: Settings::default(),
        entries: Vec::new(),
        sessions: HashMap::new(),
    };
    for job in queue {
        let outcome = if job.cancel.load(Ordering::SeqCst) {
            Err(cancelled())
        } else {
            let _scope = crate::cancel::scope(job.cancel.clone());
            std::panic::catch_unwind(AssertUnwindSafe(|| {
                state.handle(&job.id, &job.method, job.params)
            }))
            .unwrap_or_else(|panic| {
                let message = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string()))
                    .unwrap_or_else(|| "unknown panic".to_string());
                Err(RpcError::new(
                    codes::INTERNAL_ERROR,
                    format!("{} failed inside the server: {message}", job.method),
                ))
            })
        };
        in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id_key(&job.id));
        output.send(&rpc::response(&job.id, outcome));
    }
}

fn cancelled() -> RpcError {
    RpcError::new(codes::REQUEST_CANCELLED, "the request was cancelled")
}

/// An operation's failure as an answer: a cancellation, an unknown object,
/// or the failure with its causes.
fn failure(error: anyhow::Error) -> RpcError {
    if crate::cancel::is_cancelled(&error) {
        return cancelled();
    }
    let code = if error.chain().any(|cause| cause.is::<UnknownObject>()) {
        codes::UNKNOWN_OBJECT
    } else {
        codes::OPERATION_FAILED
    };
    RpcError::new(code, format!("{error:#}")).with_data(json!({
        "causes": error.chain().map(|cause| cause.to_string()).collect::<Vec<_>>(),
    }))
}

fn parse<T: for<'de> Deserialize<'de>>(params: Value) -> Result<T, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    serde_json::from_value(params).map_err(|error| RpcError::invalid_params(error.to_string()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InitializeParams {
    protocol_version: Option<String>,
    #[serde(default)]
    client_info: Option<Value>,
    /// The editor's workspace folder: its `ibcmd-rs.toml` is the settings
    /// layer of the current directory.
    workspace_folder: Option<PathBuf>,
    /// One settings file read instead of the layers (as `IBCMD_RS_CONFIG`).
    settings_file: Option<PathBuf>,
    /// Infobases the editor adds to the settings' (they replace an entry of
    /// the same id).
    #[serde(default)]
    infobases: Vec<EntryParams>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectParams {
    infobase: String,
    database: Option<String>,
    user: Option<String>,
    password: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfobaseParams {
    infobase: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChildrenParams {
    infobase: String,
    node: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusParams {
    infobase: String,
    source_dir: PathBuf,
    objects: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportParams {
    infobase: String,
    objects: Vec<String>,
    target_dir: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportParams {
    infobase: String,
    source_dir: PathBuf,
    objects: Vec<String>,
    #[serde(default)]
    confirm: bool,
    verify: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadParams {
    infobase: String,
    path: String,
}

struct State {
    output: Output,
    initialized: bool,
    shutdown: Arc<AtomicBool>,
    settings: Settings,
    entries: Vec<Entry>,
    /// Open infobases by id.
    sessions: HashMap<String, Session>,
}

impl State {
    fn handle(&mut self, id: &Value, method: &str, params: Value) -> Result<Value, RpcError> {
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(RpcError::new(
                codes::INVALID_REQUEST,
                "the server is shut down: only exit is accepted",
            ));
        }
        if !self.initialized && method != "initialize" {
            return Err(RpcError::new(
                codes::SERVER_NOT_INITIALIZED,
                "initialize first",
            ));
        }
        match method {
            "initialize" => self.initialize(parse(params)?),
            "shutdown" => {
                self.shutdown.store(true, Ordering::SeqCst);
                self.sessions.clear();
                Ok(Value::Null)
            }
            "infobases/list" => self.list(),
            "infobases/connect" => self.connect(parse(params)?),
            "tree/children" => self.children(parse(params)?),
            "objects/status" => self.status(id, parse(params)?),
            "objects/export" => self.export(id, parse(params)?),
            "objects/import" => self.import(parse(params)?),
            "source/read" => self.read(parse(params)?),
            "config/pending" => self.pending(parse(params)?),
            other => Err(RpcError::new(
                codes::METHOD_NOT_FOUND,
                format!("no method {other}"),
            )),
        }
    }

    fn initialize(&mut self, params: InitializeParams) -> Result<Value, RpcError> {
        if self.initialized {
            return Err(RpcError::new(
                codes::INVALID_REQUEST,
                "initialize was already called",
            ));
        }
        let asked = params
            .protocol_version
            .as_deref()
            .ok_or_else(|| RpcError::invalid_params("protocolVersion is required"))?;
        let server_version = format!("{}.{}", PROTOCOL_VERSION.0, PROTOCOL_VERSION.1);
        let parsed = asked.split_once('.').and_then(|(major, minor)| {
            Some((major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?))
        });
        let compatible = parsed.is_some_and(|(major, minor)| {
            major == PROTOCOL_VERSION.0 && minor <= PROTOCOL_VERSION.1
        });
        if !compatible {
            return Err(RpcError::new(
                codes::INCOMPATIBLE_PROTOCOL,
                format!(
                    "the client speaks protocol {asked}, this ibcmd-rs {} speaks {server_version}: install the extension and the executable of one release",
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .with_data(json!({
                "serverProtocolVersion": server_version,
                "serverVersion": env!("CARGO_PKG_VERSION"),
            })));
        }
        let settings_file = params.settings_file.clone();
        let env = move |name: &str| {
            if name == ENV_CONFIG
                && let Some(file) = &settings_file
            {
                return Some(file.display().to_string());
            }
            std::env::var(name).ok()
        };
        let settings = Settings::from_sources(&SettingsSources {
            env: &env,
            executable: std::env::current_exe().ok(),
            current_dir: params
                .workspace_folder
                .clone()
                .or_else(|| std::env::current_dir().ok()),
            app_data: std::env::var_os("APPDATA").map(PathBuf::from),
            native_config: None,
        })
        .map_err(|error| failure(error.context("failed to read the settings")))?;
        let entries = infobases::entries(&settings, &params.infobases)
            .map_err(|error| RpcError::invalid_params(format!("{error:#}")))?;
        self.settings = settings;
        self.entries = entries;
        self.initialized = true;
        if let Some(client) = params.client_info {
            self.output.log("debug", format!("client: {client}"));
        }
        Ok(json!({
            "protocolVersion": server_version,
            "serverInfo": {"name": "ibcmd-rs", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {
                "methods": METHODS,
                "notifications": ["progress", "log"],
                "cancelRequest": true,
            },
            "settingsFiles": self
                .settings
                .files()
                .iter()
                .map(|file| file.path.display().to_string())
                .collect::<Vec<_>>(),
        }))
    }

    fn list(&self) -> Result<Value, RpcError> {
        let infobases = self
            .entries
            .iter()
            .map(|entry| {
                let mut value = serde_json::to_value(entry).unwrap_or(Value::Null);
                value["connected"] = Value::Bool(self.sessions.contains_key(&entry.id));
                value
            })
            .collect::<Vec<_>>();
        Ok(json!({ "infobases": infobases }))
    }

    fn entry(&self, id: &str) -> Result<&Entry, RpcError> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| RpcError::new(codes::UNKNOWN_INFOBASE, format!("no infobase {id}")))
    }

    /// The open infobase `id`, opened with the default login when it is not
    /// open yet (an opened mask's id is its database's).
    fn session(&mut self, id: &str) -> Result<&mut Session, RpcError> {
        if !self.sessions.contains_key(id) {
            let entry = self.entry(id)?.clone();
            let session = Session::open(&entry, None, Login::default(), &self.settings)
                .map_err(|error| RpcError::invalid_params(format!("{error:#}")))?;
            self.output.log("info", opened_message(&session));
            self.sessions.insert(id.to_string(), session);
        }
        Ok(self.sessions.get_mut(id).expect("inserted above"))
    }

    fn connect(&mut self, params: ConnectParams) -> Result<Value, RpcError> {
        let entry = self.entry(&params.infobase)?.clone();
        let login = Login {
            user: params.user.filter(|user| !user.trim().is_empty()),
            password: params.password.map(Secret::new),
        };
        let mut session = Session::open(&entry, params.database.as_deref(), login, &self.settings)
            .map_err(|error| RpcError::invalid_params(format!("{error:#}")))?;
        // The first level of the tree: proves the login and the database.
        let root = session.configuration.children(None).map_err(failure)?;
        let id = session.entry.id.clone();
        self.output.log("info", opened_message(&session));
        let result = json!({
            "infobase": id,
            "source": session.entry.source,
            "platform": session.entry.platform,
            "xmlVersion": session.entry.xml_version,
            "configuration": root.first(),
        });
        if session.entry.id != entry.id && !self.entries.iter().any(|known| known.id == id) {
            // A database of a mask: listed from now on under its own id.
            self.entries.push(session.entry.clone());
        }
        self.sessions.insert(id, session);
        Ok(result)
    }

    fn children(&mut self, params: ChildrenParams) -> Result<Value, RpcError> {
        let session = self.session(&params.infobase)?;
        let nodes = session
            .configuration
            .children(params.node.as_deref())
            .map_err(failure)?;
        Ok(json!({ "nodes": nodes }))
    }

    fn progress(&self, id: &Value) -> impl Fn(usize, usize, &str) + use<> {
        let output = self.output.clone();
        let id = id.clone();
        move |done: usize, total: usize, what: &str| {
            output.notify(
                "progress",
                json!({"requestId": id, "message": what, "done": done, "total": total}),
            );
        }
    }

    fn status(&mut self, id: &Value, params: StatusParams) -> Result<Value, RpcError> {
        let progress = self.progress(id);
        let session = self.session(&params.infobase)?;
        let statuses = session
            .configuration
            .status(params.objects.as_deref(), &params.source_dir, &progress)
            .map_err(failure)?;
        Ok(json!({ "objects": statuses }))
    }

    fn export(&mut self, id: &Value, params: ExportParams) -> Result<Value, RpcError> {
        if params.objects.is_empty() {
            return Err(RpcError::invalid_params("objects is empty"));
        }
        let progress = self.progress(id);
        let session = self.session(&params.infobase)?;
        let writes = session
            .configuration
            .export_into(&params.objects, &params.target_dir, &progress)
            .map_err(failure)?;
        let written = writes
            .iter()
            .map(|write| write.written.len())
            .sum::<usize>();
        self.output.log(
            "info",
            format!(
                "exported {} objects into {}: {written} files written",
                writes.len(),
                params.target_dir.display()
            ),
        );
        Ok(json!({ "objects": writes }))
    }

    fn read(&mut self, params: ReadParams) -> Result<Value, RpcError> {
        let session = self.session(&params.infobase)?;
        let (exported, path) = session
            .configuration
            .read_file(&params.path)
            .map_err(|error| {
                if error.chain().any(|cause| cause.is::<UnknownObject>()) {
                    failure(error)
                } else if objects::object_of_path(&params.path).is_none() {
                    RpcError::invalid_params(format!("{error:#}"))
                } else {
                    failure(error)
                }
            })?;
        let mut result = json!({
            "path": path,
            "object": exported.object.full_name,
            "version": exported.version,
            "exists": false,
        });
        if let Some(bytes) = exported.files.get(&path) {
            result["exists"] = Value::Bool(true);
            match std::str::from_utf8(bytes) {
                Ok(text) => {
                    result["encoding"] = json!("utf-8");
                    result["content"] = json!(text);
                }
                Err(_) => {
                    result["encoding"] = json!("base64");
                    result["content"] = json!(rpc::base64(bytes));
                }
            }
        }
        Ok(result)
    }

    fn import(&mut self, params: ImportParams) -> Result<Value, RpcError> {
        if params.objects.is_empty() {
            return Err(RpcError::invalid_params("objects is empty"));
        }
        let mut prefixes = Vec::new();
        for name in &params.objects {
            prefixes.extend(objects::source_prefixes(name).ok_or_else(|| {
                RpcError::new(
                    codes::UNKNOWN_OBJECT,
                    format!("{name} is not an object name"),
                )
            })?);
        }
        let session = self.session(&params.infobase)?;
        if let ConfigSource::RowsDir(_) = session.configuration.source() {
            return Err(offline("objects/import", session));
        }
        if !params.confirm {
            return Err(RpcError::new(
                codes::CONFIRMATION_REQUIRED,
                "objects/import replaces the infobase's ConfigSave: pass confirm: true",
            ));
        }
        let platform = session.configuration.platform();
        let args = crate::cli::MssqlStageSourceObjectsArgs {
            server: session.server.clone(),
            sql_user: session.login.user.clone(),
            sql_pwd: session
                .login
                .password
                .as_ref()
                .map(|password| password.expose().to_string()),
            sql_pwd_env: ENV_DB_PASSWORD.to_string(),
            database: session.database.clone(),
            source_root: params.source_dir.clone(),
            sqlcmd: None,
            replace_config_save: true,
            allow_non_lab: true,
            batch_size: None,
            platform: Some(platform),
            source_version: Some(platform.xml_version()),
            path_prefix: prefixes,
            files: Vec::new(),
            script_output: Some(
                std::env::temp_dir()
                    .join("ibcmd-rs-serve")
                    .join(format!("{}_import.sql", session.database)),
            ),
            script_only: false,
            bulk: false,
            per_row: false,
            bcp_executable: None,
            base_free: false,
            verify: params.verify.unwrap_or(true),
        };
        let report = objects::exclusive(|| {
            crate::settings::commands::check_tree(platform, &args.source_root, &args.database)?;
            crate::mssql::stage_source_objects(&args)
        })
        .map_err(failure)?;
        Ok(json!({ "report": report }))
    }

    fn pending(&mut self, params: InfobaseParams) -> Result<Value, RpcError> {
        let session = self.session(&params.infobase)?;
        let ConfigSource::Database { sql, database } = session.configuration.source() else {
            return Err(offline("config/pending", session));
        };
        let xml = session.configuration.platform().xml_version();
        let verdict = objects::exclusive(|| {
            crate::apply_check::check_staged(sql, database, Some(xml.as_str()))
        })
        .map_err(failure)?;
        Ok(json!({
            "pending": verdict.stats.staged_rows > 0,
            "stagedRows": verdict.stats.staged_rows,
            "needsRestructuring": verdict.needs_restructuring,
            "conclusive": verdict.is_conclusive(),
            "summary": verdict.render_text(),
            "verdict": verdict,
        }))
    }
}

fn offline(method: &str, session: &Session) -> RpcError {
    let reason = match session.configuration.source() {
        ConfigSource::RowsDir(dir) => {
            objects::require_database(&ConfigSource::RowsDir(dir.clone()), method)
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default()
        }
        ConfigSource::Database { .. } => String::new(),
    };
    RpcError::new(codes::NOT_SUPPORTED_OFFLINE, reason)
}

fn opened_message(session: &Session) -> String {
    match session.configuration.source() {
        ConfigSource::RowsDir(dir) => format!(
            "infobase {}: stored rows of {} (platform {})",
            session.entry.id,
            dir.display(),
            session.entry.platform
        ),
        ConfigSource::Database { database, .. } => format!(
            "infobase {}: database {database} on {} as {} (platform {})",
            session.entry.id,
            session.server,
            session.login.user.as_deref().unwrap_or("the Windows login"),
            session.entry.platform
        ),
    }
}
