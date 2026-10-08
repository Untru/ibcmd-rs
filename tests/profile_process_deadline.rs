//! Real original-process boundary controls for the actual private production
//! profile/LIVE owners. Every unknown scenario has a fresh process. No native
//! platform, SQL, PID lookup, signals, waits, joins or recursive cleanup.

#[path = "../src/profile_process.rs"]
mod profile_process;
#[path = "../src/rac_process.rs"]
mod rac_process;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

static ORIGINAL_HOLDERS: Mutex<Vec<Child>> = Mutex::new(Vec::new());
static ORIGINAL_CALLERS: Mutex<Vec<JoinHandle<()>>> = Mutex::new(Vec::new());

const SCENARIOS: &[&str] = &[
    "production-wrapper",
    "empty",
    "stderr",
    "nonzero",
    "lossy",
    "per-stream-bound",
    "stdout-overflow",
    "stderr-overflow",
    "both-overflow",
    "overflow-inherited",
    "timeout",
    "inherited-stdout",
    "inherited-stderr",
    "shared-deadline",
    "profile-to-live",
    "live-to-profile",
    "live-overflow-to-profile",
    "spawn-failure",
    "unrepresentable-deadline",
    "active-profile-to-live",
];

fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    match arguments.get(1).map(String::as_str) {
        Some("helper") => helper(&arguments[2..]),
        Some("scenario") => scenario(&arguments[2], Path::new(&arguments[3])),
        None => {
            let directory = std::env::temp_dir().join(format!(
                "ibcmd-profile-deadline-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir(&directory).unwrap();
            for name in SCENARIOS {
                let marker = directory.join(name);
                let mut command = Command::new(std::env::current_exe().unwrap());
                command.arg("scenario").arg(name).arg(&marker);
                // SAME accepted collector owns the outer scenario original and
                // BOTH streams; output()/wait()/join() must not hide a hang.
                assert!(
                    rac_process::bounded_bytes(&mut command, Duration::from_secs(15))
                        .unwrap_or_else(|error| panic!("scenario {name}: {error:#}"))
                        .is_empty()
                );
                assert_eq!(std::fs::read(marker).unwrap(), b"PASS");
            }
            println!(
                "{} actual profile/LIVE original-process scenarios PASS",
                SCENARIOS.len()
            );
            // Keep evidence and let finite original holders expire naturally.
        }
        _ => panic!("unrecognized test role"),
    }
}

fn command(role: &str, marker: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.arg("helper").arg(role).arg(marker);
    command
}

fn profile(
    role: &str,
    marker: &Path,
    timeout: Duration,
) -> anyhow::Result<profile_process::BoundedOutput> {
    profile_process::bounded_output_with_timeout(&mut command(role, marker), timeout)
}

fn known_empty(marker: &Path) {
    let next = profile("empty", marker, Duration::from_secs(5)).unwrap();
    assert!(next.status.success());
    assert_eq!(next.stdout, "\r\n \t");
    assert!(next.stderr.is_empty());
}

fn refuse_second_launch(marker: &Path) {
    // Cross-domain attempts use the SAME production sticky registry. Neither
    // can hide unknown behind a fresh wrapper or produce a second actual child.
    let profile_marker = marker.with_extension("profile-must-not-launch");
    let live_marker = marker.with_extension("live-must-not-launch");
    let error = profile("launch-marker", &profile_marker, Duration::from_secs(5))
        .err()
        .unwrap();
    assert!(format!("{error:#}").contains("retained unproved original process"));
    let error = rac_process::bounded_bytes(
        &mut command("launch-marker", &live_marker),
        Duration::from_secs(5),
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("retained unproved original process")
    );
    assert!(!profile_marker.exists() && !live_marker.exists());
}

fn scenario(name: &str, marker: &Path) {
    let began = Instant::now();
    let timeout = if name == "shared-deadline" {
        Duration::from_secs(3)
    } else {
        Duration::from_secs(2)
    };
    match name {
        "production-wrapper" => {
            let output = profile_process::bounded_output(&mut command("empty", marker)).unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, "\r\n \t");
            assert!(output.stderr.is_empty());
        }
        "empty" => known_empty(marker),
        "stderr" => {
            let output = profile("stderr", marker, Duration::from_secs(5)).unwrap();
            assert!(output.status.success());
            assert!(output.stdout.is_empty());
            assert_eq!(output.stderr, "owned diagnostic");
            // LIVE retains its different, actual success/empty-stderr policy.
            let error =
                rac_process::bounded_bytes(&mut command("stderr", marker), Duration::from_secs(5))
                    .unwrap_err();
            assert!(error.to_string().contains("RAS session readiness failed"));
            known_empty(marker);
        }
        "nonzero" => {
            let output = profile("nonzero", marker, Duration::from_secs(5)).unwrap();
            assert_eq!(output.status.code(), Some(7));
            assert_eq!(output.stdout, "owned stdout");
            assert_eq!(output.stderr, "owned diagnostic");
            // The existing profile caller still rejects this known status;
            // complete lifetime does not create sticky-unknown custody.
            known_empty(marker);
        }
        "lossy" => {
            let output = profile("lossy", marker, Duration::from_secs(5)).unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, "\u{fffd}output");
            assert_eq!(output.stderr, "\u{fffd}diagnostic");
            let raw = rac_process::bounded_bytes(
                &mut command("raw-lossy", marker),
                Duration::from_secs(5),
            )
            .unwrap();
            assert_eq!(raw, b"\xffoutput");
            // LIVE's strict UTF-8 conversion remains in its unchanged caller;
            // this control proves the neutral collector never normalizes bytes.
            assert!(String::from_utf8(raw).is_err());
        }
        "per-stream-bound" => {
            let output = profile(name, marker, Duration::from_secs(5)).unwrap();
            assert!(output.status.success());
            assert_eq!(
                output.stdout.as_bytes(),
                vec![b'o'; profile_process::MAX_PROBE_OUTPUT_BYTES].as_slice()
            );
            assert_eq!(
                output.stderr.as_bytes(),
                vec![b'e'; profile_process::MAX_PROBE_OUTPUT_BYTES].as_slice()
            );
        }
        "stdout-overflow" | "stderr-overflow" | "both-overflow" => {
            let error = profile(name, marker, Duration::from_secs(5)).err().unwrap();
            assert_eq!(error.to_string(), "probe output exceeded 65536 bytes");
            assert_eq!(
                std::fs::read(marker.with_extension("exited")).unwrap(),
                b"direct-exit"
            );
            // Overflow was drained through BOTH EOFs: known failure allows a
            // subsequent real original, unlike immediate LIVE overflow.
            known_empty(marker);
        }
        "overflow-inherited" | "timeout" | "inherited-stdout" | "inherited-stderr"
        | "shared-deadline" | "profile-to-live" => {
            let role = if name == "profile-to-live" {
                "inherited-stdout"
            } else {
                name
            };
            let error = profile(role, marker, timeout).err().unwrap();
            let message = format!("{error:#}");
            assert!(message.contains("completion is unproved"), "{message}");
            assert!(message.contains("cycle 2 was not started"), "{message}");
            assert!(began.elapsed() < timeout + Duration::from_secs(1));
            if role != "timeout" {
                assert_eq!(
                    std::fs::read(marker.with_extension("exited")).unwrap(),
                    b"direct-exit"
                );
                assert!(message.contains("completion unproved"), "{message}");
            }
            refuse_second_launch(marker);
        }
        "live-to-profile" | "live-overflow-to-profile" => {
            let role = if name == "live-to-profile" {
                "inherited-stderr"
            } else {
                "live-overflow"
            };
            let error =
                rac_process::bounded_bytes(&mut command(role, marker), timeout).unwrap_err();
            let message = format!("{error:#}");
            assert!(message.contains("cycle 2 was not started"), "{message}");
            if name == "live-overflow-to-profile" {
                assert!(message.contains("rac output exceeded bound"), "{message}");
            } else {
                assert!(began.elapsed() < timeout + Duration::from_secs(1));
                assert_eq!(
                    std::fs::read(marker.with_extension("exited")).unwrap(),
                    b"direct-exit"
                );
            }
            refuse_second_launch(marker);
        }
        "spawn-failure" => {
            let missing = marker.with_extension("nonexistent-executable");
            assert!(!missing.exists());
            assert!(
                profile_process::bounded_output_with_timeout(&mut Command::new(missing), timeout)
                    .is_err()
            );
            known_empty(marker);
        }
        "unrepresentable-deadline" => {
            let never = marker.with_extension("must-not-launch");
            assert!(
                profile_process::bounded_output_with_timeout(
                    &mut command("launch-marker", &never),
                    Duration::MAX
                )
                .is_err()
            );
            assert!(!never.exists());
            known_empty(marker);
        }
        "active-profile-to-live" => {
            let deadline = Instant::now() + Duration::from_secs(5);
            let original_marker = marker.to_path_buf();
            let (send, receive) = mpsc::channel();
            let caller = std::thread::spawn(move || {
                let result = profile("delayed-empty", &original_marker, Duration::from_secs(5));
                send.send(result).unwrap();
            });
            ORIGINAL_CALLERS.lock().unwrap().push(caller);
            while !marker.with_extension("started").exists() {
                assert!(Instant::now() < deadline, "original helper did not start");
                std::thread::sleep(Duration::from_millis(10));
            }
            let never = marker.with_extension("active-must-not-launch");
            let error = rac_process::bounded_bytes(
                &mut command("launch-marker", &never),
                Duration::from_secs(5),
            )
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("already has an active original process")
            );
            assert!(!never.exists());
            // Bounded SAME caller result, never a join. Once BOTH and direct
            // exit are actually known, a new original may run normally.
            let output = receive
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout, "\r\n \t");
            assert!(output.stderr.is_empty());
            known_empty(marker);
        }
        _ => panic!("unrecognized scenario"),
    }
    std::fs::write(marker, b"PASS").unwrap();
}

