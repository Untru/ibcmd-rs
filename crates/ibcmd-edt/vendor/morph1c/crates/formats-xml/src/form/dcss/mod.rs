//! EDT-сайдкар DCS-НАСТРОЕК динамического списка —
//! `Forms/<N>/Attributes/<attr>/ExtInfo/ListSettings.dcss` (byte-exact read/write).
//!
//! # Асимметрия локусов (RE: SSL 227 сайдкаров / 229 динсписков)
//! Designer держит настройки ИНЛАЙН в `Ext/Form.xml` — `<ListSettings>` внутри
//! `<Settings xsi:type="DynamicList">` реквизита ([`DcsListSettings`], читает/пишет форм-кодек).
//! EDT в `Form.form` НЕ несёт их ВООБЩЕ (ни тега, ни маркера) — только ЭТОТ сайдкар. Отсутствие
//! сайдкара ⇒ настройки ПУСТЫ (`<ListSettings/>` на Designer-стороне; witness ×2
//! `InformationRegister.ВерсииПодсистем*.ФормаСписка`). Транскодинг inline⟷сайдкар выполняет
//! pipeline (`form_read`/`form_write`) через общий под-IR [`DcsListSettings`].
//!
//! # Сериализации отличаются РОВНО префиксом (сверено 1:1 по всем 227 парам корпуса)
//! Сайдкар держит settings-ns ДЕФОЛТНЫМ (`xmlns="…/data-composition-system/settings"` на корне
//! `<Settings>`), Designer — под префиксом `dcsset:`. Полная перепись элементного ценза обоих
//! корпусов совпадает ЭЛЕМЕНТ-В-ЭЛЕМЕНТ после отображения `""` ⟼ `dcsset` (26 имён, 3298 узлов):
//! `filter`/`order`/`conditionalAppearance`/`item`/`field`/`orderType`/`use`/`left`/`right`/… .
//! Прочие ns (`dcscor:`, `v8:`, `v8ui:`, `xs:`) в сайдкаре ПРЕФИКСОВАНЫ как в Designer.
//! Значения `xsi:type` — та же биекция: `OrderItemField` ⟺ `dcsset:OrderItemField` (безпрефиксные
//! = settings-ns), `dcscor:Field`/`xs:boolean`/`v8:Type` — как есть.
//!
//! ВТОРАЯ (и последняя) префиксная разница — АВТО-префикс инлайн-объявления types-ns у
//! `<right xsi:type="v8:Type">`: сериализатор нумерует его ПО ЛОКУСУ — Designer пишет
//! `xmlns:d8p1=…/8.2/data/types` + текст `d8p1:Undefined`, сайдкар — `d4p1`/`d4p1:Undefined`
//! (единственный витнесс обоих локусов: `DocumentJournal.Взаимодействия.ФормаСписка`, по 2
//! вхождения). Логическое значение ОДНО (тип `Undefined` в types-ns) — префикс лишь метка, и
//! адаптер отображает `d4p1` ⟺ `d8p1` ровно так же, как `""` ⟺ `dcsset`. Канон IR — DESIGNER-форма
//! QName ([`morph1c_core::ir::form::DcsRightValue::TypeQName`] хранит `d8p1:Undefined`), сайдкар —
//! её перепрефиксовка. Чтение ТОТАЛЬНО (принимаем ЛЮБОЙ префикс с этим URI), запись пришпилена к
//! витнессу (`d4p1`) — расхождение ловится survey-роундтрипом ГРОМКО, не молча.
//!
//! Отсюда кодек — ТОНКИЙ адаптер: он ПЕРЕПРЕФИКСИРУЕТ распарсенное дерево и делегирует
//! [`super::read::read_designer_list_settings`] (и обратно —
//! [`super::write::designer_list_settings`] + обратная перепрефиксация). Никакого второго парсера
//! DCS-настроек: одна типизация, один writer, доказуемо симметричные обёртки.
//!
//! # Envelope (hexdump-сверено, 227/227 единообразны)
//! Без BOM, CRLF, TAB-отступ, С trailing EOL, ns-блок корня фиксирован (11 объявлений, одна
//! строка на все 227 файлов). Политика escape — Designer-ская (`>`→`&gt;`, кавычка литеральная);
//! в корпусе спец-символов нет (0 escape-последовательностей).

use super::read::{claim_root_ns, read_dcs_item, read_designer_list_settings, unclaimed_labels};
use super::write::{
    designer_dcs_calculated_field, designer_dcs_field, designer_dcs_group_children,
    designer_dcs_item, designer_dcs_parameter, designer_dcs_settings_parameter_value,
    designer_list_settings,
};
use super::{FormError, XSI_NS_URI};
use crate::descriptor::Element;
use crate::emit::{render, Envelope, OutElement};
use crate::{parse, EolStyle};
use morph1c_core::ir::form::{
    DcsCalculatedField, DcsField, DcsItem, DcsListSettings, DcsParameter, DcsSettingsGroup,
};

mod form_settings;
mod list_settings;
mod reprefix;
mod sections;
mod server_state;
#[cfg(any())]
mod tests;

pub use form_settings::*;
pub use list_settings::*;
pub use sections::*;
pub use server_state::*;
pub(crate) use reprefix::*;

