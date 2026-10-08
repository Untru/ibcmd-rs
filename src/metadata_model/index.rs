//! The configuration-wide index a descriptor compiler resolves names with:
//! every metadata object and child object by its full name, and every
//! generated type by its name. Built once from all metadata XMLs of a tree.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use rayon::prelude::*;

use super::audit::descriptor_xmls;
use super::xml::{Element, MetadataXml};
use crate::parallel;

#[derive(Clone, Debug)]
pub struct ObjectEntry {
    pub kind: String,
    pub name: String,
    pub uuid: String,
    /// Full name, `Catalog.Банки` or `Catalog.Банки.Form.ФормаЭлемента`.
    pub full_name: String,
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct GeneratedType {
    pub name: String,
    pub category: String,
    pub type_id: String,
    pub value_id: String,
}

#[derive(Debug, Default)]
pub struct ConfigIndex {
    /// Objects with their own XML file (top-level objects, owned forms,
    /// templates, nested subsystems, recalculations), by full name.
    pub objects: HashMap<String, ObjectEntry>,
    /// The same objects by uuid.
    pub objects_by_uuid: HashMap<String, String>,
    /// Every uuid-bearing child inside an object's XML (`Catalog.X.Attribute.Y`,
    /// `Catalog.X.TabularSection.T.Attribute.Z`, `Enum.E.EnumValue.V`,
    /// `WebService.W.Operation.O.Parameter.P`, ...), full name -> uuid.
    pub children: HashMap<String, String>,
    /// Generated types by name (`CatalogRef.X`, `DefinedType.Y`, ...).
    pub generated_types: HashMap<String, GeneratedType>,
    /// The tree's `Configuration.xml` uuid.
    pub configuration_uuid: Option<String>,
    /// `<CompatibilityMode>` of `Configuration.xml` (`Version8_3_24`, ...).
    pub compatibility_mode: Option<String>,
}

/// The kinds that have a root family in a configuration, as `(folder, kind)`:
/// the plural the platform lays a family out in (`Catalogs`) and the kind of
/// what it holds (`Catalog`). The one table both directions read:
/// [`kind_of_collection`] and [`collection_of_kind`].
pub const ROOT_COLLECTIONS: &[(&str, &str)] = &[
    ("Languages", "Language"),
    ("Subsystems", "Subsystem"),
    ("StyleItems", "StyleItem"),
    ("Styles", "Style"),
    ("CommonPictures", "CommonPicture"),
    ("SessionParameters", "SessionParameter"),
    ("Roles", "Role"),
    ("CommonTemplates", "CommonTemplate"),
    ("FilterCriteria", "FilterCriterion"),
    ("CommonModules", "CommonModule"),
    ("CommonAttributes", "CommonAttribute"),
    ("ExchangePlans", "ExchangePlan"),
    ("XDTOPackages", "XDTOPackage"),
    ("WebServices", "WebService"),
    ("HTTPServices", "HTTPService"),
    ("WSReferences", "WSReference"),
    (
        ibcmd_schema::websocket_client::WebSocketClientLayout::FOLDER,
        ibcmd_schema::websocket_client::WebSocketClientLayout::KIND,
    ),
    ("EventSubscriptions", "EventSubscription"),
    ("ScheduledJobs", "ScheduledJob"),
    ("SettingsStorages", "SettingsStorage"),
    ("FunctionalOptions", "FunctionalOption"),
    ("FunctionalOptionsParameters", "FunctionalOptionsParameter"),
    ("DefinedTypes", "DefinedType"),
    ("CommonCommands", "CommonCommand"),
    ("CommandGroups", "CommandGroup"),
    ("Constants", "Constant"),
    ("CommonForms", "CommonForm"),
    ("Catalogs", "Catalog"),
    ("Documents", "Document"),
    ("DocumentNumerators", "DocumentNumerator"),
    ("Sequences", "Sequence"),
    ("DocumentJournals", "DocumentJournal"),
    ("Enums", "Enum"),
    ("Reports", "Report"),
    ("DataProcessors", "DataProcessor"),
    ("InformationRegisters", "InformationRegister"),
    ("AccumulationRegisters", "AccumulationRegister"),
    ("ChartsOfCharacteristicTypes", "ChartOfCharacteristicTypes"),
    ("ChartsOfAccounts", "ChartOfAccounts"),
    ("AccountingRegisters", "AccountingRegister"),
    ("ChartsOfCalculationTypes", "ChartOfCalculationTypes"),
    ("CalculationRegisters", "CalculationRegister"),
    ("BusinessProcesses", "BusinessProcess"),
    ("Tasks", "Task"),
    ("IntegrationServices", "IntegrationService"),
    ("Bots", "Bot"),
    ("ExternalDataSources", "ExternalDataSource"),
];

/// The kinds that live inside another object (`Forms`, `Templates`, ...): a
/// collection name but no root family.
pub const NESTED_COLLECTIONS: &[(&str, &str)] = &[
    ("Forms", "Form"),
    ("Templates", "Template"),
    ("Recalculations", "Recalculation"),
    ("Interfaces", "Interface"),
    ("PaletteColors", "PaletteColor"),
];

/// `Catalogs` -> `Catalog`; nested collections use the same folder names.
pub fn kind_of_collection(folder: &str) -> Option<&'static str> {
    ROOT_COLLECTIONS
        .iter()
        .chain(NESTED_COLLECTIONS)
        .find_map(|(collection, kind)| (*collection == folder).then_some(*kind))
}

