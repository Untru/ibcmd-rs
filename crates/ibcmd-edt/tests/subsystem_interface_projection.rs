use formats_xml::{metadata_picture_semantics as carrier, picture};
use ibcmd_edt::{
    ConversionOptions, Project, ReaderLimits, edt_to_xml, read_directory_source, read_xml_source,
    xml_to_edt,
};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree, publish_new};
use morph1c_core::{
    ir::{
        CommandGroupFragment, CommandInterface, Configuration, MetadataObject, ObjectKind,
        PropertyValue, Uuid,
    },
    spec::metadata::{common_picture, subsystem},
    version::{FormatVersion, with_roundtrip_target},
};
use morph1c_pipeline::{
    ConvertOptions, Format, read_config, registry::FormatRegistry, write_config,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const NIL: &str = "00000000-0000-0000-0000-000000000000";
const CI_NATIVE: &str = "Subsystems/ПодсистемаКИ/Ext/CommandInterface.xml";
const CI_EDT: &str = "Subsystems/ПодсистемаКИ/CommandInterface.cmi";
const SUB_NATIVE: &str = "Subsystems/ПодсистемаКИ.xml";
const SUB_EDT: &str = "src/Subsystems/ПодсистемаКИ/ПодсистемаКИ.mdo";
const RESOURCE: &str = "src/Subsystems/ПодсистемаКИ/ibcmd-metadata-picture-semantics.v1.json";

fn fixture() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ))
}
fn opts(version: FormatVersion) -> ConvertOptions {
    ConvertOptions::default().with_target_version(version)
}
fn public_opts(version: FormatVersion) -> ConversionOptions {
    ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: version.to_string(),
        runtime_version: Some(
            if version.minor == 20 {
                "8.3.27"
            } else {
                "8.5.1"
            }
            .into(),
        ),
    }
}
fn seed() -> Configuration {
    read_config(
        Format::Designer,
        &fixture(),
        &opts(FormatVersion::new(2, 21)),
    )
    .unwrap()
    .0
}
fn sub(config: &Configuration) -> &MetadataObject {
    config
        .objects
        .iter()
        .find(|o| o.kind.as_str() == "Subsystem")
        .unwrap()
}
fn sub_mut(config: &mut Configuration) -> &mut MetadataObject {
    config
        .objects
        .iter_mut()
        .find(|o| o.kind.as_str() == "Subsystem")
        .unwrap()
}
fn ci(config: &Configuration) -> &CommandInterface {
    sub(config).command_interface.as_ref().unwrap()
}
fn sample_ci(groups: &[&str]) -> CommandInterface {
    CommandInterface {
        native_groups_order: None,
        subsystems_visibility: vec![],
        commands: vec![],
        placement: vec![],
        order: groups
            .iter()
            .enumerate()
            .map(|(i, g)| CommandGroupFragment {
                group: (*g).into(),
                commands: vec![format!(
                    "Catalog.СтроковыеКонтактыВзаимодействий.StandardCommand.{}",
                    if i == 0 { "OpenList" } else { "Create" }
                )],
            })
            .collect(),
        subsystems_order: vec![],
    }
}
fn write(format: Format, config: &Configuration, path: &Path, version: FormatVersion) {
    with_roundtrip_target(version, || write_config(format, config, path)).unwrap();
}
fn read(format: Format, path: &Path, version: FormatVersion) -> Configuration {
    read_config(format, path, &opts(version)).unwrap().0
}
fn serialize(format: Format, object: &MetadataObject, version: FormatVersion) -> Vec<u8> {
    with_roundtrip_target(version, || {
        (FormatRegistry::for_format(format)
            .unwrap()
            .get(object.kind.as_str())
            .unwrap()
            .write)(object)
    })
    .unwrap()
}
fn descriptor(format: Format, bytes: &[u8], kind: &str) -> MetadataObject {
    (FormatRegistry::for_format(format)
        .unwrap()
        .get(kind)
        .unwrap()
        .read)(bytes)
    .unwrap()
}
fn with_nil_region(bytes: &[u8]) -> Vec<u8> {
    let mut text = String::from_utf8(bytes.to_vec()).unwrap();
    let nil = format!("\t\t<Group>{NIL}</Group>\r\n");
    if text.contains("\t</GroupsOrder>\r\n") {
        text = text.replace("\t</GroupsOrder>\r\n", &(nil + "\t</GroupsOrder>\r\n"));
    } else {
        text = text.replace(
            "</CommandInterface>",
            &format!("\t<GroupsOrder>\r\n{nil}\t</GroupsOrder>\r\n</CommandInterface>"),
        );
    }
    text.into_bytes()
}

