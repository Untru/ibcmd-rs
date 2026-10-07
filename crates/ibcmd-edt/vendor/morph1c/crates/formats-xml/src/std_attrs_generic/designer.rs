//! Designer-проекция блока StandardAttributes: decode/emit + <xr:…>/<v8:item> leaf-хелперы.

use super::*;

// ---- Designer ---------------------------------------------------------------

/// Найти Designer-обёртку `<StandardAttributes>` по варианту.
fn designer_wrapper<'a>(
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    root: &'a Element,
) -> Result<Option<&'a Element>, String> {
    match variant {
        StdAttrsVariant::Root => {
            let host = match root.child(decl.root_elem) {
                Some(h) => h,
                None => return Ok(None),
            };
            let props = match host.child("Properties") {
                Some(p) => p,
                None => return Ok(None),
            };
            Ok(props.child("StandardAttributes"))
        }
        StdAttrsVariant::Tabular => Ok(root.child("StandardAttributes")),
    }
}

pub(crate) fn decode_designer(
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    root: &Element,
    version: FormatVersion,
) -> Result<PropertyValue, String> {
    let wrapper = designer_wrapper(decl, variant, root)?;
    // Опциональный блок целиком: отсутствие обёртки ⇒ счётчик 0 (валиден лишь если
    // объявлен 0-длинный вариант; иначе select_attrs даёт §1.0-ошибку). Невариадные
    // виды НЕ имеют 0-варианта → отсутствие обёртки корректно останется ошибкой.
    let wrapper = match wrapper {
        Some(w) => w,
        None => {
            let attrs = select_attrs(decl, variant, 0)?;
            debug_assert!(attrs.is_empty());
            return Ok(pack(Vec::new()));
        }
    };
    if !wrapper.prefix.is_empty() || !wrapper.attrs.is_empty() || !wrapper.text.is_empty() {
        return Err(format!(
            "{} <StandardAttributes> must be an unprefixed attribute-less container",
            decl.label
        ));
    }
    // Вариадный выбор набора по числу `<xr:StandardAttribute>` в обёртке; виды с
    // NameVar-слотом — по последовательности `name="…"` (наборы равной длины).
    let attrs = if decl_name_slot(decl).is_some() {
        let names = wrapper
            .children
            .iter()
            .map(|a| {
                a.attrs
                    .iter()
                    .find(|at| at.name == "name")
                    .map(|at| at.value.as_str())
                    .ok_or_else(|| {
                        format!(
                            "{} <xr:StandardAttribute> without name attribute (§1.0)",
                            decl.label
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        select_attrs_by_names(decl, variant, &names)?
    } else {
        select_attrs(decl, variant, wrapper.children.len())?
    };
    let leaves = dense_leaves_for(decl, version);
    let mut records = Vec::with_capacity(attrs.len());
    for (attr_el, attr) in wrapper.children.iter().zip(attrs) {
        records.push(decode_designer_attr(decl, attr_el, attr, &leaves)?);
        attr_el.claim();
    }
    wrapper.claim();
    Ok(pack(records))
}

/// Декодировать ОДИН `<xr:StandardAttribute>` в variable-запись. НЕ claim'ит сам узел
/// (это делает вызывающий). pub(crate): переиспользуется STS-кодеком для вложенных
/// блоков стандартной ТЧ ([`crate::std_tabular_sections`]).
pub(crate) fn decode_designer_attr(
    decl: &StdAttrsDecl,
    attr_el: &Element,
    attr: &AttrDecl,
    leaves: &[&'static DenseLeafRule],
) -> Result<Vec<PropertyValue>, String> {
    if attr_el.local != "StandardAttribute" || attr_el.prefix != "xr" {
        return Err(format!(
            "{} standardAttributes: expected <xr:StandardAttribute>, got <{}>",
            decl.label, attr_el.local
        ));
    }
    if attr_el.attrs.len() != 1
        || attr_el.attrs[0].name != "name"
        || attr_el.attrs[0].value != *attr.name
    {
        return Err(format!(
            "{} <xr:StandardAttribute>: expected single name={:?}",
            decl.label, attr.name
        ));
    }
    attr_el.attrs[0].claimed.set(true);
    if attr_el.children.len() != leaves.len() {
        return Err(format!(
            "{} <xr:StandardAttribute name={:?}>: expected {} leaves, found {}",
            decl.label,
            attr.name,
            leaves.len(),
            attr_el.children.len()
        ));
    }
    let mut rec = default_record(decl);
    // NameVar-слот: Designer держит имя АТРИБУТОМ (не dense-листом) — кладём его в слот
    // здесь (сверка с вариантом уже прошла выше), X-парно EDT `<name>`-листу.
    if let Some(i) = decl_name_slot(decl) {
        rec[i] = PropertyValue::Str(attr.name.to_string());
    }
    for (child, (local, leaf)) in attr_el.children.iter().zip(leaves.iter().copied()) {
        if child.local != *local || child.prefix != "xr" {
            return Err(format!(
                "{} <xr:StandardAttribute name={:?}>: expected <xr:{local}>, got <{}>",
                decl.label, attr.name, child.local
            ));
        }
        match leaf {
            DenseLeaf::VarLoc(i) => rec[*i] = take_localized_v8(child)?,
            DenseLeaf::VarStr(i) => rec[*i] = take_plain_v8(child)?,
            DenseLeaf::VarBool(i) => {
                child.claim_with_text();
                rec[*i] = match child.text.as_str() {
                    "true" => PropertyValue::Bool(true),
                    "false" => PropertyValue::Bool(false),
                    o => return Err(format!("<xr:{local}> bool literal expected, got {o:?}")),
                };
            }
            DenseLeaf::VarEnum(i) => {
                child.claim_with_text();
                rec[*i] = PropertyValue::Enum(Token::new(child.text.clone()));
            }
            DenseLeaf::VarValue(i) => {
                child.claim();
                rec[*i] = value_codec::decode(ValueDialect::Designer, child)?;
            }
            DenseLeaf::VarCpl(i) => {
                child.claim();
                match choice_param_links::decode_designer(child) {
                    Decoded::Present(v) => rec[*i] = v,
                    Decoded::Error(e) => return Err(e),
                    Decoded::Absent => return Err("ChoiceParameterLinks absent".into()),
                }
            }
            DenseLeaf::VarCp(i) => {
                child.claim();
                match choice_parameters::decode_designer(child) {
                    Decoded::Present(v) => rec[*i] = v,
                    Decoded::Error(e) => return Err(e),
                    Decoded::Absent => return Err("ChoiceParameters absent".into()),
                }
            }
            DenseLeaf::VarLbt(i) => {
                child.claim();
                match link_by_type::decode_with_item(
                    LinkByTypeDialect::Designer,
                    child,
                    lbt_item(attr.name),
                ) {
                    Decoded::Present(v) => rec[*i] = v,
                    Decoded::Error(e) => return Err(e),
                    Decoded::Absent => return Err("LinkByType absent".into()),
                }
            }
            DenseLeaf::VarNameConst(which) => {
                expect_designer_text(child, name_const(attr, *which))?
            }
            DenseLeaf::Empty => verify_empty(child)?,
            DenseLeaf::Nil => verify_nil(child)?,
            DenseLeaf::Text(t) => expect_designer_text(child, t)?,
        }
    }
    Ok(rec)
}

pub(crate) fn emit_designer(
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    value: &PropertyValue,
    version: FormatVersion,
) -> Result<Vec<OutElement>, String> {
    // Вариадный выбор набора по числу IR-записей (NameVar-виды — по слот-именам).
    let attrs = select_attrs_for_write(decl, variant, value)?;
    let records = unpack(decl, value, attrs.len())?;
    // Опциональный блок целиком (0-длинный вариант): обёртка НЕ эмитится.
    if attrs.is_empty() {
        return Ok(Vec::new());
    }
    let mut wrapper = OutElement::branch("", "StandardAttributes");
    let leaves = dense_leaves_for(decl, version);
    for (attr, rec) in attrs.iter().zip(&records) {
        wrapper.push(emit_designer_attr(attr, rec, &leaves)?);
    }
    Ok(vec![wrapper])
}

/// Эмитировать ОДИН `<xr:StandardAttribute>`. pub(crate): переиспользуется STS-кодеком
/// для вложенных блоков ([`crate::std_tabular_sections`]).
pub(crate) fn emit_designer_attr(
    attr: &AttrDecl,
    rec: &[PropertyValue],
    leaves: &[&'static DenseLeafRule],
) -> Result<OutElement, String> {
    let mut attr_el = OutElement::branch("xr", "StandardAttribute").attr("name", attr.name);
    for (local, leaf) in leaves.iter().copied() {
        let el = match leaf {
            DenseLeaf::VarLoc(i) => emit_localized_v8(local, &rec[*i])?,
            DenseLeaf::VarStr(i) => emit_plain_v8(local, &rec[*i])?,
            DenseLeaf::VarBool(i) => match &rec[*i] {
                PropertyValue::Bool(b) => {
                    OutElement::leaf("xr", *local, if *b { "true" } else { "false" })
                }
                o => return Err(format!("<xr:{local}> expects Bool, got {:?}", o.kind())),
            },
            DenseLeaf::VarEnum(i) => match &rec[*i] {
                PropertyValue::Enum(t) => OutElement::leaf("xr", *local, t.as_str().to_string()),
                o => return Err(format!("<xr:{local}> expects Enum, got {:?}", o.kind())),
            },
            DenseLeaf::VarValue(i) => {
                value_codec::encode(ValueDialect::Designer, "xr", local, as_value(&rec[*i])?)?
            }
            DenseLeaf::VarCpl(i) => choice_param_links::encode_designer("xr", local, &rec[*i])?,
            DenseLeaf::VarCp(i) => choice_parameters::emit_designer("xr", local, &rec[*i])?,
            DenseLeaf::VarLbt(i) => link_by_type::encode_with_item(
                LinkByTypeDialect::Designer,
                "xr",
                local,
                &rec[*i],
                lbt_item(attr.name),
            )?,
            DenseLeaf::VarNameConst(which) => {
                OutElement::leaf("xr", *local, name_const(attr, *which).to_string())
            }
            DenseLeaf::Empty => OutElement::self_closing("xr", *local),
            DenseLeaf::Nil => OutElement::self_closing("xr", *local).attr("xsi:nil", "true"),
            DenseLeaf::Text(t) => OutElement::leaf("xr", *local, (*t).to_string()),
        };
        attr_el.push(el);
    }
    Ok(attr_el)
}

fn verify_empty(child: &Element) -> Result<(), String> {
    if !child.attrs.is_empty() || !child.children.is_empty() || !child.text.is_empty() {
        return Err(format!("<xr:{}> must be empty self-closing", child.local));
    }
    child.claim();
    Ok(())
}

fn verify_nil(child: &Element) -> Result<(), String> {
    if child.children.is_empty()
        && child.text.is_empty()
        && child.attrs.len() == 1
        && child.attrs[0].name == "xsi:nil"
        && child.attrs[0].value == "true"
    {
        child.attrs[0].claimed.set(true);
        child.claim();
        Ok(())
    } else {
        Err(format!(
            "<xr:{}> must be self-closing xsi:nil=\"true\"",
            child.local
        ))
    }
}

fn expect_designer_text(child: &Element, want: &str) -> Result<(), String> {
    if !child.attrs.is_empty() || !child.children.is_empty() || child.text != want {
        return Err(format!(
            "<xr:{}>: expected text {want:?}, got {:?}",
            child.local, child.text
        ));
    }
    child.claim_with_text();
    Ok(())
}

/// Designer plain-text `<xr:Comment>text</xr:Comment>` (variable). Пустой → "". Claim'ит.
/// pub(crate): переиспользуется STS-кодеком ([`crate::std_tabular_sections`]).
pub(crate) fn take_plain_v8(el: &Element) -> Result<PropertyValue, String> {
    if !el.attrs.is_empty() || !el.children.is_empty() {
        return Err(format!(
            "stdAttr <xr:{}>: expected plain text leaf",
            el.local
        ));
    }
    el.claim_with_text();
    Ok(PropertyValue::Str(el.text.clone()))
}

/// Designer localized `<xr:Synonym>/…` (`<v8:item>`). Claim'ит.
/// pub(crate): переиспользуется STS-кодеком ([`crate::std_tabular_sections`]).
pub(crate) fn take_localized_v8(el: &Element) -> Result<PropertyValue, String> {
    el.claim();
    let mut pairs: Vec<(Lang, String)> = Vec::new();
    for item in &el.children {
        if item.local != "item" || item.prefix != "v8" {
            return Err(format!(
                "stdAttr localized v8: expected <v8:item>, got <{}>",
                item.local
            ));
        }
        item.claim();
        let mut it = item.children.iter();
        let lang = it
            .next()
            .filter(|e| e.local == "lang" && e.prefix == "v8")
            .ok_or("v8:item missing v8:lang")?;
        let content = it
            .next()
            .filter(|e| e.local == "content" && e.prefix == "v8")
            .ok_or("v8:item missing v8:content")?;
        if it.next().is_some() {
            return Err("stdAttr localized v8: <v8:item> has extra child".into());
        }
        lang.claim_with_text();
        content.claim_with_text();
        pairs.push((Lang::new(lang.text.clone()), content.text.clone()));
    }
    Ok(PropertyValue::Localized(pairs))
}

/// Designer plain-text emit `<xr:Comment/>` (пусто) / `<xr:Comment>text</xr:Comment>`.
/// pub(crate): переиспользуется STS-кодеком ([`crate::std_tabular_sections`]).
pub(crate) fn emit_plain_v8(tag: &str, v: &PropertyValue) -> Result<OutElement, String> {
    let s = as_str(v)?;
    if s.is_empty() {
        return Ok(OutElement::self_closing("xr", tag));
    }
    Ok(OutElement::leaf("xr", tag, s.to_string()))
}

/// Designer localized emit `<xr:Tag/>` (empty) / `<xr:Tag><v8:item>…`.
fn emit_localized_v8(tag: &str, v: &PropertyValue) -> Result<OutElement, String> {
    let pairs = as_localized(v)?;
    if pairs.is_empty() {
        return Ok(OutElement::self_closing("xr", tag));
    }
    let mut c = OutElement::branch("xr", tag);
    for (lang, content) in pairs {
        let mut item = OutElement::branch("v8", "item");
        item.push(OutElement::leaf("v8", "lang", lang.as_str().to_string()));
        item.push(OutElement::leaf("v8", "content", content.clone()));
        c.push(item);
    }
    Ok(c)
}
