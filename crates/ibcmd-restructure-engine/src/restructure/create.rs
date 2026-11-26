//! A new catalog or document (S1-F, `docs/apply/new-object.md`): finding it in the staged image, planning its
//! tables, names, schema entry and caches.
//!
//! What a new object needs beyond its own descriptor is read from the stored configuration and put in
//! [`Inputs::objects`]: the descriptors of every catalog, document, common attribute and defined type
//! (the traversal of the tabular sections, the common attributes that apply, the types an attribute names).
//!
//! The platform numbers the tables and fields of a created object **after** those of the changed objects
//! (`new-object.md` 1.3), the main table first, then the attributes, then each tabular section with its line
//! number and its attributes; every table is created in the new generation, nothing is copied or dropped.

use std::collections::BTreeSet;

use anyhow::{Context as _, Result, bail};

use crate::compiler::families::assets::{SourceAssetRegistry, SourceAssetRole};
use crate::metadata_model::brace::{Brace, parse_row};
use crate::restructure::caches::facts::ObjectFacts as SlotFacts;
use crate::restructure::caches::members::Members;
use crate::restructure::caches::names_tables::NamesTables;
use crate::restructure::caches::root::{Collection, collection_of, collections};
use crate::restructure::common::{COMMON_ATTRIBUTE_CLASS, CommonAttributes};
use crate::restructure::entry::{EntryInput, Kind, RefTables, main_entry};
use crate::restructure::names::inflate;
use crate::restructure::object::ObjectKind;
use crate::restructure::plan::{
    Inputs, ObjectPlan, Running, StagedImage, TablePlan, configuration_uuid,
};
use crate::restructure::schema::{DbSchema, TableView, physical_tables};

/// The root collections whose descriptors the plan of a created object reads.
const CONTEXT_CLASSES: [&str; 4] = [
    "cf4abea6-37b2-11d4-940f-008048da11f9", // catalogs
    "061d872a-5787-460e-95ac-ed74ea3a3e84", // documents
    COMMON_ATTRIBUTE_CLASS,
    "c045099e-13b9-4fb6-9d50-fca00202971e", // defined types
];

const NAMES_TABLES_ROW: &str = "a07b62f0-1f01-484a-93d9-d42764cedac0.si";

/// The bare-uuid descriptors the stage adds: the files of `ConfigSave` named by a uuid that `Config` does not
/// have.
pub fn new_descriptor_names(image: &StagedImage) -> BTreeSet<String> {
    image
        .new_files
        .difference(&image.old_files)
        .filter(|name| name.len() == 36 && !name.contains('.'))
        .cloned()
        .collect()
}

/// The uuids of the descriptors a created object's plan reads, from the configuration's descriptor: the
/// objects of the four root collections in `CONTEXT_CLASSES`.
pub fn context_uuids(configuration_row: &[u8]) -> Result<Vec<String>> {
    let text = inflate(configuration_row).unwrap_or_else(|_| configuration_row.to_vec());
    let root = parse_row(&text).context("the configuration's descriptor is not brace text")?;
    let all: Vec<Collection> = collections(&root);
    Ok(all
        .into_iter()
        .filter(|collection| CONTEXT_CLASSES.contains(&collection.class.as_str()))
        .flat_map(|collection| collection.objects)
        .collect())
}

/// A new catalog or document of the staged image, read.
#[derive(Clone, Debug)]
pub struct Created {
    pub uuid: String,
    pub kind: ObjectKind,
    /// The staged descriptor row.
    pub row: Brace,
    /// The slots of the owner record (properties) and the attributes and sections.
    pub facts: SlotFacts,
    pub members: Members,
    /// The staged files of the object besides its descriptor (`<uuid>.0`, `<uuid>.1`, ...).
    pub files: Vec<String>,
}

fn kind_name(kind: ObjectKind) -> &'static str {
    match kind {
        ObjectKind::Catalog => "Catalog",
        ObjectKind::Document => "Document",
    }
}

fn entry_kind(kind: ObjectKind) -> Kind {
    match kind {
        ObjectKind::Catalog => Kind::Catalog,
        ObjectKind::Document => Kind::Document,
    }
}

fn row_text(stored: &[u8]) -> Vec<u8> {
    inflate(stored).unwrap_or_else(|_| stored.to_vec())
}

