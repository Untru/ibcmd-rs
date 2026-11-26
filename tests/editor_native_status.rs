//! Additive native-version RPC contract (#380), using only committed tiny data.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};
struct Rpc {
    child: Child,
    out: BufReader<std::process::ChildStdout>,
    id: u32,
}
impl Rpc {
    fn new(dir: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["serve", "--stdio"])
            .current_dir(dir)
            .env_remove("IBCMD_DB_PSW")
            .env("IBCMD_RS_CONFIG", dir.join("ibcmd-rs.toml"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let out = BufReader::new(child.stdout.take().unwrap());
        Self { child, out, id: 0 }
    }
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let body = serde_json::to_vec(
            &json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params}),
        )
        .unwrap();
        let input = self.child.stdin.as_mut().unwrap();
        write!(input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        input.write_all(&body).unwrap();
        input.flush().unwrap();
        loop {
            let mut size = None;
            loop {
                let mut line = String::new();
                assert!(self.out.read_line(&mut line).unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
                if let Some(n) = line.strip_prefix("Content-Length: ") {
                    size = Some(n.trim().parse::<usize>().unwrap());
                }
            }
            let mut bytes = vec![0; size.unwrap()];
            self.out.read_exact(&mut bytes).unwrap();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            if value["id"] == self.id {
                return value;
            }
        }
    }
}
impl Drop for Rpc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/file-infobase")
        .join(name)
}
#[test]
fn rpc_native_evidence_is_additive_identity_bound_and_never_clean_on_full_or_bad_reference() {
    let root = std::env::temp_dir().join(format!("issue380-native-rpc-{}", uuid::Uuid::new_v4()));
    let _scratch = crate::common::Scratch::adopt(&root);
    fs::create_dir_all(root.join("db")).unwrap();
    fs::create_dir(root.join("baseline")).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(fixture("tiny.1CD.gz")).unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    fs::write(root.join("db/1Cv8.1CD"), &bytes).unwrap();
    fs::write(
        root.join("ibcmd-rs.toml"),
        "[[database]]\nname='native'\nplatform='8.5.1'\ndb-path='db'\n",
    )
    .unwrap();
    let reference = fs::read_to_string(fixture("tiny-xml/ConfigDumpInfo.xml")).unwrap();
    let changed_reference = reference
        .replace("a86111168911c14b90b4fc704b63ebe600000000", &"0".repeat(40))
        .replace("446fd6627b646645af71333b00c13f6f00000000", &"1".repeat(40));
    fs::write(root.join("baseline/ConfigDumpInfo.xml"), &changed_reference).unwrap();
    let mut rpc = Rpc::new(&root);
    let init = rpc.call(
        "initialize",
        json!({"protocolVersion":"1.1","workspaceFolder":root}),
    );
    assert!(init["error"].is_null(), "{init}");
    let params = json!({"infobase":"native","sourceDir":fixture("tiny-xml")});
    let legacy = rpc.call("objects/status", params.clone());
    assert!(legacy["error"].is_null(), "{legacy}");
    assert!(
        legacy["result"]["objects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["databaseVersionStatus"].is_null())
    );
    let active_tree = rpc.call("tree/children", json!({"infobase":"native"}));
    let main_tree = rpc.call(
        "tree/children",
        json!({"infobase":"native", "mainConfiguration":true}),
    );
    assert!(main_tree["error"].is_null(), "{main_tree}");
    assert_eq!(active_tree["result"], main_tree["result"]);
    let mut params = params;
    params["versionBaselineDir"] = json!(root.join("baseline"));
    let result = rpc.call("objects/status", params.clone());
    assert!(result["error"].is_null(), "{result}");
    let result = &result["result"];
    assert_eq!(result["fullRequired"], false);
    assert_eq!(result["nativeOwners"]["unmappedIds"], json!([]));
    assert!(
        result["nativeOwners"]["byId"]
            .as_object()
            .unwrap()
            .values()
            .any(
                |owner| owner["object"] == "CommonModule.ФайловаяБазаПроверка"
                    && owner["root"] == "CommonModule.ФайловаяБазаПроверка"
            )
    );
    let digest = format!("{:x}", Sha256::digest(changed_reference.as_bytes()));
    for row in result["objects"].as_array().unwrap() {
        assert_eq!(row["status"], "same");
        let e = &row["databaseVersionStatus"];
        assert_eq!(e["conclusive"], true, "{row}");
        assert_eq!(e["baselineIdentity"], digest);
        assert_eq!(e["objectVersion"], row["version"]);
        assert_eq!(e["databaseSnapshot"], result["databaseSnapshot"]);
        assert_eq!(e["databaseSnapshot"].as_str().unwrap().len(), 64);
        assert_eq!(e["object"], row["object"]);
        assert_eq!(
            e["changed"],
            row["object"] == "Configuration" || row["object"] == "Language.English"
        );
    }
    fs::write(
        root.join("baseline/ConfigDumpInfo.xml"),
        reference.replace(
            "CommonModule.ФайловаяБазаПроверка\"",
            "CommonModule.OldName\"",
        ),
    )
    .unwrap();
    let full = rpc.call("objects/status", params.clone());
    assert_eq!(full["result"]["fullRequired"], true, "{full}");
    assert!(
        full["result"]["objects"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["databaseVersionStatus"]["conclusive"] == false)
    );
    fs::write(root.join("baseline/ConfigDumpInfo.xml"), "bad XML").unwrap();
    assert!(!rpc.call("objects/status", params)["error"].is_null());
    assert_eq!(
        fs::read(root.join("db/1Cv8.1CD")).unwrap(),
        bytes,
        "RPC READ must not mutate filedb"
    );
}
