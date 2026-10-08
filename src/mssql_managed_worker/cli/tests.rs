//! Consumer routing/custody tests. Effects are injected at the three lifecycle
//! leaves; production drive, public selectors, CLI and typed outcome gate run.
//! These are not native process, SQL or shutdown acceptance.
use super::*;
use crate::cli::{Cli, Commands};
use clap::Parser;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

fn argv(command: &str) -> Vec<String> {
    let mut args = vec![
        "ibcmd-rs",
        command,
        "--platform-profile",
        "platform-8.3.27.2214",
        "--database",
        "consumer_test",
        "--mode",
        "worker",
        "--allow-non-lab",
        "--managed-worker-parent",
        "F:/consumer-must-not-create",
        "--managed-platform-bin",
        "F:/must-not-run-platform",
        "--managed-powershell",
        "F:/must-not-run-pwsh.exe",
        "--managed-agent-port",
        "18040",
        "--managed-cluster-port",
        "18041",
        "--managed-ras-port",
        "18045",
        "--managed-worker-first",
        "18060",
        "--managed-worker-last",
        "18063",
        "--managed-timeout-seconds",
        "20",
        "--managed-registration-user",
        "registration_login",
        "--managed-registration-pwd-env",
        "REGISTRATION_SLOT",
        "--managed-infobase-user",
        "ib_login",
        "--managed-infobase-pwd-env",
        "INFOBASE_SLOT",
        "--sql-user",
        "client_login",
    ];
    if command == "mssql-apply-source-change" {
        args.extend([
            "--source-root",
            "F:/must-not-read-source",
            "--path",
            "CommonModules/Probe/Ext/Module.bsl",
        ]);
    }
    args.into_iter().map(str::to_owned).collect()
}
fn source() -> MssqlApplySourceChangeArgs {
    match Cli::try_parse_from(argv("mssql-apply-source-change"))
        .unwrap()
        .command
    {
        Commands::MssqlApplySourceChange(args) => args,
        _ => unreachable!(),
    }
}
fn activation() -> MssqlActivateStagedMainArgs {
    match Cli::try_parse_from(argv("mssql-activate-staged-main"))
        .unwrap()
        .command
    {
        Commands::MssqlActivateStagedMain(args) => args,
        _ => unreachable!(),
    }
}

