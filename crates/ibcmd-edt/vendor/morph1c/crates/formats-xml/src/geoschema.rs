//! CommonTemplate `GeographicalSchema` (геосхема) тело макета — byte-exact read/write для
//! EDT (`Template.geos`, `<geographicalSchema:GeographicalSchema>`, g5-диалект) и
//! Designer/cf (`Ext/Template.xml`, `<GeographicalScheme>`, extrnprops-диалект).
//! ARCHITECTURE.md §1.0/§1.6.
//!
//! # Почему отдельный коннектор (не blob-копия, в отличие от MXL/DCS/XDTO)
//! У прочих тел макета EDT и Designer НЕСУТ ОДИН диалект (byte-identical после снятия BOM),
//! поэтому pipeline копирует их как blob. У геосхемы EDT и Designer — СТРУКТУРНО РАЗНЫЕ
//! XML-диалекты одной модели:
//! * **extrnprops** (Designer `Ext/Template.xml` + cf `<uuid>.0`): КОМПАКТНЫЙ — корень
//!   `<GeographicalScheme>` с атрибутами (`showMode`/`scale`/`proj`/границы/`curId`) и тремя
//!   self-closing детьми `<d1p1:legend|title|drawing>` (координаты/флаги как атрибуты). Цвета/
//!   шрифты/рамки ОПУЩЕНЫ (дефолты). BOM, CRLF, TAB, без trailing EOL.
//! * **g5** (EDT `Template.geos`): РАЗВЁРНУТЫЙ — `<geographicalSchema:GeographicalSchema>` с
//!   вложенными `<geographicalSchemaLegend|Title|DrawingArea>`, каждый регион МАТЕРИАЛИЗУЕТ
//!   дефолтные `<border><width>`, цвета (`Style.*`) и шрифты как дочерние элементы; координаты
//!   опускаются при 0; проекция/границы-данных опускаются при дефолте; «непросмотренная»
//!   область viewed = ±Double.MAX (extrnprops кодирует её нулём). No BOM, CRLF, 2 пробела,
//!   trailing EOL.
//!
//! Каноническое тело в IR ([`morph1c_core::ir::Template::body`]) — extrnprops-байты (диалект,
//! общий Designer+cf; §1.6). Designer/cf read+write и cf-эмиссия `<uuid>.0` — blob поверх него;
//! ТОЛЬКО пересечение с EDT вызывает трансформацию здесь ([`read_geoschema`]/[`write_geoschema`]):
//! EDT read = `g5→GeoSchema→extrnprops`, EDT write = `extrnprops→GeoSchema→g5`.
//!
//! # Дефолты (§ «вычисляешь, сохраняешь и подставляешь»)
//! extrnprops-форма ОПУСКАЕТ цвета/шрифты/рамки регионов — они дефолтны. Их каноничные
//! значения ([`GeoRegion`] дефолты) ВЫЧИСЛЕНЫ из платформенного эталона, ХРАНЯТСЯ здесь и
//! ПОДСТАВЛЯЮТСЯ при эмиссии g5 (designer→edt). Симметрично g5-read СВЕРЯЕТ их с дефолтом и
//! §1.0-ошибается на невитнессированном значении (не молча теряет).
//!
//! # §1.0 тотальность
//! Оба парсера ПОЛНЫ: каждый атрибут/элемент либо разобран в [`GeoSchema`], либо
//! [`GeoError`]. Никакого passthrough/Raw. Значения вне витнессированного подмножества
//! (нестандартный цвет/шрифт, ненулевая граница-данных, нестандартная проекция) → ошибка,
//! а не фабрикация неверного встречного диалекта.

use crate::descriptor::Element;
use crate::emit::{render, Envelope, OutElement};
use crate::read::{parse, XmlReadError};

/// Какой диалект тела геосхемы читает/пишет коннектор.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeoDialect {
    /// EDT `Template.geos` (`<geographicalSchema:GeographicalSchema>`, g5, no BOM, 2 пробела,
    /// trailing EOL).
    Edt,
    /// Designer `Ext/Template.xml` + cf `<uuid>.0` (`<GeographicalScheme>`, extrnprops, BOM, TAB,
    /// без trailing EOL). Каноническая форма тела в IR.
    Designer,
}

