//! Physical-row arithmetic for SQL preflight; no SQL I/O or write authority.
//!
//! A builder cannot prove a coherent database snapshot: the acquisition adapter
//! must capture identity/schema and both tables in one bounded session. Likewise,
//! this local digest is not a cross-session SQL compare-and-swap. Publication must
//! compare the full expected rows under locks inside its own transaction.
use crate::mssql_live_continue::DatabaseIdentity;
use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

pub type Check = Arc<dyn Fn() -> Result<()> + Send + Sync>;
#[derive(Clone, Copy)]
pub struct Limits {
    pub max_parts: usize,
    pub max_part_bytes: u64,
    pub max_payload_bytes: u64,
    pub max_metadata_bytes: u64,
    pub max_name_units: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_parts: 100_000,
            max_part_bytes: 16 * 1024 * 1024,
            max_payload_bytes: 64 * 1024 * 1024,
            max_metadata_bytes: 8 * 1024 * 1024,
            max_name_units: 128,
        }
    }
}
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct Key {
    pub file_name: String,
    pub part_no: u32,
}
#[derive(Clone, Eq, PartialEq)]
pub struct Part {
    pub data_size: u64,
    /// Complete canonical header image supplied by the measured SQL adapter.
    /// Includes creation/modified/attributes; no header is synthesized here.
    pub header: Vec<u8>,
    pub binary: Arc<Vec<u8>>,
}
impl fmt::Debug for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Part")
            .field("data_size", &self.data_size)
            .field("header_bytes", &self.header.len())
            .field("binary_bytes", &self.binary.len())
            .finish()
    }
}
/// Owned transport row; never deserialized from editor request parameters.
pub struct Input {
    pub key: Key,
    pub part: Part,
}
#[derive(Clone, Copy)]
pub enum Table {
    Config,
    ConfigSave,
}
type Parts = BTreeMap<Key, Part>;

pub struct Builder {
    identity: DatabaseIdentity,
    schema: [u8; 32],
    config: Parts,
    saved: Parts,
    limits: Limits,
    check: Check,
    count: usize,
    payload: u64,
    metadata: u64,
    failed: bool,
}
impl Builder {
    pub fn new(
        identity: DatabaseIdentity,
        schema: [u8; 32],
        limits: Limits,
        check: Check,
    ) -> Result<Self> {
        check()?;
        if identity.server.is_empty() || identity.database.is_empty() {
            bail!("SQL snapshot identity is incomplete");
        }
        for value in [
            &identity.database_guid,
            &identity.family_guid,
            &identity.recovery_fork,
        ] {
            uuid::Uuid::parse_str(value)
                .map_err(|_| anyhow::anyhow!("SQL snapshot identity GUID is invalid"))?;
        }
        let metadata = [
            &identity.server,
            &identity.database,
            &identity.database_guid,
            &identity.family_guid,
            &identity.recovery_fork,
        ]
        .into_iter()
        .try_fold(0u64, |total, value| total.checked_add(value.len() as u64))
        .ok_or_else(|| anyhow::anyhow!("SQL snapshot metadata size overflow"))?;
        if limits.max_parts == 0
            || limits.max_part_bytes == 0
            || limits.max_payload_bytes == 0
            || limits.max_metadata_bytes == 0
            || limits.max_name_units == 0
            || metadata > limits.max_metadata_bytes
        {
            bail!("SQL snapshot limits are invalid or exceeded");
        }
        Ok(Self {
            identity,
            schema,
            config: BTreeMap::new(),
            saved: BTreeMap::new(),
            limits,
            check,
            count: 0,
            payload: 0,
            metadata,
            failed: false,
        })
    }
    /// Admission errors poison the builder; partial reads cannot become a token.
    pub fn push(&mut self, table: Table, input: Input) -> Result<()> {
        if self.failed {
            bail!("SQL snapshot builder was invalidated");
        }
        let result = self.push_inner(table, input);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn push_inner(&mut self, table: Table, input: Input) -> Result<()> {
        (self.check)()?;
        let Input { key, part } = input;
        if key.file_name.is_empty()
            || key.file_name.contains('\0')
            || key
                .file_name
                .encode_utf16()
                .take(self.limits.max_name_units.saturating_add(1))
                .count()
                > self.limits.max_name_units
        {
            bail!("SQL snapshot row name is invalid");
        }
        if part.header.is_empty() {
            bail!("SQL snapshot header image is missing");
        }
        let bytes = part.binary.len() as u64;
        let count = self
            .count
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("SQL part count overflow"))?;
        let payload = self
            .payload
            .checked_add(bytes)
            .ok_or_else(|| anyhow::anyhow!("SQL payload size overflow"))?;
        let metadata = self
            .metadata
            .checked_add(key.file_name.len() as u64)
            .and_then(|total| total.checked_add(part.header.len() as u64))
            .ok_or_else(|| anyhow::anyhow!("SQL metadata size overflow"))?;
        if count > self.limits.max_parts
            || bytes > self.limits.max_part_bytes
            || payload > self.limits.max_payload_bytes
            || metadata > self.limits.max_metadata_bytes
        {
            bail!("SQL snapshot resource limit exceeded");
        }
        let rows = match table {
            Table::Config => &mut self.config,
            Table::ConfigSave => &mut self.saved,
        };
        if rows.contains_key(&key) {
            bail!("SQL snapshot contains a duplicate physical part");
        }
        rows.insert(key, part);
        self.count = count;
        self.payload = payload;
        self.metadata = metadata;
        Ok(())
    }
    pub fn finish(self) -> Result<Readset> {
        if self.failed {
            bail!("SQL snapshot builder was invalidated");
        }
        validate_parts(&self.config, &self.check)?;
        validate_parts(&self.saved, &self.check)?;
        (self.check)()?;
        Ok(Readset {
            identity: self.identity,
            schema: self.schema,
            config: self.config,
            saved: self.saved,
            limits: self.limits,
            check: self.check,
        })
    }
}

