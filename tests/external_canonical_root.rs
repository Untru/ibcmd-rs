//! Independently authored MIT roots; no platform/native/foreign fixture copied.
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::{ObjectPath, PathSegment};
use ibcmd_core::family::FamilyId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::model::{CanonicalObject, CanonicalObjectParts, MetadataKind};
use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue, UnresolvedReference};
use ibcmd_schema::external_artifact::ExternalArtifactKind;
use ibcmd_xml::metadata::{
    MetadataEnvelope, MetadataRegistry, decode_external_root, encode_external_root,
    register_data_processor_codec, register_external_data_processor_codec,
    register_external_report_codec, register_report_codec,
};
use ibcmd_xml::{LexicalPolicy, XmlReader, XmlWriter};

const MAIN: &str = "10000000-0000-4000-8000-000000000438";
const OBJECT: &str = "20000000-0000-4000-8000-000000000438";
const TYPE: &str = "30000000-0000-4000-8000-000000000438";
const VALUE: &str = "40000000-0000-4000-8000-000000000438";
fn profile(version: &str) -> ProfileId {
    ProfileId::parse(&format!("xml-{version}")).unwrap()
}
fn path() -> ObjectPath {
    ObjectPath::new(vec![PathSegment::name("OwnedExternal").unwrap()]).unwrap()
}
fn text(value: &str) -> CanonicalValue {
    CanonicalValue::text(CanonicalText::new(value).unwrap())
}
fn fixture(kind: ExternalArtifactKind, version: &str, children: bool) -> Vec<u8> {
    let fields = kind.properties().iter().copied().filter(|x| kind.property_available(x, version)).map(|field| {
        let value = match field {
            "Name" => "Owned".to_owned(),
            "Synonym" => "<v8:item><!--retained translation trivia--><v8:lang>ru</v8:lang><v8:content>Own label</v8:content></v8:item>".to_owned(),
            "Comment" => "Own comment &amp; lexical".to_owned(),
            "DefaultForm" if children => format!("{}.Owned.Form.OwnForm", kind.external_kind()),
            "MainDataCompositionSchema" if children => format!("{}.Owned.Template.OwnDcs", kind.external_kind()),
            _ => String::new(),
        };
        format!("\t\t\t<{field}>{value}</{field}>\r\n")
    }).collect::<String>();
    let children = if children {
        "<Form>OwnForm</Form><Template>OwnDcs</Template>"
    } else {
        ""
    };
    format!("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!--own document trivia-->\r\n<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{version}\">\r\n\t<{} uuid=\"{MAIN}\">\r\n\t\t<InternalInfo><xr:ContainedObject><xr:ClassId>{}</xr:ClassId><xr:ObjectId>{OBJECT}</xr:ObjectId></xr:ContainedObject><xr:GeneratedType name=\"{}.Owned\" category=\"Object\"><xr:TypeId>{TYPE}</xr:TypeId><xr:ValueId>{VALUE}</xr:ValueId></xr:GeneratedType></InternalInfo>\r\n\t\t<Properties>\r\n{fields}\t\t</Properties>\r\n\t\t<ChildObjects>{children}</ChildObjects>\r\n\t</{}>\r\n</MetaDataObject>\r\n",kind.external_kind(),kind.class_id(),kind.object_type_prefix(),kind.external_kind()).into_bytes()
}
fn parts(root: &CanonicalObject) -> CanonicalObjectParts {
    let mut parts = CanonicalObjectParts::new(
        root.identity().clone(),
        root.kind().clone(),
        root.provenance().clone(),
    );
    parts.owner = root.owner();
    parts.properties = root.properties().to_vec();
    parts.references = root.references().to_vec();
    parts.generated_types = root.generated_types().to_vec();
    parts.assets = root.assets().to_vec();
    parts.opaque_facets = root.opaque_facets().clone();
    parts
}
fn edited(envelope: &MetadataEnvelope, updates: &[(&str, CanonicalValue)]) -> MetadataEnvelope {
    let mut parts = parts(envelope.root());
    for (name, value) in updates {
        let field = parts
            .properties
            .iter_mut()
            .find(|x| x.name().as_str() == *name)
            .unwrap();
        *field = CanonicalField::named(name, value.clone()).unwrap();
    }
    envelope
        .clone()
        .with_model(
            CanonicalObject::new(parts).unwrap(),
            envelope.descendants().to_vec(),
        )
        .unwrap()
}
fn returned(bytes: &[u8], version: &str) -> MetadataEnvelope {
    decode_external_root(
        &XmlReader::from_slice(bytes).unwrap(),
        profile(version),
        path(),
    )
    .unwrap()
}
fn assert_complete(actual: &MetadataEnvelope, expected: &MetadataEnvelope) {
    assert_eq!(actual.root(), expected.root());
    assert_eq!(actual.descendants(), expected.descendants());
    assert_eq!(
        actual.configuration().unwrap(),
        expected.configuration().unwrap()
    );
    assert_eq!(
        actual.external_source_binding(),
        expected.external_source_binding()
    );
}

