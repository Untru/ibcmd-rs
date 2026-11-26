//! A partial import of listed files: `infobase config import files
//! --base-dir=<dir> [--partial] [--no-check] <files>` (Untru/ibcmd-rs#363),
//! and `mssql-stage-source-objects --file` under it.
//!
//! What the platform does, as far as the lab measured it (8.3.27 and 8.5):
//! `import files --partial` stages a delta -- the rows of the listed files and
//! `root`, `version`, `versions`, nothing else (`docs/apply/restructuring.md`
//! 3.4 and finding 10: "the changed descriptor plus `root`, `version` and
//! `versions`", 4 rows for one catalog descriptor; two module files of a
//! sparse directory, 7 rows: "the descriptors, the two module rows, `root`,
//! `version`, `versions`", `docs/apply/uh-case1-plan.md` section 4). The
//! directory need not be a whole tree (`scripts/apply-trace/lab/
//! validate_sparse_import.ps1`), and a listed `Configuration.xml` makes it
//! re-import the whole tree (9 615 rows, `docs/apply/native-apply-trace.md`
//! case 2c).
//!
//! What 8.5.1.1529 does with the flags and the edge cases, measured on a
//! БСП file infobase (`docs/import/import-files.md`):
//! - `--partial` "Разрешить частичный набор файлов объектов метаданных": a
//!   listed metadata file loads its object from the directory -- the
//!   descriptor, every body and every object nested in it whose files the
//!   directory holds (`Catalogs/Валюты.xml` staged the catalog's modules and
//!   help and its four forms, an edit of the unlisted object module
//!   included). Without `--partial` the bodies of such an object that the
//!   directory lacks are removed (listed in `deleted`).
//! - a listed file the directory lacks is skipped with `--partial` (nothing
//!   staged for it); without, its row is removed.
//! - a listed body file stages its row and re-saves the owner's descriptor,
//!   whether `--partial` is given or not, in a sparse directory too.
//! - an existing stage in `ConfigSave` is added to, not replaced: two
//!   imports one after the other leave the rows of both.
//! - `--no-check` changes nothing a valid edit stages.
//!
//! How the files become rows: each listed file belongs to the object whose
//! metadata file is the file itself or is named as the nearest folder above it
//! (`Catalogs/X/Ext/ObjectModule.bsl` -> `Catalogs/X.xml`,
//! `Catalogs/X/Forms/F/Ext/Form/Module.bsl` -> `Catalogs/X/Forms/F.xml`; the
//! root `Ext` is the configuration's). Those objects are prepared by the patch
//! stage exactly as `--path-prefix` prepares them (the base rows of the
//! target, the tree's files), and of their rows only the ones the listed files
//! compile to are staged: the descriptor when the metadata file is listed, a
//! body when its source file is listed or lies in the folder of its source
//! (`Ext/Form.xml` and `Ext/Form/Module.bsl`, `Ext/Help.xml` and
//! `Ext/Help/ru.html`). The rows the stage leaves out stay the target's, as
//! a delta stage of #395 leaves them (`delta_stage`). The platform also
//! re-saves the owner's descriptor of a listed body file; a descriptor whose
//! metadata file is not listed stays the target's here.
//!
//! Refused before anything is read from the database: a file outside the
//! directory, a listed file the directory does not hold without `--partial`
//! (a deletion), a file of no object, `Configuration.xml` (the platform's
//! whole re-import). Refused after the objects are prepared: a listed file
//! none of their rows comes from, a new object (it needs `Configuration.xml`
//! to be listed), an object loaded without `--partial` that the target holds
//! a body of the directory lacks (the platform would remove it), and a
//! `ConfigSave` that already holds a stage (the platform would add to it;
//! this stage replaces `ConfigSave`).

use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::{
    PreparedCommonModuleObjectStage, PreparedMetadataObjectStage, is_configuration_metadata_xml,
    is_root_common_module_xml, is_stage_metadata_xml,
};

/// The listed files, each with the object it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesSelection {
    /// Relative to the directory, `/`-separated, in the order given, once each.
    files: Vec<String>,
    /// The metadata file of each object a listed file belongs to, sorted.
    owners: Vec<String>,
    /// The metadata files of the objects loaded whole: listed, or nested in
    /// a listed one (lower case).
    loaded: Vec<String>,
    /// The platform's rules of `import files` (`select_native`), with
    /// `--partial` (`Some(true)`) or without; `None` for the exact selection
    /// of `select`.
    native: Option<bool>,
    /// Listed files the directory does not hold, skipped under `--partial`.
    skipped: Vec<String>,
}

