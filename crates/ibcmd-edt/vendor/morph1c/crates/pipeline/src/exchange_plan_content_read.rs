//! Чтение/запись СОСТАВА плана обмена (`ExchangePlan.content`) в DESIGNER-диалекте — сайдкар
//! `<dir>/<Name>/Ext/Content.xml` (§1.2/§4). Сиблинг [`crate::help_read`]/[`crate::rights_read`].
//!
//! # Почему сайдкар нужен ОТДЕЛЬНЫМ проходом
//! EDT несёт состав ПРЯМО в дескрипторе (`<content><mdObject>…</mdObject></content>` — его читает
//! codec `formats_xml::exchange_plan_content` в спек-поле `content`). Designer в дескрипторе
//! `ExchangePlans/<Name>.xml` состава НЕ несёт ВООБЩЕ — он лежит в отдельном
//! `Ext/Content.xml`. Без этого прохода designer-IR терял состав ЦЕЛИКОМ: `designer→edt` ронял
//! 408 элементов состава SSL, а `designer→cf` не эмитил тело `<uuid>.1` (класс диффов `Состав`).
//!
//! # Раскладка (RE SSL — 1 ExchangePlan, 408 элементов; ERP — 16 планов, 18 865 элементов)
//! `Ext/Content.xml`: С BOM, CRLF, ТАБ-отступ, БЕЗ хвостового перевода строки (та же обёрточная
//! конвенция, что `Ext/Help.xml`/`Ext/Schedule.xml`). Корень `<ExchangePlanContent
//! version="2.20|2.21">` (ns extrnprops; версия формата — ВХОД чтения, см.
//! [`crate::sidecar_version`]; тело структурно ИДЕНТИЧНО между 2.20 и 2.21), элемент —
//! `<Item><Metadata>Kind.Name</Metadata><AutoRecord>Deny|Allow</AutoRecord></Item>`.
//! `<AutoRecord>` Designer пишет ВСЕГДА, EDT — опускает (см. codec: омиссия моделируется ДЛИНОЙ
//! записи). Поэтому канон, приходящий из Designer, несёт `Enum` явно, а из EDT — нет; оба дают
//! ОДИН И ТОТ ЖЕ cf-код (`formats_cf::exchange_plan_content_body`), а X-сравнение поле игнорирует
//! (`x_ignored`).
//!
//! §1.0-самопроверка на read: пере-сериализация канона обязана воспроизвести исходные байты.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::value::{PropertyValue, Token};
use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::exchange_plan::F_CONTENT;

use crate::ConvertError;

/// Вид-носитель состава.
const CONTENT_KIND: &str = "ExchangePlan";
/// Имя Designer-сайдкара внутри `Ext/`.
const FILE: &str = "Content.xml";
/// UTF-8 BOM.
const BOM: char = '\u{FEFF}';

