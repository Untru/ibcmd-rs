use super::*;
use super::tests::{designer_env, edt_env, host_of, read_emit};
use crate::emit::render;

// --- CANON-GAP additions (TypeDescription / ReportObject / BusinessProcessRoutePointRef) ---

#[test]
fn type_description_platform_bare_x_equal_and_byte_exact() {
    // EDT bare `TypeDescription` ↔ Designer `v8:TypeDescription` (platform-bare).
    let edt = host_of("<wrap><type>\n  <types>TypeDescription</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>v8:TypeDescription</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "TypeDescription");
    assert_eq!(a, b); // X by construction.
                      // Хостится `<v8:Type>` (не type-set); QName round-trips.
    assert!(!is_type_set("TypeDescription"));
    assert_eq!(
        qname_to_canon("v8:TypeDescription").as_deref(),
        Some("TypeDescription")
    );
    assert_eq!(
        canon_to_qname("TypeDescription").as_deref(),
        Some("v8:TypeDescription")
    );
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>TypeDescription</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>TypeDescription</types>\n</type>",
            edt_env().decl
        )
    );
    let (_, out_d) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>v8:TypeDescription</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>v8:TypeDescription</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn report_object_bare_and_concrete_host_as_v8_type() {
    // BARE `ReportObject` — cfg-адресуем, но хостится `<v8:Type>` (НЕ TypeSet).
    assert!(is_known_ref("ReportObject"));
    assert!(!is_type_set("ReportObject"));
    let edt = host_of("<wrap><type>\n  <types>ReportObject</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>cfg:ReportObject</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "ReportObject");
    assert_eq!(a, b);
    // Designer emit bare → `<v8:Type>cfg:ReportObject` (byte-exact, НЕ TypeSet).
    let (_, out) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>cfg:ReportObject</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out,
        format!(
            "{}\n<Type>\n\t<v8:Type>cfg:ReportObject</v8:Type>\n</Type>",
            designer_env().decl
        )
    );

    // CONCRETE `ReportObject.Имя` — тоже `<v8:Type>`.
    assert!(!is_type_set("ReportObject.АнализОпроса"));
    let edt2 =
        host_of("<wrap><type>\n  <types>ReportObject.АнализОпроса</types>\n</type></wrap>");
    let des2 = host_of(
        "<wrap><Type>\n\t<v8:Type>cfg:ReportObject.АнализОпроса</v8:Type>\n</Type></wrap>",
    );
    let a2 = match decode(TypeDialect::Edt, &edt2).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b2 = match decode(TypeDialect::Designer, &des2).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a2.parts[0].id, "ReportObject.АнализОпроса");
    assert_eq!(a2, b2);
}

#[test]
fn business_process_route_point_ref_concrete_x_equal_and_byte_exact() {
    // CONCRETE `BusinessProcessRoutePointRef.<БП>` ↔ `<v8:Type>cfg:…` (не type-set).
    assert!(is_known_ref(
        "BusinessProcessRoutePointRef.СогласованиеПродажи"
    ));
    assert!(!is_type_set(
        "BusinessProcessRoutePointRef.СогласованиеПродажи"
    ));
    let edt = host_of("<wrap><type>\n  <types>BusinessProcessRoutePointRef.СогласованиеПродажи</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>cfg:BusinessProcessRoutePointRef.СогласованиеПродажи</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(
        a.parts[0].id,
        "BusinessProcessRoutePointRef.СогласованиеПродажи"
    );
    assert_eq!(a, b);
    // EDT byte-exact.
    let (_, out) = read_emit(TypeDialect::Edt, &edt_env(), "type", "", "<wrap><type>\n  <types>BusinessProcessRoutePointRef.СогласованиеПродажи</types>\n</type></wrap>");
    assert_eq!(out, format!("{}\n<type>\n  <types>BusinessProcessRoutePointRef.СогласованиеПродажи</types>\n</type>", edt_env().decl));
}

