//! Diagnostic input admission and key coverage only. Never an insertion planner.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use flate2::{Decompress, FlushDecompress, Status};
use ibcmd_rs::metadata_model::brace::{Brace, NIL_UUID, parse_row};
use ibcmd_rs::mssql_config_apply::si::{self, SiMain};
use ibcmd_rs::restructure::caches::{facts, help_props::HelpProps, registry, type_sets::TypeSets};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const SET_LINK_LIST_CLASS: &str = "9cd510d6-abfc-11d4-9434-004095e12fc7";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RowRole {
    Registry,
    TypeSets,
    HelpProps,
}

impl RowRole {
    pub fn filename(self) -> &'static str {
        match self {
            Self::Registry => registry::REGISTRY_ROW,
            Self::TypeSets => "fe8acd6a-22c9-4b5a-aeae-232a1c8324cb.si",
            Self::HelpProps => "c4629235-4823-4320-b8b5-1d08f4c6d612.si",
        }
    }
}

/// These are exact supplied bindings, not inferred from directory names.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RowOrigin {
    pub case: String,
    pub stage: String,
    pub table: String,
    pub filename: String,
    pub part: u32,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "encoding", rename_all = "snake_case", deny_unknown_fields)]
pub enum Locator {
    Plain {
        path: PathBuf,
    },
    RawDeflate {
        path: PathBuf,
    },
    /// One closed gzip member from an append-only pack, containing raw DEFLATE.
    GzipRawDeflateRange {
        path: PathBuf,
        offset: u64,
    },
}

impl Locator {
    fn path(&self) -> &Path {
        match self {
            Self::Plain { path }
            | Self::RawDeflate { path }
            | Self::GzipRawDeflateRange { path, .. } => path,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RowBinding {
    pub role: RowRole,
    pub origin: RowOrigin,
    pub locator: Locator,
    /// Hash/length of precisely the selected file or pack member, not the whole pack.
    pub stored_length: u64,
    pub stored_sha256: String,
    /// For gzip packs, the inner raw-DEFLATE bytes also have independent custody.
    pub packed_length: u64,
    pub packed_sha256: String,
    pub plain_length: u64,
    pub plain_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestV1 {
    pub schema: String,
    pub source_head: String,
    pub case: String,
    pub stage: String,
    pub snapshot_purpose: String,
    pub registry_root_uuid: String,
    pub predecessors: Vec<FileBinding>,
    pub rows: Vec<RowBinding>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileBinding {
    pub path: PathBuf,
    pub length: u64,
    pub sha256: String,
}

impl FileBinding {
    fn verify(&self) -> Result<()> {
        physical(&self.path)?;
        ensure!(hex(&self.sha256, 64), "invalid predecessor hash");
        ensure!(
            fs::metadata(&self.path)?.len() == self.length,
            "predecessor length changed"
        );
        let mut file = fs::File::open(&self.path)?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 8192];
        let mut length = 0u64;
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            length = length
                .checked_add(count as u64)
                .context("predecessor length overflow")?;
            digest.update(&buffer[..count]);
        }
        ensure!(
            length == self.length && format!("{:x}", digest.finalize()) == self.sha256,
            "predecessor bytes changed"
        );
        physical(&self.path)?;
        Ok(())
    }
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn uuid(value: &str) -> Result<String> {
    let parsed = uuid::Uuid::parse_str(value).context("invalid UUID")?;
    let canonical = parsed.hyphenated().to_string();
    ensure!(
        value.eq_ignore_ascii_case(&canonical),
        "UUID must use full hyphenated grammar"
    );
    Ok(canonical)
}

fn atom(value: &Brace) -> Result<&str> {
    value.as_atom().context("expected atom")
}

fn list(value: &Brace) -> Result<&[Brace]> {
    value.as_list().context("expected list")
}

fn count(value: &Brace) -> Result<usize> {
    let text = atom(value)?;
    ensure!(
        !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()),
        "invalid count"
    );
    text.parse().context("count is not addressable")
}

fn counted(items: &[Brace], stride: usize) -> Result<&[Brace]> {
    let (declared, rest) = items.split_first().context("missing count")?;
    let expected = count(declared)?
        .checked_mul(stride)
        .context("count multiplication overflow")?;
    ensure!(
        rest.len() == expected,
        "declared count differs from actual items"
    );
    Ok(rest)
}

/// No symlink/reparse ancestry, including a symlink to a correctly hashed file.
fn physical(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "input path must be absolute");
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor).context("input ancestor unavailable")?;
        ensure!(!metadata.file_type().is_symlink(), "linked input ancestor");
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            ensure!(
                metadata.file_attributes() & 0x400 == 0,
                "reparse input ancestor"
            );
        }
    }
    ensure!(fs::metadata(path)?.is_file(), "input is not a regular file");
    Ok(())
}

