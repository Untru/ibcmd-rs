//! Consumer regressions; the one real CIM control is explicitly ROOT-selected.
use super::*;

fn fixture() -> (ShutdownCensus, ProcessIdentity, BTreeSet<u32>) {
    let row = CensusIdentity {
        pid: 800,
        parent: 700,
        birth_filetime: 100,
        executable: std::env::temp_dir().join("selected-tools/pwsh.exe"),
        command: "pwsh.exe -NoProfile -NonInteractive -Command root-marker".into(),
    };
    let bound = ProcessIdentity {
        pid: row.pid,
        parent: row.parent,
        birth_100ns: 106,
        executable: row.executable.clone(),
        command_sha256: format!("{:X}", Sha256::digest(row.command.as_bytes())),
    };
    let owned = CensusIdentity {
        pid: 10,
        parent: 1,
        birth_filetime: 80,
        executable: std::env::temp_dir().join("platform/ragent.exe"),
        command: "ragent.exe -d root-marker".into(),
    };
    (
        ShutdownCensus {
            observer: CensusObserverRow { identity: row },
            owned: vec![owned],
            foreign: vec![],
            listeners: vec![Listener {
                port: 7540,
                pid: 10,
            }],
        },
        bound,
        BTreeSet::from([10]),
    )
}

fn admitted(census: &ShutdownCensus, bound: &ProcessIdentity, seeds: &BTreeSet<u32>) -> bool {
    require_census_observer(census, seeds, bound).is_ok()
        && require_shutdown_census(census, seeds).is_ok()
}

#[test]
fn census_observer_separate_original_never_becomes_seed_descendant_or_listener() {
    let (census, bound, seeds) = fixture();
    assert!(admitted(&census, &bound, &seeds));
    assert_eq!(census.owned.len(), 1);
    for fault in [
        "seed",
        "child",
        "parent",
        "duplicate",
        "listener",
        "foreign-child",
    ] {
        let (mut changed, bound, mut seeds) = fixture();
        match fault {
            "seed" => {
                seeds.insert(changed.observer.identity.pid);
            }
            "child" => changed.observer.identity.parent = 10,
            "parent" => changed.owned[0].parent = changed.observer.identity.pid,
            "duplicate" => changed.owned.push(changed.observer.identity.clone()),
            "listener" => changed.listeners.push(Listener {
                port: 7545,
                pid: bound.pid,
            }),
            "foreign-child" => changed.foreign.push(ForeignProcessFact {
                pid: 900,
                parent: bound.pid,
                birth_filetime: None,
                executable_present: false,
                executable_path_sha256: None,
                command_present: false,
                command_sha256: None,
                private_root_marker: None,
            }),
            _ => unreachable!(),
        }
        assert!(!admitted(&changed, &bound, &seeds), "{fault}");
    }
}

#[test]
fn census_observer_full_identity_replacement_and_interval_drift_refuse() {
    for fault in [
        "pid", "parent", "birth", "overflow", "image", "command", "empty", "relative",
    ] {
        let (mut census, bound, seeds) = fixture();
        let row = &mut census.observer.identity;
        match fault {
            "pid" => row.pid += 1,
            "parent" => row.parent += 1,
            "birth" => row.birth_filetime += 10,
            "overflow" => row.birth_filetime = u64::MAX,
            "image" => row.executable = std::env::temp_dir().join("replacement/pwsh.exe"),
            "command" => row.command.push_str(" replacement"),
            "empty" => row.command.clear(),
            "relative" => row.executable = PathBuf::from("pwsh.exe"),
            _ => unreachable!(),
        }
        assert!(!admitted(&census, &bound, &seeds), "{fault}");
    }
    let (census, mut bound, seeds) = fixture();
    bound.birth_100ns = 109;
    assert!(admitted(&census, &bound, &seeds));
    bound.birth_100ns = 110;
    assert!(!admitted(&census, &bound, &seeds));
}

#[test]
fn census_observer_exception_never_hides_unrelated_pwsh_root_or_opaque_foreign() {
    let (mut census, bound, seeds) = fixture();
    let mut unrelated = census.observer.identity.clone();
    unrelated.pid += 1; // same image, parent and root-marker command are insufficient
    census.owned.push(unrelated);
    assert!(!admitted(&census, &bound, &seeds));
    census.owned.pop();
    census.foreign.push(ForeignProcessFact {
        pid: 900,
        parent: 901,
        birth_filetime: None,
        executable_present: false,
        executable_path_sha256: None,
        command_present: false,
        command_sha256: None,
        private_root_marker: None,
    });
    assert!(admitted(&census, &bound, &seeds));
    census.foreign[0].command_present = true;
    census.foreign[0].command_sha256 = Some("A".repeat(64));
    census.foreign[0].private_root_marker = Some(true);
    assert!(!admitted(&census, &bound, &seeds));
    census.foreign[0].private_root_marker = Some(false);
    census.foreign[0].pid = bound.pid;
    assert!(!admitted(&census, &bound, &seeds));
}

