//! `cf load`: edits of an exported tree overlaid onto the file it came from.
//! The loaded file must export back to exactly the edited tree.

mod common;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn load(tree: &Path, output: &Path, base: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "load"])
        .arg(tree)
        .arg(output)
        .arg("--base")
        .arg(base)
        .output()
        .expect("run ibcmd-rs")
}

fn exported(fixture: &str, input: &str, tag: &str) -> (PathBuf, common::Scratch) {
    let base = common::fixture(fixture).join(input);
    let tree = common::temp_dir(tag);
    let run = common::export(&base, &tree);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    (base, tree)
}

fn fresh_output(tag: &str, ext: &str) -> common::Scratch {
    common::temp_file(tag, ext)
}

#[test]
fn edited_modules_and_form_round_trip_through_export() {
    let (base, tree) = exported("test_processor", "input.epf", "load-edit");
    let module = tree.join("ТестОбработка/Ext/ObjectModule.bsl");
    let text = fs::read_to_string(&module).unwrap();
    assert!(text.contains("Сумма = 0;"));
    fs::write(&module, text.replace("Сумма = 0;", "Сумма = 1;")).unwrap();
    let form_module = tree.join("ТестОбработка/Forms/Форма/Ext/Form/Module.bsl");
    fs::write(
        &form_module,
        "\u{feff}&НаКлиенте\r\nПроцедура Тест()\r\n\tСообщить(\"загружено\");\r\nКонецПроцедуры\r\n",
    )
    .unwrap();

    let out = fresh_output("load-edit-out", "epf");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let back = common::temp_dir("load-edit-back");
    assert!(common::export(&out, &back).status.success());
    common::assert_tree_eq(&tree, &back);
}

#[test]
fn report_module_edit_round_trips() {
    let (base, tree) = exported("test_report", "input.erf", "load-report");
    let module = tree.join("ТестОтчет/Ext/ObjectModule.bsl");
    let mut text = fs::read_to_string(&module).unwrap();
    text.push_str("\r\n// правка\r\n");
    fs::write(&module, text).unwrap();
    let out = fresh_output("load-report-out", "erf");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let back = common::temp_dir("load-report-back");
    assert!(common::export(&out, &back).status.success());
    common::assert_tree_eq(&tree, &back);
}

#[test]
fn load_without_changes_is_refused() {
    let (base, tree) = exported("test_processor", "input.epf", "load-same");
    let out = fresh_output("load-same-out", "epf");
    let run = load(&tree, &out, &base);
    assert!(!run.status.success());
    assert!(String::from_utf8_lossy(&run.stderr).contains("no_changes"));
    assert!(!out.exists());
}

#[test]
fn extension_module_edit_round_trips() {
    // A .cfe container has no `versions` entry; the edit must still load.
    let (base, tree) = exported("test_extension", "input.cfe", "load-cfe");
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    let out = fresh_output("load-cfe-out", "cfe");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let back = common::temp_dir("load-cfe-back");
    assert!(common::export(&out, &back).status.success());
    // Native file-CFE export has no ConfigDumpInfo.xml. Compare the entire
    // edited export rather than dropping a generated file from either tree.
    assert!(!tree.join("ConfigDumpInfo.xml").exists());
    assert!(!back.join("ConfigDumpInfo.xml").exists());
    common::assert_tree_eq(&tree, &back);
}