fn validate_parts(rows: &Parts, check: &Check) -> Result<()> {
    let mut name: Option<&str> = None;
    let mut next = 0u32;
    let mut size = 0u64;
    let mut total = 0u64;
    for (key, part) in rows {
        check()?;
        if name != Some(key.file_name.as_str()) {
            if name.is_some() && total != size {
                bail!("SQL snapshot row payload size mismatch");
            }
            name = Some(&key.file_name);
            next = 0;
            size = part.data_size;
            total = 0;
        }
        if key.part_no != next || part.data_size != size {
            bail!("SQL snapshot has a part gap or inconsistent row size");
        }
        next = next
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("SQL row part overflow"))?;
        total = total
            .checked_add(part.binary.len() as u64)
            .ok_or_else(|| anyhow::anyhow!("SQL row payload overflow"))?;
    }
    if name.is_some() && total != size {
        bail!("SQL snapshot row payload size mismatch");
    }
    Ok(())
}

#[derive(Clone)]
pub struct Readset {
    identity: DatabaseIdentity,
    schema: [u8; 32],
    config: Parts,
    saved: Parts,
    limits: Limits,
    check: Check,
}
impl Readset {
    pub(super) fn limits(&self) -> Limits {
        self.limits
    }
    pub(super) fn identity(&self) -> &DatabaseIdentity {
        &self.identity
    }
    pub(super) fn schema(&self) -> [u8; 32] {
        self.schema
    }

