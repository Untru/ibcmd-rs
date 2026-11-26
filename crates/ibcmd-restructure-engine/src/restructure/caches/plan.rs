//! The cache rows a new catalog or document (and a new tabular section of one) rewrites, from the rows
//! as they are.
//!
//! The functions take the **inflated** text of the `Params` rows and return the inflated text of the
//! changed ones; deflating, the `siVersions` guids and the write (`ParamsRewrite`) are the caller's.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::Brace;
use crate::restructure::caches::facts::{ObjectFacts, SectionFacts, tabular_class};
use crate::restructure::caches::help_props::HelpProps;
use crate::restructure::caches::members::{Members, SectionMember, catalog_attribute_nullable};
use crate::restructure::caches::names_tables::{NamesTables, NewTable};
use crate::restructure::caches::owner_map::OwnerMap;
use crate::restructure::caches::registry;
use crate::restructure::caches::root::{
    Collection, class_of_kind, collection_of, collections, kind_of_class,
};
use crate::restructure::caches::synonyms::Synonyms;
use crate::restructure::caches::type_index::{Entry, TypeIndex, TypeSlot};
use crate::restructure::caches::type_sets::TypeSets;
use crate::restructure::caches::xdto_types::{
    CatalogShape, DocumentShape, Family, RefNames, XdtoModel, attribute_property,
    section_property_line, section_row_block,
};

/// The names of the cache rows (`Params` `FileName`).
pub mod rows {
    pub const TYPE_INDEX: &str = "2203278d-ef4f-4f68-98f1-feb257d53ecc.si";
    pub const NAMES_TABLES: &str = "a07b62f0-1f01-484a-93d9-d42764cedac0.si";
    pub const OWNER_MAP: &str = "42ed49cc-765d-4314-bc2d-af425af7bf13.si";
    pub const SYNONYMS: &str = "facbfffe-feb2-4d30-8930-a557b185e5c4.si";
    pub const TYPE_SETS: &str = "fe8acd6a-22c9-4b5a-aeae-232a1c8324cb.si";
    pub const HELP_PROPS: &str = "c4629235-4823-4320-b8b5-1d08f4c6d612.si";
    pub const XDTO: &str = "ea13a2c9-0c2f-40fa-b855-710387e3271d.si";
    pub const REGISTRY: &str = crate::restructure::caches::registry::REGISTRY_ROW;
}

/// One rewritten cache row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheRow {
    pub name: &'static str,
    /// The inflated text (with the BOM the platform writes).
    pub text: Vec<u8>,
    /// Whether the text is what the platform writes byte for byte. `false` only for
    /// `c4629235` (see [`crate::restructure::caches::help_props`]): the entry is exact, the position
    /// of a few other entries is not.
    pub exact: bool,
}

/// The index of a reference object's generated types: the categories and their index.
pub fn reference_type_slots(facts: &ObjectFacts) -> Result<Vec<TypeSlot>> {
    facts
        .generated
        .iter()
        .map(|generated| {
            let index = match generated.category {
                "Object" => 0,
                "Ref" => 1,
                "Selection" => 2,
                "List" => 3,
                "Manager" => 4,
                other => bail!("the index of the generated type {other} is not measured"),
            };
            Ok(TypeSlot {
                type_id: generated.type_id.clone(),
                value_id: generated.value_id.clone(),
                index,
            })
        })
        .collect()
}

/// What a new catalog or document needs besides its own row.
pub struct NewObject<'a> {
    /// `Catalog` or `Document`.
    pub kind: &'a str,
    /// The object's descriptor row.
    pub descriptor: &'a Brace,
    /// The configuration's root row **after** the change (the object is in its collection).
    pub root: &'a Brace,
    /// The number `DBNames` gave the object's table (`_Reference11036` -> 11036).
    pub table_number: u64,
    /// Whether the object has a help page (a `Config` row `<uuid>.1`).
    pub has_help: bool,
    /// Whether the object has predefined items (a `Config` row `<uuid>.1c`). Not built: the platform
    /// adds a service property to the XDTO type of such an object.
    pub has_predefined: bool,
    /// The inflated cache rows by name.
    pub cache: &'a dyn Fn(&str) -> Option<Vec<u8>>,
    /// The name of an object of the kind by its uuid (the neighbours of the new one in the root).
    pub name_of: &'a dyn Fn(&str) -> Option<String>,
    /// The descriptor rows of the objects of the kind **after** the change, by uuid (the new object
    /// included): read only when the new object has tabular sections, whose entries in `2203278d`
    /// follow the traversal of all of them.
    pub descriptors: &'a dyn Fn(&str) -> Option<Brace>,
}

