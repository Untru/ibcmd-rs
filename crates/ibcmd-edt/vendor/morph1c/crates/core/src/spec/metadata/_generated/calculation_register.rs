//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `CalculationRegister` (РегистрРасчета), guid=f2de87a8-64e5-45eb-a22d-b3aedab050e7.
//!
//! НЕ КОММИТИТЬ КАК ЕСТЬ (§1.0): порядок полей = EMF/.mdo (supertypes-first); Designer
//! DENSE-порядок и дефолты ВЕРИФИЦИРОВАТЬ R/X-гейтом + дифф-фикстурами (docs/RESEARCH.md).
//! `name`/`uuid` — идентичность, вне спека. Все `// TODO(draft)` — проверить.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym`  guid=cf4abea3-37b2-11d4-940f-008048da11f9
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment`  guid=cf4abea4-37b2-11d4-940f-008048da11f9
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging`  guid=19744814-daec-423b-8269-995b53ebe0ec
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject`  guid=9595ddd6-e72c-47ad-a156-672db811628c
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);
/// `useStandardCommands`  guid=d7be18bc-9899-48dd-8100-92dafc4dedea
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(5);
/// `defaultListForm`  guid=b69601f5-4cf6-11d4-9415-008048da11f9
pub const F_DEFAULT_LIST_FORM: FieldId = FieldId(6);
/// `auxiliaryListForm`  guid=367b60f5-2ad5-4399-a6b0-3dd8e4ac3650
pub const F_AUXILIARY_LIST_FORM: FieldId = FieldId(7);
/// `periodicity`  guid=a49c7a4a-8cb5-4f8b-a034-4e8f257ada40
pub const F_PERIODICITY: FieldId = FieldId(8);
/// `actionPeriod`  guid=cab8b8ca-b4ad-476e-aaaf-097b3426002e
pub const F_ACTION_PERIOD: FieldId = FieldId(9);
/// `basePeriod`  guid=3d2ad396-87e5-4c1a-a3c1-66d613ff19af
pub const F_BASE_PERIOD: FieldId = FieldId(10);
/// `schedule`  guid=d35dfaf3-843a-4745-b0fe-d47d325573a8
pub const F_SCHEDULE: FieldId = FieldId(11);
/// `scheduleValue`  guid=942f4471-e1cd-4bcf-8206-ff65bd4737cb
pub const F_SCHEDULE_VALUE: FieldId = FieldId(12);
/// `scheduleDate`  guid=c2cf06ca-3429-48c0-a02d-0df4330f6e1e
pub const F_SCHEDULE_DATE: FieldId = FieldId(13);
/// `chartOfCalculationTypes`  guid=c62fbafa-5e5d-47e7-a2a4-a90eb55bcdfd
pub const F_CHART_OF_CALCULATION_TYPES: FieldId = FieldId(14);
/// `includeHelpInContents`  guid=1f2b167a-25fe-4f47-8710-933567dfa7d9
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(15);
/// `help`  guid=038b5c85-fb1c-4082-9c4c-e69f8928bf3a
pub const F_HELP: FieldId = FieldId(16);
/// `dataLockControlMode`  guid=09c14dad-bd23-4d30-9c09-20ca3e5cd9f0
pub const F_DATA_LOCK_CONTROL_MODE: FieldId = FieldId(17);
/// `fullTextSearch`  guid=d4232326-022b-421e-b6d3-88e418f74327
pub const F_FULL_TEXT_SEARCH: FieldId = FieldId(18);
/// `listPresentation`  guid=b894dce6-9f10-45d6-958f-45993525a044
pub const F_LIST_PRESENTATION: FieldId = FieldId(19);
/// `extendedListPresentation`  guid=97f5ad64-29e4-410f-8ead-ec0dd5c5e58d
pub const F_EXTENDED_LIST_PRESENTATION: FieldId = FieldId(20);
/// `explanation`  guid=719346a1-02d5-4311-a141-4d22470a7ac3
pub const F_EXPLANATION: FieldId = FieldId(21);

fn loc_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn b(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}
fn e(id: FieldId, name: &'static str, def: &str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, PropertyValue::Enum(Token::new(def)))
}
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        b(F_USE_STANDARD_COMMANDS, "useStandardCommands"),
        s(F_DEFAULT_LIST_FORM, "defaultListForm"),  // TODO(draft): ref -> CalculationRegisterForm (form-ref Str vs Ref?)
        s(F_AUXILIARY_LIST_FORM, "auxiliaryListForm"),  // TODO(draft): ref -> CalculationRegisterForm (form-ref Str vs Ref?)
        e(F_PERIODICITY, "periodicity", "Year"),  // literals: {Year|Quarter|Month|Day}
        b(F_ACTION_PERIOD, "actionPeriod"),
        b(F_BASE_PERIOD, "basePeriod"),
        s(F_SCHEDULE, "schedule"),  // TODO(draft): ref -> InformationRegister (form-ref Str vs Ref?)
        s(F_SCHEDULE_VALUE, "scheduleValue"),  // TODO(draft): ref -> InformationRegisterResource (form-ref Str vs Ref?)
        s(F_SCHEDULE_DATE, "scheduleDate"),  // TODO(draft): ref -> InformationRegisterDimension (form-ref Str vs Ref?)
        s(F_CHART_OF_CALCULATION_TYPES, "chartOfCalculationTypes"),  // TODO(draft): ref -> ChartOfCalculationTypes (form-ref Str vs Ref?)
        b(F_INCLUDE_HELP_IN_CONTENTS, "includeHelpInContents"),
        s(F_HELP, "help").x_ignored(),  // TODO(draft): contained Help — уточнить vk  EDT-only?
        e(F_DATA_LOCK_CONTROL_MODE, "dataLockControlMode", "Automatic"),  // literals: {Automatic|Managed|AutomaticAndManaged}
        e(F_FULL_TEXT_SEARCH, "fullTextSearch", "DontUse"),  // literals: {DontUse|Use}
        loc_field(F_LIST_PRESENTATION, "listPresentation"),
        loc_field(F_EXTENDED_LIST_PRESENTATION, "extendedListPresentation"),
        loc_field(F_EXPLANATION, "explanation"),
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "StandardAttribute", child_kind: "CalculationRegister.StandardAttribute" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `CalculationRegister` (ЧЕРНОВИК).
pub fn calculation_register() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CalculationRegister",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `CalculationRegister` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "f2de87a8-64e5-45eb-a22d-b3aedab050e7";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_DEFAULT_LIST_FORM, "b69601f5-4cf6-11d4-9415-008048da11f9"),
    (F_AUXILIARY_LIST_FORM, "367b60f5-2ad5-4399-a6b0-3dd8e4ac3650"),
    (F_PERIODICITY, "a49c7a4a-8cb5-4f8b-a034-4e8f257ada40"),
    (F_ACTION_PERIOD, "cab8b8ca-b4ad-476e-aaaf-097b3426002e"),
    (F_BASE_PERIOD, "3d2ad396-87e5-4c1a-a3c1-66d613ff19af"),
    (F_SCHEDULE, "d35dfaf3-843a-4745-b0fe-d47d325573a8"),
    (F_SCHEDULE_VALUE, "942f4471-e1cd-4bcf-8206-ff65bd4737cb"),
    (F_SCHEDULE_DATE, "c2cf06ca-3429-48c0-a02d-0df4330f6e1e"),
    (F_CHART_OF_CALCULATION_TYPES, "c62fbafa-5e5d-47e7-a2a4-a90eb55bcdfd"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_DATA_LOCK_CONTROL_MODE, "09c14dad-bd23-4d30-9c09-20ca3e5cd9f0"),
    (F_FULL_TEXT_SEARCH, "d4232326-022b-421e-b6d3-88e418f74327"),
    (F_LIST_PRESENTATION, "b894dce6-9f10-45d6-958f-45993525a044"),
    (F_EXTENDED_LIST_PRESENTATION, "97f5ad64-29e4-410f-8ead-ec0dd5c5e58d"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
];
