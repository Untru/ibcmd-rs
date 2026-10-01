use super::*;
use crate::emit::{render, Envelope};
use crate::read::parse;

/// Envelope для рендера фрагмента (host как корень) — EDT-стиль (LF, 2 пробела).
pub(crate) fn edt_env() -> Envelope {
    Envelope {
        bom: false,
        eol: "\n",
        indent_unit: "  ",
        decl: "<?xml version=\"1.0\"?>",
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "
",
    }
}
/// Designer-стиль (TAB).
pub(crate) fn designer_env() -> Envelope {
    Envelope {
        bom: false,
        eol: "\n",
        indent_unit: "\t",
        decl: "<?xml version=\"1.0\"?>",
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "
",
    }
}

/// Парс фрагмента `<wrap>HOST</wrap>` и возврат host-элемента (первого ребёнка).
pub(crate) fn host_of(xml: &str) -> Element {
    let d = parse(xml.as_bytes()).expect("parse");
    d.root.children.into_iter().next().expect("host child")
}

/// READ→ENCODE→RENDER одного диалекта; вернуть (декод-значение, отрендеренные байты).
pub(crate) fn read_emit(
    dialect: TypeDialect,
    env: &Envelope,
    host_local: &str,
    host_prefix: &str,
    frag: &str,
) -> (TypeSpec, String) {
    let host = host_of(frag);
    let pv = decode(dialect, &host).expect("decode ok");
    let spec = match &pv {
        PropertyValue::Type(s) => s.clone(),
        _ => panic!("not Type"),
    };
    let out = encode(dialect, host_prefix, host_local, &spec).expect("encode ok");
    let bytes = render(env, &out);
    (spec, String::from_utf8(bytes).unwrap())
}

// --- BYTE-EXACT round-trip каждого квалификатор-вида в ОБОИХ диалектах ---

#[test]
fn edt_string_qualifier_roundtrips_byte_exact() {
    // length present.
    let frag = "<wrap><type>\n  <types>String</types>\n  <stringQualifiers>\n    <length>10</length>\n  </stringQualifiers>\n</type></wrap>";
    let inner = "<type>\n  <types>String</types>\n  <stringQualifiers>\n    <length>10</length>\n  </stringQualifiers>\n</type>";
    let (_, out) = read_emit(TypeDialect::Edt, &edt_env(), "type", "", frag);
    assert_eq!(out, format!("{}\n{inner}", edt_env().decl));
}

#[test]
fn edt_string_qualifier_empty_block_byte_exact() {
    // length=0 → пустой <stringQualifiers/> (sparse).
    let frag = "<wrap><type>\n  <types>String</types>\n  <stringQualifiers/>\n</type></wrap>";
    let inner = "<type>\n  <types>String</types>\n  <stringQualifiers/>\n</type>";
    let (spec, out) = read_emit(TypeDialect::Edt, &edt_env(), "type", "", frag);
    assert_eq!(
        spec.parts[0].qualifier,
        Some(TypeQualifier::String {
            length: 0,
            fixed: false
        })
    );
    assert_eq!(out, format!("{}\n{inner}", edt_env().decl));
}

#[test]
fn edt_number_qualifier_sparse_byte_exact() {
    // precision + nonNegative; scale=0 опущен.
    let frag = "<wrap><type>\n  <types>Number</types>\n  <numberQualifiers>\n    <precision>7</precision>\n    <nonNegative>true</nonNegative>\n  </numberQualifiers>\n</type></wrap>";
    let inner = "<type>\n  <types>Number</types>\n  <numberQualifiers>\n    <precision>7</precision>\n    <nonNegative>true</nonNegative>\n  </numberQualifiers>\n</type>";
    let (spec, out) = read_emit(TypeDialect::Edt, &edt_env(), "type", "", frag);
    assert_eq!(
        spec.parts[0].qualifier,
        Some(TypeQualifier::Number {
            precision: 7,
            scale: 0,
            nonnegative: true
        })
    );
    assert_eq!(out, format!("{}\n{inner}", edt_env().decl));
}