/// Every entry of the container at `path` by name, packed as stored.
fn packed_entries(path: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let archive = ibcmd_cf::archive::decode_packed_archive(
        fs::File::open(path).unwrap(),
        ibcmd_core::limits::ResourceLimits::for_input_bytes(fs::metadata(path).unwrap().len()),
        ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli").unwrap(),
    )
    .unwrap();
    ibcmd_rs::external::export::entries_of(&archive)
        .into_iter()
        .collect()
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// `"<entry>",<digest>` of the container's configinfo.
fn configinfo_digest(entries: &std::collections::BTreeMap<String, Vec<u8>>, entry: &str) -> String {
    use std::io::Read;
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(entries["configinfo"].as_slice())
        .read_to_string(&mut text)
        .unwrap();
    let marker = format!("\"{entry}\",");
    let at = text.find(&marker).unwrap() + marker.len();
    text[at..]
        .split([',', '}'])
        .next()
        .unwrap()
        .trim()
        .to_owned()
}

#[test]
fn extension_load_refreshes_the_configinfo_digest_of_the_edited_entry() {
    // configinfo keeps the SHA-1 of each entry's packed bytes. 8.3.27.2214
    // updating an extension already installed keeps every entry whose digest
    // did not change: with the old digest the edit never reached the infobase
    // (platform probe; with the digest refreshed it does).
    use sha1::Digest;
    let (base, tree) = exported("test_extension", "input.cfe", "load-cfe-digest");
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    let out = fresh_output("load-cfe-digest-out", "cfe");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let (before, after) = (packed_entries(&base), packed_entries(&out));
    let edited = "554f39c6-e029-4ee0-8535-8c15291a6a3f.0";
    assert_ne!(
        before[edited], after[edited],
        "the module entry was rewritten"
    );
    for (name, packed) in &after {
        if name != "configinfo" {
            assert_eq!(
                configinfo_digest(&after, name),
                base64(&sha1::Sha1::digest(packed)),
                "{name}"
            );
        }
    }
    let untouched = "554f39c6-e029-4ee0-8535-8c15291a6a3f";
    assert_eq!(before[untouched], after[untouched]);
}

#[test]
fn a_loaded_file_takes_the_next_edit_of_the_same_tree() {
    // ConfigDumpInfo.xml is derived from versions/configinfo, which a load
    // refreshes; the tree still carries the old one, and that is no edit.
    let (base, tree) = exported("test_extension", "input.cfe", "load-cfe-chain");
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    let first = fresh_output("load-cfe-chain-1", "cfe");
    let run = load(&tree, &first, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    fs::write(&module, text.replace("\"1.0\"", "\"3.0\"")).unwrap();
    let second = fresh_output("load-cfe-chain-2", "cfe");
    let run = load(&tree, &second, &first);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let back = common::temp_dir("load-cfe-chain-back");
    assert!(common::export(&second, &back).status.success());
    let module_back =
        fs::read_to_string(back.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"))
            .unwrap();
    assert!(module_back.contains("\"3.0\""), "{module_back}");
}

#[test]
fn module_text_that_is_not_utf8_is_refused_with_the_path() {
    let (base, tree) = exported("test_processor", "input.epf", "load-cp1251");
    // "Сумма = 1;" in windows-1251
    let cp1251 = [
        0xd1, 0xf3, 0xec, 0xec, 0xe0, b' ', b'=', b' ', b'1', b';', b'\r', b'\n',
    ];
    fs::write(tree.join("ТестОбработка/Ext/ObjectModule.bsl"), cp1251).unwrap();
    let out = fresh_output("load-cp1251-out", "epf");
    let run = load(&tree, &out, &base);
    assert!(!run.status.success());
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("ObjectModule.bsl"), "{stderr}");
    assert!(stderr.contains("UTF-8"), "{stderr}");
    assert!(!out.exists());
}

#[test]
fn dot_directories_of_the_tree_are_ignored() {
    let (base, tree) = exported("test_processor", "input.epf", "load-dot");
    fs::create_dir_all(tree.join(".git")).unwrap();
    fs::write(tree.join(".git/config"), "[core]\n").unwrap();
    fs::create_dir_all(tree.join(".vscode")).unwrap();
    fs::write(tree.join(".vscode/settings.json"), "{}\n").unwrap();
    let module = tree.join("ТестОбработка/Ext/ObjectModule.bsl");
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("Сумма = 0;", "Сумма = 2;")).unwrap();
    let out = fresh_output("load-dot-out", "epf");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    fs::remove_dir_all(tree.join(".git")).unwrap();
    fs::remove_dir_all(tree.join(".vscode")).unwrap();
    let back = common::temp_dir("load-dot-back");
    assert!(common::export(&out, &back).status.success());
    common::assert_tree_eq(&tree, &back);
}

#[test]
fn a_tree_of_another_dialect_is_refused_with_a_hint() {
    // A tree exported for 8.5 (XML 2.21) and loaded with the default 2.20
    // listed every XML file as changed, with no hint why.
    let base = common::fixture("test_processor").join("input.epf");
    let tree = common::temp_dir("load-dialect");
    let run = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(&base)
        .arg(&tree)
        .args(["--platform", "8.5.1", "--overwrite"])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let out = fresh_output("load-dialect-out", "epf");
    let run = load(&tree, &out, &base);
    assert!(!run.status.success());
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("--platform 8.5.1"), "{stderr}");
    assert!(!out.exists());
}

