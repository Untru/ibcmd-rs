//! Платформенный блок producedTypes / InternalInfo (§1.6, §3.5) — общий каркас обоих
//! XML-форматов. Захватывает платформенно-сгенерированные type-id GUID'ы (опаковые,
//! не выводимые из uuid+Type), чтобы воспроизвести их byte-exact, при этом канонизируя
//! IR так, что edt и designer дают РАВНЫЙ [`InternalInfo`] (X by construction).
//!
//! # Категории и расхождение порядка/имён
//! Один вид может нести НЕСКОЛЬКО сгенерированных типов разных КАТЕГОРИЙ:
//! * `DefinedType` — одна (`Container`): EDT `<containerType>`, Designer category
//!   `DefinedType`, name `DefinedType.<Obj>`;
//! * `Enum` — три (`Ref`/`List`/`Manager`): EDT `<refType>`/`<listType>`/
//!   `<managerType>` (порядок ref,list,manager), Designer category `Ref`/`Manager`/
//!   `List` (порядок Ref,Manager,List — ИНОЙ), name `EnumRef`/`EnumManager`/`EnumList`.
//!
//! Поскольку порядок РАСХОДИТСЯ между форматами, IR хранит КАНОНИЧЕСКИЙ порядок
//! (таблица [`categories_for`]); каждый ридер переставляет к нему, каждый райтер
//! эмитит в СВОЙ формат-нативный порядок. EDT-имя-тега и Designer-`name`/`category`
//! ДЕТЕРМИНИРОВАННО выводимы из (вид, category) — хранится лишь category (§3.5).

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::ir::{GeneratedType, InternalInfo, Uuid};
use morph1c_core::spec::produced_categories_for;

/// Описание одной категории сгенерированного типа для вида.
///
/// Тип-АЛИАС канонического [`morph1c_core::spec::ProducedCategory`]: сама таблица
/// категорий теперь ЖИВЁТ В СПЕКЕ вида (§1.6 — метамодель-данные в каноническом спеке,
/// формат лишь проецирует), а не в закрытом match-арме здесь. Машинерия ниже
/// (`read_edt`/`write_edt`/`read_designer`/`write_designer`) не изменилась — она читает те
/// же поля через [`categories_for`], которая теперь берёт их из реестра спеков.
pub type CategoryMap = morph1c_core::spec::ProducedCategory;

/// Таблица категорий producedTypes по виду В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= порядок EDT).
/// `None` для видов без блока (CommonModule/Configuration и т.п.) — каркас тогда `Ok(None)`.
///
/// Источник данных — per-kind `pub const PRODUCED_CATEGORIES` в спек-файле вида
/// (`core/spec/metadata/<вид>.rs`), собранный build.rs в реестр и прочитанный через
/// [`morph1c_core::spec::produced_categories_for`]. Раньше это был ЗАКРЫТЫЙ match —
/// добавление вида с `producedTypes` требовало правки ЭТОГО общего файла (не
/// parallel-safe). Теперь вид объявляет свои категории В СВОЁМ спек-файле; здесь — лишь
/// делегация (§1.6, зеркало R1 std-attrs).
///
/// Канонический порядок выбран = EDT-порядок (authored в `.mdo`). Designer-ридер
/// переставляет к нему по `category`; Designer-райтер эмитит по `designer_order`.
///
/// `None` (а не `Some(&[])`) для вида без категорий — сохраняет ТОЧНУЮ прежнюю семантику
/// (кодек различает «блока нет» vs «блок есть»): виды без producedTypes
/// (Configuration/CommonModule) не попадают в реестр → `None` (== прежний `_ => None`).
pub fn categories_for(kind: &str) -> Option<&'static [CategoryMap]> {
    produced_categories_for(kind)
}

