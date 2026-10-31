//! The bounded pending-generation shape measured on BSP 8.3.27.2214.
//! Nothing folds old aliases or edits the search-information payload: collection copies the
//! ordinary, unchanged service rows and refreshes their version tokens in one transaction.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{Result, anyhow, ensure};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::mssql_main_activation::{MAX_PLAN_BYTES, MAX_ROW_BYTES, MAX_ROWS};
use crate::sql::SqlClient;

use super::dynamic_metadata::Owners;
use super::model::{RowMeta, RowName, classify_name, hex_lower, hex_upper, quote_string};
use super::sqlgen::{self, ParamsRewrite};
use super::{ROW_COLUMNS, read_row_metas, si, versions};

/// The service classes present in both consecutive native collections. Other inventories have
/// not been measured, so accepting arbitrary `.si` rows would widen the feature without evidence.
const SERVICE_CLASSES: [&str; 16] = [
    "0b698dcd-501d-42d9-892d-5a9157bc996a",
    "1a621f0f-5568-4183-bd9f-f6ef670e7090",
    "215d232c-9c9e-4f7c-8a87-142cd3797264",
    "2203278d-ef4f-4f68-98f1-feb257d53ecc",
    "42ed49cc-765d-4314-bc2d-af425af7bf13",
    "59274b8d-4447-4bf4-9d29-bfa099a1de37",
    "a07b62f0-1f01-484a-93d9-d42764cedac0",
    "c40aafd6-c889-4229-807a-851d0bc5bc97",
    "c4629235-4823-4320-b8b5-1d08f4c6d612",
    "c77bc206-5935-48ea-b32e-508a572d94f4",
    "cf8b5e0f-5e46-4cf4-bc6f-204eae2c4e8a",
    "e05c0074-0404-4b7a-835e-9cacd405960e",
    "ea13a2c9-0c2f-40fa-b855-710387e3271d",
    "facbfffe-feb2-4d30-8930-a557b185e5c4",
    "fd1b2a86-b7df-4f32-84e2-befd4f3a2331",
    "fe8acd6a-22c9-4b5a-aeae-232a1c8324cb",
];

pub(super) const CONFIG_FILTER: &str =
    "FileName = N'DynamicallyUpdated' OR FileName LIKE N'%!_dynupdate!_%' ESCAPE N'!'";
const PARAMS_FILTER: &str = "FileName = N'siVersions' OR FileName LIKE N'%.si' OR FileName LIKE N'%!_dynupdate!_%' ESCAPE N'!'";

/// Bound before any blob read, and reject multipart, malformed metadata and duplicate keys.
fn bounded(rows: &[RowMeta]) -> Result<()> {
    ensure!(
        rows.len() <= MAX_ROWS,
        "pending service inventory exceeds {MAX_ROWS} rows"
    );
    let mut keys = HashSet::new();
    let mut bytes = 0usize;
    for row in rows {
        ensure!(
            row.part == 0 && row.attributes == 0 && row.data_size == row.byte_len,
            "{}: unmeasured pending row metadata",
            row.name
        );
        let len = usize::try_from(row.byte_len).map_err(|_| anyhow!("negative row length"))?;
        ensure!(
            len <= MAX_ROW_BYTES,
            "{}: service row is too large",
            row.name
        );
        bytes = bytes
            .checked_add(len)
            .ok_or_else(|| anyhow!("inventory size overflow"))?;
        ensure!(
            bytes <= MAX_PLAN_BYTES,
            "pending service inventory exceeds {MAX_PLAN_BYTES} bytes"
        );
        ensure!(keys.insert(row.key()), "duplicate pending row {}", row.name);
        ensure!(
            row.sha256.len() == 64 && row.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid service digest"
        );
    }
    Ok(())
}

/// Strict alias grammar; a generation must be canonical and present in the bound marker history.
fn alias(name: &str, history: &[String]) -> Result<(String, String)> {
    let lower = name.to_ascii_lowercase();
    let (head, tail) = lower
        .split_once("_dynupdate_")
        .ok_or_else(|| anyhow!("invalid alias {name}"))?;
    let generation = tail
        .get(..36)
        .ok_or_else(|| anyhow!("invalid alias generation {name}"))?;
    ensure!(
        Uuid::parse_str(generation)?.hyphenated().to_string() == generation
            && history.iter().any(|g| g == generation),
        "unbound alias generation {name}"
    );
    Ok((format!("{head}{}", &tail[36..]), generation.to_owned()))
}

pub(super) fn ordinary_alias_name(name: &str, history: &[String]) -> Result<String> {
    alias(name, history).map(|(ordinary, _)| ordinary)
}

pub(super) fn inventory(client: &dyn SqlClient, db: &str) -> Result<Vec<RowMeta>> {
    read_inventory(client, db, "Config", CONFIG_FILTER)
}

fn read_inventory(
    client: &dyn SqlClient,
    db: &str,
    table: &str,
    filter: &str,
) -> Result<Vec<RowMeta>> {
    let summary = client.query_rows(&format!("SELECT COUNT_BIG(*), ISNULL(SUM(CONVERT(bigint, DATALENGTH(BinaryData))), 0), ISNULL(MAX(CONVERT(bigint, DATALENGTH(BinaryData))), 0) FROM {db}.dbo.{table} WHERE {filter}"), &[])?;
    let size = summary
        .first()
        .ok_or_else(|| anyhow!("missing service inventory size"))?;
    let count = size.i64(0)?;
    ensure!(
        (0..=MAX_ROWS as i64).contains(&count)
            && size.i64(1)? <= MAX_PLAN_BYTES as i64
            && size.i64(2)? <= MAX_ROW_BYTES as i64,
        "unmeasured service inventory size"
    );
    // A concurrent phantom set cannot make metadata allocation unbounded after the size query.
    let rows = read_row_metas(
        client,
        &format!(
            "SELECT TOP ({}) {ROW_COLUMNS} FROM {db}.dbo.{table} WHERE {filter} ORDER BY FileName, PartNo",
            MAX_ROWS + 1
        ),
    )?;
    bounded(&rows)?;
    ensure!(
        rows.len() as i64 == count,
        "service inventory changed while being read"
    );
    Ok(rows)
}

