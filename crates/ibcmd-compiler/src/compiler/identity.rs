//! Bootstrap identities projected from the validated canonical graph.

pub(crate) use ibcmd_compiler_families::compiler::identity::derive_generation_uuid_v8;
pub use ibcmd_compiler_families::compiler::identity::{
    BootstrapIdentities, BootstrapIdentityError, BootstrapObjectIdentity, GenerationUuid,
    collect_artifact_identities, collect_bootstrap_identities,
};
