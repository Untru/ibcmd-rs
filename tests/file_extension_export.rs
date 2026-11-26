//! Native 8.3.27 clean-room file infobases: two independent extensions, one
//! adopting a common module. Expected hashes come from ibcmd, not our writer.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/file-infobase/extensions-8327")
}
fn setup(tag: &str, state: &str) -> Scratch {
    let root = std::env::temp_dir().join(format!("file-extension-{tag}-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(
        fs::File::open(fixture().join(format!("{state}.1CD.gz"))).unwrap(),
    )
    .read_to_end(&mut bytes)
    .unwrap();
    fs::write(root.join("1Cv8.1CD"), bytes).unwrap();
    Scratch(root)
}
fn export(root: &Path, name: &str, output: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["infobase", "config", "export", "--platform=8.3.27"])
        .arg(format!("--db-path={}", root.display()))
        .arg(format!("--extension={name}"))
        .arg(output)
        .output()
        .unwrap()
}
fn hashes(root: &Path) -> BTreeMap<String, String> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/"),
                format!("{:x}", Sha256::digest(fs::read(e.path()).unwrap())),
            )
        })
        .collect()
}
#[test]
fn named_extensions_match_native_in_active_and_prepared_states() {
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture().join("native-hashes.json")).unwrap()).unwrap();
    for state in ["staged", "applied", "mixed"] {
        let root = setup(&format!("native-{state}"), state);
        let before = fs::read(root.0.join("1Cv8.1CD")).unwrap();
        for (i, name) in ["ТестРасширение", "ДругоеРасширение"].iter().enumerate()
        {
            let output = root.0.join(format!("out-{i}"));
            let result = export(&root.0, name, &output);
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let actual = serde_json::to_value(hashes(&output)).unwrap();
            assert_eq!(actual, expected["cases"][state][name], "{state}/{name}");
            assert_eq!(fs::read(root.0.join("1Cv8.1CD")).unwrap(), before);
        }
    }
}
#[test]
fn missing_extension_does_not_publish_a_tree_or_change_the_database() {
    let root = setup("missing", "applied");
    let before = fs::read(root.0.join("1Cv8.1CD")).unwrap();
    let output = root.0.join("missing");
    let run = export(&root.0, "ОтсутствующееРасширение", &output);
    assert_eq!(
        run.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&run.stderr)
            .contains("расширение ОтсутствующееРасширение не найдено")
    );
    assert!(!output.exists());
    assert_eq!(fs::read(root.0.join("1Cv8.1CD")).unwrap(), before);
}

#[test]
fn native_name_case_and_trailing_spaces_select_the_same_extension() {
    let root = setup("name-spelling", "applied");
    let expected: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture().join("native-hashes.json")).unwrap()).unwrap();
    for (i, name) in ["тестрасширение", "ТестРасширение "].iter().enumerate()
    {
        let out = root.0.join(format!("spelling-{i}"));
        let run = export(&root.0, name, &out);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        assert_eq!(
            serde_json::to_value(hashes(&out)).unwrap(),
            expected["cases"]["applied"]["ТестРасширение"]
        );
    }
}