fn read_stored(binding: &RowBinding) -> Result<Vec<u8>> {
    use std::io::{Seek, SeekFrom};
    let path = binding.locator.path();
    physical(path)?;
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    let offset = match binding.locator {
        Locator::GzipRawDeflateRange { offset, .. } => offset,
        _ => {
            ensure!(
                metadata.len() == binding.stored_length,
                "file length changed"
            );
            0
        }
    };
    let end = offset
        .checked_add(binding.stored_length)
        .context("range overflow")?;
    ensure!(end <= metadata.len(), "selected range lies outside file");
    file.seek(SeekFrom::Start(offset))?;
    let length = usize::try_from(binding.stored_length).context("range is not addressable")?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .context("cannot allocate actual selected input")?;
    file.take(binding.stored_length).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() == length && hash(&bytes) == binding.stored_sha256,
        "stored bytes changed"
    );
    physical(path)?;
    Ok(bytes)
}

/// Complete raw-DEFLATE consumption, with output growth derived from actual decoded chunks.
fn inflate(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = Decompress::new(false);
    let mut output = Vec::new();
    loop {
        let consumed = usize::try_from(decoder.total_in())?;
        let before_out = decoder.total_out();
        let mut chunk = [0u8; 8192];
        let flush = if consumed == input.len() {
            FlushDecompress::Finish
        } else {
            FlushDecompress::None
        };
        let status = decoder.decompress(&input[consumed..], &mut chunk, flush)?;
        let produced = usize::try_from(decoder.total_out() - before_out)?;
        output
            .try_reserve(produced)
            .context("cannot retain actual decoded bytes")?;
        output.extend_from_slice(&chunk[..produced]);
        if status == Status::StreamEnd {
            ensure!(
                decoder.total_in() == u64::try_from(input.len())?,
                "trailing DEFLATE bytes"
            );
            return Ok(output);
        }
        ensure!(
            decoder.total_in() > consumed as u64 || produced > 0,
            "incomplete DEFLATE stream"
        );
    }
}

fn decode(binding: &RowBinding, stored: &[u8]) -> Result<Vec<u8>> {
    let packed = match &binding.locator {
        Locator::GzipRawDeflateRange { .. } => {
            let mut decoder = flate2::bufread::GzDecoder::new(Cursor::new(stored));
            let mut packed = Vec::new();
            decoder.read_to_end(&mut packed)?;
            ensure!(
                decoder.into_inner().position() == stored.len() as u64,
                "trailing gzip member bytes"
            );
            packed
        }
        _ => stored.to_vec(),
    };
    ensure!(
        packed.len() as u64 == binding.packed_length && hash(&packed) == binding.packed_sha256,
        "packed binding differs"
    );
    let plain = match binding.locator {
        Locator::Plain { .. } => packed,
        _ => inflate(&packed)?,
    };
    ensure!(
        plain.len() as u64 == binding.plain_length && hash(&plain) == binding.plain_sha256,
        "plain binding differs"
    );
    Ok(plain)
}

/// Private retained bytes: subsequent filesystem drift cannot silently replace parsed input.
pub struct ProjectionInputs {
    manifest: ManifestV1,
    plain: BTreeMap<RowRole, Vec<u8>>,
}

