//! FIELDS · unit tests moved out of `fields.rs`: choiceParameters/minMax/ColorDef/win-color+picture codec X-equality and §1.0 refusal coverage.

use super::*;

#[cfg(any())]
mod choice_parameters_tests {
    use super::*;
    use crate::read::parse;
    use morph1c_core::ir::value::ValueScalarKind;

    /// `parse` возвращает верхний элемент документа как `root` — берём его прямо (host).
    fn doc_root(xml: &str) -> Element {
        parse(xml.as_bytes()).expect("parse").root
    }

    /// choiceParameters декодируется в ИДЕНТИЧНЫЙ канонический IR из обоих диалектов (X by
    /// construction) — обёртка FormChoiceListDesTimeValue отбрасывает пустой Presentation, а скаляр
    /// идёт через уже-X-верифицированный value_codec. (Byte-exact R доказывает `survey` на реальной
    /// форме ОтправкаСообщения обоих диалектов.)
    #[test]
    fn choice_parameters_edt_designer_x_equal() {
        // EDT: повторяемые <choiceParameters> под хостом-extInfo (bool true + bool false-self-close).
        let edt_host = doc_root(
            "<extInfo>\
             <choiceParameters><name>Отбор.ИспользоватьДляОтправки</name>\
             <value xsi:type=\"form:FormChoiceListDesTimeValue\">\
             <value xsi:type=\"core:BooleanValue\"><value>true</value></value></value></choiceParameters>\
             <choiceParameters><name>Отбор.ПометкаУдаления</name>\
             <value xsi:type=\"form:FormChoiceListDesTimeValue\">\
             <value xsi:type=\"core:BooleanValue\"/></value></choiceParameters>\
             </extInfo>",
        );
        let edt_items =
            read_choice_parameters_edt(&edt_host, "choiceParameters").expect("edt read");

        // Designer: контейнер <ChoiceParameters> c <app:item> (тот же логический контент).
        let des_host = doc_root(
            "<ChoiceParameters>\
             <app:item name=\"Отбор.ИспользоватьДляОтправки\"><app:value xsi:type=\"FormChoiceListDesTimeValue\">\
             <Presentation/><Value xsi:type=\"xs:boolean\">true</Value></app:value></app:item>\
             <app:item name=\"Отбор.ПометкаУдаления\"><app:value xsi:type=\"FormChoiceListDesTimeValue\">\
             <Presentation/><Value xsi:type=\"xs:boolean\">false</Value></app:value></app:item>\
             </ChoiceParameters>",
        );
        let des_items = read_choice_parameters_designer(&des_host).expect("des read");

        assert_eq!(
            edt_items, des_items,
            "choiceParameters: edt IR != designer IR (X broken)"
        );
        assert_eq!(edt_items.len(), 2);
        // Содержимое: имя-параметр + булев Value (true, затем false).
        let (n0, s0) = choice_param_item(&edt_items[0]).expect("item0");
        assert_eq!(n0, "Отбор.ИспользоватьДляОтправки");
        match s0 {
            ChoiceParamValue::Scalar(spec) => {
                assert_eq!(spec.kind, ValueScalarKind::Bool);
                assert_eq!(spec.scalar.as_deref(), Some(&PropertyValue::Bool(true)));
            }
            _ => panic!("item0 must be a scalar"),
        }
        let (n1, s1) = choice_param_item(&edt_items[1]).expect("item1");
        assert_eq!(n1, "Отбор.ПометкаУдаления");
        match s1 {
            ChoiceParamValue::Scalar(spec) => {
                assert_eq!(spec.scalar.as_deref(), Some(&PropertyValue::Bool(false)));
            }
            _ => panic!("item1 must be a scalar"),
        }
    }

    // ---- ПОД-КЛАСС 1: без-обёрточный Undefined choiceParameters (пустой выбор) ----

    fn edt_env() -> crate::emit::Envelope {
        crate::emit::Envelope {
            bom: false,
            eol: "\n",
            indent_unit: "  ",
            decl: "<?xml version=\"1.0\"?>",
            trailing_eol: false,
            escape_gt: true,
            escape_quot: false,
            text_eol: "\n",
        }
    }
    fn designer_env() -> crate::emit::Envelope {
        crate::emit::Envelope {
            indent_unit: "\t",
            ..edt_env()
        }
    }

