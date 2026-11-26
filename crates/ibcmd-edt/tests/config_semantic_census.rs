//! Opt-in diagnostic: compare the exact production private semantic serializer.
//! Never normalizes models or prints module/query source values.
use morph1c_core::ir::{Configuration, MetadataObject};
use morph1c_core::version::{FormatVersion, with_roundtrip_target};
use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;

fn fingerprint(value: &impl Serialize) -> (String, usize) {
    #[derive(Debug)]
    struct Hash {
        hash: Sha256,
        bytes: usize,
    }
    impl Write for Hash {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.hash.update(bytes);
            self.bytes += bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = std::io::BufWriter::with_capacity(
        64 * 1024,
        Hash {
            hash: Sha256::new(),
            bytes: 0,
        },
    );
    serde_json::to_writer(&mut writer, value).unwrap();
    let writer = writer.into_inner().unwrap();
    (format!("{:x}", writer.hash.finalize()), writer.bytes)
}
fn summary(value: Option<&Value>, path: &str) -> Value {
    let Some(value) = value else {
        return json!({"type":"missing"});
    };
    let (hash, bytes) = fingerprint(value);
    let kind = match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    };
    let length = match value {
        Value::String(s) => Some(s.len()),
        Value::Array(a) => Some(a.len()),
        Value::Object(o) => Some(o.len()),
        _ => None,
    };
    let scalar = match value {
        Value::Bool(_) | Value::Number(_) | Value::Null => Some(value.clone()),
        Value::String(s)
            if path.ends_with(".Enum") && s.len() <= 128 && !s.contains(['\n', '\r']) =>
        {
            Some(value.clone())
        }
        _ => None,
    };
    json!({"type":kind,"length":length,"serialized_bytes":bytes,"sha256":hash,"scalar_or_enum":scalar})
}
fn diff(
    a: Option<&Value>,
    b: Option<&Value>,
    path: &str,
    rows: &mut Vec<Value>,
    count: &mut usize,
) {
    if a == b {
        return;
    }
    match (a, b) {
        (Some(Value::Object(a)), Some(Value::Object(b))) => {
            for key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                diff(
                    a.get(key),
                    b.get(key),
                    &format!("{path}.{key}"),
                    rows,
                    count,
                );
            }
        }
        (Some(Value::Array(a)), Some(Value::Array(b))) => {
            if a.len() != b.len() {
                *count += 1;
                if rows.len() < 1000 {
                    rows.push(json!({"path":path,"before_length":a.len(),"after_length":b.len()}));
                }
            }
            for i in 0..a.len().max(b.len()) {
                diff(a.get(i), b.get(i), &format!("{path}[{i}]"), rows, count);
            }
        }
        _ => {
            *count += 1;
            if rows.len() < 1000 {
                rows.push(json!({"path":path,"before":summary(a,path),"after":summary(b,path)}));
            }
        }
    }
}
fn classify_encoding_bom(a: &Value, b: &Value, path: &str) -> (usize, usize) {
    if a == b {
        return (0, 0);
    }
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => a
            .keys()
            .chain(b.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .fold((0, 0), |(bom, other), k| {
                let counts = match (a.get(k), b.get(k)) {
                    (Some(a), Some(b)) => classify_encoding_bom(a, b, &format!("{path}.{k}")),
                    _ => (0, 1),
                };
                (bom + counts.0, other + counts.1)
            }),
        (Value::Array(a), Value::Array(b)) => (0..a.len().max(b.len())).fold(
            (0, usize::from(a.len() != b.len())),
            |(bom, other), i| {
                let counts = match (a.get(i), b.get(i)) {
                    (Some(a), Some(b)) => classify_encoding_bom(a, b, &format!("{path}[{i}]")),
                    _ => (0, 1),
                };
                (bom + counts.0, other + counts.1)
            },
        ),
        (Value::String(a), Value::String(b))
            if path.ends_with(".Text") && a.strip_prefix('\u{FEFF}') == Some(b.as_str()) =>
        {
            (1, 0)
        }
        _ => (0, 1),
    }
}

