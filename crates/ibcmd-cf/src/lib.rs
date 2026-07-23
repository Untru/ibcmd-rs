//! CF archive adapter layer for standalone conversion.
//!
//! This layer composes platform-independent core contracts with the binary V8
//! container layer. XML, root CLI, SQL, and process-execution concerns remain
//! outside this crate.

#![forbid(unsafe_code)]

pub mod archive;
pub mod export;
pub mod overlay;
pub mod payload;
pub mod preamble;
pub mod tree;
pub mod writer;

#[cfg(test)]
mod tests {
    use ibcmd_core as _;
    use ibcmd_v8 as _;

    #[test]
    fn crate_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ibcmd-cf");
    }
}
