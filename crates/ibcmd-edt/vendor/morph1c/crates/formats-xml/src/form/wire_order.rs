//! Lexical ordering of closed, typed singleton property slots. This facet never
//! stores values, namespace aliases, arbitrary XML, or ordering inside semantic arrays.
use super::{
    FormDialect, FormError,
    fields::{Codec, FieldProj, Region},
    tables,
};
use crate::{descriptor::Element, emit::OutElement};
use morph1c_core::ir::FormBody;
use morph1c_core::ir::form::FormWireOrder;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone)]
struct Scope {
    key: String,
    kind: String,
    ext: bool,
}

fn kind(d: FormDialect, local: &str, xsi: Option<&str>, ty: Option<&str>) -> Option<String> {
    if d == FormDialect::Designer {
        if let Some(f) = tables::field_kind_by_des_tag(local) {
            return Some(f.kind.into());
        }
        if tables::group_kind(local).is_some()
            || tables::decoration_kind(local).is_some()
            || tables::addition_kind(local).is_some()
            || matches!(
                local,
                "Button" | "Table" | "ExtendedTooltip" | "ContextMenu" | "AutoCommandBar"
            )
        {
            return Some(local.into());
        }
    } else {
        if local == "extendedTooltip" {
            return Some("ExtendedTooltip".into());
        }
        if local == "contextMenu" {
            return Some("ContextMenu".into());
        }
        if local == "autoCommandBar" {
            return Some("AutoCommandBar".into());
        }
        if local == "autoTable" {
            return Some("Table".into());
        }
        if let Some(a) = tables::ADDITION_KINDS.iter().find(|a| a.edt_tag == local) {
            return Some(a.kind.into());
        }
        if !matches!(local, "items" | "columns") {
            return None;
        }
        match xsi {
            Some("form:FormField") if tables::field_kind(ty.unwrap_or("")).is_some() => {
                return ty.map(str::to_owned);
            }
            Some("form:FormGroup") => {
                let t = ty.unwrap_or("ButtonGroup");
                if tables::group_kind(t).is_some() {
                    return Some(t.into());
                }
            }
            Some("form:Decoration") => {
                return Some(
                    if ty == Some("Label") {
                        "LabelDecoration"
                    } else {
                        "PictureDecoration"
                    }
                    .into(),
                );
            }
            Some("form:Addition") => {
                let k = ty.unwrap_or("SearchStringAddition");
                if tables::addition_kind(k).is_some() {
                    return Some(k.into());
                }
            }
            Some("form:Button") => return Some("Button".into()),
            Some("form:Table") => return Some("Table".into()),
            _ => {}
        }
    }
    None
}

fn root_scope() -> Scope {
    Scope {
        key: "root".into(),
        kind: "root".into(),
        ext: false,
    }
}
fn child_scope(parent: &Scope, k: String, id: &str) -> Result<Scope, FormError> {
    let id = id
        .parse::<i64>()
        .map_err(|_| FormError::Frame("invalid typed control id in property ordering".into()))?;
    Ok(Scope {
        key: format!("{}/{}:{}", parent.key, k, id),
        kind: k,
        ext: false,
    })
}
fn ext_scope(parent: &Scope) -> Scope {
    Scope {
        key: format!("{}:ext", parent.key),
        kind: parent.kind.clone(),
        ext: true,
    }
}

