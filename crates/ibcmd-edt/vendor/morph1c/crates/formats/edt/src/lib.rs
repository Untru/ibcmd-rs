//! `formats-edt` — коннектор EDT (`.mdo` исходники): Reader/Writer дескриптора
//! объекта метаданных вокруг канонического IR (ARCHITECTURE.md §2.1), byte-exact на
//! записи (§3.2). Раскладка зеркалит `core/spec` (§5): `metadata/<вид>`, `common/`.
//!
//! Контракт среза F2b-2a (вид `CommonModule`):
//! * Reader фреймит идентичность объекта (`kind`, `name`-элемент, корневой `@uuid`)
//!   в [`MetadataObject`], затем драйвит `core::engine::read(common_module(), …)`
//!   через общий XML-субстрат → канонический разрежённый IR 10 свойств.
//! * Writer регенерирует `.mdo` БАЙТ-В-БАЙТ из [`MetadataObject`]: envelope EDT
//!   (no-BOM, CRLF, 2-space), корень `mdclass:CommonModule` с `xmlns:mdclass`+`uuid`,
//!   `<name>`, далее дети в каноническом порядке спека.
//!
//! Тело модуля `Module.bsl` — ВНЕ дескриптора (отдельный артефакт/слайс): ридер его
//! не читает, `MetadataObject.modules` остаётся пустым. Это НЕ нарушение §1.0: тело —
//! не несконсуменный фрагмент ДЕСКРИПТОРА, а отдельный файл.

pub mod common;
pub mod metadata;

use common::{
    EDT_ENVELOPE, MDCLASS_NS_URI, MDCLASS_PREFIX, NAME_ELEMENT, THIS_NODE_ATTR, UUID_ATTR,
    XMLNS_MDCLASS_ATTR,
};
use metadata::common_module::EdtCommonModule;

use formats_xml::emit::render;
use formats_xml::{
    children, parse, produced_types, LocusMap, OutElement, XmlProjection, XmlReadError,
    XmlSink, XmlSource,
};
use morph1c_core::engine::{self, EngineError};
use morph1c_core::ir::{MetadataObject, ObjectKind, Uuid};
use morph1c_core::spec::common::EntitySpec;
use morph1c_core::spec::metadata::common_module::common_module;
use morph1c_core::version::SSL;

/// Канонический код вида объекта EDT `mdclass:CommonModule`.
pub const KIND_COMMON_MODULE: &str = "CommonModule";

/// Ошибка коннектора EDT (типизированная, §1.0 — без best-effort/skip).
#[derive(Debug)]
pub enum EdtError {
    /// Сбой токенизации/структуры XML.
    Xml(XmlReadError),
    /// Дескриптор не соответствует ожидаемой EDT-обёртке (корень/ns/пролог/идентичность).
    Envelope(String),
    /// Ошибка spec-driven движка (несконсуменное, mismatch вида, …).
    Engine(EngineError),
    /// Сбой сериализации UTF-8 при чтении значения.
    Encoding(String),
}

impl std::fmt::Display for EdtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdtError::Xml(e) => write!(f, "edt: {e}"),
            EdtError::Envelope(s) => write!(f, "edt: envelope mismatch: {s}"),
            EdtError::Engine(e) => write!(f, "edt: {e}"),
            EdtError::Encoding(s) => write!(f, "edt: encoding: {s}"),
        }
    }
}

impl std::error::Error for EdtError {}

impl From<XmlReadError> for EdtError {
    fn from(e: XmlReadError) -> Self {
        EdtError::Xml(e)
    }
}
impl From<EngineError> for EdtError {
    fn from(e: EngineError) -> Self {
        EdtError::Engine(e)
    }
}

/// Распарсить hex-uuid (`8-4-4-4-12`, lower-case) в 16 байт. Любая иная форма —
/// ошибка (§1.0: не best-effort). Возвращает [`Uuid`].
fn parse_uuid(s: &str) -> Result<Uuid, EdtError> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(EdtError::Envelope(format!("malformed uuid {s:?}")));
    }
    let mut bytes = [0u8; 16];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
            .map_err(|e| EdtError::Envelope(format!("uuid hex: {e}")))?;
    }
    Ok(Uuid(bytes))
}

