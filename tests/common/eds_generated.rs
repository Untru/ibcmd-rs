// Hand-authored generated sources; fresh identities, no retained foreign CF/XML.
pub const OBJECT_UUID: &str = "43500000-0000-4000-8000-000000000001";
pub const CONFIGURATION_UUID: &str = "43500000-0000-4000-8000-000000000008";
pub const JOB_UUID: &str = "43500000-0000-4000-8000-000000000009";
pub const MODULE_UUID: &str = "43500000-0000-4000-8000-00000000000a";
pub const NAME: &str = "Source435";
pub const PAIRS: [(&str, &str); 3] = [
    (
        "43500000-0000-4000-8000-000000000002",
        "43500000-0000-4000-8000-000000000003",
    ),
    (
        "43500000-0000-4000-8000-000000000004",
        "43500000-0000-4000-8000-000000000005",
    ),
    (
        "43500000-0000-4000-8000-000000000006",
        "43500000-0000-4000-8000-000000000007",
    ),
];
pub fn document(version: &str, object: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="{version}">{object}</MetaDataObject>"#
    )
}
pub fn external_data_source(version: &str, mode: &str, order: [usize; 3]) -> String {
    let mut info = String::new();
    for index in order {
        let (category, stem) = [
            ("Manager", "ExternalDataSourceManager"),
            ("TablesManager", "ExternalDataSourceTablesManager"),
            ("CubesManager", "ExternalDataSourceCubesManager"),
        ][index];
        let (type_id, value_id) = PAIRS[index];
        info.push_str(&format!(r#"<xr:GeneratedType name="{stem}.{NAME}" category="{category}"><xr:TypeId>{type_id}</xr:TypeId><xr:ValueId>{value_id}</xr:ValueId></xr:GeneratedType>"#));
    }
    document(
        version,
        &format!(
            r#"<ExternalDataSource uuid="{OBJECT_UUID}"><InternalInfo>{info}</InternalInfo><Properties><Name>{NAME}</Name><Synonym><v8:item><v8:lang>en</v8:lang><v8:content>Fresh source</v8:content></v8:item></Synonym><Comment>line1&#10;quoted "text"</Comment><DataLockControlMode>{mode}</DataLockControlMode></Properties><ChildObjects/></ExternalDataSource>"#
        ),
    )
}
pub fn configuration(version: &str) -> String {
    let internal = configuration_internal_info();
    let compatibility = if version == "2.20" {
        "Version8_3_27"
    } else {
        "Version8_5_1"
    };
    document(
        version,
        &format!(
            r#"<Configuration uuid="{CONFIGURATION_UUID}">{internal}<Properties><Name>Generated435</Name><Synonym/><Comment>Hand authored</Comment><DefaultRunMode>ManagedApplication</DefaultRunMode><ScriptVariant>English</ScriptVariant><CompatibilityMode>{compatibility}</CompatibilityMode></Properties><ChildObjects><ScheduledJob>Job435</ScheduledJob><ExternalDataSource>{NAME}</ExternalDataSource></ChildObjects></Configuration>"#
        ),
    )
}

// Derive all seven contained identities from the real compiler registry. The
// generated source owns CONFIGURATION_UUID; no native/foreign UUIDs or second
// list of Configuration class IDs is embedded in this fixture.
fn configuration_internal_info() -> String {
    use ibcmd_core::{
        artifact::ProfileId,
        diagnostic::{ObjectPath, PathSegment, PropertyPath},
        identity::{LogicalIdentity, ObjectUuid},
        model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind},
        provenance::{CanonicalAnchor, SourceProvenance},
        validate::validate_configuration,
    };
    use product::compiler::{
        graph::{ObjectStorageRoute, build_bootstrap_graph},
        identity::collect_bootstrap_identities,
        root::{ConfigurationBodyProperties, compile_configuration_body},
        version::SpecialEntryProfile,
    };
    let profile_id = ProfileId::parse("platform-8.3.27.1989").unwrap();
    let path = ObjectPath::new(vec![PathSegment::name("Configuration").unwrap()]).unwrap();
    let object = CanonicalObject::new(CanonicalObjectParts::new(
        LogicalIdentity::new(ObjectUuid::parse(CONFIGURATION_UUID).unwrap(), path.clone()),
        MetadataKind::new("Configuration").unwrap(),
        SourceProvenance::new(
            profile_id.clone(),
            CanonicalAnchor::new(path, PropertyPath::root()),
        ),
    ))
    .unwrap();
    let configuration = CanonicalConfiguration::new(vec![object]).unwrap();
    let validated = validate_configuration(&configuration).unwrap();
    let identities = collect_bootstrap_identities(&validated).unwrap();
    let routes = identities
        .objects()
        .iter()
        .map(|object| ObjectStorageRoute::new(object.uuid(), vec![]).unwrap())
        .collect();
    let graph = build_bootstrap_graph(&identities, profile_id.clone(), routes).unwrap();
    let registry = product::profile_registry::load_bundled_profile_registry().unwrap();
    let profile = SpecialEntryProfile::from_effective(registry.get(&profile_id).unwrap()).unwrap();
    let properties = ConfigurationBodyProperties::minimal("Generated435", profile.compatibility());
    let entry = compile_configuration_body(&identities, &graph, &profile, &properties).unwrap();
    let packed = entry.outcome().compiled_payload().unwrap().bytes();
    let mut plain = Vec::new();
    std::io::Read::read_to_end(&mut flate2::read::DeflateDecoder::new(packed), &mut plain).unwrap();
    let root = product::metadata_model::brace::parse_row(&plain).unwrap();
    let fields = root.as_list().unwrap();
    let count = fields[2].as_atom().unwrap().parse::<usize>().unwrap();
    assert_eq!(count, 7);
    let mut classes = std::collections::BTreeSet::new();
    let mut identities = std::collections::BTreeSet::new();
    let mut info = String::from("<InternalInfo>");
    // Exact owner identity paths of the seven compiler sections, in the
    // compiler's declared order (root properties, directory, zero/direct).
    // Header descendants can contain other {1,0,UUID} tuples; they are not
    // contained owners and must never be collected by recursive tuple search.
    let owner_paths: [&[usize]; 7] = [
        &[1, 1, 1, 1, 1],
        &[1, 1, 1, 0],
        &[1, 1, 1],
        &[1, 1, 0],
        &[1, 1, 1],
        &[1, 1, 0],
        &[1, 1, 0],
    ];
    for (section, owner_path) in fields[3..3 + count].iter().zip(owner_paths) {
        let class = section.at(&[0]).unwrap().as_atom().unwrap();
        assert!(classes.insert(class));
        let owner = section.at(owner_path).unwrap().as_list().unwrap();
        assert_eq!(owner.len(), 3);
        assert_eq!(owner[0].as_atom(), Some("1"));
        assert_eq!(owner[1].as_atom(), Some("0"));
        let identity = owner[2].as_atom().unwrap();
        ObjectUuid::parse(identity).unwrap();
        assert!(identities.insert(identity));
        info.push_str(&format!("<xr:ContainedObject><xr:ClassId>{class}</xr:ClassId><xr:ObjectId>{identity}</xr:ObjectId></xr:ContainedObject>"));
    }
    info.push_str("</InternalInfo>");
    info
}
pub fn scheduled_job(version: &str) -> String {
    document(
        version,
        &format!(
            r#"<ScheduledJob uuid="{JOB_UUID}"><Properties><Name>Job435</Name><Synonym/><Comment/><MethodName/><Key/><Use>false</Use><Predefined>false</Predefined><RestartCountOnFailure>0</RestartCountOnFailure><RestartIntervalOnFailure>0</RestartIntervalOnFailure><Description/></Properties><ChildObjects/></ScheduledJob>"#
        ),
    )
}

pub fn handler_module(version: &str) -> String {
    document(
        version,
        &format!(
            r#"<CommonModule uuid="{MODULE_UUID}"><Properties><Name>Handler435</Name><Synonym/><Comment/><Global>false</Global><ClientManagedApplication>false</ClientManagedApplication><Server>true</Server><ExternalConnection>false</ExternalConnection><ClientOrdinaryApplication>false</ClientOrdinaryApplication><ServerCall>false</ServerCall><Privileged>false</Privileged><ReturnValuesReuse>DontUse</ReturnValuesReuse></Properties></CommonModule>"#,
        ),
    )
}
