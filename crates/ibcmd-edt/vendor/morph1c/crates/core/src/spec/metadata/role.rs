//! Канонический спек вида объекта `Role` (роль — набор прав доступа) — ARCHITECTURE.md
//! §1.4/§1.6, срез S2 (Rights/Role rights-table + текстовый sidecar `Rights.rights`).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write КАЖДОГО
//! XML-формата (EDT `.mdo`, Designer `.xml`) из него — формат хранит лишь ПРОЕКЦИЮ (имя
//! тега/ns) в зеркальном модуле. Здесь — канонический `id`, `value_kind`, `default` и
//! ПОРЯДОК эмиссии (§1.6).
//!
//! # Что здесь (и чего НЕТ)
//! Дескриптор `.mdo`/`.xml` вида Role — ТОНКИЙ: несёт лишь `synonym` (все 107 SSL) и
//! `comment` (дефолт `""`). Сама ТАБЛИЦА ПРАВ живёт в ОТДЕЛЬНОМ текстовом XML-СПУТНИКЕ
//! рядом с дескриптором — `Rights.rights` (EDT) / `Ext/Rights.xml` (Designer), ns
//! `http://v8.1c.ru/8.2/roles`, root `<Rights>`. Спутник — НЕ спек-свойство, разбираемое
//! движком: он фреймится каркасом коннектора через rights-table-кодек
//! (`formats_xml::rights` + testkit sidecar-aware R/X), аналогично тому как `Module.bsl`/
//! `producedTypes` фреймятся вне спек-региона. Поэтому в `fields()` его нет.
//!
//! Форма ТАБЛИЦЫ прав — [`RightsTable`] (ниже): это КАНОНИЧЕСКИЙ формат-нейтральный IR
//! sidecar'а (per-kind Role area, §1.6). EDT и Designer парсят ОДНО И ТО ЖЕ тело в РАВНУЮ
//! [`RightsTable`] (различаются лишь envelope-байты: BOM/`version`/трейлер) → cross-format
//! X по канон-таблице (§3.5). Метамодель-only поля (`ObjectBelonging`/
//! `ExtendedConfigurationObject`, 0 вхождений в SSL) НЕ спекнуты (нет свидетеля §1.0).
//!
//! `name`/`uuid` — идентичность объекта ([`crate::ir::MetadataObject`]), round-trip-ится
//! каркасом, а не [`FieldSpec`] (как у всех видов).
//!
//! # Порядок полей (= порядок эмиссии Designer `<Properties>`, сверено корпусом SSL)
//! `Synonym` → `Comment`. Канонический IR РАЗРЕЖЕН (§1.1): EDT-`.mdo` эмитит лишь
//! non-default; Designer DENSE эмитит и дефолт `<Comment/>`, но движок сводит к тому же
//! РАЗРЕЖЁННОМУ IR → edt==designer (X by construction, §1.6/§3.5).

use serde::{Deserialize, Serialize};

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; маркер сортировки по
/// коду языка для X (порядок языков хранится из источника — byte-exact R, §1.6).
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);

/// Сконструировать [`FieldSpec`]'ы вида `Role` в каноническом порядке.
///
/// Не `static`: `default`-значения ([`PropertyValue`]) владеют `String`/`Vec` (не
/// `const`-конструируемы). Кэшируется на первый вызов ([`role`]).
fn build_fields() -> Vec<FieldSpec> {
    vec![
        // synonym: Localized, default пустой список, маркер сорт-по-lang для X.
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        // comment: Str, default "".
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
    ]
}

/// Канонический [`EntitySpec`] вида `Role` (кэш на процесс).
///
/// 2 тонких свойства дескриптора в каноническом порядке эмиссии (`name`/`uuid` —
/// идентичность объекта, тут НЕ перечислены; таблица прав — текстовый sidecar, вне спека).
pub fn role() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Role",
        // Утечка единожды на процесс → владеемый Vec в `&'static [_]` без unsafe.
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // таблица прав — текстовый sidecar (каркас), не child-коллекция.
    })
}

