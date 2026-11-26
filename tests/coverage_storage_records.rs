//! Stored records which an XML -> CF round trip cannot express separately.
//! The surrounding configuration is the public, platform-proven coverage set.
//! The inserted body is the ten-byte, data-free empty HTML record.
#[path = "common/coverage_paths.rs"]
mod coverage_paths;

use ibcmd_cf::payload::{PayloadEncoding, encode_payload};
use ibcmd_core::limits::ResourceLimits;
use ibcmd_v8::reader::StreamingReader;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::{
    fs,
    io::Cursor,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn decode(source: &[u8]) -> Vec<u8> {
    let (mut out, mut word, mut bits) = (Vec::new(), 0u32, 0);
    for byte in source {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' | b'\r' | b'\n' => continue,
            _ => panic!("bad base64"),
        };
        word = (word << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((word >> bits) as u8);
            word &= (1 << bits) - 1;
        }
    }
    out
}

#[test]
fn minimized_selected_rows_group_profile_keeps_its_known_failure() {
    // Legacy physical fragment: a named group of kind 10 has a four-member
    // bag. Modern XML cannot express that internal panel as a named group.
    // Retain a public native group's common fields and instantiate only this
    // data-free kind and bag; this is a parser probe, not a native-built CF.
    let root = coverage_paths::root();
    let set = root.join("coverage/8.5.1.1150/corpus-compatible-form-layouts");
    let metadata =
        fs::read_to_string(set.join("native/CommonForms/CovCompatibleTable.xml")).unwrap();
    let owner = metadata
        .split("<CommonForm uuid=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let cf = decode(&fs::read(set.join("configuration.cf.b64")).unwrap());
    let mut reader = StreamingReader::open_default(Cursor::new(cf)).unwrap();
    let names = reader
        .index()
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect::<Vec<_>>();
    let temp = std::env::temp_dir().join(format!(
        "coverage-panel-profile-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _scratch = crate::common::Scratch::adopt(&temp);
    fs::create_dir_all(temp.join("rows")).unwrap();
    let mut changed = None;
    for (index, name) in names.iter().enumerate() {
        let Some(mut bytes) = reader.read_entry_data(index).unwrap() else {
            continue;
        };
        if name == &format!("{owner}.0") {
            if let Ok(body) = ibcmd_rs::module_blob::parse_form_body_blob(&bytes) {
                for (start, _) in body.layout.match_indices("{22,") {
                    let end = tuple_end(&body.layout, start).unwrap();
                    let mut fields = tuple_fields(&body.layout[start..end]);
                    if fields.len() < 22
                        || !fields[1].contains("02023637-7868-4a5f-8576-835a76e0c9ba")
                    {
                        continue;
                    }
                    let offset = usize::from(fields[5].starts_with('{'));
                    if !matches!(fields.get(5 + offset).copied(), Some("0" | "5" | "9")) {
                        continue;
                    }
                    if fields.len() <= 20 + offset {
                        continue;
                    }
                    {
                        assert!(body.module_text.is_empty());
                        fields[5 + offset] = "10";
                        fields[20 + offset] = "{0,1,0,1}";
                        let mut layout = body.layout.clone();
                        layout.replace_range(start..end, &format!("{{{}}}", fields.join(",")));
                        let plain = format!(
                            "\u{feff}{{{},{},\"\",{}}}",
                            body.revision.declared_marker(),
                            layout,
                            body.trailing.join(",")
                        );
                        bytes = encode_payload(
                            PayloadEncoding::RawDeflate,
                            plain.as_bytes(),
                            ResourceLimits::default(),
                        )
                        .unwrap();
                        ibcmd_rs::module_blob::parse_form_body_blob(&bytes)
                            .expect("the enclosing form container must remain valid");
                        changed = Some(name.clone());
                        break;
                    }
                }
            }
        }
        assert!(!name.contains(['/', '\\']));
        fs::write(temp.join("rows").join(format!("{name}__part0.bin")), bytes).unwrap();
    }
    assert!(
        changed.is_some(),
        "the public native form must carry a named group record"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["mssql-dump-config", "--rows-dir"])
        .arg(temp.join("rows"))
        .args([
            "--extract-metadata-xml",
            "--no-binary-rows",
            "--platform",
            "8.5.1",
            "--output-dir",
        ])
        .arg(temp.join("out"))
        .env("PATH", "")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let xml = fs::read_to_string(temp.join("out/CommonForms/CovCompatibleTable/Ext/Form.xml"))
            .unwrap();
        assert!(
            xml.contains("<Table "),
            "fixed profile must retain the surrounding form"
        );
        println!("coverage selected-rows identity profile: newly exported");
    } else {
        assert!(
            stderr.contains("revision 22, kind 10") && stderr.contains("4 members"),
            "unexpected profile failure: {stderr}"
        );
        println!(
            "coverage selected-rows identity profile: known revision 22, kind 10, four-member bag gap"
        );
    }
}

// The probe edits whole tuple fields, so changing the bag cannot leave stale
// string offsets or accidentally edit a quoted value in the surrounding form.
fn tuple_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let (mut depth, mut quoted, mut i) = (0usize, false, start);
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            if quoted && bytes.get(i + 1) == Some(&b'"') {
                i += 2;
                continue;
            }
            quoted = !quoted;
        } else if !quoted {
            if b == b'{' {
                depth += 1;
            } else if b == b'}' {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
        }
        i += 1;
    }
    None
}

