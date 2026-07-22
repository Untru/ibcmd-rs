//! Filesystem adapter for the pure `ibcmd-core` profile registry.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use ibcmd_core::profile::{
    ProfileDocument, ProfileRegistry, ProfileSourceKind, parse_profile_source, resolve_profiles,
};

/// Default maximum number of external profile files.
pub const DEFAULT_MAX_EXTERNAL_FILES: usize = 256;
/// Default maximum encoded size of one external profile.
pub const DEFAULT_MAX_EXTERNAL_FILE_BYTES: u64 = 1024 * 1024;
/// Default maximum encoded size of all external profiles.
pub const DEFAULT_MAX_TOTAL_EXTERNAL_BYTES: u64 = 8 * 1024 * 1024;

/// Resource bounds applied while loading untrusted external profile files.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileRegistryLimits {
    /// Maximum number of regular `.json` files in the external directory.
    pub max_external_files: usize,
    /// Maximum encoded size of one external file.
    pub max_external_file_bytes: u64,
    /// Maximum encoded size of all external files combined.
    pub max_total_external_bytes: u64,
}

impl Default for ProfileRegistryLimits {
    fn default() -> Self {
        Self {
            max_external_files: DEFAULT_MAX_EXTERNAL_FILES,
            max_external_file_bytes: DEFAULT_MAX_EXTERNAL_FILE_BYTES,
            max_total_external_bytes: DEFAULT_MAX_TOTAL_EXTERNAL_BYTES,
        }
    }
}

/// One trusted JSON profile compiled into or otherwise bundled with the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BundledProfile<'a> {
    /// Stable provenance name.
    pub name: &'a str,
    /// UTF-8 JSON document.
    pub json: &'a str,
}

/// Deterministically ordered standalone seed profiles embedded in the binary.
pub const BUNDLED_PROFILES: &[BundledProfile<'static>] = &[
    BundledProfile {
        name: "profiles/platform/8.3.24.1819.json",
        json: include_str!("../profiles/platform/8.3.24.1819.json"),
    },
    BundledProfile {
        name: "profiles/platform/8.3.27.1989.json",
        json: include_str!("../profiles/platform/8.3.27.1989.json"),
    },
    BundledProfile {
        name: "profiles/platform/8.5.1.1150.json",
        json: include_str!("../profiles/platform/8.5.1.1150.json"),
    },
    BundledProfile {
        name: "profiles/xml/2.17.json",
        json: include_str!("../profiles/xml/2.17.json"),
    },
    BundledProfile {
        name: "profiles/xml/2.20.json",
        json: include_str!("../profiles/xml/2.20.json"),
    },
    BundledProfile {
        name: "profiles/xml/2.21.json",
        json: include_str!("../profiles/xml/2.21.json"),
    },
];

/// Resolves the six project-owned seed profiles without filesystem access.
pub fn load_bundled_profile_registry() -> Result<ProfileRegistry> {
    load_profile_registry(BUNDLED_PROFILES, None, ProfileRegistryLimits::default())
}

