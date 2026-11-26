//! The cache rows a staged change rewrites: new catalogs and documents, new tabular sections and new
//! attributes of existing objects, from the stored descriptors, the staged ones and the cache rows as
//! they are.
//!
//! This composes [`crate::restructure::caches::plan`] with the attribute additions, in the order the
//! rows depend on each other: the new object first, then the new tabular section, then the new
//! attributes (of objects and of sections). The rows come back as inflated text, one per rewritten
//! row; deflating, the `siVersions` guids and the guarded rewrite are the apply's.
//!
//! Built for one new catalog and one new document per stage (not two of a kind: the rows that list them are
//! refilled in the platform's hash order of *all* the keys, which a sequential composition has only for the
//! last addition). The new tabular sections of existing objects come together, however many: all those of a
//! kind are added in one refill of the index of the generated types (`plan::new_tabular_sections`), which is
//! not composed with the sections of a new object of the same kind. Anything else is refused.

use std::collections::BTreeMap;

use anyhow::{Context, Result, ensure};

use crate::metadata_model::brace::Brace;
use crate::restructure::caches::facts::ObjectFacts;
use crate::restructure::caches::members::{
    Member, Members, SECTION_ATTRIBUTES, attribute_class, catalog_attribute_nullable,
};
use crate::restructure::caches::names_tables::NamesTables;
use crate::restructure::caches::plan::{
    CacheRow, NewObject, NewSections, catalog_shape, document_shape, new_object,
    new_tabular_sections, rows,
};
use crate::restructure::caches::registry;
use crate::restructure::caches::xdto_types::{
    Family, RefNames, XdtoModel, attribute_property, document_standard_properties,
    standard_properties,
};

/// One object whose descriptor row changed.
#[derive(Clone, Debug)]
pub struct ChangedObject {
    /// `Catalog` or `Document`.
    pub kind: &'static str,
    pub uuid: String,
    /// The table number `DBNames` gave a **new** object (`_Reference11036` -> 11036).
    pub table_number: Option<u64>,
    /// A `Config` row `<uuid>.1` exists.
    pub has_help: bool,
    /// A `Config` row `<uuid>.1c` exists.
    pub has_predefined: bool,
}

/// A staged change.
pub struct Staged<'a> {
    /// The configuration's root row **after** the change.
    pub root: &'a Brace,
    /// The stored descriptor rows (`None` for an object that is new).
    pub before: &'a dyn Fn(&str) -> Option<Brace>,
    /// The descriptor rows after the change, of every object of the kinds.
    pub after: &'a dyn Fn(&str) -> Option<Brace>,
    pub changed: &'a [ChangedObject],
    /// The inflated cache rows as they are.
    pub cache: &'a dyn Fn(&str) -> Option<Vec<u8>>,
}

