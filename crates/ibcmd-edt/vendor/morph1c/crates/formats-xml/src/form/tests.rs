//! Тесты форм-коннектора. Реальная пилот-форма (R+X byte-exact) — в `testkit` (нужен
//! корпус через `MORPH1C_FIXTURES`). Здесь — синтетические единичные проверки структуры.

use super::*;

// ===================== spec↔table drift-guard (LANE-F-3, nit c) =====================
// Каждое поле проекционной таблицы (`form/tables.rs`) ОБЯЗАНО иметь канонический спек
// (`core/spec/forms/controls`), и наоборот — каждый спек-поле либо в таблице, либо в
// известном наборе КАРКАС-glue (title/formatted декораций, эмитятся вне таблицы). Дрейф
// (переименование id, забытое поле в спеке/таблице) → провал этого теста.

/// Id-множество полей спека контрола (properties ∪ ext_info).
fn spec_field_ids(kind: &str) -> std::collections::BTreeSet<morph1c_core::ir::FieldId> {
    let spec = morph1c_core::spec::forms::controls::control_spec_for(kind)
        .unwrap_or_else(|| panic!("no ControlSpec registered for {kind}"));
    spec.properties
        .fields()
        .iter()
        .chain(spec.ext_info.fields().iter())
        .map(|f| f.id)
        .collect()
}

/// Id-множество проекционной таблицы (тело ∪ extInfo).
fn table_field_ids(
    body: &[super::fields::FieldProj],
    ext: &[super::fields::FieldProj],
) -> std::collections::BTreeSet<morph1c_core::ir::FieldId> {
    body.iter().chain(ext.iter()).map(|e| e.id).collect()
}

/// КАРКАС-glue поля спека, эмитимые ВНЕ таблицы (title/formatted декораций несёт коннектор
/// напрямую, не FieldProj).
fn glue_ids(kind: &str) -> std::collections::BTreeSet<morph1c_core::ir::FieldId> {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    match kind {
        "LabelDecoration" | "PictureDecoration" => {
            [ld::F_TITLE, ld::F_FORMATTED].into_iter().collect()
        }
        _ => std::collections::BTreeSet::new(),
    }
}

/// ЛОКАЛЬНЫЕ FieldId ERP-волны (диапазон 900+, см. блок в `tables.rs`): витнесснутые
/// ERP-корпусом свойства, чьи канонические спеки в core ещё не заведены (core — вне зоны
/// той волны). Дрейф-гард их знает ПОИМЁННО — новый безымянный id всё равно упадёт.
fn erp_local_ids() -> std::collections::BTreeSet<morph1c_core::ir::FieldId> {
    use super::tables as t;
    [
        t::F_TB_USE_ALTERNATION_ROW_COLOR,
        t::F_TB_TITLE_HEIGHT,
        t::F_TB_SHORTCUT,
        t::F_TB_REFRESH_REQUEST,
        t::F_TB_BEHAVIOR_ON_HORIZONTAL_COMPRESSION,
        t::F_ADDITION_MAX_WIDTH,
        t::F_ADDITION_HORIZONTAL_LOCATION,
        t::F_ADDITION_TOOL_TIP_REPRESENTATION,
        t::F_FF_TITLE_BACK_COLOR,
        t::F_FF_FOOTER_DATA_PATH,
        t::F_FF_FOOTER_TEXT_COLOR,
        t::F_FF_FOOTER_PICTURE,
        t::F_EXT_MARK_NEGATIVES,
        t::F_EXT_SHOW_CHECK_BOXES_IN_DROP_LIST,
        t::F_EXT_SPECIAL_TEXT_INPUT_MODE,
        t::F_EXT_VIEW_SCALING_MODE,
        t::F_EXT_ITEM_WIDTH,
        t::F_EXT_ZOOMABLE,
        t::F_EXT_PIC_ENABLE_DRAG,
        t::F_PIC_BORDER_COLOR,
        t::F_GRP_HIDDEN_STATE_TITLE_BACK_COLOR,
        t::F_GRP_POPUP_BORDER_COLOR,
        t::F_LD_BORDER_COLOR,
        t::F_DEC_SHORTCUT,
    ]
    .into_iter()
    .chain(
        // РЕИСПОЛЬЗОВАННЫЕ кросс-видовые id той же волны: поле уже канонично у ДРУГОГО вида
        // (та же каноника «minValue»/«maxWidth»/…), а спек ЭТОГО вида строки ещё не несёт.
        {
            use morph1c_core::spec::forms::controls::form_field as ff;
            use morph1c_core::spec::forms::controls::form_group as fg;
            use morph1c_core::spec::forms::controls::radio_button as rb;
            [
                ff::F_EXT_MIN_VALUE,          // TrackBar minValue (Int-кодек)
                ff::F_EXT_MAX_WIDTH,          // TrackBar/HTML/Text/Spreadsheet maxWidth
                ff::F_EXT_MAX_HEIGHT,         // HTML/Text/Chart/Picture maxHeight
                ff::F_EXT_BORDER_COLOR,       // Label/Formatted borderColor
                ff::F_EXT_TEXT_COLOR,         // Radio textColor
                ff::F_HEADER_HORIZONTAL_ALIGN, // ColumnGroup headerHorizontalAlign
                ff::F_FIXING_IN_TABLE,        // ColumnGroup fixingInTable
                fg::F_EXT_BACK_COLOR,         // Popup backColor
                fg::F_EXT_FORMAT,             // Page format
                rb::F_EXT_ITEM_HEIGHT,        // CheckBox itemHeight
                rb::F_EXT_ITEM_TITLE_HEIGHT,  // CheckBox itemTitleHeight
                rb::F_EXT_EQUAL_ELEMENTS_WIDTH, // CheckBox equalElementsWidth (EqualItemsWidth)
            ]
        },
    )
    .collect()
}

fn assert_no_drift(
    kind: &str,
    body: &[super::fields::FieldProj],
    ext: &[super::fields::FieldProj],
) {
    let tbl = table_field_ids(body, ext);
    let spec = spec_field_ids(kind);
    // Каждое табличное поле имеет спек ЛИБО входит в известный локальный набор ERP-волны.
    let local = erp_local_ids();
    let orphan: Vec<_> = tbl
        .difference(&spec)
        .filter(|id| !local.contains(id))
        .collect();
    assert!(
        orphan.is_empty(),
        "{kind}: projection fields without spec: {orphan:?}"
    );
    // Каждое спек-поле либо в таблице, либо каркас-glue.
    let missing: std::collections::BTreeSet<_> = spec.difference(&tbl).copied().collect();
    assert_eq!(
        missing,
        glue_ids(kind),
        "{kind}: spec fields not projected (and not known glue) — spec/table drift"
    );
}

#[test]
fn projection_tables_match_control_specs() {
    use super::tables;
    for fk in tables::FIELD_KINDS {
        assert_no_drift(fk.kind, tables::FORM_FIELD_COMMON, fk.ext);
    }
    for gk in tables::GROUP_KINDS {
        assert_no_drift(gk.kind, tables::FORM_GROUP_BODY, gk.ext);
    }
    for dk in tables::DECORATION_KINDS {
        assert_no_drift(dk.kind, tables::DECORATION_BODY, dk.ext);
    }
    assert_no_drift("Button", tables::BUTTON_BODY, &[]);
    assert_no_drift("Table", tables::TABLE_BODY, &[]);
}

/// CLASS 3a: PictureField `borderColor` (ERP Новости.ФормаНовости) — СОБСТВЕННЫЙ id со СВОИМИ
/// позициями в обоих диалектах (EDT `border`→`borderColor`; Designer `ValuesPicture`→
/// `BorderColor`→`Border`, обратно EDT). Гард проверяет ОБЕ проекции.
#[test]
fn picture_field_border_color_is_wired_in_both_dialects() {
    use super::tables;
    use morph1c_core::spec::forms::controls::form_field as ff;

    // EDT: `borderColor` СРАЗУ за `border`, СОБСТВЕННЫЙ id (НЕ общий ff::F_EXT_BORDER_COLOR).
    let names: Vec<(&str, &str)> = tables::IMAGE_FIELD_EXT
        .iter()
        .map(|e| (e.edt, e.des))
        .collect();
    let border_pos = names
        .iter()
        .position(|(edt, _)| *edt == "border")
        .expect("`border` present in IMAGE_FIELD_EXT");
    assert_eq!(
        names.get(border_pos + 1),
        Some(&("borderColor", "BorderColor")),
        "EDT: borderColor must directly follow border in IMAGE_FIELD_EXT"
    );
    let bc = tables::IMAGE_FIELD_EXT
        .iter()
        .find(|e| e.edt == "borderColor")
        .unwrap();
    assert_eq!(
        bc.id,
        tables::F_PIC_BORDER_COLOR,
        "picture borderColor must use its OWN id, not the shared ff::F_EXT_BORDER_COLOR"
    );
    assert_ne!(
        tables::F_PIC_BORDER_COLOR,
        ff::F_EXT_BORDER_COLOR,
        "picture and input borderColor ids MUST differ (distinct pooled-order positions)"
    );

    // Designer: F_PIC_BORDER_COLOR между ValuesPicture и Border (witness ValuesPicture→
    // BorderColor→Border).
    let slot_ids: Vec<Option<morph1c_core::ir::FieldId>> = tables::DES_FIELD_ORDER
        .iter()
        .map(|s| match s {
            tables::DesSlot::F(id) => Some(*id),
            _ => None,
        })
        .collect();
    let pos = |want: morph1c_core::ir::FieldId| {
        slot_ids
            .iter()
            .position(|x| *x == Some(want))
            .unwrap_or_else(|| panic!("slot {want:?} absent from DES_FIELD_ORDER"))
    };
    assert!(
        pos(ff::F_EXT_VALUES_PICTURE) < pos(tables::F_PIC_BORDER_COLOR)
            && pos(tables::F_PIC_BORDER_COLOR) < pos(ff::F_EXT_BORDER),
        "Designer: borderColor slot must sit between ValuesPicture and Border"
    );
}

use morph1c_core::ir::value::{PropertyValue, Token};
use morph1c_core::ir::{FormBody, FormControlKind, FormItem};
use morph1c_core::spec::forms::form_root as fr;

/// Минимальный валидный набор форм-атрибутов (то, что EDT эмитит всегда: required-поля).
fn base_attrs() -> Vec<(morph1c_core::ir::FieldId, PropertyValue)> {
    vec![
        (
            fr::F_WINDOW_OPENING_MODE,
            PropertyValue::Enum(Token::new("LockOwner")),
        ),
        (fr::F_ENABLED, PropertyValue::Bool(true)),
        (fr::F_SHOW_TITLE, PropertyValue::Enum(Token::new("auto"))),
        (fr::F_SHOW_CLOSE_BUTTON, PropertyValue::Bool(true)),
    ]
}

/// Минимальная форма: round-trip EDT через read∘write даёт равный IR.
#[test]
fn edt_minimal_form_roundtrips_ir() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    let bytes = write_form(FormDialect::Edt, &body).expect("write");
    let back = read_form(FormDialect::Edt, &bytes).expect("read");
    assert_eq!(back.attributes, body.attributes, "form attrs round-trip");
    assert!(back.items.is_empty());
}

/// Один LabelDecoration: EDT write→read round-trips the control IR. Bag несёт
/// KEEP-поля (`enabled`/`userVisible`) явно — канон-bag хранит их ВСЕГДА (LANE-F-2:
/// каждый ридер заполняет дефолт СВОЕГО формата, писатель опускает свой).
#[test]
fn edt_label_decoration_roundtrips() {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    let mut item = FormItem::new(FormControlKind::new("LabelDecoration"), "L", 1);
    item.properties = vec![
        (ld::F_ENABLED, PropertyValue::Bool(true)),
        (ld::F_USER_VISIBLE, PropertyValue::Bool(true)),
        (ld::F_MAX_WIDTH, PropertyValue::Int(100)),
    ];
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.items = vec![item];
    let bytes = write_form(FormDialect::Edt, &body).expect("write");
    let back = read_form(FormDialect::Edt, &bytes).expect("read");
    assert_eq!(back.items.len(), 1);
    assert_eq!(back.items[0].name, "L");
    assert_eq!(back.items[0].id, 1);
    assert_eq!(back.items[0].properties, body.items[0].properties);
}

