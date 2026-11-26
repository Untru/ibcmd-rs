//! Чтение Designer-SIDECAR языка (`Languages/<Имя>.xml`) в IR-строку `F_LANGUAGES`.
//!
//! # Особенность: Language — inline-в-EDT, sidecar-в-Designer
//! В EDT данные языка лежат INLINE в корне (`<languages uuid=…>` в `Configuration.mdo`) —
//! их читает [`formats_xml::configuration::decode_languages_edt`]. В **Designer** тело языка
//! вынесено в ОТДЕЛЬНЫЙ файл `Languages/<Имя>.xml`, а корневой `Configuration.xml` несёт лишь
//! bare-ссылку `<Language>Имя</Language>` в `<ChildObjects>`. Поэтому Designer-проекция корня
//! ставит `F_LANGUAGES → None` (см. `formats/designer/src/metadata/configuration.rs`): данные
//! приходят НЕ из дескриптора корня, а из этого sidecar-ридера, вызываемого whole-config
//! read'ом (pipeline) после чтения корня.
//!
//! # Форма sidecar (RE: `.fixtures/probes/*/src/Languages/<Имя>.xml`) — byte-verified
//! ```xml
//! <MetaDataObject …>
//!   <Language uuid="db4a9ccb-…">
//!     <Properties>
//!       <Name>Русский</Name>
//!       <Synonym><v8:item><v8:lang>ru</v8:lang><v8:content>Русский</v8:content></v8:item></Synonym>
//!       <Comment/>
//!       <LanguageCode>ru</LanguageCode>
//!     </Properties>
//!   </Language>
//! </MetaDataObject>
//! ```
//!
//! # Результат — IR `F_LANGUAGES`-строка (§1.6: РОВНО та же форма, что даёт EDT-ридер)
//! `List([Str(uuid), Str(name), Str(languageCode), List([Str(lang), Str(text)]×n)])`, где
//! synonym — плоский вектор пар `(lang, text)` из `<Synonym>/<v8:item>`. Это гарантирует
//! кросс-форматную эквивалентность (X): Designer-read F_LANGUAGES == EDT-read F_LANGUAGES.
//!
//! # §1.0 — строго, никаких Raw/skip
//! Отсутствие обязательного узла (`@uuid`/`<Name>`/`<LanguageCode>`), неожиданный тег/атрибут,
//! или synonym-item без `<v8:lang>`/`<v8:content>` → типизированная [`DesignerError`]. Пустой
//! `<Synonym/>` и пустой `<Comment/>` допустимы (comment платформенно-пустой, не проецируется).

use formats_xml::{parse, Element};
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::version::FormatVersion;

use crate::common::{NAME_ELEMENT, PROPERTIES_ELEMENT, ROOT_ELEMENT};
use crate::{qname, DesignerError};

/// Local-name обёртки sidecar-языка под корнем `<MetaDataObject>`.
const LANGUAGE_ELEMENT: &str = "Language";

