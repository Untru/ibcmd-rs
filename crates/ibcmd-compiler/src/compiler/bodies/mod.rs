//! Profile-gated codecs for native bodies that are not metadata-family rows.
//! The root's `compiler::bodies` re-exports them beside the writers and
//! codecs other crates own.

// The body-profile selection is `ibcmd-compiler-families`'s; the codecs
// here keep naming it from this module.
pub use ibcmd_compiler_families::compiler::bodies::BodyProfileError;
pub(crate) use ibcmd_compiler_families::compiler::bodies::SelectedBodyProfile;

pub mod dcs;
// The 8.5 picture-value rewrite of a schema template, which the DCS codec
// reads; formerly the root's `mssql_dump::dcs_2_21`, whose path the root keeps.
pub mod dcs_2_21;
pub mod form;
mod form_chart {
    //! The chart writers, in `ibcmd-form-writer` (re-exported by
    //! `ibcmd-metadata-blob`): what the moxel writer reads.
    pub(crate) use ibcmd_metadata_blob::form_chart::{ChartHost, chart_value, gantt_chart_value};
}
pub mod mxl;
pub mod mxl_native;
pub mod predefined;
pub mod rights;
pub mod support;
pub mod template;
// `TemplateKind` and its codec error, which `template` re-exports.
pub mod template_kind;
