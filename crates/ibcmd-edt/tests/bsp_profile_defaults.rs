use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, FormControlKind, FormItem, PropertyValue, Token},
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn version() -> FormatVersion {
    FormatVersion::new(2, 20)
}
fn roundtrip(source: &[u8], from: FormDialect, to: FormDialect) -> Vec<u8> {
    with_source_version(Some(version()), || {
        let body = read_form(from, source).unwrap();
        with_roundtrip_target(version(), || write_form(to, &body).unwrap())
    })
}

#[test]
fn xml220_defaults_are_profile_bound_and_nondefaults_remain_typed() {
    use morph1c_core::spec::forms::controls::{form_field as ff, form_group as fg, table as tb};
    use morph1c_core::spec::forms::form_root as fr;
    let mut form = FormBody::new();
    form.attributes = vec![
        (fr::F_GROUP, PropertyValue::Enum(Token::new("Vertical"))),
        (fr::F_SHOW_TITLE, PropertyValue::Enum(Token::new("true"))),
    ];
    let mut input = FormItem::new(FormControlKind::new("InputField"), "Input", 1);
    input.ext_info.push((
        ff::F_EXT_TEXT_SIZE,
        PropertyValue::Enum(Token::new("Enlarged")),
    ));
    let mut group = FormItem::new(FormControlKind::new("UsualGroup"), "Group", 2);
    group.ext_info = vec![
        (
            fg::F_EXT_GROUP,
            PropertyValue::Enum(Token::new("HorizontalIfPossible")),
        ),
        (
            fg::F_EXT_SHOW_TITLE,
            PropertyValue::Enum(Token::new("true")),
        ),
        (
            fg::F_EXT_REPRESENTATION,
            PropertyValue::Enum(Token::new("WeakSeparation")),
        ),
    ];
    let mut table = FormItem::new(FormControlKind::new("Table"), "Table", 3);
    table.properties = vec![
        (
            tb::F_ROW_SELECTION_MODE,
            PropertyValue::Enum(Token::new("Cell")),
        ),
        (tb::F_HORIZONTAL_LINES, PropertyValue::Bool(true)),
        (tb::F_VERTICAL_LINES, PropertyValue::Bool(false)),
        (tb::F_AUTO_MAX_CARD_HEIGHT, PropertyValue::Bool(false)),
    ];
    form.items = vec![input, group, table];
    let old =
        with_roundtrip_target(version(), || write_form(FormDialect::Designer, &form)).unwrap();
    let old = std::str::from_utf8(&old).unwrap();
    for absent in [
        "<TextSize>",
        "<RowSelectionMode>",
        "<HorizontalLines>",
        "<AutoMaxCardHeight>",
        "<Group>Vertical</Group>",
        "<Group>HorizontalIfPossible</Group>",
        "<ShowTitle>true</ShowTitle>",
        "<Representation>WeakSeparation</Representation>",
    ] {
        assert!(!old.contains(absent), "{absent}");
    }
    assert!(old.contains("<VerticalLines>false</VerticalLines>"));
    let modern = with_roundtrip_target(FormatVersion::new(2, 21), || {
        write_form(FormDialect::Designer, &form)
    })
    .unwrap();
    assert!(
        std::str::from_utf8(&modern)
            .unwrap()
            .contains("<TextSize>Enlarged</TextSize>")
    );
    assert!(
        !std::str::from_utf8(&modern)
            .unwrap()
            .contains("<HorizontalLines>true</HorizontalLines>")
    );
}

#[test]
fn native_checkbox_auto_presence_survives_and_edits_win() {
    use morph1c_core::spec::forms::controls::form_field as ff;
    let mut form = FormBody::new();
    let mut group = FormItem::new(FormControlKind::new("ColumnGroup"), "Group", 1);
    let mut checkbox = FormItem::new(FormControlKind::new("CheckBoxField"), "Checkbox", 2);
    checkbox.ext_info.push((
        ff::F_EXT_CHECK_BOX_TYPE,
        PropertyValue::Enum(Token::new("Auto")),
    ));
    group.children.push(checkbox);
    form.items.push(group);
    form.designer_checkbox_auto_presence.insert(2, true);
    let seed =
        with_roundtrip_target(version(), || write_form(FormDialect::Designer, &form)).unwrap();
    let explicit = roundtrip(&seed, FormDialect::Designer, FormDialect::Designer);
    assert!(
        std::str::from_utf8(&explicit)
            .unwrap()
            .contains("<CheckBoxType>Auto</CheckBoxType>")
    );
    assert_eq!(
        String::from_utf8(roundtrip(
            &explicit,
            FormDialect::Designer,
            FormDialect::Designer
        ))
        .unwrap(),
        String::from_utf8(explicit.clone()).unwrap()
    );
    let sparse = String::from_utf8(explicit.clone())
        .unwrap()
        .replace("\t\t\t\t\t\t<CheckBoxType>Auto</CheckBoxType>\r\n", "");
    let sparse = if sparse.as_bytes() == explicit {
        String::from_utf8(explicit)
            .unwrap()
            .lines()
            .filter(|line| !line.contains("<CheckBoxType>Auto</CheckBoxType>"))
            .collect::<Vec<_>>()
            .join("\r\n")
    } else {
        sparse
    };
    assert_eq!(
        String::from_utf8(roundtrip(
            sparse.as_bytes(),
            FormDialect::Designer,
            FormDialect::Designer
        ))
        .unwrap(),
        sparse
    );
    let mut edited = with_source_version(Some(version()), || {
        read_form(FormDialect::Designer, sparse.as_bytes())
    })
    .unwrap();
    edited.items[0].children[0]
        .ext_info
        .iter_mut()
        .find(|(id, _)| *id == ff::F_EXT_CHECK_BOX_TYPE)
        .unwrap()
        .1 = PropertyValue::Enum(Token::new("Tumbler"));
    let changed =
        with_roundtrip_target(version(), || write_form(FormDialect::Designer, &edited)).unwrap();
    assert!(
        std::str::from_utf8(&changed)
            .unwrap()
            .contains("<CheckBoxType>Tumbler</CheckBoxType>")
    );
    assert!(
        !serde_json::to_string(&edited)
            .unwrap()
            .contains("designer_checkbox_auto_presence")
    );
}

