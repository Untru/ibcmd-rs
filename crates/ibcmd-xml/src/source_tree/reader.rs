use super::*;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Explicit caller-chosen budgets; normal Source reads have no fixed resource cap.
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
        Ok(self)
    }
}
/// A default reader admits complete Source inventories. `new` is an explicit
/// limited facade; its budget never becomes a serialized source permission.
#[derive(Default)]
pub struct SourceTreeReader {
    limits: Option<ReaderLimits>,
}
impl SourceTreeReader {
    pub fn new(limits: ReaderLimits) -> Result<Self, SourceTreeError> {
        Ok(Self {
            limits: Some(limits.validate()?),
        })
    }
    pub fn read(&self, root: impl AsRef<Path>) -> Result<SourceTree, SourceTreeError> {
        read_with_budget(root.as_ref(), self.limits, Purpose::OrdinaryInput)
    }
}
pub fn read_source_tree(root: impl AsRef<Path>) -> Result<SourceTree, SourceTreeError> {
    read_with_budget(root.as_ref(), None, Purpose::OrdinaryInput)
}
/// Strict authored input retains operational/empty-directory refusal. The
/// supplied caller budget is explicit and has no absolute Source model ceiling.
pub fn read_source_tree_strict(
    root: impl AsRef<Path>,
    limits: ReaderLimits,
) -> Result<SourceTree, SourceTreeError> {
    read_with_budget(
        root.as_ref(),
        Some(limits.validate()?),
        Purpose::StrictAuthoredInput,
    )
}
fn read_with_budget(
    root: &Path,
    limits: Option<ReaderLimits>,
    purpose: Purpose,
) -> Result<SourceTree, SourceTreeError> {
    let mut entries = Vec::new();
    walk(root, limits, purpose, &mut |_| Ok(()), &mut |entry| {
        entries
            .try_reserve(1)
            .map_err(|_| SourceTreeError::CapacityExceeded)?;
        entries.push(entry);
        Ok(())
    })?;
    SourceTree::new(entries)
}