/// CLASS 3: PictureDecoration `shortcut` (тело) + `borderColor` (extInfo) читаются И пишутся в
/// ОБОИХ диалектах, round-trip'я IR (симптомы `<PictureDecoration>: unexpected child <:Shortcut>`
/// / `<:BorderColor>` сняты). Witness — Мастер_ПаспортныеДанные Силуэт / НастройкиОбменаФСС.
#[test]
fn picture_decoration_shortcut_and_border_color_roundtrip_both_dialects() {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    let build = || {
        let mut item = FormItem::new(FormControlKind::new("PictureDecoration"), "Силуэт", 82);
        item.properties = vec![
            (ld::F_ENABLED, PropertyValue::Bool(true)),
            (ld::F_USER_VISIBLE, PropertyValue::Bool(true)),
            (ld::F_WIDTH, PropertyValue::Int(10)),
            (super::tables::F_DEC_SHORTCUT, PropertyValue::Str("X".into())),
        ];
        item.ext_info = vec![
            (
                ld::F_EXT_PICTURE,
                PropertyValue::List(vec![
                    PropertyValue::Ref("CommonPicture.Силуэт".into()),
                    PropertyValue::Bool(false),
                ]),
            ),
            (
                super::tables::F_LD_BORDER_COLOR,
                PropertyValue::Ref("Web.LightSalmon".into()),
            ),
        ];
        let mut body = FormBody::new();
        body.attributes = base_attrs();
        body.items = vec![item];
        body
    };
    // Канонический ext-bag ПОСЛЕ чтения — УПЛОТНЁННЫЙ: Keep-строки `border`/`fileDragMode`
    // заполняют отсутствие каноном на чтении — без этого X-равенство sparse-EDT и
    // dense-Designer невозможно. У `fileDragMode` дефолты диалектов ПРОТИВОПОЛОЖНЫ
    // (EDT-омиссия ⟺ AsFile, Designer-омиссия ⟺ AsFileRef — witnessed, см.
    // FILE_DRAG_MODE_*_DEFAULT): рукотворный разреженный bag потому читается в РАЗНЫЕ
    // fills; реальная конверсия всегда несёт заполненное значение, и оба диалекта
    // эмитят его явно, когда оно не равно ИХ омиссии. Неподвижная точка — заполненный bag.
    let expected_ext = |drag_fill: &str| {
        vec![
            (
                ld::F_EXT_PICTURE,
                PropertyValue::List(vec![
                    PropertyValue::Ref("CommonPicture.Силуэт".into()),
                    PropertyValue::Bool(false),
                ]),
            ),
            (
                ld::F_EXT_PIC_BORDER,
                PropertyValue::Enum(Token::new("WithoutBorder")),
            ),
            (
                super::tables::F_LD_BORDER_COLOR,
                PropertyValue::Ref("Web.LightSalmon".into()),
            ),
            (
                ld::F_EXT_FILE_DRAG_MODE,
                PropertyValue::Enum(Token::new(drag_fill)),
            ),
        ]
    };
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let body = build();
        let bytes = write_form(dialect, &body).expect("write");
        let back = read_form(dialect, &bytes).expect("read (Shortcut/BorderColor now modeled)");
        assert_eq!(back.items.len(), 1, "{dialect:?}: one control");
        assert_eq!(
            back.items[0].properties, body.items[0].properties,
            "{dialect:?}: body (shortcut) round-trips"
        );
        let drag_fill = match dialect {
            FormDialect::Edt => ld::FILE_DRAG_MODE_EDT_DEFAULT,
            FormDialect::Designer => ld::FILE_DRAG_MODE_DESIGNER_DEFAULT,
        };
        assert_eq!(
            back.items[0].ext_info,
            expected_ext(drag_fill),
            "{dialect:?}: extInfo (borderColor) round-trips into the FILLED canonical bag"
        );
        // Неподвижная точка: запись заполненного bag'а даёт ТЕ ЖЕ байты (Keep-omit зеркален).
        let bytes2 = write_form(dialect, &back).expect("write filled bag");
        assert_eq!(bytes2, bytes, "{dialect:?}: filled bag is the round-trip fixed point");
    }
}

/// Designer write→read round-trips the same IR (cross-format consistency of the connector).
#[test]
fn designer_minimal_form_roundtrips_ir() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    let bytes = write_form(FormDialect::Designer, &body).expect("write");
    let back = read_form(FormDialect::Designer, &bytes).expect("read");
    assert_eq!(
        back.attributes, body.attributes,
        "designer form attrs round-trip"
    );
}

// ===================== Commands / CommandSet / ExcludedCommand (LANE-F-1) =====================

use morph1c_core::ir::{FormCommand, Lang};
use morph1c_core::spec::forms::command as fc;

/// Полнопольная команда формы (все канонические поля не-дефолтны).
fn full_command() -> FormCommand {
    let mut cmd = FormCommand::new("Обновить", 7);
    cmd.properties = vec![
        (
            fc::F_TITLE,
            PropertyValue::Localized(vec![(Lang::new("ru"), "Обновить".to_string())]),
        ),
        (
            fc::F_TOOL_TIP,
            PropertyValue::Localized(vec![(Lang::new("ru"), "Обновить список".to_string())]),
        ),
        (fc::F_SHORTCUT, PropertyValue::Str("F5".to_string())),
        (
            fc::F_PICTURE,
            // W25-канон `List([Ref, Bool(loadTransparent)])`; StdPicture.Refresh ⇒ true.
            PropertyValue::List(vec![
                PropertyValue::Ref("StdPicture.Refresh".to_string()),
                PropertyValue::Bool(true),
            ]),
        ),
        (fc::F_ACTION, PropertyValue::Str("Обновить".to_string())),
        (
            fc::F_ACTION_PURPOSE,
            PropertyValue::Enum(Token::new("Finish")),
        ),
        (
            fc::F_REPRESENTATION,
            PropertyValue::Enum(Token::new("Picture")),
        ),
        (fc::F_MODIFIES_STORED_DATA, PropertyValue::Bool(true)),
        (
            fc::F_CURRENT_ROW_USE,
            PropertyValue::Enum(Token::new("DontUse")),
        ),
        (
            fc::F_ASSOCIATED_TABLE_ELEMENT_ID,
            PropertyValue::Str("Список".to_string()),
        ),
        (
            fc::F_SELECTED_ROWS_USE,
            PropertyValue::Enum(Token::new("DontUse")),
        ),
    ];
    cmd
}

/// Команды + форм-уровневые исключённые команды round-trip'ятся ОБОИМИ диалектами в
/// РАВНЫЙ IR (X by construction: оба ридера дают один канонический bag).
#[test]
fn commands_and_excluded_roundtrip_both_dialects() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.commands.push(full_command());
    // Вторая команда: минимальная (только required-поля bag — currentRowUse/selectedRowsUse
    // заполняются ридером пер-форматным дефолтом; тут канонические значения).
    let mut minimal = FormCommand::new("Пустышка", 8);
    minimal.properties = vec![
        (
            fc::F_CURRENT_ROW_USE,
            PropertyValue::Enum(Token::new("Use")),
        ),
        (
            fc::F_SELECTED_ROWS_USE,
            PropertyValue::Enum(Token::new("Use")),
        ),
    ];
    body.commands.push(minimal);
    body.excluded_commands = vec!["Abort".to_string(), "OK".to_string(), "Retry".to_string()];

    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        assert_eq!(
            back.commands, body.commands,
            "{dialect:?}: commands round-trip"
        );
        assert_eq!(
            back.excluded_commands, body.excluded_commands,
            "{dialect:?}: excluded_commands round-trip"
        );
        // Байт-стабильность: повторная запись прочитанного IR идентична.
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: write∘read byte-stable");
    }
}

/// Пер-форматные дефолты `currentRowUse`/`selectedRowsUse`: минимальная команда, чей bag
/// несёт EDT-дефолт `Use`, НЕ эмитит тегов в EDT — а Designer эмитит `Use` явно (его
/// дефолт `Auto`); оба ридера восстанавливают ОДИН bag.
#[test]
fn command_row_use_per_format_defaults() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    let mut cmd = FormCommand::new("К", 1);
    cmd.properties = vec![
        (
            fc::F_CURRENT_ROW_USE,
            PropertyValue::Enum(Token::new("Use")),
        ),
        (
            fc::F_SELECTED_ROWS_USE,
            PropertyValue::Enum(Token::new("Use")),
        ),
    ];
    body.commands.push(cmd);

    let edt_bytes = write_form(FormDialect::Edt, &body).expect("edt write");
    let edt_text = String::from_utf8(edt_bytes.clone()).unwrap();
    assert!(
        !edt_text.contains("currentRowUse"),
        "EDT omits its default Use"
    );
    let des_bytes = write_form(FormDialect::Designer, &body).expect("designer write");
    let des_text = String::from_utf8(des_bytes.clone()).unwrap();
    assert!(
        des_text.contains("<CurrentRowUse>Use</CurrentRowUse>"),
        "Designer emits non-default Use"
    );

    let e = read_form(FormDialect::Edt, &edt_bytes).expect("edt read");
    let d = read_form(FormDialect::Designer, &des_bytes).expect("designer read");
    assert_eq!(
        e.commands, d.commands,
        "one canonical bag from both dialects"
    );
}

// ===================== Форм-уровневый КИ: group/index cmiFragmentRecord =====================

/// `<index>` БЕЗ `<group>` (размещение по индексу на КОРНЕ панели; census ERP 325 фрагментов,
/// значения ≥1) round-trip'ится ОБОИМИ диалектами — рядом с ПАРОЙ group+index (сохранён прежний
/// инвариант) и обычным фрагментом (ни группы, ни индекса). Оба ридера дают РАВНЫЙ IR (X).
#[test]
fn cmi_index_without_group_roundtrips_both_dialects() {
    use morph1c_core::ir::FormCiItem;
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.command_interface = true;
    body.form_ci_navigation_panel = vec![
        // Пара group+index (index под группой — прежний витнесснутый шейп).
        FormCiItem {
            command: "CommonCommand.Первая".into(),
            ty: "Auto".into(),
            group: Some("FormCommandBarImportant".into()),
            index: Some(2),
            user_visible: Some(false),
            user_visible_roles: Vec::new(),
        },
        // index-БЕЗ-группы (новый ERP-шейп; N≥1).
        FormCiItem {
            command: "InformationRegister.X.Command.Y".into(),
            ty: "Auto".into(),
            group: None,
            index: Some(3),
            user_visible: Some(false),
            user_visible_roles: Vec::new(),
        },
        // Ни группы, ни индекса (обычный фрагмент).
        FormCiItem {
            command: "CommonCommand.Третья".into(),
            ty: "Auto".into(),
            group: None,
            index: None,
            user_visible: None,
            user_visible_roles: Vec::new(),
        },
    ];

    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        assert_eq!(
            back.form_ci_navigation_panel, body.form_ci_navigation_panel,
            "{dialect:?}: cmi navigation panel round-trip (index-without-group decoupled)"
        );
        // Байт-стабильность: повторная запись прочитанного IR идентична.
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: cmi write∘read byte-stable");
    }
}

/// Designer опускает `<Index>` == 0: group+index-0 эмитит `<CommandGroup>` БЕЗ `<Index>`, и это
/// читается как `index=Some(0)` (X-равно EDT `<index>0`). Инвариант пары group+index сохранён.
#[test]
fn cmi_designer_group_index_zero_omits_index_leaf() {
    use morph1c_core::ir::FormCiItem;
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.command_interface = true;
    body.form_ci_navigation_panel = vec![FormCiItem {
        command: "CommonCommand.Ноль".into(),
        ty: "Auto".into(),
        group: Some("FormCommandBarImportant".into()),
        index: Some(0),
        user_visible: Some(false),
        user_visible_roles: Vec::new(),
    }];

    let des_bytes = write_form(FormDialect::Designer, &body).expect("designer write");
    let des_text = String::from_utf8(des_bytes.clone()).unwrap();
    assert!(
        des_text.contains("<CommandGroup>FormCommandBarImportant</CommandGroup>"),
        "designer emits the group"
    );
    assert!(
        !des_text.contains("<Index>"),
        "designer omits <Index> for index 0"
    );
    let d = read_form(FormDialect::Designer, &des_bytes).expect("designer read");
    // EDT эмитит `<index>0` ЯВНО при group ⇒ оба дают index=Some(0) (X).
    let edt_bytes = write_form(FormDialect::Edt, &body).expect("edt write");
    assert!(
        String::from_utf8(edt_bytes.clone())
            .unwrap()
            .contains("<index>0</index>"),
        "EDT emits explicit <index>0 under group"
    );
    let e = read_form(FormDialect::Edt, &edt_bytes).expect("edt read");
    assert_eq!(
        e.form_ci_navigation_panel, d.form_ci_navigation_panel,
        "one canonical bag (index=Some(0)) from both dialects"
    );
    assert_eq!(d.form_ci_navigation_panel[0].index, Some(0));
}

