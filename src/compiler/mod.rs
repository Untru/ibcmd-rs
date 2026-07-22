//! Pure source-to-storage-patch compilation.
//!
//! Callers supply already loaded source bytes and explicit version/profile
//! axes. Native-storage adapters remain responsible for loading and writing
//! data around this boundary. Bytes derived from a base artifact deliberately
//! stay outside this base-free compiler contract.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::StorageProfileId;
use ibcmd_core::family::FamilyId;
use ibcmd_core::storage::{
    MAX_STORAGE_PAYLOAD_BYTES, MultipartIdentity, StorageBuildError, StorageKey,
    StoragePatchBuildError, StoragePatchEntry, StoragePatchOutcome, StoragePatchTarget,
    StorageProvenance,
};
use ibcmd_core::version::{CompatibilityMode, ContainerRevision, PlatformBuild, XmlDialect};

use crate::module_blob::{
    command_interface_base_free_blockers, command_interface_xml_can_pack_without_base,
    common_module_metadata_base_free_blockers, metadata_xml_base_free_blockers,
    pack_command_interface_blob_from_xml_base_free, pack_module_blob_bytes_base_free,
    pack_module_blob_container_bytes, pack_raw_deflated_blob_from_bytes,
};

pub mod bodies;
pub mod families;
pub mod graph;
pub mod identity;
pub mod overlay;
pub mod readiness;
pub mod root;
pub mod version;
pub mod versions;

pub use overlay::compile_overlay;

const SUPPORTED_STORAGE_PROFILE: &str = "storage:mssql-config-configsave";

/// Independent coordinates used by one compiler invocation.
///
/// No coordinate is inferred from another. The legacy selector accepts only
/// its exact verified coordinates and fails closed for every future value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileAxes {
    xml_dialect: XmlDialect,
    platform_build: Option<PlatformBuild>,
    compatibility_mode: Option<CompatibilityMode>,
    storage_profile: StorageProfileId,
    container_revision: Option<ContainerRevision>,
}

impl CompileAxes {
    /// Creates an exact compiler coordinate set without deriving missing axes.
    pub fn new(
        xml_dialect: XmlDialect,
        platform_build: Option<PlatformBuild>,
        compatibility_mode: Option<CompatibilityMode>,
        storage_profile: StorageProfileId,
        container_revision: Option<ContainerRevision>,
    ) -> Self {
        Self {
            xml_dialect,
            platform_build,
            compatibility_mode,
            storage_profile,
            container_revision,
        }
    }

    /// Returns the exact source XML dialect.
    pub const fn xml_dialect(&self) -> &XmlDialect {
        &self.xml_dialect
    }

    /// Returns the independently supplied platform build, when known.
    pub const fn platform_build(&self) -> Option<&PlatformBuild> {
        self.platform_build.as_ref()
    }

    /// Returns the independently supplied compatibility mode, when known.
    pub const fn compatibility_mode(&self) -> Option<&CompatibilityMode> {
        self.compatibility_mode.as_ref()
    }

    /// Returns the exact target storage profile.
    pub const fn storage_profile(&self) -> &StorageProfileId {
        &self.storage_profile
    }

    /// Returns the independently supplied physical-container revision.
    pub const fn container_revision(&self) -> Option<&ContainerRevision> {
        self.container_revision.as_ref()
    }
}

/// Whether the adapter has mapped an AdditionalIndexes body to an exact target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdditionalIndexesMapping {
    /// The exact native target suffix is known and already present in the target.
    Confirmed,
    /// No exact native target suffix is known for this source family.
    Unmapped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PrepackedDisposition<'a> {
    BaseFree(&'a [u8]),
    NeedsBase {
        required: StorageKey,
        reason: &'a str,
    },
}

/// Adapter-prepared input crossing the transitional pure compiler seam.
///
/// A base-free payload carries bytes. A base-dependent family carries only its
/// dependency and reason, never bytes derived from the base. Family and
/// provenance are mandatory in both cases.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepackedSource<'a> {
    family: FamilyId,
    provenance: StorageProvenance,
    disposition: PrepackedDisposition<'a>,
}

