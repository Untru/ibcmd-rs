//! The extensions of the infobase and the own restructure (S1-I, #405).
//!
//! What the storage of an infobase with extensions looks like, measured on the БСП 8.3.27 corpus (four
//! extensions, `docs/apply/restructuring-extensions.md`):
//!
//! - `SchemaStorage(1)` holds the schema of the extensions (243 entries, 277 physical tables named with the
//!   suffix `X1`: `_Reference7366X1`), beside the main schema `SchemaStorage(0)`. Only the extensions that
//!   own tables have a `Params` row `DBNames-Ext-<registry id>` and a `DBNamesVersion-DBNames-Ext-<registry id>`
//!   (two of four; the third row, `DBNames-Ext-1`, is an empty list with a header of its own). The number
//!   counter is shared with the main `DBNames`: a new number is one more than the largest header of all of them.
//! - An extension adds to an object it **adopts** in a table of its own, never in the main one: the attribute
//!   `Fld7840` an extension gives the adopted catalog `_ДемоНоменклатура` sits in `Reference18` of the extension
//!   schema (`_Reference18X1`), the very number of the main table `_Reference18`, and not in `SchemaStorage(0)`.
//!   An adopted object the extension does not extend with data has no table of its own.
//! - The platform pairs an adopted object with the object it extends by identity, not by uuid: 67 of the
//!   84 adopted headers of the extension `_ДемоРасширение` carry the nil uuid where the base object would be
//!   named, and the object has a uuid of its own (`mssql_dump::extension::AdoptedObject`).
//! - A native `config apply` of a change of an object an extension adopts (case a2: `_ДемоПартнеры`, adopted by
//!   `_ДемоРасширение`, which has no table of its own for it) rebuilt the main tables only: `SchemaStorage(1)`,
//!   the `X1` tables, `DBNames-Ext-*`, `_ExtensionsInfo` and `_ExtensionsRestruct` were as they were. The same
//!   for a change of an object no extension adopts (case n: `_ДемоСтавкиНДС`). What a native apply does write
//!   for the extensions is `_ExtensionsRestructNGS`: one row per extension that owns tables (three rows of
//!   13 890, 444 and 3 864 bytes on the БСП clone, keyed by the extension's registry id), which stay after the
//!   apply. They are derived state, written again by the next native apply; the own restructure leaves them
//!   alone, and a database that holds them is not "in flight".
//!
//! What S1 does with it, fail closed:
//!
//! 1. **Read** ([`read_state`], `read_adoptions`): the number of registered extensions, the tables of the
//!    extension schema, whether that schema is idle, and the objects every extension adopts, in its active
//!    image and in its staged one.
//! 2. **Refuse** ([`check`]) when an extension adopts the changed object, by uuid, by the uuid it extends or
//!    by name (the kind is not read off an extension row, so a same-named object of another kind refuses
//!    too: the wrong side to be wrong on); when the infobase has extensions and their objects were not read;
//!    when the extension schema is not idle (`SchemaStorage(1)`: a restructure of the extensions was
//!    interrupted). The message names
//!    the extensions, and the tables of their own that the extension keeps for the object, because those
//!    are what a rebuild of the main table would leave out of step. Whether an adopted object without such
//!    tables is harmless is what case a2 says (the platform left the extensions alone); it is not what S1-I
//!    accepts: the criterion is that an adopted object is refused.
//! 3. **Prove** ([`fingerprint`]): the restructure reads the state of the extensions before its first
//!    statement and after its last, in its own transaction, and rolls back when a part differs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::restructure::reader::{RowSource, rows};
use crate::restructure::schema::{DbSchema, TableView};
use crate::restructure::storage::SchemaStorageRow;
use ibcmd_restructure::descriptor::AdoptedObject;

/// One object an extension adopts, and the image it was read from.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Adoption {
    /// The extension's name.
    pub extension: String,
    /// `active` or `staged`.
    pub image: &'static str,
    pub object: AdoptedObject,
}

/// What the plan knows about the extensions of the infobase.
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct ExtensionInputs {
    /// Rows of `_ExtensionsInfo`: how many extensions the infobase has.
    pub registered: usize,
    /// The objects they adopt were read (`read_adoptions`); a plan of an infobase with extensions
    /// refuses without it.
    pub adoptions_read: bool,
    pub adoptions: Vec<Adoption>,
    /// The tables of the extension schema (`SchemaStorage(1)`) by their `DBSchema` name: `Reference18`
    /// stands for `_Reference18X1`.
    pub tables: BTreeSet<String>,
    /// Why the extension schema is not in a state a restructure may start from, when it is not.
    pub schema_busy: Option<String>,
}

