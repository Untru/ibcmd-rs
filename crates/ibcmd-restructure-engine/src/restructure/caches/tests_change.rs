//! A whole staged change against the native result: every one of the 16 `Params` cache rows, ours
//! (`change::rewrite`) against what the platform wrote, for the native cases of the `ddl` and `trace`
//! tracks:
//!
//! | case | before -> after | what the platform was asked |
//! |---|---|---|
//! | c | `pristine` -> `c2` | a new catalog, and two new attributes of other objects |
//! | d | `d_staged` -> `d_after` | a new document with an attribute and a tabular section |
//! | h | `pristine` -> `m` | a new tabular section, attributes of every type (references, a composite type, a value storage, a uuid) of a catalog, a hierarchical catalog and a tabular section, and edits of registers |
//! | t1 | `t1_before` -> `t1_nat` | 23 attributes of every primitive type of six objects |
//!
//! Every row is compared as text; where native's row holds records of objects this module does not
//! build (the registers of case h) they are left out of *native's* side and counted. `c4629235` is
//! compared by its entries (see `help_props`).

use std::collections::{BTreeMap, BTreeSet};

use crate::metadata_model::brace::parse_row;
use crate::mssql_config_apply::si;
use crate::restructure::caches::change::{ChangedObject, Staged, rewrite};
use crate::restructure::caches::help_props::HelpProps;
use crate::restructure::caches::names_tables::NamesTables;
use crate::restructure::caches::plan::CacheRow;
use crate::restructure::caches::root::{class_of_kind, collection_of, collections};
use crate::restructure::caches::tests_corpus::{CACHE_ROWS, ROOT_ROW, Snap, lab_snap, row_name};

/// The catalogs and documents whose descriptor row differs between two snapshots' `Config`.
fn changed_objects(before: &Snap, after: &Snap) -> Vec<ChangedObject> {
    let root = parse_row(&after.config(ROOT_ROW).unwrap()).unwrap();
    let all = collections(&root);
    let names = NamesTables::parse(&after.params(row_name("a07b62f0")).unwrap()).unwrap();
    let mut out = Vec::new();
    for kind in ["Catalog", "Document"] {
        let class = class_of_kind(kind).unwrap();
        for uuid in &collection_of(&all, class).unwrap().objects {
            let now = after.config(uuid).unwrap();
            if before.config(uuid).as_ref() == Some(&now) {
                continue;
            }
            let table_number = names
                .entries
                .iter()
                .find(|entry| &entry.object == uuid)
                .and_then(|entry| entry.table.as_ref())
                .map(|table| table.number);
            out.push(ChangedObject {
                kind,
                uuid: uuid.clone(),
                table_number,
                has_help: after.has_config(&format!("{uuid}.1")),
                has_predefined: after.has_config(&format!("{uuid}.1c")),
            });
        }
    }
    out
}

fn replay(before: &Snap, after: &Snap) -> BTreeMap<&'static str, CacheRow> {
    let root = parse_row(&after.config(ROOT_ROW).unwrap()).unwrap();
    let changed = changed_objects(before, after);
    let before_row = |uuid: &str| before.descriptor(uuid);
    let after_row = |uuid: &str| after.descriptor(uuid);
    let cache = |name: &str| before.params(name);
    rewrite(&Staged {
        root: &root,
        before: &before_row,
        after: &after_row,
        changed: &changed,
        cache: &cache,
    })
    .unwrap()
    .into_iter()
    .map(|row| (row.name, row))
    .collect()
}

// ---------------------------------------------------------------------------------------------
// the registry: records
// ---------------------------------------------------------------------------------------------

/// The text of every record of a registry, with the byte span it stands in.
fn registry_records(text: &[u8]) -> (Vec<String>, si::SiMain) {
    let main = si::parse(text).unwrap();
    let records = main
        .records
        .iter()
        .map(|record| String::from_utf8_lossy(&text[record.start..record.end]).into_owned())
        .collect();
    (records, main)
}