/// `Catalog` -> `Catalogs`: the folder of a root family's kind. `None` for a
/// nested kind (`Form`) and for anything that is not a kind.
pub fn collection_of_kind(kind: &str) -> Option<&'static str> {
    ROOT_COLLECTIONS
        .iter()
        .find_map(|(collection, root_kind)| (*root_kind == kind).then_some(*collection))
}

struct Parsed {
    relative: String,
    path: PathBuf,
    kind: String,
    name: String,
    uuid: String,
    children: Vec<(String, String)>,
    generated: Vec<GeneratedType>,
    compatibility_mode: Option<String>,
}

impl ConfigIndex {
    pub fn build(root: &Path) -> Result<Self> {
        let paths = descriptor_xmls(root);
        let parsed = parallel::install(|| {
            paths
                .par_iter()
                .map(|path| parse_one(root, path))
                .collect::<Result<Vec<_>>>()
        })??;
        Ok(Self::assemble(parsed))
    }

    /// The same index from descriptor XMLs the caller already read: `(path,
    /// bytes)` for every file `descriptor_xmls(root)` lists, in its order. A
    /// base-free stage reads each metadata XML of its tree once.
    pub fn build_from_files(root: &Path, files: &[(PathBuf, Arc<Vec<u8>>)]) -> Result<Self> {
        let parsed = parallel::install(|| {
            files
                .par_iter()
                .map(|(path, bytes)| parse_bytes(root, path, bytes))
                .collect::<Result<Vec<_>>>()
        })??;
        Ok(Self::assemble(parsed))
    }

    fn assemble(parsed: Vec<Option<Parsed>>) -> Self {
        // Full names follow the folders: `Catalogs/X.xml` is `Catalog.X`,
        // `Catalogs/X/Forms/F.xml` is `Catalog.X.Form.F`.
        let mut full_by_relative: HashMap<String, String> = HashMap::new();
        let mut parsed = parsed.into_iter().flatten().collect::<Vec<_>>();
        parsed.sort_by_key(|item| item.relative.matches('/').count());
        let mut index = ConfigIndex::default();
        for item in parsed {
            let full_name = if item.kind == "Configuration" {
                index.configuration_uuid = Some(item.uuid.clone());
                index.compatibility_mode = item.compatibility_mode.clone();
                "Configuration".to_string()
            } else {
                let parts = item
                    .relative
                    .trim_end_matches(".xml")
                    .split('/')
                    .collect::<Vec<_>>();
                if parts.len() <= 2 {
                    format!("{}.{}", item.kind, item.name)
                } else {
                    let owner_relative = format!("{}.xml", parts[..parts.len() - 2].join("/"));
                    match full_by_relative.get(&owner_relative) {
                        Some(owner) => format!("{owner}.{}.{}", item.kind, item.name),
                        None => format!("{}.{}", item.kind, item.name),
                    }
                }
            };
            full_by_relative.insert(item.relative.clone(), full_name.clone());
            for (child, uuid) in item.children {
                index.children.insert(format!("{full_name}.{child}"), uuid);
            }
            for generated in item.generated {
                index
                    .generated_types
                    .insert(generated.name.clone(), generated);
            }
            index
                .objects_by_uuid
                .insert(item.uuid.clone(), full_name.clone());
            index.objects.insert(
                full_name.clone(),
                ObjectEntry {
                    kind: item.kind,
                    name: item.name,
                    uuid: item.uuid,
                    full_name,
                    path: item.path,
                },
            );
        }
        index
    }

