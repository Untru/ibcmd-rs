//! EDT-проекция блока standardAttributes: decode/emit + позиционные (peekable) leaf-хелперы.

use super::*;

// ---- EDT --------------------------------------------------------------------

pub(crate) fn decode_edt(
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    root: &Element,
) -> Result<PropertyValue, String> {
    let blocks: Vec<&Element> = root
        .children
        .iter()
        .filter(|c| c.local == "standardAttributes" && c.prefix.is_empty())
        .collect();
    // ВАРИАДНЫЙ выбор: набор атрибутов дискриминируется по НАБЛЮДАЕМОМУ числу блоков
    // (условный атрибут ⇒ наборы разной длины; опциональный блок ⇒ вариант длины 0).
    // Для невариадных видов — прежний фиксированный набор (тот же путь, счётчик обязан
    // совпасть с единственной длиной). Виды с NameVar-слотом (наборы равной длины)
    // выбирают по последовательности имён `<name>`.
    let attrs = if decl_name_slot(decl).is_some() {
        let names = blocks
            .iter()
            .map(|b| {
                b.children
                    .iter()
                    .find(|c| c.local == "name" && c.prefix.is_empty())
                    .map(|c| c.text.as_str())
                    .ok_or_else(|| {
                        format!("{} standardAttributes: block without <name> (§1.0)", decl.label)
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        select_attrs_by_names(decl, variant, &names)?
    } else {
        select_attrs(decl, variant, blocks.len())?
    };
    let mut records = Vec::with_capacity(attrs.len());
    for (block, attr) in blocks.iter().zip(attrs) {
        records.push(decode_edt_block(decl, block, attr)?);
        block.claim();
    }
    Ok(pack(records))
}

/// Декодировать ОДИН EDT-блок `<standardAttributes>` в variable-запись. НЕ claim'ит сам
/// блок (это делает вызывающий). pub(crate): переиспользуется STS-кодеком для ВЛОЖЕННЫХ
/// блоков стандартной ТЧ ([`crate::std_tabular_sections`]).
pub(crate) fn decode_edt_block(
    decl: &StdAttrsDecl,
    block: &Element,
    attr: &AttrDecl,
) -> Result<Vec<PropertyValue>, String> {
    if !block.attrs.is_empty() {
        return Err(format!(
            "{} standardAttributes block must have no attributes",
            decl.label
        ));
    }
    let mut rec = default_record(decl);
    let mut it = block.children.iter().peekable();
    for leaf in decl.edt_leaves {
        match leaf {
            EdtLeaf::ConstText { tag, text } => expect_text_leaf(decl, &mut it, tag, text)?,
            EdtLeaf::Name => expect_text_leaf(decl, &mut it, "name", attr.name)?,
            EdtLeaf::NameVar(i) => {
                // Имя сверяется с ВЫБРАННЫМ вариантом (выбор шёл по именам) и хранится в
                // слоте — write-сторона выбирает вариант по нему.
                expect_text_leaf(decl, &mut it, "name", attr.name)?;
                rec[*i] = PropertyValue::Str(attr.name.to_string());
            }
            EdtLeaf::OptLoc(tag, i) => take_opt_localized(&mut it, tag, &mut rec[*i])?,
            EdtLeaf::OptStr(tag, i) => take_opt_plain(&mut it, tag, &mut rec[*i])?,
            EdtLeaf::OptBool(tag, i) => take_opt_bool(&mut it, tag, &mut rec[*i])?,
            EdtLeaf::OptEnum(tag, i) => take_opt_enum(&mut it, tag, &mut rec[*i])?,
            EdtLeaf::OptCpl(tag, i) => take_opt_cpl(&mut it, tag, &mut rec[*i])?,
            EdtLeaf::OptCp(tag, i) => take_opt_cp(&mut it, tag, &mut rec[*i])?,
            EdtLeaf::OptLbt(tag, i) => {
                take_opt_lbt(&mut it, tag, &mut rec[*i], lbt_item(attr.name))?
            }
            EdtLeaf::ReqValue(tag, i) => {
                let fv = it
                    .next()
                    .filter(|c| c.local == *tag && c.prefix.is_empty())
                    .ok_or_else(|| {
                        format!("{} standardAttributes: expected <{tag}>", decl.label)
                    })?;
                fv.claim();
                rec[*i] = value_codec::decode(ValueDialect::Edt, fv)?;
            }
            EdtLeaf::NameConst { tag, which, omit } => {
                let want = name_const(attr, *which);
                if want != *omit {
                    expect_text_leaf(decl, &mut it, tag, want)?;
                }
            }
            EdtLeaf::ConstValueUndef { tag } => {
                expect_value_undefined(decl, &mut it, tag, ValueDialect::Edt)?
            }
        }
    }
    if let Some(extra) = it.next() {
        return Err(format!(
            "{} standardAttributes[{}]: unexpected extra leaf <{}> (§1.0)",
            decl.label, attr.name, extra.local
        ));
    }
    Ok(rec)
}

pub(crate) fn emit_edt(
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    value: &PropertyValue,
) -> Result<Vec<OutElement>, String> {
    // Вариадный выбор набора по числу IR-записей (симметрично read-счётчику блоков).
    let attrs = select_attrs_for_write(decl, variant, value)?;
    let records = unpack(decl, value, attrs.len())?;
    let mut out = Vec::with_capacity(attrs.len());
    for (attr, rec) in attrs.iter().zip(&records) {
        out.push(emit_edt_block(decl, attr, rec)?);
    }
    Ok(out)
}

/// Эмитировать ОДИН EDT-блок `<standardAttributes>`. pub(crate): переиспользуется
/// STS-кодеком для вложенных блоков ([`crate::std_tabular_sections`]).
pub(crate) fn emit_edt_block(
    decl: &StdAttrsDecl,
    attr: &AttrDecl,
    rec: &[PropertyValue],
) -> Result<OutElement, String> {
    let mut block = OutElement::branch("", "standardAttributes");
    for leaf in decl.edt_leaves {
        match leaf {
            EdtLeaf::ConstText { tag, text } => {
                block.push(OutElement::leaf("", *tag, (*text).to_string()))
            }
            EdtLeaf::Name => block.push(OutElement::leaf("", "name", attr.name.to_string())),
            EdtLeaf::NameVar(i) => {
                // Вариант ВЫБРАН по слот-именам записей — защитная сверка (§1.0).
                if as_str(&rec[*i])? != attr.name {
                    return Err(format!(
                        "{} standardAttributes: record name {:?} != selected variant attr {:?}",
                        decl.label, &rec[*i], attr.name
                    ));
                }
                block.push(OutElement::leaf("", "name", attr.name.to_string()));
            }
            EdtLeaf::OptLoc(tag, i) => {
                // MULTI-SIBLING (двуязычный ERP): по ОДНОМУ `<tag>` на язык-пару (как root
                // LocalizedKeyVal); пусто → НОЛЬ узлов. Byte-exact для N языков.
                for el in emit_localized_keyval(tag, &rec[*i])? {
                    block.push(el);
                }
            }
            EdtLeaf::OptStr(tag, i) => {
                if let Some(el) = emit_opt_text(tag, &rec[*i])? {
                    block.push(el);
                }
            }
            EdtLeaf::OptBool(tag, i) => {
                if let Some(el) = emit_opt_bool(tag, &rec[*i])? {
                    block.push(el);
                }
            }
            EdtLeaf::OptEnum(tag, i) => {
                let def = slot_enum_default(decl, *i)?;
                if let Some(el) = emit_opt_enum(tag, &rec[*i], def)? {
                    block.push(el);
                }
            }
            EdtLeaf::OptCpl(tag, i) => {
                for el in emit_opt_cpl(tag, &rec[*i])? {
                    block.push(el);
                }
            }
            EdtLeaf::OptCp(_tag, i) => {
                // MULTI-SIBLING: по `<choiceParameters>` на связку; пусто → ноль узлов.
                match &rec[*i] {
                    PropertyValue::List(l) if l.is_empty() => {}
                    v @ PropertyValue::List(_) => {
                        for el in choice_parameters::emit_edt(v)? {
                            block.push(el);
                        }
                    }
                    other => {
                        return Err(format!(
                            "stdAttr choiceParameters expects List, got {:?}",
                            other.kind()
                        ))
                    }
                }
            }
            EdtLeaf::OptLbt(tag, i) => match &rec[*i] {
                PropertyValue::Str(s) if s.is_empty() => {}
                v @ PropertyValue::Str(_) => block.push(link_by_type::encode_with_item(
                    LinkByTypeDialect::Edt,
                    "",
                    tag,
                    v,
                    lbt_item(attr.name),
                )?),
                other => {
                    return Err(format!(
                        "stdAttr linkByType expects Str, got {:?}",
                        other.kind()
                    ))
                }
            },
            EdtLeaf::ReqValue(tag, i) => block.push(value_codec::encode(
                ValueDialect::Edt,
                "",
                tag,
                as_value(&rec[*i])?,
            )?),
            EdtLeaf::NameConst { tag, which, omit } => {
                let want = name_const(attr, *which);
                if want != *omit {
                    block.push(OutElement::leaf("", *tag, want.to_string()));
                }
            }
            EdtLeaf::ConstValueUndef { tag } => block.push(value_codec::encode(
                ValueDialect::Edt,
                "",
                tag,
                &undefined_spec(),
            )?),
        }
    }
    Ok(block)
}

/// Дефолт-литерал enum-слота (для омиссии в EDT / сравнения).
fn slot_enum_default(decl: &StdAttrsDecl, i: usize) -> Result<&'static str, String> {
    match decl.var_slots[i] {
        VarSlotKind::Enum(d) => Ok(d),
        _ => Err(format!(
            "{} standardAttributes: slot {i} is not an enum slot",
            decl.label
        )),
    }
}

// EDT optional-leaf decoders (positional, peekable).

pub(crate) fn take_opt_localized<'a, I: Iterator<Item = &'a Element>>(
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    slot: &mut PropertyValue,
) -> Result<(), String> {
    // MULTI-SIBLING (двуязычный ERP): N языков кодируются N СИБЛИНГАМИ `<tag>`, каждый с
    // одной парой `<key>/<value>` (как root `LocalizedKeyVal`). Одноязычный SSL — 1 сиблинг
    // с 1+ парами. Читаем ВСЕ подряд идущие `<tag>` и сливаем пары (source-order preserve).
    let mut pairs: Vec<(Lang, String)> = Vec::new();
    let mut any = false;
    while let Some(c) = it.peek() {
        if c.local != tag || !c.prefix.is_empty() {
            break;
        }
        any = true;
        let el = it.next().unwrap();
        match take_localized(el)? {
            PropertyValue::Localized(mut p) => pairs.append(&mut p),
            _ => unreachable!("take_localized returns Localized"),
        }
    }
    if any {
        *slot = PropertyValue::Localized(pairs);
    }
    Ok(())
}

pub(crate) fn take_opt_plain<'a, I: Iterator<Item = &'a Element>>(
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    slot: &mut PropertyValue,
) -> Result<(), String> {
    if let Some(c) = it.peek() {
        if c.local == tag && c.prefix.is_empty() {
            let el = it.next().unwrap();
            if !el.attrs.is_empty() || !el.children.is_empty() {
                return Err(format!("stdAttr <{tag}>: expected plain text leaf"));
            }
            el.claim_with_text();
            *slot = PropertyValue::Str(el.text.clone());
        }
    }
    Ok(())
}

fn take_opt_bool<'a, I: Iterator<Item = &'a Element>>(
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    slot: &mut PropertyValue,
) -> Result<(), String> {
    if let Some(c) = it.peek() {
        if c.local == tag && c.prefix.is_empty() {
            let el = it.next().unwrap();
            el.claim_with_text();
            *slot = match el.text.as_str() {
                "true" => PropertyValue::Bool(true),
                other => {
                    return Err(format!(
                        "<{tag}>: presence-bool must be \"true\", got {other:?}"
                    ))
                }
            };
        }
    }
    Ok(())
}

pub(crate) fn take_opt_enum<'a, I: Iterator<Item = &'a Element>>(
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    slot: &mut PropertyValue,
) -> Result<(), String> {
    if let Some(c) = it.peek() {
        if c.local == tag && c.prefix.is_empty() {
            let el = it.next().unwrap();
            el.claim_with_text();
            *slot = PropertyValue::Enum(Token::new(el.text.clone()));
        }
    }
    Ok(())
}

fn take_opt_cpl<'a, I: Iterator<Item = &'a Element>>(
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    slot: &mut PropertyValue,
) -> Result<(), String> {
    // MULTI-SIBLING (ERP-witnessed: 2 связи = 2 сиблинга подряд, Parent-атрибут
    // СкладскиеЯчейки); ноль сиблингов = дефолт-омиссия, слот не трогаем.
    let mut els: Vec<&Element> = Vec::new();
    while let Some(c) = it.peek() {
        if c.local != tag || !c.prefix.is_empty() {
            break;
        }
        let el = it.next().unwrap();
        el.claim();
        els.push(el);
    }
    if els.is_empty() {
        return Ok(());
    }
    match choice_param_links::decode_edt_siblings(&els) {
        Decoded::Present(v) => {
            *slot = v;
            Ok(())
        }
        Decoded::Error(e) => Err(e),
        Decoded::Absent => Err("ChoiceParameterLinks absent".into()),
    }
}

/// choiceParameters std-attrs (EDT): MULTI-SIBLING — собрать ВСЕ подряд идущие `<tag>` и
/// декодировать общим codec'ом (ноль сиблингов = дефолт-омиссия, слот не трогаем).
fn take_opt_cp<'a, I: Iterator<Item = &'a Element>>(
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    slot: &mut PropertyValue,
) -> Result<(), String> {
    let mut els: Vec<&Element> = Vec::new();
    while let Some(c) = it.peek() {
        if c.local != tag || !c.prefix.is_empty() {
            break;
        }
        els.push(it.next().unwrap());
    }
    if els.is_empty() {
        return Ok(());
    }
    match choice_parameters::decode_edt_siblings(&els) {
        Decoded::Present(v) => {
            *slot = v;
            Ok(())
        }
        Decoded::Error(e) => Err(e),
        Decoded::Absent => Err("choiceParameters absent".into()),
    }
}

/// linkByType std-attrs (EDT): один опциональный `<tag>` (омитится при пустом пути).
fn take_opt_lbt<'a, I: Iterator<Item = &'a Element>>(
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    slot: &mut PropertyValue,
    item: i64,
) -> Result<(), String> {
    if let Some(c) = it.peek() {
        if c.local == tag && c.prefix.is_empty() {
            let el = it.next().unwrap();
            el.claim();
            match link_by_type::decode_with_item(LinkByTypeDialect::Edt, el, item) {
                Decoded::Present(v) => *slot = v,
                Decoded::Error(e) => return Err(e),
                Decoded::Absent => return Err("linkByType absent".into()),
            }
        }
    }
    Ok(())
}

fn expect_text_leaf<'a, I: Iterator<Item = &'a Element>>(
    decl: &StdAttrsDecl,
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    want: &str,
) -> Result<(), String> {
    let el = it
        .next()
        .ok_or_else(|| format!("{} standardAttributes: missing <{tag}>", decl.label))?;
    if el.local != tag || !el.prefix.is_empty() {
        return Err(format!(
            "{} standardAttributes: expected <{tag}>, got <{}>",
            decl.label, el.local
        ));
    }
    if !el.attrs.is_empty() || !el.children.is_empty() || el.text != want {
        return Err(format!(
            "{} standardAttributes <{tag}>: expected text {want:?}, got {:?}",
            decl.label, el.text
        ));
    }
    el.claim_with_text();
    Ok(())
}

fn expect_value_undefined<'a, I: Iterator<Item = &'a Element>>(
    decl: &StdAttrsDecl,
    it: &mut std::iter::Peekable<I>,
    tag: &str,
    dialect: ValueDialect,
) -> Result<(), String> {
    let el = it
        .next()
        .ok_or_else(|| format!("{} standardAttributes: missing <{tag}>", decl.label))?;
    if el.local != tag || !el.prefix.is_empty() {
        return Err(format!(
            "{} standardAttributes: expected <{tag}>, got <{}>",
            decl.label, el.local
        ));
    }
    el.claim();
    match value_codec::decode(dialect, el)? {
        PropertyValue::Value(ValueSpec {
            kind: ValueScalarKind::Undefined,
            ..
        }) => Ok(()),
        other => Err(format!(
            "{} standardAttributes <{tag}>: expected Undefined, got {:?}",
            decl.label,
            other.kind()
        )),
    }
}

/// EDT localized container `<synonym>/<toolTip>/…` (`<key>/<value>`). Claim'ит.
/// pub(crate): переиспользуется STS-кодеком ([`crate::std_tabular_sections`]).
pub(crate) fn take_localized(el: &Element) -> Result<PropertyValue, String> {
    el.claim();
    let mut pairs: Vec<(Lang, String)> = Vec::new();
    let mut it = el.children.iter();
    while let Some(k) = it.next() {
        if k.local != "key" || !k.prefix.is_empty() {
            return Err(format!(
                "stdAttr localized: expected <key>, got <{}>",
                k.local
            ));
        }
        k.claim_with_text();
        let v = it
            .next()
            .ok_or("stdAttr localized: <key> without <value>")?;
        if v.local != "value" || !v.prefix.is_empty() {
            return Err(format!(
                "stdAttr localized: expected <value>, got <{}>",
                v.local
            ));
        }
        v.claim_with_text();
        pairs.push((Lang::new(k.text.clone()), v.text.clone()));
    }
    Ok(PropertyValue::Localized(pairs))
}

// EDT optional emit helpers (omit when default).

/// MULTI-SIBLING EDT-локализация: по ОДНОМУ `<tag><key>lang</key><value>text</value></tag>`
/// на язык-пару (в порядке IR) — byte-exact для двуязычного ERP (N языков = N сиблингов),
/// вырождается в 1 сиблинг для одноязычного SSL. Пусто → пустой Vec (дефолт-омиссия).
pub(crate) fn emit_localized_keyval(tag: &str, v: &PropertyValue) -> Result<Vec<OutElement>, String> {
    let pairs = as_localized(v)?;
    let mut out = Vec::with_capacity(pairs.len());
    for (lang, text) in pairs {
        let mut c = OutElement::branch("", tag);
        c.push(OutElement::leaf("", "key", lang.as_str().to_string()));
        c.push(OutElement::leaf("", "value", text.clone()));
        out.push(c);
    }
    Ok(out)
}

fn emit_opt_text(tag: &str, v: &PropertyValue) -> Result<Option<OutElement>, String> {
    let s = as_str(v)?;
    if s.is_empty() {
        return Ok(None);
    }
    Ok(Some(OutElement::leaf("", tag, s.to_string())))
}

fn emit_opt_bool(tag: &str, v: &PropertyValue) -> Result<Option<OutElement>, String> {
    match v {
        PropertyValue::Bool(false) => Ok(None),
        PropertyValue::Bool(true) => Ok(Some(OutElement::leaf("", tag, "true"))),
        other => Err(format!("<{tag}> expects Bool, got {:?}", other.kind())),
    }
}

fn emit_opt_enum(
    tag: &str,
    v: &PropertyValue,
    default: &str,
) -> Result<Option<OutElement>, String> {
    match v {
        PropertyValue::Enum(t) if t.as_str() == default => Ok(None),
        PropertyValue::Enum(t) => Ok(Some(OutElement::leaf("", tag, t.as_str().to_string()))),
        other => Err(format!("<{tag}> expects Enum, got {:?}", other.kind())),
    }
}

fn emit_opt_cpl(tag: &str, v: &PropertyValue) -> Result<Vec<OutElement>, String> {
    match v {
        PropertyValue::List(l) if l.is_empty() => Ok(Vec::new()),
        // MULTI-SIBLING: по `<choiceParameterLinks>` на связь (ERP-witnessed 2 связи;
        // одно-связочный SSL вырождается в прежний одиночный сиблинг — byte-exact).
        PropertyValue::List(_) => {
            let _ = tag;
            choice_param_links::emit_edt_from_root(v)
        }
        other => Err(format!("<{tag}> expects List, got {:?}", other.kind())),
    }
}
