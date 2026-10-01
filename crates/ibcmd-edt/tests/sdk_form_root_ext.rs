use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use formats_xml::form::{FormDialect, read_form, resolve_common_picture_transparency, write_form};
use morph1c_core::ir::{FormBody, FormRootExtInfo};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use sha2::{Digest, Sha256};

fn version() -> FormatVersion {
    FormatVersion::new(2, 20)
}
fn read(dialect: FormDialect, bytes: &[u8]) -> Result<FormBody, formats_xml::form::FormError> {
    with_source_version(Some(version()), || read_form(dialect, bytes))
}
fn write(dialect: FormDialect, body: &FormBody) -> Result<Vec<u8>, formats_xml::form::FormError> {
    with_roundtrip_target(version(), || write_form(dialect, body))
}
fn kind(body: &FormBody) -> Option<&str> {
    body.root_ext_info.as_ref().map(|info| info.kind.as_str())
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn unknown_root_ext_info_placeholder_and_child_remain_rejected() {
    let mut body = FormBody::new();
    body.root_ext_info = Some(FormRootExtInfo {
        kind: "form:ObjectFormExtInfo".into(),
        events: vec![],
        user_settings_group: None,
    });
    let bytes = write(FormDialect::Edt, &body).unwrap();
    assert_eq!(kind(&read(FormDialect::Edt, &bytes).unwrap()), kind(&body));
    let text = String::from_utf8(bytes).unwrap();
    for replacement in ["form:ExchangePlanObjectFormExtInfo?", "form:UnknownExtInfo"] {
        let bad = text.replace("form:ObjectFormExtInfo", replacement);
        assert!(read(FormDialect::Edt, bad.as_bytes()).is_err());
    }
    let bad = text.replace(
        "<extInfo xsi:type=\"form:ObjectFormExtInfo\"/>",
        "<extInfo xsi:type=\"form:ObjectFormExtInfo\"><unknown/></extInfo>",
    );
    assert_ne!(bad, text);
    assert!(read(FormDialect::Edt, bad.as_bytes()).is_err());
}

fn files(root: &Path, depth: usize, count: &mut usize, found: &mut Vec<PathBuf>) {
    assert!(depth <= 64);
    for item in std::fs::read_dir(root).unwrap() {
        let item = item.unwrap();
        *count += 1;
        assert!(*count <= 524_288);
        let ty = item.file_type().unwrap();
        assert!(!ty.is_symlink());
        if ty.is_dir() {
            files(&item.path(), depth + 1, count, found);
        } else if item.file_name() == "Form.xml" {
            assert_eq!(root.file_name().unwrap(), "Ext");
            found.push(item.path());
        }
    }
}

fn bounded_bytes(path: &Path) -> Vec<u8> {
    assert!(std::fs::metadata(path).unwrap().len() <= 32 * 1024 * 1024);
    std::fs::read(path).unwrap()
}

// Descriptor-derived BOOL context is necessary for writing referenced pictures;
// this never derives a per-use transparent pixel or mutates native source bytes.
fn picture_context(edt: &Path) -> (BTreeMap<String, bool>, Vec<serde_json::Value>) {
    let mut map = BTreeMap::new();
    let mut bindings = Vec::new();
    for item in std::fs::read_dir(edt.join("CommonPictures")).unwrap() {
        let item = item.unwrap();
        assert!(item.file_type().unwrap().is_dir());
        let name = item.file_name();
        let path = item.path().join(name).with_extension("mdo");
        let bytes = bounded_bytes(&path);
        let root = formats_xml::parse(&bytes).unwrap();
        let transparent = root.root.child("transparentPixel").is_some();
        assert!(
            map.insert(
                format!("CommonPicture.{}", item.file_name().to_string_lossy()),
                transparent
            )
            .is_none()
        );
        bindings.push(serde_json::json!({"path":path,"sha256":sha(&bytes),"transparentPixel_present":transparent}));
    }
    (map, bindings)
}

#[test]
#[ignore = "requires genuine BSP native, authentic EDT and SDK sources in F lab"]
fn genuine_bsp83_native_to_edt_to_read_all_forms() {
    let start = std::time::Instant::now();
    let native = PathBuf::from(std::env::var_os("IBCMD_FORM_NATIVE").unwrap());
    let edt = PathBuf::from(std::env::var_os("IBCMD_FORM_EDT").unwrap());
    let sdk = PathBuf::from(std::env::var_os("IBCMD_FORM_SDK").unwrap());
    let report = PathBuf::from(std::env::var_os("IBCMD_FORM_REPORT").unwrap());
    let (pictures, picture_bindings) = picture_context(&edt);
    let mut all = vec![];
    files(&native, 0, &mut 0, &mut all);
    all.sort();
    assert_eq!(all.len(), 1108, "no corpus forms may be silently omitted");
    let mut rows = vec![];
    let mut failures = 0;
    let mut kind_pairs = BTreeMap::<String, usize>::new();
    for path in all {
        let rel = path.strip_prefix(&native).unwrap();
        let owner = rel.parent().unwrap().parent().unwrap();
        let edt_path = edt.join(owner).join("Form.form");
        let sdk_path = sdk.join(rel);
        let source = bounded_bytes(&path);
        let authentic = bounded_bytes(&edt_path);
        let sdk_bytes = bounded_bytes(&sdk_path);
        let mut row = serde_json::json!({"native":path,"native_sha256":sha(&source),"authentic_edt":edt_path,"authentic_edt_sha256":sha(&authentic),"native_sdk":sdk_path,"native_sdk_sha256":sha(&sdk_bytes)});
        let result = (|| -> Result<(), String> {
            let mut body =
                read(FormDialect::Designer, &source).map_err(|e| format!("native read: {e}"))?;
            let actual = read(FormDialect::Edt, &authentic)
                .map_err(|e| format!("authentic EDT read: {e}"))?;
            row["main_type_heads"] = serde_json::json!(
                body.data_attributes
                    .iter()
                    .filter(|a| a.main)
                    .flat_map(|a| a.value_type.iter())
                    .flat_map(|t| t.parts.iter())
                    .map(|t| t.id.split('.').next().unwrap())
                    .collect::<Vec<_>>()
            );
            row["inferred_root_kind"] = serde_json::json!(kind(&body));
            row["authentic_root_kind"] = serde_json::json!(kind(&actual));
            *kind_pairs
                .entry(format!("{:?} -> {:?}", kind(&body), kind(&actual)))
                .or_default() += 1;
            if kind(&body) != kind(&actual) {
                return Err("root kind differs from authentic EDT".into());
            }
            resolve_common_picture_transparency(&mut body, &pictures, false)
                .map_err(|e| format!("picture context: {e}"))?;
            let generated =
                write(FormDialect::Edt, &body).map_err(|e| format!("EDT write: {e}"))?;
            row["generated_sha256"] = serde_json::json!(sha(&generated));
            let regenerated = read(FormDialect::Edt, &generated)
                .map_err(|e| format!("generated EDT read: {e}"))?;
            if kind(&regenerated) != kind(&body) {
                return Err("generated root kind changed".into());
            }
            // The independent SDK witness must also decode to the same root kind.
            let sdk_body =
                read(FormDialect::Designer, &sdk_bytes).map_err(|e| format!("SDK read: {e}"))?;
            if kind(&sdk_body) != kind(&actual) {
                return Err("SDK inferred root differs from authentic EDT".into());
            }
            Ok(())
        })();
        match result {
            Ok(()) => row["result"] = serde_json::json!("PASS"),
            Err(error) => {
                failures += 1;
                row["result"] = serde_json::json!("FAIL");
                row["error"] = serde_json::json!(error);
            }
        }
        rows.push(row);
    }
    std::fs::write(report, serde_json::to_vec_pretty(&serde_json::json!({"scope":"1108 BSP83 managed native form bodies -> EDT writer -> EDT reader; root kinds paired against authentic EDT and fresh native SDK, not whole configuration acceptance","seconds":start.elapsed().as_secs_f64(),"files":rows.len(),"failures":failures,"kind_pairs":kind_pairs,"picture_context":picture_bindings,"rows":rows})).unwrap()).unwrap();
    eprintln!(
        "forms=1108 failures={failures} seconds={}",
        start.elapsed().as_secs_f64()
    );
    assert_eq!(failures, 0, "all failures retained in report");
}

#[test]
#[ignore = "requires genuine ExchangePlan, CoA and CoCT form pairs in F lab"]
fn genuine_object_root_kinds_roundtrip_without_placeholder() {
    let native = PathBuf::from(std::env::var_os("IBCMD_FORM_NATIVE").unwrap());
    let edt = PathBuf::from(std::env::var_os("IBCMD_FORM_EDT").unwrap());
    for relative in [
        "ExchangePlans/_ДемоАвтономнаяРабота/Forms/ФормаУзла",
        "ChartsOfAccounts/_ДемоОсновной/Forms/ФормаСчета",
        "ChartsOfCalculationTypes/_ДемоОсновныеНачисления/Forms/ФормаЭлемента",
    ] {
        let source = bounded_bytes(&native.join(relative).join("Ext/Form.xml"));
        let authentic = bounded_bytes(&edt.join(relative).join("Form.form"));
        let mut body = read(FormDialect::Designer, &source).unwrap();
        let actual = read(FormDialect::Edt, &authentic).unwrap();
        assert_eq!(kind(&body), Some("form:ObjectFormExtInfo"));
        assert_eq!(kind(&actual), kind(&body));
        assert_eq!(write(FormDialect::Designer, &body).unwrap(), source);
        if relative.starts_with("ExchangePlans/") {
            let original = String::from_utf8(source.clone()).unwrap();
            let unmodelled = original.replace("ExchangePlanObject.", "ExchangePlanRef.");
            assert_ne!(unmodelled, original);
            let unknown = read(FormDialect::Designer, unmodelled.as_bytes()).unwrap();
            assert_eq!(kind(&unknown), Some("form:ExchangePlanRefFormExtInfo?"));
            let generated = write(FormDialect::Edt, &unknown).unwrap();
            assert!(read(FormDialect::Edt, &generated).is_err());
        }
        let (pictures, _) = picture_context(&edt);
        resolve_common_picture_transparency(&mut body, &pictures, false).unwrap();
        let generated = write(FormDialect::Edt, &body).unwrap();
        assert_eq!(
            kind(&read(FormDialect::Edt, &generated).unwrap()),
            kind(&actual)
        );
        let bad = String::from_utf8(generated).unwrap().replace(
            "form:ObjectFormExtInfo",
            "form:ExchangePlanObjectFormExtInfo?",
        );
        assert!(read(FormDialect::Edt, bad.as_bytes()).is_err());
    }
}
