use super::super::tests::{change_members, check, pack, snapshot};
use super::*;
use crate::compiler::{
    CompileAxes,
    bodies::form::compile_managed_form,
    families::{
        document::compile_document_metadata,
        form::{FormEvidencedCohort, FormMetadataNativeIr, encode_form_metadata_blob},
    },
    graph::{ObjectStorageRoute, build_bootstrap_graph},
    identity::collect_bootstrap_identities,
    root::{ConfigurationBodyProperties, compile_configuration_body, compile_root},
    version::{SpecialEntryProfile, compile_version},
};
use ibcmd_core::{
    artifact::{ProfileId, StorageProfileId},
    diagnostic::{ObjectPath, PathSegment, PropertyPath},
    family::FamilyId,
    identity::{LogicalIdentity, ObjectUuid},
    model::{CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind},
    provenance::{CanonicalAnchor, SourceProvenance},
    validate::validate_configuration,
    value::{CanonicalField, CanonicalText, CanonicalValue},
    version::{PlatformBuild, XmlDialect},
};
use ibcmd_xml::{XmlReader, bundled_metadata_registry};

const CONFIG: &str = "00000000-0000-4000-8000-000000000001";
const FORM_XML: &[u8] = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20"><Width>480</Width><Height>320</Height><AutoTitle>false</AutoTitle><AutoCommandBar name="FormCommandBar" id="-1"/><Commands/><Attributes/><Parameters/><ChildItems/></Form>"#;
fn id(n: u128) -> String {
    Uuid::from_u128(n + 0x40008000000000000000).to_string()
}
fn simple(uuid: &str, kind: &str, name: &str, owner: Option<ObjectUuid>) -> CanonicalObject {
    let path = ObjectPath::new(vec![
        PathSegment::name(&format!("{kind}-{name}-{uuid}")).unwrap(),
    ])
    .unwrap();
    let mut p = CanonicalObjectParts::new(
        LogicalIdentity::new(ObjectUuid::parse(uuid).unwrap(), path.clone()),
        MetadataKind::new(kind).unwrap(),
        SourceProvenance::new(
            ProfileId::parse("xml-2.20").unwrap(),
            CanonicalAnchor::new(path, PropertyPath::root()),
        ),
    );
    p.owner = owner;
    p.properties.push(
        CanonicalField::named(
            "Name",
            CanonicalValue::text(CanonicalText::new(name).unwrap()),
        )
        .unwrap(),
    );
    CanonicalObject::new(p).unwrap()
}
fn profile() -> DocumentFormProfile {
    let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
    DocumentFormProfile::from_effective(
        registry
            .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
            .unwrap(),
    )
    .unwrap()
}

