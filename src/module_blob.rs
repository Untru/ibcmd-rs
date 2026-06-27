use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::ops::Range;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use quick_xml::Reader;
use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesStart, Event};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::cli::{ModuleBlobPackArgs, VersionsBlobPatchArgs};

const V8_MAGIC_NUMBER: u32 = 0x7fff_ffff;
const V8_PAGE_SIZE: u32 = 512;
const FILE_HEADER_SIZE: usize = 16;
const BLOCK_HEADER_SIZE: usize = 31;
const ELEM_ADDR_SIZE: usize = 12;
const ELEM_HEADER_PREFIX_SIZE: usize = 20;
const DEFAULT_INFO: &[u8] = b"\xEF\xBB\xBF{3,1,0,\"\",0}";
const STD_PICTURE_USER_UUID: &str = "6ff3ddbd-56e3-4ddf-a5bf-048c1e2dfb2f";
const STD_PICTURE_INFORMATION_REGISTER_UUID: &str = "5b87ad1b-d8cc-43c1-b5c4-dc43613c518c";

#[derive(Debug, Serialize)]
pub struct ModuleBlobPackReport {
    pub text: PathBuf,
    pub output: PathBuf,
    pub base_blob: Option<PathBuf>,
    pub info_bytes: usize,
    pub text_bytes: usize,
    pub inner_bytes: usize,
    pub output_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PackedModuleBlob {
    pub blob: Vec<u8>,
    pub info_bytes: usize,
    pub text_bytes: usize,
    pub inner_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Serialize)]
pub struct VersionsBlobPatchReport {
    pub input: PathBuf,
    pub output: PathBuf,
    pub plain_bytes: usize,
    pub output_bytes: usize,
    pub output_sha256: String,
    pub replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Clone)]
