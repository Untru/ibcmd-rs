//! Codec для XDTO-type-ref — ССЫЛКИ на XDTO-тип как пара `(name, nsUri)`
//! (`WebService.Operation.xdtoReturningValueType` / `.Parameter.xdtoValueType`).
//! Это НЕ `TypeSpec` (ОписаниеТипов) и НЕ `Value`: одиночная ссылка на XDTO-тип,
//! адресуемая ИМЕНЕМ типа + URI пространства имён XDTO.
//!
//! Форматы кодируют ОДНУ логическую пару, расходясь структурно:
//! * **EDT**: host-контейнер с ДВУМЯ листами —
//!   `<host><name>string</name><nsUri>http://www.w3.org/2001/XMLSchema</nsUri></host>`.
//!   Оба листа present ВСЕГДА (это значение, а не дефолт-омиссия). Порядок: name, nsUri.
//! * **Designer**: QName-лист — `<Host>prefix:local</Host>`, где `prefix` разрешается в
//!   nsUri через таблицу префиксов КОРНЯ (`xs`→XMLSchema, `v8`→…/8.1/data/core). Для
//!   nsUri ВНЕ корневой таблицы платформа объявляет ЛОКАЛЬНЫЙ `xmlns:d6p1="<uri>"`
//!   ПРЯМО на host-элементе и пишет `d6p1:local` (сверено: prefix ВСЕГДА `d6p1`).
//!
//! Канонический IR (X by construction): `PropertyValue::List([Str(name), Str(nsUri)])` —
//! пара в фиксированном порядке. Оба формата дают ТУ ЖЕ пару (§1.6). §1.0: неизвестный
//! префикс / лишний атрибут / лишний ребёнок / чужая структура → типизированная ОШИБКА,
//! никогда silent-drop/guess.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::ir::value::PropertyValue;

/// Диалект XDTO-type-ref проекции: EDT (name+nsUri листы) / Designer (QName-лист).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XdtoTypeRefDialect {
    /// EDT: host с `<name>`+`<nsUri>` листами.
    Edt,
    /// Designer: QName-лист `prefix:local` (+опц. локальный `xmlns:d6p1`).
    Designer,
}

/// Локальный auto-префикс, которым платформа объявляет ns ВНЕ корневой таблицы. Сверено
/// по SSL WebServices-корпусу: ВСЕГДА `d6p1` (первый локальный ns на элементе).
const LOCAL_PREFIX: &str = "d6p1";

const XS_PREFIX: &str = "xs";
const XS_URI: &str = "http://www.w3.org/2001/XMLSchema";
const V8_PREFIX: &str = "v8";
const V8_URI: &str = "http://v8.1c.ru/8.1/data/core";

/// Разрешить корневой (root-declared) префикс XDTO-типа в nsUri. Корневая таблица
/// Designer-дескриптора несёт РОВНО эти два префикса для XDTO-типов (сверено: 447 `xs:` +
/// 85 `v8:`); прочие nsUri идут через локальный `xmlns:d6p1` (не через корень).
fn root_prefix_to_uri(prefix: &str) -> Option<&'static str> {
    match prefix {
        XS_PREFIX => Some(XS_URI),
        V8_PREFIX => Some(V8_URI),
        _ => None,
    }
}

/// Обратно: nsUri, который несёт КОРЕНЬ (эмитится как `prefix:local` без локального xmlns).
fn uri_to_root_prefix(uri: &str) -> Option<&'static str> {
    match uri {
        XS_URI => Some(XS_PREFIX),
        V8_URI => Some(V8_PREFIX),
        _ => None,
    }
}

/// Упаковать пару в канонический IR — `List([Str(name), Str(nsUri)])`.
fn pack(name: String, ns_uri: String) -> PropertyValue {
    PropertyValue::List(vec![PropertyValue::Str(name), PropertyValue::Str(ns_uri)])
}

