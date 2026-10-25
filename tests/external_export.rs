//! `cf export` of external data processors (.epf) and reports (.erf): the
//! output tree must equal the platform's own dump byte for byte. Fixtures are
//! clean-room objects built by 8.3.27.2214 (`_onecdec/make_fixtures.py`).

use crate::common;

use common::{assert_tree_eq, export, fixture, temp_dir};

#[test]
fn external_data_processor_matches_native_dump() {
    let dir = fixture("test_processor");
    let out = temp_dir("epf");
    let run = export(&dir.join("input.epf"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_tree_eq(&dir.join("expected"), &out);
}

#[test]
fn external_report_matches_native_dump() {
    let dir = fixture("test_report");
    let out = temp_dir("erf");
    let run = export(&dir.join("input.erf"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_tree_eq(&dir.join("expected"), &out);
}

fn assert_exports_to(input: &std::path::Path, expected: &std::path::Path, tag: &str) {
    let out = temp_dir(tag);
    let run = export(input, &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_tree_eq(expected, &out);
}

#[test]
fn object_module_with_bom_only_text_matches_native_dump() {
    let dir = fixture("variants/bom_only_module");
    assert_exports_to(&dir.join("input.epf"), &dir.join("expected"), "bom-only");
}

#[test]
fn processor_without_object_module_matches_native_dump() {
    let dir = fixture("variants/no_module");
    assert_exports_to(&dir.join("input.epf"), &dir.join("expected"), "no-module");
}

/// The platform itself never writes an external object without `copyinfo`
/// (and renames such an object to `<Name>0` when it reads one); the export
/// must still work and leave types as the pipeline resolves them.
#[test]
fn processor_without_copyinfo_exports_like_the_original() {
    assert_exports_to(
        &fixture("variants/no_copyinfo").join("input.epf"),
        &fixture("test_processor").join("expected"),
        "no-copyinfo",
    );
}

#[test]
fn export_into_an_existing_directory_with_overwrite() {
    let dir = fixture("test_processor");
    let out = temp_dir("overwrite");
    for _ in 0..2 {
        let run = export(&dir.join("input.epf"), &out);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    assert_tree_eq(&dir.join("expected"), &out);
}

#[test]
fn a_processor_named_like_the_export_layout_matches_native_dump() {
    // `DataProcessors` is the pipeline's own folder for data processors: the
    // move out of it cleared the whole export. `ConfigDumpInfo.xml` is the
    // file the adapter removes: it removed the root XML just written.
    // Fixtures: `_onecdec/make_named_external_fixtures.py`.
    for name in ["DataProcessors", "ConfigDumpInfo"] {
        let dir = fixture(&format!("named_{name}"));
        assert_exports_to(
            &dir.join("input.epf"),
            &dir.join("expected"),
            &format!("named-{name}"),
        );
    }
}
