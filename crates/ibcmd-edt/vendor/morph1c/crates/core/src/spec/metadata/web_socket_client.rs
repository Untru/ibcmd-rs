//! Канонический спек вида объекта `WebSocketClient` (WebSocket-клиент) — ARCHITECTURE.md
//! §1.4/§1.6. Лист-вид (сервис). Один [`EntitySpec`] на вид: движок выводит read/write для
//! КАЖДОГО формата (EDT `.mdo`, Designer `.xml`); cf — зеркальная проекция.
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта, фреймится каркасом.
//!
//! Поля (сверено корпусом `coverage/s9_new`, 9 объектов edt+designer+cf) в каноническом
//! DENSE-порядке Designer: `synonym`, `comment`, `predefined`, `autoConnect`, `serverURL`,
//! `user`, `password`, `headers`, `useOSProxy`, `useOSAuthentication`, `timeout`. EDT эмитит
//! РАЗРЕЖЁННО (дефолты опущены), Designer — DENSE. objectBelonging/extendedConfigurationObject
//! по метамодели существуют, но в корпусе НЕ витнессятся → в спек НЕ включены (§1.0).
//!
//! `headers` — список HTTP-заголовков (`ValueList`); в корпусе всегда ПУСТ. Designer DENSE
//! эмитит `<Headers xsi:type="xr:ValueList"/>` (кодек [`formats_xml::Codec::EmptyValueList`]),
//! EDT его НЕ проецирует (в корпусе нет `<headers>`), cf — константный `{0}`. Смоделирован
//! как [`ValueKind::List`] с default `[]`: все три формата дают ПУСТОЙ список → сжимается в
//! дефолт (X by construction). Появится непустой — расширить кодек структурно.
//!
//! `timeout` — целое (таймаут, сек). REQUIRED (без дефолта): host present ВСЕГДА (EDT эмитит
//! даже дефолтные `30`; Designer/cf — DENSE). Не сжимается в дефолт → byte-exact R держится.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `predefined` — предопределённый (bool). Default `false`.
pub const F_PREDEFINED: FieldId = FieldId(3);
/// `autoConnect` — автоподключение (bool). Default `false`.
pub const F_AUTO_CONNECT: FieldId = FieldId(4);
/// `serverURL` — URL сервера (свободный текст). Default `""`.
pub const F_SERVER_URL: FieldId = FieldId(5);
/// `user` — пользователь (свободный текст). Default `""`.
pub const F_USER: FieldId = FieldId(6);
/// `password` — пароль (свободный текст). Default `""`.
pub const F_PASSWORD: FieldId = FieldId(7);
/// `headers` — список заголовков (`ValueList`); в корпусе всегда пуст. Default `[]`.
pub const F_HEADERS: FieldId = FieldId(8);
/// `useOSProxy` — использовать прокси ОС (bool). Default `false`.
pub const F_USE_OS_PROXY: FieldId = FieldId(9);
/// `useOSAuthentication` — использовать аутентификацию ОС (bool). Default `false`.
pub const F_USE_OS_AUTHENTICATION: FieldId = FieldId(10);
/// `timeout` — таймаут (int). Required: host present всегда (EDT эмитит даже дефолт).
pub const F_TIMEOUT: FieldId = FieldId(11);

fn localized_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Localized, PropertyValue::Localized(Vec::new()))
        .normalized(Normalize::LocalizedSortByLang)
}
fn str_empty(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}
fn bool_field(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}

/// Сконструировать [`FieldSpec`] вида `WebSocketClient` в каноническом DENSE-порядке Designer.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        localized_field(F_SYNONYM, "synonym"),
        str_empty(F_COMMENT, "comment"),
        bool_field(F_PREDEFINED, "predefined"),
        bool_field(F_AUTO_CONNECT, "autoConnect"),
        str_empty(F_SERVER_URL, "serverURL"),
        str_empty(F_USER, "user"),
        str_empty(F_PASSWORD, "password"),
        // headers — пустой ValueList; kind-зависимый кодек (auto-derive его НЕ выводит),
        // Designer маппит на `Codec::EmptyValueList` вручную, EDT/cf — см. коннекторы.
        FieldSpec::with_default(F_HEADERS, "headers", ValueKind::List, PropertyValue::List(Vec::new())),
        bool_field(F_USE_OS_PROXY, "useOSProxy"),
        bool_field(F_USE_OS_AUTHENTICATION, "useOSAuthentication"),
        // timeout — REQUIRED: host present всегда (не сжимается в дефолт).
        FieldSpec::required(F_TIMEOUT, "timeout", ValueKind::Int),
    ]
}

/// Канонический [`EntitySpec`] вида `WebSocketClient` (кэш на процесс).
pub fn web_socket_client() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "WebSocketClient",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // лист-вид: подчинённых коллекций нет.
    })
}

/// cf class-guid вида `WebSocketClient` (группа корня `.cf`; = строка KIND_TABLE в
/// `formats-cf/configuration_root.rs`). RE-проверено на `coverage/cf/s9_new.cf`.
pub const ENTITY_GUID: &str = "a7641777-7813-45c6-96ef-9d51587a6ac6";
