//! Offline measurement of the base-free descriptor compiler.
//!
//! Every metadata XML of a source tree is compiled without a base row and the
//! result is compared byte for byte with the row the platform stored for the
//! same object (`<uuid>__part0.bin`, raw deflate, or `<uuid>__part0.txt`,
//! inflated). Nothing is read from or written to a database.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;

use super::{DescriptorContext, compile_descriptor};
use ibcmd_metadata_blob::parse_simple_metadata_xml_properties;
use ibcmd_module_blob::inflate_raw;
use ibcmd_parallel as parallel;

#[derive(Debug, Clone)]
pub struct DescriptorAuditOptions {
    /// Only these kinds (`Catalog`, `CommonModule`, ...); every kind when empty.
    pub kinds: Vec<String>,
    /// Differing samples kept per kind.
    pub max_samples: usize,
    /// Where the expected and compiled text of each sample is written.
    pub diff_dir: Option<PathBuf>,
}

#[derive(Debug, Default, Serialize)]
pub struct DescriptorAuditReport {
    pub root: String,
    pub rows: String,
    pub objects: usize,
    pub identical: usize,
    pub different: usize,
    pub failed: usize,
    pub no_row: usize,
    pub kinds: BTreeMap<String, DescriptorKindStats>,
}

#[derive(Debug, Default, Serialize)]
pub struct DescriptorKindStats {
    pub total: usize,
    pub identical: usize,
    pub different: usize,
    pub failed: usize,
    pub no_row: usize,
    /// Compile errors by their first line.
    pub failures: BTreeMap<String, usize>,
    /// Where the differing rows first part, by brace path, with counts.
    pub difference_paths: BTreeMap<String, usize>,
    pub samples: Vec<DescriptorSample>,
}

#[derive(Debug, Serialize)]
pub struct DescriptorSample {
    pub path: String,
    pub uuid: String,
    pub offset: usize,
    pub brace_path: String,
    pub expected: String,
    pub actual: String,
}

enum Outcome {
    Identical,
    Different {
        offset: usize,
        brace_path: String,
        expected: Vec<u8>,
        actual: Vec<u8>,
    },
    Failed(String),
    NoRow,
}

struct Measured {
    kind: String,
    path: String,
    uuid: String,
    outcome: Outcome,
}

/// Is this file a metadata object's own XML (not a body under `Ext`)?
pub fn is_descriptor_xml(relative: &str) -> bool {
    let lower = relative.replace('\\', "/").to_ascii_lowercase();
    lower.ends_with(".xml")
        && !lower.contains("/ext/")
        && !lower.starts_with("ext/")
        && lower != "configdumpinfo.xml"
}

pub fn descriptor_xmls(root: &Path) -> Vec<PathBuf> {
    let mut paths = WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .strip_prefix(root)
                .map(|relative| is_descriptor_xml(&relative.to_string_lossy()))
                .unwrap_or(false)
        })
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

/// Reads a stored row: `<name>__part0.txt` (inflated) or `<name>__part0.bin`.
pub fn read_stored_row(rows: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    let text = rows.join(format!("{name}__part0.txt"));
    if text.is_file() {
        return Ok(Some(fs::read(&text)?));
    }
    let bin = rows.join(format!("{name}__part0.bin"));
    if bin.is_file() {
        let raw = fs::read(&bin)?;
        return Ok(Some(inflate_raw(&raw).with_context(|| {
            format!("failed to inflate {}", bin.display())
        })?));
    }
    Ok(None)
}

pub fn audit_descriptor_compiler(
    root: &Path,
    rows: &Path,
    version: &str,
    options: &DescriptorAuditOptions,
) -> Result<DescriptorAuditReport> {
    let context = DescriptorContext::new(root, version)?;
    let paths = descriptor_xmls(root);
    let measured = parallel::install(|| {
        paths
            .par_iter()
            .filter_map(|path| measure_one(root, rows, path, &context, options).transpose())
            .collect::<Result<Vec<_>>>()
    })??;

    let mut report = DescriptorAuditReport {
        root: root.display().to_string(),
        rows: rows.display().to_string(),
        ..Default::default()
    };
    for item in measured {
        let stats = report.kinds.entry(item.kind.clone()).or_default();
        stats.total += 1;
        report.objects += 1;
        match item.outcome {
            Outcome::Identical => {
                stats.identical += 1;
                report.identical += 1;
            }
            Outcome::Failed(message) => {
                stats.failed += 1;
                report.failed += 1;
                let key = message
                    .lines()
                    .next()
                    .unwrap_or("")
                    .chars()
                    .take(200)
                    .collect();
                *stats.failures.entry(key).or_default() += 1;
            }
            Outcome::NoRow => {
                stats.no_row += 1;
                report.no_row += 1;
            }
            Outcome::Different {
                offset,
                brace_path,
                expected,
                actual,
            } => {
                stats.different += 1;
                report.different += 1;
                *stats
                    .difference_paths
                    .entry(brace_path.clone())
                    .or_default() += 1;
                if stats.samples.len() < options.max_samples {
                    if let Some(dir) = &options.diff_dir {
                        write_sample(dir, &item.kind, &item.path, &expected, &actual)?;
                    }
                    stats.samples.push(DescriptorSample {
                        path: item.path,
                        uuid: item.uuid,
                        offset,
                        brace_path,
                        expected: excerpt(&expected, offset),
                        actual: excerpt(&actual, offset),
                    });
                }
            }
        }
    }
    Ok(report)
}

