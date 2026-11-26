//! The packers of the stored bodies a 1C configuration holds, from their
//! XML sources: metadata objects, forms (over the native form body writer,
//! the `ibcmd-form-writer` crate), templates, rights, styles, pictures,
//! help, predefined data, exchange plan content, flowcharts, schedules and
//! the rest of what `cf load` and the base-free compiler pack, with the
//! source context they read ([`MetadataSourceContext`]), the role-rights
//! writer they call ([`role_rights_writer`]) and the metadata XML element
//! tree ([`xml`]). The form writer's modules and items keep their paths
//! here through re-exports: [`dynamic_list`], [`form_chart`],
//! [`form_planner`]. The base-free writers of the command-interface family
//! ([`pack_interface_asset_blob`], [`interface_asset_plaintext`] and the
//! command-interface entry points) serialize through the codecs of
//! `ibcmd-compiler-families`.
//!
//! Everything here moved from the root crate (`src/module_blob.rs` with its
//! `interface_assets.rs`, `src/compiler/bodies/` and
//! `src/metadata_model/xml.rs`), which keeps every path through re-exports
//! (`module_blob::…`, `compiler::bodies::…`, `metadata_model::xml`). The
//! root keeps the entry points that read the rest of it (the exporter, the
//! command-line arguments, the moxel writer and the command-interface
//! packer that patches a base blob) and the tests that do; what those read
//! here is `pub`, and `#[doc(hidden)]` where it was private.
//!
//! The code came from the physical-adapter guard's slice, so the guard scans
//! every non-test `*.rs` file under this crate's `src/`
//! (`$GuardedCrateDirectories` in `tools/validate-physical-adapter-policy.ps1`)
//! and their literals stay in the guard's baseline. A file added here is in
//! the slice at once and fails the guard until its record is reviewed.

mod interface_assets;
pub mod role_rights_writer;
pub mod xml;

#[doc(hidden)]
pub use interface_assets::plaintext_through;
pub use interface_assets::{
    InterfaceAssetSource, command_interface_base_free_blockers,
    command_interface_xml_can_pack_without_base, interface_asset_plaintext,
    pack_command_interface_blob_from_xml_base_free, pack_interface_asset_blob,
};

#[cfg(test)]
mod source_seam_tests;

// The native form writer is the `ibcmd-form-writer` crate's: its modules
// and public items keep their paths here, and the packers call the
// helpers they share with it by these names.
use ibcmd_form_writer::{
    ALWAYS_USED_CONSTANTS_CLEAR, ALWAYS_USED_CONSTANTS_SOURCE, MetadataTypePatternElement,
    NATIVE_ZERO_UUID, StringAllowedLengthCoding, common_command_group_uuid, compact_1c_value,
    format_1c_synonyms, format_metadata_type_pattern, format_metadata_type_pattern_element,
    format_schedule_date, format_schedule_time, format_style_body_color_value,
    one_c_values_equal_ignoring_ws, pack_form_body_blob_from_form_xml_base_free_through,
    pack_form_body_blob_from_form_xml_through, parse_metadata_type_pattern_elements,
    parse_optional_xml_bool, parse_xml_bool_text, path_ends_with, style_body_key_for_name,
    style_body_ref_key, style_body_standard_code_for_name, xml_attrs_map,
};
pub use ibcmd_form_writer::{
    FORM_DYNAMIC_LIST_TYPE_UUID, FormXmlAttribute, FormXmlBodyProperties,
    FormXmlDynamicListSettings, FormXmlListSettings, FormXmlListSettingsOrder, LocalizedString,
    NATIVE_EMPTY_SETTINGS, STD_PICTURE_VALUES, XML_2_21_TREE_IN_LAYOUT_8_3,
    compile_native_form_body_8_3, form_base_free_model_blockers, native_item_color,
    native_picture_from_parts, parse_form_body_blob, parse_form_xml_body_properties,
    validate_form_xml_document, xml_local_name,
};
pub use ibcmd_form_writer::{dynamic_list, form_chart, form_planner};
// What the tests here read of the writer.
#[cfg(test)]
use ibcmd_form_writer::{
    FormXmlCommandInterfaceItem, FormXmlRights, FormXmlTablePeriod, FormXmlUseForFoldersAndItems,
    SourceConfigurationObjects, form_layout_child_item_pairs_tail_start,
    form_layout_child_items_tail_start, native_choice_list_reference,
    native_command_interface_command, native_object_standard_command, native_rights,
};

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use ibcmd_1c_text::{crlf_line_breaks, format_1c_string, is_uuid_like_ascii};
use ibcmd_form_native::{ConfigurationField, ConfigurationObject, ConfigurationObjects};
use quick_xml::Reader;
use quick_xml::escape::{resolve_xml_entity, unescape};
use quick_xml::events::{BytesStart, Event};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::role_rights_writer::SourceTreeRoleRightsSource;

pub(crate) use ibcmd_1c_text::{encode_base64, platform_base64_token};
// The module blob codec, the raw deflate helpers and digests, the
// raw-deflated packer and the versions-blob patcher are the
// `ibcmd-module-blob` crate's; the packers call them by these names.
pub use ibcmd_module_blob::{
    PackedModuleBlob, PackedRawDeflatedBlob, PatchedVersionsBlob, VersionReplacement, hex_sha256,
    module_blob_text_sha256, pack_module_blob_bytes, pack_module_blob_bytes_base_free,
    pack_module_blob_container_bytes, pack_raw_deflated_blob_from_bytes, patch_versions_blob_bytes,
    patch_versions_blob_bytes_allowing_additions, raw_deflated_first_base64_payload_sha256,
    raw_deflated_plain_sha256, unpack_module_blob_text, versions_base_free_blockers,
};
pub(crate) use ibcmd_module_blob::{deflate_raw, inflate_raw};
// The source bytes and the two answers the source context memoises are the
// `ibcmd-metadata-source` crate's; the packers and the root name them here.
pub use ibcmd_metadata_source::{HelpReference, InterfaceResolutionMemo, SourceBytes};
// The seam a writer reads the source tree through; the context implements it.
pub use ibcmd_metadata_source::MetadataSource;

// A form body record and its container revision are the `ibcmd-form-body`
// crate's (the form reader there reads them); this module unpacks them and
// keeps their public paths.
pub use ibcmd_form_body::ParsedFormBodyBlob;

pub(crate) use ibcmd_form_body::record_text::scan_braced_fields;
use ibcmd_form_body::record_text::{
    decode_base64_payload_field, parse_1c_quoted_string, scan_1c_quoted_string_end,
    scan_balanced_braces, trim_ascii_ws_range,
};

pub use ibcmd_form_body::FormBodyRevision;

// Platform-level 1C standard pictures and form type IDs, not database metadata UUIDs.
const STD_PICTURE_USER_UUID: &str = "6ff3ddbd-56e3-4ddf-a5bf-048c1e2dfb2f";
const STD_PICTURE_INFORMATION_REGISTER_UUID: &str = "5b87ad1b-d8cc-43c1-b5c4-dc43613c518c";
const STD_PICTURE_INFORMATION_UUID: &str = "4b54770b-d069-4c0e-9b17-5cc2a01134d9";
const STD_PICTURE_SAVE_FILE_UUID: &str = "818ab7d0-4654-4542-bd5e-fd9d1352b5a1";
const STD_PICTURE_LOAD_REPORT_SETTINGS_UUID: &str = "283ecabd-aaed-41d1-ad46-6cca91c29120";

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

