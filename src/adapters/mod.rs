//! Root-crate bridges around legacy providers.

/// The legacy MSSQL store's descriptor: the `ibcmd-platform-spec` crate's
/// `adapters::mssql_legacy`, at its old path.
///
/// The crate made the crate-visible items the exporter reads
/// `#[doc(hidden)] pub`; the names below re-export each of them
/// crate-visibly, which shadows the glob, so none of them joins the root's
/// public API.
pub mod mssql_legacy {
    #![allow(
        hidden_glob_reexports,
        reason = "each crate-visible re-export hides a widened item from the public glob on purpose"
    )]

    pub use ibcmd_platform_spec::adapters::mssql_legacy::*;

    pub(crate) use ibcmd_platform_spec::adapters::mssql_legacy::{
        MssqlConfigurationTableRole, MssqlExportInventoryPlan, MssqlExportInventoryScope,
        decode_task_internal_uuid_slots,
    };
    // Named only by `decode_task_internal_uuid_slots`' signature; they still
    // hide the items from the glob.
    #[expect(
        unused_imports,
        reason = "each re-export hides a widened item from the public glob"
    )]
    pub(crate) use ibcmd_platform_spec::adapters::mssql_legacy::{
        MssqlLegacyTaskInternalUuidSlotError, MssqlLegacyTaskInternalUuidSlots,
    };
}
