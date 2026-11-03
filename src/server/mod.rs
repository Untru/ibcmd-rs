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

mod apply;
mod import_plans;
pub mod infobases;
pub mod rpc;
pub mod sql_import;

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, PoisonError};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::settings::{ENV_CONFIG, Settings, SettingsSources};
use crate::stored_objects::{self as objects, ConfigSource, UnknownObject};
use infobases::{Entry, EntryParams, Login, Secret, Session};
use rpc::{RpcError, codes};

/// The protocol version this server speaks, `MAJOR.MINOR`. A client is
/// served when its major is this one and its minor is not above this one
/// (see `initialize`); the editor extension and the executable are released
/// together, so the check catches a mismatched pair.
pub const PROTOCOL_VERSION: (u32, u32) = (1, 4);

/// The methods a client may call.
pub const METHODS: &[&str] = &[
    "initialize",
    "shutdown",
    "infobases/list",
    "infobases/connect",
    "tree/children",
    "objects/status",
    "objects/export",
    "objects/exportPlan",
    "objects/import",
    "objects/import/prepare",
    "source/read",
    "config/pending",
    "config/apply",
    "config/apply/prepare",
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
        import_plans: import_plans::Registry::new(),
        connection_epoch: 0,
    };
    for job in queue {
        let outcome = if job.cancel.load(Ordering::SeqCst) {
            discard_cancelled_import_plan(
                &mut state.import_plans,
                &job.method,
                &job.params,
                state.connection_epoch,
                std::time::Instant::now(),
            );
            Err(cancelled())
        } else {
            let _scope = crate::cancel::scope(job.cancel.clone());
            std::panic::catch_unwind(AssertUnwindSafe(|| {
                state.handle(&job.id, &job.method, job.params)
            }))
            .unwrap_or_else(|panic| Err(panic_failure(&job.method, panic)))
        };
        in_flight
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id_key(&job.id));
        output.send(&rpc::response(&job.id, outcome));
    }
}

fn panic_failure(method: &str, panic: Box<dyn std::any::Any + Send>) -> RpcError {
    // Unwinding does not establish whether publication preceded the panic.
    if method == "objects/import" {
        return RpcError::new(
            codes::IMPORT_OUTCOME_UNKNOWN,
            "Import outcome is unconfirmed; inspect the database before retrying",
        );
    }
    let message = panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|text| text.to_string()))
        .unwrap_or_else(|| "unknown panic".to_string());
    RpcError::new(
        codes::INTERNAL_ERROR,
        format!("{method} failed inside the server: {message}"),
    )
}