impl<'a> PrepackedSource<'a> {
    /// Creates a named, attributable payload known to be base-free.
    pub const fn base_free(
        family: FamilyId,
        provenance: StorageProvenance,
        bytes: &'a [u8],
    ) -> Self {
        Self {
            family,
            provenance,
            disposition: PrepackedDisposition::BaseFree(bytes),
        }
    }

    /// Classifies an adapter-prepared family that still requires a base entry.
    pub const fn needs_base(
        family: FamilyId,
        provenance: StorageProvenance,
        required: StorageKey,
        reason: &'a str,
    ) -> Self {
        Self {
            family,
            provenance,
            disposition: PrepackedDisposition::NeedsBase { required, reason },
        }
    }

    /// Returns the explicit source family.
    pub const fn family(&self) -> &FamilyId {
        &self.family
    }

    /// Returns the exact preparation provenance.
    pub const fn provenance(&self) -> &StorageProvenance {
        &self.provenance
    }

    /// Returns native bytes only when they were prepared without a base.
    pub const fn base_free_bytes(&self) -> Option<&'a [u8]> {
        match &self.disposition {
            PrepackedDisposition::BaseFree(bytes) => Some(*bytes),
            PrepackedDisposition::NeedsBase { .. } => None,
        }
    }
}

/// Byte-only source input for one target entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourcePayload<'a> {
    /// BSL module text with optional explicitly source-supplied info bytes.
    ModuleText {
        text: &'a [u8],
        info: Option<&'a [u8]>,
    },
    /// An exported V8 module container that only needs outer packing.
    ModuleContainer { container: &'a [u8] },
    /// Source bytes stored as a raw-deflate native body.
    RawDeflated { bytes: &'a [u8] },
    /// Editable metadata XML, which currently requires a native base entry.
    MetadataXml { xml: &'a [u8] },
    /// Common-module metadata XML, which currently requires a native base entry.
    CommonModuleMetadataXml { xml: &'a [u8] },
    /// CommandInterface XML, base-free only when all command refs are raw.
    CommandInterfaceXml { xml: &'a [u8] },
    /// AdditionalIndexes source bytes with an explicit target-mapping decision.
    AdditionalIndexes {
        bytes: &'a [u8],
        mapping: AdditionalIndexesMapping,
    },
    /// Explicit base-free bytes or a base dependency prepared by an adapter.
    Prepacked(PrepackedSource<'a>),
    /// A source family known to require one exact base entry.
    NeedsBase {
        required: StorageKey,
        reason: &'a str,
    },
    /// A source family intentionally unsupported by the current compiler.
    Unsupported { reason: &'a str },
    /// An open family for which no compiler route is registered.
    Unknown { family: FamilyId },
}

/// One pure source compilation request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileRequest<'a> {
    target: StoragePatchTarget,
    source: SourcePayload<'a>,
}

impl<'a> CompileRequest<'a> {
    /// Pairs a validated target with already loaded source data.
    pub const fn new(target: StoragePatchTarget, source: SourcePayload<'a>) -> Self {
        Self { target, source }
    }

    /// Builds a request whose target provenance comes from the prepacked source.
    pub fn prepacked(
        key: StorageKey,
        multipart: MultipartIdentity,
        source: PrepackedSource<'a>,
    ) -> Self {
        let target = StoragePatchTarget::new(key, multipart, source.provenance().clone());
        Self::new(target, SourcePayload::Prepacked(source))
    }

    /// Returns the exact patch target.
    pub const fn target(&self) -> &StoragePatchTarget {
        &self.target
    }

    /// Returns the byte-only source payload.
    pub const fn source(&self) -> &SourcePayload<'a> {
        &self.source
    }

    fn into_parts(self) -> (StoragePatchTarget, SourcePayload<'a>) {
        (self.target, self.source)
    }
}

/// Failure to classify or compile one pure source request.
#[derive(Debug)]
pub enum CompileError {
    /// A byte-level source parser or packer rejected malformed input.
    Source {
        family: String,
        source: anyhow::Error,
    },
    /// The neutral storage-patch contract rejected an outcome or aggregate.
    Patch(StoragePatchBuildError),
    /// A prepacked payload was attributed differently from its patch target.
    PrepackedProvenanceMismatch {
        family: String,
        target: String,
        source: String,
    },
}

impl CompileError {
    fn invalid_source(family: impl Into<String>, source: anyhow::Error) -> Self {
        Self::Source {
            family: family.into(),
            source,
        }
    }
}

impl Display for CompileError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source { family, source } => {
                write!(formatter, "failed to compile {family} source: {source}")
            }
            Self::Patch(source) => write!(formatter, "failed to build storage patch: {source}"),
            Self::PrepackedProvenanceMismatch {
                family,
                target,
                source,
            } => write!(
                formatter,
                "prepacked {family} provenance `{source}` does not match target provenance `{target}`"
            ),
        }
    }
}

impl Error for CompileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Source { source, .. } => Some(source.as_ref()),
            Self::Patch(source) => Some(source),
            Self::PrepackedProvenanceMismatch { .. } => None,
        }
    }
}

