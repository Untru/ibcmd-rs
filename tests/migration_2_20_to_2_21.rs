use std::fs;
use std::path::{Path, PathBuf};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::{LossPolicy, ObjectPath};
use ibcmd_core::family::FamilyId;
use ibcmd_core::migration::executor::{MigrationExecutionRequest, MigrationExecutor};
use ibcmd_core::migration::graph::MigrationGraph;
use ibcmd_core::migration::v2_20_to_v2_21::{
    INTERFACE_MODE_CAPABILITY, PALETTE_CAPABILITY, SOURCE_PROFILE, TARGET_PROFILE, V2_20ToV2_21,
};
use ibcmd_core::profile::{CapabilityState, EffectiveProfile, ProfileRegistry};
use ibcmd_core::semantic::semantic_digest;
use ibcmd_core::validate::validate_configuration;
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
struct CapabilityFixture {
    schema_version: u32,
    source_profile: String,
    target_profile: String,
    capabilities: Vec<String>,
    source_status: String,
    target_status: String,
    expected_losses: Vec<String>,
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("migrations")
        .join("2.20-to-2.21")
}

fn profile<'a>(profiles: &'a ProfileRegistry, id: &str) -> &'a EffectiveProfile {
    profiles
        .get(&ProfileId::parse(id).expect("fixture profile id is valid"))
        .expect("fixture profile is bundled")
}

fn assert_fixture_digest(fixture: &Fixture, bytes: &[u8]) {
    let digest = Sha256::digest(bytes);
    assert_eq!(format!("{digest:x}"), fixture.sha256, "{}", fixture.id);
}

#[test]
fn evidenced_upgrade_fixtures_are_zero_loss_and_target_readable() {
    let root = fixture_root();
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.schema_version, 1);
    assert_eq!(manifest.source_profile, SOURCE_PROFILE);
    assert_eq!(manifest.target_profile, TARGET_PROFILE);

    let profiles = load_bundled_profile_registry().unwrap();
    let source_profile = profile(&profiles, SOURCE_PROFILE);
    let target_profile = profile(&profiles, TARGET_PROFILE);
    let graph = MigrationGraph::new(&profiles, vec![V2_20ToV2_21::verified_edge()]).unwrap();
    let plan = graph.plan(source_profile, target_profile).unwrap();
    let codecs = bundled_metadata_registry();

    for fixture in manifest
        .fixtures
        .iter()
        .filter(|fixture| fixture.family != "profile")
    {
        let bytes = fs::read(root.join(&fixture.path)).unwrap();
        assert_fixture_digest(fixture, &bytes);
        assert!(fixture.expected_losses.is_empty());
        assert!(matches!(
            fixture.case_kind.as_str(),
            "unchanged" | "changed"
        ));

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
        assert_eq!(execution.report().steps().len(), 1, "{}", fixture.id);
        assert!(
            execution.report().steps()[0].losses().is_empty(),
            "{}",
            fixture.id
        );
        assert_eq!(
            execution.configuration(),
            &source_configuration,
            "{}",
            fixture.id
        );

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
        assert!(encoded_text.contains("version=\"2.21\""), "{}", fixture.id);
        match fixture.case_kind.as_str() {
            "unchanged" => assert!(!encoded_text.contains(PALETTE_NAMESPACE)),
            "changed" => assert!(encoded_text.contains(PALETTE_NAMESPACE)),
            _ => unreachable!(),
        }

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

#[test]
fn newly_supported_capabilities_are_explicit_profile_evidence() {
    let root = fixture_root();
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(root.join("manifest.json")).unwrap()).unwrap();
    let fixture = manifest
        .fixtures
        .iter()
        .find(|fixture| fixture.case_kind == "newly-supported")
        .unwrap();
    let bytes = fs::read(root.join(&fixture.path)).unwrap();
    assert_fixture_digest(fixture, &bytes);

    let evidence: CapabilityFixture = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(evidence.schema_version, 1);
    assert_eq!(evidence.source_profile, SOURCE_PROFILE);
    assert_eq!(evidence.target_profile, TARGET_PROFILE);
    assert_eq!(evidence.source_status, "unsupported");
    assert_eq!(evidence.target_status, "supported");
    assert!(evidence.expected_losses.is_empty());
    assert_eq!(
        evidence.capabilities,
        [PALETTE_CAPABILITY, INTERFACE_MODE_CAPABILITY]
    );

    let profiles = load_bundled_profile_registry().unwrap();
    let source = profile(&profiles, SOURCE_PROFILE);
    let target = profile(&profiles, TARGET_PROFILE);
    for capability in evidence.capabilities {
        let capability = ibcmd_core::profile::CapabilityId::parse(&capability).unwrap();
        assert_eq!(
            source.capabilities.get(&capability).unwrap().value,
            CapabilityState::Unsupported
        );
        assert_eq!(
            target.capabilities.get(&capability).unwrap().value,
            CapabilityState::Supported
        );
    }
}
