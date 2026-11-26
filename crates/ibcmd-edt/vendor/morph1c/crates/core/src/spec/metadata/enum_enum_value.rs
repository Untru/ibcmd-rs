//! Канонический спек ДОЧЕРНЕГО вида `Enum.EnumValue` (значение перечисления) —
//! child-objects substrate, §1.1 «одна сущность — один
//! тип» (ребёнок — это [`crate::ir::MetadataObject`] рекурсивно).
//!
//! Dotted-kind ключ (`"Enum.EnumValue"`) — родитель-квалифицированный (§1.2 design);
//! stem файла ASCII snake_case (`enum_enum_value`), функция `enum_enum_value()` (=stem,
//! контракт build.rs). У child-вида НЕТ собственного корпус-пути / `HARNESS_ENTRY`:
//! он покрыт ТРАНЗИТИВНО через родителя `Enum` (R parent byte-exact ⇒ inline-дети
//! byte-exact; X parent IR-равен ⇒ `children` рекурсивно равны).
//!
//! Спек-свойства (скан SSL, 443 EnumValue × 104 enum, edt+designer): `Synonym`,
//! `Comment`, `Color`. Порядок = канонический DENSE-порядок Designer
//! (`<Properties>`: Name(идентичность), Synonym, Comment, Color). EDT эмитит РАЗРЕЖЁННО
//! (только не-дефолты: имя+synonym+редкий comment), Designer — DENSE (+ `<Color>auto`).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность ребёнка (каркас child-навигации). `Color`
//! — Designer-only (EDT его не несёт), всегда `auto` в SSL → дефолт `Enum("auto")`:
//! EDT read absent→default, Designer read `auto`==default→омиссия из bag → оба bag'а
//! идентичны (X by construction), Designer DENSE-write восстанавливает `<Color>auto`.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним значения. Default = пустой список; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `color` — цвет значения (Designer-only). Default = `Enum("auto")`.
pub const F_COLOR: FieldId = FieldId(3);

/// Сконструировать [`FieldSpec`] child-вида `Enum.EnumValue` в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(F_COLOR, "color", ValueKind::Enum, PropertyValue::Enum(Token::new("auto"))),
    ]
}

/// Канонический [`EntitySpec`] child-вида `Enum.EnumValue` (кэш на процесс). Лист-вид
/// (`children: &[]`): EnumValue не несёт собственных коллекций.
pub fn enum_enum_value() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Enum.EnumValue",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