#[test]
fn edt_date_qualifier_empty_is_datetime_byte_exact() {
    let frag = "<wrap><type>\n  <types>Date</types>\n  <dateQualifiers/>\n</type></wrap>";
    let inner = "<type>\n  <types>Date</types>\n  <dateQualifiers/>\n</type>";
    let (spec, out) = read_emit(TypeDialect::Edt, &edt_env(), "type", "", frag);
    assert_eq!(
        spec.parts[0].qualifier,
        Some(TypeQualifier::Date {
            fractions: DateFractions::DateTime
        })
    );
    assert_eq!(out, format!("{}\n{inner}", edt_env().decl));
}

#[test]
fn designer_number_qualifier_dense_byte_exact() {
    let frag = "<wrap><Type>\n\t<v8:Type>xs:decimal</v8:Type>\n\t<v8:NumberQualifiers>\n\t\t<v8:Digits>7</v8:Digits>\n\t\t<v8:FractionDigits>0</v8:FractionDigits>\n\t\t<v8:AllowedSign>Nonnegative</v8:AllowedSign>\n\t</v8:NumberQualifiers>\n</Type></wrap>";
    let inner = "<Type>\n\t<v8:Type>xs:decimal</v8:Type>\n\t<v8:NumberQualifiers>\n\t\t<v8:Digits>7</v8:Digits>\n\t\t<v8:FractionDigits>0</v8:FractionDigits>\n\t\t<v8:AllowedSign>Nonnegative</v8:AllowedSign>\n\t</v8:NumberQualifiers>\n</Type>";
    let (spec, out) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", frag);
    // Canon id == Number; qualifier dense.
    assert_eq!(spec.parts[0].id, "Number");
    assert_eq!(
        spec.parts[0].qualifier,
        Some(TypeQualifier::Number {
            precision: 7,
            scale: 0,
            nonnegative: true
        })
    );
    assert_eq!(out, format!("{}\n{inner}", designer_env().decl));
}

#[test]
fn designer_string_dense_byte_exact() {
    let frag = "<wrap><Type>\n\t<v8:Type>xs:string</v8:Type>\n\t<v8:StringQualifiers>\n\t\t<v8:Length>0</v8:Length>\n\t\t<v8:AllowedLength>Variable</v8:AllowedLength>\n\t</v8:StringQualifiers>\n</Type></wrap>";
    let inner = "<Type>\n\t<v8:Type>xs:string</v8:Type>\n\t<v8:StringQualifiers>\n\t\t<v8:Length>0</v8:Length>\n\t\t<v8:AllowedLength>Variable</v8:AllowedLength>\n\t</v8:StringQualifiers>\n</Type>";
    let (_, out) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", frag);
    assert_eq!(out, format!("{}\n{inner}", designer_env().decl));
}

#[test]
fn designer_date_dense_datetime_byte_exact() {
    let frag = "<wrap><Type>\n\t<v8:Type>xs:dateTime</v8:Type>\n\t<v8:DateQualifiers>\n\t\t<v8:DateFractions>DateTime</v8:DateFractions>\n\t</v8:DateQualifiers>\n</Type></wrap>";
    let inner = "<Type>\n\t<v8:Type>xs:dateTime</v8:Type>\n\t<v8:DateQualifiers>\n\t\t<v8:DateFractions>DateTime</v8:DateFractions>\n\t</v8:DateQualifiers>\n</Type>";
    let (_, out) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", frag);
    assert_eq!(out, format!("{}\n{inner}", designer_env().decl));
}

// --- COMPOSITE + X-equality (edt-decode == designer-decode) ---

