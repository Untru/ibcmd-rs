//! `cf load` of edits a module overlay cannot carry -- `Form.xml`, metadata
//! XML, templates, added and removed objects: the edited objects are compiled
//! from the tree against the base's own rows (the staging compiler, offline).
//! The loaded file exports the edited tree with native BSL source-file framing.

use crate::common;

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

/// Loads `tree` onto `base` and checks the result exports back to `tree`.
fn assert_loads_back(tree: &Path, base: &Path, tag: &str, ext: &str) -> serde_json::Value {
    load_back(tree, base, tag, ext, false).0
}

/// [`assert_loads_back`] for a tree whose objects came or went: its
/// ConfigDumpInfo.xml is stale, so it is left out; the exported one is
/// returned for the caller to check.
fn assert_loads_back_objects(
    tree: &Path,
    base: &Path,
    tag: &str,
    ext: &str,
) -> (serde_json::Value, String) {
    let (report, dump_info, _) = load_back(tree, base, tag, ext, true);
    (report, dump_info)
}

fn load_back(
    tree: &Path,
    base: &Path,
    tag: &str,
    ext: &str,
    objects_changed: bool,
) -> (serde_json::Value, String, common::Scratch) {
    let out = fresh_output(&format!("{tag}-out"), ext);
    let run = load(tree, &out, base);
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let back = common::temp_dir(&format!("{tag}-back"));
    let exported = common::export(&out, &back);
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    if objects_changed {
        let _ = fs::remove_file(tree.join("ConfigDumpInfo.xml"));
    }
    let dump_info = fs::read_to_string(back.join("ConfigDumpInfo.xml")).unwrap_or_default();
    if ext == "cfe" {
        // Native file-CFE export has no database dump-info document. The
        // extension test below validates every new configinfo SHA-1 instead.
        assert!(!back.join("ConfigDumpInfo.xml").exists());
    } else if objects_changed {
        fs::remove_file(back.join("ConfigDumpInfo.xml")).unwrap();
    }
    assert_same_but_versions(tree, &back);
    (serde_json::from_slice(&run.stdout).unwrap(), dump_info, out)
}

