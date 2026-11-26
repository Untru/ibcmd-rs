//! WRITE · spreadsheet (MXL) body — Designer inline `<Settings>` writer plus the shared
//! spreadsheet-children tree reused by the EDT `.mxlx` sidecar (`super::super::mxlx`).

use super::*;

/// Designer `<Settings xmlns:mxl=… xsi:type="mxl:SpreadsheetDocument">` — пустой табличный документ
/// (columns/size, rowsItem/index/row/empty, vgRows). Инлайн-`xmlns:mxl` — атрибут host'а (Designer
/// объявляет его прямо на `<Settings>`, не в корневом ns-блоке).
pub(crate) fn designer_spreadsheet_settings(ss: &MxlSpreadsheetSettings) -> OutElement {
    let mut s = OutElement::branch("", "Settings")
        .attr("xmlns:mxl", MXL_NS_URI)
        .attr("xsi:type", "mxl:SpreadsheetDocument");
    for c in spreadsheet_body_children("mxl", ss) {
        s.push(c);
    }
    s
}

/// Дети-структура табличного документа: БОГАТЫЙ вариант (`full_body`) регенерируется из generic-tree
/// (диалектное написание ХРАНИТСЯ в узлах — `prefix`/порядок атрибутов/инлайн-редекл — ⇒ `prefix`-
/// аргумент для него игнорируется, байт-точно в ТОМ ЖЕ диалекте); МИНИМАЛЬНЫЙ — прежним структурным
/// [`spreadsheet_settings_children`] (SSL/минимум byte-exact не тронут).
pub(crate) fn spreadsheet_body_children(
    prefix: &'static str,
    ss: &MxlSpreadsheetSettings,
) -> Vec<OutElement> {
    if let Some(fb) = &ss.full_body {
        return fb.iter().map(|n| {
            if n.source_layout.as_ref().is_some_and(|l| l.default_spreadsheet_namespace == prefix.is_empty()) {
                mxl_node_to_out(n, ss.envelope_without_pal)
            } else {
                mxl_node_to_out_target(n, prefix.is_empty(), &mxl_output_scope(prefix.is_empty(), ss.envelope_without_pal))
            }
        }).collect();
    }
    spreadsheet_settings_children(prefix, ss)
}

