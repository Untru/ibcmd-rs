//! Candidate semantic facts for #418. Collection policy is measured separately;
//! a staged descriptor, or a changed compressed digest, is not a collection proof.

use crate::metadata_model::export::names::own_header;
use crate::mssql_main_activation::{MAX_PLAN_BYTES, MAX_ROW_BYTES, MAX_ROWS};
use crate::sql::SqlClient;
use anyhow::{Result, ensure};
use std::collections::HashSet;
use uuid::Uuid;

use super::dynamic_metadata;
use super::dynamic_overlay;
use super::model::RowMeta;
use super::model::{RowName, classify_name, quote_string};

fn canonical_descriptor(name: &str) -> Result<Uuid> {
    let id = Uuid::parse_str(name)?;
    ensure!(
        !id.is_nil() && id.hyphenated().to_string() == name,
        "noncanonical descriptor identity"
    );
    Ok(id)
}

/// One budget shared across all active AND staged descriptor reads.
pub(super) struct Budget {
    compressed: usize,
    plain: usize,
    rows: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            compressed: MAX_PLAN_BYTES,
            plain: MAX_PLAN_BYTES,
            rows: MAX_ROWS * 2,
        }
    }
}

/// Immutable judged bytes with the complete inventoried physical header.
pub(super) struct Descriptor {
    pub meta: RowMeta,
    id: Uuid,
    plain: Vec<u8>,
}
impl Descriptor {
    fn validate_meta(id: &str, meta: &RowMeta, budget: &Budget) -> Result<()> {
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
                && (0..=MAX_ROW_BYTES as i64).contains(&meta.byte_len),
            "unmeasured descriptor header"
        );
        ensure!(
            meta.sha256.len() == 64 && meta.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid descriptor digest"
        );
        ensure!(
            budget.rows > 0 && meta.byte_len as usize <= budget.compressed,
            "aggregate descriptor read budget exceeded"
        );
        Ok(())
    }
    fn capture(id: &str, meta: &RowMeta, bytes: &[u8], budget: &mut Budget) -> Result<Self> {
        Self::validate_meta(id, meta, budget)?;
        let id = canonical_descriptor(id)?;
        dynamic_overlay::bound_blob(meta, bytes)?;
        let plain = dynamic_metadata::inflate_limit(bytes, budget.plain)?;
        let row = dynamic_metadata::descriptor_plain(&plain)?;
        let (own_id, _) =
            own_header(&row).ok_or_else(|| anyhow::anyhow!("descriptor has no own header"))?;
        ensure!(
            canonical_descriptor(&own_id)? == id,
            "descriptor own header differs from requested identity"
        );
        budget.compressed -= bytes.len();
        budget.plain -= plain.len();
        budget.rows -= 1;
        Ok(Self {
            meta: meta.clone(),
            id,
            plain,
        })
    }
    /// Server-side full header/digest/length filter precedes transfer. Exactly
    /// one returned blob must still match the first inventory before decoding.
    pub(super) fn read(
        client: &dyn SqlClient,
        db: &str,
        table: &str,
        id: &str,
        meta: &RowMeta,
        budget: &mut Budget,
    ) -> Result<Self> {
        ensure!(
            matches!(table, "Config" | "ConfigSave"),
            "invalid descriptor storage table"
        );
        Self::validate_meta(id, meta, budget)?;
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
            budget,
        )
    }
    pub(super) fn same_content(&self, other: &Self) -> Result<bool> {
        ensure!(self.id == other.id, "unrelated descriptor identities");
        Ok(self.plain == other.plain)
    }
    /// Inside the serializable transaction BEFORE any fold, publication,
    /// marker deletion or stage consumption; includes older alias preimages.
    pub(super) fn guard(&self, table: &str) -> Result<String> {
        ensure!(
            matches!(table, "Config" | "ConfigSave"),
            "invalid descriptor guard table"
        );
        Ok(dynamic_overlay::guard(
            table,
            &format!("FileName = N'{}'", quote_string(&self.meta.name)),
            std::slice::from_ref(&self.meta),
        ))
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
        rows.len() <= MAX_ROWS && history.len() <= MAX_ROWS,
        "alias inventory exceeds bound"
    );
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
    let mut total = 0usize;
    for row in rows {
        ensure!(
            row.part == 0
                && row.attributes == 0
                && row.data_size == row.byte_len
                && (0..=MAX_ROW_BYTES as i64).contains(&row.byte_len)
                && keys.insert(row.key()),
            "duplicate or unmeasured alias row"
        );
        total = total
            .checked_add(row.byte_len as usize)
            .ok_or_else(|| anyhow::anyhow!("alias size overflow"))?;
        ensure!(
            total <= MAX_PLAN_BYTES,
            "alias inventory aggregate bound exceeded"
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
pub(super) fn effective_descriptor<'a>(
    ordinary: Option<&'a RowMeta>,
    descriptor: &str,
    aliases: &'a [RowMeta],
    history: &[Uuid],
) -> Result<Option<&'a RowMeta>> {
    canonical_descriptor(descriptor)?;
    validate_inventory(aliases, history)?;
    for generation in history.iter().rev() {
        let name = format!("{descriptor}_dynupdate_{}", generation.hyphenated());
        let rows = aliases
            .iter()
            .filter(|row| row.name.eq_ignore_ascii_case(&name))
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            ensure!(
                rows.len() == 1 && rows[0].part == 0,
                "effective descriptor is missing, duplicated or multipart"
            );
            return Ok(Some(rows[0]));
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

#[cfg(test)]
mod tests {
    use super::super::versions;
    use super::*;
    use crate::sql::{Dbms, ScriptVariables, SqlParam, SqlRow, SqlValue};
    use sha2::{Digest, Sha256};

    const ID: &str = "16b3681c-426d-4d6f-9ffe-588a23974222";
    fn same_descriptor_content(active: &[u8], staged: &[u8]) -> Result<bool> {
        let mut budget = Budget::default();
        Descriptor::capture(ID, &blob_meta(ID, active), active, &mut budget)?.same_content(
            &Descriptor::capture(ID, &blob_meta(ID, staged), staged, &mut budget)?,
        )
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
    fn foreign_uuid_and_aggregate_decoded_budget_refuse_even_valid_native_payloads() {
        let foreign = "26b3681c-426d-4d6f-9ffe-588a23974222";
        assert!(
            Descriptor::capture(
                foreign,
                &blob_meta(foreign, OLD),
                OLD,
                &mut Budget::default()
            )
            .is_err()
        );
        let mut budget = Budget {
            compressed: 1024,
            plain: 577,
            rows: 2,
        };
        Descriptor::capture(ID, &blob_meta(ID, OLD), OLD, &mut budget).unwrap();
        assert!(Descriptor::capture(ID, &blob_meta(ID, STAGED), STAGED, &mut budget).is_err());
        let header = blob_meta(ID, OLD);
        assert!(Descriptor::capture(ID, &header, STAGED, &mut Budget::default()).is_err());
        let mut flags = header;
        flags.attributes = 1;
        assert!(Descriptor::capture(ID, &flags, OLD, &mut Budget::default()).is_err());
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
            assert!(
                Descriptor::read(
                    &client,
                    "[owned]",
                    "Config",
                    ID,
                    &header,
                    &mut Budget::default()
                )
                .is_err()
            );
        }
        let client = CaptureClient {
            rows: vec![OLD.to_vec()],
            expected: header.clone(),
        };
        let bound = Descriptor::read(
            &client,
            "[owned]",
            "Config",
            ID,
            &header,
            &mut Budget::default(),
        )
        .unwrap();
        let sql = bound.guard("Config").unwrap();
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
        let bound = Descriptor::capture(ID, &alias, OLD, &mut Budget::default()).unwrap();
        assert!(bound.guard("Config").unwrap().contains(&alias.name));
    }
}