#[test]
fn sdk_nil_projection_preserves_current_order_and_native_presence_only() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        for groups in [vec![NIL], vec!["NavigationPanelOrdinary", NIL]] {
            let root = tempfile::tempdir().unwrap();
            let mut model = seed();
            sub_mut(&mut model).command_interface = Some(sample_ci(&groups));
            let native_dir = root.path().join("native");
            write(Format::Designer, &model, &native_dir, version);
            let sdk_bytes = std::fs::read(native_dir.join(CI_NATIVE)).unwrap();
            assert!(
                !String::from_utf8_lossy(&sdk_bytes).contains(&format!("<Group>{NIL}</Group>"))
            );
            assert!(
                String::from_utf8_lossy(&sdk_bytes)
                    .contains(&format!("<CommandGroup>{NIL}</CommandGroup>"))
            );
            assert_eq!(
                String::from_utf8_lossy(&sdk_bytes).contains("<GroupsOrder>"),
                groups.len() > 1
            );
            let sdk_model = read(Format::Designer, &native_dir, version);
            assert_eq!(ci(&model), ci(&sdk_model));
            let sdk_out = root.path().join("sdk-out");
            write(Format::Designer, &sdk_model, &sdk_out, version);
            assert_eq!(std::fs::read(sdk_out.join(CI_NATIVE)).unwrap(), sdk_bytes);

            let original_bytes = with_nil_region(&sdk_bytes);
            std::fs::write(native_dir.join(CI_NATIVE), &original_bytes).unwrap();
            let original = read(Format::Designer, &native_dir, version);
            assert_eq!(ci(&original), ci(&sdk_model));
            assert_eq!(
                serde_json::to_vec(ci(&original)).unwrap(),
                serde_json::to_vec(ci(&sdk_model)).unwrap()
            );
            let own = root.path().join("own");
            write(Format::Designer, &original, &own, version);
            assert_eq!(std::fs::read(own.join(CI_NATIVE)).unwrap(), original_bytes);

            let edt = root.path().join("edt");
            write(Format::Edt, &original, &edt, version);
            let cross = read(Format::Edt, &edt, version);
            assert_eq!(ci(&cross), ci(&original));
            let back = root.path().join("back");
            write(Format::Designer, &cross, &back, version);
            assert_eq!(std::fs::read(back.join(CI_NATIVE)).unwrap(), sdk_bytes);

            let mut changed = original.clone();
            sub_mut(&mut changed)
                .command_interface
                .as_mut()
                .unwrap()
                .order[0]
                .commands[0] =
                "Catalog.СтроковыеКонтактыВзаимодействий.StandardCommand.Create".into();
            assert_ne!(
                serde_json::to_vec(ci(&changed)).unwrap(),
                serde_json::to_vec(ci(&original)).unwrap()
            );
            let changed_dir = root.path().join("changed");
            write(Format::Designer, &changed, &changed_dir, version);
            assert!(
                String::from_utf8_lossy(&std::fs::read(changed_dir.join(CI_NATIVE)).unwrap())
                    .contains("StandardCommand.Create")
            );
            // A changed group sequence invalidates the old lexical policy; never replay its names.
            sub_mut(&mut changed)
                .command_interface
                .as_mut()
                .unwrap()
                .order
                .reverse();
            sub_mut(&mut changed)
                .command_interface
                .as_mut()
                .unwrap()
                .order[0]
                .group = "NavigationPanelImportant".into();
            let reordered = root.path().join("reordered");
            write(Format::Designer, &changed, &reordered, version);
            let current = std::fs::read(reordered.join(CI_NATIVE)).unwrap();
            assert!(!String::from_utf8_lossy(&current).contains(&format!("<Group>{NIL}</Group>")));
            assert_eq!(
                ci(&read(Format::Designer, &reordered, version)),
                ci(&changed)
            );
            let current_before = serde_json::to_vec(ci(&changed)).unwrap();
            sub_mut(&mut changed)
                .command_interface
                .as_mut()
                .unwrap()
                .order
                .push(CommandGroupFragment {
                    group: "11111111-1111-1111-1111-111111111111".into(),
                    commands: vec![
                        "Catalog.СтроковыеКонтактыВзаимодействий.StandardCommand.OpenList".into(),
                    ],
                });
            assert_ne!(serde_json::to_vec(ci(&changed)).unwrap(), current_before);
            let appended = root.path().join("appended");
            write(Format::Designer, &changed, &appended, version);
            assert!(
                String::from_utf8_lossy(&std::fs::read(appended.join(CI_NATIVE)).unwrap())
                    .contains("<Group>11111111-1111-1111-1111-111111111111</Group>")
            );
            sub_mut(&mut changed)
                .command_interface
                .as_mut()
                .unwrap()
                .order
                .clear();
            let deleted = root.path().join("deleted");
            write(Format::Designer, &changed, &deleted, version);
            assert!(
                !String::from_utf8_lossy(&std::fs::read(deleted.join(CI_NATIVE)).unwrap())
                    .contains("<GroupsOrder>")
            );
        }
    }
}