/// Рекурсивно развернуть [`MxlNode`] generic-tree в [`OutElement`] для byte-exact ре-эмиссии:
/// префикс/local/атрибуты(в порядке)/дети/текст. Пустой самозакрытый узел ⇒ `<tag/>`.
pub(crate) fn mxl_node_to_out(n: &MxlNode, without_pal: bool) -> OutElement {
    let default_namespace = n.source_layout.as_ref().is_some_and(|l| l.default_spreadsheet_namespace);
    mxl_node_to_out_target(n, default_namespace, &mxl_output_scope(default_namespace, without_pal))
}
fn mxl_output_scope(default: bool, without_pal: bool) -> std::collections::BTreeMap<String, String> {
    if !default {
        let target = morph1c_core::version::current_roundtrip_target().unwrap_or(morph1c_core::version::SSL);
        let mut scope = std::collections::BTreeMap::from([
            ("xml".into(), "http://www.w3.org/XML/1998/namespace".into()),
            ("mxl".into(), MXL_NS_URI.into()),
        ]);
        if let Some(profile) = form_profile_for(target) {
            for (name,uri) in profile.ns_block {
                if *name == "xmlns" { scope.insert(String::new(), (*uri).into()); }
                else if let Some(prefix) = name.strip_prefix("xmlns:") { scope.insert(prefix.into(), (*uri).into()); }
            }
        }
        return scope;
    }
    [
        ("", if default { MXL_NS_URI } else { "http://v8.1c.ru/8.3/xcf/logform" }),
        ("mxl", MXL_NS_URI), ("v8", "http://v8.1c.ru/8.1/data/core"),
        ("v8ui", "http://v8.1c.ru/8.1/data/ui"),
        ("xs", "http://www.w3.org/2001/XMLSchema"),
        ("xsi", "http://www.w3.org/2001/XMLSchema-instance"),
        ("style", "http://v8.1c.ru/8.1/data/ui/style"),
        ("pal", "http://v8.1c.ru/8.1/data/ui/colors/palette"),
        ("xml", "http://www.w3.org/XML/1998/namespace"),
    ].into_iter().filter(|(prefix,_)| !default || (*prefix != "mxl" && (!without_pal || *prefix != "pal")))
        .map(|(p,u)| (p.into(), u.into())).collect()
}
fn expanded_parts(name: &str) -> (&str, &str) {
    if let Some(rest) = name.strip_prefix('{') {
        if let Some((uri, local)) = rest.split_once('}') { return (uri, local); }
    }
    ("", name)
}
fn mxl_output_prefix(uri: &str, default: bool, attribute: bool) -> String {
    match uri {
        "" => String::new(),
        MXL_NS_URI if default && !attribute => String::new(),
        MXL_NS_URI => "mxl".into(),
        "http://v8.1c.ru/8.1/data/core" => "v8".into(),
        "http://v8.1c.ru/8.1/data/ui" => "v8ui".into(),
        "http://www.w3.org/2001/XMLSchema" => "xs".into(),
        "http://www.w3.org/2001/XMLSchema-instance" => "xsi".into(),
        "http://v8.1c.ru/8.1/data/ui/style" => "style".into(),
        "http://v8.1c.ru/8.1/data/ui/colors/palette" => "pal".into(),
        "http://www.w3.org/XML/1998/namespace" => "xml".into(),
        _ => {
            let mut prefix = String::from("mxlNs");
            for byte in uri.as_bytes() { use std::fmt::Write; write!(&mut prefix, "{byte:02x}").expect("String write"); }
            prefix
        },
    }
}
fn bind_output(el: &mut OutElement, scope: &mut std::collections::BTreeMap<String, String>, prefix: &str, uri: &str) {
    if scope.get(prefix).is_some_and(|u| u == uri) { return; }
    let name = if prefix.is_empty() { "xmlns".into() } else { format!("xmlns:{prefix}") };
    if let Some((_,value)) = el.attrs.iter_mut().find(|(n,_)| n == &name) { *value = uri.into(); }
    else { el.attrs.push((name,uri.into())); }
    scope.insert(prefix.into(),uri.into());
}
fn unused_output_prefix(preferred: String, uri: &str, used: &std::collections::BTreeMap<String, String>) -> String {
    if used.get(&preferred).is_none_or(|bound| bound == uri) { return preferred; }
    let mut prefix = String::from("mxlNs");
    for byte in uri.as_bytes() { use std::fmt::Write; write!(&mut prefix, "{byte:02x}").expect("String write"); }
    while used.get(&prefix).is_some_and(|bound| bound != uri) { prefix.push('_'); }
    prefix
}
fn mxl_node_to_out_target(n: &MxlNode, default: bool, inherited: &std::collections::BTreeMap<String, String>) -> OutElement {
    let lexical = n.source_layout.as_ref().filter(|l| l.default_spreadsheet_namespace == default);
    let mut prefix = if n.namespace.is_empty() { String::new() } else { lexical.map_or_else(|| mxl_output_prefix(&n.namespace, default, false), |_| n.prefix.clone()) };
    // An unqualified QName value requires an empty default namespace. Give its element a
    // prefix when necessary, rather than changing the CURRENT element namespace.
    if prefix.is_empty() && !n.namespace.is_empty() && (n.text_qname.as_ref().is_some_and(|(uri,_)| uri.is_empty())
        || n.attrs.iter().any(|(name,value)| name == "{http://www.w3.org/2001/XMLSchema-instance}type" && expanded_parts(value).0.is_empty())) {
        prefix = mxl_output_prefix(&n.namespace, default, true);
    }
    let mut el = OutElement::branch(prefix.clone(), n.local.clone());
    let mut scope = inherited.clone();
    let mut used = std::collections::BTreeMap::from([(prefix.clone(), n.namespace.clone())]);
    let mut names = lexical.map(|l| l.attributes.iter().map(|(n,_)| n.clone()).collect::<Vec<_>>()).unwrap_or_default();
    for (name,_) in &n.attrs { if !names.contains(name) { names.push(name.clone()); } }
    for name in names {
        if let Some((_,uri)) = lexical.and_then(|l| l.namespaces.iter().find(|(key,_)| key == &name)) {
            let prefix = name.strip_prefix("xmlns:").unwrap_or("");
            let current_uri = used.get(prefix).unwrap_or(uri);
            el.attrs.push((name.clone(),current_uri.clone()));
            scope.insert(prefix.into(),current_uri.clone());
            continue;
        }
        let Some((_,current)) = n.attrs.iter().find(|(key,_)| key == &name) else { continue };
        let (uri,local) = expanded_parts(&name);
        let attr_prefix = lexical.and_then(|l| l.attributes.iter().find(|(key,_)| key == &name))
            .map(|(_,spelling)| spelling.split_once(':').map_or("", |(p,_)| p).to_owned())
            .unwrap_or_else(|| mxl_output_prefix(uri,default,true));
        let attr_prefix = if uri.is_empty() { String::new() } else { unused_output_prefix(attr_prefix, uri, &used) };
        let attr_name = if attr_prefix.is_empty() { local.into() } else { format!("{attr_prefix}:{local}") };
        if !uri.is_empty() { bind_output(&mut el,&mut scope,&attr_prefix,uri); used.insert(attr_prefix.clone(),uri.into()); }
        let value = if name == "{http://www.w3.org/2001/XMLSchema-instance}type" {
            let (type_uri,type_name) = expanded_parts(current);
            let type_prefix = lexical.and_then(|l| l.qname_prefixes.iter().find(|(key,_)| key == &name))
                .map(|(_,p)| p.clone()).unwrap_or_else(|| mxl_output_prefix(type_uri,default,false));
            let type_prefix = if type_uri.is_empty() { String::new() } else { unused_output_prefix(type_prefix,type_uri,&used) };
            bind_output(&mut el,&mut scope,&type_prefix,type_uri);
            used.insert(type_prefix.clone(),type_uri.into());
            if type_prefix.is_empty() { type_name.into() } else { format!("{type_prefix}:{type_name}") }
        } else { current.clone() };
        el.attrs.push((attr_name,value));
    }
    bind_output(&mut el,&mut scope,&prefix,&n.namespace);
    if let Some((uri,local)) = &n.text_qname {
        let preferred = lexical.and_then(|l| l.qname_prefixes.iter().find(|(key,_)| key == "#text"))
            .map(|(_,p)| p.clone()).unwrap_or_else(|| mxl_output_prefix(uri,default,false));
        let text_prefix = if uri.is_empty() { String::new() } else { unused_output_prefix(preferred,uri,&used) };
        bind_output(&mut el,&mut scope,&text_prefix,uri);
        el.text = Some(if text_prefix.is_empty() { local.clone() } else { format!("{text_prefix}:{local}") });
    }
    else if !n.text.is_empty() { el.text = Some(n.text.clone()); }
    else if n.children.is_empty() { el.self_closing = n.self_closing || lexical.is_none(); }
    for child in &n.children { el.push(mxl_node_to_out_target(child,default,&scope)); }
    el
}

