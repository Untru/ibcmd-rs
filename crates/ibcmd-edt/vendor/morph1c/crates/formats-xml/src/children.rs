//! Child-objects substrate — рекурсия по подчинённым коллекциям (§2.1/§3.4
//!). Реализует слой `read_object`/`write_object` ПОВЕРХ
//! плоского `engine::{read,write}`: после плоского property-bag корня движок
//! рекурсирует по `EntitySpec.children`-слотам, навигируя коллекцию через
//! [`crate::LocusMap::child_collection`] и читая/записывая КАЖДОГО ребёнка тем же
//! спек-движком (ребёнок — это [`MetadataObject`], §1.1 «одна сущность — один тип»).
//!
//! Где живёт рекурсия (выбор дизайн-доки §2.1 вариант b): в XML-субстрате, общем для
//! edt/designer. `core` несёт лишь ДАННЫЕ (`EntitySpec.children`); навигацию по
//! коллекции (имена тегов, обёртка `<ChildObjects>`, плоско-vs-Properties) — ПРОЕКЦИЯ.
//!
//! Идентичность ребёнка (`@uuid` + `<name>`/`<Name>`) фреймится ЗДЕСЬ (как у корня,
//! §2.2), а НЕ движком: uuid-форма (lower-hex `8-4-4-4-12`) едина у обоих XML-форматов
//! (сверено: `parse_uuid`/`format_uuid` коннекторов побайтово идентичны). Свойства
//! ребёнка — через `engine::read`/`write` с child-спеком и child-проекцией.
//!
//! Тотальность (§1.0/§2.3) рекурсивна: child-навигация claim'ит контейнер коллекции,
//! каждый элемент, его `@uuid` и `<name>`-лист, а `engine::read` ребёнка claim'ит его
//! свойства. Любой неразобранный под-элемент → `leftover>0` корня → ОШИБКА.
//!
//! # Recursion-узлы и тотальность над ВСЕМ элементом (фикс nit-1)
//! Ребёнок может сам нести child-коллекции (`URLTemplate` → `Method`,
//! `TabularSection` → `Attribute`). Тогда у элемента коллекции `<el>` есть СИБЛИНГИ к
//! property-региону (EDT: inline `<methods>`; Designer: `<ChildObjects>`), которых клон
//! property-региона НЕ доказывает консумируемыми. Поэтому [`read_one`] НЕ делает
//! blanket-`claim_subtree(el)` (он замаскировал бы неразобранный grandchild — нарушение
//! §1.0). Вместо этого: (1) claim каркаса идентичности; (2) property-claim-проход по
//! ОРИГИНАЛУ property-региона (а не клону) через ту же проекцию, что и движок;
//! (3) РЕКУРСИЯ в собственные child-коллекции ребёнка (каждая claim'ит свой контейнер +
//! элементы); (4) СВЕРКА `el.unclaimed_count()==0` — любой неразобранный под-узел `el`
//! (свойство, grandchild, лишний тег) → типизированная ОШИБКА. Тотальность доказывается
//! по ВСЕМУ `el`, а не по property-only клону.

use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::{LocusMap, XmlProjection, XmlSink, XmlSource};
use morph1c_core::engine::{self, EngineError};
use morph1c_core::ir::{MetadataObject, ObjectKind, Uuid};
use morph1c_core::spec::common::EntitySpec;
use morph1c_core::version::FormatVersion;

/// Привязка одного child-слота к проекции его вида: даёт child-спек и child-`LocusMap`.
/// Родительская проекция формата отдаёт `&'static [ChildBinding]` (по одному на
/// `ChildSlot` родителя), чтобы рекурсия знала, ЧЕМ читать каждого ребёнка.
pub struct ChildBinding {
    /// Формат-нейтральный код коллекции (= `ChildSlot.collection`).
    pub collection: &'static str,
    /// Канонический child-спек (через `spec_for(child_kind)` либо прямой конструктор).
    pub child_spec: &'static EntitySpec,
    /// Проекция child-вида в ЭТОМ формате (без `HARNESS_ENTRY` — покрыт транзитивно).
    /// `+ Sync`: бинды живут в `OnceLock`-кэше родителя (а проекции — ZST-юниты).
    pub child_map: &'static (dyn LocusMap + Sync),
}

/// Ошибка рекурсивного слоя (типизированная, §1.0): идентичность ребёнка / тотальность.
#[derive(Debug)]
pub enum ChildError {
    /// Структурная ошибка каркаса (нет контейнера, нет `@uuid`/`<name>`, лишний тег).
    Frame(String),
    /// Ошибка спек-движка на ребёнке (несконсуменное, mismatch вида, …).
    Engine(EngineError),
}

