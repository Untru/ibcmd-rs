//! The partial import of listed files (`infobase config import files`,
//! Untru/ibcmd-rs#363), proven offline: the rows of a platform-written `.cf`
//! are laid out as the Config table's `<FileName>__part0.bin` files, the
//! export of those rows (`mssql-dump-config --rows-dir`, the export path
//! `infobase config export` takes) is edited in one module and one metadata
//! file, and the edited files are staged by `mssql-stage-source-objects
//! --file ... --script-only` with its base rows read from that folder
//! (`IBCMD_RS_BASE_ROWS_DIR`: no server is reached). The bulk rows file the
//! stage writes must hold the rows of the edited files and `versions`,
//! nothing else, and the rows folder with those rows in place of its own must
//! export as the edited tree.
//!
//! The fixture is `home_page/two_variable/input.cf` (Designer `/DumpCfg` of
//! 8.3.27.2214): two common forms with a module each.

mod common;

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::Value;

const FIXTURE: &str = "home_page/two_variable/input.cf";
/// The form whose module the test edits, and the one whose metadata file it edits.
const MODULE: &str = "CommonForms/ФормаСлева/Ext/Form/Module.bsl";
const MODULE_OWNER: &str = "CommonForms/ФормаСлева.xml";
const DESCRIPTOR: &str = "CommonForms/ФормаСправа.xml";
/// Beside the tree in an export: the research command's own report.
const NOT_THE_TREE: &[&str] = &["manifest.json"];
/// The versions of the objects (`configVersion`) move with every staged row.
const VERSIONS_FILE: &str = "ConfigDumpInfo.xml";

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ibcmd-rs-import-files-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The rows of a `.cf` as the Config table holds them: an entry's payload is
/// the row's BinaryData (raw deflate).
fn rows_of(cf: &Path, dir: &Path) {
    let bytes = fs::read(cf).unwrap();
    let length = bytes.len() as u64;
    let archive = ibcmd_cf::archive::decode_packed_archive(
        std::io::Cursor::new(bytes),
        ibcmd_core::limits::ResourceLimits::for_input_bytes(length),
        ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli").unwrap(),
    )
    .unwrap();
    fs::create_dir_all(dir).unwrap();
    for entry in archive.entries() {
        fs::write(
            dir.join(format!("{}__part0.bin", entry.name())),
            entry.payload(),
        )
        .unwrap();
    }
}

/// `mssql-dump-config --rows-dir <rows> -o <out>`: the export of the rows.
fn export_rows(rows: &Path, out: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["mssql-dump-config", "--rows-dir"])
        .arg(rows)
        .args([
            "--extract-metadata-xml",
            "--extract-module-text",
            "--no-binary-rows",
            "--platform",
            "8.3.27",
            "-o",
        ])
        .arg(out)
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", text(&output.stderr));
}

/// The tree's files, the export's report aside.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = common::files(root);
    files.retain(|path, _| !NOT_THE_TREE.contains(&path.as_str()));
    files
}