#[derive(Debug, Serialize)]
pub struct VersionsBlobPatchReport {
    pub input: PathBuf,
    pub output: PathBuf,
    pub plain_bytes: usize,
    pub output_bytes: usize,
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

#[derive(Debug, Clone)]
pub struct SpreadsheetNumberFormatHint {
    pub slots: Vec<Vec<LocalizedString>>,
    pub format_slot_indices: Vec<Option<usize>>,
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
#[doc(hidden)]
pub struct RoleRightsXml {
    set_for_new_objects: bool,
    #[doc(hidden)]
    pub set_for_attributes_by_default: Option<bool>,
    #[doc(hidden)]
    pub independent_rights_of_child_objects: Option<bool>,
    #[doc(hidden)]
    pub objects: Vec<RoleObjectRightsXml>,
    #[doc(hidden)]
    pub restriction_templates: Vec<RoleRestrictionTemplateXml>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[doc(hidden)]
pub struct RoleObjectRightsXml {
    name: String,
    #[doc(hidden)]
    pub rights: Vec<RoleRightXml>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct RoleObjectRefKey {
    uuid: String,
    kind_code: Option<String>,
    slot_code: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[doc(hidden)]
pub struct RoleRightXml {
    name: String,
    #[doc(hidden)]
    pub value: bool,
    #[doc(hidden)]
    pub restriction_by_condition: Option<RoleRightRestrictionXml>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[doc(hidden)]
pub struct RoleRightRestrictionXml {
    field: Option<String>,
    condition: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[doc(hidden)]
pub struct RoleRestrictionTemplateXml {
    name: String,
    condition: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[doc(hidden)]
pub struct CommandInterfaceXmlEntry {
    name: String,
    #[doc(hidden)]
    pub common: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct ExchangePlanContentXmlItem {
    metadata: String,
    /// The stored code: `0` Deny, `1` Allow, `2` Auto (the exporter's reading).
    auto_record: u8,
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

#[derive(Debug, Clone, Copy, Default)]
struct PredefinedDataXmlStats {
    items: usize,
    root_items: usize,
    folders: usize,
    nested_child_links: usize,
    editable_value_fields: usize,
    non_empty_text_fields: usize,
    max_depth: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct FlowchartXmlItem {
    kind: String,
    id: String,
    name: String,
    tab_order: String,
    explanation: Option<String>,
    task_description: Option<String>,
    events: BTreeMap<String, Option<String>>,
}

/// The original source tree one operation holds open: the files it reads
/// are the copies captured when the tree was opened, and the tree is checked
/// for change before anything is published. A context built on one
/// (`MetadataSourceContext::with_original_source`) reads every source file
/// through it and never from the file system. The root's
/// `mssql_source_change::HeldSourceRoot` is the one implementation; the
/// methods are its own, by the same names.
pub trait OriginalSourceTree: std::fmt::Debug + Send + Sync {
    /// The canonical root the tree was opened at.
    fn canonical_root(&self) -> &Path;
    /// The captured bytes of `path`, a path under the root, or why there are
    /// none: the path is outside the root or the census, or the file changed
    /// while it was read.
    fn read_original_path(&self, path: &Path) -> Result<Arc<Vec<u8>>>;
    /// Fails when a file read so far, or the tree's census, changed since
    /// the tree was opened.
    fn require_unchanged(&self) -> Result<()>;
}

#[derive(Debug, Clone)]
pub struct MetadataSourceContext {
    source_root: PathBuf,
    /// Every metadata object a `<DataPath>` has asked for, parsed once.
    ///
    /// A field id is the field's own `uuid=`, so placing a dotted data path
    /// means reading the object's XML; a form names the same object from many
    /// paths and a run names it from many forms, so the answer is memoised
    /// here rather than re-read per field. A miss is memoised too, as `None`.
    /// Shared between clones and across the threads an audit runs on.
    configuration_objects: Arc<Mutex<BTreeMap<String, Option<Arc<ConfigurationObject>>>>>,
    /// The metadata names a role's `Rights.xml` refers to, each owner file
    /// parsed once and shared between clones.
    role_rights_source: Arc<SourceTreeRoleRightsSource>,
    /// What the interface writers resolve more than once: a metadata file's
    /// class and uuid, an owner's declared commands.
    interface_memo: Arc<Mutex<InterfaceResolutionMemo>>,
    /// Generated-type `TypeId`s a data-composition schema asked for, by type
    /// name, memoised hit and miss alike: a report names the same catalog
    /// from many fields and a tree has many reports.
    generated_type_ids: Arc<Mutex<BTreeMap<String, Option<String>>>>,
    /// `StyleItems/*.xml`: uuid -> name, read once.
    style_items: Arc<std::sync::OnceLock<BTreeMap<String, String>>>,
    /// The uuid each readable name of a help page resolves to, or why it does
    /// not: pages of one tree link the same objects over and over.
    help_references: Arc<Mutex<BTreeMap<String, Result<HelpReference, String>>>>,
    /// `Configuration.xml`'s own uuid, read once (`None` when it cannot be).
    configuration_uuid: Arc<std::sync::OnceLock<Option<String>>>,
    /// `moxel_object_refs`, read once: every chart of a spreadsheet template
    /// asks for it, and each answer read every common picture and style item
    /// of the tree (ERP УХ: 3 350 files per chart; one template took 64-89 s).
    moxel_object_refs: Arc<std::sync::OnceLock<Result<BTreeMap<String, String>, String>>>,
    /// Source files already read into memory, by the path they were read at:
    /// a base-free stage reads every metadata XML of its tree once, up front,
    /// and the resolvers below read those files from here. On the lab
    /// workstation every file open waits in the on-access scanner's queue;
    /// ERP УХ's stage re-read metadata XMLs 150 000 times, about 20 ms each.
    preloaded: PreloadedSourceFiles,
    /// Explicit operation-local original census, never a path-cache fallback.
    original_source: Option<Arc<dyn OriginalSourceTree>>,
}

/// Source files read into memory, by path: shared, and printed as a count.
#[derive(Clone, Default)]
pub(crate) struct PreloadedSourceFiles(pub(crate) Arc<HashMap<PathBuf, Arc<Vec<u8>>>>);

impl std::fmt::Debug for PreloadedSourceFiles {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} preloaded files", self.0.len())
    }
}

impl MetadataSourceContext {
    pub fn new(source_root: PathBuf) -> Self {
        Self {
            role_rights_source: Arc::new(SourceTreeRoleRightsSource::new(source_root.clone())),
            source_root,
            configuration_objects: Arc::new(Mutex::new(BTreeMap::new())),
            interface_memo: Arc::new(Mutex::new(InterfaceResolutionMemo::default())),
            generated_type_ids: Arc::new(Mutex::new(BTreeMap::new())),
            style_items: Arc::new(std::sync::OnceLock::new()),
            help_references: Arc::new(Mutex::new(BTreeMap::new())),
            configuration_uuid: Arc::new(std::sync::OnceLock::new()),
            moxel_object_refs: Arc::new(std::sync::OnceLock::new()),
            preloaded: PreloadedSourceFiles::default(),
            original_source: None,
        }
    }

    /// A context whose resolvers read the given files from memory: `files`
    /// maps the paths a tree walk gave (`<root>/<folder>/<name>.xml`) to their
    /// bytes. A file not among them is read from disk as before.
    pub fn with_preloaded(
        source_root: PathBuf,
        files: Arc<HashMap<PathBuf, Arc<Vec<u8>>>>,
    ) -> Self {
        Self {
            role_rights_source: Arc::new(SourceTreeRoleRightsSource::with_preloaded(
                source_root.clone(),
                PreloadedSourceFiles(files.clone()),
            )),
            preloaded: PreloadedSourceFiles(files),
            ..Self::new(source_root)
        }
    }

    #[doc(hidden)]
    pub fn with_original_source(source: Arc<dyn OriginalSourceTree>) -> Self {
        Self {
            original_source: Some(source.clone()),
            ..Self::new(source.canonical_root().to_owned())
        }
    }

    #[doc(hidden)]
    pub fn require_original_unchanged(&self) -> anyhow::Result<()> {
        if let Some(source) = &self.original_source {
            source.require_unchanged()?;
        }
        Ok(())
    }

    /// The bytes of a source file: shared from memory when preloaded, else
    /// read. (The resolvers ask for ERP УХ's metadata XMLs some 150 000 times;
    /// a copy each time cost a large-block allocation each.)
    fn read_source(&self, path: &Path) -> std::io::Result<SourceBytes> {
        if let Some(source) = &self.original_source {
            return source
                .read_original_path(path)
                .map(SourceBytes::Shared)
                .map_err(|error| std::io::Error::other(error.to_string()));
        }
        match self.preloaded.0.get(path) {
            Some(bytes) => Ok(SourceBytes::Shared(bytes.clone())),
            None => fs::read(path).map(SourceBytes::Owned),
        }
    }

    /// The uuid `Configuration.xml` declares for the configuration itself.
    fn configuration_uuid(&self) -> Option<String> {
        self.configuration_uuid
            .get_or_init(|| {
                let xml = self
                    .read_source(&self.source_root.join("Configuration.xml"))
                    .ok()?;
                let properties = parse_simple_metadata_xml_properties(&xml).ok()?;
                (properties.kind == "Configuration").then_some(properties.uuid)
            })
            .clone()
    }

    /// The resolver the base-free role rights writer looks names up with.
    pub fn role_rights_source(&self) -> &SourceTreeRoleRightsSource {
        &self.role_rights_source
    }

    /// The storage `TypeId` of a configuration generated type named the way a
    /// data-composition schema names it (`CatalogRef.X`, `Characteristic.X`),
    /// read from the owning object's `InternalInfo`. `None` when the tree has
    /// no such object or type.
    fn dcs_generated_type_id(&self, name: &str) -> Option<String> {
        if let Ok(cache) = self.generated_type_ids.lock()
            && let Some(found) = cache.get(name)
        {
            return found.clone();
        }
        let resolved = if name.starts_with("DefinedType.") {
            None
        } else {
            self.resolve_metadata_type_id(name).ok()
        };
        if let Ok(mut cache) = self.generated_type_ids.lock() {
            cache.insert(name.to_string(), resolved.clone());
        }
        resolved
    }

    /// The tree's configuration style items: lowercase uuid -> name. A file
    /// that does not parse contributes nothing rather than failing the scan.
    fn dcs_style_items(&self) -> &BTreeMap<String, String> {
        self.style_items
            .get_or_init(|| style_reference_types_from_source_root(&self.source_root))
    }

    /// One metadata object of the source tree, by `"<Class>.<Name>"`.
    ///
    /// Read from `<root>/<Family>/<Name>.xml` and memoised, hit and miss
    /// alike. A file that will not parse, or that holds a different class than
    /// the key names, is a miss: the caller then refuses the data path rather
    /// than writing a binding from a guess.
    fn configuration_object(&self, key: &str) -> Option<Arc<ConfigurationObject>> {
        if let Ok(cache) = self.configuration_objects.lock()
            && let Some(found) = cache.get(key)
        {
            return found.clone();
        }
        let built = self.read_configuration_object(key);
        if let Ok(mut cache) = self.configuration_objects.lock() {
            cache.insert(key.to_string(), built.clone());
        }
        built
    }

    fn read_configuration_object(&self, key: &str) -> Option<Arc<ConfigurationObject>> {
        let (class, name) = key.split_once('.')?;
        let folder = configuration_object_source_folder(class)?;
        let path = self.source_root.join(folder).join(format!("{name}.xml"));
        let xml = self.read_source(&path).ok()?;
        let object = parse_configuration_object_xml(&xml).ok().flatten()?;
        (object.class == class).then(|| Arc::new(object))
    }

    pub fn moxel_object_refs(&self) -> Result<BTreeMap<String, String>> {
        let refs = self.moxel_object_refs.get_or_init(|| {
            let mut refs = BTreeMap::new();
            self.collect_simple_metadata_refs("CommonPictures", "CommonPicture", &mut refs)
                .and_then(|()| {
                    self.collect_simple_metadata_refs("StyleItems", "StyleItem", &mut refs)
                })
                .map(|()| refs)
                .map_err(|error| format!("{error:#}"))
        });
        refs.clone().map_err(|error| anyhow!(error))
    }

    fn collect_simple_metadata_refs(
        &self,
        folder: &str,
        kind: &str,
        refs: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        let dir = self.source_root.join(folder);
        if !dir.is_dir() {
            return Ok(());
        }
        for entry in
            fs::read_dir(&dir).with_context(|| format!("failed to read {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("xml") {
                continue;
            }
            let xml = self
                .read_source(&path)
                .with_context(|| format!("failed to read metadata XML {}", path.display()))?;
            let properties = parse_simple_metadata_xml_properties(&xml)
                .with_context(|| format!("failed to parse metadata XML {}", path.display()))?;
            if properties.kind == kind {
                refs.insert(properties.uuid, format!("{}.{}", kind, properties.name));
            }
        }
        Ok(())
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
        let xml = self
            .read_source(&path)
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

    /// The `<Item id="…">` a metadata object's `Ext/Predefined.xml` gives one
    /// of its predefined items, by name.
    ///
    /// The tree nests -- a folder carries `<ChildItems>` -- and a name is
    /// looked up over the whole tree, which is how a design-time reference
    /// such as `Catalog.<X>.<Y>` resolves to the number the body stores.
    fn resolve_predefined_item_id(&self, owner_reference: &str, item_name: &str) -> Result<String> {
        let (prefix, folder) = metadata_reference_source_folder(owner_reference)
            .ok_or_else(|| anyhow!("unsupported predefined owner reference: {owner_reference}"))?;
        let owner_name = owner_reference
            .strip_prefix(&format!("{prefix}."))
            .ok_or_else(|| anyhow!("invalid predefined owner reference: {owner_reference}"))?;
        let path = self
            .source_root
            .join(folder)
            .join(owner_name)
            .join("Ext")
            .join("Predefined.xml");
        let xml = self
            .read_source(&path)
            .with_context(|| format!("failed to read Predefined XML {}", path.display()))?;
        let items = parse_predefined_data_xml(&xml)?;
        find_predefined_item_id(&items, item_name).ok_or_else(|| {
            anyhow!("{owner_reference} declares no predefined item named {item_name}")
        })
    }

    fn resolve_defined_type_type_id(&self, reference: &str) -> Result<String> {
        let name = defined_type_name_from_reference(reference)?;
        let path = self
            .source_root
            .join("DefinedTypes")
            .join(format!("{name}.xml"));
        let xml = self
            .read_source(&path)
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
        let xml = self
            .read_source(&path)
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
        let xml = self
            .read_source(&path)
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
        let xml = self
            .read_source(&path)
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

    /// The names of an information register's dimensions whose `<Master>` is
    /// `true`, in document order.
    ///
    /// That list is what indexes `OpenByValue`: the ordinal is `3 + <index of
    /// the named dimension among these>`, exact on 360 of 360 references of
    /// both corpora. The filter is `<Master>` and not a reference type --
    /// "every dimension a reference names is a master dimension" holds, but
    /// the converse test scores 285 of 360.
    fn information_register_master_dimensions(&self, register: &str) -> Result<Vec<String>> {
        let path = self
            .source_root
            .join("InformationRegisters")
            .join(format!("{register}.xml"));
        let xml = self.read_source(&path).with_context(|| {
            format!("failed to read InformationRegister XML {}", path.display())
        })?;
        let mut reader = Reader::from_reader(&xml[..]);
        let mut buffer = Vec::new();
        let mut path_stack = Vec::<String>::new();
        let mut dimensions = Vec::new();
        let mut name = None::<String>;
        let mut master = false;
        let mut text = String::new();
        loop {
            match reader.read_event_into(&mut buffer) {
                Ok(Event::Start(event)) => {
                    let local = xml_local_name(event.local_name().as_ref());
                    if local == "Dimension" && path_ends_with(&path_stack, &["InformationRegister"])
                    {
                        name = None;
                        master = false;
                    }
                    text.clear();
                    path_stack.push(local);
                }
                Ok(Event::Text(chunk)) => text.push_str(chunk.xml_content()?.as_ref()),
                Ok(Event::End(_)) => {
                    let local = path_stack.pop().unwrap_or_default();
                    let inside = path_stack
                        .iter()
                        .rev()
                        .take(2)
                        .any(|part| part == "Dimension");
                    if inside && local == "Name" && name.is_none() {
                        name = Some(text.trim().to_string());
                    } else if inside && local == "Master" {
                        master = text.trim() == "true";
                    } else if local == "Dimension"
                        && let Some(name) = name.take()
                        && master
                    {
                        dimensions.push(name);
                    }
                    text.clear();
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(error) => {
                    return Err(anyhow!(
                        "failed to read InformationRegister XML {}: {error}",
                        path.display()
                    ));
                }
            }
            buffer.clear();
        }
        Ok(dimensions)
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
        // A tabular section's types (`DocumentTabularSection.<owner>.<section>`)
        // live in the owner's own file.
        let owner_file = ibcmd_schema::generated_tabular_section_source_owner(generated_type_name);
        if let Some(owner) = owner_file {
            let path = self.source_root.join(folder).join(format!("{owner}.xml"));
            if let Ok(xml) = self.read_source(&path)
                && let Ok(type_id) = parse_generated_type_type_id(&xml, generated_type_name)
            {
                return Ok(type_id);
            }
        }
        let path = self.source_root.join(folder).join(format!("{name}.xml"));
        let xml = self
            .read_source(&path)
            .with_context(|| format!("failed to read metadata XML {}", path.display()))?;
        parse_generated_type_type_id(&xml, generated_type_name)
            .with_context(|| format!("failed to resolve TypeId from {}", path.display()))
    }

    fn resolve_metadata_reference_uuid(&self, reference: &str) -> Result<String> {
        let reference = reference.trim();
        // Objects that live in a file of their own below their owner: a
        // nested subsystem (`Subsystems/A/Subsystems/B.xml`) and a
        // calculation register's recalculation
        // (`CalculationRegisters/R/Recalculations/X.xml`).
        let nested = if let Some(rest) = reference.strip_prefix("Subsystem.")
            && rest.contains(".Subsystem.")
        {
            let names = rest.split(".Subsystem.").collect::<Vec<_>>();
            let mut path = self.source_root.join("Subsystems");
            for (index, name) in names.iter().enumerate() {
                path = if index + 1 == names.len() {
                    path.join(format!("{name}.xml"))
                } else {
                    path.join(name).join("Subsystems")
                };
            }
            Some((path, "Subsystem"))
        } else if let Some(rest) = reference.strip_prefix("CalculationRegister.")
            && let Some((register, recalculation)) = rest.split_once(".Recalculation.")
        {
            Some((
                self.source_root
                    .join("CalculationRegisters")
                    .join(register)
                    .join("Recalculations")
                    .join(format!("{recalculation}.xml")),
                "Recalculation",
            ))
        } else {
            None
        };
        if let Some((path, kind)) = nested {
            let xml = self
                .read_source(&path)
                .with_context(|| format!("failed to read {kind} XML {}", path.display()))?;
            let properties = parse_simple_metadata_xml_properties(&xml)?;
            if properties.kind != kind {
                return Err(anyhow!(
                    "expected {kind} XML at {}, got {}",
                    path.display(),
                    properties.kind
                ));
            }
            return Ok(properties.uuid);
        }
        let (prefix, folder) = metadata_reference_source_folder(reference).ok_or_else(|| {
            anyhow!("unsupported metadata reference for source resolution: {reference}")
        })?;
        self.resolve_simple_metadata_uuid(reference, prefix, folder, &format!("{prefix}."))
    }

    fn resolve_command_reference_uuid(&self, reference: &str) -> Result<String> {
        let reference = reference.trim();
        let Some((owner_reference, command_name)) = reference.split_once(".Command.") else {
            return self.resolve_metadata_reference_uuid(reference);
        };
        let (prefix, folder) =
            metadata_reference_source_folder(owner_reference).ok_or_else(|| {
                anyhow!("unsupported command owner reference for source resolution: {reference}")
            })?;
        let owner_name = owner_reference
            .strip_prefix(&format!("{prefix}."))
            .ok_or_else(|| anyhow!("invalid command owner reference: {reference}"))?;
        let path = self
            .source_root
            .join(folder)
            .join(format!("{owner_name}.xml"));
        let xml = self
            .read_source(&path)
            .with_context(|| format!("failed to read command owner XML {}", path.display()))?;
        parse_nested_command_uuid_from_xml(&xml, command_name)
            .with_context(|| format!("failed to resolve command {reference}"))
    }

    /// The uuid of a form a property names: `CommonForm.<name>`, or
    /// `<Owner>.Form.<name>` of an object, read from the form's own holder XML.
    fn resolve_form_uuid(&self, reference: &str) -> Result<String> {
        let reference = reference.trim();
        // A form the configuration no longer has is spelled by its bare uuid.
        if is_uuid_like_ascii(reference.as_bytes()) {
            return Ok(reference.to_ascii_lowercase());
        }
        let path = if let Some(name) = reference.strip_prefix("CommonForm.") {
            self.source_root
                .join("CommonForms")
                .join(format!("{name}.xml"))
        } else {
            let (owner, form) = reference
                .split_once(".Form.")
                .ok_or_else(|| anyhow!("unsupported form reference: {reference}"))?;
            let (prefix, folder) = metadata_reference_source_folder(owner)
                .ok_or_else(|| anyhow!("unsupported form owner reference: {reference}"))?;
            let owner_name = owner
                .strip_prefix(&format!("{prefix}."))
                .ok_or_else(|| anyhow!("invalid form owner reference: {reference}"))?;
            self.source_root
                .join(folder)
                .join(owner_name)
                .join("Forms")
                .join(format!("{form}.xml"))
        };
        let text = self
            .read_source(&path)
            .and_then(|bytes| {
                String::from_utf8(bytes.into_vec()).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "stream did not contain valid UTF-8",
                    )
                })
            })
            .with_context(|| format!("failed to read form XML {}", path.display()))?;
        for marker in ["<Form uuid=\"", "<CommonForm uuid=\""] {
            if let Some(at) = text.find(marker) {
                let rest = &text[at + marker.len()..];
                if let Some(end) = rest.find('"') {
                    return Ok(rest[..end].to_string());
                }
            }
        }
        Err(anyhow!("no form uuid in {}", path.display()))
    }
}

/// The source the tools build. Each method is the context's own of the same
/// name (`MetadataSourceContext::name` is the inherent method, never this
/// trait's), so a writer that reads the tree through the trait gets the very
/// answers, errors and memoised values a call on the context gives.
impl MetadataSource for MetadataSourceContext {
    fn source_root(&self) -> &Path {
        &self.source_root
    }

    fn read_source(&self, path: &Path) -> std::io::Result<SourceBytes> {
        MetadataSourceContext::read_source(self, path)
    }

    fn configuration_uuid(&self) -> Option<String> {
        MetadataSourceContext::configuration_uuid(self)
    }

    fn configuration_object(&self, key: &str) -> Option<Arc<ConfigurationObject>> {
        MetadataSourceContext::configuration_object(self, key)
    }

    fn resolve_metadata_type_id(&self, reference: &str) -> Result<String> {
        MetadataSourceContext::resolve_metadata_type_id(self, reference)
    }

    fn resolve_metadata_reference_uuid(&self, reference: &str) -> Result<String> {
        MetadataSourceContext::resolve_metadata_reference_uuid(self, reference)
    }

    fn resolve_style_item_uuid(&self, reference: &str) -> Result<String> {
        MetadataSourceContext::resolve_style_item_uuid(self, reference)
    }

    fn resolve_common_picture_uuid(&self, reference: &str) -> Result<String> {
        MetadataSourceContext::resolve_common_picture_uuid(self, reference)
    }

    fn resolve_command_group_uuid(&self, reference: &str) -> Result<String> {
        MetadataSourceContext::resolve_command_group_uuid(self, reference)
    }

    fn resolve_command_reference_uuid(&self, reference: &str) -> Result<String> {
        MetadataSourceContext::resolve_command_reference_uuid(self, reference)
    }

    fn resolve_form_uuid(&self, reference: &str) -> Result<String> {
        MetadataSourceContext::resolve_form_uuid(self, reference)
    }

    fn resolve_metadata_child_uuid(
        &self,
        owner_reference: &str,
        child_kind: &str,
        child_name: &str,
    ) -> Result<String> {
        MetadataSourceContext::resolve_metadata_child_uuid(
            self,
            owner_reference,
            child_kind,
            child_name,
        )
    }

    fn resolve_predefined_item_id(&self, owner_reference: &str, item_name: &str) -> Result<String> {
        MetadataSourceContext::resolve_predefined_item_id(self, owner_reference, item_name)
    }

    fn information_register_master_dimensions(&self, register: &str) -> Result<Vec<String>> {
        MetadataSourceContext::information_register_master_dimensions(self, register)
    }

    fn resolve_simple_metadata_uuid(
        &self,
        reference: &str,
        expected_kind: &str,
        folder: &str,
        prefix: &str,
    ) -> Result<String> {
        MetadataSourceContext::resolve_simple_metadata_uuid(
            self,
            reference,
            expected_kind,
            folder,
            prefix,
        )
    }

    fn dcs_generated_type_id(&self, name: &str) -> Option<String> {
        MetadataSourceContext::dcs_generated_type_id(self, name)
    }

    fn dcs_style_items(&self) -> &BTreeMap<String, String> {
        MetadataSourceContext::dcs_style_items(self)
    }

    fn moxel_object_refs(&self) -> Result<BTreeMap<String, String>> {
        MetadataSourceContext::moxel_object_refs(self)
    }

    fn help_reference(&self, reference: &str) -> Result<HelpReference> {
        MetadataSourceContext::help_reference(self, reference)
    }

    fn interface_memo(&self) -> &Mutex<InterfaceResolutionMemo> {
        &self.interface_memo
    }
}

impl MetadataSourceContext {
    /// The uuid a readable help reference names, read from the source tree
    /// and memoised: an object, a nested subsystem, a form, a template, a
    /// recalculation, a command, a common picture or the configuration itself
    /// -- every name the exporter's help index holds.
    ///
    /// A reference whose file -- or whose owner's file -- the tree does not
    /// have is [`HelpReference::Absent`]; a file that is there and declares
    /// something else, or does not parse, is an error.
    fn help_reference(&self, reference: &str) -> Result<HelpReference> {
        if let Ok(cache) = self.help_references.lock()
            && let Some(found) = cache.get(reference)
        {
            return found.clone().map_err(|error| anyhow!(error));
        }
        let resolved = match self.read_help_reference_uuid(reference) {
            Ok(uuid) => Ok(HelpReference::Resolved(uuid)),
            Err(error) if is_not_found(&error) => Ok(HelpReference::Absent),
            Err(error) => Err(format!("{error:#}")),
        };
        if let Ok(mut cache) = self.help_references.lock() {
            cache.insert(reference.to_string(), resolved.clone());
        }
        resolved.map_err(|error| anyhow!(error))
    }

    fn read_help_reference_uuid(&self, reference: &str) -> Result<String> {
        let parts = reference.split('.').collect::<Vec<_>>();
        let (path, kind) = match parts.as_slice() {
            ["Configuration", _] => return self.resolve_configuration_uuid(reference),
            [_, _, "Command", _] => return self.resolve_command_reference_uuid(reference),
            [class, name] => {
                let (prefix, folder) = metadata_reference_source_folder(reference)
                    .ok_or_else(|| anyhow!("unsupported reference class `{class}`"))?;
                (
                    self.source_root.join(folder).join(format!("{name}.xml")),
                    prefix,
                )
            }
            ["Subsystem", ..] if parts.iter().step_by(2).all(|class| *class == "Subsystem") => {
                let mut path = self.source_root.join("Subsystems");
                let names = parts.iter().skip(1).step_by(2).collect::<Vec<_>>();
                for (index, name) in names.iter().enumerate() {
                    path = if index + 1 == names.len() {
                        path.join(format!("{name}.xml"))
                    } else {
                        path.join(name).join("Subsystems")
                    };
                }
                (path, "Subsystem")
            }
            [
                owner_class,
                owner,
                child_class @ ("Form" | "Template" | "Recalculation"),
                child,
            ] => {
                let owner_reference = format!("{owner_class}.{owner}");
                let (_, folder) = metadata_reference_source_folder(&owner_reference)
                    .ok_or_else(|| anyhow!("unsupported owner class `{owner_class}`"))?;
                let child_folder = match *child_class {
                    "Form" => "Forms",
                    "Template" => "Templates",
                    _ => "Recalculations",
                };
                (
                    self.source_root
                        .join(folder)
                        .join(owner)
                        .join(child_folder)
                        .join(format!("{child}.xml")),
                    *child_class,
                )
            }
            [owner_class, owner, child_class, child] => {
                return self.resolve_metadata_child_uuid(
                    &format!("{owner_class}.{owner}"),
                    child_class,
                    child,
                );
            }
            _ => return Err(anyhow!("unsupported reference shape")),
        };
        let xml = self
            .read_source(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let name = parts.last().copied().unwrap_or_default();
        // The tree is read on a case-insensitive file system, where
        // `Catalog.x` finds `Catalogs/X.xml`: the file must declare the very
        // class and name the reference spells.
        if properties.kind != kind || properties.name != name {
            return Err(anyhow!(
                "{} declares {}.{}",
                path.display(),
                properties.kind,
                properties.name
            ));
        }
        Ok(properties.uuid)
    }
}

/// Whether reading the tree failed because a file or a folder is not there.
fn is_not_found(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
    })
}

/// Scans `source_root/StyleItems/*.xml` for a uuid -> semantic-name map,
/// the same fact a `DataCompositionSchema` custom-`StyleItem` style-color
/// reference needs to compile base-free (see
/// `compiler::bodies::dcs::compile_dcs_with_references`). Each object's
/// name comes from its own file name -- the same `StyleItems/<Name>.xml`
/// convention `source::infer_object_hint` already treats as this family's
/// object identity -- paired with the root `<StyleItem uuid="...">`
/// attribute read directly from that same file. A missing `StyleItems`
/// directory, or any individual file that fails to parse or carries no
/// `uuid` attribute, is skipped rather than treated as an error: this map
/// is best-effort evidence gathering for one optional compile-direction
/// coordinate, not a required precondition for the rest of the bootstrap
/// scan.
pub fn style_reference_types_from_source_root(source_root: &Path) -> BTreeMap<String, String> {
    let mut style_reference_types = BTreeMap::new();
    let Ok(entries) = fs::read_dir(source_root.join("StyleItems")) else {
        return style_reference_types;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("xml") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Ok(xml) = fs::read(&path) else {
            continue;
        };
        if let Some(uuid) = parse_style_item_root_uuid(&xml) {
            style_reference_types.insert(uuid, name.to_string());
        }
    }
    style_reference_types
}

/// Reads the `uuid` attribute off a `StyleItems/<Name>.xml` object's root
/// `<StyleItem>` element, canonicalized to the lowercase-hyphenated form
/// (matching the `compile_dcs_with_references` resolver contract). Returns
/// `None` for anything else (malformed XML, a different root element, a
/// missing or non-UUID `uuid` attribute) -- the caller treats that as "this
/// file contributes nothing," never as a hard error.
fn parse_style_item_root_uuid(xml: &[u8]) -> Option<String> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(event) | Event::Empty(event) => {
                if event.local_name().as_ref() != b"StyleItem" {
                    continue;
                }
                let uuid = xml_attr_value(&event, "uuid")?;
                return Some(
                    uuid::Uuid::parse_str(uuid.trim())
                        .ok()?
                        .hyphenated()
                        .to_string(),
                );
            }
            Event::Eof => return None,
            _ => {}
        }
        buffer.clear();
    }
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
        "Configuration" => patch_configuration_source_metadata(text, &properties, xml)?,
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

pub fn metadata_xml_base_free_blockers(xml: &[u8]) -> Result<Vec<String>> {
    let properties = parse_simple_metadata_xml_properties(xml)?;
    let shape = metadata_xml_base_free_shape(xml, &properties.kind)?;
    let mut blockers = Vec::new();
    blockers.push(format!(
        "{} metadata XML {} ({}) is an editable source projection, but not a complete native Config metadata row",
        properties.kind, properties.name, properties.uuid
    ));
    blockers.push(format!(
        "source XML shape has {} direct Properties child element(s) and {} ChildObjects entry/entries{}; these counts do not include native field order, generated type slots, owner-specific tail slots, or serialized section indexes",
        shape.direct_property_elements,
        shape.child_objects,
        shape.child_object_breakdown()
    ));
    blockers.push(
        "staging currently calls pack_simple_metadata_blob_from_xml_with_source, which patches the selected XML fields into the active metadata blob to preserve native field order, owner-specific tail slots, generated type slots, and unsupported scalar/property bodies".to_string(),
    );
    if shape.child_objects > 0 {
        blockers.push(format!(
            "source XML has {} ChildObjects entries{}, but base-free metadata row generation would also need to synthesize the serialized child-object indexes and per-child native payloads",
            shape.child_objects,
            shape.child_object_breakdown()
        ));
    }
    blockers.push(format!(
        "base-free generation is blocked until a full {} metadata compiler can create the complete native row without using the active Config blob",
        properties.kind
    ));
    Ok(blockers)
}

pub fn common_module_metadata_base_free_blockers(xml: &[u8]) -> Result<Vec<String>> {
    let properties = parse_common_module_xml_properties(xml)?;
    Ok(vec![
        format!(
            "CommonModule metadata XML {} ({}) is an editable source projection, but not a complete native Config metadata row",
            properties.name, properties.uuid
        ),
        "staging currently calls pack_common_module_metadata_blob_from_xml, which patches module flags and text properties into the active metadata blob to preserve the native envelope, field order, and trailing slots".to_string(),
        "base-free generation is blocked until a full CommonModule metadata compiler can create the complete native row without using the active Config blob".to_string(),
    ])
}

pub fn pack_style_body_blob_from_xml(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<PackedStyleBodyBlob> {
    pack_style_body_blob_from_xml_with_base(&[], xml, source)
}

pub fn pack_style_body_blob_from_xml_with_base(
    base_blob: &[u8],
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<PackedStyleBodyBlob> {
    let items = parse_style_body_xml_items(xml)?;
    let brands = items
        .iter()
        .filter(|item| item.name == "FirstBrand")
        .collect::<Vec<_>>();
    if brands.len() > 1 {
        anyhow::bail!("Style body has more than one FirstBrand colour");
    }
    let brand = brands.first().copied();
    if !base_blob.is_empty() {
        let formatted_by_key = items
            .iter()
            .filter(|item| item.name != "FirstBrand")
            .map(|item| {
                let formatted = format_style_body_item(item, source)?;
                let key = style_body_item_key(&formatted)?;
                Ok((key, formatted))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let inflated = inflate_raw(base_blob).context("failed to inflate base Style body blob")?;
        let mut plain =
            String::from_utf8(inflated).context("base Style body blob is not valid UTF-8")?;
        let body_start = plain
            .find('{')
            .ok_or_else(|| anyhow!("base Style body has no braced payload"))?;
        let fields = scan_braced_fields(&plain, body_start)?;
        if fields.first().map(|range| plain[range.clone()].trim()) != Some("2") {
            return Err(anyhow!("base Style body does not start with type marker 2"));
        }
        let count = fields
            .get(1)
            .and_then(|range| plain[range.clone()].trim().parse::<usize>().ok())
            .ok_or_else(|| anyhow!("base Style body item count is missing"))?;
        if fields.len() < 2 + count {
            return Err(anyhow!(
                "base Style body has {} fields, expected at least {}",
                fields.len(),
                2 + count
            ));
        }
        let mut replacements = Vec::<(Range<usize>, String)>::new();
        for item_range in fields.iter().skip(2).take(count) {
            let key = style_body_item_key(&plain[item_range.clone()])?;
            if let Some(formatted) = formatted_by_key.get(&key) {
                replacements.push((item_range.clone(), formatted.clone()));
            }
        }
        if let Some(trailer) = fields.get(2 + count) {
            let mut brand_text = String::new();
            style_body_brand_node(brand, source)?.write_platform(&mut brand_text, true);
            replacements.push((trailer.clone(), brand_text));
        } else if brand.is_some() {
            anyhow::bail!("base Style body has no 8.5 brand-colour trailer");
        }
        replacements.sort_by_key(|b| std::cmp::Reverse(b.0.start));
        for (range, replacement) in replacements {
            replace_1c_value_if_different(&mut plain, range, &replacement);
        }
        let blob = deflate_raw(plain.as_bytes())?;
        let output_sha256 = hex_sha256(&blob);

        return Ok(PackedStyleBodyBlob {
            blob,
            plain_bytes: plain.len(),
            output_sha256,
        });
    }

    // The 8.3.27 layout, the one the exporter reads (`STYLE_BODY_TAG` "1"):
    // `{1,<count>,{<key>,<kind>,<value>},...}` with no trailer, measured over
    // every item of ERP УХ's `Styles/Основной` (231 items, five value
    // shapes). Laid out as the platform stores it: the BOM, every nested
    // record on its own line, a list that ends with one closed on its own line.
    let mut entries = vec![
        StyleNode::token(if brand.is_some() { "2" } else { "1" }),
        StyleNode::token((items.len() - usize::from(brand.is_some())).to_string()),
    ];
    for item in &items {
        if item.name == "FirstBrand" {
            continue;
        }
        entries.push(style_body_item_node(item, source, brand.is_some())?);
    }
    if brand.is_some() {
        entries.push(style_body_brand_node(brand, source)?);
    }
    let mut text = String::from("\u{feff}");
    StyleNode::List(entries).write_platform(&mut text, true);
    let plain = text.into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedStyleBodyBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

/// A brace value of a style body, serialized the platform's way.
enum StyleNode {
    Token(String),
    List(Vec<StyleNode>),
}

impl StyleNode {
    fn token(value: impl Into<String>) -> Self {
        Self::Token(value.into())
    }

    fn write_platform(&self, out: &mut String, outermost: bool) {
        match self {
            Self::Token(value) => out.push_str(value),
            Self::List(values) => {
                if !outermost {
                    out.push_str("\r\n");
                }
                out.push('{');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    value.write_platform(out, false);
                }
                if matches!(values.last(), Some(Self::List(_))) {
                    out.push_str("\r\n");
                }
                out.push('}');
            }
        }
    }
}

/// `{0,<uuid>}` for a configuration style item, `{<code>}` for a standard one.
fn style_body_key_node(name: &str, source: Option<&MetadataSourceContext>) -> Result<StyleNode> {
    if let Some(code) = style_body_standard_code_for_name(name) {
        return Ok(StyleNode::List(vec![StyleNode::token(code.to_string())]));
    }
    if name.starts_with("StyleItem.") {
        let source = source.ok_or_else(|| {
            anyhow!("source root is required to resolve Style body reference {name}")
        })?;
        let uuid = source.resolve_style_item_uuid(name)?;
        return Ok(StyleNode::List(vec![
            StyleNode::token("0"),
            StyleNode::token(uuid),
        ]));
    }
    Err(anyhow!("unsupported Style Item name: {name}"))
}

fn style_body_ref_node(
    reference: &str,
    source: Option<&MetadataSourceContext>,
) -> Result<StyleNode> {
    let reference = reference.trim();
    let name = reference
        .strip_prefix("style:")
        .ok_or_else(|| anyhow!("unsupported Style reference: {reference}"))?;
    if style_body_standard_code_for_name(name).is_some() || name.starts_with("StyleItem.") {
        return style_body_key_node(name, source);
    }
    style_body_key_node(&format!("StyleItem.{name}"), source)
}

/// One `{<key>,<kind>,<value>}` item: kind 0 a color `{3,<variant>,<code>}`
/// (0 direct RGB, 2 web color, 3 style reference), kind 1 a style-item font
/// `{7,2,<mask>,<ref>,<members>,1,<scale>}`, kind 2 a border
/// `{3,1,<ref>,0,0,0}` -- the inverse of the exporter's
/// `parse_style_body_items`.
fn style_body_item_node(
    item: &StyleBodyXmlItem,
    source: Option<&MetadataSourceContext>,
    layout_8_5: bool,
) -> Result<StyleNode> {
    let key = style_body_key_node(&item.name, source)?;
    let (kind, value) = match &item.value {
        StyleBodyXmlValue::Color(value) => ("0", style_body_color_node(value, source)?),
        StyleBodyXmlValue::Font(attrs) => ("1", style_body_font_node(attrs, source)?),
        StyleBodyXmlValue::Border(attrs) => {
            let reference = attrs
                .get("ref")
                .ok_or_else(|| anyhow!("Style Border without ref: {}", item.name))?;
            (
                "2",
                StyleNode::List(vec![
                    StyleNode::token("3"),
                    StyleNode::token("1"),
                    style_body_ref_node(reference, source)?,
                    StyleNode::token("0"),
                    StyleNode::token("0"),
                    StyleNode::token("0"),
                ]),
            )
        }
    };
    let value = if layout_8_5 && matches!(kind, "0" | "1") {
        style_body_value_85(value, kind == "1")
    } else {
        value
    };
    Ok(StyleNode::List(vec![key, StyleNode::token(kind), value]))
}

/// 8.5 keeps the same declared font members and colour variants, raises the
/// tuple revision and adds the colour's repeated variant field.
fn style_body_value_85(value: StyleNode, font: bool) -> StyleNode {
    let StyleNode::List(mut fields) = value else {
        unreachable!("typed style value")
    };
    fields[0] = StyleNode::token(if font { "8" } else { "4" });
    if !font {
        let StyleNode::Token(variant) = &fields[1] else {
            unreachable!("typed colour variant")
        };
        fields.push(StyleNode::token(variant.clone()));
    }
    StyleNode::List(fields)
}

/// Brand colours are a trailer outside the declared item count. The evidenced
/// 8.5 layout carries its first colour as `{1,{0,<colour>}}`, or `{0}` if absent.
fn style_body_brand_node(
    brand: Option<&StyleBodyXmlItem>,
    source: Option<&MetadataSourceContext>,
) -> Result<StyleNode> {
    let Some(brand) = brand else {
        return Ok(StyleNode::List(vec![StyleNode::token("0")]));
    };
    let StyleBodyXmlValue::Color(colour) = &brand.value else {
        anyhow::bail!("FirstBrand must be a colour");
    };
    Ok(StyleNode::List(vec![
        StyleNode::token("1"),
        StyleNode::List(vec![
            StyleNode::token("0"),
            style_body_value_85(style_body_color_node(colour, source)?, false),
        ]),
    ]))
}

fn style_body_color_node(value: &str, source: Option<&MetadataSourceContext>) -> Result<StyleNode> {
    let value = value.trim();
    let (variant, code) = if let Some(hex) = value.strip_prefix('#') {
        if hex.len() != 6 {
            return Err(anyhow!("unsupported Style color literal: {value}"));
        }
        let red = u32::from_str_radix(&hex[0..2], 16)?;
        let green = u32::from_str_radix(&hex[2..4], 16)?;
        let blue = u32::from_str_radix(&hex[4..6], 16)?;
        (
            "0",
            StyleNode::List(vec![StyleNode::token(
                (red | (green << 8) | (blue << 16)).to_string(),
            )]),
        )
    } else if value.starts_with("web:") {
        let code = (-1..=512)
            .find(|code| ibcmd_1c_vocab::style_web_color_name(*code) == Some(value))
            .ok_or_else(|| anyhow!("unsupported Style web color: {value}"))?;
        (
            "2",
            StyleNode::List(vec![StyleNode::token(code.to_string())]),
        )
    } else if value.starts_with("style:") {
        ("3", style_body_ref_node(value, source)?)
    } else {
        return Err(anyhow!("unsupported Style color value: {value}"));
    };
    Ok(StyleNode::List(vec![
        StyleNode::token("3"),
        StyleNode::token(variant),
        code,
    ]))
}

/// The mask declares which of height (2), weight (4), italic (8), underline
/// (16) and strikeout (32) follow the reference, in that order; a present XML
/// attribute is a declared member. `1` and the scale close the record. Only
/// the default scale is observed and a non-default one is refused.
fn style_body_font_node(
    attrs: &BTreeMap<String, String>,
    source: Option<&MetadataSourceContext>,
) -> Result<StyleNode> {
    if attrs.get("kind").map(String::as_str) != Some("StyleItem") {
        return Err(anyhow!(
            "unsupported Style Font kind: {:?}",
            attrs.get("kind")
        ));
    }
    let reference = attrs
        .get("ref")
        .ok_or_else(|| anyhow!("Style Font without ref"))?;
    if attrs.get("scale").is_some_and(|scale| scale != "100") {
        return Err(anyhow!(
            "unobserved Style Font scale: {:?}",
            attrs.get("scale")
        ));
    }
    let mut mask = 0u32;
    let mut members = Vec::new();
    if let Some(height) = attrs.get("height") {
        let height = height
            .trim()
            .parse::<i64>()
            .with_context(|| format!("invalid Style Font height {height}"))?;
        mask |= 2;
        members.push(StyleNode::token((height * 10).to_string()));
    }
    if let Some(bold) = attrs.get("bold") {
        mask |= 4;
        let bold = parse_optional_xml_bool(Some(bold))?;
        members.push(StyleNode::token(if bold { "700" } else { "400" }));
    }
    for (bit, name) in [(8, "italic"), (16, "underline"), (32, "strikeout")] {
        if let Some(flag) = attrs.get(name) {
            mask |= bit;
            let flag = parse_optional_xml_bool(Some(flag))?;
            members.push(StyleNode::token(if flag { "1" } else { "0" }));
        }
    }
    let mut values = vec![
        StyleNode::token("7"),
        StyleNode::token("2"),
        StyleNode::token(mask.to_string()),
        style_body_ref_node(reference, source)?,
    ];
    values.extend(members);
    values.push(StyleNode::token("1"));
    values.push(StyleNode::token("100"));
    Ok(StyleNode::List(values))
}

/// `Ext/AdditionalIndexes.xml` as the platform stores it (the exporter's
/// `parse_additional_indexes` read backwards): `{1,{<count>,<record>...}}`, a
/// record `{"#",4b3b32e1-...,{1,<id>,<indexed>,<additional>,"<name>",
/// {0,<table uuid>},1,<nil uuid>}}`, a field list `{<n>,<field>...}` (`{0}`
/// when empty), a field `{"#",07c5e7a4-...,{1,{0,<uuid>}}}`, or `{1,{<code>}}`
/// for the standard Period -2, Recorder -3, LineNumber -4, Ref -5. The table
/// is the owner or one of its tabular sections; a field resolves among that
/// table's own fields first, then the owner's. Laid out as the platform does
/// (the two ERP УХ registers and the document that carry indexes); anything
/// unresolved is refused.
pub fn pack_additional_indexes_blob_from_xml(
    xml: &[u8],
    owner_xml: &[u8],
    owner_kind: &str,
    owner_name: &str,
    owner_uuid: &str,
) -> Result<Vec<u8>> {
    let indexes = parse_additional_indexes_xml(xml)?;
    let owner = parse_owner_field_uuids(owner_xml)?;
    let root = format!("{owner_kind}.{owner_name}");
    let mut records = vec![StyleNode::token(indexes.len().to_string())];
    for index in &indexes {
        let (table_uuid, section) = if index.table == root {
            (owner_uuid.to_string(), None)
        } else if let Some(section) = index
            .table
            .strip_prefix(&format!("{root}."))
            .filter(|section| !section.contains('.'))
        {
            let (uuid, _) = owner.sections.get(section).ok_or_else(|| {
                anyhow!(
                    "AdditionalIndexes table {} is not a tabular section",
                    index.table
                )
            })?;
            (uuid.clone(), Some(section))
        } else {
            return Err(anyhow!(
                "unsupported AdditionalIndexes table {}",
                index.table
            ));
        };
        let family = ibcmd_schema::metadata_child_storage_facts::AdditionalIndexStandardFieldFamily::for_table(
            owner_kind, section.is_none(),
        );
        let field_node = |name: &str| -> Result<StyleNode> {
            let slot = match family.field_code(name) {
                Some(code) => StyleNode::List(vec![StyleNode::token(code.to_string())]),
                None => {
                    let uuid = section
                        .and_then(|section| owner.sections.get(section))
                        .and_then(|(_, fields)| fields.get(name))
                        .or_else(|| owner.fields.get(name))
                        .ok_or_else(|| {
                            anyhow!(
                                "AdditionalIndexes field {name} is not a field of {}",
                                index.table
                            )
                        })?;
                    StyleNode::List(vec![StyleNode::token("0"), StyleNode::token(uuid.clone())])
                }
            };
            Ok(StyleNode::List(vec![
                StyleNode::token("\"#\""),
                StyleNode::token("07c5e7a4-56de-47f1-9895-724a499e8a8c"),
                StyleNode::List(vec![StyleNode::token("1"), slot]),
            ]))
        };
        let field_list = |names: &[String]| -> Result<StyleNode> {
            let mut values = vec![StyleNode::token(names.len().to_string())];
            for name in names {
                values.push(field_node(name)?);
            }
            Ok(StyleNode::List(values))
        };
        records.push(StyleNode::List(vec![
            StyleNode::token("\"#\""),
            StyleNode::token("4b3b32e1-14f6-4ce8-b4c4-1bc85a74237e"),
            StyleNode::List(vec![
                StyleNode::token("1"),
                StyleNode::token(index.id.clone()),
                field_list(&index.indexed_fields)?,
                field_list(&index.additional_fields)?,
                StyleNode::token(format_1c_string(&index.name)),
                StyleNode::List(vec![StyleNode::token("0"), StyleNode::token(table_uuid)]),
                StyleNode::token("1"),
                StyleNode::token("00000000-0000-0000-0000-000000000000"),
            ]),
        ]));
    }
    let mut text = String::from("\u{feff}");
    StyleNode::List(vec![StyleNode::token("1"), StyleNode::List(records)])
        .write_platform(&mut text, true);
    deflate_raw(text.as_bytes())
}

#[derive(Debug, Default)]
struct AdditionalIndexXml {
    id: String,
    name: String,
    table: String,
    indexed_fields: Vec<String>,
    additional_fields: Vec<String>,
}

fn parse_additional_indexes_xml(xml: &[u8]) -> Result<Vec<AdditionalIndexXml>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut indexes = Vec::<AdditionalIndexXml>::new();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "AdditionalIndex" {
                    let id = xml_attrs_map(&event)
                        .remove("id")
                        .ok_or_else(|| anyhow!("AdditionalIndex without id"))?;
                    indexes.push(AdditionalIndexXml {
                        id,
                        ..AdditionalIndexXml::default()
                    });
                }
                text.clear();
                path.push(local);
            }
            Ok(Event::Text(value)) => text.push_str(value.xml_content()?.as_ref()),
            Ok(Event::CData(value)) => text.push_str(value.xml_content()?.as_ref()),
            Ok(Event::GeneralRef(reference)) => {
                if let Some(ch) = reference.resolve_char_ref()? {
                    text.push(ch);
                } else {
                    let entity = reference.decode()?;
                    text.push_str(
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?,
                    );
                }
            }
            Ok(Event::End(_)) => {
                let value = text.trim().to_string();
                if let Some(index) = indexes.last_mut() {
                    if path_ends_with(&path, &["AdditionalIndex", "Name"]) {
                        index.name = value;
                    } else if path_ends_with(&path, &["AdditionalIndex", "Table"]) {
                        index.table = value;
                    } else if path_ends_with(&path, &["IndexedFields", "Field"]) {
                        index.indexed_fields.push(value);
                    } else if path_ends_with(&path, &["AdditionalFields", "Field"]) {
                        index.additional_fields.push(value);
                    }
                }
                text.clear();
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }
    Ok(indexes)
}

#[derive(Debug, Default)]
struct OwnerFieldUuids {
    /// Attribute, Dimension and Resource of the owner itself, by name.
    fields: BTreeMap<String, String>,
    /// Tabular section name -> (its uuid, its attributes by name).
    sections: BTreeMap<String, (String, BTreeMap<String, String>)>,
}

/// The uuids of an owner's fields, read from its metadata XML: the
/// attributes, dimensions and resources of its `ChildObjects`, and each
/// tabular section with the attributes of the section's own `ChildObjects`.
fn parse_owner_field_uuids(xml: &[u8]) -> Result<OwnerFieldUuids> {
    const CHILD_TAGS: [&str; 4] = ["Attribute", "Dimension", "Resource", "TabularSection"];
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    // (tag, uuid, name) of the child objects being read, outermost first
    let mut open = Vec::<(String, String, String)>::new();
    let mut result = OwnerFieldUuids::default();
    let mut text = String::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if CHILD_TAGS.contains(&local.as_str())
                    && path.last().map(String::as_str) == Some("ChildObjects")
                {
                    let uuid = xml_attrs_map(&event).remove("uuid").unwrap_or_default();
                    open.push((local.clone(), uuid, String::new()));
                }
                text.clear();
                path.push(local);
            }
            Ok(Event::Text(value)) => text.push_str(value.xml_content()?.as_ref()),
            Ok(Event::End(_)) => {
                let local = path.pop().unwrap_or_default();
                let depth = path.len();
                if local == "Name"
                    && depth >= 2
                    && path[depth - 1] == "Properties"
                    && let Some(last) = open.last_mut()
                    && last.2.is_empty()
                    && path[depth - 2] == last.0
                {
                    last.2 = text.trim().to_string();
                }
                if CHILD_TAGS.contains(&local.as_str())
                    && path.last().map(String::as_str) == Some("ChildObjects")
                    && let Some((tag, uuid, name)) = open.pop()
                {
                    match open.last() {
                        None if tag == "TabularSection" => {
                            result.sections.entry(name).or_default().0 = uuid;
                        }
                        None => {
                            result.fields.insert(name, uuid);
                        }
                        Some((parent_tag, _, parent_name)) if parent_tag == "TabularSection" => {
                            result
                                .sections
                                .entry(parent_name.clone())
                                .or_default()
                                .1
                                .insert(name, uuid);
                        }
                        Some(_) => {}
                    }
                }
                text.clear();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }
    Ok(result)
}

fn style_body_item_key(value: &str) -> Result<String> {
    let fields = scan_braced_fields(value.trim(), 0)?;
    let key = fields
        .first()
        .ok_or_else(|| anyhow!("Style body item has no key"))?;
    Ok(compact_1c_value(&value.trim()[key.clone()]))
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

fn format_style_body_item(
    item: &StyleBodyXmlItem,
    source: Option<&MetadataSourceContext>,
) -> Result<String> {
    let key = style_body_key_for_name(
        &item.name,
        source.map(|source| source as &dyn MetadataSource),
    )?;
    let (kind, value) = match &item.value {
        StyleBodyXmlValue::Color(value) => (
            "0",
            format_style_body_color_value(
                value,
                source.map(|source| source as &dyn MetadataSource),
            )?,
        ),
        StyleBodyXmlValue::Font(attrs) => {
            let node = style_body_value_85(style_body_font_node(attrs, source)?, true);
            let mut text = String::new();
            node.write_platform(&mut text, true);
            ("1", text)
        }
        StyleBodyXmlValue::Border(attrs) => ("2", format_style_body_border_value(attrs, source)?),
    };
    Ok(format!("{{{key},{kind},{value}}}"))
}

fn format_style_body_border_value(
    attrs: &BTreeMap<String, String>,
    source: Option<&MetadataSourceContext>,
) -> Result<String> {
    let reference = attrs
        .get("ref")
        .map(|value| style_body_ref_key(value, source.map(|source| source as &dyn MetadataSource)))
        .transpose()?
        .unwrap_or_else(|| "{-18}".to_string());
    Ok(format!("{{3,1,{reference},0,0,0}}"))
}

pub fn pack_schedule_blob_from_xml(xml: &[u8]) -> Result<PackedScheduleBlob> {
    let (schedule, details) = parse_schedule_xml(xml)?;
    // The compact fields, then the count of detailed daily schedules and each
    // one as a nested record of the same fields closed by its own `0` count.
    // The platform starts every nested record on a new line and closes the
    // outer one on its own line after them (ОбновлениеАгрегатов of БСП).
    let mut text = format!(
        "{{{},{}",
        schedule_fields(&schedule)?.join(","),
        details.len()
    );
    for detail in &details {
        text.push_str(&format!(
            ",\r\n{{{},0}}",
            schedule_fields(detail)?.join(",")
        ));
    }
    text.push_str(if details.is_empty() { "}" } else { "\r\n}" });

    let mut plain = b"\xEF\xBB\xBF".to_vec();
    plain.extend_from_slice(text.as_bytes());
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);

    Ok(PackedScheduleBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

fn schedule_fields(schedule: &ScheduleXmlProperties) -> Result<Vec<String>> {
    let mut fields = Vec::with_capacity(16 + schedule.week_days.len() + schedule.months.len());
    fields.push(format_schedule_date(&schedule.begin_date)?);
    fields.push(format_schedule_date(&schedule.end_date)?);
    fields.push(format_schedule_time(&schedule.begin_time)?);
    fields.push(format_schedule_time(&schedule.end_time)?);
    fields.push(format_schedule_time(&schedule.completion_time)?);
    fields.push(schedule.completion_interval.clone());
    fields.push(schedule.repeat_period_in_day.clone());
    fields.push(schedule.repeat_pause.clone());
    fields.push(schedule.week_days.len().to_string());
    fields.extend(schedule.week_days.iter().cloned());
    fields.push(schedule.week_day_in_month.clone());
    fields.push(schedule.day_in_month.clone());
    fields.push(schedule.months.len().to_string());
    fields.extend(schedule.months.iter().cloned());
    fields.push(schedule.weeks_period.clone());
    fields.push(schedule.days_repeat_period.clone());
    Ok(fields)
}

pub fn raw_deflated_looks_like_help_blob(blob: &[u8]) -> bool {
    raw_deflated_help_content_sha256(blob).is_ok()
}

pub fn raw_deflated_help_content_sha256(blob: &[u8]) -> Result<String> {
    let plain = String::from_utf8(inflate_raw(blob).context("failed to inflate Help blob")?)
        .context("Help blob is not valid UTF-8")?;
    let text = plain.trim_start_matches('\u{feff}');
    let fields = scan_braced_fields(text, 0).context("Help blob is not a 1C braced value")?;
    if fields
        .first()
        .is_none_or(|range| text[range.clone()].trim() != "5")
    {
        return Err(anyhow!("Help blob marker is not 5"));
    }
    let page_count = fields
        .get(1)
        .and_then(|range| text[range.clone()].trim().parse::<usize>().ok())
        .ok_or_else(|| anyhow!("Help blob page count is missing"))?;
    if page_count == 0 {
        return Err(anyhow!("Help blob has no pages"));
    }

    let mut hasher = Sha256::new();
    hasher.update(b"ibcmd-rs-help-v1");
    let mut index = 2usize;
    hash_usize(&mut hasher, page_count);
    for _ in 0..page_count {
        let page_range = fields
            .get(index)
            .ok_or_else(|| anyhow!("Help blob page name is missing"))?;
        let page = parse_1c_quoted_string(text[page_range.clone()].trim())
            .context("Help blob page name is not a 1C string")?;
        index += 1;
        let payload_range = fields
            .get(index)
            .ok_or_else(|| anyhow!("Help blob page payload is missing"))?;
        let payload = decode_base64_payload_field(text[payload_range.clone()].trim())
            .context("Help blob page payload is not valid base64")?;
        index += 1;
        hash_bytes(&mut hasher, page.as_bytes());
        hash_bytes(&mut hasher, &payload);
    }

    let file_count = fields
        .get(index)
        .and_then(|range| text[range.clone()].trim().parse::<usize>().ok())
        .unwrap_or(0);
    if fields.get(index).is_some() {
        index += 1;
    }
    hash_usize(&mut hasher, file_count);
    for _ in 0..file_count {
        let file_range = fields
            .get(index)
            .ok_or_else(|| anyhow!("Help blob file name is missing"))?;
        let file_name = parse_1c_quoted_string(text[file_range.clone()].trim())
            .context("Help blob file name is not a 1C string")?;
        index += 1;
        if fields.get(index).is_some_and(|range| {
            text[range.clone()]
                .trim()
                .chars()
                .all(|ch| ch.is_ascii_digit())
        }) {
            index += 1;
        }
        let payload_range = fields
            .get(index)
            .ok_or_else(|| anyhow!("Help blob file payload is missing"))?;
        let payload = decode_base64_payload_field(text[payload_range.clone()].trim())
            .context("Help blob file payload is not valid base64")?;
        index += 1;
        hash_bytes(&mut hasher, file_name.as_bytes());
        hash_bytes(&mut hasher, &payload);
    }

    Ok(format!("{:X}", hasher.finalize()))
}

fn hash_usize(hasher: &mut Sha256, value: usize) {
    hasher.update((value as u64).to_le_bytes());
}

fn hash_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hash_usize(hasher, bytes.len());
    hasher.update(bytes);
}

pub fn pack_form_body_blob_from_module_text(
    base_blob: &[u8],
    module_text: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    pack_form_body_blob_from_form_xml(base_blob, &[], Some(module_text))
}

pub fn pack_form_body_blob_from_form_xml(
    base_blob: &[u8],
    form_xml: &[u8],
    module_text: Option<&[u8]>,
) -> Result<PackedRawDeflatedBlob> {
    pack_form_body_blob_from_form_xml_with_source(base_blob, form_xml, module_text, None)
}

pub fn pack_form_body_blob_from_form_xml_with_source(
    base_blob: &[u8],
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&MetadataSourceContext>,
) -> Result<PackedRawDeflatedBlob> {
    pack_form_body_blob_from_form_xml_with_source_and_assets(
        base_blob,
        form_xml,
        module_text,
        source,
        None,
    )
}

pub fn pack_form_body_blob_from_form_xml_with_source_and_assets(
    base_blob: &[u8],
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&MetadataSourceContext>,
    form_item_assets_root: Option<&Path>,
) -> Result<PackedRawDeflatedBlob> {
    pack_form_body_blob_from_form_xml_through(
        base_blob,
        form_xml,
        module_text,
        source.map(|source| source as &dyn MetadataSource),
        form_item_assets_root,
    )
}

/// Compiles constants sets against clear always-used flags (an empty
/// infobase); returns whether `IBCMD_RS_ALWAYS_USED_CONSTANTS` is set and so
/// ignored.
pub fn clear_always_used_constants() -> bool {
    ALWAYS_USED_CONSTANTS_CLEAR.store(true, std::sync::atomic::Ordering::Relaxed);
    std::env::var_os("IBCMD_RS_ALWAYS_USED_CONSTANTS").is_some()
}

/// Installs the reader of the target's always-used constants; called once,
/// before any form is compiled, and asked only when a constants set needs it.
pub fn set_always_used_constants_source(source: impl Fn() -> Vec<String> + Send + Sync + 'static) {
    let _ = ALWAYS_USED_CONSTANTS_SOURCE.set(Box::new(source));
}

/// One predefined item's id, by name, over the whole nested tree.
fn find_predefined_item_id(items: &[PredefinedDataXmlItem], name: &str) -> Option<String> {
    for item in items {
        if item.name == name {
            return Some(item.id.clone());
        }
        if let Some(found) = find_predefined_item_id(&item.children, name) {
            return Some(found);
        }
    }
    None
}

/// Builds a managed Form body from source XML and a profile-known empty
/// marker-50 container. No external/base artifact participates in this path.
pub fn pack_form_body_blob_from_form_xml_base_free(
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&MetadataSourceContext>,
) -> Result<PackedRawDeflatedBlob> {
    pack_form_body_blob_from_form_xml_base_free_through(
        form_xml,
        module_text,
        source.map(|source| source as &dyn MetadataSource),
    )
}

fn parse_nested_command_uuid_from_xml(xml: &[u8], command_name: &str) -> Result<String> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut current_uuid = None::<String>;
    let mut current_name = None::<String>;
    let mut text_value = String::new();
    let mut collecting_name = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "Command" {
                    current_uuid = xml_attr_value(&event, "uuid")
                        .map(|value| normalize_uuid_text(&value))
                        .transpose()?;
                    current_name = None;
                } else if current_uuid.is_some()
                    && local == "Name"
                    && path_ends_with(&path, &["Command", "Properties"])
                {
                    text_value.clear();
                    collecting_name = true;
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "Command" {
                    current_uuid = None;
                    current_name = None;
                }
            }
            Ok(Event::Text(text)) if collecting_name => {
                text_value.push_str(text.xml_content()?.as_ref());
            }
            Ok(Event::CData(text)) if collecting_name => {
                text_value.push_str(text.xml_content()?.as_ref());
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if collecting_name && local == "Name" {
                    current_name = Some(text_value.trim().to_string());
                    collecting_name = false;
                    text_value.clear();
                }
                if local == "Command" {
                    if current_name.as_deref() == Some(command_name)
                        && let Some(uuid) = current_uuid.take()
                    {
                        return Ok(uuid);
                    }
                    current_uuid = None;
                    current_name = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Err(anyhow!("command {command_name} not found in owner XML"))
}

pub fn pack_role_rights_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    pack_role_rights_blob_from_xml_with_source(base_blob, xml, None)
}

pub fn pack_role_rights_blob_from_xml_with_source(
    base_blob: &[u8],
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
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
    let templates_range = fields
        .get(2)
        .ok_or_else(|| anyhow!("base Role rights body has no restriction templates field"))?
        .clone();
    let set_for_new_objects_range = fields
        .get(3)
        .ok_or_else(|| anyhow!("base Role rights body has no setForNewObjects field"))?
        .clone();
    let mut replacements =
        role_right_value_replacements(&plain, objects_range, &rights.objects, source)?;
    replacements.extend(role_restriction_template_value_replacements(
        &plain,
        templates_range,
        &rights.restriction_templates,
    )?);
    replacements.push((
        set_for_new_objects_range,
        if rights.set_for_new_objects { "1" } else { "0" }.to_string(),
    ));
    if let Some(value) = rights.set_for_attributes_by_default
        && let Some(range) = fields.get(4)
    {
        replacements.push((range.clone(), if value { "1" } else { "0" }.to_string()));
    }
    if let Some(value) = rights.independent_rights_of_child_objects
        && let Some(range) = fields.get(5)
    {
        replacements.push((range.clone(), if value { "1" } else { "0" }.to_string()));
    }
    replacements.sort_by_key(|b| std::cmp::Reverse(b.0.start));
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

#[doc(hidden)]
pub fn validate_command_interface_xml_document(xml: &[u8]) -> Result<()> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut root_seen = false;
    let mut root_closed = false;
    let mut depth = 0usize;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                if depth == 0 {
                    if root_seen {
                        return Err(anyhow!(
                            "CommandInterface XML contains more than one root element"
                        ));
                    }
                    let local = xml_local_name(event.local_name().as_ref());
                    if local != "CommandInterface" {
                        return Err(anyhow!(
                            "CommandInterface XML root must be CommandInterface, found {local}"
                        ));
                    }
                    root_seen = true;
                }
                depth += 1;
            }
            Ok(Event::Empty(event)) if depth == 0 => {
                if root_seen || root_closed {
                    return Err(anyhow!(
                        "CommandInterface XML contains more than one root element"
                    ));
                }
                let local = xml_local_name(event.local_name().as_ref());
                if local != "CommandInterface" {
                    return Err(anyhow!(
                        "CommandInterface XML root must be CommandInterface, found {local}"
                    ));
                }
                root_seen = true;
                root_closed = true;
            }
            Ok(Event::End(_)) => {
                if depth == 0 {
                    return Err(anyhow!(
                        "CommandInterface XML contains an unexpected closing element"
                    ));
                }
                depth -= 1;
                if depth == 0 {
                    root_closed = true;
                }
            }
            Ok(Event::Text(text)) if depth == 0 => {
                let text = text.xml_content()?;
                if !text
                    .trim_matches(|ch: char| ch.is_whitespace() || ch == '\u{feff}')
                    .is_empty()
                {
                    return Err(anyhow!(
                        "CommandInterface XML contains text outside the root element"
                    ));
                }
            }
            Ok(Event::CData(text)) if depth == 0 => {
                let text = text.xml_content()?;
                if !text.trim().is_empty() {
                    return Err(anyhow!(
                        "CommandInterface XML contains text outside the root element"
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => {
                return Err(error).context("failed to parse CommandInterface XML document");
            }
        }
        buffer.clear();
    }

    if !root_seen {
        return Err(anyhow!(
            "CommandInterface XML has no CommandInterface root element"
        ));
    }
    if !root_closed || depth != 0 {
        return Err(anyhow!("CommandInterface XML root element is not closed"));
    }
    Ok(())
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
        plain.push(',');
        plain.push_str(&uuid);
        plain.push(',');
        plain.push_str(&item.auto_record.to_string());
    }
    // Every stored content row of both corpora ends with one more `0` after
    // the items, and opens with the UTF-8 BOM.
    plain.push_str(",0}");
    let plain = format!("\u{feff}{plain}");

    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

/// The tree's predefined data against the stored row.
pub enum PredefinedPatch {
    /// The same items: the row with the tree's names, codes, descriptions and
    /// folder flags patched in.
    Patched(PackedRawDeflatedBlob),
    /// Not the same items: uuids only the tree has (`added`) and uuids only
    /// the row has (`removed`). A row cannot be patched into that shape; the
    /// data has to be compiled from the tree.
    ItemsDiffer {
        added: Vec<String>,
        removed: Vec<String>,
    },
}

/// As [`pack_predefined_data_blob_from_xml`], but a difference in the set of
/// items -- an item added or removed in the tree -- is an answer, not an error.
pub fn patch_predefined_data_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PredefinedPatch> {
    let (packed, added, removed) = patch_predefined_data(base_blob, xml)?;
    if added.is_empty() && removed.is_empty() {
        Ok(PredefinedPatch::Patched(packed))
    } else {
        Ok(PredefinedPatch::ItemsDiffer { added, removed })
    }
}

pub fn pack_predefined_data_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    let (packed, added, _removed) = patch_predefined_data(base_blob, xml)?;
    if !added.is_empty() {
        return Err(anyhow!(
            "PredefinedData XML contains items missing in base blob: {}",
            added.join(", ")
        ));
    }
    Ok(packed)
}

/// The row patched with the tree's items, the uuids the tree has and the row
/// lacks, and those the row has and the tree lacks.
fn patch_predefined_data(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<(PackedRawDeflatedBlob, Vec<String>, Vec<String>)> {
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
    let mut base_ids = BTreeSet::<String>::new();
    collect_predefined_replacements(
        &plain,
        table_range,
        &by_id,
        &mut seen,
        &mut base_ids,
        &mut replacements,
    )?;
    let missing = by_id
        .keys()
        .filter(|id| !seen.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    // The nil uuid is the root row of a hierarchical catalog's table, which
    // no item of the XML stands for.
    let removed = base_ids
        .iter()
        .filter(|id| id.as_str() != NATIVE_ZERO_UUID)
        .filter(|id| !by_id.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();

    replacements.sort_by_key(|b| std::cmp::Reverse(b.0.start));
    for (range, replacement) in replacements {
        plain.replace_range(range, &replacement);
    }
    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok((
        PackedRawDeflatedBlob {
            blob,
            plain_bytes: plain.len(),
            output_sha256,
        },
        missing,
        removed,
    ))
}

pub fn predefined_data_base_free_blockers(xml: &[u8]) -> Result<Vec<String>> {
    let items = parse_predefined_data_xml(xml)?;
    let stats = predefined_data_xml_stats(&items);
    let item_word = if stats.items == 1 { "item" } else { "items" };
    let root_word = if stats.root_items == 1 {
        "root row"
    } else {
        "root rows"
    };
    let folder_word = if stats.folders == 1 {
        "folder"
    } else {
        "folders"
    };
    let child_link_word = if stats.nested_child_links == 1 {
        "nested child link"
    } else {
        "nested child links"
    };
    Ok(vec![
        format!(
            "source XML has {} predefined {item_word} across {} {root_word} ({} {folder_word}, {} {child_link_word}, max depth {}), addressed by UUID, but the Config row owns the serialized predefined table order, rowset nesting and parent/children row shape",
            stats.items, stats.root_items, stats.folders, stats.nested_child_links, stats.max_depth,
        ),
        format!(
            "source XML carries only {} editable value fields (Name, Code, Description, IsFolder per item; {} non-empty text fields); staging currently patches existing base rows by item UUID and fails for source items missing from the base blob",
            stats.editable_value_fields, stats.non_empty_text_fields
        ),
        "base PredefinedData row preserves type UUID slots, parent reference slots, row value count, child table counts and trailing fields that are not fully represented in Predefined.xml".to_string(),
    ])
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

    replacements.sort_by_key(|b| std::cmp::Reverse(b.0.start));
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

pub fn business_process_flowchart_base_free_blockers(xml: &[u8]) -> Result<Vec<String>> {
    let items = parse_flowchart_xml(xml)?;
    let mut kind_counts = BTreeMap::<String, usize>::new();
    let mut event_count = 0usize;
    let mut explanation_count = 0usize;
    let mut task_description_count = 0usize;
    for item in &items {
        *kind_counts.entry(item.kind.clone()).or_default() += 1;
        event_count += item.events.len();
        if item.explanation.is_some() {
            explanation_count += 1;
        }
        if item.task_description.is_some() {
            task_description_count += 1;
        }
    }
    let kind_summary = if kind_counts.is_empty() {
        "none".to_string()
    } else {
        kind_counts
            .iter()
            .map(|(kind, count)| format!("{kind}={count}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut blockers = Vec::new();
    blockers.push(format!(
        "source XML has {} flowchart items ({kind_summary}) addressed by id/name, but the Config row owns the serialized item table order and per-item type code slots",
        items.len()
    ));
    blockers.push(
        "source XML carries editable values only; staging currently patches the existing base item payloads to preserve platform-owned geometry, connector, and nested field shapes"
            .to_string(),
    );
    if event_count > 0 {
        blockers.push(format!(
            "source XML has {event_count} event handlers, but handler positions are selected from the base event slot table for each flowchart item type"
        ));
    }
    if explanation_count > 0 || task_description_count > 0 {
        blockers.push(format!(
            "source XML has {explanation_count} explanations and {task_description_count} task descriptions, but their serialized field positions are preserved from the base blob"
        ));
    }
    Ok(blockers)
}

fn role_right_value_replacements(
    plain: &str,
    objects_range: Range<usize>,
    objects: &[RoleObjectRightsXml],
    source: Option<&MetadataSourceContext>,
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

    let mut base_entries = Vec::with_capacity(base_count);
    for object_index in 0..base_count {
        let entry_range = object_fields[object_index + 1].clone();
        let entry_fields = scan_braced_fields(plain, entry_range.start)?;
        let object_ref_range = entry_fields.first().ok_or_else(|| {
            anyhow!("base Role object rights entry {object_index} has no object reference")
        })?;
        validate_role_object_ref(plain, object_ref_range.clone())
            .with_context(|| format!("base Role object rights entry {object_index}"))?;
        let rights_range = entry_fields.get(1).ok_or_else(|| {
            anyhow!("base Role object rights entry {object_index} has no rights payload")
        })?;
        let key = source
            .map(|_| role_object_ref_key(plain, object_ref_range.clone()))
            .transpose()?;
        base_entries.push((key, rights_range.clone()));
    }

    let mut replacements = Vec::new();
    if let Some(source) = source {
        let mut objects_by_key = role_objects_by_ref_key(objects, source)?;
        for (object_index, (key, rights_range)) in base_entries.into_iter().enumerate() {
            let key = key.ok_or_else(|| {
                anyhow!("base Role object rights entry {object_index} has no key")
            })?;
            let object = objects_by_key
                .get_mut(&key)
                .and_then(VecDeque::pop_front)
                .ok_or_else(|| {
                    anyhow!("Role Rights.xml has no object matching base entry {object_index}")
                })?;
            replacements.extend(role_object_right_value_replacements(
                plain,
                rights_range,
                object,
            )?);
        }
    } else {
        for (object_index, (_, rights_range)) in base_entries.into_iter().enumerate() {
            let object = objects.get(object_index).ok_or_else(|| {
                anyhow!("Role Rights.xml has no object for base entry {object_index}")
            })?;
            replacements.extend(role_object_right_value_replacements(
                plain,
                rights_range,
                object,
            )?);
        }
    }
    Ok(replacements)
}

fn validate_role_object_ref(plain: &str, object_ref_range: Range<usize>) -> Result<()> {
    let fields = scan_braced_fields(plain, object_ref_range.start)?;
    let uuid_range = fields
        .get(1)
        .ok_or_else(|| anyhow!("object reference has no UUID field"))?;
    let uuid = plain[uuid_range.clone()].trim();
    if !is_uuid_like_ascii(uuid.as_bytes()) {
        return Err(anyhow!("object reference has invalid UUID {uuid}"));
    }
    Ok(())
}

fn role_object_ref_key(plain: &str, object_ref_range: Range<usize>) -> Result<RoleObjectRefKey> {
    let fields = scan_braced_fields(plain, object_ref_range.start)?;
    let uuid_range = fields
        .get(1)
        .ok_or_else(|| anyhow!("object reference has no UUID field"))?;
    let uuid = plain[uuid_range.clone()].trim();
    if !is_uuid_like_ascii(uuid.as_bytes()) {
        return Err(anyhow!("object reference has invalid UUID {uuid}"));
    }
    Ok(RoleObjectRefKey {
        uuid: uuid.to_string(),
        kind_code: fields
            .get(2)
            .map(|range| plain[range.clone()].trim().to_string()),
        slot_code: fields
            .get(3)
            .map(|range| plain[range.clone()].trim().to_string()),
    })
}

fn role_objects_by_ref_key<'a>(
    objects: &'a [RoleObjectRightsXml],
    source: &MetadataSourceContext,
) -> Result<BTreeMap<RoleObjectRefKey, VecDeque<&'a RoleObjectRightsXml>>> {
    let mut objects_by_key = BTreeMap::<RoleObjectRefKey, VecDeque<&RoleObjectRightsXml>>::new();
    for object in objects {
        let key = resolve_role_object_ref_key(&object.name, source)
            .with_context(|| format!("failed to resolve Role Rights.xml object {}", object.name))?;
        objects_by_key.entry(key).or_default().push_back(object);
    }
    Ok(objects_by_key)
}

fn resolve_role_object_ref_key(
    reference: &str,
    source: &MetadataSourceContext,
) -> Result<RoleObjectRefKey> {
    if let Some((owner_reference, attribute)) = reference.split_once(".StandardAttribute.") {
        let uuid = source.resolve_metadata_reference_uuid(owner_reference)?;
        let slot = role_standard_attribute_slot(owner_reference, attribute)
            .ok_or_else(|| anyhow!("unsupported Role standard attribute reference: {reference}"))?;
        return Ok(RoleObjectRefKey {
            uuid,
            kind_code: Some("1".to_string()),
            slot_code: Some(slot.to_string()),
        });
    }
    if reference.contains(".Command.") {
        return Ok(RoleObjectRefKey::direct(
            source.resolve_command_reference_uuid(reference)?,
        ));
    }
    if let Some((service_reference, template_name, method_name)) =
        role_http_service_method_reference(reference)
    {
        return Ok(RoleObjectRefKey::direct(
            source.resolve_http_service_method_uuid(
                service_reference,
                template_name,
                method_name,
            )?,
        ));
    }
    if let Some((owner_reference, child_kind, child_name)) = role_child_object_reference(reference)
    {
        return Ok(RoleObjectRefKey::direct(
            source.resolve_metadata_child_uuid(owner_reference, child_kind, child_name)?,
        ));
    }
    if reference.starts_with("Configuration.") {
        return Ok(RoleObjectRefKey::direct(
            source.resolve_configuration_uuid(reference)?,
        ));
    }
    Ok(RoleObjectRefKey::direct(
        source.resolve_metadata_reference_uuid(reference)?,
    ))
}

impl RoleObjectRefKey {
    fn direct(uuid: String) -> Self {
        Self {
            uuid,
            kind_code: Some("0".to_string()),
            slot_code: Some("0".to_string()),
        }
    }
}

fn role_standard_attribute_slot(owner_reference: &str, attribute: &str) -> Option<usize> {
    let kind = owner_reference.split_once('.')?.0;
    let attributes: &[&str] = match kind {
        "Catalog" => &[
            "PredefinedDataName",
            "Predefined",
            "Ref",
            "DeletionMark",
            "IsFolder",
            "Owner",
            "Parent",
            "Description",
            "Code",
        ],
        "Document" => &["Posted", "Ref", "DeletionMark", "Date", "Number"],
        "ExchangePlan" => &[
            "ReceivedNo",
            "SentNo",
            "Ref",
            "DeletionMark",
            "Description",
            "Code",
        ],
        "AccumulationRegister"
        | "AccountingRegister"
        | "CalculationRegister"
        | "InformationRegister" => &["Active", "Period", "Recorder", "LineNumber"],
        _ => return None,
    };
    attributes.iter().position(|name| *name == attribute)
}

fn role_http_service_method_reference(reference: &str) -> Option<(&str, &str, &str)> {
    let parts = reference.split('.').collect::<Vec<_>>();
    if parts.len() == 6
        && parts[0] == "HTTPService"
        && parts[2] == "URLTemplate"
        && parts[4] == "Method"
    {
        return Some((
            &reference[..parts[0].len() + 1 + parts[1].len()],
            parts[3],
            parts[5],
        ));
    }
    None
}

fn role_child_object_reference(reference: &str) -> Option<(&str, &str, &str)> {
    let parts = reference.split('.').collect::<Vec<_>>();
    if parts.len() == 4 && role_child_object_tag(parts[2]).is_some() {
        return Some((
            &reference[..parts[0].len() + 1 + parts[1].len()],
            parts[2],
            parts[3],
        ));
    }
    if parts.len() == 6
        && parts[2] == "TabularSection"
        && parts[4] == "Attribute"
        && role_child_object_tag(parts[4]).is_some()
    {
        return Some((
            &reference[..parts[0].len() + 1 + parts[1].len()],
            parts[4],
            parts[5],
        ));
    }
    None
}

fn role_child_object_tag(tag: &str) -> Option<&'static str> {
    match tag {
        "AddressingAttribute" => Some("AddressingAttribute"),
        "Attribute" => Some("Attribute"),
        "Dimension" => Some("Dimension"),
        "Form" => Some("Form"),
        "Resource" => Some("Resource"),
        "URLTemplate" => Some("URLTemplate"),
        "Operation" => Some("Operation"),
        _ => None,
    }
}

impl MetadataSourceContext {
    fn resolve_configuration_uuid(&self, reference: &str) -> Result<String> {
        let expected_name = reference
            .trim()
            .strip_prefix("Configuration.")
            .ok_or_else(|| anyhow!("unsupported Configuration reference: {reference}"))?;
        let path = self.source_root.join("Configuration.xml");
        let xml = self
            .read_source(&path)
            .with_context(|| format!("failed to read Configuration XML {}", path.display()))?;
        let properties = parse_simple_metadata_xml_properties(&xml)?;
        if properties.kind != "Configuration" || properties.name != expected_name {
            return Err(anyhow!(
                "expected Configuration.{expected_name} at {}, got {}.{}",
                path.display(),
                properties.kind,
                properties.name
            ));
        }
        Ok(properties.uuid)
    }

    fn resolve_metadata_child_uuid(
        &self,
        owner_reference: &str,
        child_kind: &str,
        child_name: &str,
    ) -> Result<String> {
        let (prefix, folder) =
            metadata_reference_source_folder(owner_reference).ok_or_else(|| {
                anyhow!(
                    "unsupported child owner reference for source resolution: {owner_reference}"
                )
            })?;
        let owner_name = owner_reference
            .strip_prefix(&format!("{prefix}."))
            .ok_or_else(|| anyhow!("invalid child owner reference: {owner_reference}"))?;
        let path = self
            .source_root
            .join(folder)
            .join(format!("{owner_name}.xml"));
        let xml = self
            .read_source(&path)
            .with_context(|| format!("failed to read child owner XML {}", path.display()))?;
        parse_nested_metadata_child_uuid_from_xml(&xml, child_kind, child_name).with_context(|| {
            format!("failed to resolve child {owner_reference}.{child_kind}.{child_name}")
        })
    }

    fn resolve_http_service_method_uuid(
        &self,
        service_reference: &str,
        template_name: &str,
        method_name: &str,
    ) -> Result<String> {
        let (_, folder) = metadata_reference_source_folder(service_reference).ok_or_else(|| {
            anyhow!("unsupported HTTPService reference for source resolution: {service_reference}")
        })?;
        let service_name = service_reference
            .strip_prefix("HTTPService.")
            .ok_or_else(|| anyhow!("invalid HTTPService reference: {service_reference}"))?;
        let path = self
            .source_root
            .join(folder)
            .join(format!("{service_name}.xml"));
        let xml = self
            .read_source(&path)
            .with_context(|| format!("failed to read HTTPService XML {}", path.display()))?;
        parse_http_service_method_uuid_from_xml(&xml, template_name, method_name).with_context(
            || {
                format!(
                    "failed to resolve HTTPService method {service_reference}.URLTemplate.{template_name}.Method.{method_name}"
                )
            },
        )
    }
}

fn parse_http_service_method_uuid_from_xml(
    xml: &[u8],
    template_name: &str,
    method_name: &str,
) -> Result<String> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut current_template_name = None::<String>;
    let mut current_method_uuid = None::<String>;
    let mut current_method_name = None::<String>;
    let mut collecting_template_name = false;
    let mut collecting_method_name = false;
    let mut text_value = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "URLTemplate" {
                    current_template_name = None;
                } else if local == "Method" {
                    current_method_uuid = xml_attr_value(&event, "uuid")
                        .map(|value| normalize_uuid_text(&value))
                        .transpose()?;
                    current_method_name = None;
                } else if local == "Name" && path_ends_with(&path, &["URLTemplate", "Properties"]) {
                    text_value.clear();
                    collecting_template_name = true;
                } else if local == "Name" && path_ends_with(&path, &["Method", "Properties"]) {
                    text_value.clear();
                    collecting_method_name = true;
                }
                path.push(local);
            }
            Ok(Event::Text(text)) if collecting_template_name || collecting_method_name => {
                text_value.push_str(text.xml_content()?.as_ref());
            }
            Ok(Event::CData(text)) if collecting_template_name || collecting_method_name => {
                text_value.push_str(text.xml_content()?.as_ref());
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if collecting_template_name && local == "Name" {
                    current_template_name = Some(text_value.trim().to_string());
                    collecting_template_name = false;
                    text_value.clear();
                } else if collecting_method_name && local == "Name" {
                    current_method_name = Some(text_value.trim().to_string());
                    collecting_method_name = false;
                    text_value.clear();
                }
                if local == "Method" {
                    if current_template_name.as_deref() == Some(template_name)
                        && current_method_name.as_deref() == Some(method_name)
                        && let Some(uuid) = current_method_uuid.take()
                    {
                        return Ok(uuid);
                    }
                    current_method_uuid = None;
                    current_method_name = None;
                } else if local == "URLTemplate" {
                    current_template_name = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Err(anyhow!(
        "HTTPService URLTemplate {template_name} Method {method_name} not found in XML"
    ))
}

fn parse_nested_metadata_child_uuid_from_xml(
    xml: &[u8],
    child_kind: &str,
    child_name: &str,
) -> Result<String> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut current_uuid = None::<String>;
    let mut current_name = None::<String>;
    let mut text_value = String::new();
    let mut collecting_name = false;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == child_kind {
                    current_uuid = xml_attr_value(&event, "uuid")
                        .map(|value| normalize_uuid_text(&value))
                        .transpose()?;
                    current_name = None;
                } else if current_uuid.is_some()
                    && local == "Name"
                    && path_ends_with(&path, &[child_kind, "Properties"])
                {
                    text_value.clear();
                    collecting_name = true;
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == child_kind {
                    current_uuid = None;
                    current_name = None;
                }
            }
            Ok(Event::Text(text)) if collecting_name => {
                text_value.push_str(text.xml_content()?.as_ref());
            }
            Ok(Event::CData(text)) if collecting_name => {
                text_value.push_str(text.xml_content()?.as_ref());
            }
            Ok(Event::End(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if collecting_name && local == "Name" {
                    current_name = Some(text_value.trim().to_string());
                    collecting_name = false;
                    text_value.clear();
                }
                if local == child_kind {
                    if current_name.as_deref() == Some(child_name)
                        && let Some(uuid) = current_uuid.take()
                    {
                        return Ok(uuid);
                    }
                    current_uuid = None;
                    current_name = None;
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    Err(anyhow!("{child_kind} {child_name} not found in owner XML"))
}

fn role_restriction_template_value_replacements(
    plain: &str,
    templates_range: Range<usize>,
    templates: &[RoleRestrictionTemplateXml],
) -> Result<Vec<(Range<usize>, String)>> {
    let fields = scan_braced_fields(plain, templates_range.start)?;
    let base_count_range = fields
        .first()
        .ok_or_else(|| anyhow!("base Role restriction templates field is empty"))?
        .clone();
    let base_count = plain[base_count_range]
        .trim()
        .parse::<usize>()
        .context("invalid base Role restriction templates count")?;
    if base_count != templates.len() {
        return Err(anyhow!(
            "Role Rights.xml restriction template count {} does not match base blob count {}",
            templates.len(),
            base_count
        ));
    }
    if fields.len() != base_count + 1 {
        return Err(anyhow!(
            "base Role restriction templates field count {} does not match declared count {}",
            fields.len().saturating_sub(1),
            base_count
        ));
    }

    let mut replacements = Vec::with_capacity(base_count * 2);
    for (template_index, template) in templates.iter().enumerate() {
        let entry_range = fields[template_index + 1].clone();
        let entry_fields = scan_braced_fields(plain, entry_range.start)?;
        if entry_fields.len() != 2 {
            return Err(anyhow!(
                "base Role restriction template entry {template_index} has {} fields, expected 2",
                entry_fields.len()
            ));
        }
        parse_1c_quoted_string(plain[entry_fields[0].clone()].trim()).with_context(|| {
            format!("base Role restriction template entry {template_index} has invalid name")
        })?;
        parse_1c_quoted_string(plain[entry_fields[1].clone()].trim()).with_context(|| {
            format!("base Role restriction template entry {template_index} has invalid condition")
        })?;
        replacements.push((entry_fields[0].clone(), format_1c_string(&template.name)));
        replacements.push((
            entry_fields[1].clone(),
            format_1c_string(&template.condition),
        ));
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
    let required_fields = start + count * 2;
    if fields.len() < required_fields {
        return Err(anyhow!(
            "base Role rights payload has {} fields, expected at least {}",
            fields.len(),
            required_fields
        ));
    }
    let rights_by_name = object
        .rights
        .iter()
        .map(|right| (right.name.as_str(), right))
        .collect::<BTreeMap<_, _>>();
    let mut restrictions_by_uuid = BTreeMap::<String, RoleRightRestrictionXml>::new();
    let mut replacements = Vec::with_capacity(count);
    for right_index in 0..count {
        let uuid = plain[fields[start + right_index * 2].clone()].trim();
        let name = role_right_name(uuid)
            .ok_or_else(|| anyhow!("unsupported base Role right UUID {uuid}"))?;
        let right = rights_by_name.get(name).copied();
        let value_range = fields[start + right_index * 2 + 1].clone();
        replacements.push((
            value_range,
            if right.is_some_and(|right| right.value) {
                "1"
            } else {
                "-1"
            }
            .to_string(),
        ));
        if let Some(restriction) = right.and_then(|right| right.restriction_by_condition.as_ref()) {
            restrictions_by_uuid.insert(uuid.to_string(), restriction.clone());
        }
    }
    if marker == "1" && !restrictions_by_uuid.is_empty() {
        let restrictions_count_range = fields
            .get(required_fields)
            .ok_or_else(|| {
                anyhow!("base Role restricted rights payload has no restrictions count")
            })?
            .clone();
        let restrictions_count = plain[restrictions_count_range]
            .trim()
            .parse::<usize>()
            .context("invalid base Role restrictions count")?;
        if restrictions_count > 0 {
            replacements.extend(role_restriction_condition_replacements(
                plain,
                restrictions_count,
                &fields[required_fields + 1..],
                &restrictions_by_uuid,
            )?);
        }
    }
    Ok(replacements)
}

fn role_restriction_condition_replacements(
    plain: &str,
    count: usize,
    ranges: &[Range<usize>],
    restrictions_by_uuid: &BTreeMap<String, RoleRightRestrictionXml>,
) -> Result<Vec<(Range<usize>, String)>> {
    let mut replacements = Vec::new();
    if ranges.len() == count {
        for range in ranges {
            collect_role_restriction_entry_replacement(
                plain,
                range.clone(),
                restrictions_by_uuid,
                &mut replacements,
            )?;
        }
        return Ok(replacements);
    }
    if ranges.len() != 1 {
        return Ok(replacements);
    }

    let nested = scan_braced_fields(plain, ranges[0].start)?;
    if nested.len() == count * 2
        && nested
            .first()
            .is_some_and(|range| is_uuid_like_ascii(plain[range.clone()].trim().as_bytes()))
    {
        for pair in nested.chunks(2) {
            collect_role_restriction_pair_replacement(
                plain,
                pair[0].clone(),
                pair[1].clone(),
                restrictions_by_uuid,
                &mut replacements,
            )?;
        }
        return Ok(replacements);
    }
    if nested.len() == count {
        for range in nested {
            collect_role_restriction_entry_replacement(
                plain,
                range,
                restrictions_by_uuid,
                &mut replacements,
            )?;
        }
    }
    Ok(replacements)
}

fn collect_role_restriction_entry_replacement(
    plain: &str,
    entry_range: Range<usize>,
    restrictions_by_uuid: &BTreeMap<String, RoleRightRestrictionXml>,
    replacements: &mut Vec<(Range<usize>, String)>,
) -> Result<()> {
    let fields = scan_braced_fields(plain, entry_range.start)?;
    if fields.len() != 2 {
        return Ok(());
    }
    collect_role_restriction_pair_replacement(
        plain,
        fields[0].clone(),
        fields[1].clone(),
        restrictions_by_uuid,
        replacements,
    )
}

fn collect_role_restriction_pair_replacement(
    plain: &str,
    uuid_range: Range<usize>,
    restriction_range: Range<usize>,
    restrictions_by_uuid: &BTreeMap<String, RoleRightRestrictionXml>,
    replacements: &mut Vec<(Range<usize>, String)>,
) -> Result<()> {
    let uuid = plain[uuid_range].trim();
    let Some(restriction) = restrictions_by_uuid.get(uuid) else {
        return Ok(());
    };
    if let Some(ranges) = role_restriction_condition_ranges(plain, restriction_range)? {
        replacements.push((ranges.condition, format_1c_string(&restriction.condition)));
        if let (Some(field), Some(field_range)) = (&restriction.field, ranges.field) {
            replacements.push((field_range, format_1c_string(field)));
        }
    }
    Ok(())
}

struct RoleRestrictionConditionRanges {
    condition: Range<usize>,
    field: Option<Range<usize>>,
}

fn role_restriction_condition_ranges(
    plain: &str,
    restriction_range: Range<usize>,
) -> Result<Option<RoleRestrictionConditionRanges>> {
    let wrapper = scan_braced_fields(plain, restriction_range.start)?;
    let marker = wrapper.first().map(|range| plain[range.clone()].trim());
    let body_range = match marker {
        Some("1") => wrapper.get(1),
        Some("2") => wrapper.get(2),
        _ => return Ok(None),
    };
    let Some(body_range) = body_range else {
        return Ok(None);
    };
    let body = scan_braced_fields(plain, body_range.start)?;
    if body.first().map(|range| plain[range.clone()].trim()) != Some("1") {
        return Ok(None);
    }
    let Some(condition_range) = body.get(1) else {
        return Ok(None);
    };
    parse_1c_quoted_string(plain[condition_range.clone()].trim())?;
    let field = if marker == Some("2") {
        body.get(3)
            .map(|range| role_restriction_field_text_range(plain, range.clone()))
            .transpose()?
            .flatten()
    } else {
        None
    };
    Ok(Some(RoleRestrictionConditionRanges {
        condition: condition_range.clone(),
        field,
    }))
}

fn role_restriction_field_text_range(
    plain: &str,
    field_range: Range<usize>,
) -> Result<Option<Range<usize>>> {
    if parse_1c_quoted_string(plain[field_range.clone()].trim()).is_ok() {
        return Ok(Some(field_range));
    }
    if plain.as_bytes().get(field_range.start) != Some(&b'{') {
        return Ok(None);
    }
    let fields = scan_braced_fields(plain, field_range.start)?;
    if fields.first().map(|range| plain[range.clone()].trim()) == Some("1")
        && let Some(value_range) = fields.get(1)
        && parse_1c_quoted_string(plain[value_range.clone()].trim()).is_ok()
    {
        return Ok(Some(value_range.clone()));
    }
    for range in fields {
        if plain.as_bytes().get(range.start) == Some(&b'{')
            && let Some(nested) = role_restriction_field_text_range(plain, range)?
        {
            return Ok(Some(nested));
        }
    }
    Ok(None)
}

/// The name a right uuid stands for: the exporter's table, the one the
/// platform's own `Rights.xml` files were measured against.
fn role_right_name(uuid: &str) -> Option<&'static str> {
    ibcmd_1c_vocab::role_right_name(uuid)
}

#[doc(hidden)]
pub fn parse_role_rights_xml(xml: &[u8]) -> Result<RoleRightsXml> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut set_for_new_objects = None::<bool>;
    let mut set_for_attributes_by_default = None::<bool>;
    let mut independent_rights_of_child_objects = None::<bool>;
    let mut objects = Vec::<RoleObjectRightsXml>::new();
    let mut restriction_templates = Vec::<RoleRestrictionTemplateXml>::new();
    let mut current_object = None::<RoleObjectRightsXml>;
    let mut current_right = None::<RoleRightXml>;
    let mut current_restriction_template = None::<RoleRestrictionTemplateXml>;
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
                        restriction_by_condition: None,
                    });
                } else if local == "restrictionTemplate" && path_ends_with(&path, &["Rights"]) {
                    current_restriction_template = Some(RoleRestrictionTemplateXml {
                        name: String::new(),
                        condition: String::new(),
                    });
                }
                if matches!(
                    local.as_str(),
                    "setForNewObjects"
                        | "setForAttributesByDefault"
                        | "independentRightsOfChildObjects"
                        | "name"
                        | "value"
                        | "field"
                        | "condition"
                ) {
                    text_value.clear();
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "setForNewObjects" {
                    set_for_new_objects = Some(false);
                } else if local == "setForAttributesByDefault" {
                    set_for_attributes_by_default = Some(false);
                } else if local == "independentRightsOfChildObjects" {
                    independent_rights_of_child_objects = Some(false);
                } else if local == "condition"
                    && path_ends_with(
                        &path,
                        &["Rights", "object", "right", "restrictionByCondition"],
                    )
                    && let Some(right) = current_right.as_mut()
                {
                    right.restriction_by_condition = Some(RoleRightRestrictionXml {
                        field: None,
                        condition: String::new(),
                    });
                } else if local == "condition"
                    && path_ends_with(&path, &["Rights", "restrictionTemplate"])
                    && let Some(template) = current_restriction_template.as_mut()
                {
                    template.condition = String::new();
                }
            }
            Ok(Event::Text(text)) => {
                if path_ends_with(&path, &["Rights", "setForNewObjects"])
                    || path_ends_with(&path, &["Rights", "setForAttributesByDefault"])
                    || path_ends_with(&path, &["Rights", "independentRightsOfChildObjects"])
                    || path_ends_with(&path, &["Rights", "object", "name"])
                    || path_ends_with(&path, &["Rights", "object", "right", "name"])
                    || path_ends_with(&path, &["Rights", "object", "right", "value"])
                    || path_ends_with(
                        &path,
                        &[
                            "Rights",
                            "object",
                            "right",
                            "restrictionByCondition",
                            "field",
                        ],
                    )
                    || path_ends_with(
                        &path,
                        &[
                            "Rights",
                            "object",
                            "right",
                            "restrictionByCondition",
                            "condition",
                        ],
                    )
                    || path_ends_with(&path, &["Rights", "restrictionTemplate", "name"])
                    || path_ends_with(&path, &["Rights", "restrictionTemplate", "condition"])
                {
                    text_value.push_str(text.xml_content()?.as_ref());
                }
            }
            Ok(Event::CData(text)) => {
                if path_ends_with(&path, &["Rights", "setForNewObjects"])
                    || path_ends_with(&path, &["Rights", "setForAttributesByDefault"])
                    || path_ends_with(&path, &["Rights", "independentRightsOfChildObjects"])
                    || path_ends_with(&path, &["Rights", "object", "name"])
                    || path_ends_with(&path, &["Rights", "object", "right", "name"])
                    || path_ends_with(&path, &["Rights", "object", "right", "value"])
                    || path_ends_with(
                        &path,
                        &[
                            "Rights",
                            "object",
                            "right",
                            "restrictionByCondition",
                            "field",
                        ],
                    )
                    || path_ends_with(
                        &path,
                        &[
                            "Rights",
                            "object",
                            "right",
                            "restrictionByCondition",
                            "condition",
                        ],
                    )
                    || path_ends_with(&path, &["Rights", "restrictionTemplate", "name"])
                    || path_ends_with(&path, &["Rights", "restrictionTemplate", "condition"])
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
                    "setForAttributesByDefault"
                        if path_ends_with(&path, &["Rights", "setForAttributesByDefault"]) =>
                    {
                        set_for_attributes_by_default = Some(parse_xml_bool_text(
                            "Role/setForAttributesByDefault",
                            text_value.trim(),
                        )?);
                    }
                    "independentRightsOfChildObjects"
                        if path_ends_with(
                            &path,
                            &["Rights", "independentRightsOfChildObjects"],
                        ) =>
                    {
                        independent_rights_of_child_objects = Some(parse_xml_bool_text(
                            "Role/independentRightsOfChildObjects",
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
                    "field"
                        if path_ends_with(
                            &path,
                            &[
                                "Rights",
                                "object",
                                "right",
                                "restrictionByCondition",
                                "field",
                            ],
                        ) =>
                    {
                        if let Some(right) = current_right.as_mut() {
                            let restriction =
                                right.restriction_by_condition.get_or_insert_with(|| {
                                    RoleRightRestrictionXml {
                                        field: None,
                                        condition: String::new(),
                                    }
                                });
                            restriction.field = Some(text_value.clone());
                        }
                    }
                    "condition"
                        if path_ends_with(
                            &path,
                            &[
                                "Rights",
                                "object",
                                "right",
                                "restrictionByCondition",
                                "condition",
                            ],
                        ) =>
                    {
                        if let Some(right) = current_right.as_mut() {
                            let restriction =
                                right.restriction_by_condition.get_or_insert_with(|| {
                                    RoleRightRestrictionXml {
                                        field: None,
                                        condition: String::new(),
                                    }
                                });
                            restriction.condition = text_value.clone();
                        }
                    }
                    "name" if path_ends_with(&path, &["Rights", "restrictionTemplate", "name"]) => {
                        if let Some(template) = current_restriction_template.as_mut() {
                            template.name = text_value.clone();
                        }
                    }
                    "condition"
                        if path_ends_with(
                            &path,
                            &["Rights", "restrictionTemplate", "condition"],
                        ) =>
                    {
                        if let Some(template) = current_restriction_template.as_mut() {
                            template.condition = text_value.clone();
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
                    "restrictionTemplate"
                        if path_ends_with(&path, &["Rights", "restrictionTemplate"]) =>
                    {
                        let template = current_restriction_template.take().ok_or_else(|| {
                            anyhow!("Rights.xml ended restrictionTemplate without active template")
                        })?;
                        if template.name.is_empty() {
                            return Err(anyhow!(
                                "Rights.xml contains restrictionTemplate without name"
                            ));
                        }
                        restriction_templates.push(template);
                    }
                    _ => {}
                }
                let _ = path.pop();
                if matches!(
                    local.as_str(),
                    "setForNewObjects"
                        | "setForAttributesByDefault"
                        | "independentRightsOfChildObjects"
                        | "name"
                        | "value"
                        | "field"
                        | "condition"
                ) {
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
        set_for_attributes_by_default,
        independent_rights_of_child_objects,
        objects,
        restriction_templates,
    })
}

#[doc(hidden)]
pub fn parse_command_interface_xml(xml: &[u8]) -> Result<Vec<CommandInterfaceXmlEntry>> {
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
    let mut auto_record = None::<u8>;
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

fn parse_exchange_plan_auto_record_text(value: &str) -> Result<u8> {
    match value {
        "Deny" => Ok(0),
        "Allow" => Ok(1),
        "Auto" => Ok(2),
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

fn predefined_data_xml_stats(items: &[PredefinedDataXmlItem]) -> PredefinedDataXmlStats {
    if items.is_empty() {
        return PredefinedDataXmlStats::default();
    }
    let mut stats = PredefinedDataXmlStats {
        root_items: items.len(),
        ..Default::default()
    };
    collect_predefined_data_xml_stats(items, 1, &mut stats);
    stats
}

fn collect_predefined_data_xml_stats(
    items: &[PredefinedDataXmlItem],
    depth: usize,
    stats: &mut PredefinedDataXmlStats,
) {
    stats.max_depth = stats.max_depth.max(depth);
    for item in items {
        stats.items += 1;
        stats.folders += usize::from(item.is_folder);
        stats.nested_child_links += item.children.len();
        stats.editable_value_fields += 4;
        stats.non_empty_text_fields += usize::from(!item.name.is_empty());
        stats.non_empty_text_fields += usize::from(!item.code.is_empty());
        stats.non_empty_text_fields += usize::from(!item.description.is_empty());
        collect_predefined_data_xml_stats(&item.children, depth + 1, stats);
    }
}

fn collect_predefined_replacements(
    plain: &str,
    table_range: Range<usize>,
    by_id: &BTreeMap<String, PredefinedDataXmlItem>,
    seen: &mut BTreeSet<String>,
    base_ids: &mut BTreeSet<String>,
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
                    base_ids,
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
    base_ids: &mut BTreeSet<String>,
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
        collect_predefined_item_replacements(
            plain,
            item_range,
            by_id,
            seen,
            base_ids,
            replacements,
        )?;
    }
    Ok(())
}

fn collect_predefined_item_replacements(
    plain: &str,
    item_range: Range<usize>,
    by_id: &BTreeMap<String, PredefinedDataXmlItem>,
    seen: &mut BTreeSet<String>,
    base_ids: &mut BTreeSet<String>,
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
    base_ids.insert(id.clone());
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
            base_ids,
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
    format!(r#"{{"B",{}}}"#, if value { "1" } else { "0" })
}

fn format_predefined_string_value(value: &str) -> String {
    format!(r#"{{"S",{}}}"#, format_1c_string(value))
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
            // An entity (`&lt;85%`) arrives as its own event; dropping it lost
            // the `<` / `>` of 28 ERP УХ predefined descriptions.
            Ok(Event::GeneralRef(reference)) => {
                if is_predefined_item_property_path(&path) {
                    let value = if let Some(ch) = reference.resolve_char_ref()? {
                        ch.to_string()
                    } else {
                        let entity = reference.decode()?;
                        resolve_xml_entity(entity.as_ref())
                            .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?
                            .to_string()
                    };
                    text_value.push_str(&value);
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
                            // Kept as written: a trailing space is data (ERP
                            // УХ `ЭлементыКонструктораВидовПродукцииИС`).
                            "Code" => item.code = text_value.clone(),
                            "Description" => item.description = text_value.clone(),
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
                    &crlf_line_breaks(explanation),
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
                    &crlf_line_breaks(task_description),
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
    // Every item but a connection line (1) and a decoration (0) wraps its
    // base record `{4,<id>,<title>,<name>,<tab order>}` one level deeper:
    // start, end, condition, activity, the nested business process (10),
    // and the merge, split and processing points (7, 8, 9) ERP УХ's
    // СогласованиеПродажи and its siblings carry. Read the shape, not the code.
    let wrapped = head_fields
        .first()
        .is_some_and(|range| range_starts_with_brace(plain, range));
    let _ = code;
    let base_fields = if wrapped {
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
                        kind: local.clone(),
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
            Ok(Event::GeneralRef(reference)) => {
                if current.is_some()
                    && (path
                        .last()
                        .is_some_and(|part| is_flowchart_text_property(part))
                        || path.last().map(String::as_str) == Some("Event"))
                {
                    if let Some(ch) = reference.resolve_char_ref()? {
                        text_value.push(ch);
                    } else {
                        let entity = reference.decode()?;
                        text_value.push_str(
                            resolve_xml_entity(entity.as_ref())
                                .ok_or_else(|| anyhow!("unrecognized XML entity: {entity}"))?,
                        );
                    }
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
                        // Free text is kept as written: a trailing space or
                        // line break is data (three ERP УХ graphical schemes).
                        "Explanation" if path_ends_with(&path, &["Properties", "Explanation"]) => {
                            item.explanation = Some(text_value.clone());
                        }
                        "TaskDescription"
                            if path_ends_with(&path, &["Properties", "TaskDescription"]) =>
                        {
                            item.task_description = Some(text_value.clone());
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
                if is_flowchart_item_tag(&local)
                    && let Some(item) = current.take()
                {
                    items.push(item);
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
    pack_ext_picture_blob_from_bytes_with_base(None, bytes)
}

/// An ExtPicture row from its `Picture.xml` and picture bytes: the header
/// record carries the transparency the XML declares.
pub fn pack_ext_picture_blob_from_xml_and_bytes(
    xml: &[u8],
    bytes: &[u8],
) -> Result<PackedExtPictureBlob> {
    let header = ext_picture_header_from_xml(xml)?;
    // Laid out as the platform stores it: every nested record on its own line
    // and each list that ends with one closed on its own line.
    let plain = format!(
        "\u{feff}{{1,\r\n{header},\r\n{{\r\n{}\r\n}}\r\n}}",
        platform_base64_token(bytes)
    )
    .into_bytes();
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedExtPictureBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

/// The `{<load transparent>,0,<x>,<y>}` header an ExtPicture row opens with.
///
/// A pure partition over every common picture of both corpora (БСП 610 + 1,
/// ERP УХ 3 060 + 187): `LoadTransparent` false with no `TransparentPixel`
/// stores `{0,0,-1,-1}`, and `true` with a pixel stores `{1,0,x,y}`. The
/// extensions of the БСП 8.3.27 clone add one more shape: `true` with no pixel
/// stores `{1,0,-1,-1}` (6 of 6 pictures, the SVG and PNG pictures of
/// `ServiceDesk`). The remaining mixed shape (`false` with a pixel) never
/// occurs and is refused rather than guessed.
fn ext_picture_header_from_xml(xml: &[u8]) -> Result<String> {
    let text = std::str::from_utf8(xml).context("ExtPicture XML is not UTF-8")?;
    let load_transparent = match text
        .find("LoadTransparent>")
        .map(|start| &text[start + "LoadTransparent>".len()..])
        .and_then(|rest| rest.split('<').next())
        .map(str::trim)
    {
        None | Some("false") => false,
        Some("true") => true,
        Some(other) => return Err(anyhow!("unsupported ExtPicture LoadTransparent {other}")),
    };
    let pixel = match text.find("TransparentPixel") {
        None => None,
        Some(start) => {
            let rest = &text[start..];
            let element = &rest[..rest.find('>').unwrap_or(rest.len())];
            let attribute = |name: &str| -> Result<i64> {
                let key = format!("{name}=\"");
                let at = element
                    .find(&key)
                    .ok_or_else(|| anyhow!("ExtPicture TransparentPixel has no {name}"))?
                    + key.len();
                let end = element[at..]
                    .find('"')
                    .ok_or_else(|| anyhow!("ExtPicture TransparentPixel {name} is unterminated"))?;
                element[at..at + end]
                    .trim()
                    .parse::<i64>()
                    .with_context(|| format!("invalid ExtPicture TransparentPixel {name}"))
            };
            Some((attribute("x")?, attribute("y")?))
        }
    };
    match (load_transparent, pixel) {
        (false, None) => Ok("{0,0,-1,-1}".to_string()),
        (true, None) => Ok("{1,0,-1,-1}".to_string()),
        (true, Some((x, y))) => Ok(format!("{{1,0,{x},{y}}}")),
        (load_transparent, pixel) => Err(anyhow!(
            "unobserved ExtPicture transparency: LoadTransparent {load_transparent}, TransparentPixel {pixel:?}"
        )),
    }
}

pub fn pack_ext_picture_blob_from_bytes_with_base(
    base_blob: Option<&[u8]>,
    bytes: &[u8],
) -> Result<PackedExtPictureBlob> {
    let payload = encode_base64(bytes);
    let plain = if let Some(base_blob) = base_blob {
        let mut plain = String::from_utf8(
            inflate_raw(base_blob).context("failed to inflate base ExtPicture blob")?,
        )
        .context("base ExtPicture blob is not valid UTF-8")?;
        if replace_first_base64_payload(&mut plain, &payload) {
            plain.into_bytes()
        } else {
            format!("{{1,{{0,0,-1,-1}},{{{{#base64:{payload}}}}}}}").into_bytes()
        }
    } else {
        format!("{{1,{{0,0,-1,-1}},{{{{#base64:{payload}}}}}}}").into_bytes()
    };
    let blob = deflate_raw(&plain)?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedExtPictureBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

fn replace_first_base64_payload(text: &mut String, payload: &str) -> bool {
    let prefix = "{#base64:";
    let Some(start) = text.find(prefix) else {
        return false;
    };
    let payload_start = start + prefix.len();
    let Some(relative_end) = text[payload_start..].find('}') else {
        return false;
    };
    let payload_end = payload_start + relative_end;
    text.replace_range(payload_start..payload_end, payload);
    true
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
    // The platform's own layout, measured on every help row of both corpora
    // (БСП 641 + 1, ERP УХ 6 329 + 44 + 66 with files): the BOM, each
    // `{#base64:}` payload on its own line, and the closing brace on its own
    // line when the row ends with a file payload.
    let mut text = format!("\u{feff}{{5,{}", pages.len());
    for (page, content) in pages {
        text.push(',');
        text.push_str(&format_1c_string(page));
        text.push_str(",\r\n");
        text.push_str(&platform_base64_token(content));
    }
    text.push(',');
    text.push_str(&files.len().to_string());
    for (file_name, content) in files {
        text.push(',');
        text.push_str(&format_1c_string(file_name));
        text.push_str(",1,\r\n");
        text.push_str(&platform_base64_token(content));
    }
    text.push_str(if files.is_empty() { "}" } else { "\r\n}" });
    let plain = text.into_bytes();
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

/// The `<Schedule>` of a `JobSchedule` and its `<DetailedDailySchedules>`,
/// in document order.
fn parse_schedule_xml(xml: &[u8]) -> Result<(ScheduleXmlProperties, Vec<ScheduleXmlProperties>)> {
    #[derive(Default)]
    struct Pending {
        attrs: BTreeMap<String, String>,
        week_days: Option<Vec<String>>,
        months: Option<Vec<String>>,
    }
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut main = None::<Pending>;
    let mut detail = None::<Pending>;
    let mut details = Vec::<Pending>::new();
    let mut text_target = None::<String>;
    let mut text_value = String::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["JobSchedule"]) && local == "Schedule" {
                    main = Some(Pending {
                        attrs: xml_attrs_map(&event),
                        ..Pending::default()
                    });
                } else if path_ends_with(&path, &["JobSchedule", "Schedule"])
                    && local == "DetailedDailySchedules"
                {
                    detail = Some(Pending {
                        attrs: xml_attrs_map(&event),
                        ..Pending::default()
                    });
                } else if (path_ends_with(&path, &["JobSchedule", "Schedule"])
                    || path_ends_with(&path, &["Schedule", "DetailedDailySchedules"]))
                    && (local == "WeekDays" || local == "Months")
                {
                    text_target = Some(local.clone());
                    text_value.clear();
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &["JobSchedule", "Schedule"])
                    && local == "DetailedDailySchedules"
                {
                    details.push(Pending {
                        attrs: xml_attrs_map(&event),
                        ..Pending::default()
                    });
                }
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
                    let owner = if detail.is_some() {
                        detail.as_mut()
                    } else {
                        main.as_mut()
                    };
                    if let Some(owner) = owner {
                        if local == "WeekDays" {
                            owner.week_days = Some(values);
                        } else {
                            owner.months = Some(values);
                        }
                    }
                    text_target = None;
                    text_value.clear();
                } else if local == "DetailedDailySchedules"
                    && let Some(done) = detail.take()
                {
                    details.push(done);
                }
                let _ = path.pop();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    let build = |pending: Pending| -> Result<ScheduleXmlProperties> {
        let attrs = &pending.attrs;
        Ok(ScheduleXmlProperties {
            begin_date: required_schedule_attr(attrs, "BeginDate")?,
            end_date: required_schedule_attr(attrs, "EndDate")?,
            begin_time: required_schedule_attr(attrs, "BeginTime")?,
            end_time: required_schedule_attr(attrs, "EndTime")?,
            completion_time: required_schedule_attr(attrs, "CompletionTime")?,
            completion_interval: required_schedule_number_attr(attrs, "CompletionInterval")?,
            repeat_period_in_day: required_schedule_number_attr(attrs, "RepeatPeriodInDay")?,
            repeat_pause: required_schedule_number_attr(attrs, "RepeatPause")?,
            week_day_in_month: required_schedule_number_attr(attrs, "WeekDayInMonth")?,
            day_in_month: required_schedule_number_attr(attrs, "DayInMonth")?,
            week_days: pending.week_days.unwrap_or_default(),
            months: pending.months.unwrap_or_default(),
            weeks_period: required_schedule_number_attr(attrs, "WeeksPeriod")?,
            days_repeat_period: required_schedule_number_attr(attrs, "DaysRepeatPeriod")?,
        })
    };
    let main = main.ok_or_else(|| anyhow!("JobSchedule/Schedule element is missing"))?;
    let details = details.into_iter().map(build).collect::<Result<Vec<_>>>()?;
    Ok((build(main)?, details))
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
    let digits = value.strip_prefix('-').unwrap_or(value);
    if !digits.is_empty() && digits.chars().all(|ch| ch.is_ascii_digit()) {
        Ok(())
    } else {
        Err(anyhow!("invalid schedule number: {value}"))
    }
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
        /// `<xr:TransparentPixel x y/>`, stored in the two slots after the
        /// empty string (`-1,-1` when absent): ERP УХ `ПротоколОшибок`
        /// stores `{4,1,{0,<uuid>},"",12,2,1,0,""}`.
        pixel: Option<(i64, i64)>,
    },
    StdPictureCode {
        code: i32,
        load_transparent: bool,
        pixel: Option<(i64, i64)>,
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
        pixel: Option<(i64, i64)>,
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
    let mut date_fractions = None::<String>;
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
                    &mut date_fractions,
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
                    &mut date_fractions,
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
                    &mut date_fractions,
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
        date_fractions,
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
    let mut date_fractions = None::<String>;

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
                    &mut date_fractions,
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
                    &mut date_fractions,
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
                    &mut date_fractions,
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
        date_fractions,
        StringAllowedLengthCoding::VariableIsOne,
        source.map(|source| source as &dyn MetadataSource),
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
    let mut picture_pixel = None::<(i64, i64)>;
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
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "TransparentPixel"
                    && path_ends_with(&path, &["CommonCommand", "Properties", "Picture"])
                {
                    let attrs = xml_attrs_map(&event);
                    let coordinate = |name: &str| -> Result<i64> {
                        attrs
                            .get(name)
                            .ok_or_else(|| {
                                anyhow!("CommonCommand Picture TransparentPixel has no {name}")
                            })?
                            .trim()
                            .parse::<i64>()
                            .with_context(|| {
                                format!("invalid CommonCommand TransparentPixel {name}")
                            })
                    };
                    picture_pixel = Some((coordinate("x")?, coordinate("y")?));
                }
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
        picture: parse_common_command_picture(
            picture_ref,
            picture_load_transparent,
            picture_pixel,
            source,
        )?,
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
    let mut picture_pixel = None::<(i64, i64)>;
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
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if local == "TransparentPixel"
                    && path_ends_with(&path, &["CommandGroup", "Properties", "Picture"])
                {
                    let attrs = xml_attrs_map(&event);
                    let coordinate = |name: &str| -> Result<i64> {
                        attrs
                            .get(name)
                            .ok_or_else(|| {
                                anyhow!("CommandGroup Picture TransparentPixel has no {name}")
                            })?
                            .trim()
                            .parse::<i64>()
                            .with_context(|| {
                                format!("invalid CommandGroup TransparentPixel {name}")
                            })
                    };
                    picture_pixel = Some((coordinate("x")?, coordinate("y")?));
                }
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
        picture: parse_command_group_picture(
            picture_ref,
            picture_load_transparent,
            picture_pixel,
            source,
        )?,
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

#[derive(Debug, Default)]
struct MetadataXmlBaseFreeShape {
    direct_property_elements: usize,
    child_objects: usize,
    child_object_kinds: BTreeMap<String, usize>,
}

impl MetadataXmlBaseFreeShape {
    fn child_object_breakdown(&self) -> String {
        if self.child_object_kinds.is_empty() {
            return String::new();
        }
        let breakdown = self
            .child_object_kinds
            .iter()
            .map(|(kind, count)| format!("{kind}={count}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(" ({breakdown})")
    }
}

fn metadata_xml_base_free_shape(xml: &[u8], object_kind: &str) -> Result<MetadataXmlBaseFreeShape> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut shape = MetadataXmlBaseFreeShape::default();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &[object_kind, "Properties"]) {
                    shape.direct_property_elements += 1;
                }
                if path_ends_with(&path, &["ChildObjects"]) {
                    shape.child_objects += 1;
                    *shape.child_object_kinds.entry(local.clone()).or_default() += 1;
                }
                path.push(local);
            }
            Ok(Event::Empty(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path_ends_with(&path, &[object_kind, "Properties"]) {
                    shape.direct_property_elements += 1;
                }
                if path_ends_with(&path, &["ChildObjects"]) {
                    shape.child_objects += 1;
                    *shape.child_object_kinds.entry(local).or_default() += 1;
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

    Ok(shape)
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
                if local == "CommonModule"
                    && let Some(value) = xml_attr_value(&event, "uuid")
                {
                    uuid = Some(normalize_uuid_text(&value)?);
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

/// The root header's UUID is the application module group, while Version
/// lives in the configuration properties tuple. Updating only the header
/// silently kept the released extension version when cf load compiled XML.
fn patch_configuration_source_metadata(
    text: String,
    properties: &SimpleMetadataXmlProperties,
    xml: &[u8],
) -> Result<String> {
    use crate::xml::MetadataXml;
    use ibcmd_restructure::brace::{self, Brace};
    let patched = patch_configuration_metadata_text(text, properties)?;
    let document = MetadataXml::parse(xml)?;
    let Some(version) = document.object()?.path(&["Properties", "Version"]) else {
        return Ok(patched);
    };
    let mut row = brace::parse_row(patched.as_bytes())?;
    let mut tuple = &mut row;
    for index in [3, 1, 1] {
        tuple = tuple
            .as_list_mut()
            .and_then(|fields| fields.get_mut(index))
            .context("configuration properties tuple is missing")?;
    }
    let fields = tuple
        .as_list_mut()
        .context("configuration properties is not a tuple")?;
    let marker = fields.first().and_then(Brace::as_atom);
    if !matches!(marker, Some("63" | "68" | "76")) {
        anyhow::bail!(
            "configuration properties tuple {:?} has no evidenced Version slot",
            marker
        );
    }
    let stored = fields
        .get_mut(15)
        .context("configuration Version slot is missing")?;
    if stored.as_str().is_none() {
        anyhow::bail!("configuration Version slot is not a string");
    }
    *stored = Brace::str(&version.text);
    String::from_utf8(brace::serialize_row(&row)).context("configuration row is not UTF-8")
}

fn patch_configuration_metadata_text(
    text: String,
    properties: &SimpleMetadataXmlProperties,
) -> Result<String> {
    let marker = "{1,0,";
    let marker_start = text
        .find(marker)
        .ok_or_else(|| anyhow!("configuration metadata tuple not found"))?;
    let uuid_start = marker_start + marker.len();
    let uuid_end = uuid_start + 36;
    let header_uuid = text
        .get(uuid_start..uuid_end)
        .ok_or_else(|| anyhow!("configuration metadata tuple UUID is truncated"))?;
    if !is_uuid_like_ascii(header_uuid.as_bytes()) {
        return Err(anyhow!(
            "configuration metadata tuple UUID is invalid: {header_uuid}"
        ));
    }
    let header_uuid = header_uuid.to_string();
    patch_simple_metadata_header_text(
        text,
        &SimpleMetadataXmlProperties {
            kind: properties.kind.clone(),
            uuid: header_uuid,
            name: properties.name.clone(),
            synonyms: properties.synonyms.clone(),
            comment: properties.comment.clone(),
        },
    )
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
    if !metadata_type_pattern_matches_existing(
        &text[fields[2].clone()],
        &type_text,
        std::slice::from_ref(&properties.value_type),
    ) {
        text.replace_range(fields[2].clone(), &type_text);
    }

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
    if !metadata_type_pattern_matches_existing(
        &text[fields[4].clone()],
        &type_text,
        &properties.value_types,
    ) {
        text.replace_range(fields[4].clone(), &type_text);
    }

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

    text.replace_range(
        fields[12].clone(),
        &common_command_on_main_server_unavailable_behavior_code(
            properties.on_main_server_unavailable_behavior,
        )
        .to_string(),
    );
    text.replace_range(
        fields[11].clone(),
        &common_command_parameter_use_mode_code(properties.parameter_use_mode).to_string(),
    );
    text.replace_range(fields[10].clone(), &bool_flag(properties.modifies_data));
    if !matches!(
        properties.command_parameter_type,
        CommonCommandParameterType::Empty
    ) {
        replace_1c_value_if_different(
            &mut text,
            fields[8].clone(),
            &format_common_command_parameter_type(&properties.command_parameter_type),
        );
    }
    replace_1c_value_if_different(
        &mut text,
        fields[7].clone(),
        &format_common_command_group_reference(&properties.group),
    );
    text.replace_range(
        fields[6].clone(),
        &bool_flag(properties.include_help_in_contents),
    );
    replace_1c_value_if_different(
        &mut text,
        fields[3].clone(),
        &format_1c_synonyms(&properties.tooltip),
    );
    text.replace_range(
        fields[2].clone(),
        &common_command_representation_code(properties.representation).to_string(),
    );
    if !matches!(properties.picture, CommonCommandPicture::Empty) {
        replace_1c_value_if_different(
            &mut text,
            fields[1].clone(),
            &format_common_command_picture(&properties.picture),
        );
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
    text.replace_range(fields[6].clone(), &inner_text);
    replace_1c_value_if_different(&mut text, fields[5].clone(), r#"{0}"#);
    replace_1c_value_if_different(
        &mut text,
        fields[4].clone(),
        &format_1c_synonyms(&properties.tooltip),
    );
    text.replace_range(
        fields[3].clone(),
        &common_command_representation_code(properties.representation).to_string(),
    );
    text.replace_range(
        fields[2].clone(),
        &command_group_category_code(properties.category).to_string(),
    );
    replace_1c_value_if_different(
        &mut text,
        fields[1].clone(),
        &format_command_group_picture(&properties.picture),
    );

    Ok(text)
}

#[doc(hidden)]
pub fn replace_1c_value_if_different(text: &mut String, range: Range<usize>, replacement: &str) {
    if !one_c_values_equal_ignoring_ws(&text[range.clone()], replacement) {
        text.replace_range(range, replacement);
    }
}

fn metadata_type_pattern_matches_existing(
    existing: &str,
    formatted: &str,
    expected: &[MetadataTypePatternElement],
) -> bool {
    if one_c_values_equal_ignoring_ws(existing, formatted) {
        return true;
    }

    let Ok(fields) = scan_braced_fields(existing.trim(), 0) else {
        return false;
    };
    if fields.len() != expected.len() + 1 {
        return false;
    }
    if existing[fields[0].clone()].trim() != r#""Pattern""# {
        return false;
    }

    expected
        .iter()
        .zip(fields.iter().skip(1))
        .all(|(expected, range)| match expected {
            MetadataTypePatternElement::DateTime { .. } => {
                let compact = compact_1c_value(&existing[range.clone()]);
                compact == r#"{"D"}"# || compact.starts_with(r#"{"D","#)
            }
            _ => one_c_values_equal_ignoring_ws(
                &existing[range.clone()],
                &format_metadata_type_pattern_element(expected),
            ),
        })
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

#[allow(
    clippy::too_many_arguments,
    reason = "takes the record's parts one by one; bundling them changes the signature and every caller"
)]
fn parse_constant_value_type(
    types: &[String],
    string_length: Option<String>,
    string_allowed_length: Option<String>,
    number_digits: Option<String>,
    number_fraction_digits: Option<String>,
    number_allowed_sign: Option<String>,
    date_fractions: Option<String>,
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
        date_fractions,
        StringAllowedLengthCoding::VariableIsOne,
        source.map(|source| source as &dyn MetadataSource),
        false,
    )?;
    elements
        .pop()
        .ok_or_else(|| anyhow!("Constant Properties/Type is empty"))
}

fn parse_common_command_picture(
    reference: Option<String>,
    load_transparent: Option<String>,
    pixel: Option<(i64, i64)>,
    source: Option<&MetadataSourceContext>,
) -> Result<CommonCommandPicture> {
    let Some(reference) = reference.map(|value| value.trim().to_string()) else {
        return Ok(CommonCommandPicture::Empty);
    };
    if reference.is_empty() {
        return Ok(CommonCommandPicture::Empty);
    }
    if let Some(code) = common_command_standard_picture_code(&reference) {
        let load_transparent = parse_required_metadata_bool(
            "CommonCommand",
            "Picture/LoadTransparent",
            load_transparent,
        )?;
        return Ok(CommonCommandPicture::StdPictureCode {
            code,
            load_transparent,
            pixel,
        });
    }
    if let Some(uuid) = common_command_standard_picture_uuid(&reference) {
        let load_transparent = parse_required_metadata_bool(
            "CommonCommand",
            "Picture/LoadTransparent",
            load_transparent,
        )?;
        return Ok(CommonCommandPicture::CommonPicture {
            uuid: uuid.to_string(),
            load_transparent,
            pixel,
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
        pixel,
    })
}

fn common_command_standard_picture_code(reference: &str) -> Option<i32> {
    match reference.trim() {
        "StdPicture.InputFieldClear" => Some(-2),
        "StdPicture.MoveUp" => Some(-3),
        "StdPicture.MoveDown" => Some(-4),
        "StdPicture.InputFieldOpen" => Some(-7),
        "StdPicture.MoveRight" => Some(-9),
        "StdPicture.CheckAll" => Some(-10),
        "StdPicture.UncheckAll" => Some(-11),
        _ => None,
    }
}

fn common_command_standard_picture_uuid(reference: &str) -> Option<&'static str> {
    match reference.trim() {
        "StdPicture.Information" => Some(STD_PICTURE_INFORMATION_UUID),
        "StdPicture.SaveFile" => Some(STD_PICTURE_SAVE_FILE_UUID),
        "StdPicture.User" => Some(STD_PICTURE_USER_UUID),
        "StdPicture.LoadReportSettings" => Some(STD_PICTURE_LOAD_REPORT_SETTINGS_UUID),
        "StdPicture.Change" => Some("97b2cc97-d5c6-45fb-9824-9d6d73db21fe"),
        "StdPicture.Task" => Some("37cf7cc0-abad-4385-b597-6fd2d8dc085a"),
        "StdPicture.ChooseValue" => Some("2f130057-bb2a-4e22-bba5-e108fac26940"),
        "StdPicture.DataHistory" => Some("e8a49985-fef7-45a9-b6bb-ddd2b9028172"),
        "StdPicture.BusinessProcessObject" => Some("a24cff7f-a1a5-4403-af82-a7b31852cde9"),
        "StdPicture.CloneObject" => Some("f6532868-30b9-44ab-803c-78f0f0b06b02"),
        "StdPicture.CloneListItem" => Some("448d6f55-d885-496c-870d-d1bd78374745"),
        "StdPicture.EventLog" => Some("723765ab-0b92-4745-a621-1ba0f77c92c9"),
        "StdPicture.EventLogByUser" => Some("4fddea39-5129-4b4c-83fe-4e443cd61940"),
        "StdPicture.Find" => Some("ffab30f1-da11-44b5-b34c-24da22badcf4"),
        "StdPicture.CreateInitialImage" => Some("4d2570b5-205f-413c-b4cc-b2097f61684f"),
        "StdPicture.GenerateReport" => Some("0ce78048-0196-4f80-a781-9829cdb7f43e"),
        "StdPicture.MarkToDelete" => Some("18492a87-2fe4-44af-b218-304897fed020"),
        "StdPicture.Post" => Some("20ebc47b-f4d9-439c-acd3-fdc624fbac2a"),
        "StdPicture.Reread" => Some("8f29e0e2-d5e6-41e8-a34d-9a0288156322"),
        "StdPicture.Report" => Some("db817ee1-fd28-4e7f-bb4a-53686b2b153c"),
        "StdPicture.ScheduledJob" => Some("1970a480-9b38-405e-9d9e-8209f3fad5f1"),
        "StdPicture.SetDateInterval" => Some("58174855-39be-462e-8723-cb2d95182146"),
        "StdPicture.SetTime" => Some("55ef0776-5ee4-4daf-9a9b-70d63643ab8d"),
        "StdPicture.Refresh" => Some("fc4f29e0-d168-4fe0-8e64-e982fabf2595"),
        "StdPicture.SortListAsc" => Some("91022b99-b610-48ad-954e-a297848081ce"),
        "StdPicture.SortListDesc" => Some("1fa32fdb-a180-418f-a6eb-db7516b7a30b"),
        "StdPicture.UndoPosting" => Some("8ca4ea33-603d-4992-8a41-c7924b5bd40b"),
        "StdPicture.Write" => Some("894cf65b-4109-4533-a1d7-c87b1fcc80a3"),
        "StdPicture.WriteAndClose" => Some("e6fc55a0-3d58-4b15-bdd3-717453929598"),
        "StdPicture.Delete" => Some("08a45a70-c221-4339-b3b1-9f11cb22147d"),
        other => std_picture_value_uuid(other),
    }
}

/// The uuid a `StdPicture.<Name>` is stored under when the form writer's
/// measured table (`STD_PICTURE_VALUES`) spells it `{0,<uuid>}` -- the same
/// uuids the exporter names back.
fn std_picture_value_uuid(reference: &str) -> Option<&'static str> {
    let name = reference.trim().strip_prefix("StdPicture.")?;
    let value = STD_PICTURE_VALUES
        .iter()
        .find_map(|(candidate, value)| (*candidate == name).then_some(*value))?;
    value
        .strip_prefix("{0,")
        .and_then(|rest| rest.strip_suffix('}'))
        .filter(|uuid| is_uuid_like_ascii(uuid.as_bytes()))
}

fn parse_command_group_picture(
    reference: Option<String>,
    load_transparent: Option<String>,
    pixel: Option<(i64, i64)>,
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
            pixel,
        });
    }
    if let Some(uuid) = std_picture_value_uuid(&reference) {
        let load_transparent = parse_required_metadata_bool(
            "CommandGroup",
            "Picture/LoadTransparent",
            load_transparent,
        )?;
        return Ok(CommandGroupPicture::CommonPicture {
            uuid: uuid.to_string(),
            load_transparent,
            pixel,
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
        pixel,
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

fn format_constant_type_pattern(value_type: &MetadataTypePatternElement) -> String {
    format_metadata_type_pattern(std::slice::from_ref(value_type))
}

fn format_common_command_picture(picture: &CommonCommandPicture) -> String {
    match picture {
        CommonCommandPicture::Empty => r#"{4,0,{0},"",-1,-1,1,0,""}"#.to_string(),
        CommonCommandPicture::CommonPicture {
            uuid,
            load_transparent,
            pixel,
        } => {
            let (x, y) = pixel.unwrap_or((-1, -1));
            format!(
                r#"{{4,1,{{0,{uuid}}},"",{x},{y},{},0,""}}"#,
                bool_flag(*load_transparent)
            )
        }
        CommonCommandPicture::StdPictureCode {
            code,
            load_transparent,
            pixel,
        } => {
            let (x, y) = pixel.unwrap_or((-1, -1));
            format!(
                r#"{{4,1,{{{code}}},"",{x},{y},{},0,""}}"#,
                bool_flag(*load_transparent)
            )
        }
    }
}

fn format_command_group_picture(picture: &CommandGroupPicture) -> String {
    match picture {
        CommandGroupPicture::Empty => r#"{4,0,{0},"",-1,-1,1,0,""}"#.to_string(),
        CommandGroupPicture::StdPicturePrint => r#"{4,1,{-13},"",-1,-1,1,0,""}"#.to_string(),
        CommandGroupPicture::CommonPicture {
            uuid,
            load_transparent,
            pixel,
        } => {
            let (x, y) = pixel.unwrap_or((-1, -1));
            format!(
                r#"{{4,1,{{0,{uuid}}},"",{x},{y},{},0,""}}"#,
                bool_flag(*load_transparent)
            )
        }
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

#[allow(
    clippy::too_many_arguments,
    reason = "takes the record's parts one by one; bundling them changes the signature and every caller"
)]
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

#[allow(
    clippy::too_many_arguments,
    reason = "takes the record's parts one by one; bundling them changes the signature and every caller"
)]
fn append_constant_xml_text(
    path: &[String],
    value: &str,
    types: &mut Vec<String>,
    string_length: &mut Option<String>,
    string_allowed_length: &mut Option<String>,
    number_digits: &mut Option<String>,
    number_fraction_digits: &mut Option<String>,
    number_allowed_sign: &mut Option<String>,
    date_fractions: &mut Option<String>,
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
        date_fractions,
    );

    if path_ends_with(path, &["Constant", "Properties", "UseStandardCommands"]) {
        use_standard_commands
            .get_or_insert_with(String::new)
            .push_str(value);
    }
    // A constant typed by a defined type spells it as a `<v8:TypeSet>`, which
    // resolves to the defined type's own type id like a `<v8:Type>` does.
    if path_ends_with(path, &["Constant", "Properties", "Type", "TypeSet"]) {
        types.push(value.to_string());
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "takes the record's parts one by one; bundling them changes the signature and every caller"
)]
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
    date_fractions: &mut Option<String>,
) {
    if path_ends_with(
        path,
        &[
            kind,
            "Properties",
            "Type",
            "DateQualifiers",
            "DateFractions",
        ],
    ) {
        date_fractions
            .get_or_insert_with(String::new)
            .push_str(value);
    }
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

#[allow(
    clippy::too_many_arguments,
    reason = "takes the record's parts one by one; bundling them changes the signature and every caller"
)]
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

#[allow(
    clippy::too_many_arguments,
    reason = "takes the record's parts one by one; bundling them changes the signature and every caller"
)]
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

impl ConfigurationObjects for MetadataSourceContext {
    fn object(&self, key: &str) -> Option<Arc<ConfigurationObject>> {
        self.configuration_object(key)
    }
}

/// The folder one metadata class's own XML file lives in.
///
/// Only the top-level `<Family>/<Name>.xml` files matter here: forms,
/// templates and commands live in their own files and carry no field ids.
fn configuration_object_source_folder(class: &str) -> Option<&'static str> {
    Some(match class {
        "AccountingRegister" => "AccountingRegisters",
        "AccumulationRegister" => "AccumulationRegisters",
        "BusinessProcess" => "BusinessProcesses",
        "CalculationRegister" => "CalculationRegisters",
        "Catalog" => "Catalogs",
        "ChartOfAccounts" => "ChartsOfAccounts",
        "ChartOfCalculationTypes" => "ChartsOfCalculationTypes",
        "ChartOfCharacteristicTypes" => "ChartsOfCharacteristicTypes",
        "CommonAttribute" => "CommonAttributes",
        "Constant" => "Constants",
        "DataProcessor" => "DataProcessors",
        "DefinedType" => "DefinedTypes",
        "Document" => "Documents",
        "DocumentJournal" => "DocumentJournals",
        "Enum" => "Enums",
        "ExchangePlan" => "ExchangePlans",
        "ExternalDataSource" => "ExternalDataSources",
        "FilterCriterion" => "FilterCriteria",
        "InformationRegister" => "InformationRegisters",
        "Report" => "Reports",
        "Sequence" => "Sequences",
        "SettingsStorage" => "SettingsStorages",
        "Task" => "Tasks",
        _ => return None,
    })
}

/// The tags a metadata object declares its fields with.
#[doc(hidden)]
pub fn is_configuration_field_tag(local: &str) -> bool {
    matches!(
        local,
        "Attribute"
            | "Dimension"
            | "Resource"
            | "AccountingFlag"
            | "ExtDimensionAccountingFlag"
            | "AddressingAttribute"
            | "Column"
            | "EnumValue"
            | "Recalculation"
            | "TabularSection"
            | "Cube"
            | "DimensionTable"
    )
}

/// A field being read: its name and types arrive after its `uuid=`.
#[derive(Debug, Default)]
struct PendingConfigurationField {
    uuid: String,
    tag: String,
    name: Option<String>,
    types: Vec<String>,
}

impl PendingConfigurationField {
    fn finish(self) -> Option<(String, ConfigurationField)> {
        let name = self.name.filter(|name| !name.is_empty())?;
        Some((
            name,
            ConfigurationField {
                uuid: self.uuid,
                tag: self.tag,
                types: self.types,
            },
        ))
    }
}

/// The field index of one metadata object's own XML file.
///
/// `<MetaDataObject>/<Class uuid=>` is the object's uuid, and every
/// `<ChildObjects>` entry carries the `uuid=` a `<DataPath>` stores for it --
/// verbatim, never an ordinal. A tabular section's nested `<ChildObjects>` is
/// read the same way, into a table of its own, because a section's fields and
/// the object's own share a namespace only by accident.
fn parse_configuration_object_xml(xml: &[u8]) -> Result<Option<ConfigurationObject>> {
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut text_value = String::new();
    let mut object = ConfigurationObject::default();
    let mut current = None::<PendingConfigurationField>;
    let mut nested = None::<PendingConfigurationField>;
    let mut section = BTreeMap::<String, Vec<ConfigurationField>>::new();

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) => {
                let local = xml_local_name(event.local_name().as_ref());
                if path.len() == 1 && path[0] == "MetaDataObject" && object.class.is_empty() {
                    object.class = local.clone();
                    object.uuid = xml_attr_value(&event, "uuid").unwrap_or_default();
                } else if path.len() == 3
                    && path[2] == "ChildObjects"
                    && is_configuration_field_tag(&local)
                {
                    current = Some(PendingConfigurationField {
                        uuid: xml_attr_value(&event, "uuid").unwrap_or_default(),
                        tag: local.clone(),
                        ..PendingConfigurationField::default()
                    });
                    if local == "TabularSection" {
                        section = BTreeMap::new();
                    }
                } else if path.len() == 5
                    && path[2] == "ChildObjects"
                    && path[4] == "ChildObjects"
                    && is_configuration_field_tag(&local)
                {
                    nested = Some(PendingConfigurationField {
                        uuid: xml_attr_value(&event, "uuid").unwrap_or_default(),
                        tag: local.clone(),
                        ..PendingConfigurationField::default()
                    });
                }
                path.push(local);
                text_value.clear();
            }
            Ok(Event::Text(text)) => {
                let value = text.xml_content()?;
                text_value.push_str(unescape(value.as_ref())?.as_ref());
            }
            Ok(Event::CData(text)) => {
                text_value.push_str(text.xml_content()?.as_ref());
            }
            Ok(Event::End(_)) => {
                let value = text_value.trim().to_string();
                let depth = path.len();
                if depth == 5 && path[2] == "Properties" && path[3] == "Owners" {
                    if !value.is_empty() {
                        object.owners.push(value);
                    }
                } else if depth == 5
                    && path[2] == "Properties"
                    && path[3] == "Type"
                    && matches!(path[4].as_str(), "Type" | "TypeSet")
                {
                    if !value.is_empty() {
                        object.types.push(value);
                    }
                } else if depth == 6
                    && path[2] == "ChildObjects"
                    && path[4] == "Properties"
                    && path[5] == "Name"
                {
                    if let Some(field) = current.as_mut() {
                        field.name = Some(value);
                    }
                } else if depth == 7
                    && path[2] == "ChildObjects"
                    && path[4] == "Properties"
                    && path[5] == "Type"
                    && matches!(path[6].as_str(), "Type" | "TypeSet")
                {
                    if let Some(field) = current.as_mut()
                        && !value.is_empty()
                    {
                        field.types.push(value);
                    }
                } else if depth == 8
                    && path[4] == "ChildObjects"
                    && path[6] == "Properties"
                    && path[7] == "Name"
                {
                    if let Some(field) = nested.as_mut() {
                        field.name = Some(value);
                    }
                } else if depth == 9
                    && path[4] == "ChildObjects"
                    && path[6] == "Properties"
                    && path[7] == "Type"
                    && matches!(path[8].as_str(), "Type" | "TypeSet")
                {
                    if let Some(field) = nested.as_mut()
                        && !value.is_empty()
                    {
                        field.types.push(value);
                    }
                } else if depth == 4
                    && path[2] == "ChildObjects"
                    && is_configuration_field_tag(&path[3])
                {
                    let tabular = path[3] == "TabularSection";
                    if let Some((name, field)) = current.take().and_then(|field| field.finish()) {
                        if tabular {
                            object
                                .sections
                                .insert(name.clone(), std::mem::take(&mut section));
                        }
                        object.fields.entry(name).or_default().push(field);
                    } else if tabular {
                        section = BTreeMap::new();
                    }
                } else if depth == 6
                    && path[2] == "ChildObjects"
                    && path[4] == "ChildObjects"
                    && is_configuration_field_tag(&path[5])
                    && let Some((name, field)) = nested.take().and_then(|field| field.finish())
                {
                    section.entry(name).or_default().push(field);
                }
                path.pop();
                text_value.clear();
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(error.into()),
        }
        buffer.clear();
    }

    if object.class.is_empty() || object.uuid.is_empty() {
        return Ok(None);
    }
    Ok(Some(object))
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
        | "AccountingRegisterRecord"
        | "AccountingRegisterExtDimensions"
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

#[doc(hidden)]
pub fn metadata_reference_source_folder(reference: &str) -> Option<(&'static str, &'static str)> {
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
        "CommonModule" => Some(("CommonModule", "CommonModules")),
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
        ibcmd_schema::websocket_client::WebSocketClientLayout::KIND => {
            Some(ibcmd_schema::websocket_client::WebSocketClientLayout::identity())
        }
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
        "Sequence" => Some(("Sequence", "Sequences")),
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

#[doc(hidden)]
pub fn normalize_uuid_text(value: &str) -> Result<String> {
    Ok(Uuid::parse_str(value.trim())?.hyphenated().to_string())
}

#[doc(hidden)]
pub fn xml_attr_value(event: &BytesStart<'_>, name: &str) -> Option<String> {
    event
        .attributes()
        .filter_map(Result::ok)
        .find(|attr| attr.key.as_ref() == name.as_bytes())
        .map(|attr| String::from_utf8_lossy(attr.value.as_ref()).to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        CommonCommandRepresentation, common_command_representation_code, deflate_raw, inflate_raw,
        parse_common_command_representation,
    };

    fn decode_base64_for_test(input: &str) -> anyhow::Result<Vec<u8>> {
        fn value(byte: u8) -> anyhow::Result<u8> {
            match byte {
                b'A'..=b'Z' => Ok(byte - b'A'),
                b'a'..=b'z' => Ok(byte - b'a' + 26),
                b'0'..=b'9' => Ok(byte - b'0' + 52),
                b'+' => Ok(62),
                b'/' => Ok(63),
                _ => anyhow::bail!("invalid base64 byte {byte}"),
            }
        }

        let bytes: Vec<u8> = input
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
        for chunk in bytes.chunks(4) {
            if chunk.len() != 4 {
                anyhow::bail!("invalid base64 length");
            }
            let first = value(chunk[0])?;
            let second = value(chunk[1])?;
            let third = if chunk[2] == b'=' {
                0
            } else {
                value(chunk[2])?
            };
            let fourth = if chunk[3] == b'=' {
                0
            } else {
                value(chunk[3])?
            };
            output.push((first << 2) | (second >> 4));
            if chunk[2] != b'=' {
                output.push((second << 4) | (third >> 2));
            }
            if chunk[3] != b'=' {
                output.push((third << 6) | fourth);
            }
        }
        Ok(output)
    }

    fn form_setting_base64_xml_for_test(text: &str, key: &str) -> anyhow::Result<String> {
        let key_start = text
            .find(&format!("\"{key}\""))
            .ok_or_else(|| anyhow::anyhow!("setting {key} not found"))?;
        let payload_prefix = "{#base64:";
        let payload_start = text[key_start..]
            .find(payload_prefix)
            .map(|relative| key_start + relative + payload_prefix.len())
            .ok_or_else(|| anyhow::anyhow!("setting {key} base64 payload not found"))?;
        let payload_end = text[payload_start..]
            .find('}')
            .map(|relative| payload_start + relative)
            .ok_or_else(|| anyhow::anyhow!("setting {key} base64 payload is not closed"))?;
        let bytes = decode_base64_for_test(&text[payload_start..payload_end])?;
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        String::from_utf8(bytes.to_vec()).map_err(Into::into)
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
    fn audits_metadata_xml_as_base_dependent_with_precise_blockers() -> anyhow::Result<()> {
        let xml = br#"
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21">
  <DataProcessor uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties><Name>Loader</Name></Properties>
    <ChildObjects>
      <Command uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
        <Properties><Name>Load</Name></Properties>
      </Command>
      <Form uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc">
        <Properties><Name>Main</Name></Properties>
      </Form>
    </ChildObjects>
  </DataProcessor>
</MetaDataObject>
"#;

        let blockers = super::metadata_xml_base_free_blockers(xml)?;

        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("DataProcessor metadata XML Loader"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("pack_simple_metadata_blob_from_xml_with_source"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("1 direct Properties child element(s)"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("Command=1") && blocker.contains("Form=1"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("2 ChildObjects entries (Command=1, Form=1)"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("full DataProcessor metadata compiler"))
        );
        Ok(())
    }

    #[test]
    fn audits_common_module_metadata_as_base_dependent() -> anyhow::Result<()> {
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <CommonModule uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>Utils</Name>
      <Global>false</Global>
      <ClientManagedApplication>false</ClientManagedApplication>
      <Server>true</Server>
      <ExternalConnection>false</ExternalConnection>
      <ClientOrdinaryApplication>false</ClientOrdinaryApplication>
      <ServerCall>false</ServerCall>
      <Privileged>false</Privileged>
      <ReturnValuesReuse>DontUse</ReturnValuesReuse>
    </Properties>
  </CommonModule>
</MetaDataObject>
"#;

        let blockers = super::common_module_metadata_base_free_blockers(xml)?;

        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("CommonModule metadata XML Utils"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("pack_common_module_metadata_blob_from_xml"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("full CommonModule metadata compiler"))
        );
        Ok(())
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
    fn patches_configuration_header_with_root_uuid_xml() {
        let root_uuid = "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa";
        let header_uuid = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
        let base_text = format!(
            "{{2,{{{root_uuid}}},1,{{9cd510cd-abfc-11d4-9434-004095e12fc7,{{1,{{68,{{0,{{3,{{1,0,{header_uuid}}},\"OldApp\",{{1,\"en\",\"Old app\"}},\"Old comment\",0,0,00000000-0000-0000-0000-000000000000,0}}}}}}}}}}}}"
        );
        let base_blob = deflate_raw(base_text.as_bytes()).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <Configuration uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>NewApp</Name>
      <Synonym>
        <v8:item>
          <v8:lang>en</v8:lang>
          <v8:content>New app</v8:content>
        </v8:item>
      </Synonym>
      <Comment>New comment</Comment>
    </Properties>
  </Configuration>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert!(inflated.contains(&format!("{{1,0,{header_uuid}}}")));
        assert!(inflated.contains("\"NewApp\""));
        assert!(inflated.contains("{1,\"en\",\"New app\"}"));
        assert!(inflated.contains("\"New comment\""));
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
    fn patches_constant_preserving_equivalent_type_pattern_format() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            r#"{1,
{16,
{27,
{2,
{3,
{1,0,cccccccc-cccc-4ccc-cccc-cccccccccccc},"OldName",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},
{"Pattern",
{"S",4,1}
}
},0,{0},{0},0,"",0,{"U"},{"U"},0,00000000-0000-0000-0000-000000000000,2,0,{5006,0},{3,0,0},{0,0},0,{0},{"S",""},0,0,0},
aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,dddddddd-dddd-dddddddddddd,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee,1,0,{0},{0},00000000-0000-0000-0000-000000000000,0,0,ffffffff-ffff-4fff-ffff-ffffffffffff,99999999-9999-4999-9999-999999999999,0,0},0}"#
                .as_bytes(),
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <Constant uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc">
    <Properties>
      <Name>NewConstant</Name>
      <Synonym/>
      <Comment/>
      <Type>
        <v8:Type>xs:string</v8:Type>
        <v8:StringQualifiers>
          <v8:Length>4</v8:Length>
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

        assert!(inflated.contains("{\"Pattern\",\n{\"S\",4,1}\n}"));
    }

    #[test]
    fn patches_constant_preserving_datetime_type_qualifier() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            r#"{1,
{16,
{27,
{2,
{3,
{1,0,cccccccc-cccc-4ccc-cccc-cccccccccccc},"OldConstant",
{1,"ru","Old constant"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},
{"Pattern",
{"D","T"}
}
},0,
{1,"ru","ДФ=HH"},
{0},0,"",0,
{"U"},
{"U"},0,00000000-0000-0000-0000-000000000000,2,0,
{5006,0},
{3,0,0},
{0,0},0,
{1,"ru","ДФ=HH"},
{"S",""},0,0,0},
aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,dddddddd-dddd-4ddd-dddd-dddddddddddd,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee,1,0,{0},{0},00000000-0000-0000-0000-000000000000,0,0,ffffffff-ffff-4fff-ffff-ffffffffffff,99999999-9999-4999-9999-999999999999,0,0},0}"#
                .as_bytes(),
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <Constant uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc">
    <Properties>
      <Name>OldConstant</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Old constant</v8:content>
        </v8:item>
      </Synonym>
      <Comment>Old comment</Comment>
      <Type>
        <v8:Type>xs:dateTime</v8:Type>
      </Type>
      <UseStandardCommands>true</UseStandardCommands>
    </Properties>
  </Constant>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert!(inflated.contains("{\"Pattern\",\n{\"D\",\"T\"}\n}"));
    }

    #[test]
    fn patches_constant_valuestorage_type_pattern() {
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
aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,dddddddd-dddd-4ddd-dddd-dddddddddddd,eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee,1,0,{0},{0},00000000-0000-0000-0000-000000000000,0,0,ffffffff-ffff-4fff-ffff-ffffffffffff,99999999-9999-4999-9999-999999999999,0,0},0}"#,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <Constant uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc">
    <Properties>
      <Name>ValueStorageConstant</Name>
      <Synonym/>
      <Comment/>
      <Type>
        <v8:Type>v8:ValueStorage</v8:Type>
      </Type>
      <UseStandardCommands>true</UseStandardCommands>
    </Properties>
  </Constant>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert!(inflated.contains(r##"{"Pattern",{"#",e199ca70-93cf-46ce-a54b-6edc88c3a296}}"##));
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
    fn patches_common_command_preserving_equivalent_picture_format() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,dddddddd-dddd-4ddd-dddd-dddddddddddd,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,
{0},"",-1,-1,1,0,""},3,
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
      <Name>OldCommand</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Old synonym</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Group>NavigationPanelOrdinary</Group>
      <Representation>Auto</Representation>
      <ToolTip/>
      <Picture/>
      <Shortcut/>
      <IncludeHelpInContents>true</IncludeHelpInContents>
      <CommandParameterType/>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert!(inflated.contains("{4,0,\n{0},\"\",-1,-1,1,0,\"\"},3,"));
    }

    #[test]
    fn patches_common_command_preserving_existing_picture_when_xml_empty() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,dddddddd-dddd-4ddd-dddd-dddddddddddd,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,1,
{0,1f046bc2-d6c5-46a3-a459-b2c0508f86fb},"",-1,-1,1,0,""},2,
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
      <Name>OldCommand</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Old synonym</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Group>NavigationPanelOrdinary</Group>
      <Representation>PictureAndText</Representation>
      <ToolTip/>
      <Picture/>
      <Shortcut/>
      <IncludeHelpInContents>true</IncludeHelpInContents>
      <CommandParameterType/>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert!(inflated.contains("{0,1f046bc2-d6c5-46a3-a459-b2c0508f86fb}"));
    }

    #[test]
    fn patches_common_command_preserving_existing_parameter_type_when_xml_empty() {
        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br##"{1,
{2,
{1,
{2,dddddddd-dddd-4ddd-dddd-dddddddddddd,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,{0},"",-1,-1,1,0,""},3,
{0},1,
{0,0,0},0,
{1,77ea1b8f-dd79-4717-9dba-5628e7f348cf},
{"Pattern",
{"#",225a597b-50b5-499c-b93b-9cd248b3f18e},
{"#",2c2dfd96-3d8c-4420-b46c-e87260ddb55a}},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldCommand",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},0,0,0}
}
},0}"##,
        );
        let base_blob = deflate_raw(&active).unwrap();
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <CommonCommand uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>OldCommand</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Old synonym</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Group>NavigationPanelOrdinary</Group>
      <Representation>Auto</Representation>
      <ToolTip/>
      <Picture/>
      <Shortcut/>
      <IncludeHelpInContents>true</IncludeHelpInContents>
      <CommandParameterType/>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#;

        let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, xml).unwrap();
        let inflated = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();

        assert!(inflated.contains(r##"{"#",225a597b-50b5-499c-b93b-9cd248b3f18e}"##));
        assert!(inflated.contains(r##"{"#",2c2dfd96-3d8c-4420-b46c-e87260ddb55a}"##));
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
    fn packs_style_body_xml_with_85_standard_items() -> anyhow::Result<()> {
        let mut xml = String::from("<Style>");
        xml.push_str(r##"<Item name="ImportantColor"><Color>#123456</Color></Item>"##);
        for name in [
            "TitleLevel3",
            "TitleLevel5",
            "TextLevel1",
            "TextLevel2",
            "TextLevel3",
            "SubtitleLevel1",
            "TitleLevel2",
        ] {
            xml.push_str(&format!(
                r#"<Item name="{name}"><Font ref="style:{name}" kind="StyleItem"/></Item>"#
            ));
        }
        xml.push_str("</Style>");
        let packed = super::pack_style_body_blob_from_xml(xml.as_bytes(), None)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?.replace("\r\n", "");
        assert!(text.contains("{{-47},0,{3,0,{5649426}}}"), "{text}");
        for code in [-50, -52, -53, -54, -55, -56, -59] {
            assert!(
                text.contains(&format!("{{{{{code}}},1,{{7,2,0,{{{code}}},1,100}}}}")),
                "{text}"
            );
        }
        Ok(())
    }

    #[test]
    fn packs_style_body_xml_preserving_base_order_and_format() -> anyhow::Result<()> {
        let mut base_text = b"\xEF\xBB\xBF".to_vec();
        base_text.extend_from_slice(
            b"{2,3,\n{\n{-44},0,\n{4,3,\n{-44},3}\n},\n{\n{-42},0,\n{4,0,\n{14474460},0}\n},\n{\n{-43},0,\n{4,0,\n{15658734},0}\n},\n{0}\n}",
        );
        let base = super::deflate_raw(&base_text)?;
        let xml = br##"<?xml version="1.0" encoding="UTF-8"?>
<Style xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" version="2.21">
	<Item name="NavigationColor">
		<Color>#DCDCDC</Color>
	</Item>
	<Item name="AuxiliaryNavigationColor">
		<Color>#EEEEEE</Color>
	</Item>
	<Item name="ActivityColor">
		<Color>style:ActivityColor</Color>
	</Item>
</Style>
"##;

        let packed = super::pack_style_body_blob_from_xml_with_base(&base, xml, None)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(text.as_bytes(), base_text.as_slice());

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
            "\u{feff}{00010101000000,00010101000000,00010101080000,00010101170000,00010101000000,0,60,0,2,6,7,0,1,12,1,2,3,4,5,6,7,8,9,10,11,12,1,0,0}"
        );
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_form_body_module_text_preserving_base_layout() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{7,{\"layout\"}},\"Old module\",{3,{\"picture\"},\"payload\"}}",
        )?;
        let parsed = super::parse_form_body_blob(&base)?;
        assert_eq!(parsed.layout, "{7,{\"layout\"}}");
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing_fields, 1);

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
    fn packs_form_body_xml_top_level_properties() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},1,0,1,1,1,0,1,1,1},\"Old module\",{0}}",
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Title>
		<v8:item>
			<v8:lang>ru</v8:lang>
			<v8:content>Новый заголовок</v8:content>
		</v8:item>
		<v8:item>
			<v8:lang>en</v8:lang>
			<v8:content>New title</v8:content>
		</v8:item>
	</Title>
	<Width>80</Width>
	<Height>30</Height>
	<WindowOpeningMode>LockWholeInterface</WindowOpeningMode>
	<EnterKeyBehavior>DefaultButton</EnterKeyBehavior>
	<AutoTitle>false</AutoTitle>
	<SaveDataInSettings>UseList</SaveDataInSettings>
	<AutoSaveDataInSettings>Use</AutoSaveDataInSettings>
	<Group>Horizontal</Group>
	<CommandBarLocation>Bottom</CommandBarLocation>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[2].clone()], "2");
        assert_eq!(&parsed.layout[fields[3].clone()], "80");
        assert_eq!(&parsed.layout[fields[4].clone()], "30");
        assert_eq!(&parsed.layout[fields[5].clone()], "0");
        assert_eq!(&parsed.layout[fields[6].clone()], "1");
        assert_eq!(&parsed.layout[fields[7].clone()], "1");
        assert_eq!(&parsed.layout[fields[9].clone()], "0");
        assert_eq!(
            &parsed.layout[fields[10].clone()],
            r#"{1,2,{"ru","Новый заголовок"},{"en","New title"}}"#
        );
        assert_eq!(&parsed.layout[fields[11].clone()], "1");
        assert_eq!(&parsed.layout[fields[13].clone()], "0");
        assert_eq!(&parsed.layout[fields[14].clone()], "0");
        assert_eq!(&parsed.layout[fields[17].clone()], "3");
        assert_eq!(&parsed.layout[fields[18].clone()], "1");
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(
            packed.plain_bytes,
            String::from_utf8(super::inflate_raw(&packed.blob)?)?.len()
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_preserves_equivalent_root_title_format() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,1,
{"ru","Title"}
},1,0,1,1,1,0,1,1,1},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Title>
		<v8:item>
			<v8:lang>ru</v8:lang>
			<v8:content>Title</v8:content>
		</v8:item>
	</Title>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains("{1,1,\n{\"ru\",\"Title\"}\n}"));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_explicit_default_window_opening_mode() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,0,2,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,1,1},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<WindowOpeningMode>DontBlock</WindowOpeningMode>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[2].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_lock_owner_window_alias() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,0,2,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,1,1},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<WindowOpeningMode>LockOwnerWindow</WindowOpeningMode>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[2].clone()], "1");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_auto_fill_check_false() -> anyhow::Result<()> {
        let base_text = r##"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,2,24,{"B",1},0,{"#",59ef2b80-c86b-11d5-a3c1-0050bae0a776,0}},"Old module",{0}}"##;
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<AutoFillCheck>false</AutoFillCheck>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#"24,{"B",0}"#));
        assert_eq!(parsed.module_text, "Old module");

        let base_false_text = base_text.replace(r#"24,{"B",1}"#, r#"24,{"B",0}"#);
        let base = super::deflate_raw(base_false_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<AutoFillCheck>true</AutoFillCheck>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#"24,{"B",1}"#));

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_save_window_settings_false() -> anyhow::Result<()> {
        let base_text = r#"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"FormCommandBar",{1,0}},1,cd5394d0-7dda-4b56-8927-93ccbe967a01,{22,{1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,"MainGroup",{1,0}},"","",0,1,"",0,0,0,0,0,0,3,3,0,0,0,100,1,1,0,0,0,{59,0},1,{1,0},{4,0,{0},"",-1,-1,1,0,""},0,0,1,0,2,0,0,0,2,0},"Old module",{0}}"#;
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<SaveWindowSettings>false</SaveWindowSettings>
</Form>
"#;

        assert_eq!(
            super::parse_form_xml_body_properties(xml)?.save_window_settings,
            Some(false)
        );
        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 23].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        let base_true_text = base_text.replace("{59,0},1,{1,0}", "{59,0},0,{1,0}");
        let base = super::deflate_raw(base_true_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<SaveWindowSettings>true</SaveWindowSettings>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 23].clone()], "1");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_show_close_button_false() -> anyhow::Result<()> {
        let base_text = r#"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"FormCommandBar",{1,0}},1,cd5394d0-7dda-4b56-8927-93ccbe967a01,{22,{1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,"MainGroup",{1,0}},"","",0,1,"",0,0,0,0,0,0,3,3,0,0,0,100,1,1,0,0,0,{59,0},1,{1,0},{4,0,{0},"",-1,-1,1,0,""},0,0,1,0,2,0,0,0,2,0},"Old module",{0}}"#;
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ShowCloseButton>false</ShowCloseButton>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 18].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        let base_true_text = base_text.replace("100,1,1,0,0,0,{59,0}", "100,1,0,0,0,0,{59,0}");
        let base = super::deflate_raw(base_true_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ShowCloseButton>true</ShowCloseButton>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 18].clone()], "1");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_show_title_false() -> anyhow::Result<()> {
        let base_text = r#"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"FormCommandBar",{1,0}},1,cd5394d0-7dda-4b56-8927-93ccbe967a01,{22,{1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,"MainGroup",{1,0}},"","",0,1,"",0,0,0,0,0,0,3,3,0,0,0,100,1,1,0,0,0,{59,0},1,{1,0},{4,0,{0},"",-1,-1,1,0,""},0,0,1,0,2,0,0,0,2,0},"Old module",{0}}"#;
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ShowTitle>false</ShowTitle>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 17].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        let base_false_text = base_text.replace("100,1,1,0,0,0,{59,0}", "100,0,1,0,0,0,{59,0}");
        let base = super::deflate_raw(base_false_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ShowTitle>true</ShowTitle>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 17].clone()], "1");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_root_horizontal_align() -> anyhow::Result<()> {
        let base_text = r#"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"FormCommandBar",{1,0}},1,cd5394d0-7dda-4b56-8927-93ccbe967a01,{22,{1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,"MainGroup",{1,0}},"","",0,1,"",0,0,0,0,0,0,3,3,0,0,0,100,1,1,0,0,0,{59,0},1,{1,0},{4,0,{0},"",-1,-1,1,0,""},0,0,1,0,2,0,0,0,2,0},"Old module",{0}}"#;
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<HorizontalAlign>Center</HorizontalAlign>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 11].clone()], "1");
        assert_eq!(parsed.module_text, "Old module");

        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<HorizontalAlign>Right</HorizontalAlign>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 11].clone()], "2");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_root_scaling_mode() -> anyhow::Result<()> {
        let base_text = r#"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"FormCommandBar",{1,0}},1,cd5394d0-7dda-4b56-8927-93ccbe967a01,{22,{1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,5,"MainGroup",{1,0}},"","",0,1,"",0,0,0,0,0,0,3,3,0,0,0,100,1,1,0,0,0,{59,0},1,{1,0},{4,0,{0},"",-1,-1,1,0,""},0,0,1,0,2,0,0,0,2,0},"Old module",{0}}"#;
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ScalingMode>Compact</ScalingMode>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 6].clone()], "2");
        assert_eq!(parsed.module_text, "Old module");

        let base_compact_text = base_text.replace(r#""",0,0,0,0,0,0,3,3"#, r#""",0,2,0,0,0,0,3,3"#);
        let base = super::deflate_raw(base_compact_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ScalingMode>Normal</ScalingMode>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_items_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 6].clone()], "1");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_root_scaling_mode_with_pages_item() -> anyhow::Result<()> {
        let base_text = r#"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"FormCommandBar",{1,0}},1,cd5394d0-7dda-4b56-8927-93ccbe967a01,{22,{31,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,3,"PagesGroup",{1,0}},"","",0,1,"",0,0,0,0,0,0,3,3,0,0,0,100,1,1,0,0,0,{59,0},1,{1,0},{4,0,{0},"",-1,-1,1,0,""},0,0,1,0,2,0,0,0,2,0},"Old module",{0}}"#;
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ScalingMode>Compact</ScalingMode>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let tail_start =
            super::form_layout_child_item_pairs_tail_start(&parsed.layout, &fields).unwrap();

        assert_eq!(&parsed.layout[fields[tail_start + 6].clone()], "2");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_customizable_false_for_vertical_layout() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Group>Vertical</Group>
	<Customizable>false</Customizable>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[11].clone()], "0");
        assert_eq!(&parsed.layout[fields[13].clone()], "1");
        assert_eq!(&parsed.layout[fields[14].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_auto_url_false_for_vertical_layout() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Group>Vertical</Group>
	<AutoURL>false</AutoURL>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[11].clone()], "0");
        assert_eq!(&parsed.layout[fields[13].clone()], "0");
        assert_eq!(&parsed.layout[fields[14].clone()], "1");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_auto_url_and_customizable_false_for_vertical_layout()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Group>Vertical</Group>
	<AutoURL>false</AutoURL>
	<Customizable>false</Customizable>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[11].clone()], "0");
        assert_eq!(&parsed.layout[fields[13].clone()], "0");
        assert_eq!(&parsed.layout[fields[14].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn skips_form_auto_url_for_property_bag_layout() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,0,1,1,0,1,4,0,{\"#\",59ef2b80-c86b-11d5-a3c1-0050bae0a776,0},24,{\"B\",0},25,{\"U\"},26,{\"B\",1},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<AutoURL>true</AutoURL>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[13].clone()], "0");
        assert_eq!(&parsed.layout[fields[18].clone()], "4");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_document_posting_options_auto_property_bag() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,6,2,{\"#\",adeb08a0-415c-11d6-b9d1-0050bae0a95d,3},3,{\"#\",20d89b09-bd04-4304-a8c7-4d07fac6338a,0},4,{\"B\",0},24,{\"B\",0},25,{\"U\"},26,{\"B\",1},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<AutoTime>DontUse</AutoTime>
	<UsePostingMode>Auto</UsePostingMode>
	<RepostOnWrite>true</RepostOnWrite>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r##"2,{"#",adeb08a0-415c-11d6-b9d1-0050bae0a95d,0}"##),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r##"3,{"#",20d89b09-bd04-4304-a8c7-4d07fac6338a,3}"##),
            "{}",
            parsed.layout
        );
        assert!(parsed.layout.contains(r#"4,{"B",1}"#), "{}", parsed.layout);
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_document_posting_options_regular_property_bag() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,6,2,{\"#\",adeb08a0-415c-11d6-b9d1-0050bae0a95d,0},3,{\"#\",20d89b09-bd04-4304-a8c7-4d07fac6338a,3},4,{\"B\",1},24,{\"B\",0},25,{\"U\"},26,{\"B\",1},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<AutoTime>CurrentOrLast</AutoTime>
	<UsePostingMode>Regular</UsePostingMode>
	<RepostOnWrite>false</RepostOnWrite>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r##"2,{"#",adeb08a0-415c-11d6-b9d1-0050bae0a95d,3}"##),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r##"3,{"#",20d89b09-bd04-4304-a8c7-4d07fac6338a,0}"##),
            "{}",
            parsed.layout
        );
        assert!(parsed.layout.contains(r#"4,{"B",0}"#), "{}", parsed.layout);
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_report_form_options_auto_property_bag() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,0,0,0,1,1,0,00000000-0000-0000-0000-000000000000,0,{1,0},0,0,1,1,1,0,0,5,7,{\"#\",acbc2eeb-2efb-48e4-b78a-661fd09fcf80,1},21,{\"#\",f26c3706-a6ca-45cb-869a-e6ad38cd5f78,1},23,{\"N\",1},27,{\"#\",b9311bea-b26b-4ae0-8b5d-7b64048fd2df,1},29,{\"#\",874260df-7e23-4f02-9e10-5794914b5adf,0},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ReportFormType>Main</ReportFormType>
	<AutoShowState>Auto</AutoShowState>
	<ReportResultViewMode>Auto</ReportResultViewMode>
	<ViewModeApplicationOnSetReportResult>Auto</ViewModeApplicationOnSetReportResult>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r##"7,{"#",acbc2eeb-2efb-48e4-b78a-661fd09fcf80,0}"##),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r##"21,{"#",f26c3706-a6ca-45cb-869a-e6ad38cd5f78,0}"##),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r##"27,{"#",b9311bea-b26b-4ae0-8b5d-7b64048fd2df,0}"##),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r##"29,{"#",874260df-7e23-4f02-9e10-5794914b5adf,0}"##),
            "{}",
            parsed.layout
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_report_form_options_variant_property_bag() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,0,0,0,1,1,0,00000000-0000-0000-0000-000000000000,0,{1,0},0,0,1,1,1,0,0,5,7,{\"#\",acbc2eeb-2efb-48e4-b78a-661fd09fcf80,0},21,{\"#\",f26c3706-a6ca-45cb-869a-e6ad38cd5f78,0},23,{\"N\",0},27,{\"#\",b9311bea-b26b-4ae0-8b5d-7b64048fd2df,0},29,{\"#\",874260df-7e23-4f02-9e10-5794914b5adf,0},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ReportFormType>Variant</ReportFormType>
	<AutoShowState>ShowOnComposition</AutoShowState>
	<ReportResultViewMode>Default</ReportResultViewMode>
	<ViewModeApplicationOnSetReportResult>Auto</ViewModeApplicationOnSetReportResult>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r##"7,{"#",acbc2eeb-2efb-48e4-b78a-661fd09fcf80,2}"##),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r##"21,{"#",f26c3706-a6ca-45cb-869a-e6ad38cd5f78,3}"##),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r##"27,{"#",b9311bea-b26b-4ae0-8b5d-7b64048fd2df,1}"##),
            "{}",
            parsed.layout
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_report_result_and_details_data_refs() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            r##"{4,{59,0,0,0,0,1,1,0,00000000-0000-0000-0000-000000000000,0,{1,0},0,0,1,1,1,0,0,2,5,{"#",11cfd3e0-86f8-4480-aaa5-dc6a6ccac689,{0,""}},6,{"#",11cfd3e0-86f8-4480-aaa5-dc6a6ccac689,{0,""}},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,"FormCommandBar",{1,0}}},"Old module",{4,2,{9,{3,02023637-7868-4a5f-8576-835a76e0c9ba},0,"Результат",0,0,0,0,0,0,0},{9,{4,02023637-7868-4a5f-8576-835a76e0c9ba},0,"ДанныеРасшифровки",0,0,0,0,0,0,0}}}"##
                .as_bytes(),
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ReportResult>Результат</ReportResult>
	<DetailsData>ДанныеРасшифровки</DetailsData>
	<Attributes>
		<Attribute name="Результат" id="3"></Attribute>
		<Attribute name="ДанныеРасшифровки" id="4"></Attribute>
	</Attributes>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r##"5,{"#",11cfd3e0-86f8-4480-aaa5-dc6a6ccac689,{1,{3},""}}"##),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r##"6,{"#",11cfd3e0-86f8-4480-aaa5-dc6a6ccac689,{1,{4},""}}"##),
            "{}",
            parsed.layout
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_command_set_single_excluded_command() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,0,{1,0},0,0,1,1,1,0,0,{0},{1,342c531d-dc73-458a-8ac4-6a746916a33b},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<CommandSet>
		<ExcludedCommand>CustomizeForm</ExcludedCommand>
	</CommandSet>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains("{1,198ea630-fda2-4cda-8a23-f999f4c67ee6}"),
            "{}",
            parsed.layout
        );
        assert!(
            !parsed
                .layout
                .contains("342c531d-dc73-458a-8ac4-6a746916a33b")
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_command_set_multiple_excluded_commands() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,1,1,{\"N\",0},{0,1,0},{1,198ea630-fda2-4cda-8a23-f999f4c67ee6},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<CommandSet>
		<ExcludedCommand>Change</ExcludedCommand>
		<ExcludedCommand>CustomizeForm</ExcludedCommand>
		<ExcludedCommand>Create</ExcludedCommand>
	</CommandSet>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(
            &parsed.layout[fields[22].clone()],
            "{3,198ea630-fda2-4cda-8a23-f999f4c67ee6,4f834c38-add1-45e4-a9f3-cefe3efac5c9,6886601d-276c-4d3f-af0a-05c586025608}"
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    /// `Copy` is spelled for two different root uuids, so it cannot be packed
    /// from the name alone.
    ///
    /// This fixture used to pack it as `342c531d-…`, which is what an ordinary
    /// list form stores. A business-process form stores `68baa1bc-…` for the
    /// same `<ExcludedCommand>Copy</ExcludedCommand>`: both
    /// `BusinessProcesses/Задание/Forms/ДействиеВыполнить` and
    /// `BusinessProcesses/Задание/Forms/ДействиеПроверить` of ERP УХ 3.3.3.3
    /// carry `68baa1bc-…` in their stored root command set.
    ///
    /// The class of the form's main attribute now decides, measured over the
    /// 3 313 forms whose excluded commands pair one for one with the uuids
    /// their body stores. This form names no main attribute, so nothing
    /// decides, and it is still refused rather than packed with one of the
    /// two.
    #[test]
    fn refuses_an_excluded_command_whose_name_two_uuids_share() {
        let base = super::deflate_raw(
            b"{4,{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,1,1,{\"N\",0},{0,1,0},{1,198ea630-fda2-4cda-8a23-f999f4c67ee6},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )
        .expect("fixture deflates");
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<CommandSet>
		<ExcludedCommand>Copy</ExcludedCommand>
	</CommandSet>
</Form>
"#;

        let error = super::pack_form_body_blob_from_form_xml(&base, xml, None)
            .expect_err("an ambiguous command name is refused");

        assert!(
            format!("{error:#}").contains("no measured uuid for <ExcludedCommand>Copy"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn packs_form_body_xml_use_for_folders_and_items_items_property_bag() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,4,0,{\"#\",59ef2b80-c86b-11d5-a3c1-0050bae0a776,1},24,{\"B\",0},25,{\"U\"},26,{\"B\",1},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<UseForFoldersAndItems>Items</UseForFoldersAndItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(
            &parsed.layout[fields[20].clone()],
            r##"{"#",59ef2b80-c86b-11d5-a3c1-0050bae0a776,0}"##
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_use_for_folders_and_items_folders_property_bag() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,4,0,{\"#\",59ef2b80-c86b-11d5-a3c1-0050bae0a776,0},24,{\"B\",0},25,{\"U\"},26,{\"B\",1},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<UseForFoldersAndItems>Folders</UseForFoldersAndItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(
            &parsed.layout[fields[20].clone()],
            r##"{"#",59ef2b80-c86b-11d5-a3c1-0050bae0a776,1}"##
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_conversations_representation_show_property_bag() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,2,21,{\"#\",f26c3706-a6ca-45cb-869a-e6ad38cd5f78,0},24,{\"B\",0},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ConversationsRepresentation>Show</ConversationsRepresentation>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(
            &parsed.layout[fields[20].clone()],
            r##"{"#",f26c3706-a6ca-45cb-869a-e6ad38cd5f78,1}"##
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_conversations_representation_dont_show_property_bag()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,1,2,21,{\"#\",f26c3706-a6ca-45cb-869a-e6ad38cd5f78,1},24,{\"B\",0},{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ConversationsRepresentation>DontShow</ConversationsRepresentation>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(
            &parsed.layout[fields[20].clone()],
            r##"{"#",f26c3706-a6ca-45cb-869a-e6ad38cd5f78,0}"##
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_vertical_scroll_use_if_necessary() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},0,0,1,1,1,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}},1,77ffcc29-7f2d-4223-b22f-19666e7250ba,{48,{1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,2,\"Field\"},\"\",\"\",0,1,\"\",0,0,0,0,0,0,3,3,0,0,0,100},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<VerticalScroll>useIfNecessary</VerticalScroll>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[30].clone()], "2");
        assert_eq!(&parsed.layout[fields[40].clone()], "2");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn skips_form_customizable_for_non_vertical_layout() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,1,0,0,1,0,0,00000000-0000-0000-0000-000000000000,1,{1,0},1,0,0,0,1,0,0,0,{0},{0},1,{22,{-1,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,9,\"FormCommandBar\",{1,0}}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Customizable>false</Customizable>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[11].clone()], "1");
        assert_eq!(&parsed.layout[fields[13].clone()], "0");
        assert_eq!(&parsed.layout[fields[14].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn skips_form_save_data_in_settings_for_property_bag_layout() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,0,0,0,0,1,0,0,00000000-0000-0000-0000-000000000000,0,{1,0},0,0,1,1,1,0,0,21,5,{\"B\",0}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<SaveDataInSettings>UseList</SaveDataInSettings>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[6].clone()], "0");
        assert_eq!(&parsed.layout[fields[18].clone()], "21");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_events() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{7,{0,\"OnOpen\",\"OldOpen\"},{1,\"ChoiceProcessing\",\"OldChoice\"},{2,\"AfterWrite\",\"OldAfterWrite\"},{3,\"BeforeWrite\",\"OldBeforeWrite\"},{4,\"OnWriteAtServer\",\"OldWriteAtServer\"},{5,\"FillCheckProcessingAtServer\",\"OldFillCheck\"},{6,\"52dbb775-1631-4fd5-8c55-1615b5881dac\",\"OldBeforeClose\"}},\"Old module\",{3,{\"picture\"},\"payload\"}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Events>
		<Event name="OnOpen">NewOpen</Event>
		<Event name="ChoiceProcessing">NewChoice</Event>
		<Event name="AfterWrite">NewAfterWrite</Event>
		<Event name="BeforeWrite">NewBeforeWrite</Event>
		<Event name="OnWriteAtServer">NewWriteAtServer</Event>
		<Event name="FillCheckProcessingAtServer">NewFillCheck</Event>
		<Event name="BeforeClose">NewBeforeClose</Event>
	</Events>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains("\"OnOpen\",\"NewOpen\""));
        assert!(parsed.layout.contains("\"ChoiceProcessing\",\"NewChoice\""));
        assert!(parsed.layout.contains("\"AfterWrite\",\"NewAfterWrite\""));
        assert!(parsed.layout.contains("\"BeforeWrite\",\"NewBeforeWrite\""));
        assert!(
            parsed
                .layout
                .contains("\"OnWriteAtServer\",\"NewWriteAtServer\"")
        );
        assert!(
            parsed
                .layout
                .contains("\"FillCheckProcessingAtServer\",\"NewFillCheck\"")
        );
        assert!(
            parsed
                .layout
                .contains("\"52dbb775-1631-4fd5-8c55-1615b5881dac\",\"NewBeforeClose\"")
        );
        assert!(!parsed.layout.contains("OldOpen"));
        assert!(!parsed.layout.contains("OldChoice"));
        assert!(!parsed.layout.contains("OldAfterWrite"));
        assert!(!parsed.layout.contains("OldBeforeWrite"));
        assert!(!parsed.layout.contains("OldWriteAtServer"));
        assert!(!parsed.layout.contains("OldFillCheck"));
        assert!(!parsed.layout.contains("OldBeforeClose"));
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec![r#"{3,{"picture"},"payload"}"#]);
        assert_eq!(
            packed.plain_bytes,
            String::from_utf8(super::inflate_raw(&packed.blob)?)?.len()
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_preserves_duplicate_event_uuid_alias_handlers() -> anyhow::Result<()> {
        let duplicate_uuid = "eba5f295-c611-4dd9-84b5-22911ad60c53";
        let base = super::deflate_raw(
            format!(
                r#"{{4,{{7,{{0,"{duplicate_uuid}","OldMain"}},{{1,"{duplicate_uuid}","OldSubject"}}}},"Old module",{{0}}}}"#
            )
            .as_bytes(),
        )?;
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Events>
		<Event name="{duplicate_uuid}">ГлавнаяЗадачаПредставлениеНажатие</Event>
		<Event name="{duplicate_uuid}">ПредметНажатие</Event>
	</Events>
</Form>
"#
        );

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        assert!(
            parsed.layout.contains(&format!(
                r#""{duplicate_uuid}","ГлавнаяЗадачаПредставлениеНажатие""#
            )),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(&format!(r#""{duplicate_uuid}","ПредметНажатие""#)),
            "{}",
            parsed.layout
        );
        assert_eq!(
            parsed
                .layout
                .matches(&format!(
                    r#""{duplicate_uuid}","ГлавнаяЗадачаПредставлениеНажатие""#
                ))
                .count(),
            1,
            "{}",
            parsed.layout
        );
        assert_eq!(
            parsed
                .layout
                .matches(&format!(r#""{duplicate_uuid}","ПредметНажатие""#))
                .count(),
            1,
            "{}",
            parsed.layout
        );
        assert!(!parsed.layout.contains("OldMain"));
        assert!(!parsed.layout.contains("OldSubject"));

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_auto_command_bar() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{59,{22,{-1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},0,0,0,9,\"OldBar\",{1,0}}},\"Old module\",{3,{\"picture\"},\"payload\"}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<AutoCommandBar name="NewBar" id="-1"/>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains("\"NewBar\""));
        assert!(!parsed.layout.contains("OldBar"));
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec![r#"{3,{"picture"},"payload"}"#]);
        assert_eq!(
            packed.plain_bytes,
            String::from_utf8(super::inflate_raw(&packed.blob)?)?.len()
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_auto_command_bar_autofill() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,{22,{-1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},0,0,0,9,"OldBar",{1,0},{1,0},0,1,0,0,0,2,2,{4,4,{0},4},{8,3,0,1,100},{0,0,0},1,{1,0,1,0},0,1,0,0,0,3,3,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<AutoCommandBar name="NewBar" id="-1">
		<HorizontalAlign>Center</HorizontalAlign>
		<Autofill>false</Autofill>
	</AutoCommandBar>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains("\"NewBar\""));
        assert!(parsed.layout.contains("{1,1,0,0}"), "{}", parsed.layout);
        assert!(!parsed.layout.contains("{1,0,1,0}"));
        assert!(!parsed.layout.contains("OldBar"));
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_auto_command_bar_child_items() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,{22,{-1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},0,0,0,9,"OldBar",{1,0},{1,0},0,1,0,0,0,2,2,{4,4,{0},4},{8,3,0,1,100},{0,0,0},1,{1,0,1,0},0,1,0,0,0,3,3,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<AutoCommandBar name="NewBar" id="-1">
		<ChildItems>
			<Button name="NewButton" id="54">
				<Type>CommandBarButton</Type>
			</Button>
		</ChildItems>
	</AutoCommandBar>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let bar_fields = super::scan_braced_fields(&parsed.layout, fields[1].start)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, bar_fields[23].start)?;

        assert_eq!(&parsed.layout[bar_fields[6].clone()], r#""NewBar""#);
        assert_eq!(&parsed.layout[bar_fields[21].clone()], "1");
        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert_eq!(&parsed.layout[button_fields[5].clone()], r#""NewButton""#);
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_child_auto_command_bar_settings() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{22,{27,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,0,0,9,"RowsBar",{1,0},{1,0},0,1,0,0,0,2,2,{4,4,{0},4},{8,3,0,1,100},{0,0,0},1,{1,0,1,0},0,1,0,0,0,3,3,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<AutoCommandBar name="RowsBar" id="27">
			<HorizontalAlign>Right</HorizontalAlign>
			<Autofill>false</Autofill>
		</AutoCommandBar>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let bar_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[bar_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[bar_fields[5].clone()], "9");
        assert_eq!(&parsed.layout[bar_fields[20].clone()], "{1,2,0,0}");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_child_auto_command_bar_settings() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<AutoCommandBar name="RowsBar" id="27">
			<HorizontalAlign>Right</HorizontalAlign>
			<Autofill>false</Autofill>
		</AutoCommandBar>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let bar_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[bar_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[bar_fields[5].clone()], "9");
        assert_eq!(&parsed.layout[bar_fields[6].clone()], r#""RowsBar""#);
        assert_eq!(&parsed.layout[bar_fields[20].clone()], "{1,2,0,0}");
        assert_eq!(&parsed.layout[bar_fields[21].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn does_not_inject_text_document_context_menu_slots_into_input_field() -> anyhow::Result<()> {
        let form_uuid = "02023637-7868-4a5f-8576-835a76e0c9ba";
        let mut field_parts = vec!["0".to_string(); 43];
        field_parts[0] = "48".to_string();
        field_parts[1] = format!("{{22,{form_uuid}}}");
        field_parts[5] = "2".to_string();
        field_parts[6] = r#""Field""#.to_string();
        field_parts[7] = "1".to_string();
        field_parts[9] = "{1,0}".to_string();
        field_parts[10] = "{1,0}".to_string();
        field_parts[11] = "{0}".to_string();
        field_parts[18] = "{1,0}".to_string();
        field_parts[19] = "{1,0}".to_string();
        field_parts[41] = "1".to_string();
        field_parts[42] =
            format!(r#"{{22,{{23,{form_uuid}}},0,0,0,8,"OldMenu",{{1,0}},{{1,0}},0,1,0}}"#);
        let field = format!("{{{}}}", field_parts.join(","));
        let base_text = format!(
            "{{4,{{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{field}}},\"Old module\",{{0}}}}"
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<InputField name="Field" id="22">
			<ContextMenu name="NewMenu" id="23"/>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let field_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        assert_eq!(&parsed.layout[field_fields[41].clone()], "1");
        assert!(parsed.layout.contains("OldMenu"));
        assert!(!parsed.layout.contains("NewMenu"));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn patches_existing_text_document_field_context_menu() -> anyhow::Result<()> {
        let form_uuid = "02023637-7868-4a5f-8576-835a76e0c9ba";
        let mut field_parts = vec!["0".to_string(); 43];
        field_parts[0] = "48".to_string();
        field_parts[1] = format!("{{22,{form_uuid}}}");
        field_parts[5] = "7".to_string();
        field_parts[6] = r#""Editor""#.to_string();
        field_parts[41] = "1".to_string();
        field_parts[42] =
            format!(r#"{{22,{{23,{form_uuid}}},0,0,0,8,"OldMenu",{{1,0}},{{1,0}},0,1,0}}"#);
        let base = super::deflate_raw(
            format!(
                "{{4,{{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{{{}}}}},\"Old module\",{{0}}}}",
                field_parts.join(",")
            )
            .as_bytes(),
        )?;
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><TextDocumentField name="Editor" id="22"><ContextMenu name="NewMenu" id="23"/></TextDocumentField></ChildItems></Form>"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let field_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let context_fields = super::scan_braced_fields(&parsed.layout, field_fields[42].start)?;
        assert_eq!(&parsed.layout[field_fields[41].clone()], "1");
        assert_eq!(&parsed.layout[context_fields[6].clone()], r#""NewMenu""#);
        assert!(!parsed.layout.contains("OldMenu"));
        Ok(())
    }

    #[test]
    fn rejects_invalid_text_document_field_direct_children_before_packing() -> anyhow::Result<()> {
        let form_uuid = "02023637-7868-4a5f-8576-835a76e0c9ba";
        let mut field_parts = vec!["0".to_string(); 43];
        field_parts[0] = "48".to_string();
        field_parts[1] = format!("{{22,{form_uuid}}}");
        field_parts[5] = "7".to_string();
        field_parts[6] = r#""Editor""#.to_string();
        field_parts[41] = "0".to_string();
        let base = super::deflate_raw(
            format!(
                "{{4,{{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{{{}}}}},\"Old module\",{{0}}}}",
                field_parts.join(",")
            )
            .as_bytes(),
        )?;
        let original = super::parse_form_body_blob(&base)?.layout;
        for xml in [
            br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><TextDocumentField name="Editor" id="22"><ContextMenu name="First" id="23"/><ContextMenu name="Second" id="24"/></TextDocumentField></ChildItems></Form>"#
                .as_slice(),
            br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><TextDocumentField name="Editor" id="22"><AutoCommandBar name="Foreign" id="23"/></TextDocumentField></ChildItems></Form>"#
                .as_slice(),
        ] {
            assert!(super::pack_form_body_blob_from_form_xml(&base, xml, None).is_err());
            assert_eq!(super::parse_form_body_blob(&base)?.layout, original);
        }
        field_parts[41] = "01".to_string();
        let malformed_base = super::deflate_raw(
            format!(
                "{{4,{{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{{{}}}}},\"Old module\",{{0}}}}",
                field_parts.join(",")
            )
            .as_bytes(),
        )?;
        let malformed_original = super::parse_form_body_blob(&malformed_base)?.layout;
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><TextDocumentField name="Editor" id="22"/></ChildItems></Form>"#;
        assert!(super::pack_form_body_blob_from_form_xml(&malformed_base, xml, None).is_err());
        assert_eq!(
            super::parse_form_body_blob(&malformed_base)?.layout,
            malformed_original
        );
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_dynamic_list_fields() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{7,{"layout"}},"Old module",{4,1,{9,{1},0,"List",{1,0},{"Pattern",{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}},{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,{0,1,"Field",{0,1,{"Old.Path","OldField"}}},{0,0}}},{0,0},{0,0},{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<Attributes>
		<Attribute name="List" id="1">
			<Type>
				<v8:Type>cfg:DynamicList</v8:Type>
			</Type>
			<Settings xsi:type="DynamicList">
				<Field>
					<dataPath>List.Period</dataPath>
					<field>Period</field>
				</Field>
				<Field>
					<dataPath>List.Amount</dataPath>
					<field>Amount</field>
				</Field>
			</Settings>
		</Attribute>
	</Attributes>
</Form>
"#;

        let parsed_xml = super::parse_form_xml_body_properties(xml)?;
        let fields = &parsed_xml.attributes[0]
            .settings
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("settings were not parsed from form XML"))?
            .fields;
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].data_path.as_deref(), Some("List.Period"));
        assert_eq!(fields[0].field.as_deref(), Some("Period"));

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed.trailing[0]
                .contains(r#""Field",{0,2,{"List.Period","Period"},{"List.Amount","Amount"}}"#)
        );
        assert!(!parsed.trailing[0].contains("Old.Path"));
        assert!(!parsed.trailing[0].contains("OldField"));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_metadata_only_filter_removes_physical_filter_property()
    -> anyhow::Result<()> {
        let filter_xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Filter xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n\
\t<item xsi:type=\"FilterItemComparison\">\r\n\
\t\t<left xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xsi:type=\"dcscor:Field\">Old</left>\r\n\
\t\t<comparisonType>Equal</comparisonType>\r\n\
\t\t<right xsi:type=\"xs:string\">Old</right>\r\n\
\t</item>\r\n\
\t<viewMode>Normal</viewMode>\r\n\
\t<userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</userSettingID>\r\n\
</Filter>";
        let mut filter_bytes = b"\xEF\xBB\xBF".to_vec();
        filter_bytes.extend_from_slice(filter_xml.as_bytes());
        let filter_payload = super::encode_base64(&filter_bytes);
        let base_text = format!(
            r##"{{4,{{7,{{"layout"}}}},"Old module",{{4,1,{{9,{{1}},0,"List",{{1,0}},{{"Pattern",{{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}}}},{{0,{{0,{{"B",1}},0}}}},{{0,{{0,{{"B",1}},0}}}},{{0,0}},{{0,0}},0,0,0,0,{{0,2,"Filter",{{"#",21743ff3-2db3-4cfc-9404-90ed8209437f,{{#base64:{filter_payload}}}}},"DynamicalDataSelection",{{"B",1}}}},{{0,0}}}}}},{{0,0}},{{0,0}},{{0}}}}"##
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:dcscor="http://v8.1c.ru/8.1/data-composition-system/core" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<Attributes>
		<Attribute name="List" id="1">
			<Type>
				<v8:Type>cfg:DynamicList</v8:Type>
			</Type>
			<Settings xsi:type="DynamicList">
				<DynamicDataRead>true</DynamicDataRead>
				<ListSettings>
					<dcsset:filter>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</dcsset:userSettingID>
					</dcsset:filter>
				</ListSettings>
			</Settings>
		</Attribute>
	</Attributes>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        assert!(!parsed.trailing[0].contains("\"Filter\""));
        assert!(parsed.trailing[0].contains("\"DynamicalDataSelection\""));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_replaces_filter_through_platform_evidenced_comparison()
    -> anyhow::Result<()> {
        let filter_xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Filter xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n\
\t<item xsi:type=\"FilterItemComparison\">\r\n\
\t\t<left xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xsi:type=\"dcscor:Field\">Old</left>\r\n\
\t\t<comparisonType>Equal</comparisonType>\r\n\
\t\t<right xsi:type=\"xs:string\">Old</right>\r\n\
\t</item>\r\n\
\t<viewMode>Normal</viewMode>\r\n\
\t<userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</userSettingID>\r\n\
</Filter>";
        let mut filter_bytes = b"\xEF\xBB\xBF".to_vec();
        filter_bytes.extend_from_slice(filter_xml.as_bytes());
        let filter_payload = super::encode_base64(&filter_bytes);
        let base_text = format!(
            r##"{{4,{{7,{{"layout"}}}},"Old module",{{4,1,{{9,{{1}},0,"List",{{1,0}},{{"Pattern",{{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}}}},{{0,{{0,{{"B",1}},0}}}},{{0,{{0,{{"B",1}},0}}}},{{0,0}},{{0,0}},0,0,0,0,{{0,1,"Filter",{{"#",21743ff3-2db3-4cfc-9404-90ed8209437f,{{#base64:{filter_payload}}}}}}},{{0,0}}}}}},{{0,0}},{{0,0}},{{0}}}}"##
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:dcscor="http://v8.1c.ru/8.1/data-composition-system/core" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<Attributes>
		<Attribute name="List" id="1">
			<Type>
				<v8:Type>cfg:DynamicList</v8:Type>
			</Type>
			<Settings xsi:type="DynamicList">
				<ListSettings>
					<dcsset:filter>
						<dcsset:item xsi:type="dcsset:FilterItemComparison">
							<dcsset:left xsi:type="dcscor:Field">Code</dcsset:left>
							<dcsset:comparisonType>Equal</dcsset:comparisonType>
							<dcsset:right xsi:type="xs:string">A</dcsset:right>
						</dcsset:item>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</dcsset:userSettingID>
					</dcsset:filter>
				</ListSettings>
			</Settings>
		</Attribute>
	</Attributes>
</Form>
"#
        .as_bytes();

        let parsed_xml = super::parse_form_xml_body_properties(xml)?;
        let parsed_filter = parsed_xml.attributes[0]
            .settings
            .as_ref()
            .and_then(|settings| settings.list_settings.filter.as_ref())
            .ok_or_else(|| anyhow::anyhow!("filter was not parsed from form XML"))?;
        assert!(matches!(
            parsed_filter.canonical.items(),
            [ibcmd_core::dcs::DcsFilterItem::Comparison(comparison)]
                if comparison.field().as_str() == "Code"
                    && comparison.right().as_string().as_str() == "A"
        ));

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let filter_after = form_setting_base64_xml_for_test(&parsed.trailing[0], "Filter")?;

        assert!(filter_after.contains(">Code</left>"), "{filter_after}");
        assert!(
            filter_after.contains("<right xsi:type=\"xs:string\">A</right>"),
            "{filter_after}"
        );
        assert!(
            filter_after.contains("FilterItemComparison"),
            "{filter_after}"
        );
        assert!(!filter_after.contains(">Old</left>"));
        assert!(filter_after.contains("<viewMode>Normal</viewMode>"));
        assert!(
            filter_after
                .contains("<userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</userSettingID>")
        );
        let filter_uuid = ibcmd_schema::bundled_dcs_filter_policy()?
            .comparison_storage_record_type_uuid()
            .to_string();
        assert!(
            parsed.trailing[0].contains(&format!("\"Filter\",{{\"#\",{filter_uuid},{{#base64:"))
        );
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_dynamic_order_preserves_equivalent_wrapped_payload()
    -> anyhow::Result<()> {
        let order_xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Order xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n\
\t<item xsi:type=\"OrderItemField\">\r\n\
\t\t<field>Код</field>\r\n\
\t\t<orderType>Asc</orderType>\r\n\
\t</item>\r\n\
\t<viewMode>Normal</viewMode>\r\n\
\t<userSettingID>88619765-ccb3-46c6-ac52-38e9c992ebd4</userSettingID>\r\n\
</Order>";
        let mut order_bytes = b"\xEF\xBB\xBF".to_vec();
        order_bytes.extend_from_slice(order_xml.as_bytes());
        let order_payload = super::encode_base64(&order_bytes);
        let base_text = format!(
            r##"{{4,{{7,{{"layout"}}}},"Old module",{{4,1,{{9,{{1}},0,"List",{{1,0}},{{"Pattern",{{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}}}},{{0,{{0,{{"B",1}},0}}}},{{0,{{0,{{"B",1}},0}}}},{{0,0}},{{0,0}},0,0,0,0,{{0,1,"Order",{{"#",11743ff3-2db3-4cfc-9404-90ed8209437f,
{{#base64:{order_payload}
}}
}}}},{{0,0}}}}}},{{0,0}},{{0,0}},{{0}}}}"##
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<Attributes>
		<Attribute name="List" id="1">
			<Type>
				<v8:Type>cfg:DynamicList</v8:Type>
			</Type>
			<Settings xsi:type="DynamicList">
				<ListSettings>
					<dcsset:order>
						<dcsset:item xsi:type="dcsset:OrderItemField">
							<dcsset:field>Код</dcsset:field>
							<dcsset:orderType>Asc</dcsset:orderType>
						</dcsset:item>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>88619765-ccb3-46c6-ac52-38e9c992ebd4</dcsset:userSettingID>
					</dcsset:order>
				</ListSettings>
			</Settings>
		</Attribute>
	</Attributes>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let order_after = form_setting_base64_xml_for_test(&parsed.trailing[0], "Order")?;

        assert!(order_after.contains("<field>Код</field>"), "{order_after}");
        assert!(
            order_after
                .contains("<userSettingID>88619765-ccb3-46c6-ac52-38e9c992ebd4</userSettingID>")
        );
        assert!(
            parsed.trailing[0]
                .contains("\"Order\",{\"#\",11743ff3-2db3-4cfc-9404-90ed8209437f,\n{#base64:"),
            "{}",
            parsed.trailing[0]
        );
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_dynamic_query_text_preserves_crlf_and_entity_ref()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{7,{\"layout\"}},\"Old module\",{4,1,{9,{1},0,\"List\",{1,0},{\"Pattern\",{\"#\",65abad24-838b-4987-8b35-ed9e2bd4d9c8}},{0,{0,{\"B\",1},0}},{0,{0,{\"B\",1},0}},{0,0},{0,0},0,0,0,0,{0,1,\"QueryText\",{\"S\",\"SELECT\r\n\t&Param\"}},{0,0}}},{0,0},{0,0},{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<Attributes>
		<Attribute name="List" id="1">
			<Type>
				<v8:Type>cfg:DynamicList</v8:Type>
			</Type>
			<Settings xsi:type="DynamicList">
				<QueryText>SELECT
	&amp;Param</QueryText>
			</Settings>
		</Attribute>
	</Attributes>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.trailing[0].contains("SELECT\r\n\t&Param"));
        assert!(!parsed.trailing[0].contains("&amp;Param"));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_removes_missing_events() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{7,{0,\"OnOpen\",\"OldOpen\"},{1,\"ChoiceProcessing\",\"OldChoice\"}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Events>
		<Event name="OnOpen">NewOpen</Event>
	</Events>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains("\"OnOpen\",\"NewOpen\""));
        assert!(!parsed.layout.contains("ChoiceProcessing"));
        assert!(!parsed.layout.contains("OldChoice"));
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_clears_empty_events_section() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{7,{0,\"OnOpen\",\"OldOpen\"},{1,\"ChoiceProcessing\",\"OldChoice\"}},\"Old module\",{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Events/>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.layout, "{7}");
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_attribute() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Attributes>
		<Attribute name="Description" id="1">
			<Type>
				<v8:Type>xs:string</v8:Type>
			<v8:StringQualifiers>
				<v8:Length>80</v8:Length>
				<v8:AllowedLength>Variable</v8:AllowedLength>
			</v8:StringQualifiers>
			</Type>
			<MainAttribute>true</MainAttribute>
		</Attribute>
	</Attributes>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let attributes_fields = super::scan_braced_fields(&parsed.trailing[0], 0)?;
        let attribute_fields =
            super::scan_braced_fields(&parsed.trailing[0], attributes_fields[2].start)?;

        assert_eq!(parsed.layout, r#"{7,{"layout"}}"#);
        assert_eq!(&parsed.trailing[0][attributes_fields[0].clone()], "4");
        assert_eq!(&parsed.trailing[0][attributes_fields[1].clone()], "1");
        assert_eq!(&parsed.trailing[0][attribute_fields[0].clone()], "9");
        assert_eq!(&parsed.trailing[0][attribute_fields[1].clone()], "{1}");
        assert_eq!(
            &parsed.trailing[0][attribute_fields[3].clone()],
            r#""Description""#
        );
        assert_eq!(&parsed.trailing[0][attribute_fields[10].clone()], "1");
        assert!(
            parsed.trailing[0].contains(r#""S",80"#),
            "{}",
            parsed.trailing[0]
        );
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_removes_missing_attributes() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{7,{"layout"}},"Old module",{4,2,{9,{1},0,"Keep",{1,0},{"Pattern",{"S"}},{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,{0,0},{0,0}},{9,{2},0,"Drop",{1,0},{"Pattern",{"S"}},{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,{0,0},{0,0}}},{0,0},{0,0},{0}}"#,
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Attributes>
		<Attribute name="Keep" id="1">
			<Type>
				<v8:Type>xs:string</v8:Type>
			</Type>
		</Attribute>
	</Attributes>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let attributes_fields = super::scan_braced_fields(&parsed.trailing[0], 0)?;

        assert_eq!(&parsed.trailing[0][attributes_fields[0].clone()], "4");
        assert_eq!(&parsed.trailing[0][attributes_fields[1].clone()], "1");
        assert!(parsed.trailing[0].contains(r#""Keep""#));
        assert!(!parsed.trailing[0].contains(r#""Drop""#));
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_clears_empty_attributes_section() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{7,{"layout"}},"Old module",{4,1,{9,{1},0,"Drop",{1,0},{"Pattern",{"S"}},{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,{0,0},{0,0}}},{0,0},{0,0},{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Attributes/>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_parameter_preserves_equivalent_type_format()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{7,{"layout"}},"Old module",{0},{0,1,{0,"Text",{"Pattern",
{"S"}
},1}},{0,0},{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Parameters>
		<Parameter name="Text">
			<Type>
				<v8:Type>xs:string</v8:Type>
			</Type>
			<KeyParameter>true</KeyParameter>
		</Parameter>
	</Parameters>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.trailing[1].contains("\"Text\",{\"Pattern\",\n{\"S\"}\n},1"));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_parameter_string_qualifiers() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Parameters>
		<Parameter name="Text">
			<Type>
				<v8:Type>xs:string</v8:Type>
				<v8:StringQualifiers>
					<v8:Length>30</v8:Length>
					<v8:AllowedLength>Fixed</v8:AllowedLength>
				</v8:StringQualifiers>
			</Type>
			<KeyParameter>true</KeyParameter>
		</Parameter>
	</Parameters>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        // `Fixed` is the 0 and `Variable` the 1, not the other way round:
        // measured over every form of ERP УХ whose `<Type>` is a lone
        // `xs:string` with a non-zero `<Length>` -- 283 parameters, 5 343
        // attributes and 34 164 value-table columns -- with no exception in
        // any of the three. This assertion used to hold the reading the code
        // had, which the corpus contradicts.
        assert!(
            parsed.trailing[1].contains(r#""Text",{"Pattern",{"S",30,0}},1"#),
            "{}",
            parsed.trailing[1]
        );
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_removes_missing_parameters() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{7,{"layout"}},"Old module",{0},{0,2,{0,"Keep",{"Pattern",{"S"}},0},{0,"Drop",{"Pattern",{"S"}},0}},{0,0},{0}}"#,
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Parameters>
		<Parameter name="Keep">
			<Type>
				<v8:Type>xs:string</v8:Type>
			</Type>
		</Parameter>
	</Parameters>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let parameters_fields = super::scan_braced_fields(&parsed.trailing[1], 0)?;

        assert_eq!(&parsed.trailing[1][parameters_fields[0].clone()], "0");
        assert_eq!(&parsed.trailing[1][parameters_fields[1].clone()], "1");
        assert!(parsed.trailing[1].contains(r#""Keep""#));
        assert!(!parsed.trailing[1].contains(r#""Drop""#));
        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_clears_empty_parameters_section() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{7,{"layout"}},"Old module",{0},{0,1,{0,"Drop",{"Pattern",{"S"}},0}},{0,0},{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Parameters/>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_command_preserves_nested_localized_values() -> anyhow::Result<()>
    {
        let base = super::deflate_raw(
            b"{4,{7,{\"layout\"}},\"Old module\",{0},{0,0},{0,1,{11,{2,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},\"Do\",{1,1,{\"ru\",\"Title\"}},{1,1,{\"ru\",\"Tip\"}},0,0,0,\"Do\",0,0,0,{0,0}}},{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Commands>
		<Command name="Do" id="2">
			<Title>
				<v8:item>
					<v8:lang>ru</v8:lang>
					<v8:content>Title</v8:content>
				</v8:item>
			</Title>
			<ToolTip>
				<v8:item>
					<v8:lang>ru</v8:lang>
					<v8:content>Tip</v8:content>
				</v8:item>
			</ToolTip>
			<Action>Do</Action>
		</Command>
	</Commands>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.trailing[2].contains(r#""Do",{1,1,{"ru","Title"}},{1,1,{"ru","Tip"}}"#));
        assert!(!parsed.trailing[2].contains(r#"{1,"ru","Title"}"#));
        assert!(!parsed.trailing[2].contains(r#"{1,"ru","Tip"}"#));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_removes_missing_commands() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{7,{\"layout\"}},\"Old module\",{0},{0,0},{0,2,{11,{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},\"Keep\",{0},{0},0,0,0,\"KeepAction\",0,0,0,{0,0}},{11,{2,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},\"Drop\",{0},{0},0,0,0,\"DropAction\",0,0,0,{0,0}}},{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Commands>
		<Command name="Keep" id="1">
			<Action>KeepActionNew</Action>
		</Command>
	</Commands>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let commands_fields = super::scan_braced_fields(&parsed.trailing[2], 0)?;

        assert_eq!(&parsed.trailing[2][commands_fields[0].clone()], "0");
        assert_eq!(&parsed.trailing[2][commands_fields[1].clone()], "1");
        assert!(parsed.trailing[2].contains(r#""Keep""#));
        assert!(parsed.trailing[2].contains(r#""KeepActionNew""#));
        assert!(!parsed.trailing[2].contains(r#""Drop""#));
        assert!(!parsed.trailing[2].contains("DropAction"));
        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_clears_empty_commands_section() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{4,{7,{\"layout\"}},\"Old module\",{0},{0,0},{0,1,{11,{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},\"Drop\",{0},{0},0,0,0,\"DropAction\",0,0,0,{0,0}}},{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Commands/>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_command_interface() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0,1,{3,0,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{0},1,{0,eacad741-96b9-4b3a-bf79-dde9ecead1a1},0,1,{0,{0,{"B",1},0}}}}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandInterface>
		<NavigationPanel>
			<Item>
				<Command>DataProcessor.Loader.Command.Load</Command>
				<Type>Added</Type>
				<CommandGroup>FormNavigationPanelImportant</CommandGroup>
				<Index>2</Index>
				<DefaultVisible>false</DefaultVisible>
				<Visible>
					<xr:Common>false</xr:Common>
				</Visible>
			</Item>
		</NavigationPanel>
	</CommandInterface>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.layout, r#"{7,{"layout"}}"#);
        assert_eq!(parsed.module_text, "Old module");
        assert!(parsed.trailing[3].contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"));
        assert!(parsed.trailing[3].contains("dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2"));
        assert!(!parsed.trailing[3].contains("eacad741-96b9-4b3a-bf79-dde9ecead1a1"));
        assert!(parsed.trailing[3].contains("},2,0,{0,{0,{\"B\",0},0}}"));
        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(
            packed.plain_bytes,
            String::from_utf8(super::inflate_raw(&packed.blob)?)?.len()
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_removes_missing_command_interface_items() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0,2,{3,0,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{0},1,{0,eacad741-96b9-4b3a-bf79-dde9ecead1a1},0,1,{0,{0,{"B",1},0}}},{3,0,{0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},{0},1,{0,dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2},1,1,{0,{0,{"B",1},0}}}}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<CommandInterface>
		<NavigationPanel>
			<Item>
				<CommandGroup>FormNavigationPanelImportant</CommandGroup>
				<Index>2</Index>
			</Item>
		</NavigationPanel>
	</CommandInterface>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.trailing[3], 0)?;

        assert_eq!(&parsed.trailing[3][fields[0].clone()], "0");
        assert_eq!(&parsed.trailing[3][fields[1].clone()], "1");
        assert!(parsed.trailing[3].contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"));
        assert!(!parsed.trailing[3].contains("bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"));
        assert!(parsed.trailing[3].contains("dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2"));
        assert!(parsed.trailing[3].contains("},2,1,"));

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_clears_empty_command_interface_section() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0,1,{3,0,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{0},1,{0,eacad741-96b9-4b3a-bf79-dde9ecead1a1},0,1,{0,{0,{"B",1},0}}}}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<CommandInterface/>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.trailing[3], "{0}");
        assert_eq!(parsed.layout, r#"{7,{"layout"}}"#);
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_child_items() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            r##"{4,{59,2,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{22,{64,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,0,0,0,"OldBar",{1,"en","Old bar"},0,1,1,cccccccc-cccc-4ccc-cccc-cccccccccccc,{34,{44,dddddddd-dddd-4ddd-dddd-dddddddddddd},0,0,0,"OldButton",{1,"en","Old button"},1,{0,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee},{0}}},ffffffff-ffff-4fff-ffff-ffffffffffff,{73,{25,11111111-1111-4111-8111-111111111111},0,1,0,"Rows",0,0,0,{1,0},1,22222222-2222-4222-8222-222222222222,{48,{40,33333333-3333-4333-8333-333333333333},0,0,0,2,"Наименование",1,0,{1,0},"OnChange","OldChange","StartChoice","OldChoice"},"97365900-eadf-4dfd-a9aa-fbb9ecabd079","OldGetData"}},"Old module",{0}}"##
                .as_bytes(),
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Commands>
		<Command name="Do" id="2">
			<Action>Do</Action>
		</Command>
	</Commands>
	<ChildItems>
		<CommandBar name="NewBar" id="64">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>New bar</v8:content>
				</v8:item>
			</Title>
			<ChildItems>
				<Button name="NewButton" id="44">
					<Type>UsualButton</Type>
					<CommandName>Form.Command.Do</CommandName>
					<DataPath>Items.Rows.CurrentData.Наименование</DataPath>
					<Title>
						<v8:item>
							<v8:lang>en</v8:lang>
							<v8:content>New button</v8:content>
						</v8:item>
					</Title>
				</Button>
				<InputField name="Наименование" id="40">
					<Events>
						<Event name="OnChange">NewChange</Event>
						<Event name="StartChoice">NewChoice</Event>
					</Events>
				</InputField>
			</ChildItems>
		</CommandBar>
		<Table name="Rows" id="25">
			<Events>
				<Event name="OnGetDataAtServer">NewGetData</Event>
			</Events>
			<ChildItems/>
		</Table>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#""NewBar""#));
        assert!(parsed.layout.contains(r#"{1,"en","New bar"}"#));
        assert!(parsed.layout.contains(r#""NewButton""#));
        assert!(parsed.layout.contains(r#"{1,"en","New button"}"#));
        assert!(
            parsed
                .layout
                .contains(r#""NewButton",{1,"en","New button"},0,"#),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains("{2,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee}")
        );
        assert!(
            !parsed
                .layout
                .contains("{0,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee}")
        );
        assert!(parsed.layout.contains("{2,{25},{40}}"));
        assert!(
            parsed
                .layout
                .contains(r#""97365900-eadf-4dfd-a9aa-fbb9ecabd079","NewGetData""#)
        );
        assert!(parsed.layout.contains(r#""OnChange","NewChange""#));
        assert!(parsed.layout.contains(r#""StartChoice","NewChoice""#));
        assert!(!parsed.layout.contains("OldGetData"));
        assert!(!parsed.layout.contains("OldChange"));
        assert!(!parsed.layout.contains("OldChoice"));
        assert!(!parsed.layout.contains("OldBar"));
        assert!(!parsed.layout.contains("Old bar"));
        assert!(!parsed.layout.contains("OldButton"));
        assert!(!parsed.layout.contains("Old button"));
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);
        assert_eq!(
            packed.plain_bytes,
            String::from_utf8(super::inflate_raw(&packed.blob)?)?.len()
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_table_columns_column_as_direct_table_child() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{25,11111111-1111-4111-8111-111111111111},0,1,0,"Rows",0,0,0,{1,0},1,22222222-2222-4222-8222-222222222222,{48,{40,33333333-3333-4333-8333-333333333333},0,0,0,1,"OldColumn",1,0,{1,"en","Old title"}}}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<Table name="Rows" id="25">
			<Columns>
				<Column name="NewColumn" id="40">
					<Type>LabelField</Type>
					<Title>
						<v8:item>
							<v8:lang>en</v8:lang>
							<v8:content>New title</v8:content>
						</v8:item>
					</Title>
				</Column>
			</Columns>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let column_fields = super::scan_braced_fields(&parsed.layout, table_fields[12].start)?;

        assert_eq!(&parsed.layout[table_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[column_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[column_fields[5].clone()], "1");
        assert_eq!(&parsed.layout[column_fields[6].clone()], r#""NewColumn""#);
        assert_eq!(
            &parsed.layout[column_fields[9].clone()],
            r#"{1,"en","New title"}"#
        );
        assert!(!parsed.layout.contains("OldColumn"));
        assert!(!parsed.layout.contains("Old title"));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_picture_decoration() -> anyhow::Result<()> {
        let item_uuid = uuid::Uuid::new_v4().hyphenated().to_string();
        let layout = format!(
            r#"{{59,1,{item_uuid},{{12,{{165,{item_uuid}}},0,0,0,1,"OldPicture",{{1,"en","Old picture"}},{{1,0}},{{#base64:R0lGODlh}},"OnClick","OldClick"}}}}"#
        );
        let base_text = format!(r#"{{4,{layout},"Old module",{{0}}}}"#);
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<ChildItems>
		<PictureDecoration name="NewPicture" id="165">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>New picture</v8:content>
				</v8:item>
			</Title>
			<Picture>
				<xr:Abs>Picture.gif</xr:Abs>
				<xr:LoadTransparent>false</xr:LoadTransparent>
			</Picture>
			<Events>
				<Event name="OnClick">NewClick</Event>
			</Events>
		</PictureDecoration>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed.layout.contains(r#""NewPicture""#),
            "{}",
            parsed.layout
        );
        assert!(
            parsed.layout.contains(r#"{1,"en","New picture"}"#),
            "{}",
            parsed.layout
        );
        assert!(parsed.layout.contains(r#""OnClick","NewClick""#));
        assert!(!parsed.layout.contains("OldPicture"));
        assert!(!parsed.layout.contains("Old picture"));
        assert!(!parsed.layout.contains("OldClick"));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_picture_decoration() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<PictureDecoration name="Logo" id="165">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Logo</v8:content>
				</v8:item>
			</Title>
			<Events>
				<Event name="OnClick">LogoClick</Event>
			</Events>
		</PictureDecoration>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let picture_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert!(super::is_uuid_like_ascii(
            parsed.layout[layout_fields[2].clone()].as_bytes()
        ));
        assert_eq!(&parsed.layout[picture_fields[0].clone()], "12");
        assert_eq!(&parsed.layout[picture_fields[5].clone()], "1");
        assert_eq!(&parsed.layout[picture_fields[6].clone()], r#""Logo""#);
        assert_eq!(
            &parsed.layout[picture_fields[7].clone()],
            r#"{1,"en","Logo"}"#
        );
        assert_eq!(&parsed.layout[picture_fields[9].clone()], "{0}");
        assert!(parsed.layout.contains(r#""OnClick","LogoClick""#));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_checkbox_preserves_shifted_title_slot() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{48,{104,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,3,"RequiredApproval",1,0,{1,0},{1,1,{"ru","""Approval"" text"}},{2,{1},{0,5651130b-3653-479e-ac26-905647417c29}},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},{4,4,{0},4},{8,3,0,1,100},{4,4,{0},4},{4,4,{0},4},{4,4,{0},4},{8,3,0,1,100},{0,0,0},0,{12,{106,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"RequiredApprovalExtendedTooltip",{1,0},{1,0},1,0,0,2,2},{0},0,0,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<CheckBoxField name="RequiredApproval" id="104">
			<Title>
				<v8:item>
					<v8:lang>ru</v8:lang>
					<v8:content>&quot;Approval&quot; text</v8:content>
				</v8:item>
			</Title>
			<ExtendedTooltip name="RequiredApprovalExtendedTooltip" id="106"/>
		</CheckBoxField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r#""RequiredApproval",1,0,{1,0},{1,1,{"ru","""Approval"" text"}}"#)
        );
        assert!(!parsed.layout.contains(
            r#""RequiredApproval",1,0,{1,"ru","""Approval"" text"},{1,1,{"ru","""Approval"" text"}}"#
        ));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_checkbox_field() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<CheckBoxField name="RequiredApproval" id="104">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Approval required</v8:content>
				</v8:item>
			</Title>
		</CheckBoxField>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let checkbox_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[checkbox_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[checkbox_fields[5].clone()], "3");
        assert_eq!(
            &parsed.layout[checkbox_fields[6].clone()],
            r#""RequiredApproval""#
        );
        assert_eq!(&parsed.layout[checkbox_fields[9].clone()], "{1,0}");
        assert_eq!(
            &parsed.layout[checkbox_fields[10].clone()],
            r#"{1,"en","Approval required"}"#
        );
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_table_preserves_extended_data_path() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{52,02023637-7868-4a5f-8576-835a76e0c9ba},0,1,0,"Rows",0,0,1,{1,0},{1,0},{2,{1},{0,573cf342-5482-4ff7-ae70-2b91deaeaf0d}},0,1,0,0,0,1,1,0,0,0,{0}}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="52">
			<DataPath>Object</DataPath>
		</Table>
	</ChildItems>
	<Attributes>
		<Attribute name="Object" id="1"/>
	</Attributes>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains("{2,{1},{0,573cf342-5482-4ff7-ae70-2b91deaeaf0d}}")
        );
        assert!(!parsed.layout.contains("{1,{1}},0,1,0,0,0"));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_table_preserves_root_object_data_path() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{52,02023637-7868-4a5f-8576-835a76e0c9ba},0,1,0,"Rows",0,0,1,{1,0},{1,0},{0},0,1,0,0,0,1,1,0,0,0,{0}}},"Old module",{0}}"#,
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="52">
			<DataPath>Объект</DataPath>
		</Table>
	</ChildItems>
	<Attributes>
		<Attribute name="Объект" id="1"/>
	</Attributes>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#""Rows",0,0,1,{1,0},{1,0},{0}"#));
        assert!(!parsed.layout.contains("{1,{1}},0,1,0,0,0"));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_extended_tooltip_preserves_empty_name_slot()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{48,{20,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,1,"InfoLabel",1,0,{1,0},{1,0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1,3,0,{4,0,{0},"",-1,-1,1,0,""},{4,0,{0},"",-1,-1,1,0,""},{4,4,{0},4},{8,3,0,1,100},{4,4,{0},4},{4,4,{0},4},{4,4,{0},4},{8,3,0,1,100},{0,0,0},1,{12,0,0,2,2,2,{1,0},0,{4,4,{0},4},{4,4,{0},4},{8,3,0,1,100},2,{0,1,0},{4,4,{0},4},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},1,0,0,1,0,2},{0},0,0,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<LabelField name="InfoLabel" id="20">
			<ExtendedTooltip name="InfoLabelExtendedTooltip" id="22"/>
		</LabelField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains("{12,0,0,2,2,2,{1,0},0"));
        assert!(!parsed.layout.contains("InfoLabelExtendedTooltip"));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_removes_missing_top_level_child_items() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,2,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{34,{44,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,0,0,"KeepButton",{0},1,{0},{0}},cccccccc-cccc-4ccc-cccc-cccccccccccc,{73,{25,dddddddd-dddd-4ddd-dddd-dddddddddddd},0,1,0,"DropTable",0,0,0,{1,0},1}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Button name="KeepButton" id="44"></Button>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[0].clone()], "59");
        assert_eq!(&parsed.layout[fields[1].clone()], "1");
        assert!(parsed.layout.contains("KeepButton"));
        assert!(!parsed.layout.contains("DropTable"));
        assert!(
            !parsed
                .layout
                .contains("cccccccc-cccc-4ccc-cccc-cccccccccccc")
        );
        assert!(
            !parsed
                .layout
                .contains("dddddddd-dddd-4ddd-dddd-dddddddddddd")
        );
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_clears_empty_child_items_section() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{34,{44,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,0,0,"DropButton",{0},1,{0},{0}}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems/>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.layout, "{59,0}");
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_top_level_button() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<Button name="HelpButton" id="44">
			<Type>Hyperlink</Type>
			<CommandName>Form.StandardCommand.Help</CommandName>
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Help</v8:content>
				</v8:item>
			</Title>
		</Button>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[0].clone()], "59");
        assert_eq!(&parsed.layout[fields[1].clone()], "1");
        assert!(super::is_uuid_like_ascii(
            parsed.layout[fields[2].clone()].as_bytes()
        ));
        assert!(parsed.layout.contains(r#"{34,{44,"#), "{}", parsed.layout);
        assert!(parsed.layout.contains(r#""HelpButton""#));
        assert!(parsed.layout.contains(r#"{1,"en","Help"}"#));
        assert!(
            parsed
                .layout
                .contains(",2,{0,39bb0fe9-771d-4dd5-8a6e-2d16984523af},{0}")
        );
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_top_level_button_default_button() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Button name="Save" id="44">
			<DefaultButton>true</DefaultButton>
		</Button>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert!(button_fields.len() > 20, "{}", parsed.layout);
        assert_eq!(&parsed.layout[button_fields[11].clone()], "1");
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_top_level_button_with_local_command() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0},{0,0},{0,0},{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Commands>
		<Command name="Do" id="2">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Run</v8:content>
				</v8:item>
			</Title>
			<Action>Do</Action>
			<CurrentRowUse>DontUse</CurrentRowUse>
		</Command>
	</Commands>
	<ChildItems>
		<Button name="RunButton" id="44">
			<Type>CommandBarButton</Type>
			<CommandName>Form.Command.Do</CommandName>
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Run</v8:content>
				</v8:item>
			</Title>
		</Button>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let button_command_fields =
            super::scan_braced_fields(&parsed.layout, button_fields[8].start)?;
        let command_uuid = parsed.layout[button_command_fields[1].clone()].to_string();
        let commands_fields = super::scan_braced_fields(&parsed.trailing[2], 0)?;
        let command_entry_fields =
            super::scan_braced_fields(&parsed.trailing[2], commands_fields[2].start)?;
        let command_identity_fields =
            super::scan_braced_fields(&parsed.trailing[2], command_entry_fields[1].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[button_command_fields[0].clone()], "2");
        assert!(super::is_uuid_like_ascii(command_uuid.as_bytes()));
        assert_eq!(&parsed.trailing[2][commands_fields[1].clone()], "1");
        assert_eq!(&parsed.trailing[2][command_identity_fields[0].clone()], "2");
        assert_eq!(
            &parsed.trailing[2][command_identity_fields[1].clone()],
            command_uuid
        );
        assert!(parsed.trailing[2].contains(r#""Do",{1,"en","Run"}"#));
        assert!(parsed.trailing[2].contains(r#""Do",3,0,0,{0,0}"#));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_extended_button_properties() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{34,{44,22222222-2222-4222-8222-222222222222},0,0,0,"Save",{1,0},1,{0},{0},3,0,0,0,2,2,0,0,0,{4,4,{0},4},{4,4,{0},4},{4,4,{0},4},{8,3,0,1,100},{0,0,0},0,{4,0,{0},"",-1,-1,1,0,""},1,{"Pattern"},"",0,0,1,{12,{45,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"SaveExtendedTooltip",{1,0},{1,0},1,0,0,2,2,{4,4,{0},4},{4,4,{0},4},{4,4,{0},4},{0},0,0,0,1,{1,0},{0,0,0},0,3},{"U"},1,0,0,1,0,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<Button name="Save" id="44">
			<Type>CommandBarButton</Type>
			<Representation>PictureAndText</Representation>
			<DefaultButton>true</DefaultButton>
			<ExtendedTooltip name="SaveHelp" id="46"/>
		</Button>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[button_fields[4].clone()], "0");
        assert_eq!(&parsed.layout[button_fields[10].clone()], "2");
        assert_eq!(&parsed.layout[button_fields[11].clone()], "1");
        assert!(
            parsed
                .layout
                .contains(r#"{12,{46,02023637-7868-4a5f-8576-835a76e0c9ba}"#)
        );
        assert!(parsed.layout.contains(r#""SaveHelp""#));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_button_skip_on_input() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let base = super::deflate_raw(
                br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{34,{44,22222222-2222-4222-8222-222222222222},0,0,0,"Save",{1,0},1,{0},{0},3,0,0,0,2,2,0,0,0,{4,4,{0},4},{4,4,{0},4},{4,4,{0},4},{8,3,0,1,100},{0,0,0},0,{4,0,{0},"",-1,-1,1,0,""},1,{"Pattern"},"",2,0,1,{12,{45,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"SaveExtendedTooltip",{1,0},{1,0},1,0,0,2,2,{4,4,{0},4},{4,4,{0},4},{4,4,{0},4},{0},0,0,0,1,{1,0},{0,0,0},0,3},{"U"},1,0,0,1,0,0}},"Old module",{0}}"#,
            )?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<Button name="Save" id="44">
			<SkipOnInput>{value}</SkipOnInput>
		</Button>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let button_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
            assert_eq!(&parsed.layout[button_fields[29].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_button_location_in_command_bar() -> anyhow::Result<()> {
        for (value, expected_code) in [
            ("InAdditionalSubmenu", "1"),
            ("InCommandBar", "2"),
            ("InCommandBarAndInAdditionalSubmenu", "3"),
        ] {
            let button = [
                "34",
                "{44,22222222-2222-4222-8222-222222222222}",
                "0",
                "0",
                "0",
                "\"Save\"",
                "{1,0}",
                "1",
                "{0}",
                "{0}",
                "3",
                "0",
                "0",
                "0",
                "2",
                "2",
                "0",
                "0",
                "0",
                "{4,4,{0},4}",
                "{4,4,{0},4}",
                "{4,4,{0},4}",
                "{8,3,0,1,100}",
                "{0,0,0}",
                "0",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "1",
                "{\"Pattern\"}",
                "\"\"",
                "2",
                "0",
                "1",
                "{0}",
                "{\"U\"}",
                "1",
                "0",
                "0",
                "1",
                "0",
                "0",
                "0",
                "3",
                "3",
                "3",
                "0",
                "0",
                "0",
                "0",
                "0",
                "0",
                "1",
                "0",
                "0",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "0",
                "0",
                "0",
                "1",
                "\"\"",
            ]
            .join(",");
            let layout = format!(
                "{{4,{{59,1,11111111-1111-4111-8111-111111111111,{{{button}}}}},\"Old module\",{{0}}}}"
            );
            let base = super::deflate_raw(layout.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<Button name="Save" id="44">
			<LocationInCommandBar>{value}</LocationInCommandBar>
		</Button>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let button_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
            assert_eq!(&parsed.layout[button_fields[49].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_extended_button_show_title() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let button = [
                "34",
                "{44,22222222-2222-4222-8222-222222222222}",
                "0",
                "0",
                "0",
                "\"Save\"",
                "{1,0}",
                "1",
                "{0}",
                "{0}",
                "3",
                "0",
                "0",
                "0",
                "2",
                "2",
                "0",
                "0",
                "0",
                "{4,4,{0},4}",
                "{4,4,{0},4}",
                "{4,4,{0},4}",
                "{8,3,0,1,100}",
                "{0,0,0}",
                "0",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "1",
                "{\"Pattern\"}",
                "\"\"",
                "0",
                "0",
                "1",
                "{0}",
                "{\"U\"}",
                "1",
                "0",
                "0",
                "1",
                "0",
                "0",
                "0",
                "3",
                "3",
                "3",
                "0",
                "0",
                "0",
                "0",
                "0",
                "0",
                "1",
                "0",
                "0",
                "{4,0,{0},\"\",-1,-1,1,0,\"\"}",
                "0",
                "0",
                "0",
                "1",
                "\"\"",
            ]
            .join(",");
            let layout = format!(
                "{{4,{{59,1,11111111-1111-4111-8111-111111111111,{{{button}}}}},\"Old module\",{{0}}}}"
            );
            let base = super::deflate_raw(layout.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<Button name="Save" id="44">
			<ShowTitle>{value}</ShowTitle>
		</Button>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let button_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
            assert_eq!(&parsed.layout[button_fields[7].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_top_level_groups() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<CommandBar name="Actions" id="64">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Actions</v8:content>
				</v8:item>
			</Title>
		</CommandBar>
		<UsualGroup name="MainGroup" id="22">
			<Group>Horizontal</Group>
			<ShowTitle>false</ShowTitle>
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Main</v8:content>
				</v8:item>
			</Title>
		</UsualGroup>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.layout, 0)?;

        assert_eq!(&parsed.layout[fields[0].clone()], "59");
        assert_eq!(&parsed.layout[fields[1].clone()], "2");
        assert!(super::is_uuid_like_ascii(
            parsed.layout[fields[2].clone()].as_bytes()
        ));
        assert!(super::is_uuid_like_ascii(
            parsed.layout[fields[4].clone()].as_bytes()
        ));
        assert!(parsed.layout.contains(r#"{22,{64,"#), "{}", parsed.layout);
        assert!(
            parsed
                .layout
                .contains(r#",0,"Actions",{1,"en","Actions"},0,1,0}"#)
        );
        assert!(parsed.layout.contains(r#"{22,{22,"#), "{}", parsed.layout);
        assert!(
            parsed
                .layout
                .contains(r#",5,"MainGroup",{1,"en","Main"},1,0,0}"#)
        );
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_group_layout_group() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{22,{22,22222222-2222-4222-8222-222222222222},0,0,0,5,"MainGroup",{0},0,1,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<UsualGroup name="MainGroup" id="22">
			<Group>HorizontalIfPossible</Group>
			<ShowTitle>false</ShowTitle>
		</UsualGroup>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[group_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[group_fields[5].clone()], "5");
        assert_eq!(&parsed.layout[group_fields[8].clone()], "3");
        assert_eq!(&parsed.layout[group_fields[9].clone()], "0");
        assert_eq!(&parsed.layout[group_fields[10].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_pages_and_page() -> anyhow::Result<()> {
        let pages = r#"{22,{8,22222222-2222-4222-8222-222222222222},0,0,0,3,"PagesGroup",{1,0},{1,0},0,1,0,0,0,2,2,{4,4,{0},4},{8,3,0,1,100},{0,0,0},1,{4,0,{0},2,0,0},1,33333333-3333-4333-8333-333333333333,{22,{9,22222222-2222-4222-8222-222222222222},0,0,0,4,"MainPage",{1,0},{1,0},0,1,1}}"#;
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{pages}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core">
	<ChildItems>
		<Pages name="PagesGroup" id="8">
			<Height>14</Height>
			<ToolTip>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Pages tip</v8:content>
				</v8:item>
			</ToolTip>
			<ChildItems>
				<Page name="MainPage" id="9">
					<Group>HorizontalIfPossible</Group>
					<ScrollOnCompress>false</ScrollOnCompress>
					<ToolTip>
						<v8:item>
							<v8:lang>en</v8:lang>
							<v8:content>Page tip</v8:content>
						</v8:item>
					</ToolTip>
				</Page>
			</ChildItems>
		</Pages>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r#",3,"PagesGroup",{1,0},{1,"en","Pages tip"},0,1,0,0,14,2,2,"#),
            "{}",
            parsed.layout
        );
        assert!(
            parsed
                .layout
                .contains(r#",4,"MainPage",{1,0},{1,"en","Page tip"},3,1,0}"#),
            "{}",
            parsed.layout
        );
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_pages_and_page_properties() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<Pages name="PagesGroup" id="8">
			<Group>HorizontalIfPossible</Group>
			<ShowTitle>false</ShowTitle>
			<Height>14</Height>
			<ToolTip>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Pages tip</v8:content>
				</v8:item>
			</ToolTip>
			<ChildItems>
				<Page name="MainPage" id="9">
					<Group>HorizontalIfPossible</Group>
					<ShowTitle>false</ShowTitle>
					<ScrollOnCompress>false</ScrollOnCompress>
					<ToolTip>
						<v8:item>
							<v8:lang>en</v8:lang>
							<v8:content>Page tip</v8:content>
						</v8:item>
					</ToolTip>
				</Page>
			</ChildItems>
		</Pages>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let pages_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let page_fields = super::scan_braced_fields(&parsed.layout, pages_fields[23].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[pages_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[pages_fields[5].clone()], "3");
        assert_eq!(&parsed.layout[pages_fields[6].clone()], r#""PagesGroup""#);
        assert_eq!(
            &parsed.layout[pages_fields[8].clone()],
            r#"{1,"en","Pages tip"}"#
        );
        assert_eq!(&parsed.layout[pages_fields[9].clone()], "3");
        assert_eq!(&parsed.layout[pages_fields[10].clone()], "0");
        assert_eq!(&parsed.layout[pages_fields[13].clone()], "14");
        assert_eq!(&parsed.layout[pages_fields[21].clone()], "1");
        assert!(super::is_uuid_like_ascii(
            parsed.layout[pages_fields[22].clone()].as_bytes()
        ));
        assert_eq!(&parsed.layout[page_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[page_fields[5].clone()], "4");
        assert_eq!(&parsed.layout[page_fields[6].clone()], r#""MainPage""#);
        assert_eq!(
            &parsed.layout[page_fields[8].clone()],
            r#"{1,"en","Page tip"}"#
        );
        assert_eq!(&parsed.layout[page_fields[9].clone()], "3");
        assert_eq!(&parsed.layout[page_fields[10].clone()], "0");
        assert_eq!(&parsed.layout[page_fields[11].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_extended_usual_group_properties_and_children()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            r#"{4,{59,1,11111111-1111-4111-8111-111111111111,{22,{22,22222222-2222-4222-8222-222222222222},0,0,0,5,"MainGroup",{1,0},{1,0},0,1,0,0,0,2,2,{4,4,
{0},4},{8,3,0,1,100},{0,0,0},1,{38,0,0,0,0,{0},{1,0},{"Pattern"},"",{4,4,{0},4},0,0,0,1,{1,0},0,0,3,3,2,0,1,0,{4,4,{0},4},0,2,0,0,0,0,0,0,{4,0,{0},"",-1,-1,1,0,""},{0,1,0},0,0,0,0,2,0,0,0},0,1,0,1,{12,{23,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"MainGroupРасширеннаяПодсказка",{1,0},{1,0},0,0,0,2,2,{4,4,{0},4},{4,4,{0},4},{4,4,{0},4},{0},0,0,0,1,{1,0},{0,0,0},0,3},0,3,3,0}},"Old module",{0}}"#
                .as_bytes(),
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<UsualGroup name="MainGroup" id="22">
			<Group>HorizontalIfPossible</Group>
			<Behavior>Collapsible</Behavior>
			<Representation>NormalSeparation</Representation>
			<ShowTitle>false</ShowTitle>
			<ChildItems>
				<Button name="Run" id="44">
					<Type>UsualButton</Type>
				</Button>
			</ChildItems>
		</UsualGroup>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, group_fields[20].start)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, group_fields[23].start)?;

        assert_eq!(&parsed.layout[group_fields[9].clone()], "0");
        assert_eq!(&parsed.layout[group_fields[21].clone()], "1");
        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert_eq!(&parsed.layout[button_fields[5].clone()], r#""Run""#);
        assert_eq!(&parsed.layout[options_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[3].clone()], "3");
        assert_eq!(&parsed.layout[options_fields[4].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[11].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[22].clone()], "2");
        assert_eq!(&parsed.layout[options_fields[24].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[27].clone()], "2");
        assert_eq!(&parsed.layout[options_fields[28].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[35].clone()], "3");
        assert_eq!(&parsed.layout[options_fields[36].clone()], "2");
        assert!(parsed.layout.contains("{4,4,\n{0},4}"));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_extended_usual_group_properties_and_children() -> anyhow::Result<()>
    {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<UsualGroup name="MainGroup" id="22">
			<Group>HorizontalIfPossible</Group>
			<Behavior>Collapsible</Behavior>
			<Representation>NormalSeparation</Representation>
			<ShowTitle>false</ShowTitle>
			<ChildItems>
				<Button name="Run" id="44">
					<Type>UsualButton</Type>
				</Button>
			</ChildItems>
		</UsualGroup>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, group_fields[20].start)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, group_fields[23].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[group_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[group_fields[5].clone()], "5");
        assert_eq!(&parsed.layout[group_fields[6].clone()], r#""MainGroup""#);
        assert_eq!(&parsed.layout[group_fields[9].clone()], "3");
        assert_eq!(&parsed.layout[group_fields[10].clone()], "0");
        assert_eq!(&parsed.layout[group_fields[21].clone()], "1");
        assert!(super::is_uuid_like_ascii(
            parsed.layout[group_fields[22].clone()].as_bytes()
        ));
        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert_eq!(&parsed.layout[button_fields[5].clone()], r#""Run""#);
        assert_eq!(&parsed.layout[options_fields[0].clone()], "38");
        assert_eq!(&parsed.layout[options_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[3].clone()], "3");
        assert_eq!(&parsed.layout[options_fields[4].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[11].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[22].clone()], "2");
        assert_eq!(&parsed.layout[options_fields[24].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[27].clone()], "2");
        assert_eq!(&parsed.layout[options_fields[28].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[35].clone()], "3");
        assert_eq!(&parsed.layout[options_fields[36].clone()], "2");
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_usual_group_title_color_with_form_system_codes() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><UsualGroup name="FormText" id="22"><TitleTextColor>style:FormTextColor</TitleTextColor><Behavior>Usual</Behavior></UsualGroup><UsualGroup name="Selection" id="23"><TitleTextColor>style:FieldSelectionBackColor</TitleTextColor><Behavior>Usual</Behavior></UsualGroup></ChildItems></Form>"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains("{4,3,{-3},3}"));
        assert!(parsed.layout.contains("{4,3,{-21},3}"));
        assert!(!parsed.layout.contains("{4,3,{-11},3}"));
        Ok(())
    }

    #[test]
    fn popup_behavior_transitions_preserve_undeclared_title_style_slots() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            r#"{4,{59,1,11111111-1111-4111-8111-111111111111,{22,{22,22222222-2222-4222-8222-222222222222},0,0,0,5,"MainGroup",{1,0},{1,0},0,1,0,0,0,2,2,{4,3,{0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},3},{8,2,0,{-31},1,100},{0,0,0},1,{38,0,0,0,0,{0},{1,0},{"Pattern"},"",{4,4,{0},4},0,0,0,1,{1,0},0,0,3,3,2,0,1,0,{4,4,{0},4},0,2,0,0,0,0,0,0,{4,0,{0},"",-1,-1,1,0,""},{0,1,0},0,0,0,0,2,0,0,0},0,1,0,1,{12,{23,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,0,"MainGroupExtendedTooltip",{1,0},{1,0},0,0,0,2,2,{4,4,{0},4},{4,4,{0},4},{4,4,{0},4},{0},0,0,0,1,{1,0},{0,0,0},0,3},0,3,3,0}},"Old module",{0}}"#
                .as_bytes(),
        )?;
        let popup_xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><UsualGroup name="MainGroup" id="22"><Behavior>PopUp</Behavior></UsualGroup></ChildItems></Form>"#;
        let usual_xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><UsualGroup name="MainGroup" id="22"><Behavior>Usual</Behavior></UsualGroup></ChildItems></Form>"#;

        let new_base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let new_popup = super::pack_form_body_blob_from_form_xml(&new_base, popup_xml, None)?;
        let new_popup_parsed = super::parse_form_body_blob(&new_popup.blob)?;
        let new_popup_layout_fields = super::scan_braced_fields(&new_popup_parsed.layout, 0)?;
        let new_popup_group_fields =
            super::scan_braced_fields(&new_popup_parsed.layout, new_popup_layout_fields[3].start)?;
        assert_eq!(
            &new_popup_parsed.layout[new_popup_group_fields[16].clone()],
            "{4,4,{0},4}"
        );
        assert_eq!(
            &new_popup_parsed.layout[new_popup_group_fields[17].clone()],
            "{8,3,0,1,100}"
        );

        let popup = super::pack_form_body_blob_from_form_xml(&base, popup_xml, None)?;
        let popup_parsed = super::parse_form_body_blob(&popup.blob)?;
        let popup_layout_fields = super::scan_braced_fields(&popup_parsed.layout, 0)?;
        let popup_group_fields =
            super::scan_braced_fields(&popup_parsed.layout, popup_layout_fields[3].start)?;
        let popup_options =
            super::scan_braced_fields(&popup_parsed.layout, popup_group_fields[20].start)?;

        assert_eq!(
            &popup_parsed.layout[popup_group_fields[16].clone()],
            "{4,3,{0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},3}"
        );
        assert_eq!(
            &popup_parsed.layout[popup_group_fields[17].clone()],
            "{8,2,0,{-31},1,100}"
        );
        assert_eq!(&popup_parsed.layout[popup_options[28].clone()], "2");

        let usual = super::pack_form_body_blob_from_form_xml(&popup.blob, usual_xml, None)?;
        let usual_parsed = super::parse_form_body_blob(&usual.blob)?;
        let usual_layout_fields = super::scan_braced_fields(&usual_parsed.layout, 0)?;
        let usual_group_fields =
            super::scan_braced_fields(&usual_parsed.layout, usual_layout_fields[3].start)?;
        let usual_options =
            super::scan_braced_fields(&usual_parsed.layout, usual_group_fields[20].start)?;

        assert_eq!(
            &usual_parsed.layout[usual_group_fields[16].clone()],
            "{4,3,{0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},3}"
        );
        assert_eq!(
            &usual_parsed.layout[usual_group_fields[17].clone()],
            "{8,2,0,{-31},1,100}"
        );
        assert_eq!(&usual_parsed.layout[usual_options[28].clone()], "0");
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_group_with_nested_button() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<CommandBar name="Actions" id="64">
			<ChildItems>
				<Button name="HelpButton" id="44">
					<Type>CommandBarButton</Type>
					<CommandName>Form.StandardCommand.Help</CommandName>
					<Title>
						<v8:item>
							<v8:lang>en</v8:lang>
							<v8:content>Help</v8:content>
						</v8:item>
					</Title>
				</Button>
			</ChildItems>
		</CommandBar>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, group_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[group_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[group_fields[5].clone()], "0");
        assert_eq!(&parsed.layout[group_fields[6].clone()], r#""Actions""#);
        assert_eq!(&parsed.layout[group_fields[10].clone()], "1");
        assert!(super::is_uuid_like_ascii(
            parsed.layout[group_fields[11].clone()].as_bytes()
        ));
        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert_eq!(&parsed.layout[button_fields[5].clone()], r#""HelpButton""#);
        assert_eq!(&parsed.layout[button_fields[7].clone()], "1");
        assert!(
            parsed
                .layout
                .contains("39bb0fe9-771d-4dd5-8a6e-2d16984523af")
        );
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_button_show_title() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Button name="RunButton" id="44">
			<ShowTitle>false</ShowTitle>
		</Button>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert_eq!(&parsed.layout[button_fields[4].clone()], "1");
        assert_eq!(&parsed.layout[button_fields[5].clone()], r#""RunButton""#);
        assert_eq!(&parsed.layout[button_fields[7].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_command_bar_button_location() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<CommandBar name="Actions" id="64">
			<ChildItems>
				<Button name="RunButton" id="44">
					<Type>CommandBarButton</Type>
					<LocationInCommandBar>InCommandBarAndInAdditionalSubmenu</LocationInCommandBar>
				</Button>
			</ChildItems>
		</CommandBar>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let command_bar_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let button_fields =
            super::scan_braced_fields(&parsed.layout, command_bar_fields[12].start)?;

        assert_eq!(&parsed.layout[command_bar_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert_eq!(&parsed.layout[button_fields[4].clone()], "0");
        assert_eq!(&parsed.layout[button_fields[5].clone()], r#""RunButton""#);
        assert_eq!(&parsed.layout[button_fields[49].clone()], "3");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_nested_group_tree() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<CommandBar name="Actions" id="64">
			<ChildItems>
				<ButtonGroup name="MoreActions" id="22">
					<ChildItems>
						<Button name="HelpButton" id="44">
							<Type>CommandBarButton</Type>
							<CommandName>Form.StandardCommand.Help</CommandName>
						</Button>
					</ChildItems>
				</ButtonGroup>
			</ChildItems>
		</CommandBar>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let command_bar_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let button_group_fields =
            super::scan_braced_fields(&parsed.layout, command_bar_fields[12].start)?;
        let button_fields =
            super::scan_braced_fields(&parsed.layout, button_group_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[command_bar_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[command_bar_fields[5].clone()], "0");
        assert_eq!(&parsed.layout[command_bar_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[button_group_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[button_group_fields[5].clone()], "6");
        assert_eq!(&parsed.layout[button_group_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert_eq!(&parsed.layout[button_fields[5].clone()], r#""HelpButton""#);
        assert_eq!(&parsed.layout[button_fields[7].clone()], "1");
        assert!(
            parsed
                .layout
                .contains("39bb0fe9-771d-4dd5-8a6e-2d16984523af")
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_child_event_uuid_alias() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            r#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{48,{40,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,0,0,2,"Name",1,0,{1,0},"fe115cc8-9e33-4684-a166-bd5136fe7a9f","OldChange"}},"Old module",{0}}"#
                .as_bytes(),
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Name" id="40">
			<Events>
				<Event name="OnChange">NewChange</Event>
			</Events>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r#""fe115cc8-9e33-4684-a166-bd5136fe7a9f","NewChange""#),
            "{}",
            parsed.layout
        );
        assert!(!parsed.layout.contains("OldChange"));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_appends_button_to_existing_group() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{22,{64,22222222-2222-4222-8222-222222222222},0,0,0,0,"Actions",{0},0,1,0}},"Old module",{0}}"#,
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<CommandBar name="Actions" id="64">
			<ChildItems>
				<Button name="HelpButton" id="44">
					<Type>CommandBarButton</Type>
					<CommandName>Form.StandardCommand.Help</CommandName>
				</Button>
			</ChildItems>
		</CommandBar>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let button_fields = super::scan_braced_fields(&parsed.layout, group_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[group_fields[0].clone()], "22");
        assert_eq!(
            &parsed.layout[group_fields[1].clone()],
            "{64,22222222-2222-4222-8222-222222222222}"
        );
        assert_eq!(&parsed.layout[group_fields[10].clone()], "1");
        assert!(super::is_uuid_like_ascii(
            parsed.layout[group_fields[11].clone()].as_bytes()
        ));
        assert_eq!(&parsed.layout[button_fields[0].clone()], "34");
        assert_eq!(&parsed.layout[button_fields[5].clone()], r#""HelpButton""#);
        assert_eq!(&parsed.layout[button_fields[7].clone()], "1");
        assert!(
            parsed
                .layout
                .contains("39bb0fe9-771d-4dd5-8a6e-2d16984523af")
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_removes_missing_nested_group_child_items() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{22,{64,22222222-2222-4222-8222-222222222222},0,0,0,0,"Actions",{0},0,1,2,33333333-3333-4333-8333-333333333333,{34,{44,44444444-4444-4444-8444-444444444444},0,0,0,"KeepButton",{0},1,{0},{0}},55555555-5555-4555-8555-555555555555,{34,{45,66666666-6666-4666-8666-666666666666},0,0,0,"DropButton",{0},1,{0},{0}}}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<CommandBar name="Actions" id="64">
			<ChildItems>
				<Button name="KeepButton" id="44"/>
			</ChildItems>
		</CommandBar>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[group_fields[10].clone()], "1");
        assert!(parsed.layout.contains("KeepButton"));
        assert!(!parsed.layout.contains("DropButton"));
        assert!(
            !parsed
                .layout
                .contains("55555555-5555-4555-8555-555555555555")
        );
        assert!(
            !parsed
                .layout
                .contains("66666666-6666-4666-8666-666666666666")
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_read_only() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{78,22222222-2222-4222-8222-222222222222},0,0,0,2,"Author",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ReadOnly>true</ReadOnly>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[14].clone()], "1");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_input_field_read_only() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ReadOnly>{value}</ReadOnly>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[input_fields[5].clone()], "2");
            assert_eq!(&parsed.layout[input_fields[6].clone()], r#""Author""#);
            assert_eq!(&parsed.layout[input_fields[14].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_data_path() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{78,22222222-2222-4222-8222-222222222222},0,0,0,2,"AuthorField",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<Attributes>
		<Attribute name="Author" id="1"></Attribute>
	</Attributes>
	<ChildItems>
		<InputField name="AuthorField" id="78">
			<DataPath>Author</DataPath>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[11].clone()], "{1,{1}}");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_input_field_data_path() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<Attributes>
		<Attribute name="Author" id="1"></Attribute>
	</Attributes>
	<ChildItems>
		<InputField name="AuthorField" id="78">
			<DataPath>Author</DataPath>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[5].clone()], "2");
        assert_eq!(&parsed.layout[input_fields[6].clone()], r#""AuthorField""#);
        assert_eq!(&parsed.layout[input_fields[11].clone()], "{1,{1}}");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_skip_on_input() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let base = super::deflate_raw(
                br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{78,22222222-2222-4222-8222-222222222222},0,0,0,2,"Author",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0}},"Old module",{0}}"#,
            )?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<SkipOnInput>{value}</SkipOnInput>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[input_fields[15].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_input_field_skip_on_input() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<SkipOnInput>{value}</SkipOnInput>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[input_fields[5].clone()], "2");
            assert_eq!(&parsed.layout[input_fields[6].clone()], r#""Author""#);
            assert_eq!(&parsed.layout[input_fields[15].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_input_field_extended_options() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<Width>22</Width>
			<HorizontalStretch>false</HorizontalStretch>
			<AutoMaxWidth>false</AutoMaxWidth>
			<MaxWidth>54</MaxWidth>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[5].clone()], "2");
        assert_eq!(&parsed.layout[input_fields[6].clone()], r#""Author""#);
        assert_eq!(&parsed.layout[options_fields[0].clone()], "38");
        assert_eq!(&parsed.layout[options_fields[2].clone()], "22");
        assert_eq!(&parsed.layout[options_fields[4].clone()], "0");
        assert_eq!(&parsed.layout[options_fields[49].clone()], "0");
        assert_eq!(&parsed.layout[options_fields[50].clone()], "54");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_title_location() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{78,22222222-2222-4222-8222-222222222222},0,0,0,2,"Author",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<TitleLocation>Left</TitleLocation>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[7].clone()], "2");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_title_location_right() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{78,22222222-2222-4222-8222-222222222222},0,0,0,2,"Author",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<TitleLocation>Right</TitleLocation>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[7].clone()], "4");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_edit_mode() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{78,22222222-2222-4222-8222-222222222222},0,0,0,2,"Author",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<EditMode>EnterOnInput</EditMode>
			<AutoEditMode>true</AutoEditMode>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[26].clone()], "2");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_edit_mode_directly() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{78,22222222-2222-4222-8222-222222222222},0,0,0,2,"Author",1,0,{1,0},{1,0},{0},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,3,0,3,1}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<EditMode>Directly</EditMode>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[26].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_auto_max_width_false() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 51];
        options[0] = "38".to_string();
        options[49] = "1".to_string();
        options[50] = "35".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<AutoMaxWidth>false</AutoMaxWidth>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[49].clone()], "0");
        assert_eq!(&parsed.layout[options_fields[50].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_width() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 53];
        options[0] = "38".to_string();
        options[2] = "0".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<Width>22</Width>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[2].clone()], "22");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_height() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 53];
        options[0] = "38".to_string();
        options[3] = "0".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<Height>2</Height>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[3].clone()], "2");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_text_document_field() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{20,22222222-2222-4222-8222-222222222222},0,0,0,7,"OldEditor",1,0,{1,0},{1,0},{1,{8}},{0},1,0,2,0,2,{1,0},{1,0},1,1,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0}},"Old module",{4,1,{9,{8},0,"ProcedureText",0,0,0,0,0,0,0}},{0},{0}}"#,
        )?;
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<TextDocumentField name="ProcedureEditor" id="20">
			<DataPath>ProcedureText</DataPath>
			<TitleLocation>None</TitleLocation>
			<Width>18</Width>
			<Height>5</Height>
		</TextDocumentField>
	</ChildItems>
	<Attributes>
		<Attribute name="ProcedureText" id="8"/>
	</Attributes>
</Form>"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let text_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[text_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[text_fields[5].clone()], "7");
        assert_eq!(
            &parsed.layout[text_fields[6].clone()],
            r#""ProcedureEditor""#
        );
        assert_eq!(&parsed.layout[text_fields[7].clone()], "0");
        assert_eq!(&parsed.layout[text_fields[11].clone()], "{1,{8}}");
        assert_eq!(&parsed.layout[text_fields[22].clone()], "18");
        assert_eq!(&parsed.layout[text_fields[23].clone()], "5");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_text_document_field_width() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<TextDocumentField name="ProcedureEditor" id="20">
			<Width>18</Width>
			<Height>5</Height>
		</TextDocumentField>
	</ChildItems>
</Form>"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let text_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[text_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[text_fields[5].clone()], "7");
        assert_eq!(
            &parsed.layout[text_fields[6].clone()],
            r#""ProcedureEditor""#
        );
        assert_eq!(&parsed.layout[text_fields[22].clone()], "18");
        assert_eq!(&parsed.layout[text_fields[23].clone()], "5");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_text_document_field_read_only() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let base = super::deflate_raw(
                br#"{4,{59,1,11111111-1111-4111-8111-111111111111,{48,{20,22222222-2222-4222-8222-222222222222},0,0,0,7,"ProcedureEditor",1,0,{1,0},{1,0},{1,{8}},{0},1,2,2,0,2,{1,0},{1,0},1,1,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0}},"Old module",{4,1,{9,{8},0,"ProcedureText",0,0,0,0,0,0,0}},{0},{0}}"#,
            )?;
            let xml = format!(
                r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<TextDocumentField name="ProcedureEditor" id="20">
			<ReadOnly>{value}</ReadOnly>
		</TextDocumentField>
	</ChildItems>
</Form>"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let text_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[text_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[text_fields[5].clone()], "7");
            assert_eq!(&parsed.layout[text_fields[14].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_max_width() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 53];
        options[0] = "38".to_string();
        options[49] = "1".to_string();
        options[50] = "0".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<AutoMaxWidth>false</AutoMaxWidth>
			<MaxWidth>54</MaxWidth>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[49].clone()], "0");
        assert_eq!(&parsed.layout[options_fields[50].clone()], "54");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_auto_max_height_false() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 53];
        options[0] = "38".to_string();
        options[52] = "1".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<AutoMaxHeight>false</AutoMaxHeight>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[52].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_max_height() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 54];
        options[0] = "38".to_string();
        options[52] = "1".to_string();
        options[53] = "0".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<AutoMaxHeight>false</AutoMaxHeight>
			<MaxHeight>28</MaxHeight>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[52].clone()], "0");
        assert_eq!(&parsed.layout[options_fields[53].clone()], "28");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_horizontal_stretch() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 51];
        options[0] = "38".to_string();
        options[4] = "2".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<HorizontalStretch>false</HorizontalStretch>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[4].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_vertical_stretch() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 51];
        options[0] = "38".to_string();
        options[5] = "2".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<VerticalStretch>true</VerticalStretch>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[5].clone()], "1");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_multi_line() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[8] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<MultiLine>{value}</MultiLine>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[8].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_wrap() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[6] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<Wrap>{value}</Wrap>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[6].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_text_edit() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[41] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<TextEdit>{value}</TextEdit>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[41].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_auto_cell_height() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 75];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{83,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[4] = "1".to_string();
            input_fields[5] = r#"{0,{0,{"B",0},0}}"#.to_string();
            input_fields[6] = "2".to_string();
            input_fields[7] = r#""Author""#.to_string();
            input_fields[29] = "0".to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            input_fields[40] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="83">
			<AutoCellHeight>{value}</AutoCellHeight>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[input_fields[29].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_child_item_show_in_header() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 75];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{83,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[4] = "1".to_string();
            input_fields[5] = r#"{0,{0,{"B",0},0}}"#.to_string();
            input_fields[6] = "2".to_string();
            input_fields[7] = r#""Author""#.to_string();
            input_fields[21] = "1".to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            input_fields[40] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="83">
			<ShowInHeader>{value}</ShowInHeader>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[input_fields[21].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_column_group_show_in_header() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let column_group = r#"{22,{140,22222222-2222-4222-8222-222222222222},0,0,0,2,"DebitAccountGroup",{1,0},{1,0},0,1,0,0,0,2,2,{4,4,{0},4},{8,2,60,{-31},700,0,0,0,1,100},{0,0,0},1,{5,1,1,0,3,{4,0,{0},"",-1,-1,1,0,""},{4,4,{0},4},{0},{"Pattern"},"",{1,0},0,1,2,2,2},0}"#;
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{column_group}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<ColumnGroup name="DebitAccountGroup" id="140">
			<Group>InCell</Group>
			<ShowInHeader>{value}</ShowInHeader>
		</ColumnGroup>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let option_fields = super::scan_braced_fields(&parsed.layout, group_fields[20].start)?;

            assert_eq!(&parsed.layout[group_fields[0].clone()], "22");
            assert_eq!(&parsed.layout[group_fields[5].clone()], "2");
            assert_eq!(&parsed.layout[option_fields[1].clone()], "2");
            assert_eq!(&parsed.layout[option_fields[3].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_password_mode() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[7] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<PasswordMode>{value}</PasswordMode>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[7].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_drop_list_button() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 40];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        let mut options = vec!["2".to_string(); 53];
        options[0] = "38".to_string();
        options[47] = "2".to_string();
        input_fields[39] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<DropListButton>false</DropListButton>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[options_fields[47].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_clear_button() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[13] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ClearButton>{value}</ClearButton>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[13].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_open_button() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[15] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<OpenButton>{value}</OpenButton>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[15].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_choice_button() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[12] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ChoiceButton>{value}</ChoiceButton>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[12].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_choice_list_button() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[11] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ChoiceListButton>{value}</ChoiceListButton>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[11].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_spin_button() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[14] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<SpinButton>{value}</SpinButton>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[14].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_list_choice_mode() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[19] = "0".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ListChoiceMode>{value}</ListChoiceMode>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[19].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_create_button() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[45] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<CreateButton>{value}</CreateButton>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[45].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_quick_choice() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[23] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<QuickChoice>{value}</QuickChoice>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[23].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_shifted_extended_options() -> anyhow::Result<()> {
        let mut input_fields = vec!["0".to_string(); 41];
        input_fields[0] = "48".to_string();
        input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
        input_fields[5] = "2".to_string();
        input_fields[6] = r#""Author""#.to_string();
        input_fields[39] = "1".to_string();
        let mut options = vec!["2".to_string(); 53];
        options[0] = "38".to_string();
        options[13] = "2".to_string();
        options[23] = "2".to_string();
        input_fields[40] = format!("{{{}}}", options.join(","));
        let input_field = format!("{{{}}}", input_fields.join(","));
        let base_text = format!(
            r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
        );
        let base = super::deflate_raw(base_text.as_bytes())?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ClearButton>true</ClearButton>
			<QuickChoice>false</QuickChoice>
		</InputField>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[40].start)?;

        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[39].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[13].clone()], "1");
        assert_eq!(&parsed.layout[options_fields[23].clone()], "0");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_choose_type() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[32] = "1".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ChooseType>{value}</ChooseType>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[32].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_auto_mark_incomplete() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[31] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<AutoMarkIncomplete>{value}</AutoMarkIncomplete>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[31].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_mark_required_complete() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[31] = "2".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<MarkRequiredComplete>{value}</MarkRequiredComplete>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[31].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_input_field_choice_button_representation() -> anyhow::Result<()>
    {
        for (value, expected_code) in [
            ("ShowInDropList", "1"),
            ("ShowInDropListAndInInputField", "2"),
            ("ShowInInputField", "3"),
        ] {
            let mut input_fields = vec!["0".to_string(); 40];
            input_fields[0] = "48".to_string();
            input_fields[1] = "{78,22222222-2222-4222-8222-222222222222}".to_string();
            input_fields[5] = "2".to_string();
            input_fields[6] = r#""Author""#.to_string();
            let mut options = vec!["2".to_string(); 53];
            options[0] = "38".to_string();
            options[46] = "0".to_string();
            input_fields[39] = format!("{{{}}}", options.join(","));
            let input_field = format!("{{{}}}", input_fields.join(","));
            let base_text = format!(
                r#"{{4,{{59,1,11111111-1111-4111-8111-111111111111,{input_field}}},"Old module",{{0}}}}"#
            );
            let base = super::deflate_raw(base_text.as_bytes())?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<InputField name="Author" id="78">
			<ChoiceButtonRepresentation>{value}</ChoiceButtonRepresentation>
		</InputField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let input_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
            let options_fields = super::scan_braced_fields(&parsed.layout, input_fields[39].start)?;

            assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[options_fields[46].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_nested_input_field() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<UsualGroup name="MainGroup" id="22">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Main</v8:content>
				</v8:item>
			</Title>
			<ChildItems>
				<InputField name="Description" id="40">
					<Title>
						<v8:item>
							<v8:lang>en</v8:lang>
							<v8:content>Description</v8:content>
						</v8:item>
					</Title>
					<Events>
						<Event name="OnChange">DescriptionOnChange</Event>
					</Events>
				</InputField>
			</ChildItems>
		</UsualGroup>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, group_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[group_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[group_fields[5].clone()], "5");
        assert_eq!(&parsed.layout[group_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[4].clone()], "0");
        assert_eq!(&parsed.layout[input_fields[5].clone()], "2");
        assert_eq!(&parsed.layout[input_fields[6].clone()], r#""Description""#);
        assert_eq!(
            &parsed.layout[input_fields[9].clone()],
            r#"{1,"en","Description"}"#
        );
        assert!(
            parsed
                .layout
                .contains(r#""OnChange","DescriptionOnChange""#)
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_table_with_input_field() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<Table name="Rows" id="25">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Rows</v8:content>
				</v8:item>
			</Title>
			<Events>
				<Event name="OnGetDataAtServer">RowsGetData</Event>
			</Events>
			<ChildItems>
				<InputField name="Description" id="40">
					<Events>
						<Event name="OnChange">DescriptionOnChange</Event>
					</Events>
				</InputField>
			</ChildItems>
		</Table>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, table_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[0].clone()], "73");
        assert_eq!(&parsed.layout[table_fields[3].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[5].clone()], r#""Rows""#);
        assert_eq!(
            &parsed.layout[table_fields[9].clone()],
            r#"{1,"en","Rows"}"#
        );
        assert_eq!(&parsed.layout[table_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[6].clone()], r#""Description""#);
        assert!(
            parsed
                .layout
                .contains(r#""OnGetDataAtServer","RowsGetData""#)
        );
        assert!(
            parsed
                .layout
                .contains(r#""OnChange","DescriptionOnChange""#)
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_table_data_path() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Attributes>
		<Attribute name="Rows" id="6"/>
	</Attributes>
	<ChildItems>
		<Table name="Rows" id="25">
			<DataPath>Rows</DataPath>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[0].clone()], "73");
        assert_eq!(&parsed.layout[table_fields[5].clone()], r#""Rows""#);
        assert_eq!(&parsed.layout[table_fields[11].clone()], "{1,{6}}");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_appends_input_field_to_existing_table() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{25,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},0}},"Old module",{0}}"#,
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<Table name="Rows" id="25">
			<ChildItems>
				<InputField name="Description" id="40">
					<Events>
						<Event name="OnChange">DescriptionOnChange</Event>
					</Events>
				</InputField>
			</ChildItems>
		</Table>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let input_fields = super::scan_braced_fields(&parsed.layout, table_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[0].clone()], "73");
        assert_eq!(
            &parsed.layout[table_fields[1].clone()],
            "{25,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb}"
        );
        assert_eq!(&parsed.layout[table_fields[10].clone()], "1");
        assert!(super::is_uuid_like_ascii(
            parsed.layout[table_fields[11].clone()].as_bytes()
        ));
        assert_eq!(&parsed.layout[input_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[input_fields[6].clone()], r#""Description""#);
        assert!(
            parsed
                .layout
                .contains(r#""OnChange","DescriptionOnChange""#)
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_table_layout_properties() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{25,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},0,{0},0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<Attributes>
		<Attribute name="Rows" id="6">
			<MainAttribute>true</MainAttribute>
		</Attribute>
	</Attributes>
	<ChildItems>
		<Table name="Rows" id="25">
			<Representation>List</Representation>
			<HeightInTableRows>6</HeightInTableRows>
			<RowSelectionMode>Cell</RowSelectionMode>
			<EnableStartDrag>true</EnableStartDrag>
			<EnableDrag>true</EnableDrag>
			<FileDragMode>AsFile</FileDragMode>
			<DataPath>Rows</DataPath>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[table_fields[8].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[11].clone()], "{1,{6}}");
        assert_eq!(&parsed.layout[table_fields[21].clone()], "6");
        assert_eq!(&parsed.layout[table_fields[24].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[26].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[29].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[30].clone()], "2");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_auto_refresh_properties() -> anyhow::Result<()>
    {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,2,5,{"B",1},6,{"N",30}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<AutoRefresh>false</AutoRefresh>
			<AutoRefreshPeriod>60</AutoRefreshPeriod>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#"5,{"B",0},6,{"N",60}"#));
        assert!(!parsed.layout.contains(r#"5,{"B",1},6,{"N",30}"#));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_period() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,7,{"#",2fdc88ec-7c9b-43cd-8ba5-873f043bdd88,{0,00010101000000,00010101000000}}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<Period>
				<v8:variant xsi:type="v8:StandardPeriodVariant">Custom</v8:variant>
				<v8:startDate>2026-01-01T00:00:00</v8:startDate>
				<v8:endDate>2026-01-31T23:59:59</v8:endDate>
			</Period>
		</Table>
	</ChildItems>
</Form>
"#;

        let properties = super::parse_form_xml_body_properties(xml)?;
        assert_eq!(
            properties.child_items[0].period,
            Some(super::FormXmlTablePeriod {
                variant: Some("Custom".to_string()),
                start_date: Some("20260101000000".to_string()),
                end_date: Some("20260131235959".to_string()),
            })
        );

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed.layout.contains(
                r##"7,{"#",2fdc88ec-7c9b-43cd-8ba5-873f043bdd88,{0,20260101000000,20260131235959}}"##
            ),
            "{}",
            parsed.layout
        );
        assert!(!parsed.layout.contains("00010101000000,00010101000000"));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_use_alternation_row_color() -> anyhow::Result<()>
    {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,3,5,{"B",1},6,{"N",30},9,{"B",0}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<UseAlternationRowColor>true</UseAlternationRowColor>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#"9,{"B",1}"#), "{}", parsed.layout);
        assert!(!parsed.layout.contains(r#"9,{"B",0}"#));
        assert!(parsed.layout.contains(r#"5,{"B",1},6,{"N",30}"#));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_default_item() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,11,{"B",0}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<DefaultItem>true</DefaultItem>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#"11,{"B",1}"#), "{}", parsed.layout);
        assert!(!parsed.layout.contains(r#"11,{"B",0}"#));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_head_properties() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,0,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,0}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<Representation>List</Representation>
			<CommandBarLocation>Top</CommandBarLocation>
		</Table>
	</ChildItems>
</Form>
"#;

        let properties = super::parse_form_xml_body_properties(xml)?;
        assert_eq!(
            properties.child_items[0].table_representation.as_deref(),
            Some("List")
        );
        assert_eq!(
            properties.child_items[0]
                .table_command_bar_location
                .as_deref(),
            Some("Top")
        );
        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[table_fields[8].clone()], "0");
        assert_eq!(&parsed.layout[table_fields[13].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[18].clone()], "1");
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_choice_folders_and_items() -> anyhow::Result<()>
    {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,8,{"#",59ef2b80-c86b-11d5-a3c1-0050bae0a776,0}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<ChoiceFoldersAndItems>Folders</ChoiceFoldersAndItems>
		</Table>
	</ChildItems>
</Form>
"#;

        let properties = super::parse_form_xml_body_properties(xml)?;
        assert_eq!(
            properties.child_items[0].choice_folders_and_items,
            Some(super::FormXmlUseForFoldersAndItems::Folders)
        );

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r##"8,{"#",59ef2b80-c86b-11d5-a3c1-0050bae0a776,1}"##),
            "{}",
            parsed.layout
        );
        assert!(
            !parsed
                .layout
                .contains(r##"8,{"#",59ef2b80-c86b-11d5-a3c1-0050bae0a776,0}"##)
        );
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_restore_current_row() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,12,{"B",1}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<RestoreCurrentRow>false</RestoreCurrentRow>
		</Table>
	</ChildItems>
</Form>
"#;

        let properties = super::parse_form_xml_body_properties(xml)?;
        assert_eq!(properties.child_items[0].restore_current_row, Some(false));

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#"12,{"B",0}"#), "{}", parsed.layout);
        assert!(!parsed.layout.contains(r#"12,{"B",1}"#));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_row_filter_nil() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,10,{"U"}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<RowFilter xsi:nil="true"/>
		</Table>
	</ChildItems>
</Form>
"#;

        let properties = super::parse_form_xml_body_properties(xml)?;
        assert_eq!(properties.child_items[0].row_filter_nil, Some(true));

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#"10,{"U"}"#), "{}", parsed.layout);
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_row_picture_data_path() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,19,{"S",""}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<RowPictureDataPath>Rows.DefaultPicture</RowPictureDataPath>
		</Table>
	</ChildItems>
</Form>
"#;

        let properties = super::parse_form_xml_body_properties(xml)?;
        assert_eq!(
            properties.child_items[0].row_picture_data_path.as_deref(),
            Some("Rows.DefaultPicture")
        );

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed.layout.contains(r#"19,{"S","Rows.DefaultPicture"}"#),
            "{}",
            parsed.layout
        );
        assert!(!parsed.layout.contains(r#"19,{"S",""}"#));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_update_on_data_change() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,14,{"#",eac7bfa0-10b4-4369-996c-d258871ad519,1}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<UpdateOnDataChange>Auto</UpdateOnDataChange>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains(r##"14,{"#",eac7bfa0-10b4-4369-996c-d258871ad519,0}"##),
            "{}",
            parsed.layout
        );
        assert!(
            !parsed
                .layout
                .contains(r##"14,{"#",eac7bfa0-10b4-4369-996c-d258871ad519,1}"##)
        );
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_allow_getting_current_row_url()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,20,{"B",1}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<AllowGettingCurrentRowURL>false</AllowGettingCurrentRowURL>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#"20,{"B",0}"#), "{}", parsed.layout);
        assert!(!parsed.layout.contains(r#"20,{"B",1}"#));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_user_settings_group() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,2,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{22,{288,cccccccc-cccc-4ccc-8ccc-cccccccccccc},0,0,0,5,"SettingsGroup",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{1,0},0,0,0,3,3,0},bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,{55,{1,dddddddd-dddd-4ddd-8ddd-dddddddddddd},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},0,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,1,16,{"N",100}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<UsualGroup name="SettingsGroup" id="288"/>
		<Table name="Rows" id="1">
			<UserSettingsGroup>SettingsGroup</UserSettingsGroup>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed.layout.contains(r#"16,{"N",288}"#),
            "{}",
            parsed.layout
        );
        assert!(!parsed.layout.contains(r#"16,{"N",100}"#));
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_wrapper55_table_skip_on_input() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{55,{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},{1,0},{0},1,1,0,0,1,0,0,0,0,0,0,0,1,0,1,1,0,1,2,2,1,1,0,0,1,0,2,0,0,1,1,{1,{10000000}},{4,0,{0},"",-1,-1,1,0,""},{3,4,{0}},{0,0,0},1,0,2,5,{"B",1},6,{"N",30}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="1">
			<SkipOnInput>false</SkipOnInput>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[table_fields[12].clone()], "0");
        assert_eq!(&parsed.layout[table_fields[13].clone()], "1");
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_removes_missing_nested_table_child_items() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{25,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,1,0,"Rows",0,0,0,{1,0},2,cccccccc-cccc-4ccc-8ccc-cccccccccccc,{48,{40,dddddddd-dddd-4ddd-8ddd-dddddddddddd},0,0,0,2,"KeepColumn",1,0,{0}},eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee,{48,{41,ffffffff-ffff-4fff-8fff-ffffffffffff},0,0,0,2,"DropColumn",1,0,{0}}}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<Table name="Rows" id="25">
			<ChildItems>
				<InputField name="KeepColumn" id="40"/>
			</ChildItems>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[10].clone()], "1");
        assert!(parsed.layout.contains("KeepColumn"));
        assert!(!parsed.layout.contains("DropColumn"));
        assert!(
            !parsed
                .layout
                .contains("eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee")
        );
        assert!(
            !parsed
                .layout
                .contains("ffffffff-ffff-4fff-8fff-ffffffffffff")
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_nested_label_field() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<UsualGroup name="MainGroup" id="22">
			<ChildItems>
				<LabelField name="DescriptionLabel" id="41">
					<Title>
						<v8:item>
							<v8:lang>en</v8:lang>
							<v8:content>Description</v8:content>
						</v8:item>
					</Title>
				</LabelField>
			</ChildItems>
		</UsualGroup>
	</ChildItems>
</Form>
"#
        .as_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let group_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let label_fields = super::scan_braced_fields(&parsed.layout, group_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[group_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[group_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[label_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[label_fields[4].clone()], "0");
        assert_eq!(&parsed.layout[label_fields[5].clone()], "1");
        assert_eq!(
            &parsed.layout[label_fields[6].clone()],
            r#""DescriptionLabel""#
        );
        assert_eq!(
            &parsed.layout[label_fields[9].clone()],
            r#"{1,"en","Description"}"#
        );

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_label_field_show_in_header() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<LabelField name="DescriptionLabel" id="41">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Description</v8:content>
				</v8:item>
			</Title>
			<ShowInHeader>{value}</ShowInHeader>
		</LabelField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let label_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
            assert_eq!(&parsed.layout[label_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[label_fields[5].clone()], "1");
            assert_eq!(
                &parsed.layout[label_fields[6].clone()],
                r#""DescriptionLabel""#
            );
            assert_eq!(
                &parsed.layout[label_fields[9].clone()],
                r#"{1,"en","Description"}"#
            );
            assert_eq!(&parsed.layout[label_fields[20].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_checkbox_field_show_in_header() -> anyhow::Result<()> {
        for (value, expected_code) in [("true", "1"), ("false", "0")] {
            let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
            let xml = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<CheckBoxField name="EnabledFlag" id="42">
			<Title>
				<v8:item>
					<v8:lang>en</v8:lang>
					<v8:content>Enabled</v8:content>
				</v8:item>
			</Title>
			<ShowInHeader>{value}</ShowInHeader>
		</CheckBoxField>
	</ChildItems>
</Form>
"#
            );

            let packed = super::pack_form_body_blob_from_form_xml(&base, xml.as_bytes(), None)?;
            let parsed = super::parse_form_body_blob(&packed.blob)?;
            let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
            let checkbox_fields =
                super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

            assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
            assert_eq!(&parsed.layout[checkbox_fields[0].clone()], "48");
            assert_eq!(&parsed.layout[checkbox_fields[5].clone()], "3");
            assert_eq!(
                &parsed.layout[checkbox_fields[6].clone()],
                r#""EnabledFlag""#
            );
            assert_eq!(
                &parsed.layout[checkbox_fields[10].clone()],
                r#"{1,"en","Enabled"}"#
            );
            assert_eq!(&parsed.layout[checkbox_fields[21].clone()], expected_code);
            assert_eq!(parsed.module_text, "Old module");
        }

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_table_with_search_addition() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<Table name="Rows" id="25">
			<SearchStringAddition name="SearchRows" id="134">
				<AdditionSource>
					<Item>Rows</Item>
					<Type>SearchStringRepresentation</Type>
				</AdditionSource>
			</SearchStringAddition>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let addition_fields = super::scan_braced_fields(&parsed.layout, table_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[0].clone()], "73");
        assert_eq!(&parsed.layout[table_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[addition_fields[0].clone()], "6");
        assert_eq!(&parsed.layout[addition_fields[5].clone()], "0");
        assert_eq!(
            &parsed.layout[addition_fields[6].clone()],
            r#""SearchRows""#
        );
        assert_eq!(&parsed.layout[addition_fields[19].clone()], "{25,0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_table_with_auto_command_bar() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<Table name="Rows" id="25">
			<AutoCommandBar name="RowsBar" id="27"/>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let bar_fields = super::scan_braced_fields(&parsed.layout, table_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[0].clone()], "73");
        assert_eq!(&parsed.layout[table_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[bar_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[bar_fields[5].clone()], "9");
        assert_eq!(&parsed.layout[bar_fields[6].clone()], r#""RowsBar""#);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_preserving_existing_empty_nested_auto_command_bar_row()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{22,{27,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},0,0,0,9,"RowsBar",{1,0},{1,0},0,1,0,0,0,2,2,{4,4,{0},4},{8,3,0,1,100},{0,0,0},1,{0,0,1},0,1,0,0,0,3,3,0}},"Old module",{0}}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<AutoCommandBar name="RowsBar" id="27"/>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let bar_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;

        assert_eq!(bar_fields.len(), 29, "{}", parsed.layout);
        assert_eq!(&parsed.layout[bar_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[bar_fields[5].clone()], "9");
        assert_eq!(&parsed.layout[bar_fields[20].clone()], "{0,0,1}");
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_search_addition_type() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,{6,{134,11111111-1111-4111-8111-111111111111},0,0,0,1,"OldStatus",{1,0}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<ViewStatusAddition name="NewStatus" id="134">
			<AdditionSource>
				<Type>ViewStatusRepresentation</Type>
			</AdditionSource>
		</ViewStatusAddition>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed.layout.contains(r#""NewStatus""#),
            "{}",
            parsed.layout
        );
        assert!(!parsed.layout.contains("OldStatus"));
        assert!(
            parsed
                .layout
                .contains(r#",{6,{134,11111111-1111-4111-8111-111111111111},0,0,0,1,"NewStatus""#)
                || parsed.layout.contains(
                    r#"{6,{134,11111111-1111-4111-8111-111111111111},0,0,0,1,"NewStatus""#
                ),
            "{}",
            parsed.layout
        );
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_search_addition_source_item() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,{73,{25,11111111-1111-4111-8111-111111111111},0,1,0,"Rows",0,0,0,{1,0},1,22222222-2222-4222-8222-222222222222,{6,{134,33333333-3333-4333-8333-333333333333},0,0,0,0,"OldSearch",{1,0},{1,0},1,1,0,1,{1,0},0,0,0,0,0,{26,0}}},{73,{26,44444444-4444-4444-8444-444444444444},0,1,0,"OldRows"}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<SearchStringAddition name="NewSearch" id="134">
			<AdditionSource>
				<Item>Rows</Item>
				<Type>SearchStringRepresentation</Type>
			</AdditionSource>
		</SearchStringAddition>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed.layout.contains(r#""NewSearch""#),
            "{}",
            parsed.layout
        );
        assert!(parsed.layout.contains("{25,0}"), "{}", parsed.layout);
        assert!(!parsed.layout.contains("{26,0}"));
        assert!(!parsed.layout.contains("OldSearch"));
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_direct_search_addition_removes_missing_table_children()
    -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{25,11111111-1111-4111-8111-111111111111},0,1,0,"Rows",0,0,0,{1,0},2,22222222-2222-4222-8222-222222222222,{6,{134,33333333-3333-4333-8333-333333333333},0,0,0,0,"OldSearch",{1,0},{1,0},1,1,0,1,{1,0},0,0,0,0,0,{25,0}},44444444-4444-4444-8444-444444444444,{48,{40,55555555-5555-4555-8555-555555555555},0,0,0,2,"OldColumn",1,0,{1,0}}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="25">
			<SearchStringAddition name="NewSearch" id="134">
				<AdditionSource>
					<Item>Rows</Item>
					<Type>SearchStringRepresentation</Type>
				</AdditionSource>
			</SearchStringAddition>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let addition_fields = super::scan_braced_fields(&parsed.layout, table_fields[12].start)?;

        assert_eq!(&parsed.layout[table_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[addition_fields[0].clone()], "6");
        assert_eq!(&parsed.layout[addition_fields[6].clone()], r#""NewSearch""#);
        assert!(!parsed.layout.contains("OldColumn"), "{}", parsed.layout);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_adds_direct_search_addition_to_existing_table() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{25,11111111-1111-4111-8111-111111111111},0,1,0,"Rows",0,0,0,{1,0},0}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Rows" id="25">
			<SearchStringAddition name="NewSearch" id="134">
				<AdditionSource>
					<Item>Rows</Item>
					<Type>SearchStringRepresentation</Type>
				</AdditionSource>
			</SearchStringAddition>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let addition_fields = super::scan_braced_fields(&parsed.layout, table_fields[12].start)?;

        assert_eq!(&parsed.layout[layout_fields[1].clone()], "1");
        assert_eq!(&parsed.layout[table_fields[10].clone()], "1");
        assert_eq!(&parsed.layout[addition_fields[0].clone()], "6");
        assert_eq!(&parsed.layout[addition_fields[6].clone()], r#""NewSearch""#);
        assert_eq!(&parsed.layout[addition_fields[19].clone()], "{25,0}");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_direct_search_addition_context_menu() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{25,11111111-1111-4111-8111-111111111111},0,1,0,"Rows",0,0,0,{1,0},1,22222222-2222-4222-8222-222222222222,{6,{134,33333333-3333-4333-8333-333333333333},0,0,0,0,"RowsSearch",{1,0},{1,0},1,1,0,1,{1,0},0,1,{22,{30,44444444-4444-4444-8444-444444444444},0,0,0,8,"OldSearchContext",{1,0},{1,0},0,1,0},0,0,{25,0}}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<ChildItems>
		<Table name="Rows" id="25">
			<SearchStringAddition name="RowsSearch" id="134">
				<AdditionSource>
					<Item>Rows</Item>
					<Type>SearchStringRepresentation</Type>
				</AdditionSource>
				<ContextMenu name="RowsSearchContext" id="30">
					<Title>
						<v8:item>
							<v8:lang>en</v8:lang>
							<v8:content>Search actions</v8:content>
						</v8:item>
					</Title>
				</ContextMenu>
			</SearchStringAddition>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let table_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let addition_fields = super::scan_braced_fields(&parsed.layout, table_fields[12].start)?;
        let context_fields = super::scan_braced_fields(&parsed.layout, addition_fields[16].start)?;

        assert_eq!(&parsed.layout[addition_fields[15].clone()], "1");
        assert_eq!(&parsed.layout[addition_fields[19].clone()], "{25,0}");
        assert_eq!(&parsed.layout[context_fields[0].clone()], "22");
        assert_eq!(&parsed.layout[context_fields[5].clone()], "8");
        assert_eq!(
            &parsed.layout[context_fields[6].clone()],
            r#""RowsSearchContext""#
        );
        assert_eq!(
            &parsed.layout[context_fields[7].clone()],
            r#"{1,"en","Search actions"}"#
        );
        assert!(!parsed.layout.contains("OldSearchContext"));
        assert_eq!(parsed.module_text, "Old module");

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_nested_child_does_not_patch_other_parent() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br##"{4,{59,2,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,{73,{25,11111111-1111-4111-8111-111111111111},0,1,0,"Left",0,0,0,{1,0},1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,{48,{40,22222222-2222-4222-8222-222222222222},0,0,0,2,"OldLeft",1,0,{1,0}}},cccccccc-cccc-4ccc-cccc-cccccccccccc,{73,{26,33333333-3333-4333-8333-333333333333},0,1,0,"Rows",0,0,0,{1,0},0}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Table name="Left" id="25"/>
		<Table name="Rows" id="26">
			<ChildItems>
				<InputField name="NewRows" id="40"/>
			</ChildItems>
		</Table>
	</ChildItems>
</Form>
"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains(r#""OldLeft""#), "{}", parsed.layout);
        assert!(parsed.layout.contains(r#""NewRows""#), "{}", parsed.layout);

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_uuid_based_events() -> anyhow::Result<()> {
        let unknown_event_uuid = "213d1900-dcad-4616-9f20-3f077156a40f";
        let base = super::deflate_raw(
            format!(r#"{{4,{{59,{{3,1d632984-de3c-4b4b-ad9f-d69682a10182,"СтарыйВыбор",3699f6a3-9a2a-4c82-a775-6ff4824a08ca,"СтароеОповещение",9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,"СтароеСоздание",{unknown_event_uuid},"СтарыйUuid",1,0,1d632984-de3c-4b4b-ad9f-d69682a10182,0,1,3699f6a3-9a2a-4c82-a775-6ff4824a08ca,0,1,9f2e5ddb-3492-4f5d-8f0d-416b8d1d5c5b,0,1}}}},"",{{0}}}}"#).as_bytes(),
        )?;
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Events>
		<Event name="ChoiceProcessing">НовыйВыбор</Event>
		<Event name="NotificationProcessing">НовоеОповещение</Event>
		<Event name="OnCreateAtServer">НовоеСоздание</Event>
		<Event name="{unknown_event_uuid}">НовыйUuid</Event>
	</Events>
</Form>
"#
        )
        .into_bytes();

        let packed = super::pack_form_body_blob_from_form_xml(&base, &xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.layout.contains("\"НовыйВыбор\""));
        assert!(parsed.layout.contains("\"НовоеОповещение\""));
        assert!(parsed.layout.contains("\"НовоеСоздание\""));
        assert!(parsed.layout.contains("\"НовыйUuid\""));
        assert!(!parsed.layout.contains("СтарыйВыбор"));
        assert!(!parsed.layout.contains("СтароеОповещение"));
        assert!(!parsed.layout.contains("СтароеСоздание"));
        assert!(!parsed.layout.contains("СтарыйUuid"));
        assert_eq!(parsed.module_text, "");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        Ok(())
    }

    #[test]
    fn packs_role_rights_xml_preserving_base_identifiers() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{10,{2,{{1,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,0,0},{1,1,499e8968-ca89-43f0-9955-8756058b1b53,-1,1,{499e8968-ca89-43f0-9955-8756058b1b53,{1,{1,\"WHERE OLD\"}}}}},{{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,0,0},{0,1c87578f-9e09-4ec0-a991-5629c87b1588,1,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,-1}}},{1,{\"OldTemplate\",\"WHERE OLD TEMPLATE\"}},4294967295,0,0,4294967295}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>true</setForNewObjects>
	<setForAttributesByDefault>false</setForAttributesByDefault>
	<independentRightsOfChildObjects>true</independentRightsOfChildObjects>
	<object>
		<name>InformationRegister.Prices</name>
		<right>
			<name>Get</name>
			<value>true</value>
			<restrictionByCondition><condition>WHERE TRUE</condition></restrictionByCondition>
		</right>
	</object>
	<object>
		<name>Catalog.Products</name>
		<right><name>View</name><value>true</value></right>
		<right><name>Read</name><value>false</value></right>
		<right><name>Update</name><value>true</value></right>
	</object>
	<restrictionTemplate>
		<name>NewTemplate</name>
		<condition>WHERE NEW TEMPLATE</condition>
	</restrictionTemplate>
</Rights>
"#;

        let packed = super::pack_role_rights_blob_from_xml(&base, xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"));
        assert!(text.contains("1c87578f-9e09-4ec0-a991-5629c87b1588,-1"));
        assert!(text.contains("aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1"));
        assert!(text.contains("499e8968-ca89-43f0-9955-8756058b1b53,1"));
        assert!(text.contains(r#""WHERE TRUE""#));
        assert!(text.contains(r#""NewTemplate","WHERE NEW TEMPLATE""#));
        assert!(!text.contains("WHERE OLD"));
        assert!(!text.contains("OldTemplate"));
        assert!(text.ends_with(",1,0,1,4294967295}"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_role_rights_xml_omitted_rights_as_false() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{10,{1,{{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,0,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1,b7bab52d-c1b1-4bd8-8276-02db08d42352,1}}},{0},0,1,0,4294967295}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>false</setForNewObjects>
	<setForAttributesByDefault>true</setForAttributesByDefault>
	<independentRightsOfChildObjects>false</independentRightsOfChildObjects>
	<object>
		<name>Catalog.Products</name>
		<right>
			<name>View</name>
			<value>true</value>
		</right>
	</object>
</Rights>
"#;

        let packed = super::pack_role_rights_blob_from_xml(&base, xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1"));
        assert!(text.contains("b7bab52d-c1b1-4bd8-8276-02db08d42352,-1"));
        assert!(!text.contains("b7bab52d-c1b1-4bd8-8276-02db08d42352,1"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_role_rights_xml_string_field_restrictions() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            br#"{10,{1,{{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,0,0},{1,1,24abfe06-289a-48c5-8bb4-032c733e45c5,1,1,{24abfe06-289a-48c5-8bb4-032c733e45c5,{2,{1,"",0},{1,"WHERE OLD",1,{{0},{1,"OldField"}}}}}}}},{0},0,1,0,4294967295}"#,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>false</setForNewObjects>
	<setForAttributesByDefault>true</setForAttributesByDefault>
	<independentRightsOfChildObjects>false</independentRightsOfChildObjects>
	<object>
		<name>InformationRegister.Prices</name>
		<right>
			<name>TotalsControl</name>
			<value>true</value>
			<restrictionByCondition>
				<field>DataVersion</field>
				<condition>WHERE NEW</condition>
			</restrictionByCondition>
		</right>
	</object>
</Rights>
"#;

        let packed = super::pack_role_rights_blob_from_xml(&base, xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(r#"{1,"WHERE NEW",1,{{0},{1,"DataVersion"}}}"#));
        assert!(!text.contains("WHERE OLD"));
        assert!(!text.contains("OldField"));

        Ok(())
    }

    #[test]
    fn packs_role_rights_xml_uses_serialized_object_order() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{10,{3,{{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,1,1},{0,b7bab52d-c1b1-4bd8-8276-02db08d42352,-1}},{{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,0,0},{0,1c87578f-9e09-4ec0-a991-5629c87b1588,-1}},{{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,1,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,-1}}},{0},0,1,0,4294967295}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>false</setForNewObjects>
	<setForAttributesByDefault>true</setForAttributesByDefault>
	<independentRightsOfChildObjects>false</independentRightsOfChildObjects>
	<object>
		<name>InformationRegister.Prices.StandardAttribute.Period</name>
		<right><name>Edit</name><value>true</value></right>
	</object>
	<object>
		<name>InformationRegister.Prices</name>
		<right><name>Read</name><value>true</value></right>
	</object>
	<object>
		<name>InformationRegister.Prices.StandardAttribute.Active</name>
		<right><name>View</name><value>true</value></right>
	</object>
</Rights>
"#;

        let packed = super::pack_role_rights_blob_from_xml(&base, xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(
            "{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,0,0},{0,1c87578f-9e09-4ec0-a991-5629c87b1588,1}"
        ));
        assert!(text.contains(
            "{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,1,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1}"
        ));
        assert!(text.contains(
            "{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,1,1},{0,b7bab52d-c1b1-4bd8-8276-02db08d42352,1}"
        ));

        Ok(())
    }

    #[test]
    fn audits_business_process_flowchart_as_base_dependent_with_precise_blockers()
    -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<GraphicalSchema xmlns="http://v8.1c.ru/8.3/xcf/gsrc" version="2.20">
	<Items>
		<Start id="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
			<Properties>
				<Name>Start</Name>
				<TabOrder>0</TabOrder>
				<Event name="BeforeStart">BeforeStartHandler</Event>
			</Properties>
		</Start>
		<Activity id="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
			<Properties>
				<Name>Review</Name>
				<TabOrder>1</TabOrder>
				<Explanation>Review the request</Explanation>
				<TaskDescription>Approve or reject</TaskDescription>
				<Event name="OnExecute">OnExecuteHandler</Event>
			</Properties>
		</Activity>
	</Items>
</GraphicalSchema>
"#;

        let blockers = super::business_process_flowchart_base_free_blockers(xml)?;

        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("Activity=1") && blocker.contains("Start=1"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("serialized item table order"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("geometry, connector, and nested field shapes"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("2 event handlers"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("1 explanations and 1 task descriptions"))
        );

        Ok(())
    }

    const PREDEFINED_TYPE: &str = "ae135932-4f94-44df-92c1-c91f15a92848";
    const PREDEFINED_FOLDER: &str = "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb";
    const PREDEFINED_ITEM: &str = "cccccccc-cccc-4ccc-cccc-cccccccccccc";

    /// A stored predefined table of a hierarchical catalog: the root row
    /// "Элементы" (the nil uuid), a folder and an item below it.
    fn predefined_base_plain() -> String {
        let (type_uuid, folder_uuid, item_uuid) =
            (PREDEFINED_TYPE, PREDEFINED_FOLDER, PREDEFINED_ITEM);
        format!(
            "{{0,{{1,{{7}},{{2,{{1,1,{{2,0,5,{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"B\",1}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Элементы\"}},{{\"S\",\"\"}},1,{{1,1,{{2,1,7,{{\"#\",{type_uuid},{{1,{folder_uuid}}}}},{{\"B\",1}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Folder\"}},{{\"S\",\"F\"}},{{\"S\",\"Folder description\"}},{{\"N\",0}},1,{{1,1,{{2,2,7,{{\"#\",{type_uuid},{{1,{item_uuid}}}}},{{\"B\",0}},{{\"#\",{type_uuid},{{1,00000000-0000-0000-0000-000000000000}}}},{{\"S\",\"Item\"}},{{\"S\",\"I\"}},{{\"S\",\"Item description\"}},{{\"N\",0}},0}}}}}}}}}}}}}},-1,3}}}}"
        )
    }

    #[test]
    fn packs_predefined_data_xml_preserving_base_shape() -> anyhow::Result<()> {
        let (folder_uuid, item_uuid) = (PREDEFINED_FOLDER, PREDEFINED_ITEM);
        let base_plain = predefined_base_plain();
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

        assert!(text.contains(r#"{"S","NewFolder"}"#));
        assert!(text.contains(r#"{"S","NF"}"#));
        assert!(text.contains(r#"{"S","New folder description"}"#));
        assert!(text.contains(r#"{"S","NewItem"}"#));
        assert!(text.contains(r#"{"S","NI"}"#));
        assert!(text.contains(r#"{"S","New item description"}"#));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn a_different_set_of_predefined_items_is_reported_not_patched() -> anyhow::Result<()> {
        let (folder_uuid, item_uuid) = (PREDEFINED_FOLDER, PREDEFINED_ITEM);
        // A uuid the stored table lacks (built, not written out: the policy
        // guard keeps the literals of this file to a known set).
        let new_uuid = format!("{}-dddd-4ddd-dddd-{}", "dddddddd", "dddddddddddd");
        let new_uuid = new_uuid.as_str();
        // The stored table opens with a root row of the nil uuid, which no item
        // of the XML stands for.
        let base_plain = predefined_base_plain();
        let base = super::deflate_raw(base_plain.as_bytes())?;
        let xml = |items: &str| {
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<PredefinedData xmlns="http://v8.1c.ru/8.3/xcf/predef" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="CatalogPredefinedItems" version="2.20">{items}</PredefinedData>
"#
            )
        };
        let item = |id: &str, name: &str, folder: bool, children: &str| {
            format!(
                "<Item id=\"{id}\"><Name>{name}</Name><Code>C</Code><Description>D</Description><IsFolder>{folder}</IsFolder>{children}</Item>"
            )
        };
        let leaf = item(item_uuid, "Item", false, "");
        let folder_with = |children: &str| {
            item(
                folder_uuid,
                "Folder",
                true,
                &format!("<ChildItems>{children}</ChildItems>"),
            )
        };

        // The same items: patched, the nil root row does not count as removed.
        let same = xml(&folder_with(&leaf));
        assert!(matches!(
            super::patch_predefined_data_blob_from_xml(&base, same.as_bytes())?,
            super::PredefinedPatch::Patched(_)
        ));
        // An item the row lacks.
        let added = xml(&folder_with(&format!(
            "{leaf}{}",
            item(new_uuid, "New", false, "")
        )));
        match super::patch_predefined_data_blob_from_xml(&base, added.as_bytes())? {
            super::PredefinedPatch::ItemsDiffer { added, removed } => {
                assert_eq!(added, [new_uuid]);
                assert!(removed.is_empty());
            }
            super::PredefinedPatch::Patched(_) => panic!("an added item was patched"),
        }
        // An item the tree no longer has.
        let removed = xml(&folder_with(""));
        match super::patch_predefined_data_blob_from_xml(&base, removed.as_bytes())? {
            super::PredefinedPatch::ItemsDiffer { added, removed } => {
                assert!(added.is_empty());
                assert_eq!(removed, [item_uuid]);
            }
            super::PredefinedPatch::Patched(_) => panic!("a removed item was patched"),
        }
        // The older entry point still refuses only an added item.
        assert!(super::pack_predefined_data_blob_from_xml(&base, added.as_bytes()).is_err());
        assert!(super::pack_predefined_data_blob_from_xml(&base, removed.as_bytes()).is_ok());
        Ok(())
    }

    #[test]
    fn audits_predefined_data_as_base_dependent_with_precise_blockers() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<PredefinedData xmlns="http://v8.1c.ru/8.3/xcf/predef" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:type="CatalogPredefinedItems" version="2.20">
	<Item id="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
		<Name>Folder</Name>
		<Code>F</Code>
		<Description>Folder description</Description>
		<IsFolder>true</IsFolder>
		<ChildItems>
			<Item id="cccccccc-cccc-4ccc-cccc-cccccccccccc">
				<Name>Item</Name>
				<Code>I</Code>
				<Description>Item description</Description>
				<IsFolder>false</IsFolder>
			</Item>
		</ChildItems>
	</Item>
</PredefinedData>
"#;

        let blockers = super::predefined_data_base_free_blockers(xml)?;

        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("2 predefined items"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("1 root row"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("1 folder") && blocker.contains("1 nested"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("8 editable value fields"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("patches existing base rows by item UUID"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("type UUID slots"))
        );

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
            "\u{feff}{5,1,\"ru\",\r\n{#base64:PGh0bWw+PC9odG1sPg==},1,\"shot.png\",1,\r\n{#base64:iVBORw0KGgo=}\r\n}"
        );
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn recognizes_raw_deflated_help_blob_shape() -> anyhow::Result<()> {
        let help = super::pack_help_blob_from_parts(&[("ru".to_string(), b"body".to_vec())], &[])?;
        let module_like = super::deflate_raw(b"{4,{59,0},\"module\",{0}}")?;

        assert!(super::raw_deflated_looks_like_help_blob(&help.blob));
        assert!(!super::raw_deflated_looks_like_help_blob(&module_like));
        Ok(())
    }

    #[test]
    fn hashes_help_content_without_base64_format_noise() -> anyhow::Result<()> {
        let packed =
            super::pack_help_blob_from_parts(&[("ru".to_string(), b"body".to_vec())], &[])?;
        let legacy = super::deflate_raw(b"{5,1,\"ru\",\r\n{#base64:Ym9k\r\neQ==}}")?;

        assert_eq!(
            super::raw_deflated_help_content_sha256(&packed.blob)?,
            super::raw_deflated_help_content_sha256(&legacy)?
        );
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
    fn packs_ext_picture_blob_preserving_base_wrapper() -> anyhow::Result<()> {
        let base = super::deflate_raw(b"{1,{7,8,9,10},{{#base64:T0xE}},\"tail\"}")?;
        let packed = super::pack_ext_picture_blob_from_bytes_with_base(Some(&base), b"PK\x03\x04")?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(text, "{1,{7,8,9,10},{{#base64:UEsDBA==}},\"tail\"}");
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
    fn packs_common_command_load_report_settings_picture_reference() {
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
        <xr:Ref>StdPicture.LoadReportSettings</xr:Ref>
        <xr:LoadTransparent>true</xr:LoadTransparent>
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
        assert!(inflated.contains(super::STD_PICTURE_LOAD_REPORT_SETTINGS_UUID));
        assert!(
            inflated.contains("{4,1,{0,283ecabd-aaed-41d1-ad46-6cca91c29120},\"\",-1,-1,1,0,\"\"}")
        );
        assert!(!inflated.contains("StdPicture.LoadReportSettings"));
    }

    #[test]
    fn resolves_common_command_standard_picture_uuids() {
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.Task"),
            Some("37cf7cc0-abad-4385-b597-6fd2d8dc085a")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.EventLog"),
            Some("723765ab-0b92-4745-a621-1ba0f77c92c9")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.CloneObject"),
            Some("f6532868-30b9-44ab-803c-78f0f0b06b02")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.Find"),
            Some("ffab30f1-da11-44b5-b34c-24da22badcf4")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.MarkToDelete"),
            Some("18492a87-2fe4-44af-b218-304897fed020")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.Post"),
            Some("20ebc47b-f4d9-439c-acd3-fdc624fbac2a")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.Reread"),
            Some("8f29e0e2-d5e6-41e8-a34d-9a0288156322")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.Report"),
            Some("db817ee1-fd28-4e7f-bb4a-53686b2b153c")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.ScheduledJob"),
            Some("1970a480-9b38-405e-9d9e-8209f3fad5f1")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.SetTime"),
            Some("55ef0776-5ee4-4daf-9a9b-70d63643ab8d")
        );
        assert_eq!(
            super::common_command_standard_picture_uuid("StdPicture.UndoPosting"),
            Some("8ca4ea33-603d-4992-8a41-c7924b5bd40b")
        );
        assert_eq!(
            super::common_command_standard_picture_code("StdPicture.InputFieldOpen"),
            Some(-7)
        );
        assert_eq!(
            super::common_command_standard_picture_code("StdPicture.MoveRight"),
            Some(-9)
        );
        assert_eq!(
            super::format_common_command_picture(&super::CommonCommandPicture::StdPictureCode {
                code: -7,
                load_transparent: true,
                pixel: None,
            }),
            r#"{4,1,{-7},"",-1,-1,1,0,""}"#
        );
        assert_eq!(
            super::format_common_command_picture(&super::CommonCommandPicture::StdPictureCode {
                code: -9,
                load_transparent: true,
                pixel: None,
            }),
            r#"{4,1,{-9},"",-1,-1,1,0,""}"#
        );
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
    fn an_ext_picture_header_follows_the_declared_transparency() {
        let xml = |body: &str| {
            format!(
                "<ExtPicture><Picture><xr:Abs>Picture.svg</xr:Abs>{body}</Picture></ExtPicture>"
            )
            .into_bytes()
        };
        let header = |body: &str| super::ext_picture_header_from_xml(&xml(body));
        assert_eq!(header("").unwrap(), "{0,0,-1,-1}");
        assert_eq!(
            header("<xr:LoadTransparent>false</xr:LoadTransparent>").unwrap(),
            "{0,0,-1,-1}"
        );
        // Six pictures of the БСП 8.3.27 extensions store it so.
        assert_eq!(
            header("<xr:LoadTransparent>true</xr:LoadTransparent>").unwrap(),
            "{1,0,-1,-1}"
        );
        assert_eq!(
            header(
                "<xr:LoadTransparent>true</xr:LoadTransparent><xr:TransparentPixel x=\"3\" y=\"4\"/>"
            )
            .unwrap(),
            "{1,0,3,4}"
        );
        assert!(
            header(
                "<xr:LoadTransparent>false</xr:LoadTransparent><xr:TransparentPixel x=\"3\" y=\"4\"/>"
            )
            .is_err()
        );
    }
}
