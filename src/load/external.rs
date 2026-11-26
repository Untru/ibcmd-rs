//! The compiled load of an external data processor or report (.epf/.erf):
//! the tree is in the external layout (`<Name>.xml`, `<Name>/...`), the
//! compiler reads the configuration one. The base is exported to scratch the
//! way the export adapter sees it (`DataProcessors/<Name>...`), the tree's
//! edits are carried over into that layout, the objects are compiled as a
//! configuration's, and the object's row goes back into the external main
//! row (`crate::external::header::external_main_text`).

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::Path,
};

use anyhow::{Context, Result};

use super::{LoadError, LoadReport, TreeDiff, compiled, keys_by_output};
use crate::{
    external::{export, header, rename},
    legacy_version::InfobaseConfigSourceVersion,
    module_blob::{deflate_raw, inflate_raw},
};

/// Root properties the external XML leaves out, which the configuration's
/// XML carries (`crate::external::root_xml`).
const INTERNAL_ONLY: [&str; 4] = [
    "UseStandardCommands",
    "IncludeHelpInContents",
    "ExtendedPresentation",
    "Explanation",
];

/// `Ok(None)` when `base` is not an external object.
pub(crate) fn load_external(
    edited: &Path,
    base: &Path,
    output: &Path,
    source_version: InfobaseConfigSourceVersion,
    diff: &TreeDiff,
    report: &mut LoadReport,
) -> std::result::Result<Option<()>, LoadError> {
    let (archive, limits) = compiled::decode_base(base)?;
    let entries = archive
        .image()
        .entries()
        .iter()
        .map(|entry| {
            (
                entry.logical_key().as_str().to_owned(),
                entry.packed_payload().to_vec(),
            )
        })
        .collect::<Vec<_>>();
    let Some(adapted) = export::adapt(entries)? else {
        return Ok(None);
    };
    let main = &adapted.main;
    let name = main.name.as_str();
    let folder = main.kind.internal_folder();

    let scratch = Scratch::new()?;
    let exported = crate::mssql_dump::export_packed_entries_to_source(
        "storage:cf-cli",
        adapted.entries.clone(),
        &scratch.0,
        true,
        source_version,
        Some(&adapted.foreign),
    )
    .context("failed to export the base in the configuration layout")?;
    let keys = keys_by_output(&exported);
    write_foreign_stubs(&scratch.0, &adapted.foreign, source_version)?;
    write_configuration_stub(&scratch.0, source_version)?;

    // The tree's edits, carried into the configuration layout.
    let root = format!("{name}.xml");
    let under = format!("{name}/");
    let map = |path: &str| -> Option<String> {
        if path == root {
            Some(format!("{folder}/{name}.xml"))
        } else {
            path.strip_prefix(&under)
                .map(|rest| format!("{folder}/{name}/{rest}"))
        }
    };
    let unmapped = diff
        .changed
        .iter()
        .chain(&diff.added)
        .chain(&diff.removed)
        .filter(|path| map(path).is_none())
        .cloned()
        .collect::<Vec<_>>();
    if !unmapped.is_empty() {
        return Err(LoadError::Unsupported(unmapped));
    }
    let mut internal = TreeDiff::default();
    for (paths, into) in [
        (&diff.changed, &mut internal.changed),
        (&diff.added, &mut internal.added),
    ] {
        for path in paths {
            let target = map(path).expect("mapped above");
            let bytes =
                fs::read(edited.join(path)).with_context(|| format!("failed to read {path}"))?;
            let bytes = if *path == root {
                let base_xml = fs::read_to_string(scratch.0.join(&target))
                    .with_context(|| format!("the base export has no {target}"))?;
                let edited_xml =
                    String::from_utf8(bytes).with_context(|| format!("{path} is not UTF-8"))?;
                merged_root(&base_xml, &edited_xml, main)?.into_bytes()
            } else {
                internal_bytes(path, bytes, main)
            };
            let full = scratch.0.join(&target);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).context("scratch directory")?;
            }
            fs::write(&full, bytes).with_context(|| format!("failed to write {target}"))?;
            into.push(target);
        }
    }
    for path in &diff.removed {
        let target = map(path).expect("mapped above");
        let _ = fs::remove_file(scratch.0.join(&target));
        internal.removed.push(target);
    }

    let base_rows = adapted.entries.iter().cloned().collect::<HashMap<_, _>>();
    let edit = compiled::compile_edit(&scratch.0, base_rows, &internal, &keys)?;
    // The object's row goes back into the main row, under the main entry.
    let original_main = entries_text(&archive, &main.main_uuid)?;
    let mut rows = BTreeMap::new();
    for (key, packed) in edit.rows {
        if key == main.object_id {
            let internal_row = String::from_utf8(inflate_raw(&packed)?)
                .context("the compiled object row is not UTF-8")?;
            let text = header::external_main_text(main, &internal_row, &original_main)?;
            rows.insert(main.main_uuid.clone(), deflate_raw(text.as_bytes())?);
        } else {
            rows.insert(key, packed);
        }
    }
    if edit.dropped.contains(&main.object_id) {
        return Err(anyhow::anyhow!("an external object's own row cannot be removed").into());
    }
    compiled::write_edit(&archive, rows, &edit.dropped, output, limits)?;
    report.compiled_objects = edit.prefixes.into_iter().collect();
    report.removed_objects = edit
        .removed
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    report.applied = compiled::applied_paths(diff);
    Ok(Some(()))
}