// ============================================================================
// Rights-table IR (per-kind Role area, §1.6) — КАНОНИЧЕСКАЯ форма спутника прав.
//
// Формат-нейтрален (ни BOM, ни имён тегов): EDT и Designer парсят ОДНО тело в РАВНУЮ
// [`RightsTable`] → cross-format X по канон-таблице. `Rights.rights`-кодек (`formats_xml::
// rights`) строит эту форму из байтов спутника и регенерирует их byte-exact per формат.
// ============================================================================

/// Каноническая таблица прав роли (содержимое спутника `Rights.rights`/`Ext/Rights.xml`).
///
/// Три флага корня + УПОРЯДОЧЕННЫЙ список объектов + УПОРЯДОЧЕННЫЙ список шаблонов
/// ограничений (порядок структурен — часть идентичности значения, byte-exact R сохраняет
/// его). §1.0: любой объект/право/флаг/шаблон/элемент, не покрытый этой формой, —
/// типизированная ошибка кодека (не best-effort).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RightsTable {
    /// `<setForNewObjects>` — устанавливать права для новых объектов.
    pub set_for_new_objects: bool,
    /// `<setForAttributesByDefault>` — устанавливать права реквизитов по умолчанию.
    pub set_for_attributes_by_default: bool,
    /// `<independentRightsOfChildObjects>` — независимые права подчинённых объектов.
    pub independent_rights_of_child_objects: bool,
    /// Объекты-права в исходном порядке (`<object>` элементы).
    pub objects: Vec<RightsObject>,
    /// Шаблоны ограничения доступа (`<restrictionTemplate>` — top-level, ПОСЛЕ объектов),
    /// в исходном порядке: `(имя, условие)`. Пусто у большинства ролей.
    pub restriction_templates: Vec<RestrictionTemplate>,
}

/// Права роли на ОДИН объект метаданных (`<object>`): полное имя `Kind.Name` + список
/// прав в исходном порядке.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RightsObject {
    /// `<name>` объекта — полное имя `Configuration.X`/`Catalog.Y`/… (свободная строка).
    pub name: String,
    /// `<right>` записи в исходном порядке.
    pub rights: Vec<Right>,
}

/// Одно право (`<right>`): имя + булево значение + опциональное ограничение по условию.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Right {
    /// `<name>` права — свободный идентификатор (`Read`/`Update`/`View`/…).
    pub name: String,
    /// `<value>` — строго bool (`true`/`false`).
    pub value: bool,
    /// `<restrictionByCondition>` — ограничение доступа по условию (34 роли из 107), либо
    /// `None`. Несёт опциональное `<field>` + текст `<condition>` (BSL/SQL, verbatim).
    pub restriction: Option<RightRestriction>,
}

/// `<restrictionByCondition>` внутри `<right>`: опциональное поле + текст условия.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RightRestriction {
    /// `<field>` — имя поля (1 вхождение в SSL), либо `None`.
    pub field: Option<String>,
    /// `<condition>` — текст условия (BSL/SQL, многострочный, с XML-entity `&amp;`/`&gt;`/
    /// `&lt;`) — переносится ВЕРБАТИМ (raw-байты между тегами) для byte-exact round-trip.
    pub condition: String,
}

/// `<restrictionTemplate>` (top-level после объектов): именованный шаблон ограничения.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestrictionTemplate {
    /// `<name>` шаблона (`ДляОбъекта(ПолеОбъекта)` и т.п.).
    pub name: String,
    /// `<condition>` — текст шаблона (verbatim, как у [`RightRestriction::condition`]).
    pub condition: String,
}

// Реестр (`crate::spec::registry`) собирается build.rs'ом, который ИМЕНУЕТ `role::role`
// — отдельной саморегистрации не нужно. Код вида берётся из `EntitySpec::entity` (== "Role").
