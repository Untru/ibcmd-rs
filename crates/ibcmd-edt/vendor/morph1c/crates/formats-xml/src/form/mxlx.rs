//! EDT-сайдкар табличного документа РЕКВИЗИТА формы —
//! `Forms/<N>/Attributes/<attr>/ExtInfo/SpreadsheetData.mxlx` (byte-exact read/write).
//!
//! # Асимметрия хранения (RE: coverage/s10_forms Форма_Группы + SSL 10/10 сайдкаров)
//! Designer держит тело табличного документа ИНЛАЙН в `Ext/Form.xml` — `<Settings xmlns:mxl=…
//! xsi:type="mxl:SpreadsheetDocument">` у `<Attribute>` ([`MxlSpreadsheetSettings`], читает/пишет
//! форм-кодек). EDT в `Form.form` несёт лишь ПУСТОЙ маркер `<extInfo
//! xsi:type="form:SpreadsheetDocumentExtInfo"/>`, а само тело — ЭТОТ сайдкар. Содержимое обеих
//! сериализаций сверено 1:1 (11/11 пар корпуса SSL+coverage): та же структура
//! `[languageSettings]+columns+rowsItem+[templateMode]+vgRows`, только Designer пишет её с
//! `mxl:`-префиксом, а сайдкар — с ДЕФОЛТНЫМ ns (`xmlns="…/spreadsheet"` на корне `<document>`).
//! Транскодинг inline⟷сайдкар выполняет pipeline (`form_read`/`form_write`) через общий под-IR
//! [`MxlSpreadsheetSettings`].
//!
//! Сайдкар ОПЦИОНАЛЕН: маркер есть у ВСЕХ реквизитов скаляр-типа `SpreadsheetDocument` (36 в
//! SSL), сайдкар — лишь у несущих сохранённое тело (10 в SSL; ⟺ Designer-инлайн `<Settings>`).
//!
//! # Envelope (hexdump-сверено: coverage 837 B + SSL 545 B варианты)
//! Без BOM, CRLF, TAB-отступ, БЕЗ trailing EOL — стиль платформенного Designer-сериализатора
//! (не EDT `.form`: там 2 пробела + trailing EOL); политика escape взята Designer-ская
//! (`>`→`&gt;`, кавычка литеральная) — в witnessed-текстах спец-символов нет.

use super::read::{
    capture_full_body, claim_root_ns, is_minimal_spreadsheet_body, read_spreadsheet_settings_body,
    rich_spreadsheet_settings, unclaimed_labels,
};
use super::write::spreadsheet_body_children;
use super::{FormError, MXL_NS_URI, XSI_NS_URI};
use crate::emit::{render, Envelope, OutElement};
use crate::{parse, EolStyle};
use morph1c_core::ir::form::MxlSpreadsheetSettings;

/// XML-декларация сайдкара (та же, что у форм).
const MXLX_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>";

/// Полный фиксированный ns-блок корня `<document>` (7 объявлений; spreadsheet-ns — ДЕФОЛТНЫЙ).
/// Порядок и URI сверены по корпусу (coverage + SSL 10/10) — воспроизводятся byte-exact.
const MXLX_ROOT_NS: &[(&str, &str)] = &[
    ("xmlns", MXL_NS_URI),
    ("xmlns:pal", "http://v8.1c.ru/8.1/data/ui/colors/palette"),
    ("xmlns:style", "http://v8.1c.ru/8.1/data/ui/style"),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:xsi", XSI_NS_URI),
];

/// Envelope `SpreadsheetData.mxlx` (см. модульный docstring).
fn mxlx_envelope() -> Envelope {
    Envelope {
        bom: false,
        eol: "\r\n",
        indent_unit: "\t",
        decl: MXLX_DECL,
        trailing_eol: false,
        escape_gt: true,
        escape_quot: false,
        text_eol: "\n",
    }
}