#[test]
fn the_report_names_the_applied_files() {
    let (base, tree) = exported("test_extension", "input.cfe", "load-applied");
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    let out = fresh_output("load-applied-out", "cfe");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(
        report["applied"],
        serde_json::json!(["CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"])
    );
    assert_eq!(report["warnings"], serde_json::json!([]));
}

#[test]
fn a_base_changed_since_the_export_is_named_in_a_warning() {
    // The tree was exported from `base`; the load goes onto `changed`, whose
    // module differs from the one the tree was exported with. The tree's text
    // replaces it -- the report says so for that module.
    let (base, tree) = exported("test_extension", "input.cfe", "load-drift");
    // CFE has no ConfigDumpInfo.xml; its export index records provenance.
    export_with_index(&base, &tree);
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let text = fs::read_to_string(&module).unwrap();
    let other = common::temp_dir("load-drift-other");
    assert!(common::export(&base, &other).status.success());
    let other_module = other.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    fs::write(&other_module, text.replace("\"1.0\"", "\"1.5\"")).unwrap();
    let changed = fresh_output("load-drift-base", "cfe");
    assert!(load(&other, &changed, &base).status.success());

    fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    let out = fresh_output("load-drift-out", "cfe");
    let run = load(&tree, &out, &changed);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    let warnings = report["warnings"].to_string();
    assert!(
        warnings.contains("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"),
        "{warnings}"
    );
}

#[test]
fn a_module_added_to_a_form_that_had_none_is_loaded() {
    // choice_list_dates' common form has no module: its Form/Module.bsl is an
    // added file, which used to be refused (4 of 253 corpus forms).
    let (base, tree) = exported("choice_list_dates", "input.cf", "load-new-form-module");
    let module = tree.join("CommonForms/ФормаДаты/Ext/Form/Module.bsl");
    assert!(!module.exists());
    fs::create_dir_all(module.parent().unwrap()).unwrap();
    fs::write(
        &module,
        "\u{feff}&НаКлиенте\r\nПроцедура Проверка()\r\n\tСообщить(\"модуль добавлен\");\r\nКонецПроцедуры\r\n",
    )
    .unwrap();
    let out = fresh_output("load-new-form-module-out", "cf");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let back = common::temp_dir("load-new-form-module-back");
    assert!(common::export(&out, &back).status.success());
    for tree in [&tree, &back] {
        let _ = fs::remove_file(tree.join("ConfigDumpInfo.xml"));
    }
    common::assert_tree_eq(&tree, &back);
}

