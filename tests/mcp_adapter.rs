//! Real SDK client launches the binary over supported MCP stdio; no native platform.
//! Current descriptor capability boundary is Unix-only.
use rmcp::{ServiceExt, model::*, transport::TokioChildProcess};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, process::Stdio};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external")
}
fn corpus() -> (crate::common::Scratch, PathBuf, PathBuf) {
    let root = crate::common::Scratch::adopt(
        std::env::temp_dir().join(format!("ibcmd-mcp-{}", uuid::Uuid::new_v4())),
    );
    fs::create_dir_all(root.join("rows")).unwrap();
    fs::create_dir_all(root.join("source")).unwrap();
    let cf = fixture_root().join("home_page/one_column/input.cf");
    let limits =
        ibcmd_core::limits::ResourceLimits::for_input_bytes(fs::metadata(&cf).unwrap().len());
    let profile = ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli").unwrap();
    let archive =
        ibcmd_cf::archive::decode_packed_archive(fs::File::open(cf).unwrap(), limits, profile)
            .unwrap();
    for entry in archive.entries() {
        fs::write(
            root.join("rows")
                .join(format!("{}__part0.bin", entry.name())),
            entry.payload(),
        )
        .unwrap();
    }
    let settings = root.join("ibcmd-rs.toml");
    fs::write(
        &settings,
        "[[database]]\nname = \"demo\"\nplatform = \"8.3.27\"\nrows-dir = \"rows\"\n",
    )
    .unwrap();
    let policy = root.join("mcp.toml");
    fs::write(
        &policy,
        "[mcp]\ndatabase = \"demo\"\nsource-root = \"source\"\n",
    )
    .unwrap();
    (root, settings, policy)
}
async fn call(
    client: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    name: &str,
    args: Value,
) -> CallToolResult {
    client
        .call_tool(
            CallToolRequestParams::new(name.to_owned())
                .with_arguments(args.as_object().unwrap().clone()),
        )
        .await
        .unwrap()
}
fn value(result: &CallToolResult) -> Value {
    assert_eq!(result.is_error, Some(false), "{result:?}");
    serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap()
}
#[tokio::test]
async fn real_mcp_client_reads_fixed_fixture_and_enforces_policy() {
    let (root, settings, policy) = corpus();
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command
        .arg("mcp")
        .arg("--settings")
        .arg(&settings)
        .arg("--policy")
        .arg(&policy)
        .stderr(Stdio::piped());
    let client = ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 6);
    assert!(
        tools
            .iter()
            .all(|t| t.input_schema.get("additionalProperties") == Some(&json!(false)))
    );
    let tree = value(&call(&client, "tree", json!({})).await);
    assert!(!tree["nodes"].as_array().unwrap().is_empty());
    // Select the configuration itself and prove export/read/compare use the real engine.
    let exported = value(&call(&client, "export", json!({"objects":["Configuration"]})).await);
    let files = exported["objects"][0]["files"].as_object().unwrap();
    assert!(!files.is_empty());
    let reference = root.join("cli-reference");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(fixture_root().join("home_page/one_column/input.cf"))
        .arg(&reference)
        .args(["--source-version", "2.20", "--overwrite"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (path, file) in files {
        assert_eq!(file["encoding"], "utf-8", "fixture should be text: {path}");
        assert_eq!(
            fs::read(reference.join(path)).unwrap(),
            file["content"].as_str().unwrap().as_bytes()
        );
        let target = root.join("source").join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, file["content"].as_str().unwrap()).unwrap();
    }
    let path = files
        .keys()
        .find(|path| path.as_str() == "Configuration.xml")
        .unwrap();
    let read = value(&call(&client, "read", json!({"path":path})).await);
    assert_eq!(read["file"], files[path]);
    for tool in ["status", "compare"] {
        let comparison = value(&call(&client, tool, json!({"objects":["Configuration"]})).await);
        assert_eq!(comparison["comparisonScope"], "selected-objects");
        assert_eq!(comparison["objects"][0]["status"], "same");
        for field in ["changed", "baseOnly", "localOnly"] {
            assert_eq!(comparison["objects"][0][field], json!([]));
        }
    }
    let extra = "Ext/LocalOnly.bsl";
    fs::create_dir_all(root.join("source/Ext")).unwrap();
    fs::write(root.join("source").join(extra), "extra source file").unwrap();
    let local_only = value(&call(&client, "status", json!({"objects":["Configuration"]})).await);
    assert_eq!(local_only["objects"][0]["status"], "modified");
    assert!(
        local_only["objects"][0]["localOnly"]
            .as_array()
            .unwrap()
            .contains(&json!(extra))
    );
    fs::rename(
        root.join("source").join(extra),
        root.join("local-only-evidence.bsl"),
    )
    .unwrap();
    fs::rename(
        root.join("source").join(path),
        root.join("missing-evidence.xml"),
    )
    .unwrap();
    let missing = value(&call(&client, "compare", json!({"objects":["Configuration"]})).await);
    assert!(
        missing["objects"][0]["baseOnly"]
            .as_array()
            .unwrap()
            .contains(&json!(path))
    );
    fs::rename(
        root.join("missing-evidence.xml"),
        root.join("source").join(path),
    )
    .unwrap();
    fs::write(root.join("source").join(path), "changed").unwrap();
    let changed = value(&call(&client, "compare", json!({"objects":["Configuration"]})).await);
    assert!(
        changed["objects"][0]["changed"]
            .as_array()
            .unwrap()
            .contains(&json!(path))
    );
    fs::create_dir_all(root.join("source/CommonModules/OnlyLocal/Ext")).unwrap();
    fs::write(root.join("source/CommonModules/OnlyLocal.xml"), "<local />").unwrap();
    fs::write(
        root.join("source/CommonModules/OnlyLocal/Ext/Module.bsl"),
        "local module",
    )
    .unwrap();
    let absent = value(
        &call(
            &client,
            "status",
            json!({"objects":["CommonModule.OnlyLocal"]}),
        )
        .await,
    );
    assert_eq!(absent["objects"][0]["status"], "notInBase");
    assert_eq!(
        absent["objects"][0]["localOnly"].as_array().unwrap().len(),
        2
    );

    #[cfg(unix)]
    {
        // Source and row symlinks are refused, even if they point inside the root.
        let original = root.join("source").join(path);
        let saved = root.join("source").join("saved.xml");
        fs::rename(&original, &saved).unwrap();
        std::os::unix::fs::symlink(&saved, &original).unwrap();
        assert_eq!(
            call(&client, "compare", json!({"objects":["Configuration"]}))
                .await
                .is_error,
            Some(true)
        );
        let row = fs::read_dir(root.join("rows"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let moved = root.join("saved-row.bin");
        fs::rename(&row, &moved).unwrap();
        std::os::unix::fs::symlink(&moved, &row).unwrap();
        assert_eq!(call(&client, "tree", json!({})).await.is_error, Some(true));
        // Move the symlink to evidence rather than deleting it, then restore input.
        fs::rename(&row, root.join("row-symlink-evidence")).unwrap();
        fs::rename(moved, row).unwrap();
    }
    #[cfg(unix)]
    {
        // Even a symlink to the SAME original root inode is a changed binding.
        fs::rename(root.join("source"), root.join("saved-source-root")).unwrap();
        std::os::unix::fs::symlink(root.join("saved-source-root"), root.join("source")).unwrap();
        assert_eq!(call(&client, "tree", json!({})).await.is_error, Some(true));
        fs::rename(
            root.join("source"),
            root.join("source-root-symlink-evidence"),
        )
        .unwrap();
        fs::rename(root.join("saved-source-root"), root.join("source")).unwrap();
    }
    for args in [
        json!({"server":"other"}),
        json!({"database":"other"}),
        json!({"settings":"elsewhere"}),
    ] {
        assert_eq!(call(&client, "tree", args).await.is_error, Some(true));
    }
    for path in [
        "../secret",
        "/etc/passwd",
        "Catalogs/../secret",
        "Catalogs\\secret",
        "Catalogs/x:secret",
    ] {
        assert_eq!(
            call(&client, "read", json!({"path":path})).await.is_error,
            Some(true)
        );
    }
    assert_eq!(
        call(&client, "import", json!({"objects":["Configuration"]}))
            .await
            .is_error,
        Some(true)
    );
    assert!(
        client
            .call_tool(CallToolRequestParams::new("unknown"))
            .await
            .is_err()
    );
    fs::write(
        &policy,
        "[mcp]\ndatabase = \"other\"\nsource-root = \"source\"\n",
    )
    .unwrap();
    assert_eq!(call(&client, "tree", json!({})).await.is_error, Some(true));
    // Restore original policy, then replace the root: identity remains pinned.
    fs::write(
        &policy,
        "[mcp]\ndatabase = \"demo\"\nsource-root = \"source\"\n",
    )
    .unwrap();
    fs::rename(root.join("rows"), root.join("old-rows")).unwrap();
    fs::create_dir(root.join("rows")).unwrap();
    assert_eq!(call(&client, "tree", json!({})).await.is_error, Some(true));
    client.cancel().await.unwrap();
    // Unique external TMPDIR evidence retained; no cleanup outside Trash.
}

#[derive(Clone)]
struct LegacyClient;
impl rmcp::ClientHandler for LegacyClient {
    fn get_info(&self) -> ClientConfig {
        ClientConfig::new(
            ClientCapabilities::default(),
            Implementation::new("ibcmd-mcp-test", "1"),
        )
        .with_protocol_version(ProtocolVersion::V_2025_11_25)
    }
}
#[tokio::test]
async fn legacy_initialize_and_modern_discover_are_supported() {
    use rmcp::service::{ClientLifecycleMode, ClientServiceExt};
    let (root, settings, policy) = corpus();
    let combined = root.join("combined.toml");
    fs::write(
        &combined,
        format!(
            "{}\n{}",
            fs::read_to_string(&settings).unwrap(),
            fs::read_to_string(&policy).unwrap()
        ),
    )
    .unwrap();
    for modern in [false, true] {
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
        command.arg("mcp").arg("--settings");
        if modern {
            command.arg(&combined);
        } else {
            command.arg(&settings).arg("--policy").arg(&policy);
        }
        command.stderr(Stdio::piped());
        let mode = if modern {
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            }
        } else {
            ClientLifecycleMode::Initialize
        };
        let client = LegacyClient
            .serve_with_lifecycle(TokioChildProcess::new(command).unwrap(), mode)
            .await
            .unwrap();
        let info = client.peer_info().unwrap();
        assert_eq!(info.server_info.as_ref().unwrap().name, "ibcmd-rs");
        assert_eq!(
            info.protocol_version,
            if modern {
                ProtocolVersion::V_2026_07_28
            } else {
                ProtocolVersion::V_2025_11_25
            }
        );
        assert!(info.capabilities.tools.is_some());
        assert_eq!(client.list_all_tools().await.unwrap().len(), 6);
        let unknown = client
            .call_tool(CallToolRequestParams::new("unknown_tool"))
            .await
            .unwrap_err();
        assert!(
            matches!(unknown, rmcp::service::ServiceError::McpError(error) if error.code == ErrorCode::INVALID_PARAMS)
        );
        // Send a genuine MCP cancellation notification through the SDK handle.
        let request = ClientRequest::CallToolRequest(CallToolRequest::new(
            CallToolRequestParams::new("export").with_arguments(
                json!({"objects":["Configuration"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        ));
        let pending = client
            .send_cancellable_request(request, rmcp::service::PeerRequestOptions::no_options())
            .await
            .unwrap();
        pending
            .cancel(Some("integration cancellation".into()))
            .await
            .unwrap();
        assert_eq!(client.list_all_tools().await.unwrap().len(), 6);
        client.cancel().await.unwrap();
    }
}

#[tokio::test]
async fn real_mcp_client_reads_disposable_filedb_without_mutation() {
    use std::io::Read;
    let root = std::env::temp_dir().join(format!("ibcmd-mcp-filedb-{}", uuid::Uuid::new_v4()));
    let _scratch = crate::common::Scratch::adopt(&root);
    let db = root.join("db");
    fs::create_dir_all(&db).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture.join("tiny.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    let target = db.join("1Cv8.1CD");
    fs::write(&target, &bytes).unwrap();
    let settings = root.join("ibcmd-rs.toml");
    fs::write(&settings, format!("[[database]]\nname='demo'\nplatform='8.5.1'\ndb-path='db'\nagent-write=false\n[mcp]\ndatabase='demo'\nsource-root='{}'\n",fixture.join("tiny-xml").display())).unwrap();
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command
        .arg("mcp")
        .arg("--settings")
        .arg(&settings)
        .stderr(Stdio::piped());
    let client = ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap();
    assert!(
        !value(&call(&client, "tree", json!({})).await)["nodes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let module = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";
    let result = value(&call(&client, "read", json!({"path":module})).await);
    let source = fs::read(fixture.join("tiny-xml").join(module)).unwrap();
    assert_eq!(
        result["file"]["content"].as_str().unwrap().as_bytes(),
        source
    );
    let exported = value(
        &call(
            &client,
            "export",
            json!({"objects":["CommonModule.ФайловаяБазаПроверка"]}),
        )
        .await,
    );
    assert_eq!(
        exported["objects"][0]["files"][module]["content"],
        result["file"]["content"]
    );
    let comparison = value(
        &call(
            &client,
            "compare",
            json!({"objects":["CommonModule.ФайловаяБазаПроверка"]}),
        )
        .await,
    );
    assert_eq!(comparison["objects"][0]["status"], "same");
    assert_eq!(comparison["objects"][0]["localOnly"], json!([]));
    assert_eq!(
        call(
            &client,
            "import",
            json!({"objects":["CommonModule.ФайловаяБазаПроверка"]})
        )
        .await
        .is_error,
        Some(true)
    );
    assert_eq!(fs::read(&target).unwrap(), bytes);
    assert!(!ibcmd_filedb::backup_file(&db).exists());
    // Optional native oracle is only in this external test orchestrator, never
    // the production Rust adapter. All native state remains under this test root.
    if let Some(native) = std::env::var_os("IBCMD_MCP_NATIVE") {
        let native_out = root.join("native-export");
        let output = std::process::Command::new(native)
            .arg("infobase")
            .arg(format!("--db-path={}", db.display()))
            .arg(format!("--data={}", root.join("native-data").display()))
            .arg(format!("--temp={}", root.join("native-temp").display()))
            .args(["config", "export"])
            .arg(&native_out)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "native export: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(native_out.join(module)).unwrap(), source);
    }
    // Replacement under the same path cannot silently reconnect this session.
    fs::rename(&target, root.join("old.1CD")).unwrap();
    fs::write(&target, &bytes).unwrap();
    assert_eq!(call(&client, "tree", json!({})).await.is_error, Some(true));
    client.cancel().await.unwrap();
}

fn writable_corpus(opt_in: Option<bool>) -> (crate::common::Scratch, PathBuf, PathBuf) {
    use std::io::Read;
    let root = crate::common::Scratch::adopt(
        std::env::temp_dir().join(format!("ibcmd-mcp-write-{}", uuid::Uuid::new_v4())),
    );
    let db = root.join("db");
    let source = root.join("source");
    fs::create_dir_all(&db).unwrap();
    fs::create_dir_all(&source).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture.join("tiny.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(db.join("1Cv8.1CD"), bytes).unwrap();
    for entry in walkdir::WalkDir::new(fixture.join("tiny-xml"))
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
    {
        let relative = entry.path().strip_prefix(fixture.join("tiny-xml")).unwrap();
        let path = source.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(entry.path(), path).unwrap();
    }
    let settings = root.join("ibcmd-rs.toml");
    let opt_in = opt_in
        .map(|v| format!("agent-write={v}\n"))
        .unwrap_or_default();
    fs::write(&settings,format!("[[database]]\nname='demo'\nplatform='8.5.1'\ndb-path='db'\n{opt_in}[mcp]\ndatabase='demo'\nsource-root='source'\n")).unwrap();
    (root, db, settings)
}
async fn start_client(
    settings: &std::path::Path,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
    command
        .arg("mcp")
        .arg("--settings")
        .arg(settings)
        .stderr(Stdio::piped());
    ().serve(TokioChildProcess::new(command).unwrap())
        .await
        .unwrap()
}
const WRITE_OBJECT: &str = "CommonModule.ФайловаяБазаПроверка";
const WRITE_MODULE: &str = "CommonModules/ФайловаяБазаПроверка/Ext/Module.bsl";

#[tokio::test]
async fn real_mcp_import_requires_fixed_explicit_opt_in_and_rechecks_policy() {
    for opt_in in [None, Some(false), Some(true)] {
        let (root, db, settings) = writable_corpus(opt_in);
        let target = db.join("1Cv8.1CD");
        let before = fs::read(&target).unwrap();
        let client = start_client(&settings).await;
        // An agent cannot set opt-in or redirect to another source, database,
        // server or settings through otherwise valid import arguments.
        for key in [
            "agent-write",
            "agentWrite",
            "source-root",
            "db-path",
            "database",
            "server",
            "settings",
            "confirm",
        ] {
            let mut args = json!({"objects":[WRITE_OBJECT]});
            args[key] = json!(true);
            assert_eq!(call(&client, "import", args).await.is_error, Some(true));
        }
        if opt_in == Some(true) {
            // Changes outside the model revoke an already established session.
            let original = fs::read_to_string(&settings).unwrap();
            fs::write(
                &settings,
                original.replace("agent-write=true", "agent-write=false"),
            )
            .unwrap();
        }
        assert_eq!(
            call(&client, "import", json!({"objects":[WRITE_OBJECT]}))
                .await
                .is_error,
            Some(true)
        );
        assert_eq!(fs::read(&target).unwrap(), before);
        assert!(!ibcmd_filedb::backup_file(&db).exists());
        assert!(!root.join("db/1Cv8.1CD.ibcmd-rs.tmp").exists());
        client.cancel().await.unwrap();
    }
}

#[tokio::test]
async fn source_tools_consistently_refuse_hidden_files_directories_and_links() {
    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            if !keep_fixtures() {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }
    fn keep_fixtures() -> bool {
        std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").is_some_and(|value| value == "1")
    }
    for kind in ["file", "directory", "symlink"] {
        let (root, db, settings) = writable_corpus(Some(true));
        let _scratch = Scratch(root.to_path_buf());
        let source = root.join("source");
        let module = source.join(WRITE_MODULE);
        fs::write(
            &module,
            fs::read_to_string(&module).unwrap() + "\r\n// hidden-entry admission candidate\r\n",
        )
        .unwrap();
        match kind {
            "file" => fs::write(source.join(".hidden"), "synthetic hidden data").unwrap(),
            "directory" => {
                fs::create_dir(source.join(".git")).unwrap();
                fs::write(source.join(".git/config"), "synthetic repository").unwrap();
            }
            "symlink" => {
                let sentinel = root.join("outside-sentinel");
                fs::write(&sentinel, "synthetic outside data").unwrap();
                std::os::unix::fs::symlink(&sentinel, source.join(".hidden-link")).unwrap();
            }
            _ => unreachable!(),
        }
        let before = fs::read(db.join("1Cv8.1CD")).unwrap();
        let client = start_client(&settings).await;
        for tool in ["status", "compare", "import"] {
            assert_eq!(
                call(&client, tool, json!({"objects":[WRITE_OBJECT]}))
                    .await
                    .is_error,
                Some(true),
                "{tool} accepted hidden {kind}"
            );
        }
        assert_eq!(fs::read(db.join("1Cv8.1CD")).unwrap(), before);
        for suffix in ["bak", "tmp", "bak.new"] {
            assert!(!db.join(format!("1Cv8.1CD.ibcmd-rs.{suffix}")).exists());
        }
        client.cancel().await.unwrap();
        drop(_scratch);
        assert_eq!(root.exists(), keep_fixtures());
    }
}

#[tokio::test]
async fn real_mcp_client_stages_module_and_native_orchestrator_applies() {
    let (root, db, settings) = writable_corpus(Some(true));
    let target = db.join("1Cv8.1CD");
    let before = fs::read(&target).unwrap();
    let source = root.join("source");
    let module_path = source.join(WRITE_MODULE);
    let expected =
        fs::read_to_string(&module_path).unwrap() + "\r\n// MCP issue386 staged module edit\r\n";
    fs::write(&module_path, &expected).unwrap();
    let client = start_client(&settings).await;
    let old = value(&call(&client, "read", json!({"path":WRITE_MODULE})).await);
    assert_ne!(old["file"]["content"], expected);
    let staged = value(&call(&client, "import", json!({"objects":[WRITE_OBJECT]})).await);
    assert_eq!(staged["status"], "staged");
    assert_eq!(staged["stagePublished"], true);
    assert_eq!(staged["restartRequired"], true);
    assert_eq!(staged["applied"], false);
    assert_eq!(fs::read(ibcmd_filedb::backup_file(&db)).unwrap(), before);
    assert_ne!(fs::read(&target).unwrap(), before);
    // Atomic publication replaces the inode: never silently rebind an old session.
    assert_eq!(call(&client, "tree", json!({})).await.is_error, Some(true));
    assert_eq!(
        call(&client, "import", json!({"objects":[WRITE_OBJECT]}))
            .await
            .is_error,
        Some(true)
    );
    client.cancel().await.unwrap();
    let restarted = start_client(&settings).await;
    assert_eq!(
        value(&call(&restarted, "read", json!({"path":WRITE_MODULE})).await)["file"]["content"],
        expected
    );
    restarted.cancel().await.unwrap();
    // Native commands occur ONLY in the external test orchestrator. New lab DB,
    // data/temp/output paths are under this root; no user DB or ERP fixtures.
    if let Some(native) = std::env::var_os("IBCMD_MCP_NATIVE") {
        let output = root.join("native-export");
        for (operation, destination) in [("apply", None), ("export", Some(&output))] {
            let mut command = std::process::Command::new(&native);
            command
                .arg("infobase")
                .arg(format!("--db-path={}", db.display()))
                .arg(format!("--data={}", root.join("native-data").display()))
                .arg(format!("--temp={}", root.join("native-temp").display()))
                .args(["config", operation]);
            if let Some(destination) = destination {
                command.arg(destination);
            }
            let result = command.output().unwrap();
            fs::write(
                root.join(format!("native-{operation}.log")),
                [result.stdout.as_slice(), result.stderr.as_slice()].concat(),
            )
            .unwrap();
            assert!(
                result.status.success(),
                "native {operation}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        assert_eq!(
            fs::read(output.join(WRITE_MODULE)).unwrap(),
            expected.as_bytes()
        );
        // Compare every source file, not only the edited module. Dump inventory
        // versions may change; the stored/native inventory is compared separately.
        for entry in walkdir::WalkDir::new(&source)
            .into_iter()
            .map(Result::unwrap)
            .filter(|e| e.file_type().is_file())
        {
            let relative = entry.path().strip_prefix(&source).unwrap();
            if relative != std::path::Path::new("ConfigDumpInfo.xml") {
                assert_eq!(
                    fs::read(entry.path()).unwrap(),
                    fs::read(output.join(relative)).unwrap(),
                    "native mismatch: {}",
                    relative.display()
                );
            }
        }
        fn inventory(
            path: &std::path::Path,
        ) -> std::collections::BTreeMap<String, (String, String)> {
            use quick_xml::{Reader, events::Event};
            let bytes = fs::read(path).unwrap();
            let mut reader = Reader::from_reader(bytes.as_slice());
            let mut result = std::collections::BTreeMap::new();
            loop {
                match reader.read_event().unwrap() {
                    Event::Start(element) | Event::Empty(element)
                        if element.name().as_ref() == b"Metadata" =>
                    {
                        let attrs = element
                            .attributes()
                            .map(|a| {
                                let a = a.unwrap();
                                (
                                    String::from_utf8(a.key.as_ref().to_vec()).unwrap(),
                                    String::from_utf8(a.value.into_owned()).unwrap(),
                                )
                            })
                            .collect::<std::collections::BTreeMap<_, _>>();
                        let version = attrs["configVersion"].clone();
                        assert_eq!(version.len(), 40);
                        assert!(version.bytes().all(|b| b.is_ascii_hexdigit()));
                        assert!(
                            result
                                .insert(attrs["name"].clone(), (attrs["id"].clone(), version))
                                .is_none()
                        );
                    }
                    Event::Eof => break,
                    _ => {}
                }
            }
            assert!(!result.is_empty());
            result
        }
        let baseline = inventory(&source.join("ConfigDumpInfo.xml"));
        let native = inventory(&output.join("ConfigDumpInfo.xml"));
        assert_eq!(
            baseline.keys().collect::<Vec<_>>(),
            native.keys().collect::<Vec<_>>()
        );
        for (name, (id, version)) in &baseline {
            assert_eq!(id, &native[name].0);
            if name == &format!("{WRITE_OBJECT}.Module") {
                assert_ne!(
                    version, &native[name].1,
                    "edited module version must change"
                );
            } else {
                assert_eq!(
                    version, &native[name].1,
                    "unselected version changed: {name}"
                );
            }
        }
        eprintln!("MCP native write acceptance evidence: {}", root.display());
    }
}

#[tokio::test]
async fn real_mcp_import_cancellation_before_publication_preserves_target() {
    let (root, db, settings) = writable_corpus(Some(true));
    let module = root.join("source").join(WRITE_MODULE);
    fs::write(
        &module,
        fs::read_to_string(&module).unwrap() + "\r\n// cancellable import\r\n",
    )
    .unwrap();
    let target = db.join("1Cv8.1CD");
    let before = fs::read(&target).unwrap();
    let client = start_client(&settings).await;
    let request = ClientRequest::CallToolRequest(CallToolRequest::new(
        CallToolRequestParams::new("import").with_arguments(
            json!({"objects":[WRITE_OBJECT]})
                .as_object()
                .unwrap()
                .clone(),
        ),
    ));
    let pending = client
        .send_cancellable_request(request, rmcp::service::PeerRequestOptions::no_options())
        .await
        .unwrap();
    pending
        .cancel(Some("cancel before publication".into()))
        .await
        .unwrap();
    // Wait for the cancelled worker to leave bounded admission. A queued second
    // mutation is never used as a probe, and cancellation is a real SDK message.
    let mut idle = false;
    for _ in 0..100 {
        let result = call(&client, "tree", json!({})).await;
        if result.is_error == Some(false) {
            idle = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(idle, "cancelled worker did not return to idle");
    assert_eq!(fs::read(target).unwrap(), before);
    assert!(!ibcmd_filedb::backup_file(&db).exists());
    client.cancel().await.unwrap();
}

#[path = "common/mcp_export_fixture.rs"]
mod export_fixture;
#[tokio::test]
async fn real_mcp_export_rejects_incomplete_maps_without_returning_partial_files() {
    use export_fixture::{Case, Scenario};
    for rows_dir in [false, true] {
        for scenario in [
            Scenario::InvalidDescriptor,
            Scenario::OpaqueForm,
            Scenario::NestedInvalid,
            Scenario::LegacyNestedInvalid,
            Scenario::LegacyCommand,
            Scenario::CommonForm,
            Scenario::LegacyNestedValid,
        ] {
            let case = Case::new(scenario, rows_dir);
            let before = case.input();
            let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
            command
                .arg("mcp")
                .arg("--settings")
                .arg(&case.settings)
                .stderr(Stdio::piped());
            let client = ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap();
            let response = call(&client, "export", json!({"objects":[case.object]})).await;
            if matches!(
                scenario,
                Scenario::InvalidDescriptor
                    | Scenario::OpaqueForm
                    | Scenario::NestedInvalid
                    | Scenario::LegacyNestedInvalid
            ) {
                assert_eq!(
                    response.is_error,
                    Some(true),
                    "{scenario:?}, rows_dir={rows_dir}: {response:?}"
                );
                for content in &response.content {
                    let text = &content.as_text().unwrap().text;
                    assert!(!text.contains("\"files\""));
                    assert!(!text.contains("KeepMe"));
                }
            } else {
                let payload = value(&response);
                let files = payload["objects"][0]["files"].as_object().unwrap();
                assert!(!files.is_empty());
                if matches!(scenario, Scenario::CommonForm) {
                    assert!(files.contains_key("CommonForms/OpaqueForm/Ext/Form.xml"));
                }
                if matches!(scenario, Scenario::LegacyNestedValid) {
                    let path = "Catalogs/CorpusList/Forms/ListForm.xml";
                    let read = value(&call(&client, "read", json!({"path":path})).await);
                    assert_eq!(read["file"]["content"], files[path]["content"]);
                }
            }
            if matches!(scenario, Scenario::InvalidDescriptor | Scenario::OpaqueForm) {
                let good =
                    value(&call(&client, "export", json!({"objects":["Language.English"]})).await);
                assert_eq!(
                    good["objects"][0]["files"]["Languages/English.xml"]["content"],
                    fs::read_to_string(
                        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                            .join("tests/fixtures/file-infobase/tiny-xml/Languages/English.xml")
                    )
                    .unwrap()
                );
            }
            assert_eq!(
                case.input(),
                before,
                "MCP export must not modify DB, rows, source or sentinels"
            );
            client.cancel().await.unwrap();
        }
    }
}

#[tokio::test]
async fn real_mcp_read_refuses_only_the_requested_invalid_descriptor() {
    use export_fixture::{Case, Scenario};
    for (rows_dir, scenario) in [
        (false, Scenario::NestedInvalid),
        (true, Scenario::NestedInvalid),
        (false, Scenario::LegacyNestedInvalid),
        (true, Scenario::LegacyNestedInvalid),
    ] {
        let case = Case::new(scenario, rows_dir);
        let before = case.input();
        let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"));
        command
            .arg("mcp")
            .arg("--settings")
            .arg(&case.settings)
            .stderr(Stdio::piped());
        let client = ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap();
        for _ in 0..2 {
            let refused = call(
                &client,
                "read",
                json!({"path":"Catalogs/CorpusList/Forms/ListForm.xml"}),
            )
            .await;
            assert_eq!(refused.is_error, Some(true), "{refused:?}");
            for content in &refused.content {
                let text = &content.as_text().unwrap().text;
                assert!(!text.contains("<IncludeHelpInContents>false"));
                assert!(!text.contains("\"version\""));
            }
            for path in [
                "Catalogs/CorpusList.xml",
                "Catalogs/CorpusList/Forms/ListForm/Ext/Form/Module.bsl",
                "Catalogs/CorpusList/Forms/ListForm/Ext/Form.xml",
            ] {
                let good = value(&call(&client, "read", json!({"path":path})).await);
                assert!(good["file"].is_object(), "{path}: {good}");
            }
            let missing = value(
                &call(
                    &client,
                    "read",
                    json!({"path":"Catalogs/CorpusList/Forms/ListForm/Ext/nonexistent.txt"}),
                )
                .await,
            );
            assert!(missing["file"].is_null());
        }
        assert_eq!(case.input(), before);
        client.cancel().await.unwrap();
    }
}
