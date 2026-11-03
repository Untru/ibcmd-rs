//! Retained public directory fixtures; no platform/compiler/database needed.
use super::{RootAnchor, SourceChangeError};
use std::{fs, path::PathBuf, process::Command};

// No Drop cleanup. Replacements park old directories in the same owned root.
struct Fixture {
    root: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("ibcmd-directory-witness-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        fs::create_dir(&source).unwrap();
        fs::create_dir(root.join("parking")).unwrap();
        Self {
            root: fs::canonicalize(root).unwrap(),
            source: fs::canonicalize(source).unwrap(),
        }
    }
    fn anchor(&self) -> std::sync::Arc<RootAnchor> {
        RootAnchor::open(&self.source).unwrap()
    }
    fn child_output(&self, test: &str, output: std::process::Output) {
        fs::write(self.root.join("stdout.txt"), &output.stdout).unwrap();
        fs::write(self.root.join("stderr.txt"), &output.stderr).unwrap();
        fs::write(
            self.root.join("child-result.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "test": test,
                "exit": output.status.code(),
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "isolated {test} failed; logs retained"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("running 1 test"));
        assert!(stdout.contains("1 passed; 0 failed; 0 ignored;"));
    }
}

#[test]
fn unchanged_directory_and_ordinary_child_edits_keep_witness() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.source.join("parent/member")).unwrap();
    let anchor = fixture.anchor();
    let before = anchor.directory_witness("parent/member").unwrap();
    assert_eq!(before, anchor.directory_witness("parent/member").unwrap());
    fs::write(fixture.source.join("parent/member/new.txt"), b"before").unwrap();
    fs::write(fixture.source.join("parent/member/new.txt"), b"after").unwrap();
    fs::rename(
        fixture.source.join("parent/member/new.txt"),
        fixture.source.join("parent/member/renamed.txt"),
    )
    .unwrap();
    assert_eq!(before, anchor.directory_witness("parent/member").unwrap());
    // A detached witness must not retain a member/ancestor rename restriction.
    fs::rename(
        fixture.source.join("parent"),
        fixture.source.join("renamed-parent"),
    )
    .unwrap();
}

#[test]
fn same_path_new_directory_identity_changes_witness() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.source.join("member")).unwrap();
    let anchor = fixture.anchor();
    let before = anchor.directory_witness("member").unwrap();
    fs::rename(
        fixture.source.join("member"),
        fixture.root.join("parking/member"),
    )
    .unwrap();
    fs::create_dir(fixture.source.join("member")).unwrap();
    let after = anchor.directory_witness("member").unwrap();
    assert_ne!(before.directory, after.directory);
    assert_ne!(before, after);
}

#[test]
fn changed_ancestor_with_unchanged_leaf_changes_witness() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.source.join("ancestor/member")).unwrap();
    let anchor = fixture.anchor();
    let before = anchor.directory_witness("ancestor/member").unwrap();
    fs::rename(
        fixture.source.join("ancestor"),
        fixture.root.join("parking/ancestor"),
    )
    .unwrap();
    fs::create_dir(fixture.source.join("ancestor")).unwrap();
    fs::rename(
        fixture.root.join("parking/ancestor/member"),
        fixture.source.join("ancestor/member"),
    )
    .unwrap();
    let after = anchor.directory_witness("ancestor/member").unwrap();
    assert_eq!(before.root, after.root);
    assert_eq!(
        before.directory, after.directory,
        "leaf must really be unchanged"
    );
    assert_ne!(before, after, "ancestry was omitted from detached evidence");
}

#[test]
fn witness_is_bound_to_root_and_exact_enumerated_spelling() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.source.join("ancestor/member")).unwrap();
    let anchor = fixture.anchor();
    let before = anchor.directory_witness("ancestor/member").unwrap();
    {
        let nested = RootAnchor::open(&fixture.source.join("ancestor")).unwrap();
        let different_root = nested.directory_witness("member").unwrap();
        assert_eq!(before.directory, different_root.directory);
        assert_ne!(before.root, different_root.root);
        assert_ne!(before, different_root);
    }
    fs::rename(
        fixture.source.join("ancestor"),
        fixture.source.join("ANCESTOR"),
    )
    .unwrap();
    assert!(
        fs::read_dir(&fixture.source)
            .unwrap()
            .any(|entry| { entry.unwrap().file_name() == "ANCESTOR" })
    );
    let different_spelling = anchor.directory_witness("ANCESTOR/member").unwrap();
    assert_eq!(before.root, different_spelling.root);
    assert_eq!(before.directory, different_spelling.directory);
    assert_ne!(before, different_spelling, "exact spelling was omitted");
}

#[test]
fn noncanonical_paths_and_regular_file_members_refuse() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.source.join("parent/member")).unwrap();
    fs::write(fixture.source.join("regular"), b"untouched public content").unwrap();
    let anchor = fixture.anchor();
    for path in [
        "",
        ".",
        "..",
        "../outside",
        "parent/../member",
        "parent/./member",
        "parent//member",
        "parent/member/",
        r"parent\member",
        "/absolute",
        "C:relative",
        "parent/CON",
        "parent/member.",
        "parent/member ",
    ] {
        assert!(anchor.directory_witness(path).is_err(), "accepted {path:?}");
    }
    assert!(anchor.directory_witness("regular").is_err());
    assert!(anchor.directory_witness("regular/member").is_err());
    assert_eq!(
        fs::read(fixture.source.join("regular")).unwrap(),
        b"untouched public content"
    );
}

