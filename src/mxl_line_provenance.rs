//! Offline JSONL trace for final MXL line palettes in a saved candidate dump.
use crate::cli::MxlLineProvenanceCorpusArgs;
use crate::mssql_dump::{
    MoxelLineTraceEvent, MoxelLineTraceSink, extract_inflated_moxel_spreadsheet_xml_with_line_trace,
    extract_moxel_spreadsheet_xml_with_line_trace,
};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

const MAX_CORPUS_FILES: usize = 100_000;
const MAX_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CORPUS_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_TRACE_EVENTS: usize = 1_000_000;

#[derive(Debug, Serialize)]
pub struct MxlLineProvenanceSummary {
    pub schema_version: u32,
    pub scanned_assets: usize,
    pub traced_assets: usize,
    pub final_lines: usize,
    pub output: String,
    pub rejected_assets: usize,
}

struct Sink {
    events: RefCell<Vec<MoxelLineTraceEvent>>,
}

impl Sink {
    fn take(self) -> Vec<MoxelLineTraceEvent> {
        self.events.into_inner()
    }
}

impl MoxelLineTraceSink for Sink {
    fn record_moxel_line(&self, event: MoxelLineTraceEvent) {
        self.events.borrow_mut().push(event);
    }
}

/// Walks raw `candidate_dump` payloads and uses the production compatible-MXL
/// decoder/extractor.  Non-MXL assets are intentionally ignored; no filename,
/// path, runtime name, or UUID becomes part of an event.
pub fn run_mxl_line_provenance_corpus(
    args: &MxlLineProvenanceCorpusArgs,
) -> Result<MxlLineProvenanceSummary> {
    let root = args.run_root.join("candidate_dump");
    let canonical_root = fs::canonicalize(&root)
        .with_context(|| format!("canonicalize {}", root.display()))?;
    let file = File::create(&args.output)
        .with_context(|| format!("create {}", args.output.display()))?;
    let mut output = BufWriter::new(file);
    let mut summary = MxlLineProvenanceSummary {
        schema_version: 1,
        scanned_assets: 0,
        traced_assets: 0,
        final_lines: 0,
        output: args.output.display().to_string(),
        rejected_assets: 0,
    };
    let object_refs = BTreeMap::new();
    let mut total_bytes = 0u64;
    let mut process_asset = |asset: PathBuf| -> Result<()> {
        if summary.scanned_assets >= MAX_CORPUS_FILES { bail!("MXL trace file limit exceeded"); }
        let metadata = fs::metadata(&asset).with_context(|| format!("metadata {}", asset.display()))?;
        if metadata.len() > MAX_ASSET_BYTES || total_bytes.saturating_add(metadata.len()) > MAX_CORPUS_BYTES {
            summary.rejected_assets += 1;
            bail!("MXL trace byte limit exceeded");
        }
        summary.scanned_assets += 1;
        total_bytes += metadata.len();
        let bytes = fs::read(&asset).with_context(|| format!("read {}", asset.display()))?;
        let sink = Sink { events: RefCell::new(Vec::new()) };
        let extracted = extract_moxel_spreadsheet_xml_with_line_trace(
            &bytes,
            &object_refs,
            Some(&sink),
        )
        .or_else(|| {
            extract_inflated_moxel_spreadsheet_xml_with_line_trace(&bytes, &object_refs, Some(&sink))
        });
        if extracted.is_none() {
            return Ok(());
        }
        let events = sink.take();
        if summary.final_lines.saturating_add(events.len()) > MAX_TRACE_EVENTS {
            summary.rejected_assets += 1;
            bail!("MXL trace event limit exceeded");
        }
        summary.traced_assets += 1;
        summary.final_lines += events.len();
        for event in events {
            serde_json::to_writer(&mut output, &event)?;
            output.write_all(b"\n")?;
        }
        Ok(())
    };
    if args.asset.is_empty() {
        for entry in WalkDir::new(&canonical_root).sort_by_file_name() {
            let entry = entry.with_context(|| format!("walk {}", canonical_root.display()))?;
            if entry.file_type().is_file() && entry.file_name() != "manifest.json" {
                process_asset(entry.into_path())?;
            }
        }
    } else {
        for asset in &args.asset {
            process_asset(candidate_asset_path(&canonical_root, asset)?)?;
        }
    }
    output.flush()?;
    Ok(summary)
}

fn candidate_asset_path(root: &Path, asset: &Path) -> Result<PathBuf> {
    if asset.as_os_str().is_empty()
        || asset.components().any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("asset must be a non-empty path relative to candidate_dump");
    }
    let path = fs::canonicalize(root.join(asset))
        .with_context(|| format!("canonicalize {}", asset.display()))?;
    if !path.starts_with(root) || !path.is_file() {
        bail!("asset is not a file: {}", asset.display());
    }
    Ok(path)
}
