//! Opt-in Role-sidecar preflight. This is not whole configuration or SDK acceptance.
//! Every rights source is immutable; journals contain hashes and counts only.
use formats_xml::{registry::SidecarFormat, rights};
use morph1c_core::spec::metadata::role::{Right, RightRestriction, RightsObject, RightsTable};
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn regular(path: &Path) -> Result<fs::Metadata, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return Err("reparse source".into());
        }
    }
    if meta.file_type().is_symlink() {
        return Err("linked source".into());
    }
    Ok(meta)
}
fn no_link_ancestry(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("absolute path required".into());
    }
    for ancestor in path.ancestors() {
        if !regular(ancestor)?.is_dir() {
            return Err("directory ancestry required".into());
        }
    }
    Ok(())
}
fn file_hash(path: &Path) -> Result<(String, u64), String> {
    if !regular(path)?.is_file() {
        return Err("regular file required".into());
    }
    let mut input = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let mut digest = Sha256::new();
    let mut len = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
        len = len.checked_add(n as u64).ok_or("length overflow")?;
    }
    Ok((format!("{:x}", digest.finalize()), len))
}
fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    no_link_ancestry(root)?;
    let mut stack = vec![root.to_owned()];
    let mut out = Vec::new();
    let mut names = BTreeSet::new();
    while let Some(dir) = stack.pop() {
        if !regular(&dir)?.is_dir() {
            return Err("source directory required".into());
        }
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("non-UTF8 path")?
                .replace('\\', "/");
            ibcmd_xml::source_tree::validate_source_path_safety(&relative)
                .map_err(|e| e.to_string())?;
            if !names.insert(relative.to_lowercase()) {
                return Err("case-colliding source path".into());
            }
            let meta = regular(&path)?;
            if meta.is_dir() {
                stack.push(path);
            } else if meta.is_file() {
                out.push(path);
            } else {
                return Err("special source entry".into());
            }
        }
    }
    out.sort();
    Ok(out)
}
fn is_rights(path: &Path, fmt: SidecarFormat) -> bool {
    match fmt {
        SidecarFormat::EdtRights => path.file_name().is_some_and(|n| n == "Rights.rights"),
        SidecarFormat::DesignerRights => {
            path.file_name().is_some_and(|n| n == "Rights.xml")
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|n| n == "Ext")
        }
        _ => false,
    }
}
fn roster(root: &Path) -> Result<Vec<Value>, String> {
    files(root)?
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("non-UTF8 path")?
                .replace('\\', "/");
            let (digest, len) = file_hash(&path)?;
            Ok(json!({"path_sha256":sha(relative.as_bytes()),"sha256":digest,"length":len}))
        })
        .collect()
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, value).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())
}
fn read(bytes: &[u8], fmt: SidecarFormat, version: FormatVersion) -> Result<RightsTable, String> {
    with_source_version(Some(version), || rights::read(bytes, fmt)).map_err(|e| e.to_string())
}
fn write(
    table: &RightsTable,
    fmt: SidecarFormat,
    version: FormatVersion,
) -> Result<Vec<u8>, String> {
    with_roundtrip_target(version, || rights::write(table, fmt)).map_err(|e| e.to_string())
}
fn opposite(fmt: SidecarFormat) -> SidecarFormat {
    if fmt == SidecarFormat::EdtRights {
        SidecarFormat::DesignerRights
    } else {
        SidecarFormat::EdtRights
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    lanes: Vec<Lane>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lane {
    id: String,
    root: PathBuf,
    format: String,
    source_minor: u16,
    expected_rights: usize,
}
#[derive(Serialize)]
struct Failure {
    phase: String,
    error_sha256: String,
    error_length: usize,
}
#[derive(Serialize)]
struct Row {
    lane: String,
    path_sha256: String,
    source_sha256: String,
    source_length: u64,
    checks: Vec<Value>,
    failures: Vec<Failure>,
}
fn observed<T>(
    path: &Path,
    expected: &(String, u64),
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if file_hash(path)? != *expected {
        return Err("source changed before operation".into());
    }
    let result = operation();
    if file_hash(path)? != *expected {
        return Err("source changed after operation".into());
    }
    result
}
fn check(row: &mut Row, phase: &str, operation: impl FnOnce() -> Result<Value, String>) {
    match operation() {
        Ok(value) => row.checks.push(json!({"phase":phase,"result":value})),
        Err(error) => row.failures.push(Failure {
            phase: phase.to_owned(),
            error_sha256: sha(error.as_bytes()),
            error_length: error.len(),
        }),
    }
}
fn mutated(table: &RightsTable) -> RightsTable {
    let mut changed = table.clone();
    changed.set_for_new_objects = !changed.set_for_new_objects;
    changed.set_for_attributes_by_default = !changed.set_for_attributes_by_default;
    changed.independent_rights_of_child_objects = !changed.independent_rights_of_child_objects;
    changed.objects.reverse();
    changed.restriction_templates.reverse();
    for object in &mut changed.objects {
        object.rights.reverse();
        for right in &mut object.rights {
            right.value = !right.value;
            right.restrictions.reverse();
            for restriction in &mut right.restrictions {
                restriction.condition.push_str("\r\n1 = 1");
            }
        }
    }
    changed
}
fn preflight(lane: &Lane, fmt: SidecarFormat, path: &Path) -> Result<Row, String> {
    let version = FormatVersion::new(2, lane.source_minor);
    let expected = file_hash(path)?;
    let relative = path
        .strip_prefix(&lane.root)
        .map_err(|e| e.to_string())?
        .to_str()
        .ok_or("non-UTF8 path")?
        .replace('\\', "/");
    let mut row = Row {
        lane: lane.id.clone(),
        path_sha256: sha(relative.as_bytes()),
        source_sha256: expected.0.clone(),
        source_length: expected.1,
        checks: Vec::new(),
        failures: Vec::new(),
    };
    let mut table = None;
    let mut original = Vec::new();
    check(&mut row, "source-read", || {
        observed(path, &expected, || {
            original = fs::read(path).map_err(|e| e.to_string())?;
            if (sha(&original), original.len() as u64) != expected {
                return Err("read byte identity mismatch".into());
            }
            let value = read(&original, fmt, version)?;
            let restrictions: usize = value
                .objects
                .iter()
                .flat_map(|o| &o.rights)
                .map(|r| r.restrictions.len())
                .sum();
            let multi: usize = value
                .objects
                .iter()
                .flat_map(|o| &o.rights)
                .filter(|r| r.restrictions.len() > 1)
                .count();
            let counts = json!({"objects":value.objects.len(),"rights":value.objects.iter().map(|o|o.rights.len()).sum::<usize>(),
            "restrictions":restrictions,"multi_restriction_rights":multi,"templates":value.restriction_templates.len()});
            table = Some(value);
            Ok(counts)
        })
    });
    if let Some(table) = table {
        check(&mut row, "own-dialect-byte-exact", || {
            observed(path, &expected, || {
                let bytes = write(&table, fmt, version)?;
                if bytes != original {
                    return Err("own-dialect byte mismatch".into());
                }
                Ok(json!({"output_sha256":sha(&bytes),"length":bytes.len()}))
            })
        });
        for minor in [20, 21] {
            let target = FormatVersion::new(2, minor);
            check(&mut row, &format!("cross-canonical-2.{minor}"), || {
                observed(path, &expected, || {
                    let bytes = write(&table, opposite(fmt), target)?;
                    let returned = read(&bytes, opposite(fmt), target)?;
                    if returned != table {
                        return Err("cross-dialect canonical mismatch".into());
                    }
                    let raw_return = write(&returned, fmt, version)?;
                    // Source-only framing is not transported by the opposite XML dialect.
                    // Record raw return honestly; public provenance is a separate product gate.
                    Ok(json!({"output_sha256":sha(&bytes),"length":bytes.len(),
                        "canonical_equal":true,"raw_return_byte_exact":raw_return==original,
                        "raw_return_sha256":sha(&raw_return),"raw_return_length":raw_return.len()}))
                })
            });
            check(
                &mut row,
                &format!("current-values-and-order-2.{minor}"),
                || {
                    observed(path, &expected, || {
                        let current = mutated(&table);
                        if current == table {
                            return Err("mutation did not change current state".into());
                        }
                        for output_format in [fmt, opposite(fmt)] {
                            let bytes = write(&current, output_format, target)?;
                            if read(&bytes, output_format, target)? != current {
                                return Err("current values/order lost".into());
                            }
                        }
                        Ok(json!({"all_current_values_equal":true}))
                    })
                },
            );
        }
    }
    Ok(row)
}

fn synthetic() -> RightsTable {
    RightsTable {
        set_for_new_objects: false,
        set_for_attributes_by_default: true,
        independent_rights_of_child_objects: false,
        objects: vec![RightsObject {
            name: "Catalog.Synthetic".into(),
            rights: vec![Right {
                name: "Read".into(),
                value: true,
                restrictions: vec![
                    RightRestriction {
                        field: None,
                        condition: "A &gt; 1\r\nB &lt; 4".into(),
                    },
                    RightRestriction {
                        field: Some("Second".into()),
                        condition: "C = 2".into(),
                    },
                    RightRestriction {
                        field: None,
                        condition: "C = 2".into(),
                    },
                ],
            }],
        }],
        restriction_templates: Vec::new(),
        source_layout: None,
    }
}
#[test]
fn current_restrictions_order_duplicates_conditions_and_flags_roundtrip() {
    let original = synthetic();
    let changed = mutated(&original);
    assert_ne!(original, changed);
    for minor in [20, 21] {
        for fmt in [SidecarFormat::EdtRights, SidecarFormat::DesignerRights] {
            for table in [&original, &changed] {
                let bytes = write(table, fmt, FormatVersion::new(2, minor)).unwrap();
                assert_eq!(
                    read(&bytes, fmt, FormatVersion::new(2, minor)).unwrap(),
                    *table
                );
            }
        }
    }
}
#[test]
fn roster_detects_content_addition_and_link_is_not_silently_skipped() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("Rights.rights"), b"first").unwrap();
    let before = roster(temp.path()).unwrap();
    fs::write(temp.path().join("Rights.rights"), b"edited").unwrap();
    assert_ne!(before, roster(temp.path()).unwrap());
    let changed = roster(temp.path()).unwrap();
    fs::write(temp.path().join("Extra.xml"), b"extra").unwrap();
    assert_ne!(changed, roster(temp.path()).unwrap());
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            temp.path().join("Rights.rights"),
            temp.path().join("Linked"),
        )
        .unwrap();
        assert!(roster(temp.path()).is_err());
    }
}
#[test]
#[ignore = "opt-in all discovered Role-sidecar lab preflight; requires reviewed manifest and FIFO launcher"]
fn genuine_all_role_rights_preflight() {
    let started = Instant::now();
    let manifest_path =
        PathBuf::from(std::env::var_os("IBCMD_RIGHTS_MANIFEST").expect("manifest required"));
    let run = PathBuf::from(std::env::var_os("IBCMD_RIGHTS_RUN").expect("fresh lab run required"));
    let lab = Path::new("F:/ibcmd/lab/07");
    assert!(run.is_absolute() && run.starts_with(lab));
    no_link_ancestry(&run).expect("safe run ancestry");
    assert!(
        fs::read_dir(&run).unwrap().next().is_none(),
        "test output must be fresh"
    );
    let manifest_bytes = fs::read(&manifest_path).unwrap();
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(
        manifest.lanes.len(),
        12,
        "all three lanes/four corpora required"
    );
    let mut ids = BTreeSet::new();
    let mut journal = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(run.join("rows.jsonl"))
        .unwrap();
    let mut processed = 0_usize;
    let mut failures = 0_usize;
    let mut lane_results = Vec::new();
    for lane in &manifest.lanes {
        assert!(ids.insert(&lane.id));
        assert!(matches!(lane.source_minor, 20 | 21));
        assert_eq!(lane.root.file_name().unwrap(), "Roles");
        let fmt = match lane.format.as_str() {
            "edt" => SidecarFormat::EdtRights,
            "designer" => SidecarFormat::DesignerRights,
            _ => panic!("unknown format"),
        };
        let before = roster(&lane.root).unwrap();
        write_json(&run.join(format!("{}-before.json", lane.id)), &before).unwrap();
        let sources: Vec<_> = files(&lane.root)
            .unwrap()
            .into_iter()
            .filter(|p| is_rights(p, fmt))
            .collect();
        assert_eq!(
            sources.len(),
            lane.expected_rights,
            "complete discovery must match manifest"
        );
        let mut lane_failed = 0;
        for path in &sources {
            let row = preflight(lane, fmt, path).expect("source I/O/security failure");
            if !row.failures.is_empty() {
                lane_failed += 1;
                failures += 1;
            }
            serde_json::to_writer(&mut journal, &row).unwrap();
            journal.write_all(b"\n").unwrap();
            journal.flush().unwrap();
            processed += 1;
        }
        let after = roster(&lane.root).unwrap();
        write_json(&run.join(format!("{}-after.json", lane.id)), &after).unwrap();
        let unchanged = before == after;
        if !unchanged {
            failures += 1;
        }
        lane_results.push(
            json!({"id":lane.id,"processed":sources.len(),"failed_files":lane_failed,
            "source_unchanged":unchanged,"roster_files":before.len()}),
        );
        write_json(
            &run.join(format!("{}-progress.json", lane.id)),
            &json!({"processed":processed,"failed_files":failures}),
        )
        .unwrap();
    }
    journal.flush().unwrap();
    let result = json!({"status":if failures==0 {"SCOPED_PREFLIGHT_PASS"}else{"SCOPED_PREFLIGHT_FAIL"},
        "complete":true,"processed":processed,"failed_files":failures,"lanes":lane_results,
        "manifest_sha256":sha(&manifest_bytes),"journal_sha256":file_hash(&run.join("rows.jsonl")).unwrap().0,
        "elapsed_seconds":started.elapsed().as_secs_f64(),"sdk_acceptance":false,"whole_configuration_acceptance":false});
    write_json(&run.join("result.json"), &result).unwrap();
    assert_eq!(
        failures, 0,
        "all failures retained in hash-only local journal"
    );
}