// Reuses the clean-room Document property fixture of the typed codec tests.
fn document_xml(owner: &str, name: &str, seed: u128) -> Vec<u8> {
    let generated = ["Object", "Ref", "Selection", "List", "Manager"].into_iter().enumerate().map(|(index, category)| format!("<xr:GeneratedType name=\"Document{category}.{name}\" category=\"{category}\"><xr:TypeId>{}</xr:TypeId><xr:ValueId>{}</xr:ValueId></xr:GeneratedType>", id(seed + index as u128 * 2), id(seed + index as u128 * 2 + 1))).collect::<String>();
    format!(r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:cfg="http://v8.1c.ru/8.1/data/enterprise/current-config" version="2.20"><Document uuid="{owner}"><InternalInfo>{generated}</InternalInfo><Properties><Name>{name}</Name><Synonym/><Comment/><UseStandardCommands>true</UseStandardCommands><Numerator/><NumberType>String</NumberType><NumberLength>11</NumberLength><NumberAllowedLength>Variable</NumberAllowedLength><NumberPeriodicity>Year</NumberPeriodicity><CheckUnique>true</CheckUnique><Autonumbering>true</Autonumbering><StandardAttributes/><Characteristics/><BasedOn/><InputByString></InputByString><CreateOnInput>DontUse</CreateOnInput><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><DefaultObjectForm></DefaultObjectForm><DefaultListForm/><DefaultChoiceForm/><AuxiliaryObjectForm/><AuxiliaryListForm/><AuxiliaryChoiceForm/><Posting>Allow</Posting><RealTimePosting>Allow</RealTimePosting><RegisterRecordsDeletion>AutoDelete</RegisterRecordsDeletion><RegisterRecordsWritingOnPost>WriteSelected</RegisterRecordsWritingOnPost><SequenceFilling>AutoFill</SequenceFilling><RegisterRecords/><PostInPrivilegedMode>false</PostInPrivilegedMode><UnpostInPrivilegedMode>false</UnpostInPrivilegedMode><IncludeHelpInContents>false</IncludeHelpInContents><DataLockFields/><DataLockControlMode>Managed</DataLockControlMode><FullTextSearch>Use</FullTextSearch><ObjectPresentation/><ExtendedObjectPresentation/><ListPresentation/><ExtendedListPresentation/><Explanation/><ChoiceHistoryOnInput>DontUse</ChoiceHistoryOnInput><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties><ChildObjects><Form>Main</Form></ChildObjects></Document></MetaDataObject>"#).into_bytes()
}
fn fixture(count: usize) -> Rows {
    let registry = crate::profile_registry::load_bundled_profile_registry().unwrap();
    let profile_id = ProfileId::parse("platform-8.3.27.1989").unwrap();
    let effective = registry.get(&profile_id).unwrap();
    let special = SpecialEntryProfile::from_effective(effective).unwrap();
    let form_profile = profile();
    let mut objects = vec![simple(CONFIG, "Configuration", "Synthetic", None)];
    for n in 0..count {
        let owner = id(100 + n as u128);
        let name = format!("Doc{n}");
        let xml = document_xml(&owner, &name, 1000 + n as u128 * 100);
        let source = XmlReader::from_slice(&xml).unwrap();
        let envelope = bundled_metadata_registry()
            .decode(
                &FamilyId::parse("Document").unwrap(),
                &source,
                ProfileId::parse("xml-2.20").unwrap(),
                ObjectPath::new(vec![PathSegment::name(&name).unwrap()]).unwrap(),
            )
            .unwrap();
        objects.push(envelope.root().clone());
        objects.extend(envelope.descendants().iter().cloned());
        objects.push(simple(
            &id(200 + n as u128),
            "Form",
            "Main",
            Some(ObjectUuid::parse(&owner).unwrap()),
        ));
    }
    let canonical = CanonicalConfiguration::new(objects).unwrap();
    let validated = validate_configuration(&canonical).unwrap();
    let identities = collect_bootstrap_identities(&validated).unwrap();
    let routes = canonical
        .objects()
        .iter()
        .map(|object| ObjectStorageRoute::new(object.identity().uuid(), vec![]).unwrap())
        .collect();
    let graph = build_bootstrap_graph(&identities, profile_id, routes).unwrap();
    let mut rows = Rows::new();
    for entry in [
        compile_root(&graph, &special).unwrap(),
        compile_version(&graph, &special).unwrap(),
        compile_configuration_body(
            &identities,
            &graph,
            &special,
            &ConfigurationBodyProperties::minimal("Synthetic", special.compatibility()),
        )
        .unwrap(),
    ] {
        rows.insert(
            entry.target().key().as_str().into(),
            entry.outcome().compiled_payload().unwrap().bytes().to_vec(),
        );
    }
    let axes = CompileAxes::new(
        XmlDialect::parse("2.20").unwrap(),
        Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
        None,
        StorageProfileId::parse("storage:mssql-config-configsave").unwrap(),
        None,
    );
    for n in 0..count {
        let owner = ObjectUuid::parse(&id(100 + n as u128)).unwrap();
        let entry =
            compile_document_metadata(&validated, &graph, owner, &axes, &form_profile.document)
                .unwrap();
        rows.insert(
            owner.to_string(),
            entry.outcome().compiled_payload().unwrap().bytes().to_vec(),
        );
        let form = FormMetadataNativeIr {
            uuid: ObjectUuid::parse(&id(200 + n as u128)).unwrap(),
            name: "Main".into(),
            synonyms: vec![],
            comment: String::new(),
            cohort: FormEvidencedCohort::ManagedPlatformAndMobile,
        };
        rows.insert(
            form.uuid.to_string(),
            encode_form_metadata_blob(&form, &form_profile.metadata).unwrap(),
        );
        let route = SourceAssetRegistry
            .route("Form", SourceAssetRole::FormModule)
            .unwrap();
        rows.insert(
            format!("{}{}", form.uuid, route.suffix()),
            compile_managed_form(&form_profile.body, FORM_XML, Some(b"// original"), None).unwrap(),
        );
    }
    let mut names = rows.keys().cloned().collect::<Vec<_>>();
    names.push("versions".into());
    rows.insert(
        "versions".into(),
        pack(&crate::metadata_model::root::versions_row(&names, || {
            id(999)
        })),
    );
    rows
}
fn prepare(rows: &Rows, path: &str, text: &[u8]) -> Result<ModuleEdit> {
    prepare_form_module(
        &snapshot(rows, &Rows::new()),
        path,
        text,
        &profile(),
        Limits::default(),
        &check(),
    )
}
fn path(n: usize) -> String {
    format!("Documents/Doc{n}/Forms/Main/Ext/Form/Module.bsl")
}
fn mutate(rows: &mut Rows, key: &str, edit: impl FnOnce(&mut Brace)) {
    let plain = crate::module_blob::inflate_raw(&rows[key]).unwrap();
    let mut descriptor = parse_row(&plain).unwrap();
    edit(&mut descriptor);
    rows.insert(
        key.into(),
        pack(&crate::metadata_model::brace::serialize_row(&descriptor)),
    );
}
#[test]
fn form_module_preserves_parent_metadata_layout_trailing_and_staged_neighbour() {
    let rows = fixture(2);
    let owner = id(101);
    let saved = Rows::from([
        (owner.clone(), rows[&owner].clone()),
        ("versions".into(), rows["versions"].clone()),
    ]);
    let snapshot = snapshot(&rows, &saved);
    let before = snapshot.digest().unwrap();
    let edit = prepare_form_module(
        &snapshot,
        &path(0),
        b"// edited\n",
        &profile(),
        Limits::default(),
        &check(),
    )
    .unwrap();
    let key = format!("{}.0", id(200));
    assert_eq!(
        edit.replacements.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([key.clone(), "versions".into()])
    );
    let old = decode_managed_form(&profile().body, &rows[&key]).unwrap();
    let new = decode_managed_form(&profile().body, &edit.replacements[&key]).unwrap();
    assert_eq!(new.module_text(), "// edited\n");
    assert_eq!(old.parsed().layout, new.parsed().layout);
    assert_eq!(old.parsed().trailing, new.parsed().trailing);
    assert_eq!(snapshot.digest().unwrap(), before);
    assert!(
        prepare(&rows, &path(0), b"// original")
            .unwrap()
            .replacements
            .is_empty()
    );
}
#[test]
fn form_refuses_wrong_role_parent_and_missing_membership() {
    let rows = fixture(1);
    for wrong in [
        path(1),
        path(0).replace("Module.bsl", "Form.xml"),
        path(0).to_lowercase(),
        path(0).replace("Documents", "Catalogs"),
    ] {
        assert!(prepare(&rows, &wrong, b"// edited").is_err());
    }
    let mut missing = rows.clone();
    mutate(&mut missing, &id(100), |node| {
        assert!(change_members(node, &id(200), None))
    });
    assert!(
        prepare(&missing, &path(0), b"// edited")
            .err()
            .unwrap()
            .to_string()
            .contains("selection is not")
    );
}
#[test]
fn form_refuses_duplicate_and_multiple_parent_occurrences() {
    let rows = fixture(2);
    let mut duplicate = rows.clone();
    mutate(&mut duplicate, &id(100), |node| {
        assert!(change_members(node, &id(200), Some(&id(200))))
    });
    assert!(prepare(&duplicate, &path(0), b"// edit").is_err());
    let mut shared = rows.clone();
    let text = String::from_utf8(crate::module_blob::inflate_raw(&shared[&id(101)]).unwrap())
        .unwrap()
        .replace(&id(201), &id(200));
    shared.insert(id(101), pack(text.as_bytes()));
    assert!(
        prepare(&shared, &path(0), b"// edit")
            .err()
            .unwrap()
            .to_string()
            .contains("multiple parents")
    );
}
#[test]
fn form_refuses_changed_descriptor_id_and_unsupported_root_owner() {
    let rows = fixture(1);
    for key in [id(100), id(200)] {
        let mut changed = rows.clone();
        let text = String::from_utf8(crate::module_blob::inflate_raw(&rows[&key]).unwrap())
            .unwrap()
            .replace(&key, &id(909));
        changed.insert(key, pack(text.as_bytes()));
        assert!(
            prepare(&changed, &path(0), b"// edit")
                .err()
                .unwrap()
                .to_string()
                .contains("identity changed")
        );
    }
    let other = super::super::tests::fixture();
    assert!(
        prepare(&other, &path(0), b"// edit")
            .err()
            .unwrap()
            .to_string()
            .contains("unexamined root family")
    );
}
#[test]
fn form_refuses_body_profile_budget_and_cancellation_failures() {
    let rows = fixture(1);
    let key = format!("{}.0", id(200));
    let mut broken = rows.clone();
    broken.insert(key, pack(b"not a managed Form"));
    assert!(prepare(&broken, &path(0), b"// edit").is_err());
    let snapshot = snapshot(&rows, &Rows::new());
    assert!(
        prepare_form_module(
            &snapshot,
            &path(0),
            b"// edit",
            &profile(),
            Limits {
                inflated_bytes: 1,
                ..Limits::default()
            },
            &check()
        )
        .is_err()
    );
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let cancel: Check = std::sync::Arc::new(move || {
        if observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 8 {
            anyhow::bail!("form-cancel");
        }
        Ok(())
    });
    assert_eq!(
        prepare_form_module(
            &snapshot,
            &path(0),
            b"// edit",
            &profile(),
            Limits::default(),
            &cancel
        )
        .err()
        .unwrap()
        .to_string(),
        "form-cancel"
    );
    let mut effective = crate::profile_registry::load_bundled_profile_registry()
        .unwrap()
        .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
        .unwrap()
        .clone();
    effective.storage_profile = None;
    assert!(DocumentFormProfile::from_effective(&effective).is_err());
}

#[test]
fn form_refuses_case_colliding_parent_and_sibling_names() {
    let mut rows = fixture(2);
    let plain =
        String::from_utf8(crate::module_blob::inflate_raw(&rows[&id(101)]).unwrap()).unwrap();
    rows.insert(id(101), pack(plain.replace("Doc1", "doc0").as_bytes()));
    assert!(
        prepare(&rows, &path(0), b"// edit")
            .err()
            .unwrap()
            .to_string()
            .contains("ambiguous SQL Document source path")
    );
    let mut rows = fixture(1);
    let mut form = decode_form_metadata_blob(&rows[&id(200)], &profile().metadata).unwrap();
    form.uuid = ObjectUuid::parse(&id(909)).unwrap();
    form.name = "main".into();
    rows.insert(
        id(909),
        encode_form_metadata_blob(&form, &profile().metadata).unwrap(),
    );
    rows.insert(
        format!("{}.0", id(909)),
        rows[&format!("{}.0", id(200))].clone(),
    );
    mutate(&mut rows, &id(100), |node| {
        assert!(change_members(node, &id(200), Some(&id(909))))
    });
    let names = rows.keys().cloned().collect::<Vec<_>>();
    rows.insert(
        "versions".into(),
        pack(&crate::metadata_model::root::versions_row(&names, || {
            id(999)
        })),
    );
    assert!(
        prepare(&rows, &path(0), b"// edit")
            .err()
            .unwrap()
            .to_string()
            .contains("ambiguous SQL Form source path")
    );
}
#[test]
fn form_refuses_collision_with_another_documents_generated_identity() {
    let mut rows = fixture(2);
    let plain =
        String::from_utf8(crate::module_blob::inflate_raw(&rows[&id(101)]).unwrap()).unwrap();
    rows.insert(id(101), pack(plain.replace(&id(1100), &id(200)).as_bytes()));
    assert!(
        prepare(&rows, &path(0), b"// edit")
            .err()
            .unwrap()
            .to_string()
            .contains("colliding identity")
    );
}

// The caller supplies the recorder's framed physical headers; this fixture owns
// only the existing typed clean-room logical payloads and codec profile.
pub(super) fn transaction_fixture<F>(
    make_snapshot: F,
) -> (Readset, String, Vec<u8>, DocumentFormProfile)
where
    F: FnOnce(&BTreeMap<String, Vec<u8>>, &BTreeMap<String, Vec<u8>>, &str) -> Readset,
{
    let rows = fixture(2);
    let profile = profile();
    let route = SourceAssetRegistry
        .route("Form", SourceAssetRole::FormModule)
        .unwrap();
    let selected_key = format!("{}{}", id(200), route.suffix());
    let old_text = decode_managed_form(&profile.body, &rows[&selected_key])
        .unwrap()
        .module_text()
        .as_bytes()
        .to_vec();
    let neighbour = id(101);
    let saved = BTreeMap::from([
        (neighbour.clone(), rows[&neighbour].clone()),
        ("versions".into(), rows["versions"].clone()),
        (selected_key.clone(), rows[&selected_key].clone()),
    ]);
    (
        make_snapshot(&rows, &saved, &selected_key),
        path(0),
        old_text,
        profile,
    )
}

fn raw_filesystem_form_case() -> (Rows, String, Vec<u8>) {
    let mut rows = fixture(2);
    let key = format!("{}.0", id(200));
    let raw = b"// raw LF form module\n// second line\n".to_vec();
    rows.insert(
        key.clone(),
        compile_managed_form(&profile().body, FORM_XML, Some(&raw), None).unwrap(),
    );
    assert_eq!(
        decode_managed_form(&profile().body, &rows[&key])
            .unwrap()
            .module_text()
            .as_bytes(),
        raw
    );
    (rows, key, raw)
}

#[test]
fn filesystem_form_native_noop_keeps_raw_storage_layout_and_versions() {
    let (rows, key, raw) = raw_filesystem_form_case();
    let saved = Rows::from([
        (id(101), rows[&id(101)].clone()),
        ("versions".into(), rows["versions"].clone()),
    ]);
    let captured = snapshot(&rows, &saved);
    let before = captured.digest().unwrap();
    let source = crate::mssql_dump::native_module_file_bytes(&raw);
    assert!(source.starts_with(b"\xef\xbb\xbf"));
    assert!(source.windows(2).any(|pair| pair == b"\r\n"));
    assert_ne!(source, raw);
    let original_source = source.clone();
    let old_body = rows[&key].clone();
    let old_versions = rows["versions"].clone();
    let edit = prepare_form_module_file(
        &captured,
        &path(0),
        &source,
        &profile(),
        Limits::default(),
        &check(),
    )
    .unwrap();
    assert!(edit.replacements().is_empty());
    // The native module-only route refuses the abbreviated base-free envelope.
    assert!(
        prepare_form_module_file_layout(&captured, &path(0), &source, Limits::default(), &check(),)
            .is_err()
    );

    // The storage API retains its original EOL semantics for the same bytes.
    assert!(
        !prepare_form_module(
            &captured,
            &path(0),
            &source,
            &profile(),
            Limits::default(),
            &check(),
        )
        .unwrap()
        .replacements()
        .is_empty()
    );
    assert!(
        prepare_form_module(
            &captured,
            &path(0),
            &raw,
            &profile(),
            Limits::default(),
            &check(),
        )
        .unwrap()
        .replacements()
        .is_empty()
    );
    assert_eq!(rows[&key], old_body);
    assert_eq!(rows["versions"], old_versions);
    assert_eq!(source, original_source);
    assert_eq!(captured.digest().unwrap(), before);
}

#[test]
fn filesystem_form_edit_reuses_loader_policy_and_preserves_exact_nonmodule_state() {
    let (rows, key, _) = raw_filesystem_form_case();
    let captured = snapshot(&rows, &Rows::new());
    let before = captured.digest().unwrap();
    let source = b"\xef\xbb\xbf// changed file\n// second line\n".to_vec();
    let original_source = source.clone();
    let expected = crate::load::module_text_from_bytes(&source, &path(0)).unwrap();
    assert_ne!(
        source, expected,
        "LF source must exercise loader CRLF expansion"
    );
    let edit = prepare_form_module_file(
        &captured,
        &path(0),
        &source,
        &profile(),
        Limits::default(),
        &check(),
    )
    .unwrap();
    assert_eq!(
        edit.replacements().keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([key.clone(), "versions".into()])
    );
    // The native module-only route refuses the abbreviated base-free envelope.
    assert!(
        prepare_form_module_file_layout(&captured, &path(0), &source, Limits::default(), &check(),)
            .is_err()
    );
    let old = decode_managed_form(&profile().body, &rows[&key]).unwrap();
    let new = decode_managed_form(&profile().body, &edit.replacements()[&key]).unwrap();
    assert_eq!(
        new.module_text(),
        std::str::from_utf8(&expected)
            .unwrap()
            .trim_start_matches('\u{feff}')
    );
    assert_eq!(old.parsed().revision, new.parsed().revision);
    assert_eq!(old.parsed().layout, new.parsed().layout);
    assert_eq!(old.parsed().trailing, new.parsed().trailing);
    assert_eq!(old.parsed().trailing_fields, new.parsed().trailing_fields);
    let old_versions = crate::module_blob::inflate_raw(&rows["versions"]).unwrap();
    let new_versions = crate::module_blob::inflate_raw(&edit.replacements()["versions"]).unwrap();
    validate_sql_versions_change(&old_versions, &new_versions, &BTreeSet::from([key])).unwrap();
    assert_eq!(source, original_source);
    assert_eq!(captured.digest().unwrap(), before);
}

#[test]
fn filesystem_form_bounds_expanded_storage_text_at_exact_limit() {
    let (rows, key, _) = raw_filesystem_form_case();
    let captured = snapshot(&rows, &Rows::new());
    let source = b"\n\n\n\n";
    let expected = crate::load::module_text_from_bytes(source, &path(0)).unwrap();
    assert_eq!(expected, b"\r\n\r\n\r\n\r\n");
    let error = prepare_form_module_file(
        &captured,
        &path(0),
        source,
        &profile(),
        Limits {
            source_bytes: 7,
            ..Limits::default()
        },
        &check(),
    )
    .err()
    .unwrap();
    assert_eq!(
        error.to_string(),
        "SQL compiled Form module source budget exceeded"
    );
    let edit = prepare_form_module_file(
        &captured,
        &path(0),
        source,
        &profile(),
        Limits {
            source_bytes: 8,
            ..Limits::default()
        },
        &check(),
    )
    .unwrap();
    assert_eq!(
        decode_managed_form(&profile().body, &edit.replacements()[&key])
            .unwrap()
            .module_text()
            .as_bytes(),
        expected
    );
}

#[test]
fn filesystem_form_post_loader_cancellation_retains_typed_error_and_inputs() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (rows, _, _) = raw_filesystem_form_case();
    let captured = snapshot(&rows, &Rows::new());
    let before = captured.digest().unwrap();
    let source = b"\n\n\n\n".to_vec();
    let original_source = source.clone();
    // The expansion-bound refusal identifies the last checkpoint before the
    // post-loader budget, without coupling the test to a resolver walk count.
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let count: Check = Arc::new(move || {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let error = prepare_form_module_file(
        &captured,
        &path(0),
        &source,
        &profile(),
        Limits {
            source_bytes: 7,
            ..Limits::default()
        },
        &count,
    )
    .err()
    .unwrap();
    assert_eq!(
        error.to_string(),
        "SQL compiled Form module source budget exceeded"
    );
    let post_loader_check = calls.load(Ordering::SeqCst);
    assert!(post_loader_check > 1);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let cancel: Check = Arc::new(move || {
        if observed.fetch_add(1, Ordering::SeqCst) + 1 == post_loader_check {
            return Err(crate::cancel::Cancelled.into());
        }
        Ok(())
    });
    let error = prepare_form_module_file(
        &captured,
        &path(0),
        &source,
        &profile(),
        Limits {
            source_bytes: 8,
            ..Limits::default()
        },
        &cancel,
    )
    .err()
    .unwrap();
    assert!(crate::cancel::is_cancelled(&error));
    assert_eq!(calls.load(Ordering::SeqCst), post_loader_check);
    assert_eq!(source, original_source);
    assert_eq!(captured.digest().unwrap(), before);
}

#[test]
fn native_module_envelope_preparation_preserves_full_form_and_staged_neighbour() {
    // Clean-room ten-field envelope, not a new base-free compiler profile.
    let (mut rows, key, raw) = raw_filesystem_form_case();
    let mut plain =
        String::from_utf8(crate::module_blob::inflate_raw(&rows[&key]).unwrap()).unwrap();
    let close = plain.rfind('}').unwrap();
    plain.insert_str(close, ",{0,0},0,0");
    rows.insert(key.clone(), pack(plain.as_bytes()));
    let parsed = crate::module_blob::parse_form_body_blob(&rows[&key]).unwrap();
    assert_eq!(parsed.revision, crate::module_blob::FormBodyRevision::V4);
    assert_eq!(parsed.trailing_fields, 7);
    assert!(decode_managed_form(&profile().body, &rows[&key]).is_err());
    let saved = Rows::from([
        (id(101), rows[&id(101)].clone()),
        ("versions".into(), rows["versions"].clone()),
    ]);
    let captured = snapshot(&rows, &saved);
    let before = captured.digest().unwrap();
    let source = crate::mssql_dump::native_module_file_bytes(&raw);
    let noop =
        prepare_form_module_file_layout(&captured, &path(0), &source, Limits::default(), &check())
            .unwrap();
    assert!(noop.replacements().is_empty());
    let changed_source = "\u{feff}// changed native module\n// Юникод \"quoted\"\n".as_bytes();
    let expected = crate::load::module_text_from_bytes(changed_source, &path(0)).unwrap();
    let edit = prepare_form_module_file_layout(
        &captured,
        &path(0),
        changed_source,
        Limits::default(),
        &check(),
    )
    .unwrap();
    assert_eq!(
        edit.replacements().keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([key.clone(), "versions".into()])
    );
    let changed = crate::module_blob::parse_form_body_blob(&edit.replacements()[&key]).unwrap();
    assert_eq!(
        changed.module_text,
        std::str::from_utf8(&expected)
            .unwrap()
            .trim_start_matches('\u{feff}')
    );
    assert_eq!(changed.revision, parsed.revision);
    assert_eq!(changed.layout, parsed.layout);
    assert_eq!(changed.trailing, parsed.trailing);
    assert_eq!(changed.trailing_fields, parsed.trailing_fields);
    assert_eq!(captured.digest().unwrap(), before);
    assert_eq!(edit.readset_digest(), &before);
    assert_eq!(crate::mssql_dump::native_module_file_bytes(&raw), source);
}
