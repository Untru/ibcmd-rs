//! Канонический спек ДОЧЕРНЕГО вида `SettingsStorage.TemplateRef` — REF/STUB-коллекция
//! макетов (child-objects substrate §3.2). Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! Метамодель: `SettingsStorage.templates: Template[*]`. SSL-корпус несёт 0 макетов
//! (cf-дескриптор витнессит ПУСТУЮ Template-коллекцию `{3daea016…,0}` — та же
//! коллекция-guid, что у DataProcessor/Report), поэтому форма стаба reuse-proven от
//! `InformationRegister.TemplateRef` (тот же метамодельный `Template`-лист): EDT-стаб
//! `<templates uuid><name><synonym>[comment][templateType]`, Designer — bare
//! `<Template>Имя</Template>`. X — по ИМЕНИ+ПОРЯДКУ (kind `.TemplateRef`). Появление
//! первого макета в корпусе валидируется R/X-гейтом, не молча.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `templateType` — тип макета. Default SpreadsheetDocument (1С-дефолт, EDT омитит).
pub const F_TEMPLATE_TYPE: FieldId = FieldId(3);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_TEMPLATE_TYPE, "templateType", ValueKind::Enum, PropertyValue::Enum(Token::new("SpreadsheetDocument"))),
    ]
}

/// `&'static EntitySpec` вида `SettingsStorage.TemplateRef` (кэш на процесс).
pub fn settings_storage_template_ref() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "SettingsStorage.TemplateRef",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
