//! Designer-проекция вида `IntegrationService` + его дочернего вида `Channel` (зеркало
//! `core/spec/metadata/integration_service*.rs`, ARCHITECTURE.md §5; child-objects
//! substrate). ТОЛЬКО размещение/кодировка — канонику держит
//! `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ спека, что EDT → оба формата дают РАВНЫЙ IR
//! (§3.5), включая ребёнка.
//!
//! Карты `FieldId → (XmlLocus, Codec)` ВЫВОДЯТСЯ из спеков (`docs/APPROACH.md` §2.2): тег
//! = `[Kind, "Properties", UpperCamel(имя)]` (родитель) / `[UpperCamel(имя)]` (ребёнок под
//! `<Properties>`), кодек = по value_kind. Дитя Channel — под
//! `<ChildObjects>/<IntegrationServiceChannel>/<Properties>` (props_wrapped=true),
//! recursion-узел (`<InternalInfo>` GeneratedType Manager); локусы коллекции — здесь.

use formats_xml::children::ChildBinding;
use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{ChildLocus, FieldProjection, LocusMap};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::common::EntitySpec;

use morph1c_core::spec::metadata::integration_service::integration_service;
use morph1c_core::spec::metadata::integration_service_channel::integration_service_channel;

// ===== РОДИТЕЛЬ IntegrationService =====
pub struct DesignerIntegrationService;

impl LocusMap for DesignerIntegrationService {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        projection(DeriveDialect::Designer, integration_service(), field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        match collection {
            "Channel" => Some(ChildLocus {
                container: &["IntegrationService", "ChildObjects"],
                child_tag: "IntegrationServiceChannel",
                props_wrapped: true,
                name_tag: "Name",
                bare_ref: false,
            }),
            _ => None,
        }
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        is_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // Designer-IntegrationService ВСЕГДА несёт обёртку `<ChildObjects>`: без каналов →
        // самозакрывающийся `<ChildObjects/>` (сверено по coverage-корпусу).
        true
    }
}

// ===== Дитя Channel (recursion-узел: InternalInfo GeneratedType Manager) =====
pub struct DesignerChannel;
impl LocusMap for DesignerChannel {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        // CHILD node: Designer locus is relative to the Channel's own `<Properties>`
        // (single-segment tag), not the top-level `[Kind,"Properties",Tag]` path.
        projection(DeriveDialect::DesignerChild, integration_service_channel(), field)
    }
}

/// `&'static` бинды child-видов родителя (кэш на процесс).
fn is_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static B: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    B.get_or_init(|| {
        vec![ChildBinding { collection: "Channel", child_spec: integration_service_channel(), child_map: &DesignerChannel }]
    })
}

// ===== Строка R+X-харнесса =====
use morph1c_core::ir::MetadataObject;

fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("IntegrationService", spec(), &DesignerIntegrationService, bytes)
        .map_err(|e| e.to_string())
}
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("IntegrationService", spec(), &DesignerIntegrationService, obj)
        .map_err(|e| e.to_string())
}
fn spec() -> &'static EntitySpec {
    integration_service()
}

/// Строка R+X-харнесса Designer/IntegrationService. Дитя Channel покрыто ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "IntegrationService",
    read,
    write,
    corpus_subpath: "coverage/designer/s5_reports_services/IntegrationServices",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
