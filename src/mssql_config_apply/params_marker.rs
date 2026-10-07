//! Bound descriptor facts and the partial 8.3.27.2214 Params-marker correction.
//! Native S/R5 retain the marker when the staged descriptor is decoded-equal;
//! genuine Comment/Synonym changes still collect it. The remaining #418 matrix
//! is open; no row-count boundary or wider service-information parity is claimed.

use crate::compiler::families::native::descriptor_own_uuid;
use crate::sql::SqlClient;
use anyhow::{Result, ensure};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[cfg(test)]
use super::dynamic_metadata;
use super::dynamic_overlay;
use super::model::RowMeta;
use super::model::{RowName, classify_name, quote_string};
use super::versions::{DynamicHistory, VersionsRow};
use crate::mssql_platform_profile::MssqlNativePlatformProfile;

fn canonical_descriptor(name: &str) -> Result<Uuid> {
    let id = Uuid::parse_str(name)?;
    ensure!(
        !id.is_nil() && id.hyphenated().to_string() == name,
        "noncanonical descriptor identity"
    );
    Ok(id)
}

/// Validate a complete DEFLATE stream in fixed scratch space, then allocate
/// exactly its measured decoded length. No format-independent size ceiling or
/// aggregate stage budget is added to the ordinary storage path.
fn inflate_complete(bytes: &[u8]) -> Result<Vec<u8>> {
    fn pass(bytes: &[u8], mut emit: impl FnMut(&[u8]) -> Result<()>) -> Result<usize> {
        let mut decoder = flate2::Decompress::new(false);
        let mut scratch = [0u8; 8192];
        let mut length = 0usize;
        loop {
            let before_in = decoder.total_in();
            let before_out = decoder.total_out();
            let offset = usize::try_from(before_in)?;
            let status = decoder.decompress(
                &bytes[offset..],
                &mut scratch,
                flate2::FlushDecompress::None,
            )?;
            let produced = usize::try_from(decoder.total_out() - before_out)?;
            length = length
                .checked_add(produced)
                .ok_or_else(|| anyhow::anyhow!("decoded length overflow"))?;
            emit(&scratch[..produced])?;
            if status == flate2::Status::StreamEnd {
                ensure!(
                    decoder.total_in() == bytes.len() as u64,
                    "descriptor has trailing compressed data"
                );
                return Ok(length);
            }
            ensure!(
                decoder.total_in() != before_in || produced != 0,
                "descriptor has incomplete DEFLATE stream"
            );
        }
    }
    let length = pass(bytes, |_| Ok(()))?;
    let mut plain = Vec::new();
    plain.try_reserve_exact(length)?;
    let measured = pass(bytes, |chunk| {
        ensure!(
            chunk.len() <= length - plain.len(),
            "descriptor decoded length changed"
        );
        plain.extend_from_slice(chunk);
        Ok(())
    })?;
    ensure!(
        measured == length && plain.len() == length,
        "descriptor decoded length changed"
    );
    Ok(plain)
}

