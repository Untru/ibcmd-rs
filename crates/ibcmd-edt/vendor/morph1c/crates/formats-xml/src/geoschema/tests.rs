//! Byte-exact round-trip тесты коннектора геосхемы против ВИТНЕССИРОВАННЫХ платформенных тел
//! (эталон: `.fixtures/coverage` s12 `Макет_ГеографическаяСхема`, декомпилировано из cf в оба
//! формата — идентично для s4/s12/пример). Дефолтная пустая геосхема.

use super::*;

/// Собрать байты из строк-линий + CRLF + опц. BOM/trailing-EOL (структурный EOL тел — CRLF).
fn bytes(lines: &[&str], bom: bool, trailing_eol: bool) -> Vec<u8> {
    let mut s = String::new();
    if bom {
        s.push('\u{feff}');
    }
    s.push_str(&lines.join("\r\n"));
    if trailing_eol {
        s.push_str("\r\n");
    }
    s.into_bytes()
}

/// Витнессированное Designer/extrnprops тело (BOM, CRLF, TAB, БЕЗ trailing EOL) — 744 байта.
fn designer_body() -> Vec<u8> {
    bytes(
        &[
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            "<GeographicalScheme xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:d1p1=\"http://v8.1c.ru/8.2/data/geo\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"d1p1:GeographicalSchema\" curId=\"1\" proj=\"CylindricalMillerProjection\" showMode=\"AllData\" scale=\"1\" xmn=\"0\" xmx=\"0\" ymn=\"0\" ymx=\"0\" dxn=\"0\" dxx=\"0\" dyn=\"0\" dyx=\"0\" vxn=\"0\" vxx=\"0\" vyn=\"0\" vyx=\"0\" useOutput=\"Auto\">",
            "\t<d1p1:legend show=\"true\" trnsprnt=\"true\" scale=\"true\" left=\"75\" right=\"0\" top=\"5\" bottom=\"0\"/>",
            "\t<d1p1:title show=\"true\" trnsprnt=\"true\" left=\"0\" right=\"0\" top=\"0\" bottom=\"95\"/>",
            "\t<d1p1:drawing trnsprnt=\"true\" left=\"0\" right=\"25\" top=\"5\" bottom=\"0\"/>",
            "</GeographicalScheme>",
        ],
        /*bom=*/ true,
        /*trailing_eol=*/ false,
    )
}

/// Витнессированное EDT/g5 тело (no BOM, CRLF, 2 пробела, trailing EOL) — 2548 байт.
fn edt_body() -> Vec<u8> {
    bytes(
        &[
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            "<geographicalSchema:GeographicalSchema xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:geographicalSchema=\"http://g5.1c.ru/v8/dt/geographicalschema\">",
            "  <geographicalSchemaLegend>",
            "    <border xsi:type=\"core:BorderDef\">",
            "      <width>1</width>",
            "    </border>",
            "    <transparent>true</transparent>",
            "    <leftCoordinatePercentage>75</leftCoordinatePercentage>",
            "    <topCoordinatePercentage>5</topCoordinatePercentage>",
            "    <scaleLineTextColor xsi:type=\"core:ColorRef\">",
            "      <color>Style.FormTextColor</color>",
            "    </scaleLineTextColor>",
            "    <borderColor xsi:type=\"core:ColorRef\">",
            "      <color>Style.BorderColor</color>",
            "    </borderColor>",
            "    <backColor xsi:type=\"core:ColorRef\">",
            "      <color>Style.FieldBackColor</color>",
            "    </backColor>",
            "    <scaleLineTextFont xsi:type=\"core:FontRef\">",
            "      <font>Style.TextFont</font>",
            "    </scaleLineTextFont>",
            "    <shown>true</shown>",
            "    <scaleLineShown>true</scaleLineShown>",
            "  </geographicalSchemaLegend>",
            "  <geographicalSchemaTitle>",
            "    <border xsi:type=\"core:BorderDef\">",
            "      <width>1</width>",
            "    </border>",
            "    <transparent>true</transparent>",
            "    <bottomCoordinatePercentage>95</bottomCoordinatePercentage>",
            "    <font xsi:type=\"core:FontRef\">",
            "      <font>Style.TextFont</font>",
            "    </font>",
            "    <textColor xsi:type=\"core:ColorRef\">",
            "      <color>Style.FormTextColor</color>",
            "    </textColor>",
            "    <borderColor xsi:type=\"core:ColorRef\">",
            "      <color>Style.BorderColor</color>",
            "    </borderColor>",
            "    <backColor xsi:type=\"core:ColorRef\">",
            "      <color>Style.FieldBackColor</color>",
            "    </backColor>",
            "    <shown>true</shown>",
            "  </geographicalSchemaTitle>",
            "  <geographicalSchemaDrawingArea>",
            "    <border xsi:type=\"core:BorderDef\">",
            "      <width>1</width>",
            "    </border>",
            "    <transparent>true</transparent>",
            "    <topCoordinatePercentage>5</topCoordinatePercentage>",
            "    <rightCoordinatePercentage>25</rightCoordinatePercentage>",
            "    <borderColor xsi:type=\"core:ColorRef\">",
            "      <color>Style.BorderColor</color>",
            "    </borderColor>",
            "    <backColor xsi:type=\"core:ColorRef\">",
            "      <color>Style.FieldBackColor</color>",
            "    </backColor>",
            "  </geographicalSchemaDrawingArea>",
            "  <showMode>AllData</showMode>",
            "  <scale>1.0</scale>",
            "  <viewedXMin>1.7976931348623157E308</viewedXMin>",
            "  <viewedYMin>1.7976931348623157E308</viewedYMin>",
            "  <viewedXMax>-1.7976931348623157E308</viewedXMax>",
            "  <viewedYMax>-1.7976931348623157E308</viewedYMax>",
            "  <currentID>1</currentID>",
            "</geographicalSchema:GeographicalSchema>",
        ],
        /*bom=*/ false,
        /*trailing_eol=*/ true,
    )
}