/// §1.0: EDT `<group>` БЕЗ `<index>` не витнесснут (census 0/8194) → типизированный отказ (не
/// тихая привязка к 0).
#[test]
fn cmi_edt_group_without_index_is_typed_error() {
    use morph1c_core::ir::FormCiItem;
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.command_interface = true;
    body.form_ci_navigation_panel = vec![FormCiItem {
        command: "CommonCommand.Первая".into(),
        ty: "Auto".into(),
        group: Some("FormCommandBarImportant".into()),
        index: Some(2),
        user_visible: Some(false),
        user_visible_roles: Vec::new(),
    }];
    let bytes = write_form(FormDialect::Edt, &body).expect("write");
    // Позитивный контроль: пара group+index читается.
    read_form(FormDialect::Edt, &bytes).expect("group+index reads");
    // Убрать `<index>2</index>` ⇒ фрагмент несёт group БЕЗ index.
    let text = String::from_utf8(bytes).unwrap();
    let mutated = text.replace("<index>2</index>", "");
    let err = read_form(FormDialect::Edt, mutated.as_bytes())
        .expect_err("EDT <group> without <index> must error (§1.0)");
    assert!(
        format!("{err}").contains("without <index>"),
        "error must name the group-without-index violation: {err}"
    );
}

/// §1.0: неопознанный ребёнок команды — типизированная ошибка (нет Raw/skip).
#[test]
fn command_stray_child_is_typed_error() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.commands.push(full_command());
    let bytes = write_form(FormDialect::Edt, &body).expect("write");
    let text = String::from_utf8(bytes).unwrap();
    let mutated = text.replace(
        "<shortcut>F5</shortcut>",
        "<shortcut>F5</shortcut>\r\n    <unknownTag>x</unknownTag>",
    );
    let err = read_form(FormDialect::Edt, mutated.as_bytes()).expect_err("stray child must error");
    assert!(
        format!("{err}").contains("unknownTag"),
        "error names the stray node: {err}"
    );
}

/// §1.0 (LANE-F-3, nit a): DataPath ОБЯЗАН нести РОВНО один `<segments>`. Позитивный
/// контроль (один сегмент читается) + негатив: два `<segments>` → типизированная ошибка,
/// а не молчаливый выбор первого (ранее покрывался лишь happy-path).
#[test]
fn datapath_multiple_segments_is_typed_error() {
    use morph1c_core::spec::forms::controls::form_field as ff;
    let mut item = FormItem::new(FormControlKind::new("InputField"), "Поле", 1);
    item.properties = vec![
        (ff::F_ENABLED, PropertyValue::Bool(true)),
        (ff::F_USER_VISIBLE, PropertyValue::Bool(true)),
        (
            ff::F_DATA_PATH,
            PropertyValue::Ref("Объект.Поле".to_string()),
        ),
    ];
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.items = vec![item];
    let bytes = write_form(FormDialect::Edt, &body).expect("write");
    // Позитивный контроль: один `<segments>` читается без ошибки.
    read_form(FormDialect::Edt, &bytes).expect("single-segment DataPath reads");
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.contains("<segments>Объект.Поле</segments>"),
        "single segment emitted"
    );
    // Негатив: инъекция второго `<segments>` ⇒ §1.0-ошибка.
    let mutated = text.replace(
        "<segments>Объект.Поле</segments>",
        "<segments>Объект.Поле</segments>\r\n        <segments>Второй</segments>",
    );
    let err = read_form(FormDialect::Edt, mutated.as_bytes())
        .expect_err("DataPath with >1 <segments> must error (§1.0)");
    assert!(
        format!("{err}").contains("segments"),
        "error must name the segments violation: {err}"
    );
}

// ===================== Версионный Designer-конверт `<Form>` (2.20 ERP / 2.21 SSL) =====================

/// Drift-guard реестра конвертов (зеркало `formats_designer::common::tests`): ERP-блок ==
/// SSL-блок МИНУС `xmlns:pal` (тот же порядок), а `version_value` каждого профиля ==
/// `format.to_string()` (производность от FORMATS.md §2, не второй хардкод).
#[test]
fn form_envelope_profiles_hold_their_invariants() {
    let ssl_minus_pal: Vec<_> = DESIGNER_FORM_NS
        .iter()
        .filter(|(name, _)| *name != "xmlns:pal")
        .collect();
    let erp: Vec<_> = DESIGNER_FORM_NS_ERP.iter().collect();
    assert_eq!(
        erp, ssl_minus_pal,
        "ERP <Form> ns-block must be exactly the SSL block minus xmlns:pal, same order"
    );
    assert!(
        DESIGNER_FORM_NS.iter().any(|(n, _)| *n == "xmlns:pal"),
        "SSL block must carry pal"
    );
    for p in form_envelope_profiles() {
        assert_eq!(
            p.version_value,
            p.format.to_string(),
            "version_value must be derived from FormatVersion (FORMATS.md §2)"
        );
    }
}

/// БЕЗ амбьентного таргета Designer-писатель эмитит ПРЕЖНИЙ SSL-конверт (2.21, с `xmlns:pal`)
/// — SSL-путь байт-в-байт не двигается; под таргетом ERP — 2.20-конверт (БЕЗ `xmlns:pal`).
/// Ридер детектит ОБЕ версии по `version=` САМОГО файла и даёт равный IR (версия — свойство
/// файла, в `FormBody` не течёт); write∘read байт-стабилен в СВОЕЙ версии.
#[test]
fn designer_form_envelope_is_versioned() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();

    let ssl_bytes = write_form(FormDialect::Designer, &body).expect("ssl write");
    let ssl_text = String::from_utf8(ssl_bytes.clone()).unwrap();
    assert!(
        ssl_text.contains(" version=\"2.21\">"),
        "default target stays 2.21"
    );
    assert!(
        ssl_text.contains(" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\""),
        "2.21 root declares xmlns:pal"
    );

    let erp_bytes = morph1c_core::version::with_roundtrip_target(
        morph1c_core::version::ERP,
        || write_form(FormDialect::Designer, &body),
    )
    .expect("erp write");
    let erp_text = String::from_utf8(erp_bytes.clone()).unwrap();
    assert!(
        erp_text.contains(" version=\"2.20\">"),
        "ERP target stamps 2.20"
    );
    assert!(
        !erp_text.contains("xmlns:pal"),
        "2.20 root must NOT declare xmlns:pal"
    );

    let from_ssl = read_form(FormDialect::Designer, &ssl_bytes).expect("2.21 envelope reads");
    let from_erp = read_form(FormDialect::Designer, &erp_bytes).expect("2.20 envelope reads");
    assert_eq!(
        from_ssl.attributes, from_erp.attributes,
        "version must not leak into FormBody"
    );

    let erp_again = morph1c_core::version::with_roundtrip_target(
        morph1c_core::version::ERP,
        || write_form(FormDialect::Designer, &from_erp),
    )
    .unwrap();
    assert_eq!(erp_again, erp_bytes, "2.20 write∘read byte-stable");
}

/// §1.0: не-witnessed версия — отказ и на read (чужой `version=` с именованием witnessed-
/// набора), и на write (не-witnessed амбьентный таргет); а 2.20-файл, несущий ЧУЖОЙ
/// `xmlns:pal`, отвергает тотальность (лишний ns-атрибут не claim'ится).
#[test]
fn designer_form_unwitnessed_version_and_alien_ns_are_refused() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    let text = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();

    // Не-witnessed версия на read.
    let mutated = text.replace(" version=\"2.21\">", " version=\"2.19\">");
    let err = read_form(FormDialect::Designer, mutated.as_bytes())
        .expect_err("2.19 envelope is not witnessed — read must refuse");
    let msg = format!("{err}");
    assert!(
        msg.contains("2.19") && msg.contains("witnessed"),
        "error names the version and the witnessed set: {msg}"
    );

    // 2.20-файл с 2.21-ns-блоком (лишний pal) — тотальность (§1.0), не проглатывание.
    let mutated = text.replace(" version=\"2.21\">", " version=\"2.20\">");
    let err = read_form(FormDialect::Designer, mutated.as_bytes())
        .expect_err("a 2.20 file carrying xmlns:pal must be refused (totality)");
    assert!(
        format!("{err}").contains("pal"),
        "error names the alien ns attribute: {err}"
    );

    // Не-witnessed таргет на write. (2.17 с версии witness'а `scripts/version_probe.sh`
    // ВИТНЕССИРОВАН и пишется — здесь берётся 2.19, которой оракула нет.)
    let err = morph1c_core::version::with_roundtrip_target(
        morph1c_core::version::FormatVersion::new(2, 19),
        || write_form(FormDialect::Designer, &body),
    )
    .expect_err("2.19 target has no witnessed <Form> envelope — write must refuse");
    assert!(
        format!("{err}").contains("2.19"),
        "write error names the target: {err}"
    );
}

/// 2.17-конверт `<Form>` ВИТНЕССИРОВАН (`scripts/version_probe.sh`: ns-блок корня формы в
/// 2.17 байт-идентичен 2.20-му, различается только `version=`), поэтому и read, и write под
/// таргетом 2.17 обязаны РАБОТАТЬ — до этого 2.17-источник не читался в принципе.
#[test]
fn designer_form_2_17_envelope_round_trips() {
    const V217: morph1c_core::version::FormatVersion =
        morph1c_core::version::FormatVersion::new(2, 17);
    let mut body = FormBody::new();
    body.attributes = base_attrs();

    let bytes = morph1c_core::version::with_roundtrip_target(V217, || {
        write_form(FormDialect::Designer, &body)
    })
    .expect("2.17 is a witnessed <Form> envelope");
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(text.contains(" version=\"2.17\">"), "2.17 stamp: {}", &text[..300.min(text.len())]);
    assert!(!text.contains("xmlns:pal"), "2.17 ns-block carries no `pal`");

    let back = read_form(FormDialect::Designer, &bytes).expect("2.17 envelope reads back");
    let regen = morph1c_core::version::with_roundtrip_target(V217, || {
        write_form(FormDialect::Designer, &back)
    })
    .expect("re-write");
    assert_eq!(regen, bytes, "2.17 Designer form envelope round-trips byte-exactly");
}

/// W25: Designer `<xr:LoadTransparent>` — НЕЗАВИСИМЫЙ флаг (ERP несёт CommonPicture/Abs c
/// LoadTransparent="true" ПРОТИВ префикс-правила StdPicture⇒true). Читается как есть и
/// round-trip'ит ОБЕ полярности байт-точно (раньше true≠дерив отвергался).
#[test]
fn designer_command_picture_load_transparent_is_independent() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.commands.push(full_command());
    let bytes = write_form(FormDialect::Designer, &body).expect("write");
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.contains("<xr:LoadTransparent>true</xr:LoadTransparent>"),
        "canon carries LoadTransparent=true → emitted verbatim"
    );
    // Мутируем true→false: теперь это ВАЛИДНАЯ независимая комбинация — не ошибка чтения.
    let mutated = text.replace(
        "<xr:LoadTransparent>true</xr:LoadTransparent>",
        "<xr:LoadTransparent>false</xr:LoadTransparent>",
    );
    let body2 = read_form(FormDialect::Designer, mutated.as_bytes())
        .expect("independent LoadTransparent=false must read, not error");
    // Канон несёт ПРОЧИТАННЫЙ (false), а не дерив-значение (StdPicture⇒true).
    let pic = body2.commands[0]
        .get(fc::F_PICTURE)
        .expect("command carries a picture");
    assert_eq!(
        pic,
        &PropertyValue::List(vec![
            PropertyValue::Ref("StdPicture.Refresh".to_string()),
            PropertyValue::Bool(false),
        ]),
        "independent LoadTransparent=false stored as-is (not re-derived to true): {pic:?}"
    );
    // И пере-запись эмитит именно false (round-trip флага байт-точен).
    let text2 =
        String::from_utf8(write_form(FormDialect::Designer, &body2).expect("re-write")).unwrap();
    assert!(
        text2.contains("<xr:LoadTransparent>false</xr:LoadTransparent>")
            && !text2.contains("<xr:LoadTransparent>true</xr:LoadTransparent>"),
        "re-written LoadTransparent is false, not re-derived true: {text2}"
    );
}

/// §1.0: не-bool Designer `<xr:LoadTransparent>` — типизированный отказ (не глотается).
#[test]
fn designer_command_picture_load_transparent_non_bool_is_typed_error() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.commands.push(full_command());
    let text =
        String::from_utf8(write_form(FormDialect::Designer, &body).expect("write")).unwrap();
    let mutated = text.replace(
        "<xr:LoadTransparent>true</xr:LoadTransparent>",
        "<xr:LoadTransparent>maybe</xr:LoadTransparent>",
    );
    let err = read_form(FormDialect::Designer, mutated.as_bytes())
        .expect_err("non-bool LoadTransparent must error");
    assert!(
        format!("{err}").contains("LoadTransparent"),
        "error names the field: {err}"
    );
}

// ===================== KLASS 1: attribute View/Edit common+roles (ERP W23) =====================

