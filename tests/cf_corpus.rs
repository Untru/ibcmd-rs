use std::{env, fs, path::PathBuf};

use serde_json::Value;
use sha2::{Digest, Sha256};

const FORMAT16_BASE_OFFSET: usize = 0x1359;
const FORMAT15_SENTINEL: u64 = 0x7fff_ffff;
const FORMAT16_SENTINEL: u64 = 0xffff_ffff_ffff_ffff;

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cf")
}

fn manifest() -> Value {
    serde_json::from_slice(&fs::read(corpus_root().join("manifest.json")).unwrap()).unwrap()
}

fn decode_base64(source: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = 0_u32;
    let mut bits = 0_u8;
    let mut saw_padding = false;

    for byte in source.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            saw_padding = true;
            continue;
        }
        if saw_padding {
            return Err("non-padding data after Base64 padding".to_string());
        }
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err(format!("invalid Base64 byte 0x{byte:02x}")),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xff) as u8);
            buffer &= if bits == 0 { 0 } else { (1_u32 << bits) - 1 };
        }
    }

    if bits > 0 && buffer != 0 {
        return Err("non-zero trailing Base64 bits".to_string());
    }
    Ok(output)
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn parse_hex(bytes: &[u8]) -> u64 {
    u64::from_str_radix(std::str::from_utf8(bytes).unwrap(), 16).unwrap()
}

fn block_header(bytes: &[u8], offset: usize, width: usize) -> (u64, u64, u64) {
    let size = 3 * width + 7;
    let raw = &bytes[offset..offset + size];
    assert_eq!(&raw[..2], b"\r\n");
    assert_eq!(raw[2 + width], b' ');
    assert_eq!(raw[3 + 2 * width], b' ');
    assert_eq!(raw[4 + 3 * width], b' ');
    assert_eq!(&raw[5 + 3 * width..], b"\r\n");
    (
        parse_hex(&raw[2..2 + width]),
        parse_hex(&raw[3 + width..3 + 2 * width]),
        parse_hex(&raw[4 + 2 * width..4 + 3 * width]),
    )
}

fn utf16_name(name: &str) -> Vec<u8> {
    name.encode_utf16()
        .flat_map(u16::to_le_bytes)
        .chain([0, 0, 0, 0])
        .collect()
}

fn find_from(haystack: &[u8], needle: &[u8], start: usize) -> Option<usize> {
    haystack[start..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position + start)
}

fn assert_ordered_names(bytes: &[u8], names: &[Value]) {
    let mut cursor = 0;
    for name in names {
        let name = name.as_str().unwrap();
        let needle = utf16_name(name);
        cursor = find_from(bytes, &needle, cursor)
            .unwrap_or_else(|| panic!("missing ordered UTF-16LE entry name {name}"));
        cursor += needle.len();
    }
}

fn assert_format15(bytes: &[u8], base: usize, count: u32, require_single_page: bool) {
    assert_eq!(read_u32(bytes, base), FORMAT15_SENTINEL as u32);
    assert_eq!(read_u32(bytes, base + 4), 512);
    assert_eq!(read_u32(bytes, base + 8), count);
    assert_eq!(read_u32(bytes, base + 12), 0);
    let (toc_size, toc_page, next) = block_header(bytes, base + 16, 8);
    assert_eq!(toc_size, u64::from(count) * 12);
    assert!(toc_page > 0);
    if require_single_page {
        assert!(toc_page >= toc_size);
        assert_eq!(next, FORMAT15_SENTINEL);
    } else if toc_size > toc_page {
        assert_ne!(next, FORMAT15_SENTINEL);
    }
}

fn assert_format16(bytes: &[u8], base: usize, count: u32, require_single_page: bool) {
    assert_eq!(read_u64(bytes, base), FORMAT16_SENTINEL);
    assert_eq!(read_u32(bytes, base + 8), 512);
    assert_eq!(read_u32(bytes, base + 12), count);
    assert_eq!(read_u32(bytes, base + 16), 0);
    let (toc_size, toc_page, next) = block_header(bytes, base + 20, 16);
    assert_eq!(toc_size, u64::from(count) * 24);
    assert!(toc_page > 0);
    if require_single_page {
        assert!(toc_page >= toc_size);
        assert_eq!(next, FORMAT16_SENTINEL);
    } else if toc_size > toc_page {
        assert_ne!(next, FORMAT16_SENTINEL);
    }
}