// Traversal purpose and resource budget are independent. Only ordinary input
// skips operational-looking trees; closed staged verification inventories ALL
// physical entries, including empty directories, through the same walker.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Purpose {
    OrdinaryInput,
    StrictAuthoredInput,
    ClosedStagedOutput,
}
fn redirect(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}
fn strict_open(path: &Path) -> Result<File, SourceTreeError> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000).share_mode(1);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || redirect(&metadata) {
        return Err(SourceTreeError::UnsafePath(path.display().to_string()));
    }
    Ok(file)
}
/// Transient exact file/parent-directory domain derived before any staged writes.
/// Not a second Source model or authority reconstructed from disk observations.
pub(super) struct ClosedOutput<'a> {
    files: BTreeMap<&'a str, &'a SourceEntry>,
    directories: BTreeMap<String, String>,
    limits: Option<ReaderLimits>,
}
impl<'a> ClosedOutput<'a> {
    pub(super) fn prepare(
        expected: &'a SourceTree,
        limits: Option<ReaderLimits>,
    ) -> Result<Self, SourceTreeError> {
        expected.validate()?;
        if let Some(limits) = limits {
            limits.validate()?;
            if expected.entries().len() > limits.files {
                return Err(SourceTreeError::TooManyFiles);
            }
            let mut total = 0usize;
            for entry in expected.entries() {
                if entry.path().as_str().split('/').count().saturating_sub(1) > limits.depth {
                    return Err(SourceTreeError::DepthExceeded);
                }
                check_asset(entry.path(), entry.bytes().len(), Some(limits))?;
                total = total
                    .checked_add(entry.bytes().len())
                    .ok_or(SourceTreeError::TotalTooLarge)?;
                if total > limits.total_bytes {
                    return Err(SourceTreeError::TotalTooLarge);
                }
            }
        }
        let directories = directory_domain(expected.entries(), limits.map(|l| l.directories))?;
        let files = expected
            .entries()
            .iter()
            .map(|e| (e.path().as_str(), e))
            .collect();
        Ok(Self {
            files,
            directories,
            limits,
        })
    }
    pub(super) fn verify(mut self, root: &Path) -> Result<(), SourceTreeError> {
        walk(
            root,
            self.limits,
            Purpose::ClosedStagedOutput,
            &mut |raw| {
                let fold = raw.chars().flat_map(char::to_lowercase).collect::<String>();
                match self.directories.remove(&fold) {
                    Some(expected) if expected == raw => Ok(()),
                    _ => Err(mismatch()),
                }
            },
            &mut |entry| {
                let original = self
                    .files
                    .remove(entry.path().as_str())
                    .ok_or_else(mismatch)?;
                if &entry != original {
                    return Err(mismatch());
                }
                Ok(())
            },
        )?;
        if !self.files.is_empty() || !self.directories.is_empty() {
            return Err(mismatch());
        }
        Ok(())
    }
}
fn mismatch() -> SourceTreeError {
    SourceTreeError::PathConflict {
        first: SourcePath::new("staging").expect("fixed safe path"),
        second: SourcePath::new("tree").expect("fixed safe path"),
    }
}
#[cfg(test)]
pub(super) fn verify_with_limits(
    root: &Path,
    expected: &SourceTree,
    limits: ReaderLimits,
) -> Result<(), SourceTreeError> {
    ClosedOutput::prepare(expected, Some(limits.validate()?))?.verify(root)
}
struct State {
    limits: Option<ReaderLimits>,
    total: usize,
    dirs: usize,
    files: usize,
}
fn increment(
    counter: &mut usize,
    maximum: Option<usize>,
    error: fn() -> SourceTreeError,
) -> Result<(), SourceTreeError> {
    let next = counter
        .checked_add(1)
        .ok_or(SourceTreeError::CapacityExceeded)?;
    if maximum.is_some_and(|maximum| next > maximum) {
        return Err(error());
    }
    *counter = next;
    Ok(())
}
fn relative(root: &Path, path: &Path) -> Result<String, SourceTreeError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| SourceTreeError::UnsafePath("outside root".into()))?;
    let mut value = String::new();
    for component in relative.components() {
        let text = component
            .as_os_str()
            .to_str()
            .ok_or_else(|| SourceTreeError::UnsafePath("non-UTF8 filename".into()))?;
        if text.contains('\\') {
            return Err(SourceTreeError::UnsafePath(text.into()));
        }
        if !value.is_empty() {
            value.push('/');
        }
        value
            .try_reserve(text.len())
            .map_err(|_| SourceTreeError::CapacityExceeded)?;
        value.push_str(text);
    }
    Ok(value)
}
fn walk(
    root: &Path,
    limits: Option<ReaderLimits>,
    purpose: Purpose,
    directory: &mut impl FnMut(&str) -> Result<(), SourceTreeError>,
    accept: &mut impl FnMut(SourceEntry) -> Result<(), SourceTreeError>,
) -> Result<(), SourceTreeError> {
    let metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || redirect(&metadata) {
        return Err(SourceTreeError::UnsafePath(root.display().to_string()));
    }
    let mut state = State {
        limits,
        total: 0,
        dirs: 1,
        files: 0,
    };
    if limits.is_some_and(|l| state.dirs > l.directories) {
        return Err(SourceTreeError::TooManyDirectories);
    }
    directory("")?;
    // Explicit DFS work stack retains the original sorted traversal, without a
    // recursive Rust frame for each user-authored directory depth.
    let mut pending = vec![(root.to_path_buf(), true, 0usize)];
    while let Some((path, is_dir, depth)) = pending.pop() {
        if !is_dir {
            let source_path = SourcePath::new(relative(root, &path)?)?;
            let bytes = read_complete(&path, &source_path, &mut state)?;
            accept(SourceEntry::from_bytes(source_path, bytes)?)?;
            continue;
        }
        if limits.is_some_and(|l| depth > l.depth) {
            return Err(SourceTreeError::DepthExceeded);
        }
        let metadata = fs::symlink_metadata(&path)?;
        if !metadata.is_dir() || redirect(&metadata) {
            return Err(SourceTreeError::UnsafePath(path.display().to_string()));
        }
        let mut children: Vec<(PathBuf, bool, usize)> = Vec::new();
        for item in fs::read_dir(&path)? {
            let entry = item?;
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| SourceTreeError::UnsafePath("non-UTF8 filename".into()))?;
            if name.contains('\\') {
                return Err(SourceTreeError::UnsafePath(name.into()));
            }
            let child = entry.path();
            let raw = relative(root, &child)?;
            if purpose == Purpose::OrdinaryInput
                && !parent_configuration_resource(&raw)
                && matches!(name, ".git" | "target" | ".idea" | ".vscode")
            {
                continue;
            }
            let metadata = fs::symlink_metadata(&child)?;
            if redirect(&metadata) || (!metadata.is_file() && !metadata.is_dir()) {
                return Err(SourceTreeError::UnsafePath(child.display().to_string()));
            }
            let next_depth = depth
                .checked_add(1)
                .ok_or(SourceTreeError::CapacityExceeded)?;
            if metadata.is_dir() {
                // Closed membership is checked before any input-only path rule.
                directory(&raw)?;
                increment(&mut state.dirs, limits.map(|l| l.directories), || {
                    SourceTreeError::TooManyDirectories
                })?;
            } else {
                increment(&mut state.files, limits.map(|l| l.files), || {
                    SourceTreeError::TooManyFiles
                })?;
            }
            children
                .try_reserve(1)
                .map_err(|_| SourceTreeError::CapacityExceeded)?;
            children.push((child, metadata.is_dir(), next_depth));
        }
        if purpose == Purpose::StrictAuthoredInput && path != root && children.is_empty() {
            return Err(SourceTreeError::UnsafePath(path.display().to_string()));
        }
        children.sort_by(|a, b| a.0.file_name().cmp(&b.0.file_name()));
        pending
            .try_reserve(children.len())
            .map_err(|_| SourceTreeError::CapacityExceeded)?;
        pending.extend(children.into_iter().rev());
    }
    Ok(())
}
fn check_asset(
    path: &SourcePath,
    actual: usize,
    limits: Option<ReaderLimits>,
) -> Result<(), SourceTreeError> {
    if limits.is_some_and(|l| actual > l.asset_bytes) {
        return Err(SourceTreeError::AssetTooLarge {
            path: path.clone(),
            actual,
        });
    }
    Ok(())
}
fn read_complete(
    path: &Path,
    source_path: &SourcePath,
    state: &mut State,
) -> Result<Vec<u8>, SourceTreeError> {
    let mut file = strict_open(path)?;
    let announced =
        usize::try_from(file.metadata()?.len()).map_err(|_| SourceTreeError::CapacityExceeded)?;
    check_asset(source_path, announced, state.limits)?;
    let total = state
        .total
        .checked_add(announced)
        .ok_or(SourceTreeError::TotalTooLarge)?;
    if state.limits.is_some_and(|l| total > l.total_bytes) {
        return Err(SourceTreeError::TotalTooLarge);
    }
    let mut bytes = Vec::new();
    let mut scratch = [0u8; 65_536];
    loop {
        let count = match file.read(&mut scratch) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 {
            break;
        }
        let length = bytes
            .len()
            .checked_add(count)
            .ok_or(SourceTreeError::CapacityExceeded)?;
        check_asset(source_path, length, state.limits)?;
        let total = state
            .total
            .checked_add(length)
            .ok_or(SourceTreeError::TotalTooLarge)?;
        if state.limits.is_some_and(|l| total > l.total_bytes) {
            return Err(SourceTreeError::TotalTooLarge);
        }
        bytes
            .try_reserve(count)
            .map_err(|_| SourceTreeError::CapacityExceeded)?;
        bytes.extend_from_slice(&scratch[..count]);
    }
    if bytes.len() != announced || file.metadata()?.len() != announced as u64 {
        return Err(SourceTreeError::ChangedDuringRead(source_path.clone()));
    }
    state.total = total;
    Ok(bytes)
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