/// A partial import that cannot be carried out, with its reason in Russian.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesRefused {
    pub message: String,
    /// What this version does not serve (exit code 1 of the drop-in), rather
    /// than a request that cannot work (-1).
    pub unsupported: bool,
}

impl fmt::Display for FilesRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FilesRefused {}

impl FilesRefused {
    /// `ConfigSave` already holds a stage, which the platform's `import
    /// files` would add to.
    pub(super) fn existing_stage(rows: i64) -> Self {
        unsupported(format!(
            "Загрузка невозможна: в ConfigSave уже есть подготовленная загрузка (строк: {rows}); \
             платформа добавила бы к ней загружаемые файлы, а добавление к подготовленной \
             загрузке {PLANNED}. Примените ее (infobase config apply) или загрузите каталог \
             целиком (infobase config import). В ConfigSave ничего не записано."
        ))
    }
}

fn failed(message: String) -> FilesRefused {
    FilesRefused {
        message,
        unsupported: false,
    }
}

fn unsupported(message: String) -> FilesRefused {
    FilesRefused {
        message,
        unsupported: true,
    }
}

const PLANNED: &str = "не поддерживается в этой версии ibcmd-rs";

impl FilesSelection {
    pub fn files(&self) -> &[String] {
        &self.files
    }

    pub fn owners(&self) -> &[String] {
        &self.owners
    }

    /// The listed files the directory does not hold, which `--partial` skips.
    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// Without `--partial`, refuses an object loaded whole that the target
    /// holds a body of and the directory does not: the platform removes such
    /// a body (`deleted`), this stage would keep it. `target_names` holds the
    /// target's published rows (`None` when they were not read); the prepared
    /// objects are the ones `trim` kept.
    pub(super) fn refuse_removed_bodies(
        &self,
        root: &Path,
        target_names: Option<&[String]>,
        metadata_objects: &[PreparedMetadataObjectStage],
        common_modules: &[PreparedCommonModuleObjectStage],
    ) -> Result<(), FilesRefused> {
        if self.native != Some(false) || self.loaded.is_empty() {
            return Ok(());
        }
        let Some(target_names) = target_names else {
            return Err(unsupported(
                "Загрузка невозможна: без --partial платформа удаляет тела загружаемых объектов, \
                 которых нет в каталоге, а строки базы для этой проверки не прочитаны; укажите \
                 --partial. В ConfigSave ничего не записано."
                    .to_string(),
            ));
        };
        let loaded =
            |xml: &Path| relative_lower(root, xml).is_some_and(|xml| self.loaded.contains(&xml));
        let mut removed = Vec::new();
        for object in metadata_objects.iter().filter(|object| loaded(&object.xml)) {
            let staged = object
                .body_rows
                .iter()
                .map(|body| body.body_id.as_str())
                .collect::<Vec<_>>();
            removed.extend(removed_bodies(&object.object_id, &staged, target_names));
        }
        for module in common_modules.iter().filter(|module| loaded(&module.xml)) {
            let staged = if module.has_module_body {
                vec![module.module_body_id.as_str()]
            } else {
                Vec::new()
            };
            removed.extend(removed_bodies(&module.module_id, &staged, target_names));
        }
        if removed.is_empty() {
            return Ok(());
        }
        Err(unsupported(format!(
            "Загрузка невозможна: без --partial платформа удалила бы строки {}: их файлов нет в \
             каталоге {}; удаление {PLANNED}. С --partial эти строки остаются. В ConfigSave \
             ничего не записано.",
            removed.join(", "),
            shown(root)
        )))
    }

    /// An existing measured body-only selection can be compiled without
    /// rebuilding unrelated assets of its owner. Metadata-file selections
    /// and every other family keep the complete object preparation route.
    pub(super) fn measured_body_files(
        &self,
        root: &Path,
        xml: &Path,
    ) -> Result<Option<Vec<String>>, FilesRefused> {
        let relative = xml
            .strip_prefix(root)
            .map_err(|_| failed("selected metadata owner escapes the source root".to_owned()))?
            .to_string_lossy()
            .replace('\\', "/");
        let mut files = Vec::new();
        for file in &self.files {
            if owner_of(root, file)? == relative {
                if !crate::mssql_source_change::measured_main_source_path(file)
                    || file.ends_with("/Ext/Form.xml")
                    || file.ends_with("/Ext/Form/Module.bsl")
                {
                    return Ok(None);
                }
                files.push(file.clone());
            }
        }
        Ok((!files.is_empty()).then_some(files))
    }

