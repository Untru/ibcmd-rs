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
fn designer_env() -> Envelope {
    Envelope {
        bom: false,
        eol: "\n",
        indent_unit: "\t",
        decl: "<?xml version=\"1.0\"?>",
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "\n",
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
fn spec_of(pv: PropertyValue) -> ValueSpec {
    match pv {
        PropertyValue::Value(s) => s,
        _ => panic!("not Value"),
    }
}

// X-equality across all 6 kinds: EDT-decode == Designer-decode.
#[test]
fn undefined_x_equal() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"core:UndefinedValue\"/></w>"),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:nil=\"true\"/></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.kind, ValueScalarKind::Undefined);
    assert_eq!(e, d);
}
#[test]
fn string_empty_x_equal() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"core:StringValue\"/></w>"),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"xs:string\"/></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.kind, ValueScalarKind::Str);
    assert_eq!(
        e.scalar.as_deref(),
        Some(&PropertyValue::Str(String::new()))
    );
    assert_eq!(e, d);
}
#[test]
fn boolean_false_x_equal() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"core:BooleanValue\"/></w>"),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"xs:boolean\">false</v></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.scalar.as_deref(), Some(&PropertyValue::Bool(false)));
    assert_eq!(e, d);
}
#[test]
fn boolean_true_x_equal() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"core:BooleanValue\"><value>true</value></v></w>"),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"xs:boolean\">true</v></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.scalar.as_deref(), Some(&PropertyValue::Bool(true)));
    assert_eq!(e, d);
}
#[test]
fn number_x_equal() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"core:NumberValue\"><value>3600</value></v></w>"),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"xs:decimal\">3600</v></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.kind, ValueScalarKind::Number);
    assert_eq!(e, d);
}
#[test]
fn date_x_equal() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of(
                "<w><v xsi:type=\"core:DateValue\"><value>0001-01-01T00:00:00</value></v></w>",
            ),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"xs:dateTime\">0001-01-01T00:00:00</v></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.kind, ValueScalarKind::Date);
    assert_eq!(e, d);
}
#[test]
fn reference_x_equal() {
    let e = spec_of(decode(ValueDialect::Edt, &host_of("<w><v xsi:type=\"core:ReferenceValue\"><value>Catalog.X.EmptyRef</value></v></w>")).unwrap());
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"xr:DesignTimeRef\">Catalog.X.EmptyRef</v></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.kind, ValueScalarKind::Reference);
    assert_eq!(e, d);
}

/// AccountType (ERP-witnessed fillValue std-атрибута Type Хозрасчетного): EDT
/// `form:AccountTypeValue`+`<value>` ⇔ Designer `ent:AccountType` текст — X-equal.
#[test]
fn account_type_x_equal() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of(
                "<w><v xsi:type=\"form:AccountTypeValue\"><value>ActivePassive</value></v></w>",
            ),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"ent:AccountType\">ActivePassive</v></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.kind, ValueScalarKind::AccountType);
    assert_eq!(
        e.scalar.as_deref(),
        Some(&PropertyValue::Str("ActivePassive".into()))
    );
    assert_eq!(e, d);
}

/// §1.0: литерал вне enum'а Active/Passive/ActivePassive — отказ в обоих диалектах.
#[test]
fn account_type_foreign_literal_refused() {
    let err = decode(
        ValueDialect::Edt,
        &host_of("<w><v xsi:type=\"form:AccountTypeValue\"><value>Both</value></v></w>"),
    )
    .unwrap_err();
    assert!(err.contains("AccountType literal"), "{err}");
    let err = decode(
        ValueDialect::Designer,
        &host_of("<w><v xsi:type=\"ent:AccountType\">Both</v></w>"),
    )
    .unwrap_err();
    assert!(err.contains("AccountType literal"), "{err}");
}