/// Подгрузить состав из Designer-сайдкара в спек-поле `content`. EDT (состав в дескрипторе) и cf
/// (контейнер) — no-op; не-ExchangePlan — no-op; отсутствие файла — no-op (пустой состав).
pub fn attach_exchange_plan_content(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer || kind != CONTENT_KIND {
        return Ok(());
    }
    let Some(path) = sidecar_path(descriptor_path) else {
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
    let doc = formats_xml::parse(&bytes)
        .map_err(|e| read_err(format!("Content.xml {}: {e}", path.display())))?;
    let root = &doc.root;
    if root.local != "ExchangePlanContent" {
        return Err(read_err(format!(
            "Content.xml root is <{}>, expected <ExchangePlanContent> (§1.0)",
            root.local
        )));
    }
    // Версия формата — ВХОД чтения (witnessed: 2.20 на всех 16 ERP-сайдкарах, 2.21 у SSL);
    // §1.0-самопроверка ниже сериализует ИМЕННО ею (см. [`crate::sidecar_version`]).
    let version = root
        .attr("version")
        .ok_or_else(|| read_err("Content.xml root has no version attribute (§1.0)".into()))?;
    let version =
        crate::sidecar_version::parse_witnessed(&version.value, "Content.xml").map_err(read_err)?;
    let mut items: Vec<PropertyValue> = Vec::with_capacity(root.children.len());
    for child in &root.children {
        if child.local != "Item" {
            return Err(read_err(format!(
                "Content.xml carries <{}>, only <Item> witnessed (§1.0)",
                child.local
            )));
        }
        let md = child
            .child("Metadata")
            .filter(|e| !e.text.is_empty())
            .ok_or_else(|| read_err("Content.xml <Item> has no <Metadata> (§1.0)".into()))?;
        let auto = child
            .child("AutoRecord")
            .filter(|e| !e.text.is_empty())
            .ok_or_else(|| read_err("Content.xml <Item> has no <AutoRecord> (§1.0)".into()))?;
        // §1.0: иные дети <Item> не засвидетельствованы — не тихий скип.
        if child.children.len() != 2 {
            return Err(read_err(format!(
                "Content.xml <Item> has {} children, only <Metadata>+<AutoRecord> witnessed (§1.0)",
                child.children.len()
            )));
        }
        items.push(PropertyValue::List(vec![
            PropertyValue::Str(md.text.clone()),
            PropertyValue::Enum(Token::new(&auto.text)),
        ]));
    }
    if items.is_empty() {
        return Err(read_err(
            "Content.xml declares no <Item> (§1.0 — an empty sidecar is unwitnessed; a plan \
             without content simply has no file)"
                .into(),
        ));
    }
    // §1.0: дескриптор-read состав НЕ заполняет (Designer его там не несёт) — только этот проход.
    if obj.get(F_CONTENT).is_some() {
        return Err(read_err(
            "object already carries an exchange-plan content before the sidecar attach \
             (unexpected — the Designer descriptor does not carry it)"
                .into(),
        ));
    }
    let content = PropertyValue::List(items);
    // §1.0-самопроверка: канон обязан пере-сериализоваться в ИСХОДНЫЕ байты (ВЕРСИЕЙ
    // ИСТОЧНИКА — версия свойство файла, не IR, §1.6).
    if serialize(&content, version)? != bytes {
        return Err(read_err(format!(
            "Content.xml {} does not round-trip byte-exactly through the IR (§1.0)",
            path.display()
        )));
    }
    obj.properties.push((F_CONTENT, content));
    Ok(())
}

/// Write-side mirror: эмитить `content` в Designer-сайдкар. EDT (состав едет в дескрипторе) и cf
/// (тело собирает cf-ассемблер) — no-op; пустой/отсутствующий состав — no-op (файла нет).
pub fn write_exchange_plan_content(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format != Format::Designer || kind != CONTENT_KIND {
        return Ok(());
    }
    let content = match obj.get(F_CONTENT) {
        Some(v @ PropertyValue::List(l)) if !l.is_empty() => v,
        _ => return Ok(()),
    };
    let path = sidecar_path(descriptor_out).ok_or_else(|| ConvertError::Write {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason: "descriptor path has no parent/stem to anchor Ext/Content.xml at".into(),
    })?;
    // Версия ТАРГЕТА — амбьентный round-trip-таргет (формат выхода — параметр, FORMATS.md §1).
    let bytes = serialize(content, crate::sidecar_version::write_target()).map_err(|e| {
        ConvertError::Write {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: e.to_string(),
        }
    })?;
    crate::form_write::write_file(&path, &bytes)
}

/// `<dir>/<Name>.xml` → `<dir>/<Name>/Ext/Content.xml`.
fn sidecar_path(descriptor_path: &Path) -> Option<PathBuf> {
    Some(
        descriptor_path
            .parent()?
            .join(descriptor_path.file_stem()?)
            .join("Ext")
            .join(FILE),
    )
}

/// Канон → байты Designer-сайдкара (BOM, CRLF, таб-отступ, без хвостового перевода строки)
/// под ЗАДАННОЙ версией формата (2.20 ERP / 2.21 SSL — оба witnessed; ns-блок побайтно ОДИН
/// И ТОТ ЖЕ, различается ТОЛЬКО `version=`).
/// §1.0: элемент БЕЗ явного `autoRecord` (EDT-канон омиссии) в Designer-раскладку не выразим
/// «как есть» — Designer тег пишет ВСЕГДА; омиссия == `Deny` (witnessed 408/408 SSL; ERP несёт
/// и явный `Allow` — 562/18865), поэтому подставляется он.
fn serialize(
    content: &PropertyValue,
    version: morph1c_core::version::FormatVersion,
) -> Result<Vec<u8>, ConvertError> {
    let items = match content {
        PropertyValue::List(l) => l,
        other => {
            return Err(ConvertError::Write {
                kind: CONTENT_KIND.into(),
                object: String::new(),
                reason: format!(
                    "exchange-plan content must be a List, got {:?}",
                    other.kind()
                ),
            })
        }
    };
    let mut s = String::new();
    s.push(BOM);
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    s.push_str(&format!(
        "<ExchangePlanContent xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" \
         xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
         xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{version}\">\r\n",
    ));
    for item in items {
        let fields = match item {
            PropertyValue::List(f) => f,
            other => {
                return Err(ConvertError::Write {
                    kind: CONTENT_KIND.into(),
                    object: String::new(),
                    reason: format!("content item must be a List, got {:?}", other.kind()),
                })
            }
        };
        let md = match fields.first() {
            Some(PropertyValue::Str(x)) => x.as_str(),
            _ => {
                return Err(ConvertError::Write {
                    kind: CONTENT_KIND.into(),
                    object: String::new(),
                    reason: "content item has no mdObject path".into(),
                })
            }
        };
        // Омиссия (EDT-канон) == `Deny` — witnessed на всех 408 элементах SSL.
        let auto = match fields.get(1) {
            Some(PropertyValue::Enum(t)) => t.as_str(),
            None => AUTO_RECORD_DEFAULT,
            Some(other) => {
                return Err(ConvertError::Write {
                    kind: CONTENT_KIND.into(),
                    object: String::new(),
                    reason: format!("content autoRecord must be Enum, got {:?}", other.kind()),
                })
            }
        };
        s.push_str("\t<Item>\r\n\t\t<Metadata>");
        s.push_str(md);
        s.push_str("</Metadata>\r\n\t\t<AutoRecord>");
        s.push_str(auto);
        s.push_str("</AutoRecord>\r\n\t</Item>\r\n");
    }
    s.push_str("</ExchangePlanContent>");
    Ok(s.into_bytes())
}

/// Режим авторегистрации, соответствующий ОМИССИИ тега в EDT (witnessed 408/408 SSL).
const AUTO_RECORD_DEFAULT: &str = "Deny";

#[cfg(any())]
mod tests {
    use super::*;

    const SRC: &str = concat!(
        "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<ExchangePlanContent xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" ",
        "xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" ",
        "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
        "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.21\">\r\n",
        "\t<Item>\r\n\t\t<Metadata>InformationRegister.А</Metadata>\r\n",
        "\t\t<AutoRecord>Deny</AutoRecord>\r\n\t</Item>\r\n",
        "\t<Item>\r\n\t\t<Metadata>Catalog.Б</Metadata>\r\n",
        "\t\t<AutoRecord>Deny</AutoRecord>\r\n\t</Item>\r\n",
        "</ExchangePlanContent>"
    );

    fn parse_items(src: &str) -> PropertyValue {
        let doc = formats_xml::parse(src.as_bytes()).expect("xml");
        let mut items = Vec::new();
        for child in &doc.root.children {
            items.push(PropertyValue::List(vec![
                PropertyValue::Str(child.child("Metadata").unwrap().text.clone()),
                PropertyValue::Enum(Token::new(&child.child("AutoRecord").unwrap().text)),
            ]));
        }
        PropertyValue::List(items)
    }

    /// Byte-exact round-trip Designer-сайдкара (это и есть §1.0-самопроверка на read).
    #[test]
    fn designer_sidecar_roundtrips_byte_exact() {
        let content = parse_items(SRC);
        assert_eq!(
            String::from_utf8(serialize(&content, morph1c_core::version::SSL).expect("serialize"))
                .unwrap(),
            SRC
        );
    }

    /// ERP-витнесс (формат 2.20, явный `Allow`): корень несёт `version="2.20"` — ридер детектит
    /// её и §1.0-самопроверка сериализует ИМЕННО ею; тело структурно ИДЕНТИЧНО 2.21 (witnessed:
    /// все 16 ERP-сайдкаров отличаются от SSL только `version=`).
    #[test]
    fn erp_2_20_sidecar_roundtrips_byte_exact() {
        let src = SRC.replace("version=\"2.21\"", "version=\"2.20\"").replace(
            "<Metadata>Catalog.Б</Metadata>\r\n\t\t<AutoRecord>Deny</AutoRecord>",
            "<Metadata>Catalog.Б</Metadata>\r\n\t\t<AutoRecord>Allow</AutoRecord>",
        );
        let content = parse_items(&src);
        assert_eq!(
            String::from_utf8(serialize(&content, morph1c_core::version::ERP).expect("serialize"))
                .unwrap(),
            src
        );
    }

    /// EDT-канон (омиссия `autoRecord`) проецируется в Designer с явным `Deny` — witnessed
    /// соответствие омиссии и `Deny` (§1.6).
    #[test]
    fn edt_omission_projects_to_explicit_deny() {
        let content = PropertyValue::List(vec![PropertyValue::List(vec![PropertyValue::Str(
            "Catalog.Б".into(),
        )])]);
        let out =
            String::from_utf8(serialize(&content, morph1c_core::version::SSL).expect("serialize"))
                .unwrap();
        assert!(out.contains("<AutoRecord>Deny</AutoRecord>"), "{out}");
    }
}
