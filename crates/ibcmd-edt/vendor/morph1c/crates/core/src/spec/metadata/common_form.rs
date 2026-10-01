//! Канонический спек вида объекта `CommonForm` (общая форма) — ARCHITECTURE.md §1.4/§1.6.
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для КАЖДОГО
//! XML-формата (EDT `.mdo`, Designer `.xml`); формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`, `value_kind`, `default`, нормализация, `since`-гейт и ПОРЯДОК
//! эмиссии (§1.6).
//!
//! Это ДЕСКРИПТОР `CommonForms/<Имя>/<Имя>.mdo` (метаданные-ОБЪЕКТ), НЕ тело формы
//! `Form.form` (дерево контролов — под-IR L1f, `crate::ir::form`, ведётся форм-лейном).
//! Перечисление CommonForm как MetadataObject в whole-config READ разблокирует
//! `CommonForm.<N>`-ссылки Role-прав (ref-harvest резолвит их плоским `Kind.Name`).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта в
//! [`crate::ir::MetadataObject`], round-trip-ится каркасом коннектора.
//!
//! Поля (подтверждено корпусом SSL, 104 объекта edt+designer; Designer DENSE — все
//! serialized-поля present всегда, EDT SPARSE — дефолты опущены):
//! * `Synonym` — локализованный синоним (present 104/104);
//! * `Comment` — свободный текст (default `""`; EDT 2/104, Designer DENSE `<Comment/>`);
//! * `FormType` — тип формы (Enum {Ordinary|Managed}; default `Managed`; EDT 0/104 —
//!   всегда дефолт, Designer DENSE `<FormType>Managed</FormType>`);
//! * `IncludeHelpInContents` — bool включения справки в содержание (default `false`;
//!   EDT 5/104, Designer DENSE);
//! * `Help` — EDT-only const-блок `<help><pages><lang>ru</lang></pages></help>` (presence;
//!   EDT 62/104, Designer несёт справку в спутнике `Ext/`, а НЕ в дескрипторе → X-исключён);
//! * `UsePurposes` — назначения формы. СПИСОК enum-значений, present 104/104 в ОБОИХ, но
//!   ЛИТЕРАЛЫ РАСХОДЯТСЯ между форматами (EDT `PersonalComputer`/`MobileDevice` ↔ Designer
//!   `PlatformApplication`/`MobilePlatformApplication`) → X-исключён (`x_ignore`), как у
//!   `Configuration.usePurposes`; R обоих форматов byte-exact на своих литералах;
//! * `UseInInterfaceCompatibilityMode` — режим совместимости интерфейса (Enum
//!   {Auto|Version8_2|Version8_2AllowVersion8_1|…}; default `Any`; since 8.5.1/2.21; EDT
//!   0/104 — всегда дефолт, Designer DENSE `<UseInInterfaceCompatibilityMode>Any</…>`);
//! * `UseStandardCommands` — bool стандартных команд (default `false`; EDT 49/104,
//!   Designer DENSE);
//! * `ExtendedPresentation` — локализованное расширенное представление (default пустой;
//!   EDT 0/104, Designer DENSE `<ExtendedPresentation/>`);
//! * `Explanation` — локализованное пояснение (default пустой; EDT 1/104, Designer DENSE).
//!
//! Порядок = метамодельный (= DENSE-порядок Designer `<Properties>`): Synonym, Comment,
//! FormType, IncludeHelpInContents, Help, UsePurposes, UseInInterfaceCompatibilityMode,
//! UseStandardCommands, ExtendedPresentation, Explanation. EDT эмитит те же в том же
//! порядке, опуская дефолты (help физически перед usePurposes — совпадает).
//!
//! Metamodel-поля БЕЗ корпус-witness в SSL (`objectBelonging`/`extendedConfigurationObject`,
//! occ=0/104) НАМЕРЕННО НЕ включены (§1.0: не спекулируем без оракула; зеркалит решение
//! `Subsystem`). Их GUID сохранены в [`FIELD_GUIDS`] на случай будущего cf/ERP-грайнда.
//!
//! Лист-вид дескриптора: подчинённых МЕТАДАННЫХ-коллекций нет (тело формы — вне дескриптора).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};
use crate::version::{FormatVersion, Since};