#[test]
fn corrupt_cas_and_unknown_registry_fail_before_publication() {
    for kind in [
        "missing-cas",
        "bad-digest",
        "registry-layout",
        "registry-envelope",
    ] {
        let root = setup(kind, "applied");
        let file = root.0.join("1Cv8.1CD");
        if kind == "missing-cas" || kind == "bad-digest" {
            let rows = ibcmd_filedb::FileDb::open(&root.0)
                .unwrap()
                .config_rows("CONFIGCAS")
                .unwrap();
            let mut bytes = fs::read(&file).unwrap();
            if kind == "missing-cas" {
                let needle: Vec<u8> = rows[0]
                    .file_name
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect();
                let at = bytes
                    .windows(needle.len())
                    .position(|v| v == needle)
                    .unwrap();
                bytes[at] = if bytes[at] == b'f' { b'e' } else { b'f' };
            } else {
                let needle = &rows[0].binary[..16];
                let at = bytes
                    .windows(needle.len())
                    .position(|v| v == needle)
                    .unwrap();
                bytes[at] ^= 1;
            }
            fs::write(&file, bytes).unwrap();
        } else {
            let mut bytes = fs::read(&file).unwrap();
            let needle: &[u8] = if kind == "registry-layout" {
                b"_EXTNAME"
            } else {
                &[0x43, 0xc2, 0x9a, 0x14]
            };
            let at = bytes
                .windows(needle.len())
                .position(|v| v == needle)
                .unwrap();
            bytes[at] ^= 1;
            fs::write(&file, bytes).unwrap();
        }
        let before = fs::read(&file).unwrap();
        let out = root.0.join("rejected");
        let result = export(&root.0, "ТестРасширение", &out);
        assert!(!result.status.success(), "{kind}");
        assert!(!out.exists(), "{kind}");
        assert_eq!(fs::read(&file).unwrap(), before, "{kind}");
        assert!(
            !fs::read_dir(&root.0).unwrap().map(Result::unwrap).any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with(".ibcmd-rs-extension-export-"))
        );
    }
}

#[test]
fn incomplete_staging_is_not_replaced_by_the_active_image() {
    let root = setup("incomplete", "staged");
    let rows = ibcmd_filedb::FileDb::open(&root.0)
        .unwrap()
        .config_rows("CONFIGCASSAVE")
        .unwrap();
    let row = rows
        .iter()
        .find(|r| r.file_name.ends_with("__configinfo"))
        .unwrap();
    let needle: Vec<u8> = row
        .file_name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let mut bytes = fs::read(root.0.join("1Cv8.1CD")).unwrap();
    let at = bytes
        .windows(needle.len())
        .position(|v| v == needle)
        .unwrap();
    // Preserve the physical record size while removing the required root name.
    bytes[at + needle.len() - 2] ^= 1;
    fs::write(root.0.join("1Cv8.1CD"), bytes).unwrap();
    let before = fs::read(root.0.join("1Cv8.1CD")).unwrap();
    let out = root.0.join("out");
    let run = export(&root.0, "ТестРасширение", &out);
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("configinfo"));
    assert!(!out.exists());
    assert_eq!(fs::read(root.0.join("1Cv8.1CD")).unwrap(), before);
}

#[test]
fn output_contents_and_incremental_refusals_are_preserved() {
    let root = setup("output", "applied");
    let out = root.0.join("out");
    fs::create_dir(&out).unwrap();
    fs::write(out.join("keep"), b"original").unwrap();
    let result = export(&root.0, "ТестРасширение", &out);
    assert!(!result.status.success());
    assert_eq!(fs::read(out.join("keep")).unwrap(), b"original");
    for option in ["--sync", "--base=missing.xml"] {
        let output = root.0.join(option.replace(['=', '/'], "_"));
        let result = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["infobase", "config", "export", "--platform=8.3.27", option])
            .arg(format!("--db-path={}", root.0.display()))
            .arg("--extension=ТестРасширение")
            .arg(&output)
            .output()
            .unwrap();
        assert!(!result.status.success());
        assert!(!output.exists());
    }
}

#[test]
fn a_staged_record_not_classified_by_the_manifest_is_refused() {
    let root = setup("unclassified", "staged");
    let rows = ibcmd_filedb::FileDb::open(&root.0)
        .unwrap()
        .config_rows("CONFIGCASSAVE")
        .unwrap();
    let row = rows
        .iter()
        .find(|r| !r.file_name.ends_with("__configinfo"))
        .unwrap();
    let needle: Vec<u8> = row
        .file_name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    let file = root.0.join("1Cv8.1CD");
    let mut bytes = fs::read(&file).unwrap();
    let at = bytes
        .windows(needle.len())
        .position(|v| v == needle)
        .unwrap();
    let logical_offset = row.file_name.find("__").unwrap() + 2;
    bytes[at + logical_offset * 2] ^= 1;
    fs::write(&file, bytes).unwrap();
    let out = root.0.join("out");
    let result = export(&root.0, "ТестРасширение", &out);
    assert!(!result.status.success());
    assert!(!out.exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("not classified"));
}