#[test]
fn checkbox_auto_materialization_follows_typed_three_state_not_parent_kind() {
    use morph1c_core::spec::forms::controls::form_field as ff;
    let mut form = FormBody::new();
    let mut checkbox = FormItem::new(FormControlKind::new("CheckBoxField"), "Checkbox", 1);
    checkbox.ext_info = vec![
        (
            ff::F_EXT_CHECK_BOX_TYPE,
            PropertyValue::Enum(Token::new("Auto")),
        ),
        (ff::F_EXT_THREE_STATE, PropertyValue::Bool(true)),
    ];
    form.items.push(checkbox);
    let render = |form: &FormBody| {
        String::from_utf8(
            with_roundtrip_target(version(), || write_form(FormDialect::Designer, form)).unwrap(),
        )
        .unwrap()
    };
    assert!(!render(&form).contains("<CheckBoxType>"));
    form.items[0].ext_info[1].1 = PropertyValue::Bool(false);
    assert!(render(&form).contains("<CheckBoxType>Auto</CheckBoxType>"));
    form.items[0].ext_info[1].1 = PropertyValue::Bool(true);
    form.items[0].ext_info[0].1 = PropertyValue::Enum(Token::new("Tumbler"));
    assert!(render(&form).contains("<CheckBoxType>Tumbler</CheckBoxType>"));
    let semantic = serde_json::to_vec(&form).unwrap();
    form.designer_checkbox_auto_presence.insert(1, false);
    assert_eq!(semantic, serde_json::to_vec(&form).unwrap());
    form.items[0].ext_info[0].1 = PropertyValue::Enum(Token::new("Auto"));
    assert_ne!(semantic, serde_json::to_vec(&form).unwrap());
}

fn paths(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    assert!(depth <= 64);
    assert!(out.len() <= 65536);
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        assert!(!ty.is_symlink());
        if ty.is_dir() {
            paths(&entry.path(), depth + 1, out);
        } else if entry.file_name() == "Form.form" {
            out.push(entry.path());
        }
    }
}

#[test]
#[ignore = "requires genuine EDT source and fresh SDK XML in F lab"]
fn genuine_bsp83_cross_form_census() {
    let start = std::time::Instant::now();
    let root = PathBuf::from(std::env::var_os("IBCMD_EDT_CENSUS_ROOT").unwrap());
    let native = PathBuf::from(std::env::var_os("IBCMD_EDT_NATIVE_ROOT").unwrap());
    let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let mut all = Vec::new();
    paths(&root, 0, &mut all);
    all.sort();
    let mut failures = BTreeMap::<String, Vec<String>>::new();
    let mut differences = Vec::new();
    let mut passed = 0;
    for source in &all {
        let rel = source.strip_prefix(&root).unwrap();
        let expected = native.join(rel.parent().unwrap()).join("Ext/Form.xml");
        let snapshot = tempfile::tempdir_in(&lab).unwrap();
        std::fs::copy(source, snapshot.path().join("Form.form")).unwrap();
        let limits = ibcmd_edt::ReaderLimits {
            files: 1,
            directories: 1,
            depth: 2,
            asset_bytes: 32 * 1024 * 1024,
            total_bytes: 32 * 1024 * 1024,
        };
        let tree = ibcmd_edt::read_xml_source(snapshot.path(), limits).unwrap();
        let result = with_source_version(Some(version()), || {
            read_form(FormDialect::Edt, tree.entries()[0].bytes())
        })
        .and_then(|body| {
            with_roundtrip_target(version(), || write_form(FormDialect::Designer, &body))
        });
        let path = rel.to_string_lossy().replace('\\', "/");
        match result {
            Ok(bytes) if bytes == std::fs::read(&expected).unwrap() => passed += 1,
            Ok(bytes) => {
                differences.push(path.clone());
                if differences.len() <= 128 {
                    let n = differences.len();
                    std::fs::write(lab.join(format!("{n}.generated.xml")), bytes).unwrap();
                    std::fs::copy(expected, lab.join(format!("{n}.sdk.xml"))).unwrap();
                }
            }
            Err(error) => failures.entry(error.to_string()).or_default().push(path),
        }
    }
    let report = serde_json::json!({"files":all.len(),"passed":passed,"differences":differences,"failures":failures,"seconds":start.elapsed().as_secs_f64(),"scope":"direct typed Form.form body; attached sidecars are tested by whole configuration acceptance"});
    std::fs::write(
        lab.join("cross-form-census.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    eprintln!(
        "files={} passed={} differences={} failures={} seconds={}",
        all.len(),
        passed,
        differences.len(),
        failures.len(),
        start.elapsed().as_secs_f64()
    );
    assert!(failures.is_empty());
}
