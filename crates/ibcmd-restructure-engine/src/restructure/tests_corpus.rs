//! Conformance of the model against the lab corpora. The big samples live in the lab (snapshots of
//! `scripts/restructure-lab/snapshot.py`), so these tests skip themselves when the lab folder is
//! absent: set `IBCMD_RS_DDL_LAB` to another location, default `F:\ibcmd\lab\04\restructure`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::Value;

use crate::restructure::names::{DbNames, inflate};
use crate::restructure::plan::{Inputs, PlanOptions, StagedImage, plan};
use crate::restructure::schema::{
    DbSchema, IndexDef, TableView, declared_indexes, implicit_indexes, table_columns,
};

fn lab() -> Option<PathBuf> {
    let root = std::env::var_os("IBCMD_RS_DDL_LAB")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"F:\ibcmd\lab\04\restructure"));
    root.join("snap").is_dir().then_some(root)
}

/// One snapshot of the lab: `snap/<db>/<label>/{svc.json,schema.txt}` and the blob store.
struct Snapshot {
    root: PathBuf,
    dir: PathBuf,
    svc: Value,
}

impl Snapshot {
    fn open(db: &str, label: &str) -> Option<Self> {
        let root = lab()?;
        let dir = root.join("snap").join(db).join(label);
        let text = std::fs::read_to_string(dir.join("svc.json")).ok()?;
        Some(Self {
            root,
            dir,
            svc: serde_json::from_str(&text).ok()?,
        })
    }

    fn blob(&self, sha: &str) -> Vec<u8> {
        std::fs::read(self.root.join("blobs").join(sha)).unwrap()
    }

    fn schema(&self) -> Vec<u8> {
        self.blob(self.svc["DBSchema"][0].as_str().unwrap())
    }