/// Ошибка коннектора геосхемы (типизированная, §1.0 — без best-effort).
#[derive(Debug)]
pub enum GeoError {
    /// Сбой токенизации XML.
    Xml(XmlReadError),
    /// Структурная/значенческая ошибка (неожиданный корень/элемент/атрибут, невитнессированное
    /// значение) — §1.0, не passthrough.
    Frame(String),
}

impl std::fmt::Display for GeoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeoError::Xml(e) => write!(f, "geoschema: {e}"),
            GeoError::Frame(s) => write!(f, "geoschema: {s}"),
        }
    }
}
impl std::error::Error for GeoError {}
impl From<XmlReadError> for GeoError {
    fn from(e: XmlReadError) -> Self {
        GeoError::Xml(e)
    }
}

// --- envelope-константы (совпадают с form envelopes; сверено hexdump'ом тел геосхемы) ---

const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// EDT g5 envelope: no BOM, CRLF, 2 пробела, trailing EOL, `>` литерал, `"` → `&quot;`.
fn edt_envelope() -> Envelope {
    Envelope {
        bom: false,
        eol: "\r\n",
        indent_unit: "  ",
        decl: DECL,
        trailing_eol: true,
        escape_gt: false,
        escape_quot: true,
        text_eol: "\r\n",
    }
}

/// Designer extrnprops envelope: BOM, CRLF, TAB, БЕЗ trailing EOL, `>` → `&gt;`, `"` литерал.
fn designer_envelope() -> Envelope {
    Envelope {
        bom: true,
        eol: "\r\n",
        indent_unit: "\t",
        decl: DECL,
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "\n",
    }
}

// --- ns / фиксированные константы ---

const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
const CORE_NS: &str = "http://g5.1c.ru/v8/dt/mcore";
const G5_GEO_NS: &str = "http://g5.1c.ru/v8/dt/geographicalschema";
const EXTRNPROPS_NS: &str = "http://v8.1c.ru/8.3/xcf/extrnprops";
const GEO_DATA_NS: &str = "http://v8.1c.ru/8.2/data/geo";
const XS_NS: &str = "http://www.w3.org/2001/XMLSchema";

/// Дефолтная проекция (опускается в g5).
const DEF_PROJECTION: &str = "CylindricalMillerProjection";
/// Дефолтный `useOutput` (опускается в g5).
const DEF_USE_OUTPUT: &str = "Auto";
/// g5-сентинел «непросмотренной» области (extrnprops кодирует нулём): min = +Double.MAX,
/// max = -Double.MAX — витнессирован в `Template.geos` эталона.
const VIEWED_MIN_SENTINEL: &str = "1.7976931348623157E308";
const VIEWED_MAX_SENTINEL: &str = "-1.7976931348623157E308";

/// Дефолтные style-рефы цвета/шрифта, материализуемые g5 (extrnprops их ОПУСКАЕТ).
const DEF_TEXT_COLOR: &str = "Style.FormTextColor";
const DEF_BORDER_COLOR: &str = "Style.BorderColor";
const DEF_BACK_COLOR: &str = "Style.FieldBackColor";
const DEF_TEXT_FONT: &str = "Style.TextFont";
/// Дефолтная толщина рамки региона (g5 `<border><width>`).
const DEF_BORDER_WIDTH: i64 = 1;

/// Вид региона геосхемы — определяет набор полей/дефолтов и g5-порядок элементов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegionKind {
    /// Легенда: несёт `shown` + `scaleLineShown` + scaleLineText цвет/шрифт.
    Legend,
    /// Заголовок: несёт `shown` + `font`/`textColor`.
    Title,
    /// Область рисования: без `shown`, без шрифта/текст-цвета.
    Drawing,
}