/// The trees are equal but for the configVersion values of
/// ConfigDumpInfo.xml: a compiled entry gets a new generation. The tree's
/// ConfigDumpInfo.xml is derived and goes stale when objects come or go;
/// `assert_loads_back_objects` checks those against the file.
fn assert_same_but_versions(expected: &Path, actual: &Path) {
    let strip = |files: std::collections::BTreeMap<String, Vec<u8>>| {
        files
            .into_iter()
            .filter(|(path, _)| !path.starts_with(".ibcmd/"))
            .map(|(path, bytes)| {
                if path != "ConfigDumpInfo.xml" {
                    return (path, bytes);
                }
                let text = String::from_utf8(bytes).unwrap();
                let mut out = String::new();
                let mut rest = text.as_str();
                while let Some(at) = rest.find("configVersion=\"") {
                    out.push_str(&rest[..at]);
                    let tail = &rest[at + 15..];
                    rest = &tail[tail.find('"').unwrap() + 1..];
                }
                out.push_str(rest);
                (path, out.into_bytes())
            })
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let (left, right) = (strip(common::files(expected)), strip(common::files(actual)));
    let differ = left
        .keys()
        .chain(right.keys())
        .filter(|path| left.get(*path) != right.get(*path))
        .collect::<std::collections::BTreeSet<_>>();
    assert!(differ.is_empty(), "differ: {differ:?}");
}

fn edit(path: &Path, from: &str, to: &str) {
    let text = fs::read_to_string(path).unwrap();
    assert!(text.contains(from), "{} lacks {from}", path.display());
    fs::write(path, text.replacen(from, to, 1)).unwrap();
}

#[test]
fn a_form_xml_edit_of_a_configuration_loads() {
    let (base, tree) = exported("choice_list_dates", "input.cf", "compiled-form");
    edit(
        &tree.join("CommonForms/ФормаДаты/Ext/Form.xml"),
        "2024-01-01T00:00:00",
        "2025-02-03T04:05:06",
    );
    let report = assert_loads_back(&tree, &base, "compiled-form", "cf");
    assert_eq!(
        report["compiled_objects"],
        serde_json::json!(["CommonForms/ФормаДаты"])
    );
}

#[test]
fn a_metadata_property_edit_of_a_configuration_loads() {
    let (base, tree) = exported("choice_list_dates", "input.cf", "compiled-synonym");
    edit(
        &tree.join("CommonForms/ФормаДаты.xml"),
        "<Synonym/>",
        "<Synonym>\r\n\t\t\t\t<v8:item>\r\n\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n\t\t\t\t\t<v8:content>Даты</v8:content>\r\n\t\t\t\t</v8:item>\r\n\t\t\t</Synonym>",
    );
    assert_loads_back(&tree, &base, "compiled-synonym", "cf");
}

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

#[test]
fn an_object_s_unchanged_bodies_stay_byte_for_byte() {
    // Only the form's metadata changes: its body entry is the base's, not a
    // recompiled one (a recompile may lose what the compiler cannot read).
    let (base, tree) = exported("choice_list_dates", "input.cf", "compiled-kept");
    edit(
        &tree.join("CommonForms/ФормаДаты.xml"),
        "<Comment/>",
        "<Comment>x</Comment>",
    );
    let (_, _, out) = load_back(&tree, &base, "compiled-kept", "cf", false);
    let (before, after) = (packed_entries(&base), packed_entries(&out));
    let body = "5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a21.0";
    assert_eq!(before[body], after[body]);
    assert_ne!(
        before["5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a21"],
        after["5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a21"]
    );
}

#[test]
fn an_added_common_module_of_a_configuration_loads() {
    let (base, tree) = exported("choice_list_dates", "input.cf", "compiled-added");
    let template = fs::read_to_string(
        common::fixture("test_extension").join("expected/CommonModules/ТестРасширение_Модуль.xml"),
    )
    .unwrap();
    let module = template
        .replace(
            "554f39c6-e029-4ee0-8535-8c15291a6a3f",
            "0a1b2c3d-4e5f-4a6b-8c7d-8e9f0a1b2c3d",
        )
        .replace("ТестРасширение_Модуль", "НовыйМодуль");
    fs::create_dir_all(tree.join("CommonModules/НовыйМодуль/Ext")).unwrap();
    fs::write(tree.join("CommonModules/НовыйМодуль.xml"), module).unwrap();
    fs::write(
        tree.join("CommonModules/НовыйМодуль/Ext/Module.bsl"),
        "\u{feff}Процедура Новая() Экспорт\r\nКонецПроцедуры\r\n",
    )
    .unwrap();
    edit(
        &tree.join("Configuration.xml"),
        "\t\t\t<CommonForm>ФормаДаты</CommonForm>",
        "\t\t\t<CommonModule>НовыйМодуль</CommonModule>\r\n\t\t\t<CommonForm>ФормаДаты</CommonForm>",
    );
    let (report, dump_info) = assert_loads_back_objects(&tree, &base, "compiled-added", "cf");
    assert!(
        dump_info.contains("id=\"0a1b2c3d-4e5f-4a6b-8c7d-8e9f0a1b2c3d.0\""),
        "{dump_info}"
    );
    assert_eq!(
        report["compiled_objects"],
        serde_json::json!(["CommonModules/НовыйМодуль", "Configuration.xml"])
    );
}

#[test]
fn a_removed_common_form_of_a_configuration_loads() {
    let (base, tree) = exported("choice_list_dates", "input.cf", "compiled-removed");
    fs::remove_file(tree.join("CommonForms/ФормаДаты.xml")).unwrap();
    fs::remove_dir_all(tree.join("CommonForms")).unwrap();
    edit(
        &tree.join("Configuration.xml"),
        "\r\n\t\t\t<CommonForm>ФормаДаты</CommonForm>",
        "",
    );
    let (report, dump_info, out) = load_back(&tree, &base, "compiled-removed", "cf", true);
    assert!(!dump_info.contains("5b6f3a52"), "{dump_info}");
    assert_eq!(
        report["removed_objects"],
        serde_json::json!(["CommonForms/ФормаДаты"])
    );
    assert!(
        !packed_entries(&out)
            .keys()
            .any(|key| key.starts_with("5b6f3a52-0c0e-4d0b-9d49-3f7f2f8e1a21"))
    );
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

/// The configinfo's `"<entry>",<digest>` pairs.
fn configinfo_digests(
    entries: &std::collections::BTreeMap<String, Vec<u8>>,
) -> std::collections::BTreeMap<String, String> {
    use std::io::Read;
    let mut text = String::new();
    flate2::read::DeflateDecoder::new(entries["configinfo"].as_slice())
        .read_to_string(&mut text)
        .unwrap();
    let block = &text[text.rfind("\n{").unwrap() + 2..];
    let fields = block
        .trim_end()
        .trim_end_matches('}')
        .split(',')
        .skip(1)
        .collect::<Vec<_>>();
    fields
        .chunks(2)
        .map(|pair| {
            (
                pair[0].trim().trim_matches('"').to_owned(),
                pair[1].trim().to_owned(),
            )
        })
        .collect()
}

fn sha1_base64(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    base64(&Sha1::digest(bytes))
}

#[test]
fn an_extension_metadata_edit_loads_with_its_configinfo_digest() {
    let (base, tree) = exported("test_extension", "input.cfe", "compiled-ext");
    edit(
        &tree.join("CommonModules/ТестРасширение_Модуль.xml"),
        "<Comment/>",
        "<Comment>правка</Comment>",
    );
    let (_, _, out) = load_back(&tree, &base, "compiled-ext", "cfe", false);
    let entries = packed_entries(&out);
    let digests = configinfo_digests(&entries);
    let key = "554f39c6-e029-4ee0-8535-8c15291a6a3f";
    assert_eq!(digests[key], sha1_base64(&entries[key]));
    assert_ne!(packed_entries(&base)[key], entries[key]);
}

#[test]
fn an_added_common_module_of_an_extension_is_listed_in_configinfo() {
    let (base, tree) = exported("test_extension", "input.cfe", "compiled-ext-added");
    let module = fs::read_to_string(tree.join("CommonModules/ТестРасширение_Модуль.xml"))
        .unwrap()
        .replace(
            "554f39c6-e029-4ee0-8535-8c15291a6a3f",
            "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d",
        )
        .replace("ТестРасширение_Модуль", "ТестРасширение_Второй");
    fs::create_dir_all(tree.join("CommonModules/ТестРасширение_Второй/Ext")).unwrap();
    fs::write(tree.join("CommonModules/ТестРасширение_Второй.xml"), module).unwrap();
    fs::write(
        tree.join("CommonModules/ТестРасширение_Второй/Ext/Module.bsl"),
        "\u{feff}Процедура Вторая() Экспорт\r\nКонецПроцедуры\r\n",
    )
    .unwrap();
    edit(
        &tree.join("Configuration.xml"),
        "<CommonModule>ТестРасширение_Модуль</CommonModule>",
        "<CommonModule>ТестРасширение_Модуль</CommonModule>\r\n\t\t\t<CommonModule>ТестРасширение_Второй</CommonModule>",
    );
    let (_, _, out) = load_back(&tree, &base, "compiled-ext-added", "cfe", true);
    let entries = packed_entries(&out);
    let digests = configinfo_digests(&entries);
    for key in [
        "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d",
        "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d.0",
    ] {
        assert_eq!(digests.get(key), Some(&sha1_base64(&entries[key])), "{key}");
    }
    assert_eq!(digests.len(), entries.len() - 1);
}

/// A copy of the platform's dump of an edited tree
/// (`_onecdec/make_edit_fixtures.py`).
fn platform_edited_tree(case: &str, tag: &str) -> common::Scratch {
    let tree = common::temp_dir(tag);
    let source = common::fixture("edits").join(case).join("tree");
    for (path, bytes) in common::files(&source) {
        let target = tree.join(&path);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    tree
}

#[test]
#[ignore = "children added to an adopted object need its full record; refused for now"]
fn an_own_attribute_added_to_an_adopted_document_loads() {
    let base = common::fixture("adopted/document_children").join("input.cfe");
    let tree = platform_edited_tree("ext_own_attribute", "compiled-own-attr");
    let (report, _) = assert_loads_back_objects(&tree, &base, "compiled-own-attr", "cfe");
    assert_eq!(
        report["compiled_objects"],
        serde_json::json!(["Documents/Документ"])
    );
}

#[test]
fn an_own_attribute_added_to_an_adopted_document_is_refused_by_name() {
    let base = common::fixture("adopted/document_children").join("input.cfe");
    let tree = platform_edited_tree("ext_own_attribute", "compiled-own-attr-refused");
    let out = fresh_output("compiled-own-attr-refused-out", "cfe");
    let run = load(&tree, &out, &base);
    assert!(!run.status.success());
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains("Documents/Документ.xml"), "{stderr}");
    assert!(stderr.contains("adopted object"), "{stderr}");
    assert!(!out.exists());
}

#[test]
fn a_root_property_edit_of_an_external_data_processor_loads() {
    let (base, tree) = exported("test_processor", "input.epf", "compiled-epf-root");
    edit(
        &tree.join("ТестОбработка.xml"),
        "<Comment/>",
        "<Comment>правка</Comment>",
    );
    let report = assert_loads_back(&tree, &base, "compiled-epf-root", "epf");
    assert_eq!(report["applied"], serde_json::json!(["ТестОбработка.xml"]));
}

#[test]
fn a_form_xml_edit_of_an_external_data_processor_loads() {
    let (base, tree) = exported("test_processor", "input.epf", "compiled-epf-form");
    edit(
        &tree.join("ТестОбработка/Forms/Форма/Ext/Form.xml"),
        "\t<Attributes>",
        "\t<Events>\r\n\t\t<Event name=\"OnOpen\">ПриОткрытии</Event>\r\n\t</Events>\r\n\t<Attributes>",
    );
    assert_loads_back(&tree, &base, "compiled-epf-form", "epf");
}

#[test]
fn a_root_property_edit_of_an_external_report_loads() {
    let (base, tree) = exported("test_report", "input.erf", "compiled-erf-root");
    edit(
        &tree.join("ТестОтчет.xml"),
        "<Comment/>",
        "<Comment>правка</Comment>",
    );
    assert_loads_back(&tree, &base, "compiled-erf-root", "erf");
}

#[test]
fn an_adopted_form_s_interceptor_edit_loads() {
    let (base, tree) = exported("adopted/form_events", "input.cfe", "compiled-adopted-form");
    edit(
        &tree.join("Catalogs/Справочник/Forms/ФормаЭлемента/Ext/Form.xml"),
        "<Event name=\"OnOpen\" callType=\"Before\">Расш_ПриОткрытииПеред</Event>",
        "<Event name=\"OnOpen\" callType=\"Override\">Расш_ПриОткрытииПеред</Event>",
    );
    assert_loads_back(&tree, &base, "compiled-adopted-form", "cfe");
}

/// Both module-only overlay and descriptor compilation must reopen a closed
/// module with fresh metadata, rather than carrying its binary-only frame.
#[test]
fn closed_module_replacement_is_fresh_on_both_load_routes() {
    for compiled in [false, true] {
        let tag = format!("closed-replacement-{compiled}");
        let (base, tree) = exported("closed_module", "input.cfe", &tag);
        let module = tree.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bin");
        if std::env::var_os("IBCMD_TEST_KEEP_FIXTURES").as_deref()
            == Some(std::ffi::OsStr::new("1"))
        {
            let retained = common::temp_dir(&format!("{tag}-closed-input")).with_extension("bin");
            fs::rename(&module, retained).unwrap();
        } else {
            fs::remove_file(&module).unwrap();
        }
        let text = b"Procedure Reopened()\r\nEndProcedure\r\n";
        fs::write(module.with_extension("bsl"), text).unwrap();
        if compiled {
            edit(
                &tree.join("Configuration.xml"),
                "<Version/>",
                "<Version>1.0.0.1</Version>",
            );
        }
        let out = fresh_output(&format!("{tag}-out"), "cfe");
        let run = load(&tree, &out, &base);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&run.stdout).unwrap();
        assert_eq!(
            report["compiled_objects"]
                .as_array()
                .is_some_and(|v| !v.is_empty()),
            compiled
        );
        let archive = ibcmd_v8::format15::parse(&fs::read(&out).unwrap()).unwrap();
        let packed = archive
            .elements
            .into_iter()
            .find(|e| e.name == "554f39c6-e029-4ee0-8535-8c15291a6a3f.0")
            .unwrap()
            .data
            .unwrap();
        let fresh = ibcmd_rs::module_blob::pack_module_blob_bytes(text, None, None).unwrap();
        assert_eq!(
            packed, fresh.blob,
            "closed info/header survived route {compiled}"
        );
        let back = common::temp_dir(&format!("{tag}-back"));
        assert!(common::export(&out, &back).status.success());
        assert_eq!(
            fs::read(back.join("CommonModules/ТестРасширение_Модуль/Ext/Module.bsl")).unwrap(),
            b"\xef\xbb\xbfProcedure Reopened()\r\nEndProcedure\r\n"
        );
    }
}
