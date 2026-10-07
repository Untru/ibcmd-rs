//! Literal UUID event identities in native emission. The authored owner lists,
//! handler values and native source spelling remain separate typed contracts.
use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, FormEvent, FormRootExtInfo, merge_form_events, merge_table_events},
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
use sha2::{Digest, Sha256};

const LITERAL_BEFORE_WRITE: &str = "9cc34712-da5f-4faa-a653-343d2085fbe8";
const LITERAL_BEFORE_WRITE_SERVER: &str = "bf0ac0e1-bcbb-4dfe-8fc4-0b1923b461a6";

fn event(name: &str) -> FormEvent {
    FormEvent {
        name: name.into(),
        handler: format!("Handle{name}"),
    }
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn semantics(body: &FormBody) -> Vec<u8> {
    serde_json::to_vec(body).unwrap()
}
fn write(body: &FormBody, dialect: FormDialect, version: FormatVersion) -> Vec<u8> {
    with_roundtrip_target(version, || write_form(dialect, body)).unwrap()
}
fn read(bytes: &[u8], dialect: FormDialect, version: FormatVersion) -> FormBody {
    with_source_version(Some(version), || read_form(dialect, bytes)).unwrap()
}
fn payload(bytes: &[u8]) -> Vec<(String, String)> {
    let doc = formats_xml::parse(bytes).unwrap();
    doc.root
        .child("Events")
        .unwrap()
        .children
        .iter()
        .map(|node| (node.attr("name").unwrap().value.clone(), node.text.clone()))
        .collect()
}
fn names(events: Vec<&FormEvent>) -> Vec<String> {
    events.into_iter().map(|event| event.name.clone()).collect()
}
fn fixture(version: FormatVersion) -> FormBody {
    let mut body = FormBody::new();
    // The main typed attribute independently determines native Document owner
    // partitioning; an ExtInfo marker alone is absent from the native format.
    body.data_attributes.push(
        serde_json::from_value(serde_json::json!({
            "name":"Object", "id":1, "title":null, "fill_checking":null,
            "value_type":{"parts":[{"id":"DocumentObject.Example", "qualifier":null}]},
            "main":true, "saved_data":true
        }))
        .unwrap(),
    );
    body.root_ext_info = Some(FormRootExtInfo {
        kind: "form:DocumentFormExtInfo".into(),
        events: vec![
            event("AfterWrite"),
            event("BeforeWrite"),
            event("BeforeWriteAtServer"),
        ],
        user_settings_group: None,
    });
    body.events = vec![
        event("OnOpen"),
        event(LITERAL_BEFORE_WRITE),
        event("OnCreateAtServer"),
        event(LITERAL_BEFORE_WRITE_SERVER),
    ];
    // Use the real EDT reader to construct its current canonical default values.
    read(
        &write(&body, FormDialect::Edt, version),
        FormDialect::Edt,
        version,
    )
}

#[test]
fn document_symbolic_and_literal_identities_interleave_without_semantic_loss() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body = fixture(version);
        body.events[1].handler = "ChangedLiteralHandler".into();
        body.root_ext_info.as_mut().unwrap().events[1].handler = "ChangedDocumentHandler".into();
        let before = semantics(&body);
        let native = write(&body, FormDialect::Designer, version);
        let emitted = payload(&native);
        assert_eq!(
            emitted
                .iter()
                .map(|event| event.0.as_str())
                .collect::<Vec<_>>(),
            [
                "AfterWrite",
                "OnOpen",
                "BeforeWrite",
                "BeforeWriteAtServer",
                LITERAL_BEFORE_WRITE,
                "OnCreateAtServer",
                LITERAL_BEFORE_WRITE_SERVER,
            ]
        );
        assert!(emitted.contains(&(LITERAL_BEFORE_WRITE.into(), "ChangedLiteralHandler".into())));
        assert!(emitted.contains(&("BeforeWrite".into(), "ChangedDocumentHandler".into())));
        assert_eq!(
            semantics(&body),
            before,
            "native emission cannot edit authored IR"
        );
        let returned = read(&native, FormDialect::Designer, version);
        assert_eq!(
            semantics(&returned),
            before,
            "full typed native reread, no normalization"
        );
        assert_eq!(write(&returned, FormDialect::Designer, version), native);
        let edt = write(&returned, FormDialect::Edt, version);
        assert_eq!(semantics(&read(&edt, FormDialect::Edt, version)), before);
    }
}

#[test]
fn uppercase_uuid_keys_are_case_independent_and_equal_keys_remain_stable() {
    let own = [
        event("9CC34712-DA5F-4FAA-A653-343D2085FBE8"),
        event(LITERAL_BEFORE_WRITE),
    ];
    let ext = [event("BeforeWrite")];
    assert_eq!(
        names(merge_form_events(&own, &ext, true)),
        [
            "BeforeWrite",
            "9CC34712-DA5F-4FAA-A653-343D2085FBE8",
            LITERAL_BEFORE_WRITE,
        ]
    );
    assert_eq!(own[0].name, "9CC34712-DA5F-4FAA-A653-343D2085FBE8");
    // Outside Document the symbolic identity equals these literal keys. Stable
    // sorting keeps both root events before the later extension event.
    assert_eq!(
        names(merge_form_events(&own, &ext, false)),
        [
            "9CC34712-DA5F-4FAA-A653-343D2085FBE8",
            LITERAL_BEFORE_WRITE,
            "BeforeWrite",
        ]
    );
}