fn measure_one(
    root: &Path,
    rows: &Path,
    path: &Path,
    context: &DescriptorContext,
    options: &DescriptorAuditOptions,
) -> Result<Option<Measured>> {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    let xml = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let Ok(properties) = parse_simple_metadata_xml_properties(&xml) else {
        return Ok(None);
    };
    if !options.kinds.is_empty() && !options.kinds.contains(&properties.kind) {
        return Ok(None);
    }
    let outcome = match read_stored_row(rows, &properties.uuid)? {
        None => Outcome::NoRow,
        Some(expected) => {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compile_descriptor(&properties.kind, path, &xml, context)
            })) {
                Err(_) => Outcome::Failed("compiler panicked".to_string()),
                Ok(Err(error)) => Outcome::Failed(format!("{error:#}")),
                Ok(Ok(actual)) if actual == expected => Outcome::Identical,
                Ok(Ok(actual)) => {
                    let offset = first_difference(&expected, &actual);
                    Outcome::Different {
                        offset,
                        brace_path: brace_path_at(&expected, offset),
                        expected,
                        actual,
                    }
                }
            }
        }
    };
    Ok(Some(Measured {
        kind: properties.kind,
        path: relative,
        uuid: properties.uuid,
        outcome,
    }))
}

fn first_difference(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right.iter())
        .position(|(l, r)| l != r)
        .unwrap_or_else(|| left.len().min(right.len()))
}

/// The position of `offset` in the brace tree, as element indexes from the
/// outermost list inward (`1.3` = the fourth element of the list that is the
/// second element of the row).
pub fn brace_path_at(text: &[u8], offset: usize) -> String {
    let mut stack: Vec<usize> = Vec::new();
    let mut in_string = false;
    let mut index = 0;
    while index < offset.min(text.len()) {
        let byte = text[index];
        if in_string {
            if byte == b'"' {
                if text.get(index + 1) == Some(&b'"') {
                    index += 1;
                } else {
                    in_string = false;
                }
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' => stack.push(0),
                b'}' => {
                    stack.pop();
                }
                b',' => {
                    if let Some(top) = stack.last_mut() {
                        *top += 1;
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    stack
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn excerpt(text: &[u8], offset: usize) -> String {
    let start = offset.saturating_sub(60);
    let end = (offset + 60).min(text.len());
    String::from_utf8_lossy(&text[start.min(end)..end]).into_owned()
}

fn write_sample(dir: &Path, kind: &str, path: &str, expected: &[u8], actual: &[u8]) -> Result<()> {
    let folder = dir.join(kind);
    fs::create_dir_all(&folder)?;
    let stem = path.trim_end_matches(".xml").replace(['/', '\\'], "__");
    fs::write(folder.join(format!("{stem}.expected.txt")), expected)?;
    fs::write(folder.join(format!("{stem}.actual.txt")), actual)?;
    Ok(())
}

/// One line per kind, for the terminal.
pub fn summary_table(report: &DescriptorAuditReport) -> String {
    let mut lines = vec![format!(
        "{:<34} {:>6} {:>9} {:>9} {:>7} {:>6}",
        "kind", "total", "identical", "different", "failed", "no_row"
    )];
    for (kind, stats) in &report.kinds {
        lines.push(format!(
            "{:<34} {:>6} {:>9} {:>9} {:>7} {:>6}",
            kind, stats.total, stats.identical, stats.different, stats.failed, stats.no_row
        ));
    }
    lines.push(format!(
        "{:<34} {:>6} {:>9} {:>9} {:>7} {:>6}",
        "TOTAL", report.objects, report.identical, report.different, report.failed, report.no_row
    ));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brace_path_counts_elements_and_skips_strings() {
        let text = br#"{1,{2,"a,{b""}",{3,4}}}"#;
        let offset = text.iter().position(|b| *b == b'4').unwrap();
        assert_eq!(brace_path_at(text, offset), "1.2.1");
    }
}