/// An object the restructure changes.
#[derive(Clone, Copy, Debug)]
pub struct ChangedObject<'a> {
    /// `Catalog`, `Document`.
    pub kind: &'a str,
    pub name: &'a str,
    pub uuid: &'a str,
    /// The main table by its `DBSchema` name: `Reference18`.
    pub table: &'a str,
}

/// Why the extensions stop a restructure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The infobase has extensions and the plan was not given their objects.
    NotRead { registered: usize },
    /// `SchemaStorage(1)` is not idle.
    SchemaBusy { reason: String },
    /// An extension adopts the object.
    Adopted {
        object: String,
        adoptions: Vec<Adoption>,
        /// The tables the extensions keep for the object (`Reference18`), `X1` left off.
        extension_tables: Vec<String>,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotRead { registered } => write!(
                formatter,
                "the infobase has {registered} extension(s) and the objects they adopt were not read: \
                 the restructure cannot tell whether one of them adopts the changed object"
            ),
            Self::SchemaBusy { reason } => write!(
                formatter,
                "the schema of the extensions is not idle ({reason}): an interrupted restructure of the \
                 extensions has to be finished first"
            ),
            Self::Adopted {
                object,
                adoptions,
                extension_tables,
            } => {
                let by = adoptions
                    .iter()
                    .map(|adoption| {
                        format!(
                            "{} (object {} {}, {} image)",
                            adoption.extension,
                            adoption.object.name,
                            adoption.object.uuid,
                            adoption.image
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                write!(formatter, "{object} is adopted by the extension {by}")?;
                if extension_tables.is_empty() {
                    write!(
                        formatter,
                        "; the extension keeps no table of its own for it"
                    )?;
                } else {
                    write!(
                        formatter,
                        "; the extension keeps tables of its own for it ({})",
                        extension_tables
                            .iter()
                            .map(|table| format!("_{table}X1"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )?;
                }
                write!(
                    formatter,
                    ": the own restructure does not change an object an extension adopts, the platform's \
                     apply does"
                )
            }
        }
    }
}

impl std::error::Error for Refusal {}

/// Whether `adoption` is of `object`: the same uuid, the uuid the header names, or the same name.
fn adopts(adoption: &Adoption, object: &ChangedObject<'_>) -> bool {
    let adopted = &adoption.object;
    let uuid = object.uuid.to_ascii_lowercase();
    adopted.uuid.eq_ignore_ascii_case(&uuid)
        || adopted
            .extends
            .as_deref()
            .is_some_and(|extends| extends.eq_ignore_ascii_case(&uuid))
        || (!adopted.name.is_empty() && adopted.name.eq_ignore_ascii_case(object.name))
}

/// The tables of the extension schema that belong to `table`: itself and its sub-tables.
fn tables_of(tables: &BTreeSet<String>, table: &str) -> Vec<String> {
    let sub = format!("{table}_");
    tables
        .iter()
        .filter(|name| name.as_str() == table || name.starts_with(&sub))
        .cloned()
        .collect()
}

/// Refuses when the extensions stand in the way of changing `changed`.
pub fn check(inputs: &ExtensionInputs, changed: &[ChangedObject<'_>]) -> Result<(), Refusal> {
    if let Some(reason) = &inputs.schema_busy {
        return Err(Refusal::SchemaBusy {
            reason: reason.clone(),
        });
    }
    if inputs.registered == 0 {
        return Ok(());
    }
    if !inputs.adoptions_read {
        return Err(Refusal::NotRead {
            registered: inputs.registered,
        });
    }
    for object in changed {
        let adoptions: Vec<Adoption> = inputs
            .adoptions
            .iter()
            .filter(|adoption| adopts(adoption, object))
            .cloned()
            .collect();
        if !adoptions.is_empty() {
            return Err(Refusal::Adopted {
                object: format!("{} {}", object.kind, object.name),
                adoptions,
                extension_tables: tables_of(&inputs.tables, object.table),
            });
        }
    }
    Ok(())
}

/// The table names of an extension schema (`SchemaStorage(1)`), main tables and sub-tables.
pub fn schema_tables(current_schema: &[u8]) -> Result<BTreeSet<String>> {
    let schema = DbSchema::parse(current_schema).context("the DBSchema of the extensions")?;
    let mut names = BTreeSet::new();
    for node in schema.tables() {
        names.insert(TableView::new(node)?.name().to_owned());
    }
    Ok(names)
}

fn scalar(connection: &mut dyn RowSource, query: &str) -> Result<i64> {
    let mut value = 0;
    rows(connection, query, |row| {
        value = row.i64(0)?;
        Ok(())
    })
    .with_context(|| query.to_owned())?;
    Ok(value)
}

/// Rows of a table the infobase may not have (the count of `sys.partitions`, so no scan).
fn table_rows(connection: &mut dyn RowSource, table: &str) -> Result<i64> {
    scalar(
        connection,
        &format!(
            "SELECT CAST(ISNULL(SUM(p.rows), 0) AS bigint) FROM sys.tables t \
             JOIN sys.partitions p ON p.object_id = t.object_id AND p.index_id IN (0, 1) \
             WHERE t.name = N'{table}'"
        ),
    )
}

/// What the plan reads of the extensions through the connection it already has: `storage` are the
/// `SchemaStorage` rows it read.
pub fn read_state(
    connection: &mut dyn RowSource,
    storage: &[SchemaStorageRow],
) -> Result<ExtensionInputs> {
    let mut inputs = ExtensionInputs {
        registered: usize::try_from(table_rows(connection, "_ExtensionsInfo")?).unwrap_or(0),
        ..ExtensionInputs::default()
    };
    if let Some(row) = storage.iter().find(|row| row.schema_id == 1) {
        inputs.tables = schema_tables(&row.current_schema)?;
        if !row.is_idle() {
            inputs.schema_busy = Some(format!(
                "SchemaStorage(1) has status {} and generations of {} / {} bytes",
                row.status,
                row.new_gen_created.len(),
                row.new_gen_dropped.len()
            ));
        }
    }
    Ok(inputs)
}

/// The state of everything the extensions keep, as a digest per part. A restructure of the main
/// configuration leaves every part as it was.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    parts: BTreeMap<String, String>,
}

fn sha256_hex(chunks: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for chunk in chunks {
        hasher.update((chunk.len() as u64).to_le_bytes());
        hasher.update(chunk);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Reads the state of the extensions: the `SchemaStorage` rows but the main one, the extensions'
/// `DBNames` rows, the registry, the restructure bookkeeping, and the row count of every table of the
/// extension schema.
pub fn fingerprint(connection: &mut dyn RowSource) -> Result<Fingerprint> {
    let mut parts = BTreeMap::new();
    let mut schema = None;
    rows(
        connection,
        "SELECT SchemaID, Status, CurrentSchema, NewGenCreated, NewGenDropped FROM dbo.SchemaStorage \
         WHERE SchemaID <> 0 ORDER BY SchemaID",
        |mut row| {
            let id = row.i64(0)?;
            let status = row.i64(1)?.to_le_bytes();
            let current = row.take_binary(2)?;
            let created = row.take_binary(3)?;
            let dropped = row.take_binary(4)?;
            parts.insert(
                format!("SchemaStorage({id})"),
                sha256_hex(&[&status, &current, &created, &dropped]),
            );
            if id == 1 {
                schema = Some(current);
            }
            Ok(())
        },
    )
    .context("SchemaStorage of the extensions")?;
    rows(
        connection,
        "SELECT FileName, PartNo, BinaryData FROM dbo.Params \
         WHERE FileName LIKE N'DBNames%-Ext-%' ORDER BY FileName, PartNo",
        |mut row| {
            let name = row.take_text(0)?;
            let part = row.i64(1)?;
            let data = row.take_binary(2)?;
            parts.insert(
                format!("Params {name}#{part}"),
                sha256_hex(&[name.as_bytes(), &data]),
            );
            Ok(())
        },
    )
    .context("Params DBNames-Ext-*")?;
    for table in [
        "_ExtensionsInfo",
        "_ExtensionsRestruct",
        "_ExtensionsRestructNGS",
    ] {
        if !table_exists(connection, table)? {
            continue;
        }
        let mut counted = (0, 0);
        rows(
            connection,
            &format!(
                "SELECT COUNT_BIG(*), CAST(ISNULL(CHECKSUM_AGG(BINARY_CHECKSUM(*)), 0) AS bigint) FROM dbo.{table}"
            ),
            |row| {
                counted = (row.i64(0)?, row.i64(1)?);
                Ok(())
            },
        )
        .with_context(|| table.to_owned())?;
        parts.insert(
            table.to_owned(),
            format!("{} rows, checksum {}", counted.0, counted.1),
        );
    }
    if let Some(schema) = schema {
        for name in schema_tables(&schema)? {
            let table = format!("_{name}X1");
            parts.insert(
                format!("table {table}"),
                format!("{} rows", table_rows(connection, &table)?),
            );
        }
    }
    Ok(Fingerprint { parts })
}

fn table_exists(connection: &mut dyn RowSource, table: &str) -> Result<bool> {
    Ok(scalar(
        connection,
        &format!("SELECT COUNT_BIG(*) FROM sys.tables WHERE name = N'{table}'"),
    )? > 0)
}

impl Fingerprint {
    /// How many parts the fingerprint has.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The parts that differ from `other`, by name.
    pub fn differences(&self, other: &Fingerprint) -> Vec<String> {
        let mut names: BTreeSet<&String> = self.parts.keys().collect();
        names.extend(other.parts.keys());
        names
            .into_iter()
            .filter(|name| self.parts.get(*name) != other.parts.get(*name))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adoption(extension: &str, name: &str, uuid: &str, extends: Option<&str>) -> Adoption {
        Adoption {
            extension: extension.to_owned(),
            image: "active",
            object: AdoptedObject {
                row: uuid.to_owned(),
                uuid: uuid.to_owned(),
                name: name.to_owned(),
                extends: extends.map(str::to_owned),
            },
        }
    }

    const PARTNERS: &str = "5eab8a1b-070f-4dcf-bdcc-a259c62c3693";

    fn changed() -> ChangedObject<'static> {
        ChangedObject {
            kind: "Catalog",
            name: "_ДемоПартнеры",
            uuid: PARTNERS,
            table: "Reference20",
        }
    }

    fn inputs(adoptions: Vec<Adoption>, tables: &[&str]) -> ExtensionInputs {
        ExtensionInputs {
            registered: 4,
            adoptions_read: true,
            adoptions,
            tables: tables.iter().map(|table| (*table).to_owned()).collect(),
            ..ExtensionInputs::default()
        }
    }

    #[test]
    fn an_infobase_without_extensions_asks_nothing() {
        assert_eq!(check(&ExtensionInputs::default(), &[changed()]), Ok(()));
    }

    #[test]
    fn an_object_no_extension_adopts_passes() {
        let inputs = inputs(
            vec![
                adoption(
                    "_ДемоРасширение",
                    "_ДемоНоменклатура",
                    "2cb40f25-2c61-41e9-9689-e1e4c639e8cb",
                    None,
                ),
                adoption(
                    "ServiceDesk",
                    "Пользователи",
                    "11111111-2222-4333-8444-555555555555",
                    None,
                ),
            ],
            &["Reference18"],
        );
        assert_eq!(check(&inputs, &[changed()]), Ok(()));
    }

    #[test]
    fn an_adopted_object_is_refused_by_its_name() {
        // the extension's uuid is not the configuration's, and its header names no base object
        let inputs = inputs(
            vec![adoption(
                "_ДемоРасширение",
                "_ДемоПартнеры",
                "3014d9c1-cb00-49fb-81b3-e8ced354975f",
                None,
            )],
            &[],
        );
        let refusal = check(&inputs, &[changed()]).unwrap_err();
        let text = refusal.to_string();
        assert!(
            text.contains("Catalog _ДемоПартнеры is adopted by the extension _ДемоРасширение"),
            "{text}"
        );
        assert!(text.contains("keeps no table of its own"), "{text}");
        assert!(matches!(refusal, Refusal::Adopted { .. }));
    }

    #[test]
    fn an_adopted_object_is_refused_by_the_uuid_it_extends_or_its_own() {
        for adoption in [
            adoption(
                "E",
                "Другое",
                "3014d9c1-cb00-49fb-81b3-e8ced354975f",
                Some(PARTNERS),
            ),
            adoption("E", "Другое", &PARTNERS.to_ascii_uppercase(), None),
        ] {
            assert!(check(&inputs(vec![adoption], &[]), &[changed()]).is_err());
        }
    }

    #[test]
    fn the_message_names_the_tables_the_extension_keeps_for_the_object() {
        let inputs = inputs(
            vec![adoption(
                "ServiceDesk",
                "_ДемоПартнеры",
                "3014d9c1-cb00-49fb-81b3-e8ced354975f",
                None,
            )],
            &[
                "Reference20",
                "Reference20_VT77",
                "Reference200",
                "Reference21",
            ],
        );
        let text = check(&inputs, &[changed()]).unwrap_err().to_string();
        // the table and its sub-table, not the tables whose names only start alike
        assert!(
            text.contains("_Reference20X1, _Reference20_VT77X1"),
            "{text}"
        );
        assert!(!text.contains("Reference200"), "{text}");
    }

    #[test]
    fn extensions_whose_objects_were_not_read_refuse() {
        let inputs = ExtensionInputs {
            registered: 4,
            ..ExtensionInputs::default()
        };
        assert_eq!(
            check(&inputs, &[changed()]),
            Err(Refusal::NotRead { registered: 4 })
        );
    }

    #[test]
    fn a_busy_extension_schema_refuses() {
        let busy = ExtensionInputs {
            schema_busy: Some("status 200".to_owned()),
            ..ExtensionInputs::default()
        };
        assert!(matches!(
            check(&busy, &[changed()]),
            Err(Refusal::SchemaBusy { .. })
        ));
    }

    #[test]
    fn a_nameless_header_is_not_matched_by_name() {
        let inputs = inputs(
            vec![adoption(
                "E",
                "",
                "3014d9c1-cb00-49fb-81b3-e8ced354975f",
                None,
            )],
            &[],
        );
        let unnamed = ChangedObject {
            name: "",
            ..changed()
        };
        assert_eq!(check(&inputs, &[unnamed]), Ok(()));
    }

    #[test]
    fn fingerprints_name_the_parts_that_differ() {
        let one = Fingerprint {
            parts: BTreeMap::from([
                ("SchemaStorage(1)".to_owned(), "a".to_owned()),
                ("_ExtensionsInfo".to_owned(), "4 rows".to_owned()),
            ]),
        };
        assert!(one.differences(&one.clone()).is_empty());
        let mut two = one.clone();
        two.parts
            .insert("SchemaStorage(1)".to_owned(), "b".to_owned());
        two.parts
            .insert("table _Reference18X1".to_owned(), "3 rows".to_owned());
        assert_eq!(
            one.differences(&two),
            vec![
                "SchemaStorage(1)".to_owned(),
                "table _Reference18X1".to_owned()
            ]
        );
    }

    /// The four extensions of the БСП 8.3.27 corpus clone, as `mssql-restructure --extensions-report` read them
    /// (`tests/fixtures/native-evidence/extension-adoptions/bsp8327.json`): the three catalogs of the twin
    /// experiments (`docs/apply/restructuring-extensions.md`, section 3).
    #[test]
    fn the_corpus_extensions_refuse_the_adopted_catalogs_and_let_the_others_pass() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/native-evidence/extension-adoptions/bsp8327.json"
        ))
        .unwrap();
        let text = |value: &serde_json::Value, key: &str| value[key].as_str().unwrap().to_owned();
        let adoptions = fixture["adoptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| Adoption {
                extension: text(entry, "extension"),
                image: "active",
                object: AdoptedObject {
                    row: text(&entry["object"], "row"),
                    uuid: text(&entry["object"], "uuid"),
                    name: text(&entry["object"], "name"),
                    extends: entry["object"]["extends"].as_str().map(str::to_owned),
                },
            })
            .collect::<Vec<_>>();
        assert_eq!(adoptions.len(), 75);
        let inputs = ExtensionInputs {
            registered: 4,
            adoptions_read: true,
            adoptions,
            tables: fixture["tables"]
                .as_array()
                .unwrap()
                .iter()
                .map(|table| table.as_str().unwrap().to_owned())
                .collect(),
            schema_busy: None,
        };
        let catalog = |name, uuid, table| ChangedObject {
            kind: "Catalog",
            name,
            uuid,
            table,
        };
        // adopted by `_ДемоРасширение`, which keeps no table of its own for it
        let partners = catalog("_ДемоПартнеры", PARTNERS, "Reference20");
        let text = check(&inputs, &[partners]).unwrap_err().to_string();
        assert!(
            text.contains("adopted by the extension _ДемоРасширение"),
            "{text}"
        );
        assert!(text.contains("keeps no table of its own"), "{text}");
        // adopted, and extended with data: `_Reference18X1`
        let nomenclature = catalog(
            "_ДемоНоменклатура",
            "bb3d8c09-0a16-47ae-a113-d33038c15948",
            "Reference18",
        );
        let text = check(&inputs, &[nomenclature]).unwrap_err().to_string();
        assert!(
            text.contains("keeps tables of its own for it (_Reference18X1"),
            "{text}"
        );
        // adopted by nobody
        let vat = catalog(
            "_ДемоСтавкиНДС",
            "b78a9e4c-2486-4e73-81ed-3ee6ad7e3055",
            "Reference23",
        );
        assert_eq!(check(&inputs, &[vat]), Ok(()));
    }
}