use morph1c_core::ir::{FormDataAttribute, TypeRef, TypeSpec};

/// Скалярный (Boolean) данные-реквизит с заданными view/edit флагами общего доступа и ролями.
fn bool_data_attr(
    name: &str,
    id: i64,
    view_common: bool,
    view_roles: Vec<(String, bool)>,
    edit_common: bool,
    edit_roles: Vec<(String, bool)>,
) -> FormDataAttribute {
    FormDataAttribute {
        name: name.to_string(),
        id,
        title: None,
        value_type: Some(TypeSpec {
            parts: vec![TypeRef {
                id: "Boolean".to_string(),
                qualifier: None,
            }],
        }),
        fill_checking: None,
        view_common,
        edit_common,
        view_roles,
        edit_roles,
        main: false,
        saved_data: false,
        settings_saved_data: Vec::new(),
        designer_unavailable_paths: Vec::new(),
        columns: Vec::new(),
        additional_columns: Vec::new(),
        value_list_ext: false,
        value_list_item_type: None,
        spreadsheet_ext: false,
        functional_options: Vec::new(),
        not_default_use_always: Vec::new(),
        dynamic_list: None,
        chart_settings: None,
        spreadsheet_settings: None,
    }
}

fn role(name: &str) -> (String, bool) {
    (name.to_string(), true)
}

/// Все витнесснутые шейпы View/Edit-прав реквизита round-trip'ятся ОБОИМИ диалектами в РАВНЫЙ
/// IR, и EDT-канон == Designer-канон (X). Покрывает: (false,roles) — SSL-запрет+роли;
/// (true,roles) — ОБЩИЙ доступ С ролевыми исключениями (ERP W23 ФизическиеЛица.ФормаЭлемента);
/// (true,∅) — дефолт (edt `<common>true`, designer absent); (false,∅) — запрет без ролей.
#[test]
fn attr_view_edit_common_roles_roundtrip_both_dialects() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.data_attributes = vec![
        bool_data_attr(
            "A",
            1,
            false,
            vec![role("Role.ViewA1"), role("Role.ViewA2")],
            false,
            vec![role("Role.EditA1")],
        ),
        bool_data_attr(
            "B",
            2,
            true,
            vec![role("Role.ViewB1")],
            true,
            vec![role("Role.EditB1"), role("Role.EditB2")],
        ),
        bool_data_attr("C", 3, true, vec![], true, vec![]),
        bool_data_attr("D", 4, false, vec![], false, vec![]),
    ];

    let mut per_dialect = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        assert_eq!(
            back.data_attributes, body.data_attributes,
            "{dialect:?}: data-attribute view/edit round-trip"
        );
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: write∘read byte-stable");
        per_dialect.push(back.data_attributes);
    }
    assert_eq!(
        per_dialect[0], per_dialect[1],
        "EDT и Designer одной формы канонизуются в идентичный IR (X)"
    );
}

// ============ ERP Gantt-каскад: attribute extInfo `form:GanttChartExtInfo` (пустой маркер) ============

/// GanttChart-типизированный данные-реквизит (backing GanttChartField). Тело диаграммы держит
/// сайдкар `Attributes/<attr>/ExtInfo/GanttChart.chart` (chart_settings=None ЗДЕСЬ, вне
/// Form.form); Form.form несёт лишь ПУСТОЙ EDT-маркер `form:GanttChartExtInfo`, детерминированный
/// скаляр-типом GanttChart. Designer маркера не несёт вовсе (регенерация на EDT-write).
fn gantt_data_attr(name: &str, id: i64) -> FormDataAttribute {
    let mut a = bool_data_attr(name, id, true, vec![], true, vec![]);
    a.value_type = Some(TypeSpec {
        parts: vec![TypeRef {
            id: "GanttChart".to_string(),
            qualifier: None,
        }],
    });
    a
}

/// GanttChart-реквизит round-trip'ится ОБОИМИ диалектами в РАВНЫЙ IR (X: EDT-канон ==
/// Designer-канон), а EDT несёт ПУСТОЙ самозакрытый маркер `form:GanttChartExtInfo`.
#[test]
fn gantt_data_attr_ext_info_marker_roundtrip_both_dialects() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.data_attributes = vec![gantt_data_attr("ГрафикЗанятостиРЦ", 25)];

    // EDT эмитит пустой самозакрытый маркер (детерминант — value_type=GanttChart, не chart_settings).
    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    assert!(
        edt.contains("<extInfo xsi:type=\"form:GanttChartExtInfo\"/>"),
        "EDT emits the empty GanttChartExtInfo marker: {edt}"
    );

    let mut per_dialect = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        assert_eq!(
            back.data_attributes, body.data_attributes,
            "{dialect:?}: GanttChart attr round-trip"
        );
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: write∘read byte-stable");
        per_dialect.push(back.data_attributes);
    }
    assert_eq!(
        per_dialect[0], per_dialect[1],
        "EDT и Designer GanttChart-реквизита ⇒ идентичный IR (X)"
    );
}

/// §1.0: `form:GanttChartExtInfo` — ПУСТОЙ маркер; любой под-узел ⇒ громкий типизированный отказ
/// (невитнесснутое под-поле не глотаем).
#[test]
fn gantt_ext_info_nonempty_is_typed_error() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.data_attributes = vec![gantt_data_attr("Г", 25)];
    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    let mutated = edt.replace(
        "<extInfo xsi:type=\"form:GanttChartExtInfo\"/>",
        "<extInfo xsi:type=\"form:GanttChartExtInfo\"><foo>x</foo></extInfo>",
    );
    let err = read_form(FormDialect::Edt, mutated.as_bytes())
        .expect_err("non-empty GanttChartExtInfo must error (§1.0)");
    assert!(
        format!("{err}").contains("must be empty"),
        "error names the violation: {err}"
    );
}

/// §1.0: маркер ⟺ скаляр-тип GanttChart. Маркер, оставленный при НЕ-GanttChart-типе, ⇒ отказ
/// (иначе — тихая пере/недо-генерация маркера на кросс-конверсии).
#[test]
fn gantt_ext_info_marker_type_divergence_is_typed_error() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.data_attributes = vec![gantt_data_attr("Г", 25)];
    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    // Тип → Boolean, но маркер GanttChartExtInfo оставлен ⇒ scalar_marker != determinant.
    let mutated = edt.replace("<types>GanttChart</types>", "<types>Boolean</types>");
    let err = read_form(FormDialect::Edt, mutated.as_bytes())
        .expect_err("marker without its GanttChart determinant must error (§1.0)");
    assert!(
        format!("{err}").contains("determinant"),
        "error flags the marker/type divergence: {err}"
    );
}

/// Байты общего-доступа-С-ролями: EDT эмитит ВЕДУЩИЙ `<common>true</common>` перед `<for>`;
/// Designer — `<xr:Common>true</xr:Common>` + `<xr:Value>` роли (не absent, т.к. роли есть).
#[test]
fn attr_common_true_with_roles_emits_witnessed_bytes() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.data_attributes = vec![bool_data_attr(
        "B",
        2,
        true,
        vec![role("Role.V")],
        true,
        vec![role("Role.E")],
    )];

    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    assert!(
        edt.contains("<common>true</common>") && edt.contains("<role>Role.V</role>"),
        "EDT view/edit common=true+roles: ведущий <common>true> + <for>-роли: {edt}"
    );
    let des = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    assert!(
        des.contains("<xr:Common>true</xr:Common>")
            && des.contains("<xr:Value name=\"Role.V\">true</xr:Value>"),
        "Designer emits Common=true + role Value: {des}"
    );
}

/// §1.0: Designer `<View><xr:Common>true</xr:Common></View>` БЕЗ `<xr:Value>`-ролей неотличим от
/// absent-дефолта (true,∅) ⇒ не round-trip'ится ⇒ типизированный отказ (не тихое схлопывание).
#[test]
fn designer_view_common_true_without_roles_is_typed_error() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.data_attributes = vec![bool_data_attr(
        "B",
        2,
        true,
        vec![role("Role.V")],
        false,
        vec![role("Role.E")],
    )];
    let des = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    // Снять единственную роль у View ⇒ `<View><xr:Common>true></View>` без ролей.
    let mutated = des.replace("<xr:Value name=\"Role.V\">true</xr:Value>", "");
    let err = read_form(FormDialect::Designer, mutated.as_bytes())
        .expect_err("designer present-true View without roles must error (§1.0)");
    assert!(
        format!("{err}").contains("without <xr:Value>-roles"),
        "error names the violation: {err}"
    );
}

/// §1.0: EDT `<edit><common>false</common><for>…` (present-false рядом с ролями) не витнессирован
/// (EDT опускает false) ⇒ типизированный отказ.
#[test]
fn edt_common_false_alongside_for_is_typed_error() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    // view (true,roles) ⇒ ровно один `<common>true</common>`; edit (false,roles) ⇒ без `<common>`.
    body.data_attributes = vec![bool_data_attr(
        "B",
        2,
        true,
        vec![role("Role.V")],
        false,
        vec![role("Role.E")],
    )];
    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    let mutated = edt.replace("<common>true</common>", "<common>false</common>");
    let err = read_form(FormDialect::Edt, mutated.as_bytes())
        .expect_err("EDT <common>false alongside <for> must error (§1.0)");
    assert!(
        format!("{err}").contains("want \"true\""),
        "error names the violation: {err}"
    );
}

// ===================== KLASS 3: formCommand <use> common=true + roles (ERP W23) ===============

/// FormCommand `<use>` с common=true И ролевыми исключениями (ERP W23
/// ЖурналДокументовБезналичныеПлатежи.ФормаСписка) round-trip'ится ОБОИМИ диалектами в равный IR:
/// EDT `<use><common>true</common><for>×N`, Designer `<Use><xr:Common>true</xr:Common><xr:Value>×N`.
#[test]
fn command_use_common_true_roles_roundtrip_both_dialects() {
    use super::tables;
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    let mut cmd = FormCommand::new("Платежи", 5);
    cmd.properties = vec![
        (fc::F_CURRENT_ROW_USE, PropertyValue::Enum(Token::new("Use"))),
        (
            fc::F_SELECTED_ROWS_USE,
            PropertyValue::Enum(Token::new("Use")),
        ),
        (
            tables::F_CMD_USE,
            PropertyValue::List(vec![
                PropertyValue::Bool(true),
                PropertyValue::List(vec![
                    PropertyValue::Str("Role.R1".to_string()),
                    PropertyValue::Bool(true),
                ]),
                PropertyValue::List(vec![
                    PropertyValue::Str("Role.R2".to_string()),
                    PropertyValue::Bool(true),
                ]),
            ]),
        ),
    ];
    body.commands.push(cmd);

    let expected_use = PropertyValue::List(vec![
        PropertyValue::Bool(true),
        PropertyValue::List(vec![
            PropertyValue::Str("Role.R1".to_string()),
            PropertyValue::Bool(true),
        ]),
        PropertyValue::List(vec![
            PropertyValue::Str("Role.R2".to_string()),
            PropertyValue::Bool(true),
        ]),
    ]);
    let mut per_dialect = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        assert_eq!(
            back.commands[0].get(tables::F_CMD_USE),
            Some(&expected_use),
            "{dialect:?}: command <use> common=true+roles value round-trip"
        );
        // Байт-стабильность повторной записи (кодек само-консистентен, порядок-нейтрален).
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: write∘read byte-stable");
        per_dialect.push(back.commands[0].get(tables::F_CMD_USE).cloned());
    }
    assert_eq!(per_dialect[0], per_dialect[1], "cross-dialect use canon (X)");

    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    assert!(
        edt.contains("<use>") && edt.contains("<common>true</common>") && edt.contains("<role>Role.R1</role>"),
        "EDT <use> emits leading <common>true> + <for>-roles: {edt}"
    );
    let des = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    assert!(
        des.contains("<xr:Common>true</xr:Common>")
            && des.contains("<xr:Value name=\"Role.R1\">true</xr:Value>"),
        "Designer <Use> emits Common=true + role Value: {des}"
    );
}

// ===================== UserVisible-роли (form-control + CI-item) =====================

/// Построить форму с ОДНИМ LabelDecoration, несущим заданное значение `userVisible`.
fn form_with_user_visible(uv: PropertyValue) -> FormBody {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    let mut item = FormItem::new(FormControlKind::new("LabelDecoration"), "L", 1);
    item.properties = vec![
        (ld::F_ENABLED, PropertyValue::Bool(true)),
        (ld::F_USER_VISIBLE, uv),
        (ld::F_MAX_WIDTH, PropertyValue::Int(100)),
    ];
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.items = vec![item];
    body
}

