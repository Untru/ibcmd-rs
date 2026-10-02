use super::*;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub struct ReaderLimits {
    pub files: usize,
    pub directories: usize,
    pub depth: usize,
    pub asset_bytes: usize,
    pub total_bytes: usize,
}
impl Default for ReaderLimits {
    fn default() -> Self {
        Self {
            files: 65_536,
            directories: 65_536,
            depth: MAX_SOURCE_DEPTH,
            asset_bytes: ibcmd_core::asset::MAX_ASSET_BYTES,
            total_bytes: ibcmd_core::model::MAX_CONFIGURATION_RETAINED_BYTES,
        }
    }
}
impl ReaderLimits {
    pub fn validate(self) -> Result<Self, SourceTreeError> {
        if self.files == 0
            || self.directories == 0
            || self.depth == 0
            || self.asset_bytes == 0
            || self.total_bytes == 0
        {
            return Err(SourceTreeError::InvalidLimits);
        }
        if self.files > MAX_SOURCE_FILES
            || self.directories > MAX_SOURCE_DIRECTORIES
            || self.depth > MAX_SOURCE_DEPTH
            || self.asset_bytes > MAX_SOURCE_FILE_BYTES
            || self.total_bytes > MAX_SOURCE_RETAINED_BYTES
        {
            return Err(SourceTreeError::InvalidLimits);
        }
        Ok(self)
    }
}
#[derive(Default)]
pub struct SourceTreeReader {
    limits: ReaderLimits,
}
impl SourceTreeReader {
    pub fn new(limits: ReaderLimits) -> Result<Self, SourceTreeError> {
        Ok(Self {
            limits: limits.validate()?,
        })
    }
    pub fn read(&self, root: impl AsRef<Path>) -> Result<SourceTree, SourceTreeError> {
        read_with_limits(root, self.limits)
    }
}
pub fn read_source_tree(root: impl AsRef<Path>) -> Result<SourceTree, SourceTreeError> {
    read_with_limits(root, ReaderLimits::default())
}
fn read_with_limits(
    root: impl AsRef<Path>,
    limits: ReaderLimits,
) -> Result<SourceTree, SourceTreeError> {
    let mut entries = Vec::new();
    walk_with_limits(root.as_ref(), limits, &mut |entry| {
        entries.push(entry);
        Ok(())
    })?;
    SourceTree::new(entries)
}

