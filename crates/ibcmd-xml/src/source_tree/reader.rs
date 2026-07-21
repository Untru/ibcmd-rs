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
            files: MAX_SOURCE_FILES,
            directories: MAX_SOURCE_DIRECTORIES,
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
            || self.asset_bytes > ibcmd_core::asset::MAX_ASSET_BYTES
            || self.total_bytes > ibcmd_core::model::MAX_CONFIGURATION_RETAINED_BYTES
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
    let root = root.as_ref();
    let m = fs::symlink_metadata(root)?;
    if !m.file_type().is_dir() || m.file_type().is_symlink() {
        return Err(SourceTreeError::UnsafePath(root.display().to_string()));
    }
    let mut state = State {
        limits,
        total: 0,
        dirs: 1,
        files: 0,
        out: vec![],
    };
    visit(root, root, 0, &mut state)?;
    SourceTree::new(state.out)
}
struct State {
    limits: ReaderLimits,
    total: usize,
    dirs: usize,
    files: usize,
    out: Vec<SourceEntry>,
}
fn visit(root: &Path, dir: &Path, depth: usize, s: &mut State) -> Result<(), SourceTreeError> {
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
        if matches!(n, ".git" | "target" | ".idea" | ".vscode") {
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
            visit(root, &e.path(), depth + 1, s)?
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
            let mut kind = classify(path.as_str());
            let xml = if path.as_str().to_ascii_lowercase().ends_with(".xml") {
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
                && xml.as_ref().is_some_and(|d| {
                    matches!(
                        d.root().name().local(),
                        "MetaDataObject" | "Configuration" | "DefinedType"
                    )
                })
            {
                kind = SourceKind::MetadataXml
            }
            let uuid = if matches!(
                kind,
                SourceKind::ConfigurationRoot | SourceKind::MetadataXml
            ) {
                derive_uuid(&path, xml.as_ref().unwrap())?
            } else {
                None
            };
            s.out.push(SourceEntry::new(path, kind, bytes, uuid)?);
        }
    }
    Ok(())
}
pub(crate) fn classify(p: &str) -> SourceKind {
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
