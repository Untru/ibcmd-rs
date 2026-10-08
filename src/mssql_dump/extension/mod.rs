//! Rows of a configuration *extension*: what they add to the ordinary
//! configuration row format, and how the platform prints it.
//!
//! An extension stores every adopted (borrowed) object in the ordinary
//! descriptor layout of its kind, but the object's md header carries the
//! adoption state after the comment:
//!
//! ```text
//! {3,{1,0,<uuid>},"Name",<synonym>,"Comment",
//!  <belonging>,<N>,(<property guid>,<state>) x N,<extended object uuid>,
//!  <M>,(<property guid>,<state>,<value>) x M}
//! ```
//!
//! An object of the extension itself, like every object of an ordinary
//! configuration, carries `0,0,<nil uuid>,0` there. `belonging` 1 is an adopted
//! object; the `N` pairs name the properties the extension can override (the
//! platform prints exactly those, beside `Name` and `Comment`), `state` 2 is
//! a property that only records its value and 3 one the extension overrides;
//! the extended object uuid is the base configuration's object when the
//! mapping is not by identity; the `M` triples carry the values of the
//! properties the extension adds to (`MultiState`, type lists).
//!
//! The export works in two steps that keep every ordinary converter blind to
//! extensions: [`normalize_descriptor`] rewrites the adopted tail to the
//! ordinary one and remembers it, the ordinary converters print the object,
//! and [`project_object_xml`] then reduces that print to what the platform
//! writes for an adopted object.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_cf::export::{
    StorageExportDisposition, StorageExportEntryReport, StorageExportPlan, StorageExportReport,
};
use ibcmd_core::storage::StorageImage;
use sha1::{Digest, Sha1};

use super::{
    BinaryConfigRow, DirectStorageExportRecord, StorageImageSourceExportReport,
    config_row_from_binary, export_direct_storage_rows_to_source, inflate_raw_deflate,
};
use crate::cli::InfobaseConfigSourceVersion;
use crate::sql::SqlExec;

mod form;
mod project;
mod properties;
pub(super) mod root;

pub(crate) use project::project_object_xml;

pub(crate) const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// The white space a stored row puts between the elements of a list.
const WHITESPACE: [char; 4] = ['\r', '\n', '\t', ' '];

/// `<xr:State>` values of the stored state codes.
const STATE_RECORDED: u8 = 2;
const STATE_EXTENDED: u8 = 3;

/// Property id every adopted object lists when the platform prints
/// `ExtendedConfigurationObject` for it.
pub(crate) const EXTENDED_OBJECT_PROPERTY: &str = "9595ddd6-e72c-47ad-a156-672db811628c";

/// The adoption state of one md header.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AdoptedHeader {
    /// The object's own uuid (the header's `{1,0,<uuid>}`).
    pub uuid: String,
    /// `(property guid, state)` in stored order.
    pub properties: Vec<(String, u8)>,
    /// The base object the adopted one is mapped to, when it is not the same
    /// uuid.
    pub extended_object: Option<String>,
    /// `(property guid, state, value as stored)` of the added values.
    pub added_values: Vec<(String, u8, String)>,
}

/// A descriptor row with its adoption headers normalized.
#[derive(Debug)]
pub(crate) struct NormalizedDescriptor {
    pub text: String,
    pub adopted: Vec<AdoptedHeader>,
}

/// The two spellings of an md header the platform has used: the `{3,...}` block
/// of every kind since 8.3.24 and the older `{1,...}` block (a language of a
/// 8.3.21 extension). Only the identity tuple and the length of an ordinary
/// tail differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeaderSpelling {
    /// `{3,{1,0,<uuid>},name,synonym,comment,<tail of 4>}`.
    Current,
    /// `{1,{0,0,<uuid>},name,synonym,comment,<tail of 2>}`.
    Older,
}

impl HeaderSpelling {
    fn ordinary_tail(self) -> String {
        match self {
            Self::Current => format!("0,0,{NIL_UUID},0"),
            Self::Older => "0,0".to_owned(),
        }
    }

    fn opening(self) -> &'static str {
        match self {
            Self::Current => "{3,",
            Self::Older => "{1,",
        }
    }
}

/// The next `{1,0,<uuid>}` (preceded by `{3,`) or `{0,0,<uuid>}` (preceded by
/// `{1,`) at or after `from`: the index of its `{`, the spelling.
fn next_identity(text: &str, from: usize) -> Option<(usize, HeaderSpelling)> {
    let mut from = from;
    loop {
        let current = text[from..].find("{1,0,").map(|found| from + found);
        let older = text[from..].find("{0,0,").map(|found| from + found);
        let (start, spelling) = match (current, older) {
            (None, None) => return None,
            (Some(current), None) => (current, HeaderSpelling::Current),
            (None, Some(older)) => (older, HeaderSpelling::Older),
            (Some(current), Some(older)) if current <= older => (current, HeaderSpelling::Current),
            (Some(_), Some(older)) => (older, HeaderSpelling::Older),
        };
        let uuid_start = start + 5;
        let bytes = text.as_bytes();
        let shaped = bytes.get(uuid_start + 36) == Some(&b'}')
            && is_uuid(&text[uuid_start..uuid_start + 36])
            && text[..start]
                .trim_end_matches(WHITESPACE)
                .ends_with(spelling.opening());
        if shaped {
            return Some((start, spelling));
        }
        from = start + 5;
    }
}

/// Every adopted header of `text` rewritten to the ordinary tail of its
/// spelling. Text without an adopted header comes back unchanged.
pub(crate) fn normalize_descriptor(text: &str) -> Result<NormalizedDescriptor> {
    let mut adopted = Vec::new();
    let mut output = String::with_capacity(text.len());
    let mut copied = 0usize;
    let mut search = 0usize;
    while let Some((identity_start, spelling)) = next_identity(text, search) {
        search = identity_start + 5;
        let uuid = &text[identity_start + 5..identity_start + 41];
        let before = text[..identity_start].trim_end_matches(WHITESPACE);
        let open = before.len() - 3;
        let Some(header_end) = scan_braced_end(text, open) else {
            continue;
        };
        let Some(tail_start) = header_tail_start(text, open, header_end) else {
            continue;
        };
        if tail_start < copied {
            continue;
        }
        let tail = &text[tail_start..header_end - 1];
        let fields = split_top_level_fields(tail)?;
        if fields.first().map(|field| field.trim()) == Some("0") {
            // An ordinary header; nothing to remember.
            search = header_end.max(search);
            continue;
        }
        let header = parse_adopted_tail(uuid, &fields, spelling == HeaderSpelling::Older)?;
        output.push_str(&text[copied..tail_start]);
        output.push_str(&spelling.ordinary_tail());
        copied = header_end - 1;
        adopted.push(header);
        search = search.max(header_end);
    }
    if adopted.is_empty() {
        return Ok(NormalizedDescriptor {
            text: text.to_owned(),
            adopted,
        });
    }
    output.push_str(&text[copied..]);
    Ok(NormalizedDescriptor {
        text: output,
        adopted,
    })
}