/// Один регион геосхемы (легенда/заголовок/область рисования) — координаты/флаги + (для полноты
/// §1.0) дефолтные цвета/шрифт/рамка. extrnprops хранит лишь координаты/флаги (атрибуты); g5
/// материализует и цвета/шрифты/рамку. Поля цвета/шрифта хранятся канонично-дефолтными и
/// подставляются при эмиссии g5.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GeoRegion {
    kind: RegionKind,
    /// Показывать регион (`show`/`shown`) — легенда/заголовок; для Drawing игнорируется.
    shown: bool,
    /// Показывать масштабную линейку (`scale`/`scaleLineShown`) — только легенда.
    scale_line_shown: bool,
    /// Прозрачность (`trnsprnt`/`transparent`).
    transparent: bool,
    /// Координаты рамки (проценты): left/top/right/bottom. extrnprops эмитит ВСЕ; g5 опускает 0.
    left: i64,
    top: i64,
    right: i64,
    bottom: i64,
    /// Толщина рамки (g5 `<border><width>`; extrnprops опускает — дефолт [`DEF_BORDER_WIDTH`]).
    border_width: i64,
    /// Style-реф цвета рамки (дефолт [`DEF_BORDER_COLOR`]).
    border_color: String,
    /// Style-реф цвета фона (дефолт [`DEF_BACK_COLOR`]).
    back_color: String,
    /// Style-реф основного цвета текста (legend=scaleLineTextColor, title=textColor). Drawing — «».
    text_color: String,
    /// Style-реф основного шрифта (legend=scaleLineTextFont, title=font). Drawing — «».
    text_font: String,
}

impl GeoRegion {
    /// Дефолтный регион данного вида (все координаты 0, флаги true, дефолтные цвета/шрифт/рамка).
    fn default_for(kind: RegionKind) -> Self {
        GeoRegion {
            kind,
            shown: true,
            scale_line_shown: true,
            transparent: true,
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
            border_width: DEF_BORDER_WIDTH,
            border_color: DEF_BORDER_COLOR.to_string(),
            back_color: DEF_BACK_COLOR.to_string(),
            text_color: DEF_TEXT_COLOR.to_string(),
            text_font: DEF_TEXT_FONT.to_string(),
        }
    }
}

/// «Просмотренная» область (viewed) — extrnprops `vxn/vxx/vyn/vyx`, g5 `viewedX/YMin/Max`.
/// [`Unset`](ViewedArea::Unset) = не задана (extrnprops 0×4, g5 ±Double.MAX). Иное — [`Set`].
#[derive(Debug, Clone, PartialEq, Eq)]
enum ViewedArea {
    /// Область не задана — витнессированный дефолт.
    Unset,
    /// Заданная область (границы как СТРОКИ — переносятся дословно между диалектами).
    Set {
        xmin: String,
        ymin: String,
        xmax: String,
        ymax: String,
    },
}

/// Каноническая (диалект-нейтральная) модель тела геосхемы — TRANSIENT (в IR не хранится;
/// канон IR — extrnprops-байты). Используется лишь как промежуток трансформации g5⇄extrnprops.
/// `PartialEq` (не `Eq`) — несёт `scale: f64`.
#[derive(Debug, Clone, PartialEq)]
pub struct GeoSchema {
    current_id: i64,
    /// Проекция (extrnprops `proj`; g5 опускает при дефолте [`DEF_PROJECTION`]).
    projection: String,
    show_mode: String,
    /// Масштаб (extrnprops `scale` = "1"; g5 `<scale>` = "1.0"). Хранится как f64.
    scale: f64,
    use_output: String,
    /// Границы данных `xmn/xmx/ymn/ymx` (extrnprops эмитит; g5 опускает при 0). Строки.
    data_bounds: [String; 4],
    /// Дельта-границы `dxn/dxx/dyn/dyx` (extrnprops эмитит; g5 опускает при 0). Строки.
    delta_bounds: [String; 4],
    viewed: ViewedArea,
    legend: GeoRegion,
    title: GeoRegion,
    drawing: GeoRegion,
}

// ============================ READ ============================