#[cfg(unix)]
#[test]
fn output_symlinks_are_refused_without_writing_the_target() {
    let root = setup("symlink", "applied");
    let target = root.0.join("target");
    fs::create_dir(&target).unwrap();
    let link = root.0.join("linked");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let result = export(&root.0, "ТестРасширение", &link);
    assert!(!result.status.success());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    assert!(link.is_symlink());
}

#[test]
fn an_oversized_physical_part_number_cannot_wrap_into_part_zero() {
    let root = setup("part-range", "applied");
    let mut db = ibcmd_filedb::FileDb::open(&root.0).unwrap();
    let rows = db.config_rows("CONFIGCAS").unwrap();
    let table = db.table("CONFIGCAS").unwrap();
    let part_offset = 1 + table
        .fields
        .iter()
        .take_while(|f| f.name != "PARTNO")
        .map(ibcmd_filedb::Field::size)
        .sum::<usize>();
    let needle: Vec<u8> = rows[0]
        .file_name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    drop(db);
    let file = root.0.join("1Cv8.1CD");
    let mut bytes = fs::read(&file).unwrap();
    let text_at = bytes
        .windows(needle.len())
        .position(|v| v == needle)
        .unwrap();
    // The free flag and NVC character count precede the first UTF-16 unit.
    let record_at = text_at - 1 - std::mem::size_of::<u16>();
    // N(10): positive 4294967296, one past u32::MAX; formerly cast to zero.
    bytes[record_at + part_offset..record_at + part_offset + 6]
        .copy_from_slice(&[0x14, 0x29, 0x49, 0x67, 0x29, 0x60]);
    fs::write(&file, bytes).unwrap();
    let before = fs::read(&file).unwrap();
    let out = root.0.join("out");
    let result = export(&root.0, "ТестРасширение", &out);
    assert!(
        !result.status.success(),
        "an invalid physical part was published"
    );
    assert!(!out.exists());
    assert_eq!(fs::read(&file).unwrap(), before);
}

#[test]
fn unknown_staging_record_flags_cannot_hide_a_prepared_image() {
    let root = setup("unknown-staging-flag", "mixed");
    let file = root.0.join("1Cv8.1CD");
    let mut bytes = fs::read(&file).unwrap();
    let rows = ibcmd_filedb::FileDb::open(&root.0)
        .unwrap()
        .config_rows("CONFIGCASSAVE")
        .unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        let needle: Vec<_> = row
            .file_name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let positions: Vec<_> = bytes
            .windows(needle.len())
            .enumerate()
            .filter(|(at, value)| *at >= 3 && *value == needle && bytes[*at - 3] == 0)
            .map(|(at, _)| at)
            .collect();
        assert_eq!(positions.len(), 1);
        let record_start = positions[0] - 3;
        assert_eq!(bytes[record_start], 0);
        bytes[record_start] = 0xff;
    }
    fs::write(&file, &bytes).unwrap();
    let output = root.0.join("unknown-flag");
    let result = export(&root.0, "ТестРасширение", &output);
    assert!(
        !result.status.success(),
        "a corrupt prepared image fell back to active"
    );
    assert!(!output.exists());
    assert_eq!(fs::read(file).unwrap(), bytes);
}

#[test]
fn unmeasured_config_field_precision_and_collation_are_refused() {
    for (tag, from, to) in [
        (
            "part-precision",
            r#"{"PARTNO","N",0,10,0,"CS"}"#,
            r#"{"PARTNO","N",0,10,1,"CS"}"#,
        ),
        (
            "size-precision",
            r#"{"DATASIZE","N",0,20,0,"CS"}"#,
            r#"{"DATASIZE","N",0,20,1,"CS"}"#,
        ),
        (
            "name-collation",
            r#"{"FILENAME","NVC",0,128,0,"CI"}"#,
            r#"{"FILENAME","NVC",0,128,0,"CS"}"#,
        ),
        (
            "part-collation",
            r#"{"PARTNO","N",0,10,0,"CS"}"#,
            r#"{"PARTNO","N",0,10,0,"CI"}"#,
        ),
    ] {
        let root = setup(tag, "applied");
        let file = root.0.join("1Cv8.1CD");
        let mut bytes = fs::read(&file).unwrap();
        assert_eq!(from.len(), to.len());
        let offsets: Vec<_> = bytes
            .windows(from.len())
            .enumerate()
            .filter(|(_, value)| *value == from.as_bytes())
            .map(|(at, _)| at)
            .collect();
        assert!(!offsets.is_empty());
        for at in offsets {
            bytes[at..at + to.len()].copy_from_slice(to.as_bytes());
        }
        fs::write(&file, &bytes).unwrap();
        let output = root.0.join("unknown-profile");
        let result = export(&root.0, "ТестРасширение", &output);
        assert!(
            !result.status.success(),
            "unmeasured {tag} profile was published"
        );
        assert!(!output.exists());
        assert_eq!(fs::read(file).unwrap(), bytes);
    }
}