/// `short_tail`: the older spelling ends after the property entries.
fn parse_adopted_tail(uuid: &str, fields: &[&str], short_tail: bool) -> Result<AdoptedHeader> {
    let field = |index: usize| -> Result<&str> {
        fields
            .get(index)
            .map(|field| field.trim())
            .ok_or_else(|| anyhow!("adopted header {uuid} ends before field {index}"))
    };
    if field(0)? != "1" {
        bail!(
            "header {uuid} has object belonging {:?}; only 0 and 1 are known",
            field(0)?
        );
    }
    let count: usize = field(1)?.parse().map_err(|_| {
        anyhow!(
            "adopted header {uuid} has a bad property count {:?}",
            field(1)
        )
    })?;
    let mut properties = Vec::with_capacity(count);
    let mut index = 2usize;
    for _ in 0..count {
        let guid = field(index)?;
        let state: u8 = field(index + 1)?.parse().map_err(|_| {
            anyhow!(
                "adopted header {uuid} has a bad state {:?} for {guid}",
                field(index + 1)
            )
        })?;
        if !is_uuid(guid) || !matches!(state, STATE_RECORDED | STATE_EXTENDED) {
            bail!("adopted header {uuid} has an unknown property entry {guid},{state}");
        }
        properties.push((guid.to_ascii_lowercase(), state));
        index += 2;
    }
    if short_tail && index == fields.len() {
        return Ok(AdoptedHeader {
            uuid: uuid.to_ascii_lowercase(),
            properties,
            extended_object: None,
            added_values: Vec::new(),
        });
    }
    let extended = field(index)?;
    if !is_uuid(extended) {
        bail!("adopted header {uuid} has a bad extended object {extended:?}");
    }
    index += 1;
    let extended_object = (extended != NIL_UUID).then(|| extended.to_ascii_lowercase());
    let added: usize = field(index)?
        .parse()
        .map_err(|_| anyhow!("adopted header {uuid} has a bad added-value count"))?;
    index += 1;
    let mut added_values = Vec::with_capacity(added);
    for _ in 0..added {
        let guid = field(index)?;
        let state: u8 = field(index + 1)?
            .parse()
            .map_err(|_| anyhow!("adopted header {uuid} has a bad added-value state"))?;
        let value = fields
            .get(index + 2)
            .ok_or_else(|| anyhow!("adopted header {uuid} ends inside an added value"))?
            .trim();
        added_values.push((guid.to_ascii_lowercase(), state, value.to_owned()));
        index += 3;
    }
    if index != fields.len() {
        bail!(
            "adopted header {uuid} has {} trailing fields",
            fields.len() - index
        );
    }
    Ok(AdoptedHeader {
        uuid: uuid.to_ascii_lowercase(),
        properties,
        extended_object,
        added_values,
    })
}

fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

/// The index just past the `}` that closes the braced value opened at `open`.
fn scan_braced_end(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'{') {
        return None;
    }
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = skip_string(bytes, index + 1)?,
            b'{' => {
                depth += 1;
                index += 1;
            }
            b'}' => {
                depth = depth.checked_sub(1)?;
                index += 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => index += 1,
        }
    }
    None
}

/// The index just past the quote that closes a string whose body starts at
/// `from` (`""` is an escaped quote).
fn skip_string(bytes: &[u8], mut from: usize) -> Option<usize> {
    loop {
        let quote = from + bytes.get(from..)?.iter().position(|byte| *byte == b'"')?;
        if bytes.get(quote + 1) == Some(&b'"') {
            from = quote + 2;
        } else {
            return Some(quote + 1);
        }
    }
}

/// Where the tail of the md header at `open..end` starts: after the comment
/// and its comma, i.e. after the fifth top-level field.
fn header_tail_start(text: &str, open: usize, end: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = open + 1;
    for _ in 0..5 {
        while index < end && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        match *bytes.get(index)? {
            b'{' => index = scan_braced_end(text, index)?,
            b'"' => index = skip_string(bytes, index + 1)?,
            _ => {
                while index < end && !matches!(bytes[index], b',' | b'}') {
                    index += 1;
                }
            }
        }
        while index < end && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if bytes.get(index) != Some(&b',') {
            return None;
        }
        index += 1;
    }
    (index < end).then_some(index)
}

