//! External data processors (.epf) and reports (.erf).
//!
//! The configuration pipeline already knows every rule for `DataProcessor`
//! and `Report` objects; an external object is the same object in a
//! different wrapper. This module adapts it at the boundary: the main row is
//! rewritten into the internal row shape, the pipeline runs unchanged, and
//! the produced tree is moved and renamed into the external layout.

pub mod brace;
pub mod copyinfo;
pub mod export;
pub mod header;
pub mod rename;
pub mod root_xml;
pub mod versions;

use sha1::{Digest, Sha1};

pub use ibcmd_schema::external_artifact::{
    DATA_PROCESSOR_CLASS as EXTERNAL_DATA_PROCESSOR_CLASS, ExternalArtifactKind as ExternalKind,
    REPORT_CLASS as EXTERNAL_REPORT_CLASS,
};

/// Deterministic uuid-shaped id for rows the adapter has to invent (never
/// written to output: the fields that carry it are dropped again).
pub fn derived_uuid(seed: &str) -> String {
    let d = Sha1::digest(seed.as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-5{:x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        d[0],
        d[1],
        d[2],
        d[3],
        d[4],
        d[5],
        d[6] & 0x0f,
        d[7],
        (d[8] & 0x3f) | 0x80,
        d[9],
        d[10],
        d[11],
        d[12],
        d[13],
        d[14],
        d[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_round_trips_through_class_id() {
        for kind in [ExternalKind::DataProcessor, ExternalKind::Report] {
            assert_eq!(ExternalKind::from_class_id(kind.class_id()), Some(kind));
        }
        assert_eq!(
            ExternalKind::from_class_id("bf845118-327b-4682-b5c6-285d2a0eb296"),
            None
        );
    }

    #[test]
    fn derived_uuid_is_stable_and_uuid_shaped() {
        let a = derived_uuid("x");
        assert_eq!(a, derived_uuid("x"));
        assert_ne!(a, derived_uuid("y"));
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes()[14], b'5');
    }
}