    pub fn parts(&self, table: Table) -> impl Iterator<Item = (&Key, &Part)> {
        match table {
            Table::Config => &self.config,
            Table::ConfigSave => &self.saved,
        }
        .iter()
    }
    /// Compares complete captured values, never just digests or selected rows.
    /// This is the pure CAS predicate, NOT a transactional capability. A future
    /// publisher must collect `current` on its ONE dedicated session while all
    /// CONFIG/CONFIGSAVE locks remain held through DML and COMMIT. In particular
    /// snapshot::capture rolls back before returning and cannot establish that.
    pub fn require_equal(&self, current: &Readset) -> Result<()> {
        (self.check)()?;
        (current.check)()?;
        anyhow::ensure!(
            self.identity.server == current.identity.server
                && self.identity.database == current.identity.database
                && self.identity.database_guid == current.identity.database_guid
                && self.identity.family_guid == current.identity.family_guid
                && self.identity.recovery_fork == current.identity.recovery_fork,
            "SQL readset target identity changed"
        );
        anyhow::ensure!(self.schema == current.schema, "SQL readset schema changed");
        let bytes_equal = |a: &[u8], b: &[u8]| -> Result<()> {
            anyhow::ensure!(a.len() == b.len(), "SQL readset byte length changed");
            for (a, b) in a.chunks(64 * 1024).zip(b.chunks(64 * 1024)) {
                (self.check)()?;
                (current.check)()?;
                anyhow::ensure!(a == b, "SQL readset bytes changed");
            }
            Ok(())
        };
        for (expected, actual) in [
            (&self.config, &current.config),
            (&self.saved, &current.saved),
        ] {
            (self.check)()?;
            (current.check)()?;
            anyhow::ensure!(
                expected.len() == actual.len(),
                "SQL readset physical coverage changed"
            );
            for ((a, part_a), (b, part_b)) in expected.iter().zip(actual) {
                (self.check)()?;
                (current.check)()?;
                anyhow::ensure!(a == b, "SQL readset physical key changed");
                anyhow::ensure!(
                    part_a.data_size == part_b.data_size,
                    "SQL readset total size changed"
                );
                bytes_equal(&part_a.header, &part_b.header)?;
                bytes_equal(part_a.binary.as_slice(), part_b.binary.as_slice())?;
            }
        }
        (self.check)()?;
        (current.check)()?;
        Ok(())
    }

    /// Local plan binding/fault oracle only; not SQL transaction authorization.
    pub fn digest(&self) -> Result<[u8; 32]> {
        let mut hash = Sha256::new();
        feed(&mut hash, b"ibcmd-sql-readset-v1", &self.check)?;
        for value in [
            &self.identity.server,
            &self.identity.database,
            &self.identity.database_guid,
            &self.identity.family_guid,
            &self.identity.recovery_fork,
        ] {
            feed(&mut hash, value.as_bytes(), &self.check)?;
        }
        feed(&mut hash, &self.schema, &self.check)?;
        for (label, rows) in [
            (b"CONFIG".as_slice(), &self.config),
            (b"CONFIGSAVE".as_slice(), &self.saved),
        ] {
            feed(&mut hash, label, &self.check)?;
            hash.update((rows.len() as u64).to_le_bytes());
            for (key, part) in rows {
                feed(&mut hash, key.file_name.as_bytes(), &self.check)?;
                hash.update(key.part_no.to_le_bytes());
                hash.update(part.data_size.to_le_bytes());
                feed(&mut hash, &part.header, &self.check)?;
                feed(&mut hash, &part.binary, &self.check)?;
            }
        }
        (self.check)()?;
        Ok(hash.finalize().into())
    }
    /// Pure row arithmetic. The transaction adapter still needs compiler proof,
    /// exact target identity/readset CAS and same-session publication.
    pub fn with_selected_stage(
        &self,
        replacements: impl IntoIterator<Item = Input>,
    ) -> Result<Self> {
        let mut selected = Builder::new(
            self.identity.clone(),
            self.schema,
            self.limits,
            self.check.clone(),
        )?;
        for input in replacements {
            selected.push(Table::ConfigSave, input)?;
        }
        let selected = selected.finish()?;
        let mut names = BTreeSet::new();
        for key in selected.saved.keys() {
            (self.check)()?;
            names.insert(key.file_name.clone());
        }
        if names.is_empty() {
            bail!("SQL replacement stage is empty");
        }
        let mut existing = BTreeSet::new();
        for key in self.config.keys().chain(self.saved.keys()) {
            (self.check)()?;
            existing.insert(key.file_name.as_str());
        }
        for name in &names {
            (self.check)()?;
            if !existing.contains(name.as_str()) {
                bail!("SQL partial import cannot add a new stored row");
            }
        }
        let mut merged = Builder::new(
            self.identity.clone(),
            self.schema,
            self.limits,
            self.check.clone(),
        )?;
        for (key, part) in &self.config {
            merged.push(
                Table::Config,
                Input {
                    key: key.clone(),
                    part: part.clone(),
                },
            )?;
        }
        for (key, part) in &self.saved {
            if !names.contains(key.file_name.as_str()) {
                merged.push(
                    Table::ConfigSave,
                    Input {
                        key: key.clone(),
                        part: part.clone(),
                    },
                )?;
            }
        }
        for (key, part) in selected.saved {
            merged.push(Table::ConfigSave, Input { key, part })?;
        }
        merged.finish()
    }
}
fn feed(hash: &mut Sha256, bytes: &[u8], check: &Check) -> Result<()> {
    hash.update((bytes.len() as u64).to_le_bytes());
    for chunk in bytes.chunks(64 * 1024) {
        check()?;
        hash.update(chunk);
    }
    check()?;
    Ok(())
}

