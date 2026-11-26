//! Pure source-to-storage-patch compilation.
//!
//! Callers supply already loaded source bytes and explicit version/profile
//! axes. Native-storage adapters remain responsible for loading and writing
//! data around this boundary. Bytes derived from a base artifact deliberately
//! stay outside this base-free compiler contract.
//!
//! The coordinator and its body codecs are the `ibcmd-compiler` crate's
//! `compiler` module, and this module keeps their paths. `bodies` re-exports
//! the codecs and the writers other crates own, and `families` is the root's
//! view of the family compilers.

pub use ibcmd_compiler::compiler::{
    AdditionalIndexesMapping, CompileAxes, CompileError, CompileRequest, CompileResult,
    PrepackedSource, SourcePayload, artifact, bootstrap, compile_overlay, compile_prepacked,
    compile_source, external_intake, external_owned, graph, identity, overlay, readiness, version,
    versions,
};
// The axes check the SQL adapter runs before it compiles.
pub(crate) use ibcmd_compiler::compiler::unsupported_axes_reason;

pub mod bodies;
pub mod families;

/// The bootstrap root and Configuration body compilers:
/// `ibcmd-compiler`'s `compiler::root`.
pub mod root {
    pub use ibcmd_compiler::compiler::root::{
        ConfigurationBodyProperties, ConfigurationEvidencedPropertyDigits,
        ConfigurationLocalizedString, ConfigurationRunMode, ConfigurationScriptVariant,
        SpecialEntryBuildError, compile_configuration_body, compile_root,
    };
    // The class-id to family-kind table the export and load adapters read.
    pub(crate) use ibcmd_compiler::compiler::root::configuration_family_kind;
}
