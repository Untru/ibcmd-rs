//! Form body коннектор (под-IR L1f) — byte-exact read/write тела формы для EDT
//! (`Form.form`, `<form:Form>`) и Designer (`Ext/Form.xml`, `<Form>`). ARCHITECTURE.md
//! §1.2/§1.6.
//!
//! Закрывает ПИЛОТНУЮ форму `CommonForm.ФормаПроизвольногоСообщения` (1 контрол
//! `LabelDecoration`). Тело формы — НЕ дескриптор-объект (нет uuid/InternalInfo); это
//! отдельный per-form файл. Коннектор един для обоих форматов через [`FormDialect`]:
//! envelope, форм-атрибуты (через спек-движок + ПЕР-ФОРМАТНЫЕ дефолты), служебные дети
//! (`autoCommandBar`, `commandInterface`), данные-модель (`attributes`/`parameters` через
//! Type-codec) и контрол `LabelDecoration` с decorator-frame.
//!
//! # §1.0 тотальность
//! Каждый элемент/атрибут обоих файлов либо типизирован в IR, либо вызывает ОШИБКУ.
//! Тотальность сверяется `root.unclaimed_count()==0` по ВСЕМУ дереву формы. Нет
//! Raw/skip/best-effort.

use std::sync::OnceLock;

use crate::emit::Envelope;

mod availability;
mod event_catalog;
mod event_semantics;
mod strict_resource;
mod event_owners;
#[doc(hidden)]
pub use event_owners::is_native_form_event_path;
pub use event_semantics::{EVENT_SEMANTICS_RESOURCE, apply_event_semantics_resource, write_event_semantics_resource, same_event_semantics_resource, event_semantics_resource_count};
mod chart;
mod chart_semantics;
mod series_info;
mod trend_transport;
pub use chart_semantics::{CHART_SEMANTICS_RESOURCE, project_chart_semantics, apply_chart_semantics_resource, same_chart_semantics_resource, chart_semantics_resource_count};
mod dcss;
mod fields;
mod report_refs;
mod wire_order;
pub(crate) use fields::{
    color_from_designer, color_to_designer, decode_edt_color, render_edt_color,
};
mod mxlx;
mod picture_defaults;
mod picture_semantics;
mod pictures;
pub use picture_defaults::resolve_common_picture_transparency;
pub use picture_semantics::{
    PICTURE_SEMANTICS_RESOURCE, apply_native_picture_resource, write_native_picture_resource, attach_choice_picture_assets, choice_picture_assets, bind_picture_semantics, picture_semantics_resource_count,
    project_native_picture_glyphs, project_picture_semantics, read_picture_semantics_resource, same_picture_semantics_resource,
};
mod projection;
mod read;
mod tables;
mod write;

pub use chart::{designer_dense_chart_settings, project_native_trends, native_percentage_integer,native_percentage_projection, percentage_numeric_equal, read_chart_sidecar, write_chart_sidecar};
pub use dcss::{
    DcsSettingsSection, read_conditional_appearance_dcssca, read_list_settings_dcss,
    write_conditional_appearance_dcssca, write_form_settings_blob, write_list_settings_dcss,
    write_list_settings_section, write_server_state,
};
pub use mxlx::{read_spreadsheet_mxlx, write_spreadsheet_mxlx};
pub use pictures::{PictureSlot, picture_slots, set_sidecar_ext, sidecar_slots};
pub use read::read_form;
pub use write::write_form;

/// Какой XML-формат тела формы читает/пишет коннектор.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormDialect {
    /// EDT `Form.form` (`<form:Form>`, sparse, CRLF, 2-space, no BOM, trailing EOL).
    Edt,
    /// Designer `Ext/Form.xml` (`<Form>`, sparse-per-own-defaults, CRLF, TAB, BOM, нет trailing EOL).
    Designer,
}

/// Ошибка форм-коннектора (типизированная, §1.0).
#[derive(Debug)]
pub enum FormError {
    /// Сбой токенизации XML.
    Xml(crate::XmlReadError),
    /// Несоответствие envelope (BOM/EOL/пролог/корень/ns).
    Envelope(String),
    /// Структурная ошибка (неожиданный/неразобранный элемент, §1.0).
    Frame(String),
    /// Ошибка спек-движка.
    Engine(morph1c_core::engine::EngineError),
}