    /// The rows of a service table: (name, part) -> sha.
    fn rows(&self, table: &str) -> BTreeMap<(String, i64), String> {
        self.svc[table]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                (
                    (
                        row["name"].as_str().unwrap().to_owned(),
                        row["part"].as_i64().unwrap(),
                    ),
                    row["sha"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }

    fn row(&self, table: &str, name: &str) -> Option<Vec<u8>> {
        self.rows(table)
            .get(&(name.to_owned(), 0))
            .map(|sha| self.blob(sha))
    }

    fn schema_txt(&self) -> String {
        std::fs::read_to_string(self.dir.join("schema.txt")).unwrap()
    }
}

#[test]
fn corpus_schema_and_names_round_trip_byte_for_byte() {
    for (db, label) in [
        ("ibcmd_rs_04_ddl_bsp8327_a", "a2_after"),
        ("ibcmd_rs_04_ddl_bsp85_a", "s85_restored"),
    ] {
        let Some(snapshot) = Snapshot::open(db, label) else {
            eprintln!("skipped: no lab snapshot {db}/{label}");
            return;
        };
        let text = snapshot.schema();
        let schema = DbSchema::parse(&text).unwrap();
        assert!(schema.len() > 1700, "{db}: {} tables", schema.len());
        assert_eq!(schema.to_text(), text, "{db} DBSchema");

        let names_row = snapshot.row("Params", "DBNames").unwrap();
        let names_text = inflate(&names_row).unwrap();
        let names = DbNames::parse(&names_text).unwrap();
        assert!(names.entries.len() > 5000);
        assert_eq!(names.to_text(), names_text, "{db} DBNames");
        assert_eq!(DbNames::parse_row(&names.to_row().unwrap()).unwrap(), names);
    }
}

/// `dbschema_check.py` in Rust: every table (and sub-table) of the schema against the columns and
/// indexes the snapshot's database really has.
struct SqlTable {
    columns: Vec<(String, String, bool)>,
    indexes: Vec<(String, bool, bool, Vec<String>)>,
}

fn parse_schema_txt(text: &str) -> BTreeMap<String, SqlTable> {
    let mut tables = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("T ") {
            current = Some(name.to_owned());
            tables.insert(
                name.to_owned(),
                SqlTable {
                    columns: Vec::new(),
                    indexes: Vec::new(),
                },
            );
        } else if let Some(rest) = line.strip_prefix("  C ") {
            let table = tables.get_mut(current.as_ref().unwrap()).unwrap();
            let mut parts = rest.split(' ');
            let name = parts.next().unwrap().to_owned();
            let sql_type = parts.next().unwrap().to_owned();
            let nullable = parts.next().unwrap() == "NULL";
            table.columns.push((name, sql_type, nullable));
        } else if let Some(rest) = line.strip_prefix("  I ") {
            let table = tables.get_mut(current.as_ref().unwrap()).unwrap();
            let (name, rest) = rest.split_once(' ').unwrap();
            let (kind, rest) = rest.split_once(' ').unwrap();
            let (flags, rest) = rest.split_once("] (").unwrap();
            let flags = flags.trim_start_matches('[');
            let columns = rest.split(')').next().unwrap();
            table.indexes.push((
                name.to_owned(),
                flags.contains("UNIQUE") || flags.contains("PK"),
                kind == "CLUSTERED",
                columns.split(',').map(str::to_owned).collect(),
            ));
        }
    }
    tables
}

fn check_model_against_sql(db: &str, label: &str) -> Option<(usize, usize)> {
    let snapshot = Snapshot::open(db, label)?;
    let sql = parse_schema_txt(&snapshot.schema_txt());
    let schema = DbSchema::parse(&snapshot.schema()).unwrap();
    let mut tables = 0;
    let mut inline_keys = 0;
    let mut problems = Vec::new();
    for position in 0..schema.len() {
        let object = schema.view(position).unwrap();
        let mut jobs: Vec<(String, TableView<'_>, Option<TableView<'_>>)> =
            vec![(format!("_{}", object.name()), object, None)];
        for sub in object.subtables().unwrap() {
            jobs.push((
                format!("_{}_{}", object.name(), sub.name()),
                sub,
                Some(object),
            ));
        }
        for (sql_name, table, owner) in jobs {
            tables += 1;
            let Some(real) = sql.get(&sql_name) else {
                problems.push(format!("{sql_name}: missing in SQL"));
                continue;
            };
            let expected = table_columns(&table, owner.as_ref()).unwrap();
            let columns: Vec<_> = expected
                .iter()
                .map(|column| {
                    (
                        column.name.clone(),
                        column.sql_type.ddl().replace(", ", ","),
                        column.nullable,
                    )
                })
                .collect();
            if columns != real.columns {
                problems.push(format!("{sql_name}: columns differ"));
            }
            let mut expected_indexes: Vec<IndexDef> =
                declared_indexes(&table, owner.as_ref()).unwrap();
            let implicit = implicit_indexes(&table, owner.as_ref()).unwrap();
            expected_indexes.extend(implicit.iter().cloned());
            for index in &expected_indexes {
                let found = real
                    .indexes
                    .iter()
                    .any(|(name, unique, clustered, columns)| {
                        (index.name.is_empty() && name.starts_with("PK__") || *name == index.name)
                            && *unique == index.unique
                            && *clustered == index.clustered
                            && *columns == index.columns
                    });
                if !found {
                    problems.push(format!(
                        "{sql_name}: index {} is missing or different",
                        index.name
                    ));
                }
                if index.name.is_empty() {
                    inline_keys += 1;
                }
            }
            // Every real index is explained by the model.
            for (name, unique, clustered, columns) in &real.indexes {
                let explained = expected_indexes.iter().any(|index| {
                    (index.name.is_empty() && name.starts_with("PK__") || *name == index.name)
                        && index.unique == *unique
                        && index.clustered == *clustered
                        && index.columns == *columns
                });
                if !explained {
                    problems.push(format!("{sql_name}: real index {name} is not in the model"));
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{db}: {} of {tables} tables differ, first: {:?}",
        problems.len(),
        &problems[..problems.len().min(6)]
    );
    Some((tables, inline_keys))
}

#[test]
fn corpus_model_reproduces_the_tables_and_indexes_of_the_database() {
    for (db, label) in [
        ("ibcmd_rs_04_ddl_bsp8327_a", "a2_after"),
        ("ibcmd_rs_04_ddl_bsp85_a", "s85_restored"),
    ] {
        match check_model_against_sql(db, label) {
            Some((tables, inline_keys)) => {
                eprintln!("{db}/{label}: {tables} tables match ({inline_keys} inline keys)");
                assert!(tables > 1700);
            }
            None => eprintln!("skipped: no lab snapshot {db}/{label}"),
        }
    }
}

/// The plan's input as it stands in a snapshot of a database that holds a staged image.
fn inputs_of(staged: &Snapshot) -> Inputs {
    let params = staged.rows("Params");
    let mut inputs = Inputs {
        schema: staged.schema(),
        main_names: staged.blob(&params[&("DBNames".to_owned(), 0)]),
        root_row: staged.row("Config", "root").unwrap_or_default(),
        ..Inputs::default()
    };
    for ((name, _), sha) in &params {
        if name.starts_with("DBNames-Ext-") {
            inputs
                .extension_names
                .push((name.clone(), staged.blob(sha)));
        }
        if name.ends_with(".si") || name == "siVersions" {
            inputs.cache_rows.push((name.clone(), staged.blob(sha)));
        }
    }
    let config = staged.rows("Config");
    let save = staged.rows("ConfigSave");
    let mut image = StagedImage {
        old_files: config.keys().map(|(name, _)| name.clone()).collect(),
        new_files: save.keys().map(|(name, _)| name.clone()).collect(),
        deleted: staged.row("ConfigSave", "deleted"),
        ..StagedImage::default()
    };
    let bare = |name: &str| name.len() == 36 && !name.contains('.');
    for ((name, part), sha) in &config {
        if *part == 0 && bare(name) {
            image.old_descriptors.insert(name.clone(), staged.blob(sha));
        }
    }
    for ((name, part), sha) in &save {
        if *part == 0 && bare(name) {
            image.new_descriptors.insert(name.clone(), staged.blob(sha));
        }
    }
    assert!(
        image.new_descriptors.len() > 4000 || image.new_descriptors.len() < 100,
        "{}",
        image.new_descriptors.len()
    );
    inputs.staged = image;
    inputs
}

/// The plan against what the native apply produced: `DBNames` text, `DBSchema` entries (but the two
/// system tables the platform upgraded on its own), and the derived caches -- the XDTO model and the
/// object registry -- as text (only the deflate stream differs).
fn assert_equals_native(plan: &crate::restructure::plan::Plan, after: &Snapshot) {
    // The derived caches (a plan that writes none -- a stage that only widens strings -- is checked by the
    // test itself against the rows before): every row the plan rewrites is the platform's text, and the XDTO
    // model and the object registry are among them whenever the plan rewrites anything.
    let cache = |name: &str| {
        plan.caches
            .iter()
            .find(|cache| cache.row_name == name)
            .unwrap_or_else(|| panic!("the plan does not update {name}"))
    };
    for name in [
        "ea13a2c9-0c2f-40fa-b855-710387e3271d.si",
        "1a621f0f-5568-4183-bd9f-f6ef670e7090.si",
    ]
    .into_iter()
    .filter(|_| !plan.caches.is_empty())
    {
        cache(name);
    }
    for update in plan
        .caches
        .iter()
        .filter(|cache| cache.row_name != "siVersions")
    {
        let name = update.row_name.as_str();
        let native_row = inflate(&after.row("Params", name).unwrap()).unwrap();
        let ours_row = inflate(&update.row).unwrap();
        if update.what.contains("approximate") {
            // `c4629235` of a created object: the entries are the platform's, their order is approximate (derived-caches.md 4;
            // the twin protocol of S1-F counts it as the one row of 16 that differs). The length is exact.
            assert_eq!(ours_row.len(), native_row.len(), "{name}");
            continue;
        }
        if ours_row != native_row {
            let at = ours_row
                .iter()
                .zip(&native_row)
                .position(|(a, b)| a != b)
                .unwrap_or(ours_row.len().min(native_row.len()));
            let show = |row: &[u8]| {
                String::from_utf8_lossy(&row[at.saturating_sub(60)..(at + 120).min(row.len())])
                    .into_owned()
            };
            panic!(
                "{name} differs from the platform's at byte {at} ({} vs {} bytes):
ours:   {:?}
native: {:?}",
                ours_row.len(),
                native_row.len(),
                show(&ours_row),
                show(&native_row)
            );
        }
    }

    // DBNames: exactly what the platform stored.
    let native_names = inflate(&after.row("Params", "DBNames").unwrap()).unwrap();
    assert_eq!(plan.new_names_text, native_names);

    // DBSchema: every table entry is the platform's except the two system tables it upgraded on its
    // own (a platform-build drift, not a consequence of the change); the table order differs only by
    // where those two sit.
    let ours = DbSchema::parse(&plan.new_schema).unwrap();
    let native = DbSchema::parse(&after.schema()).unwrap();
    assert_eq!(ours.len(), native.len());
    let by_name = |schema: &DbSchema| -> BTreeMap<String, crate::metadata_model::brace::Brace> {
        (0..schema.len())
            .map(|position| {
                let table = &schema.tables()[position];
                (
                    schema.view(position).unwrap().name().to_owned(),
                    table.clone(),
                )
            })
            .collect()
    };
    let (ours_map, native_map) = (by_name(&ours), by_name(&native));
    assert_eq!(
        ours_map.keys().collect::<BTreeSet<_>>(),
        native_map.keys().collect::<BTreeSet<_>>()
    );
    let differing: Vec<_> = ours_map
        .iter()
        .filter(|(name, entry)| native_map[*name] != **entry)
        .map(|(name, _)| name.as_str())
        .collect();
    // (A base that had a native apply already has them upgraded: nothing differs there.)
    assert!(
        differing
            .iter()
            .all(|name| ["DbCopies", "DbCopiesUpdates"].contains(name)),
        "{differing:?}"
    );
    // The created tables (S1-F) and then the rebuilt ones sit at the end, before ConfigChngR (the last table of the БСП),
    // in the order they were made (`DbSchema::insert_rebuilt`).
    assert_eq!(ours.position("ConfigChngR"), Some(ours.len() - 1));
    let rebuilt: Vec<&str> = plan
        .objects
        .iter()
        .filter(|object| object.created)
        .chain(plan.objects.iter().filter(|object| !object.created))
        .map(|object| object.object.as_str())
        .collect();
    let tail: Vec<String> = (ours.len() - 1 - rebuilt.len()..ours.len() - 1)
        .map(|position| ours.view(position).unwrap().name().to_owned())
        .collect();
    assert_eq!(tail, rebuilt);
}

/// The plan of case a2 made from the staged snapshot against what the native apply produced.
#[test]
fn corpus_plan_of_case_a2_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_bsp8327_a", "a2_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_bsp8327_a", "a2_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of case a2");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    assert_eq!(plan.objects.len(), 1);
    assert_eq!(plan.objects[0].object, "Reference20");
    assert_equals_native(&plan, &after);

    // siVersions: the same rows in the same order, new versions for the two rows we rewrote.
    let versions = String::from_utf8(
        plan.caches
            .iter()
            .find(|cache| cache.row_name == "siVersions")
            .unwrap()
            .row
            .clone(),
    )
    .unwrap();
    let before = String::from_utf8(staged.row("Params", "siVersions").unwrap()).unwrap();
    let words = |text: &str| -> Vec<String> {
        text.split([',', '"', '{', '}'])
            .filter(|word| !word.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let (old_words, new_words) = (words(&before), words(&versions));
    assert_eq!(old_words.len(), new_words.len());
    let mut changed: Vec<String> = old_words
        .iter()
        .zip(&new_words)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(index, _)| old_words[index - 1].clone())
        .collect();
    changed.sort();
    assert_eq!(
        changed,
        [
            "1a621f0f-5568-4183-bd9f-f6ef670e7090.si",
            "ea13a2c9-0c2f-40fa-b855-710387e3271d.si"
        ]
        .map(str::to_owned)
        .to_vec()
    );
}

/// The types case (S1 step 1): attributes of every primitive type on five catalogs and a document, one
/// stage, against the native apply of the same stage.
#[test]
fn corpus_plan_of_the_types_case_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s1_base", "t1_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s1_t1_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the types case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    let objects: Vec<&str> = plan
        .objects
        .iter()
        .map(|object| object.object.as_str())
        .collect();
    // The platform's order: the configuration's own (not the table numbers, not the names).
    assert_eq!(
        objects,
        [
            "Reference569",
            "Reference16",
            "Reference20",
            "Reference2598",
            "Reference9367",
            "Document39"
        ]
    );
    assert_equals_native(&plan, &after);
}

/// S1-B: eleven attributes deleted in six objects of the pristine БСП -- the middle, the last, the first,
/// indexed ones (of a flat catalog, a hierarchical one and a document), the only attribute of an object,
/// the field of a hierarchical catalog that is nullable -- and two replaced (deleted and added in one
/// stage), against the native apply of the same stage.
#[test]
fn corpus_plan_of_the_deletion_case_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_b1_base", "b1_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_b1_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the deletion case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    let removed: usize = plan
        .objects
        .iter()
        .map(|object| object.removals.len())
        .sum();
    let added: usize = plan
        .objects
        .iter()
        .map(|object| object.additions.len())
        .sum();
    assert_eq!((removed, added), (11, 2));
    assert_eq!(plan.objects.len(), 6);
    assert_equals_native(&plan, &after);
}

/// S1-C: six variable strings widened in five objects of the pristine БСП -- a catalog, a hierarchical
/// catalog (two attributes, one indexed), a document (one indexed, one widened by a single character), the
/// widest allowed (1024) -- against the native apply of the same stage.
#[test]
fn corpus_plan_of_the_widening_case_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_c1_base", "c1_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_c1_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the widening case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    let widened: usize = plan
        .objects
        .iter()
        .map(|object| object.widenings.len())
        .sum();
    assert_eq!(widened, 6);
    assert_eq!(plan.objects.len(), 5);
    // Nothing is added or removed, so no cache row is written (XDTO, registry and siVersions stay): the
    // platform, too, left the text of all sixteen rows as it was.
    assert!(plan.caches.is_empty());
    let rows = staged.rows("Params");
    let mut compared = 0;
    for (name, part) in rows.keys().filter(|(name, _)| name.ends_with(".si")) {
        let before = inflate(&staged.row("Params", name).unwrap()).unwrap();
        let native = inflate(&after.row("Params", name).unwrap()).unwrap();
        assert!(before == native, "{name} ({part}) changed natively");
        compared += 1;
    }
    assert_eq!(compared, 16);
    assert_equals_native(&plan, &after);
}

/// S1-B, the additional-order index: an attribute of a catalog and one of a document that have it are
/// deleted; the catalog loses the index of the attribute, the document keeps `ByDocDate` (which listed the
/// attribute last) and loses the field from its list.
#[test]
fn corpus_plan_of_the_additional_order_deletion_case_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_b2_base", "b2_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_b2_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the additional-order deletion case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    let removed: usize = plan
        .objects
        .iter()
        .map(|object| object.removals.len())
        .sum();
    assert_eq!((removed, plan.objects.len()), (2, 2));
    assert!(
        plan.objects
            .iter()
            .flat_map(|object| object.removals.iter())
            .any(|removal| removal
                .indexes
                .iter()
                .any(|name| name.starts_with("ByDocDate")))
    );
    assert_equals_native(&plan, &after);
}