/// The rows the change rewrites, each once, in their final text.
pub fn rewrite(staged: &Staged<'_>) -> Result<Vec<CacheRow>> {
    let mut rows_now: BTreeMap<&'static str, CacheRow> = BTreeMap::new();
    let current = |rows_now: &BTreeMap<&'static str, CacheRow>, name: &str| -> Option<Vec<u8>> {
        rows_now
            .get(name)
            .map(|row| row.text.clone())
            .or_else(|| (staged.cache)(name))
    };
    let mut store = |rows_now: &mut BTreeMap<&'static str, CacheRow>, made: Vec<CacheRow>| {
        for row in made {
            match rows_now.get_mut(row.name) {
                Some(kept) => {
                    kept.text = row.text;
                    kept.exact &= row.exact;
                }
                None => {
                    rows_now.insert(row.name, row);
                }
            }
        }
    };

    // what is new
    let mut new_objects: Vec<&ChangedObject> = Vec::new();
    let mut existing: Vec<&ChangedObject> = Vec::new();
    for changed in staged.changed {
        if (staged.before)(&changed.uuid).is_none() {
            new_objects.push(changed);
        } else {
            existing.push(changed);
        }
    }
    for kind in ["Catalog", "Document"] {
        ensure!(
            new_objects.iter().filter(|o| o.kind == kind).count() <= 1,
            "more than one new {kind} in one stage: the caches are built for one"
        );
    }
    let mut section_operations: BTreeMap<&str, usize> = BTreeMap::new();

    // new objects, in the order of the kinds
    new_objects.sort_by_key(|o| o.kind != "Catalog");
    for object in &new_objects {
        let descriptor = (staged.after)(&object.uuid)
            .with_context(|| format!("no staged descriptor for {}", object.uuid))?;
        let members = Members::parse(object.kind, &descriptor)?;
        if !members.sections.is_empty() {
            *section_operations.entry(object.kind).or_default() += 1;
        }
        let kind = object.kind;
        let name_of = |uuid: &str| -> Option<String> {
            let row = (staged.after)(uuid)?;
            ObjectFacts::parse(kind, &row).ok().map(|facts| facts.name)
        };
        let snapshot = rows_now.clone();
        let cache = |name: &str| current(&snapshot, name);
        let made = new_object(&NewObject {
            kind,
            descriptor: &descriptor,
            root: staged.root,
            table_number: object
                .table_number
                .with_context(|| format!("no table number for the new {kind} {}", object.uuid))?,
            has_help: object.has_help,
            has_predefined: object.has_predefined,
            cache: &cache,
            name_of: &name_of,
            descriptors: staged.after,
        })?;
        store(&mut rows_now, made);
    }

    // new tabular sections and new attributes of existing objects: the checks and the list of what is added
    // first; the sections of a kind together, then the attributes
    let mut new_sections: BTreeMap<&'static str, Vec<(String, String)>> = BTreeMap::new();
    let mut attribute_jobs: Vec<AttributeJob> = Vec::new();
    for object in &existing {
        let kind = object.kind;
        let before = (staged.before)(&object.uuid).expect("checked above");
        let after = (staged.after)(&object.uuid)
            .with_context(|| format!("no staged descriptor for {}", object.uuid))?;
        let (was, is) = (
            Members::parse(kind, &before)?,
            Members::parse(kind, &after)?,
        );
        ensure!(
            is.others == was.others,
            "{kind} {} changes a collection beyond attributes and tabular sections",
            object.uuid
        );
        let known = |members: &[Member], uuid: &str| members.iter().any(|m| m.uuid == uuid);
        // attributes may only be added, in the same relative order
        let survivors: Vec<&str> = is
            .attributes
            .iter()
            .filter(|m| known(&was.attributes, &m.uuid))
            .map(|m| m.uuid.as_str())
            .collect();
        ensure!(
            survivors
                == was
                    .attributes
                    .iter()
                    .map(|m| m.uuid.as_str())
                    .collect::<Vec<_>>(),
            "{kind} {} removes or moves an attribute: only additions are built",
            object.uuid
        );
        // sections: new ones through new_tabular_section
        let was_sections: Vec<&str> = was.sections.iter().map(|s| s.uuid.as_str()).collect();
        let is_sections: Vec<&str> = is.sections.iter().map(|s| s.uuid.as_str()).collect();
        let kept: Vec<&str> = is_sections
            .iter()
            .copied()
            .filter(|uuid| was_sections.contains(uuid))
            .collect();
        ensure!(
            kept == was_sections,
            "{kind} {} removes or moves a tabular section: only additions are built",
            object.uuid
        );
        for section in is
            .sections
            .iter()
            .filter(|s| !was_sections.contains(&s.uuid.as_str()))
        {
            ensure!(
                section_operations.get(kind).copied().unwrap_or(0) == 0,
                "new tabular sections of {kind} objects together with the sections of a new {kind}: the index of the generated types is refilled once, for one of them"
            );
            new_sections
                .entry(kind)
                .or_default()
                .push((object.uuid.clone(), section.uuid.clone()));
        }
        // new attributes of the object
        let added: Vec<Member> = is
            .attributes
            .iter()
            .filter(|m| !known(&was.attributes, &m.uuid))
            .cloned()
            .collect();
        if !added.is_empty() {
            attribute_jobs.push(AttributeJob {
                kind,
                owner: object.uuid.clone(),
                descriptor: after.clone(),
                all: is.attributes.clone(),
                added,
                section: None,
            });
        }
        // new attributes of the sections that were there
        for section in &is.sections {
            let Some(old) = was.sections.iter().find(|s| s.uuid == section.uuid) else {
                continue;
            };
            let added: Vec<Member> = section
                .attributes
                .iter()
                .filter(|m| !known(&old.attributes, &m.uuid))
                .cloned()
                .collect();
            let survivors: Vec<&str> = section
                .attributes
                .iter()
                .filter(|m| known(&old.attributes, &m.uuid))
                .map(|m| m.uuid.as_str())
                .collect();
            ensure!(
                survivors
                    == old
                        .attributes
                        .iter()
                        .map(|m| m.uuid.as_str())
                        .collect::<Vec<_>>(),
                "section {} of {kind} {} removes or moves an attribute: only additions are built",
                section.name,
                object.uuid
            );
            if !added.is_empty() {
                attribute_jobs.push(AttributeJob {
                    kind,
                    owner: object.uuid.clone(),
                    descriptor: after.clone(),
                    all: section.attributes.clone(),
                    added,
                    section: Some((section.uuid.clone(), section.name.clone())),
                });
            }
        }
    }
    for (kind, sections) in &new_sections {
        let snapshot = rows_now.clone();
        let cache = |name: &str| current(&snapshot, name);
        let pairs: Vec<(&str, &str)> = sections
            .iter()
            .map(|(owner, section)| (owner.as_str(), section.as_str()))
            .collect();
        let made = new_tabular_sections(&NewSections {
            kind,
            root: staged.root,
            descriptor: staged.after,
            sections: &pairs,
            cache: &cache,
        })?;
        store(&mut rows_now, made);
    }
    for job in &attribute_jobs {
        let added: Vec<&Member> = job.added.iter().collect();
        add_attributes(
            &mut rows_now,
            &current,
            &mut store,
            job.kind,
            &job.owner,
            &job.descriptor,
            &job.all,
            &added,
            job.section
                .as_ref()
                .map(|(uuid, name)| (uuid.as_str(), name.as_str())),
        )?;
    }
    Ok(rows_now.into_values().collect())
}

