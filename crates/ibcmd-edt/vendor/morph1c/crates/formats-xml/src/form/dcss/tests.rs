use super::*;

/// Минимальный витнесс (SSL 1155 B — 171/227 сайдкаров ИМЕННО такие): три пустые группы
/// (filter/order/conditionalAppearance) с viewMode+userSettingID + items-мета.
fn boilerplate_bytes() -> Vec<u8> {
    let mut s = String::from(DCSS_DECL);
    s.push_str("\r\n<Settings");
    for (n, u) in DCSS_ROOT_NS {
        s.push_str(&format!(" {n}=\"{u}\""));
    }
    s.push_str(
        ">\r\n\
\t<filter>\r\n\t\t<viewMode>Normal</viewMode>\r\n\t\t<userSettingID>f-guid</userSettingID>\r\n\t</filter>\r\n\
\t<order>\r\n\t\t<item xsi:type=\"OrderItemField\">\r\n\t\t\t<field>Дата</field>\r\n\t\t\t<orderType>Asc</orderType>\r\n\t\t</item>\r\n\
\t\t<viewMode>Normal</viewMode>\r\n\t\t<userSettingID>o-guid</userSettingID>\r\n\t</order>\r\n\
\t<conditionalAppearance>\r\n\t\t<viewMode>Normal</viewMode>\r\n\t\t<userSettingID>c-guid</userSettingID>\r\n\t</conditionalAppearance>\r\n\
\t<itemsViewMode>Normal</itemsViewMode>\r\n\t<itemsUserSettingID>i-guid</itemsUserSettingID>\r\n</Settings>\r\n",
    );
    s.into_bytes()
}

#[test]
fn boilerplate_roundtrip_byte_exact() {
    let src = boilerplate_bytes();
    let ls = read_list_settings_dcss(&src).expect("read boilerplate dcss");
    assert!(!ls.is_empty(), "boilerplate carries filter/order/CA groups");
    let ord = ls.order.as_ref().expect("order group");
    assert_eq!(ord.items.len(), 1, "one OrderItemField");
    assert_eq!(ls.items_view_mode.as_deref(), Some("Normal"));
    assert_eq!(
        write_list_settings_dcss(&ls),
        src,
        "boilerplate dcss byte-exact"
    );
}

#[test]
fn unknown_child_is_loud() {
    // §1.0: незнакомый под-элемент — громкий отказ, не глотаем.
    let s = String::from_utf8(boilerplate_bytes()).unwrap().replace(
        "\t<itemsViewMode>",
        "\t<mystery>1</mystery>\r\n\t<itemsViewMode>",
    );
    let err = read_list_settings_dcss(s.as_bytes()).unwrap_err();
    assert!(err.to_string().contains("§1.0"), "got: {err}");
}

#[test]
fn wrong_root_is_loud() {
    let s = String::from_utf8(boilerplate_bytes())
        .unwrap()
        .replace("Settings", "Nastroiki");
    let err = read_list_settings_dcss(s.as_bytes()).unwrap_err();
    assert!(err.to_string().contains("dcss:"), "got: {err}");
}