/// Разобрать байты Designer-sidecar языка (`Languages/<Имя>.xml`) в IR `F_LANGUAGES`-строку.
///
/// Возвращает `(name, row)`: `name` — имя языка (`<Name>` текст, == стем файла sidecar), `row`
/// — `List([uuid, name, languageCode, synonym-pairs])` (форма EDT-ридера, X by construction).
/// §1.0: любая структурная аномалия → [`DesignerError`] (не best-effort).
pub fn read_language_sidecar(bytes: &[u8]) -> Result<(String, PropertyValue), DesignerError> {
    let descriptor = parse(bytes)?;
    let root = descriptor.root;

    if !root.prefix.is_empty() || root.local != ROOT_ELEMENT {
        return Err(DesignerError::Envelope(format!(
            "language sidecar: unexpected root <{}>, want <{ROOT_ELEMENT}>",
            qname(&root)
        )));
    }

    // <Language uuid="…"> под корнем.
    let lang = root.child(LANGUAGE_ELEMENT).ok_or_else(|| {
        DesignerError::Envelope(format!("language sidecar: missing <{LANGUAGE_ELEMENT}>"))
    })?;
    if !lang.prefix.is_empty() {
        return Err(DesignerError::Envelope(format!(
            "language sidecar: <{LANGUAGE_ELEMENT}> must be unprefixed, got prefix {:?}",
            lang.prefix
        )));
    }
    let uuid = lang
        .attr("uuid")
        .ok_or_else(|| {
            DesignerError::Envelope("language sidecar: <Language> missing @uuid".into())
        })?
        .value
        .clone();
    // Валидировать hex-форму (§1.0: не best-effort). Значение хранится строкой (как читает EDT).
    let _ = crate::parse_uuid(&uuid)?;

    // <Properties> → <Name>, <Synonym>, <Comment>, <LanguageCode>.
    let props = lang.child(PROPERTIES_ELEMENT).ok_or_else(|| {
        DesignerError::Envelope(format!("language sidecar: missing <{PROPERTIES_ELEMENT}>"))
    })?;

    let mut name: Option<String> = None;
    let mut language_code: Option<String> = None;
    let mut synonym: Vec<PropertyValue> = Vec::new();

    for child in &props.children {
        if !child.prefix.is_empty() {
            return Err(DesignerError::Envelope(format!(
                "language sidecar: <Properties> child <{}> must be unprefixed (§1.0)",
                qname(child)
            )));
        }
        match child.local.as_str() {
            NAME_ELEMENT => {
                require_text_leaf(child, "Name")?;
                name = Some(child.text.clone());
            }
            "LanguageCode" => {
                require_text_leaf(child, "LanguageCode")?;
                language_code = Some(child.text.clone());
            }
            "Comment" => {
                // Платформенно-пустой `<Comment/>` (не проецируется в F_LANGUAGES). Текст, если
                // есть, игнорируется намеренно — comment не входит в IR-строку языка (как у EDT).
                if !child.children.is_empty() {
                    return Err(DesignerError::Envelope(
                        "language sidecar: <Comment> must be a leaf (§1.0)".into(),
                    ));
                }
            }
            "Synonym" => {
                synonym = read_synonym(child)?;
            }
            other => {
                return Err(DesignerError::Envelope(format!(
                    "language sidecar: unexpected <Properties> child <{other}> (§1.0)"
                )));
            }
        }
    }

    let name = name
        .ok_or_else(|| DesignerError::Envelope("language sidecar: missing <Name> (§1.0)".into()))?;
    let language_code = language_code.ok_or_else(|| {
        DesignerError::Envelope("language sidecar: missing <LanguageCode> (§1.0)".into())
    })?;

    let row = PropertyValue::List(vec![
        PropertyValue::Str(uuid),
        PropertyValue::Str(name.clone()),
        PropertyValue::Str(language_code),
        PropertyValue::List(synonym),
    ]);
    Ok((name, row))
}

/// Собрать namespace-блок Designer-корня `<MetaDataObject>` sidecar-языка ДЛЯ ТАРГЕТ-ВЕРСИИ.
///
/// Sidecar языка — обычный Designer-дескриптор, поэтому его обёртка версионна ровно так же, как
/// у любого другого (`ENVELOPE_PROFILES` — единственный реестр ns-блоков, FORMATS.md §1/§3).
/// Раньше блок был вшит КОНСТАНТОЙ версии 2.21: конверсия под таргет 2.17/2.20 писала
/// `Languages/<Имя>.xml` с чужим `xmlns:pal` и `version="2.21"` — единственный файл дампа, не
/// совпадавший с платформенным эталоном 2.17 (711/712). Теперь блок производится из таргета.
///
/// §1.0: версия вне реестра — типизированный отказ, а не «похожий» блок.
fn mdobject_ns(target: FormatVersion) -> Result<String, DesignerError> {
    let profile = crate::common::profile_for(target).ok_or_else(|| {
        DesignerError::Envelope(format!(
            "language sidecar: no Designer envelope profile for format {target} \
             (not in the FORMATS.md §2 registry)"
        ))
    })?;
    let mut s = String::new();
    for (name, uri) in profile.ns_block {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&format!("{name}=\"{uri}\""));
    }
    s.push_str(&format!(" version=\"{}\"", profile.version_value));
    Ok(s)
}

