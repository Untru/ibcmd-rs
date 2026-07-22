//! Pure source-to-storage-patch compilation.
//!
//! Callers supply already loaded source bytes, explicit version/profile axes,
//! and optional base bytes. Native-storage adapters remain responsible for
//! loading and writing data around this boundary.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::StorageProfileId;
use ibcmd_core::family::FamilyId;
use ibcmd_core::storage::{
    MultipartIdentity, StorageKey, StoragePatchBuildError, StoragePatchEntry, StoragePatchOutcome,
    StoragePatchTarget, StorageProvenance,
};
use ibcmd_core::version::{CompatibilityMode, ContainerRevision, PlatformBuild, XmlDialect};

use crate::module_blob::{
    command_interface_base_free_blockers, command_interface_xml_can_pack_without_base,
    common_module_metadata_base_free_blockers, metadata_xml_base_free_blockers,
    pack_command_interface_blob_from_xml, pack_common_module_metadata_blob_from_xml,
    pack_module_blob_bytes, pack_module_blob_container_bytes, pack_raw_deflated_blob_from_bytes,
    pack_simple_metadata_blob_from_xml,
};

pub mod overlay;

pub use overlay::compile_overlay;

/// Independent coordinates used by one compiler invocation.
///
/// No coordinate is inferred from another. The current legacy byte packers do
/// not branch on every axis yet, but retaining them at this boundary makes that
/// selection explicit and extensible.
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

/// Adapter-prepared bytes crossing the transitional pure compiler seam.
///
/// The family and provenance are mandatory so an adapter cannot submit an
/// anonymous payload. The compiler verifies that provenance against the patch
/// target before accepting the bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrepackedSource<'a> {
    family: FamilyId,
    provenance: StorageProvenance,
    bytes: &'a [u8],
}

