//! The exporter's unit tests that call root-only code, apart from its own
//! tests so that they stay in the root when the exporter moves into a crate
//! (#559):
//! - `compatibility_tests` re-runs itself alone in a child process through
//!   `mssql_platform_profile`'s test helper, which runs the root's test
//!   binary (formerly in `model_export.rs`);
//! - `extension_flag_tests` reads an extension role through both root
//!   callers of the rights codec, the exporter's `parse_role_rights_blob`
//!   and the root's own `compiler::bodies::rights::decode_role_rights`
//!   (formerly in `role_rights.rs`);
//! - the research-corpus lab tests compare the spreadsheet XML with the lab's
//!   baseline through `plan::explain_source_diff_file` (formerly in
//!   `tests.rs`).

#[cfg(feature = "research-corpus-tests")]
use std::collections::BTreeMap;

// What `extension_flag_tests` reads as `super::…` in `role_rights.rs`.
use super::role_rights::parse_role_rights_blob;

#[cfg(feature = "research-corpus-tests")]
use ibcmd_moxel::format_moxel_spreadsheet_xml;

#[cfg(feature = "research-corpus-tests")]
use super::parse_moxel_spreadsheet_text;

mod compatibility_tests {
    use super::super::model_export::configuration_compatibility_from_storage;
    use crate::sql::{
        Dbms, ScriptVariables, SqlClient, SqlLogin, SqlParam, SqlRow, SqlTarget, SqlValue,
    };
    use anyhow::Result;
    use std::collections::VecDeque;
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    struct StageClient {
        replies: Mutex<VecDeque<Vec<SqlRow>>>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl SqlClient for StageClient {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            self.calls.lock().unwrap().push(query.to_owned());
            for row in self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected SQL read")
            {
                each(row)?;
            }
            Ok(())
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("read-only probe")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("read-only probe")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("unexpected JSON query")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("read-only probe")
        }
    }

    #[test]
    fn compatibility_probe_reads_a_completed_stage_over_empty_config() {
        // Other unit tests temporarily install process-wide offline rows.
        // The shared helper also proves the selected child actually ran.
        if crate::mssql_platform_profile::tests::run_isolated_profile_test(
            "mssql_dump::root_tests::compatibility_tests::compatibility_probe_reads_a_completed_stage_over_empty_config",
        ) {
            return;
        }
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase/tiny-source");
        let path = root.join("Configuration.xml");
        let xml = fs::read(&path).unwrap();
        let facts = ibcmd_metadata_model::metadata_model::root::configuration_facts(&xml).unwrap();
        let context =
            ibcmd_metadata_model::metadata_model::DescriptorContext::new(&root, "2.21").unwrap();
        let packed = |bytes: Vec<u8>| ibcmd_module_blob::deflate_raw(&bytes).unwrap();
        let descriptor = packed(
            ibcmd_metadata_model::metadata_model::compile_descriptor(
                "Configuration",
                &path,
                &xml,
                &context,
            )
            .unwrap(),
        );
        let root_row = packed(ibcmd_metadata_model::metadata_model::root::root_row(&facts));
        let versions = packed(ibcmd_metadata_model::metadata_model::root::versions_row(
            &[facts.uuid.clone()],
            || "10000000-0000-4000-8000-000000000001".to_owned(),
        ));
        let header = |name: &str, size: usize| SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text(name.to_owned()),
                SqlValue::Int(0),
                SqlValue::Int(size as i64),
            ],
        };
        let binary = |name: &str, bytes: Vec<u8>| SqlRow {
            result_set: 0,
            values: vec![
                SqlValue::Text(name.to_owned()),
                SqlValue::Int(0),
                SqlValue::Int(bytes.len() as i64),
                SqlValue::Binary(bytes),
            ],
        };
        let calls = Arc::new(Mutex::new(Vec::new()));
        let client = StageClient {
            calls: Arc::clone(&calls),
            replies: Mutex::new(VecDeque::from([
                vec![],
                vec![
                    header("root", root_row.len()),
                    header(&facts.uuid, descriptor.len()),
                    header("versions", versions.len()),
                ],
                vec![binary("versions", versions)],
                vec![binary("root", root_row)],
                vec![binary(&facts.uuid, descriptor)],
            ])),
        };
        let sql = crate::sql::SqlExec::with_client(
            SqlTarget {
                server: "memory-only".to_owned(),
                database: None,
                login: SqlLogin::Integrated,
                trust_server_certificate: false,
            },
            Box::new(client),
        );
        assert!(
            configuration_compatibility_from_storage(&sql, "compatibility-stage-test", true)
                .unwrap()
        );
        let calls = calls.lock().unwrap();
        assert!(
            calls
                .iter()
                .skip(1)
                .all(|query| query.contains("[ConfigSave]")),
            "{calls:?}"
        );
    }
}

