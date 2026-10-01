//! Чтение/запись КАРТЫ МАРШРУТА бизнес-процесса (`BusinessProcess`) как сайдкара дескриптора
//! (§1.2/§4) — сиблинг [`crate::schedule_read`]/[`crate::xdto_read`]: «дескриптор-read метаданных
//! не трогает тело; whole-config-конвейер присоединяет его отдельным проходом».
//!
//! # Раскладка (RE SSL: `BusinessProcesses/Задание`; ERP: 19 бизнес-процессов — ОБА диалекта)
//! * **EDT** — `<obj-dir>/Flowchart.scheme`: БЕЗ BOM, CRLF — И В РАЗМЕТКЕ, И ВНУТРИ ТЕКСТОВЫХ
//!   УЗЛОВ; корневой `<GraphicalSchema>` БЕЗ атрибута `version`.
//! * **Designer** — `<dir>/<Name>/Ext/Flowchart.xml`: С BOM, CRLF в РАЗМЕТКЕ, но LF ВНУТРИ
//!   текстовых узлов (многострочный `<v8:content>`); корень несёт `version="2.20|2.21"` —
//!   ПОСЛЕДНИМ атрибутом (witnessed 19/19 ERP + SSL; версия формата дампа — ВХОД чтения, см.
//!   [`crate::sidecar_version`]; иная позиция атрибута не воспроизвелась бы byte-exact и
//!   §1.0-отвергается самопроверкой).
//!
//! Иными словами `EDT-файл == Designer-файл` минус BOM, минус `version=`, плюс CRLF-нормализация
//! текстовых узлов — ОДИН кодек обслуживает оба диалекта (§1.6). ВНУТРИ схемы структурных
//! отличий 2.20 от 2.21 не witnessed (ERP 19/19 канонизируются и воспроизводятся байт-в-байт
//! тем же кодеком); появись они — самопроверка откажет громко.
//!
//! # Канон
//! КАНОН = EDT-форма БЕЗ CRLF в текстовых узлах: без BOM, без `version=`, разметка CRLF, текстовые
//! узлы LF (инвариант канонического IR — «текст в IR всегда LF»; ровно так же канонизирует
//! [`crate::dcs_read`], у которого тот же перекос диалектов). Оба диалекта дают РАВНЫЙ канон.
//!
//! # §1.0-самопроверка на read
//! Пере-сериализация канона в ИСХОДНЫЙ диалект ОБЯЗАНА воспроизвести исходные байты — иначе
//! ГРОМКИЙ отказ (а не тихая нормализация непрошенного кодирования). Это и есть гарантия
//! byte-exact round-trip'а обоих xml→xml-направлений.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::MetadataObject;
use morph1c_core::version::FormatVersion;

use crate::ConvertError;

/// Вид, несущий карту маршрута (единственный — RE: сайдкар есть только у `BusinessProcesses/`).
const FLOWCHART_KIND: &str = "BusinessProcess";
/// Имя EDT-сайдкара (рядом с `.mdo`).
const EDT_FILE: &str = "Flowchart.scheme";
/// Имя Designer-сайдкара (внутри `Ext/`).
const DESIGNER_FILE: &str = "Flowchart.xml";
/// UTF-8 BOM — Designer-сайдкар его несёт, EDT нет.
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
/// Префикс атрибута версии схемы, который несёт ТОЛЬКО Designer-корень (значение —
/// witnessed-версия формата дампа: `2.20` ERP / `2.21` SSL; ср. [`crate::schedule_read`]).
const DESIGNER_VERSION_PREFIX: &str = " version=\"";