/// The new catalogs and documents of the staged image. A new descriptor of another kind of object is left
/// alone: its file stays an added file, which the plan refuses.
pub fn find_created(image: &StagedImage) -> Result<Vec<Created>> {
    let mut out = Vec::new();
    for uuid in new_descriptor_names(image) {
        let Some(stored) = image.new_descriptors.get(&uuid) else {
            continue;
        };
        let row =
            parse_row(&row_text(stored)).with_context(|| format!("staged descriptor {uuid}"))?;
        let tag = row
            .as_list()
            .and_then(|root| root.get(1))
            .and_then(Brace::as_list)
            .and_then(|record| record.first())
            .and_then(Brace::as_atom);
        let kind = match tag {
            Some("56" | "57") => ObjectKind::Catalog,
            Some("40") => ObjectKind::Document,
            _ => continue,
        };
        let facts = SlotFacts::parse(kind_name(kind), &row)
            .with_context(|| format!("the new {} {uuid}", kind.label()))?;
        let members = Members::parse(kind_name(kind), &row)
            .with_context(|| format!("the new {} {}", kind.label(), facts.name))?;
        let prefix = format!("{uuid}.");
        let files = image
            .new_files
            .iter()
            .filter(|name| name.to_ascii_lowercase().starts_with(&prefix))
            .cloned()
            .collect();
        out.push(Created {
            uuid,
            kind,
            row,
            facts,
            members,
            files,
        });
    }
    Ok(out)
}

/// A stage creates at most one catalog and one document: how the platform numbers several of a kind, and whether
/// its header then moves once or more, is not traced (n5 has one of each). Fails closed.
pub fn check_count(created: &[Created]) -> Result<()> {
    for kind in [ObjectKind::Catalog, ObjectKind::Document] {
        let names: Vec<&str> = created
            .iter()
            .filter(|item| item.kind == kind)
            .map(|item| item.facts.name.as_str())
            .collect();
        if names.len() > 1 {
            bail!(
                "the stage creates {} new {} ({}): the numbering of several is not traced, one catalog and one document at a time",
                names.len(),
                if matches!(kind, ObjectKind::Catalog) {
                    "catalogs"
                } else {
                    "documents"
                },
                names.join(", ")
            );
        }
    }
    Ok(())
}

/// The staged image without the created objects' descriptors and files: what the planner of changed objects
/// looks at.
pub fn without_created(image: &StagedImage, created: &[Created]) -> StagedImage {
    let mut rest = image.clone();
    for item in created {
        rest.new_descriptors.remove(&item.uuid);
        rest.new_files.remove(&item.uuid);
        for file in &item.files {
            rest.new_files.remove(file);
        }
    }
    rest
}

/// What the plan of created objects reads besides the objects themselves.
pub struct Context {
    /// The reference and owner tables, defined types included.
    pub refs: RefTables,
    pub common: CommonAttributes,
    /// The staged configuration descriptor, which lists the created objects.
    pub configuration: Brace,
    descriptors: std::collections::BTreeMap<String, Vec<u8>>,
}

impl Context {
    /// A descriptor row of the configuration after the stage (staged first, then stored).
    pub fn descriptor(&self, uuid: &str) -> Option<Brace> {
        parse_row(&row_text(self.descriptors.get(&uuid.to_ascii_lowercase())?)).ok()
    }
}

/// Builds the context from the plan's input.
pub fn context(inputs: &Inputs) -> Result<Context> {
    let mut descriptors = std::collections::BTreeMap::new();
    for (name, row) in &inputs.staged.old_descriptors {
        descriptors.insert(name.to_ascii_lowercase(), row.clone());
    }
    for (name, row) in &inputs.objects {
        descriptors.insert(name.to_ascii_lowercase(), row.clone());
    }
    for (name, row) in &inputs.staged.new_descriptors {
        descriptors.insert(name.to_ascii_lowercase(), row.clone());
    }
    let configuration = configuration_uuid(&inputs.root_row)?;
    let configuration = descriptors
        .get(&configuration)
        .with_context(|| format!("the configuration's descriptor {configuration} was not read"))?;
    let configuration =
        parse_row(&row_text(configuration)).context("the configuration's descriptor")?;
    let all = collections(&configuration);

    let names_row = inputs
        .cache_rows
        .iter()
        .find(|(name, _)| name == NAMES_TABLES_ROW)
        .map(|(_, row)| row_text(row))
        .context("Params has no names and tables row")?;
    let mut refs = RefTables::build(&NamesTables::parse(&names_row)?);
    let in_context = |uuid: &str| -> Result<Brace> {
        let row = descriptors
            .get(&uuid.to_ascii_lowercase())
            .with_context(|| format!("the descriptor {uuid} was not read"))?;
        parse_row(&row_text(row)).with_context(|| format!("descriptor {uuid}"))
    };
    if let Ok(defined) = collection_of(&all, "c045099e-13b9-4fb6-9d50-fca00202971e") {
        for uuid in &defined.objects {
            refs.add_defined(&in_context(uuid)?)?;
        }
    }
    let order = collection_of(&all, COMMON_ATTRIBUTE_CLASS)
        .map(|collection| collection.objects.clone())
        .unwrap_or_default();
    let common = CommonAttributes::from_rows(&order, &|uuid: &str| in_context(uuid).ok())?;
    Ok(Context {
        refs,
        common,
        configuration,
        descriptors,
    })
}

