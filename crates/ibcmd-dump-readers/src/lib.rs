//! Independent readers of stored configuration records used by source export.
//!
//! Additional-index records and evidence-bound configuration properties do not
//! need an SQL connection, active export state, the CLI or a file writer. The
//! root exporter retains their former module paths through private aliases.
//! Their source literals remain covered by the physical-adapter policy guard.
//!
//! The declarative schema facts of the metadata owners' stored records
//! (`metadata_owner_graph`: family layouts, identity semantics, diagnostic
//! vocabulary and the verified EDT produced-type order), formerly the root's
//! module of the same name, need none of those either; the root re-exports
//! the module at its old path.
//!
//! The readers of an external object's and an extension's stored rows keep
//! the root's module paths, which the root re-exports: `external::brace`, the
//! top-level fields of a brace list and the byte-order mark before it.

pub mod additional_indexes;
pub mod configuration_properties_evidence;
pub mod metadata_owner_graph;

/// External data processors and reports: what of the root's `external`
/// module reads no more than a stored row.
pub mod external {
    pub mod brace;
}
