//! Typed source-only BSP aggregate/predefined/recalculation sidecar orchestration.
use crate::{ConvertError, Format};
use formats_xml::{
    emit::{render, Envelope},
    source_extensions as codec, OutElement,
};
use morph1c_core::ir::MetadataObject;
use std::path::Path;
fn error(path: &Path, reason: String) -> ConvertError {
    ConvertError::Read {
        kind: "source-extension".into(),
        object: path.display().to_string(),
        reason,
    }
}
fn read(path: &Path) -> Result<formats_xml::read::Descriptor, ConvertError> {
    let bytes = std::fs::read(path).map_err(|e| error(path, e.to_string()))?;
    formats_xml::parse(&bytes).map_err(|e| error(path, e.to_string()))
}
fn base(path: &Path) -> Result<std::path::PathBuf, ConvertError> {
    let dir = path
        .parent()
        .ok_or_else(|| error(path, "missing descriptor directory".into()))?;
    let stem = path
        .file_stem()
        .ok_or_else(|| error(path, "missing descriptor stem".into()))?;
    Ok(dir.join(stem))
}
fn version(root: &formats_xml::Element) -> Result<String, String> {
    root.attr("version")
        .map(|a| a.value.clone())
        .filter(|v| v == "2.20" || v == "2.21")
        .ok_or_else(|| "missing explicit XML dialect".into())
}
pub fn attach(format: Format, path: &Path, obj: &mut MetadataObject) -> Result<(), ConvertError> {
    if format != Format::Designer
        || !matches!(
            obj.kind.as_str(),
            "AccumulationRegister" | "ChartOfCalculationTypes" | "CalculationRegister"
        )
    {
        return Ok(());
    }
    let directory = base(path)?;
    let root = read(path)?;
    let version = version(&root.root).map_err(|e| error(path, e))?;
    if obj.kind.as_str() == "AccumulationRegister" {
        let body = directory.join("Ext/Aggregates.xml");
        if body.is_file() {
            let doc = read(&body)?;
            codec::claim_sidecar_envelope(&doc.root, "AccumulationRegisterAggregates", &version)
                .map_err(|e| error(&body, e))?;
            obj.source_extensions.aggregates =
                Some(codec::read_aggregates(&doc.root, false).map_err(|e| error(&body, e))?);
        }
    }
    if obj.kind.as_str() == "ChartOfCalculationTypes" {
        let body = directory.join("Ext/Predefined.xml");
        if body.is_file() {
            let doc = read(&body)?;
            codec::claim_sidecar_envelope(&doc.root, "PredefinedData", &version)
                .map_err(|e| error(&body, e))?;
            let mut items = codec::read_calculation_predefined(&doc.root, false)
                .map_err(|e| error(&body, e))?;
            let numeric = matches!(obj.get(morph1c_core::spec::metadata::chart_of_calculation_types::F_CODE_TYPE),Some(morph1c_core::ir::PropertyValue::Enum(token)) if token.as_str()=="Number");
            for item in &mut items {
                item.numeric_code = numeric;
            }
            obj.source_extensions.calculation_predefined = Some(items);
        }
    }
    if obj.kind.as_str() == "CalculationRegister" {
        for name in obj.source_extensions.recalculation_refs.clone() {
            let body = directory.join("Recalculations").join(format!("{name}.xml"));
            let doc = read(&body)?;
            let v = formats_designer::common::verify_root_envelope(&doc.root)
                .map_err(|e| error(&body, e))?;
            if v.to_string() != version {
                return Err(error(
                    &body,
                    "recalculation XML dialect differs from owner".into(),
                ));
            }
            if doc.root.children.len() != 1 || !doc.root.text.is_empty() {
                return Err(error(
                    &body,
                    "unexpected recalculation envelope content".into(),
                ));
            }
            doc.root.claim();
            let e = &doc.root.children[0];
            if e.local != "Recalculation" {
                return Err(error(&body, "expected Recalculation".into()));
            }
            let item = codec::read_recalculation(e, false, Some(&obj.name))
                .map_err(|e| error(&body, e))?;
            if item.name != name {
                return Err(error(
                    &body,
                    "recalculation name differs from declared reference".into(),
                ));
            }
            if doc.root.unclaimed_count() != 0 {
                return Err(error(
                    &body,
                    "uncovered recalculation envelope cells".into(),
                ));
            }
            obj.source_extensions.recalculations.push(item);
        }
    }
    Ok(())
}
const BODY_ENVELOPE: Envelope = formats_designer::common::DESIGNER_ENVELOPE;
fn write(path: &Path, root: OutElement) -> Result<(), ConvertError> {
    let bytes = render(&BODY_ENVELOPE, &root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| error(path, e.to_string()))?;
    }
    std::fs::write(path, bytes).map_err(|e| error(path, e.to_string()))
}
pub fn emit(format: Format, path: &Path, obj: &MetadataObject) -> Result<(), ConvertError> {
    if format != Format::Designer
        || (obj.source_extensions.aggregates.is_none()
            && obj.source_extensions.calculation_predefined.is_none()
            && obj.source_extensions.recalculations.is_empty())
    {
        return Ok(());
    }
    let directory = base(path)?;
    let target = morph1c_core::version::current_roundtrip_target().ok_or_else(|| {
        error(
            path,
            "source extension output requires explicit XML target".into(),
        )
    })?;
    let version = target.to_string();
    if let Some(values) = &obj.source_extensions.aggregates {
        write(
            &directory.join("Ext/Aggregates.xml"),
            codec::sidecar_envelope(codec::emit_aggregates(values, false), &version),
        )?;
    }
    if let Some(values) = &obj.source_extensions.calculation_predefined {
        write(
            &directory.join("Ext/Predefined.xml"),
            codec::sidecar_envelope(codec::emit_calculation_predefined(values, false), &version),
        )?;
    }
    for r in &obj.source_extensions.recalculations {
        let mut root =
            formats_designer::common::emit_root_envelope(target).map_err(|e| error(path, e))?;
        root.push(codec::emit_recalculation(r, false, Some(&obj.name)));
        write(
            &directory
                .join("Recalculations")
                .join(format!("{}.xml", r.name)),
            root,
        )?;
    }
    Ok(())
}