#[test]
fn picture_platform_alias_x_equal_and_byte_exact() {
    // EDT bare `Picture` ↔ Designer `v8ui:Picture` (v8ui-alias, host `<v8:Type>`).
    // Witnessed: ERP EDT `<valueType><types>Picture</types>` ↔ Designer
    // `<v8:Type>v8ui:Picture</v8:Type>` (см. PLATFORM_ALIASES).
    let edt = host_of("<wrap><type>\n  <types>Picture</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>v8ui:Picture</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "Picture");
    assert_eq!(a, b); // X by construction.
                      // Хостится `<v8:Type>` (не type-set); QName round-trips обе стороны.
    assert!(!is_type_set("Picture"));
    assert_eq!(qname_to_canon("v8ui:Picture").as_deref(), Some("Picture"));
    assert_eq!(canon_to_qname("Picture").as_deref(), Some("v8ui:Picture"));
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>Picture</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>Picture</types>\n</type>",
            edt_env().decl
        )
    );
    let (_, out_d) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>v8ui:Picture</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>v8ui:Picture</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn font_platform_alias_x_equal_and_byte_exact() {
    // EDT bare `Font` ↔ Designer `v8ui:Font` (v8ui-alias, envelope-declared, host `<v8:Type>`).
    // Witnessed: ERP EDT `<valueType><types>Font</types>` (CommonForms/НастройкаКолонтитулов)
    // ↔ Designer `<v8:Type>v8ui:Font</v8:Type>` (тот же паттерн, что `Picture`; см. PLATFORM_ALIASES).
    let edt = host_of("<wrap><type>\n  <types>Font</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>v8ui:Font</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "Font");
    assert_eq!(a, b); // X by construction: EDT-decoded IR == Designer-decoded IR.
                      // Хостится `<v8:Type>` (не type-set); QName round-trips обе стороны.
    assert!(!is_type_set("Font"));
    assert_eq!(qname_to_canon("v8ui:Font").as_deref(), Some("Font"));
    assert_eq!(canon_to_qname("Font").as_deref(), Some("v8ui:Font"));
    // Font envelope-declared (не инлайн) — как Picture: НЕ в INLINE_NS_QNAMES.
    assert_eq!(inline_ns_for_qname("v8ui:Font"), None);
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>Font</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!("{}\n<type>\n  <types>Font</types>\n</type>", edt_env().decl)
    );
    let (_, out_d) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>v8ui:Font</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>v8ui:Font</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn unknown_font_like_type_id_still_errors() {
    // §1.0: неизвестный QName (`v8ui:Fnt` — опечатка, `v8:Font` — неверный префикс: `Font`
    // НЕ в PLATFORM_BARE) обязан ЭРРОРИТЬ через движок, НЕ silent-drop / НЕ подхват `Font`.
    let h1 = host_of("<wrap><Type>\n\t<v8:Type>v8ui:Fnt</v8:Type>\n</Type></wrap>");
    let e1 = decode(TypeDialect::Designer, &h1).unwrap_err();
    assert!(e1.contains("unknown Designer QName"), "got: {e1}");
    let h2 = host_of("<wrap><Type>\n\t<v8:Type>v8:Font</v8:Type>\n</Type></wrap>");
    let e2 = decode(TypeDialect::Designer, &h2).unwrap_err();
    assert!(e2.contains("unknown Designer QName"), "got: {e2}");
    // EDT-ветка: близкий-но-неизвестный bare id (`Fonts`) тоже эррорит (не подхват `Font`).
    let he = host_of("<wrap><type>\n  <types>Fonts</types>\n</type></wrap>");
    let ee = decode(TypeDialect::Edt, &he).unwrap_err();
    assert!(ee.contains("unknown EDT type-id"), "got: {ee}");
}

#[test]
fn spreadsheet_document_inline_ns_alias_x_equal_and_byte_exact() {
    // EDT bare `SpreadsheetDocument` ↔ Designer `mxl:SpreadsheetDocument` с ИНЛАЙН-ns
    // `xmlns:mxl` на `<v8:Type>` (host `<v8:Type>`, не type-set). Witnessed: SSL edt
    // CommonForms/ОписаниеИзмененийПрограммы `<types>SpreadsheetDocument</types>` ↔ SSL
    // designer_8.5.1 `<v8:Type xmlns:mxl="http://v8.1c.ru/8.2/data/spreadsheet">
    // mxl:SpreadsheetDocument</v8:Type>` (см. PLATFORM_ALIASES/INLINE_NS_QNAMES).
    let edt = host_of("<wrap><type>\n  <types>SpreadsheetDocument</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type xmlns:mxl=\"http://v8.1c.ru/8.2/data/spreadsheet\">mxl:SpreadsheetDocument</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "SpreadsheetDocument");
    assert_eq!(a, b); // X by construction.
                      // Хостится `<v8:Type>` (не type-set); QName round-trips обе стороны.
    assert!(!is_type_set("SpreadsheetDocument"));
    assert_eq!(
        qname_to_canon("mxl:SpreadsheetDocument").as_deref(),
        Some("SpreadsheetDocument")
    );
    assert_eq!(
        canon_to_qname("SpreadsheetDocument").as_deref(),
        Some("mxl:SpreadsheetDocument")
    );
    assert_eq!(
        inline_ns_for_qname("mxl:SpreadsheetDocument"),
        Some(("xmlns:mxl", "http://v8.1c.ru/8.2/data/spreadsheet"))
    );
    // Byte-exact обе проекции (Designer восстанавливает инлайн-`xmlns:mxl`).
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>SpreadsheetDocument</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>SpreadsheetDocument</types>\n</type>",
            edt_env().decl
        )
    );
    let des_frag = "<wrap><Type>\n\t<v8:Type xmlns:mxl=\"http://v8.1c.ru/8.2/data/spreadsheet\">mxl:SpreadsheetDocument</v8:Type>\n</Type></wrap>";
    let (_, out_d) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", des_frag);
    assert_eq!(out_d, format!("{}\n<Type>\n\t<v8:Type xmlns:mxl=\"http://v8.1c.ru/8.2/data/spreadsheet\">mxl:SpreadsheetDocument</v8:Type>\n</Type>", designer_env().decl));
}

