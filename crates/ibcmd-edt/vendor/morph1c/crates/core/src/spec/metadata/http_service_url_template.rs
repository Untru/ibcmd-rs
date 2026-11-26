//! Канонический спек ДОЧЕРНЕГО вида `HTTPService.URLTemplate` — шаблон URL
//! HTTP-сервиса. child-objects substrate, §1.1.
//!
//! **RECURSION-узел:** несёт собственную child-коллекцию `Method`
//! (`HTTPService.URLTemplate.Method`) — доказательство рекурсивного субстрата
//! (родитель `HTTPService` → `URLTemplate` → `Method`, два уровня вложенности).
//!
//! Dotted-kind ключ (`"HTTPService.URLTemplate"`); stem = ASCII snake_case, функция =
//! stem (build.rs). БЕЗ собственного `HARNESS_ENTRY` — покрыт ТРАНЗИТИВНО через корень.
//!
//! Спек-свойства (скан SSL, 14 URLTemplate × 2 HTTPService, edt+designer): `Synonym`,
//! `Comment`, `Template`. Порядок = DENSE-порядок Designer (`<Properties>`:
//! Name(идентичность), Synonym, Comment, Template). EDT — РАЗРЕЖЁННО (имя+synonym+
//! template), Designer — DENSE (+`<Comment/>`). Собственные дети `Method` — НЕ свойства,
//! а [`ChildSlot`] (рекурсия движка).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним шаблона. Default = пустой список; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `template` — строка-шаблон URL (свободный текст). Required (всегда present).
pub const F_TEMPLATE: FieldId = FieldId(3);

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
        FieldSpec::required(F_TEMPLATE, "template", ValueKind::Str),
    ]
}

/// Слоты дочерних коллекций `URLTemplate`: единственная — `Method` (рекурсия).
const CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "Method", child_kind: "HTTPService.URLTemplate.Method" }];

/// Канонический [`EntitySpec`] вида `HTTPService.URLTemplate` (кэш на процесс).
/// Recursion-узел: несёт `Method`-коллекцию.
pub fn http_service_url_template() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "HTTPService.URLTemplate",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}
