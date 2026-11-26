//! Opt-in lab census, not whole configuration or independent SDK acceptance.
//! Every discovered EDT managed form is read by the production pipeline, written
//! to disposable native files, then read again including owned sidecars/modules.
//! Reports contain source-path and error hashes, never handlers or module text.
use formats_xml::form::{bind_picture_semantics, resolve_common_picture_transparency};
use morph1c_core::ir::MetadataObject;
use morph1c_core::version::{FormatVersion, with_roundtrip_target, with_source_version};
use morph1c_pipeline::{Format, FormatRegistry, attach_form_body, layout, write_form_bodies};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn regular(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err("reparse source".into());
        }
    }
    if metadata.file_type().is_symlink() {
        return Err("linked source".into());
    }
    Ok(metadata)
}
fn file_sha(path: &Path) -> Result<(String, u64), String> {
    if !regular(path)?.is_file() {
        return Err("expected regular file".into());
    }
    let mut input = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let mut digest = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
        count = count.checked_add(n as u64).ok_or("byte count overflow")?;
    }
    Ok((format!("{:x}", digest.finalize()), count))
}
fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut result = Vec::new();
    let mut identities = BTreeSet::new();
    while let Some(directory) = pending.pop() {
        if !regular(&directory)?.is_dir() {
            return Err("expected regular directory".into());
        }
        for entry in fs::read_dir(&directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("non UTF-8 source path")?
                .replace('\\', "/");
            ibcmd_xml::source_tree::validate_source_path_safety(&relative)
                .map_err(|e| e.to_string())?;
            if !identities.insert(relative.to_lowercase()) {
                return Err("case-colliding source".into());
            }
            let metadata = regular(&path)?;
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                result.push(path);
            } else {
                return Err("special source entry".into());
            }
        }
    }
    result.sort();
    Ok(result)
}
fn snapshot(root: &Path) -> Result<Vec<Value>, String> {
    files(root)?
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .ok_or("non UTF-8 path")?
                .replace('\\', "/");
            let (digest, bytes) = file_sha(&path)?;
            Ok(json!({"path_sha256":sha(relative.as_bytes()),"sha256":digest,"bytes":bytes}))
        })
        .collect()
}
fn semantic_sha(value: &impl Serialize) -> Result<String, String> {
    struct Hash(Sha256);
    impl Write for Hash {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Hash(Sha256::new());
    serde_json::to_writer(&mut output, value).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", output.0.finalize()))
}
fn append(output: &mut File, row: &Value) {
    serde_json::to_writer(&mut *output, row).unwrap();
    output.write_all(b"\n").unwrap();
    output.flush().unwrap();
}
fn fresh_json(path: &Path, row: &Value) {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap();
    serde_json::to_writer_pretty(&mut output, row).unwrap();
    output.write_all(b"\n").unwrap();
}
fn picture_context(
    root: &Path,
    version: FormatVersion,
) -> Result<(BTreeMap<String, bool>, Vec<Value>), String> {
    use morph1c_core::{ir::PropertyValue, spec::metadata::common_picture::F_TRANSPARENT_PIXEL};
    let directory = root.join("CommonPictures");
    if !directory.exists() {
        return Ok((BTreeMap::new(), Vec::new()));
    }
    let mut result = BTreeMap::new();
    let mut bindings = Vec::new();
    let registry = FormatRegistry::for_format(Format::Edt).map_err(|e| e.to_string())?;
    let reader = registry
        .get("CommonPicture")
        .ok_or("no CommonPicture reader")?
        .read;
    for path in files(&directory)? {
        if path.extension().and_then(|s| s.to_str()) != Some("mdo") {
            continue;
        }
        let raw = fs::read(&path).map_err(|e| e.to_string())?;
        let object = with_source_version(Some(version), || reader(&raw))?;
        let pixel_present = match object.get(F_TRANSPARENT_PIXEL) {
            None => false,
            Some(PropertyValue::List(values)) => !values.is_empty(),
            Some(_) => return Err("invalid typed CommonPicture transparent pixel".into()),
        };
        if result
            .insert(format!("CommonPicture.{}", object.name), pixel_present)
            .is_some()
        {
            return Err("duplicate CommonPicture descriptor identity".into());
        }
        bindings.push(json!({"path_sha256":sha(path.strip_prefix(root).unwrap().to_str().unwrap().as_bytes()),"sha256":sha(&raw),"bytes":raw.len()}));
    }
    Ok((result, bindings))
}

// Use the production descriptor reader to bind actual declaration UUID and Help
// fields. Only this form is retained for sidecar attachment; other owner fields
// are outside this form-only census. A fake CommonForm/UUID is never substituted.
fn declared_owner(
    registry: &FormatRegistry,
    root: &Path,
    form: &Path,
    version: FormatVersion,
) -> Result<(MetadataObject, PathBuf), String> {
    let form_dir = form.parent().ok_or("no form directory")?;
    let own = form_dir
        .parent()
        .is_some_and(|p| p.file_name().is_some_and(|n| n == "CommonForms"));
    let owner_dir = if own {
        form_dir
    } else {
        let forms = form_dir.parent().ok_or("no Forms directory")?;
        if forms.file_name().is_none_or(|name| name != "Forms") {
            return Err("unclaimed Form.form layout".into());
        }
        forms.parent().ok_or("no metadata owner")?
    };
    let name = owner_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("no owner name")?;
    let anchor = owner_dir.join(format!("{name}.mdo"));
    let prefix = owner_dir
        .parent()
        .ok_or("no kind directory")?
        .strip_prefix(root)
        .map_err(|e| e.to_string())?;
    let matches: Vec<_> = registry
        .iter()
        .filter(|kind| {
            let directory: PathBuf = layout::kind_rel_dir(registry, kind).into_iter().collect();
            directory == prefix
        })
        .collect();
    if matches.len() != 1 {
        return Err("no unique registered owner descriptor".into());
    }
    let raw = fs::read(&anchor).map_err(|e| e.to_string())?;
    let mut object = with_source_version(Some(version), || (matches[0].read)(&raw))?;
    if object.name != name {
        return Err("owner descriptor/path name mismatch".into());
    }
    if own {
        if object.kind.as_str() != "CommonForm" {
            return Err("wrong CommonForm owner".into());
        }
    } else {
        let form_name = form_dir
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("no form name")?;
        object
            .children
            .retain(|child| child.kind.as_str().ends_with(".FormRef") && child.name == form_name);
        if object.children.len() != 1 {
            return Err("form does not have one exact declared metadata identity".into());
        }
    }
    Ok((object, anchor))
}
fn bind_form(
    object: &mut MetadataObject,
    pictures: &BTreeMap<String, bool>,
    edt: bool,
) -> Result<(), String> {
    if object.form_bodies.len() != 1 {
        return Err("did not attach exactly one managed form".into());
    }
    let uuid = if object.kind.as_str() == "CommonForm" {
        object.uuid
    } else {
        object.children[0].uuid
    };
    resolve_common_picture_transparency(&mut object.form_bodies[0].body, pictures, edt)
        .map_err(|e| e.to_string())?;
    bind_picture_semantics(&mut object.form_bodies[0].body, uuid, edt).map_err(|e| e.to_string())
}

#[test]
fn fingerprint_preserves_handler_values_order_presence_and_assets() {
    let base = json!({"handlers":["first","second"],"presence":null,"asset":[0,255]});
    for changed in [
        json!({"handlers":["second","first"],"presence":null,"asset":[0,255]}),
        json!({"handlers":["first","edited"],"presence":null,"asset":[0,255]}),
        json!({"handlers":["first","second"],"presence":false,"asset":[0,255]}),
        json!({"handlers":["first","second"],"presence":null,"asset":[0,254]}),
    ] {
        assert_ne!(
            semantic_sha(&base).unwrap(),
            semantic_sha(&changed).unwrap()
        );
    }
}
#[test]
fn snapshots_include_all_files_and_detect_added_or_changed_sidecars() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("Items")).unwrap();
    fs::write(temp.path().join("Form.form"), b"body").unwrap();
    fs::write(temp.path().join("Items/Picture.png"), [0, 1, 255]).unwrap();
    let before = snapshot(temp.path()).unwrap();
    assert_eq!(before.len(), 2);
    fs::write(temp.path().join("Items/Picture.png"), [0, 2, 255]).unwrap();
    assert_ne!(before, snapshot(temp.path()).unwrap());
    fs::write(temp.path().join("Module.bsl"), b"module").unwrap();
    assert_eq!(snapshot(temp.path()).unwrap().len(), 3);
}

