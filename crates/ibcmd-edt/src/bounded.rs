use crate::EdtError;
use ibcmd_xml::source_tree::{ReaderLimits, SourceEntry, SourcePath, SourceTree};
use quick_xml::{Reader, events::Event};
use std::fs;
use std::io::{BufRead, Read, Seek};
use std::path::Path;

const MAX_XML_DEPTH: usize = 128;
const MAX_XML_ATTRIBUTES: usize = 256;

/// Run before either XML parser builds a recursive tree. All source-derived names
/// that borrowed codecs can use as filenames must be portable components.
pub(crate) fn validate_xml(path: &str, bytes: &[u8]) -> Result<(), EdtError> {
    validate_xml_reader(path, std::io::Cursor::new(bytes))
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum PreflightPolicy {
    Legacy,
    DiskSource,
}
pub(crate) fn validate_xml_reader<R: BufRead + Seek>(path: &str, input: R) -> Result<(), EdtError> {
    validate_xml_with_policy(path, input, PreflightPolicy::Legacy)
}
pub(crate) fn validate_xml_source_reader<R: BufRead + Seek>(
    path: &str,
    input: R,
) -> Result<(), EdtError> {
    validate_xml_with_policy(path, input, PreflightPolicy::DiskSource)
}
fn validate_xml_with_policy<R: BufRead + Seek>(
    path: &str,
    mut input: R,
    policy: PreflightPolicy,
) -> Result<(), EdtError> {
    let origin = input.stream_position().map_err(EdtError::source)?;
    ibcmd_xml::XmlReader::inspect_reader(&mut input).map_err(EdtError::source)?;
    input
        .seek(std::io::SeekFrom::Start(origin))
        .map_err(EdtError::source)?;
    let mut reader = Reader::from_reader(input);
    let mut buffer = Vec::new();
    reader.config_mut().check_end_names = true;
    let mut depth = 0usize;
    let mut names = Vec::new();
    let mut form_body = false;
    let mut dump_info = false;
    let mut streamed_mxl = false;
    // Directory source inspection is iterative. Complete lexical validation
    // and typed filename checks are independent of the legacy memory parser's
    // recursive-shape quotas. Typed decoders apply their own explicit policy.
    // Opaque XML-backed templates, Rights and DCS contain arbitrary human labels.
    // Only these known document roles derive physical paths from name facets.
    let mut metadata_names = path.ends_with(".mdo");
    let dump_path = matches!(
        path,
        "ConfigDumpInfo.xml" | ".ibcmd-provenance/xml/ConfigDumpInfo.xml"
    );
    let mut help_pages = false;
    let mut picture_files = false;
    let mut html_pages = false;
    let mut graph_items = false;
    let mut graph_item: Option<(usize, String, bool)> = None;
    loop {
        let before = reader.buffer_position();
        let event = reader
            .read_event_into(&mut buffer)
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
                if names.is_empty() && depth == 0 {
                    let name = e.name();
                    let raw = std::str::from_utf8(name.as_ref()).map_err(EdtError::source)?;
                    let key = raw.split_once(':').map_or_else(
                        || "xmlns".to_owned(),
                        |(prefix, _)| format!("xmlns:{prefix}"),
                    );
                    let mut uri = String::new();
                    for attribute in e.attributes() {
                        let attribute = attribute.map_err(EdtError::source)?;
                        if attribute.key.as_ref() == key.as_bytes() {
                            uri = attribute
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(EdtError::source)?
                                .into_owned();
                        }
                    }
                    metadata_names |= path.ends_with(".xml")
                        && local == "MetaDataObject"
                        && uri == "http://v8.1c.ru/8.3/MDClasses";
                    help_pages = path.ends_with(".xml") && local == "Help";
                    html_pages = path.ends_with(".htmldoc")
                        && local == "HtmlDocument"
                        && uri == "http://g5.1c.ru/v8/dt/html-document";
                    graph_items = (path.ends_with(".xml")
                        || path.ends_with(".scheme")
                        || path.ends_with(".flowchart"))
                        && local == "GraphicalSchema"
                        && uri == "http://v8.1c.ru/8.3/xcf/scheme";
                    picture_files = path.ends_with(".xml") && local == "ExtPicture"
                        || path.ends_with(".flowchart")
                        || path.ends_with(".geos")
                        || graph_items;
                }
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
                        // Complete lexical validation was streamed before this semantic preflight.
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
                    for a in e.attributes().take(if policy == PreflightPolicy::Legacy {
                        MAX_XML_ATTRIBUTES
                    } else {
                        usize::MAX
                    }) {
                        let a = a.map_err(EdtError::source)?;
                        if a.key.as_ref() == b"xmlns" {
                            dump_info = a
                                .decode_and_unescape_value(reader.decoder())
                                .map_err(EdtError::source)?
                                == "http://v8.1c.ru/8.3/xcf/dumpinfo";
                        }
                    }
                }
                if graph_items && names.len() == 2 && names[1] == "Items" {
                    graph_item = Some((3, String::new(), false));
                }
                if let Some((_, _, has_picture)) = graph_item.as_mut()
                    && names.len() == 4
                    && names[3] == "Properties"
                    && local == "Picture"
                {
                    *has_picture = true;
                }
                for (n, a) in e.attributes().enumerate() {
                    if policy == PreflightPolicy::Legacy && !streamed_mxl && n >= MAX_XML_ATTRIBUTES
                    {
                        return Err(EdtError::new(format!(
                            "{path}: XML attribute budget exceeded"
                        )));
                    }
                    let a = a.map_err(EdtError::source)?;
                    if html_pages && local == "pages" && a.key.as_ref() == b"lang" {
                        component(
                            &a.decode_and_unescape_value(reader.decoder())
                                .map_err(EdtError::source)?,
                        )?;
                    }
                    if !streamed_mxl
                        && (metadata_names || form_body || dump_path)
                        && matches!(a.key.as_ref(), b"name" | b"Name" | b"lang")
                    {
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
                    depth = depth
                        .checked_add(1)
                        .ok_or_else(|| EdtError::new(format!("{path}: XML depth overflow")))?;
                    if policy == PreflightPolicy::Legacy && !streamed_mxl && depth > MAX_XML_DEPTH {
                        return Err(EdtError::new(format!("{path}: XML depth budget exceeded")));
                    }
                    names.push(local);
                }
            }
            Event::Text(e) => {
                let value = e.decode().map_err(EdtError::source)?;
                let value = quick_xml::escape::unescape(&value).map_err(EdtError::source)?;
                let leaf = names.last().map(String::as_str);
                if let Some((_, name, _)) = graph_item.as_mut()
                    && names.len() == 5
                    && names[3] == "Properties"
                    && leaf == Some("Name")
                {
                    name.push_str(&value);
                }
                let derived_name = (metadata_names || form_body)
                    && matches!(leaf, Some("Name" | "name"))
                    || metadata_names
                        && (matches!(leaf, Some("LanguageCode" | "languageCode" | "lang"))
                            || names
                                .iter()
                                .rev()
                                .nth(1)
                                .is_some_and(|parent| parent == "ChildObjects"))
                    || help_pages && leaf == Some("Page")
                    || picture_files && leaf == Some("Abs");
                if !streamed_mxl && derived_name {
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
                    && metadata_names
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
                if graph_item
                    .as_ref()
                    .is_some_and(|(item_depth, _, _)| *item_depth == names.len())
                {
                    let (_, name, has_picture) = graph_item.take().expect("checked graph item");
                    if has_picture {
                        component(name.trim())?;
                    }
                }
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
        buffer.clear();
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
    ibcmd_xml::source_tree::validate_source_path_safety(value).map_err(EdtError::source)?;
    Ok(())
}

// These names are logical identities, not payload allocations or paths with
// a platform-independent byte ceiling. Keep the same portable syntax checks;
// actual filesystem operations determine the physical component limit.
fn logical_form_name(value: &str) -> Result<(), EdtError> {
    component(value)
}

/// XML is declared by the adapter document role, never guessed from payload.
/// Raw carriers receive no acceptance exemption: full typed read/re-emission
/// and complete accounting must claim their bytes before conversion succeeds.
pub(crate) fn declared_xml(path: &str) -> bool {
    if path == ".project" {
        return true;
    }
    let path = path.strip_prefix(".ibcmd-provenance/xml/").unwrap_or(path);
    let path = path.strip_prefix("src/").unwrap_or(path);
    // The help codec claims the complete `_files` subtree as raw resources,
    // regardless of resource filename. Conversion still requires a real help
    // owner/page and byte-complete read/re-emission; an orphan is never accepted.
    if path.starts_with("Help/_files/") || path.contains("/Help/_files/") {
        return false;
    }
    let parts = path.split('/').collect::<Vec<_>>();
    if matches!(parts.as_slice(), ["CommonPictures", _, "Ext", "Picture", _])
        || matches!(parts.as_slice(), ["CommonPictures", _, file] if file.starts_with("Picture.") && !file.ends_with(".mdo"))
    {
        // Raw image bytes still require their real owner/wrapper, typed image
        // membership and complete byte-for-byte read/re-emission accounting.
        return false;
    }
    if matches!(
        path,
        "Ext/StandaloneConfigurationContent.bin" | "Configuration/MobileApplicationContent.scc"
    ) {
        return true;
    }
    matches!(
        path.rsplit('.')
            .next()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some(
            "xml"
                | "mdo"
                | "form"
                | "rights"
                | "dcs"
                | "style"
                | "xdto"
                | "geos"
                | "flowchart"
                | "scheme"
                | "htmldoc"
                | "cmi"
                | "cai"
                | "hpwa"
                | "dcss"
                | "dcssca"
                | "chart"
                | "mxlx"
                | "dcsat"
                | "schedule"
                | "wsdl"
                | "xsd"
        )
    )
}

pub(crate) fn validate_tree(tree: &SourceTree) -> Result<(), EdtError> {
    for e in tree.entries() {
        if declared_xml(e.path().as_str()) {
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
            if declared_xml(&name) {
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
    fn source_operation_declared_xml_has_no_generic_shape_ceiling_and_keeps_all_guards() {
        let mut xml = String::from(
            "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses'><Catalog><Properties><Name>Owner</Name><Future",
        );
        for index in 0..300 {
            xml.push_str(&format!(" a{index}='value'"));
        }
        xml.push('>');
        for _ in 0..4096 {
            xml.push_str("<nested>");
        }
        for _ in 0..4096 {
            xml.push_str("</nested>");
        }
        xml.push_str("</Future></Properties></Catalog></MetaDataObject>");
        assert!(validate_xml("Catalogs/Owner.xml", xml.as_bytes()).is_err());
        validate_xml_source_reader("Catalogs/Owner.xml", std::io::Cursor::new(xml.as_bytes()))
            .unwrap();
        for invalid in [
            xml.replace("<Name>Owner</Name>", "<Name>../escape</Name>"),
            xml.replace("</Future>", "</different>"),
            xml.replace("<Name>Owner</Name>", "<Name>&unknown;</Name>"),
        ] {
            assert!(
                validate_xml_source_reader(
                    "Catalogs/Owner.xml",
                    std::io::Cursor::new(invalid.as_bytes())
                )
                .is_err()
            );
        }
        // Source semantic checks must inspect attributes beyond the old ceiling,
        // rather than silently treating a truncated prefix as the whole input.
        let mut form = String::from("<Form><item");
        for index in 0..300 {
            form.push_str(&format!(" a{index}='value'"));
        }
        form.push_str(" name='../escape'/></Form>");
        assert!(
            validate_xml_source_reader("Form.form", std::io::Cursor::new(form.as_bytes())).is_err()
        );
    }
    #[test]
    fn source_operation_opaque_assets_keep_complete_xml_without_shape_quotas() {
        let mut xml = String::from("<payload");
        for index in 0..300 {
            xml.push_str(&format!(" a{index}='value'"));
        }
        xml.push('>');
        for _ in 0..400 {
            xml.push_str("<nested>");
        }
        xml.push_str("<name>../human label</name>");
        for _ in 0..400 {
            xml.push_str("</nested>");
        }
        xml.push_str("</payload>");
        let asset = "DataProcessors/P/Templates/T/Ext/Template.bin";
        assert!(validate_xml(asset, xml.as_bytes()).is_err()); // legacy quota remains explicit.
        validate_xml_source_reader(asset, std::io::Cursor::new(xml.as_bytes())).unwrap();
        assert!(
            validate_xml_source_reader("Descriptor.mdo", std::io::Cursor::new(xml.as_bytes()))
                .is_err()
        );
        let corrupt = xml.replace("</payload>", "</different>");
        assert!(
            validate_xml_source_reader(asset, std::io::Cursor::new(corrupt.as_bytes())).is_err()
        );
        assert!(
            validate_xml_source_reader(
                asset,
                std::io::Cursor::new(b"<payload>&unknown;</payload>".as_slice())
            )
            .is_err()
        );
    }
    #[test]
    fn path_derived_names_use_lexical_safety_not_utf8_byte_quotas() {
        let physical = "Имя".repeat(50); //150 UTF16 characters /300 UTF8 bytes.
        assert!(SourcePath::new(&physical).is_err());
        component(&physical).unwrap();
        let form = format!("<Form><items name='{}'/></Form>", "Имя".repeat(1000));
        validate_xml("Form.form", form.as_bytes()).unwrap();
        for name in ["../escape", "a/b", "a\\b", "a:stream", "NUL", "trailing."] {
            assert!(component(name).is_err(), "{name}");
        }
    }
    #[test]
    fn filename_checks_apply_to_path_deriving_roles_only() {
        let opaque = b"<dicMessageTypeResponse xmlns='http://www.fss.ru/integration/types/sedo/arm/v01'><dicList><dic><name xmlns='http://www.fss.ru/integration/types/common/v01'>human / label: ../free</name></dic></dicList></dicMessageTypeResponse>";
        validate_xml("DataProcessors/X/Templates/T/Ext/Template.bin", opaque).unwrap();
        for body in [
            b"<Rights><name>../human label</name></Rights>".as_slice(),
            b"<DataCompositionSchema><name>../query label</name></DataCompositionSchema>",
        ] {
            validate_xml("Ext/Template.xml", body).unwrap();
        }
        for (path, body) in [
            (
                "Catalogs/C.xml",
                "<MetaDataObject xmlns='http://v8.1c.ru/8.3/MDClasses'><Catalog><Properties><Name>../escape</Name></Properties></Catalog></MetaDataObject>",
            ),
            (
                "Catalogs/C/C.mdo",
                "<mdclass:Catalog xmlns:mdclass='http://g5.1c.ru/v8/dt/metadata/mdclass'><name>../escape</name></mdclass:Catalog>",
            ),
            ("Form.form", "<Form><items name='../escape'/></Form>"),
            (
                "Ext/Form.xml",
                "<Form><Attributes><Attribute name='../escape'/></Attributes></Form>",
            ),
            ("Ext/Help.xml", "<Help><Page>../escape</Page></Help>"),
            (
                "Ext/Picture.xml",
                "<ExtPicture><Picture><Abs>../escape.png</Abs></Picture></ExtPicture>",
            ),
        ] {
            assert!(validate_xml(path, body.as_bytes()).is_err(), "{path}");
        }
        assert!(validate_xml("Template.htmldoc", b"<h:HtmlDocument xmlns:h='http://g5.1c.ru/v8/dt/html-document'><pages lang='../escape'/></h:HtmlDocument>").is_err());
        let graph = "<GraphicalSchema xmlns='http://v8.1c.ru/8.3/xcf/scheme'><Items><Decoration><Properties><Name>../label</Name>{picture}</Properties></Decoration></Items></GraphicalSchema>";
        validate_xml(
            "Ext/Template.xml",
            graph.replace("{picture}", "").as_bytes(),
        )
        .unwrap();
        assert!(
            validate_xml(
                "Ext/Template.xml",
                graph
                    .replace("{picture}", "<Picture><Abs>Picture.png</Abs></Picture>")
                    .as_bytes()
            )
            .is_err()
        );
        // The scoped name rule does not weaken complete syntax/entity checks.
        assert!(validate_xml("Ext/Template.bin", b"<a><name>safe</name></different>").is_err());
        assert!(validate_xml("Ext/Template.bin", b"<a>&unknown;</a>").is_err());
    }
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
        for unsafe_name in [
            format!("{name}/escape"),
            format!("{name}\\escape"),
            format!("{name}:escape"),
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
        for name in [
            format!("{name}/escape"),
            format!("{name}\\escape"),
            format!("{name}:escape"),
        ] {
            let form = format!("<Form><name>{name}</name></Form>");
            assert!(validate_xml("Form.form", form.as_bytes()).is_err());
        }
        assert!(SourcePath::new(name).is_err());
    }
}