#[test]
fn unknown_spreadsheet_like_type_id_still_errors() {
    // §1.0: близкий-но-неизвестный id (`SpreadsheetDocumentField` — это КОНТРОЛ, не
    // value-type) обязан ЭРРОРИТЬ через движок, а не подхватываться `SpreadsheetDocument`.
    let h = host_of("<wrap><type>\n  <types>SpreadsheetDocumentField</types>\n</type></wrap>");
    let e = decode(TypeDialect::Edt, &h).unwrap_err();
    assert!(e.contains("unknown EDT type-id"), "got: {e}");
    // Designer-ветка: `mxl:SpreadsheetDocument` без ИНЛАЙН-ns или иной QName (`v8:` вместо
    // `mxl:`) тоже эррорит (`v8:SpreadsheetDocument` не в PLATFORM_BARE).
    let hd =
        host_of("<wrap><Type>\n\t<v8:Type>v8:SpreadsheetDocument</v8:Type>\n</Type></wrap>");
    let ed = decode(TypeDialect::Designer, &hd).unwrap_err();
    assert!(ed.contains("unknown Designer QName"), "got: {ed}");
}

#[test]
fn formatted_document_inline_ns_alias_x_equal_and_byte_exact() {
    // EDT bare `FormattedDocument` ↔ Designer `fd:FormattedDocument` с ИНЛАЙН-ns
    // `xmlns:fd` на `<v8:Type>` (host `<v8:Type>`, не type-set). Witnessed: ERP edt
    // CommonForms/ВосстановлениеПаролей `<valueType><types>FormattedDocument</types>` ↔ ERP
    // designer_8.3.27 `<v8:Type xmlns:fd="http://v8.1c.ru/8.2/data/formatted-document">
    // fd:FormattedDocument</v8:Type>` (55/55 byte-identical по ERP+SSL; см. PLATFORM_ALIASES/
    // INLINE_NS_QNAMES).
    let edt = host_of("<wrap><type>\n  <types>FormattedDocument</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type xmlns:fd=\"http://v8.1c.ru/8.2/data/formatted-document\">fd:FormattedDocument</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "FormattedDocument");
    assert_eq!(a, b); // X by construction.
                      // Хостится `<v8:Type>` (не type-set); QName round-trips обе стороны.
    assert!(!is_type_set("FormattedDocument"));
    assert_eq!(
        qname_to_canon("fd:FormattedDocument").as_deref(),
        Some("FormattedDocument")
    );
    assert_eq!(
        canon_to_qname("FormattedDocument").as_deref(),
        Some("fd:FormattedDocument")
    );
    assert_eq!(
        inline_ns_for_qname("fd:FormattedDocument"),
        Some(("xmlns:fd", "http://v8.1c.ru/8.2/data/formatted-document"))
    );
    // Byte-exact обе проекции (Designer восстанавливает инлайн-`xmlns:fd`).
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>FormattedDocument</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>FormattedDocument</types>\n</type>",
            edt_env().decl
        )
    );
    let des_frag = "<wrap><Type>\n\t<v8:Type xmlns:fd=\"http://v8.1c.ru/8.2/data/formatted-document\">fd:FormattedDocument</v8:Type>\n</Type></wrap>";
    let (_, out_d) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", des_frag);
    assert_eq!(out_d, format!("{}\n<Type>\n\t<v8:Type xmlns:fd=\"http://v8.1c.ru/8.2/data/formatted-document\">fd:FormattedDocument</v8:Type>\n</Type>", designer_env().decl));
}

#[test]
fn text_document_inline_ns_alias_x_equal_and_byte_exact() {
    // EDT bare `TextDocument` ↔ Designer `d5p1:TextDocument` с ИНЛАЙН-ns
    // `xmlns:d5p1` на `<v8:Type>` (host `<v8:Type>`, не type-set). Witnessed: SSL edt
    // Catalogs/ШаблоныСообщений/Forms/ФормаЭлемента `<valueType><types>TextDocument</types>` ↔
    // SSL designer_8.5.1 `<v8:Type xmlns:d5p1="http://v8.1c.ru/8.1/data/txtedt">
    // d5p1:TextDocument</v8:Type>` (4/4 byte-identical по SSL; см. PLATFORM_ALIASES/INLINE_NS_QNAMES).
    let edt = host_of("<wrap><type>\n  <types>TextDocument</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.1/data/txtedt\">d5p1:TextDocument</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "TextDocument");
    assert_eq!(a, b); // X by construction.
                      // Хостится `<v8:Type>` (не type-set); QName round-trips обе стороны.
    assert!(!is_type_set("TextDocument"));
    assert_eq!(
        qname_to_canon("d5p1:TextDocument").as_deref(),
        Some("TextDocument")
    );
    assert_eq!(
        canon_to_qname("TextDocument").as_deref(),
        Some("d5p1:TextDocument")
    );
    assert_eq!(
        inline_ns_for_qname("d5p1:TextDocument"),
        Some(("xmlns:d5p1", "http://v8.1c.ru/8.1/data/txtedt"))
    );
    // Byte-exact обе проекции (Designer восстанавливает инлайн-`xmlns:d5p1`).
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>TextDocument</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>TextDocument</types>\n</type>",
            edt_env().decl
        )
    );
    let des_frag = "<wrap><Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.1/data/txtedt\">d5p1:TextDocument</v8:Type>\n</Type></wrap>";
    let (_, out_d) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", des_frag);
    assert_eq!(out_d, format!("{}\n<Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.1/data/txtedt\">d5p1:TextDocument</v8:Type>\n</Type>", designer_env().decl));
}