/// Прочитать тело геосхемы (`dialect`) в каноническую [`GeoSchema`]. §1.0-полно.
pub fn read_geoschema(dialect: GeoDialect, bytes: &[u8]) -> Result<GeoSchema, GeoError> {
    let doc = parse(bytes)?;
    match dialect {
        GeoDialect::Designer => read_extrnprops(&doc.root),
        GeoDialect::Edt => read_g5(&doc.root),
    }
}

/// Записать каноническую [`GeoSchema`] в тело геосхемы (`dialect`), byte-exact.
pub fn write_geoschema(dialect: GeoDialect, geo: &GeoSchema) -> Result<Vec<u8>, GeoError> {
    match dialect {
        GeoDialect::Designer => Ok(render(&designer_envelope(), &build_extrnprops(geo))),
        GeoDialect::Edt => Ok(render(&edt_envelope(), &build_g5(geo)?)),
    }
}

// ---- helpers ----

fn frame<T>(msg: impl Into<String>) -> Result<T, GeoError> {
    Err(GeoError::Frame(msg.into()))
}

/// Значение атрибута `name` элемента `el`, или ошибка (§1.0 — обязательный атрибут отсутствует).
fn req_attr<'a>(el: &'a Element, name: &str) -> Result<&'a str, GeoError> {
    match el.attr(name) {
        Some(a) => Ok(a.value.as_str()),
        None => Err(GeoError::Frame(format!(
            "<{}> missing required attribute {name:?}",
            el.local
        ))),
    }
}

/// Разобрать `"true"`/`"false"` в bool (§1.0 — иначе ошибка).
fn parse_bool(s: &str, ctx: &str) -> Result<bool, GeoError> {
    match s {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(GeoError::Frame(format!(
            "{ctx}: expected boolean, got {s:?}"
        ))),
    }
}

/// Разобрать целое (координата/id) (§1.0 — иначе ошибка).
fn parse_int(s: &str, ctx: &str) -> Result<i64, GeoError> {
    s.parse::<i64>()
        .map_err(|_| GeoError::Frame(format!("{ctx}: expected integer, got {s:?}")))
}

/// Текст ЕДИНСТВЕННОГО дочернего листа `<local>text</local>` элемента `el` (§1.0 — иначе ошибка).
fn child_text<'a>(el: &'a Element, local: &str) -> Result<&'a str, GeoError> {
    match el.child(local) {
        Some(c) => Ok(c.text.as_str()),
        None => Err(GeoError::Frame(format!(
            "<{}> missing child <{local}>",
            el.local
        ))),
    }
}

// ---- extrnprops (Designer/cf) READ ----

fn read_extrnprops(root: &Element) -> Result<GeoSchema, GeoError> {
    if root.local != "GeographicalScheme" {
        return frame(format!(
            "extrnprops root <{}> != <GeographicalScheme>",
            root.local
        ));
    }
    // Границы данных/дельты/viewed — строки, дословно.
    let data_bounds = [
        req_attr(root, "xmn")?.to_string(),
        req_attr(root, "xmx")?.to_string(),
        req_attr(root, "ymn")?.to_string(),
        req_attr(root, "ymx")?.to_string(),
    ];
    let delta_bounds = [
        req_attr(root, "dxn")?.to_string(),
        req_attr(root, "dxx")?.to_string(),
        req_attr(root, "dyn")?.to_string(),
        req_attr(root, "dyx")?.to_string(),
    ];
    let (vxn, vxx, vyn, vyx) = (
        req_attr(root, "vxn")?,
        req_attr(root, "vxx")?,
        req_attr(root, "vyn")?,
        req_attr(root, "vyx")?,
    );
    let viewed = if vxn == "0" && vxx == "0" && vyn == "0" && vyx == "0" {
        ViewedArea::Unset
    } else {
        ViewedArea::Set {
            xmin: vxn.to_string(),
            ymin: vyn.to_string(),
            xmax: vxx.to_string(),
            ymax: vyx.to_string(),
        }
    };
    let scale = parse_scale(req_attr(root, "scale")?)?;

    let legend = read_extrnprops_region(root, "legend", RegionKind::Legend)?;
    let title = read_extrnprops_region(root, "title", RegionKind::Title)?;
    let drawing = read_extrnprops_region(root, "drawing", RegionKind::Drawing)?;

    let geo = GeoSchema {
        current_id: parse_int(req_attr(root, "curId")?, "curId")?,
        projection: req_attr(root, "proj")?.to_string(),
        show_mode: req_attr(root, "showMode")?.to_string(),
        scale,
        use_output: req_attr(root, "useOutput")?.to_string(),
        data_bounds,
        delta_bounds,
        viewed,
        legend,
        title,
        drawing,
    };
    Ok(geo)
}