pub(super) fn read_bound_blob(
    client: &dyn SqlClient,
    db: &str,
    table: &str,
    meta: &RowMeta,
) -> Result<Vec<u8>> {
    ensure!(
        (0..=MAX_ROW_BYTES as i64).contains(&meta.byte_len),
        "unbounded semantic row"
    );
    let mut blobs = Vec::new();
    client.read_rows(&format!("SELECT TOP (2) BinaryData FROM {db}.dbo.{table} WHERE FileName = N'{}' AND PartNo = {} AND CONVERT(bigint, DataSize) = {} AND DATALENGTH(BinaryData) = {} AND HASHBYTES('SHA2_256', BinaryData) = 0x{}",
        quote_string(&meta.name), meta.part, meta.data_size, meta.byte_len, meta.sha256), &[], &mut |mut row| {
        blobs.push(row.take_binary(0)?); Ok(())
    })?;
    ensure!(
        blobs.len() == 1,
        "semantic row {} changed after inventory",
        meta.name
    );
    let bytes = blobs.pop().expect("one bound row");
    bound_blob(meta, &bytes)?;
    Ok(bytes)
}

/// No structural removal: the list must name every existing alias and the marker, exactly once.
pub(super) fn judge_deleted(
    plain: &[u8],
    rows: &[RowMeta],
    history: &[String],
    kinds: &Owners,
) -> Result<Vec<String>> {
    let entries = super::parse_removals(plain).ok_or_else(|| anyhow!("unreadable deleted list"))?;
    ensure!(
        !entries.is_empty() && !history.is_empty(),
        "not a pending generation list"
    );
    ensure!(
        history.iter().collect::<HashSet<_>>().len() == history.len(),
        "duplicate marker history generation"
    );
    let first = &history[0];
    ensure!(
        !rows.iter().any(|row| row
            .name
            .eq_ignore_ascii_case(&format!("deleted_dynupdate_{first}"))),
        "the initial legacy generation has an unmeasured removal alias"
    );
    ensure!(
        history.iter().all(|generation| rows.iter().any(|row| row
            .name
            .eq_ignore_ascii_case(&format!("versions_dynupdate_{generation}")))),
        "history generation lacks its versions alias"
    );
    let expected: HashSet<_> = rows.iter().map(|r| r.name.to_ascii_lowercase()).collect();
    let mut names = HashSet::new();
    for (name, flag) in entries {
        let lower = name.to_ascii_lowercase();
        ensure!(
            flag == "0" && expected.contains(&lower) && names.insert(lower.clone()),
            "deleted: unmeasured removal or duplicate {name}"
        );
        if lower == "dynamicallyupdated" {
            continue;
        }
        let (ordinary, _) = alias(&lower, history)?;
        match classify_name(&ordinary) {
            RowName::Service("versions") | RowName::Other
                if ordinary == "versions" || ordinary == "deleted" => {}
            RowName::Descriptor(owner) | RowName::Body { owner, .. } => {
                ensure!(
                    kinds.admits(owner),
                    "deleted: unmeasured alias owner {name}"
                );
                if let RowName::Body { suffix, .. } = classify_name(&ordinary) {
                    ensure!(
                        kinds.role(owner, suffix).is_some(),
                        "deleted: unmeasured alias body {name}"
                    );
                }
            }
            _ => return Err(anyhow!("deleted: unmeasured alias {name}")),
        }
    }
    ensure!(
        names == expected,
        "deleted does not exactly cover the pending Config inventory"
    );
    let mut names: Vec<_> = names.into_iter().collect();
    names.sort();
    Ok(names)
}

/// Exact CAS of the full selected inventory, including phantom rows and physical metadata.
/// Called inside the engine's serializable transaction BEFORE any publication or no-op cleanup.
pub(super) fn guard(table: &str, filter: &str, rows: &[RowMeta]) -> String {
    let mut sql = format!(
        "-- bind pending {table} inventory before publication or no-op cleanup\nIF (SELECT COUNT_BIG(*) FROM dbo.{table} WITH (UPDLOCK, HOLDLOCK) WHERE {filter}) <> {} THROW 57209, 'Pending {table} inventory drifted', 1;\n",
        rows.len()
    );
    for row in rows {
        writeln!(sql, "IF (SELECT COUNT_BIG(*) FROM dbo.{table} WITH (UPDLOCK, HOLDLOCK) WHERE FileName = N'{}' AND PartNo = {} AND CONVERT(bigint, DataSize) = {} AND DATALENGTH(BinaryData) = {} AND Attributes = {} AND CONVERT(varchar(27), Creation, 121) = '{}' AND CONVERT(varchar(27), Modified, 121) = '{}' AND HASHBYTES('SHA2_256', BinaryData) = 0x{}) <> 1 THROW 57209, 'Pending {table} row drifted', 1;",
            quote_string(&row.name), row.part, row.data_size, row.byte_len, row.attributes,
            quote_string(&row.creation), quote_string(&row.modified), row.sha256).unwrap();
    }
    sql
}

pub(super) fn bound_blob(meta: &RowMeta, bytes: &[u8]) -> Result<()> {
    ensure!(
        usize::try_from(meta.byte_len).ok() == Some(bytes.len())
            && meta
                .sha256
                .eq_ignore_ascii_case(&hex_lower(&Sha256::digest(bytes))),
        "{}.binary data changed after the inventory was judged",
        meta.name
    );
    Ok(())
}