mod extension_flag_tests {
    use std::collections::BTreeMap;
    use std::time::{Duration, Instant};

    use crate::compiler::bodies::rights::{
        RightsCodecProfile, RoleRightsSummary, decode_role_rights,
    };
    use crate::mssql_dump::extension::{ExtensionContext, activate};
    use ibcmd_core::artifact::ProfileId;

    /// The rights codec reads a root flag stored as `2` (an extension role's
    /// defaulted `false`) as `false` only when its caller says an extension's
    /// export is running. Both root callers -- the export adapter's
    /// `parse_role_rights_blob` and the public `decode_role_rights` -- take
    /// that from the process-wide extension state: with an extension active,
    /// both read the body. (The inactive side is the codec's own test in
    /// `ibcmd-compiler`; here another test may hold the process-wide state.)
    #[test]
    fn both_root_callers_read_a_root_flag_of_2_while_an_extension_is_active() {
        let plain = b"\xef\xbb\xbf{10,\r\n{0},\r\n{0},2,1,2,4294967295}";
        let blob = crate::module_blob::deflate_raw(plain).unwrap();
        let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
        let profile = RightsCodecProfile::from_effective(
            registry
                .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
                .unwrap(),
        )
        .unwrap();

        // Other tests activate the process-wide context too; wait for it.
        let started = Instant::now();
        let guard = loop {
            match activate(ExtensionContext::default()) {
                Ok(guard) => break guard,
                Err(error) if started.elapsed() > Duration::from_secs(120) => {
                    panic!("{error:#}")
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        };
        let parsed = super::parse_role_rights_blob(&blob, &BTreeMap::new(), &BTreeMap::new());
        let decoded = decode_role_rights(&profile, &blob);
        drop(guard);

        let parsed = parsed.expect("the export adapter reads the extension role");
        assert!(!parsed.set_for_new_objects);
        assert!(!parsed.independent_rights_of_child_objects);
        assert_eq!(
            *decoded.unwrap().summary(),
            RoleRightsSummary {
                object_count: 0,
                restriction_template_count: 0,
                set_for_new_objects: false,
                set_for_attributes_by_default: true,
                independent_rights_of_child_objects: false,
            }
        );
    }
}

#[cfg(feature = "research-corpus-tests")]
fn assert_moxel_template_matches_lab_baseline(
    raw: &str,
    baseline_relative_path: &[&str],
    label: &str,
) {
    let raw = if raw.trim_start().starts_with("{8,") {
        raw
    } else {
        let body_start = raw.find("{8,").unwrap();
        &raw[body_start..]
    };
    let spreadsheet = parse_moxel_spreadsheet_text(raw, &BTreeMap::new()).unwrap();
    let xml = format_moxel_spreadsheet_xml(&spreadsheet);
    let baseline_path = baseline_relative_path.iter().fold(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ut_ibcmd_20260629_164647")
            .join("ibcmd"),
        |path, segment| path.join(segment),
    );
    let baseline = std::fs::read_to_string(baseline_path).unwrap();
    let temp_root =
        std::env::temp_dir().join(format!("ibcmd-rs-moxel-{label}-{}", std::process::id()));
    let left_root = temp_root.join("left");
    let right_root = temp_root.join("right");
    let rel_path = std::path::Path::new("Template.xml");
    std::fs::create_dir_all(&left_root).unwrap();
    std::fs::create_dir_all(&right_root).unwrap();
    std::fs::write(
        left_root.join(rel_path),
        baseline.trim_start_matches('\u{feff}'),
    )
    .unwrap();
    std::fs::write(
        right_root.join(rel_path),
        xml.trim_start_matches('\u{feff}'),
    )
    .unwrap();
    let report = crate::plan::explain_source_diff_file(
        &left_root,
        &right_root,
        "Template.xml",
        &[],
        Some(20),
    )
    .unwrap();
    if report.summary.total_differences != 0 {
        panic!(
            "{label} diff summary: left_only={} right_only={} value_or_attr_diff={} first_diffs={:?}",
            report.summary.left_only,
            report.summary.right_only,
            report.summary.value_or_attr_diff,
            report.differences
        );
    }
    let baseline = baseline.trim_start_matches('\u{feff}');
    let xml = xml.trim_start_matches('\u{feff}');
    if baseline != xml {
        let mismatch = baseline
            .bytes()
            .zip(xml.bytes())
            .position(|(left, right)| left != right)
            .unwrap_or_else(|| baseline.len().min(xml.len()));
        panic!(
            "{label} byte mismatch at {} left_len={} right_len={} left={:?} right={:?}",
            mismatch,
            baseline.len(),
            xml.len(),
            &baseline[mismatch.saturating_sub(40)..baseline.len().min(mismatch + 40)],
            &xml[mismatch.saturating_sub(40)..xml.len().min(mismatch + 40)]
        );
    }
}

#[cfg(feature = "research-corpus-tests")]
#[test]
#[ignore = "requires external research corpus"]
fn formats_moxel_invoice_1096_matches_lab_baseline_template_xml() {
    let raw = include_str!("../../tests/fixtures/moxel_invoice_1096_raw.txt");
    assert_moxel_template_matches_lab_baseline(
        raw,
        &[
            "DataProcessors",
            "ПечатьОбщихФорм",
            "Templates",
            "ПФ_MXL_СчетФактура1096_ru",
            "Ext",
            "Template.xml",
        ],
        "1096",
    );
}

#[cfg(feature = "research-corpus-tests")]
#[test]
#[ignore = "requires external research corpus"]
fn formats_moxel_ukd_info_buyer_2020_matches_lab_baseline_template_xml() {
    let raw = include_str!("../../tests/fixtures/moxel_ukd_info_buyer_2020_raw.txt");
    assert_moxel_template_matches_lab_baseline(
        raw,
        &[
            "DataProcessors",
            "ОбменСКонтрагентами",
            "Templates",
            "УКД_ИнформацияПокупателя_2020",
            "Ext",
            "Template.xml",
        ],
        "ukd_info_buyer_2020",
    );
}

#[cfg(feature = "research-corpus-tests")]
#[test]
#[ignore = "requires external research corpus"]
fn formats_moxel_contract_parameter_map_matches_lab_baseline_template_xml() {
    let raw = include_str!("../../tests/fixtures/moxel_contract_parameter_map_raw.txt");
    assert_moxel_template_matches_lab_baseline(
        raw,
        &[
            "DataProcessors",
            "ФорматДоговорнойДокумент101XML",
            "Templates",
            "СоответствияПараметровИМеток",
            "Ext",
            "Template.xml",
        ],
        "contract_parameter_map",
    );
}

#[cfg(feature = "research-corpus-tests")]
#[test]
#[ignore = "requires external research corpus"]
fn formats_moxel_contract_typical_names_matches_lab_baseline_template_xml() {
    let raw = include_str!("../../tests/fixtures/moxel_contract_typical_names_raw.txt");
    assert_moxel_template_matches_lab_baseline(
        raw,
        &[
            "DataProcessors",
            "ФорматДоговорнойДокумент101XML",
            "Templates",
            "ПереченьТиповыхНаименованийЭлементовДоговоров",
            "Ext",
            "Template.xml",
        ],
        "contract_typical_names",
    );
}

#[cfg(feature = "research-corpus-tests")]
#[test]
#[ignore = "requires external research corpus"]
fn formats_moxel_eis_appendix_matches_lab_baseline_template_xml() {
    let raw = include_str!("../../tests/fixtures/moxel_eis_appendix_raw.txt");
    assert_moxel_template_matches_lab_baseline(
        raw,
        &[
            "DataProcessors",
            "ЭлектронноеАктированиеЕИС",
            "Templates",
            "ЭД_ПриложениеЕИС_ru",
            "Ext",
            "Template.xml",
        ],
        "eis_appendix",
    );
}

#[cfg(feature = "research-corpus-tests")]
#[test]
#[ignore = "requires external research corpus"]
fn debug_moxel_card_matches_lab_baseline() {
    let out_dir = std::fs::read_to_string(".tmp_selected_dp_outdir.txt").unwrap();
    let out_dir = out_dir.trim();
    let raw_path = format!(
        "{out_dir}\\ibcmd_rs_dump\\Config_inflated\\018b2a34-2af6-49b1-85a2-d1e7fb95bfe0.0__part0.txt"
    );
    let raw = std::fs::read_to_string(raw_path).unwrap();
    assert_moxel_template_matches_lab_baseline(
        &raw,
        &[
            "DataProcessors",
            "РаботаСНоменклатурой",
            "Templates",
            "ПФ_MXL_КарточкаНоменклатуры",
            "Ext",
            "Template.xml",
        ],
        "card_nomenclature",
    );
}
