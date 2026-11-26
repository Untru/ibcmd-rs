//! `formats-designer` — коннектор Designer-XML (`.xml` выгрузка Конфигуратора):
//! Reader/Writer дескриптора объекта метаданных вокруг канонического IR
//! (ARCHITECTURE.md §2.1), byte-exact на записи (§3.2). Раскладка зеркалит `core/spec`
//! (§5): `metadata/<вид>`, `common/`.
//!
//! Envelope ВЕРСИЯ-ОСОЗНАН (FORMATS.md §1/§3, [`common`]): Reader ДЕТЕКТИТ версию
//! формата из корневого `version=` и сверяет РОВНО её ns-блок (реестр
//! [`common::ENVELOPE_PROFILES`]: SSL 2.21 c `pal` / ERP 2.20 без `pal` / …); Writer
//! эмитит ns-блок+`version=` ЗАДАННОЙ таргет-версии ([`write_descriptor_versioned`]),
//! обратно-совместимый [`write_descriptor`] фиксирует SSL. Детектированная/таргет
//! версия драйвит И envelope, И since-гейтинг движка (2.21-only поля не ждутся/не
//! эмитятся в 2.20).
//!
//! Контракт (общий kind-параметризованный путь, пилот-вид `CommonModule`):
//! * Reader фреймит идентичность объекта (`kind`, `<Name>`-элемент, `<CommonModule
//!   uuid=…>`) в [`MetadataObject`], сверяет байт-обёртку (BOM/CRLF/TAB, пролог) +
//!   версия-осознанный ns-блок/`version`, затем драйвит `core::engine::read(…)` ПОД
//!   детектированной версией через общий XML-субстрат → канонический разрежённый IR.
//!   Это ТОТ ЖЕ IR, что даёт EDT (§1.6/§3.5).
//! * Writer регенерирует `.xml` БАЙТ-В-БАЙТ из [`MetadataObject`] под таргет-версией:
//!   envelope Designer (BOM, CRLF, TAB), корень `<MetaDataObject>` с ns-блоком+version
//!   таргета, `<<kind> uuid>` → `<Properties>` → `<Name>` + спек-свойства (DENSE:
//!   проекция эмитит даже дефолты — `<Privileged>false</Privileged>`, `<Comment/>`, …).
//!
//! Тело модуля `Ext/Module.bsl` — ВНЕ дескриптора (отдельный артефакт/слайс): ридер
//! его не читает, `MetadataObject.modules` остаётся пустым (как у EDT). Это НЕ
//! нарушение §1.0: тело — отдельный файл, а не несконсуменный фрагмент дескриптора.

pub mod common;
pub mod language;
pub mod metadata;

use common::{
    emit_root_envelope, verify_root_envelope, DESIGNER_ENVELOPE, NAME_ELEMENT, PROPERTIES_ELEMENT,
    ROOT_ELEMENT, UUID_ATTR,
};
use metadata::common_module::DesignerCommonModule;

use formats_xml::emit::render;
use formats_xml::{
    children, parse, produced_types, Element, LocusMap, OutElement, XmlProjection,
    XmlReadError, XmlSink, XmlSource,
};
use morph1c_core::engine::{self, EngineError};
use morph1c_core::ir::{MetadataObject, ObjectKind, Uuid};
use morph1c_core::spec::common::EntitySpec;
use morph1c_core::spec::metadata::common_module::common_module;
use morph1c_core::version::{FormatVersion, SSL};

/// Канонический код вида объекта Designer `CommonModule`.
pub const KIND_COMMON_MODULE: &str = "CommonModule";

/// Ошибка коннектора Designer (типизированная, §1.0 — без best-effort/skip).
#[derive(Debug)]
pub enum DesignerError {
    /// Сбой токенизации/структуры XML.
    Xml(XmlReadError),
    /// Дескриптор не соответствует ожидаемой Designer-обёртке (BOM/EOL/пролог/корень/
    /// ns-блок/version/идентичность).
    Envelope(String),
    /// Ошибка spec-driven движка (несконсуменное, mismatch вида, …).
    Engine(EngineError),
}