/// `synonym` — локализованный синоним. guid=cf4abea3-37b2-11d4-940f-008048da11f9
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default `""`. guid=cf4abea4-…
pub const F_COMMENT: FieldId = FieldId(2);
/// `formType` — тип формы (Enum). Default `Managed`. guid=e3331ed0-…
pub const F_FORM_TYPE: FieldId = FieldId(5);
/// `includeHelpInContents` — включить справку в содержание (bool). Default `false`. guid=1f2b167a-…
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(6);
/// `help` — EDT-only const-блок `<help>` (presence bool). Default `false`; X-исключён
/// (Designer несёт справку в спутнике, не в дескрипторе). guid=038b5c85-…
pub const F_HELP: FieldId = FieldId(7);
/// `usePurposes` — назначения формы (СПИСОК enum). Default пустой; X-исключён
/// (литералы EDT↔Designer расходятся). guid=da648ef9-…
pub const F_USE_PURPOSES: FieldId = FieldId(8);
/// `useInInterfaceCompatibilityMode` — режим совместимости интерфейса (Enum). Default
/// `Any`; since 8.5.1. guid=c137bf33-…
pub const F_USE_IN_INTERFACE_COMPATIBILITY_MODE: FieldId = FieldId(9);
/// `useStandardCommands` — использовать стандартные команды (bool). Default `false`. guid=d7be18bc-…
pub const F_USE_STANDARD_COMMANDS: FieldId = FieldId(10);
/// `extendedPresentation` — локализованное расширенное представление. Default пустой. guid=c9ba86bf-…
pub const F_EXTENDED_PRESENTATION: FieldId = FieldId(11);
/// `explanation` — локализованное пояснение. Default пустой. guid=719346a1-…
pub const F_EXPLANATION: FieldId = FieldId(12);

fn loc(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}

/// Сконструировать [`FieldSpec`] вида `CommonForm` в каноническом (метамодельном) порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        loc(F_SYNONYM, "synonym"),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        // FormType — Enum {Ordinary|Managed}; default Managed (SSL: все формы управляемые).
        FieldSpec::with_default(
            F_FORM_TYPE,
            "formType",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Managed")),
        ),
        FieldSpec::with_default(
            F_INCLUDE_HELP_IN_CONTENTS,
            "includeHelpInContents",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        // help — EDT-only presence-блок (тот же `HelpConst`, что у Subsystem/Catalog/
        // InformationRegister). Default false; X-исключён (Designer — справка в спутнике).
        FieldSpec::with_default(F_HELP, "help", ValueKind::Bool, PropertyValue::Bool(false))
            .x_ignored(),
        // usePurposes — СПИСОК enum-значений; default пустой; X-исключён (литералы формато-
        // расходятся, как у Configuration.usePurposes). R обоих byte-exact на своих литералах.
        FieldSpec::with_default(F_USE_PURPOSES, "usePurposes", ValueKind::List, PropertyValue::List(Vec::new()))
            .x_ignored(),
        // useInInterfaceCompatibilityMode — Enum; default Any; since 8.5.1/2.21 (гейт §1.5).
        FieldSpec::with_default(
            F_USE_IN_INTERFACE_COMPATIBILITY_MODE,
            "useInInterfaceCompatibilityMode",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Any")),
        )
        .gated(Since(FormatVersion::new(2, 21))),
        FieldSpec::with_default(
            F_USE_STANDARD_COMMANDS,
            "useStandardCommands",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        loc(F_EXTENDED_PRESENTATION, "extendedPresentation"),
        loc(F_EXPLANATION, "explanation"),
    ]
}

/// Канонический [`EntitySpec`] вида `CommonForm` (кэш на процесс). Лист-вид дескриптора
/// (тело формы `Form.form` — под-IR L1f, вне спека дескриптора).
pub fn common_form() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonForm",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}

/// GUID объекта `CommonForm` (идентичность в `.cf`; = `@MdClass ^id`). Используется
/// `configuration_root::KIND_TABLE` (уже объявлен там как B4) и будущим cf-коннектором.
pub const ENTITY_GUID: &str = "07ee8426-87f1-11d5-b99c-0050bae0a95d";

/// `FieldId -> GUID свойства` (идентичность слота в `.cf`; = `@MdProperty ^id`). Для
/// будущего cf-коннектора (матч слота по GUID). Включает и не-эмитируемые в SSL поля
/// (`objectBelonging`/`extendedConfigurationObject`) — чтобы cf/ERP-грайнд имел их GUID.
pub const FIELD_GUIDS: &[(FieldId, &str)] = &[
    (F_SYNONYM, "cf4abea3-37b2-11d4-940f-008048da11f9"),
    (F_COMMENT, "cf4abea4-37b2-11d4-940f-008048da11f9"),
    (F_FORM_TYPE, "e3331ed0-3854-478d-b6b5-4f14acdd6edb"),
    (F_INCLUDE_HELP_IN_CONTENTS, "1f2b167a-25fe-4f47-8710-933567dfa7d9"),
    (F_HELP, "038b5c85-fb1c-4082-9c4c-e69f8928bf3a"),
    (F_USE_PURPOSES, "da648ef9-2f12-418f-8e2f-8956bc10a66f"),
    (F_USE_IN_INTERFACE_COMPATIBILITY_MODE, "c137bf33-c377-41a9-a131-f0c6ef763891"),
    (F_USE_STANDARD_COMMANDS, "d7be18bc-9899-48dd-8100-92dafc4dedea"),
    (F_EXTENDED_PRESENTATION, "c9ba86bf-a1e7-440f-9450-adb61f886f88"),
    (F_EXPLANATION, "719346a1-02d5-4311-a141-4d22470a7ac3"),
];
