//! Clean-room mutations of the public writable tiny database; never native/private inputs.
#![allow(dead_code)]
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase")
}
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
pub fn mutate_descriptor(
    db: &Path,
    id: &str,
    mutate: impl FnOnce(&mut ibcmd_rs::metadata_model::brace::Brace),
) {
    use ibcmd_rs::metadata_model::brace::{parse_row, serialize_row};
    use std::io::Write;
    let mut source = ibcmd_filedb::FileDb::open(db).unwrap();
    let mut rows = source.config_rows("CONFIG").unwrap();
    drop(source);
    let row = rows.iter_mut().find(|row| row.file_name == id).unwrap();
    let mut plain = Vec::new();
    flate2::read::DeflateDecoder::new(row.binary.as_slice())
        .read_to_end(&mut plain)
        .unwrap();
    let mut descriptor = parse_row(&plain).unwrap();
    mutate(&mut descriptor);
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    packed.write_all(&serialize_row(&descriptor)).unwrap();
    row.binary = packed.finish().unwrap();
    row.data_size = row.binary.len() as u64;
    ibcmd_filedb::replace_config_table(db, "CONFIG", &rows).unwrap();
}

pub fn install_unadmitted_common_module_descriptor(db: &Path) {
    use ibcmd_rs::metadata_model::brace::Brace;
    mutate_descriptor(db, "6d1c2a0e-1cd0-4f00-9a00-000000000001", |descriptor| {
        let fields = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
        assert_eq!(fields[0].as_atom(), Some("12"));
        assert_eq!(fields[8].as_atom(), Some("0"));
        // A clean-room unknown ReturnValuesReuse discriminator leaves the object's
        // identity and its independent, valid module body intact.
        fields[8] = Brace::num(99);
    });
}

fn install_synthetic_object(
    db: &Path,
    kind: &str,
    name: &str,
    uuid: &str,
    object: &[u8],
    body: Option<&[u8]>,
) {
    use ibcmd_rs::metadata_model::{DescriptorContext, compile_descriptor, root};
    use std::{io::Write, sync::Arc};
    let source = fixture().join("tiny-source");
    let configuration = fs::read_to_string(source.join("Configuration.xml"))
        .unwrap()
        .replace(
            "</ChildObjects>",
            &format!("<{kind}>{name}</{kind}></ChildObjects>"),
        )
        .into_bytes();
    let mut files = tree(&source)
        .into_iter()
        .filter(|(name, _)| name.ends_with(".xml"))
        .map(|(name, bytes)| (source.join(name), Arc::new(bytes)))
        .collect::<Vec<_>>();
    *files
        .iter_mut()
        .find(|(path, _)| *path == source.join("Configuration.xml"))
        .unwrap() = (
        source.join("Configuration.xml"),
        Arc::new(configuration.clone()),
    );
    let object_path = source.join(format!("{kind}s/{name}.xml"));
    files.push((object_path.clone(), Arc::new(object.to_vec())));
    let context = DescriptorContext::with_files(&source, "2.21", &files).unwrap();
    let configuration_uuid = root::configuration_facts(&configuration).unwrap().uuid;
    let configuration = compile_descriptor(
        "Configuration",
        &source.join("Configuration.xml"),
        &configuration,
        &context,
    )
    .unwrap();
    let object = compile_descriptor(kind, &object_path, object, &context).unwrap();
    let mut source = ibcmd_filedb::FileDb::open(db).unwrap();
    let mut rows = source.config_rows("CONFIG").unwrap();
    drop(source);
    let template = rows[0].clone();
    let mut replace = |name: String, bytes: &[u8]| {
        let mut packed =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        packed.write_all(bytes).unwrap();
        let mut row = template.clone();
        row.file_name = name.clone();
        row.binary = packed.finish().unwrap();
        row.data_size = row.binary.len() as u64;
        if let Some(old) = rows.iter_mut().find(|row| row.file_name == name) {
            *old = row;
        } else {
            rows.push(row);
        }
    };
    replace(configuration_uuid, &configuration);
    replace(uuid.into(), &object);
    if let Some(body) = body {
        replace(format!("{uuid}.0"), body);
    }
    let versions = root::versions_row(
        &rows
            .iter()
            .map(|row| row.file_name.clone())
            .collect::<Vec<_>>(),
        || uuid::Uuid::new_v4().to_string(),
    );
    let versions_row = rows
        .iter_mut()
        .find(|row| row.file_name == "versions")
        .unwrap();
    let mut packed = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    packed.write_all(&versions).unwrap();
    versions_row.binary = packed.finish().unwrap();
    versions_row.data_size = versions_row.binary.len() as u64;
    ibcmd_filedb::replace_config_table(db, "CONFIG", &rows).unwrap();
}

