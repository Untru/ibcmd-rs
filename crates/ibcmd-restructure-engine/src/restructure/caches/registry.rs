//! `1a621f0f-5568-4183-bd9f-f6ef670e7090.si`: the object registry (the "search information" of the
//! configuration), the records of a new catalog or document, of a new tabular section, and of new
//! attributes.
//!
//! (A changed synonym of an existing object is written by [`set_synonyms`].)
//!
//! The row lists every metadata object in pre-order: `uuid, owner uuid, kind, "name", {synonyms},
//! flag, flag`, the *kind* being the index of the object's class id in the row's own class list (the
//! editing of the text is [`crate::mssql_config_apply::si`], measured there for forms and templates).
//! What a new object adds, measured on case c (a catalog), case d (a document with an attribute and a
//! tabular section with an attribute) and case h (a tabular section, attributes):
//!
//! - the object's record stands after the subtree of the object the root lists before it (before the
//!   next one when it is the first), its owner is the configuration;
//! - its two flags are `IncludeHelpInContents` and `UseStandardCommands` of the descriptor (checked on
//!   all 115 catalogs and 25 documents of the БСП corpus);
//! - its attributes follow it, then each tabular section with the attributes of the section; an
//!   attribute has the flags `0,0`;
//! - the synonym block is the object's own synonym (`{1,0}` when empty), languages sorted.

use anyhow::{Context, Result, bail, ensure};

use crate::mssql_config_apply::si::{self, Insertion, NewRecord, SiMain};
use crate::restructure::caches::facts::{ObjectFacts, tabular_class};
use crate::restructure::caches::members::{
    Member, Members, SECTION_ATTRIBUTES, SectionMember, attribute_class,
};
use crate::restructure::caches::root::class_of_kind;

/// The registry row.
pub const REGISTRY_ROW: &str = "1a621f0f-5568-4183-bd9f-f6ef670e7090.si";

fn escaped(text: &str) -> String {
    text.replace('"', "\"\"")
}

fn synonyms(pairs: &[(String, String)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(language, text)| (language.clone(), escaped(text)))
        .collect()
}

fn kind_of(main: &SiMain, class: &str) -> Result<usize> {
    main.kind_of_class(class)
        .with_context(|| format!("the registry has no class {class}"))
}

fn attribute_record(
    main: &SiMain,
    member: &Member,
    parent: &str,
    class: &str,
) -> Result<NewRecord> {
    Ok(NewRecord {
        uuid: member.uuid.clone(),
        parent: parent.to_owned(),
        kind: kind_of(main, class)?,
        name: member.name.clone(),
        synonyms: synonyms(&member.synonyms),
        flags: (0, 0),
    })
}

fn section_records(
    main: &SiMain,
    section: &SectionMember,
    owner: &str,
    section_class: &str,
) -> Result<Vec<NewRecord>> {
    let mut out = vec![NewRecord {
        uuid: section.uuid.clone(),
        parent: owner.to_owned(),
        kind: kind_of(main, section_class)?,
        name: section.name.clone(),
        synonyms: synonyms(&section.synonyms),
        flags: (0, 0),
    }];
    for attribute in &section.attributes {
        out.push(attribute_record(
            main,
            attribute,
            &section.uuid,
            SECTION_ATTRIBUTES,
        )?);
    }
    Ok(out)
}

/// The records of a new `kind` object: itself, its attributes, its tabular sections with theirs.
pub fn object_records(
    main: &SiMain,
    kind: &str,
    facts: &ObjectFacts,
    members: &Members,
    owner: &str,
) -> Result<Vec<NewRecord>> {
    let class = class_of_kind(kind).with_context(|| format!("no root class for {kind}"))?;
    let flag = |name: &str| -> Result<u8> {
        match facts.number(name)? {
            0 => Ok(0),
            1 => Ok(1),
            other => bail!("{name} of {kind} {} is {other}, not a flag", facts.name),
        }
    };
    let mut out = vec![NewRecord {
        uuid: facts.uuid.clone(),
        parent: owner.to_owned(),
        kind: kind_of(main, class)?,
        name: facts.name.clone(),
        synonyms: synonyms(&facts.synonym),
        flags: (flag("IncludeHelpInContents")?, flag("UseStandardCommands")?),
    }];
    let attributes = attribute_class(kind).context("the kind has no attribute class")?;
    for attribute in &members.attributes {
        out.push(attribute_record(main, attribute, &facts.uuid, attributes)?);
    }
    let sections = tabular_class(kind).context("the kind has no tabular sections")?;
    for section in &members.sections {
        out.extend(section_records(main, section, &facts.uuid, sections)?);
    }
    Ok(out)
}