fn export_with_index(base: &Path, tree: &Path) {
    let run = Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(base)
        .arg(tree)
        .args(["--source-version", "2.20", "--overwrite", "--index"])
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn an_indexed_tree_loads_without_exporting_the_base_again() {
    // `cf export --index` records the base and every file's digest; a load
    // onto that same base diffs against the index instead of exporting the
    // base again (minutes on a large configuration), and the result is the
    // one the full path writes. After the load the index follows the new
    // file, so the next edit of the same tree loads onto it the same way.
    let base = common::fixture("test_extension").join("input.cfe");
    let tree = common::temp_dir("load-index");
    export_with_index(&base, &tree);
    assert!(tree.join(".ibcmd/index.tsv").exists());
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();

    let fast = fresh_output("load-index-fast", "cfe");
    let run = load(&tree, &fast, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(report["base_export"], "index", "{report}");

    // The same edit through the full path: byte-identical output.
    let plain = common::temp_dir("load-index-plain");
    assert!(common::export(&base, &plain).status.success());
    let plain_module = plain.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    fs::write(&plain_module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    let full = fresh_output("load-index-full", "cfe");
    let run = load(&plain, &full, &base);
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(report["base_export"], "full", "{report}");
    assert_eq!(fs::read(&fast).unwrap(), fs::read(&full).unwrap());

    // Next edit of the indexed tree, onto the file just written.
    fs::write(&module, text.replace("\"1.0\"", "\"3.0\"")).unwrap();
    let next = fresh_output("load-index-next", "cfe");
    let run = load(&tree, &next, &fast);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(report["base_export"], "index", "{report}");
    assert_eq!(
        report["applied"],
        serde_json::json!(["CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"])
    );
    let back = common::temp_dir("load-index-back");
    assert!(common::export(&next, &back).status.success());
    let module_back =
        fs::read_to_string(back.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"))
            .unwrap();
    assert!(module_back.contains("\"3.0\""));
}

#[test]
fn an_index_of_another_base_falls_back_to_the_full_path() {
    let base = common::fixture("test_extension").join("input.cfe");
    let other = common::fixture("closed_module").join("input.cfe");
    let tree = common::temp_dir("load-index-other");
    export_with_index(&other, &tree);
    // The tree came from `other`; loaded onto `base`, the index says nothing.
    let module = tree.join("CommonModules/ТестРасширение_Модуль.xml");
    assert!(module.exists());
    let out = fresh_output("load-index-other-out", "cfe");
    let run = load(&tree, &out, &base);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(!text.contains("\"base_export\": \"index\""), "{text}");
}

#[test]
fn an_edit_carrying_an_older_modification_time_still_loads() {
    // Explorer, robocopy, `cp -p` and unpacked archives keep a file's old
    // date: an edited file older than the index is still an edit.
    let base = common::fixture("test_extension").join("input.cfe");
    let tree = common::temp_dir("load-old-mtime");
    export_with_index(&base, &tree);
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let exported = fs::metadata(&module).unwrap().modified().unwrap();
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    fs::File::options()
        .write(true)
        .open(&module)
        .unwrap()
        .set_modified(exported - std::time::Duration::from_secs(3600))
        .unwrap();

    let out = fresh_output("load-old-mtime-out", "cfe");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(
        report["applied"],
        serde_json::json!(["CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"])
    );
}

#[test]
fn a_tree_left_by_an_interrupted_update_is_refused() {
    // `cf export --update` marks the tree while it rewrites it; a tree that
    // still carries the mark mixes two files and its index names neither.
    let base = common::fixture("test_extension").join("input.cfe");
    let tree = common::temp_dir("load-interrupted");
    export_with_index(&base, &tree);
    fs::write(tree.join(".ibcmd/update-in-progress"), "").unwrap();
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    let out = fresh_output("load-interrupted-out", "cfe");
    let run = load(&tree, &out, &base);
    assert!(!run.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(text.contains("interrupted"), "{text}");
    assert!(!out.exists());
}

#[test]
fn a_form_saved_with_lf_line_breaks_loads() {
    // Editors and git on Linux and macOS save the exported CRLF tree with LF
    // (#471); structural indentation can use either line ending.
    let (base, tree) = exported("test_processor", "input.epf", "load-lf");
    let form = tree.join("ТестОбработка/Forms/Форма/Ext/Form.xml");
    let text = fs::read_to_string(&form).unwrap();
    assert!(text.contains("\r\n"));
    fs::write(&form, text.replace("\r\n", "\n")).unwrap();

    let out = fresh_output("load-lf-out", "epf");
    let run = load(&tree, &out, &base);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );

    let back = common::temp_dir("load-lf-back");
    assert!(common::export(&out, &back).status.success());
    fs::write(&form, text).unwrap();
    common::assert_tree_eq(&tree, &back);
}

fn run_8_5(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        .args(["--platform", "8.5.1"])
        .output()
        .expect("run ibcmd-rs")
}

/// The content of a subsystem, top-level or nested, is loaded (#417): the
/// patch of a subsystem's base row kept its old content, and a nested one
/// had no entry at all.
#[test]
fn an_edited_subsystem_content_loads_at_any_depth() {
    let base = common::fixture("nested_subsystems").join("input.cf");
    let module = "\t\t\t<Content>\r\n\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">CommonModule.ФайловаяБазаПроверка</xr:Item>\r\n\t\t\t</Content>\r\n";
    let empty = "\t\t\t<Content/>\r\n";
    for (tag, file, from, to) in [
        (
            "nested",
            "Subsystems/Верх/Subsystems/Низ.xml",
            module,
            empty,
        ),
        ("top", "Subsystems/Верх.xml", empty, module),
    ] {
        let tree = common::temp_dir(&format!("load-subsystem-{tag}"));
        let run = run_8_5(&[
            "cf".as_ref(),
            "export".as_ref(),
            base.as_os_str(),
            tree.as_os_str(),
        ]);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let path = tree.join(file);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{tag}: {text}");
        fs::write(&path, text.replace(from, to)).unwrap();

        let out = fresh_output(&format!("load-subsystem-{tag}-out"), "cf");
        let run = run_8_5(&[
            "cf".as_ref(),
            "load".as_ref(),
            tree.as_os_str(),
            out.as_os_str(),
            "--base".as_ref(),
            base.as_os_str(),
        ]);
        assert!(
            run.status.success(),
            "{tag}: {}{}",
            String::from_utf8_lossy(&run.stdout),
            String::from_utf8_lossy(&run.stderr)
        );
        let back = common::temp_dir(&format!("load-subsystem-{tag}-back"));
        let run = run_8_5(&[
            "cf".as_ref(),
            "export".as_ref(),
            out.as_os_str(),
            back.as_os_str(),
        ]);
        assert!(run.status.success());
        let edited = fs::read(tree.join(file)).unwrap();
        assert_eq!(fs::read(back.join(file)).unwrap(), edited, "{tag}");
    }
}

fn plain_row(packed: &[u8]) -> String {
    use std::io::Read;
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(packed)
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn row_generation(packed: &[u8], name: &str) -> String {
    let text = plain_row(packed);
    let marker = format!("\"{name}\",");
    let start = text.find(&marker).unwrap() + marker.len();
    let generation = text[start..].trim_start().get(..36).unwrap();
    uuid::Uuid::parse_str(generation).unwrap();
    generation.to_owned()
}

#[test]
fn a_recompiled_subsystem_updates_its_versions_entry() {
    let base = common::fixture("nested_subsystems").join("input.cf");
    let tree = common::temp_dir("load-subsystem-versions");
    let output = run_8_5(&[
        "cf".as_ref(),
        "export".as_ref(),
        base.as_os_str(),
        tree.as_os_str(),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let relative = "Subsystems/Верх/Subsystems/Низ.xml";
    let file = tree.join(relative);
    let xml = fs::read(&file).unwrap();
    let subsystem = ibcmd_rs::module_blob::parse_simple_metadata_xml_properties(&xml)
        .unwrap()
        .uuid;
    let module = ibcmd_rs::module_blob::parse_simple_metadata_xml_properties(
        &fs::read(tree.join("CommonModules/ФайловаяБазаПроверка.xml")).unwrap(),
    )
    .unwrap()
    .uuid;
    let text = String::from_utf8(xml).unwrap();
    let start = text.find("\t\t\t<Content>").unwrap();
    let end = text[start..].find("</Content>").unwrap() + start + "</Content>".len();
    let edited = format!("{}<Content/>{}", &text[..start + 3], &text[end..]);
    fs::write(file, edited).unwrap();
    let stored = packed_entries(&base);
    let compiled = ibcmd_rs::mssql::compile_source_rows_offline(
        &tree,
        stored.clone().into_iter().collect(),
        &[relative.to_owned()],
        &[],
    )
    .unwrap();
    assert_ne!(
        compiled
            .rows
            .iter()
            .find(|(id, _)| id == &subsystem)
            .unwrap()
            .1,
        stored[&subsystem]
    );
    let updated = compiled.versions.unwrap();
    assert_ne!(
        row_generation(&updated, &subsystem),
        row_generation(&stored["versions"], &subsystem)
    );
    assert_eq!(
        row_generation(&updated, &module),
        row_generation(&stored["versions"], &module)
    );
}

#[test]
fn an_adopted_subsystem_retains_its_stored_control_state() {
    use ibcmd_rs::extension::adoption::{header_of, parse};
    for fixture in ["props_b0", "props_b1", "props_b2", "props_all"] {
        let (base, tree) = exported(
            &format!("adopted/{fixture}"),
            "input.cfe",
            &format!("load-subsystem-adoption-{fixture}"),
        );
        let path = tree.join("Subsystems/Подсистема.xml");
        let bytes = fs::read(&path).unwrap();
        let uuid = ibcmd_rs::module_blob::parse_simple_metadata_xml_properties(&bytes)
            .unwrap()
            .uuid;
        let original = plain_row(&packed_entries(&base)[&uuid]);
        let adoption = parse(header_of(&original, &uuid).unwrap()).unwrap();
        assert!(adoption.adopted);
        let text = String::from_utf8(bytes).unwrap();
        let edited = text.replace("<Comment/>", "<Comment>edited subsystem comment</Comment>");
        assert_ne!(text, edited);
        fs::write(&path, edited).unwrap();
        let out = fresh_output(&format!("load-subsystem-adoption-{fixture}-out"), "cfe");
        let output = load(&tree, &out, &base);
        assert!(
            output.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let loaded = plain_row(&packed_entries(&out)[&uuid]);
        assert_eq!(
            parse(header_of(&loaded, &uuid).unwrap()).unwrap(),
            adoption,
            "{fixture}"
        );
        let back = common::temp_dir(&format!("load-subsystem-adoption-{fixture}-back"));
        assert!(common::export(&out, &back).status.success());
        common::assert_tree_eq(&tree, &back);
    }
}