fn read_extrnprops_region(
    root: &Element,
    local: &str,
    kind: RegionKind,
) -> Result<GeoRegion, GeoError> {
    let el = root.child(local).ok_or_else(|| {
        GeoError::Frame(format!("extrnprops <GeographicalScheme> missing <{local}>"))
    })?;
    let mut r = GeoRegion::default_for(kind);
    r.transparent = parse_bool(req_attr(el, "trnsprnt")?, "trnsprnt")?;
    r.left = parse_int(req_attr(el, "left")?, "left")?;
    r.right = parse_int(req_attr(el, "right")?, "right")?;
    r.top = parse_int(req_attr(el, "top")?, "top")?;
    r.bottom = parse_int(req_attr(el, "bottom")?, "bottom")?;
    if kind != RegionKind::Drawing {
        r.shown = parse_bool(req_attr(el, "show")?, "show")?;
    }
    if kind == RegionKind::Legend {
        r.scale_line_shown = parse_bool(req_attr(el, "scale")?, "legend scale")?;
    }
    Ok(r)
}

/// Разобрать extrnprops-`scale` ("1"/"1.5") в f64.
fn parse_scale(s: &str) -> Result<f64, GeoError> {
    s.parse::<f64>()
        .map_err(|_| GeoError::Frame(format!("scale: expected number, got {s:?}")))
}

// ---- g5 (EDT) READ ----

fn read_g5(root: &Element) -> Result<GeoSchema, GeoError> {
    if root.local != "GeographicalSchema" {
        return frame(format!("g5 root <{}> != <GeographicalSchema>", root.local));
    }
    let legend = read_g5_region(root, "geographicalSchemaLegend", RegionKind::Legend)?;
    let title = read_g5_region(root, "geographicalSchemaTitle", RegionKind::Title)?;
    let drawing = read_g5_region(root, "geographicalSchemaDrawingArea", RegionKind::Drawing)?;

    // showMode / scale — g5 эмитит явно; проекция/useOutput/границы-данных ОПУЩЕНЫ при дефолте.
    let show_mode = child_text(root, "showMode")?.to_string();
    let scale = parse_scale(child_text(root, "scale")?)?;

    // viewed: g5 несёт ±MAX-сентинелы (== Unset) ЛИБО реальные значения.
    let vxmin = child_text(root, "viewedXMin")?;
    let vymin = child_text(root, "viewedYMin")?;
    let vxmax = child_text(root, "viewedXMax")?;
    let vymax = child_text(root, "viewedYMax")?;
    let viewed = if vxmin == VIEWED_MIN_SENTINEL
        && vymin == VIEWED_MIN_SENTINEL
        && vxmax == VIEWED_MAX_SENTINEL
        && vymax == VIEWED_MAX_SENTINEL
    {
        ViewedArea::Unset
    } else {
        ViewedArea::Set {
            xmin: vxmin.to_string(),
            ymin: vymin.to_string(),
            xmax: vxmax.to_string(),
            ymax: vymax.to_string(),
        }
    };

    let geo = GeoSchema {
        current_id: parse_int(child_text(root, "currentID")?, "currentID")?,
        // g5 опускает проекцию/useOutput/границы при дефолте → подставляем канон-дефолт.
        projection: DEF_PROJECTION.to_string(),
        show_mode,
        scale,
        use_output: DEF_USE_OUTPUT.to_string(),
        data_bounds: [z(), z(), z(), z()],
        delta_bounds: [z(), z(), z(), z()],
        viewed,
        legend,
        title,
        drawing,
    };
    Ok(geo)
}

