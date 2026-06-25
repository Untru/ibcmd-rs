use std::collections::BTreeMap;
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

    #[cfg(test)]
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

    #[cfg(test)]
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

#[derive(Debug, Clone, Copy)]
enum CommonCommandRepresentation {
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
        "ActionsPanel" => Ok(CommandGroupCategory::ActionsPanel),
        "FormCommandBar" => Ok(CommandGroupCategory::FormCommandBar),
        other => Err(anyhow!("unsupported CommandGroup Category: {other}")),
    }
}

fn common_command_representation_code(value: CommonCommandRepresentation) -> u8 {
    match value {
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
        "SettingsStorageManager" => Some("SettingsStorages"),
        "ReportObject" | "ReportManager" => Some("Reports"),
        "TaskObject" | "TaskRef" | "TaskSelection" | "TaskList" | "TaskManager" => Some("Tasks"),
        _ => None,
    }
}

#[cfg(test)]
fn metadata_reference_source_folder(reference: &str) -> Option<(&'static str, &'static str)> {
    let prefix = reference.split_once('.')?.0;
    match prefix {
        "CommonAttribute" => Some(("CommonAttribute", "CommonAttributes")),
        "EventSubscription" => Some(("EventSubscription", "EventSubscriptions")),
        "FilterCriterion" => Some(("FilterCriterion", "FilterCriteria")),
        "FunctionalOption" => Some(("FunctionalOption", "FunctionalOptions")),
        "FunctionalOptionsParameter" => {
            Some(("FunctionalOptionsParameter", "FunctionalOptionsParameters"))
        }
        "HTTPService" => Some(("HTTPService", "HTTPServices")),
        "Language" => Some(("Language", "Languages")),
        "Role" => Some(("Role", "Roles")),
        "ScheduledJob" => Some(("ScheduledJob", "ScheduledJobs")),
        "SettingsStorage" => Some(("SettingsStorage", "SettingsStorages")),
        "StyleItem" => Some(("StyleItem", "StyleItems")),
        "WebService" => Some(("WebService", "WebServices")),
        "XDTOPackage" => Some(("XDTOPackage", "XDTOPackages")),
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
        DEFAULT_INFO, MetadataSourceContext, build_module_inner, deflate_raw, inflate_raw,
        parse_v8_container,
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
    fn patches_command_group_metadata_blob_from_xml() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            r#"{1,
{3,
{4,0,{0},"",-1,-1,1,0,""},4,3,
{0},
{0},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldGroup",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0}
},0}"#
                .as_bytes(),
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
            super::metadata_type_source_folder("BusinessProcessRoutePointRef.Sales"),
            Some("BusinessProcesses")
        );
        assert_eq!(
            super::metadata_type_source_folder("CatalogTabularSection.Goods.Items"),
            Some("Catalogs")
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
            super::metadata_type_source_folder("SettingsStorageManager.Settings"),
            Some("SettingsStorages")
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
                "XDTOPackage.АдминистрированиеОбменаДанными_2_4_5_1",
                "ac7ea771-4b10-4d43-9c0a-9cd36e4c49a4",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
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