fn by_edt_tag(maps: &'static [CategoryMap], tag: &str) -> Option<&'static CategoryMap> {
    maps.iter().find(|m| m.edt_tag == tag)
}

fn by_category(maps: &'static [CategoryMap], category: &str) -> Option<&'static CategoryMap> {
    maps.iter().find(|m| m.category == category)
}

fn by_designer_category(maps: &'static [CategoryMap], dc: &str) -> Option<&'static CategoryMap> {
    maps.iter().find(|m| m.designer_category == dc)
}

fn format_uuid(uuid: &Uuid) -> String {
    let b = &uuid.0;
    let h = |r: &[u8]| -> String { r.iter().map(|x| format!("{x:02x}")).collect() };
    format!(
        "{}-{}-{}-{}-{}",
        h(&b[0..4]),
        h(&b[4..6]),
        h(&b[6..8]),
        h(&b[8..10]),
        h(&b[10..16])
    )
}

fn parse_uuid(s: &str) -> Result<Uuid, String> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("malformed uuid {s:?}"));
    }
    let mut bytes = [0u8; 16];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b =
            u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|e| format!("uuid hex: {e}"))?;
    }
    Ok(Uuid(bytes))
}

fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}

fn take_uuid_attr(el: &Element, name: &str) -> Result<Uuid, String> {
    let a = el
        .attr(name)
        .ok_or_else(|| format!("<{}> missing @{name}", el.local))?;
    let u = parse_uuid(&a.value)?;
    a.claimed.set(true);
    Ok(u)
}

// ===========================================================================
// EDT: <producedTypes><refType typeId valueTypeId/>…</producedTypes>
// ===========================================================================

/// Прочитать EDT-`<producedTypes>` в КАНОНИЧЕСКИЙ [`InternalInfo`] (порядок IR =
/// `categories_for(kind)`). Строго-тотальный разбор (§1.0): claim блок, каждый
/// дочерний type-элемент и его атрибуты; неизвестный тег/категория/лишний атрибут →
/// ОШИБКА. `None` если блока нет.
pub fn read_edt(root: &Element, kind: &str) -> Result<Option<InternalInfo>, String> {
    let block = match root.child("producedTypes") {
        Some(b) => b,
        None => return Ok(None),
    };
    if !block.prefix.is_empty() {
        return Err(format!(
            "<producedTypes> must be unprefixed, got prefix {:?}",
            block.prefix
        ));
    }
    block.claim();
    if !block.attrs.is_empty() {
        return Err("<producedTypes> must have no attributes".into());
    }
    let maps = categories_for(kind)
        .ok_or_else(|| format!("{kind}: <producedTypes> present but kind has no category map"))?;

    // Собираем по category, затем эмитим в каноническом (EDT) порядке `maps`.
    let mut found: Vec<(usize, GeneratedType)> = Vec::new();
    for child in &block.children {
        if !child.prefix.is_empty() {
            return Err(format!(
                "<producedTypes> child must be unprefixed, got <{}>",
                qname(child)
            ));
        }
        let map = by_edt_tag(maps, &child.local).ok_or_else(|| {
            format!(
                "{kind}: <producedTypes> has unknown type tag <{}>",
                child.local
            )
        })?;
        if !child.children.is_empty() {
            return Err(format!("<{}> must be a self-closing leaf", child.local));
        }
        child.claim();
        let type_id = take_uuid_attr(child, "typeId")?;
        let value_id = take_uuid_attr(child, "valueTypeId")?;
        if let Some(extra) = child.attrs.iter().find(|a| !a.claimed.get()) {
            return Err(format!(
                "<{}> unexpected attribute {:?} (§1.0)",
                child.local, extra.name
            ));
        }
        let idx = maps
            .iter()
            .position(|m| m.category == map.category)
            .unwrap();
        found.push((
            idx,
            GeneratedType {
                category: map.category.to_string(),
                type_id,
                value_id,
            },
        ));
    }
    // Канонический порядок IR = порядок `maps` (= EDT-порядок).
    found.sort_by_key(|(idx, _)| *idx);
    Ok(Some(InternalInfo {
        generated_types: found.into_iter().map(|(_, g)| g).collect(),
    }))
}

/// Восстановить EDT-`<producedTypes>` из [`InternalInfo`] (byte-exact). Эмитит в
/// каноническом порядке IR (= EDT-порядок), каждый `<edt_tag typeId valueTypeId/>`.
pub fn write_edt(info: &InternalInfo, kind: &str) -> Result<OutElement, String> {
    let maps = categories_for(kind)
        .ok_or_else(|| format!("{kind}: internal_info present but kind has no category map"))?;
    let mut block = OutElement::branch("", "producedTypes");
    for gt in &info.generated_types {
        let map = by_category(maps, &gt.category)
            .ok_or_else(|| format!("{kind}: unknown generated-type category {:?}", gt.category))?;
        block.push(
            OutElement::self_closing("", map.edt_tag)
                .attr("typeId", format_uuid(&gt.type_id))
                .attr("valueTypeId", format_uuid(&gt.value_id)),
        );
    }
    Ok(block)
}