    /// БЕЗ-ОБЁРТОЧНЫЙ Undefined (ERP ×7: РеквизитыОрганизации/ВыплатаЗарплаты/…): EDT прямой
    /// `<value xsi:type="core:UndefinedValue"/>` ⟺ Designer `<app:value xsi:nil="true"/>` — X-equal,
    /// канон арность-1 `[name]` (ОТЛИЧЕН от завёрнутого-Undefined арности-2).
    #[test]
    fn choice_parameters_bare_undefined_x_equal_and_distinct() {
        let edt_host = doc_root(
            "<extInfo><choiceParameters><name>ПоОстаткам</name>\
             <value xsi:type=\"core:UndefinedValue\"/></choiceParameters></extInfo>",
        );
        let edt_items =
            read_choice_parameters_edt(&edt_host, "choiceParameters").expect("edt read");
        let des_host = doc_root(
            "<ChoiceParameters><app:item name=\"ПоОстаткам\">\
             <app:value xsi:nil=\"true\"/></app:item></ChoiceParameters>",
        );
        let des_items = read_choice_parameters_designer(&des_host).expect("des read");
        assert_eq!(edt_items, des_items, "bare Undefined: edt IR != designer IR");
        // Канон — арность-1 `[Str(name)]`.
        assert_eq!(
            edt_items,
            vec![PropertyValue::List(vec![PropertyValue::Str(
                "ПоОстаткам".into()
            )])]
        );
        let (name, v) = choice_param_item(&edt_items[0]).expect("item");
        assert_eq!(name, "ПоОстаткам");
        assert!(matches!(v, ChoiceParamValue::BareUndefined));

        // ОТЛИЧЕН от ЗАВЁРНУТОГО-Undefined (арность-2 `[name, Value(Undefined)]`; ERP ×1
        // РесурсныеСпецификации) — разные байт-формы, разный IR.
        let wrapped_host = doc_root(
            "<extInfo><choiceParameters><name>ПоОстаткам</name>\
             <value xsi:type=\"form:FormChoiceListDesTimeValue\">\
             <value xsi:type=\"core:UndefinedValue\"/></value></choiceParameters></extInfo>",
        );
        let wrapped =
            read_choice_parameters_edt(&wrapped_host, "choiceParameters").expect("wrapped read");
        assert_ne!(edt_items, wrapped, "bare vs wrapped Undefined must differ in IR");
    }

    /// Round-trip БАЙТ-ТОЧЕН в ОБОИХ диалектах (bare Undefined re-emits its own byte-form, не
    /// коллапсирует в обёртку и не наоборот).
    #[test]
    fn choice_parameters_bare_undefined_byte_exact_roundtrip() {
        // EDT.
        let edt_host = doc_root(
            "<extInfo><choiceParameters><name>ПоОстаткам</name>\
             <value xsi:type=\"core:UndefinedValue\"/></choiceParameters></extInfo>",
        );
        let items = read_choice_parameters_edt(&edt_host, "choiceParameters").expect("edt read");
        let mut out = OutElement::branch("", "extInfo");
        emit_choice_parameters_edt(&mut out, &items).expect("edt emit");
        let bytes = String::from_utf8(crate::emit::render(&edt_env(), &out.children[0])).unwrap();
        assert_eq!(
            bytes,
            "<?xml version=\"1.0\"?>\n<choiceParameters>\n  <name>ПоОстаткам</name>\n  \
             <value xsi:type=\"core:UndefinedValue\"/>\n</choiceParameters>"
        );
        // Designer.
        let des_host = doc_root(
            "<ChoiceParameters><app:item name=\"ПоОстаткам\">\
             <app:value xsi:nil=\"true\"/></app:item></ChoiceParameters>",
        );
        let des_items = read_choice_parameters_designer(&des_host).expect("des read");
        let cont = emit_choice_parameters_designer(&des_items).expect("des emit");
        let dbytes = String::from_utf8(crate::emit::render(&designer_env(), &cont)).unwrap();
        assert_eq!(
            dbytes,
            "<?xml version=\"1.0\"?>\n<ChoiceParameters>\n\t<app:item name=\"ПоОстаткам\">\n\t\t\
             <app:value xsi:nil=\"true\"/>\n\t</app:item>\n</ChoiceParameters>"
        );
    }

    /// §1.0: без-обёрточное значение ИНОГО вида, чем `core:UndefinedValue` (напр. прямой
    /// `core:StringValue` без FormChoiceListDesTimeValue), НЕ витнессировано → типизированный отказ
    /// (не тихий приём). Только `Undefined` witnessed как без-обёрточный.
    #[test]
    fn choice_parameters_edt_unwrapped_nonundefined_refused() {
        let edt_host = doc_root(
            "<extInfo><choiceParameters><name>X</name>\
             <value xsi:type=\"core:StringValue\"><value>z</value></value></choiceParameters></extInfo>",
        );
        let err = read_choice_parameters_edt(&edt_host, "choiceParameters").unwrap_err();
        // decode_fcldtv требует обёртку FormChoiceListDesTimeValue — ловит чужой прямой xsi.
        assert!(format!("{err:?}").contains("FormChoiceListDesTimeValue"), "{err:?}");
    }

    /// Непустой Presentation в форм-choiceParameters НЕ смоделирован → §1.0-ошибка (не тихий дроп).
    #[test]
    fn choice_parameters_designer_nonempty_presentation_errors() {
        let des_host = doc_root(
            "<ChoiceParameters>\
             <app:item name=\"X\"><app:value xsi:type=\"FormChoiceListDesTimeValue\">\
             <Presentation><v8:item><v8:lang>ru</v8:lang><v8:content>z</v8:content></v8:item></Presentation>\
             <Value xsi:type=\"xs:boolean\">true</Value></app:value></app:item>\
             </ChoiceParameters>",
        );
        assert!(read_choice_parameters_designer(&des_host).is_err());
    }
}

