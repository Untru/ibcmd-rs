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
    let view = morph1c_core::ir::semantic_view::ConfigurationSemanticView {
        configuration: config,
        template_body: morph1c_pipeline::dcs_template_semantic_body,
    };
    serde_json::to_writer(&mut writer, &view).map_err(EdtError::source)?;
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

#[cfg(test)]
mod module_scaffold_tests {
    use super::*;
    use morph1c_core::ir::{FormBody, MetadataObject, NamedFormBody, ObjectKind, Uuid};

    #[test]
    fn module_scaffold_raw_edits_remain_visible_even_with_forged_generated_hashes() {
        let text = "#Область Current\n\n#КонецОбласти";
        let mut config = Configuration::new();
        let mut object = MetadataObject::new(ObjectKind::new("CommonForm"), "F", Uuid([0x37; 16]));
        object.form_bodies.push(NamedFormBody {
            name: "F".into(),
            body: FormBody {
                source_module_scaffold_edt_lf: true,
                ..FormBody::new()
            },
            ordinary_body: None,
            module: Some(text.into()),
            help: vec![],
            help_resources: vec![],
        });
        config.objects.push(object);
        let options = ConversionOptions {
            edt_version: "2025.2.3".into(),
            xml_dialect: "2.21".into(),
            runtime_version: Some("8.5.1".into()),
        };
        let entry = |path, bytes: Vec<u8>| {
            SourceEntry::from_bytes(SourcePath::new(path).unwrap(), bytes).unwrap()
        };
        let path = "src/CommonForms/F/Module.bsl";
        let original =
            SourceTree::new(vec![entry("Configuration.xml", b"<native/>".to_vec())]).unwrap();
        let generated = SourceTree::new(vec![entry(path, text.as_bytes().to_vec())]).unwrap();
        let retained = retain(&original, generated, &options, &config).unwrap();
        assert_eq!(
            restore(&retained, &options, &config).unwrap(),
            Some(original)
        );
        let mut lexical_only = config.clone();
        lexical_only.objects[0].form_bodies[0]
            .body
            .source_module_scaffold_edt_lf = false;
        assert_eq!(
            semantic_digest(&lexical_only).unwrap(),
            semantic_digest(&config).unwrap()
        );
        for edited in [
            text.replace('\n', "\r\n"),
            text.replace("Current", "Edited"),
            format!("{text}\n// current code edit"),
        ] {
            let mut current = config.clone();
            current.objects[0].form_bodies[0].module = Some(edited.clone());
            assert_ne!(
                semantic_digest(&current).unwrap(),
                semantic_digest(&config).unwrap()
            );
            let mut manifest: serde_json::Value = serde_json::from_slice(
                retained
                    .entries()
                    .iter()
                    .find(|entry| entry.path().as_str() == MANIFEST)
                    .unwrap()
                    .bytes(),
            )
            .unwrap();
            manifest["generated"][path] = serde_json::json!(digest(edited.as_bytes()));
            let mut entries = retained
                .entries()
                .iter()
                .filter(|entry| ![path, MANIFEST].contains(&entry.path().as_str()))
                .cloned()
                .collect::<Vec<_>>();
            entries.push(entry(path, edited.into_bytes()));
            entries.push(entry(MANIFEST, serde_json::to_vec(&manifest).unwrap()));
            let forged = SourceTree::new(entries).unwrap();
            assert!(
                restore(&forged, &options, &current)
                    .unwrap_err()
                    .to_string()
                    .contains("stale provenance")
            );
        }
    }
}