fn classify_module_tree(a: &MetadataObject, b: &MetadataObject) -> (usize, usize) {
    use morph1c_core::ir::ModuleBody;
    let mut bom = 0;
    let mut other = usize::from(a.modules.len() != b.modules.len())
        + usize::from(a.children.len() != b.children.len());
    for (a, b) in a.modules.iter().zip(&b.modules) {
        if a.slot != b.slot {
            other += 1;
        }
        if a.body == b.body {
            continue;
        }
        match (&a.body, &b.body) {
            (ModuleBody::Text(a), ModuleBody::Text(b))
                if a.strip_prefix('\u{FEFF}') == Some(b.as_str()) =>
            {
                bom += 1
            }
            _ => other += 1,
        }
    }
    for (a, b) in a.children.iter().zip(&b.children) {
        if key(a) != key(b) {
            other += 1;
        }
        let counts = classify_module_tree(a, b);
        bom += counts.0;
        other += counts.1;
    }
    (bom, other)
}

fn key(object: &MetadataObject) -> String {
    let uuid = object
        .uuid
        .0
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("{uuid}:{}:{}", object.kind.as_str(), object.name)
}
fn object_fields(object: &MetadataObject) -> BTreeMap<&'static str, (String, usize)> {
    let mut fields = BTreeMap::new();
    macro_rules! field { ($($name:ident),*) => { $(fields.insert(stringify!($name),fingerprint(&object.$name));)* }; }
    field!(
        kind,
        name,
        uuid,
        internal_info,
        this_node,
        rights,
        xdto_schema,
        ws_definition,
        flowchart,
        command_interface,
        picture,
        config_pictures,
        config_blobs,
        standalone_content,
        root_command_interface,
        main_section_command_interface,
        home_page_work_area,
        client_application_interface,
        help,
        help_resources,
        schedule,
        style_records,
        children,
        modules,
        forms,
        form_bodies,
        templates,
        source_extensions
    );
    // Identical borrowed FieldId sorter used by the production property serializer.
    let mut properties = object.properties.iter().collect::<Vec<_>>();
    properties.sort_by_key(|p| p.0.0);
    fields.insert("properties", fingerprint(&properties));
    fields
}

#[test]
fn diagnostic_differences_keep_missing_and_lengths_without_source_text() {
    let a = json!({"module":{"Text":"secret original BSL"},"values":[null,1],"optional":null});
    let b = json!({"module":{"Text":"secret edited BSL"},"values":[null]});
    let mut rows = vec![];
    let mut count = 0;
    diff(Some(&a), Some(&b), "$", &mut rows, &mut count);
    assert!(count >= 4);
    let report = serde_json::to_string(&rows).unwrap();
    assert!(!report.contains("secret"));
    assert!(report.contains("missing"));
    assert!(report.contains("before_length"));
}