#[test]
fn checked_in_corpus_has_exact_hashes_coordinates_and_object_graph() {
    let manifest = manifest();
    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["fixture_encoding"], "base64-rfc4648");

    let fixtures = manifest["fixtures"].as_array().unwrap();
    assert_eq!(fixtures.len(), 2);
    for fixture in fixtures {
        assert_eq!(fixture["artifact_kind"], "configuration");
        assert_eq!(fixture["provenance"]["origin"], "hand-authored-clean-room");
        assert_eq!(fixture["provenance"]["contains_application_code"], false);
        assert_eq!(
            fixture["provenance"]["runtime_dependencies"],
            Value::Array(vec![])
        );

        let encoded =
            fs::read_to_string(corpus_root().join(fixture["path"].as_str().unwrap())).unwrap();
        let bytes = decode_base64(&encoded).unwrap();
        assert_eq!(
            bytes.len() as u64,
            fixture["decoded_size"].as_u64().unwrap()
        );
        assert_eq!(
            sha256_hex(&bytes),
            fixture["decoded_sha256"].as_str().unwrap()
        );

        let coordinates = &fixture["coordinates"];
        let count = coordinates["declared_element_count"].as_u64().unwrap() as u32;
        let base = coordinates["container_base_offset"].as_u64().unwrap() as usize;
        match coordinates["container_revision"].as_str().unwrap() {
            "Format15" => {
                assert_eq!(base, 0);
                assert_format15(&bytes, base, count, true);
            }
            "Format16" => {
                assert_eq!(base, FORMAT16_BASE_OFFSET);
                assert_format15(&bytes[..base], 0, 5, true);
                assert_format16(&bytes, base, count, true);
                assert_eq!(
                    sha256_hex(&bytes[..base]),
                    coordinates["preamble"]["sha256"].as_str().unwrap()
                );
                assert_eq!(
                    sha256_hex(&bytes[base..]),
                    coordinates["primary_container"]["sha256"].as_str().unwrap()
                );
            }
            revision => panic!("unsupported fixture revision {revision}"),
        }
        assert_ordered_names(
            &bytes[base..],
            fixture["expected"]["ordered_entries"].as_array().unwrap(),
        );
    }
}

#[test]
fn optional_external_corpus_is_hash_pinned_and_never_downloaded() {
    let Some(root) = env::var_os("IBCMD_CF_EXTERNAL_CORPUS") else {
        return;
    };
    let root = PathBuf::from(root);
    let manifest = manifest();
    assert_eq!(manifest["external_corpus"]["network_access"], false);

    for artifact in manifest["external_corpus"]["artifacts"].as_array().unwrap() {
        assert_eq!(artifact["checked_in"], false);
        assert_eq!(artifact["contains_application_code"], true);
        let path = root.join(artifact["local_name"].as_str().unwrap());
        let bytes = fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "failed to read external corpus file {}: {error}",
                path.display()
            )
        });
        assert_eq!(
            bytes.len() as u64,
            artifact["decoded_size"].as_u64().unwrap()
        );
        assert_eq!(sha256_hex(&bytes), artifact["sha256"].as_str().unwrap());

        let count = artifact["declared_element_count"].as_u64().unwrap() as u32;
        let base = artifact["container_base_offset"].as_u64().unwrap() as usize;
        match artifact["container_revision"].as_str().unwrap() {
            "Format15" => assert_format15(&bytes, base, count, false),
            "Format16" => assert_format16(&bytes, base, count, false),
            revision => panic!("unsupported external revision {revision}"),
        }
        assert_ordered_names(
            &bytes[base..],
            artifact["required_entries"].as_array().unwrap(),
        );
    }
}
