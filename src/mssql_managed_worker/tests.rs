use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
struct Host {
    observations: Rc<RefCell<Vec<Observation>>>,
    effects: Rc<RefCell<Vec<&'static str>>>,
    registered: Uuid,
    register_error: bool,
    load_error: bool,
    switch_error: bool,
    stop_error: bool,
    cold_observations: Vec<bool>,
}

impl OwnedRuntime for Host {
    fn observe(&mut self) -> Result<Observation> {
        let mut o = self.observations.borrow_mut();
        if o.is_empty() {
            bail!("observation exhausted");
        }
        Ok(o.remove(0))
    }
    fn register(&mut self) -> Result<Uuid> {
        self.effects.borrow_mut().push("register");
        if self.register_error {
            bail!("original registration outcome unknown");
        }
        Ok(self.registered)
    }
    fn load(&mut self, _: Uuid) -> Result<()> {
        self.effects.borrow_mut().push("load");
        if self.load_error {
            bail!("original load outcome unknown");
        }
        Ok(())
    }
    fn turn_off(&mut self, _: Uuid, _: &ProcessIdentity) -> Result<()> {
        self.effects.borrow_mut().push("turn_off");
        if self.switch_error {
            bail!("handoff command unknown");
        }
        Ok(())
    }
    fn stop_owned(&mut self, journal: &mut Journal) -> Result<()> {
        journal.append("test_direct_stop_intent", ())?;
        self.effects.borrow_mut().push("stop_owned");
        if self.stop_error {
            bail!("original stop completion unproved");
        }
        journal.append("test_direct_stop_confirmed", ())?;
        Ok(())
    }
    fn require_cold(&mut self) -> Result<()> {
        self.effects.borrow_mut().push("require_cold");
        if self.cold_observations.is_empty() || !self.cold_observations.remove(0) {
            bail!("new private process/listener or original exit/pipe proof lost");
        }
        Ok(())
    }
}

fn identity(pid: u32) -> ProcessIdentity {
    ProcessIdentity {
        pid,
        parent: 40,
        birth_100ns: u64::from(pid) * 100,
        executable: std::env::current_exe().unwrap(),
        command_sha256: "A".repeat(64),
    }
}

fn binding(target: Uuid) -> LifetimeBinding {
    LifetimeBinding {
        nonce: Uuid::new_v4(),
        root: std::env::temp_dir(),
        cluster: Uuid::new_v4(),
        infobase: target,
        database: "owned_one".into(),
        agent: identity(41),
        ras: identity(42),
    }
}

fn observation(
    b: &LifetimeBinding,
    registered: bool,
    loaded: bool,
    pid: Option<u32>,
) -> Observation {
    Observation {
        binding: b.clone(),
        registrations: if registered {
            BTreeSet::from([b.infobase])
        } else {
            BTreeSet::new()
        },
        loaded: if loaded {
            BTreeSet::from([b.infobase])
        } else {
            BTreeSet::new()
        },
        worker: pid.map(|pid| (Uuid::from_u128(u128::from(pid)), identity(pid))),
        password_only_admins_exact: true,
        authenticated_inventory_exact: true,
        lease_original_handle_exact: true,
        unknown_administration: false,
    }
}

/// The tests exercise the same file-backed journal and orchestration used by
/// the creator; only native effects/observations are replaced.
fn setup(
    extra: impl FnOnce(&LifetimeBinding) -> Vec<Observation>,
) -> (ManagedWorker<Host>, PathBuf, Rc<RefCell<Vec<&'static str>>>) {
    let id = Uuid::new_v4();
    let initial = binding(Uuid::nil());
    let mut registered = initial.clone();
    registered.infobase = id;
    let mut records = vec![
        observation(&initial, false, false, None),
        observation(&initial, false, false, None),
        observation(&registered, true, false, None),
        observation(&registered, true, true, Some(43)),
    ];
    records.extend(extra(&registered));
    let effects = Rc::new(RefCell::new(Vec::new()));
    let host = Host {
        observations: Rc::new(RefCell::new(records)),
        effects: effects.clone(),
        registered: id,
        register_error: false,
        load_error: false,
        switch_error: false,
        stop_error: false,
        cold_observations: vec![true; 6],
    };
    let path = std::env::temp_dir().join(format!("ibcmd-managed-journal-{}.jsonl", initial.nonce));
    let journal = Journal::create(&path, initial.nonce).unwrap();
    let mut session = ManagedWorker::from_fresh_creator(host, initial, journal).unwrap();
    session.register_and_load().unwrap();
    (session, path, effects)
}

