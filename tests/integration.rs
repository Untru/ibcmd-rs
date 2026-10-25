//! The root package's integration suites as one test binary.
//!
//! Every `tests/<suite>.rs` below is a module of this crate, so the root
//! library is linked into one test executable instead of one per suite. Its
//! tests are named `<suite>::<test>`; one suite runs with
//! `cargo test --locked -p ibcmd-rs --test integration <suite>:: --no-default-features`.
//!
//! The suites share this process and run in parallel with each other, so a
//! suite here must not change process-wide state: no environment variables or
//! working directory, scratch paths of its own (`common::temp_dir` refuses a
//! tag two suites share), and no in-process run of the engine parts that keep
//! one operation per process (cancellation, offline rows, offline compile,
//! stored-object views). A suite that needs that, or its own `main`, is a
//! `[[test]]` target of its own in `Cargo.toml`. The manifest sets
//! `autotests = false`; `every_suite_file_is_built` checks that each file is
//! built exactly once.

mod common;

mod bootstrap_artifact_scope;
mod bootstrap_external_intake;
mod bootstrap_external_named_owners;
mod cache_insertion_trace;
mod cache_slot_inspection;
mod cache_trace_typed_diagnostic;
mod cf_bootstrap;
mod cf_bootstrap_external;
mod cf_bootstrap_package_intent;
mod cf_cli;
mod cf_config_save;
mod cf_corpus;
mod cf_corruption;
mod cf_eds_matrix;
mod cf_empty_child_collections;
mod cf_export;
mod cf_export_opaque;
mod cf_external_data_source;
mod cf_load_compiled;
mod cf_native_roundtrip;
mod cf_ordinary_empty_families;
mod cf_overlay;
mod cf_platform_8_5;
mod cf_roundtrip;
mod cf_standard_attribute_presence;
mod cf_update;
mod cf_websocket_client;
mod cfe_source_roundtrip;
mod choice_list_dates;
mod closed_modules;
mod compatibility_matrix;
mod configuration_properties;
mod conversion_cli;
mod default_style;
mod dropin_cli;
mod editor_native_status;
mod edt_conversion_cli;
mod export_base_sync;
mod export_notes;
mod extension_adopted;
mod extension_adopted_properties;
mod extension_export;
mod extension_v85;
mod external_canonical_root;
mod external_corpus;
mod external_export;
mod file_extension_export;
mod file_infobase;
mod form_compat_namespace;
mod form_events;
mod home_page_work_area;
mod import_files;
mod interface_metadata;
// Its descriptor capability boundary is Unix-only.
#[cfg(unix)]
mod mcp_adapter;
mod migration_2_20_to_2_21;
mod migration_2_21_to_2_20;
mod native_export_commands;
mod offline_conversion_matrix;
mod old_roots;
#[cfg(not(feature = "platform-oracle"))]
mod platform_oracle_boundary;
mod platform_probes;
mod portable_root_smoke;
mod postgres_export;
mod postgres_import;
mod record_upgrade;
mod register_main_filter;
mod role_rights;
mod root_command_interface;
mod source_parity_matrix;
mod source_three_way_oracle;
mod template_fonts;
mod xml_profiles;

#[test]
fn scratch_tags_are_refused_across_suites() {
    common::claim_tag("integration-claim-probe", "tests/first.rs");
    common::claim_tag("integration-claim-probe", "tests/first.rs");
    let second = std::panic::catch_unwind(|| {
        common::claim_tag("integration-claim-probe", "tests/second.rs");
    });
    assert!(second.is_err(), "a second suite claimed the same tag");
}

/// Each `tests/<suite>.rs` is a module here or a `[[test]]` target in
/// `Cargo.toml`, never both: with `autotests = false` nothing else builds it.
#[test]
fn every_suite_file_is_built() {
    use std::collections::BTreeSet;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let modules: BTreeSet<&str> = include_str!("integration.rs")
        .lines()
        .filter_map(|line| line.strip_prefix("mod ")?.strip_suffix(';'))
        .filter(|module| *module != "common")
        .collect();
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let targets: BTreeSet<&str> = manifest
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("path = \"tests/")?
                .strip_suffix(".rs\"")
        })
        .filter(|target| *target != "integration")
        .collect();
    let mut files = BTreeSet::new();
    for entry in std::fs::read_dir(root.join("tests")).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
            if stem != "integration" {
                files.insert(stem);
            }
        }
    }
    let both: Vec<_> = modules.intersection(&targets).collect();
    assert!(both.is_empty(), "suites built twice: {both:?}");
    let declared: BTreeSet<String> = modules
        .union(&targets)
        .map(|suite| (*suite).to_owned())
        .collect();
    let unbuilt: Vec<_> = files.difference(&declared).collect();
    assert!(
        unbuilt.is_empty(),
        "tests/<suite>.rs files that no target builds: {unbuilt:?}; add `mod <suite>;` to tests/integration.rs"
    );
    let missing: Vec<_> = declared.difference(&files).collect();
    assert!(
        missing.is_empty(),
        "declared suites without a file: {missing:?}"
    );
}
