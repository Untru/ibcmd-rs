//! Проекция форм-АТРИБУТОВ: каноническое поле ↔ (тег, кодек, ПЕР-ФОРМАТНЫЙ дефолт) для
//! EDT и Designer. Канонику (id/value_kind/порядок) держит `core/spec/forms/form_root`;
//! здесь — лишь размещение/кодировка/дефолт каждого формата (§1.6).
//!
//! Дефолты EDT и Designer ПРОТИВОПОЛОЖНЫ для auto/save/enable-семейства (сверено по
//! корпусу): READ-движок берёт пер-форматный дефолт при отсутствии ячейки, WRITE опускает
//! то, что == дефолту ЭТОГО формата.

use super::FormDialect;
use crate::emit::OutElement;
use morph1c_core::ir::FieldId;
use morph1c_core::ir::value::{PropertyValue, Token};
use morph1c_core::spec::forms::form_root as fr;

/// Вид значения форм-атрибута в проекции.
#[derive(Clone, Copy)]
enum AttrKind {
    Bool,
    Enum,
    Int,
    /// Целое с EDT-десятичной кодировкой `N.0` (`scale`: EDT `101.0` ⟺ канон `Int(101)` ⟺
    /// Designer `101`). Только в EDT-таблице; Designer-строка того же поля — [`AttrKind::Int`].
    IntEdtDecimal,
    /// Пер-форматный enum-мэппинг `(канон=EDT, литерал этого формата)` — литералы
    /// РАСХОДЯТСЯ (`verticalScroll`: EDT `UseIfNecessary` ⟺ Designer `useIfNecessary`).
    EnumMap(&'static [(&'static str, &'static str)]),
    /// Свободный текст (канон `Str`; ERP-волна: `groupList`/`settingsStorage`).
    Str,
}

/// Описание одного форм-атрибута в КОНКРЕТНОМ формате.
struct AttrProj {
    /// Канонический id поля.
    id: FieldId,
    /// Имя тега в этом формате (EDT lower / Designer Upper).
    tag: &'static str,
    /// Вид значения (Bool/Enum/Int/EnumMap).
    kind: AttrKind,
    /// Пер-форматный дефолт ИЛИ `None` (= канонический `fs.default`).
    default: Option<DefaultVal>,
    /// Designer-only: xsi:type на узле (для `WindowViewMode`).
    xsi_type: Option<&'static str>,
}

/// Пер-форматное дефолт-значение (Bool/Enum).
#[derive(Clone, Copy)]
enum DefaultVal {
    Bool(bool),
    Enum(&'static str),
}

impl DefaultVal {
    fn to_value(self) -> PropertyValue {
        match self {
            DefaultVal::Bool(b) => PropertyValue::Bool(b),
            DefaultVal::Enum(s) => PropertyValue::Enum(Token::new(s)),
        }
    }
}

/// Мэппинг литералов `verticalScroll` (канон=EDT ⟺ Designer lower-camel; SSL:
/// UseIfNecessary×83, UseWithoutStretch×2 — зеркальны в обоих диалектах).
const VERTICAL_SCROLL_MAP: &[(&str, &str)] = &[
    ("UseIfNecessary", "useIfNecessary"),
    ("Use", "use"),
    ("UseWithoutStretch", "useWithoutStretch"),
];

/// Таблица EDT форм-атрибутов. Пер-форматный дефолт `None` ⇒ канонический (= EDT-дефолт):
/// EDT почти всегда совпадает с каноническим, кроме отсутствия (нет переопределений).
fn edt_table() -> &'static [AttrProj] {
    &[
        AttrProj {
            id: fr::F_COMMAND_BAR_LOCATION,
            tag: "commandBarLocation",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SHOW_COMMAND_BAR,
            tag: "showCommandBar",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_WINDOW_OPENING_MODE,
            tag: "windowOpeningMode",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_WINDOW_VIEW_MODE,
            tag: "windowViewMode",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SAVE_WINDOW_SETTINGS,
            tag: "saveWindowSettings",
            kind: AttrKind::Bool,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_AUTO_URL,
            tag: "autoUrl",
            kind: AttrKind::Bool,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_AUTO_TITLE,
            tag: "autoTitle",
            kind: AttrKind::Bool,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_GROUP,
            tag: "group",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_AUTO_FILL_CHECK,
            tag: "autoFillCheck",
            kind: AttrKind::Bool,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_ALLOW_FORM_CUSTOMIZE,
            tag: "allowFormCustomize",
            kind: AttrKind::Bool,
            default: None,
            xsi_type: None,
        },
        // enabled — EDT-ДЕФОЛТ = `false` (омиссия значит `false`), НЕ «эмитит всегда». Прежнее
        // `None` (⇒ канон-`required` без дефолта ⇒ EDT-омиссия НЕ попадала в bag) расходило IR
        // диалектов ровно на носителях омиссии. Кросс-витнесс-ценз по ПОЛНОМУ корпусу
        // (join по форме, `ag_at_census`): таблица ДИЗЪЮНКТНА, клетки «оба опускают» НЕТ —
        //   SSL  876/876:  des `<ABSENT>`⟺edt `true` 875 | des `false`⟺edt `<ABSENT>` 1;
        //   ERP  11 400/11 400: 11 395 | 5;
        //   coverage 28/28: 28 | 0.
        // Ни один диалект НИ РАЗУ не написал противоположный литерал (`edt=false` 0 вхождений,
        // `des=true` 0) ⇒ омиссия каждой стороны имеет РОВНО ОДНО значение, fill восстановим.
        AttrProj {
            id: fr::F_ENABLED,
            tag: "enabled",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(false)),
            xsi_type: None,
        },
        // showTitle — ТРИ-состояние (`auto`/`true`/`false`). EDT-ДЕФОЛТ = `false`: EDT эмитит
        // `auto` (×864) и `true` (×5) и ОПУСКАЕТ `false` (×7); Designer эмитит все три ВСЕГДА.
        // Контингентная таблица по всем 876 формам (`probe_defaults`) — ЧИСТАЯ, без
        // внедиагональных клеток: EDT-`<ABSENT>` ⟺ Designer-`false` ровно на одних и тех же 7
        // формах. Раньше дефолт был `None` (EDT-омиссия ⇒ поля нет в bag) ⇒ IR двух диалектов
        // расходился, и designer→cf ОТКАЗЫВАЛ на `false` («нет витнессированной ячейки»).
        AttrProj {
            id: fr::F_SHOW_TITLE,
            tag: "showTitle",
            kind: AttrKind::Enum,
            default: Some(DefaultVal::Enum("false")),
            xsi_type: None,
        },
        // showCloseButton — та же природа, что `enabled`: EDT-ДЕФОЛТ `false`. Ценз (ДИЗЪЮНКТНО,
        // клетки «оба опускают» НЕТ): SSL des `<ABSENT>`⟺edt `true` 875 | des `false`⟺edt
        // `<ABSENT>` 1; ERP 11 364 | 36; coverage 28 | 0.
        AttrProj {
            id: fr::F_SHOW_CLOSE_BUTTON,
            tag: "showCloseButton",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(false)),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_WIDTH,
            tag: "width",
            kind: AttrKind::Int,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_AUTO_SAVE_DATA_IN_SETTINGS,
            tag: "autoSaveDataInSettings",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_ENTER_KEY_BEHAVIOR,
            tag: "enterKeyBehavior",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SAVE_DATA_IN_SETTINGS,
            tag: "saveDataInSettings",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_HORIZONTAL_ALIGN,
            tag: "horizontalAlign",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_VERTICAL_SCROLL,
            tag: "verticalScroll",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_CONVERSATIONS_REPRESENTATION,
            tag: "conversationsRepresentation",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_HEIGHT,
            tag: "height",
            kind: AttrKind::Int,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_VERTICAL_ALIGN,
            tag: "verticalAlign",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_HORIZONTAL_SPACING,
            tag: "horizontalSpacing",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_VERTICAL_SPACING,
            tag: "verticalSpacing",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_CHILD_ITEMS_WIDTH,
            tag: "childItemsWidth",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SCALE,
            tag: "scale",
            kind: AttrKind::IntEdtDecimal,
            default: None,
            xsi_type: None,
        },
        // --- ERP-волна (core: form_root F_SCALING_MODE..F_SETTINGS_STORAGE) ---
        // scalingMode (ERP 29 x Normal/Compact), collapseItemsByImportanceVariant (32 x
        // DontUse/Use), childrenAlign (3 x None), groupList (4, имя элемента),
        // settingsStorage (6, metadata-ref). Все опциональны (absent => не в bag).
        AttrProj {
            id: fr::F_SCALING_MODE,
            tag: "scalingMode",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_COLLAPSE_ITEMS_BY_IMPORTANCE_VARIANT,
            tag: "collapseItemsByImportanceVariant",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_CHILDREN_ALIGN,
            tag: "childrenAlign",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_GROUP_LIST,
            tag: "groupList",
            kind: AttrKind::Str,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SETTINGS_STORAGE,
            tag: "settingsStorage",
            kind: AttrKind::Str,
            default: None,
            xsi_type: None,
        },
    ]
}