/// The registry text with the records of a new object at their place (`predecessor` / `successor`: the
/// objects of the kind the root lists around it).
pub fn add_object(
    text: &[u8],
    kind: &str,
    facts: &ObjectFacts,
    members: &Members,
    predecessor: Option<&str>,
    successor: Option<&str>,
) -> Result<Vec<u8>> {
    let main = si::parse(text).context("the object registry does not parse")?;
    let owner = main
        .records
        .first()
        .context("the object registry is empty")?
        .uuid
        .clone();
    let class = class_of_kind(kind).context("no root class for the kind")?;
    let at = main.place(&owner, kind_of(&main, class)?, predecessor, successor)?;
    let records = object_records(&main, kind, facts, members, &owner)?;
    si::insert_records(text, &main, &[Insertion { at, records }])
}

/// The registry text with a new tabular section (and its attributes) of `owner`. `sections` are all the
/// owner's sections in metadata order and `added` the index of the new one; its neighbours are the
/// nearest sections the registry already lists.
pub fn add_section(
    text: &[u8],
    kind: &str,
    owner: &str,
    sections: &[SectionMember],
    added: usize,
) -> Result<Vec<u8>> {
    let main = si::parse(text).context("the object registry does not parse")?;
    let class = tabular_class(kind).context("the kind has no tabular sections")?;
    let section = sections.get(added).context("no such section")?;
    let listed = |candidate: &&SectionMember| main.index_of(&candidate.uuid).is_some();
    let previous = sections[..added].iter().rev().find(listed);
    let next = sections[added + 1..].iter().find(listed);
    let at = main.place(
        owner,
        kind_of(&main, class)?,
        previous.map(|section| section.uuid.as_str()),
        next.map(|section| section.uuid.as_str()),
    )?;
    let records = section_records(&main, section, owner, class)?;
    si::insert_records(text, &main, &[Insertion { at, records }])
}

/// The registry text with new attributes of an existing object (`class` is the class of its
/// attribute collection: [`attribute_class`] or [`SECTION_ATTRIBUTES`]). `attributes` are all of
/// the owner's attributes in metadata order and `added` the uuids of the new ones.
pub fn add_attributes(
    text: &[u8],
    owner: &str,
    class: &str,
    attributes: &[Member],
    added: &[&str],
) -> Result<Vec<u8>> {
    let main = si::parse(text).context("the object registry does not parse")?;
    let kind = kind_of(&main, class)?;
    let mut insertions = Vec::new();
    let mut previous: Option<&Member> = None;
    let mut pending: Vec<&Member> = Vec::new();
    let mut flush = |previous: Option<&Member>,
                     next: Option<&Member>,
                     pending: &mut Vec<&Member>|
     -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        let at = main.place(
            owner,
            kind,
            previous.map(|member| member.uuid.as_str()),
            next.map(|member| member.uuid.as_str()),
        )?;
        let records = pending
            .drain(..)
            .map(|member| attribute_record(&main, member, owner, class))
            .collect::<Result<Vec<_>>>()?;
        insertions.push(Insertion { at, records });
        Ok(())
    };
    for attribute in attributes {
        if added.contains(&attribute.uuid.as_str()) {
            pending.push(attribute);
        } else {
            flush(previous, Some(attribute), &mut pending)?;
            previous = Some(attribute);
        }
    }
    flush(previous, None, &mut pending)?;
    ensure!(!insertions.is_empty(), "no attribute to add");
    si::insert_records(text, &main, &insertions)
}

/// The registry text with the synonyms of existing records written anew: an object whose synonym changed
/// takes the new text in its record (the native apply does this for a synonym-only stage; nothing else of the
/// record moves). `changes` are `(uuid, synonyms)` with the texts as the descriptor gives them (quotes not
/// doubled); a uuid the registry does not list is left out (not every object with a header has a record).
/// Returns the text and how many records were rewritten.
pub fn set_synonyms(
    text: &[u8],
    changes: &[(String, Vec<(String, String)>)],
) -> Result<(Vec<u8>, usize)> {
    let main = si::parse(text).context("the object registry does not parse")?;
    let indexed: Vec<(usize, Vec<(String, String)>)> = changes
        .iter()
        .filter_map(|(uuid, pairs)| main.index_of(uuid).map(|index| (index, synonyms(pairs))))
        .collect();
    si::set_synonyms(text, &main, &indexed)
}

/// The names the registry knows, by uuid (lower case).
pub fn names(text: &[u8]) -> Result<std::collections::BTreeMap<String, String>> {
    let main = si::parse(text).context("the object registry does not parse")?;
    Ok(main
        .records
        .into_iter()
        .map(|record| (record.uuid, record.name))
        .collect())
}

#[cfg(test)]
#[path = "../../../../../tests/support/restructure_registry.rs"]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::subtree_text;
