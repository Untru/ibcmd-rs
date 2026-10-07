//! Codec для `picture`/`Picture` subkind'а `.Command` (ссылка на картинку). ДАННО-УПРАВЛЯЕМ
//! и PARENT-АГНОСТИЧЕН: кодек не знает о родителе — всё выводит из host-элемента, поэтому
//! ЛЮБОЙ `.Command` (Catalog/Document/…) объявляет `Codec::PictureRef(dialect)` в СВОЕЙ
//! проекции без пер-родительского хардкода. Структура расходится между форматами:
//! * EDT: пусто → тег ОТСУТСТВУЕТ (дефолт-омиссия); непусто →
//!   `<picture xsi:type="core:PictureRef"><picture>StdPicture.X</picture></picture>`.
//! * Designer: пусто → `<Picture/>` (DENSE self-closing); непусто →
//!   `<Picture><xr:Ref>StdPicture.X</xr:Ref><xr:LoadTransparent>true</xr:LoadTransparent>
//!   </Picture>`.
//!
//! ## `LoadTransparent` — ПОЛЕ, а не выводимая константа
//! Раньше флаг считался выводимым из префикса ссылки (`StdPicture.*`→true, иначе false) и в
//! IR не жил. Это ОШИБКА КЛАССА «константа вместо переменной»: в корпусе
//! `integration_subsystem` есть `CommonPicture.инт_Кафка` с `LoadTransparent=true` — правилу
//! следуют 27/28 ссылок, одна НЕТ. Платформа даёт флаг ЗАДАТЬ ⇒ он обязан жить в IR, иначе
//! designer→designer МОЛЧА переписал бы его в `false` (§1.0-потеря).
//!
//! Current IR: `List([Str(ref), Bool(loadTransparent), optional List([Int(x), Int(y)])])`.
//! Native emits every supplied value. The whole-project adapter uses a typed resource
//! for EDT transparency; standalone EDT emission refuses a nonrepresentable tuple.
//! Empty reference with default true/no pixel collapses to an empty container.
//! (`Str("")` = empty reference, default —
//! [`picture_ref_default`]). У EDT флага НЕТ (доказано платформой: designer→edt→cf→designer
//! теряет его и восстанавливает правилом) ⇒ EDT-ридер ВЫВОДИТ флаг тем же правилом
//! ([`default_load_transparent`] — ДЕФОЛТ РЕКОНСТРУКЦИИ), пишет только ссылку, а поле
//! X-исключено в спеке (см. [`picture_ref_field`]). §1.0: иная структура/чужой ns/иной
//! xsi → ОШИБКА.
//!
//! [`picture_ref_default`]: morph1c_core::spec::common::picture_ref_default
//! [`picture_ref_field`]: morph1c_core::spec::common::picture_ref_field

use crate::descriptor::Element;
use crate::emit::OutElement;
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::PropertyValue;

/// Диалект.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureDialect {
    /// EDT: `<picture xsi:type="core:PictureRef"><picture>ref</picture></picture>`.
    Edt,
    /// Designer: `<Picture><xr:Ref>ref</xr:Ref><xr:LoadTransparent>true</…></Picture>`.
    Designer,
}

const XSI_TYPE: &str = "xsi:type";
const PICTURE_REF: &str = "core:PictureRef";

/// ДЕФОЛТ РЕКОНСТРУКЦИИ `loadTransparent` по префиксу ссылки: `CommonPicture.*` → `false`,
/// иначе (`StdPicture.*` / пусто) → `true`. Это ровно то, что делает САМА платформа, когда
/// восстанавливает потерянный в EDT флаг (`.fixtures/versions/README.md`) — поэтому EDT-ридер
/// применяет его же. Для Designer/`.cf`, где флаг ХРАНИТСЯ, правило НЕ навязывается.
pub fn default_load_transparent(reference: &str) -> bool {
    !reference.starts_with("CommonPicture.")
}

