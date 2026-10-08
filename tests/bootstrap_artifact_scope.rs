//! Generated cleanroom controls for artifact identity/graph seams only.
use ibcmd_core::{
    artifact::ProfileId,
    diagnostic::{ObjectPath, PathSegment, PropertyPath},
    identity::{LogicalIdentity, ObjectUuid},
    model::{
        CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, GeneratedType,
        GeneratedTypeKind, MetadataKind,
    },
    provenance::{CanonicalAnchor, SourceProvenance},
    validate::validate_configuration,
};
use ibcmd_rs::compiler::{
    artifact::{ArtifactScope, PackageIntent},
    graph::{ObjectStorageRoute, StorageSuffix, build_artifact_graph, build_bootstrap_graph},
    identity::{collect_artifact_identities, collect_bootstrap_identities},
};
use ibcmd_xml::{XmlReader, metadata::inspect_package_identity};
fn uuid(n: u32) -> ObjectUuid {
    ObjectUuid::parse(&format!("73000000-0000-4000-8000-{n:012x}")).unwrap()
}
fn object(n: u32, kind: &str, owner: Option<u32>, generated: &[u32]) -> CanonicalObject {
    let path = ObjectPath::new(vec![PathSegment::name(&format!("Own{n}")).unwrap()]).unwrap();
    let provenance = SourceProvenance::new(
        ProfileId::parse("own:scope").unwrap(),
        CanonicalAnchor::new(path.clone(), PropertyPath::root()),
    );
    let mut parts = CanonicalObjectParts::new(
        LogicalIdentity::new(uuid(n), path),
        MetadataKind::new(kind).unwrap(),
        provenance,
    );
    parts.owner = owner.map(uuid);
    parts.generated_types = generated
        .iter()
        .map(|n| GeneratedType::new(uuid(*n), GeneratedTypeKind::new("Object").unwrap()))
        .collect();
    CanonicalObject::new(parts).unwrap()
}
fn scope(kind: &str, main: u32, contained: u32) -> ArtifactScope {
    let class = match kind {
        "ExternalDataProcessor" => ibcmd_rs::external::ExternalKind::DataProcessor.class_id(),
        _ => ibcmd_rs::external::ExternalKind::Report.class_id(),
    };
    let xml = format!(
        r#"<m:MetaDataObject xmlns:m="http://v8.1c.ru/8.3/MDClasses" xmlns:x="http://v8.1c.ru/8.3/xcf/readable" version="2.20"><m:{kind} uuid="{}"><m:InternalInfo><x:ContainedObject><x:ClassId>{class}</x:ClassId><x:ObjectId>{}</x:ObjectId></x:ContainedObject></m:InternalInfo><m:Properties><m:Name>Own</m:Name></m:Properties></m:{kind}></m:MetaDataObject>"#,
        uuid(main),
        uuid(contained)
    );
    ArtifactScope::from_source_identity(
        inspect_package_identity(&XmlReader::from_slice(xml.as_bytes()).unwrap())
            .unwrap()
            .unwrap(),
    )
    .unwrap()
}
#[test]
fn external_primary_alias_preserves_actual_contained_module_owner_and_exact_services() {
    for (kind, internal) in [
        ("ExternalDataProcessor", "DataProcessor"),
        ("ExternalReport", "Report"),
    ] {
        let scope = scope(kind, 10, 1);
        let model = CanonicalConfiguration::new(vec![
            object(1, internal, None, &[101]),
            object(2, "Form", Some(1), &[]),
        ])
        .unwrap();
        let validated = validate_configuration(&model).unwrap();
        let identities = collect_artifact_identities(&validated, scope).unwrap();
        assert_eq!(identities.scope().root_uuid(), uuid(1));
        assert_eq!(identities.scope().main_uuid(), uuid(10));
        let graph = build_artifact_graph(
            &identities,
            ProfileId::parse("own:scope").unwrap(),
            vec![
                ObjectStorageRoute::new(
                    uuid(1),
                    vec![
                        StorageSuffix::new(".0").unwrap(),
                        StorageSuffix::new(".1").unwrap(),
                    ],
                )
                .unwrap(),
                ObjectStorageRoute::new(uuid(2), vec![]).unwrap(),
            ],
        )
        .unwrap();
        let keys = graph
            .inventory_keys()
            .map(|k| k.as_str().to_owned())
            .collect::<Vec<_>>();
        let mut expected = vec![
            uuid(10).to_string(),
            format!("{}.0", uuid(1)),
            format!("{}.1", uuid(1)),
            uuid(2).to_string(),
            "root".into(),
            "version".into(),
            "versions".into(),
            "copyinfo".into(),
        ];
        expected.sort();
        assert_eq!(keys, expected);
        assert!(!graph.contains_key(&uuid(1).to_string()));
        assert_eq!(
            graph.primary_object_entry(uuid(1)).unwrap().key().as_str(),
            uuid(10).to_string()
        );
        assert!(
            graph
                .object_entry(uuid(1), &StorageSuffix::new(".0").unwrap())
                .is_some()
        );
        graph.validate_special_references().unwrap();
        assert!(graph.require_configuration_scope().is_err());
        assert!(
            build_bootstrap_graph(&identities, ProfileId::parse("own:scope").unwrap(), vec![])
                .is_err()
        );
        assert!(collect_bootstrap_identities(&validated).is_err());
    }
}
#[test]
fn external_main_alias_cannot_collide_with_objects_generated_types_or_foreign_roots() {
    let selected = scope("ExternalDataProcessor", 10, 1);
    for extra in [
        object(10, "Attribute", Some(1), &[]),
        object(2, "Attribute", Some(1), &[10]),
        object(2, "Catalog", None, &[]),
    ] {
        let model = CanonicalConfiguration::new(vec![object(1, "DataProcessor", None, &[]), extra])
            .unwrap();
        assert!(
            collect_artifact_identities(&validate_configuration(&model).unwrap(), selected)
                .is_err()
        );
    }
    for root in [
        object(1, "Report", None, &[]),
        object(2, "DataProcessor", None, &[]),
    ] {
        let model = CanonicalConfiguration::new(vec![root]).unwrap();
        assert!(
            collect_artifact_identities(&validate_configuration(&model).unwrap(), selected)
                .is_err()
        );
    }
}
#[test]
fn extension_version_change_retains_scope_and_replaces_only_service_inventory() {
    let model = CanonicalConfiguration::new(vec![
        object(1, "Configuration", None, &[]),
        object(2, "CommonModule", None, &[]),
    ])
    .unwrap();
    let validated = validate_configuration(&model).unwrap();
    let mut scopes = vec![];
    for version in ["1.0.0", "1.0.1"] {
        let xml = format!(
            r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21"><Configuration uuid="{}"><Properties><ConfigurationExtensionPurpose>Customization</ConfigurationExtensionPurpose><Version>{version}</Version></Properties></Configuration></MetaDataObject>"#,
            uuid(1)
        );
        let scope = ArtifactScope::from_source_identity(
            inspect_package_identity(&XmlReader::from_slice(xml.as_bytes()).unwrap())
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(scope.intent(), PackageIntent::Extension);
        scopes.push(scope);
        let ids = collect_artifact_identities(&validated, scope).unwrap();
        let graph = build_artifact_graph(
            &ids,
            ProfileId::parse("own:scope").unwrap(),
            vec![
                ObjectStorageRoute::new(uuid(1), vec![]).unwrap(),
                ObjectStorageRoute::new(uuid(2), vec![]).unwrap(),
            ],
        )
        .unwrap();
        assert!(graph.contains_key("configinfo"));
        for key in ["root", "version", "versions", "copyinfo"] {
            assert!(!graph.contains_key(key));
        }
        assert!(graph.require_configuration_scope().is_err());
    }
    assert_eq!(scopes[0], scopes[1]);
    let ordinary = collect_bootstrap_identities(&validated).unwrap();
    let graph = build_bootstrap_graph(
        &ordinary,
        ProfileId::parse("own:scope").unwrap(),
        vec![
            ObjectStorageRoute::new(uuid(1), vec![]).unwrap(),
            ObjectStorageRoute::new(uuid(2), vec![]).unwrap(),
        ],
    )
    .unwrap();
    graph.require_configuration_scope().unwrap();
    assert!(!graph.contains_key("configinfo"));
    for key in ["root", "version", "versions"] {
        assert!(graph.contains_key(key));
    }
}
#[test]
fn package_identity_rejects_wrong_namespace_duplicate_missing_and_nil_contained_ids() {
    let valid = format!(
        r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:x="http://v8.1c.ru/8.3/xcf/readable"><ExternalDataProcessor uuid="{}"><InternalInfo><x:ContainedObject><x:ClassId>{}</x:ClassId><x:ObjectId>{}</x:ObjectId></x:ContainedObject></InternalInfo></ExternalDataProcessor></MetaDataObject>"#,
        uuid(10),
        ibcmd_rs::external::ExternalKind::DataProcessor.class_id(),
        uuid(1)
    );
    let inspect = |s: &str| inspect_package_identity(&XmlReader::from_slice(s.as_bytes()).unwrap());
    for bad in [
        valid.replace("http://v8.1c.ru/8.3/xcf/readable", "urn:foreign"),
        valid.replace(&format!("<x:ObjectId>{}</x:ObjectId>", uuid(1)), ""),
        valid.replace(&uuid(1).to_string(), "00000000-0000-0000-0000-000000000000"),
        valid.replace(
            "</x:ContainedObject>",
            &format!("<x:ObjectId>{}</x:ObjectId></x:ContainedObject>", uuid(1)),
        ),
    ] {
        assert!(inspect(&bad).is_err());
    }
    let wrongclass = valid.replace(
        ibcmd_rs::external::ExternalKind::DataProcessor.class_id(),
        ibcmd_rs::external::ExternalKind::Report.class_id(),
    );
    assert!(ArtifactScope::from_source_identity(inspect(&wrongclass).unwrap().unwrap()).is_err());
}

#[test]
fn ordinary_prefix_and_nondefault_shared_compatibility_keep_configuration_scope_both_dialects() {
    use ibcmd_xml::metadata::inspect_package_intent;
    for dialect in ["2.20", "2.21"] {
        for prefix in ["", "Own_", "Склад_"] {
            let xml = format!(
                r#"<m:MetaDataObject xmlns:m="http://v8.1c.ru/8.3/MDClasses" version="{dialect}"><m:Configuration uuid="{}"><m:Properties><m:NamePrefix>{prefix}</m:NamePrefix><m:CompatibilityMode>Version8_3_24</m:CompatibilityMode><m:ConfigurationExtensionCompatibilityMode>Version8_3_24</m:ConfigurationExtensionCompatibilityMode></m:Properties></m:Configuration></m:MetaDataObject>"#,
                uuid(1)
            );
            let source = xml.as_bytes().to_vec();
            let document = XmlReader::from_slice(&source).unwrap();
            assert_eq!(
                inspect_package_intent(&document).unwrap(),
                Some(PackageIntent::Configuration)
            );
            let identity = inspect_package_identity(&document).unwrap().unwrap();
            assert_eq!(identity.intent, PackageIntent::Configuration);
            assert_eq!(identity.main_uuid, uuid(1));
            assert_eq!(identity.contained, None);
            let selected = ArtifactScope::from_source_identity(identity).unwrap();
            assert_eq!(selected, ArtifactScope::configuration(uuid(1)));
            let model = CanonicalConfiguration::new(vec![
                object(1, "Configuration", None, &[]),
                object(2, "Catalog", None, &[101]),
            ])
            .unwrap();
            let validated = validate_configuration(&model).unwrap();
            let identities = collect_artifact_identities(&validated, selected).unwrap();
            assert_eq!(
                identities,
                collect_bootstrap_identities(&validated).unwrap()
            );
            let routes = vec![
                ObjectStorageRoute::new(uuid(1), vec![]).unwrap(),
                ObjectStorageRoute::new(uuid(2), vec![]).unwrap(),
            ];
            let explicit = build_artifact_graph(
                &identities,
                ProfileId::parse("own:scope").unwrap(),
                routes.clone(),
            )
            .unwrap();
            let existing =
                build_bootstrap_graph(&identities, ProfileId::parse("own:scope").unwrap(), routes)
                    .unwrap();
            assert_eq!(explicit.entries(), existing.entries());
            explicit.require_configuration_scope().unwrap();
            assert_eq!(source, xml.as_bytes());
            // Actual extension purpose, not either shared coordinate, selects it.
            let extension = xml.replace("</m:Properties>", "<m:ConfigurationExtensionPurpose>Customization</m:ConfigurationExtensionPurpose></m:Properties>");
            let identity =
                inspect_package_identity(&XmlReader::from_slice(extension.as_bytes()).unwrap())
                    .unwrap()
                    .unwrap();
            let selected = ArtifactScope::from_source_identity(identity).unwrap();
            assert_eq!(selected.intent(), PackageIntent::Extension);
            let identities = collect_artifact_identities(&validated, selected).unwrap();
            assert!(
                build_bootstrap_graph(&identities, ProfileId::parse("own:scope").unwrap(), vec![])
                    .is_err()
            );
        }
    }
}
