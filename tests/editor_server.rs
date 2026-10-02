//! `ibcmd-rs serve --stdio` driven through its pipes, as an editor drives it,
//! on offline infobases: folders of stored Config rows (`--rows-dir`) built
//! here from the `.cf` fixtures -- each top-level CF entry becomes
//! `<name>__part0.bin` holding its packed (raw deflate) bytes, the layout
//! `metadata_model::audit::read_stored_row` reads. No SQL Server is needed.
//!
//! The files the server exports must be the command line's: `cf export` of
//! the same file (the full export) and `mssql-dump-config --rows-dir
//! --object` (the selective one), byte for byte.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_ibcmd-rs");

/// (infobase id, fixture): a configuration with `Ext` bodies of its own,
/// a language and two common forms; constants, defined types, an exchange
/// plan and session parameters; a document and an information register.
const FIXTURES: &[(&str, &str)] = &[
    ("demo", "home_page/one_column/input.cf"),
    ("ep", "upgrade/exchange_plan/current.cf"),
    ("mf", "main_filter/cf/input.cf"),
];

/// Writes the rows of `cf` into `dir` as `--rows-dir` reads them.
fn rows_dir(cf: &Path, dir: &Path) {
    let limits =
        ibcmd_core::limits::ResourceLimits::for_input_bytes(fs::metadata(cf).unwrap().len());
    let profile = ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli").unwrap();
    let archive =
        ibcmd_cf::archive::decode_packed_archive(fs::File::open(cf).unwrap(), limits, profile)
            .unwrap();
    fs::create_dir_all(dir).unwrap();
    for entry in archive.entries() {
        fs::write(
            dir.join(format!("{}__part0.bin", entry.name())),
            entry.payload(),
        )
        .unwrap();
    }
}