/// XML-декларация сайдкара (та же, что у форм).
const DCSS_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// URI ns настроек DCS (в сайдкаре — ДЕФОЛТНЫЙ; в Designer — под префиксом `dcsset`).
const DCSSET_NS_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/settings";

/// Префикс, под которым тот же settings-ns живёт в Designer-инлайне.
const DCSSET: &str = "dcsset";

/// URI ns типов (`v8:Type`-значение отбора объявляет его ИНЛАЙН на самом `<right>`).
const TYPES_NS_URI: &str = "http://v8.1c.ru/8.2/data/types";
/// Авто-префикс types-ns в DESIGNER-локусе (канон IR — см. модульный docstring).
const TYPES_PREFIX_DESIGNER: &str = "d8p1";
/// Авто-префикс types-ns в САЙДКАР-локусе (witness Взаимодействия.ФормаСписка ×2).
const TYPES_PREFIX_DCSS: &str = "d4p1";

/// URI ns схемы XSD (`xs`).
const XS_NS_URI: &str = "http://www.w3.org/2001/XMLSchema";
/// URI ns core DCS (Designer-префикс `dcscor`).
const DCSCOR_NS_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/core";
/// URI ns данных 8.1 (Designer-префикс `v8`) — в секции получает АВТО-префикс.
const V8_NS_URI: &str = "http://v8.1c.ru/8.1/data/core";
/// URI ns UI 8.1 (Designer-префикс `v8ui`) — в секции получает АВТО-префикс.
const V8UI_NS_URI: &str = "http://v8.1c.ru/8.1/data/ui";

/// URI ns СХЕМЫ DCS (Designer-префикс `dcssch`) — ns полей/параметров data-set'а.
const DCSSCH_NS_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";

/// URI ns СТИЛЕЙ (`style:`) — платформенные стили и config-`StyleItem`-ссылки цветов/шрифтов.
const STYLE_NS_URI: &str = "http://v8.1c.ru/8.1/data/ui/style";

/// URI ns СИСТЕМНЫХ ШРИФТОВ (`sys:`, `sys:DefaultGUIFont` и т.п.).
const SYS_FONTS_NS_URI: &str = "http://v8.1c.ru/8.1/data/ui/fonts/system";

/// URI ns DCS-COMMON — дети `<orderExpression>` DCS-вычисляемого-поля (`expression`/`orderType`/
/// `autoOrder`) объявляют его ДЕФОЛТНЫМ (`xmlns="…/common"`) ИНЛАЙН на каждом ребёнке. Платформа
/// хранит его в кэше ВЕРБАТИМ (witness ОперацииСПодключаемымОборудованием) — [`reprefix`]
/// пере-объявляет его локально, а [`check_cache_qnames`] его ПРОПУСКАЕТ.
const DCS_COMMON_NS_URI: &str = "http://v8.1c.ru/8.1/data-composition-system/common";

/// Резолвер ССЫЛОК КОНФИГУРАЦИИ, нужных внутри cf-блоба: имя `StyleItem` ⟼ его object-uuid.
///
/// Ссылки, которые cf-локус ПЕРЕ-кодирует (всё остальное — `DesignTimeValue`
/// вроде `Перечисление.ТехническиеСтатусыМЧД.Отменена` — уезжает в блоб ВЕРБАТИМ):
/// * ЦВЕТ условного оформления: Designer/сайдкар пишут `<dcscor:value xsi:type="v8ui:Color">
///   style:ТекстЗапрещеннойЯчейкиЦвет`, cf — `0:<uuid элемента стиля>` (тот же дискриминатор
///   `0`+uuid, что у brace-узла цвета `{4,3,{0,<uuid>},3}`; witness МашиночитаемыеДоверенности —
///   и ssl.cf (8.5.1), и erp.cf (8.3.27) переписывают ОДИНАКОВО ⇒ без версионного гейта);
/// * ШРИФТ условного оформления: `<dcscor:value xsi:type="v8ui:Font" ref="style:<Имя>">` ⟼
///   `ref="0:<uuid>"` (witness erp.cf ВажнаяНадписьШрифт/ЗаголовокУдаленногоРеквизитаШрифт).
/// НЕразрешимое имя (`None`) = ПЛАТФОРМЕННЫЙ стиль (`style:NormalTextFont`,
/// `style:SpecialTextColor`) — уезжает ВЕРБАТИМ (тот же дискриминатор, что у
/// `codecs::color_node`: registry None → platform).
/// Резолвер даёт `formats-cf` (реестр контейнера); formats-xml сам реестра не знает.
pub type StyleItemUuid<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Envelope `ListSettings.dcss` (см. модульный docstring).
fn dcss_envelope() -> Envelope {
    Envelope {
        bom: false,
        eol: "\r\n",
        indent_unit: "\t",
        decl: DCSS_DECL,
        trailing_eol: true,
        escape_gt: true,
        escape_quot: false,
        text_eol: "\n",
    }
}

/// Envelope XML-документа секции: BOM, CRLF, TAB, БЕЗ trailing EOL (сверено побайтно —
/// пустая `<Order …/>` ровно 210 B).
fn section_envelope() -> Envelope {
    Envelope {
        bom: true,
        eol: "\r\n",
        indent_unit: "\t",
        decl: DCSS_DECL,
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "\n",
    }
}
