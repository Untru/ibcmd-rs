//! What the packers of a configuration's stored bodies read of its XML
//! source tree, as one seam: [`MetadataSource`]. A writer that takes a
//! `&dyn MetadataSource` reads the tree only through it, so it can live in a
//! crate that does not depend on the packers (`ibcmd-metadata-blob`, whose
//! `MetadataSourceContext` is the implementation the tools build), and a test
//! can answer it with a stub.
//!
//! The types the seam returns are here too: a source file's bytes
//! ([`SourceBytes`]), and the answers the context memoises for the interface
//! and help writers ([`InterfaceResolutionMemo`], [`HelpReference`]). They
//! moved from `ibcmd-metadata-blob`, which re-exports them.
//!
//! They came from the physical-adapter guard's slice, so the guard scans
//! every non-test `*.rs` file under this crate's `src/`
//! (`$GuardedCrateDirectories` in `tools/validate-physical-adapter-policy.ps1`).
//! A file added here is in the slice at once and fails the guard until its
//! record is reviewed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use ibcmd_form_native::ConfigurationObject;

/// A configuration's XML source tree as the packers read it: its files, and
/// the names it declares resolved to the uuids and type ids a stored body
/// holds.
///
/// The methods are `MetadataSourceContext`'s own, by the same names; its
/// implementation calls them, so its answers, its errors and what it
/// memoises are theirs. A resolver's error names the reference and the file
/// it read, and the caller adds its own context.
pub trait MetadataSource: std::fmt::Debug + Send + Sync {
    /// The root of the tree: the folder that holds `Configuration.xml`.
    fn source_root(&self) -> &Path;

    /// The bytes of a source file, a path under the root: shared when the
    /// source holds them in memory, else read for the caller.
    fn read_source(&self, path: &Path) -> std::io::Result<SourceBytes>;

    /// The uuid `Configuration.xml` declares for the configuration itself,
    /// or `None` when it cannot be read.
    fn configuration_uuid(&self) -> Option<String>;

    /// One metadata object of the tree, by `"<Class>.<Name>"`; `None` when
    /// the tree has no such object, its file does not parse or holds another
    /// class.
    fn configuration_object(&self, key: &str) -> Option<Arc<ConfigurationObject>>;

    /// The storage `TypeId` of a generated or defined type
    /// (`cfg:CatalogRef.X`, `DefinedType.X`), read from the owning object.
    fn resolve_metadata_type_id(&self, reference: &str) -> Result<String>;

    /// The uuid of a metadata object a reference names (`Catalog.X`, a nested
    /// `Subsystem.A.Subsystem.B`, `CalculationRegister.R.Recalculation.X`).
    fn resolve_metadata_reference_uuid(&self, reference: &str) -> Result<String>;

    /// The uuid of `StyleItem.<Name>`.
    fn resolve_style_item_uuid(&self, reference: &str) -> Result<String>;

    /// The uuid of `CommonPicture.<Name>`.
    fn resolve_common_picture_uuid(&self, reference: &str) -> Result<String>;

    /// The uuid of `CommandGroup.<Name>`.
    fn resolve_command_group_uuid(&self, reference: &str) -> Result<String>;

    /// The uuid of a command a reference names: `<Owner>.Command.<Name>`,
    /// read from the owner's XML; any other reference as
    /// [`resolve_metadata_reference_uuid`](Self::resolve_metadata_reference_uuid)
    /// resolves it.
    fn resolve_command_reference_uuid(&self, reference: &str) -> Result<String>;

    /// The uuid of a form a property names: `CommonForm.<Name>` or
    /// `<Owner>.Form.<Name>`, read from the form's own XML. A bare uuid (a
    /// form the configuration no longer has) answers itself, lowercase.
    fn resolve_form_uuid(&self, reference: &str) -> Result<String>;

    /// The uuid of the child `<child_kind>` named `child_name` that the XML
    /// of the object `owner_reference` names declares (an attribute, a
    /// tabular section, a command).
    fn resolve_metadata_child_uuid(
        &self,
        owner_reference: &str,
        child_kind: &str,
        child_name: &str,
    ) -> Result<String>;

    /// The `<Item id="...">` the `Ext/Predefined.xml` of the object
    /// `owner_reference` gives its predefined item `item_name`, looked up
    /// over the whole nested tree.
    fn resolve_predefined_item_id(&self, owner_reference: &str, item_name: &str) -> Result<String>;

    /// The names of the information register's dimensions whose `<Master>`
    /// is `true`, in document order.
    fn information_register_master_dimensions(&self, register: &str) -> Result<Vec<String>>;

    /// The uuid of `<prefix><Name>`, read from `<folder>/<Name>.xml`, which
    /// must declare the class `expected_kind`.
    fn resolve_simple_metadata_uuid(
        &self,
        reference: &str,
        expected_kind: &str,
        folder: &str,
        prefix: &str,
    ) -> Result<String>;

    /// The storage `TypeId` of a generated type named the way a
    /// data-composition schema names it (`CatalogRef.X`), or `None`.
    fn dcs_generated_type_id(&self, name: &str) -> Option<String>;

    /// The tree's style items: lowercase uuid -> name.
    fn dcs_style_items(&self) -> &BTreeMap<String, String>;

    /// The tree's common pictures and style items, uuid ->
    /// `CommonPicture.<Name>` / `StyleItem.<Name>`: what the template
    /// exporter names a chart's references by.
    fn moxel_object_refs(&self) -> Result<BTreeMap<String, String>>;

    /// What a readable help reference names in the tree: a uuid, or
    /// [`HelpReference::Absent`] when no file of the tree declares it.
    fn help_reference(&self, reference: &str) -> Result<HelpReference>;

    /// What the interface writers resolve more than once, kept for the life
    /// of the source.
    fn interface_memo(&self) -> &Mutex<InterfaceResolutionMemo>;
}

/// A source file's bytes: shared with the preloaded store, or read for the
/// caller.
pub enum SourceBytes {
    Shared(Arc<Vec<u8>>),
    Owned(Vec<u8>),
}

impl std::ops::Deref for SourceBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Shared(bytes) => bytes,
            Self::Owned(bytes) => bytes,
        }
    }
}

impl SourceBytes {
    /// The bytes as an owned vector (a copy when they are shared).
    pub fn into_vec(self) -> Vec<u8> {
        match self {
            Self::Shared(bytes) => bytes.as_ref().clone(),
            Self::Owned(bytes) => bytes,
        }
    }
}

/// What resolving names reads more than once, kept for the life of the
/// source context: the class and uuid of a metadata file, and the commands an
/// owner declares.
#[derive(Debug, Default)]
pub struct InterfaceResolutionMemo {
    /// A metadata file's `(class, uuid)`, by path; `None` when it has none.
    pub headers: BTreeMap<PathBuf, Option<(String, String)>>,
    /// The commands an owner's file declares, name -> uuid, by path; `None`
    /// when it cannot be read.
    pub commands: BTreeMap<PathBuf, Option<Arc<BTreeMap<String, String>>>>,
}

/// What a readable reference names in the source tree.
#[derive(Clone, Debug)]
pub enum HelpReference {
    /// The uuid of what it names.
    Resolved(String),
    /// No file of the tree declares it: the configuration has no such object.
    Absent,
}
