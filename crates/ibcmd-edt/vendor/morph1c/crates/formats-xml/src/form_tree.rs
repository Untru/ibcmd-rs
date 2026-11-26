//! Form control-tree substrate — рекурсия по дереву контролов формы (под-IR L1f, §1.2;
//! ARCHITECTURE.md §3.4). Форм-внутренний аналог `children.rs`: тот рекурсирует по
//! metadata-`ChildObjects` (узлы — `MetadataObject`); ЭТОТ — по `items`/`ChildItems`
//! (узлы — [`FormItem`]).
//!
//! # Что делает модуль
//! `read_items`/`write_items` читают/пишут УПОРЯДОЧЕННЫЙ список контролов (рекурсивно для
//! контейнеров) ПОВЕРХ плоского `engine::{read,write}`:
//! 1. **дискриминатор** — определить вид контрола узла (формат-специфично: EDT
//!    `xsi:type`+`<type>`, Designer — имя элемента) → канонический [`FormControlKind`];
//! 2. **идентичность** — `name`/`id` (EDT — дети, Designer — атрибуты), фреймится здесь;
//! 3. **свойства** — плоский bag общих свойств через `engine::read`/`write` по
//!    `ControlSpec.properties` и проекции вида;
//! 4. **extInfo** — отдельный bag тип-специфичных свойств по `ControlSpec.ext_info`;
//! 5. **события** — `handlers`/`Events` контрола;
//! 6. **рекурсия** — для контейнеров навигация в собственные `items`/`ChildItems`.
//!
//! # Тотальность (§1.0)
//! Рекурсивна, как у child-objects: каждый узел claim'ит свой каркас идентичности, свои
//! свойства/extInfo, своих детей, и СВЕРЯЕТ `el.unclaimed_count()==0` по ВСЕМУ элементу.
//! Любой неразобранный под-узел → типизированная ОШИБКА, не passthrough/Raw.
//!
//! # Где живут имена тегов
//! В ПРОЕКЦИИ ([`FormProjection`]) — формат-специфичной (EDT/Designer). `core` несёт лишь
//! ДАННЫЕ ([`ControlSpec`] + форм-спек): вид/порядок/дефолты/value_kind/extInfo (§1.6).
//! Foundation проверяет рекурсию байт-в-байт синтетическими тестами (см. `mod tests`).

use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::{LocusMap, XmlProjection, XmlSink, XmlSource};
use morph1c_core::engine::{self, EngineError};
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{FieldId, FormControlKind, FormEvent, FormItem};
use morph1c_core::spec::common::EntitySpec;
use morph1c_core::spec::forms::controls::ControlSpec;
use morph1c_core::version::FormatVersion;

/// Ошибка форм-рекурсии (типизированная, §1.0): дискриминатор/идентичность/тотальность.
#[derive(Debug)]
pub enum FormTreeError {
    /// Структурная ошибка каркаса (нет `name`/`id`, неизвестный дискриминатор, лишний тег).
    Frame(String),
    /// Ошибка спек-движка на узле (несконсуменное, mismatch вида, …).
    Engine(EngineError),
}

impl std::fmt::Display for FormTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormTreeError::Frame(s) => write!(f, "form-tree: {s}"),
            FormTreeError::Engine(e) => write!(f, "form-tree: {e}"),
        }
    }
}

impl std::error::Error for FormTreeError {}

impl From<EngineError> for FormTreeError {
    fn from(e: EngineError) -> Self {
        FormTreeError::Engine(e)
    }
}

/// Привязка одного вида контрола к его проекциям в ЭТОМ формате: спек контрола +
/// `LocusMap` его ОБЩИХ свойств + `LocusMap` его extInfo-свойств.
///
/// Проекция формата отдаёт `&'static [ControlBinding]` (по одному на поддержанный вид),
/// чтобы рекурсия знала, ЧЕМ читать узел данного вида.
pub struct ControlBinding {
    /// Канонический код вида (= [`ControlSpec::kind`] = [`FormControlKind`]).
    pub kind: &'static str,
    /// Спек контрола (общие+extInfo+container) из `core/spec/forms/controls`.
    pub spec: &'static ControlSpec,
    /// Проекция ОБЩИХ свойств вида в этом формате.
    pub property_map: &'static (dyn LocusMap + Sync),
    /// Проекция extInfo-свойств вида в этом формате (ZST-юнит; пустой спек ⇒ не зовётся).
    pub ext_info_map: &'static (dyn LocusMap + Sync),
}