pub(super) struct Collection {
    pub guard_sql: String,
    pub rewrites: Vec<ParamsRewrite>,
    pub preimages: Vec<super::recovery::BoundParamsPreimage>,
    ordinary: Vec<RowMeta>,
}

fn service_names() -> BTreeSet<String> {
    SERVICE_CLASSES
        .iter()
        .map(|id| format!("{id}.si"))
        .collect()
}

/// Validate the entire siVersions map before changing any one token (no duplicate/unknown names).
fn version_names(plain: &[u8]) -> Result<BTreeSet<String>> {
    ensure!(plain.len() <= 16 * 1024, "unmeasured siVersions text size");
    let text = std::str::from_utf8(versions::strip_bom(plain))?.trim();
    let inner = text
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
        .ok_or_else(|| anyhow!("siVersions is not a flat list"))?;
    let fields: Vec<_> = inner.split(',').map(str::trim).collect();
    ensure!(
        fields.first() == Some(&"0") && fields.get(1) == Some(&"16") && fields.len() == 34,
        "unmeasured siVersions header/count"
    );
    let mut names = BTreeSet::new();
    for pair in fields[2..].chunks_exact(2) {
        let name = pair[0]
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .ok_or_else(|| anyhow!("unquoted SI name"))?
            .to_ascii_lowercase();
        ensure!(names.insert(name), "duplicate siVersions name");
        let version = Uuid::parse_str(pair[1])?;
        ensure!(!version.is_nil(), "nil SI version");
    }
    ensure!(
        names == service_names(),
        "unmeasured siVersions service inventory"
    );
    Ok(names)
}

pub(super) fn collect(client: &dyn SqlClient, db: &str, history: &[String]) -> Result<Collection> {
    let rows = read_inventory(client, db, "Params", PARAMS_FILTER)?;
    let ordinary = validate_service_rows(&rows, history)?;
    let by_name: HashMap<_, _> = rows
        .iter()
        .map(|r| (r.name.to_ascii_lowercase(), r))
        .collect();
    let version = by_name
        .get("siversions")
        .ok_or_else(|| anyhow!("Params lacks siVersions"))?;
    let bytes = read_bound_blob(client, db, "Params", version)?;
    let preimage = super::recovery::BoundParamsPreimage::new((*version).clone(), bytes.clone())?;
    // siVersions is RAW BOM brace text, unlike ordinary `.si` payloads. Preserve that storage
    // encoding; wrapped/deflated variants are unmeasured and fail the flat-map validator.
    let mut plain = bytes;
    for name in version_names(&plain)? {
        plain = si::set_si_version(&plain, &name, Uuid::new_v4())?;
    }
    Ok(Collection {
        guard_sql: guard("Params", PARAMS_FILTER, &rows),
        rewrites: vec![ParamsRewrite {
            file_name: "siVersions".to_owned(),
            old_data_size: version.data_size,
            old_sha256_hex: version.sha256.clone(),
            new_bytes: plain,
            set_creation: false,
        }],
        ordinary,
        preimages: vec![preimage],
    })
}

fn validate_service_rows(rows: &[RowMeta], history: &[String]) -> Result<Vec<RowMeta>> {
    bounded(rows)?;
    let expected = service_names();
    let mut ordinary = Vec::new();
    let mut generations: HashMap<String, BTreeSet<String>> = HashMap::new();
    let by_name: HashMap<_, _> = rows
        .iter()
        .map(|r| (r.name.to_ascii_lowercase(), r))
        .collect();
    for row in rows {
        let name = row.name.to_ascii_lowercase();
        if name == "siversions" {
            continue;
        }
        if name.contains("_dynupdate_") {
            let (base, generation) = alias(&name, history)?;
            ensure!(
                expected.contains(&base),
                "unmeasured Params alias {}",
                row.name
            );
            let source = by_name
                .get(&base)
                .ok_or_else(|| anyhow!("SI alias lacks ordinary source"))?;
            ensure!(
                row.sha256 == source.sha256 && row.data_size == source.data_size,
                "Params alias payload differs from ordinary source {}",
                row.name
            );
            ensure!(
                generations.entry(generation).or_default().insert(base),
                "duplicate SI alias"
            );
        } else {
            ensure!(
                expected.contains(&name),
                "unmeasured service row {}",
                row.name
            );
            ordinary.push(row.clone());
        }
    }
    ensure!(
        ordinary.len() == 16 && generations.values().all(|names| names == &expected),
        "incomplete service collection generation"
    );
    // The restored native baseline carries one legacy generation with no collected SI aliases.
    // Every subsequent force measured here adds all sixteen: absence of an entire later set is
    // corruption, not an empty collection. A complete first set has not been measured here and is
    // also refused. `history` is the ordered, equal Config/Params marker sequence, never a set.
    ensure!(
        !history.is_empty() && !generations.contains_key(&history[0]),
        "unmeasured collected initial generation"
    );
    ensure!(
        history
            .iter()
            .skip(1)
            .all(|generation| generations.contains_key(generation)),
        "a later history generation lacks its complete service collection"
    );
    ensure!(
        by_name.contains_key("siversions"),
        "Params lacks siVersions"
    );
    Ok(ordinary)
}