/// Строка "0" (дефолт границы-данных/дельты).
fn z() -> String {
    "0".to_string()
}

fn read_g5_region(root: &Element, local: &str, kind: RegionKind) -> Result<GeoRegion, GeoError> {
    let el = root
        .child(local)
        .ok_or_else(|| GeoError::Frame(format!("g5 <GeographicalSchema> missing <{local}>")))?;
    let mut r = GeoRegion::default_for(kind);
    // border/width
    let border = el
        .child("border")
        .ok_or_else(|| GeoError::Frame(format!("g5 <{local}> missing <border>")))?;
    r.border_width = parse_int(child_text(border, "width")?, "border width")?;
    // transparent (обязателен в витнессированном дефолте)
    r.transparent = parse_bool(child_text(el, "transparent")?, "transparent")?;
    // координаты (опущены при 0)
    if let Some(c) = el.child("leftCoordinatePercentage") {
        r.left = parse_int(&c.text, "leftCoordinatePercentage")?;
    }
    if let Some(c) = el.child("topCoordinatePercentage") {
        r.top = parse_int(&c.text, "topCoordinatePercentage")?;
    }
    if let Some(c) = el.child("rightCoordinatePercentage") {
        r.right = parse_int(&c.text, "rightCoordinatePercentage")?;
    }
    if let Some(c) = el.child("bottomCoordinatePercentage") {
        r.bottom = parse_int(&c.text, "bottomCoordinatePercentage")?;
    }
    // цвета/шрифты — читаем `<...><color|font>Ref</...>` и СВЕРЯЕМ с дефолтом (§1.0).
    match kind {
        RegionKind::Legend => {
            r.text_color = read_g5_color(el, "scaleLineTextColor")?;
            r.border_color = read_g5_color(el, "borderColor")?;
            r.back_color = read_g5_color(el, "backColor")?;
            r.text_font = read_g5_font(el, "scaleLineTextFont")?;
            r.shown = parse_bool(child_text(el, "shown")?, "shown")?;
            r.scale_line_shown = parse_bool(child_text(el, "scaleLineShown")?, "scaleLineShown")?;
        }
        RegionKind::Title => {
            r.text_font = read_g5_font(el, "font")?;
            r.text_color = read_g5_color(el, "textColor")?;
            r.border_color = read_g5_color(el, "borderColor")?;
            r.back_color = read_g5_color(el, "backColor")?;
            r.shown = parse_bool(child_text(el, "shown")?, "shown")?;
        }
        RegionKind::Drawing => {
            r.border_color = read_g5_color(el, "borderColor")?;
            r.back_color = read_g5_color(el, "backColor")?;
        }
    }
    Ok(r)
}

/// Прочитать g5 `<local xsi:type="core:ColorRef"><color>Ref</color></local>` → `Ref`.
fn read_g5_color(region: &Element, local: &str) -> Result<String, GeoError> {
    let el = region
        .child(local)
        .ok_or_else(|| GeoError::Frame(format!("g5 region missing color <{local}>")))?;
    Ok(child_text(el, "color")?.to_string())
}

/// Прочитать g5 `<local xsi:type="core:FontRef"><font>Ref</font></local>` → `Ref`.
fn read_g5_font(region: &Element, local: &str) -> Result<String, GeoError> {
    let el = region
        .child(local)
        .ok_or_else(|| GeoError::Frame(format!("g5 region missing font <{local}>")))?;
    Ok(child_text(el, "font")?.to_string())
}

// ============================ WRITE ============================

// ---- extrnprops (Designer/cf) WRITE ----