/// New attributes of an existing object (`section`: of one of its tabular sections), to be added once the new
/// tabular sections of the stage are in.
struct AttributeJob {
    kind: &'static str,
    owner: String,
    descriptor: Brace,
    all: Vec<Member>,
    added: Vec<Member>,
    section: Option<(String, String)>,
}

type Store<'s> = dyn FnMut(&mut BTreeMap<&'static str, CacheRow>, Vec<CacheRow>) + 's;
type Current<'s> = dyn Fn(&BTreeMap<&'static str, CacheRow>, &str) -> Option<Vec<u8>> + 's;

/// The registry records and the XDTO lines of new attributes of an existing object (`section`:
/// of one of its tabular sections, as `(uuid, name)`).
#[allow(clippy::too_many_arguments)]
fn add_attributes(
    rows_now: &mut BTreeMap<&'static str, CacheRow>,
    current: &Current<'_>,
    store: &mut Store<'_>,
    kind: &'static str,
    owner: &str,
    descriptor: &Brace,
    all: &[Member],
    added: &[&Member],
    section: Option<(&str, &str)>,
) -> Result<()> {
    let facts = ObjectFacts::parse(kind, descriptor)?;
    let registry_text = current(rows_now, rows::REGISTRY).context("Params has no registry row")?;
    let class = match section {
        Some(_) => SECTION_ATTRIBUTES,
        None => attribute_class(kind).context("no attribute class")?,
    };
    let registry_owner = section.map_or(owner, |(uuid, _)| uuid);
    let uuids: Vec<&str> = added.iter().map(|m| m.uuid.as_str()).collect();
    let text = registry::add_attributes(&registry_text, registry_owner, class, all, &uuids)?;
    let mut made = vec![CacheRow {
        name: rows::REGISTRY,
        text,
        exact: true,
    }];

    let names = NamesTables::parse(
        &current(rows_now, rows::NAMES_TABLES).context("Params has no names row")?,
    )?;
    let refs = RefNames::build(&names);
    let mut model =
        XdtoModel::parse(&current(rows_now, rows::XDTO).context("Params has no XDTO row")?)?;
    let family = if kind == "Catalog" {
        Family::Catalog
    } else {
        Family::Document
    };
    let (type_name, standard) = match section {
        Some((_, name)) => (
            format!(
                "{}.{}.{name}",
                if kind == "Catalog" {
                    "CatalogTabularSectionRow"
                } else {
                    "DocumentTabularSectionRow"
                },
                facts.name
            ),
            0,
        ),
        None => {
            let object_type = if family == Family::Catalog {
                "CatalogObject"
            } else {
                "DocumentObject"
            };
            let standard = if kind == "Catalog" {
                standard_properties(&facts.name, catalog_shape(&facts)?).len()
            } else {
                document_standard_properties(&facts.name, document_shape(&facts)?).len()
            };
            (format!("{object_type}.{}", facts.name), standard)
        }
    };
    let shape = (kind == "Catalog" && section.is_none())
        .then(|| catalog_shape(&facts))
        .transpose()?;
    for member in added {
        let position = all
            .iter()
            .position(|m| m.uuid == member.uuid)
            .context("an added attribute is not in the list")?;
        let nullable = shape.is_some_and(|shape| {
            catalog_attribute_nullable(shape.hierarchical, shape.hierarchy_type, member.usage)
        });
        let line = attribute_property(&member.name, &member.pattern, nullable, &refs)?;
        model.insert_property_lines(&type_name, standard + position, &[line])?;
    }
    made.push(CacheRow {
        name: rows::XDTO,
        text: model.render(),
        exact: true,
    });
    store(rows_now, made);
    Ok(())
}