impl Collection {
    /// Generation names are chosen by the online engine; every source row was CAS-locked earlier.
    pub fn render(&self, generation: Uuid) -> String {
        let mut sql =
            String::from("-- native measured service collection: raw unchanged SI copies\n");
        for row in &self.ordinary {
            let lower = row.name.to_ascii_lowercase();
            let stem = lower.strip_suffix(".si").expect("validated SI name");
            let name = format!("{stem}_dynupdate_{generation}.si");
            writeln!(sql, "IF EXISTS (SELECT 1 FROM dbo.Params WITH (UPDLOCK, HOLDLOCK) WHERE FileName = N'{name}') THROW 57209, 'SI alias already exists', 1;\nINSERT dbo.Params (FileName, PartNo, Creation, Modified, Attributes, DataSize, BinaryData) SELECT N'{name}', PartNo, @now, @now, Attributes, DataSize, BinaryData FROM dbo.Params WHERE FileName = N'{}' AND PartNo = 0;\nIF @@ROWCOUNT <> 1 THROW 57209, 'SI alias copy failed', 1;", quote_string(&row.name)).unwrap();
        }
        for rewrite in &self.rewrites {
            writeln!(sql, "UPDATE dbo.Params SET Modified = @now, DataSize = {}, BinaryData = 0x{} WHERE FileName = N'siVersions' AND PartNo = 0;\nIF @@ROWCOUNT <> 1 THROW 57209, 'SI versions update failed', 1;", rewrite.new_bytes.len(), hex_upper(&rewrite.new_bytes)).unwrap();
        }
        sql
    }
}

pub(super) fn publish_deleted(generation: Uuid) -> String {
    format!(
        "-- preserve the pending-removal list beside the new generation\nIF EXISTS (SELECT 1 FROM dbo.Config WITH (UPDLOCK, HOLDLOCK) WHERE FileName = N'deleted_dynupdate_{generation}') THROW 57209, 'Deleted alias already exists', 1;\nINSERT dbo.Config (FileName, PartNo, Creation, Modified, Attributes, DataSize, BinaryData) SELECT N'deleted_dynupdate_{generation}', PartNo, @now, @now, Attributes, DataSize, BinaryData FROM dbo.ConfigSave WHERE FileName = N'deleted' AND PartNo = 0;\nIF @@ROWCOUNT <> 1 THROW 57209, 'Deleted alias copy failed', 1;\n"
    )
}

#[derive(Default)]
pub(super) struct Appends {
    pub guard_sql: String,
    pub rows: Vec<super::recovery::AppendedExistingRow>,
    pub registration_ids: Vec<String>,
}

/// Capture absent additions and their exact new keys, not just owner/file names. The transaction
/// asserts the complete touched registration/list preimage so concurrent writes cannot turn an
/// offline recovery recipe into deletion of someone else's entry.
pub(super) fn plan_appends(
    client: &dyn SqlClient,
    db: &str,
    files: &[sqlgen::AppendedFile],
    nodes: &[super::objects::RegistrationNode],
) -> Result<Appends> {
    if files.is_empty() {
        return Ok(Appends::default());
    }
    let mut unique = HashSet::new();
    for file in files {
        ensure!(
            file.object_hex.len() == 32
                && file.object_hex.bytes().all(|b| b.is_ascii_hexdigit())
                && file.file_name.is_ascii()
                && file.file_name.len() <= 128
                && unique.insert((
                    file.object_hex.to_ascii_uppercase(),
                    file.file_name.to_ascii_lowercase()
                )),
            "unmeasured or duplicate appended file"
        );
    }
    let objects = files
        .iter()
        .map(|file| format!("0x{}", file.object_hex))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT TOP ({}) CONVERT(varchar(32), _IDRRef, 2), CONVERT(varchar(32), _MDObjID, 2), CONVERT(varchar(8), _NodeTRef, 2), CONVERT(varchar(32), _NodeRRef, 2), ISNULL(CONVERT(varchar(32), _MessageNo), 'NULL') FROM {db}.dbo._ConfigChngR WHERE _MDObjID IN ({objects}) ORDER BY _IDRRef",
        MAX_ROWS + 1
    );
    let mut registrations = Vec::new();
    client.read_rows(&query, &[], &mut |row| {
        registrations.push((
            row.text(0)?.to_ascii_uppercase(),
            row.text(1)?.to_ascii_uppercase(),
            row.text(2)?.to_ascii_uppercase(),
            row.text(3)?.to_ascii_uppercase(),
            row.text(4)?.to_owned(),
        ));
        Ok(())
    })?;
    ensure!(
        registrations.len() <= MAX_ROWS,
        "registration recovery inventory is too large"
    );
    let mut ids = HashSet::new();
    for (id, object, node_type, node, message) in &registrations {
        ensure!(
            [id, object, node]
                .iter()
                .all(|value| value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit()))
                && node_type.len() == 8
                && node_type.bytes().all(|b| b.is_ascii_hexdigit())
                && ids.insert(id.clone()),
            "invalid registration identity"
        );
        ensure!(
            message == "NULL" || message.parse::<i64>().is_ok(),
            "invalid registration message number"
        );
    }
    // Absence is part of the preimage too: registrations::plan may have observed rows that
    // disappeared before this read. A later insertion must not create unrecorded appends.
    let mut guard = format!(
        "-- exact registration/file-list preimage for offline recovery\nIF (SELECT COUNT_BIG(*) FROM dbo._ConfigChngR WITH (UPDLOCK, HOLDLOCK) WHERE _MDObjID IN ({objects})) <> {} THROW 57318, 'Registration recovery preimage drifted', 1;\n",
        registrations.len()
    );
    if registrations.is_empty() {
        return Ok(Appends {
            guard_sql: guard,
            rows: Vec::new(),
            registration_ids: Vec::new(),
        });
    }
    let id_list = ids
        .iter()
        .map(|id| format!("0x{id}"))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT TOP ({}) CONVERT(varchar(32), _ConfigChngR_IDRRef, 2), CONVERT(varchar(8), _KeyField, 2), _FileName FROM {db}.dbo._ConfigChngR_ExtProps WHERE _ConfigChngR_IDRRef IN ({id_list}) ORDER BY _ConfigChngR_IDRRef, _KeyField",
        MAX_ROWS + 1
    );
    let mut listed = Vec::new();
    client.read_rows(&query, &[], &mut |row| {
        listed.push((
            row.text(0)?.to_ascii_uppercase(),
            row.text(1)?.to_ascii_uppercase(),
            row.text(2)?.to_owned(),
        ));
        Ok(())
    })?;
    ensure!(
        listed.len() <= MAX_ROWS,
        "file-list recovery inventory is too large"
    );
    let mut keys = HashSet::new();
    for (id, key, name) in &listed {
        ensure!(
            ids.contains(id)
                && key.len() == 8
                && keys.insert((id, key))
                && u32::from_str_radix(key, 16).is_ok_and(|key| key <= i32::MAX as u32)
                && name.encode_utf16().count() <= 128
                && !name.chars().any(char::is_control),
            "unmeasured file-list identity/key"
        );
    }
    for (id, object, node_type, node, message) in &registrations {
        let message = if message == "NULL" {
            "_MessageNo IS NULL".to_owned()
        } else {
            format!("_MessageNo = {message}")
        };
        writeln!(guard, "IF NOT EXISTS (SELECT 1 FROM dbo._ConfigChngR WITH (UPDLOCK, HOLDLOCK) WHERE _IDRRef = 0x{id} AND _MDObjID = 0x{object} AND _NodeTRef = 0x{node_type} AND _NodeRRef = 0x{node} AND {message}) THROW 57318, 'Registration recovery row drifted', 1;").unwrap();
    }
    writeln!(guard, "IF (SELECT COUNT_BIG(*) FROM dbo._ConfigChngR_ExtProps WITH (UPDLOCK, HOLDLOCK) WHERE _ConfigChngR_IDRRef IN ({id_list})) <> {} THROW 57318, 'File-list recovery preimage drifted', 1;", listed.len()).unwrap();
    for (id, key, name) in &listed {
        writeln!(guard, "IF NOT EXISTS (SELECT 1 FROM dbo._ConfigChngR_ExtProps WITH (UPDLOCK, HOLDLOCK) WHERE _ConfigChngR_IDRRef = 0x{id} AND _KeyField = 0x{key} AND _FileName = N'{}') THROW 57318, 'File-list recovery row drifted', 1;", quote_string(name)).unwrap();
    }
    let mut rows = Vec::new();
    let mut registration_ids = Vec::new();
    for (id, object, node_type, node, _) in &registrations {
        if !nodes.iter().any(|eligible| {
            eligible.type_ref.eq_ignore_ascii_case(node_type)
                && eligible.reference.eq_ignore_ascii_case(node)
        }) {
            continue;
        }
        registration_ids.push(id.clone());
        let old: Vec<_> = listed.iter().filter(|(rid, _, _)| rid == id).collect();
        let mut missing: Vec<_> = files
            .iter()
            .filter(|file| {
                file.object_hex.eq_ignore_ascii_case(object)
                    && !old
                        .iter()
                        .any(|(_, _, name)| name.eq_ignore_ascii_case(&file.file_name))
            })
            .map(|file| file.file_name.clone())
            .collect();
        missing.sort();
        let mut key = old
            .iter()
            .map(|(_, key, _)| i64::from(u32::from_str_radix(key, 16).expect("validated key")))
            .max()
            .unwrap_or(-1);
        for name in missing {
            key += 1;
            ensure!(key <= i64::from(i32::MAX), "file-list key overflow");
            rows.push(super::recovery::AppendedExistingRow {
                registration_id: id.clone(),
                key_hex: format!("{key:08X}"),
                file_name: name,
            });
        }
    }
    ensure!(
        rows.len() <= MAX_ROWS,
        "too many planned recovery additions"
    );
    Ok(Appends {
        guard_sql: guard,
        rows,
        registration_ids,
    })
}