// ПОД-КЛАСС 2: InputField minValue/maxValue — nullable-скаляр xsi-вида через общий value_codec
// (Codec::Value). ERP несёт Number-ДЕСЯТИЧНЫЙ (`99.99`/`0.001`) И `core:StringValue` (`"1"`),
// чего узкий Int-кодек не покрывал. Оба диалекта → ОДИН канон (X); round-trip байт-точен.
#[cfg(any())]
mod min_max_value_tests {
    use super::*;
    use crate::emit::{render, Envelope};
    use crate::read::parse;
    use morph1c_core::ir::value::ValueScalarKind;

    fn env() -> Envelope {
        Envelope {
            bom: false,
            eol: "\n",
            indent_unit: "  ",
            decl: "<?xml version=\"1.0\"?>",
            trailing_eol: false,
            escape_gt: true,
            escape_quot: false,
            text_eol: "\n",
        }
    }
    fn min_proj() -> FieldProj {
        FieldProj {
            id: ff::F_EXT_MIN_VALUE,
            edt: "minValue",
            des: "MinValue",
            region: Region::Ext,
            codec: Codec::Value,
            policy: Policy::Symmetric,
            des_attr: false,
        }
    }
    fn host(xml: &str) -> Element {
        parse(xml.as_bytes()).expect("parse").root
    }
    fn spec_of(v: &PropertyValue) -> &morph1c_core::ir::value::ValueSpec {
        match v {
            PropertyValue::Value(s) => s,
            other => panic!("want Value, got {other:?}"),
        }
    }
    fn rt_edt(proj: &FieldProj, xml: &str) -> String {
        let v = decode_edt(proj, &host(xml)).expect("edt decode");
        let out = render_edt(proj, &v).expect("edt render");
        String::from_utf8(render(&env(), &out)).unwrap()
    }
    fn rt_des(proj: &FieldProj, xml: &str) -> String {
        let v = decode_designer(proj, &host(xml)).expect("des decode");
        let out = render_designer(proj, &v).expect("des render");
        String::from_utf8(render(&env(), &out)).unwrap()
    }

    /// Number-ДЕСЯТИЧНЫЙ (`99.99`) — прежний Int-кодек падал `bad int "99.99"`. Оба диалекта →
    /// ОДИН канон `Value(Number, "99.99")`; round-trip байт-точен (десятичный текст как есть).
    #[test]
    fn min_value_decimal_x_equal_and_byte_exact() {
        let p = min_proj();
        let e = decode_edt(
            &p,
            &host("<minValue xsi:type=\"core:NumberValue\"><value>99.99</value></minValue>"),
        )
        .expect("edt");
        let d = decode_designer(&p, &host("<MinValue xsi:type=\"xs:decimal\">99.99</MinValue>"))
            .expect("des");
        assert_eq!(e, d, "decimal min: edt IR != designer IR (X broken)");
        let sp = spec_of(&e);
        assert_eq!(sp.kind, ValueScalarKind::Number);
        assert_eq!(sp.scalar.as_deref(), Some(&PropertyValue::Str("99.99".into())));
        // Byte-exact round-trip обоих диалектов.
        assert_eq!(
            rt_edt(
                &p,
                "<minValue xsi:type=\"core:NumberValue\"><value>99.99</value></minValue>"
            ),
            "<?xml version=\"1.0\"?>\n<minValue xsi:type=\"core:NumberValue\">\n  <value>99.99</value>\n</minValue>"
        );
        assert_eq!(
            rt_des(&p, "<MinValue xsi:type=\"xs:decimal\">99.99</MinValue>"),
            "<?xml version=\"1.0\"?>\n<MinValue xsi:type=\"xs:decimal\">99.99</MinValue>"
        );
    }

    /// `core:StringValue` (`"1"`) min/max — прежний кодек падал `want "core:NumberValue"`. Оба
    /// диалекта → `Value(Str, "1")`; round-trip байт-точен.
    #[test]
    fn min_value_string_x_equal_and_byte_exact() {
        let p = min_proj();
        let e = decode_edt(
            &p,
            &host("<minValue xsi:type=\"core:StringValue\"><value>1</value></minValue>"),
        )
        .expect("edt");
        let d = decode_designer(&p, &host("<MinValue xsi:type=\"xs:string\">1</MinValue>"))
            .expect("des");
        assert_eq!(e, d, "string min: edt IR != designer IR (X broken)");
        assert_eq!(spec_of(&e).kind, ValueScalarKind::Str);
        assert_eq!(
            rt_edt(
                &p,
                "<minValue xsi:type=\"core:StringValue\"><value>1</value></minValue>"
            ),
            "<?xml version=\"1.0\"?>\n<minValue xsi:type=\"core:StringValue\">\n  <value>1</value>\n</minValue>"
        );
        assert_eq!(
            rt_des(&p, "<MinValue xsi:type=\"xs:string\">1</MinValue>"),
            "<?xml version=\"1.0\"?>\n<MinValue xsi:type=\"xs:string\">1</MinValue>"
        );
    }