/// Immutable judged bytes with the complete inventoried physical header.
pub(super) struct Descriptor {
    id: Uuid,
    plain: Vec<u8>,
}
impl Descriptor {
    fn validate_meta(id: &str, meta: &RowMeta) -> Result<()> {
        canonical_descriptor(id)?;
        let name = meta.name.to_ascii_lowercase();
        if name != id {
            let generation = name
                .strip_prefix(&format!("{id}_dynupdate_"))
                .ok_or_else(|| anyhow::anyhow!("descriptor physical name differs"))?;
            canonical_descriptor(generation)?;
        }
        ensure!(
            meta.part == 0
                && meta.attributes == 0
                && meta.data_size == meta.byte_len
                && meta.byte_len >= 0,
            "unmeasured descriptor header"
        );
        ensure!(
            meta.sha256.len() == 64 && meta.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid descriptor digest"
        );
        Ok(())
    }
    fn capture(id: &str, meta: &RowMeta, bytes: &[u8]) -> Result<Self> {
        Self::validate_meta(id, meta)?;
        let id = canonical_descriptor(id)?;
        dynamic_overlay::bound_blob(meta, bytes)?;
        let plain = inflate_complete(bytes)?;
        let own_id = descriptor_own_uuid(&plain)?
            .ok_or_else(|| anyhow::anyhow!("descriptor has no own header"))?;
        ensure!(
            canonical_descriptor(&own_id.to_ascii_lowercase())? == id,
            "descriptor own header differs from requested identity"
        );
        Ok(Self { id, plain })
    }
    /// Server-side full header/digest/length filter precedes transfer. Exactly
    /// one returned blob must still match the first inventory before decoding.
    pub(super) fn read(
        client: &dyn SqlClient,
        db: &str,
        table: &str,
        id: &str,
        meta: &RowMeta,
    ) -> Result<Self> {
        ensure!(
            matches!(table, "Config" | "ConfigSave"),
            "invalid descriptor storage table"
        );
        Self::validate_meta(id, meta)?;
        let mut bytes = None;
        client.read_rows(
            &format!(
                "SELECT TOP (2) BinaryData FROM {db}.dbo.{table} WHERE {}",
                predicate(meta)
            ),
            &[],
            &mut |mut row| {
                ensure!(bytes.is_none(), "duplicate descriptor capture");
                bytes = Some(row.take_binary(0)?);
                Ok(())
            },
        )?;
        Self::capture(
            id,
            meta,
            &bytes.ok_or_else(|| anyhow::anyhow!("descriptor changed after inventory"))?,
        )
    }
    pub(super) fn same_content(&self, other: &Self) -> Result<bool> {
        ensure!(self.id == other.id, "unrelated descriptor identities");
        Ok(self.plain == other.plain)
    }
}
fn predicate(meta: &RowMeta) -> String {
    format!(
        "FileName = N'{}' AND PartNo = {} AND CONVERT(bigint, DataSize) = {} AND DATALENGTH(BinaryData) = {} AND Attributes = {} AND CONVERT(varchar(27), Creation, 121) = '{}' AND CONVERT(varchar(27), Modified, 121) = '{}' AND HASHBYTES('SHA2_256', BinaryData) = 0x{}",
        quote_string(&meta.name),
        meta.part,
        meta.data_size,
        meta.byte_len,
        meta.attributes,
        quote_string(&meta.creation),
        quote_string(&meta.modified),
        meta.sha256
    )
}

/// Validate alias closure instead of trusting arbitrary history and ignoring
/// orphan rows. Caller binds the marker bytes that produced this history.
fn validate_inventory(rows: &[RowMeta], history: &[Uuid]) -> Result<()> {
    ensure!(
        history.iter().collect::<HashSet<_>>().len() == history.len()
            && history.iter().all(|id| !id.is_nil()),
        "invalid descriptor history"
    );
    let generations = history
        .iter()
        .map(|id| id.hyphenated().to_string())
        .collect::<Vec<_>>();
    let mut keys = HashSet::new();
    for row in rows {
        ensure!(
            row.part == 0
                && row.attributes == 0
                && row.data_size == row.byte_len
                && row.byte_len >= 0
                && keys.insert(row.key()),
            "duplicate or unmeasured alias row"
        );
        let ordinary = dynamic_overlay::ordinary_alias_name(&row.name, &generations)?;
        ensure!(
            matches!(
                classify_name(&ordinary),
                RowName::Descriptor(_) | RowName::Body { .. } | RowName::Service("versions")
            ) || ordinary == "deleted",
            "unknown pending alias"
        );
    }
    for generation in generations {
        ensure!(
            rows.iter().any(|row| row
                .name
                .eq_ignore_ascii_case(&format!("versions_dynupdate_{generation}"))),
            "history lacks versions alias"
        );
    }
    Ok(())
}

/// Latest descriptor alias from the *ordered, validated* publication history.
/// The ordinary physical row is only the fallback. A stage reverting to that
/// physical row can still change the descriptor published by a pending alias.
#[cfg(test)]
fn effective_descriptor<'a>(
    ordinary: Option<&'a RowMeta>,
    descriptor: &str,
    aliases: &'a [RowMeta],
    history: &[Uuid],
) -> Result<Option<&'a RowMeta>> {
    DescriptorHistory::new(aliases, history)?.effective(ordinary, descriptor)
}

/// Validate history once and index physical aliases, rather than rescanning
/// the complete inventory for every ordinary staged descriptor.
struct DescriptorHistory<'a> {
    aliases: HashMap<String, &'a RowMeta>,
    generations: Vec<Uuid>,
}
impl<'a> DescriptorHistory<'a> {
    fn new(aliases: &'a [RowMeta], history: &[Uuid]) -> Result<Self> {
        validate_inventory(aliases, history)?;
        Ok(Self {
            aliases: aliases
                .iter()
                .map(|row| (row.name.to_ascii_lowercase(), row))
                .collect(),
            generations: history.to_vec(),
        })
    }
    fn effective(
        &self,
        ordinary: Option<&'a RowMeta>,
        descriptor: &str,
    ) -> Result<Option<&'a RowMeta>> {
        canonical_descriptor(descriptor)?;
        for generation in self.generations.iter().rev() {
            let name = format!("{descriptor}_dynupdate_{}", generation.hyphenated());
            if let Some(row) = self.aliases.get(&name) {
                return Ok(Some(*row));
            }
        }
        if let Some(row) = ordinary {
            ensure!(
                row.name.eq_ignore_ascii_case(descriptor) && row.part == 0,
                "ordinary descriptor identity differs"
            );
        }
        Ok(ordinary)
    }
}

