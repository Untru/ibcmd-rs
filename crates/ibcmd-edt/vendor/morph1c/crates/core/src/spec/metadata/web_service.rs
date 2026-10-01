//! Канонический спек вида объекта `WebService` (Web-сервис) — ARCHITECTURE.md
//! §1.4/§1.6, child-objects substrate. RECURSION-несущий
//! вид: доказывает рекурсивный субстрат на ДВУХ уровнях (`WebService` → `Operation` →
//! `Parameter`), inline-дети в обоих XML-форматах (как `HTTPService`), плюс НОВЫЙ
//! субстрат: `xdtoPackages` (типизированный value-list) и xdto-type-ref (пара
//! `name`+`nsUri`) у операций/параметров.
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`). Формат хранит лишь ПРОЕКЦИЮ; здесь —
//! канонический `id`/`value_kind`/`default`/нормализация/ПОРЯДОК + СЛОТ дочерней
//! коллекции `Operation → WebService.Operation`.
//!
//! Спек-свойства (скан SSL, 13 объектов edt+designer). Порядок = DENSE-порядок Designer
//! `<Properties>`: `Synonym, Comment, Namespace, XDTOPackages, DescriptorFileName,
//! ReuseSessions, SessionMaxAge`. EDT эмитит РАЗРЕЖЁННО (опускает дефолтные
//! `comment`/`reuseSessions`/пустой `xdtoPackages`), Designer — DENSE.
//!
//! ВНЕ спека: `name`/`uuid` — идентичность. Дочерние `operations` — НЕ свойства, а
//! [`ChildSlot`] (рекурсия движка). Тело модуля `Module.bsl` — отдельный артефакт.
//! Поля `objectBelonging`/`extendedConfigurationObject` метамодели в SSL-корпусе НЕ
//! встречены (0 occ) → в спек НЕ включены (§1.0: только засвидетельствованное).

use crate::ir::value::{PropertyValue, Token, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `namespace` — целевое пространство имён Web-сервиса (URI). Required.
pub const F_NAMESPACE: FieldId = FieldId(3);
/// `xdtoPackages` — СПИСОК типизированных (`String`/`Reference`) ссылок на XDTO-пакеты.
/// Default = пустой список. IR — `List([List([Str(variant), Str(value)])])`.
pub const F_XDTO_PACKAGES: FieldId = FieldId(4);
/// `descriptorFileName` — имя дескриптор-файла (`.1cws`). Required.
pub const F_DESCRIPTOR_FILE_NAME: FieldId = FieldId(5);
/// `reuseSessions` — режим повторного использования сессий. Default = `DontUse` (EDT
/// его ВСЕГДА опускает; Designer DENSE эмитит).
pub const F_REUSE_SESSIONS: FieldId = FieldId(6);
/// `sessionMaxAge` — максимальный возраст сессии (целое, секунды). Required.
pub const F_SESSION_MAX_AGE: FieldId = FieldId(7);

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
        FieldSpec::required(F_NAMESPACE, "namespace", ValueKind::Str),
        FieldSpec::with_default(
            F_XDTO_PACKAGES,
            "xdtoPackages",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        ),
        FieldSpec::required(F_DESCRIPTOR_FILE_NAME, "descriptorFileName", ValueKind::Str),
        FieldSpec::with_default(
            F_REUSE_SESSIONS,
            "reuseSessions",
            ValueKind::Enum,
            PropertyValue::Enum(Token::new("DontUse")),
        ),
        FieldSpec::required(F_SESSION_MAX_AGE, "sessionMaxAge", ValueKind::Int),
    ]
}

/// Слоты дочерних коллекций `WebService`: единственная — `Operation` (recursion).
const CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "Operation", child_kind: "WebService.Operation" }];

/// Канонический [`EntitySpec`] вида `WebService` (кэш на процесс).
pub fn web_service() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "WebService",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}