// Validate a staged tree one file at a time. Re-reading the complete inventory
// would retain a second copy of every payload just before publication.
pub(super) fn verify_with_limits(
    root: &Path,
    expected: &SourceTree,
    limits: ReaderLimits,
) -> Result<(), SourceTreeError> {
    let mut remaining = expected
        .entries()
        .iter()
        .map(|entry| (entry.path().as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let mismatch = || SourceTreeError::PathConflict {
        first: SourcePath::new("staging").expect("fixed safe path"),
        second: SourcePath::new("tree").expect("fixed safe path"),
    };
    walk_with_limits(root, limits, &mut |entry| {
        let original = remaining
            .remove(entry.path().as_str())
            .ok_or_else(mismatch)?;
        if &entry != original {
            return Err(mismatch());
        }
        Ok(())
    })?;
    if !remaining.is_empty() {
        return Err(mismatch());
    }
    Ok(())
}

fn walk_with_limits(
    root: &Path,
    limits: ReaderLimits,
    accept: &mut impl FnMut(SourceEntry) -> Result<(), SourceTreeError>,
) -> Result<(), SourceTreeError> {
    let m = fs::symlink_metadata(root)?;
    if !m.file_type().is_dir() || m.file_type().is_symlink() {
        return Err(SourceTreeError::UnsafePath(root.display().to_string()));
    }
    let mut state = State {
        limits,
        total: 0,
        dirs: 1,
        files: 0,
    };
    visit(root, root, 0, &mut state, accept)
}
struct State {
    limits: ReaderLimits,
    total: usize,
    dirs: usize,
    files: usize,
}
fn visit(
    root: &Path,
    dir: &Path,
    depth: usize,
    s: &mut State,
    accept: &mut impl FnMut(SourceEntry) -> Result<(), SourceTreeError>,
) -> Result<(), SourceTreeError> {
    if depth > s.limits.depth {
        return Err(SourceTreeError::DepthExceeded);
    }
    let mut es = Vec::new();
    for item in fs::read_dir(dir)? {
        let e = item?;
        let ty = e.file_type()?;
        let n = e.file_name();
        let n = n
            .to_str()
            .ok_or_else(|| SourceTreeError::UnsafePath("non-UTF8 filename".into()))?;
        if n.contains('\\') {
            return Err(SourceTreeError::UnsafePath(n.into()));
        }
        let entry_path = e.path();
        let parent_resource = entry_path
            .strip_prefix(root)
            .ok()
            .and_then(|p| p.to_str())
            .is_some_and(|p| parent_configuration_resource(&p.replace('\\', "/")));
        if !parent_resource && matches!(n, ".git" | "target" | ".idea" | ".vscode") {
            continue;
        }
        if ty.is_symlink() || (!ty.is_file() && !ty.is_dir()) {
            return Err(SourceTreeError::UnsafePath(e.path().display().to_string()));
        }
        if ty.is_dir() {
            s.dirs += 1;
            if s.dirs > s.limits.directories {
                return Err(SourceTreeError::TooManyDirectories);
            }
        } else {
            s.files += 1;
            if s.files > s.limits.files {
                return Err(SourceTreeError::TooManyFiles);
            }
        }
        es.push((e, ty));
    }
    es.sort_by_key(|(entry, _)| entry.file_name());
    for (e, ty) in es {
        if ty.is_dir() {
            visit(root, &e.path(), depth + 1, s, accept)?
        } else {
            let entry_path = e.path();
            let relative = entry_path
                .strip_prefix(root)
                .map_err(|_| SourceTreeError::UnsafePath("outside root".into()))?;
            let parts = relative
                .components()
                .map(|x| {
                    x.as_os_str()
                        .to_str()
                        .ok_or_else(|| SourceTreeError::UnsafePath("non-UTF8".into()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let path = SourcePath::new(parts.join("/"))?;
            let announced = usize::try_from(e.metadata()?.len()).map_err(|_| {
                SourceTreeError::AssetTooLarge {
                    path: path.clone(),
                    actual: usize::MAX,
                }
            })?;
            if announced > s.limits.asset_bytes {
                return Err(SourceTreeError::AssetTooLarge {
                    path,
                    actual: announced,
                });
            }
            if s.total
                .checked_add(announced)
                .filter(|x| *x <= s.limits.total_bytes)
                .is_none()
            {
                return Err(SourceTreeError::TotalTooLarge);
            }
            let remaining = s.limits.total_bytes - s.total;
            let read_limit = s.limits.asset_bytes.min(remaining);
            let f = File::open(e.path())?;
            let mut bytes = Vec::with_capacity(announced.min(read_limit));
            f.take((read_limit + 1) as u64).read_to_end(&mut bytes)?;
            if bytes.len() > s.limits.asset_bytes {
                return Err(SourceTreeError::AssetTooLarge {
                    path,
                    actual: bytes.len(),
                });
            }
            if bytes.len() > remaining {
                return Err(SourceTreeError::TotalTooLarge);
            }
            s.total = s
                .total
                .checked_add(bytes.len())
                .ok_or(SourceTreeError::TotalTooLarge)?;
            if s.total > s.limits.total_bytes {
                return Err(SourceTreeError::TotalTooLarge);
            }
            // Use the same streaming body validation and descriptor identity
            // rules for both disk inventories and entries built by adapters.
            accept(SourceEntry::from_bytes(path, bytes)?)?;
        }
    }
    Ok(())
}
pub(crate) fn parent_configuration_resource(path: &str) -> bool {
    let path = path.strip_prefix(".ibcmd-provenance/xml/").unwrap_or(path);
    [
        "Ext/ParentConfigurations/",
        "Configuration/ParentConfigurations/",
        "src/Configuration/ParentConfigurations/",
    ]
    .iter()
    .any(|prefix| {
        path.strip_prefix(prefix)
            .is_some_and(|relative| !relative.is_empty())
    })
}

pub(crate) fn classify(p: &str) -> SourceKind {
    if parent_configuration_resource(p) {
        return SourceKind::Binary;
    }
    let l = p.to_ascii_lowercase();
    let e = l.rsplit('.').next().unwrap_or("");
    if l == "configuration.xml" {
        SourceKind::ConfigurationRoot
    } else if e == "bsl" {
        SourceKind::Module
    } else if l.contains("/forms/") || l.ends_with("/form.xml") {
        SourceKind::Form
    } else if l.contains("/templates/") || l.ends_with("/template.xml") || e == "mxl" {
        SourceKind::Template
    } else if e == "xml" {
        if l.starts_with("ext/")
            || l.contains("/ext/")
            || l.split('/').any(|x| {
                matches!(
                    x,
                    "catalogs"
                        | "documents"
                        | "informationregisters"
                        | "accumulationregisters"
                        | "accountingregisters"
                        | "calculationregisters"
                        | "chartsofcharacteristictypes"
                        | "chartsofaccounts"
                        | "chartsofcalculationtypes"
                        | "chartsofcalculationregisters"
                        | "commonmodules"
                        | "commonforms"
                        | "commonpictures"
                        | "commontemplates"
                        | "commonattributes"
                        | "commandgroups"
                        | "documentjournals"
                        | "reports"
                        | "dataprocessors"
                        | "enums"
                        | "exchangeplans"
                        | "eventsubscriptions"
                        | "filtercriteria"
                        | "functionaloptions"
                        | "functionaloptionsparameters"
                        | "httpservices"
                        | "languages"
                        | "scheduledjobs"
                        | "sessionparameters"
                        | "settingsstorages"
                        | "styleitems"
                        | "styles"
                        | "subsystems"
                        | "roles"
                        | "commoncommands"
                        | "businessprocesses"
                        | "bots"
                        | "definedtypes"
                        | "tasks"
                        | "constants"
                        | "documentnumerators"
                        | "integrationservices"
                        | "sequences"
                        | "webservices"
                        | "wsreferences"
                        | "xdtopackages"
                )
            })
        {
            SourceKind::MetadataXml
        } else {
            SourceKind::OtherXml
        }
    } else if matches!(
        e,
        "bin" | "png" | "jpg" | "jpeg" | "gif" | "ico" | "svg" | "zip"
    ) {
        SourceKind::Binary
    } else {
        SourceKind::Other
    }
}
// Complete grammar is already checked by inspect_slice. Identity uses only
// root/direct-child attributes, so it never needs a body DOM for large Rights
// or other source sidecars classified alongside metadata descriptors.
pub(crate) fn derive_uuid_from_bytes(
    path: &SourcePath,
    bytes: &[u8],
) -> Result<Option<ObjectUuid>, SourceTreeError> {
    derive_uuid_from_reader(path, bytes)
}

pub(crate) fn derive_uuid_from_reader<R: std::io::BufRead>(
    path: &SourcePath,
    input: R,
) -> Result<Option<ObjectUuid>, SourceTreeError> {
    use quick_xml::{Reader, events::Event};
    let mut reader = Reader::from_reader(input);
    let mut buffer = Vec::new();
    let mut depth = 0usize;
    let mut count = 0usize;
    let mut candidate = None;
    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|e| SourceTreeError::Xml {
                path: path.clone(),
                message: e.to_string(),
            })?;
        match event {
            Event::Start(ref e) | Event::Empty(ref e) => {
                if depth <= 1 {
                    for a in e.attributes() {
                        let a = a.map_err(|e| SourceTreeError::Xml {
                            path: path.clone(),
                            message: e.to_string(),
                        })?;
                        let key = a.key.as_ref();
                        if key == b"xmlns" || key.starts_with(b"xmlns:") {
                            continue;
                        }
                        if key
                            .rsplit(|b| *b == b':')
                            .next()
                            .unwrap_or(key)
                            .eq_ignore_ascii_case(b"uuid")
                        {
                            let value =
                                a.decode_and_unescape_value(reader.decoder()).map_err(|e| {
                                    SourceTreeError::Xml {
                                        path: path.clone(),
                                        message: e.to_string(),
                                    }
                                })?;
                            let uuid = ObjectUuid::parse(&value)
                                .map_err(|_| SourceTreeError::InvalidUuid { path: path.clone() })?;
                            count += 1;
                            candidate = Some(uuid);
                        }
                    }
                    if depth == 0 && count > 0 {
                        return if count == 1 {
                            Ok(candidate)
                        } else {
                            Err(SourceTreeError::AmbiguousUuid { path: path.clone() })
                        };
                    }
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => depth -= 1,
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if count > 1 {
        Err(SourceTreeError::AmbiguousUuid { path: path.clone() })
    } else {
        Ok(candidate)
    }
}

#[cfg(test)]
pub(crate) fn derive_uuid(
    path: &SourcePath,
    d: &crate::XmlDocument,
) -> Result<Option<ObjectUuid>, SourceTreeError> {
    let collect = |e: &crate::XmlElement| -> Result<Vec<ObjectUuid>, SourceTreeError> {
        let mut candidates = vec![];
        for a in e.attributes() {
            if matches!(a.kind(),crate::AttributeKind::Ordinary(q)if q.local().eq_ignore_ascii_case("uuid"))
            {
                candidates.push(
                    ObjectUuid::parse(a.value())
                        .map_err(|_| SourceTreeError::InvalidUuid { path: path.clone() })?,
                );
            }
        }
        Ok(candidates)
    };
    let root = collect(d.root())?;
    if root.len() > 1 {
        return Err(SourceTreeError::AmbiguousUuid { path: path.clone() });
    }
    if let Some(uuid) = root.first() {
        return Ok(Some(*uuid));
    }
    let mut candidates = vec![];
    for e in d.root().children().iter().filter_map(|n| {
        if let crate::XmlNode::Element(e) = n {
            Some(e)
        } else {
            None
        }
    }) {
        candidates.extend(collect(e)?);
    }
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(Some(candidates[0])),
        _ => Err(SourceTreeError::AmbiguousUuid { path: path.clone() }),
    }
}
