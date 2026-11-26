use std::fs;
use std::path::{Path, PathBuf};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::{LossPolicy, ObjectPath, PathSegment, PropertyPath, Severity};
use ibcmd_core::family::FamilyId;
use ibcmd_core::identity::{LogicalIdentity, ObjectUuid};
use ibcmd_core::migration::executor::{
    MigrationExecution, MigrationExecutionError, MigrationExecutionRequest, MigrationExecutor,
};
use ibcmd_core::migration::graph::MigrationGraph;
use ibcmd_core::migration::report::MigrationLossDisposition;
use ibcmd_core::migration::v2_20_to_v2_21::V2_20ToV2_21;
use ibcmd_core::migration::v2_21_to_v2_20::{
    INTERFACE_MODE_LOSS_CODE, SOURCE_PROFILE, TARGET_PROFILE, V2_21ToV2_20,
};
use ibcmd_core::model::{
    CanonicalConfiguration, CanonicalObject, CanonicalObjectParts, MetadataKind,
};
use ibcmd_core::profile::ProfileRegistry;
use ibcmd_core::provenance::{CanonicalAnchor, SourceProvenance};
use ibcmd_core::semantic::semantic_digest;
use ibcmd_core::validate::validate_configuration;
use ibcmd_core::value::{CanonicalField, CanonicalText, CanonicalValue, EnumToken};
use ibcmd_rs::profile_registry::load_bundled_profile_registry;
use ibcmd_xml::{XmlReader, bundled_metadata_registry};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const PALETTE_NAMESPACE: &str = "http://v8.1c.ru/8.1/data/ui/colors/palette";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    source_profile: String,
    target_profile: String,
    fixtures: Vec<Fixture>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    id: String,
    path: String,
    family: String,
    #[serde(rename = "case")]
    case_kind: String,
    sha256: String,
    expected_losses: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LossyFormFixture {
    schema_version: u32,
    family: String,
    uuid: String,
    object_path: String,
    name: String,
    property: String,
    value: String,
    loss_code: String,
    default_policy: String,
    warn_action: String,
    drop_action: String,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("migrations")
        .join("2.21-to-2.20")
}

fn manifest() -> Manifest {
    serde_json::from_slice(&fs::read(fixture_root().join("manifest.json")).unwrap()).unwrap()
}

fn profile<'a>(
    profiles: &'a ProfileRegistry,
    id: &str,
) -> &'a ibcmd_core::profile::EffectiveProfile {
    profiles
        .get(&ProfileId::parse(id).expect("fixture profile id is valid"))
        .expect("fixture profile is bundled")
}

fn assert_fixture_digest(fixture: &Fixture, bytes: &[u8]) {
    let digest = Sha256::digest(bytes);
    assert_eq!(format!("{digest:x}"), fixture.sha256, "{}", fixture.id);
}

fn graph(profiles: &ProfileRegistry) -> MigrationGraph {
    MigrationGraph::new(profiles, vec![V2_21ToV2_20::verified_edge()]).unwrap()
}

#[test]
fn exact_profile_pair_is_bidirectionally_plannable() {
    let profiles = load_bundled_profile_registry().unwrap();
    let graph = MigrationGraph::new(
        &profiles,
        vec![V2_20ToV2_21::verified_edge(), V2_21ToV2_20::verified_edge()],
    )
    .unwrap();
    let source = profile(&profiles, SOURCE_PROFILE);
    let target = profile(&profiles, TARGET_PROFILE);

    let downgrade = graph.plan(source, target).unwrap();
    assert_eq!(
        downgrade.step_ids()[0].as_str(),
        "migration:xcf-2.21-to-2.20"
    );
    let upgrade = graph.plan(target, source).unwrap();
    assert_eq!(upgrade.step_ids()[0].as_str(), "migration:xcf-2.20-to-2.21");
}

