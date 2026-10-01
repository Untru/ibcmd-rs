//! Канонический спек ДОЧЕРНЕГО вида `AccumulationRegister.TemplateRef` — REF/STUB-
//! коллекция макетов (child-objects substrate §3.2). Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! Зеркало `InformationRegister.TemplateRef`: EDT несёт стаб `<templates uuid><name>
//! <synonym>[<templateType>]`. Вид edt-only. EDT-стаб (сверено 2 макета ERP-корпуса):
//! name(идентичность), synonym, [comment], [templateType].

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `templateType` — тип макета. Default SpreadsheetDocument (1С-дефолт: EDT омитит его). Enum.
pub const F_TEMPLATE_TYPE: FieldId = FieldId(3);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_TEMPLATE_TYPE, "templateType", ValueKind::Enum, PropertyValue::Enum(Token::new("SpreadsheetDocument"))),
    ]
}

/// `&'static EntitySpec` вида `AccumulationRegister.TemplateRef` (кэш на процесс).
pub fn accumulation_register_template_ref() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccumulationRegister.TemplateRef",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
