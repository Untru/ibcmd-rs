use std::fmt::Display;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Typed runtime-evidence argument categories used to keep secrets out of
/// durable subprocess journals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SanitizedRuntimeArgumentKind {
    PasswordSourceMarker,
    QueryDigestMarker,
    Ordinary,
}

/// Exact serialization and filesystem policy for durable subprocess journals.
pub(crate) struct SubprocessJournalSchema;

impl SubprocessJournalSchema {
    pub(crate) const fn redacted_password_source_marker() -> &'static str {
        "<password-source:redacted>"
    }

    pub(crate) fn argument_kind(argument: &str) -> SanitizedRuntimeArgumentKind {
        if matches!(
            argument,
            "<password-source:none>"
                | "<password-source:inline>"
                | "<password-source:settings>"
                | "<password-source:environment>"
                | "<password-source:redacted>"
        ) {
            SanitizedRuntimeArgumentKind::PasswordSourceMarker
        } else if argument
            .strip_prefix("<query-sha256:")
            .and_then(|digest| digest.strip_suffix('>'))
            .is_some_and(|digest| {
                digest.len() == 64
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
        {
            SanitizedRuntimeArgumentKind::QueryDigestMarker
        } else {
            SanitizedRuntimeArgumentKind::Ordinary
        }
    }

    pub(crate) fn query_digest_marker(query: &str) -> String {
        format!("<query-sha256:{:x}>", Sha256::digest(query.as_bytes()))
    }

    pub(crate) fn journal_parent(path: &Path) -> Option<&Path> {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
    }

    pub(crate) fn missing_parent_error(path: &Path) -> String {
        format!("journal path must have a parent: {}", path.display())
    }

    pub(crate) fn temporary_file_name(path: &Path, nonce: impl Display) -> String {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("subprocess-journal");
        format!(".{file_name}.{nonce}.tmp")
    }
}

#[cfg(test)]
mod tests {
    use super::{SanitizedRuntimeArgumentKind, SubprocessJournalSchema};

    #[test]
    fn subprocess_journal_schema_preserves_exact_redaction_markers() {
        assert_eq!(
            SubprocessJournalSchema::redacted_password_source_marker(),
            "<password-source:redacted>"
        );
        assert_eq!(
            SubprocessJournalSchema::query_digest_marker("SELECT top-secret-query"),
            "<query-sha256:f6525b0cc6deb8b9f73e69ef2c742d4a80d59c2fd2f0f364a2cf97319e5bd1dc>"
        );
        for marker in [
            "<password-source:none>",
            "<password-source:inline>",
            "<password-source:settings>",
            "<password-source:environment>",
            "<password-source:redacted>",
        ] {
            assert_eq!(
                SubprocessJournalSchema::argument_kind(marker),
                SanitizedRuntimeArgumentKind::PasswordSourceMarker,
                "{marker}"
            );
        }
        let query_marker =
            "<query-sha256:f6525b0cc6deb8b9f73e69ef2c742d4a80d59c2fd2f0f364a2cf97319e5bd1dc>";
        assert_eq!(
            SubprocessJournalSchema::argument_kind(query_marker),
            SanitizedRuntimeArgumentKind::QueryDigestMarker
        );
        assert_eq!(
            SubprocessJournalSchema::argument_kind("top-secret-password"),
            SanitizedRuntimeArgumentKind::Ordinary
        );
    }

    #[test]
    fn subprocess_journal_schema_rejects_inexact_marker_like_arguments() {
        for hostile in [
            "--password=<password-source:environment>",
            "<password-source:environment>suffix",
            "<password-source:",
            "<query-sha256:",
            "<query-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa>",
            "<query-sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA>",
            "<query-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa>suffix",
            "prefix<query-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa>",
        ] {
            assert_eq!(
                SubprocessJournalSchema::argument_kind(hostile),
                SanitizedRuntimeArgumentKind::Ordinary,
                "{hostile}"
            );
        }
    }

    #[test]
    fn subprocess_journal_schema_preserves_atomic_path_contract() {
        let path = std::path::Path::new("target/runtime-journal.json");
        assert_eq!(
            SubprocessJournalSchema::journal_parent(path),
            Some(std::path::Path::new("target"))
        );
        assert_eq!(
            SubprocessJournalSchema::temporary_file_name(path, "nonce"),
            ".runtime-journal.json.nonce.tmp"
        );
        assert_eq!(
            SubprocessJournalSchema::temporary_file_name(std::path::Path::new(""), "nonce"),
            ".subprocess-journal.nonce.tmp"
        );

        let parentless = std::path::Path::new("runtime-journal.json");
        assert_eq!(SubprocessJournalSchema::journal_parent(parentless), None);
        assert_eq!(
            SubprocessJournalSchema::missing_parent_error(parentless),
            "journal path must have a parent: runtime-journal.json"
        );
    }
}
