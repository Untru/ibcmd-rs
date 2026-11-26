use super::*;
use crate::emit::{render, Envelope};
use crate::read::parse;

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
fn des_env() -> Envelope {
    Envelope {
        indent_unit: "\t",
        ..edt_env()
    }
}
fn host_of(xml: &str) -> Element {
    parse(xml.as_bytes())
        .expect("parse")
        .root
        .children
        .into_iter()
        .next()
        .expect("host")
}
fn spec_of(pv: PropertyValue) -> StyleValueSpec {
    match pv {
        PropertyValue::StyleValue(s) => s,
        other => panic!("not StyleValue: {:?}", other.kind()),
    }
}
fn dec_edt(xml: &str) -> StyleValueSpec {
    spec_of(decode(StyleValueDialect::Edt, &host_of(&format!("<w>{xml}</w>"))).unwrap())
}
fn dec_des(xml: &str) -> StyleValueSpec {
    spec_of(
        decode(
            StyleValueDialect::Designer,
            &host_of(&format!("<w>{xml}</w>")),
        )
        .unwrap(),
    )
}

// --- X-equality: EDT-decode == Designer-decode across all four sub-kinds ---
#[test]
fn font_ref_x_equal() {
    let e = dec_edt(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>Style.TextLevel2</font><bold>true</bold><italic>false</italic>\
         <underline>false</underline><strikeout>false</strikeout></value></value>",
    );
    let d = dec_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"style:TextLevel2\" bold=\"true\" italic=\"false\" \
         underline=\"false\" strikeout=\"false\" kind=\"StyleItem\"/>",
    );
    assert_eq!(e, d);
    assert_eq!(
        e,
        StyleValueSpec::Font(FontStyle::Ref {
            font: "Style.TextLevel2".into(),
            height: None,
            flags: FontFlags {
                bold: Some(true),
                italic: Some(false),
                underline: Some(false),
                strikeout: Some(false),
            },
            scale: None,
        })
    );
}
#[test]
fn font_ref_partial_flags_x_equal() {
    // ERP `ЖирныйПодчеркнутыйШрифт` (witnessed): только bold+underline переопределены —
    // italic/strikeout опущены в ОБОИХ диалектах (тристейт, а не «все-или-ни-одного»).
    let e = dec_edt(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>Style.NormalTextFont</font><bold>true</bold><underline>true</underline></value></value>",
    );
    let d = dec_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"style:NormalTextFont\" bold=\"true\" \
         underline=\"true\" kind=\"StyleItem\"/>",
    );
    assert_eq!(e, d);
    assert_eq!(
        e,
        StyleValueSpec::Font(FontStyle::Ref {
            font: "Style.NormalTextFont".into(),
            height: None,
            flags: FontFlags {
                bold: Some(true),
                italic: None,
                underline: Some(true),
                strikeout: None,
            },
            scale: None,
        })
    );
}
#[test]
fn font_ref_bare_x_equal() {
    // Только <font> (ни флагов, ни height/scale) ⇔ Designer ref+kind без флагов.
    let e = dec_edt(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>Style.TitleLevel2</font></value></value>",
    );
    let d =
        dec_des("<Value xsi:type=\"v8ui:Font\" ref=\"style:TitleLevel2\" kind=\"StyleItem\"/>");
    assert_eq!(e, d);
}
#[test]
fn font_ref_height_scale_x_equal() {
    let e = dec_edt(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>Style.NormalTextFont</font><height>12.0</height><bold>false</bold>\
         <italic>false</italic><underline>false</underline><strikeout>false</strikeout>\
         <scale>90</scale></value></value>",
    );
    let d = dec_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"style:NormalTextFont\" height=\"12\" bold=\"false\" \
         italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"StyleItem\" scale=\"90\"/>",
    );
    assert_eq!(e, d);
}
#[test]
fn font_system_x_equal() {
    // Системный шрифт: EDT `System.DefaultGUIFont` ⇔ Designer sys:-ref + kind=WindowsFont
    // (witnessed ERP StyleItems/ИнформационныйЦентрПолужирныйШрифт10 — height+флаги).
    let e = dec_edt(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>System.DefaultGUIFont</font><height>10.0</height><bold>true</bold>\
         <italic>false</italic><underline>false</underline><strikeout>false</strikeout></value></value>",
    );
    let d = dec_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"sys:DefaultGUIFont\" height=\"10\" bold=\"true\" \
         italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"WindowsFont\"/>",
    );
    assert_eq!(e, d);
    assert_eq!(
        e,
        StyleValueSpec::Font(FontStyle::Ref {
            font: "System.DefaultGUIFont".into(),
            height: Some(10),
            flags: FontFlags {
                bold: Some(true),
                italic: Some(false),
                underline: Some(false),
                strikeout: Some(false),
            },
            scale: None,
        })
    );
}
#[test]
fn font_system_bare_x_equal() {
    // Голый системный шрифт (без height/флагов) — witnessed ERP (`ref="sys:DefaultGUIFont"
    // kind="WindowsFont"` без прочих атрибутов).
    let e = dec_edt(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>System.DefaultGUIFont</font></value></value>",
    );
    let d = dec_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"sys:DefaultGUIFont\" kind=\"WindowsFont\"/>",
    );
    assert_eq!(e, d);
}
#[test]
fn font_system_kind_ref_mismatch_errors() {
    // Рассинхрон @kind↔@ref-префикса — невитнессированная форма (§1.0): sys:-ref при
    // kind=StyleItem и style:-ref при kind=WindowsFont оба отклоняются.
    for el in [
        "<Value xsi:type=\"v8ui:Font\" ref=\"sys:DefaultGUIFont\" kind=\"StyleItem\"/>",
        "<Value xsi:type=\"v8ui:Font\" ref=\"style:TextFont\" kind=\"WindowsFont\"/>",
    ] {
        let err = decode(StyleValueDialect::Designer, &host_of(&format!("<w>{el}</w>")))
            .unwrap_err();
        assert!(err.contains("does not match kind"), "{err}");
    }
}
#[test]
fn font_def_x_equal() {
    // EDT FontDef опускает флаги/scale; Designer эмитит их дефолтными + kind=Absolute.
    let e = dec_edt(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontDef\">\
         <faceName>Arial</faceName><height>14.0</height></value></value>",
    );
    let d = dec_des(
        "<Value xsi:type=\"v8ui:Font\" faceName=\"Arial\" height=\"14\" bold=\"false\" \
         italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"100\"/>",
    );
    assert_eq!(e, d);
    assert_eq!(
        e,
        StyleValueSpec::Font(FontStyle::Def {
            face_name: "Arial".into(),
            height: 14,
            face: FontFace::default(),
        })
    );
}
#[test]
fn font_def_flag_x_equal() {
    // ERP `ЖирныйШрифтEDI` (witnessed): FontDef с bold=true — EDT несёт ТОЛЬКО
    // true-флаг, Designer — все четыре денсово (bold="true", остальные false).
    let e = dec_edt(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontDef\">\
         <faceName>Arial</faceName><height>10.0</height><bold>true</bold></value></value>",
    );
    let d = dec_des(
        "<Value xsi:type=\"v8ui:Font\" faceName=\"Arial\" height=\"10\" bold=\"true\" \
         italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"100\"/>",
    );
    assert_eq!(e, d);
    assert_eq!(
        e,
        StyleValueSpec::Font(FontStyle::Def {
            face_name: "Arial".into(),
            height: 10,
            face: FontFace { bold: true, italic: false, underline: false, strikeout: false },
        })
    );
}
#[test]
fn color_ref_x_equal() {
    for (edt_name, des) in [
        ("Palette.Blue", "pal:Blue"),
        ("Web.Gray", "web:Gray"),
        ("Style.FormTextColor", "style:FormTextColor"),
    ] {
        let e = dec_edt(&format!(
            "<value xsi:type=\"core:ColorValue\"><value xsi:type=\"core:ColorRef\">\
             <color>{edt_name}</color></value></value>"
        ));
        let d = dec_des(&format!("<Value xsi:type=\"v8ui:Color\">{des}</Value>"));
        assert_eq!(e, d, "color ref {edt_name}");
        assert_eq!(e, StyleValueSpec::Color(ColorStyle::Ref(edt_name.into())));
    }
}
#[test]
fn border_def_x_equal() {
    // Пустой EDT BorderDef ⇔ Designer width="0" + <v8ui:style>WithoutBorder.
    let e = dec_edt(
        "<value xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderDef\"/></value>",
    );
    let d = dec_des(
        "<Value xsi:type=\"v8ui:Border\" width=\"0\">\
         <v8ui:style xsi:type=\"v8ui:ControlBorderType\">WithoutBorder</v8ui:style></Value>",
    );
    assert_eq!(e, d);
    assert_eq!(
        e,
        StyleValueSpec::Border(BorderStyle::Def {
            style: "WithoutBorder".into(),
            width: 0
        })
    );
}