impl std::fmt::Display for DesignerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DesignerError::Xml(e) => write!(f, "designer: {e}"),
            DesignerError::Envelope(s) => write!(f, "designer: envelope mismatch: {s}"),
            DesignerError::Engine(e) => write!(f, "designer: {e}"),
        }
    }
}

impl std::error::Error for DesignerError {}

impl From<XmlReadError> for DesignerError {
    fn from(e: XmlReadError) -> Self {
        DesignerError::Xml(e)
    }
}
impl From<EngineError> for DesignerError {
    fn from(e: EngineError) -> Self {
        DesignerError::Engine(e)
    }
}

/// Распарсить hex-uuid (`8-4-4-4-12`, lower-case) в 16 байт. Любая иная форма —
/// ошибка (§1.0: не best-effort). Возвращает [`Uuid`].
fn parse_uuid(s: &str) -> Result<Uuid, DesignerError> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(DesignerError::Envelope(format!("malformed uuid {s:?}")));
    }
    let mut bytes = [0u8; 16];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|e| DesignerError::Envelope(format!("uuid hex: {e}")))?;
    }
    Ok(Uuid(bytes))
}

/// Форматировать 16 байт обратно в каноническую hex-строку uuid (lower-case, дефисы
/// `8-4-4-4-12`) — ровно как пишет Designer.
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

/// Designer-коннектор для одного вида (F2b: `CommonModule`).
///
/// Симметрия §2.1: один тип несёт и [`read`](DesignerCommonModuleConnector::read), и
/// [`write`](DesignerCommonModuleConnector::write) вокруг общего IR.
pub struct DesignerCommonModuleConnector;

impl Default for DesignerCommonModuleConnector {
    fn default() -> Self {
        Self
    }
}

impl DesignerCommonModuleConnector {
    /// Прочитать дескриптор `.xml` (байты) в канонический [`MetadataObject`].
    pub fn read(&self, bytes: &[u8]) -> Result<MetadataObject, DesignerError> {
        read_descriptor(
            KIND_COMMON_MODULE,
            common_module(),
            &DesignerCommonModule,
            bytes,
        )
    }

    /// Регенерировать `.xml` БАЙТ-В-БАЙТ из канонического [`MetadataObject`].
    pub fn write(&self, obj: &MetadataObject) -> Result<Vec<u8>, DesignerError> {
        write_descriptor(
            KIND_COMMON_MODULE,
            common_module(),
            &DesignerCommonModule,
            obj,
        )
    }
}