fn slots(s: &Scope, d: FormDialect) -> BTreeSet<&'static str> {
    let mut out = BTreeSet::new();
    let mut add = |t: &'static [FieldProj]| {
        for f in t {
            match d {
                FormDialect::Designer if !f.des_attr => {
                    out.insert(f.des);
                }
                FormDialect::Edt if (f.region == Region::Ext) == s.ext => {
                    out.insert(f.edt);
                }
                _ => {}
            }
        }
    };
    if s.kind == "root" {
        out.extend(super::projection::lexical_property_tags(d));
        out.extend(match d {
            FormDialect::Designer => vec![
                "Title",
                "CreateButtonsGroupTitle",
                "CommandSet",
                "ShowCommandBar",
                "AutoCommandBar",
                "Events",
                "ChildItems",
                "Attributes",
                "Commands",
                "Parameters",
                "CommandInterface",
                "UseForFoldersAndItems",
                "AutoTime",
                "UsePostingMode",
                "RepostOnWrite",
                "CustomSettingsFolder",
                "ReportFormType",
                "AutoShowState",
                "ReportResultViewMode",
                "ViewModeApplicationOnSetReportResult",
            ],
            FormDialect::Edt => vec![
                "title",
                "createButtonsGroupTitle",
                "autoCommandBar",
                "commandInterface",
                "extInfo",
                "excludedCommands",
            ],
        });
        return out;
    }
    if let Some(f) = tables::field_kind(&s.kind) {
        add(tables::FORM_FIELD_COMMON);
        add(f.ext);
    } else if let Some(g) = tables::group_kind(&s.kind) {
        add(tables::FORM_GROUP_BODY);
        add(g.ext);
    } else if let Some(g) = tables::decoration_kind(&s.kind) {
        add(tables::DECORATION_BODY);
        add(g.ext);
    } else {
        match s.kind.as_str() {
            "Button" => add(tables::BUTTON_BODY),
            "Table" => {
                add(tables::TABLE_BODY);
                add(tables::DYNAMIC_LIST_EXT);
            }
            "ExtendedTooltip" => {
                add(tables::TOOLTIP_BODY);
                add(tables::LABEL_DECORATION_EXT);
            }
            _ => {}
        }
    }
    if tables::addition_kind(&s.kind).is_some() && d == FormDialect::Designer {
        out.extend([
            "Visible",
            "Enabled",
            "AdditionSource",
            "Title",
            "ToolTip",
            "ToolTipRepresentation",
            "GroupHorizontalAlign",
            "Width",
            "AutoMaxWidth",
            "MaxWidth",
            "HorizontalStretch",
            "HorizontalLocation",
            "ContextMenu",
            "ExtendedTooltip",
            "ChildItems",
        ]);
    }
    if tables::addition_kind(&s.kind).is_some() && d == FormDialect::Edt {
        out.extend([
            "title",
            "visible",
            "enabled",
            "userVisible",
            "source",
            "toolTip",
            "toolTipRepresentation",
            "groupHorizontalAlign",
            "width",
            "autoMaxWidth",
            "maxWidth",
            "horizontalStretch",
            "horizontalLocation",
            "contextMenu",
            "extendedTooltip",
            "extInfo",
        ]);
    }
    // Closed structural wrappers may move as one slot; their internal sequence is
    // never visited as a property scope and is never reordered.
    out.extend(match d {
        FormDialect::Designer => vec![
            "Title",
            "TitleHeight",
            "Font",
            "TitleFont",
            "FooterFont",
            "CommandSet",
            "ExtendedTooltip",
            "ContextMenu",
            "AutoCommandBar",
            "Events",
            "ChildItems",
            "Table",
            "ToolTip",
            "ToolTipRepresentation",
        ],
        FormDialect::Edt => vec![
            "title",
            "font",
            "titleFont",
            "footerFont",
            "extendedTooltip",
            "contextMenu",
            "autoCommandBar",
            "extInfo",
            "excludedCommands",
        ],
    });
    out
}