fn cached(cache: &dyn Fn(&str) -> Option<Vec<u8>>, name: &str) -> Result<Vec<u8>> {
    cache(name).with_context(|| format!("Params has no row {name}"))
}

/// The rows a new catalog or document rewrites: the object registry `1a621f0f`, `2203278d`, `a07b62f0`,
/// `42ed49cc` (catalogs), `facbfffe`, `fe8acd6a`, `c4629235` and the XDTO model `ea13a2c9`.
///
/// Refused (nothing is built): another kind; an object with predefined items; an object that lists
/// forms, templates, commands or any collection beyond its attributes and tabular sections (their
/// records reach rows this module does not build); an attribute of a defined or characteristic type.
pub fn new_object(input: &NewObject<'_>) -> Result<Vec<CacheRow>> {
    let kind = input.kind;
    if !["Catalog", "Document"].contains(&kind) {
        bail!("the caches of a new {kind} are not measured (Catalog and Document are)");
    }
    let class = class_of_kind(kind).context("no root class for the kind")?;
    let facts = ObjectFacts::parse(kind, input.descriptor)?;
    let listed = Members::parse(kind, input.descriptor)?;
    if input.has_predefined {
        bail!(
            "{kind} {} has predefined items: the properties the platform adds to its XDTO type are not built",
            facts.name
        );
    }
    if !listed.others.is_empty() {
        bail!(
            "{kind} {} lists collections beyond its attributes and tabular sections {:?}: the registry records of forms, templates and commands are not built",
            facts.name,
            listed.others
        );
    }
    let collections = collections(input.root);
    let members = collection_of(&collections, class)?;
    let position = members
        .objects
        .iter()
        .position(|object| *object == facts.uuid)
        .with_context(|| format!("the root does not list {kind} {}", facts.name))?;
    let predecessor = position
        .checked_sub(1)
        .map(|index| members.objects[index].as_str());
    let successor = members.objects.get(position + 1).map(String::as_str);
    let mut out = Vec::new();

    // 1a621f0f: the object registry
    out.push(CacheRow {
        name: rows::REGISTRY,
        text: registry::add_object(
            &cached(input.cache, rows::REGISTRY)?,
            kind,
            &facts,
            &listed,
            predecessor,
            successor,
        )?,
        exact: true,
    });

    // 2203278d: the type index
    let mut index = TypeIndex::parse(&cached(input.cache, rows::TYPE_INDEX)?)?;
    index.refill_section(
        class,
        &members.objects,
        vec![Entry {
            object: facts.uuid.clone(),
            types: reference_type_slots(&facts)?,
        }],
    )?;
    if !facts.sections.is_empty() {
        let section_class = tabular_class(kind).context("the kind has no tabular sections")?;
        let traversal = section_traversal(&collections, class, input.descriptors)?;
        let added = facts
            .sections
            .iter()
            .map(section_entry)
            .collect::<Vec<Entry>>();
        index.refill_section(section_class, &traversal, added)?;
    }
    out.push(CacheRow {
        name: rows::TYPE_INDEX,
        text: index.render(),
        exact: true,
    });

    // a07b62f0: names -> tables
    let mut names = NamesTables::parse(&cached(input.cache, rows::NAMES_TABLES)?)?;
    names.add_object(&NewTable {
        kind,
        name: &facts.name,
        object: &facts.uuid,
        table_number: input.table_number,
        ref_type_id: &facts.generated_type("Ref")?.type_id,
        predecessor,
        successor,
    })?;
    out.push(CacheRow {
        name: rows::NAMES_TABLES,
        text: names.render(),
        exact: true,
    });

    // 42ed49cc: owners of catalogs
    if kind == "Catalog" {
        let mut owner_map = OwnerMap::parse(&cached(input.cache, rows::OWNER_MAP)?)?;
        owner_map.add_catalog(&members.objects, &facts.uuid, &facts.references("Owners")?)?;
        out.push(CacheRow {
            name: rows::OWNER_MAP,
            text: owner_map.render(),
            exact: true,
        });
    }

    // facbfffe: presentations
    let mut synonyms = Synonyms::parse(&cached(input.cache, rows::SYNONYMS)?)?;
    for property in [
        "ListPresentation",
        "ExtendedListPresentation",
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "Explanation",
    ] {
        let pairs = facts.localized(property)?;
        if pairs.is_empty() {
            continue;
        }
        synonyms.add_object(property, &facts.uuid, &pairs, &collections, &kind_of_class)?;
    }
    out.push(CacheRow {
        name: rows::SYNONYMS,
        text: synonyms.render(),
        exact: true,
    });

    // fe8acd6a: sets of types
    let mut sets = TypeSets::parse(&cached(input.cache, rows::TYPE_SETS)?)?;
    let types: Vec<(u32, String)> = reference_type_slots(&facts)?
        .into_iter()
        .map(|slot| (slot.index, slot.type_id))
        .collect();
    sets.add_object(kind, &types)?;
    out.push(CacheRow {
        name: rows::TYPE_SETS,
        text: sets.render(),
        exact: true,
    });

    // c4629235: properties (entry exact, position approximate)
    let mut help = HelpProps::parse(&cached(input.cache, rows::HELP_PROPS)?)?;
    let entry = if kind == "Catalog" {
        HelpProps::catalog_entry(&facts, input.has_help)?
    } else {
        HelpProps::document_entry(&facts, input.has_help)?
    };
    help.add_entry_approximately(entry)?;
    out.push(CacheRow {
        name: rows::HELP_PROPS,
        text: help.render(),
        exact: false,
    });

    // ea13a2c9: the XDTO model
    let neighbour = |uuid: Option<&str>| -> Result<Option<String>> {
        uuid.map(|uuid| {
            (input.name_of)(uuid).with_context(|| format!("the name of {kind} {uuid} is unknown"))
        })
        .transpose()
    };
    let predecessor_name = neighbour(predecessor)?;
    let successor_name = neighbour(successor)?;
    let mut model = XdtoModel::parse(&cached(input.cache, rows::XDTO)?)?;
    let family = if kind == "Catalog" {
        Family::Catalog
    } else {
        Family::Document
    };
    let refs = RefNames::build(&names);
    let shape = (kind == "Catalog")
        .then(|| catalog_shape(&facts))
        .transpose()?;
    let mut attribute_lines = Vec::new();
    for attribute in &listed.attributes {
        let nullable = shape.is_some_and(|shape| {
            catalog_attribute_nullable(shape.hierarchical, shape.hierarchy_type, attribute.usage)
        });
        attribute_lines.push(attribute_property(
            &attribute.name,
            &attribute.pattern,
            nullable,
            &refs,
        )?);
    }
    let mut section_blocks = Vec::new();
    for section in &listed.sections {
        attribute_lines.push(section_property_line(family, &facts.name, &section.name));
        section_blocks.push(section_row_block(
            family,
            &facts.name,
            &section.name,
            &section_lines(section, &refs)?,
        ));
    }
    match shape {
        Some(shape) => model.add_catalog(
            &facts.name,
            shape,
            predecessor_name.as_deref(),
            successor_name.as_deref(),
            &attribute_lines,
            &section_blocks,
        )?,
        None => model.add_document(
            &facts.name,
            document_shape(&facts)?,
            predecessor_name.as_deref(),
            successor_name.as_deref(),
            &attribute_lines,
            &section_blocks,
        )?,
    }
    out.push(CacheRow {
        name: rows::XDTO,
        text: model.render(),
        exact: true,
    });
    Ok(out)
}