#[test]
fn empty_native_group_slots_are_names_only_and_current_group_edits_invalidate_them() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let root = tempfile::tempdir().unwrap();
        let mut model = seed();
        sub_mut(&mut model).command_interface = Some(sample_ci(&["NavigationPanelOrdinary", NIL]));
        let input = root.path().join("input");
        write(Format::Designer, &model, &input, version);
        let sdk = std::fs::read(input.join(CI_NATIVE)).unwrap();
        let base = String::from_utf8(with_nil_region(&sdk)).unwrap();
        for (index, empty) in [
            "NavigationPanelImportant",
            "ActionsPanelReports",
            "CommandGroup.ПустаяГруппа",
            "11111111-1111-1111-1111-111111111111",
        ]
        .into_iter()
        .enumerate()
        {
            let native = base.replace(
                "<GroupsOrder>",
                &format!("<GroupsOrder>\r\n\t\t<Group>{empty}</Group>"),
            );
            std::fs::write(input.join(CI_NATIVE), &native).unwrap();
            let decoded = read(Format::Designer, &input, version);
            assert_eq!(ci(&decoded), ci(&model));
            assert_eq!(
                serde_json::to_vec(ci(&decoded)).unwrap(),
                serde_json::to_vec(ci(&model)).unwrap()
            );
            let own = root.path().join(format!("own-{index}"));
            write(Format::Designer, &decoded, &own, version);
            assert_eq!(
                std::fs::read(own.join(CI_NATIVE)).unwrap(),
                native.as_bytes()
            );
            let mut changed = decoded.clone();
            let interface = sub_mut(&mut changed).command_interface.as_mut().unwrap();
            interface.order[0].commands[0] =
                "Catalog.СтроковыеКонтактыВзаимодействий.StandardCommand.Create".into();
            let current = root.path().join(format!("current-{index}"));
            write(Format::Designer, &changed, &current, version);
            let current_bytes = std::fs::read(current.join(CI_NATIVE)).unwrap();
            assert!(String::from_utf8_lossy(&current_bytes).contains("StandardCommand.Create"));
            assert!(
                String::from_utf8_lossy(&current_bytes)
                    .contains(&format!("<Group>{empty}</Group>"))
            );
            // Changing the empty physical name is current source spelling, with no old command values.
            let interface = sub_mut(&mut changed).command_interface.as_mut().unwrap();
            interface
                .native_groups_order
                .as_mut()
                .unwrap()
                .region_groups[0] = "CommandGroup.ДругаяГруппа".into();
            let empty_edit = root.path().join(format!("empty-edit-{index}"));
            write(Format::Designer, &changed, &empty_edit, version);
            let bytes = std::fs::read(empty_edit.join(CI_NATIVE)).unwrap();
            assert!(
                String::from_utf8_lossy(&bytes)
                    .contains("<Group>CommandGroup.ДругаяГруппа</Group>")
            );
            assert!(!String::from_utf8_lossy(&bytes).contains(&format!("<Group>{empty}</Group>")));
            let interface = sub_mut(&mut changed).command_interface.as_mut().unwrap();
            interface.order[0].group = "NavigationPanelSeeAlso".into();
            let moved = root.path().join(format!("moved-{index}"));
            write(Format::Designer, &changed, &moved, version);
            let bytes = std::fs::read(moved.join(CI_NATIVE)).unwrap();
            assert!(
                !String::from_utf8_lossy(&bytes)
                    .contains("<Group>CommandGroup.ДругаяГруппа</Group>")
            );
            assert!(
                String::from_utf8_lossy(&bytes).contains("<Group>NavigationPanelSeeAlso</Group>")
            );
            assert_eq!(ci(&read(Format::Designer, &moved, version)), ci(&changed));
        }
    }
}

#[test]
fn groups_order_unknown_mismatch_and_duplicate_regions_remain_errors() {
    let version = FormatVersion::new(2, 21);
    let root = tempfile::tempdir().unwrap();
    let mut model = seed();
    sub_mut(&mut model).command_interface = Some(sample_ci(&["NavigationPanelOrdinary", NIL]));
    write(Format::Designer, &model, root.path(), version);
    let path = root.path().join(CI_NATIVE);
    let valid = String::from_utf8(std::fs::read(&path).unwrap()).unwrap();
    for bad in [
        valid.replace(
            "<Group>NavigationPanelOrdinary</Group>",
            "<Group>UnrelatedGroup</Group>",
        ),
        valid.replace("</GroupsOrder>", "</GroupsOrder>\r\n\t<GroupsOrder/>"),
        valid.replace("<GroupsOrder>", "<UnexpectedGroupsOrder>"),
        valid.replace(
            "<GroupsOrder>",
            "<GroupsOrder>\r\n\t\t<Group>bad/group</Group>",
        ),
        valid.replace(
            "<GroupsOrder>",
            "<GroupsOrder>\r\n\t\t<Group><Nested/></Group>",
        ),
        valid.replace(
            "<Group>NavigationPanelOrdinary</Group>",
            "<Group extra=\"true\">NavigationPanelOrdinary</Group>",
        ),
        valid.replace(
            "<Group>NavigationPanelOrdinary</Group>",
            "<Group>NavigationPanelOrdinary</Group><Group>NavigationPanelOrdinary</Group>",
        ),
    ] {
        std::fs::write(&path, bad).unwrap();
        assert!(read_config(Format::Designer, root.path(), &opts(version)).is_err());
    }
}