#[cfg(test)]
mod exact_readset_tests {
    use super::*;
    fn fixture() -> Readset {
        let identity = DatabaseIdentity {
            server: "synthetic".into(),
            database: "synthetic".into(),
            database_guid: uuid::Uuid::from_u128(1).to_string(),
            family_guid: uuid::Uuid::from_u128(2).to_string(),
            recovery_fork: uuid::Uuid::from_u128(3).to_string(),
        };
        let mut builder =
            Builder::new(identity, [4; 32], Limits::default(), Arc::new(|| Ok(()))).unwrap();
        for table in [Table::Config, Table::ConfigSave] {
            builder
                .push(
                    table,
                    Input {
                        key: Key {
                            file_name: "neighbour".into(),
                            part_no: 0,
                        },
                        part: Part {
                            data_size: 128 * 1024,
                            header: vec![1, 2, 3],
                            binary: Arc::new(vec![9; 128 * 1024]),
                        },
                    },
                )
                .unwrap();
        }
        builder.finish().unwrap()
    }
    #[test]
    fn exact_readset_accepts_identical_rows_without_digest_authority() {
        let a = fixture();
        let b = fixture();
        a.require_equal(&b).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
    #[test]
    fn exact_readset_refuses_identity_schema_coverage_headers_and_payload_changes() {
        let mutations: [fn(&mut Readset); 12] = [
            |r| r.identity.server.push('x'),
            |r| r.identity.database.push('x'),
            |r| r.identity.database_guid = uuid::Uuid::from_u128(5).to_string(),
            |r| r.identity.family_guid = uuid::Uuid::from_u128(5).to_string(),
            |r| r.identity.recovery_fork = uuid::Uuid::from_u128(5).to_string(),
            |r| r.schema[0] ^= 1,
            |r| {
                r.config.clear();
            },
            |r| {
                let part = r.saved.values().next().unwrap().clone();
                r.saved.insert(
                    Key {
                        file_name: "added".into(),
                        part_no: 0,
                    },
                    part,
                );
            },
            |r| {
                let (mut key, part) = r.saved.pop_first().unwrap();
                key.part_no = 1;
                r.saved.insert(key, part);
            },
            |r| r.saved.values_mut().next().unwrap().data_size += 1,
            |r| r.saved.values_mut().next().unwrap().header[0] ^= 1,
            |r| Arc::make_mut(&mut r.saved.values_mut().next().unwrap().binary)[100_000] ^= 1,
        ];
        for mutation in mutations {
            let a = fixture();
            let mut b = fixture();
            mutation(&mut b);
            assert!(a.require_equal(&b).is_err());
        }
        let a = fixture();
        let mut b = fixture();
        let (mut key, part) = b.config.pop_first().unwrap();
        key.file_name = "Neighbour".into();
        b.config.insert(key, part);
        assert!(a.require_equal(&b).is_err());
    }
    #[derive(Debug)]
    struct Cancelled;
    impl std::fmt::Display for Cancelled {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "synthetic cancellation")
        }
    }
    impl std::error::Error for Cancelled {}
    #[test]
    fn exact_readset_preserves_typed_cancellation_during_comparison() {
        let mut a = fixture();
        let b = fixture();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let seen = count.clone();
        a.check = Arc::new(move || {
            if seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 3 {
                return Err(anyhow::Error::new(Cancelled));
            }
            Ok(())
        });
        assert!(
            a.require_equal(&b)
                .unwrap_err()
                .downcast_ref::<Cancelled>()
                .is_some()
        );
        assert!(count.load(std::sync::atomic::Ordering::SeqCst) > 3);
    }
}
