//! Lab diagnostic only: production registry descriptor reads, without bodies or
//! a whole configuration claim. All candidates and failures remain in the report.
use formats_xml::registry::CorpusLayout;
use morph1c_pipeline::{Format, layout, registry::FormatRegistry};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf, time::Instant};

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
#[ignore = "requires genuine immutable corpus; run under the shared heavy FIFO"]
fn genuine_all_registry_metadata_descriptors_read() {
    let root = PathBuf::from(std::env::var_os("IBCMD_DESCRIPTOR_ROOT").unwrap());
    let output = PathBuf::from(std::env::var_os("IBCMD_DESCRIPTOR_REPORT").unwrap());
    let format = match std::env::var("IBCMD_DESCRIPTOR_FORMAT").unwrap().as_str() {
        "edt" => Format::Edt,
        "designer" => Format::Designer,
        value => panic!("unsupported diagnostic format: {value}"),
    };
    assert!(root.is_dir());
    assert!(!output.exists(), "never overwrite a previous capture");
    let started = Instant::now();
    let registry = FormatRegistry::for_format(format).unwrap();
    let mut rows = Vec::<Value>::new();
    let mut enumeration_errors = Vec::new();
    let mut excluded_non_descriptor_rows = Vec::new();
    let mut counts = BTreeMap::<String, usize>::new();
    for kind in registry.iter() {
        if !matches!(
            kind.layout,
            CorpusLayout::DirPerObject { .. }
                | CorpusLayout::FilePerObject { .. }
                | CorpusLayout::Nested { .. }
                | CorpusLayout::SingletonFile { .. }
                | CorpusLayout::BinarySidecar { .. }
                | CorpusLayout::TextSidecar { .. }
        ) {
            excluded_non_descriptor_rows.push(kind.kind);
            continue;
        }
        let candidates = match layout::collect_objects_checked(&registry, kind, &root) {
            Ok(candidates) => candidates,
            Err(error) => {
                let error = error.to_string();
                enumeration_errors.push(json!({"kind":kind.kind,
                    "error_bytes":error.len(),"error_sha256":hash(error.as_bytes())}));
                continue;
            }
        };
        counts.insert(kind.kind.to_owned(), candidates.len());
        for (_, path) in candidates {
            let before = fs::read(&path).unwrap();
            let result = (kind.read)(&before);
            let after = fs::read(&path).unwrap();
            let error = result.as_ref().err();
            rows.push(
                json!({"path":path.strip_prefix(&root).unwrap().to_string_lossy(),
                "kind":kind.kind,"bytes":before.len(),"sha256":hash(&before),
                "input_unchanged":before==after,"read_ok":result.is_ok(),
                "error_bytes":error.map(String::len),
                "error_sha256":error.map(|e|hash(e.as_bytes()))}),
            );
            // The IR belongs to this one descriptor; no aggregate full-model retention.
            drop(result);
        }
    }
    assert!(!rows.is_empty(), "empty corpus cannot produce a pass");
    let failures = rows
        .iter()
        .filter(|row| row["read_ok"] != true || row["input_unchanged"] != true)
        .count();
    let status = if failures == 0 && enumeration_errors.is_empty() {
        "DESCRIPTOR_READ_CENSUS_PASS_NOT_WHOLE_ACCEPTANCE"
    } else {
        "DESCRIPTOR_READ_CENSUS_FAIL"
    };
    let report = json!({"status":status,"scope":"registered descriptor roots and inline metadata children only; external descriptors, modules and bodies require the full product gates",
        "root":root,"format":format.code(),"elapsed_seconds":started.elapsed().as_secs_f64(),
        "descriptor_count":rows.len(),"failed_count":failures,"counts":counts,
        "excluded_non_descriptor_registry_rows":excluded_non_descriptor_rows,
        "enumeration_errors":enumeration_errors,"rows":rows});
    use std::io::Write;
    let mut stream = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .unwrap();
    stream
        .write_all(serde_json::to_string_pretty(&report).unwrap().as_bytes())
        .unwrap();
    println!(
        "descriptor census: {} files, {} failures; report {}",
        rows.len(),
        failures,
        output.display()
    );
    assert_eq!(
        failures, 0,
        "all failures retained; inspect descriptor paths individually"
    );
    assert!(enumeration_errors.is_empty());
}