/// The full export of the command line, without `ConfigDumpInfo.xml` (only
/// a full export writes it).
fn reference(cf: &Path, dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let output = common::export(cf, dir);
    assert!(
        output.status.success(),
        "cf export failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut files = common::files(dir);
    assert!(files.remove("ConfigDumpInfo.xml").is_some());
    files
}

/// One test's infobases: rows folders, references and a settings file.
struct Corpus {
    root: PathBuf,
    settings: PathBuf,
}

impl Corpus {
    fn new(tag: &str) -> Self {
        let root = common::temp_dir(&format!("editor-server-{tag}"));
        for (id, fixture) in FIXTURES {
            let cf = common::fixture(fixture);
            rows_dir(&cf, &root.join("rows").join(id));
        }
        // The first infobase comes from the settings file, with its rows
        // folder relative to the file; the others from `initialize`.
        let settings = root.join("ibcmd-rs.toml");
        fs::write(
            &settings,
            "platform = \"8.3.27\"\n\n[[database]]\nname = \"demo\"\nplatform = \"8.3.27\"\nrows-dir = \"rows/demo\"\n\n[[database]]\nserver = \"sql01\"\nname = \"bsp_*\"\nplatform = \"8.3.27\"\n",
        )
        .unwrap();
        Self { root, settings }
    }

    fn rows(&self, id: &str) -> PathBuf {
        self.root.join("rows").join(id)
    }

    fn reference(&self, id: &str) -> (PathBuf, BTreeMap<String, Vec<u8>>) {
        let fixture = FIXTURES.iter().find(|(of, _)| *of == id).unwrap().1;
        let dir = self.root.join("reference").join(id);
        let files = reference(&common::fixture(fixture), &dir);
        (dir, files)
    }

    fn initialize_params(&self) -> Value {
        json!({
            "protocolVersion": "1.0",
            "clientInfo": {"name": "editor-server test", "version": "0"},
            "workspaceFolder": self.root,
            "infobases": [
                {"name": "ep", "rowsDir": self.rows("ep"), "platform": "8.3.27"},
                {"name": "mf", "rowsDir": self.rows("mf"), "platform": "8.3.27"},
            ],
        })
    }
}

/// A running server and the notifications it sent.
struct Server {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
    notifications: Vec<Value>,
}

impl Server {
    fn start(corpus: &Corpus) -> Self {
        let mut child = Command::new(BIN)
            .args(["serve", "--stdio"])
            .current_dir(&corpus.root)
            .env("IBCMD_RS_CONFIG", &corpus.settings)
            .env_remove("IBCMD_RS_PLATFORM")
            .env_remove("IBCMD_DB_PSW")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start ibcmd-rs serve");
        let stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
            next_id: 0,
            notifications: Vec::new(),
        }
    }

    fn write_raw(&mut self, bytes: &[u8]) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        stdin.write_all(bytes).unwrap();
        stdin.flush().unwrap();
    }

    fn frame(message: &Value) -> Vec<u8> {
        let body = serde_json::to_vec(message).unwrap();
        let mut frame = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        frame.extend(body);
        frame
    }

    fn read_message(&mut self) -> Value {
        let mut length = None;
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "the server closed its output"
            );
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            let (name, value) = line.split_once(':').unwrap();
            if name.eq_ignore_ascii_case("Content-Length") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        let mut body = vec![0; length.expect("Content-Length")];
        self.stdout.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// Reads until the response to `id`, keeping the notifications.
    fn response(&mut self, id: &Value) -> Value {
        loop {
            let message = self.read_message();
            if message.get("id") == Some(id) && message.get("method").is_none() {
                return message;
            }
            assert!(message.get("method").is_some(), "unexpected {message}");
            self.notifications.push(message);
        }
    }

    fn send(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = json!(self.next_id);
        let frame =
            Self::frame(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        self.write_raw(&frame);
        id
    }

    /// The whole response.
    fn call(&mut self, method: &str, params: Value) -> Value {
        let id = self.send(method, params);
        let response = self.response(&id);
        assert_eq!(response["jsonrpc"], "2.0");
        response
    }

    /// The result, which must be there.
    fn ok(&mut self, method: &str, params: Value) -> Value {
        let response = self.call(method, params);
        assert!(
            response.get("error").is_none(),
            "{method} failed: {}",
            response["error"]
        );
        response["result"].clone()
    }

    /// The error code of a call that must fail.
    fn error(&mut self, method: &str, params: Value) -> i64 {
        let response = self.call(method, params);
        response["error"]["code"]
            .as_i64()
            .unwrap_or_else(|| panic!("{method} did not fail: {response}"))
    }

    fn notify(&mut self, method: &str, params: Value) {
        let frame = Self::frame(&json!({"jsonrpc": "2.0", "method": method, "params": params}));
        self.write_raw(&frame);
    }

    /// Sends `exit` (or closes stdin) and waits for the exit code.
    fn finish(mut self, exit: bool) -> i32 {
        if exit {
            self.notify("exit", Value::Null);
        }
        drop(self.stdin.take());
        self.child.wait().unwrap().code().unwrap()
    }
}

/// Every object node of the tree, depth first: (full name, descriptor path).
fn walk(server: &mut Server, infobase: &str) -> Vec<(String, String)> {
    let mut objects = Vec::new();
    let mut pending = vec![Value::Null];
    while let Some(node) = pending.pop() {
        let result = server.ok("tree/children", json!({"infobase": infobase, "node": node}));
        for child in result["nodes"].as_array().unwrap() {
            if child["nodeType"] != "group" {
                objects.push((
                    child["fullName"].as_str().unwrap().to_string(),
                    child["path"].as_str().unwrap().to_string(),
                ));
            }
            if child["hasChildren"] == true {
                pending.push(child["id"].clone());
            }
        }
    }
    objects
}

