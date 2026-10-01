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

// Identical streaming serde policy to the private configuration fingerprint:
// existing IR serializers sort FieldId property bags; all other order/presence
// and values remain intact. No census-specific normalization is applied.
fn semantic_sha(value: &impl serde::Serialize) -> String {
    struct Hash(Sha256);
    impl std::io::Write for Hash {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut hash = Hash(Sha256::new());
    serde_json::to_writer(&mut hash, value).unwrap();
    format!("{:x}", hash.0.finalize())
}

fn differences(
    a: &serde_json::Value,
    b: &serde_json::Value,
    path: &str,
    out: &mut Vec<serde_json::Value>,
) {
    if a == b {
        return;
    }
    assert!(out.len() < 65_536);
    match (a, b) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for key in a
                .keys()
                .chain(b.keys())
                .collect::<std::collections::BTreeSet<_>>()
            {
                differences(
                    a.get(key).unwrap_or(&serde_json::Value::Null),
                    b.get(key).unwrap_or(&serde_json::Value::Null),
                    &format!("{path}.{key}"),
                    out,
                );
            }
        }
        (serde_json::Value::Array(a), serde_json::Value::Array(b)) if a.len() == b.len() => {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                differences(a, b, &format!("{path}[{i}]"), out);
            }
        }
        _ => {
            let abbreviated = |value: &serde_json::Value| {
                let text = value.to_string();
                serde_json::json!({"serialized_sha256":sha(text.as_bytes()),"characters":text.chars().count(),"prefix":text.chars().take(160).collect::<String>()})
            };
            out.push(
                serde_json::json!({"path":path,"before":abbreviated(a),"after":abbreviated(b)}),
            );
        }
    }
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