/// The created objects in the order the platform numbers them: the kinds in the configuration's order, each
/// kind in the order the staged configuration lists it.
pub fn in_platform_order(created: Vec<Created>, context: &Context) -> Result<Vec<Created>> {
    let all = collections(&context.configuration);
    let mut keyed = Vec::new();
    for item in created {
        let class = match item.kind {
            ObjectKind::Catalog => CONTEXT_CLASSES[0],
            ObjectKind::Document => CONTEXT_CLASSES[1],
        };
        let position = collection_of(&all, class)?
            .objects
            .iter()
            .position(|uuid| *uuid == item.uuid)
            .with_context(|| {
                format!(
                    "the staged configuration does not list the new {} {}",
                    item.kind.label(),
                    item.facts.name
                )
            })?;
        keyed.push(((item.kind, position), item));
    }
    keyed.sort_by_key(|(key, _)| *key);
    Ok(keyed.into_iter().map(|(_, item)| item).collect())
}

/// Hands out the numbers of the created objects in the platform's order (traced: cases c, d, n1-n5): first the
/// main tables of all of them (kinds in the configuration's order), then the attributes of each, then the tabular
/// sections of each with their line number and attributes.
pub fn allocate(running: &mut Running, created: &[Created]) -> Result<()> {
    for item in created {
        running.allocate(&item.uuid, entry_kind(item.kind).table_kind())?;
    }
    for item in created {
        for attribute in &item.members.attributes {
            running.allocate(&attribute.uuid, "Fld")?;
        }
    }
    for item in created {
        for section in &item.members.sections {
            running.allocate(&section.uuid, "VT")?;
            running.allocate(&section.uuid, "LineNo")?;
            for attribute in &section.attributes {
                running.allocate(&attribute.uuid, "Fld")?;
            }
        }
    }
    Ok(())
}

/// The place of every catalog and document in the platform's walk of the staged configuration: the kind, then the
/// place in the configuration's own list. A changed object takes the numbers of its new attributes in this walk, and
/// a created catalog takes one number that no entry records (`Running::reserve`): traced on N1-N7, where a catalog
/// created before a changed one takes it before the changed one's attribute and a catalog created after takes it
/// after. The created objects' own numbers come before the walk (`allocate`).
pub fn walk_positions(
    context: &Context,
) -> Result<std::collections::HashMap<String, (ObjectKind, usize)>> {
    let all = collections(&context.configuration);
    let mut places = std::collections::HashMap::new();
    for (class, kind) in [
        (CONTEXT_CLASSES[0], ObjectKind::Catalog),
        (CONTEXT_CLASSES[1], ObjectKind::Document),
    ] {
        for (position, uuid) in collection_of(&all, class)?.objects.iter().enumerate() {
            places.insert(uuid.to_ascii_lowercase(), (kind, position));
        }
    }
    Ok(places)
}

