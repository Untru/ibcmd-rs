//! Native ConfigDumpInfo version status, independent of editor byte comparison.
use crate::mssql_dump::incremental::DumpInfo;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigVersionEntry {
    pub name: String,
    pub id: String,
    pub config_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDumpInfoSnapshot {
    pub entries: Vec<ConfigVersionEntry>,
    pub parts: BTreeMap<String, String>,
}
impl ConfigDumpInfoSnapshot {
    pub fn parse(xml: &[u8]) -> Result<Self> {
        let parsed = DumpInfo::parse(xml)?;
        Ok(Self {
            entries: parsed
                .entries
                .into_iter()
                .map(|entry| ConfigVersionEntry {
                    name: entry.name,
                    id: entry.id,
                    config_version: entry.config_version,
                })
                .collect(),
            parts: parsed.parts,
        })
    }
    /// Maps each versioned row to its metadata object, using canonical
    /// references and root descriptors from this exact inventory. Unmapped rows are explicit; callers
    /// must not report them unchanged. Configuration uses the editor's root
    /// name `Configuration`; owned forms/templates retain their full names.
    pub fn owners(&self) -> NativeVersionOwners {
        use crate::metadata_model::index::{NESTED_COLLECTIONS, ROOT_COLLECTIONS};
        let root_kind = |kind: &str| {
            kind == "Configuration" || ROOT_COLLECTIONS.iter().any(|(_, k)| *k == kind)
        };
        let nested_kind = |kind: &str| {
            kind == "Subsystem"
                || kind == "Command"
                || NESTED_COLLECTIONS.iter().any(|(_, k)| *k == kind)
        };
        let reference = |name: &str| {
            let parts = name.split('.').collect::<Vec<_>>();
            parts.len() >= 2
                && parts.len() % 2 == 0
                && root_kind(parts[0])
                && parts.iter().all(|part| !part.is_empty())
                && parts[2..].chunks(2).all(|pair| nested_kind(pair[0]))
        };
        let roots = self
            .entries
            .iter()
            .filter(|entry| {
                entry.name.split('.').count() == 2
                    && reference(&entry.name)
                    && uuid::Uuid::parse_str(&entry.id).is_ok()
            })
            .map(|entry| entry.name.as_str())
            .collect::<std::collections::HashSet<_>>();
        let mut by_id = BTreeMap::new();
        let mut unmapped_ids = Vec::new();
        for entry in &self.entries {
            // A row is the descriptor's canonical reference, or exactly one
            // role after it. Never climb past a missing/unknown nested kind.
            // Inline members (notably Command modules) need not have a
            // standalone descriptor version entry. Storage row UUIDs are not
            // universally the metadata object's UUID.
            let candidate = if entry.name.split('.').count() % 2 == 0 {
                entry.name.as_str()
            } else {
                entry
                    .name
                    .rsplit_once('.')
                    .map(|(owner, _)| owner)
                    .unwrap_or("")
            };
            let root = candidate.split('.').take(2).collect::<Vec<_>>().join(".");
            if !reference(candidate) || !roots.contains(root.as_str()) {
                unmapped_ids.push(entry.id.clone());
                continue;
            }
            // Owned Command descriptors live inside the owner's XML, unlike
            // independently exportable forms/templates. Preserve their native
            // reference but assign their CommandModule to that editor object.
            let parts = candidate.split('.').collect::<Vec<_>>();
            let export_object = if parts.len() > 2 && parts[parts.len() - 2] == "Command" {
                let without_name = candidate.rsplit_once('.').unwrap().0;
                without_name.rsplit_once('.').unwrap().0
            } else {
                candidate
            };
            let normalize = |name: &str| {
                if name.starts_with("Configuration.") {
                    "Configuration".to_owned()
                } else {
                    name.to_owned()
                }
            };
            by_id.insert(
                entry.id.clone(),
                NativeVersionOwner {
                    object: normalize(export_object),
                    root: normalize(&root),
                    native_object: candidate.to_owned(),
                },
            );
        }
        NativeVersionOwners {
            by_id,
            unmapped_ids,
        }
    }

    /// Native status ignores a reference-only XML dialect header difference.
    /// An ID removal or rename requires a full export; version changes can
    /// occur even when the exported XML bytes are identical.
    pub fn compare(&self, reference: &Self) -> NativeExportStatus {
        let previous = reference
            .entries
            .iter()
            .map(|entry| (&entry.id, entry))
            .collect::<BTreeMap<_, _>>();
        let now = self
            .entries
            .iter()
            .map(|entry| (&entry.id, entry))
            .collect::<BTreeMap<_, _>>();
        let full_required = reference.entries.iter().any(|entry| {
            now.get(&entry.id)
                .is_none_or(|current| current.name != entry.name)
        }) || reference
            .parts
            .iter()
            .any(|(id, name)| self.parts.get(id) != Some(name));
        let mut changes = self
            .entries
            .iter()
            .filter_map(|entry| {
                let old = previous.get(&entry.id);
                let kind = match old {
                    None => ChangeKind::Added,
                    Some(old)
                        if old.name != entry.name || old.config_version != entry.config_version =>
                    {
                        ChangeKind::Modified
                    }
                    _ => return None,
                };
                Some(VersionChange {
                    name: entry.name.clone(),
                    id: entry.id.clone(),
                    kind,
                    config_version: entry.config_version.clone(),
                    reference_version: old.map(|entry| entry.config_version.clone()),
                })
            })
            .collect::<Vec<_>>();
        changes.sort_by(|a, b| a.name.cmp(&b.name));
        NativeExportStatus {
            full_required,
            changes,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVersionOwner {
    /// Independently exportable/editor object, e.g. Catalog.X.Form.F.
    /// An inline Command module belongs to its owning descriptor object.
    pub object: String,
    /// Exact native owner reference, including inline commands.
    pub native_object: String,
    /// Top-level object, e.g. Catalog.X (or Configuration).
    pub root: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeVersionOwners {
    pub by_id: BTreeMap<String, NativeVersionOwner>,
    pub unmapped_ids: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Added,
    Modified,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionChange {
    pub name: String,
    pub id: String,
    pub kind: ChangeKind,
    pub config_version: String,
    pub reference_version: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeExportStatus {
    pub full_required: bool,
    pub changes: Vec<VersionChange>,
}
impl NativeExportStatus {
    pub fn text(&self, short: bool) -> String {
        if self.full_required {
            return format!("{}: all\n", if short { "M" } else { "modified" });
        }
        self.changes
            .iter()
            .map(|change| {
                format!(
                    "{}: {}\n",
                    match (change.kind, short) {
                        (ChangeKind::Added, true) => "A",
                        (ChangeKind::Added, false) => "added",
                        (ChangeKind::Modified, true) => "M",
                        (ChangeKind::Modified, false) => "modified",
                    },
                    change.name
                )
            })
            .collect()
    }
}