    /// §1.0: невитнессированный xsi-тип min/max → ТИПИЗИРОВАННЫЙ отказ (не guess) в обоих диалектах.
    #[test]
    fn min_value_unknown_xsi_refused() {
        let p = min_proj();
        assert!(decode_edt(&p, &host("<minValue xsi:type=\"core:FooValue\"/>")).is_err());
        assert!(decode_designer(&p, &host("<MinValue xsi:type=\"xs:foo\">1</MinValue>")).is_err());
    }
}

#[cfg(any())]
mod color_def_tests {
    //! EDT `core:ColorDef` с ОПУЩЕННЫМИ нулевыми RGB-компонентами (census ERP: `<blue>`-only ×60,
    //! red+green ×49, green+blue ×42, red-only ×21, green-only ×7, red+blue ×1, пустой самозакрытый
    //! ×649 из 1829; 0 explicit `<c>0</c>`). Reader читает любую подрешётку {red,green,blue}
    //! (отсутствие → 0), writer симметрично ОПУСКАЕТ нули (все-нулевой → самозакрытый).
    use super::*;
    use crate::emit::{render, Envelope};
    use crate::read::parse;

    fn doc_root(xml: &str) -> Element {
        parse(xml.as_bytes()).expect("parse").root
    }

    fn edt_env() -> Envelope {
        Envelope {
            bom: false,
            eol: "\n",
            indent_unit: "  ",
            decl: "<?xml version=\"1.0\"?>",
            trailing_eol: false,
            escape_gt: true,
            escape_quot: false,
            text_eol: "\n",
        }
    }

    fn dec(xml: &str) -> String {
        match decode_edt_color(&doc_root(xml), "value").expect("decode") {
            PropertyValue::Ref(s) => s,
            other => panic!("want Ref, got {other:?}"),
        }
    }

    /// Reader: одна/две компоненты и пустой ColorDef канонизуются в `#RRGGBB` (отсутствие → 0).
    #[test]
    fn colordef_omitted_zero_components_decode() {
        // green-only ⇒ #00XX00.
        assert_eq!(
            dec("<value xsi:type=\"core:ColorDef\"><green>153</green></value>"),
            "#009900"
        );
        // red+blue ⇒ #XX00XX.
        assert_eq!(
            dec("<value xsi:type=\"core:ColorDef\"><red>255</red><blue>128</blue></value>"),
            "#FF0080"
        );
        // blue-only (доминирующий частичный класс ERP).
        assert_eq!(
            dec("<value xsi:type=\"core:ColorDef\"><blue>200</blue></value>"),
            "#0000C8"
        );
        // пустой самозакрытый ⇒ #000000.
        assert_eq!(dec("<value xsi:type=\"core:ColorDef\"/>"), "#000000");
        // полный триплет — прежний happy-path (регресс-контроль).
        assert_eq!(
            dec("<value xsi:type=\"core:ColorDef\">\
                 <red>255</red><green>255</green><blue>153</blue></value>"),
            "#FFFF99"
        );
    }

    /// §1.0: непустой ColorDef с ребёнком НЕ из {red,green,blue} — ГРОМКИЙ типизированный отказ.
    #[test]
    fn colordef_unknown_child_is_typed_error() {
        let err = decode_edt_color(
            &doc_root(
                "<value xsi:type=\"core:ColorDef\"><green>1</green><alpha>7</alpha></value>",
            ),
            "value",
        )
        .expect_err("unknown ColorDef child must error");
        assert!(
            format!("{err}").contains("alpha"),
            "error must name the stray child: {err}"
        );
    }

    /// Writer симметричен reader'у: ОПУСКАЕТ нулевые компоненты; все-нулевой → самозакрытый.
    #[test]
    fn render_edt_color_omits_zero_components() {
        let kids = |el: &OutElement| -> Vec<(String, Option<String>)> {
            el.children
                .iter()
                .map(|c| (c.local.clone(), c.text.clone()))
                .collect()
        };
        // green-only.
        let el = render_edt_color("value", "#009900").expect("render");
        assert!(!el.self_closing);
        assert_eq!(kids(&el), vec![("green".into(), Some("153".into()))]);
        // red+blue (порядок red,green,blue сохранён — green опущен).
        let el = render_edt_color("value", "#FF0080").expect("render");
        assert_eq!(
            kids(&el),
            vec![
                ("red".into(), Some("255".into())),
                ("blue".into(), Some("128".into())),
            ]
        );
        // all-zero ⇒ самозакрытый без детей.
        let el = render_edt_color("value", "#000000").expect("render");
        assert!(el.self_closing, "#000000 → self-closing ColorDef");
        assert!(el.children.is_empty());
    }

