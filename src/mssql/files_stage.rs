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
//! case 2c). Nothing in the repository records the platform's help for
//! `--base-dir`, `--partial` or `--no-check`, nor what it does without
//! `--partial`, with a listed file that is missing (a deletion) or with one
//! outside the directory; this module takes the conservative reading of each.
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
//! directory, a listed file the directory does not hold (a deletion), a file of
//! no object, `Configuration.xml` (the platform's whole re-import). Refused
//! after the objects are prepared: a listed file none of their rows comes from,
//! and a new object (it needs `Configuration.xml` to be listed).

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

    /// An existing measured body-only selection can be compiled without
    /// rebuilding unrelated assets of its owner. Metadata-file selections
    /// and every other family keep the complete object preparation route.
    #[cfg(test)]
    pub(super) fn measured_body_files(
        &self,
        root: &Path,
        xml: &Path,
    ) -> Result<Option<Vec<String>>, FilesRefused> {
        self.measured_body_files_with_source(root, xml, None)
    }

    pub(super) fn measured_body_files_with_source(
        &self,
        root: &Path,
        xml: &Path,
        source: Option<&crate::module_blob::MetadataSourceContext>,
    ) -> Result<Option<Vec<String>>, FilesRefused> {
        let relative = xml
            .strip_prefix(root)
            .map_err(|_| failed("selected metadata owner escapes the source root".to_owned()))?
            .to_string_lossy()
            .replace('\\', "/");
        let mut files = Vec::new();
        for file in &self.files {
            if owner_of_with_source(root, file, source)? == relative {
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

/// The metadata file of the object a listed file belongs to: the file itself,
/// else the nearest folder above it named as a metadata file, else (for the
/// root `Ext`) the configuration's. The candidate is chosen by its name; a
/// candidate the directory does not hold is refused, not passed over.
fn owner_of_with_source(
    root: &Path,
    relative: &str,
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<String, FilesRefused> {
    let is_file = |path: &Path| -> Result<bool, FilesRefused> {
        match source {
            Some(source) => source
                .source_file_exists(path)
                .map_err(|e| failed(e.to_string())),
            None => Ok(path.is_file()),
        }
    };
    if is_owner_xml(relative) {
        return Ok(relative.to_string());
    }
    let mut folder = relative;
    while let Some(cut) = folder.rfind('/') {
        folder = &folder[..cut];
        let candidate = format!("{folder}.xml");
        if is_owner_xml(&candidate) {
            if is_file(&root.join(&candidate))? {
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
        if is_file(&root.join("Configuration.xml"))? {
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

/// Checks the listed files against the directory and finds their objects.
/// Reads the directory only: nothing here reaches a database.
pub fn select(root: &Path, files: &[String]) -> Result<FilesSelection, FilesRefused> {
    select_with_source(root, files, None)
}

pub(super) fn select_with_source(
    root: &Path,
    files: &[String],
    source: Option<&crate::module_blob::MetadataSourceContext>,
) -> Result<FilesSelection, FilesRefused> {
    if files.is_empty() {
        return Err(failed(
            "Не указаны файлы для загрузки (пути относительно --base-dir)".to_string(),
        ));
    }
    let original = source.and_then(|source| source.original_source());
    if let Some(original) = original {
        original
            .require_unchanged()
            .map_err(|e| failed(e.to_string()))?;
    }
    let canonical_root = if let Some(original) = original {
        original.canonical_root().to_owned()
    } else {
        fs::canonicalize(root)
            .ok()
            .filter(|path| path.is_dir())
            .ok_or_else(|| {
                failed(format!(
                    "каталог файлов конфигурации не найден: {}",
                    root.display()
                ))
            })?
    };
    let mut selected = Vec::<String>::new();
    let mut owners = Vec::<String>::new();
    for given in files {
        let relative = if let Some(original) = original {
            let path = PathBuf::from(given.replace('\\', "/"));
            let relative = if path.is_absolute() {
                original
                    .relative_path(&path)
                    .map_err(|e| failed(e.to_string()))?
            } else {
                given.replace('\\', "/")
            };
            // Exact captured spelling, no canonicalization through an alias.
            original
                .member(&relative)
                .map_err(|e| failed(e.to_string()))?;
            Some(relative)
        } else {
            relative_inside(root, &canonical_root, given)
        };
        let Some(relative) = relative else {
            return Err(failed(format!(
                "Загрузка невозможна: файл {given} находится вне каталога {}",
                root.display()
            )));
        };
        let path = root.join(&relative);
        let present = match source {
            Some(source) if original.is_some() => source
                .source_file_exists(&path)
                .map_err(|e| failed(e.to_string()))?,
            _ => path.exists(),
        };
        if !present {
            return Err(unsupported(format!(
                "Загрузка невозможна: файла {relative} нет в каталоге {}; удаление файлов и объектов \
                 частичной загрузкой {PLANNED}",
                root.display()
            )));
        }
        if original.is_none() && !path.is_file() {
            return Err(failed(format!(
                "Загрузка невозможна: {relative} в каталоге {} не файл",
                root.display()
            )));
        }
        // A link that leads out of the directory is a file outside it.
        let inside = original.is_some()
            || fs::canonicalize(&path)
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
        owners.push(owner_of_with_source(root, &relative, source)?);
        selected.push(relative);
    }
    owners.sort();
    owners.dedup();
    if let Some(original) = original {
        original
            .require_unchanged()
            .map_err(|e| failed(e.to_string()))?;
    }
    Ok(FilesSelection {
        files: selected,
        owners,
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
