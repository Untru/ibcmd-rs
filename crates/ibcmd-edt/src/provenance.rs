use crate::{ConversionOptions, EdtError};
use ibcmd_xml::source_tree::{SourceEntry, SourcePath, SourceTree};
use morph1c_core::ir::Configuration;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(crate) const PREFIX: &str = ".ibcmd-provenance/";
pub(crate) const MANIFEST: &str = ".ibcmd-provenance/manifest.json";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub(crate) version: u8,
    pub(crate) edt_version: String,
    pub(crate) xml_dialect: String,
    pub(crate) runtime_version: Option<String>,
    pub(crate) semantics: String,
    pub(crate) generated: BTreeMap<String, String>,
    pub(crate) original: BTreeMap<String, String>,
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub(crate) fn semantic_digest(config: &Configuration) -> Result<String, EdtError> {
    // The private IR's serde adapters compare property bags by stable FieldId.
    // Stream every remaining ordered value directly into SHA-256. A JSON Value
    // tree would expand large binary bodies to millions of heap nodes.
    struct HashWriter(Sha256);
    impl std::io::Write for HashWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.update(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = std::io::BufWriter::with_capacity(64 * 1024, HashWriter(Sha256::new()));
    serde_json::to_writer(&mut writer, config).map_err(EdtError::source)?;
    let writer = writer.into_inner().map_err(EdtError::source)?;
    Ok(format!("{:x}", writer.0.finalize()))
}
pub(crate) fn retain(
    original: &SourceTree,
    project: SourceTree,
    o: &ConversionOptions,
    config: &Configuration,
) -> Result<SourceTree, EdtError> {
    let manifest = Manifest {
        version: 1,
        edt_version: o.edt_version.clone(),
        xml_dialect: o.xml_dialect.clone(),
        runtime_version: o.runtime_version.clone(),
        semantics: semantic_digest(config)?,
        generated: project
            .entries()
            .iter()
            .map(|e| (e.path().as_str().to_string(), digest(e.bytes())))
            .collect(),
        original: original
            .entries()
            .iter()
            .map(|e| (e.path().as_str().to_string(), digest(e.bytes())))
            .collect(),
    };
    let mut entries = project.entries().to_vec();
    entries.push(
        SourceEntry::from_bytes(
            SourcePath::new(MANIFEST).map_err(EdtError::source)?,
            serde_json::to_vec_pretty(&manifest).map_err(EdtError::source)?,
        )
        .map_err(EdtError::source)?,
    );
    for e in original.entries() {
        entries.push(
            e.with_path(
                SourcePath::new(format!("{PREFIX}xml/{}", e.path())).map_err(EdtError::source)?,
            )
            .map_err(EdtError::source)?,
        );
    }
    SourceTree::new(entries).map_err(EdtError::source)
}
pub(crate) fn restore(
    project: &SourceTree,
    o: &ConversionOptions,
    config: &Configuration,
) -> Result<Option<SourceTree>, EdtError> {
    let entry = project
        .entries()
        .iter()
        .find(|e| e.path().as_str() == MANIFEST);
    if entry.is_none() {
        if project
            .entries()
            .iter()
            .any(|e| e.path().as_str().starts_with(PREFIX))
        {
            return Err(EdtError::new("provenance payload exists without manifest"));
        }
        return Ok(None);
    }
    let manifest: Manifest =
        serde_json::from_slice(entry.unwrap().bytes()).map_err(EdtError::source)?;
    if manifest.version != 1
        || manifest.edt_version != o.edt_version
        || manifest.xml_dialect != o.xml_dialect
        || manifest.runtime_version != o.runtime_version
    {
        return Err(EdtError::new(
            "provenance profile differs from requested conversion; remove provenance for typed conversion",
        ));
    }
    let generated = project
        .entries()
        .iter()
        .filter(|e| !e.path().as_str().starts_with(PREFIX))
        .map(|e| (e.path().as_str().to_string(), digest(e.bytes())))
        .collect::<BTreeMap<_, _>>();
    if generated != manifest.generated || semantic_digest(config)? != manifest.semantics {
        return Err(EdtError::new(
            "stale provenance: EDT files changed; refusing to restore original XML. Remove .ibcmd-provenance for typed conversion",
        ));
    }
    let prefix = format!("{PREFIX}xml/");
    let originals = project
        .entries()
        .iter()
        .filter_map(|e| e.path().as_str().strip_prefix(&prefix).map(|p| (p, e)))
        .collect::<Vec<_>>();
    let hashes = originals
        .iter()
        .map(|(p, e)| (p.to_string(), digest(e.bytes())))
        .collect::<BTreeMap<_, _>>();
    if hashes != manifest.original
        || project
            .entries()
            .iter()
            .filter(|e| e.path().as_str().starts_with(PREFIX))
            .count()
            != originals.len() + 1
    {
        return Err(EdtError::new(
            "provenance payload inventory/digest mismatch",
        ));
    }
    let entries = originals
        .into_iter()
        .map(|(p, e)| {
            e.with_path(SourcePath::new(p).map_err(EdtError::source)?)
                .map_err(EdtError::source)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(SourceTree::new(entries).map_err(EdtError::source)?))
}