/// A plan that writes no cache row, against the platform's apply that left the text of all sixteen `.si` rows
/// as it was.
fn assert_no_cache_change(
    plan: &crate::restructure::plan::Plan,
    staged: &Snapshot,
    after: &Snapshot,
) {
    assert!(plan.caches.is_empty());
    let rows = staged.rows("Params");
    let mut compared = 0;
    for (name, part) in rows.keys().filter(|(name, _)| name.ends_with(".si")) {
        let before = inflate(&staged.row("Params", name).unwrap()).unwrap();
        let native = inflate(&after.row("Params", name).unwrap()).unwrap();
        assert!(before == native, "{name} ({part}) changed natively");
        compared += 1;
    }
    assert_eq!(compared, 16);
}

/// S1-D, the trace of one flag alone: `ЦелевоеВремя` of `КлючевыеОперации` (a number of a flat catalog)
/// goes from `DontIndex` to `Index`; against the native apply of the same stage.
#[test]
fn corpus_plan_of_the_index_flag_alone_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_d0_base", "d0_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_d0_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the index flag case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    assert_eq!(plan.objects.len(), 1);
    let switches: Vec<_> = plan.switches().collect();
    assert_eq!(switches.len(), 1);
    assert_eq!((switches[0].from, switches[0].to), (0, 1));
    assert_eq!(switches[0].added, ["ByFieldFld2646"]);
    assert_no_cache_change(&plan, &staged, &after);
    assert_equals_native(&plan, &after);
}

