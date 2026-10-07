//! Канонический спек вида объекта `CommandGroup` (группа команд) — ARCHITECTURE.md
//! §1.4/§1.6. ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write
//! для КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь
//! — канонический `id`, `value_kind`, `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name` (= имя файла/каталога) и `uuid` (GUID) — идентичность
//! объекта в [`crate::ir::MetadataObject`], round-trip-ится каркасом коннектора.
//!
//! Поля (подтверждено корпусом SSL, 6 объектов edt+designer):
//! * `Synonym` — локализованный синоним (6/6);
//! * `Comment` — свободный текст (1/6 непустой; EDT опускает дефолт, Designer `<Comment/>`);
//! * `Representation` — перечислимый литерал представления (`Auto`/`Picture`/…);
//! * `ToolTip` — локализованная подсказка (1/6 непустая; default = пустой список);
//! * `Picture` — ссылка на стандартную/общую картинку (`PictureRef`; `""` = пусто, дефолт);
//! * `Category` — перечислимый литерал категории (`FormCommandBar`/`ActionsPanel`/…).
//!
//! Лист-вид: подчинённых коллекций нет. Порядок = DENSE-порядок Designer `<Properties>`:
//! Synonym, Comment, Representation, ToolTip, Picture, Category (EDT эмитит то же,
//! опуская дефолтные comment/toolTip/picture).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize, picture_ref_field};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `representation` — литерал представления (перечислимый). Required (всегда present).
pub const F_REPRESENTATION: FieldId = FieldId(3);
/// `toolTip` — локализованная подсказка. Default = пустой список; сорт по коду языка.
pub const F_TOOLTIP: FieldId = FieldId(4);
/// `picture` — ссылка на картинку (`PictureRef`). Default = `""` (пусто).
pub const F_PICTURE: FieldId = FieldId(5);
/// `category` — литерал категории (перечислимый). Required (всегда present).
pub const F_CATEGORY: FieldId = FieldId(6);

/// Сконструировать [`FieldSpec`] вида `CommandGroup` в каноническом порядке.
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
        // Representation — перечислимый литерал (Token round-trip'ится byte-exact, X by constr.).
        // Default `Auto`: EDT (sparse) опускает при дефолте, Designer (dense) эмитит → один IR.
        FieldSpec::with_default(
            F_REPRESENTATION,
            "representation",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("Text")),
        ),
        FieldSpec::with_default(
            F_TOOLTIP,
            "toolTip",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        // Picture — ссылка на картинку; кодек PictureRef. Default = "" (пусто).
        picture_ref_field(F_PICTURE),
        // Category — перечислимый литерал. Default `FormCommandBar` (sparse EDT омитит).
        FieldSpec::with_default(
            F_CATEGORY,
            "category",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("NavigationPanel")),
        ),
    ]
}

/// Канонический [`EntitySpec`] вида `CommandGroup` (кэш на процесс).
pub fn command_group() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommandGroup",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}
