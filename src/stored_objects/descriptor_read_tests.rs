//! Version custody of the exact conversion used for a requested descriptor.
use super::*;
#[path = "../../tests/common/mcp_export_fixture.rs"]
mod fixture;
use fixture::{Case, Scenario};
#[test]
fn changed_versions_during_descriptor_conversion_refuse_then_retry_fresh() {
    if crate::test_process::isolated_engine_test(
        "stored_objects::descriptor_read_tests::changed_versions_during_descriptor_conversion_refuse_then_retry_fresh",
    ) {
        return;
    }
    const PATH: &str = "Catalogs/CorpusList/Forms/ListForm.xml";
    for rows_dir in [false, true] {
        let case = Case::new(Scenario::NestedInvalid, rows_dir);
        let mut view = StoredConfiguration::new(
            if rows_dir {
                ConfigSource::RowsDir(case.rows.clone())
            } else {
                ConfigSource::FileDb(case.database.clone())
            },
            crate::platform::parse("8.5.1").unwrap(),
        );
        let changed = std::cell::Cell::new(false);
        let _engine = engine();
        let result = view.export_with_descriptor_check_unlocked(
            &["Catalog.CorpusList".into()],
            &|done, _, _| {
                if done == 0 && !changed.replace(true) {
                    // Change AFTER inventory resolution, BEFORE the renderer reads.
                    // The rendered value is valid, but belongs to a different digest.
                    case.nested_value(crate::metadata_model::brace::Brace::flag(false));
                }
            },
            false,
            Some(PATH),
        );
        assert!(changed.get());
        assert!(
            result.is_err(),
            "a valid conversion must not be certified for the earlier versions digest; rows_dir={rows_dir}"
        );
        assert!(
            view.exports.is_empty(),
            "no changed-snapshot proof may enter the cache"
        );
        drop(_engine);
        let (fresh, path) = view.read_file(PATH).unwrap();
        assert!(
            String::from_utf8_lossy(&fresh.files[&path])
                .contains("<IncludeHelpInContents>false</IncludeHelpInContents>")
        );
        let (cached, _) = view.read_file(PATH).unwrap();
        assert!(
            Arc::ptr_eq(&fresh, &cached),
            "the proven retry remains cached"
        );
    }
}
