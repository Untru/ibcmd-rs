//! EDT-проекция вида `WebService` + рекурсивных детей `Operation` → `Parameter`
//! (зеркало `core/spec/metadata/web_service*.rs`, ARCHITECTURE.md §5; child-objects
//! substrate). ТОЛЬКО размещение/кодировка ячеек — канонику
//! (id/порядок/дефолты) держит `core/spec` (§1.6).
//!
//! Доказывает РЕКУРСИЮ субстрата (два уровня вложенных коллекций) на EDT: дети — inline
//! прямые элементы родителя (`<operations uuid><name>…<parameters uuid><name>…`), плоская
//! идентичность (без `<Properties>`-обёртки). Рекурсию ведёт `formats_xml::children`.
//!
//! РОДИТЕЛЬ `WebService`: `synonym`→`<synonym>` (LocalizedKeyVal); `comment`→`<comment>`
//! (PlainText, EDT опускает дефолт); `namespace`→`<namespace>` (PlainText);
//! `xdtoPackages`→сиблинги `<xdtoPackages xsi:type>` (XdtoPackages, EDT multi-node);
//! `descriptorFileName`→`<descriptorFileName>` (PlainText); `reuseSessions`→
//! `<reuseSessions>` (EnumText, EDT sparse-опускает дефолт `DontUse`, эмитит `Use`/`AutoUse`);
//! `sessionMaxAge`→`<sessionMaxAge>` (IntText). Коллекция `Operation`→inline `<operations>`.
//!
//! `Operation` (recursion-узел): `synonym`/`comment`; `xdtoReturningValueType`→
//! `<xdtoReturningValueType>` (XdtoTypeRef — пара `name`+`nsUri`); `nillable`→
//! `<nillable>` (BoolPresence, EDT опускает `false`); `transactioned` — EDT ВСЕГДА
//! опускает (`x_ignore`); `procedureName`/`dataLockControlMode`. Коллекция `Parameter`→
//! inline `<parameters>`. `Parameter` (лист): `synonym`/`comment`/`xdtoValueType`
//! (XdtoTypeRef)/`nillable` (BoolPresence)/`transferDirection` (EnumText, EDT опускает `In`).
//!
//! Дети — БЕЗ `HARNESS_ENTRY` (покрыты транзитивно через корень `WebService`).

use formats_xml::children::ChildBinding;
use formats_xml::{
    ChildLocus, Codec, FieldProjection, LocusMap, XdtoPackagesDialect, XdtoTypeRefDialect, XmlLocus,
};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::web_service::{
    F_COMMENT, F_DESCRIPTOR_FILE_NAME, F_NAMESPACE, F_REUSE_SESSIONS, F_SESSION_MAX_AGE, F_SYNONYM,
    F_XDTO_PACKAGES,
};
use morph1c_core::spec::metadata::web_service_operation::{
    web_service_operation, F_COMMENT as OP_COMMENT, F_DATA_LOCK_CONTROL_MODE as OP_DLCM,
    F_NILLABLE as OP_NILLABLE, F_PROCEDURE_NAME as OP_PROC, F_SYNONYM as OP_SYNONYM,
    F_TRANSACTIONED as OP_TRANSACTIONED, F_XDTO_RETURNING_VALUE_TYPE as OP_RET,
};
use morph1c_core::spec::metadata::web_service_operation_parameter::{
    web_service_operation_parameter, F_COMMENT as PR_COMMENT, F_NILLABLE as PR_NILLABLE,
    F_SYNONYM as PR_SYNONYM, F_TRANSFER_DIRECTION as PR_DIR, F_XDTO_VALUE_TYPE as PR_TYPE,
};

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

// --- Parameter (лист): локусы относительны элемента `<parameters>` ------------
const PR_P_SYNONYM: &[&str] = &["synonym"];
const PR_P_COMMENT: &[&str] = &["comment"];
const PR_P_TYPE: &[&str] = &["xdtoValueType"];
const PR_P_NILLABLE: &[&str] = &["nillable"];
const PR_P_DIR: &[&str] = &["transferDirection"];

/// EDT-проекция child-вида `WebService.Operation.Parameter` (лист).
pub struct EdtParameter;

impl LocusMap for EdtParameter {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == PR_SYNONYM {
            Some(FieldProjection::new(flat(PR_P_SYNONYM), Codec::LocalizedKeyVal))
        } else if field == PR_COMMENT {
            Some(FieldProjection::new(flat(PR_P_COMMENT), Codec::PlainText))
        } else if field == PR_TYPE {
            Some(FieldProjection::new(flat(PR_P_TYPE), Codec::XdtoTypeRef(XdtoTypeRefDialect::Edt)))
        } else if field == PR_NILLABLE {
            Some(FieldProjection::new(flat(PR_P_NILLABLE), Codec::BoolPresence))
        } else if field == PR_DIR {
            Some(FieldProjection::new(flat(PR_P_DIR), Codec::EnumText))
        } else {
            None
        }
    }
}

// --- Operation (recursion-узел): локусы относительны элемента `<operations>` ---
const OP_P_SYNONYM: &[&str] = &["synonym"];
const OP_P_COMMENT: &[&str] = &["comment"];
const OP_P_RET: &[&str] = &["xdtoReturningValueType"];
const OP_P_NILLABLE: &[&str] = &["nillable"];
const OP_P_PROC: &[&str] = &["procedureName"];
const OP_P_DLCM: &[&str] = &["dataLockControlMode"];

/// EDT-проекция child-вида `WebService.Operation` (recursion-узел: коллекция Parameter).
pub struct EdtOperation;