#[test]
fn unknown_symbolic_and_malformed_uuid_names_keep_authored_fallback() {
    for unknown in [
        "UnknownEvent",
        "9cc34712da5f4faaa653343d2085fbe8",
        "9cc34712_da5f-4faa-a653-343d2085fbe8",
        "9cc34712-da5f-4faa-a653-343d2085fbeg",
        "{9cc34712-da5f-4faa-a653-343d2085fbe8}",
    ] {
        let own = [event("OnCreateAtServer"), event(unknown)];
        let ext = [event("AfterWrite")];
        assert_eq!(
            names(merge_form_events(&own, &ext, true)),
            ["OnCreateAtServer", unknown, "AfterWrite"]
        );
    }
}

#[test]
fn uppercase_literal_names_preserve_current_typed_roundtrip() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let mut body = fixture(version);
        body.events[1].name = LITERAL_BEFORE_WRITE.to_ascii_uppercase();
        body.events[3].name = LITERAL_BEFORE_WRITE_SERVER.to_ascii_uppercase();
        let before = semantics(&body);
        let native = write(&body, FormDialect::Designer, version);
        assert!(
            payload(&native)
                .contains(&(body.events[1].name.clone(), body.events[1].handler.clone()))
        );
        assert_eq!(
            semantics(&read(&native, FormDialect::Designer, version)),
            before
        );
        assert_eq!(semantics(&body), before);
    }
}

#[test]
fn a_single_native_owner_keeps_original_order_even_with_literal_uuid_names() {
    let own = [
        event(LITERAL_BEFORE_WRITE_SERVER),
        event("OnOpen"),
        event(LITERAL_BEFORE_WRITE),
    ];
    assert_eq!(
        names(merge_form_events(&own, &[], true)),
        [LITERAL_BEFORE_WRITE_SERVER, "OnOpen", LITERAL_BEFORE_WRITE]
    );
    assert_eq!(
        names(merge_table_events(&own, &[])),
        [LITERAL_BEFORE_WRITE_SERVER, "OnOpen", LITERAL_BEFORE_WRITE]
    );
}

#[test]
fn native_source_event_order_survives_handler_edits_with_full_typed_roundtrip() {
    for minor in [20, 21] {
        let version = FormatVersion::new(2, minor);
        let native = write(&fixture(version), FormDialect::Designer, version);
        let text = String::from_utf8(native).unwrap();
        let after = "<Event name=\"AfterWrite\">HandleAfterWrite</Event>";
        let open = "<Event name=\"OnOpen\">HandleOnOpen</Event>";
        assert!(text.contains(after) && text.contains(open));
        let reordered = text
            .replace(after, "TEMP_EVENT")
            .replace(open, after)
            .replace("TEMP_EVENT", open);
        let mut source = read(reordered.as_bytes(), FormDialect::Designer, version);
        assert_eq!(
            write(&source, FormDialect::Designer, version),
            reordered.as_bytes()
        );
        source
            .events
            .iter_mut()
            .find(|event| event.name == LITERAL_BEFORE_WRITE)
            .unwrap()
            .handler = "CurrentValue".into();
        let before = semantics(&source);
        let changed = write(&source, FormDialect::Designer, version);
        assert_eq!(payload(&changed)[0].0, "OnOpen");
        assert!(payload(&changed).contains(&(LITERAL_BEFORE_WRITE.into(), "CurrentValue".into())));
        assert_eq!(semantics(&source), before);
        assert_eq!(
            semantics(&read(&changed, FormDialect::Designer, version)),
            before
        );
    }
}