#[test]
fn composite_type_x_equal_edt_designer() {
    // FixedArray + String{len 1}: order preserved, qualifier binds to String.
    let edt = "<wrap><type>\n  <types>FixedArray</types>\n  <types>String</types>\n  <stringQualifiers>\n    <length>1</length>\n  </stringQualifiers>\n</type></wrap>";
    let des = "<wrap><Type>\n\t<v8:Type>v8:FixedArray</v8:Type>\n\t<v8:Type>xs:string</v8:Type>\n\t<v8:StringQualifiers>\n\t\t<v8:Length>1</v8:Length>\n\t\t<v8:AllowedLength>Variable</v8:AllowedLength>\n\t</v8:StringQualifiers>\n</Type></wrap>";
    let spec_edt = match decode(TypeDialect::Edt, &host_of(edt)).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let spec_des = match decode(TypeDialect::Designer, &host_of(des)).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    // X: identical canonical TypeSpec from both formats.
    assert_eq!(spec_edt, spec_des);
    assert_eq!(spec_edt.parts.len(), 2);
    assert_eq!(spec_edt.parts[0].id, "FixedArray");
    assert_eq!(spec_edt.parts[0].qualifier, None);
    assert_eq!(spec_edt.parts[1].id, "String");
    assert_eq!(
        spec_edt.parts[1].qualifier,
        Some(TypeQualifier::String {
            length: 1,
            fixed: false
        })
    );
}

#[test]
fn composite_ref_x_equal_and_order_preserved() {
    let edt = "<wrap><type>\n  <types>CatalogRef.ВнешниеПользователи</types>\n  <types>CatalogRef.Пользователи</types>\n</type></wrap>";
    let des = "<wrap><Type>\n\t<v8:Type>cfg:CatalogRef.ВнешниеПользователи</v8:Type>\n\t<v8:Type>cfg:CatalogRef.Пользователи</v8:Type>\n</Type></wrap>";
    let a = match decode(TypeDialect::Edt, &host_of(edt)).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &host_of(des)).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a, b);
    assert_eq!(a.parts[0].id, "CatalogRef.ВнешниеПользователи");
    assert_eq!(a.parts[1].id, "CatalogRef.Пользователи");
}

// --- PRESENT-EMPTY ---

