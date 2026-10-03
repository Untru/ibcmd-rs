//! READ · MXL / spreadsheet-document node building and `<Settings>` reading (Designer
//! spreadsheet-typed attributes + shared MXL body capture consumed by [`super::super::mxlx`]).

use super::*;

/// Скаляр-тип «Табличный документ»: РОВНО одна компонента `SpreadsheetDocument` без
/// квалификатора. Детерминант EDT-маркера `form:SpreadsheetDocumentExtInfo` (37/37 корпус
/// SSL+coverage: маркер ⟺ скаляр; составные типы, включающие `SpreadsheetDocument`, маркера НЕ
/// несут) — EDT-ридер сверяет (§1.0), Designer-ридер выводит.
pub(crate) fn is_scalar_spreadsheet_type(vt: &Option<TypeSpec>) -> bool {
    matches!(vt, Some(ts)
        if ts.parts.len() == 1
            && ts.parts[0].id == "SpreadsheetDocument"
            && ts.parts[0].qualifier.is_none())
}

/// Детерминант ПУСТОГО EDT-маркера extInfo реквизита класса Chart/GraphicalSchema:
/// РОВНО одна компонента `Chart`/`GraphicalSchema` без квалификатора ⇒ канон-токен
/// (эмиссия — `form:{токен}ExtInfo` с поправкой Scheme; witness ОценкаПроизводительности /
/// КартаМаршрутаБизнесПроцесса, определитель 1/1 в обоих корпусах). EDT-ридер сверяет
/// (§1.0), писатель регенерирует; Designer маркера не несёт.
pub(crate) fn scalar_marker_ext(vt: &Option<TypeSpec>) -> Option<&'static str> {
    match vt {
        Some(ts) if ts.parts.len() == 1 && ts.parts[0].qualifier.is_none() => {
            match ts.parts[0].id.as_str() {
                "Chart" => Some("Chart"),
                "GanttChart" => Some("GanttChart"),
                "GraphicalSchema" => Some("GraphicalSchema"),
                "GeographicalSchema" => Some("GeographicalSchema"),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Прочитать Designer `<Settings xmlns:mxl=… xsi:type="mxl:SpreadsheetDocument">` → пустой
/// табличный документ по умолчанию ([`MxlSpreadsheetSettings`]). Инлайн-`xmlns:mxl` и `xsi:type`
/// (уже склеймлен диспетчером) — атрибуты host'а. §1.0: любая незнакомая структура/под-элемент ⇒
/// ошибка (не Blob, не skip).
pub(crate) fn read_designer_spreadsheet_settings(
    s: &Element,
) -> Result<MxlSpreadsheetSettings, FormError> {
    // Инлайн-объявление `xmlns:mxl` на самом `<Settings>` — claim (иначе тотальность §1.0 поймает).
    if let Some(a) = s.attr("xmlns:mxl") {
        a.claimed.set(true);
    }
    s.claim();
    // БОГАТЫЙ вариант (живой заполненный документ) не влезает в минимальный конверт — захватываем
    // ПОЛНОЕ тело generic-tree'ем (byte-exact Designer-флейвор: `mxl:`-префикс, семантический порядок
    // атрибутов, без инлайн-редекла). Минимальный (пустой) — прежним структурным путём (SSL-safe).
    if is_minimal_spreadsheet_body(s, "mxl") {
        read_spreadsheet_settings_body(s, "mxl")
    } else {
        Ok(rich_spreadsheet_settings(capture_full_body(s)?, false))
    }
}

/// §1.0-детектор МИНИМАЛЬНОГО (пустого) табличного документа — РОВНО тот набор, что принимает
/// [`read_spreadsheet_settings_body`]: верхнеуровневые дети ⊆ `{languageSettings, columns, rowsItem,
/// templateMode, vgRows}` (с префиксом `prefix`), `columns` = единственный `<size>`, ровно один
/// `rowsItem`, чья `row` = единственный `<empty>`. Всё, что богаче (columnsItem, ячейки, namedItem,
/// merge, font/format, …), ⇒ `false` ⇒ generic-захват. НЕ клеймит (чистая проверка формы).
pub(crate) fn is_minimal_spreadsheet_body(host: &Element, prefix: &str) -> bool {
    const MIN_TOP: &[&str] = &[
        "languageSettings",
        "columns",
        "rowsItem",
        "templateMode",
        "vgRows",
    ];
    for c in &host.children {
        if c.prefix != prefix || !MIN_TOP.contains(&c.local.as_str()) {
            return false;
        }
    }
    let Some(cols) = host
        .children
        .iter()
        .find(|c| c.local == "columns" && c.prefix == prefix)
    else {
        return false;
    };
    if cols.children.len() != 1 || cols.children[0].local != "size" {
        return false;
    }
    let rows: Vec<&Element> = host
        .children
        .iter()
        .filter(|c| c.local == "rowsItem" && c.prefix == prefix)
        .collect();
    if rows.len() != 1 {
        return false;
    }
    let Some(row) = rows[0]
        .children
        .iter()
        .find(|c| c.local == "row" && c.prefix == prefix)
    else {
        return false;
    };
    row.children.len() == 1 && row.children[0].local == "empty"
}

/// Захватить ВСЕ верхнеуровневые дети `host` (тело табличного документа) в generic-tree, склеймив
/// каждое поддерево целиком (§1.0-тотальность — ничего не выброшено, всё регенерируется byte-exact).
pub(crate) fn capture_full_body(host: &Element) -> Result<Vec<MxlNode>, FormError> {
    // Native Settings uses actual enclosing declarations captured by the form reader.
    // Standalone sidecars begin with only the XML builtin and the empty default namespace.
    let mut scope = host.resolved_namespace_scope.borrow().as_ref().map(|scope| (**scope).clone())
        .unwrap_or_else(|| std::collections::BTreeMap::from([
            (String::new(), String::new()),
            ("xml".into(), "http://www.w3.org/XML/1998/namespace".into()),
        ]));
    bind_mxl_namespaces(host, &mut scope)?;
    let source_default = scope.get("").is_some_and(|u| u == super::super::MXL_NS_URI);
    host.children.iter().map(|el| {
        let node = build_mxl_node_scoped(el, &scope, source_default)?;
        el.claim_subtree();
        Ok(node)
    }).collect()
}

pub(super) fn bind_mxl_namespaces(el: &Element, scope: &mut std::collections::BTreeMap<String, String>) -> Result<(), FormError> {
    let mut seen = std::collections::BTreeSet::new();
    for a in &el.attrs {
        let prefix = if a.name == "xmlns" { "" } else if let Some(p) = a.name.strip_prefix("xmlns:") { p } else { continue };
        if a.name == "xmlns:" {
            return Err(FormError::Frame("MXL malformed namespace declaration".into()));
        }
        if !seen.insert(prefix) {
            return Err(FormError::Frame("MXL duplicate namespace declaration".into()));
        }
        if prefix == "xmlns" || (prefix == "xml" && a.value != "http://www.w3.org/XML/1998/namespace") {
            return Err(FormError::Frame("MXL invalid reserved namespace binding".into()));
        }
        if (!prefix.is_empty() && a.value.is_empty())
            || (prefix != "xml" && a.value == "http://www.w3.org/XML/1998/namespace")
            || a.value == "http://www.w3.org/2000/xmlns/"
        {
            return Err(FormError::Frame("MXL invalid reserved namespace binding".into()));
        }
        scope.insert(prefix.into(), a.value.clone());
        if !prefix.is_empty() { mxl_expanded(&format!("{prefix}:binding"), scope, false)?; }
    }
    Ok(())
}
fn mxl_expanded(name: &str, scope: &std::collections::BTreeMap<String, String>, default: bool) -> Result<String, FormError> {
    let (prefix, local) = name.split_once(':').unwrap_or(("", name));
    let start = |c: char| c == '_' || c.is_ascii_alphabetic() || matches!(c as u32,
        0xc0..=0xd6 | 0xd8..=0xf6 | 0xf8..=0x2ff | 0x370..=0x37d | 0x37f..=0x1fff
        | 0x200c..=0x200d | 0x2070..=0x218f | 0x2c00..=0x2fef | 0x3001..=0xd7ff
        | 0xf900..=0xfdcf | 0xfdf0..=0xfffd | 0x10000..=0xeffff);
    let valid = |part: &str| {
        let mut chars = part.chars();
        chars.next().is_some_and(start) && chars.all(|c| start(c) || c.is_ascii_digit()
            || matches!(c, '-' | '.' | '\u{b7}') || matches!(c as u32, 0x300..=0x36f | 0x203f..=0x2040))
    };
    if !valid(local) || (!prefix.is_empty() && !valid(prefix))
        || (name.contains(':') && prefix.is_empty())
    {
        return Err(FormError::Frame("MXL malformed QName".into()));
    }
    let uri = if prefix.is_empty() && !default { "" } else {
        scope.get(prefix).map(String::as_str).ok_or_else(|| FormError::Frame("MXL unbound namespace prefix".into()))?
    };
    Ok(if uri.is_empty() { local.into() } else { format!("{{{uri}}}{local}") })
}
fn build_mxl_node_scoped(el: &Element, inherited: &std::collections::BTreeMap<String, String>, source_default: bool) -> Result<MxlNode, FormError> {
    use morph1c_core::ir::form::MxlNodeLayout;
    let mut scope = inherited.clone();
    bind_mxl_namespaces(el, &mut scope)?;
    let element_name = if el.prefix.is_empty() { el.local.clone() } else { format!("{}:{}", el.prefix, el.local) };
    mxl_expanded(&element_name, &scope, true)?;
    let namespace = scope.get(&el.prefix).cloned().ok_or_else(|| FormError::Frame("MXL unbound element namespace".into()))?;
    let mut attrs = Vec::new();
    let mut lexical_attrs = Vec::new();
    let mut namespaces = Vec::new();
    let mut qnames = Vec::new();
    for a in &el.attrs {
        if a.name == "xmlns" || a.name.starts_with("xmlns:") {
            namespaces.push((a.name.clone(), a.value.clone()));
            lexical_attrs.push((a.name.clone(), a.name.clone()));
            continue;
        }
        let name = mxl_expanded(&a.name, &scope, false)?;
        if attrs.iter().any(|(n, _)| n == &name) {
            return Err(FormError::Frame("MXL duplicate expanded attribute".into()));
        }
        let value = if name == "{http://www.w3.org/2001/XMLSchema-instance}type" {
            let expanded = mxl_expanded(&a.value, &scope, true)?;
            qnames.push((name.clone(), a.value.split_once(':').map_or("", |(p, _)| p).into()));
            expanded
        } else { a.value.clone() };
        lexical_attrs.push((name.clone(), a.name.clone()));
        attrs.push((name, value));
    }
    attrs.sort_by(|a, b| a.0.cmp(&b.0));
    let children = el.children.iter().map(|c| build_mxl_node_scoped(c, &scope, source_default)).collect::<Result<Vec<_>, _>>()?;
    let qname_text = (namespace == "http://v8.1c.ru/8.1/data/core" && el.local == "Type")
        || attrs.iter().any(|(name,value)| name == "{http://www.w3.org/2001/XMLSchema-instance}type"
            && value == "{http://www.w3.org/2001/XMLSchema}QName");
    let text_qname = if qname_text && !el.text.is_empty() {
        let expanded = mxl_expanded(&el.text, &scope, true)?;
        let (uri, local) = if let Some(rest) = expanded.strip_prefix('{') {
            rest.split_once('}').expect("expanded QName")
        } else { ("", expanded.as_str()) };
        qnames.push(("#text".into(), el.text.split_once(':').map_or("", |(p,_)| p).into()));
        Some((uri.into(), local.into()))
    } else { None };
    let self_closing = children.is_empty() && el.text.is_empty();
    Ok(MxlNode {
        prefix: el.prefix.clone(), namespace, local: el.local.clone(), attrs, children,
        text: if text_qname.is_some() { String::new() } else { el.text.clone() }, text_qname, self_closing,
        source_layout: Some(MxlNodeLayout { default_spreadsheet_namespace: source_default, namespaces, attributes: lexical_attrs, qname_prefixes: qnames }),
    })
}

/// Собрать БОГАТЫЙ [`MxlSpreadsheetSettings`] (generic-tree тело + presence-бит конверта). Минимальные
/// структурные поля — САНТИНЕЛИ (не читаются: писатели ветвятся по `full_body`, cf громко отказывает).
pub(crate) fn rich_spreadsheet_settings(
    full_body: Vec<MxlNode>,
    envelope_without_pal: bool,
) -> MxlSpreadsheetSettings {
    MxlSpreadsheetSettings {
        envelope_without_pal,
        language_settings: None,
        columns_size: String::new(),
        rows_index: String::new(),
        row_empty: false,
        template_mode: None,
        vg_rows: String::new(),
        full_body: Some(full_body),
    }
}

/// Прочитать СТРУКТУРУ табличного документа (общую Designer-инлайну и EDT `.mxlx`-сайдкару) из
/// детей `host` с ns-префиксом `prefix` (`"mxl"` у Designer `<Settings>`, `""` у корня
/// `<document>` mxlx — там spreadsheet-ns объявлен ДЕФОЛТНЫМ). §1.0: любая незнакомая
/// структура/под-элемент ⇒ ошибка (не Blob, не skip). Смоделирован ровно наблюдаемый пустой
/// документ: `[languageSettings]`, `columns/size`, `rowsItem/index`, `rowsItem/row/empty`,
/// `[templateMode]`, `vgRows` — содержимое обоих сериализаций сверено 1:1 (11/11 корпус
/// SSL+coverage: designer-инлайн ⟺ сайдкар).
pub(crate) fn read_spreadsheet_settings_body(
    host: &Element,
    prefix: &str,
) -> Result<MxlSpreadsheetSettings, FormError> {
    let s = host;
    // ОПЦИОНАЛЬНЫЙ префикс `<languageSettings>` (RE: РедактированиеТабличногоДокумента).
    let language_settings = match s
        .children
        .iter()
        .find(|c| c.local == "languageSettings" && c.prefix == prefix)
    {
        Some(ls) => Some(read_mxl_language_settings(ls, prefix)?),
        None => None,
    };
    // `<columns><size>N</size></columns>` (ровно один ребёнок size).
    let columns = mxl_child(s, prefix, "columns", "SpreadsheetDocument")?;
    let columns_size = mxl_leaf(columns, prefix, "size")?;
    if columns.children.len() != 1 {
        return Err(FormError::Frame(
            "mxl columns: expected exactly <size> (§1.0)".into(),
        ));
    }
    // `<rowsItem><index>N</index><row><empty>b</empty></row></rowsItem>`.
    let rows_item = mxl_child(s, prefix, "rowsItem", "SpreadsheetDocument")?;
    let rows_index = mxl_leaf(rows_item, prefix, "index")?;
    let row = mxl_child(rows_item, prefix, "row", "rowsItem")?;
    let empty_text = mxl_leaf(row, prefix, "empty")?;
    let row_empty = match empty_text.as_str() {
        "true" => true,
        "false" => false,
        other => {
            return Err(FormError::Frame(format!(
                "mxl empty={other:?}, want bool (§1.0)"
            )))
        }
    };
    if row.children.len() != 1 {
        return Err(FormError::Frame(
            "mxl row: expected exactly <empty> (§1.0)".into(),
        ));
    }
    if rows_item.children.len() != 2 {
        return Err(FormError::Frame(
            "mxl rowsItem: expected exactly <index>+<row> (§1.0)".into(),
        ));
    }
    // ОПЦИОНАЛЬНЫЙ `<templateMode>b</templateMode>` (между rowsItem и vgRows).
    let template_mode = match s
        .children
        .iter()
        .find(|c| c.local == "templateMode" && c.prefix == prefix)
    {
        Some(tm) => {
            tm.claim_with_text();
            match tm.text.as_str() {
                "true" => Some(true),
                "false" => Some(false),
                other => {
                    return Err(FormError::Frame(format!(
                        "mxl templateMode={other:?}, want bool (§1.0)"
                    )))
                }
            }
        }
        None => None,
    };
    // `<vgRows>N</vgRows>`.
    let vg_rows = mxl_leaf(s, prefix, "vgRows")?;
    // columns+rowsItem+vgRows (3), плюс опциональные languageSettings-префикс и templateMode.
    let expected = 3 + language_settings.is_some() as usize + template_mode.is_some() as usize;
    if s.children.len() != expected {
        return Err(FormError::Frame(
            "mxl SpreadsheetDocument: expected [languageSettings]+<columns>+<rowsItem>+[templateMode]+<vgRows> (§1.0)".into(),
        ));
    }
    Ok(MxlSpreadsheetSettings {
        // Designer-инлайн / общий каркас: pal-присутствие не наблюдаемо здесь (это концерн
        // САЙДКАРА `.mxlx`). Дефолт `false` (SSL-флавор); `read_spreadsheet_mxlx` переопределит
        // его presence-битом конверта сайдкара.
        envelope_without_pal: false,
        language_settings,
        columns_size,
        rows_index,
        row_empty,
        template_mode,
        vg_rows,
        // МИНИМАЛЬНЫЙ вариант — богатого generic-tree нет (writers идут структурным путём).
        full_body: None,
    })
}

/// Прочитать `<languageSettings>` табличного документа → [`MxlLanguageSettings`]:
/// `currentLanguage`, `defaultLanguage` + список `<languageInfo>` (каждый —
/// `id`+`code`+`description`). §1.0-strict. Общая обоим сериализациям (Designer `mxl:`-префикс /
/// mxlx-сайдкар default-ns).
pub(crate) fn read_mxl_language_settings(
    ls: &Element,
    prefix: &str,
) -> Result<MxlLanguageSettings, FormError> {
    ls.claim();
    let current_language = mxl_leaf(ls, prefix, "currentLanguage")?;
    let default_language = mxl_leaf(ls, prefix, "defaultLanguage")?;
    let mut languages = Vec::new();
    for li in ls
        .children
        .iter()
        .filter(|c| c.local == "languageInfo" && c.prefix == prefix)
    {
        li.claim();
        let id = mxl_leaf(li, prefix, "id")?;
        let code = mxl_leaf(li, prefix, "code")?;
        let description = mxl_leaf(li, prefix, "description")?;
        if li.children.len() != 3 {
            return Err(FormError::Frame(
                "mxl languageInfo: expected <id>+<code>+<description> (§1.0)".into(),
            ));
        }
        languages.push(MxlLanguageInfo {
            id,
            code,
            description,
        });
    }
    // currentLanguage + defaultLanguage + N languageInfo.
    if ls.children.len() != 2 + languages.len() {
        return Err(FormError::Frame(
            "mxl languageSettings: expected currentLanguage+defaultLanguage+languageInfo* (§1.0)"
                .into(),
        ));
    }
    Ok(MxlLanguageSettings {
        current_language,
        default_language,
        languages,
    })
}

/// Найти обязательного ребёнка `<prefix:local>` внутри `parent` (иначе §1.0-ошибка) и claim'нуть.
pub(crate) fn mxl_child<'a>(
    parent: &'a Element,
    prefix: &str,
    local: &str,
    ctx: &str,
) -> Result<&'a Element, FormError> {
    let c = parent
        .children
        .iter()
        .find(|c| c.local == local && c.prefix == prefix)
        .ok_or_else(|| FormError::Frame(format!("{ctx}: missing mxl <{local}> (§1.0)")))?;
    c.claim();
    Ok(c)
}

/// Прочитать текст обязательного листа `<prefix:local>` внутри `parent` (claim'ит).
pub(crate) fn mxl_leaf(parent: &Element, prefix: &str, local: &str) -> Result<String, FormError> {
    let c = parent
        .children
        .iter()
        .find(|c| c.local == local && c.prefix == prefix)
        .ok_or_else(|| FormError::Frame(format!("missing mxl <{local}> (§1.0)")))?;
    c.claim_with_text();
    if !c.children.is_empty() {
        return Err(FormError::Frame(format!(
            "mxl <{local}>: must be a text leaf (§1.0)"
        )));
    }
    Ok(c.text.clone())
}