/// Current semantic tuple. Pixel coordinates reuse the typed Point representation.
pub fn pack(reference: String, load_transparent: bool, pixel: Option<(i64, i64)>) -> PropertyValue {
    let mut values = vec![
        PropertyValue::Str(reference),
        PropertyValue::Bool(load_transparent),
    ];
    if let Some((x, y)) = pixel {
        values.push(PropertyValue::List(vec![
            PropertyValue::Int(x),
            PropertyValue::Int(y),
        ]));
    }
    PropertyValue::List(values)
}
/// Decode the complete current tuple; legacy two-slot references have no pixel.
pub fn unpack(value: &PropertyValue) -> Result<(&str, bool, Option<(i64, i64)>), String> {
    match value {
        PropertyValue::List(values) if values.len() == 2 || values.len() == 3 => {
            match (&values[0], &values[1]) {
                (PropertyValue::Str(reference), PropertyValue::Bool(flag)) => {
                    let pixel = values
                        .get(2)
                        .map(crate::transparent_pixel::pixel_of)
                        .transpose()?;
                    Ok((reference, *flag, pixel))
                }
                _ => Err("picture requires Ref string and LoadTransparent boolean".into()),
            }
        }
        _ => Err("picture requires List([Ref, LoadTransparent, optional Point])".into()),
    }
}

/// Декодировать host-элемент (уже claimed `locate`'ом). Пусто → `Str("")` + дефолт-флаг.
pub fn decode(dialect: PictureDialect, host: &Element) -> Decoded {
    let res = match dialect {
        PictureDialect::Edt => decode_edt(host),
        PictureDialect::Designer => decode_designer(host),
    };
    match res {
        Ok(v) => Decoded::Present(v),
        Err(e) => Decoded::Error(e),
    }
}

/// EDT флага НЕ несёт ⇒ ссылка читается, флаг ВЫВОДИТСЯ дефолт-правилом реконструкции.
fn decode_edt(host: &Element) -> Result<PropertyValue, String> {
    let reference = match host.attr(XSI_TYPE) {
        Some(a) => {
            if a.value != PICTURE_REF {
                return Err(format!(
                    "picture {XSI_TYPE} must be {PICTURE_REF:?}, got {:?}",
                    a.value
                ));
            }
            a.claimed.set(true);
            if host.attrs.iter().any(|at| !at.claimed.get()) {
                return Err("picture has unexpected extra attribute (§1.0)".into());
            }
            if host.children.len() != 1 {
                return Err("picture: expected single <picture> child".into());
            }
            let inner = &host.children[0];
            if inner.local != "picture"
                || !inner.prefix.is_empty()
                || !inner.attrs.is_empty()
                || !inner.children.is_empty()
            {
                return Err("picture: expected <picture>ref</picture> leaf".into());
            }
            inner.claim_with_text();
            inner.text.clone()
        }
        None => {
            // Пустой host (self-closing) → "".
            if !host.attrs.is_empty() || !host.children.is_empty() || !host.text.is_empty() {
                return Err("empty picture must be self-closing".into());
            }
            String::new()
        }
    };
    let lt = default_load_transparent(&reference);
    Ok(pack(reference, lt, None))
}

