//! Канонический спек ДОЧЕРНЕГО (внук) вида `HTTPService.URLTemplate.Method` — метод
//! шаблона URL HTTP-сервиса. child-objects substrate,
//! §1.1 «одна сущность — один тип» (ребёнок — [`crate::ir::MetadataObject`] рекурсивно).
//!
//! Dotted-kind ключ (`"HTTPService.URLTemplate.Method"`) — родитель-цепочка (§1.2);
//! stem файла ASCII snake_case, функция = stem (контракт build.rs). У child-вида НЕТ
//! собственного корпус-пути/`HARNESS_ENTRY`: покрыт ТРАНЗИТИВНО через корень
//! `HTTPService` (R parent byte-exact ⇒ inline-внуки byte-exact; X parent IR-равен ⇒
//! `children` рекурсивно равны).
//!
//! Спек-свойства (скан SSL, 16 Method × 2 HTTPService, edt+designer): `Synonym`,
//! `Comment`, `HTTPMethod`, `Handler`. Порядок = канонический DENSE-порядок Designer
//! (`<Properties>`: Name(идентичность), Synonym, Comment, HTTPMethod, Handler). EDT —
//! РАЗРЕЖЁННО (имя+synonym+handler+редкий httpMethod), Designer — DENSE (+`<Comment/>`,
//! всегда `<HTTPMethod>`).
//!
//! ВНЕ спека: `name`/`uuid` — идентичность (каркас child-навигации). `httpMethod`
//! default = `Enum("GET")`: EDT опускает GET (sparse), Designer эмитит `<HTTPMethod>GET`
//! (dense) → оба bag'а идентичны (X by construction).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним метода. Default = пустой список; сорт по языку.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `httpMethod` — HTTP-глагол (`GET`/`POST`/`DELETE`/…). Default = `Enum("GET")`.
pub const F_HTTP_METHOD: FieldId = FieldId(3);
/// `handler` — имя обработчика (свободный текст). Required (всегда present).
pub const F_HANDLER: FieldId = FieldId(4);

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
        FieldSpec::with_default(
            F_HTTP_METHOD,
            "httpMethod",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("GET")),
        ),
        FieldSpec::required(F_HANDLER, "handler", ValueKind::Str),
    ]
}

/// Канонический [`EntitySpec`] вида `HTTPService.URLTemplate.Method` (кэш на процесс).
/// Лист-вид (`children: &[]`): метод не несёт собственных коллекций.
pub fn http_service_url_template_method() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "HTTPService.URLTemplate.Method",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