/// Весь корпус SSL: КАЖДЫЙ из 227 сайдкаров читается типизированно и переписывается
/// byte-exact (§1.0 — тотальность + обратимость на РЕАЛЬНЫХ витнессах, не только на
/// синтетике). INFRA-SKIP, если корпуса нет.
#[test]
fn ssl_corpus_roundtrip_byte_exact() {
    let root = fixtures_root().join("SSL/edt/src");
    if !root.is_dir() {
        eprintln!("ssl_corpus_roundtrip_byte_exact INFRA-SKIP (corpus absent)");
        return;
    }
    let mut seen = 0usize;
    let mut bad = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n == "ListSettings.dcss") {
                let src = std::fs::read(&p).expect("read sidecar");
                seen += 1;
                match read_list_settings_dcss(&src) {
                    Ok(ls) => {
                        if write_list_settings_dcss(&ls) != src {
                            bad.push(format!("{}: NOT byte-exact on write", p.display()));
                        }
                    }
                    Err(e) => bad.push(format!("{}: read failed: {e}", p.display())),
                }
            }
        }
    }
    assert!(
        seen >= 227,
        "expected the 227 SSL ListSettings.dcss sidecars, saw {seen}"
    );
    assert!(
        bad.is_empty(),
        "{} sidecar(s) failed:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// СЕКЦИОННЫЙ локус, шрифты УО: ns ссылки `ref` объявляется ИНЛАЙН КАНОНИЧЕСКИМ префиксом,
/// config-`StyleItem` переписывается в `0:<uuid>` (style-ns остаётся объявленным), платформенный
/// `style:` и `sys:` — вербатим. Витнессы erp.cf r31: ПоказателиРасчетаЗарплаты/ФормаСписка
/// `b62913ec-…0` (config → `xmlns:style` + `ref="0:654523be-…"`); РабочиеМеста/ФормаСписка
/// `aeafdd56-…0` (`ref="style:TextFont"` вербатим + `xmlns:style`); ВидыБюджетов/ФормаСписка
/// `216df259-…0` (`ref="sys:DefaultGUIFont"` вербатим + `xmlns:sys`).
#[test]
fn section_font_ref_declares_its_ns_and_rewrites_config_style_items() {
    let mut s = String::from(DCSS_DECL);
    s.push_str("\r\n<Settings");
    for (n, u) in DCSS_ROOT_NS {
        s.push_str(&format!(" {n}=\"{u}\""));
    }
    s.push_str(concat!(
        ">\r\n\t<conditionalAppearance>\r\n\t\t<item>\r\n\t\t\t<selection/>\r\n",
        "\t\t\t<filter>\r\n\t\t\t\t<item xsi:type=\"FilterItemComparison\">\r\n",
        "\t\t\t\t\t<left xsi:type=\"dcscor:Field\">Поле</left>\r\n",
        "\t\t\t\t\t<comparisonType>Equal</comparisonType>\r\n",
        "\t\t\t\t\t<right xsi:type=\"xs:boolean\">false</right>\r\n",
        "\t\t\t\t</item>\r\n\t\t\t</filter>\r\n\t\t\t<appearance>\r\n",
        "\t\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n",
        "\t\t\t\t\t<dcscor:parameter>Шрифт</dcscor:parameter>\r\n",
        "\t\t\t\t\t<dcscor:value xsi:type=\"v8ui:Font\" ref=\"style:ШрифтКонфиг\" kind=\"StyleItem\"/>\r\n",
        "\t\t\t\t</dcscor:item>\r\n",
        "\t\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n",
        "\t\t\t\t\t<dcscor:parameter>ШрифтЗаголовка</dcscor:parameter>\r\n",
        "\t\t\t\t\t<dcscor:value xsi:type=\"v8ui:Font\" ref=\"style:TextFont\" bold=\"true\" italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"StyleItem\"/>\r\n",
        "\t\t\t\t</dcscor:item>\r\n",
        "\t\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n",
        "\t\t\t\t\t<dcscor:parameter>ШрифтПодвала</dcscor:parameter>\r\n",
        "\t\t\t\t\t<dcscor:value xsi:type=\"v8ui:Font\" ref=\"sys:DefaultGUIFont\" bold=\"false\" italic=\"false\" underline=\"false\" strikeout=\"true\" kind=\"WindowsFont\"/>\r\n",
        "\t\t\t\t</dcscor:item>\r\n",
        "\t\t\t</appearance>\r\n\t\t</item>\r\n",
        "\t\t<viewMode>Normal</viewMode>\r\n\t\t<userSettingID>c-guid</userSettingID>\r\n",
        "\t</conditionalAppearance>\r\n</Settings>\r\n",
    ));
    let ls = read_list_settings_dcss(s.as_bytes()).expect("read dcss");
    let resolver = |name: &str| {
        (name == "ШрифтКонфиг").then(|| "aaaaaaaa-bbbb-cccc-dddd-eeeeffff0002".to_string())
    };
    let xml = write_list_settings_section(DcsSettingsSection::Appearance, Some(&ls), &resolver)
        .expect("write Appearance section");
    let text = String::from_utf8(xml).expect("utf-8");
    assert!(
        text.contains(
            "<value xmlns:d5p1=\"http://v8.1c.ru/8.1/data/ui\" \
             xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xsi:type=\"d5p1:Font\" \
             ref=\"0:aaaaaaaa-bbbb-cccc-dddd-eeeeffff0002\" kind=\"StyleItem\"/>"
        ),
        "config font: xmlns:style declared canonically, ref respelled to 0:<uuid>:\n{text}"
    );
    assert!(
        text.contains(
            "<value xmlns:d5p1=\"http://v8.1c.ru/8.1/data/ui\" \
             xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xsi:type=\"d5p1:Font\" \
             ref=\"style:TextFont\" bold=\"true\""
        ),
        "platform font: xmlns:style declared, ref verbatim:\n{text}"
    );
    assert!(
        text.contains(
            "<value xmlns:d5p1=\"http://v8.1c.ru/8.1/data/ui\" \
             xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xsi:type=\"d5p1:Font\" \
             ref=\"sys:DefaultGUIFont\""
        ),
        "sys font: xmlns:sys declared, ref verbatim:\n{text}"
    );
}

/// Пустая схема ⇒ РОВНО платформенный «пустой документ» кэша `ServerState`.
///
/// Это ПИН, а не тавтология: те же 180 байт лежат base64'ом в `data_attrs::SERVER_STATE_EMPTY`
/// — константе, которую 187 SSL-списков несут дословно. Тест доказывает, что общий writer
/// воспроизводит её сам (декл · `xmlns=""` · порядок ns · TAB · CRLF · БЕЗ BOM · БЕЗ trailing
/// EOL · ДВА хвостовых пробела), поэтому «пустой» случай — не отдельная ветка и не может
/// разъехаться с «непустым».
#[test]
fn empty_server_state_is_the_platform_empty_document() {
    let out = write_server_state(&[], &[], &[], &|_| None).expect("write empty ServerState");
    let expected = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<UniversalListServerOnlyState xmlns=\"\" ",
        "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
        "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>  ",
    );
    assert_eq!(
        String::from_utf8(out.clone()).expect("utf-8"),
        expected,
        "the empty ServerState document must be byte-exact"
    );
    // Длина — независимая проверка: конверт оракула несёт `len-1 = 0xb3` = 179 ⇒ ровно 180 B.
    assert_eq!(out.len(), 180, "the empty document is 180 bytes");
}