/// The top-level, comma separated fields of `tail` (no surrounding braces),
/// untrimmed.
fn split_top_level_fields(tail: &str) -> Result<Vec<&str>> {
    let bytes = tail.as_bytes();
    let mut fields = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                index = skip_string(bytes, index + 1)
                    .ok_or_else(|| anyhow!("unterminated string in an md header tail"))?;
                continue;
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| anyhow!("unbalanced braces in an md header tail"))?
            }
            b',' if depth == 0 => {
                fields.push(&tail[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    fields.push(&tail[start..]);
    Ok(fields)
}

/// Reads the references of the configuration an extension extends, on the
/// first request (see [`ExtensionContext::base_indexes`]).
pub(crate) type BaseIndexProvider = Arc<dyn Fn() -> Option<ResolvedIndexes> + Send + Sync>;

/// What an export of one extension knows about its adopted objects.
#[derive(Default)]
pub(crate) struct ExtensionContext {
    adopted: BTreeMap<String, AdoptedHeader>,
    /// SHA-1 of every storage row's packed bytes, by row name: the digests the
    /// extension's CAS manifest holds and `ConfigDumpInfo.xml` prints.
    packed_sha1: Option<BTreeMap<String, [u8; 20]>>,
    /// The uuid of the md header of the extension's root object: the owner
    /// of the root's module and command-interface rows.
    root_header: Option<String>,
    /// Why the ordinary converters printed nothing for a row, by row name.
    diagnostics: Mutex<BTreeMap<String, String>>,
    /// The references the export resolved: type id -> `cfg:` name, and
    /// object id -> metadata reference. The projection prints values that
    /// were stored as ids with them.
    indexes: OnceLock<ResolvedIndexes>,
    /// The extension's compatibility mode as a packed platform version
    /// (`80324` is 8.3.24): it decides the shape of the forms' XML.
    compatibility: OnceLock<u32>,
    /// Where the references of the configuration the extension extends come
    /// from, and what was read. The platform names the objects a *value* of
    /// the extension points at (an empty reference in a fill value, say)
    /// through the whole configuration, where the extension's own type lists
    /// know nothing of them and stay ids.
    base_provider: Option<BaseIndexProvider>,
    base_indexes: OnceLock<Option<ResolvedIndexes>>,
}

impl std::fmt::Debug for ExtensionContext {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExtensionContext")
            .field("adopted", &self.adopted.len())
            .field("root_header", &self.root_header)
            .field("compatibility", &self.compatibility)
            .finish_non_exhaustive()
    }
}

/// See [`ExtensionContext::note_indexes`].
#[derive(Clone, Debug, Default)]
pub(crate) struct ResolvedIndexes {
    pub type_index: BTreeMap<String, String>,
    pub object_refs: BTreeMap<String, String>,
    /// The compatibility mode of the configuration the indexes were read from
    /// (`Version8_3_27`), when they were read from a whole configuration.
    pub compatibility_mode: Option<String>,
}

impl ExtensionContext {
    pub(crate) fn new(adopted: impl IntoIterator<Item = AdoptedHeader>) -> Self {
        Self {
            adopted: adopted
                .into_iter()
                .map(|header| (header.uuid.clone(), header))
                .collect(),
            packed_sha1: None,
            root_header: None,
            diagnostics: Mutex::default(),
            indexes: OnceLock::new(),
            compatibility: OnceLock::new(),
            base_provider: None,
            base_indexes: OnceLock::new(),
        }
    }

    pub(crate) fn with_base_provider(mut self, provider: Option<BaseIndexProvider>) -> Self {
        self.base_provider = provider;
        self
    }

    /// The references of the extended configuration, read on the first call
    /// (an export that never points at one never reads them).
    pub(crate) fn base_indexes(&self) -> Option<&ResolvedIndexes> {
        self.base_indexes
            .get_or_init(|| self.base_provider.as_ref().and_then(|provider| provider()))
            .as_ref()
    }

    /// Remembers the extension's compatibility mode (once).
    pub(crate) fn note_compatibility(&self, packed_version: u32) {
        let _ = self.compatibility.set(packed_version);
    }

    pub(crate) fn compatibility(&self) -> Option<u32> {
        self.compatibility.get().copied()
    }

    /// Remembers the reference indexes of the export (once).
    pub(crate) fn note_indexes(
        &self,
        type_index: &BTreeMap<String, String>,
        object_refs: &BTreeMap<String, String>,
    ) {
        let _ = self.indexes.set(ResolvedIndexes {
            type_index: type_index.clone(),
            object_refs: object_refs.clone(),
            compatibility_mode: None,
        });
    }

    pub(crate) fn indexes(&self) -> Option<&ResolvedIndexes> {
        self.indexes.get()
    }

    /// The compatibility mode of the configuration the extension extends
    /// (`Version8_3_27`), read with the references of that configuration; none
    /// when they cannot be read. Read before the rows are converted in
    /// parallel (see `export_extension_image_to_source`).
    pub(crate) fn extended_compatibility_mode(&self) -> Option<&str> {
        self.base_indexes()?.compatibility_mode.as_deref()
    }

    /// Remembers why the converters printed nothing for `row`.
    pub(crate) fn note_diagnostic(&self, row: &str, text: String) {
        if let Ok(mut diagnostics) = self.diagnostics.lock() {
            diagnostics.entry(row.to_owned()).or_insert(text);
        }
    }

    fn diagnostic(&self, row: &str) -> Option<String> {
        self.diagnostics.lock().ok()?.get(row).cloned()
    }

    pub(crate) fn with_packed_sha1(mut self, packed_sha1: BTreeMap<String, [u8; 20]>) -> Self {
        self.packed_sha1 = Some(packed_sha1);
        self
    }

    pub(crate) fn with_root_header(mut self, root_header: Option<String>) -> Self {
        self.root_header = root_header;
        self
    }

    /// Whether `owner` is the md header uuid of the extension's root object.
    pub(crate) fn is_root_header(&self, owner: &str) -> bool {
        self.root_header
            .as_deref()
            .is_some_and(|root| root.eq_ignore_ascii_case(owner))
    }

    pub(crate) fn packed_sha1(&self) -> Option<&BTreeMap<String, [u8; 20]>> {
        self.packed_sha1.as_ref()
    }

    pub(crate) fn adopted(&self, uuid: &str) -> Option<&AdoptedHeader> {
        self.adopted.get(&uuid.to_ascii_lowercase())
    }

    pub(crate) fn len(&self) -> usize {
        self.adopted.len()
    }
}

static ACTIVE: RwLock<Option<Arc<ExtensionContext>>> = RwLock::new(None);

/// Keeps a context active for the process; the export is ordinary again once
/// it is dropped.
pub(crate) struct ExtensionGuard {
    _private: (),
}

impl Drop for ExtensionGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = ACTIVE.write() {
            *active = None;
        }
    }
}

pub(crate) fn activate(context: ExtensionContext) -> Result<ExtensionGuard> {
    let mut active = ACTIVE
        .write()
        .map_err(|_| anyhow!("the extension export context is poisoned"))?;
    if active.is_some() {
        bail!("an extension export is already running in this process");
    }
    *active = Some(Arc::new(context));
    Ok(ExtensionGuard { _private: () })
}

/// The context of the extension export running now, if any.
pub(crate) fn active() -> Option<Arc<ExtensionContext>> {
    ACTIVE.read().ok()?.clone()
}

