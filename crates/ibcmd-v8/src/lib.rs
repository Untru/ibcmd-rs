//! Binary V8 container layer for standalone conversion.
//!
//! This layer may use core contracts, but it remains independent of the CF,
//! XML, root CLI, SQL, and process-execution layers.

#![forbid(unsafe_code)]

pub mod block;
pub mod format;
pub mod format15;
pub mod format16;
pub mod reader;
pub mod writer;

#[cfg(test)]
mod tests {
    use ibcmd_core as _;

    #[test]
    fn crate_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ibcmd-v8");
    }
}