#[test]
fn unknown_formatted_document_like_type_id_still_errors() {
    // §1.0: близкий-но-неизвестный id (`FormattedDocumentField` — это КОНТРОЛ, не
    // value-type) обязан ЭРРОРИТЬ через движок, а не подхватываться `FormattedDocument`.
    let h = host_of("<wrap><type>\n  <types>FormattedDocumentField</types>\n</type></wrap>");
    let e = decode(TypeDialect::Edt, &h).unwrap_err();
    assert!(e.contains("unknown EDT type-id"), "got: {e}");
    // Designer-ветка: `fd:FormattedDocument` без ИНЛАЙН-ns / иной QName (`v8:` вместо `fd:`)
    // тоже эррорит (`v8:FormattedDocument` не в PLATFORM_BARE).
    let hd = host_of("<wrap><Type>\n\t<v8:Type>v8:FormattedDocument</v8:Type>\n</Type></wrap>");
    let ed = decode(TypeDialect::Designer, &hd).unwrap_err();
    assert!(ed.contains("unknown Designer QName"), "got: {ed}");
}

#[test]
fn unknown_picture_like_type_id_still_errors() {
    // §1.0: близкий-но-неизвестный id (`Pictures`) обязан ЭРРОРИТЬ через движок,
    // а не подхватываться `Picture`. EDT-ветка.
    let h = host_of("<wrap><type>\n  <types>Pictures</types>\n</type></wrap>");
    let e = decode(TypeDialect::Edt, &h).unwrap_err();
    assert!(e.contains("unknown EDT type-id"), "got: {e}");
    // Designer-ветка: неизвестный QName (не `v8ui:Picture`) тоже эррорит.
    let hd = host_of("<wrap><Type>\n\t<v8:Type>v8ui:Pictures</v8:Type>\n</Type></wrap>");
    let ed = decode(TypeDialect::Designer, &hd).unwrap_err();
    assert!(ed.contains("unknown Designer QName"), "got: {ed}");
}

#[test]
fn data_composition_sort_direction_platform_alias_x_equal_and_byte_exact() {
    // EDT bare `DataCompositionSortDirection` ↔ Designer `dcscor:DataCompositionSortDirection`
    // (dcscor-alias, ENVELOPE-declared — как Font/Picture, НЕ инлайн; host `<v8:Type>`).
    // Witnessed SSL+ERP: EDT `<valueType><types>DataCompositionSortDirection</types>`
    // (CommonForms/ФормаНастроекОтчета Form.form:10364/12198) ↔ Designer
    // `<v8:Type>dcscor:DataCompositionSortDirection</v8:Type>` (…/Ext/Form.xml:4345/5724,
    // `xmlns:dcscor` в корне-envelope; см. PLATFORM_ALIASES).
    let edt =
        host_of("<wrap><type>\n  <types>DataCompositionSortDirection</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>dcscor:DataCompositionSortDirection</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "DataCompositionSortDirection");
    assert_eq!(a, b); // X by construction: EDT-decoded IR == Designer-decoded IR.
                      // Хостится `<v8:Type>` (не type-set); QName round-trips обе стороны.
    assert!(!is_type_set("DataCompositionSortDirection"));
    assert_eq!(
        qname_to_canon("dcscor:DataCompositionSortDirection").as_deref(),
        Some("DataCompositionSortDirection")
    );
    assert_eq!(
        canon_to_qname("DataCompositionSortDirection").as_deref(),
        Some("dcscor:DataCompositionSortDirection")
    );
    // Envelope-declared (не инлайн) — как Font: НЕ в INLINE_NS_QNAMES.
    assert_eq!(
        inline_ns_for_qname("dcscor:DataCompositionSortDirection"),
        None
    );
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>DataCompositionSortDirection</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>DataCompositionSortDirection</types>\n</type>",
            edt_env().decl
        )
    );
    let (_, out_d) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", "<wrap><Type>\n\t<v8:Type>dcscor:DataCompositionSortDirection</v8:Type>\n</Type></wrap>");
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>dcscor:DataCompositionSortDirection</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn unknown_data_composition_sort_direction_like_type_id_still_errors() {
    // §1.0: искажённый/близкий id обязан ЭРРОРИТЬ через движок, НЕ silent-drop / НЕ подхват.
    // Designer: неверный префикс (`dcsset:` вместо `dcscor:` — это ДРУГОЙ ns) и опечатка QName.
    let h1 = host_of("<wrap><Type>\n\t<v8:Type>dcsset:DataCompositionSortDirection</v8:Type>\n</Type></wrap>");
    let e1 = decode(TypeDialect::Designer, &h1).unwrap_err();
    assert!(e1.contains("unknown Designer QName"), "got: {e1}");
    let h2 = host_of("<wrap><Type>\n\t<v8:Type>dcscor:DataCompositionSortDirections</v8:Type>\n</Type></wrap>");
    let e2 = decode(TypeDialect::Designer, &h2).unwrap_err();
    assert!(e2.contains("unknown Designer QName"), "got: {e2}");
    // `v8:`-префикс тоже неверен (`DataCompositionSortDirection` не в PLATFORM_BARE).
    let h3 = host_of(
        "<wrap><Type>\n\t<v8:Type>v8:DataCompositionSortDirection</v8:Type>\n</Type></wrap>",
    );
    let e3 = decode(TypeDialect::Designer, &h3).unwrap_err();
    assert!(e3.contains("unknown Designer QName"), "got: {e3}");
    // EDT-ветка: близкий-но-неизвестный bare id (`DataCompositionSortDirections`) тоже эррорит.
    let he =
        host_of("<wrap><type>\n  <types>DataCompositionSortDirections</types>\n</type></wrap>");
    let ee = decode(TypeDialect::Edt, &he).unwrap_err();
    assert!(ee.contains("unknown EDT type-id"), "got: {ee}");
}