/// §1.0: тип-параметр, чей QName опирается на префикс, не связанный в корне кэша, — ГРОМКО.
///
/// `CatalogRef.X` Designer-эмитится как `cfg:CatalogRef.X`, а `cfg` объявлен в корне ФОРМЫ, не
/// кэша: молча получился бы висячий префикс. Корпус витнессит лишь `xs:`-примитивы и `TypeId`.
#[test]
fn server_state_dangling_type_prefix_is_loud() {
    use morph1c_core::ir::{TypeRef, TypeSpec};
    let p = DcsParameter {
        name: "Пользователь".into(),
        title: None,
        value_type: Some(TypeSpec {
            parts: vec![TypeRef {
                id: "CatalogRef.Пользователи".into(),
                qualifier: None,
            }],
        }),
        value: None,
        use_restriction: None,
        value_list_allowed: false,
        available_as_field: None,
    };
    let err = write_server_state(&[], &[], &[p], &|_| None)
        .expect_err("a cfg:-QName type must refuse, not dangle");
    assert!(err.to_string().contains("§1.0"), "got: {err}");
}

/// Кэш `ServerState` вычисляемого-поля с `orderExpression` — прямой witness erp.cf
/// InformationRegister.ОперацииСПодключаемымОборудованием/ФормаСписка (`c97fc993-…73.0`):
/// (a) дети `orderExpression` (`expression`/`orderType`/`autoOrder`) КЭШ несёт ВЕРБАТИМ с
/// ИНЛАЙН дефолтным `xmlns="…/common"` (не роняются в пустой ns), (b) `[Field]` идёт ПЕРЕД
/// `[ExpressionField]` (оракул кладёт 22 `<Field>` затем один `<ExpressionField>`).
#[test]
fn server_state_order_expression_common_ns_and_field_before_calc() {
    use morph1c_core::ir::form::{DcsCalculatedField, DcsField, DcsOrderExpression};
    let field = DcsField {
        nested: false,
        data_path: "Ссылка".into(),
        field: "Ссылка".into(),
        presentation_expression: None,
        title: None,
        value_type: None,
        use_restriction: None,
        attribute_use_restriction: None,
        appearance: vec![],
        available_values: vec![],
    };
    let calc = DcsCalculatedField {
        data_path: "УниверсальнаяДата".into(),
        expression: "Дата".into(),
        title: None,
        use_restriction: None,
        presentation_expression: None,
        order_expressions: vec![
            DcsOrderExpression {
                expression: "Дата".into(),
                order_type: "Asc".into(),
                auto_order: false,
            },
            DcsOrderExpression {
                expression: "НомерОперации".into(),
                order_type: "Asc".into(),
                auto_order: false,
            },
        ],
        appearance: vec![],
        value_type: None,
    };
    let out = write_server_state(&[field], &[calc], &[], &|_| None)
        .expect("write ServerState with calc orderExpression");
    let s = String::from_utf8(out).expect("utf-8");
    const C: &str = "http://v8.1c.ru/8.1/data-composition-system/common";
    // (a) common-ns дети orderExpression — ВЕРБАТИМ с инлайн дефолтным xmlns (byte-exact оракул).
    assert!(
        s.contains(&format!("<expression xmlns=\"{C}\">Дата</expression>")),
        "orderExpression <expression> must keep inline common-ns:\n{s}"
    );
    assert!(
        s.contains(&format!("<orderType xmlns=\"{C}\">Asc</orderType>")),
        "orderExpression <orderType> must keep inline common-ns:\n{s}"
    );
    assert!(
        s.contains(&format!("<autoOrder xmlns=\"{C}\">false</autoOrder>")),
        "orderExpression <autoOrder> must keep inline common-ns:\n{s}"
    );
    // (b) Field ПЕРЕД ExpressionField.
    let fi = s.find("</Field>").expect("a <Field> must be emitted");
    let ei = s.find("<ExpressionField").expect("an <ExpressionField> must be emitted");
    assert!(fi < ei, "Field must precede ExpressionField (oracle order):\n{s}");
}