/// S1-D: the index flag on and off in six objects of the pristine БСП -- a number and a string, a flat
/// catalog, a hierarchical one (a pair of indexes per attribute), a document; with the additional order (a
/// hierarchical catalog, a document whose date index lists the attribute) on and off; against the native
/// apply of the same stage.
#[test]
fn corpus_plan_of_the_index_flags_on_and_off_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_d1_base", "d1_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_d1_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the index flags case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    assert_eq!(plan.objects.len(), 6);
    let switches: Vec<_> = plan.switches().collect();
    assert_eq!(switches.len(), 12);
    assert_eq!(switches.iter().filter(|switch| switch.to == 0).count(), 6);
    assert_eq!(switches.iter().filter(|switch| switch.to == 2).count(), 2);
    assert_no_cache_change(&plan, &staged, &after);
    assert_equals_native(&plan, &after);
}

/// S1-E: new attributes in tabular sections that were there -- a document's section (two attributes, one first
/// and one last, one indexed), a catalog's sections in a flat and a hierarchical catalog (declared indexes
/// already there) -- against the native apply of the same stage.
#[test]
fn corpus_plan_of_the_attributes_of_existing_sections_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_e3_base", "e3_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_e3_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the section attributes case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    eprintln!("{}", plan.summary());
    assert_eq!(plan.objects.len(), 4);
    assert!(plan.objects.iter().all(|object| {
        object
            .sections
            .iter()
            .all(|section| section.created.is_none())
    }));
    assert_equals_native(&plan, &after);
}