#[test]
fn census_observer_missing_null_extra_or_duplicate_response_is_not_a_binding() {
    let (census, bound, _) = fixture();
    let row = census.observer.identity;
    let valid = serde_json::json!({"observer":{"identity":{
        "pid":row.pid,"parent":row.parent,"birth_filetime":row.birth_filetime,
        "executable":row.executable,"command":row.command}},
        "owned":[],"foreign":[],"listeners":[]});
    let decoded: ShutdownCensus = serde_json::from_value(valid.clone()).unwrap();
    assert!(admitted(&decoded, &bound, &BTreeSet::new()));
    for fault in [
        "missing",
        "null",
        "identity-null",
        "extra",
        "identity-extra",
    ] {
        let mut changed = valid.clone();
        match fault {
            "missing" => {
                changed.as_object_mut().unwrap().remove("observer");
            }
            "null" => changed["observer"] = serde_json::Value::Null,
            "identity-null" => changed["observer"]["identity"] = serde_json::Value::Null,
            "extra" => changed["observer"]["grant"] = serde_json::Value::Bool(true),
            "identity-extra" => {
                changed["observer"]["identity"]["grant"] = serde_json::Value::Bool(true)
            }
            _ => unreachable!(),
        }
        assert!(
            serde_json::from_value::<ShutdownCensus>(changed).is_err(),
            "{fault}"
        );
    }
    let duplicate = format!(
        "{{\"observer\":{},\"observer\":{},\"owned\":[],\"foreign\":[],\"listeners\":[]}}",
        valid["observer"], valid["observer"]
    );
    assert!(serde_json::from_str::<ShutdownCensus>(&duplicate).is_err());
}

#[cfg(windows)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadonlyTool {
    path: PathBuf,
    bytes: u64,
    sha256: String,
}

#[cfg(windows)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadonlyRequest {
    protocol: String,
    powershell: ReadonlyTool,
    root_marker: PathBuf,
    seconds: u64,
}

// Test caller custody only: this receipt is produced by command::run's policy
// AFTER completed_at proved the original direct exit/BOTH. It is deliberately
// separate from census/authority binding, which a negative test may invalidate.
struct ReadonlyTerminal {
    original_index: usize,
    exit: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

struct ReadonlyCustody<C> {
    collectors: Vec<C>,
    failed: bool,
    dispatch_started: bool,
    terminal: Option<ReadonlyTerminal>,
    original_error: Option<anyhow::Error>,
    original_panic: Option<Box<dyn std::any::Any + Send>>,
}

impl<C> ReadonlyCustody<C> {
    fn new() -> Self {
        Self {
            collectors: Vec::new(),
            failed: false,
            dispatch_started: false,
            terminal: None,
            original_error: None,
            original_panic: None,
        }
    }

    fn physical_terminal_proved(&self) -> bool {
        self.terminal
            .as_ref()
            .is_some_and(|receipt| receipt.original_index == 0 && self.collectors.len() == 1)
    }
}

enum ReadonlyOutcome<C> {
    Settled(ReadonlyCustody<C>),
    Retained(ReadonlyCustody<C>),
}

fn protect_readonly<C>(
    mut custody: ReadonlyCustody<C>,
    operation: impl FnOnce(&mut ReadonlyCustody<C>) -> Result<()>,
) -> ReadonlyOutcome<C> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(&mut custody)));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => custody.original_error = Some(error),
        Err(panic) => custody.original_panic = Some(panic),
    }
    // A returned launch refusal with no original Child is known before effect.
    // A panic during a pending launch cannot make that same absence claim.
    let pending_panic = custody.dispatch_started
        && custody.collectors.is_empty()
        && custody.original_panic.is_some();
    if pending_panic || (!custody.collectors.is_empty() && !custody.physical_terminal_proved()) {
        ReadonlyOutcome::Retained(custody)
    } else {
        ReadonlyOutcome::Settled(custody)
    }
}

#[cfg(windows)]
fn finish_readonly(outcome: ReadonlyOutcome<OriginalChild>) -> Result<()> {
    match outcome {
        ReadonlyOutcome::Retained(custody) => {
            // No unwind/drop, retry, wait, signal, adoption or replacement budget.
            // SAME collectors/readers, original error and panic stay resident.
            let _same_original_owner = custody;
            loop {
                std::thread::park();
            }
        }
        ReadonlyOutcome::Settled(mut custody) => {
            if let Some(panic) = custody.original_panic.take() {
                std::panic::resume_unwind(panic);
            }
            if let Some(error) = custody.original_error.take() {
                return Err(error);
            }
            Ok(())
        }
    }
}