impl std::fmt::Display for ChildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChildError::Frame(s) => write!(f, "child-objects: {s}"),
            ChildError::Engine(e) => write!(f, "child-objects: {e}"),
        }
    }
}

impl std::error::Error for ChildError {}

impl From<EngineError> for ChildError {
    fn from(e: EngineError) -> Self {
        ChildError::Engine(e)
    }
}

/// Распарсить hex-uuid (`8-4-4-4-12`, lower-case) в 16 байт. Едина для edt/designer.
pub fn parse_uuid(s: &str) -> Result<Uuid, ChildError> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ChildError::Frame(format!("malformed child uuid {s:?}")));
    }
    let mut bytes = [0u8; 16];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|e| ChildError::Frame(format!("child uuid hex: {e}")))?;
    }
    Ok(Uuid(bytes))
}

/// Форматировать 16 байт в каноническую hex-строку uuid (как пишут оба формата).
pub fn format_uuid(uuid: &Uuid) -> String {
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

/// Найти бинд по коду коллекции (родитель отдаёт упорядоченный список).
fn binding<'a>(bindings: &'a [ChildBinding], collection: &str) -> Option<&'a ChildBinding> {
    bindings.iter().find(|b| b.collection == collection)
}

/// Спуститься по пути контейнера (для Designer — `["Enum","ChildObjects"]`), claim'я
/// каждый промежуточный узел контейнера. Пустой путь → сам корень (EDT). `None` если
/// контейнера нет (коллекция пуста и обёртки в источнике нет — валидно).
fn descend_container<'a>(root: &'a Element, path: &[&str]) -> Option<&'a Element> {
    let mut cur = root;
    for seg in path {
        cur = cur.child(seg)?;
    }
    Some(cur)
}

/// РЕКУРСИВНО прочитать всех детей корня по `parent_spec.children` (в каноническом
/// порядке слотов), вернуть упорядоченный `Vec<MetadataObject>`. Claim'ит весь
/// под-материал детей (§1.0). `parent_map` — родительская проекция (даёт child-локусы);
/// `bindings` — child-спеки/проекции по коллекции.
pub fn read_children<M: LocusMap + ?Sized>(
    root: &Element,
    parent_spec: &EntitySpec,
    parent_map: &M,
    bindings: &[ChildBinding],
    version: FormatVersion,
    emit_defaults: bool,
) -> Result<Vec<MetadataObject>, ChildError> {
    read_children_named(
        root,
        parent_spec,
        parent_map,
        bindings,
        version,
        emit_defaults,
        "",
    )
}

/// Как [`read_children`], но с ПРЕФИКСОМ имени родителя (`<Parent>` или
/// `<GrandParent>.<Parent>`) для генерации Designer type-name'ов producedTypes
/// recursion-узлов (напр. `CatalogTabularSection.<Parent>.<TS>`). Корневой вызов — `""`.
pub fn read_children_named<M: LocusMap + ?Sized>(
    root: &Element,
    parent_spec: &EntitySpec,
    parent_map: &M,
    bindings: &[ChildBinding],
    version: FormatVersion,
    emit_defaults: bool,
    name_prefix: &str,
) -> Result<Vec<MetadataObject>, ChildError> {
    let mut out = Vec::new();
    for slot in parent_spec.children() {
        let loc = parent_map
            .child_collection(slot.collection)
            .ok_or_else(|| {
                ChildError::Frame(format!(
                    "{}: projection has no child_collection for {:?}",
                    parent_spec.entity, slot.collection
                ))
            })?;
        let bind = binding(bindings, slot.collection).ok_or_else(|| {
            ChildError::Frame(format!(
                "{}: no ChildBinding for collection {:?}",
                parent_spec.entity, slot.collection
            ))
        })?;

        // Контейнер коллекции: для Designer — `<ChildObjects>` (claim), для EDT — корень.
        let container = match descend_container(root, loc.container) {
            Some(c) => c,
            None => continue, // обёртки нет ⇒ детей нет (валидно).
        };
        if !loc.container.is_empty() {
            container.claim();
        }

        // Дотированный bare-ref (см. `LocusMap::bare_ref_dotted`): ожидаемый префикс
        // ссылки = `<ParentKind>.<ParentName>.<Seg>.` (`Seg` — последний дот-сегмент
        // child-вида; `name_prefix` на корневом вызове = имя родителя).
        let dotted_prefix = parent_map.bare_ref_dotted(slot.collection).then(|| {
            let seg = bind.child_spec.entity.rsplit('.').next().unwrap_or_default();
            format!("{}.{}.{}.", parent_spec.entity, name_prefix, seg)
        });

        for el in container
            .children
            .iter()
            .filter(|c| c.local == loc.child_tag && c.prefix.is_empty())
        {
            let child = read_one(
                el,
                &loc,
                bind,
                version,
                emit_defaults,
                name_prefix,
                dotted_prefix.as_deref(),
            )?;
            out.push(child);
        }
    }
    Ok(out)
}