/// Прочитать Designer-дескриптор ЛЮБОГО вида в канонический [`MetadataObject`]
/// (kind-параметризованный путь — общий для всех metadata-видов Designer).
///
/// Шаги: токенизация → сверка байт-обёртки (BOM/EOL/пролог/корень) → ДЕТЕКТ версии
/// формата из `version=` + сверка/claim версионного ns-блока ([`verify_root_envelope`],
/// FORMATS.md §1: формат входа детектится) → claim каркаса (`<<kind> uuid>`,
/// `<Properties>`, `<Name>`) → движок читает свойства ПОД ДЕТЕКТИРОВАННОЙ версией
/// (since-гейтинг корректен для входа) → §1.0-проверка тотальности (`leftover==0` по
/// ВСЕМУ дереву `<MetaDataObject>`). Поддерживает ВСЕ версии реестра (SSL 2.21 / ERP
/// 2.20 / …) — не хардкод одного корпуса.
///
/// `kind` — local-name обёртки вида под корнем (= канонический код вида, напр.
/// `CommonModule`); `spec`/`map` — его канонический спек и Designer-проекция. Локусы в
/// `map` стартуют ИМЕННО с `[kind, "Properties", …]`, поэтому путь согласован с обёрткой.
pub fn read_descriptor<M: LocusMap>(
    kind: &str,
    spec: &EntitySpec,
    map: &M,
    bytes: &[u8],
) -> Result<MetadataObject, DesignerError> {
    let descriptor = parse(bytes)?;

    // Lexical BOM/EOL/declaration spelling is retained by adapter provenance.
    // The bounded adapter validates UTF-8/XML; semantic namespace/version guards remain.

    let root = descriptor.root;

    // --- envelope: корень <MetaDataObject> (без префикса) ---
    if !root.prefix.is_empty() || root.local != ROOT_ELEMENT {
        return Err(DesignerError::Envelope(format!(
            "unexpected root <{}>, want <{ROOT_ELEMENT}>",
            qname(&root)
        )));
    }
    // Каркас claim'ит сам узел корня; его атрибуты (ns-блок+version) и детей —
    // ниже / в движке, иначе они попали бы в `leftover` (§1.0).
    root.claim();

    // --- envelope: DETECT версию формата + сверить/claim версионный блок (§3, FORMATS.md
    // §1: формат входа детектится). `verify_root_envelope` читает корневой `version=`,
    // выбирает профиль реестра (SSL 2.21+pal / ERP 2.20 no-pal / …), сверяет+claim'ит
    // РОВНО его ns-блок и `version`. Чужой/лишний ns (напр. `pal` в ERP) НЕ claim'ится →
    // упрётся в тотальность ниже (§1.0). Детектированная версия драйвит движок И
    // since-гейтинг (не безусловный SSL). ---
    let version = verify_root_envelope(&root).map_err(DesignerError::Envelope)?;

    // --- каркас: <<kind> uuid> → <Properties> → <Name> ---
    let cm = root
        .child(kind)
        .ok_or_else(|| DesignerError::Envelope(format!("missing <{kind}> element")))?;
    if !cm.prefix.is_empty() {
        return Err(DesignerError::Envelope(format!(
            "<{kind}> must be unprefixed, got prefix {:?}",
            cm.prefix
        )));
    }
    cm.claim();

    let uuid_attr = cm
        .attr(UUID_ATTR)
        .ok_or_else(|| DesignerError::Envelope(format!("missing <{kind}> @uuid")))?;
    let uuid = parse_uuid(&uuid_attr.value)?;
    uuid_attr.claimed.set(true);

    let props = cm.child(PROPERTIES_ELEMENT).ok_or_else(|| {
        DesignerError::Envelope(format!("missing <{PROPERTIES_ELEMENT}> element"))
    })?;
    if !props.prefix.is_empty() {
        return Err(DesignerError::Envelope(format!(
            "<{PROPERTIES_ELEMENT}> must be unprefixed, got prefix {:?}",
            props.prefix
        )));
    }
    props.claim();

    // <Name> — первый ребёнок <Properties> (идентичность, не из спека).
    let name_el = props
        .child(NAME_ELEMENT)
        .ok_or_else(|| DesignerError::Envelope(format!("missing <{NAME_ELEMENT}> element")))?;
    if !name_el.prefix.is_empty() {
        return Err(DesignerError::Envelope(format!(
            "<{NAME_ELEMENT}> must be unprefixed, got prefix {:?}",
            name_el.prefix
        )));
    }
    if !name_el.attrs.is_empty() || !name_el.children.is_empty() {
        return Err(DesignerError::Envelope(format!(
            "<{NAME_ELEMENT}> must be a plain text leaf"
        )));
    }
    let name = name_el.text.clone();
    name_el.claim_with_text();

    // --- каркас: платформенный блок <InternalInfo> (если есть), дитя <<kind>> и
    // СИБЛИНГ <Properties> (идёт ДО него). Вне спек-региона → фреймится здесь.
    // Claim'им всё поддерево → leftover==0 по всему <MetaDataObject>. Категории type-id
    // (Container/Ref/List/Manager) и канон-порядок IR — по виду (общий каркас §3.5).
    // Ключ реестра категорий — КАНОН-вид `spec.entity` (для top-level видов == `kind`;
    // standalone-дескриптор ДОЧЕРНЕГО вида — EDS-таблица: корень `Table`, спек
    // `ExternalDataSource.Table` — резолвится только по entity).
    let (internal_info, this_node) =
        produced_types::read_designer_with_this_node(cm, spec.entity, &name)
            .map_err(DesignerError::Envelope)?;

    // --- child-objects: рекурсивное чтение `<ChildObjects>` (§3.4) ПЕРЕД движком ---
    // `engine::read` ниже проверяет тотальность по ВСЕМУ <MetaDataObject>, включая
    // `<ChildObjects>`, которого НЕТ в parent-LocusMap. Поэтому детей читаем+claim'им
    // первыми (каждый проверяет свою тотальность сам). Лист-вид: bindings/children
    // пусты → no-op.
    let extensions = formats_xml::source_extensions::read_designer_refs(kind, &root).map_err(DesignerError::Envelope)?;
    let source = XmlSource {
        root,
        fields: spec.fields().iter().map(|f| f.id).collect(),
    };
    let children = children::read_children_named(
        &source.root,
        spec,
        map,
        map.child_bindings(),
        version,
        /*emit_defaults=*/ true,
        &name,
    )
    .map_err(|e| DesignerError::Engine(child_engine_err(e)))?;

    // --- spec-driven чтение свойств корня через общий XML-субстрат ---
    // `source.root` = весь <MetaDataObject> → leftover считается по ВСЕМУ дереву.
    // Проекция ведётся под ДЕТЕКТИРОВАННОЙ версией → since-гейтинг корректен для входа
    // (ERP 2.20 не несёт 2.21-only полей — движок их не ждёт).
    let proj = XmlProjection::new(map, version, /*emit_defaults=*/ true);
    let bag = engine::read(spec, &proj, &source)?;

    let mut obj = MetadataObject::new(ObjectKind::new(kind), name, uuid);
    obj.internal_info = internal_info;
    obj.this_node = this_node;
    obj.properties = bag;
    obj.children = children;
    obj.source_extensions = extensions;
    Ok(obj)
}