#[test]
fn both_public_commands_select_fresh_without_old_uuids() {
    let source = source();
    let activation = activation();
    assert!(source.managed_worker.managed_worker_parent.is_some());
    assert!(activation.managed_worker.managed_worker_parent.is_some());
    assert!(source.cluster_id.is_none() && source.infobase_id.is_none());
    assert!(activation.cluster_id.is_none() && activation.infobase_id.is_none());
    assert_eq!(source.rac, Path::new("rac"));
    assert_eq!(activation.ras_endpoint, "localhost:1545");
}
#[test]
fn supplied_old_identity_or_explicit_endpoint_cannot_select_fresh() {
    for command in ["mssql-apply-source-change", "mssql-activate-staged-main"] {
        for extra in [
            vec!["--cluster-id", "11111111-1111-4111-8111-111111111111"],
            vec!["--infobase-id", "22222222-2222-4222-8222-222222222222"],
            vec!["--rac", "rac"],
            vec!["--ras-endpoint", "localhost:1545"],
            vec!["--infobase-user", "foreign"],
            vec!["--infobase-pwd", "foreign"],
        ] {
            let mut args = argv(command);
            args.extend(extra.into_iter().map(str::to_owned));
            assert!(Cli::try_parse_from(args).is_err());
        }
    }
}
#[test]
fn old_cli_route_still_requires_both_uuids() {
    for command in ["mssql-apply-source-change", "mssql-activate-staged-main"] {
        let mut args = Vec::new();
        let mut original = argv(command).into_iter();
        while let Some(arg) = original.next() {
            if arg.starts_with("--managed-") {
                original.next();
            } else {
                args.push(arg);
            }
        }
        let refusal = Cli::try_parse_from(args.clone()).unwrap_err().to_string();
        assert!(refusal.contains("--cluster-id") && refusal.contains("--infobase-id"));
        args.extend(
            ["--cluster-id", "11111111-1111-4111-8111-111111111111"]
                .into_iter()
                .map(str::to_owned),
        );
        assert!(
            Cli::try_parse_from(args.clone())
                .unwrap_err()
                .to_string()
                .contains("--infobase-id")
        );
        args.extend(
            ["--infobase-id", "22222222-2222-4222-8222-222222222222"]
                .into_iter()
                .map(str::to_owned),
        );
        assert!(Cli::try_parse_from(args).is_ok());
    }
}
#[test]
fn partial_managed_group_is_rejected_by_clap() {
    for command in ["mssql-apply-source-change", "mssql-activate-staged-main"] {
        for (flag, _) in CHILD_ARGS {
            let mut args = argv(command);
            let position = args.iter().position(|s| s == flag).unwrap();
            args.drain(position..position + 2);
            assert!(Cli::try_parse_from(args).is_err(), "{command}: {flag}");
        }
    }
}
#[test]
fn managed_parameters_without_selection_are_rejected() {
    // Keep the original ROOT 25PASS/1FAIL witness: all twelve children, no
    // parent, both legacy UUIDs. It must now fail for BOTH public commands.
    for command in ["mssql-apply-source-change", "mssql-activate-staged-main"] {
        let mut args = argv(command);
        let position = args
            .iter()
            .position(|s| s == "--managed-worker-parent")
            .unwrap();
        args.drain(position..position + 2);
        args.extend(
            [
                "--cluster-id",
                "11111111-1111-4111-8111-111111111111",
                "--infobase-id",
                "22222222-2222-4222-8222-222222222222",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        assert!(Cli::try_parse_from(args).is_err(), "{command}");
    }
}
#[test]
fn public_source_refuses_watch_before_any_creator_or_reader() {
    let mut args = source();
    args.watch = true;
    assert!(
        crate::mssql_apply::apply_source_change(&args)
            .unwrap_err()
            .to_string()
            .contains("--watch")
    );
    assert!(
        crate::mssql_apply::watch_source_changes(&args)
            .unwrap_err()
            .to_string()
            .contains("--watch")
    );
}
#[test]
fn public_commands_refuse_dry_run_without_creator_effects() {
    let mut source = source();
    source.dry_run = true;
    let mut activation = activation();
    activation.dry_run = true;
    assert!(
        crate::mssql_apply::apply_source_change(&source)
            .unwrap_err()
            .to_string()
            .contains("before creator effects")
    );
    assert!(
        crate::mssql::activate_staged_main(&activation)
            .unwrap_err()
            .to_string()
            .contains("before creator effects")
    );
}
#[test]
fn public_commands_refuse_write_without_acknowledgement() {
    let mut source = source();
    source.allow_non_lab = false;
    let mut activation = activation();
    activation.allow_non_lab = false;
    assert!(
        crate::mssql_apply::apply_source_change(&source)
            .unwrap_err()
            .to_string()
            .contains("acknowledgement")
    );
    assert!(
        crate::mssql::activate_staged_main(&activation)
            .unwrap_err()
            .to_string()
            .contains("acknowledgement")
    );
}
#[test]
fn public_commands_refuse_external_sql_or_profile_drift() {
    let mut source = source();
    source.sqlcmd = Some("must-not-run".into());
    assert!(
        crate::mssql_apply::apply_source_change(&source)
            .unwrap_err()
            .to_string()
            .contains("built-in SQL")
    );
    let mut activation = activation();
    activation.platform_profile = MssqlNativePlatformProfile::Platform8_3_27_1989;
    assert!(
        crate::mssql::activate_staged_main(&activation)
            .unwrap_err()
            .to_string()
            .contains("platform-8.3.27.2214")
    );
}
#[test]
fn public_source_refuses_extension_before_creation() {
    let mut args = source();
    args.extension = Some("foreign".into());
    assert!(
        crate::mssql_apply::apply_source_change(&args)
            .unwrap_err()
            .to_string()
            .contains("main configuration only")
    );
}
#[test]
fn public_programmatic_args_cannot_adopt_an_endpoint() {
    let mut args = activation();
    args.cluster_id = Some(uuid::Uuid::new_v4());
    assert!(
        crate::mssql::activate_staged_main(&args)
            .unwrap_err()
            .to_string()
            .contains("cannot adopt")
    );
}
#[test]
fn public_source_rejects_unsupported_cohort_before_creation() {
    for path in [
        "InformationRegisters/Unknown/Ext/RecordSetModule.bsl",
        "Constants/Unknown/Ext/ObjectModule.bsl",
        "Catalogs/Unknown/Forms/Card/Ext/Form.xml",
    ] {
        let mut args = source();
        args.source_path = path.into();
        assert!(
            crate::mssql_apply::apply_source_change(&args)
                .unwrap_err()
                .to_string()
                .contains("no measured module/form/template owner-body cohort"),
            "{path} must fail at the shared cohort check before source or creator effects"
        );
    }
}

#[test]
fn fresh_source_uses_current_measured_module_form_and_template_cohorts() {
    for path in [
        "CommonModules/Probe/Ext/Module.bsl",
        "CommonForms/Probe/Ext/Form.xml",
        "Catalogs/Probe/Ext/ObjectModule.bsl",
        "InformationRegisters/Probe/Ext/ManagerModule.bsl",
        "DataProcessors/Probe/Templates/Page/Ext/Template.xml",
        "Reports/Probe/Templates/Main/Ext/Template.xml",
        "ExchangePlans/Probe/Templates/Text/Ext/Template.txt",
    ] {
        let mut args = source();
        args.source_path = path.into();
        crate::mssql_apply::require_supported_main_source_cohort(&args)
            .unwrap_or_else(|error| panic!("current measured cohort {path}: {error}"));
    }
}

fn portable_group() -> ManagedWorkerCliArgs {
    let mut group = source().managed_worker;
    let root = std::env::temp_dir();
    group.managed_worker_parent = Some(root.join("consumer-must-not-create"));
    group.managed_platform_bin = Some(root.join("must-not-run-platform"));
    group.managed_powershell = Some(root.join("must-not-run-pwsh.exe"));
    group
}

#[test]
fn credentials_resolve_once_in_separate_domains_without_environment_mutation() {
    let args = source();
    let mut reads = Vec::new();
    let resolved = resolve_inputs_with(
        &portable_group(),
        &args.server,
        &args.database,
        Some("client_login"),
        None,
        "CLIENT_SLOT",
        |name| {
            reads.push(name.to_owned());
            Ok(format!("secret-for-{name}"))
        },
    )
    .unwrap();
    assert_eq!(reads, ["CLIENT_SLOT", "REGISTRATION_SLOT", "INFOBASE_SLOT"]);
    assert_eq!(resolved.sql_user, "client_login");
    assert_eq!(resolved.creator.database_user, "registration_login");
    assert_eq!(resolved.creator.infobase_user, "ib_login");
    assert_eq!(resolved.sql_password, "secret-for-CLIENT_SLOT");
    assert_eq!(
        resolved.creator.database_password,
        "secret-for-REGISTRATION_SLOT"
    );
    assert_eq!(
        resolved.creator.infobase_password,
        "secret-for-INFOBASE_SLOT"
    );
}
#[test]
fn integrated_or_empty_credentials_do_not_become_registration_authority() {
    let args = source();
    let mut reads = 0;
    let refusal = resolve_inputs_with(
        &portable_group(),
        &args.server,
        &args.database,
        None,
        None,
        "CLIENT_SLOT",
        |_| {
            reads += 1;
            Ok("must-not-read".into())
        },
    );
    assert!(refusal.is_err());
    assert_eq!(reads, 0);
    let refusal = resolve_inputs_with(
        &portable_group(),
        &args.server,
        &args.database,
        Some("client"),
        Some("client-secret"),
        "unused",
        |_| Ok(String::new()),
    );
    assert!(refusal.is_err());
}
#[test]
fn invalid_ports_and_deadline_refuse_before_credentials_or_creation() {
    let args = source();
    for group in [
        ManagedWorkerCliArgs {
            managed_timeout_seconds: Some(0),
            ..portable_group()
        },
        ManagedWorkerCliArgs {
            managed_timeout_seconds: Some(121),
            ..portable_group()
        },
        ManagedWorkerCliArgs {
            managed_worker_last: Some(18188),
            ..portable_group()
        },
        ManagedWorkerCliArgs {
            managed_ras_port: Some(18060),
            ..portable_group()
        },
        ManagedWorkerCliArgs {
            managed_agent_port: Some(0),
            ..portable_group()
        },
    ] {
        let refusal = resolve_inputs_with(
            &group,
            &args.server,
            &args.database,
            Some("client"),
            Some("secret"),
            "unused",
            |_| panic!("credential lookup before preflight"),
        );
        assert!(refusal.is_err());
    }
}
#[test]
fn typed_committed_but_unproved_handoff_is_not_success() {
    for state in [
        PublicationState::NotStarted,
        PublicationState::StageUnproved,
        PublicationState::Staged,
        PublicationState::CommitUnproved,
        PublicationState::Committed,
    ] {
        assert!(
            require_completed(&ManagedOutcome {
                publication: state,
                handoff_proved: false,
                diagnostic: None
            })
            .is_err()
        );
    }
    assert!(
        require_completed(&ManagedOutcome {
            publication: PublicationState::Committed,
            handoff_proved: true,
            diagnostic: Some("unproved".into())
        })
        .is_err()
    );
    require_completed(&ManagedOutcome {
        publication: PublicationState::Committed,
        handoff_proved: true,
        diagnostic: None,
    })
    .unwrap();
}

struct Owner {
    identity: usize,
    dropped: Rc<Cell<bool>>,
    trace: Rc<RefCell<Vec<&'static str>>>,
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}
fn owner() -> Owner {
    Owner {
        identity: 7,
        dropped: Rc::new(Cell::new(false)),
        trace: Rc::new(RefCell::new(Vec::new())),
    }
}
fn retain(result: Completion<usize, Owner, Owner>) -> Retained<Owner, Owner, usize> {
    match result {
        Completion::Unknown(retained) => retained,
        Completion::Known(_) => panic!("unknown was reported known"),
    }
}
#[test]
fn same_owner_is_registered_borrowed_then_cold_before_success() {
    let owner = owner();
    let trace = owner.trace.clone();
    let dropped = owner.dropped.clone();
    let result: Completion<usize, Owner, Owner> = drive(
        Fresh::Ready(owner),
        |owner| {
            assert_eq!(owner.identity, 7);
            owner.trace.borrow_mut().push("register/load");
            Ok(())
        },
        |owner| {
            assert_eq!(owner.identity, 7);
            owner.trace.borrow_mut().push("typed stage/commit/handoff");
            Ok(9)
        },
        |owner| {
            assert_eq!(owner.identity, 7);
            owner.trace.borrow_mut().push("original cold/BOTH");
            Ok(())
        },
    );
    assert!(matches!(result, Completion::Known(9)));
    assert_eq!(
        *trace.borrow(),
        [
            "register/load",
            "typed stage/commit/handoff",
            "original cold/BOTH"
        ]
    );
    assert!(dropped.get());
}
#[test]
fn retained_creation_preserves_owner_and_never_registers_or_signals() {
    let owner = owner();
    let dropped = owner.dropped.clone();
    let held = retain(drive(
        Fresh::Retained(owner),
        |_| panic!("register"),
        |_| panic!("consume"),
        |_| panic!("cold"),
    ));
    assert!(matches!(held, Retained::Creation(_)));
    assert!(!dropped.get());
}
#[test]
fn register_failure_keeps_strong_original_and_skips_consumer_shutdown() {
    let owner = owner();
    let dropped = owner.dropped.clone();
    let held = retain(drive(
        Fresh::Ready(owner),
        |_| bail!("original register error"),
        |_| panic!("consume"),
        |_| panic!("cold"),
    ));
    match &held {
        Retained::Ready { failure, .. } => {
            assert_eq!(failure.phase, "register/load");
            assert_eq!(
                failure._error.as_ref().unwrap().to_string(),
                "original register error"
            );
        }
        _ => panic!("wrong original"),
    }
    assert!(!dropped.get());
}
#[test]
fn commit_unknown_keeps_original_without_undo_or_cold_signal() {
    let owner = owner();
    let dropped = owner.dropped.clone();
    let held = retain(drive(
        Fresh::Ready(owner),
        |_| Ok(()),
        |_| bail!("SQL commit unproved"),
        |_| panic!("cold"),
    ));
    assert!(matches!(held, Retained::Ready { .. }));
    assert!(!dropped.get());
}
#[test]
fn confirmed_commit_and_failed_cold_preserve_report_and_owner() {
    let owner = owner();
    let dropped = owner.dropped.clone();
    let held = retain(drive(
        Fresh::Ready(owner),
        |_| Ok(()),
        |_| Ok(42),
        |_| bail!("BOTH missing"),
    ));
    match &held {
        Retained::Ready {
            failure,
            _completed_report,
            ..
        } => {
            assert_eq!(failure.phase, "owned cold shutdown");
            assert_eq!(*_completed_report, Some(42));
        }
        _ => panic!("wrong owner"),
    }
    assert!(!dropped.get());
}
#[test]
fn consumer_panic_keeps_original_payload_owner_and_never_cleans_up() {
    let owner = owner();
    let dropped = owner.dropped.clone();
    let held = retain(drive(
        Fresh::Ready(owner),
        |_| Ok(()),
        |_| panic!("original consumer panic"),
        |_| panic!("cold"),
    ));
    match &held {
        Retained::Ready { failure, .. } => assert!(failure._panic.is_some()),
        _ => panic!("wrong owner"),
    }
    assert!(!dropped.get());
}
#[test]
fn generated_typed_target_overrides_only_with_private_creator_values() {
    let mut source = source();
    let mut activation = activation();
    // Pure consumer mapping fixture, not an authority fixture: no native owner,
    // verifier, stage, commit or signal can be minted from this value.
    source.cluster_id = Some(uuid::Uuid::new_v4());
    activation.infobase_id = Some(uuid::Uuid::new_v4());
    let resolved = resolve_inputs_with(
        &portable_group(),
        "selected-sql",
        "selected-db",
        Some("client-login"),
        Some("client-password"),
        "unused",
        |name| Ok(format!("{name}-password")),
    )
    .unwrap();
    let target = GeneratedTarget {
        rac: resolved.creator.platform_bin.join("rac.exe"),
        endpoint: "localhost:18045".into(),
        server: "selected-sql".into(),
        database: "selected-db".into(),
        cluster: uuid::Uuid::new_v4(),
        infobase: uuid::Uuid::new_v4(),
    };
    let bound_source = bind_source(&target, &source, &resolved);
    let bound_activation = bind_activation(&target, &activation, &resolved);
    assert_eq!(bound_source.cluster_id, Some(target.cluster));
    assert_eq!(bound_activation.infobase_id, Some(target.infobase));
    assert_eq!(bound_source.rac, target.rac);
    assert_eq!(bound_activation.ras_endpoint, target.endpoint);
    assert_eq!(bound_source.database, target.database);
    assert_eq!(bound_activation.server, target.server);
    assert_eq!(bound_source.sql_pwd.as_deref(), Some("client-password"));
    assert_eq!(
        bound_activation.infobase_pwd.as_deref(),
        Some("INFOBASE_SLOT-password")
    );
    assert!(bound_source.managed_worker.managed_worker_parent.is_none());
    assert!(
        bound_activation
            .managed_worker
            .managed_worker_parent
            .is_none()
    );
    assert_eq!(bound_source.source_path, source.source_path);
}
#[test]
fn explicit_empty_sql_password_refuses_without_environment_fallback() {
    let refused = resolve_inputs_with(
        &portable_group(),
        "server",
        "database",
        Some("client"),
        Some(""),
        "MUST_NOT_READ",
        |_| panic!("empty explicit password must not fallback"),
    );
    assert!(refused.is_err());
}
#[test]
fn typed_postcommit_refusal_enters_same_retention_and_cannot_signal_or_undo() {
    let owner = owner();
    let dropped = owner.dropped.clone();
    let held = retain(drive(
        Fresh::Ready(owner),
        |_| Ok(()),
        |_| {
            let outcome = ManagedOutcome {
                publication: PublicationState::Committed,
                handoff_proved: false,
                diagnostic: Some("replacement unproved".into()),
            };
            require_completed(&outcome)?;
            Ok(1)
        },
        |_| panic!("unproved postcommit must not signal or undo"),
    ));
    match &held {
        Retained::Ready { failure, .. } => assert_eq!(failure.phase, "publication/handoff"),
        _ => panic!("original owner lost"),
    }
    assert!(!dropped.get());
}

#[test]
fn public_fresh_commands_refuse_tail_before_files_credentials_or_creation() {
    let missing =
        std::env::temp_dir().join(format!("managed-cli-tail-refusal-{}", uuid::Uuid::new_v4()));
    assert!(!missing.exists());
    let mut source = source();
    let mut activation = activation();
    for group in [&mut source.managed_worker, &mut activation.managed_worker] {
        group.managed_worker_parent = Some(missing.join("creator"));
        group.managed_platform_bin = Some(missing.join("platform"));
        group.managed_powershell = Some(missing.join("pwsh.exe"));
        group.managed_registration_pwd_env = Some("MUST_NOT_LOOK_UP_REGISTRATION".into());
        group.managed_infobase_pwd_env = Some("MUST_NOT_LOOK_UP_INFOBASE".into());
    }
    source.source_root = missing.join("source");
    source.tail_log_output = Some(missing.join("source-tail.bak"));
    activation.tail_log_output = Some(missing.join("activation-tail.bak"));
    source.sql_user = None;
    activation.sql_user = None;
    source.allow_non_lab = false;
    activation.allow_non_lab = false;
    for refusal in [
        crate::mssql_apply::apply_source_change(&source).unwrap_err(),
        crate::mssql::activate_staged_main(&activation).unwrap_err(),
    ] {
        assert_eq!(
            refusal.to_string(),
            "--tail-log-output is only valid for live activation; fresh managed worker refused before creator effects"
        );
    }
    assert!(!missing.exists());
    assert!(
        !source
            .managed_worker
            .managed_worker_parent
            .as_ref()
            .unwrap()
            .exists()
    );
    assert!(
        !activation
            .managed_worker
            .managed_worker_parent
            .as_ref()
            .unwrap()
            .exists()
    );
    assert!(!source.source_root.exists());
}

const CHILD_ARGS: [(&str, &str); 12] = [
    ("--managed-platform-bin", "F:/must-not-run-platform"),
    ("--managed-powershell", "F:/must-not-run-pwsh.exe"),
    ("--managed-agent-port", "18040"),
    ("--managed-cluster-port", "18041"),
    ("--managed-ras-port", "18045"),
    ("--managed-worker-first", "18060"),
    ("--managed-worker-last", "18063"),
    ("--managed-timeout-seconds", "20"),
    ("--managed-registration-user", "registration_login"),
    ("--managed-registration-pwd-env", "REGISTRATION_SLOT"),
    ("--managed-infobase-user", "ib_login"),
    ("--managed-infobase-pwd-env", "INFOBASE_SLOT"),
];
const LEGACY_ARGS: [(&str, &str); 6] = [
    ("--cluster-id", "11111111-1111-4111-8111-111111111111"),
    ("--infobase-id", "22222222-2222-4222-8222-222222222222"),
    ("--rac", "rac"),
    ("--ras-endpoint", "localhost:1545"),
    ("--infobase-user", "foreign"),
    ("--infobase-pwd", "foreign"),
];
fn ordinary_argv(command: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut original = argv(command).into_iter();
    while let Some(arg) = original.next() {
        if arg.starts_with("--managed-") {
            original.next();
        } else {
            args.push(arg);
        }
    }
    args
}
fn only_child(index: usize) -> ManagedWorkerCliArgs {
    let full = portable_group();
    let mut group = ManagedWorkerCliArgs::default();
    match index {
        0 => group.managed_platform_bin = full.managed_platform_bin,
        1 => group.managed_powershell = full.managed_powershell,
        2 => group.managed_agent_port = full.managed_agent_port,
        3 => group.managed_cluster_port = full.managed_cluster_port,
        4 => group.managed_ras_port = full.managed_ras_port,
        5 => group.managed_worker_first = full.managed_worker_first,
        6 => group.managed_worker_last = full.managed_worker_last,
        7 => group.managed_timeout_seconds = full.managed_timeout_seconds,
        8 => group.managed_registration_user = full.managed_registration_user,
        9 => group.managed_registration_pwd_env = full.managed_registration_pwd_env,
        10 => group.managed_infobase_user = full.managed_infobase_user,
        11 => group.managed_infobase_pwd_env = full.managed_infobase_pwd_env,
        _ => panic!("actual child index"),
    }
    group
}
fn remove_child(group: &mut ManagedWorkerCliArgs, index: usize) {
    match index {
        0 => group.managed_platform_bin = None,
        1 => group.managed_powershell = None,
        2 => group.managed_agent_port = None,
        3 => group.managed_cluster_port = None,
        4 => group.managed_ras_port = None,
        5 => group.managed_worker_first = None,
        6 => group.managed_worker_last = None,
        7 => group.managed_timeout_seconds = None,
        8 => group.managed_registration_user = None,
        9 => group.managed_registration_pwd_env = None,
        10 => group.managed_infobase_user = None,
        11 => group.managed_infobase_pwd_env = None,
        _ => panic!("actual child index"),
    }
}
fn require_incomplete_entry_refusals(mut group: ManagedWorkerCliArgs) {
    let missing =
        std::env::temp_dir().join(format!("managed-cli-selection-{}", uuid::Uuid::new_v4()));
    assert!(!missing.exists());
    if let Some(parent) = &mut group.managed_worker_parent {
        *parent = missing.join("parent");
    }
    if let Some(platform) = &mut group.managed_platform_bin {
        *platform = missing.join("platform");
    }
    if let Some(powershell) = &mut group.managed_powershell {
        *powershell = missing.join("pwsh.exe");
    }
    let mut source = source();
    let mut activation = activation();
    source.managed_worker = group.clone();
    activation.managed_worker = group;
    source.source_root = missing.join("source");
    source.cluster_id = Some(uuid::Uuid::new_v4());
    activation.infobase_id = Some(uuid::Uuid::new_v4());
    source.platform_profile = MssqlNativePlatformProfile::Platform8_5_1_1150;
    activation.platform_profile = MssqlNativePlatformProfile::Platform8_5_1_1150;
    source.tail_log_output = Some(missing.join("tail.bak"));
    activation.tail_log_output = Some(missing.join("tail.bak"));
    source.sql_user = None;
    activation.sql_user = None;
    source.watch = true;
    for error in [
        crate::mssql_apply::apply_source_change(&source).unwrap_err(),
        crate::mssql::activate_staged_main(&activation).unwrap_err(),
        crate::mssql_apply::watch_source_changes(&source).unwrap_err(),
        apply(&source).unwrap_err(),
        activate(&activation).unwrap_err(),
    ] {
        assert_eq!(
            error.to_string(),
            "managed worker parameters require --managed-worker-parent and all twelve child parameters before any effects"
        );
    }
    assert!(!missing.exists());
}
#[test]
fn every_single_managed_orphan_is_refused_by_both_cli_commands() {
    for command in ["mssql-apply-source-change", "mssql-activate-staged-main"] {
        for (flag, value) in CHILD_ARGS {
            let mut args = ordinary_argv(command);
            args.extend([flag, value].into_iter().map(str::to_owned));
            assert!(Cli::try_parse_from(args).is_err(), "{command}: {flag}");
        }
    }
}
#[test]
fn all_managed_children_conflict_with_each_legacy_argument_without_parent() {
    for command in ["mssql-apply-source-change", "mssql-activate-staged-main"] {
        for (flag, value) in CHILD_ARGS {
            for (legacy, old_value) in LEGACY_ARGS {
                let mut args = ordinary_argv(command);
                args.extend(
                    [flag, value, legacy, old_value]
                        .into_iter()
                        .map(str::to_owned),
                );
                let error = Cli::try_parse_from(args).unwrap_err();
                assert_eq!(
                    error.kind(),
                    clap::error::ErrorKind::ArgumentConflict,
                    "{command}: {flag} / {legacy}"
                );
            }
        }
    }
}
#[test]
fn programmatic_orphans_refuse_public_private_and_watch_before_other_policies() {
    for index in 0..12 {
        require_incomplete_entry_refusals(only_child(index));
    }
    let mut all_children = portable_group();
    all_children.managed_worker_parent = None;
    require_incomplete_entry_refusals(all_children);
}
#[test]
fn every_programmatic_partial_parent_group_refuses_all_effect_entries() {
    for index in 0..12 {
        let mut partial = portable_group();
        remove_child(&mut partial, index);
        require_incomplete_entry_refusals(partial);
    }
    require_incomplete_entry_refusals(ManagedWorkerCliArgs {
        managed_worker_parent: Some(std::env::temp_dir().join("must-not-create-parent-only")),
        ..Default::default()
    });
}
#[test]
fn empty_group_keeps_the_ordinary_public_policy_route() {
    let mut source = source();
    let mut activation = activation();
    source.managed_worker = Default::default();
    activation.managed_worker = Default::default();
    source.cluster_id = Some(uuid::Uuid::new_v4());
    source.infobase_id = Some(uuid::Uuid::new_v4());
    activation.cluster_id = Some(uuid::Uuid::new_v4());
    activation.infobase_id = Some(uuid::Uuid::new_v4());
    source.platform_profile = MssqlNativePlatformProfile::Platform8_5_1_1150;
    activation.platform_profile = MssqlNativePlatformProfile::Platform8_5_1_1150;
    for error in [
        crate::mssql_apply::apply_source_change(&source).unwrap_err(),
        crate::mssql::activate_staged_main(&activation).unwrap_err(),
        crate::mssql_apply::watch_source_changes(&source).unwrap_err(),
    ] {
        assert!(error.to_string().contains("explicitly unsupported"));
        assert!(!error.to_string().contains("managed worker"));
    }
}
#[test]
fn complete_group_and_empty_group_select_only_their_original_routes() {
    assert!(!selects_fresh(&ManagedWorkerCliArgs::default()).unwrap());
    assert!(selects_fresh(&portable_group()).unwrap());
    assert!(require_fresh_selection(&ManagedWorkerCliArgs::default()).is_err());
    require_fresh_selection(&portable_group()).unwrap();
}
#[test]
fn complete_programmatic_group_cannot_adopt_any_legacy_target_field() {
    let missing =
        std::env::temp_dir().join(format!("managed-cli-adoption-{}", uuid::Uuid::new_v4()));
    assert!(!missing.exists());
    for index in 0..6 {
        let mut source = source();
        let mut activation = activation();
        source.managed_worker.managed_worker_parent = Some(missing.join("source-parent"));
        activation.managed_worker.managed_worker_parent = Some(missing.join("activation-parent"));
        match index {
            0 => {
                source.cluster_id = Some(uuid::Uuid::new_v4());
                activation.cluster_id = source.cluster_id;
            }
            1 => {
                source.infobase_id = Some(uuid::Uuid::new_v4());
                activation.infobase_id = source.infobase_id;
            }
            2 => {
                source.rac = missing.join("rac.exe");
                activation.rac = source.rac.clone();
            }
            3 => {
                source.ras_endpoint = "foreign:1545".into();
                activation.ras_endpoint = source.ras_endpoint.clone();
            }
            4 => {
                source.infobase_user = Some("foreign".into());
                activation.infobase_user = source.infobase_user.clone();
            }
            5 => {
                source.infobase_pwd = Some("foreign".into());
                activation.infobase_pwd = source.infobase_pwd.clone();
            }
            _ => unreachable!(),
        }
        for error in [
            crate::mssql_apply::apply_source_change(&source).unwrap_err(),
            crate::mssql::activate_staged_main(&activation).unwrap_err(),
        ] {
            assert!(error.to_string().contains("cannot adopt"));
        }
        assert!(
            crate::mssql_apply::watch_source_changes(&source)
                .unwrap_err()
                .to_string()
                .contains("--watch")
        );
    }
    assert!(!missing.exists());
}
