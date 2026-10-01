use crate::EdtError;
use ibcmd_xml::source_tree::{ReaderLimits, SourceEntry, SourcePath, SourceTree};
use quick_xml::{Reader, events::Event};
use std::fs;
use std::io::Read;
use std::path::Path;

const MAX_XML_DEPTH: usize = 128;
const MAX_XML_ATTRIBUTES: usize = 256;

/// Run before either XML parser builds a recursive tree. All source-derived names
/// that borrowed codecs can use as filenames must be portable components.
pub(crate) fn validate_xml(path: &str, bytes: &[u8]) -> Result<(), EdtError> {
    let mut reader = Reader::from_reader(bytes);
    reader.config_mut().check_end_names = true;
    let mut depth = 0usize;
    let mut names = Vec::new();
    let mut form_body = false;
    let mut dump_info = false;
    let mut streamed_mxl = false;
    loop {
        let before = reader.buffer_position();
        let event = reader
            .read_event()
            .map_err(|e| EdtError::new(format!("{path}: {e}")))?;
        // A streaming scan is bounded by its input, not by the number of cells
        // or XML events. Every non-EOF event must consume source bytes, so a
        // large valid asset cannot hit an arbitrary event-count ceiling.
        if !matches!(event, Event::Eof) && reader.buffer_position() <= before {
            return Err(EdtError::new(format!(
                "{path}: XML reader made no progress"
            )));
        }
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                if names.is_empty() && depth == 0 && local == "document" {
                    let name = e.name();
                    let raw = std::str::from_utf8(name.as_ref()).map_err(EdtError::source)?;
                    let namespace_key = raw
                        .split_once(':')
                        .map_or_else(|| "xmlns".to_owned(), |(p, _)| format!("xmlns:{p}"));
                    for a in e.attributes() {
                        let a = a.map_err(EdtError::source)?;
                        if a.key.as_ref() == namespace_key.as_bytes() {
                            streamed_mxl = a
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(EdtError::source)?
                                == "http://v8.1c.ru/8.2/data/spreadsheet";
                        }
                    }
                    if streamed_mxl {
                        // The MXL codec is iterative and never derives physical
                        // filenames from cell text/attributes. Use the shared
                        // complete XML checks without recursive-model ceilings.
                        ibcmd_xml::XmlReader::inspect_slice(bytes).map_err(EdtError::source)?;
                    }
                }
                if names.is_empty() && depth == 0 && local == "Form" {
                    form_body = path.ends_with(".form") || path.ends_with("Form.xml");
                }
                if names.is_empty()
                    && depth == 0
                    && local == "ConfigDumpInfo"
                    && matches!(
                        path,
                        "ConfigDumpInfo.xml" | ".ibcmd-provenance/xml/ConfigDumpInfo.xml"
                    )
                {
                    for a in e.attributes().take(MAX_XML_ATTRIBUTES) {
                        let a = a.map_err(EdtError::source)?;
                        if a.key.as_ref() == b"xmlns" {
                            dump_info = a
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(EdtError::source)?
                                == "http://v8.1c.ru/8.3/xcf/dumpinfo";
                        }
                    }
                }
                for (n, a) in e.attributes().enumerate() {
                    if !streamed_mxl && n >= MAX_XML_ATTRIBUTES {
                        return Err(EdtError::new(format!(
                            "{path}: XML attribute budget exceeded"
                        )));
                    }
                    let a = a.map_err(EdtError::source)?;
                    if !streamed_mxl && matches!(a.key.as_ref(), b"name" | b"Name" | b"lang") {
                        let value = a
                            .decode_and_unescape_value(reader.decoder())
                            .map_err(EdtError::source)?;
                        if form_body && matches!(a.key.as_ref(), b"name" | b"Name") {
                            logical_form_name(&value)?;
                        } else if dump_info
                            && a.key.as_ref() == b"name"
                            && local == "Metadata"
                            && names.get(1).map(String::as_str) == Some("ConfigVersions")
                            && names[2..].iter().all(|name| name == "Metadata")
                        {
                            // Dump manifest references are qualified logical
                            // identities, never filesystem components.
                            logical_form_name(&value)?;
                        } else {
                            component(&value)?;
                        }
                    }
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                    if !streamed_mxl && depth > MAX_XML_DEPTH {
                        return Err(EdtError::new(format!("{path}: XML depth budget exceeded")));
                    }
                    names.push(local);
                }
            }
            Event::Text(e) => {
                let value = e.decode().map_err(EdtError::source)?;
                let value = quick_xml::escape::unescape(&value).map_err(EdtError::source)?;
                if !streamed_mxl
                    && (matches!(
                        names.last().map(String::as_str),
                        Some(
                            "Name"
                                | "name"
                                | "LanguageCode"
                                | "languageCode"
                                | "lang"
                                | "Page"
                                | "Abs"
                        )
                    ) || names
                        .iter()
                        .rev()
                        .nth(1)
                        .is_some_and(|p| p == "ChildObjects"))
                {
                    let check = if form_body
                        && matches!(names.last().map(String::as_str), Some("Name" | "name"))
                    {
                        logical_form_name(value.trim())
                    } else {
                        component(value.trim())
                    };
                    check.map_err(|e| {
                        EdtError::new(format!("{path}: unsafe metadata filename: {e}"))
                    })?;
                }
                if !streamed_mxl
                    && matches!(
                        names.last().map(String::as_str),
                        Some("parentSubsystem" | "ParentSubsystem")
                    )
                {
                    // Qualified subsystem identities are dotted; no filesystem escape.
                    if value.contains('/')
                        || value.contains('\\')
                        || value.contains(':')
                        || value.contains("..")
                    {
                        return Err(EdtError::new(format!(
                            "{path}: unsafe subsystem parent reference"
                        )));
                    }
                }
            }
            Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| EdtError::new(format!("{path}: unmatched XML end")))?;
                names.pop();
            }
            Event::DocType(_) | Event::CData(_) => {
                // Typed morph codecs do not account for entity declarations or CDATA.
                return Err(EdtError::new(format!(
                    "{path}: DTD/CDATA requires an explicit codec"
                )));
            }
            Event::Eof => {
                if depth != 0 {
                    return Err(EdtError::new(format!("{path}: unclosed XML")));
                }
                break;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn component(value: &str) -> Result<(), EdtError> {
    if value.is_empty() {
        return Ok(());
    }
    if value.contains(['/', '\\']) {
        return Err(EdtError::new("name is not a single path component"));
    }
    SourcePath::new(value).map_err(EdtError::source)?;
    Ok(())
}

// Form identifiers and dump references can exceed portable filename limits
// without naming a file.
// Only bounded long identifiers receive this exception; physical paths keep
// SourcePath limits and borrowed file writes independently check components.
fn logical_form_name(value: &str) -> Result<(), EdtError> {
    if value.len() <= ibcmd_xml::source_tree::MAX_SOURCE_COMPONENT_BYTES {
        return component(value);
    }
    if value.len() > 4096
        || value.ends_with('.')
        || value.contains("..")
        || value
            .chars()
            .any(|c| !c.is_alphanumeric() && !matches!(c, '_' | '-' | '.'))
    {
        return Err(EdtError::new("unsafe or unbounded logical form identifier"));
    }
    Ok(())
}

fn structured(path: &str, bytes: &[u8]) -> bool {
    if path == ".project" {
        return true;
    }
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "bsl" | "html" | "json") {
        return false;
    }
    matches!(
        ext.as_str(),
        "xml" | "mdo" | "form" | "rights" | "dcs" | "style" | "xdto" | "geos" | "flowchart"
    ) || std::str::from_utf8(bytes).is_ok_and(|s| {
        s.trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with("<?xml")
    })
}

