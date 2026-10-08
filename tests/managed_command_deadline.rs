//! Finite OS controls call the same private command owner as NativeRuntime.
//! They cannot create native Ready/Observation/worker capabilities.
#![allow(dead_code)] // Shared private child also contains unrelated shutdown owners.

#[path = "../src/mssql_managed_worker/identity.rs"]
mod identity;
use identity::ProcessIdentity;
#[path = "../src/mssql_managed_worker/child.rs"]
mod child;
#[path = "../src/mssql_managed_worker/command.rs"]
mod managed_command;
#[path = "../src/rac_process.rs"]
mod rac_process;

use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static ORIGINAL_HOLDERS: Mutex<Vec<Child>> = Mutex::new(Vec::new());
const SCENARIOS: &[&str] = &[
    "empty",
    "nonzero-receipt",
    "nonzero-text",
    "stderr-text",
    "strict-utf8",
    "valid-utf8",
    "stdout-overflow",
    "stderr-overflow",
    "timeout",
    "inherited-stdout",
    "inherited-stderr",
    "expired-before-start",
    "late-tool-check",
    "tool-check-refusal",
    "late-policy",
    "known-policy-refusal",
    "late-queued-terminal",
    "startup-deadline",
    "deadline-overflow",
];

fn main() {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("helper") => helper(&args[2], Path::new(&args[3])),
        Some("scenario") => scenario(&args[2], Path::new(&args[3])),
        None => {
            if !cfg!(windows) {
                println!("managed original-handle controls require Windows; no runtime claim");
                return;
            }
            let directory =
                std::env::temp_dir().join(format!("managed-deadline-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&directory).unwrap();
            for name in SCENARIOS {
                let marker = directory.join(name);
                let mut outer = Command::new(std::env::current_exe().unwrap());
                outer.arg("scenario").arg(name).arg(&marker);
                // Actual accepted original/BOTH outer collector; no output/wait/join.
                assert!(
                    rac_process::bounded_bytes(&mut outer, Duration::from_secs(12))
                        .unwrap_or_else(|error| panic!("{name}: {error:#}"))
                        .is_empty()
                );
                assert_eq!(
                    std::fs::read(marker.with_extension("result")).unwrap(),
                    b"PASS"
                );
            }
            println!("{} actual managed command scenarios PASS", SCENARIOS.len());
        }
        _ => panic!("unknown test role"),
    }
}

fn scenario(name: &str, marker: &Path) {
    let executable = std::env::current_exe().unwrap();
    let mut failed = false;
    let mut originals = Vec::new();
    let arguments = |role: &str, path: &Path| {
        vec![
            "helper".into(),
            role.into(),
            path.to_string_lossy().into_owned(),
        ]
    };
    let unknown = matches!(
        name,
        "stdout-overflow"
            | "stderr-overflow"
            | "timeout"
            | "inherited-stdout"
            | "inherited-stderr"
            | "late-policy"
    );
    match name {
        "deadline-overflow" => assert!(managed_command::deadline(None, Duration::MAX).is_err()),
        "startup-deadline" => {
            let original = Instant::now() + Duration::from_millis(40);
            assert_eq!(
                managed_command::deadline(Some(original), Duration::from_secs(120)).unwrap(),
                original
            );
            std::thread::sleep(Duration::from_millis(60));
            assert!(managed_command::deadline(Some(original), Duration::from_secs(120)).is_err());
            assert!(originals.is_empty());
        }
        "expired-before-start" | "late-tool-check" | "tool-check-refusal" => {
            let deadline = Instant::now() + Duration::from_millis(40);
            if name == "expired-before-start" {
                std::thread::sleep(Duration::from_millis(60));
            }
            let result = managed_command::run(
                &mut failed,
                &mut originals,
                &executable,
                &arguments("empty", marker),
                deadline,
                || {
                    if name == "late-tool-check" {
                        std::thread::sleep(Duration::from_millis(60));
                    }
                    if name == "tool-check-refusal" {
                        anyhow::bail!("pinned tool refused");
                    }
                    Ok(())
                },
                managed_command::strict_text,
            );
            assert!(result.is_err());
            assert!(originals.is_empty());
            assert!(!marker.exists());
        }
        "late-queued-terminal" => {
            let deadline = Instant::now() + Duration::from_millis(150);
            let mut original =
                child::OriginalChild::spawn_at(&executable, &arguments("empty", marker), deadline)
                    .unwrap();
            std::thread::sleep(Duration::from_millis(200));
            assert!(original.completed_at(deadline).is_err());
            assert!(!original.terminal_proved());
            originals.push(original);
            // This isolated case exercises OriginalChild's actual queued-status
            // boundary; it does not fabricate NativeRuntime's failed flag.
            std::fs::write(marker.with_extension("result"), b"PASS").unwrap();
            return;
        }
        _ => {
            let role = match name {
                "nonzero-receipt" | "nonzero-text" => "nonzero",
                "stderr-text" => "stderr",
                "strict-utf8" => "invalid-utf8",
                "valid-utf8" => "utf8",
                "late-policy" | "known-policy-refusal" => "empty",
                other => other,
            };
            let timeout = if matches!(name, "timeout" | "inherited-stdout" | "inherited-stderr") {
                Duration::from_millis(500)
            } else {
                Duration::from_secs(3)
            };
            let deadline = managed_command::deadline(None, timeout).unwrap();
            let result = managed_command::run(
                &mut failed,
                &mut originals,
                &executable,
                &arguments(role, marker),
                deadline,
                || Ok(()),
                |output| {
                    if name == "nonzero-receipt" {
                        assert_eq!(output.0, 7);
                        assert!(output.1.is_empty() && output.2.is_empty());
                        return Ok(String::new());
                    }
                    if name == "late-policy" {
                        // Consume precisely this original deadline during policy.
                        while Instant::now() < deadline {
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                    if name == "known-policy-refusal" {
                        anyhow::bail!("known policy refusal");
                    }
                    managed_command::strict_text(output)
                },
            );
            if matches!(name, "empty" | "nonzero-receipt" | "valid-utf8") {
                assert_eq!(
                    result.unwrap(),
                    if name == "valid-utf8" {
                        "Юникод"
                    } else {
                        ""
                    }
                );
            } else {
                assert!(result.is_err());
            }
            assert_eq!(originals.len(), 1);
            assert_eq!(originals[0].terminal_proved(), !unknown);
            assert_eq!(failed, unknown);
        }
    }
    let next_marker = marker.with_extension("second");
    if unknown {
        // Late natural EOF is not a continuation grant. SAME runtime owns originals.
        if name == "inherited-stdout" {
            std::thread::sleep(Duration::from_millis(4200));
        }
        let result = managed_command::run(
            &mut failed,
            &mut originals,
            &executable,
            &arguments("empty", &next_marker),
            Instant::now() + Duration::from_secs(2),
            || Ok(()),
            managed_command::strict_text,
        );
        assert!(result.is_err());
        assert!(!next_marker.exists());
        assert_eq!(originals.len(), 1);
    } else {
        assert_eq!(
            managed_command::run(
                &mut failed,
                &mut originals,
                &executable,
                &arguments("empty", &next_marker),
                Instant::now() + Duration::from_secs(3),
                || Ok(()),
                managed_command::strict_text
            )
            .unwrap(),
            ""
        );
        assert!(originals.last().unwrap().terminal_proved());
    }
    // Keep these exact originals until this finite scenario process ends.
    std::fs::write(marker.with_extension("result"), b"PASS").unwrap();
}

fn helper(role: &str, marker: &Path) {
    std::fs::write(marker, b"START").unwrap();
    match role {
        "empty" => {}
        "nonzero" => std::process::exit(7),
        "stderr" => std::io::stderr().write_all(b"known stderr").unwrap(),
        "invalid-utf8" => std::io::stdout().write_all(&[0xff]).unwrap(),
        "utf8" => std::io::stdout().write_all("Юникод".as_bytes()).unwrap(),
        "stdout-overflow" => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 1024 * 1024 + 1]);
        }
        "stderr-overflow" => {
            let _ = std::io::stderr().write_all(&vec![b'x'; 1024 * 1024 + 1]);
        }
        "timeout" | "hold" => std::thread::sleep(Duration::from_secs(4)),
        "inherited-stdout" | "inherited-stderr" => {
            let mut holder = Command::new(std::env::current_exe().unwrap());
            holder
                .arg("helper")
                .arg("hold")
                .arg(marker.with_extension("holder"));
            holder.stdin(Stdio::null());
            holder.stdout(if role == "inherited-stdout" {
                Stdio::inherit()
            } else {
                Stdio::null()
            });
            holder.stderr(if role == "inherited-stderr" {
                Stdio::inherit()
            } else {
                Stdio::null()
            });
            ORIGINAL_HOLDERS
                .lock()
                .unwrap()
                .push(holder.spawn().unwrap());
        }
        _ => panic!("unknown helper"),
    }
}
