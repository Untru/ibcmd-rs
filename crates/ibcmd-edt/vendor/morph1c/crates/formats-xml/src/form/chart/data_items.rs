//! Typed current mcore values and persisted RealDataItem collections.
//! No source values or opaque XML are retained by these projections.
use super::*;
use crate::type_codec::{self, TypeDialect};
use crate::value_codec::{self, ValueDialect};
use morph1c_core::ir::Uuid;
use morph1c_core::ir::form::ChartTypedValue;
use morph1c_core::ir::value::{PropertyValue, ValueScalarKind, ValueSpec};

/// Check expanded value QNames before the prefix-based field projections run.
/// Native Chart values inherit the logform default namespace until a collection
/// member resets it to the empty URI, as the original SDK stream writer does.
pub(super) fn validate_value_namespaces(root: &Element, native: bool) -> Result<(), FormError> {
    use std::collections::BTreeMap;
    fn walk(
        el: &Element,
        inherited: &BTreeMap<String, String>,
        member: bool,
    ) -> Result<(), FormError> {
        let mut ns = inherited.clone();
        for a in &el.attrs {
            if a.name == "xmlns" {
                ns.insert(String::new(), a.value.clone());
            } else if let Some(p) = a.name.strip_prefix("xmlns:") {
                ns.insert(p.into(), a.value.clone());
            }
        }
        let element_uri = ns.get(&el.prefix).cloned().unwrap_or_default();
        *el.resolved_name.borrow_mut() = Some((element_uri.clone(), el.local.clone()));
        if element_uri == "http://v8.1c.ru/8.1/data/core"
            && matches!(el.local.as_str(), "Type" | "TypeSet")
        {
            let (prefix, local) = el
                .text
                .trim()
                .split_once(':')
                .unwrap_or(("", el.text.trim()));
            if let Some(uri) = ns.get(prefix) {
                *el.resolved_text.borrow_mut() = Some((uri.clone(), local.into()));
            }
        }
        if member {
            if !el.prefix.is_empty()
                || el.local != "values"
                || ns.get("").is_some_and(|uri| !uri.is_empty())
            {
                return Err(frame(
                    "chart typed collection member requires empty namespace URI".into(),
                ));
            }
            if let Some(a) = el.attr("xmlns") {
                a.claimed.set(true);
            }
        }
        for name in ["xsi:type", "xsi:nil"] {
            if el.attr(name).is_some() && ns.get("xsi").map(String::as_str) != Some(XSI_NS_URI) {
                return Err(frame(
                    "chart typed value requires XML Schema instance namespace URI".into(),
                ));
            }
        }
        if let Some(a) = el.attr("xsi:type") {
            if !a.value.contains(':') {
                *el.resolved_type.borrow_mut() =
                    Some((ns.get("").cloned().unwrap_or_default(), a.value.clone()));
            }
            if let Some((prefix, local)) = a.value.split_once(':') {
                let uri = ns
                    .get(prefix)
                    .ok_or_else(|| frame("chart typed value has unbound type prefix".into()))?;
                *el.resolved_type.borrow_mut() = Some((uri.clone(), local.into()));
                let expected = match prefix {
                    "core" => Some(CORE_NS_URI),
                    "xs" => Some("http://www.w3.org/2001/XMLSchema"),
                    "v8" => Some("http://v8.1c.ru/8.1/data/core"),
                    "xr" => Some("http://v8.1c.ru/8.3/xcf/readable"),
                    "v8ui" => Some("http://v8.1c.ru/8.1/data/ui"),
                    "common" => Some("http://g5.1c.ru/v8/dt/metadata/common"),
                    _ => None,
                };
                if let Some(uri) = expected {
                    if ns.get(prefix).map(String::as_str) != Some(uri) {
                        return Err(frame("chart typed value has wrong namespace URI".into()));
                    }
                }
            }
        }
        let collection = el
            .resolved_type
            .borrow()
            .as_ref()
            .is_some_and(|(uri, local)| {
                matches!(
                    (uri.as_str(), local.as_str()),
                    (CORE_NS_URI, "ValueList" | "FixedArrayValue")
                        | ("http://v8.1c.ru/8.3/xcf/readable", "ValueList")
                        | ("http://v8.1c.ru/8.1/data/core", "FixedArray")
                )
            });
        for child in &el.children {
            walk(child, &ns, collection)?;
        }
        Ok(())
    }
    let mut ns = BTreeMap::new();
    if native {
        for (name, uri) in super::super::DESIGNER_FORM_NS {
            ns.insert(
                name.strip_prefix("xmlns:").unwrap_or("").into(),
                (*uri).into(),
            );
        }
    }
    walk(root, &ns, false)
}

