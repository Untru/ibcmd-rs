//! Round-trip instrument for `DataCompositionSchema` template bodies.
//!
//! For every schema template of a source tree it compiles `Ext/Template.xml`
//! through the loader's own entry ([`crate::mssql::compile_dcs_template_body`],
//! resolving types and style items against the same tree), hands the compiled
//! body to the exporter's own reader with the type index and object references
//! of the database the tree was exported from, and compares the result with
//! the exported file byte for byte. It also compares the compiled body with the
//! body the platform stored, which measures how close the writer comes to the
//! platform rather than whether the cycle closes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;

use crate::compiler::bodies::dcs::{DcsTemplateKind, decode_compatible_dcs};
use crate::module_blob::{
    MetadataSourceContext, parse_simple_metadata_xml_properties, parse_template_type_from_xml,
};
use crate::mssql_dump::offline_context::OfflineFormContextFactory;
use crate::parallel;
use ibcmd_core::artifact::ProfileId;

#[derive(Debug, Serialize)]
pub struct DcsTemplateWriterReport {
    pub root: PathBuf,
    pub bodies: PathBuf,
    pub dump: PathBuf,
    /// `DataCompositionSchema` templates found in the tree.
    pub templates: usize,
    /// Templates the loader compiled base-free.
    pub compiled: usize,
    /// Templates the loader refused, by reason (numbers masked).
    pub refused: BTreeMap<String, usize>,
    /// Compiled templates the exporter reads back byte for byte.
    pub round_trip_exact: usize,
    /// Compiled templates the exporter reads back differently or not at all,
    /// by path.
    pub round_trip_failed: BTreeMap<String, String>,
    /// Refused templates, by path, with the full reason.
    pub refused_templates: BTreeMap<String, String>,
    /// Compiled templates whose stored body was found.
    pub stored_compared: usize,
    /// Compiled bodies identical to the stored body.
    pub stored_exact: usize,
    pub stored_primary_exact: usize,
    pub stored_settings_exact: usize,
    pub stored_settings_total: usize,
    pub stored_terminal_exact: usize,
    /// Where compiled and stored bodies first diverge, grouped by a short
    /// window on both sides.
    pub stored_shapes: BTreeMap<String, usize>,
}

struct Outcome {
    path: String,
    compiled: Result<Vec<u8>, String>,
    round_trip: Option<Result<(), String>>,
    stored: Option<StoredComparison>,
}

struct StoredComparison {
    exact: bool,
    primary: bool,
    settings: (usize, usize),
    terminal: bool,
    shape: Option<String>,
}

