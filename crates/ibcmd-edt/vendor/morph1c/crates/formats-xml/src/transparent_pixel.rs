//! Codec для EDT-узла `<transparentPixel>` вида `CommonPicture` (координата прозрачного
//! пикселя картинки, metamodel `Point` — `CommonPicture.transparentPixel`).
//!
//! # Носитель — ТОЛЬКО EDT-дескриптор
//! * **EDT**: `<transparentPixel><x>10</x><y>7</y></transparentPixel>` — SPARSE-листья:
//!   нулевая координата ОПУСКАЕТСЯ (witnessed ERP `ВажностьНовостиОченьВажная`: только
//!   `<x>14</x>`, designer-зеркало несёт `x="14" y="0"`). Присутствие узла = «пиксель есть».
//! * **Designer**: дескриптор узла НЕ несёт — носитель ОБЁРТКА сайдкара `Ext/Picture.xml`
//!   (`<xr:LoadTransparent>true</…>` + `<xr:TransparentPixel x=… y=…/>`), её читает/пишет
//!   `morph1c_pipeline::picture_read` В ТО ЖЕ спек-поле (зеркало `predefined`-паттерна;
//!   поле в спеке `x_ignore`).
//! * **cf**: envelope тела `<uuid>.0` — `{1,0,x,y}` (см. `formats_cf::picture_body`).
//!
//! Канонический IR — `PropertyValue::List([Int(x), Int(y)])`; пустой `List` («пикселя
//! нет», дефолт спека) до кодека не доходит (re-sparsify). §1.0: иная структура узла /
//! нецелочисленный текст / неожиданный лист → ОШИБКА, не догадка. Witnessed-скан ERP
//! (30/2458 носителей): листы только `x`/`y`, в этом порядке, без атрибутов.

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

/// Декодировать узел `<transparentPixel>` (уже locate'нут). Присутствие ⇒
/// `List([Int(x), Int(y)])`; опущенный лист = координата 0 (sparse, witnessed).
pub fn decode(host: &Element) -> Decoded {
    match decode_inner(host) {
        Ok(v) => Decoded::Present(v),
        Err(e) => Decoded::Error(e),
    }
}

fn decode_inner(host: &Element) -> Result<PropertyValue, String> {
    host.claim();
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("transparentPixel: node must be attribute-less, no text (§1.0)".into());
    }
    let mut x: Option<i64> = None;
    let mut y: Option<i64> = None;
    for c in &host.children {
        if !c.prefix.is_empty() || !c.attrs.is_empty() || !c.children.is_empty() {
            return Err(format!(
                "transparentPixel: leaf <{}> carries prefix/attrs/children (§1.0)",
                c.local
            ));
        }
        let slot = match c.local.as_str() {
            "x" => &mut x,
            "y" => &mut y,
            other => {
                return Err(format!(
                    "transparentPixel: unexpected <{other}> (only <x>/<y> witnessed — §1.0)"
                ))
            }
        };
        if slot.is_some() {
            return Err(format!("transparentPixel: duplicate <{}> (§1.0)", c.local));
        }
        let n: i64 = c.text.parse().map_err(|e| {
            format!(
                "transparentPixel: <{}> text {:?} is not an integer: {e} (§1.0)",
                c.local, c.text
            )
        })?;
        c.claim_with_text();
        *slot = Some(n);
    }
    // SPARSE: опущенный лист = 0 (witnessed `ВажностьНовостиОченьВажная`: <x> без <y>,
    // designer-зеркало y="0").
    Ok(PropertyValue::List(vec![
        PropertyValue::Int(x.unwrap_or(0)),
        PropertyValue::Int(y.unwrap_or(0)),
    ]))
}

/// Claim (для leftover) — то же, что decode.
pub fn claim(host: &Element) {
    let _ = decode_inner(host);
}

/// Эмитировать `<tag>` из IR `List([Int(x), Int(y)])`. SPARSE-листья: нулевая координата
/// опускается (byte-exact к witnessed EDT); пустой List сюда не доходит (re-sparsify).
pub fn encode(ns: &str, tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let (x, y) = pixel_of(value)?;
    let mut el = OutElement::branch(ns, tag);
    if x != 0 {
        el.push(OutElement::leaf("", "x", x.to_string()));
    }
    if y != 0 {
        el.push(OutElement::leaf("", "y", y.to_string()));
    }
    Ok(el)
}

/// Разобрать канонический IR-узел пикселя `List([Int(x), Int(y)])` → `(x, y)`.
/// Общий помощник кодека и cf/designer-писателей (`pub` — его зовёт
/// `morph1c_pipeline::picture_read` и `formats_cf`). §1.0: иная форма → ошибка.
pub fn pixel_of(value: &PropertyValue) -> Result<(i64, i64), String> {
    match value {
        PropertyValue::List(l) => match l.as_slice() {
            [PropertyValue::Int(x), PropertyValue::Int(y)] => Ok((*x, *y)),
            _ => Err(format!(
                "transparentPixel: IR node must be List([Int(x), Int(y)]), got {} element(s) (§1.0)",
                l.len()
            )),
        },
        other => Err(format!(
            "transparentPixel: expected List, got {:?} (§1.0)",
            other.kind()
        )),
    }
}

#[cfg(any())]
mod tests {
    use super::*;

    fn parse_host(xml: &str) -> crate::read::Descriptor {
        crate::parse(xml.as_bytes()).expect("xml")
    }

    // Witnessed ВажностиНовостей: оба листа → List([10,7]) и байт-точная переэмиссия.
    #[test]
    fn both_leaves_roundtrip() {
        let doc =
            parse_host("<transparentPixel><x>10</x><y>7</y></transparentPixel>");
        let v = match decode(&doc.root) {
            Decoded::Present(v) => v,
            _ => panic!("decode"),
        };
        assert_eq!(pixel_of(&v).unwrap(), (10, 7));
        let out = encode("", "transparentPixel", &v).unwrap();
        assert_eq!(out.children.len(), 2);
    }

    // Witnessed ВажностьНовостиОченьВажная: только <x> → y=0; re-emit опускает <y>.
    #[test]
    fn sparse_y_zero() {
        let doc = parse_host("<transparentPixel><x>14</x></transparentPixel>");
        let v = match decode(&doc.root) {
            Decoded::Present(v) => v,
            _ => panic!("decode"),
        };
        assert_eq!(pixel_of(&v).unwrap(), (14, 0));
        let out = encode("", "transparentPixel", &v).unwrap();
        assert_eq!(out.children.len(), 1, "zero y is omitted (sparse)");
    }

    // §1.0: посторонний лист — ошибка, не догадка.
    #[test]
    fn unexpected_leaf_is_refused() {
        let doc = parse_host("<transparentPixel><z>1</z></transparentPixel>");
        match decode(&doc.root) {
            Decoded::Error(e) => assert!(e.contains("unexpected"), "{e}"),
            _ => panic!("expected error"),
        }
    }
}