/// Кэш `ServerState` DCS-поля с `availableValues` — прямой witness erp.cf
/// Document.СчетНаОплатуКлиенту.ФормаСозданияСчетовНаОплату поле `Состояние`: обёртка ДОЛЖНА
/// быть `<dcssch:availableValue>` (ns dcssch, `xmlns:dcssch` уже в области видимости на `<Field>`),
/// НЕ беспрефиксная `<availableValue>` (падала бы в пустой ns → XDTO ABORT на типе
/// `{…/data-composition-system/schema}DataSetFieldField`, killer #4).
#[test]
fn dcs_available_value_uses_dcssch_prefix_erp_witness() {
    use morph1c_core::ir::form::{DcsAvailableValue, DcsField, DcsParamValue};
    use morph1c_core::ir::{Lang, PropertyValue};
    let field = DcsField {
        nested: false,
        data_path: "Состояние".into(),
        field: "Состояние".into(),
        presentation_expression: None,
        title: None,
        value_type: None,
        use_restriction: None,
        attribute_use_restriction: None,
        appearance: vec![],
        available_values: vec![DcsAvailableValue {
            value: DcsParamValue::Str("Выставлен".into()),
            presentation: Some(PropertyValue::Localized(vec![(
                Lang::new("ru"),
                "Выставлен".into(),
            )])),
        }],
    };
    let out = write_server_state(&[field], &[], &[], &|_| None)
        .expect("write ServerState with availableValues");
    let s = String::from_utf8(out).expect("utf-8");
    // Обёртка в ns dcssch (byte-exact erp.cf ServerState).
    assert!(
        s.contains("<dcssch:availableValue>"),
        "availableValue wrapper must be dcssch-prefixed:\n{s}"
    );
    // И НИКОГДА беспрефиксная (пустой ns ломает DataSetFieldField content-model).
    assert!(
        !s.contains("<availableValue>"),
        "bare <availableValue> (empty ns) must NOT appear (XDTO killer):\n{s}"
    );
}

/// Корень корпуса (env `MORPH1C_FIXTURES` / `../../.fixtures`).
fn fixtures_root() -> std::path::PathBuf {
    std::env::var("MORPH1C_FIXTURES")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.fixtures")
        })
}

#[cfg(any())]
mod cf_settings_blob_tests {
    /// The EMPTY `<Settings/>` blob the form-body root[3] carries on 875 of the 876 SSL forms —
    /// reproduced byte-exact by [`super::super::write_form_settings_blob`]. This is what pins the root's
    /// ns ORDER (it differs from the `.dcss` sidecar's) and the no-trailing-EOL envelope, without
    /// any platform round-trip.
    const EMPTY_SETTINGS_XML: &[u8] = b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" \
xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" \
xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" \
xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" \
xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" \
xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" \
xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" \
xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" \
xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" \
xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>";

    #[test]
    fn empty_conditional_appearance_reproduces_the_constant_blob() {
        assert_eq!(
            super::super::write_form_settings_blob(&[], false, &|_| None),
            EMPTY_SETTINGS_XML,
            "the empty <Settings/> blob must reproduce byte-exact"
        );
    }

    /// V50-таргет (2.20 / 8.3.27-writer): ростер корня БЕЗ `xmlns:pal` — 10 объявлений,
    /// остальное байт-в-байт как V59. Витнесс erp.cf: пустой блоб root[3]
    /// ExchangePlan.МобильноеПриложениеЗаказыКлиентов/ФормаГлавногоУзла (`77cbc40e-…0`,
    /// декодированные 527+BOM байт сверены с этим ожиданием побайтно, r31).
    #[test]
    fn v50_empty_blob_drops_pal_only() {
        let expected = String::from_utf8(EMPTY_SETTINGS_XML.to_vec())
            .expect("utf-8")
            .replace(
                " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"",
                "",
            );
        assert_eq!(
            super::super::write_form_settings_blob(&[], true, &|_| None),
            expected.as_bytes(),
            "the V50 empty <Settings/> blob must be the pal-less roster"
        );
    }