#[test]
fn shutdown_retains_history_and_original_journal_and_invalidates_publication() {
    let (mut session, path, effects) = setup(|b| vec![observation(b, true, true, Some(43))]);
    let cold = session.shutdown().unwrap();
    assert_eq!(cold.session.ever_loaded.len(), 1);
    assert_eq!(
        &*effects.borrow(),
        &["register", "load", "stop_owned", "require_cold"]
    );
    drop(cold);
    assert!(session.prepare().is_err());
    drop(session);
    let rows = retained_rows(&path);
    let position = |event| rows.iter().position(|row| row["event"] == event).unwrap();
    assert!(position("cold_shutdown_intent") < position("test_direct_stop_intent"));
    assert!(position("test_direct_stop_confirmed") < position("cold_shutdown_confirmed"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn shutdown_unknown_and_foreign_observation_never_issue_cold_or_undo_authority() {
    for scenario in ["second_base", "stop_unknown", "cold_unknown"] {
        let (mut session, path, effects) = setup(|b| {
            let mut o = observation(b, true, true, Some(43));
            if scenario == "second_base" {
                o.loaded.insert(Uuid::new_v4());
            }
            vec![o]
        });
        session.runtime.stop_error = scenario == "stop_unknown";
        if scenario == "cold_unknown" {
            session.runtime.cold_observations = vec![false];
        }
        assert!(session.shutdown().is_err(), "{scenario}");
        assert_eq!(
            effects.borrow().contains(&"stop_owned"),
            scenario != "second_base"
        );
        assert!(session.prepare().is_err());
        assert_eq!(session.ever_loaded.len(), 1);
        dispose(session, &path);
    }
}

#[test]
fn cold_replacement_prevents_undo_and_unknown_sql_can_never_retry() {
    for scenario in ["before_sql", "unknown_sql", "after_sql"] {
        let (mut session, path, _) = setup(|b| vec![observation(b, true, true, Some(43))]);
        session.runtime.cold_observations = match scenario {
            "before_sql" => vec![true, false],
            "after_sql" => vec![true, true, false],
            _ => vec![true, true],
        };
        let mut cold = session.shutdown().unwrap();
        let calls = RefCell::new(0);
        let result = cold.run_cold_transaction_once(|_| {
            *calls.borrow_mut() += 1;
            if scenario == "unknown_sql" {
                bail!("COMMIT response lost");
            }
            Ok(())
        });
        assert!(result.is_err(), "{scenario}");
        assert_eq!(*calls.borrow(), usize::from(scenario != "before_sql"));
        if scenario == "after_sql" {
            assert!(result.unwrap_err().to_string().contains("SQL confirmed"));
        }
        assert!(
            cold.run_cold_transaction_once(|_| {
                *calls.borrow_mut() += 1;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(*calls.borrow(), usize::from(scenario != "before_sql"));
        drop(cold);
        dispose(session, &path);
    }
}

fn retained_rows(path: &Path) -> Vec<serde_json::Value> {
    // Windows journal intentionally denies any second handle until session ends.
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn dispose(session: ManagedWorker<Host>, path: &Path) {
    drop(session);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn second_base_and_removed_registration_refuse_before_stage() {
    for scenario in [
        "registered_b",
        "loaded_b",
        "disconnect_b",
        "target_unregistered",
        "admin",
        "lease",
        "uuid",
        "pid",
        "birth",
        "command",
        "worker_uuid",
        "worker_pid",
    ] {
        let (mut session, path, effects) = setup(|b| {
            let mut o = observation(b, true, true, Some(43));
            let second = Uuid::new_v4();
            match scenario {
                "registered_b" => {
                    o.registrations.insert(second);
                }
                "loaded_b" | "disconnect_b" => {
                    o.loaded.insert(second);
                }
                "target_unregistered" => o.registrations.clear(),
                "admin" => o.unknown_administration = true,
                "lease" => o.lease_original_handle_exact = false,
                "uuid" => o.binding.infobase = second,
                "pid" => o.binding.agent.pid += 1,
                "birth" => o.binding.ras.birth_100ns += 1,
                "command" => o.binding.ras.command_sha256 = "B".repeat(64),
                "worker_uuid" => o.worker.as_mut().unwrap().0 = second,
                "worker_pid" => o.worker.as_mut().unwrap().1.pid += 1,
                _ => unreachable!(),
            }
            vec![o]
        });
        let writes = RefCell::new(0);
        let result = session.publish_and_handoff(
            || {
                *writes.borrow_mut() += 1;
                Ok(())
            },
            || {
                *writes.borrow_mut() += 1;
                Ok(())
            },
        );
        assert!(result.is_err(), "{scenario}");
        assert_eq!(*writes.borrow(), 0, "{scenario}");
        assert_eq!(&*effects.borrow(), &["register", "load"]);
        // Missing current connections/registration never erases load history.
        assert_eq!(session.ever_loaded.len(), 1);
        dispose(session, &path);
    }
}

#[test]
fn post_commit_identity_failure_is_committed_and_never_signals() {
    let (mut session, path, effects) = setup(|b| {
        let clean = observation(b, true, true, Some(43));
        let mut drift = observation(b, true, true, Some(43));
        drift.binding.agent.birth_100ns += 1;
        vec![clean, observation(b, true, true, Some(43)), drift]
    });
    let mut committed = false;
    let outcome = session
        .publish_and_handoff(
            || Ok(()),
            || {
                committed = true;
                Ok(())
            },
        )
        .unwrap();
    assert!(committed);
    assert_eq!(outcome.publication, PublicationState::Committed);
    assert!(!outcome.handoff_proved);
    assert_eq!(&*effects.borrow(), &["register", "load"]);
    drop(session);
    let rows = retained_rows(&path);
    assert!(rows.iter().any(|row| row["event"] == "commit_confirmed"));
    assert_eq!(rows.last().unwrap()["value"]["publication"], "Committed");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn uncertain_sql_and_failed_handoff_are_distinct_from_rollback() {
    for which in ["stage", "commit", "handoff"] {
        let (mut session, path, effects) = setup(|b| {
            (0..3)
                .map(|_| observation(b, true, true, Some(43)))
                .collect()
        });
        session.runtime.switch_error = which == "handoff";
        let outcome = session
            .publish_and_handoff(
                || {
                    if which == "stage" {
                        bail!("stage SQL response lost")
                    } else {
                        Ok(())
                    }
                },
                || {
                    if which == "commit" {
                        bail!("COMMIT response lost")
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap();
        assert_eq!(
            outcome.publication,
            match which {
                "stage" => PublicationState::StageUnproved,
                "commit" => PublicationState::CommitUnproved,
                _ => PublicationState::Committed,
            }
        );
        assert!(!outcome.handoff_proved);
        assert_eq!(effects.borrow().contains(&"turn_off"), which == "handoff");
        dispose(session, &path);
    }
}

#[test]
fn successful_turn_off_without_replacement_is_not_handoff_success() {
    let (mut session, path, _) = setup(|b| {
        (0..4)
            .map(|_| observation(b, true, true, Some(43)))
            .collect()
    });
    let outcome = session.publish_and_handoff(|| Ok(()), || Ok(())).unwrap();
    assert_eq!(outcome.publication, PublicationState::Committed);
    assert!(!outcome.handoff_proved);
    assert!(
        outcome
            .diagnostic
            .unwrap()
            .contains("replacement not proved")
    );
    dispose(session, &path);
}

#[test]
fn actual_orchestration_records_intents_before_native_effect_and_replacement() {
    let (mut session, path, effects) = setup(|b| {
        let mut records: Vec<_> = (0..3)
            .map(|_| observation(b, true, true, Some(43)))
            .collect();
        records.push(observation(b, true, true, Some(44)));
        records
    });
    let outcome = session.publish_and_handoff(|| Ok(()), || Ok(())).unwrap();
    assert_eq!(outcome.publication, PublicationState::Committed);
    assert!(outcome.handoff_proved);
    assert_eq!(&*effects.borrow(), &["register", "load", "turn_off"]);
    drop(session);
    let rows = retained_rows(&path);
    let events: Vec<_> = rows
        .iter()
        .map(|row| row["event"].as_str().unwrap())
        .collect();
    assert_eq!(
        events,
        [
            "creator_barrier",
            "registration_intent",
            "registration_confirmed",
            "load_intent",
            "load_confirmed",
            "stage_intent",
            "stage_confirmed",
            "commit_intent",
            "commit_confirmed",
            "handoff_intent",
            "handoff_command_confirmed",
            "replacement_confirmed"
        ]
    );
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["sequence"], index + 1);
    }
    std::fs::remove_file(path).unwrap();
}
