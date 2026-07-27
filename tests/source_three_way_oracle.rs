use std::fs;
use std::path::PathBuf;

use ibcmd_rs::{
    cli::SourceThreeWayOracleArgs,
    source_oracle::{SourceOracleAgreement, classify_agreement, run_source_three_way_oracle},
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Fixture {
    schema_version: u32,
    description: String,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    native: String,
    edt: String,
    ours: String,
    agreement: SourceOracleAgreement,
}

#[test]
fn reusable_hash_only_fixture_covers_each_oracle_branch() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/three-way-oracle/classification-fixture.json");
    let fixture: Fixture = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(fixture.schema_version, 1);
    assert!(
        fixture
            .description
            .contains("No application source content")
    );
    assert_eq!(fixture.cases.len(), 5);
    for case in fixture.cases {
        assert_eq!(
            classify_agreement(Some(&case.native), Some(&case.edt), Some(&case.ours)),
            case.agreement
        );
    }
}

#[test]
fn missing_path_hashes_remain_deterministically_classified() {
    assert_eq!(
        classify_agreement(Some("a"), None, Some("a")),
        SourceOracleAgreement::NativeOursNotEdt
    );
    assert_eq!(
        classify_agreement(None, None, None),
        SourceOracleAgreement::AllEqual
    );
}

#[test]
fn offline_run_writes_stable_hash_only_evidence_for_all_branches() {
    let root = std::env::temp_dir().join(format!(
        "ibcmd-rs-three-way-oracle-{}",
        uuid::Uuid::new_v4()
    ));
    let native = root.join("native");
    let edt = root.join("edt");
    let ours = root.join("ours");
    for tree in [&native, &edt, &ours] {
        fs::create_dir_all(tree).unwrap();
    }
    write_case(&native, "all-equal", "same");
    write_case(&edt, "all-equal", "same");
    write_case(&ours, "all-equal", "same");
    write_case(&native, "native-edt", "same");
    write_case(&edt, "native-edt", "same");
    write_case(&ours, "native-edt", "ours");
    write_case(&native, "native-ours", "same");
    write_case(&edt, "native-ours", "edt");
    write_case(&ours, "native-ours", "same");
    write_case(&native, "edt-ours", "native");
    write_case(&edt, "edt-ours", "same");
    write_case(&ours, "edt-ours", "same");
    write_case(&native, "all-different", "native");
    write_case(&edt, "all-different", "edt");
    write_case(&ours, "all-different", "ours");

    let args = SourceThreeWayOracleArgs {
        native,
        edt,
        ours,
        source_version: "synthetic-1".to_string(),
        native_tool_version: "native synthetic".to_string(),
        edt_tool_version: "edt synthetic import/export".to_string(),
        ours_tool_version: "ours synthetic".to_string(),
        max_files: 10,
        max_total_bytes: 1024,
        max_file_bytes: 128,
        output: root.join("report.json"),
        markdown: root.join("report.md"),
    };
    let report = run_source_three_way_oracle(&args).unwrap();
    assert_eq!(report.summary.all_equal, 1);
    assert_eq!(report.summary.native_edt_not_ours, 1);
    assert_eq!(report.summary.native_ours_not_edt, 1);
    assert_eq!(report.summary.edt_ours_not_native, 1);
    assert_eq!(report.summary.all_different, 1);
    let json = fs::read_to_string(&args.output).unwrap();
    let markdown = fs::read_to_string(&args.markdown).unwrap();
    assert!(json.contains("offline_research_only"));
    assert!(markdown.contains("cannot prove a decoder"));
    assert!(run_source_three_way_oracle(&args).is_err());
    let nested_output_args = SourceThreeWayOracleArgs {
        output: args.native.join("must-not-write.json"),
        markdown: root.join("second-report.md"),
        ..args
    };
    assert!(run_source_three_way_oracle(&nested_output_args).is_err());
    fs::remove_dir_all(root).unwrap();
}

fn write_case(root: &std::path::Path, name: &str, content: &str) {
    fs::write(root.join(format!("{name}.txt")), content).unwrap();
}