// Byte-exact round-trip each form per dialect.
fn rt_edt(xml: &str, want: &str) {
    let h = host_of(&format!("<w>{xml}</w>"));
    let spec = spec_of(decode(ValueDialect::Edt, &h).unwrap());
    let out = encode(ValueDialect::Edt, "", "v", &spec).unwrap();
    let bytes = String::from_utf8(render(&edt_env(), &out)).unwrap();
    assert_eq!(bytes, format!("{}\n{want}", edt_env().decl), "edt rt {xml}");
}
fn rt_des(xml: &str, want: &str) {
    let h = host_of(&format!("<w>{xml}</w>"));
    let spec = spec_of(decode(ValueDialect::Designer, &h).unwrap());
    let out = encode(ValueDialect::Designer, "", "v", &spec).unwrap();
    let bytes = String::from_utf8(render(&designer_env(), &out)).unwrap();
    assert_eq!(
        bytes,
        format!("{}\n{want}", designer_env().decl),
        "des rt {xml}"
    );
}
#[test]
fn byte_exact_edt_all() {
    rt_edt(
        "<v xsi:type=\"core:UndefinedValue\"/>",
        "<v xsi:type=\"core:UndefinedValue\"/>",
    );
    rt_edt(
        "<v xsi:type=\"core:StringValue\"/>",
        "<v xsi:type=\"core:StringValue\"/>",
    );
    rt_edt(
        "<v xsi:type=\"core:StringValue\"><value>x</value></v>",
        "<v xsi:type=\"core:StringValue\">\n  <value>x</value>\n</v>",
    );
    rt_edt(
        "<v xsi:type=\"core:BooleanValue\"/>",
        "<v xsi:type=\"core:BooleanValue\"/>",
    );
    rt_edt(
        "<v xsi:type=\"core:BooleanValue\"><value>true</value></v>",
        "<v xsi:type=\"core:BooleanValue\">\n  <value>true</value>\n</v>",
    );
    rt_edt(
        "<v xsi:type=\"core:NumberValue\"><value>0</value></v>",
        "<v xsi:type=\"core:NumberValue\">\n  <value>0</value>\n</v>",
    );
    rt_edt(
        "<v xsi:type=\"core:DateValue\"><value>0001-01-01T00:00:00</value></v>",
        "<v xsi:type=\"core:DateValue\">\n  <value>0001-01-01T00:00:00</value>\n</v>",
    );
    rt_edt(
        "<v xsi:type=\"core:ReferenceValue\"/>",
        "<v xsi:type=\"core:ReferenceValue\"/>",
    );
    rt_edt(
        "<v xsi:type=\"core:ReferenceValue\"><value>Catalog.X.EmptyRef</value></v>",
        "<v xsi:type=\"core:ReferenceValue\">\n  <value>Catalog.X.EmptyRef</value>\n</v>",
    );
    rt_edt(
        "<v xsi:type=\"form:AccountTypeValue\"><value>ActivePassive</value></v>",
        "<v xsi:type=\"form:AccountTypeValue\">\n  <value>ActivePassive</value>\n</v>",
    );
}
#[test]
fn byte_exact_designer_all() {
    rt_des("<v xsi:nil=\"true\"/>", "<v xsi:nil=\"true\"/>");
    rt_des("<v xsi:type=\"xs:string\"/>", "<v xsi:type=\"xs:string\"/>");
    rt_des(
        "<v xsi:type=\"xs:string\">x</v>",
        "<v xsi:type=\"xs:string\">x</v>",
    );
    rt_des(
        "<v xsi:type=\"xs:boolean\">false</v>",
        "<v xsi:type=\"xs:boolean\">false</v>",
    );
    rt_des(
        "<v xsi:type=\"xs:boolean\">true</v>",
        "<v xsi:type=\"xs:boolean\">true</v>",
    );
    rt_des(
        "<v xsi:type=\"xs:decimal\">3600</v>",
        "<v xsi:type=\"xs:decimal\">3600</v>",
    );
    rt_des(
        "<v xsi:type=\"xs:dateTime\">0001-01-01T00:00:00</v>",
        "<v xsi:type=\"xs:dateTime\">0001-01-01T00:00:00</v>",
    );
    rt_des(
        "<v xsi:type=\"xr:DesignTimeRef\"/>",
        "<v xsi:type=\"xr:DesignTimeRef\"/>",
    );
    rt_des(
        "<v xsi:type=\"xr:DesignTimeRef\">Catalog.X.EmptyRef</v>",
        "<v xsi:type=\"xr:DesignTimeRef\">Catalog.X.EmptyRef</v>",
    );
    rt_des(
        "<v xsi:type=\"ent:AccountType\">ActivePassive</v>",
        "<v xsi:type=\"ent:AccountType\">ActivePassive</v>",
    );
}

// PRESENCE-OF-DEFAULT: explicit empty `<value></value>` StringValue/ReferenceValue is
// DISTINCT from the self-close host, and each round-trips to its own bytes (byte-exact R).
#[test]
fn edt_explicit_empty_string_distinct_from_self_close() {
    // Self-close ⇒ canonical Some(Str("")).
    let sc = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"core:StringValue\"/></w>"),
        )
        .unwrap(),
    );
    assert_eq!(
        sc.scalar.as_deref(),
        Some(&PropertyValue::Str(String::new()))
    );
    // Explicit `<value></value>` ⇒ presence-of-default marker (scalar None).
    let ee = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"core:StringValue\"><value></value></v></w>"),
        )
        .unwrap(),
    );
    assert_eq!(ee.kind, ValueScalarKind::Str);
    assert_eq!(ee.scalar, None);
    // The two forms are NOT equal (distinct in IR).
    assert_ne!(sc, ee);
}
#[test]
fn edt_explicit_empty_value_byte_exact_roundtrip() {
    // The explicit `<value></value>` byte-form must re-emit itself (not collapse to
    // self-close). Witnessed in ERP `fillValue`.
    rt_edt(
        "<v xsi:type=\"core:StringValue\"><value></value></v>",
        "<v xsi:type=\"core:StringValue\">\n  <value></value>\n</v>",
    );
    rt_edt(
        "<v xsi:type=\"core:ReferenceValue\"><value></value></v>",
        "<v xsi:type=\"core:ReferenceValue\">\n  <value></value>\n</v>",
    );
    // Self-close still round-trips to self-close (unchanged, additive).
    rt_edt(
        "<v xsi:type=\"core:StringValue\"/>",
        "<v xsi:type=\"core:StringValue\"/>",
    );
}
#[test]
fn designer_collapses_explicit_empty_marker_to_self_close() {
    // A cross-format ERP→Designer of the explicit-empty marker (scalar None) yields a
    // self-close (Designer has one empty form); non-lossy for R (no ERP Designer corpus).
    let spec = ValueSpec {
        kind: ValueScalarKind::Str,
        scalar: None,
    };
    let out = encode(ValueDialect::Designer, "", "v", &spec).unwrap();
    let bytes = String::from_utf8(render(&designer_env(), &out)).unwrap();
    assert_eq!(
        bytes,
        format!("{}\n<v xsi:type=\"xs:string\"/>", designer_env().decl)
    );
}