#[test]
fn external_root_both_families_and_editions_preserve_full_xml_graph_and_scope() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            for children in [false, true] {
                let bytes = fixture(kind, version, children);
                let envelope = returned(&bytes, version);
                assert_eq!(envelope.root().kind().as_str(), kind.internal_kind());
                assert_eq!(
                    envelope.root().identity().uuid(),
                    ObjectUuid::parse(OBJECT).unwrap()
                );
                let binding = envelope.external_source_binding().unwrap();
                assert_eq!(
                    binding.identity().main_uuid,
                    ObjectUuid::parse(MAIN).unwrap()
                );
                assert_ne!(
                    binding.identity().main_uuid,
                    envelope.root().identity().uuid()
                );
                assert_eq!(envelope.root().generated_types().len(), 1);
                assert_eq!(
                    envelope.root().generated_types()[0].kind().as_str(),
                    "Object"
                );
                assert_eq!(
                    envelope.root().generated_types()[0].uuid(),
                    ObjectUuid::parse(TYPE).unwrap()
                );
                assert_eq!(
                    envelope.root().generated_types()[0].value_id(),
                    Some(ObjectUuid::parse(VALUE).unwrap())
                );
                let scope = ibcmd_rs::compiler::artifact::ArtifactScope::from_source_identity(
                    binding.identity(),
                )
                .unwrap();
                assert_eq!(scope.root_uuid(), envelope.root().identity().uuid());
                assert_eq!(scope.main_uuid(), binding.identity().main_uuid);
                let encoded = encode_external_root(&envelope, &profile(version)).unwrap();
                assert_eq!(encoded, bytes);
                assert_complete(&returned(&encoded, version), &envelope);
                assert_eq!(
                    encode_external_root(&returned(&encoded, version), &profile(version)).unwrap(),
                    encoded
                );
                // These are independently typed slots, not a universal inequality rule.
                // This XML-only control makes no fresh native header claim.
                let coincident = String::from_utf8(bytes.clone())
                    .unwrap()
                    .replace(MAIN, OBJECT)
                    .into_bytes();
                let coincident_envelope = returned(&coincident, version);
                let coincident_binding = coincident_envelope.external_source_binding().unwrap();
                assert_eq!(
                    coincident_binding.identity().main_uuid,
                    coincident_envelope.root().identity().uuid()
                );
                let coincident_scope =
                    ibcmd_rs::compiler::artifact::ArtifactScope::from_source_identity(
                        coincident_binding.identity(),
                    )
                    .unwrap();
                assert_eq!(coincident_scope.main_uuid(), coincident_scope.root_uuid());
                let coincident_encoded =
                    encode_external_root(&coincident_envelope, &profile(version)).unwrap();
                assert_eq!(coincident_encoded, coincident);
                assert_complete(
                    &returned(&coincident_encoded, version),
                    &coincident_envelope,
                );
                assert_eq!(
                    encode_external_root(
                        &returned(&coincident_encoded, version),
                        &profile(version)
                    )
                    .unwrap(),
                    coincident_encoded
                );
            }
        }
    }
}