    /// The paths the stage scans: the owners' metadata files and the listed
    /// files themselves (the guard compares the listed files).
    pub fn scan_prefixes(&self) -> Vec<String> {
        let mut prefixes = self.owners.clone();
        prefixes.extend(self.files.iter().cloned());
        prefixes.sort();
        prefixes.dedup();
        prefixes
    }

    /// Keeps of the prepared objects the rows the listed files compile to;
    /// refuses a listed file none of them comes from.
    pub(super) fn trim(
        &self,
        root: &Path,
        metadata_objects: &mut Vec<PreparedMetadataObjectStage>,
        common_modules: &mut Vec<PreparedCommonModuleObjectStage>,
    ) -> Result<(), FilesRefused> {
        let listed = self
            .files
            .iter()
            .map(|file| file.to_lowercase())
            .collect::<Vec<_>>();
        let mut claimed = vec![false; listed.len()];
        let mut claim = |source: &Path, folder_too: bool| -> bool {
            let Some(source) = relative_lower(root, source) else {
                return false;
            };
            let folder = folder_too.then(|| source_folder(&source)).flatten();
            let mut any = false;
            for (index, file) in listed.iter().enumerate() {
                let inside = folder
                    .as_deref()
                    .is_some_and(|folder| file.starts_with(&format!("{folder}/")));
                if *file == source || inside {
                    claimed[index] = true;
                    any = true;
                }
            }
            any
        };
        for object in metadata_objects.iter_mut() {
            if !claim(&object.xml, false) {
                object.metadata_blob = Vec::new();
                object.metadata_blob_sha256.clear();
                object.metadata_plain_bytes = 0;
            }
            object.body_rows.retain(|body| claim(&body.path, true));
        }
        metadata_objects
            .retain(|object| !object.metadata_blob.is_empty() || !object.body_rows.is_empty());
        for module in common_modules.iter_mut() {
            if !claim(&module.xml, false) {
                module.metadata_blob = Vec::new();
                module.metadata_blob_sha256.clear();
                module.metadata_plain_bytes = 0;
            }
            if module.has_module_body && !claim(&module.text, true) {
                module.has_module_body = false;
            }
        }
        common_modules.retain(|module| module.row_count() > 0);
        let unplaced = self
            .files
            .iter()
            .zip(&claimed)
            .filter(|(_, claimed)| !**claimed)
            .map(|(file, _)| file.as_str())
            .collect::<Vec<_>>();
        if unplaced.is_empty() {
            return Ok(());
        }
        Err(unsupported(format!(
            "Загрузка невозможна: из файлов {} не собирается ни одна строка конфигурации их объектов; \
             частичная загрузка таких файлов {PLANNED}. В ConfigSave ничего не записано.",
            unplaced.join(", ")
        )))
    }

    /// Refuses the objects the stage would add: a new object needs the
    /// configuration's own list of objects (`Configuration.xml`), which a
    /// partial import does not take here.
    pub(super) fn refuse_added(&self, added: &[String]) -> Result<(), FilesRefused> {
        if added.is_empty() {
            return Ok(());
        }
        Err(unsupported(format!(
            "Загрузка невозможна: объектов {} нет в конфигурации базы; добавление объектов \
             частичной загрузкой {PLANNED}, загрузите каталог целиком командой infobase config import",
            added.join(", ")
        )))
    }
}

/// A directory as the user would write it: without the `\\?\` of a
/// canonical Windows path.
fn shown(root: &Path) -> String {
    let text = root.display().to_string();
    text.strip_prefix(r"\\?\")
        .map(str::to_string)
        .unwrap_or(text)
}

/// The source path relative to the root, `/`-separated, lower case.
fn relative_lower(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let parts = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("/").to_lowercase())
}

/// The folder whose files a body compiled from `source` also reads: the
/// source without its extension (`.../ext/form.xml` -> `.../ext/form`, which
/// holds `Module.bsl` and the items' pictures; `.../ext/help.xml` ->
/// `.../ext/help` with the pages).
fn source_folder(source: &str) -> Option<String> {
    let (folder, name) = source.rsplit_once('/').unwrap_or(("", source));
    let (stem, _) = name.rsplit_once('.')?;
    if stem.is_empty() {
        return None;
    }
    Some(if folder.is_empty() {
        stem.to_string()
    } else {
        format!("{folder}/{stem}")
    })
}

