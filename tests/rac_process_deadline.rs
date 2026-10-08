//! Actual OS boundary helpers; no external executable, SQL or platform needed.
//! Every unknown scenario has a fresh test process. Finite inherited-pipe
//! holders expire naturally; tests never signal or adopt a descendant by PID.

#[path = "../src/rac_process.rs"]
mod rac_process;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

static ORIGINAL_HELPER_DESCENDANT: Mutex<Option<Child>> = Mutex::new(None);

fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    match arguments.get(1).map(String::as_str) {
        Some("helper") => helper(&arguments[2..]),
        Some("scenario") => scenario(&arguments[2], Path::new(&arguments[3])),
        None => {
            let directory = std::env::temp_dir().join(format!(
                "ibcmd-rac-deadline-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir(&directory).unwrap();
            for name in [
                "zero",
                "empty",
                "nonzero",
                "stderr",
                "nonempty",
                "oversize",
                "timeout",
                "inherited-stdout",
                "inherited-stderr",
                "shared-deadline",
            ] {
                let marker = directory.join(name);
                let mut command = Command::new(std::env::current_exe().unwrap());
                command.arg("scenario").arg(name).arg(&marker);
                // The outer command itself also has bounded direct+BOTH
                // completion. Never use output()/wait()/join() in this test.
                let bytes = rac_process::bounded_bytes(&mut command, Duration::from_secs(15))
                    .unwrap_or_else(|error| panic!("scenario {name}: {error:#}"));
                assert!(bytes.is_empty());
                assert_eq!(std::fs::read(marker).unwrap(), b"PASS");
            }
            println!("10 actual original-process/BOTH scenarios PASS");
            // Evidence files stay available; no recursive deletion or signals.
        }
        _ => panic!("unrecognized test role"),
    }
}

fn command(role: &str, marker: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.arg("helper").arg(role).arg(marker);
    command
}

fn scenario(name: &str, marker: &Path) {
    let began = Instant::now();
    let timeout = if name == "shared-deadline" {
        Duration::from_secs(3)
    } else if matches!(name, "timeout" | "inherited-stdout" | "inherited-stderr") {
        Duration::from_secs(2)
    } else {
        Duration::from_secs(5)
    };
    let result = rac_process::bounded_bytes(&mut command(name, marker), timeout);
    match name {
        "zero" => assert!(result.unwrap().is_empty()),
        "empty" => assert_eq!(result.unwrap(), b"\r\n \t"),
        "nonempty" => assert_eq!(result.unwrap(), b"session : owned-test-session\r\n"),
        "nonzero" | "stderr" => {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("RAS session readiness failed")
            );
            // Direct+BOTH known failure is not confused with unknown. An actual
            // succeeding helper may run again (the session gate still refuses
            // the failed call itself).
            assert_eq!(
                rac_process::bounded_bytes(&mut command("empty", marker), Duration::from_secs(5))
                    .unwrap(),
                b"\r\n \t"
            );
        }
        "oversize" | "timeout" | "inherited-stdout" | "inherited-stderr" | "shared-deadline" => {
            let error = result.unwrap_err().to_string();
            assert!(error.contains("cycle 2 was not started"), "{error}");
            if name == "oversize" {
                assert!(error.contains("rac output exceeded bound"), "{error}");
            } else {
                assert!(began.elapsed() < timeout + Duration::from_secs(1));
                if name.starts_with("inherited-") || name == "shared-deadline" {
                    assert!(error.contains("completion unproved"), "{error}");
                    assert_eq!(
                        std::fs::read(marker.with_extension("exited")).unwrap(),
                        b"direct-exit"
                    );
                }
            }
            let launch_marker = marker.with_extension("must-not-launch");
            let second = rac_process::bounded_bytes(
                &mut command("launch-marker", &launch_marker),
                Duration::from_secs(5),
            )
            .unwrap_err();
            assert!(
                second
                    .to_string()
                    .contains("retained unproved original process")
            );
            assert!(!launch_marker.exists());
        }
        _ => unreachable!(),
    }
    std::fs::write(marker, b"PASS").unwrap();
}

fn helper(arguments: &[String]) -> ! {
    let role = arguments[0].as_str();
    let marker = PathBuf::from(&arguments[1]);
    match role {
        "zero" => {}
        "empty" => std::io::stdout().write_all(b"\r\n \t").unwrap(),
        "nonempty" => std::io::stdout()
            .write_all(b"session : owned-test-session\r\n")
            .unwrap(),
        "nonzero" => std::process::exit(7),
        "stderr" => std::io::stderr()
            .write_all(b"owned test diagnostic")
            .unwrap(),
        "oversize" => std::io::stdout()
            .write_all(&vec![b'x'; 1024 * 1024 + 1])
            .unwrap(),
        "timeout" => std::thread::sleep(Duration::from_secs(4)),
        "hold-stdout" | "hold-stderr" => std::thread::sleep(Duration::from_secs(4)),
        "inherited-stdout" | "inherited-stderr" | "shared-deadline" => {
            let hold_stdout = role != "inherited-stderr";
            let holder = if hold_stdout {
                "hold-stdout"
            } else {
                "hold-stderr"
            };
            let mut descendant = command(holder, &marker);
            descendant
                .stdin(Stdio::null())
                .stdout(if hold_stdout {
                    Stdio::inherit()
                } else {
                    Stdio::null()
                })
                .stderr(if hold_stdout {
                    Stdio::null()
                } else {
                    Stdio::inherit()
                });
            // This exact new child is intentionally finite. The direct helper
            // exits without terminating it; no external/guessed process is used.
            *ORIGINAL_HELPER_DESCENDANT.lock().unwrap() = Some(descendant.spawn().unwrap());
            if role == "shared-deadline" {
                // A fresh pipe budget after direct exit would incorrectly
                // admit this helper at roughly 4s. One 3s budget must refuse.
                std::thread::sleep(Duration::from_secs(2));
            }
            std::fs::write(marker.with_extension("exited"), b"direct-exit").unwrap();
        }
        "launch-marker" => std::fs::write(marker, b"unexpected launch").unwrap(),
        _ => panic!("unrecognized helper role"),
    }
    std::io::stdout().flush().unwrap();
    std::io::stderr().flush().unwrap();
    std::process::exit(0)
}
