//! Own MIT cleanroom inputs, source ownership only; no native artifact builder.
use ibcmd_core::{
    artifact::ProfileId,
    asset::{AssetReference, MediaKind},
    diagnostic::{ObjectPath, PathSegment},
    identity::ObjectUuid,
    model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts},
    source_policy::SourceOperationPolicy,
    value::{CanonicalField, CanonicalText, CanonicalValue, CanonicalValueKind},
};
use ibcmd_rs::compiler::{external_intake::ExternalIntake, external_owned::ExternalOwnedSource};
use ibcmd_schema::{external_artifact::ExternalArtifactKind, external_named::ExternalNamedKind};
use ibcmd_xml::{
    XmlReader,
    metadata::{ExternalNamedOwnerContext, decode_external_named_owner},
    source_tree::{ReaderLimits, SourceEntry, SourcePath, SourceTree},
};
const MAIN: &str = "11000000-0000-4000-8000-000000000901";
const OBJECT: &str = "22000000-0000-4000-8000-000000000901";
const TYPE: &str = "33000000-0000-4000-8000-000000000901";
const VALUE: &str = "44000000-0000-4000-8000-000000000901";
const FORM: &str = "55000000-0000-4000-8000-000000000901";
const TEMPLATE: &str = "66000000-0000-4000-8000-000000000901";
const ROOT: &str = "different-file.xml";
const FORM_META: &str = "different-file/Forms/Display.xml";
const FORM_BODY: &str = "different-file/Forms/Display/Ext/Form.xml";
const FORM_MODULE: &str = "different-file/Forms/Display/Ext/Form/Module.bsl";
const TEMPLATE_META: &str = "different-file/Templates/Grid.xml";
const TEMPLATE_BODY: &str = "different-file/Templates/Grid/Ext/Template.xml";
const DCS:&[u8]=br#"<DataCompositionSchema xmlns="http://v8.1c.ru/8.1/data-composition-system/schema"><dataSource><name>Own</name></dataSource></DataCompositionSchema>"#;
fn profile(v: &str) -> ProfileId {
    ProfileId::parse(&format!("xml-{v}")).unwrap()
}
fn path() -> ObjectPath {
    ObjectPath::new(vec![PathSegment::name("OwnExternal").unwrap()]).unwrap()
}
fn entry(p: &str, b: &[u8]) -> SourceEntry {
    SourceEntry::from_bytes(SourcePath::new(p).unwrap(), b.to_vec()).unwrap()
}
fn root(kind: ExternalArtifactKind, v: &str) -> Vec<u8> {
    let props = kind
        .properties()
        .iter()
        .copied()
        .filter(|p| kind.property_available(p, v))
        .map(|p| format!("<{p}>{}</{p}>", if p == "Name" { "Own" } else { "" }))
        .collect::<String>();
    format!("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"{v}\"><{} uuid=\"{MAIN}\"><InternalInfo><xr:ContainedObject><xr:ClassId>{}</xr:ClassId><xr:ObjectId>{OBJECT}</xr:ObjectId></xr:ContainedObject><xr:GeneratedType name=\"{}.Own\" category=\"Object\"><xr:TypeId>{TYPE}</xr:TypeId><xr:ValueId>{VALUE}</xr:ValueId></xr:GeneratedType></InternalInfo><Properties>{props}</Properties><ChildObjects><Form>Display</Form><Template>Grid</Template></ChildObjects></{}></MetaDataObject>\r\n",kind.external_kind(),kind.class_id(),kind.object_type_prefix(),kind.external_kind()).into_bytes()
}
fn named(kind: ExternalNamedKind, v: &str) -> Vec<u8> {
    let localized = "<v8:item><v8:lang>en</v8:lang><v8:content>Own label</v8:content></v8:item>";
    let (name, uuid, props) = match kind {
        ExternalNamedKind::Form => (
            "Display",
            FORM,
            format!(
                "<FormType>Managed</FormType><IncludeHelpInContents>false</IncludeHelpInContents><UsePurposes><v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value><v8:Value xsi:type=\"app:ApplicationUsePurpose\">MobilePlatformApplication</v8:Value></UsePurposes>{}<ExtendedPresentation>{localized}</ExtendedPresentation>",
                if v == "2.21" {
                    "<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>"
                } else {
                    ""
                }
            ),
        ),
        ExternalNamedKind::Template => (
            "Grid",
            TEMPLATE,
            "<TemplateType>DataCompositionSchema</TemplateType>".to_owned(),
        ),
    };
    format!("\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<!--own envelope--><MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:app=\"http://v8.1c.ru/8.2/managed-application/core\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{v}\"><{} uuid=\"{uuid}\"><Properties><Name>{name}</Name><Synonym>{localized}</Synonym><Comment><!--keep-->Own comment</Comment>{props}</Properties></{}></MetaDataObject>\r\n",kind.family(),kind.family()).into_bytes()
}
fn fixture(k: ExternalArtifactKind, v: &str) -> SourceTree {
    SourceTree::new(vec![entry(ROOT,&root(k,v)),entry(FORM_META,&named(ExternalNamedKind::Form,v)),entry(FORM_BODY,format!("<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" version=\"{v}\"><AutoCommandBar name=\"Own\"/></Form>").as_bytes()),entry(FORM_MODULE,b"// Own module\r\n"),entry(TEMPLATE_META,&named(ExternalNamedKind::Template,v)),entry(TEMPLATE_BODY,DCS)]).unwrap()
}
fn read(t: SourceTree, v: &str) -> ExternalOwnedSource {
    ExternalOwnedSource::from_tree(t, None, profile(v), path()).unwrap()
}
fn equal(a: &ExternalOwnedSource, b: &ExternalOwnedSource) {
    assert_eq!(a.tree(), b.tree());
    assert_eq!(a.configuration(), b.configuration());
    assert_eq!(a.envelope().root(), b.envelope().root());
    assert_eq!(
        a.envelope().external_source_binding(),
        b.envelope().external_source_binding()
    );
    assert_eq!(a.claims(), b.claims());
    assert_eq!(a.named_owners(), b.named_owners());
    assert_eq!(a.identities(), b.identities());
    assert_eq!(a.graph(), b.graph());
}
fn replace(t: &SourceTree, p: &str, b: Vec<u8>) -> SourceTree {
    SourceTree::new(
        t.entries()
            .iter()
            .map(|e| {
                if e.path().as_str() == p {
                    entry(p, &b)
                } else {
                    e.clone()
                }
            })
            .collect(),
    )
    .unwrap()
}
fn mutate(t: &SourceTree, p: &str, from: &str, to: &str) -> SourceTree {
    let b = t.entries().iter().find(|e| e.path().as_str() == p).unwrap();
    replace(
        t,
        p,
        String::from_utf8(b.bytes().to_vec())
            .unwrap()
            .replace(from, to)
            .into_bytes(),
    )
}
fn parts(o: &CanonicalObject) -> CanonicalObjectParts {
    let mut p = CanonicalObjectParts::new(
        o.identity().clone(),
        o.kind().clone(),
        o.provenance().clone(),
    );
    p.owner = o.owner();
    p.properties = o.properties().to_vec();
    p.references = o.references().to_vec();
    p.generated_types = o.generated_types().to_vec();
    p.assets = o.assets().to_vec();
    p.opaque_facets = o.opaque_facets().clone();
    p
}
fn field(o: &CanonicalObject, n: &str) -> CanonicalValue {
    o.properties()
        .iter()
        .find(|f| f.name().as_str() == n)
        .unwrap()
        .value()
        .clone()
}
#[test]
fn full_source_matrix_keeps_canonical_binding_assets_and_same_bundle() {
    for k in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for v in ["2.20", "2.21"] {
            let t = fixture(k, v);
            assert!(ExternalIntake::from_tree(t.clone(), None, profile(v), path()).is_err());
            let a = read(t, v);
            assert_eq!(a.configuration().len(), 3);
            assert_eq!(a.named_owners().len(), 2);
            assert_eq!(a.claims().len(), 3);
            let f = &a.configuration().objects()[1];
            assert_eq!(f.owner(), Some(ObjectUuid::parse(OBJECT).unwrap()));
            assert_eq!(f.assets().len(), 2);
            assert_eq!(
                field(f, "ExtendedPresentation")
                    .as_sequence()
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                f.properties()
                    .iter()
                    .any(|f| f.name().as_str() == "UseInInterfaceCompatibilityMode"),
                v == "2.21"
            );
            assert!(a.claims().iter().all(|c| c.route().suffix() == ".0"));
            for c in a.claims() {
                let e = a
                    .tree()
                    .entries()
                    .iter()
                    .find(|e| e.path() == c.source_path())
                    .unwrap();
                c.content().verify_bytes(e.bytes()).unwrap();
            }
            let b = a
                .with_current(a.envelope(), a.configuration(), &[])
                .unwrap();
            equal(&a, &b);
        }
    }
}
#[test]
fn direct_and_directory_use_same_exact_retained_owner_census() {
    for k in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for v in ["2.20", "2.21"] {
            let t = fixture(k, v);
            let dir = std::env::temp_dir().join(format!(
                "ibcmd-named-{}-{}-{}",
                std::process::id(),
                v,
                k.internal_kind()
            ));
            std::fs::create_dir(&dir).unwrap();
            for e in t.entries() {
                let p = dir.join(e.path().as_str());
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, e.bytes()).unwrap();
            }
            let a = ExternalOwnedSource::read(
                dir.join(ROOT),
                profile(v),
                path(),
                ReaderLimits::default(),
            )
            .unwrap();
            let b = ExternalOwnedSource::read(&dir, profile(v), path(), ReaderLimits::default())
                .unwrap();
            equal(&a, &b);
            equal(&a, &read(t, v));
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}
#[test]
fn current_property_and_asset_edits_forward_full_ir_and_preserve_callers() {
    for k in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for v in ["2.20", "2.21"] {
            let a = read(fixture(k, v), v);
            let before = a.tree().clone();
            let edit = entry(FORM_MODULE, b"// Changed own module\r\n");
            let mut objects = a.configuration().objects().to_vec();
            let mut p = parts(&objects[1]);
            for f in &mut p.properties {
                if f.name().as_str() == "Comment" {
                    *f = CanonicalField::named(
                        "Comment",
                        CanonicalValue::text(CanonicalText::new("Changed & typed").unwrap()),
                    )
                    .unwrap();
                }
                if f.name().as_str() == "ExtendedPresentation" {
                    let mut fields = f.value().as_sequence().unwrap()[0]
                        .as_record()
                        .unwrap()
                        .to_vec();
                    fields[1] = CanonicalField::named(
                        "content",
                        CanonicalValue::text(CanonicalText::new("Changed presentation").unwrap()),
                    )
                    .unwrap();
                    *f = CanonicalField::named(
                        "ExtendedPresentation",
                        CanonicalValue::sequence(vec![CanonicalValue::record(fields).unwrap()])
                            .unwrap(),
                    )
                    .unwrap();
                }
            }
            p.assets[1] = AssetReference::new(
                edit.digest(),
                edit.bytes().len() as u64,
                MediaKind::new("text/x-1c-bsl").unwrap(),
            )
            .unwrap();
            objects[1] =
                CanonicalObject::new_with_policy(p, SourceOperationPolicy::Source).unwrap();
            let desired =
                CanonicalConfiguration::new_with_policy(objects, SourceOperationPolicy::Source)
                    .unwrap();
            let b = a
                .with_current(a.envelope(), &desired, &[edit.clone()])
                .unwrap();
            assert_eq!(b.configuration(), &desired);
            assert_eq!(a.tree(), &before);
            assert_eq!(
                b.tree()
                    .entries()
                    .iter()
                    .find(|e| e.path() == edit.path())
                    .unwrap(),
                &edit
            );
            let metadata = b
                .tree()
                .entries()
                .iter()
                .find(|e| e.path().as_str() == FORM_META)
                .unwrap();
            assert!(String::from_utf8_lossy(metadata.bytes()).contains("<!--keep-->"));
            let c = read(b.tree().clone(), v);
            equal(&b, &c);
            assert!(a.with_current(a.envelope(), &desired, &[]).is_err());
        }
    }
}
#[test]
fn authored_absence_and_empty_localized_presence_are_distinct() {
    for v in ["2.20", "2.21"] {
        let t = fixture(ExternalArtifactKind::Report, v);
        let present = read(
            mutate(
                &t,
                FORM_META,
                "<ExtendedPresentation><v8:item><v8:lang>en</v8:lang><v8:content>Own label</v8:content></v8:item></ExtendedPresentation>",
                "<ExtendedPresentation/>",
            ),
            v,
        );
        let absent = read(
            mutate(
                &t,
                FORM_META,
                "<ExtendedPresentation><v8:item><v8:lang>en</v8:lang><v8:content>Own label</v8:content></v8:item></ExtendedPresentation>",
                "",
            ),
            v,
        );
        assert_ne!(present.configuration(), absent.configuration());
        equal(
            &present,
            &present
                .with_current(present.envelope(), present.configuration(), &[])
                .unwrap(),
        );
        equal(
            &absent,
            &absent
                .with_current(absent.envelope(), absent.configuration(), &[])
                .unwrap(),
        );
    }
}
#[test]
fn missing_body_or_metadata_and_orphan_wrong_name_refuse_after_positive() {
    for v in ["2.20", "2.21"] {
        let t = fixture(ExternalArtifactKind::DataProcessor, v);
        let a = read(t.clone(), v);
        equal(
            &a,
            &a.with_current(a.envelope(), a.configuration(), &[])
                .unwrap(),
        );
        for missing in [FORM_META, FORM_BODY, TEMPLATE_META, TEMPLATE_BODY] {
            let bad = SourceTree::new(
                t.entries()
                    .iter()
                    .filter(|e| e.path().as_str() != missing)
                    .cloned()
                    .collect(),
            )
            .unwrap();
            assert!(
                ExternalOwnedSource::from_tree(bad, None, profile(v), path()).is_err(),
                "{missing}"
            );
        }
        let mut orphan = t.entries().to_vec();
        orphan.push(entry("different-file/Ext/Unknown.bin", b"own"));
        assert!(
            ExternalOwnedSource::from_tree(
                SourceTree::new(orphan).unwrap(),
                None,
                profile(v),
                path()
            )
            .is_err()
        );
        assert!(
            ExternalOwnedSource::from_tree(
                mutate(&t, FORM_META, "<Name>Display</Name>", "<Name>Other</Name>"),
                None,
                profile(v),
                path()
            )
            .is_err()
        );
        let optional = SourceTree::new(
            t.entries()
                .iter()
                .filter(|e| e.path().as_str() != FORM_MODULE)
                .cloned()
                .collect(),
        )
        .unwrap();
        assert_eq!(read(optional, v).claims().len(), 2);
    }
}
#[test]
fn identity_nil_root_generated_and_duplicate_collisions_refuse() {
    for v in ["2.20", "2.21"] {
        let t = fixture(ExternalArtifactKind::Report, v);
        read(t.clone(), v);
        for id in [
            "00000000-0000-0000-0000-000000000000",
            MAIN,
            OBJECT,
            TYPE,
            VALUE,
            TEMPLATE,
        ] {
            let bytes = t
                .entries()
                .iter()
                .find(|e| e.path().as_str() == FORM_META)
                .unwrap();
            let mutated = String::from_utf8(bytes.bytes().to_vec())
                .unwrap()
                .replace(FORM, id)
                .into_bytes();
            let entries = t
                .entries()
                .iter()
                .map(|e| {
                    if e.path().as_str() == FORM_META {
                        entry(FORM_META, &mutated)
                    } else {
                        e.clone()
                    }
                })
                .collect();
            // Duplicate physical metadata UUIDs are rejected by the existing
            // SourceTree boundary; all other collisions reach actual N1/A0.
            match SourceTree::new(entries) {
                Ok(tree) => assert!(
                    ExternalOwnedSource::from_tree(tree, None, profile(v), path()).is_err(),
                    "{id}"
                ),
                Err(error) => assert!(
                    matches!(
                        error,
                        ibcmd_xml::source_tree::SourceTreeError::UuidConflict { .. }
                    ),
                    "{error}"
                ),
            }
        }
        assert!(
            ExternalOwnedSource::from_tree(
                mutate(
                    &t,
                    ROOT,
                    "<Form>Display</Form>",
                    "<Form>Display</Form><Form>Display</Form>"
                ),
                None,
                profile(v),
                path()
            )
            .is_err()
        );
    }
}
#[test]
fn closed_properties_namespace_qname_and_boolean_negatives_both_editions() {
    for v in ["2.20", "2.21"] {
        let t = fixture(ExternalArtifactKind::Report, v);
        read(t.clone(), v);
        for (from, to) in [
            (
                "<FormType>Managed</FormType>",
                "<FormType>Future</FormType>",
            ),
            (
                "<IncludeHelpInContents>false</IncludeHelpInContents>",
                "<IncludeHelpInContents>1</IncludeHelpInContents>",
            ),
            ("<Comment>", "<Comment unsupported=\"1\">"),
            (
                "<Comment><!--keep-->Own comment</Comment>",
                "<Comment/><Comment/>",
            ),
            ("<Comment><!--keep-->Own comment</Comment>", "<Unknown/>"),
            ("http://v8.1c.ru/8.3/MDClasses", "urn:foreign"),
            ("app:ApplicationUsePurpose", "v8:ApplicationUsePurpose"),
            ("MobilePlatformApplication", "PlatformApplication"),
            ("MobilePlatformApplication", "FuturePurpose"),
        ] {
            assert!(
                ExternalOwnedSource::from_tree(
                    mutate(&t, FORM_META, from, to),
                    None,
                    profile(v),
                    path()
                )
                .is_err(),
                "{from}=>{to}"
            );
        }
        assert!(
            ExternalOwnedSource::from_tree(
                mutate(
                    &t,
                    FORM_BODY,
                    "http://v8.1c.ru/8.3/xcf/logform",
                    "urn:foreign"
                ),
                None,
                profile(v),
                path()
            )
            .is_err()
        );
        assert!(
            ExternalOwnedSource::from_tree(
                mutate(
                    &t,
                    TEMPLATE_BODY,
                    "http://v8.1c.ru/8.1/data-composition-system/schema",
                    "urn:foreign"
                ),
                None,
                profile(v),
                path()
            )
            .is_err()
        );
    }
}
#[test]
fn expanded_aliases_and_scoped_namespace_bindings_keep_full_inverse() {
    for v in ["2.20", "2.21"] {
        let t = fixture(ExternalArtifactKind::DataProcessor, v);
        let aliased = mutate(
            &mutate(&t, FORM_META, "v8:", "core:"),
            FORM_META,
            "xmlns:v8=",
            "xmlns:core=",
        );
        let a = read(aliased.clone(), v);
        equal(
            &a,
            &a.with_current(a.envelope(), a.configuration(), &[])
                .unwrap(),
        );
        let scoped = mutate(
            &aliased,
            FORM_META,
            "<ExtendedPresentation>",
            "<ExtendedPresentation xmlns:core=\"http://v8.1c.ru/8.1/data/core\">",
        );
        read(scoped, v);
        let foreign = mutate(
            &aliased,
            FORM_META,
            "<ExtendedPresentation>",
            "<ExtendedPresentation xmlns:core=\"urn:foreign\">",
        );
        assert!(ExternalOwnedSource::from_tree(foreign, None, profile(v), path()).is_err());
    }
}
#[test]
fn current_context_identity_presence_order_and_unknown_asset_refuse_atomically() {
    for v in ["2.20", "2.21"] {
        let a = read(fixture(ExternalArtifactKind::Report, v), v);
        let before = a.tree().clone();
        for mode in 0..4 {
            let mut objects = a.configuration().objects().to_vec();
            let mut p = parts(&objects[1]);
            match mode {
                0 => p.owner = Some(ObjectUuid::parse(MAIN).unwrap()),
                1 => {
                    p.properties.remove(2);
                }
                2 => p.properties.swap(1, 2),
                _ => p.assets.clear(),
            };
            if let Ok(o) = CanonicalObject::new_with_policy(p, SourceOperationPolicy::Source) {
                objects[1] = o;
                let c =
                    CanonicalConfiguration::new_with_policy(objects, SourceOperationPolicy::Source)
                        .unwrap();
                assert!(a.with_current(a.envelope(), &c, &[]).is_err());
            }
            assert_eq!(a.tree(), &before);
        }
        assert!(
            a.with_current(
                a.envelope(),
                a.configuration(),
                &[entry("other.bin", b"unknown")]
            )
            .is_err()
        );
        let c = ExternalNamedOwnerContext::from_root(a.envelope()).unwrap();
        let d = XmlReader::from_slice(
            a.tree()
                .entries()
                .iter()
                .find(|e| e.path().as_str() == FORM_META)
                .unwrap()
                .bytes(),
        )
        .unwrap();
        assert!(
            decode_external_named_owner(
                &d,
                profile(v),
                path(),
                &c,
                ExternalNamedKind::Form,
                "Display"
            )
            .is_err()
        );
        assert!(
            decode_external_named_owner(
                &d,
                profile(v),
                a.configuration().objects()[1].identity().path().clone(),
                &c,
                ExternalNamedKind::Form,
                "Undeclared"
            )
            .is_err()
        );
    }
}
#[test]
fn template_source_type_reuses_existing_file_dispatch_without_html_wildcard() {
    for v in ["2.20", "2.21"] {
        let t = fixture(ExternalArtifactKind::Report, v);
        read(t.clone(), v);
        for (kind, file, body) in [
            ("TextDocument", "Template.txt", b"Own text".as_slice()),
            ("BinaryData", "Template.bin", b"\x00\xffown".as_slice()),
            ("AddIn", "Template.bin", b"own binary".as_slice()),
        ] {
            let typed = mutate(&t, TEMPLATE_META, "DataCompositionSchema", kind);
            let entries = typed
                .entries()
                .iter()
                .map(|e| {
                    if e.path().as_str() == TEMPLATE_BODY {
                        entry(&format!("different-file/Templates/Grid/Ext/{file}"), body)
                    } else {
                        e.clone()
                    }
                })
                .collect();
            let a = read(SourceTree::new(entries).unwrap(), v);
            equal(
                &a,
                &a.with_current(a.envelope(), a.configuration(), &[])
                    .unwrap(),
            );
        }
        for kind in [
            "Future",
            "HTMLDocument",
            "ActiveDocument",
            "GeographicalSchema",
        ] {
            assert!(
                ExternalOwnedSource::from_tree(
                    mutate(&t, TEMPLATE_META, "DataCompositionSchema", kind),
                    None,
                    profile(v),
                    path()
                )
                .is_err(),
                "{kind}"
            );
        }
    }
}
#[test]
fn interface_property_editions_and_unknown_values_have_no_native_layout_inference() {
    let t = fixture(ExternalArtifactKind::DataProcessor, "2.20");
    read(t.clone(), "2.20");
    assert!(ExternalOwnedSource::from_tree(mutate(&t,FORM_META,"<ExtendedPresentation>","<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode><ExtendedPresentation>"),None,profile("2.20"),path()).is_err());
    let t = fixture(ExternalArtifactKind::DataProcessor, "2.21");
    read(t.clone(), "2.21");
    assert!(
        ExternalOwnedSource::from_tree(
            mutate(&t, FORM_META, ">Any<", ">Future<"),
            None,
            profile("2.21"),
            path()
        )
        .is_err()
    );
    let absent = mutate(
        &t,
        FORM_META,
        "<UseInInterfaceCompatibilityMode>Any</UseInInterfaceCompatibilityMode>",
        "",
    );
    let a = read(absent, "2.21");
    equal(
        &a,
        &a.with_current(a.envelope(), a.configuration(), &[])
            .unwrap(),
    );
}