impl<'a> PrepackedSource<'a> {
    /// Creates a named, attributable prepacked payload.
    pub const fn new(family: FamilyId, provenance: StorageProvenance, bytes: &'a [u8]) -> Self {
        Self {
            family,
            provenance,
            bytes,
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

    /// Returns the already packed native bytes.
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }
}

/// Byte-only source input for one target entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourcePayload<'a> {
    /// BSL module text with optional explicit base and info element bytes.
    ModuleText {
        text: &'a [u8],
        base: Option<&'a [u8]>,
        info: Option<&'a [u8]>,
    },
    /// An exported V8 module container that only needs outer packing.
    ModuleContainer { container: &'a [u8] },
    /// Source bytes stored as a raw-deflate native body.
    RawDeflated { bytes: &'a [u8] },
    /// Editable metadata XML, patched into an explicit native base when present.
    MetadataXml {
        xml: &'a [u8],
        base: Option<&'a [u8]>,
    },
    /// Common-module metadata XML and its optional explicit native base.
    CommonModuleMetadataXml {
        xml: &'a [u8],
        base: Option<&'a [u8]>,
    },
    /// CommandInterface XML, which is base-free only for raw command refs.
    CommandInterfaceXml {
        xml: &'a [u8],
        base: Option<&'a [u8]>,
    },
    /// AdditionalIndexes source bytes with an explicit target-mapping decision.
    AdditionalIndexes {
        bytes: &'a [u8],
        mapping: AdditionalIndexesMapping,
    },
    /// Bytes prepared by a legacy family packer outside this pure boundary.
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
    _axes: &CompileAxes,
    request: CompileRequest<'_>,
) -> CompileResult<StoragePatchEntry> {
    let (target, source) = request.into_parts();
    match source {
        SourcePayload::ModuleText { text, base, info } => {
            let packed = pack_module_blob_bytes(text, base, info)
                .map_err(|source| CompileError::invalid_source("module-text", source))?;
            compiled_entry(target, packed.blob)
        }
        SourcePayload::ModuleContainer { container } => {
            let packed = pack_module_blob_container_bytes(container)
                .map_err(|source| CompileError::invalid_source("module-container", source))?;
            compiled_entry(target, packed.blob)
        }
        SourcePayload::RawDeflated { bytes } => {
            let packed = pack_raw_deflated_blob_from_bytes(bytes)
                .map_err(|source| CompileError::invalid_source("raw-deflated", source))?;
            compiled_entry(target, packed.blob)
        }
        SourcePayload::MetadataXml { xml, base } => match base {
            Some(base) => {
                let packed = pack_simple_metadata_blob_from_xml(base, xml)
                    .map_err(|source| CompileError::invalid_source("metadata", source))?;
                compiled_entry(target, packed.blob)
            }
            None => {
                let blockers = metadata_xml_base_free_blockers(xml)
                    .map_err(|source| CompileError::invalid_source("metadata", source))?;
                needs_target_base(target, "metadata XML requires a base entry", blockers)
            }
        },
        SourcePayload::CommonModuleMetadataXml { xml, base } => match base {
            Some(base) => {
                let packed =
                    pack_common_module_metadata_blob_from_xml(base, xml).map_err(|source| {
                        CompileError::invalid_source("common-module-metadata", source)
                    })?;
                compiled_entry(target, packed.blob)
            }
            None => {
                let blockers =
                    common_module_metadata_base_free_blockers(xml).map_err(|source| {
                        CompileError::invalid_source("common-module-metadata", source)
                    })?;
                needs_target_base(
                    target,
                    "CommonModule metadata XML requires a base entry",
                    blockers,
                )
            }
        },
        SourcePayload::CommandInterfaceXml { xml, base } => match base {
            Some(base) => {
                let packed = pack_command_interface_blob_from_xml(base, xml)
                    .map_err(|source| CompileError::invalid_source("command-interface", source))?;
                compiled_entry(target, packed.blob)
            }
            None if command_interface_xml_can_pack_without_base(xml)
                .map_err(|source| CompileError::invalid_source("command-interface", source))? =>
            {
                let packed = pack_command_interface_blob_from_xml(&[], xml)
                    .map_err(|source| CompileError::invalid_source("command-interface", source))?;
                compiled_entry(target, packed.blob)
            }
            None => {
                let blockers = command_interface_base_free_blockers(xml)
                    .map_err(|source| CompileError::invalid_source("command-interface", source))?;
                needs_target_base(
                    target,
                    "CommandInterface XML requires a base entry",
                    blockers,
                )
            }
        },
        SourcePayload::AdditionalIndexes {
            bytes,
            mapping: AdditionalIndexesMapping::Confirmed,
        } => {
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
            compiled_entry(target, source.bytes().to_vec())
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

/// Accepts adapter-prepared bytes through the same classified patch contract.
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
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use ibcmd_core::storage::StoragePatchOutcome;

    use super::*;

    fn axes() -> CompileAxes {
        CompileAxes::new(
            XmlDialect::parse("2.20").unwrap(),
            Some(PlatformBuild::parse("8.3.27.1989").unwrap()),
            Some(CompatibilityMode::parse("Version8_3_24").unwrap()),
            StorageProfileId::parse("storage:mssql-test").unwrap(),
            Some(ContainerRevision::parse("revision-1").unwrap()),
        )
    }

    fn target(key: &str, provenance: &str) -> StoragePatchTarget {
        StoragePatchTarget::new(
            StorageKey::new(key).unwrap(),
            MultipartIdentity::single(),
            StorageProvenance::new(provenance).unwrap(),
        )
    }

    fn deflate(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn compiled_bytes(entry: &StoragePatchEntry) -> &[u8] {
        match entry.outcome() {
            StoragePatchOutcome::Compiled(payload) => payload.bytes(),
            outcome => panic!("expected Compiled, got {outcome:?}"),
        }
    }

    #[test]
    fn explicit_axes_remain_independent() {
        let axes = axes();
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
            &axes(),
            CompileRequest::new(
                target("module.0", "CommonModules/Tools/Ext/Module.bsl"),
                SourcePayload::ModuleText {
                    text: b"Procedure Run()\nEndProcedure",
                    base: None,
                    info: None,
                },
            ),
        )
        .unwrap();
        assert!(!compiled_bytes(&module).is_empty());

        let raw = compile_source(
            &axes(),
            CompileRequest::new(
                target("template.0", "Templates/Raw/Ext/Template.txt"),
                SourcePayload::RawDeflated { bytes: b"raw body" },
            ),
        )
        .unwrap();
        assert!(!compiled_bytes(&raw).is_empty());
    }

    #[test]
    fn metadata_without_base_needs_it_and_with_base_compiles() {
        let xml = br#"
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
        let no_base = compile_source(
            &axes(),
            CompileRequest::new(
                target(
                    "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
                    "SessionParameters/CurrentUser.xml",
                ),
                SourcePayload::MetadataXml { xml, base: None },
            ),
        )
        .unwrap();
        match no_base.outcome() {
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

        let mut base_plain = b"\xEF\xBB\xBF".to_vec();
        base_plain.extend_from_slice(
            br#"{1,{3,{1,0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},"OldName",{1,"ru","Old"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0},0}"#,
        );
        let base = deflate(&base_plain);
        let compiled = compile_source(
            &axes(),
            CompileRequest::new(
                target(
                    "bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb",
                    "SessionParameters/CurrentUser.xml",
                ),
                SourcePayload::MetadataXml {
                    xml,
                    base: Some(&base),
                },
            ),
        )
        .unwrap();
        assert!(!compiled_bytes(&compiled).is_empty());
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
            &axes(),
            CompileRequest::new(
                target("subsystem.0", "Subsystems/Sales/Ext/CommandInterface.xml"),
                SourcePayload::CommandInterfaceXml {
                    xml: raw_xml,
                    base: None,
                },
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
            &axes(),
            CompileRequest::new(
                target("subsystem.1", "Subsystems/Admin/Ext/CommandInterface.xml"),
                SourcePayload::CommandInterfaceXml {
                    xml: readable_xml,
                    base: None,
                },
            ),
        )
        .unwrap();
        assert!(matches!(
            needs_base.outcome(),
            StoragePatchOutcome::NeedsBase { .. }
        ));

        let base = deflate(
            b"{7,1,1,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{0,{0,{\"B\",0},0}},0,0,0,0,0}",
        );
        let compiled = compile_source(
            &axes(),
            CompileRequest::new(
                target("subsystem.1", "Subsystems/Admin/Ext/CommandInterface.xml"),
                SourcePayload::CommandInterfaceXml {
                    xml: readable_xml,
                    base: Some(&base),
                },
            ),
        )
        .unwrap();
        assert!(!compiled_bytes(&compiled).is_empty());
    }

    #[test]
    fn unmapped_and_unknown_sources_are_unsupported() {
        let unmapped = compile_source(
            &axes(),
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
            &axes(),
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
        let error = compile_source(
            &axes(),
            CompileRequest::new(
                target("broken.0", "Broken.xml"),
                SourcePayload::MetadataXml {
                    xml: b"not XML",
                    base: None,
                },
            ),
        )
        .unwrap_err();
        assert!(matches!(error, CompileError::Source { .. }));
    }

    #[test]
    fn prepacked_provenance_must_match_target() {
        let source = PrepackedSource::new(
            FamilyId::parse("form-body").unwrap(),
            StorageProvenance::new("Forms/Main/Ext/Form.xml").unwrap(),
            b"packed",
        );
        let request = CompileRequest::new(
            target("form.0", "Forms/Other/Ext/Form.xml"),
            SourcePayload::Prepacked(source),
        );
        let error = compile_source(&axes(), request).unwrap_err();
        assert!(matches!(
            error,
            CompileError::PrepackedProvenanceMismatch { .. }
        ));
    }

    #[test]
    fn public_compiler_boundary_has_no_io_or_adapter_types() {
        fn production(source: &str) -> &str {
            source.split("#[cfg(test)]").next().unwrap()
        }

        let source = format!(
            "{}\n{}",
            production(include_str!("mod.rs")),
            production(include_str!("overlay.rs"))
        );
        let forbidden = [
            ["std", "path"].join("::"),
            ["std", "fs"].join("::"),
            ["std", "process"].join("::"),
            ["crate", "cli"].join("::"),
            ["crate", "mssql"].join("::"),
            ["MetadataSource", "Context"].concat(),
            ["Path", "Buf"].concat(),
            ["sql", "cmd"].concat(),
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