fn hold_stream(marker: &Path, stdout: bool, milliseconds: u64) {
    let mut holder = command("holder", marker);
    holder.arg(milliseconds.to_string()).stdin(Stdio::null());
    holder.stdout(if stdout {
        Stdio::inherit()
    } else {
        Stdio::null()
    });
    holder.stderr(if stdout {
        Stdio::null()
    } else {
        Stdio::inherit()
    });
    ORIGINAL_HOLDERS
        .lock()
        .unwrap()
        .push(holder.spawn().unwrap());
}

fn helper(arguments: &[String]) -> ! {
    let role = arguments[0].as_str();
    let marker = PathBuf::from(&arguments[1]);
    let bound = profile_process::MAX_PROBE_OUTPUT_BYTES;
    match role {
        "zero" => {}
        "empty" | "delayed-empty" => {
            if role == "delayed-empty" {
                std::fs::write(marker.with_extension("started"), b"original-started").unwrap();
                std::thread::sleep(Duration::from_secs(2));
            }
            std::io::stdout().write_all(b"\r\n \t").unwrap();
        }
        "stderr" => std::io::stderr().write_all(b"owned diagnostic").unwrap(),
        "nonzero" => {
            std::io::stdout().write_all(b"owned stdout").unwrap();
            std::io::stderr().write_all(b"owned diagnostic").unwrap();
            std::io::stdout().flush().unwrap();
            std::io::stderr().flush().unwrap();
            std::process::exit(7);
        }
        "lossy" | "raw-lossy" => {
            std::io::stdout().write_all(b"\xffoutput").unwrap();
            if role == "lossy" {
                std::io::stderr().write_all(b"\xffdiagnostic").unwrap();
            }
        }
        "per-stream-bound" => {
            std::io::stdout().write_all(&vec![b'o'; bound]).unwrap();
            std::io::stderr().write_all(&vec![b'e'; bound]).unwrap();
        }
        "stdout-overflow" | "stderr-overflow" | "both-overflow" => {
            if role != "stderr-overflow" {
                std::io::stdout().write_all(&vec![b'o'; bound + 1]).unwrap();
            }
            if role != "stdout-overflow" {
                std::io::stderr().write_all(&vec![b'e'; bound + 1]).unwrap();
            }
        }
        "live-overflow" => std::io::stdout()
            .write_all(&vec![b'x'; 1024 * 1024 + 1])
            .unwrap(),
        "timeout" => std::thread::sleep(Duration::from_secs(4)),
        "holder" => std::thread::sleep(Duration::from_millis(arguments[2].parse().unwrap())),
        "inherited-stdout" | "inherited-stderr" | "overflow-inherited" => {
            let stdout = role != "inherited-stderr";
            if role == "overflow-inherited" {
                std::io::stdout().write_all(&vec![b'o'; bound + 1]).unwrap();
            }
            hold_stream(&marker, stdout, 4000);
        }
        "shared-deadline" => {
            hold_stream(&marker, true, 2600);
            hold_stream(&marker, false, 4000);
            std::thread::sleep(Duration::from_secs(2));
        }
        "launch-marker" => std::fs::write(&marker, b"unexpected launch").unwrap(),
        _ => panic!("unrecognized helper role"),
    }
    std::io::stdout().flush().unwrap();
    std::io::stderr().flush().unwrap();
    std::fs::write(marker.with_extension("exited"), b"direct-exit").unwrap();
    std::process::exit(0)
}