/// Loads trusted bundled inputs and, optionally, an external profile directory.
///
/// External profiles are read in filename order. Only regular files whose
/// extension is exactly `.json` are considered; symlinks and subdirectories are
/// ignored. The core parser enforces strict JSON, profile identity, inheritance,
/// provenance, and the external `experimental` trust boundary.
pub fn load_profile_registry(
    bundled: &[BundledProfile<'_>],
    external_dir: Option<&Path>,
    limits: ProfileRegistryLimits,
) -> Result<ProfileRegistry> {
    let mut documents = Vec::<ProfileDocument>::new();
    for profile in bundled {
        documents.push(
            parse_profile_source(profile.name, ProfileSourceKind::Bundled, profile.json)
                .with_context(|| format!("failed to parse bundled profile `{}`", profile.name))?,
        );
    }

    if let Some(directory) = external_dir {
        documents.extend(load_external_profiles(directory, limits)?);
    }

    resolve_profiles(documents).context("failed to resolve profile registry")
}

fn load_external_profiles(
    directory: &Path,
    limits: ProfileRegistryLimits,
) -> Result<Vec<ProfileDocument>> {
    let mut files = Vec::<(String, PathBuf)>::new();
    let mut qualifying_file_count = 0_usize;
    for entry in fs::read_dir(directory).with_context(|| {
        format!(
            "failed to read external profile directory `{}`",
            directory.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "failed to enumerate external profile directory `{}`",
                directory.display()
            )
        })?;
        let file_type = entry.file_type().with_context(|| {
            format!(
                "failed to inspect external profile `{}`",
                entry.path().display()
            )
        })?;
        if !file_type.is_file()
            || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        qualifying_file_count = qualifying_file_count
            .checked_add(1)
            .ok_or_else(|| anyhow!("external profile file count overflow"))?;
        if qualifying_file_count > limits.max_external_files {
            bail!(
                "external profile file count exceeds limit {}",
                limits.max_external_files
            );
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("external profile filename is not valid UTF-8"))?;
        files.push((name, entry.path()));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut total_bytes = 0_u64;
    let mut documents = Vec::with_capacity(files.len());
    for (name, path) in files {
        let metadata = fs::metadata(&path)
            .with_context(|| format!("failed to inspect external profile `{name}`"))?;
        if metadata.len() > limits.max_external_file_bytes {
            bail!(
                "external profile `{name}` size {} exceeds per-file limit {}",
                metadata.len(),
                limits.max_external_file_bytes
            );
        }

        let mut bytes = Vec::new();
        File::open(&path)
            .with_context(|| format!("failed to open external profile `{name}`"))?
            .take(limits.max_external_file_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .with_context(|| format!("failed to read external profile `{name}`"))?;
        let actual_bytes = u64::try_from(bytes.len())
            .map_err(|_| anyhow!("external profile `{name}` is too large to account for"))?;
        if actual_bytes > limits.max_external_file_bytes {
            bail!(
                "external profile `{name}` size {actual_bytes} exceeds per-file limit {}",
                limits.max_external_file_bytes
            );
        }
        total_bytes = total_bytes
            .checked_add(actual_bytes)
            .ok_or_else(|| anyhow!("external profile byte count overflow"))?;
        if total_bytes > limits.max_total_external_bytes {
            bail!(
                "external profile total size {total_bytes} exceeds limit {}",
                limits.max_total_external_bytes
            );
        }

        let json = String::from_utf8(bytes)
            .with_context(|| format!("external profile `{name}` is not valid UTF-8"))?;
        let source_name = format!("external/{name}");
        documents.push(
            parse_profile_source(&source_name, ProfileSourceKind::External, &json)
                .with_context(|| format!("failed to parse external profile `{name}`"))?,
        );
    }
    Ok(documents)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use ibcmd_core::artifact::ProfileId;
    use ibcmd_core::detection::{DetectionObservations, detect_profiles, require_exact_target};
    use ibcmd_core::profile::ProfileStatus;

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "ibcmd-rs-profile-{label}-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str, contents: &str) {
            fs::write(self.0.join(name), contents).unwrap();
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn experimental(id: &str) -> String {
        format!(r#"{{"schema_version":1,"id":"{id}","status":"experimental"}}"#)
    }

    #[test]
    fn bundled_seed_profiles_keep_version_axes_independent() {
        let registry = load_bundled_profile_registry().unwrap();
        assert_eq!(BUNDLED_PROFILES.len(), 6);
        assert_eq!(registry.profiles().len(), 6);

        for version in ["2.17", "2.20", "2.21"] {
            let id = ProfileId::parse(&format!("xml-{version}")).unwrap();
            let profile = registry.get(&id).unwrap();
            assert_eq!(profile.status.value, ProfileStatus::Experimental);
            assert_eq!(
                profile.xml_dialect.as_ref().unwrap().value.to_string(),
                version
            );
            assert!(profile.platform_build.is_none());
            assert!(profile.compatibility_mode.is_none());
            assert!(profile.storage_profile.is_none());
            assert!(profile.container_revision.is_none());
            assert!(profile.dbms.is_none());
            assert_eq!(
                profile.fingerprints.len(),
                if version == "2.21" { 3 } else { 1 }
            );
            assert_eq!(profile.fingerprints["xcf.version"].value, version);
            assert_eq!(
                profile.constants.len(),
                if version == "2.21" { 3 } else { 1 }
            );
            assert!(profile.capabilities.is_empty());
            assert_eq!(profile.inheritance_chain.last(), Some(&id));
            assert_eq!(profile.source_chain.len(), profile.inheritance_chain.len());
            assert_eq!(
                profile
                    .evidence
                    .iter()
                    .map(|value| value.value.as_str())
                    .collect::<Vec<_>>(),
                match version {
                    "2.17" => vec!["src/module_blob.rs", "src/mssql.rs"],
                    "2.20" | "2.21" => vec![
                        "src/module_blob.rs",
                        "src/mssql.rs",
                        "src/source.rs",
                        "tests/portable_root_smoke.rs"
                    ],
                    _ => unreachable!(),
                }
            );
        }

        for version in ["8.3.24.1819", "8.3.27.1989", "8.5.1.1150"] {
            let id = ProfileId::parse(&format!("platform-{version}")).unwrap();
            let profile = registry.get(&id).unwrap();
            assert_eq!(profile.status.value, ProfileStatus::Experimental);
            assert_eq!(
                profile.platform_build.as_ref().unwrap().value.to_string(),
                version
            );
            assert!(profile.xml_dialect.is_none());
            assert!(profile.compatibility_mode.is_none());
            if version == "8.3.27.1989" {
                assert_eq!(
                    profile.storage_profile.as_ref().unwrap().value.as_str(),
                    "storage:mssql-config-configsave"
                );
            } else {
                assert!(profile.storage_profile.is_none());
            }
            assert!(profile.container_revision.is_none());
            assert!(profile.dbms.is_none());
            assert!(profile.fingerprints.is_empty());
            if version == "8.3.27.1989" {
                assert_eq!(
                    profile.constants["bootstrap.metadata.functional_option.layout"].value,
                    "functional-option-v1-crlf-no-bom"
                );
                assert_eq!(
                    profile.constants["bootstrap.metadata.language.layout"].value,
                    "language-v1-crlf-no-bom"
                );
            } else {
                assert!(profile.constants.is_empty());
            }
            assert!(profile.capabilities.is_empty());
            assert_eq!(profile.inheritance_chain, [id]);
            assert_eq!(profile.source_chain.len(), 1);
        }

        assert!(
            registry
                .get(&ProfileId::parse("platform-8.3.24.1819").unwrap())
                .unwrap()
                .evidence
                .is_empty()
        );
        assert!(
            registry
                .get(&ProfileId::parse("platform-8.3.27.1989").unwrap())
                .unwrap()
                .evidence
                .iter()
                .any(|value| value.value == "docs/ssl-lab-2026-06-25.md")
        );
        assert!(
            registry
                .get(&ProfileId::parse("platform-8.5.1.1150").unwrap())
                .unwrap()
                .evidence
                .is_empty()
        );
    }

    #[test]
    fn bundled_detection_keeps_xml_and_platform_axes_independent() {
        let registry = load_bundled_profile_registry().unwrap();
        let xml_observations =
            DetectionObservations::try_new(None, None, [("xcf.version", "2.20")]).unwrap();
        let xml_result = detect_profiles(&registry, &xml_observations);
        let xml = require_exact_target(&xml_result).unwrap();
        assert_eq!(xml.id().as_str(), "xml-2.20");
        assert_eq!(
            xml.profile.xml_dialect.as_ref().unwrap().value.to_string(),
            "2.20"
        );
        assert!(xml.profile.platform_build.is_none());

        let platform_observations = DetectionObservations::try_new(
            Some("8.3.27.1989".parse().unwrap()),
            None,
            std::iter::empty::<(&str, &str)>(),
        )
        .unwrap();
        let platform_result = detect_profiles(&registry, &platform_observations);
        let platform = require_exact_target(&platform_result).unwrap();
        assert_eq!(platform.id().as_str(), "platform-8.3.27.1989");
        assert_eq!(
            platform
                .profile
                .platform_build
                .as_ref()
                .unwrap()
                .value
                .to_string(),
            "8.3.27.1989"
        );
        assert!(platform.profile.xml_dialect.is_none());
    }

    #[test]
    fn rejects_verified_external_profile() {
        let directory = TempDirectory::new("verified");
        directory.write(
            "profile.json",
            r#"{"schema_version":1,"id":"external","status":"verified"}"#,
        );

        let error = load_profile_registry(
            &[],
            Some(directory.path()),
            ProfileRegistryLimits::default(),
        )
        .unwrap_err();
        assert!(
            format!("{error:#}").contains("must explicitly declare experimental status"),
            "{error:#}"
        );
    }

    #[test]
    fn filesystem_creation_order_does_not_affect_registry() {
        let forward = TempDirectory::new("forward");
        forward.write("a.json", &experimental("a"));
        forward.write(
            "b.json",
            r#"{"schema_version":1,"id":"b","extends":"a","status":"experimental","constants":{"key":"value"}}"#,
        );

        let reverse = TempDirectory::new("reverse");
        reverse.write(
            "b.json",
            r#"{"schema_version":1,"id":"b","extends":"a","status":"experimental","constants":{"key":"value"}}"#,
        );
        reverse.write("a.json", &experimental("a"));

        let first =
            load_profile_registry(&[], Some(forward.path()), ProfileRegistryLimits::default())
                .unwrap();
        let second =
            load_profile_registry(&[], Some(reverse.path()), ProfileRegistryLimits::default())
                .unwrap();
        assert_eq!(first, second);
        let b = first.get(&ProfileId::parse("b").unwrap()).unwrap();
        assert_eq!(b.source_chain[0].source.name, "external/a.json");
        assert_eq!(b.source_chain[1].source.name, "external/b.json");
    }

    #[test]
    fn rejects_bundled_external_duplicate_id() {
        let directory = TempDirectory::new("duplicate");
        directory.write("external.json", &experimental("same"));
        let bundled_json = experimental("same");
        let bundled = [BundledProfile {
            name: "bundled/same.json",
            json: &bundled_json,
        }];

        let error = load_profile_registry(
            &bundled,
            Some(directory.path()),
            ProfileRegistryLimits::default(),
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("duplicate profile `same`"));
    }

    #[test]
    fn zero_external_file_limit_rejects_first_qualifying_file_before_parsing() {
        let directory = TempDirectory::new("zero-count");
        directory.write("ignored.JSON", "not JSON");
        directory.write("profile.json", "also not JSON");
        let limits = ProfileRegistryLimits {
            max_external_files: 0,
            ..ProfileRegistryLimits::default()
        };

        let error = load_profile_registry(&[], Some(directory.path()), limits).unwrap_err();
        assert_eq!(
            format!("{error:#}"),
            "external profile file count exceeds limit 0"
        );
    }

    #[test]
    fn external_file_limit_rejects_limit_plus_one_independent_of_directory_order() {
        let forward = TempDirectory::new("count-forward");
        forward.write("a.json", &experimental("a"));
        forward.write("b.json", &experimental("b"));
        forward.write("c.json", &experimental("c"));

        let reverse = TempDirectory::new("count-reverse");
        reverse.write("c.json", &experimental("c"));
        reverse.write("b.json", &experimental("b"));
        reverse.write("a.json", &experimental("a"));

        let limits = ProfileRegistryLimits {
            max_external_files: 2,
            ..ProfileRegistryLimits::default()
        };
        let forward_error = load_profile_registry(&[], Some(forward.path()), limits).unwrap_err();
        let reverse_error = load_profile_registry(&[], Some(reverse.path()), limits).unwrap_err();
        assert_eq!(
            format!("{forward_error:#}"),
            "external profile file count exceeds limit 2"
        );
        assert_eq!(
            format!("{reverse_error:#}"),
            "external profile file count exceeds limit 2"
        );
    }

    #[test]
    fn enforces_per_file_and_total_byte_limits() {
        let directory = TempDirectory::new("bytes");
        let a = experimental("a");
        let b = experimental("b");
        directory.write("a.json", &a);
        directory.write("b.json", &b);

        let per_file = ProfileRegistryLimits {
            max_external_file_bytes: u64::try_from(a.len() - 1).unwrap(),
            ..ProfileRegistryLimits::default()
        };
        let error = load_profile_registry(&[], Some(directory.path()), per_file).unwrap_err();
        assert!(format!("{error:#}").contains("per-file limit"));

        let total = ProfileRegistryLimits {
            max_total_external_bytes: u64::try_from(a.len() + b.len() - 1).unwrap(),
            ..ProfileRegistryLimits::default()
        };
        let error = load_profile_registry(&[], Some(directory.path()), total).unwrap_err();
        assert!(format!("{error:#}").contains("total size"));
    }

    #[test]
    fn ignores_non_json_files_and_subdirectories() {
        let directory = TempDirectory::new("filter");
        directory.write("profile.json", &experimental("profile"));
        directory.write("notes.txt", "not json");
        fs::create_dir(directory.path().join("nested.json")).unwrap();

        let registry = load_profile_registry(
            &[],
            Some(directory.path()),
            ProfileRegistryLimits::default(),
        )
        .unwrap();
        assert_eq!(registry.profiles().len(), 1);
    }
}
