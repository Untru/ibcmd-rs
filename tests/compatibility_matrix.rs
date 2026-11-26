use std::{path::PathBuf, process::Command};

use ibcmd_rs::{
    compatibility::{
        BUNDLED_COMPATIBILITY_MATRIX_JSON, BUNDLED_COMPATIBILITY_SCHEMA_JSON, CompatibilityLookup,
        CompatibilityQuery, RouteSupport, UnsupportedReason, VerificationStatus,
        current_compatibility_report, parse_compatibility_matrix, validate_repository_evidence,
    },
    profile_registry::load_bundled_profile_registry,
};
use serde_json::Value;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn bundled_matrix_and_schema_are_strict_repository_evidence() {
    let schema: Value = serde_json::from_str(BUNDLED_COMPATIBILITY_SCHEMA_JSON).unwrap();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(schema["properties"]["schema_version"]["const"], 1);

    let matrix = current_compatibility_report().unwrap();
    validate_repository_evidence(&matrix, &repository_root()).unwrap();
    assert!(matrix.routes.iter().all(|route| !route.evidence.is_empty()));
    assert!(matrix.routes.iter().any(|route| {
        route.support == RouteSupport::Supported && route.status == VerificationStatus::Verified
    }));
    assert!(matrix.routes.iter().any(|route| {
        route.support == RouteSupport::Unsupported
            && route.status == VerificationStatus::Experimental
    }));
}

#[test]
fn verified_without_green_test_evidence_is_rejected() {
    let profiles = load_bundled_profile_registry().unwrap();
    let mut matrix = parse_compatibility_matrix(BUNDLED_COMPATIBILITY_MATRIX_JSON).unwrap();
    let route = matrix
        .routes
        .iter_mut()
        .find(|route| route.status == VerificationStatus::Verified)
        .unwrap();
    route.evidence.retain(|id| !id.starts_with("test-"));

    let error =
        ibcmd_rs::compatibility::validate_compatibility_matrix(&matrix, &profiles).unwrap_err();
    assert!(error.to_string().contains("evidence derives Experimental"));
}

#[test]
fn exact_lookup_rejects_unknown_and_unrecorded_profiles_or_routes() {
    let profiles = load_bundled_profile_registry().unwrap();
    let matrix = current_compatibility_report().unwrap();

    let unknown = CompatibilityQuery {
        operation: "convert",
        source_artifact: "xml_source_tree",
        target_artifact: "cf_archive",
        source_profile: "xml-2.99",
        target_profile: "platform-8.3.27.1989",
        family: "CommonModule",
    };
    assert_eq!(
        matrix.lookup(&unknown, &profiles),
        CompatibilityLookup::Unsupported(UnsupportedReason::UnknownSourceProfile)
    );

    let absent = CompatibilityQuery {
        operation: "convert",
        source_artifact: "xml_source_tree",
        target_artifact: "cf_archive",
        source_profile: "xml-2.21",
        target_profile: "platform-8.3.27.1989",
        family: "Catalog",
    };
    assert_eq!(
        matrix.lookup(&absent, &profiles),
        CompatibilityLookup::Unsupported(UnsupportedReason::NoExactRoute)
    );
}

#[test]
fn compatibility_cli_emits_the_embedded_source_of_truth() {
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .arg("compatibility")
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let cli: Value = serde_json::from_slice(&output.stdout).unwrap();
    let embedded: Value = serde_json::from_str(BUNDLED_COMPATIBILITY_MATRIX_JSON).unwrap();
    assert_eq!(cli, embedded);
}
