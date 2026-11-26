//! Offline EDT adapter. The pinned codec IR is private to this crate.
#![forbid(unsafe_code)]

pub use ibcmd_xml::source_tree::ReaderLimits;

use ibcmd_core::model::CanonicalConfiguration;
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use std::path::Path;

mod bounded;
mod codec;
mod directory;
mod disk;
mod extensions;
mod provenance;

pub use directory::{
    DirectoryConversion, DirectoryFileAccounting, DirectorySource, read_directory_project,
    read_directory_source,
};

#[derive(Clone, Debug)]
pub struct ConversionOptions {
    /// Explicit supported EDT project model version; no latest-version fallback.
    pub edt_version: String,
    /// Designer XML dialect, currently 2.20 or 2.21.
    pub xml_dialect: String,
    /// Explicit runtime stamped into PROJECT.PMF when generating a project.
    pub runtime_version: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Project {
    tree: SourceTree,
}
impl Project {
    pub fn tree(&self) -> &SourceTree {
        &self.tree
    }
    pub fn entries(&self) -> &[SourceEntry] {
        self.tree.entries()
    }
    pub fn canonical(
        &self,
        options: &ConversionOptions,
    ) -> Result<CanonicalConfiguration, EdtError> {
        Ok(edt_to_xml(self, options)?.canonical)
    }
    pub fn from_tree(tree: SourceTree) -> Result<Self, EdtError> {
        tree.validate().map_err(EdtError::source)?;
        bounded::validate_tree(&tree)?;
        if !tree
            .entries()
            .iter()
            .any(|e| e.path().as_str() == "DT-INF/PROJECT.PMF")
        {
            return Err(EdtError::new("missing DT-INF/PROJECT.PMF"));
        }
        if !tree
            .entries()
            .iter()
            .any(|e| e.path().as_str() == "src/Configuration/Configuration.mdo")
        {
            return Err(EdtError::new("missing src/Configuration/Configuration.mdo"));
        }
        Ok(Self { tree })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    Converted,
    Retained,
}
#[derive(Clone, Debug)]
pub struct FileAccounting {
    pub path: SourcePath,
    pub disposition: Disposition,
}
/// Explicit semantic resources used by this conversion. These capabilities
/// describe adapter transport; they do not claim the installed EDT model stores
/// the additional per-use values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceExtensionUse {
    pub id: &'static str,
    pub resources: usize,
    pub references: usize,
}
#[derive(Debug)]
pub struct Conversion {
    pub tree: SourceTree,
    pub canonical: CanonicalConfiguration,
    pub accounting: Vec<FileAccounting>,
    pub extensions: Vec<SourceExtensionUse>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdtError {
    message: String,
}
impl EdtError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
    pub(crate) fn source(error: impl std::fmt::Display) -> Self {
        Self::new(error.to_string())
    }
}
impl std::fmt::Display for EdtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "edt: {}", self.message)
    }
}
impl std::error::Error for EdtError {}

pub fn read_project(path: impl AsRef<Path>, limits: ReaderLimits) -> Result<Project, EdtError> {
    Project::from_tree(bounded::read_tree(path.as_ref(), limits)?)
}

/// Bounded XML inventory, using the same pre-parser security boundary as EDT.
pub fn read_xml_source(
    path: impl AsRef<Path>,
    limits: ReaderLimits,
) -> Result<SourceTree, EdtError> {
    bounded::read_tree(path.as_ref(), limits)
}

pub fn edt_to_xml(project: &Project, options: &ConversionOptions) -> Result<Conversion, EdtError> {
    codec::edt_to_xml(project, options)
}

pub fn xml_to_edt(
    source: &SourceTree,
    options: &ConversionOptions,
) -> Result<Conversion, EdtError> {
    codec::xml_to_edt(source, options)
}
