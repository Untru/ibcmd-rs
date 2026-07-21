//! Lossless XML/XCF adapter layer for standalone conversion.
//!
//! This layer depends only on `ibcmd-core`. An XML parser may be added when the
//! adapter implementation needs it; container, CF, CLI, SQL, and process
//! concerns do not belong here.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use ibcmd_core as _;

    #[test]
    fn crate_identity_is_stable() {
        assert_eq!(env!("CARGO_PKG_NAME"), "ibcmd-xml");
    }
}