impl std::fmt::Display for FormError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormError::Xml(e) => write!(f, "form: {e}"),
            FormError::Envelope(s) => write!(f, "form: envelope: {s}"),
            FormError::Frame(s) => write!(f, "form: {s}"),
            FormError::Engine(e) => write!(f, "form: {e}"),
        }
    }
}
impl std::error::Error for FormError {}
impl From<crate::XmlReadError> for FormError {
    fn from(e: crate::XmlReadError) -> Self {
        FormError::Xml(e)
    }
}
impl From<morph1c_core::engine::EngineError> for FormError {
    fn from(e: morph1c_core::engine::EngineError) -> Self {
        FormError::Engine(e)
    }
}

// --- envelope-константы ---

/// XML-декларация (общая обоим форматам).
const FORM_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";
/// URI ns `form` (EDT-корень `<form:Form>`).
const FORM_NS_URI: &str = "http://g5.1c.ru/v8/dt/form";
/// URI ns `xsi`.
const XSI_NS_URI: &str = "http://www.w3.org/2001/XMLSchema-instance";
/// URI ns `core` (EDT mcore-типы `core:ColorRef`/`core:PictureRef`/`core:NumberValue`/
/// `core:StringValue`). Корень `<form:Form>` объявляет `xmlns:core` РОВНО когда тело
/// содержит хоть один `core:`-тип (сверено 104/104 CommonForms, 0 расхождений) — позиция
/// фиксирована МЕЖДУ `xmlns:xsi` и `xmlns:form`.
const CORE_NS_URI: &str = "http://g5.1c.ru/v8/dt/mcore";
/// URI ns `schema` (EDT DataCompositionSchema-типы `schema:DataCompositionSchemaDataSetField`).
/// Корень `<form:Form>` объявляет `xmlns:schema` РОВНО когда тело содержит DCS-схема-поля
/// (реквизит-динсписок EXTENDED-формы; присутствует лишь у АдреснаяКнига/ФайлыВТоме) — позиция
/// ПОСЛЕДНЯЯ (после `xmlns:form`; сверено).
const SCHEMA_NS_URI: &str = "http://g5.1c.ru/v8/dt/data-composition-system/schema";
/// URI ns `settings` (EDT DCS-настройки: `settings:SettingsParameterValue` в appearance
/// вычисляемого поля динсписка). Корень `<form:Form>` объявляет `xmlns:settings` РОВНО когда
/// тело несёт `settings:`-тип (ERP-witness ОтклоненияВСтоимостиТоваров.ФормаСписка) — позиция
/// ПОСЛЕДНЯЯ (после `xmlns:form`; в witness'е core, form, settings).
const SETTINGS_NS_URI: &str = "http://g5.1c.ru/v8/dt/data-composition-system/settings";
/// URI ns `mxl` (табличный документ `mxl:SpreadsheetDocument` в `<Settings>` реквизита; Designer
/// объявляет `xmlns:mxl` ИНЛАЙН на самом `<Settings>`, не в корневом ns-блоке).
const MXL_NS_URI: &str = "http://v8.1c.ru/8.2/data/spreadsheet";

/// Envelope EDT-тела формы (no BOM, CRLF, 2 пробела, trailing EOL; `>`/кавычки как `.mdo`).
fn edt_envelope() -> Envelope {
    Envelope {
        bom: false,
        eol: "\r\n",
        indent_unit: "  ",
        decl: FORM_DECL,
        trailing_eol: true,
        escape_gt: false,
        escape_quot: true,
        text_eol: "\r\n",
    }
}

/// Envelope Designer-тела формы (BOM, CRLF, TAB, БЕЗ trailing EOL; `>` экранируется, кавычки нет).
fn designer_envelope() -> Envelope {
    Envelope {
        bom: true,
        eol: "\r\n",
        indent_unit: "\t",
        decl: FORM_DECL,
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "\n",
    }
}

