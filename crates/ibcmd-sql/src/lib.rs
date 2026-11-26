//! Database clients and values shared by the infobase adapters.
//!
//! `runtime_evidence_schema`, formerly the root's module of the same name,
//! is the exact serialization and filesystem policy of the durable journal
//! of their calls (the `--sqlcmd` tools' runs and the built-in client's
//! queries): the redaction markers that keep secrets out of it and its
//! atomic replacement path. The root re-exports it at its old path.

pub mod runtime_evidence_schema;
pub mod sql;
pub use sql::*;
