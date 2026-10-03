use formats_xml::form::{
    FormDialect, apply_data_path_semantics_resource, project_data_path_semantics, read_form,
    write_form,
};
use morph1c_core::{
    ir::{FormBody, PropertyValue, Uuid, form::DataPathSpec},
    spec::forms::controls::form_field as ff,
    version::{FormatVersion, with_roundtrip_target},
};
fn fixture(segments: &str) -> Vec<u8> {
    format!(concat!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\">\r\n",
        "<items xsi:type=\"form:FormField\"><name>Field</name><id>1</id><dataPath xsi:type=\"form:DataPath\">{}</dataPath><type>LabelField</type><extInfo xsi:type=\"form:LabelFieldExtInfo\"/></items>\r\n",
        "</form:Form>\r\n"),segments).into_bytes()
}
fn body() -> FormBody {
    read_form(FormDialect::Edt, &fixture("<segments>List.A</segments>")).unwrap()
}
fn get(body: &FormBody) -> &PropertyValue {
    body.items[0].get(ff::F_DATA_PATH).unwrap()
}
fn uuid() -> Uuid {
    Uuid([7; 16])
}
#[test]
fn original_form_helper_segments_join_split_markers_and_trailing_empty_rules() {
    for (source, expected) in [
        (
            "<segments>~List.~A...</segments>",
            PropertyValue::Ref("List.A".into()),
        ),
        (
            "<segments>List</segments><segments>~A</segments>",
            PropertyValue::Ref("List.A".into()),
        ),
        (
            "<segments>List..A</segments>",
            PropertyValue::Ref("List..A".into()),
        ),
        ("<segments></segments>", PropertyValue::Ref(String::new())),
        (
            "<segments>List.A~literal</segments>",
            PropertyValue::Ref("List.A~literal".into()),
        ),
    ] {
        let current = read_form(FormDialect::Edt, &fixture(source)).unwrap();
        assert_eq!(get(&current), &expected);
    }
    for wrong in [
        "<segments unknown=\"true\">A</segments>",
        "<form:segments>A</form:segments>",
        "<extraPaths>A</extraPaths>",
        "<segments><unknown/></segments>",
    ] {
        assert!(
            read_form(FormDialect::Edt, &fixture(wrong)).is_err(),
            "unknown DataPath shape accepted"
        );
    }
}
#[test]
fn native_ordered_alternates_remain_current_semantic_values() {
    for minor in [20, 21] {
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            let native = write_form(FormDialect::Designer, &body()).unwrap();
            let s = String::from_utf8(native).unwrap().replace(
                "<DataPath>List.A</DataPath>",
                "<DataPath>~List.A~Other.B~  Next.C  ~Other.B</DataPath>",
            );
            let current = read_form(FormDialect::Designer, s.as_bytes()).unwrap();
            assert_eq!(
                get(&current),
                &PropertyValue::DataPath(DataPathSpec {
                    segments: vec!["List".into(), "A".into()],
                    extra_paths: vec!["Other.B".into(), "Next.C".into(), "Other.B".into()]
                })
            );
            let json = serde_json::to_vec(get(&current)).unwrap();
            let mut edited = current.clone();
            let PropertyValue::DataPath(path) = &mut edited.items[0]
                .properties
                .iter_mut()
                .find(|(id, _)| *id == ff::F_DATA_PATH)
                .unwrap()
                .1
            else {
                panic!("rich path missing")
            };
            path.extra_paths.swap(0, 1);
            assert_ne!(json, serde_json::to_vec(get(&edited)).unwrap());
        });
    }
}
#[test]
fn current_rich_paths_survive_both_standard_projections_with_closed_transport() {
    for minor in [20, 21] {
        let profile = FormatVersion::new(2, minor);
        with_roundtrip_target(profile, || {
            for dialect in [FormDialect::Edt, FormDialect::Designer] {
                let mut original = body();
                original.items[0]
                    .properties
                    .iter_mut()
                    .find(|(id, _)| *id == ff::F_DATA_PATH)
                    .unwrap()
                    .1 = PropertyValue::DataPath(DataPathSpec {
                    segments: vec!["List".into(), "A~literal".into()],
                    extra_paths: vec!["Other.B".into(), "Next.C".into(), "Other.B".into()],
                });
                let (projected, resource) =
                    project_data_path_semantics(&original, uuid(), dialect, profile, None)
                        .unwrap()
                        .unwrap();
                let bytes = write_form(dialect, &projected).unwrap();
                let mut read = read_form(dialect, &bytes).unwrap();
                apply_data_path_semantics_resource(&mut read, uuid(), dialect, profile, &resource)
                    .unwrap();
                assert_eq!(get(&read), get(&original));
                let mut edited = original.clone();
                let PropertyValue::DataPath(p) = &mut edited.items[0]
                    .properties
                    .iter_mut()
                    .find(|(id, _)| *id == ff::F_DATA_PATH)
                    .unwrap()
                    .1
                else {
                    unreachable!()
                };
                p.extra_paths.remove(0);
                p.extra_paths.push("CURRENT.D".into());
                let (_, current_resource) =
                    project_data_path_semantics(&edited, uuid(), dialect, profile, None)
                        .unwrap()
                        .unwrap();
                assert_ne!(resource, current_resource);
            }
        });
    }
}
#[test]
fn rich_transport_rejects_current_projection_edits_orphans_unknown_duplicates_and_wrong_profile_atomically()
 {
    let profile = FormatVersion::new(2, 21);
    let mut original = body();
    original.items[0]
        .properties
        .iter_mut()
        .find(|(id, _)| *id == ff::F_DATA_PATH)
        .unwrap()
        .1 = PropertyValue::DataPath(DataPathSpec {
        segments: vec!["List".into(), "A".into()],
        extra_paths: vec!["Other.B".into()],
    });
    with_roundtrip_target(profile, || {
        let (projected, resource) =
            project_data_path_semantics(&original, uuid(), FormDialect::Edt, profile, None)
                .unwrap()
                .unwrap();
        let target = read_form(
            FormDialect::Edt,
            &write_form(FormDialect::Edt, &projected).unwrap(),
        )
        .unwrap();
        let mut changed = target.clone();
        changed.items[0]
            .properties
            .iter_mut()
            .find(|(id, _)| *id == ff::F_DATA_PATH)
            .unwrap()
            .1 = PropertyValue::Ref("CURRENT.X".into());
        let before = serde_json::to_vec(&changed).unwrap();
        assert!(
            apply_data_path_semantics_resource(
                &mut changed,
                uuid(),
                FormDialect::Edt,
                profile,
                &resource
            )
            .is_err()
        );
        assert_eq!(before, serde_json::to_vec(&changed).unwrap());
        let mut deleted = target.clone();
        deleted.items.clear();
        assert!(
            apply_data_path_semantics_resource(
                &mut deleted,
                uuid(),
                FormDialect::Edt,
                profile,
                &resource
            )
            .is_err()
        );
        for mutate in [0, 1, 2, 3] {
            let mut json: serde_json::Value = serde_json::from_slice(&resource).unwrap();
            match mutate {
                0 => {
                    json["records"][0]["current"]["unknown"] = true.into();
                }
                1 => {
                    let row = json["records"][0].clone();
                    json["records"].as_array_mut().unwrap().push(row);
                }
                2 => {
                    json["records"][0]["binding"]["owner"][0]["kind"] = "UnknownOwner".into();
                }
                _ => {
                    json["records"][0]["current"]["segments"][0] = "Unrelated".into();
                }
            }
            let mut copy = target.clone();
            let before = serde_json::to_vec(&copy).unwrap();
            assert!(
                apply_data_path_semantics_resource(
                    &mut copy,
                    uuid(),
                    FormDialect::Edt,
                    profile,
                    &serde_json::to_vec(&json).unwrap()
                )
                .is_err()
            );
            assert_eq!(before, serde_json::to_vec(&copy).unwrap());
        }
        let duplicate = String::from_utf8(resource.clone())
            .unwrap()
            .replace("\"extra_paths\":", "\"extra_paths\":[],\"extra_paths\":");
        assert!(
            apply_data_path_semantics_resource(
                &mut target.clone(),
                uuid(),
                FormDialect::Edt,
                profile,
                duplicate.as_bytes()
            )
            .is_err()
        );
        assert!(
            apply_data_path_semantics_resource(
                &mut target.clone(),
                uuid(),
                FormDialect::Edt,
                FormatVersion::new(2, 20),
                &resource
            )
            .is_err()
        );
    });
}