    /// Форм-уровневое УО с `style:`-ссылками, V50-таргет: config-`StyleItem` цвет/шрифт ⟼
    /// `0:<uuid>`, платформенные имена и `sys:`-шрифт — ВЕРБАТИМ, ростер без pal.
    /// Закон снят побайтно с erp.cf (r31): цвет — ФормаОтправкиPushУведомления `4d3af4d2-…0`;
    /// шрифт config — СервисShare/ВыборФайловКПубликации `342ec436-…0`; шрифт-платформа —
    /// ПравилаИнтеграцииС1СДокументооборотом/ВыборРеквизитаПотребителя `8d247cc7-…0`;
    /// цвет-платформа — ОтправкиОтчетности/ФормаЭлемента `82a09bc8-…0`.
    #[test]
    fn config_style_refs_rewrite_to_uuid() {
        // Корень `.dcssca` ERP-флавора (11 объявлений, БЕЗ lf/pal — ценз 506/506).
        let mut s = String::from(super::super::DCSS_DECL);
        s.push_str("\r\n<ConditionalAppearance");
        for (n, u) in [
            ("xmlns", "http://v8.1c.ru/8.1/data-composition-system/settings"),
            ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            (
                "xmlns:dcscor",
                "http://v8.1c.ru/8.1/data-composition-system/core",
            ),
            ("xmlns:style", "http://v8.1c.ru/8.1/data/ui/style"),
            ("xmlns:sys", "http://v8.1c.ru/8.1/data/ui/fonts/system"),
            ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
            ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
            ("xmlns:web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
            ("xmlns:ent", "http://v8.1c.ru/8.1/data/enterprise"),
            ("xmlns:win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
        ] {
            s.push_str(&format!(" {n}=\"{u}\""));
        }
        s.push_str(concat!(
            ">\r\n\t<item>\r\n\t\t<selection/>\r\n\t\t<filter>\r\n",
            "\t\t\t<item xsi:type=\"FilterItemComparison\">\r\n",
            "\t\t\t\t<left xsi:type=\"dcscor:Field\">Поле</left>\r\n",
            "\t\t\t\t<comparisonType>Equal</comparisonType>\r\n",
            "\t\t\t\t<right xsi:type=\"xs:boolean\">false</right>\r\n",
            "\t\t\t</item>\r\n\t\t</filter>\r\n\t\t<appearance>\r\n",
            "\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n",
            "\t\t\t\t<dcscor:parameter>ЦветТекста</dcscor:parameter>\r\n",
            "\t\t\t\t<dcscor:value xsi:type=\"v8ui:Color\">style:ЦветКонфиг</dcscor:value>\r\n",
            "\t\t\t</dcscor:item>\r\n",
            "\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n",
            "\t\t\t\t<dcscor:parameter>ЦветФона</dcscor:parameter>\r\n",
            "\t\t\t\t<dcscor:value xsi:type=\"v8ui:Color\">style:SpecialTextColor</dcscor:value>\r\n",
            "\t\t\t</dcscor:item>\r\n",
            "\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n",
            "\t\t\t\t<dcscor:parameter>Шрифт</dcscor:parameter>\r\n",
            "\t\t\t\t<dcscor:value xsi:type=\"v8ui:Font\" ref=\"style:ШрифтКонфиг\" kind=\"StyleItem\"/>\r\n",
            "\t\t\t</dcscor:item>\r\n",
            "\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n",
            "\t\t\t\t<dcscor:parameter>ШрифтЗаголовка</dcscor:parameter>\r\n",
            "\t\t\t\t<dcscor:value xsi:type=\"v8ui:Font\" ref=\"style:NormalTextFont\" bold=\"true\" italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"StyleItem\"/>\r\n",
            "\t\t\t</dcscor:item>\r\n",
            "\t\t\t<dcscor:item xsi:type=\"SettingsParameterValue\">\r\n",
            "\t\t\t\t<dcscor:parameter>ШрифтПодвала</dcscor:parameter>\r\n",
            "\t\t\t\t<dcscor:value xsi:type=\"v8ui:Font\" ref=\"sys:DefaultGUIFont\" bold=\"false\" italic=\"true\" underline=\"false\" strikeout=\"false\" kind=\"WindowsFont\"/>\r\n",
            "\t\t\t</dcscor:item>\r\n",
            "\t\t</appearance>\r\n\t</item>\r\n</ConditionalAppearance>\r\n",
        ));
        let (items, without_lf_pal) =
            super::super::read_conditional_appearance_dcssca(s.as_bytes()).expect("read dcssca");
        assert!(without_lf_pal, "ERP flavor");
        let resolver = |name: &str| match name {
            "ЦветКонфиг" => Some("aaaaaaaa-bbbb-cccc-dddd-eeeeffff0001".to_string()),
            "ШрифтКонфиг" => Some("aaaaaaaa-bbbb-cccc-dddd-eeeeffff0002".to_string()),
            _ => None,
        };
        let blob = super::super::write_form_settings_blob(&items, true, &resolver);
        let text = String::from_utf8(blob).expect("utf-8");
        assert!(
            !text.contains("xmlns:pal"),
            "V50 roster must not declare pal:\n{text}"
        );
        assert!(
            text.contains("xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\""),
            "style stays in the FIXED roster even when unused:\n{text}"
        );
        assert!(
            text.contains(
                "<dcscor:value xsi:type=\"v8ui:Color\">0:aaaaaaaa-bbbb-cccc-dddd-eeeeffff0001</dcscor:value>"
            ),
            "config colour must respell to 0:<uuid>:\n{text}"
        );
        assert!(
            text.contains("<dcscor:value xsi:type=\"v8ui:Color\">style:SpecialTextColor</dcscor:value>"),
            "platform colour rides verbatim:\n{text}"
        );
        assert!(
            text.contains("ref=\"0:aaaaaaaa-bbbb-cccc-dddd-eeeeffff0002\" kind=\"StyleItem\""),
            "config font ref must respell to 0:<uuid>, other attrs verbatim:\n{text}"
        );
        assert!(
            text.contains("ref=\"style:NormalTextFont\" bold=\"true\""),
            "platform font ref rides verbatim:\n{text}"
        );
        assert!(
            text.contains("ref=\"sys:DefaultGUIFont\""),
            "sys font ref rides verbatim:\n{text}"
        );
        // V59-таргет: тот же контент, но ростер С pal (SSL 876/876).
        let v59 = String::from_utf8(super::super::write_form_settings_blob(
            &items, false, &resolver,
        ))
        .expect("utf-8");
        assert!(v59.contains("xmlns:pal"), "V59 roster carries pal:\n{v59}");
    }

    /// The ONE SSL form-level conditional appearance, taken from its `.dcssca` sidecar, must
    /// serialize into the `<Settings>` blob the oracle carries at the end of root[3] — i.e. the
    /// sidecar's `<item>` verbatim, wrapped in `<conditionalAppearance>`.
    #[test]
    fn ssl_conditional_appearance_blob_is_byte_exact() {
        let Ok(fixtures) = std::env::var("MORPH1C_FIXTURES") else {
            eprintln!("INFRA-SKIP: MORPH1C_FIXTURES unset");
            return;
        };
        let p = std::path::Path::new(&fixtures).join(
            "SSL/edt/src/DataProcessors/РаботаСФайлами/Forms/ВерсияПрисоединенногоФайла/\
             ConditionalAppearance.dcssca",
        );
        let Ok(bytes) = std::fs::read(&p) else {
            eprintln!("INFRA-SKIP: {} absent", p.display());
            return;
        };
        let (items, without_lf_pal) =
            super::super::read_conditional_appearance_dcssca(&bytes).expect("read dcssca");
        assert!(!without_lf_pal, "SSL flavor carries lf+pal");
        let blob = super::super::write_form_settings_blob(&items, false, &|_| None);
        let text = String::from_utf8(blob).expect("utf-8");
        assert!(
            text.contains("<conditionalAppearance>\r\n\t\t<item>\r\n\t\t\t<selection>"),
            "the appearance must nest one level deeper than in the sidecar:\n{text}"
        );
        assert!(
            text.ends_with("\t</conditionalAppearance>\r\n</Settings>"),
            "no trailing EOL after the root:\n{text}"
        );
        assert!(
            text.contains("<left xsi:type=\"dcscor:Field\">ОбъектПрототип.Том</left>"),
            "the filter's left field survives verbatim:\n{text}"
        );
    }
}

#[cfg(any())]
mod dcssca_tests {
    /// The ONE SSL `ConditionalAppearance.dcssca` re-serializes BYTE-EXACT through the
    /// reader→writer pair (the same acceptance the `.dcss` sidecar holds to).
    #[test]
    fn ssl_dcssca_sidecar_round_trips_byte_exact() {
        let Ok(fixtures) = std::env::var("MORPH1C_FIXTURES") else {
            eprintln!("INFRA-SKIP: MORPH1C_FIXTURES unset");
            return;
        };
        let p = std::path::Path::new(&fixtures).join(
            "SSL/edt/src/DataProcessors/РаботаСФайлами/Forms/ВерсияПрисоединенногоФайла/\
             ConditionalAppearance.dcssca",
        );
        let Ok(bytes) = std::fs::read(&p) else {
            eprintln!("INFRA-SKIP: {} absent", p.display());
            return;
        };
        let (items, without_lf_pal) =
            super::super::read_conditional_appearance_dcssca(&bytes).expect("read dcssca");
        assert_eq!(items.len(), 1, "one <item>");
        assert!(!without_lf_pal, "SSL flavor carries lf+pal");
        assert_eq!(
            super::super::write_conditional_appearance_dcssca(&items, without_lf_pal),
            bytes,
            "dcssca must re-serialize byte-exact"
        );
    }
}

#[cfg(any())]
mod erp_extinfo_body_tests {
    //! ПОД-КЛАССЫ 2/4a/4b семейства «тело ExtInfo form-атрибута» (ERP dcss-сайдкары).
    use morph1c_core::ir::form::{DcsItem, DcsPresentation, DcsRightValue};

    /// ПОД-КЛАСС 2: `<filter>`-группа несёт ГРУППОВОЕ `<userSettingPresentation xsi:type="xs:string">`
    /// ПОСЛЕ userSettingID (witness ERP ОтветственныеЗаАктуализациюТокенов…ФормаСписка). Сайдкар
    /// ERP-flavored (БЕЗ pal, 10 ns) — тест покрывает и pal-опциональность конверта.
    const FILTER_PRESENTATION_DCSS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Settings xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\">\r\n\
\t<filter>\r\n\
\t\t<item xsi:type=\"FilterItemComparison\">\r\n\
\t\t\t<use>false</use>\r\n\
\t\t\t<left xsi:type=\"dcscor:Field\">Ссылка.Ответственный</left>\r\n\
\t\t\t<comparisonType>Equal</comparisonType>\r\n\
\t\t</item>\r\n\
\t\t<userSettingID>e42308fa-e52a-4887-ae4e-b43b7fdaf4ce</userSettingID>\r\n\
\t\t<userSettingPresentation xsi:type=\"xs:string\">Ответственный</userSettingPresentation>\r\n\
\t</filter>\r\n\
\t<order>\r\n\
\t\t<viewMode>Normal</viewMode>\r\n\
\t\t<userSettingID>88619765-ccb3-46c6-ac52-38e9c992ebd4</userSettingID>\r\n\
\t</order>\r\n\
\t<conditionalAppearance>\r\n\
\t\t<viewMode>Normal</viewMode>\r\n\
\t\t<userSettingID>b75fecce-942b-4aed-abc9-e6a02e460fb3</userSettingID>\r\n\
\t</conditionalAppearance>\r\n\
\t<itemsViewMode>Normal</itemsViewMode>\r\n\
\t<itemsUserSettingID>911b6018-f537-43e8-a417-da56b22f9aec</itemsUserSettingID>\r\n\
</Settings>\r\n";

    #[test]
    fn filter_group_user_setting_presentation_byte_exact() {
        let src = FILTER_PRESENTATION_DCSS.as_bytes().to_vec();
        let ls = super::super::read_list_settings_dcss(&src).expect("read dcss");
        assert!(ls.envelope_without_pal, "ERP-flavor: конверт без xmlns:pal");
        let f = ls.filter.as_ref().expect("filter group");
        assert_eq!(
            f.user_setting_presentation,
            Some(DcsPresentation::Str("Ответственный".into())),
            "групповое представление отбора прочитано"
        );
        assert_eq!(
            super::super::write_list_settings_dcss(&ls),
            src,
            "dcss byte-exact re-emit (вкл. userSettingPresentation + конверт без pal)"
        );
    }

    // --- ПОД-КЛАСС 4a: designer `<dcsset:right xsi:type="v8:Type">` под ЛЮБЫМ авто-префиксом ------

    const D10_RIGHT_LS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ListSettings xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dcsset:filter>\
<dcsset:item xsi:type=\"dcsset:FilterItemComparison\">\
<dcsset:left xsi:type=\"dcscor:Field\">Поле</dcsset:left>\
<dcsset:comparisonType>Equal</dcsset:comparisonType>\
<dcsset:right xmlns:d10p1=\"http://v8.1c.ru/8.2/data/types\" xsi:type=\"v8:Type\">d10p1:Undefined</dcsset:right>\
</dcsset:item>\
</dcsset:filter>\
</ListSettings>";

    #[test]
    fn designer_right_v8type_accepts_deep_locus_prefix() {
        // ЧекиККМ: right внутри вложенной группы ⇒ авто-префикс `d10p1` (не `d8p1`). Принимаем и
        // КАНОНИЗИРУЕМ к `d8p1:Undefined` (под ним cf-reprefix и sidecar-адаптер его пере-выводят).
        let root = crate::parse(D10_RIGHT_LS.as_bytes()).expect("parse").root;
        let ls = crate::form::read::read_designer_list_settings(&root).expect("read designer LS");
        let item = &ls.filter.as_ref().unwrap().items[0];
        let DcsItem::FilterComparison { right, .. } = item else {
            panic!("expected FilterItemComparison, got {item:?}");
        };
        assert_eq!(
            right.as_slice(),
            &[DcsRightValue::TypeQName("d8p1:Undefined".into())],
            "любой авто-префикс канонизируется к d8p1"
        );
    }

    #[test]
    fn designer_right_v8type_without_inline_ns_refuses() {
        // §1.0: `v8:Type` без инлайн-объявления types-ns — типизированный отказ.
        let bad = D10_RIGHT_LS.replace(" xmlns:d10p1=\"http://v8.1c.ru/8.2/data/types\"", "");
        let root = crate::parse(bad.as_bytes()).expect("parse").root;
        let e = crate::form::read::read_designer_list_settings(&root)
            .expect_err("no inline types-ns must refuse");
        assert!(e.to_string().contains("§1.0"), "loud typed refusal: {e}");
    }

    // --- ПОД-КЛАСС 4b: `SettingsParameterValue` БЕЗ `<dcscor:value>` (use=false dataParameter) ----

    const SPV_NO_VALUE_LS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ListSettings xmlns:dcsset=\"http://v8.1c.ru/8.1/data-composition-system/settings\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dcsset:dataParameters>\
<dcscor:item xsi:type=\"dcsset:SettingsParameterValue\">\
<dcscor:use>false</dcscor:use>\
<dcscor:parameter>Организация</dcscor:parameter>\
</dcscor:item>\
</dcsset:dataParameters>\
</ListSettings>";

    #[test]
    fn settings_parameter_value_optional_missing_value() {
        let root = crate::parse(SPV_NO_VALUE_LS.as_bytes()).expect("parse").root;
        let ls = crate::form::read::read_designer_list_settings(&root).expect("read designer LS");
        assert_eq!(ls.data_parameters.len(), 1);
        let p = &ls.data_parameters[0];
        assert_eq!(p.value, None, "неиспользуемый dataParameter — без value");
        assert_eq!(p.used, Some(false));
        assert_eq!(p.parameter, "Организация");
        // Writer НЕ эмитит `<dcscor:value>` для None (byte-круг замыкается).
        let out = crate::form::write::designer_dcs_settings_parameter_value(p);
        assert!(
            out.children.iter().all(|c| c.local != "value"),
            "None ⇒ ни одного <dcscor:value>"
        );
        assert!(out.children.iter().any(|c| c.local == "parameter"));
    }
}

#[cfg(any())]
mod dcs_settings_extras_sidecar_tests {
    //! DCS-schema settings extras в EDT-сайдкаре, byte-exact round-trip:
    //! * SC4 — `itemsUserSettingPresentation` ПОСЛЕ itemsUserSettingID (witness ERP
    //!   ПравилаРаспределенияРасходов.ФормаСпискаВручную реквизит ПоказателиКЗаполнению);
    //! * SC5 — CA-элемент БЕЗ `<appearance>` + пустой САМОЗАКРЫТЫЙ `<filter/>` (witness ERP
    //!   НДССостояниеРеализации0.ФормаРабочееМесто).
    use morph1c_core::ir::form::{DcsItem, DcsPresentation};

    /// 11-ns корень сайдкара (SSL-flavor, с pal) — соответствует `DCSS_ROOT_NS`.
    const NS11: &str = "xmlns=\"http://v8.1c.ru/8.1/data-composition-system/settings\" \
xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" \
xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" \
xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" \
xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" \
xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" \
xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" \
xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" \
xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\"";

    #[test]
    fn sc4_items_user_setting_presentation_byte_exact() {
        let src = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Settings {NS11}>\r\n\
\t<filter>\r\n\t\t<viewMode>Normal</viewMode>\r\n\t\t<userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</userSettingID>\r\n\t</filter>\r\n\
\t<order>\r\n\t\t<viewMode>Normal</viewMode>\r\n\t\t<userSettingID>88619765-ccb3-46c6-ac52-38e9c992ebd4</userSettingID>\r\n\t</order>\r\n\
\t<conditionalAppearance>\r\n\t\t<viewMode>Normal</viewMode>\r\n\t\t<userSettingID>b75fecce-942b-4aed-abc9-e6a02e460fb3</userSettingID>\r\n\t</conditionalAppearance>\r\n\
\t<itemsViewMode>Normal</itemsViewMode>\r\n\
\t<itemsUserSettingID>911b6018-f537-43e8-a417-da56b22f9aec</itemsUserSettingID>\r\n\
\t<itemsUserSettingPresentation xsi:type=\"v8:LocalStringType\">\r\n\
\t\t<v8:item>\r\n\t\t\t<v8:lang>ru</v8:lang>\r\n\t\t\t<v8:content>Сгруппировать по типу</v8:content>\r\n\t\t</v8:item>\r\n\
\t\t<v8:item>\r\n\t\t\t<v8:lang>en</v8:lang>\r\n\t\t\t<v8:content>Group by type</v8:content>\r\n\t\t</v8:item>\r\n\
\t</itemsUserSettingPresentation>\r\n\
</Settings>\r\n"
        )
        .into_bytes();
        let ls = super::super::read_list_settings_dcss(&src).expect("read dcss");
        match ls
            .items_user_setting_presentation
            .as_ref()
            .expect("SC4: itemsUserSettingPresentation прочитан")
        {
            DcsPresentation::LocalString(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0].1, "Сгруппировать по типу");
                assert_eq!(pairs[1].1, "Group by type");
            }
            other => panic!("ожидался LocalString, получен {other:?}"),
        }
        assert_eq!(
            super::super::write_list_settings_dcss(&ls),
            src,
            "SC4 byte-exact re-emit (itemsUserSettingPresentation)"
        );
    }

    #[test]
    fn sc5_ca_item_without_appearance_byte_exact() {
        let src = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Settings {NS11}>\r\n\
\t<conditionalAppearance>\r\n\
\t\t<item>\r\n\
\t\t\t<selection>\r\n\t\t\t\t<item>\r\n\t\t\t\t\t<field>СчетФактураНаНеподтвержденнуюРеализацию0</field>\r\n\t\t\t\t</item>\r\n\t\t\t</selection>\r\n\
\t\t\t<filter/>\r\n\
\t\t</item>\r\n\
\t\t<viewMode>Normal</viewMode>\r\n\
\t\t<userSettingID>b75fecce-942b-4aed-abc9-e6a02e460fb3</userSettingID>\r\n\
\t</conditionalAppearance>\r\n\
\t<itemsViewMode>Normal</itemsViewMode>\r\n\
\t<itemsUserSettingID>911b6018-f537-43e8-a417-da56b22f9aec</itemsUserSettingID>\r\n\
</Settings>\r\n"
        )
        .into_bytes();
        let ls = super::super::read_list_settings_dcss(&src).expect("read dcss");
        let ca = ls
            .conditional_appearance
            .as_ref()
            .expect("CA group present");
        let DcsItem::ConditionalAppearance {
            appearance,
            filter,
            selection,
            ..
        } = &ca.items[0]
        else {
            panic!("ожидался CA-элемент, получен {:?}", ca.items[0]);
        };
        assert!(appearance.is_empty(), "SC5: CA-элемент без <appearance>");
        assert!(filter.is_empty(), "SC5: пустой <filter/>");
        assert!(selection.is_some(), "selection присутствует");
        assert_eq!(
            super::super::write_list_settings_dcss(&ls),
            src,
            "SC5 byte-exact re-emit (CA без appearance, самозакрытый filter)"
        );
    }

    /// ERP round 14 — абсолютный `#RRGGBB` цвет условного оформления: платформа пишет hex ВЕРБАТИМ
    /// (витнесс синтетик-DL `#0000C0`/`#FFFF00`, cf_compare==0), прочие флейворы не-абсолютны.
    #[test]
    fn absolute_rgb_hex_discriminator() {
        assert!(super::super::is_absolute_rgb_hex("#0000C0"));
        assert!(super::super::is_absolute_rgb_hex("#FFFF00"));
        assert!(super::super::is_absolute_rgb_hex("#abcdef"));
        assert!(!super::super::is_absolute_rgb_hex("#FFF")); // 3-hex short form не витнессирован
        assert!(!super::super::is_absolute_rgb_hex("#GGGGGG")); // не hex
        assert!(!super::super::is_absolute_rgb_hex("web:Yellow"));
        assert!(!super::super::is_absolute_rgb_hex("style:Тест"));
        assert!(!super::super::is_absolute_rgb_hex("auto"));
    }
}