fn dialect(native: bool) -> ValueDialect {
    if native {
        ValueDialect::Designer
    } else {
        ValueDialect::Edt
    }
}

fn mark_type(el: &Element, ty: &str, path: &str) -> Result<(), FormError> {
    el.claim();
    claim_xsi(el, ty, path)?;
    expect_attrs_claimed(el, path)?;
    if !el.text.is_empty() {
        return Err(frame(format!(
            "chart {path}: text in typed value container"
        )));
    }
    Ok(())
}

fn scalar_text(el: &Element, ty: &str, path: &str, native: bool) -> Result<String, FormError> {
    el.claim();
    claim_xsi(el, ty, path)?;
    expect_attrs_claimed(el, path)?;
    if native {
        if !el.children.is_empty() {
            return Err(frame(format!("chart {path}: scalar value has children")));
        }
        el.claim_text();
        Ok(el.text.clone())
    } else {
        if !el.text.is_empty() {
            return Err(frame(format!("chart {path}: text outside scalar value")));
        }
        match el.children.as_slice() {
            [] => Ok(String::new()),
            [value] if value.prefix.is_empty() && value.local == "value" => leaf_text(value, path),
            _ => Err(frame(format!("chart {path}: malformed scalar value child"))),
        }
    }
}

fn wrapped_value_child<'a>(
    el: &'a Element,
    ty: &str,
    path: &str,
) -> Result<&'a Element, FormError> {
    mark_type(el, ty, path)?;
    match el.children.as_slice() {
        [value] if value.prefix.is_empty() && value.local == "value" => Ok(value),
        _ => Err(frame(format!(
            "chart {path}: typed wrapper requires one value child"
        ))),
    }
}