/// Собрать байты Designer-sidecar языка (`Languages/<Name>.xml`) из IR `F_LANGUAGES`-строки —
/// ОБРАТНАЯ операция к [`read_language_sidecar`]. Envelope платформенный: BOM + `<?xml…?>` + CRLF +
/// TAB-отступ, БЕЗ завершающего EOL (Designer-конвенция; RE: fixture-sidecar byte-verified).
///
/// `row` — `List([Str(uuid), Str(name), Str(languageCode), List([Str(lang), Str(content)]×n)])`
/// (форма, которую даёт и EDT-inline read, и [`read_language_sidecar`]). Whole-config write
/// (`pipeline::language_write`) вызывает это для КАЖДОГО языка корня, когда target = Designer, —
/// иначе Designer-дерево несёт висячую ссылку `<Language>` из `ChildObjects` без файла-тела.
/// §1.0: аномальная форма строки → типизированная [`DesignerError`] (не best-effort).
pub fn write_language_sidecar(row: &PropertyValue) -> Result<Vec<u8>, DesignerError> {
    let cells = match row {
        PropertyValue::List(c) if c.len() == 4 => c,
        other => {
            return Err(DesignerError::Envelope(format!(
                "language row must be List[uuid, name, languageCode, synonym], got {other:?} (§1.0)"
            )))
        }
    };
    let str_cell = |v: &PropertyValue, what: &str| -> Result<String, DesignerError> {
        match v {
            PropertyValue::Str(s) => Ok(s.clone()),
            other => Err(DesignerError::Envelope(format!(
                "language {what} must be a Str, got {other:?} (§1.0)"
            ))),
        }
    };
    let uuid = str_cell(&cells[0], "uuid")?;
    // Валидировать hex-форму uuid (§1.0: не пишем малформированный дескриптор).
    let _ = crate::parse_uuid(&uuid)?;
    let name = str_cell(&cells[1], "name")?;
    let code = str_cell(&cells[2], "languageCode")?;
    let synonym = match &cells[3] {
        PropertyValue::List(pairs) => pairs,
        other => {
            return Err(DesignerError::Envelope(format!(
                "language synonym must be a List of (lang, content) pairs, got {other:?} (§1.0)"
            )))
        }
    };
    if synonym.len() % 2 != 0 {
        return Err(DesignerError::Envelope(format!(
            "language synonym must be flat (lang, content) pairs (even length), got {} items (§1.0)",
            synonym.len()
        )));
    }

    // Версия обёртки — АМБЬЕНТНЫЙ round-trip-таргет (его ставит `pipeline::convert` из
    // `--v8version` либо версии источника); вне scope — SSL, прежнее поведение.
    let target = morph1c_core::version::current_roundtrip_target()
        .unwrap_or(morph1c_core::version::SSL);
    let ns = mdobject_ns(target)?;

    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    s.push_str(&format!("<MetaDataObject {ns}>\r\n"));
    s.push_str(&format!(
        "\t<Language uuid=\"{}\">\r\n",
        xml_escape_attr(&uuid)
    ));
    s.push_str("\t\t<Properties>\r\n");
    s.push_str(&format!(
        "\t\t\t<Name>{}</Name>\r\n",
        xml_escape_text(&name)
    ));
    if synonym.is_empty() {
        s.push_str("\t\t\t<Synonym/>\r\n");
    } else {
        s.push_str("\t\t\t<Synonym>\r\n");
        let mut i = 0;
        while i < synonym.len() {
            let lang = str_cell(&synonym[i], "synonym lang")?;
            let content = str_cell(&synonym[i + 1], "synonym content")?;
            s.push_str("\t\t\t\t<v8:item>\r\n");
            s.push_str(&format!(
                "\t\t\t\t\t<v8:lang>{}</v8:lang>\r\n",
                xml_escape_text(&lang)
            ));
            s.push_str(&format!(
                "\t\t\t\t\t<v8:content>{}</v8:content>\r\n",
                xml_escape_text(&content)
            ));
            s.push_str("\t\t\t\t</v8:item>\r\n");
            i += 2;
        }
        s.push_str("\t\t\t</Synonym>\r\n");
    }
    s.push_str("\t\t\t<Comment/>\r\n");
    s.push_str(&format!(
        "\t\t\t<LanguageCode>{}</LanguageCode>\r\n",
        xml_escape_text(&code)
    ));
    s.push_str("\t\t</Properties>\r\n");
    s.push_str("\t</Language>\r\n");
    s.push_str("</MetaDataObject>");

    // BOM + UTF-8 bytes (no trailing EOL — Designer sidecar convention).
    let mut out = Vec::with_capacity(3 + s.len());
    out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    out.extend_from_slice(s.as_bytes());
    Ok(out)
}