fn repeatable(s: &Scope, tag: &str, d: FormDialect) -> bool {
    if d != FormDialect::Edt {
        return false;
    }
    if matches!(tag, "title" | "excludedCommands")
        || (tables::addition_kind(&s.kind).is_some() && tag == "toolTip")
        || (s.kind == "root" && tag == "createButtonsGroupTitle")
    {
        return true;
    }
    let mut tables_to_check: Vec<&'static [FieldProj]> = Vec::new();
    if let Some(f) = tables::field_kind(&s.kind) {
        tables_to_check.extend([tables::FORM_FIELD_COMMON, f.ext]);
    } else if let Some(g) = tables::group_kind(&s.kind) {
        tables_to_check.extend([tables::FORM_GROUP_BODY, g.ext]);
    } else if let Some(g) = tables::decoration_kind(&s.kind) {
        tables_to_check.extend([tables::DECORATION_BODY, g.ext]);
    } else {
        match s.kind.as_str() {
            "Button" => tables_to_check.push(tables::BUTTON_BODY),
            "Table" => tables_to_check.push(tables::TABLE_BODY),
            "ExtendedTooltip" => tables_to_check.push(tables::TOOLTIP_BODY),
            _ => {}
        }
    }
    tables_to_check
        .into_iter()
        .flatten()
        .any(|f| f.edt == tag && matches!(f.codec, Codec::Localized))
}
fn property_order<'a>(
    names: impl Iterator<Item = (&'a str, &'a str)>,
    allowed: &BTreeSet<&str>,
) -> Vec<String> {
    names
        .filter(|(p, l)| p.is_empty() && allowed.contains(l))
        .map(|(_, l)| l.into())
        .collect()
}
fn insert(
    out: &mut BTreeMap<String, Vec<String>>,
    scope: &Scope,
    order: Vec<String>,
) -> Result<(), FormError> {
    if out.insert(scope.key.clone(), order).is_some() {
        return Err(FormError::Frame(format!(
            "duplicate typed control identity in property ordering: {}",
            scope.key
        )));
    }
    Ok(())
}
fn read_orders(
    d: FormDialect,
    el: &Element,
    parent: &Scope,
    root: bool,
    out: &mut BTreeMap<String, Vec<String>>,
) -> Result<(), FormError> {
    let text = |name: &str| {
        el.children
            .iter()
            .find(|c| c.prefix.is_empty() && c.local == name)
            .map(|c| c.text.as_str())
    };
    let control = kind(
        d,
        &el.local,
        el.attr("xsi:type").map(|a| a.value.as_str()),
        text("type"),
    );
    let scope = if root {
        Some(root_scope())
    } else if let Some(k) = control {
        Some(child_scope(
            parent,
            k,
            if d == FormDialect::Designer {
                el.attr("id").map(|a| a.value.as_str()).unwrap_or("0")
            } else {
                text("id").unwrap_or("0")
            },
        )?)
    } else if d == FormDialect::Edt && el.local == "extInfo" && el.prefix.is_empty() && !parent.ext
    {
        Some(ext_scope(parent))
    } else {
        None
    };
    let next = scope.as_ref().unwrap_or(parent);
    if let Some(s) = &scope {
        insert(
            out,
            s,
            property_order(
                el.children
                    .iter()
                    .map(|c| (c.prefix.as_str(), c.local.as_str())),
                &slots(s, d),
            ),
        )?;
    }
    // Only control/container descendants are matched. A similarly named element
    // inside DCS values or events cannot become a lexical property scope.
    for c in &el.children {
        if c.prefix.is_empty()
            && matches!(
                c.local.as_str(),
                "items"
                    | "columns"
                    | "ChildItems"
                    | "extInfo"
                    | "ExtendedTooltip"
                    | "extendedTooltip"
                    | "ContextMenu"
                    | "contextMenu"
                    | "AutoCommandBar"
                    | "autoCommandBar"
                    | "Table"
                    | "autoTable"
                    | "searchStringAddition"
                    | "viewStatusAddition"
                    | "searchControlAddition"
            )
            || (d == FormDialect::Designer
                && c.prefix.is_empty()
                && kind(d, &c.local, None, None).is_some())
        {
            read_orders(d, c, next, false, out)?;
        }
    }
    Ok(())
}
fn out_scope(
    d: FormDialect,
    el: &OutElement,
    parent: &Scope,
    root: bool,
) -> Result<Option<Scope>, FormError> {
    let text = |name: &str| {
        el.children
            .iter()
            .find(|c| c.prefix.is_empty() && c.local == name)
            .and_then(|c| c.text.as_deref())
    };
    Ok(if root {
        Some(root_scope())
    } else if let Some(k) = kind(
        d,
        &el.local,
        el.attrs
            .iter()
            .find(|(k, _)| k == "xsi:type")
            .map(|(_, v)| v.as_str()),
        text("type"),
    ) {
        Some(child_scope(
            parent,
            k,
            if d == FormDialect::Designer {
                el.attrs
                    .iter()
                    .find(|(k, _)| k == "id")
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("0")
            } else {
                text("id").unwrap_or("0")
            },
        )?)
    } else if d == FormDialect::Edt && el.local == "extInfo" && el.prefix.is_empty() && !parent.ext
    {
        Some(ext_scope(parent))
    } else {
        None
    })
}
fn walk_out(
    d: FormDialect,
    el: &mut OutElement,
    parent: &Scope,
    root: bool,
    facet: Option<&FormWireOrder>,
    orders: &mut BTreeMap<String, Vec<String>>,
) -> Result<(), FormError> {
    let scope = out_scope(d, el, parent, root)?;
    let next = scope.as_ref().unwrap_or(parent);
    if let Some(s) = &scope {
        let allowed = slots(s, d);
        let order = property_order(
            el.children
                .iter()
                .map(|c| (c.prefix.as_str(), c.local.as_str())),
            &allowed,
        );
        insert(orders, s, order.clone())?;
        if let Some(source) = facet.and_then(|f| f.scopes.get(&s.key)) {
            let mut seen = BTreeSet::new();
            for tag in source {
                if !allowed.contains(tag.as_str()) || (!seen.insert(tag) && !repeatable(s, tag, d))
                {
                    return Err(FormError::Frame(
                        "unknown or duplicate typed property-order slot".into(),
                    ));
                }
            }
            let eligible: BTreeSet<_> = order.iter().map(String::as_str).collect();
            let indexes: Vec<_> = el
                .children
                .iter()
                .enumerate()
                .filter(|(_, c)| c.prefix.is_empty() && eligible.contains(c.local.as_str()))
                .map(|(i, _)| i)
                .collect();
            let mut queues = BTreeMap::<String, VecDeque<OutElement>>::new();
            for i in &indexes {
                let node = std::mem::replace(&mut el.children[*i], OutElement::branch("", ""));
                queues
                    .entry(node.local.clone())
                    .or_default()
                    .push_back(node);
            }
            let mut moved = Vec::with_capacity(indexes.len());
            // Repeated known regions use stable queues. Their independently typed
            // array order is immutable even when property slots were interleaved.
            // Added/deleted semantic entries never restore source values.
            for tag in source.iter().chain(order.iter()) {
                if let Some(node) = queues.get_mut(tag).and_then(VecDeque::pop_front) {
                    moved.push(node);
                }
            }
            if moved.len() != indexes.len() {
                return Err(FormError::Frame(
                    "inconsistent typed property-order queues".into(),
                ));
            }
            for (i, c) in indexes.into_iter().zip(moved) {
                el.children[i] = c;
            }
        }
    }
    for c in &mut el.children {
        if c.prefix.is_empty()
            && matches!(
                c.local.as_str(),
                "items"
                    | "columns"
                    | "ChildItems"
                    | "extInfo"
                    | "ExtendedTooltip"
                    | "extendedTooltip"
                    | "ContextMenu"
                    | "contextMenu"
                    | "AutoCommandBar"
                    | "autoCommandBar"
                    | "Table"
                    | "autoTable"
                    | "searchStringAddition"
                    | "viewStatusAddition"
                    | "searchControlAddition"
            )
            || (d == FormDialect::Designer
                && c.prefix.is_empty()
                && kind(d, &c.local, None, None).is_some())
        {
            walk_out(d, c, next, false, facet, orders)?;
        }
    }
    Ok(())
}