    /// Байтовый round-trip decode∘render∘parse∘decode для каждого частичного шейпа.
    #[test]
    fn colordef_render_decode_roundtrip() {
        for canon in ["#009900", "#FF0080", "#0000C8", "#000000", "#FFFF99"] {
            let out = render_edt_color("value", canon).expect("render");
            let bytes = render(&edt_env(), &out);
            let host = parse(&bytes).expect("re-parse").root;
            let back = match decode_edt_color(&host, "value").expect("decode") {
                PropertyValue::Ref(s) => s,
                other => panic!("want Ref, got {other:?}"),
            };
            assert_eq!(back, canon, "round-trip {canon}");
        }
    }
}

#[cfg(any())]
mod win_color_and_picture_tests {
    //! W25 (picture LoadTransparent — независимый флаг) и системные цвета Windows (`win:` ⟺
    //! `Windows.`), оба ERP-only. §1.0: витнессированное round-trip'ит байт-точно, невитнесснутое —
    //! типизированный отказ.
    use super::*;

    // --- Windows системные цвета (Designer `win:X` ⟺ canon/EDT `Windows.X`) ---

    /// Каждый витнессированный `win:X` биективен с каноном `Windows.X` в ОБЕ стороны.
    #[test]
    fn win_system_color_designer_canon_bijection() {
        for name in WINDOWS_SYSTEM_COLORS {
            let des = format!("win:{name}");
            let canon = color_from_designer(&des, "TextColor").expect("win→canon");
            assert_eq!(canon, format!("Windows.{name}"));
            let back = color_to_designer(&canon, "TextColor").expect("canon→win");
            assert_eq!(back, des, "round-trip win:{name}");
        }
    }