/// Минимальное XML-экранирование текстового узла (`&`, `<`, `>`).
fn xml_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// XML-экранирование значения атрибута (текст + `"`).
fn xml_escape_attr(s: &str) -> String {
    xml_escape_text(s).replace('"', "&quot;")
}

/// Прочитать `<Synonym>` → плоский `Vec<Str>` пар `(lang, text)` (форма synonym EDT-ридера).
/// Каждый item — `<v8:item><v8:lang>ru</v8:lang><v8:content>Русский</v8:content></v8:item>`.
/// Пустой `<Synonym/>` → пустой вектор. §1.0: item без lang/content → ошибка.
fn read_synonym(syn: &Element) -> Result<Vec<PropertyValue>, DesignerError> {
    if !syn.attrs.is_empty() || !syn.text.is_empty() {
        return Err(DesignerError::Envelope(
            "language sidecar: <Synonym> must be attribute-less, no text (§1.0)".into(),
        ));
    }
    let mut out: Vec<PropertyValue> = Vec::new();
    for item in &syn.children {
        if item.local != "item" || item.prefix != "v8" {
            return Err(DesignerError::Envelope(format!(
                "language sidecar: <Synonym> child must be <v8:item>, got <{}> (§1.0)",
                qname(item)
            )));
        }
        if !item.attrs.is_empty() || !item.text.is_empty() {
            return Err(DesignerError::Envelope(
                "language sidecar: <v8:item> must be attribute-less, no text (§1.0)".into(),
            ));
        }
        let mut it = item.children.iter();
        let lang = take_v8_leaf(it.next(), "lang")?;
        let content = take_v8_leaf(it.next(), "content")?;
        if let Some(extra) = it.next() {
            return Err(DesignerError::Envelope(format!(
                "language sidecar: <v8:item> unexpected extra child <{}> (§1.0)",
                qname(extra)
            )));
        }
        out.push(PropertyValue::Str(lang));
        out.push(PropertyValue::Str(content));
    }
    Ok(out)
}

/// Взять `<v8:<local>>text</v8:<local>>`-лист (§1.0: строго — нужный тег, plain-text-лист).
fn take_v8_leaf(el: Option<&Element>, local: &str) -> Result<String, DesignerError> {
    let el = el.ok_or_else(|| {
        DesignerError::Envelope(format!(
            "language sidecar: <v8:item> missing <v8:{local}> (§1.0)"
        ))
    })?;
    if el.local != local || el.prefix != "v8" {
        return Err(DesignerError::Envelope(format!(
            "language sidecar: expected <v8:{local}>, got <{}> (§1.0)",
            qname(el)
        )));
    }
    if !el.attrs.is_empty() || !el.children.is_empty() {
        return Err(DesignerError::Envelope(format!(
            "language sidecar: <v8:{local}> must be a plain text leaf (§1.0)"
        )));
    }
    Ok(el.text.clone())
}

/// §1.0: элемент — plain-text-лист (без атрибутов, без детей). Пустой текст допустим.
fn require_text_leaf(el: &Element, ctx: &str) -> Result<(), DesignerError> {
    if !el.attrs.is_empty() || !el.children.is_empty() {
        return Err(DesignerError::Envelope(format!(
            "language sidecar: <{ctx}> must be a plain text leaf (§1.0)"
        )));
    }
    Ok(())
}

#[cfg(any())]
mod tests {
    use super::*;

    const RU_SIDECAR: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.21">
	<Language uuid="db4a9ccb-9ef5-4b3c-8577-b6fe5db1b62e">
		<Properties>
			<Name>Test</Name>
			<Synonym>
				<v8:item>
					<v8:lang>ru</v8:lang>
					<v8:content>Testovy</v8:content>
				</v8:item>
			</Synonym>
			<Comment/>
			<LanguageCode>ru</LanguageCode>
		</Properties>
	</Language>
</MetaDataObject>"#;