/// The property lines of the attributes of a tabular section's row type (never nullable).
fn section_lines(section: &SectionMember, refs: &RefNames) -> Result<Vec<String>> {
    section
        .attributes
        .iter()
        .map(|attribute| attribute_property(&attribute.name, &attribute.pattern, false, refs))
        .collect()
}

/// The shape of a catalog for the XDTO standard properties.
pub fn catalog_shape(facts: &ObjectFacts) -> Result<CatalogShape> {
    Ok(CatalogShape {
        hierarchical: facts.number("Hierarchical")? == 1,
        hierarchy_type: facts.number("HierarchyType")?,
        owners: facts.references("Owners")?.len(),
        code_length: facts.number("CodeLength")?,
        code_type: facts.number("CodeType")?,
        description_length: facts.number("DescriptionLength")?,
    })
}

/// The shape of a document for the XDTO standard properties.
pub fn document_shape(facts: &ObjectFacts) -> Result<DocumentShape> {
    Ok(DocumentShape {
        number_type: facts.number("NumberType")?,
    })
}

/// The tabular sections of every object of the root that has some, in the root's order
/// (`(section uuid)`), for the tabular-section collection of `owner_class` objects.
pub fn section_traversal(
    collections: &[Collection],
    owner_class: &str,
    descriptor: &dyn Fn(&str) -> Option<Brace>,
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    for collection in collections.iter().filter(|c| c.class == owner_class) {
        for object in &collection.objects {
            let row = descriptor(object)
                .with_context(|| format!("the descriptor of {object} is not available"))?;
            let facts = ObjectFacts::parse(
                kind_of_class(owner_class).context("an unknown owner class")?,
                &row,
            )?;
            for section in facts.sections {
                if seen.insert(section.uuid.clone(), ()).is_some() {
                    bail!("tabular section {} is listed twice", section.uuid);
                }
                out.push(section.uuid);
            }
        }
    }
    Ok(out)
}