    /// uuid of an object or child object by full name.
    pub fn uuid_of(&self, full_name: &str) -> Option<&str> {
        self.objects
            .get(full_name)
            .map(|entry| entry.uuid.as_str())
            .or_else(|| self.children.get(full_name).map(String::as_str))
    }

    pub fn generated_type(&self, name: &str) -> Option<&GeneratedType> {
        self.generated_types.get(name)
    }

    /// The configuration's compatibility mode as `(major, minor, release)`
    /// (`Version8_3_24` -> `(8, 3, 24)`); `None` when absent or `DontUse`.
    pub fn compatibility_version(&self) -> Option<(u32, u32, u32)> {
        let mode = self
            .compatibility_mode
            .as_deref()?
            .strip_prefix("Version")?;
        let mut parts = mode.split('_').map(|part| part.parse::<u32>().ok());
        Some((
            parts.next()??,
            parts.next()??,
            parts.next().flatten().unwrap_or(0),
        ))
    }
}

fn parse_one(root: &Path, path: &Path) -> Result<Option<Parsed>> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_bytes(root, path, &bytes)
}

/// `parse_one` of a file already read.
fn parse_bytes(root: &Path, path: &Path, bytes: &[u8]) -> Result<Option<Parsed>> {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    // A file this reader cannot parse is left out; its own compile reports it.
    let Ok(doc) = MetadataXml::parse(bytes) else {
        return Ok(None);
    };
    let Ok(object) = doc.object() else {
        return Ok(None);
    };
    let Some(uuid) = object.attr("uuid") else {
        return Ok(None);
    };
    let name = object
        .path(&["Properties", "Name"])
        .map(|name| name.text.clone())
        .unwrap_or_default();
    let mut children = Vec::new();
    let mut generated = Vec::new();
    collect_generated(object, &mut generated);
    if let Some(child_objects) = object.child("ChildObjects") {
        collect_children(child_objects, "", &mut children, &mut generated);
    }
    let compatibility_mode = (object.name == "Configuration")
        .then(|| object.path(&["Properties", "CompatibilityMode"]))
        .flatten()
        .map(|mode| mode.text.clone());
    Ok(Some(Parsed {
        relative,
        path: path.to_path_buf(),
        kind: object.name.clone(),
        name,
        uuid: uuid.to_ascii_lowercase(),
        children,
        generated,
        compatibility_mode,
    }))
}

fn collect_generated(object: &Element, out: &mut Vec<GeneratedType>) {
    let Some(info) = object.child("InternalInfo") else {
        return;
    };
    for generated in info.children_named("GeneratedType") {
        out.push(GeneratedType {
            name: generated.attr("name").unwrap_or_default().to_string(),
            category: generated.attr("category").unwrap_or_default().to_string(),
            type_id: generated
                .child_text("TypeId")
                .unwrap_or_default()
                .to_ascii_lowercase(),
            value_id: generated
                .child_text("ValueId")
                .unwrap_or_default()
                .to_ascii_lowercase(),
        });
    }
}

fn collect_children(
    child_objects: &Element,
    prefix: &str,
    out: &mut Vec<(String, String)>,
    generated: &mut Vec<GeneratedType>,
) {
    for child in &child_objects.children {
        let Some(uuid) = child.attr("uuid") else {
            continue;
        };
        let name = child
            .path(&["Properties", "Name"])
            .map(|name| name.text.as_str())
            .unwrap_or_default();
        let full = format!("{prefix}{}.{name}", child.name);
        out.push((full.clone(), uuid.to_ascii_lowercase()));
        collect_generated(child, generated);
        if let Some(nested) = child.child("ChildObjects") {
            collect_children(nested, &format!("{full}."), out, generated);
        }
    }
}