#[test]
fn present_empty_type_both_dialects() {
    let edt = host_of("<wrap><type/></wrap>");
    let des = host_of("<wrap><Type/></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert!(a.parts.is_empty());
    assert_eq!(a, b);
    // Emit → self-closing host.
    let oe = encode(TypeDialect::Edt, "", "type", &a).unwrap();
    let bytes = String::from_utf8(render(&edt_env(), &oe)).unwrap();
    assert_eq!(bytes, format!("{}\n<type/>", edt_env().decl));
}

// --- ADVERSARIAL (§1.0: typed ERROR, never silent) ---

#[test]
fn unknown_edt_type_id_errors() {
    let h = host_of("<wrap><type>\n  <types>TotallyBogusType</types>\n</type></wrap>");
    let e = decode(TypeDialect::Edt, &h).unwrap_err();
    assert!(e.contains("unknown EDT type-id"), "got: {e}");
}

#[test]
fn unknown_designer_qname_errors() {
    let h = host_of("<wrap><Type>\n\t<v8:Type>v8:Bogus</v8:Type>\n</Type></wrap>");
    let e = decode(TypeDialect::Designer, &h).unwrap_err();
    assert!(e.contains("unknown Designer QName"), "got: {e}");
}

#[test]
fn binary_qualifier_errors_on_emit_both_dialects() {
    // BinaryData quals — UNKNOWN; codec must ERROR, not guess (§6).
    let spec = TypeSpec {
        parts: vec![TypeRef {
            id: "String".to_string(),
            qualifier: Some(TypeQualifier::Binary {
                length: 4,
                fixed: false,
            }),
        }],
    };
    // id String + Binary qualifier: mismatch error first.
    let e = encode(TypeDialect::Edt, "", "type", &spec).unwrap_err();
    assert!(e.contains("does not match component id"), "got: {e}");
    // Pure binary block emit also errors (BinaryData id not known → unknown id).
    let spec2 = TypeSpec {
        parts: vec![TypeRef {
            id: "BinaryData".to_string(),
            qualifier: Some(TypeQualifier::Binary {
                length: 4,
                fixed: false,
            }),
        }],
    };
    let e2 = encode(TypeDialect::Edt, "", "type", &spec2).unwrap_err();
    assert!(e2.contains("unknown canon id"), "got: {e2}");
}

#[test]
fn orphan_qualifier_errors() {
    // numberQualifiers but no Number component → orphan.
    let h = host_of("<wrap><type>\n  <types>String</types>\n  <numberQualifiers>\n    <precision>5</precision>\n  </numberQualifiers>\n</type></wrap>");
    let e = decode(TypeDialect::Edt, &h).unwrap_err();
    assert!(e.contains("orphan"), "got: {e}");
}

#[test]
fn out_of_domain_allowed_sign_errors() {
    let h = host_of("<wrap><Type>\n\t<v8:Type>xs:decimal</v8:Type>\n\t<v8:NumberQualifiers>\n\t\t<v8:Digits>5</v8:Digits>\n\t\t<v8:FractionDigits>0</v8:FractionDigits>\n\t\t<v8:AllowedSign>Negative</v8:AllowedSign>\n\t</v8:NumberQualifiers>\n</Type></wrap>");
    let e = decode(TypeDialect::Designer, &h).unwrap_err();
    assert!(e.contains("AllowedSign domain"), "got: {e}");
}

#[test]
fn unexpected_child_in_type_errors() {
    let h = host_of("<wrap><type>\n  <types>String</types>\n  <stringQualifiers/>\n  <bogus>x</bogus>\n</type></wrap>");
    let e = decode(TypeDialect::Edt, &h).unwrap_err();
    assert!(e.contains("unexpected child"), "got: {e}");
}

#[test]
fn value_list_trap_canonicalizes() {
    // Designer v8:ValueListType ↔ canon ValueList.
    let des = host_of("<wrap><Type>\n\t<v8:Type>v8:ValueListType</v8:Type>\n</Type></wrap>");
    let s = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(s.parts[0].id, "ValueList");
    // EDT side: bare ValueList canonical, and X-equal.
    let edt = host_of("<wrap><type>\n  <types>ValueList</types>\n</type></wrap>");
    let s2 = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(s, s2);
}

/// SC7: EDT bare `HorizontalAlign` ↔ Designer `v8ui:HorizontalAlign` (тот же паттерн, что
/// VerticalAlign). ERP-witness Catalog.ЭлементыФинансовыхОтчетов.РедактированиеЭлемента…
/// реквизит «Горизонтальное положение». X-равно.
#[test]
fn horizontal_align_v8ui_alias() {
    assert_eq!(
        qname_to_canon("v8ui:HorizontalAlign").as_deref(),
        Some("HorizontalAlign")
    );
    assert_eq!(
        canon_to_qname("HorizontalAlign").as_deref(),
        Some("v8ui:HorizontalAlign")
    );
    let des =
        host_of("<wrap><Type>\n\t<v8:Type>v8ui:HorizontalAlign</v8:Type>\n</Type></wrap>");
    let s = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(s.parts[0].id, "HorizontalAlign");
    let edt = host_of("<wrap><type>\n  <types>HorizontalAlign</types>\n</type></wrap>");
    let s2 = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(s, s2, "SC7: edt ≡ designer (X)");
}

#[test]
fn canon_qname_roundtrips_primitives_and_aliases() {
    for (canon, qname) in PRIMITIVES {
        assert_eq!(qname_to_canon(qname).as_deref(), Some(*canon));
        assert_eq!(canon_to_qname(canon).as_deref(), Some(*qname));
    }
    // ValueList трап: local-name алиас.
    assert_eq!(
        qname_to_canon("v8:ValueListType").as_deref(),
        Some("ValueList")
    );
    assert_eq!(
        canon_to_qname("ValueList").as_deref(),
        Some("v8:ValueListType")
    );
    // platform bare.
    assert_eq!(
        qname_to_canon("v8:FixedStructure").as_deref(),
        Some("FixedStructure")
    );
    assert_eq!(
        canon_to_qname("FixedStructure").as_deref(),
        Some("v8:FixedStructure")
    );
    // ref.
    assert_eq!(
        qname_to_canon("cfg:CatalogRef.X").as_deref(),
        Some("CatalogRef.X")
    );
    assert_eq!(
        canon_to_qname("CatalogRef.X").as_deref(),
        Some("cfg:CatalogRef.X")
    );
}

#[test]
fn unknown_qname_is_none() {
    // незнакомый платформенный / неизвестный ref-kind / случайный QName.
    assert_eq!(qname_to_canon("v8:Bogus"), None);
    assert_eq!(qname_to_canon("cfg:BogusRef.X"), None);
    assert_eq!(qname_to_canon("zz:string"), None);
    assert_eq!(canon_to_qname("Bogus"), None);
}

// --- TYPE-SET (EDT <types> ↔ Designer <v8:TypeSet>) ---

#[test]
fn type_set_x_equal_bare_ref_and_defined_type() {
    // BARE ref-kind: EDT <types>ExchangePlanRef ↔ Designer <v8:TypeSet>cfg:ExchangePlanRef.
    let edt = host_of("<wrap><type>\n  <types>ExchangePlanRef</types>\n</type></wrap>");
    let des =
        host_of("<wrap><Type>\n\t<v8:TypeSet>cfg:ExchangePlanRef</v8:TypeSet>\n</Type></wrap>");
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "ExchangePlanRef");
    assert_eq!(a, b); // X by construction.

    // DefinedType.Имя: EDT <types> ↔ Designer <v8:TypeSet>.
    let edt2 = host_of("<wrap><type>\n  <types>DefinedType.Безопасный</types>\n</type></wrap>");
    let des2 = host_of(
        "<wrap><Type>\n\t<v8:TypeSet>cfg:DefinedType.Безопасный</v8:TypeSet>\n</Type></wrap>",
    );
    let a2 = match decode(TypeDialect::Edt, &edt2).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b2 = match decode(TypeDialect::Designer, &des2).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a2.parts[0].id, "DefinedType.Безопасный");
    assert_eq!(a2, b2);
}