/// Таблица Designer форм-атрибутов с ПЕР-ФОРМАТНЫМИ дефолтами (противоположны EDT).
fn designer_table() -> &'static [AttrProj] {
    &[
        AttrProj {
            id: fr::F_COMMAND_BAR_LOCATION,
            tag: "CommandBarLocation",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SHOW_COMMAND_BAR,
            tag: "ShowCommandBar",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_WINDOW_OPENING_MODE,
            tag: "WindowOpeningMode",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_WINDOW_VIEW_MODE,
            tag: "WindowViewMode",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: Some("lf:FormWindowViewMode"),
        },
        AttrProj {
            id: fr::F_SAVE_WINDOW_SETTINGS,
            tag: "SaveWindowSettings",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(true)),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_AUTO_URL,
            tag: "AutoURL",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(true)),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_AUTO_TITLE,
            tag: "AutoTitle",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(true)),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_GROUP,
            tag: "Group",
            kind: AttrKind::Enum,
            default: Some(DefaultVal::Enum("Auto")),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_AUTO_FILL_CHECK,
            tag: "AutoFillCheck",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(true)),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_ALLOW_FORM_CUSTOMIZE,
            tag: "Customizable",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(true)),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_ENABLED,
            tag: "Enabled",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(true)),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SHOW_TITLE,
            tag: "ShowTitle",
            kind: AttrKind::Enum,
            default: Some(DefaultVal::Enum("auto")),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SHOW_CLOSE_BUTTON,
            tag: "ShowCloseButton",
            kind: AttrKind::Bool,
            default: Some(DefaultVal::Bool(true)),
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_WIDTH,
            tag: "Width",
            kind: AttrKind::Int,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_AUTO_SAVE_DATA_IN_SETTINGS,
            tag: "AutoSaveDataInSettings",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_ENTER_KEY_BEHAVIOR,
            tag: "EnterKeyBehavior",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SAVE_DATA_IN_SETTINGS,
            tag: "SaveDataInSettings",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_HORIZONTAL_ALIGN,
            tag: "HorizontalAlign",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        // verticalScroll: литералы расходятся регистром (EnumMap).
        AttrProj {
            id: fr::F_VERTICAL_SCROLL,
            tag: "VerticalScroll",
            kind: AttrKind::EnumMap(VERTICAL_SCROLL_MAP),
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_CONVERSATIONS_REPRESENTATION,
            tag: "ConversationsRepresentation",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_HEIGHT,
            tag: "Height",
            kind: AttrKind::Int,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_VERTICAL_ALIGN,
            tag: "VerticalAlign",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_HORIZONTAL_SPACING,
            tag: "HorizontalSpacing",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_VERTICAL_SPACING,
            tag: "VerticalSpacing",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_CHILD_ITEMS_WIDTH,
            tag: "ChildItemsWidth",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SCALE,
            tag: "Scale",
            kind: AttrKind::Int,
            default: None,
            xsi_type: None,
        },
        // --- ERP-волна (core: form_root F_SCALING_MODE..F_SETTINGS_STORAGE) ---
        // scalingMode (ERP 29 x Normal/Compact), collapseItemsByImportanceVariant (32 x
        // DontUse/Use), childrenAlign (3 x None), groupList (4, имя элемента),
        // settingsStorage (6, metadata-ref). Все опциональны (absent => не в bag).
        AttrProj {
            id: fr::F_SCALING_MODE,
            tag: "ScalingMode",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_COLLAPSE_ITEMS_BY_IMPORTANCE_VARIANT,
            tag: "CollapseItemsByImportanceVariant",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_CHILDREN_ALIGN,
            tag: "ChildrenAlign",
            kind: AttrKind::Enum,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_GROUP_LIST,
            tag: "GroupList",
            kind: AttrKind::Str,
            default: None,
            xsi_type: None,
        },
        AttrProj {
            id: fr::F_SETTINGS_STORAGE,
            tag: "SettingsStorage",
            kind: AttrKind::Str,
            default: None,
            xsi_type: None,
        },
    ]
}

