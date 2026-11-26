//! Source-only configuration-extension compilation.
//!
//! An extension uses the ordinary typed descriptor and body encoders, then
//! records which properties of each adopted object its XML controls. Its CFE
//! manifest is generated from the compiled packed rows, without a base image.
use crate::legacy_version::InfobaseConfigSourceVersion;
use crate::metadata_model::brace::{Brace, serialize_row};
use crate::metadata_model::xml::{Element, MetadataXml};
use crate::metadata_model::{DescriptorContext, ObjectXml};
use crate::mssql_dump::extension::properties::guid_for;
use anyhow::{Context, Result, anyhow, bail};
use ibcmd_core::storage::{
    MultipartIdentity, StorageKey, StoragePatch, StoragePatchEntry, StoragePatchOutcome,
    StoragePatchTarget, StorageProvenance,
};
use std::collections::BTreeMap;
use std::path::Path;

pub(crate) struct Compilation {
    pub patch: StoragePatch,
    pub retained: usize,
    pub source_files: usize,
    pub metadata_files: usize,
    pub asset_files: usize,
    pub non_source_files: usize,
}
pub(crate) fn patch(root: &Path, version: InfobaseConfigSourceVersion) -> Result<Compilation> {
    let stage = crate::mssql::empty_stage::prepare_empty_stage(root, Some(version.as_str()))?;
    if !stage.context.extension {
        bail!("CFE compilation requires an extension source package");
    }
    let failures = stage
        .failures()
        .map(|f| format!("{} {}: {}", f.source, f.family, f.error))
        .collect::<Vec<_>>();
    if !failures.is_empty() {
        bail!(
            "{} extension rows cannot be compiled without a base:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
    let mut entries = BTreeMap::<String, Vec<u8>>::new();
    for row in stage.rows().filter(|row| row.family != "service") {
        if let Some(old) = entries.insert(row.file_name.clone(), row.blob.clone()) {
            if old != row.blob {
                bail!("conflicting compiled extension row {}", row.file_name);
            }
        }
    }
    let identity = crate::mssql_extension_stage::ConfigInfoIdentity {
        storage_format: stage.context.facts.compatibility,
        configuration_id: uuid::Uuid::parse_str(&stage.context.facts.uuid)?,
        // Native XML import creates an unsigned descriptor, not an opaque
        // identity borrowed from another extension container.
        descriptor: Vec::new(),
    };
    let rows = entries
        .iter()
        .map(|(n, b)| crate::mssql_extension_stage::ExtensionStageRow::new(n, b.clone()))
        .collect::<Vec<_>>();
    let manifest =
        crate::mssql_extension_stage::generate_container_configinfo_manifest(&identity, &rows)?;
    entries.insert(
        "configinfo".into(),
        crate::module_blob::deflate_raw(&manifest)?,
    );
    verify_source(root, &entries, version)?;
    let retained: usize = entries.values().map(Vec::len).sum();
    let patch = entries
        .into_iter()
        .map(|(name, bytes)| {
            Ok(StoragePatchEntry::new(
                StoragePatchTarget::new(
                    StorageKey::new(&name)?,
                    MultipartIdentity::single(),
                    StorageProvenance::new(&format!("bootstrap:cfe:{name}"))?,
                ),
                StoragePatchOutcome::compiled(bytes)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let metadata_files = stage.objects.len();
    let non_source_files = stage
        .tree_files
        .iter()
        .filter(|p| {
            p.file_name().is_some_and(|p| p == "ConfigDumpInfo.xml")
                || p.components().any(|c| c.as_os_str() == ".ibcmd")
        })
        .count();
    let source_files = stage.tree_files.len() - non_source_files;
    Ok(Compilation {
        patch: StoragePatch::with_retained_byte_limit(patch, retained.saturating_mul(2))?,
        retained,
        source_files,
        metadata_files,
        asset_files: source_files.saturating_sub(metadata_files),
        non_source_files,
    })
}

/// Extension descriptors retain the ordinary kind's representation; only the
/// metadata header and the configuration's extension coordinates differ.
pub(crate) fn compile_descriptor(
    kind: &str,
    path: &Path,
    xml: &[u8],
    context: &DescriptorContext,
) -> Result<Vec<u8>> {
    let doc = MetadataXml::parse(xml)?;
    let original = doc.object()?;
    let mut element = original.clone();
    normalize_values(&mut element);
    fill_uncontrolled(&mut element);
    if kind == "Language" && element.path(&["Properties", "LanguageCode"]).is_none() {
        element
            .children
            .iter_mut()
            .find(|e| e.name == "Properties")
            .context("Language Properties")?
            .children
            .push(Element {
                name: "LanguageCode".into(),
                ..Default::default()
            });
    }
    // Auto has a third extension-only representation; the ordinary root
    // encoder accepts the first two. Replace its field after encoding.
    if kind == "Configuration" {
        if let Some(p) = element.children.iter_mut().find(|e| e.name == "Properties") {
            if let Some(mode) = p
                .children
                .iter_mut()
                .find(|e| e.name == "DefaultRunMode" && e.text == "Auto")
            {
                mode.text = "ManagedApplication".into();
            }
        }
    }
    let object = ObjectXml {
        element: &element,
        kind,
        uuid: original
            .attr("uuid")
            .unwrap_or_default()
            .to_ascii_lowercase(),
        name: original
            .path(&["Properties", "Name"])
            .map(|e| e.text.clone())
            .unwrap_or_default(),
        path,
    };
    let mut row = crate::metadata_model::compile_object(&object, context)?;
    let mut headers = BTreeMap::new();
    collect_headers(original, context, &mut headers)?;
    if kind == "Configuration" {
        let module = original
            .child("InternalInfo")
            .context("Configuration InternalInfo")?
            .children_named("ContainedObject")
            .find(|e| {
                e.child_text("ClassId") == Some(crate::metadata_model::root::MODULE_GROUP_CLASS_ID)
            })
            .and_then(|e| e.child_text("ObjectId"))
            .context("configuration module group")?;
        if let Some(header) = headers.remove(&object.uuid) {
            headers.insert(module.to_string(), header);
        }
        let fields = at_mut(&mut row, &[3, 1, 1])?
            .as_list_mut()
            .context("configuration properties tuple")?;
        let p = original
            .child("Properties")
            .context("configuration properties")?;
        fields[42] = Brace::str(p.child_text("NamePrefix").unwrap_or_default());
        fields[43] = Brace::num(packed_version(
            p.child_text("ConfigurationExtensionCompatibilityMode")
                .unwrap_or_default(),
        )? as i64);
        fields[44] = Brace::num(match p.child_text("ConfigurationExtensionPurpose") {
            Some("Patch") => 0,
            Some("Customization") => 1,
            Some("AddOn") => 2,
            other => bail!("unknown extension purpose {other:?}"),
        });
        fields[49] = Brace::flag(
            p.child_text("KeepMappingToExtendedConfigurationObjectsByIDs") == Some("true"),
        );
        if p.child_text("DefaultRunMode") == Some("Auto") {
            fields[21] = Brace::num(2);
        }
    }
    replace_headers(&mut row, &headers)?;
    Ok(serialize_row(&row))
}

fn packed_version(text: &str) -> Result<u32> {
    let parts = text
        .strip_prefix("Version")
        .context("extension compatibility must be VersionX_Y_Z")?
        .split('_')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if parts.len() != 3 || parts[1] > 99 || parts[2] > 99 {
        bail!("invalid extension compatibility {text}");
    }
    Ok(parts[0] * 10000 + parts[1] * 100 + parts[2])
}
fn at_mut<'a>(mut row: &'a mut Brace, path: &[usize]) -> Result<&'a mut Brace> {
    for &i in path {
        row = row
            .as_list_mut()
            .and_then(|l| l.get_mut(i))
            .ok_or_else(|| anyhow!("missing compiled tuple at {path:?}"))?;
    }
    Ok(row)
}
fn normalize_values(e: &mut Element) {
    for child in &mut e.children {
        normalize_values(child);
    }
    if let Some(check) = e.child("CheckValue").cloned() {
        let name = e.name.clone();
        *e = check;
        e.name = name;
    }
}
fn state_guid(kind: &str, name: &str) -> Result<&'static str> {
    guid_for(kind, name).ok_or_else(|| {
        anyhow!("{kind} property {name} has no evidenced extension property identifier")
    })
}
fn collect_headers(
    e: &Element,
    ctx: &DescriptorContext,
    out: &mut BTreeMap<String, Vec<Brace>>,
) -> Result<()> {
    if let Some(p) = e.child("Properties") {
        if p.child_text("ObjectBelonging") == Some("Adopted") {
            let uuid = e
                .attr("uuid")
                .context("adopted object uuid")?
                .to_ascii_lowercase();
            let mut states = BTreeMap::<String, u8>::new();
            let fixed_root = [
                "Synonym",
                "ConfigurationExtensionPurpose",
                "KeepMappingToExtendedConfigurationObjectsByIDs",
                "NamePrefix",
                "ConfigurationExtensionCompatibilityMode",
                "ScriptVariant",
                "Vendor",
                "Version",
                "Caption",
                "ShortCaption",
                "BriefInformation",
                "DetailedInformation",
                "Copyright",
                "VendorInformationAddress",
                "ConfigurationInformationAddress",
            ];
            for prop in &p.children {
                if matches!(prop.name.as_str(), "ObjectBelonging" | "Name" | "Comment")
                    || (e.name == "Document" && prop.name == "RegisterRecords")
                    || (e.name == "Configuration" && fixed_root.contains(&prop.name.as_str()))
                {
                    continue;
                }
                states.insert(state_guid(&e.name, &prop.name)?.to_owned(), 2);
            }
            let mut widened = Vec::new();
            if let Some(info) = e.child("InternalInfo") {
                for s in info.children_named("PropertyState") {
                    let name = s.child_text("Property").context("property state name")?;
                    let state = s.child_text("State").context("property state value")?;
                    if e.name == "Configuration"
                        && ["HomePageWorkArea", "Logo", "Splash"].contains(&name)
                    {
                        // These three ids are evidenced as one inseparable
                        // changed-asset group by the extension exporter.
                        for id in [
                            "d98a8e01-7219-41ce-9f23-dada0860b9bf",
                            "740eb5f6-e214-4e30-aefa-f15bab91c688",
                            "3035a9db-d6b2-450e-8b22-4430577f8dab",
                        ] {
                            states.insert(id.into(), 3);
                        }
                        continue;
                    }
                    let id = state_guid(&e.name, name)?;
                    let code = match state {
                        "Extended" => 3,
                        "MultiState" => {
                            if p.child(name).and_then(|p| p.child("CheckValue")).is_some() {
                                2
                            } else {
                                3
                            }
                        }
                        "Controlled" | "Checked" => 2,
                        other => bail!("unsupported extension property state {other}"),
                    };
                    states.insert(id.into(), code);
                    if state == "MultiState" {
                        let prop = p.child(name).context("extended property")?;
                        let value = prop
                            .child("ExtendValue")
                            .context("extended property ExtendValue")?;
                        match name {
                            "Type" | "CommandParameterType" => {
                                let mut value = value.clone();
                                value.name = name.into();
                                widened.push((
                                    id,
                                    crate::brace_list![
                                        Brace::str("#"),
                                        Brace::uuid("f5c65050-3bbb-11d5-b988-0050bae0a95d"),
                                        crate::metadata_model::types::type_pattern(
                                            Some(&value),
                                            ctx
                                        )?
                                    ],
                                ));
                            }
                            _ => bail!("unsupported widened extension property {name}"),
                        }
                    }
                }
            }
            let mut tail = vec![Brace::num(1), Brace::num(states.len() as i64)];
            for (id, state) in states {
                tail.push(Brace::uuid(&id));
                tail.push(Brace::num(state as i64));
            }
            tail.push(Brace::uuid(
                p.child_text("ExtendedConfigurationObject")
                    .unwrap_or(crate::metadata_model::brace::NIL_UUID),
            ));
            tail.push(Brace::num(widened.len() as i64));
            for (id, value) in widened {
                tail.extend([Brace::uuid(id), Brace::num(3), value]);
            }
            out.insert(uuid, tail);
        }
    }
    for c in &e.children {
        if c.name == "ChildObjects" {
            for child in &c.children {
                collect_headers(child, ctx, out)?;
            }
        }
    }
    Ok(())
}
fn replace_headers(row: &mut Brace, headers: &BTreeMap<String, Vec<Brace>>) -> Result<()> {
    let Some(fields) = row.as_list_mut() else {
        return Ok(());
    };
    if fields.first().and_then(Brace::as_atom) == Some("3")
        && fields
            .get(1)
            .and_then(|n| n.at(&[0]))
            .and_then(Brace::as_atom)
            == Some("1")
    {
        if let Some(uuid) = fields
            .get(1)
            .and_then(|n| n.at(&[2]))
            .and_then(Brace::as_atom)
        {
            if let Some(tail) = headers.get(uuid) {
                fields.truncate(5);
                fields.extend(tail.clone());
                return Ok(());
            }
        }
    }
    for f in fields {
        replace_headers(f, headers)?;
    }
    Ok(())
}

/// The absent fields of an adopted object are not checks on its base object.
/// Fill those private encoder inputs from the kind's typed slot definitions;
/// only the original XML determines which properties the header controls.
fn fill_uncontrolled(e: &mut Element) {
    if e.path(&["Properties", "ObjectBelonging"])
        .map(|p| p.text.as_str())
        == Some("Adopted")
    {
        if let Some(p) = e.children.iter_mut().find(|p| p.name == "Properties") {
            let defaults: &[(&str, &str)] = match e.name.as_str() {
                "CommonModule" => &[("ReturnValuesReuse", "DontUse")],
                "CommonCommand" | "Command" => &[
                    ("Group", "0:00000000-0000-0000-0000-000000000000"),
                    ("Representation", "Auto"),
                    ("ParameterUseMode", "Single"),
                    ("OnMainServerUnavalableBehavior", "Auto"),
                    ("ModifiesData", "false"),
                    ("IncludeHelpInContents", "false"),
                ],
                _ => &[],
            };
            for &(name, value) in defaults {
                if p.child(name).is_none() {
                    p.children.push(Element {
                        name: name.into(),
                        text: value.into(),
                        ..Default::default()
                    });
                }
            }
            if e.name == "StyleItem" && p.child("Value").is_none() {
                let value = match p.child_text("Type") {
                    Some("Color") => Element {
                        name: "Value".into(),
                        text: "web:Black".into(),
                        ..Default::default()
                    },
                    _ => Element {
                        name: "Value".into(),
                        ..Default::default()
                    },
                };
                p.children.push(value);
            }
        }
        if let Some(layout) = crate::metadata_model::objects::layout(&e.name) {
            if let Some(p) = e.children.iter_mut().find(|p| p.name == "Properties") {
                for slot in layout.slots {
                    fill_slot(p, slot);
                }
            }
        }
    }
    for c in &mut e.children {
        if c.name == "ChildObjects" {
            for child in &mut c.children {
                fill_uncontrolled(child);
            }
        }
    }
}
fn fill_slot(p: &mut Element, slot: &crate::metadata_model::objects::parts::Slot) {
    use crate::metadata_model::objects::parts::Slot;
    let (name, value) = match slot {
        Slot::Flag(name) => (*name, "false"),
        Slot::Number(name) => (*name, "0"),
        Slot::Code(name, table) => (*name, table.first().map(|v| v.0).unwrap_or_default()),
        Slot::Modern(inner) | Slot::Since8_5_1(inner) => {
            fill_slot(p, inner);
            return;
        }
        _ => return,
    };
    if p.child(name).is_none() {
        p.children.push(Element {
            name: name.into(),
            text: value.into(),
            ..Default::default()
        });
    }
}

struct ExportScratch(std::path::PathBuf);
impl Drop for ExportScratch {
    fn drop(&mut self) {
        if std::env::var_os("IBCMD_RS_CFE_BUILD_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
fn verify_source(
    root: &Path,
    entries: &BTreeMap<String, Vec<u8>>,
    version: InfobaseConfigSourceVersion,
) -> Result<()> {
    let out = ExportScratch(std::env::temp_dir().join(format!(
        "ibcmd-cfe-build-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    )));
    let report = super::export_entries_to_source(
        "storage:cfe-bootstrap",
        entries
            .iter()
            .map(|(n, b)| (n.clone(), b.clone()))
            .collect(),
        &out.0,
        false,
        version,
    )?;
    if report.storage.failed != 0 || report.storage.opaque != 0 {
        let errors = report
            .storage
            .entries
            .iter()
            .filter(|e| e.disposition != ibcmd_cf::export::StorageExportDisposition::Supported)
            .map(|e| format!("{}: {:?}", e.logical_key, e.message))
            .collect::<Vec<_>>();
        bail!(
            "compiled extension cannot be exported completely: {} failed, {} opaque rows; {}",
            report.storage.failed,
            report.storage.opaque,
            errors.join("; ")
        );
    }
    fn files(root: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
        let mut files = BTreeMap::new();
        for p in crate::source_listing::walk(root).files {
            let rel = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
            if rel == "ConfigDumpInfo.xml" || rel.starts_with(".ibcmd/") {
                continue;
            }
            files.insert(rel, std::fs::read(&p)?);
        }
        Ok(files)
    }
    let expected = files(root)?;
    let actual = files(&out.0)?;
    let names = expected
        .keys()
        .chain(actual.keys())
        .collect::<std::collections::BTreeSet<_>>();
    let differences = names
        .into_iter()
        .filter(|n| expected.get(*n) != actual.get(*n))
        .cloned()
        .collect::<Vec<_>>();
    if !differences.is_empty() {
        bail!(
            "compiled extension export differs in {} source files: {}; scratch {}",
            differences.len(),
            differences.join(", "),
            out.0.display()
        );
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source_fixture() -> ExportScratch {
        let root = ExportScratch(
            std::env::temp_dir().join(format!("ibcmd-cfe-builder-test-{}", uuid::Uuid::new_v4())),
        );
        std::fs::create_dir_all(root.0.join("Languages")).unwrap();
        std::fs::write(root.0.join("Configuration.xml"),include_bytes!("../../tests/fixtures/native-evidence/extension-empty/native-8.5.1/Configuration.xml")).unwrap();
        std::fs::write(
            root.0.join("Languages/Русский.xml"),
            include_bytes!(
                "../../tests/fixtures/native-evidence/extension-empty/native-8.5.1/Language.xml"
            ),
        )
        .unwrap();
        root
    }
    #[test]
    fn empty_native_extension_compiles_from_sources_without_base_rows() {
        let root = source_fixture();
        let compilation = patch(&root.0, InfobaseConfigSourceVersion::V2_21).unwrap();
        assert_eq!(
            (
                compilation.source_files,
                compilation.metadata_files,
                compilation.asset_files
            ),
            (2, 2, 0)
        );
        let names = compilation
            .patch
            .entries()
            .iter()
            .map(|e| e.target().key().as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(names.len(), 3);
        assert!(names.contains("configinfo"));
        for forbidden in ["root", "version", "versions"] {
            assert!(!names.contains(forbidden));
        }
    }
    #[test]
    fn unknown_controlled_property_is_refused_instead_of_dropped() {
        let root = source_fixture();
        let path = root.0.join("Languages/Русский.xml");
        let xml = std::fs::read_to_string(&path).unwrap().replace(
            "<Comment/>",
            "<Comment/><UnmappedProperty>true</UnmappedProperty>",
        );
        std::fs::write(&path, xml).unwrap();
        let error = patch(&root.0, InfobaseConfigSourceVersion::V2_21)
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("UnmappedProperty") && error.contains("no evidenced extension property"),
            "{error}"
        );
    }
}