/// Whether a relative path is the metadata file of an object the patch stage
/// prepares (the configuration's own is refused before this is asked).
fn is_owner_xml(relative: &str) -> bool {
    is_stage_metadata_xml(relative) || is_root_common_module_xml(relative)
}

/// The body rows of the object `object_id` the target holds and a whole
/// load of it would not stage (`staged`): what the platform lists in
/// `deleted` without `--partial`.
fn removed_bodies(object_id: &str, staged: &[&str], target_names: &[String]) -> Vec<String> {
    let prefix = format!("{}.", object_id.to_ascii_lowercase());
    // Use what the target actually publishes, including roles beyond any
    // measured corpus. A role we cannot load must be refused, not left over.
    target_names
        .iter()
        .filter(|row| {
            row.to_ascii_lowercase().starts_with(&prefix)
                && row.len() > prefix.len()
                && !staged.iter().any(|staged| staged.eq_ignore_ascii_case(row))
        })
        .cloned()
        .collect()
}

/// The files of the object of the metadata file `relative` that the
/// directory holds: the ones of its folder, nested objects' included
/// (`Catalogs/X.xml` -> `Catalogs/X/**`), relative and `/`-separated, sorted.
fn object_files(root: &Path, relative: &str) -> Vec<String> {
    let Some(folder) = relative.strip_suffix(".xml") else {
        return Vec::new();
    };
    let mut files = walkdir::WalkDir::new(root.join(folder))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let inside = entry.path().strip_prefix(root).ok()?;
            Some(
                inside
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/"),
            )
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}

/// The metadata file of the object a listed file belongs to: the file itself,
/// else the nearest folder above it named as a metadata file, else (for the
/// root `Ext`) the configuration's. The candidate is chosen by its name; a
/// candidate the directory does not hold is refused, not passed over.
fn owner_of(root: &Path, relative: &str) -> Result<String, FilesRefused> {
    if is_owner_xml(relative) {
        return Ok(relative.to_string());
    }
    let mut folder = relative;
    while let Some(cut) = folder.rfind('/') {
        folder = &folder[..cut];
        let candidate = format!("{folder}.xml");
        if is_owner_xml(&candidate) {
            if root.join(&candidate).is_file() {
                return Ok(candidate);
            }
            return Err(unsupported(format!(
                "Загрузка невозможна: файл {relative} принадлежит объекту {candidate}, которого нет в каталоге {}; \
                 частичная загрузка без файла описания объекта {PLANNED}",
                root.display()
            )));
        }
    }
    let in_root_ext = relative
        .split('/')
        .next()
        .is_some_and(|first| first.eq_ignore_ascii_case("Ext"))
        && relative.contains('/');
    if in_root_ext {
        if root.join("Configuration.xml").is_file() {
            return Ok("Configuration.xml".to_string());
        }
        return Err(unsupported(format!(
            "Загрузка невозможна: файл {relative} принадлежит конфигурации, а в каталоге {} нет Configuration.xml; \
             частичная загрузка без файла описания объекта {PLANNED}",
            root.display()
        )));
    }
    Err(unsupported(format!(
        "Загрузка невозможна: файл {relative} не принадлежит ни одному объекту конфигурации; \
         его загрузка {PLANNED}"
    )))
}