#[test]
fn dynamic_list_platform_alias_x_equal_and_byte_exact() {
    // EDT bare `DynamicList` ↔ Designer `cfg:DynamicList` (cfg-alias, ENVELOPE-declared —
    // как ConstantsSet/ReportBuilder, НЕ инлайн; host `<v8:Type>`). Witnessed SSL: EDT
    // `<valueType><types>DynamicList</types>` (CommonForms/ВыборКонтакта Form.form:1541) ↔
    // Designer `<v8:Type>cfg:DynamicList</v8:Type>` (…/Ext/Form.xml:715, `xmlns:cfg` в
    // корне-envelope; см. PLATFORM_ALIASES).
    let edt = host_of("<wrap><type>\n  <types>DynamicList</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>cfg:DynamicList</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "DynamicList");
    assert_eq!(a, b); // X by construction: EDT-decoded IR == Designer-decoded IR.
                      // Хостится `<v8:Type>` (не type-set — bare `DynamicList` НЕ ref-kind); QName round-trips.
    assert!(!is_type_set("DynamicList"));
    assert_eq!(
        qname_to_canon("cfg:DynamicList").as_deref(),
        Some("DynamicList")
    );
    assert_eq!(
        canon_to_qname("DynamicList").as_deref(),
        Some("cfg:DynamicList")
    );
    // Envelope-declared (не инлайн) — как ConstantsSet: НЕ в INLINE_NS_QNAMES.
    assert_eq!(inline_ns_for_qname("cfg:DynamicList"), None);
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>DynamicList</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>DynamicList</types>\n</type>",
            edt_env().decl
        )
    );
    let (_, out_d) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>cfg:DynamicList</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>cfg:DynamicList</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn unknown_dynamic_list_like_type_id_still_errors() {
    // §1.0: искажённый/близкий id обязан ЭРРОРИТЬ, НЕ silent-drop / НЕ подхват.
    // Designer: неверный префикс (`v8:` — `DynamicList` не в PLATFORM_BARE) и опечатка QName.
    let h1 = host_of("<wrap><Type>\n\t<v8:Type>v8:DynamicList</v8:Type>\n</Type></wrap>");
    let e1 = decode(TypeDialect::Designer, &h1).unwrap_err();
    assert!(e1.contains("unknown Designer QName"), "got: {e1}");
    let h2 = host_of("<wrap><Type>\n\t<v8:Type>cfg:DynamicLists</v8:Type>\n</Type></wrap>");
    let e2 = decode(TypeDialect::Designer, &h2).unwrap_err();
    assert!(e2.contains("unknown Designer QName"), "got: {e2}");
    // EDT-ветка: близкий-но-неизвестный bare id (`DynamicLists`) тоже эррорит.
    let he = host_of("<wrap><type>\n  <types>DynamicLists</types>\n</type></wrap>");
    let ee = decode(TypeDialect::Edt, &he).unwrap_err();
    assert!(ee.contains("unknown EDT type-id"), "got: {ee}");
}

#[test]
fn information_register_record_manager_concrete_x_equal_and_byte_exact() {
    // CONCRETE `InformationRegisterRecordManager.<Имя>` ↔ `<v8:Type>cfg:…` (не type-set).
    // Witnessed SSL: EDT `<types>InformationRegisterRecordManager.ХранилищеФайлов</types>`
    // (InformationRegisters/ХранилищеФайлов/Forms/ФормаЗаписи/Form.form:140) ↔ Designer
    // `<v8:Type>cfg:InformationRegisterRecordManager.ХранилищеФайлов</v8:Type>` (…/Ext/Form.xml:43).
    assert!(is_known_ref(
        "InformationRegisterRecordManager.ХранилищеФайлов"
    ));
    assert!(!is_type_set(
        "InformationRegisterRecordManager.ХранилищеФайлов"
    ));
    let edt = host_of("<wrap><type>\n  <types>InformationRegisterRecordManager.ХранилищеФайлов</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>cfg:InformationRegisterRecordManager.ХранилищеФайлов</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(
        a.parts[0].id,
        "InformationRegisterRecordManager.ХранилищеФайлов"
    );
    assert_eq!(a, b); // X by construction.
                      // Byte-exact обе проекции.
    let (_, out_e) = read_emit(TypeDialect::Edt, &edt_env(), "type", "", "<wrap><type>\n  <types>InformationRegisterRecordManager.ХранилищеФайлов</types>\n</type></wrap>");
    assert_eq!(out_e, format!("{}\n<type>\n  <types>InformationRegisterRecordManager.ХранилищеФайлов</types>\n</type>", edt_env().decl));
    let (_, out_d) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", "<wrap><Type>\n\t<v8:Type>cfg:InformationRegisterRecordManager.ХранилищеФайлов</v8:Type>\n</Type></wrap>");
    assert_eq!(out_d, format!("{}\n<Type>\n\t<v8:Type>cfg:InformationRegisterRecordManager.ХранилищеФайлов</v8:Type>\n</Type>", designer_env().decl));
}

