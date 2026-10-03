//! CHART · юнит-тесты, вынесенные из `chart.rs`: designer/EDT byte-exact roundtrip,
//! designer→EDT транскод-синтез, §1.0-отказы (незнакомое поле / нарушение модельного порядка).

use super::*;
use crate::emit::Envelope;

/// Designer-подобный envelope для standalone-рендера `<Settings>` в тестах.
fn test_designer_envelope() -> Envelope {
    Envelope {
        bom: false,
        eol: "\r\n",
        indent_unit: "\t",
        decl: FORM_DECL,
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "\n",
    }
}

/// Мини-витнесс designer-блока (фрагменты sezon/vypoln): скалярные поля, цвет, шрифт,
/// рамка, линия, локализация, пустая ось, серия с текстом.
fn designer_doc() -> Vec<u8> {
    let body = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Settings xmlns:d4p1=\"http://v8.1c.ru/8.2/data/chart\" xsi:type=\"d4p1:Chart\">\r\n\
\t<d4p1:seriesCurId>1</d4p1:seriesCurId>\r\n\
\t<d4p1:realExSeriesData>\r\n\
\t\t<d4p1:id>1</d4p1:id>\r\n\
\t\t<d4p1:color>auto</d4p1:color>\r\n\
\t\t<d4p1:line width=\"2\" gap=\"false\">\r\n\
\t\t\t<v8ui:style xsi:type=\"v8ui:ChartLineType\">Solid</v8ui:style>\r\n\
\t\t</d4p1:line>\r\n\
\t\t<d4p1:marker>Auto</d4p1:marker>\r\n\
\t\t<d4p1:text>\r\n\
\t\t\t<v8:item>\r\n\
\t\t\t\t<v8:lang>#</v8:lang>\r\n\
\t\t\t\t<v8:content>Pivot</v8:content>\r\n\
\t\t\t</v8:item>\r\n\
\t\t</d4p1:text>\r\n\
\t\t<d4p1:strIsChanged>false</d4p1:strIsChanged>\r\n\
\t</d4p1:realExSeriesData>\r\n\
\t<d4p1:chartType>Column3D</d4p1:chartType>\r\n\
\t<d4p1:labelsDelimiter>, </d4p1:labelsDelimiter>\r\n\
\t<d4p1:labelsColor>#333333</d4p1:labelsColor>\r\n\
\t<d4p1:labelsFont kind=\"AutoFont\"/>\r\n\
\t<d4p1:labelsBorder width=\"1\">\r\n\
\t\t<v8ui:style xsi:type=\"v8ui:ControlBorderType\">Single</v8ui:style>\r\n\
\t</d4p1:labelsBorder>\r\n\
\t<d4p1:title/>\r\n\
\t<d4p1:chFont ref=\"style:NormalTextFont\" height=\"6\" kind=\"StyleItem\"/>\r\n\
\t<d4p1:baseVal>0</d4p1:baseVal>\r\n\
\t<d4p1:isRandomizedNewValues>true</d4p1:isRandomizedNewValues>\r\n\
\t<d4p1:valuesAxis/>\r\n\
</Settings>";
    body.as_bytes().to_vec()
}

#[test]
fn designer_roundtrip_byte_exact() {
    let src = designer_doc();
    let d = parse(&src).expect("parse designer settings");
    let cs = read_designer_chart_settings("Chart", &d.root).expect("read designer chart");
    assert_eq!(
        d.root.unclaimed_count(),
        0,
        "тотальность §1.0 по designer-блоку"
    );
    assert_eq!(cs.kind, "Chart");
    let out = designer_chart_settings(&cs).expect("write designer chart");
    let bytes = render(&test_designer_envelope(), &out);
    assert_eq!(
        String::from_utf8_lossy(&bytes),
        String::from_utf8_lossy(&src),
        "designer эхо byte-exact"
    );
}

/// Мини-витнесс EDT-сайдкара (фрагмент sezon; `translucenceMode` включён — эхо-режим).
fn edt_sidecar_doc() -> Vec<u8> {
    // NB: `concat!`, НЕ `\`-line-continuation — последняя СЪЕДАЕТ ведущие пробелы строк
    // продолжения, из-за чего фикстура становилась flush-left, тогда как реальный сайдкар
    // (и наш writer) — 2-space-отступ на уровень (сверено `cat -A` по erp.cf-выгрузке).
    let body = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<chart:Chart xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:chart=\"http://g5.1c.ru/v8/dt/chart/model\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">\r\n",
        "  <seriesCurId>1</seriesCurId>\r\n",
        "  <isSeriesDesign>true</isSeriesDesign>\r\n",
        "  <realExSeriesData>\r\n",
        "    <properties>\r\n",
        "      <id>1</id>\r\n",
        "      <valInfo xsi:type=\"core:UndefinedValue\"/>\r\n",
        "      <line>\r\n",
        "        <width>2</width>\r\n",
        "        <style>Solid</style>\r\n",
        "      </line>\r\n",
        "      <marker>Auto</marker>\r\n",
        "      <text>\r\n",
        "        <key>#</key>\r\n",
        "        <value>Pivot</value>\r\n",
        "      </text>\r\n",
        "      <key xsi:type=\"core:UndefinedValue\"/>\r\n",
        "    </properties>\r\n",
        "  </realExSeriesData>\r\n",
        "  <labelsColor xsi:type=\"core:ColorDef\">\r\n",
        "    <red>51</red>\r\n",
        "    <green>51</green>\r\n",
        "    <blue>51</blue>\r\n",
        "  </labelsColor>\r\n",
        "  <ttlBorder xsi:type=\"core:BorderDef\"/>\r\n",
        "  <chFont xsi:type=\"core:FontRef\">\r\n",
        "    <font>Style.NormalTextFont</font>\r\n",
        "    <height>6.0</height>\r\n",
        "  </chFont>\r\n",
        "  <baseVal>0</baseVal>\r\n",
        "  <translucenceMode>Auto</translucenceMode>\r\n",
        "  <valuesAxis>\r\n",
        "    <interval>\r\n",
        "      <leftIsNum>true</leftIsNum>\r\n",
        "      <rightIsNum>true</rightIsNum>\r\n",
        "    </interval>\r\n",
        "  </valuesAxis>\r\n",
        "</chart:Chart>\r\n",
    );
    body.as_bytes().to_vec()
}

