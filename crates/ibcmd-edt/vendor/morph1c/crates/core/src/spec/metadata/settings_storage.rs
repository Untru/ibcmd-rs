//! Канонический спек вида `SettingsStorage` (хранилище настроек) — ARCHITECTURE.md
//! §1.4/§1.6, child-objects substrate. Метамодель:
//! `MdClass.xcore` класс `SettingsStorage`, guid=46b4cd97-fd13-4eaa-aba2-3bddd7699218.
//!
//! ЛЁГКИЙ структурный вид: 4 form-ref-поля (default/auxiliary × save/load — ссылки на
//! СОБСТВЕННЫЕ формы `SettingsStorage.<self>.Form.<name>`) + ref/stub-коллекции
//! Form/Template. БЕЗ standardAttributes/DB-persistence/представлений.
//!
//! Порядок свойств = Designer DENSE-порядку `<Properties>` (сверено 1/1 SSL:
//! Name, Synonym, Comment, DefaultSaveForm, DefaultLoadForm, AuxiliarySaveForm,
//! AuxiliaryLoadForm); EDT эмитит РАЗРЕЖЁННО в том же относительном порядке (сверено).
//! `objectBelonging`/`extendedConfigurationObject` — structural-tail (MdObject-супертип):
//! SSL-корпус их не витнессит (всегда дефолт) → объявлены канонически, НЕ проецируются.
//!
//! ВНЕ спека: `name`/`uuid` — идентичность; `producedTypes` (одна категория Manager) —
//! платформенный [`crate::ir::InternalInfo`]-блок (каркас, см. [`PRODUCED_CATEGORIES`]);
//! `managerModule` — тело-сайдкар (`ManagerModule.bsl`), вне дескриптора.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default []; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий. Default "".
pub const F_COMMENT: FieldId = FieldId(2);
/// `objectBelonging` — принадлежность (enum {Native|Adopted}). Default Native.
/// structural-tail: не витнессится SSL (всегда дефолт) → не проецируется.
pub const F_OBJECT_BELONGING: FieldId = FieldId(3);
/// `extendedConfigurationObject` — Uuid расширяемого объекта (Str-leaf; пустой=дефолт).
/// structural-tail: не витнессится SSL → не проецируется.
pub const F_EXTENDED_CONFIGURATION_OBJECT: FieldId = FieldId(4);
/// `defaultSaveForm` — form-ref на СОБСТВЕННУЮ форму (`SettingsStorage.<self>.Form.<name>`).
/// Default "".
pub const F_DEFAULT_SAVE_FORM: FieldId = FieldId(5);
/// `defaultLoadForm` — form-ref. Default "".
pub const F_DEFAULT_LOAD_FORM: FieldId = FieldId(6);
/// `auxiliarySaveForm` — form-ref. Default "" (SSL: всегда пуст).
pub const F_AUXILIARY_SAVE_FORM: FieldId = FieldId(7);
/// `auxiliaryLoadForm` — form-ref. Default "" (SSL: всегда пуст).
pub const F_AUXILIARY_LOAD_FORM: FieldId = FieldId(8);

fn empty_str() -> PropertyValue {
    PropertyValue::Str(String::new())
}

fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(F_SYNONYM, "synonym", ValueKind::Localized, PropertyValue::Localized(Vec::new()))
            .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_DEFAULT_SAVE_FORM, "defaultSaveForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_DEFAULT_LOAD_FORM, "defaultLoadForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_AUXILIARY_SAVE_FORM, "auxiliarySaveForm", ValueKind::Str, empty_str()),
        FieldSpec::with_default(F_AUXILIARY_LOAD_FORM, "auxiliaryLoadForm", ValueKind::Str, empty_str()),
        // structural-tail: канонически объявлены, форматы не проецируют (дефолт-омиссия).
        FieldSpec::with_default(F_OBJECT_BELONGING, "objectBelonging", ValueKind::Enum, PropertyValue::Enum(Token::new("Native"))),
        FieldSpec::with_default(F_EXTENDED_CONFIGURATION_OBJECT, "extendedConfigurationObject", ValueKind::Str, empty_str()),
    ]
}

/// Слоты дочерних коллекций в КАНОНИЧЕСКОМ порядке эмиссии (Form < Template — как у всех
/// структурных видов). Обе — ref/stub-коллекции (X по имени+порядку).
const CHILDREN: &[ChildSlot] = &[
    ChildSlot { collection: "Form", child_kind: "SettingsStorage.FormRef" },
    ChildSlot { collection: "Template", child_kind: "SettingsStorage.TemplateRef" },
];

/// Канонический [`EntitySpec`] вида `SettingsStorage` (кэш на процесс).
pub fn settings_storage() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "SettingsStorage",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}

// ============================================================================
// Категории платформенно-сгенерированных типов (`producedTypes`/`InternalInfo`)
// — §1.6/§3.5, R2 data-drive; build.rs подхватывает `pub const` по имени.
// ============================================================================
/// Категории producedTypes вида `SettingsStorage` в каноническом порядке IR (= EDT):
/// одна категория Manager (сверено 1/1 SSL: EDT `<managerType typeId valueTypeId/>`,
/// Designer `<xr:GeneratedType name="SettingsStorageManager.<Имя>" category="Manager">`).
pub const PRODUCED_CATEGORIES: &[crate::spec::common::ProducedCategory] = &[
    crate::spec::common::ProducedCategory {
        category: "Manager",
        edt_tag: "managerType",
        designer_category: "Manager",
        designer_type_name: "SettingsStorageManager",
        designer_order: 0,
    },
];

/// GUID класса `SettingsStorage` (идентичность в `.cf`; = `@MdClass ^id`). В `ssl.cf` этот
/// guid — маркер Form-коллекции дескриптора (RE 1/1; == class-guid `SettingsStorageForm`
/// у формы: b8533c0c — см. cf-проекцию).
pub const ENTITY_GUID: &str = "46b4cd97-fd13-4eaa-aba2-3bddd7699218";

/// `FieldId -> GUID свойства` (метамодель `@MdProperty ^id`; провенанс cf-слот-привязки).
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_OBJECT_BELONGING, "19744814-daec-423b-8269-995b53ebe0ec"),
    (F_EXTENDED_CONFIGURATION_OBJECT, "9595ddd6-e72c-47ad-a156-672db811628c"),
    (F_DEFAULT_SAVE_FORM, "f5094eee-42ee-4e29-8536-a6f06f20b66a"),
    (F_DEFAULT_LOAD_FORM, "48fe5600-73bd-42d1-9c4f-de6647049f4a"),
    (F_AUXILIARY_SAVE_FORM, "7cb99c54-28fe-422e-9c07-1262b43e3bf4"),
    (F_AUXILIARY_LOAD_FORM, "8993266c-719b-4ad0-ae5f-b86bd62a09c6"),
];