impl ProjectionInputs {
    pub fn load(manifest: ManifestV1, expected_source_head: &str) -> Result<Self> {
        ensure!(
            manifest.schema == "cache-insertion-trace-input-v1",
            "unknown input schema"
        );
        ensure!(
            hex(expected_source_head, 40) && manifest.source_head == expected_source_head,
            "source binding differs"
        );
        for text in [&manifest.case, &manifest.stage, &manifest.snapshot_purpose] {
            ensure!(
                !text.is_empty() && !text.chars().any(char::is_control),
                "invalid snapshot identity"
            );
        }
        ensure!(
            !manifest.predecessors.is_empty(),
            "missing predecessor binding"
        );
        for predecessor in &manifest.predecessors {
            predecessor.verify()?;
        }
        ensure!(
            uuid(&manifest.registry_root_uuid)? != NIL_UUID,
            "nil registry root"
        );
        let mut plain = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for binding in &manifest.rows {
            ensure!(
                binding.origin.case == manifest.case && binding.origin.stage == manifest.stage,
                "mixed case/stage rows"
            );
            ensure!(
                binding.origin.table == "Params" && binding.origin.part == 0,
                "wrong row table/part"
            );
            ensure!(
                binding.origin.filename == binding.role.filename(),
                "wrong row family filename"
            );
            ensure!(
                !binding.origin.version.is_empty()
                    && !binding.origin.version.chars().any(char::is_control),
                "missing row version"
            );
            ensure!(
                hex(&binding.stored_sha256, 64)
                    && hex(&binding.packed_sha256, 64)
                    && hex(&binding.plain_sha256, 64),
                "invalid byte hash"
            );
            let offset = match binding.locator {
                Locator::GzipRawDeflateRange { offset, .. } => offset,
                _ => 0,
            };
            physical(binding.locator.path())?;
            let physical_path = fs::canonicalize(binding.locator.path())?;
            // One physical range cannot stand in for two independently named rows.
            ensure!(
                paths.insert((physical_path, offset, binding.stored_length)),
                "aliased row range"
            );
            ensure!(!plain.contains_key(&binding.role), "duplicate row role");
            let stored = read_stored(binding)?;
            plain.insert(binding.role, decode(binding, &stored)?);
        }
        ensure!(
            plain.len() == 3
                && [RowRole::Registry, RowRole::TypeSets, RowRole::HelpProps]
                    .iter()
                    .all(|r| plain.contains_key(r)),
            "missing required row"
        );
        let inputs = Self { manifest, plain };
        inputs.verify_sources()?;
        Ok(inputs)
    }

    pub fn verify_sources(&self) -> Result<()> {
        for predecessor in &self.manifest.predecessors {
            predecessor.verify()?;
        }
        for binding in &self.manifest.rows {
            let _ = read_stored(binding)?;
        }
        Ok(())
    }

    pub fn raw(&self, role: RowRole) -> &[u8] {
        &self.plain[&role]
    }

    pub fn project(&self) -> Result<Coverage> {
        self.verify_sources()?;
        let result = coverage(
            self.raw(RowRole::Registry),
            self.raw(RowRole::TypeSets),
            self.raw(RowRole::HelpProps),
            &self.manifest.registry_root_uuid,
        )?;
        self.verify_sources()?;
        Ok(result)
    }
}