// §1.0 adversarial: unknown xsi-type errors.
#[test]
fn unknown_edt_errors() {
    assert!(decode(
        ValueDialect::Edt,
        &host_of("<w><v xsi:type=\"core:FooValue\"/></w>")
    )
    .unwrap_err()
    .contains("unknown EDT xsi:type"));
}
#[test]
fn unknown_designer_errors() {
    assert!(decode(
        ValueDialect::Designer,
        &host_of("<w><v xsi:type=\"xs:foo\">1</v></w>")
    )
    .unwrap_err()
    .contains("unknown Designer xsi:type"));
}

// ===================== SC5: ValueList (пустой список значений) =====================

/// EDT `core:ValueList` (self-close) ⟺ Designer `xr:ValueList` (self-close) — X-равно,
/// `scalar==None`. ERP-witness Document.ВходящийДокументСЭДОФСС.ФормаВыбораТребования choiceList.
#[test]
fn value_list_x_equal_and_empty() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"core:ValueList\"/></w>"),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"xr:ValueList\"/></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.kind, ValueScalarKind::ValueList);
    assert_eq!(e.scalar, None);
    assert_eq!(e, d, "SC5: edt ≡ designer (X)");
}

/// SC5 byte-exact round-trip оба диалекта (self-close ⇒ self-close).
#[test]
fn value_list_byte_exact_both() {
    rt_edt(
        "<v xsi:type=\"core:ValueList\"/>",
        "<v xsi:type=\"core:ValueList\"/>",
    );
    rt_des(
        "<v xsi:type=\"xr:ValueList\"/>",
        "<v xsi:type=\"xr:ValueList\"/>",
    );
}

/// §1.0: непустой ValueList не витнессирован — encode отказывает; Designer с текстом — decode
/// отказывает.
#[test]
fn value_list_nonempty_refused() {
    let bad = ValueSpec {
        kind: ValueScalarKind::ValueList,
        scalar: Some(Box::new(PropertyValue::Str("x".into()))),
    };
    assert!(encode(ValueDialect::Edt, "", "v", &bad)
        .unwrap_err()
        .contains("non-empty ValueList"));
    assert!(decode(
        ValueDialect::Designer,
        &host_of("<w><v xsi:type=\"xr:ValueList\">x</v></w>")
    )
    .unwrap_err()
    .contains("xr:ValueList must be empty"));
}

// ===================== SC6: AccountType без <value> (дефолт Active) =====================

/// EDT `form:AccountTypeValue` БЕЗ `<value>` (self-close) ⟺ Designer `ent:AccountType`=`Active`:
/// оба → литерал `Active` (cf-код 0). X-равно. ERP-witness Хозрасчетный.ФормаСчета choiceList.
#[test]
fn account_type_omitted_value_is_active_x_equal() {
    let e = spec_of(
        decode(
            ValueDialect::Edt,
            &host_of("<w><v xsi:type=\"form:AccountTypeValue\"/></w>"),
        )
        .unwrap(),
    );
    let d = spec_of(
        decode(
            ValueDialect::Designer,
            &host_of("<w><v xsi:type=\"ent:AccountType\">Active</v></w>"),
        )
        .unwrap(),
    );
    assert_eq!(e.kind, ValueScalarKind::AccountType);
    assert_eq!(e.scalar.as_deref(), Some(&PropertyValue::Str("Active".into())));
    assert_eq!(e, d, "SC6: edt ≡ designer (X)");
}

/// SC6 byte-exact: EDT `Active` ⇒ self-close, `Passive`/`ActivePassive` ⇒ явный `<value>`.
#[test]
fn account_type_edt_omits_only_active() {
    rt_edt(
        "<v xsi:type=\"form:AccountTypeValue\"/>",
        "<v xsi:type=\"form:AccountTypeValue\"/>",
    );
    rt_edt(
        "<v xsi:type=\"form:AccountTypeValue\"><value>Passive</value></v>",
        "<v xsi:type=\"form:AccountTypeValue\">\n  <value>Passive</value>\n</v>",
    );
    // Designer эмитит литерал ВСЕГДА (в т.ч. Active) — не self-close.
    rt_des(
        "<v xsi:type=\"ent:AccountType\">Active</v>",
        "<v xsi:type=\"ent:AccountType\">Active</v>",
    );
}