#[test]
#[ignore = "Actual whole BSP private configuration read/write/read; requires shared heavy FIFO and F lab env"]
fn genuine_bsp_configuration_semantics() {
    let native = PathBuf::from(std::env::var_os("IBCMD_CONFIG_NATIVE").unwrap());
    let project = PathBuf::from(std::env::var_os("IBCMD_CONFIG_PROJECT").unwrap());
    let report = PathBuf::from(std::env::var_os("IBCMD_CONFIG_REPORT").unwrap());
    assert!(!project.exists() && !report.exists());
    let start = std::time::Instant::now();
    let version = FormatVersion::new(2, 20);
    let (source, skipped) = formats_xml::read::with_verbatim_in_text_eol(|| {
        read_config(Format::Designer, &native, &ConvertOptions::default())
    })
    .unwrap();
    assert!(skipped.is_empty());
    assert_eq!(source.source_version, Some(version));
    let src = project.join("src");
    with_roundtrip_target(version, || write_config(Format::Edt, &source, &src)).unwrap();
    std::fs::create_dir(project.join("DT-INF")).unwrap();
    std::fs::write(
        project.join("DT-INF/PROJECT.PMF"),
        b"Manifest-Version: 1.0\r\nRuntime-Version: 8.3.27\r\n",
    )
    .unwrap();
    let (returned, skipped) = read_config(Format::Edt, &src, &ConvertOptions::default()).unwrap();
    assert!(skipped.is_empty());
    assert_eq!(returned.source_version, Some(version));
    let source_hash = fingerprint(&source);
    let returned_hash = fingerprint(&returned);
    let source_root = Configuration {
        source_version: source.source_version,
        properties: source.properties.clone(),
        objects: vec![],
    };
    let returned_root = Configuration {
        source_version: returned.source_version,
        properties: returned.properties.clone(),
        objects: vec![],
    };
    let mut rows = vec![];
    let mut path_count = 0;
    diff(
        Some(&serde_json::to_value(source_root).unwrap()),
        Some(&serde_json::to_value(returned_root).unwrap()),
        "$.configuration",
        &mut rows,
        &mut path_count,
    );
    let source_keys = source.objects.iter().map(key).collect::<Vec<_>>();
    let returned_keys = returned.objects.iter().map(key).collect::<Vec<_>>();
    let order_equal = source_keys == returned_keys;
    let a = source
        .objects
        .iter()
        .map(|o| (key(o), o))
        .collect::<BTreeMap<_, _>>();
    let b = returned
        .objects
        .iter()
        .map(|o| (key(o), o))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(a.len(), source.objects.len());
    assert_eq!(b.len(), returned.objects.len());
    let mut changed = vec![];
    let mut bom_differences = 0;
    let mut other_differences = 0;
    let mut unexpanded_objects = 0;
    let mut all_module_bom_differences = 0;
    let mut all_module_other_differences = 0;
    for object_key in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
        let before = a.get(object_key);
        let after = b.get(object_key);
        let before_hash = before.map(fingerprint);
        let after_hash = after.map(fingerprint);
        if before_hash == after_hash {
            continue;
        }
        let mut field_rows = vec![];
        if let (Some(before), Some(after)) = (before, after) {
            let counts = classify_module_tree(before, after);
            all_module_bom_differences += counts.0;
            all_module_other_differences += counts.1;
            let left = object_fields(before);
            let right = object_fields(after);
            for field in left.keys().chain(right.keys()).collect::<BTreeSet<_>>() {
                if left.get(field) != right.get(field) {
                    field_rows.push(
                        json!({"field":field,"before":left.get(field),"after":right.get(field)}),
                    );
                }
            }
            // This cap limits diagnostic expansion only, never model acceptance.
            if before_hash.as_ref().unwrap().1 <= 16 * 1024 * 1024
                && after_hash.as_ref().unwrap().1 <= 16 * 1024 * 1024
            {
                let before_value = serde_json::to_value(before).unwrap();
                let after_value = serde_json::to_value(after).unwrap();
                let counts = classify_encoding_bom(&before_value, &after_value, "$.object");
                bom_differences += counts.0;
                other_differences += counts.1;
                diff(
                    Some(&before_value),
                    Some(&after_value),
                    &format!("$.objects[{object_key}]"),
                    &mut rows,
                    &mut path_count,
                );
            } else {
                unexpanded_objects += 1;
            }
        } else {
            other_differences += 1;
        }
        changed.push(json!({"identity":object_key,"before":before_hash,"after":after_hash,"fields":field_rows}));
    }
    let equal = source_hash == returned_hash;
    let result = json!({"scope":"exact production streaming serde; no normalization; source values suppressed","equal":equal,"source_fingerprint":source_hash,"returned_fingerprint":returned_hash,"source_objects":source.objects.len(),"returned_objects":returned.objects.len(),"object_order_equal":order_equal,"first_order_difference":source_keys.iter().zip(&returned_keys).position(|(a,b)|a!=b),"changed_object_count":changed.len(),"changed_objects":changed,"typed_path_difference_count":path_count,"typed_path_rows":rows,"typed_paths_truncated":path_count>1000,"all_objects_typed_module_bom_differences":all_module_bom_differences,"all_objects_typed_module_other_differences":all_module_other_differences,"exact_leading_encoding_bom_differences":bom_differences,"other_leaf_differences":other_differences,"unexpanded_changed_objects":unexpanded_objects,"elapsed_seconds":start.elapsed().as_secs_f64()});
    std::fs::write(report, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
    println!("whole configuration semantic diagnostic complete: equal={equal}");
}

#[test]
fn bom_classifier_requires_exact_one_leading_marker_and_text_slot() {
    assert_eq!(
        classify_encoding_bom(
            &json!({"Text":"\u{FEFF}\u{FEFF}x"}),
            &json!({"Text":"\u{FEFF}x"}),
            "$"
        ),
        (1, 0)
    );
    for (a, b) in [
        (json!({"Text":"x\u{FEFF}"}), json!({"Text":"x"})),
        (json!({"Text":"\u{FEFF}x"}), json!({"Text":"y"})),
        (json!({"other":"\u{FEFF}x"}), json!({"other":"x"})),
    ] {
        assert_eq!(classify_encoding_bom(&a, &b, "$"), (0, 1));
    }
}
