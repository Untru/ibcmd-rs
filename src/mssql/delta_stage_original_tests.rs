//! Calls the alias identity handler used by the actual delta planner.
use super::*;
use crate::mssql::source_original_consumers_tests::{Fixture, OBJECT, metadata, stage_args};

#[test]
fn original_actual_delta_derivation_keeps_clean_pending_alias_and_forced_units() {
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        let a = f.write(
            "Catalogs/A.xml",
            metadata("Catalog", "A", OBJECT, version, ""),
        );
        let b = f.write(
            "Catalogs/B.xml",
            metadata(
                "Catalog",
                "B",
                "22345678-1234-4234-8234-123456789abc",
                version,
                "",
            ),
        );
        let source = f.context();
        let aliases = BTreeMap::from([(format!("{OBJECT}.0"), "pending-name".into())]);
        // Only the SQL/export boundary is substituted: it reports identical
        // files. The production derivation still parses the original XMLs.
        let compared = super::super::stage_guard::TargetComparison {
            differences: Vec::new(),
            compared: 2,
            identical: 2,
            seconds: 0.0,
        };
        let result = plan_from_comparison(
            &stage_args(&f),
            &[a.as_path(), b.as_path()],
            &Plan::default(),
            &aliases,
            Some(&source),
            compared,
        )
        .unwrap();
        let Outcome::Active(delta) = result else {
            panic!("actual derivation must remain active");
        };
        assert!(delta.prepares(&f.root, &a));
        assert!(!delta.prepares(&f.root, &b));
        assert_eq!(
            delta.aliased_units,
            HashSet::from(["catalogs/a.xml".to_owned()])
        );
        assert!(delta.dirty_units.is_empty());
        assert_eq!(
            (
                delta.stats.compared,
                delta.stats.identical,
                delta.stats.differing
            ),
            (2, 2, 0)
        );
        let overrides = Plan {
            changed: vec![(
                "22345678-1234-4234-8234-123456789abc".into(),
                "Catalogs/B.xml".into(),
            )],
            ..Plan::default()
        };
        let Outcome::Active(forced) = plan_from_comparison(
            &stage_args(&f),
            &[a.as_path(), b.as_path()],
            &overrides,
            &aliases,
            Some(&source),
            super::super::stage_guard::TargetComparison {
                differences: Vec::new(),
                compared: 2,
                identical: 2,
                seconds: 0.0,
            },
        )
        .unwrap() else {
            panic!("actual forced derivation must remain active");
        };
        assert!(forced.prepares(&f.root, &a));
        assert!(forced.prepares(&f.root, &b));
        assert_eq!(
            forced.forced_units,
            HashSet::from(["catalogs/b.xml".into()])
        );
        assert_eq!(
            forced.forced_ids,
            HashSet::from(["22345678-1234-4234-8234-123456789abc".into()])
        );
        source.require_original_unchanged().unwrap();
    }
}

#[test]
fn original_actual_delta_derivation_failure_has_no_plan_or_output() {
    let f = Fixture::new();
    let a = f.write(
        "Catalogs/A.xml",
        metadata("Catalog", "A", OBJECT, "2.20", ""),
    );
    let source = f.context();
    let make_comparison = || super::super::stage_guard::TargetComparison {
        differences: Vec::new(),
        compared: 1,
        identical: 1,
        seconds: 0.0,
    };
    let aliases = BTreeMap::from([(OBJECT.to_owned(), "pending-name".into())]);
    assert!(
        plan_from_comparison(
            &stage_args(&f),
            &[a.as_path()],
            &Plan::default(),
            &aliases,
            Some(&source),
            make_comparison()
        )
        .is_ok()
    );
    let bad = f.root.join("Missing.xml");
    assert!(
        plan_from_comparison(
            &stage_args(&f),
            &[a.as_path(), bad.as_path()],
            &Plan::default(),
            &aliases,
            Some(&source),
            make_comparison()
        )
        .is_err()
    );
    assert!(!f.top.join("bulk").exists());
    let foreign = Fixture::new();
    assert!(
        plan_from_comparison(
            &stage_args(&foreign),
            &[a.as_path()],
            &Plan::default(),
            &aliases,
            Some(&source),
            make_comparison()
        )
        .is_err()
    );
}