/// Exports one configuration extension's storage image to a source tree.
///
/// The rows go through the ordinary family decoders with their adoption
/// headers normalized ([`normalize_descriptor`]); the objects they print are
/// then reduced to what the platform writes for an adopted object
/// ([`project_object_xml`]) and `ConfigDumpInfo.xml` is written from the CAS
/// digests.
pub fn export_extension_image_to_source(
    image: &StorageImage,
    output_dir: &Path,
    source_version: InfobaseConfigSourceVersion,
    base_provider: Option<BaseIndexProvider>,
) -> Result<StorageImageSourceExportReport> {
    let plan = StorageExportPlan::from_image(image);
    let mut rows = Vec::with_capacity(plan.records().len());
    let mut records = Vec::with_capacity(plan.records().len());
    let mut adopted = Vec::new();
    let mut adopted_rows = BTreeMap::<String, usize>::new();
    let mut packed_sha1 = BTreeMap::new();
    let mut refused = Vec::<StorageExportEntryReport>::new();
    let mut root_header = None::<String>;
    for record in plan.records() {
        let payload = record.packed_payload().with_context(|| {
            format!(
                "failed to materialize storage record `{}`",
                record.logical_key()
            )
        })?;
        packed_sha1.insert(
            record.logical_name().to_owned(),
            <[u8; 20]>::from(Sha1::digest(&payload)),
        );
        let packed_bytes = payload.len();
        let mut bytes = payload.into_owned();
        if !record.logical_name().contains('.') {
            match normalize_row(&bytes) {
                Ok(Some(normalized)) => {
                    adopted_rows.insert(record.logical_name().to_owned(), normalized.adopted.len());
                    if normalized.is_root {
                        root_header = normalized.adopted.first().map(|header| header.uuid.clone());
                    }
                    adopted.extend(normalized.adopted);
                    bytes = normalized.packed;
                }
                Ok(None) => {}
                Err(error) => {
                    refused.push(StorageExportEntryReport::failed_packed(
                        record.logical_name(),
                        record.logical_key(),
                        record.part_count(),
                        packed_bytes,
                        format!("{error:#}"),
                    ));
                    continue;
                }
            }
        }
        let data_size = i64::try_from(bytes.len()).with_context(|| {
            format!(
                "storage record `{}` is too large for the row boundary",
                record.logical_key()
            )
        })?;
        rows.push(config_row_from_binary(BinaryConfigRow {
            file_name: record.logical_name().to_owned(),
            part_no: 0,
            data_size,
            binary: bytes,
        }));
        records.push(DirectStorageExportRecord {
            logical_name: record.logical_name().to_owned(),
            logical_key: record.logical_key().to_owned(),
            part_count: record.part_count(),
            packed_bytes,
        });
    }
    // A refused row is neither exported nor versioned.
    for entry in &refused {
        packed_sha1.remove(&entry.logical_name);
    }
    write_lab_normalized_rows(&rows)?;

    let guard = activate(
        ExtensionContext::new(adopted)
            .with_packed_sha1(packed_sha1)
            .with_root_header(root_header)
            .with_base_provider(base_provider),
    )?;
    // The forms follow the compatibility mode of the configuration the
    // extension extends. Read it here: the read converts rows on the export's
    // own thread pool, so a worker asking for it first would wait on itself.
    if let Some(context) = active() {
        let _ = context.extended_compatibility_mode();
    }
    let exported = export_direct_storage_rows_to_source(
        rows,
        records,
        image
            .source_profile()
            .map(|profile| profile.as_str().to_owned()),
        plan.physical_entries(),
        output_dir,
        false,
        source_version,
        // The extended configuration is resolved through this module's
        // ExtensionContext base provider, not through the .cfe export's
        // foreign references.
        None,
    );
    let context = active();
    drop(guard);
    let mut report = exported?;
    let context = context.ok_or_else(|| anyhow!("the extension export context vanished"))?;
    let mut entries = std::mem::take(&mut report.storage.entries);
    for entry in &mut entries {
        if entry.disposition == StorageExportDisposition::Opaque
            && let Some(reason) = context.diagnostic(&entry.logical_name)
        {
            entry.message = Some(format!(
                "no legacy family decoder recognized this storage entry: {reason}"
            ));
        }
        if entry.disposition == StorageExportDisposition::Supported {
            if let Err(error) = adjust_form_files(output_dir, entry, &context, &plan) {
                fail_outputs(output_dir, entry, error);
                continue;
            }
        }
        if !adopted_rows.contains_key(&entry.logical_name)
            || entry.disposition != StorageExportDisposition::Supported
        {
            continue;
        }
        if let Err(error) = project_outputs(output_dir, entry, &context) {
            fail_outputs(output_dir, entry, error);
        }
    }
    entries.extend(refused);
    report.storage = StorageExportReport::from_parts(
        report.storage.source_profile.clone(),
        report.storage.physical_entries,
        plan.records().len(),
        entries,
    );
    Ok(report)
}

fn fail_outputs(output_dir: &Path, entry: &mut StorageExportEntryReport, error: anyhow::Error) {
    entry.disposition = StorageExportDisposition::Failed;
    entry.message = Some(format!("{error:#}"));
    for output in std::mem::take(&mut entry.outputs) {
        // An approximate form or projection is never left as a successful output.
        let _ = std::fs::remove_file(output_dir.join(output));
    }
}

/// The reference indexes of the configuration the extensions of `database`
/// extend: the type ids and object ids of its metadata rows (the `Config`
/// table, one read of every row without a dot in its name).
pub(crate) fn fetch_base_indexes(
    sql: &SqlExec,
    database: &str,
    source_version: InfobaseConfigSourceVersion,
) -> Result<ResolvedIndexes> {
    let rows = super::fetch::fetch_metadata_rows(sql, database, "Config")
        .with_context(|| format!("failed to read the metadata rows of {database}"))?;
    Ok(base_indexes_from_rows(&rows, source_version))
}

/// The provider of [`fetch_base_indexes`] for an export to hand over: it reads
/// the rows when the export first asks and answers nothing when they cannot be
/// read (a reference to the extended configuration then stays an id).
pub(crate) fn base_index_provider(
    sql: &SqlExec,
    database: &str,
    source_version: InfobaseConfigSourceVersion,
) -> BaseIndexProvider {
    let sql = sql.clone();
    let database = database.to_owned();
    Arc::new(move || fetch_base_indexes(&sql, &database, source_version).ok())
}

/// The indexes of [`fetch_base_indexes`] over metadata rows already read.
pub(crate) fn base_indexes_from_rows(
    rows: &[super::ConfigRow],
    source_version: InfobaseConfigSourceVersion,
) -> ResolvedIndexes {
    let texts = super::build_metadata_text_rows(rows);
    let types = super::build_metadata_type_indexes_from_texts(&texts);
    let objects = super::refs::build_metadata_object_reference_indexes_from_texts(&texts);
    ResolvedIndexes {
        type_index: types.references,
        object_refs: objects.references,
        compatibility_mode: super::refs::configuration_compatibility_mode_from_texts(
            &texts,
            source_version,
        ),
    }
}