fn show(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .replace('\r', "\\r")
        .replace('\n', "\\n\n")
}

#[test]
fn designer_read_write_is_byte_exact() {
    let d = designer_body();
    let geo = read_geoschema(GeoDialect::Designer, &d).expect("read designer");
    let out = write_geoschema(GeoDialect::Designer, &geo).expect("write designer");
    assert_eq!(
        out,
        d,
        "designer round-trip\n--- got ---\n{}\n--- want ---\n{}",
        show(&out),
        show(&d)
    );
}

#[test]
fn edt_read_write_is_byte_exact() {
    let e = edt_body();
    let geo = read_geoschema(GeoDialect::Edt, &e).expect("read edt");
    let out = write_geoschema(GeoDialect::Edt, &geo).expect("write edt");
    assert_eq!(
        out,
        e,
        "edt round-trip\n--- got ---\n{}\n--- want ---\n{}",
        show(&out),
        show(&e)
    );
}

#[test]
fn designer_to_edt_matches_witnessed() {
    // The crux: designer extrnprops → g5, substituting the g5-only defaults (colors/fonts/border,
    // viewed-bounds ±MAX sentinel). Must reproduce the platform's Template.geos byte-exact.
    let geo = read_geoschema(GeoDialect::Designer, &designer_body()).expect("read designer");
    let out = write_geoschema(GeoDialect::Edt, &geo).expect("write edt");
    let want = edt_body();
    assert_eq!(
        out,
        want,
        "designer→edt\n--- got ---\n{}\n--- want ---\n{}",
        show(&out),
        show(&want)
    );
}

#[test]
fn edt_to_designer_matches_witnessed() {
    // g5 → extrnprops (canonical): collapse the materialized defaults + ±MAX sentinel back to the
    // compact designer form. Must reproduce the platform's Ext/Template.xml byte-exact.
    let geo = read_geoschema(GeoDialect::Edt, &edt_body()).expect("read edt");
    let out = write_geoschema(GeoDialect::Designer, &geo).expect("write designer");
    let want = designer_body();
    assert_eq!(
        out,
        want,
        "edt→designer\n--- got ---\n{}\n--- want ---\n{}",
        show(&out),
        show(&want)
    );
}

#[test]
fn cross_format_ir_is_equal() {
    // §1.6: the canonical GeoSchema parsed from either dialect is IDENTICAL (edt==designer).
    let from_d = read_geoschema(GeoDialect::Designer, &designer_body()).expect("read designer");
    let from_e = read_geoschema(GeoDialect::Edt, &edt_body()).expect("read edt");
    assert_eq!(
        from_d, from_e,
        "canonical GeoSchema must be dialect-neutral (§1.6)"
    );
}