/// Рекурсивно прочитать собственные child-коллекции ОДНОГО ребёнка (recursion-узла).
/// Навигация — от `el` (элемент коллекции), через child-проекцию `bind.child_map` и её
/// `child_bindings()`. Лист-ребёнок (`child_spec.children()` пуст) → `Vec::new()` без
/// какой-либо навигации. Вынесено отдельной мономорфной функцией (а не через `read_children`
/// напрямую), чтобы рекурсия шла по `&dyn LocusMap` ребёнка (бинды — type-erased).
fn read_grandchildren(
    el: &Element,
    bind: &ChildBinding,
    version: FormatVersion,
    emit_defaults: bool,
    name_prefix: &str,
) -> Result<Vec<MetadataObject>, ChildError> {
    // Лист-вид: нет собственных коллекций → нечего навигировать (и `el` не несёт
    // вложенных контейнеров — это докажет totality-сверка в `read_one`).
    if bind.child_spec.children().is_empty() {
        return Ok(Vec::new());
    }
    read_children_named(
        el,
        bind.child_spec,
        bind.child_map,
        bind.child_map.child_bindings(),
        version,
        emit_defaults,
        name_prefix,
    )
}

/// Прочитать ОДИН элемент коллекции в [`MetadataObject`]: фрейм идентичности
/// (`@uuid`+`<name>`) + плоский bag через движок + РЕКУРСИЯ в собственные коллекции
/// ребёнка. Claim'ит РОВНО консумируемое и сверяет тотальность по ВСЕМУ `el` (§1.0,
/// фикс nit-1 — НЕ blanket `claim_subtree`).
fn read_one(
    el: &Element,
    loc: &crate::locus::ChildLocus,
    bind: &ChildBinding,
    version: FormatVersion,
    emit_defaults: bool,
    name_prefix: &str,
    dotted_prefix: Option<&str>,
) -> Result<MetadataObject, ChildError> {
    // BARE-REF коллекция (Designer `<Form>Имя</Form>`): элемент — лишь имя-ссылка БЕЗ
    // `@uuid`/`<Properties>`/свойств. uuid=zero, properties=[]. Тотальность: claim узла
    // + текста. EDT-сторона той же коллекции НЕ bare (полный inline-стаб) — её ветка ниже.
    if loc.bare_ref {
        if !el.attrs.is_empty() || !el.children.is_empty() {
            return Err(ChildError::Frame(format!(
                "<{}> bare-ref child must be a plain text leaf (no attrs/children)",
                loc.child_tag
            )));
        }
        el.claim_with_text();
        let _ = (version, emit_defaults);
        // ДОТИРОВАННЫЙ bare-ref (`LocusMap::bare_ref_dotted`): текст = `<префикс><Имя>`,
        // канон-имя — ГОЛОЕ (§1.6); иной префикс → типизированная ошибка (§1.0).
        let name = match dotted_prefix {
            Some(prefix) => el
                .text
                .strip_prefix(prefix)
                .ok_or_else(|| {
                    ChildError::Frame(format!(
                        "<{}> dotted bare-ref {:?} does not start with the expected prefix \
                         {prefix:?} (§1.0)",
                        loc.child_tag, el.text
                    ))
                })?
                .to_string(),
            None => el.text.clone(),
        };
        // properties/children пусты (имя — единственная информация в ссылке).
        return Ok(MetadataObject::new(
            ObjectKind::new(bind.child_spec.entity),
            name,
            Uuid([0u8; 16]),
        ));
    }
    el.claim();
    // @uuid — на самом элементе коллекции (в обоих форматах).
    let uuid_attr = el
        .attr("uuid")
        .ok_or_else(|| ChildError::Frame(format!("<{}> child missing @uuid", loc.child_tag)))?;
    let uuid = parse_uuid(&uuid_attr.value)?;
    uuid_attr.claimed.set(true);

    // Где лежат идентичность+свойства: плоско под элементом (EDT) или в <Properties>
    // (Designer). `props_root` — узел, от которого движок читает свойства (его и
    // передаём как `source.root`, чтобы child-локусы были относительны ему).
    let props_root = if loc.props_wrapped {
        let p = el.child("Properties").ok_or_else(|| {
            ChildError::Frame(format!("<{}> child missing <Properties>", loc.child_tag))
        })?;
        p.claim();
        p
    } else {
        el
    };

    // <name>/<Name> — идентичность ребёнка (не из спека). Plain text leaf.
    let name_el = props_root.child(loc.name_tag).ok_or_else(|| {
        ChildError::Frame(format!(
            "<{}> child missing <{}>",
            loc.child_tag, loc.name_tag
        ))
    })?;
    if !name_el.attrs.is_empty() || !name_el.children.is_empty() {
        return Err(ChildError::Frame(format!(
            "<{}> must be a plain text leaf",
            loc.name_tag
        )));
    }
    let name = name_el.text.clone();
    name_el.claim_with_text();

    // Полное имя ребёнка для Designer-type-name'ов producedTypes (`<Parent>.<Child>`).
    let full_name = if name_prefix.is_empty() {
        name.clone()
    } else {
        format!("{name_prefix}.{name}")
    };

    // --- каркас: платформенный блок producedTypes/InternalInfo recursion-узла (если вид
    // несёт категорию). Лежит ДО <name>/<Properties> сиблингом — фреймится здесь, как у
    // корня. Claim'ит всё под-дерево → не попадёт в leftover. props_wrapped различает
    // Designer (`<InternalInfo>`) и EDT (`<producedTypes>`). Лист-вид/без категории → None.
    let child_internal_info = if loc.props_wrapped {
        crate::produced_types::read_designer(el, bind.child_spec.entity, &full_name)
            .map_err(ChildError::Frame)?
    } else {
        crate::produced_types::read_edt(el, bind.child_spec.entity).map_err(ChildError::Frame)?
    };

    // Свойства ребёнка — общий спек-движок (тот же путь, что у корня): локусы
    // child-проекции относительны `props_root`. Движок читает КЛОН `props_root` и
    // ПРОВЕРЯЕТ его тотальность (`leftover==0` по под-дереву property-региона) — то же
    // §1.0, что у корня. Клон нужен, потому что у `props_root` (Designer recursion-узел:
    // `el` сам, props_wrapped=false на EDT) могут быть СИБЛИНГИ к свойствам (вложенный
    // `<methods>`/`<ChildObjects>`), которых property-спек не покрывает — на клоне они
    // дали бы leftover>0. Поэтому движок проверяет ТОЛЬКО property-регион (клон, см.
    // фильтр ниже), а тотальность ВСЕГО `el` (свойства + grandchildren) — сверкой (4).
    let proj = XmlProjection::new(bind.child_map, version, emit_defaults);
    let fields: Vec<_> = bind.child_spec.fields().iter().map(|f| f.id).collect();
    // Клон property-региона БЕЗ под-контейнеров собственных коллекций ребёнка: иначе
    // движковая `leftover`-проверка клона упала бы на не-property сиблингах (вложенный
    // `<methods>` лежит ПОД `props_root` у EDT, где props_root==el). Property-спек их не
    // знает → исключаем из клона ровно дочерние-коллекционные контейнеры (по тегам loc'ов
    // child-проекции). Сами grandchildren разберёт рекурсия (3) на ОРИГИНАЛЕ.
    let props_clone = clone_without_child_collections(props_root, bind);
    let source = XmlSource {
        root: props_clone,
        fields: fields.clone(),
    };
    let bag = engine::read(bind.child_spec, &proj, &source)?;

    // Property-claim-проход по ОРИГИНАЛУ `props_root` (клон выше имел НЕЗАВИСИМЫЕ Cell'ы):
    // помечает РОВНО property-узлы, которые покрывает проекция, чтобы totality-сверка (4)
    // и родительский leftover увидели их востребованными. Тот же claim, что движок делает
    // в своей `leftover`-фазе, но направленный на оригинал (субстратный хелпер).
    crate::claim_props(bind.child_map, version, emit_defaults, props_root, &fields);

    // (3) РЕКУРСИЯ: собственные child-коллекции ребёнка (recursion-узел). Навигация от
    // `el` (НЕ props_root): Designer-вложенный `<ChildObjects>` — сиблинг `<Properties>`.
    let children = read_grandchildren(el, bind, version, emit_defaults, &full_name)?;

    // (4) ТОТАЛЬНОСТЬ по ВСЕМУ `el` (§1.0, фикс nit-1): свойства claim'ены (выше),
    // каркас идентичности claim'ен, grandchildren claim'ены рекурсией. Любой
    // НЕразобранный под-узел `el` (лишний тег/свойство/непокрытый grandchild) → ОШИБКА.
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(ChildError::Frame(format!(
            "<{}> child {name:?}: {leftover} unconsumed node(s) under element (no \
             passthrough/Raw — §1.0)",
            loc.child_tag
        )));
    }

    let mut obj = MetadataObject::new(ObjectKind::new(bind.child_spec.entity), name, uuid);
    obj.properties = bag;
    obj.children = children;
    obj.internal_info = child_internal_info;
    Ok(obj)
}

