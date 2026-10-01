//! EDT-проекция вида `Subsystem` + дочерней self-reference коллекции `Subsystem.SubsystemRef`
//! (зеркало `core/spec/metadata/subsystem.rs`, ARCHITECTURE.md §5). ТОЛЬКО размещение/
//! кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)`:
//! * `synonym` → контейнер `<synonym>` с парами `<key>/<value>` (`LocalizedKeyVal`);
//! * `comment` → `<comment>текст</comment>` (`PlainText`; дефолт пустой — опущен);
//! * `includeHelpInContents` → `<includeHelpInContents>true</…>` (`BoolPresence`; дефолт опущен);
//! * `includeInCommandInterface` → `<includeInCommandInterface>true</…>` (`BoolPresence`; дефолт опущен);
//! * `useOneCommand` → `<useOneCommand>true</…>` (`BoolPresence`; дефолт опущен);
//! * `explanation` → контейнер `<explanation>` с парами `<key>/<value>` (`LocalizedKeyVal`; дефолт опущен);
//! * `picture` → `<picture xsi:type="core:PictureRef"><picture>ref</picture></picture>`
//!   (`PictureRef`; дефолт пустой опущен);
//! * `content` → сиблинги `<content>Path</content>` под КОРНЕМ (`RefList(Edt)`; пустой → опущен).
//!
//! Дочерняя коллекция `Subsystem` (вложенные подсистемы) — BARE-REF сиблинги
//! `<subsystems>Имя</subsystems>` под корнем (`container: []`, `bare_ref: true`).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок детей в
//! файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — лишь имена. `xsi`/`core`-ns
//! корня объявляются ЛИШЬ при непустом `<picture>` (сверено: 1/3 top-level несёт картинку).

use formats_xml::children::ChildBinding;
use formats_xml::ref_list::RefListDialect;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::subsystem::{
    F_COMMENT, F_CONTENT, F_EXPLANATION, F_HELP, F_INCLUDE_HELP_IN_CONTENTS,
    F_INCLUDE_IN_COMMAND_INTERFACE, F_PARENT_SUBSYSTEM, F_PICTURE, F_SYNONYM, F_USE_ONE_COMMAND,
};
use morph1c_core::spec::metadata::subsystem_subsystem_ref::subsystem_subsystem_ref;

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];
const P_INCLUDE_HELP: &[&str] = &["includeHelpInContents"];
const P_HELP: &[&str] = &["help"];
const P_INCLUDE_IN_CI: &[&str] = &["includeInCommandInterface"];
const P_USE_ONE_COMMAND: &[&str] = &["useOneCommand"];
const P_EXPLANATION: &[&str] = &["explanation"];
const P_PICTURE: &[&str] = &["picture"];
const P_CONTENT: &[&str] = &["content"];
const P_PARENT_SUBSYSTEM: &[&str] = &["parentSubsystem"];

/// Карта проекции EDT для `Subsystem`.
pub struct EdtSubsystem;

