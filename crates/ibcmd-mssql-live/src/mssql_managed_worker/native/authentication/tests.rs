use super::*;

fn plan() -> Plan {
    Plan::new(
        "ibcmd_12345678-1234-4234-8234-123456789abc",
        Uuid::parse_str("abcdef12-1234-4234-8234-123456789abc").unwrap(),
    )
    .unwrap()
}
fn row(name: &str) -> BTreeMap<String, String> {
    [
        ("name", name),
        ("auth", "pwd"),
        ("descr", ""),
        ("os-user", ""),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Denied,
    ZeroWithoutMutation,
    Bypass,
    NonzeroMutation,
    MissingControl,
    ControlDrift,
    RemoveNoop,
    UnknownChallenge,
    ForeignAfterChallenge,
    LateBeforeChallenge,
}
struct Mock {
    mode: Mode,
    measured: bool,
    admins: BTreeMap<String, BTreeMap<String, String>>,
    calls: Vec<String>,
    mutations: Vec<(Family, Credentials, Action, Credentials)>,
    records: Vec<(String, serde_json::Value)>,
    challenged: bool,
}
impl Mock {
    fn new(mode: Mode, measured: bool) -> Self {
        let p = plan();
        Self {
            mode,
            measured,
            admins: BTreeMap::from([(p.administrator.clone(), row(&p.administrator))]),
            calls: vec![],
            mutations: vec![],
            records: vec![],
            challenged: false,
        }
    }
}
impl Io for Mock {
    fn inventory(&mut self, family: Family, allowed: &[&str]) -> Result<Inventory> {
        self.calls.push(format!("{}:inventory", family.name()));
        if self.challenged && self.mode == Mode::ForeignAfterChallenge {
            self.admins.insert("foreign".into(), row("foreign"));
        }
        if self.mode == Mode::LateBeforeChallenge
            && self
                .records
                .iter()
                .any(|(event, _)| event == "correct_admin_mutation_observed")
        {
            bail!("deadline expired before collector");
        }
        Inventory::from_rows(self.admins.values().cloned().collect(), allowed)
    }
    fn mutation(
        &mut self,
        family: Family,
        challenge: Credentials,
        action: Action,
        credentials: Credentials,
    ) -> Result<Receipt> {
        self.calls
            .push(format!("{}:{challenge:?}:{action:?}", family.name()));
        self.mutations
            .push((family, challenge, action, credentials));
        let name = plan().name(family, challenge);
        if credentials == Credentials::Correct {
            match action {
                Action::Register if self.mode != Mode::MissingControl => {
                    self.admins
                        .insert(name, row(&plan().name(family, challenge)));
                    if self.mode == Mode::ControlDrift {
                        self.admins
                            .get_mut(&plan().administrator)
                            .unwrap()
                            .insert("descr".into(), "changed".into());
                    }
                }
                Action::Remove if self.mode != Mode::RemoveNoop => {
                    self.admins.remove(&name);
                }
                _ => {}
            }
            return Ok(Receipt {
                exit: 0,
                stdout: vec![],
                stderr: vec![],
            });
        }
        self.challenged = true;
        if self.mode == Mode::UnknownChallenge {
            bail!("original exit/pipes unproved");
        }
        if matches!(self.mode, Mode::Bypass | Mode::NonzeroMutation) {
            self.admins.insert(name.clone(), row(&name));
        }
        Ok(Receipt {
            exit: if matches!(self.mode, Mode::Bypass | Mode::ZeroWithoutMutation) {
                0
            } else {
                -1
            },
            stdout: vec![255, 128],
            stderr: vec![],
        })
    }
    fn record(&mut self, event: &'static str, value: serde_json::Value) -> Result<()> {
        self.records.push((event.into(), value));
        Ok(())
    }
    fn denial_is_measured(&self, _family: Family, _kind: Credentials, _receipt: &Receipt) -> bool {
        self.measured
    }
}

#[test]
fn actual_protocol_requires_two_controls_and_four_mutation_denials() {
    let mut io = Mock::new(Mode::Denied, true);
    verify(&mut io, &plan()).unwrap();
    assert_eq!(io.mutations.len(), 8);
    assert_eq!(
        io.records
            .iter()
            .filter(|(event, _)| event == "correct_admin_mutation_observed")
            .count(),
        2
    );
    let challenges: Vec<_> = io
        .records
        .iter()
        .filter(|(event, _)| event == "administration_mutation_challenge")
        .collect();
    assert_eq!(challenges.len(), 4);
    assert!(
        challenges
            .iter()
            .all(|(_, record)| record["classified_denial"] == true
                && record["observed_mutation"] == false
                && record["product_ownership"] == false)
    );
    assert_eq!(io.admins.len(), 1);
    assert_eq!(
        io.mutations
            .iter()
            .filter(|(_, kind, _, _)| *kind == Credentials::ImplicitOs)
            .count(),
        2
    );
}
#[test]
fn unclassified_nonzero_and_success_without_mutation_never_admit() {
    for (mode, measured) in [(Mode::Denied, false), (Mode::ZeroWithoutMutation, true)] {
        let mut io = Mock::new(mode, measured);
        assert!(verify(&mut io, &plan()).is_err());
        assert_eq!(io.mutations.len(), 3);
        assert_eq!(io.admins.len(), 1);
        assert!(
            !io.records
                .iter()
                .any(|(_, record)| record["classified_denial"] == true)
        );
        assert!(
            io.mutations
                .iter()
                .all(|(family, _, _, _)| *family == Family::Agent)
        );
    }
}
#[test]
fn bypass_with_zero_or_nonzero_is_removed_only_after_exact_proved_addition() {
    for mode in [Mode::Bypass, Mode::NonzeroMutation] {
        let mut io = Mock::new(mode, true);
        assert!(verify(&mut io, &plan()).is_err());
        assert_eq!(io.admins.len(), 1);
        assert_eq!(
            io.mutations.last(),
            Some(&(
                Family::Agent,
                Credentials::WrongPassword,
                Action::Remove,
                Credentials::Correct
            ))
        );
        let record = &io
            .records
            .iter()
            .find(|(event, _)| event == "administration_mutation_challenge")
            .unwrap()
            .1;
        assert_eq!(record["observed_mutation"], true);
        assert_eq!(record["classified_denial"], false);
    }
}
#[test]
fn control_presence_baseline_fields_and_restoration_are_required() {
    for mode in [Mode::MissingControl, Mode::ControlDrift, Mode::RemoveNoop] {
        let mut io = Mock::new(mode, true);
        assert!(verify(&mut io, &plan()).is_err());
        assert!(!io.challenged);
        assert!(
            !io.records
                .iter()
                .any(|(event, _)| event == "correct_admin_mutation_observed")
        );
    }
}
#[test]
fn original_unknown_has_no_after_collector_cleanup_retry_or_new_family() {
    let mut io = Mock::new(Mode::UnknownChallenge, true);
    assert!(verify(&mut io, &plan()).is_err());
    assert_eq!(io.calls.last().unwrap(), "agent:WrongPassword:Register");
    assert_eq!(io.mutations.len(), 3);
    assert!(
        !io.records
            .iter()
            .any(|(event, _)| event == "administration_mutation_challenge")
    );
}
#[test]
fn foreign_inventory_and_expired_before_collector_do_not_start_next_write() {
    let mut foreign = Mock::new(Mode::ForeignAfterChallenge, true);
    assert!(verify(&mut foreign, &plan()).is_err());
    assert_eq!(foreign.mutations.len(), 3);
    let mut late = Mock::new(Mode::LateBeforeChallenge, true);
    assert!(verify(&mut late, &plan()).is_err());
    assert_eq!(late.mutations.len(), 2);
    assert!(!late.challenged);
}
#[test]
fn actual_argument_builder_uses_exact_family_scope_and_credentials() {
    let p = plan();
    for family in [Family::Agent, Family::Cluster] {
        for credentials in [
            Credentials::Correct,
            Credentials::WrongPassword,
            Credentials::ImplicitOs,
        ] {
            let argv = p.mutation_arguments(
                family,
                credentials,
                Action::Register,
                credentials,
                ["correct-secret", "wrong-secret", "new-secret"],
            );
            assert_eq!(&argv[..3], &[family.name(), "admin", "register"]);
            assert!(argv.contains(&format!("--name={}", p.name(family, credentials))));
            assert!(argv.contains(&"--pwd=new-secret".into()));
            assert_eq!(
                argv.iter().filter(|a| a.starts_with("--cluster=")).count(),
                usize::from(family == Family::Cluster)
            );
            let prefix = format!("--{}-pwd=", family.name());
            let auth: Vec<_> = argv.iter().filter(|a| a.starts_with(&prefix)).collect();
            match credentials {
                Credentials::Correct => assert_eq!(auth, vec![&format!("{prefix}correct-secret")]),
                Credentials::WrongPassword => {
                    assert_eq!(auth, vec![&format!("{prefix}wrong-secret")])
                }
                Credentials::ImplicitOs => assert!(auth.is_empty()),
            }
            assert_eq!(
                argv.iter().filter(|a| a.contains("-user=")).count(),
                usize::from(credentials != Credentials::ImplicitOs)
            );
            let remove = p.mutation_arguments(
                family,
                credentials,
                Action::Remove,
                Credentials::Correct,
                ["correct-secret", "wrong-secret", "new-secret"],
            );
            assert!(
                !remove
                    .iter()
                    .any(|a| a.starts_with("--pwd=") || a.starts_with("--auth="))
            );
        }
    }
}
#[test]
fn complete_inventory_rejects_duplicates_foreign_os_and_bounded_fields() {
    let p = plan();
    let baseline = row(&p.administrator);
    for rows in [
        vec![],
        vec![baseline.clone(), baseline.clone()],
        vec![row("foreign")],
        vec![baseline.clone(); 3],
    ] {
        assert!(Inventory::from_rows(rows, &[&p.administrator]).is_err());
    }
    for (key, value) in [
        ("auth", "os"),
        ("os-user", "DOMAIN\\user"),
        ("name", "foreign"),
    ] {
        let mut bad = baseline.clone();
        bad.insert(key.into(), value.into());
        assert!(Inventory::from_rows(vec![bad], &[&p.administrator]).is_err());
    }
    let mut large = baseline.clone();
    large.insert("descr".into(), "x".repeat(4097));
    assert!(Inventory::from_rows(vec![large], &[&p.administrator]).is_err());
    let original = Inventory::from_rows(vec![baseline.clone()], &[&p.administrator]).unwrap();
    let mut changed = baseline;
    changed.insert("opaque".into(), "changed".into());
    let current = Inventory::from_rows(vec![changed], &[&p.administrator]).unwrap();
    assert_ne!(current, original);
    assert_ne!(current.digest().unwrap(), original.digest().unwrap());
}
#[test]
fn arbitrary_names_nil_clusters_and_noncanonical_uuids_are_refused() {
    for name in [
        "foreign",
        "ibcmd_00000000-0000-0000-0000-000000000000",
        "ibcmd_12345678123442348234123456789abc",
        "ibcmd_12345678-1234-4234-8234-123456789ABC",
    ] {
        assert!(Plan::new(name, plan().cluster).is_err());
    }
    assert!(Plan::new(&plan().administrator, Uuid::nil()).is_err());
}
