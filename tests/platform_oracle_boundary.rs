#![cfg(not(feature = "platform-oracle"))]

use std::{fs, path::PathBuf, process::Command};

const ORACLE_COMMANDS: &[&str] = &["infobase", "probe", "profile-run", "dump-sources"];
const FORBIDDEN_BINARY_MARKERS: &[&[u8]] = &[
    b"ibcmd.exe",
    b"1cv8.exe",
    b"1cv8c.exe",
    b"\\1cv8\\",
    b"/1cv8/",
    b".jar",
    b"org.eclipse",
    b"JNI",
    b"OSGi",
];

#[test]
fn default_cli_has_no_platform_oracle_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .arg("--help")
        .env("PATH", "")
        .output()
        .unwrap();
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
}

#[test]
fn default_binary_has_no_known_platform_or_edt_payload_markers() {
    let executable = std::env::var_os("CARGO_BIN_EXE_ibcmd-rs")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_ibcmd-rs")));
    let bytes = fs::read(&executable).unwrap();

    for marker in FORBIDDEN_BINARY_MARKERS {
        assert!(
            !bytes.windows(marker.len()).any(|window| window == *marker),
            "default binary {} contains forbidden marker `{}`",
            executable.display(),
            String::from_utf8_lossy(marker)
        );
    }
}