#[test]
fn external_current_name_comment_synonym_and_owned_refs_edit_complete_inverse() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let bytes = fixture(kind, version, true);
            let original = returned(&bytes, version);
            let synonym = CanonicalValue::sequence(vec![
                CanonicalValue::record(vec![
                    CanonicalField::named("lang", text("en")).unwrap(),
                    CanonicalField::named("content", text("CURRENT & label")).unwrap(),
                ])
                .unwrap(),
            ])
            .unwrap();
            let reference = |suffix| format!("{}.Current.{suffix}", kind.external_kind());
            let forms = CanonicalValue::sequence(vec![CanonicalValue::reference(
                UnresolvedReference::new("metadata", &reference("Form.OwnForm")).unwrap(),
            )])
            .unwrap();
            let templates = CanonicalValue::sequence(vec![CanonicalValue::reference(
                UnresolvedReference::new("metadata", &reference("Template.OwnDcs")).unwrap(),
            )])
            .unwrap();
            let mut updates = vec![
                ("Name", text("Current")),
                (
                    "ObjectTypeName",
                    text(&format!("{}.Current", kind.object_type_prefix())),
                ),
                ("Comment", text("CURRENT < & comment")),
                ("Synonym", synonym),
                ("DefaultForm", text(&reference("Form.OwnForm"))),
                ("ChildForms", forms),
                ("ChildTemplates", templates),
            ];
            if kind == ExternalArtifactKind::Report {
                updates.push((
                    "MainDataCompositionSchema",
                    text(&reference("Template.OwnDcs")),
                ));
            }
            let current = edited(&original, &updates);
            let actual = encode_external_root(&current, &profile(version)).unwrap();
            let xml = String::from_utf8(actual.clone()).unwrap();
            assert!(xml.contains("<Name>Current</Name>"));
            assert!(xml.contains("CURRENT &lt; &amp; comment"));
            assert!(xml.contains("CURRENT &amp; label"));
            assert!(xml.contains("<!--retained translation trivia-->"));
            assert!(xml.contains(&format!("{}.Current", kind.object_type_prefix())));
            assert_complete(&returned(&actual, version), &current);
            assert_eq!(
                encode_external_root(&returned(&actual, version), &profile(version)).unwrap(),
                actual
            );
            assert_eq!(
                encode_external_root(&original, &profile(version)).unwrap(),
                bytes,
                "caller/source immutable"
            );
        }
    }
}

#[test]
fn external_explicit_registry_routes_by_bound_source_family_without_internal_alias() {
    let mut registry = MetadataRegistry::default();
    register_data_processor_codec(&mut registry).unwrap();
    register_report_codec(&mut registry).unwrap();
    register_external_data_processor_codec(&mut registry).unwrap();
    register_external_report_codec(&mut registry).unwrap();
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let bytes = fixture(kind, version, false);
            let doc = XmlReader::from_slice(&bytes).unwrap();
            let family = FamilyId::parse(kind.external_kind()).unwrap();
            let envelope = registry
                .decode(&family, &doc, profile(version), path())
                .unwrap();
            assert_eq!(
                registry.encode(&envelope, &profile(version)).unwrap(),
                bytes
            );
            assert!(
                registry
                    .decode(
                        &FamilyId::parse(kind.internal_kind()).unwrap(),
                        &doc,
                        profile(version),
                        path()
                    )
                    .is_err()
            );
            assert!(
                MetadataEnvelope::from_parts(envelope.root().clone(), vec![], doc.clone()).is_err(),
                "public same-family constructor cannot forge bound source identity"
            );
            assert_eq!(envelope.external_source_binding().unwrap().kind(), kind);
        }
    }
}