#[test]
fn data_composition_filter_alias_x_equal_and_byte_exact() {
    // EDT bare `DataCompositionFilter` ↔ Designer `dcsset:Filter` (local-name РАЗНЫЙ,
    // dcsset-alias, ENVELOPE-declared в форме — НЕ инлайн; host `<v8:Type>`). Witnessed SSL:
    // EDT `<valueType><types>DataCompositionFilter</types>` (DataProcessors/
    // ИнтерактивноеИзменениеВыгрузки/Forms/СоставВыгрузки/Form.form:422) ↔ Designer
    // `<v8:Type>dcsset:Filter</v8:Type>` (…/Ext/Form.xml:182).
    let edt = host_of("<wrap><type>\n  <types>DataCompositionFilter</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>dcsset:Filter</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "DataCompositionFilter");
    assert_eq!(a, b); // X by construction.
    assert!(!is_type_set("DataCompositionFilter"));
    assert_eq!(
        qname_to_canon("dcsset:Filter").as_deref(),
        Some("DataCompositionFilter")
    );
    assert_eq!(
        canon_to_qname("DataCompositionFilter").as_deref(),
        Some("dcsset:Filter")
    );
    // Envelope-declared (не инлайн): НЕ в INLINE_NS_QNAMES.
    assert_eq!(inline_ns_for_qname("dcsset:Filter"), None);
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>DataCompositionFilter</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>DataCompositionFilter</types>\n</type>",
            edt_env().decl
        )
    );
    let (_, out_d) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>dcsset:Filter</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>dcsset:Filter</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn fill_checking_platform_bare_x_equal_and_byte_exact() {
    // EDT bare `FillChecking` ↔ Designer `v8:FillChecking` (platform-bare, механический `v8:`).
    // Witnessed SSL: EDT `<valueType><types>FillChecking</types>` (DataProcessors/
    // СкрытиеКонфиденциальнойИнформации/Forms/Форма/Form.form:2319) ↔ Designer
    // `<v8:Type>v8:FillChecking</v8:Type>` (…/Ext/Form.xml:1167).
    let edt = host_of("<wrap><type>\n  <types>FillChecking</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>v8:FillChecking</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "FillChecking");
    assert_eq!(a, b); // X by construction.
    assert!(!is_type_set("FillChecking"));
    assert_eq!(
        qname_to_canon("v8:FillChecking").as_deref(),
        Some("FillChecking")
    );
    assert_eq!(
        canon_to_qname("FillChecking").as_deref(),
        Some("v8:FillChecking")
    );
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>FillChecking</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>FillChecking</types>\n</type>",
            edt_env().decl
        )
    );
    let (_, out_d) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>v8:FillChecking</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>v8:FillChecking</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn gantt_chart_auto_ns_x_equal_and_byte_exact() {
    // «Диаграмма Ганта» — АВТО-NS тип (как `Chart`, ТА ЖЕ uri data/chart, иной local-name),
    // value-type реквизита формы (backing GanttChartField). Designer-префикс авто-`d{N}p1`;
    // ERP даёт ТОЛЬКО глубину 5 (форма) → `d5p1:GanttChart`. Witnessed (DataProcessor
    // ДиспетчированиеПроизводства, 17/17 d5p1): EDT `<types>GanttChart</types>` ↔ Designer
    // `<v8:Type xmlns:d5p1="http://v8.1c.ru/8.2/data/chart">d5p1:GanttChart</v8:Type>`.
    let edt = host_of("<wrap><type>\n  <types>GanttChart</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.2/data/chart\">d5p1:GanttChart</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "GanttChart");
    assert_eq!(a, b); // X by construction.
    assert!(!is_type_set("GanttChart"));
    // §1.0: чужой uri на инлайне = другой одноимённый тип → ошибка (не guess).
    let bad = host_of("<wrap><Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.2/data/geo\">d5p1:GanttChart</v8:Type>\n</Type></wrap>");
    assert!(decode(TypeDialect::Designer, &bad)
        .unwrap_err()
        .contains("different type"));
    // Byte-exact EDT.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>GanttChart</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>GanttChart</types>\n</type>",
            edt_env().decl
        )
    );
    // Byte-exact Designer: непустой envelope (форма) → глубина 5 → `d5p1` + инлайн-ns.
    let out = encode_scoped(
        TypeDialect::Designer,
        "",
        "Type",
        &b,
        &[("xmlns:v8", "http://v8.1c.ru/8.1/data/core")],
    )
    .unwrap();
    let out_d = String::from_utf8(render(&designer_env(), &out)).unwrap();
    assert_eq!(out_d, format!("{}\n<Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.2/data/chart\">d5p1:GanttChart</v8:Type>\n</Type>", designer_env().decl));
}

