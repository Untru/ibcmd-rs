//! Канонический спек вида объекта `HTTPService` (HTTP-сервис) — ARCHITECTURE.md
//! §1.4/§1.6, child-objects substrate. RECURSION-несущий
//! вид: доказывает рекурсивный субстрат на ДВУХ уровнях вложенности
//! (`HTTPService` → `URLTemplate` → `Method`), inline-дети в обоих XML-форматах, БЕЗ
//! платформенного блока `producedTypes`/`standardAttributes` (минимальный, чистый
//! recursion-пилот).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`/`value_kind`/`default`/нормализация/ПОРЯДОК + СЛОТ дочерней
//! коллекции `URLTemplate → HTTPService.URLTemplate`.
//!
//! Спек-свойства (скан SSL, 2 объекта edt+designer). Порядок = DENSE-порядок Designer
//! `<Properties>`: `Synonym, Comment, RootURL, ReuseSessions, SessionMaxAge`. EDT эмитит
//! РАЗРЕЖЁННО (опускает дефолтный `comment`), Designer — DENSE (`<Comment/>`).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность. Дочерние `urlTemplates` — НЕ свойства, а
//! [`ChildSlot`] (рекурсия движка). Тело модуля `Module.bsl` — отдельный артефакт.

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `rootURL` — корневой URL сервиса (свободный текст). Required (всегда present).
pub const F_ROOT_URL: FieldId = FieldId(3);
/// `reuseSessions` — режим повторного использования сессий (`AutoUse`/…). Required
/// (всегда present в обоих форматах — разрежённый EDT его НЕ опускает).
pub const F_REUSE_SESSIONS: FieldId = FieldId(4);
/// `sessionMaxAge` — максимальный возраст сессии (целое, секунды). Required.
pub const F_SESSION_MAX_AGE: FieldId = FieldId(5);

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
        FieldSpec::required(F_ROOT_URL, "rootURL", ValueKind::Str),
        FieldSpec::with_default(
            F_REUSE_SESSIONS,
            "reuseSessions",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("DontUse")),
        ),
        FieldSpec::required(F_SESSION_MAX_AGE, "sessionMaxAge", ValueKind::Int),
    ]
}

/// Слоты дочерних коллекций `HTTPService`: единственная — `URLTemplate` (recursion).
const CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "URLTemplate", child_kind: "HTTPService.URLTemplate" }];

/// Канонический [`EntitySpec`] вида `HTTPService` (кэш на процесс).
pub fn http_service() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "HTTPService",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}