/// A cancelled queued publication never reaches the handler. Still revoke its
/// supplied capability, including malformed requests and mismatched bindings.
fn discard_cancelled_import_plan<P>(
    plans: &mut import_plans::Registry<P>,
    method: &str,
    params: &Value,
    epoch: u64,
    now: std::time::Instant,
) {
    if method != "objects/import" {
        return;
    }
    let Some(token) = params
        .get("plan")
        .and_then(Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
    else {
        return;
    };
    let target = params.get("infobase").and_then(Value::as_str).unwrap_or("");
    // take removes before checking target, epoch or expiry. No payload is used.
    let _ = plans.take(&token, target, epoch, now);
}

fn cancelled() -> RpcError {
    RpcError::new(codes::REQUEST_CANCELLED, "the request was cancelled")
}

/// An operation's failure as an answer: a cancellation, an unknown object,
/// or the failure with its causes.
fn failure(error: anyhow::Error) -> RpcError {
    // This state takes precedence over cancellation and never exposes paths.
    if error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<ibcmd_filedb::Error>(),
            Some(ibcmd_filedb::Error::Published { .. })
        )
    }) {
        return RpcError::new(
            codes::IMPORT_PUBLISHED_RECOVERY_REQUIRED,
            "Database changes were published; backup update failed. Inspect recovery before retrying",
        );
    }
    // An acknowledged commit or indeterminate outcome is authoritative even
    // when its inner cause is cancellation. Never turn either into no-write.
    if error.chain().any(|cause| {
        cause
            .downcast_ref::<sql_import::transaction::Report>()
            .is_some_and(|report| report.state == sql_import::transaction::State::Committed)
    }) {
        return RpcError::new(
            codes::IMPORT_COMMITTED_RECOVERY_REQUIRED,
            "Database changes were committed; inspect recovery before retrying",
        );
    }
    if error.chain().any(|cause| {
        cause
            .downcast_ref::<sql_import::transaction::Report>()
            .is_some_and(|report| {
                report.state == sql_import::transaction::State::Unknown || report.cleanup_uncertain
            })
    }) {
        return RpcError::new(
            codes::IMPORT_OUTCOME_UNKNOWN,
            "Import outcome is unconfirmed; inspect the database before retrying",
        );
    }
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
    auth: Option<infobases::AuthMode>,
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
    #[serde(default)]
    main_configuration: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusParams {
    infobase: String,
    source_dir: PathBuf,
    objects: Option<Vec<String>>,
    version_baseline_dir: Option<PathBuf>,
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
struct ExportPlanParams {
    infobase: String,
    #[serde(default)]
    objects: Vec<String>,
    #[serde(default)]
    changed: bool,
    source_dir: PathBuf,
    target_dir: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportParams {
    infobase: String,
    source_dir: PathBuf,
    #[serde(default)]
    objects: Vec<String>,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    changed: bool,
    plan: Option<String>,
    #[serde(default)]
    confirm: bool,
    // Preserve the request shape; prepared publication always verifies and
    // no-plan SQL publication is refused, so this cannot weaken validation.
    #[serde(rename = "verify")]
    _verify: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadParams {
    infobase: String,
    path: String,
}

enum ImportPlan {
    FileDb(Box<crate::file_import::Plan>),
    SqlModule(Box<sql_import::CommonModulePlan>),
}

struct State {
    output: Output,
    initialized: bool,
    shutdown: Arc<AtomicBool>,
    settings: Settings,
    entries: Vec<Entry>,
    /// Open infobases by id.
    sessions: HashMap<String, Session>,
    import_plans: import_plans::Registry<ImportPlan>,
    connection_epoch: u64,
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
                self.import_plans.clear();
                Ok(Value::Null)
            }
            "infobases/list" => self.list(),
            "infobases/connect" => self.connect(parse(params)?),
            "tree/children" => self.children(parse(params)?),
            "objects/status" => self.status(id, parse(params)?),
            "objects/export" => self.export(id, parse(params)?),
            "objects/exportPlan" => self.export_plan(id, parse(params)?),
            "objects/import" => self.import(parse(params)?),
            "objects/import/prepare" => self.prepare_import(parse(params)?),
            "source/read" => self.read(parse(params)?),
            "config/pending" => self.pending(parse(params)?),
            "config/apply" => self.apply_config(parse(params)?),
            "config/apply/prepare" => self.prepare_apply(parse(params)?),
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
        if params.auth == Some(infobases::AuthMode::Windows)
            && (params.user.is_some() || params.password.is_some())
        {
            return Err(RpcError::invalid_params(
                "Windows authentication does not accept user or password",
            ));
        }
        let login = Login {
            auth: params.auth,
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
        self.connection_epoch = self.connection_epoch.wrapping_add(1);
        self.import_plans.clear();
        self.sessions.insert(id, session);
        Ok(result)
    }

    fn children(&mut self, params: ChildrenParams) -> Result<Value, RpcError> {
        let session = self.session(&params.infobase)?;
        let nodes = if params.main_configuration {
            session
                .configuration
                .main_view_mut()
                .children(params.node.as_deref())
        } else {
            session.configuration.children(params.node.as_deref())
        }
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
        if let Some(baseline) = params.version_baseline_dir {
            return serde_json::to_value(
                session
                    .configuration
                    .status_with_versions(
                        params.objects.as_deref(),
                        &params.source_dir,
                        &baseline,
                        &progress,
                    )
                    .map_err(failure)?,
            )
            .map_err(|error| failure(error.into()));
        }
        let statuses = session
            .configuration
            .status(params.objects.as_deref(), &params.source_dir, &progress)
            .map_err(failure)?;
        Ok(json!({ "objects": statuses }))
    }

    fn export_plan(&mut self, id: &Value, params: ExportPlanParams) -> Result<Value, RpcError> {
        let progress = self.progress(id);
        let session = self.session(&params.infobase)?;
        serde_json::to_value(
            session
                .configuration
                .prepare_editor_export(
                    &params.objects,
                    params.changed,
                    &params.source_dir,
                    &params.target_dir,
                    &progress,
                )
                .map_err(failure)?,
        )
        .map_err(|error| failure(error.into()))
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
            "files": exported.files.keys().collect::<Vec<_>>(),
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

    fn prepare_import(&mut self, params: ImportParams) -> Result<Value, RpcError> {
        let session = self.session(&params.infobase)?;
        let selection = crate::file_import::Selection {
            files: params.files,
            objects: params.objects,
            changed: params.changed,
        };
        let (plan, preflight) = match session.configuration.source() {
            ConfigSource::FileDb(target) => {
                let plan = crate::file_import::prepare(
                    target,
                    &params.source_dir,
                    &selection,
                    session.configuration.platform(),
                )
                .map_err(failure)?;
                let preflight = json!(plan.summary());
                (ImportPlan::FileDb(Box::new(plan)), preflight)
            }
            ConfigSource::Database { sql, database } => {
                let client = sql.client().ok_or_else(|| {
                    RpcError::invalid_params("SQL guarded import requires the built-in client")
                })?;
                let plan = sql_import::CommonModulePlan::prepare_supported(
                    client,
                    database,
                    &params.source_dir,
                    &selection,
                    params._verify,
                    Arc::new(|| crate::cancel::check().map_err(Into::into)),
                )
                .map_err(|error| {
                    if let Some(refused) = error.downcast_ref::<sql_import::RequestRefused>() {
                        RpcError::invalid_params(refused.to_string())
                    } else {
                        failure(error)
                    }
                })?;
                let preflight = json!(plan.summary());
                (ImportPlan::SqlModule(Box::new(plan)), preflight)
            }
            ConfigSource::RowsDir(_) => {
                return Err(RpcError::invalid_params(
                    "safe preflight requires a file-db or supported SQL connection",
                ));
            }
        };
        let token = self
            .import_plans
            .insert(
                &params.infobase,
                self.connection_epoch,
                plan,
                std::time::Instant::now(),
            )
            .map_err(|_| {
                RpcError::new(codes::INTERNAL_ERROR, "could not retain import preflight")
            })?;
        Ok(json!({"plan": token.to_string(), "preflight": preflight}))
    }

    fn import(&mut self, params: ImportParams) -> Result<Value, RpcError> {
        if let Some(token) = &params.plan {
            if !params.confirm {
                return Err(RpcError::new(
                    codes::CONFIRMATION_REQUIRED,
                    "confirm the prepared import before publication",
                ));
            }
            let token = uuid::Uuid::parse_str(token)
                .map_err(|_| RpcError::invalid_params("invalid import preflight; prepare again"))?;
            let plan = self
                .import_plans
                .take(
                    &token,
                    &params.infobase,
                    self.connection_epoch,
                    std::time::Instant::now(),
                )
                .map_err(|_| {
                    RpcError::invalid_params(
                        "import preflight expired or connection changed; prepare again",
                    )
                })?;
            let session = self.session(&params.infobase)?;
            return match (plan, session.configuration.source()) {
                (ImportPlan::FileDb(plan), ConfigSource::FileDb(target)) => {
                    let report = plan.publish(target).map_err(failure)?;
                    Ok(
                        json!({"report": {"backend":"file-db-partial-configsave", "stagedRows":report.rows_after, "backup":report.backup}}),
                    )
                }
                (ImportPlan::SqlModule(plan), ConfigSource::Database { sql, database }) => {
                    // A token selects its retained bytes; confirmation may not
                    // replace its selection or disable verification.
                    if !params.files.is_empty()
                        || !params.objects.is_empty()
                        || params.changed
                        || params._verify == Some(false)
                    {
                        return Err(RpcError::invalid_params(
                            "SQL publication accepts only its prepared token; prepare again",
                        ));
                    }
                    let client = sql.client().ok_or_else(|| {
                        RpcError::invalid_params("SQL guarded import requires the built-in client")
                    })?;
                    let rows = plan.staged_rows();
                    let backend = plan.backend();
                    let report = plan.publish(
                        client,
                        database,
                        &params.source_dir,
                        Arc::new(|| crate::cancel::check().map_err(Into::into)),
                    );
                    sql_import_reply(report, rows, backend)
                }
                _ => Err(RpcError::invalid_params(
                    "connection changed since preflight",
                )),
            };
        }
        if !params.files.is_empty() || params.changed {
            return Err(RpcError::invalid_params(
                "files/changed import requires objects/import/prepare",
            ));
        }
        if params.objects.is_empty() {
            return Err(RpcError::invalid_params("objects is empty"));
        }
        for name in &params.objects {
            if objects::source_prefixes(name).is_none() {
                return Err(RpcError::new(
                    codes::UNKNOWN_OBJECT,
                    format!("{name} is not an object name"),
                ));
            }
        }
        let session = self.session(&params.infobase)?;
        if let ConfigSource::RowsDir(_) = session.configuration.source() {
            return Err(offline("objects/import", session));
        }
        if matches!(session.configuration.source(), ConfigSource::FileDb(_)) {
            return Err(offline("objects/import", session));
        }
        // No direct editor RPC may enter the legacy full CONFIGSAVE replacement
        // path. Only an independently prepared, admitted one-shot SQL plan may publish.
        Err(RpcError::invalid_params(
            "SQL import requires guarded preflight; prepare again",
        ))
    }

    fn pending(&mut self, params: InfobaseParams) -> Result<Value, RpcError> {
        let session = self.session(&params.infobase)?;
        if let ConfigSource::FileDb(path) = session.configuration.source() {
            let mut db = ibcmd_filedb::FileDb::open(path).map_err(|e| failure(e.into()))?;
            let rows = db
                .config_headers("CONFIGSAVE")
                .map_err(|e| failure(e.into()))?;
            return Ok(
                json!({"pending": !rows.is_empty(), "stagedRows": rows.len(),
                "conclusive": true, "needsRestructuring": null}),
            );
        }
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

/// Preserve acknowledged/uncertain outcomes before generic cancellation mapping.
fn sql_import_reply(
    report: sql_import::transaction::Report,
    rows: usize,
    backend: &'static str,
) -> Result<Value, RpcError> {
    use sql_import::transaction::State;
    if !matches!(report.state, State::Committed | State::Unchanged)
        || report.error.is_some()
        || report.cleanup_uncertain
    {
        return Err(failure(report.into()));
    }
    Ok(json!({"report": {"backend":backend,
        "stagedRows":rows, "state":format!("{:?}",report.state), "nativeApplied":false}}))
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
        ConfigSource::FileDb(_) => format!("{method} is not yet available for file-db"),
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
        ConfigSource::FileDb(path) => format!(
            "infobase {}: file database {} (platform {})",
            session.entry.id,
            path.display(),
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

#[cfg(test)]
mod import_error_tests {
    use super::*;
    #[test]
    fn sql_publication_reply_preserves_terminal_outcomes() {
        use sql_import::transaction::{Report, State};
        for state in [State::Unchanged, State::Committed] {
            let value = sql_import_reply(
                Report {
                    state,
                    error: None,
                    cleanup_uncertain: false,
                },
                2,
                "mssql-common-module-configsave",
            )
            .unwrap();
            assert_eq!(value["report"]["state"], format!("{state:?}"));
            assert_eq!(value["report"]["nativeApplied"], false);
        }
        for (state, uncertain, code) in [
            (
                State::Committed,
                false,
                codes::IMPORT_COMMITTED_RECOVERY_REQUIRED,
            ),
            (State::Unknown, false, codes::IMPORT_OUTCOME_UNKNOWN),
            (State::RolledBack, true, codes::IMPORT_OUTCOME_UNKNOWN),
            (State::RolledBack, false, codes::REQUEST_CANCELLED),
        ] {
            let error = sql_import_reply(
                Report {
                    state,
                    error: Some(crate::cancel::Cancelled.into()),
                    cleanup_uncertain: uncertain,
                },
                2,
                "mssql-common-module-configsave",
            )
            .unwrap_err();
            assert_eq!(error.code, code);
            assert!(error.data.is_none());
        }
    }

    #[test]
    fn sql_reply_keeps_the_prepared_module_backend() {
        for backend in [
            "mssql-common-module-configsave",
            "mssql-document-form-module-configsave",
        ] {
            let value = sql_import_reply(
                sql_import::transaction::Report {
                    state: sql_import::transaction::State::Committed,
                    error: None,
                    cleanup_uncertain: false,
                },
                2,
                backend,
            )
            .unwrap();
            assert_eq!(value["report"]["backend"], backend);
            assert_eq!(value["report"]["state"], "Committed");
            assert_eq!(value["report"]["nativeApplied"], false);
        }
    }

    #[test]
    fn publication_panic_is_redacted_and_never_claims_no_write() {
        let payloads: Vec<Box<dyn std::any::Any + Send>> = vec![
            Box::new("private-panic-sentinel".to_owned()),
            Box::new("private-panic-sentinel"),
            Box::new(17_u32),
            Box::new(crate::cancel::Cancelled),
        ];
        for payload in payloads {
            let response = panic_failure("objects/import", payload);
            assert_eq!(response.code, codes::IMPORT_OUTCOME_UNKNOWN);
            assert_eq!(
                response.message,
                "Import outcome is unconfirmed; inspect the database before retrying"
            );
            assert!(response.data.is_none());
        }
        let response = panic_failure("objects/status", Box::new("existing status error"));
        assert_eq!(response.code, codes::INTERNAL_ERROR);
        assert_eq!(
            response.message,
            "objects/status failed inside the server: existing status error"
        );
    }

    #[test]
    fn queued_cancel_burns_import_token_even_with_malformed_or_stale_binding() {
        use std::time::Instant;
        for (target, epoch) in [
            (json!("db"), 1),
            (json!("other"), 1),
            (json!("db"), 2),
            (Value::Null, 1),
            (json!(17), 1),
        ] {
            let now = Instant::now();
            let mut plans = import_plans::Registry::new();
            let token = plans.insert("db", 1, (), now).unwrap();
            let other = plans.insert("unrelated", 1, (), now).unwrap();
            // Other malformed fields must not prevent revocation.
            let params = json!({"plan":token.to_string(),"infobase":target,
                "files":17,"confirm":"invalid"});
            discard_cancelled_import_plan(&mut plans, "objects/import", &params, epoch, now);
            assert_eq!(
                plans.take(&token, "db", 1, now),
                Err(import_plans::TakeError::Missing)
            );
            assert_eq!(plans.take(&other, "unrelated", 1, now), Ok(()));
        }
    }

    #[test]
    fn queued_cancel_does_not_revoke_unidentified_or_other_method_tokens() {
        let now = std::time::Instant::now();
        let mut plans = import_plans::Registry::new();
        let token = plans.insert("db", 1, (), now).unwrap();
        discard_cancelled_import_plan(
            &mut plans,
            "objects/status",
            &json!({"plan":token.to_string(),"infobase":"db"}),
            1,
            now,
        );
        for params in [
            Value::Null,
            json!({}),
            json!({"plan":null}),
            json!({"plan":17}),
            json!({"plan":"not-a-uuid"}),
        ] {
            discard_cancelled_import_plan(&mut plans, "objects/import", &params, 1, now);
        }
        assert_eq!(plans.take(&token, "db", 1, now), Ok(()));
    }

    #[test]
    fn sql_terminal_outcomes_precede_cancellation_and_are_redacted() {
        use sql_import::transaction::{Report, State};
        for (state, uncertain, expected) in [
            (State::Unknown, false, codes::IMPORT_OUTCOME_UNKNOWN),
            (
                State::Committed,
                false,
                codes::IMPORT_COMMITTED_RECOVERY_REQUIRED,
            ),
            (
                State::Committed,
                true,
                codes::IMPORT_COMMITTED_RECOVERY_REQUIRED,
            ),
            (State::RolledBack, true, codes::IMPORT_OUTCOME_UNKNOWN),
            (State::RolledBack, false, codes::REQUEST_CANCELLED),
            (State::NotPublished, false, codes::REQUEST_CANCELLED),
        ] {
            let cause = anyhow::Error::from(crate::cancel::Cancelled)
                .context("private-user/path/password-sentinel");
            let response = failure(
                anyhow::Error::from(Report {
                    state,
                    error: Some(cause),
                    cleanup_uncertain: uncertain,
                })
                .context("private-outer-sentinel"),
            );
            assert_eq!(response.code, expected);
            assert!(!response.message.contains("sentinel"));
            assert!(response.data.is_none());
        }
        let response = failure(anyhow::Error::from(Report {
            state: State::Unknown,
            error: Some(anyhow::anyhow!("private-noncancellation-sentinel")),
            cleanup_uncertain: true,
        }));
        assert_eq!(response.code, codes::IMPORT_OUTCOME_UNKNOWN);
        assert!(!response.message.contains("sentinel"));
        assert!(response.data.is_none());
    }

    #[test]
    fn source_cancel_and_published_recovery_have_distinct_safe_codes() {
        let cancelled_source =
            crate::mssql_source_change::SourceChangeError::Cancelled(crate::cancel::Cancelled);
        assert_eq!(
            failure(anyhow::Error::from(cancelled_source).context("source capture")).code,
            codes::REQUEST_CANCELLED
        );
        assert_eq!(
            failure(anyhow::anyhow!("ordinary I/O error")).code,
            codes::OPERATION_FAILED
        );
        let error = ibcmd_filedb::Error::Published {
            recovery: "/private/path-sentinel".into(),
            error: std::io::Error::other("private-user-sentinel"),
        };
        let response = failure(anyhow::Error::from(error).context("import"));
        assert_eq!(response.code, codes::IMPORT_PUBLISHED_RECOVERY_REQUIRED);
        assert!(!response.message.contains("sentinel"));
        assert!(response.message.contains("published"));
    }
}