/// Склонировать property-регион ребёнка, ИСКЛЮЧАЯ прямые под-элементы, являющиеся
/// контейнерами/элементами СОБСТВЕННЫХ коллекций ребёнка. Нужно, чтобы движковая
/// `leftover`-проверка клона видела только property-узлы (grandchild-контейнеры
/// разбирает рекурсия на оригинале). Для лист-ребёнка (нет коллекций) — точная копия.
fn clone_without_child_collections(props_root: &Element, bind: &ChildBinding) -> Element {
    // Теги, которыми ребёнок навигирует свои коллекции (EDT inline-тег + Designer
    // обёртка-контейнер). Эти прямые под-элементы НЕ принадлежат property-региону.
    let mut drop_tags: Vec<&str> = Vec::new();
    for slot in bind.child_spec.children() {
        if let Some(cl) = bind.child_map.child_collection(slot.collection) {
            // EDT: дети — прямые `<child_tag>` под props_root (container пуст). Designer:
            // обёртка — первый сегмент container ПОД el; но props_root у Designer — это
            // <Properties>, а обёртка — СИБЛИНГ его (под el), поэтому в props_root её нет.
            if cl.container.is_empty() {
                drop_tags.push(cl.child_tag); // EDT inline (`methods`)
            } else {
                drop_tags.push(cl.container[0]); // Designer wrapper (`ChildObjects`)
            }
        }
    }
    // Платформенный producedTypes/InternalInfo recursion-узла — НЕ property-регион (его
    // разбирает produced_types на оригинале). Для EDT props_root==el несёт <producedTypes>;
    // исключаем, иначе движковый leftover клона тронул бы его. (Designer InternalInfo —
    // сиблинг <Properties>, в props_root его и так нет.)
    drop_tags.push("producedTypes");
    drop_tags.push("InternalInfo");
    let mut copy = props_root.clone();
    copy.children
        .retain(|c| !(c.prefix.is_empty() && drop_tags.contains(&c.local.as_str())));
    copy
}