#[test]
#[ignore = "requires genuine BSP native sources; materializes only disposable F-lab sidecars"]
fn genuine_bsp83_all_forms_semantic_digest_census() {
    use formats_xml::registry::Format;
    use morph1c_core::ir::{MetadataObject, ObjectKind, Uuid};
    use morph1c_pipeline::{attach_form_body, write_form_bodies};
    let start = std::time::Instant::now();
    let native = PathBuf::from(std::env::var_os("IBCMD_FORM_NATIVE").unwrap());
    let edt = PathBuf::from(std::env::var_os("IBCMD_FORM_EDT").unwrap());
    let report = PathBuf::from(std::env::var_os("IBCMD_FORM_SEMANTIC_REPORT").unwrap());
    let lab = PathBuf::from(std::env::var_os("IBCMD_FORM_SCRATCH").unwrap());
    let (pictures, picture_bindings) = picture_context(&edt);
    let mut all = vec![];
    files(&native, 0, &mut 0, &mut all);
    all.sort();
    assert_eq!(all.len(), 1108);
    let mut rows = vec![];
    let mut errors = 0;
    let mut semantic_differences = 0;
    let mut pure_asset_differences = 0;
    let mut body_semantic_differences = 0;
    let mut module_bom_only_differences = 0;
    for path in all {
        let form_dir = path.parent().unwrap().parent().unwrap();
        let name = form_dir.file_name().unwrap().to_string_lossy().to_string();
        let anchor = form_dir.parent().unwrap().join(&name).with_extension("xml");
        let mut row = serde_json::json!({"native":path,"native_sha256":sha(&bounded_bytes(&path))});
        let result = (|| -> Result<(), String> {
            let mut source =
                MetadataObject::new(ObjectKind::new("CommonForm"), &name, Uuid([9; 16]));
            with_source_version(Some(version()), || {
                attach_form_body(Format::Designer, "CommonForm", &anchor, &mut source)
            })
            .map_err(|e| format!("native body/sidecar read: {e}"))?;
            if source.form_bodies.len() != 1 {
                return Err("native did not attach exactly one managed body".into());
            }
            resolve_common_picture_transparency(&mut source.form_bodies[0].body, &pictures, false)
                .map_err(|e| format!("native picture context: {e}"))?;
            let scratch = tempfile::tempdir_in(&lab).unwrap();
            let target_anchor = scratch.path().join(&name).join(format!("{name}.mdo"));
            with_roundtrip_target(version(), || {
                write_form_bodies(Format::Edt, &target_anchor, &source)
            })
            .map_err(|e| format!("EDT body/sidecar write: {e}"))?;
            let mut returned =
                MetadataObject::new(ObjectKind::new("CommonForm"), &name, Uuid([9; 16]));
            with_source_version(Some(version()), || {
                attach_form_body(Format::Edt, "CommonForm", &target_anchor, &mut returned)
            })
            .map_err(|e| format!("generated EDT body/sidecar read: {e}"))?;
            if returned.form_bodies.len() != 1 {
                return Err("generated EDT did not attach exactly one managed body".into());
            }
            resolve_common_picture_transparency(&mut returned.form_bodies[0].body, &pictures, true)
                .map_err(|e| format!("EDT picture context: {e}"))?;
            let before = semantic_sha(&source.form_bodies);
            let after = semantic_sha(&returned.form_bodies);
            let body_before = semantic_sha(&source.form_bodies[0].body);
            let body_after = semantic_sha(&returned.form_bodies[0].body);
            row["source_body_semantic_sha256"] = serde_json::json!(body_before);
            row["returned_body_semantic_sha256"] = serde_json::json!(body_after);
            if body_before != body_after {
                body_semantic_differences += 1;
            }
            // Classify the exact observed byte-encoding difference separately;
            // raw module strings and whole digests are never rewritten or masked.
            let bom_only = match (
                &source.form_bodies[0].module,
                &returned.form_bodies[0].module,
            ) {
                (Some(before), Some(after)) => {
                    before.strip_prefix('\u{feff}') == Some(after.as_str())
                }
                _ => false,
            };
            row["module_exact_leading_bom_only_difference"] = serde_json::json!(bom_only);
            if bom_only {
                module_bom_only_differences += 1;
            }
            row["source_semantic_sha256"] = serde_json::json!(before);
            row["returned_semantic_sha256"] = serde_json::json!(after);
            if before != after {
                let mut delta = vec![];
                differences(
                    &serde_json::to_value(&source.form_bodies).unwrap(),
                    &serde_json::to_value(&returned.form_bodies).unwrap(),
                    "forms",
                    &mut delta,
                );
                assert!(!delta.is_empty());
                let assets_only = delta.iter().all(|d| {
                    let p = d["path"].as_str().unwrap();
                    p.contains(".pictures[")
                        || p.contains(".help_resources[")
                        || p.contains(".ordinary_body")
                });
                if assets_only {
                    pure_asset_differences += 1;
                } else {
                    semantic_differences += 1;
                }
                row["result"] = serde_json::json!(if assets_only {
                    "ASSET_DIFFERENCE"
                } else {
                    "SEMANTIC_DIFFERENCE"
                });
                row["differences"] = serde_json::json!(delta);
            } else {
                row["result"] = serde_json::json!("PASS");
            }
            Ok(())
        })();
        if let Err(error) = result {
            errors += 1;
            row["result"] = serde_json::json!("ERROR");
            row["error"] = serde_json::json!(error);
        }
        rows.push(row);
    }
    std::fs::write(report,serde_json::to_vec_pretty(&serde_json::json!({"scope":"all1108 native managed bodies with pipeline sidecars and modules -> disposable EDT -> pipeline read; CommonForm stand-in excludes metadata and owner-level help acceptance; private streaming serde semantic fingerprints, no extra normalization; body and exact module BOM-only differences reported separately","seconds":start.elapsed().as_secs_f64(),"files":rows.len(),"errors":errors,"semantic_differences":semantic_differences,"pure_asset_differences":pure_asset_differences,"body_semantic_differences":body_semantic_differences,"module_bom_only_differences":module_bom_only_differences,"picture_context":picture_bindings,"rows":rows})).unwrap()).unwrap();
    eprintln!(
        "forms=1108 errors={errors} semantic_differences={semantic_differences} pure_asset_differences={pure_asset_differences} seconds={}",
        start.elapsed().as_secs_f64()
    );
    assert_eq!(
        (errors, semantic_differences, pure_asset_differences),
        (0, 0, 0),
        "all differences retained in report"
    );
}

fn dynamic_body() -> FormBody {
    read(FormDialect::Edt, b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\"/></attributes></form:Form>\r\n").unwrap()
}

#[test]
fn exact_empty_settings_only_are_semantically_absent() {
    use morph1c_core::ir::{DcsListSettings, DcsSettingsGroup};
    let mut body = dynamic_body();
    let absent = semantic_sha(&body);
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .list_settings = Some(DcsListSettings::default());
    assert_eq!(semantic_sha(&body), absent);
    assert!(
        body.data_attributes[0]
            .dynamic_list
            .as_ref()
            .unwrap()
            .list_settings
            .is_some()
    );
    let native = String::from_utf8(write(FormDialect::Designer, &body).unwrap()).unwrap();
    assert!(native.contains("<ListSettings/>"));
    for settings in [
        DcsListSettings {
            envelope_without_pal: true,
            ..Default::default()
        },
        DcsListSettings {
            items_view_mode: Some("".into()),
            ..Default::default()
        },
        DcsListSettings {
            items_user_setting_id: Some("id".into()),
            ..Default::default()
        },
        DcsListSettings {
            filter: Some(DcsSettingsGroup {
                view_mode: None,
                user_setting_id: None,
                user_setting_presentation: None,
                items: vec![],
            }),
            ..Default::default()
        },
    ] {
        body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .list_settings = Some(settings);
        assert_ne!(semantic_sha(&body), absent);
    }
}