/// НЕРЕГРЕСС (hot-path 372k носителей): БЕЗролевой `userVisible` (common-only) остаётся РОВНО
/// `Bool` и его вывод байт-стабилен в обоих диалектах — роли пусты ⇒ прежний путь и байты.
#[test]
fn user_visible_common_only_no_roles_byte_identical() {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    for uv in [PropertyValue::Bool(true), PropertyValue::Bool(false)] {
        for dialect in [FormDialect::Edt, FormDialect::Designer] {
            let body = form_with_user_visible(uv.clone());
            let bytes = write_form(dialect, &body).expect("write");
            let back = read_form(dialect, &bytes).expect("read");
            assert_eq!(
                back.items[0].get(ld::F_USER_VISIBLE),
                Some(&uv),
                "{dialect:?}: userVisible={uv:?} value unchanged (still Bool, no roles)"
            );
            let bytes2 = write_form(dialect, &back).unwrap();
            assert_eq!(bytes, bytes2, "{dialect:?}: userVisible={uv:?} byte-stable");
        }
    }
}

/// КЛАСС 1: `userVisible` с ролями БЕЗ ведущего `<common>` (common=false) — witness
/// РесурсныеСпецификации.ФормаЭлемента. Round-trip байт-стабилен и X-равен по диалектам.
#[test]
fn user_visible_roles_without_common_roundtrip_both_dialects() {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    let uv = PropertyValue::List(vec![
        PropertyValue::Bool(false),
        PropertyValue::List(vec![
            PropertyValue::Str("Role.A".into()),
            PropertyValue::Bool(true),
        ]),
        PropertyValue::List(vec![
            PropertyValue::Str("Role.B".into()),
            PropertyValue::Bool(true),
        ]),
    ]);
    let mut per = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let body = form_with_user_visible(uv.clone());
        let bytes = write_form(dialect, &body).expect("write");
        let back = read_form(dialect, &bytes).expect("read");
        assert_eq!(
            back.items[0].get(ld::F_USER_VISIBLE),
            Some(&uv),
            "{dialect:?}: roles-without-common value round-trip"
        );
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: byte-stable");
        per.push(back.items[0].get(ld::F_USER_VISIBLE).cloned());
    }
    assert_eq!(per[0], per[1], "cross-dialect userVisible-roles canon (X)");
    let edt = String::from_utf8(write_form(FormDialect::Edt, &form_with_user_visible(uv.clone())).unwrap())
        .unwrap();
    assert!(
        edt.contains("<role>Role.A</role>") && !edt.contains("<common>"),
        "EDT roles-without-common: <for> без <common>: {edt}"
    );
    let des = String::from_utf8(
        write_form(FormDialect::Designer, &form_with_user_visible(uv)).unwrap(),
    )
    .unwrap();
    assert!(
        des.contains("<xr:Common>false</xr:Common>")
            && des.contains("<xr:Value name=\"Role.A\">true</xr:Value>"),
        "Designer common=false + xr:Value роли: {des}"
    );
}

/// КЛАСС 1: `userVisible` с ВЕДУЩИМ `<common>true</common>` + роли — witness
/// мирСкважины.ФормаЭлемента (общий доступ с ролевыми исключениями).
#[test]
fn user_visible_common_true_with_roles_roundtrip_both_dialects() {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    let uv = PropertyValue::List(vec![
        PropertyValue::Bool(true),
        PropertyValue::List(vec![
            PropertyValue::Str("Role.АдминистраторСистемы".into()),
            PropertyValue::Bool(true),
        ]),
    ]);
    let mut per = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let body = form_with_user_visible(uv.clone());
        let bytes = write_form(dialect, &body).expect("write");
        let back = read_form(dialect, &bytes).expect("read");
        assert_eq!(
            back.items[0].get(ld::F_USER_VISIBLE),
            Some(&uv),
            "{dialect:?}: common=true+roles value round-trip"
        );
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: byte-stable");
        per.push(back.items[0].get(ld::F_USER_VISIBLE).cloned());
    }
    assert_eq!(per[0], per[1], "cross-dialect canon (X)");
    let edt =
        String::from_utf8(write_form(FormDialect::Edt, &form_with_user_visible(uv.clone())).unwrap())
            .unwrap();
    assert!(
        edt.contains("<common>true</common>")
            && edt.contains("<role>Role.АдминистраторСистемы</role>"),
        "EDT: ведущий <common>true> + <for>-роль: {edt}"
    );
    let des = String::from_utf8(
        write_form(FormDialect::Designer, &form_with_user_visible(uv)).unwrap(),
    )
    .unwrap();
    assert!(
        des.contains("<xr:Common>true</xr:Common>")
            && des.contains("<xr:Value name=\"Role.АдминистраторСистемы\">true</xr:Value>"),
        "Designer: xr:Common=true + xr:Value роль: {des}"
    );
}

/// §1.0-ОТКАЗ: Designer `<UserVisible><xr:Common>true</xr:Common></UserVisible>` БЕЗ ролей
/// невитнессирован (Designer опускает `true` → absent-дефолт; present-true неотличим от absent) —
/// ридер обязан ГРОМКО упасть, а не тихо принять. Крафтим из валидного false-носителя.
#[test]
fn user_visible_designer_common_true_no_roles_is_typed_error() {
    let body = form_with_user_visible(PropertyValue::Bool(false));
    let bytes = write_form(FormDialect::Designer, &body).expect("write false");
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.matches("<xr:Common>false</xr:Common>").count() == 1,
        "ровно один <xr:Common> (userVisible) в минимальной форме"
    );
    let mutated = text.replace(
        "<xr:Common>false</xr:Common>",
        "<xr:Common>true</xr:Common>",
    );
    let err = read_form(FormDialect::Designer, mutated.as_bytes())
        .expect_err("common=true БЕЗ ролей должен упасть (§1.0)");
    let msg = format!("{err}");
    assert!(
        msg.contains("unwitnessed") || msg.contains("§1.0"),
        "типизированный §1.0-отказ, got: {msg}"
    );
}

/// КЛАСС 2: пункт формоуровневого КИ несёт РОЛЕВУЮ видимость (`<Visible>`/`<userVisible>` с
/// ролями) — witness ЗаказКлиента.ФормаСписка. Round-trip байт-стабилен и X-равен по диалектам.
#[test]
fn cmi_item_visible_roles_roundtrip_both_dialects() {
    use morph1c_core::ir::FormCiItem;
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.command_interface = true;
    body.form_ci_navigation_panel = vec![FormCiItem {
        command: "AccumulationRegister.ТоварыКОтгрузке.StandardCommand.OpenByRecorder".into(),
        ty: "Auto".into(),
        group: None,
        index: None,
        user_visible: Some(true),
        user_visible_roles: vec![("Role.ПолныеПрава".into(), true)],
    }];
    let mut per = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        assert_eq!(
            back.form_ci_navigation_panel, body.form_ci_navigation_panel,
            "{dialect:?}: cmi item visible-roles round-trip"
        );
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: cmi write∘read byte-stable");
        per.push(back.form_ci_navigation_panel.clone());
    }
    assert_eq!(per[0], per[1], "cross-dialect cmi item roles canon (X)");
    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    assert!(
        edt.contains("<common>true</common>") && edt.contains("<role>Role.ПолныеПрава</role>"),
        "EDT cmi: <common>true> + <for>-роль: {edt}"
    );
    let des = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    assert!(
        des.contains("<DefaultVisible>false</DefaultVisible>")
            && des.contains("<xr:Common>true</xr:Common>")
            && des.contains("<xr:Value name=\"Role.ПолныеПрава\">true</xr:Value>"),
        "Designer cmi: DefaultVisible + <Visible> с xr:Common=true + xr:Value: {des}"
    );
}

// ===================== KLASS 2: named-addition absent autoMaxWidth (ERP) =====================

/// Именованное добавление Таблицы (SearchString/…) БЕЗ `<autoMaxWidth>` читается (absent ⟺ false,
/// OppositeBool) — ERP-witness 56 форм; а на записи ПУСТОЙ extInfo эмитится САМОЗАКРЫТО
/// `<extInfo …/>` (65 SELFCLOSE-носителей). Present-`true` по-прежнему кладёт Bool(true).
#[test]
fn edt_named_addition_optional_auto_max_width() {
    use morph1c_core::spec::forms::controls::table as tbl;
    use super::tables as t;
    let ak = t::addition_kind("SearchStringAddition").unwrap();

    // 1) extInfo без autoMaxWidth (самозакрытый) — читается, bag БЕЗ auto_max_width.
    let empty_xml = concat!(
        "<searchStringAddition>\n",
        "  <name>Поиск</name>\n",
        "  <id>10</id>\n",
        "  <source>Список</source>\n",
        "  <extInfo xsi:type=\"form:SearchStringAdditionExtInfo\"/>\n",
        "</searchStringAddition>\n",
    );
    let d = crate::parse(empty_xml.as_bytes()).expect("parse empty-ext addition");
    let item =
        super::read::read_edt_addition(&d.root, ak).expect("addition without autoMaxWidth reads");
    assert!(
        item.get_ext(tbl::F_ADDITION_AUTO_MAX_WIDTH).is_none(),
        "absent autoMaxWidth ⇒ bag lacks it (false)"
    );
    // На записи extInfo пуст ⇒ самозакрыт.
    let out = super::write::edt_addition(&item).expect("write addition");
    let ext = out
        .children
        .iter()
        .find(|c| c.local == "extInfo")
        .expect("extInfo present");
    assert!(
        ext.self_closing && ext.children.is_empty(),
        "пустой extInfo эмитится самозакрытым <extInfo/>"
    );

    // 2) present-true — по-прежнему Bool(true), extInfo НЕ самозакрыт.
    let true_xml = concat!(
        "<searchStringAddition>\n",
        "  <name>Поиск</name>\n",
        "  <id>11</id>\n",
        "  <source>Список</source>\n",
        "  <extInfo xsi:type=\"form:SearchStringAdditionExtInfo\">\n",
        "    <autoMaxWidth>true</autoMaxWidth>\n",
        "  </extInfo>\n",
        "</searchStringAddition>\n",
    );
    let d2 = crate::parse(true_xml.as_bytes()).expect("parse true-ext addition");
    let item2 =
        super::read::read_edt_addition(&d2.root, ak).expect("addition with autoMaxWidth reads");
    assert_eq!(
        item2.get_ext(tbl::F_ADDITION_AUTO_MAX_WIDTH),
        Some(&PropertyValue::Bool(true)),
        "present autoMaxWidth ⇒ Bool(true)"
    );
    let out2 = super::write::edt_addition(&item2).expect("write addition2");
    let ext2 = out2
        .children
        .iter()
        .find(|c| c.local == "extInfo")
        .expect("extInfo2 present");
    assert!(
        !ext2.self_closing && ext2.children.iter().any(|c| c.local == "autoMaxWidth"),
        "extInfo с autoMaxWidth НЕ самозакрыт"
    );

    // 3) §1.0: present-false не витнессирован ⇒ отказ.
    let false_xml = concat!(
        "<searchStringAddition>\n",
        "  <name>Поиск</name>\n",
        "  <id>12</id>\n",
        "  <source>Список</source>\n",
        "  <extInfo xsi:type=\"form:SearchStringAdditionExtInfo\">\n",
        "    <autoMaxWidth>false</autoMaxWidth>\n",
        "  </extInfo>\n",
        "</searchStringAddition>\n",
    );
    let d3 = crate::parse(false_xml.as_bytes()).expect("parse false-ext addition");
    let err = super::read::read_edt_addition(&d3.root, ak)
        .expect_err("present <autoMaxWidth>false must error (§1.0)");
    assert!(
        format!("{err}").contains("autoMaxWidth"),
        "error names autoMaxWidth: {err}"
    );
}

// ===================== GanttChartField / PDFDocumentField (ERP census-батч) =====================

use morph1c_core::ir::FormEvent;
use morph1c_core::spec::forms::controls::form_field as ff;
use morph1c_core::spec::forms::controls::table as tb;

/// Геометрический extInfo диаграммы/PDF (ChartField-родня): все 6 полей в каноне.
fn geom_ext() -> Vec<(morph1c_core::ir::FieldId, PropertyValue)> {
    vec![
        (ff::F_EXT_WIDTH, PropertyValue::Int(50)),
        (ff::F_EXT_AUTO_MAX_WIDTH, PropertyValue::Bool(true)),
        (ff::F_EXT_HEIGHT, PropertyValue::Int(10)),
        (ff::F_EXT_AUTO_MAX_HEIGHT, PropertyValue::Bool(true)),
        (ff::F_EXT_HORIZONTAL_STRETCH, PropertyValue::Bool(true)),
        (ff::F_EXT_VERTICAL_STRETCH, PropertyValue::Bool(true)),
    ]
}