/// The `2203278d` entry of a tabular section: its own type and the type of its row.
fn section_entry(section: &SectionFacts) -> Entry {
    let [(section_type, section_value), (row_type, row_value)] = section.types.clone();
    Entry {
        object: section.uuid.clone(),
        types: vec![
            TypeSlot {
                type_id: section_type,
                value_id: section_value,
                index: 0,
            },
            TypeSlot {
                type_id: row_type,
                value_id: row_value,
                index: 1,
            },
        ],
    }
}

/// What a new tabular section of a catalog or document needs.
pub struct NewSection<'a> {
    /// `Catalog` or `Document`.
    pub kind: &'a str,
    /// The root row (its objects of the kind are the ones to traverse).
    pub root: &'a Brace,
    /// The descriptors of the objects **after** the change, by uuid.
    pub descriptor: &'a dyn Fn(&str) -> Option<Brace>,
    /// The object that gets the section, and the section (uuid).
    pub owner: &'a str,
    pub section: &'a str,
    pub cache: &'a dyn Fn(&str) -> Option<Vec<u8>>,
}

/// What the new tabular sections of one stage need: all of one kind together, because the index of the
/// generated types is refilled in the hash order of *all* the keys of its section (the tabular sections of
/// every catalog, or of every document).
pub struct NewSections<'a> {
    pub kind: &'a str,
    pub root: &'a Brace,
    /// The descriptors of the objects **after** the change, by uuid.
    pub descriptor: &'a dyn Fn(&str) -> Option<Brace>,
    /// `(owner, section)` of each new section, the sections of one owner in the order of its descriptor: the
    /// section before a new one is there when the new one is placed.
    pub sections: &'a [(&'a str, &'a str)],
    pub cache: &'a dyn Fn(&str) -> Option<Vec<u8>>,
}

/// The rows a new tabular section rewrites: the object registry, `2203278d` and the XDTO model.
pub fn new_tabular_section(input: &NewSection<'_>) -> Result<Vec<CacheRow>> {
    new_tabular_sections(&NewSections {
        kind: input.kind,
        root: input.root,
        descriptor: input.descriptor,
        sections: &[(input.owner, input.section)],
        cache: input.cache,
    })
}

/// The rows the new tabular sections of a stage rewrite: the object registry, `2203278d` and the XDTO model.
pub fn new_tabular_sections(input: &NewSections<'_>) -> Result<Vec<CacheRow>> {
    let kind = input.kind;
    let class =
        class_of_kind(kind).with_context(|| format!("the caches of {kind} are not measured"))?;
    let section_class = tabular_class(kind).context("the kind has no tabular sections")?;
    let collections = collections(input.root);
    let traversal = section_traversal(&collections, class, input.descriptor)?;
    let refs = RefNames::build(&NamesTables::parse(&cached(
        input.cache,
        rows::NAMES_TABLES,
    )?)?);
    let mut registry_text = cached(input.cache, rows::REGISTRY)?;
    let mut model = XdtoModel::parse(&cached(input.cache, rows::XDTO)?)?;
    let mut entries = Vec::new();
    for (owner_uuid, section_uuid) in input.sections {
        let row = (input.descriptor)(owner_uuid)
            .with_context(|| format!("the descriptor of {owner_uuid} is not available"))?;
        let owner = ObjectFacts::parse(kind, &row)?;
        let position = owner
            .sections
            .iter()
            .position(|section| section.uuid == *section_uuid)
            .with_context(|| format!("{kind} {} has no section {section_uuid}", owner.name))?;
        let section = &owner.sections[position];
        let after = position
            .checked_sub(1)
            .map(|index| owner.sections[index].name.as_str());
        let listed = Members::parse(kind, &row)?;
        let member = listed
            .sections
            .get(position)
            .filter(|member| member.uuid == *section_uuid)
            .context("the attribute lists and the section list of the descriptor disagree")?;

        entries.push(section_entry(section));
        registry_text =
            registry::add_section(&registry_text, kind, owner_uuid, &listed.sections, position)?;
        let row_lines = section_lines(member, &refs)?;
        if kind == "Catalog" {
            model.add_tabular_section(&owner.name, &section.name, after, &row_lines)?;
        } else {
            model.add_document_section(&owner.name, &section.name, after, &row_lines)?;
        }
    }
    let mut index = TypeIndex::parse(&cached(input.cache, rows::TYPE_INDEX)?)?;
    index.refill_section(section_class, &traversal, entries)?;
    Ok(vec![
        CacheRow {
            name: rows::REGISTRY,
            text: registry_text,
            exact: true,
        },
        CacheRow {
            name: rows::TYPE_INDEX,
            text: index.render(),
            exact: true,
        },
        CacheRow {
            name: rows::XDTO,
            text: model.render(),
            exact: true,
        },
    ])
}