#[test]
fn current_root_rename_with_explicit_derived_refs_and_template_edits_is_complete() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for v in ["2.20", "2.21"] {
            let a = read(fixture(kind, v), v);
            let before = a.tree().clone();
            let old_root = before
                .entries()
                .iter()
                .find(|e| e.path().as_str() == ROOT)
                .unwrap();
            let changed = String::from_utf8(old_root.bytes().to_vec())
                .unwrap()
                .replace("<Name>Own</Name>", "<Name>Renamed</Name>")
                .replace(
                    &format!("{}.Own", kind.object_type_prefix()),
                    &format!("{}.Renamed", kind.object_type_prefix()),
                );
            let doc = XmlReader::from_slice(changed.as_bytes()).unwrap();
            let decoded =
                ibcmd_xml::metadata::decode_external_root(&doc, profile(v), path()).unwrap();
            let root = a
                .envelope()
                .clone()
                .with_model(decoded.root().clone(), vec![])
                .unwrap();
            let edit=entry(TEMPLATE_BODY,br#"<GraphicalSchema xmlns="http://v8.1c.ru/8.3/xcf/scheme"><Items/></GraphicalSchema>"#);
            let mut objects = a.configuration().objects().to_vec();
            objects[0] = root.root().clone();
            let mut p = parts(&objects[2]);
            for f in &mut p.properties {
                match f.name().as_str() {
                    "Comment" => {
                        *f = CanonicalField::named(
                            "Comment",
                            CanonicalValue::text(
                                CanonicalText::new("Edited own template").unwrap(),
                            ),
                        )
                        .unwrap()
                    }
                    "TemplateType" => {
                        *f = CanonicalField::named(
                            "TemplateType",
                            CanonicalValue::enum_token(
                                ibcmd_core::value::EnumToken::new("GraphicalSchema").unwrap(),
                            ),
                        )
                        .unwrap()
                    }
                    _ => {}
                }
            }
            p.assets[0] = AssetReference::new(
                edit.digest(),
                edit.bytes().len() as u64,
                MediaKind::new("application/xml").unwrap(),
            )
            .unwrap();
            objects[2] =
                CanonicalObject::new_with_policy(p, SourceOperationPolicy::Source).unwrap();
            let current =
                CanonicalConfiguration::new_with_policy(objects, SourceOperationPolicy::Source)
                    .unwrap();
            let b = a.with_current(&root, &current, &[edit]).unwrap();
            assert_eq!(b.configuration(), &current);
            assert_eq!(
                b.envelope().external_source_binding(),
                a.envelope().external_source_binding()
            );
            assert_eq!(a.tree(), &before);
            equal(&b, &read(b.tree().clone(), v));
            let mut wrong = parts(root.root());
            wrong
                .properties
                .iter_mut()
                .find(|f| f.name().as_str() == "ChildForms")
                .unwrap()
                .clone_from(
                    a.envelope()
                        .root()
                        .properties()
                        .iter()
                        .find(|f| f.name().as_str() == "ChildForms")
                        .unwrap(),
                );
            let wrong = a
                .envelope()
                .clone()
                .with_model(CanonicalObject::new(wrong).unwrap(), vec![])
                .unwrap();
            let mut mismatched = current.objects().to_vec();
            mismatched[0] = wrong.root().clone();
            let mismatched =
                CanonicalConfiguration::new_with_policy(mismatched, SourceOperationPolicy::Source)
                    .unwrap();
            assert!(a.with_current(&wrong, &mismatched, &[]).is_err());
            assert_eq!(a.tree(), &before);
        }
    }
}
#[test]
fn selected_alias_contract_stays_physical_and_memory_paths_remain_exact() {
    for kind in [
        ExternalArtifactKind::DataProcessor,
        ExternalArtifactKind::Report,
    ] {
        for v in ["2.20", "2.21"] {
            let t = fixture(kind, v);
            read(t.clone(), v);
            assert!(
                ExternalOwnedSource::from_tree(
                    t.clone(),
                    Some(&SourcePath::new("DIFFERENT-FILE.XML").unwrap()),
                    profile(v),
                    path()
                )
                .is_err()
            );
            let dir = std::env::temp_dir().join(format!(
                "ibcmd-owned-alias-{}-{v}-{}",
                std::process::id(),
                kind.internal_kind()
            ));
            std::fs::create_dir(&dir).unwrap();
            for e in t.entries() {
                let p = dir.join(e.path().as_str());
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(p, e.bytes()).unwrap();
            }
            let good = ExternalOwnedSource::read(&dir, profile(v), path(), ReaderLimits::default())
                .unwrap();
            #[cfg(windows)]
            {
                let alias = ExternalOwnedSource::read(
                    dir.join("DIFFERENT-FILE.XML"),
                    profile(v),
                    path(),
                    ReaderLimits::default(),
                )
                .unwrap();
                equal(&good, &alias);
            }
            std::fs::write(dir.join("not-root.xml"), named(ExternalNamedKind::Form, v)).unwrap();
            assert!(
                ExternalOwnedSource::read(
                    dir.join("not-root.xml"),
                    profile(v),
                    path(),
                    ReaderLimits::default()
                )
                .is_err()
            );
            assert_eq!(good.tree(), &t);
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}
#[cfg(unix)]
#[test]
fn physical_link_and_nonregular_named_sources_never_borrow_foreign_bytes() {
    use std::os::unix::{fs::symlink, net::UnixListener};
    let v = "2.20";
    let t = fixture(ExternalArtifactKind::Report, v);
    let base = std::env::temp_dir().join(format!("ibcmd-owned-link-{}", std::process::id()));
    std::fs::create_dir(&base).unwrap();
    let own = base.join("own");
    std::fs::create_dir(&own).unwrap();
    for e in t.entries() {
        let p = own.join(e.path().as_str());
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, e.bytes()).unwrap();
    }
    let a = ExternalOwnedSource::read(&own, profile(v), path(), ReaderLimits::default()).unwrap();
    equal(&a, &read(t, v));
    std::fs::write(base.join("outside.xml"), DCS).unwrap();
    std::fs::remove_file(own.join(TEMPLATE_BODY)).unwrap();
    symlink(base.join("outside.xml"), own.join(TEMPLATE_BODY)).unwrap();
    assert!(ExternalOwnedSource::read(&own, profile(v), path(), ReaderLimits::default()).is_err());
    std::fs::remove_file(own.join(TEMPLATE_BODY)).unwrap();
    // sockaddr_un limits the bind address, independently of source-tree paths.
    // Create the genuine socket nearby, then move its node into the same tree.
    let socket = base.join("s");
    let listener = UnixListener::bind(&socket).unwrap();
    std::fs::rename(&socket, own.join(TEMPLATE_BODY)).unwrap();
    assert!(ExternalOwnedSource::read(&own, profile(v), path(), ReaderLimits::default()).is_err());
    drop(listener);
    std::fs::remove_dir_all(base).unwrap();
}