    #[test]
    fn parses_ru_sidecar_into_edt_shaped_row() {
        let (name, row) = read_language_sidecar(RU_SIDECAR).expect("parse sidecar");
        assert_eq!(name, "Test");
        let cells = match &row {
            PropertyValue::List(c) => c,
            other => panic!("row must be List, got {other:?}"),
        };
        assert_eq!(cells.len(), 4, "row = [uuid, name, langCode, synonym]");
        assert_eq!(
            cells[0],
            PropertyValue::Str("db4a9ccb-9ef5-4b3c-8577-b6fe5db1b62e".into())
        );
        assert_eq!(cells[1], PropertyValue::Str("Test".into()));
        assert_eq!(cells[2], PropertyValue::Str("ru".into()));
        assert_eq!(
            cells[3],
            PropertyValue::List(vec![
                PropertyValue::Str("ru".into()),
                PropertyValue::Str("Testovy".into()),
            ])
        );
    }

    #[test]
    fn empty_synonym_is_ok() {
        let bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.21">
	<Language uuid="11111111-1111-1111-1111-111111111111">
		<Properties>
			<Name>English</Name>
			<Synonym/>
			<Comment/>
			<LanguageCode>en</LanguageCode>
		</Properties>
	</Language>
</MetaDataObject>"#;
        let (name, row) = read_language_sidecar(bytes).expect("parse empty-synonym sidecar");
        assert_eq!(name, "English");
        if let PropertyValue::List(cells) = &row {
            assert_eq!(
                cells[3],
                PropertyValue::List(vec![]),
                "empty synonym → empty list"
            );
        } else {
            panic!("row must be List");
        }
    }

    #[test]
    fn missing_uuid_errors() {
        let bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21">
	<Language>
		<Properties><Name>X</Name><LanguageCode>x</LanguageCode></Properties>
	</Language>
</MetaDataObject>"#;
        assert!(
            read_language_sidecar(bytes).is_err(),
            "missing @uuid must error (§1.0)"
        );
    }

    #[test]
    fn missing_language_code_errors() {
        let bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21">
	<Language uuid="11111111-1111-1111-1111-111111111111">
		<Properties><Name>X</Name></Properties>
	</Language>
</MetaDataObject>"#;
        assert!(
            read_language_sidecar(bytes).is_err(),
            "missing <LanguageCode> must error (§1.0)"
        );
    }

    #[test]
    fn write_then_read_round_trips_the_row() {
        // The writer is the inverse of the reader: writing an IR row then reading it back yields
        // the SAME (name, row). Pins the sidecar writer (edt→designer language emission) offline.
        let (_name, row) = read_language_sidecar(RU_SIDECAR).expect("parse fixture-shaped sidecar");
        let bytes = write_language_sidecar(&row).expect("write language sidecar");
        assert_eq!(&bytes[0..3], &[0xEF, 0xBB, 0xBF], "BOM present");
        assert!(
            !bytes.ends_with(b"\n"),
            "no trailing EOL (Designer convention)"
        );
        let (name2, row2) = read_language_sidecar(&bytes).expect("re-read written sidecar");
        assert_eq!(name2, "Test");
        assert_eq!(row2, row, "write→read round-trips the language row");
    }

    #[test]
    fn write_empty_synonym_round_trips() {
        let row = PropertyValue::List(vec![
            PropertyValue::Str("11111111-1111-1111-1111-111111111111".into()),
            PropertyValue::Str("English".into()),
            PropertyValue::Str("en".into()),
            PropertyValue::List(vec![]),
        ]);
        let bytes = write_language_sidecar(&row).expect("write empty-synonym sidecar");
        let (name, row2) = read_language_sidecar(&bytes).expect("re-read");
        assert_eq!(name, "English");
        assert_eq!(row2, row);
    }

    #[test]
    fn write_rejects_malformed_row() {
        // §1.0: a row that is not List[uuid,name,code,synonym] must error, never write garbage.
        let bad = PropertyValue::Str("nope".into());
        assert!(write_language_sidecar(&bad).is_err());
    }

    #[test]
    fn unknown_property_child_errors() {
        let bytes = br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.21">
	<Language uuid="11111111-1111-1111-1111-111111111111">
		<Properties><Name>X</Name><LanguageCode>x</LanguageCode><Bogus>y</Bogus></Properties>
	</Language>
</MetaDataObject>"#;
        assert!(
            read_language_sidecar(bytes).is_err(),
            "unknown <Properties> child must error (§1.0)"
        );
    }
}