#[test]
#[ignore = "requires unchanged, SHA-bound genuine UH83/UH85 EDT and SDK pairs on F"]
fn genuine_sdk_uuid_events_and_deferred_use_always_contracts() {
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    // Four event pairs exercise two independent Document forms in both formats.
    // Four UseAlways pairs retain evidence of the deferred lossless-IR problem;
    // they are not represented as completed SDK acceptance by this test.
    let witnesses = [
        (
            "83",
            "Documents/Лот/Forms/ФормаДокумента/Ext/Form.xml",
            "ef454943283901e70e29b07879d7921b67cc676a1a1479572fd83bf0ab8f378b",
            "be11b35da5e0ac0a70f90cbbdd44195a909eea4792c4378226299c4440f8961d",
            true,
        ),
        (
            "83",
            "Documents/СтрокаПланаЗакупок/Forms/ФормаДокумента/Ext/Form.xml",
            "b82cbaebf3740100faa5b349316c9f77b0d1c2f66c483bc5e84db2e3f93d25ee",
            "3b81071670c17a1727dcbffb38a3116a8366a2de0736fda6c57a60884a60debe",
            true,
        ),
        (
            "85",
            "Documents/Лот/Forms/ФормаДокумента/Ext/Form.xml",
            "99e489434013e36ceba2984d7378ad85329ccbe80d6c3b20fc714a7bc8280fe2",
            "ee93455c7f80bc694fb05ac5a1c041e89177dc70d834d4c7f6ec8d8689a3c042",
            true,
        ),
        (
            "85",
            "Documents/СтрокаПланаЗакупок/Forms/ФормаДокумента/Ext/Form.xml",
            "caa7a445a9ab4439fdc33ad1cfb943d222803987ca58905e7ca4766bbf51408e",
            "17f6634e35377eb4564fd7fd5abc7d5b904031ed52f7dd4af40e32ed87768ecf",
            true,
        ),
        (
            "83",
            "Catalogs/Лоты/Forms/ФормаСписка/Ext/Form.xml",
            "6374c48ef6d48b86e70309c9b146fcafcf01e71a59ded2c56aa4915c484f399d",
            "5f5a788b54d4af59949e27aa336d627149caebbd23283ee95739b69df790d22f",
            false,
        ),
        (
            "83",
            "Documents/Лот/Forms/ФормаСписка/Ext/Form.xml",
            "1c8203ef508f7102931dbb9448bf4dd552f9c9681423490aec8c7d71f1a8f061",
            "f00c47f2fda583420afa57bc3ac9ad763877cd657759219c31743c54607e8c32",
            false,
        ),
        (
            "85",
            "Catalogs/Лоты/Forms/ФормаСписка/Ext/Form.xml",
            "8f2398b017ead0c713cd8dad061f305644aa6f8bda2ba65138a7f78d52dd3c68",
            "5102571ff076f310e1c2b833c9d114674f1047467afcd1b80544e0a7645b38f8",
            false,
        ),
        (
            "85",
            "Documents/Лот/Forms/ФормаСписка/Ext/Form.xml",
            "f7280a60c01989a4023ecbe58144512db6551c3ffa4f296c21f9d16fec06c481",
            "c3dc1de0fd2f88cb8afd2da7904a1d6f8c4ee0cca8fd48ec1800747bae21b20c",
            false,
        ),
    ];
    let mut events = 0;
    let mut deferred = 0;
    for (profile, relative, source_sha, sdk_sha, event_case) in witnesses {
        let version = FormatVersion::new(2, if profile == "83" { 20 } else { 21 });
        let native_root = lab.join(if profile == "83" {
            "native-reference-uha83-r1/native-xml"
        } else {
            "native-reference-uha85-affinity-r1/native-xml"
        });
        let edt_root = lab.join(if profile == "83" {
            "oracle-uha83-r1/authentic-workspace/OracleConfiguration/src"
        } else {
            "oracle-uha85-r2/authentic-workspace/OracleConfiguration/src"
        });
        let form = std::path::Path::new(relative)
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let source = std::fs::read(edt_root.join(form).join("Form.form")).unwrap();
        let sdk = std::fs::read(native_root.join(relative)).unwrap();
        assert_eq!(sha(&source), source_sha);
        assert_eq!(sha(&sdk), sdk_sha);
        let body = read(&source, FormDialect::Edt, version);
        let before = semantics(&body);
        let expected = read(&sdk, FormDialect::Designer, version);
        if event_case {
            let native = write(&body, FormDialect::Designer, version);
            assert_eq!(payload(&native), payload(&sdk), "{profile} {relative}");
            let returned = read(&native, FormDialect::Designer, version);
            assert_eq!(returned.events, body.events);
            assert_eq!(returned.root_ext_info, body.root_ext_info);
            assert_eq!(expected.events, body.events);
            assert_eq!(expected.root_ext_info, body.root_ext_info);
            events += 1;
        } else {
            let source_paths = &body
                .data_attributes
                .iter()
                .find(|attribute| attribute.name == "Список")
                .unwrap()
                .not_default_use_always;
            let sdk_paths = &expected
                .data_attributes
                .iter()
                .find(|attribute| attribute.name == "Список")
                .unwrap()
                .not_default_use_always;
            assert!(
                source_paths.len() > sdk_paths.len(),
                "authored repeats need lossless preservation, {relative}"
            );
            let native = write(&body, FormDialect::Designer, version);
            let returned = read(&native, FormDialect::Designer, version);
            assert_eq!(
                &returned
                    .data_attributes
                    .iter()
                    .find(|attribute| attribute.name == "Список")
                    .unwrap()
                    .not_default_use_always,
                source_paths
            );
            deferred += 1;
        }
        assert_eq!(semantics(&body), before);
        assert_eq!(
            sha(&std::fs::read(edt_root.join(form).join("Form.form")).unwrap()),
            source_sha
        );
        assert_eq!(
            sha(&std::fs::read(native_root.join(relative)).unwrap()),
            sdk_sha
        );
    }
    assert_eq!((events, deferred), (4, 4));
}