// These probes exercise the SAME protected caller used below; they simulate
// only ownership, not Win32 handles, original process completion or grants.
struct CustodyDropProbe(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl Drop for CustodyDropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[derive(Debug)]
struct CustodyError(std::sync::Arc<()>);

impl std::fmt::Display for CustodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("readonly custody fault probe")
    }
}

impl std::error::Error for CustodyError {}

#[test]
fn census_observer_custody_unknown_error_and_panic_keep_same_owner() {
    for panic_after_dispatch in [false, true] {
        let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let marker = std::sync::Arc::new(());
        let outcome = protect_readonly(ReadonlyCustody::new(), |custody| {
            custody.dispatch_started = true;
            custody.collectors.push(CustodyDropProbe(drops.clone()));
            custody.failed = true; // actual command timeout/reader fault analogue
            if panic_after_dispatch {
                std::panic::panic_any(marker.clone());
            }
            Err(anyhow::Error::new(CustodyError(marker.clone())))
        });
        let ReadonlyOutcome::Retained(custody) = outcome else {
            panic!("UNKNOWN caller discarded original owner");
        };
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(std::sync::Arc::ptr_eq(&custody.collectors[0].0, &drops));
        if panic_after_dispatch {
            let payload = custody.original_panic.as_ref().unwrap();
            assert!(std::sync::Arc::ptr_eq(
                payload.downcast_ref::<std::sync::Arc<()>>().unwrap(),
                &marker,
            ));
        } else {
            let error = custody.original_error.as_ref().unwrap();
            assert!(std::sync::Arc::ptr_eq(
                &error.downcast_ref::<CustodyError>().unwrap().0,
                &marker,
            ));
        }
        assert!(custody.terminal.is_none());
        // A finite pure probe ends here; the actual retained branch parks.
        drop(custody);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}

#[test]
fn census_observer_custody_partial_completion_never_becomes_terminal() {
    for faulty_receipt in [false, true] {
        let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let outcome = protect_readonly(ReadonlyCustody::new(), |custody| {
            custody.dispatch_started = true;
            custody.collectors.push(CustodyDropProbe(drops.clone()));
            if faulty_receipt {
                custody.terminal = Some(ReadonlyTerminal {
                    original_index: 1,
                    exit: 0,
                    stdout: vec![],
                    stderr: vec![],
                });
            }
            Ok(()) // even a caller success cannot release unknown original custody
        });
        let ReadonlyOutcome::Retained(custody) = outcome else {
            panic!("partial original output admitted");
        };
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(!custody.physical_terminal_proved());
    }
}

#[test]
fn census_observer_custody_known_terminal_policy_refusal_is_not_resident() {
    for policy_outcome in [0u8, 1, 2] {
        let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let outcome = protect_readonly(ReadonlyCustody::new(), |custody| {
            custody.dispatch_started = true;
            custody.collectors.push(CustodyDropProbe(drops.clone()));
            custody.terminal = Some(ReadonlyTerminal {
                original_index: 0,
                exit: 0,
                stdout: b"same complete census".to_vec(),
                stderr: vec![],
            });
            custody.failed = policy_outcome != 0; // later policy cannot undo physical EOF
            if policy_outcome == 2 {
                std::panic::panic_any("post-terminal policy panic");
            }
            if policy_outcome == 1 {
                bail!("post-terminal forged parent refusal");
            }
            Ok(())
        });
        let ReadonlyOutcome::Settled(custody) = outcome else {
            panic!("known physical terminal falsely parked");
        };
        let receipt = custody.terminal.as_ref().unwrap();
        assert_eq!(receipt.exit, 0);
        assert_eq!(receipt.stdout, b"same complete census");
        assert!(receipt.stderr.is_empty());
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(custody.original_error.is_some(), policy_outcome == 1);
        assert_eq!(custody.original_panic.is_some(), policy_outcome == 2);
        drop(custody);
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}

#[test]
fn census_observer_custody_pre_effect_refusal_and_pending_panic_differ() {
    for dispatch_started in [false, true] {
        let outcome = protect_readonly::<CustodyDropProbe>(ReadonlyCustody::new(), |custody| {
            custody.dispatch_started = dispatch_started;
            bail!("known before-effect validation or launch refusal")
        });
        assert!(matches!(outcome, ReadonlyOutcome::Settled(_)));
    }
    let outcome = protect_readonly::<CustodyDropProbe>(ReadonlyCustody::new(), |custody| {
        custody.dispatch_started = true;
        std::panic::panic_any("pending launch panic without original returned");
    });
    let ReadonlyOutcome::Retained(custody) = outcome else {
        panic!("pending panic called known absence");
    };
    assert!(custody.collectors.is_empty() && custody.original_panic.is_some());
}

/// Actual readonly Win32/CIM/listener census, not an always-positive mock.
/// No private registry, server/SQL, mutation, kill or ownership authority.
/// ROOT supplies and pins current tool+request, and runs this ignored test once.
#[cfg(windows)]
#[test]
#[ignore = "ROOT-selected actual readonly census; requires exact pinned request/tool"]
fn root_selected_readonly_census_observer_original_handle_and_both() -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(120);
    let request_path = std::env::var_os("IBCMD_CENSUS_OBSERVER_ROOT_REQUEST")
        .context("ROOT readonly request required")?;
    let expected = std::env::var("IBCMD_CENSUS_OBSERVER_ROOT_REQUEST_SHA256")
        .context("ROOT readonly request SHA required")?;
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
    {
        bail!("exact readonly request digest required");
    }
    super::super::child::require_deadline(deadline)?;
    let request_path = PathBuf::from(request_path);
    if fs::metadata(&request_path)?.len() > 16384 {
        bail!("readonly request initial bound");
    }
    let request_file = Tool::pin(request_path)?;
    if request_file.digest != expected {
        bail!("bounded exact ROOT readonly request required");
    }
    let bytes = fs::read(&request_file.path)?;
    if bytes.len() > 16384 {
        bail!("readonly request opened bound");
    }
    let request: ReadonlyRequest = serde_json::from_slice(&bytes)?;
    request_file.check()?;
    let leaf = request
        .root_marker
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let nonce = leaf
        .strip_prefix("census-observer-")
        .context("unique readonly root marker required")?;
    let parsed = Uuid::parse_str(nonce)?;
    if request.protocol != "native-auth409/census-readonly-request-v1"
        || request.seconds != 120
        || parsed.is_nil()
        || parsed.to_string() != nonce
        || !request.root_marker.is_absolute()
        || !request
            .root_marker
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("f:\\")
        || request.root_marker.to_string_lossy().len() > 4096
    {
        bail!("ROOT readonly closed policy/unused marker required");
    }
    let tool = Tool::pin(request.powershell.path)?;
    if tool.digest != request.powershell.sha256
        || fs::metadata(&tool.path)?.len() != request.powershell.bytes
    {
        bail!("ROOT readonly PowerShell bytes changed");
    }
    let root = request.root_marker.to_string_lossy().replace('\'', "''");
    let argv = vec![
        "-NoProfile".into(),
        "-NonInteractive".into(),
        "-Command".into(),
        shutdown_census_script(&root, "", "0"),
    ];
    let outcome = protect_readonly(ReadonlyCustody::new(), |custody| {
        let original_index = custody.collectors.len();
        custody.dispatch_started = true;
        let raw = super::super::command::run(
            &mut custody.failed,
            &mut custody.collectors,
            &tool.path,
            &argv,
            deadline,
            || tool.check(),
            |output| {
                custody.terminal = Some(ReadonlyTerminal {
                    original_index,
                    exit: output.0,
                    stdout: output.1.clone(),
                    stderr: output.2.clone(),
                });
                super::super::command::strict_text(output)
            },
        )?;
        let census: ShutdownCensus = serde_json::from_str(&raw)?;
        assert_eq!(custody.collectors.len(), original_index + 1);
        let original = &mut custody.collectors[original_index];
        let bound = original.bind_completed_census(&census.observer.identity, raw.as_bytes())?;
        require_census_observer(&census, &BTreeSet::new(), &bound)?;
        require_shutdown_census(&census, &BTreeSet::new())?;
        assert!(census.owned.is_empty() && census.listeners.is_empty());
        assert_eq!(bound.pid, original.pid());
        assert!(original.terminal_proved() && original.direct_exit_proved());
        request_file.check()?;
        tool.check()?;
        original.require_command_current()?;
        // Physical completion was latched before this deliberate sticky refusal.
        let mut replaced = census.observer.identity.clone();
        replaced.parent = replaced.parent.wrapping_add(1);
        assert!(
            original
                .bind_completed_census(&replaced, raw.as_bytes())
                .is_err()
        );
        assert!(!original.terminal_proved()); // no second dispatch or late acceptance
        super::super::child::require_deadline(deadline)?;
        Ok(())
    });
    finish_readonly(outcome)
}