/// S1-E: new tabular sections -- a catalog and a document, a hierarchical catalog, a flat one, sections with
/// indexed attributes, a section in an object that also gets a new own attribute and a new attribute in a
/// section it had -- against the native apply of the same stage (case e4: a catalog with one new section and a
/// document with two).
#[test]
fn corpus_plan_of_new_sections_in_a_catalog_and_a_document_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_e4_base", "e4_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_e4_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the new sections case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    eprintln!("{}", plan.summary());
    assert_eq!(plan.objects.len(), 2);
    assert_equals_native(&plan, &after);
}

/// S1-E: five new tabular sections in one stage -- three catalogs (one hierarchical) and two documents.
#[test]
fn corpus_plan_of_five_new_sections_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_e1_base", "e1_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_e1_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of the five sections case");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    eprintln!("{}", plan.summary());
    assert_eq!(plan.objects.len(), 5);
    assert_equals_native(&plan, &after);
}

/// S1-E, the shapes of e1 on objects no extension adopts (the twin runs through the gate refuse an adopted one): a flat
/// catalog, a hierarchical one that has a section already, a subordinate one, two documents (one has a section already).
#[test]
fn corpus_plan_of_new_sections_on_objects_no_extension_adopts_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_e5_base", "e5_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_e5_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of case e5");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    eprintln!("{}", plan.summary());
    assert_eq!(plan.objects.len(), 5);
    assert_eq!(
        plan.objects
            .iter()
            .flat_map(|object| &object.sections)
            .filter(|section| section.created.is_some())
            .count(),
        5
    );
    assert_equals_native(&plan, &after);
}