fn uuid(text: &str, path: &str) -> Result<Uuid, FormError> {
    if text.len() != 36 || ![8, 13, 18, 23].iter().all(|i| text.as_bytes()[*i] == b'-') {
        return Err(frame(format!("chart {path}: malformed reference UUID")));
    }
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(frame(format!("chart {path}: malformed reference UUID")));
    }
    let mut bytes = [0; 16];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|_| frame(format!("chart {path}: malformed reference UUID")))?;
    }
    Ok(Uuid(bytes))
}
fn uuid_text(value: &Uuid) -> String {
    let hex: String = value.0.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

/// Read a value's actual scalar kind, ordered collection or full type description.
pub(super) fn read_value(el: &Element, path: &str, native: bool) -> Result<ChartValue, FormError> {
    if native && el.resolved_type.borrow().is_some() {
        let mut normalized = el.clone();
        normalize_native_value(&mut normalized);
        let value = read_value_inner(&normalized, path, native)?;
        if normalized.unclaimed_count() != 0 {
            return Err(frame(format!(
                "chart {path}: unconsumed typed value content"
            )));
        }
        transfer_claims(el, &normalized);
        return Ok(value);
    }
    read_value_inner(el, path, native)
}

/// Resolve native aliases into the shared codecs' conventional spellings. Only
/// namespace names change; every value and every unknown node still reaches a
/// strict typed decoder. Successful claims are copied back one-for-one.
fn normalize_native_value(el: &mut Element) {
    fn prefix(uri: &str) -> Option<&'static str> {
        super::super::DESIGNER_FORM_NS
            .iter()
            .find_map(|(name, actual)| {
                (*actual == uri).then(|| name.strip_prefix("xmlns:").unwrap_or(""))
            })
    }
    if let Some((uri, _)) = el.resolved_name.borrow().as_ref() {
        if let Some(p) = prefix(uri) {
            el.prefix = p.into();
        }
    }
    let ty = el.resolved_type.borrow().clone();
    if let Some((uri, local)) = ty {
        if let Some(p) = prefix(&uri) {
            if let Some(a) = el.attrs.iter_mut().find(|a| a.name == "xsi:type") {
                a.value = if p.is_empty() {
                    local
                } else {
                    format!("{p}:{local}")
                };
            }
        }
    }
    if let Some((uri, local)) = el.resolved_text.borrow().as_ref() {
        if let Some(p) = prefix(uri) {
            el.text = if p.is_empty() {
                local.clone()
            } else {
                format!("{p}:{local}")
            };
        }
    }
    for a in &el.attrs {
        if (a.name == "xmlns" || a.name.starts_with("xmlns:"))
            && (a.value.is_empty() || prefix(&a.value).is_some())
        {
            a.claimed.set(true);
        }
    }
    for child in &mut el.children {
        normalize_native_value(child);
    }
}
fn transfer_claims(original: &Element, normalized: &Element) {
    original.claimed.set(normalized.claimed.get());
    original.text_claimed.set(normalized.text_claimed.get());
    for (a, b) in original.attrs.iter().zip(&normalized.attrs) {
        a.claimed.set(b.claimed.get());
    }
    for (a, b) in original.children.iter().zip(&normalized.children) {
        transfer_claims(a, b);
    }
}
fn read_value_inner(el: &Element, path: &str, native: bool) -> Result<ChartValue, FormError> {
    let ty = el.attr("xsi:type").map(|a| a.value.as_str());
    if !native && ty.is_some_and(super::value_enum::is_edt_type) {
        return super::value_enum::read(el, path, false).map(|v| ChartValue::Value(Box::new(v)));
    }
    if ty
        == Some(if native {
            "v8:StandardPeriod"
        } else {
            "core:StandardPeriodValue"
        })
    {
        return super::value_period::read(el, path, native).map(|v| ChartValue::Value(Box::new(v)));
    }
    if native && ty == Some("v8ui:ChartLineType") {
        return super::value_enum::read(el, path, true).map(|v| ChartValue::Value(Box::new(v)));
    }
    if !native && ty == Some("core:IrresolvableReferenceValue") {
        mark_type(el, "core:IrresolvableReferenceValue", path)?;
        let mut ids = [None, None];
        for child in &el.children {
            let index = match (child.prefix.as_str(), child.local.as_str()) {
                ("", "refTypeId") => 0,
                ("", "instanceId") => 1,
                _ => {
                    return Err(frame(format!(
                        "chart {path}: unknown unresolved reference field"
                    )));
                }
            };
            if ids[index].is_some() {
                return Err(frame(format!(
                    "chart {path}: duplicate unresolved reference field"
                )));
            }
            ids[index] = Some(uuid(&leaf_text(child, path)?, path)?);
        }
        let [Some(ref_type_id), Some(instance_id)] = ids else {
            return Err(frame(format!(
                "chart {path}: incomplete unresolved reference identities"
            )));
        };
        return Ok(ChartValue::Value(Box::new(
            ChartTypedValue::IrresolvableReference {
                ref_type_id,
                instance_id,
            },
        )));
    }
    let reference = if native {
        matches!(ty, Some("xr:MDObjectRef" | "xr:DesignTimeRef"))
    } else {
        ty == Some("core:ReferenceValue")
    };
    if reference {
        let text = scalar_text(el, ty.unwrap(), path, native)?;
        if native && ty == Some("xr:DesignTimeRef") {
            if let Some((left, right)) = text.split_once('.') {
                if let (Ok(ref_type_id), Ok(instance_id)) = (uuid(left, path), uuid(right, path)) {
                    return Ok(ChartValue::Value(Box::new(
                        ChartTypedValue::IrresolvableReference {
                            ref_type_id,
                            instance_id,
                        },
                    )));
                }
            }
        }
        return Ok(ChartValue::Value(Box::new(ChartTypedValue::Reference(
            (!text.is_empty()).then_some(text),
        ))));
    }
    let binary = if native {
        "xs:base64Binary"
    } else {
        "core:BinaryValue"
    };
    if ty == Some(binary) {
        return Ok(ChartValue::Value(Box::new(ChartTypedValue::Binary(
            scalar_text(el, binary, path, native)?,
        ))));
    }
    let font = if native {
        "v8ui:Font"
    } else {
        "core:FontValue"
    };
    if ty == Some(font) {
        let current = if native {
            el.claim();
            claim_xsi(el, font, path)?;
            chart_read_designer_font(el, path)?
        } else {
            chart_read_edt_font(wrapped_value_child(el, font, path)?, path)?
        };
        let ChartValue::Font(font) = current else {
            unreachable!()
        };
        return Ok(ChartValue::Value(Box::new(ChartTypedValue::Font(
            normalize_chart_font(font),
        ))));
    }
    let color = if native {
        "v8ui:Color"
    } else {
        "core:ColorValue"
    };
    if ty == Some(color) {
        let current = if native {
            let current = scalar_text(el, color, path, true)?;
            check_color_canon(&current, path)?;
            current
        } else {
            let ChartValue::Color(current) =
                chart_read_edt_color(wrapped_value_child(el, color, path)?, path)?
            else {
                unreachable!()
            };
            current
        };
        return Ok(ChartValue::Value(Box::new(ChartTypedValue::Color(current))));
    }
    let border = if native {
        "v8ui:Border"
    } else {
        "core:BorderValue"
    };
    if ty == Some(border) {
        return Ok(ChartValue::Value(Box::new(super::value_border::read(
            el, path, native,
        )?)));
    }
    let null = if native { "v8:Null" } else { "core:NullValue" };
    if ty == Some(null) {
        mark_type(el, null, path)?;
        if !el.children.is_empty() {
            return Err(frame(format!("chart {path}: Null value has children")));
        }
        return Ok(ChartValue::Value(Box::new(ChartTypedValue::Null)));
    }
    let list = if native {
        "xr:ValueList"
    } else {
        "core:ValueList"
    };
    let fixed = if native {
        "v8:FixedArray"
    } else {
        "core:FixedArrayValue"
    };
    if ty == Some(list) || ty == Some(fixed) {
        let fixed_array = ty == Some(fixed);
        mark_type(el, if fixed_array { fixed } else { list }, path)?;
        let mut values = Vec::new();
        for child in &el.children {
            // ChartExportXmlModule injects ChartFeatureNameProvider: both ordered
            // mcore collections use the unqualified local name `values`.
            let expected = ("", "values");
            if (child.prefix.as_str(), child.local.as_str()) != expected {
                return Err(frame(format!(
                    "chart {path}: unrecognized typed collection child"
                )));
            }
            let ChartValue::Value(value) = read_value(child, path, native)? else {
                unreachable!()
            };
            values.push(*value);
        }
        return Ok(ChartValue::Value(Box::new(if fixed_array {
            ChartTypedValue::FixedArray(values)
        } else {
            ChartTypedValue::ValueList(values)
        })));
    }
    let types = if native {
        "v8:TypeDescription"
    } else {
        "core:TypeDescriptionValue"
    };
    if ty == Some(types) {
        mark_type(el, types, path)?;
        let (host, td) = if native {
            (el, TypeDialect::Designer)
        } else {
            let [value] = el.children.as_slice() else {
                return Err(frame(format!(
                    "chart {path}: TypeDescription needs one value child"
                )));
            };
            if !value.prefix.is_empty() || value.local != "value" {
                return Err(frame(format!(
                    "chart {path}: invalid TypeDescription child"
                )));
            }
            value.claim();
            expect_attrs_claimed(value, path)?;
            (value, TypeDialect::Edt)
        };
        let value = type_codec::decode(td, host).map_err(frame)?;
        type_codec::claim(td, host);
        return Ok(ChartValue::Value(Box::new(ChartTypedValue::Scalar(value))));
    }
    // The EBoolean model also permits an authored explicit false child.
    if !native && ty == Some("core:BooleanValue") && !el.children.is_empty() {
        mark_type(el, "core:BooleanValue", path)?;
        let [child] = el.children.as_slice() else {
            return Err(frame(format!(
                "chart {path}: BooleanValue has multiple children"
            )));
        };
        if !child.prefix.is_empty() || child.local != "value" {
            return Err(frame(format!("chart {path}: invalid BooleanValue child")));
        }
        let value = parse_bool(&leaf_text(child, path)?, path)?;
        return Ok(ChartValue::Value(Box::new(ChartTypedValue::Scalar(
            PropertyValue::Value(ValueSpec {
                kind: ValueScalarKind::Bool,
                scalar: Some(Box::new(PropertyValue::Bool(value))),
            }),
        ))));
    }
    el.claim();
    let mut value = match value_codec::decode(dialect(native), el) {
        Ok(value) => value,
        Err(original_error) => {
            if native {
                if let Some((uri, local)) = el.resolved_type.borrow().as_ref() {
                    if super::value_enum::validate_native_qname(local, uri) {
                        return super::value_enum::read_native_resolved(el, path, uri)
                            .map(|v| ChartValue::Value(Box::new(v)));
                    }
                }
            }
            return Err(frame(original_error));
        }
    };
    value_codec::claim(dialect(native), el);
    expect_attrs_claimed(el, path)?;
    if let PropertyValue::Value(spec) = &mut value {
        if matches!(spec.kind, ValueScalarKind::Str | ValueScalarKind::Reference)
            && spec.scalar.is_none()
        {
            spec.scalar = Some(Box::new(PropertyValue::Str(String::new())));
        }
        if spec.kind == ValueScalarKind::Number {
            if let Some(PropertyValue::Str(text)) = spec.scalar.as_deref_mut() {
                let normalized = super::semantic::normalize_big_decimal(text).ok_or_else(|| {
                    frame(format!("chart {path}: invalid NumberValue BigDecimal"))
                })?;
                *text = designer_decimal(&normalized);
            }
        }
    }
    Ok(ChartValue::Value(Box::new(ChartTypedValue::Scalar(value))))
}

