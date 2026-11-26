//! Shared fixture helpers for the planner and its root apply gate tests.

use crate::restructure::names::{DbNames, deflate, inflate};
use crate::restructure::plan::{Inputs, PlanOptions, StagedImage};
use ibcmd_restructure::brace::parse_row;
use std::collections::BTreeSet;

pub(super) const STAGED_SCHEMA: &[u8] = include_bytes!(
    "../../tests/fixtures/native-evidence/restructure/dbschema-a2-staged-excerpt.txt"
);

pub(super) const NAMES: &[u8] =
    include_bytes!("../../tests/fixtures/native-evidence/restructure/dbnames-a2-excerpt.txt");

pub(super) const OLD_ROW: &[u8] = include_bytes!(
    "../../tests/fixtures/native-evidence/restructure/catalog-reference20-old.deflate"
);

pub(super) const NEW_ROW: &[u8] = include_bytes!(
    "../../tests/fixtures/native-evidence/restructure/catalog-reference20-new.deflate"
);

pub(super) const CATALOG: &str = "5eab8a1b-070f-4dcf-bdcc-a259c62c3693";

pub(super) const VERSION: &str = "d516886c-0000-4000-8000-00000000abcd";

pub(super) fn names_row(max: u64) -> Vec<u8> {
    DbNames {
        max,
        entries: Vec::new(),
    }
    .to_row()
    .unwrap()
}

pub(super) fn inputs(old: &[u8], new: &[u8]) -> Inputs {
    let files: BTreeSet<String> = [CATALOG.to_owned()].into();
    let mut new_files = files.clone();
    new_files.insert("deleted".to_owned());
    Inputs {
        schema: STAGED_SCHEMA.to_vec(),
        main_names: DbNames::parse(NAMES).unwrap().to_row().unwrap(),
        // The header numbers of the extensions of the lab database.
        extension_names: vec![
            ("DBNames-Ext-1".to_owned(), names_row(10989)),
            ("DBNames-Ext-347eea02".to_owned(), names_row(7841)),
            ("DBNames-Ext-b3fa0ef0".to_owned(), names_row(11033)),
        ],
        predefined_tables: BTreeSet::new(),
        cache_rows: Vec::new(),
        cache_sizes: Default::default(),
        root_row: Vec::new(),
        staged: StagedImage {
            old_files: files,
            new_files,
            old_descriptors: [(CATALOG.to_owned(), old.to_vec())].into(),
            new_descriptors: [(CATALOG.to_owned(), new.to_vec())].into(),
            deleted: Some(deflate("\u{feff}0".as_bytes()).unwrap()),
        },
        objects: Default::default(),
        extensions: Default::default(),
    }
}

pub(super) fn options() -> PlanOptions {
    PlanOptions {
        names_version: Some(VERSION.to_owned()),
        // the fixtures hold no cache rows (the corpus test has the real ones)
        skip_xdto: true,
        skip_registry: true,
        ..PlanOptions::default()
    }
}

/// The row with the type pattern `from` that follows the first `marker` replaced by `to`.
pub(super) fn retype_after(row: &[u8], marker: &str, from: &str, to: &str) -> Vec<u8> {
    let text = String::from_utf8(inflate(row).unwrap()).unwrap();
    let needle = format!("{{\"Pattern\",\r\n{from}");
    let start = text.find(marker).unwrap();
    let at = start + text[start..].find(&needle).unwrap();
    let mut changed = text.clone();
    changed.replace_range(at..at + needle.len(), &format!("{{\"Pattern\",\r\n{to}"));
    assert!(parse_row(changed.as_bytes()).is_ok());
    deflate(changed.as_bytes()).unwrap()
}

/// The attribute of case a2 named "Клиент" (a boolean, `Fld151`, in the main table) as a variable string of
/// `limit` characters: the fixtures hold no limited string among the own attributes of the catalog, and
/// the stored schema has to agree, so it changes with the row.
pub(super) fn client_as_string(row: &[u8], limit: u64) -> Vec<u8> {
    retype_after(
        row,
        "\"Клиент\"",
        "{\"B\"}",
        &format!("{{\"S\",{limit},1}}"),
    )
}

pub(super) fn widen_client(row: &[u8], from: &str, to: &str) -> Vec<u8> {
    retype_after(row, "\"Клиент\"", from, to)
}

/// The inputs of case a2 with "Клиент" a string of 50 characters in the stored state; the staged row is `new`.
pub(super) fn string_client_inputs(new: &[u8]) -> Inputs {
    let mut base = inputs(&client_as_string(OLD_ROW, 50), new);
    let schema = String::from_utf8(STAGED_SCHEMA.to_vec()).unwrap();
    let at = schema.find("\"Fld151\"").unwrap();
    let boolean = at + schema[at..].find("{\"L\",0,0,\"\",0}").unwrap();
    let mut text = schema.clone();
    text.replace_range(
        boolean..boolean + "{\"L\",0,0,\"\",0}".len(),
        &format!("{{\"S\",{},0,\"\",0}}", 0x8000_0000u64 | 50),
    );
    base.schema = text.into_bytes();
    base
}

/// The row with `Indexing` of the attribute "Клиент" (a boolean of case a2, `Fld151`) changed from `from` to
/// `to` (0 `DontIndex`, 1 `Index`, 2 `IndexWithAdditionalOrder`).
pub(super) fn client_indexing(row: &[u8], from: u8, to: u8) -> Vec<u8> {
    let text = String::from_utf8(inflate(row).unwrap()).unwrap();
    let start = text.find("\"Клиент\"").unwrap();
    let old = format!("{{\"B\",1}},1,0,0}},{from},0,1,1}}");
    let at = start + text[start..].find(&old).unwrap();
    let mut changed = text.clone();
    changed.replace_range(
        at..at + old.len(),
        &format!("{{\"B\",1}},1,0,0}},{to},0,1,1}}"),
    );
    assert!(parse_row(changed.as_bytes()).is_ok());
    deflate(changed.as_bytes()).unwrap()
}