impl EdtSubsystem {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal)
        } else if field == F_COMMENT {
            FieldProjection::new(flat(P_COMMENT), Codec::PlainText)
        } else if field == F_INCLUDE_HELP_IN_CONTENTS {
            FieldProjection::new(flat(P_INCLUDE_HELP), Codec::BoolPresence)
        } else if field == F_HELP {
            // EDT-only `<help><pages><lang>ru</lang></pages></help>` — фикс const-блок
            // (тот же `HelpConst`, что у Catalog/InformationRegister). X-исключён (спек).
            FieldProjection::new(flat(P_HELP), Codec::HelpConst)
        } else if field == F_INCLUDE_IN_COMMAND_INTERFACE {
            FieldProjection::new(flat(P_INCLUDE_IN_CI), Codec::BoolPresence)
        } else if field == F_USE_ONE_COMMAND {
            FieldProjection::new(flat(P_USE_ONE_COMMAND), Codec::BoolPresence)
        } else if field == F_EXPLANATION {
            FieldProjection::new(flat(P_EXPLANATION), Codec::LocalizedKeyVal)
        } else if field == F_PICTURE {
            FieldProjection::new(flat(P_PICTURE), Codec::PictureRef(formats_xml::picture::PictureDialect::Edt))
        } else if field == F_CONTENT {
            FieldProjection::new(flat(P_CONTENT), Codec::RefList(RefListDialect::Edt))
        } else if field == F_PARENT_SUBSYSTEM {
            // EDT-only обратная ссылка `<parentSubsystem>Subsystem.Имя</parentSubsystem>`
            // (plain-text ref). Физически ПОСЛЕ вложенных `<subsystems>` → `trailing_fields`.
            // X-исключён (спек `x_ignore`).
            FieldProjection::new(flat(P_PARENT_SUBSYSTEM), Codec::PlainText)
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for EdtSubsystem {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Вложенные подсистемы — BARE-REF сиблинги `<subsystems>Имя</subsystems>` под
            // корнем (`container` пуст). Тело вложенной подсистемы — в отдельном файле.
            "Subsystem" => Some(ChildLocus {
                container: &[],
                child_tag: "subsystems",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: true,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        subsystem_bindings()
    }

    fn trailing_fields(&self) -> &'static [FieldId] {
        // `<parentSubsystem>` физически ПОСЛЕ вложенных `<subsystems>` (сверено корпусом:
        // Мультиязычность/ОбменДанными/РаботаВМоделиСервиса несут и детей, и parentSubsystem;
        // ссылка идёт последней). Эмитируется после inline-детей (см. write_descriptor).
        &[F_PARENT_SUBSYSTEM]
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }

    fn root_extra_namespaces_optional(&self) -> bool {
        // xsi+core объявляются на корне подсистемы ЛИШЬ при непустом <picture>
        // (его `xsi:type="core:PictureRef"` требует обоих ns). Сверено по SSL: 1/3
        // top-level несёт непустую картинку ⟺ 1/3 объявляет эти ns.
        true
    }

    fn root_extra_namespaces_present(&self, obj: &morph1c_core::ir::MetadataObject) -> bool {
        // ns нужны ⟺ picture-поле непусто (`Str` ≠ "").
        obj.properties.iter().any(|(id, v)| {
            *id == F_PICTURE
                && matches!(v, morph1c_core::ir::value::PropertyValue::Str(s) if !s.is_empty())
        })
    }
}

/// Бинды дочерних коллекций `Subsystem` (self-reference: bare-ref, без своей проекции-
/// карты полей — лист без полей; но `LocusMap` нужен для контракта). Кэш на процесс.
fn subsystem_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Subsystem",
            child_spec: subsystem_subsystem_ref(),
            child_map: &EdtSubsystemRef,
        }]
    })
}

/// EDT-проекция child-вида `Subsystem.SubsystemRef` (bare-ref: полей нет). БЕЗ
/// `HARNESS_ENTRY` (покрыт транзитивно через родителя).
pub struct EdtSubsystemRef;

impl LocusMap for EdtSubsystemRef {
    fn lookup(&self, _field: FieldId) -> Option<FieldProjection> {
        None
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::subsystem::subsystem;

/// R-read для харнесса: `.mdo`-байты Subsystem → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Subsystem", subsystem(), &EdtSubsystem, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Subsystem → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Subsystem", subsystem(), &EdtSubsystem, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/Subsystem. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Subsystem",
    read,
    write,
    corpus_subpath: "coverage/edt/s4_common/src/Subsystems",
    // РЕКУРСИВНАЯ раскладка: подсистемы вложены (`<parent>/Subsystems/<child>/<child>.mdo`),
    // поэтому плоский `DirPerObject` перечислил бы лишь 3 верхних из 87. `Nested` спускается
    // в подкаталог `Subsystems` каждого объекта → все 87 round-trip-ятся (R+X).
    layout: formats_xml::CorpusLayout::Nested {
        ext: "mdo",
        nesting_dir: "Subsystems",
        dir_per_object: true,
    },
};