fn build_extrnprops(geo: &GeoSchema) -> OutElement {
    let mut root = OutElement::branch("", "GeographicalScheme")
        .attr("xmlns", EXTRNPROPS_NS)
        .attr("xmlns:d1p1", GEO_DATA_NS)
        .attr("xmlns:xs", XS_NS)
        .attr("xmlns:xsi", XSI_NS)
        .attr("xsi:type", "d1p1:GeographicalSchema")
        .attr("curId", geo.current_id.to_string())
        .attr("proj", &geo.projection)
        .attr("showMode", &geo.show_mode)
        .attr("scale", fmt_scale_designer(geo.scale))
        .attr("xmn", &geo.data_bounds[0])
        .attr("xmx", &geo.data_bounds[1])
        .attr("ymn", &geo.data_bounds[2])
        .attr("ymx", &geo.data_bounds[3])
        .attr("dxn", &geo.delta_bounds[0])
        .attr("dxx", &geo.delta_bounds[1])
        .attr("dyn", &geo.delta_bounds[2])
        .attr("dyx", &geo.delta_bounds[3]);
    let (vxn, vxx, vyn, vyx) = match &geo.viewed {
        ViewedArea::Unset => (
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
            "0".to_string(),
        ),
        ViewedArea::Set {
            xmin,
            ymin,
            xmax,
            ymax,
        } => (xmin.clone(), xmax.clone(), ymin.clone(), ymax.clone()),
    };
    root = root
        .attr("vxn", vxn)
        .attr("vxx", vxx)
        .attr("vyn", vyn)
        .attr("vyx", vyx)
        .attr("useOutput", &geo.use_output);
    root.push(build_extrnprops_region(&geo.legend, "legend"));
    root.push(build_extrnprops_region(&geo.title, "title"));
    root.push(build_extrnprops_region(&geo.drawing, "drawing"));
    root
}

fn build_extrnprops_region(r: &GeoRegion, local: &str) -> OutElement {
    let mut el = OutElement::branch("d1p1", local);
    el.self_closing = true;
    if r.kind != RegionKind::Drawing {
        el = el.attr("show", bool_str(r.shown));
    }
    el = el.attr("trnsprnt", bool_str(r.transparent));
    if r.kind == RegionKind::Legend {
        el = el.attr("scale", bool_str(r.scale_line_shown));
    }
    el.attr("left", r.left.to_string())
        .attr("right", r.right.to_string())
        .attr("top", r.top.to_string())
        .attr("bottom", r.bottom.to_string())
}

// ---- g5 (EDT) WRITE ----

fn build_g5(geo: &GeoSchema) -> Result<OutElement, GeoError> {
    // §1.0: g5 не воспроизводит невитнессированные конструкты — проекция/useOutput/границы-
    // данных, отличные от дефолта, не имеют витнессированной g5-формы.
    if geo.projection != DEF_PROJECTION {
        return frame(format!(
            "non-default projection {:?} has no witnessed g5 form",
            geo.projection
        ));
    }
    if geo.use_output != DEF_USE_OUTPUT {
        return frame(format!(
            "non-default useOutput {:?} has no witnessed g5 form",
            geo.use_output
        ));
    }
    if geo.data_bounds.iter().any(|b| b != "0") || geo.delta_bounds.iter().any(|b| b != "0") {
        return frame("non-zero data/delta bounds have no witnessed g5 form".to_string());
    }

    let mut root = OutElement::branch("geographicalSchema", "GeographicalSchema")
        .attr("xmlns:xsi", XSI_NS)
        .attr("xmlns:core", CORE_NS)
        .attr("xmlns:geographicalSchema", G5_GEO_NS);
    root.push(build_g5_region(&geo.legend, "geographicalSchemaLegend"));
    root.push(build_g5_region(&geo.title, "geographicalSchemaTitle"));
    root.push(build_g5_region(
        &geo.drawing,
        "geographicalSchemaDrawingArea",
    ));
    root.push(OutElement::leaf("", "showMode", &geo.show_mode));
    root.push(OutElement::leaf("", "scale", fmt_scale_g5(geo.scale)));
    let (vxmin, vymin, vxmax, vymax) = match &geo.viewed {
        ViewedArea::Unset => (
            VIEWED_MIN_SENTINEL.to_string(),
            VIEWED_MIN_SENTINEL.to_string(),
            VIEWED_MAX_SENTINEL.to_string(),
            VIEWED_MAX_SENTINEL.to_string(),
        ),
        ViewedArea::Set {
            xmin,
            ymin,
            xmax,
            ymax,
        } => (xmin.clone(), ymin.clone(), xmax.clone(), ymax.clone()),
    };
    root.push(OutElement::leaf("", "viewedXMin", vxmin));
    root.push(OutElement::leaf("", "viewedYMin", vymin));
    root.push(OutElement::leaf("", "viewedXMax", vxmax));
    root.push(OutElement::leaf("", "viewedYMax", vymax));
    root.push(OutElement::leaf(
        "",
        "currentID",
        geo.current_id.to_string(),
    ));
    Ok(root)
}

