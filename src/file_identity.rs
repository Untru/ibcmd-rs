//! Physical identity of an opened file, including hard-link and symlink
//! aliases: `ibcmd-compiler`'s `file_identity`, which the external intake
//! reads; the save command reads it here.

#[expect(
    unused_imports,
    reason = "the root keeps the path; its one reader, the exporter, names the crate"
)]
pub(crate) use ibcmd_compiler::file_identity::file_identity;