/// Подгрузить карту маршрута (если сайдкар существует) в `obj.flowchart`. Не-`BusinessProcess`
/// виды и cf (контейнер) — no-op; BusinessProcess БЕЗ сайдкара — тоже no-op (witnessed: тогда
/// платформа НЕ эмитит элемент `.7` вовсе).
///
/// §1.0: сайдкар, чья пере-сериализация не воспроизводит исходные байты → ГРОМКИЙ отказ.
pub fn attach_flowchart(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if kind != FLOWCHART_KIND || format == Format::Cf {
        return Ok(());
    }
    let Some(path) = sidecar_path(format, descriptor_path) else {
        return Ok(());
    };
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let read_err = |reason: String| ConvertError::Read {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason,
    };
    let (canon, src_version) = canonicalize(format, &bytes)
        .map_err(|e| read_err(format!("flowchart {}: {e}", path.display())))?;
    // §1.0-самопроверка: канон обязан пере-сериализоваться в ИСХОДНЫЕ байты (Designer —
    // ВЕРСИЕЙ ИСТОЧНИКА: версия — свойство файла, детектится из его корня, §1.6).
    let back = match (format, src_version) {
        (Format::Designer, Some(v)) => serialize_designer(&canon, v),
        _ => serialize(format, &canon),
    };
    if back != bytes {
        return Err(read_err(format!(
            "flowchart {} does not round-trip byte-exactly through the IR (the sidecar carries a \
             byte-shape this codec does not model — refusing to silently normalize it, §1.0)",
            path.display()
        )));
    }
    if obj.flowchart.is_some() {
        return Err(read_err(
            "object already carries a flowchart before the sidecar attach (unexpected — the \
             descriptor projection must not populate it)"
                .into(),
        ));
    }
    obj.flowchart = Some(canon);
    Ok(())
}

/// Write-side mirror of [`attach_flowchart`]: эмитить `obj.flowchart` рядом с только что
/// записанным дескриптором в раскладке/кодировке ЦЕЛЕВОГО формата. cf (контейнер — тело собирает
/// cf-ассемблер) и объект без карты — no-op. §1.0: карта у вида БЕЗ раскладки сайдкара —
/// типизированный отказ (тихий дроп запрещён).
pub fn write_flowchart(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    let Some(canon) = obj.flowchart.as_ref() else {
        return Ok(());
    };
    if format == Format::Cf {
        return Ok(());
    }
    if kind != FLOWCHART_KIND {
        return Err(ConvertError::Write {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason:
                "object carries a flowchart but only BusinessProcess has a witnessed flowchart \
                     sidecar layout (§1.0 — silent drop forbidden)"
                    .into(),
        });
    }
    let path = sidecar_path(format, descriptor_out).ok_or_else(|| ConvertError::Write {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason: "descriptor path has no parent/stem to anchor the flowchart sidecar at".into(),
    })?;
    crate::form_write::write_file(&path, &serialize(format, canon))
}

/// Путь сайдкара относительно дескриптора: EDT `<obj-dir>/Flowchart.scheme`,
/// Designer `<dir>/<Name>/Ext/Flowchart.xml`.
fn sidecar_path(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        Format::Edt => Some(descriptor_path.parent()?.join(EDT_FILE)),
        Format::Designer => Some(
            descriptor_path
                .parent()?
                .join(descriptor_path.file_stem()?)
                .join("Ext")
                .join(DESIGNER_FILE),
        ),
        Format::Cf => None,
    }
}

/// Дисковые байты диалекта → (КАНОН, witnessed-версия формата источника — ТОЛЬКО у Designer;
/// см. модульный docstring). `pub(crate)`: тот же `<GraphicalSchema>`-диалект несёт сайдкар
/// GraphicalSchema-МАКЕТА (`crate::graph_template_read`) — один кодек на оба (§1.6).
pub(crate) fn canonicalize(
    format: Format,
    bytes: &[u8],
) -> Result<(Vec<u8>, Option<FormatVersion>), String> {
    match format {
        // EDT: BOM'а нет; CRLF в текстовых узлах → LF (структурные CRLF не трогаем).
        Format::Edt => {
            if bytes.starts_with(BOM) {
                return Err("EDT sidecar carries a BOM (witnessed: it does NOT) — §1.0".into());
            }
            Ok((
                crate::dcs_read::transcode_text_node_newlines(bytes, false),
                None,
            ))
        }
        // Designer: снять BOM и корневой `version=` — его ЗНАЧЕНИЕ детектится как witnessed
        // версия формата (2.20 ERP / 2.21 SSL; текстовые узлы УЖЕ LF — канон).
        Format::Designer => {
            let body = bytes.strip_prefix(BOM).ok_or_else(|| {
                "Designer sidecar carries no BOM (witnessed: it DOES) — §1.0".to_string()
            })?;
            let text = std::str::from_utf8(body).map_err(|e| format!("not UTF-8: {e}"))?;
            // Искать `version=` ТОЛЬКО внутри КОРНЕВОГО тега: ПЕРЕД ним стоит XML-пролог
            // `<?xml version="1.0"…`, и наивный поиск с начала файла нашёл бы «1.0» вместо
            // версии формата (ср. `pipeline::source_format_version`).
            let root_start = text.find("<GraphicalSchema").ok_or_else(|| {
                "Designer sidecar has no <GraphicalSchema …> root tag (§1.0)".to_string()
            })?;
            let root_end = root_tag_end(text).ok_or_else(|| {
                "Designer sidecar has no <GraphicalSchema …> root tag (§1.0)".to_string()
            })?;
            let head = &text[root_start..root_end];
            let vpos = head.find(DESIGNER_VERSION_PREFIX).ok_or_else(|| {
                format!(
                    "Designer sidecar root carries no {DESIGNER_VERSION_PREFIX:?}…\" attribute \
                     (witnessed: it does; an unwitnessed schema version would be silently \
                     restamped with the target's) — §1.0"
                )
            })?;
            let vstart = vpos + DESIGNER_VERSION_PREFIX.len();
            let vend = vstart
                + head[vstart..]
                    .find('"')
                    .ok_or_else(|| "unterminated root version attribute (§1.0)".to_string())?;
            let version =
                crate::sidecar_version::parse_witnessed(&head[vstart..vend], "flowchart")?;
            let mut canon = String::with_capacity(text.len());
            canon.push_str(&text[..root_start + vpos]);
            canon.push_str(&text[root_start + vend + 1..]);
            Ok((canon.into_bytes(), Some(version)))
        }
        Format::Cf => Err("cf is a container, not a file sidecar".into()),
    }
}