/// Lab aid: `IBCMD_RS_EXTENSION_NORMALIZED_ROWS_OUT=<dir>` writes the rows the
/// converters receive (adoption headers normalized) in the `--rows-dir` layout.
fn write_lab_normalized_rows(rows: &[super::ConfigRow]) -> Result<()> {
    let Some(dir) = std::env::var_os("IBCMD_RS_EXTENSION_NORMALIZED_ROWS_OUT")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
    else {
        return Ok(());
    };
    std::fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    for row in rows {
        let bytes = row.binary_bytes()?;
        let path = dir.join(format!("{}__part0.bin", row.file_name));
        std::fs::write(&path, bytes.as_ref())
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

/// A one-line account of a converter diagnostic (tokens only, no payload).
pub(crate) fn describe_diagnostic(
    diagnostic: &super::MetadataSourceExtractionDiagnostic,
) -> String {
    let mut text = format!(
        "{} (family {}, stage {}, signature {}",
        diagnostic.code,
        diagnostic.family,
        diagnostic.parser_stage,
        diagnostic.structural_signature
    );
    for (name, value) in [
        ("field", diagnostic.field_index),
        ("collection", diagnostic.collection_index),
        ("item", diagnostic.item_index),
    ] {
        if let Some(value) = value {
            text.push_str(&format!(", {name} {value}"));
        }
    }
    if let Some(role) = &diagnostic.collection_role {
        text.push_str(&format!(", role {role}"));
    }
    text.push(')');
    text
}

/// The normalized packed row of a descriptor that holds an adopted header.
struct NormalizedRow {
    packed: Vec<u8>,
    adopted: Vec<AdoptedHeader>,
    /// The row is the root `Configuration` record (`{2,...}`).
    is_root: bool,
}

fn normalize_row(packed: &[u8]) -> Result<Option<NormalizedRow>> {
    let Ok(plain) = inflate_raw_deflate(packed) else {
        return Ok(None);
    };
    let Ok(text) = String::from_utf8(plain) else {
        return Ok(None);
    };
    let (bom, body) = match text.strip_prefix('\u{feff}') {
        Some(rest) => ("\u{feff}", rest),
        None => ("", text.as_str()),
    };
    let normalized = normalize_descriptor(body)?;
    if normalized.adopted.is_empty() {
        return Ok(None);
    }
    let is_root = normalized.text.trim_start().starts_with("{2,");
    let packed = crate::module_blob::deflate_raw(format!("{bom}{}", normalized.text).as_bytes())?;
    Ok(Some(NormalizedRow {
        packed,
        adopted: normalized.adopted,
        is_root,
    }))
}

/// One object an extension adopts, as its descriptor row says.
///
/// The platform pairs an adopted object with the object it extends by identity
/// (the kind and the name), and only in the other cases names the base object
/// in the header: 67 of the 84 adopted headers of the БСП extension
/// `_ДемоРасширение` carry the nil uuid there, and the object has a uuid of its
/// own (`Catalog._ДемоПартнеры` is `3014d9c1-...` in the extension and
/// `5eab8a1b-...` in the configuration).
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct AdoptedObject {
    /// The descriptor row that holds the object (a bare uuid).
    pub row: String,
    /// The object's own uuid in the extension.
    pub uuid: String,
    /// The object's name.
    pub name: String,
    /// The uuid of the object it extends, when the header names it.
    pub extends: Option<String>,
}

/// The objects an extension image adopts: for every descriptor row named by the
/// uuid of its own first header, that header, when it is an adopted one.
/// Children (attributes, sections) sit in the same rows and are not listed.
pub fn adopted_objects(image: &StorageImage) -> Result<Vec<AdoptedObject>> {
    let plan = StorageExportPlan::from_image(image);
    let mut objects = Vec::new();
    for record in plan.records() {
        if record.logical_name().contains('.') {
            continue;
        }
        let payload = record.packed_payload().with_context(|| {
            format!(
                "failed to materialize storage record `{}`",
                record.logical_key()
            )
        })?;
        let Ok(plain) = inflate_raw_deflate(&payload) else {
            continue;
        };
        let Ok(text) = String::from_utf8(plain) else {
            continue;
        };
        let body = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let normalized = normalize_descriptor(body)
            .with_context(|| format!("descriptor row {}", record.logical_name()))?;
        let Some(first) = normalized.adopted.first() else {
            continue;
        };
        if !first.uuid.eq_ignore_ascii_case(record.logical_name()) {
            continue;
        }
        // A header no reader of ours can name is listed without a name: it is
        // still found by its uuids.
        let name = header_name(body, &first.uuid).unwrap_or_default();
        objects.push(AdoptedObject {
            row: record.logical_name().to_owned(),
            uuid: first.uuid.clone(),
            name,
            extends: first.extended_object.clone(),
        });
    }
    Ok(objects)
}

/// The quoted name that follows the identity `{1,0,<uuid>}` of a header.
fn header_name(text: &str, uuid: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let uuid = uuid.to_ascii_lowercase();
    let at = ["{1,0,", "{0,0,"].iter().find_map(|opening| {
        let identity = format!("{opening}{uuid}}}");
        lower.find(&identity).map(|at| at + identity.len())
    })?;
    let rest = text[at..].trim_start_matches(|c| c == ',' || WHITESPACE.contains(&c));
    let rest = rest.strip_prefix('"')?;
    let mut name = String::new();
    let mut chars = rest.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if chars.peek() == Some(&'"') {
                chars.next();
                name.push('"');
                continue;
            }
            return Some(name);
        }
        name.push(c);
    }
    None
}

/// The first packed platform version whose forms declare the
/// data-composition-schema namespace at the root (`dcssch`). Evidence brackets
/// it: an extension in the 8.3.14 compatibility mode writes none, the modes
/// 8.3.21 and 8.3.24 write it. Where between the two the boundary lies is not
/// on record; 8.3.15 is assumed.
const FIRST_COMPATIBILITY_WITH_SCHEMA_NAMESPACE: u32 = 80315;

const SCHEMA_NAMESPACE_DECLARATION: &str =
    r#" xmlns:dcssch="http://v8.1c.ru/8.1/data-composition-system/schema""#;

/// What the platform does to the form XML of an extension beyond what the
/// stored body states: the older shape of a form in an old compatibility mode
/// (no schema namespace on the root of `Ext/Form.xml`) and the completion of
/// items saved by an older platform (see [`form`]), and the call types of an
/// adopted form that has no base form record. The base form of the others and
/// their call types are written with the form itself
/// (`form_extension::form_adoption`).
fn adjust_form_files(
    output_dir: &Path,
    entry: &StorageExportEntryReport,
    context: &ExtensionContext,
    plan: &StorageExportPlan,
) -> Result<()> {
    let adopted = entry
        .logical_name
        .strip_suffix(".0")
        .is_some_and(|uuid| context.adopted(uuid).is_some());
    let old_root = context
        .compatibility()
        .is_some_and(|compatibility| compatibility < FIRST_COMPATIBILITY_WITH_SCHEMA_NAMESPACE);
    for output in &entry.outputs {
        let normalized = output.replace('\\', "/");
        if !normalized.ends_with("/Ext/Form.xml") {
            continue;
        }
        let path = output_dir.join(output);
        let bytes = std::fs::read(&path).with_context(|| format!("failed to read {output}"))?;
        let text = String::from_utf8(bytes).with_context(|| format!("{output} is not UTF-8"))?;
        let mut adjusted = text.clone();
        if let Some(upgraded) = form::upgrade_items(&adjusted) {
            adjusted = upgraded;
        }
        if adopted {
            let record = plan
                .records()
                .iter()
                .find(|record| record.logical_name() == entry.logical_name)
                .context("the adopted form's storage body is missing")?;
            let packed = record.packed_payload()?;
            let body = crate::module_blob::parse_form_body_blob(&packed)
                .context("the adopted form's storage body is not readable")?;
            if let Some(called) = form::add_call_types(&adjusted, &body.layout)? {
                adjusted = called;
            }
        }
        if old_root && !adjusted.contains("dcssch:") {
            adjusted = without_schema_namespace(&adjusted);
        }
        if adjusted != text {
            std::fs::write(&path, adjusted.as_bytes())
                .with_context(|| format!("failed to write {output}"))?;
        }
    }
    Ok(())
}

/// `text` without the schema namespace declaration on the `<Form>` root.
fn without_schema_namespace(text: &str) -> String {
    let Some(root_start) = text.find("<Form ") else {
        return text.to_owned();
    };
    let Some(root_length) = text[root_start..].find('>') else {
        return text.to_owned();
    };
    let root_end = root_start + root_length;
    let root = &text[root_start..root_end];
    format!(
        "{}{}{}",
        &text[..root_start],
        root.replacen(SCHEMA_NAMESPACE_DECLARATION, "", 1),
        &text[root_end..]
    )
}