/// Structural validation precedes existing parsers' count arithmetic and UUID maps.
fn registry_preflight(tree: &Brace, declared_root: &str) -> Result<()> {
    let [version, classes, records] = list(tree)? else {
        bail!("registry outer arity");
    };
    ensure!(atom(version)? == "4", "registry version");
    let classes = counted(list(classes)?, 1)?;
    let mut class_ids = BTreeSet::new();
    for class in classes {
        ensure!(
            class_ids.insert(uuid(atom(class)?)?),
            "duplicate registry class"
        );
    }
    let records = counted(list(records)?, 7)?;
    ensure!(!records.is_empty(), "missing registry root");
    let root = uuid(declared_root)?;
    ensure!(root != NIL_UUID, "nil declared root");
    let mut ids = BTreeSet::new();
    let mut open = Vec::<String>::new();
    for (ordinal, record) in records.chunks_exact(7).enumerate() {
        let key = uuid(atom(&record[0])?)?;
        let parent = uuid(atom(&record[1])?)?;
        ensure!(
            key != NIL_UUID && ids.insert(key.clone()),
            "nil/duplicate registry record"
        );
        ensure!(
            count(&record[2])? < classes.len(),
            "registry kind outside class roster"
        );
        ensure!(
            record[3].as_str().is_some() && record[4].as_list().is_some(),
            "registry name/synonym shape"
        );
        let _ = atom(&record[5])?;
        let _ = atom(&record[6])?;
        if ordinal == 0 {
            ensure!(
                key == root && parent == NIL_UUID,
                "registry root binding differs"
            );
        } else {
            ensure!(parent != NIL_UUID && parent != key, "extra root/self owner");
            // Only an open ancestor is a valid owner in the admitted depth-first row.
            // This rejects missing/forward/cyclic owners and a reopened closed subtree.
            while open.last() != Some(&parent) && !open.is_empty() {
                open.pop();
            }
            ensure!(!open.is_empty(), "orphan/forward/nonpreorder owner");
        }
        open.push(key);
    }
    Ok(())
}

fn sets_preflight(tree: &Brace) -> Result<()> {
    let [version, body] = list(tree)? else {
        bail!("type sets outer arity");
    };
    ensure!(atom(version)? == "0", "type sets version");
    let mut keys = BTreeSet::new();
    for pair in counted(list(body)?, 2)?.chunks_exact(2) {
        let key = uuid(atom(&pair[0])?)?;
        ensure!(
            key != NIL_UUID && keys.insert(key),
            "nil/duplicate set declaration"
        );
        let (tag, members) = list(&pair[1])?.split_first().context("empty Pattern")?;
        ensure!(tag.as_str() == Some("Pattern"), "unknown set envelope");
        let mut types = BTreeSet::new();
        for member in members {
            let fields = list(member)?;
            ensure!(!fields.is_empty(), "empty type member");
            if fields[0].as_str() == Some("#") {
                ensure!(fields.len() == 2, "type reference arity");
                let id = uuid(atom(&fields[1])?)?;
                ensure!(
                    id != NIL_UUID && types.insert(id),
                    "nil/duplicate type binding"
                );
            }
            // Primitive/unknown semantic members remain whole Brace values.
            // A1.3 must establish their meaning; coverage is not a type-graph proof.
        }
    }
    Ok(())
}

fn help_preflight(tree: &Brace) -> Result<()> {
    let [version, body] = list(tree)? else {
        bail!("help outer arity");
    };
    ensure!(atom(version)? == "0", "help version");
    let (n, mut rest) = list(body)?.split_first().context("missing help count")?;
    let n = count(n)?;
    ensure!(
        n <= rest.len() / 2,
        "help count exceeds actual entry headers"
    );
    let mut keys = BTreeSet::new();
    for _ in 0..n {
        let [key, properties, tail @ ..] = rest else {
            bail!("truncated help entry");
        };
        ensure!(keys.insert(uuid(atom(key)?)?), "duplicate help key");
        let length = count(properties)?
            .checked_mul(2)
            .context("property count overflow")?;
        ensure!(length <= tail.len(), "property count exceeds actual tail");
        let mut ids = BTreeSet::new();
        for pair in tail[..length].chunks_exact(2) {
            ensure!(ids.insert(count(&pair[0])?), "duplicate property id");
        }
        rest = &tail[length..];
    }
    ensure!(rest.is_empty(), "trailing help entry items");
    Ok(())
}

