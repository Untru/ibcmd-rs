//! Base-free metadata descriptor compiler: a metadata object's XML becomes the
//! complete native text of its Config row without reading the target
//! database, so a configuration can be loaded into an empty infobase.
//!
//! The model is the `ibcmd-metadata-model` crate's `metadata_model` module,
//! and this module keeps its paths. The crate made a few of its crate-visible
//! items `#[doc(hidden)] pub` for the root's export and apply adapters; the
//! modules below name each of them crate-visibly, which shadows the glob, so
//! none of them joins the root's public API.

#![allow(
    hidden_glob_reexports,
    reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
)]

pub use ibcmd_metadata_model::metadata_model::*;

// Read only by the exporter, which names the crate; it still hides the item from the glob.
#[expect(
    unused_imports,
    reason = "the re-export hides a widened item from the public glob"
)]
pub(crate) use ibcmd_metadata_model::metadata_model::{external_data_source_children, interface};

pub(crate) mod external_data_source;

/// The platform's standard pictures, the `ibcmd-1c-vocab` crate's table,
/// under the path the crate's readers and writers name them by.
pub mod standard_pictures {
    #[expect(
        unused_imports,
        reason = "the root keeps the path; its one reader, the exporter, names the crate"
    )]
    pub(crate) use ibcmd_metadata_model::metadata_model::standard_pictures::{
        standard_picture_name, standard_picture_uuid,
    };
}

/// Reference objects: `ibcmd-metadata-model`'s `metadata_model::objects`.
pub mod objects {
    pub use ibcmd_metadata_model::metadata_model::objects::*;

    // Read only by the own apply (`ibcmd-config-apply`); it still hides the item from the glob.
    #[expect(
        unused_imports,
        reason = "the re-export hides a widened item from the public glob"
    )]
    pub(crate) use ibcmd_metadata_model::metadata_model::objects::owned_body_classes;
    pub(crate) use ibcmd_restructure::descriptor::{layout, standard_markers};

    /// The parts every reference object composes.
    pub(crate) mod parts {
        #[expect(
            unused_imports,
            reason = "the root keeps the path; its one reader, the exporter, names the crate"
        )]
        pub(crate) use ibcmd_metadata_model::metadata_model::objects::parts::Compat;
        #[cfg(test)]
        #[expect(
            unused_imports,
            reason = "the root keeps the path; its one reader, the exporter, names the crate"
        )]
        pub(crate) use ibcmd_metadata_model::metadata_model::objects::parts::compatibility;
        pub(crate) use ibcmd_restructure::descriptor::Slot;
    }
}

/// Registers, journals, sequences and numerators:
/// `ibcmd-metadata-model`'s `metadata_model::registers`.
pub mod registers {
    pub use ibcmd_metadata_model::metadata_model::registers::*;

    /// The parts the register families compose.
    pub mod parts {
        pub use ibcmd_metadata_model::metadata_model::registers::parts::*;

        // Read by a root test only; outside tests it still keeps the
        // widened table out of the glob above.
        #[cfg_attr(not(test), allow(unused_imports))]
        pub(crate) use ibcmd_metadata_model::metadata_model::registers::parts::STANDARD_PICTURE_CODES;
    }
}

/// The simple kinds: `ibcmd-metadata-model`'s `metadata_model::simple`.
pub mod simple {
    pub use ibcmd_metadata_model::metadata_model::simple::*;

    /// Their export direction.
    pub(crate) mod export {
        #[expect(
            unused_imports,
            reason = "the root keeps the path; its one reader, the exporter, names the crate"
        )]
        pub(crate) use ibcmd_metadata_model::metadata_model::simple::export::use_standard_commands;
    }
}

/// The common kinds: `ibcmd-metadata-model`'s `metadata_model::common`.
pub mod common {
    pub use ibcmd_metadata_model::metadata_model::common::*;

    // Read only by the exporter, which names the crate; it still hides the item from the glob.
    #[expect(
        unused_imports,
        reason = "the re-export hides a widened item from the public glob"
    )]
    pub(crate) use ibcmd_metadata_model::metadata_model::common::{
        MAIN_SERVER_UNAVAILABLE_BEHAVIORS, PARAMETER_USE_MODES, REPRESENTATIONS,
    };
}

/// The Configuration root: `ibcmd-metadata-model`'s `metadata_model::root`.
pub mod root {
    pub use ibcmd_metadata_model::metadata_model::root::*;

    /// Its export direction.
    pub mod export {
        pub use ibcmd_metadata_model::metadata_model::root::export::*;

        // Read only by the exporter, which names the crate; it still hides the item from the glob.
        #[expect(
            unused_imports,
            reason = "the re-export hides a widened item from the public glob"
        )]
        pub(crate) use ibcmd_metadata_model::metadata_model::root::export::{
            root_configuration_uuid, stored_shape,
        };

        // Read by no root code since `apply_check` moved to its crate; the
        // re-export still keeps the widened items out of the glob above.
        #[allow(
            unused_imports,
            reason = "hides the widened items from the public glob of this module"
        )]
        pub(crate) use ibcmd_metadata_model::metadata_model::root::export::{
            fold_split_compatibility, version_text,
        };
    }
}

/// Stored rows back to XML: `ibcmd-metadata-model`'s `metadata_model::export`.
pub mod export {
    pub use ibcmd_metadata_model::metadata_model::export::*;

    /// Owned names and the root's class kinds.
    pub mod names {
        pub use ibcmd_metadata_model::metadata_model::export::names::*;

        // Read only by the own apply (`ibcmd-config-apply`); it still hides the item from the glob.
        #[expect(
            unused_imports,
            reason = "the re-export hides a widened item from the public glob"
        )]
        pub(crate) use ibcmd_metadata_model::metadata_model::export::names::root_class_kinds;
    }

    /// The value writers the families share.
    pub mod values {
        #[expect(
            unused_imports,
            reason = "the root keeps the path; its one reader, the exporter, names the crate"
        )]
        pub(crate) use ibcmd_metadata_model::metadata_model::export::values::UnsupportedDescriptorValue;
    }
}

#[cfg(test)]
mod collection_tests;