    /// EDT-сторона уже несёт `Windows.X` через общий `core:ColorRef` — canon совпадает с Designer.
    #[test]
    fn win_system_color_edt_parity() {
        // Витнесс: Catalog.ДокументыРеализацииПолномочийНалоговыхОрганов.ФормаЭлемента
        // TextColor `win:ButtonDarkShadow` ⟺ EDT `<color>Windows.ButtonDarkShadow</color>`.
        let xml = "<value xsi:type=\"core:ColorRef\"><color>Windows.ButtonDarkShadow</color></value>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let canon = match decode_edt_color(&host, "value").expect("decode") {
            PropertyValue::Ref(s) => s,
            other => panic!("want Ref, got {other:?}"),
        };
        assert_eq!(canon, "Windows.ButtonDarkShadow");
        // EDT-рендер симметричен (общий ColorRef-путь, без спец-кода).
        let el = render_edt_color("value", &canon).expect("render");
        assert_eq!(el.children.len(), 1);
        assert_eq!(el.children[0].text.as_deref(), Some("Windows.ButtonDarkShadow"));
        // И Designer-текст того же канона — исходный `win:`.
        assert_eq!(
            color_to_designer(&canon, "TextColor").expect("canon→win"),
            "win:ButtonDarkShadow"
        );
    }

    /// §1.0: невитнессированное `win:`-имя — типизированный отказ (замкнутый allow-list).
    #[test]
    fn win_system_color_unwitnessed_name_is_typed_error() {
        let err = color_from_designer("win:HotTrackColor", "BackColor")
            .expect_err("unwitnessed win: name must error");
        assert!(
            format!("{err}").contains("win:") || format!("{err}").contains("system color"),
            "error names the encoding: {err}"
        );
        // Обратное направление тоже гейтится allow-list'ом.
        assert!(color_to_designer("Windows.HotTrackColor", "BackColor").is_err());
    }

    // --- picture LoadTransparent — независимый канон List([Ref, Bool]) ---

    /// `picture_canon`/`picture_ref_lt` — round-trip обеих полярностей независимого флага.
    #[test]
    fn picture_canon_carries_independent_load_transparent() {
        // CommonPicture c LoadTransparent=true — ERP-комбинация, раньше отвергавшаяся.
        let v = picture_canon("CommonPicture.ПиктограммыГруппСотрудников".into(), true);
        let (r, lt) = picture_ref_lt(&v).expect("unpack");
        assert_eq!(r, "CommonPicture.ПиктограммыГруппСотрудников");
        assert!(lt, "independent LoadTransparent=true preserved (not derived to false)");
        // StdPicture c LoadTransparent=false — тоже независимо хранится.
        let v = picture_canon("StdPicture.Refresh".into(), false);
        assert_eq!(picture_ref_lt(&v).unwrap(), ("StdPicture.Refresh", false));
    }

    /// Голый `Ref` (не-конвертированный источник) ДЕРИВИТ флаг из вида ссылки (§1.0-дефолт).
    #[test]
    fn picture_ref_lt_derives_for_bare_ref() {
        let std = PropertyValue::Ref("StdPicture.Refresh".into());
        assert_eq!(picture_ref_lt(&std).unwrap(), ("StdPicture.Refresh", true));
        let common = PropertyValue::Ref("CommonPicture.X".into());
        assert_eq!(picture_ref_lt(&common).unwrap(), ("CommonPicture.X", false));
        let abs = PropertyValue::Ref("abs:png".into());
        assert_eq!(picture_ref_lt(&abs).unwrap(), ("abs:png", false));
    }

    /// Designer picture-узел читает независимый LoadTransparent КАК ЕСТЬ (CommonPicture+true).
    #[test]
    fn decode_designer_picture_reads_independent_true() {
        let xml = "<RowsPicture><xr:Ref>CommonPicture.ПиктограммыГруппСотрудников</xr:Ref>\
                   <xr:LoadTransparent>true</xr:LoadTransparent></RowsPicture>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let v = decode_designer_picture(&host, "RowsPicture").expect("decode");
        assert_eq!(
            picture_ref_lt(&v).unwrap(),
            ("CommonPicture.ПиктограммыГруппСотрудников", true),
            "CommonPicture c LoadTransparent=true читается независимо, не отвергается"
        );
    }

    /// §1.0: не-bool `<xr:LoadTransparent>` — типизированный отказ.
    #[test]
    fn decode_designer_picture_non_bool_lt_is_typed_error() {
        let xml = "<RowsPicture><xr:Ref>CommonPicture.X</xr:Ref>\
                   <xr:LoadTransparent>maybe</xr:LoadTransparent></RowsPicture>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let err = decode_designer_picture(&host, "RowsPicture")
            .expect_err("non-bool LoadTransparent must error");
        assert!(
            format!("{err}").contains("LoadTransparent"),
            "error names the field: {err}"
        );
    }

    // --- ВСТРОЕННЫЙ прозрачный пиксель (form:FormPicture с содержимым / Abs+TransparentPixel) ---

    /// FieldProj-заглушка picture-слота для round-trip'ов (id несуществен для render/decode).
    fn pic_entry() -> FieldProj {
        fp(
            FieldId(0),
            "valuesPicture",
            "ValuesPicture",
            Region::Ext,
            Codec::PictureRef,
            Policy::Symmetric,
        )
    }

    /// EDT: непустой `<valuesPicture xsi:type="form:FormPicture"><transparentPixel>…` читается
    /// в канон-с-пикселем и рендерится БАЙТ-ТОЧНО обратно (оба листа `<x>`/`<y>`).
    #[test]
    fn edt_form_picture_transparent_pixel_roundtrip() {
        let xml = "<valuesPicture xsi:type=\"form:FormPicture\">\
                   <transparentPixel><x>7</x><y>4</y></transparentPixel></valuesPicture>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let canon = decode_edt_picture(&host, "valuesPicture").expect("decode");
        assert_eq!(picture_pixel(&canon), Some((7, 4)));
        assert_eq!(
            picture_ref_lt(&canon).unwrap(),
            ("", true),
            "EDT marker ref empty, LT denormalized true"
        );
        // Round-trip: канон-abs (после attach) рендерится к witnessed markup (НЕ self-closing,
        // sparse-листья `<x>` перед `<y>`), re-decode возвращает пиксель.
        let canon = picture_canon_px("abs:bmp".into(), (7, 4));
        let out = render_edt(&pic_entry(), &canon).expect("render edt");
        assert!(!out.self_closing, "embedded picture is NOT the empty self-closing marker");
        let bytes = crate::emit::render(&crate::form::edt_envelope(), &out);
        let text = String::from_utf8(bytes.clone()).unwrap();
        for needle in [
            "<valuesPicture xsi:type=\"form:FormPicture\">",
            "<transparentPixel>",
            "<x>7</x>",
            "<y>4</y>",
            "</transparentPixel>",
            "</valuesPicture>",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
        let back = decode_edt_picture(&crate::read::parse(&bytes).unwrap().root, "valuesPicture")
            .expect("re-decode");
        assert_eq!(picture_pixel(&back), Some((7, 4)));
    }

    /// EDT SPARSE-пиксель (`<transparentPixel><y>30</y></transparentPixel>` — `<x>` опущен ⇒ 0):
    /// читается (0,30), рендерится байт-точно (общий sparse-кодек опускает нулевой `<x>`).
    #[test]
    fn edt_form_picture_sparse_pixel_roundtrip() {
        let xml = "<picture xsi:type=\"form:FormPicture\">\
                   <transparentPixel><y>30</y></transparentPixel></picture>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let canon = decode_edt_picture(&host, "picture").expect("decode");
        assert_eq!(picture_pixel(&canon), Some((0, 30)));
        let out = render_edt(&pic_entry(), &picture_canon_px("abs:jpg".into(), (0, 30)))
            .expect("render edt");
        let bytes = crate::emit::render(&crate::form::edt_envelope(), &out);
        let text = String::from_utf8(bytes).unwrap();
        assert!(
            text.contains("<y>30</y>") && !text.contains("<x>"),
            "sparse: zero <x> omitted, got:\n{text}"
        );
    }

    /// Designer: `<ValuesPicture><xr:Abs>…</xr:Abs><xr:LoadTransparent>true</…>
    /// <xr:TransparentPixel x=".." y=".."/>` читается в тот же канон + рендерится байт-точно
    /// (DENSE-атрибуты: `x="0"` эмитится явно).
    #[test]
    fn designer_form_picture_transparent_pixel_roundtrip() {
        let xml = "<ValuesPicture><xr:Abs>ValuesPicture.bmp</xr:Abs>\
                   <xr:LoadTransparent>true</xr:LoadTransparent>\
                   <xr:TransparentPixel x=\"7\" y=\"4\"/></ValuesPicture>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let canon = decode_designer_picture(&host, "ValuesPicture").expect("decode");
        assert_eq!(picture_ref_lt(&canon).unwrap(), ("abs:bmp", true));
        assert_eq!(picture_pixel(&canon), Some((7, 4)));
        // Render byte-exact (DENSE) + re-decode.
        let out = render_designer(&pic_entry(), &canon).expect("render des");
        let bytes = crate::emit::render(&crate::form::designer_envelope(), &out);
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(
            text.contains("<xr:LoadTransparent>true</xr:LoadTransparent>")
                && text.contains("<xr:TransparentPixel x=\"7\" y=\"4\"/>"),
            "Designer DENSE TransparentPixel markup, got:\n{text}"
        );
        let back = decode_designer_picture(&crate::read::parse(&bytes).unwrap().root, "ValuesPicture")
            .expect("re-decode");
        assert_eq!(picture_pixel(&back), Some((7, 4)));
    }

    /// Designer DENSE `x="0"` (sparse EDT-твин с опущенным `<x>`) — оба диалекта дают РАВНЫЙ
    /// пиксель (0,30) ⇒ IR-канон совпадает (§1.6).
    #[test]
    fn edt_and_designer_form_picture_pixel_agree() {
        let edt = "<picture xsi:type=\"form:FormPicture\">\
                   <transparentPixel><y>30</y></transparentPixel></picture>";
        let des = "<Picture><xr:Abs>Picture.jpg</xr:Abs>\
                   <xr:LoadTransparent>true</xr:LoadTransparent>\
                   <xr:TransparentPixel x=\"0\" y=\"30\"/></Picture>";
        let ce = decode_edt_picture(&crate::read::parse(edt.as_bytes()).unwrap().root, "picture")
            .expect("edt");
        let cd = decode_designer_picture(&crate::read::parse(des.as_bytes()).unwrap().root, "Picture")
            .expect("des");
        assert_eq!(picture_pixel(&ce), picture_pixel(&cd), "pixel agrees across dialects");
        assert_eq!(picture_pixel(&ce), Some((0, 30)));
        assert!(picture_ref_lt(&ce).unwrap().1 && picture_ref_lt(&cd).unwrap().1, "both LT=true");
    }

    /// §1.0: непустой FormPicture с посторонним ребёнком (не `<transparentPixel>`) — отказ.
    #[test]
    fn edt_form_picture_unexpected_child_is_typed_error() {
        let xml = "<picture xsi:type=\"form:FormPicture\"><bogus/></picture>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let err = decode_edt_picture(&host, "picture").expect_err("bogus child must error");
        assert!(
            format!("{err}").contains("transparentPixel"),
            "error names the expected child: {err}"
        );
    }

    /// §1.0: Designer Abs+`LoadTransparent=true` БЕЗ `<xr:TransparentPixel>` — невитнессированное
    /// противоречие (LT денормализует наличие пикселя) → отказ.
    #[test]
    fn designer_form_picture_lt_true_without_pixel_is_typed_error() {
        let xml = "<ValuesPicture><xr:Abs>ValuesPicture.png</xr:Abs>\
                   <xr:LoadTransparent>true</xr:LoadTransparent></ValuesPicture>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let err = decode_designer_picture(&host, "ValuesPicture").expect_err("LT=true no pixel");
        assert!(format!("{err}").contains("TransparentPixel"), "{err}");
    }

    /// §1.0: Designer Abs+`LoadTransparent=false` С `<xr:TransparentPixel>` — тоже отказ.
    #[test]
    fn designer_form_picture_pixel_with_lt_false_is_typed_error() {
        let xml = "<ValuesPicture><xr:Abs>ValuesPicture.png</xr:Abs>\
                   <xr:LoadTransparent>false</xr:LoadTransparent>\
                   <xr:TransparentPixel x=\"1\" y=\"2\"/></ValuesPicture>";
        let host = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let err = decode_designer_picture(&host, "ValuesPicture").expect_err("pixel with LT=false");
        assert!(format!("{err}").contains("TransparentPixel"), "{err}");
    }
}