#[test]
fn type_set_byte_exact_both_dialects() {
    // EDT emit: <types>ExchangePlanRef.
    let edt = host_of("<wrap><type>\n  <types>ExchangePlanRef</types>\n</type></wrap>");
    let (_, out) = read_emit(
        TypeDialect::Edt,
        &edt_env(),
        "type",
        "",
        "<wrap><type>\n  <types>ExchangePlanRef</types>\n</type></wrap>",
    );
    let _ = edt;
    assert_eq!(
        out,
        format!(
            "{}\n<type>\n  <types>ExchangePlanRef</types>\n</type>",
            edt_env().decl
        )
    );

    // Designer emit: <v8:TypeSet>cfg:ExchangePlanRef (NOT <v8:Type>).
    let frag = "<wrap><Type>\n\t<v8:TypeSet>cfg:ExchangePlanRef</v8:TypeSet>\n</Type></wrap>";
    let (_, out_d) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", frag);
    assert_eq!(
        out_d,
        format!(
            "{}\n<Type>\n\t<v8:TypeSet>cfg:ExchangePlanRef</v8:TypeSet>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn concrete_ref_stays_v8_type_not_typeset() {
    // CatalogRef.Имя — конкретный тип → Designer эмитит <v8:Type> (НЕ TypeSet).
    let frag = "<wrap><Type>\n\t<v8:Type>cfg:CatalogRef.Товары</v8:Type>\n</Type></wrap>";
    let (_, out) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", frag);
    assert_eq!(
        out,
        format!(
            "{}\n<Type>\n\t<v8:Type>cfg:CatalogRef.Товары</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
    assert!(!is_type_set("CatalogRef.Товары"));
    assert!(is_type_set("ExchangePlanRef"));
    assert!(is_type_set("DefinedType.X"));
}

#[test]
fn event_subscription_ref_kinds_classify_correctly() {
    // EventSubscription `Source` ref-kinds: bare RecordSet/Object → TypeSet, `Kind.Имя`
    // → Type. NB: bare `*Manager` — ИСКЛЮЧЕНИЕ (ERP-корпус: контр-witness bare Manager →
    // `<v8:Type>`, НЕ TypeSet — см. `NON_TYPESET_BARE_REFS`); проверяются ниже отдельно.
    for kind in [
        "AccountingRegisterRecordSet",
        "AccumulationRegisterRecordSet",
        "CalculationRegisterRecordSet",
        "ChartOfAccountsObject",
        "ChartOfCalculationTypesObject",
        "ExchangePlanObject",
    ] {
        // BARE kind — известная ссылка И type-set (Designer хостит как <v8:TypeSet>).
        assert!(is_known_ref(kind), "bare {kind} must be a known ref");
        assert!(is_type_set(kind), "bare {kind} must be a type-set");
        // КОНКРЕТНЫЙ `Kind.Имя` — известная ссылка, но НЕ type-set (обычный <v8:Type>).
        let concrete = format!("{kind}.Имя");
        assert!(
            is_known_ref(&concrete),
            "concrete {concrete} must be a known ref"
        );
        assert!(
            !is_type_set(&concrete),
            "concrete {concrete} must NOT be a type-set"
        );
    }
    // BARE `*Manager` — известная ссылка, но НЕ type-set (ERP counter-witness, хостится
    // `<v8:Type>`); `Kind.Имя` — тоже не type-set.
    for kind in ["CatalogManager", "DocumentManager", "BusinessProcessManager"] {
        assert!(is_known_ref(kind), "bare {kind} must be a known ref");
        assert!(
            !is_type_set(kind),
            "bare {kind} must NOT be a type-set (ERP: bare Manager → <v8:Type>)"
        );
        let concrete = format!("{kind}.Имя");
        assert!(is_known_ref(&concrete), "concrete {concrete} must be a known ref");
        assert!(!is_type_set(&concrete), "concrete {concrete} must NOT be a type-set");
    }
    // X by construction для bare ExchangePlanObject (новый kind): EDT <types> ↔ TypeSet.
    let edt = host_of("<wrap><type>\n  <types>ExchangePlanObject</types>\n</type></wrap>");
    let des = host_of(
        "<wrap><Type>\n\t<v8:TypeSet>cfg:ExchangePlanObject</v8:TypeSet>\n</Type></wrap>",
    );
    let a = match decode(TypeDialect::Edt, &edt).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    let b = match decode(TypeDialect::Designer, &des).unwrap() {
        PropertyValue::Type(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(a.parts[0].id, "ExchangePlanObject");
    assert_eq!(a, b);
    // Concrete CatalogObject.Имя (новый сосед DocumentManager и пр.) — обычный <v8:Type>.
    let frag = "<wrap><Type>\n\t<v8:Type>cfg:DocumentManager.Заказ</v8:Type>\n</Type></wrap>";
    let (_, out) = read_emit(TypeDialect::Designer, &designer_env(), "Type", "", frag);
    assert_eq!(
        out,
        format!(
            "{}\n<Type>\n\t<v8:Type>cfg:DocumentManager.Заказ</v8:Type>\n</Type>",
            designer_env().decl
        )
    );
}

#[test]
fn type_set_hosting_concrete_id_errors() {
    // <v8:TypeSet> с КОНКРЕТНЫМ типом (CatalogRef.Имя) → ошибка (§1.0).
    let h = host_of(
        "<wrap><Type>\n\t<v8:TypeSet>cfg:CatalogRef.Товары</v8:TypeSet>\n</Type></wrap>",
    );
    let e = decode(TypeDialect::Designer, &h).unwrap_err();
    assert!(e.contains("non-type-set"), "got: {e}");
}