pub struct PatchedVersionsBlob {
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
    pub replacements: Vec<VersionReplacement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommonModuleXmlProperties {
    pub uuid: String,
    pub name: String,
    pub synonyms: Vec<LocalizedString>,
    pub comment: String,
    pub global: bool,
    pub client_managed_application: bool,
    pub server: bool,
    pub external_connection: bool,
    pub client_ordinary_application: bool,
    pub server_call: bool,
    pub privileged: bool,
    pub return_values_reuse: ReturnValuesReuse,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalizedString {
    pub lang: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimpleMetadataXmlProperties {
    pub kind: String,
    pub uuid: String,
    pub name: String,
    pub synonyms: Vec<LocalizedString>,
    pub comment: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub enum ReturnValuesReuse {
    DontUse,
    DuringRequest,
    DuringSession,
}

#[derive(Debug, Clone)]
pub struct PackedCommonModuleMetadataBlob {
    pub properties: CommonModuleXmlProperties,
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PackedSimpleMetadataBlob {
    pub properties: SimpleMetadataXmlProperties,
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PackedStyleBodyBlob {
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PackedScheduleBlob {
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PackedRawDeflatedBlob {
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PackedExtPictureBlob {
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone)]
pub struct PackedHelpBlob {
    pub blob: Vec<u8>,
    pub plain_bytes: usize,
    pub output_sha256: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RoleRightsXml {
    set_for_new_objects: bool,
    objects: Vec<RoleObjectRightsXml>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RoleObjectRightsXml {
    name: String,
    rights: Vec<RoleRightXml>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RoleRightXml {
    name: String,
    value: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct CommandInterfaceXmlEntry {
    name: String,
    common: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ExchangePlanContentXmlItem {
    metadata: String,
    auto_record: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PredefinedDataXmlItem {
    id: String,
    name: String,
    code: String,
    description: String,
    is_folder: bool,
    children: Vec<PredefinedDataXmlItem>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FlowchartXmlItem {
    id: String,
    name: String,
    tab_order: String,
    explanation: Option<String>,
    task_description: Option<String>,
    events: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone)]
pub struct MetadataSourceContext {
    source_root: PathBuf,
}

impl MetadataSourceContext {
    pub fn new(source_root: PathBuf) -> Self {
        Self { source_root }
    }

    fn resolve_common_picture_uuid(&self, reference: &str) -> Result<String> {
        let name = reference
            .trim()
            .strip_prefix("CommonPicture.")
            .ok_or_else(|| anyhow!("unsupported CommonCommand Picture reference: {reference}"))?;
        let path = self
            .source_root
            .join("CommonPictures")
            .join(format!("{name}.xml"));
        let xml = fs::read(&path)
            .with_context(|| format!("failed to read CommonPicture XML {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)?;
        if properties.kind != "CommonPicture" {
            return Err(anyhow!(
                "expected CommonPicture XML at {}, got {}",
                path.display(),
                properties.kind
            ));
        }
        Ok(properties.uuid)
    }

    fn resolve_defined_type_type_id(&self, reference: &str) -> Result<String> {
        let name = defined_type_name_from_reference(reference)?;
        let path = self
            .source_root
            .join("DefinedTypes")
            .join(format!("{name}.xml"));
        let xml = fs::read(&path)
            .with_context(|| format!("failed to read DefinedType XML {}", path.display()))?;
        parse_defined_type_type_id(&xml, name)
            .with_context(|| format!("failed to resolve TypeId from {}", path.display()))
    }

    fn resolve_command_group_uuid(&self, reference: &str) -> Result<String> {
        let name = reference
            .trim()
            .strip_prefix("CommandGroup.")
            .ok_or_else(|| anyhow!("unsupported CommandGroup reference: {reference}"))?;
        let path = self
            .source_root
            .join("CommandGroups")
            .join(format!("{name}.xml"));
        let xml = fs::read(&path)
            .with_context(|| format!("failed to read CommandGroup XML {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)?;
        if properties.kind != "CommandGroup" {
            return Err(anyhow!(
                "expected CommandGroup XML at {}, got {}",
                path.display(),
                properties.kind
            ));
        }
        Ok(properties.uuid)
    }

    fn resolve_style_item_uuid(&self, reference: &str) -> Result<String> {
        let name = reference
            .trim()
            .strip_prefix("StyleItem.")
            .ok_or_else(|| anyhow!("unsupported StyleItem reference: {reference}"))?;
        let path = self
            .source_root
            .join("StyleItems")
            .join(format!("{name}.xml"));
        let xml = fs::read(&path)
            .with_context(|| format!("failed to read StyleItem XML {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)?;
        if properties.kind != "StyleItem" {
            return Err(anyhow!(
                "expected StyleItem XML at {}, got {}",
                path.display(),
                properties.kind
            ));
        }
        Ok(properties.uuid)
    }

    fn resolve_simple_metadata_uuid(
        &self,
        reference: &str,
        expected_kind: &str,
        folder: &str,
        prefix: &str,
    ) -> Result<String> {
        let name = reference
            .trim()
            .strip_prefix(prefix)
            .ok_or_else(|| anyhow!("unsupported {expected_kind} reference: {reference}"))?;
        let path = self.source_root.join(folder).join(format!("{name}.xml"));
        let xml = fs::read(&path)
            .with_context(|| format!("failed to read {expected_kind} XML {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)?;
        if properties.kind != expected_kind {
            return Err(anyhow!(
                "expected {expected_kind} XML at {}, got {}",
                path.display(),
                properties.kind
            ));
        }
        Ok(properties.uuid)
    }

    fn resolve_metadata_type_id(&self, reference: &str) -> Result<String> {
        let generated_type_name = reference
            .trim()
            .strip_prefix("cfg:")
            .unwrap_or_else(|| reference.trim());
        if is_defined_type_reference(reference) {
            return self.resolve_defined_type_type_id(reference);
        }
        let folder = metadata_type_source_folder(generated_type_name).ok_or_else(|| {
            anyhow!("unsupported metadata type reference for source resolution: {reference}")
        })?;
        let name = generated_type_name
            .split_once('.')
            .map(|(_, name)| name)
            .ok_or_else(|| anyhow!("invalid metadata type reference: {reference}"))?;
        let path = self.source_root.join(folder).join(format!("{name}.xml"));
        let xml = fs::read(&path)
            .with_context(|| format!("failed to read metadata XML {}", path.display()))?;
        parse_generated_type_type_id(&xml, generated_type_name)
            .with_context(|| format!("failed to resolve TypeId from {}", path.display()))
    }

    fn resolve_metadata_reference_uuid(&self, reference: &str) -> Result<String> {
        let reference = reference.trim();
        let (prefix, folder) = metadata_reference_source_folder(reference).ok_or_else(|| {
            anyhow!("unsupported metadata reference for source resolution: {reference}")
        })?;
        self.resolve_simple_metadata_uuid(reference, prefix, folder, &format!("{prefix}."))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionReplacement {
    pub name: String,
    pub old_uuid: String,
    pub new_uuid: String,
}

#[derive(Debug, Clone)]
struct ModuleElement {
    header: Vec<u8>,
    data: Vec<u8>,
}

#[derive(Debug, Clone)]
struct ParsedElement {
    name: String,
    header: Vec<u8>,
    data: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
struct BlockHeader {
    data_size: usize,
    page_size: usize,
    next_page_addr: u32,
}

pub fn pack_module_blob(args: &ModuleBlobPackArgs) -> Result<ModuleBlobPackReport> {
    let text = fs::read(&args.text)
        .with_context(|| format!("failed to read BSL text {}", args.text.display()))?;
    let base_blob = match &args.base_blob {
        Some(path) => Some(
            fs::read(path)
                .with_context(|| format!("failed to read base blob {}", path.display()))?,
        ),
        None => None,
    };
    let info = match &args.info_file {
        Some(path) => Some(
            fs::read(path).with_context(|| format!("failed to read info {}", path.display()))?,
        ),
        None => None,
    };
    let packed = pack_module_blob_bytes(&text, base_blob.as_deref(), info.as_deref())?;

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&args.output, &packed.blob)
        .with_context(|| format!("failed to write {}", args.output.display()))?;

    Ok(ModuleBlobPackReport {
        text: args.text.clone(),
        output: args.output.clone(),
        base_blob: args.base_blob.clone(),
        info_bytes: packed.info_bytes,
        text_bytes: packed.text_bytes,
        inner_bytes: packed.inner_bytes,
        output_bytes: packed.blob.len(),
        output_sha256: packed.output_sha256,
    })
}

pub fn pack_module_blob_bytes(
    text: &[u8],
    base_blob: Option<&[u8]>,
    info: Option<&[u8]>,
) -> Result<PackedModuleBlob> {
    let base_elements = match base_blob {
        Some(blob) => Some(read_base_elements_from_blob(blob)?),
        None => None,
    };

    let info = match (info, &base_elements) {
        (Some(bytes), _) => bytes.to_vec(),
        (None, Some(elements)) => elements
            .get("info")
            .map(|element| element.data.clone())
            .unwrap_or_else(|| DEFAULT_INFO.to_vec()),
        (None, None) => DEFAULT_INFO.to_vec(),
    };

    let info_header = base_elements
        .as_ref()
        .and_then(|elements| elements.get("info"))
        .map(|element| element.header.clone())
        .unwrap_or_else(|| make_element_header("info"));
    let text_header = base_elements
        .as_ref()
        .and_then(|elements| elements.get("text"))
        .map(|element| element.header.clone())
        .unwrap_or_else(|| make_element_header("text"));

    let inner = build_module_inner(&[
        ModuleElement {
            header: info_header,
            data: info.clone(),
        },
        ModuleElement {
            header: text_header,
            data: text.to_vec(),
        },
    ])?;
    let blob = deflate_raw(&inner)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedModuleBlob {
        blob,
        info_bytes: info.len(),
        text_bytes: text.len(),
        inner_bytes: inner.len(),
        output_sha256,
    })
}

pub fn unpack_module_blob_text(blob: &[u8]) -> Result<Vec<u8>> {
    let elements = read_base_elements_from_blob(blob)?;
    let text = elements
        .get("text")
        .ok_or_else(|| anyhow!("module blob does not contain text element"))?;
    Ok(text.data.clone())
}

pub fn pack_common_module_metadata_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedCommonModuleMetadataBlob> {
    let properties = parse_common_module_xml_properties(xml)?;
    let plain = inflate_raw(base_blob).context("failed to inflate common module metadata blob")?;
    let text = String::from_utf8(plain).context("metadata blob is not valid UTF-8")?;
    let patched = patch_common_module_metadata_text(text, &properties)?;
    let plain = patched.into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedCommonModuleMetadataBlob {
        properties,
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_simple_metadata_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedSimpleMetadataBlob> {
    pack_simple_metadata_blob_from_xml_with_source(base_blob, xml, None)
}

pub fn pack_simple_metadata_blob_from_xml_with_source(
    base_blob: &[u8],
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<PackedSimpleMetadataBlob> {
    let properties = parse_simple_metadata_xml_properties(xml)?;
    let plain = inflate_raw(base_blob).context("failed to inflate metadata blob")?;
    let text = String::from_utf8(plain).context("metadata blob is not valid UTF-8")?;
    let patched = match properties.kind.as_str() {
        "Constant" => {
            let constant = parse_constant_xml_properties(xml, source)?;
            patch_constant_metadata_text(text, &constant)?
        }
        "DefinedType" => {
            let defined_type = parse_defined_type_xml_properties(xml, source)?;
            patch_defined_type_metadata_text(text, &defined_type)?
        }
        "CommonCommand" => {
            let command = parse_common_command_xml_properties(xml, source)?;
            patch_common_command_metadata_text(text, &command)?
        }
        "CommandGroup" => {
            let command_group = parse_command_group_xml_properties(xml, source)?;
            patch_command_group_metadata_text(text, &command_group)?
        }
        _ => patch_simple_metadata_header_text(text, &properties)?,
    };
    let plain = patched.into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedSimpleMetadataBlob {
        properties,
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_style_body_blob_from_xml(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<PackedStyleBodyBlob> {
    let items = parse_style_body_xml_items(xml)?;
    let mut fields = Vec::with_capacity(items.len() + 3);
    fields.push("2".to_string());
    fields.push(items.len().to_string());
    for item in &items {
        fields.push(format_style_body_item(item, source)?);
    }
    fields.push("{0}".to_string());
    let plain = format!("{{{}}}", fields.join(",")).into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedStyleBodyBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

#[derive(Debug, Clone)]
struct StyleBodyXmlItem {
    name: String,
    value: StyleBodyXmlValue,
}

#[derive(Debug, Clone)]
enum StyleBodyXmlValue {
    Color(String),
    Font(BTreeMap<String, String>),
    Border(BTreeMap<String, String>),
}

fn parse_style_body_xml_items(xml: &[u8]) -> Result<Vec<StyleBodyXmlItem>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut items = Vec::<StyleBodyXmlItem>::new();
    let mut item_name = None::<String>;
    let mut color_text = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["Style"]) && local == "Item" {
                    item_name = xml_attr_value(&event, "name");
                    color_text = None;
                } else if path_ends_with(&path, &["Style", "Item"]) && local == "Color" {
                    color_text = Some(String::new());
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["Style", "Item"]) {
                    if local == "Font" {
                        let name = item_name
                            .clone()
                            .ok_or_else(|| anyhow!("Style Item name is missing"))?;
                        items.push(StyleBodyXmlItem {
                            name,
                            value: StyleBodyXmlValue::Font(xml_attrs_map(&event)),
                        });
                    } else if local == "Border" {
                        let name = item_name
                            .clone()
                            .ok_or_else(|| anyhow!("Style Item name is missing"))?;
                        items.push(StyleBodyXmlItem {
                            name,
                            value: StyleBodyXmlValue::Border(xml_attrs_map(&event)),
                        });
                    }
                }
            }
            Ok(Event::Text(text)) => {
                if path_ends_with(&path, &["Style", "Item", "Color"])
                    && let Some(value) = color_text.as_mut()
                {
                    let text = text.xml_content()?;
                    let text = unescape(text.as_ref())?;
                    value.push_str(text.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if path_ends_with(&path, &["Style", "Item", "Color"])
                    && let Some(value) = color_text.as_mut()
                {
                    value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if path_ends_with(&path, &["Style", "Item", "Color"])
                    && let Some(value) = color_text.as_mut()
                {
                    let text = if let Some(ch) = reference.resolve_char_ref()? {
                        ch.to_string()
                    } else {
                        let entity = reference.decode()?;
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                            .to_string()
                    };
                    value.push_str(&text);
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "Color" && path_ends_with(&path, &["Style", "Item", "Color"]) {
                    let name = item_name
                        .clone()
                        .ok_or_else(|| anyhow!("Style Item name is missing"))?;
                    let value = color_text.take().unwrap_or_default();
                    items.push(StyleBodyXmlItem {
                        name,
                        value: StyleBodyXmlValue::Color(value.trim().to_string()),
                    });
                } else if local == "Item" && path_ends_with(&path, &["Style", "Item"]) {
                    item_name = None;
                    color_text = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(items)
}

fn xml_attrs_map(event: &BytesStart<'_>) -> BTreeMap<String, String> {
    event
        .attributes()
        .with_checks(false)
        .filter_map(|attr| attr.ok())
        .filter_map(|attr| {
            let name = String::from_utf8_lossy(attr.key.local_name().as_ref()).to_string();
            let value = attr.unescape_value().ok().map(|value| value.into_owned())?;
            Some((name, value))
        })
        .collect()
}

fn format_style_body_item(
    item: &StyleBodyXmlItem,
    source: Option<&MetadataSourceContext>,
) -> Result<String> {
    let key = style_body_key_for_name(&item.name, source)?;
    let (kind, value) = match &item.value {
        StyleBodyXmlValue::Color(value) => ("0", format_style_body_color_value(value, source)?),
        StyleBodyXmlValue::Font(attrs) => ("1", format_style_body_font_value(attrs, source)?),
        StyleBodyXmlValue::Border(attrs) => ("2", format_style_body_border_value(attrs, source)?),
    };
    Ok(format!("{{{key},{kind},{value}}}"))
}

fn style_body_key_for_name(name: &str, source: Option<&MetadataSourceContext>) -> Result<String> {
    if let Some(code) = style_body_standard_code_for_name(name) {
        return Ok(format!("{{{code}}}"));
    }
    if name.starts_with("StyleItem.") {
        let source = source.ok_or_else(|| {
            anyhow!("source root is required to resolve Style body reference {name}")
        })?;
        let uuid = source.resolve_style_item_uuid(name)?;
        return Ok(format!("{{0,{uuid}}}"));
    }
    Err(anyhow!("unsupported Style Item name: {name}"))
}

fn style_body_ref_key(reference: &str, source: Option<&MetadataSourceContext>) -> Result<String> {
    let reference = reference.trim();
    let name = reference
        .strip_prefix("style:")
        .ok_or_else(|| anyhow!("unsupported Style reference: {reference}"))?;
    if style_body_standard_code_for_name(name).is_some() || name.starts_with("StyleItem.") {
        return style_body_key_for_name(name, source);
    }
    style_body_key_for_name(&format!("StyleItem.{name}"), source)
}

fn format_style_body_color_value(
    value: &str,
    source: Option<&MetadataSourceContext>,
) -> Result<String> {
    let value = value.trim();
    if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 {
            return Err(anyhow!("unsupported Style color literal: {value}"));
        }
        let red = u32::from_str_radix(&hex[0..2], 16)?;
        let green = u32::from_str_radix(&hex[2..4], 16)?;
        let blue = u32::from_str_radix(&hex[4..6], 16)?;
        let packed = red | (green << 8) | (blue << 16);
        return Ok(format!("{{4,0,{{{packed}}},0}}"));
    }
    if let Some(name) = value.strip_prefix("web:") {
        let code = style_body_web_color_code(name)
            .ok_or_else(|| anyhow!("unsupported Style web color: {value}"))?;
        return Ok(format!("{{4,2,{{{code}}},2}}"));
    }
    if value.starts_with("style:") {
        let key = style_body_ref_key(value, source)?;
        return Ok(format!("{{4,3,{key},3}}"));
    }
    Err(anyhow!("unsupported Style color value: {value}"))
}

fn format_style_body_font_value(
    attrs: &BTreeMap<String, String>,
    source: Option<&MetadataSourceContext>,
) -> Result<String> {
    let reference = attrs
        .get("ref")
        .map(|value| style_body_ref_key(value, source))
        .transpose()?
        .unwrap_or_else(|| "{-20}".to_string());
    let height = attrs
        .get("height")
        .map(|value| value.as_str())
        .unwrap_or("0");
    let bold = parse_optional_xml_bool(attrs.get("bold"))?;
    let italic = parse_optional_xml_bool(attrs.get("italic"))?;
    let underline = parse_optional_xml_bool(attrs.get("underline"))?;
    let strikeout = parse_optional_xml_bool(attrs.get("strikeout"))?;
    let scale = attrs
        .get("scale")
        .map(|value| value.as_str())
        .unwrap_or("100");
    if height == "0" && !bold && !italic && !underline && !strikeout && scale == "100" {
        return Ok(format!("{{8,2,0,{reference},1,100}}"));
    }
    let weight = if bold { 700 } else { 400 };
    Ok(format!(
        "{{8,2,{height},{reference},{weight},{italic},{underline},{strikeout},1,{scale}}}",
        italic = bool_code(italic),
        underline = bool_code(underline),
        strikeout = bool_code(strikeout),
    ))
}

fn format_style_body_border_value(
    attrs: &BTreeMap<String, String>,
    source: Option<&MetadataSourceContext>,
) -> Result<String> {
    let reference = attrs
        .get("ref")
        .map(|value| style_body_ref_key(value, source))
        .transpose()?
        .unwrap_or_else(|| "{-18}".to_string());
    Ok(format!("{{3,1,{reference},0,0,0}}"))
}

fn parse_optional_xml_bool(value: Option<&String>) -> Result<bool> {
    match value.map(|value| value.as_str()) {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(value) => Err(anyhow!("invalid XML boolean: {value}")),
        None => Ok(false),
    }
}

fn bool_code(value: bool) -> u8 {
    if value { 1 } else { 0 }
}

fn style_body_standard_code_for_name(name: &str) -> Option<i32> {
    match name {
        "FormBackColor" => Some(-1),
        "FormTextColor" => Some(-11),
        "ButtonBackColor" => Some(-3),
        "ButtonTextColor" => Some(-15),
        "FieldBackColor" => Some(-7),
        "FieldTextColor" => Some(-13),
        "FieldSelectionBackColor" => Some(-21),
        "FieldSelectedTextColor" => Some(-10),
        "FieldAlternativeBackColor" => Some(-14),
        "ToolTipBackColor" => Some(-23),
        "ToolTipTextColor" => Some(-24),
        "SpecialTextColor" => Some(-16),
        "NegativeTextColor" => Some(-17),
        "BorderColor" => Some(-22),
        "ReportHeaderBackColor" => Some(-25),
        "ReportGroup1BackColor" => Some(-26),
        "ReportGroup2BackColor" => Some(-27),
        "ReportLineColor" => Some(-28),
        "ControlBorder" => Some(-18),
        "TextFont" => Some(-20),
        "SmallTextFont" => Some(-30),
        "NormalTextFont" => Some(-31),
        "LargeTextFont" => Some(-32),
        "ExtraLargeTextFont" => Some(-33),
        "ButtonBorderColor" => Some(-34),
        "TableHeaderBackColor" => Some(-35),
        "TableHeaderTextColor" => Some(-36),
        "TableFooterBackColor" => Some(-37),
        "TableFooterTextColor" => Some(-38),
        _ => None,
    }
}

fn style_body_web_color_code(name: &str) -> Option<i32> {
    match name {
        "Black" => Some(8),
        "Blue" => Some(10),
        "Cream" => Some(20),
        "DarkBlue" => Some(23),
        "DarkRed" => Some(33),
        "DarkSlateGray" => Some(37),
        "FireBrick" => Some(44),
        "FloralWhite" => Some(45),
        "ForestGreen" => Some(46),
        "Gainsboro" => Some(48),
        "Gray" => Some(52),
        "Green" => Some(53),
        "HoneyDew" => Some(55),
        "LightCyan" => Some(67),
        "LightGoldenRod" => Some(68),
        "LightGoldenRodYellow" => Some(69),
        "LightGray" => Some(71),
        "LightPink" => Some(72),
        "LightYellow" => Some(79),
        "Maroon" => Some(84),
        "MintCream" => Some(97),
        "MistyRose" => Some(98),
        "Red" => Some(119),
        "RosyBrown" => Some(120),
        "Silver" => Some(128),
        "SlateBlue" => Some(130),
        "SteelBlue" => Some(134),
        "Violet" => Some(140),
        "VioletRed" => Some(141),
        "WhiteSmoke" => Some(144),
        "Yellow" => Some(145),
        _ => None,
    }
}

pub fn pack_schedule_blob_from_xml(xml: &[u8]) -> Result<PackedScheduleBlob> {
    let schedule = parse_schedule_xml(xml)?;
    let mut fields = Vec::with_capacity(16 + schedule.week_days.len() + schedule.months.len());
    fields.push(format_schedule_date(&schedule.begin_date)?);
    fields.push(format_schedule_date(&schedule.end_date)?);
    fields.push(format_schedule_time(&schedule.begin_time)?);
    fields.push(format_schedule_time(&schedule.end_time)?);
    fields.push(format_schedule_time(&schedule.completion_time)?);
    fields.push(schedule.completion_interval);
    fields.push(schedule.repeat_period_in_day);
    fields.push(schedule.repeat_pause);
    fields.push(schedule.week_days.len().to_string());
    fields.extend(schedule.week_days);
    fields.push(schedule.week_day_in_month);
    fields.push(schedule.day_in_month);
    fields.push(schedule.months.len().to_string());
    fields.extend(schedule.months);
    fields.push(schedule.weeks_period);
    fields.push(schedule.days_repeat_period);

    let plain = format!("{{{}}}", fields.join(",")).into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedScheduleBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_raw_deflated_blob_from_bytes(bytes: &[u8]) -> Result<PackedRawDeflatedBlob> {
    let blob = deflate_raw(bytes)?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: bytes.len(),
        output_sha256,
    })
}

pub fn pack_moxel_spreadsheet_blob_from_xml(xml: &[u8]) -> Result<PackedRawDeflatedBlob> {
    let spreadsheet = parse_spreadsheet_document_xml(xml)?;
    let column_count = spreadsheet.column_count.max(
        spreadsheet
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().map(|cell| cell.column_index + 1))
            .max()
            .unwrap_or(1),
    );
    let declared_columns = column_count.saturating_sub(1);
    let mut fields = vec![
        "8".to_string(),
        "1".to_string(),
        declared_columns.to_string(),
        r#"{"ru","ru",0,1,"ru","Русский","Русский",0}"#.to_string(),
        "{0}".to_string(),
        "{0}".to_string(),
    ];
    for row in &spreadsheet.rows {
        for row_index in row.expanded_indexes() {
            fields.push(row_index.to_string());
            fields.push(row_format_index_for_moxel(row.format_index).to_string());
            fields.push(row.cells.len().to_string());
            for cell in &row.cells {
                fields.push(cell.column_index.to_string());
                fields.push(format_spreadsheet_cell_for_moxel(cell));
            }
        }
    }
    fields.extend(format_spreadsheet_column_sets_for_moxel(&spreadsheet));
    if !spreadsheet.merges.is_empty() {
        fields.push(format_spreadsheet_merges_for_moxel(&spreadsheet.merges));
    }
    if !spreadsheet.areas.is_empty() {
        fields.push(format_spreadsheet_named_areas_for_moxel(&spreadsheet.areas));
    }
    if let Some(print_area) = &spreadsheet.print_area {
        fields.push(format_spreadsheet_area_bounds_for_moxel(print_area));
    }
    if let Some(print_settings) = &spreadsheet.print_settings {
        fields.push(format_spreadsheet_print_settings_for_moxel(print_settings));
    }
    fields.extend(format_spreadsheet_lines_for_moxel(&spreadsheet.lines));
    fields.extend(format_spreadsheet_formats_for_moxel(
        &spreadsheet,
        column_count,
    ));
    fields.extend(format_spreadsheet_fonts_for_moxel(&spreadsheet.fonts));
    fields.push("2".to_string());
    fields.push("{0,1}".to_string());

    let plain_body = format!("{{{}}}", fields.join(","));
    let plain = format!("MOXCEL\0\u{8}\0\u{1}\0\u{c}\0\u{feff}{plain_body}");
    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXml {
    column_count: usize,
    column_sets: Vec<SpreadsheetDocumentXmlColumnSet>,
    rows: Vec<SpreadsheetDocumentXmlRow>,
    merges: Vec<SpreadsheetDocumentXmlMerge>,
    areas: Vec<SpreadsheetDocumentXmlArea>,
    print_area: Option<SpreadsheetDocumentXmlArea>,
    print_settings: Option<SpreadsheetDocumentXmlPrintSettings>,
    default_format_index: Option<usize>,
    formats: Vec<SpreadsheetDocumentXmlFormat>,
    fonts: Vec<SpreadsheetDocumentXmlFont>,
    lines: Vec<SpreadsheetDocumentXmlLine>,
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlColumnSet {
    id: Option<String>,
    size: usize,
    columns: Vec<SpreadsheetDocumentXmlColumn>,
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlColumn {
    index: usize,
    format_index: usize,
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlRow {
    index: usize,
    index_to: Option<usize>,
    format_index: usize,
    columns_id: Option<String>,
    empty: bool,
    cells: Vec<SpreadsheetDocumentXmlCell>,
}

impl SpreadsheetDocumentXmlRow {
    fn expanded_indexes(&self) -> std::ops::RangeInclusive<usize> {
        let end = self.index_to.unwrap_or(self.index).max(self.index);
        self.index..=end
    }
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlCell {
    column_index: usize,
    format_index: usize,
    text: Option<String>,
    parameter: Option<String>,
    detail_parameter: Option<String>,
    empty_text: bool,
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlMerge {
    row: i32,
    column: i32,
    height: i32,
    width: i32,
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlArea {
    name: String,
    area_type: String,
    begin_column: i32,
    begin_row: i32,
    end_column: i32,
    end_row: i32,
    columns_id: Option<String>,
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlPrintSettings {
    page_orientation: Option<String>,
    scale: Option<usize>,
    collate: Option<bool>,
    copies: Option<usize>,
    per_page: Option<usize>,
    top_margin: Option<usize>,
    left_margin: Option<usize>,
    bottom_margin: Option<usize>,
    right_margin: Option<usize>,
    header_size: Option<usize>,
    footer_size: Option<usize>,
    fit_to_page: Option<bool>,
    black_and_white: Option<bool>,
    printer_name: Option<String>,
    paper: Option<usize>,
    paper_source: Option<usize>,
    page_width: Option<usize>,
    page_height: Option<usize>,
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlFormat {
    font: Option<usize>,
    border: Option<usize>,
    left_border: Option<usize>,
    top_border: Option<usize>,
    right_border: Option<usize>,
    bottom_border: Option<usize>,
    height: Option<usize>,
    border_color: Option<String>,
    width: Option<usize>,
    horizontal_alignment: Option<String>,
    vertical_alignment: Option<String>,
    back_color: Option<String>,
    text_color: Option<String>,
    text_placement: Option<String>,
    fill_type: Option<String>,
    drawing_border: Option<usize>,
    by_selected_columns: Option<bool>,
    details_use: Option<String>,
    hyper_link: Option<bool>,
    protection: Option<bool>,
    indent: Option<usize>,
    auto_indent: Option<usize>,
    mask: Option<String>,
    pic_index: Option<usize>,
    picture_size_mode: Option<String>,
    pic_horizontal_alignment: Option<String>,
    pic_vertical_alignment: Option<String>,
}

#[derive(Debug, Default)]
struct SpreadsheetDocumentXmlFont {
    ref_name: Option<String>,
    face_name: Option<String>,
    height: Option<usize>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikeout: bool,
    kind: String,
    scale: Option<usize>,
}

#[derive(Debug)]
struct SpreadsheetDocumentXmlLine {
    style: String,
    line_type: String,
    width: usize,
}

impl Default for SpreadsheetDocumentXmlLine {
    fn default() -> Self {
        Self {
            style: String::new(),
            line_type: "v8ui:SpreadsheetDocumentCellLineType".to_string(),
            width: 1,
        }
    }
}

fn parse_spreadsheet_document_xml(xml: &[u8]) -> Result<SpreadsheetDocumentXml> {
    let mut reader = Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut document = SpreadsheetDocumentXml::default();
    let mut current_column_set = None::<SpreadsheetDocumentXmlColumnSet>;
    let mut current_column = None::<SpreadsheetDocumentXmlColumn>;
    let mut current_row = None::<SpreadsheetDocumentXmlRow>;
    let mut current_cell = None::<SpreadsheetDocumentXmlCell>;
    let mut current_merge = None::<SpreadsheetDocumentXmlMerge>;
    let mut current_area = None::<SpreadsheetDocumentXmlArea>;
    let mut current_print_settings = None::<SpreadsheetDocumentXmlPrintSettings>;
    let mut current_format = None::<SpreadsheetDocumentXmlFormat>;
    let mut current_line = None::<SpreadsheetDocumentXmlLine>;
    let mut c_depth = 0usize;
    let mut next_column_index = 0usize;
    let mut text = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "columns" {
                    current_column_set = Some(SpreadsheetDocumentXmlColumnSet::default());
                } else if current_column_set.is_some() && local == "columnsItem" {
                    current_column = Some(SpreadsheetDocumentXmlColumn::default());
                } else if local == "rowsItem" {
                    current_row = Some(SpreadsheetDocumentXmlRow::default());
                    next_column_index = 0;
                } else if local == "merge" {
                    current_merge = Some(SpreadsheetDocumentXmlMerge::default());
                } else if local == "namedItem" {
                    current_area = Some(SpreadsheetDocumentXmlArea::default());
                } else if local == "printArea" {
                    current_area = Some(SpreadsheetDocumentXmlArea::default());
                } else if local == "printSettings" {
                    current_print_settings = Some(SpreadsheetDocumentXmlPrintSettings::default());
                } else if local == "format" {
                    current_format = Some(SpreadsheetDocumentXmlFormat::default());
                } else if local == "font"
                    && let Some(font) = parse_spreadsheet_font_xml_attributes(&event)?
                {
                    document.fonts.push(font);
                } else if local == "line" {
                    current_line = Some(parse_spreadsheet_line_xml_attributes(&event)?);
                } else if local == "style"
                    && let Some(line) = current_line.as_mut()
                {
                    if let Some(line_type) = xml_attribute_value(&event, "type")? {
                        line.line_type = line_type;
                    }
                } else if current_row.is_some() && local == "c" {
                    c_depth += 1;
                    if c_depth == 1 {
                        current_cell = Some(SpreadsheetDocumentXmlCell {
                            column_index: next_column_index,
                            ..Default::default()
                        });
                    }
                } else if current_cell.is_some() && local == "tl" {
                    if let Some(cell) = current_cell.as_mut() {
                        cell.empty_text = true;
                    }
                }
                if spreadsheet_text_element(&local) {
                    text.clear();
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "font" {
                    if let Some(font) = parse_spreadsheet_font_xml_attributes(&event)? {
                        document.fonts.push(font);
                    }
                } else if current_cell.is_some()
                    && local == "tl"
                    && let Some(cell) = current_cell.as_mut()
                {
                    cell.empty_text = true;
                }
            }
            Ok(Event::Text(event)) => {
                if path
                    .last()
                    .is_some_and(|part| spreadsheet_text_element(part))
                {
                    let value = event.xml_content()?;
                    let value = unescape(value.as_ref())?;
                    text.push_str(value.as_ref());
                }
            }
            Ok(Event::CData(event)) => {
                if path
                    .last()
                    .is_some_and(|part| spreadsheet_text_element(part))
                {
                    text.push_str(event.xml_content()?.as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if path
                    .last()
                    .is_some_and(|part| spreadsheet_text_element(part))
                {
                    let value = if let Some(ch) = reference.resolve_char_ref()? {
                        ch.to_string()
                    } else {
                        let entity = reference.decode()?;
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                            .to_string()
                    };
                    text.push_str(&value);
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                apply_spreadsheet_text_value(
                    &path,
                    &local,
                    &text,
                    &mut document,
                    current_column_set.as_mut(),
                    current_column.as_mut(),
                    current_row.as_mut(),
                    current_cell.as_mut(),
                    current_merge.as_mut(),
                    current_area.as_mut(),
                    current_print_settings.as_mut(),
                    current_format.as_mut(),
                    current_line.as_mut(),
                );
                if local == "c" && current_row.is_some() {
                    if c_depth == 1
                        && let Some(mut cell) = current_cell.take()
                    {
                        next_column_index = cell.column_index + 1;
                        normalize_spreadsheet_cell(&mut cell);
                        if let Some(row) = current_row.as_mut() {
                            row.cells.push(cell);
                        }
                    }
                    c_depth = c_depth.saturating_sub(1);
                } else if local == "rowsItem"
                    && let Some(mut row) = current_row.take()
                {
                    if row.empty {
                        row.cells.clear();
                    } else {
                        row.cells.sort_by_key(|cell| cell.column_index);
                    }
                    document.rows.push(row);
                } else if local == "columnsItem"
                    && let Some(column) = current_column.take()
                    && let Some(column_set) = current_column_set.as_mut()
                {
                    column_set.columns.push(column);
                } else if local == "columns"
                    && let Some(column_set) = current_column_set.take()
                {
                    document.column_count = document.column_count.max(column_set.size);
                    document.column_sets.push(column_set);
                } else if local == "merge"
                    && let Some(merge) = current_merge.take()
                {
                    document.merges.push(merge);
                } else if local == "namedItem"
                    && let Some(area) = current_area.take()
                {
                    document.areas.push(area);
                } else if local == "printArea"
                    && let Some(area) = current_area.take()
                {
                    document.print_area = Some(area);
                } else if local == "printSettings"
                    && let Some(print_settings) = current_print_settings.take()
                {
                    document.print_settings = Some(print_settings);
                } else if local == "format"
                    && let Some(format) = current_format.take()
                {
                    document.formats.push(format);
                } else if local == "line"
                    && let Some(line) = current_line.take()
                {
                    document.lines.push(line);
                }
                if spreadsheet_text_element(&local) {
                    text.clear();
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }
    if document.rows.is_empty() {
        return Err(anyhow!("SpreadsheetDocument XML has no rowsItem entries"));
    }
    document.rows.sort_by_key(|row| row.index);
    Ok(document)
}

fn parse_spreadsheet_font_xml_attributes(
    event: &BytesStart<'_>,
) -> Result<Option<SpreadsheetDocumentXmlFont>> {
    let mut font = SpreadsheetDocumentXmlFont::default();
    let mut seen = false;
    for attr in event.attributes() {
        let attr = attr?;
        let key = xml_local_name(attr.key.local_name().as_ref());
        let value = attr.unescape_value()?.into_owned();
        match key.as_str() {
            "ref" => {
                font.ref_name = Some(value);
                seen = true;
            }
            "faceName" => {
                font.face_name = Some(value);
                seen = true;
            }
            "height" => {
                if let Ok(height) = value.parse::<usize>() {
                    font.height = Some(height);
                    seen = true;
                }
            }
            "bold" => {
                font.bold = value.eq_ignore_ascii_case("true");
                seen = true;
            }
            "italic" => {
                font.italic = value.eq_ignore_ascii_case("true");
                seen = true;
            }
            "underline" => {
                font.underline = value.eq_ignore_ascii_case("true");
                seen = true;
            }
            "strikeout" => {
                font.strikeout = value.eq_ignore_ascii_case("true");
                seen = true;
            }
            "kind" => {
                font.kind = value;
                seen = true;
            }
            "scale" => {
                if let Ok(scale) = value.parse::<usize>() {
                    font.scale = Some(scale);
                    seen = true;
                }
            }
            _ => {}
        }
    }
    Ok(seen.then_some(font))
}

fn parse_spreadsheet_line_xml_attributes(
    event: &BytesStart<'_>,
) -> Result<SpreadsheetDocumentXmlLine> {
    let mut line = SpreadsheetDocumentXmlLine::default();
    if let Some(width) = xml_attribute_value(event, "width")?
        && let Ok(width) = width.parse::<usize>()
    {
        line.width = width;
    }
    Ok(line)
}

fn xml_attribute_value(event: &BytesStart<'_>, local_name: &str) -> Result<Option<String>> {
    for attr in event.attributes() {
        let attr = attr?;
        if xml_local_name(attr.key.local_name().as_ref()) == local_name {
            return Ok(Some(attr.unescape_value()?.into_owned()));
        }
    }
    Ok(None)
}

fn spreadsheet_text_element(local: &str) -> bool {
    matches!(
        local,
        "size"
            | "id"
            | "index"
            | "indexTo"
            | "formatIndex"
            | "i"
            | "f"
            | "content"
            | "parameter"
            | "detailParameter"
            | "empty"
            | "r"
            | "c"
            | "h"
            | "w"
            | "name"
            | "type"
            | "beginRow"
            | "endRow"
            | "beginColumn"
            | "endColumn"
            | "columnsID"
            | "pageOrientation"
            | "scale"
            | "collate"
            | "copies"
            | "perPage"
            | "topMargin"
            | "leftMargin"
            | "bottomMargin"
            | "rightMargin"
            | "headerSize"
            | "footerSize"
            | "fitToPage"
            | "blackAndWhite"
            | "printerName"
            | "paper"
            | "paperSource"
            | "pageWidth"
            | "pageHeight"
            | "defaultFormatIndex"
            | "font"
            | "border"
            | "leftBorder"
            | "topBorder"
            | "rightBorder"
            | "bottomBorder"
            | "height"
            | "borderColor"
            | "width"
            | "horizontalAlignment"
            | "verticalAlignment"
            | "backColor"
            | "textColor"
            | "textPlacement"
            | "fillType"
            | "drawingBorder"
            | "bySelectedColumns"
            | "detailsUse"
            | "hyperLink"
            | "protection"
            | "indent"
            | "autoIndent"
            | "mask"
            | "picIndex"
            | "pictureSizeMode"
            | "picHorizontalAlignment"
            | "picVerticalAlignment"
            | "style"
    )
}

fn apply_spreadsheet_text_value(
    path: &[String],
    local: &str,
    text: &str,
    document: &mut SpreadsheetDocumentXml,
    column_set: Option<&mut SpreadsheetDocumentXmlColumnSet>,
    column: Option<&mut SpreadsheetDocumentXmlColumn>,
    row: Option<&mut SpreadsheetDocumentXmlRow>,
    cell: Option<&mut SpreadsheetDocumentXmlCell>,
    merge: Option<&mut SpreadsheetDocumentXmlMerge>,
    area: Option<&mut SpreadsheetDocumentXmlArea>,
    print_settings: Option<&mut SpreadsheetDocumentXmlPrintSettings>,
    format: Option<&mut SpreadsheetDocumentXmlFormat>,
    line: Option<&mut SpreadsheetDocumentXmlLine>,
) {
    let value = text.trim();
    match local {
        "size" if path_ends_with(path, &["columns", "size"]) => {
            if let Ok(size) = value.parse::<usize>() {
                if let Some(column_set) = column_set {
                    column_set.size = size;
                }
                document.column_count = document.column_count.max(size);
            }
        }
        "id" if path_ends_with(path, &["columns", "id"]) => {
            if let Some(column_set) = column_set
                && !value.is_empty()
            {
                column_set.id = Some(text.to_string());
            }
        }
        "index" if path_ends_with(path, &["columns", "columnsItem", "index"]) => {
            if let Some(column) = column
                && let Ok(index) = value.parse::<usize>()
            {
                column.index = index;
            }
        }
        "index" if path_ends_with(path, &["rowsItem", "index"]) => {
            if let Some(row) = row
                && let Ok(index) = value.parse::<usize>()
            {
                row.index = index;
            }
        }
        "indexTo" if path_ends_with(path, &["rowsItem", "indexTo"]) => {
            if let Some(row) = row
                && let Ok(index_to) = value.parse::<usize>()
            {
                row.index_to = Some(index_to);
            }
        }
        "formatIndex" if path_ends_with(path, &["rowsItem", "row", "formatIndex"]) => {
            if let Some(row) = row
                && let Ok(format_index) = value.parse::<usize>()
            {
                row.format_index = format_index;
            }
        }
        "formatIndex"
            if path_ends_with(path, &["columns", "columnsItem", "column", "formatIndex"]) =>
        {
            if let Some(column) = column
                && let Ok(format_index) = value.parse::<usize>()
            {
                column.format_index = format_index;
            }
        }
        "columnsID" if path_ends_with(path, &["rowsItem", "row", "columnsID"]) => {
            if let Some(row) = row
                && !value.is_empty()
            {
                row.columns_id = Some(text.to_string());
            }
        }
        "empty" if path_ends_with(path, &["rowsItem", "row", "empty"]) => {
            if let Some(row) = row {
                row.empty = value.eq_ignore_ascii_case("true");
            }
        }
        "i" => {
            if let Some(cell) = cell
                && let Ok(index) = value.parse::<usize>()
            {
                cell.column_index = index;
            }
        }
        "f" => {
            if let Some(cell) = cell
                && let Ok(format_index) = value.parse::<usize>()
            {
                cell.format_index = format_index;
            }
        }
        "content" => {
            if let Some(cell) = cell {
                cell.text = Some(text.to_string());
                cell.empty_text = false;
            }
        }
        "parameter" => {
            if let Some(cell) = cell {
                cell.parameter = Some(text.to_string());
            }
        }
        "detailParameter" => {
            if let Some(cell) = cell {
                cell.detail_parameter = Some(text.to_string());
            }
        }
        "r" if path_ends_with(path, &["merge", "r"]) => {
            if let Some(merge) = merge
                && let Ok(row) = value.parse::<i32>()
            {
                merge.row = row;
            }
        }
        "c" if path_ends_with(path, &["merge", "c"]) => {
            if let Some(merge) = merge
                && let Ok(column) = value.parse::<i32>()
            {
                merge.column = column;
            }
        }
        "h" if path_ends_with(path, &["merge", "h"]) => {
            if let Some(merge) = merge
                && let Ok(height) = value.parse::<i32>()
            {
                merge.height = height.max(0);
            }
        }
        "w" if path_ends_with(path, &["merge", "w"]) => {
            if let Some(merge) = merge
                && let Ok(width) = value.parse::<i32>()
            {
                merge.width = width.max(0);
            }
        }
        "name" if path_ends_with(path, &["namedItem", "name"]) => {
            if let Some(area) = area {
                area.name = text.to_string();
            }
        }
        "type" if spreadsheet_area_property_path(path, "type") => {
            if let Some(area) = area {
                area.area_type = text.to_string();
            }
        }
        "beginRow" if spreadsheet_area_property_path(path, "beginRow") => {
            if let Some(area) = area
                && let Ok(begin_row) = value.parse::<i32>()
            {
                area.begin_row = begin_row;
            }
        }
        "endRow" if spreadsheet_area_property_path(path, "endRow") => {
            if let Some(area) = area
                && let Ok(end_row) = value.parse::<i32>()
            {
                area.end_row = end_row;
            }
        }
        "beginColumn" if spreadsheet_area_property_path(path, "beginColumn") => {
            if let Some(area) = area
                && let Ok(begin_column) = value.parse::<i32>()
            {
                area.begin_column = begin_column;
            }
        }
        "endColumn" if spreadsheet_area_property_path(path, "endColumn") => {
            if let Some(area) = area
                && let Ok(end_column) = value.parse::<i32>()
            {
                area.end_column = end_column;
            }
        }
        "columnsID" if path_ends_with(path, &["namedItem", "area", "columnsID"]) => {
            if let Some(area) = area
                && !value.is_empty()
            {
                area.columns_id = Some(text.to_string());
            }
        }
        "pageOrientation" if path_ends_with(path, &["printSettings", "pageOrientation"]) => {
            if let Some(print_settings) = print_settings
                && !value.is_empty()
            {
                print_settings.page_orientation = Some(text.to_string());
            }
        }
        "scale" if path_ends_with(path, &["printSettings", "scale"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.scale = Some(parsed)
            });
        }
        "collate" if path_ends_with(path, &["printSettings", "collate"]) => {
            set_spreadsheet_print_settings_bool(print_settings, value, |settings, parsed| {
                settings.collate = Some(parsed)
            });
        }
        "copies" if path_ends_with(path, &["printSettings", "copies"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.copies = Some(parsed)
            });
        }
        "perPage" if path_ends_with(path, &["printSettings", "perPage"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.per_page = Some(parsed)
            });
        }
        "topMargin" if path_ends_with(path, &["printSettings", "topMargin"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.top_margin = Some(parsed)
            });
        }
        "leftMargin" if path_ends_with(path, &["printSettings", "leftMargin"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.left_margin = Some(parsed)
            });
        }
        "bottomMargin" if path_ends_with(path, &["printSettings", "bottomMargin"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.bottom_margin = Some(parsed)
            });
        }
        "rightMargin" if path_ends_with(path, &["printSettings", "rightMargin"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.right_margin = Some(parsed)
            });
        }
        "headerSize" if path_ends_with(path, &["printSettings", "headerSize"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.header_size = Some(parsed)
            });
        }
        "footerSize" if path_ends_with(path, &["printSettings", "footerSize"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.footer_size = Some(parsed)
            });
        }
        "fitToPage" if path_ends_with(path, &["printSettings", "fitToPage"]) => {
            set_spreadsheet_print_settings_bool(print_settings, value, |settings, parsed| {
                settings.fit_to_page = Some(parsed)
            });
        }
        "blackAndWhite" if path_ends_with(path, &["printSettings", "blackAndWhite"]) => {
            set_spreadsheet_print_settings_bool(print_settings, value, |settings, parsed| {
                settings.black_and_white = Some(parsed)
            });
        }
        "printerName" if path_ends_with(path, &["printSettings", "printerName"]) => {
            if let Some(print_settings) = print_settings {
                print_settings.printer_name = Some(text.to_string());
            }
        }
        "paper" if path_ends_with(path, &["printSettings", "paper"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.paper = Some(parsed)
            });
        }
        "paperSource" if path_ends_with(path, &["printSettings", "paperSource"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.paper_source = Some(parsed)
            });
        }
        "pageWidth" if path_ends_with(path, &["printSettings", "pageWidth"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.page_width = Some(parsed)
            });
        }
        "pageHeight" if path_ends_with(path, &["printSettings", "pageHeight"]) => {
            set_spreadsheet_print_settings_usize(print_settings, value, |settings, parsed| {
                settings.page_height = Some(parsed)
            });
        }
        "defaultFormatIndex" if path_ends_with(path, &["defaultFormatIndex"]) => {
            if let Ok(parsed) = value.parse::<usize>() {
                document.default_format_index = Some(parsed);
            }
        }
        "font" if path_ends_with(path, &["format", "font"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.font = Some(parsed)
            });
        }
        "border" if path_ends_with(path, &["format", "border"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.border = Some(parsed)
            });
        }
        "leftBorder" if path_ends_with(path, &["format", "leftBorder"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.left_border = Some(parsed)
            });
        }
        "topBorder" if path_ends_with(path, &["format", "topBorder"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.top_border = Some(parsed)
            });
        }
        "rightBorder" if path_ends_with(path, &["format", "rightBorder"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.right_border = Some(parsed)
            });
        }
        "bottomBorder" if path_ends_with(path, &["format", "bottomBorder"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.bottom_border = Some(parsed)
            });
        }
        "height" if path_ends_with(path, &["format", "height"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.height = Some(parsed)
            });
        }
        "borderColor" if path_ends_with(path, &["format", "borderColor"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.border_color = Some(parsed)
            });
        }
        "width" if path_ends_with(path, &["format", "width"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.width = Some(parsed)
            });
        }
        "horizontalAlignment" if path_ends_with(path, &["format", "horizontalAlignment"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.horizontal_alignment = Some(parsed)
            });
        }
        "verticalAlignment" if path_ends_with(path, &["format", "verticalAlignment"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.vertical_alignment = Some(parsed)
            });
        }
        "backColor" if path_ends_with(path, &["format", "backColor"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.back_color = Some(parsed)
            });
        }
        "textColor" if path_ends_with(path, &["format", "textColor"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.text_color = Some(parsed)
            });
        }
        "textPlacement" if path_ends_with(path, &["format", "textPlacement"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.text_placement = Some(parsed)
            });
        }
        "fillType" if path_ends_with(path, &["format", "fillType"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.fill_type = Some(parsed)
            });
        }
        "drawingBorder" if path_ends_with(path, &["format", "drawingBorder"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.drawing_border = Some(parsed)
            });
        }
        "bySelectedColumns" if path_ends_with(path, &["format", "bySelectedColumns"]) => {
            set_spreadsheet_format_bool(format, value, |format, parsed| {
                format.by_selected_columns = Some(parsed)
            });
        }
        "detailsUse" if path_ends_with(path, &["format", "detailsUse"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.details_use = Some(parsed)
            });
        }
        "hyperLink" if path_ends_with(path, &["format", "hyperLink"]) => {
            set_spreadsheet_format_bool(format, value, |format, parsed| {
                format.hyper_link = Some(parsed)
            });
        }
        "protection" if path_ends_with(path, &["format", "protection"]) => {
            set_spreadsheet_format_bool(format, value, |format, parsed| {
                format.protection = Some(parsed)
            });
        }
        "indent" if path_ends_with(path, &["format", "indent"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.indent = Some(parsed)
            });
        }
        "autoIndent" if path_ends_with(path, &["format", "autoIndent"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.auto_indent = Some(parsed)
            });
        }
        "mask" if path_ends_with(path, &["format", "mask"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.mask = Some(parsed)
            });
        }
        "picIndex" if path_ends_with(path, &["format", "picIndex"]) => {
            set_spreadsheet_format_usize(format, value, |format, parsed| {
                format.pic_index = Some(parsed)
            });
        }
        "pictureSizeMode" if path_ends_with(path, &["format", "pictureSizeMode"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.picture_size_mode = Some(parsed)
            });
        }
        "picHorizontalAlignment" if path_ends_with(path, &["format", "picHorizontalAlignment"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.pic_horizontal_alignment = Some(parsed)
            });
        }
        "picVerticalAlignment" if path_ends_with(path, &["format", "picVerticalAlignment"]) => {
            set_spreadsheet_format_string(format, text, |format, parsed| {
                format.pic_vertical_alignment = Some(parsed)
            });
        }
        "style" if path_ends_with(path, &["line", "style"]) => {
            if let Some(line) = line {
                line.style = text.to_string();
            }
        }
        _ => {}
    }
}

fn set_spreadsheet_print_settings_usize(
    print_settings: Option<&mut SpreadsheetDocumentXmlPrintSettings>,
    value: &str,
    setter: impl FnOnce(&mut SpreadsheetDocumentXmlPrintSettings, usize),
) {
    if let Some(print_settings) = print_settings
        && let Ok(parsed) = value.parse::<usize>()
    {
        setter(print_settings, parsed);
    }
}

fn set_spreadsheet_print_settings_bool(
    print_settings: Option<&mut SpreadsheetDocumentXmlPrintSettings>,
    value: &str,
    setter: impl FnOnce(&mut SpreadsheetDocumentXmlPrintSettings, bool),
) {
    if let Some(print_settings) = print_settings {
        setter(print_settings, value.eq_ignore_ascii_case("true"));
    }
}

fn set_spreadsheet_format_usize(
    format: Option<&mut SpreadsheetDocumentXmlFormat>,
    value: &str,
    setter: impl FnOnce(&mut SpreadsheetDocumentXmlFormat, usize),
) {
    if let Some(format) = format
        && let Ok(parsed) = value.parse::<usize>()
    {
        setter(format, parsed);
    }
}

fn set_spreadsheet_format_bool(
    format: Option<&mut SpreadsheetDocumentXmlFormat>,
    value: &str,
    setter: impl FnOnce(&mut SpreadsheetDocumentXmlFormat, bool),
) {
    if let Some(format) = format {
        setter(format, value.eq_ignore_ascii_case("true"));
    }
}

fn set_spreadsheet_format_string(
    format: Option<&mut SpreadsheetDocumentXmlFormat>,
    value: &str,
    setter: impl FnOnce(&mut SpreadsheetDocumentXmlFormat, String),
) {
    if let Some(format) = format {
        setter(format, value.to_string());
    }
}

fn spreadsheet_area_property_path(path: &[String], property: &str) -> bool {
    path_ends_with(path, &["namedItem", "area", property])
        || path_ends_with(path, &["printArea", property])
}

fn normalize_spreadsheet_cell(cell: &mut SpreadsheetDocumentXmlCell) {
    if cell.parameter.is_some() {
        cell.text = None;
        cell.empty_text = false;
    }
}

fn row_format_index_for_moxel(format_index: usize) -> usize {
    format_index.saturating_sub(1)
}

fn cell_format_index_for_moxel(format_index: usize) -> usize {
    if format_index <= 1 {
        0
    } else {
        format_index - 1
    }
}

fn format_spreadsheet_cell_for_moxel(cell: &SpreadsheetDocumentXmlCell) -> String {
    let format_index = cell_format_index_for_moxel(cell.format_index);
    let localized = if let Some(parameter) = &cell.parameter {
        format!(
            "{{1,1,{{{},{}}}}}",
            format_1c_string(""),
            format_1c_string(parameter)
        )
    } else if let Some(text) = &cell.text {
        format!(
            "{{1,1,{{{},{}}}}}",
            format_1c_string("ru"),
            format_1c_string(text)
        )
    } else if cell.empty_text {
        "{1,0}".to_string()
    } else {
        return format!("{{0,{format_index}}}");
    };
    if let Some(detail_parameter) = &cell.detail_parameter {
        return format!(
            "{{24,{format_index},{},{localized},0}}",
            format_1c_string(detail_parameter)
        );
    }
    format!("{{16,{format_index},{localized},0}}")
}

fn format_spreadsheet_column_sets_for_moxel(spreadsheet: &SpreadsheetDocumentXml) -> Vec<String> {
    let has_column_metadata = spreadsheet
        .column_sets
        .iter()
        .any(|column_set| column_set.id.is_some() || !column_set.columns.is_empty());
    let has_row_columns_id = spreadsheet.rows.iter().any(|row| row.columns_id.is_some());
    if !has_column_metadata && !has_row_columns_id {
        return Vec::new();
    }

    let column_count = spreadsheet
        .column_count
        .max(
            spreadsheet
                .column_sets
                .iter()
                .flat_map(|column_set| column_set.columns.iter().map(|column| column.index + 1))
                .max()
                .unwrap_or(1),
        )
        .max(1);
    let default_set = spreadsheet
        .column_sets
        .iter()
        .find(|column_set| column_set.id.is_none());
    let mut fields = vec![format_spreadsheet_column_set_for_moxel(
        default_set,
        column_count,
        None,
    )];

    let additional_sets = spreadsheet
        .column_sets
        .iter()
        .filter(|column_set| column_set.id.is_some())
        .collect::<Vec<_>>();
    let height = spreadsheet
        .rows
        .iter()
        .map(|row| *row.expanded_indexes().end())
        .max()
        .unwrap_or(0)
        + 1;
    fields.push(height.to_string());
    fields.push(additional_sets.len().to_string());
    for column_set in &additional_sets {
        fields.push(format_spreadsheet_column_set_for_moxel(
            Some(column_set),
            column_count,
            column_set.id.as_deref(),
        ));
    }

    let mut row_pairs = Vec::<(usize, usize)>::new();
    for row in &spreadsheet.rows {
        let Some(columns_id) = row.columns_id.as_deref() else {
            continue;
        };
        let Some(set_index) = additional_sets
            .iter()
            .position(|column_set| column_set.id.as_deref() == Some(columns_id))
        else {
            continue;
        };
        for row_index in row.expanded_indexes() {
            row_pairs.push((row_index, set_index));
        }
    }
    fields.push(row_pairs.len().to_string());
    for (row_index, set_index) in row_pairs {
        fields.push(row_index.to_string());
        fields.push(set_index.to_string());
    }

    fields
}

fn format_spreadsheet_column_set_for_moxel(
    column_set: Option<&SpreadsheetDocumentXmlColumnSet>,
    fallback_size: usize,
    id: Option<&str>,
) -> String {
    let declared_size = column_set
        .map(|column_set| column_set.size)
        .filter(|size| *size > 0)
        .unwrap_or(fallback_size);
    let size = declared_size
        .max(
            column_set
                .into_iter()
                .flat_map(|column_set| column_set.columns.iter().map(|column| column.index + 1))
                .max()
                .unwrap_or(1),
        )
        .max(1);
    let synthesized;
    let columns = if let Some(column_set) = column_set {
        if column_set.columns.is_empty() {
            synthesized = synthesize_spreadsheet_columns(size);
            &synthesized
        } else {
            &column_set.columns
        }
    } else {
        synthesized = synthesize_spreadsheet_columns(size);
        &synthesized
    };
    let uuid = id.unwrap_or("00000000-0000-0000-0000-000000000000");
    let mut fields = Vec::with_capacity(columns.len() * 2 + 4);
    fields.push(size.to_string());
    fields.push("0".to_string());
    fields.push(uuid.to_string());
    fields.push(columns.len().to_string());
    for column in columns {
        fields.push(column.index.to_string());
        fields.push(column.format_index.max(1).to_string());
    }
    format!("{{{}}}", fields.join(","))
}

fn synthesize_spreadsheet_columns(size: usize) -> Vec<SpreadsheetDocumentXmlColumn> {
    (0..size)
        .map(|index| SpreadsheetDocumentXmlColumn {
            index,
            format_index: index + 1,
        })
        .collect()
}

fn format_spreadsheet_merges_for_moxel(merges: &[SpreadsheetDocumentXmlMerge]) -> String {
    let mut fields = Vec::with_capacity(merges.len() + 1);
    fields.push(merges.len().to_string());
    for merge in merges {
        let begin_column = merge.column.max(0);
        let begin_row = merge.row.max(0);
        let end_column = begin_column + merge.width.max(0);
        let end_row = begin_row + merge.height.max(0);
        fields.push(format!(
            "{{{begin_column},{begin_row},{end_column},{end_row}}}"
        ));
    }
    format!("{{{}}}", fields.join(","))
}

fn format_spreadsheet_named_areas_for_moxel(areas: &[SpreadsheetDocumentXmlArea]) -> String {
    let mut fields = Vec::with_capacity(areas.len() * 2 + 1);
    fields.push(areas.len().to_string());
    for area in areas {
        fields.push(format_1c_string(&area.name));
        fields.push(format!(
            "{{1,{},0}}",
            format_spreadsheet_area_bounds_for_moxel(area)
        ));
    }
    format!("{{{}}}", fields.join(","))
}

fn format_spreadsheet_area_bounds_for_moxel(area: &SpreadsheetDocumentXmlArea) -> String {
    let area_type = spreadsheet_area_type_code(&area.area_type);
    let columns_id = area
        .columns_id
        .as_deref()
        .filter(|value| !value.is_empty())
        .unwrap_or("00000000-0000-0000-0000-000000000000");
    format!(
        "{{{area_type},{},{},{},{},{columns_id}}}",
        area.begin_column.max(0),
        area.begin_row.max(0),
        area.end_column.max(area.begin_column).max(0),
        area.end_row.max(area.begin_row).max(0)
    )
}

fn spreadsheet_area_type_code(area_type: &str) -> &'static str {
    match area_type {
        "Rows" => "1",
        "Columns" => "2",
        _ => "3",
    }
}

fn format_spreadsheet_print_settings_for_moxel(
    settings: &SpreadsheetDocumentXmlPrintSettings,
) -> String {
    let pairs = [
        (
            0,
            format_spreadsheet_print_number(settings.paper.unwrap_or(0)),
        ),
        (
            1,
            format_spreadsheet_print_number(
                settings
                    .page_orientation
                    .as_deref()
                    .map(spreadsheet_page_orientation_code)
                    .unwrap_or(1),
            ),
        ),
        (
            2,
            format_spreadsheet_print_number(settings.scale.unwrap_or(100)),
        ),
        (
            3,
            format_spreadsheet_print_number(bool_to_usize(settings.collate.unwrap_or(true))),
        ),
        (
            4,
            format_spreadsheet_print_number(settings.copies.unwrap_or(1)),
        ),
        (
            5,
            format_spreadsheet_print_number(settings.per_page.unwrap_or(1)),
        ),
        (
            6,
            format_spreadsheet_print_number(settings.top_margin.unwrap_or(0)),
        ),
        (
            7,
            format_spreadsheet_print_number(settings.left_margin.unwrap_or(0)),
        ),
        (
            8,
            format_spreadsheet_print_number(settings.bottom_margin.unwrap_or(0)),
        ),
        (
            9,
            format_spreadsheet_print_number(settings.right_margin.unwrap_or(0)),
        ),
        (
            10,
            format_spreadsheet_print_number(settings.header_size.unwrap_or(0)),
        ),
        (
            11,
            format_spreadsheet_print_number(settings.footer_size.unwrap_or(0)),
        ),
        (
            12,
            format_spreadsheet_print_number(bool_to_usize(settings.fit_to_page.unwrap_or(false))),
        ),
        (
            13,
            format_spreadsheet_print_number(bool_to_usize(
                settings.black_and_white.unwrap_or(false),
            )),
        ),
        (
            14,
            format_spreadsheet_print_string(settings.printer_name.as_deref().unwrap_or("")),
        ),
        (
            15,
            format_spreadsheet_print_number(settings.paper_source.unwrap_or(0)),
        ),
        (
            16,
            format_spreadsheet_print_number(settings.page_width.unwrap_or(0)),
        ),
        (
            17,
            format_spreadsheet_print_number(settings.page_height.unwrap_or(0)),
        ),
    ];
    let mut fields = Vec::with_capacity(pairs.len() * 2 + 2);
    fields.push("0".to_string());
    fields.push(pairs.len().to_string());
    for (key, value) in pairs {
        fields.push(key.to_string());
        fields.push(value);
    }
    format!("{{{{{}}}}}", fields.join(","))
}

fn format_spreadsheet_print_number(value: usize) -> String {
    format!(r#"{{"N",{value}}}"#)
}

fn format_spreadsheet_print_string(value: &str) -> String {
    format!(r#"{{"S",{}}}"#, format_1c_string(value))
}

fn spreadsheet_page_orientation_code(value: &str) -> usize {
    match value {
        "Landscape" => 2,
        _ => 1,
    }
}

fn bool_to_usize(value: bool) -> usize {
    if value { 1 } else { 0 }
}

fn format_spreadsheet_formats_for_moxel(
    spreadsheet: &SpreadsheetDocumentXml,
    column_count: usize,
) -> Vec<String> {
    if spreadsheet.formats.is_empty() && spreadsheet.default_format_index.is_none() {
        return Vec::new();
    }
    let body_format_count = spreadsheet
        .formats
        .len()
        .max(spreadsheet.default_format_index.unwrap_or(0));
    let column_placeholder_count = column_count.max(1);
    let count = body_format_count + column_placeholder_count;
    let mut style_refs = Vec::<SpreadsheetStyleRefSlot>::new();
    let mut format_fields = Vec::with_capacity(count + 1);
    format_fields.push(count.to_string());
    for index in 0..body_format_count {
        let field = spreadsheet
            .formats
            .get(index)
            .and_then(|format| format_spreadsheet_format_for_moxel(format, &mut style_refs))
            .unwrap_or_else(spreadsheet_empty_format_for_moxel);
        format_fields.push(field);
    }
    for _ in 0..column_placeholder_count {
        format_fields.push(spreadsheet_empty_format_for_moxel());
    }
    let mut fields = style_refs
        .iter()
        .map(format_spreadsheet_style_ref_slot_for_moxel)
        .collect::<Vec<_>>();
    fields.push(format!("{{{}}}", format_fields.join(",")));
    fields
}

fn spreadsheet_empty_format_for_moxel() -> String {
    "{1,0}".to_string()
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SpreadsheetStyleRefSlot {
    DirectColor(u32),
    SystemStyle(i32),
    WebColor(u32),
}

fn format_spreadsheet_format_for_moxel(
    format: &SpreadsheetDocumentXmlFormat,
    style_refs: &mut Vec<SpreadsheetStyleRefSlot>,
) -> Option<String> {
    let mut values = Vec::<(u8, usize)>::new();
    push_spreadsheet_format_value(&mut values, 0, format.font);
    if let Some(border) = format.border {
        for bit in [1, 2, 3, 4] {
            push_spreadsheet_format_value(&mut values, bit, Some(border));
        }
    } else {
        push_spreadsheet_format_value(&mut values, 1, format.left_border);
        push_spreadsheet_format_value(&mut values, 2, format.top_border);
        push_spreadsheet_format_value(&mut values, 3, format.right_border);
        push_spreadsheet_format_value(&mut values, 4, format.bottom_border);
    }
    push_spreadsheet_format_value(&mut values, 6, format.height);
    push_spreadsheet_format_value(
        &mut values,
        5,
        format
            .border_color
            .as_deref()
            .and_then(|value| spreadsheet_style_ref_index(value, style_refs)),
    );
    push_spreadsheet_format_value(&mut values, 7, format.width);
    push_spreadsheet_format_value(
        &mut values,
        8,
        format
            .horizontal_alignment
            .as_deref()
            .and_then(spreadsheet_horizontal_alignment_code),
    );
    push_spreadsheet_format_value(
        &mut values,
        9,
        format
            .vertical_alignment
            .as_deref()
            .and_then(spreadsheet_vertical_alignment_code),
    );
    push_spreadsheet_format_value(
        &mut values,
        10,
        format
            .text_color
            .as_deref()
            .and_then(|value| spreadsheet_style_ref_index(value, style_refs)),
    );
    push_spreadsheet_format_value(
        &mut values,
        11,
        format
            .back_color
            .as_deref()
            .and_then(|value| spreadsheet_style_ref_index(value, style_refs)),
    );
    push_spreadsheet_format_value(
        &mut values,
        14,
        format
            .text_placement
            .as_deref()
            .and_then(spreadsheet_text_placement_code),
    );
    push_spreadsheet_format_value(
        &mut values,
        15,
        format
            .fill_type
            .as_deref()
            .and_then(spreadsheet_fill_type_code),
    );
    push_spreadsheet_format_value(
        &mut values,
        16,
        format.protection.map(spreadsheet_protection_code),
    );
    push_spreadsheet_format_value(
        &mut values,
        19,
        format
            .details_use
            .as_deref()
            .and_then(spreadsheet_details_use_code),
    );
    push_spreadsheet_format_value(
        &mut values,
        20,
        format.by_selected_columns.map(bool_to_usize),
    );
    push_spreadsheet_format_value(&mut values, 26, format.hyper_link.map(bool_to_usize));
    push_spreadsheet_format_value(&mut values, 30, format.indent);
    push_spreadsheet_format_value(&mut values, 31, format.auto_indent);
    if format.mask.as_deref() == Some("") {
        push_spreadsheet_format_value(&mut values, 34, Some(0));
    }
    push_spreadsheet_format_value(&mut values, 35, format.pic_index);
    push_spreadsheet_format_value(
        &mut values,
        36,
        format
            .picture_size_mode
            .as_deref()
            .and_then(spreadsheet_picture_size_mode_code),
    );
    push_spreadsheet_format_value(
        &mut values,
        37,
        format
            .pic_horizontal_alignment
            .as_deref()
            .and_then(spreadsheet_picture_alignment_code),
    );
    push_spreadsheet_format_value(
        &mut values,
        38,
        format
            .pic_vertical_alignment
            .as_deref()
            .and_then(spreadsheet_picture_alignment_code),
    );
    if values.is_empty() {
        return None;
    }
    values.sort_by_key(|(bit, _)| *bit);
    let flags = values
        .iter()
        .fold(0u64, |acc, (bit, _)| acc | (1u64 << bit));
    let mut fields = Vec::with_capacity(values.len() + 1);
    fields.push(flags.to_string());
    fields.extend(values.into_iter().map(|(_, value)| value.to_string()));
    Some(format!("{{{}}}", fields.join(",")))
}

fn push_spreadsheet_format_value(values: &mut Vec<(u8, usize)>, bit: u8, value: Option<usize>) {
    if let Some(value) = value {
        values.push((bit, value));
    }
}

fn spreadsheet_style_ref_index(
    value: &str,
    style_refs: &mut Vec<SpreadsheetStyleRefSlot>,
) -> Option<usize> {
    let slot = spreadsheet_style_ref_slot(value)?;
    if let Some(index) = style_refs.iter().position(|existing| existing == &slot) {
        return Some(index);
    }
    let index = style_refs.len();
    style_refs.push(slot);
    Some(index)
}

fn spreadsheet_style_ref_slot(value: &str) -> Option<SpreadsheetStyleRefSlot> {
    if let Some(color) = spreadsheet_direct_color_code(value) {
        return Some(SpreadsheetStyleRefSlot::DirectColor(color));
    }
    if let Some(code) = spreadsheet_system_style_code(value) {
        return Some(SpreadsheetStyleRefSlot::SystemStyle(code));
    }
    spreadsheet_web_color_code(value).map(SpreadsheetStyleRefSlot::WebColor)
}

fn format_spreadsheet_style_ref_slot_for_moxel(slot: &SpreadsheetStyleRefSlot) -> String {
    match slot {
        SpreadsheetStyleRefSlot::DirectColor(value) => format!("{{3,0,{{{value}}}}}"),
        SpreadsheetStyleRefSlot::SystemStyle(value) => format!("{{3,3,{{{value}}}}}"),
        SpreadsheetStyleRefSlot::WebColor(value) => format!("{{3,2,{{{value}}}}}"),
    }
}

fn spreadsheet_direct_color_code(value: &str) -> Option<u32> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let red = u32::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u32::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u32::from_str_radix(&hex[4..6], 16).ok()?;
    Some(red | (green << 8) | (blue << 16))
}

fn spreadsheet_system_style_code(value: &str) -> Option<i32> {
    match value {
        "style:FieldBackColor" => Some(-10),
        "style:ButtonBackColor" => Some(-7),
        "style:ReportLineColor" => Some(-28),
        _ => None,
    }
}

fn spreadsheet_web_color_code(value: &str) -> Option<u32> {
    match value {
        "d3p1:Crimson" => Some(21),
        "d3p1:Gainsboro" => Some(48),
        "d3p1:LemonChiffon" => Some(64),
        "d3p1:LightYellow" => Some(79),
        "d3p1:PaleGoldenrod" => Some(108),
        "d3p1:RoyalBlue" => Some(121),
        _ => None,
    }
}

fn spreadsheet_horizontal_alignment_code(value: &str) -> Option<usize> {
    match value {
        "Left" => Some(0),
        "Right" => Some(2),
        "Center" => Some(6),
        _ => None,
    }
}

fn spreadsheet_vertical_alignment_code(value: &str) -> Option<usize> {
    match value {
        "Top" => Some(0),
        "Center" => Some(24),
        _ => None,
    }
}

fn spreadsheet_text_placement_code(value: &str) -> Option<usize> {
    match value {
        "Auto" => Some(0),
        "Block" => Some(2),
        "Wrap" => Some(3),
        _ => None,
    }
}

fn spreadsheet_fill_type_code(value: &str) -> Option<usize> {
    match value {
        "Text" => Some(0),
        "Parameter" => Some(1),
        "Template" => Some(2),
        _ => None,
    }
}

fn spreadsheet_details_use_code(value: &str) -> Option<usize> {
    match value {
        "Cell" => Some(0),
        "Row" => Some(1),
        _ => None,
    }
}

fn spreadsheet_protection_code(value: bool) -> usize {
    if value { 0 } else { 1 }
}

fn spreadsheet_picture_size_mode_code(value: &str) -> Option<usize> {
    match value {
        "Proportionally" => Some(6),
        _ => None,
    }
}

fn spreadsheet_picture_alignment_code(value: &str) -> Option<usize> {
    match value {
        "Center" => Some(2),
        _ => None,
    }
}

fn format_spreadsheet_lines_for_moxel(lines: &[SpreadsheetDocumentXmlLine]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line.line_type.ends_with("SpreadsheetDocumentCellLineType"))
        .filter_map(|line| spreadsheet_line_style_code(&line.style))
        .map(|code| format!("{{3,3,{{{code}}}}}"))
        .collect()
}

fn spreadsheet_line_style_code(value: &str) -> Option<i32> {
    match value {
        "None" => Some(-1),
        "Solid" => Some(-3),
        "Dotted" => Some(-10),
        _ => None,
    }
}

fn format_spreadsheet_fonts_for_moxel(fonts: &[SpreadsheetDocumentXmlFont]) -> Vec<String> {
    fonts
        .iter()
        .filter_map(format_spreadsheet_font_for_moxel)
        .collect()
}

fn format_spreadsheet_font_for_moxel(font: &SpreadsheetDocumentXmlFont) -> Option<String> {
    match font.kind.as_str() {
        "Absolute" => {
            let face_name = font.face_name.as_deref().unwrap_or("Arial");
            let height = font.height.unwrap_or(8) * 10;
            let weight = spreadsheet_font_weight(font.bold);
            let scale = font.scale.unwrap_or(100);
            Some(format!(
                "{{7,0,575,{height},{},{},{},{weight},0,0,0,0,0,0,0,0,{},1,{scale}}}",
                bool_to_usize(font.italic),
                bool_to_usize(font.underline),
                bool_to_usize(font.strikeout),
                format_1c_string(face_name)
            ))
        }
        "StyleItem" => {
            let ref_code = spreadsheet_font_ref_code(font.ref_name.as_deref()?)?;
            let weight = spreadsheet_font_weight(font.bold);
            Some(format!(
                "{{7,2,60,{{{ref_code}}},{weight},{},{},{},1,100}}",
                bool_to_usize(font.italic),
                bool_to_usize(font.underline),
                bool_to_usize(font.strikeout)
            ))
        }
        _ => None,
    }
}

fn spreadsheet_font_weight(bold: bool) -> usize {
    if bold { 700 } else { 400 }
}

fn spreadsheet_font_ref_code(ref_name: &str) -> Option<i32> {
    match ref_name {
        "style:TextFont" => Some(-20),
        "style:NormalTextFont" => Some(-31),
        "style:LargeTextFont" => Some(-32),
        _ => None,
    }
}

pub fn pack_form_body_blob_from_module_text(
    base_blob: &[u8],
    module_text: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    let inflated = inflate_raw(base_blob).context("failed to inflate base Form body blob")?;
    let mut plain =
        String::from_utf8(inflated).context("base Form body blob is not valid UTF-8")?;
    let body_start = plain
        .find('{')
        .ok_or_else(|| anyhow!("base Form body has no braced payload"))?;
    let fields = scan_braced_fields(&plain, body_start)?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some("4") {
        return Err(anyhow!("base Form body does not start with type marker 4"));
    }
    let module_range = fields
        .get(2)
        .ok_or_else(|| anyhow!("base Form body has no module text field"))?
        .clone();
    let module_text = std::str::from_utf8(module_text)
        .context("Form module text is not valid UTF-8")?
        .trim_start_matches('\u{feff}');
    plain.replace_range(module_range, &format_1c_string(module_text));
    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_role_rights_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    let rights = parse_role_rights_xml(xml)?;
    let inflated = inflate_raw(base_blob).context("failed to inflate base Role rights blob")?;
    let mut plain =
        String::from_utf8(inflated).context("base Role rights blob is not valid UTF-8")?;
    let body_start = plain
        .find('{')
        .ok_or_else(|| anyhow!("base Role rights body has no braced payload"))?;
    let fields = scan_braced_fields(&plain, body_start)?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some("10") {
        return Err(anyhow!(
            "base Role rights body does not start with type marker 10"
        ));
    }
    let objects_range = fields
        .get(1)
        .ok_or_else(|| anyhow!("base Role rights body has no object rights field"))?
        .clone();
    let set_for_new_objects_range = fields
        .get(4)
        .ok_or_else(|| anyhow!("base Role rights body has no setForNewObjects field"))?
        .clone();
    let mut replacements = role_right_value_replacements(&plain, objects_range, &rights.objects)?;
    replacements.push((
        set_for_new_objects_range,
        if rights.set_for_new_objects { "1" } else { "0" }.to_string(),
    ));
    replacements.sort_by(|left, right| right.0.start.cmp(&left.0.start));
    for (range, replacement) in replacements {
        plain.replace_range(range, &replacement);
    }
    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_command_interface_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    let entries = parse_command_interface_xml(xml)?;
    let inflated =
        inflate_raw(base_blob).context("failed to inflate base CommandInterface blob")?;
    let mut plain =
        String::from_utf8(inflated).context("base CommandInterface blob is not valid UTF-8")?;
    let body_start = plain
        .find('{')
        .ok_or_else(|| anyhow!("base CommandInterface body has no braced payload"))?;
    let fields = scan_braced_fields(&plain, body_start)?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some("7") {
        return Err(anyhow!(
            "base CommandInterface body does not start with type marker 7"
        ));
    }
    let count_range = fields
        .get(2)
        .ok_or_else(|| anyhow!("base CommandInterface body has no command count"))?;
    let count = plain[count_range.clone()]
        .trim()
        .parse::<usize>()
        .context("invalid base CommandInterface command count")?;
    if count != entries.len() {
        return Err(anyhow!(
            "CommandInterface.xml command count {} does not match base blob command count {}",
            entries.len(),
            count
        ));
    }
    let required_fields = 3 + count * 2;
    if fields.len() < required_fields {
        return Err(anyhow!(
            "base CommandInterface body has {} fields, expected at least {}",
            fields.len(),
            required_fields
        ));
    }

    let mut replacements = Vec::with_capacity(count);
    for (index, entry) in entries.iter().enumerate() {
        let common_range = fields[3 + index * 2 + 1].clone();
        let common = if entry.common { "1" } else { "0" };
        replacements.push((
            common_range,
            format!("{{{{0,{{{{0,{{{{\"B\",{common}}}}},0}}}}}}}}"),
        ));
    }
    replacements.sort_by(|left, right| right.0.start.cmp(&left.0.start));
    for (range, replacement) in replacements {
        plain.replace_range(range, &replacement);
    }
    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_exchange_plan_content_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
    source: &MetadataSourceContext,
) -> Result<PackedRawDeflatedBlob> {
    let items = parse_exchange_plan_content_xml(xml)?;
    if !base_blob.is_empty() {
        let inflated =
            inflate_raw(base_blob).context("failed to inflate base ExchangePlanContent blob")?;
        let plain = String::from_utf8(inflated)
            .context("base ExchangePlanContent blob is not valid UTF-8")?;
        let body_start = plain
            .find('{')
            .ok_or_else(|| anyhow!("base ExchangePlanContent body has no braced payload"))?;
        let fields = scan_braced_fields(&plain, body_start)?;
        if fields.first().map(|range| plain[range.clone()].trim()) != Some("2") {
            return Err(anyhow!(
                "base ExchangePlanContent body does not start with type marker 2"
            ));
        }
    }

    let mut plain = format!("{{2,{}", items.len());
    for item in items {
        let uuid = source
            .resolve_metadata_reference_uuid(&item.metadata)
            .with_context(|| format!("failed to resolve ExchangePlanContent {}", item.metadata))?;
        let auto_record = if item.auto_record { "1" } else { "0" };
        plain.push(',');
        plain.push_str(&uuid);
        plain.push(',');
        plain.push_str(auto_record);
    }
    plain.push('}');

    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_predefined_data_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    let items = parse_predefined_data_xml(xml)?;
    let by_id = flatten_predefined_xml_items(&items)?;
    let inflated = inflate_raw(base_blob).context("failed to inflate base PredefinedData blob")?;
    let mut plain =
        String::from_utf8(inflated).context("base PredefinedData blob is not valid UTF-8")?;
    let body_start = plain
        .find('{')
        .ok_or_else(|| anyhow!("base PredefinedData body has no braced payload"))?;
    let fields = scan_braced_fields(&plain, body_start)?;
    if !matches!(
        fields.first().map(|range| plain[range.clone()].trim()),
        Some("0" | "1")
    ) {
        return Err(anyhow!(
            "base PredefinedData body does not start with type marker 0 or 1"
        ));
    }
    let table_range = fields
        .get(1)
        .ok_or_else(|| anyhow!("base PredefinedData body has no table field"))?
        .clone();
    let mut replacements = Vec::<(Range<usize>, String)>::new();
    let mut seen = BTreeSet::<String>::new();
    collect_predefined_replacements(&plain, table_range, &by_id, &mut seen, &mut replacements)?;
    let missing = by_id
        .keys()
        .filter(|id| !seen.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(anyhow!(
            "PredefinedData XML contains items missing in base blob: {}",
            missing.join(", ")
        ));
    }

    replacements.sort_by(|left, right| right.0.start.cmp(&left.0.start));
    for (range, replacement) in replacements {
        plain.replace_range(range, &replacement);
    }
    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_business_process_flowchart_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    let items = parse_flowchart_xml(xml)?;
    let by_id = items
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let inflated =
        inflate_raw(base_blob).context("failed to inflate base BusinessProcess Flowchart blob")?;
    let mut plain = String::from_utf8(inflated)
        .context("base BusinessProcess Flowchart blob is not valid UTF-8")?;
    let body_start = plain
        .find('{')
        .ok_or_else(|| anyhow!("base BusinessProcess Flowchart body has no braced payload"))?;
    let fields = scan_braced_fields(&plain, body_start)?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some("5") {
        return Err(anyhow!(
            "base BusinessProcess Flowchart body does not start with type marker 5"
        ));
    }
    let item_count = plain[fields
        .get(2)
        .ok_or_else(|| anyhow!("base BusinessProcess Flowchart has no item count"))?
        .clone()]
    .trim()
    .parse::<usize>()
    .context("invalid BusinessProcess Flowchart item count")?;
    let mut replacements = Vec::<(Range<usize>, String)>::new();
    let mut seen = BTreeSet::<String>::new();
    let mut index = 3usize;
    for _ in 0..item_count {
        let code_range = fields
            .get(index)
            .ok_or_else(|| anyhow!("base BusinessProcess Flowchart item has no code"))?
            .clone();
        let code = plain[code_range].trim().to_string();
        let body_range = fields
            .get(index + 1)
            .ok_or_else(|| anyhow!("base BusinessProcess Flowchart item has no body"))?
            .clone();
        collect_flowchart_item_replacements(
            &plain,
            &code,
            body_range,
            &by_id,
            &mut seen,
            &mut replacements,
        )?;
        index += 2;
    }
    let missing = by_id
        .keys()
        .filter(|id| !seen.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(anyhow!(
            "Flowchart.xml contains items missing in base blob: {}",
            missing.join(", ")
        ));
    }

    replacements.sort_by(|left, right| right.0.start.cmp(&left.0.start));
    for (range, replacement) in replacements {
        plain.replace_range(range, &replacement);
    }
    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

fn role_right_value_replacements(
    plain: &str,
    objects_range: Range<usize>,
    objects: &[RoleObjectRightsXml],
) -> Result<Vec<(Range<usize>, String)>> {
    let object_fields = scan_braced_fields(plain, objects_range.start)?;
    let base_count_range = object_fields
        .first()
        .ok_or_else(|| anyhow!("base Role object rights field is empty"))?
        .clone();
    let base_count = plain[base_count_range]
        .trim()
        .parse::<usize>()
        .context("invalid base Role object rights count")?;
    if base_count != objects.len() {
        return Err(anyhow!(
            "Role Rights.xml object count {} does not match base blob object count {}",
            objects.len(),
            base_count
        ));
    }
    if object_fields.len() != base_count + 1 {
        return Err(anyhow!(
            "base Role object rights field count {} does not match declared count {}",
            object_fields.len().saturating_sub(1),
            base_count
        ));
    }

    let mut replacements = Vec::new();
    for (object_index, object) in objects.iter().enumerate() {
        let entry_range = object_fields[object_index + 1].clone();
        let entry_fields = scan_braced_fields(plain, entry_range.start)?;
        let rights_range = entry_fields.get(1).ok_or_else(|| {
            anyhow!("base Role object rights entry {object_index} has no rights payload")
        })?;
        replacements.extend(role_object_right_value_replacements(
            plain,
            rights_range.clone(),
            object,
        )?);
    }
    Ok(replacements)
}

fn role_object_right_value_replacements(
    plain: &str,
    rights_range: Range<usize>,
    object: &RoleObjectRightsXml,
) -> Result<Vec<(Range<usize>, String)>> {
    let fields = scan_braced_fields(plain, rights_range.start)?;
    let marker = fields
        .first()
        .map(|range| plain[range.clone()].trim())
        .ok_or_else(|| anyhow!("base Role rights payload is empty"))?;
    let (start, count) = match marker {
        "0" => (1usize, (fields.len().saturating_sub(1)) / 2),
        "1" => {
            let count_range = fields
                .get(1)
                .ok_or_else(|| anyhow!("base Role restricted rights payload has no count"))?;
            let count = plain[count_range.clone()]
                .trim()
                .parse::<usize>()
                .context("invalid base Role restricted rights count")?;
            (2usize, count)
        }
        _ => return Err(anyhow!("unsupported base Role rights marker {marker}")),
    };
    if object.rights.len() != count {
        return Err(anyhow!(
            "Role Rights.xml object {} right count {} does not match base blob right count {}",
            object.name,
            object.rights.len(),
            count
        ));
    }
    let required_fields = start + count * 2;
    if fields.len() < required_fields {
        return Err(anyhow!(
            "base Role rights payload has {} fields, expected at least {}",
            fields.len(),
            required_fields
        ));
    }
    let mut replacements = Vec::with_capacity(count);
    for (right_index, right) in object.rights.iter().enumerate() {
        let value_range = fields[start + right_index * 2 + 1].clone();
        replacements.push((
            value_range,
            if right.value { "1" } else { "-1" }.to_string(),
        ));
    }
    Ok(replacements)
}

fn parse_role_rights_xml(xml: &[u8]) -> Result<RoleRightsXml> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut set_for_new_objects = None::<bool>;
    let mut objects = Vec::<RoleObjectRightsXml>::new();
    let mut current_object = None::<RoleObjectRightsXml>;
    let mut current_right = None::<RoleRightXml>;
    let mut text_value = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "object" && path_ends_with(&path, &["Rights"]) {
                    current_object = Some(RoleObjectRightsXml {
                        name: String::new(),
                        rights: Vec::new(),
                    });
                } else if local == "right" && path_ends_with(&path, &["Rights", "object"]) {
                    current_right = Some(RoleRightXml {
                        name: String::new(),
                        value: false,
                    });
                }
                if matches!(local.as_str(), "setForNewObjects" | "name" | "value") {
                    text_value.clear();
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "setForNewObjects" {
                    set_for_new_objects = Some(false);
                }
            }
            Ok(Event::Text(text)) => {
                if path_ends_with(&path, &["Rights", "setForNewObjects"])
                    || path_ends_with(&path, &["Rights", "object", "name"])
                    || path_ends_with(&path, &["Rights", "object", "right", "name"])
                    || path_ends_with(&path, &["Rights", "object", "right", "value"])
                {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if path_ends_with(&path, &["Rights", "setForNewObjects"])
                    || path_ends_with(&path, &["Rights", "object", "name"])
                    || path_ends_with(&path, &["Rights", "object", "right", "name"])
                    || path_ends_with(&path, &["Rights", "object", "right", "value"])
                {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                match local.as_str() {
                    "setForNewObjects"
                        if path_ends_with(&path, &["Rights", "setForNewObjects"]) =>
                    {
                        set_for_new_objects = Some(parse_xml_bool_text(
                            "Role/setForNewObjects",
                            text_value.trim(),
                        )?);
                    }
                    "name" if path_ends_with(&path, &["Rights", "object", "name"]) => {
                        if let Some(object) = current_object.as_mut() {
                            object.name = text_value.clone();
                        }
                    }
                    "name" if path_ends_with(&path, &["Rights", "object", "right", "name"]) => {
                        if let Some(right) = current_right.as_mut() {
                            right.name = text_value.clone();
                        }
                    }
                    "value" if path_ends_with(&path, &["Rights", "object", "right", "value"]) => {
                        if let Some(right) = current_right.as_mut() {
                            right.value =
                                parse_xml_bool_text("Role/right/value", text_value.trim())?;
                        }
                    }
                    "right" if path_ends_with(&path, &["Rights", "object", "right"]) => {
                        let right = current_right.take().ok_or_else(|| {
                            anyhow!("Rights.xml ended right without active right")
                        })?;
                        if right.name.is_empty() {
                            return Err(anyhow!("Rights.xml contains right without name"));
                        }
                        if let Some(object) = current_object.as_mut() {
                            object.rights.push(right);
                        }
                    }
                    "object" if path_ends_with(&path, &["Rights", "object"]) => {
                        let object = current_object.take().ok_or_else(|| {
                            anyhow!("Rights.xml ended object without active object")
                        })?;
                        if object.name.is_empty() {
                            return Err(anyhow!("Rights.xml contains object without name"));
                        }
                        objects.push(object);
                    }
                    _ => {}
                }
                let _ = path.pop();
                if matches!(local.as_str(), "setForNewObjects" | "name" | "value") {
                    text_value.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(RoleRightsXml {
        set_for_new_objects: set_for_new_objects
            .ok_or_else(|| anyhow!("Rights.xml has no setForNewObjects"))?,
        objects,
    })
}

fn parse_command_interface_xml(xml: &[u8]) -> Result<Vec<CommandInterfaceXmlEntry>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut entries = Vec::<CommandInterfaceXmlEntry>::new();
    let mut current = None::<CommandInterfaceXmlEntry>;
    let mut text_value = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "Command"
                    && path_ends_with(&path, &["CommandInterface", "CommandsVisibility"])
                {
                    let name = xml_attr_value(&event, "name").ok_or_else(|| {
                        anyhow!("CommandInterface Command element has no name attribute")
                    })?;
                    current = Some(CommandInterfaceXmlEntry {
                        name,
                        common: false,
                    });
                }
                if local == "Common" {
                    text_value.clear();
                }
                path.push(local);
            }
            Ok(Event::Text(text)) => {
                if path_ends_with(
                    &path,
                    &[
                        "CommandInterface",
                        "CommandsVisibility",
                        "Command",
                        "Visibility",
                        "Common",
                    ],
                ) {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if path_ends_with(
                    &path,
                    &[
                        "CommandInterface",
                        "CommandsVisibility",
                        "Command",
                        "Visibility",
                        "Common",
                    ],
                ) {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                match local.as_str() {
                    "Common"
                        if path_ends_with(
                            &path,
                            &[
                                "CommandInterface",
                                "CommandsVisibility",
                                "Command",
                                "Visibility",
                                "Common",
                            ],
                        ) =>
                    {
                        if let Some(entry) = current.as_mut() {
                            entry.common = parse_xml_bool_text(
                                "CommandInterface/Command/Visibility/Common",
                                text_value.trim(),
                            )?;
                        }
                    }
                    "Command"
                        if path_ends_with(
                            &path,
                            &["CommandInterface", "CommandsVisibility", "Command"],
                        ) =>
                    {
                        let entry = current.take().ok_or_else(|| {
                            anyhow!("CommandInterface ended Command without active command")
                        })?;
                        entries.push(entry);
                    }
                    _ => {}
                }
                let _ = path.pop();
                if local == "Common" {
                    text_value.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(entries)
}

fn parse_exchange_plan_content_xml(xml: &[u8]) -> Result<Vec<ExchangePlanContentXmlItem>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut items = Vec::<ExchangePlanContentXmlItem>::new();
    let mut metadata = None::<String>;
    let mut auto_record = None::<bool>;
    let mut text_value = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["ExchangePlanContent"]) && local == "Item" {
                    metadata = None;
                    auto_record = None;
                }
                if matches!(local.as_str(), "Metadata" | "AutoRecord") {
                    text_value.clear();
                }
                path.push(local);
            }
            Ok(Event::Text(text)) => {
                if path_ends_with(&path, &["ExchangePlanContent", "Item", "Metadata"])
                    || path_ends_with(&path, &["ExchangePlanContent", "Item", "AutoRecord"])
                {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if path_ends_with(&path, &["ExchangePlanContent", "Item", "Metadata"])
                    || path_ends_with(&path, &["ExchangePlanContent", "Item", "AutoRecord"])
                {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                match local.as_str() {
                    "Metadata"
                        if path_ends_with(&path, &["ExchangePlanContent", "Item", "Metadata"]) =>
                    {
                        metadata = Some(text_value.trim().to_string());
                    }
                    "AutoRecord"
                        if path_ends_with(
                            &path,
                            &["ExchangePlanContent", "Item", "AutoRecord"],
                        ) =>
                    {
                        auto_record =
                            Some(parse_exchange_plan_auto_record_text(text_value.trim())?);
                    }
                    "Item" if path_ends_with(&path, &["ExchangePlanContent", "Item"]) => {
                        items.push(ExchangePlanContentXmlItem {
                            metadata: metadata.take().ok_or_else(|| {
                                anyhow!("ExchangePlanContent Item has no Metadata")
                            })?,
                            auto_record: auto_record.take().ok_or_else(|| {
                                anyhow!("ExchangePlanContent Item has no AutoRecord")
                            })?,
                        });
                    }
                    _ => {}
                }
                let _ = path.pop();
                if matches!(local.as_str(), "Metadata" | "AutoRecord") {
                    text_value.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(items)
}

fn parse_exchange_plan_auto_record_text(value: &str) -> Result<bool> {
    match value {
        "Deny" => Ok(false),
        "Auto" => Ok(true),
        _ => Err(anyhow!("invalid ExchangePlanContent AutoRecord: {value}")),
    }
}

fn flatten_predefined_xml_items(
    items: &[PredefinedDataXmlItem],
) -> Result<BTreeMap<String, PredefinedDataXmlItem>> {
    let mut by_id = BTreeMap::new();
    flatten_predefined_xml_items_into(items, &mut by_id)?;
    Ok(by_id)
}

fn flatten_predefined_xml_items_into(
    items: &[PredefinedDataXmlItem],
    by_id: &mut BTreeMap<String, PredefinedDataXmlItem>,
) -> Result<()> {
    for item in items {
        if by_id.insert(item.id.clone(), item.clone()).is_some() {
            return Err(anyhow!("duplicate PredefinedData item id {}", item.id));
        }
        flatten_predefined_xml_items_into(&item.children, by_id)?;
    }
    Ok(())
}

fn collect_predefined_replacements(
    plain: &str,
    table_range: Range<usize>,
    by_id: &BTreeMap<String, PredefinedDataXmlItem>,
    seen: &mut BTreeSet<String>,
    replacements: &mut Vec<(Range<usize>, String)>,
) -> Result<()> {
    let table_fields = scan_wrapped_braced_fields(plain, table_range)?;
    for field in table_fields {
        if !range_starts_with_brace(plain, &field) {
            continue;
        }
        let rowset_fields = scan_wrapped_braced_fields(plain, field)?;
        if rowset_fields
            .first()
            .map(|range| plain[range.clone()].trim())
            != Some("2")
        {
            continue;
        }
        for child_field in rowset_fields {
            if range_starts_with_brace(plain, &child_field)
                && scan_wrapped_braced_fields(plain, child_field.clone())
                    .ok()
                    .and_then(|fields| {
                        fields
                            .first()
                            .map(|range| plain[range.clone()].trim() == "1")
                    })
                    == Some(true)
            {
                collect_predefined_children_replacements(
                    plain,
                    child_field,
                    by_id,
                    seen,
                    replacements,
                )?;
            }
        }
    }
    Ok(())
}

fn collect_predefined_children_replacements(
    plain: &str,
    children_range: Range<usize>,
    by_id: &BTreeMap<String, PredefinedDataXmlItem>,
    seen: &mut BTreeSet<String>,
    replacements: &mut Vec<(Range<usize>, String)>,
) -> Result<()> {
    let fields = scan_wrapped_braced_fields(plain, children_range)?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some("1") {
        return Ok(());
    }
    let count = fields
        .get(1)
        .ok_or_else(|| anyhow!("PredefinedData children list has no count"))?
        .clone();
    let count = plain[count]
        .trim()
        .parse::<usize>()
        .context("invalid PredefinedData children count")?;
    for item_range in fields.into_iter().skip(2).take(count) {
        collect_predefined_item_replacements(plain, item_range, by_id, seen, replacements)?;
    }
    Ok(())
}

fn collect_predefined_item_replacements(
    plain: &str,
    item_range: Range<usize>,
    by_id: &BTreeMap<String, PredefinedDataXmlItem>,
    seen: &mut BTreeSet<String>,
    replacements: &mut Vec<(Range<usize>, String)>,
) -> Result<()> {
    let fields = scan_wrapped_braced_fields(plain, item_range)?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some("2") {
        return Ok(());
    }
    let value_count = fields
        .get(2)
        .ok_or_else(|| anyhow!("PredefinedData item has no value count"))?;
    let value_count = plain[value_count.clone()]
        .trim()
        .parse::<usize>()
        .context("invalid PredefinedData item value count")?;
    let value_start = 3usize;
    let after_values = value_start + value_count;
    if fields.len() < after_values {
        return Err(anyhow!(
            "PredefinedData item has {} fields, expected at least {}",
            fields.len(),
            after_values
        ));
    }
    let id = parse_predefined_uuid_value_from_plain(plain, fields[value_start].clone())?;
    if let Some(item) = by_id.get(&id) {
        seen.insert(id);
        let is_folder_range = fields[value_start + 1].clone();
        if parse_predefined_bool_value_from_plain(plain, is_folder_range.clone()).is_some() {
            replacements.push((
                is_folder_range,
                format_predefined_bool_value(item.is_folder),
            ));
        }
        let has_parent_ref = fields
            .get(value_start + 2)
            .and_then(|range| scan_wrapped_braced_fields(plain, range.clone()).ok())
            .and_then(|fields| {
                fields
                    .first()
                    .map(|range| plain[range.clone()].trim() == r##""#""##)
            })
            .unwrap_or(false);
        let name_offset = if has_parent_ref {
            value_start + 3
        } else {
            value_start + 2
        };
        push_predefined_string_replacement(
            plain,
            fields.get(name_offset).cloned(),
            &item.name,
            replacements,
        );
        push_predefined_string_replacement(
            plain,
            fields.get(name_offset + 1).cloned(),
            &item.code,
            replacements,
        );
        push_predefined_string_replacement(
            plain,
            fields.get(name_offset + 2).cloned(),
            &item.description,
            replacements,
        );
    }
    if fields
        .get(after_values)
        .is_some_and(|range| plain[range.clone()].trim() == "1")
        && let Some(children_range) = fields.get(after_values + 1)
    {
        collect_predefined_children_replacements(
            plain,
            children_range.clone(),
            by_id,
            seen,
            replacements,
        )?;
    }
    Ok(())
}

fn push_predefined_string_replacement(
    plain: &str,
    range: Option<Range<usize>>,
    value: &str,
    replacements: &mut Vec<(Range<usize>, String)>,
) {
    let Some(range) = range else {
        return;
    };
    if parse_predefined_string_value_from_plain(plain, range.clone()).is_some() {
        replacements.push((range, format_predefined_string_value(value)));
    }
}

fn scan_wrapped_braced_fields(plain: &str, range: Range<usize>) -> Result<Vec<Range<usize>>> {
    let mut range = trim_ascii_ws_range(plain, range);
    let mut fields = scan_braced_fields(plain, range.start)?;
    while fields.len() == 1 && range_starts_with_brace(plain, &fields[0]) {
        range = fields[0].clone();
        fields = scan_braced_fields(plain, range.start)?;
    }
    Ok(fields)
}

fn range_starts_with_brace(plain: &str, range: &Range<usize>) -> bool {
    plain[range.clone()].trim_start().starts_with('{')
}

fn parse_predefined_uuid_value_from_plain(plain: &str, range: Range<usize>) -> Result<String> {
    let fields = scan_wrapped_braced_fields(plain, range)?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some(r##""#""##) {
        return Err(anyhow!("PredefinedData uuid value has unexpected marker"));
    }
    let ref_fields = scan_wrapped_braced_fields(
        plain,
        fields
            .get(2)
            .ok_or_else(|| anyhow!("PredefinedData uuid value has no ref payload"))?
            .clone(),
    )?;
    let uuid = plain[ref_fields
        .get(1)
        .ok_or_else(|| anyhow!("PredefinedData uuid ref has no uuid"))?
        .clone()]
    .trim();
    normalize_uuid_text(uuid)
}

fn parse_predefined_bool_value_from_plain(plain: &str, range: Range<usize>) -> Option<bool> {
    let fields = scan_wrapped_braced_fields(plain, range).ok()?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some(r#""B""#) {
        return None;
    }
    match plain[fields.get(1)?.clone()].trim() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    }
}

fn parse_predefined_string_value_from_plain(plain: &str, range: Range<usize>) -> Option<String> {
    let fields = scan_wrapped_braced_fields(plain, range).ok()?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some(r#""S""#) {
        return None;
    }
    fields
        .get(1)
        .and_then(|range| parse_1c_quoted_string(plain[range.clone()].trim()).ok())
}

fn format_predefined_bool_value(value: bool) -> String {
    format!(r#"{{{{"B",{}}}}}"#, if value { "1" } else { "0" })
}

fn format_predefined_string_value(value: &str) -> String {
    format!(r#"{{{{"S",{}}}}}"#, format_1c_string(value))
}

fn parse_predefined_data_xml(xml: &[u8]) -> Result<Vec<PredefinedDataXmlItem>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut stack = Vec::<PredefinedDataXmlItem>::new();
    let mut roots = Vec::<PredefinedDataXmlItem>::new();
    let mut text_value = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "Item"
                    && (path_ends_with(&path, &["PredefinedData"])
                        || path_ends_with(&path, &["PredefinedData", "Item", "ChildItems"])
                        || path.last().map(String::as_str) == Some("ChildItems"))
                {
                    let id = xml_attr_value(&event, "id")
                        .ok_or_else(|| anyhow!("PredefinedData Item has no id attribute"))
                        .and_then(|value| normalize_uuid_text(&value))?;
                    stack.push(PredefinedDataXmlItem {
                        id,
                        name: String::new(),
                        code: String::new(),
                        description: String::new(),
                        is_folder: false,
                        children: Vec::new(),
                    });
                }
                if matches!(local.as_str(), "Name" | "Code" | "Description" | "IsFolder") {
                    text_value.clear();
                }
                path.push(local);
            }
            Ok(Event::Text(text)) => {
                if is_predefined_item_property_path(&path) {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if is_predefined_item_property_path(&path) {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                match local.as_str() {
                    "Name" | "Code" | "Description" | "IsFolder"
                        if is_predefined_item_property_path(&path) =>
                    {
                        let item = stack
                            .last_mut()
                            .ok_or_else(|| anyhow!("PredefinedData property outside Item"))?;
                        match local.as_str() {
                            "Name" => item.name = text_value.trim().to_string(),
                            "Code" => item.code = text_value.trim().to_string(),
                            "Description" => item.description = text_value.trim().to_string(),
                            "IsFolder" => {
                                item.is_folder = parse_xml_bool_text(
                                    "PredefinedData/Item/IsFolder",
                                    text_value.trim(),
                                )?;
                            }
                            _ => {}
                        }
                    }
                    "Item" if path.last().map(String::as_str) == Some("Item") => {
                        let item = stack.pop().ok_or_else(|| {
                            anyhow!("PredefinedData ended Item without active item")
                        })?;
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(item);
                        } else {
                            roots.push(item);
                        }
                    }
                    _ => {}
                }
                let _ = path.pop();
                if matches!(local.as_str(), "Name" | "Code" | "Description" | "IsFolder") {
                    text_value.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(roots)
}

fn is_predefined_item_property_path(path: &[String]) -> bool {
    matches!(
        path.last().map(String::as_str),
        Some("Name" | "Code" | "Description" | "IsFolder")
    ) && path.iter().any(|part| part == "Item")
}

fn collect_flowchart_item_replacements(
    plain: &str,
    code: &str,
    body_range: Range<usize>,
    by_id: &BTreeMap<String, FlowchartXmlItem>,
    seen: &mut BTreeSet<String>,
    replacements: &mut Vec<(Range<usize>, String)>,
) -> Result<()> {
    let fields = scan_wrapped_braced_fields(plain, body_range)?;
    let base_range = fields
        .first()
        .ok_or_else(|| anyhow!("BusinessProcess Flowchart item has no base field"))?
        .clone();
    let (id, name_range, tab_order_range) = flowchart_base_ranges(plain, code, base_range)?;
    let Some(item) = by_id.get(&id) else {
        return Ok(());
    };
    seen.insert(id);
    push_1c_string_replacement(plain, Some(name_range), &item.name, replacements);
    replacements.push((tab_order_range, item.tab_order.clone()));

    match code {
        "2" => patch_flowchart_events(
            plain,
            fields.get(3).cloned(),
            item,
            &["BeforeStart"],
            replacements,
        )?,
        "3" => patch_flowchart_events(
            plain,
            fields.get(3).cloned(),
            item,
            &["OnComplete"],
            replacements,
        )?,
        "4" => patch_flowchart_events(
            plain,
            fields.get(3).cloned(),
            item,
            &["ConditionCheck"],
            replacements,
        )?,
        "5" => {
            if let Some(explanation) = item.explanation.as_deref() {
                push_1c_string_replacement(
                    plain,
                    fields.get(3).cloned(),
                    explanation,
                    replacements,
                );
            }
            patch_flowchart_events(
                plain,
                fields.get(5).cloned(),
                item,
                &[
                    "InteractiveActivationProcessing",
                    "BeforeCreateTasks",
                    "OnCreateTask",
                    "OnExecute",
                    "CheckExecutionProcessing",
                    "BeforeExecute",
                    "BeforeExecuteInteractively",
                ],
                replacements,
            )?;
            if let Some(task_description) = item.task_description.as_deref() {
                push_1c_string_replacement(
                    plain,
                    fields.get(7).cloned(),
                    task_description,
                    replacements,
                );
            }
        }
        _ => {}
    }
    Ok(())
}

fn flowchart_base_ranges(
    plain: &str,
    code: &str,
    base_range: Range<usize>,
) -> Result<(String, Range<usize>, Range<usize>)> {
    let head_fields = scan_wrapped_braced_fields(plain, base_range)?;
    let base_fields = if matches!(code, "2" | "3" | "4" | "5") {
        scan_wrapped_braced_fields(
            plain,
            head_fields
                .first()
                .ok_or_else(|| anyhow!("BusinessProcess Flowchart typed item has no base"))?
                .clone(),
        )?
    } else {
        head_fields
    };
    let id = plain[base_fields
        .get(1)
        .ok_or_else(|| anyhow!("BusinessProcess Flowchart item has no id"))?
        .clone()]
    .trim()
    .to_string();
    let name_range = base_fields
        .get(3)
        .ok_or_else(|| anyhow!("BusinessProcess Flowchart item has no name"))?
        .clone();
    let tab_order_range = base_fields
        .get(4)
        .ok_or_else(|| anyhow!("BusinessProcess Flowchart item has no tab order"))?
        .clone();
    Ok((id, name_range, tab_order_range))
}

fn patch_flowchart_events(
    plain: &str,
    events_range: Option<Range<usize>>,
    item: &FlowchartXmlItem,
    event_names: &[&str],
    replacements: &mut Vec<(Range<usize>, String)>,
) -> Result<()> {
    let Some(events_range) = events_range else {
        return Ok(());
    };
    let fields = scan_wrapped_braced_fields(plain, events_range)?;
    let count = plain[fields
        .first()
        .ok_or_else(|| anyhow!("BusinessProcess Flowchart events has no count"))?
        .clone()]
    .trim()
    .parse::<usize>()
    .context("invalid BusinessProcess Flowchart event count")?;
    for event_range in fields.into_iter().skip(1).take(count) {
        let event_fields = scan_wrapped_braced_fields(plain, event_range)?;
        let index = plain[event_fields
            .first()
            .ok_or_else(|| anyhow!("BusinessProcess Flowchart event has no index"))?
            .clone()]
        .trim()
        .parse::<usize>()
        .context("invalid BusinessProcess Flowchart event index")?;
        let Some(name) = event_names.get(index) else {
            continue;
        };
        let Some(handler) = item.events.get(*name) else {
            continue;
        };
        let handler = handler.as_deref().unwrap_or("");
        push_1c_string_replacement(plain, event_fields.get(1).cloned(), handler, replacements);
    }
    Ok(())
}

fn push_1c_string_replacement(
    plain: &str,
    range: Option<Range<usize>>,
    value: &str,
    replacements: &mut Vec<(Range<usize>, String)>,
) {
    let Some(range) = range else {
        return;
    };
    if parse_1c_quoted_string(plain[range.clone()].trim()).is_ok() {
        replacements.push((range, format_1c_string(value)));
    }
}

fn parse_flowchart_xml(xml: &[u8]) -> Result<Vec<FlowchartXmlItem>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut items = Vec::<FlowchartXmlItem>::new();
    let mut current = None::<FlowchartXmlItem>;
    let mut current_event = None::<String>;
    let mut text_value = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if is_flowchart_item_tag(&local)
                    && path_ends_with(&path, &["GraphicalSchema", "Items"])
                {
                    let id = xml_attr_value(&event, "id")
                        .ok_or_else(|| anyhow!("Flowchart item has no id attribute"))?;
                    current = Some(FlowchartXmlItem {
                        id,
                        name: String::new(),
                        tab_order: String::new(),
                        explanation: None,
                        task_description: None,
                        events: BTreeMap::new(),
                    });
                }
                if local == "Event" {
                    current_event = xml_attr_value(&event, "name");
                    text_value.clear();
                } else if is_flowchart_text_property(&local) {
                    text_value.clear();
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "Event"
                    && let Some(item) = current.as_mut()
                    && let Some(name) = xml_attr_value(&event, "name")
                {
                    item.events.insert(name, None);
                }
            }
            Ok(Event::Text(text)) => {
                if current.is_some()
                    && (path
                        .last()
                        .is_some_and(|part| is_flowchart_text_property(part))
                        || path.last().map(String::as_str) == Some("Event"))
                {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if current.is_some()
                    && (path
                        .last()
                        .is_some_and(|part| is_flowchart_text_property(part))
                        || path.last().map(String::as_str) == Some("Event"))
                {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if let Some(item) = current.as_mut() {
                    match local.as_str() {
                        "Name" if path_ends_with(&path, &["Properties", "Name"]) => {
                            item.name = text_value.trim().to_string();
                        }
                        "TabOrder" if path_ends_with(&path, &["Properties", "TabOrder"]) => {
                            item.tab_order = text_value.trim().to_string();
                        }
                        "Explanation" if path_ends_with(&path, &["Properties", "Explanation"]) => {
                            item.explanation = Some(text_value.trim().to_string());
                        }
                        "TaskDescription"
                            if path_ends_with(&path, &["Properties", "TaskDescription"]) =>
                        {
                            item.task_description = Some(text_value.trim().to_string());
                        }
                        "Event" => {
                            if let Some(name) = current_event.take() {
                                let handler = if text_value.trim().is_empty() {
                                    None
                                } else {
                                    Some(text_value.trim().to_string())
                                };
                                item.events.insert(name, handler);
                            }
                        }
                        _ => {}
                    }
                }
                if is_flowchart_item_tag(&local) {
                    if let Some(item) = current.take() {
                        items.push(item);
                    }
                }
                let _ = path.pop();
                if local == "Event" || is_flowchart_text_property(&local) {
                    text_value.clear();
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }
    Ok(items)
}

fn is_flowchart_item_tag(value: &str) -> bool {
    matches!(
        value,
        "Decoration" | "ConnectionLine" | "Start" | "Completion" | "Condition" | "Activity"
    )
}

fn is_flowchart_text_property(value: &str) -> bool {
    matches!(
        value,
        "Name" | "TabOrder" | "Explanation" | "TaskDescription"
    )
}

fn parse_xml_bool_text(name: &str, value: &str) -> Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(anyhow!("{name} must be true or false, got {value}")),
    }
}

pub fn pack_base64_payload_blob_from_bytes(bytes: &[u8]) -> Result<PackedRawDeflatedBlob> {
    let payload = encode_base64(bytes);
    let plain = format!("{{#base64:{payload}}}").into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn pack_ext_picture_blob_from_bytes(bytes: &[u8]) -> Result<PackedExtPictureBlob> {
    let payload = encode_base64(bytes);
    let plain = format!("{{1,{{0,0,-1,-1}},{{{{#base64:{payload}}}}}}}").into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedExtPictureBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn parse_ext_picture_file_name_from_xml(xml: &[u8]) -> Result<String> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut file_name = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                path.push(xml_local_name(event.local_name().as_ref()));
            }
            Ok(Event::Text(event)) => {
                if path_ends_with(&path, &["Picture", "Abs"]) {
                    let value = event.xml_content()?;
                    let value = unescape(value.as_ref())?;
                    file_name
                        .get_or_insert_with(String::new)
                        .push_str(value.as_ref());
                }
            }
            Ok(Event::CData(event)) => {
                if path_ends_with(&path, &["Picture", "Abs"]) {
                    file_name
                        .get_or_insert_with(String::new)
                        .push_str(event.xml_content()?.as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if path_ends_with(&path, &["Picture", "Abs"]) {
                    let value = if let Some(ch) = reference.resolve_char_ref()? {
                        ch.to_string()
                    } else {
                        let entity = reference.decode()?;
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                            .to_string()
                    };
                    file_name.get_or_insert_with(String::new).push_str(&value);
                }
            }
            Ok(Event::End(_)) => {
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    let file_name = file_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("ExtPicture Picture/xr:Abs is missing"))?;
    if file_name.contains('/') || file_name.contains('\\') || file_name == "." || file_name == ".."
    {
        return Err(anyhow!("unsupported ExtPicture file name: {file_name}"));
    }
    Ok(file_name)
}

pub fn pack_help_blob_from_parts(
    pages: &[(String, Vec<u8>)],
    files: &[(String, Vec<u8>)],
) -> Result<PackedHelpBlob> {
    if pages.is_empty() {
        return Err(anyhow!("at least one Help page is required"));
    }
    let mut fields = Vec::with_capacity(2 + pages.len() * 2 + 1 + files.len() * 3);
    fields.push("5".to_string());
    fields.push(pages.len().to_string());
    for (page, content) in pages {
        fields.push(format_1c_string(page));
        fields.push(format!("{{#base64:{}}}", encode_base64(content)));
    }
    fields.push(files.len().to_string());
    for (file_name, content) in files {
        fields.push(format_1c_string(file_name));
        fields.push("1".to_string());
        fields.push(format!("{{#base64:{}}}", encode_base64(content)));
    }
    let plain = format!("{{{}}}", fields.join(",")).into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedHelpBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn parse_help_pages_from_xml(xml: &[u8]) -> Result<Vec<String>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut pages = Vec::<String>::new();
    let mut page_text = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["Help"]) && local == "Page" {
                    page_text = Some(String::new());
                }
                path.push(local);
            }
            Ok(Event::Text(event)) => {
                if path_ends_with(&path, &["Help", "Page"])
                    && let Some(value) = page_text.as_mut()
                {
                    let text = event.xml_content()?;
                    let text = unescape(text.as_ref())?;
                    value.push_str(text.as_ref());
                }
            }
            Ok(Event::CData(event)) => {
                if path_ends_with(&path, &["Help", "Page"])
                    && let Some(value) = page_text.as_mut()
                {
                    value.push_str(event.xml_content()?.as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if path_ends_with(&path, &["Help", "Page"])
                    && let Some(value) = page_text.as_mut()
                {
                    let text = if let Some(ch) = reference.resolve_char_ref()? {
                        ch.to_string()
                    } else {
                        let entity = reference.decode()?;
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                            .to_string()
                    };
                    value.push_str(&text);
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "Page" && path_ends_with(&path, &["Help", "Page"]) {
                    let page = page_text.take().unwrap_or_default().trim().to_string();
                    if !page.is_empty() {
                        pages.push(page);
                    }
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(pages)
}

pub fn parse_template_type_from_xml(xml: &[u8]) -> Result<Option<String>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut text = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                path.push(xml_local_name(event.local_name().as_ref()));
            }
            Ok(Event::Text(event)) => {
                if path_ends_with(&path, &["Properties", "TemplateType"]) {
                    let value = event.xml_content()?;
                    let value = unescape(value.as_ref())?;
                    text.get_or_insert_with(String::new)
                        .push_str(value.as_ref());
                }
            }
            Ok(Event::CData(event)) => {
                if path_ends_with(&path, &["Properties", "TemplateType"]) {
                    text.get_or_insert_with(String::new)
                        .push_str(event.xml_content()?.as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if path_ends_with(&path, &["Properties", "TemplateType"]) {
                    let value = if let Some(ch) = reference.resolve_char_ref()? {
                        ch.to_string()
                    } else {
                        let entity = reference.decode()?;
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                            .to_string()
                    };
                    text.get_or_insert_with(String::new).push_str(&value);
                }
            }
            Ok(Event::End(_)) => {
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(text.map(|value| value.trim().to_string()))
}

#[derive(Debug, Clone)]
struct ScheduleXmlProperties {
    begin_date: String,
    end_date: String,
    begin_time: String,
    end_time: String,
    completion_time: String,
    completion_interval: String,
    repeat_period_in_day: String,
    repeat_pause: String,
    week_day_in_month: String,
    day_in_month: String,
    week_days: Vec<String>,
    months: Vec<String>,
    weeks_period: String,
    days_repeat_period: String,
}

fn parse_schedule_xml(xml: &[u8]) -> Result<ScheduleXmlProperties> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut attrs = None::<BTreeMap<String, String>>;
    let mut text_target = None::<String>;
    let mut text_value = String::new();
    let mut week_days = None::<Vec<String>>;
    let mut months = None::<Vec<String>>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["JobSchedule"]) && local == "Schedule" {
                    attrs = Some(xml_attrs_map(&event));
                } else if path_ends_with(&path, &["JobSchedule", "Schedule"])
                    && (local == "WeekDays" || local == "Months")
                {
                    text_target = Some(local.clone());
                    text_value.clear();
                }
                path.push(local);
            }
            Ok(Event::Text(text)) => {
                if text_target.is_some() {
                    let text = text.xml_content()?;
                    let text = unescape(text.as_ref())?;
                    text_value.push_str(text.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if text_target.is_some() {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                if text_target.is_some() {
                    let text = if let Some(ch) = reference.resolve_char_ref()? {
                        ch.to_string()
                    } else {
                        let entity = reference.decode()?;
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                            .to_string()
                    };
                    text_value.push_str(&text);
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if text_target.as_deref() == Some(local.as_str()) {
                    let values = parse_schedule_number_text_list(&text_value)?;
                    if local == "WeekDays" {
                        week_days = Some(values);
                    } else if local == "Months" {
                        months = Some(values);
                    }
                    text_target = None;
                    text_value.clear();
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    let attrs = attrs.ok_or_else(|| anyhow!("JobSchedule/Schedule element is missing"))?;
    Ok(ScheduleXmlProperties {
        begin_date: required_schedule_attr(&attrs, "BeginDate")?,
        end_date: required_schedule_attr(&attrs, "EndDate")?,
        begin_time: required_schedule_attr(&attrs, "BeginTime")?,
        end_time: required_schedule_attr(&attrs, "EndTime")?,
        completion_time: required_schedule_attr(&attrs, "CompletionTime")?,
        completion_interval: required_schedule_number_attr(&attrs, "CompletionInterval")?,
        repeat_period_in_day: required_schedule_number_attr(&attrs, "RepeatPeriodInDay")?,
        repeat_pause: required_schedule_number_attr(&attrs, "RepeatPause")?,
        week_day_in_month: required_schedule_number_attr(&attrs, "WeekDayInMonth")?,
        day_in_month: required_schedule_number_attr(&attrs, "DayInMonth")?,
        week_days: week_days.unwrap_or_default(),
        months: months.unwrap_or_default(),
        weeks_period: required_schedule_number_attr(&attrs, "WeeksPeriod")?,
        days_repeat_period: required_schedule_number_attr(&attrs, "DaysRepeatPeriod")?,
    })
}

fn required_schedule_attr(attrs: &BTreeMap<String, String>, name: &str) -> Result<String> {
    attrs
        .get(name)
        .cloned()
        .ok_or_else(|| anyhow!("JobSchedule/Schedule @{name} is missing"))
}

fn required_schedule_number_attr(attrs: &BTreeMap<String, String>, name: &str) -> Result<String> {
    let value = required_schedule_attr(attrs, name)?;
    validate_schedule_number(&value)?;
    Ok(value)
}

fn parse_schedule_number_text_list(text: &str) -> Result<Vec<String>> {
    text.split_whitespace()
        .map(|value| {
            validate_schedule_number(value)?;
            Ok(value.to_string())
        })
        .collect()
}

fn validate_schedule_number(value: &str) -> Result<()> {
    if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
        Ok(())
    } else {
        Err(anyhow!("invalid schedule number: {value}"))
    }
}

fn format_schedule_date(value: &str) -> Result<String> {
    let mut parts = value.split('-');
    let year = parts
        .next()
        .ok_or_else(|| anyhow!("invalid schedule date: {value}"))?;
    let month = parts
        .next()
        .ok_or_else(|| anyhow!("invalid schedule date: {value}"))?;
    let day = parts
        .next()
        .ok_or_else(|| anyhow!("invalid schedule date: {value}"))?;
    if parts.next().is_some()
        || year.len() != 4
        || month.len() != 2
        || day.len() != 2
        || !year
            .chars()
            .chain(month.chars())
            .chain(day.chars())
            .all(|ch| ch.is_ascii_digit())
    {
        return Err(anyhow!("invalid schedule date: {value}"));
    }
    Ok(format!("{year}{month}{day}000000"))
}

fn format_schedule_time(value: &str) -> Result<String> {
    let mut parts = value.split(':');
    let hour = parts
        .next()
        .ok_or_else(|| anyhow!("invalid schedule time: {value}"))?;
    let minute = parts
        .next()
        .ok_or_else(|| anyhow!("invalid schedule time: {value}"))?;
    let second = parts
        .next()
        .ok_or_else(|| anyhow!("invalid schedule time: {value}"))?;
    if parts.next().is_some()
        || hour.len() != 2
        || minute.len() != 2
        || second.len() != 2
        || !hour
            .chars()
            .chain(minute.chars())
            .chain(second.chars())
            .all(|ch| ch.is_ascii_digit())
    {
        return Err(anyhow!("invalid schedule time: {value}"));
    }
    Ok(format!("00010101{hour}{minute}{second}"))
}

#[derive(Debug, Clone)]
struct ConstantXmlProperties {
    simple: SimpleMetadataXmlProperties,
    value_type: MetadataTypePatternElement,
    use_standard_commands: bool,
}

#[derive(Debug, Clone)]
struct DefinedTypeXmlProperties {
    simple: SimpleMetadataXmlProperties,
    value_types: Vec<MetadataTypePatternElement>,
}

#[derive(Debug, Clone)]
struct CommonCommandXmlProperties {
    simple: SimpleMetadataXmlProperties,
    picture: CommonCommandPicture,
    representation: CommonCommandRepresentation,
    tooltip: Vec<LocalizedString>,
    include_help_in_contents: bool,
    group: CommonCommandGroupReference,
    command_parameter_type: CommonCommandParameterType,
    parameter_use_mode: CommonCommandParameterUseMode,
    modifies_data: bool,
    on_main_server_unavailable_behavior: CommonCommandOnMainServerUnavailableBehavior,
}

#[derive(Debug, Clone)]
struct CommandGroupXmlProperties {
    simple: SimpleMetadataXmlProperties,
    picture: CommandGroupPicture,
    representation: CommonCommandRepresentation,
    tooltip: Vec<LocalizedString>,
    category: CommandGroupCategory,
}

#[derive(Debug, Clone)]
enum MetadataTypePatternElement {
    Boolean,
    String {
        length: Option<u32>,
        allowed_length_flag: u8,
    },
    Number {
        digits: u32,
        fraction_digits: u32,
        allowed_sign_flag: u8,
    },
    DateTime,
    Reference {
        type_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommonCommandRepresentation {
    Text,
    Auto,
    Picture,
    PictureAndText,
}

#[derive(Debug, Clone, Copy)]
enum CommonCommandParameterUseMode {
    Single,
    Multiple,
}

#[derive(Debug, Clone, Copy)]
enum CommonCommandOnMainServerUnavailableBehavior {
    Auto,
}

#[derive(Debug, Clone)]
enum CommonCommandPicture {
    Empty,
    CommonPicture {
        uuid: String,
        load_transparent: bool,
    },
}

#[derive(Debug, Clone)]
enum CommonCommandGroupReference {
    BuiltIn { uuid: String },
    CommandGroup { uuid: String },
}

#[derive(Debug, Clone)]
enum CommonCommandParameterType {
    Empty,
    DefinedType { type_id: String },
}

#[derive(Debug, Clone)]
enum CommandGroupPicture {
    Empty,
    CommonPicture {
        uuid: String,
        load_transparent: bool,
    },
    StdPicturePrint,
}

#[derive(Debug, Clone, Copy)]
enum CommandGroupCategory {
    NavigationPanel,
    FormNavigationPanel,
    ActionsPanel,
    FormCommandBar,
}

fn parse_constant_xml_properties(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<ConstantXmlProperties> {
    let simple = parse_simple_metadata_xml_properties(xml)?;
    if simple.kind != "Constant" {
        return Err(anyhow!(
            "expected Constant XML, got metadata kind {}",
            simple.kind
        ));
    }

    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();

    let mut types = Vec::<String>::new();
    let mut string_length = None::<String>;
    let mut string_allowed_length = None::<String>;
    let mut number_digits = None::<String>;
    let mut number_fraction_digits = None::<String>;
    let mut number_allowed_sign = None::<String>;
    let mut use_standard_commands = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                path.push(xml_local_name(event.local_name().as_ref()));
            }
            Ok(Event::Text(text)) => {
                let value = text.xml_content()?;
                let value = unescape(value.as_ref())?;
                append_constant_xml_text(
                    &path,
                    value.as_ref(),
                    &mut types,
                    &mut string_length,
                    &mut string_allowed_length,
                    &mut number_digits,
                    &mut number_fraction_digits,
                    &mut number_allowed_sign,
                    &mut use_standard_commands,
                );
            }
            Ok(Event::CData(text)) => {
                append_constant_xml_text(
                    &path,
                    text.xml_content()?.as_ref(),
                    &mut types,
                    &mut string_length,
                    &mut string_allowed_length,
                    &mut number_digits,
                    &mut number_fraction_digits,
                    &mut number_allowed_sign,
                    &mut use_standard_commands,
                );
            }
            Ok(Event::GeneralRef(reference)) => {
                let value = if let Some(ch) = reference.resolve_char_ref()? {
                    ch.to_string()
                } else {
                    let entity = reference.decode()?;
                    resolve_xml_entity(entity.as_ref())
                        .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                        .to_string()
                };
                append_constant_xml_text(
                    &path,
                    &value,
                    &mut types,
                    &mut string_length,
                    &mut string_allowed_length,
                    &mut number_digits,
                    &mut number_fraction_digits,
                    &mut number_allowed_sign,
                    &mut use_standard_commands,
                );
            }
            Ok(Event::End(_)) => {
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    let value_type = parse_constant_value_type(
        &types,
        string_length,
        string_allowed_length,
        number_digits,
        number_fraction_digits,
        number_allowed_sign,
        source,
    )?;
    let use_standard_commands =
        parse_required_metadata_bool("Constant", "UseStandardCommands", use_standard_commands)?;

    Ok(ConstantXmlProperties {
        simple,
        value_type,
        use_standard_commands,
    })
}

fn parse_defined_type_xml_properties(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<DefinedTypeXmlProperties> {
    let simple = parse_simple_metadata_xml_properties(xml)?;
    if simple.kind != "DefinedType" {
        return Err(anyhow!(
            "expected DefinedType XML, got metadata kind {}",
            simple.kind
        ));
    }

    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();

    let mut types = Vec::<String>::new();
    let mut string_length = None::<String>;
    let mut string_allowed_length = None::<String>;
    let mut number_digits = None::<String>;
    let mut number_fraction_digits = None::<String>;
    let mut number_allowed_sign = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                path.push(xml_local_name(event.local_name().as_ref()));
            }
            Ok(Event::Text(text)) => {
                let value = text.xml_content()?;
                let value = unescape(value.as_ref())?;
                append_metadata_type_xml_text(
                    "DefinedType",
                    &path,
                    value.as_ref(),
                    &mut types,
                    &mut string_length,
                    &mut string_allowed_length,
                    &mut number_digits,
                    &mut number_fraction_digits,
                    &mut number_allowed_sign,
                );
            }
            Ok(Event::CData(text)) => {
                append_metadata_type_xml_text(
                    "DefinedType",
                    &path,
                    text.xml_content()?.as_ref(),
                    &mut types,
                    &mut string_length,
                    &mut string_allowed_length,
                    &mut number_digits,
                    &mut number_fraction_digits,
                    &mut number_allowed_sign,
                );
            }
            Ok(Event::GeneralRef(reference)) => {
                let value = if let Some(ch) = reference.resolve_char_ref()? {
                    ch.to_string()
                } else {
                    let entity = reference.decode()?;
                    resolve_xml_entity(entity.as_ref())
                        .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                        .to_string()
                };
                append_metadata_type_xml_text(
                    "DefinedType",
                    &path,
                    &value,
                    &mut types,
                    &mut string_length,
                    &mut string_allowed_length,
                    &mut number_digits,
                    &mut number_fraction_digits,
                    &mut number_allowed_sign,
                );
            }
            Ok(Event::End(_)) => {
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    let value_types = parse_metadata_type_pattern_elements(
        "DefinedType",
        &types,
        string_length,
        string_allowed_length,
        number_digits,
        number_fraction_digits,
        number_allowed_sign,
        source,
        true,
    )?;

    Ok(DefinedTypeXmlProperties {
        simple,
        value_types,
    })
}

fn parse_common_command_xml_properties(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<CommonCommandXmlProperties> {
    let simple = parse_simple_metadata_xml_properties(xml)?;
    if simple.kind != "CommonCommand" {
        return Err(anyhow!(
            "expected CommonCommand XML, got metadata kind {}",
            simple.kind
        ));
    }

    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();

    let mut representation = None::<String>;
    let mut picture_ref = None::<String>;
    let mut picture_load_transparent = None::<String>;
    let mut tooltip = Vec::<LocalizedString>::new();
    let mut pending_tooltip_lang = None::<String>;
    let mut pending_tooltip_content = None::<String>;
    let mut include_help_in_contents = None::<String>;
    let mut group = None::<String>;
    let mut command_parameter_types = Vec::<String>::new();
    let mut parameter_use_mode = None::<String>;
    let mut modifies_data = None::<String>;
    let mut on_main_server_unavailable_behavior = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["CommonCommand", "Properties", "ToolTip"])
                    && local == "item"
                {
                    pending_tooltip_lang = None;
                    pending_tooltip_content = None;
                }
                path.push(local);
            }
            Ok(Event::Text(text)) => {
                let value = text.xml_content()?;
                let value = unescape(value.as_ref())?;
                append_common_command_xml_text(
                    &path,
                    value.as_ref(),
                    &mut representation,
                    &mut picture_ref,
                    &mut picture_load_transparent,
                    &mut pending_tooltip_lang,
                    &mut pending_tooltip_content,
                    &mut include_help_in_contents,
                    &mut group,
                    &mut command_parameter_types,
                    &mut parameter_use_mode,
                    &mut modifies_data,
                    &mut on_main_server_unavailable_behavior,
                );
            }
            Ok(Event::CData(text)) => {
                append_common_command_xml_text(
                    &path,
                    text.xml_content()?.as_ref(),
                    &mut representation,
                    &mut picture_ref,
                    &mut picture_load_transparent,
                    &mut pending_tooltip_lang,
                    &mut pending_tooltip_content,
                    &mut include_help_in_contents,
                    &mut group,
                    &mut command_parameter_types,
                    &mut parameter_use_mode,
                    &mut modifies_data,
                    &mut on_main_server_unavailable_behavior,
                );
            }
            Ok(Event::GeneralRef(reference)) => {
                let value = if let Some(ch) = reference.resolve_char_ref()? {
                    ch.to_string()
                } else {
                    let entity = reference.decode()?;
                    resolve_xml_entity(entity.as_ref())
                        .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                        .to_string()
                };
                append_common_command_xml_text(
                    &path,
                    &value,
                    &mut representation,
                    &mut picture_ref,
                    &mut picture_load_transparent,
                    &mut pending_tooltip_lang,
                    &mut pending_tooltip_content,
                    &mut include_help_in_contents,
                    &mut group,
                    &mut command_parameter_types,
                    &mut parameter_use_mode,
                    &mut modifies_data,
                    &mut on_main_server_unavailable_behavior,
                );
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "item"
                    && path_ends_with(&path, &["CommonCommand", "Properties", "ToolTip", "item"])
                {
                    if let Some(lang) = pending_tooltip_lang.take() {
                        tooltip.push(LocalizedString {
                            lang,
                            content: pending_tooltip_content.take().unwrap_or_default(),
                        });
                    }
                    pending_tooltip_content = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(CommonCommandXmlProperties {
        simple,
        picture: parse_common_command_picture(picture_ref, picture_load_transparent, source)?,
        representation: parse_common_command_representation(representation)?,
        tooltip,
        include_help_in_contents: parse_required_metadata_bool(
            "CommonCommand",
            "IncludeHelpInContents",
            include_help_in_contents,
        )?,
        group: parse_common_command_group_reference(group, source)?,
        command_parameter_type: parse_common_command_parameter_type(
            &command_parameter_types,
            source,
        )?,
        parameter_use_mode: parse_common_command_parameter_use_mode(parameter_use_mode)?,
        modifies_data: parse_required_metadata_bool(
            "CommonCommand",
            "ModifiesData",
            modifies_data,
        )?,
        on_main_server_unavailable_behavior:
            parse_common_command_on_main_server_unavailable_behavior(
                on_main_server_unavailable_behavior,
            )?,
    })
}

fn parse_command_group_xml_properties(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<CommandGroupXmlProperties> {
    let simple = parse_simple_metadata_xml_properties(xml)?;
    if simple.kind != "CommandGroup" {
        return Err(anyhow!(
            "expected CommandGroup XML, got metadata kind {}",
            simple.kind
        ));
    }

    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();

    let mut representation = None::<String>;
    let mut picture_ref = None::<String>;
    let mut picture_load_transparent = None::<String>;
    let mut tooltip = Vec::<LocalizedString>::new();
    let mut pending_tooltip_lang = None::<String>;
    let mut pending_tooltip_content = None::<String>;
    let mut category = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["CommandGroup", "Properties", "ToolTip"])
                    && local == "item"
                {
                    pending_tooltip_lang = None;
                    pending_tooltip_content = None;
                }
                path.push(local);
            }
            Ok(Event::Text(text)) => {
                let value = text.xml_content()?;
                let value = unescape(value.as_ref())?;
                append_command_group_xml_text(
                    &path,
                    value.as_ref(),
                    &mut representation,
                    &mut picture_ref,
                    &mut picture_load_transparent,
                    &mut pending_tooltip_lang,
                    &mut pending_tooltip_content,
                    &mut category,
                );
            }
            Ok(Event::CData(text)) => {
                append_command_group_xml_text(
                    &path,
                    text.xml_content()?.as_ref(),
                    &mut representation,
                    &mut picture_ref,
                    &mut picture_load_transparent,
                    &mut pending_tooltip_lang,
                    &mut pending_tooltip_content,
                    &mut category,
                );
            }
            Ok(Event::GeneralRef(reference)) => {
                let value = if let Some(ch) = reference.resolve_char_ref()? {
                    ch.to_string()
                } else {
                    let entity = reference.decode()?;
                    resolve_xml_entity(entity.as_ref())
                        .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                        .to_string()
                };
                append_command_group_xml_text(
                    &path,
                    &value,
                    &mut representation,
                    &mut picture_ref,
                    &mut picture_load_transparent,
                    &mut pending_tooltip_lang,
                    &mut pending_tooltip_content,
                    &mut category,
                );
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "item"
                    && path_ends_with(&path, &["CommandGroup", "Properties", "ToolTip", "item"])
                {
                    if let Some(lang) = pending_tooltip_lang.take() {
                        tooltip.push(LocalizedString {
                            lang,
                            content: pending_tooltip_content.take().unwrap_or_default(),
                        });
                    }
                    pending_tooltip_content = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(CommandGroupXmlProperties {
        simple,
        picture: parse_command_group_picture(picture_ref, picture_load_transparent, source)?,
        representation: parse_common_command_representation(representation)?,
        tooltip,
        category: parse_command_group_category(category)?,
    })
}

pub fn parse_simple_metadata_xml_properties(xml: &[u8]) -> Result<SimpleMetadataXmlProperties> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();

    let mut kind = None::<String>;
    let mut uuid = None::<String>;
    let mut name = None::<String>;
    let mut comment = None::<String>;
    let mut synonyms = Vec::<LocalizedString>::new();
    let mut pending_synonym_lang = None::<String>;
    let mut pending_synonym_content = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["MetaDataObject"]) && kind.is_none() {
                    kind = Some(local.clone());
                    if let Some(value) = xml_attr_value(&event, "uuid") {
                        uuid = Some(normalize_uuid_text(&value)?);
                    }
                }
                if is_simple_metadata_properties_path(&path, kind.as_deref()) {
                    if local == "Name" {
                        name = Some(String::new());
                    } else if local == "Comment" {
                        comment = Some(String::new());
                    }
                }
                if is_simple_metadata_synonym_path(&path, kind.as_deref()) && local == "item" {
                    pending_synonym_lang = None;
                    pending_synonym_content = None;
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["MetaDataObject"]) && kind.is_none() {
                    kind = Some(local.clone());
                    if let Some(value) = xml_attr_value(&event, "uuid") {
                        uuid = Some(normalize_uuid_text(&value)?);
                    }
                } else if is_simple_metadata_properties_path(&path, kind.as_deref())
                    && local == "Comment"
                {
                    comment = Some(String::new());
                }
            }
            Ok(Event::Text(text)) => {
                let value = text.xml_content()?;
                let value = unescape(value.as_ref())?;
                append_simple_metadata_xml_text(
                    &path,
                    kind.as_deref(),
                    value.as_ref(),
                    &mut name,
                    &mut comment,
                    &mut pending_synonym_lang,
                    &mut pending_synonym_content,
                );
            }
            Ok(Event::CData(text)) => {
                append_simple_metadata_xml_text(
                    &path,
                    kind.as_deref(),
                    text.xml_content()?.as_ref(),
                    &mut name,
                    &mut comment,
                    &mut pending_synonym_lang,
                    &mut pending_synonym_content,
                );
            }
            Ok(Event::GeneralRef(reference)) => {
                let value = if let Some(ch) = reference.resolve_char_ref()? {
                    ch.to_string()
                } else {
                    let entity = reference.decode()?;
                    resolve_xml_entity(entity.as_ref())
                        .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                        .to_string()
                };
                append_simple_metadata_xml_text(
                    &path,
                    kind.as_deref(),
                    &value,
                    &mut name,
                    &mut comment,
                    &mut pending_synonym_lang,
                    &mut pending_synonym_content,
                );
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "item" && is_simple_metadata_synonym_item_path(&path, kind.as_deref()) {
                    if let Some(lang) = pending_synonym_lang.take() {
                        synonyms.push(LocalizedString {
                            lang,
                            content: pending_synonym_content.take().unwrap_or_default(),
                        });
                    }
                    pending_synonym_content = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    let kind = kind.ok_or_else(|| anyhow!("metadata object kind not found in XML"))?;
    Ok(SimpleMetadataXmlProperties {
        kind: kind.clone(),
        uuid: uuid.ok_or_else(|| anyhow!("{kind} uuid not found in XML"))?,
        name: name.ok_or_else(|| anyhow!("{kind} Properties/Name not found in XML"))?,
        synonyms,
        comment: comment.unwrap_or_default(),
    })
}

pub fn parse_common_module_xml_properties(xml: &[u8]) -> Result<CommonModuleXmlProperties> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();

    let mut uuid = None::<String>;
    let mut name = None::<String>;
    let mut comment = None::<String>;
    let mut synonyms = Vec::<LocalizedString>::new();
    let mut pending_synonym_lang = None::<String>;
    let mut pending_synonym_content = None::<String>;
    let mut global = None::<String>;
    let mut client_managed_application = None::<String>;
    let mut server = None::<String>;
    let mut external_connection = None::<String>;
    let mut client_ordinary_application = None::<String>;
    let mut server_call = None::<String>;
    let mut privileged = None::<String>;
    let mut return_values_reuse = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "CommonModule" {
                    if let Some(value) = xml_attr_value(&event, "uuid") {
                        uuid = Some(normalize_uuid_text(&value)?);
                    }
                }
                if path_ends_with(&path, &["CommonModule", "Properties"]) {
                    if local == "Name" {
                        name = Some(String::new());
                    } else if local == "Comment" {
                        comment = Some(String::new());
                    }
                }
                if path_ends_with(&path, &["CommonModule", "Properties", "Synonym"])
                    && local == "item"
                {
                    pending_synonym_lang = None;
                    pending_synonym_content = None;
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "CommonModule" {
                    if let Some(value) = xml_attr_value(&event, "uuid") {
                        uuid = Some(normalize_uuid_text(&value)?);
                    }
                } else if path_ends_with(&path, &["CommonModule", "Properties"])
                    && local == "Comment"
                {
                    comment = Some(String::new());
                }
            }
            Ok(Event::Text(text)) => {
                let value = text.xml_content()?;
                let value = unescape(value.as_ref())?;
                append_common_module_xml_text(
                    &path,
                    value.as_ref(),
                    &mut name,
                    &mut comment,
                    &mut pending_synonym_lang,
                    &mut pending_synonym_content,
                    &mut global,
                    &mut client_managed_application,
                    &mut server,
                    &mut external_connection,
                    &mut client_ordinary_application,
                    &mut server_call,
                    &mut privileged,
                    &mut return_values_reuse,
                );
            }
            Ok(Event::CData(text)) => {
                append_common_module_xml_text(
                    &path,
                    text.xml_content()?.as_ref(),
                    &mut name,
                    &mut comment,
                    &mut pending_synonym_lang,
                    &mut pending_synonym_content,
                    &mut global,
                    &mut client_managed_application,
                    &mut server,
                    &mut external_connection,
                    &mut client_ordinary_application,
                    &mut server_call,
                    &mut privileged,
                    &mut return_values_reuse,
                );
            }
            Ok(Event::GeneralRef(reference)) => {
                let value = if let Some(ch) = reference.resolve_char_ref()? {
                    ch.to_string()
                } else {
                    let entity = reference.decode()?;
                    resolve_xml_entity(entity.as_ref())
                        .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                        .to_string()
                };
                append_common_module_xml_text(
                    &path,
                    &value,
                    &mut name,
                    &mut comment,
                    &mut pending_synonym_lang,
                    &mut pending_synonym_content,
                    &mut global,
                    &mut client_managed_application,
                    &mut server,
                    &mut external_connection,
                    &mut client_ordinary_application,
                    &mut server_call,
                    &mut privileged,
                    &mut return_values_reuse,
                );
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "item"
                    && path_ends_with(&path, &["CommonModule", "Properties", "Synonym", "item"])
                {
                    if let Some(lang) = pending_synonym_lang.take() {
                        synonyms.push(LocalizedString {
                            lang,
                            content: pending_synonym_content.take().unwrap_or_default(),
                        });
                    }
                    pending_synonym_content = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Ok(CommonModuleXmlProperties {
        uuid: uuid.ok_or_else(|| anyhow!("CommonModule uuid not found in XML"))?,
        name: name.ok_or_else(|| anyhow!("CommonModule Properties/Name not found in XML"))?,
        synonyms,
        comment: comment.unwrap_or_default(),
        global: parse_required_xml_bool("Global", global)?,
        client_managed_application: parse_required_xml_bool(
            "ClientManagedApplication",
            client_managed_application,
        )?,
        server: parse_required_xml_bool("Server", server)?,
        external_connection: parse_required_xml_bool("ExternalConnection", external_connection)?,
        client_ordinary_application: parse_required_xml_bool(
            "ClientOrdinaryApplication",
            client_ordinary_application,
        )?,
        server_call: parse_required_xml_bool("ServerCall", server_call)?,
        privileged: parse_required_xml_bool("Privileged", privileged)?,
        return_values_reuse: parse_return_values_reuse(return_values_reuse)?,
    })
}

pub fn patch_versions_blob(args: &VersionsBlobPatchArgs) -> Result<VersionsBlobPatchReport> {
    let input = fs::read(&args.input)
        .with_context(|| format!("failed to read versions blob {}", args.input.display()))?;
    let patched = patch_versions_blob_bytes(&input, &args.changes, !args.no_standard_entries)?;

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&args.output, &patched.blob)
        .with_context(|| format!("failed to write {}", args.output.display()))?;

    Ok(VersionsBlobPatchReport {
        input: args.input.clone(),
        output: args.output.clone(),
        plain_bytes: patched.plain_bytes,
        output_bytes: patched.blob.len(),
        output_sha256: patched.output_sha256,
        replacements: patched.replacements,
    })
}

pub fn patch_versions_blob_bytes(
    input: &[u8],
    changes: &[String],
    include_standard_entries: bool,
) -> Result<PatchedVersionsBlob> {
    let plain = inflate_raw(input).context("failed to inflate versions blob")?;
    let mut text = String::from_utf8(plain).context("versions blob is not valid UTF-8")?;

    let mut replacements = Vec::new();
    replacements.push(replace_header_uuid(&mut text)?);

    let mut names = Vec::new();
    if include_standard_entries {
        names.extend([
            "root".to_string(),
            "version".to_string(),
            "versions".to_string(),
        ]);
    }
    names.extend(changes.iter().cloned());
    names.sort();
    names.dedup();

    for name in names {
        replacements.push(replace_named_uuid(&mut text, &name)?);
    }

    let plain = text.into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PatchedVersionsBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
        replacements,
    })
}

fn patch_common_module_metadata_text(
    mut text: String,
    properties: &CommonModuleXmlProperties,
) -> Result<String> {
    text = patch_simple_metadata_header_text(
        text,
        &SimpleMetadataXmlProperties {
            kind: "CommonModule".to_string(),
            uuid: properties.uuid.clone(),
            name: properties.name.clone(),
            synonyms: properties.synonyms.clone(),
            comment: properties.comment.clone(),
        },
    )?;

    let marker = format!("{{1,0,{}}},", properties.uuid);
    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("metadata tuple not found for {}", properties.uuid))?;

    let base_object_start = text[..marker_start]
        .rfind("{3,")
        .ok_or_else(|| anyhow!("common module base object marker not found"))?;
    let owner_object_start = text[..base_object_start]
        .rfind("{12,")
        .ok_or_else(|| anyhow!("common module object marker not found"))?;
    let base_object_end = scan_balanced_braces(&text, base_object_start)?;
    let flags_start = expect_byte_after_ws(&text, base_object_end, b',')?;
    let owner_object_end = scan_balanced_braces(&text, owner_object_start)?;
    let flags_end = owner_object_end
        .checked_sub(1)
        .ok_or_else(|| anyhow!("common module object end is invalid"))?;
    let flags_text = format_common_module_flags(properties);
    text.replace_range(flags_start..flags_end, &flags_text);

    Ok(text)
}

fn patch_simple_metadata_header_text(
    mut text: String,
    properties: &SimpleMetadataXmlProperties,
) -> Result<String> {
    let marker = format!("{{1,0,{}}},", properties.uuid);
    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("metadata tuple not found for {}", properties.uuid))?;

    let name_start = skip_ascii_ws(&text, marker_start + marker.len());
    let name_end = replace_1c_quoted_string(&mut text, name_start, &properties.name)?;

    let synonym_start = expect_byte_after_ws(&text, name_end, b',')?;
    let synonym_start = skip_ascii_ws(&text, synonym_start);
    let synonym_end = scan_balanced_braces(&text, synonym_start)?;
    let synonym_text = format_1c_synonyms(&properties.synonyms);
    text.replace_range(synonym_start..synonym_end, &synonym_text);

    let comment_comma = synonym_start + synonym_text.len();
    let comment_start = expect_byte_after_ws(&text, comment_comma, b',')?;
    let comment_start = skip_ascii_ws(&text, comment_start);
    let _ = replace_1c_quoted_string(&mut text, comment_start, &properties.comment)?;

    Ok(text)
}

fn patch_constant_metadata_text(
    mut text: String,
    properties: &ConstantXmlProperties,
) -> Result<String> {
    text = patch_simple_metadata_header_text(text, &properties.simple)?;

    let marker = format!("{{1,0,{}}}", properties.simple.uuid);
    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("metadata tuple not found for {}", properties.simple.uuid))?;

    let typed_object_start = text[..marker_start]
        .rfind("{2,")
        .ok_or_else(|| anyhow!("constant typed object marker not found"))?;
    let fields = scan_braced_fields(&text, typed_object_start)?;
    if fields.len() < 3 {
        return Err(anyhow!(
            "constant typed object has {} fields, expected at least 3",
            fields.len()
        ));
    }
    let type_text = format_constant_type_pattern(&properties.value_type);
    text.replace_range(fields[2].clone(), &type_text);

    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("metadata tuple not found for {}", properties.simple.uuid))?;
    let constant_object_start = text[..marker_start]
        .rfind("{16,")
        .ok_or_else(|| anyhow!("constant object marker not found"))?;
    let fields = scan_braced_fields(&text, constant_object_start)?;
    if fields.len() < 8 {
        return Err(anyhow!(
            "constant object has {} fields, expected at least 8",
            fields.len()
        ));
    }
    text.replace_range(
        fields[7].clone(),
        &bool_flag(properties.use_standard_commands),
    );

    Ok(text)
}

fn patch_defined_type_metadata_text(
    mut text: String,
    properties: &DefinedTypeXmlProperties,
) -> Result<String> {
    text = patch_simple_metadata_header_text(text, &properties.simple)?;

    let marker = format!("{{1,0,{}}}", properties.simple.uuid);
    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("metadata tuple not found for {}", properties.simple.uuid))?;
    let defined_type_start = text[..marker_start]
        .rfind("{0,")
        .ok_or_else(|| anyhow!("defined type object marker not found"))?;
    let fields = scan_braced_fields(&text, defined_type_start)?;
    if fields.len() < 5 {
        return Err(anyhow!(
            "defined type object has {} fields, expected at least 5",
            fields.len()
        ));
    }

    let type_text = format_metadata_type_pattern(&properties.value_types);
    text.replace_range(fields[4].clone(), &type_text);

    Ok(text)
}

fn patch_common_command_metadata_text(
    mut text: String,
    properties: &CommonCommandXmlProperties,
) -> Result<String> {
    text = patch_simple_metadata_header_text(text, &properties.simple)?;

    let marker = format!("{{1,0,{}}}", properties.simple.uuid);
    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("metadata tuple not found for {}", properties.simple.uuid))?;
    let base_object_start = text[..marker_start]
        .rfind("{3,")
        .ok_or_else(|| anyhow!("common command base object marker not found"))?;
    let command_object_start = text[..base_object_start]
        .rfind("{9,")
        .ok_or_else(|| anyhow!("common command object marker not found"))?;
    let fields = scan_braced_fields(&text, command_object_start)?;
    if fields.len() < 13 {
        return Err(anyhow!(
            "common command object has {} fields, expected at least 13",
            fields.len()
        ));
    }

    let replacements = [
        (
            fields[12].clone(),
            common_command_on_main_server_unavailable_behavior_code(
                properties.on_main_server_unavailable_behavior,
            )
            .to_string(),
        ),
        (
            fields[11].clone(),
            common_command_parameter_use_mode_code(properties.parameter_use_mode).to_string(),
        ),
        (
            fields[10].clone(),
            bool_flag(properties.modifies_data).to_string(),
        ),
        (
            fields[8].clone(),
            format_common_command_parameter_type(&properties.command_parameter_type),
        ),
        (
            fields[7].clone(),
            format_common_command_group_reference(&properties.group),
        ),
        (
            fields[6].clone(),
            bool_flag(properties.include_help_in_contents).to_string(),
        ),
        (fields[3].clone(), format_1c_synonyms(&properties.tooltip)),
        (
            fields[2].clone(),
            common_command_representation_code(properties.representation).to_string(),
        ),
        (
            fields[1].clone(),
            format_common_command_picture(&properties.picture),
        ),
    ];

    for (range, replacement) in replacements {
        text.replace_range(range, &replacement);
    }

    Ok(text)
}

fn patch_command_group_metadata_text(
    mut text: String,
    properties: &CommandGroupXmlProperties,
) -> Result<String> {
    text = patch_simple_metadata_header_text(text, &properties.simple)?;

    let marker = format!("{{1,0,{}}}", properties.simple.uuid);
    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("metadata tuple not found for {}", properties.simple.uuid))?;
    let inner_object_start = text[..marker_start]
        .rfind("{3,")
        .ok_or_else(|| anyhow!("command group inner metadata marker not found"))?;
    let command_group_start = text[..inner_object_start]
        .rfind("{3,")
        .ok_or_else(|| anyhow!("command group object marker not found"))?;
    let fields = scan_braced_fields(&text, command_group_start)?;
    if fields.len() < 7 {
        return Err(anyhow!(
            "command group object has {} fields, expected at least 7",
            fields.len()
        ));
    }

    let inner_text = text[fields[6].clone()].to_string();
    let inner_text = patch_simple_metadata_header_text(inner_text, &properties.simple)?;
    let replacements = [
        (fields[6].clone(), inner_text),
        (fields[5].clone(), r#"{0}"#.to_string()),
        (fields[4].clone(), format_1c_synonyms(&properties.tooltip)),
        (
            fields[3].clone(),
            common_command_representation_code(properties.representation).to_string(),
        ),
        (
            fields[2].clone(),
            command_group_category_code(properties.category).to_string(),
        ),
        (
            fields[1].clone(),
            format_command_group_picture(&properties.picture),
        ),
    ];

    for (range, replacement) in replacements {
        text.replace_range(range, &replacement);
    }

    Ok(text)
}

fn parse_required_xml_bool(name: &str, value: Option<String>) -> Result<bool> {
    parse_required_metadata_bool("CommonModule", name, value)
}

fn parse_required_metadata_bool(kind: &str, name: &str, value: Option<String>) -> Result<bool> {
    let value = value.ok_or_else(|| anyhow!("{kind} Properties/{name} not found in XML"))?;
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(anyhow!("invalid {kind} boolean {name}: {other}")),
    }
}

fn parse_return_values_reuse(value: Option<String>) -> Result<ReturnValuesReuse> {
    let value = value
        .ok_or_else(|| anyhow!("CommonModule Properties/ReturnValuesReuse not found in XML"))?;
    match value.trim() {
        "DontUse" => Ok(ReturnValuesReuse::DontUse),
        "DuringRequest" => Ok(ReturnValuesReuse::DuringRequest),
        "DuringSession" => Ok(ReturnValuesReuse::DuringSession),
        other => Err(anyhow!("unsupported ReturnValuesReuse value: {other}")),
    }
}

fn format_common_module_flags(properties: &CommonModuleXmlProperties) -> String {
    [
        bool_flag(properties.client_ordinary_application),
        bool_flag(properties.server),
        bool_flag(properties.external_connection),
        bool_flag(properties.privileged),
        bool_flag(properties.global),
        bool_flag(properties.client_managed_application),
        return_values_reuse_flag(properties.return_values_reuse),
        bool_flag(properties.server_call),
    ]
    .join(",")
}

fn parse_constant_value_type(
    types: &[String],
    string_length: Option<String>,
    string_allowed_length: Option<String>,
    number_digits: Option<String>,
    number_fraction_digits: Option<String>,
    number_allowed_sign: Option<String>,
    source: Option<&MetadataSourceContext>,
) -> Result<MetadataTypePatternElement> {
    let mut elements = parse_metadata_type_pattern_elements(
        "Constant",
        types,
        string_length,
        string_allowed_length,
        number_digits,
        number_fraction_digits,
        number_allowed_sign,
        source,
        false,
    )?;
    elements
        .pop()
        .ok_or_else(|| anyhow!("Constant Properties/Type is empty"))
}

fn parse_metadata_type_pattern_elements(
    kind: &str,
    types: &[String],
    string_length: Option<String>,
    string_allowed_length: Option<String>,
    number_digits: Option<String>,
    number_fraction_digits: Option<String>,
    number_allowed_sign: Option<String>,
    source: Option<&MetadataSourceContext>,
    allow_multiple: bool,
) -> Result<Vec<MetadataTypePatternElement>> {
    if types.is_empty() {
        return Err(anyhow!("{kind} Properties/Type has no Type entries"));
    }
    if !allow_multiple && types.len() != 1 {
        return Err(anyhow!(
            "only single-type {kind} values are supported, got {} types",
            types.len()
        ));
    }

    types
        .iter()
        .map(|type_name| {
            parse_metadata_type_pattern_element(
                kind,
                type_name,
                string_length.as_deref(),
                string_allowed_length.as_deref(),
                number_digits.as_deref(),
                number_fraction_digits.as_deref(),
                number_allowed_sign.as_deref(),
                source,
            )
        })
        .collect()
}

fn parse_metadata_type_pattern_element(
    kind: &str,
    type_name: &str,
    string_length: Option<&str>,
    string_allowed_length: Option<&str>,
    number_digits: Option<&str>,
    number_fraction_digits: Option<&str>,
    number_allowed_sign: Option<&str>,
    source: Option<&MetadataSourceContext>,
) -> Result<MetadataTypePatternElement> {
    match type_name.trim() {
        "xs:boolean" => Ok(MetadataTypePatternElement::Boolean),
        "xs:string" => {
            let length = parse_optional_u32("StringQualifiers/Length", string_length)?;
            let allowed_length_flag = parse_string_allowed_length_flag(string_allowed_length)?;
            Ok(MetadataTypePatternElement::String {
                length: length.filter(|value| *value > 0),
                allowed_length_flag,
            })
        }
        "xs:decimal" => Ok(MetadataTypePatternElement::Number {
            digits: parse_required_u32("NumberQualifiers/Digits", number_digits)?,
            fraction_digits: parse_required_u32(
                "NumberQualifiers/FractionDigits",
                number_fraction_digits,
            )?,
            allowed_sign_flag: parse_number_allowed_sign_flag(number_allowed_sign)?,
        }),
        "xs:dateTime" => Ok(MetadataTypePatternElement::DateTime),
        other if other.starts_with("cfg:") => {
            let source = source.ok_or_else(|| {
                anyhow!("{kind} type {other} requires --source-root to resolve TypeId")
            })?;
            Ok(MetadataTypePatternElement::Reference {
                type_id: source.resolve_metadata_type_id(other)?,
            })
        }
        other => Err(anyhow!("{kind} type is not supported yet: {other}")),
    }
}

fn parse_common_command_picture(
    reference: Option<String>,
    load_transparent: Option<String>,
    source: Option<&MetadataSourceContext>,
) -> Result<CommonCommandPicture> {
    let Some(reference) = reference.map(|value| value.trim().to_string()) else {
        return Ok(CommonCommandPicture::Empty);
    };
    if reference.is_empty() {
        return Ok(CommonCommandPicture::Empty);
    }
    if reference == "StdPicture.User" {
        let load_transparent = parse_required_metadata_bool(
            "CommonCommand",
            "Picture/LoadTransparent",
            load_transparent,
        )?;
        return Ok(CommonCommandPicture::CommonPicture {
            uuid: STD_PICTURE_USER_UUID.to_string(),
            load_transparent,
        });
    }
    if reference.starts_with("StdPicture.") {
        return Err(anyhow!(
            "CommonCommand Picture reference {reference} is not supported yet; StdPicture UUID mapping is platform-owned"
        ));
    }
    if !reference.starts_with("CommonPicture.") {
        return Err(anyhow!(
            "unsupported CommonCommand Picture reference: {reference}"
        ));
    }

    let source = source.ok_or_else(|| {
        anyhow!(
            "CommonCommand Picture {reference} requires --source-root to resolve CommonPicture UUID"
        )
    })?;
    let uuid = source.resolve_common_picture_uuid(&reference)?;
    let load_transparent =
        parse_required_metadata_bool("CommonCommand", "Picture/LoadTransparent", load_transparent)?;

    Ok(CommonCommandPicture::CommonPicture {
        uuid,
        load_transparent,
    })
}

fn parse_command_group_picture(
    reference: Option<String>,
    load_transparent: Option<String>,
    source: Option<&MetadataSourceContext>,
) -> Result<CommandGroupPicture> {
    let Some(reference) = reference.map(|value| value.trim().to_string()) else {
        return Ok(CommandGroupPicture::Empty);
    };
    if reference.is_empty() {
        return Ok(CommandGroupPicture::Empty);
    }
    if reference == "StdPicture.Print" {
        return Ok(CommandGroupPicture::StdPicturePrint);
    }
    if reference == "StdPicture.InformationRegister" {
        let load_transparent = parse_required_metadata_bool(
            "CommandGroup",
            "Picture/LoadTransparent",
            load_transparent,
        )?;
        return Ok(CommandGroupPicture::CommonPicture {
            uuid: STD_PICTURE_INFORMATION_REGISTER_UUID.to_string(),
            load_transparent,
        });
    }
    if !reference.starts_with("CommonPicture.") {
        return Err(anyhow!(
            "unsupported CommandGroup Picture reference: {reference}"
        ));
    }
    let source = source.ok_or_else(|| {
        anyhow!(
            "CommandGroup Picture {reference} requires --source-root to resolve CommonPicture UUID"
        )
    })?;
    let uuid = source.resolve_common_picture_uuid(&reference)?;
    let load_transparent =
        parse_required_metadata_bool("CommandGroup", "Picture/LoadTransparent", load_transparent)?;
    Ok(CommandGroupPicture::CommonPicture {
        uuid,
        load_transparent,
    })
}

fn parse_common_command_group_reference(
    value: Option<String>,
    source: Option<&MetadataSourceContext>,
) -> Result<CommonCommandGroupReference> {
    let value = value.ok_or_else(|| anyhow!("CommonCommand Properties/Group not found in XML"))?;
    let reference = value.trim();
    if reference.is_empty() {
        return Err(anyhow!("CommonCommand Properties/Group is empty"));
    }
    if reference.starts_with("CommandGroup.") {
        let source = source.ok_or_else(|| {
            anyhow!(
                "CommonCommand Group {reference} requires --source-root to resolve CommandGroup UUID"
            )
        })?;
        let uuid = source.resolve_command_group_uuid(reference)?;
        return Ok(CommonCommandGroupReference::CommandGroup { uuid });
    }
    let uuid = common_command_group_uuid(reference)
        .ok_or_else(|| anyhow!("unsupported CommonCommand Group: {reference}"))?;
    Ok(CommonCommandGroupReference::BuiltIn { uuid })
}

fn parse_common_command_parameter_type(
    types: &[String],
    source: Option<&MetadataSourceContext>,
) -> Result<CommonCommandParameterType> {
    if types.is_empty() {
        return Ok(CommonCommandParameterType::Empty);
    }
    if types.len() != 1 {
        return Err(anyhow!(
            "only single CommonCommand CommandParameterType TypeSet is supported, got {}",
            types.len()
        ));
    }

    let reference = types[0].trim();
    if !is_defined_type_reference(reference) {
        return Err(anyhow!(
            "unsupported CommonCommand CommandParameterType TypeSet: {reference}"
        ));
    }
    let source = source.ok_or_else(|| {
        anyhow!(
            "CommonCommand CommandParameterType {reference} requires --source-root to resolve DefinedType TypeId"
        )
    })?;
    let type_id = source.resolve_defined_type_type_id(reference)?;
    Ok(CommonCommandParameterType::DefinedType { type_id })
}

fn parse_common_command_representation(
    value: Option<String>,
) -> Result<CommonCommandRepresentation> {
    let value =
        value.ok_or_else(|| anyhow!("CommonCommand Properties/Representation not found in XML"))?;
    match value.trim() {
        "Text" => Ok(CommonCommandRepresentation::Text),
        "Auto" => Ok(CommonCommandRepresentation::Auto),
        "Picture" => Ok(CommonCommandRepresentation::Picture),
        "PictureAndText" => Ok(CommonCommandRepresentation::PictureAndText),
        other => Err(anyhow!("unsupported CommonCommand Representation: {other}")),
    }
}

fn parse_common_command_parameter_use_mode(
    value: Option<String>,
) -> Result<CommonCommandParameterUseMode> {
    let value = value
        .ok_or_else(|| anyhow!("CommonCommand Properties/ParameterUseMode not found in XML"))?;
    match value.trim() {
        "Single" => Ok(CommonCommandParameterUseMode::Single),
        "Multiple" => Ok(CommonCommandParameterUseMode::Multiple),
        other => Err(anyhow!(
            "unsupported CommonCommand ParameterUseMode: {other}"
        )),
    }
}

fn parse_common_command_on_main_server_unavailable_behavior(
    value: Option<String>,
) -> Result<CommonCommandOnMainServerUnavailableBehavior> {
    let value = value.ok_or_else(|| {
        anyhow!("CommonCommand Properties/OnMainServerUnavalableBehavior not found in XML")
    })?;
    match value.trim() {
        "Auto" => Ok(CommonCommandOnMainServerUnavailableBehavior::Auto),
        other => Err(anyhow!(
            "unsupported CommonCommand OnMainServerUnavalableBehavior: {other}"
        )),
    }
}

fn parse_command_group_category(value: Option<String>) -> Result<CommandGroupCategory> {
    let value =
        value.ok_or_else(|| anyhow!("CommandGroup Properties/Category not found in XML"))?;
    match value.trim() {
        "NavigationPanel" => Ok(CommandGroupCategory::NavigationPanel),
        "FormNavigationPanel" => Ok(CommandGroupCategory::FormNavigationPanel),
        "ActionsPanel" => Ok(CommandGroupCategory::ActionsPanel),
        "FormCommandBar" => Ok(CommandGroupCategory::FormCommandBar),
        other => Err(anyhow!("unsupported CommandGroup Category: {other}")),
    }
}

fn common_command_representation_code(value: CommonCommandRepresentation) -> u8 {
    match value {
        CommonCommandRepresentation::Text => 0,
        CommonCommandRepresentation::Picture => 1,
        CommonCommandRepresentation::PictureAndText => 2,
        CommonCommandRepresentation::Auto => 3,
    }
}

fn common_command_parameter_use_mode_code(value: CommonCommandParameterUseMode) -> u8 {
    match value {
        CommonCommandParameterUseMode::Single => 0,
        CommonCommandParameterUseMode::Multiple => 1,
    }
}

fn common_command_on_main_server_unavailable_behavior_code(
    value: CommonCommandOnMainServerUnavailableBehavior,
) -> u8 {
    match value {
        CommonCommandOnMainServerUnavailableBehavior::Auto => 0,
    }
}

fn command_group_category_code(value: CommandGroupCategory) -> u8 {
    match value {
        CommandGroupCategory::NavigationPanel => 1,
        CommandGroupCategory::FormNavigationPanel => 2,
        CommandGroupCategory::ActionsPanel => 4,
        CommandGroupCategory::FormCommandBar => 8,
    }
}

fn common_command_group_uuid(reference: &str) -> Option<String> {
    match reference.trim() {
        "NavigationPanelOrdinary" => Some("77ea1b8f-dd79-4717-9dba-5628e7f348cf".to_string()),
        "NavigationPanelSeeAlso" => Some("bc80566a-86a5-4e87-acd4-872239385a2e".to_string()),
        "ActionsPanelCreate" => Some("4f499c31-050b-47c5-aa84-d0366c0a0da8".to_string()),
        "ActionsPanelReports" => Some("5b360bff-01a1-49b6-93d2-26e7e8e3a038".to_string()),
        "ActionsPanelTools" => Some("aabb34e1-98c1-4bd0-bf7f-243f95437b44".to_string()),
        "FormCommandBarCreateBasedOn" => Some("dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0".to_string()),
        "FormCommandBarImportant" => Some("cb50f5c0-8013-4262-93a2-f0db379d6b6b".to_string()),
        "FormNavigationPanelGoTo" => Some("eacad741-96b9-4b3a-bf79-dde9ecead1a1".to_string()),
        "FormNavigationPanelImportant" => Some("dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2".to_string()),
        _ => None,
    }
}

fn parse_optional_u32(name: &str, value: Option<&str>) -> Result<Option<u32>> {
    value
        .map(|value| {
            value
                .trim()
                .parse::<u32>()
                .with_context(|| format!("invalid metadata {name}: {value}"))
        })
        .transpose()
}

fn parse_required_u32(name: &str, value: Option<&str>) -> Result<u32> {
    value
        .ok_or_else(|| anyhow!("metadata {name} not found in XML"))?
        .trim()
        .parse::<u32>()
        .with_context(|| format!("invalid metadata {name}"))
}

fn parse_string_allowed_length_flag(value: Option<&str>) -> Result<u8> {
    match value.map(str::trim).unwrap_or("Variable") {
        "Fixed" => Ok(0),
        "Variable" => Ok(1),
        other => Err(anyhow!(
            "unsupported metadata StringQualifiers/AllowedLength: {other}"
        )),
    }
}

fn parse_number_allowed_sign_flag(value: Option<&str>) -> Result<u8> {
    match value.map(str::trim).unwrap_or("Any") {
        "Any" => Ok(0),
        "Nonnegative" => Ok(1),
        other => Err(anyhow!(
            "unsupported metadata NumberQualifiers/AllowedSign: {other}"
        )),
    }
}

fn format_constant_type_pattern(value_type: &MetadataTypePatternElement) -> String {
    format_metadata_type_pattern(std::slice::from_ref(value_type))
}

fn format_metadata_type_pattern(value_types: &[MetadataTypePatternElement]) -> String {
    let mut output = r#"{"Pattern""#.to_string();
    for value_type in value_types {
        output.push(',');
        output.push_str(&format_metadata_type_pattern_element(value_type));
    }
    output.push('}');
    output
}

fn format_metadata_type_pattern_element(value_type: &MetadataTypePatternElement) -> String {
    match value_type {
        MetadataTypePatternElement::Boolean => r#"{"B"}"#.to_string(),
        MetadataTypePatternElement::String {
            length: Some(length),
            allowed_length_flag,
        } => format!(r#"{{"S",{length},{allowed_length_flag}}}"#),
        MetadataTypePatternElement::String { length: None, .. } => r#"{"S"}"#.to_string(),
        MetadataTypePatternElement::Number {
            digits,
            fraction_digits,
            allowed_sign_flag,
        } => format!(r#"{{"N",{digits},{fraction_digits},{allowed_sign_flag}}}"#),
        MetadataTypePatternElement::DateTime => r#"{"D"}"#.to_string(),
        MetadataTypePatternElement::Reference { type_id } => format!("{{\"#\",{type_id}}}"),
    }
}

fn format_common_command_picture(picture: &CommonCommandPicture) -> String {
    match picture {
        CommonCommandPicture::Empty => r#"{4,0,{0},"",-1,-1,1,0,""}"#.to_string(),
        CommonCommandPicture::CommonPicture {
            uuid,
            load_transparent,
        } => format!(
            r#"{{4,1,{{0,{uuid}}},"",-1,-1,{},0,""}}"#,
            bool_flag(*load_transparent)
        ),
    }
}

fn format_command_group_picture(picture: &CommandGroupPicture) -> String {
    match picture {
        CommandGroupPicture::Empty => r#"{4,0,{0},"",-1,-1,1,0,""}"#.to_string(),
        CommandGroupPicture::StdPicturePrint => r#"{4,1,{-13},"",-1,-1,1,0,""}"#.to_string(),
        CommandGroupPicture::CommonPicture {
            uuid,
            load_transparent,
        } => format!(
            r#"{{4,1,{{0,{uuid}}},"",-1,-1,{},0,""}}"#,
            bool_flag(*load_transparent)
        ),
    }
}

fn format_common_command_parameter_type(parameter_type: &CommonCommandParameterType) -> String {
    match parameter_type {
        CommonCommandParameterType::Empty => r#"{"Pattern"}"#.to_string(),
        CommonCommandParameterType::DefinedType { type_id } => {
            format!("{{\"Pattern\",{{\"#\",{type_id}}}}}")
        }
    }
}

fn format_common_command_group_reference(group: &CommonCommandGroupReference) -> String {
    match group {
        CommonCommandGroupReference::BuiltIn { uuid }
        | CommonCommandGroupReference::CommandGroup { uuid } => format!("{{1,{uuid}}}"),
    }
}

fn bool_flag(value: bool) -> String {
    if value { "1" } else { "0" }.to_string()
}

fn return_values_reuse_flag(value: ReturnValuesReuse) -> String {
    match value {
        ReturnValuesReuse::DontUse => "0",
        ReturnValuesReuse::DuringRequest => "1",
        ReturnValuesReuse::DuringSession => "2",
    }
    .to_string()
}

fn append_common_module_xml_text(
    path: &[String],
    value: &str,
    name: &mut Option<String>,
    comment: &mut Option<String>,
    pending_synonym_lang: &mut Option<String>,
    pending_synonym_content: &mut Option<String>,
    global: &mut Option<String>,
    client_managed_application: &mut Option<String>,
    server: &mut Option<String>,
    external_connection: &mut Option<String>,
    client_ordinary_application: &mut Option<String>,
    server_call: &mut Option<String>,
    privileged: &mut Option<String>,
    return_values_reuse: &mut Option<String>,
) {
    if path_ends_with(path, &["CommonModule", "Properties", "Name"]) {
        name.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(path, &["CommonModule", "Properties", "Comment"]) {
        comment.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(
        path,
        &["CommonModule", "Properties", "Synonym", "item", "lang"],
    ) {
        pending_synonym_lang
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &["CommonModule", "Properties", "Synonym", "item", "content"],
    ) {
        pending_synonym_content
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &["CommonModule", "Properties", "Global"]) {
        global.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(
        path,
        &["CommonModule", "Properties", "ClientManagedApplication"],
    ) {
        client_managed_application
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &["CommonModule", "Properties", "Server"]) {
        server.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(path, &["CommonModule", "Properties", "ExternalConnection"]) {
        external_connection
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &["CommonModule", "Properties", "ClientOrdinaryApplication"],
    ) {
        client_ordinary_application
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &["CommonModule", "Properties", "ServerCall"]) {
        server_call.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(path, &["CommonModule", "Properties", "Privileged"]) {
        privileged.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(path, &["CommonModule", "Properties", "ReturnValuesReuse"]) {
        return_values_reuse
            .get_or_insert_with(String::new)
            .push_str(value);
    }
}

fn append_simple_metadata_xml_text(
    path: &[String],
    kind: Option<&str>,
    value: &str,
    name: &mut Option<String>,
    comment: &mut Option<String>,
    pending_synonym_lang: &mut Option<String>,
    pending_synonym_content: &mut Option<String>,
) {
    let Some(kind) = kind else {
        return;
    };
    if path_ends_with(path, &[kind, "Properties", "Name"]) {
        name.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(path, &[kind, "Properties", "Comment"]) {
        comment.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(path, &[kind, "Properties", "Synonym", "item", "lang"]) {
        pending_synonym_lang
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &[kind, "Properties", "Synonym", "item", "content"]) {
        pending_synonym_content
            .get_or_insert_with(String::new)
            .push_str(value);
    }
}

fn append_constant_xml_text(
    path: &[String],
    value: &str,
    types: &mut Vec<String>,
    string_length: &mut Option<String>,
    string_allowed_length: &mut Option<String>,
    number_digits: &mut Option<String>,
    number_fraction_digits: &mut Option<String>,
    number_allowed_sign: &mut Option<String>,
    use_standard_commands: &mut Option<String>,
) {
    append_metadata_type_xml_text(
        "Constant",
        path,
        value,
        types,
        string_length,
        string_allowed_length,
        number_digits,
        number_fraction_digits,
        number_allowed_sign,
    );

    if path_ends_with(path, &["Constant", "Properties", "UseStandardCommands"]) {
        use_standard_commands
            .get_or_insert_with(String::new)
            .push_str(value);
    }
}

fn append_metadata_type_xml_text(
    kind: &str,
    path: &[String],
    value: &str,
    types: &mut Vec<String>,
    string_length: &mut Option<String>,
    string_allowed_length: &mut Option<String>,
    number_digits: &mut Option<String>,
    number_fraction_digits: &mut Option<String>,
    number_allowed_sign: &mut Option<String>,
) {
    if path_ends_with(path, &[kind, "Properties", "Type", "Type"]) {
        types.push(value.to_string());
    } else if path_ends_with(
        path,
        &[kind, "Properties", "Type", "StringQualifiers", "Length"],
    ) {
        string_length
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &[
            kind,
            "Properties",
            "Type",
            "StringQualifiers",
            "AllowedLength",
        ],
    ) {
        string_allowed_length
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &[kind, "Properties", "Type", "NumberQualifiers", "Digits"],
    ) {
        number_digits
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &[
            kind,
            "Properties",
            "Type",
            "NumberQualifiers",
            "FractionDigits",
        ],
    ) {
        number_fraction_digits
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &[
            kind,
            "Properties",
            "Type",
            "NumberQualifiers",
            "AllowedSign",
        ],
    ) {
        number_allowed_sign
            .get_or_insert_with(String::new)
            .push_str(value);
    }
}

fn append_common_command_xml_text(
    path: &[String],
    value: &str,
    representation: &mut Option<String>,
    picture_ref: &mut Option<String>,
    picture_load_transparent: &mut Option<String>,
    pending_tooltip_lang: &mut Option<String>,
    pending_tooltip_content: &mut Option<String>,
    include_help_in_contents: &mut Option<String>,
    group: &mut Option<String>,
    command_parameter_types: &mut Vec<String>,
    parameter_use_mode: &mut Option<String>,
    modifies_data: &mut Option<String>,
    on_main_server_unavailable_behavior: &mut Option<String>,
) {
    if path_ends_with(path, &["CommonCommand", "Properties", "Representation"]) {
        representation
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &["CommonCommand", "Properties", "Picture", "Ref"]) {
        picture_ref.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(
        path,
        &["CommonCommand", "Properties", "Picture", "LoadTransparent"],
    ) {
        picture_load_transparent
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &["CommonCommand", "Properties", "ToolTip", "item", "lang"],
    ) {
        pending_tooltip_lang
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &["CommonCommand", "Properties", "ToolTip", "item", "content"],
    ) {
        pending_tooltip_content
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &["CommonCommand", "Properties", "IncludeHelpInContents"],
    ) {
        include_help_in_contents
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &["CommonCommand", "Properties", "Group"]) {
        group.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(
        path,
        &[
            "CommonCommand",
            "Properties",
            "CommandParameterType",
            "TypeSet",
        ],
    ) {
        command_parameter_types.push(value.to_string());
    } else if path_ends_with(path, &["CommonCommand", "Properties", "ParameterUseMode"]) {
        parameter_use_mode
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &["CommonCommand", "Properties", "ModifiesData"]) {
        modifies_data
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &[
            "CommonCommand",
            "Properties",
            "OnMainServerUnavalableBehavior",
        ],
    ) {
        on_main_server_unavailable_behavior
            .get_or_insert_with(String::new)
            .push_str(value);
    }
}

fn append_command_group_xml_text(
    path: &[String],
    value: &str,
    representation: &mut Option<String>,
    picture_ref: &mut Option<String>,
    picture_load_transparent: &mut Option<String>,
    pending_tooltip_lang: &mut Option<String>,
    pending_tooltip_content: &mut Option<String>,
    category: &mut Option<String>,
) {
    if path_ends_with(path, &["CommandGroup", "Properties", "Representation"]) {
        representation
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &["CommandGroup", "Properties", "Picture", "Ref"]) {
        picture_ref.get_or_insert_with(String::new).push_str(value);
    } else if path_ends_with(
        path,
        &["CommandGroup", "Properties", "Picture", "LoadTransparent"],
    ) {
        picture_load_transparent
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &["CommandGroup", "Properties", "ToolTip", "item", "lang"],
    ) {
        pending_tooltip_lang
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(
        path,
        &["CommandGroup", "Properties", "ToolTip", "item", "content"],
    ) {
        pending_tooltip_content
            .get_or_insert_with(String::new)
            .push_str(value);
    } else if path_ends_with(path, &["CommandGroup", "Properties", "Category"]) {
        category.get_or_insert_with(String::new).push_str(value);
    }
}

fn is_simple_metadata_properties_path(path: &[String], kind: Option<&str>) -> bool {
    kind.is_some_and(|kind| path_ends_with(path, &[kind, "Properties"]))
}

fn is_simple_metadata_synonym_path(path: &[String], kind: Option<&str>) -> bool {
    kind.is_some_and(|kind| path_ends_with(path, &[kind, "Properties", "Synonym"]))
}

fn is_simple_metadata_synonym_item_path(path: &[String], kind: Option<&str>) -> bool {
    kind.is_some_and(|kind| path_ends_with(path, &[kind, "Properties", "Synonym", "item"]))
}

fn replace_1c_quoted_string(text: &mut String, start: usize, value: &str) -> Result<usize> {
    let end = scan_1c_quoted_string_end(text, start)?;
    let replacement = format_1c_string(value);
    text.replace_range(start..end, &replacement);
    Ok(start + replacement.len())
}

fn scan_1c_quoted_string_end(text: &str, start: usize) -> Result<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return Err(anyhow!("expected 1C string at byte {start}"));
    }
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            if bytes.get(index + 1) == Some(&b'"') {
                index += 2;
            } else {
                return Ok(index + 1);
            }
        } else {
            index += 1;
        }
    }
    Err(anyhow!("unterminated 1C string at byte {start}"))
}

fn parse_1c_quoted_string(text: &str) -> Result<String> {
    let text = text.trim();
    let end = scan_1c_quoted_string_end(text, 0)?;
    if end != text.len() {
        return Err(anyhow!("unexpected trailing data after 1C string"));
    }
    Ok(text[1..end - 1].replace("\"\"", "\""))
}

fn scan_balanced_braces(text: &str, start: usize) -> Result<usize> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return Err(anyhow!("expected 1C braced expression at byte {start}"));
    }

    let mut index = start;
    let mut depth = 0usize;
    let mut in_string = false;
    while index < bytes.len() {
        match bytes[index] {
            b'"' if in_string && bytes.get(index + 1) == Some(&b'"') => {
                index += 2;
            }
            b'"' => {
                in_string = !in_string;
                index += 1;
            }
            b'{' if !in_string => {
                depth += 1;
                index += 1;
            }
            b'}' if !in_string => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| anyhow!("unbalanced 1C braced expression"))?;
                index += 1;
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {
                index += 1;
            }
        }
    }
    Err(anyhow!("unterminated 1C braced expression at byte {start}"))
}

fn scan_braced_fields(text: &str, start: usize) -> Result<Vec<Range<usize>>> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return Err(anyhow!("expected 1C braced expression at byte {start}"));
    }

    let mut fields = Vec::new();
    let mut field_start = start + 1;
    let mut index = start + 1;
    let mut depth = 0usize;
    let mut in_string = false;

    while index < bytes.len() {
        match bytes[index] {
            b'"' if in_string && bytes.get(index + 1) == Some(&b'"') => {
                index += 2;
            }
            b'"' => {
                in_string = !in_string;
                index += 1;
            }
            b'{' if !in_string => {
                depth += 1;
                index += 1;
            }
            b'}' if !in_string && depth > 0 => {
                depth -= 1;
                index += 1;
            }
            b'}' if !in_string => {
                fields.push(trim_ascii_ws_range(text, field_start..index));
                return Ok(fields);
            }
            b',' if !in_string && depth == 0 => {
                fields.push(trim_ascii_ws_range(text, field_start..index));
                field_start = index + 1;
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    Err(anyhow!("unterminated 1C braced expression at byte {start}"))
}

fn trim_ascii_ws_range(text: &str, range: Range<usize>) -> Range<usize> {
    let bytes = text.as_bytes();
    let mut start = range.start;
    let mut end = range.end;
    while start < end
        && bytes
            .get(start)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        start += 1;
    }
    while end > start
        && bytes
            .get(end - 1)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        end -= 1;
    }
    start..end
}

fn skip_ascii_ws(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let mut index = start;
    while bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        index += 1;
    }
    index
}

fn expect_byte_after_ws(text: &str, start: usize, expected: u8) -> Result<usize> {
    let index = skip_ascii_ws(text, start);
    if text.as_bytes().get(index) == Some(&expected) {
        Ok(index + 1)
    } else {
        Err(anyhow!(
            "expected byte {} at byte {}",
            expected as char,
            index
        ))
    }
}

fn format_1c_synonyms(synonyms: &[LocalizedString]) -> String {
    let mut output = format!("{{{}", synonyms.len());
    for synonym in synonyms {
        output.push(',');
        output.push_str(&format_1c_string(&synonym.lang));
        output.push(',');
        output.push_str(&format_1c_string(&synonym.content));
    }
    output.push('}');
    output
}

fn format_1c_string(value: &str) -> String {
    let escaped = value.replace('"', "\"\"");
    format!("\"{escaped}\"")
}

fn is_defined_type_reference(reference: &str) -> bool {
    let reference = reference.trim();
    reference.starts_with("cfg:DefinedType.") || reference.starts_with("DefinedType.")
}

fn defined_type_name_from_reference(reference: &str) -> Result<&str> {
    let reference = reference.trim();
    if let Some(name) = reference.strip_prefix("cfg:DefinedType.") {
        Ok(name)
    } else if let Some(name) = reference.strip_prefix("DefinedType.") {
        Ok(name)
    } else {
        Err(anyhow!("unsupported DefinedType reference: {reference}"))
    }
}

fn metadata_type_source_folder(generated_type_name: &str) -> Option<&'static str> {
    let prefix = generated_type_name.split_once('.')?.0;
    match prefix {
        "BusinessProcessObject"
        | "BusinessProcessRef"
        | "BusinessProcessSelection"
        | "BusinessProcessList"
        | "BusinessProcessManager"
        | "BusinessProcessRoutePointRef" => Some("BusinessProcesses"),
        "AccumulationRegisterObject"
        | "AccumulationRegisterRecordSet"
        | "AccumulationRegisterRecordKey"
        | "AccumulationRegisterSelection"
        | "AccumulationRegisterList"
        | "AccumulationRegisterManager" => Some("AccumulationRegisters"),
        "AccountingRegisterObject"
        | "AccountingRegisterRecordSet"
        | "AccountingRegisterRecordKey"
        | "AccountingRegisterSelection"
        | "AccountingRegisterList"
        | "AccountingRegisterManager" => Some("AccountingRegisters"),
        "CatalogObject" | "CatalogRef" | "CatalogSelection" | "CatalogList" | "CatalogManager" => {
            Some("Catalogs")
        }
        "CatalogTabularSection" | "CatalogTabularSectionRow" => Some("Catalogs"),
        "CommandGroup" => Some("CommandGroups"),
        "CommonCommand" => Some("CommonCommands"),
        "CommonPicture" => Some("CommonPictures"),
        "CommonForm" => Some("CommonForms"),
        "ChartOfCharacteristicTypesObject"
        | "ChartOfCharacteristicTypesRef"
        | "ChartOfCharacteristicTypesSelection"
        | "ChartOfCharacteristicTypesList"
        | "ChartOfCharacteristicTypesManager"
        | "ChartOfCharacteristicTypesTabularSection"
        | "ChartOfCharacteristicTypesTabularSectionRow"
        | "Characteristic" => Some("ChartsOfCharacteristicTypes"),
        "ChartOfAccountsObject"
        | "ChartOfAccountsRef"
        | "ChartOfAccountsSelection"
        | "ChartOfAccountsList"
        | "ChartOfAccountsManager" => Some("ChartsOfAccounts"),
        "ChartOfCalculationTypesObject"
        | "ChartOfCalculationTypesRef"
        | "ChartOfCalculationTypesSelection"
        | "ChartOfCalculationTypesList"
        | "ChartOfCalculationTypesManager" => Some("ChartsOfCalculationTypes"),
        "ChartOfCalculationRegistersObject"
        | "ChartOfCalculationRegistersRef"
        | "ChartOfCalculationRegistersSelection"
        | "ChartOfCalculationRegistersList"
        | "ChartOfCalculationRegistersManager" => Some("ChartsOfCalculationRegisters"),
        "CalculationRegisterObject"
        | "CalculationRegisterRecordSet"
        | "CalculationRegisterRecordKey"
        | "CalculationRegisterSelection"
        | "CalculationRegisterList"
        | "CalculationRegisterManager" => Some("CalculationRegisters"),
        "DataProcessorObject"
        | "DataProcessorManager"
        | "DataProcessorTabularSection"
        | "DataProcessorTabularSectionRow" => Some("DataProcessors"),
        "DefinedType" => Some("DefinedTypes"),
        "DocumentObject" | "DocumentRef" | "DocumentSelection" | "DocumentList"
        | "DocumentManager" => Some("Documents"),
        "DocumentTabularSection" | "DocumentTabularSectionRow" => Some("Documents"),
        "DocumentJournalSelection" | "DocumentJournalList" | "DocumentJournalManager" => {
            Some("DocumentJournals")
        }
        "EnumRef" | "EnumList" | "EnumManager" => Some("Enums"),
        "ExchangePlanObject"
        | "ExchangePlanRef"
        | "ExchangePlanSelection"
        | "ExchangePlanList"
        | "ExchangePlanManager" => Some("ExchangePlans"),
        "InformationRegisterRecordSet"
        | "InformationRegisterRecordKey"
        | "InformationRegisterSelection"
        | "InformationRegisterList"
        | "InformationRegisterManager"
        | "InformationRegisterRecord"
        | "InformationRegisterRecordManager" => Some("InformationRegisters"),
        "FilterCriterionList" | "FilterCriterionManager" => Some("FilterCriteria"),
        "ConstantManager" | "ConstantValueManager" | "ConstantValueKey" => Some("Constants"),
        "CommonTemplate" => Some("CommonTemplates"),
        "SettingsStorageManager" => Some("SettingsStorages"),
        "ReportObject" | "ReportManager" => Some("Reports"),
        "TaskObject" | "TaskRef" | "TaskSelection" | "TaskList" | "TaskManager" => Some("Tasks"),
        _ => None,
    }
}

fn metadata_reference_source_folder(reference: &str) -> Option<(&'static str, &'static str)> {
    let prefix = reference.split_once('.')?.0;
    match prefix {
        "AccumulationRegister" => Some(("AccumulationRegister", "AccumulationRegisters")),
        "AccountingRegister" => Some(("AccountingRegister", "AccountingRegisters")),
        "BusinessProcess" => Some(("BusinessProcess", "BusinessProcesses")),
        "ChartOfCharacteristicTypes" => {
            Some(("ChartOfCharacteristicTypes", "ChartsOfCharacteristicTypes"))
        }
        "Catalog" => Some(("Catalog", "Catalogs")),
        "CommonAttribute" => Some(("CommonAttribute", "CommonAttributes")),
        "CommonForm" => Some(("CommonForm", "CommonForms")),
        "CalculationRegister" => Some(("CalculationRegister", "CalculationRegisters")),
        "ChartOfAccounts" => Some(("ChartOfAccounts", "ChartsOfAccounts")),
        "ChartOfCalculationTypes" => Some(("ChartOfCalculationTypes", "ChartsOfCalculationTypes")),
        "ChartOfCalculationRegisters" => Some((
            "ChartOfCalculationRegisters",
            "ChartsOfCalculationRegisters",
        )),
        "CommonCommand" => Some(("CommonCommand", "CommonCommands")),
        "CommonPicture" => Some(("CommonPicture", "CommonPictures")),
        "CommonTemplate" => Some(("CommonTemplate", "CommonTemplates")),
        "Constant" => Some(("Constant", "Constants")),
        "DefinedType" => Some(("DefinedType", "DefinedTypes")),
        "DataProcessor" => Some(("DataProcessor", "DataProcessors")),
        "Document" => Some(("Document", "Documents")),
        "DocumentJournal" => Some(("DocumentJournal", "DocumentJournals")),
        "CommandGroup" => Some(("CommandGroup", "CommandGroups")),
        "EventSubscription" => Some(("EventSubscription", "EventSubscriptions")),
        "FilterCriterion" => Some(("FilterCriterion", "FilterCriteria")),
        "FunctionalOption" => Some(("FunctionalOption", "FunctionalOptions")),
        "FunctionalOptionsParameter" => {
            Some(("FunctionalOptionsParameter", "FunctionalOptionsParameters"))
        }
        "HTTPService" => Some(("HTTPService", "HTTPServices")),
        "Language" => Some(("Language", "Languages")),
        "InformationRegister" => Some(("InformationRegister", "InformationRegisters")),
        "ExchangePlan" => Some(("ExchangePlan", "ExchangePlans")),
        "Role" => Some(("Role", "Roles")),
        "ScheduledJob" => Some(("ScheduledJob", "ScheduledJobs")),
        "SettingsStorage" => Some(("SettingsStorage", "SettingsStorages")),
        "SessionParameter" => Some(("SessionParameter", "SessionParameters")),
        "Report" => Some(("Report", "Reports")),
        "StyleItem" => Some(("StyleItem", "StyleItems")),
        "Subsystem" => Some(("Subsystem", "Subsystems")),
        "Task" => Some(("Task", "Tasks")),
        "WebService" => Some(("WebService", "WebServices")),
        "XDTOPackage" => Some(("XDTOPackage", "XDTOPackages")),
        "Enum" => Some(("Enum", "Enums")),
        _ => None,
    }
}

fn parse_defined_type_type_id(xml: &[u8], expected_name: &str) -> Result<String> {
    let expected_generated_name = format!("DefinedType.{expected_name}");
    parse_generated_type_type_id(xml, &expected_generated_name)
}

fn parse_generated_type_type_id(xml: &[u8], expected_generated_name: &str) -> Result<String> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut generated_type_depth = None::<usize>;
    let mut type_id = None::<String>;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                let is_matching_generated_type = local == "GeneratedType"
                    && xml_attr_value(&event, "name").as_deref() == Some(expected_generated_name);
                path.push(local);
                if is_matching_generated_type {
                    generated_type_depth = Some(path.len());
                }
            }
            Ok(Event::Text(text)) => {
                if generated_type_depth.is_some()
                    && path_ends_with(&path, &["GeneratedType", "TypeId"])
                {
                    let value = text.xml_content()?;
                    let value = unescape(value.as_ref())?;
                    type_id
                        .get_or_insert_with(String::new)
                        .push_str(value.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if generated_type_depth.is_some()
                    && path_ends_with(&path, &["GeneratedType", "TypeId"])
                {
                    type_id
                        .get_or_insert_with(String::new)
                        .push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "GeneratedType" && generated_type_depth == Some(path.len()) {
                    generated_type_depth = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    let type_id = type_id.ok_or_else(|| {
        anyhow!("GeneratedType {expected_generated_name} TypeId not found in XML")
    })?;
    normalize_uuid_text(&type_id)
}

fn normalize_uuid_text(value: &str) -> Result<String> {
    Ok(Uuid::parse_str(value.trim())?.hyphenated().to_string())
}

fn path_ends_with(path: &[String], suffix: &[&str]) -> bool {
    if path.len() < suffix.len() {
        return false;
    }
    path[path.len() - suffix.len()..]
        .iter()
        .zip(suffix)
        .all(|(left, right)| left == right)
}

fn xml_attr_value(event: &BytesStart<'_>, name: &str) -> Option<String> {
    event
        .attributes()
        .filter_map(Result::ok)
        .find(|attr| attr.key.as_ref() == name.as_bytes())
        .map(|attr| String::from_utf8_lossy(attr.value.as_ref()).to_string())
}

fn xml_local_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).to_string()
}

fn read_base_elements_from_blob(blob: &[u8]) -> Result<BTreeMap<String, ParsedElement>> {
    let inner = inflate_raw(blob).context("failed to inflate base blob")?;
    let elements = parse_v8_container(&inner).context("failed to parse base blob")?;
    Ok(elements
        .into_iter()
        .map(|element| (element.name.clone(), element))
        .collect())
}

fn build_module_inner(elements: &[ModuleElement; 2]) -> Result<Vec<u8>> {
    let toc_len = elements.len() * ELEM_ADDR_SIZE;
    let toc_block_total = BLOCK_HEADER_SIZE + toc_len;
    let mut offset = FILE_HEADER_SIZE + toc_block_total;

    let mut addresses = Vec::with_capacity(elements.len());
    for element in elements {
        let header_addr = offset;
        offset += BLOCK_HEADER_SIZE + element.header.len();

        let data_addr = offset;
        let data_page = page_size_for_data(element.data.len());
        offset += BLOCK_HEADER_SIZE + data_page;

        addresses.push((header_addr, data_addr));
    }

    let mut bytes = Vec::with_capacity(offset);
    write_u32(&mut bytes, V8_MAGIC_NUMBER);
    write_u32(&mut bytes, V8_PAGE_SIZE);
    write_u32(&mut bytes, 1);
    write_u32(&mut bytes, 0);

    let mut toc = Vec::with_capacity(toc_len);
    for (header_addr, data_addr) in addresses {
        write_u32(&mut toc, checked_u32(header_addr, "header address")?);
        write_u32(&mut toc, checked_u32(data_addr, "data address")?);
        write_u32(&mut toc, V8_MAGIC_NUMBER);
    }
    write_block(&mut bytes, &toc, toc.len())?;

    for element in elements {
        write_block(&mut bytes, &element.header, element.header.len())?;
        write_block(
            &mut bytes,
            &element.data,
            page_size_for_data(element.data.len()),
        )?;
    }

    Ok(bytes)
}

fn write_block(target: &mut Vec<u8>, data: &[u8], page_size: usize) -> Result<()> {
    if page_size < data.len() {
        return Err(anyhow!(
            "page size {} is less than data size {}",
            page_size,
            data.len()
        ));
    }
    let header = format!(
        "\r\n{:08x} {:08x} {:08x} \r\n",
        data.len(),
        page_size,
        V8_MAGIC_NUMBER
    );
    if header.len() != BLOCK_HEADER_SIZE {
        return Err(anyhow!("invalid block header length {}", header.len()));
    }
    target.extend_from_slice(header.as_bytes());
    target.extend_from_slice(data);
    target.resize(target.len() + (page_size - data.len()), 0);
    Ok(())
}

fn page_size_for_data(len: usize) -> usize {
    if len < V8_PAGE_SIZE as usize {
        V8_PAGE_SIZE as usize
    } else {
        len
    }
}

fn make_element_header(name: &str) -> Vec<u8> {
    let mut header = vec![0; ELEM_HEADER_PREFIX_SIZE];
    for unit in name.encode_utf16() {
        header.extend_from_slice(&unit.to_le_bytes());
    }
    header.extend_from_slice(&[0, 0, 0, 0]);
    header
}

fn parse_v8_container(bytes: &[u8]) -> Result<Vec<ParsedElement>> {
    if bytes.len() < FILE_HEADER_SIZE + BLOCK_HEADER_SIZE {
        return Err(anyhow!("container is too short"));
    }
    if read_u32(bytes, 0)? != V8_MAGIC_NUMBER {
        return Err(anyhow!("unexpected file header next page marker"));
    }
    if read_u32(bytes, 8)? != 1 {
        return Err(anyhow!("unsupported module container storage version"));
    }

    let toc_header = read_block_header(bytes, FILE_HEADER_SIZE)?;
    let toc_start = FILE_HEADER_SIZE + BLOCK_HEADER_SIZE;
    let toc_end = toc_start + toc_header.data_size;
    if toc_end > bytes.len() {
        return Err(anyhow!("TOC block exceeds container length"));
    }
    if toc_header.data_size % ELEM_ADDR_SIZE != 0 {
        return Err(anyhow!("TOC size is not divisible by element address size"));
    }

    let mut result = Vec::new();
    for entry in bytes[toc_start..toc_end].chunks_exact(ELEM_ADDR_SIZE) {
        let header_addr = read_u32(entry, 0)? as usize;
        let data_addr = read_u32(entry, 4)? as usize;
        let marker = read_u32(entry, 8)?;
        if marker != V8_MAGIC_NUMBER {
            continue;
        }
        let header = read_block_payload(bytes, header_addr)?;
        let data = read_block_payload(bytes, data_addr)?;
        let name = element_name(&header)?;
        result.push(ParsedElement { name, header, data });
    }
    Ok(result)
}

fn read_block_payload(bytes: &[u8], offset: usize) -> Result<Vec<u8>> {
    let header = read_block_header(bytes, offset)?;
    let start = offset + BLOCK_HEADER_SIZE;
    let data_end = start + header.data_size;
    let page_end = start + header.page_size;
    if data_end > bytes.len() || page_end > bytes.len() {
        return Err(anyhow!("block at {} exceeds container length", offset));
    }
    if header.next_page_addr != V8_MAGIC_NUMBER {
        return Err(anyhow!("multi-page V8 blocks are not supported yet"));
    }
    Ok(bytes[start..data_end].to_vec())
}

fn read_block_header(bytes: &[u8], offset: usize) -> Result<BlockHeader> {
    let end = offset + BLOCK_HEADER_SIZE;
    if end > bytes.len() {
        return Err(anyhow!("block header at {} exceeds input length", offset));
    }
    let raw = &bytes[offset..end];
    if raw[0] != b'\r'
        || raw[1] != b'\n'
        || raw[10] != b' '
        || raw[19] != b' '
        || raw[28] != b' '
        || raw[29] != b'\r'
        || raw[30] != b'\n'
    {
        return Err(anyhow!("invalid block header at {}", offset));
    }
    Ok(BlockHeader {
        data_size: parse_hex_u32(&raw[2..10])? as usize,
        page_size: parse_hex_u32(&raw[11..19])? as usize,
        next_page_addr: parse_hex_u32(&raw[20..28])?,
    })
}

fn element_name(header: &[u8]) -> Result<String> {
    if header.len() < ELEM_HEADER_PREFIX_SIZE {
        return Err(anyhow!("element header is too short"));
    }
    let raw = &header[ELEM_HEADER_PREFIX_SIZE..];
    let mut units = Vec::new();
    for pair in raw.chunks_exact(2) {
        let unit = u16::from_le_bytes([pair[0], pair[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    String::from_utf16(&units).context("element name is not valid UTF-16LE")
}

fn inflate_raw(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = DeflateDecoder::new(input);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output)?;
    Ok(output)
}

fn deflate_raw(input: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input)?;
    encoder.finish().context("failed to finish deflate stream")
}

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(ALPHABET[(first >> 2) as usize] as char);
        output.push(ALPHABET[(((first & 0b0000_0011) << 4) | (second >> 4)) as usize] as char);
        if chunk.len() > 1 {
            output.push(ALPHABET[(((second & 0b0000_1111) << 2) | (third >> 6)) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(ALPHABET[(third & 0b0011_1111) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn parse_hex_u32(bytes: &[u8]) -> Result<u32> {
    let text = std::str::from_utf8(bytes)?;
    Ok(u32::from_str_radix(text, 16)?)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset + 4;
    if end > bytes.len() {
        return Err(anyhow!("u32 at {} exceeds input length", offset));
    }
    Ok(u32::from_le_bytes(bytes[offset..end].try_into()?))
}

fn write_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn checked_u32(value: usize, name: &str) -> Result<u32> {
    value
        .try_into()
        .with_context(|| format!("{name} does not fit into u32: {value}"))
}

fn replace_header_uuid(text: &mut String) -> Result<VersionReplacement> {
    let marker = ",\"\",";
    let marker_start = text
        .find(marker)
        .ok_or_else(|| anyhow!("versions header marker not found"))?;
    let uuid_start = marker_start + marker.len();
    replace_uuid_at(text, uuid_start, "<generation>")
}

fn replace_named_uuid(text: &mut String, name: &str) -> Result<VersionReplacement> {
    let marker = format!("\"{name}\",");
    let marker_start = text
        .find(&marker)
        .ok_or_else(|| anyhow!("versions entry not found: {name}"))?;
    let uuid_start = marker_start + marker.len();
    replace_uuid_at(text, uuid_start, name)
}

fn replace_uuid_at(text: &mut String, uuid_start: usize, name: &str) -> Result<VersionReplacement> {
    let uuid_end = uuid_start + 36;
    let old_uuid = text
        .get(uuid_start..uuid_end)
        .ok_or_else(|| anyhow!("UUID for {name} exceeds text length"))?
        .to_string();
    if !is_uuid_text(&old_uuid) {
        return Err(anyhow!("invalid UUID for {name}: {old_uuid}"));
    }
    let new_uuid = Uuid::new_v4().hyphenated().to_string();
    text.replace_range(uuid_start..uuid_end, &new_uuid);
    Ok(VersionReplacement {
        name: name.to_string(),
        old_uuid,
        new_uuid,
    })
}

fn is_uuid_text(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            if *byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_hexdigit() {
            return false;
        }
    }
    true
}

pub fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:X}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        CommonCommandRepresentation, DEFAULT_INFO, MetadataSourceContext, build_module_inner,
        common_command_representation_code, deflate_raw, inflate_raw,
        parse_common_command_representation, parse_v8_container,
    };
    use crate::module_blob::ModuleElement;

    #[test]
    fn packs_module_inner_with_plain_info_and_text() {
        let inner = build_module_inner(&[
            ModuleElement {
                header: super::make_element_header("info"),
                data: DEFAULT_INFO.to_vec(),
            },
            ModuleElement {
                header: super::make_element_header("text"),
                data: b"\xEF\xBB\xBFProcedure Test()\r\nEndProcedure".to_vec(),
            },
        ])
        .unwrap();

        assert_eq!(&inner[0..4], &0x7fff_ffff_u32.to_le_bytes());
        assert_eq!(&inner[8..12], &1_u32.to_le_bytes());

        let elements = parse_v8_container(&inner).unwrap();
        assert_eq!(elements.len(), 2);
        assert_eq!(elements[0].name, "info");
        assert_eq!(elements[0].data, DEFAULT_INFO);
        assert_eq!(elements[1].name, "text");
        assert!(elements[1].data.ends_with(b"EndProcedure"));
    }

    #[test]
    fn module_outer_blob_is_raw_deflate() {
        let inner = build_module_inner(&[
            ModuleElement {
                header: super::make_element_header("info"),
                data: DEFAULT_INFO.to_vec(),
            },
            ModuleElement {
                header: super::make_element_header("text"),
                data: b"text".to_vec(),
            },
        ])
        .unwrap();
        let blob = deflate_raw(&inner).unwrap();
        assert_eq!(inflate_raw(&blob).unwrap(), inner);
    }

    #[test]
    fn unpacks_module_blob_text_element() {
        let text = b"Procedure Run()\r\nEndProcedure\r\n";
        let packed = super::pack_module_blob_bytes(text, None, None).unwrap();
        assert_eq!(super::unpack_module_blob_text(&packed.blob).unwrap(), text);
    }

    #[test]
    fn parses_common_module_xml_properties() {
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <CommonModule uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>BankServer</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Bank server</v8:content>
        </v8:item>
        <v8:item>
          <v8:lang>en</v8:lang>
          <v8:content>Bank server EN</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Global>true</Global>
      <ClientManagedApplication>true</ClientManagedApplication>
      <Server>false</Server>
      <ExternalConnection>false</ExternalConnection>
      <ClientOrdinaryApplication>true</ClientOrdinaryApplication>
      <ServerCall>false</ServerCall>
      <Privileged>false</Privileged>
      <ReturnValuesReuse>DuringRequest</ReturnValuesReuse>
    </Properties>
  </CommonModule>
</MetaDataObject>
"#;

        let properties = super::parse_common_module_xml_properties(xml).unwrap();

        assert_eq!(properties.uuid, "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
        assert_eq!(properties.name, "BankServer");
        assert_eq!(properties.comment, "");
        assert_eq!(properties.synonyms.len(), 2);
        assert_eq!(properties.synonyms[0].lang, "ru");
        assert_eq!(properties.synonyms[0].content, "Bank server");
        assert_eq!(properties.synonyms[1].lang, "en");
        assert!(properties.global);
        assert!(properties.client_managed_application);
        assert!(!properties.server);
        assert!(!properties.external_connection);
        assert!(properties.client_ordinary_application);
        assert!(!properties.server_call);
        assert!(!properties.privileged);
        assert_eq!(
            properties.return_values_reuse,
            super::ReturnValuesReuse::DuringRequest
        );
    }

    #[test]
    fn parses_simple_metadata_xml_properties() {
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <SessionParameter uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
    <Properties>
      <Name>CurrentUser</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Current user</v8:content>
        </v8:item>
      </Synonym>
      <Comment>Used in tests</Comment>
      <Type>
        <v8:Type>xs:string</v8:Type>
      </Type>
    </Properties>
  </SessionParameter>
</MetaDataObject>
"#;

        let properties = super::parse_simple_metadata_xml_properties(xml).unwrap();

        assert_eq!(properties.kind, "SessionParameter");
        assert_eq!(properties.uuid, "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
        assert_eq!(properties.name, "CurrentUser");
        assert_eq!(properties.comment, "Used in tests");
        assert_eq!(properties.synonyms.len(), 1);
        assert_eq!(properties.synonyms[0].lang, "ru");
        assert_eq!(properties.synonyms[0].content, "Current user");
    }

    #[test]
    fn parses_configuration_xml_properties() {
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <Configuration uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>DemoApp</Name>
      <Synonym>
        <v8:item>
          <v8:lang>en</v8:lang>
          <v8:content>Demo app</v8:content>
        </v8:item>
      </Synonym>
      <Comment>Configuration comment</Comment>
    </Properties>
  </Configuration>
</MetaDataObject>
"#;

        let properties = super::parse_simple_metadata_xml_properties(xml).unwrap();

        assert_eq!(properties.kind, "Configuration");
        assert_eq!(properties.uuid, "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
        assert_eq!(properties.name, "DemoApp");
        assert_eq!(properties.comment, "Configuration comment");
        assert_eq!(properties.synonyms.len(), 1);
        assert_eq!(properties.synonyms[0].lang, "en");
        assert_eq!(properties.synonyms[0].content, "Demo app");
    }

    #[test]
    fn patches_simple_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{1,0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},"OldName",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <SessionParameter uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
    <Properties>
      <Name>NewName</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New &quot;quoted&quot; synonym</v8:content>
        </v8:item>
      </Synonym>
      <Comment>New comment</Comment>
    </Properties>
  </SessionParameter>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "SessionParameter");
        assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        assert!(inflated.contains("\"NewName\""));
        assert!(inflated.contains("{1,\"ru\",\"New \"\"quoted\"\" synonym\"}"));
        assert!(inflated.contains("\"New comment\""));
        assert!(inflated.contains("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn round_trips_additional_simple_metadata_families() {
        for (kind, uuid, name, comment) in [
            (
                "AccumulationRegister",
                "11111111-1111-4111-8111-111111111111",
                "Продажи",
                "Обороты продаж",
            ),
            (
                "AccountingRegister",
                "22222222-2222-4222-8222-222222222222",
                "Хозрасчеты",
                "Бухгалтерский регистр",
            ),
            (
                "CalculationRegister",
                "33333333-3333-4333-8333-333333333333",
                "Премии",
                "Регистр расчета",
            ),
            (
                "ChartOfAccounts",
                "44444444-4444-4444-8444-444444444444",
                "ПланСчетов",
                "План счетов",
            ),
            (
                "ChartOfCalculationTypes",
                "55555555-5555-4555-8555-555555555555",
                "ВидыРасчета",
                "Виды расчета",
            ),
            (
                "ChartOfCalculationRegisters",
                "66666666-6666-4666-8666-666666666666",
                "Начисления",
                "План начислений",
            ),
            (
                "EventSubscription",
                "77777777-7777-4777-8777-777777777777",
                "ЗаписатьВерсиюОбъекта",
                "Подписка на запись",
            ),
            (
                "FunctionalOption",
                "88888888-8888-4888-8888-888888888888",
                "ВыполнятьЗамерыПроизводительности",
                "Функциональная опция",
            ),
            (
                "FunctionalOptionsParameter",
                "99999999-9999-4999-9999-999999999999",
                "ОбщиеНастройкиУзлов",
                "Параметр функциональных опций",
            ),
            (
                "Role",
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
                "АдминистраторСистемы",
                "Роль пользователя",
            ),
            (
                "ScheduledJob",
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
                "ЗагрузкаКурсовВалют",
                "Регламентное задание",
            ),
            (
                "StyleItem",
                "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
                "ВажнаяНадписьШрифт",
                "Элемент стиля",
            ),
            (
                "Subsystem",
                "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
                "СтандартныеПодсистемы",
                "Подсистема",
            ),
            (
                "SettingsStorage",
                "f0f0f0f0-f0f0-4f0f-8f0f-f0f0f0f0f0f0",
                "ХранилищеВариантовОтчетов",
                "Хранилище настроек",
            ),
            (
                "XDTOPackage",
                "12121212-1212-4121-8121-121212121212",
                "АдминистрированиеОбменаДанными_2_4_5_1",
                "XDTO пакет",
            ),
            (
                "Task",
                "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
                "ЗадачаИсполнителя",
                "Задача",
            ),
        ] {
            let base_blob = {
                let mut active = b"\xEF\xBB\xBF".to_vec();
                active.extend_from_slice(
                    format!(
                        "{{1,\n{{3,\n{{1,0,{uuid}}},\"OldName\",\n{{1,\"ru\",\"Old synonym\"}},\"Old comment\",0,0,00000000-0000-0000-0000-000000000000,0}},0}}"
                    )
                    .as_bytes(),
                );
                deflate_raw(&active).unwrap()
            };

            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <{kind} uuid="{uuid}">
    <Properties>
      <Name>{name}</Name>
      <Synonym>
        <item>
          <lang>ru</lang>
          <content>{name}</content>
        </item>
      </Synonym>
      <Comment>{comment}</Comment>
    </Properties>
  </{kind}>
</MetaDataObject>
"#
            );

            let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml.as_bytes())
                .unwrap_or_else(|error| panic!("{kind}: {error}"));
            let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

            assert_eq!(packed.properties.kind, kind);
            assert_eq!(packed.properties.uuid, uuid);
            assert!(inflated.contains(&format!("\"{name}\"")), "{inflated}");
            assert!(inflated.contains(&format!("\"{comment}\"")), "{inflated}");
            assert!(inflated.contains("00000000-0000-0000-0000-000000000000"));
        }
    }

    #[test]
    fn round_trips_additional_business_metadata_families() {
        for (kind, uuid, name, comment) in [
            (
                "Catalog",
                "13131313-1313-4131-8131-131313131313",
                "РолиИсполнителей",
                "Справочник",
            ),
            (
                "Document",
                "14141414-1414-4141-8141-141414141414",
                "АктОбУничтоженииПерсональныхДанных",
                "Документ",
            ),
            (
                "BusinessProcess",
                "15151515-1515-4151-8151-151515151515",
                "Задание",
                "Бизнес-процесс",
            ),
            (
                "ExchangePlan",
                "16161616-1616-4161-8161-161616161616",
                "ОбновлениеИнформационнойБазы",
                "План обмена",
            ),
            (
                "Report",
                "17171717-1717-4171-8171-171717171717",
                "БизнесПроцессы",
                "Отчет",
            ),
            (
                "DataProcessor",
                "18181818-1818-4181-8181-181818181818",
                "АвтоматическоеИзвлечениеТекстов",
                "Обработка",
            ),
            (
                "Enum",
                "19191919-1919-4191-8191-191919191919",
                "ВариантыВажностиЗадачи",
                "Перечисление",
            ),
            (
                "ChartOfCharacteristicTypes",
                "1a1a1a1a-1a1a-41a1-81a1-1a1a1a1a1a1a",
                "ОбъектыАдресацииЗадач",
                "План видов характеристик",
            ),
        ] {
            let base_blob = {
                let mut active = b"\xEF\xBB\xBF".to_vec();
                active.extend_from_slice(
                    format!(
                        "{{1,\n{{3,\n{{1,0,{uuid}}},\"OldName\",\n{{1,\"ru\",\"Old synonym\"}},\"Old comment\",0,0,00000000-0000-0000-0000-000000000000,0}},0}}"
                    )
                    .as_bytes(),
                );
                deflate_raw(&active).unwrap()
            };

            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <{kind} uuid="{uuid}">
    <Properties>
      <Name>{name}</Name>
      <Synonym>
        <item>
          <lang>ru</lang>
          <content>{name}</content>
        </item>
      </Synonym>
      <Comment>{comment}</Comment>
    </Properties>
  </{kind}>
</MetaDataObject>
"#
            );

            let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml.as_bytes())
                .unwrap_or_else(|error| panic!("{kind}: {error}"));
            let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

            assert_eq!(packed.properties.kind, kind);
            assert_eq!(packed.properties.uuid, uuid);
            assert!(inflated.contains(&format!("\"{name}\"")), "{inflated}");
            assert!(inflated.contains(&format!("\"{comment}\"")), "{inflated}");
            assert!(inflated.contains("00000000-0000-0000-0000-000000000000"));
        }
    }

    #[test]
    fn round_trips_real_new_sfc_simple_metadata_families() -> anyhow::Result<()> {
        let sfc_root = std::path::PathBuf::from(r"D:\УХА\sfc");
        if !sfc_root.is_dir() {
            return Ok(());
        }

        for (kind, file) in [
            ("Bot", "Bots/ОповещенияПользователейОСобытиях.xml"),
            (
                "DocumentNumerator",
                "DocumentNumerators/ДенежныеДокументы.xml",
            ),
            (
                "IntegrationService",
                "IntegrationServices/ОбменСообщениями.xml",
            ),
            ("Sequence", "Sequences/ДокументыОрганизаций.xml"),
            ("Style", "Styles/Основной.xml"),
            ("WSReference", "WSReferences/UpdateFilesApiImplService.xml"),
        ] {
            let xml = std::fs::read(sfc_root.join(file))?;
            let parsed = super::parse_simple_metadata_xml_properties(&xml)?;
            assert_eq!(parsed.kind, kind);

            let mut active = b"\xEF\xBB\xBF".to_vec();
            active.extend_from_slice(
                format!(
                    "{{1,\n{{3,\n{{1,0,{uuid}}},\"OldName\",\n{{1,\"ru\",\"Old synonym\"}},\"Old comment\",0,0,00000000-0000-0000-0000-000000000000,0}},0}}",
                    uuid = parsed.uuid,
                )
                .as_bytes(),
            );
            let base_blob = deflate_raw(&active)?;

            let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, &xml)?;
            let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;

            assert_eq!(packed.properties.kind, kind);
            assert_eq!(packed.properties.uuid, parsed.uuid);
            assert!(
                inflated.contains(&format!("\"{}\"", parsed.name)),
                "{inflated}"
            );
            if !parsed.comment.is_empty() {
                assert!(
                    inflated.contains(&format!("\"{}\"", parsed.comment)),
                    "{inflated}"
                );
            }
            assert!(inflated.contains("00000000-0000-0000-0000-000000000000"));
        }

        Ok(())
    }

    #[test]
    fn round_trips_common_form_and_template_from_lab_sources() -> anyhow::Result<()> {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");

        for (kind, file, uuid, expected_name) in [
            (
                "CommonForm",
                "CommonForms/ФормаОтчета.xml",
                "9d6d77a9-1f55-4162-93a5-14bb3f3febaf",
                "ФормаОтчета",
            ),
            (
                "CommonTemplate",
                "CommonTemplates/ВидыДокументовУдостоверяющихЛичность.xml",
                "1682d528-87bf-48c5-acf9-57ab654a615a",
                "ВидыДокументовУдостоверяющихЛичность",
            ),
        ] {
            let xml = std::fs::read(lab_root.join(file))?;
            let mut active = b"\xEF\xBB\xBF".to_vec();
            active.extend_from_slice(
                format!(
                    "{{1,\n{{3,\n{{1,0,{uuid}}},\"OldName\",\n{{1,\"ru\",\"Old synonym\"}},\"Old comment\",0,0,00000000-0000-0000-0000-000000000000,0}},0}}"
                )
                .as_bytes(),
            );
            let base_blob = deflate_raw(&active)?;

            let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, &xml)?;
            let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;

            assert_eq!(packed.properties.kind, kind);
            assert_eq!(packed.properties.uuid, uuid);
            assert!(
                inflated.contains(&format!("\"{expected_name}\"")),
                "{inflated}"
            );
            assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        }

        Ok(())
    }

    #[test]
    fn patches_session_parameter_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{1,0,5efc4bc4-b711-4620-8d2e-9d947c6cc141},"OldParameter",
{1,"ru","Old parameter"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8">
  <SessionParameter uuid="5efc4bc4-b711-4620-8d2e-9d947c6cc141">
    <Properties>
      <Name>АвторизованныйПользователь</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Авторизованный пользователь</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Type>
        <v8:Type>cfg:CatalogRef.ВнешниеПользователи</v8:Type>
        <v8:Type>cfg:CatalogRef.Пользователи</v8:Type>
      </Type>
    </Properties>
  </SessionParameter>
</MetaDataObject>
"#
        .as_bytes();

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "SessionParameter");
        assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        assert!(inflated.contains("\"АвторизованныйПользователь\""));
        assert!(inflated.contains("{1,\"ru\",\"Авторизованный пользователь\"}"));
        assert!(inflated.contains("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn patches_language_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{1,0,db4a9ccb-9ef5-4b3c-8577-b6fe5db1b62e},"OldLanguage",
{1,"ru","Old language"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8">
  <Language uuid="db4a9ccb-9ef5-4b3c-8577-b6fe5db1b62e">
    <Properties>
      <Name>Русский</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Русский</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <LanguageCode>ru</LanguageCode>
    </Properties>
  </Language>
</MetaDataObject>
"#
        .as_bytes();

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "Language");
        assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        assert!(inflated.contains("\"Русский\""));
        assert!(inflated.contains("{1,\"ru\",\"Русский\"}"));
        assert!(inflated.contains("00000000-0000-0000-0000-000000000000"));
    }

    #[test]
    fn patches_settings_storage_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{1,0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},"OldStorage",
{1,"ru","Old storage"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <SettingsStorage uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
    <Properties>
      <Name>NewStorage</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New storage</v8:content>
        </v8:item>
      </Synonym>
      <Comment>New comment</Comment>
      <DefaultSaveForm>SettingsStorage.NewStorage.Form.SaveForm</DefaultSaveForm>
      <DefaultLoadForm>SettingsStorage.NewStorage.Form.LoadForm</DefaultLoadForm>
      <AuxiliarySaveForm/>
      <AuxiliaryLoadForm/>
    </Properties>
  </SettingsStorage>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "SettingsStorage");
        assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        assert!(inflated.contains("\"NewStorage\""));
        assert!(inflated.contains("{1,\"ru\",\"New storage\"}"));
        assert!(inflated.contains("\"New comment\""));
    }

    #[test]
    fn patches_common_attribute_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{1,0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},"OldAttribute",
{1,"ru","Old attribute"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <CommonAttribute uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>NewAttribute</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New attribute</v8:content>
        </v8:item>
      </Synonym>
      <Comment>New comment</Comment>
    </Properties>
  </CommonAttribute>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "CommonAttribute");
        assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        assert!(inflated.contains("\"NewAttribute\""));
        assert!(inflated.contains("{1,\"ru\",\"New attribute\"}"));
        assert!(inflated.contains("\"New comment\""));
    }

    #[test]
    fn patches_web_service_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{1,0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},"OldService",
{1,"ru","Old service"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <WebService uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
    <Properties>
      <Name>NewService</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New service</v8:content>
        </v8:item>
      </Synonym>
      <Comment>New comment</Comment>
    </Properties>
  </WebService>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "WebService");
        assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        assert!(inflated.contains("\"NewService\""));
        assert!(inflated.contains("{1,\"ru\",\"New service\"}"));
        assert!(inflated.contains("\"New comment\""));
    }

    #[test]
    fn patches_common_picture_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{1,0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},"OldPicture",
{1,"ru","Old picture"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <CommonPicture uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>NewPicture</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New picture</v8:content>
        </v8:item>
      </Synonym>
      <Comment>New comment</Comment>
    </Properties>
  </CommonPicture>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "CommonPicture");
        assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        assert!(inflated.contains("\"NewPicture\""));
        assert!(inflated.contains("{1,\"ru\",\"New picture\"}"));
        assert!(inflated.contains("\"New comment\""));
    }

    #[test]
    fn patches_constant_type_and_use_standard_commands() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{16,
{27,
{2,
{3,
{1,0,cccccccc-cccc-4ccc-cccc-cccccccccccc},"OldName",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},
{"Pattern",{"B"}}
},0,{0},{0},0,"",0,{"U"},{"U"},0,00000000-0000-0000-0000-000000000000,2,0,{5006,0},{3,0,0},{0,0},0,{0},{"S",""},0,0,0},
aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,dddddddd-dddd-4ddd-dddd-dddddddddddd,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee,1,0,{0},{0},00000000-0000-0000-0000-000000000000,0,0,ffffffff-ffff-4fff-ffff-ffffffffffff,99999999-9999-4999-9999-999999999999,0,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <Constant uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc">
    <Properties>
      <Name>NewConstant</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New synonym</v8:content>
        </v8:item>
      </Synonym>
      <Comment>New comment</Comment>
      <Type>
        <v8:Type>xs:string</v8:Type>
        <v8:StringQualifiers>
          <v8:Length>50</v8:Length>
          <v8:AllowedLength>Variable</v8:AllowedLength>
        </v8:StringQualifiers>
      </Type>
      <UseStandardCommands>true</UseStandardCommands>
    </Properties>
  </Constant>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "Constant");
        assert!(inflated.contains("\"NewConstant\""));
        assert!(inflated.contains("{1,\"ru\",\"New synonym\"}"));
        assert!(inflated.contains("\"New comment\""));
        assert!(inflated.contains(r#"{"Pattern",{"S",50,1}}"#));
        assert!(inflated.contains(",1,1,{0}"));
    }

    #[test]
    fn patches_defined_type_builtin_type_pattern() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldType",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},
{"Pattern",{"S",10,1}}
},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <DefinedType uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewType</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New synonym</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Type>
        <v8:Type>xs:boolean</v8:Type>
        <v8:Type>xs:string</v8:Type>
        <v8:StringQualifiers>
          <v8:Length>80</v8:Length>
          <v8:AllowedLength>Variable</v8:AllowedLength>
        </v8:StringQualifiers>
      </Type>
    </Properties>
  </DefinedType>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "DefinedType");
        assert!(inflated.contains("\"NewType\""), "{inflated}");
        assert!(inflated.contains("{1,\"ru\",\"New synonym\"}"));
        assert!(inflated.contains(r#"{"Pattern",{"B"},{"S",80,1}}"#));
    }

    #[test]
    fn patches_defined_type_cfg_reference_type_pattern() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Catalogs"))?;
        std::fs::write(
            root.join("Catalogs").join("TestUsers.xml"),
            br#"
<MetaDataObject>
  <Catalog uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <InternalInfo>
      <xr:GeneratedType xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" name="CatalogRef.TestUsers" category="Ref">
        <xr:TypeId>bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb</xr:TypeId>
      </xr:GeneratedType>
    </InternalInfo>
    <Properties>
      <Name>TestUsers</Name>
      <Synonym/>
      <Comment/>
    </Properties>
  </Catalog>
</MetaDataObject>
"#,
        )?;

        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{0,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldType",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},
{"Pattern",{"S",10,1}}
},0}"#,
        );
        let base_blob = deflate_raw(&active)?;
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <DefinedType uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewType</Name>
      <Synonym/>
      <Comment/>
      <Type>
        <v8:Type>cfg:CatalogRef.TestUsers</v8:Type>
      </Type>
    </Properties>
  </DefinedType>
</MetaDataObject>
"#;
        let source = super::MetadataSourceContext::new(root.clone());

        let packed =
            super::pack_simple_metadata_blob_from_xml_with_source(&base_blob, xml, Some(&source))?;
        let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(packed.properties.kind, "DefinedType");
        assert!(inflated.contains("{\"Pattern\",{\"#\",bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb}}"));

        Ok(())
    }

    #[test]
    fn patches_common_command_metadata_fields() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,dddddddd-dddd-4ddd-dddd-dddddddddddd,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,{0},"",-1,-1,1,0,""},3,
{0},1,
{0,0,0},0,
{1,77ea1b8f-dd79-4717-9dba-5628e7f348cf},
{"Pattern"},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldCommand",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},0,0,0}
}
},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <CommonCommand uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewCommand</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New synonym</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Group>NavigationPanelOrdinary</Group>
      <Representation>PictureAndText</Representation>
      <ToolTip>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New tip</v8:content>
        </v8:item>
      </ToolTip>
      <Picture/>
      <Shortcut/>
      <IncludeHelpInContents>true</IncludeHelpInContents>
      <CommandParameterType/>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>true</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "CommonCommand");
        assert!(inflated.contains("\"NewCommand\""), "{inflated}");
        assert!(inflated.contains("{1,\"ru\",\"New synonym\"}"));
        assert!(inflated.contains("{1,\"ru\",\"New tip\"}"));
        assert!(inflated.contains("{4,0,{0},\"\",-1,-1,1,0,\"\"},2,"));
        assert!(inflated.contains("{0,0,0},1,"));
        assert!(inflated.contains("},1,0,0}"));
    }

    #[test]
    fn parses_common_command_representation_text() {
        assert_eq!(
            parse_common_command_representation(Some("Text".to_string())).unwrap(),
            CommonCommandRepresentation::Text
        );
        assert_eq!(
            common_command_representation_code(CommonCommandRepresentation::Text),
            0
        );
    }

    #[test]
    fn patches_command_group_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{4,0,{0},"",-1,-1,1,0,""},4,3,
{0},
{0},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldGroup",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0}
},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommandGroup uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewGroup</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New group</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Representation>Picture</Representation>
      <ToolTip>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Group tip</v8:content>
        </v8:item>
      </ToolTip>
      <Picture>
        <xr:Ref>StdPicture.Print</xr:Ref>
        <xr:LoadTransparent>true</xr:LoadTransparent>
      </Picture>
      <Category>ActionsPanel</Category>
    </Properties>
  </CommandGroup>
</MetaDataObject>
"#
        .as_bytes();

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(packed.properties.kind, "CommandGroup");
        assert!(inflated.contains("\"NewGroup\""), "{inflated}");
        assert!(inflated.contains("{1,\"ru\",\"New group\"}"));
        assert!(inflated.contains("{1,\"ru\",\"Group tip\"}"));
        assert!(inflated.contains("{4,1,{-13},\"\",-1,-1,1,0,\"\"}"));
        assert!(inflated.contains(",4,1,"));
    }

    #[test]
    fn patches_command_group_navigation_panel_categories() {
        let cases = [
            (
                "NavigationPanel",
                r#"{1,
{3,
{4,0,{0},"",-1,-1,1,0,""},1,3,
{0},
{0},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldGroup",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0}
},0}"#,
                ",1,3,",
            ),
            (
                "FormNavigationPanel",
                r#"{1,
{3,
{4,0,{0},"",-1,-1,1,0,""},2,3,
{0},
{0},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldGroup",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0}
},0}"#,
                ",2,3,",
            ),
        ];

        for (category, active_inner, expected_code) in cases {
            let mut active = b"\xEF\xBB\xBF".to_vec();
            active.extend_from_slice(active_inner.as_bytes());
            let base_blob = deflate_raw(&active).unwrap();
            let xml = format!(
                r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommandGroup uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewGroup</Name>
      <Synonym/>
      <Comment/>
      <Representation>Auto</Representation>
      <ToolTip/>
      <Picture/>
      <Category>{category}</Category>
    </Properties>
  </CommandGroup>
</MetaDataObject>
"#
            );
            let packed =
                super::pack_simple_metadata_blob_from_xml(&base_blob, xml.as_bytes()).unwrap();
            let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();
            assert!(inflated.contains(expected_code), "{inflated}");
        }
    }

    #[test]
    fn patches_command_group_metadata_blob_from_lab_xml() -> anyhow::Result<()> {
        let source_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let source = super::MetadataSourceContext::new(source_root);

        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{4,0,{0},"",-1,-1,1,0,""},4,3,
{0},
{0},
{3,
{1,0,c59e11f3-6bcb-404a-9d76-1416c12be354},"OldGroup",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0}
},0}"#,
        );
        let base_blob = deflate_raw(&active)?;
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommandGroup uuid="c59e11f3-6bcb-404a-9d76-1416c12be354">
    <Properties>
      <Name>Органайзер</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Органайзер</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture>
        <xr:Ref>CommonPicture.Органайзер</xr:Ref>
        <xr:LoadTransparent>false</xr:LoadTransparent>
      </Picture>
      <Category>FormCommandBar</Category>
    </Properties>
  </CommandGroup>
</MetaDataObject>
"#
        .as_bytes();

        let packed =
            super::pack_simple_metadata_blob_from_xml_with_source(&base_blob, xml, Some(&source))?;
        let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;

        assert_eq!(packed.properties.kind, "CommandGroup");
        assert!(inflated.contains("\"Органайзер\""), "{inflated}");
        assert!(inflated.contains("{1,\"ru\",\"Органайзер\"}"));
        assert!(
            inflated.contains("{4,1,{0,dce82d28-9a7b-4d4c-af13-90f459cf4af2},\"\",-1,-1,0,0,\"\"}")
        );

        Ok(())
    }

    #[test]
    fn patches_common_command_picture_and_parameter_type() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("CommonPictures"))?;
        std::fs::create_dir_all(root.join("DefinedTypes"))?;
        std::fs::create_dir_all(root.join("CommandGroups"))?;
        std::fs::write(
            root.join("CommonPictures").join("TestPicture.xml"),
            br#"
<MetaDataObject>
  <CommonPicture uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>TestPicture</Name>
      <Synonym/>
      <Comment/>
    </Properties>
  </CommonPicture>
</MetaDataObject>
"#,
        )?;
        std::fs::write(
            root.join("DefinedTypes").join("TestOwner.xml"),
            br#"
<MetaDataObject>
  <DefinedType uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
    <InternalInfo>
      <xr:GeneratedType xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" name="DefinedType.TestOwner" category="DefinedType">
        <xr:TypeId>cccccccc-cccc-4ccc-cccc-cccccccccccc</xr:TypeId>
      </xr:GeneratedType>
    </InternalInfo>
    <Properties>
      <Name>TestOwner</Name>
      <Synonym/>
      <Comment/>
    </Properties>
  </DefinedType>
</MetaDataObject>
"#,
        )?;
        std::fs::write(
            root.join("CommandGroups").join("Органайзер.xml"),
            r#"
<MetaDataObject>
  <CommandGroup uuid="c59e11f3-6bcb-404a-9d76-1416c12be354">
    <Properties>
      <Name>Органайзер</Name>
      <Synonym/>
      <Comment/>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture/>
      <Category>FormCommandBar</Category>
    </Properties>
  </CommandGroup>
</MetaDataObject>
"#,
        )?;

        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,dddddddd-dddd-4ddd-dddd-dddddddddddd,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,{0},"",-1,-1,1,0,""},3,
{0},1,
{0,0,0},0,
{1,77ea1b8f-dd79-4717-9dba-5628e7f348cf},
{"Pattern"},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldCommand",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},0,0,0}
}
},0}"#,
        );
        let base_blob = deflate_raw(&active)?;
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommonCommand uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewCommand</Name>
      <Synonym/>
      <Comment/>
      <Group>CommandGroup.Органайзер</Group>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture>
        <xr:Ref>CommonPicture.TestPicture</xr:Ref>
        <xr:LoadTransparent>false</xr:LoadTransparent>
      </Picture>
      <Shortcut/>
      <IncludeHelpInContents>false</IncludeHelpInContents>
      <CommandParameterType>
        <v8:TypeSet>cfg:DefinedType.TestOwner</v8:TypeSet>
      </CommandParameterType>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#
        .as_bytes();
        let source = super::MetadataSourceContext::new(root.clone());

        let packed =
            super::pack_simple_metadata_blob_from_xml_with_source(&base_blob, xml, Some(&source))?;
        let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(packed.properties.kind, "CommonCommand");
        assert!(
            inflated
                .contains(r#"{4,1,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},"",-1,-1,0,0,""},1,"#)
        );
        assert!(inflated.contains("{1,c59e11f3-6bcb-404a-9d76-1416c12be354}"));
        assert!(inflated.contains("{\"Pattern\",{\"#\",cccccccc-cccc-4ccc-cccc-cccccccccccc}}"));

        Ok(())
    }

    #[test]
    fn packs_style_body_xml_with_standard_and_style_item_refs() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-style-body-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("StyleItems"))?;
        std::fs::write(
            root.join("StyleItems/ErrorBackColor.xml"),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
	<StyleItem uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa">
		<Properties>
			<Name>ErrorBackColor</Name>
		</Properties>
	</StyleItem>
</MetaDataObject>
"#,
        )?;
        std::fs::write(
            root.join("StyleItems/StrikeFont.xml"),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
	<StyleItem uuid="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb">
		<Properties>
			<Name>StrikeFont</Name>
		</Properties>
	</StyleItem>
</MetaDataObject>
"#,
        )?;
        let source = MetadataSourceContext::new(root.clone());
        let xml = br##"<?xml version="1.0" encoding="UTF-8"?>
<Style xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" xmlns:web="http://v8.1c.ru/8.1/data/ui/colors/web" version="2.21">
	<Item name="FormBackColor">
		<Color>web:Cream</Color>
	</Item>
	<Item name="ControlBorder">
		<Border ref="style:ControlBorder"/>
	</Item>
	<Item name="TextFont">
		<Font ref="style:TextFont" kind="StyleItem"/>
	</Item>
	<Item name="StyleItem.ErrorBackColor">
		<Color>#FFC8C8</Color>
	</Item>
	<Item name="StyleItem.StrikeFont">
		<Font ref="style:TextFont" kind="StyleItem" bold="false" italic="false" underline="false" strikeout="true"/>
	</Item>
</Style>
"##;

        let packed = super::pack_style_body_blob_from_xml(xml, Some(&source))?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.starts_with("{2,5,"));
        assert!(text.contains("{{-1},0,{4,2,{20},2}}"));
        assert!(text.contains("{{-18},2,{3,1,{-18},0,0,0}}"));
        assert!(text.contains("{{-20},1,{8,2,0,{-20},1,100}}"));
        assert!(text.contains("{{0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},0,{4,0,{13158655},0}}"));
        assert!(text.contains(
            "{{0,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb},1,{8,2,0,{-20},400,0,0,1,1,100}}"
        ));
        assert!(text.ends_with(",{0}}"));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_scheduled_job_schedule_xml() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<JobSchedule xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:ent="http://v8.1c.ru/8.1/data/enterprise" version="2.17">
	<Schedule BeginDate="0001-01-01" EndDate="0001-01-01" BeginTime="08:00:00" EndTime="17:00:00" CompletionTime="00:00:00" CompletionInterval="0" RepeatPeriodInDay="60" RepeatPause="0" WeekDayInMonth="0" DayInMonth="1" WeeksPeriod="1" DaysRepeatPeriod="0">
		<ent:WeekDays>6 7</ent:WeekDays>
		<ent:Months>1 2 3 4 5 6 7 8 9 10 11 12</ent:Months>
	</Schedule>
</JobSchedule>
"#;

        let packed = super::pack_schedule_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(
            text,
            "{00010101000000,00010101000000,00010101080000,00010101170000,00010101000000,0,60,0,2,6,7,0,1,12,1,2,3,4,5,6,7,8,9,10,11,12,1,0}"
        );
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_raw_deflated_blob_from_bytes() -> anyhow::Result<()> {
        let bytes = b"<?xml version=\"1.0\"?><Definition><Item name=\"A\"/></Definition>";

        let packed = super::pack_raw_deflated_blob_from_bytes(bytes)?;

        assert_eq!(super::inflate_raw(&packed.blob)?, bytes);
        assert_eq!(packed.plain_bytes, bytes.len());

        Ok(())
    }

    #[test]
    fn packs_simple_spreadsheet_document_xml() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<columns>
		<size>3</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>0</f>
					<tl>
						<v8:item>
							<v8:lang>ru</v8:lang>
							<v8:content>Hello</v8:content>
						</v8:item>
					</tl>
				</c>
			</c>
			<c>
				<i>2</i>
				<c>
					<f>0</f>
					<parameter>Name</parameter>
				</c>
			</c>
		</row>
	</rowsItem>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.starts_with("MOXCEL\0"));
        assert!(text.contains(r#"{16,0,{1,1,{"ru","Hello"}},0}"#));
        assert!(text.contains(r#"{16,0,{1,1,{"","Name"}},0}"#));
        assert!(text.contains(r#"2,{16,0,{1,1,{"","Name"}},0}"#));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_detail_parameter_cells() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>3</f>
					<parameter>Name</parameter>
					<detailParameter>Version</detailParameter>
				</c>
			</c>
		</row>
	</rowsItem>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(r#"{24,2,"Version",{1,1,{"","Name"}},0}"#));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_column_sets_and_row_columns_id() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
	<columns>
		<size>2</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>3</formatIndex>
			</column>
		</columnsItem>
		<columnsItem>
			<index>1</index>
			<column>
				<formatIndex>4</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<columns>
		<id>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</id>
		<size>1</size>
		<columnsItem>
			<index>0</index>
			<column>
				<formatIndex>5</formatIndex>
			</column>
		</columnsItem>
	</columns>
	<rowsItem>
		<index>1</index>
		<row>
			<columnsID>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</columnsID>
			<empty>true</empty>
		</row>
	</rowsItem>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(
            "{2,0,00000000-0000-0000-0000-000000000000,2,0,3,1,4},2,1,{1,0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa,1,0,5},1,1,0"
        ));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_empty_row_ranges() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>1</index>
		<indexTo>3</indexTo>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(",1,0,0,2,0,0,3,0,0,"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_merges() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
	<columns>
		<size>4</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<merge>
		<r>1</r>
		<c>2</c>
		<h>3</h>
		<w>1</w>
	</merge>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("{1,{2,1,3,4}}"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_named_and_print_areas() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<columns>
		<size>5</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<namedItem xsi:type="NamedItemCells">
		<name>Header</name>
		<area>
			<type>Rectangle</type>
			<beginRow>1</beginRow>
			<endRow>3</endRow>
			<beginColumn>2</beginColumn>
			<endColumn>4</endColumn>
			<columnsID>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</columnsID>
		</area>
	</namedItem>
	<printArea>
		<type>Rows</type>
		<beginRow>5</beginRow>
		<endRow>7</endRow>
		<beginColumn>0</beginColumn>
		<endColumn>4</endColumn>
	</printArea>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(
            text.contains(r#"{1,"Header",{1,{3,2,1,4,3,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},0}}"#)
        );
        assert!(text.contains("{1,0,5,4,7,00000000-0000-0000-0000-000000000000}"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_print_settings() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<printSettings>
		<pageOrientation>Landscape</pageOrientation>
		<scale>80</scale>
		<collate>true</collate>
		<copies>2</copies>
		<perPage>1</perPage>
		<topMargin>1000</topMargin>
		<leftMargin>1100</leftMargin>
		<bottomMargin>1200</bottomMargin>
		<rightMargin>1300</rightMargin>
		<headerSize>140</headerSize>
		<footerSize>150</footerSize>
		<fitToPage>false</fitToPage>
		<blackAndWhite>true</blackAndWhite>
		<printerName>Printer "A"</printerName>
		<paper>9</paper>
		<paperSource>7</paperSource>
		<pageWidth>210</pageWidth>
		<pageHeight>297</pageHeight>
	</printSettings>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(
            r#"{{0,18,0,{"N",9},1,{"N",2},2,{"N",80},3,{"N",1},4,{"N",2},5,{"N",1},6,{"N",1000},7,{"N",1100},8,{"N",1200},9,{"N",1300},10,{"N",140},11,{"N",150},12,{"N",0},13,{"N",1},14,{"S","Printer ""A"""},15,{"N",7},16,{"N",210},17,{"N",297}}}"#
        ));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_basic_formats() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<c>
				<c>
					<f>2</f>
					<parameter>Name</parameter>
				</c>
			</c>
		</row>
	</rowsItem>
	<defaultFormatIndex>2</defaultFormatIndex>
	<format>
		<width>72</width>
	</format>
	<format>
		<horizontalAlignment>Center</horizontalAlignment>
		<fillType>Parameter</fillType>
	</format>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(r#"{3,{128,72},{33024,6,1},{1,0}}"#));
        assert!(text.contains(r#"{16,1,{1,1,{"","Name"}},0}"#));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_format_colors() -> anyhow::Result<()> {
        let xml = br##"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<format>
		<width>72</width>
		<textColor>#009646</textColor>
		<backColor>style:ButtonBackColor</backColor>
	</format>
</document>
"##;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("{3,0,{4625920}},{3,3,{-7}},{2,{3200,72,0,1},{1,0}}"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_fonts() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<font faceName="Arial" height="8" bold="true" italic="false" underline="false" strikeout="false" kind="Absolute" scale="100"/>
	<font ref="style:NormalTextFont" bold="true" italic="false" underline="true" strikeout="false" kind="StyleItem"/>
	<format>
		<font>1</font>
	</format>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("{2,{1,1},{1,0}}"));
        assert!(text.contains(r#"{7,0,575,80,0,0,0,700,0,0,0,0,0,0,0,0,"Arial",1,100}"#));
        assert!(text.contains("{7,2,60,{-31},700,0,1,0,1,100}"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_spreadsheet_lines() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<document xmlns="http://v8.1c.ru/8.2/data/spreadsheet" xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
	<columns>
		<size>1</size>
	</columns>
	<rowsItem>
		<index>0</index>
		<row>
			<empty>true</empty>
		</row>
	</rowsItem>
	<line width="1" gap="false">
		<v8ui:style xsi:type="v8ui:SpreadsheetDocumentCellLineType">None</v8ui:style>
	</line>
	<line width="1" gap="false">
		<v8ui:style xsi:type="v8ui:SpreadsheetDocumentCellLineType">Solid</v8ui:style>
	</line>
	<format>
		<border>0</border>
	</format>
</document>
"#;

        let packed = super::pack_moxel_spreadsheet_blob_from_xml(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("{3,3,{-1}},{3,3,{-3}},{2,{30,0,0,0,0},{1,0}}"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_form_body_module_text_preserving_base_layout() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{7,{\"layout\"}},\"Old module\",{3,{\"picture\"},\"payload\"}}",
        )?;
        let packed = super::pack_form_body_blob_from_module_text(
            &base,
            b"\xEF\xBB\xBFProcedure Run()\r\n\tMessage(\"Hi\");\r\nEndProcedure\r\n",
        )?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(
            text,
            "{4,{7,{\"layout\"}},\"Procedure Run()\r\n\tMessage(\"\"Hi\"\");\r\nEndProcedure\r\n\",{3,{\"picture\"},\"payload\"}}"
        );
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_role_rights_xml_preserving_base_identifiers() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{10,{2,{{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,0,0},{0,11111111-1111-4111-8111-111111111111,1,22222222-2222-4222-8222-222222222222,-1}},{{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,0,0},{1,1,33333333-3333-4333-8333-333333333333,-1,0}}},{0},4294967295,0,0,4294967295}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>true</setForNewObjects>
	<object>
		<name>Catalog.Products</name>
		<right><name>Read</name><value>false</value></right>
		<right><name>Update</name><value>true</value></right>
	</object>
	<object>
		<name>InformationRegister.Prices</name>
		<right>
			<name>Read</name>
			<value>true</value>
			<restrictionByCondition><condition>WHERE TRUE</condition></restrictionByCondition>
		</right>
	</object>
</Rights>
"#;

        let packed = super::pack_role_rights_blob_from_xml(&base, xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"));
        assert!(text.contains("11111111-1111-4111-8111-111111111111,-1"));
        assert!(text.contains("22222222-2222-4222-8222-222222222222,1"));
        assert!(text.contains("33333333-3333-4333-8333-333333333333,1"));
        assert!(text.ends_with(",4294967295,1,0,4294967295}"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_command_interface_xml_preserving_command_refs() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{7,1,2,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{{0,{{0,{{\"B\",0}},0}}}},{100,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},{{0,{{0,{{\"B\",1}},0}}}},0,0,0,0,0}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="Catalog.Products.StandardCommand.OpenList">
			<Visibility><xr:Common>true</xr:Common></Visibility>
		</Command>
		<Command name="100:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
			<Visibility><xr:Common>false</xr:Common></Visibility>
		</Command>
	</CommandsVisibility>
</CommandInterface>
"#;

        let packed = super::pack_command_interface_blob_from_xml(&base, xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(
            text.contains("{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{{0,{{0,{{\"B\",1}},0}}}}")
        );
        assert!(
            text.contains("{100,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},{{0,{{0,{{\"B\",0}},0}}}}")
        );
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_exchange_plan_content_xml_with_metadata_refs() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-module-blob-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Catalogs"))?;
        std::fs::create_dir_all(root.join("InformationRegisters"))?;
        std::fs::write(
            root.join("Catalogs/Customers.xml"),
            br#"<MetaDataObject><Catalog uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"><Properties><Name>Customers</Name></Properties></Catalog></MetaDataObject>"#,
        )?;
        std::fs::write(
            root.join("InformationRegisters/Prices.xml"),
            br#"<MetaDataObject><InformationRegister uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc"><Properties><Name>Prices</Name></Properties></InformationRegister></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(b"{2,0}")?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<ExchangePlanContent xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20">
	<Item>
		<Metadata>Catalog.Customers</Metadata>
		<AutoRecord>Deny</AutoRecord>
	</Item>
	<Item>
		<Metadata>InformationRegister.Prices</Metadata>
		<AutoRecord>Auto</AutoRecord>
	</Item>
</ExchangePlanContent>
"#;

        let packed = super::pack_exchange_plan_content_blob_from_xml(&base, xml, &source)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(
            text,
            "{2,2,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,0,cccccccc-cccc-4ccc-cccc-cccccccccccc,1}"
        );
        assert_eq!(packed.plain_bytes, text.len());

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_predefined_data_xml_preserving_base_shape() -> anyhow::Result<()> {
        let type_uuid = "ae135932-4f94-44df-92c1-c91f15a92848";
        let folder_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let item_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let base_plain = format!(
            "{{0,{{1,{{7}},{{2,{{1,1,{{2,0,5,{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"B\",1}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Элементы\"}},{{\"S\",\"\"}},1,{{1,1,{{2,1,7,{{\"#\",{type_uuid},{{1,{folder_uuid}}}}},{{\"B\",1}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Folder\"}},{{\"S\",\"F\"}},{{\"S\",\"Folder description\"}},{{\"N\",0}},1,{{1,1,{{2,2,7,{{\"#\",{type_uuid},{{1,{item_uuid}}}}},{{\"B\",0}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Item\"}},{{\"S\",\"I\"}},{{\"S\",\"Item description\"}},{{\"N\",0}},0}}}}}}}}}}}}}},-1,3}}}}"
        );
        let base = super::deflate_raw(base_plain.as_bytes())?;
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<PredefinedData xmlns="http://v8.1c.ru/8.3/xcf/predef" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="CatalogPredefinedItems" version="2.20">
	<Item id="{folder_uuid}">
		<Name>NewFolder</Name>
		<Code>NF</Code>
		<Description>New folder description</Description>
		<IsFolder>true</IsFolder>
		<ChildItems>
			<Item id="{item_uuid}">
				<Name>NewItem</Name>
				<Code>NI</Code>
				<Description>New item description</Description>
				<IsFolder>false</IsFolder>
			</Item>
		</ChildItems>
	</Item>
</PredefinedData>
"#
        );

        let packed = super::pack_predefined_data_blob_from_xml(&base, xml.as_bytes())?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(r#"{{"S","NewFolder"}}"#));
        assert!(text.contains(r#"{{"S","NF"}}"#));
        assert!(text.contains(r#"{{"S","New folder description"}}"#));
        assert!(text.contains(r#"{{"S","NewItem"}}"#));
        assert!(text.contains(r#"{{"S","NI"}}"#));
        assert!(text.contains(r#"{{"S","New item description"}}"#));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_flowchart_xml_preserving_base_shape() -> anyhow::Result<()> {
        let start_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let done_uuid = "cccccccc-cccc-4ccc-cccc-cccccccccccc";
        let style = "{7,{3,4,{0}},{3,3,{-22}},{3,3,{-3}},{7,1,0,{0},1,100},{1,0},1,1,1,0,0,0,0,0}";
        let line_style =
            "{7,{3,0,{0}},{3,3,{-22}},{3,3,{-3}},{7,1,0,{0},1,100},{1,0},1,1,1,1,0,0,0,0}";
        let border = "{4,0,{0},1,1,0,e45c0cd8-a878-4bcb-8e1a-af934481e1cc,0}";
        let start_head = format!("{{{{4,1,{{1,0}},\"Start\",1}},4,{start_uuid},0}}");
        let completion_head = format!("{{{{4,3,{{1,0}},\"Done\",3}},4,{done_uuid},0}}");
        let line_head = "{4,2,{1,0},\"Line\",2}";
        let start_geometry = format!("{{{style},5,10,20,50,60}}");
        let completion_geometry = format!("{{{style},5,70,80,110,120}}");
        let start_shape = format!("{{{{{start_geometry},1}}}}");
        let completion_shape = format!("{{{{{completion_geometry},1}}}}");
        let line_geometry = format!("{{{line_style},6,2,50,60,70,80,{border},0,4,2,0,0,1}}");
        let line_shape = format!("{{{line_geometry}}}");
        let start_item = format!("{{{start_head},2,{start_shape},{{1,{{0,\"BeforeStart\"}}}}}}");
        let line_item = format!("{{{line_head},3,1,0,3,0,{line_shape}}}");
        let completion_item =
            format!("{{{completion_head},2,{completion_shape},{{1,{{0,\"OnDone\"}}}}}}");
        let base_plain = format!(
            "{{5,{{{{1,{style},1,20,20}}}},3,2,{start_item},1,{line_item},3,{completion_item},4}}"
        );
        let base = super::deflate_raw(base_plain.as_bytes())?;
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<GraphicalSchema xmlns="http://v8.1c.ru/8.3/xcf/scheme" version="2.20">
	<Items>
		<Start id="1" uuid="{start_uuid}">
			<Properties>
				<Name>StartRenamed</Name>
				<TabOrder>11</TabOrder>
			</Properties>
			<Events>
				<Event name="BeforeStart">BeforeStartRenamed</Event>
			</Events>
		</Start>
		<ConnectionLine id="2">
			<Properties>
				<Name>LineRenamed</Name>
				<TabOrder>12</TabOrder>
			</Properties>
		</ConnectionLine>
		<Completion id="3" uuid="{done_uuid}">
			<Properties>
				<Name>DoneRenamed</Name>
				<TabOrder>13</TabOrder>
			</Properties>
			<Events>
				<Event name="OnComplete">OnDoneRenamed</Event>
			</Events>
		</Completion>
	</Items>
</GraphicalSchema>
"#
        );

        let packed = super::pack_business_process_flowchart_blob_from_xml(&base, xml.as_bytes())?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(r#""StartRenamed",11"#));
        assert!(text.contains(r#""BeforeStartRenamed""#));
        assert!(text.contains(r#""LineRenamed",12"#));
        assert!(text.contains(r#""DoneRenamed",13"#));
        assert!(text.contains(r#""OnDoneRenamed""#));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_base64_payload_blob_from_bytes() -> anyhow::Result<()> {
        let packed = super::pack_base64_payload_blob_from_bytes(b"PK\x03\x04")?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(text, "{#base64:UEsDBA==}");
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_help_blob_from_pages_and_files() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Help xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20">
	<Page>ru</Page>
</Help>
"#;
        let pages = super::parse_help_pages_from_xml(xml)?;

        assert_eq!(pages, vec!["ru"]);
        let packed = super::pack_help_blob_from_parts(
            &[("ru".to_string(), b"<html></html>".to_vec())],
            &[("shot.png".to_string(), b"\x89PNG\r\n\x1a\n".to_vec())],
        )?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(
            text,
            "{5,1,\"ru\",{#base64:PGh0bWw+PC9odG1sPg==},1,\"shot.png\",1,{#base64:iVBORw0KGgo=}}"
        );
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_ext_picture_blob_from_xml_referenced_bytes() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<ExtPicture xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.17">
	<Picture>
		<xr:Abs>Picture.zip</xr:Abs>
		<xr:LoadTransparent>false</xr:LoadTransparent>
	</Picture>
</ExtPicture>
"#;

        assert_eq!(
            super::parse_ext_picture_file_name_from_xml(xml)?,
            "Picture.zip"
        );
        let packed = super::pack_ext_picture_blob_from_bytes(b"PK\x03\x04")?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(text, "{1,{0,0,-1,-1},{{#base64:UEsDBA==}}}");
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn parses_template_type_from_metadata_xml() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses">
	<CommonTemplate uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa">
		<Properties>
			<Name>SharedText</Name>
			<TemplateType>TextDocument</TemplateType>
		</Properties>
	</CommonTemplate>
</MetaDataObject>
"#;

        assert_eq!(
            super::parse_template_type_from_xml(xml)?,
            Some("TextDocument".to_string())
        );

        Ok(())
    }

    #[test]
    fn patches_common_command_metadata_blob_from_lab_xml() -> anyhow::Result<()> {
        let source_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let source = super::MetadataSourceContext::new(source_root);

        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,becf53b6-3fbc-4c70-822f-4a70b0434353,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,{0},"",-1,-1,1,0,""},3,
{0},1,
{0,0,0},0,
{1,cb50f5c0-8013-4262-93a2-f0db379d6b6b},
{"Pattern"},
{3,
{1,0,becf53b6-3fbc-4c70-822f-4a70b0434353},"OldCommand",
{1,"ru","Old synonym"},"Old comment",{1,cb50f5c0-8013-4262-93a2-f0db379d6b6b},3,{0,0,0},0,0},0,0,0}
}
},0}"#,
        );
        let base_blob = deflate_raw(&active)?;
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommonCommand uuid="becf53b6-3fbc-4c70-822f-4a70b0434353">
    <Properties>
      <Name>ДополнительныеСведенияКоманднаяПанель</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Дополнительные сведения</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Group>FormCommandBarImportant</Group>
      <Representation>Picture</Representation>
      <ToolTip>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Дополнительные сведения</v8:content>
        </v8:item>
      </ToolTip>
      <Picture>
        <xr:Ref>CommonPicture.ДополнительныеСведения</xr:Ref>
        <xr:LoadTransparent>false</xr:LoadTransparent>
      </Picture>
      <Shortcut/>
      <IncludeHelpInContents>false</IncludeHelpInContents>
      <CommandParameterType>
        <v8:TypeSet>cfg:DefinedType.ВладелецДополнительныхСведений</v8:TypeSet>
      </CommandParameterType>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#
        .as_bytes();

        let packed =
            super::pack_simple_metadata_blob_from_xml_with_source(&base_blob, xml, Some(&source))?;
        let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;

        assert_eq!(packed.properties.kind, "CommonCommand");
        assert!(inflated.contains("\"ДополнительныеСведенияКоманднаяПанель\""));
        assert!(inflated.contains("{1,\"ru\",\"Дополнительные сведения\"}"));
        assert!(
            inflated.contains("{4,1,{0,a755cb43-492d-4069-9b6a-29b92ebb5b0e},\"\",-1,-1,0,0,\"\"}")
        );
        assert!(inflated.contains("{1,cb50f5c0-8013-4262-93a2-f0db379d6b6b}"));
        assert!(inflated.contains("{\"Pattern\",{\"#\",2da879f6-1141-480b-b647-fdf6698f8aba}}"));

        Ok(())
    }

    #[test]
    fn packs_common_command_std_picture_user_reference() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,dddddddd-dddd-4ddd-dddd-dddddddddddd,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,{0},"",-1,-1,1,0,""},3,
{0},1,
{0,0,0},0,
{1,77ea1b8f-dd79-4717-9dba-5628e7f348cf},
{"Pattern"},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldCommand",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},0,0,0}
}
},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommonCommand uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewCommand</Name>
      <Synonym/>
      <Comment/>
      <Group>FormCommandBarImportant</Group>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture>
        <xr:Ref>StdPicture.User</xr:Ref>
        <xr:LoadTransparent>false</xr:LoadTransparent>
      </Picture>
      <Shortcut/>
      <IncludeHelpInContents>false</IncludeHelpInContents>
      <CommandParameterType/>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#
        .as_bytes();

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();
        assert!(inflated.contains(super::STD_PICTURE_USER_UUID));
        assert!(
            inflated.contains("{4,1,{0,6ff3ddbd-56e3-4ddf-a5bf-048c1e2dfb2f},\"\",-1,-1,0,0,\"\"}")
        );
        assert!(!inflated.contains("StdPicture.User"));
    }

    #[test]
    fn packs_command_group_std_picture_information_register_reference() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{4,0,{0},"",-1,-1,1,0,""},1,3,
{0},
{0},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldGroup",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0}
},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommandGroup uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewGroup</Name>
      <Synonym/>
      <Comment/>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture>
        <xr:Ref>StdPicture.InformationRegister</xr:Ref>
        <xr:LoadTransparent>true</xr:LoadTransparent>
      </Picture>
      <Category>NavigationPanel</Category>
    </Properties>
  </CommandGroup>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml.as_bytes()).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();
        assert!(inflated.contains(super::STD_PICTURE_INFORMATION_REGISTER_UUID));
        assert!(
            inflated.contains("{4,1,{0,5b87ad1b-d8cc-43c1-b5c4-dc43613c518c},\"\",-1,-1,1,0,\"\"}")
        );
        assert!(!inflated.contains("StdPicture.InformationRegister"));
    }

    #[test]
    fn rejects_common_command_multiple_parameter_types() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,dddddddd-dddd-4ddd-dddd-dddddddddddd,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,{0},"",-1,-1,1,0,""},3,
{0},1,
{0,0,0},0,
{1,77ea1b8f-dd79-4717-9dba-5628e7f348cf},
{"Pattern"},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldCommand",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},0,0,0}
}
},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommonCommand uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewCommand</Name>
      <Synonym/>
      <Comment/>
      <Group>FormCommandBarImportant</Group>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture/>
      <Shortcut/>
      <IncludeHelpInContents>false</IncludeHelpInContents>
      <CommandParameterType>
        <v8:TypeSet>cfg:DefinedType.TestOwner</v8:TypeSet>
        <v8:TypeSet>cfg:DefinedType.TestOwner</v8:TypeSet>
      </CommandParameterType>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#
        .as_bytes();

        let error = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("only single CommonCommand CommandParameterType TypeSet is supported"),
            "{error}"
        );
    }

    #[test]
    fn patches_common_module_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{12,
{3,
{1,0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},"OldName",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0,1,0,0,0,0,0,1},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <CommonModule uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>NewName</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>New &quot;quoted&quot; synonym</v8:content>
        </v8:item>
        <v8:item>
          <v8:lang>en</v8:lang>
          <v8:content>English</v8:content>
        </v8:item>
      </Synonym>
      <Comment>New comment</Comment>
      <Global>true</Global>
      <ClientManagedApplication>false</ClientManagedApplication>
      <Server>true</Server>
      <ExternalConnection>true</ExternalConnection>
      <ClientOrdinaryApplication>false</ClientOrdinaryApplication>
      <ServerCall>false</ServerCall>
      <Privileged>true</Privileged>
      <ReturnValuesReuse>DuringSession</ReturnValuesReuse>
    </Properties>
  </CommonModule>
</MetaDataObject>
"#;

        let packed = super::pack_common_module_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert_eq!(
            packed.properties.synonyms[0].content,
            "New \"quoted\" synonym"
        );
        assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        assert!(inflated.contains("\"NewName\""));
        assert!(inflated.contains("{2,\"ru\",\"New \"\"quoted\"\" synonym\",\"en\",\"English\"}"));
        assert!(inflated.contains("\"New comment\""));
        assert!(inflated.contains(",0,1,1,1,1,0,2,0}"));
    }

    #[test]
    fn resolves_additional_metadata_source_folders() {
        assert_eq!(
            super::metadata_type_source_folder("AccumulationRegisterObject.Sales"),
            Some("AccumulationRegisters")
        );
        assert_eq!(
            super::metadata_type_source_folder("AccountingRegisterManager.Entries"),
            Some("AccountingRegisters")
        );
        assert_eq!(
            super::metadata_type_source_folder("ChartOfCalculationTypesList.Payouts"),
            Some("ChartsOfCalculationTypes")
        );
        assert_eq!(
            super::metadata_type_source_folder("ChartOfCalculationRegistersRef.Accruals"),
            Some("ChartsOfCalculationRegisters")
        );
        assert_eq!(
            super::metadata_type_source_folder("CalculationRegisterObject.Premiums"),
            Some("CalculationRegisters")
        );
        assert_eq!(
            super::metadata_type_source_folder("ChartOfAccountsObject.Plan"),
            Some("ChartsOfAccounts")
        );
        assert_eq!(
            super::metadata_type_source_folder("BusinessProcessRoutePointRef.Sales"),
            Some("BusinessProcesses")
        );
        assert_eq!(
            super::metadata_type_source_folder("BusinessProcessObject.Sales"),
            Some("BusinessProcesses")
        );
        assert_eq!(
            super::metadata_type_source_folder("BusinessProcessRef.Sales"),
            Some("BusinessProcesses")
        );
        assert_eq!(
            super::metadata_type_source_folder("BusinessProcessSelection.Sales"),
            Some("BusinessProcesses")
        );
        assert_eq!(
            super::metadata_type_source_folder("BusinessProcessList.Sales"),
            Some("BusinessProcesses")
        );
        assert_eq!(
            super::metadata_type_source_folder("BusinessProcessManager.Sales"),
            Some("BusinessProcesses")
        );
        assert_eq!(
            super::metadata_type_source_folder("CatalogTabularSection.Goods.Items"),
            Some("Catalogs")
        );
        assert_eq!(
            super::metadata_type_source_folder("CommonCommand.АвтономнаяРабота"),
            Some("CommonCommands")
        );
        assert_eq!(
            super::metadata_type_source_folder("CommonPicture.Бот"),
            Some("CommonPictures")
        );
        assert_eq!(
            super::metadata_type_source_folder("CommonForm.АвтономнаяРабота"),
            Some("CommonForms")
        );
        assert_eq!(
            super::metadata_type_source_folder("Characteristic.Dimension"),
            Some("ChartsOfCharacteristicTypes")
        );
        assert_eq!(
            super::metadata_type_source_folder("DataProcessorTabularSection.Batch.Items"),
            Some("DataProcessors")
        );
        assert_eq!(
            super::metadata_type_source_folder("DocumentTabularSection.Invoice.Rows"),
            Some("Documents")
        );
        assert_eq!(
            super::metadata_type_source_folder("FilterCriterionList.Criteria"),
            Some("FilterCriteria")
        );
        assert_eq!(
            super::metadata_type_source_folder("InformationRegisterRecord.RegisterItem"),
            Some("InformationRegisters")
        );
        assert_eq!(
            super::metadata_type_source_folder("ConstantValueKey.SomeConstant"),
            Some("Constants")
        );
        assert_eq!(
            super::metadata_type_source_folder("CommandGroup.Органайзер"),
            Some("CommandGroups")
        );
        assert_eq!(
            super::metadata_type_source_folder("CommonTemplate.СтруктураПодчиненности"),
            Some("CommonTemplates")
        );
        assert_eq!(
            super::metadata_type_source_folder("SettingsStorageManager.Settings"),
            Some("SettingsStorages")
        );
        assert_eq!(
            super::metadata_type_source_folder("TaskObject.ExecutorTask"),
            Some("Tasks")
        );
        assert_eq!(
            super::metadata_type_source_folder("TaskRef.ExecutorTask"),
            Some("Tasks")
        );
        assert_eq!(
            super::metadata_type_source_folder("TaskSelection.ExecutorTask"),
            Some("Tasks")
        );
        assert_eq!(
            super::metadata_type_source_folder("TaskList.ExecutorTask"),
            Some("Tasks")
        );
        assert_eq!(
            super::metadata_type_source_folder("TaskManager.ExecutorTask"),
            Some("Tasks")
        );
    }

    #[test]
    fn resolves_metadata_references_from_lab_sources() {
        let source_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let source = MetadataSourceContext::new(source_root);

        for (reference, expected_uuid) in [
            (
                "Role.АдминистраторСистемы",
                "76702e9e-fa7a-4b98-befa-f9b37db2dae0",
            ),
            ("Language.Русский", "db4a9ccb-9ef5-4b3c-8577-b6fe5db1b62e"),
            (
                "CommonAttribute.ОтредактированныеПредопределенныеРеквизиты",
                "141c7b66-d689-4c8e-ace8-b8e1d8c7fbaa",
            ),
            (
                "Constant.АвтоматическиНастраиватьРазрешенияВПрофиляхБезопасности",
                "9893e2d6-f3f8-4d73-bb06-19bf26d216ab",
            ),
            (
                "CommonForm.ФормаОтчета",
                "9d6d77a9-1f55-4162-93a5-14bb3f3febaf",
            ),
            (
                "CommonForm.ФормаНастроекОтчета",
                "6106e958-354c-4211-992c-3c3819e8828e",
            ),
            (
                "CommonForm.ФормаВариантаОтчета",
                "1f55f330-7c0a-4a29-905f-51d664515bc5",
            ),
            (
                "CommonForm.АвтономнаяРабота",
                "1f3057c2-135f-44b2-9f86-34481fbc5596",
            ),
            (
                "ChartOfCharacteristicTypes.ДополнительныеРеквизитыИСведения",
                "1055d15b-8cb5-4ff0-a526-7fd20a08a96c",
            ),
            (
                "ChartOfCharacteristicTypes.ОбъектыАдресацииЗадач",
                "ad083c26-7461-4e94-b524-0174242fbd91",
            ),
            (
                "CommonTemplate.ВидыДокументовУдостоверяющихЛичность",
                "1682d528-87bf-48c5-acf9-57ab654a615a",
            ),
            (
                "CommonTemplate.СтруктураПодчиненности",
                "7a62d031-c340-4b1e-90af-fff697a2e979",
            ),
            (
                "ScheduledJob.ЗагрузкаКурсовВалют",
                "c7ffd8ab-15e9-4cf1-a7fd-d05534dff000",
            ),
            (
                "FunctionalOption.ВыполнятьЗамерыПроизводительности",
                "7f06703e-24cd-4db7-be88-3fbd65e5c252",
            ),
            (
                "FunctionalOptionsParameter.ОбщиеНастройкиУзлов",
                "f9479915-cdee-40d5-ba53-101132aac672",
            ),
            (
                "EventSubscription.ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных",
                "a64b15fa-fc34-43fe-a366-d27c0f1c3df2",
            ),
            (
                "FilterCriterion.СвязанныеДокументы",
                "18bf6916-83cc-41e5-a35b-1489450ae632",
            ),
            (
                "SettingsStorage.ХранилищеВариантовОтчетов",
                "14512818-58b0-44cc-b00d-d37913c57aad",
            ),
            (
                "Enum.ВариантыВажностиЗадачи",
                "c39750ca-e33f-40c2-b830-119423d9a2ae",
            ),
            (
                "Enum.ВариантыОтображенияМеток",
                "fd9177e4-1277-4f77-be3e-07e169fad918",
            ),
            (
                "SessionParameter.АвторизованныйПользователь",
                "5efc4bc4-b711-4620-8d2e-9d947c6cc141",
            ),
            (
                "DefinedType.Пользователь",
                "a72517c3-8c91-4e40-81ac-83c762789e87",
            ),
            (
                "Catalog.РолиИсполнителей",
                "45c0003f-0ed7-4582-b84e-217fdc4ddeaf",
            ),
            (
                "BusinessProcess.Задание",
                "dad11c2e-08fc-4a6b-8829-8be6c64c15fc",
            ),
            (
                "Document.ЭлектронноеПисьмоИсходящее",
                "8f2b8a8e-4cd3-45e3-89ae-1cc4bd0ff30a",
            ),
            (
                "InformationRegister.ДополнительныеСведения",
                "3ad5d8a7-3071-46aa-aebf-306bdb67983b",
            ),
            (
                "Report.БизнесПроцессы",
                "c5f91669-13d8-4f0a-a054-2701078da38a",
            ),
            (
                "Task.ЗадачаИсполнителя",
                "3ad08f4a-6202-4099-b6cc-bc116e6731a0",
            ),
            (
                "StyleItem.ВажнаяНадписьШрифт",
                "fa2a9ef2-00a1-44f4-a82c-6c7288dd62dc",
            ),
            (
                "HTTPService.exchange_dsl_1_0_0_1",
                "c09df096-f9cc-4b2f-a44e-69147339dc8c",
            ),
            (
                "WebService.EnterpriseDataUpload_1_0_1_1",
                "9ad3b432-5b49-44ee-9d8d-83c36458d927",
            ),
            (
                "WebService.RemoteControl",
                "03d08c14-f814-4e12-8f96-020c36cca2bf",
            ),
            (
                "XDTOPackage.АдминистрированиеОбменаДанными_2_4_5_1",
                "ac7ea771-4b10-4d43-9c0a-9cd36e4c49a4",
            ),
            (
                "DocumentJournal.Взаимодействия",
                "7da57c89-af2c-445a-96f7-39250f70306f",
            ),
            (
                "Subsystem.СтандартныеПодсистемы",
                "0421b67e-ed26-491d-ab98-ec59002ed4ce",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
            );
        }

        for (reference, expected_type_id) in [
            (
                "BusinessProcessObject.Задание",
                "4a670c5f-960b-4b36-b587-59bcea4d8449",
            ),
            (
                "BusinessProcessRef.Задание",
                "07d25a98-bdd8-4f7b-b87b-172294158755",
            ),
            (
                "BusinessProcessSelection.Задание",
                "d0447d5c-7808-4532-8a98-0cb3974a90bf",
            ),
            (
                "BusinessProcessList.Задание",
                "9c74798b-2430-4cda-97f2-44472b8d59ac",
            ),
            (
                "BusinessProcessManager.Задание",
                "9f615ee8-8711-4ca9-98d0-f0a258dcdfd2",
            ),
            (
                "BusinessProcessRoutePointRef.Задание",
                "35f39a4f-8a59-4b48-aa38-ef5f2640d375",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "CatalogObject.РолиИсполнителей",
                "ef072c81-fde0-4cf3-a449-5572c679b351",
            ),
            (
                "CatalogRef.РолиИсполнителей",
                "44422b6d-5eb8-49c6-856b-dd9009611933",
            ),
            (
                "CatalogSelection.РолиИсполнителей",
                "7a78b42f-4b88-4938-a59a-f1227ae3e4da",
            ),
            (
                "CatalogList.РолиИсполнителей",
                "26cbb04c-a1eb-45fc-8e62-cc3f010f34cd",
            ),
            (
                "CatalogManager.РолиИсполнителей",
                "28f405d9-0472-418a-a888-838dd917ced7",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "DataProcessorObject.АвтоматическоеИзвлечениеТекстов",
                "5db20d4f-615f-4911-9cd4-45ff4f623dd2",
            ),
            (
                "DataProcessorManager.АвтоматическоеИзвлечениеТекстов",
                "76b2ec7a-4ddb-4d50-aec2-4a1b6bb1b3b9",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "ReportObject.БизнесПроцессы",
                "1d3afecc-d10d-4795-a819-cadc3d5ecd95",
            ),
            (
                "ReportManager.БизнесПроцессы",
                "5a14c1b5-a349-4c68-aee7-d7b6b35e78a4",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "DocumentObject.АктОбУничтоженииПерсональныхДанных",
                "12576083-65c9-4698-a669-bd9dec07cc88",
            ),
            (
                "DocumentRef.АктОбУничтоженииПерсональныхДанных",
                "6851400e-2dbc-4f37-868b-a4683b097408",
            ),
            (
                "DocumentSelection.АктОбУничтоженииПерсональныхДанных",
                "54a7694c-e379-4c71-8e87-e8cd69ac617a",
            ),
            (
                "DocumentList.АктОбУничтоженииПерсональныхДанных",
                "300148b3-5f55-43a5-a9a6-0840f74a0c3e",
            ),
            (
                "DocumentManager.АктОбУничтоженииПерсональныхДанных",
                "d67de8ab-dd38-424a-aaea-753b75c3b7e8",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "ExchangePlanObject.ОбновлениеИнформационнойБазы",
                "0bcfe249-60b0-40fe-bf8e-531749953e91",
            ),
            (
                "ExchangePlanRef.ОбновлениеИнформационнойБазы",
                "4676cf0b-d6fd-4c39-a5f0-43da2d37c210",
            ),
            (
                "ExchangePlanSelection.ОбновлениеИнформационнойБазы",
                "0a64a383-8e2c-435e-a2fa-f8fc69fad418",
            ),
            (
                "ExchangePlanList.ОбновлениеИнформационнойБазы",
                "7559cd4f-0728-442e-9593-02e845cea7fd",
            ),
            (
                "ExchangePlanManager.ОбновлениеИнформационнойБазы",
                "80a1960a-e4be-445e-8446-cfb59885e83e",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "TaskObject.ЗадачаИсполнителя",
                "edccd440-4634-484c-b31d-443ba8674912",
            ),
            (
                "TaskRef.ЗадачаИсполнителя",
                "526f0ebe-d70d-4909-8ae9-86bbabfa55da",
            ),
            (
                "TaskSelection.ЗадачаИсполнителя",
                "a29971a7-e94c-4876-b4de-5ba996cfef0d",
            ),
            (
                "TaskList.ЗадачаИсполнителя",
                "a3849f5f-312a-4950-a52e-ee3f915b5490",
            ),
            (
                "TaskManager.ЗадачаИсполнителя",
                "cb17d3c0-8e58-4bed-aea8-a6cc40c5bd74",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "ChartOfCharacteristicTypesObject.ОбъектыАдресацииЗадач",
                "20630724-4f14-4d31-b479-6b01d7d318e0",
            ),
            (
                "ChartOfCharacteristicTypesRef.ОбъектыАдресацииЗадач",
                "48723115-46af-4d1f-8070-bc9ce5745356",
            ),
            (
                "ChartOfCharacteristicTypesSelection.ОбъектыАдресацииЗадач",
                "e676427d-6d38-4ea2-b363-157ba41d7156",
            ),
            (
                "ChartOfCharacteristicTypesList.ОбъектыАдресацииЗадач",
                "4f89d4d5-72f5-4640-9c43-e6c104c86198",
            ),
            (
                "Characteristic.ОбъектыАдресацииЗадач",
                "6357c29c-abbc-467d-961b-8ccb5be8c151",
            ),
            (
                "ChartOfCharacteristicTypesManager.ОбъектыАдресацииЗадач",
                "5b41e3e3-086e-45b2-b05c-aeae98d85834",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "EnumRef.ВажностьПроблемыУчета",
                "c8f0f421-adcf-417b-8509-93d4569c4435",
            ),
            (
                "EnumManager.ВажностьПроблемыУчета",
                "7b08935e-284b-4995-ae95-f93cc6666d02",
            ),
            (
                "EnumList.ВажностьПроблемыУчета",
                "001333d9-a79f-4306-900e-a56c9e37802f",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [(
            "SettingsStorageManager.ХранилищеВариантовОтчетов",
            "c78f03a8-fbb0-4d73-b78e-23dc1810a05c",
        )] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "InformationRegisterRecord.АдминистративнаяИерархия",
                "b849da51-b14a-4348-87e7-9ba778ef267d",
            ),
            (
                "InformationRegisterManager.АдминистративнаяИерархия",
                "f91e151b-de23-41be-a03d-15c69393c1c3",
            ),
            (
                "InformationRegisterSelection.АдминистративнаяИерархия",
                "2f2b5932-32e0-411b-accc-79d663c5308c",
            ),
            (
                "InformationRegisterList.АдминистративнаяИерархия",
                "b957dd6a-b02c-4096-8a02-4ca35d78a3b3",
            ),
            (
                "InformationRegisterRecordSet.АдминистративнаяИерархия",
                "ef23c5b7-2a2b-4573-a996-b7e9b0c719c9",
            ),
            (
                "InformationRegisterRecordKey.АдминистративнаяИерархия",
                "139107d1-4583-43ef-8b20-283a3074458a",
            ),
            (
                "InformationRegisterRecordManager.АдминистративнаяИерархия",
                "f9f726d6-bd9a-4cf8-bb57-ee742ca0fad4",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "InformationRegisterRecord.АдресныеОбъекты",
                "9ea90ec5-ab70-486e-8cc5-1707d8e5998e",
            ),
            (
                "InformationRegisterManager.АдресныеОбъекты",
                "28648720-16ff-487f-8f79-68a1480055bd",
            ),
            (
                "InformationRegisterSelection.АдресныеОбъекты",
                "f8538bed-cc51-4c92-a18e-7b5a933a1025",
            ),
            (
                "InformationRegisterList.АдресныеОбъекты",
                "aba286a3-0324-4840-92f2-7edc3980054e",
            ),
            (
                "InformationRegisterRecordSet.АдресныеОбъекты",
                "d7d6d986-6bec-45a0-b3ec-e29f286da38c",
            ),
            (
                "InformationRegisterRecordKey.АдресныеОбъекты",
                "273accea-5bfd-4163-9a96-86f4995ef650",
            ),
            (
                "InformationRegisterRecordManager.АдресныеОбъекты",
                "f9342f93-5225-4459-b1fe-d2553b85a5af",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_uuid) in [
            (
                "CommonPicture.Предупреждение",
                "ac2e5217-aaeb-4b6f-b063-538c84f2da06",
            ),
            (
                "CommonPicture.Взаимодействия",
                "44cf6d0a-0a5b-4ca1-b91e-af61f40fb825",
            ),
        ] {
            assert_eq!(
                source.resolve_common_picture_uuid(reference).unwrap(),
                expected_uuid
            );
        }

        for (reference, expected_uuid) in [
            (
                "CommonCommand.АвтономнаяРабота",
                "75ffd0b9-79be-4600-a310-591fddb6d63e",
            ),
            (
                "CommandGroup.Органайзер",
                "c59e11f3-6bcb-404a-9d76-1416c12be354",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
            );
        }

        for (reference, expected_uuid) in [
            (
                "CommandGroup.Взаимодействия",
                "e4842271-4fc0-4e15-afef-876f05af78c0",
            ),
            (
                "CommandGroup.Информация",
                "31ee6430-b65d-42fa-859b-c4f1c40686ae",
            ),
            (
                "CommandGroup.Органайзер",
                "c59e11f3-6bcb-404a-9d76-1416c12be354",
            ),
        ] {
            assert_eq!(
                source.resolve_command_group_uuid(reference).unwrap(),
                expected_uuid
            );
        }
    }

    #[test]
    fn resolves_additional_metadata_type_ids_from_synthetic_sources() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-module-types-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("Catalogs")).unwrap();
        std::fs::create_dir_all(root.join("AccumulationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("AccountingRegisters")).unwrap();
        std::fs::create_dir_all(root.join("CalculationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfAccounts")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCharacteristicTypes")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationTypes")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("BusinessProcesses")).unwrap();
        std::fs::create_dir_all(root.join("DataProcessors")).unwrap();
        std::fs::create_dir_all(root.join("Documents")).unwrap();
        std::fs::create_dir_all(root.join("ExchangePlans")).unwrap();
        std::fs::create_dir_all(root.join("Enums")).unwrap();
        std::fs::create_dir_all(root.join("InformationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("Reports")).unwrap();

        std::fs::write(
            root.join("Catalogs/РолиИсполнителей.Товары.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Catalog uuid="20202020-2020-4202-8202-202020202020">
    <Properties>
      <Name>РолиИсполнителей</Name>
      <GeneratedTypes>
        <GeneratedType name="CatalogTabularSection.РолиИсполнителей.Товары">
          <TypeId>21212121-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Catalog>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Catalogs/РолиИсполнителей.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Catalog uuid="20202020-2020-4202-8202-202020202020">
    <Properties>
      <Name>РолиИсполнителей</Name>
      <GeneratedTypes>
        <GeneratedType name="CatalogRef.РолиИсполнителей">
          <TypeId>22222222-2222-4222-8222-222222222222</TypeId>
        </GeneratedType>
        <GeneratedType name="CatalogSelection.РолиИсполнителей">
          <TypeId>23232323-2323-4232-8232-232323232323</TypeId>
        </GeneratedType>
        <GeneratedType name="CatalogList.РолиИсполнителей">
          <TypeId>24242424-2424-4242-8242-242424242424</TypeId>
        </GeneratedType>
        <GeneratedType name="CatalogManager.РолиИсполнителей">
          <TypeId>25252525-2525-4252-8252-252525252525</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Catalog>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccumulationRegisters/Продажи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccumulationRegister uuid="11111111-1111-4111-8111-111111111111">
    <Properties>
      <Name>Продажи</Name>
      <GeneratedTypes>
        <GeneratedType name="AccumulationRegisterObject.Продажи">
          <TypeId>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </AccumulationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccountingRegisters/Хозрасчеты.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccountingRegister uuid="22222222-2222-4222-8222-222222222222">
    <Properties>
      <Name>Хозрасчеты</Name>
      <GeneratedTypes>
        <GeneratedType name="AccountingRegisterManager.Хозрасчеты">
          <TypeId>bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </AccountingRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CalculationRegisters/Премии.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <CalculationRegister uuid="33333333-3333-4333-8333-333333333333">
    <Properties>
      <Name>Премии</Name>
      <GeneratedTypes>
        <GeneratedType name="CalculationRegisterList.Премии">
          <TypeId>cccccccc-cccc-4ccc-8ccc-cccccccccccc</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </CalculationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfAccounts uuid="44444444-4444-4444-8444-444444444444">
    <Properties>
      <Name>ПланСчетов</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfAccountsObject.ПланСчетов">
          <TypeId>dddddddd-dddd-4ddd-8ddd-dddddddddddd</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfAccounts>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач.Товары.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCharacteristicTypes uuid="1b1b1b1b-1b1b-41b1-81b1-1b1b1b1b1b1b">
    <Properties>
      <Name>ОбъектыАдресацииЗадач</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfCharacteristicTypesTabularSection.ОбъектыАдресацииЗадач.Товары">
          <TypeId>1c1c1c1c-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
        <GeneratedType name="ChartOfCharacteristicTypesTabularSectionRow.ОбъектыАдресацииЗадач.Товары">
          <TypeId>1d1d1d1d-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfCharacteristicTypes>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationTypes/ВидыРасчета.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationTypes uuid="55555555-5555-4555-8555-555555555555">
    <Properties>
      <Name>ВидыРасчета</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfCalculationTypesSelection.ВидыРасчета">
          <TypeId>eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee</TypeId>
        </GeneratedType>
        <GeneratedType name="ChartOfCalculationTypesObject.ВидыРасчета">
          <TypeId>eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeea</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfCalculationTypes>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationRegisters/Начисления.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationRegisters uuid="66666666-6666-4666-8666-666666666666">
    <Properties>
      <Name>Начисления</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfCalculationRegistersManager.Начисления">
          <TypeId>ffffffff-ffff-4fff-8fff-ffffffffffff</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfCalculationRegisters>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("BusinessProcesses/Задание.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <BusinessProcess uuid="15151515-1515-4151-8151-151515151515">
    <Properties>
      <Name>Задание</Name>
      <GeneratedTypes>
        <GeneratedType name="BusinessProcessRef.Задание">
          <TypeId>11111111-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </BusinessProcess>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("DataProcessors/АвтоматическоеИзвлечениеТекстов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <DataProcessor uuid="18181818-1818-4181-8181-181818181818">
    <Properties>
      <Name>АвтоматическоеИзвлечениеТекстов</Name>
      <GeneratedTypes>
        <GeneratedType name="DataProcessorManager.АвтоматическоеИзвлечениеТекстов">
          <TypeId>22222222-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </DataProcessor>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Documents/АктОбУничтоженииПерсональныхДанных.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Document uuid="14141414-1414-4141-8141-141414141414">
    <Properties>
      <Name>АктОбУничтоженииПерсональныхДанных</Name>
      <GeneratedTypes>
        <GeneratedType name="DocumentRef.АктОбУничтоженииПерсональныхДанных">
          <TypeId>26262626-2626-4262-8262-262626262626</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentSelection.АктОбУничтоженииПерсональныхДанных">
          <TypeId>27272727-2727-4272-8272-272727272727</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentList.АктОбУничтоженииПерсональныхДанных">
          <TypeId>28282828-2828-4282-8282-282828282828</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentManager.АктОбУничтоженииПерсональныхДанных">
          <TypeId>29292929-2929-4292-8292-292929292929</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentTabularSection.АктОбУничтоженииПерсональныхДанных.Товары">
          <TypeId>1e1e1e1e-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentObject.АктОбУничтоженииПерсональныхДанных">
          <TypeId>33333333-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Document>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Documents/АктОбУничтоженииПерсональныхДанных.Товары.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Document uuid="14141414-1414-4141-8141-141414141414">
    <Properties>
      <Name>АктОбУничтоженииПерсональныхДанных</Name>
      <GeneratedTypes>
        <GeneratedType name="DocumentTabularSection.АктОбУничтоженииПерсональныхДанных.Товары">
          <TypeId>1e1e1e1e-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentObject.АктОбУничтоженииПерсональныхДанных">
          <TypeId>33333333-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Document>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Enums/ВариантыВажностиЗадачи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Enum uuid="19191919-1919-4191-8191-191919191919">
    <Properties>
      <Name>ВариантыВажностиЗадачи</Name>
      <GeneratedTypes>
        <GeneratedType name="EnumRef.ВариантыВажностиЗадачи">
          <TypeId>30303030-3030-4303-8303-303030303030</TypeId>
        </GeneratedType>
        <GeneratedType name="EnumList.ВариантыВажностиЗадачи">
          <TypeId>31313131-3131-4313-8313-313131313131</TypeId>
        </GeneratedType>
        <GeneratedType name="EnumManager.ВариантыВажностиЗадачи">
          <TypeId>32323232-3232-4323-8323-323232323232</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Enum>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("InformationRegisters/АдминистративнаяИерархия.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <InformationRegister uuid="33333333-3333-4333-8333-333333333334">
    <Properties>
      <Name>АдминистративнаяИерархия</Name>
      <GeneratedTypes>
        <GeneratedType name="InformationRegisterRecord.АдминистративнаяИерархия">
          <TypeId>33333333-3333-4333-8333-333333333331</TypeId>
        </GeneratedType>
        <GeneratedType name="InformationRegisterRecordSet.АдминистративнаяИерархия">
          <TypeId>33333333-3333-4333-8333-333333333332</TypeId>
        </GeneratedType>
        <GeneratedType name="InformationRegisterRecordKey.АдминистративнаяИерархия">
          <TypeId>33333333-3333-4333-8333-333333333333</TypeId>
        </GeneratedType>
        <GeneratedType name="InformationRegisterRecordManager.АдминистративнаяИерархия">
          <TypeId>33333333-3333-4333-8333-333333333335</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </InformationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ExchangePlans/ОбновлениеИнформационнойБазы.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ExchangePlan uuid="16161616-1616-4161-8161-161616161616">
    <Properties>
      <Name>ОбновлениеИнформационнойБазы</Name>
      <GeneratedTypes>
        <GeneratedType name="ExchangePlanObject.ОбновлениеИнформационнойБазы">
          <TypeId>44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ExchangePlan>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Reports/БизнесПроцессы.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Report uuid="17171717-1717-4171-8171-171717171717">
    <Properties>
      <Name>БизнесПроцессы</Name>
      <GeneratedTypes>
        <GeneratedType name="ReportObject.БизнесПроцессы">
          <TypeId>55555555-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Report>
</MetaDataObject>
"#,
        )
        .unwrap();

        let source = MetadataSourceContext::new(root);
        for (reference, expected_type_id) in [
            (
                "AccumulationRegisterObject.Продажи",
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "AccountingRegisterManager.Хозрасчеты",
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            ),
            (
                "CalculationRegisterList.Премии",
                "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            ),
            (
                "ChartOfAccountsObject.ПланСчетов",
                "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            ),
            (
                "ChartOfCalculationTypesSelection.ВидыРасчета",
                "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
            ),
            (
                "ChartOfCalculationTypesObject.ВидыРасчета",
                "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeea",
            ),
            (
                "ChartOfCalculationRegistersManager.Начисления",
                "ffffffff-ffff-4fff-8fff-ffffffffffff",
            ),
            (
                "CatalogTabularSection.РолиИсполнителей.Товары",
                "21212121-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CatalogRef.РолиИсполнителей",
                "22222222-2222-4222-8222-222222222222",
            ),
            (
                "CatalogSelection.РолиИсполнителей",
                "23232323-2323-4232-8232-232323232323",
            ),
            (
                "CatalogList.РолиИсполнителей",
                "24242424-2424-4242-8242-242424242424",
            ),
            (
                "CatalogManager.РолиИсполнителей",
                "25252525-2525-4252-8252-252525252525",
            ),
            (
                "ChartOfCharacteristicTypesTabularSection.ОбъектыАдресацииЗадач.Товары",
                "1c1c1c1c-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ChartOfCharacteristicTypesTabularSectionRow.ОбъектыАдресацииЗадач.Товары",
                "1d1d1d1d-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "BusinessProcessRef.Задание",
                "11111111-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DataProcessorManager.АвтоматическоеИзвлечениеТекстов",
                "22222222-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DocumentObject.АктОбУничтоженииПерсональныхДанных",
                "33333333-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DocumentTabularSection.АктОбУничтоженииПерсональныхДанных.Товары",
                "1e1e1e1e-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DocumentRef.АктОбУничтоженииПерсональныхДанных",
                "26262626-2626-4262-8262-262626262626",
            ),
            (
                "DocumentSelection.АктОбУничтоженииПерсональныхДанных",
                "27272727-2727-4272-8272-272727272727",
            ),
            (
                "DocumentList.АктОбУничтоженииПерсональныхДанных",
                "28282828-2828-4282-8282-282828282828",
            ),
            (
                "DocumentManager.АктОбУничтоженииПерсональныхДанных",
                "29292929-2929-4292-8292-292929292929",
            ),
            (
                "ExchangePlanObject.ОбновлениеИнформационнойБазы",
                "44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "EnumRef.ВариантыВажностиЗадачи",
                "30303030-3030-4303-8303-303030303030",
            ),
            (
                "EnumList.ВариантыВажностиЗадачи",
                "31313131-3131-4313-8313-313131313131",
            ),
            (
                "EnumManager.ВариантыВажностиЗадачи",
                "32323232-3232-4323-8323-323232323232",
            ),
            (
                "InformationRegisterRecord.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333331",
            ),
            (
                "InformationRegisterRecordSet.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333332",
            ),
            (
                "InformationRegisterRecordKey.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333333",
            ),
            (
                "InformationRegisterRecordManager.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333335",
            ),
            (
                "ReportObject.БизнесПроцессы",
                "55555555-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }
    }

    #[test]
    fn resolves_additional_metadata_references_from_synthetic_sources() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-module-refs-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        std::fs::create_dir_all(root.join("AccumulationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("AccountingRegisters")).unwrap();
        std::fs::create_dir_all(root.join("CalculationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("Catalogs")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfAccounts")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationTypes")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("Documents")).unwrap();
        std::fs::create_dir_all(root.join("Enums")).unwrap();
        std::fs::create_dir_all(root.join("InformationRegisters")).unwrap();

        std::fs::write(
            root.join("AccumulationRegisters/Продажи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccumulationRegister uuid="11111111-1111-4111-8111-111111111111">
    <Properties>
      <Name>Продажи</Name>
    </Properties>
  </AccumulationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccountingRegisters/Хозрасчеты.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccountingRegister uuid="22222222-2222-4222-8222-222222222222">
    <Properties>
      <Name>Хозрасчеты</Name>
    </Properties>
  </AccountingRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Catalogs/РолиИсполнителей.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Catalog uuid="20202020-2020-4202-8202-202020202020">
    <Properties>
      <Name>РолиИсполнителей</Name>
    </Properties>
  </Catalog>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CalculationRegisters/Премии.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <CalculationRegister uuid="33333333-3333-4333-8333-333333333333">
    <Properties>
      <Name>Премии</Name>
    </Properties>
  </CalculationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfAccounts uuid="44444444-4444-4444-8444-444444444444">
    <Properties>
      <Name>ПланСчетов</Name>
    </Properties>
  </ChartOfAccounts>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationTypes/ВидыРасчета.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationTypes uuid="55555555-5555-4555-8555-555555555555">
    <Properties>
      <Name>ВидыРасчета</Name>
    </Properties>
  </ChartOfCalculationTypes>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationRegisters/Начисления.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationRegisters uuid="66666666-6666-4666-8666-666666666666">
    <Properties>
      <Name>Начисления</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfCalculationRegistersManager.Начисления">
          <TypeId>ffffffff-ffff-4fff-8fff-ffffffffffff</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfCalculationRegisters>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Documents/АктОбУничтоженииПерсональныхДанных.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Document uuid="14141414-1414-4141-8141-141414141414">
    <Properties>
      <Name>АктОбУничтоженииПерсональныхДанных</Name>
    </Properties>
  </Document>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Enums/ВариантыВажностиЗадачи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Enum uuid="19191919-1919-4191-8191-191919191919">
    <Properties>
      <Name>ВариантыВажностиЗадачи</Name>
    </Properties>
  </Enum>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("InformationRegisters/АдминистративнаяИерархия.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <InformationRegister uuid="33333333-3333-4333-8333-333333333334">
    <Properties>
      <Name>АдминистративнаяИерархия</Name>
    </Properties>
  </InformationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();

        let source = MetadataSourceContext::new(root);
        for (reference, expected_uuid) in [
            (
                "AccumulationRegister.Продажи",
                "11111111-1111-4111-8111-111111111111",
            ),
            (
                "AccountingRegister.Хозрасчеты",
                "22222222-2222-4222-8222-222222222222",
            ),
            (
                "CalculationRegister.Премии",
                "33333333-3333-4333-8333-333333333333",
            ),
            (
                "Catalog.РолиИсполнителей",
                "20202020-2020-4202-8202-202020202020",
            ),
            (
                "ChartOfAccounts.ПланСчетов",
                "44444444-4444-4444-8444-444444444444",
            ),
            (
                "ChartOfCalculationTypes.ВидыРасчета",
                "55555555-5555-4555-8555-555555555555",
            ),
            (
                "ChartOfCalculationRegisters.Начисления",
                "66666666-6666-4666-8666-666666666666",
            ),
            (
                "Document.АктОбУничтоженииПерсональныхДанных",
                "14141414-1414-4141-8141-141414141414",
            ),
            (
                "Enum.ВариантыВажностиЗадачи",
                "19191919-1919-4191-8191-191919191919",
            ),
            (
                "InformationRegister.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333334",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
            );
        }
    }

    #[test]
    fn resolves_simple_metadata_references_from_synthetic_sources() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-simple-refs-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));

        let cases = [
            (
                "CommonAttributes",
                "ОтредактированныеПредопределенныеРеквизиты",
                "CommonAttribute",
                "41414141-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CommonForms",
                "ФормаОтчета",
                "CommonForm",
                "42424242-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CommonTemplates",
                "СтруктураПодчиненности",
                "CommonTemplate",
                "43434343-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "EventSubscriptions",
                "ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных",
                "EventSubscription",
                "44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "FunctionalOptions",
                "ВыполнятьЗамерыПроизводительности",
                "FunctionalOption",
                "45454545-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "FunctionalOptionsParameters",
                "ОбщиеНастройкиУзлов",
                "FunctionalOptionsParameter",
                "46464646-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "HTTPServices",
                "exchange_dsl_1_0_0_1",
                "HTTPService",
                "47474747-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Languages",
                "Русский",
                "Language",
                "48484848-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Roles",
                "АдминистраторСистемы",
                "Role",
                "49494949-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ScheduledJobs",
                "ЗагрузкаКурсовВалют",
                "ScheduledJob",
                "4a4a4a4a-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "SettingsStorages",
                "ХранилищеВариантовОтчетов",
                "SettingsStorage",
                "4b4b4b4b-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "StyleItems",
                "ВажнаяНадписьШрифт",
                "StyleItem",
                "4c4c4c4c-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Subsystems",
                "СтандартныеПодсистемы",
                "Subsystem",
                "4d4d4d4d-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Tasks",
                "ЗадачаИсполнителя",
                "Task",
                "4e4e4e4e-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "WebServices",
                "RemoteControl",
                "WebService",
                "4f4f4f4f-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "XDTOPackages",
                "АдминистрированиеОбменаДанными_2_4_5_1",
                "XDTOPackage",
                "50505050-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "BusinessProcesses",
                "Задание",
                "BusinessProcess",
                "51515151-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DataProcessors",
                "АвтоматическоеИзвлечениеТекстов",
                "DataProcessor",
                "52525252-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ExchangePlans",
                "ОбновлениеИнформационнойБазы",
                "ExchangePlan",
                "53535353-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Reports",
                "БизнесПроцессы",
                "Report",
                "54545454-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
        ];

        for (folder, name, kind, uuid) in cases {
            std::fs::create_dir_all(root.join(folder)).unwrap();
            std::fs::write(
                root.join(folder).join(format!("{name}.xml")),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <{kind} uuid="{uuid}">
    <Properties>
      <Name>{name}</Name>
    </Properties>
  </{kind}>
</MetaDataObject>
"#
                ),
            )
            .unwrap();
        }

        let source = MetadataSourceContext::new(root);
        for (reference, expected_uuid) in [
            (
                "CommonAttribute.ОтредактированныеПредопределенныеРеквизиты",
                "41414141-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CommonForm.ФормаОтчета",
                "42424242-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CommonTemplate.СтруктураПодчиненности",
                "43434343-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "EventSubscription.ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных",
                "44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "FunctionalOption.ВыполнятьЗамерыПроизводительности",
                "45454545-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "FunctionalOptionsParameter.ОбщиеНастройкиУзлов",
                "46464646-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "HTTPService.exchange_dsl_1_0_0_1",
                "47474747-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            ("Language.Русский", "48484848-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
            (
                "Role.АдминистраторСистемы",
                "49494949-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ScheduledJob.ЗагрузкаКурсовВалют",
                "4a4a4a4a-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "SettingsStorage.ХранилищеВариантовОтчетов",
                "4b4b4b4b-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "StyleItem.ВажнаяНадписьШрифт",
                "4c4c4c4c-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Subsystem.СтандартныеПодсистемы",
                "4d4d4d4d-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Task.ЗадачаИсполнителя",
                "4e4e4e4e-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "WebService.RemoteControl",
                "4f4f4f4f-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "XDTOPackage.АдминистрированиеОбменаДанными_2_4_5_1",
                "50505050-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "BusinessProcess.Задание",
                "51515151-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DataProcessor.АвтоматическоеИзвлечениеТекстов",
                "52525252-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ExchangePlan.ОбновлениеИнформационнойБазы",
                "53535353-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Report.БизнесПроцессы",
                "54545454-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
            );
        }
    }

    #[test]
    fn patches_versions_uuids_without_changing_text_length() {
        let mut text = "\u{feff}{1,2,\"\",aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,\"root\",bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,\"file.0\",cccccccc-cccc-4ccc-cccc-cccccccccccc}".to_string();
        let original_len = text.len();

        let header = super::replace_header_uuid(&mut text).unwrap();
        let root = super::replace_named_uuid(&mut text, "root").unwrap();
        let file = super::replace_named_uuid(&mut text, "file.0").unwrap();

        assert_eq!(text.len(), original_len);
        assert_eq!(header.old_uuid, "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa");
        assert_eq!(root.old_uuid, "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
        assert_eq!(file.old_uuid, "cccccccc-cccc-4ccc-cccc-cccccccccccc");
        assert!(super::is_uuid_text(&header.new_uuid));
        assert!(super::is_uuid_text(&root.new_uuid));
        assert!(super::is_uuid_text(&file.new_uuid));
    }
}