/// КАНОН → дисковые байты целевого диалекта (обратная [`canonicalize`]). Designer штампуется
/// версией АМБЬЕНТНОГО round-trip-таргета ([`crate::sidecar_version::write_target`]);
/// §1.0-самопроверка на read зовёт [`serialize_designer`] с ВЕРСИЕЙ ИСТОЧНИКА напрямую.
pub(crate) fn serialize(format: Format, canon: &[u8]) -> Vec<u8> {
    match format {
        // EDT: канон как есть, но текстовые узлы — обратно в CRLF.
        Format::Edt => crate::dcs_read::transcode_text_node_newlines(canon, true),
        Format::Designer => serialize_designer(canon, crate::sidecar_version::write_target()),
        Format::Cf => Vec::new(), // контейнер — не файловый сайдкар.
    }
}

/// Designer-байты из канона под ЗАДАННОЙ версией формата: BOM + ` version="…"` в корневой тег
/// (перед его закрывающей `>` — witnessed: version ПОСЛЕДНИЙ атрибут корня, 19/19 ERP + SSL).
pub(crate) fn serialize_designer(canon: &[u8], version: FormatVersion) -> Vec<u8> {
    let text = String::from_utf8_lossy(canon);
    let attr = format!("{DESIGNER_VERSION_PREFIX}{version}\"");
    let mut out = Vec::with_capacity(canon.len() + BOM.len() + attr.len());
    out.extend_from_slice(BOM);
    match root_tag_end(&text) {
        Some(at) => {
            out.extend_from_slice(text[..at].as_bytes());
            out.extend_from_slice(attr.as_bytes());
            out.extend_from_slice(text[at..].as_bytes());
        }
        None => out.extend_from_slice(canon),
    }
    out
}

/// Позиция закрывающей `>` КОРНЕВОГО тега `<GraphicalSchema …>` (куда Designer вставляет
/// `version=`). `None` — если корня нет (тогда сериализация отдаёт канон как есть, а
/// §1.0-самопроверка на read это поймает).
fn root_tag_end(text: &str) -> Option<usize> {
    let start = text.find("<GraphicalSchema")?;
    text[start..].find('>').map(|i| start + i)
}

#[cfg(any())]
mod tests {
    use super::*;