/// S1-E, the shapes of e3 on such objects: a hierarchical catalog, a subordinate catalog whose section has indexed attributes
/// already (new indexed ones first and in the middle), a document (one first, one indexed last).
#[test]
fn corpus_plan_of_section_attributes_on_objects_no_extension_adopts_equals_the_native_result() {
    let (Some(staged), Some(after)) = (
        Snapshot::open("ibcmd_rs_04_ddl_s2_e6_base", "e6_staged"),
        Snapshot::open("ibcmd_rs_04_ddl_s2_e6_nat", "nat_after"),
    ) else {
        eprintln!("skipped: no lab snapshots of case e6");
        return;
    };
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    eprintln!("{}", plan.summary());
    assert_eq!(plan.objects.len(), 3);
    assert!(
        plan.objects
            .iter()
            .flat_map(|object| &object.sections)
            .all(|section| section.created.is_none())
    );
    assert_equals_native(&plan, &after);
}

/// The `.si` rows the plan does not write are the rows the platform left as they were.
fn assert_other_caches_unchanged(
    plan: &crate::restructure::plan::Plan,
    staged: &Snapshot,
    after: &Snapshot,
) {
    let written: BTreeSet<&str> = plan
        .caches
        .iter()
        .map(|cache| cache.row_name.as_str())
        .collect();
    let mut compared = 0;
    for (name, _) in staged
        .rows("Params")
        .keys()
        .filter(|(name, _)| name.ends_with(".si"))
    {
        if written.contains(name.as_str()) {
            continue;
        }
        let before = inflate(&staged.row("Params", name).unwrap()).unwrap();
        let native = inflate(&after.row("Params", name).unwrap()).unwrap();
        assert!(
            before == native,
            "{name} changed natively and the plan does not write it"
        );
        compared += 1;
    }
    assert!(compared > 0);
}

