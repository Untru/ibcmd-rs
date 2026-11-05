//! Kind names the dump pipeline routes on by name.
//!
//! The guarded physical-adapter slice must not repeat a family name in a
//! routing decision, so the facts live here: a caller compares against a
//! schema predicate instead of spelling the literal again.

/// The configuration's own common-template family (`CommonTemplates/`).
pub const COMMON_TEMPLATE_KIND: &str = "CommonTemplate";

/// Whether a row's kind is the configuration's own common-template family.
#[must_use]
pub const fn is_common_template_kind(kind: &str) -> bool {
    matches!(kind.as_bytes(), b"CommonTemplate")
}
