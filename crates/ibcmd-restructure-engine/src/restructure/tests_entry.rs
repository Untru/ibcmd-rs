//! The entry builder against the stored entries of the БСП: every catalog and document of the lab
//! snapshots is rebuilt from its descriptor, `DBNames` and the common attributes, and the text must equal
//! the stored `DBSchema` entry. Objects the builder refuses are counted with the reason; a wrong entry is a
//! failure. Skipped without the lab.

use std::collections::BTreeMap;

use crate::metadata_model::brace::{parse_row, serialize};
use crate::restructure::caches::facts::ObjectFacts;
use crate::restructure::caches::members::Members;
use crate::restructure::caches::names_tables::NamesTables;
use crate::restructure::caches::root::{class_of_kind, collection_of, collections};
use crate::restructure::caches::tests_corpus::{ROOT_ROW, Snap, lab_snap, row_name};
use crate::restructure::common::{COMMON_ATTRIBUTE_CLASS, CommonAttributes};
use crate::restructure::entry::{EntryInput, Kind, RefTables, main_entry};

const DEFINED_TYPE_CLASS: &str = "c045099e-13b9-4fb6-9d50-fca00202971e";
use crate::restructure::names::DbNames;
use crate::restructure::schema::DbSchema;

#[derive(Default, Debug)]
struct Report {
    checked: usize,
    equal: usize,
    equal_names: Vec<String>,
    refused: BTreeMap<String, usize>,
    different: Vec<String>,
}

fn first_difference(built: &str, stored: &str) -> String {
    let at = built
        .bytes()
        .zip(stored.bytes())
        .position(|(a, b)| a != b)
        .unwrap_or(built.len().min(stored.len()));
    let boundary = |text: &str, mut index: usize| {
        index = index.min(text.len());
        while !text.is_char_boundary(index) {
            index -= 1;
        }
        index
    };
    let from = boundary(built, at.saturating_sub(100));
    let from_stored = boundary(stored, from);
    format!(
        "at {at}\n    built:  {}\n    stored: {}",
        built[from..boundary(built, at + 140)].replace("\r\n", " "),
        stored[from_stored..boundary(stored, at + 140)].replace("\r\n", " ")
    )
}

fn check(snap: &Snap) -> Report {
    let schema = DbSchema::parse(&snap.schema().unwrap()).unwrap();
    let names = DbNames::parse(&snap.params("DBNames").unwrap()).unwrap();
    let root = parse_row(&snap.config(ROOT_ROW).unwrap()).unwrap();
    let all = collections(&root);
    let order = collection_of(&all, COMMON_ATTRIBUTE_CLASS)
        .unwrap()
        .objects
        .clone();
    let common = CommonAttributes::from_rows(&order, &|uuid: &str| snap.descriptor(uuid)).unwrap();
    let mut refs =
        RefTables::build(&NamesTables::parse(&snap.params(row_name("a07b62f0")).unwrap()).unwrap());
    for uuid in &collection_of(&all, DEFINED_TYPE_CLASS).unwrap().objects {
        refs.add_defined(&snap.descriptor(uuid).unwrap()).unwrap();
    }
    let mut report = Report::default();
    for kind_name in ["Catalog", "Document"] {
        let kind = Kind::parse(kind_name).unwrap();
        let class = class_of_kind(kind_name).unwrap();
        for uuid in &collection_of(&all, class).unwrap().objects {
            let row = parse_row(&snap.config(uuid).unwrap()).unwrap();
            let facts = ObjectFacts::parse(kind_name, &row).unwrap();
            let members = Members::parse(kind_name, &row).unwrap();
            report.checked += 1;
            let built = main_entry(&EntryInput {
                kind,
                facts: &facts,
                members: &members,
                names: &names,
                common: &common,
                refs: &refs,
            });
            let built = match built {
                Ok(built) => built,
                Err(error) => {
                    // the first cause only, without the object's name
                    let reason = format!("{error:#}");
                    let reason = reason.replace(&facts.name, "<name>");
                    *report
                        .refused
                        .entry(format!("{kind_name}: {reason}"))
                        .or_default() += 1;
                    continue;
                }
            };
            let table = names.number_of(uuid, kind.table_kind()).unwrap();
            let name = format!("{}{table}", kind.table_kind());
            let position = schema.position(&name).unwrap();
            let stored = serialize(&schema.tables()[position]);
            let built = serialize(&built);
            if built == stored {
                report.equal += 1;
                report.equal_names.push(facts.name.clone());
            } else {
                report.different.push(format!(
                    "{kind_name} {} ({name}): {}",
                    facts.name,
                    first_difference(&built, &stored)
                ));
            }
        }
    }
    report
}