#[test]
#[ignore = "requires both complete genuine UH EDT corpora; shared heavy FIFO, one test thread"]
fn all_uha83_and_uha85_forms_pipeline_native_semantic_preflight() {
    let started = Instant::now();
    let run = PathBuf::from(std::env::var_os("IBCMD_FORM_PREFLIGHT_RUN").unwrap());
    assert!(
        run.starts_with(Path::new("F:/ibcmd/lab/07")),
        "all disposable outputs must be in F lab"
    );
    fs::create_dir(&run).unwrap();
    let executable = std::env::current_exe().unwrap();
    fresh_json(
        &run.join("invocation.json"),
        &json!({"scope":"complete managed form bodies, owned sidecars and modules; production semantic serialization; metadata-owner fields and CommonForm owner Help excluded; not independent SDK byte acceptance",
        "source_commit":std::env::var("IBCMD_FORM_PREFLIGHT_COMMIT").unwrap(),
        "test_source_sha256":std::env::var("IBCMD_FORM_PREFLIGHT_SOURCE_SHA").unwrap(),
        "test_executable":executable,"test_executable_binding":file_sha(&executable).unwrap(),
        "expected_corpora":2,"expected_forms_each":13044,"extra_semantic_normalization":false}),
    );
    let mut journal = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(run.join("rows.jsonl"))
        .unwrap();
    let registry = FormatRegistry::for_format(Format::Edt).unwrap();
    let mut processed = 0;
    let mut failures = 0;
    let mut corpus_results = Vec::new();
    for (profile, minor) in [("83", 20), ("85", 21)] {
        let root =
            PathBuf::from(std::env::var_os(format!("IBCMD_FORM_PREFLIGHT_UH{profile}")).unwrap());
        let version = FormatVersion::new(2, minor);
        let all: Vec<_> = files(&root)
            .unwrap()
            .into_iter()
            .filter(|p| p.file_name().is_some_and(|n| n == "Form.form"))
            .collect();
        let (pictures, picture_before) = picture_context(&root, version).unwrap();
        fresh_json(
            &run.join(format!("corpus-{profile}-before.json")),
            &json!({"source":root,"profile":format!("2.{minor}"),"form_count":all.len(),"picture_descriptor_bindings":picture_before}),
        );
        for (index, form) in all.iter().enumerate() {
            let relative = form
                .strip_prefix(&root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            let case_started = Instant::now();
            let mut phase = "source-snapshot";
            let mut row =
                json!({"corpus":profile,"index":index,"path_sha256":sha(relative.as_bytes())});
            let before = snapshot(form.parent().unwrap());
            let mut descriptor_before = None;
            let result = (|| -> Result<(), String> {
                let source_files = before.as_ref().map_err(Clone::clone)?;
                row["input_files"] = json!(source_files);
                phase = "source-descriptor";
                let (mut source, anchor) = declared_owner(&registry, &root, form, version)?;
                descriptor_before = Some((anchor.clone(), file_sha(&anchor)?));
                row["descriptor_path_sha256"] = json!(sha(anchor
                    .strip_prefix(&root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .as_bytes()));
                row["descriptor_binding"] = json!(descriptor_before.as_ref().unwrap().1);
                phase = "source-form-read";
                // Keep the current owner and selected FormRef declarations intact.
                let owner_kind = source.kind.as_str().to_owned();
                with_source_version(Some(version), || {
                    attach_form_body(Format::Edt, &owner_kind, &anchor, &mut source)
                })
                .map_err(|e| e.to_string())?;
                phase = "source-picture-binding";
                bind_form(&mut source, &pictures, true)?;
                let source_sha = semantic_sha(&source.form_bodies)?;
                row["source_semantic_sha256"] = json!(source_sha);
                let scratch = tempfile::tempdir_in(&run).map_err(|e| e.to_string())?;
                let output = scratch.path().join(format!("{}.xml", source.name));
                phase = "native-write";
                with_roundtrip_target(version, || {
                    write_form_bodies(Format::Designer, &output, &source)
                })
                .map_err(|e| e.to_string())?;
                row["native_output_files"] = json!(snapshot(scratch.path())?);
                let mut returned = source;
                returned.form_bodies.clear();
                phase = "native-read";
                with_source_version(Some(version), || {
                    attach_form_body(Format::Designer, &owner_kind, &output, &mut returned)
                })
                .map_err(|e| e.to_string())?;
                phase = "native-picture-binding";
                bind_form(&mut returned, &pictures, false)?;
                let returned_sha = semantic_sha(&returned.form_bodies)?;
                row["returned_semantic_sha256"] = json!(returned_sha);
                phase = "semantic-comparison";
                if source_sha != returned_sha {
                    return Err("complete production form semantic fingerprints differ".into());
                }
                Ok(())
            })();
            let after = snapshot(form.parent().unwrap());
            let input_unchanged = before
                .as_ref()
                .ok()
                .zip(after.as_ref().ok())
                .is_some_and(|(a, b)| a == b);
            let descriptor_unchanged = descriptor_before
                .as_ref()
                .map(|(path, binding)| file_sha(path).is_ok_and(|current| current == *binding));
            row["input_after"] = json!(after.as_ref().ok());
            row["input_unchanged"] = json!(input_unchanged);
            row["descriptor_unchanged"] = json!(descriptor_unchanged);
            row["seconds"] = json!(case_started.elapsed().as_secs_f64());
            let error = result.err();
            let failed = error.is_some() || !input_unchanged || descriptor_unchanged == Some(false);
            if failed {
                failures += 1;
            }
            row["status"] = json!(if failed {
                "FAIL"
            } else {
                "FORM_SEMANTIC_PASS_NOT_SDK_ACCEPTANCE"
            });
            row["phase"] = json!(phase);
            row["error_sha256"] = json!(error.as_ref().map(|e| sha(e.as_bytes())));
            row["error_bytes"] = json!(error.as_ref().map(String::len));
            append(&mut journal, &row);
            processed += 1;
            if index % 100 == 0 {
                fresh_json(
                    &run.join(format!("progress-{processed:06}.json")),
                    &json!({"processed":processed,"failures":failures,"seconds":started.elapsed().as_secs_f64(),"complete":false}),
                );
            }
        }
        let discovered_after: Vec<_> = files(&root)
            .unwrap()
            .into_iter()
            .filter(|p| p.file_name().is_some_and(|n| n == "Form.form"))
            .collect();
        let (_, picture_after) = picture_context(&root, version).unwrap();
        corpus_results.push(json!({"corpus":profile,"expected":13044,"discovered":all.len(),"discovered_after":discovered_after.len(),"enumeration_unchanged":all==discovered_after,"picture_context_unchanged":picture_before==picture_after}));
    }
    drop(journal);
    let complete = processed == 26088
        && corpus_results.iter().all(|r| {
            r["discovered"] == 13044
                && r["enumeration_unchanged"] == true
                && r["picture_context_unchanged"] == true
        });
    fresh_json(
        &run.join("result.json"),
        &json!({"status":if complete && failures==0 {"FORM_PREFLIGHT_PASS_NOT_WHOLE_ACCEPTANCE"} else {"FORM_PREFLIGHT_FAIL_OR_INCOMPLETE"},"complete":complete,"processed":processed,"failures":failures,"corpora":corpus_results,"rows_binding":file_sha(&run.join("rows.jsonl")).unwrap(),"seconds":started.elapsed().as_secs_f64()}),
    );
    println!("complete={complete} processed={processed} failures={failures}");
    assert!(
        complete && failures == 0,
        "all processed rows and failures retained in lab report"
    );
}
