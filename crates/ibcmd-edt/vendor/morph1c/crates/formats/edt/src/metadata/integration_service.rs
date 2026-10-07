//! EDT-проекция вида `IntegrationService` + его дочернего вида `Channel` (зеркало
//! `core/spec/metadata/integration_service*.rs`, ARCHITECTURE.md §5; child-objects
//! substrate). ТОЛЬКО размещение/кодировка ячеек — канонику
//! держит `core/spec` (§1.6).
//!
//! Карты `FieldId → (XmlLocus, Codec)` ВЫВОДЯТСЯ из спеков (`docs/APPROACH.md` §2.2): тег
//! = имя поля verbatim, кодек = по value_kind — для родителя И для ребёнка `Channel`.
//! РОДИТЕЛЬ — плоские дети корня `.mdo`. Дитя Channel — inline `<integrationServiceChannels
//! uuid>` (props_wrapped=false), recursion-узел с собственным `<producedTypes>`(managerType);
//! локусы коллекции (`child_collection`/`child_bindings`) — структурны, задаются здесь.

use formats_xml::children::ChildBinding;
use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{ChildLocus, FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::EntitySpec;

use morph1c_core::spec::metadata::integration_service::integration_service;
use morph1c_core::spec::metadata::integration_service_channel::integration_service_channel;

// ===== РОДИТЕЛЬ IntegrationService =====
pub struct EdtIntegrationService;

impl LocusMap for EdtIntegrationService {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, integration_service(), field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            "Channel" => Some(ChildLocus {
                container: &[],
                child_tag: "integrationServiceChannels",
                props_wrapped: false,
                name_tag: "name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        is_bindings()
    }
}

// ===== Дитя Channel (recursion-узел: producedTypes managerType) =====
pub struct EdtChannel;
impl LocusMap for EdtChannel {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Edt, integration_service_channel(), field)
    }
}

/// `&'static` бинды child-видов родителя (кэш на процесс).
fn is_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding { collection: "Channel", child_spec: integration_service_channel(), child_map: &EdtChannel }]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("IntegrationService", spec(), &EdtIntegrationService, bytes).map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("IntegrationService", spec(), &EdtIntegrationService, obj).map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    integration_service()
}

/// Строка R+X-харнесса EDT/IntegrationService. Дитя Channel покрыто ТРАНЗИТИВНО.
/// ERP-раскладка: `edt/cfg/src/…` (НЕ `edt/src/`).
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "IntegrationService",
    read,
    write,
    corpus_subpath: "coverage/edt/s5_reports_services/src/IntegrationServices",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
