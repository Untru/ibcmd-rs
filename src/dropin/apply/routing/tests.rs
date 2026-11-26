use std::collections::VecDeque;
use std::io::Cursor;

use super::*;
use crate::dropin::apply::tests::{request, sample_report, session};
use crate::mssql_config_apply::{ApplyMode, ExclusiveAccessUnprovable, NeedsNativeApply};

struct Backend {
    calls: Vec<&'static str>,
    exclusive: VecDeque<Result<ConfigApplyReport>>,
    dynamic: Option<Result<ConfigApplyReport>>,
    qualifies: bool,
}
impl Dispatch for Backend {
    fn exclusive(&mut self) -> Result<ConfigApplyReport> {
        self.calls.push("exclusive");
        self.exclusive
            .pop_front()
            .expect("unexpected exclusive dispatch")
    }
    fn dynamic(&mut self) -> Result<ConfigApplyReport> {
        self.calls.push("dynamic");
        self.dynamic.take().expect("unexpected dynamic dispatch")
    }
    fn qualifies(&mut self) -> bool {
        self.calls.push("qualify");
        self.qualifies
    }
}
struct Input {
    terminal: bool,
    bytes: Cursor<Vec<u8>>,
    asked: usize,
    output: Vec<u8>,
}
impl Interaction for Input {
    fn available(&self) -> bool {
        self.terminal
    }
    fn choose(&mut self, listing: &str) -> Result<Option<Choice>> {
        self.asked += 1;
        read_choice(&mut self.bytes, &mut self.output, listing)
    }
}
fn input(bytes: &[u8]) -> Input {
    Input {
        terminal: true,
        bytes: Cursor::new(bytes.to_vec()),
        asked: 0,
        output: vec![],
    }
}
fn refused(database: &str, late: bool) -> anyhow::Error {
    ExclusiveAccessRefused {
        database: database.into(),
        sessions: vec![session(51, "owned-host", "1CV83 Server")],
        in_transaction: late,
    }
    .into()
}
fn backend(error: anyhow::Error, qualifies: bool) -> Backend {
    Backend {
        calls: vec![],
        exclusive: [Err(error)].into(),
        dynamic: None,
        qualifies,
    }
}
fn req(mode: &str) -> ApplyRequest {
    request(&["config", "apply", &format!("--dynamic={mode}")])
}

#[test]
fn explicit_dynamic_choice_uses_real_prompt_reader_then_fresh_dynamic_dispatch() {
    let mut db = backend(refused("db", false), true);
    let mut report = sample_report();
    report.mode = ApplyMode::Dynamic;
    db.dynamic = Some(Ok(report));
    let mut io = input(b"3\r\n");
    assert!(
        matches!(apply(&req("prompt"),"db",&mut db,&mut io),Outcome::Applied(r) if r.mode==ApplyMode::Dynamic)
    );
    assert_eq!(db.calls, ["exclusive", "qualify", "dynamic"]);
    assert_eq!(io.asked, 1);
    let text = String::from_utf8(io.output).unwrap();
    assert!(
        text.contains("owned-host")
            && text.contains("SQL-guard")
            && text.contains("3 Обновить динамически")
    );
}

#[test]
fn force_never_exclusive_and_other_modes_never_silently_become_dynamic() {
    let mut db = backend(refused("db", false), true);
    db.dynamic = Some(Ok(sample_report()));
    let mut io = input(b"1\n");
    assert!(matches!(
        apply(&req("force"), "db", &mut db, &mut io),
        Outcome::Applied(_)
    ));
    assert_eq!(db.calls, ["dynamic"]);
    assert_eq!(io.asked, 0);
    for mode in ["auto", "disable"] {
        let mut db = backend(refused("db", false), true);
        let mut io = input(b"3\n");
        assert!(matches!(
            apply(&req(mode), "db", &mut db, &mut io),
            Outcome::Failed(_)
        ));
        assert!(!db.calls.contains(&"dynamic"));
        assert_eq!(io.asked, 0);
    }
}

#[test]
fn successful_exclusive_and_true_noop_do_not_read_input_or_qualify() {
    for noop in [false, true] {
        let mut report = sample_report();
        report.nothing_to_apply = noop;
        let mut db = backend(refused("db", false), true);
        db.exclusive = [Ok(report)].into();
        let mut io = input(b"3\n");
        let result = apply(&req("prompt"), "db", &mut db, &mut io);
        assert_eq!(matches!(result, Outcome::NothingToApply(_)), noop);
        assert_eq!(db.calls, ["exclusive"]);
        assert_eq!(io.asked, 0);
    }
}

#[test]
fn cancel_eof_invalid_and_oversized_input_dispatch_no_second_operation() {
    for bytes in [
        b"1\n".to_vec(),
        vec![],
        b"\n".to_vec(),
        b"3extra\n".to_vec(),
        b"yes\n".to_vec(),
        b"2".to_vec(),
        b"2\r".to_vec(),
        b"3".to_vec(),
        b"3\r".to_vec(),
        vec![b'3'; 65],
        vec![0xff, b'\n'],
    ] {
        let mut db = backend(refused("db", false), true);
        let mut io = input(&bytes);
        let result = apply(&req("prompt"), "db", &mut db, &mut io);
        if bytes.as_slice() == b"1\n" {
            assert!(matches!(result, Outcome::Cancelled));
        } else {
            assert!(matches!(result, Outcome::Refused(_)));
        }
        assert_eq!(db.calls, ["exclusive", "qualify"]);
        assert_eq!(io.asked, 1);
    }
    assert_eq!(super::super::tell(Outcome::Cancelled, None), 0);
}