/// Binding the exact node-table classification closes equal-count eligibility swaps after
/// registrations::plan. The full registration/list preimage remains guarded separately.
pub(super) fn node_guard(
    client: &dyn SqlClient,
    db: &str,
    expected: &[super::objects::RegistrationNode],
) -> Result<String> {
    let mut types = Vec::new();
    client.read_rows(&format!("SELECT DISTINCT TOP ({}) CONVERT(bigint, CONVERT(int, _NodeTRef)), CONVERT(varchar(8), _NodeTRef, 2) FROM {db}.dbo._ConfigChngR ORDER BY 1", MAX_ROWS + 1), &[], &mut |row| {
        types.push((row.i64(0)?, row.text(1)?.to_ascii_uppercase())); Ok(())
    })?;
    ensure!(types.len() <= MAX_ROWS, "node type inventory is too large");
    let mut images = Vec::new();
    for (plan, kind) in types {
        ensure!(
            plan > 0 && kind == format!("{plan:08X}"),
            "unmeasured node type"
        );
        let mut rows = Vec::new();
        client.read_rows(&format!("SELECT TOP ({}) CONVERT(varchar(32), _IDRRef, 2), CASE WHEN _PredefinedID = 0x00000000000000000000000000000000 THEN 0 ELSE 1 END, CONVERT(int, _Marked) FROM {db}.dbo._Node{plan} ORDER BY _IDRRef", MAX_ROWS + 1), &[], &mut |row| {
            rows.push((row.text(0)?.to_ascii_uppercase(), row.i64(1)?, row.i64(2)?)); Ok(())
        })?;
        ensure!(rows.len() <= MAX_ROWS, "node table inventory is too large");
        images.push((plan, kind, rows));
    }
    render_node_guard(&images, expected)
}

