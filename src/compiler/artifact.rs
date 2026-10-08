//! Shared bootstrap artifact admission, before either compiler can publish.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use ibcmd_xml::{XmlReader, metadata::inspect_package_intent};

pub use ibcmd_xml::metadata::PackageIntent;

#[derive(Debug)]
pub struct SourcePackage {
    pub intent: PackageIntent,
    pub root_xml: PathBuf,
}

/// Finds the one package root among a directory's root-level XML documents,
/// or inspects an explicitly supplied XML file. Nested family/body documents
/// belong to that package and are validated by its existing source compiler.
/// No source-tree reader quotas or archive suffix inference are introduced.
pub fn discover_source_package(source: &Path) -> Result<SourcePackage> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("failed to inspect source `{}`", source.display()))?;
    if metadata.file_type().is_symlink() {
        bail!("package source `{}` is a symbolic link", source.display());
    }
    let mut files = if metadata.is_file() {
        vec![source.to_path_buf()]
    } else if metadata.is_dir() {
        let mut files = Vec::new();
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            let path = entry.path();
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
            {
                let kind = entry.file_type()?;
                if kind.is_symlink() {
                    bail!("root XML `{}` is a symbolic link", path.display());
                }
                if kind.is_file() {
                    files.push(path);
                }
            }
        }
        files
    } else {
        bail!(
            "package source `{}` is not a file or directory",
            source.display()
        );
    };
    files.sort();
    let mut package: Option<SourcePackage> = None;
    for path in files {
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read root XML `{}`", path.display()))?;
        let document = XmlReader::from_slice(&bytes)
            .with_context(|| format!("invalid root XML `{}`", path.display()))?;
        let Some(intent) = inspect_package_intent(&document)
            .with_context(|| format!("invalid package root `{}`", path.display()))?
        else {
            continue;
        };
        if let Some(previous) = &package {
            bail!(
                "ambiguous package roots: `{}` ({:?}) and `{}` ({intent:?})",
                previous.root_xml.display(),
                previous.intent,
                path.display(),
            );
        }
        package = Some(SourcePackage {
            intent,
            root_xml: path,
        });
    }
    package.with_context(|| {
        format!(
            "source `{}` contains no Configuration, Extension, ExternalDataProcessor or ExternalReport package root",
            source.display(),
        )
    })
}

/// Checks a recognized requested suffix against the XML intent. Other suffixes
/// retain the existing CF CLI behavior; none select a different builder.
pub fn validate_output_intent(intent: PackageIntent, output: &Path) -> Result<()> {
    let Some(extension) = output.extension().and_then(|value| value.to_str()) else {
        return Ok(());
    };
    let requested = match extension.to_ascii_lowercase().as_str() {
        "cf" => PackageIntent::Configuration,
        "cfe" => PackageIntent::Extension,
        "epf" => PackageIntent::ExternalDataProcessor,
        "erf" => PackageIntent::ExternalReport,
        _ => return Ok(()),
    };
    if intent != requested {
        bail!(
            "output `{}` requests {requested:?}, but XML declares {intent:?}",
            output.display()
        );
    }
    Ok(())
}

/// Artifact selection over the existing canonical graph, never a second IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactScope {
    intent: PackageIntent,
    root_uuid: ibcmd_core::identity::ObjectUuid,
    main_uuid: ibcmd_core::identity::ObjectUuid,
}
impl ArtifactScope {
    pub const fn configuration(root_uuid: ibcmd_core::identity::ObjectUuid) -> Self {
        Self {
            intent: PackageIntent::Configuration,
            root_uuid,
            main_uuid: root_uuid,
        }
    }
    pub fn from_source_identity(
        identity: ibcmd_xml::metadata::PackageRootIdentity,
    ) -> Result<Self> {
        let root_uuid = match identity.intent {
            PackageIntent::Configuration | PackageIntent::Extension => {
                if identity.contained.is_some() {
                    bail!("configuration scope cannot alias an external contained root");
                }
                identity.main_uuid
            }
            PackageIntent::ExternalDataProcessor | PackageIntent::ExternalReport => {
                let contained = identity
                    .contained
                    .context("external package requires contained identity")?;
                let kind = match identity.intent {
                    PackageIntent::ExternalDataProcessor => {
                        crate::external::ExternalKind::DataProcessor
                    }
                    _ => crate::external::ExternalKind::Report,
                };
                if contained.class_id.to_string() != kind.class_id() {
                    bail!(
                        "external package class differs from declared {:?}",
                        identity.intent
                    );
                }
                contained.object_id
            }
        };
        if [root_uuid, identity.main_uuid]
            .iter()
            .any(|u| u.as_bytes().iter().all(|b| *b == 0))
        {
            bail!("artifact main/owner identity cannot be nil");
        }
        Ok(Self {
            intent: identity.intent,
            root_uuid,
            main_uuid: identity.main_uuid,
        })
    }
    pub const fn intent(self) -> PackageIntent {
        self.intent
    }
    /// Canonical semantic/module owner; for an external object this is ObjectId.
    pub const fn root_uuid(self) -> ibcmd_core::identity::ObjectUuid {
        self.root_uuid
    }
    /// Physical main descriptor identity; external aliases do not own modules.
    pub const fn main_uuid(self) -> ibcmd_core::identity::ObjectUuid {
        self.main_uuid
    }
    pub const fn root_kind(self) -> &'static str {
        match self.intent {
            PackageIntent::Configuration | PackageIntent::Extension => "Configuration",
            PackageIntent::ExternalDataProcessor => "DataProcessor",
            PackageIntent::ExternalReport => "Report",
        }
    }
}
impl SourcePackage {
    pub fn artifact_scope(&self) -> Result<ArtifactScope> {
        let bytes = fs::read(&self.root_xml)?;
        let doc = XmlReader::from_slice(&bytes)?;
        let identity = ibcmd_xml::metadata::inspect_package_identity(&doc)?
            .context("package root disappeared")?;
        if identity.intent != self.intent {
            bail!("package intent changed before graph selection");
        }
        ArtifactScope::from_source_identity(identity)
    }
}