#[test]
fn color_def_x_equal() {
    // green=150 blue=70 (red опущен) ⇔ #009646.
    let e = dec_edt(
        "<value xsi:type=\"core:ColorValue\"><value xsi:type=\"core:ColorDef\">\
         <green>150</green><blue>70</blue></value></value>",
    );
    let d = dec_des("<Value xsi:type=\"v8ui:Color\">#009646</Value>");
    assert_eq!(e, d);
    assert_eq!(
        e,
        StyleValueSpec::Color(ColorStyle::Def {
            red: 0,
            green: 150,
            blue: 70
        })
    );
}

// --- byte-exact round-trip both dialects ---
fn rt_des(el: &str) {
    // Designer host — single self-close (Font) or leaf (Color); round-trips verbatim.
    let spec = dec_des(el);
    let out = encode(StyleValueDialect::Designer, "", "Value", &spec).unwrap();
    let bytes = String::from_utf8(render(&des_env(), &out)).unwrap();
    assert_eq!(bytes, format!("{}\n{el}", des_env().decl), "des rt");
}
/// Структурный R (EDT): encode(decode(x)) → re-parse → decode == первый spec. (Точная
/// байтовость EDT-вложенного `<value>` проверяется корпусным R-гейтом харнесса.)
fn rt_edt_struct(inner: &str) {
    let s1 = dec_edt(inner);
    let out = encode(StyleValueDialect::Edt, "", "value", &s1).unwrap();
    let bytes = render(&edt_env(), &out);
    // Rendered bytes carry the outer `<value>` host directly as the document root.
    let host = parse(&bytes).expect("re-parse").root;
    let s2 = spec_of(decode(StyleValueDialect::Edt, &host).unwrap());
    assert_eq!(s1, s2, "edt structural round-trip");
}
#[test]
fn edt_structural_roundtrip() {
    rt_edt_struct(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>Style.NormalTextFont</font><height>8.0</height><bold>false</bold>\
         <italic>false</italic><underline>true</underline><strikeout>false</strikeout></value></value>",
    );
    // Частичные тристейт-флаги + FontDef c true-флагом (witnessed ERP).
    rt_edt_struct(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>Style.NormalTextFont</font><bold>true</bold><underline>true</underline></value></value>",
    );
    rt_edt_struct(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontDef\">\
         <faceName>Arial</faceName><height>8.0</height><italic>true</italic></value></value>",
    );
    rt_edt_struct(
        "<value xsi:type=\"core:ColorValue\"><value xsi:type=\"core:ColorDef\">\
         <red>128</red><green>122</green><blue>89</blue></value></value>",
    );
    rt_edt_struct(
        "<value xsi:type=\"core:ColorValue\"><value xsi:type=\"core:ColorRef\">\
         <color>Web.Red</color></value></value>",
    );
    rt_edt_struct(
        "<value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontRef\">\
         <font>System.DefaultGUIFont</font><height>8.0</height><bold>true</bold>\
         <italic>false</italic><underline>false</underline><strikeout>false</strikeout></value></value>",
    );
}
#[test]
fn edt_border_structural_roundtrip() {
    rt_edt_struct(
        "<value xsi:type=\"core:BorderValue\"><value xsi:type=\"core:BorderDef\"/></value>",
    );
}
#[test]
fn designer_border_structural_roundtrip() {
    // Border host — ветка с листом `<v8ui:style>` (не verbatim-однострочник): проверяем
    // структурный R (decode→encode→re-parse→decode == исходный spec).
    let el = "<Value xsi:type=\"v8ui:Border\" width=\"0\">\
         <v8ui:style xsi:type=\"v8ui:ControlBorderType\">WithoutBorder</v8ui:style></Value>";
    let s1 = dec_des(el);
    let out = encode(StyleValueDialect::Designer, "", "Value", &s1).unwrap();
    let bytes = render(&des_env(), &out);
    // Rendered bytes carry `<Value>` directly as the document root (as in rt_edt_struct).
    let host = parse(&bytes).expect("re-parse").root;
    let s2 = spec_of(decode(StyleValueDialect::Designer, &host).unwrap());
    assert_eq!(s1, s2, "designer border structural round-trip");
    assert_eq!(
        s1,
        StyleValueSpec::Border(BorderStyle::Def {
            style: "WithoutBorder".into(),
            width: 0
        })
    );
}
#[test]
fn designer_roundtrip_verbatim() {
    rt_des("<Value xsi:type=\"v8ui:Color\">#B22222</Value>");
    rt_des("<Value xsi:type=\"v8ui:Color\">pal:Red</Value>");
    rt_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"style:TextLevel3\" bold=\"false\" italic=\"false\" \
         underline=\"false\" strikeout=\"false\" kind=\"StyleItem\" scale=\"80\"/>",
    );
    rt_des(
        "<Value xsi:type=\"v8ui:Font\" faceName=\"Arial\" height=\"20\" bold=\"false\" \
         italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"100\"/>",
    );
    // Системный шрифт (kind=WindowsFont) — обе witnessed-формы ERP (голая и height+флаги).
    rt_des("<Value xsi:type=\"v8ui:Font\" ref=\"sys:DefaultGUIFont\" kind=\"WindowsFont\"/>");
    rt_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"sys:DefaultGUIFont\" height=\"10\" bold=\"true\" \
         italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"WindowsFont\"/>",
    );
    // Частичные тристейт-флаги (witnessed ERP `ЖирныйПодчеркнутыйШрифт`/`ПодчеркнутыйШрифт`).
    rt_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"style:NormalTextFont\" bold=\"true\" \
         underline=\"true\" kind=\"StyleItem\"/>",
    );
    rt_des(
        "<Value xsi:type=\"v8ui:Font\" ref=\"style:NormalTextFont\" underline=\"true\" \
         kind=\"StyleItem\"/>",
    );
    // Absolute с true-флагом (witnessed ERP `ЖирныйШрифтEDI`).
    rt_des(
        "<Value xsi:type=\"v8ui:Font\" faceName=\"Arial\" height=\"10\" bold=\"true\" \
         italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"100\"/>",
    );
}