impl From<StoragePatchBuildError> for CompileError {
    fn from(source: StoragePatchBuildError) -> Self {
        Self::Patch(source)
    }
}

/// Result returned by the pure source compiler.
pub type CompileResult<T> = std::result::Result<T, CompileError>;

/// Compiles or classifies one byte-only source request.
///
/// Malformed XML or packer input is returned as [`CompileError::Source`]. Only
/// valid inputs whose implementation genuinely needs a base or is unavailable
/// become non-compiled [`StoragePatchOutcome`] values.
pub fn compile_source(
    axes: &CompileAxes,
    request: CompileRequest<'_>,
) -> CompileResult<StoragePatchEntry> {
    compile_source_with_payload_limit(axes, request, MAX_STORAGE_PAYLOAD_BYTES)
}

fn compile_source_with_payload_limit(
    axes: &CompileAxes,
    request: CompileRequest<'_>,
    maximum_payload_bytes: usize,
) -> CompileResult<StoragePatchEntry> {
    let (target, source) = request.into_parts();
    if let Some(reason) = unsupported_axes_reason(axes) {
        return unsupported_entry(target, &reason);
    }

    match source {
        SourcePayload::ModuleText { text, info } => {
            let source_bytes = text
                .len()
                .checked_add(info.map_or(0, <[u8]>::len))
                .ok_or_else(|| {
                    CompileError::invalid_source(
                        "module-text",
                        anyhow::anyhow!("combined module text and info size overflow"),
                    )
                })?;
            ensure_source_input_within_limit("module-text", source_bytes, maximum_payload_bytes)?;
            let packed = pack_module_blob_bytes_base_free(text, info)
                .map_err(|source| CompileError::invalid_source("module-text", source))?;
            compiled_entry(target, packed.blob)
        }
        SourcePayload::ModuleContainer { container } => {
            ensure_source_input_within_limit(
                "module-container",
                container.len(),
                maximum_payload_bytes,
            )?;
            let packed = pack_module_blob_container_bytes(container)
                .map_err(|source| CompileError::invalid_source("module-container", source))?;
            compiled_entry(target, packed.blob)
        }
        SourcePayload::RawDeflated { bytes } => {
            ensure_source_input_within_limit("raw-deflated", bytes.len(), maximum_payload_bytes)?;
            let packed = pack_raw_deflated_blob_from_bytes(bytes)
                .map_err(|source| CompileError::invalid_source("raw-deflated", source))?;
            compiled_entry(target, packed.blob)
        }
        SourcePayload::MetadataXml { xml } => {
            ensure_source_input_within_limit("metadata", xml.len(), maximum_payload_bytes)?;
            let blockers = metadata_xml_base_free_blockers(xml)
                .map_err(|source| CompileError::invalid_source("metadata", source))?;
            needs_target_base(target, "metadata XML requires a base entry", blockers)
        }
        SourcePayload::CommonModuleMetadataXml { xml } => {
            ensure_source_input_within_limit(
                "common-module-metadata",
                xml.len(),
                maximum_payload_bytes,
            )?;
            let blockers = common_module_metadata_base_free_blockers(xml)
                .map_err(|source| CompileError::invalid_source("common-module-metadata", source))?;
            needs_target_base(
                target,
                "CommonModule metadata XML requires a base entry",
                blockers,
            )
        }
        SourcePayload::CommandInterfaceXml { xml } => {
            ensure_source_input_within_limit(
                "command-interface",
                xml.len(),
                maximum_payload_bytes,
            )?;
            if command_interface_xml_can_pack_without_base(xml)
                .map_err(|source| CompileError::invalid_source("command-interface", source))?
            {
                let packed = pack_command_interface_blob_from_xml_base_free(xml)
                    .map_err(|source| CompileError::invalid_source("command-interface", source))?;
                compiled_entry(target, packed.blob)
            } else {
                let blockers = command_interface_base_free_blockers(xml)
                    .map_err(|source| CompileError::invalid_source("command-interface", source))?;
                needs_target_base(
                    target,
                    "CommandInterface XML requires a base entry",
                    blockers,
                )
            }
        }
        SourcePayload::AdditionalIndexes {
            bytes,
            mapping: AdditionalIndexesMapping::Confirmed,
        } => {
            ensure_source_input_within_limit(
                "additional-indexes",
                bytes.len(),
                maximum_payload_bytes,
            )?;
            let packed = pack_raw_deflated_blob_from_bytes(bytes)
                .map_err(|source| CompileError::invalid_source("additional-indexes", source))?;
            compiled_entry(target, packed.blob)
        }
        SourcePayload::AdditionalIndexes {
            mapping: AdditionalIndexesMapping::Unmapped,
            ..
        } => unsupported_entry(
            target,
            "AdditionalIndexes target suffix is not mapped for this source family",
        ),
        SourcePayload::Prepacked(source) => {
            if target.provenance() != source.provenance() {
                return Err(CompileError::PrepackedProvenanceMismatch {
                    family: source.family().to_string(),
                    target: target.provenance().to_string(),
                    source: source.provenance().to_string(),
                });
            }
            match source.disposition {
                PrepackedDisposition::BaseFree(bytes) => {
                    if bytes.len() > maximum_payload_bytes {
                        return Err(CompileError::Patch(
                            StorageBuildError::PayloadTooLarge {
                                maximum: maximum_payload_bytes,
                                actual: bytes.len(),
                            }
                            .into(),
                        ));
                    }
                    compiled_entry(target, bytes.to_vec())
                }
                PrepackedDisposition::NeedsBase { required, reason } => {
                    let outcome = StoragePatchOutcome::needs_base(required, reason)?;
                    Ok(StoragePatchEntry::new(target, outcome))
                }
            }
        }
        SourcePayload::NeedsBase { required, reason } => {
            let outcome = StoragePatchOutcome::needs_base(required, reason)?;
            Ok(StoragePatchEntry::new(target, outcome))
        }
        SourcePayload::Unsupported { reason } => unsupported_entry(target, reason),
        SourcePayload::Unknown { family } => unsupported_entry(
            target,
            &format!(
                "source family `{}` is not supported by the legacy storage compiler",
                family.as_str()
            ),
        ),
    }
}