impl LocusMap for EdtOperation {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == OP_SYNONYM {
            Some(FieldProjection::new(flat(OP_P_SYNONYM), Codec::LocalizedKeyVal))
        } else if field == OP_COMMENT {
            Some(FieldProjection::new(flat(OP_P_COMMENT), Codec::PlainText))
        } else if field == OP_RET {
            Some(FieldProjection::new(flat(OP_P_RET), Codec::XdtoTypeRef(XdtoTypeRefDialect::Edt)))
        } else if field == OP_NILLABLE {
            Some(FieldProjection::new(flat(OP_P_NILLABLE), Codec::BoolPresence))
        } else if field == OP_TRANSACTIONED {
            // EDT ВСЕГДА опускает `transactioned` (дефолт false), Designer DENSE эмитит →
            // формат-локальная асимметрия. EDT-проекция поле НЕ размещает (lookup=None ⇒
            // движок держит его дефолтным на R). X-равенство держит core-дефолт.
            None
        } else if field == OP_PROC {
            Some(FieldProjection::new(flat(OP_P_PROC), Codec::PlainText))
        } else if field == OP_DLCM {
            Some(FieldProjection::new(flat(OP_P_DLCM), Codec::EnumText))
        } else {
            None
        }
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // EDT: внуки — inline `<parameters uuid><name>…` прямо под `<operations>`.
            "Parameter" => Some(ChildLocus {
                container: &[],
                child_tag: "parameters",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        parameter_bindings()
    }
}

/// `&'static` бинды внук-вида `Parameter` (кэш на процесс).
fn parameter_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Parameter",
            child_spec: web_service_operation_parameter(),
            child_map: &EdtParameter,
        }]
    })
}

// --- Родитель WebService ------------------------------------------------------
const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];
const P_NAMESPACE: &[&str] = &["namespace"];
const P_XDTO_PACKAGES: &[&str] = &["xdtoPackages"];
const P_DESCRIPTOR: &[&str] = &["descriptorFileName"];
const P_REUSE_SESSIONS: &[&str] = &["reuseSessions"];
const P_SESSION_MAX_AGE: &[&str] = &["sessionMaxAge"];

/// Карта проекции EDT для родителя `WebService`.
pub struct EdtWebService;

impl LocusMap for EdtWebService {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == F_SYNONYM {
            Some(FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal))
        } else if field == F_COMMENT {
            Some(FieldProjection::new(flat(P_COMMENT), Codec::PlainText))
        } else if field == F_NAMESPACE {
            Some(FieldProjection::new(flat(P_NAMESPACE), Codec::PlainText))
        } else if field == F_XDTO_PACKAGES {
            Some(FieldProjection::new(flat(P_XDTO_PACKAGES), Codec::XdtoPackages(XdtoPackagesDialect::Edt)))
        } else if field == F_DESCRIPTOR_FILE_NAME {
            Some(FieldProjection::new(flat(P_DESCRIPTOR), Codec::PlainText))
        } else if field == F_REUSE_SESSIONS {
            // EDT .mdo несёт `reuseSessions` при значении != дефолт `DontUse` (sparse):
            // witnessed `Use`/`AutoUse` эмитятся, `DontUse` опускается (omitting-twin
            // `WebСервис_ПовторноеИспользованиеСеансов_НеИспользовать` его не несёт). Designer
            // DENSE эмитит всегда → X by construction (оба сжимают `DontUse` до дефолта).
            Some(FieldProjection::new(flat(P_REUSE_SESSIONS), Codec::EnumText))
        } else if field == F_SESSION_MAX_AGE {
            Some(FieldProjection::new(flat(P_SESSION_MAX_AGE), Codec::IntText))
        } else {
            None
        }
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // EDT: дети — inline `<operations uuid><name>…` прямо под корнем.
            "Operation" => Some(ChildLocus {
                container: &[],
                child_tag: "operations",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        operation_bindings()
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }

    fn root_extra_namespaces_optional(&self) -> bool {
        // xsi+core объявляются на корне Web-сервиса лишь при непустом `xdtoPackages`
        // (его элементы несут `xsi:type="core:StringValue|core:ReferenceValue"`).
        true
    }

    fn root_extra_namespaces_present(&self, obj: &morph1c_core::ir::MetadataObject) -> bool {
        // ns нужны ⟺ объект несёт ≥1 элемент `xdtoPackages`. Сверено по SSL-корпусу:
        // 10/13 объявляют ns ⟺ 10/13 несут xdtoPackages; 3/13 без пакетов — только
        // `xmlns:mdclass`. Иные edt-поля (`xdtoReturningValueType`/`xdtoValueType`) —
        // плоские `<name>/<nsUri>` без xsi, ns не требуют.
        obj.properties.iter().any(|(id, v)| {
            *id == F_XDTO_PACKAGES && matches!(v, morph1c_core::ir::value::PropertyValue::List(l) if !l.is_empty())
        })
    }
}

/// `&'static` бинды child-вида `Operation` (кэш на процесс).
fn operation_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Operation",
            child_spec: web_service_operation(),
            child_map: &EdtOperation,
        }]
    })
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::web_service::web_service;

/// R-read для харнесса: `.mdo`-байты WebService → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("WebService", web_service(), &EdtWebService, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR WebService → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("WebService", web_service(), &EdtWebService, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/WebService. Дети (`Operation`/`Parameter`) покрыты ТРАНЗИТИВНО
/// (R byte-exact ⇒ inline-дети byte-exact; X IR-равен ⇒ `children` рекурсивно равны).
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "WebService",
    read,
    write,
    corpus_subpath: "coverage/edt/s5_reports_services/src/WebServices",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
