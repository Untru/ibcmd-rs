//! Metadata records stored in older record versions, read as the platform
//! reads them (it upgrades a record when it loads the configuration). The
//! fixtures (`_onecdec/make_upgrade_fixtures.py`) are a platform-built
//! configuration and derived copies whose records are rewritten into the
//! older shapes the corpora keep; the platform's own dump of every derived
//! copy equals the dump of the original (checked when the fixtures are made).

use crate::common;

use std::collections::BTreeMap;

/// Every file of the platform dump except ConfigDumpInfo.xml, which carries
/// the container's record versions rather than the objects.
fn same_tree(case: &str, input: &str) {
    let dir = common::fixture("upgrade").join(case);
    let out = common::temp_dir(&format!("upgrade-{case}-{input}"));
    let run = common::export(&dir.join(input), &out);
    assert!(
        run.status.success(),
        "{case}/{input}: {}",
        String::from_utf8_lossy(&run.stdout)
    );
    let skip = |files: BTreeMap<String, Vec<u8>>| -> BTreeMap<String, Vec<u8>> {
        files
            .into_iter()
            .filter(|(path, _)| path != "ConfigDumpInfo.xml")
            .collect()
    };
    let expected = skip(common::files(&dir.join("expected")));
    let actual = skip(common::files(&out));
    let differ: Vec<_> = expected
        .keys()
        .chain(actual.keys())
        .filter(|path| expected.get(*path) != actual.get(*path))
        .collect();
    assert!(
        differ.is_empty(),
        "{case}/{input}: {differ:?}\n{}",
        String::from_utf8_lossy(&run.stdout)
    );
}

#[test]
fn an_exchange_plan_with_seven_standard_attributes_is_written() {
    // Plans in 8.3.21 mode and below keep no ExchangeDate: the standard
    // attribute table holds seven entries (every plan of five real
    // configurations in 8.3.21 mode and below).
    same_tree("exchange_plan", "current.cf");
}

#[test]
fn an_exchange_plan_stored_as_36_is_written() {
    same_tree("exchange_plan", "v36.cf");
}

#[test]
fn an_exchange_plan_stored_as_35_is_written_with_the_objects_typed_by_it() {
    // `{35,…}` also hid the plan's reference type from the type index: the
    // constant, session parameter and defined type of that type were lost.
    same_tree("exchange_plan", "v35.cf");
}

#[test]
fn header_blocks_in_the_older_spelling_are_read() {
    // Every header `{1,{0,0,<uuid>},…,0,0}` and the root's section
    // identities `{0,0,<id>}` (a configuration saved in 8.3.9 mode); the
    // platform dumps the file to the same XML.
    same_tree("exchange_plan", "old_headers.cf");
}

#[test]
fn header_blocks_spelled_two_are_read() {
    // Every header `{2,{1,0,<uuid>},…,0,0,<nil>}` (8.3.14 mode).
    same_tree("exchange_plan", "old_headers_2.cf");
}

#[test]
fn a_constant_stored_as_14_is_refused_not_stubbed() {
    // `{14,…}` keeps no ValueKey type ids; the platform derives them on load
    // (the same ids on every load, by a rule not known here), so the file
    // cannot be written. Before, a header-only stub was.
    let dir = common::fixture("upgrade").join("exchange_plan");
    let out = common::temp_dir("upgrade-constant-v14");
    let run = common::export(&dir.join("constant_v14.cf"), &out);
    let report = String::from_utf8_lossy(&run.stdout);
    assert!(run.status.success(), "{report}");
    assert!(!out.join("Constants/КонстантаУзел.xml").exists());
    assert!(
        report.contains("Constants/КонстантаУзел.xml not written"),
        "{report}"
    );
    let expected = common::files(&dir.join("expected"));
    let actual = common::files(&out);
    for (path, bytes) in &expected {
        if path == "ConfigDumpInfo.xml" || path == "Constants/КонстантаУзел.xml" {
            continue;
        }
        assert!(actual.get(path) == Some(bytes), "{path} differs");
    }
}