/// Свести [`children::ChildError`] к движковой ошибке коннектора (типизированно, §1.0).
fn child_engine_err(e: children::ChildError) -> EngineError {
    match e {
        children::ChildError::Engine(eng) => eng,
        children::ChildError::Frame(s) => EngineError::Projection {
            entity: "child-objects",
            field: None,
            reason: s,
        },
    }
}

// Амбьентная таргет-версия для версия-БЛАЙНД фасада `write_descriptor` (и, через него,
// для kind-параметризованных per-kind `HARNESS_ENTRY.write`-обёрток, которые НЕ несут
// версию в сигнатуре `fn(&MetadataObject)`). ПО УМОЛЧАНИЮ `None` → фасад фиксирует SSL
// (весь текущий SSL-корпус byte-exact, поведение не меняется).
//
// Единственный установщик — `with_roundtrip_target` (round-trip R-гейт): для байт-в-байт
// round-trip писатель ОБЯЗАН воспроизвести версию ИСТОЧНИКА (ERP 2.20 без `pal` / SSL 2.21
// с `pal`). Это версия ВЫХОДА (параметр конверсии); версию ВХОДА несёт сам IR —
// `Configuration::source_version`. Скоуп — ровно один write-вызов, сбрасывается сразу
// (RAII-guard).
// НОСИТЕЛЬ ПЕРЕЕХАЛ в `morph1c_core::version` (кросс-форматная забота: cf configuration-root
// тоже version-aware); здесь — делегирующие реэкспорты, API прежний.
pub use morph1c_core::version::{current_roundtrip_target, with_roundtrip_target};

/// Регенерировать Designer-дескриптор `.xml` ЛЮБОГО вида БАЙТ-В-БАЙТ из IR.
///
/// Формат ВЫХОДА — параметр (FORMATS.md §1), и в сигнатуре per-kind `write()` его нет
/// (версия ВХОДА, живущая в `Configuration::source_version`, — это про чтение, а таргет
/// записи задаётся отдельно). Исторические per-kind `write()` не несут таргет → этот
/// 4-арг вход берёт версию из АМБЬЕНТНОГО round-trip таргета ([`with_roundtrip_target`]),
/// если он установлен (round-trip воспроизводит версию источника: ERP 2.20 / SSL 2.21),
/// иначе фиксирует SSL (обратная совместимость — весь текущий SSL-корпус byte-exact).
/// Явная версионная запись — [`write_descriptor_versioned`].
pub fn write_descriptor<M: LocusMap>(
    kind: &str,
    spec: &EntitySpec,
    map: &M,
    obj: &MetadataObject,
) -> Result<Vec<u8>, DesignerError> {
    let target = current_roundtrip_target().unwrap_or(SSL);
    write_descriptor_versioned(kind, spec, map, obj, target)
}