#[cfg(unix)]
#[test]
fn symlink_at_any_directory_component_refuses() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.root.join("outside/member")).unwrap();
    fs::write(
        fixture.root.join("outside/public.txt"),
        b"outside remains unchanged",
    )
    .unwrap();
    symlink(
        fixture.root.join("outside"),
        fixture.source.join("link-parent"),
    )
    .unwrap();
    symlink(
        fixture.root.join("outside/member"),
        fixture.source.join("link-leaf"),
    )
    .unwrap();
    let anchor = fixture.anchor();
    assert!(anchor.directory_witness("link-parent/member").is_err());
    assert!(anchor.directory_witness("link-leaf").is_err());
    assert_eq!(
        fs::read(fixture.root.join("outside/public.txt")).unwrap(),
        b"outside remains unchanged"
    );
}

#[cfg(windows)]
#[test]
fn junction_at_any_directory_component_refuses() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.root.join("outside/member")).unwrap();
    fs::write(
        fixture.root.join("outside/public.txt"),
        b"outside remains unchanged",
    )
    .unwrap();
    for (name, target) in [("link-parent", "outside"), ("link-leaf", "outside/member")] {
        let output = Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(fixture.source.join(name))
            .arg(fixture.root.join(target))
            .output()
            .unwrap();
        fs::write(
            fixture.root.join(format!("{name}-stdout.txt")),
            &output.stdout,
        )
        .unwrap();
        fs::write(
            fixture.root.join(format!("{name}-stderr.txt")),
            &output.stderr,
        )
        .unwrap();
        assert!(output.status.success(), "owned junction setup failed");
    }
    let anchor = fixture.anchor();
    assert!(anchor.directory_witness("link-parent/member").is_err());
    assert!(anchor.directory_witness("link-leaf").is_err());
    assert_eq!(
        fs::read(fixture.root.join("outside/public.txt")).unwrap(),
        b"outside remains unchanged"
    );
}

#[cfg(unix)]
#[test]
fn rebound_root_name_refuses_directory_witness() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.source.join("member")).unwrap();
    let anchor = fixture.anchor();
    anchor.directory_witness("member").unwrap();
    fs::rename(
        &fixture.source,
        fixture.root.join("parking/original-source"),
    )
    .unwrap();
    fs::create_dir_all(fixture.source.join("member")).unwrap();
    assert!(matches!(
        anchor.directory_witness("member"),
        Err(SourceChangeError::HeldRootChanged)
    ));
}

#[test]
fn directory_witness_cancellation_remains_typed() {
    const TEST: &str = "mssql_source_change::files::directory_witness_tests::directory_witness_cancellation_remains_typed";
    const CHILD: &str = "IBCMD_DIRECTORY_WITNESS_CANCEL_CHILD";
    match std::env::var(CHILD) {
        Ok(value) => {
            assert_eq!(value, "1");
            let fixture = Fixture::new();
            fs::create_dir(fixture.source.join("member")).unwrap();
            let anchor = fixture.anchor();
            let flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
            let _scope = crate::cancel::scope(flag);
            assert!(matches!(
                anchor.directory_witness("member"),
                Err(SourceChangeError::Cancelled(_))
            ));
        }
        Err(std::env::VarError::NotPresent) => {
            let fixture = Fixture::new();
            let output = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture", "--test-threads=1"])
                .env(CHILD, "1")
                .env("IBCMD_TEST_KEEP_FIXTURES", "1")
                .output()
                .unwrap();
            fixture.child_output(TEST, output);
        }
        Err(error) => panic!("invalid cancellation child sentinel: {error}"),
    }
}

#[cfg(unix)]
#[test]
fn repeated_directory_witnesses_release_transient_descriptors_under_low_fd_limit() {
    const TEST: &str = "mssql_source_change::files::directory_witness_tests::repeated_directory_witnesses_release_transient_descriptors_under_low_fd_limit";
    const CHILD: &str = "IBCMD_DIRECTORY_WITNESS_LOW_FD_CHILD";
    match std::env::var(CHILD) {
        Ok(value) => {
            assert_eq!(value, "1");
            let fixture = Fixture::new();
            let relative = (0..24)
                .map(|n| format!("d{n}"))
                .collect::<Vec<_>>()
                .join("/");
            fs::create_dir_all(fixture.source.join(&relative)).unwrap();
            let anchor = fixture.anchor();
            let before = anchor.directory_witness(&relative).unwrap();
            for _ in 0..256 {
                assert_eq!(before, anchor.directory_witness(&relative).unwrap());
            }
            fs::rename(
                fixture.source.join(&relative),
                fixture.root.join("parking/member"),
            )
            .unwrap();
        }
        Err(std::env::VarError::NotPresent) => {
            let fixture = Fixture::new();
            let output = Command::new("sh")
                .args([
                    "-c",
                    "ulimit -S -n 128 && exec \"$1\" --exact \"$2\" --nocapture --test-threads=1",
                    "directory-witness-low-fd",
                ])
                .arg(std::env::current_exe().unwrap())
                .arg(TEST)
                .env(CHILD, "1")
                .env("IBCMD_TEST_KEEP_FIXTURES", "1")
                .output()
                .unwrap();
            fixture.child_output(TEST, output);
        }
        Err(error) => panic!("invalid low-FD child sentinel: {error}"),
    }
}