/// Дети-структура табличного документа — общая ОБЕИМ сериализациям (Designer-инлайн
/// `<Settings>` с `mxl:`-префиксом / EDT-сайдкар `SpreadsheetData.mxlx` с default-ns, `""`):
/// `[languageSettings]`, `columns/size`, `rowsItem/index/row/empty`, `[templateMode]`, `vgRows`
/// — содержимое сверено 1:1 по 11/11 парам корпуса SSL+coverage.
pub(crate) fn spreadsheet_settings_children(
    prefix: &'static str,
    ss: &MxlSpreadsheetSettings,
) -> Vec<OutElement> {
    let mut out = Vec::new();
    // Опциональный `<languageSettings>` ПЕРВЫМ (RE: РедактированиеТабличногоДокумента).
    if let Some(ls) = &ss.language_settings {
        let mut els = OutElement::branch(prefix, "languageSettings");
        els.push(OutElement::leaf(
            prefix,
            "currentLanguage",
            ls.current_language.clone(),
        ));
        els.push(OutElement::leaf(
            prefix,
            "defaultLanguage",
            ls.default_language.clone(),
        ));
        for li in &ls.languages {
            let mut info = OutElement::branch(prefix, "languageInfo");
            info.push(OutElement::leaf(prefix, "id", li.id.clone()));
            info.push(OutElement::leaf(prefix, "code", li.code.clone()));
            info.push(OutElement::leaf(
                prefix,
                "description",
                li.description.clone(),
            ));
            els.push(info);
        }
        out.push(els);
    }
    let mut columns = OutElement::branch(prefix, "columns");
    columns.push(OutElement::leaf(prefix, "size", ss.columns_size.clone()));
    out.push(columns);
    let mut rows_item = OutElement::branch(prefix, "rowsItem");
    rows_item.push(OutElement::leaf(prefix, "index", ss.rows_index.clone()));
    let mut row = OutElement::branch(prefix, "row");
    row.push(OutElement::leaf(
        prefix,
        "empty",
        if ss.row_empty { "true" } else { "false" },
    ));
    rows_item.push(row);
    out.push(rows_item);
    // Опциональный `<templateMode>` между rowsItem и vgRows.
    if let Some(tm) = ss.template_mode {
        out.push(OutElement::leaf(
            prefix,
            "templateMode",
            if tm { "true" } else { "false" },
        ));
    }
    out.push(OutElement::leaf(prefix, "vgRows", ss.vg_rows.clone()));
    out
}
