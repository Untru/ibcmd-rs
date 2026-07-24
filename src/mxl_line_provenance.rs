//! Offline JSONL trace for final MXL line palettes in a saved candidate dump.
use crate::cli::MxlLineProvenanceCorpusArgs;
use crate::mssql_dump::{
    extract_inflated_moxel_spreadsheet_xml_with_line_trace,
    extract_moxel_spreadsheet_xml_with_line_trace, MoxelLineTraceEvent, MoxelLineTraceSink,
};
use anyhow::{bail, Context, Result};
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

#[derive(Clone, Copy)]
struct CorpusLimits {
    files: usize,
    asset_bytes: u64,
    corpus_bytes: u64,
    trace_events: usize,
}

impl CorpusLimits {
    const PRODUCTION: Self = Self {
        files: MAX_CORPUS_FILES,
        asset_bytes: MAX_ASSET_BYTES,
        corpus_bytes: MAX_CORPUS_BYTES,
        trace_events: MAX_TRACE_EVENTS,
    };
}

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
    event_capacity: usize,
    remaining: std::cell::Cell<usize>,
    overflow: std::cell::Cell<bool>,
}

impl Sink {
    fn new(event_capacity: usize) -> Self {
        Self {
            events: RefCell::new(Vec::new()),
            event_capacity,
            remaining: std::cell::Cell::new(event_capacity),
            overflow: std::cell::Cell::new(false),
        }
    }

    fn take(self) -> Vec<MoxelLineTraceEvent> {
        self.events.into_inner()
    }

    fn overflowed(&self) -> bool {
        self.overflow.get()
    }
}

impl MoxelLineTraceSink for Sink {
    fn try_reserve_event(&self) -> bool {
        let remaining = self.remaining.get();
        if remaining == 0 {
            self.overflow.set(true);
            false
        } else {
            self.remaining.set(remaining - 1);
            true
        }
    }

    fn record_moxel_line(&self, event: MoxelLineTraceEvent) {
        let mut events = self.events.borrow_mut();
        if events.len() < self.event_capacity {
            events.push(event);
        } else {
            // Do not retain or allocate for excess provenance: a truncated
            // trace would be misleading, so the caller rejects the asset.
            self.overflow.set(true);
        }
    }
}

/// Walks raw `candidate_dump` payloads and uses the production compatible-MXL
/// decoder/extractor.  Non-MXL assets are intentionally ignored; no filename,
/// path, runtime name, or UUID becomes part of an event.
pub fn run_mxl_line_provenance_corpus(
    args: &MxlLineProvenanceCorpusArgs,
) -> Result<MxlLineProvenanceSummary> {
    run_mxl_line_provenance_corpus_with_limits(args, CorpusLimits::PRODUCTION)
}

fn run_mxl_line_provenance_corpus_with_limits(
    args: &MxlLineProvenanceCorpusArgs,
    limits: CorpusLimits,
) -> Result<MxlLineProvenanceSummary> {
    let root = args.run_root.join("candidate_dump");
    let canonical_root =
        fs::canonicalize(&root).with_context(|| format!("canonicalize {}", root.display()))?;
    let file =
        File::create(&args.output).with_context(|| format!("create {}", args.output.display()))?;
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
        if summary.scanned_assets >= limits.files {
            bail!("MXL trace file limit exceeded");
        }
        let asset = canonical_asset_path(&canonical_root, &asset)?;
        let metadata =
            fs::metadata(&asset).with_context(|| format!("metadata {}", asset.display()))?;
        if metadata.len() > limits.asset_bytes
            || total_bytes.saturating_add(metadata.len()) > limits.corpus_bytes
        {
            summary.rejected_assets += 1;
            bail!("MXL trace byte limit exceeded");
        }
        summary.scanned_assets += 1;
        total_bytes += metadata.len();
        let bytes = fs::read(&asset).with_context(|| format!("read {}", asset.display()))?;
        let remaining_events = limits.trace_events.saturating_sub(summary.final_lines);
        let sink = Sink::new(remaining_events);
        let extracted =
            extract_moxel_spreadsheet_xml_with_line_trace(&bytes, &object_refs, Some(&sink))
                .or_else(|| {
                    extract_inflated_moxel_spreadsheet_xml_with_line_trace(
                        &bytes,
                        &object_refs,
                        Some(&sink),
                    )
                });
        if sink.overflowed() {
            summary.rejected_assets += 1;
            bail!("MXL trace event limit exceeded");
        }
        if extracted.is_none() {
            return Ok(());
        }
        let events = sink.take();
        if summary.final_lines.saturating_add(events.len()) > limits.trace_events {
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
        || asset
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("asset must be a non-empty path relative to candidate_dump");
    }
    canonical_asset_path(root, &root.join(asset))
}