#[test]
fn original_alias_reader_keeps_exact_units_and_legacy_positive() {
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        let a = f.write(
            "Catalogs/A.xml",
            metadata("Catalog", "A", OBJECT, version, ""),
        );
        let b = f.write(
            "Catalogs/B.xml",
            metadata(
                "Catalog",
                "B",
                "22345678-1234-4234-8234-123456789abc",
                version,
                "",
            ),
        );
        let context = f.context();
        let aliases = HashSet::from([OBJECT.to_owned()]);
        let xmls = [a.as_path(), b.as_path()];
        let expected = HashSet::from(["catalogs/a.xml".to_owned()]);
        assert_eq!(
            source_aliased_units(&f.root, &xmls, &aliases, None).unwrap(),
            expected
        );
        assert_eq!(
            source_aliased_units(&f.root, &xmls, &aliases, Some(&context)).unwrap(),
            expected
        );
        assert!(
            source_aliased_units(&f.root, &xmls, &HashSet::new(), Some(&context))
                .unwrap()
                .is_empty()
        );
        context.require_original_unchanged().unwrap();
    }
}

#[test]
fn original_alias_reader_uses_complete_existing_parser_beyond_long_comment_and_root_tag() {
    for version in ["2.20", "2.21"] {
        for root_attribute in [false, true] {
            let f = Fixture::new();
            let body = metadata("Catalog", "A", OBJECT, version, "");
            let long = "p".repeat(16_397);
            let xml = if root_attribute {
                body.replacen(
                    "<MetaDataObject ",
                    &format!("<MetaDataObject retainedComment=\"{long}\" "),
                    1,
                )
            } else {
                body.replacen(
                    "?><MetaDataObject",
                    &format!("?><!--{long}--><MetaDataObject"),
                    1,
                )
            };
            assert!(xml.find(OBJECT).unwrap() > HEAD_BYTES);
            let a = f.write("Catalogs/A.xml", xml);
            let context = f.context();
            assert_eq!(
                source_aliased_units(
                    &f.root,
                    &[a.as_path()],
                    &HashSet::from([OBJECT.to_owned()]),
                    Some(&context)
                )
                .unwrap(),
                HashSet::from(["catalogs/a.xml".to_owned()])
            );
            context.require_original_unchanged().unwrap();
        }
    }
}

#[test]
fn original_alias_rayon_failures_are_errors_not_absent_units() {
    let f = Fixture::new();
    let a = f.write(
        "Catalogs/A.xml",
        metadata("Catalog", "A", OBJECT, "2.20", ""),
    );
    let context = f.context();
    let aliases = HashSet::from([OBJECT.to_owned()]);
    assert_eq!(
        source_aliased_units(&f.root, &[a.as_path()], &aliases, Some(&context))
            .unwrap()
            .len(),
        1
    );
    for bad in [
        f.root.join("Catalogs/Missing.xml"),
        f.root.join("catalogs/A.xml"),
        f.top.join("foreign.xml"),
    ] {
        assert!(
            source_aliased_units(
                &f.root,
                &[a.as_path(), bad.as_path()],
                &aliases,
                Some(&context)
            )
            .is_err()
        );
    }
}

#[cfg(unix)]
#[test]
fn original_alias_retains_returned_identity_but_final_gate_refuses_same_size_drift() {
    let f = Fixture::new();
    let xml = metadata("Catalog", "A", OBJECT, "2.20", "");
    let a = f.write("Catalogs/A.xml", &xml);
    let context = f.context();
    let aliases = HashSet::from([OBJECT.to_owned()]);
    let accepted = source_aliased_units(&f.root, &[a.as_path()], &aliases, Some(&context)).unwrap();
    assert_eq!(accepted, HashSet::from(["catalogs/a.xml".to_owned()]));
    let changed = xml.replace(OBJECT, "32345678-1234-4234-8234-123456789abc");
    assert_eq!(changed.len(), xml.len());
    fs::write(&a, changed).unwrap();
    assert_eq!(
        source_aliased_units(&f.root, &[a.as_path()], &aliases, Some(&context)).unwrap(),
        accepted
    );
    assert!(context.require_original_unchanged().is_err());
    assert!(!f.top.join("scripts").exists());
}