fn write_tree(root: &Path, files: &BTreeMap<String, Vec<u8>>) {
    for (path, bytes) in files {
        let target = root.join(path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
}

/// Replaces `from` with `to` once in a file of the tree.
fn edit(root: &Path, relative: &str, from: &str, to: &str) {
    let path = root.join(relative);
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content.matches(from).count(), 1, "{relative}: {from}");
    fs::write(&path, content.replacen(from, to, 1)).unwrap();
}

/// `mssql-stage-source-objects --file ... --script-only` over the rows
/// folder: the process's output.
fn stage(tree: &Path, rows: &Path, script: &Path, files: &[&str]) -> Output {
    let mut args: Vec<&OsStr> = vec![
        "mssql-stage-source-objects".as_ref(),
        "--database".as_ref(),
        "ibcmd_rs_import_files".as_ref(),
        "--platform".as_ref(),
        "8.3.27".as_ref(),
        "--source-root".as_ref(),
        tree.as_os_str(),
        "--replace-config-save".as_ref(),
        "--allow-non-lab".as_ref(),
        "--script-only".as_ref(),
        "--verify".as_ref(),
        "--script-output".as_ref(),
        script.as_os_str(),
    ];
    for file in files {
        args.push("--file".as_ref());
        args.push(file.as_ref());
    }
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        .env("IBCMD_RS_BASE_ROWS_DIR", rows)
        // nothing may be launched: no PATH to find sqlcmd or a platform on
        .env("PATH", "")
        .output()
        .unwrap()
}

fn staged(tree: &Path, rows: &Path, script: &Path, files: &[&str]) -> Value {
    let output = stage(tree, rows, script, files);
    assert!(
        output.status.success(),
        "{}\n{}",
        text(&output.stdout),
        text(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The rows of a bcp native file of the bulk stage's table, whole:
/// `FileName -> BinaryData` (the parts joined in order).
fn read_bcp(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut rows = BTreeMap::<String, Vec<(i32, Vec<u8>)>>::new();
    let mut at = 0;
    let take = |at: &mut usize, len: usize| {
        let slice = &bytes[*at..*at + len];
        *at += len;
        slice
    };
    while at < bytes.len() {
        let name_len = u16::from_le_bytes(take(&mut at, 2).try_into().unwrap()) as usize;
        let units = take(&mut at, name_len)
            .chunks(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        let _kind = take(&mut at, 1)[0];
        let _data_size = i64::from_le_bytes(take(&mut at, 8).try_into().unwrap());
        let part = i32::from_le_bytes(take(&mut at, 4).try_into().unwrap());
        let len = i64::from_le_bytes(take(&mut at, 8).try_into().unwrap()) as usize;
        let blob = take(&mut at, len).to_vec();
        rows.entry(String::from_utf16(&units).unwrap())
            .or_default()
            .push((part, blob));
    }
    rows.into_iter()
        .map(|(name, mut parts)| {
            parts.sort_by_key(|(part, _)| *part);
            (name, parts.into_iter().flat_map(|(_, blob)| blob).collect())
        })
        .collect()
}

/// The rows the stage wrote: its bulk rows file beside its prepare script.
fn staged_rows(report: &Value) -> BTreeMap<String, Vec<u8>> {
    let prepare = report["scripts"][0].as_str().unwrap();
    assert!(prepare.ends_with("_bulk_prepare.sql"), "{prepare}");
    let rows_file = prepare.replace("_bulk_prepare.sql", "_bulk_rows.bcp");
    read_bcp(&fs::read(rows_file).unwrap())
}

/// The uuid of the object a metadata file describes.
fn uuid_of(path: &Path) -> String {
    let xml = fs::read_to_string(path).unwrap();
    let at = xml.find(" uuid=\"").unwrap() + " uuid=\"".len();
    xml[at..at + 36].to_string()
}

/// The base rows of the fixture, their export, and the export edited in one
/// module and one metadata file.
struct Case {
    scratch: Scratch,
    rows: PathBuf,
    exported: BTreeMap<String, Vec<u8>>,
    edited: PathBuf,
}

fn case(tag: &str) -> Case {
    let scratch = Scratch::new(tag);
    let rows = scratch.join("rows");
    rows_of(&common::fixture(FIXTURE), &rows);
    let export = scratch.join("export");
    export_rows(&rows, &export);
    let exported = tree(&export);
    for file in [MODULE, MODULE_OWNER, DESCRIPTOR] {
        assert!(exported.contains_key(file), "the fixture has no {file}");
    }
    let edited = scratch.join("edited");
    write_tree(&edited, &exported);
    edit(&edited, MODULE, "Номер = 1;", "Номер = 2;");
    edit(
        &edited,
        DESCRIPTOR,
        "<Comment/>",
        "<Comment>Изменено частичной загрузкой</Comment>",
    );
    Case {
        scratch,
        rows,
        exported,
        edited,
    }
}

/// The rows folder with the staged rows in place of its own, exported.
fn export_with(
    case: &Case,
    rows: &BTreeMap<String, Vec<u8>>,
    tag: &str,
) -> BTreeMap<String, Vec<u8>> {
    let overridden = case.scratch.join(&format!("rows-{tag}"));
    fs::create_dir_all(&overridden).unwrap();
    for entry in fs::read_dir(&case.rows).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), overridden.join(entry.file_name())).unwrap();
    }
    for (name, blob) in rows {
        fs::write(overridden.join(format!("{name}__part0.bin")), blob).unwrap();
    }
    let out = case.scratch.join(&format!("export-{tag}"));
    export_rows(&overridden, &out);
    tree(&out)
}

/// The files of `actual` that differ from `expected`, or that only one has.
fn differing(
    expected: &BTreeMap<String, Vec<u8>>,
    actual: &BTreeMap<String, Vec<u8>>,
) -> BTreeSet<String> {
    expected
        .keys()
        .chain(actual.keys())
        .filter(|path| expected.get(*path) != actual.get(*path))
        .cloned()
        .collect()
}

#[test]
fn listed_files_stage_their_rows_only_and_export_as_edited() {
    let case = case("whole");
    let script = case.scratch.join("stage").join("stage.sql");
    let report = staged(&case.edited, &case.rows, &script, &[MODULE, DESCRIPTOR]);
    let rows = staged_rows(&report);

    // The module's row is its form's body, the metadata file's the
    // descriptor of its form: those two and `versions`, no other row.
    let module_form = uuid_of(&case.edited.join(MODULE_OWNER));
    let descriptor = uuid_of(&case.edited.join(DESCRIPTOR));
    let expected = [
        format!("{module_form}.0"),
        descriptor.clone(),
        "versions".to_string(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    assert_eq!(rows.keys().cloned().collect::<BTreeSet<_>>(), expected);
    for (name, blob) in &rows {
        let base = fs::read(case.rows.join(format!("{name}__part0.bin"))).unwrap();
        assert_ne!(blob, &base, "{name}: the staged row is the target's own");
    }
    // The guard compared the listed files, and only them.
    assert_eq!(report["verification"]["checked_files"], 2, "{report}");
    assert_eq!(report["verification"]["identical_files"], 2, "{report}");

    // The target's rows with the staged ones in their place export as the
    // edited tree; the versions of the objects move with them.
    let edited = tree(&case.edited);
    let after = export_with(&case, &rows, "whole");
    assert_eq!(
        differing(&edited, &after),
        BTreeSet::from([VERSIONS_FILE.to_string()])
    );
    assert_eq!(
        differing(&case.exported, &after),
        BTreeSet::from([
            MODULE.to_string(),
            DESCRIPTOR.to_string(),
            VERSIONS_FILE.to_string()
        ])
    );
}

#[test]
fn a_sparse_directory_stages_the_same_rows() {
    // The directory holds the listed files and their objects' metadata
    // files, nothing else (the platform's `--partial` imports such a one:
    // scripts/apply-trace/lab/validate_sparse_import.ps1).
    let case = case("sparse");
    let sparse = case.scratch.join("sparse");
    let edited = tree(&case.edited);
    write_tree(
        &sparse,
        &edited
            .iter()
            .filter(|(path, _)| [MODULE, MODULE_OWNER, DESCRIPTOR].contains(&path.as_str()))
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect(),
    );
    let report = staged(
        &sparse,
        &case.rows,
        &case.scratch.join("stage-sparse").join("stage.sql"),
        &[MODULE, DESCRIPTOR],
    );
    let rows = staged_rows(&report);
    let module_form = uuid_of(&sparse.join(MODULE_OWNER));
    let descriptor = uuid_of(&sparse.join(DESCRIPTOR));
    assert_eq!(
        rows.keys().cloned().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            format!("{module_form}.0"),
            descriptor,
            "versions".to_string()
        ])
    );
    let after = export_with(&case, &rows, "sparse");
    assert_eq!(
        differing(&edited, &after),
        BTreeSet::from([VERSIONS_FILE.to_string()])
    );
}

#[test]
fn what_the_stage_cannot_place_is_refused_before_anything_is_written() {
    let case = case("refused");
    fs::write(case.edited.join("notes.txt"), b"not a configuration file").unwrap();
    // a file in an object's folder that no row of the object is built from
    let stray = "CommonForms/ФормаСправа/Ext/Notes.txt";
    fs::create_dir_all(case.edited.join(stray).parent().unwrap()).unwrap();
    fs::write(case.edited.join(stray), b"notes").unwrap();
    for (files, needle) in [
        (vec!["../elsewhere/Catalogs/X.xml"], "вне каталога"),
        (vec!["CommonForms/Удаленная.xml"], "удаление"),
        (vec!["Configuration.xml"], "Configuration.xml"),
        (vec!["notes.txt"], "не принадлежит ни одному объекту"),
        (
            vec![MODULE, "CommonForms/ФормаСлева/Ext/Form/Items/Picture.png"],
            "нет в каталоге",
        ),
        (vec![MODULE, stray], "не собирается ни одна строка"),
    ] {
        let script = case.scratch.join("refused").join("stage.sql");
        let output = stage(&case.edited, &case.rows, &script, &files);
        let stderr = text(&output.stderr);
        assert!(
            !output.status.success(),
            "{files:?}: {}",
            text(&output.stdout)
        );
        assert!(stderr.contains(needle), "{files:?}: {stderr}");
        assert!(!stderr.contains("panicked"), "{files:?}: {stderr}");
        assert!(
            !script.parent().unwrap().exists(),
            "{files:?}: wrote scripts"
        );
    }
}
