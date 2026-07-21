//! Canonical metadata-envelope mapping and lossless fallback support.

mod common;
mod fallback;
mod registry;

pub use common::{
    MetadataDecodeError, MetadataEnvelope, decode_metadata_envelope,
    decode_metadata_envelope_with_dialect,
};
pub use registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