pub(super) struct PlanInput<'a> {
    pub client: &'a dyn SqlClient,
    pub db: &'a str,
    pub profile: MssqlNativePlatformProfile,
    pub staged: &'a [RowMeta],
    pub replaced: &'a [RowMeta],
    pub special: &'a [RowMeta],
    pub active_versions: &'a VersionsRow,
    pub history: &'a DynamicHistory,
    pub config_marker: Option<&'a [u8]>,
    pub params_marker: Option<&'a [u8]>,
}

pub(super) struct Decision {
    pub clear: bool,
    pub guard_sql: String,
}

fn storage_header(meta: &RowMeta) -> Result<()> {
    ensure!(
        meta.part == 0
            && meta.attributes == 0
            && meta.data_size == meta.byte_len
            && meta.byte_len >= 0
            && meta.sha256.len() == 64
            && meta.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "unsupported marker-decision storage header"
    );
    Ok(())
}

fn read_storage_value(client: &dyn SqlClient, db: &str, meta: &RowMeta) -> Result<Vec<u8>> {
    storage_header(meta)?;
    let mut value = None;
    client.read_rows(
        &format!(
            "SELECT TOP (2) BinaryData FROM {db}.dbo.Config WHERE {}",
            predicate(meta)
        ),
        &[],
        &mut |mut row| {
            ensure!(value.is_none(), "duplicate bound storage value");
            value = Some(row.take_binary(0)?);
            Ok(())
        },
    )?;
    let value = value.ok_or_else(|| anyhow::anyhow!("storage value changed after inventory"))?;
    dynamic_overlay::bound_blob(meta, &value)?;
    Ok(value)
}