/// Регенерировать Designer-дескриптор `.xml` ЛЮБОГО вида БАЙТ-В-БАЙТ из IR под ЗАДАННОЙ
/// таргет-версией формата (FORMATS.md §1: формат выхода — параметр).
///
/// НЕ эхо входа: дескриптор строится из IR+спека+envelope (§1.1). Корень
/// `<MetaDataObject>` получает ns-блок+`version=` ТАРГЕТ-версии
/// ([`emit_root_envelope`]); движок и since-гейтинг ведутся под ней (2.21-only поля НЕ
/// эмитятся в 2.20-таргет). `<<kind>>` — `uuid`, `<Properties>` — `<Name>` затем
/// спек-свойства (DENSE: даже дефолты). Неизвестная версия ⇒ envelope-ошибка (§1.0).
pub fn write_descriptor_versioned<M: LocusMap>(
    kind: &str,
    spec: &EntitySpec,
    map: &M,
    obj: &MetadataObject,
    target: FormatVersion,
) -> Result<Vec<u8>, DesignerError> {
    if obj.kind.as_str() != kind {
        return Err(DesignerError::Envelope(format!(
            "writer expects kind {kind}, got {}",
            obj.kind.as_str()
        )));
    }

    // Корень <MetaDataObject> + ns-блок + `version=` ТАРГЕТ-версии (реестр обёрток).
    let mut root = emit_root_envelope(target).map_err(DesignerError::Envelope)?;

    // <<kind> uuid="…"> → [<InternalInfo>] → <Properties> → <Name> + спек-свойства.
    let mut cm = OutElement::branch("", kind).attr(UUID_ATTR, format_uuid(&obj.uuid));

    // <InternalInfo> — ПЕРЕД <Properties> (структурная позиция эталона), если объект
    // несёт платформенный блок. name/category регенерируются из вида+имени+category.
    if let Some(info) = &obj.internal_info {
        cm.push(
            produced_types::write_designer_with_this_node(
                info,
                // Ключ — канон-вид `spec.entity` (см. read-сторону).
                spec.entity,
                &obj.name,
                obj.this_node.as_ref(),
            )
            .map_err(DesignerError::Envelope)?,
        );
    }

    let mut props = OutElement::branch("", PROPERTIES_ELEMENT);

    // Движок дописывает спек-свойства в каноническом порядке (DENSE) под таргет-версией.
    let proj = XmlProjection::new(map, target, /*emit_defaults=*/ true);
    let mut sink = XmlSink::default();
    engine::write(spec, &proj, &obj.properties, &mut sink)?;
    // ГОЛОВНЫЕ свойства (физически ПЕРЕД каркасным `<Name>`; witnessed: `ObjectBelonging`
    // корня расширения) отделяем ДО хойста — зеркало `trailing_fields` EDT-писателя.
    // Дефолт `properties_head_fields()==&[]` ⇒ ВСЕ прочие виды байт-идентичны.
    let head_ids = map.properties_head_fields();
    let emit_order = map.field_emit_order_for_version(target);
    let (mut head_sink, mut body_sink) = (XmlSink::default(), XmlSink::default());
    for (el, id) in sink.children.into_iter().zip(sink.order_tags) {
        let dst = if head_ids.contains(&id) {
            &mut head_sink
        } else {
            &mut body_sink
        };
        dst.children.push(el);
        dst.order_tags.push(id);
    }
    for child in head_sink.ordered(emit_order.as_deref()) {
        props.push(child);
    }
    props.push(OutElement::leaf("", NAME_ELEMENT, obj.name.clone()));
    // ХОЙСТ структурных блоков, эмитированных СПЕК-СВОЙСТВОМ как сиблингов `<Properties>`
    // (корень Configuration): `<InternalInfo>` (платформенные containedObjects) — ПЕРЕД
    // `<Properties>`; `<ChildObjects>` (имена объектов) — ПОСЛЕ. Это структурная позиция
    // Designer-дескриптора (InternalInfo, Properties, ChildObjects — сиблинги под
    // `<<kind>>`); прочие виды таких узлов не эмитят (хойст — no-op). ns/local — критерий.
    let mut pre_property_blocks: Vec<OutElement> = Vec::new();
    let mut post_property_blocks: Vec<OutElement> = Vec::new();
    for child in body_sink.ordered(emit_order.as_deref()) {
        if child.prefix.is_empty() && child.local == "InternalInfo" {
            pre_property_blocks.push(child);
        } else if child.prefix.is_empty() && child.local == "ChildObjects" {
            post_property_blocks.push(child);
        } else {
            props.push(child);
        }
    }

    for block in pre_property_blocks {
        cm.push(block);
    }
    cm.push(props);
    for block in post_property_blocks {
        cm.push(block);
    }

    // --- child-objects: `<ChildObjects>`-обёртка ПОСЛЕ `<Properties>` (§3.4) ---
    // Designer оборачивает всех детей в единый `<ChildObjects>` (порядок = слоты
    // спека). Пуст детей ⇒ обёртки нет (в SSL все Enum имеют >=1 EnumValue).
    let child_nodes = children::write_children_named(
        &obj.children,
        spec,
        map,
        map.child_bindings(),
        target,
        /*emit_defaults=*/ true,
        &obj.name,
    )
    .map_err(|e| DesignerError::Engine(child_engine_err(e)))?;
    if !child_nodes.is_empty() {
        let mut child_objects = OutElement::branch("", "ChildObjects");
        for node in child_nodes {
            child_objects.push(node);
        }
        cm.push(child_objects);
    } else if map.emit_empty_child_container() {
        // Вид всегда несёт обёртку (Catalog): пустой → самозакрывающийся `<ChildObjects/>`.
        cm.push(OutElement::self_closing("", "ChildObjects"));
    }

    formats_xml::source_extensions::write_designer_refs(&obj.source_extensions, &mut cm);
    root.push(cm);

    Ok(render(&DESIGNER_ENVELOPE, &root))
}

