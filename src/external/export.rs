//! `cf export` of an external object: adapt the entries, run the
//! configuration pipeline, then move and rename its output.

use std::{fs, path::Path};

use anyhow::{Context, Result};
use ibcmd_cf::{
    archive::PackedCfArchive,
    export::{StorageExportDisposition, StorageExportEntryReport},
};

use crate::{
    legacy_version::InfobaseConfigSourceVersion,
    module_blob::{deflate_raw, inflate_raw},
    mssql_dump::{self, StorageImageSourceExportReport},
};

use super::{
    brace, copyinfo,
    header::{self, ExternalMain},
    rename, root_xml, versions,
};

fn text_of(payload: &[u8]) -> Result<String> {
    String::from_utf8(inflate_raw(payload)?).context("entry is not UTF-8 text")
}

pub fn entries_of(archive: &PackedCfArchive) -> Vec<(String, Vec<u8>)> {
    archive
        .entries()
        .iter()
        .map(|entry| (entry.name().to_owned(), entry.payload().to_vec()))
        .collect()
}

/// `Some(main)` when `root` names a main row wrapped as an external object.
pub fn detect(entries: &[(String, Vec<u8>)]) -> Result<Option<ExternalMain>> {
    detect_by(|wanted| {
        entries
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(wanted))
            .map(|(_, payload)| payload.as_slice())
    })
}

/// [`detect`] on an open archive: only `root` and the main row are read,
/// nothing is copied (every `cf export` asks, a configuration runs to GBs).
pub fn detect_in_archive(archive: &PackedCfArchive) -> Result<Option<ExternalMain>> {
    detect_by(|wanted| {
        archive
            .entries()
            .iter()
            .find(|entry| entry.name().eq_ignore_ascii_case(wanted))
            .map(|entry| entry.payload())
    })
}

fn detect_by<'a>(find: impl Fn(&str) -> Option<&'a [u8]>) -> Result<Option<ExternalMain>> {
    let Some(root) = find("root") else {
        return Ok(None);
    };
    // Rows that do not decode are the configuration pipeline's business.
    let Ok(root) = text_of(root) else {
        return Ok(None);
    };
    let Some((f, _)) = brace::fields(brace::strip_bom(&root).trim(), 0) else {
        return Ok(None);
    };
    // A configuration root carries a base64 hash in the third field.
    if f.first() != Some(&"2") || f.get(2).is_some_and(|hash| !hash.is_empty()) {
        return Ok(None);
    }
    let Some(main_uuid) = f.get(1) else {
        return Ok(None);
    };
    let Some(main) = find(main_uuid) else {
        return Ok(None);
    };
    let Ok(main_text) = text_of(main) else {
        return Ok(None);
    };
    header::parse_main(main_uuid, &main_text)
}

/// The platform's `AnyIBRef` type id, which a dump made in the configuration
/// infobase writes `cfg:AnyRef` for an external object (corpus: 17 of 17;
/// only an empty infobase keeps `AnyIBRef`).
const ANY_REF_TYPE_ID: &str = "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63";

/// Configuration references an external object carries in `copyinfo`, in the
/// spelling the pipeline indexes use.
pub fn foreign_references(
    info: &copyinfo::CopyInfo,
    main: &ExternalMain,
) -> mssql_dump::ForeignReferences {
    let mut types: Vec<(String, String, String)> = copyinfo::resolve(info)
        .into_iter()
        .map(|t| (t.type_id, format!("cfg:{}", t.name), t.category.to_owned()))
        .collect();
    types.push((
        ANY_REF_TYPE_ID.into(),
        "cfg:AnyRef".into(),
        "TypeSet".into(),
    ));
    let mut objects = copyinfo::object_references(info);
    // Help links name the object itself by its main uuid, which the adapter
    // moved to the object id; renamed to the external spelling on output.
    objects
        .entry(main.main_uuid.clone())
        .or_insert_with(|| format!("{}.{}", main.kind.internal_kind(), main.name));
    mssql_dump::ForeignReferences { objects, types }
}

/// An external object's entries as the configuration pipeline reads them:
/// the main row rewritten into the object's `DataProcessor`/`Report` row
/// (under its object id), `versions` naming it so, `copyinfo` read into the
/// references it resolves.
pub struct Adapted {
    pub main: ExternalMain,
    pub entries: Vec<(String, Vec<u8>)>,
    pub foreign: mssql_dump::ForeignReferences,
}

