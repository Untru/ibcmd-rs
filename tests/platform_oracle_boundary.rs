#![cfg(not(feature = "platform-oracle"))]

use std::{fs, path::PathBuf, process::Command};

/// Commands that locate or run an installed platform: never in a release.
/// `infobase` is released: its `config export|import` read and write SQL
/// Server directly and every other native command is refused, never run.
const ORACLE_COMMANDS: &[&str] = &["probe", "profile-run", "dump-sources"];
/// `infobase config` research commands that run the platform's own ibcmd.
const ORACLE_INFOBASE_COMMANDS: &[&str] = &["roundtrip", "sweep"];
const FORBIDDEN_BINARY_MARKERS: &[&[u8]] = &[
    b"ibcmd.exe",
    b"1cv8.exe",
    b"1cv8c.exe",
    b"\\1cv8\\",
    b"/1cv8/",
    b".jar",
    b"org.eclipse",
    b"JNI_CreateJavaVM",
    b"JNIEnv",
    b"JavaVM",
    b"OSGi",
];
const EDT_SOURCE_IDS: &[&[u8]] = &[
    b"org.eclipse.xtext.ui.shared.xtextBuilder",
    b"org.eclipse.xtext.ui.shared.xtextNature",
    b".settings/org.eclipse.core.resources.prefs",
];

fn exclude_declarative_edt_ids(bytes: &mut [u8]) {
    for identifier in EDT_SOURCE_IDS {
        let positions: Vec<_> = bytes
            .windows(identifier.len())
            .enumerate()
            .filter_map(|(index, window)| {
                (window == *identifier
                    && (identifier.starts_with(b".settings/")
                        || !matches!(
                            bytes.get(index + identifier.len()),
                            Some(b'.' | b'/' | b'$')
                        )))
                .then_some(index)
            })
            .collect();
        for index in positions {
            bytes[index..index + identifier.len()].fill(0);
        }
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        .env("PATH", "")
        .output()
        .unwrap()
}

#[test]
fn default_cli_has_no_platform_oracle_commands() {
    let output = run(&["--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();

    for command in ORACLE_COMMANDS {
        assert!(
            !help.lines().any(|line| {
                line.trim_start()
                    .strip_prefix(command)
                    .is_some_and(|tail| tail.starts_with(char::is_whitespace))
            }),
            "default CLI unexpectedly exposes `{command}`:\n{help}"
        );
    }
    assert!(help.contains("convert"));
    assert!(help.contains("cf"));
    assert!(help.contains("compatibility"));
    assert!(help.contains("infobase"));
}

#[test]
fn released_infobase_mode_refuses_rather_than_runs_the_platform() {
    let output = run(&["infobase", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("export"));
    assert!(help.contains("import"));
    for command in ORACLE_INFOBASE_COMMANDS {
        assert!(!help.contains(command), "help names `{command}`:\n{help}");
        // not a command of the release: an incomplete `config`, exit 2 as
        // the platform gives for a command it does not know
        let output = run(&["infobase", "config", command, "--db-name=x"]);
        assert_eq!(output.status.code(), Some(2), "{command}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("Указана неполная команда"), "{stdout}");
    }
    // What only the platform does is refused by name, with no PATH to find
    // it on and nothing launched. `config apply` itself is served by
    // ibcmd-rs (without the platform) since 0.4, and its dynamic update since 0.5 (#347); its
    // extensions are not.
    for args in [
        &[
            "infobase",
            "create",
            "--dbms=MSSQLServer",
            "--db-server=localhost",
            "--db-name=ibcmd_rs_boundary",
        ][..],
        &[
            "infobase",
            "config",
            "apply",
            "--dbms=MSSQLServer",
            "--db-server=localhost",
            "--db-name=ibcmd_rs_boundary",
            "--extension=E",
        ][..],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("не поддерживается в этой версии ibcmd-rs"),
            "{args:?}: {stderr}"
        );
    }
}

#[test]
fn default_binary_has_no_known_platform_or_edt_payload_markers() {
    let executable = std::env::var_os("CARGO_BIN_EXE_ibcmd-rs")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ibcmd-rs")));
    let mut bytes = fs::read(&executable).unwrap();
    // Exact source-format IDs are legitimate data; runtime/Java markers remain
    // forbidden. This does not allow Eclipse package names generally.
    exclude_declarative_edt_ids(&mut bytes);

    for marker in FORBIDDEN_BINARY_MARKERS {
        if let Some(offset) = bytes
            .windows(marker.len())
            .position(|window| window == *marker)
        {
            let start = offset.saturating_sub(32);
            let end = (offset + marker.len() + 32).min(bytes.len());
            panic!(
                "default binary {} contains forbidden marker `{}` at offset {offset:#x}; surrounding bytes: {:02x?}",
                executable.display(),
                String::from_utf8_lossy(marker),
                &bytes[start..end]
            );
        }
    }
}

#[test]
fn declarative_project_ids_do_not_allow_runtime_eclipse_payloads() {
    let mut bytes = b"org.eclipse.xtext.ui.shared.xtextBuilder org.eclipse.equinox.launcher org/eclipse/Foo.class JNI_CreateJavaVM".to_vec();
    exclude_declarative_edt_ids(&mut bytes);
    assert!(
        !bytes
            .windows(EDT_SOURCE_IDS[0].len())
            .any(|part| part == EDT_SOURCE_IDS[0])
    );
    for marker in [b"org.eclipse".as_slice(), b"JNI_CreateJavaVM"] {
        assert!(bytes.windows(marker.len()).any(|part| part == marker));
    }
    for identifier in EDT_SOURCE_IDS {
        for suffix in [b".evil.Launcher".as_slice(), b"/Runtime", b"$Runtime"] {
            if identifier.starts_with(b".settings/") {
                continue;
            }
            let mut extended = [*identifier, suffix].concat();
            exclude_declarative_edt_ids(&mut extended);
            assert!(
                extended
                    .windows(b"org.eclipse".len())
                    .any(|part| part == b"org.eclipse")
            );
        }
    }
    let mut preferences = b".settings/org.eclipse.core.resources.prefs.ibcmd-provenance/ org.eclipse.core.resources.prefs.evil.Launcher".to_vec();
    exclude_declarative_edt_ids(&mut preferences);
    assert!(
        preferences
            .windows(b"org.eclipse.core.resources.prefs.evil".len())
            .any(|part| part == b"org.eclipse.core.resources.prefs.evil")
    );
}
