use formats_xml::form::{FormDialect, read_form, write_form};
use morph1c_core::{
    ir::{FormBody, PropertyValue},
    version::{FormatVersion, with_roundtrip_target, with_source_version},
};
fn read(dialect: FormDialect, bytes: &[u8]) -> Result<FormBody, formats_xml::form::FormError> {
    with_source_version(Some(FormatVersion::new(2, 20)), || {
        read_form(dialect, bytes)
    })
}
fn write(dialect: FormDialect, body: &FormBody) -> Vec<u8> {
    with_roundtrip_target(FormatVersion::new(2, 20), || write_form(dialect, body)).unwrap()
}
fn edt_source() -> String {
    let attribute = |name: &str, id: i64| {
        format!(
            "<attributes><name>{name}</name><id>{id}</id><valueType><types>SpreadsheetDocument</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:SpreadsheetDocumentExtInfo\"/></attributes>"
        )
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\"><attributes><name>Report</name><id>1</id><valueType><types>ReportObject.Report</types></valueType><main>true</main><view><common>true</common></view><edit><common>true</common></edit></attributes>{}{}<extInfo xsi:type=\"form:ReportFormExtInfo\"><reportResult>3</reportResult><detailsInformation>0</detailsInformation></extInfo></form:Form>\r\n",
        attribute("Result", 3),
        attribute("Other", 4)
    )
}
fn native_source() -> String {
    let body = read(FormDialect::Edt, edt_source().as_bytes()).unwrap();
    String::from_utf8(write(FormDialect::Designer, &body))
        .unwrap()
        .replace(
            "<ReportResult>Result</ReportResult>",
            "<ReportResult xsi:type=\"xs:decimal\">3</ReportResult>",
        )
        .replace(
            "<DetailsData>0</DetailsData>",
            "<DetailsData xsi:type=\"xs:decimal\">0</DetailsData>",
        )
}
fn report() -> FormBody {
    read(FormDialect::Designer, native_source().as_bytes()).unwrap()
}
#[test]
fn declared_numeric_report_refs_are_canonical_and_keep_each_source_spelling() {
    let native = native_source();
    let form = report();
    let info = form.report_form.as_ref().unwrap();
    assert_eq!(info.report_result.as_deref(), Some("Result"));
    assert_eq!(info.details_data.as_deref(), Some("0"));
    assert_eq!(write(FormDialect::Designer, &form), native.as_bytes());
    let edt = write(FormDialect::Edt, &form);
    assert!(String::from_utf8_lossy(&edt).contains("<reportResult>3</reportResult>"));
    let decoded = read(FormDialect::Edt, &edt).unwrap();
    assert_eq!(
        serde_json::to_vec(&form).unwrap(),
        serde_json::to_vec(&decoded).unwrap()
    );
    let sdk = String::from_utf8(write(FormDialect::Designer, &decoded)).unwrap();
    assert!(sdk.contains("<ReportResult>Result</ReportResult>"));
    assert!(sdk.contains("<DetailsData>0</DetailsData>"));
    let mut edited = form.clone();
    edited.report_form.as_mut().unwrap().report_result = Some("Other".into());
    for dialect in [FormDialect::Designer, FormDialect::Edt] {
        let generated = write(dialect, &edited);
        assert_eq!(
            read(dialect, &generated)
                .unwrap()
                .report_form
                .unwrap()
                .report_result
                .as_deref(),
            Some("Other")
        );
    }
    assert_ne!(
        serde_json::to_vec(&form).unwrap(),
        serde_json::to_vec(&edited).unwrap()
    );
}
#[test]
fn report_numeric_refs_require_known_unique_targets_and_exact_qname() {
    let native = native_source();
    for source in [native.replace(">3</ReportResult>",">999</ReportResult>"),native.replace("id=\"4\"","id=\"3\""),native.replace("xsi:type=\"xs:decimal\">3","xsi:type=\"xs:string\">3"),native.replace("<ReportResult xsi:type=\"xs:decimal\">3</ReportResult>","<ReportResult xsi:type=\"xs:decimal\">3</ReportResult><ReportResult>Other</ReportResult>"),native.replace("xmlns:xs=\"http://www.w3.org/2001/XMLSchema\"","xmlns:xs=\"urn:wrong\""),native.replace(">3</ReportResult>",">3.5</ReportResult>")] {
 assert!(read(FormDialect::Designer,source.as_bytes()).is_err());
 }
    assert!(
        read(
            FormDialect::Edt,
            edt_source()
                .replace(
                    "<reportResult>3</reportResult>",
                    "<reportResult>999</reportResult>"
                )
                .as_bytes()
        )
        .is_err()
    );
    assert!(
        read(
            FormDialect::Edt,
            edt_source().replace("<id>4</id>", "<id>3</id>").as_bytes()
        )
        .is_err()
    );
}
#[test]
#[ignore = "Read-only genuine original/EDT/nativeSDK numeric report references on F"]
fn genuine_report_numeric_refs_match_independent_sdk_projection() {
    use std::path::Path;
    let rel = Path::new("Reports/ДвиженияНастраиваемойОтчетности/Forms/ФормаОтчета");
    let original = std::fs::read(
        Path::new(r"F:\ibcmd\lab\04\release-20261001\rc\out\uha8327_db_r1\tree")
            .join(rel)
            .join("Ext/Form.xml"),
    )
    .unwrap();
    let source = read(FormDialect::Designer, &original).unwrap();
    assert_eq!(write(FormDialect::Designer, &source), original);
    let sdk = std::fs::read(
        Path::new(r"F:\ibcmd\lab\07\native-reference-uha83-r1\native-xml")
            .join(rel)
            .join("Ext/Form.xml"),
    )
    .unwrap();
    let expected = read(FormDialect::Designer, &sdk).unwrap();
    let edt = std::fs::read(
        Path::new(r"F:\ibcmd\lab\07\oracle-uha83-r1\authentic-workspace\OracleConfiguration\src")
            .join(rel)
            .join("Form.form"),
    )
    .unwrap();
    let genuine = read(FormDialect::Edt, &edt).unwrap();
    let a = source.report_form.as_ref().unwrap();
    let b = expected.report_form.as_ref().unwrap();
    let c = genuine.report_form.as_ref().unwrap();
    assert_eq!(a.report_result, b.report_result);
    assert_eq!(a.report_result, c.report_result);
    assert_eq!(a.details_data, b.details_data);
    assert_eq!(a.details_data, c.details_data);
    let generated = write(FormDialect::Designer, &genuine);
    let generated = String::from_utf8(generated).unwrap();
    assert!(generated.contains(&format!(
        "<ReportResult>{}</ReportResult>",
        b.report_result.as_ref().unwrap()
    )));
    assert!(generated.contains("<DetailsData>0</DetailsData>"));
}

