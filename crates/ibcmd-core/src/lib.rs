//! Platform-independent contracts for standalone conversion.
//!
//! This is the bottom layer for profiles, canonical models, diagnostics, and
//! migration contracts. It must remain independent of CLI, XML, SQL, process
//! execution, and the other codec crates.

#![forbid(unsafe_code)]

pub mod artifact;
pub mod version;

#[cfg(test)]
mod tests {
    #[test]
    fn crate_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ibcmd-core");
    }
}