/// Прочитать байты `SpreadsheetData.mxlx` → [`MxlSpreadsheetSettings`] (byte-exact-обратимо).
/// §1.0: envelope/корень/ns сверяются, каждый узел claim'ится или ошибка (не Blob, не skip).
pub fn read_spreadsheet_mxlx(bytes: &[u8]) -> Result<MxlSpreadsheetSettings, FormError> {
    let descriptor = parse(bytes)?;
    let env = descriptor.bytes_env;
    if env.bom {
        return Err(FormError::Envelope("mxlx: unexpected BOM".into()));
    }
    if env.eol != EolStyle::Crlf {
        return Err(FormError::Envelope(format!(
            "mxlx: must be CRLF, found {:?}",
            env.eol
        )));
    }
    match descriptor.decl.as_deref() {
        Some(d) if d == MXLX_DECL => {}
        other => {
            return Err(FormError::Envelope(format!(
                "mxlx: unexpected decl: {other:?}"
            )))
        }
    }
    let root = descriptor.root;
    if !root.prefix.is_empty() || root.local != "document" {
        return Err(FormError::Envelope(format!(
            "mxlx: unexpected root <{}{}{}>",
            root.prefix,
            if root.prefix.is_empty() { "" } else { ":" },
            root.local
        )));
    }
    root.claim();
    // `xmlns:pal` — ОПЦИОНАЛЕН: SSL несёт его 10/10, ERP-сайдкары идут БЕЗ него (ценз 18/18,
    // witness Catalog.Номенклатура.ФормаЭлемента). Присутствие переносится presence-битом
    // (`MxlSpreadsheetSettings::envelope_without_pal`) — re-emit byte-exact в обе стороны;
    // отсутствующий НЕ выдумываем. Остальные 6 объявлений — обязательный каркас.
    let required: Vec<(&str, &str)> = MXLX_ROOT_NS
        .iter()
        .copied()
        .filter(|(name, _)| *name != "xmlns:pal")
        .collect();
    claim_root_ns(&root, &required)?;
    let pal_uri = MXLX_ROOT_NS
        .iter()
        .find(|(name, _)| *name == "xmlns:pal")
        .map(|(_, u)| *u)
        .expect("MXLX_ROOT_NS carries xmlns:pal");
    let envelope_without_pal = match root.attr("xmlns:pal") {
        Some(a) => {
            if a.value != pal_uri {
                return Err(FormError::Envelope(format!(
                    "mxlx root xmlns:pal={:?} want {pal_uri:?}",
                    a.value
                )));
            }
            a.claimed.set(true);
            false
        }
        None => true,
    };
    // БОГАТЫЙ сайдкар (живой заполненный документ) не влезает в минимальный конверт — захватываем
    // ПОЛНОЕ тело generic-tree'ем (byte-exact EDT-флейвор: default-ns, алфавитный порядок атрибутов,
    // инлайн-редекл `xmlns:v8`/`xmlns:v8ui`). Минимальный (пустой) — прежним структурным путём
    // (SSL/ERP-минимум byte-exact не тронут; presence-бит pal переносится в обоих вариантах).
    let ss = if is_minimal_spreadsheet_body(&root, "") {
        let mut ss = read_spreadsheet_settings_body(&root, "")?;
        ss.envelope_without_pal = envelope_without_pal;
        ss
    } else {
        rich_spreadsheet_settings(capture_full_body(&root)?, envelope_without_pal)
    };
    // §1.0 тотальность по всему дереву сайдкара.
    let leftover = root.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "{leftover} unconsumed node(s) in SpreadsheetData.mxlx (no passthrough/Raw — §1.0): {:?}",
            unclaimed_labels(&root)
        )));
    }
    Ok(ss)
}

