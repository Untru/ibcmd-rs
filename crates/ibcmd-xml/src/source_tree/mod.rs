//! A bounded, portable inventory of a 1C XML source tree.
mod reader;
mod writer;

use ibcmd_core::identity::ObjectUuid;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

pub use reader::{ReaderLimits, SourceTreeReader, read_source_tree};
pub use writer::{SourceTreeWriter, publish_new};

pub const MAX_SOURCE_FILES: usize = 65_536;
pub const MAX_SOURCE_DIRECTORIES: usize = 65_536;
pub const MAX_SOURCE_DEPTH: usize = 64;
pub const MAX_SOURCE_COMPONENT_BYTES: usize = 255;
pub const MAX_SOURCE_PATH_BYTES: usize = 4_096;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourcePath(Box<str>);
impl SourcePath {
    pub fn new(value: impl AsRef<str>) -> Result<Self, SourceTreeError> {
        let value = value.as_ref();
        if value.starts_with("\\\\")
            || value.starts_with('/')
            || value.as_bytes().get(1) == Some(&b':')
        {
            return Err(SourceTreeError::UnsafePath(value.to_string()));
        }
        let value = value.replace('\\', "/");
        if value.is_empty() || value.len() > MAX_SOURCE_PATH_BYTES {
            return Err(SourceTreeError::UnsafePath(value));
        }
        let parts: Vec<_> = value.split('/').collect();
        if parts.len() > MAX_SOURCE_DEPTH
            || parts.iter().any(|p| {
                p.is_empty()
                    || *p == "."
                    || *p == ".."
                    || p.len() > MAX_SOURCE_COMPONENT_BYTES
                    || p.ends_with(['.', ' '])
                    || p.chars().any(|c| {
                        c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
                    })
                    || reserved(p)
                    || matches!(*p, ".git" | "target" | ".idea" | ".vscode")
            })
        {
            return Err(SourceTreeError::UnsafePath(value));
        }
        Ok(Self(value.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Display for SourcePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
fn reserved(s: &str) -> bool {
    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceKind {
    ConfigurationRoot,
    MetadataXml,
    Module,
    Form,
    Template,
    Binary,
    OtherXml,
    Other,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEntry {
    path: SourcePath,
    kind: SourceKind,
    bytes: Box<[u8]>,
    uuid: Option<ObjectUuid>,
    digest: ibcmd_core::storage::Sha256Digest,
}
impl SourceEntry {
    pub fn from_bytes(path: SourcePath, bytes: Vec<u8>) -> Result<Self, SourceTreeError> {
        let mut kind = reader::classify(path.as_str());
        let document = if path.as_str().to_ascii_lowercase().ends_with(".xml") {
            Some(
                crate::XmlReader::from_slice(&bytes).map_err(|e| SourceTreeError::Xml {
                    path: path.clone(),
                    message: e.to_string(),
                })?,
            )
        } else {
            None
        };
        if matches!(kind, SourceKind::OtherXml)
            && document.as_ref().is_some_and(|d| {
                matches!(
                    d.root().name().local(),
                    "MetaDataObject" | "Configuration" | "DefinedType"
                )
            })
        {
            kind = SourceKind::MetadataXml;
        }
        let uuid = if matches!(
            kind,
            SourceKind::ConfigurationRoot | SourceKind::MetadataXml
        ) {
            reader::derive_uuid(&path, document.as_ref().unwrap())?
        } else {
            None
        };
        Self::new(path, kind, bytes, uuid)
    }
    pub(crate) fn new(
        path: SourcePath,
        kind: SourceKind,
        bytes: Vec<u8>,
        uuid: Option<ObjectUuid>,
    ) -> Result<Self, SourceTreeError> {
        if bytes.len() > ibcmd_core::asset::MAX_ASSET_BYTES {
            return Err(SourceTreeError::AssetTooLarge {
                path,
                actual: bytes.len(),
            });
        }
        Ok(Self {
            path,
            kind,
            digest: ibcmd_core::storage::Sha256Digest::for_bytes(&bytes),
            bytes: bytes.into(),
            uuid,
        })
    }
    pub fn path(&self) -> &SourcePath {
        &self.path
    }
    pub fn kind(&self) -> SourceKind {
        self.kind
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn uuid(&self) -> Option<ObjectUuid> {
        self.uuid
    }
    pub const fn digest(&self) -> ibcmd_core::storage::Sha256Digest {
        self.digest
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceTree {
    entries: Vec<SourceEntry>,
}
impl SourceTree {
    pub fn new(mut entries: Vec<SourceEntry>) -> Result<Self, SourceTreeError> {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let tree = Self { entries };
        tree.validate()?;
        Ok(tree)
    }
    pub fn entries(&self) -> &[SourceEntry] {
        &self.entries
    }
    pub fn validate(&self) -> Result<(), SourceTreeError> {
        if self.entries.len() > MAX_SOURCE_FILES {
            return Err(SourceTreeError::TooManyFiles);
        }
        count_directories(&self.entries, MAX_SOURCE_DIRECTORIES)?;
        let mut folded = BTreeMap::new();
        let mut uuids = BTreeMap::new();
        let mut total = 0usize;
        for e in &self.entries {
            total = total
                .checked_add(e.bytes.len())
                .ok_or(SourceTreeError::TotalTooLarge)?;
            if total > ibcmd_core::model::MAX_CONFIGURATION_RETAINED_BYTES {
                return Err(SourceTreeError::TotalTooLarge);
            }
            let fold = e
                .path
                .as_str()
                .chars()
                .flat_map(char::to_lowercase)
                .collect::<String>();
            if let Some(old) = folded.insert(fold, e.path.clone()) {
                return Err(SourceTreeError::PathConflict {
                    first: old,
                    second: e.path.clone(),
                });
            }
        }
        for e in &self.entries {
            for p in e
                .path
                .as_str()
                .match_indices('/')
                .map(|(i, _)| &e.path.as_str()[..i])
            {
                let parent_fold = p.chars().flat_map(char::to_lowercase).collect::<String>();
                if let Some(previous) = folded.get(&parent_fold) {
                    return Err(SourceTreeError::PathConflict {
                        first: previous.clone(),
                        second: e.path.clone(),
                    });
                }
            }
        }
        for e in &self.entries {
            if let Some(u) = e.uuid
                && let Some(old) = uuids.insert(u, e.path.clone())
            {
                return Err(SourceTreeError::UuidConflict {
                    uuid: u,
                    first: old,
                    second: e.path.clone(),
                });
            }
        }
        Ok(())
    }
}
fn count_directories(entries: &[SourceEntry], maximum: usize) -> Result<(), SourceTreeError> {
    let mut directories = BTreeMap::<String, String>::new();
    directories.insert(String::new(), String::new());
    for entry in entries {
        for (index, _) in entry.path.as_str().match_indices('/') {
            let raw = entry.path.as_str()[..index].to_owned();
            let fold = raw.chars().flat_map(char::to_lowercase).collect::<String>();
            if let Some(old) = directories.insert(fold, raw.clone())
                && old != raw
            {
                return Err(SourceTreeError::PathConflict {
                    first: SourcePath(old.into()),
                    second: SourcePath(raw.into()),
                });
            }
            if directories.len() > maximum {
                return Err(SourceTreeError::TooManyDirectories);
            }
        }
    }
    if directories.len() > maximum {
        Err(SourceTreeError::TooManyDirectories)
    } else {
        Ok(())
    }
}
#[derive(Debug)]
pub enum SourceTreeError {
    UnsafePath(String),
    AssetTooLarge {
        path: SourcePath,
        actual: usize,
    },
    TotalTooLarge,
    TooManyFiles,
    PathConflict {
        first: SourcePath,
        second: SourcePath,
    },
    UuidConflict {
        uuid: ObjectUuid,
        first: SourcePath,
        second: SourcePath,
    },
    Io(std::io::Error),
    Xml {
        path: SourcePath,
        message: String,
    },
    ExistingDestination,
    TooManyDirectories,
    DepthExceeded,
    InvalidUuid {
        path: SourcePath,
    },
    AmbiguousUuid {
        path: SourcePath,
    },
    InvalidLimits,
    TemporaryNameExhausted,
}
impl Display for SourceTreeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafePath(p) => write!(f, "unsafe source path: {p}"),
            Self::AssetTooLarge { path, actual } => write!(f, "asset {path} too large: {actual}"),
            Self::TotalTooLarge => f.write_str("source tree exceeds retained-byte limit"),
            Self::TooManyFiles => f.write_str("source tree has too many files"),
            Self::PathConflict { first, second } => {
                write!(f, "conflicting paths: {first} and {second}")
            }
            Self::UuidConflict {
                uuid,
                first,
                second,
            } => {
                write!(f, "duplicate UUID {uuid} at {first} and {second}")
            }
            Self::Io(e) => e.fmt(f),
            Self::Xml { path, message } => write!(f, "invalid XML at {path}: {message}"),
            Self::ExistingDestination => f.write_str("destination already exists"),
            Self::TooManyDirectories => f.write_str("source tree has too many directories"),
            Self::DepthExceeded => f.write_str("source tree depth exceeds limit"),
            Self::InvalidUuid { path } => write!(f, "invalid UUID at {path}"),
            Self::AmbiguousUuid { path } => write!(f, "ambiguous UUID at {path}"),
            Self::InvalidLimits => f.write_str("invalid source tree limits"),
            Self::TemporaryNameExhausted => f.write_str("no temporary source tree name available"),
        }
    }
}
impl Error for SourceTreeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}
impl From<std::io::Error> for SourceTreeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "ibcmd-xml-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn file(&self, p: &str, b: &[u8]) {
            let x = self.0.join(p);
            if let Some(d) = x.parent() {
                fs::create_dir_all(d).unwrap()
            }
            fs::write(x, b).unwrap()
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn paths_normalize_and_reject_windows_hazards() {
        assert_eq!(SourcePath::new("A\\B.xml").unwrap().as_str(), "A/B.xml");
        for path in [
            "..\\x",
            "/absolute",
            "C:\\x",
            "\\\\server\\x",
            "\\\\?\\x",
            "CON",
            "COM1.txt",
            "a//b",
            "a/../b",
            "a.",
            "a ",
            ".git/a",
            "a/.idea/x",
            "a/.vscode/x",
        ] {
            assert!(SourcePath::new(path).is_err(), "{path}");
        }
    }
    #[test]
    fn entries_have_digest_and_tree_rejects_case_parent_and_uuid_conflicts() {
        let p = SourcePath::new("abc").unwrap();
        let e = SourceEntry::new(p.clone(), SourceKind::Other, b"abc".to_vec(), None).unwrap();
        assert_eq!(
            e.digest().to_string(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let a = SourceEntry::new(
            SourcePath::new("A").unwrap(),
            SourceKind::Other,
            vec![],
            None,
        )
        .unwrap();
        let b = SourceEntry::new(
            SourcePath::new("a/b").unwrap(),
            SourceKind::Other,
            vec![],
            None,
        )
        .unwrap();
        assert!(matches!(
            SourceTree::new(vec![a, b]),
            Err(SourceTreeError::PathConflict { .. })
        ));
        let u = ObjectUuid::parse("12345678-90ab-cdef-0123-456789abcdef").unwrap();
        let a = SourceEntry::new(
            SourcePath::new("a").unwrap(),
            SourceKind::Other,
            vec![],
            Some(u),
        )
        .unwrap();
        let b = SourceEntry::new(
            SourcePath::new("b").unwrap(),
            SourceKind::Other,
            vec![],
            Some(u),
        )
        .unwrap();
        assert!(matches!(
            SourceTree::new(vec![a, b]),
            Err(SourceTreeError::UuidConflict { .. })
        ));
    }
    #[test]
    fn path_bounds_and_ignored_components() {
        assert!(SourcePath::new("target/a").is_err());
        assert!(SourcePath::new("a\u{1}").is_err());
        assert!(
            SourcePath::new("a/".to_owned() + &"x".repeat(MAX_SOURCE_COMPONENT_BYTES + 1)).is_err()
        );
        assert!(
            SourcePath::new(
                (0..MAX_SOURCE_DEPTH + 1)
                    .map(|_| "a")
                    .collect::<Vec<_>>()
                    .join("/")
            )
            .is_err()
        );
        assert!(
            SourcePath::new(
                (0..21)
                    .map(|_| "x".repeat(200))
                    .collect::<Vec<_>>()
                    .join("/")
            )
            .is_err()
        );
    }
    #[test]
    fn exact_and_case_duplicates() {
        for pair in [("x", "x"), ("X", "x")] {
            let a = SourceEntry::new(
                SourcePath::new(pair.0).unwrap(),
                SourceKind::Other,
                vec![],
                None,
            )
            .unwrap();
            let b = SourceEntry::new(
                SourcePath::new(pair.1).unwrap(),
                SourceKind::Other,
                vec![],
                None,
            )
            .unwrap();
            assert!(matches!(
                SourceTree::new(vec![a, b]),
                Err(SourceTreeError::PathConflict { .. })
            ));
        }
    }
    #[test]
    fn parent_conflict_both_orders() {
        for pair in [("A", "a/b"), ("a/b", "A")] {
            let a = SourceEntry::new(
                SourcePath::new(pair.0).unwrap(),
                SourceKind::Other,
                vec![],
                None,
            )
            .unwrap();
            let b = SourceEntry::new(
                SourcePath::new(pair.1).unwrap(),
                SourceKind::Other,
                vec![],
                None,
            )
            .unwrap();
            assert!(SourceTree::new(vec![a, b]).is_err());
        }
    }
    #[test]
    fn reader_inventory_and_sorting() {
        let t = Temp::new();
        t.file(
            "Configuration.xml",
            b"<Configuration uuid='12345678-90ab-cdef-0123-456789abcdef'/>",
        );
        t.file("Catalogs/A.xml", b"<MetaDataObject/>");
        t.file("ChartsOfCalculationRegisters/B.xml", b"<MetaDataObject/>");
        t.file("Catalogs/A/Module.bsl", b"x");
        t.file("Catalogs/A/Forms/F.xml", b"<f/>");
        t.file("Templates/T.mxl", b"z");
        t.file("Templates/T/Ext/Template.xml", b"<document/>");
        t.file("x.png", b"\0x");
        let tree = read_source_tree(&t.0).unwrap();
        let inventory = tree
            .entries()
            .iter()
            .map(|e| (e.path().as_str(), e.kind(), e.bytes()))
            .collect::<Vec<_>>();
        assert_eq!(
            inventory,
            vec![
                (
                    "Catalogs/A.xml",
                    SourceKind::MetadataXml,
                    b"<MetaDataObject/>".as_slice()
                ),
                (
                    "Catalogs/A/Forms/F.xml",
                    SourceKind::Form,
                    b"<f/>".as_slice()
                ),
                ("Catalogs/A/Module.bsl", SourceKind::Module, b"x".as_slice()),
                (
                    "ChartsOfCalculationRegisters/B.xml",
                    SourceKind::MetadataXml,
                    b"<MetaDataObject/>".as_slice(),
                ),
                (
                    "Configuration.xml",
                    SourceKind::ConfigurationRoot,
                    b"<Configuration uuid='12345678-90ab-cdef-0123-456789abcdef'/>".as_slice(),
                ),
                ("Templates/T.mxl", SourceKind::Template, b"z".as_slice()),
                (
                    "Templates/T/Ext/Template.xml",
                    SourceKind::Template,
                    b"<document/>".as_slice(),
                ),
                ("x.png", SourceKind::Binary, b"\0x".as_slice()),
            ]
        );
        assert_eq!(
            tree.entries()
                .iter()
                .find(|entry| entry.path().as_str() == "Configuration.xml")
                .and_then(SourceEntry::uuid)
                .unwrap()
                .to_string(),
            "12345678-90ab-cdef-0123-456789abcdef"
        );
        for entry in tree.entries() {
            assert_eq!(
                entry.digest(),
                ibcmd_core::storage::Sha256Digest::for_bytes(entry.bytes())
            );
        }
    }
    #[test]
    fn metadata_ext_subfiles_include_root_and_nested_ext() {
        for path in ["Ext/Help.xml", "Catalogs/A/Ext/Help.xml"] {
            let entry =
                SourceEntry::from_bytes(SourcePath::new(path).unwrap(), b"<Help/>".to_vec())
                    .unwrap();
            assert_eq!(entry.kind(), SourceKind::MetadataXml, "{path}");
        }
    }
    #[test]
    fn uuid_errors_and_root_wins() {
        let p = SourcePath::new("Configuration.xml").unwrap();
        assert!(matches!(
            SourceEntry::from_bytes(p.clone(), b"<Configuration uuid='bad'/>".to_vec()),
            Err(SourceTreeError::InvalidUuid { .. })
        ));
        assert!(matches!(SourceEntry::from_bytes(p.clone(),b"<Configuration><A uuid='12345678-90ab-cdef-0123-456789abcdef'/><B uuid='22345678-90ab-cdef-0123-456789abcdef'/></Configuration>".to_vec()),Err(SourceTreeError::AmbiguousUuid{..})));
        let e=SourceEntry::from_bytes(p,b"<Configuration uuid='12345678-90ab-cdef-0123-456789abcdef'><A uuid='22345678-90ab-cdef-0123-456789abcdef'/></Configuration>".to_vec()).unwrap();
        assert_eq!(
            e.uuid().unwrap().to_string(),
            "12345678-90ab-cdef-0123-456789abcdef"
        );
    }
    #[test]
    fn public_entries_report_duplicate_uuid_paths_before_writing() {
        let uuid = ObjectUuid::parse("12345678-90ab-cdef-0123-456789abcdef").unwrap();
        let first_path = SourcePath::new("Catalogs/A.xml").unwrap();
        let second_path = SourcePath::new("Catalogs/B.xml").unwrap();
        let first = SourceEntry::from_bytes(
            first_path.clone(),
            b"<MetaDataObject uuid='12345678-90ab-cdef-0123-456789abcdef'/>".to_vec(),
        )
        .unwrap();
        let second = SourceEntry::from_bytes(
            second_path.clone(),
            b"<MetaDataObject uuid='12345678-90ab-cdef-0123-456789abcdef'/>".to_vec(),
        )
        .unwrap();
        assert!(matches!(
            SourceTree::new(vec![second, first]),
            Err(SourceTreeError::UuidConflict {
                uuid: actual,
                first,
                second,
            }) if actual == uuid && first == first_path && second == second_path
        ));
    }
    #[test]
    fn malformed_xml_is_preflight_error() {
        assert!(matches!(
            SourceEntry::from_bytes(SourcePath::new("x.xml").unwrap(), b"<x>".to_vec()),
            Err(SourceTreeError::Xml { .. })
        ));
    }
    #[test]
    fn reader_limits_branches() {
        let t = Temp::new();
        t.file("a", b"123");
        assert!(matches!(
            SourceTreeReader::new(ReaderLimits {
                files: 0,
                ..ReaderLimits::default()
            }),
            Err(SourceTreeError::InvalidLimits)
        ));
        assert!(matches!(
            SourceTreeReader::new(ReaderLimits {
                files: 1,
                directories: 1,
                depth: 1,
                asset_bytes: 2,
                total_bytes: 10
            })
            .unwrap()
            .read(&t.0),
            Err(SourceTreeError::AssetTooLarge { .. })
        ));
        assert!(matches!(
            SourceTreeReader::new(ReaderLimits {
                files: 1,
                directories: 1,
                depth: 1,
                asset_bytes: 10,
                total_bytes: 2
            })
            .unwrap()
            .read(&t.0),
            Err(SourceTreeError::TotalTooLarge)
        ));
    }
    #[test]
    fn reader_file_dir_depth_limits() {
        let t = Temp::new();
        t.file("a", b"");
        t.file("d/e/f", b"");
        assert!(matches!(
            SourceTreeReader::new(ReaderLimits {
                files: 0,
                ..ReaderLimits::default()
            }),
            Err(SourceTreeError::InvalidLimits)
        ));
        assert!(matches!(
            SourceTreeReader::new(ReaderLimits {
                files: 1,
                directories: 9,
                depth: 9,
                asset_bytes: 9,
                total_bytes: 9
            })
            .unwrap()
            .read(&t.0),
            Err(SourceTreeError::TooManyFiles)
        ));
        assert!(matches!(
            SourceTreeReader::new(ReaderLimits {
                files: 9,
                directories: 1,
                depth: 9,
                asset_bytes: 9,
                total_bytes: 9
            })
            .unwrap()
            .read(&t.0),
            Err(SourceTreeError::TooManyDirectories)
        ));
        assert!(matches!(
            SourceTreeReader::new(ReaderLimits {
                files: 9,
                directories: 9,
                depth: 1,
                asset_bytes: 9,
                total_bytes: 9
            })
            .unwrap()
            .read(&t.0),
            Err(SourceTreeError::DepthExceeded)
        ));
    }
    #[test]
    fn publish_round_trip() {
        let t = Temp::new();
        t.file("Configuration.xml", b"<Configuration/>");
        t.file("a.bin", b"abc");
        let a = read_source_tree(&t.0).unwrap();
        let d = t.0.join("out");
        publish_new(&a, &d).unwrap();
        assert_eq!(read_source_tree(d).unwrap(), a);
    }
    #[test]
    fn existing_destination_is_unchanged_and_no_temp() {
        let t = Temp::new();
        let e = SourceTree::new(vec![
            SourceEntry::from_bytes(SourcePath::new("a").unwrap(), vec![1]).unwrap(),
        ])
        .unwrap();
        let d = t.0.join("out");
        fs::write(&d, b"sentinel").unwrap();
        assert!(matches!(
            publish_new(&e, &d),
            Err(SourceTreeError::ExistingDestination)
        ));
        assert_eq!(fs::read(&d).unwrap(), b"sentinel");
        assert!(!fs::read_dir(&t.0).unwrap().any(|x| {
            x.unwrap()
                .file_name()
                .to_string_lossy()
                .contains("ibcmd-new")
        }));
    }
    #[test]
    fn writer_io_error_does_not_leave_a_staging_directory() {
        let t = Temp::new();
        let tree = SourceTree::new(vec![
            SourceEntry::from_bytes(SourcePath::new("a.bin").unwrap(), vec![1]).unwrap(),
        ])
        .unwrap();
        let destination = t.0.join("missing-parent/out");
        assert!(matches!(
            publish_new(&tree, &destination),
            Err(SourceTreeError::Io(_))
        ));
        assert_eq!(fs::read_dir(&t.0).unwrap().count(), 0);
    }
    #[cfg(unix)]
    #[test]
    fn reader_rejects_file_and_directory_symlinks() {
        use std::os::unix::fs::symlink;
        let file_tree = Temp::new();
        file_tree.file("a", b"");
        symlink(file_tree.0.join("a"), file_tree.0.join("link")).unwrap();
        assert!(matches!(
            read_source_tree(&file_tree.0),
            Err(SourceTreeError::UnsafePath(_))
        ));

        let directory_tree = Temp::new();
        fs::create_dir(directory_tree.0.join("directory")).unwrap();
        symlink(
            directory_tree.0.join("directory"),
            directory_tree.0.join("link"),
        )
        .unwrap();
        assert!(matches!(
            read_source_tree(&directory_tree.0),
            Err(SourceTreeError::UnsafePath(_))
        ));
    }
    #[test]
    fn implicit_directories_are_preflight_bounded() {
        let e = SourceEntry::new(
            SourcePath::new("a/b/c").unwrap(),
            SourceKind::Other,
            vec![],
            None,
        )
        .unwrap();
        assert!(matches!(
            count_directories(&[e], 2),
            Err(SourceTreeError::TooManyDirectories)
        ));
    }
    #[test]
    fn implicit_directory_case_aliases_conflict_in_both_orders() {
        for (a, b) in [("A/x", "a/y"), ("a/y", "A/x")] {
            let x = SourceEntry::new(SourcePath::new(a).unwrap(), SourceKind::Other, vec![], None)
                .unwrap();
            let y = SourceEntry::new(SourcePath::new(b).unwrap(), SourceKind::Other, vec![], None)
                .unwrap();
            assert!(matches!(
                SourceTree::new(vec![x, y]),
                Err(SourceTreeError::PathConflict { .. })
            ));
        }
    }
}