pub fn audit_dcs_template_writer(
    root: &Path,
    bodies: &Path,
    dump: &Path,
) -> Result<DcsTemplateWriterReport> {
    let context = OfflineFormContextFactory::from_dump_dir(dump, None)
        .with_context(|| format!("failed to build the export context from {}", dump.display()))?;
    let mut holders = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.with_context(|| format!("failed to walk {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let parent = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str());
        if !matches!(parent, Some("Templates" | "CommonTemplates"))
            || path.extension().and_then(|extension| extension.to_str()) != Some("xml")
        {
            continue;
        }
        let xml = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
        if parse_template_type_from_xml(&xml)?.as_deref() != Some("DataCompositionSchema") {
            continue;
        }
        let properties = parse_simple_metadata_xml_properties(&xml)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        holders.push((path.to_path_buf(), properties.uuid));
    }
    holders.sort();

    let source = MetadataSourceContext::new(root.to_path_buf());
    let provider = ProfileId::parse("provider:mssql-legacy").expect("static profile is valid");
    let target = ProfileId::parse("xml-2.20").expect("static profile is valid");
    let outcomes = parallel::install(|| {
        holders
            .par_iter()
            .map(|(holder, uuid)| {
                let body_path = holder.with_extension("").join("Ext").join("Template.xml");
                let path = body_path
                    .strip_prefix(root)
                    .unwrap_or(&body_path)
                    .to_string_lossy()
                    .replace('\\', "/");
                let Ok(source_xml) = fs::read(&body_path) else {
                    return Outcome {
                        path,
                        compiled: Err("Template.xml is missing".to_string()),
                        round_trip: None,
                        stored: None,
                    };
                };
                let compiled = crate::mssql::compile_dcs_template_body(&source_xml, Some(&source))
                    .map_err(|error| error.to_string());
                let Ok(blob) = compiled.as_ref() else {
                    return Outcome {
                        path,
                        compiled,
                        round_trip: None,
                        stored: None,
                    };
                };
                let body = match decode_compatible_dcs(DcsTemplateKind::Schema, blob) {
                    Ok(body) => body,
                    Err(error) => {
                        return Outcome {
                            path,
                            compiled,
                            round_trip: Some(Err(format!("compiled body does not decode: {error}"))),
                            stored: None,
                        };
                    }
                };
                let round_trip = match crate::mssql_dump::normalize_data_composition_schema_template_documents_with_profiles(
                    &body.documents(),
                    &context.dcs_type_index,
                    &context.object_refs,
                    &provider,
                    &target,
                ) {
                    Ok(exported) if exported == source_xml => Ok(()),
                    Ok(exported) => {
                        let at = exported
                            .iter()
                            .zip(&source_xml)
                            .position(|(left, right)| left != right)
                            .unwrap_or_else(|| exported.len().min(source_xml.len()));
                        Err(format!(
                            "differs at byte {at}: wrote `{}` expected `{}`",
                            window(&exported, at),
                            window(&source_xml, at)
                        ))
                    }
                    Err(error) => Err(format!("exporter refused the compiled body: {error}")),
                };
                let stored = stored_plain(bodies, uuid).map(|stored| compare_stored(body.plaintext(), &stored));
                Outcome {
                    path,
                    compiled,
                    round_trip: Some(round_trip),
                    stored,
                }
            })
            .collect::<Vec<_>>()
    })?;

    let mut report = DcsTemplateWriterReport {
        root: root.to_path_buf(),
        bodies: bodies.to_path_buf(),
        dump: dump.to_path_buf(),
        templates: outcomes.len(),
        compiled: 0,
        refused: BTreeMap::new(),
        round_trip_exact: 0,
        round_trip_failed: BTreeMap::new(),
        refused_templates: BTreeMap::new(),
        stored_compared: 0,
        stored_exact: 0,
        stored_primary_exact: 0,
        stored_settings_exact: 0,
        stored_settings_total: 0,
        stored_terminal_exact: 0,
        stored_shapes: BTreeMap::new(),
    };
    for outcome in outcomes {
        match outcome.compiled {
            Err(reason) => {
                *report.refused.entry(mask_numbers(&reason)).or_insert(0) += 1;
                report.refused_templates.insert(outcome.path, reason);
                continue;
            }
            Ok(_) => report.compiled += 1,
        }
        match outcome.round_trip {
            Some(Ok(())) => report.round_trip_exact += 1,
            Some(Err(reason)) => {
                report
                    .round_trip_failed
                    .insert(outcome.path.clone(), reason);
            }
            None => {}
        }
        if let Some(stored) = outcome.stored {
            report.stored_compared += 1;
            report.stored_exact += usize::from(stored.exact);
            report.stored_primary_exact += usize::from(stored.primary);
            report.stored_settings_exact += stored.settings.0;
            report.stored_settings_total += stored.settings.1;
            report.stored_terminal_exact += usize::from(stored.terminal);
            if let Some(shape) = stored.shape {
                *report.stored_shapes.entry(shape).or_insert(0) += 1;
            }
        }
    }
    Ok(report)
}

fn stored_plain(bodies: &Path, uuid: &str) -> Option<Vec<u8>> {
    [format!("{uuid}.0__part0.txt"), format!("{uuid}.0.txt")]
        .iter()
        .find_map(|name| fs::read(bodies.join(name)).ok())
}

fn compare_stored(compiled: &[u8], stored: &[u8]) -> StoredComparison {
    let compiled_documents = split_documents(compiled);
    let stored_documents = split_documents(stored);
    let mut comparison = StoredComparison {
        exact: compiled == stored,
        primary: false,
        settings: (0, 0),
        terminal: false,
        shape: None,
    };
    let (Some(compiled_documents), Some(stored_documents)) = (compiled_documents, stored_documents)
    else {
        comparison.shape = Some("envelope does not frame".to_string());
        return comparison;
    };
    if compiled_documents.len() != stored_documents.len() {
        comparison.shape = Some(format!(
            "document count {} vs {}",
            compiled_documents.len(),
            stored_documents.len()
        ));
        return comparison;
    }
    let last = stored_documents.len() - 1;
    for (index, (left, right)) in compiled_documents.iter().zip(&stored_documents).enumerate() {
        let equal = left == right;
        if index == 0 {
            comparison.primary = equal;
        } else if index == last {
            comparison.terminal = equal;
        } else {
            comparison.settings.1 += 1;
            comparison.settings.0 += usize::from(equal);
        }
        if !equal && comparison.shape.is_none() {
            let at = left
                .iter()
                .zip(right.iter())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| left.len().min(right.len()));
            let role = match index {
                0 => "primary",
                index if index == last => "terminal",
                _ => "settings",
            };
            comparison.shape = Some(mask_generated_prefixes(&format!(
                "{role}: wrote `{}` stored `{}`",
                window(left, at),
                window(right, at)
            )));
        }
    }
    comparison
}