#[test]
fn actual_reader_requires_complete_lf_or_crlf_before_any_choice() {
    for (word, expected) in [
        (b'1', Choice::Cancel),
        (b'2', Choice::RetryExclusive),
        (b'3', Choice::Dynamic),
    ] {
        for ending in [b"\n".as_slice(), b"\r\n".as_slice()] {
            let mut bytes = vec![word];
            bytes.extend_from_slice(ending);
            assert_eq!(
                read_choice(&mut Cursor::new(bytes), &mut Vec::new(), "sessions").unwrap(),
                Some(match word {
                    b'1' => Choice::Cancel,
                    b'2' => Choice::RetryExclusive,
                    _ => Choice::Dynamic,
                })
            );
        }
        for bytes in [vec![word], vec![word, b'\r']] {
            assert!(
                read_choice(&mut Cursor::new(bytes), &mut Vec::new(), "sessions").is_err(),
                "{expected:?}"
            );
        }
    }
}

#[test]
fn closed_nonterminal_input_never_prompts_even_with_force_warning_flag() {
    let mut request = req("prompt");
    request.force = true;
    let mut db = backend(refused("db", false), true);
    let mut io = input(b"3\n");
    io.terminal = false;
    assert!(matches!(
        apply(&request, "db", &mut db, &mut io),
        Outcome::Failed(_)
    ));
    assert_eq!(io.asked, 0);
    assert_eq!(db.calls, ["exclusive", "qualify"]);
}

#[test]
fn force_warning_flag_and_input_failure_never_supply_dynamic_consent() {
    let mut request = req("prompt");
    request.force = true;
    let mut db = backend(refused("db", false), true);
    let mut io = input(b"");
    assert!(matches!(
        apply(&request, "db", &mut db, &mut io),
        Outcome::Refused(_)
    ));
    assert_eq!(db.calls, ["exclusive", "qualify"]);
    struct BrokenInput;
    impl Interaction for BrokenInput {
        fn available(&self) -> bool {
            true
        }
        fn choose(&mut self, _: &str) -> Result<Option<Choice>> {
            bail!("terminal read failed")
        }
    }
    let mut db = backend(refused("db", false), true);
    assert!(matches!(
        apply(&request, "db", &mut db, &mut BrokenInput),
        Outcome::Refused(_)
    ));
    assert_eq!(db.calls, ["exclusive", "qualify"]);
}

#[test]
fn late_foreign_empty_visibility_and_unknown_failures_never_offer_a_retry() {
    let empty = ExclusiveAccessRefused {
        database: "db".into(),
        sessions: vec![],
        in_transaction: false,
    }
    .into();
    for error in [
        refused("db", true),
        refused("foreign", false),
        empty,
        ExclusiveAccessUnprovable {
            reason: "partial/unknown session census".into(),
        }
        .into(),
        anyhow::anyhow!("COMMIT response lost: exclusive access is not established"),
        NeedsNativeApply::apply("unknown metadata").into(),
    ] {
        let mut db = backend(error, true);
        let mut io = input(b"2\n");
        let _ = apply(&req("prompt"), "db", &mut db, &mut io);
        assert_eq!(io.asked, 0);
        assert!(!db.calls.contains(&"dynamic"));
        assert_eq!(db.calls.iter().filter(|&&s| s == "exclusive").count(), 1);
    }
}

#[test]
fn unqualified_stage_and_unproved_exclusivity_or_termination_never_prompt() {
    for (qualifies, exclusive, terminate) in [
        (false, ExclusivityMode::Sql, SessionTerminate::Disable),
        (true, ExclusivityMode::Assumed, SessionTerminate::Disable),
        (true, ExclusivityMode::Sql, SessionTerminate::Force),
        (true, ExclusivityMode::Sql, SessionTerminate::Prompt),
    ] {
        let mut request = req("prompt");
        request.exclusivity = exclusive;
        request.session_terminate = terminate;
        let mut db = backend(refused("db", false), qualifies);
        let mut io = input(b"3\n");
        let _ = apply(&request, "db", &mut db, &mut io);
        assert_eq!(io.asked, 0);
        assert!(!db.calls.contains(&"dynamic"));
    }
}

#[test]
fn one_explicit_retry_keeps_exclusive_guards_and_cannot_loop_or_fall_back() {
    for success in [true, false] {
        let mut db = backend(refused("db", false), true);
        db.exclusive.push_back(if success {
            Ok(sample_report())
        } else {
            Err(refused("db", true))
        });
        let mut io = input(b"2\n3\n");
        let result = apply(&req("prompt"), "db", &mut db, &mut io);
        assert_eq!(matches!(result, Outcome::Applied(_)), success);
        assert_eq!(db.calls, ["exclusive", "qualify", "exclusive"]);
        assert_eq!(io.asked, 1);
    }
}

#[test]
fn dynamic_choice_preserves_fresh_admission_refusal_and_unknown_outcome() {
    for error in [
        NeedsNativeApply::apply("stage changed during prompt; CAS refused").into(),
        anyhow::anyhow!("transaction outcome uncertain; inspect recovery artifact before retry"),
    ] {
        let mut db = backend(refused("db", false), true);
        db.dynamic = Some(Err(error));
        let mut io = input(b"3\n2\n");
        let result = apply(&req("prompt"), "db", &mut db, &mut io);
        assert!(!matches!(
            result,
            Outcome::Applied(_) | Outcome::NothingToApply(_)
        ));
        assert_eq!(db.calls, ["exclusive", "qualify", "dynamic"]);
        assert_eq!(io.asked, 1);
    }
}