/// ВЕРСИОННЫЙ ГЕЙТ РЕКОНСТРУКЦИИ (§1.0): свойства, которого версия источника не знает,
/// в мешке быть не должно — «присутствует в мешке» обязано значить «было в источнике».
///
/// Витнесс — `models/version_facts.jsonl`: `<Orientation>` у RadioButtonField появляется
/// только в 2.21 (дампы одной конфигурации, собранные 8.3.24/8.3.27/8.5.1). Раньше ридер
/// достраивал его ВСЕГДА (поле объявлено обязательным, `Policy::Keep`), и конверсия
/// 2.17→2.17 падала «потерей» свойства, которого в источнике не было.
#[cfg(any())]
mod source_version_fill_gate {
    use super::*;
    use crate::form::tables;
    use morph1c_core::version::FormatVersion;

    /// Прочитать extInfo-поля RadioButtonField из Designer-элемента под заданной версией
    /// источника.
    fn read_radio_ext(xml: &str, source: Option<FormatVersion>) -> Vec<(FieldId, PropertyValue)> {
        let el = crate::read::parse(xml.as_bytes()).expect("parse").root;
        let mut bag = Vec::new();
        morph1c_core::version::with_source_version(source, || {
            read_fields_designer(
                "RadioButtonField",
                &el,
                tables::RADIO_BUTTON_FIELD_EXT,
                Region::Ext,
                &mut bag,
            )
        })
        .expect("read");
        bag
    }