#[test]
fn data_analysis_time_interval_unit_type_auto_ns_x_equal_and_byte_exact() {
    // «Единица интервала времени анализа данных» — АВТО-NS тип (как `GanttChart`, но uri
    // data-analysis), value-type реквизита формы (периодичность Gantt-каскада). Глубина 5
    // (форма) → `d5p1`. Witnessed (DataProcessor ДиспетчированиеПроизводстваПооперационное.
    // ДиспетчированиеПроизводства, реквизит Периодичность id 20): EDT
    // `<types>DataAnalysisTimeIntervalUnitType</types>` ↔ Designer `<v8:Type
    // xmlns:d5p1="http://v8.1c.ru/8.2/data/data-analysis">d5p1:DataAnalysisTimeIntervalUnitType</v8:Type>`.
    let edt = host_of(
        "<wrap><type>\n  <types>DataAnalysisTimeIntervalUnitType</types>\n</type></wrap>",
    );
    let des = host_of("<wrap><Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.2/data/data-analysis\">d5p1:DataAnalysisTimeIntervalUnitType</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "DataAnalysisTimeIntervalUnitType");
    assert_eq!(a, b); // X by construction.
    assert!(!is_type_set("DataAnalysisTimeIntervalUnitType"));
    // §1.0: чужой uri на инлайне = другой одноимённый тип → ошибка (не guess).
    let bad = host_of("<wrap><Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.2/data/chart\">d5p1:DataAnalysisTimeIntervalUnitType</v8:Type>\n</Type></wrap>");
    assert!(decode(TypeDialect::Designer, &bad)
        .unwrap_err()
        .contains("different type"));
    // Byte-exact EDT.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>DataAnalysisTimeIntervalUnitType</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>DataAnalysisTimeIntervalUnitType</types>\n</type>",
            edt_env().decl
        )
    );
    // Byte-exact Designer: непустой envelope (форма) → глубина 5 → `d5p1` + инлайн-ns.
    let out = encode_scoped(
        TypeDialect::Designer,
        "",
        "Type",
        &b,
        &[("xmlns:v8", "http://v8.1c.ru/8.1/data/core")],
    )
    .unwrap();
    let out_d = String::from_utf8(render(&designer_env(), &out)).unwrap();
    assert_eq!(out_d, format!("{}\n<Type>\n\t<v8:Type xmlns:d5p1=\"http://v8.1c.ru/8.2/data/data-analysis\">d5p1:DataAnalysisTimeIntervalUnitType</v8:Type>\n</Type>", designer_env().decl));
}

#[test]
fn pdf_document_inline_ns_alias_x_equal_and_byte_exact() {
    // EDT bare `PDFDocument` ↔ Designer `pdfdoc:PDFDocument` с ИНЛАЙН-ns `xmlns:pdfdoc`
    // на `<v8:Type>` (host `<v8:Type>`, не type-set; ФИКС-префикс `pdfdoc:`). Witnessed ERP
    // (6/6): EDT DataProcessors/СервисДоставки/Forms/ПросмотрPDF `<valueType><types>PDFDocument
    // </types>` ↔ Designer `<v8:Type xmlns:pdfdoc="http://v8.1c.ru/8.3/data/pdf">pdfdoc:PDFDocument
    // </v8:Type>` (см. PLATFORM_ALIASES/INLINE_NS_QNAMES).
    let edt = host_of("<wrap><type>\n  <types>PDFDocument</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type xmlns:pdfdoc=\"http://v8.1c.ru/8.3/data/pdf\">pdfdoc:PDFDocument</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "PDFDocument");
    assert_eq!(a, b); // X by construction.
    assert!(!is_type_set("PDFDocument"));
    assert_eq!(
        qname_to_canon("pdfdoc:PDFDocument").as_deref(),
        Some("PDFDocument")
    );
    assert_eq!(
        canon_to_qname("PDFDocument").as_deref(),
        Some("pdfdoc:PDFDocument")
    );
    assert_eq!(
        inline_ns_for_qname("pdfdoc:PDFDocument"),
        Some(("xmlns:pdfdoc", "http://v8.1c.ru/8.3/data/pdf"))
    );
    // Byte-exact обе проекции (Designer восстанавливает инлайн-`xmlns:pdfdoc`).
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>PDFDocument</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>PDFDocument</types>\n</type>",
            edt_env().decl
        )
    );
    let des_frag = "<wrap><Type>\n\t<v8:Type xmlns:pdfdoc=\"http://v8.1c.ru/8.3/data/pdf\">pdfdoc:PDFDocument</v8:Type>\n</Type></wrap>";
    let (_, out_d) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", des_frag);
    assert_eq!(out_d, format!("{}\n<Type>\n\t<v8:Type xmlns:pdfdoc=\"http://v8.1c.ru/8.3/data/pdf\">pdfdoc:PDFDocument</v8:Type>\n</Type>", designer_env().decl));
}