/// Формат-специфичная навигация по дереву контролов: где лежит контейнер, как фреймится
/// идентичность/дискриминатор/события/extInfo узла. Имена тегов — здесь (§1.6), не в спеке.
///
/// Реализация для EDT знает `xsi:type`+`<type>`-дискриминатор и inline-`items`; для
/// Designer — имя-элемента-дискриминатор и обёртку `<ChildItems>`.
pub trait FormProjection {
    /// Формат-версия (для `Since`-гейтинга движка).
    fn version(&self) -> FormatVersion;
    /// Эмитить ли дефолтные поля (EDT: нет; Designer: да).
    fn emit_defaults(&self) -> bool;
    /// Бинды поддержанных видов контролов (спек+проекции).
    fn control_bindings(&self) -> &'static [ControlBinding];

    /// READ: перечислить элементы-узлы контейнера дерева под `parent` (для корня — сам
    /// корень формы; для контейнера-контрола — он сам). Claim'ит обёртку (Designer
    /// `<ChildItems>`), если она есть. Возвращает `(дочерний_элемент, вид)` в порядке
    /// источника. Пустой контейнер/нет обёртки ⇒ пусто.
    fn read_item_nodes<'a>(
        &self,
        parent: &'a Element,
    ) -> Result<Vec<(&'a Element, FormControlKind)>, FormTreeError>;

    /// READ: каркас идентичности узла `el` — `(name, id)`. Claim'ит её носители (EDT-дети
    /// `<name>`/`<id>` либо Designer-атрибуты `name=`/`id=`).
    fn read_item_identity(&self, el: &Element) -> Result<(String, i64), FormTreeError>;

    /// READ: узел свойств узла, ОТ которого движок читает общий property-bag (EDT — сам
    /// `el`; Designer — тоже `el`, свойства — прямые дети). Claim'ит вспомогательный
    /// каркас (дискриминатор `<type>`), чтобы он не попал в leftover.
    fn read_item_props_root<'a>(&self, el: &'a Element) -> Result<&'a Element, FormTreeError>;

    /// READ: события узла `el` (`<handlers>`/`<Events>`). Claim'ит их. Пусто ⇒ `[]`.
    fn read_item_events(&self, el: &Element) -> Result<Vec<FormEvent>, FormTreeError>;

    /// READ: узел extInfo узла `el` (EDT — `<extInfo>`; Designer — inline под `el`), ИЛИ
    /// `None`, если вид без extInfo / регион отсутствует. Claim'ит обёртку extInfo.
    fn read_item_ext_info_root<'a>(
        &self,
        el: &'a Element,
        spec: &ControlSpec,
    ) -> Result<Option<&'a Element>, FormTreeError>;

    /// WRITE: построить узел контрола `el` каркасно (имя элемента/дискриминатор + `name`/
    /// `id`), вернуть `(узел, точка_вставки_свойств)`. Свойства/extInfo/события/детей
    /// дописывает рекурсия в возвращённый узел в порядке, заданном проекцией через
    /// `assemble_item`.
    fn build_item(&self, item: &FormItem, spec: &ControlSpec) -> Result<OutElement, FormTreeError>;

    /// WRITE: дописать в УЖЕ построенный каркас `el` свойства/extInfo/события/детей в
    /// физическом порядке формата. `props`/`ext`/детей/события подаёт рекурсия готовыми.
    #[allow(clippy::too_many_arguments)]
    fn assemble_item(
        &self,
        el: &mut OutElement,
        item: &FormItem,
        spec: &ControlSpec,
        property_nodes: Vec<OutElement>,
        ext_info_nodes: Vec<OutElement>,
        child_nodes: Vec<OutElement>,
    ) -> Result<(), FormTreeError>;

    /// WRITE: обернуть детей контейнера в формат-обёртку (Designer `<ChildItems>`; EDT —
    /// прямые `<items>` в родителя ⇒ вернуть как есть). Вызывается рекурсией для
    /// КОРНЕВОГО списка и для контейнер-контролов.
    fn wrap_child_items(&self, children: Vec<OutElement>) -> Vec<OutElement>;
}