/// S1 combinations (`edit_cases_s4.py`, `mix_case.ps1`): the plan of a stage that mixes the built operations, made offline
/// from the staged snapshot, against the native apply of the same stage -- `DBNames`, every `DBSchema` entry, every cache row
/// the plan writes, and the rows it does not write are the ones the platform left alone. Returns the plan for the test's own
/// counts.
fn corpus_mix(case: &str) -> Option<crate::restructure::plan::Plan> {
    let staged = Snapshot::open(
        &format!("ibcmd_rs_04_ddl_s2_{case}_base"),
        &format!("{case}_staged"),
    )?;
    let after = Snapshot::open(&format!("ibcmd_rs_04_ddl_s2_{case}_nat"), "nat_after")?;
    let plan = plan(&inputs_of(&staged), &PlanOptions::default()).unwrap();
    eprintln!("{case}: {}", plan.summary());
    assert_equals_native(&plan, &after);
    assert_other_caches_unchanged(&plan, &staged, &after);
    Some(plan)
}

fn operations(plan: &crate::restructure::plan::Plan) -> [usize; 6] {
    let count = |pick: fn(&crate::restructure::plan::ObjectPlan) -> usize| -> usize {
        plan.objects.iter().map(pick).sum()
    };
    [
        count(|object| object.additions.len()),
        count(|object| object.removals.len()),
        count(|object| object.widenings.len()),
        count(|object| object.switches.len()),
        count(|object| {
            object
                .sections
                .iter()
                .filter(|section| section.created.is_some())
                .count()
        }),
        count(|object| {
            object
                .sections
                .iter()
                .filter(|section| section.created.is_none())
                .map(|section| section.additions.len())
                .sum()
        }),
    ]
}

/// Mix m1: attributes deleted and strings widened in the same objects (a hierarchical catalog, two flat ones).
#[test]
fn corpus_mix_delete_and_widen_in_the_same_objects_equals_the_native_result() {
    let Some(plan) = corpus_mix("m1") else {
        eprintln!("skipped: no lab snapshots of mix m1");
        return;
    };
    // [add, delete, widen, switch, new sections, new section attributes]
    assert_eq!(operations(&plan), [0, 4, 6, 0, 0, 0]);
    assert_eq!(plan.objects.len(), 3);
}

/// Mix m4: attributes deleted in some objects, tabular sections and section attributes added in others.
#[test]
fn corpus_mix_delete_in_some_objects_and_sections_in_others_equals_the_native_result() {
    let Some(plan) = corpus_mix("m4") else {
        eprintln!("skipped: no lab snapshots of mix m4");
        return;
    };
    assert_eq!(operations(&plan), [0, 3, 0, 0, 2, 2]);
    assert_eq!(plan.objects.len(), 5);
}