/// Stand-ins for the configuration objects the external object refers to
/// (its `copyinfo`): an XML naming each one's uuid and generated types, so
/// the compiler resolves `CatalogRef.X` and the like as the export did. They
/// are read, never compiled.
fn write_foreign_stubs(
    root: &Path,
    foreign: &crate::mssql_dump::ForeignReferences,
    source_version: InfobaseConfigSourceVersion,
) -> Result<()> {
    for (uuid, reference) in &foreign.objects {
        let Some((kind, name)) = reference.split_once('.') else {
            continue;
        };
        // A top-level object only (`Catalog.X`, not `Catalog.X.Form.Y`), its
        // name an identifier: the name is a path under the scratch tree and
        // XML text, and it comes from the file.
        if !crate::external::header::is_object_name(name)
            || !uuid.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
        {
            continue;
        }
        let Some(folder) = crate::mssql_dump::root_family_folder(kind) else {
            continue;
        };
        let path = root.join(folder).join(format!("{name}.xml"));
        if path.exists() {
            continue;
        }
        let mut types = String::new();
        for (type_id, spelled, category) in &foreign.types {
            let spelled = spelled.strip_prefix("cfg:").unwrap_or(spelled);
            let Some((type_kind, type_name)) = spelled.split_once('.') else {
                continue;
            };
            if type_name == name && type_kind.starts_with(kind) {
                types.push_str(&format!(
                    "\t\t\t<xr:GeneratedType name=\"{spelled}\" category=\"{category}\">\r\n\
                     \t\t\t\t<xr:TypeId>{type_id}</xr:TypeId>\r\n\
                     \t\t\t\t<xr:ValueId>00000000-0000-0000-0000-000000000000</xr:ValueId>\r\n\
                     \t\t\t</xr:GeneratedType>\r\n"
                ));
            }
        }
        let xml = format!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject \
             xmlns=\"http://v8.1c.ru/8.3/MDClasses\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" \
             xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{}\">\r\n\
             \t<{kind} uuid=\"{uuid}\">\r\n\t\t<InternalInfo>\r\n{types}\t\t</InternalInfo>\r\n\
             \t\t<Properties>\r\n\t\t\t<Name>{name}</Name>\r\n\t\t\t<Synonym/>\r\n\
             \t\t\t<Comment/>\r\n\t\t</Properties>\r\n\t</{kind}>\r\n</MetaDataObject>",
            source_version.as_str()
        );
        fs::create_dir_all(path.parent().expect("under the root")).context("stub directory")?;
        fs::write(&path, xml).with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

/// The configuration root a base-free compile of a new object reads
/// (`mssql::empty_stage::StageContext::for_objects`): only its uuid and
/// compatibility mode. An external object has no configuration of its own;
/// it is compiled as one of the platform its XML dialect names. Never
/// written over an exported root.
fn write_configuration_stub(
    root: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> Result<()> {
    let path = root.join("Configuration.xml");
    if path.exists() {
        return Ok(());
    }
    let compatibility = if source_version == InfobaseConfigSourceVersion::V2_21 {
        "Version8_5_1"
    } else {
        "Version8_3_27"
    };
    let xml = format!(
        "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject \
         xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"{}\">\r\n\
         \t<Configuration uuid=\"00000000-0000-0000-0000-000000000000\">\r\n\
         \t\t<Properties>\r\n\t\t\t<Name>External</Name>\r\n\
         \t\t\t<CompatibilityMode>{compatibility}</CompatibilityMode>\r\n\
         \t\t</Properties>\r\n\t</Configuration>\r\n</MetaDataObject>",
        source_version.as_str()
    );
    fs::write(&path, xml).context("failed to write the configuration stub")
}

/// A file of the object's folder with its references back in the
/// configuration spelling.
fn internal_bytes(path: &str, bytes: Vec<u8>, main: &header::ExternalMain) -> Vec<u8> {
    let Ok(text) = String::from_utf8(bytes.clone()) else {
        return bytes;
    };
    if path.ends_with(".xml") {
        rename::to_internal_references(&text, main.kind, &main.name).into_bytes()
    } else if path.ends_with(".html") {
        rename::to_internal_html_references(&text, main.kind, &main.name).into_bytes()
    } else {
        bytes
    }
}

/// The base's configuration-layout root XML with the edited external root's
/// properties and child objects: the properties the external XML leaves out
/// stay the base's, and so do the root's uuid and InternalInfo.
fn merged_root(base: &str, edited: &str, main: &header::ExternalMain) -> Result<String> {
    let edited = rename::to_internal_references(edited, main.kind, &main.name);
    let base_properties = properties(base).context("the base root XML has no <Properties>")?;
    let edited_properties =
        properties(&edited).context("the edited root XML has no <Properties>")?;
    let edited_by_name = edited_properties
        .iter()
        .cloned()
        .collect::<BTreeMap<_, _>>();
    let mut merged = String::new();
    for (tag, element) in &base_properties {
        if INTERNAL_ONLY.contains(&tag.as_str()) {
            merged.push_str(element);
        } else {
            merged.push_str(
                edited_by_name
                    .get(tag)
                    .with_context(|| format!("the edited root XML lost <{tag}>"))?,
            );
        }
    }
    let known = base_properties
        .iter()
        .map(|(tag, _)| tag.as_str())
        .collect::<BTreeSet<_>>();
    if let Some((tag, _)) = edited_properties
        .iter()
        .find(|(tag, _)| !known.contains(tag.as_str()))
    {
        anyhow::bail!("the edited root XML has a property <{tag}> the object has not");
    }
    let (open, close) = properties_span(base).expect("found above");
    let mut out = format!("{}{merged}{}", &base[..open], &base[close..]);
    // Child objects: the edited ones.
    let base_children =
        child_objects_span(&out).context("the base root XML has no child objects")?;
    let edited_children =
        child_objects_span(&edited).context("the edited root XML has no child objects")?;
    out.replace_range(
        base_children.0..base_children.1,
        &edited[edited_children.0..edited_children.1],
    );
    Ok(out)
}

/// The span of the text between `<Properties>` and `</Properties>`.
fn properties_span(xml: &str) -> Option<(usize, usize)> {
    let open = root_line(xml, 0, "<Properties>")? + "\t\t<Properties>".len();
    let close = root_line(xml, open, "</Properties>")?;
    Some((open, close))
}

/// Where the first line from `from` on that holds `tag` at the root's
/// children depth (two tabs) starts: a nested element's line (four tabs)
/// holds the same text after two of its tabs.
fn root_line(xml: &str, from: usize, tag: &str) -> Option<usize> {
    let needle = format!("\n\t\t{tag}");
    xml[from..].find(&needle).map(|at| from + at + 1)
}

/// The root's properties, `(tag, element text with its line)`, in order.
fn properties(xml: &str) -> Option<Vec<(String, String)>> {
    let (open, close) = properties_span(xml)?;
    let body = &xml[open..close];
    let mut out: Vec<(String, String)> = Vec::new();
    // Every property starts a line at three tabs; its text runs to the next.
    // A multi-line property's closing tag (`\t\t\t</Synonym>`) continues it.
    let mut starts = body
        .match_indices("\n\t\t\t<")
        .map(|(at, _)| at + 1)
        .filter(|at| !body[*at..].starts_with("\t\t\t</"))
        .collect::<Vec<_>>();
    starts.push(body.len());
    let lead = &body[..starts[0]];
    for pair in starts.windows(2) {
        let element = &body[pair[0]..pair[1]];
        let tag = element
            .trim_start()
            .trim_start_matches('<')
            .split(|c: char| c == '>' || c == '/' || c.is_whitespace())
            .next()?
            .to_owned();
        out.push((tag, element.to_owned()));
    }
    if let Some(first) = out.first_mut() {
        first.1.insert_str(0, lead);
    }
    Some(out)
}

/// The span of the root's `<ChildObjects>` element (self-closed or not).
fn child_objects_span(xml: &str) -> Option<(usize, usize)> {
    if let Some(at) = root_line(xml, 0, "<ChildObjects/>") {
        return Some((at, at + "\t\t<ChildObjects/>".len()));
    }
    let at = root_line(xml, 0, "<ChildObjects>")?;
    let end = root_line(xml, at, "</ChildObjects>")? + "\t\t</ChildObjects>".len();
    Some((at, end))
}

fn entries_text(archive: &ibcmd_cf::archive::CfArchive, key: &str) -> Result<String> {
    let entry = archive
        .image()
        .entries()
        .iter()
        .find(|entry| entry.logical_key().as_str().eq_ignore_ascii_case(key))
        .with_context(|| format!("the base has no entry {key}"))?;
    String::from_utf8(inflate_raw(entry.packed_payload())?).context("the main row is not UTF-8")
}

/// A scratch directory removed when dropped.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Result<Self> {
        let dir = std::env::temp_dir().join(format!(
            "ibcmd-load-external-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_child_objects_are_not_cut_at_a_tabular_sections_own() {
        let xml = "<MetaDataObject>\r\n\t<DataProcessor>\r\n\t\t<Properties>\r\n\t\t\t<Name>X</Name>\r\n\
                   \t\t</Properties>\r\n\t\t<ChildObjects>\r\n\t\t\t<TabularSection>\r\n\
                   \t\t\t\t<Properties>\r\n\t\t\t\t</Properties>\r\n\t\t\t\t<ChildObjects/>\r\n\
                   \t\t\t</TabularSection>\r\n\t\t\t<TabularSection>\r\n\t\t\t\t<ChildObjects>\r\n\
                   \t\t\t\t</ChildObjects>\r\n\t\t\t</TabularSection>\r\n\t\t\t<Form>Ф</Form>\r\n\
                   \t\t</ChildObjects>\r\n\t</DataProcessor>\r\n</MetaDataObject>";
        let (at, end) = child_objects_span(xml).unwrap();
        assert!(xml[at..end].starts_with("\t\t<ChildObjects>\r\n"));
        assert!(xml[at..end].ends_with("<Form>Ф</Form>\r\n\t\t</ChildObjects>"));
        let (open, close) = properties_span(xml).unwrap();
        assert_eq!(&xml[open..close], "\r\n\t\t\t<Name>X</Name>\r\n");
    }
}