fn table(dialect: FormDialect) -> &'static [AttrProj] {
    match dialect {
        FormDialect::Edt => edt_table(),
        FormDialect::Designer => designer_table(),
    }
}

pub(super) fn lexical_property_tags(dialect: FormDialect) -> Vec<&'static str> {
    table(dialect).iter().map(|field| field.tag).collect()
}

fn find(dialect: FormDialect, id: FieldId) -> Option<&'static AttrProj> {
    table(dialect).iter().find(|a| a.id == id)
}

/// Эмитировать один EDT форм-атрибут (или `None`, если == EDT-дефолту/отсутствует).
pub(crate) fn edt_attr_node(id: FieldId, value: Option<&PropertyValue>) -> Option<OutElement> {
    attr_node(FormDialect::Edt, id, value)
}

/// Эмитировать один Designer форм-атрибут (или `None`).
pub(crate) fn designer_attr_node(id: FieldId, value: Option<&PropertyValue>) -> Option<OutElement> {
    attr_node(FormDialect::Designer, id, value)
}

/// Общий: эмитировать узел атрибута, если его эффективное значение != дефолту ЭТОГО формата.
fn profile_default(
    proj: &AttrProj,
    dialect: FormDialect,
    version: Option<morph1c_core::version::FormatVersion>,
) -> Option<DefaultVal> {
    if dialect == FormDialect::Designer
        && version == Some(morph1c_core::version::FormatVersion::new(2, 20))
    {
        match proj.id {
            fr::F_GROUP => return Some(DefaultVal::Enum("Vertical")),
            fr::F_SHOW_TITLE => return Some(DefaultVal::Enum("true")),
            _ => {}
        }
    }
    proj.default
}