fn set_picture(object: &mut MetadataObject, reference: &str, lt: bool, pixel: Option<(i64, i64)>) {
    let value = picture::pack(reference.into(), lt, pixel);
    if let Some(entry) = object
        .properties
        .iter_mut()
        .find(|(f, _)| *f == subsystem::F_PICTURE)
    {
        entry.1 = value;
    } else {
        object.properties.push((subsystem::F_PICTURE, value));
    }
}
fn shared_picture(pixel: Option<(i64, i64)>) -> MetadataObject {
    let mut object =
        MetadataObject::new(ObjectKind::new("CommonPicture"), "Shared", Uuid([0x68; 16]));
    if let Some((x, y)) = pixel {
        object.properties.push((
            common_picture::F_TRANSPARENT_PIXEL,
            PropertyValue::List(vec![PropertyValue::Int(x), PropertyValue::Int(y)]),
        ));
    }
    object
}
#[test]
fn subsystem_current_context_and_closed_resource_preserve_explicit_tuple() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let mut object = sub(&seed()).clone();
        set_picture(&mut object, "CommonPicture.Shared", false, None);
        for pixel in [Some((14, 6)), None, Some((-1, -1))] {
            let defaults = carrier::common_picture_defaults(&[shared_picture(pixel)]).unwrap();
            carrier::resolve(&mut object, &defaults).unwrap();
            assert_eq!(
                picture::unpack(object.get(subsystem::F_PICTURE).unwrap()).unwrap(),
                ("CommonPicture.Shared", pixel.is_some(), None)
            );
            let (projected, bytes) = carrier::project_with_defaults(&object, &defaults).unwrap();
            assert!(bytes.is_none());
            let mut back = descriptor(
                Format::Edt,
                &serialize(Format::Edt, &projected, version),
                "Subsystem",
            );
            carrier::resolve(&mut back, &defaults).unwrap();
            assert_eq!(
                back.get(subsystem::F_PICTURE),
                object.get(subsystem::F_PICTURE)
            );
        }
        let defaults = carrier::common_picture_defaults(&[shared_picture(Some((14, 6)))]).unwrap();
        // A nested Subsystem owns a separate descriptor/carrier. Its CURRENT
        // tuple must not be projected or restored through this parent resource.
        let mut child = object.clone();
        child.uuid = Uuid([0x79; 16]);
        child.children.clear();
        set_picture(&mut child, "CommonPicture.Shared", false, Some((7, 9)));
        object.children.push(child.clone());
        set_picture(&mut object, "CommonPicture.Shared", true, None);
        let (projected, resource) = carrier::project_with_defaults(&object, &defaults).unwrap();
        assert!(resource.is_none());
        assert_eq!(projected.children.last().unwrap(), &child);
        set_picture(&mut object, "CommonPicture.Shared", false, Some((1, 2)));
        let (_, own_resource) = carrier::project_with_defaults(&object, &defaults).unwrap();
        let (_, child_resource) = carrier::project_with_defaults(&child, &defaults).unwrap();
        let mut bad: serde_json::Value = serde_json::from_slice(&own_resource.unwrap()).unwrap();
        let child_row: serde_json::Value =
            serde_json::from_slice(&child_resource.unwrap()).unwrap();
        bad["pictures"]
            .as_array_mut()
            .unwrap()
            .push(child_row["pictures"][0].clone());
        let before = serde_json::to_vec(&object).unwrap();
        let reason = carrier::apply(&mut object, &serde_json::to_vec(&bad).unwrap()).unwrap_err();
        assert!(reason.contains("command deleted/unknown"));
        assert_eq!(serde_json::to_vec(&object).unwrap(), before);
        object.children.pop();
        for tuple in [
            ("CommonPicture.Shared", false, None),
            ("CommonPicture.Shared", true, Some((1, 9))),
            ("", false, Some((-1, 4))),
        ] {
            set_picture(&mut object, tuple.0, tuple.1, tuple.2);
            let native = serialize(Format::Designer, &object, version);
            let (projected, bytes) = carrier::project_with_defaults(&object, &defaults).unwrap();
            let bytes = bytes.unwrap();
            let mut back = descriptor(
                Format::Edt,
                &serialize(Format::Edt, &projected, version),
                "Subsystem",
            );
            carrier::apply(&mut back, &bytes).unwrap();
            carrier::resolve(&mut back, &defaults).unwrap();
            assert_eq!(
                back.get(subsystem::F_PICTURE),
                object.get(subsystem::F_PICTURE)
            );
            assert_eq!(serialize(Format::Designer, &back, version), native);
            let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            for property in [
                "owner_kind",
                "command_kind",
                "command_uuid",
                "reference",
                "slot",
                "unknown",
            ] {
                let mut bad = original.clone();
                match property {
                    "owner_kind" => bad[property] = serde_json::json!("Catalog"),
                    "command_uuid" => {
                        bad["pictures"][0][property] = serde_json::json!(vec![0_u8; 16])
                    }
                    _ => bad["pictures"][0][property] = serde_json::json!("Changed"),
                }
                let before = serde_json::to_vec(&back).unwrap();
                assert!(carrier::apply(&mut back, &serde_json::to_vec(&bad).unwrap()).is_err());
                assert_eq!(serde_json::to_vec(&back).unwrap(), before);
            }
        }
    }
}

