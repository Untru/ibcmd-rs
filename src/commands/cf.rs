//! Offline `cf inspect` and `cf verify` commands.
//!
//! The command layer opens files directly, relies on the bounded streaming V8
//! reader, and decodes only selected payloads. It never probes `PATH`, starts a
//! 1C process, or guesses payload compression from bytes or names.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    fs::File,
    path::{Path, PathBuf},
};

use ibcmd_cf::payload::{PayloadDecoder, PayloadEncoding};
use ibcmd_core::{artifact::StorageProfileId, limits::ResourceLimits, storage::Sha256Digest};
use ibcmd_v8::{
    format::Revision,
    reader::{ContainerIndex, EntryIndex, StreamingReader},
};
use serde::Serialize;

use crate::cli::{CfArgs, CfCommands, CfCompression, CfInspectArgs, CfVerifyArgs};

const REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct CfReport {
    pub schema_version: u32,
    pub command: &'static str,
    pub ok: bool,
    pub input: String,
    pub profile: CfProfileReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<CfLayoutReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection: Option<CfSelectionReport>,
    pub elements: Vec<CfElementReport>,
    pub errors: Vec<CfDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CfProfileReport {
    pub id: String,
    pub compression: &'static str,
    pub compression_source: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct CfLayoutReport {
    pub revision: &'static str,
    pub base_offset: u64,
    pub stream_length: u64,
    pub preamble_bytes: u64,
    pub page_size: Option<u32>,
    pub storage_version: u32,
    pub reserved: Option<u32>,
    pub element_count: usize,
    pub indexed_pages: usize,
    pub encoded_payload_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CfSelectionReport {
    pub requested: Vec<String>,
    pub selected_count: usize,
    pub archive_element_count: usize,
    pub list_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CfElementReport {
    pub index: usize,
    pub name: String,
    pub data_state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<&'static str>,
    pub header_bytes: u64,
    pub header_pages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packed_bytes: Option<u64>,
    pub data_pages: usize,
    pub payload_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpacked_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packed_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpacked_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CfDiagnostic {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual: Option<String>,
}

#[derive(Debug)]
pub struct CfCommandError {
    report: Box<CfReport>,
}

impl CfCommandError {
    #[must_use]
    pub const fn report(&self) -> &CfReport {
        &self.report
    }
}

impl fmt::Display for CfCommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.report.errors.first() {
            Some(error) => formatter.write_str(&error.message),
            None => formatter.write_str("CF command failed"),
        }
    }
}

impl Error for CfCommandError {}

#[derive(Debug)]
struct RunOptions {
    command: &'static str,
    input: PathBuf,
    profile: String,
    compression: CfCompression,
    requested: Vec<String>,
    list_only: bool,
    expected_sha256: Vec<String>,
}

/// Executes one offline CF command and returns a stable JSON-serializable report.
pub fn run(args: CfArgs) -> Result<CfReport, CfCommandError> {
    let options = match args.command {
        CfCommands::Inspect(args) => inspect_options(args),
        CfCommands::Verify(args) => verify_options(args),
    };
    execute(options)
}

fn inspect_options(args: CfInspectArgs) -> RunOptions {
    RunOptions {
        command: "inspect",
        input: args.input,
        profile: args.profile,
        compression: args.compression,
        requested: args.elements,
        list_only: false,
        expected_sha256: Vec::new(),
    }
}

fn verify_options(args: CfVerifyArgs) -> RunOptions {
    RunOptions {
        command: "verify",
        input: args.input,
        profile: args.profile,
        compression: args.compression,
        requested: args.elements,
        list_only: args.list_only,
        expected_sha256: args.expected_sha256,
    }
}

fn execute(options: RunOptions) -> Result<CfReport, CfCommandError> {
    let profile = profile_report(&options);
    if let Err(source) = StorageProfileId::parse(&options.profile) {
        return Err(failure(
            &options,
            profile,
            None,
            None,
            Vec::new(),
            diagnostic(
                "invalid_profile",
                format!("invalid storage profile `{}`: {source}", options.profile),
            ),
        ));
    }

    let expected = parse_expectations(&options)
        .map_err(|error| failure(&options, profile.clone(), None, None, Vec::new(), error))?;
    if options.list_only && !expected.is_empty() {
        return Err(failure(
            &options,
            profile,
            None,
            None,
            Vec::new(),
            diagnostic(
                "invalid_options",
                "--list-only cannot be combined with --expect-sha256".to_owned(),
            ),
        ));
    }

    let source = File::open(&options.input).map_err(|source| {
        failure(
            &options,
            profile.clone(),
            None,
            None,
            Vec::new(),
            diagnostic(
                "open_failed",
                format!("failed to open `{}`: {source}", options.input.display()),
            ),
        )
    })?;
    let mut reader =
        StreamingReader::open(source, ResourceLimits::default()).map_err(|source| {
            failure(
                &options,
                profile.clone(),
                None,
                None,
                Vec::new(),
                diagnostic(
                    "invalid_archive",
                    format!("failed to index CF archive: {source}"),
                ),
            )
        })?;

    let layout = layout_report(reader.index());
    if let Some(error) = duplicate_name_diagnostic(reader.index()) {
        return Err(failure(
            &options,
            profile,
            Some(layout),
            None,
            Vec::new(),
            error,
        ));
    }

    let requested = effective_selection(&options.requested, &expected);
    let selected = select_indices(reader.index(), &requested);
    let missing = missing_diagnostics(reader.index(), &requested);
    let selection = CfSelectionReport {
        requested,
        selected_count: selected.len(),
        archive_element_count: reader.index().entries.len(),
        list_only: options.list_only,
    };
    if !missing.is_empty() {
        return Err(failure_many(
            &options,
            profile,
            Some(layout),
            Some(selection),
            Vec::new(),
            missing,
        ));
    }

    let encoding = payload_encoding(options.compression);
    let mut decoder = PayloadDecoder::new(ResourceLimits::default());
    let mut elements = Vec::with_capacity(selected.len());
    let mut errors = Vec::new();
    for index in selected {
        let native = reader.index().entries[index].clone();
        match inspect_element(
            &mut reader,
            &mut decoder,
            index,
            &native,
            encoding,
            options.list_only,
        ) {
            Ok(element) => {
                if let Some(expected_digest) = expected.get(&native.name) {
                    let actual = element.unpacked_sha256.as_deref();
                    if actual != Some(expected_digest.as_str()) {
                        errors.push(CfDiagnostic {
                            code: "digest_mismatch",
                            message: format!(
                                "element `{}` unpacked SHA-256 does not match expectation",
                                native.name
                            ),
                            element: Some(native.name.clone()),
                            expected: Some(expected_digest.clone()),
                            actual: actual.map(str::to_owned),
                        });
                    }
                }
                elements.push(element);
            }
            Err(error) => {
                errors.push(error);
                return Err(failure_many(
                    &options,
                    profile,
                    Some(layout),
                    Some(selection),
                    elements,
                    errors,
                ));
            }
        }
    }

    if !errors.is_empty() {
        return Err(failure_many(
            &options,
            profile,
            Some(layout),
            Some(selection),
            elements,
            errors,
        ));
    }

    Ok(CfReport {
        schema_version: REPORT_SCHEMA_VERSION,
        command: options.command,
        ok: true,
        input: display_path(&options.input),
        profile,
        layout: Some(layout),
        selection: Some(selection),
        elements,
        errors: Vec::new(),
    })
}

fn inspect_element(
    reader: &mut StreamingReader<File>,
    decoder: &mut PayloadDecoder,
    index: usize,
    native: &EntryIndex,
    encoding: PayloadEncoding,
    list_only: bool,
) -> Result<CfElementReport, CfDiagnostic> {
    let packed_bytes = native.data.as_ref().map(|chain| chain.data_size);
    let mut report = CfElementReport {
        index,
        name: native.name.clone(),
        data_state: if native.data.is_some() {
            "present"
        } else {
            "absent"
        },
        compression: native.data.as_ref().map(|_| encoding_name(encoding)),
        header_bytes: native.header.data_size,
        header_pages: native.header.pages.len(),
        packed_bytes,
        data_pages: native.data.as_ref().map_or(0, |chain| chain.pages.len()),
        payload_verified: false,
        unpacked_bytes: None,
        packed_sha256: None,
        unpacked_sha256: None,
    };
    if list_only || native.data.is_none() {
        report.payload_verified = native.data.is_none();
        return Ok(report);
    }

    let packed = reader
        .read_entry_data(index)
        .map_err(|source| CfDiagnostic {
            code: "payload_read_failed",
            message: format!("failed to read element `{}` payload: {source}", native.name),
            element: Some(native.name.clone()),
            expected: None,
            actual: None,
        })?
        .expect("present entry index must yield a payload");
    let decoded = decoder
        .decode(encoding, &packed)
        .map_err(|source| CfDiagnostic {
            code: "payload_decode_failed",
            message: format!(
                "failed to decode element `{}` as {}: {source}",
                native.name,
                encoding_name(encoding)
            ),
            element: Some(native.name.clone()),
            expected: None,
            actual: None,
        })?;
    report.payload_verified = true;
    report.unpacked_bytes = u64::try_from(decoded.bytes().len()).ok();
    report.packed_sha256 = Some(Sha256Digest::for_bytes(&packed).to_string());
    report.unpacked_sha256 = Some(Sha256Digest::for_bytes(decoded.bytes()).to_string());
    Ok(report)
}

fn parse_expectations(options: &RunOptions) -> Result<BTreeMap<String, String>, CfDiagnostic> {
    let mut expectations = BTreeMap::new();
    for value in &options.expected_sha256 {
        let Some((name, digest)) = value.split_once('=') else {
            return Err(diagnostic(
                "invalid_expectation",
                format!("invalid SHA-256 expectation `{value}`; expected NAME=SHA256"),
            ));
        };
        if name.is_empty() {
            return Err(diagnostic(
                "invalid_expectation",
                format!("invalid SHA-256 expectation `{value}`; element name is empty"),
            ));
        }
        Sha256Digest::parse(digest).map_err(|source| {
            diagnostic(
                "invalid_expectation",
                format!("invalid SHA-256 expectation for `{name}`: {source}"),
            )
        })?;
        if expectations
            .insert(name.to_owned(), digest.to_owned())
            .is_some()
        {
            return Err(CfDiagnostic {
                code: "duplicate_expectation",
                message: format!("element `{name}` has more than one SHA-256 expectation"),
                element: Some(name.to_owned()),
                expected: None,
                actual: None,
            });
        }
    }
    Ok(expectations)
}

fn effective_selection(requested: &[String], expected: &BTreeMap<String, String>) -> Vec<String> {
    let mut selection = Vec::new();
    let mut seen = BTreeSet::new();
    for name in requested.iter().chain(expected.keys()) {
        if seen.insert(name.clone()) {
            selection.push(name.clone());
        }
    }
    selection
}

fn select_indices(index: &ContainerIndex, requested: &[String]) -> Vec<usize> {
    if requested.is_empty() {
        return (0..index.entries.len()).collect();
    }
    let requested = requested
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    index
        .entries
        .iter()
        .enumerate()
        .filter_map(|(entry_index, entry)| {
            requested
                .contains(entry.name.as_str())
                .then_some(entry_index)
        })
        .collect()
}

fn missing_diagnostics(index: &ContainerIndex, requested: &[String]) -> Vec<CfDiagnostic> {
    let available = index
        .entries
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<BTreeSet<_>>();
    requested
        .iter()
        .filter(|name| !available.contains(name.as_str()))
        .map(|name| CfDiagnostic {
            code: "element_not_found",
            message: format!("CF archive has no top-level element named `{name}`"),
            element: Some(name.clone()),
            expected: None,
            actual: None,
        })
        .collect()
}

fn duplicate_name_diagnostic(index: &ContainerIndex) -> Option<CfDiagnostic> {
    let mut first = BTreeMap::new();
    for (entry_index, entry) in index.entries.iter().enumerate() {
        if let Some(first_index) = first.insert(entry.name.as_str(), entry_index) {
            return Some(CfDiagnostic {
                code: "duplicate_element",
                message: format!(
                    "CF element `{}` occurs at indices {first_index} and {entry_index}",
                    entry.name
                ),
                element: Some(entry.name.clone()),
                expected: None,
                actual: None,
            });
        }
    }
    None
}

fn layout_report(index: &ContainerIndex) -> CfLayoutReport {
    let (revision, page_offset, reserved_offset) = match index.revision {
        Revision::Format15 => ("format15", 4, 12),
        Revision::Format16 => ("format16", 8, 16),
    };
    CfLayoutReport {
        revision,
        base_offset: index.base_offset,
        stream_length: index.stream_length,
        preamble_bytes: index.base_offset,
        page_size: read_u32(&index.raw_file_header, page_offset),
        storage_version: index.storage_version,
        reserved: read_u32(&index.raw_file_header, reserved_offset),
        element_count: index.entries.len(),
        indexed_pages: index.indexed_pages,
        encoded_payload_bytes: index.encoded_payload_bytes,
    }
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
}

fn payload_encoding(compression: CfCompression) -> PayloadEncoding {
    match compression {
        CfCompression::RawDeflate => PayloadEncoding::RawDeflate,
        CfCompression::Stored => PayloadEncoding::Stored,
    }
}

fn encoding_name(encoding: PayloadEncoding) -> &'static str {
    match encoding {
        PayloadEncoding::RawDeflate => "raw-deflate",
        PayloadEncoding::Stored => "stored",
    }
}

fn profile_report(options: &RunOptions) -> CfProfileReport {
    CfProfileReport {
        id: options.profile.clone(),
        compression: encoding_name(payload_encoding(options.compression)),
        compression_source: "explicit_cli_contract",
    }
}

fn diagnostic(code: &'static str, message: String) -> CfDiagnostic {
    CfDiagnostic {
        code,
        message,
        element: None,
        expected: None,
        actual: None,
    }
}

fn failure(
    options: &RunOptions,
    profile: CfProfileReport,
    layout: Option<CfLayoutReport>,
    selection: Option<CfSelectionReport>,
    elements: Vec<CfElementReport>,
    error: CfDiagnostic,
) -> CfCommandError {
    failure_many(options, profile, layout, selection, elements, vec![error])
}

fn failure_many(
    options: &RunOptions,
    profile: CfProfileReport,
    layout: Option<CfLayoutReport>,
    selection: Option<CfSelectionReport>,
    elements: Vec<CfElementReport>,
    errors: Vec<CfDiagnostic>,
) -> CfCommandError {
    CfCommandError {
        report: Box::new(CfReport {
            schema_version: REPORT_SCHEMA_VERSION,
            command: options.command,
            ok: false,
            input: display_path(&options.input),
            profile,
            layout,
            selection,
            elements,
            errors,
        }),
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use ibcmd_cf::payload::encode_payload;
    use ibcmd_v8::writer::{Format15Document, Format15Element, write_format15_to_vec};

    use super::*;

    struct TempFile(PathBuf);

    impl TempFile {
        fn new(bytes: &[u8]) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "ibcmd-rs-cf-command-{}-{nonce}.cf",
                std::process::id()
            ));
            fs::write(&path, bytes).unwrap();
            Self(path)
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn archive() -> (TempFile, String) {
        let unpacked = b"offline payload";
        let packed = encode_payload(
            PayloadEncoding::RawDeflate,
            unpacked,
            ResourceLimits::default(),
        )
        .unwrap();
        let bytes = write_format15_to_vec(&Format15Document::new(
            7,
            vec![
                Format15Element::named("root", Some(packed)),
                Format15Element::named("absent", None),
            ],
        ))
        .unwrap();
        (
            TempFile::new(&bytes),
            Sha256Digest::for_bytes(unpacked).to_string(),
        )
    }

    #[test]
    fn inspect_reports_layout_and_verified_payloads() {
        let (archive, _) = archive();
        let report = run(CfArgs {
            command: CfCommands::Inspect(CfInspectArgs {
                input: archive.0.clone(),
                profile: "storage:cf-test".to_owned(),
                compression: CfCompression::RawDeflate,
                elements: Vec::new(),
            }),
        })
        .unwrap();
        assert!(report.ok);
        assert_eq!(report.layout.unwrap().revision, "format15");
        assert_eq!(report.elements.len(), 2);
        assert!(report.elements[0].payload_verified);
        assert_eq!(report.elements[1].data_state, "absent");
    }

    #[test]
    fn verify_selects_one_element_and_checks_digest() {
        let (archive, digest) = archive();
        let report = run(CfArgs {
            command: CfCommands::Verify(CfVerifyArgs {
                input: archive.0.clone(),
                profile: "storage:cf-test".to_owned(),
                compression: CfCompression::RawDeflate,
                elements: vec!["root".to_owned()],
                list_only: false,
                expected_sha256: vec![format!("root={digest}")],
            }),
        })
        .unwrap();
        assert!(report.ok);
        assert_eq!(report.elements.len(), 1);
        assert_eq!(report.elements[0].name, "root");
    }

    #[test]
    fn wrong_digest_is_a_machine_readable_failure() {
        let (archive, _) = archive();
        let error = run(CfArgs {
            command: CfCommands::Verify(CfVerifyArgs {
                input: archive.0.clone(),
                profile: "storage:cf-test".to_owned(),
                compression: CfCompression::RawDeflate,
                elements: vec!["root".to_owned()],
                list_only: false,
                expected_sha256: vec![format!("root={}", "0".repeat(64))],
            }),
        })
        .unwrap_err();
        assert!(!error.report().ok);
        assert_eq!(error.report().errors[0].code, "digest_mismatch");
    }

    #[test]
    fn list_only_does_not_read_or_decode_selected_payload() {
        let bytes = write_format15_to_vec(&Format15Document::new(
            7,
            vec![Format15Element::named(
                "opaque",
                Some(b"not a deflate stream".to_vec()),
            )],
        ))
        .unwrap();
        let archive = TempFile::new(&bytes);
        let report = run(CfArgs {
            command: CfCommands::Verify(CfVerifyArgs {
                input: archive.0.clone(),
                profile: "storage:cf-test".to_owned(),
                compression: CfCompression::RawDeflate,
                elements: vec!["opaque".to_owned()],
                list_only: true,
                expected_sha256: Vec::new(),
            }),
        })
        .unwrap();
        assert!(report.ok);
        assert!(!report.elements[0].payload_verified);
        assert!(report.elements[0].unpacked_sha256.is_none());
    }
}
