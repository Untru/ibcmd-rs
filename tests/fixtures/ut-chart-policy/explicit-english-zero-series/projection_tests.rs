// Included only by the proposed moxel unit-test patch. Public source-owned
// CF, never an application/ERP blob. No subprocess, native, SQL or temp writes.

fn explicit_english_source_owned_blob() -> Vec<u8> {
    use sha2::Digest as _;
    let bytes: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ut-chart-policy/explicit-english-zero-series/source.cf"
    ));
    assert_eq!(bytes.len(), 7505);
    assert_eq!(
        format!("{:x}", sha2::Sha256::digest(bytes)),
        "b2fa2d388face5e7281711d3714815cc304e12bd220f7495be15abfa0a613b8b"
    );
    let archive = ibcmd_cf::archive::decode_packed_archive(
        std::io::Cursor::new(bytes),
        ibcmd_core::limits::ResourceLimits::for_input_bytes(bytes.len() as u64),
        ibcmd_core::artifact::StorageProfileId::parse("storage:test").unwrap(),
    )
    .unwrap();
    archive
        .entries()
        .iter()
        .find(|entry| entry.name() == "30000000-0000-4000-8000-000000000002.0")
        .expect("source-owned spreadsheet row")
        .payload()
        .to_vec()
}

#[test]
fn explicit_english_projection_republishes_zero_series_extra_name() {
    let blob = explicit_english_source_owned_blob();
    let refs = BTreeMap::new();
    let stored = try_extract_moxel_spreadsheet_xml(&blob, &refs).unwrap();
    assert!(stored.contains("<d3p1:realSeriesCount>0</d3p1:realSeriesCount>"));
    let english = try_extract_moxel_spreadsheet_xml_with_generated_types_and_policy(
        &blob,
        &refs,
        &refs,
        MoxelChartSeriesAutomaticName::RepublishedInEnglish,
    )
    .unwrap();
    assert_eq!(
        english,
        stored.replacen(
            "<v8:content>Сводная</v8:content>",
            "<v8:content>Pivot</v8:content>",
            1,
        )
    );
}

#[test]
fn explicit_english_projection_keeps_default_and_stored_identical() {
    let blob = explicit_english_source_owned_blob();
    let refs = BTreeMap::new();
    assert_eq!(
        MoxelChartSeriesAutomaticName::host(),
        MoxelChartSeriesAutomaticName::Stored
    );
    assert_eq!(
        try_extract_moxel_spreadsheet_xml(&blob, &refs).unwrap(),
        try_extract_moxel_spreadsheet_xml_with_generated_types_and_policy(
            &blob,
            &refs,
            &refs,
            MoxelChartSeriesAutomaticName::Stored,
        )
        .unwrap(),
    );
}

#[test]
fn explicit_english_projection_leaves_compiler_source_roundtrip_stored() {
    let xml = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ut-chart-policy/wrapper-prototype/tree/CommonTemplates/ChartPolicySeed/Ext/Template.xml"
    ));
    let blob = crate::compiler::bodies::template::compile_evidenced_template(
        crate::compiler::bodies::template::TemplateKind::SpreadsheetDocument,
        crate::compiler::bodies::template::TemplateSource::Spreadsheet {
            xml,
            source: None,
            number_format_hint: None,
        },
    )
    .expect("valid cached-name XML must keep passing the compiler's source guard");
    let roundtrip = try_extract_moxel_spreadsheet_xml(&blob, &BTreeMap::new()).unwrap();
    assert!(roundtrip.contains("<v8:content>Сводная</v8:content>"));
    assert!(!roundtrip.contains("<v8:content>Pivot</v8:content>"));
}

#[test]
fn chart_policy_full_decoder_keeps_default_stored() {
    let blob = explicit_english_source_owned_blob();
    let refs = BTreeMap::new();
    let stored = try_extract_moxel_spreadsheet_xml(&blob, &refs).unwrap();
    let english = try_extract_moxel_spreadsheet_xml_with_generated_types_and_policy(
        &blob,
        &refs,
        &refs,
        MoxelChartSeriesAutomaticName::RepublishedInEnglish,
    )
    .unwrap();
    assert_eq!(
        english,
        stored.replacen(
            "<v8:content>Сводная</v8:content>",
            "<v8:content>Pivot</v8:content>",
            1
        )
    );
    assert_eq!(
        try_extract_moxel_spreadsheet_xml_with_generated_types_and_policy(
            &blob,
            &refs,
            &refs,
            MoxelChartSeriesAutomaticName::Russian
        )
        .unwrap(),
        stored
    );
    assert_eq!(
        try_extract_moxel_spreadsheet_xml(&blob, &refs).unwrap(),
        stored
    );
}
