//! Opt-in read-only typed form census. All bounded snapshots and reports live in F lab.
use formats_xml::form::{FormDialect, read_form};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
fn files(root: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let kind = entry.file_type().unwrap();
        assert!(!kind.is_symlink(), "laboratory census rejects symlinks");
        if kind.is_dir() {
            files(&entry.path(), out);
        } else if entry.path().extension().is_some_and(|e| e == "form")
            || entry.file_name() == "Form.xml"
        {
            out.push(entry.path());
        }
    }
}
#[test]
#[ignore = "requires authentic corpus IBCMD_EDT_CENSUS_ROOT and F laboratory"]
fn all_authentic_edt_forms_have_typed_coverage() {
    let root = PathBuf::from(std::env::var_os("IBCMD_EDT_CENSUS_ROOT").unwrap());
    let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let dialect = if std::env::var("IBCMD_EDT_CENSUS_DIALECT").unwrap_or_default() == "xml" {
        FormDialect::Designer
    } else {
        FormDialect::Edt
    };
    let mut paths = Vec::new();
    files(&root, &mut paths);
    paths.sort();
    assert!(!paths.is_empty());
    let mut failures = BTreeMap::<String, Vec<String>>::new();
    let mut passed = 0;
    let mut parameters = 0;
    for path in &paths {
        let result = (|| -> Result<_, String> {
            let snapshot = tempfile::tempdir_in(&lab).map_err(|e| e.to_string())?;
            std::fs::copy(
                path,
                snapshot.path().join(if dialect == FormDialect::Edt {
                    "Form.form"
                } else {
                    "Form.xml"
                }),
            )
            .map_err(|e| e.to_string())?;
            let limits = ibcmd_edt::ReaderLimits {
                files: 1,
                directories: 1,
                depth: 2,
                asset_bytes: 256 * 1024 * 1024,
                total_bytes: 256 * 1024 * 1024,
            };
            let tree =
                ibcmd_edt::read_xml_source(snapshot.path(), limits).map_err(|e| e.to_string())?;
            read_form(dialect, tree.entries()[0].bytes()).map_err(|e| e.to_string())
        })();
        match result {
            Ok(body) => {
                passed += 1;
                parameters += body
                    .form_ci_navigation_panel
                    .iter()
                    .chain(&body.form_ci_command_bar)
                    .filter(|item| item.command_parameter.is_some())
                    .count();
            }
            Err(reason) => failures.entry(reason).or_default().push(
                path.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            ),
        }
    }
    let report = serde_json::json!({"root":root, "dialect":format!("{dialect:?}"),"files":paths.len(),"passed":passed,"command_parameters":parameters,"failure_kinds":failures.len(),"failures":failures});
    std::fs::write(
        lab.join(if dialect == FormDialect::Edt {
            "form-census-edt.json"
        } else {
            "form-census-xml.json"
        }),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    eprintln!(
        "forms={} passed={} failure_kinds={} command_parameters={}",
        paths.len(),
        passed,
        failures.len(),
        parameters
    );
    assert!(
        failures.is_empty(),
        "see F laboratory form-census.json for all failures"
    );
}