    /// Витнессированные дисковые байты ОДНОЙ И ТОЙ ЖЕ карты в двух диалектах (сжатая до одного
    /// многострочного `<v8:content>` выжимка SSL `Задание` — именно она несёт весь перекос
    /// диалектов: BOM, `version=` и переводы строк ВНУТРИ текстового узла).
    const CANON: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<GraphicalSchema xmlns=\"http://v8.1c.ru/8.3/xcf/scheme\">\r\n",
        "\t<BackColor>style:FieldBackColor</BackColor>\r\n",
        "\t<Items>\r\n",
        "\t\t<Decoration id=\"25\">\r\n",
        "\t\t\t<Properties>\r\n",
        "\t\t\t\t<Title>\r\n",
        "\t\t\t\t\t<v8:item>\r\n",
        "\t\t\t\t\t\t<v8:content>первая строка\nвторая строка</v8:content>\r\n",
        "\t\t\t\t\t</v8:item>\r\n",
        "\t\t\t\t</Title>\r\n",
        "\t\t\t</Properties>\r\n",
        "\t\t</Decoration>\r\n",
        "\t</Items>\r\n",
        "</GraphicalSchema>"
    );

    fn designer_bytes() -> Vec<u8> {
        let with_version = CANON.replacen(
            "<GraphicalSchema xmlns=\"http://v8.1c.ru/8.3/xcf/scheme\">",
            "<GraphicalSchema xmlns=\"http://v8.1c.ru/8.3/xcf/scheme\" version=\"2.21\">",
            1,
        );
        let mut out = BOM.to_vec();
        out.extend_from_slice(with_version.as_bytes());
        out
    }

    /// EDT-файл = канон с CRLF и ВНУТРИ текстового узла.
    fn edt_bytes() -> Vec<u8> {
        CANON
            .replace(
                "первая строка\nвторая строка",
                "первая строка\r\nвторая строка",
            )
            .into_bytes()
    }

    /// §1.6: ОБА диалекта дают РАВНЫЙ канон; Designer-версия детектится из корня.
    #[test]
    fn both_dialects_yield_the_same_canonical_flowchart() {
        let (e, ev) = canonicalize(Format::Edt, &edt_bytes()).expect("edt");
        let (d, dv) = canonicalize(Format::Designer, &designer_bytes()).expect("designer");
        assert_eq!(e, d, "edt canon == designer canon (§1.6)");
        assert_eq!(ev, None, "EDT carries no format version");
        assert_eq!(dv, Some(morph1c_core::version::SSL), "detected 2.21");
        assert_eq!(String::from_utf8(e).unwrap(), CANON);
    }

    /// Byte-exact round-trip в ОБОИХ диалектах — и КРОСС-диалектно (§1.6): это и есть
    /// §1.0-самопроверка на read.
    #[test]
    fn serialize_reproduces_the_witnessed_bytes() {
        let (canon, _) = canonicalize(Format::Designer, &designer_bytes()).expect("designer");
        assert_eq!(serialize(Format::Designer, &canon), designer_bytes());
        assert_eq!(serialize(Format::Edt, &canon), edt_bytes());
        let (from_edt, _) = canonicalize(Format::Edt, &edt_bytes()).expect("edt");
        assert_eq!(serialize(Format::Edt, &from_edt), edt_bytes());
        assert_eq!(serialize(Format::Designer, &from_edt), designer_bytes());
    }

    /// ERP-витнесс: корень несёт `version="2.20"` ПОСЛЕДНИМ атрибутом (19/19 бизнес-процессов)
    /// — детект + byte-exact re-serialize ВЕРСИЕЙ ИСТОЧНИКА; write-side под амбьентным
    /// ERP-таргетом даёт те же байты.
    #[test]
    fn erp_2_20_designer_flowchart_roundtrips() {
        let erp = String::from_utf8(designer_bytes())
            .unwrap()
            .replace("version=\"2.21\"", "version=\"2.20\"");
        let (canon, v) = canonicalize(Format::Designer, erp.as_bytes()).expect("designer 2.20");
        assert_eq!(v, Some(morph1c_core::version::ERP), "detected 2.20");
        assert_eq!(
            String::from_utf8(canon.clone()).unwrap(),
            CANON,
            "same canon as 2.21"
        );
        assert_eq!(
            serialize_designer(&canon, morph1c_core::version::ERP),
            erp.as_bytes(),
            "re-serialized under the SOURCE version"
        );
        let via_ambient =
            morph1c_core::version::with_roundtrip_target(morph1c_core::version::ERP, || {
                serialize(Format::Designer, &canon)
            });
        assert_eq!(
            via_ambient,
            erp.as_bytes(),
            "ambient ERP target restamps 2.20"
        );
    }

    /// §1.0: Designer-сайдкар с НЕвитнессированной версией схемы → ГРОМКИЙ отказ (иначе она была
    /// бы молча перештампована версией таргета).
    #[test]
    fn unwitnessed_designer_version_is_refused() {
        let bad = String::from_utf8(designer_bytes())
            .unwrap()
            .replace("version=\"2.21\"", "version=\"2.19\"");
        let err = canonicalize(Format::Designer, bad.as_bytes()).unwrap_err();
        assert!(err.contains("version"), "{err}");
    }
}