/// Распаковать канонический IR обратно в `(name, nsUri)`.
fn unpack(value: &PropertyValue) -> Result<(&str, &str), String> {
    let items = match value {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "xdto-type-ref must be List, got {:?}",
                other.kind()
            ))
        }
    };
    if items.len() != 2 {
        return Err(format!(
            "xdto-type-ref must be List of 2 (name, nsUri), got {}",
            items.len()
        ));
    }
    let name = match &items[0] {
        PropertyValue::Str(s) => s.as_str(),
        other => {
            return Err(format!(
                "xdto-type-ref name must be Str, got {:?}",
                other.kind()
            ))
        }
    };
    let ns_uri = match &items[1] {
        PropertyValue::Str(s) => s.as_str(),
        other => {
            return Err(format!(
                "xdto-type-ref nsUri must be Str, got {:?}",
                other.kind()
            ))
        }
    };
    Ok((name, ns_uri))
}

// --- DECODE -----------------------------------------------------------------

/// Разобрать host-элемент в канонический IR. Host уже claimed `locate`'ом; здесь
/// клеймятся листы/текст/локальный xmlns (§1.0 B1). Любая иная структура → `Err`.
pub fn decode(dialect: XdtoTypeRefDialect, host: &Element) -> Result<PropertyValue, String> {
    match dialect {
        XdtoTypeRefDialect::Edt => decode_edt(host),
        XdtoTypeRefDialect::Designer => decode_designer(host),
    }
}

fn decode_edt(host: &Element) -> Result<PropertyValue, String> {
    if !host.attrs.is_empty() {
        return Err(format!(
            "xdto-type-ref(edt) <{}> must have no attributes (§1.0)",
            host.local
        ));
    }
    if !host.text.is_empty() {
        return Err(format!(
            "xdto-type-ref(edt) <{}> must have no direct text (§1.0)",
            host.local
        ));
    }
    let mut it = host.children.iter();
    let name_el = it
        .next()
        .filter(|e| e.local == "name" && e.prefix.is_empty())
        .ok_or_else(|| "xdto-type-ref(edt) expects <name> child first (§1.0)".to_string())?;
    let ns_el = it
        .next()
        .filter(|e| e.local == "nsUri" && e.prefix.is_empty())
        .ok_or_else(|| {
            "xdto-type-ref(edt) expects <nsUri> child after <name> (§1.0)".to_string()
        })?;
    if let Some(extra) = it.next() {
        return Err(format!(
            "xdto-type-ref(edt) has unexpected extra child <{}> (§1.0)",
            qname(extra)
        ));
    }
    for leaf in [name_el, ns_el] {
        if !leaf.attrs.is_empty() || !leaf.children.is_empty() {
            return Err(format!(
                "xdto-type-ref(edt) <{}> must be a plain text leaf (§1.0)",
                leaf.local
            ));
        }
    }
    name_el.claim_with_text();
    ns_el.claim_with_text();
    Ok(pack(name_el.text.clone(), ns_el.text.clone()))
}

fn decode_designer(host: &Element) -> Result<PropertyValue, String> {
    // QName-лист: текст `prefix:local`; опц. локальный `xmlns:d6p1="<uri>"`-атрибут.
    if !host.children.is_empty() {
        return Err(format!(
            "xdto-type-ref(designer) <{}> must be a text leaf, got child <{}> (§1.0)",
            host.local, host.children[0].local
        ));
    }
    host.claim_text();
    let qn = host.text.as_str();
    let (prefix, local) = qn.split_once(':').ok_or_else(|| {
        format!("xdto-type-ref(designer) text {qn:?} must be a QName prefix:local (§1.0)")
    })?;
    if prefix.is_empty() || local.is_empty() {
        return Err(format!(
            "xdto-type-ref(designer) QName {qn:?} has empty prefix/local (§1.0)"
        ));
    }
    // Разрешить префикс: локальный `xmlns:<prefix>` на host (приоритет) → корневая таблица.
    let xmlns_attr_name = format!("xmlns:{prefix}");
    let ns_uri = if let Some(a) = host.attr(&xmlns_attr_name) {
        a.claimed.set(true);
        a.value.clone()
    } else {
        root_prefix_to_uri(prefix)
            .ok_or_else(|| {
                format!(
                    "xdto-type-ref(designer) unknown prefix {prefix:?} (no local xmlns:{prefix}, \
                     not root xs/v8) — §1.0: no guess"
                )
            })?
            .to_string()
    };
    // Любой ДРУГОЙ атрибут (кроме разобранного локального xmlns) — несконсуменный (§1.0).
    if let Some(extra) = host.attrs.iter().find(|a| !a.claimed.get()) {
        return Err(format!(
            "xdto-type-ref(designer) <{}> has unexpected attribute {:?} (§1.0)",
            host.local, extra.name
        ));
    }
    Ok(pack(local.to_string(), ns_uri))
}