/// Rewrites the object XML files one storage row produced.
fn project_outputs(
    output_dir: &Path,
    entry: &StorageExportEntryReport,
    context: &ExtensionContext,
) -> Result<()> {
    for output in &entry.outputs {
        if !output.ends_with(".xml") {
            continue;
        }
        let path = output_dir.join(output);
        let bytes = std::fs::read(&path).with_context(|| format!("failed to read {output}"))?;
        let text = String::from_utf8(bytes).with_context(|| format!("{output} is not UTF-8"))?;
        if !text.contains("<MetaDataObject") {
            continue;
        }
        let projected = project_object_xml(&text, context)
            .with_context(|| format!("failed to project {output}"))?;
        if projected != text {
            std::fs::write(&path, projected.as_bytes())
                .with_context(|| format!("failed to write {output}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADOPTED_MODULE: &str = "{1,\r\n{12,\r\n{3,\r\n{1,0,eb50ccde-ac43-46b8-a693-56b559ca323a},\"Name\",\r\n{0},\"\",1,3,9595ddd6-e72c-47ad-a156-672db811628c,2,d5963243-262e-4398-b4d7-fb16d06484f6,3,c474bab9-d13a-4fbd-bfb0-9214d6dc2fde,2,640d7486-8abd-40aa-a244-2ed899b7225a,0},1,1,1,0,0,0,0,0},0}";

    #[test]
    fn the_schema_namespace_leaves_only_the_root() {
        let text = concat!(
            "<?xml version=\"1.0\"?>\r\n<Form xmlns=\"a\" xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\" version=\"2.20\">\r\n",
            "\t<Item xmlns:dcssch=\"http://v8.1c.ru/8.1/data-composition-system/schema\"/>\r\n</Form>"
        );
        let stripped = without_schema_namespace(text);
        assert!(stripped.contains("<Form xmlns=\"a\" version=\"2.20\">"));
        assert!(stripped.contains("<Item xmlns:dcssch="));
    }

    #[test]
    fn an_adopted_tail_becomes_the_ordinary_one() {
        let normalized = normalize_descriptor(ADOPTED_MODULE).unwrap();
        assert_eq!(normalized.adopted.len(), 1);
        let header = &normalized.adopted[0];
        assert_eq!(header.uuid, "eb50ccde-ac43-46b8-a693-56b559ca323a");
        assert_eq!(
            header.properties,
            vec![
                (EXTENDED_OBJECT_PROPERTY.to_owned(), 2),
                ("d5963243-262e-4398-b4d7-fb16d06484f6".to_owned(), 3),
                ("c474bab9-d13a-4fbd-bfb0-9214d6dc2fde".to_owned(), 2),
            ]
        );
        assert_eq!(
            header.extended_object.as_deref(),
            Some("640d7486-8abd-40aa-a244-2ed899b7225a")
        );
        assert!(header.added_values.is_empty());
        assert_eq!(
            normalized.text,
            "{1,\r\n{12,\r\n{3,\r\n{1,0,eb50ccde-ac43-46b8-a693-56b559ca323a},\"Name\",\r\n{0},\"\",0,0,00000000-0000-0000-0000-000000000000,0},1,1,1,0,0,0,0,0},0}"
        );
    }

    #[test]
    fn an_ordinary_header_is_left_alone() {
        let text = "{1,{12,{3,{1,0,eb50ccde-ac43-46b8-a693-56b559ca323a},\"N\",{0},\"\",0,0,00000000-0000-0000-0000-000000000000,0},1}}";
        let normalized = normalize_descriptor(text).unwrap();
        assert!(normalized.adopted.is_empty());
        assert_eq!(normalized.text, text);
    }

    #[test]
    fn added_values_and_nested_headers_are_read() {
        let text = concat!(
            "{1,{2,{3,{1,0,98b71d7d-a47a-4538-aa66-3201fccbf890},\"C, \"\"q\"\"\",{0},\"a,b\",",
            "1,2,9595ddd6-e72c-47ad-a156-672db811628c,2,7d14f63a-87e8-4188-a28b-02da93f6bcbd,3,",
            "41f87891-ddd5-4e0f-a4f2-8039fb71981b,1,7d14f63a-87e8-4188-a28b-02da93f6bcbd,3,{\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d,{\"Pattern\",{\"#\",2064b508-2456-409c-b230-603228757b20}}}},",
            "{3,{1,0,523d19b5-0dd0-4377-9e32-3b28b327d814},\"A\",{0},\"\",1,1,b1053250-abe6-11d4-9434-004095e12fc7,2,00000000-0000-0000-0000-000000000000,0}}}"
        );
        let normalized = normalize_descriptor(text).unwrap();
        assert_eq!(normalized.adopted.len(), 2);
        assert_eq!(normalized.adopted[0].added_values.len(), 1);
        assert_eq!(
            normalized.adopted[0].added_values[0].2,
            "{\"#\",f5c65050-3bbb-11d5-b988-0050bae0a95d,{\"Pattern\",{\"#\",2064b508-2456-409c-b230-603228757b20}}}"
        );
        assert_eq!(normalized.adopted[1].extended_object, None);
        assert!(!normalized.text.contains("b1053250"));
        assert_eq!(
            normalized
                .text
                .matches(",0,0,00000000-0000-0000-0000-000000000000,0}")
                .count(),
            2
        );
    }

    #[test]
    fn an_unknown_belonging_is_refused() {
        let text = "{1,{3,{1,0,eb50ccde-ac43-46b8-a693-56b559ca323a},\"N\",{0},\"\",5,0,00000000-0000-0000-0000-000000000000,0}}";
        assert!(normalize_descriptor(text).is_err());
    }

    /// The extension context is process-wide, and the ordinary converters read
    /// it: a test that runs an extension export beside the other tests of this
    /// process would turn some of them into extension conversions. Such a test
    /// runs again alone in a child process (`test` is its name in the crate);
    /// this returns `false` in the parent, whose verdict is the child's, and
    /// `true` where the body is to run.
    fn alone(test: &str) -> bool {
        const CHILD: &str = "IBCMD_RS_ISOLATED_EXTENSION_TEST";
        if std::env::var_os(CHILD).is_some() {
            return true;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--test-threads=1", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("1 passed"),
            "{test} failed alone:
{stdout}
{}",
            String::from_utf8_lossy(&output.stderr)
        );
        false
    }

    #[test]
    fn active_unrelated_context_keeps_ordinary_v76_addressed_refusals() {
        if !alone(
            "mssql_dump::extension::tests::active_unrelated_context_keeps_ordinary_v76_addressed_refusals",
        ) {
            return;
        }
        use crate::metadata_model::brace::Brace;
        use crate::mssql_dump::MetadataSourceFailureClass;
        use crate::mssql_dump::configuration_interface_pair_tests::{fixture, physical, tuple_mut};

        let (original, context) = fixture("2.21", "Taxi", true);
        assert!(physical(&original, &context, InfobaseConfigSourceVersion::V2_21).is_ok());
        let active = activate(ExtensionContext::new(std::iter::empty())).unwrap();
        assert!(super::active().is_some());
        let failure = physical(&original, &context, InfobaseConfigSourceVersion::V2_20)
            .err()
            .unwrap();
        assert_eq!(failure.class, MetadataSourceFailureClass::Unsupported);
        assert_eq!(failure.structural_signature, "v76_root_requires_xml_2_21");
        let mut malformed = original.clone();
        tuple_mut(&mut malformed)[38] = Brace::num(0);
        tuple_mut(&mut malformed)[62] = Brace::num(6);
        for version in [
            InfobaseConfigSourceVersion::V2_20,
            InfobaseConfigSourceVersion::V2_21,
        ] {
            let failure = physical(&malformed, &context, version).err().unwrap();
            assert_eq!(failure.class, MetadataSourceFailureClass::Malformed);
            assert_eq!(failure.structural_signature, "unknown_v76_interface_pair");
        }
        drop(active);
        assert!(super::active().is_none());
        assert!(physical(&original, &context, InfobaseConfigSourceVersion::V2_21).is_ok());
    }

    /// The extension `_ДемоПустоеРасширение` of the БСП 8.3.27 and the БСП 8.5
    /// demonstration bases: the same two rows in both, and the tree each
    /// platform's own `config export --extension` wrote for them
    /// (`tests/fixtures/native-evidence/extension-empty/`). 8.5 prints two
    /// empty captions after the version; the tree is otherwise the same.
    #[test]
    fn the_empty_extension_exports_to_the_native_tree_of_both_platforms() {
        if !alone(
            "mssql_dump::extension::tests::the_empty_extension_exports_to_the_native_tree_of_both_platforms",
        ) {
            return;
        }
        use crate::mssql_dump::cas::{CasHash, CasStorageRow, resolve_cas_storage_image};
        use crate::mssql_extension_stage::{
            ConfigInfoIdentity, ExtensionStageRow, generate_configinfo_manifest,
        };
        use flate2::{Compression, write::DeflateEncoder};
        use std::io::Write as _;

        // One export at a time: the extension context is process-wide.
        static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
        let _guard = ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(|error| error.into_inner());

        const ROOT: &str = "6a5f6dac-3040-4142-b853-482f745e1e4b";
        const LANGUAGE: &str = "efc14f05-e4ab-4235-8c63-9082143c3ca9";
        let rows: [(&str, &[u8]); 2] = [
            (
                ROOT,
                include_bytes!(
                    "../../../tests/fixtures/native-evidence/extension-empty/rows/6a5f6dac-3040-4142-b853-482f745e1e4b.bin"
                ),
            ),
            (
                LANGUAGE,
                include_bytes!(
                    "../../../tests/fixtures/native-evidence/extension-empty/rows/efc14f05-e4ab-4235-8c63-9082143c3ca9.bin"
                ),
            ),
        ];
        let identity = ConfigInfoIdentity {
            storage_format: 80_324,
            configuration_id: uuid::Uuid::parse_str(ROOT).unwrap(),
            descriptor: Vec::new(),
        };
        let stage = rows
            .iter()
            .map(|(name, packed)| ExtensionStageRow::new(*name, packed.to_vec()))
            .collect::<Vec<_>>();
        let manifest = generate_configinfo_manifest(&identity, &stage).unwrap();
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&manifest).unwrap();
        let packed_manifest = encoder.finish().unwrap();
        let root_hash = CasHash::for_packed_bytes(&packed_manifest);
        let mut cas = vec![CasStorageRow::new(root_hash, packed_manifest)];
        cas.extend(rows.iter().map(|(_, packed)| {
            CasStorageRow::new(CasHash::for_packed_bytes(packed), packed.to_vec())
        }));
        let image = resolve_cas_storage_image(root_hash, cas).unwrap();

        let native_8_3_27: [(&str, &[u8]); 3] = [
            (
                "Configuration.xml",
                include_bytes!(
                    "../../../tests/fixtures/native-evidence/extension-empty/native-8.3.27/Configuration.xml"
                ),
            ),
            (
                "ConfigDumpInfo.xml",
                include_bytes!(
                    "../../../tests/fixtures/native-evidence/extension-empty/native-8.3.27/ConfigDumpInfo.xml"
                ),
            ),
            (
                "Languages/Русский.xml",
                include_bytes!(
                    "../../../tests/fixtures/native-evidence/extension-empty/native-8.3.27/Language.xml"
                ),
            ),
        ];
        let native_8_5_1: [(&str, &[u8]); 3] = [
            (
                "Configuration.xml",
                include_bytes!(
                    "../../../tests/fixtures/native-evidence/extension-empty/native-8.5.1/Configuration.xml"
                ),
            ),
            (
                "ConfigDumpInfo.xml",
                include_bytes!(
                    "../../../tests/fixtures/native-evidence/extension-empty/native-8.5.1/ConfigDumpInfo.xml"
                ),
            ),
            (
                "Languages/Русский.xml",
                include_bytes!(
                    "../../../tests/fixtures/native-evidence/extension-empty/native-8.5.1/Language.xml"
                ),
            ),
        ];
        for (version, native) in [
            (InfobaseConfigSourceVersion::V2_20, native_8_3_27),
            (InfobaseConfigSourceVersion::V2_21, native_8_5_1),
        ] {
            let out = std::env::temp_dir().join(format!(
                "ibcmd-extension-empty-{}-{}",
                std::process::id(),
                version.as_str()
            ));
            let _ = std::fs::remove_dir_all(&out);
            let report = export_extension_image_to_source(&image, &out, version, None).unwrap();
            assert_eq!((report.storage.failed, report.storage.opaque), (0, 0));
            for (path, expected) in native {
                let written = std::fs::read(out.join(path)).unwrap_or_default();
                assert!(
                    written == expected,
                    "{path} of the {} tree differs from the native one",
                    version.as_str()
                );
            }
            let _ = std::fs::remove_dir_all(&out);
        }
    }

    /// The objects an extension adopts are read off the descriptor rows without
    /// an export: the upstream case `adopted/catalog_modules` adopts one
    /// catalog, mapped to the configuration's by uuid; the empty extension
    /// adopts nothing.
    #[test]
    fn the_objects_an_extension_adopts_are_listed_off_its_rows() {
        let root = std::env::var_os("IBCMD_UPSTREAM_FIXTURES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external")
            });
        let dir = root.join("adopted/catalog_modules");
        if !dir.is_dir() {
            eprintln!(
                "upstream extension fixtures are not present at {}",
                root.display()
            );
            return;
        }
        let image = upstream_image(&dir).unwrap();
        let objects = adopted_objects(&image).unwrap();
        let catalog = objects
            .iter()
            .find(|object| object.name == "Справочник")
            .unwrap_or_else(|| panic!("the catalog is not listed: {objects:?}"));
        assert_eq!(catalog.uuid, "e0000000-0000-4000-8000-000000000002");
        assert_eq!(
            catalog.extends.as_deref(),
            Some("b0000000-0000-4000-8000-000000000002")
        );
        assert_eq!(catalog.row, catalog.uuid);
    }

    #[test]
    fn a_header_name_is_read_with_its_quotes() {
        let text =
            "{3, {1,0,0B3FA0EF-9968-11F1-8F4F-00E04C680093}, \"Имя \"\"в кавычках\"\"\", {0}";
        assert_eq!(
            header_name(text, "0b3fa0ef-9968-11f1-8f4f-00e04c680093").as_deref(),
            Some("Имя \"в кавычках\"")
        );
        assert_eq!(
            header_name(text, "00000000-0000-0000-0000-000000000000"),
            None
        );
        // the older spelling of a language
        let older = "{1, {0,0,0b3fa0ef-9968-11f1-8f4f-00e04c680093},\"Русский\",{0}";
        assert_eq!(
            header_name(older, "0b3fa0ef-9968-11f1-8f4f-00e04c680093").as_deref(),
            Some("Русский")
        );
    }

    /// Platform-made fixtures of upstream PR 387 (`tests/fixtures/external`,
    /// present once that PR is merged, or named by `IBCMD_UPSTREAM_FIXTURES`):
    /// an extension as a `.cfe` container beside the tree the 8.3.27.2214
    /// platform dumped for it. These are the cases the extension export equals
    /// byte for byte. Each was set apart by a probe that changes one value, so
    /// they tell apart what the two BSP extensions of the first corpus could
    /// not (the code type and length ids of a catalog, the write mode and
    /// periodicity of a register, the default roles and the managed
    /// application module of the root, the members 3, 21 and 49 of the root
    /// tuple).
    const UPSTREAM_MATCHING_CASES: [&str; 22] = [
        "extension_roots/values",
        "extension_roots/spellings",
        "extension_roots/modules",
        "extension_roots/roles",
        "extension_roots/values_v85",
        "adopted/props_all",
        "adopted/props_b0",
        "adopted/props_b1",
        "adopted/props_b2",
        "adopted/module_all",
        "adopted/module_b0",
        "adopted/module_b1",
        "adopted/module_b2",
        "adopted/catalog_modules",
        "adopted/catalog_object_module",
        "adopted/document_children",
        "adopted/form_events",
        "adopted/form_events_shared",
        "adopted/role",
        "adopted/subscription",
        "adopted/kinds",
        "adopted/foreign_links",
    ];

    /// The other upstream cases: each fails closed or differs, and is listed in
    /// `docs/extensions/parity.md` (predefined items and exchange plan content
    /// of an adopted object, widened types with a check value, and a container
    /// whose CAS content is missing).
    const UPSTREAM_OPEN_CASES: [&str; 4] = [
        "adopted/predefined",
        "adopted/exchange_plan",
        "adopted/widened",
        "extension_roots/unknown_property",
    ];

    /// The storage image of the `.cfe` container of an upstream case.
    fn upstream_image(dir: &std::path::Path) -> Result<StorageImage, String> {
        use crate::mssql_dump::cas::{CasHash, CasStorageRow, resolve_cas_storage_image};
        use ibcmd_core::artifact::StorageProfileId;
        use ibcmd_core::limits::ResourceLimits;

        let cfe = std::fs::read(dir.join("input.cfe")).map_err(|error| error.to_string())?;
        let archive = ibcmd_cf::archive::decode_packed_archive(
            std::io::Cursor::new(cfe),
            ResourceLimits::default(),
            StorageProfileId::parse("storage:cfe").unwrap(),
        )
        .map_err(|error| error.to_string())?;
        let mut root_hash = None;
        let mut rows = Vec::new();
        for entry in archive.entries() {
            let packed = entry.payload().to_vec();
            let hash = CasHash::for_packed_bytes(&packed);
            if entry.name() == "configinfo" {
                root_hash = Some(hash);
            }
            rows.push(CasStorageRow::new(hash, packed));
        }
        let root_hash = root_hash.ok_or("the container has no configinfo")?;
        resolve_cas_storage_image(root_hash, rows).map_err(|error| error.to_string())
    }

    /// The files of the case that our export does not reproduce (an empty list
    /// when the case matches), or a reason the case could not be run.
    fn upstream_case_differences(
        root: &std::path::Path,
        case: &str,
    ) -> Result<Vec<String>, String> {
        let dir = root.join(case);
        let image = upstream_image(&dir)?;

        // One export at a time: the extension context is process-wide.
        static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
        let _guard = ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let out = std::env::temp_dir().join(format!(
            "ibcmd-upstream-{}-{}",
            std::process::id(),
            case.replace('/', "-")
        ));
        let _ = std::fs::remove_dir_all(&out);
        export_extension_image_to_source(&image, &out, InfobaseConfigSourceVersion::V2_20, None)
            .map_err(|error| format!("{error:#}"))?;

        // The dump info compares with its version stamps blanked, as elsewhere.
        fn blanked(bytes: &[u8]) -> String {
            let text = String::from_utf8_lossy(bytes);
            let mut result = String::new();
            let mut rest: &str = &text;
            while let Some(at) = rest.find("configVersion=\"") {
                result.push_str(&rest[..at + 15]);
                rest = &rest[at + 15..];
                rest = &rest[rest.find('"').unwrap_or(0)..];
            }
            result.push_str(rest);
            result
        }
        let mut differing = Vec::new();
        let mut stack = vec![dir.clone()];
        while let Some(current) = stack.pop() {
            for entry in std::fs::read_dir(&current).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let relative = path.strip_prefix(&dir).unwrap().to_owned();
                if relative == std::path::Path::new("input.cfe") {
                    continue;
                }
                let expected = std::fs::read(&path).unwrap();
                let ours = std::fs::read(out.join(&relative)).ok();
                let same = match &ours {
                    Some(ours) if relative.ends_with("ConfigDumpInfo.xml") => {
                        blanked(ours) == blanked(&expected)
                    }
                    Some(ours) => *ours == expected,
                    None => false,
                };
                if !same {
                    differing.push(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&out);
        differing.sort();
        Ok(differing)
    }

    #[test]
    fn the_export_equals_the_platform_dumps_of_the_upstream_fixtures() {
        if !alone(
            "mssql_dump::extension::tests::the_export_equals_the_platform_dumps_of_the_upstream_fixtures",
        ) {
            return;
        }
        let root = std::env::var_os("IBCMD_UPSTREAM_FIXTURES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external")
            });
        if !root.join("adopted").is_dir() {
            eprintln!(
                "upstream extension fixtures are not present at {}",
                root.display()
            );
            return;
        }
        let mut failures = Vec::new();
        for case in UPSTREAM_MATCHING_CASES {
            match upstream_case_differences(&root, case) {
                Ok(differing) if differing.is_empty() => {}
                Ok(differing) => failures.push(format!("{case}: {differing:?}")),
                Err(reason) => failures.push(format!("{case}: {reason}")),
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        for case in UPSTREAM_OPEN_CASES {
            eprintln!(
                "open upstream case {case}: {:?}",
                upstream_case_differences(&root, case)
            );
        }
    }
}