/// GanttChartField с авто-таблицей (`item.auto_table`) + событиями OnChange (тело) и
/// DetailProcessing (extInfo).
fn gantt_field() -> FormItem {
    let mut it = FormItem::new(FormControlKind::new("GanttChartField"), "Диаграмма", 99);
    it.properties = vec![(ff::F_DATA_PATH, PropertyValue::Ref("Диаграмма".into()))];
    it.ext_info = geom_ext();
    it.events = vec![
        FormEvent {
            name: "OnChange".into(),
            handler: "ДиаграммаПриИзменении".into(),
        },
        FormEvent {
            name: "DetailProcessing".into(),
            handler: "ДиаграммаОбработкаРасшифровки".into(),
        },
    ];
    // Авто-таблица: минимальная Table (dataPath + одно не-дефолтное поле для содержательности).
    let mut at = FormItem::new(FormControlKind::new("Table"), "Table", 247);
    at.properties = vec![(tb::F_DATA_PATH, PropertyValue::Ref("Диаграмма".into()))];
    it.auto_table = Some(Box::new(at));
    it
}

/// PDFDocumentField с viewStatusAddition (`item.additions`) + scale/currentPageNumber.
fn pdf_field() -> FormItem {
    let mut it = FormItem::new(FormControlKind::new("PDFDocumentField"), "ПолеДокумента", 1);
    it.properties = vec![(ff::F_DATA_PATH, PropertyValue::Ref("ПолеДокумента".into()))];
    it.ext_info = {
        let mut e = geom_ext();
        e.push((ff::F_EXT_SCALE, PropertyValue::Int(100)));
        e.push((ff::F_EXT_CURRENT_PAGE_NUMBER, PropertyValue::Int(1)));
        e
    };
    // Единственное добавление — viewStatusAddition (source + пустой ext autoMaxWidth в каноне).
    let mut add = FormItem::new(
        FormControlKind::new("ViewStatusAddition"),
        "ПолеДокументаСостояниеПросмотра",
        4,
    );
    add.properties = vec![(tb::F_ADDITION_SOURCE, PropertyValue::Ref("ПолеДокумента".into()))];
    add.ext_info = vec![(tb::F_ADDITION_AUTO_MAX_WIDTH, PropertyValue::Bool(true))];
    it.additions = vec![add];
    it
}

fn form_with(item: FormItem) -> FormBody {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.items = vec![item];
    body
}

/// Byte-exact round-trip НА КАНОНЕ (`write∘read∘write` идемпотентен по байтам) + канон-стабильность
/// (`read∘write` идемпотентен по IR), для обоих контролов в ОБОИХ диалектах.
#[test]
fn gantt_pdf_roundtrip_byte_exact_both_dialects() {
    for build in [gantt_field as fn() -> FormItem, pdf_field] {
        let body = form_with(build());
        for dialect in [FormDialect::Edt, FormDialect::Designer] {
            // Канон = read∘write исходного IR.
            let canon_bytes = write_form(dialect, &body)
                .and_then(|b| read_form(dialect, &b))
                .and_then(|ir| write_form(dialect, &ir))
                .unwrap_or_else(|e| panic!("{dialect:?} canonicalize: {e}"));
            // Byte-exact round-trip: перечитать канон-байты и переписать — идентично.
            let ir = read_form(dialect, &canon_bytes)
                .unwrap_or_else(|e| panic!("{dialect:?} read canon: {e}"));
            let again = write_form(dialect, &ir).unwrap();
            assert_eq!(
                canon_bytes, again,
                "{dialect:?}: write∘read byte-exact on canon"
            );
            // Канон-стабильность: повторный read∘write даёт РАВНЫЙ IR.
            let ir2 = read_form(dialect, &again).unwrap();
            assert_eq!(ir, ir2, "{dialect:?}: read∘write IR-idempotent");
        }
    }
}

/// Структурные инварианты чтения (оба диалекта видят одну и ту же структуру узла).
#[test]
fn gantt_pdf_structural_both_dialects() {
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        // GanttChartField: auto_table присутствует с верной идентичностью, оба события несутся.
        let g = read_form(dialect, &write_form(dialect, &form_with(gantt_field())).unwrap()).unwrap();
        let gi = &g.items[0];
        assert_eq!(gi.kind.as_str(), "GanttChartField");
        let at = gi.auto_table.as_ref().expect("gantt carries auto_table");
        assert_eq!((at.kind.as_str(), at.name.as_str(), at.id), ("Table", "Table", 247));
        let ev: std::collections::BTreeSet<&str> =
            gi.events.iter().map(|e| e.name.as_str()).collect();
        assert!(
            ev.contains("OnChange") && ev.contains("DetailProcessing"),
            "{dialect:?}: gantt events {ev:?}"
        );

        // PDFDocumentField: одно добавление viewStatusAddition + scale/currentPageNumber в каноне.
        let p = read_form(dialect, &write_form(dialect, &form_with(pdf_field())).unwrap()).unwrap();
        let pi = &p.items[0];
        assert_eq!(pi.kind.as_str(), "PDFDocumentField");
        assert_eq!(pi.additions.len(), 1, "{dialect:?}: pdf one addition");
        assert_eq!(pi.additions[0].kind.as_str(), "ViewStatusAddition");
        assert_eq!(
            pi.get_ext(ff::F_EXT_SCALE),
            Some(&PropertyValue::Int(100)),
            "{dialect:?}: pdf scale in canon"
        );
        assert_eq!(
            pi.get_ext(ff::F_EXT_CURRENT_PAGE_NUMBER),
            Some(&PropertyValue::Int(1)),
            "{dialect:?}: pdf currentPageNumber in canon"
        );
    }
}

// X-канон (edt=designer после normalize_form_for_x) — в `testkit` (там живёт нормализатор):
// см. `testkit/tests/forms_pilot.rs::gantt_pdf_x_canon_synthetic`.

/// §1.0: невитнесснутое extInfo-поле на новом контроле — громкий отказ (читатель не глотает
/// незаявленный узел). Проверяем на designer-фрагменте GanttChartField с чужим `<BackColor>`.
#[test]
fn gantt_designer_refuses_unwitnessed_ext_field() {
    let fk = super::tables::field_kind("GanttChartField").expect("registered");
    let xml = concat!(
        "<GanttChartField name=\"Д\" id=\"9\">\n",
        "  <DataPath>Д</DataPath>\n",
        "  <НезаявленныйУзел>x</НезаявленныйУзел>\n",
        "</GanttChartField>\n",
    );
    let d = crate::parse(xml.as_bytes()).expect("parse gantt fragment");
    let err = super::read::read_designer_field(&d.root, fk)
        .expect_err("unwitnessed child must error (§1.0)");
    assert!(
        format!("{err}").contains("unexpected child") || format!("{err}").contains("unconsumed"),
        "error flags the stray node: {err}"
    );
}

// ===================== KLASS: «непотреблённые узлы» (ERP, оба диалекта) =====================
// keyField (ПОВТОРЯЕМЫЙ) / SettingsComposer userSettingsGroup↔CustomSettingsFolder /
// DisplayImportance (доб.+подсказка) / AdditionSource-опциональный.

/// Порядок local-имён детей OutElement — для проверки byte-порядка эмиссии.
fn child_names(el: &crate::emit::OutElement) -> Vec<String> {
    el.children.iter().map(|c| c.local.clone()).collect()
}

/// ПОД-КЛАСС 1 — DynamicList `<keyField>` ПОВТОРЯЕМЫЙ. EDT-чтение собирает ВСЕ ключи (каждый
/// claim'ится ⇒ класс `keyField`-падений закрыт); обе записи ставят keyField ПОСЛЕ
/// fields/parameters (byte-порядок ERP: СписокДокументов/ЗастрахованныеЛицаСЭДО).
#[test]
fn dynamic_list_repeated_key_fields_read_and_order() {
    let edt_xml = concat!(
        "<extInfo xsi:type=\"form:DynamicListExtInfo\">\n",
        "  <customQuery>true</customQuery>\n",
        "  <queryText>ВЫБРАТЬ 1</queryText>\n",
        "  <fields xsi:type=\"schema:DataCompositionSchemaDataSetField\">\n",
        "    <dataPath>Поле</dataPath>\n",
        "    <field>Поле</field>\n",
        "  </fields>\n",
        "  <keyField>Регистратор</keyField>\n",
        "  <keyField>Организация</keyField>\n",
        "  <keyField>ТипЗапасов</keyField>\n",
        "</extInfo>\n",
    );
    let d = crate::parse(edt_xml.as_bytes()).expect("parse edt dl extInfo");
    let dl = super::read::read_edt_dynamic_list_attr(&d.root).expect("read edt dl");
    assert_eq!(
        dl.key_fields,
        vec![
            "Регистратор".to_string(),
            "Организация".to_string(),
            "ТипЗапасов".to_string()
        ],
        "все keyField собраны В ПОРЯДКЕ"
    );
    // Каждый <keyField> ЗАКЛЕЙМЁН (иначе 2-й/3-й — несконсуменный узел = класс падений).
    for kf in d.root.children.iter().filter(|c| c.local == "keyField") {
        assert!(
            kf.claimed.get() && kf.text_claimed.get(),
            "keyField {:?} claim'ится",
            kf.text
        );
    }
    // EDT-запись: keyField ПОСЛЕ fields, все 3.
    let eo = super::write::edt_dynamic_list_attr(&dl).expect("write edt dl");
    let en = child_names(&eo);
    let ef = en.iter().position(|n| n == "fields").expect("fields эмитятся");
    let ek = en.iter().position(|n| n == "keyField").expect("keyField эмитятся");
    assert!(ek > ef, "EDT keyField ПОСЛЕ fields: {en:?}");
    assert_eq!(
        eo.children.iter().filter(|c| c.local == "keyField").count(),
        3,
        "все 3 keyField эмитятся"
    );
    // Designer-запись ТОГО ЖЕ IR: KeyField ПОСЛЕ Field, все 3.
    let dof = super::write::designer_dynamic_list_attr(&dl).expect("write designer dl");
    let dn = child_names(&dof);
    let df = dn.iter().position(|n| n == "Field").expect("Field эмитятся");
    let dk = dn.iter().position(|n| n == "KeyField").expect("KeyField эмитятся");
    assert!(dk > df, "Designer KeyField ПОСЛЕ Field: {dn:?}");
    assert_eq!(
        dof.children.iter().filter(|c| c.local == "KeyField").count(),
        3
    );
}

/// ПОД-КЛАСС 1 (Designer-чтение): МНОЖЕСТВЕННЫЙ `<KeyField>` внутри `<Settings>` собирается ВЕСЬ.
#[test]
fn designer_dynamic_list_repeated_key_fields_read() {
    let xml = concat!(
        "<Settings xsi:type=\"DynamicList\">\n",
        "  <ManualQuery>true</ManualQuery>\n",
        "  <DynamicDataRead>true</DynamicDataRead>\n",
        "  <QueryText>ВЫБРАТЬ 1</QueryText>\n",
        "  <KeyField>Контрагент</KeyField>\n",
        "  <KeyField>ПоставленВручную</KeyField>\n",
        "</Settings>\n",
    );
    let d = crate::parse(xml.as_bytes()).expect("parse designer settings");
    let dl = super::read::read_designer_dynamic_list_attr(&d.root).expect("read designer dl");
    assert_eq!(
        dl.key_fields,
        vec!["Контрагент".to_string(), "ПоставленВручную".to_string()]
    );
    for kf in d.root.children.iter().filter(|c| c.local == "KeyField") {
        assert!(
            kf.claimed.get() && kf.text_claimed.get(),
            "KeyField claim'ится"
        );
    }
}

/// ПОД-КЛАСС 2 — форма КОМПОНОВЩИКА НАСТРОЕК: EDT `form:SettingsComposerFormExtInfo`>
/// `<userSettingsGroup>` читается (не задваивается с отчётной) и пишется ПОСЛЕ `<handlers>`.
#[test]
fn settings_composer_root_ext_info_edt() {
    let xml = concat!(
        "<extInfo xsi:type=\"form:SettingsComposerFormExtInfo\">\n",
        "  <handlers>\n",
        "    <event>OnUpdateUserSettingSetAtServer</event>\n",
        "    <name>ПриОбновлении</name>\n",
        "  </handlers>\n",
        "  <userSettingsGroup>ГруппаПользовательскихНастроек</userSettingsGroup>\n",
        "</extInfo>\n",
    );
    let d = crate::parse(xml.as_bytes()).expect("parse settings-composer extInfo");
    let (rx, report, _uf, _df, _gl) =
        super::read::read_edt_root_ext_info(&d.root).expect("read root extInfo");
    assert!(report.is_none(), "не отчётная форма");
    assert_eq!(
        rx.user_settings_group.as_deref(),
        Some("ГруппаПользовательскихНастроек")
    );
    assert_eq!(
        d.root.unclaimed_count(),
        0,
        "все узлы SettingsComposer extInfo claimed (handlers + userSettingsGroup)"
    );
    // Запись: handlers, ПОТОМ userSettingsGroup.
    let out = super::write::edt_root_ext_info(&rx, None, None, None, None);
    assert_eq!(
        child_names(&out),
        vec!["handlers".to_string(), "userSettingsGroup".to_string()],
        "порядок EDT extInfo: handlers → userSettingsGroup"
    );
}