/// The descriptor files of a full export: `Configuration.xml` and every
/// `<Collection>/<Name>.xml` (and deeper `.../<Collection>/<Name>.xml`).
fn descriptors(files: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let mut paths = files
        .keys()
        .filter(|path| {
            let parts = path.split('/').collect::<Vec<_>>();
            path.ends_with(".xml")
                && (parts.len() == 1 && parts[0] == "Configuration.xml"
                    || parts.len() % 2 == 0
                        && !parts.contains(&"Ext")
                        && !path.ends_with("/Ext/Form.xml"))
        })
        .cloned()
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn content(result: &Value) -> Vec<u8> {
    let text = result["content"].as_str().unwrap();
    match result["encoding"].as_str().unwrap() {
        "utf-8" => text.as_bytes().to_vec(),
        "base64" => decode_base64(text),
        other => panic!("unknown encoding {other}"),
    }
}

fn decode_base64(text: &str) -> Vec<u8> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        buffer = (buffer << 6) | ALPHABET.iter().position(|c| *c == byte).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

#[test]
fn serve_speaks_the_protocol_and_its_lifecycle() {
    let corpus = Corpus::new("protocol");
    let mut server = Server::start(&corpus);

    // Before initialize, and with a protocol of another major.
    assert_eq!(server.error("infobases/list", json!({})), -32002);
    assert_eq!(server.error("initialize", json!({})), -32602);
    let refused = server.call("initialize", json!({"protocolVersion": "2.0"}));
    assert_eq!(refused["error"]["code"], -32010);
    assert_eq!(refused["error"]["data"]["serverProtocolVersion"], "1.0");
    assert_eq!(
        server.error("initialize", json!({"protocolVersion": "1.1"})),
        -32010
    );

    let initialized = server.ok("initialize", corpus.initialize_params());
    assert_eq!(initialized["protocolVersion"], "1.0");
    assert_eq!(initialized["serverInfo"]["name"], "ibcmd-rs");
    assert_eq!(
        initialized["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    let methods = initialized["capabilities"]["methods"].as_array().unwrap();
    for method in [
        "infobases/list",
        "tree/children",
        "objects/status",
        "objects/export",
        "objects/import",
        "source/read",
        "config/pending",
    ] {
        assert!(methods.contains(&json!(method)), "{method}");
    }
    assert_eq!(
        initialized["settingsFiles"],
        json!([corpus.settings.display().to_string()])
    );
    assert_eq!(
        server.error("initialize", corpus.initialize_params()),
        -32600
    );

    // The settings' infobases first, then the editor's.
    let list = server.ok("infobases/list", json!({}));
    let infobases = list["infobases"].as_array().unwrap();
    let ids = infobases
        .iter()
        .map(|entry| entry["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["demo", "sql01/bsp_*", "ep", "mf"]);
    assert_eq!(infobases[0]["source"], "rows-dir");
    assert_eq!(
        infobases[0]["rowsDir"],
        json!(corpus.root.join("rows/demo").display().to_string())
    );
    assert_eq!(infobases[0]["xmlVersion"], "2.20");
    assert!(
        infobases[0]["origin"]
            .as_str()
            .unwrap()
            .ends_with("ibcmd-rs.toml:5")
    );
    assert_eq!(infobases[1]["source"], "mssql");
    assert_eq!(infobases[1]["mask"], true);
    assert_eq!(infobases[2]["origin"], "initialize");
    assert_eq!(infobases[2]["connected"], false);

    // The errors of the protocol.
    assert_eq!(server.error("no/such", json!({})), -32601);
    assert_eq!(
        server.error("tree/children", json!({"infobase": "nope"})),
        -32011
    );
    assert_eq!(server.error("tree/children", json!({})), -32602);
    // A mask names no database until connect says which.
    assert_eq!(
        server.error("tree/children", json!({"infobase": "sql01/bsp_*"})),
        -32602
    );
    assert_eq!(
        server.error(
            "tree/children",
            json!({"infobase": "demo", "node": "Catalog.Нет"})
        ),
        -32012
    );
    server.write_raw(b"Content-Length: 9\r\n\r\nnot json!");
    assert_eq!(server.response(&Value::Null)["error"]["code"], -32700);

    // A rows folder holds no ConfigSave and is never written.
    // The password arrives in memory and goes nowhere: not into the
    // answer, not into a notification.
    let connected = server.ok(
        "infobases/connect",
        json!({"infobase": "ep", "user": "reader", "password": "pa55-in-memory"}),
    );
    assert!(!connected.to_string().contains("pa55-in-memory"));
    assert!(
        server
            .notifications
            .iter()
            .all(|message| !message.to_string().contains("pa55-in-memory"))
    );
    assert_eq!(connected["source"], "rows-dir");
    assert_eq!(connected["configuration"]["nodeType"], "configuration");
    assert_eq!(
        server.error("config/pending", json!({"infobase": "ep"})),
        -32013
    );
    assert_eq!(
        server.error(
            "objects/import",
            json!({"infobase": "ep", "sourceDir": corpus.root, "objects": ["Configuration"], "confirm": true})
        ),
        -32013
    );
    let list = server.ok("infobases/list", json!({}));
    assert_eq!(list["infobases"][2]["connected"], true);

    assert_eq!(server.ok("shutdown", Value::Null), Value::Null);
    assert_eq!(server.error("infobases/list", json!({})), -32600);
    assert_eq!(server.finish(true), 0, "exit after shutdown");

    // The end of stdin without shutdown ends the server with 1.
    let mut server = Server::start(&corpus);
    server.ok("initialize", corpus.initialize_params());
    assert_eq!(server.finish(false), 1);
}

#[test]
fn invalid_rpc_envelopes_do_not_dispatch_operations() {
    let corpus = Corpus::new("invalid-envelopes");
    let mut server = Server::start(&corpus);
    for message in [
        json!({"id": 1, "method": "initialize", "params": corpus.initialize_params()}),
        json!({"jsonrpc": "1.0", "id": 1, "method": "initialize", "params": corpus.initialize_params()}),
        json!({"jsonrpc": "2.0", "id": true, "method": "initialize", "params": corpus.initialize_params()}),
        json!({"jsonrpc": "2.0", "id": {}, "method": "initialize", "params": corpus.initialize_params()}),
        json!({"jsonrpc": "2.0", "id": [], "method": "initialize", "params": corpus.initialize_params()}),
    ] {
        server.write_raw(&Server::frame(&message));
        let response = server.read_message();
        assert_eq!(response["error"]["code"], -32600, "{response}");
    }
    server.ok("initialize", corpus.initialize_params());
    // An invalid exit notification must not terminate the server.
    server.write_raw(&Server::frame(&json!({"method": "exit"})));
    let rejected = server.read_message();
    assert_eq!(rejected["error"]["code"], -32600);
    assert!(rejected["id"].is_null());
    server.ok("infobases/list", Value::Null);
    server.ok("shutdown", Value::Null);
    assert_eq!(server.finish(true), 0);
}

#[test]
fn serve_exports_objects_as_the_command_line_does() {
    let corpus = Corpus::new("export");
    let mut server = Server::start(&corpus);
    server.ok("initialize", corpus.initialize_params());
    for (id, _) in FIXTURES {
        let (_, expected) = corpus.reference(id);

        // The tree names every object a full export writes a descriptor of.
        let objects = walk(&mut server, id);
        let mut paths = objects
            .iter()
            .map(|(_, path)| path.clone())
            .collect::<Vec<_>>();
        paths.sort();
        assert_eq!(paths, descriptors(&expected), "{id}: the tree");
        let names = objects
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        assert_eq!(names[0], "Configuration");

        // All objects exported into an empty folder are the full export.
        let target = corpus.root.join("server").join(id);
        let result = server.ok(
            "objects/export",
            json!({"infobase": id, "objects": names, "targetDir": target}),
        );
        assert_eq!(result["objects"].as_array().unwrap().len(), names.len());
        let exported = common::files(&target);
        assert_eq!(
            exported.keys().collect::<Vec<_>>(),
            expected.keys().collect::<Vec<_>>(),
            "{id}: the exported files"
        );
        for (path, bytes) in &expected {
            assert!(exported[path] == *bytes, "{id}: {path} differs");
        }
        // Progress was reported for the request.
        assert!(server.notifications.iter().any(|message| {
            message["method"] == "progress" && message["params"]["message"] == "Configuration"
        }));

        // The selective export of the command line writes the same files.
        let cli = corpus.root.join("cli").join(id);
        let mut command = Command::new(BIN);
        command
            .arg("mssql-dump-config")
            .arg("--rows-dir")
            .arg(corpus.rows(id))
            .args([
                "--extract-metadata-xml",
                "--extract-module-text",
                "--no-binary-rows",
                "--platform",
                "8.3.27",
                "-o",
            ])
            .arg(&cli);
        for name in &names {
            command.args(["--object", name]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut from_cli = common::files(&cli);
        assert!(from_cli.remove("manifest.json").is_some());
        assert!(from_cli == exported, "{id}: mssql-dump-config --object");

        // Every file reads back as the export writes it, nothing written.
        for (path, bytes) in &expected {
            let read = server.ok("source/read", json!({"infobase": id, "path": path}));
            assert_eq!(read["exists"], true, "{id}: {path}");
            assert_eq!(read["path"], json!(path));
            assert!(content(&read) == *bytes, "{id}: source/read {path}");
        }
        let missing = server.ok(
            "source/read",
            json!({"infobase": id, "path": "Ext/NoSuchModule.bsl"}),
        );
        assert_eq!(missing["exists"], false);
        assert_eq!(missing["object"], "Configuration");
        assert_eq!(
            server.error(
                "source/read",
                json!({"infobase": id, "path": "../Configuration.xml"})
            ),
            -32602
        );
    }

    // One object alone, into a folder that holds others: only its files.
    let target = corpus.root.join("server").join("ep");
    fs::write(target.join("README.md"), "not an object").unwrap();
    let constant = walk(&mut server, "ep")
        .into_iter()
        .find(|(name, _)| name.starts_with("Constant."))
        .unwrap();
    fs::remove_file(target.join(&constant.1)).unwrap();
    let result = server.ok(
        "objects/export",
        json!({"infobase": "ep", "objects": [constant.0], "targetDir": target}),
    );
    assert_eq!(result["objects"][0]["written"], json!([constant.1]));
    assert_eq!(result["objects"][0]["unchanged"], 0);
    assert_eq!(
        fs::read_to_string(target.join("README.md")).unwrap(),
        "not an object"
    );

    assert_eq!(server.ok("shutdown", Value::Null), Value::Null);
    assert_eq!(server.finish(true), 0);
}

#[test]
fn serve_compares_a_source_folder_with_the_base() {
    let corpus = Corpus::new("status");
    let (reference, expected) = corpus.reference("demo");
    let folder = corpus.root.join("work");
    for (path, bytes) in &expected {
        let target = folder.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    let mut server = Server::start(&corpus);
    server.ok("initialize", corpus.initialize_params());

    let status = |server: &mut Server, objects: Option<Vec<&str>>| -> BTreeMap<String, Value> {
        let mut params = json!({"infobase": "demo", "sourceDir": folder});
        if let Some(objects) = objects {
            params["objects"] = json!(objects);
        }
        server.ok("objects/status", params)["objects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|object| {
                (
                    object["object"].as_str().unwrap().to_string(),
                    object.clone(),
                )
            })
            .collect()
    };

    // An export of the base is the base.
    let all = status(&mut server, None);
    assert_eq!(all.len(), 4, "{all:?}");
    assert!(
        all.values().all(|object| object["status"] == "same"),
        "{all:?}"
    );

    // A module edited, a form file gone, a file added, an object only in
    // the folder, an object not exported.
    let forms = expected
        .keys()
        .filter(|path| {
            path.starts_with("CommonForms/")
                && path.ends_with(".xml")
                && path.matches('/').count() == 1
        })
        .cloned()
        .collect::<Vec<_>>();
    let edited = forms[0].trim_end_matches(".xml").to_string();
    let module = format!("{edited}/Ext/Form/Module.bsl");
    fs::write(folder.join(&module), "// edited\r\n").unwrap();
    fs::remove_file(folder.join(format!("{edited}/Ext/Form.xml"))).unwrap();
    fs::write(folder.join(format!("{edited}/Ext/Extra.txt")), "extra").unwrap();
    fs::create_dir_all(folder.join("Catalogs")).unwrap();
    fs::write(folder.join("Catalogs/ТолькоВПапке.xml"), "<x/>").unwrap();
    let other = forms[1].trim_end_matches(".xml").to_string();
    fs::remove_file(folder.join(&forms[1])).unwrap();
    fs::remove_dir_all(folder.join(&other)).unwrap();
    // Outside every object: never reported, never touched.
    fs::create_dir_all(folder.join(".git")).unwrap();
    fs::write(folder.join(".git/HEAD"), "ref").unwrap();

    let edited_name = format!("CommonForm.{}", edited.trim_start_matches("CommonForms/"));
    let other_name = format!("CommonForm.{}", other.trim_start_matches("CommonForms/"));
    let all = status(&mut server, None);
    assert_eq!(all["Configuration"]["status"], "same");
    let changed = &all[&edited_name];
    assert_eq!(changed["status"], "modified");
    assert_eq!(changed["changed"], json!([module]));
    assert_eq!(
        changed["baseOnly"],
        json!([format!("{edited}/Ext/Form.xml")])
    );
    assert_eq!(
        changed["localOnly"],
        json!([format!("{edited}/Ext/Extra.txt")])
    );
    assert_eq!(all[&other_name]["status"], "notExported");
    assert_eq!(all["Catalog.ТолькоВПапке"]["status"], "notInBase");
    assert_eq!(
        all["Catalog.ТолькоВПапке"]["localOnly"],
        json!(["Catalogs/ТолькоВПапке.xml"])
    );

    // Asked by name: only those, an unknown one only in the folder too.
    let some = status(
        &mut server,
        Some(vec![&edited_name, "Catalog.ТолькоВПапке"]),
    );
    assert_eq!(some.len(), 2);
    assert_eq!(some[&edited_name]["status"], "modified");
    assert_eq!(some["Catalog.ТолькоВПапке"]["status"], "notInBase");
    // What is no object name at all is refused.
    assert_eq!(
        server.error(
            "objects/status",
            json!({"infobase": "demo", "sourceDir": folder, "objects": ["Нечто"]})
        ),
        -32012
    );

    // The command line answers the same.
    let output = Command::new(BIN)
        .args(["objects", "status", "--platform", "8.3.27", "--rows-dir"])
        .arg(corpus.rows("demo"))
        .arg("--source-dir")
        .arg(&folder)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let from_cli: Value = serde_json::from_slice(&output.stdout).unwrap();
    let from_cli = from_cli["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|object| {
            (
                object["object"].as_str().unwrap().to_string(),
                object.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(from_cli, all);

    // Exporting the two objects restores them; nothing else changes.
    let result = server.ok(
        "objects/export",
        json!({"infobase": "demo", "objects": [edited_name, other_name], "targetDir": folder}),
    );
    assert_eq!(
        result["objects"][0]["removed"],
        json!([format!("{edited}/Ext/Extra.txt")])
    );
    let all = status(&mut server, None);
    for (name, object) in &all {
        if name == "Catalog.ТолькоВПапке" {
            assert_eq!(object["status"], "notInBase");
        } else {
            assert_eq!(object["status"], "same", "{name}: {object}");
        }
    }
    assert_eq!(fs::read_to_string(folder.join(".git/HEAD")).unwrap(), "ref");
    fs::remove_dir_all(folder.join(".git")).unwrap();
    fs::remove_dir_all(folder.join("Catalogs")).unwrap();
    let mut restored = common::files(&folder);
    let mut full = common::files(&reference);
    restored.remove("ConfigDumpInfo.xml");
    full.remove("ConfigDumpInfo.xml");
    assert!(restored == full, "the folder is the export again");

    // The tree on the command line is the server's.
    let output = Command::new(BIN)
        .args([
            "objects",
            "tree",
            "--platform",
            "8.3.27",
            "--node",
            "Configuration",
            "--rows-dir",
        ])
        .arg(corpus.rows("demo"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let from_cli: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        from_cli,
        server.ok(
            "tree/children",
            json!({"infobase": "demo", "node": "Configuration"})
        )
    );

    assert_eq!(server.ok("shutdown", Value::Null), Value::Null);
    assert_eq!(server.finish(true), 0);
}

#[test]
fn serve_cancels_a_request_that_waits() {
    let corpus = Corpus::new("cancel");
    let mut server = Server::start(&corpus);
    server.ok("initialize", corpus.initialize_params());
    let folder = corpus.root.join("work");
    fs::create_dir_all(&folder).unwrap();
    // Whole-configuration comparisons, then an export and its cancellation
    // in the same write: the export is still queued behind the comparisons
    // (each one exports a configuration) when the reader takes the
    // cancellation a few microseconds later.
    let mut frames = Vec::new();
    for (id, method, params) in [
        (
            1001,
            "objects/status",
            json!({"infobase": "ep", "sourceDir": folder}),
        ),
        (
            1003,
            "objects/status",
            json!({"infobase": "mf", "sourceDir": folder}),
        ),
        (
            1004,
            "objects/status",
            json!({"infobase": "demo", "sourceDir": folder}),
        ),
        (
            1002,
            "objects/export",
            json!({"infobase": "ep", "objects": ["Configuration"], "targetDir": folder}),
        ),
    ] {
        frames.extend(Server::frame(
            &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}),
        ));
    }
    frames.extend(Server::frame(
        &json!({"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 1002}}),
    ));
    // A cancellation of an unknown request is ignored.
    frames.extend(Server::frame(
        &json!({"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 4242}}),
    ));
    server.write_raw(&frames);
    for id in [1001, 1003, 1004] {
        let status = server.response(&json!(id));
        assert!(status.get("result").is_some(), "{status}");
    }
    let export = server.response(&json!(1002));
    assert_eq!(export["error"]["code"], -32800, "{export}");
    assert!(!folder.join("Configuration.xml").exists());
    // The server goes on serving.
    let result = server.ok(
        "objects/export",
        json!({"infobase": "ep", "objects": ["Configuration"], "targetDir": folder}),
    );
    assert!(
        !result["objects"][0]["written"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(server.ok("shutdown", Value::Null), Value::Null);
    assert_eq!(server.finish(true), 0);
}

/// Rewrites one stored row of a rows folder: inflated, edited, deflated
/// again (raw deflate, as the table stores it).
fn edit_row(rows: &Path, row: &str, edit: impl FnOnce(String) -> String) {
    let path = rows.join(format!("{row}__part0.bin"));
    let packed = fs::read(&path).unwrap();
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(&packed[..])
        .read_to_string(&mut text)
        .unwrap();
    let text = edit(text);
    let mut encoder =
        flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    fs::write(&path, encoder.finish().unwrap()).unwrap();
}

#[test]
fn serve_rereads_an_object_when_versions_changes() {
    let corpus = Corpus::new("versions");
    let (reference, _) = corpus.reference("demo");
    let mut server = Server::start(&corpus);
    server.ok("initialize", corpus.initialize_params());

    let forms = server.ok(
        "tree/children",
        json!({"infobase": "demo", "node": "Configuration/CommonForms"}),
    );
    let form = forms["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["label"] == "ФормаСлева")
        .unwrap()
        .clone();
    let uuid = form["uuid"].as_str().unwrap().to_string();
    let module = "CommonForms/ФормаСлева/Ext/Form/Module.bsl";
    let read =
        |server: &mut Server| server.ok("source/read", json!({"infobase": "demo", "path": module}));
    let before = read(&mut server);
    assert!(
        String::from_utf8(content(&before))
            .unwrap()
            .contains("Номер = 1;")
    );

    // The form's body row changes and `versions` does not: the server
    // answers from what it read, as `versions` is its consistency signal
    // (the platform writes a new version for every row it writes).
    let body = format!("{uuid}.0");
    edit_row(&corpus.rows("demo"), &body, |text| {
        assert!(text.contains("Номер = 1;"));
        text.replace("Номер = 1;", "Номер = 2;")
    });
    assert_eq!(read(&mut server), before);

    // A new version of the row: the inventory and the exported files are
    // read anew.
    edit_row(&corpus.rows("demo"), "versions", |text| {
        let key = format!("\"{body}\",");
        let at = text.find(&key).unwrap() + key.len();
        let mut text = text;
        text.replace_range(at..at + 36, "0f0f0f0f-0000-4000-8000-000000000001");
        text
    });
    let after = read(&mut server);
    assert!(
        String::from_utf8(content(&after))
            .unwrap()
            .contains("Номер = 2;")
    );
    assert_ne!(after["version"], before["version"]);
    let status = server.ok(
        "objects/status",
        json!({"infobase": "demo", "sourceDir": reference, "objects": [form["fullName"]]}),
    );
    assert_eq!(status["objects"][0]["status"], "modified");
    assert_eq!(status["objects"][0]["changed"], json!([module]));
    // Nothing else changed.
    let configuration = server.ok(
        "objects/status",
        json!({"infobase": "demo", "sourceDir": reference, "objects": ["Configuration"]}),
    );
    assert_eq!(configuration["objects"][0]["status"], "same");

    assert_eq!(server.ok("shutdown", Value::Null), Value::Null);
    assert_eq!(server.finish(true), 0);
}