/// Полное имя тега элемента (`prefix:local` или `local`) для диагностики.
fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}

/// Удобный фасад: прочитать `.xml`-байты CommonModule в IR.
pub fn read_common_module(bytes: &[u8]) -> Result<MetadataObject, DesignerError> {
    DesignerCommonModuleConnector.read(bytes)
}

/// Удобный фасад: записать CommonModule-IR в `.xml`-байты под SSL (byte-exact).
pub fn write_common_module(obj: &MetadataObject) -> Result<Vec<u8>, DesignerError> {
    DesignerCommonModuleConnector.write(obj)
}

/// Детект версии формата Designer-дескриптора по его байтам (корневой `version=`), БЕЗ
/// полного чтения свойств. Для round-trip: детектим версию входа → пишем той же версией
/// (`write_*_for`). Envelope-ошибка при неизвестной версии/битой обёртке (§1.0).
pub fn detect_designer_version(bytes: &[u8]) -> Result<FormatVersion, DesignerError> {
    let descriptor = parse(bytes)?;
    verify_root_envelope(&descriptor.root).map_err(DesignerError::Envelope)
}

/// Удобный фасад: записать CommonModule-IR в `.xml`-байты под ЗАДАННОЙ версией формата
/// (byte-exact для этой версии). Для round-trip разноверсионного входа.
pub fn write_common_module_for(
    obj: &MetadataObject,
    target: FormatVersion,
) -> Result<Vec<u8>, DesignerError> {
    write_descriptor_versioned(
        KIND_COMMON_MODULE,
        common_module(),
        &DesignerCommonModule,
        obj,
        target,
    )
}

/// Индекс первого различающегося байта (или хвост короче) для отладки R-гейта.
pub fn first_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    let n = a.len().min(b.len());
    for i in 0..n {
        if a[i] != b[i] {
            return Some(i);
        }
    }
    if a.len() != b.len() {
        Some(n)
    } else {
        None
    }
}