/// Форматировать 16 байт обратно в каноническую hex-строку uuid (lower-case,
/// дефисы `8-4-4-4-12`) — ровно как пишет EDT.
fn format_uuid(uuid: &Uuid) -> String {
    let b = &uuid.0;
    let h = |r: &[u8]| -> String { r.iter().map(|x| format!("{x:02x}")).collect() };
    format!(
        "{}-{}-{}-{}-{}",
        h(&b[0..4]),
        h(&b[4..6]),
        h(&b[6..8]),
        h(&b[8..10]),
        h(&b[10..16]),
    )
}

/// EDT-коннектор для одного вида (F2b: `CommonModule`).
///
/// Симметрия §2.1: один тип несёт и [`read`](EdtCommonModuleConnector::read), и
/// [`write`](EdtCommonModuleConnector::write) вокруг общего IR.
pub struct EdtCommonModuleConnector;

impl Default for EdtCommonModuleConnector {
    fn default() -> Self {
        Self
    }
}

impl EdtCommonModuleConnector {
    /// Прочитать дескриптор `.mdo` (байты) в канонический [`MetadataObject`].
    pub fn read(&self, bytes: &[u8]) -> Result<MetadataObject, EdtError> {
        read_descriptor(KIND_COMMON_MODULE, common_module(), &EdtCommonModule, bytes)
    }

    /// Регенерировать `.mdo` БАЙТ-В-БАЙТ из канонического [`MetadataObject`].
    pub fn write(&self, obj: &MetadataObject) -> Result<Vec<u8>, EdtError> {
        write_descriptor(KIND_COMMON_MODULE, common_module(), &EdtCommonModule, obj)
    }
}