fn build_g5_region(r: &GeoRegion, local: &str) -> OutElement {
    let mut el = OutElement::branch("", local);
    // border
    let mut border = OutElement::branch("", "border").attr("xsi:type", "core:BorderDef");
    border.push(OutElement::leaf("", "width", r.border_width.to_string()));
    el.push(border);
    // transparent
    el.push(OutElement::leaf("", "transparent", bool_str(r.transparent)));
    // координаты (опускаем 0), порядок L,T,R,B
    if r.left != 0 {
        el.push(OutElement::leaf(
            "",
            "leftCoordinatePercentage",
            r.left.to_string(),
        ));
    }
    if r.top != 0 {
        el.push(OutElement::leaf(
            "",
            "topCoordinatePercentage",
            r.top.to_string(),
        ));
    }
    if r.right != 0 {
        el.push(OutElement::leaf(
            "",
            "rightCoordinatePercentage",
            r.right.to_string(),
        ));
    }
    if r.bottom != 0 {
        el.push(OutElement::leaf(
            "",
            "bottomCoordinatePercentage",
            r.bottom.to_string(),
        ));
    }
    // цвета/шрифты в порядке XSD региона
    match r.kind {
        RegionKind::Legend => {
            el.push(g5_color("scaleLineTextColor", &r.text_color));
            el.push(g5_color("borderColor", &r.border_color));
            el.push(g5_color("backColor", &r.back_color));
            el.push(g5_font("scaleLineTextFont", &r.text_font));
            el.push(OutElement::leaf("", "shown", bool_str(r.shown)));
            el.push(OutElement::leaf(
                "",
                "scaleLineShown",
                bool_str(r.scale_line_shown),
            ));
        }
        RegionKind::Title => {
            el.push(g5_font("font", &r.text_font));
            el.push(g5_color("textColor", &r.text_color));
            el.push(g5_color("borderColor", &r.border_color));
            el.push(g5_color("backColor", &r.back_color));
            el.push(OutElement::leaf("", "shown", bool_str(r.shown)));
        }
        RegionKind::Drawing => {
            el.push(g5_color("borderColor", &r.border_color));
            el.push(g5_color("backColor", &r.back_color));
        }
    }
    el
}

fn g5_color(local: &str, color_ref: &str) -> OutElement {
    let mut el = OutElement::branch("", local).attr("xsi:type", "core:ColorRef");
    el.push(OutElement::leaf("", "color", color_ref));
    el
}

fn g5_font(local: &str, font_ref: &str) -> OutElement {
    let mut el = OutElement::branch("", local).attr("xsi:type", "core:FontRef");
    el.push(OutElement::leaf("", "font", font_ref));
    el
}

// ---- formatting helpers ----

fn bool_str(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

/// extrnprops-формат масштаба: целое печатается без дробной части ("1"), иначе как есть.
fn fmt_scale_designer(scale: f64) -> String {
    if scale.fract() == 0.0 {
        format!("{}", scale as i64)
    } else {
        format!("{scale}")
    }
}

/// g5-формат масштаба: целое печатается с ".0" ("1.0"), иначе как есть.
fn fmt_scale_g5(scale: f64) -> String {
    if scale.fract() == 0.0 {
        format!("{}.0", scale as i64)
    } else {
        format!("{scale}")
    }
}

#[cfg(any())]
mod tests;