type NodeImage = (i64, String, Vec<(String, i64, i64)>);
fn render_node_guard(
    images: &[NodeImage],
    expected: &[super::objects::RegistrationNode],
) -> Result<String> {
    let wanted: BTreeSet<_> = expected
        .iter()
        .map(|n| {
            (
                n.plan,
                n.type_ref.to_ascii_uppercase(),
                n.reference.to_ascii_uppercase(),
            )
        })
        .collect();
    ensure!(wanted.len() == expected.len(), "duplicate eligible node");
    let mut actual = BTreeSet::new();
    let mut guard = format!(
        "-- exact dynamic append node eligibility\nIF (SELECT COUNT_BIG(*) FROM (SELECT DISTINCT _NodeTRef FROM dbo._ConfigChngR WITH (UPDLOCK, HOLDLOCK)) T) <> {} THROW 57318, 'Node type preimage drifted', 1;\n",
        images.len()
    );
    for (plan, kind, rows) in images {
        writeln!(guard, "IF NOT EXISTS (SELECT 1 FROM dbo._ConfigChngR WITH (UPDLOCK, HOLDLOCK) WHERE _NodeTRef = 0x{kind}) THROW 57318, 'Node type preimage drifted', 1;\nIF (SELECT COUNT_BIG(*) FROM dbo._Node{plan} WITH (UPDLOCK, HOLDLOCK)) <> {} THROW 57318, 'Node table preimage drifted', 1;", rows.len()).unwrap();
        let mut ids = HashSet::new();
        for (reference, own, marked) in rows {
            ensure!(
                reference.len() == 32
                    && reference.bytes().all(|b| b.is_ascii_hexdigit())
                    && ids.insert(reference)
                    && matches!(own, 0 | 1)
                    && matches!(marked, 0 | 1),
                "unmeasured node identity/classification"
            );
            if *own == 0 {
                ensure!(*marked == 0, "eligible node is marked for deletion");
                actual.insert((*plan, kind.clone(), reference.clone()));
            }
            let own_operator = if *own == 0 { "=" } else { "<>" };
            writeln!(guard, "IF NOT EXISTS (SELECT 1 FROM dbo._Node{plan} WITH (UPDLOCK, HOLDLOCK) WHERE _IDRRef = 0x{reference} AND _PredefinedID {own_operator} 0x00000000000000000000000000000000 AND CONVERT(int, _Marked) = {marked}) THROW 57318, 'Node eligibility preimage drifted', 1;").unwrap();
        }
    }
    ensure!(
        actual == wanted,
        "eligible node inventory changed after registration planning"
    );
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_model::brace::{Brace, serialize_row};
    use crate::sql::{Dbms, ScriptVariables, SqlParam, SqlRow, SqlValue};

    struct CannedLists {
        old: Vec<(String, String)>,
        registration_present: bool,
    }
    const REG: &str = "11AA1111111111111111111111111111";
    const OBJECT: &str = "22BB2222222222222222222222222222";
    fn eligible_node() -> super::super::objects::RegistrationNode {
        super::super::objects::RegistrationNode {
            plan: 1,
            type_ref: "00000001".to_owned(),
            reference: "33CC3333333333333333333333333333".to_owned(),
        }
    }
    impl SqlClient for CannedLists {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("read-only planner")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("read-only planner")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("read-only planner")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("read-only planner")
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            let values = if query.contains("_ConfigChngR_ExtProps") {
                self.old
                    .iter()
                    .map(|(key, file)| vec![REG.to_owned(), key.clone(), file.clone()])
                    .collect::<Vec<_>>()
            } else if self.registration_present {
                vec![vec![
                    REG.to_owned(),
                    OBJECT.to_owned(),
                    "00000001".to_owned(),
                    "33CC3333333333333333333333333333".to_owned(),
                    "NULL".to_owned(),
                ]]
            } else {
                Vec::new()
            };
            for row in values {
                each(SqlRow {
                    result_set: 0,
                    values: row.into_iter().map(SqlValue::Text).collect(),
                })?;
            }
            Ok(())
        }
    }

    #[test]
    fn recovery_records_only_missing_exact_keys_and_repeated_append_records_nothing() {
        let existing = "old-alias.0";
        let files = [existing, "new-alias.1", "new-alias.0"]
            .iter()
            .map(|name| sqlgen::AppendedFile {
                object_hex: OBJECT.to_owned(),
                file_name: (*name).to_owned(),
            })
            .collect::<Vec<_>>();
        let client = CannedLists {
            registration_present: true,
            old: vec![
                ("00000000".to_owned(), "body.0".to_owned()),
                ("00000001".to_owned(), existing.to_owned()),
            ],
        };
        let plan = plan_appends(&client, "[owned]", &files, &[eligible_node()]).unwrap();
        assert_eq!(
            plan.rows
                .iter()
                .map(|r| (&*r.registration_id, &*r.key_hex, &*r.file_name))
                .collect::<Vec<_>>(),
            vec![
                (REG, "00000002", "new-alias.0"),
                (REG, "00000003", "new-alias.1")
            ]
        );
        assert!(plan.guard_sql.contains("WITH (UPDLOCK, HOLDLOCK)"));
        assert_eq!(plan.registration_ids, [REG]);
        let ineligible = plan_appends(&client, "[owned]", &files, &[]).unwrap();
        assert!(ineligible.rows.is_empty() && ineligible.registration_ids.is_empty());
        // The ineligible rows are still part of the full preimage CAS.
        assert!(
            ineligible
                .guard_sql
                .contains("Registration recovery row drifted")
        );
        assert!(
            plan.guard_sql
                .contains("_KeyField = 0x00000001 AND _FileName = N'old-alias.0'")
        );
        let repeated = CannedLists {
            registration_present: true,
            old: client
                .old
                .into_iter()
                .chain(
                    plan.rows
                        .iter()
                        .map(|r| (r.key_hex.clone(), r.file_name.clone())),
                )
                .collect(),
        };
        assert!(
            plan_appends(&repeated, "[owned]", &files, &[eligible_node()])
                .unwrap()
                .rows
                .is_empty()
        );
        let overflow = CannedLists {
            registration_present: true,
            old: vec![("7FFFFFFF".to_owned(), existing.to_owned())],
        };
        assert!(plan_appends(&overflow, "[owned]", &files, &[eligible_node()]).is_err());
        let negative = CannedLists {
            registration_present: true,
            old: vec![("80000000".to_owned(), existing.to_owned())],
        };
        assert!(plan_appends(&negative, "[owned]", &files, &[eligible_node()]).is_err());
        assert!(
            plan_appends(
                &repeated,
                "[owned]",
                &[files[0].clone(), files[0].clone()],
                &[eligible_node()]
            )
            .is_err()
        );
    }

    #[test]
    fn empty_registration_preimage_is_locked_even_if_prior_plan_observed_the_owner() {
        // Model the owner disappearing between registrations::plan and plan_appends. Its
        // planned dropped_files still exist, but no recovery entry may be invented or omitted
        // if another writer subsequently recreates a row before publication.
        let prior = super::super::registrations::RegistrationPlan {
            dropped_files: vec![sqlgen::AppendedFile {
                object_hex: OBJECT.to_owned(),
                file_name: "old-alias.0".to_owned(),
            }],
            ..Default::default()
        };
        let empty = CannedLists {
            old: Vec::new(),
            registration_present: false,
        };
        let plan =
            plan_appends(&empty, "[owned]", &prior.dropped_files, &[eligible_node()]).unwrap();
        assert!(plan.rows.is_empty());
        assert!(plan.guard_sql.contains(&format!(
            "FROM dbo._ConfigChngR WITH (UPDLOCK, HOLDLOCK) WHERE _MDObjID IN (0x{OBJECT})) <> 0 THROW 57318"
        )));
        assert!(!plan.guard_sql.contains("_ConfigChngR_ExtProps"));
        assert!(
            plan_appends(&empty, "[owned]", &[], &[])
                .unwrap()
                .guard_sql
                .is_empty()
        );
    }

    #[test]
    fn node_eligibility_binds_exact_membership_even_when_count_is_unchanged() {
        let node = eligible_node();
        let own = "44DD4444444444444444444444444444".to_owned();
        let image = (
            1,
            node.type_ref.clone(),
            vec![(node.reference.clone(), 0, 0), (own.clone(), 1, 0)],
        );
        let sql =
            render_node_guard(std::slice::from_ref(&image), std::slice::from_ref(&node)).unwrap();
        assert!(sql.contains("FROM dbo._Node1 WITH (UPDLOCK, HOLDLOCK)) <> 2"));
        assert!(sql.contains(&format!(
            "_IDRRef = 0x{} AND _PredefinedID =",
            node.reference
        )));
        assert!(sql.contains(&format!("_IDRRef = 0x{own} AND _PredefinedID <>")));
        let mut swap = image.clone();
        swap.2[0].0 = "55EE5555555555555555555555555555".to_owned();
        assert!(render_node_guard(&[swap], std::slice::from_ref(&node)).is_err());
        let mut marked = image.clone();
        marked.2[0].2 = 1;
        assert!(render_node_guard(&[marked], std::slice::from_ref(&node)).is_err());
        assert!(render_node_guard(&[image], &[]).is_err());
    }

    #[test]
    fn actual_native_si_versions_is_raw_bom_text_and_deflate_is_refused() {
        let raw = include_bytes!("../../tests/fixtures/dynamic-overlay/siVersions.native.txt");
        assert!(raw.starts_with(b"\xef\xbb\xbf{0,16,"));
        assert_eq!(version_names(raw).unwrap().len(), 16);
        assert!(version_names(&versions::deflate_row(raw).unwrap()).is_err());
        let rewritten =
            si::set_si_version(raw, &format!("{}.si", SERVICE_CLASSES[0]), Uuid::new_v4()).unwrap();
        assert!(rewritten.starts_with(b"\xef\xbb\xbf{0,16,"));
        assert_eq!(rewritten.len(), raw.len());
        assert_eq!(
            version_names(&rewritten).unwrap(),
            version_names(raw).unwrap()
        );
    }

    fn meta(name: &str) -> RowMeta {
        RowMeta {
            name: name.to_owned(),
            part: 0,
            data_size: 1,
            byte_len: 1,
            attributes: 0,
            creation: "2026-01-01 00:00:00.000".to_owned(),
            modified: "2026-01-01 00:00:00.000".to_owned(),
            sha256: hex_lower(&Sha256::digest(b"x")),
        }
    }

    #[test]
    fn stage_header_guard_binds_flags_and_dates_even_when_bytes_are_unchanged() {
        let original = meta("deleted");
        let sql = guard("ConfigSave", "1=1", std::slice::from_ref(&original));
        assert!(sql.contains("dbo.ConfigSave WITH (UPDLOCK, HOLDLOCK) WHERE 1=1"));
        assert!(sql.contains("Attributes = 0 AND CONVERT(varchar(27), Creation, 121) = '2026-01-01 00:00:00.000' AND CONVERT(varchar(27), Modified, 121) = '2026-01-01 00:00:00.000'"));
        assert!(sql.contains("DataSize) = 1 AND DATALENGTH(BinaryData) = 1"));
        assert!(sql.contains(&format!(
            "HASHBYTES('SHA2_256', BinaryData) = 0x{}",
            original.sha256
        )));
        assert!(sql.contains("THROW 57209, 'Pending ConfigSave row drifted'"));
        for change in ["flags", "creation", "modified"] {
            let mut drifted = original.clone();
            match change {
                "flags" => drifted.attributes = 1,
                "creation" => drifted.creation = "2026-01-02 00:00:00.000".to_owned(),
                _ => drifted.modified = "2026-01-02 00:00:00.000".to_owned(),
            }
            assert_eq!(drifted.sha256, original.sha256);
            assert_ne!(sql, guard("ConfigSave", "1=1", &[drifted]), "{change}");
        }
    }

    #[test]
    fn collection_requires_every_later_set_and_refuses_partial_legacy_set() {
        let g0 = Uuid::new_v4().to_string();
        let g1 = Uuid::new_v4().to_string();
        let history = vec![g0.clone(), g1.clone()];
        let mut rows: Vec<_> = service_names()
            .into_iter()
            .map(|name| meta(&name))
            .collect();
        rows.push(meta("siVersions"));
        // A single initial legacy generation is the measured exception, not any missing set.
        assert!(validate_service_rows(&rows, std::slice::from_ref(&g0)).is_ok());
        assert!(validate_service_rows(&rows, &history).is_err());
        let missing_later = rows.clone();
        for id in SERVICE_CLASSES {
            rows.push(meta(&format!("{id}_dynupdate_{g1}.si")));
        }
        assert_eq!(validate_service_rows(&rows, &history).unwrap().len(), 16);
        let mut partial = rows.clone();
        partial.pop();
        assert!(validate_service_rows(&partial, &history).is_err());
        let mut partial_g0 = rows.clone();
        partial_g0.push(meta(&format!("{}_dynupdate_{g0}.si", SERVICE_CLASSES[0])));
        assert!(validate_service_rows(&partial_g0, &history).is_err());
        assert!(
            validate_service_rows(&rows, &[g1, g0]).is_err(),
            "marker order cannot move the exception"
        );
        let mut stale = rows.clone();
        stale.last_mut().unwrap().sha256 = "0".repeat(64);
        assert!(validate_service_rows(&stale, &history).is_err());
        let mut unknown = missing_later;
        unknown.push(meta("unknown.si"));
        assert!(validate_service_rows(&unknown, &history).is_err());
    }

    #[test]
    fn deleted_inventory_requires_exact_history_bound_names_and_flags() {
        let id = SERVICE_CLASSES[0];
        let g0 = Uuid::new_v4().to_string();
        let g1 = Uuid::new_v4().to_string();
        let history = vec![g0.clone(), g1.clone()];
        let kinds = Owners::from(HashMap::from([(id.to_owned(), "CommonForm")]));
        let names = [
            format!("{id}_dynupdate_{g0}.0"),
            format!("versions_dynupdate_{g0}"),
            format!("{id}_dynupdate_{g1}.0"),
            format!("versions_dynupdate_{g1}"),
            format!("deleted_dynupdate_{g1}"),
            "DynamicallyUpdated".to_owned(),
        ];
        let rows: Vec<_> = names.iter().map(|name| meta(name)).collect();
        let list = |names: &[String]| {
            format!(
                "{},{}",
                names.len(),
                names
                    .iter()
                    .map(|name| format!("\"{name}\",0"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        assert_eq!(
            judge_deleted(list(&names).as_bytes(), &rows, &history, &kinds)
                .unwrap()
                .len(),
            names.len()
        );
        assert!(judge_deleted(list(&names[..5]).as_bytes(), &rows, &history, &kinds).is_err());
        let mut duplicate = names.to_vec();
        duplicate[0] = duplicate[1].clone();
        assert!(judge_deleted(list(&duplicate).as_bytes(), &rows, &history, &kinds).is_err());
        let bad_flag = list(&names).replacen(",0", ",1", 1);
        assert!(judge_deleted(bad_flag.as_bytes(), &rows, &history, &kinds).is_err());
        assert!(judge_deleted(list(&names).as_bytes(), &rows, &[g1, g0], &kinds).is_err());
        assert!(
            judge_deleted(list(&names).as_bytes(), &rows, &history, &Owners::default()).is_err()
        );
    }

    #[test]
    fn semantic_blob_must_match_first_inventory_and_uppercase_si_cannot_panic() {
        let row = meta("siVersions");
        assert!(bound_blob(&row, b"x").is_ok());
        assert!(bound_blob(&row, b"y").is_err()); // same-size ABA payload
        let ordinary = SERVICE_CLASSES
            .iter()
            .map(|id| meta(&format!("{id}.SI")))
            .collect();
        let collection = Collection {
            ordinary,
            guard_sql: String::new(),
            rewrites: Vec::new(),
            preimages: Vec::new(),
        };
        let sql = collection.render(Uuid::new_v4());
        assert_eq!(sql.matches("INSERT dbo.Params").count(), 16);
        assert!(sql.contains(".SI' AND PartNo = 0"));
        assert!(sql.contains(".si'"));
    }

    #[test]
    fn si_versions_rejects_duplicate_unknown_count_and_invalid_version() {
        let mut fields = vec![Brace::num(0), Brace::num(16)];
        for name in service_names() {
            fields.push(Brace::str(name));
            fields.push(Brace::atom(Uuid::new_v4()));
        }
        assert_eq!(
            version_names(&serialize_row(&Brace::list(fields.clone())))
                .unwrap()
                .len(),
            16
        );
        for (position, replacement) in [
            (2, fields[4].clone()),
            (2, Brace::str("unknown.si")),
            (1, Brace::num(15)),
            (3, Brace::atom("invalid")),
            (3, Brace::atom(Uuid::nil())),
        ] {
            let mut invalid = fields.clone();
            invalid[position] = replacement;
            assert!(version_names(&serialize_row(&Brace::list(invalid))).is_err());
        }
    }

    #[test]
    fn alias_requires_exact_known_generation_and_body_suffix() {
        let generation = Uuid::new_v4().to_string();
        let history = vec![generation.clone()];
        let id = SERVICE_CLASSES[0];
        assert_eq!(
            alias(&format!("{id}_dynupdate_{generation}.si"), &history)
                .unwrap()
                .0,
            format!("{id}.si")
        );
        assert!(alias(&format!("{id}_dynupdate_{}.si", Uuid::new_v4()), &history).is_err());
        assert!(alias(&format!("{id}_dynupdate_short"), &history).is_err());
    }
}
