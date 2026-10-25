//! `cf export --update`: an indexed tree brought up to a newer version of the
//! file by rewriting only what changed. The acceptance check is always the
//! same: the tree ends exactly as a full export of the newer file.

use crate::common;

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(args)
        .output()
        .unwrap()
}

fn export_indexed(input: &Path, tree: &Path) {
    let out = run(&[
        "cf".as_ref(),
        "export".as_ref(),
        input.as_os_str(),
        tree.as_os_str(),
        "--overwrite".as_ref(),
        "--index".as_ref(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn update(input: &Path, tree: &Path) -> serde_json::Value {
    let out = run(&[
        "cf".as_ref(),
        "export".as_ref(),
        input.as_os_str(),
        tree.as_os_str(),
        "--update".as_ref(),
    ]);
    let body = if out.stdout.is_empty() {
        &out.stderr
    } else {
        &out.stdout
    };
    serde_json::from_slice(body).unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(body)))
}

fn update_status(input: &Path, tree: &Path) -> (bool, serde_json::Value) {
    let out = run(&[
        "cf".as_ref(),
        "export".as_ref(),
        input.as_os_str(),
        tree.as_os_str(),
        "--update".as_ref(),
    ]);
    let body = if out.stdout.is_empty() {
        &out.stderr
    } else {
        &out.stdout
    };
    let report = serde_json::from_slice(body)
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(body)));
    (out.status.success(), report)
}