#[test]
fn the_entry_of_every_catalog_and_document_is_rebuilt_from_its_descriptor() {
    for name in ["pristine", "c2", "d_after"] {
        let snap = lab_snap!(name);
        let report = check(&snap);
        eprintln!(
            "{name}: {} objects, {} entries equal, {} refused, {} different",
            report.checked,
            report.equal,
            report.refused.values().sum::<usize>(),
            report.different.len()
        );
        for (reason, count) in &report.refused {
            eprintln!("  refused {count}: {reason}");
        }
        for difference in report.different.iter().take(12) {
            eprintln!("  DIFFERENT {difference}");
        }
        assert!(report.different.is_empty(), "{name}");
        // the objects of the native cases are among the covered ones
        let native_case = match name {
            "c2" => Some("ДемоНовыйСправочник"),
            "d_after" => Some("ДемоНовыйДокумент"),
            _ => None,
        };
        if let Some(object) = native_case {
            assert!(
                report.equal_names.iter().any(|n| n == object),
                "{object} is not rebuilt"
            );
        }
        assert!(
            report.equal >= 100,
            "{name}: only {} entries rebuilt",
            report.equal
        );
        assert_eq!(
            report.equal + report.refused.values().sum::<usize>(),
            report.checked
        );
    }
}

/// The names of the builder's own refusals must not hide an object: the corpus is fully covered or
/// refused by a reason. (Filled in when the builder is complete.)
#[allow(dead_code)]
fn row_of(snap: &Snap, short: &str) -> Vec<u8> {
    snap.params(row_name(short)).unwrap()
}

// ---------------------------------------------------------------------------------------------
// synthetic: what the builder refuses (no lab)
// ---------------------------------------------------------------------------------------------

mod synthetic {
    use super::*;
    use crate::metadata_model::brace::Brace;
    use crate::restructure::caches::members::{Member, SectionMember};
    use crate::restructure::entry::subtable;

    const SECTION: &str = "11111111-1111-1111-1111-111111111111";
    const ATTRIBUTE: &str = "22222222-2222-2222-2222-222222222222";
    const DOCUMENT_REF: &str = "33333333-3333-3333-3333-333333333333";
    const CATALOG_REF: &str = "44444444-4444-4444-4444-444444444444";

    fn names() -> DbNames {
        DbNames::parse(
            format!(
                "{{12,{{3,{{{SECTION},\"VT\",10}},{{{SECTION},\"LineNo\",11}},{{{ATTRIBUTE},\"Fld\",12}}}}}}"
            )
            .as_bytes(),
        )
        .unwrap()
    }

    fn refs() -> RefTables {
        RefTables::default()
            .with_table(
                "a1111111-0000-0000-0000-000000000001",
                DOCUMENT_REF,
                "Document41",
            )
            .with_table(
                "a1111111-0000-0000-0000-000000000002",
                CATALOG_REF,
                "Reference7",
            )
    }

    fn section(type_id: &str) -> SectionMember {
        SectionMember {
            uuid: SECTION.to_owned(),
            name: "Товары".to_owned(),
            synonyms: Vec::new(),
            attributes: vec![Member {
                uuid: ATTRIBUTE.to_owned(),
                name: "Основание".to_owned(),
                synonyms: Vec::new(),
                pattern: Brace::List(vec![
                    Brace::str("Pattern"),
                    Brace::List(vec![Brace::str("#"), Brace::atom(type_id)]),
                ]),
                usage: None,
                indexing: Some(0),
            }],
        }
    }

    #[test]
    fn a_document_reference_in_the_section_of_a_document_is_refused() {
        let error = subtable(
            &names(),
            &refs(),
            Kind::Document,
            "Document5",
            &section(DOCUMENT_REF),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("refers to a document"),
            "{error:#}"
        );
    }

    #[test]
    fn the_same_reference_in_a_catalog_and_a_catalog_reference_in_a_document_are_built() {
        let built = subtable(
            &names(),
            &refs(),
            Kind::Catalog,
            "Reference5",
            &section(DOCUMENT_REF),
        )
        .unwrap();
        assert!(serialize(&built).contains("\"Document41\""));
        let built = subtable(
            &names(),
            &refs(),
            Kind::Document,
            "Document5",
            &section(CATALOG_REF),
        )
        .unwrap();
        let text = serialize(&built);
        assert!(
            text.contains("\"VT10\"")
                && text.contains("\"LineNo11\"")
                && text.contains("\"Reference7\"")
        );
    }
}