fn attr_node(
    dialect: FormDialect,
    id: FieldId,
    value: Option<&PropertyValue>,
) -> Option<OutElement> {
    let proj = find(dialect, id)?;
    let spec = fr::form_root();
    let fs = spec.field(id)?;
    // Эффективное значение: из bag, иначе канонический spec.default.
    let eff: &PropertyValue = match value {
        Some(v) => v,
        None => match &fs.default {
            Some(d) => d,
            None => return None, // required без значения — невозможно для каноничного bag.
        },
    };
    // Дефолт ЭТОГО формата: пер-форматный override, иначе канонический spec.default.
    let fmt_default: Option<PropertyValue> = profile_default(
        proj,
        dialect,
        morph1c_core::version::current_roundtrip_target(),
    )
    .map(|d| d.to_value())
    .or_else(|| fs.default.clone());
    if let Some(fd) = &fmt_default {
        if eff == fd {
            return None; // == дефолту формата ⇒ опускаем (sparse).
        }
    }
    Some(render_attr(proj, eff))
}

/// Срендерить узел атрибута по его значению (Bool→"true"/"false", Enum→литерал,
/// Int→число, EnumMap→литерал ЭТОГО формата).
fn render_attr(proj: &AttrProj, value: &PropertyValue) -> OutElement {
    let text = match (proj.kind, value) {
        (AttrKind::EnumMap(map), PropertyValue::Enum(t)) => map
            .iter()
            .find(|(canon, _)| *canon == t.as_str())
            .map(|(_, d)| d.to_string())
            // Незамапленный канон-литерал сюда не доходит (read его отверг); формат-литерал
            // без пары воспроизводим как есть — R остаётся byte-exact.
            .unwrap_or_else(|| t.as_str().to_string()),
        (_, PropertyValue::Bool(b)) => (if *b { "true" } else { "false" }).to_string(),
        (_, PropertyValue::Enum(t)) => t.as_str().to_string(),
        (AttrKind::IntEdtDecimal, PropertyValue::Int(n)) => format!("{n}.0"),
        (_, PropertyValue::Int(n)) => n.to_string(),
        // Свободный текст — как есть (ERP-волна: groupList/settingsStorage).
        (_, PropertyValue::Str(s)) => s.clone(),
        // value_kind гарантирован спеком; иные виды сюда не доходят.
        (_, other) => format!("{other:?}"),
    };
    let mut el = OutElement::leaf("", proj.tag, text);
    if let Some(x) = proj.xsi_type {
        el = el.attr("xsi:type", x);
    }
    el
}

// --- READ-сторона: прямое чтение форм-атрибутов (без XmlLocus — теги динамичны) ---