/// РЕКУРСИВНО записать всех детей в [`OutElement`]-узлы по `parent_spec.children`.
/// Возвращает узлы для добавления родителем В НУЖНОЕ МЕСТО (EDT — прямо в корень после
/// спек-свойств; Designer — оборачиваются в `<ChildObjects>` вызывающим, см. ниже).
pub fn write_children<M: LocusMap + ?Sized>(
    children: &[MetadataObject],
    parent_spec: &EntitySpec,
    parent_map: &M,
    bindings: &[ChildBinding],
    version: FormatVersion,
    emit_defaults: bool,
) -> Result<Vec<OutElement>, ChildError> {
    write_children_named(
        children,
        parent_spec,
        parent_map,
        bindings,
        version,
        emit_defaults,
        "",
    )
}

/// Как [`write_children`], но с ПРЕФИКСОМ имени родителя для Designer-type-name'ов
/// producedTypes recursion-узлов. Корневой вызов — `""`.
pub fn write_children_named<M: LocusMap + ?Sized>(
    children: &[MetadataObject],
    parent_spec: &EntitySpec,
    parent_map: &M,
    bindings: &[ChildBinding],
    version: FormatVersion,
    emit_defaults: bool,
    name_prefix: &str,
) -> Result<Vec<OutElement>, ChildError> {
    let mut out = Vec::new();
    let mut slots = parent_spec.children().iter().collect::<Vec<_>>();
    if let Some(order) = parent_map.child_emit_order() {
        if order.len() != slots.len()
            || slots.iter().any(|slot| order.iter().filter(|name| **name == slot.collection).count() != 1)
        {
            return Err(ChildError::Frame(format!("{}: incomplete or duplicate physical child order", parent_spec.entity)));
        }
        slots.sort_by_key(|slot| order.iter().position(|name| *name == slot.collection).unwrap());
    }
    for slot in slots {
        let loc = parent_map
            .child_collection(slot.collection)
            .ok_or_else(|| {
                ChildError::Frame(format!(
                    "{}: projection has no child_collection for {:?}",
                    parent_spec.entity, slot.collection
                ))
            })?;
        let bind = binding(bindings, slot.collection).ok_or_else(|| {
            ChildError::Frame(format!(
                "{}: no ChildBinding for collection {:?}",
                parent_spec.entity, slot.collection
            ))
        })?;

        // Дотированный bare-ref: восстановить префикс `<ParentKind>.<ParentName>.<Seg>.`
        // (зеркало read-стороны, R byte-exact).
        let dotted_prefix = parent_map.bare_ref_dotted(slot.collection).then(|| {
            let seg = bind.child_spec.entity.rsplit('.').next().unwrap_or_default();
            format!("{}.{}.{}.", parent_spec.entity, name_prefix, seg)
        });

        for child in children
            .iter()
            .filter(|c| c.kind.as_str() == bind.child_spec.entity)
        {
            out.push(write_one(
                child,
                &loc,
                bind,
                version,
                emit_defaults,
                name_prefix,
                dotted_prefix.as_deref(),
            )?);
        }
    }
    Ok(out)
}