pub(crate) fn unsupported_axes_reason(axes: &CompileAxes) -> Option<String> {
    let dialect = axes.xml_dialect().as_version().components();
    if dialect != [2, 20] && dialect != [2, 21] {
        return Some(format!(
            "legacy source compiler does not support XML dialect {}",
            axes.xml_dialect()
        ));
    }
    if axes.storage_profile().as_str() != SUPPORTED_STORAGE_PROFILE {
        return Some(format!(
            "legacy source compiler does not support storage profile {}",
            axes.storage_profile()
        ));
    }
    if let Some(platform) = axes.platform_build() {
        return Some(format!(
            "legacy source compiler has no verified platform-build selector for {platform}"
        ));
    }
    if let Some(compatibility) = axes.compatibility_mode() {
        return Some(format!(
            "legacy source compiler has no verified compatibility-mode selector for {compatibility}"
        ));
    }
    if let Some(revision) = axes.container_revision() {
        return Some(format!(
            "legacy source compiler has no verified container-revision selector for {revision}"
        ));
    }
    None
}

fn ensure_source_input_within_limit(
    family: &str,
    actual: usize,
    maximum: usize,
) -> CompileResult<()> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(CompileError::invalid_source(
            family,
            anyhow::anyhow!(
                "source input is {actual} bytes, exceeding the {maximum}-byte compiler bound"
            ),
        ))
    }
}