#[test]
fn form_module_read_strips_only_one_designer_encoding_signature() {
    use formats_xml::registry::Format;
    use morph1c_core::ir::{MetadataObject, ObjectKind, Uuid};
    use morph1c_pipeline::attach_form_body;
    let dir = tempfile::tempdir().unwrap();
    let native = dir.path().join("Witness/Ext");
    std::fs::create_dir_all(native.join("Form")).unwrap();
    std::fs::write(
        native.join("Form.xml"),
        write(FormDialect::Designer, &FormBody::new()).unwrap(),
    )
    .unwrap();
    let anchor = dir.path().join("Witness.xml");
    for text in [
        "\u{feff} \t// source\r\n\u{feff}Interior",
        " \t// bare\r\n",
        "\u{feff}\u{feff}Double",
    ] {
        std::fs::write(native.join("Form/Module.bsl"), text.as_bytes()).unwrap();
        let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "Witness", Uuid([8; 16]));
        with_source_version(Some(version()), || {
            attach_form_body(Format::Designer, "CommonForm", &anchor, &mut obj)
        })
        .unwrap();
        assert_eq!(
            obj.form_bodies[0].module.as_deref(),
            Some(text.strip_prefix('\u{feff}').unwrap_or(text))
        );
        assert_eq!(
            std::fs::read(native.join("Form/Module.bsl")).unwrap(),
            text.as_bytes()
        );
    }
}

#[test]
fn public_empty_settings_and_module_return_exact_and_edits_reject_forged_hash() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use morph1c_core::ir::{
        DcsListSettings, MetadataObject, NamedFormBody, ObjectKind, PropertyValue, Token, Uuid,
    };
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    let mut cfg = read_config(
        Format::Designer,
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0;
    let mut body = dynamic_body();
    body.data_attributes[0]
        .dynamic_list
        .as_mut()
        .unwrap()
        .list_settings = Some(DcsListSettings::default());
    let mut obj = MetadataObject::new(ObjectKind::new("CommonForm"), "Settings", Uuid([77; 16]));
    let form_type = morph1c_core::spec::registry::spec_for("CommonForm")
        .unwrap()
        .fields()
        .iter()
        .find(|f| f.name == "formType")
        .unwrap()
        .id;
    obj.properties
        .push((form_type, PropertyValue::Enum(Token::new("Managed"))));
    obj.form_bodies.push(NamedFormBody {
        name: "Settings".into(),
        body,
        ordinary_body: None,
        module: Some(" // BSL\r\n\u{feff}Interior".into()),
        help: vec![],
        help_resources: vec![],
    });
    cfg.objects.push(obj);
    let dir = tempfile::tempdir().unwrap();
    write_config(Format::Designer, &cfg, dir.path()).unwrap();
    let path = dir.path().join("Configuration.xml");
    let root = String::from_utf8(std::fs::read(&path).unwrap())
        .unwrap()
        .replace(
            "</Language>",
            "</Language>\r\n\t\t\t<CommonForm>Settings</CommonForm>",
        );
    std::fs::write(path, root).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let generated = xml_to_edt(&original, &options).unwrap().tree;
    assert!(
        !generated
            .entries()
            .iter()
            .any(|e| e.path().as_str().ends_with("ListSettings.dcss"))
    );
    assert_eq!(
        edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
            .unwrap()
            .tree,
        original
    );
    for (path, edited) in [
        (
            "src/CommonForms/Settings/Module.bsl",
            b" // edited\r\n".to_vec(),
        ),
        (
            "src/CommonForms/Settings/Attributes/List/ExtInfo/ListSettings.dcss",
            formats_xml::form::write_list_settings_dcss(&DcsListSettings {
                items_view_mode: Some("Normal".into()),
                ..Default::default()
            }),
        ),
    ] {
        let manifest_path = ".ibcmd-provenance/manifest.json";
        let mut manifest: serde_json::Value = serde_json::from_slice(
            generated
                .entries()
                .iter()
                .find(|e| e.path().as_str() == manifest_path)
                .unwrap()
                .bytes(),
        )
        .unwrap();
        manifest["generated"][path] = serde_json::json!(sha(&edited));
        let mut entries = generated
            .entries()
            .iter()
            .filter(|e| e.path().as_str() != path && e.path().as_str() != manifest_path)
            .cloned()
            .collect::<Vec<_>>();
        entries.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), edited).unwrap());
        entries.push(
            SourceEntry::from_bytes(
                SourcePath::new(manifest_path).unwrap(),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap(),
        );
        let changed = SourceTree::new(entries).unwrap();
        assert!(edt_to_xml(&Project::from_tree(changed).unwrap(), &options).is_err());
    }
}
