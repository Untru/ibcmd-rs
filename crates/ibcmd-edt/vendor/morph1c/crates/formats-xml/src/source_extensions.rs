//! Closed typed grammars for BSP source features absent from the pinned codecs.
use crate::{Element, OutElement};
use morph1c_core::ir::source_extensions::*;
fn out_leaf(
    prefix: impl Into<String>,
    local: impl Into<String>,
    value: impl Into<String>,
) -> OutElement {
    let value = value.into();
    if value.is_empty() {
        OutElement::self_closing(prefix, local)
    } else {
        OutElement::leaf(prefix, local, value)
    }
}
fn err(e: &Element, s: &str) -> String {
    format!("source extension <{}>: {s}", e.local)
}
fn branch(e: &Element) -> Result<(), String> {
    if !e.prefix.is_empty()
        && !(e.prefix == "v8" && e.local == "item")
        && !(e.prefix == "xr" && e.local == "GeneratedType")
    {
        return Err(err(e, "unexpected container namespace prefix"));
    }
    if !e.text.is_empty() {
        return Err(err(e, "unexpected container text"));
    }
    e.claim();
    Ok(())
}
fn check(e: &Element) -> Result<(), String> {
    if e.unclaimed_count() != 0 {
        Err(err(
            e,
            &format!("uncovered cells: {:?}", e.unclaimed_names(8)),
        ))
    } else {
        Ok(())
    }
}
fn leaf(e: &Element) -> Result<String, String> {
    let prefix = match e.local.as_str() {
        "lang" | "content" => "v8",
        "TypeId" | "ValueId" => "xr",
        _ => "",
    };
    if e.prefix != prefix {
        return Err(err(e, "unexpected leaf namespace prefix"));
    }
    if !e.attrs.is_empty() || !e.children.is_empty() {
        return Err(err(e, "expected plain text leaf"));
    }
    e.claim_with_text();
    Ok(e.text.clone())
}
fn optional<'a>(e: &'a Element, tag: &str) -> Result<Option<&'a Element>, String> {
    let v = e
        .children
        .iter()
        .filter(|c| c.local == tag)
        .collect::<Vec<_>>();
    if v.len() > 1 {
        Err(err(e, &format!("duplicate {tag}")))
    } else {
        Ok(v.first().copied())
    }
}
fn required<'a>(e: &'a Element, tag: &str) -> Result<&'a Element, String> {
    optional(e, tag)?.ok_or_else(|| err(e, &format!("missing {tag}")))
}
fn text(e: &Element, tag: &str, default: &str) -> Result<String, String> {
    optional(e, tag)?
        .map(leaf)
        .unwrap_or_else(|| Ok(default.into()))
}
fn attr(e: &Element, name: &str) -> Result<String, String> {
    let a = e
        .attr(name)
        .ok_or_else(|| err(e, &format!("missing @{name}")))?;
    a.claimed.set(true);
    Ok(a.value.clone())
}
fn fixed_attr(e: &Element, name: &str, value: &str) -> Result<(), String> {
    if attr(e, name)? != value {
        Err(err(e, &format!("unexpected @{name}")))
    } else {
        Ok(())
    }
}
fn uuid(value: String) -> Result<String, String> {
    if value.len() != 36
        || !value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
    {
        Err("malformed source UUID".into())
    } else {
        Ok(value)
    }
}
fn bool_value(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid source Boolean {value:?}")),
    }
}
fn enum_value(value: String, allowed: &[&str]) -> Result<String, String> {
    if allowed.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(format!("unsupported source enum {value:?}"))
    }
}
fn strings(e: &Element, tag: &str) -> Result<Vec<String>, String> {
    e.children
        .iter()
        .filter(|c| c.local == tag)
        .map(leaf)
        .collect()
}
fn localized(e: &Element, edt: bool, tag: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    if edt {
        for c in e.children.iter().filter(|c| c.local == tag) {
            branch(c)?;
            let k = leaf(required(c, "key")?)?;
            let v = leaf(required(c, "value")?)?;
            check(c)?;
            out.push((k, v));
        }
    } else if let Some(host) = optional(e, tag)? {
        branch(host)?;
        for c in &host.children {
            if c.local != "item" || c.prefix != "v8" {
                return Err(err(c, "expected v8:item"));
            }
            branch(c)?;
            let k = leaf(required(c, "lang")?)?;
            let v = leaf(required(c, "content")?)?;
            check(c)?;
            out.push((k, v));
        }
        check(host)?;
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    if out.windows(2).any(|w| w[0].0 == w[1].0) {
        return Err("duplicate localized language".into());
    }
    Ok(out)
}
fn emit_localized(edt: bool, tag: &str, values: &[(String, String)]) -> Vec<OutElement> {
    if edt {
        values
            .iter()
            .map(|(k, v)| {
                let mut e = OutElement::branch("", tag);
                e.push(out_leaf("", "key", k));
                e.push(out_leaf("", "value", v));
                e
            })
            .collect()
    } else {
        let mut e = OutElement::branch("", tag);
        if values.is_empty() {
            e.self_closing = true;
        }
        for (k, v) in values {
            let mut item = OutElement::branch("v8", "item");
            item.push(out_leaf("v8", "lang", k));
            item.push(out_leaf("v8", "content", v));
            e.push(item);
        }
        vec![e]
    }
}
pub fn read_edt(kind: &str, root: &Element) -> Result<SourceExtensions, String> {
    let mut out = SourceExtensions::default();
    if kind == "AccumulationRegister" {
        if let Some(e) = optional(root, "aggregates")? {
            fixed_attr(e, "xsi:type", "aggregates:AccumulationRegisterAggregates")?;
            fixed_attr(root, "xmlns:aggregates", "http://g5.1c.ru/v8/dt/aggregates")?;
            fixed_attr(
                root,
                "xmlns:xsi",
                "http://www.w3.org/2001/XMLSchema-instance",
            )?;
            out.aggregates = Some(read_aggregates(e, true)?);
        }
    }
    if kind == "CalculationRegister" {
        for e in root.children.iter().filter(|e| e.local == "recalculations") {
            out.recalculations.push(read_recalculation(e, true, None)?);
        }
        out.recalculation_refs = out.recalculations.iter().map(|r| r.name.clone()).collect();
    }
    if kind == "ChartOfCalculationTypes" {
        if let Some(e) = optional(root, "predefined")? {
            if !e.children.is_empty() {
                fixed_attr(root, "xmlns:core", "http://g5.1c.ru/v8/dt/mcore")?;
                fixed_attr(
                    root,
                    "xmlns:xsi",
                    "http://www.w3.org/2001/XMLSchema-instance",
                )?;
            }
            out.calculation_predefined = Some(read_calculation_predefined(e, true)?);
        }
    }
    Ok(out)
}
/// XML item code does not retain the EDT value variant independently of its
/// chart owner. Reject contradictory edited source rather than erase the variant.
pub fn validate_predefined_code_kind(obj: &morph1c_core::ir::MetadataObject) -> Result<(), String> {
    if let Some(items) = &obj.source_extensions.calculation_predefined {
        let numeric = matches!(obj.get(morph1c_core::spec::metadata::chart_of_calculation_types::F_CODE_TYPE), Some(morph1c_core::ir::PropertyValue::Enum(token)) if token.as_str() == "Number");
        if items.iter().any(|item| item.numeric_code != numeric) {
            return Err("calculation predefined code value type disagrees with owner CodeType".into());
        }
    }
    Ok(())
}
pub fn read_designer_refs(kind: &str, root: &Element) -> Result<SourceExtensions, String> {
    let mut out = SourceExtensions::default();
    if kind == "CalculationRegister" {
        let object = required(root, kind)?;
        if let Some(children) = optional(object, "ChildObjects")? {
            for e in children
                .children
                .iter()
                .filter(|e| e.local == "Recalculation")
            {
                out.recalculation_refs.push(leaf(e)?);
            }
        }
    }
    Ok(out)
}
pub fn write_edt(extra: &SourceExtensions, root: &mut OutElement) -> Result<(), String> {
    if let Some(items) = &extra.aggregates {
        root.attrs.push((
            "xmlns:aggregates".into(),
            "http://g5.1c.ru/v8/dt/aggregates".into(),
        ));
        root.push(emit_aggregates(items, true));
    }
    for r in &extra.recalculations {
        let e = emit_recalculation(r, true, None);
        let pos = root
            .children
            .iter()
            .position(|e| e.local == "forms" || e.local == "templates")
            .unwrap_or(root.children.len());
        root.children.insert(pos, e);
    }
    if let Some(items) = &extra.calculation_predefined {
        root.push(emit_calculation_predefined(items, true));
    }
    Ok(())
}
pub fn write_designer_refs(extra: &SourceExtensions, object: &mut OutElement) {
    if extra.recalculations.is_empty() {
        return;
    }
    let index = object
        .children
        .iter()
        .position(|e| e.local == "ChildObjects")
        .unwrap_or_else(|| {
            object.push(OutElement::branch("", "ChildObjects"));
            object.children.len() - 1
        });
    let children = &mut object.children[index];
    children.self_closing = false;
    for r in &extra.recalculations {
        let pos = children
            .children
            .iter()
            .position(|e| e.local == "Form" || e.local == "Template")
            .unwrap_or(children.children.len());
        children
            .children
            .insert(pos, out_leaf("", "Recalculation", &r.name));
    }
}
pub fn read_aggregates(e: &Element, edt: bool) -> Result<Vec<Aggregate>, String> {
    branch(e)?;
    let mut out = Vec::new();
    for child in &e.children {
        if child.local != if edt { "aggregates" } else { "Aggregate" } {
            return Err(err(child, "expected aggregate"));
        }
        branch(child)?;
        let id = uuid(attr(child, "id")?)?;
        let usage = enum_value(
            text(child, if edt { "use" } else { "Use" }, "Auto")?,
            &["Auto", "Always"],
        )?;
        let periodicity = enum_value(
            text(
                child,
                if edt { "periodicity" } else { "Periodicity" },
                "Nonperiodical",
            )?,
            &[
                "Nonperiodical",
                "Auto",
                "Day",
                "Month",
                "Quarter",
                "HalfYear",
                "Year",
            ],
        )?;
        let dimensions = if edt {
            strings(child, "dimensions")?
                .into_iter()
                .map(|r| (r, true))
                .collect()
        } else {
            let mut values = Vec::new();
            if let Some(host) = optional(child, "Dimensions")? {
                branch(host)?;
                for dim in &host.children {
                    if dim.local != "Dimension" {
                        return Err(err(dim, "expected Dimension"));
                    }
                    let reference = attr(dim, "ref")?;
                    if !dim.children.is_empty() {
                        return Err(err(dim, "unexpected children"));
                    }
                    let value = bool_value(&dim.text)?;
                    if !value {
                        return Err(err(
                            dim,
                            "disabled aggregate dimensions lack a reversible EDT projection",
                        ));
                    }
                    dim.claim_with_text();
                    check(dim)?;
                    values.push((reference, value));
                }
                check(host)?;
            }
            values
        };
        check(child)?;
        out.push(Aggregate {
            id,
            usage,
            periodicity,
            dimensions,
        });
    }
    check(e)?;
    Ok(out)
}
pub fn emit_aggregates(values: &[Aggregate], edt: bool) -> OutElement {
    let mut root = OutElement::branch(
        "",
        if edt {
            "aggregates"
        } else {
            "AccumulationRegisterAggregates"
        },
    );
    if edt {
        root = root.attr("xsi:type", "aggregates:AccumulationRegisterAggregates");
    }
    for item in values {
        let mut e = OutElement::branch("", if edt { "aggregates" } else { "Aggregate" })
            .attr("id", &item.id);
        if !edt || item.usage != "Auto" {
            e.push(out_leaf("", if edt { "use" } else { "Use" }, &item.usage));
        }
        if !edt || item.periodicity != "Nonperiodical" {
            e.push(out_leaf(
                "",
                if edt { "periodicity" } else { "Periodicity" },
                &item.periodicity,
            ));
        }
        if edt {
            for (reference, enabled) in &item.dimensions {
                if *enabled {
                    e.push(out_leaf("", "dimensions", reference));
                }
            }
        } else {
            let mut dims = OutElement::branch("", "Dimensions");
            dims.self_closing = item.dimensions.is_empty();
            for (reference, enabled) in &item.dimensions {
                dims.push(out_leaf("", "Dimension", enabled.to_string()).attr("ref", reference));
            }
            e.push(dims);
        }
        root.push(e);
    }
    root
}
pub fn read_calculation_predefined(
    e: &Element,
    edt: bool,
) -> Result<Vec<CalculationPredefined>, String> {
    branch(e)?;
    let mut out = Vec::new();
    for item in &e.children {
        if item.local != if edt { "items" } else { "Item" } {
            return Err(err(item, "expected calculation predefined Item"));
        }
        branch(item)?;
        let id = uuid(attr(item, "id")?)?;
        let name = leaf(required(item, if edt { "name" } else { "Name" })?)?;
        let description = text(item, if edt { "description" } else { "Description" }, "")?;
        let code_el = required(item, if edt { "code" } else { "Code" })?;
        let (numeric_code, code) = if edt {
            branch(code_el)?;
            let ty = attr(code_el, "xsi:type")?;
            let numeric = match ty.as_str() {
                "core:StringValue" => false,
                "core:NumberValue" => true,
                _ => return Err(err(code_el, "unsupported code type")),
            };
            let value = leaf(required(code_el, "value")?)?;
            check(code_el)?;
            (numeric, value)
        } else {
            (false, leaf(code_el)?)
        };
        let action_period_is_base = bool_value(&text(
            item,
            if edt {
                "actionPeriodIsBase"
            } else {
                "ActionPeriodIsBase"
            },
            "false",
        )?)?;
        fn refs(item: &Element, edt: bool, tag: &str) -> Result<Vec<String>, String> {
            if edt {
                strings(item, tag)
            } else {
                let mut v = Vec::new();
                if let Some(host) = optional(item, tag)? {
                    branch(host)?;
                    for c in &host.children {
                        if c.local != "CalculationType" {
                            return Err(err(c, "expected CalculationType"));
                        }
                        v.push(leaf(c)?);
                    }
                    check(host)?;
                }
                Ok(v)
            }
        }
        let displaced = refs(item, edt, if edt { "displaced" } else { "Displaced" })?;
        let base = refs(item, edt, if edt { "base" } else { "Base" })?;
        let leading = refs(item, edt, if edt { "leading" } else { "Leading" })?;
        check(item)?;
        out.push(CalculationPredefined {
            id,
            name,
            code,
            numeric_code,
            description,
            action_period_is_base,
            displaced,
            base,
            leading,
        });
    }
    check(e)?;
    Ok(out)
}
pub fn emit_calculation_predefined(values: &[CalculationPredefined], edt: bool) -> OutElement {
    let mut root = OutElement::branch("", if edt { "predefined" } else { "PredefinedData" });
    for item in values {
        let mut e = OutElement::branch("", if edt { "items" } else { "Item" }).attr("id", &item.id);
        e.push(out_leaf("", if edt { "name" } else { "Name" }, &item.name));
        if edt {
            if !item.description.is_empty() {
                e.push(out_leaf("", "description", &item.description));
            }
            let mut code = OutElement::branch("", "code").attr(
                "xsi:type",
                if item.numeric_code {
                    "core:NumberValue"
                } else {
                    "core:StringValue"
                },
            );
            code.push(out_leaf("", "value", &item.code));
            e.push(code);
        } else {
            e.push(out_leaf("", "Code", &item.code));
            e.push(out_leaf("", "Description", &item.description));
        }
        if !edt || item.action_period_is_base {
            e.push(out_leaf(
                "",
                if edt {
                    "actionPeriodIsBase"
                } else {
                    "ActionPeriodIsBase"
                },
                item.action_period_is_base.to_string(),
            ));
        }
        for (tag, values) in [
            (if edt { "displaced" } else { "Displaced" }, &item.displaced),
            (if edt { "base" } else { "Base" }, &item.base),
            (if edt { "leading" } else { "Leading" }, &item.leading),
        ] {
            if edt {
                for value in values {
                    e.push(out_leaf("", tag, value));
                }
            } else if !values.is_empty() {
                let mut host = OutElement::branch("", tag);
                for value in values {
                    host.push(out_leaf("", "CalculationType", value));
                }
                e.push(host);
            }
        }
        root.push(e);
    }
    root
}
pub fn read_recalculation(
    e: &Element,
    edt: bool,
    owner: Option<&str>,
) -> Result<Recalculation, String> {
    branch(e)?;
    let id = uuid(attr(e, "uuid")?)?;
    let props = if edt {
        e
    } else {
        let p = required(e, "Properties")?;
        branch(p)?;
        p
    };
    let name = leaf(required(props, if edt { "name" } else { "Name" })?)?;
    let synonym = localized(props, edt, if edt { "synonym" } else { "Synonym" })?;
    let comment = text(props, if edt { "comment" } else { "Comment" }, "")?;
    let data_lock_mode = enum_value(
        text(
            props,
            if edt {
                "dataLockControlMode"
            } else {
                "DataLockControlMode"
            },
            "Automatic",
        )?,
        &["Automatic", "Managed", "AutomaticAndManaged"],
    )?;
    let mut generated_types = Vec::new();
    let info = required(e, if edt { "producedTypes" } else { "InternalInfo" })?;
    branch(info)?;
    for t in &info.children {
        branch(t)?;
        let category = if edt {
            match t.local.as_str() {
                "recordType" => "Record",
                "managerType" => "Manager",
                "recordSetType" => "RecordSet",
                _ => return Err(err(t, "unknown produced type")),
            }
            .to_string()
        } else {
            if t.local != "GeneratedType" || t.prefix != "xr" {
                return Err(err(t, "expected xr:GeneratedType"));
            }
            let category = enum_value(attr(t, "category")?, &["Record", "Manager", "RecordSet"])?;
            let expect = format!(
                "Recalculation{category}.{}.{}",
                owner.ok_or("missing recalculation owner")?,
                name
            );
            if attr(t, "name")? != expect {
                return Err(err(t, "generated type name disagrees with identity"));
            }
            category
        };
        let (type_id, value_id) = if edt {
            (uuid(attr(t, "typeId")?)?, uuid(attr(t, "valueTypeId")?)?)
        } else {
            (
                uuid(leaf(required(t, "TypeId")?)?)?,
                uuid(leaf(required(t, "ValueId")?)?)?,
            )
        };
        check(t)?;
        generated_types.push(RecalculationType {
            category,
            type_id,
            value_id,
        });
    }
    check(info)?;
    if generated_types.len() != 3
        || !["Record", "Manager", "RecordSet"]
            .iter()
            .all(|c| generated_types.iter().filter(|t| t.category == *c).count() == 1)
    {
        return Err("recalculation requires exactly three generated categories".into());
    }
    let dims = if edt {
        e
    } else {
        let c = required(e, "ChildObjects")?;
        branch(c)?;
        c
    };
    let mut dimensions = Vec::new();
    for d in dims
        .children
        .iter()
        .filter(|d| d.local == if edt { "dimensions" } else { "Dimension" })
    {
        branch(d)?;
        let id = uuid(attr(d, "uuid")?)?;
        let p = if edt {
            d
        } else {
            let p = required(d, "Properties")?;
            branch(p)?;
            p
        };
        let name = leaf(required(p, if edt { "name" } else { "Name" })?)?;
        let synonym = localized(p, edt, if edt { "synonym" } else { "Synonym" })?;
        let comment = text(p, if edt { "comment" } else { "Comment" }, "")?;
        let register_dimension = text(
            p,
            if edt {
                "registerDimension"
            } else {
                "RegisterDimension"
            },
            "",
        )?;
        let leading_data = if edt {
            strings(p, "leadingRegisterData")?
        } else {
            let mut v = Vec::new();
            if let Some(host) = optional(p, "LeadingRegisterData")? {
                branch(host)?;
                for item in &host.children {
                    if item.local != "Item" || item.prefix != "xr" {
                        return Err(err(item, "expected xr:Item"));
                    }
                    fixed_attr(item, "xsi:type", "xr:MDObjectRef")?;
                    if !item.children.is_empty() {
                        return Err(err(item, "expected leaf"));
                    }
                    item.claim_with_text();
                    check(item)?;
                    v.push(item.text.clone());
                }
                check(host)?;
            }
            v
        };
        check(p)?;
        check(d)?;
        dimensions.push(RecalculationDimension {
            id,
            name,
            synonym,
            comment,
            register_dimension,
            leading_data,
        });
    }
    if !edt {
        check(props)?;
        check(dims)?;
    }
    check(e)?;
    Ok(Recalculation {
        id,
        name,
        synonym,
        comment,
        data_lock_mode,
        generated_types,
        dimensions,
    })
}
pub fn emit_recalculation(value: &Recalculation, edt: bool, owner: Option<&str>) -> OutElement {
    let mut root = OutElement::branch(
        "",
        if edt {
            "recalculations"
        } else {
            "Recalculation"
        },
    )
    .attr("uuid", &value.id);
    let mut info = OutElement::branch("", if edt { "producedTypes" } else { "InternalInfo" });
    for t in &value.generated_types {
        let e = if edt {
            OutElement::self_closing(
                "",
                match t.category.as_str() {
                    "Record" => "recordType",
                    "Manager" => "managerType",
                    _ => "recordSetType",
                },
            )
            .attr("typeId", &t.type_id)
            .attr("valueTypeId", &t.value_id)
        } else {
            let mut e = OutElement::branch("xr", "GeneratedType")
                .attr(
                    "name",
                    format!(
                        "Recalculation{}.{}.{}",
                        t.category,
                        owner.unwrap_or(""),
                        value.name
                    ),
                )
                .attr("category", &t.category);
            e.push(out_leaf("xr", "TypeId", &t.type_id));
            e.push(out_leaf("xr", "ValueId", &t.value_id));
            e
        };
        info.push(e);
    }
    root.push(info);
    let mut props = OutElement::branch("", "Properties");
    props.push(out_leaf("", if edt { "name" } else { "Name" }, &value.name));
    for e in emit_localized(edt, if edt { "synonym" } else { "Synonym" }, &value.synonym) {
        props.push(e);
    }
    if !edt || !value.comment.is_empty() {
        props.push(out_leaf(
            "",
            if edt { "comment" } else { "Comment" },
            &value.comment,
        ));
    }
    if !edt || value.data_lock_mode != "Automatic" {
        props.push(out_leaf(
            "",
            if edt {
                "dataLockControlMode"
            } else {
                "DataLockControlMode"
            },
            &value.data_lock_mode,
        ));
    }
    if edt {
        root.children.extend(props.children);
    } else {
        root.push(props);
    }
    let mut children = OutElement::branch("", "ChildObjects");
    for d in &value.dimensions {
        let mut e = OutElement::branch("", if edt { "dimensions" } else { "Dimension" })
            .attr("uuid", &d.id);
        let mut p = OutElement::branch("", "Properties");
        p.push(out_leaf("", if edt { "name" } else { "Name" }, &d.name));
        for c in emit_localized(edt, if edt { "synonym" } else { "Synonym" }, &d.synonym) {
            p.push(c);
        }
        if !edt || !d.comment.is_empty() {
            p.push(out_leaf(
                "",
                if edt { "comment" } else { "Comment" },
                &d.comment,
            ));
        }
        if !edt || !d.register_dimension.is_empty() {
            p.push(out_leaf(
                "",
                if edt {
                    "registerDimension"
                } else {
                    "RegisterDimension"
                },
                &d.register_dimension,
            ));
        }
        if edt {
            for r in &d.leading_data {
                p.push(out_leaf("", "leadingRegisterData", r));
            }
            e.children.extend(p.children);
        } else {
            let mut l = OutElement::branch("", "LeadingRegisterData");
            l.self_closing = d.leading_data.is_empty();
            for r in &d.leading_data {
                l.push(out_leaf("xr", "Item", r).attr("xsi:type", "xr:MDObjectRef"));
            }
            p.push(l);
            e.push(p);
        }
        children.push(e);
    }
    if edt {
        root.children.extend(children.children);
    } else {
        root.push(children);
    }
    root
}
// Sidecar callers must validate their document envelope before these grammars.
pub fn claim_sidecar_envelope(root: &Element, kind: &str, version: &str) -> Result<(), String> {
    if root.local != kind || !root.prefix.is_empty() {
        return Err(err(root, "unexpected sidecar root"));
    }
    let namespace = match kind {
        "AccumulationRegisterAggregates" => "http://v8.1c.ru/8.3/xcf/extrnprops",
        "PredefinedData" => "http://v8.1c.ru/8.3/xcf/predef",
        _ => return Err("unknown sidecar kind".into()),
    };
    for (name, value) in [
        ("xmlns", namespace),
        ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
        ("xmlns:xr", "http://v8.1c.ru/8.3/xcf/readable"),
        ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
        ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
        ("version", version),
    ] {
        fixed_attr(root, name, value)?;
    }
    if kind == "PredefinedData" {
        fixed_attr(root, "xsi:type", "CalculationTypePredefinedItems")?;
    }
    Ok(())
}
pub fn sidecar_envelope(mut root: OutElement, version: &str) -> OutElement {
    let namespace = if root.local == "PredefinedData" {
        "http://v8.1c.ru/8.3/xcf/predef"
    } else {
        "http://v8.1c.ru/8.3/xcf/extrnprops"
    };
    for (name, value) in [
        ("xmlns", namespace),
        ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
        ("xmlns:xr", "http://v8.1c.ru/8.3/xcf/readable"),
        ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
        ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    ] {
        root.attrs.push((name.into(), value.into()));
    }
    if root.local == "PredefinedData" {
        root.attrs
            .push(("xsi:type".into(), "CalculationTypePredefinedItems".into()));
    }
    root.attrs.push(("version".into(), version.into()));
    root
}
