use super::*;
use crate::mssql::source_original_consumers_tests::{Fixture, OBJECT, metadata};

#[test]
fn original_dbtree_scan_preserves_identity_after_long_metadata_prologue() {
    for version in ["2.20", "2.21"] {
        let f = Fixture::new();
        f.write(
            "Configuration.xml",
            metadata("Configuration", "C", OBJECT, version, ""),
        );
        let xml = metadata(
            "Catalog",
            "A",
            "22345678-1234-4234-8234-123456789abc",
            version,
            "",
        );
        let xml = xml.replacen(
            "?><MetaDataObject",
            &format!("?><!--{}--><MetaDataObject", "x".repeat(16_393)),
            1,
        );
        f.write("Catalogs/A.xml", &xml);
        f.write(
            "CommonModules/M/Ext/Module.bsl",
            "Procedure P()\nEndProcedure\n",
        );
        let context = f.context();
        let result = scan_original(&context, false).unwrap();
        assert_eq!(result.files, 3);
        assert!(result.unreadable.is_empty(), "{:?}", result.unreadable);
        assert_eq!(result.objects.len(), 2);
        assert_eq!(
            result.objects[0],
            TreeObject {
                rel: "Catalogs/A.xml".into(),
                kind: "Catalog".into(),
                uuid: "22345678-1234-4234-8234-123456789abc".into()
            }
        );
        assert_eq!(result.objects[1].kind, "Configuration");
        assert_eq!(result.objects[1].uuid, OBJECT);
        context.require_original_unchanged().unwrap();
    }
}

#[test]
fn original_dbtree_scan_requires_root_and_refuses_current_census_drift() {
    let f = Fixture::new();
    f.write(
        "Catalogs/A.xml",
        metadata("Catalog", "A", OBJECT, "2.20", ""),
    );
    let context = f.context();
    assert_eq!(scan_original(&context, true).unwrap().objects.len(), 1);
    assert!(scan_original(&context, false).is_err());
    f.write("New.bin", b"late member");
    assert!(scan_original(&context, true).is_err());
}
