//! A successful native import may discard an inapplicable property. Such a
//! round trip must not be counted as coverage of the requested feature.
use std::{fs, path::PathBuf};

use serde_json::Value;

#[test]
fn corpus_features_are_retained_by_the_native_round_trip() {
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let cases: Value = serde_json::from_slice(
        &fs::read(fixtures.join("coverage-records/native-features.json")).unwrap(),
    )
    .unwrap();
    for case in cases["features"].as_array().unwrap() {
        let platforms: Vec<&str> = match case["platforms"].as_array() {
            Some(values) => values.iter().map(|v| v.as_str().unwrap()).collect(),
            None => vec!["8.3.27.2214", "8.5.1.1150"],
        };
        assert!(!platforms.is_empty());
        for platform in platforms {
            let set = fixtures
                .join("coverage")
                .join(platform)
                .join(case["set"].as_str().unwrap());
            let manifest: Value =
                serde_json::from_slice(&fs::read(set.join("manifest.json")).unwrap()).unwrap();
            assert_eq!(manifest["evidence"]["status"], "platform_proven");
            let path = case["file"].as_str().unwrap();
            assert!(
                manifest["variants"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|variant| {
                        variant["status"] == "platform_proven"
                            && variant["files"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|file| file["path"] == path)
                    }),
                "{platform}: feature {} is not owned by a proven variant",
                case["id"]
            );
            let bytes = fs::read(set.join("native").join(path)).unwrap();
            let text = std::str::from_utf8(&bytes).unwrap();
            for marker in case["contains"].as_array().unwrap() {
                let marker = marker.as_str().unwrap();
                assert!(
                    text.contains(marker),
                    "{platform}: native discarded feature {} ({path}): {marker}",
                    case["id"]
                );
            }
        }
    }
}