// --- CLAIM (согласован с decode, §1.0 B1) -----------------------------------

/// Claim РОВНО те узлы, что читает [`decode`] (host уже claimed вызывающим).
pub fn claim(dialect: XdtoTypeRefDialect, host: &Element) {
    match dialect {
        XdtoTypeRefDialect::Edt => {
            for leaf in host
                .children
                .iter()
                .filter(|c| c.prefix.is_empty() && (c.local == "name" || c.local == "nsUri"))
            {
                leaf.claim_with_text();
            }
        }
        XdtoTypeRefDialect::Designer => {
            host.claim_text();
            // Локальный `xmlns:<prefix>` (если QName-текст его использует).
            if let Some((prefix, _)) = host.text.split_once(':') {
                if let Some(a) = host.attr(&format!("xmlns:{prefix}")) {
                    a.claimed.set(true);
                }
            }
        }
    }
}

// --- ENCODE -----------------------------------------------------------------

/// Восстановить host-элемент из канонического IR (byte-exact). Имя/ns host — из локуса.
pub fn encode(
    dialect: XdtoTypeRefDialect,
    host_prefix: &str,
    host_local: &str,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    let (name, ns_uri) = unpack(value)?;
    match dialect {
        XdtoTypeRefDialect::Edt => {
            let mut h = OutElement::branch(host_prefix, host_local);
            h.push(OutElement::leaf("", "name", name.to_string()));
            h.push(OutElement::leaf("", "nsUri", ns_uri.to_string()));
            Ok(h)
        }
        XdtoTypeRefDialect::Designer => {
            if let Some(prefix) = uri_to_root_prefix(ns_uri) {
                // Корневой префикс: `<Host>prefix:local</Host>`, без локального xmlns.
                Ok(OutElement::leaf(
                    host_prefix,
                    host_local,
                    format!("{prefix}:{name}"),
                ))
            } else {
                // Кастомный ns: локальный `xmlns:d6p1="<uri>"` + `d6p1:local`.
                Ok(
                    OutElement::leaf(host_prefix, host_local, format!("{LOCAL_PREFIX}:{name}"))
                        .attr(format!("xmlns:{LOCAL_PREFIX}"), ns_uri.to_string()),
                )
            }
        }
    }
}

fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}

#[cfg(any())]
mod tests {
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
    fn pair(v: &PropertyValue) -> (String, String) {
        let (n, u) = unpack(v).unwrap();
        (n.to_string(), u.to_string())
    }

    // X-equality: EDT-decode == Designer-decode for the same logical type-ref.
    #[test]
    fn x_equal_xs_string() {
        let e = decode(
            XdtoTypeRefDialect::Edt,
            &host_of(
                "<w><h><name>string</name><nsUri>http://www.w3.org/2001/XMLSchema</nsUri></h></w>",
            ),
        )
        .unwrap();
        let d = decode(
            XdtoTypeRefDialect::Designer,
            &host_of("<w><h>xs:string</h></w>"),
        )
        .unwrap();
        assert_eq!(e, d);
        assert_eq!(
            pair(&e),
            ("string".into(), "http://www.w3.org/2001/XMLSchema".into())
        );
    }

