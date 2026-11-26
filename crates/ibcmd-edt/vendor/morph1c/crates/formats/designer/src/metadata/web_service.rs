//! Designer-проекция вида `WebService` + рекурсивных детей `Operation` → `Parameter`
//! (зеркало `core/spec/metadata/web_service*.rs`, ARCHITECTURE.md §5; child-objects
//! substrate). ТОЛЬКО размещение/кодировка — канонику держит
//! `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ
//! IR (§3.5), включая `children` рекурсивно (два уровня).
//!
//! Доказывает РЕКУРСИЮ на Designer: дети — полные сущности в ВЛОЖЕННЫХ `<ChildObjects>`
//! (`<WebService>…<ChildObjects><Operation uuid><Properties>…<ChildObjects><Parameter
//! uuid><Properties>…`). Идентичность в `<Properties>` (props_wrapped). Рекурсию ведёт
//! `formats_xml::children`.
//!
//! РОДИТЕЛЬ `WebService` (пути от корня через `<WebService>/<Properties>`, DENSE):
//! `synonym`→`<Synonym>` (LocalizedV8); `comment`→`<Comment>` (PlainText, дефолт `""`→
//! self-closing); `namespace`→`<Namespace>` (PlainText); `xdtoPackages`→`<XDTOPackages>`
//! (XdtoPackages — контейнер `<xr:Item>`'ов, пустой → self-closing);
//! `descriptorFileName`→`<DescriptorFileName>` (PlainText); `reuseSessions`→
//! `<ReuseSessions>` (EnumText, DENSE эмитит `DontUse`); `sessionMaxAge`→`<SessionMaxAge>`
//! (IntText).
//!
//! `Operation` (recursion-узел): `synonym`/`comment`; `xdtoReturningValueType`→
//! `<XDTOReturningValueType>` (XdtoTypeRef — QName-лист); `nillable`/`transactioned`→
//! `<Nillable>`/`<Transactioned>` (BoolText, DENSE эмитит `false`); `procedureName`/
//! `dataLockControlMode`; коллекция `Parameter`→вложенный `<ChildObjects><Parameter>`.
//! `Parameter` (лист): `synonym`/`comment`/`xdtoValueType` (XdtoTypeRef)/`nillable`
//! (BoolText)/`transferDirection` (EnumText, DENSE эмитит `In`).
//!
//! Дети — БЕЗ `HARNESS_ENTRY` (покрыты транзитивно).

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

/// `PropElement{path:[tag], ns:""}` (хелпер таблицы; Designer property-теги без префикса).
const fn elem(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

// --- Parameter (лист): локусы относительны `<Properties>` параметра (props_wrapped) ---
const PR_P_SYNONYM: &[&str] = &["Synonym"];
const PR_P_COMMENT: &[&str] = &["Comment"];
const PR_P_TYPE: &[&str] = &["XDTOValueType"];
const PR_P_NILLABLE: &[&str] = &["Nillable"];
const PR_P_DIR: &[&str] = &["TransferDirection"];

/// Designer-проекция child-вида `WebService.Operation.Parameter` (лист).
pub struct DesignerParameter;

impl LocusMap for DesignerParameter {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == PR_SYNONYM {
            Some(FieldProjection::new(elem(PR_P_SYNONYM), Codec::LocalizedV8))
        } else if field == PR_COMMENT {
            Some(FieldProjection::new(elem(PR_P_COMMENT), Codec::PlainText))
        } else if field == PR_TYPE {
            Some(FieldProjection::new(elem(PR_P_TYPE), Codec::XdtoTypeRef(XdtoTypeRefDialect::DesignerAtDepth(8))))
        } else if field == PR_NILLABLE {
            Some(FieldProjection::new(elem(PR_P_NILLABLE), Codec::BoolText))
        } else if field == PR_DIR {
            Some(FieldProjection::new(elem(PR_P_DIR), Codec::EnumText))
        } else {
            None
        }
    }
}

// --- Operation (recursion-узел): локусы относительны `<Properties>` операции ---
const OP_P_SYNONYM: &[&str] = &["Synonym"];
const OP_P_COMMENT: &[&str] = &["Comment"];
const OP_P_RET: &[&str] = &["XDTOReturningValueType"];
const OP_P_NILLABLE: &[&str] = &["Nillable"];
const OP_P_TRANSACTIONED: &[&str] = &["Transactioned"];
const OP_P_PROC: &[&str] = &["ProcedureName"];
const OP_P_DLCM: &[&str] = &["DataLockControlMode"];