#[test]
fn external_unknown_duplicate_namespace_identity_and_undeclared_references_refuse() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let bytes = fixture(kind, version, true);
            let source = String::from_utf8(bytes.clone()).unwrap();
            let unknown_class = "90000000-0000-4000-8000-000000000438";
            let invalid=[
                source.replace(kind.class_id(),unknown_class),
                source.replace(TYPE,"00000000-0000-0000-0000-000000000000"),
                source.replace(VALUE,TYPE),
                source.replace("category=\"Object\"","category=\"Manager\""),
                source.replace(&format!("{}.Owned",kind.object_type_prefix()),"DataProcessorObject.Owned"),
                source.replace("<Name>Owned</Name>","<Name>Owned</Name><Name>Decoy</Name>"),
                source.replace("</Properties>","<Unknown>authored</Unknown></Properties>"),
                source.replace("<Name>Owned</Name>","<Name xmlns=\"urn:foreign\">Owned</Name>"),
                source.replace("<Name>Owned</Name>","<Name unexpected=\"x\">Owned</Name>"),
                source.replace("<Form>OwnForm</Form>","<Form>OwnForm</Form><Form>OwnForm</Form>"),
                source.replace("<Form>OwnForm</Form>","<Form>Other</Form>"),
                source.replace("</ChildObjects>","<Attribute uuid=\"90000000-0000-4000-8000-000000000439\"><Properties><Name>Undecoded</Name></Properties></Attribute></ChildObjects>"),
                source.replace("<xr:TypeId>","<xr:Decoy>1</xr:Decoy><xr:TypeId>"),
                source.replace("</InternalInfo>","<xr:Unknown/></InternalInfo>"),
                source.replace("</MetaDataObject>","<Extra/></MetaDataObject>"),
            ];
            for (case, text) in invalid.iter().enumerate() {
                assert!(
                    decode_external_root(
                        &XmlReader::from_slice(text.as_bytes()).unwrap(),
                        profile(version),
                        path()
                    )
                    .is_err(),
                    "{kind:?}/{version} case{case}"
                );
            }
            let envelope = returned(&bytes, version);
            let wrong_version = if version == "2.20" { "2.21" } else { "2.20" };
            assert!(
                decode_external_root(
                    &XmlReader::from_slice(&bytes).unwrap(),
                    profile(wrong_version),
                    path()
                )
                .is_err()
            );
            assert!(
                encode_external_root(&envelope, &profile(wrong_version)).is_err(),
                "cross-edition facet migration remains explicit pending contract"
            );
            assert_eq!(
                encode_external_root(&envelope, &profile(version)).unwrap(),
                bytes
            );
        }
    }
}

#[test]
fn external_stale_current_identity_name_type_and_property_presence_edits_are_atomic_refusals() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let bytes = fixture(kind, version, true);
            let envelope = returned(&bytes, version);
            let stale = edited(&envelope, &[("Name", text("CurrentWithoutTypeAndRefs"))]);
            assert!(encode_external_root(&stale, &profile(version)).is_err());
            let mut p = parts(envelope.root());
            p.kind = MetadataKind::new("Configuration").unwrap();
            assert!(
                envelope
                    .clone()
                    .with_model(CanonicalObject::new(p).unwrap(), vec![])
                    .is_err()
            );
            let mut p = parts(envelope.root());
            p.identity = ibcmd_core::identity::LogicalIdentity::new(
                ObjectUuid::parse(MAIN).unwrap(),
                path(),
            );
            assert!(
                envelope
                    .clone()
                    .with_model(CanonicalObject::new(p).unwrap(), vec![])
                    .is_err()
            );
            let mut p = parts(envelope.root());
            p.generated_types[0] = ibcmd_core::model::GeneratedType::new(
                ObjectUuid::parse("80000000-0000-4000-8000-000000000438").unwrap(),
                p.generated_types[0].kind().clone(),
            )
            .with_value_id(ObjectUuid::parse(VALUE).unwrap());
            let changed_type = envelope
                .clone()
                .with_model(CanonicalObject::new(p).unwrap(), vec![])
                .unwrap();
            assert!(
                encode_external_root(&changed_type, &profile(version)).is_err(),
                "current generated identity cannot evade bound emitter"
            );
            let mut p = parts(envelope.root());
            p.properties.retain(|x| x.name().as_str() != "Comment");
            let removal = envelope
                .clone()
                .with_model(CanonicalObject::new(p).unwrap(), vec![])
                .unwrap();
            assert!(
                encode_external_root(&removal, &profile(version)).is_err(),
                "unimplemented source-slot removal is never destructive success"
            );
            assert_eq!(
                encode_external_root(&envelope, &profile(version)).unwrap(),
                bytes
            );
            assert_eq!(
                XmlWriter::to_vec(envelope.source_document(), LexicalPolicy::Preserve).unwrap(),
                bytes
            );
        }
    }
}