fn aggregate_set_links(value: &Brace) -> Result<BTreeSet<String>> {
    let [tag, class, body] = list(value)? else {
        bail!("aggregate wrapper arity");
    };
    ensure!(
        tag.as_str() == Some("#") && uuid(atom(class)?)? == SET_LINK_LIST_CLASS,
        "unknown aggregate wrapper"
    );
    let [zero, count_token, members @ ..] = list(body)? else {
        bail!("aggregate list header");
    };
    ensure!(
        atom(zero)? == "0" && count(count_token)? == members.len(),
        "aggregate count/version"
    );
    let mut links = BTreeSet::new();
    for member in members {
        let [tag, class, target] = list(member)? else {
            bail!("set link wrapper arity");
        };
        ensure!(
            tag.as_str() == Some("#") && uuid(atom(class)?)? == facts::METADATA_REF,
            "unknown set link class"
        );
        let [one, id] = list(target)? else {
            bail!("set link target arity");
        };
        ensure!(atom(one)? == "1", "unknown set link target version");
        let id = uuid(atom(id)?)?;
        ensure!(
            id != NIL_UUID && links.insert(id),
            "nil/duplicate aggregate link"
        );
    }
    Ok(links)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOrigin {
    MetadataRecord { record_ordinal: usize },
    TypeSet { set_ordinal: usize },
    NilAggregate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphCompleteness {
    Partial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceStatus {
    NotIdentified,
}

pub struct Coverage {
    pub registry: SiMain,
    pub sets: TypeSets,
    pub help: HelpProps,
    /// Native cache ordinal is retained for comparison, never a traversal seed.
    pub origins: Vec<KeyOrigin>,
    pub pending_atoms: [&'static str; 5],
    pub first_visit_authority: Option<()>,
    pub graph_completeness: GraphCompleteness,
    pub trace_status: TraceStatus,
}

/// Only an independent literal byte comparison: never an expected-order generator.
/// `None` means identical whole byte slices, including BOM, CRLF and EOF.
pub fn first_raw_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(a, b)| a != b)
        .or_else(|| (left.len() != right.len()).then_some(left.len().min(right.len())))
}

/// Also usable by cleanroom controls; no filesystem, planner, sorting, or rendering.
pub fn coverage(
    registry_raw: &[u8],
    sets_raw: &[u8],
    help_raw: &[u8],
    root: &str,
) -> Result<Coverage> {
    registry_preflight(&parse_row(registry_raw)?, root)?;
    sets_preflight(&parse_row(sets_raw)?)?;
    help_preflight(&parse_row(help_raw)?)?;
    let registry = si::parse(registry_raw)?;
    let sets = TypeSets::parse(sets_raw)?;
    let help = HelpProps::parse(help_raw)?;
    let metadata: BTreeMap<_, _> = registry
        .records
        .iter()
        .enumerate()
        .map(|(i, r)| (r.uuid.clone(), i))
        .collect();
    let set_keys: BTreeMap<_, _> = sets
        .sets
        .iter()
        .enumerate()
        .map(|(i, (key, _))| Ok((uuid(key)?, i)))
        .collect::<Result<_>>()?;
    ensure!(
        set_keys.keys().all(|key| !metadata.contains_key(key)),
        "metadata/set declaration collision"
    );
    let mut origins = Vec::new();
    let mut emitted_sets = BTreeSet::new();
    let mut aggregate = None;
    for entry in &help.entries {
        let key = uuid(&entry.key)?;
        let origin = if key == NIL_UUID {
            let value = entry
                .props
                .iter()
                .find(|(id, _)| id.parse::<usize>() == Ok(23))
                .context("nil aggregate lacks property 23")?;
            aggregate = Some(aggregate_set_links(&value.1)?);
            KeyOrigin::NilAggregate
        } else if let Some(&record_ordinal) = metadata.get(&key) {
            KeyOrigin::MetadataRecord { record_ordinal }
        } else if let Some(&set_ordinal) = set_keys.get(&key) {
            ensure!(
                entry
                    .props
                    .iter()
                    .any(|(id, _)| id.parse::<usize>() == Ok(23)),
                "set help entry lacks property 23"
            );
            emitted_sets.insert(key);
            KeyOrigin::TypeSet { set_ordinal }
        } else {
            bail!("unresolved help key {key}");
        };
        origins.push(origin);
    }
    ensure!(
        aggregate.context("missing nil aggregate")? == emitted_sets,
        "nil aggregate is not an exact emitted-set bijection"
    );
    Ok(Coverage {
        registry,
        sets,
        help,
        origins,
        pending_atoms: ["A1.3", "A1.4", "A1.5", "A1.6", "A1.7"],
        first_visit_authority: None,
        graph_completeness: GraphCompleteness::Partial,
        trace_status: TraceStatus::NotIdentified,
    })
}