/// Полный фиксированный ns-блок Designer-корня `<Form>` формата **2.21** (SSL 8.5.1) —
/// 18 объявлений ВКЛЮЧАЯ `xmlns:pal`. Порядок и URI сверены по корпусу — воспроизводятся
/// byte-exact.
///
/// Имя оставлено без суффикса версии: этот блок дополнительно служит SCOPE'ом для
/// [`crate::type_codec::encode_scoped`] (инлайн-ns на `<v8:Type>`), где версия НЕ влияет:
/// единственная версионная дельта — `pal` (палитра цветов), а `pal` не является
/// типовым префиксом (инлайн-логика справляется о `dcsset`/`cfg`/`v8ui`/`mxl`/`fd`/`d{N}p1`).
const DESIGNER_FORM_NS: &[(&str, &str)] = &[
    ("xmlns", "http://v8.1c.ru/8.3/xcf/logform"),
    ("xmlns:app", "http://v8.1c.ru/8.2/managed-application/core"),
    (
        "xmlns:cfg",
        "http://v8.1c.ru/8.1/data/enterprise/current-config",
    ),
    (
        "xmlns:dcscor",
        "http://v8.1c.ru/8.1/data-composition-system/core",
    ),
    (
        "xmlns:dcssch",
        "http://v8.1c.ru/8.1/data-composition-system/schema",
    ),
    (
        "xmlns:dcsset",
        "http://v8.1c.ru/8.1/data-composition-system/settings",
    ),
    ("xmlns:ent", "http://v8.1c.ru/8.1/data/enterprise"),
    (
        "xmlns:lf",
        "http://v8.1c.ru/8.2/managed-application/logform",
    ),
    ("xmlns:pal", "http://v8.1c.ru/8.1/data/ui/colors/palette"),
    ("xmlns:style", "http://v8.1c.ru/8.1/data/ui/style"),
    ("xmlns:sys", "http://v8.1c.ru/8.1/data/ui/fonts/system"),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xmlns:web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
    ("xmlns:win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
    ("xmlns:xr", "http://v8.1c.ru/8.3/xcf/readable"),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
];

/// ns-блок Designer-корня `<Form>` формата **2.20** (ERP 8.3.27) — тот же порядок, что у
/// [`DESIGNER_FORM_NS`], но БЕЗ строки `xmlns:pal` (17 объявлений). Сверено по корпусу
/// ERP (`CommonForms/**/Ext/Form.xml` + формы объектов): дельта корня формы 2.20↔2.21
/// та же, что у дескрипторов (`formats_designer::common::NS_BLOCK_ERP`) — отсутствие
/// `pal` + значение `version=`.
/// ЭТАЛОН СВЕРКИ: реестр конвертов формы строится теперь из данных
/// (`designer.form.envelope`), а этот блок остаётся образцом, с которым тест
/// [`tests`] сверяет снятый с дампов ростер 2.17/2.20.
#[cfg(any())]
const DESIGNER_FORM_NS_ERP: &[(&str, &str)] = &[
    ("xmlns", "http://v8.1c.ru/8.3/xcf/logform"),
    ("xmlns:app", "http://v8.1c.ru/8.2/managed-application/core"),
    (
        "xmlns:cfg",
        "http://v8.1c.ru/8.1/data/enterprise/current-config",
    ),
    (
        "xmlns:dcscor",
        "http://v8.1c.ru/8.1/data-composition-system/core",
    ),
    (
        "xmlns:dcssch",
        "http://v8.1c.ru/8.1/data-composition-system/schema",
    ),
    (
        "xmlns:dcsset",
        "http://v8.1c.ru/8.1/data-composition-system/settings",
    ),
    ("xmlns:ent", "http://v8.1c.ru/8.1/data/enterprise"),
    (
        "xmlns:lf",
        "http://v8.1c.ru/8.2/managed-application/logform",
    ),
    ("xmlns:style", "http://v8.1c.ru/8.1/data/ui/style"),
    ("xmlns:sys", "http://v8.1c.ru/8.1/data/ui/fonts/system"),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xmlns:web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
    ("xmlns:win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
    ("xmlns:xr", "http://v8.1c.ru/8.3/xcf/readable"),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
];

/// Один профиль версионной части Designer-конверта формы: `{версия → (version=, ns-блок)}`.
/// Зеркало `formats_designer::common::EnvelopeProfile` для корня `<Form>` (logform), НЕ
/// дескриптора (`MetaDataObject`/MDClasses): у формы СВОЙ дефолтный ns и свой ns-набор.
/// `version_value` == `format.to_string()` (производно от FORMATS.md §2, не второй хардкод).
struct FormEnvelopeProfile {
    /// Версия формата выгрузки (ключ; из `core::version`, FORMATS.md §2).
    format: morph1c_core::version::FormatVersion,
    /// Точное значение атрибута `version=` корня, == `format.to_string()`.
    version_value: &'static str,
    /// ns-блок корня `<Form>` в ТОЧНОМ порядке эталона этой версии.
    ns_block: &'static [(&'static str, &'static str)],
}

/// Имя конструкции «корневая обёртка Designer-формы» в таблице раскладок
/// (`models/format_layouts.jsonl`): ростер ns и версии, в которых он наблюдался.
const FORM_ENVELOPE_CONSTRUCT: &str = "designer.form.envelope";

/// Реестр витнессированных Designer-конвертов формы — ОТВЕТ ДАННЫХ.
///
/// Ни версии, ни ns-ростеры здесь не записаны: они сняты с корней `<Form>` РЕАЛЬНЫХ дампов
/// одной конфигурации, собранной каждой доступной платформой (`tools/build_format_layouts.py`;
/// в 2.17 и 2.20 ростер один — 17 объявлений без `pal`, в 2.21 их 18, ровно как на
/// дескрипторах). Версия, дампа которой нет, в реестр не попадает: неизвестный `version=` на
/// read и не-witnessed таргет на write — громкая ошибка (§1.0: угаданный ns-блок молча дал бы
/// дамп не той версии).
fn form_envelope_profiles() -> &'static [FormEnvelopeProfile] {
    static PROFILES: OnceLock<Vec<FormEnvelopeProfile>> = OnceLock::new();
    PROFILES.get_or_init(|| {
        let mut rows = morph1c_core::version::layout::rows(FORM_ENVELOPE_CONSTRUCT);
        rows.sort_by_key(|l| l.version());
        rows.into_iter()
            .map(|l| FormEnvelopeProfile {
                format: l.version(),
                version_value: Box::leak(l.version().to_string().into_boxed_str()),
                ns_block: l.ns,
            })
            .collect()
    })
}

/// Профиль конверта формы для таргет-версии ПИСАТЕЛЯ (`None` — не witnessed → ошибка выше).
fn form_profile_for(
    format: morph1c_core::version::FormatVersion,
) -> Option<&'static FormEnvelopeProfile> {
    form_envelope_profiles().iter().find(|p| p.format == format)
}