/// Rebuild a value from its current kind and payload.
pub(super) fn write_value(
    name: &str,
    value: &ChartValue,
    path: &str,
    native: bool,
) -> Result<OutElement, FormError> {
    let ChartValue::Value(value) = value else {
        return Err(frame(format!("chart {path}: expected typed mcore value")));
    };
    let prefix = if native { "d4p1" } else { "" };
    match value.as_ref() {
        ChartTypedValue::Enum { .. }
        | ChartTypedValue::ProjectedEnum { .. }
        | ChartTypedValue::SysEnum(_)
        | ChartTypedValue::ChartLineType(_) => super::value_enum::write(name, value, path, native),
        ChartTypedValue::StandardPeriod { .. } => {
            super::value_period::write(name, value, path, native)
        }
        ChartTypedValue::Null => Ok(OutElement::self_closing(prefix, name).attr(
            "xsi:type",
            if native { "v8:Null" } else { "core:NullValue" },
        )),
        ChartTypedValue::Binary(text) => {
            let mut host = OutElement::branch(prefix, name).attr(
                "xsi:type",
                if native {
                    "xs:base64Binary"
                } else {
                    "core:BinaryValue"
                },
            );
            if native {
                host.text = Some(text.clone());
            } else if !text.is_empty() {
                host.push(OutElement::leaf("", "value", text.clone()));
            }
            host.self_closing = host.children.is_empty() && text.is_empty();
            Ok(host)
        }
        ChartTypedValue::Reference(reference) => {
            if native && reference.is_none() {
                return Err(frame(format!(
                    "chart {path}: nullable reference requires bound semantic transport"
                )));
            }
            let mut host = OutElement::branch(prefix, name).attr(
                "xsi:type",
                if native {
                    if name == "values" {
                        "xr:DesignTimeRef"
                    } else {
                        "xr:MDObjectRef"
                    }
                } else {
                    "core:ReferenceValue"
                },
            );
            if let Some(text) = reference {
                if text.is_empty() {
                    return Err(frame(format!(
                        "chart {path}: empty current reference must be nullable"
                    )));
                }
                if native {
                    host.text = Some(text.clone());
                } else {
                    host.push(OutElement::leaf("", "value", text.clone()));
                }
            } else {
                host.self_closing = true;
            }
            Ok(host)
        }
        ChartTypedValue::IrresolvableReference {
            ref_type_id,
            instance_id,
        } => {
            if native {
                Ok(OutElement::leaf(
                    prefix,
                    name,
                    format!("{}.{}", uuid_text(ref_type_id), uuid_text(instance_id)),
                )
                .attr("xsi:type", "xr:DesignTimeRef"))
            } else {
                let mut host = OutElement::branch("", name)
                    .attr("xsi:type", "core:IrresolvableReferenceValue");
                host.push(OutElement::leaf("", "refTypeId", uuid_text(ref_type_id)));
                host.push(OutElement::leaf("", "instanceId", uuid_text(instance_id)));
                Ok(host)
            }
        }
        ChartTypedValue::Font(font) => {
            if native {
                Ok(chart_designer_font_out(name, font, path)?.attr("xsi:type", "v8ui:Font"))
            } else {
                let mut host = OutElement::branch("", name).attr("xsi:type", "core:FontValue");
                host.push(chart_edt_font_out("value", font, path)?);
                Ok(host)
            }
        }
        ChartTypedValue::Color(color) => {
            check_color_canon(color, path)?;
            if native {
                Ok(OutElement::leaf(prefix, name, color.clone()).attr("xsi:type", "v8ui:Color"))
            } else {
                let mut host = OutElement::branch("", name).attr("xsi:type", "core:ColorValue");
                host.push(chart_edt_color_out("value", color, path)?);
                Ok(host)
            }
        }
        ChartTypedValue::Border { .. } | ChartTypedValue::BorderRef(_) => {
            super::value_border::write(name, value, path, native)
        }
        ChartTypedValue::ValueList(values) | ChartTypedValue::FixedArray(values) => {
            let fixed = matches!(value.as_ref(), ChartTypedValue::FixedArray(_));
            let ty = match (fixed, native) {
                (false, true) => "xr:ValueList",
                (true, true) => "v8:FixedArray",
                (false, false) => "core:ValueList",
                (true, false) => "core:FixedArrayValue",
            };
            let mut host = OutElement::branch(prefix, name).attr("xsi:type", ty);
            for current in values {
                let mut child = write_value(
                    "values",
                    &ChartValue::Value(Box::new(current.clone())),
                    path,
                    native,
                )?;
                child.prefix = String::new();
                if native {
                    child.attrs.insert(0, ("xmlns".into(), String::new()));
                }
                host.push(child);
            }
            if host.children.is_empty() {
                host.self_closing = true;
            }
            Ok(host)
        }
        ChartTypedValue::Scalar(PropertyValue::Type(spec)) => {
            if native {
                Ok(
                    type_codec::encode(TypeDialect::Designer, prefix, name, spec)
                        .map_err(frame)?
                        .attr("xsi:type", "v8:TypeDescription"),
                )
            } else {
                let mut host =
                    OutElement::branch("", name).attr("xsi:type", "core:TypeDescriptionValue");
                host.push(type_codec::encode(TypeDialect::Edt, "", "value", spec).map_err(frame)?);
                Ok(host)
            }
        }
        ChartTypedValue::Scalar(PropertyValue::Value(spec)) => {
            if spec.kind == ValueScalarKind::Number {
                if let Some(PropertyValue::Str(text)) = spec.scalar.as_deref() {
                    let normalized = super::semantic::normalize_big_decimal(text).ok_or_else(|| {
                        frame(format!("chart {path}: invalid current NumberValue BigDecimal"))
                    })?;
                    let mut current = spec.clone();
                    current.scalar = Some(Box::new(PropertyValue::Str(designer_decimal(&normalized))));
                    return value_codec::encode(dialect(native), prefix, name, &current).map_err(frame);
                }
            }
            value_codec::encode(dialect(native), prefix, name, spec).map_err(frame)
        }
        ChartTypedValue::Scalar(PropertyValue::StyleValue(spec)) => {
            crate::style_value_codec::encode(
                if native {
                    crate::style_value_codec::StyleValueDialect::Designer
                } else {
                    crate::style_value_codec::StyleValueDialect::Edt
                },
                prefix,
                name,
                spec,
            )
            .map_err(frame)
        }
        _ => Err(frame(format!(
            "chart {path}: malformed current mcore value"
        ))),
    }
}