#[test]
fn portable_xml_fixtures_are_zero_loss_and_target_readable() {
    let root = fixture_root();
    let manifest = manifest();
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.source_profile, SOURCE_PROFILE);
    assert_eq!(manifest.target_profile, TARGET_PROFILE);

    let profiles = load_bundled_profile_registry().unwrap();
    let source_profile = profile(&profiles, SOURCE_PROFILE);
    let target_profile = profile(&profiles, TARGET_PROFILE);
    let graph = graph(&profiles);
    let plan = graph.plan(source_profile, target_profile).unwrap();
    let codecs = bundled_metadata_registry();

    for fixture in manifest
        .fixtures
        .iter()
        .filter(|fixture| fixture.family != "Form")
    {
        let bytes = fs::read(root.join(&fixture.path)).unwrap();
        assert_fixture_digest(fixture, &bytes);
        assert!(fixture.expected_losses.is_empty());
        assert!(matches!(
            fixture.case_kind.as_str(),
            "unchanged" | "changed"
        ));
        let source_text = std::str::from_utf8(&bytes).unwrap();
        if fixture.case_kind == "changed" {
            assert!(source_text.contains(PALETTE_NAMESPACE));
        }

        let family = FamilyId::parse(&fixture.family).unwrap();
        let document = XmlReader::from_slice(&bytes).unwrap();
        let envelope = codecs
            .decode(
                &family,
                &document,
                ProfileId::parse(SOURCE_PROFILE).unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        let source_configuration = envelope.configuration().unwrap();
        let source_digest =
            semantic_digest(&validate_configuration(&source_configuration).unwrap());

        let execution = MigrationExecutor::new(&graph)
            .execute(MigrationExecutionRequest::new(
                &plan,
                &source_configuration,
                LossPolicy::Error,
            ))
            .unwrap();
        assert!(execution.report().is_complete(), "{}", fixture.id);
        assert!(execution.report().steps()[0].losses().is_empty());
        assert_eq!(execution.configuration(), &source_configuration);

        let (migrated, _) = execution.into_parts();
        let mut objects = migrated.into_objects();
        let migrated_root = objects.remove(0);
        let migrated_envelope = envelope.with_model(migrated_root, objects).unwrap();
        let encoded = codecs
            .encode(
                &migrated_envelope,
                &ProfileId::parse(TARGET_PROFILE).unwrap(),
            )
            .unwrap();
        let encoded_text = std::str::from_utf8(&encoded).unwrap();
        assert!(encoded_text.contains("version=\"2.20\""));
        assert!(!encoded_text.contains(PALETTE_NAMESPACE));

        let target_document = XmlReader::from_slice(&encoded).unwrap();
        let target_envelope = codecs
            .decode(
                &family,
                &target_document,
                ProfileId::parse(TARGET_PROFILE).unwrap(),
                ObjectPath::root(),
            )
            .unwrap();
        let target_configuration = target_envelope.configuration().unwrap();
        let target_digest =
            semantic_digest(&validate_configuration(&target_configuration).unwrap());
        assert_eq!(source_digest, target_digest, "{}", fixture.id);
    }
}

fn lossy_form_fixture() -> (Fixture, LossyFormFixture) {
    let manifest = manifest();
    let fixture = manifest
        .fixtures
        .into_iter()
        .find(|fixture| fixture.case_kind == "lossy")
        .unwrap();
    let bytes = fs::read(fixture_root().join(&fixture.path)).unwrap();
    assert_fixture_digest(&fixture, &bytes);
    let model = serde_json::from_slice(&bytes).unwrap();
    (fixture, model)
}

fn form_configuration(fixture: &LossyFormFixture) -> CanonicalConfiguration {
    assert_eq!(fixture.schema_version, 1);
    assert_eq!(fixture.family, "Form");
    assert_eq!(fixture.property, "UseInInterfaceCompatibilityMode");
    assert_eq!(fixture.value, "Any");
    let path = ObjectPath::new(vec![PathSegment::name(&fixture.object_path).unwrap()]).unwrap();
    let mut parts = CanonicalObjectParts::new(
        LogicalIdentity::new(ObjectUuid::parse(&fixture.uuid).unwrap(), path.clone()),
        MetadataKind::new(&fixture.family).unwrap(),
        SourceProvenance::new(
            ProfileId::parse(SOURCE_PROFILE).unwrap(),
            CanonicalAnchor::new(path, PropertyPath::root()),
        ),
    );
    parts.properties = vec![
        CanonicalField::named(
            "Name",
            CanonicalValue::text(CanonicalText::new(&fixture.name).unwrap()),
        )
        .unwrap(),
        CanonicalField::named(
            &fixture.property,
            CanonicalValue::enum_token(EnumToken::new(&fixture.value).unwrap()),
        )
        .unwrap(),
    ];
    CanonicalConfiguration::new(vec![CanonicalObject::new(parts).unwrap()]).unwrap()
}