/// A listed path made relative to the directory: a relative one is taken
/// from the directory, an absolute one must lie inside it; `..` may not
/// leave it. `None` for a path outside.
fn relative_inside(root: &Path, canonical_root: &Path, given: &str) -> Option<String> {
    let given = PathBuf::from(given.replace('\\', "/"));
    let relative = if given.is_absolute() {
        match given
            .strip_prefix(root)
            .or_else(|_| given.strip_prefix(canonical_root))
        {
            Ok(relative) => relative.to_path_buf(),
            Err(_) => fs::canonicalize(&given)
                .ok()?
                .strip_prefix(canonical_root)
                .ok()?
                .to_path_buf(),
        }
    } else {
        given
    };
    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Checks the listed files against the directory and finds their objects:
/// the files exactly as listed, a missing one refused (what
/// `mssql-apply-source-change` hands over). Reads the directory only:
/// nothing here reaches a database.
pub fn select(root: &Path, files: &[String]) -> Result<FilesSelection, FilesRefused> {
    select_with(root, files, None)
}

/// [`select`] by the rules of the platform's `import files` (see the module
/// documentation): a listed metadata file brings the files of its object,
/// and with `--partial` (`partial`) a missing file is skipped.
pub fn select_native(
    root: &Path,
    files: &[String],
    partial: bool,
) -> Result<FilesSelection, FilesRefused> {
    select_with(root, files, Some(partial))
}

fn select_with(
    root: &Path,
    files: &[String],
    native: Option<bool>,
) -> Result<FilesSelection, FilesRefused> {
    if files.is_empty() {
        return Err(failed(
            "Не указаны файлы для загрузки (пути относительно --base-dir)".to_string(),
        ));
    }
    let canonical_root = fs::canonicalize(root)
        .ok()
        .filter(|path| path.is_dir())
        .ok_or_else(|| {
            failed(format!(
                "каталог файлов конфигурации не найден: {}",
                root.display()
            ))
        })?;
    let mut selected = Vec::<String>::new();
    let mut owners = Vec::<String>::new();
    let mut loaded = Vec::<String>::new();
    let mut skipped = Vec::<String>::new();
    for given in files {
        let Some(relative) = relative_inside(root, &canonical_root, given) else {
            return Err(failed(format!(
                "Загрузка невозможна: файл {given} находится вне каталога {}",
                root.display()
            )));
        };
        let path = root.join(&relative);
        if !path.exists() {
            // The platform skips it under `--partial` and removes its row
            // without (8.5.1.1529).
            if native == Some(true) {
                skipped.push(relative);
                continue;
            }
            return Err(unsupported(format!(
                "Загрузка невозможна: файла {relative} нет в каталоге {}; без --partial платформа \
                 удалила бы его строку, удаление файлов и объектов частичной загрузкой {PLANNED} \
                 (с --partial такой файл пропускается)",
                root.display()
            )));
        }
        if !path.is_file() {
            return Err(failed(format!(
                "Загрузка невозможна: {relative} в каталоге {} не файл",
                root.display()
            )));
        }
        // A link that leads out of the directory is a file outside it.
        let inside = fs::canonicalize(&path)
            .map(|canonical| canonical.starts_with(&canonical_root))
            .unwrap_or(false);
        if !inside {
            return Err(failed(format!(
                "Загрузка невозможна: файл {given} находится вне каталога {}",
                root.display()
            )));
        }
        if is_configuration_metadata_xml(&relative) {
            // The platform re-imports the whole tree for it (case 2c of
            // docs/apply/native-apply-trace.md); the whole import is
            // `infobase config import`.
            return Err(unsupported(format!(
                "Загрузка невозможна: частичная загрузка Configuration.xml {PLANNED} (платформа в этом \
                 случае загружает конфигурацию целиком); загрузите каталог целиком командой \
                 infobase config import"
            )));
        }
        if selected
            .iter()
            .any(|known| known.to_lowercase() == relative.to_lowercase())
        {
            continue;
        }
        owners.push(owner_of(root, &relative)?);
        // A metadata file loads its object whole: the files of its folder
        // the directory holds, nested objects' too (8.5.1.1529).
        let loads_object = native.is_some() && is_owner_xml(&relative);
        if loads_object {
            loaded.push(relative.to_lowercase());
        }
        selected.push(relative.clone());
        if loads_object {
            for file in object_files(root, &relative) {
                if selected
                    .iter()
                    .any(|known| known.to_lowercase() == file.to_lowercase())
                {
                    continue;
                }
                let owner = owner_of(root, &file)?;
                if owner != relative && is_owner_xml(&owner) {
                    loaded.push(owner.to_lowercase());
                }
                owners.push(owner);
                selected.push(file);
            }
        }
    }
    owners.sort();
    owners.dedup();
    loaded.sort();
    loaded.dedup();
    Ok(FilesSelection {
        files: selected,
        owners,
        loaded,
        native,
        skipped,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str, files: &[&str]) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("ibcmd-rs-files-stage-{tag}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            for file in files {
                let path = dir.join(file);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, b"x").unwrap();
            }
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn list(files: &[&str]) -> Vec<String> {
        files.iter().map(|file| file.to_string()).collect()
    }

    #[test]
    fn narrow_preparation_keeps_metadata_and_unmeasured_mixed_selections_whole() {
        let tree = Scratch::new(
            "measured-body345",
            &[
                "Catalogs/X.xml",
                "Catalogs/X/Ext/ObjectModule.bsl",
                "Catalogs/X/Ext/ManagerModule.bsl",
                "Catalogs/X/Ext/Help.xml",
                "InformationRegisters/R.xml",
                "InformationRegisters/R/Ext/RecordSetModule.bsl",
                "Reports/Y/Templates/T.xml",
                "Reports/Y/Templates/T/Ext/Template.xml",
            ],
        );
        let xml = tree.0.join("Catalogs/X.xml");
        let files = list(&["Catalogs/X/Ext/ObjectModule.bsl"]);
        assert_eq!(
            select(&tree.0, &files)
                .unwrap()
                .measured_body_files(&tree.0, &xml)
                .unwrap(),
            Some(files)
        );
        for files in [
            list(&["Catalogs/X.xml", "Catalogs/X/Ext/ObjectModule.bsl"]),
            list(&["Catalogs/X/Ext/ObjectModule.bsl", "Catalogs/X/Ext/Help.xml"]),
        ] {
            assert_eq!(
                select(&tree.0, &files)
                    .unwrap()
                    .measured_body_files(&tree.0, &xml)
                    .unwrap(),
                None
            );
        }
        let files = list(&["InformationRegisters/R/Ext/RecordSetModule.bsl"]);
        assert_eq!(
            select(&tree.0, &files)
                .unwrap()
                .measured_body_files(&tree.0, &tree.0.join("InformationRegisters/R.xml"))
                .unwrap(),
            None
        );
        let files = list(&["Reports/Y/Templates/T/Ext/Template.xml"]);
        assert_eq!(
            select(&tree.0, &files)
                .unwrap()
                .measured_body_files(&tree.0, &tree.0.join("Reports/Y/Templates/T.xml"))
                .unwrap(),
            Some(files)
        );
    }

    #[test]
    fn each_file_belongs_to_the_nearest_object_above_it() {
        let tree = Scratch::new(
            "owners",
            &[
                "Configuration.xml",
                "Ext/ManagedApplicationModule.bsl",
                "Catalogs/X.xml",
                "Catalogs/X/Ext/ObjectModule.bsl",
                "Catalogs/X/Commands/C/Ext/CommandModule.bsl",
                "Catalogs/X/Forms/F.xml",
                "Catalogs/X/Forms/F/Ext/Form.xml",
                "Catalogs/X/Forms/F/Ext/Form/Module.bsl",
                "Catalogs/X/Templates/T.xml",
                "Catalogs/X/Templates/T/Ext/Template.xml",
                "CommonModules/M.xml",
                "CommonModules/M/Ext/Module.bsl",
            ],
        );
        let selection = select(
            &tree.0,
            &list(&[
                "Catalogs/X/Ext/ObjectModule.bsl",
                "Catalogs\\X\\Commands\\C\\Ext\\CommandModule.bsl",
                "./Catalogs/X/Forms/F/Ext/Form/Module.bsl",
                "Catalogs/X/Templates/T/Ext/Template.xml",
                "CommonModules/M/Ext/Module.bsl",
                "Ext/ManagedApplicationModule.bsl",
                "Catalogs/X.xml",
                "Catalogs/X/../X.xml",
            ]),
        )
        .unwrap();
        assert_eq!(
            selection.files(),
            [
                "Catalogs/X/Ext/ObjectModule.bsl",
                "Catalogs/X/Commands/C/Ext/CommandModule.bsl",
                "Catalogs/X/Forms/F/Ext/Form/Module.bsl",
                "Catalogs/X/Templates/T/Ext/Template.xml",
                "CommonModules/M/Ext/Module.bsl",
                "Ext/ManagedApplicationModule.bsl",
                "Catalogs/X.xml",
            ]
        );
        assert_eq!(
            selection.owners(),
            [
                "Catalogs/X.xml",
                "Catalogs/X/Forms/F.xml",
                "Catalogs/X/Templates/T.xml",
                "CommonModules/M.xml",
                "Configuration.xml",
            ]
        );
        // an absolute path inside the directory is taken too
        let absolute = tree.0.join("Catalogs/X/Ext/ObjectModule.bsl");
        let selection = select(&tree.0, &[absolute.to_string_lossy().into_owned()]).unwrap();
        assert_eq!(selection.files(), ["Catalogs/X/Ext/ObjectModule.bsl"]);
    }

    #[test]
    fn what_cannot_be_placed_is_refused_by_name() {
        let tree = Scratch::new(
            "refused",
            &[
                "Configuration.xml",
                "Catalogs/X.xml",
                "Catalogs/X/Ext/ObjectModule.bsl",
                "Catalogs/Y/Ext/ObjectModule.bsl",
                "Catalogs/X/Forms/F/Ext/Form.xml",
                "ConfigDumpInfo.xml",
            ],
        );
        let refused = |files: &[&str]| select(&tree.0, &list(files)).unwrap_err();

        let outside = refused(&["../elsewhere/Catalogs/X.xml"]);
        assert!(!outside.unsupported);
        assert!(outside.message.contains("вне каталога"), "{outside}");
        let outside = std::env::temp_dir().join("Catalogs").join("X.xml");
        let outside = select(&tree.0, &[outside.to_string_lossy().into_owned()]).unwrap_err();
        assert!(outside.message.contains("вне каталога"), "{outside}");

        let deleted = refused(&["Catalogs/Z.xml"]);
        assert!(deleted.unsupported);
        assert!(
            deleted
                .message
                .contains("файла Catalogs/Z.xml нет в каталоге")
                && deleted.message.contains("удаление"),
            "{deleted}"
        );

        let directory = refused(&["Catalogs/X"]);
        assert!(directory.message.contains("не файл"), "{directory}");

        let root = refused(&["Configuration.xml"]);
        assert!(root.unsupported);
        assert!(root.message.contains("Configuration.xml"), "{root}");

        // the object's own file is not in the directory
        let no_owner = refused(&["Catalogs/Y/Ext/ObjectModule.bsl"]);
        assert!(no_owner.unsupported);
        assert!(no_owner.message.contains("Catalogs/Y.xml"), "{no_owner}");
        let no_form = refused(&["Catalogs/X/Forms/F/Ext/Form.xml"]);
        assert!(
            no_form.message.contains("Catalogs/X/Forms/F.xml"),
            "{no_form}"
        );

        // a file of no object
        let stray = refused(&["ConfigDumpInfo.xml"]);
        assert!(stray.unsupported);
        assert!(
            stray.message.contains("не принадлежит ни одному объекту"),
            "{stray}"
        );

        assert!(select(&tree.0, &[]).is_err());
        assert!(select(&tree.0.join("missing"), &list(&["Catalogs/X.xml"])).is_err());
    }

    #[test]
    fn a_listed_metadata_file_loads_its_object_by_the_platforms_rules() {
        // 8.5.1.1529 (`docs/import/import-files.md`): `Catalogs/Валюты.xml`
        // staged the catalog's bodies and its forms from the directory
        let tree = Scratch::new(
            "native",
            &[
                "Catalogs/X.xml",
                "Catalogs/X/Ext/ObjectModule.bsl",
                "Catalogs/X/Ext/Help.xml",
                "Catalogs/X/Ext/Help/ru.html",
                "Catalogs/X/Forms/F.xml",
                "Catalogs/X/Forms/F/Ext/Form.xml",
                "Catalogs/X/Forms/F/Ext/Form/Module.bsl",
                "Catalogs/XY.xml",
                "Catalogs/XY/Ext/ObjectModule.bsl",
            ],
        );
        let selection = select_native(&tree.0, &list(&["Catalogs/X.xml"]), true).unwrap();
        assert_eq!(
            selection.files(),
            [
                "Catalogs/X.xml",
                "Catalogs/X/Ext/Help.xml",
                "Catalogs/X/Ext/Help/ru.html",
                "Catalogs/X/Ext/ObjectModule.bsl",
                "Catalogs/X/Forms/F.xml",
                "Catalogs/X/Forms/F/Ext/Form.xml",
                "Catalogs/X/Forms/F/Ext/Form/Module.bsl",
            ]
        );
        assert_eq!(
            selection.owners(),
            ["Catalogs/X.xml", "Catalogs/X/Forms/F.xml"]
        );
        assert_eq!(
            selection.loaded,
            ["catalogs/x.xml", "catalogs/x/forms/f.xml"]
        );
        // the exact selection of `select` keeps the file alone
        assert_eq!(
            select(&tree.0, &list(&["Catalogs/X.xml"])).unwrap().files(),
            ["Catalogs/X.xml"]
        );
        // a body file loads no object
        let body =
            select_native(&tree.0, &list(&["Catalogs/X/Ext/ObjectModule.bsl"]), false).unwrap();
        assert_eq!(body.files(), ["Catalogs/X/Ext/ObjectModule.bsl"]);
        assert!(body.loaded.is_empty());
    }

    #[test]
    fn a_missing_file_is_skipped_with_partial_and_refused_without() {
        let tree = Scratch::new(
            "skipped",
            &["Catalogs/X.xml", "Catalogs/X/Ext/ObjectModule.bsl"],
        );
        let files = list(&[
            "Catalogs/X/Ext/ManagerModule.bsl",
            "Catalogs/X/Ext/ObjectModule.bsl",
        ]);
        let selection = select_native(&tree.0, &files, true).unwrap();
        assert_eq!(selection.files(), ["Catalogs/X/Ext/ObjectModule.bsl"]);
        assert_eq!(selection.skipped(), ["Catalogs/X/Ext/ManagerModule.bsl"]);
        let refused = select_native(&tree.0, &files, false).unwrap_err();
        assert!(refused.unsupported);
        assert!(
            refused
                .message
                .contains("с --partial такой файл пропускается"),
            "{refused}"
        );
        assert!(select(&tree.0, &files).is_err());
    }

    #[test]
    fn a_whole_load_without_partial_would_remove_the_bodies_the_directory_lacks() {
        let target = list(&["o", "o.0", "o.3"]);
        assert_eq!(removed_bodies("o", &["o.0"], &target), ["o.3"]);
        // the roles the platform lists for rows the target does not hold
        // (`.1c`, `.1d`) remove nothing
        assert!(removed_bodies("o", &["o.0", "o.3"], &target).is_empty());
    }

    #[test]
    fn a_whole_load_refuses_every_actual_target_body_role() {
        let target = list(&["o", "o.0", "o.1e", "o.20", "other.20"]);
        assert_eq!(removed_bodies("o", &["o.0"], &target), ["o.1e", "o.20"]);
    }

    #[test]
    fn a_whole_load_sees_a_body_published_only_by_a_dynamic_alias() {
        let generation = "06cb0442-0c47-4fad-986a-f08f28287c1b";
        let owner = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let alias = format!("{owner}_dynupdate_{generation}.3");
        let rows = std::collections::HashMap::from([(alias, std::sync::Arc::new(vec![1]))]);
        let marker = format!("{{1,1,{generation}}}");
        let aliases = crate::mssql_dump::dynamic_generation_aliases(
            Some(marker.as_bytes()),
            rows.keys().map(String::as_str),
        )
        .unwrap();
        let target = super::super::file_import_target_row_names(&rows, &aliases);
        assert_eq!(removed_bodies(owner, &[], &target), [format!("{owner}.3")]);
    }

    #[test]
    fn final_prepared_rows_refuse_missing_target_bodies_only_without_partial() {
        let tree = Scratch::new("final-body-rows", &["Catalogs/X.xml"]);
        let object = PreparedMetadataObjectStage {
            object_id: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_owned(),
            kind: "Catalog".to_owned(),
            xml: tree.0.join("Catalogs/X.xml"),
            properties: super::super::SimpleMetadataXmlProperties {
                kind: "Catalog".to_owned(),
                uuid: "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa".to_owned(),
                name: "X".to_owned(),
                synonyms: Vec::new(),
                comment: String::new(),
            },
            metadata_plain_bytes: 1,
            metadata_blob: vec![1],
            metadata_blob_sha256: "descriptor".to_owned(),
            body_rows: Vec::new(),
        };
        let target = list(&["aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa.20"]);
        for partial in [false, true] {
            let selection = select_native(&tree.0, &list(&["Catalogs/X.xml"]), partial).unwrap();
            let mut objects = vec![object.clone()];
            selection
                .trim(&tree.0, &mut objects, &mut Vec::new())
                .unwrap();
            let result = selection.refuse_removed_bodies(&tree.0, Some(&target), &objects, &[]);
            if partial {
                assert!(result.is_ok());
            } else {
                assert!(result.unwrap_err().message.contains(&target[0]));
            }
        }
    }

    #[test]
    fn a_body_reads_its_source_and_the_folder_of_the_same_name() {
        assert_eq!(
            source_folder("catalogs/x/forms/f/ext/form.xml").as_deref(),
            Some("catalogs/x/forms/f/ext/form")
        );
        assert_eq!(
            source_folder("catalogs/x/ext/help.xml").as_deref(),
            Some("catalogs/x/ext/help")
        );
        assert_eq!(source_folder("ext/.hidden"), None);
        assert_eq!(source_folder("ext/module"), None);
    }
}