/// Записать [`MxlSpreadsheetSettings`] в байты `SpreadsheetData.mxlx` (byte-exact к
/// платформенному экспорту: оба witnessed-варианта — 545 B минимальный и 837 B с
/// languageSettings+templateMode — регенерируются в точности).
pub fn write_spreadsheet_mxlx(ss: &MxlSpreadsheetSettings) -> Vec<u8> {
    let mut root = OutElement::branch("", "document");
    for (name, uri) in MXLX_ROOT_NS {
        // presence-точный re-emit: ERP-flavored сайдкар (без pal) не получает выдуманного
        // объявления; SSL-flavor (default false) — прежний 7-ns блок byte-exact.
        if ss.envelope_without_pal && *name == "xmlns:pal" {
            continue;
        }
        root = root.attr(*name, *uri);
    }
    for c in spreadsheet_body_children("", ss) {
        root.push(c);
    }
    render(&mxlx_envelope(), &root)
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::form::{MxlLanguageInfo, MxlLanguageSettings};

    /// Минимальный витнесс (SSL 545 B): без languageSettings/templateMode.
    fn minimal_bytes() -> Vec<u8> {
        let s = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n\
\t<columns>\r\n\t\t<size>0</size>\r\n\t</columns>\r\n\
\t<rowsItem>\r\n\t\t<index>0</index>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>\r\n\t\t</row>\r\n\t</rowsItem>\r\n\
\t<vgRows>0</vgRows>\r\n</document>";
        s.as_bytes().to_vec()
    }

    #[test]
    fn minimal_roundtrip_byte_exact() {
        let src = minimal_bytes();
        let ss = read_spreadsheet_mxlx(&src).expect("read minimal mxlx");
        assert!(ss.language_settings.is_none());
        assert_eq!(ss.columns_size, "0");
        assert_eq!(ss.rows_index, "0");
        assert!(ss.row_empty);
        assert_eq!(ss.template_mode, None);
        assert_eq!(ss.vg_rows, "0");
        assert_eq!(write_spreadsheet_mxlx(&ss), src, "minimal mxlx byte-exact");
    }

    /// ERP-флавор (без `xmlns:pal`) минимального сайдкара (ценз ERP 18/18 без pal; тело идентично
    /// SSL-минимуму МИНУС строка pal). Собран escape'ами (не литеральным отступом).
    fn nopal_minimal_bytes() -> Vec<u8> {
        let s = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\r\n\
\t<columns>\r\n\t\t<size>0</size>\r\n\t</columns>\r\n\
\t<rowsItem>\r\n\t\t<index>0</index>\r\n\t\t<row>\r\n\t\t\t<empty>true</empty>\r\n\t\t</row>\r\n\t</rowsItem>\r\n\
\t<vgRows>0</vgRows>\r\n</document>";
        s.as_bytes().to_vec()
    }

    #[test]
    fn nopal_minimal_roundtrip_byte_exact() {
        // §1.0: pal-опциональность byte-exact на ERP-flavored сайдкаре (present-путь — см.
        // minimal_roundtrip_byte_exact; тут absent-путь).
        let src = nopal_minimal_bytes();
        let ss = read_spreadsheet_mxlx(&src).expect("read nopal mxlx");
        assert!(ss.envelope_without_pal, "pal absent ⇒ presence-бит взведён");
        assert!(ss.language_settings.is_none());
        assert_eq!(ss.template_mode, None);
        assert_eq!(
            write_spreadsheet_mxlx(&ss),
            src,
            "nopal mxlx byte-exact (конверт БЕЗ xmlns:pal)"
        );
        // Взаимо-исключимость флейворов: SSL-writer НЕ должен ронять pal, ERP-writer — обязан.
        let text = String::from_utf8(write_spreadsheet_mxlx(&ss)).unwrap();
        assert!(!text.contains("xmlns:pal"), "ERP-flavor не объявляет pal");
    }

    #[test]
    fn pal_presence_bit_flips_only_pal() {
        // Тот же IR с/без бита ⇒ дельта РОВНО одна ns-строка pal.
        let mut ss = read_spreadsheet_mxlx(&nopal_minimal_bytes()).unwrap();
        ss.envelope_without_pal = false;
        let with = String::from_utf8(write_spreadsheet_mxlx(&ss)).unwrap();
        assert!(with.contains(
            " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\""
        ));
        ss.envelope_without_pal = true;
        let without = String::from_utf8(write_spreadsheet_mxlx(&ss)).unwrap();
        assert_eq!(
            with.replace(
                " xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\"",
                ""
            ),
            without,
            "бит убирает ТОЛЬКО объявление pal, прочее byte-exact"
        );
    }

    #[test]
    fn full_variant_roundtrip_byte_exact() {
        // Полный витнесс (coverage 837 B): languageSettings + templateMode.
        let ss = MxlSpreadsheetSettings {
            envelope_without_pal: false,
            language_settings: Some(MxlLanguageSettings {
                current_language: "ru".into(),
                default_language: "ru".into(),
                languages: vec![MxlLanguageInfo {
                    id: "ru".into(),
                    code: "Русский".into(),
                    description: "Русский".into(),
                }],
            }),
            columns_size: "0".into(),
            rows_index: "0".into(),
            row_empty: true,
            template_mode: Some(true),
            vg_rows: "0".into(),
            full_body: None,
        };
        let bytes = write_spreadsheet_mxlx(&ss);
        let back = read_spreadsheet_mxlx(&bytes).expect("read back");
        assert_eq!(back, ss, "IR round-trips through mxlx bytes");
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\t<languageSettings>\r\n\t\t<currentLanguage>ru</currentLanguage>"));
        assert!(text.contains(
            "\t<templateMode>true</templateMode>\r\n\t<vgRows>0</vgRows>\r\n</document>"
        ));
        assert!(text.ends_with("</document>"), "no trailing EOL");
    }

    #[test]
    fn coverage_fixture_roundtrip_byte_exact() {
        // Реальный committed-витнесс: coverage/edt/s10_forms Форма_Группы ТабличныйДокумент.
        let p = morph1c_testkit_fixture(
            "coverage/edt/s10_forms/src/CommonForms/Форма_Группы/Attributes/ТабличныйДокумент/ExtInfo/SpreadsheetData.mxlx",
        );
        let src = match p {
            Some(b) => b,
            None => {
                eprintln!("coverage_fixture_roundtrip_byte_exact INFRA-SKIP (corpus absent)");
                return;
            }
        };
        let ss = read_spreadsheet_mxlx(&src).expect("read coverage mxlx");
        assert!(
            ss.language_settings.is_some(),
            "coverage variant carries languageSettings"
        );
        assert_eq!(ss.template_mode, Some(true));
        assert_eq!(write_spreadsheet_mxlx(&ss), src, "coverage mxlx byte-exact");
    }

    #[test]
    fn bad_envelope_is_loud() {
        // §1.0: нарушение конверта (не-CRLF/BOM/чужой корень) — по-прежнему громкий отказ.
        let bom = {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(&minimal_bytes());
            v
        };
        assert!(read_spreadsheet_mxlx(&bom).is_err(), "BOM must be rejected");
        let wrong_root = String::from_utf8(minimal_bytes())
            .unwrap()
            .replace("<document ", "<spreadsheet ")
            .replace("</document>", "</spreadsheet>");
        assert!(
            read_spreadsheet_mxlx(wrong_root.as_bytes()).is_err(),
            "wrong root must be rejected"
        );
    }

    #[test]
    fn extra_top_level_becomes_rich_and_roundtrips() {
        // Всё, что выходит за минимальный конверт (здесь — лишний top-level `<mystery>`), НЕ
        // проглатывается: тело становится БОГАТЫМ (generic-tree) и регенерируется byte-exact
        // (§1.0 «читается ПОЛНОСТЬЮ» + «round-trip байт-точен» — сильнее прежнего loud-guard'а).
        let s = String::from_utf8(minimal_bytes()).unwrap().replace(
            "\t<vgRows>0</vgRows>",
            "\t<vgRows>0</vgRows>\r\n\t<mystery>1</mystery>",
        );
        let ss = read_spreadsheet_mxlx(s.as_bytes()).expect("rich body captured");
        assert!(ss.full_body.is_some(), "non-minimal ⇒ богатый generic-tree");
        assert_eq!(
            write_spreadsheet_mxlx(&ss),
            s.as_bytes(),
            "богатый generic-tree byte-exact (ничего не выброшено)"
        );
    }

    #[test]
    fn minimal_shape_still_detected() {
        // Гард диспетчера: минимальный документ РАСПОЗНАЁТСЯ как минимальный (⇒ структурный путь,
        // full_body=None, SSL/минимум byte-exact), богатый — нет.
        let min = read_spreadsheet_mxlx(&minimal_bytes()).unwrap();
        assert!(min.full_body.is_none(), "минимум ⇒ структурный путь");
        let rich = String::from_utf8(minimal_bytes()).unwrap().replace(
            "\t\t<size>0</size>",
            "\t\t<size>1</size>\r\n\t\t<columnsItem>\r\n\t\t\t<index>0</index>\r\n\t\t\t\
             <column>\r\n\t\t\t\t<formatIndex>1</formatIndex>\r\n\t\t\t</column>\r\n\t\t</columnsItem>",
        );
        let rss = read_spreadsheet_mxlx(rich.as_bytes()).expect("rich columns captured");
        assert!(rss.full_body.is_some(), "columnsItem ⇒ богатый");
        assert_eq!(
            write_spreadsheet_mxlx(&rss),
            rich.as_bytes(),
            "богатые columns byte-exact"
        );
    }

    #[test]
    fn rich_edt_fixtures_roundtrip_byte_exact() {
        // Реальные платформенные ERP-витнессы (оба структурных варианта богатого тела): DataProcessor
        // мирАнализ (6 КБ — font/format/tl-ячейки/line/v8:item/v8ui:style) и Catalog Номенклатура
        // (11 КБ — namedItem/merge/area/w/r). Полный EDT-путь read→write byte-exact.
        for rel in [
            "ERP/edt/src/DataProcessors/мирАнализПлатежекПоИдентификаторам/Forms/Форма/Attributes/СписокТекста/ExtInfo/SpreadsheetData.mxlx",
            "ERP/edt/src/Catalogs/Номенклатура/Forms/ФормаЭлемента/Attributes/КарточкаНоменклатуры/ExtInfo/SpreadsheetData.mxlx",
        ] {
            let src = match morph1c_testkit_fixture(rel) {
                Some(b) => b,
                None => {
                    eprintln!("rich_edt_fixtures_roundtrip_byte_exact INFRA-SKIP (corpus absent): {rel}");
                    continue;
                }
            };
            let ss = read_spreadsheet_mxlx(&src).expect("read rich mxlx");
            assert!(ss.full_body.is_some(), "богатое тело ⇒ generic-tree: {rel}");
            assert!(ss.envelope_without_pal, "ERP-сайдкар без xmlns:pal: {rel}");
            assert_eq!(write_spreadsheet_mxlx(&ss), src, "rich EDT mxlx byte-exact: {rel}");
        }
    }

    #[test]
    fn rich_generic_capture_emit_both_flavors() {
        use super::super::read::capture_full_body;
        use super::super::write::mxl_node_to_out;
        // Синтетика (concat — реальный байтовый layout, без литерального отступа): захват+эмиссия
        // generic-tree byte-exact для ОБОИХ диалектных написаний. EDT-флейвор: default-ns + инлайн
        // xmlns:v8-редекл + self-closing font с 3 атрибутами В ПОРЯДКЕ. Designer-флейвор: mxl:-префикс
        // + foreign v8: без редекла + СЕМАНТИЧЕСКИЙ (не алфавитный) порядок атрибутов font.
        let edt_flavor = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<document xmlns=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\">\r\n",
            "\t<columns>\r\n\t\t<size>1</size>\r\n\t\t<columnsItem>\r\n\t\t\t<index>0</index>\r\n\t\t\t<column>\r\n\t\t\t\t<formatIndex>1</formatIndex>\r\n\t\t\t</column>\r\n\t\t</columnsItem>\r\n\t</columns>\r\n",
            "\t<tl>\r\n\t\t<v8:item xmlns:v8=\"http://v8.1c.ru/8.1/data/core\">\r\n\t\t\t<v8:lang>ru</v8:lang>\r\n\t\t\t<v8:content>Дата</v8:content>\r\n\t\t</v8:item>\r\n\t</tl>\r\n",
            "\t<font bold=\"false\" faceName=\"Arial\" height=\"10\"/>\r\n",
            "\t<vgRows>0</vgRows>\r\n</document>",
        );
        let designer_flavor = concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<Settings xmlns:mxl=\"http://v8.1c.ru/8.2/data/spreadsheet\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\">\r\n",
            "\t<mxl:columns>\r\n\t\t<mxl:size>1</mxl:size>\r\n\t\t<mxl:columnsItem>\r\n\t\t\t<mxl:index>0</mxl:index>\r\n\t\t\t<mxl:column>\r\n\t\t\t\t<mxl:formatIndex>1</mxl:formatIndex>\r\n\t\t\t</mxl:column>\r\n\t\t</mxl:columnsItem>\r\n\t</mxl:columns>\r\n",
            "\t<mxl:tl>\r\n\t\t<v8:item>\r\n\t\t\t<v8:lang>ru</v8:lang>\r\n\t\t\t<v8:content>Дата</v8:content>\r\n\t\t</v8:item>\r\n\t</mxl:tl>\r\n",
            "\t<mxl:font faceName=\"Arial\" height=\"10\" bold=\"false\"/>\r\n",
            "\t<mxl:vgRows>0</mxl:vgRows>\r\n</Settings>",
        );
        let env = mxlx_envelope();
        for body in [edt_flavor, designer_flavor] {
            let descriptor = parse(body.as_bytes()).expect("parse synthetic body");
            let root = descriptor.root;
            // Пересобрать корень с ИСХОДНЫМИ атрибутами (ns-блок) + generic-tree детей.
            let mut out = OutElement::branch(root.prefix.clone(), root.local.clone());
            for a in &root.attrs {
                out = out.attr(a.name.clone(), a.value.clone());
            }
            for n in capture_full_body(&root).unwrap() {
                out.push(mxl_node_to_out(&n, false));
            }
            assert_eq!(
                render(&env, &out),
                body.as_bytes(),
                "generic capture+emit byte-exact (флейвор-специфичный порядок/префикс/редекл)"
            );
        }
    }

    /// Прочитать fixtures-файл по относительному пути (env `MORPH1C_FIXTURES` / `../../.fixtures`),
    /// `None` если корпус отсутствует (INFRA-SKIP, не провал).
    fn morph1c_testkit_fixture(rel: &str) -> Option<Vec<u8>> {
        let root = std::env::var("MORPH1C_FIXTURES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.fixtures")
            });
        std::fs::read(root.join(rel)).ok()
    }
}