/// All deciding reads are covered by exact transaction preimages. Unknown
/// streams/identities/multipart layouts refuse rather than guess collection.
/// Other platform profiles retain their prior policy pending their own oracle.
pub(super) fn plan(input: PlanInput<'_>) -> Result<Decision> {
    let descriptors = input
        .staged
        .iter()
        .filter(|row| matches!(classify_name(&row.name), RowName::Descriptor(_)))
        .collect::<Vec<_>>();
    let prior_clear = !descriptors.is_empty();
    if input.profile != MssqlNativePlatformProfile::Platform8_3_27_2214
        || input.params_marker.is_none()
        || descriptors.is_empty()
    {
        return Ok(Decision {
            clear: prior_clear,
            guard_sql: String::new(),
        });
    }

    // The original planner parsed these marker bytes after its inventory.
    // Bind them to that SAME inventory, so an intermediate change/reversion
    // cannot make a different history choose an alias behind an older guard.
    let marker_metas = input
        .special
        .iter()
        .filter(|row| row.name.eq_ignore_ascii_case("DynamicallyUpdated"))
        .collect::<Vec<_>>();
    match input.config_marker {
        Some(bytes) => {
            ensure!(marker_metas.len() == 1, "Config marker inventory differs");
            storage_header(marker_metas[0])?;
            dynamic_overlay::bound_blob(marker_metas[0], bytes)?;
        }
        None => ensure!(
            marker_metas.is_empty(),
            "Config marker appeared during planning"
        ),
    }
    let params_metas = super::read_row_metas(
        input.client,
        &format!(
            "SELECT TOP (2) {} FROM {}.dbo.Params WHERE FileName = N'DynamicallyUpdated' ORDER BY PartNo",
            super::ROW_COLUMNS,
            input.db
        ),
    )?;
    ensure!(params_metas.len() == 1, "Params marker inventory differs");
    storage_header(&params_metas[0])?;
    dynamic_overlay::bound_blob(
        &params_metas[0],
        input.params_marker.expect("present above"),
    )?;
    ensure!(
        super::versions::parse_dynamic_history(input.config_marker, input.params_marker)?
            == *input.history,
        "marker history changed while planning"
    );

    let aliases = input
        .special
        .iter()
        .filter(|row| !row.name.eq_ignore_ascii_case("DynamicallyUpdated"))
        .cloned()
        .collect::<Vec<_>>();
    let descriptor_history = DescriptorHistory::new(&aliases, &input.history.generations)?;
    let descriptor_names = descriptors
        .iter()
        .map(|row| row.name.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    ensure!(
        descriptor_names.len() == descriptors.len(),
        "duplicate or multipart staged descriptor"
    );
    let ordinary = input
        .replaced
        .iter()
        .filter(|row| descriptor_names.contains(&row.name.to_ascii_lowercase()))
        .cloned()
        .collect::<Vec<_>>();
    let mut ordinary_by_name = HashMap::new();
    for row in &ordinary {
        ensure!(
            row.part == 0
                && ordinary_by_name
                    .insert(row.name.to_ascii_lowercase(), row)
                    .is_none(),
            "duplicate or multipart ordinary descriptor"
        );
    }
    let mut pairs = Vec::new();
    for staged in descriptors {
        let name = staged.name.to_ascii_lowercase();
        let base = ordinary_by_name.get(&name).copied();
        let active = descriptor_history.effective(base, &name)?;
        pairs.push((staged, active));
    }

    // Validate all deciding headers before transfer; do not cap the stage's
    // row count or aggregate bytes. Decoded bodies live only for this pair.
    for (staged, active) in &pairs {
        for meta in std::iter::once(*staged).chain(active.iter().copied()) {
            Descriptor::validate_meta(&staged.name.to_ascii_lowercase(), meta)?;
        }
    }
    let mut changed = false;
    for (staged, active) in pairs {
        let id = staged.name.to_ascii_lowercase();
        let after = Descriptor::read(input.client, input.db, "ConfigSave", &id, staged)?;
        if let Some(active) = active {
            let before = Descriptor::read(input.client, input.db, "Config", &id, active)?;
            changed |= !before.same_content(&after)?;
        } else {
            changed = true; // Creation remains governed by the existing structural gate.
        }
    }

    // Ordinary version-map bytes decide which generations are current. Keep
    // their complete header/digest under the same transaction guard as aliases.
    let version_metas = super::read_row_metas(
        input.client,
        &format!(
            "SELECT TOP (2) {} FROM {}.dbo.Config WHERE FileName = N'versions' ORDER BY PartNo",
            super::ROW_COLUMNS,
            input.db
        ),
    )?;
    ensure!(
        version_metas.len() == 1,
        "ordinary versions inventory differs"
    );
    let version_bytes = read_storage_value(input.client, input.db, &version_metas[0])?;
    let version_plain = inflate_complete(&version_bytes)?;
    ensure!(
        super::versions::parse_versions_plain(&version_plain)? == *input.active_versions,
        "ordinary version map changed while planning"
    );

    let mut guard_sql = String::from("-- Params marker semantic decision preimages\n");
    guard_sql.push_str(&dynamic_overlay::guard("ConfigSave", "1=1", input.staged));
    guard_sql.push_str(&dynamic_overlay::guard(
        "Config",
        dynamic_overlay::CONFIG_FILTER,
        input.special,
    ));
    guard_sql.push_str(&dynamic_overlay::guard(
        "Config",
        "EXISTS (SELECT 1 FROM dbo.ConfigSave staged WHERE staged.FileName = dbo.Config.FileName)",
        input.replaced,
    ));
    guard_sql.push_str(&dynamic_overlay::guard(
        "Config",
        "FileName = N'versions'",
        &version_metas,
    ));
    guard_sql.push_str(&dynamic_overlay::guard(
        "Params",
        "FileName = N'DynamicallyUpdated'",
        &params_metas,
    ));
    Ok(Decision {
        clear: changed,
        guard_sql,
    })
}

#[cfg(test)]
mod tests {
    use super::super::versions;
    use super::*;
    use crate::sql::{Dbms, ScriptVariables, SqlParam, SqlRow, SqlValue};
    use sha2::{Digest, Sha256};

    const ID: &str = "16b3681c-426d-4d6f-9ffe-588a23974222";
    fn same_descriptor_content(active: &[u8], staged: &[u8]) -> Result<bool> {
        Descriptor::capture(ID, &blob_meta(ID, active), active)?.same_content(&Descriptor::capture(
            ID,
            &blob_meta(ID, staged),
            staged,
        )?)
    }
    fn blob_meta(name: &str, bytes: &[u8]) -> RowMeta {
        let mut header = meta(name);
        header.data_size = bytes.len() as i64;
        header.byte_len = bytes.len() as i64;
        header.sha256 = super::super::model::hex_lower(&Sha256::digest(bytes));
        header
    }

    const OLD: &[u8] =
        include_bytes!("../../docs/apply/evidence/params-marker/f4-descriptor-before.deflate");
    const STAGED: &[u8] =
        include_bytes!("../../docs/apply/evidence/params-marker/f4-descriptor-staged.deflate");

    #[test]
    fn actual_f4_recompressed_descriptor_is_not_a_metadata_change() {
        assert_ne!(OLD, STAGED);
        assert_eq!((OLD.len(), STAGED.len()), (173, 174));
        assert!(same_descriptor_content(OLD, STAGED).unwrap());
        assert_eq!(dynamic_metadata::inflate(OLD).unwrap().len(), 289);
    }

    #[test]
    fn identical_stored_descriptor_is_equal_but_damaged_streams_are_refused() {
        assert!(same_descriptor_content(OLD, OLD).unwrap());
        for length in 0..OLD.len() {
            assert!(same_descriptor_content(&OLD[..length], STAGED).is_err());
        }
        let mut trailing = OLD.to_vec();
        trailing.push(0);
        assert!(same_descriptor_content(&trailing, STAGED).is_err());
    }

    #[test]
    fn genuine_decoded_change_is_distinct_without_normalization() {
        let mut plain = dynamic_metadata::inflate(OLD).unwrap();
        let index = plain.iter().position(|byte| *byte == b'"').unwrap() + 1;
        plain.insert(index, b'X');
        let changed = versions::deflate_row(&plain).unwrap();
        assert!(!same_descriptor_content(OLD, &changed).unwrap());
    }

    fn meta(name: &str) -> RowMeta {
        RowMeta {
            name: name.to_owned(),
            part: 0,
            data_size: 173,
            byte_len: 173,
            attributes: 0,
            creation: "4026-09-30 08:41:57.000".to_owned(),
            modified: "4026-09-30 08:41:57.000".to_owned(),
            sha256: "a".repeat(64),
        }
    }

    #[test]
    fn descriptor_uses_latest_effective_alias_not_equal_physical_base() {
        let id = "16b3681c-426d-4d6f-9ffe-588a23974222";
        let g1 = Uuid::parse_str("06cb0442-0c47-4fad-986a-f08f28287c1b").unwrap();
        let g2 = Uuid::parse_str("bd4ea842-aee4-41ee-94cc-ede196546578").unwrap();
        let ordinary = meta(id);
        let older = meta(&format!("{id}_dynupdate_{g1}"));
        let newer = meta(&format!("{id}_dynupdate_{g2}"));
        let aliases = vec![
            newer.clone(),
            older,
            meta(&format!("versions_dynupdate_{g1}")),
            meta(&format!("versions_dynupdate_{g2}")),
        ];
        assert_eq!(
            effective_descriptor(Some(&ordinary), id, &aliases, &[g1, g2]).unwrap(),
            Some(&aliases[0])
        );
        assert_eq!(
            effective_descriptor(Some(&ordinary), id, &aliases, &[g1]).is_err(),
            true
        );
        assert_eq!(
            effective_descriptor(Some(&ordinary), id, &[], &[]).unwrap(),
            Some(&ordinary)
        );
        let mut malformed = newer.clone();
        malformed.part = 1;
        assert!(effective_descriptor(Some(&ordinary), id, &[malformed], &[g2]).is_err());
        assert!(effective_descriptor(Some(&ordinary), id, &[newer.clone(), newer], &[g2]).is_err());
    }

    #[test]
    fn foreign_uuid_header_drift_and_unsupported_flags_refuse() {
        let foreign = "26b3681c-426d-4d6f-9ffe-588a23974222";
        assert!(Descriptor::capture(foreign, &blob_meta(foreign, OLD), OLD,).is_err());
        let header = blob_meta(ID, OLD);
        assert!(Descriptor::capture(ID, &header, STAGED).is_err());
        let mut flags = header;
        flags.attributes = 1;
        assert!(Descriptor::capture(ID, &flags, OLD).is_err());
    }

    #[test]
    fn orphan_unknown_missing_versions_and_duplicate_history_refuse() {
        let generation = Uuid::parse_str("06cb0442-0c47-4fad-986a-f08f28287c1b").unwrap();
        let aliases = vec![
            meta(&format!("{ID}_dynupdate_{generation}")),
            meta(&format!("versions_dynupdate_{generation}")),
        ];
        assert!(
            effective_descriptor(None, ID, &aliases, &[generation])
                .unwrap()
                .is_some()
        );
        assert!(effective_descriptor(None, ID, &aliases[..1], &[generation]).is_err());
        assert!(effective_descriptor(None, ID, &aliases, &[generation, generation]).is_err());
        assert!(effective_descriptor(None, ID, &aliases, &[]).is_err());
        let mut unknown = aliases;
        unknown[0].name = format!("opaque_dynupdate_{generation}");
        assert!(effective_descriptor(None, ID, &unknown, &[generation]).is_err());
    }

    struct CaptureClient {
        rows: Vec<Vec<u8>>,
        expected: RowMeta,
    }

    /// A read-only fixture of the storage API, including server-side header
    /// predicates. It never executes the generated transaction.
    struct PlanClient {
        storage: Vec<(&'static str, RowMeta, Vec<u8>)>,
    }
    impl SqlClient for PlanClient {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("read-only")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("read-only")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("read-only")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("read-only")
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            assert!(query.starts_with("SELECT TOP (2) "));
            let table = ["ConfigSave", "Config", "Params"]
                .into_iter()
                .find(|table| query.contains(&format!("FROM [owned].dbo.{table} WHERE ")))
                .expect("only the planner's storage reads are allowed");
            let binary = query.starts_with("SELECT TOP (2) BinaryData ");
            let mut matched = false;
            for (stored_table, meta, bytes) in &self.storage {
                if *stored_table != table
                    || !query.contains(&format!("FileName = N'{}'", meta.name))
                {
                    continue;
                }
                matched = true;
                let values = if binary {
                    assert!(
                        query.contains(&predicate(meta)),
                        "all physical fields must precede transfer"
                    );
                    vec![SqlValue::Binary(bytes.clone())]
                } else {
                    vec![
                        SqlValue::Text(meta.name.clone()),
                        SqlValue::Int(i64::from(meta.part)),
                        SqlValue::Int(meta.data_size),
                        SqlValue::Int(meta.byte_len),
                        SqlValue::Int(i64::from(meta.attributes)),
                        SqlValue::Text(meta.creation.clone()),
                        SqlValue::Text(meta.modified.clone()),
                        SqlValue::Text(meta.sha256.clone()),
                    ]
                };
                each(SqlRow {
                    result_set: 0,
                    values,
                })?;
            }
            assert!(matched, "unexpected query: {query}");
            Ok(())
        }
    }
    fn planner_fixture(after: &[u8]) -> PlanClient {
        let generation = "06cb0442-0c47-4fad-986a-f08f28287c1b";
        let version =
            versions::deflate_row(format!("{{1,1,\"\",{generation}}}").as_bytes()).unwrap();
        let marker = format!("{{0,1,{generation}}}").into_bytes();
        PlanClient {
            storage: vec![
                ("Config", blob_meta(ID, OLD), OLD.to_vec()),
                ("ConfigSave", blob_meta(ID, after), after.to_vec()),
                ("Config", blob_meta("versions", &version), version),
                ("Params", blob_meta("DynamicallyUpdated", &marker), marker),
            ],
        }
    }
    fn fixture_plan(client: &PlanClient) -> Result<Decision> {
        let staged = client
            .storage
            .iter()
            .filter(|(table, _, _)| *table == "ConfigSave")
            .map(|(_, meta, _)| meta.clone())
            .collect::<Vec<_>>();
        let replaced = client
            .storage
            .iter()
            .filter(|(table, meta, _)| {
                *table == "Config" && matches!(classify_name(&meta.name), RowName::Descriptor(_))
            })
            .map(|(_, meta, _)| meta.clone())
            .collect::<Vec<_>>();
        let version = &client
            .storage
            .iter()
            .find(|(_, meta, _)| meta.name == "versions")
            .unwrap()
            .2;
        let marker = &client
            .storage
            .iter()
            .find(|(table, _, _)| *table == "Params")
            .unwrap()
            .2;
        let special = client
            .storage
            .iter()
            .filter(|(table, meta, _)| {
                *table == "Config"
                    && (meta.name.contains("_dynupdate_") || meta.name == "DynamicallyUpdated")
            })
            .map(|(_, meta, _)| meta.clone())
            .collect::<Vec<_>>();
        let config_marker = client
            .storage
            .iter()
            .find(|(table, meta, _)| *table == "Config" && meta.name == "DynamicallyUpdated")
            .map(|(_, _, bytes)| bytes.as_slice());
        plan(PlanInput {
            client,
            db: "[owned]",
            profile: MssqlNativePlatformProfile::Platform8_3_27_2214,
            staged: &staged,
            replaced: &replaced,
            special: &special,
            active_versions: &versions::parse_versions(version)?,
            history: &versions::parse_dynamic_history(config_marker, Some(marker))?,
            config_marker,
            params_marker: Some(marker),
        })
    }
    #[test]
    fn planner_retains_recompressed_descriptor_marker_and_binds_all_preimages() {
        let client = planner_fixture(STAGED);
        let decision = fixture_plan(&client).unwrap();
        assert!(!decision.clear);
        for (table, meta, _) in &client.storage {
            assert!(
                decision
                    .guard_sql
                    .contains(&format!("dbo.{table} WITH (UPDLOCK, HOLDLOCK)"))
            );
            assert!(decision.guard_sql.contains(&meta.name));
            for value in [&meta.creation, &meta.modified, &meta.sha256] {
                assert!(decision.guard_sql.contains(value));
            }
        }
        // The whole-stage predicate also guards untouched staged bodies;
        // the special-row predicate proves absence of an undeclared overlay.
        assert!(decision.guard_sql.contains("WHERE 1=1"));
        assert!(decision.guard_sql.contains(dynamic_overlay::CONFIG_FILTER));
    }
    #[test]
    fn planner_collects_real_descriptor_changes_and_refuses_unknown_streams() {
        let mut plain = dynamic_metadata::inflate(OLD).unwrap();
        let index = plain.iter().position(|byte| *byte == b'"').unwrap() + 1;
        plain.insert(index, b'X');
        let changed = versions::deflate_row(&plain).unwrap();
        assert!(fixture_plan(&planner_fixture(&changed)).unwrap().clear);
        let mut trailing = STAGED.to_vec();
        trailing.push(0);
        assert!(fixture_plan(&planner_fixture(&trailing)).is_err());
        let mut drift = planner_fixture(STAGED);
        drift.storage[1].1.sha256 = "0".repeat(64);
        assert!(fixture_plan(&drift).is_err());
    }

    fn large_descriptor(id: &str, length: usize, entropy: bool) -> Vec<u8> {
        use std::io::Write;
        let mut plain = format!("\u{feff}{{3,{{1,0,{id}}},\"Object\",{{0}},\"").into_bytes();
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut state = 0x1c05_4180_0abc_def1u64;
        for _ in 0..length {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            plain.push(if entropy {
                alphabet[(state & 63) as usize]
            } else {
                b'A'
            });
        }
        plain.extend_from_slice(b"\",0,0,00000000-0000-0000-0000-000000000000,0}");
        let mut encoder =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&plain).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn ordinary_planner_accepts_more_than_128_descriptors_and_32mib_decoded_total() {
        let mut client = planner_fixture(STAGED);
        client
            .storage
            .retain(|(_, meta, _)| !matches!(classify_name(&meta.name), RowName::Descriptor(_)));
        let mut decoded_total = 0usize;
        for index in 1..=129 {
            let id = Uuid::from_u128(index).hyphenated().to_string();
            let bytes = large_descriptor(&id, 260 * 1024, false);
            decoded_total += inflate_complete(&bytes).unwrap().len();
            client
                .storage
                .push(("Config", blob_meta(&id, &bytes), bytes.clone()));
            client
                .storage
                .push(("ConfigSave", blob_meta(&id, &bytes), bytes));
        }
        assert!(decoded_total > 32 * 1024 * 1024);
        let decision = fixture_plan(&client).unwrap();
        assert!(!decision.clear);
        assert!(
            decision
                .guard_sql
                .contains("EXISTS (SELECT 1 FROM dbo.ConfigSave staged")
        );
        assert!(!decision.guard_sql.contains("FileName IN ("));
        for (_, meta, _) in &client.storage {
            assert!(decision.guard_sql.contains(&meta.name));
        }
    }

    #[test]
    fn ordinary_descriptor_accepts_actual_compressed_and_decoded_row_over_32mib() {
        let bytes = large_descriptor(ID, 48 * 1024 * 1024, true);
        assert!(
            bytes.len() > 32 * 1024 * 1024,
            "actual compressed row crosses former aggregate and per-row caps"
        );
        let meta = blob_meta(ID, &bytes);
        let descriptor = Descriptor::capture(ID, &meta, &bytes).unwrap();
        assert!(descriptor.plain.len() > 32 * 1024 * 1024);
        assert_eq!(descriptor_own_uuid(&descriptor.plain).unwrap(), Some(ID));
        let generation = "bd4ea842-aee4-41ee-94cc-ede196546578";
        let mut alias = meta;
        alias.name = format!("{ID}_dynupdate_{generation}");
        let versions =
            versions::deflate_row(format!("{{1,1,\"\",{generation}}}").as_bytes()).unwrap();
        validate_inventory(
            &[
                alias,
                blob_meta(&format!("versions_dynupdate_{generation}"), &versions),
            ],
            &[Uuid::parse_str(generation).unwrap()],
        )
        .unwrap();
    }
    #[test]
    fn planner_uses_latest_published_descriptor_and_guards_older_aliases() {
        let mut client = planner_fixture(STAGED);
        let g1 = "bd4ea842-aee4-41ee-94cc-ede196546578";
        let g2 = "719baa18-69ed-439a-8962-1de53d98e05e";
        let mut plain = dynamic_metadata::inflate(OLD).unwrap();
        let index = plain.iter().position(|byte| *byte == b'"').unwrap() + 1;
        plain.insert(index, b'X');
        let changed = versions::deflate_row(&plain).unwrap();
        for (generation, descriptor) in [(g1, OLD), (g2, changed.as_slice())] {
            let name = format!("{ID}_dynupdate_{generation}");
            client
                .storage
                .push(("Config", blob_meta(&name, descriptor), descriptor.to_vec()));
            let name = format!("versions_dynupdate_{generation}");
            let bytes =
                versions::deflate_row(format!("{{1,1,\"\",{generation}}}").as_bytes()).unwrap();
            client
                .storage
                .push(("Config", blob_meta(&name, &bytes), bytes));
        }
        let marker = format!("{{1,2,{g1},{g2}}}").into_bytes();
        client
            .storage
            .push(("Config", blob_meta("DynamicallyUpdated", &marker), marker));
        let params = format!("{{0,3,06cb0442-0c47-4fad-986a-f08f28287c1b,{g1},{g2}}}").into_bytes();
        client.storage[3] = ("Params", blob_meta("DynamicallyUpdated", &params), params);
        let decision = fixture_plan(&client).unwrap();
        assert!(
            decision.clear,
            "reverting to the ordinary row changes the effective alias"
        );
        assert!(decision.guard_sql.contains(&format!("{ID}_dynupdate_{g1}")));
        assert!(decision.guard_sql.contains(&format!("{ID}_dynupdate_{g2}")));
        client.storage[1] = ("ConfigSave", blob_meta(ID, &changed), changed);
        assert!(!fixture_plan(&client).unwrap().clear);
        client.storage[4].1.attributes = 1;
        assert!(
            fixture_plan(&client).is_err(),
            "older aliases must also satisfy admission"
        );
    }
    impl SqlClient for CaptureClient {
        fn dbms(&self) -> Dbms {
            Dbms::SqlServer
        }
        fn max_connections(&self) -> usize {
            1
        }
        fn run_script(&self, _: &str, _: ScriptVariables) -> Result<()> {
            panic!("read-only")
        }
        fn execute(&self, _: &str, _: &[SqlParam<'_>]) -> Result<u64> {
            panic!("read-only")
        }
        fn query_json(&self, _: &str) -> Result<Option<String>> {
            panic!("read-only")
        }
        fn write_rows(&self, _: &str, _: &[&str], _: &[Vec<SqlParam<'_>>]) -> Result<u64> {
            panic!("read-only")
        }
        fn read_rows(
            &self,
            query: &str,
            _: &[SqlParam<'_>],
            each: &mut dyn FnMut(SqlRow) -> Result<()>,
        ) -> Result<()> {
            assert!(query.contains(&predicate(&self.expected)));
            for bytes in &self.rows {
                each(SqlRow {
                    result_set: 0,
                    values: vec![SqlValue::Binary(bytes.clone())],
                })?;
            }
            Ok(())
        }
    }

    #[test]
    fn server_fullheader_filter_client_digest_and_alias_cas_bind_judged_payload() {
        let header = blob_meta(ID, OLD);
        for rows in [
            vec![],
            vec![STAGED.to_vec()],
            vec![OLD.to_vec(), OLD.to_vec()],
        ] {
            let client = CaptureClient {
                rows,
                expected: header.clone(),
            };
            assert!(Descriptor::read(&client, "[owned]", "Config", ID, &header,).is_err());
        }
        let client = CaptureClient {
            rows: vec![OLD.to_vec()],
            expected: header.clone(),
        };
        Descriptor::read(&client, "[owned]", "Config", ID, &header).unwrap();
        let sql = dynamic_overlay::guard(
            "Config",
            &format!("FileName = N'{ID}'"),
            std::slice::from_ref(&header),
        );
        for field in [
            "WITH (UPDLOCK, HOLDLOCK)",
            "PartNo = 0",
            "DataSize) = 173",
            "DATALENGTH(BinaryData) = 173",
            "Attributes = 0",
            &header.creation,
            &header.modified,
            &header.sha256,
        ] {
            assert!(sql.contains(field));
        }
        let mut alias = header;
        alias.name = format!("{ID}_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b");
        Descriptor::capture(ID, &alias, OLD).unwrap();
        assert!(
            dynamic_overlay::guard(
                "Config",
                &format!("FileName = N'{}'", alias.name),
                &[alias.clone()]
            )
            .contains(&alias.name)
        );
    }
}