#[test]
fn edt_sidecar_roundtrip_byte_exact() {
    let src = edt_sidecar_doc();
    let cs = read_chart_sidecar(&src).expect("read chart sidecar");
    assert_eq!(cs.kind, "Chart");
    let out = write_chart_sidecar(&cs).expect("write chart sidecar");
    assert_eq!(
        String::from_utf8_lossy(&out),
        String::from_utf8_lossy(&src),
        "EDT сайдкар эхо byte-exact"
    );
}

#[test]
fn designer_to_edt_transcode_synthesizes_model_constants() {
    let src = designer_doc();
    let d = parse(&src).expect("parse designer settings");
    let cs = read_designer_chart_settings("Chart", &d.root).expect("read designer chart");
    let out = write_chart_sidecar(&cs).expect("transcode to EDT sidecar");
    let s = String::from_utf8(out).expect("utf8");
    // Синтез EDT-only констант модели.
    assert!(
        s.contains("<translucenceMode>Auto</translucenceMode>"),
        "{s}"
    );
    assert!(
        s.contains("<bubbleSizing>IncreaseArea</bubbleSizing>"),
        "{s}"
    );
    assert!(s.contains("<valuesReferenceLines/>"), "{s}");
    // Designer-пустая ось материализуется interval-константой.
    assert!(s.contains("<leftIsNum>true</leftIsNum>"), "{s}");
    // Дефолт-шкалы: titleArea без font + titlePlacement.
    assert!(
        s.contains("<titlePlacement>SpecialArea</titlePlacement>"),
        "{s}"
    );
    // xcore-омиссии: пустой title, isRandomizedNewValues=true, авто-цвет серии — опущены.
    assert!(!s.contains("<title>"), "{s}");
    assert!(!s.contains("isRandomizedNewValues"), "{s}");
    // Дизайнер-цвет и style-шрифт транскодируются в mcore-формы.
    assert!(s.contains("xsi:type=\"core:ColorDef\""), "{s}");
    assert!(s.contains("<font>Style.NormalTextFont</font>"), "{s}");
    assert!(s.contains("<height>6.0</height>"), "{s}");
    // BigDecimal-поле эмитится и нулевым.
    assert!(s.contains("<baseVal>0</baseVal>"), "{s}");
}

#[test]
fn unknown_designer_field_is_loud() {
    let src = String::from_utf8(designer_doc()).unwrap().replace(
        "\t<d4p1:chartType>",
        "\t<d4p1:mystery>1</d4p1:mystery>\r\n\t<d4p1:chartType>",
    );
    let d = parse(src.as_bytes()).expect("parse");
    let err = read_designer_chart_settings("Chart", &d.root).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("незнакомое поле") && msg.contains("mystery") && msg.contains("§1.0"),
        "got: {msg}"
    );
}

#[test]
fn unknown_sidecar_field_is_loud() {
    let src = String::from_utf8(edt_sidecar_doc())
        .unwrap()
        .replace("  <baseVal>", "  <mystery>1</mystery>\r\n  <baseVal>");
    let err = read_chart_sidecar(src.as_bytes()).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("незнакомое поле") && msg.contains("mystery") && msg.contains("§1.0"),
        "got: {msg}"
    );
}

#[test]
fn sidecar_model_order_violation_is_loud() {
    // labelsColor (rank 18) ПОСЛЕ translucenceMode (rank 111) — нарушение модельного порядка.
    let src = String::from_utf8(edt_sidecar_doc())
        .unwrap()
        .replace("  <baseVal>0</baseVal>\r\n", "")
        .replace("  <valuesAxis>", "  <baseVal>0</baseVal>\r\n  <valuesAxis>");
    // baseVal (64) после translucenceMode (111) → отказ.
    let err = read_chart_sidecar(src.as_bytes()).unwrap_err();
    assert!(err.to_string().contains("модельный порядок"), "got: {err}");
}