/// Accepts adapter-prepared input through the same classified patch contract.
pub fn compile_prepacked(
    axes: &CompileAxes,
    key: StorageKey,
    multipart: MultipartIdentity,
    source: PrepackedSource<'_>,
) -> CompileResult<StoragePatchEntry> {
    compile_source(axes, CompileRequest::prepacked(key, multipart, source))
}

fn compiled_entry(target: StoragePatchTarget, bytes: Vec<u8>) -> CompileResult<StoragePatchEntry> {
    let outcome = StoragePatchOutcome::compiled(bytes)?;
    Ok(StoragePatchEntry::new(target, outcome))
}

fn needs_target_base(
    target: StoragePatchTarget,
    summary: &str,
    blockers: Vec<String>,
) -> CompileResult<StoragePatchEntry> {
    let required = target.key().clone();
    let reason = if blockers.is_empty() {
        summary.to_owned()
    } else {
        format!("{summary}: {}", blockers.join("; "))
    };
    let outcome = StoragePatchOutcome::needs_base(required, &reason)?;
    Ok(StoragePatchEntry::new(target, outcome))
}

fn unsupported_entry(target: StoragePatchTarget, reason: &str) -> CompileResult<StoragePatchEntry> {
    let outcome = StoragePatchOutcome::unsupported(reason)?;
    Ok(StoragePatchEntry::new(target, outcome))
}

#[cfg(test)]
mod tests {
    use ibcmd_core::storage::StoragePatchOutcome;

    use super::*;