fn defaults() -> Vec<(String, ChartValue)> {
    vec![
        ("dataValue".into(), ChartValue::Absent),
        ("infoValue".into(), ChartValue::Absent),
        ("tooltip".into(), ChartValue::Absent),
        ("isToolTipFormatted".into(), ChartValue::Bool(false)),
        ("editMode".into(), ChartValue::Enum("Auto".into())),
    ]
}

fn read_item(
    el: &Element,
    path: &str,
    native: bool,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    el.claim();
    expect_attrs_claimed(el, path)?;
    if !el.text.is_empty() {
        return Err(frame(format!("chart {path}: text in data item")));
    }
    let mut fields = defaults();
    let mut seen = Vec::new();
    for child in &el.children {
        if child.prefix != if native { "d4p1" } else { "" } {
            return Err(frame(format!(
                "chart {path}: unrecognized data item namespace"
            )));
        }
        let name = match (child.local.as_str(), native) {
            ("valData", true) | ("dataValue", _) => "dataValue",
            ("valInfo", true) | ("infoValue", _) => "infoValue",
            ("toolTip", true) | ("tooltip", _) => "tooltip",
            ("isToolTipFormatted", false) => "isToolTipFormatted",
            ("editMode", _) => "editMode",
            _ => return Err(frame(format!("chart {path}: unrecognized data item field"))),
        };
        if seen.contains(&name) {
            return Err(frame(format!(
                "chart {path}: duplicate data item field {name}"
            )));
        }
        seen.push(name);
        let value = match name {
            "dataValue" | "infoValue" => read_value(child, path, native)?,
            "tooltip" => ChartValue::Str(leaf_text(child, path)?),
            "isToolTipFormatted" => ChartValue::Bool(parse_bool(&leaf_text(child, path)?, path)?),
            "editMode" => {
                let mode = leaf_text(child, path)?;
                if !["Auto", "Use", "DontUse"].contains(&mode.as_str()) {
                    return Err(frame(format!("chart {path}: invalid data item edit mode")));
                }
                ChartValue::Enum(mode)
            }
            _ => unreachable!(),
        };
        fields.iter_mut().find(|(n, _)| n == name).unwrap().1 = value;
    }
    Ok(fields)
}