    fn orientation_of(bag: &[(FieldId, PropertyValue)]) -> Option<&PropertyValue> {
        use morph1c_core::spec::forms::controls::radio_button as rb;
        bag.iter()
            .find(|(id, _)| *id == rb::F_EXT_ORIENTATION)
            .map(|(_, v)| v)
    }

    /// 2.17-дамп тега не несёт (его в этой версии не существует) ⇒ поля в мешке НЕТ.
    #[test]
    fn absent_property_unknown_to_source_version_is_not_materialized() {
        let bag = read_radio_ext(
            "<RadioButtonField name=\"Поле\" id=\"1\"><ColumnsCount>4</ColumnsCount></RadioButtonField>",
            Some(FormatVersion::new(2, 17)),
        );
        assert_eq!(
            orientation_of(&bag),
            None,
            "2.17 не знает <Orientation> — ридер не вправе его достраивать"
        );
        // Умолчание при этом РАЗРЕШИМО из IR (мешок + спек), а не потеряно.
        let item = {
            let mut it = morph1c_core::ir::FormItem::new(
                morph1c_core::ir::FormControlKind::new("RadioButtonField"),
                "Поле",
                1,
            );
            it.ext_info = bag.clone();
            it
        };
        let r = morph1c_core::resolve::resolve_control(
            &item,
            "orientation",
            Some(FormatVersion::new(2, 17)),
        )
        .expect("spec declares orientation");
        assert!(!r.is_explicit(), "значение не задано в источнике");
        assert_eq!(
            r.value(),
            Some(&PropertyValue::Enum(Token::new("Vertical"))),
            "умолчание платформы разрешается из IR"
        );
    }

    /// 2.21 свойство знает ⇒ омиссия снова ДАННЫЕ (Designer опускает свой дефолт), и ридер
    /// восстанавливает его как прежде — кросс-форматная KEEP-пара не сломана.
    #[test]
    fn absent_property_known_to_source_version_is_still_reconstructed() {
        let bag = read_radio_ext(
            "<RadioButtonField name=\"Поле\" id=\"1\"><ColumnsCount>4</ColumnsCount></RadioButtonField>",
            Some(FormatVersion::new(2, 21)),
        );
        assert_eq!(
            orientation_of(&bag),
            Some(&PropertyValue::Enum(Token::new("Vertical")))
        );
    }

    /// Версия источника неизвестна ⇒ гейта нет (прежнее поведение, ничего не ломаем).
    #[test]
    fn unknown_source_version_keeps_previous_behaviour() {
        let bag = read_radio_ext(
            "<RadioButtonField name=\"Поле\" id=\"1\"><ColumnsCount>4</ColumnsCount></RadioButtonField>",
            None,
        );
        assert_eq!(
            orientation_of(&bag),
            Some(&PropertyValue::Enum(Token::new("Vertical")))
        );
    }

    /// ЗАДАННОЕ свойство остаётся в мешке при ЛЮБОЙ версии: гейт снимает только
    /// РЕКОНСТРУКЦИЮ, а не прочитанное значение (иначе EDT-дамп 2.20, который тег ПИШЕТ,
    /// потерял бы данные).
    #[test]
    fn explicit_property_is_never_dropped() {
        let bag = read_radio_ext(
            "<RadioButtonField name=\"Поле\" id=\"1\">\
             <Orientation>HorizontalIfPossible</Orientation></RadioButtonField>",
            Some(FormatVersion::new(2, 17)),
        );
        assert_eq!(
            orientation_of(&bag),
            Some(&PropertyValue::Enum(Token::new("HorizontalIfPossible")))
        );
    }
}