/// Прочитать EDT-дескриптор ЛЮБОГО вида в канонический [`MetadataObject`]
/// (kind-параметризованный путь — общий для всех metadata-видов EDT).
///
/// Шаги: токенизация → сверка envelope (BOM/EOL/пролог) → корень `mdclass:<kind>` + ns
/// → фрейминг идентичности (`<name>`, корневой `@uuid`) + claim каркасных узлов →
/// движок читает свойства по спеку → §1.0-проверка тотальности (движок: `leftover==0`).
///
/// `kind` — local-name корня (= канонический код вида); `spec` — его канонический
/// [`EntitySpec`] (`core/spec`); `map` — EDT-проекция этого вида (`metadata/<kind>.rs`).
/// Каноника (id/порядок/дефолты/нормализация) — за спеком, не здесь (§1.6).
pub fn read_descriptor<M: LocusMap>(
    kind: &str,
    spec: &EntitySpec,
    map: &M,
    bytes: &[u8],
) -> Result<MetadataObject, EdtError> {
    let descriptor = parse(bytes)?;

    // Lexical BOM/EOL/declaration spelling is retained by adapter provenance.
    // The bounded adapter validates UTF-8/XML; semantic namespace/version guards remain.

    let root = descriptor.root;

    // --- envelope: корень mdclass:<kind> + ns ---
    if root.prefix != MDCLASS_PREFIX || root.local != kind {
        return Err(EdtError::Envelope(format!(
            "unexpected root <{}:{}>, want <{MDCLASS_PREFIX}:{kind}>",
            root.prefix, root.local
        )));
    }
    // Каркас claim'ит сам узел корня (его атрибуты/детей claim'им по отдельности
    // ниже и в движке); иначе корень попал бы в `leftover` (§1.0).
    root.claim();

    // Пер-вид доп. ns-объявления корня (напр. EDT Enum: xmlns:xsi+xmlns:core, нужные
    // const-блоку standardAttributes). Claim+сверка URI; отсутствие → ошибка ЛИБО
    // допустимо для видов с УСЛОВНЫМ объявлением (DataProcessor — ns лишь при наличии
    // Attribute/TabularSection; на чтении claim'им, если присутствуют). Иной URI → ошибка.
    // Доп. ns корня (xsi/core) — declare-iff-used (см. write_descriptor): EDT объявляет их
    // РОВНО когда тело использует `xsi:type`/`core:…`. Поэтому на чтении их отсутствие
    // ВСЕГДА допустимо (не ошибка) — claim'им лишь присутствующие (сверив URI), чтобы они
    // не попали в `leftover` (§1.0). Byte-exact round-trip держит write-скан тела.
    // Хвостовые доп. ns (после `xmlns:mdclass`, напр. `xmlns:mdclassExtension` корня
    // расширения) claim'ятся тем же правилом — позиция значима лишь на записи.
    for (attr_name, want_uri) in map
        .root_extra_namespaces()
        .iter()
        .chain(map.root_extra_namespaces_trailing())
    {
        if let Some(a) = root.attr(attr_name) {
            if a.value != *want_uri {
                return Err(EdtError::Envelope(format!(
                    "root {attr_name} = {:?}, want {want_uri:?}",
                    a.value
                )));
            }
            a.claimed.set(true);
        }
    }

    // Каркасные атрибуты корня: xmlns:mdclass (claim+сверка) и uuid (claim+забор).
    let ns = root
        .attr(XMLNS_MDCLASS_ATTR)
        .ok_or_else(|| EdtError::Envelope(format!("missing {XMLNS_MDCLASS_ATTR}")))?;
    if ns.value != MDCLASS_NS_URI {
        return Err(EdtError::Envelope(format!(
            "{XMLNS_MDCLASS_ATTR} = {:?}, want {MDCLASS_NS_URI:?}",
            ns.value
        )));
    }
    ns.claimed.set(true);

    let uuid_attr = root
        .attr(UUID_ATTR)
        .ok_or_else(|| EdtError::Envelope("missing root @uuid".into()))?;
    let uuid = parse_uuid(&uuid_attr.value)?;
    uuid_attr.claimed.set(true);

    // Опц. root-атрибут `thisNode` (узел-идентичность плана обмена). Claim+забор, если
    // присутствует; прочие виды его не несут (тогда `None`).
    let this_node = match root.attr(THIS_NODE_ATTR) {
        Some(a) => {
            let u = parse_uuid(&a.value)?;
            a.claimed.set(true);
            Some(u)
        }
        None => None,
    };

    // Каркасный элемент <name> (идентичность объекта, НЕ из спека). Без префикса.
    let name_el = root
        .child(NAME_ELEMENT)
        .ok_or_else(|| EdtError::Envelope("missing <name> element".into()))?;
    if !name_el.prefix.is_empty() {
        return Err(EdtError::Envelope(format!(
            "<name> must be unprefixed, got prefix {:?}",
            name_el.prefix
        )));
    }
    // <name> не должен нести атрибутов/детей — чистый текстовый лист.
    if !name_el.attrs.is_empty() || !name_el.children.is_empty() {
        return Err(EdtError::Envelope(
            "<name> must be a plain text leaf".into(),
        ));
    }
    let name = name_el.text.clone();
    // Каркас клеймит и узел, и текст <name> (значение идентичности), иначе текст
    // попал бы в leftover (§1.0 после фикса B1: непустой текст обязан быть claimed).
    name_el.claim_with_text();

    // --- каркас: платформенный блок <producedTypes> (если есть) ---
    // Структурно вне спек-региона (идёт ДО <name>), поэтому фреймится здесь, как
    // идентичность, а не как спек-свойство. Claim'им всё поддерево → leftover==0.
    // Категории type-id (Container/Ref/List/Manager) — по виду (общий каркас §3.5).
    // Ключ реестра категорий — КАНОН-вид `spec.entity`, не on-disk имя корня `kind`: для
    // top-level видов они совпадают, а standalone-дескриптор ДОЧЕРНЕГО вида (EDS-таблица:
    // корень `Table`, спек `ExternalDataSource.Table`) резолвится только по entity.
    let internal_info =
        produced_types::read_edt(&root, spec.entity).map_err(EdtError::Envelope)?;

    // --- child-objects: рекурсивное чтение подчинённых коллекций (§3.4) ---
    // ВАЖЕН ПОРЯДОК: `engine::read` (ниже) первым делом проверяет тотальность корня
    // (`leftover==0` по ВСЕМУ дереву <mdclass:…>), включая inline-дети `<enumValues>`,
    // которых НЕТ в parent-LocusMap. Поэтому детей читаем+claim'им ПЕРВЫМИ: каждый
    // ребёнок ПРОВЕРЯЕТ свою тотальность сам и claim'ит своё под-дерево в исходном
    // дереве. После этого parent-leftover увидит детей востребованными. (Лист-вид:
    // bindings/children пусты → no-op.)
    let extensions = formats_xml::source_extensions::read_edt(kind, &root).map_err(EdtError::Envelope)?;
    let source = XmlSource {
        root,
        fields: spec.fields().iter().map(|f| f.id).collect(),
    };
    let children = children::read_children_named(
        &source.root,
        spec,
        map,
        map.child_bindings(),
        SSL,
        /*emit_defaults=*/ false,
        &name,
    )
    .map_err(|e| EdtError::Engine(child_engine_err(e)))?;

    // --- spec-driven чтение свойств корня через общий XML-субстрат ---
    let proj = XmlProjection::new(map, SSL, /*emit_defaults=*/ false);
    let bag = engine::read(spec, &proj, &source)?;

    let mut obj = MetadataObject::new(ObjectKind::new(kind), name, uuid);
    obj.internal_info = internal_info;
    obj.this_node = this_node;
    obj.properties = bag;
    obj.children = children;
    obj.source_extensions = extensions;
    // modules/forms/templates остаются пустыми (вне дескриптора).
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

/// Регенерировать EDT-дескриптор `.mdo` ЛЮБОГО вида БАЙТ-В-БАЙТ из IR
/// (kind-параметризованный путь — общий для всех metadata-видов EDT).
///
/// НЕ эхо входа: дескриптор строится из IR+спека+envelope (§1.1). Корень
/// `mdclass:<kind>` получает `xmlns:mdclass` и `uuid`, затем `<name>`, затем движок
/// дописывает дочерние свойства в каноническом порядке спека (EDT-проекция дефолты
/// опускает).
pub fn write_descriptor<M: LocusMap>(
    kind: &str,
    spec: &EntitySpec,
    map: &M,
    obj: &MetadataObject,
) -> Result<Vec<u8>, EdtError> {
    if obj.kind.as_str() != kind {
        return Err(EdtError::Envelope(format!(
            "writer expects kind {kind}, got {}",
            obj.kind.as_str()
        )));
    }

    // Корень: prefix:local + каркасные атрибуты. Доп. ns корня (xsi/core) объявляются
    // УСЛОВНО — не по пер-видовому предикату, а по ФАКТУ использования: EDT объявляет
    // `xmlns:xsi`/`xmlns:core` РОВНО когда тело дескриптора их использует
    // (`xsi:type="core:…"` в minValue/std-attrs). Поэтому строим корень БЕЗ доп. ns,
    // собираем тело, затем (ниже, перед render) сканируем дерево и вставляем нужные ns
    // ПЕРЕД `xmlns:mdclass` (порядок эталона: xsi, core, mdclass, uuid). Так один общий
    // путь корректен для ВСЕХ видов — и минимальных (без ns), и полных (с ns) — byte-exact.
    let mut root = OutElement::branch(MDCLASS_PREFIX, kind)
        .attr(XMLNS_MDCLASS_ATTR, MDCLASS_NS_URI)
        .attr(UUID_ATTR, format_uuid(&obj.uuid));
    // `thisNode` — root-атрибут ПОСЛЕ `uuid` (платформенная идентичность плана обмена;
    // только у видов, несущих узел — `None` у прочих → не эмитим).
    if let Some(tn) = &obj.this_node {
        root = root.attr(THIS_NODE_ATTR, format_uuid(tn));
    }

    // <producedTypes> — ПЕРЕД <name> (структурная позиция эталона), если объект
    // несёт платформенный блок. Каркасный (как идентичность), не спек-свойство.
    if let Some(info) = &obj.internal_info {
        // Ключ — канон-вид `spec.entity` (см. read-сторону: standalone-дескриптор
        // дочернего вида резолвит категории только по entity).
        root.push(produced_types::write_edt(info, spec.entity).map_err(EdtError::Envelope)?);
    }

    // <name> — первый ребёнок ПОСЛЕ producedTypes (идентичность, перед спек-свойствами).
    root.push(OutElement::leaf("", NAME_ELEMENT, obj.name.clone()));

    // Движок дописывает спек-свойства в каноническом порядке.
    let proj = XmlProjection::new(map, SSL, /*emit_defaults=*/ false);
    let mut sink = XmlSink::default();
    engine::write(spec, &proj, &obj.properties, &mut sink)?;
    // ХВОСТОВЫЕ свойства КОРНЯ (физически ПОСЛЕ inline-детей; напр. EDT Subsystem
    // `<parentSubsystem>` после вложенных `<subsystems>`) отделяем ДО упорядочивания по
    // `order_tags` (1:1 с `children`) — зеркало `children::write_one`. Дефолт
    // `trailing_fields()==&[]` ⇒ ВСЕ прочие виды байт-идентичны (нет хвостовых листьев).
    let trailing_ids = map.trailing_fields();
    let (mut lead_sink, mut trailing_props) = (XmlSink::default(), Vec::new());
    for (el, id) in sink.children.into_iter().zip(sink.order_tags) {
        if trailing_ids.contains(&id) {
            trailing_props.push(el);
        } else {
            lead_sink.children.push(el);
            lead_sink.order_tags.push(id);
        }
    }
    for child in lead_sink.ordered(map.field_emit_order()) {
        root.push(child);
    }

    // --- child-objects: inline-дети ПОСЛЕ спек-свойств (EDT — прямо в корень) ---
    let child_nodes = children::write_children_named(
        &obj.children,
        spec,
        map,
        map.child_bindings(),
        SSL,
        /*emit_defaults=*/ false,
        &obj.name,
    )
    .map_err(|e| EdtError::Engine(child_engine_err(e)))?;
    for node in child_nodes {
        root.push(node);
    }
    // ХВОСТОВЫЕ свойства корня — ПОСЛЕ inline-детей (byte-exact позиция parentSubsystem).
    for node in trailing_props {
        root.push(node);
    }

    formats_xml::source_extensions::write_edt(&obj.source_extensions, &mut root).map_err(EdtError::Envelope)?;

    // Доп. ns корня (xsi/core) — declare-iff-used: объявляем РОВНО те, чей префикс тело
    // реально использует (`xsi:type`, `core:…`). Вставляем ПЕРЕД `xmlns:mdclass` (порядок
    // эталона). Так минимальный дескриптор их НЕ несёт, полный — несёт (byte-exact оба).
    let extra_ns: Vec<(String, String)> = map
        .root_extra_namespaces()
        .iter()
        .filter(|(attr, _)| tree_uses_prefix(&root, attr.strip_prefix("xmlns:").unwrap_or(attr)))
        .map(|(a, u)| ((*a).to_string(), (*u).to_string()))
        .collect();
    if !extra_ns.is_empty() {
        root.attrs.splice(0..0, extra_ns);
    }
    // ХВОСТОВЫЕ доп. ns — declare-iff-used, но ПОСЛЕ `xmlns:mdclass` (witnessed-порядок
    // корня расширения: `xsi, mdclass, mdclassExtension`; см. `root_extra_namespaces_trailing`).
    let trailing_ns: Vec<(String, String)> = map
        .root_extra_namespaces_trailing()
        .iter()
        .filter(|(attr, _)| tree_uses_prefix(&root, attr.strip_prefix("xmlns:").unwrap_or(attr)))
        .map(|(a, u)| ((*a).to_string(), (*u).to_string()))
        .collect();
    if !trailing_ns.is_empty() {
        let base = root
            .attrs
            .iter()
            .position(|(name, _)| name.as_str() == XMLNS_MDCLASS_ATTR)
            .expect("root always carries xmlns:mdclass (set above)");
        root.attrs.splice(base + 1..base + 1, trailing_ns);
    }

    Ok(render(&EDT_ENVELOPE, &root))
}

/// Использует ли поддерево `el` XML-префикс `prefix` (имя элемента, имя или ЗНАЧЕНИЕ
/// атрибута вида `prefix:...` — напр. `xsi:type` / `core:UndefinedValue`)? Основа
/// declare-iff-used для доп. ns корня EDT (xsi/core).
fn tree_uses_prefix(el: &OutElement, prefix: &str) -> bool {
    let is_qname = |s: &str| s.strip_prefix(prefix).is_some_and(|r| r.starts_with(':'));
    if el.prefix == prefix {
        return true;
    }
    if el
        .attrs
        .iter()
        .any(|(name, value)| is_qname(name) || is_qname(value))
    {
        return true;
    }
    el.children.iter().any(|c| tree_uses_prefix(c, prefix))
}

/// Удобный фасад: прочитать `.mdo`-байты CommonModule в IR.
pub fn read_common_module(bytes: &[u8]) -> Result<MetadataObject, EdtError> {
    EdtCommonModuleConnector.read(bytes)
}

/// Удобный фасад: записать CommonModule-IR в `.mdo`-байты (byte-exact).
pub fn write_common_module(obj: &MetadataObject) -> Result<Vec<u8>, EdtError> {
    EdtCommonModuleConnector.write(obj)
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
