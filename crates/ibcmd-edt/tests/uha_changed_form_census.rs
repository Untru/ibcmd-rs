//! Selected genuine failed-form census through production attachment/write APIs.
//! Exact SDK bytes remain authoritative; ordered content is never normalized.
//! Only the explicitly bound taxonomy forms, metadata owners and current picture
//! descriptors are read. No whole configuration conversion or SDK invocation.
use formats_xml::form::{
    FormProjectionContext, bind_picture_semantics, resolve_common_picture_transparency,
};
use morph1c_core::ir::{Configuration, MetadataObject};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use morph1c_pipeline::{
    Format, FormatRegistry, attach_form_body, layout, write_form_bodies_with_context,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn regular(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("reparse source".into());
        }
    }
    if metadata.file_type().is_symlink() {
        return Err("linked source".into());
    }
    Ok(metadata)
}
fn file_sha(path: &Path) -> Result<(String, u64), String> {
    if !regular(path)?.is_file() {
        return Err("expected regular file".into());
    }
    let mut input = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let mut digest = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
        count = count.checked_add(n as u64).ok_or("byte count overflow")?;
    }
    Ok((format!("{:x}", digest.finalize()), count))
}
fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut result = Vec::new();
    let mut identities = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        if !regular(&directory)?.is_dir() {
            return Err("expected regular directory".into());
        }
        for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("non UTF-8 source path")?
                .replace('\\', "/");
            ibcmd_xml::source_tree::validate_source_path_safety(&relative)
                .map_err(|e| e.to_string())?;
            if !identities.insert(relative.to_lowercase()) {
                return Err("case-colliding source".into());
            }
            let metadata = regular(&path)?;
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                result.push(path);
            } else {
                return Err("special source entry".into());
            }
        }
    }
    result.sort();
    Ok(result)
}
fn snapshot(root: &Path) -> Result<Vec<Value>, String> {
    files(root)?
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .ok_or("non UTF-8 path")?
                .replace('\\', "/");
            let (digest, bytes) = file_sha(&path)?;
            Ok(json!({"path_sha256":sha(relative.as_bytes()),"sha256":digest,"bytes":bytes}))
        })
        .collect()
}
fn semantic_sha(value: &impl Serialize) -> Result<String, String> {
    struct Hash(Sha256);
    impl Write for Hash {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Hash(Sha256::new());
    serde_json::to_writer(&mut output, value).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", output.0.finalize()))
}
fn append(output: &mut File, row: &Value) {
    serde_json::to_writer(&mut *output, row).unwrap();
    output.write_all(b"\n").unwrap();
    output.flush().unwrap();
}
fn fresh_json(path: &Path, row: &Value) {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    serde_json::to_writer_pretty(&mut output, row).unwrap();
    output.write_all(b"\n").unwrap();
}
fn picture_context(
    root: &Path,
    version: FormatVersion,
) -> Result<(BTreeMap<String, bool>, Vec<Value>), String> {
    use morph1c_core::{ir::PropertyValue, spec::metadata::common_picture::F_TRANSPARENT_PIXEL};
    let directory = root.join("CommonPictures");
    if !directory.exists() {
        return Ok((BTreeMap::new(), Vec::new()));
    }
    let mut result = BTreeMap::new();
    let mut bindings = Vec::new();
    let registry = FormatRegistry::for_format(Format::Edt).map_err(|e| e.to_string())?;
    let reader = registry
        .get("CommonPicture")
        .ok_or("no CommonPicture reader")?
        .read;
    for path in files(&directory)? {
        if path.extension().and_then(|s| s.to_str()) != Some("mdo") {
            continue;
        }
        let raw = fs::read(&path).map_err(|e| e.to_string())?;
        let object = with_source_version(Some(version), || reader(&raw))?;
        let pixel_present = match object.get(F_TRANSPARENT_PIXEL) {
            None => false,
            Some(PropertyValue::List(values)) => !values.is_empty(),
            Some(_) => return Err("invalid typed CommonPicture transparent pixel".into()),
        };
        if result
            .insert(format!("CommonPicture.{}", object.name), pixel_present)
            .is_some()
        {
            return Err("duplicate CommonPicture descriptor identity".into());
        }
        bindings.push(json!({"path_sha256":sha(path.strip_prefix(root).unwrap().to_str().unwrap().as_bytes()),"sha256":sha(&raw),"bytes":raw.len()}));
    }
    Ok((result, bindings))
}

// Discover registered root descriptors only. Bodies, assets and nested FormRef
// descriptors are never opened to construct this CURRENT metadata namespace.
fn metadata_context(
    registry: &FormatRegistry,
    root: &Path,
    version: FormatVersion,
) -> Result<(Configuration, Vec<Value>), String> {
    let mut config = Configuration::new().with_source_version(Some(version));
    let mut bindings = Vec::new();
    let mut identities = BTreeSet::new();
    // Production read_config copies the registered root's properties into the
    // Configuration before building FormProjectionContext. Compatibility is a
    // CURRENT metadata dependency, independently of the native target version.
    let root_kind = registry.root_kind().ok_or("no Configuration reader")?;
    let root_path = layout::singleton_path(registry, root_kind, root)
        .ok_or("no Configuration descriptor layout")?;
    let root_relative = root_path
        .strip_prefix(root)
        .map_err(|e| e.to_string())?
        .to_str()
        .ok_or("non UTF-8 Configuration descriptor path")?
        .replace('\\', "/");
    ibcmd_xml::source_tree::validate_source_path_safety(&root_relative)
        .map_err(|e| e.to_string())?;
    let root_before = file_sha(&root_path)?;
    let root_raw = fs::read(&root_path).map_err(|e| e.to_string())?;
    if root_before != (sha(&root_raw), root_raw.len() as u64) {
        return Err("Configuration descriptor changed before decode".into());
    }
    let root_object = with_source_version(Some(version), || (root_kind.read)(&root_raw))?;
    if root_object.kind.as_str() != root_kind.kind {
        return Err("registered Configuration descriptor identity mismatch".into());
    }
    if file_sha(&root_path)? != root_before {
        return Err("Configuration descriptor changed during decode".into());
    }
    bindings.push(json!({"path_sha256":sha(root_relative.as_bytes()),"sha256":root_before.0,"bytes":root_before.1,"kind":root_kind.kind}));
    identities.insert(root_relative.to_lowercase());
    config.properties = root_object.properties.clone();
    config.objects.push(root_object);
    for kind in registry.iter().filter(|kind| {
        matches!(
            kind.kind,
            "Catalog"
                | "Document"
                | "Enum"
                | "ChartOfCharacteristicTypes"
                | "ChartOfAccounts"
                | "ChartOfCalculationTypes"
                | "ExchangePlan"
                | "BusinessProcess"
                | "Task"
                | "InformationRegister"
                | "AccumulationRegister"
                | "AccountingRegister"
                | "CalculationRegister"
                | "DocumentJournal"
                | "CommonAttribute"
        )
    }) {
        let directory = layout::kind_rel_dir(registry, kind)
            .into_iter()
            .fold(root.to_path_buf(), |path, segment| path.join(segment));
        if !directory.exists() {
            continue;
        }
        if !regular(&directory)?.is_dir() {
            return Err("invalid metadata kind directory".into());
        }
        let mut owners = fs::read_dir(&directory)
            .map_err(|e| e.to_string())?
            .map(|entry| entry.map(|entry| entry.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        owners.sort();
        for owner in owners {
            if !regular(&owner)?.is_dir() {
                return Err("unexpected metadata root entry".into());
            }
            let name = owner
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or("non UTF-8 owner name")?;
            let path = owner.join(format!("{name}.mdo"));
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("non UTF-8 descriptor path")?
                .replace('\\', "/");
            ibcmd_xml::source_tree::validate_source_path_safety(&relative)
                .map_err(|e| e.to_string())?;
            if !identities.insert(relative.to_lowercase()) {
                return Err("case-colliding metadata descriptor".into());
            }
            let before = file_sha(&path)?;
            let raw = fs::read(&path).map_err(|e| e.to_string())?;
            if before != (sha(&raw), raw.len() as u64) {
                return Err("metadata descriptor changed before decode".into());
            }
            let object = with_source_version(Some(version), || (kind.read)(&raw))?;
            if object.kind.as_str() != kind.kind || object.name != name {
                return Err("registered metadata descriptor identity mismatch".into());
            }
            if file_sha(&path)? != before {
                return Err("metadata descriptor changed during decode".into());
            }
            bindings.push(json!({"path_sha256":sha(relative.as_bytes()),"sha256":before.0,"bytes":before.1,"kind":kind.kind}));
            config.objects.push(object);
        }
    }
    FormProjectionContext::new(&config).map_err(|e| e.to_string())?;
    Ok((config, bindings))
}

// Use the production descriptor reader to bind actual declaration UUID and Help
// fields. Only this form is retained for sidecar attachment; other owner fields
// are outside this form-only census. A fake CommonForm/UUID is never substituted.
fn declared_owner(
    registry: &FormatRegistry,
    root: &Path,
    form: &Path,
    version: FormatVersion,
) -> Result<(MetadataObject, PathBuf), String> {
    let form_dir = form.parent().ok_or("no form directory")?;
    let own = form_dir
        .parent()
        .is_some_and(|p| p.file_name().is_some_and(|n| n == "CommonForms"));
    let owner_dir = if own {
        form_dir
    } else {
        let forms = form_dir.parent().ok_or("no Forms directory")?;
        if forms.file_name().is_none_or(|name| name != "Forms") {
            return Err("unclaimed Form.form layout".into());
        }
        forms.parent().ok_or("no metadata owner")?
    };
    let name = owner_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("no owner name")?;
    let anchor = owner_dir.join(format!("{name}.mdo"));
    let prefix = owner_dir
        .parent()
        .ok_or("no kind directory")?
        .strip_prefix(root)
        .map_err(|e| e.to_string())?;
    let matches: Vec<_> = registry
        .iter()
        .filter(|kind| {
            let directory: PathBuf = layout::kind_rel_dir(registry, kind).into_iter().collect();
            directory == prefix
        })
        .collect();
    if matches.len() != 1 {
        return Err("no unique registered owner descriptor".into());
    }
    let raw = fs::read(&anchor).map_err(|e| e.to_string())?;
    let mut object = with_source_version(Some(version), || (matches[0].read)(&raw))?;
    if object.name != name {
        return Err("owner descriptor/path name mismatch".into());
    }
    if own {
        if object.kind.as_str() != "CommonForm" {
            return Err("wrong CommonForm owner".into());
        }
    } else {
        let form_name = form_dir
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("no form name")?;
        object
            .children
            .retain(|child| child.kind.as_str().ends_with(".FormRef") && child.name == form_name);
        if object.children.len() != 1 {
            return Err("form does not have one exact declared metadata identity".into());
        }
    }
    Ok((object, anchor))
}
fn bind_form(
    object: &mut MetadataObject,
    pictures: &BTreeMap<String, bool>,
    edt: bool,
) -> Result<(), String> {
    if object.form_bodies.len() != 1 {
        return Err("did not attach exactly one managed form".into());
    }
    let uuid = if object.kind.as_str() == "CommonForm" {
        object.uuid
    } else {
        object.children[0].uuid
    };
    resolve_common_picture_transparency(&mut object.form_bodies[0].body, pictures, edt)
        .map_err(|e| e.to_string())?;
    bind_picture_semantics(&mut object.form_bodies[0].body, uuid, edt).map_err(|e| e.to_string())
}

#[test]
fn census_metadata_matches_production_common_attributes_and_compatibility() {
    use formats_xml::form::{FormDialect, read_form, write_form_with_context};
    use morph1c_core::ir::{
        FormControlKind, FormItem, ObjectKind, PropertyValue, Token, TypeSpec, Uuid, ValueSpec,
    };
    use morph1c_core::spec::forms::controls::form_field::F_DATA_PATH;
    use morph1c_core::spec::metadata::{
        common_attribute as ca, configuration::F_COMPATIBILITY_MODE,
    };
    use morph1c_pipeline::{ConvertOptions, read_config, write_config};

    let registry = FormatRegistry::for_format(Format::Edt).unwrap();
    let mut configuration = read_config(
        Format::Designer,
        Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0;
    let owner_name = configuration
        .objects
        .iter()
        .find(|object| object.kind.as_str() == "Catalog")
        .unwrap()
        .name
        .clone();
    let mut common =
        MetadataObject::new(ObjectKind::new("CommonAttribute"), "Shared", Uuid([2; 16]));
    common.properties = vec![
        (ca::F_TYPE, PropertyValue::Type(TypeSpec { parts: vec![] })),
        (
            ca::F_MIN_VALUE,
            PropertyValue::Value(ValueSpec {
                kind: Default::default(),
                scalar: None,
            }),
        ),
        (
            ca::F_MAX_VALUE,
            PropertyValue::Value(ValueSpec {
                kind: Default::default(),
                scalar: None,
            }),
        ),
        (
            ca::F_FILL_VALUE,
            PropertyValue::Value(ValueSpec {
                kind: Default::default(),
                scalar: None,
            }),
        ),
        (ca::F_AUTO_USE, PropertyValue::Enum(Token::new("DontUse"))),
        (
            ca::F_DATA_SEPARATION,
            PropertyValue::Enum(Token::new("DontUse")),
        ),
        (
            ca::F_CONTENT,
            PropertyValue::List(vec![PropertyValue::List(vec![
                PropertyValue::Str(format!("Catalog.{owner_name}")),
                PropertyValue::Enum(Token::new("Use")),
            ])]),
        ),
    ];
    configuration.objects.push(common);

    let form_xml = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\">\r\n",
        "<attributes><name>List</name><valueType><types>DynamicList</types></valueType>\r\n",
        "<view><common>true</common></view><edit><common>true</common></edit>\r\n",
        "<extInfo xsi:type=\"form:DynamicListExtInfo\"><autoFillAvailableFields>true</autoFillAvailableFields></extInfo>\r\n",
        "</attributes></form:Form>\r\n"
    );
    let mut body = read_form(FormDialect::Edt, form_xml.as_bytes()).unwrap();
    let list = body.data_attributes[0].dynamic_list.as_mut().unwrap();
    list.main_table = Some(format!("Catalog.{owner_name}"));
    list.custom_query = false;
    list.auto_fill_available_fields = true;
    list.calculated_fields
        .push(serde_json::from_value(json!({"data_path":"Computed","expression":""})).unwrap());
    for (id, name) in [(1, "Shared"), (2, "Computed")] {
        let mut item = FormItem::new(FormControlKind::new("LabelField"), name, id);
        item.properties
            .push((F_DATA_PATH, PropertyValue::Ref(format!("List.{name}"))));
        body.items.push(item);
    }

    for (mode, computed_path) in [("8.3.18", "~List.Computed"), ("8.3.27", "List.Computed")] {
        let root = configuration
            .objects
            .iter_mut()
            .find(|object| object.kind.as_str() == "Configuration")
            .unwrap();
        root.properties
            .retain(|(id, _)| *id != F_COMPATIBILITY_MODE);
        root.properties
            .push((F_COMPATIBILITY_MODE, PropertyValue::Enum(Token::new(mode))));
        configuration.properties = root.properties.clone();
        for minor in [20, 21] {
            let version = FormatVersion::new(2, minor);
            let directory = tempfile::tempdir().unwrap();
            let source = directory.path().join("src");
            fs::create_dir(&source).unwrap();
            let dt_inf = directory.path().join("DT-INF");
            fs::create_dir(&dt_inf).unwrap();
            // read_config derives EDT source version from the real project
            // envelope, not from ConvertOptions' target version.
            let runtime = if minor == 20 { "8.3.27" } else { "8.5.1" };
            fs::write(
                dt_inf.join("PROJECT.PMF"),
                format!("Runtime-Version: {runtime}\n"),
            )
            .unwrap();
            with_roundtrip_target(version, || {
                write_config(Format::Edt, &configuration, &source)
            })
            .unwrap();
            let before = snapshot(directory.path()).unwrap();
            let production = read_config(Format::Edt, &source, &ConvertOptions::default())
                .unwrap()
                .0;
            let (census, bindings) = metadata_context(&registry, &source, version).unwrap();
            assert_eq!(census.properties, production.properties);
            let production_common = production
                .objects
                .iter()
                .find(|object| object.kind.as_str() == "CommonAttribute")
                .unwrap();
            let census_common = census
                .objects
                .iter()
                .find(|object| object.kind.as_str() == "CommonAttribute")
                .unwrap();
            assert_eq!(census_common.properties, production_common.properties);
            for kind in ["Configuration", "CommonAttribute"] {
                assert_eq!(
                    bindings
                        .iter()
                        .filter(|binding| binding["kind"] == kind)
                        .count(),
                    1
                );
            }
            let expected_context = FormProjectionContext::new(&production).unwrap();
            let context = FormProjectionContext::new(&census).unwrap();
            assert_eq!(
                context.dependency_sha256(&body, version).unwrap(),
                expected_context.dependency_sha256(&body, version).unwrap()
            );
            let bytes = with_roundtrip_target(version, || {
                write_form_with_context(FormDialect::Designer, &body, &context)
            })
            .unwrap();
            let expected = with_roundtrip_target(version, || {
                write_form_with_context(FormDialect::Designer, &body, &expected_context)
            })
            .unwrap();
            assert_eq!(bytes, expected);
            let parsed = formats_xml::parse(&bytes).unwrap();
            let paths: Vec<_> = parsed
                .root
                .child("ChildItems")
                .unwrap()
                .children
                .iter()
                .map(|item| item.child("DataPath").unwrap().text.as_str())
                .collect();
            assert_eq!(paths, ["List.Shared", computed_path]);
            assert_eq!(snapshot(directory.path()).unwrap(), before);
            assert_eq!(
                metadata_context(&registry, &source, version).unwrap().1,
                bindings
            );
        }
    }
}

#[test]
#[ignore = "requires bound genuine UH83/85 failed-form taxonomy and F-lab output"]
fn all_changed_uha83_uha85_forms_production_projection_matches_sdk() {
    let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let run = PathBuf::from(std::env::var_os("IBCMD_UHA_FORM_CENSUS_RUN").unwrap());
    assert!(run.starts_with(&lab));
    fs::create_dir(&run).unwrap();
    let first_case_diagnostic = std::env::var("IBCMD_UHA_FORM_FIRST_CASE_ONLY")
        .ok()
        .as_deref()
        == Some("1");
    let start = Instant::now();
    let exe = std::env::current_exe().unwrap();
    fresh_json(
        &run.join("invocation.json"),
        &json!({
            "scope":"ALL selected failed forms; production descriptor/form attachment, current CommonPicture context, native write and native reread; exact SDK byte comparison; not whole configuration acceptance",
            "extra_normalization":false, "first_case_diagnostic_only":first_case_diagnostic, "executable":exe,"executable_binding":file_sha(&exe).unwrap(),
            "source_binding":std::env::var("IBCMD_UHA_FORM_SOURCE_BINDING").unwrap(),
        }),
    );
    let registry = FormatRegistry::for_format(Format::Edt).unwrap();
    let mut journal = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(run.join("rows.jsonl"))
        .unwrap();
    let mut total = 0usize;
    let mut failures = 0usize;
    let mut results = Vec::new();
    for (profile, minor, wanted, reference, oracle) in [
        (
            "83",
            20,
            1982,
            "native-reference-uha83-r1",
            "oracle-uha83-r1",
        ),
        (
            "85",
            21,
            2673,
            "native-reference-uha85-affinity-r1",
            "oracle-uha85-r2",
        ),
    ] {
        let version = FormatVersion::new(2, minor);
        let input = lab.join(format!(
            "{oracle}/authentic-workspace/OracleConfiguration/src"
        ));
        let expected_root = lab.join(reference).join("native-xml");
        let taxonomy_path = lab.join(format!(
            "uha{profile}-b486-form-diff-classification-r4/result.json"
        ));
        let taxonomy_raw = fs::read(&taxonomy_path).unwrap();
        let taxonomy: Value = serde_json::from_slice(&taxonomy_raw).unwrap();
        let selected = taxonomy["files"].as_array().unwrap();
        assert_eq!(selected.len(), wanted);
        let mut identities = BTreeSet::new();
        let (pictures, context_before) = picture_context(&input, version).unwrap();
        let (metadata, metadata_before) = metadata_context(&registry, &input, version).unwrap();
        fresh_json(
            &run.join(format!("metadata-{profile}-before.json")),
            &json!({"objects":metadata.objects.len(),"descriptors":metadata_before}),
        );
        let projection_context = FormProjectionContext::new(&metadata).unwrap();
        let mut corpus_failed = 0usize;
        let mut exact = 0usize;
        for (index, case) in selected.iter().enumerate() {
            if first_case_diagnostic && index > 0 {
                break;
            }
            let native_path = case["path"].as_str().unwrap();
            ibcmd_xml::source_tree::validate_source_path_safety(native_path).unwrap();
            assert!(native_path.ends_with("/Ext/Form.xml"));
            assert!(identities.insert(native_path.to_lowercase()));
            let form_prefix = native_path.strip_suffix("/Ext/Form.xml").unwrap();
            let form = input.join(form_prefix).join("Form.form");
            let expected = expected_root.join(native_path);
            let mut row = json!({"profile":profile,"index":index,"native_path":native_path,"input_path_sha256":sha(form_prefix.as_bytes())});
            let before = snapshot(form.parent().unwrap());
            let mut descriptor_before = None;
            let mut phase = "input-snapshot";
            let result = (|| -> Result<(), String> {
                row["input_before"] = json!(before.as_ref().map_err(Clone::clone)?);
                phase = "sdk-binding";
                let expected_raw = fs::read(&expected).map_err(|e| e.to_string())?;
                if sha(&expected_raw) != case["expected_sha256"].as_str().unwrap()
                    || expected_raw.len() as u64 != case["expected_bytes"].as_u64().unwrap()
                {
                    return Err("SDK reference differs from bound taxonomy".into());
                }
                row["sdk_sha256"] = json!(sha(&expected_raw));
                row["sdk_bytes"] = json!(expected_raw.len());
                phase = "owner-descriptor";
                let (mut object, anchor) = declared_owner(&registry, &input, &form, version)?;
                descriptor_before = Some((anchor.clone(), file_sha(&anchor)?));
                row["descriptor_binding"] = json!(descriptor_before.as_ref().unwrap().1);
                let kind = object.kind.as_str().to_owned();
                phase = "production-attach";
                with_source_version(Some(version), || {
                    attach_form_body(Format::Edt, &kind, &anchor, &mut object)
                })
                .map_err(|e| e.to_string())?;
                bind_form(&mut object, &pictures, true)?;
                let source_semantic = semantic_sha(&object.form_bodies)?;
                row["source_semantic_sha256"] = json!(source_semantic);
                let scratch = run.join(format!("forms-{profile}/{index:05}"));
                fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
                let output = scratch.join(format!("{}.xml", object.name));
                phase = "production-native-write";
                with_roundtrip_target(version, || {
                    write_form_bodies_with_context(
                        Format::Designer,
                        &output,
                        &object,
                        &projection_context,
                    )
                })
                .map_err(|e| e.to_string())?;
                let produced = if kind == "CommonForm" {
                    scratch.join(&object.name).join("Ext/Form.xml")
                } else {
                    scratch
                        .join(&object.name)
                        .join("Forms")
                        .join(&object.children[0].name)
                        .join("Ext/Form.xml")
                };
                row["generated_form"] = json!(produced);
                phase = "generated-body-read";
                let current = fs::read(&produced).map_err(|e| e.to_string())?;
                row["generated_form"] = json!(produced);
                row["generated_sha256"] = json!(sha(&current));
                row["generated_bytes"] = json!(current.len());
                let sdk_equal = current == expected_raw;
                row["sdk_byte_exact"] = json!(sdk_equal);
                row["first_difference"] =
                    json!(current.iter().zip(&expected_raw).position(|(a, b)| a != b));
                row["output_inventory"] = json!(snapshot(&scratch)?);
                let mut returned = object;
                returned.form_bodies.clear();
                phase = "production-native-reread";
                with_source_version(Some(version), || {
                    attach_form_body(Format::Designer, &kind, &output, &mut returned)
                })
                .map_err(|e| e.to_string())?;
                bind_form(&mut returned, &pictures, false)?;
                let returned_semantic = semantic_sha(&returned.form_bodies)?;
                row["returned_semantic_sha256"] = json!(returned_semantic);
                row["semantic_equal"] = json!(source_semantic == returned_semantic);
                phase = "exact-sdk-and-current-semantic-comparison";
                if !sdk_equal || source_semantic != returned_semantic {
                    return Err("SDK bytes or current form semantics differ".into());
                }
                Ok(())
            })();
            let after = snapshot(form.parent().unwrap());
            let unchanged = before
                .as_ref()
                .ok()
                .zip(after.as_ref().ok())
                .is_some_and(|(a, b)| a == b)
                && descriptor_before
                    .as_ref()
                    .is_none_or(|(p, sha)| file_sha(p).is_ok_and(|v| v == *sha));
            let sdk_unchanged = file_sha(&expected).is_ok_and(|(digest, len)| {
                digest == case["expected_sha256"].as_str().unwrap()
                    && len == case["expected_bytes"].as_u64().unwrap()
            });
            row["input_after"] = json!(after.as_ref().ok());
            row["input_unchanged"] = json!(unchanged);
            row["sdk_unchanged"] = json!(sdk_unchanged);
            row["phase"] = json!(phase);
            row["error_sha256"] = json!(result.as_ref().err().map(|e| sha(e.as_bytes())));
            row["error_local"] = json!(result.as_ref().err());
            let failed = result.is_err() || !unchanged || !sdk_unchanged;
            row["status"] = json!(if failed {
                "FAIL"
            } else {
                "SELECTED_FORM_EXACT_AND_SEMANTIC_PASS"
            });
            append(&mut journal, &row);
            total += 1;
            if row["sdk_byte_exact"] == true {
                exact += 1;
            }
            if failed {
                failures += 1;
                corpus_failed += 1;
            }
        }
        let (_, context_after) = picture_context(&input, version).unwrap();
        let (_, metadata_after) = metadata_context(&registry, &input, version).unwrap();
        fresh_json(
            &run.join(format!("metadata-{profile}-after.json")),
            &json!({"descriptors":metadata_after}),
        );
        let inputs_unchanged = context_before == context_after
            && metadata_before == metadata_after
            && fs::read(&taxonomy_path).unwrap() == taxonomy_raw;
        if !inputs_unchanged {
            failures += 1;
        }
        results.push(json!({"profile":profile,"selected":wanted,"processed":if first_case_diagnostic {1} else {selected.len()},"sdk_exact":exact,"failed":corpus_failed,"taxonomy_sha256":sha(&taxonomy_raw),"metadata_descriptor_count":metadata_before.len(),"metadata_before_sha256":semantic_sha(&metadata_before).unwrap(),"metadata_after_sha256":semantic_sha(&metadata_after).unwrap(),"metadata_context_unchanged":metadata_before==metadata_after,"picture_context_unchanged":context_before==context_after,"taxonomy_unchanged":fs::read(&taxonomy_path).unwrap()==taxonomy_raw}));
    }
    fresh_json(
        &run.join("result.json"),
        &json!({"status":if first_case_diagnostic {"INCOMPLETE_FIRST_CASE_DIAGNOSTIC_NOT_ACCEPTANCE"} else if failures==0 {"SELECTED_FORMS_PASS_NOT_WHOLE_ACCEPTANCE"} else {"FAIL"},"processed":total,"failures":failures,"corpora":results,"seconds":start.elapsed().as_secs_f64()}),
    );
    eprintln!(
        "selected_forms={total} failures={failures} seconds={:.3}",
        start.elapsed().as_secs_f64()
    );
    assert_eq!(total, if first_case_diagnostic { 2 } else { 4655 });
    assert_eq!(failures, 0, "all failures retained in rows.jsonl");
}