#[test]
fn accounting_record_type_platform_alias_x_equal_and_byte_exact() {
    // EDT bare `AccountingRecordType` ↔ Designer `ent:AccountingRecordType` (ФИКС-префикс
    // `ent:`, ENVELOPE-declared — БЕЗ инлайна, как dcscor/cfg-алиасы; host `<v8:Type>`).
    // Witnessed ERP (8×): EDT Documents/ОперацияМеждународный/Forms/ФормаДокумента
    // `<valueType><types>AccountingRecordType</types>` ↔ Designer
    // `<v8:Type>ent:AccountingRecordType</v8:Type>` (см. PLATFORM_ALIASES).
    let edt =
        host_of("<wrap><type>\n  <types>AccountingRecordType</types>\n</type></wrap>");
    let des =
        host_of("<wrap><Type>\n\t<v8:Type>ent:AccountingRecordType</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "AccountingRecordType");
    assert_eq!(a, b); // X by construction.
    assert!(!is_type_set("AccountingRecordType"));
    // ФИКС-алиас → НЕ инлайн (ent в envelope формы).
    assert_eq!(inline_ns_for_qname("ent:AccountingRecordType"), None);
    assert_eq!(
        qname_to_canon("ent:AccountingRecordType").as_deref(),
        Some("AccountingRecordType")
    );
    assert_eq!(
        canon_to_qname("AccountingRecordType").as_deref(),
        Some("ent:AccountingRecordType")
    );
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>AccountingRecordType</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>AccountingRecordType</types>\n</type>",
            edt_env().decl
        )
    );
    let (_, out_d) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>ent:AccountingRecordType</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>ent:AccountingRecordType</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn type_meta_platform_bare_x_equal_and_byte_exact() {
    // Мета-тип «Тип»: EDT bare `Type` ↔ Designer `v8:Type` (platform-bare, механический `v8:`,
    // ENVELOPE-declared). Witnessed ERP (21×): EDT Catalogs/КлючиРеестраДокументов/Forms/
    // ФормаВыбора `<valueType><types>Type</types>` ↔ Designer `<v8:Type>v8:Type</v8:Type>`.
    let edt = host_of("<wrap><type>\n  <types>Type</types>\n</type></wrap>");
    let des = host_of("<wrap><Type>\n\t<v8:Type>v8:Type</v8:Type>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "Type");
    assert_eq!(a, b); // X by construction.
    assert!(!is_type_set("Type"));
    assert_eq!(qname_to_canon("v8:Type").as_deref(), Some("Type"));
    assert_eq!(canon_to_qname("Type").as_deref(), Some("v8:Type"));
    // Byte-exact обе проекции.
    let (_, out_e) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>Type</types>\n</type></wrap>",
    );
    assert_eq!(
        out_e,
        format!("{}\n<type>\n  <types>Type</types>\n</type>", edt_env().decl)
    );
    let (_, out_d) = read_emit(
        TypeDialect::Designer,
        &designer_env(),
        "Type",
        "",
        "<wrap><Type>\n\t<v8:Type>v8:Type</v8:Type>\n</Type></wrap>",
    );
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:Type>v8:Type</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn v8_type_host_tag_still_wraps_ordinary_types() {
    // РЕГРЕССИЯ: добавление мета-типа `Type`→`v8:Type` НЕ должно сломать служебный host-тег
    // `<v8:Type>`, который оборачивает ЛЮБУЮ компоненту (различие — по ИМЕНИ тега, не тексту).
    // Хост с ДВУМЯ компонентами: обычный concrete-ref `cfg:CatalogRef.Валюты` И мета `v8:Type`
    // → парсятся как [CatalogRef.Валюты, Type], round-trip byte-exact в обоих диалектах.
    let des_frag = "<wrap><Type>\n\t<v8:Type>cfg:CatalogRef.Валюты</v8:Type>\n\t<v8:Type>v8:Type</v8:Type>\n</Type></wrap>";
    let des = host_of(des_frag);
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(b.parts.len(), 2);
    assert_eq!(b.parts[0].id, "CatalogRef.Валюты"); // host-тег обернул обычный тип.
    assert_eq!(b.parts[1].id, "Type"); // текст `v8:Type` = мета-тип «Тип».
    let (_, out_d) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", des_frag);
    assert_eq!(out_d, format!("{}\n<Type>\n\t<v8:Type>cfg:CatalogRef.Валюты</v8:Type>\n\t<v8:Type>v8:Type</v8:Type>\n</Type>", designer_env().decl));
    // EDT-твин того же набора.
    let edt_frag =
        "<wrap><type>\n  <types>CatalogRef.Валюты</types>\n  <types>Type</types>\n</type></wrap>";
    let (_, out_e) = read_emit(TypeDialect::Edt, &edt_env(), "type", "", edt_frag);
    assert_eq!(
        out_e,
        format!(
            "{}\n<type>\n  <types>CatalogRef.Валюты</types>\n  <types>Type</types>\n</type>",
            edt_env().decl
        )
    );
}