pub(crate) fn validate_tree(tree: &SourceTree) -> Result<(), EdtError> {
    for e in tree.entries() {
        if structured(e.path().as_str(), e.bytes()) {
            validate_xml(e.path().as_str(), e.bytes())?;
        }
    }
    Ok(())
}

/// Unlike the general XML inventory, EDT inventory never skips editor/build
/// directories silently. Unsupported entries are rejected by codec accounting.
pub(crate) fn read_tree(root: &Path, limits: ReaderLimits) -> Result<SourceTree, EdtError> {
    let limits = limits.validate().map_err(EdtError::source)?;
    let metadata = fs::symlink_metadata(root).map_err(EdtError::source)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || reparse(&metadata) {
        return Err(EdtError::new("project root must be an ordinary directory"));
    }
    let mut state = State {
        limits,
        files: Vec::new(),
        dirs: 1,
        bytes: 0,
    };
    visit(root, root, 0, &mut state)?;
    SourceTree::new(state.files).map_err(EdtError::source)
}
struct State {
    limits: ReaderLimits,
    files: Vec<SourceEntry>,
    dirs: usize,
    bytes: usize,
}
fn visit(root: &Path, dir: &Path, depth: usize, s: &mut State) -> Result<(), EdtError> {
    if depth > s.limits.depth {
        return Err(EdtError::new("project depth budget exceeded"));
    }
    // Bound directory enumeration itself, not just the subsequent recursion.
    let mut children = Vec::new();
    for e in fs::read_dir(dir).map_err(EdtError::source)? {
        let e = e.map_err(EdtError::source)?;
        if children.len() >= s.limits.files + s.limits.directories {
            return Err(EdtError::new("directory entry budget exceeded"));
        }
        children.push(e);
    }
    children.sort_by_key(|e| e.file_name());
    for e in children {
        let raw_name = e.file_name();
        let raw_name = raw_name
            .to_str()
            .ok_or_else(|| EdtError::new("non-UTF8 project filename"))?;
        component(raw_name)
            .map_err(|e| EdtError::new(format!("non-portable project filename: {e}")))?;
        let metadata = fs::symlink_metadata(e.path()).map_err(EdtError::source)?;
        if metadata.file_type().is_symlink()
            || reparse(&metadata)
            || (!metadata.is_file() && !metadata.is_dir())
        {
            return Err(EdtError::new(format!(
                "unsafe project entry {}",
                e.path().display()
            )));
        }
        let path = e.path();
        let relative = path.strip_prefix(root).map_err(EdtError::source)?;
        let name = relative
            .to_str()
            .ok_or_else(|| EdtError::new("non-UTF8 project path"))?
            .replace('\\', "/");
        SourcePath::new(&name).map_err(EdtError::source)?;
        if metadata.is_dir() {
            s.dirs += 1;
            if s.dirs > s.limits.directories {
                return Err(EdtError::new("project directory budget exceeded"));
            }
            visit(root, &path, depth + 1, s)?;
        } else {
            if s.files.len() >= s.limits.files {
                return Err(EdtError::new("project file budget exceeded"));
            }
            let remaining = s
                .limits
                .total_bytes
                .checked_sub(s.bytes)
                .ok_or_else(|| EdtError::new("project total byte budget exceeded"))?;
            let cap = s.limits.asset_bytes.min(remaining);
            if metadata.len() > cap as u64 {
                return Err(EdtError::new(format!(
                    "{name}: project byte budget exceeded"
                )));
            }
            let mut data = Vec::new();
            fs::File::open(path)
                .map_err(EdtError::source)?
                .take(cap as u64 + 1)
                .read_to_end(&mut data)
                .map_err(EdtError::source)?;
            if data.len() > cap {
                return Err(EdtError::new(format!(
                    "{name}: project byte budget exceeded"
                )));
            }
            s.bytes += data.len();
            if structured(&name, &data) {
                validate_xml(&name, &data)?;
            }
            s.files.push(
                SourceEntry::from_bytes(SourcePath::new(name).map_err(EdtError::source)?, data)
                    .map_err(EdtError::source)?,
            );
        }
    }
    Ok(())
}
#[cfg(windows)]
fn reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn reparse(_: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streamed_mxl_depth_and_cell_names_are_not_metadata_limits() {
        let mut xml = String::from("<document xmlns='http://v8.1c.ru/8.2/data/spreadsheet'");
        for i in 0..300 {
            xml.push_str(&format!(" a{i}='value'"));
        }
        xml.push('>');
        for _ in 0..400 {
            xml.push_str("<nested>");
        }
        xml.push_str("<Name>../ThisIsCellText</Name>");
        for _ in 0..400 {
            xml.push_str("</nested>");
        }
        xml.push_str("</document>");
        validate_xml("Template.mxlx", xml.as_bytes()).unwrap();
        assert!(
            validate_xml(
                "Descriptor.mdo",
                xml.replace("data/spreadsheet", "unknown").as_bytes()
            )
            .is_err()
        );
        for unsafe_xml in [
            "<!DOCTYPE document><document xmlns='http://v8.1c.ru/8.2/data/spreadsheet'/>",
            "<document xmlns='http://v8.1c.ru/8.2/data/spreadsheet'><![CDATA[text]]></document>",
            "<document xmlns='http://v8.1c.ru/8.2/data/spreadsheet'>&unknown;</document>",
        ] {
            assert!(validate_xml("Template.mxlx", unsafe_xml.as_bytes()).is_err());
        }
    }
    #[test]
    fn large_flat_xml_is_limited_by_input_not_event_count() {
        // More than two million events, the ceiling that rejected real MXL.
        let mut xml = String::from("<document>");
        for _ in 0..700_000 {
            xml.push_str("<cell>x</cell>");
        }
        xml.push_str("</document>");
        validate_xml("Template.xml", xml.as_bytes()).unwrap();

        // Reaching the end of a large input must still validate its final tag.
        let end = xml.rfind("</document>").unwrap();
        xml.truncate(end);
        xml.push_str("</different>");
        assert!(validate_xml("Template.xml", xml.as_bytes()).is_err());
        xml.truncate(end);
        assert!(validate_xml("Template.xml", xml.as_bytes()).is_err());
    }
    #[test]
    fn dump_qualified_references_do_not_extend_filesystem_names() {
        let name = "DataProcessor.СопоставлениеОбъектовИнформационныхБаз.TabularSection.ТаблицаАвтоматическиСопоставленныхОбъектов.Attribute.УникальныйИдентификаторПриемника";
        assert!(name.len() > ibcmd_xml::source_tree::MAX_SOURCE_COMPONENT_BYTES);
        let dump = |name: &str| {
            format!(
                "<ConfigDumpInfo xmlns='http://v8.1c.ru/8.3/xcf/dumpinfo'><ConfigVersions><Metadata name='Catalog.Parent'><Metadata name='{name}'/></Metadata></ConfigVersions></ConfigDumpInfo>"
            )
        };
        let body = dump(name);
        for path in [
            "ConfigDumpInfo.xml",
            ".ibcmd-provenance/xml/ConfigDumpInfo.xml",
        ] {
            validate_xml(path, body.as_bytes()).unwrap();
        }
        assert!(validate_xml("Descriptor.mdo", body.as_bytes()).is_err());
        assert!(
            validate_xml(
                "ConfigDumpInfo.xml",
                body.replace("http://v8.1c.ru/8.3/xcf/dumpinfo", "urn:unknown")
                    .as_bytes(),
            )
            .is_err()
        );
        for unsafe_name in [
            format!("{name}/escape"),
            format!("{name}\\escape"),
            format!("{name}:escape"),
            "а".repeat(4097),
        ] {
            assert!(validate_xml("ConfigDumpInfo.xml", dump(&unsafe_name).as_bytes()).is_err());
        }
        assert!(SourcePath::new(name).is_err());
    }
    #[test]
    fn logical_form_names_do_not_extend_filesystem_names() {
        let name = "ДлинноеИмяЭлемента".repeat(20);
        assert!(name.len() > 255);
        let form = format!("<Form><items name='{name}'/></Form>");
        assert!(validate_xml("Form.form", form.as_bytes()).is_ok());
        assert!(validate_xml("Descriptor.mdo", form.as_bytes()).is_err());
        for name in [
            format!("{name}/escape"),
            format!("{name}\\escape"),
            format!("{name}:escape"),
            "а".repeat(4097),
        ] {
            let form = format!("<Form><name>{name}</name></Form>");
            assert!(validate_xml("Form.form", form.as_bytes()).is_err());
        }
        assert!(SourcePath::new(name).is_err());
    }
}