fn split_documents(plain: &[u8]) -> Option<Vec<&[u8]>> {
    let read_u32 = |offset: usize| {
        plain
            .get(offset..offset + 4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four bytes")))
    };
    let read_u64 = |offset: usize| {
        plain
            .get(offset..offset + 8)
            .map(|bytes| u64::from_le_bytes(bytes.try_into().expect("eight bytes")))
    };
    let count = usize::try_from(read_u32(4)?).ok()?;
    let mut offset = 8 + 8 * (count + 1);
    let mut documents = Vec::with_capacity(count + 2);
    for index in 0..=count {
        let length = usize::try_from(read_u64(8 + 8 * index)?).ok()?;
        documents.push(plain.get(offset..offset + length)?);
        offset += length;
    }
    documents.push(plain.get(offset..)?);
    Some(documents)
}

/// Up to 30 characters either side of `at`, with line breaks made visible.
fn window(bytes: &[u8], at: usize) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut start = at.saturating_sub(30).min(text.len());
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut middle = at.min(text.len());
    while !text.is_char_boundary(middle) {
        middle -= 1;
    }
    let mut end = (at + 40).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}|{}", &text[start..middle], &text[middle..end])
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn mask_numbers(reason: &str) -> String {
    let mut out = String::with_capacity(reason.len());
    let mut in_number = false;
    for character in reason.chars() {
        if character.is_ascii_digit() {
            if !in_number {
                out.push('N');
                in_number = true;
            }
        } else {
            in_number = false;
            out.push(character);
        }
    }
    out
}

fn mask_generated_prefixes(shape: &str) -> String {
    let mut out = String::with_capacity(shape.len());
    let characters: Vec<char> = shape.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        // `d<digits>p<digits>` -> `dNpM`
        if characters[index] == 'd' && characters.get(index + 1).is_some_and(char::is_ascii_digit) {
            let mut cursor = index + 1;
            while characters.get(cursor).is_some_and(char::is_ascii_digit) {
                cursor += 1;
            }
            if characters.get(cursor) == Some(&'p')
                && characters.get(cursor + 1).is_some_and(char::is_ascii_digit)
            {
                cursor += 1;
                while characters.get(cursor).is_some_and(char::is_ascii_digit) {
                    cursor += 1;
                }
                out.push_str("dNpM");
                index = cursor;
                continue;
            }
        }
        out.push(characters[index]);
        index += 1;
    }
    out
}