/// ПОД-КЛАСС 2 (Designer-запись): форма с `root_ext_info.user_settings_group` эмитит корневой
/// `<CustomSettingsFolder>`.
#[test]
fn settings_composer_custom_settings_folder_designer_write() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.root_ext_info = Some(morph1c_core::ir::FormRootExtInfo {
        kind: "form:SettingsComposerFormExtInfo".into(),
        events: vec![],
        user_settings_group: Some("ГруппаПользовательскихНастроек".into()),
    });
    let bytes = write_form(FormDialect::Designer, &body).expect("designer write");
    let text = String::from_utf8(bytes).unwrap();
    assert!(
        text.contains("<CustomSettingsFolder>ГруппаПользовательскихНастроек</CustomSettingsFolder>"),
        "designer эмитит CustomSettingsFolder: {text}"
    );
}

/// ПОД-КЛАСС 3 — DisplayImportance добавления: EDT `<displayImportance>` (ЧИЛД после id) ⟺
/// Designer атрибут `DisplayImportance`. Читается ОБА диалекта (узел claim'ится), пишется ОБА.
#[test]
fn addition_display_importance_both_dialects() {
    let ak = super::tables::addition_kind("SearchStringAddition").unwrap();
    // EDT: <displayImportance> после id.
    let edt_xml = concat!(
        "<searchStringAddition>\n",
        "  <name>Дерево</name>\n",
        "  <id>5</id>\n",
        "  <displayImportance>VeryHigh</displayImportance>\n",
        "  <source>Список</source>\n",
        "  <extInfo xsi:type=\"form:SearchStringAdditionExtInfo\"/>\n",
        "</searchStringAddition>\n",
    );
    let d = crate::parse(edt_xml.as_bytes()).expect("parse edt addition");
    let item = super::read::read_edt_addition(&d.root, ak).expect("read edt addition");
    assert_eq!(
        item.get(super::tables::F_ADDITION_DISPLAY_IMPORTANCE),
        Some(&PropertyValue::Enum(Token::new("VeryHigh"))),
        "displayImportance прочитан"
    );
    assert_eq!(
        d.root.unclaimed_count(),
        0,
        "все узлы addition claimed (displayImportance больше не висит)"
    );
    // EDT-запись: <displayImportance> есть.
    let eo = super::write::edt_addition(&item).expect("write edt addition");
    assert!(
        eo.children
            .iter()
            .any(|c| c.local == "displayImportance" && c.text.as_deref() == Some("VeryHigh")),
        "EDT эмитит <displayImportance>"
    );
    // Designer-запись: атрибут DisplayImportance.
    let dof = super::write::designer_addition(&item).expect("write designer addition");
    assert!(
        dof.attrs
            .iter()
            .any(|(n, v)| n == "DisplayImportance" && v == "VeryHigh"),
        "Designer эмитит атрибут DisplayImportance"
    );
    // Designer-чтение round-trips.
    let des_xml = concat!(
        "<SearchStringAddition name=\"Дерево\" id=\"5\" DisplayImportance=\"VeryHigh\">\n",
        "  <AdditionSource>\n",
        "    <Item>Список</Item>\n",
        "    <Type>SearchStringRepresentation</Type>\n",
        "  </AdditionSource>\n",
        "</SearchStringAddition>\n",
    );
    let dd = crate::parse(des_xml.as_bytes()).expect("parse designer addition");
    let ditem = super::read::read_designer_addition(&dd.root, ak).expect("read designer addition");
    assert_eq!(
        ditem.get(super::tables::F_ADDITION_DISPLAY_IMPORTANCE),
        Some(&PropertyValue::Enum(Token::new("VeryHigh")))
    );
    assert_eq!(
        dd.root.unclaimed_count(),
        0,
        "designer addition: DisplayImportance-атрибут claim'ится"
    );
}

/// ПОД-КЛАСС 3 — DisplayImportance расширенной ПОДСКАЗКИ: EDT `<displayImportance>` (ЧИЛД
/// подсказки) ⟺ Designer атрибут `DisplayImportance` элемента `<ExtendedTooltip>`. Проверяем
/// через доб.-носитель подсказки.
#[test]
fn ext_tooltip_display_importance_both_dialects() {
    use morph1c_core::ir::form::DecoratorBody;
    let ak = super::tables::addition_kind("SearchStringAddition").unwrap();
    let edt_xml = concat!(
        "<searchStringAddition>\n",
        "  <name>Доб</name>\n",
        "  <id>5</id>\n",
        "  <extendedTooltip>\n",
        "    <name>ДобПодсказка</name>\n",
        "    <id>7</id>\n",
        "    <displayImportance>High</displayImportance>\n",
        "    <type>Label</type>\n",
        "    <extInfo xsi:type=\"form:LabelDecorationExtInfo\">\n",
        "      <horizontalAlign>Left</horizontalAlign>\n",
        "    </extInfo>\n",
        "  </extendedTooltip>\n",
        "  <source>Список</source>\n",
        "  <extInfo xsi:type=\"form:SearchStringAdditionExtInfo\"/>\n",
        "</searchStringAddition>\n",
    );
    let d = crate::parse(edt_xml.as_bytes()).expect("parse edt addition+tooltip");
    let item = super::read::read_edt_addition(&d.root, ak).expect("read edt addition");
    let tip = item.ext_tooltip.as_ref().expect("ext_tooltip present");
    let DecoratorBody::Tooltip(tb) = &tip.body else {
        panic!("tooltip body")
    };
    assert_eq!(tb.display_importance.as_deref(), Some("High"));
    assert_eq!(
        d.root.unclaimed_count(),
        0,
        "tooltip displayImportance claim'ится"
    );
    // EDT-запись: <displayImportance> внутри extendedTooltip.
    let eo = super::write::edt_addition(&item).expect("write edt addition");
    let tt = eo
        .children
        .iter()
        .find(|c| c.local == "extendedTooltip")
        .expect("extendedTooltip emitted");
    assert!(
        tt.children
            .iter()
            .any(|c| c.local == "displayImportance" && c.text.as_deref() == Some("High")),
        "EDT tooltip эмитит <displayImportance>"
    );
    // Designer-запись: атрибут DisplayImportance на <ExtendedTooltip> (bare-ref).
    let dof = super::write::designer_addition(&item).expect("write designer addition");
    let dtt = dof
        .children
        .iter()
        .find(|c| c.local == "ExtendedTooltip")
        .expect("ExtendedTooltip emitted");
    assert!(
        dtt.attrs
            .iter()
            .any(|(n, v)| n == "DisplayImportance" && v == "High"),
        "Designer tooltip эмитит атрибут DisplayImportance"
    );
}

/// ПОД-КЛАСС 3 — `<AdditionSource>` ОПЦИОНАЛЕН: добавление без источника читается (source=None)
/// на ОБОИХ диалектах и на записи источник опускается (Designer — без `<AdditionSource>`).
#[test]
fn addition_without_source_optional_both_dialects() {
    let ak = super::tables::addition_kind("SearchStringAddition").unwrap();
    // EDT без <source>.
    let edt_xml = concat!(
        "<searchStringAddition>\n",
        "  <name>Доб</name>\n",
        "  <id>5</id>\n",
        "  <extInfo xsi:type=\"form:SearchStringAdditionExtInfo\"/>\n",
        "</searchStringAddition>\n",
    );
    let d = crate::parse(edt_xml.as_bytes()).expect("parse edt sourceless addition");
    let item = super::read::read_edt_addition(&d.root, ak).expect("read edt sourceless addition");
    assert!(
        item.get(tb::F_ADDITION_SOURCE).is_none(),
        "нет source ⇒ свойство отсутствует"
    );
    assert_eq!(d.root.unclaimed_count(), 0, "все узлы claimed без source");
    let eo = super::write::edt_addition(&item).expect("write edt addition");
    assert!(
        !eo.children.iter().any(|c| c.local == "source"),
        "EDT не эмитит <source> без источника"
    );
    // Designer без <AdditionSource>.
    let des_xml = concat!(
        "<SearchStringAddition name=\"Доб\" id=\"5\">\n",
        "  <ExtendedTooltip name=\"П\" id=\"7\"/>\n",
        "</SearchStringAddition>\n",
    );
    let dd = crate::parse(des_xml.as_bytes()).expect("parse designer sourceless addition");
    let ditem =
        super::read::read_designer_addition(&dd.root, ak).expect("read designer sourceless addition");
    assert!(
        ditem.get(tb::F_ADDITION_SOURCE).is_none(),
        "Designer без AdditionSource ⇒ source отсутствует"
    );
    assert_eq!(
        dd.root.unclaimed_count(),
        0,
        "designer: нет несконсуменных узлов без AdditionSource"
    );
    let dof = super::write::designer_addition(&ditem).expect("write designer addition");
    assert!(
        !dof.children.iter().any(|c| c.local == "AdditionSource"),
        "Designer не эмитит <AdditionSource> без источника"
    );
}

// ============================================================================
// ХВОСТ мелких форм-классов ERP (SC1..SC9) — round-trip оба диалекта + отказы.
// ============================================================================

/// SC1: реквизит с `id==0` — EDT ОПУСКАЕТ `<id>`, Designer эмитит `id="0"`. Канон id=0 общий;
/// round-trip оба диалекта, X-равно. ERP-witness Catalog.ДоговорыКонтрагентов.ФормаЭлемента /
/// Report.мирМониторингЗаказа.ФормаОтчета.
#[test]
fn sc1_zero_id_attribute_edt_omits_designer_emits() {
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.data_attributes = vec![
        bool_data_attr("Обычный", 5, true, vec![], true, vec![]),
        bool_data_attr("НулевойId", 0, true, vec![], true, vec![]),
    ];
    let mut per = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        let ids: Vec<i64> = back.data_attributes.iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![5, 0], "{dialect:?}: id=0 сохранён");
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: write-read byte-stable");
        per.push(back.data_attributes.clone());
    }
    assert_eq!(per[0], per[1], "SC1: edt equiv designer (X)");
    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    assert!(edt.contains("<id>5</id>"), "EDT эмитит ненулевой id");
    assert_eq!(edt.matches("<id>0</id>").count(), 0, "EDT ОПУСКАЕТ id=0: {edt}");
    let des = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    assert!(des.contains("id=\"0\""), "Designer эмитит id=0: {des}");
}

/// SC2: Table-именованное добавление `searchControlAddition` несёт МУЛЬТИЯЗЫЧНЫЙ `<title>`.
/// Read даёт F_ADDITION_TITLE=Localized(ru+en), 0 неклеймнутых; write возвращает `<title>`
/// ПЕРВЫМ. ERP-witness DataProcessor.МобильноеРабочееМестоКладовщика.СписокПриходныхОрдеров.
#[test]
fn sc2_named_addition_multilang_title() {
    use morph1c_core::spec::forms::controls::table as tbl;
    let ak = super::tables::addition_kind("SearchControlAddition").unwrap();
    let xml = concat!(
        "<searchControlAddition>\n",
        "  <title>\n    <key>ru</key>\n    <value>Поиск</value>\n  </title>\n",
        "  <title>\n    <key>en</key>\n    <value>Search</value>\n  </title>\n",
        "  <name>УправлениеПоиском</name>\n",
        "  <id>49</id>\n",
        "  <type>SearchControlAddition</type>\n",
        "  <source>Список</source>\n",
        "  <extInfo xsi:type=\"form:SearchControlAdditionExtInfo\">\n",
        "    <autoMaxWidth>true</autoMaxWidth>\n",
        "  </extInfo>\n",
        "</searchControlAddition>\n",
    );
    let d = crate::parse(xml.as_bytes()).expect("parse addition with title");
    let item = super::read::read_edt_addition(&d.root, ak).expect("addition with title reads");
    assert_eq!(d.root.unclaimed_count(), 0, "SC2: все узлы claimed (title потреблён)");
    match item.get(tbl::F_ADDITION_TITLE) {
        Some(PropertyValue::Localized(pairs)) => {
            assert_eq!(pairs.len(), 2, "ru+en");
            assert_eq!(pairs[0].1, "Поиск");
            assert_eq!(pairs[1].1, "Search");
        }
        other => panic!("SC2: title не Localized: {other:?}"),
    }
    let out = super::write::edt_addition(&item).expect("write addition");
    assert_eq!(
        out.children.iter().filter(|c| c.local == "title").count(),
        2,
        "SC2: два <title> на выходе"
    );
    assert_eq!(out.children[0].local, "title", "SC2: <title> первым");
}