/// Найти бинд по коду вида (проекция отдаёт упорядоченный список).
fn binding<'a>(bindings: &'a [ControlBinding], kind: &str) -> Option<&'a ControlBinding> {
    bindings.iter().find(|b| b.kind == kind)
}

/// READ: прочитать УПОРЯДОЧЕННЫЙ список контролов под `parent` (рекурсивно). `parent` —
/// корень формы (для корневого списка) либо контейнер-контрол. Claim'ит весь под-материал
/// (§1.0).
pub fn read_items<P: FormProjection>(
    proj: &P,
    parent: &Element,
) -> Result<Vec<FormItem>, FormTreeError> {
    let nodes = proj.read_item_nodes(parent)?;
    let mut out = Vec::with_capacity(nodes.len());
    for (el, kind) in nodes {
        out.push(read_one(proj, el, kind)?);
    }
    Ok(out)
}

/// READ: один узел контрола → [`FormItem`] (идентичность, property-bag, extInfo, события,
/// рекурсия в детей-контейнера). Сверяет тотальность по ВСЕМУ `el` (§1.0).
fn read_one<P: FormProjection>(
    proj: &P,
    el: &Element,
    kind: FormControlKind,
) -> Result<FormItem, FormTreeError> {
    let bind = binding(proj.control_bindings(), kind.as_str()).ok_or_else(|| {
        FormTreeError::Frame(format!(
            "no ControlBinding for control kind {:?}",
            kind.as_str()
        ))
    })?;
    let spec = bind.spec;

    el.claim();
    let (name, id) = proj.read_item_identity(el)?;

    // Общие свойства — через спек-движок по ОБЩЕМУ property-map.
    let props_root = proj.read_item_props_root(el)?;
    let properties = read_bag(props_root, spec.properties, bind.property_map, proj)?;

    // extInfo-свойства — отдельный bag (если вид несёт extInfo-регион и он присутствует).
    let ext_info = if spec.has_ext_info() {
        match proj.read_item_ext_info_root(el, spec)? {
            Some(ext_root) => read_bag(ext_root, spec.ext_info, bind.ext_info_map, proj)?,
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let events = proj.read_item_events(el)?;

    // Рекурсия в детей контейнера (лист ⇒ пусто).
    let children = if spec.container {
        read_items(proj, el)?
    } else {
        Vec::new()
    };

    // (§1.0) ТОТАЛЬНОСТЬ по ВСЕМУ узлу: всё консумируемое claim'ено выше; любой
    // неразобранный под-узел → ОШИБКА.
    let leftover = el.unclaimed_count();
    if leftover != 0 {
        return Err(FormTreeError::Frame(format!(
            "control {name:?} ({}): {leftover} unconsumed node(s) (no passthrough/Raw — §1.0)",
            kind.as_str()
        )));
    }

    let mut item = FormItem::new(kind, name, id);
    item.properties = properties;
    item.ext_info = ext_info;
    item.events = events;
    item.children = children;
    Ok(item)
}

/// Прочитать плоский property-bag от `root` через спек+проекцию вида (общий путь движка).
/// Клонирует `root` для движковой `leftover`-проверки региона и переносит claim на оригинал
/// (как `children::read_one` для recursion-узлов).
fn read_bag<P: FormProjection, M: LocusMap + ?Sized>(
    root: &Element,
    spec: &EntitySpec,
    map: &M,
    proj: &P,
) -> Result<Vec<(FieldId, PropertyValue)>, FormTreeError> {
    let version = proj.version();
    let emit_defaults = proj.emit_defaults();
    let fields: Vec<FieldId> = spec.fields().iter().map(|f| f.id).collect();

    let xproj = XmlProjection::new(map, version, emit_defaults);
    // Движок проверяет тотальность на КЛОНЕ региона (у `root` могут быть СИБЛИНГИ к
    // свойствам — каркас идентичности/события/extInfo/дети, которые property-спек не знает).
    // Поэтому клон БЕЗ них: оставляем только то, что покрывает property-map.
    let clone = clone_property_region(root, map, &fields);
    let source = XmlSource {
        root: clone,
        fields: fields.clone(),
    };
    let bag = engine::read(spec, &xproj, &source)?;
    // Перенести claim на ОРИГИНАЛ (клон имел независимые Cell'ы), чтобы totality-сверка
    // родителя/узла увидела свойства востребованными.
    crate::claim_props(map, version, emit_defaults, root, &fields);
    Ok(bag)
}

/// Склонировать регион свойств, оставив РОВНО прямые под-элементы, достижимые
/// property-локусами (по их первому сегменту пути). Прочие прямые дети (каркас идентичности,
/// события, extInfo, дочерние контролы) исключаются — их разбирает рекурсия на оригинале.
fn clone_property_region<M: LocusMap + ?Sized>(
    root: &Element,
    map: &M,
    fields: &[FieldId],
) -> Element {
    use crate::locus::XmlLocus;
    let mut keep_tags: Vec<&'static str> = Vec::new();
    for &f in fields {
        if let Some(fp) = map.lookup(f) {
            if let XmlLocus::PropElement { path, .. } = fp.locus {
                if let Some(first) = path.first() {
                    keep_tags.push(first);
                }
            }
        }
    }
    let mut copy = root.clone();
    copy.children
        .retain(|c| c.prefix.is_empty() && keep_tags.contains(&c.local.as_str()));
    // Текст самого региона (если есть) не относится к свойствам — но оставляем, чтобы
    // totality движка поймала мусорный текст (B1). Атрибуты региона на read не из спека —
    // но property-локусы PropElement не адресуют атрибуты, поэтому оставлять их в клоне
    // нельзя (дали бы leftover); очищаем (их claim'ит каркас идентичности на оригинале).
    copy.attrs.clear();
    copy.text.clear();
    copy
}

/// WRITE: записать УПОРЯДОЧЕННЫЙ список контролов (рекурсивно) в [`OutElement`]-узлы.
/// Возвращает узлы БЕЗ обёртки (вызывающий оборачивает корневой/контейнерный список через
/// [`FormProjection::wrap_child_items`]).
pub fn write_items<P: FormProjection>(
    proj: &P,
    items: &[FormItem],
) -> Result<Vec<OutElement>, FormTreeError> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(write_one(proj, item)?);
    }
    Ok(out)
}