/// Записать ОДИН [`MetadataObject`]-ребёнка в `<child_tag uuid=…>` (+ `<Properties>`
/// для Designer), `<name>`/`<Name>`, спек-свойства, затем РЕКУРСИВНО его собственных
/// детей (recursion-узел): EDT — inline под `el` после свойств; Designer — обёрнуты в
/// `<ChildObjects>`-сиблинг `<Properties>` (симметрия с корневым `write_descriptor`).
fn write_one(
    child: &MetadataObject,
    loc: &crate::locus::ChildLocus,
    bind: &ChildBinding,
    version: FormatVersion,
    emit_defaults: bool,
    name_prefix: &str,
    dotted_prefix: Option<&str>,
) -> Result<OutElement, ChildError> {
    // BARE-REF: Designer `<child_tag>Имя</child_tag>` — лишь имя-ссылка (без uuid/Properties/
    // свойств). Тело под-объекта — отдельный файл; в дескрипторе — членство+порядок.
    // Дотированный вариант (EDT `<tables>`) восстанавливает полный путь из канон-имени.
    if loc.bare_ref {
        let text = match dotted_prefix {
            Some(prefix) => format!("{prefix}{}", child.name),
            None => child.name.clone(),
        };
        return Ok(OutElement::leaf("", loc.child_tag, text));
    }
    let mut el = OutElement::branch("", loc.child_tag).attr("uuid", format_uuid(&child.uuid));

    let full_name = if name_prefix.is_empty() {
        child.name.clone()
    } else {
        format!("{name_prefix}.{}", child.name)
    };

    let proj = XmlProjection::new(bind.child_map, version, emit_defaults);
    let mut sink = XmlSink::default();
    engine::write(bind.child_spec, &proj, &child.properties, &mut sink)?;
    // ХВОСТОВЫЕ свойства (физически ПОСЛЕ inline-детей; EDT `TabularSection.use`) делим до
    // упорядочивания, по `order_tags` (1:1 с `children`). Остальное — ведущие (до детей).
    let trailing_ids = bind.child_map.trailing_fields();
    let mut lead_sink = XmlSink::default();
    let mut trailing_props: Vec<OutElement> = Vec::new();
    for (el, id) in sink.children.into_iter().zip(sink.order_tags) {
        if trailing_ids.contains(&id) {
            trailing_props.push(el);
        } else {
            lead_sink.children.push(el);
            lead_sink.order_tags.push(id);
        }
    }
    // Пер-форматный физический порядок ВЕДУЩИХ свойств ребёнка.
    let child_props = lead_sink.ordered(bind.child_map.field_emit_order());

    // Собственные дети ребёнка (recursion-узел). Лист-ребёнок → пусто.
    let grandchildren = if bind.child_spec.children().is_empty() {
        Vec::new()
    } else {
        write_children_named(
            &child.children,
            bind.child_spec,
            bind.child_map,
            bind.child_map.child_bindings(),
            version,
            emit_defaults,
            &full_name,
        )?
    };

    if loc.props_wrapped {
        // Designer: [<InternalInfo>] — затем <Properties> — затем сиблинг <ChildObjects>.
        if let Some(info) = &child.internal_info {
            el.push(
                crate::produced_types::write_designer(info, bind.child_spec.entity, &full_name)
                    .map_err(ChildError::Frame)?,
            );
        }
        let mut props = OutElement::branch("", "Properties");
        props.push(OutElement::leaf("", loc.name_tag, child.name.clone()));
        for c in child_props {
            props.push(c);
        }
        el.push(props);
        if !grandchildren.is_empty() {
            let mut child_objects = OutElement::branch("", "ChildObjects");
            for g in grandchildren {
                child_objects.push(g);
            }
            el.push(child_objects);
        } else if bind.child_map.emit_empty_child_container() {
            // Recursion-узел ВСЕГДА несёт обёртку (Designer `WebService.Operation`): при
            // нуле grandchildren → самозакрывающийся `<ChildObjects/>` (сверено: 14/143
            // операций без параметров несут `<ChildObjects/>`). Дефолт `false` — прочие
            // recursion-узлы (HTTPService.URLTemplate) пустую обёртку опускают.
            el.push(OutElement::self_closing("", "ChildObjects"));
        }
    } else {
        // EDT: [<producedTypes>] + <name> + ведущие свойства + inline-дети + ХВОСТОВЫЕ
        // свойства (после детей: `TabularSection.use`), все прямыми под `el`.
        if let Some(info) = &child.internal_info {
            el.push(
                crate::produced_types::write_edt(info, bind.child_spec.entity)
                    .map_err(ChildError::Frame)?,
            );
        }
        el.push(OutElement::leaf("", loc.name_tag, child.name.clone()));
        for c in child_props {
            el.push(c);
        }
        for g in grandchildren {
            el.push(g);
        }
        for t in trailing_props {
            el.push(t);
        }
    }
    Ok(el)
}