/// SC9: `searchStringAddition` несёт МУЛЬТИЯЗЫЧНЫЙ `<toolTip>` (ru+en). Ранее читался ЛИШЬ
/// первый ⇒ 5 неклеймнутых узлов второго. Теперь оба потреблены. ERP-witness
/// InformationRegister.НастройкиИсключенийПроверкиДокументов.
#[test]
fn sc9_named_addition_multilang_tooltip_all_consumed() {
    use super::tables as t;
    let ak = t::addition_kind("SearchStringAddition").unwrap();
    let xml = concat!(
        "<searchStringAddition>\n",
        "  <toolTip>\n    <key>ru</key>\n    <value>Поиск по значениям</value>\n  </toolTip>\n",
        "  <toolTip>\n    <key>en</key>\n    <value>Search by values</value>\n  </toolTip>\n",
        "  <toolTipRepresentation>Button</toolTipRepresentation>\n",
        "  <name>СтрокаПоиска</name>\n",
        "  <id>24</id>\n",
        "  <source>Дерево</source>\n",
        "  <extInfo xsi:type=\"form:SearchStringAdditionExtInfo\">\n",
        "    <autoMaxWidth>true</autoMaxWidth>\n",
        "  </extInfo>\n",
        "</searchStringAddition>\n",
    );
    let d = crate::parse(xml.as_bytes()).expect("parse addition with multilang tooltip");
    let item = super::read::read_edt_addition(&d.root, ak).expect("multilang tooltip reads");
    assert_eq!(d.root.unclaimed_count(), 0, "SC9: ОБА <toolTip> потреблены (было 5 неклеймнутых)");
    match item.get(t::F_ADDITION_TOOL_TIP) {
        Some(PropertyValue::Localized(pairs)) => assert_eq!(pairs.len(), 2, "ru+en toolTip"),
        other => panic!("SC9: toolTip не Localized(2): {other:?}"),
    }
    let out = super::write::edt_addition(&item).expect("write addition");
    assert_eq!(
        out.children.iter().filter(|c| c.local == "toolTip").count(),
        2,
        "SC9: два <toolTip> на выходе"
    );
}

/// SC4: formCommand `<use>` с ролями БЕЗ явного `<value>` — EDT опускает дефолт `false`,
/// Designer эмитит `<xr:Value>false`. Round-trip оба, X-равно. ERP-witness
/// Document.Отпуск.ФормаДокумента (ПодробнееОРасчетеНДФЛ).
#[test]
fn sc4_command_use_roles_value_false_edt_omits() {
    use super::tables;
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    let mut cmd = FormCommand::new("ПодробнееОРасчетеНДФЛ", 3);
    cmd.properties = vec![
        (fc::F_CURRENT_ROW_USE, PropertyValue::Enum(Token::new("Use"))),
        (fc::F_SELECTED_ROWS_USE, PropertyValue::Enum(Token::new("Use"))),
        (
            tables::F_CMD_USE,
            PropertyValue::List(vec![
                PropertyValue::Bool(false),
                PropertyValue::List(vec![
                    PropertyValue::Str("Role.R1".to_string()),
                    PropertyValue::Bool(false),
                ]),
                PropertyValue::List(vec![
                    PropertyValue::Str("Role.R2".to_string()),
                    PropertyValue::Bool(false),
                ]),
            ]),
        ),
    ];
    body.commands.push(cmd);
    let mut per = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: byte-stable");
        per.push(back.commands[0].get(tables::F_CMD_USE).cloned());
    }
    assert_eq!(per[0], per[1], "SC4: edt equiv designer (X)");
    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    assert!(edt.contains("<role>Role.R1</role>"), "EDT эмитит роли");
    assert_eq!(
        edt.matches("<value>false</value>").count(),
        0,
        "EDT ОПУСКАЕТ <value>false в <for>: {edt}"
    );
    let des = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    assert!(
        des.contains("<xr:Value name=\"Role.R1\">false</xr:Value>"),
        "Designer эмитит явный false: {des}"
    );
}

/// SC8: добавление-КАК-КОНТРОЛ (`<items xsi:type="form:Addition">`) несёт ВЛОЖЕННЫЙ `<items>`.
/// Round-trip оба диалекта. ERP-witness Document.ВходящийЗапросФССДляРасчетаПособия.ФормаСписка.
#[test]
fn sc8_addition_control_nested_items_roundtrip_both_dialects() {
    use morph1c_core::spec::forms::controls::label_decoration as ld;
    let build = || {
        let mut child =
            FormItem::new(FormControlKind::new("LabelDecoration"), "ВложеннаяНадпись", 20);
        child.properties = vec![
            (ld::F_ENABLED, PropertyValue::Bool(true)),
            (ld::F_USER_VISIBLE, PropertyValue::Bool(true)),
        ];
        let mut add = FormItem::new(
            FormControlKind::new("SearchControlAddition"),
            "УправлениеПоиском",
            21,
        );
        add.properties = vec![(tb::F_ADDITION_SOURCE, PropertyValue::Ref("Список".into()))];
        add.ext_info = vec![(tb::F_ADDITION_AUTO_MAX_WIDTH, PropertyValue::Bool(true))];
        add.children = vec![child];
        form_with(add)
    };
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let body = build();
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        assert_eq!(back.items.len(), 1, "{dialect:?}: одно добавление-контрол");
        assert_eq!(
            back.items[0].kind.as_str(),
            "SearchControlAddition",
            "{dialect:?}: вид сохранён"
        );
        assert_eq!(back.items[0].children.len(), 1, "{dialect:?}: вложенный контрол прочитан");
        assert_eq!(back.items[0].children[0].name, "ВложеннаяНадпись");
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: byte-stable");
    }
}

/// SC3: форма динамического списка несёт `groupList` — EDT ВНУТРИ `form:DynamicListFormExtInfo`,
/// Designer прямым ребёнком корня `<GroupList>`. F_GROUP_LIST round-trip'ится оба, X-равно.
/// ERP-witness Catalog.ВидыОтправляемыхДокументов.ФормаСписка.
#[test]
fn sc3_group_list_roundtrip_both_dialects() {
    use morph1c_core::ir::FormRootExtInfo;
    let mut body = FormBody::new();
    body.attributes = base_attrs();
    body.attributes.push((fr::F_GROUP_LIST, PropertyValue::Str("Дерево".into())));
    body.root_ext_info = Some(FormRootExtInfo {
        kind: "form:DynamicListFormExtInfo".into(),
        events: Vec::new(),
        user_settings_group: None,
    });
    let find_gl = |b: &FormBody| {
        b.attributes
            .iter()
            .find(|(k, _)| *k == fr::F_GROUP_LIST)
            .map(|(_, v)| v.clone())
    };
    let mut per = Vec::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        let bytes = write_form(dialect, &body).unwrap_or_else(|e| panic!("{dialect:?} write: {e}"));
        let back = read_form(dialect, &bytes).unwrap_or_else(|e| panic!("{dialect:?} read: {e}"));
        assert_eq!(
            find_gl(&back),
            Some(PropertyValue::Str("Дерево".into())),
            "{dialect:?}: groupList round-trips"
        );
        let bytes2 = write_form(dialect, &back).unwrap();
        assert_eq!(bytes, bytes2, "{dialect:?}: byte-stable");
        per.push(find_gl(&back));
    }
    assert_eq!(per[0], per[1], "SC3: edt equiv designer (X)");
    let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
    assert!(
        edt.contains("form:DynamicListFormExtInfo") && edt.contains("<groupList>Дерево</groupList>"),
        "EDT несёт groupList ВНУТРИ extInfo: {edt}"
    );
    let des = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
    assert!(
        des.contains("<GroupList>Дерево</GroupList>"),
        "Designer несёт GroupList прямым ребёнком: {des}"
    );
}

/// АЛИАС ИМЕНИ ЕНУМА `childrenAlign`: EDT `ItemsAutoTitlesLeft` ⟺ Designer `TitlesLeftDataAuto`.
///
/// Ценз (покарьерный join ПОЛНОГО корпуса ERP 8.3.27 — 11 400 форм ⟂ 11 400 форм): у тега
/// `childrenAlign` 63 носителя в КАЖДОМ диалекте, из них РОВНО 3 расходятся именем значения
/// (`CommonForms/НастройкаПодтвержденияКриптоопераций` UsualGroup id=151,
/// `CommonForms/ОтключениеПодтвержденияКриптоопераций` UsualGroup id=107,
/// `InformationRegisters/УчетныеЗаписиЭДО/Forms/ПомощникПодключенияЭДО` Page id=905), остальные
/// 60 дословно равны. SSL 8.5.1 (876 форм) и coverage носителей шестого литерала не имеют.
/// Канон IR = EDT-написание (та же конвенция, что у `heightControlVariant`/`verticalScroll`).
#[test]
fn children_align_enum_alias_edt_vs_designer() {
    use morph1c_core::spec::forms::controls::form_group as fg;
    let build = |kind: &str, name: &str, id: i64, lit: &str| {
        let mut g = FormItem::new(FormControlKind::new(kind), name, id);
        g.properties = vec![
            (fg::F_ENABLED, PropertyValue::Bool(true)),
            (fg::F_USER_VISIBLE, PropertyValue::Bool(true)),
        ];
        g.ext_info = vec![(
            fg::F_EXT_CHILDREN_ALIGN,
            PropertyValue::Enum(Token::new(lit)),
        )];
        form_with(g)
    };
    for (kind, name, id) in [
        ("UsualGroup", "ГруппаОсновное", 151),
        ("Page", "СтраницаЗапросаСведений", 905),
    ] {
        // Шестой литерал: диалекты пишут РАЗНЫЕ имена ОДНОГО канона.
        let body = build(kind, name, id, "ItemsAutoTitlesLeft");
        let edt = String::from_utf8(write_form(FormDialect::Edt, &body).unwrap()).unwrap();
        let des = String::from_utf8(write_form(FormDialect::Designer, &body).unwrap()).unwrap();
        assert!(
            edt.contains("<childrenAlign>ItemsAutoTitlesLeft</childrenAlign>"),
            "{kind}: EDT пишет канон дословно: {edt}"
        );
        assert!(
            des.contains("<ChildrenAlign>TitlesLeftDataAuto</ChildrenAlign>"),
            "{kind}: Designer пишет свой алиас: {des}"
        );
        // ОБА диалекта читаются в ОДИН канон (X-равенство).
        let mut per = Vec::new();
        for (dialect, bytes) in [
            (FormDialect::Edt, edt.as_bytes()),
            (FormDialect::Designer, des.as_bytes()),
        ] {
            let back = read_form(dialect, bytes)
                .unwrap_or_else(|e| panic!("{kind} {dialect:?} read: {e}"));
            per.push(
                back.items[0]
                    .ext_info
                    .iter()
                    .find(|(k, _)| *k == fg::F_EXT_CHILDREN_ALIGN)
                    .map(|(_, v)| v.clone()),
            );
            let again = write_form(dialect, &back).unwrap();
            assert_eq!(again, bytes, "{kind} {dialect:?}: byte-stable");
        }
        assert_eq!(
            per[0],
            Some(PropertyValue::Enum(Token::new("ItemsAutoTitlesLeft"))),
            "{kind}: EDT → канон"
        );
        assert_eq!(per[0], per[1], "{kind}: edt ≡ designer (X)");
        // Пять ОБЩИХ литералов диалекты пишут дословно одинаково.
        for lit in [
            "None",
            "ItemsLeftTitlesLeft",
            "ItemsRightTitlesLeft",
            "ItemsLeftTitlesRight",
            "ItemsRightTitlesRight",
        ] {
            let b = build(kind, name, id, lit);
            let e = String::from_utf8(write_form(FormDialect::Edt, &b).unwrap()).unwrap();
            let d = String::from_utf8(write_form(FormDialect::Designer, &b).unwrap()).unwrap();
            assert!(
                e.contains(&format!("<childrenAlign>{lit}</childrenAlign>"))
                    && d.contains(&format!("<ChildrenAlign>{lit}</ChildrenAlign>")),
                "{kind} {lit}: общий литерал дословен в обоих диалектах"
            );
        }
    }
    // §1.0: НЕвитнессированный литерал (напр. метамодельный `Auto`, которого в корпусе нет
    // ни в одном диалекте) — ТИПИЗИРОВАННЫЙ отказ, не догадка.
    let bad = build("UsualGroup", "Г", 1, "Auto");
    assert!(
        write_form(FormDialect::Designer, &bad).is_err(),
        "Designer: невитнессированный канон отказывает (§1.0)"
    );
}
