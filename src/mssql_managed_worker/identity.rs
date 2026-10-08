use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ProcessIdentity {
    pub pid: u32,
    pub parent: u32,
    /// Original process-handle FILETIME, rather than rounded CIM time.
    pub birth_100ns: u64,
    pub executable: PathBuf,
    pub command_sha256: String,
}