/// `None` for entries that are not an external object's.
pub fn adapt(entries: Vec<(String, Vec<u8>)>) -> Result<Option<Adapted>> {
    let Some(main) = detect(&entries)? else {
        return Ok(None);
    };
    let mut adapted = Vec::with_capacity(entries.len());
    let mut foreign = foreign_references(&copyinfo::CopyInfo::default(), &main);
    for (name, payload) in entries {
        if name.eq_ignore_ascii_case(&main.main_uuid) {
            let row = header::internal_row_text(&main)?;
            adapted.push((main.object_id.clone(), deflate_raw(row.as_bytes())?));
        } else if name == "copyinfo" {
            foreign = foreign_references(&copyinfo::parse(&text_of(&payload)?)?, &main);
        } else if name == "versions" {
            let text = versions::rewrite(
                &text_of(&payload)?,
                &[(main.main_uuid.as_str(), main.object_id.as_str())],
                &["copyinfo"],
                &[],
            )?;
            adapted.push((name, deflate_raw(text.as_bytes())?));
        } else {
            adapted.push((name, payload));
        }
    }
    Ok(Some(Adapted {
        main,
        entries: adapted,
        foreign,
    }))
}

pub fn export_if_external(
    archive: &PackedCfArchive,
    output_dir: &Path,
    overwrite: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<Option<StorageImageSourceExportReport>> {
    let Some(main) = detect_in_archive(archive)? else {
        return Ok(None);
    };
    let entries = entries_of(archive);

    // The report speaks of the entries as stored; the payloads themselves
    // move into the adapted set rather than being copied a second time.
    let stored = entries
        .iter()
        .map(|(name, payload)| (name.clone(), payload.len()))
        .collect::<Vec<_>>();
    let mut adapted = Vec::with_capacity(entries.len());
    let mut foreign = foreign_references(&copyinfo::CopyInfo::default(), &main);
    for (name, payload) in entries {
        if name.eq_ignore_ascii_case(&main.main_uuid) {
            let row = header::internal_row_text(&main)?;
            adapted.push((main.object_id.clone(), deflate_raw(row.as_bytes())?));
        } else if name == "copyinfo" {
            foreign = foreign_references(&copyinfo::parse(&text_of(&payload)?)?, &main);
        } else if name == "versions" {
            let text = versions::rewrite(
                &text_of(&payload)?,
                &[(main.main_uuid.as_str(), main.object_id.as_str())],
                &["copyinfo"],
                &[],
            )?;
            adapted.push((name, deflate_raw(text.as_bytes())?));
        } else {
            adapted.push((name, payload));
        }
    }

    let mut report = mssql_dump::export_packed_entries_to_source(
        archive.source_profile().as_str(),
        adapted,
        output_dir,
        overwrite,
        source_version,
        Some(&foreign),
    )?;
    finish(output_dir, &main, &mut report, &stored)?;
    Ok(Some(report))
}

fn finish(
    output_dir: &Path,
    main: &ExternalMain,
    report: &mut StorageImageSourceExportReport,
    stored: &[(String, usize)],
) -> Result<()> {
    let folder = output_dir.join(main.kind.internal_folder());
    let name = &main.name;
    // Native .epf/.erf dumps carry no ConfigDumpInfo.xml. Removed first: an
    // object may be named ConfigDumpInfo, and its root XML goes there.
    let dump_info = output_dir.join("ConfigDumpInfo.xml");
    if dump_info.exists() {
        fs::remove_file(dump_info)?;
    }
    // The pipeline's folder is parked under a name no object can have (1C
    // names start with a letter): an object named like it (`DataProcessors`)
    // would otherwise be moved onto it, clearing the whole export first.
    let parked = output_dir.join(".ibcmd-external");
    if parked.exists() {
        fs::remove_dir_all(&parked)
            .with_context(|| format!("failed to clear {}", parked.display()))?;
    }
    rename_directory(&folder, &parked)
        .with_context(|| format!("failed to move {}", folder.display()))?;
    let (from_dir, to_dir) = (parked.join(name), output_dir.join(name));
    let from_xml = parked.join(format!("{name}.xml"));
    let to_xml = output_dir.join(format!("{name}.xml"));
    if to_dir.exists() {
        fs::remove_dir_all(&to_dir)
            .with_context(|| format!("failed to clear {}", to_dir.display()))?;
    }
    if from_dir.exists() {
        rename_directory(&from_dir, &to_dir)
            .with_context(|| format!("failed to move {}", from_dir.display()))?;
    }
    let root = fs::read_to_string(&from_xml)
        .with_context(|| format!("pipeline wrote no root XML {}", from_xml.display()))?;
    fs::write(&to_xml, root_xml::to_external_root(&root, main)?)
        .with_context(|| format!("failed to write {}", to_xml.display()))?;
    fs::remove_file(&from_xml)?;
    if fs::read_dir(&parked).is_ok_and(|mut dir| dir.next().is_none()) {
        fs::remove_dir(&parked)?;
    } else {
        // Nothing else is expected there; whatever is keeps its place.
        rename_directory(&parked, &folder)?;
    }
    if to_dir.exists() {
        rename_tree(&to_dir, main)?;
    }
    rewrite_report(report, main, stored);
    Ok(())
}

// Windows scanners can briefly hold a directory after its files are written.
// Retry only access/sharing failures, keeping other errors immediate.
fn rename_directory(from: &Path, to: &Path) -> std::io::Result<()> {
    for attempt in 0..10 {
        match fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(error)
                if cfg!(windows)
                    && matches!(error.raw_os_error(), Some(5 | 32 | 33))
                    && attempt < 9 =>
            {
                std::thread::sleep(std::time::Duration::from_millis(50 * (attempt + 1)));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

/// Renames object references in every XML/HTML file under `dir`. A file that
/// is not UTF-8 (a legacy help page the pipeline writes through unchanged) is
/// left as it is.
fn rename_tree(dir: &Path, main: &ExternalMain) -> Result<()> {
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();
        let renamable = path
            .extension()
            .is_some_and(|ext| ext == "xml" || ext == "html");
        if !(entry.file_type().is_file() && renamable) {
            continue;
        }
        let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let renamed = if path.extension().is_some_and(|ext| ext == "html") {
            rename::to_external_html_references(text, main.kind, &main.name)
        } else {
            rename::to_external_references(text, main.kind, &main.name)
        };
        if renamed != text {
            fs::write(path, renamed)
                .with_context(|| format!("failed to write {}", path.display()))?;
        }
    }
    Ok(())
}

/// The pipeline's report of the adapted entries, told of the container as
/// stored (`entries`): the main row under its own name and size, `versions`
/// at its stored size, `copyinfo` when there is one, paths as the export lays
/// them out.
fn rewrite_report(
    report: &mut StorageImageSourceExportReport,
    main: &ExternalMain,
    entries: &[(String, usize)],
) {
    let prefix = format!("{}/", main.kind.internal_folder());
    let backslashed = format!("{}\\", main.kind.internal_folder());
    let stored = |name: &str| {
        entries
            .iter()
            .find(|(entry, _)| entry.eq_ignore_ascii_case(name))
            .map(|(_, bytes)| *bytes)
    };
    let storage = &mut report.storage;
    for entry in &mut storage.entries {
        if entry.logical_name == main.object_id {
            entry.logical_name.clone_from(&main.main_uuid);
            entry.logical_key.clone_from(&main.main_uuid);
        }
        if let Some(bytes) = stored(&entry.logical_name) {
            entry.packed_bytes = bytes;
        }
        for output in &mut entry.outputs {
            if let Some(rest) = output.strip_prefix(&prefix) {
                *output = rest.to_owned();
            }
        }
        entry
            .outputs
            .retain(|output| output != "ConfigDumpInfo.xml");
        if let Some(message) = &mut entry.message {
            *message = message
                .replace(&format!(" {prefix}"), " ")
                .replace(&format!(" {backslashed}"), " ");
            if let Some(rest) = message.strip_prefix(&prefix) {
                *message = rest.to_owned();
            }
        }
    }
    if let Some(bytes) = stored("copyinfo") {
        storage.entries.push(StorageExportEntryReport {
            logical_name: "copyinfo".into(),
            logical_key: "copyinfo".into(),
            part_count: 1,
            packed_bytes: bytes,
            disposition: StorageExportDisposition::Supported,
            outputs: Vec::new(),
            message: Some("configuration references, used to resolve types".into()),
        });
    }
    storage.physical_entries = entries.len();
    let count = |d: StorageExportDisposition| {
        storage
            .entries
            .iter()
            .filter(|entry| entry.disposition == d)
            .count()
    };
    let (supported, opaque, failed) = (
        count(StorageExportDisposition::Supported),
        count(StorageExportDisposition::Opaque),
        count(StorageExportDisposition::Failed),
    );
    storage.supported = supported;
    storage.opaque = opaque;
    storage.failed = failed;
    storage.logical_entries = storage.entries.len();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_main() -> ExternalMain {
        ExternalMain {
            kind: crate::external::ExternalKind::DataProcessor,
            main_uuid: "73380bb1-bee3-4b73-a3df-1dbea99011d2".into(),
            object_id: "11111111-bee3-4b73-a3df-1dbea99011d2".into(),
            name: "Ввод".into(),
            header: vec![],
            collections: vec![],
        }
    }

    #[test]
    fn the_report_speaks_of_the_container_as_stored() {
        let main = sample_main();
        let entries = vec![
            (main.main_uuid.clone(), 40),
            ("versions".to_owned(), 30),
            ("root".to_owned(), 10),
        ];
        let entry = |name: &str, bytes: usize, outputs: &[&str], message: Option<&str>| {
            let mut entry = StorageExportEntryReport::supported_packed(
                name,
                name,
                1,
                bytes,
                outputs.iter().map(|o| (*o).to_owned()).collect(),
            );
            entry.message = message.map(str::to_owned);
            entry
        };
        let mut report = StorageImageSourceExportReport {
            output_dir: "out".into(),
            source_version: "2.20".into(),
            files_written: 1,
            storage: ibcmd_cf::export::StorageExportReport::from_parts(
                None,
                2,
                3,
                vec![
                    entry(&main.object_id, 99, &["DataProcessors/Ввод.xml"], None),
                    entry("versions", 77, &[], None),
                    entry(
                        "root",
                        10,
                        &[],
                        Some("DataProcessors/Ввод/Forms/Ф/Ext/Form.xml withheld (x)"),
                    ),
                ],
            ),
        };
        rewrite_report(&mut report, &main, &entries);
        let storage = &report.storage;
        assert_eq!(storage.physical_entries, 3);
        assert!(
            storage.entries.iter().all(|e| e.logical_name != "copyinfo"),
            "no copyinfo stored"
        );
        let main_entry = &storage.entries[0];
        assert_eq!(
            (main_entry.logical_name.as_str(), main_entry.packed_bytes),
            (main.main_uuid.as_str(), 40)
        );
        assert_eq!(main_entry.outputs, vec!["Ввод.xml".to_owned()]);
        assert_eq!(storage.entries[1].packed_bytes, 30);
        assert_eq!(
            storage.entries[2].message.as_deref(),
            Some("Ввод/Forms/Ф/Ext/Form.xml withheld (x)")
        );
    }

    #[test]
    fn help_links_to_the_object_itself_resolve() {
        let foreign = foreign_references(&copyinfo::CopyInfo::default(), &sample_main());
        assert_eq!(
            foreign.objects["73380bb1-bee3-4b73-a3df-1dbea99011d2"],
            "DataProcessor.Ввод"
        );
    }

    #[test]
    fn any_ib_ref_is_written_as_any_ref() {
        let foreign = foreign_references(&copyinfo::CopyInfo::default(), &sample_main());
        assert!(foreign.types.contains(&(
            "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63".to_owned(),
            "cfg:AnyRef".to_owned(),
            "TypeSet".to_owned()
        )));
    }

    #[test]
    fn rename_skips_files_that_are_not_utf8() {
        let dir = std::env::temp_dir().join(format!("ibcmd-rename-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Ext/Help")).unwrap();
        // windows-1251 "ссылка DataProcessor.Ввод" — upstream writes such pages through unchanged
        let cp1251: &[u8] =
            b"<a href=\"DataProcessor.\xc2\xe2\xee\xe4/Help\">\xf1\xf1\xfb\xeb\xea\xe0</a>";
        std::fs::write(dir.join("Ext/Help/ru.html"), cp1251).unwrap();
        std::fs::write(dir.join("Ext/Help.xml"), "<a>DataProcessor.Ввод</a>").unwrap();
        rename_tree(&dir, &sample_main()).unwrap();
        assert_eq!(std::fs::read(dir.join("Ext/Help/ru.html")).unwrap(), cp1251);
        assert_eq!(
            std::fs::read_to_string(dir.join("Ext/Help.xml")).unwrap(),
            "<a>ExternalDataProcessor.Ввод</a>"
        );
    }

    #[test]
    fn undecodable_root_is_left_to_the_configuration_path() {
        let entries = vec![("root".to_owned(), b"not a raw deflate stream".to_vec())];
        assert!(detect(&entries).unwrap().is_none());
    }

    #[test]
    fn plain_configuration_is_not_external() {
        let root = deflate_raw(
            "\u{feff}{2,30ffe4cc-eef2-4371-8b26-046597e37e22,h55/Mge2fMFz==}".as_bytes(),
        )
        .unwrap();
        let entries = vec![("root".to_owned(), root)];
        assert!(detect(&entries).unwrap().is_none());
    }

    #[test]
    fn detects_external_processor_fixture() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/external/test_processor/input.epf"
        );
        let archive = read_archive(std::path::Path::new(path));
        let main = detect(&entries_of(&archive)).unwrap().unwrap();
        assert_eq!(main.name, "ТестОбработка");
        assert_eq!(main.main_uuid, "dc0e1582-2d76-4880-ba01-3711da6f4448");
        assert_eq!(main.object_id, "80ba9317-3805-45ad-950d-e4cab1dd642b");
    }

    fn read_archive(path: &std::path::Path) -> PackedCfArchive {
        let file = std::fs::File::open(path).unwrap();
        ibcmd_cf::archive::decode_packed_archive(
            file,
            ibcmd_core::limits::ResourceLimits::default(),
            ibcmd_core::artifact::StorageProfileId::parse("storage:cf-cli").unwrap(),
        )
        .unwrap()
    }
}