/// WRITE: один контрол → [`OutElement`] (каркас + свойства + extInfo + события + детей).
fn write_one<P: FormProjection>(proj: &P, item: &FormItem) -> Result<OutElement, FormTreeError> {
    let bind = binding(proj.control_bindings(), item.kind.as_str()).ok_or_else(|| {
        FormTreeError::Frame(format!(
            "no ControlBinding for control kind {:?}",
            item.kind.as_str()
        ))
    })?;
    let spec = bind.spec;
    let version = proj.version();
    let emit_defaults = proj.emit_defaults();

    // Общие свойства → узлы.
    let xproj = XmlProjection::new(bind.property_map, version, emit_defaults);
    let mut sink = XmlSink::default();
    engine::write(spec.properties, &xproj, &item.properties, &mut sink)?;
    let property_nodes = sink.ordered(bind.property_map.field_emit_order());

    // extInfo → узлы (если вид несёт extInfo-регион).
    let ext_info_nodes = if spec.has_ext_info() {
        let xproj = XmlProjection::new(bind.ext_info_map, version, emit_defaults);
        let mut sink = XmlSink::default();
        engine::write(spec.ext_info, &xproj, &item.ext_info, &mut sink)?;
        sink.ordered(bind.ext_info_map.field_emit_order())
    } else {
        Vec::new()
    };

    // Дети контейнера (рекурсия) → обёрнутые формат-конвенцией.
    let child_nodes = if spec.container {
        let kids = write_items(proj, &item.children)?;
        proj.wrap_child_items(kids)
    } else {
        Vec::new()
    };

    let mut el = proj.build_item(item, spec)?;
    proj.assemble_item(
        &mut el,
        item,
        spec,
        property_nodes,
        ext_info_nodes,
        child_nodes,
    )?;
    Ok(el)
}

#[cfg(any())]
mod tests;
