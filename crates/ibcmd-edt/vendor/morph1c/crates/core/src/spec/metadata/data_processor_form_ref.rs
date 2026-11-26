//! Канонический спек ДОЧЕРНЕГО вида `DataProcessor.FormRef` — REF/STUB-коллекция форм
//! (child-objects substrate §3.2 X-asymmetry). Лист-вид. БЕЗ `HARNESS_ENTRY`.
//!
//! Как `ExchangePlan.FormRef`/`Document.FormRef`: EDT `.mdo` несёт ПОЛНЫЙ inline-стаб
//! формы (`<forms uuid><name><synonym>[comment][includeHelpInContents][help]<usePurposes>…`),
//! Designer — лишь BARE-ссылку `<Form>Имя</Form>`. X для FormRef — по ИМЕНИ+ПОРЯДКУ
//! (X-исключение `morph1c_testkit`, kind оканчивается на `.FormRef`).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализ., Default [].
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `includeHelpInContents` — Default false.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(3);
/// `help` — EDT const-блок (presence). Default false.
pub const F_HELP: FieldId = FieldId(4);
/// `usePurposes` — ПЕРЕМЕННЫЙ список назначений (`List([Str…])`), как у
/// `SettingsStorage.FormRef` (метамодель: `usePurposes: ApplicationUsePurpose[*]`).
/// Required (всегда present). Witnessed ERP 2026-07: 11 DataProcessor-стабов несут только
/// `[PersonalComputer]` (напр. `ЕдиныйНалоговыйСчетЛичныйКабинет.Form.ФормаВыбораПериода`) —
/// кодек-константа непригодна.
pub const F_USE_PURPOSES: FieldId = FieldId(5);
/// `extendedPresentation` — локализ. Default [] (EDT-стаб; в корпусе 1 форма несёт).
pub const F_EXTENDED_PRESENTATION: FieldId = FieldId(6);

/// `formType` — тип формы. Designer вложенный дескриптор `Forms/<Имя>.xml` несёт ВСЕГДА
/// (witnessed: `Managed` 18/18 coverage + 772/772 SSL); EDT-стаб опускает дефолт. Default Managed.
pub const F_FORM_TYPE: FieldId = FieldId(7);
/// `useInInterfaceCompatibilityMode` — Designer вложенный дескриптор несёт ВСЕГДА
/// (witnessed: `Any`); EDT-стаб опускает дефолт. Default Any.
pub const F_USE_IN_INTERFACE_COMPATIBILITY_MODE: FieldId = FieldId(8);

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents", ValueKind::Bool, PropertyValue::Bool(false)),
        FieldSpec::with_default(F_HELP, "help", ValueKind::List, PropertyValue::List(Vec::new())),
        // usePurposes — переменный список (1..2 значений witnessed ERP). Required presence.
        FieldSpec::required(F_USE_PURPOSES, "usePurposes", ValueKind::List),
        FieldSpec::with_default(F_EXTENDED_PRESENTATION, "extendedPresentation", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_FORM_TYPE, "formType", ValueKind::Enum, PropertyValue::Enum(Token::new("Managed"))),
        FieldSpec::with_default(F_USE_IN_INTERFACE_COMPATIBILITY_MODE, "useInInterfaceCompatibilityMode", ValueKind::Enum, PropertyValue::Enum(Token::new("Any"))),
    ]
}

/// `&'static EntitySpec` вида `DataProcessor.FormRef` (кэш на процесс).
pub fn data_processor_form_ref() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "DataProcessor.FormRef",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