/// Mix m2: the same attribute widened and re-indexed (on, off, the additional order), in a hierarchical catalog, a flat one and a
/// document.
#[test]
fn corpus_mix_widen_and_switch_the_index_of_the_same_attribute_equals_the_native_result() {
    let Some(plan) = corpus_mix("m2") else {
        eprintln!("skipped: no lab snapshots of mix m2");
        return;
    };
    assert_eq!(operations(&plan), [0, 0, 6, 6, 0, 0]);
    assert_eq!(plan.objects.len(), 3);
    // Nothing is added or removed, so no cache row is written.
    assert!(plan.caches.is_empty());
}

/// Mix m5: every operation in one stage on six objects -- a document and a hierarchical catalog get an own attribute, a section
/// and a section attribute; two catalogs are deleted from, widened, re-indexed; one gets a section.
#[test]
fn corpus_mix_every_operation_in_one_stage_on_six_objects_equals_the_native_result() {
    let Some(plan) = corpus_mix("m5") else {
        eprintln!("skipped: no lab snapshots of mix m5");
        return;
    };
    assert_eq!(operations(&plan), [3, 2, 2, 4, 2, 2]);
    assert_eq!(plan.objects.len(), 6);
}

/// Mix m3: a new section and a widened own attribute in one object; a document with a new section, a new attribute of an old section,
/// a widened attribute and a new own attribute; a document with a new section and a new own attribute.
#[test]
fn corpus_mix_sections_with_widened_and_new_own_attributes_equals_the_native_result() {
    let Some(plan) = corpus_mix("m3") else {
        eprintln!("skipped: no lab snapshots of mix m3");
        return;
    };
    assert_eq!(operations(&plan), [2, 0, 2, 0, 3, 1]);
    assert_eq!(plan.objects.len(), 3);
}

/// Mix m7: attributes that are added with the index flag (`Index`, `IndexWithAdditionalOrder`), in a flat catalog, a hierarchical
/// catalog (nullable ones too) and a document: the declared indexes are the ones of a switch.
#[test]
fn corpus_mix_new_attributes_that_come_indexed_equals_the_native_result() {
    let Some(plan) = corpus_mix("m7") else {
        eprintln!("skipped: no lab snapshots of mix m7");
        return;
    };
    assert_eq!(operations(&plan), [7, 0, 0, 0, 0, 0]);
    assert_eq!(plan.objects.len(), 5);
    assert!(
        plan.objects
            .iter()
            .flat_map(|object| &object.additions)
            .all(|addition| addition.indexing != 0 && !addition.indexes.is_empty())
    );
}

/// Mix m6, the large stage: 17 objects, every operation -- deletes, widenings and re-indexing in catalogs and documents that come
/// with new own attributes, new sections and new attributes of old sections in other objects, all in one plan.
#[test]
fn corpus_mix_one_large_stage_on_seventeen_objects_equals_the_native_result() {
    let Some(plan) = corpus_mix("m6") else {
        eprintln!("skipped: no lab snapshots of mix m6");
        return;
    };
    assert_eq!(operations(&plan), [4, 4, 6, 8, 4, 4]);
    assert_eq!(plan.objects.len(), 17);
}

/// Mix f1 (S1-F): a new flat catalog next to two widened strings and three switched indexes of other objects (a catalog, a
/// hierarchical one, a document twice).
#[test]
fn corpus_mix_a_new_catalog_next_to_widenings_and_index_switches_equals_the_native_result() {
    let Some(plan) = corpus_mix("f1") else {
        eprintln!("skipped: no lab snapshots of mix f1");
        return;
    };
    assert_eq!(operations(&plan), [0, 0, 2, 3, 0, 0]);
    assert_eq!(plan.objects.len(), 5);
    assert_eq!(
        plan.objects.iter().filter(|object| object.created).count(),
        1
    );
}

/// Mix f3 (S1-F): a new catalog and a new document next to two new own attributes, a widened string and two switched indexes.
#[test]
fn corpus_mix_a_new_catalog_and_document_next_to_every_attribute_operation_equals_the_native_result()
 {
    let Some(plan) = corpus_mix("f3") else {
        eprintln!("skipped: no lab snapshots of mix f3");
        return;
    };
    assert_eq!(operations(&plan), [2, 0, 1, 2, 0, 0]);
    assert_eq!(plan.objects.len(), 7);
    assert_eq!(
        plan.objects.iter().filter(|object| object.created).count(),
        2
    );
}