fn replace(tree: &SourceTree, path: &str, bytes: Vec<u8>) -> SourceTree {
    let mut entries: Vec<_> = tree
        .entries()
        .iter()
        .filter(|e| e.path().as_str() != path)
        .cloned()
        .collect();
    entries.push(SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes).unwrap());
    SourceTree::new(entries).unwrap()
}
fn strip(tree: &SourceTree) -> SourceTree {
    SourceTree::new(
        tree.entries()
            .iter()
            .filter(|e| !e.path().as_str().starts_with(".ibcmd-provenance/"))
            .cloned()
            .collect(),
    )
    .unwrap()
}
fn bytes<'a>(tree: &'a SourceTree, path: &str) -> &'a [u8] {
    tree.entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes()
}

#[test]
fn public_authentic_reference_only_subsystem_uses_current_common_picture_context() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let root = tempfile::tempdir().unwrap();
        let mut model = seed();
        set_picture(sub_mut(&mut model), "CommonPicture.Shared", false, None);
        let native = root.path().join("native");
        write(Format::Designer, &model, &native, version);
        let options = public_opts(version);
        let source = read_xml_source(&native, ReaderLimits::default()).unwrap();
        let generated = xml_to_edt(&source, &options).unwrap().tree;
        const COMMON: &str = "src/CommonPictures/Shared/Shared.mdo";
        const IMAGE: &str = "src/CommonPictures/Shared/Picture.png";
        let png = vec![
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 4, 0, 0, 0, 181, 28, 12, 2, 0, 0, 0, 11, 73, 68, 65, 84, 120, 218, 99, 100, 248, 15,
            0, 1, 5, 1, 1, 39, 24, 227, 102, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ];
        for pixel in [Some((14, 6)), None, Some((-1, -1))] {
            let common_bytes = serialize(Format::Edt, &shared_picture(pixel), version);
            let current = replace(
                &replace(&strip(&generated), COMMON, common_bytes.clone()),
                IMAGE,
                png.clone(),
            );
            let converted = edt_to_xml(&Project::from_tree(current).unwrap(), &options).unwrap();
            let object = descriptor(
                Format::Designer,
                bytes(&converted.tree, SUB_NATIVE),
                "Subsystem",
            );
            assert_eq!(
                picture::unpack(object.get(subsystem::F_PICTURE).unwrap()).unwrap(),
                ("CommonPicture.Shared", pixel.is_some(), None)
            );
            assert!(
                converted
                    .extensions
                    .iter()
                    .all(|e| e.id != "ibcmd-metadata-picture-semantics/1")
            );
            let mut manifest: serde_json::Value =
                serde_json::from_slice(bytes(&generated, ".ibcmd-provenance/manifest.json"))
                    .unwrap();
            manifest["generated"][COMMON] =
                serde_json::json!(format!("{:x}", Sha256::digest(&common_bytes)));
            manifest["generated"][IMAGE] = serde_json::json!(format!("{:x}", Sha256::digest(&png)));
            let forged = replace(
                &replace(
                    &replace(&generated, COMMON, common_bytes),
                    IMAGE,
                    png.clone(),
                ),
                ".ibcmd-provenance/manifest.json",
                serde_json::to_vec(&manifest).unwrap(),
            );
            assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options).is_err());
        }
    }
}