pub(super) fn capture(
    d: FormDialect,
    root: &Element,
    body: &FormBody,
) -> Result<FormWireOrder, FormError> {
    let mut scopes = BTreeMap::new();
    read_orders(d, root, &root_scope(), true, &mut scopes)?;
    let mut canonical = match d {
        FormDialect::Designer => {
            let v = root
                .attr("version")
                .and_then(|a| super::detect_form_profile(&a.value).map(|p| p.format))
                .ok_or_else(|| {
                    FormError::Frame("invalid source version in property ordering".into())
                })?;
            morph1c_core::version::with_roundtrip_target(v, || super::write::designer_root(body))?
        }
        FormDialect::Edt => match morph1c_core::version::current_source_version() {
            Some(v) => {
                morph1c_core::version::with_roundtrip_target(v, || super::write::edt_root(body))?
            }
            None => super::write::edt_root(body)?,
        },
    };
    let mut baseline = BTreeMap::new();
    walk_out(d, &mut canonical, &root_scope(), true, None, &mut baseline)?;
    scopes.retain(|k, v| baseline.get(k) != Some(v));
    Ok(FormWireOrder {
        designer: d == FormDialect::Designer,
        version: root
            .attr("version")
            .map(|a| a.value.clone())
            .or_else(|| morph1c_core::version::current_source_version().map(|v| v.to_string())),
        scopes,
    })
}
pub(super) fn apply(
    d: FormDialect,
    body: &FormBody,
    root: &mut OutElement,
) -> Result<(), FormError> {
    let Some(f) = &body.source_wire_order else {
        return Ok(());
    };
    if f.designer != (d == FormDialect::Designer) {
        return Ok(());
    }
    if f.designer
        && f.version.as_deref()
            != root
                .attrs
                .iter()
                .find(|(k, _)| k == "version")
                .map(|(_, v)| v.as_str())
    {
        return Ok(());
    }
    if !f.designer
        && f.version.is_some()
        && f.version != morph1c_core::version::current_roundtrip_target().map(|v| v.to_string())
    {
        return Ok(());
    }
    walk_out(d, root, &root_scope(), true, Some(f), &mut BTreeMap::new())
}

