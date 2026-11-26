//! READ · shared low-level helpers: generic Element/leaf/type accessors, presence flags
//! and validators used by BOTH dialects (no dialect-specific tag/ns knowledge here).

use super::*;

pub(crate) fn read_handlers(h: &Element) -> Result<FormEvent, FormError> {
    h.claim();
    let event = leaf_text(h, "event")?;
    let name = leaf_text(h, "name")?;
    expect_only_children(h, &["event", "name"])?;
    if event.is_empty() || name.is_empty() {
        return Err(FormError::Frame("event identity and handler must be nonempty".into()));
    }
    Ok(FormEvent {
        name: event,
        handler: name,
    })
}

/// presence-true bool (формат-агностичный): `<tag>true</tag>` ⇒ `true`; отсутствие ⇒ `false`;
/// иное значение — §1.0-ошибка (в корпусе эти флаги эмитятся лишь как `true`).
pub(crate) fn read_presence_true(el: &Element, tag: &str) -> Result<bool, FormError> {
    match el.child(tag).filter(|c| c.prefix.is_empty()) {
        Some(v) => {
            v.claim_with_text();
            if v.text != "true" {
                return Err(FormError::Frame(format!(
                    "<{tag}>={:?}, want true (§1.0)",
                    v.text
                )));
            }
            Ok(true)
        }
        None => Ok(false),
    }
}

// ============================== shared helpers ==============================

/// Прочитать `<valueType>`/`<Type>`: present-empty (`<tag/>`) → `None`; иначе Type-codec.
pub(crate) fn read_value_type(
    parent: &Element,
    tag: &str,
    dialect: crate::TypeDialect,
) -> Result<Option<TypeSpec>, FormError> {
    let host = match parent.child(tag).filter(|c| c.prefix.is_empty()) {
        Some(h) => h,
        None => return Err(FormError::Frame(format!("missing <{tag}>"))),
    };
    decode_type_host(host, dialect)
}

/// Декодировать host-элемент типа (`<valueType>`/`<Type>`/`<itemValueType>`/Settings-
/// TypeDescription) через общий type-codec → `Option<TypeSpec>` (пустой набор ⇒ present-empty
/// `None`). Клеймит host и его type-детей (§1.0-тотальность: незнакомый ребёнок ⇒ ошибка).
/// Атрибуты host'а (напр. Designer `xsi:type` у `<Settings>`) НЕ трогает — их клеймит вызыватель.
pub(crate) fn decode_type_host(
    host: &Element,
    dialect: crate::TypeDialect,
) -> Result<Option<TypeSpec>, FormError> {
    host.claim();
    type_codec::claim(dialect, host);
    match type_codec::decode(dialect, host).map_err(FormError::Frame)? {
        PropertyValue::Type(ts) if ts.parts.is_empty() => Ok(None),
        PropertyValue::Type(ts) => Ok(Some(ts)),
        other => Err(FormError::Frame(format!(
            "type host: not a Type: {other:?}"
        ))),
    }
}

/// Прочитать `<tag><common>true</common></tag>` (claim) — EDT presence-денормализация.
pub(crate) fn read_common_true(el: &Element, tag: &str) -> Result<(), FormError> {
    el.claim();
    let c = el
        .child("common")
        .ok_or_else(|| FormError::Frame(format!("<{tag}>: no <common>")))?;
    c.claim_with_text();
    if c.text != "true" {
        return Err(FormError::Frame(format!(
            "<{tag}><common>={:?}, want true",
            c.text
        )));
    }
    expect_only_children(el, &["common"])?;
    Ok(())
}

pub(crate) fn read_bool_text(el: &Element, tag: &str) -> Result<PropertyValue, FormError> {
    el.claim_with_text();
    match el.text.as_str() {
        "true" => Ok(PropertyValue::Bool(true)),
        "false" => Ok(PropertyValue::Bool(false)),
        other => Err(FormError::Frame(format!("<{tag}>={other:?}, want bool"))),
    }
}

/// Текст обязательного листа-ребёнка (claim).
pub(crate) fn leaf_text(parent: &Element, tag: &str) -> Result<String, FormError> {
    let el = parent
        .child(tag)
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame(format!("missing <{tag}>")))?;
    el.claim_with_text();
    Ok(el.text.clone())
}

/// Текст ОПЦИОНАЛЬНОГО листа-ребёнка (claim, если есть). `None` ⇒ тег отсутствует.
pub(crate) fn leaf_text_opt(parent: &Element, tag: &str) -> Option<String> {
    parent.child(tag).filter(|c| c.prefix.is_empty()).map(|el| {
        el.claim_with_text();
        el.text.clone()
    })
}

/// Текст ВСЕХ одноимённых листьев-детей (ПОВТОРЯЕМЫЙ элемент; claim каждого). Пусто ⇒ нет тега.
/// В отличие от [`leaf_text_opt`] клеймит КАЖДОЕ вхождение — иначе второе и далее остаётся
/// несконсуменным (§1.0), что и есть класс `keyField`-падений.
pub(crate) fn leaf_texts_all(parent: &Element, tag: &str) -> Vec<String> {
    parent
        .children
        .iter()
        .filter(|c| c.local == tag && c.prefix.is_empty())
        .map(|el| {
            el.claim_with_text();
            el.text.clone()
        })
        .collect()
}

