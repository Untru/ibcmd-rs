//! The platform registry: the `ibcmd-platform-spec` crate's `platform`, at
//! its old path. The process-wide export platform
//! (`note_export_platform`, `export_writes_default_button_importance`)
//! keeps its one owner there.
//!
//! The public names are listed, not globbed: the crate made three of its
//! crate-visible functions `#[doc(hidden)] pub` for the root, one of them
//! only under its `test-fixtures` feature, which the root's dev-dependency
//! turns on for every test build (the integration tests' too). A glob would
//! re-export that one publicly in those builds, where a crate-visible
//! `#[cfg(test)]` re-export, active only in the root's unit tests, cannot
//! shadow it. The three are re-exported crate-visibly instead.

pub use ibcmd_platform_spec::platform::{
    CONSTANT_DEFAULT_BUTTON_IMPORTANCE, CONSTANT_FEATURE_PREFIX, CONSTANT_FORM_LAYOUT,
    CONSTANT_XML_FORMAT, FEATURE_PALETTE_COLORS, FormLayout, PLATFORM_FLAG_HELP, Platform,
    PlatformFeature, PlatformSpec, XmlFormat, export_writes_default_button_importance,
    feature_uuid, for_xml_version, known, note_export_platform, parse, parse_flag,
};

pub(crate) use ibcmd_platform_spec::platform::{clear_export_platform, of_xml_dialect};
// The tests' per-thread override of the export platform (the crate's
// `test-fixtures` feature).
#[cfg(test)]
pub(crate) use ibcmd_platform_spec::platform::with_export_default_button_importance;
