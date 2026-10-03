//! ⚠️ ЧЕРНОВИК (AUTO-GENERATED `extract-mdclasses.py --emit-rust`). Источник:
//! EDT-метамодель `MdClass.xcore`, класс `ChartOfAccountsTabularSection` (ТабличнаяЧасть), guid=4c7fec95-d1bd-4508-8a01-f1db090d9af8.
//!
//! НЕ КОММИТИТЬ КАК ЕСТЬ (§1.0): порядок полей = EMF/.mdo (supertypes-first); Designer
//! DENSE-порядок и дефолты ВЕРИФИЦИРОВАТЬ R/X-гейтом + дифф-фикстурами (docs/RESEARCH.md).
//! `name`/`uuid` — идентичность, вне спека. Все `// TODO(draft)` — проверить.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};
use crate::version::{FormatVersion, Since};

/// `synonym`  guid=cf4abea3-37b2-11d4-940f-008048da11f9
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment`  guid=cf4abea4-37b2-11d4-940f-008048da11f9
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging`  guid=19744814-daec-423b-8269-995b53ebe0ec
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject`  guid=9595ddd6-e72c-47ad-a156-672db811628c
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);
/// `toolTip`  guid=4690ff70-e3fa-4914-9127-6a9acc5fc949
pub const F_TOOL_TIP: FieldId = FieldId(5);
/// `fillChecking`  guid=2723eb98-b4c1-498a-a6f3-70444757902f
pub const F_FILL_CHECKING: FieldId = FieldId(6);
/// `lineNumberLength` (since 8.3.27)  guid=8a4aa669-77ae-4d4f-a799-d6b496c38a26
pub const F_LINE_NUMBER_LENGTH: FieldId = FieldId(7);

fn loc_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn e(id: FieldId, name: &'static str, def: &str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Enum, PropertyValue::Enum(Token::new(def)))
}
fn s(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}
fn i(id: FieldId, name: &'static str, def: i64) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Int, PropertyValue::Int(def))
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc_field(F_SYNONYM, "synonym"),
        s(F_COMMENT, "comment"),
        e(F_OBJECT_BELONGING, "objectBelonging", "Native"),  // literals: {Native|Adopted}
        s(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject"),  // TODO(draft): тип Uuid — уточнить кодировку
        loc_field(F_TOOL_TIP, "toolTip"),
        e(F_FILL_CHECKING, "fillChecking", "DontCheck"),  // literals: {DontCheck|ShowError}
        i(F_LINE_NUMBER_LENGTH, "lineNumberLength", 0)
            .gated(Since(FormatVersion::new(2, 20))),
    ]
}

const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "StandardAttribute", child_kind: "ChartOfAccountsTabularSection.StandardAttribute" },  // TODO(draft): Ref/stub?
];

/// Канонический [`EntitySpec`] вида `ChartOfAccountsTabularSection` (ЧЕРНОВИК).
pub fn chart_of_accounts_tabular_section() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "ChartOfAccountsTabularSection",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

/// GUID объекта `ChartOfAccountsTabularSection` (идентичность в `.cf`; = `@MdClass ^id`).
pub const ENTITY_GUID: &str = "4c7fec95-d1bd-4508-8a01-f1db090d9af8";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`).
/// cf-проекция матчит слот по GUID, а не по позиции.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_TOOL_TIP, "4690ff70-e3fa-4914-9127-6a9acc5fc949"),
    (F_FILL_CHECKING, "2723eb98-b4c1-498a-a6f3-70444757902f"),
    (F_LINE_NUMBER_LENGTH, "8a4aa669-77ae-4d4f-a799-d6b496c38a26"),
];