/// Явный булев лист-ребёнок (текст `true`/`false`), ОБЯЗАТЕЛЬНО присутствующий (claim). В отличие
/// от [`read_presence_true`] требует явного значения (Designer эмитит `false` явно, не опускает).
pub(crate) fn read_bool_leaf(parent: &Element, tag: &str) -> Result<bool, FormError> {
    let el = parent
        .child(tag)
        .filter(|c| c.prefix.is_empty())
        .ok_or_else(|| FormError::Frame(format!("missing <{tag}>")))?;
    match read_bool_text(el, tag)? {
        PropertyValue::Bool(b) => Ok(b),
        _ => unreachable!("read_bool_text returns Bool"),
    }
}

pub(crate) fn leaf_int(parent: &Element, tag: &str) -> Result<i64, FormError> {
    parse_int(&leaf_text(parent, tag)?)
}

/// EDT `<id>` контрола/реквизита/таблицы: ОПЦИОНАЛЕН — EDT ОПУСКАЕТ дефолт `0` (self-серия
/// «опустить целочисленный дефолт»), Designer эмитит `id="0"` атрибутом всегда. Отсутствие ⇒ `0`
/// (канон id=0 ⟺ Designer `id="0"`). ERP-witness: Catalog.ДоговорыКонтрагентов.ФормаЭлемента
/// реквизит ПоддержкаЭлектронногоАктированияВЕИС (`<attributes>` без `<id>`) и
/// Report.мирМониторингЗаказа.ФормаОтчета главная `<autoTable>` (без `<id>`) — оба Designer `id="0"`.
pub(crate) fn read_edt_id(parent: &Element) -> Result<i64, FormError> {
    match parent.child("id").filter(|c| c.prefix.is_empty()) {
        Some(el) => {
            el.claim_with_text();
            parse_int(&el.text)
        }
        None => Ok(0),
    }
}

pub(crate) fn expect_leaf_value(parent: &Element, tag: &str, want: &str) -> Result<(), FormError> {
    let v = leaf_text(parent, tag)?;
    if v != want {
        return Err(FormError::Frame(format!("<{tag}>={v:?}, want {want:?}")));
    }
    Ok(())
}

pub(crate) fn attr_value(el: &Element, name: &str) -> Result<String, FormError> {
    let a = el
        .attr(name)
        .ok_or_else(|| FormError::Frame(format!("missing @{name}")))?;
    a.claimed.set(true);
    Ok(a.value.clone())
}

pub(crate) fn parse_int(s: &str) -> Result<i64, FormError> {
    s.parse::<i64>()
        .map_err(|e| FormError::Frame(format!("bad int {s:?}: {e}")))
}

/// Сверить, что у `el` нет НЕОЖИДАННЫХ детей-элементов (кроме `allowed` local-names без
/// префикса). Чисто-структурная сверка; totality всё равно ловит неклеймнутое.
pub(crate) fn expect_only_children(el: &Element, allowed: &[&str]) -> Result<(), FormError> {
    for c in &el.children {
        if !(c.prefix.is_empty() && allowed.contains(&c.local.as_str())) {
            return Err(FormError::Frame(format!(
                "<{}>: unexpected child <{}:{}> (§1.0)",
                el.local, c.prefix, c.local
            )));
        }
    }
    Ok(())
}

/// Как [`expect_only_children`], но множество допустимых имён = `head` (фикс-каркас) ∪ `tail`
/// (имена, лениво спроецированные из табличных строк) — БЕЗ материализации объединённого
/// `Vec<&str>` на каждый читаемый контрол. Приём/отказ ПОБИТОВО идентичны конкатенации
/// `head` и `tail` в один срез: членство в объединении = членство в `head` ИЛИ в `tail`.
/// `tail` должен быть `Clone` (пере-итерируется по разу на ребёнка); табличные
/// `slice::Iter[.filter].map` — не-захватывающие, значит `Clone`.
pub(crate) fn expect_only_children_ext<'a, I>(el: &Element, head: &[&str], tail: I) -> Result<(), FormError>
where
    I: IntoIterator<Item = &'a str> + Clone,
{
    for c in &el.children {
        let name = c.local.as_str();
        let ok = c.prefix.is_empty()
            && (head.contains(&name) || tail.clone().into_iter().any(|t| t == name));
        if !ok {
            return Err(FormError::Frame(format!(
                "<{}>: unexpected child <{}:{}> (§1.0)",
                el.local, c.prefix, c.local
            )));
        }
    }
    Ok(())
}

pub(crate) fn expect_no_children(el: &Element) -> Result<(), FormError> {
    if !el.children.is_empty() {
        return Err(FormError::Frame(format!(
            "<{}>: must have no children",
            el.local
        )));
    }
    Ok(())
}

/// Отсортировать property-bag в КАНОНИЧЕСКИЙ порядок спека (X-структурность; §1.6).
pub(crate) fn sort_props_by_spec(
    bag: &mut [(morph1c_core::ir::FieldId, PropertyValue)],
    spec: &morph1c_core::spec::common::EntitySpec,
) {
    let order = |id: morph1c_core::ir::FieldId| -> usize {
        spec.fields()
            .iter()
            .position(|f| f.id == id)
            .unwrap_or(usize::MAX)
    };
    bag.sort_by_key(|(id, _)| order(*id));
}

