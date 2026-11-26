//! Native SQL registry data from the public CovCombined extension, not an app.
#[path = "common/coverage_paths.rs"]
mod coverage_paths;

use ibcmd_rs::mssql_extensions::{ExtensionRegistryDecodeError, decode_extension_zipped_info};

#[test]
fn public_coverage_sql_envelope_reproduces_the_registry_gap() {
    let encoded = std::fs::read_to_string(
        coverage_paths::root().join("coverage-records/extension-sql-8327.hex"),
    )
    .unwrap();
    let hex = encoded.trim();
    assert_eq!(hex.len() % 2, 0);
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect::<Vec<_>>();
    match decode_extension_zipped_info(&bytes) {
        Ok(info) => {
            assert!(info.active, "the native-applied extension was active");
            println!("coverage SQL extension envelope: newly decoded");
        }
        Err(ExtensionRegistryDecodeError::UnsupportedEnvelopeHeader) => {
            println!("coverage SQL extension envelope: known UnsupportedEnvelopeHeader gap");
        }
        Err(error) => panic!("unexpected registry failure: {error}"),
    }
}