fn execute_form(
    source: &CanonicalConfiguration,
    policy: LossPolicy,
) -> Result<MigrationExecution, MigrationExecutionError> {
    let profiles = load_bundled_profile_registry().unwrap();
    let graph = graph(&profiles);
    let plan = graph
        .plan(
            profile(&profiles, SOURCE_PROFILE),
            profile(&profiles, TARGET_PROFILE),
        )
        .unwrap();
    MigrationExecutor::new(&graph).execute(MigrationExecutionRequest::new(&plan, source, policy))
}

fn has_property(configuration: &CanonicalConfiguration, name: &str) -> bool {
    configuration.objects()[0]
        .properties()
        .iter()
        .any(|field| field.name().as_str() == name)
}

#[test]
fn default_error_warn_and_explicit_drop_are_distinct_and_reported() {
    let (fixture_record, fixture) = lossy_form_fixture();
    assert_eq!(fixture.default_policy, "error");
    assert_eq!(fixture.warn_action, "retain");
    assert_eq!(fixture.drop_action, "remove");
    assert_eq!(fixture.loss_code, INTERFACE_MODE_LOSS_CODE);
    assert_eq!(fixture_record.expected_losses, [INTERFACE_MODE_LOSS_CODE]);
    let source = form_configuration(&fixture);
    let snapshot = source.clone();

    let error = execute_form(&source, LossPolicy::Error).unwrap_err();
    let error_report = error.report().unwrap();
    assert_eq!(source, snapshot);
    let blocked = &error_report.steps()[0].apply_diagnostics().diagnostics()[0];
    assert_eq!(blocked.code().as_str(), INTERFACE_MODE_LOSS_CODE);
    assert_eq!(blocked.severity(), Severity::Error);
    assert_eq!(blocked.object_path(), source.objects()[0].identity().path());
    assert_eq!(
        blocked.property_path(),
        &PropertyPath::new(vec![
            PathSegment::name("UseInInterfaceCompatibilityMode").unwrap()
        ])
        .unwrap()
    );

    let warned = execute_form(&source, LossPolicy::Warn).unwrap();
    assert!(has_property(warned.configuration(), &fixture.property));
    let warning = &warned.report().steps()[0].losses()[0];
    assert_eq!(
        warning.actual_disposition(),
        MigrationLossDisposition::ContinueWithWarning
    );
    assert_eq!(
        warning.diagnostic().code().as_str(),
        INTERFACE_MODE_LOSS_CODE
    );

    let dropped = execute_form(&source, LossPolicy::DropExplicitly).unwrap();
    assert!(!has_property(dropped.configuration(), &fixture.property));
    assert!(has_property(dropped.configuration(), "Name"));
    let loss = &dropped.report().steps()[0].losses()[0];
    assert_eq!(
        loss.actual_disposition(),
        MigrationLossDisposition::DroppedExplicitly
    );
    assert_eq!(loss.diagnostic().code().as_str(), INTERFACE_MODE_LOSS_CODE);
    assert_eq!(loss.diagnostic().object_path(), blocked.object_path());
    assert_eq!(loss.diagnostic().property_path(), blocked.property_path());

    let report_json = serde_json::to_string(dropped.report()).unwrap();
    assert!(report_json.contains(INTERFACE_MODE_LOSS_CODE));
    assert!(report_json.contains("dropped_explicitly"));
    assert_eq!(source, snapshot);
}
