use super::*;

/// `cfg:AnyIBRef` — единственный witnessed `cfg:`-алиас с local-name'ом, отличным от канона.
#[test]
fn maps_the_designer_config_alias_to_its_canon() {
    assert_eq!(canon_for_config_local("AnyIBRef"), Some("AnyRef"));
    // Канон уже канонический / local == canon / не платформенный → без подмены.
    assert_eq!(canon_for_config_local("AnyRef"), None);
    assert_eq!(canon_for_config_local("ConstantsSet"), None);
    assert_eq!(canon_for_config_local("ReportBuilder"), None);
    assert_eq!(canon_for_config_local("CatalogRef.Товары"), None);
    // Алиасы с ДРУГИМ префиксом (v8:/dcsset:) сюда не попадают.
    assert_eq!(canon_for_config_local("ValueListType"), None);
    assert_eq!(canon_for_config_local("SettingsComposer"), None);
}