/// Детект профиля конверта по значению корневого `version=` — версионный вход РИДЕРА
/// (FORMATS.md §1: формат ВХОДА детектится по самому файлу). `None` — не witnessed.
fn detect_form_profile(version_value: &str) -> Option<&'static FormEnvelopeProfile> {
    form_envelope_profiles()
        .iter()
        .find(|p| p.version_value == version_value)
}

/// Человекочитаемый список витнессированных версий конверта формы (для ошибок).
fn witnessed_form_versions() -> String {
    form_envelope_profiles()
        .iter()
        .map(|p| p.version_value)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Каноническое (EDT) имя ФОРМ-уровневой автокомандной панели. EDT эмитит его явно
/// (`<name>FormCommandBar</name>`); Designer сериализует ту же фиксированную служебную панель с
/// ПУСТЫМ именем. Канонизуется к EDT-написанию в `read_designer` и регенерируется (пустое) в
/// `write_designer` ⇒ под-IR формат-нейтрален (ср. `testkit::normalize_form_for_x`, которая
/// зануляла это имя для X). Совпадает с cf-энкодерным `witnessed_auto_command_bar().name`.
pub(crate) const FORM_COMMAND_BAR_NAME: &str = "FormCommandBar";

#[cfg(any())]
mod tests;

#[doc(hidden)]
pub use chart::normalize_big_decimal;
