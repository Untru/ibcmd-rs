//! Канонический спек ДОЧЕРНЕГО вида `IntegrationService.Channel` (канал сервиса
//! интеграции) — child-objects substrate, §1.1 (ребёнок —
//! это [`crate::ir::MetadataObject`] рекурсивно). Лист-вид (`children: &[]`), но
//! RECURSION-УЗЕЛ: несёт собственный `uuid`+`producedTypes`/`InternalInfo`(Manager).
//!
//! Dotted-kind (`"IntegrationService.Channel"`); stem ASCII snake_case; БЕЗ
//! `HARNESS_ENTRY` (покрыт ТРАНЗИТИВНО через родителя `IntegrationService`).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes`/`InternalInfo` — платформенный
//! каркас (одна категория `Manager`, per-kind [`PRODUCED_CATEGORIES`]).
//!
//! Метамодельные `objectBelonging`/`extendedConfigurationObject` в корпусе ERP
//! отсутствуют (n=0) → не включены (см. родителя).
//!
//! Поля (Designer DENSE-порядок `<Properties>`, сверено корпусом ERP, 4 канала edt+designer;
//! Designer DENSE — все present, EDT SPARSE — дефолты опущены):
//! * `synonym` — локализ. (default []);
//! * `comment` — свободный текст (default `""`);
//! * `externalIntegrationServiceChannelName` — внешнее имя канала (default `""`);
//! * `messageDirection` — направление `{Receive|Send}` (default `Send` — EDT омитит `Send`,
//!   сверено: output-каналы БЕЗ `<messageDirection>` в EDT ↔ Designer `Send`);
//! * `receiveMessageProcessing` — обработчик приёма (default `""`; EDT омитит пустой);
//! * `transactioned` — транзакционность (bool; default `false`).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ. Default = [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `externalIntegrationServiceChannelName` — Default = `""`.
pub const F_EXTERNAL_INTEGRATION_SERVICE_CHANNEL_NAME: FieldId = FieldId(3);
/// `messageDirection` — `{Receive|Send}`. Default = `Send` (EDT омитит).
pub const F_MESSAGE_DIRECTION: FieldId = FieldId(4);
/// `receiveMessageProcessing` — обработчик приёма. Default = `""`.
pub const F_RECEIVE_MESSAGE_PROCESSING: FieldId = FieldId(5);
/// `transactioned` — транзакционность (bool). Default = `false`.
pub const F_TRANSACTIONED: FieldId = FieldId(6);

/// Сконструировать [`FieldSpec`] канала в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(
            F_EXTERNAL_INTEGRATION_SERVICE_CHANNEL_NAME,
            "externalIntegrationServiceChannelName",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
        FieldSpec::with_default(
            F_MESSAGE_DIRECTION,
            "messageDirection",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Send")),
        ),
        FieldSpec::with_default(
            F_RECEIVE_MESSAGE_PROCESSING,
            "receiveMessageProcessing",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
        FieldSpec::with_default(F_TRANSACTIONED, "transactioned", ValueKind::Bool, PropertyValue::Bool(false)),
    ]
}

/// `&'static EntitySpec` вида `IntegrationService.Channel` (кэш на процесс).
pub fn integration_service_channel() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "IntegrationService.Channel",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// вида В КАНОНИЧЕСКОМ ПОРЯДКЕ IR (= EDT-порядок) — §1.6/§3.5, R2 data-drive.
// Читается обобщённым кодеком `formats_xml::produced_types` через реестр
// (`morph1c_core::spec::produced_categories_for`); build.rs подхватывает этот
// `pub const` по имени БЕЗ правки общих файлов (parallel-safe, зеркало R1).
// ============================================================================
/// Категории producedTypes вида `IntegrationService.Channel` в каноническом порядке IR.
/// Одна категория `Manager` (сверено корпусом: EDT `<managerType>`, Designer
/// `IntegrationServiceChannelManager.<Parent>.<Chan>` category `Manager`).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[crate::spec::common::ProducedCategory {
    category: "Manager",
    edt_tag: "managerType",
    designer_category: "Manager",
    designer_type_name: "IntegrationServiceChannelManager",
    designer_order: 0,
}];
