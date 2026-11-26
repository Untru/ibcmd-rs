//! EDT-проекция вида `HTTPService` + рекурсивных детей `URLTemplate` → `Method`
//! (зеркало `core/spec/metadata/http_service*.rs`, ARCHITECTURE.md §5; child-objects
//! substrate). ТОЛЬКО размещение/кодировка ячеек — канонику
//! (id/порядок/дефолты) держит `core/spec` (§1.6).
//!
//! Доказывает РЕКУРСИЮ субстрата (два уровня вложенных коллекций) на EDT: дети — inline
//! прямые элементы родителя (`<urlTemplates uuid><name>…<methods uuid><name>…`), плоская
//! идентичность (без `<Properties>`-обёртки). Рекурсию ведёт `formats_xml::children`.
//!
//! Карты `FieldId → (XmlLocus, Codec)` ВЫВОДЯТСЯ из спеков (`docs/APPROACH.md` §2.2): тег
//! = имя поля verbatim, кодек = по value_kind — для КАЖДОГО из трёх узлов (родитель +
//! два уровня детей). Локусы дочерних коллекций (`child_collection`/`child_bindings`) —
//! структурны (имена контейнеров `urlTemplates`/`methods`), задаются здесь, не деривацией.
//!
//! Дети — БЕЗ `HARNESS_ENTRY` (покрыты транзитивно через корень `HTTPService`).

use formats_xml::children::ChildBinding;
use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{ChildLocus, FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::http_service_url_template::http_service_url_template;
use morph1c_core::spec::metadata::http_service_url_template_method::http_service_url_template_method;

/// EDT-проекция child-вида `HTTPService.URLTemplate.Method` (лист) — деривация.
pub struct EdtMethod;

impl LocusMap for EdtMethod {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, http_service_url_template_method(), field)
    }
}

/// EDT-проекция child-вида `HTTPService.URLTemplate` (recursion-узел: коллекция Method).
pub struct EdtUrlTemplate;

impl LocusMap for EdtUrlTemplate {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, http_service_url_template(), field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // EDT: внуки — inline `<methods uuid><name>…` прямо под `<urlTemplates>`.
            "Method" => Some(ChildLocus {
                container: &[],
                child_tag: "methods",
                props_wrapped: false,
                name_tag: "name",
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
            child_map: &EdtMethod,
        }]
    })
}

/// Карта проекции EDT для родителя `HTTPService` — деривация.
pub struct EdtHttpService;

impl LocusMap for EdtHttpService {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, http_service(), field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            // EDT: дети — inline `<urlTemplates uuid><name>…` прямо под корнем.
            "URLTemplate" => Some(ChildLocus {
                container: &[],
                child_tag: "urlTemplates",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        url_template_bindings()
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
            child_map: &EdtUrlTemplate,
        }]
    })
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::http_service::http_service;

/// R-read для харнесса: `.mdo`-байты HTTPService → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("HTTPService", http_service(), &EdtHttpService, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR HTTPService → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("HTTPService", http_service(), &EdtHttpService, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/HTTPService. Дети (`URLTemplate`/`Method`) покрыты
/// ТРАНЗИТИВНО (R byte-exact ⇒ inline-дети byte-exact; X IR-равен ⇒ `children`
/// рекурсивно равны) — отдельной записи у них нет.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "HTTPService",
    read,
    write,
    corpus_subpath: "coverage/edt/s5_reports_services/src/HTTPServices",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