fn canonical_asset_path(root: &Path, asset: &Path) -> Result<PathBuf> {
    let path =
        fs::canonicalize(asset).with_context(|| format!("canonicalize {}", asset.display()))?;
    if !path.starts_with(root) || !path.is_file() {
        bail!("asset is not a file: {}", asset.display());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("ibcmd-mxl-provenance-{name}-{nonce}"))
    }

    fn args(root: &Path) -> MxlLineProvenanceCorpusArgs {
        MxlLineProvenanceCorpusArgs {
            run_root: root.to_owned(),
            output: root.join("trace.jsonl"),
            asset: Vec::new(),
        }
    }

    fn event(index: usize) -> MoxelLineTraceEvent {
        MoxelLineTraceEvent {
            output_line_index: index,
            raw_parents: Vec::new(),
            transformations: Vec::new(),
            format_support: Vec::new(),
            final_style: "solid",
            final_type: "thin",
            final_width: 1,
            final_gap: false,
            ambiguous: false,
            fail_closed: false,
        }
    }

    #[test]
    fn sink_accepts_only_remaining_global_budget_and_marks_overflow() {
        let sink = Sink::new(2);
        assert!(sink.try_reserve_event());
        sink.record_moxel_line(event(0));
        assert!(sink.try_reserve_event());
        sink.record_moxel_line(event(1));
        assert!(!sink.overflowed());

        // The third event is deliberately dropped.  In particular this path
        // does not grow the retained vector after the budget is exhausted.
        assert!(!sink.try_reserve_event());
        assert!(sink.overflowed());
        let events = sink.take();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].output_line_index, 0);
        assert_eq!(events[1].output_line_index, 1);
    }

    #[test]
    fn sink_with_no_remaining_budget_never_retains_an_event() {
        let sink = Sink::new(0);
        assert!(!sink.try_reserve_event());
        assert!(sink.overflowed());
        assert!(sink.take().is_empty());
    }

    #[test]
    fn corpus_limits_reject_file_count_per_file_and_total_bytes() {
        let root = test_root("limits");
        let dump = root.join("candidate_dump");
        fs::create_dir_all(&dump).unwrap();
        fs::write(dump.join("a.raw"), b"x").unwrap();
        fs::write(dump.join("b.raw"), b"y").unwrap();

        let mut limits = CorpusLimits {
            files: 0,
            asset_bytes: 1,
            corpus_bytes: 2,
            trace_events: 1,
        };
        let error = run_mxl_line_provenance_corpus_with_limits(&args(&root), limits).unwrap_err();
        assert!(error.to_string().contains("file limit"));

        limits.files = 2;
        limits.asset_bytes = 0;
        let error = run_mxl_line_provenance_corpus_with_limits(&args(&root), limits).unwrap_err();
        assert!(error.to_string().contains("byte limit"));

        limits.asset_bytes = 1;
        limits.corpus_bytes = 1;
        let error = run_mxl_line_provenance_corpus_with_limits(&args(&root), limits).unwrap_err();
        assert!(error.to_string().contains("byte limit"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_asset_rejects_noncanonical_escape() {
        let root = test_root("containment");
        let dump = root.join("candidate_dump");
        fs::create_dir_all(&dump).unwrap();
        fs::write(root.join("outside.raw"), b"x").unwrap();
        let canonical = fs::canonicalize(&dump).unwrap();
        assert!(candidate_asset_path(&canonical, Path::new("../outside.raw")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn canonical_asset_path_rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = test_root("symlink");
        let dump = root.join("candidate_dump");
        fs::create_dir_all(&dump).unwrap();
        let outside = root.join("outside.raw");
        fs::write(&outside, b"x").unwrap();
        symlink(&outside, dump.join("escape.raw")).unwrap();
        let canonical = fs::canonicalize(&dump).unwrap();
        assert!(candidate_asset_path(&canonical, Path::new("escape.raw")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn canonical_asset_path_rejects_reparse_escape_when_symlinks_are_available() {
        use std::os::windows::fs::symlink_file;

        let root = test_root("reparse");
        let dump = root.join("candidate_dump");
        fs::create_dir_all(&dump).unwrap();
        let outside = root.join("outside.raw");
        fs::write(&outside, b"x").unwrap();
        if symlink_file(&outside, dump.join("escape.raw")).is_err() {
            // Creating reparse points requires a Windows privilege on some
            // CI hosts; containment itself is covered on Unix above.
            fs::remove_dir_all(root).unwrap();
            return;
        }
        let canonical = fs::canonicalize(&dump).unwrap();
        assert!(candidate_asset_path(&canonical, Path::new("escape.raw")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