/// The uuids of the records native inserted (against `before`) below an object that is no catalog and
/// no document: not ours.
fn foreign_records(
    before: &[u8],
    native: &[u8],
    own_classes: &BTreeSet<String>,
) -> BTreeSet<String> {
    let (_, was) = registry_records(before);
    let (_, is) = registry_records(native);
    let old: BTreeSet<&str> = was.records.iter().map(|r| r.uuid.as_str()).collect();
    let parent: BTreeMap<&str, &str> = is
        .records
        .iter()
        .map(|r| (r.uuid.as_str(), r.parent.as_str()))
        .collect();
    let owner = is.records[0].uuid.as_str();
    let mut out = BTreeSet::new();
    for record in &is.records {
        if old.contains(record.uuid.as_str()) {
            continue;
        }
        // the child of the configuration this record hangs under
        let mut at = record.uuid.as_str();
        while let Some(up) = parent.get(at) {
            if *up == owner {
                break;
            }
            at = up;
        }
        let top = is.index_of(at).map(|i| &is.records[i]);
        let own = top.is_some_and(|top| own_classes.contains(&is.classes[top.kind]));
        if !own {
            out.insert(record.uuid.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// the XDTO model: top-level items
// ---------------------------------------------------------------------------------------------

/// The top-level items of the model as `(name, text)`: object types with their lines, value types.
fn xdto_items(xml: &str) -> Vec<(String, String)> {
    let mut items = Vec::new();
    let mut open: Option<(String, String)> = None;
    for line in xml.split_inclusive("\r\n") {
        if let Some((_, text)) = open.as_mut() {
            text.push_str(line);
            if line.starts_with("\t\t</objectType>") {
                items.push(open.take().unwrap());
            }
            continue;
        }
        if line.starts_with("\t\t<objectType name=\"") || line.starts_with("\t\t<valueType ") {
            let name = line
                .split("name=\"")
                .nth(1)
                .and_then(|rest| rest.split('"').next())
                .unwrap_or_default()
                .to_owned();
            if line.starts_with("\t\t<valueType ") {
                items.push((name, line.to_owned()));
            } else {
                open = Some((name, line.to_owned()));
            }
        }
    }
    items
}

const OWN_PREFIXES: [&str; 6] = [
    "CatalogRef.",
    "CatalogObject.",
    "CatalogTabularSectionRow.",
    "DocumentRef.",
    "DocumentObject.",
    "DocumentTabularSectionRow.",
];

fn is_own_item(name: &str) -> bool {
    OWN_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

// ---------------------------------------------------------------------------------------------
// the comparison
// ---------------------------------------------------------------------------------------------

struct Outcome {
    /// `(row, how it was compared)`
    rows: Vec<(String, String)>,
}

fn compare_case(case: &str, before: &Snap, after: &Snap) -> Outcome {
    let ours = replay(before, after);
    let own_classes: BTreeSet<String> = {
        let registry = after.params(row_name("1a621f0f")).unwrap();
        let main = si::parse(&registry).unwrap();
        ["Catalog", "Document"]
            .iter()
            .map(|kind| class_of_kind(kind).unwrap().to_owned())
            .filter(|class| main.kind_of_class(class).is_some())
            .collect()
    };
    let mut out = Outcome { rows: Vec::new() };
    for (short, _) in CACHE_ROWS {
        let name = row_name(short);
        let was = before.params(name).unwrap();
        let native = after.params(name).unwrap();
        let mine = ours.get(name).map(|row| row.text.clone());
        let how = match (*short, mine) {
            (_, None) => {
                assert!(
                    was == native,
                    "{case}: {short} changed natively and we wrote nothing"
                );
                "unchanged, and native left it".to_owned()
            }
            ("c4629235", Some(mine)) => compare_help(case, &mine, &native),
            ("1a621f0f", Some(mine)) => {
                if mine == native {
                    "equal".to_owned()
                } else {
                    let (ours_records, ours_main) = registry_records(&mine);
                    let foreign = foreign_records(&was, &native, &own_classes);
                    let (native_records, native_main) = registry_records(&native);
                    let native_records: Vec<String> = native_main
                        .records
                        .iter()
                        .zip(native_records)
                        .filter(|(record, _)| !foreign.contains(&record.uuid))
                        .map(|(_, text)| text)
                        .collect();
                    assert_eq!(ours_main.classes, native_main.classes, "{case}: classes");
                    assert!(
                        ours_records == native_records,
                        "{case}: the registry differs from native's beyond the foreign records"
                    );
                    format!(
                        "equal but {} records of other kinds of objects",
                        foreign.len()
                    )
                }
            }
            ("ea13a2c9", Some(mine)) => {
                if mine == native {
                    "equal".to_owned()
                } else {
                    let ours_model =
                        crate::restructure::caches::xdto_types::XdtoModel::parse(&mine).unwrap();
                    let native_model =
                        crate::restructure::caches::xdto_types::XdtoModel::parse(&native).unwrap();
                    let (ours_items, native_items) =
                        (xdto_items(&ours_model.xml), xdto_items(&native_model.xml));
                    let pick = |items: &[(String, String)]| -> Vec<(String, String)> {
                        items
                            .iter()
                            .filter(|(n, _)| is_own_item(n))
                            .cloned()
                            .collect()
                    };
                    assert!(
                        pick(&ours_items) == pick(&native_items),
                        "{case}: the XDTO types of catalogs and documents differ from native's"
                    );
                    // what is not ours is as it was
                    let was_model =
                        crate::restructure::caches::xdto_types::XdtoModel::parse(&was).unwrap();
                    let others = |items: &[(String, String)]| -> Vec<(String, String)> {
                        items
                            .iter()
                            .filter(|(n, _)| !is_own_item(n))
                            .cloned()
                            .collect()
                    };
                    let old_others = others(&xdto_items(&was_model.xml));
                    assert!(
                        others(&ours_items) == old_others,
                        "{case}: we touched an XDTO type of another kind of object"
                    );
                    let old_set: BTreeSet<&(String, String)> = old_others.iter().collect();
                    let native_others = others(&native_items);
                    format!(
                        "equal in the {} types of catalogs and documents; native also changed {} others",
                        pick(&native_items).len(),
                        native_others
                            .iter()
                            .filter(|item| !old_set.contains(item))
                            .count()
                    )
                }
            }
            (short, Some(mine)) => {
                assert!(mine == native, "{case}: {short} differs from native's");
                "equal".to_owned()
            }
        };
        out.rows.push(((*short).to_owned(), how));
    }
    out
}

fn compare_help(case: &str, mine: &[u8], native: &[u8]) -> String {
    let ours = HelpProps::parse(mine).unwrap();
    let theirs = HelpProps::parse(native).unwrap();
    assert_eq!(ours.entries.len(), theirs.entries.len(), "{case}: c4629235");
    let by_key = |help: &HelpProps| -> BTreeMap<String, _> {
        help.entries
            .iter()
            .map(|entry| (entry.key.clone(), entry.clone()))
            .collect()
    };
    assert!(
        by_key(&ours) == by_key(&theirs),
        "{case}: c4629235 entries differ"
    );
    if ours == theirs {
        return "equal".to_owned();
    }
    // neighbours of native's row that are not neighbours in ours
    let keys = |help: &HelpProps| -> Vec<String> {
        help.entries.iter().map(|entry| entry.key.clone()).collect()
    };
    let (a, b) = (keys(&ours), keys(&theirs));
    let links: BTreeSet<(&str, &str)> = a
        .windows(2)
        .map(|pair| (pair[0].as_str(), pair[1].as_str()))
        .collect();
    let broken = b
        .windows(2)
        .filter(|pair| !links.contains(&(pair[0].as_str(), pair[1].as_str())))
        .count();
    assert!(broken < 30, "{case}: {broken} neighbour links broken");
    format!(
        "the same {} entries, {broken} of {} neighbour links differ (order only)",
        b.len(),
        b.len() - 1
    )
}

fn report(case: &str, outcome: &Outcome) {
    eprintln!("== {case}");
    for (short, how) in &outcome.rows {
        eprintln!("  {short}: {how}");
    }
}

#[test]
fn case_c_a_new_catalog_every_row_is_native() {
    let before = lab_snap!("pristine");
    let after = lab_snap!("c2");
    let outcome = compare_case("c", &before, &after);
    report("c", &outcome);
}

#[test]
fn case_d_a_new_document_every_row_is_native() {
    let before = lab_snap!("d_staged");
    let after = lab_snap!("d_after");
    let outcome = compare_case("d", &before, &after);
    report("d", &outcome);
}

#[test]
fn case_h_a_new_tabular_section_and_attributes_of_every_type_every_row_is_native() {
    let before = lab_snap!("pristine");
    let after = lab_snap!("m");
    let outcome = compare_case("h", &before, &after);
    report("h", &outcome);
}

#[test]
fn the_types_case_every_row_is_native() {
    let before = lab_snap!("t1_before");
    let after = lab_snap!("t1_nat");
    let outcome = compare_case("t1", &before, &after);
    report("t1", &outcome);
}

#[test]
fn case_e1_five_new_tabular_sections_every_row_is_native() {
    let before = lab_snap!("e1_before");
    let after = lab_snap!("e1_nat");
    let outcome = compare_case("e1", &before, &after);
    report("e1", &outcome);
}

#[test]
fn case_e3_new_attributes_of_old_tabular_sections_every_row_is_native() {
    let before = lab_snap!("e3_before");
    let after = lab_snap!("e3_nat");
    let outcome = compare_case("e3", &before, &after);
    report("e3", &outcome);
}

#[test]
fn case_e4_own_attributes_section_attributes_and_new_sections_together_every_row_is_native() {
    let before = lab_snap!("e4_before");
    let after = lab_snap!("e4_nat");
    let outcome = compare_case("e4", &before, &after);
    report("e4", &outcome);
}

#[test]
fn case_e5_new_tabular_sections_on_objects_no_extension_adopts_every_row_is_native() {
    let before = lab_snap!("e5_before");
    let after = lab_snap!("e5_nat");
    let outcome = compare_case("e5", &before, &after);
    report("e5", &outcome);
}

#[test]
fn case_e6_section_attributes_on_objects_no_extension_adopts_every_row_is_native() {
    let before = lab_snap!("e6_before");
    let after = lab_snap!("e6_nat");
    let outcome = compare_case("e6", &before, &after);
    report("e6", &outcome);
}

/// Not a check: with `IBCMD_RS_CACHES_DUMP=<dir>` (and `IBCMD_RS_CACHES_CASE=c|d|h|t1`, default `c`) it
/// writes the rows our code makes for a case, inflated (`<row>.txt`) and as stored (`<row>.deflated`),
/// for the platform proofs of `derived-caches.md` (rows swapped into a twin of the native state).
#[test]
fn dump_the_rows_of_a_case_for_the_twin_proofs() {
    let Some(dir) = std::env::var_os("IBCMD_RS_CACHES_DUMP") else {
        return;
    };
    let case = std::env::var("IBCMD_RS_CACHES_CASE").unwrap_or_else(|_| "c".to_owned());
    let (before, after) = match case.as_str() {
        "c" => (lab_snap!("pristine"), lab_snap!("c2")),
        "d" => (lab_snap!("d_staged"), lab_snap!("d_after")),
        "h" => (lab_snap!("pristine"), lab_snap!("m")),
        "t1" => (lab_snap!("t1_before"), lab_snap!("t1_nat")),
        other => panic!("unknown case {other}"),
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, row) in replay(&before, &after) {
        std::fs::write(dir.join(format!("{name}.txt")), &row.text).unwrap();
        std::fs::write(
            dir.join(format!("{name}.deflated")),
            crate::restructure::names::deflate(&row.text).unwrap(),
        )
        .unwrap();
    }
}

// ---------------------------------------------------------------------------------------------
// what is refused
// ---------------------------------------------------------------------------------------------

/// A staged change made of catalogs the snapshot already has, presented as new: only the refusals are
/// looked at.
fn refusal_of(pick: impl Fn(&Snap, &[String]) -> Vec<ChangedObject>) -> String {
    let Some(snap) = Snap::open("pristine") else {
        eprintln!("skipped: no lab snapshot pristine");
        return "skipped".to_owned();
    };
    let root = parse_row(&snap.config(ROOT_ROW).unwrap()).unwrap();
    let all = collections(&root);
    let catalogs = collection_of(&all, class_of_kind("Catalog").unwrap())
        .unwrap()
        .objects
        .clone();
    let changed = pick(&snap, &catalogs);
    let nothing = |_: &str| None;
    let after_row = |uuid: &str| snap.descriptor(uuid);
    let cache = |name: &str| snap.params(name);
    let error = rewrite(&Staged {
        root: &root,
        before: &nothing,
        after: &after_row,
        changed: &changed,
        cache: &cache,
    })
    .unwrap_err();
    format!("{error:#}")
}

fn changed(kind: &'static str, uuid: &str, has_predefined: bool) -> ChangedObject {
    ChangedObject {
        kind,
        uuid: uuid.to_owned(),
        table_number: Some(11036),
        has_help: false,
        has_predefined,
    }
}

#[test]
fn a_new_catalog_that_lists_forms_or_commands_is_refused() {
    use crate::restructure::caches::members::Members;
    let message = refusal_of(|snap, catalogs| {
        let uuid = catalogs
            .iter()
            .find(|uuid| {
                let row = snap.descriptor(uuid).unwrap();
                !Members::parse("Catalog", &row).unwrap().others.is_empty()
            })
            .unwrap();
        vec![changed("Catalog", uuid, false)]
    });
    assert!(
        message == "skipped" || message.contains("collections beyond its attributes"),
        "{message}"
    );
}

#[test]
fn a_new_catalog_with_predefined_items_is_refused() {
    use crate::restructure::caches::members::Members;
    let message = refusal_of(|snap, catalogs| {
        let uuid = catalogs
            .iter()
            .find(|uuid| {
                let row = snap.descriptor(uuid).unwrap();
                Members::parse("Catalog", &row).unwrap().others.is_empty()
            })
            .unwrap();
        vec![changed("Catalog", uuid, true)]
    });
    assert!(
        message == "skipped" || message.contains("predefined items"),
        "{message}"
    );
}

#[test]
fn two_new_catalogs_in_one_stage_are_refused() {
    let message = refusal_of(|_, catalogs| {
        vec![
            changed("Catalog", &catalogs[0], false),
            changed("Catalog", &catalogs[1], false),
        ]
    });
    assert!(
        message == "skipped" || message.contains("more than one new Catalog"),
        "{message}"
    );
}