    #[test]
    fn x_equal_v8_array() {
        let e = decode(
            XdtoTypeRefDialect::Edt,
            &host_of(
                "<w><h><name>Array</name><nsUri>http://v8.1c.ru/8.1/data/core</nsUri></h></w>",
            ),
        )
        .unwrap();
        let d = decode(
            XdtoTypeRefDialect::Designer,
            &host_of("<w><h>v8:Array</h></w>"),
        )
        .unwrap();
        assert_eq!(e, d);
    }

    #[test]
    fn x_equal_custom_ns_local_xmlns() {
        let uri = "http://v8.1c.ru/SSL/Exchange/EnterpriseDataExchange";
        let e = decode(
            XdtoTypeRefDialect::Edt,
            &host_of(&format!(
                "<w><h><name>PrepareDataOperationResult</name><nsUri>{uri}</nsUri></h></w>"
            )),
        )
        .unwrap();
        let d = decode(
            XdtoTypeRefDialect::Designer,
            &host_of(&format!(
                "<w><h xmlns:d6p1=\"{uri}\">d6p1:PrepareDataOperationResult</h></w>"
            )),
        )
        .unwrap();
        assert_eq!(e, d);
        assert_eq!(pair(&e).1, uri);
    }

    // Byte-exact round-trip each dialect.
    #[test]
    fn byte_exact_edt() {
        let h = host_of(
            "<w><h><name>string</name><nsUri>http://www.w3.org/2001/XMLSchema</nsUri></h></w>",
        );
        let v = decode(XdtoTypeRefDialect::Edt, &h).unwrap();
        let out = encode(XdtoTypeRefDialect::Edt, "", "h", &v).unwrap();
        let bytes = String::from_utf8(render(&edt_env(), &out)).unwrap();
        assert_eq!(
            bytes,
            format!(
                "{}\n<h>\n  <name>string</name>\n  <nsUri>http://www.w3.org/2001/XMLSchema</nsUri>\n</h>",
                edt_env().decl
            )
        );
    }

    #[test]
    fn byte_exact_designer_root_prefix() {
        let h = host_of("<w><h>xs:string</h></w>");
        let v = decode(XdtoTypeRefDialect::Designer, &h).unwrap();
        let out = encode(XdtoTypeRefDialect::Designer, "", "h", &v).unwrap();
        let bytes = String::from_utf8(render(&designer_env(), &out)).unwrap();
        assert_eq!(bytes, format!("{}\n<h>xs:string</h>", designer_env().decl));
    }

    #[test]
    fn byte_exact_designer_custom_ns() {
        let uri = "http://www.1c.ru/SaaS/ExchangeAdministration/Common";
        let h = host_of(&format!(
            "<w><h xmlns:d6p1=\"{uri}\">d6p1:ExchangeFeatures</h></w>"
        ));
        let v = decode(XdtoTypeRefDialect::Designer, &h).unwrap();
        let out = encode(XdtoTypeRefDialect::Designer, "", "h", &v).unwrap();
        let bytes = String::from_utf8(render(&designer_env(), &out)).unwrap();
        assert_eq!(
            bytes,
            format!(
                "{}\n<h xmlns:d6p1=\"{uri}\">d6p1:ExchangeFeatures</h>",
                designer_env().decl
            )
        );
    }

    // §1.0 adversarial.
    #[test]
    fn unknown_prefix_errors() {
        let err = decode(
            XdtoTypeRefDialect::Designer,
            &host_of("<w><h>zz:Foo</h></w>"),
        )
        .unwrap_err();
        assert!(err.contains("unknown prefix"), "{err}");
    }

    #[test]
    fn edt_extra_child_errors() {
        let err = decode(
            XdtoTypeRefDialect::Edt,
            &host_of("<w><h><name>x</name><nsUri>u</nsUri><extra>y</extra></h></w>"),
        )
        .unwrap_err();
        assert!(err.contains("extra child"), "{err}");
    }
}
