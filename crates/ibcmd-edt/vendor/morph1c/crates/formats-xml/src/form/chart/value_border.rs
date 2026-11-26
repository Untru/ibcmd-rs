//! Current BorderValue: the original SDK supports both BorderDef and BorderRef.
use super::*;
use morph1c_core::ir::form::ChartTypedValue;

// Complete Mcore.BorderStyle literal domain, rather than a corpus witness list.
const STYLES: &[&str] = &[
    "WithoutBorder",
    "Single",
    "Double",
    "Embossed",
    "Indented",
    "Underline",
    "DoubleUnderline",
    "Rounded",
    "Overline",
];
fn definition(style: &str, width: &str, path: &str) -> Result<ChartTypedValue, FormError> {
    if !STYLES.contains(&style) {
        return Err(frame(format!("chart {path}: unknown BorderStyle literal")));
    }
    let width = width
        .parse::<i32>()
        .map_err(|_| frame(format!("chart {path}: BorderDef width is not an EInt")))?;
    Ok(ChartTypedValue::Border {
        style: style.into(),
        width: width.to_string(),
    })
}
fn reference(value: &str, native: bool, path: &str) -> Result<ChartTypedValue, FormError> {
    let prefix = if native { "style:" } else { "Style." };
    let name = value
        .strip_prefix(prefix)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            frame(format!(
                "chart {path}: invalid BorderRef symbolic namespace/empty target"
            ))
        })?;
    Ok(ChartTypedValue::BorderRef(format!("Style.{name}")))
}
pub(super) fn read(el: &Element, path: &str, native: bool) -> Result<ChartTypedValue, FormError> {
    el.claim();
    claim_xsi(
        el,
        if native {
            "v8ui:Border"
        } else {
            "core:BorderValue"
        },
        path,
    )?;
    if !el.text.is_empty() {
        return Err(frame(format!(
            "chart {path}: text in BorderValue container"
        )));
    }
    if native {
        if let Some(attr) = el.attr("ref") {
            attr.claimed.set(true);
            if !el.children.is_empty() {
                return Err(frame(format!(
                    "chart {path}: BorderRef has definition children"
                )));
            }
            expect_attrs_claimed(el, path)?;
            return reference(&attr.value, true, path);
        }
        let width = attr_req(el, "width", path)?;
        expect_attrs_claimed(el, path)?;
        let [style] = el.children.as_slice() else {
            return Err(frame(format!(
                "chart {path}: BorderDef requires one style child"
            )));
        };
        if style.prefix != "v8ui" || style.local != "style" {
            return Err(frame(format!(
                "chart {path}: invalid native BorderDef style role"
            )));
        }
        claim_xsi(style, "v8ui:ControlBorderType", path)?;
        definition(&leaf_text(style, path)?, &width, path)
    } else {
        expect_attrs_claimed(el, path)?;
        let [value] = el.children.as_slice() else {
            return Err(frame(format!(
                "chart {path}: BorderValue requires one value child"
            )));
        };
        if !value.prefix.is_empty() || value.local != "value" {
            return Err(frame(format!("chart {path}: invalid EDT BorderValue role")));
        }
        value.claim();
        if !value.text.is_empty() {
            return Err(frame(format!(
                "chart {path}: text in Border value container"
            )));
        }
        match value.attr("xsi:type").map(|a| a.value.as_str()) {
            Some("core:BorderRef") => {
                claim_xsi(value, "core:BorderRef", path)?;
                expect_attrs_claimed(value, path)?;
                let [border] = value.children.as_slice() else {
                    return Err(frame(format!(
                        "chart {path}: BorderRef requires one border child"
                    )));
                };
                if !border.prefix.is_empty() || border.local != "border" {
                    return Err(frame(format!(
                        "chart {path}: invalid BorderRef target role"
                    )));
                }
                reference(&leaf_text(border, path)?, false, path)
            }
            Some("core:BorderDef") => {
                claim_xsi(value, "core:BorderDef", path)?;
                expect_attrs_claimed(value, path)?;
                let mut style = None;
                let mut width = None;
                for child in &value.children {
                    if !child.prefix.is_empty() {
                        return Err(frame(format!(
                            "chart {path}: invalid BorderDef field namespace"
                        )));
                    }
                    match child.local.as_str() {
                        "style" if style.is_none() => style = Some(leaf_text(child, path)?),
                        "width" if width.is_none() => width = Some(leaf_text(child, path)?),
                        _ => {
                            return Err(frame(format!(
                                "chart {path}: unknown/duplicate BorderDef field"
                            )));
                        }
                    }
                }
                definition(
                    style.as_deref().unwrap_or("WithoutBorder"),
                    width.as_deref().unwrap_or("0"),
                    path,
                )
            }
            _ => Err(frame(format!(
                "chart {path}: unknown BorderValue concrete type"
            ))),
        }
    }
}
pub(super) fn write(
    name: &str,
    value: &ChartTypedValue,
    path: &str,
    native: bool,
) -> Result<OutElement, FormError> {
    let prefix = if native { "d4p1" } else { "" };
    match value {
        ChartTypedValue::BorderRef(current) => {
            let ChartTypedValue::BorderRef(current) = reference(current, false, path)? else {
                unreachable!()
            };
            if native {
                Ok(OutElement::self_closing(prefix, name)
                    .attr("xsi:type", "v8ui:Border")
                    .attr(
                        "ref",
                        format!("style:{}", current.strip_prefix("Style.").unwrap()),
                    ))
            } else {
                let mut outer = OutElement::branch("", name).attr("xsi:type", "core:BorderValue");
                let mut inner = OutElement::branch("", "value").attr("xsi:type", "core:BorderRef");
                inner.push(OutElement::leaf("", "border", current));
                outer.push(inner);
                Ok(outer)
            }
        }
        ChartTypedValue::Border { style, width } => {
            let ChartTypedValue::Border { style, width } = definition(style, width, path)? else {
                unreachable!()
            };
            if native {
                let mut outer = OutElement::branch(prefix, name)
                    .attr("xsi:type", "v8ui:Border")
                    .attr("width", width);
                outer.push(
                    OutElement::leaf("v8ui", "style", style)
                        .attr("xsi:type", "v8ui:ControlBorderType"),
                );
                Ok(outer)
            } else {
                let mut outer = OutElement::branch("", name).attr("xsi:type", "core:BorderValue");
                let mut inner = OutElement::branch("", "value").attr("xsi:type", "core:BorderDef");
                if style != "WithoutBorder" {
                    inner.push(OutElement::leaf("", "style", style));
                }
                if width != "0" {
                    inner.push(OutElement::leaf("", "width", width));
                }
                inner.self_closing = inner.children.is_empty();
                outer.push(inner);
                Ok(outer)
            }
        }
        _ => Err(frame(format!(
            "chart {path}: expected current typed BorderValue"
        ))),
    }
}
