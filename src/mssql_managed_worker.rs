//! Internal lifetime authority for a product-created worker: the
//! `ibcmd-mssql-live` crate's `mssql_managed_worker`, re-exported
//! crate-visibly. Its command glue (`cli`), which reads the root's CLI and
//! runs the root's apply and activation, stays here.

pub(crate) use ibcmd_mssql_live::mssql_managed_worker::*;

pub(crate) mod cli;
