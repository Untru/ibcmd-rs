//! Canonical metadata-envelope mapping and lossless fallback support.

mod common;
mod constant;
mod fallback;
mod functional_option;
mod functional_options_parameter;
mod language;
mod registry;

pub use common::{
    MetadataDecodeError, MetadataEnvelope, decode_metadata_envelope,
    decode_metadata_envelope_with_dialect,
};
pub use constant::{bundled_metadata_registry, register_constant_codec};
pub use functional_option::register_functional_option_codec;
pub use functional_options_parameter::register_functional_options_parameter_codec;
pub use language::register_language_codec;
pub use registry::{
    MetadataEncodeError, MetadataFamilyCodec, MetadataRegistry, MetadataRegistryError,
};