fn tuple_fields(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    assert_eq!(bytes[0], b'{');
    assert_eq!(bytes[bytes.len() - 1], b'}');
    let (mut depth, mut quoted, mut i, mut start) = (0usize, false, 1usize, 1usize);
    let mut fields = Vec::new();
    while i < bytes.len() - 1 {
        let b = bytes[i];
        if b == b'"' {
            if quoted && bytes.get(i + 1) == Some(&b'"') {
                i += 2;
                continue;
            }
            quoted = !quoted;
        } else if !quoted {
            if b == b'{' {
                depth += 1;
            } else if b == b'}' {
                depth -= 1;
            } else if b == b',' && depth == 0 {
                fields.push(text[start..i].trim());
                start = i + 1;
            }
        }
        i += 1;
    }
    assert!(!quoted && depth == 0);
    fields.push(text[start..text.len() - 1].trim());
    fields
}

fn tree(root: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(
        root: &std::path::Path,
        current: &std::path::Path,
        out: &mut BTreeMap<String, Vec<u8>>,
    ) {
        if !current.is_dir() {
            return;
        }
        for entry in fs::read_dir(current).unwrap() {
            let p = entry.unwrap().path();
            if p.is_dir() {
                visit(root, &p, out);
            } else if p.file_name().unwrap() != "ConfigDumpInfo.xml" {
                out.insert(
                    p.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    fs::read(p).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}

#[test]
fn clean_room_corpus_stored_row_route_holds_its_baseline() {
    let root = coverage_paths::root().join("coverage");
    let mut sets = Vec::new();
    for platform in fs::read_dir(&root).unwrap() {
        let p = platform.unwrap().path();
        if !p.is_dir() {
            continue;
        }
        for entry in fs::read_dir(p).unwrap() {
            let p = entry.unwrap().path();
            if p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("corpus-")
                && p.join("manifest.json").is_file()
            {
                sets.push(p);
            }
        }
    }
    sets.sort();
    assert!(!sets.is_empty());
    let temp = std::env::temp_dir().join(format!(
        "coverage-stored-rows-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _scratch = crate::common::Scratch::adopt(&temp);
    fs::create_dir(&temp).unwrap();
    let mut reports = Vec::new();
    for (i, set) in sets.iter().enumerate() {
        let m: Value =
            serde_json::from_slice(&fs::read(set.join("manifest.json")).unwrap()).unwrap();
        let bytes = decode(&fs::read(set.join("configuration.cf.b64")).unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            m["configuration_cf"]["decoded_sha256"].as_str().unwrap()
        );
        let work = temp.join(i.to_string());
        fs::create_dir_all(work.join("rows")).unwrap();
        let mut reader = StreamingReader::open_default(Cursor::new(bytes)).unwrap();
        let names = reader
            .index()
            .entries
            .iter()
            .map(|e| e.name.clone())
            .collect::<Vec<_>>();
        for (index, name) in names.iter().enumerate() {
            assert!(!name.contains(['/', '\\']));
            if let Some(bytes) = reader.read_entry_data(index).unwrap() {
                fs::write(work.join("rows").join(format!("{name}__part0.bin")), bytes).unwrap();
            }
        }
        let platform = m["evidence"]["platform_version"].as_str().unwrap();
        // Completeness is measured independently against every native file.
        let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["mssql-dump-config", "--rows-dir"])
            .arg(work.join("rows"))
            .args([
                "--extract-metadata-xml",
                "--no-binary-rows",
                "--platform",
                platform,
                "--output-dir",
            ])
            .arg(work.join("out"))
            .env("PATH", "")
            .output()
            .unwrap();
        let expected = tree(&set.join("native"));
        let actual = tree(&work.join("out"));
        let actual = actual
            .into_iter()
            .filter(|(path, _)| path != "manifest.json")
            .collect::<BTreeMap<_, _>>();
        let paths = expected
            .keys()
            .chain(actual.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut equal = Vec::new();
        let mut differences = Vec::new();
        for path in paths {
            if expected.get(&path) == actual.get(&path) {
                equal.push(path);
            } else {
                let status = if !actual.contains_key(&path) {
                    "missing"
                } else if !expected.contains_key(&path) {
                    "extra"
                } else {
                    "different"
                };
                differences.push(serde_json::json!({"path":path,"status":status}));
            }
        }
        if let Ok(bytes) = fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/coverage")
                .join(set.strip_prefix(&root).unwrap())
                .join("storage-baseline.json"),
        ) {
            let baseline: Value = serde_json::from_slice(&bytes).unwrap();
            for path in baseline["equal"].as_array().unwrap() {
                let path = path.as_str().unwrap();
                assert!(
                    output.status.success() && equal.iter().any(|p| p == path),
                    "stored-row baseline regressed for {}: {path}",
                    m["set"]
                );
            }
        }
        println!(
            "coverage stored rows {}: exit {:?}, equal {}, differences {}",
            m["set"].as_str().unwrap(),
            output.status.code(),
            equal.len(),
            differences.len()
        );
        reports.push(serde_json::json!({"set":set.strip_prefix(&root).unwrap().to_string_lossy().replace('\\',"/"),"exit_code":output.status.code(),"equal":equal,"differences":differences,"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}));
    }
    if let Some(path) = std::env::var_os("IBCMD_COVERAGE_STORAGE_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&reports).unwrap()).unwrap();
    }
}

fn assert_native_interface_descriptor_parity(
    source: &std::path::Path,
    xml: &std::path::Path,
    target: &str,
    work: &std::path::Path,
) {
    use ibcmd_rs::metadata_model::{
        DescriptorContext, brace::parse_row, compile_descriptor, export::object_names,
    };
    let manifest: Value =
        serde_json::from_slice(&fs::read(source.join("manifest.json")).unwrap()).unwrap();
    let version = manifest["evidence"]["source_version"].as_str().unwrap();
    let context = DescriptorContext::new(xml, version).unwrap();
    let compiled = compile_descriptor(
        "Interface",
        &xml.join(target),
        &fs::read(xml.join(target)).unwrap(),
        &context,
    )
    .unwrap();
    let descriptor = parse_row(&compiled).unwrap();
    let names =
        object_names("Interface", &descriptor).expect("compiled native Interface has an identity");
    let cf = decode(&fs::read(source.join("configuration.cf.b64")).unwrap());
    let mut reader = StreamingReader::open_default(Cursor::new(cf)).unwrap();
    let position = reader
        .index()
        .entries
        .iter()
        .position(|entry| entry.name == names.uuid)
        .expect("native CF contains the Interface descriptor");
    let stored = reader.read_entry_data(position).unwrap().unwrap();
    let stored = ibcmd_cf::payload::decode_payload(
        PayloadEncoding::RawDeflate,
        &stored,
        ResourceLimits::default(),
    )
    .unwrap();
    assert_eq!(
        descriptor,
        parse_row(stored.bytes()).unwrap(),
        "the complete descriptor compiled from native XML must match the native CF record"
    );
    let packed = encode_payload(
        PayloadEncoding::RawDeflate,
        &compiled,
        ResourceLimits::default(),
    )
    .unwrap();
    let archive =
        ibcmd_v8::writer::write_format15_to_vec(&ibcmd_v8::writer::Format15Document::new(
            7,
            vec![ibcmd_v8::writer::Format15Element::named(
                &names.uuid,
                Some(packed),
            )],
        ))
        .unwrap();
    let cf = work.join("compiled-interface.cf");
    fs::write(&cf, archive).unwrap();
    let output = work.join("compiled-interface-xml");
    let result = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(cf)
        .arg(&output)
        .args(["--source-version", version, "--fail-on-opaque"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        tree(&output),
        BTreeMap::from([(
            target.to_owned(),
            fs::read(source.join("native").join(target)).unwrap()
        )]),
        "the compiled descriptor must export the complete native Interface XML byte-for-byte"
    );
}

#[test]
fn minimized_xml_probes_keep_their_precise_contracts() {
    // Current native normalizes three properties away and refuses two legacy
    // commands. Probe only these generated XML fragments in a proven native
    // context. They are deliberately not presented as CF or SQL parity.
    let fixtures = coverage_paths::root();
    let document: Value = serde_json::from_slice(
        &fs::read(fixtures.join("coverage-records/source-probes.json")).unwrap(),
    )
    .unwrap();
    let expectations: Value = serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/coverage-records/source-probe-expectations.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        document["cases"].as_array().unwrap().len(),
        expectations["cases"].as_object().unwrap().len()
    );
    let temp = std::env::temp_dir().join(format!(
        "coverage-source-probes-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _scratch = crate::common::Scratch::adopt(&temp);
    let mut reports = Vec::new();
    for (index, definition) in document["cases"].as_array().unwrap().iter().enumerate() {
        let mut case = definition.clone();
        let expected = expectations["cases"]
            .get(case["id"].as_str().unwrap())
            .expect("missing converter refusal expectation")
            .as_object()
            .unwrap()
            .clone();
        case.as_object_mut().unwrap().extend(expected);
        let set = case["set"].as_str().unwrap();
        let source = fixtures.join("coverage").join(set);
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(fs::read(source.join("manifest.json")).unwrap())
            ),
            case["baseline_manifest_sha256"].as_str().unwrap(),
            "{}: re-prove the native context before changing this source probe",
            case["id"]
        );
        let work = temp.join(index.to_string());
        let xml = work.join("xml");
        for (path, bytes) in tree(&source.join("native")) {
            let target = xml.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
        let target = xml.join(case["target"].as_str().unwrap());
        let mut text = fs::read_to_string(&target).unwrap();
        let operation = &case["operation"];
        match operation["kind"].as_str().unwrap() {
            "identity" => {}
            "subtree_from_input" => {
                let input =
                    fs::read_to_string(source.join(operation["input"].as_str().unwrap())).unwrap();
                let tag = operation["tag"].as_str().unwrap();
                let opening = format!("<{tag}");
                let closing = format!("</{tag}>");
                let start = input.find(&opening).unwrap();
                let end = start + input[start..].find(&closing).unwrap() + closing.len();
                let feature = &input[start..end];
                if let Some(at) = text.find(&opening) {
                    let until = at + text[at..].find(&closing).unwrap() + closing.len();
                    text.replace_range(at..until, feature);
                } else {
                    let at = text.rfind("</document>").unwrap();
                    text.insert_str(at, feature);
                }
            }
            "replace_once" | "insert_before" => {
                let anchor = operation["anchor"].as_str().unwrap();
                let fragment = operation["fragment"].as_str().unwrap();
                assert_eq!(
                    text.matches(anchor).count(),
                    1,
                    "{}: ambiguous patch anchor",
                    case["id"]
                );
                let at = text.find(anchor).unwrap();
                if operation["kind"] == "replace_once" {
                    text.replace_range(at..at + anchor.len(), fragment);
                } else {
                    text.insert_str(at, fragment);
                }
            }
            other => panic!("unknown source probe operation {other}"),
        }
        fs::write(target, text).unwrap();
        fs::create_dir_all(work.join("empty")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .arg("audit-empty-stage")
            .arg(&xml)
            .arg(work.join("empty"))
            .args([
                "--platform",
                case["reader_platform"]
                    .as_str()
                    .unwrap_or(case["native_platform"].as_str().unwrap()),
                "--max-samples",
                "18446744073709551615",
                "--output",
            ])
            .arg(work.join("audit.json"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: Value =
            serde_json::from_slice(&fs::read(work.join("audit.json")).unwrap()).unwrap();
        let rows: Vec<&Value> = report["samples"]
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.starts_with("failed |"))
            .flat_map(|(_, rows)| rows.as_array().unwrap())
            .filter(|row| row["source"] == case["failure_source"])
            .collect();
        match case["expected_parity"].as_str() {
            Some("native_interface_descriptor") => {
                assert_eq!(case["operation"]["kind"], "identity");
                assert!(
                    rows.is_empty(),
                    "{}: supported metadata refused: {rows:?}",
                    case["id"]
                );
                assert_native_interface_descriptor_parity(
                    &source,
                    &xml,
                    case["target"].as_str().unwrap(),
                    &work,
                );
            }
            None => {
                assert!(
                    rows.iter().any(|row| row["detail"]
                        .as_str()
                        .unwrap()
                        .contains(case["expected_failure"].as_str().unwrap())),
                    "{}: retain the precise known refusal; replace this assertion with equivalence when supported: {rows:?}",
                    case["id"]
                );
            }
            Some(other) => panic!("unsupported parity contract: {other}"),
        }
        reports.push(serde_json::json!({"case":case,"failures":rows}));
    }
    if let Some(path) = std::env::var_os("IBCMD_COVERAGE_SOURCE_REPORT") {
        fs::write(path, serde_json::to_vec_pretty(&reports).unwrap()).unwrap();
    }
}

#[test]
fn coverage_empty_html_stored_record_has_a_precise_known_gap() {
    let root = coverage_paths::root();
    let fixture = root.join("coverage/8.3.27.2342/commontemplates");
    let temp = std::env::temp_dir().join(format!(
        "coverage-empty-html-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _scratch = crate::common::Scratch::adopt(&temp);
    fs::create_dir_all(temp.join("rows")).unwrap();
    let cf = decode(&fs::read(fixture.join("configuration.cf.b64")).unwrap());
    let mut reader = StreamingReader::open_default(Cursor::new(cf)).unwrap();
    let names = reader
        .index()
        .entries
        .iter()
        .map(|e| e.name.clone())
        .collect::<Vec<_>>();
    for (i, name) in names.iter().enumerate() {
        assert!(!name.contains(['/', '\\']));
        if let Some(bytes) = reader.read_entry_data(i).unwrap() {
            fs::write(temp.join("rows").join(format!("{name}__part0.bin")), bytes).unwrap();
        }
    }
    let metadata =
        fs::read_to_string(fixture.join("native/CommonTemplates/CovTemplateType_HTMLDocument.xml"))
            .unwrap();
    let uuid = metadata
        .split("<CommonTemplate uuid=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let body = fs::read(coverage_paths::root().join("coverage-records/empty-html.txt")).unwrap();
    assert_eq!(body, b"\xef\xbb\xbf{5,0,0}");
    let encoded = encode_payload(
        PayloadEncoding::RawDeflate,
        &body,
        ResourceLimits::default(),
    )
    .unwrap();
    fs::write(
        temp.join("rows").join(format!("{uuid}.0__part0.bin")),
        encoded,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["mssql-dump-config", "--rows-dir"])
        .arg(temp.join("rows"))
        .args([
            "--extract-metadata-xml",
            "--no-binary-rows",
            "--platform",
            "8.3.27.2214",
            "--output-dir",
        ])
        .arg(temp.join("out"))
        .env("PATH", "")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        // A future fix is accepted only if the body remains an empty HTML template.
        let xml =
            fs::read_to_string(temp.join("out/CommonTemplates/CovTemplateType_HTMLDocument.xml"));
        assert!(
            xml.as_ref()
                .is_ok_and(|s| s.contains("<TemplateType>HTMLDocument</TemplateType>")),
            "fixed export must retain the descriptor: {stderr}"
        );
        println!("coverage stored-empty-html: newly equal");
    } else {
        assert!(
            stderr.contains("failed to extract help")
                && stderr.contains("CovTemplateType_HTMLDocument"),
            "unexpected refusal: {stderr}"
        );
        println!("coverage stored-empty-html: refused (known, not in parity baseline): {stderr}");
    }
    let manifest_path = temp.join("out/manifest.json");
    if let Ok(bytes) = fs::read(manifest_path) {
        let _: Value = serde_json::from_slice(&bytes).unwrap();
    }
}