/// The native serializer generates this inline namespace from XML nesting depth.
/// A nested typed Column has depth seven, unlike a root Attribute's depth five.
/// This fixes the projection itself; neither source bytes nor QName equality are
/// normalized, and the canonical ConditionalAppearance type identity is unchanged.
pub(super) fn bind_column_type_depth(root: &mut OutElement) {
    fn walk(el: &mut OutElement, path: &mut Vec<String>) {
        if el.prefix == "v8"
            && el.local == "Type"
            && path.last().is_some_and(|t| t == "Type")
            && path.len() >= 3
            && path[path.len() - 2] == "Column"
            && path[path.len() - 3] == "Columns"
            && path
                .windows(2)
                .any(|p| p[0] == "Attributes" && p[1] == "Attribute")
        {
            if let Some((prefix, "ConditionalAppearance")) =
                el.text.as_deref().and_then(|t| t.split_once(':'))
            {
                let declaration = format!("xmlns:{prefix}");
                if let Some((name, _)) = el.attrs.iter_mut().find(|(name, uri)| {
                    name == &declaration && uri == "http://v8.1c.ru/8.3/data/entext"
                }) {
                    let prefix = format!("d{}p1", path.len() + 1);
                    *name = format!("xmlns:{prefix}");
                    el.text = Some(format!("{prefix}:ConditionalAppearance"));
                }
            }
        }
        path.push(el.local.clone());
        for child in &mut el.children {
            walk(child, path);
        }
        path.pop();
    }
    walk(root, &mut Vec::new());
}