#[cfg(any())]
mod tests {
    //! Синтетические тесты тотальности рекурсии (фикс nit-1, §1.0). Строят минимальный
    //! 2-уровневый recursion-вид (`Node` → коллекция `Inner` лист-детей) поверх EDT-
    //! раскладки (inline-дети, плоская идентичность) и проверяют, что НЕразобранный
    //! grandchild под recursion-узлом — ОШИБКА, а не молчаливый claim_subtree.

    use super::*;
    use crate::locus::{Codec, FieldProjection, XmlLocus};
    use crate::{parse, ChildLocus, LocusMap};
    use morph1c_core::ir::value::{PropertyValue, ValueKind};
    use morph1c_core::ir::FieldId;
    use morph1c_core::spec::common::{ChildSlot, EntitySpec, FieldSpec};
    use morph1c_core::version::SSL;

    const F_NAME_FIELD: FieldId = FieldId(1);

    // Лист-ребёнок `Inner`: одно текстовое свойство `<value>` (помимо идентичности
    // `<name>`). children: пуст.
    fn inner_spec() -> &'static EntitySpec {
        use std::sync::OnceLock;
        static S: OnceLock<EntitySpec> = OnceLock::new();
        S.get_or_init(|| EntitySpec {
            entity: "Node.Inner",
            fields: Box::leak(
                vec![FieldSpec::with_default(
                    F_NAME_FIELD,
                    "value",
                    ValueKind::Str,
                    PropertyValue::Str(String::new()),
                )]
                .into_boxed_slice(),
            ),
            children: &[],
        })
    }

    // Recursion-узел `Outer`: одно свойство `<value>` + СОБСТВЕННАЯ коллекция `Inner`.
    fn outer_spec() -> &'static EntitySpec {
        use std::sync::OnceLock;
        static S: OnceLock<EntitySpec> = OnceLock::new();
        static SLOT: &[ChildSlot] = &[ChildSlot {
            collection: "Inner",
            child_kind: "Node.Inner",
        }];
        S.get_or_init(|| EntitySpec {
            entity: "Node.Outer",
            fields: Box::leak(
                vec![FieldSpec::with_default(
                    F_NAME_FIELD,
                    "value",
                    ValueKind::Str,
                    PropertyValue::Str(String::new()),
                )]
                .into_boxed_slice(),
            ),
            children: SLOT,
        })
    }

    fn parent_spec() -> &'static EntitySpec {
        use std::sync::OnceLock;
        static S: OnceLock<EntitySpec> = OnceLock::new();
        static SLOT: &[ChildSlot] = &[ChildSlot {
            collection: "Outer",
            child_kind: "Node.Outer",
        }];
        S.get_or_init(|| EntitySpec {
            entity: "Node",
            fields: Box::leak(Vec::new().into_boxed_slice()),
            children: SLOT,
        })
    }

    fn flat(tag: &'static [&'static str]) -> XmlLocus {
        XmlLocus::PropElement { path: tag, ns: "" }
    }

    struct InnerMap;
    impl LocusMap for InnerMap {
        fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
            (field == F_NAME_FIELD)
                .then(|| FieldProjection::new(flat(&["value"]), Codec::PlainText))
        }
    }

    struct OuterMap;
    impl LocusMap for OuterMap {
        fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
            (field == F_NAME_FIELD)
                .then(|| FieldProjection::new(flat(&["value"]), Codec::PlainText))
        }
        fn child_collection(&self, c: &str) -> Option<ChildLocus> {
            (c == "Inner").then_some(ChildLocus {
                container: &[],
                child_tag: "inner",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            })
        }
        fn child_bindings(&self) -> &'static [ChildBinding] {
            use std::sync::OnceLock;
            static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
            B.get_or_init(|| {
                vec![ChildBinding {
                    collection: "Inner",
                    child_spec: inner_spec(),
                    child_map: &InnerMap,
                }]
            })
        }
    }

    struct ParentMap;
    impl LocusMap for ParentMap {
        fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
            None
        }
        fn child_collection(&self, c: &str) -> Option<ChildLocus> {
            (c == "Outer").then_some(ChildLocus {
                container: &[],
                child_tag: "outer",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            })
        }
        fn child_bindings(&self) -> &'static [ChildBinding] {
            use std::sync::OnceLock;
            static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
            B.get_or_init(|| {
                vec![ChildBinding {
                    collection: "Outer",
                    child_spec: outer_spec(),
                    child_map: &OuterMap,
                }]
            })
        }
    }

    const UUID: &str = "00000000-0000-0000-0000-000000000001";

    // Контроль: well-formed 2-уровневое дерево читается БЕЗ ошибки и тотально (родитель
    // leftover==0 после клейма каркаса корня).
    #[test]
    fn recursion_node_well_formed_reads_total() {
        let xml = format!(
            "<root>\
               <outer uuid=\"{UUID}\"><name>O</name><value>v</value>\
                 <inner uuid=\"{UUID}\"><name>I</name><value>w</value></inner>\
               </outer>\
             </root>"
        );
        let d = parse(xml.as_bytes()).expect("parse");
        d.root.claim(); // каркас корня (как делает коннектор)
        let kids = read_children(
            &d.root,
            parent_spec(),
            &ParentMap,
            ParentMap.child_bindings(),
            SSL,
            false,
        )
        .expect("well-formed recursion must read");
        assert_eq!(kids.len(), 1, "one Outer");
        assert_eq!(kids[0].children.len(), 1, "one Inner grandchild");
        assert_eq!(d.root.unclaimed_count(), 0, "all nodes consumed (§1.0)");
    }

    // ФИКС nit-1: НЕразобранный grandchild под recursion-узлом (лишний `<stray>` внутри
    // `<inner>`) → ОШИБКА. До фикса blanket `claim_subtree(outer)` поглотил бы `<stray>`
    // молча (leftover родителя остался бы 0) — латентная дыра §1.0.
    #[test]
    fn recursion_node_unconsumed_grandchild_errors() {
        let xml = format!(
            "<root>\
               <outer uuid=\"{UUID}\"><name>O</name><value>v</value>\
                 <inner uuid=\"{UUID}\"><name>I</name><value>w</value>\
                   <stray>leftover</stray>\
                 </inner>\
               </outer>\
             </root>"
        );
        let d = parse(xml.as_bytes()).expect("parse");
        d.root.claim();
        let res = read_children(
            &d.root,
            parent_spec(),
            &ParentMap,
            ParentMap.child_bindings(),
            SSL,
            false,
        );
        assert!(
            res.is_err(),
            "unconsumed grandchild <stray> under recursion node MUST error (§1.0), got Ok"
        );
    }

    // ФИКС nit-1 (второй угол): лишний тег ПРЯМО под recursion-узлом `<outer>` (сиблинг
    // свойств, не принадлежащий ни property-спеку, ни коллекции `Inner`) → ОШИБКА.
    #[test]
    fn recursion_node_unconsumed_sibling_errors() {
        let xml = format!(
            "<root>\
               <outer uuid=\"{UUID}\"><name>O</name><value>v</value>\
                 <bogus>x</bogus>\
                 <inner uuid=\"{UUID}\"><name>I</name><value>w</value></inner>\
               </outer>\
             </root>"
        );
        let d = parse(xml.as_bytes()).expect("parse");
        d.root.claim();
        let res = read_children(
            &d.root,
            parent_spec(),
            &ParentMap,
            ParentMap.child_bindings(),
            SSL,
            false,
        );
        assert!(
            res.is_err(),
            "unconsumed sibling <bogus> under recursion node MUST error (§1.0)"
        );
    }
}