#[test]
fn unmeasured_cas_attributes_refuse_before_publication() {
    let root = setup("cas-attributes", "applied");
    let mut db = ibcmd_filedb::FileDb::open(&root.0).unwrap();
    let row = db.config_rows("CONFIGCAS").unwrap().remove(0);
    let table = db.table("CONFIGCAS").unwrap();
    let offset = 1 + table
        .fields
        .iter()
        .take_while(|f| f.name != "ATTRIBUTES")
        .map(ibcmd_filedb::Field::size)
        .sum::<usize>();
    let needle: Vec<_> = row
        .file_name
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    drop(db);
    let path = root.0.join("1Cv8.1CD");
    let mut bytes = fs::read(&path).unwrap();
    let positions: Vec<_> = bytes
        .windows(needle.len())
        .enumerate()
        .filter(|(at, v)| *at >= 3 && **v == needle && bytes[*at - 3] == 0)
        .map(|(at, _)| at)
        .collect();
    assert_eq!(positions.len(), 1);
    let record = positions[0] - 3;
    // N(5), positive one instead of the measured zero.
    bytes[record + offset..record + offset + 3].copy_from_slice(&[0x10, 0, 1]);
    fs::write(&path, &bytes).unwrap();
    let out = root.0.join("out");
    let result = export(&root.0, "ТестРасширение", &out);
    assert!(
        !result.status.success(),
        "unmeasured CAS Attributes were published"
    );
    assert!(!out.exists());
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn unknown_main_configuration_preparation_refuses_extension_publication() {
    let root = setup("main-prepared", "applied");
    let mut db = ibcmd_filedb::FileDb::open(&root.0).unwrap();
    assert!(db.config_rows("CONFIGSAVE").unwrap().is_empty());
    let main = db.table("CONFIG").unwrap();
    let stage = db.table("CONFIGSAVE").unwrap();
    let old = format!(
        "{{\"Files\",{},{},{}}}",
        stage.records_page, stage.blob_page, stage.index_page
    );
    let replacement = format!(
        "{{\"Files\",{:02},{:02},{:02}}}",
        main.records_page, main.blob_page, main.index_page
    );
    assert_eq!(old.len(), replacement.len());
    drop(db);
    let path = root.0.join("1Cv8.1CD");
    let mut before = fs::read(&path).unwrap();
    let positions: Vec<_> = before
        .windows(old.len())
        .enumerate()
        .filter_map(|(i, bytes)| (bytes == old.as_bytes()).then_some(i))
        .collect();
    assert_eq!(positions.len(), 1);
    // Reuse valid CONFIG objects under the identical CONFIGSAVE schema. This
    // creates a nonempty, unclassified main stage without invoking a writer.
    before[positions[0]..positions[0] + old.len()].copy_from_slice(replacement.as_bytes());
    fs::write(&path, &before).unwrap();
    let mut db = ibcmd_filedb::FileDb::open(&root.0).unwrap();
    assert!(!db.config_rows("CONFIGSAVE").unwrap().is_empty());
    drop(db);
    let output = root.0.join("out");
    let result = export(&root.0, "ТестРасширение", &output);
    assert!(
        !result.status.success(),
        "an unmeasured prepared main configuration was ignored"
    );
    assert!(!output.exists());
    assert_eq!(fs::read(path).unwrap(), before);
}
