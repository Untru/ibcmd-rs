//! Scoped source-template projections; no full configuration or SDK process is launched.
use morph1c_core::ir::{MetadataObject, ObjectKind, PropertyValue, Template, Token, Uuid};
use morph1c_core::version::FormatVersion;
use morph1c_pipeline::{ConvertError, ConvertOptions, Format, read_config, write_config};
use sha2::{Digest, Sha256};

fn mxl(inner: &str) -> String {
    format!(
        "<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:core=\"http://v8.1c.ru/8.1/data/core\" xmlns:chart=\"http://v8.1c.ru/8.2/data/chart\" xmlns:i=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:x=\"http://www.w3.org/2001/XMLSchema\">{inner}</document>"
    )
}
#[test]
fn mxl_string_fields_use_xml_literal_newlines_without_touching_entities_or_markup() {
    let inner = "<detailParameter>a\r\nb\rc</detailParameter><v i:type=\"x:string\">d\r\ne&#13;&#10;f</v><chart:valData i:type=\"x:string\">g\rh</chart:valData><core:content>j\rk</core:content><!--keep\r\n--><?keep x?><v i:type=\"x:base64Binary\">a\r\nb</v><foreign xmlns=\"urn:other\"><detailParameter>l\r\nm</detailParameter></foreign>";
    let native =
        morph1c_pipeline::project_mxl_content_newlines(mxl(inner).as_bytes(), false).unwrap();
    let expected = inner
        .replacen("a\r\nb\rc", "a\nb\nc", 1)
        .replace("d\r\ne", "d\ne")
        .replace("g\rh", "g\nh")
        .replace("j\rk", "j\nk");
    assert_eq!(native, mxl(&expected).as_bytes());
    assert_eq!(
        morph1c_pipeline::project_mxl_content_newlines(&native, false).unwrap(),
        native
    );
    let edt = morph1c_pipeline::project_mxl_content_newlines(&native, true).unwrap();
    assert!(String::from_utf8(edt).unwrap().contains("a\r\nb\r\nc"));
    let rebound = mxl(
        "<v xmlns:x=\"urn:other\" i:type=\"x:string\">a\r\nb</v><chart:valData i:type=\"x:decimal\">a\r\nb</chart:valData>",
    );
    assert_eq!(
        morph1c_pipeline::project_mxl_content_newlines(rebound.as_bytes(), false).unwrap(),
        rebound.as_bytes()
    );
}
#[test]
fn dcs_picture_binary_framing_is_protected_by_expanded_type_and_namespace() {
    let source = "<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\" xmlns:c=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:i=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:p=\"http://v8.1c.ru/8.1/data/ui\" xmlns:core=\"http://v8.1c.ru/8.1/data/core\"><core:content>\r\n</core:content><c:value i:type=\"p:Picture\">YWJj\r\nZGVm</c:value><query>a\r\nb</query><c:value xmlns:p=\"urn:other\" i:type=\"p:Picture\">a\r\nb</c:value></DataCompositionSchema>";
    let output = read_dcs_edt(source.as_bytes()).unwrap();
    assert_eq!(
        output,
        source
            .replace("<core:content>\r\n", "<core:content>\n")
            .replace("<query>a\r\nb", "<query>a\nb")
            .replace("i:type=\"p:Picture\">a\r\nb", "i:type=\"p:Picture\">a\nb")
            .as_bytes()
    );
    assert_eq!(write_dcs_edt(&output).unwrap(), source.as_bytes());
    assert!(read_dcs_edt(b"<root><x></root>").is_err());
}
fn read_dcs_edt(body: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let mut cfg = configuration();
    cfg.objects.push(object(
        "Current",
        "DataCompositionSchema",
        b"<DataCompositionSchema xmlns=\"http://v8.1c.ru/8.1/data-composition-system/schema\"/>"
            .to_vec(),
    ));
    let edt = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &cfg, edt.path())?;
    std::fs::write(
        edt.path().join("CommonTemplates/Current/Template.dcs"),
        body,
    )
    .unwrap();
    let current = read_config(Format::Edt, edt.path(), &ConvertOptions::default())?.0;
    Ok(body_bytes(&current).to_vec())
}
fn write_dcs_edt(body: &[u8]) -> Result<Vec<u8>, ConvertError> {
    let mut cfg = configuration();
    cfg.objects
        .push(object("Current", "DataCompositionSchema", body.to_vec()));
    let edt = tempfile::tempdir().unwrap();
    write_config(Format::Edt, &cfg, edt.path())?;
    Ok(std::fs::read(edt.path().join("CommonTemplates/Current/Template.dcs")).unwrap())
}
fn configuration() -> morph1c_core::ir::Configuration {
    read_config(
        Format::Designer,
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/subsystem-ci/src"
        )),
        &ConvertOptions::default(),
    )
    .unwrap()
    .0
}
fn object(name: &str, kind: &str, bytes: Vec<u8>) -> MetadataObject {
    let mut obj = MetadataObject::new(ObjectKind::new("CommonTemplate"), name, Uuid([9; 16]));
    obj.properties.push((
        morph1c_core::spec::metadata::common_template::F_TEMPLATE_TYPE,
        PropertyValue::Enum(Token::new(kind)),
    ));
    obj.templates.push(Template {
        name: name.into(),
        properties: vec![],
        pages: vec![],
        resources: vec![],
        body: Some(bytes),
    });
    obj
}
#[test]
fn current_template_edits_remain_authoritative_in_both_profiles() {
    for minor in [20, 21] {
        let options = ConvertOptions::default().with_target_version(FormatVersion::new(2, minor));
        let source = mxl("<detailParameter>first\nsecond</detailParameter>");
        let mut cfg = configuration();
        cfg.objects.push(object(
            "Current",
            "SpreadsheetDocument",
            source.as_bytes().to_vec(),
        ));
        let edt = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &cfg, edt.path()).unwrap();
        let mut current = read_config(Format::Edt, edt.path(), &options).unwrap().0;
        let body = current
            .objects
            .iter_mut()
            .find(|o| o.name == "Current")
            .unwrap()
            .templates[0]
            .body
            .as_mut()
            .unwrap();
        *body = mxl("<detailParameter>changed\nordered\nvalue</detailParameter>").into_bytes();
        let native = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &current, native.path()).unwrap();
        let bytes = std::fs::read(
            native
                .path()
                .join("CommonTemplates/Current/Ext/Template.xml"),
        )
        .unwrap();
        assert_eq!(
            bytes,
            [b"\xef\xbb\xbf".as_slice(), body_bytes(&current)].concat()
        );
        let reread = read_config(Format::Designer, native.path(), &options)
            .unwrap()
            .0;
        assert_eq!(body_bytes(&reread), body_bytes(&current));
    }
}
fn body_bytes(cfg: &morph1c_core::ir::Configuration) -> &[u8] {
    cfg.objects
        .iter()
        .find(|o| o.name == "Current")
        .unwrap()
        .templates[0]
        .body
        .as_deref()
        .unwrap()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
#[ignore = "opt-in exact 45-template source/SDK census; manifests contain private paths only in F lab"]
fn genuine_all_45_templates_match_sdk_native_bytes() {
    let manifest =
        std::path::PathBuf::from(std::env::var_os("IBCMD_TEMPLATE_MANIFEST").expect("manifest"));
    let report =
        std::path::PathBuf::from(std::env::var_os("IBCMD_TEMPLATE_REPORT").expect("report"));
    assert!(!report.exists());
    let input = std::fs::read(&manifest).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&input).unwrap();
    let rows = json["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 45);
    let mut reports = vec![];
    for (index, row) in rows.iter().enumerate() {
        let path = |key: &str| std::path::PathBuf::from(row[key].as_str().unwrap());
        let source = path("source");
        let expected = path("expected");
        let historical = path("actual");
        let bytes = std::fs::read(&source).unwrap();
        let sdk = std::fs::read(&expected).unwrap();
        let old = std::fs::read(&historical).unwrap();
        assert_eq!(hash(&bytes), row["source_sha256"].as_str().unwrap());
        assert_eq!(hash(&sdk), row["expected_sha256"].as_str().unwrap());
        assert_eq!(hash(&old), row["actual_sha256"].as_str().unwrap());
        let kind = row["template_kind"].as_str().unwrap();
        let edt_file = if kind == "SpreadsheetDocument" {
            "Template.mxlx"
        } else {
            "Template.dcs"
        };
        let name = format!("Template{index}");
        let mut cfg = configuration();
        // Descriptor write is production code; replace only this fresh source body
        // with the exact authentic EDT bytes, then production read/emit.
        cfg.objects.push(object(&name, kind, bytes.clone()));
        let edt = tempfile::tempdir().unwrap();
        write_config(Format::Edt, &cfg, edt.path()).unwrap();
        std::fs::write(
            edt.path()
                .join("CommonTemplates")
                .join(&name)
                .join(edt_file),
            &bytes,
        )
        .unwrap();
        let current = read_config(Format::Edt, edt.path(), &ConvertOptions::default())
            .unwrap()
            .0;
        let native = tempfile::tempdir().unwrap();
        write_config(Format::Designer, &current, native.path()).unwrap();
        let output = std::fs::read(
            native
                .path()
                .join("CommonTemplates")
                .join(&name)
                .join("Ext/Template.xml"),
        )
        .unwrap();
        let source_after = std::fs::read(&source).unwrap();
        let expected_after = std::fs::read(&expected).unwrap();
        let old_after = std::fs::read(&historical).unwrap();
        assert_eq!(bytes, source_after);
        assert_eq!(sdk, expected_after);
        assert_eq!(old, old_after);
        reports.push(serde_json::json!({"index":index,"path_sha256":row["path_sha256"],"template_kind":kind,"source_sha256":hash(&bytes),"expected_sha256":hash(&sdk),"historical_sha256":hash(&old),"output_sha256":hash(&output),"expected_bytes":sdk.len(),"output_bytes":output.len(),"exact":sdk==output,"sources_unchanged":true}));
    }
    let failures = reports.iter().filter(|r| r["exact"] != true).count();
    std::fs::write(report,serde_json::to_vec_pretty(&serde_json::json!({"scope":"45 template source codecs only; not whole configuration/SDK acceptance","manifest_sha256":hash(&input),"processed":reports.len(),"failures":failures,"complete":true,"rows":reports})).unwrap()).unwrap();
    assert_eq!(failures, 0);
}