/// Designer-проекция child-вида `WebService.Operation` (recursion-узел).
pub struct DesignerOperation;

impl LocusMap for DesignerOperation {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == OP_SYNONYM {
            Some(FieldProjection::new(elem(OP_P_SYNONYM), Codec::LocalizedV8))
        } else if field == OP_COMMENT {
            Some(FieldProjection::new(elem(OP_P_COMMENT), Codec::PlainText))
        } else if field == OP_RET {
            Some(FieldProjection::new(elem(OP_P_RET), Codec::XdtoTypeRef(XdtoTypeRefDialect::Designer)))
        } else if field == OP_NILLABLE {
            Some(FieldProjection::new(elem(OP_P_NILLABLE), Codec::BoolText))
        } else if field == OP_TRANSACTIONED {
            Some(FieldProjection::new(elem(OP_P_TRANSACTIONED), Codec::BoolText))
        } else if field == OP_PROC {
            Some(FieldProjection::new(elem(OP_P_PROC), Codec::PlainText))
        } else if field == OP_DLCM {
            Some(FieldProjection::new(elem(OP_P_DLCM), Codec::EnumText))
        } else {
            None
        }
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Designer: внуки — `<ChildObjects><Parameter uuid><Properties><Name>…`
            // ПОД элементом `<Operation>` (container относителен этого элемента).
            "Parameter" => Some(ChildLocus {
                container: &["ChildObjects"],
                child_tag: "Parameter",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        parameter_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // Designer `Operation` ВСЕГДА несёт обёртку `<ChildObjects>`: без параметров →
        // самозакрывающийся `<ChildObjects/>` (сверено: 14/143 операций без параметров).
        true
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
            child_map: &DesignerParameter,
        }]
    })
}

// --- Родитель WebService: пути от корня через <WebService>/<Properties> ---
const P_SYNONYM: &[&str] = &["WebService", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["WebService", "Properties", "Comment"];
const P_NAMESPACE: &[&str] = &["WebService", "Properties", "Namespace"];
const P_XDTO_PACKAGES: &[&str] = &["WebService", "Properties", "XDTOPackages"];
const P_DESCRIPTOR: &[&str] = &["WebService", "Properties", "DescriptorFileName"];
const P_REUSE_SESSIONS: &[&str] = &["WebService", "Properties", "ReuseSessions"];
const P_SESSION_MAX_AGE: &[&str] = &["WebService", "Properties", "SessionMaxAge"];

/// Карта проекции Designer для родителя `WebService`.
pub struct DesignerWebService;

impl LocusMap for DesignerWebService {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == F_SYNONYM {
            Some(FieldProjection::new(elem(P_SYNONYM), Codec::LocalizedV8))
        } else if field == F_COMMENT {
            Some(FieldProjection::new(elem(P_COMMENT), Codec::PlainText))
        } else if field == F_NAMESPACE {
            Some(FieldProjection::new(elem(P_NAMESPACE), Codec::PlainText))
        } else if field == F_XDTO_PACKAGES {
            Some(FieldProjection::new(
                elem(P_XDTO_PACKAGES),
                Codec::XdtoPackages(XdtoPackagesDialect::Designer),
            ))
        } else if field == F_DESCRIPTOR_FILE_NAME {
            Some(FieldProjection::new(elem(P_DESCRIPTOR), Codec::PlainText))
        } else if field == F_REUSE_SESSIONS {
            Some(FieldProjection::new(elem(P_REUSE_SESSIONS), Codec::EnumText))
        } else if field == F_SESSION_MAX_AGE {
            Some(FieldProjection::new(elem(P_SESSION_MAX_AGE), Codec::IntText))
        } else {
            None
        }
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Designer: `<WebService><ChildObjects><Operation uuid><Properties>…`.
            "Operation" => Some(ChildLocus {
                container: &["WebService", "ChildObjects"],
                child_tag: "Operation",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        operation_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // Designer-WebService ВСЕГДА несёт обёртку `<ChildObjects>`: без операций →
        // самозакрывающийся `<ChildObjects/>` (сверено по coverage-корпусу).
        true
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
            child_map: &DesignerOperation,
        }]
    })
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::web_service::web_service;

/// R-read для харнесса: `.xml`-байты WebService → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("WebService", web_service(), &DesignerWebService, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR WebService → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("WebService", web_service(), &DesignerWebService, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/WebService. Дети (`Operation`/`Parameter`) покрыты
/// ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "WebService",
    read,
    write,
    corpus_subpath: "coverage/designer/s5_reports_services/WebServices",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