pub fn install_unadmitted_form_asset(db: &Path, opaque: bool) {
    let form_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaa1";
    let form = format!(r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><CommonForm uuid="{form_uuid}"><Properties><Name>OpaqueForm</Name><FormType>Managed</FormType><IncludeHelpInContents>false</IncludeHelpInContents><UseStandardCommands>false</UseStandardCommands></Properties></CommonForm></MetaDataObject>"#).into_bytes();
    // This clean-room managed-form body has a valid module and a diagnosed,
    // unsupported dynamic-list property. The form XML must be withheld.
    let attribute = format!(
        r##"{{9,{{17}},0,"List",{{1,0}},{{"Pattern",{{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}}}},{{0,{{0,{{"B",1}},0}}}},{{0,{{0,{{"B",1}},0}}}},{{0,0}},{{0,0}},0,0,0,0,{{0,3,"MainTable",{{"#",fc01b5df-97fe-449b-83d4-218a090e681e,11111111-1111-4111-8111-111111111111}},"AutoSaveUserSettings",{{"B",1}},"ItemsViewMode",{{"S","{}"}}}},{{0,0}}}}"##,
        '\u{1}'
    );
    let opaque_radio = r#"{37,{38,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,"OpaqueRadio",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{7,3,0,1,100},{3,4,{0}},{3,4,{0}},{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{8,{9,2}}}"#;
    let body = if opaque {
        // A diagnosed opaque ChoiceList is normally an observational report,
        // rather than a conversion error. Selected export must reject it too.
        format!("{{4,{{59,1,cccccccc-cccc-4ccc-8ccc-cccccccccccc,{opaque_radio}}},\"Procedure KeepMe()\r\nEndProcedure\r\n\",{{0}}}}")
    } else {
        format!("{{4,{{0}},\"Procedure KeepMe()\r\nEndProcedure\r\n\",{{4,1,{attribute}}}}}")
    }.into_bytes();
    install_synthetic_object(
        db,
        "CommonForm",
        "OpaqueForm",
        form_uuid,
        &form,
        Some(&body),
    );
}

pub fn install_legacy_fallback_object(
    db: &Path,
    command: bool,
    invalid_slot: Option<usize>,
) -> String {
    use ibcmd_rs::metadata_model::brace::Brace;
    let uuid = "ffffffff-ffff-4fff-8fff-ffffffffffff";
    let (kind, name, properties) = if command {
        (
            "CommonCommand",
            "LegacyCommand",
            "<Group>ActionsPanelTools</Group><Representation>Auto</Representation><Picture/><ToolTip/><Shortcut/><IncludeHelpInContents>false</IncludeHelpInContents><CommandParameterType/><ParameterUseMode>Single</ParameterUseMode><ModifiesData>false</ModifiesData><OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>",
        )
    } else {
        (
            "Subsystem",
            "LegacySubsystem",
            "<IncludeHelpInContents>false</IncludeHelpInContents><IncludeInCommandInterface>true</IncludeInCommandInterface><UseOneCommand>false</UseOneCommand><Picture/><Explanation/><Content/>",
        )
    };
    let xml = format!(
        r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><{kind} uuid="{uuid}"><Properties><Name>{name}</Name>{properties}</Properties><ChildObjects/></{kind}></MetaDataObject>"#
    );
    install_synthetic_object(db, kind, name, uuid, xml.as_bytes(), None);
    mutate_descriptor(db, uuid, |descriptor| {
        let payload = descriptor.as_list_mut().unwrap()[1].as_list_mut().unwrap();
        let fields = if command {
            let body = payload[1].as_list_mut().unwrap();
            let fields = body[2].as_list_mut().unwrap();
            assert_eq!(fields[0].as_atom(), Some("9"));
            assert_eq!(fields.pop().unwrap().as_atom(), Some("0"));
            fields[0] = Brace::num(8);
            fields
        } else {
            assert_eq!(payload[0].as_atom(), Some("22"));
            payload[0] = Brace::num(21);
            payload
        };
        if let Some(slot) = invalid_slot {
            if slot == fields.len() {
                fields.push(Brace::num(99));
            } else {
                fields[slot] = Brace::num(99);
            }
        }
    });
    format!("{kind}.{name}")
}