#[test]
fn public_subsystem_resource_current_edits_and_rehashed_provenance_are_checked() {
    for version in [FormatVersion::new(2, 20), FormatVersion::new(2, 21)] {
        let root = tempfile::tempdir().unwrap();
        let mut config = seed();
        set_picture(
            sub_mut(&mut config),
            "CommonPicture.Shared",
            true,
            Some((13, 3)),
        );
        sub_mut(&mut config).command_interface = Some(sample_ci(&[NIL]));
        let native = root.path().join("native");
        write(Format::Designer, &config, &native, version);
        // Independent platform spelling, rather than the SDK nil-excluded default.
        let original_ci = with_nil_region(&std::fs::read(native.join(CI_NATIVE)).unwrap());
        std::fs::write(native.join(CI_NATIVE), &original_ci).unwrap();
        let source = read_xml_source(&native, ReaderLimits::default()).unwrap();
        let options = public_opts(version);
        let converted = xml_to_edt(&source, &options).unwrap();
        assert_eq!(
            converted
                .extensions
                .iter()
                .find(|e| e.id == "ibcmd-metadata-picture-semantics/1")
                .map(|e| (e.resources, e.references)),
            Some((1, 1))
        );
        let generated = converted.tree;
        assert_eq!(
            edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
                .unwrap()
                .tree,
            source
        );
        let disk = read_directory_source(&native)
            .unwrap()
            .xml_to_edt(&options)
            .unwrap();
        let disk_edt = root.path().join("disk-edt");
        disk.publish_new(&disk_edt).unwrap();
        let disk_back = read_directory_source(&disk_edt)
            .unwrap()
            .edt_to_xml(&options)
            .unwrap();
        let disk_native = root.path().join("disk-native");
        disk_back.publish_new(&disk_native).unwrap();
        assert_eq!(
            read_xml_source(&disk_native, ReaderLimits::default()).unwrap(),
            source
        );
        let stripped = strip(&generated);
        let returned = edt_to_xml(&Project::from_tree(stripped.clone()).unwrap(), &options)
            .unwrap()
            .tree;
        assert_eq!(bytes(&returned, SUB_NATIVE), bytes(&source, SUB_NATIVE));
        assert!(!String::from_utf8_lossy(bytes(&returned, CI_NATIVE)).contains("<GroupsOrder>"));

        let mut record: serde_json::Value =
            serde_json::from_slice(bytes(&generated, RESOURCE)).unwrap();
        record["pictures"][0]["pixel"]["x"] = serde_json::json!(14);
        record["pictures"][0]["load_transparent"] = serde_json::json!(false);
        let edited_bytes = serde_json::to_vec(&record).unwrap();
        let edited = replace(&generated, RESOURCE, edited_bytes.clone());
        let current = edt_to_xml(&Project::from_tree(strip(&edited)).unwrap(), &options)
            .unwrap()
            .tree;
        let decoded = descriptor(Format::Designer, bytes(&current, SUB_NATIVE), "Subsystem");
        assert_eq!(
            picture::unpack(decoded.get(subsystem::F_PICTURE).unwrap()).unwrap(),
            ("CommonPicture.Shared", false, Some((14, 3)))
        );
        let mut manifest: serde_json::Value =
            serde_json::from_slice(bytes(&generated, ".ibcmd-provenance/manifest.json")).unwrap();
        manifest["generated"][RESOURCE] =
            serde_json::json!(format!("{:x}", Sha256::digest(&edited_bytes)));
        let forged = replace(
            &edited,
            ".ibcmd-provenance/manifest.json",
            serde_json::to_vec(&manifest).unwrap(),
        );
        assert!(edt_to_xml(&Project::from_tree(forged).unwrap(), &options).is_err());
        let changed_descriptor = String::from_utf8(bytes(&stripped, SUB_EDT).to_vec())
            .unwrap()
            .replace("CommonPicture.Shared", "StdPicture.Help");
        assert!(
            edt_to_xml(
                &Project::from_tree(replace(&stripped, SUB_EDT, changed_descriptor.into_bytes()))
                    .unwrap(),
                &options
            )
            .is_err()
        );
        let deleted = SourceTree::new(
            stripped
                .entries()
                .iter()
                .filter(|e| e.path().as_str() != SUB_EDT)
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(edt_to_xml(&Project::from_tree(deleted).unwrap(), &options).is_err());

        // Disk stripped-current route must use the same typed resource and values.
        let changed_dir = root.path().join("stripped-current");
        publish_new(&strip(&edited), &changed_dir).unwrap();
        let changed_out = root.path().join("stripped-current-native");
        read_directory_source(&changed_dir)
            .unwrap()
            .edt_to_xml(&options)
            .unwrap()
            .publish_new(&changed_out)
            .unwrap();
        assert_eq!(
            std::fs::read(changed_out.join(SUB_NATIVE)).unwrap(),
            bytes(&current, SUB_NATIVE)
        );
    }
}

fn fingerprint_files(paths: &[PathBuf]) -> std::collections::BTreeMap<String, serde_json::Value> {
    paths
        .iter()
        .map(|path| {
            let bytes = std::fs::read(path).unwrap();
            (
                path.to_string_lossy().replace('\\', "/"),
                serde_json::json!({
                    "bytes": bytes.len(), "sha256": format!("{:x}", Sha256::digest(&bytes)),
                }),
            )
        })
        .collect()
}

#[test]
#[ignore = "requires immutable genuine UH83/UH85 SDK/native/authentic EDT fixtures and fresh report path"]
fn genuine_seven_nil_interfaces_and_two_contextual_subsystem_pictures() {
    let lab = PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").expect("IBCMD_EDT_LAB"));
    let destination = PathBuf::from(
        std::env::var_os("IBCMD_SUBSYSTEM_INTERFACE_REPORT")
            .expect("fresh IBCMD_SUBSYSTEM_INTERFACE_REPORT"),
    );
    assert!(destination.starts_with(&lab));
    assert!(!destination.exists(), "report must be fresh");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut binding_paths = vec![std::env::current_exe().unwrap()];
    for source in [
        "crates/ibcmd-edt/tests/subsystem_interface_projection.rs",
        "crates/ibcmd-edt/tests/dcs_qname_directory.rs",
        "crates/ibcmd-edt/tests/subsystems_visibility.rs",
        "crates/ibcmd-edt/vendor/morph1c/crates/core/src/ir/mod.rs",
        "crates/ibcmd-edt/vendor/morph1c/crates/pipeline/src/cmi_read.rs",
        "crates/ibcmd-edt/vendor/morph1c/crates/pipeline/src/ext_read.rs",
        "crates/ibcmd-edt/vendor/morph1c/crates/formats-xml/src/metadata_picture_semantics.rs",
    ] {
        binding_paths.push(repo.join(source));
    }
    let mut cohorts = Vec::new();
    for (profile, minor, native_dir, edt_dir, original) in [
        (
            "UH83",
            20,
            "native-reference-uha83-r1/native-xml",
            "oracle-uha83-r1/authentic-workspace/OracleConfiguration/src",
            lab.parent()
                .unwrap()
                .join("04/release-20261001/rc/out/uha8327_db_r1/tree"),
        ),
        (
            "UH85",
            21,
            "native-reference-uha85-affinity-r1/native-xml",
            "oracle-uha85-r2/authentic-workspace/OracleConfiguration/src",
            lab.parent().unwrap().join("v85/native/uha_20260923/native"),
        ),
    ] {
        let comparison = lab.join(format!("convert-uha{}-b4863be7-full-pair-r1/direct-native-sdk-configuration-data-comparison.json", if minor == 20 { "83" } else { "85" }));
        let report: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&comparison).unwrap()).unwrap();
        binding_paths.push(comparison.clone());
        let sdk = lab.join(native_dir);
        let edt = lab.join(edt_dir);
        let version = FormatVersion::new(2, minor);
        let paths: Vec<String> = report["different"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["path"].as_str().unwrap())
            .filter(|p| {
                p.ends_with("CommandInterface.xml")
                    || (p.starts_with("Subsystems/") && !p.contains("/Ext/"))
            })
            .map(str::to_owned)
            .collect();
        assert_eq!(paths.len(), 9);
        for path in &paths {
            let native_path = Path::new(path);
            binding_paths.push(sdk.join(path));
            binding_paths.push(original.join(path));
            let source = if path.ends_with("CommandInterface.xml") {
                edt.join(native_path.parent().unwrap().parent().unwrap())
                    .join("CommandInterface.cmi")
            } else {
                let name = native_path.file_stem().unwrap().to_str().unwrap();
                edt.join(native_path.parent().unwrap())
                    .join(name)
                    .join(format!("{name}.mdo"))
            };
            if !path.ends_with("CommandInterface.xml") {
                let object = descriptor(Format::Edt, &std::fs::read(&source).unwrap(), "Subsystem");
                let reference = picture::unpack(object.get(subsystem::F_PICTURE).unwrap())
                    .unwrap()
                    .0;
                let common = reference.strip_prefix("CommonPicture.").unwrap();
                binding_paths.push(
                    edt.join("CommonPictures")
                        .join(common)
                        .join(format!("{common}.mdo")),
                );
            }
            binding_paths.push(source);
        }
        cohorts.push((profile, version, sdk, edt, original, comparison, paths));
    }
    let before = fingerprint_files(&binding_paths);
    let mut results = Vec::new();
    for (profile, version, sdk, edt, original, comparison, paths) in cohorts {
        let mut interface_count = 0;
        let mut picture_count = 0;
        for path in paths {
            let expected = std::fs::read(sdk.join(&path)).unwrap();
            let original_bytes = std::fs::read(original.join(&path)).unwrap();
            let native_path = Path::new(&path);
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if path.ends_with("CommandInterface.xml") {
                    interface_count += 1;
                    let source = edt
                        .join(native_path.parent().unwrap().parent().unwrap())
                        .join("CommandInterface.cmi");
                    let source_bytes = std::fs::read(&source).unwrap();
                    let root = tempfile::tempdir().unwrap();
                    let config = seed();
                    let input = root.path().join("edt");
                    write(Format::Edt, &config, &input, version);
                    std::fs::write(input.join(CI_EDT), &source_bytes).unwrap();
                    let decoded = read(Format::Edt, &input, version);
                    let output = root.path().join("native");
                    write(Format::Designer, &decoded, &output, version);
                    assert!(
                        std::fs::read(output.join(CI_NATIVE)).unwrap() == expected,
                        "genuine interface native bytes mismatch"
                    );
                    let native_decoded = read(Format::Designer, &output, version);
                    assert!(
                        ci(&native_decoded) == ci(&decoded),
                        "genuine current interface values changed"
                    );
                    let own = root.path().join("own");
                    write(Format::Designer, &native_decoded, &own, version);
                    assert!(std::fs::read(own.join(CI_NATIVE)).unwrap() == expected);
                    std::fs::write(output.join(CI_NATIVE), &original_bytes).unwrap();
                    let original_decoded = read(Format::Designer, &output, version);
                    let original_own = root.path().join("original-own");
                    write(Format::Designer, &original_decoded, &original_own, version);
                    assert!(
                        std::fs::read(original_own.join(CI_NATIVE)).unwrap() == original_bytes,
                        "original native nil/presence spelling changed"
                    );
                } else {
                    picture_count += 1;
                    let name = native_path.file_stem().unwrap().to_str().unwrap();
                    let source = edt
                        .join(native_path.parent().unwrap())
                        .join(name)
                        .join(format!("{name}.mdo"));
                    let source_bytes = std::fs::read(&source).unwrap();
                    let mut object = descriptor(Format::Edt, &source_bytes, "Subsystem");
                    let reference = picture::unpack(object.get(subsystem::F_PICTURE).unwrap())
                        .unwrap()
                        .0
                        .to_owned();
                    let common = reference.strip_prefix("CommonPicture.").unwrap();
                    let cp_path = edt
                        .join("CommonPictures")
                        .join(common)
                        .join(format!("{common}.mdo"));
                    let cp_bytes = std::fs::read(&cp_path).unwrap();
                    let defaults = carrier::common_picture_defaults(&[descriptor(
                        Format::Edt,
                        &cp_bytes,
                        "CommonPicture",
                    )])
                    .unwrap();
                    carrier::resolve(&mut object, &defaults).unwrap();
                    assert!(
                        serialize(Format::Designer, &object, version) == expected,
                        "genuine subsystem native bytes mismatch"
                    );
                    assert_eq!(
                        picture::unpack(object.get(subsystem::F_PICTURE).unwrap())
                            .unwrap()
                            .2,
                        None
                    );
                    let sdk_object = descriptor(Format::Designer, &expected, "Subsystem");
                    assert!(
                        serialize(Format::Designer, &sdk_object, version) == expected,
                        "SDK source-native picture tuple changed"
                    );
                    let original_object =
                        descriptor(Format::Designer, &original_bytes, "Subsystem");
                    assert!(
                        serialize(Format::Designer, &original_object, version) == original_bytes,
                        "source-native picture tuple changed"
                    );
                }
            }));
            let failure_sha256 = outcome.as_ref().err().map(|payload| {
                let message = payload
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| payload.downcast_ref::<&str>().copied())
                    .unwrap_or("non-string panic");
                format!("{:x}", Sha256::digest(message.as_bytes()))
            });
            results.push(serde_json::json!({
                "status": if outcome.is_ok() { "PASS_SCOPED" } else { "FAIL_SCOPED" },
                "failure_sha256": failure_sha256,
                "profile": profile, "version": version.to_string(), "path": path,
                "comparison_path": comparison, "all_case_assertions_passed": outcome.is_ok(),
                "sdk_sha256": format!("{:x}", Sha256::digest(&expected)),
                "original_native_sha256": format!("{:x}", Sha256::digest(&original_bytes)),
            }));
        }
        assert_eq!((interface_count, picture_count), (7, 2));
    }
    let after = fingerprint_files(&binding_paths);
    let unchanged = before == after;
    let passed = unchanged && results.iter().all(|row| row["status"] == "PASS_SCOPED");
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .unwrap();
    serde_json::to_writer_pretty(file, &serde_json::json!({
        "status": if passed { "PASS_SCOPED" } else { "FAIL_SCOPED" }, "scope": "selected 7 CommandInterface and 2 Subsystem metadata pairs per UH profile; not whole-product acceptance",
        "profiles": ["2.20", "2.21"], "pairs": 18, "results": results,
        "source_before": before, "source_after": after, "source_unchanged": unchanged,
    })).unwrap();
    assert!(
        unchanged,
        "source/reference/runtime/executable input changed"
    );
    assert!(
        passed,
        "one or more scoped cases failed; persisted hashes and result retained"
    );
}
