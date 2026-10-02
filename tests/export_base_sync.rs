//! `--base` and `--sync` of the configuration export (Untru/ibcmd-rs#362),
//! offline: the rows of a fixture's `.cf` are laid out as the Config table's
//! `<FileName>__part0.bin` files and exported by `mssql-dump-config
//! --rows-dir`, the export path `infobase config export` takes.
//!
//! The acceptance check is always a full export of the same rows: what an
//! incremental export writes is byte for byte what the full export writes,
//! and a folder updated with `--sync` ends as the full export.

mod common;

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde_json::Value;

/// Files no comparison looks at: `manifest.json` is the research command's
/// own report of the rows it converted, beside the tree.
const NOT_THE_TREE: &[&str] = &["manifest.json"];

/// Standard Base64 (the `.cf.b64` fixtures of `tests/fixtures/native-evidence`).
fn decode_base64(text: &str) -> Vec<u8> {
    let mut output = Vec::new();
    let (mut buffer, mut bits) = (0_u32, 0_u8);
    for byte in text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace() && *byte != b'=')
    {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid Base64 byte 0x{byte:02x}"),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    output
}

/// The rows of a `.cf` (or a `.cf.b64`) as `mssql-dump-config --rows-dir`
/// reads them: a CF entry's payload is the row's BinaryData (raw deflate).
fn rows_of(cf: &Path, dir: &Path) {
    let bytes = if cf.extension().is_some_and(|extension| extension == "b64") {
        decode_base64(&fs::read_to_string(cf).unwrap())
    } else {
        fs::read(cf).unwrap()
    };
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

/// `mssql-dump-config --rows-dir <rows> -o <out> <extra>`: its report.
fn dump(rows: &Path, out: &Path, extra: &[&OsStr]) -> Value {
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
        .args(extra)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{extra:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn base_arg(path: &Path) -> Vec<&OsStr> {
    vec!["--base".as_ref(), path.as_os_str()]
}

/// The tree's files, `manifest.json` aside.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = common::files(root);
    files.retain(|path, _| !NOT_THE_TREE.contains(&path.as_str()));
    files
}

fn copy_tree(from: &Path, to: &Path) {
    for (path, bytes) in common::files(from) {
        let target = to.join(&path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
}

fn put(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

/// A fixture's rows and their full export.
struct Exported {
    rows: PathBuf,
    full: PathBuf,
}

/// A fixture by its path under `tests/fixtures`.
fn fixture(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(path)
}

fn exported(path: &str, tag: &str) -> Exported {
    let rows = common::temp_dir(&format!("{tag}-rows"));
    rows_of(&fixture(path), &rows);
    let full = common::temp_dir(&format!("{tag}-full"));
    let report = dump(&rows, &full, &[]);
    assert!(report.get("incremental").is_none(), "{report}");
    assert!(full.join("ConfigDumpInfo.xml").is_file(), "{path}");
    Exported { rows, full }
}

fn config_dump_info(root: &Path) -> String {
    String::from_utf8(fs::read(root.join("ConfigDumpInfo.xml")).unwrap()).unwrap()
}

/// `(name, id)` of every row of a ConfigDumpInfo.xml (its top-level entries:
/// the lines with a configVersion).
fn entries(text: &str) -> Vec<(String, String)> {
    let attribute = |line: &str, name: &str| {
        let marker = format!(" {name}=\"");
        let start = line.find(&marker)? + marker.len();
        let end = start + line[start..].find('"')?;
        Some(line[start..end].to_owned())
    };
    text.lines()
        .filter(|line| line.contains(" configVersion=\""))
        .map(|line| {
            (
                attribute(line, "name").unwrap(),
                attribute(line, "id").unwrap(),
            )
        })
        .collect()
}

/// The ConfigDumpInfo.xml `text` with the configVersion of the row `id`
/// moved: the base of a configuration where that row changed.
fn with_version_moved(text: &str, id: &str) -> String {
    text.lines()
        .map(|line| {
            if line.contains(&format!(" id=\"{id}\" ")) {
                let at = line.find(" configVersion=\"").unwrap() + " configVersion=\"".len();
                let mut line = line.to_owned();
                let moved = if &line[at..at + 1] == "0" { "1" } else { "0" };
                line.replace_range(at..at + 1, moved);
                line
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Configurations of the fixtures (paths under `tests/fixtures`).
const FIXTURES: &[&str] = &[
    // two common forms with modules, the home page, the interface
    "external/home_page/two_equal/input.cf",
    // a form body written as a module only (`CommonForm.ФормаСправа.Module`)
    "external/export_notes/wrong_kind.cf",
    // a constant, a defined type, an exchange plan, a session parameter
    "external/upgrade/exchange_plan/current.cf",
    // a role and its rights
    "external/role_rights/new_true_emt_false/input.cf",
    // the platform's own .cf: catalogs with object modules and list forms, a
    // report with its modules and a data composition schema template
    "native-evidence/8.3.27.2214/dcs-form-list-settings-server-state/configuration.cf.b64",
    // the platform's own .cf: a task
    "native-evidence/8.3.27.2214/task-basic/configuration.cf.b64",
];

/// The fixture most cases below update.
const HOME_PAGE: &str = "external/home_page/two_equal/input.cf";

#[test]
fn nothing_changed_writes_config_dump_info_alone() {
    for (index, fixture) in FIXTURES.iter().enumerate() {
        let tag = format!("base-sync-same-{index}");
        let exported = exported(fixture, &tag);
        let base = common::temp_dir(&format!("{tag}-base")).with_extension("xml");
        fs::copy(exported.full.join("ConfigDumpInfo.xml"), &base).unwrap();

        // into an empty folder: ConfigDumpInfo.xml alone, the full export's
        let out = common::temp_dir(&format!("{tag}-out"));
        let report = dump(&exported.rows, &out, &base_arg(&base));
        let incremental = &report["incremental"];
        assert_eq!(incremental["mode"], "incremental", "{fixture}: {report}");
        assert_eq!(incremental["changed_entries"], 0, "{fixture}: {report}");
        assert_eq!(
            incremental["unchanged_entries"].as_u64().unwrap() as usize,
            entries(&config_dump_info(&exported.full)).len(),
            "{fixture}: {report}"
        );
        assert_eq!(incremental["written_files"], 1, "{fixture}: {report}");
        assert_eq!(
            tree(&out).keys().collect::<Vec<_>>(),
            vec!["ConfigDumpInfo.xml"],
            "{fixture}"
        );
        assert_eq!(
            fs::read(out.join("ConfigDumpInfo.xml")).unwrap(),
            fs::read(exported.full.join("ConfigDumpInfo.xml")).unwrap(),
            "{fixture}"
        );

        // over the full export itself, with --sync: nothing moves
        let again = common::temp_dir(&format!("{tag}-again"));
        copy_tree(&exported.full, &again);
        let report = dump(
            &exported.rows,
            &again,
            &[
                "--base".as_ref(),
                again.join("ConfigDumpInfo.xml").as_os_str(),
                "--sync".as_ref(),
            ],
        );
        assert_eq!(
            report["incremental"]["removed_files"],
            serde_json::json!([]),
            "{fixture}: {report}"
        );
        assert_eq!(tree(&again), tree(&exported.full), "{fixture}");
    }
}

#[test]
fn a_changed_row_writes_its_own_files_and_no_other() {
    for (index, fixture) in FIXTURES.iter().enumerate() {
        let tag = format!("base-sync-row-{index}");
        let exported = exported(fixture, &tag);
        let full = tree(&exported.full);
        let info = config_dump_info(&exported.full);
        let mut owners = BTreeMap::<String, String>::new();
        let mut written_by = BTreeMap::<String, BTreeSet<String>>::new();
        for (name, id) in entries(&info) {
            let base = common::temp_dir(&format!("{tag}-base")).with_extension("xml");
            fs::write(&base, with_version_moved(&info, &id)).unwrap();
            let out = common::temp_dir(&format!("{tag}-out"));
            let report = dump(&exported.rows, &out, &base_arg(&base));
            assert_eq!(
                report["incremental"]["changed_entries"], 1,
                "{fixture} {name}: {report}"
            );
            let written = tree(&out);
            // the full export's bytes, its ConfigDumpInfo.xml among them
            for (path, bytes) in &written {
                assert_eq!(Some(bytes), full.get(path), "{fixture} {name}: {path}");
            }
            assert!(written.contains_key("ConfigDumpInfo.xml"));
            let own = written
                .into_keys()
                .filter(|path| path != "ConfigDumpInfo.xml")
                .collect::<BTreeSet<_>>();
            for path in &own {
                if let Some(other) = owners.insert(path.clone(), name.clone()) {
                    panic!("{fixture}: {path} is written by {other} and by {name}");
                }
            }
            written_by.insert(name, own);
        }
        // every file of the full export is written by exactly one row
        let every = full
            .keys()
            .filter(|path| *path != "ConfigDumpInfo.xml")
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            owners.keys().cloned().collect::<BTreeSet<_>>(),
            every,
            "{fixture}: {written_by:?}"
        );
        // and the row is the entry the file's path names
        for (name, expected) in [
            ("Configuration.БазаТест", vec!["Configuration.xml"]),
            ("Language.Русский", vec!["Languages/Русский.xml"]),
            (
                "Configuration.БазаТест.ClientApplicationInterface",
                vec!["Ext/ClientApplicationInterface.xml"],
            ),
            (
                "CommonForm.ФормаСлева.Form",
                vec![
                    "CommonForms/ФормаСлева/Ext/Form.xml",
                    "CommonForms/ФормаСлева/Ext/Form/Module.bsl",
                ],
            ),
            (
                "Role.РольПроверка.Rights",
                vec!["Roles/РольПроверка/Ext/Rights.xml"],
            ),
        ] {
            if let Some(own) = written_by.get(name) {
                let expected: BTreeSet<String> = expected.into_iter().map(str::to_owned).collect();
                assert_eq!(own, &expected, "{fixture} {name}");
            }
        }
    }
}

#[test]
fn sync_removes_what_the_configuration_no_longer_has() {
    let tag = "base-sync-removed";
    let exported = exported(HOME_PAGE, tag);
    let info = config_dump_info(&exported.full);
    // The base lists a catalog the configuration no longer has, and the
    // folder holds its files, a stray file and the user's own.
    let removed = "\t\t<Metadata name=\"Catalog.Удалённый\" id=\"0f0f0f0f-0000-4000-8000-000000000001\" \
configVersion=\"0123456789abcdef0123456789abcdef00000000\"/>\r\n\
\t\t<Metadata name=\"Catalog.Удалённый.ObjectModule\" id=\"0f0f0f0f-0000-4000-8000-000000000001.0\" \
configVersion=\"0123456789abcdef0123456789abcdef00000000\"/>\r\n";
    let base_text = info.replacen(
        "\t\t<Metadata name=\"CommonForm.",
        &format!("{removed}\t\t<Metadata name=\"CommonForm."),
        1,
    );
    assert_ne!(base_text, info);
    let base = common::temp_dir(&format!("{tag}-base")).with_extension("xml");
    fs::write(&base, &base_text).unwrap();
    let make_folder = |name: &str| {
        let folder = common::temp_dir(&format!("{tag}-{name}"));
        copy_tree(&exported.full, &folder);
        put(&folder, "Catalogs/Удалённый.xml", "old");
        put(&folder, "Catalogs/Удалённый/Ext/ObjectModule.bsl", "old");
        put(&folder, "CommonForms/Лишняя.xml", "stray");
        put(&folder, "README.md", "mine");
        put(&folder, ".git/HEAD", "ref: refs/heads/master");
        folder
    };

    // --base alone writes what changed and removes nothing
    let kept = make_folder("kept");
    let before = common::files(&kept);
    let report = dump(&exported.rows, &kept, &base_arg(&base));
    assert_eq!(report["incremental"]["removed_entries"], 2, "{report}");
    assert_eq!(report["incremental"]["changed_entries"], 0, "{report}");
    let mut after = common::files(&kept);
    let mut before = before;
    after.remove("manifest.json");
    before.remove("manifest.json");
    assert_eq!(after, before);

    // --base --sync: the folder ends as the full export, the user's files stay
    let synced = make_folder("synced");
    let report = dump(
        &exported.rows,
        &synced,
        &["--base".as_ref(), base.as_os_str(), "--sync".as_ref()],
    );
    assert_eq!(
        report["incremental"]["removed_files"],
        serde_json::json!([
            "Catalogs/Удалённый.xml",
            "Catalogs/Удалённый/Ext/ObjectModule.bsl",
            "CommonForms/Лишняя.xml",
        ]),
        "{report}"
    );
    let mut ours = tree(&synced);
    assert_eq!(ours.remove("README.md").as_deref(), Some(&b"mine"[..]));
    assert_eq!(
        ours.remove(".git/HEAD").as_deref(),
        Some(&b"ref: refs/heads/master"[..])
    );
    assert_eq!(ours, tree(&exported.full));
    assert!(!synced.join("Catalogs").exists());
}

#[test]
fn sync_without_a_base_leaves_the_full_export() {
    let tag = "base-sync-full";
    let exported = exported(HOME_PAGE, tag);
    let folder = common::temp_dir(&format!("{tag}-folder"));
    copy_tree(&exported.full, &folder);
    // an object removed since, a file of a form that lost its module, a stale
    // file of the configuration, and the user's own
    put(&folder, "CommonForms/Старая.xml", "old");
    put(&folder, "CommonForms/Старая/Ext/Form.xml", "old");
    put(&folder, "Ext/Splash.xml", "old");
    put(&folder, "CommonForms/ФормаСлева.xml", "edited");
    put(&folder, "notes.txt", "mine");
    put(&folder, ".ibcmd/index.tsv", "mine");
    let report = dump(&exported.rows, &folder, &["--sync".as_ref()]);
    let incremental = &report["incremental"];
    assert_eq!(incremental["mode"], "full", "{report}");
    assert_eq!(incremental["unchanged_entries"], 0, "{report}");
    assert_eq!(
        incremental["removed_files"],
        serde_json::json!([
            "CommonForms/Старая.xml",
            "CommonForms/Старая/Ext/Form.xml",
            "Ext/Splash.xml",
        ]),
        "{report}"
    );
    let mut ours = tree(&folder);
    assert!(ours.remove("notes.txt").is_some());
    assert!(ours.remove(".ibcmd/index.tsv").is_some());
    // every file written again, the edited one too
    assert_eq!(ours, tree(&exported.full));
}

#[test]
fn a_rename_rewrites_every_file_that_names_the_object() {
    // The base and the folder are an export from before the common form
    // `ФормаСтарая` was renamed `ФормаСлева`: its rows kept their versions, as
    // the rows that name it do (Configuration.xml lists it, the home page
    // places it), since 1C keeps a reference as an id.
    let tag = "base-sync-renamed";
    let exported = exported(HOME_PAGE, tag);
    let info = config_dump_info(&exported.full);
    let base_text = info.replace("CommonForm.ФормаСлева", "CommonForm.ФормаСтарая");
    let base = common::temp_dir(&format!("{tag}-base")).with_extension("xml");
    fs::write(&base, &base_text).unwrap();
    let folder = common::temp_dir(&format!("{tag}-folder"));
    copy_tree(&exported.full, &folder);
    fs::rename(
        folder.join("CommonForms/ФормаСлева.xml"),
        folder.join("CommonForms/ФормаСтарая.xml"),
    )
    .unwrap();
    fs::rename(
        folder.join("CommonForms/ФормаСлева"),
        folder.join("CommonForms/ФормаСтарая"),
    )
    .unwrap();
    let mut named_elsewhere = 0;
    for (path, bytes) in common::files(&folder) {
        let text = String::from_utf8_lossy(&bytes);
        if path.starts_with("CommonForms/") || !text.contains("ФормаСлева") {
            continue;
        }
        named_elsewhere += 1;
        fs::write(
            folder.join(&path),
            text.replace("ФормаСлева", "ФормаСтарая"),
        )
        .unwrap();
    }
    assert!(named_elsewhere > 0, "the fixture names the form elsewhere");
    let report = dump(
        &exported.rows,
        &folder,
        &["--base".as_ref(), base.as_os_str(), "--sync".as_ref()],
    );
    let incremental = &report["incremental"];
    assert_eq!(incremental["mode"], "full", "{report}");
    assert!(
        incremental["reason"]
            .as_str()
            .unwrap()
            .contains("CommonForm.ФормаСтарая -> CommonForm.ФормаСлева"),
        "{report}"
    );
    assert_eq!(
        incremental["changed_entries"].as_u64().unwrap() as usize,
        entries(&info).len(),
        "{report}"
    );
    assert_eq!(
        incremental["removed_files"],
        serde_json::json!([
            "CommonForms/ФормаСтарая.xml",
            "CommonForms/ФормаСтарая/Ext/Form.xml",
            "CommonForms/ФормаСтарая/Ext/Form/Module.bsl",
        ]),
        "{report}"
    );
    assert_eq!(tree(&folder), tree(&exported.full));
}

#[test]
fn a_base_of_another_xml_version_exports_every_row() {
    let tag = "base-sync-dialect";
    let exported = exported(HOME_PAGE, tag);
    let info = config_dump_info(&exported.full);
    let base = common::temp_dir(&format!("{tag}-base")).with_extension("xml");
    fs::write(&base, info.replace("version=\"2.20\"", "version=\"2.21\"")).unwrap();
    let out = common::temp_dir(&format!("{tag}-out"));
    let report = dump(&exported.rows, &out, &base_arg(&base));
    assert_eq!(report["incremental"]["mode"], "full", "{report}");
    assert_eq!(tree(&out), tree(&exported.full));
}

#[test]
fn a_base_that_is_no_config_dump_info_is_refused() {
    let tag = "base-sync-bad";
    let rows = common::temp_dir(&format!("{tag}-rows"));
    rows_of(&fixture("external/choice_list_dates/input.cf"), &rows);
    let base = common::temp_dir(&format!("{tag}-base")).with_extension("xml");
    fs::write(&base, "<MetaDataObject/>").unwrap();
    let out = common::temp_dir(&format!("{tag}-out"));
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["mssql-dump-config", "--rows-dir"])
        .arg(&rows)
        .args([
            "--extract-metadata-xml",
            "--extract-module-text",
            "--no-binary-rows",
            "--platform",
            "8.3.27",
            "-o",
        ])
        .arg(&out)
        .arg("--base")
        .arg(&base)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("is not a ConfigDumpInfo.xml"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!out.exists(), "nothing is written before the base is read");
}

/// A newer version of the `.cf` at `base`: `edit` applied to its export,
/// loaded back with `cf load` (as `tests/cf_update.rs` builds one).
fn newer(base: &Path, tag: &str, edit: impl FnOnce(&Path)) -> PathBuf {
    let tree = common::temp_dir(&format!("{tag}-edit"));
    assert!(common::export(base, &tree).status.success());
    edit(&tree);
    let output = common::temp_dir(&format!("{tag}-newer")).with_extension("cf");
    let _ = fs::remove_file(&output);
    let out = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "load"])
        .arg(&tree)
        .arg(&output)
        .arg("--base")
        .arg(base)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    output
}

#[test]
fn an_edited_configuration_updates_the_export_by_its_versions() {
    // The common form had no module; the newer configuration gives it one,
    // which changes the form body row alone.
    let tag = "base-sync-edited";
    let base_cf = common::fixture("choice_list_dates").join("input.cf");
    let newer_cf = newer(&base_cf, tag, |tree| {
        let module = tree.join("CommonForms/ФормаДаты/Ext/Form/Module.bsl");
        fs::create_dir_all(module.parent().unwrap()).unwrap();
        fs::write(
            &module,
            "\u{feff}&НаКлиенте\r\nПроцедура Проверка()\r\nКонецПроцедуры\r\n",
        )
        .unwrap();
    });
    let old_rows = common::temp_dir(&format!("{tag}-old-rows"));
    rows_of(&base_cf, &old_rows);
    let new_rows = common::temp_dir(&format!("{tag}-new-rows"));
    rows_of(&newer_cf, &new_rows);
    let expected = common::temp_dir(&format!("{tag}-expected"));
    dump(&new_rows, &expected, &[]);

    // the export of the older configuration, brought up to the newer one
    let folder = common::temp_dir(&format!("{tag}-folder"));
    dump(&old_rows, &folder, &[]);
    let base = common::temp_dir(&format!("{tag}-base")).with_extension("xml");
    fs::copy(folder.join("ConfigDumpInfo.xml"), &base).unwrap();
    let report = dump(
        &new_rows,
        &folder,
        &[
            "--base".as_ref(),
            folder.join("ConfigDumpInfo.xml").as_os_str(),
            "--sync".as_ref(),
        ],
    );
    assert_eq!(report["incremental"]["mode"], "incremental", "{report}");
    assert_eq!(tree(&folder), tree(&expected));

    // what that update wrote: the form body's files and ConfigDumpInfo.xml
    let out = common::temp_dir(&format!("{tag}-out"));
    let report = dump(&new_rows, &out, &base_arg(&base));
    assert_eq!(report["incremental"]["changed_entries"], 1, "{report}");
    assert_eq!(
        tree(&out).keys().collect::<Vec<_>>(),
        vec![
            "CommonForms/ФормаДаты/Ext/Form.xml",
            "CommonForms/ФормаДаты/Ext/Form/Module.bsl",
            "ConfigDumpInfo.xml",
        ]
    );
    let expected = tree(&expected);
    for (path, bytes) in tree(&out) {
        assert_eq!(Some(&bytes), expected.get(&path), "{path}");
    }
}

#[test]
fn the_options_need_a_source_export_of_the_whole_configuration() {
    let tag = "base-sync-options";
    let rows = common::temp_dir(&format!("{tag}-rows"));
    rows_of(&fixture("external/choice_list_dates/input.cf"), &rows);
    let out = common::temp_dir(&format!("{tag}-out"));
    for extra in [
        vec!["--sync", "--file-name", "versions"],
        vec!["--sync", "--overwrite"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
            .args(["mssql-dump-config", "--rows-dir"])
            .arg(&rows)
            .args([
                "--extract-metadata-xml",
                "--extract-module-text",
                "--no-binary-rows",
                "--platform",
                "8.3.27",
                "-o",
            ])
            .arg(&out)
            .args(&extra)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{extra:?}");
    }
    // without the source flags
    let output = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["mssql-dump-config", "--rows-dir"])
        .arg(&rows)
        .args(["--platform", "8.3.27", "--sync", "-o"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--no-binary-rows"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