#[test]
fn public_numeric_report_reference_roundtrip_and_edited_reference_rejects_forged_hash() {
    use ibcmd_edt::{
        ConversionOptions, Project, ReaderLimits, edt_to_xml, read_xml_source, xml_to_edt,
    };
    use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
    use morph1c_core::ir::{MetadataObject, NamedFormBody, ObjectKind, Token, Uuid};
    use morph1c_pipeline::{ConvertOptions, Format, read_config, write_config};
    use sha2::{Digest, Sha256};
    let fixture = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/subsystem-ci/src"
    ));
    let mut cfg = read_config(Format::Designer, fixture, &ConvertOptions::default())
        .unwrap()
        .0;
    let form = report();
    let mut obj = MetadataObject::new(
        ObjectKind::new("CommonForm"),
        "ReportReferences",
        Uuid([42; 16]),
    );
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
        name: "ReportReferences".into(),
        body: form,
        ordinary_body: None,
        module: None,
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
            "</Language>\r\n\t\t\t<CommonForm>ReportReferences</CommonForm>",
        );
    std::fs::write(path, root).unwrap();
    let options = ConversionOptions {
        edt_version: "2025.2.3".into(),
        xml_dialect: "2.21".into(),
        runtime_version: Some("8.5.1".into()),
    };
    let original = read_xml_source(dir.path(), ReaderLimits::default()).unwrap();
    let generated = xml_to_edt(&original, &options).unwrap().tree;
    let returned = edt_to_xml(&Project::from_tree(generated.clone()).unwrap(), &options)
        .unwrap()
        .tree;
    assert_eq!(returned, original);
    let path = "src/CommonForms/ReportReferences/Form.form";
    let bytes = generated
        .entries()
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap()
        .bytes();
    let mut edited_form = with_source_version(Some(FormatVersion::new(2, 21)), || {
        read_form(FormDialect::Edt, bytes)
    })
    .unwrap();
    edited_form.report_form.as_mut().unwrap().report_result = Some("Other".into());
    let edited = with_roundtrip_target(FormatVersion::new(2, 21), || {
        write_form(FormDialect::Edt, &edited_form)
    })
    .unwrap();
    assert!(edited != bytes);
    let hash = format!("{:x}", Sha256::digest(&edited));
    let mut manifest: serde_json::Value = serde_json::from_slice(
        generated
            .entries()
            .iter()
            .find(|e| e.path().as_str() == ".ibcmd-provenance/manifest.json")
            .unwrap()
            .bytes(),
    )
    .unwrap();
    manifest["generated"][path] = serde_json::Value::String(hash);
    let altered = SourceTree::new(
        generated
            .entries()
            .iter()
            .map(|e| {
                if e.path().as_str() == path {
                    SourceEntry::from_bytes(SourcePath::new(path).unwrap(), edited.clone()).unwrap()
                } else if e.path().as_str() == ".ibcmd-provenance/manifest.json" {
                    SourceEntry::from_bytes(
                        e.path().clone(),
                        serde_json::to_vec(&manifest).unwrap(),
                    )
                    .unwrap()
                } else {
                    e.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    assert!(
        edt_to_xml(&Project::from_tree(altered).unwrap(), &options)
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
}