#[test]
fn external_report_auxiliary_variant_is_an_explicit_edition_owned_value() {
    let bytes = fixture(ExternalArtifactKind::Report, "2.21", true);
    let envelope = returned(&bytes, "2.21");
    let current = edited(
        &envelope,
        &[(
            "AuxiliaryVariantForm",
            text("ExternalReport.Owned.Form.OwnForm"),
        )],
    );
    let actual = encode_external_root(&current, &profile("2.21")).unwrap();
    assert!(String::from_utf8(actual.clone()).unwrap().contains(
        "<AuxiliaryVariantForm>ExternalReport.Owned.Form.OwnForm</AuxiliaryVariantForm>"
    ));
    assert_complete(&returned(&actual, "2.21"), &current);
    let invalid = String::from_utf8(bytes)
        .unwrap()
        .replace("version=\"2.21\"", "version=\"2.20\"");
    assert!(
        decode_external_root(
            &XmlReader::from_slice(invalid.as_bytes()).unwrap(),
            profile("2.20"),
            path()
        )
        .is_err()
    );
    // This is XML/root semantics only: it proves no fresh native slot encoding.
}

#[test]
fn external_expanded_namespace_aliases_preserve_root_and_local_shadow_refuses() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for version in ["2.20", "2.21"] {
            let plain = fixture(kind, version, true);
            let mut source = String::from_utf8(plain)
                .unwrap()
                .replace(
                    "xmlns=\"http://v8.1c.ru/8.3/MDClasses\"",
                    "xmlns:m=\"http://v8.1c.ru/8.3/MDClasses\"",
                )
                .replace("<MetaDataObject ", "<m:MetaDataObject ")
                .replace("</MetaDataObject>", "</m:MetaDataObject>")
                .replace(
                    &format!("<{} uuid=", kind.external_kind()),
                    &format!("<m:{} uuid=", kind.external_kind()),
                )
                .replace(
                    &format!("</{}>", kind.external_kind()),
                    &format!("</m:{}>", kind.external_kind()),
                );
            for name in [
                "InternalInfo",
                "Properties",
                "ChildObjects",
                "Form",
                "Template",
            ]
            .into_iter()
            .chain(kind.properties().iter().copied())
            {
                source = source
                    .replace(&format!("<{name}>"), &format!("<m:{name}>"))
                    .replace(&format!("</{name}>"), &format!("</m:{name}>"));
            }
            let envelope = returned(source.as_bytes(), version);
            assert_eq!(
                envelope.root().identity().uuid(),
                ObjectUuid::parse(OBJECT).unwrap()
            );
            assert_eq!(envelope.external_source_binding().unwrap().kind(), kind);
            assert_eq!(
                encode_external_root(&envelope, &profile(version)).unwrap(),
                source.as_bytes()
            );
            assert_complete(&returned(source.as_bytes(), version), &envelope);
            let shadow = source.replace("<m:Comment>", "<m:Comment xmlns:m=\"urn:foreign\">");
            assert!(
                decode_external_root(
                    &XmlReader::from_slice(shadow.as_bytes()).unwrap(),
                    profile(version),
                    path()
                )
                .is_err()
            );
        }
    }
}