fn load(tree: &Path, output: &Path, base: &Path) {
    let out = run(&[
        "cf".as_ref(),
        "load".as_ref(),
        tree.as_os_str(),
        output.as_os_str(),
        "--base".as_ref(),
        base.as_os_str(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A newer version of `base`: `edit` applied to an export of it, loaded.
fn newer(base: &Path, tag: &str, edit: impl FnOnce(&Path)) -> PathBuf {
    let tree = common::temp_dir(&format!("{tag}-edit"));
    assert!(common::export(base, &tree).status.success());
    edit(&tree);
    let output = common::temp_dir(&format!("{tag}-newer")).with_extension("bin");
    let _ = fs::remove_file(&output);
    load(&tree, &output, base);
    output
}

fn assert_like_full_export(tree: &Path, input: &Path, tag: &str) {
    let full = common::temp_dir(&format!("{tag}-full"));
    assert!(common::export(input, &full).status.success());
    let _ = fs::remove_dir_all(tree.join(".ibcmd").join("staging"));
    let mut ours = common::files(tree);
    ours.retain(|path, _| !path.starts_with(".ibcmd/"));
    let expected = common::files(&full);
    let differ = expected
        .keys()
        .filter(|path| ours.get(*path) != expected.get(*path))
        .collect::<Vec<_>>();
    let extra = ours
        .keys()
        .filter(|path| !expected.contains_key(*path))
        .collect::<Vec<_>>();
    assert!(
        differ.is_empty() && extra.is_empty(),
        "{tag}: differ {differ:?} extra {extra:?}"
    );
}

#[test]
fn an_extension_module_changed_is_rewritten_alone() {
    let base = common::fixture("test_extension").join("input.cfe");
    let newer = newer(&base, "update-cfe", |tree| {
        let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
        let text = fs::read_to_string(&module).unwrap();
        fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    });
    let tree = common::temp_dir("update-cfe-tree");
    export_indexed(&base, &tree);
    let report = update(&newer.with_extension("bin"), &tree);
    assert_eq!(report["update"]["mode"], "incremental", "{report}");
    assert_eq!(
        report["update"]["rewritten"],
        serde_json::json!(["CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"])
    );
    assert!(!tree.join("ConfigDumpInfo.xml").exists());
    assert_like_full_export(&tree, &newer, "update-cfe");
    // The index now names the newer file: a load onto it takes the index.
    let again = update(&newer, &tree);
    assert_eq!(again["update"]["mode"], "unchanged", "{again}");
}

#[test]
fn a_configuration_form_body_changed_is_rewritten_alone() {
    // The common form had no module; its form body is the changed entry.
    let base = common::fixture("choice_list_dates").join("input.cf");
    let newer = newer(&base, "update-cf", |tree| {
        let module = tree.join("CommonForms/ФормаДаты/Ext/Form/Module.bsl");
        fs::create_dir_all(module.parent().unwrap()).unwrap();
        fs::write(
            &module,
            "\u{feff}&НаКлиенте\r\nПроцедура Проверка()\r\nКонецПроцедуры\r\n",
        )
        .unwrap();
    });
    let tree = common::temp_dir("update-cf-tree");
    export_indexed(&base, &tree);
    let report = update(&newer, &tree);
    assert_eq!(report["update"]["mode"], "incremental", "{report}");
    assert_like_full_export(&tree, &newer, "update-cf");
}

#[test]
fn a_tree_without_an_index_is_refused_and_left_alone() {
    // Without an index nothing tells the tree's own edits from the export;
    // a full export over it would wipe them, and a `.git` beside them.
    let base = common::fixture("choice_list_dates").join("input.cf");
    let tree = common::temp_dir("update-noindex-tree");
    assert!(common::export(&base, &tree).status.success());
    fs::create_dir_all(tree.join(".git")).unwrap();
    fs::write(tree.join(".git/config"), "[core]").unwrap();
    let before = common::files(&tree);
    let (status, report) = update_status(&base, &tree);
    assert!(!status, "{report}");
    assert_eq!(report["ok"], false, "{report}");
    assert!(report.to_string().contains("--index"), "{report}");
    assert_eq!(common::files(&tree), before);
}

#[test]
fn an_update_into_a_new_directory_exports_in_full() {
    let base = common::fixture("choice_list_dates").join("input.cf");
    let tree = common::temp_dir("update-new-tree");
    let report = update(&base, &tree);
    assert_eq!(report["update"]["mode"], "full", "{report}");
    assert!(tree.join(".ibcmd/index.tsv").exists());
    assert_like_full_export(&tree, &base, "update-new");
}

#[test]
fn a_tree_with_local_edits_is_refused_and_left_alone() {
    // An edit not loaded yet would be overwritten (or, left in place, lost
    // to the index): the update names it and changes nothing.
    let base = common::fixture("test_extension").join("input.cfe");
    let newer = newer(&base, "update-dirty", |tree| {
        let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
        let text = fs::read_to_string(&module).unwrap();
        fs::write(&module, text.replace("\"1.0\"", "\"2.0\"")).unwrap();
    });
    let tree = common::temp_dir("update-dirty-tree");
    export_indexed(&base, &tree);
    let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
    let text = fs::read_to_string(&module).unwrap();
    fs::write(&module, text.replace("\"1.0\"", "\"локально\"")).unwrap();
    let before = common::files(&tree);
    let (status, report) = update_status(&newer, &tree);
    assert!(!status, "{report}");
    assert!(
        report
            .to_string()
            .contains("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"),
        "{report}"
    );
    assert_eq!(common::files(&tree), before);
}

#[test]
fn a_full_update_keeps_the_trees_own_files() {
    // A changed metadata row takes a full export: it replaces the files the
    // export wrote and leaves everything else (a `.git`) where it was.
    let base = common::fixture("choice_list_dates").join("input.cf");
    let newer = newer(&base, "update-full", |tree| {
        let form = tree.join("CommonForms/ФормаДаты.xml");
        let text = fs::read_to_string(&form).unwrap();
        assert!(text.contains("<Comment/>"), "{text}");
        fs::write(
            &form,
            text.replacen("<Comment/>", "<Comment>правка</Comment>", 1),
        )
        .unwrap();
    });
    let tree = common::temp_dir("update-full-tree");
    export_indexed(&base, &tree);
    fs::create_dir_all(tree.join(".git")).unwrap();
    fs::write(tree.join(".git/config"), "[core]").unwrap();
    let report = update(&newer, &tree);
    assert_eq!(report["update"]["mode"], "full", "{report}");
    assert_eq!(report["ok"], true, "{report}");
    assert_eq!(
        fs::read_to_string(tree.join(".git/config")).unwrap(),
        "[core]"
    );
    // The comparison walks every file; the `.git` has been checked above.
    fs::remove_dir_all(tree.join(".git")).unwrap();
    assert_like_full_export(&tree, &newer, "update-full");
    // The index follows the newer file.
    let again = update(&newer, &tree);
    assert_eq!(again["update"]["mode"], "unchanged", "{again}");
}

#[test]
fn an_interrupted_update_is_refused() {
    let base = common::fixture("choice_list_dates").join("input.cf");
    let tree = common::temp_dir("update-interrupted-tree");
    export_indexed(&base, &tree);
    fs::write(tree.join(".ibcmd/update-in-progress"), "").unwrap();
    let (status, report) = update_status(&base, &tree);
    assert!(!status, "{report}");
    assert!(report.to_string().contains("interrupted"), "{report}");
}

#[test]
fn a_tree_without_config_dump_info_updates_from_the_index_versions() {
    // Native file-CFE export has no ConfigDumpInfo.xml; the index keeps
    // the versions, so the update stays incremental.
    let base = common::fixture("test_extension").join("input.cfe");
    let newer = newer(&base, "update-nodump", |tree| {
        let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl");
        let text = fs::read_to_string(&module).unwrap();
        fs::write(&module, text.replace("\"1.0\"", "\"3.0\"")).unwrap();
    });
    let tree = common::temp_dir("update-nodump-tree");
    export_indexed(&base, &tree);
    assert!(!tree.join("ConfigDumpInfo.xml").exists());
    let report = update(&newer, &tree);
    assert_eq!(report["update"]["mode"], "incremental", "{report}");
    assert_eq!(
        report["update"]["rewritten"],
        serde_json::json!(["CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"])
    );
    assert!(!tree.join("ConfigDumpInfo.xml").exists());
    assert!(
        fs::read_to_string(tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl"))
            .unwrap()
            .contains("\"3.0\"")
    );
}

#[test]
fn the_file_the_index_names_is_answered_without_an_export() {
    let base = common::fixture("choice_list_dates").join("input.cf");
    let tree = common::temp_dir("update-current-tree");
    export_indexed(&base, &tree);
    let report = update(&base, &tree);
    assert_eq!(report["update"]["mode"], "unchanged", "{report}");
    assert!(report["export"].is_null(), "{report}");
}