/// A repeated EDT realDataItems element represents one persisted item.
pub(super) fn read_edt(el: &Element, path: &str) -> Result<Vec<(String, ChartValue)>, FormError> {
    read_item(el, path, false)
}

/// A native realDataItems container preserves the complete ordered item list.
pub(super) fn read_native(el: &Element, path: &str) -> Result<ChartValue, FormError> {
    el.claim();
    expect_attrs_claimed(el, path)?;
    if !el.text.is_empty() {
        return Err(frame(format!("chart {path}: text in data item collection")));
    }
    let mut items = Vec::new();
    for item in &el.children {
        if item.prefix != "d4p1" || item.local != "item" {
            return Err(frame(format!(
                "chart {path}: unrecognized data item collection child"
            )));
        }
        items.push(read_item(item, path, true)?);
    }
    Ok(ChartValue::Items(items))
}

fn write_item(
    name: &str,
    fields: &[(String, ChartValue)],
    path: &str,
    native: bool,
) -> Result<OutElement, FormError> {
    let mut host = OutElement::branch(if native { "d4p1" } else { "" }, name);
    let defaults = defaults();
    if fields.len() != defaults.len()
        || fields
            .iter()
            .any(|(n, _)| defaults.iter().all(|(d, _)| d != n))
    {
        return Err(frame(format!(
            "chart {path}: incomplete or unrecognized current data item fields"
        )));
    }
    for (name, default) in defaults {
        let current: Vec<_> = fields
            .iter()
            .filter(|(n, _)| n == &name)
            .map(|(_, v)| v)
            .collect();
        let [value] = current.as_slice() else {
            return Err(frame(format!(
                "chart {path}: duplicate or missing current data item field"
            )));
        };
        let value = *value;
        if value == &default {
            continue;
        }
        match (name.as_str(), value) {
            ("dataValue" | "infoValue", value) => {
                host.push(write_value(&name, value, path, native)?)
            }
            ("tooltip", ChartValue::Str(text)) => {
                let mut child =
                    OutElement::leaf(if native { "d4p1" } else { "" }, &name, text.clone());
                if text.is_empty() {
                    child.self_closing = true;
                }
                host.push(child);
            }
            ("isToolTipFormatted", ChartValue::Bool(formatted)) => {
                if native {
                    if *formatted {
                        return Err(frame(format!(
                            "chart {path}: formatted data-item tooltip requires bound semantic transport"
                        )));
                    }
                } else {
                    host.push(OutElement::leaf("", &name, formatted.to_string()));
                }
            }
            ("editMode", ChartValue::Enum(mode))
                if ["Auto", "Use", "DontUse"].contains(&mode.as_str()) =>
            {
                if mode != "Auto" {
                    host.push(OutElement::leaf(
                        if native { "d4p1" } else { "" },
                        &name,
                        mode.clone(),
                    ));
                }
            }
            _ => {
                return Err(frame(format!(
                    "chart {path}: wrong current data item field kind"
                )));
            }
        }
    }
    if host.children.is_empty() {
        host.self_closing = true;
    }
    Ok(host)
}

pub(super) fn write_edt(
    name: &str,
    fields: &[(String, ChartValue)],
    path: &str,
) -> Result<OutElement, FormError> {
    write_item(name, fields, path, false)
}
pub(super) fn write_native(
    name: &str,
    items: &[Vec<(String, ChartValue)>],
    path: &str,
) -> Result<OutElement, FormError> {
    let mut host = OutElement::branch("d4p1", name);
    for item in items {
        host.push(write_item("item", item, path, true)?);
    }
    if items.is_empty() {
        host.self_closing = true;
    }
    Ok(host)
}