// --- §1.0 adversarial ---
#[test]
fn unknown_edt_inner_errors() {
    let err = decode(
        StyleValueDialect::Edt,
        &host_of(
            "<w><value xsi:type=\"core:FontValue\"><value xsi:type=\"core:FontThing\">\
             <font>Style.X</font></value></value></w>",
        ),
    )
    .unwrap_err();
    assert!(err.contains("unknown EDT inner Font xsi:type"), "{err}");
}
#[test]
fn unknown_designer_xsi_errors() {
    // `v8ui:Gradient` не засвидетельствован (Font/Color/Border покрыты) → §1.0-ошибка.
    let err = decode(
        StyleValueDialect::Designer,
        &host_of("<w><Value xsi:type=\"v8ui:Gradient\"/></w>"),
    )
    .unwrap_err();
    assert!(err.contains("unknown Designer xsi:type"), "{err}");
}
#[test]
fn edt_extra_child_errors() {
    let err = decode(
        StyleValueDialect::Edt,
        &host_of(
            "<w><value xsi:type=\"core:ColorValue\"><value xsi:type=\"core:ColorRef\">\
             <color>Web.Red</color><bogus>x</bogus></value></value></w>",
        ),
    )
    .unwrap_err();
    assert!(err.contains("unexpected extra child"), "{err}");
}
#[test]
fn designer_font_def_nondefault_scale_errors() {
    // scale != 100 у Absolute-шрифта НЕ витнессирован (51/51 ERP несут 100) → отказ.
    let err = decode(
        StyleValueDialect::Designer,
        &host_of(
            "<w><Value xsi:type=\"v8ui:Font\" faceName=\"Arial\" height=\"14\" bold=\"true\" \
             italic=\"false\" underline=\"false\" strikeout=\"false\" kind=\"Absolute\" scale=\"90\"/></w>",
        ),
    )
    .unwrap_err();
    assert!(err.contains("scale must be 100"), "{err}");
}
#[test]
fn designer_color_bad_hex_errors() {
    let err = decode(
        StyleValueDialect::Designer,
        &host_of("<w><Value xsi:type=\"v8ui:Color\">#12</Value></w>"),
    )
    .unwrap_err();
    assert!(err.contains("must be #RRGGBB"), "{err}");
}
#[test]
fn designer_extra_attr_errors() {
    let err = decode(
        StyleValueDialect::Designer,
        &host_of("<w><Value xsi:type=\"v8ui:Font\" ref=\"style:X\" kind=\"StyleItem\" bogus=\"1\"/></w>"),
    )
    .unwrap_err();
    assert!(err.contains("unexpected attribute"), "{err}");
}