    fn explicit_axes() -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse("2.20").unwrap(),
            Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
            Some(CompatibilityMode::parse("Version8_3_24").unwrap()),
            StorageProfileId::parse("storage:mssql-test").unwrap(),
            Some(ContainerRevision::parse("revision-1").unwrap()),
        )
    }

    fn supported_axes(dialect: &str) -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse(dialect).unwrap(),
            None,
            None,
            StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
            None,
        )
    }

    fn target(key: &str, provenance: &str) -> StoragePatchTarget {
        StoragePatchTarget::new(
            StorageKey::new(key).unwrap(),
            MultipartIdentity::single(),
            StorageProvenance::new(provenance).unwrap(),
        )
    }

    fn compiled_bytes(entry: &StoragePatchEntry) -> &[u8] {
        match entry.outcome() {
            StoragePatchOutcome::Compiled(payload) => payload.bytes(),
            outcome => panic!("expected Compiled, got {outcome:?}"),
        }
    }

    #[test]
    fn explicit_axes_remain_independent() {
        let axes = explicit_axes();
        assert_eq!(axes.xml_dialect().to_string(), "2.20");
        assert_eq!(
            axes.platform_build().map(ToString::to_string).as_deref(),
            Some("8.3.27.1989")
        );
        assert_eq!(
            axes.compatibility_mode()
                .map(ToString::to_string)
                .as_deref(),
            Some("Version8_3_24")
        );
        assert_eq!(axes.storage_profile().as_str(), "storage:mssql-test");
        assert_eq!(
            axes.container_revision()
                .map(ToString::to_string)
                .as_deref(),
            Some("revision-1")
        );
    }

    #[test]
    fn module_and_raw_sources_compile_without_a_base() {
        let module = compile_source(
            &supported_axes("2.20"),
            CompileRequest::new(
                target("module.0", "CommonModules/Tools/Ext/Module.bsl"),
                SourcePayload::ModuleText {
                    text: b"Procedure Run()\nEndProcedure",
                    info: None,
                },
            ),
        )
        .unwrap();
        assert!(!compiled_bytes(&module).is_empty());

        let raw = compile_source(
            &supported_axes("2.21"),
            CompileRequest::new(
                target("template.0", "Templates/Raw/Ext/Template.txt"),
                SourcePayload::RawDeflated { bytes: b"raw body" },
            ),
        )
        .unwrap();
        assert!(!compiled_bytes(&raw).is_empty());
    }

    #[test]
    fn metadata_and_common_module_metadata_remain_base_dependent() {
        let metadata_xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <SessionParameter uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
    <Properties>
      <Name>CurrentUser</Name>
      <Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>User</v8:content></v8:item></Synonym>
      <Comment>Compiler test</Comment>
    </Properties>
  </SessionParameter>
</MetaDataObject>
"#;
        let metadata = compile_source(
            &supported_axes("2.20"),
            CompileRequest::new(
                target(
                    "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
                    "SessionParameters/CurrentUser.xml",
                ),
                SourcePayload::MetadataXml { xml: metadata_xml },
            ),
        )
        .unwrap();
        match metadata.outcome() {
            StoragePatchOutcome::NeedsBase { required, reason } => {
                assert_eq!(required.as_str(), "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb");
                assert!(
                    reason
                        .as_str()
                        .contains("metadata XML requires a base entry")
                );
            }
            outcome => panic!("expected NeedsBase, got {outcome:?}"),
        }

        let common_module_xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <CommonModule uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>Tools</Name>
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
        let common_module = compile_source(
            &supported_axes("2.21"),
            CompileRequest::new(
                target(
                    "aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa",
                    "CommonModules/Tools.xml",
                ),
                SourcePayload::CommonModuleMetadataXml {
                    xml: common_module_xml,
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            common_module.outcome(),
            StoragePatchOutcome::NeedsBase { .. }
        ));
    }

    #[test]
    fn command_interface_classifies_raw_and_readable_references() {
        let raw_xml = br#"
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
  <CommandsVisibility>
    <Command name="100:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
      <Visibility><xr:Common>true</xr:Common></Visibility>
    </Command>
  </CommandsVisibility>
</CommandInterface>
"#;
        let raw = compile_source(
            &supported_axes("2.20"),
            CompileRequest::new(
                target("subsystem.0", "Subsystems/Sales/Ext/CommandInterface.xml"),
                SourcePayload::CommandInterfaceXml { xml: raw_xml },
            ),
        )
        .unwrap();
        assert!(!compiled_bytes(&raw).is_empty());

        let readable_xml = br#"
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
  <CommandsVisibility>
    <Command name="Catalog.Products.StandardCommand.OpenList">
      <Visibility><xr:Common>true</xr:Common></Visibility>
    </Command>
  </CommandsVisibility>
</CommandInterface>
"#;
        let needs_base = compile_source(
            &supported_axes("2.20"),
            CompileRequest::new(
                target("subsystem.1", "Subsystems/Admin/Ext/CommandInterface.xml"),
                SourcePayload::CommandInterfaceXml { xml: readable_xml },
            ),
        )
        .unwrap();
        assert!(matches!(
            needs_base.outcome(),
            StoragePatchOutcome::NeedsBase { .. }
        ));
    }

    #[test]
    fn unmapped_and_unknown_sources_are_unsupported() {
        let unmapped = compile_source(
            &supported_axes("2.20"),
            CompileRequest::new(
                target(
                    "indexes.unknown",
                    "Documents/Order/Ext/AdditionalIndexes.xml",
                ),
                SourcePayload::AdditionalIndexes {
                    bytes: b"<AdditionalIndexes/>",
                    mapping: AdditionalIndexesMapping::Unmapped,
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            unmapped.outcome(),
            StoragePatchOutcome::Unsupported { .. }
        ));

        let unknown = compile_source(
            &supported_axes("2.20"),
            CompileRequest::new(
                target("future.0", "FutureObjects/One.xml"),
                SourcePayload::Unknown {
                    family: FamilyId::parse("future-object").unwrap(),
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            unknown.outcome(),
            StoragePatchOutcome::Unsupported { .. }
        ));
    }

    #[test]
    fn malformed_xml_is_a_source_error() {
        let metadata_error = compile_source(
            &supported_axes("2.20"),
            CompileRequest::new(
                target("broken.0", "Broken.xml"),
                SourcePayload::MetadataXml { xml: b"not XML" },
            ),
        )
        .unwrap_err();
        assert!(matches!(metadata_error, CompileError::Source { .. }));

        let command_interface_error = compile_source(
            &supported_axes("2.20"),
            CompileRequest::new(
                target("broken.1", "Ext/CommandInterface.xml"),
                SourcePayload::CommandInterfaceXml { xml: b"not XML" },
            ),
        )
        .unwrap_err();
        assert!(matches!(
            command_interface_error,
            CompileError::Source { family, .. } if family == "command-interface"
        ));
    }

    #[test]
    fn unknown_or_unverified_axes_fail_closed() {
        let candidates = [
            CompileAxes::new(
                XmlDialect::parse("2.99").unwrap(),
                None,
                None,
                StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
                None,
            ),
            CompileAxes::new(
                XmlDialect::parse("2.20").unwrap(),
                None,
                None,
                StorageProfileId::parse("storage:future").unwrap(),
                None,
            ),
            CompileAxes::new(
                XmlDialect::parse("2.20").unwrap(),
                Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
                None,
                StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
                None,
            ),
            CompileAxes::new(
                XmlDialect::parse("2.20").unwrap(),
                None,
                Some(CompatibilityMode::parse("Version8_3_24").unwrap()),
                StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
                None,
            ),
            CompileAxes::new(
                XmlDialect::parse("2.20").unwrap(),
                None,
                None,
                StorageProfileId::parse(SUPPORTED_STORAGE_PROFILE).unwrap(),
                Some(ContainerRevision::parse("revision-1").unwrap()),
            ),
        ];

        for (index, axes) in candidates.into_iter().enumerate() {
            let key = format!("unsupported-axes.{index}");
            let entry = compile_source(
                &axes,
                CompileRequest::new(
                    target(&key, "source/axis-check"),
                    SourcePayload::RawDeflated { bytes: b"body" },
                ),
            )
            .unwrap();
            assert!(matches!(
                entry.outcome(),
                StoragePatchOutcome::Unsupported { .. }
            ));
        }
    }

    #[test]
    fn prepacked_provenance_must_match_target() {
        let source = PrepackedSource::base_free(
            FamilyId::parse("form-body").unwrap(),
            StorageProvenance::new("Forms/Main/Ext/Form.xml").unwrap(),
            b"packed",
        );
        let request = CompileRequest::new(
            target("form.0", "Forms/Other/Ext/Form.xml"),
            SourcePayload::Prepacked(source),
        );
        let error = compile_source(&supported_axes("2.20"), request).unwrap_err();
        assert!(matches!(
            error,
            CompileError::PrepackedProvenanceMismatch { .. }
        ));
    }

    #[test]
    fn prepacked_input_makes_base_dependency_explicit() {
        let compiled = compile_prepacked(
            &supported_axes("2.20"),
            StorageKey::new("raw.0").unwrap(),
            MultipartIdentity::single(),
            PrepackedSource::base_free(
                FamilyId::parse("raw-template").unwrap(),
                StorageProvenance::new("Templates/Raw/Ext/Template.txt").unwrap(),
                b"packed",
            ),
        )
        .unwrap();
        assert_eq!(compiled_bytes(&compiled), b"packed");

        let needs_base = compile_prepacked(
            &supported_axes("2.20"),
            StorageKey::new("form.0").unwrap(),
            MultipartIdentity::single(),
            PrepackedSource::needs_base(
                FamilyId::parse("form-body").unwrap(),
                StorageProvenance::new("Forms/Main/Ext/Form.xml").unwrap(),
                StorageKey::new("form.0").unwrap(),
                "form body must be resolved by a later overlay",
            ),
        )
        .unwrap();
        assert!(matches!(
            needs_base.outcome(),
            StoragePatchOutcome::NeedsBase { required, .. } if required.as_str() == "form.0"
        ));
    }

    #[test]
    fn source_limits_are_checked_before_prepacked_clone_or_module_pack() {
        let prepacked = CompileRequest::prepacked(
            StorageKey::new("raw.0").unwrap(),
            MultipartIdentity::single(),
            PrepackedSource::base_free(
                FamilyId::parse("raw-template").unwrap(),
                StorageProvenance::new("Templates/Raw/Ext/Template.txt").unwrap(),
                b"four",
            ),
        );
        let error =
            compile_source_with_payload_limit(&supported_axes("2.20"), prepacked, 3).unwrap_err();
        assert!(matches!(
            error,
            CompileError::Patch(StoragePatchBuildError::Storage(
                StorageBuildError::PayloadTooLarge {
                    maximum: 3,
                    actual: 4
                }
            ))
        ));

        let module = CompileRequest::new(
            target("module.0", "CommonModules/Tools/Ext/Module.bsl"),
            SourcePayload::ModuleText {
                text: b"123",
                info: Some(b"456"),
            },
        );
        let error =
            compile_source_with_payload_limit(&supported_axes("2.20"), module, 5).unwrap_err();
        match error {
            CompileError::Source { family, source } => {
                assert_eq!(family, "module-text");
                assert_eq!(
                    source.to_string(),
                    "source input is 6 bytes, exceeding the 5-byte compiler bound"
                );
            }
            error => panic!("expected module-text Source error, got {error:?}"),
        }
    }

    #[test]
    fn public_compiler_boundary_has_no_io_or_adapter_types() {
        fn production(source: &str) -> &str {
            source.split("#[cfg(test)]").next().unwrap()
        }

        let source = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            production(include_str!("mod.rs")),
            production(include_str!("identity.rs")),
            production(include_str!("graph.rs")),
            production(include_str!("overlay.rs")),
            production(include_str!("readiness.rs")),
            production(include_str!("root.rs")),
            production(include_str!("version.rs")),
            production(include_str!("versions.rs"))
        );
        let forbidden = [
            ["std", "path"].join("::"),
            ["std", "fs"].join("::"),
            ["std", "process"].join("::"),
            ["std", "net"].join("::"),
            ["crate", "cli"].join("::"),
            ["crate", "mssql"].join("::"),
            ["rand", "::"].concat(),
            ["MetadataSource", "Context"].concat(),
            ["Path", "Buf"].concat(),
            ["sql", "cmd"].concat(),
            ["pack_simple_metadata_blob", "_from_xml"].concat(),
            ["pack_common_module_metadata_blob", "_from_xml"].concat(),
            ["pack_module_blob_bytes", "("].concat(),
            ["pack_module_blob_bytes", ","].concat(),
            ["pack_command_interface_blob_from_xml", "("].concat(),
            ["pack_command_interface_blob_from_xml", ","].concat(),
            ["dyn", " Fn"].concat(),
            ["impl", " Fn"].concat(),
        ];
        for forbidden in forbidden {
            assert!(
                !source.contains(&forbidden),
                "pure compiler boundary contains forbidden token {forbidden:?}"
            );
        }
    }
}