/// Designer НЕСЁТ флаг явно ⇒ читаем ОБА значения (без сверки с правилом: флаг — данные).
fn decode_designer(host: &Element) -> Result<PropertyValue, String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("Picture container must be attribute-less, no text".into());
    }
    if host.children.is_empty() {
        // <Picture/> — картинки нет; флаг берём канонический для пустой ссылки.
        return Ok(pack(String::new(), default_load_transparent(""), None));
    }
    // <xr:Ref>ref</xr:Ref> + <xr:LoadTransparent>true|false</xr:LoadTransparent>.
    if host.children.len() != 2 && host.children.len() != 3 {
        return Err(format!(
            "Picture: expected Ref, LoadTransparent, optional TransparentPixel, got {} children",
            host.children.len()
        ));
    }
    let r = &host.children[0];
    if r.local != "Ref" || r.prefix != "xr" || !r.attrs.is_empty() || !r.children.is_empty() {
        return Err("Picture: expected <xr:Ref>ref</xr:Ref>".into());
    }
    r.claim_with_text();
    let lt = &host.children[1];
    if lt.local != "LoadTransparent"
        || lt.prefix != "xr"
        || !lt.attrs.is_empty()
        || !lt.children.is_empty()
    {
        return Err("Picture: expected <xr:LoadTransparent>true|false</xr:LoadTransparent>".into());
    }
    let flag = match lt.text.as_str() {
        "true" => true,
        "false" => false,
        other => {
            return Err(format!(
                "Picture <xr:LoadTransparent>: expected true|false, got {other:?} (§1.0)"
            ))
        }
    };
    lt.claim_with_text();
    let pixel = if let Some(p) = host.children.get(2) {
        if p.local != "TransparentPixel"
            || p.prefix != "xr"
            || !p.children.is_empty()
            || !p.text.is_empty()
            || p.attrs.len() != 2
        {
            return Err(
                "Picture: expected empty xr:TransparentPixel with exact x/y attributes".into(),
            );
        }
        let coordinate = |name: &str| -> Result<i64, String> {
            let a = p
                .attr(name)
                .ok_or_else(|| format!("TransparentPixel missing {name}"))?;
            let n = a
                .value
                .parse::<i64>()
                .map_err(|e| format!("TransparentPixel {name} is not an integer: {e}"))?;
            a.claimed.set(true);
            Ok(n)
        };
        let point = (coordinate("x")?, coordinate("y")?);
        p.claim();
        Some(point)
    } else {
        None
    };
    Ok(pack(r.text.clone(), flag, pixel))
}

/// Claim (host claimed выше) — то же, что decode.
pub fn claim(dialect: PictureDialect, host: &Element) {
    let _ = decode(dialect, host);
}

/// Эмитировать host-элемент `<tag>` (имя/ns — из локуса). Пустой → self-closing.
pub fn encode(
    dialect: PictureDialect,
    ns: &str,
    tag: &str,
    value: &PropertyValue,
) -> Result<OutElement, String> {
    let (reference, load_transparent, pixel) = unpack(value)?;
    Ok(match dialect {
        PictureDialect::Edt => {
            if pixel.is_some() || load_transparent != default_load_transparent(reference) {
                return Err("PictureRef transparency requires the typed metadata semantic resource in whole-project EDT emission".into());
            }
            if reference.is_empty() {
                // EDT пустой picture — тег отсутствует; но re-sparsify дефолта уже
                // опустил его. Сюда дойдёт лишь непустой; на всякий — self-closing.
                OutElement::self_closing(ns, tag)
            } else {
                // Флаг EDT НЕ несёт (его нет в формате) — пишем только ссылку.
                let mut host = OutElement::branch(ns, tag).attr(XSI_TYPE, PICTURE_REF);
                host.push(OutElement::leaf("", "picture", reference.to_string()));
                host
            }
        }
        PictureDialect::Designer => {
            if reference.is_empty()
                && load_transparent == default_load_transparent("")
                && pixel.is_none()
            {
                OutElement::self_closing(ns, tag)
            } else {
                let mut host = OutElement::branch(ns, tag);
                host.push(OutElement::leaf("xr", "Ref", reference.to_string()));
                host.push(OutElement::leaf(
                    "xr",
                    "LoadTransparent",
                    if load_transparent { "true" } else { "false" }.to_string(),
                ));
                if let Some((x, y)) = pixel {
                    host.push(
                        OutElement::self_closing("xr", "TransparentPixel")
                            .attr("x", x.to_string())
                            .attr("y", y.to_string()),
                    );
                }
                host
            }
        }
    })
}