/// Прочитать ВСЕ форм-атрибуты данного формата от корня `root` в канонический разрежённый
/// bag. Claim'ит узлы атрибутов. Каждый атрибут: present ⇒ значение; absent ⇒ пер-форматный
/// дефолт (или канонический). Сжатие против КАНОНИЧЕСКОГО `fs.default` ⇒ оба формата дают
/// равный bag (X). Узлы, НЕ принадлежащие ни одному атрибуту, остаются неклеймнутыми (их
/// разбирают другие регионы; §1.0-тотальность сверяет вызывающий).
pub(crate) fn read_form_attrs(
    dialect: FormDialect,
    root: &crate::descriptor::Element,
) -> Result<Vec<(FieldId, PropertyValue)>, String> {
    let spec = fr::form_root();
    let mut bag = Vec::new();
    for proj in table(dialect) {
        let fs = spec.field(proj.id).expect("form_root spec has field");
        // Локализуем узел атрибута среди прямых детей корня (по тегу, без префикса).
        let node = root
            .children
            .iter()
            .find(|c| c.local == proj.tag && c.prefix.is_empty());
        let value: PropertyValue = match node {
            Some(el) => {
                // Claim узел+текст; xsi:type (если есть у Designer) — claim.
                el.claim_with_text();
                if let Some(x) = proj.xsi_type {
                    match el.attr("xsi:type") {
                        Some(a) if a.value == x => a.claimed.set(true),
                        Some(a) => {
                            return Err(format!(
                                "form attr <{}> xsi:type={:?}, want {x:?}",
                                proj.tag, a.value
                            ));
                        }
                        None => {
                            return Err(format!("form attr <{}> missing xsi:type {x:?}", proj.tag));
                        }
                    }
                }
                if !el.attrs.iter().all(|a| a.claimed.get()) {
                    return Err(format!("form attr <{}> has unexpected attribute", proj.tag));
                }
                if !el.children.is_empty() {
                    return Err(format!("form attr <{}> must be a text leaf", proj.tag));
                }
                decode_attr(proj, &el.text)?
            }
            None => match profile_default(
                proj,
                dialect,
                morph1c_core::version::current_source_version(),
            )
            .map(|d| d.to_value())
            .or_else(|| fs.default.clone())
            {
                Some(v) => v,
                // Нет ни пер-форматного, ни канонического дефолта ⇒ атрибут ОПЦИОНАЛЕН (absent
                // валиден — напр. `showTitle` опускают DataProcessor-формы). Пропускаем: не пушим
                // в bag; write опустит (attr_node вернёт None). Same-dialect round-trip byte-exact.
                None => continue,
            },
        };
        // §1.6: сжатие против КАНОНИЧЕСКОГО fs.default ⇒ равный bag у обоих форматов.
        let is_canon_default = fs.default.as_ref() == Some(&value);
        if !is_canon_default {
            bag.push((proj.id, value));
        }
    }
    Ok(bag)
}

/// Декодировать текст атрибута по его виду (Bool/Enum/Int/EnumMap).
fn decode_attr(proj: &AttrProj, text: &str) -> Result<PropertyValue, String> {
    match proj.kind {
        AttrKind::Bool => match text {
            "true" => Ok(PropertyValue::Bool(true)),
            "false" => Ok(PropertyValue::Bool(false)),
            other => Err(format!(
                "form attr <{}>: not a bool literal {other:?}",
                proj.tag
            )),
        },
        AttrKind::Enum => Ok(PropertyValue::Enum(Token::new(text.to_string()))),
        AttrKind::Int => text
            .parse::<i64>()
            .map(PropertyValue::Int)
            .map_err(|e| format!("form attr <{}>: bad int {text:?}: {e}", proj.tag)),
        AttrKind::IntEdtDecimal => text
            .strip_suffix(".0")
            .ok_or_else(|| format!("form attr <{}>={text:?}: want N.0 decimal (§1.0)", proj.tag))?
            .parse::<i64>()
            .map(PropertyValue::Int)
            .map_err(|e| format!("form attr <{}>: bad decimal {text:?}: {e}", proj.tag)),
        AttrKind::EnumMap(map) => match map.iter().find(|(_, d)| *d == text) {
            Some((canon, _)) => Ok(PropertyValue::Enum(Token::new(*canon))),
            None => Err(format!(
                "form attr <{}>={text:?}: unmapped enum literal (§1.0)",
                proj.tag
            )),
        },
        // Свободный текст — как есть (ERP-волна: groupList/settingsStorage).
        AttrKind::Str => Ok(PropertyValue::Str(text.to_string())),
    }
}
