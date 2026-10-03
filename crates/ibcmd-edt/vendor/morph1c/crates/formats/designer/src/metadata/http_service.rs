//! Designer-проекция вида `HTTPService` + рекурсивных детей `URLTemplate` → `Method`
//! (зеркало `core/spec/metadata/http_service*.rs`, ARCHITECTURE.md §5; child-objects
//! substrate). ТОЛЬКО размещение/кодировка — канонику держит
//! `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ
//! IR (§3.5), включая `children` рекурсивно (два уровня).
//!
//! Доказывает РЕКУРСИЮ на Designer: дети — полные сущности в ВЛОЖЕННЫХ `<ChildObjects>`
//! (`<HTTPService>…<ChildObjects><URLTemplate uuid><Properties>…<ChildObjects><Method
//! uuid><Properties>…`). Идентичность в `<Properties>` (props_wrapped). Рекурсию ведёт
//! `formats_xml::children`.
//!
//! РОДИТЕЛЬ `HTTPService` (пути от корня через `<HTTPService>/<Properties>`, DENSE):
//! `synonym`→`<Synonym>` (LocalizedV8); `comment`→`<Comment>` (PlainText, дефолт `""`→
//! self-closing); `rootURL`→`<RootURL>` (PlainText); `reuseSessions`→`<ReuseSessions>`
//! (EnumText); `sessionMaxAge`→`<SessionMaxAge>` (IntText).
//!
//! `URLTemplate` (recursion-узел): `synonym`/`comment`/`template`; коллекция `Method`→
//! вложенный `<ChildObjects><Method>`. `Method` (лист): `synonym`/`comment`/`httpMethod`
//! (EnumText, DENSE эмитит `GET`)/`handler`.
//!
//! Дети — БЕЗ `HARNESS_ENTRY` (покрыты транзитивно).

use formats_xml::children::ChildBinding;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::http_service::{
    F_COMMENT, F_REUSE_SESSIONS, F_ROOT_URL, F_SESSION_MAX_AGE, F_SYNONYM,
};
use morph1c_core::spec::metadata::http_service_url_template::{
    http_service_url_template, F_COMMENT as UT_COMMENT, F_SYNONYM as UT_SYNONYM,
    F_TEMPLATE as UT_TEMPLATE,
};
use morph1c_core::spec::metadata::http_service_url_template_method::{
    http_service_url_template_method, F_COMMENT as M_COMMENT, F_HANDLER as M_HANDLER,
    F_HTTP_METHOD as M_HTTP_METHOD, F_SYNONYM as M_SYNONYM,
};

/// `PropElement{path:[tag], ns:""}` (хелпер таблицы; Designer-теги без префикса).
const fn elem(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

// --- Метод (лист): локусы относительны `<Properties>` метода (props_wrapped=true) ---
const M_P_SYNONYM: &[&str] = &["Synonym"];
const M_P_COMMENT: &[&str] = &["Comment"];
const M_P_HTTP_METHOD: &[&str] = &["HTTPMethod"];
const M_P_HANDLER: &[&str] = &["Handler"];

/// Designer-проекция child-вида `HTTPService.URLTemplate.Method` (лист).
pub struct DesignerMethod;

impl LocusMap for DesignerMethod {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == M_SYNONYM {
            Some(FieldProjection::new(elem(M_P_SYNONYM), Codec::LocalizedV8))
        } else if field == M_COMMENT {
            Some(FieldProjection::new(elem(M_P_COMMENT), Codec::PlainText))
        } else if field == M_HTTP_METHOD {
            Some(FieldProjection::new(elem(M_P_HTTP_METHOD), Codec::EnumText))
        } else if field == M_HANDLER {
            Some(FieldProjection::new(elem(M_P_HANDLER), Codec::PlainText))
        } else {
            None
        }
    }
}

// --- URLTemplate (recursion-узел): локусы относительны `<Properties>` шаблона ---
const UT_P_SYNONYM: &[&str] = &["Synonym"];
const UT_P_COMMENT: &[&str] = &["Comment"];
const UT_P_TEMPLATE: &[&str] = &["Template"];

/// Designer-проекция child-вида `HTTPService.URLTemplate` (recursion-узел).
pub struct DesignerUrlTemplate;

impl LocusMap for DesignerUrlTemplate {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == UT_SYNONYM {
            Some(FieldProjection::new(elem(UT_P_SYNONYM), Codec::LocalizedV8))
        } else if field == UT_COMMENT {
            Some(FieldProjection::new(elem(UT_P_COMMENT), Codec::PlainText))
        } else if field == UT_TEMPLATE {
            Some(FieldProjection::new(elem(UT_P_TEMPLATE), Codec::PlainText))
        } else {
            None
        }
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // Designer: внуки — `<ChildObjects><Method uuid><Properties><Name>…`
            // ПОД элементом `<URLTemplate>` (container относителен этого элемента).
            "Method" => Some(ChildLocus {
                container: &["ChildObjects"],
                child_tag: "Method",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        method_bindings()
    }
}

/// `&'static` бинды внук-вида `Method` (кэш на процесс).
fn method_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "Method",
            child_spec: http_service_url_template_method(),
            child_map: &DesignerMethod,
        }]
    })
}

// --- Родитель HTTPService: пути от корня через <HTTPService>/<Properties> ---
const P_SYNONYM: &[&str] = &["HTTPService", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["HTTPService", "Properties", "Comment"];
const P_ROOT_URL: &[&str] = &["HTTPService", "Properties", "RootURL"];
const P_REUSE_SESSIONS: &[&str] = &["HTTPService", "Properties", "ReuseSessions"];
const P_SESSION_MAX_AGE: &[&str] = &["HTTPService", "Properties", "SessionMaxAge"];

/// Карта проекции Designer для родителя `HTTPService`.
pub struct DesignerHttpService;

impl LocusMap for DesignerHttpService {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == F_SYNONYM {
            Some(FieldProjection::new(elem(P_SYNONYM), Codec::LocalizedV8))
        } else if field == F_COMMENT {
            Some(FieldProjection::new(elem(P_COMMENT), Codec::PlainText))
        } else if field == F_ROOT_URL {
            Some(FieldProjection::new(elem(P_ROOT_URL), Codec::PlainText))
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
            // Designer: `<HTTPService><ChildObjects><URLTemplate uuid><Properties>…`.
            "URLTemplate" => Some(ChildLocus {
                container: &["HTTPService", "ChildObjects"],
                child_tag: "URLTemplate",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        url_template_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // Designer-HTTPService ВСЕГДА несёт обёртку `<ChildObjects>`: без URL-шаблонов →
        // самозакрывающийся `<ChildObjects/>` (сверено по coverage-корпусу).
        true
    }
}

/// `&'static` бинды child-вида `URLTemplate` (кэш на процесс).
fn url_template_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding {
            collection: "URLTemplate",
            child_spec: http_service_url_template(),
            child_map: &DesignerUrlTemplate,
        }]
    })
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::http_service::http_service;

/// R-read для харнесса: `.xml`-байты HTTPService → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("HTTPService", http_service(), &DesignerHttpService, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR HTTPService → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("HTTPService", http_service(), &DesignerHttpService, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/HTTPService. Дети (`URLTemplate`/`Method`) покрыты
/// ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "HTTPService",
    read,
    write,
    corpus_subpath: "coverage/designer/s5_reports_services/HTTPServices",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