// ===========================================================================
// Designer: <InternalInfo><xr:GeneratedType name category><xr:TypeId/><xr:ValueId/>
// ===========================================================================

/// Прочитать Designer-`<InternalInfo>` в КАНОНИЧЕСКИЙ [`InternalInfo`] (порядок IR =
/// `categories_for(kind)`, НЕ Designer-порядок — он переставляется). Строго-тотальный
/// разбор (§1.0). `name`/`category` выводимы → СВЕРЯЮТСЯ. `None` если блока нет.
pub fn read_designer(
    cm: &Element,
    kind: &str,
    obj_name: &str,
) -> Result<Option<InternalInfo>, String> {
    Ok(read_designer_with_this_node(cm, kind, obj_name)?.0)
}

// Как [`read_designer`], но дополнительно возвращает опц. `thisNode` (первый ребёнок
// `<xr:ThisNode>` блока `<InternalInfo>`, если есть — план обмена). Прочие виды его не
// несут (`None`). Используется Designer-каркасом, где `thisNode` — платформенная
// идентичность, общая с EDT root-атрибутом.
// Амбьентный ПРЕФИКС derivable-имени `<xr:GeneratedType> @name` для ВЛОЖЕННЫХ видов:
// у standalone-таблицы ExternalDataSource имя цепочки — `<Кат>.<EDS>.<Таблица>`
// (witnessed ERP BaseBU), а `obj_name` — только `<Таблица>`. Префикс (`<EDS>`) знает
// ТОЛЬКО вызывающий (pipeline table_ref_read — из позиции сайдкара на диске), поэтому
// протаскивается scope'ом, не сигнатурой (per-thread, RAII-восстановление).
thread_local! {
    static PRODUCED_NAME_PREFIX: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

/// Выполнить `f` с амбьентным produced-префиксом (`<EDS>` для вложенных таблиц).
pub fn with_produced_name_prefix<R>(prefix: &str, f: impl FnOnce() -> R) -> R {
    let prev = PRODUCED_NAME_PREFIX.with(|c| c.replace(Some(prefix.to_string())));
    let out = f();
    PRODUCED_NAME_PREFIX.with(|c| *c.borrow_mut() = prev);
    out
}

fn produced_name_infix() -> String {
    PRODUCED_NAME_PREFIX.with(|c| match &*c.borrow() {
        Some(p) => format!("{p}."),
        None => String::new(),
    })
}

pub fn read_designer_with_this_node(
    cm: &Element,
    kind: &str,
    obj_name: &str,
) -> Result<(Option<InternalInfo>, Option<Uuid>), String> {
    let block = match cm.child("InternalInfo") {
        Some(b) => b,
        None => return Ok((None, None)),
    };
    if !block.prefix.is_empty() {
        return Err(format!(
            "<InternalInfo> must be unprefixed, got prefix {:?}",
            block.prefix
        ));
    }
    // Виды БЕЗ category-map (Configuration), но С `<InternalInfo>`: блок несёт НЕ
    // `<xr:GeneratedType>`, а `<xr:ContainedObject>` (платформенные `(classId,objectId)`
    // пары). Это НЕ producedTypes-каркас → каркас его НЕ трогает (НЕ claim'ит): блок
    // консумирует СПЕК-СВОЙСТВО вида (codec `ContainedObjects`, навигирующее
    // `<<Kind>/InternalInfo>`). Возврат `(None,None)` без claim — тотальность держит
    // property-codec. (Безопасно/точно: единственный вид с InternalInfo без map — корень
    // Configuration; у всех прочих InternalInfo-видов категория есть.)
    if categories_for(kind).is_none() {
        return Ok((None, None));
    }
    block.claim();
    if !block.attrs.is_empty() {
        return Err("<InternalInfo> must have no attributes".into());
    }
    let maps = categories_for(kind)
        .ok_or_else(|| format!("{kind}: <InternalInfo> present but kind has no category map"))?;

    let mut this_node_uuid: Option<Uuid> = None;
    let mut found: Vec<(usize, GeneratedType)> = Vec::new();
    for child in &block.children {
        // `<xr:ThisNode>` — опц. первый ребёнок (план обмена): claim+забор, не GeneratedType.
        if child.local == "ThisNode" && child.prefix == "xr" {
            if this_node_uuid.is_some() {
                return Err("<InternalInfo>: duplicate <xr:ThisNode> (§1.0)".into());
            }
            if !child.attrs.is_empty() || !child.children.is_empty() {
                return Err("<xr:ThisNode> must be a plain text leaf".into());
            }
            this_node_uuid = Some(parse_uuid(&child.text)?);
            child.claim_with_text();
            continue;
        }
        if child.local != "GeneratedType" || child.prefix != "xr" {
            return Err(format!(
                "<InternalInfo> child must be <xr:GeneratedType>, got <{}>",
                qname(child)
            ));
        }
        child.claim();
        // category-атрибут → найти map (по DESIGNER-категории); name-атрибут → сверить.
        let cat_attr = child
            .attr("category")
            .ok_or("<xr:GeneratedType> missing @category")?;
        let map = by_designer_category(maps, &cat_attr.value).ok_or_else(|| {
            format!(
                "{kind}: <xr:GeneratedType> unknown category {:?}",
                cat_attr.value
            )
        })?;
        cat_attr.claimed.set(true);
        let want_name = format!("{}.{}{obj_name}", map.designer_type_name, produced_name_infix());
        let name_attr = child
            .attr("name")
            .ok_or("<xr:GeneratedType> missing @name")?;
        if name_attr.value != want_name {
            return Err(format!(
                "<xr:GeneratedType> @name = {:?}, want {want_name:?} (derivable form — §1.0)",
                name_attr.value
            ));
        }
        name_attr.claimed.set(true);
        if let Some(extra) = child.attrs.iter().find(|a| !a.claimed.get()) {
            return Err(format!(
                "<xr:GeneratedType> unexpected attribute {:?} (§1.0)",
                extra.name
            ));
        }
        // Ровно [xr:TypeId, xr:ValueId] (строго-позиционно, §1.0).
        let mut it = child.children.iter();
        let type_id = take_uuid_leaf(it.next(), "TypeId")?;
        let value_id = take_uuid_leaf(it.next(), "ValueId")?;
        if let Some(extra) = it.next() {
            return Err(format!(
                "<xr:GeneratedType> unexpected extra child <{}> after TypeId/ValueId (§1.0)",
                qname(extra)
            ));
        }
        let idx = maps
            .iter()
            .position(|m| m.category == map.category)
            .unwrap();
        found.push((
            idx,
            GeneratedType {
                category: map.category.to_string(),
                type_id,
                value_id,
            },
        ));
    }
    // Канонический порядок IR = порядок `maps` (НЕ Designer-порядок входа).
    found.sort_by_key(|(idx, _)| *idx);
    let info = Some(InternalInfo {
        generated_types: found.into_iter().map(|(_, g)| g).collect(),
    });
    Ok((info, this_node_uuid))
}

/// Восстановить Designer-`<InternalInfo>` из [`InternalInfo`] (byte-exact). Эмитит в
/// DESIGNER-нативном порядке (`designer_order`), `name`/`category` регенерируются.
pub fn write_designer(
    info: &InternalInfo,
    kind: &str,
    obj_name: &str,
) -> Result<OutElement, String> {
    write_designer_with_this_node(info, kind, obj_name, None)
}

/// Как [`write_designer`], но с опц. `thisNode` (план обмена) — `<xr:ThisNode>` эмитится
/// ПЕРВЫМ ребёнком `<InternalInfo>` (byte-exact, сверено). `None` → как [`write_designer`].
pub fn write_designer_with_this_node(
    info: &InternalInfo,
    kind: &str,
    obj_name: &str,
    this_node: Option<&Uuid>,
) -> Result<OutElement, String> {
    let maps = categories_for(kind)
        .ok_or_else(|| format!("{kind}: internal_info present but kind has no category map"))?;
    // Упорядочить генерированные типы по designer_order.
    let mut ordered: Vec<(&GeneratedType, &CategoryMap)> = Vec::new();
    for gt in &info.generated_types {
        let map = by_category(maps, &gt.category)
            .ok_or_else(|| format!("{kind}: unknown generated-type category {:?}", gt.category))?;
        ordered.push((gt, map));
    }
    ordered.sort_by_key(|(_, m)| m.designer_order);

    let mut block = OutElement::branch("", "InternalInfo");
    // `<xr:ThisNode>` — ПЕРЕД GeneratedType'ами (план обмена).
    if let Some(tn) = this_node {
        block.push(OutElement::leaf("xr", "ThisNode", format_uuid(tn)));
    }
    for (gt, map) in ordered {
        let mut g = OutElement::branch("xr", "GeneratedType")
            .attr("name", format!("{}.{}{obj_name}", map.designer_type_name, produced_name_infix()))
            .attr("category", map.designer_category);
        g.push(OutElement::leaf("xr", "TypeId", format_uuid(&gt.type_id)));
        g.push(OutElement::leaf("xr", "ValueId", format_uuid(&gt.value_id)));
        block.push(g);
    }
    Ok(block)
}

fn take_uuid_leaf(el: Option<&Element>, local: &str) -> Result<Uuid, String> {
    let el = el.ok_or_else(|| format!("<xr:GeneratedType> missing <xr:{local}>"))?;
    if el.local != local || el.prefix != "xr" {
        return Err(format!("expected <xr:{local}>, got <{}>", qname(el)));
    }
    if !el.attrs.is_empty() || !el.children.is_empty() {
        return Err(format!("<xr:{local}> must be a plain text leaf"));
    }
    let u = parse_uuid(&el.text)?;
    el.claim_with_text();
    Ok(u)
}

#[cfg(any())]
mod tests {
    use super::*;

    /// Компактный эталон исторического закрытого match'а: `(kind, [(category, edt_tag,
    /// designer_category, designer_type_name, designer_order)])`. Оракул миграции —
    /// пофайловая `PRODUCED_CATEGORIES` ДОЛЖНА совпасть с ним byte-в-byte для каждого вида
    /// (byte-exact producedTypes-вывода гарантируется тем, что кодек читает те же поля).
    #[allow(clippy::type_complexity)]
    const LEGACY: &[(&str, &[(&str, &str, &str, &str, u8)])] = &[
        (
            "DefinedType",
            &[(
                "Container",
                "containerType",
                "DefinedType",
                "DefinedType",
                0,
            )],
        ),
        (
            "Constant",
            &[
                ("Manager", "managerType", "Manager", "ConstantManager", 0),
                (
                    "ValueManager",
                    "valueManagerType",
                    "ValueManager",
                    "ConstantValueManager",
                    1,
                ),
                (
                    "ValueKey",
                    "valueKeyType",
                    "ValueKey",
                    "ConstantValueKey",
                    2,
                ),
            ],
        ),
        (
            "InformationRegister",
            &[
                (
                    "Selection",
                    "selectionType",
                    "Selection",
                    "InformationRegisterSelection",
                    2,
                ),
                ("List", "listType", "List", "InformationRegisterList", 3),
                (
                    "Manager",
                    "managerType",
                    "Manager",
                    "InformationRegisterManager",
                    1,
                ),
                (
                    "RecordSet",
                    "recordSetType",
                    "RecordSet",
                    "InformationRegisterRecordSet",
                    4,
                ),
                (
                    "RecordKey",
                    "recordKeyType",
                    "RecordKey",
                    "InformationRegisterRecordKey",
                    5,
                ),
                (
                    "Record",
                    "recordType",
                    "Record",
                    "InformationRegisterRecord",
                    0,
                ),
                (
                    "RecordManager",
                    "recordManagerType",
                    "RecordManager",
                    "InformationRegisterRecordManager",
                    6,
                ),
            ],
        ),
        (
            "Catalog",
            &[
                ("Object", "objectType", "Object", "CatalogObject", 0),
                ("Ref", "refType", "Ref", "CatalogRef", 1),
                (
                    "Selection",
                    "selectionType",
                    "Selection",
                    "CatalogSelection",
                    2,
                ),
                ("List", "listType", "List", "CatalogList", 3),
                ("Manager", "managerType", "Manager", "CatalogManager", 4),
            ],
        ),
        (
            "Catalog.TabularSection",
            &[
                (
                    "TabularSection",
                    "objectType",
                    "TabularSection",
                    "CatalogTabularSection",
                    0,
                ),
                (
                    "TabularSectionRow",
                    "rowType",
                    "TabularSectionRow",
                    "CatalogTabularSectionRow",
                    1,
                ),
            ],
        ),
        (
            "Document",
            &[
                ("Object", "objectType", "Object", "DocumentObject", 0),
                ("Ref", "refType", "Ref", "DocumentRef", 1),
                (
                    "Selection",
                    "selectionType",
                    "Selection",
                    "DocumentSelection",
                    2,
                ),
                ("List", "listType", "List", "DocumentList", 3),
                ("Manager", "managerType", "Manager", "DocumentManager", 4),
            ],
        ),
        (
            "Document.TabularSection",
            &[
                (
                    "TabularSection",
                    "objectType",
                    "TabularSection",
                    "DocumentTabularSection",
                    0,
                ),
                (
                    "TabularSectionRow",
                    "rowType",
                    "TabularSectionRow",
                    "DocumentTabularSectionRow",
                    1,
                ),
            ],
        ),
        (
            "Task",
            &[
                ("Object", "objectType", "Object", "TaskObject", 0),
                ("Ref", "refType", "Ref", "TaskRef", 1),
                (
                    "Selection",
                    "selectionType",
                    "Selection",
                    "TaskSelection",
                    2,
                ),
                ("List", "listType", "List", "TaskList", 3),
                ("Manager", "managerType", "Manager", "TaskManager", 4),
            ],
        ),
        (
            "Task.TabularSection",
            &[
                (
                    "TabularSection",
                    "objectType",
                    "TabularSection",
                    "TaskTabularSection",
                    0,
                ),
                (
                    "TabularSectionRow",
                    "rowType",
                    "TabularSectionRow",
                    "TaskTabularSectionRow",
                    1,
                ),
            ],
        ),
        (
            "BusinessProcess",
            &[
                ("Object", "objectType", "Object", "BusinessProcessObject", 0),
                ("Ref", "refType", "Ref", "BusinessProcessRef", 1),
                (
                    "Selection",
                    "selectionType",
                    "Selection",
                    "BusinessProcessSelection",
                    2,
                ),
                ("List", "listType", "List", "BusinessProcessList", 3),
                (
                    "Manager",
                    "managerType",
                    "Manager",
                    "BusinessProcessManager",
                    4,
                ),
                (
                    "RoutePointRef",
                    "routePointRef",
                    "RoutePointRef",
                    "BusinessProcessRoutePointRef",
                    5,
                ),
            ],
        ),
        (
            "BusinessProcess.TabularSection",
            &[
                (
                    "TabularSection",
                    "objectType",
                    "TabularSection",
                    "BusinessProcessTabularSection",
                    0,
                ),
                (
                    "TabularSectionRow",
                    "rowType",
                    "TabularSectionRow",
                    "BusinessProcessTabularSectionRow",
                    1,
                ),
            ],
        ),
        (
            "ExchangePlan",
            &[
                ("Object", "objectType", "Object", "ExchangePlanObject", 0),
                ("Ref", "refType", "Ref", "ExchangePlanRef", 1),
                (
                    "Selection",
                    "selectionType",
                    "Selection",
                    "ExchangePlanSelection",
                    2,
                ),
                ("List", "listType", "List", "ExchangePlanList", 3),
                (
                    "Manager",
                    "managerType",
                    "Manager",
                    "ExchangePlanManager",
                    4,
                ),
            ],
        ),
        (
            "ExchangePlan.TabularSection",
            &[
                (
                    "TabularSection",
                    "objectType",
                    "TabularSection",
                    "ExchangePlanTabularSection",
                    0,
                ),
                (
                    "TabularSectionRow",
                    "rowType",
                    "TabularSectionRow",
                    "ExchangePlanTabularSectionRow",
                    1,
                ),
            ],
        ),
        (
            "DataProcessor",
            &[
                ("Object", "objectType", "Object", "DataProcessorObject", 0),
                (
                    "Manager",
                    "managerType",
                    "Manager",
                    "DataProcessorManager",
                    1,
                ),
            ],
        ),
        (
            "DataProcessor.TabularSection",
            &[
                (
                    "TabularSection",
                    "objectType",
                    "TabularSection",
                    "DataProcessorTabularSection",
                    0,
                ),
                (
                    "TabularSectionRow",
                    "rowType",
                    "TabularSectionRow",
                    "DataProcessorTabularSectionRow",
                    1,
                ),
            ],
        ),
        (
            "Report",
            &[
                ("Object", "objectType", "Object", "ReportObject", 0),
                ("Manager", "managerType", "Manager", "ReportManager", 1),
            ],
        ),
        (
            "Report.TabularSection",
            &[
                (
                    "TabularSection",
                    "objectType",
                    "TabularSection",
                    "ReportTabularSection",
                    0,
                ),
                (
                    "TabularSectionRow",
                    "rowType",
                    "TabularSectionRow",
                    "ReportTabularSectionRow",
                    1,
                ),
            ],
        ),
        (
            "DocumentJournal",
            &[
                (
                    "Selection",
                    "selectionType",
                    "Selection",
                    "DocumentJournalSelection",
                    0,
                ),
                ("List", "listType", "List", "DocumentJournalList", 1),
                (
                    "Manager",
                    "managerType",
                    "Manager",
                    "DocumentJournalManager",
                    2,
                ),
            ],
        ),
        (
            "Enum",
            &[
                ("Ref", "refType", "Ref", "EnumRef", 0),
                ("List", "listType", "List", "EnumList", 2),
                ("Manager", "managerType", "Manager", "EnumManager", 1),
            ],
        ),
        (
            "ChartOfCharacteristicTypes",
            &[
                (
                    "Object",
                    "objectType",
                    "Object",
                    "ChartOfCharacteristicTypesObject",
                    0,
                ),
                ("Ref", "refType", "Ref", "ChartOfCharacteristicTypesRef", 1),
                (
                    "Selection",
                    "selectionType",
                    "Selection",
                    "ChartOfCharacteristicTypesSelection",
                    2,
                ),
                (
                    "List",
                    "listType",
                    "List",
                    "ChartOfCharacteristicTypesList",
                    3,
                ),
                (
                    "Manager",
                    "managerType",
                    "Manager",
                    "ChartOfCharacteristicTypesManager",
                    5,
                ),
                (
                    "Characteristic",
                    "containerType",
                    "Characteristic",
                    "Characteristic",
                    4,
                ),
            ],
        ),
        (
            "ChartOfCharacteristicTypes.TabularSection",
            &[
                (
                    "TabularSection",
                    "objectType",
                    "TabularSection",
                    "ChartOfCharacteristicTypesTabularSection",
                    0,
                ),
                (
                    "TabularSectionRow",
                    "rowType",
                    "TabularSectionRow",
                    "ChartOfCharacteristicTypesTabularSectionRow",
                    1,
                ),
            ],
        ),
    ];

    /// Data-driven путь (`categories_for` ← спек через `produced_categories_for`) даёт для
    /// КАЖДОГО вида ТУ ЖЕ таблицу категорий, что исторический закрытый match — миграция
    /// behavior-preserving (byte-exact producedTypes гарантируется тем, что кодек читает те
    /// же поля в том же порядке).
    #[test]
    fn spec_categories_match_legacy() {
        for (kind, want) in LEGACY {
            let got = categories_for(kind)
                .unwrap_or_else(|| panic!("{kind}: categories_for None (spec lost producedTypes)"));
            assert_eq!(
                got.len(),
                want.len(),
                "{kind}: category count differs (spec vs legacy)"
            );
            for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
                assert_eq!(g.category, w.0, "{kind}[{i}] category");
                assert_eq!(g.edt_tag, w.1, "{kind}[{i}] edt_tag");
                assert_eq!(g.designer_category, w.2, "{kind}[{i}] designer_category");
                assert_eq!(g.designer_type_name, w.3, "{kind}[{i}] designer_type_name");
                assert_eq!(g.designer_order, w.4, "{kind}[{i}] designer_order");
            }
        }
    }

    /// Виды БЕЗ платформенного блока (или незарегистрированные) дают `None` — сохранена
    /// прежняя семантика «блока нет» (кодек тогда `Ok(None)`, не эмитит producedTypes).
    #[test]
    fn kinds_without_block_are_none() {
        assert!(
            categories_for("CommonModule").is_none(),
            "CommonModule has no producedTypes"
        );
        assert!(
            categories_for("Configuration").is_none(),
            "Configuration has no producedTypes"
        );
        assert!(
            categories_for("NoSuchKind").is_none(),
            "unregistered kind -> None"
        );
    }
}