/// Plans one created object whose numbers [`allocate`] has handed out: its schema entry and its tables.
/// Returns the plan of the object and the entry to put before `ConfigChngR`.
pub fn plan_created(
    running: &Running,
    schema: &DbSchema,
    inputs: &Inputs,
    context: &Context,
    item: &Created,
) -> Result<(ObjectPlan, Brace)> {
    let label = item.kind.label();
    let name = &item.facts.name;
    if !item.members.others.is_empty() {
        bail!(
            "the new {label} {name} lists collections beyond its attributes and tabular sections {:?}: forms, templates and commands are not built",
            item.members.others
        );
    }
    if item
        .files
        .iter()
        .any(|file| file.to_ascii_lowercase().ends_with(".1c"))
    {
        bail!("the new {label} {name} has predefined items: not built");
    }
    // Its other files are modules and the help page: rows the apply moves without reading them.
    for file in &item.files {
        let suffix = file.rsplit('.').next().unwrap_or_default();
        let admitted = SourceAssetRegistry
            .route_by_suffix(kind_name(item.kind), suffix)
            .is_some_and(|route| {
                matches!(
                    route.role(),
                    SourceAssetRole::ObjectModule
                        | SourceAssetRole::ManagerModule
                        | SourceAssetRole::Help
                )
            });
        if !admitted {
            bail!(
                "the new {label} {name} has the file {file}, which is not a module or a help page: not built"
            );
        }
    }
    if item.facts.number("DataHistory")? != 0 {
        bail!("the new {label} {name} keeps data history: not built");
    }
    // A common attribute that applies to every new object must be a data separator: another one adds
    // properties to the XDTO type that are not modelled (`new-object.md` 1.2).
    for attribute in context.common.automatic() {
        if !attribute.separates_data {
            bail!(
                "the common attribute {} applies to every new object and is no data separator: not built",
                attribute.name
            );
        }
    }
    for attribute in &context.common.attributes {
        let changed = match (
            inputs.staged.new_descriptors.get(&attribute.uuid),
            inputs.objects.get(&attribute.uuid),
        ) {
            (Some(staged), Some(stored)) => row_text(staged) != row_text(stored),
            (Some(_), None) => true,
            _ => false,
        };
        if changed {
            bail!(
                "the stage changes the common attribute {}: a new object is not planned with it",
                attribute.name
            );
        }
    }

    let entry = main_entry(&EntryInput {
        kind: entry_kind(item.kind),
        facts: &item.facts,
        members: &item.members,
        names: &running.names_after,
        common: &context.common,
        refs: &context.refs,
    })?;
    let view = TableView::new(&entry)?;
    let object = view.name().to_owned();
    if schema.position(&object).is_some() {
        bail!("the schema has a table {object} already");
    }
    let tables = physical_tables(&view)?
        .into_iter()
        .map(TablePlan::created)
        .collect();
    Ok((
        ObjectPlan {
            kind: item.kind,
            object,
            object_uuid: item.uuid.clone(),
            object_name: name.clone(),
            additions: Vec::new(),
            removals: Vec::new(),
            widenings: Vec::new(),
            switches: Vec::new(),
            sections: Vec::new(),
            tables,
            alter: Vec::new(),
            created: true,
        },
        entry,
    ))
}

/// The cache rows of the created objects, on top of the rows the changed objects' updates left (`caches`):
/// `caches::change::rewrite` over the created objects, its rows replacing or joining the plan's cache updates.
pub fn extend_caches(
    inputs: &Inputs,
    context: &Context,
    created: &[Created],
    names_after: &crate::restructure::names::DbNames,
    options: &crate::restructure::plan::PlanOptions,
    caches: &mut Vec<crate::restructure::plan::CacheUpdate>,
) -> Result<()> {
    use crate::restructure::caches::change::{ChangedObject, Staged, rewrite};
    use crate::restructure::caches::plan::rows;
    use crate::restructure::names::deflate;
    use crate::restructure::plan::CacheUpdate;

    let changed = created
        .iter()
        .map(|item| {
            Ok(ChangedObject {
                kind: kind_name(item.kind),
                uuid: item.uuid.clone(),
                table_number: Some(
                    names_after
                        .number_of(&item.uuid, entry_kind(item.kind).table_kind())
                        .with_context(|| format!("{} has no table number", item.facts.name))?,
                ),
                has_help: item
                    .files
                    .iter()
                    .any(|file| file.to_ascii_lowercase() == format!("{}.1", item.uuid)),
                has_predefined: false,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let made = {
        let cache = |name: &str| -> Option<Vec<u8>> {
            if let Some(update) = caches.iter().find(|update| update.row_name == name) {
                return inflate(&update.row).ok();
            }
            inputs
                .cache_rows
                .iter()
                .find(|(row, _)| row == name)
                .map(|(_, stored)| row_text(stored))
        };
        let none = |_: &str| -> Option<Brace> { None };
        let after = |uuid: &str| context.descriptor(uuid);
        rewrite(&Staged {
            root: &context.configuration,
            before: &none,
            after: &after,
            changed: &changed,
            cache: &cache,
        })?
    };
    for row in made {
        if (options.skip_xdto && row.name == rows::XDTO)
            || (options.skip_registry && row.name == rows::REGISTRY)
        {
            continue;
        }
        let update = CacheUpdate {
            row_name: row.name.to_owned(),
            row: deflate(&row.text)?,
            what: format!(
                "rows of the new {}{}",
                created
                    .iter()
                    .map(|item| item.facts.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                if row.exact {
                    ""
                } else {
                    " (the order of some entries is approximate)"
                }
            ),
            set_creation: true,
        };
        match caches
            .iter_mut()
            .find(|other| other.row_name == update.row_name)
        {
            Some(slot) => *slot = update,
            None => caches.push(update),
        }
    }
    Ok(())
}
