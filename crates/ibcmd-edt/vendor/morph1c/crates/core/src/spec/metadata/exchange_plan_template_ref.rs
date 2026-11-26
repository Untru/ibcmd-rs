//! Канонический спек ДОЧЕРНЕГО вида `ExchangePlan.TemplateRef` — REF/STUB-коллекция
//! макетов (child-objects substrate §3.2). Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! Как `Document.TemplateRef`: EDT несёт стаб `<templates uuid><name><synonym>[comment]
//! [templateType]`, Designer — bare `<Template>Имя</Template>`. X — по ИМЕНИ+ПОРЯДКУ
//! (kind `.TemplateRef`).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `templateType` — тип макета. Default SpreadsheetDocument (EDT омитит дефолт). Enum.
pub const F_TEMPLATE_TYPE: FieldId = FieldId(3);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_TEMPLATE_TYPE, "templateType", ValueKind::Enum, PropertyValue::Enum(Token::new("SpreadsheetDocument"))),
    ]
}

/// `&'static EntitySpec` вида `ExchangePlan.TemplateRef` (кэш на процесс).
pub fn exchange_plan_template_ref() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ExchangePlan.TemplateRef",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
