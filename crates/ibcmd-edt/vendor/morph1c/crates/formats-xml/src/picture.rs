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
//! Канонический IR: `List([Str(ref), Bool(loadTransparent)])` (`Str("")` = пусто, дефолт —
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

/// Собрать канонический IR `List([Str(ref), Bool(lt)])`.
fn pack(reference: String, load_transparent: bool) -> PropertyValue {
    PropertyValue::List(vec![
        PropertyValue::Str(reference),
        PropertyValue::Bool(load_transparent),
    ])
}

/// Разобрать канонический IR (строго, §1.0).
fn unpack(value: &PropertyValue) -> Result<(&str, bool), String> {
    match value {
        PropertyValue::List(v) if v.len() == 2 => match (&v[0], &v[1]) {
            (PropertyValue::Str(s), PropertyValue::Bool(b)) => Ok((s.as_str(), *b)),
            _ => Err("picture must be List([Str(ref), Bool(loadTransparent)]) (§1.0)".into()),
        },
        other => Err(format!(
            "picture expects List([Str, Bool]), got {:?}",
            other.kind()
        )),
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
    Ok(pack(reference, lt))
}

/// Designer НЕСЁТ флаг явно ⇒ читаем ОБА значения (без сверки с правилом: флаг — данные).
fn decode_designer(host: &Element) -> Result<PropertyValue, String> {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Err("Picture container must be attribute-less, no text".into());
    }
    if host.children.is_empty() {
        // <Picture/> — картинки нет; флаг берём канонический для пустой ссылки.
        return Ok(pack(String::new(), default_load_transparent("")));
    }
    // <xr:Ref>ref</xr:Ref> + <xr:LoadTransparent>true|false</xr:LoadTransparent>.
    if host.children.len() != 2 {
        return Err(format!(
            "Picture: expected <xr:Ref>+<xr:LoadTransparent>, got {} children",
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
    Ok(pack(r.text.clone(), flag))
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
    let (reference, load_transparent) = unpack(value)?;
    Ok(match dialect {
        PictureDialect::Edt => {
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
            if reference.is_empty() {
                OutElement::self_closing(ns, tag)
            } else {
                let mut host = OutElement::branch(ns, tag);
                host.push(OutElement::leaf("xr", "Ref", reference.to_string()));
                host.push(OutElement::leaf(
                    "xr",
                    "LoadTransparent",
                    if load_transparent { "true" } else { "false" }.to_string(),
                ));
                host
            }
        }
    })
}
