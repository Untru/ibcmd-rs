//! Чтение/запись СОСТАВА СТИЛЯ (`Style`) как сайдкара дескриптора (§1.2/§4) — сиблинг
//! [`crate::schedule_read`]: «дескриптор-read метаданных не трогает тело; whole-config-конвейер
//! присоединяет его отдельным проходом». cf-сборщик эмитит состав телом `<uuid>.0`
//! (`formats_cf::style_body`).
//!
//! # Раскладка и кодирование (RE ERP `Styles/Основной` — ЕДИНСТВЕННЫЙ носитель корпусов,
//! 231 запись, ОБА диалекта; SSL стилей не несёт)
//! * **EDT** — `<obj-dir>/Style.style`: БЕЗ BOM, CRLF, отступ 2 ПРОБЕЛА, хвостовой перевод
//!   строки ЕСТЬ. Корень `<style:Style xmlns:xsi=… xmlns:core=… xmlns:style=…>`; записи —
//!   `<items xsi:type="style:ColorStyleItem|FontStyleItem|BorderStyleItem">` с `<name>` и
//!   значением `<color|font|border xsi:type="core:ColorRef|ColorDef|FontRef|FontDef|BorderRef">`
//!   (внутренние формы — те же, что у StyleItem-значений: `<color>Web.Cream</color>`,
//!   `<red>242</red>`, `<font>Style.TextFont</font>` + тристейт-флаги, `<border>Style.X</border>`).
//! * **Designer** — `<dir>/<Name>/Ext/Style.xml`: С BOM, CRLF, отступ ТАБ, хвостового перевода
//!   строки НЕТ. Корень `<Style … version="2.20|2.21">` (версия — ВХОД чтения,
//!   [`crate::sidecar_version`]); записи — `<Item name="…">` с `<Color>web:X|style:X|#RRGGBB</Color>`,
//!   `<Font ref="style:X|sys:X" [height] [флаги] kind="StyleItem|WindowsFont" [scale]/>`
//!   (абсолют — `faceName=… kind="Absolute" scale="100"`), `<Border ref="style:X"/>`.
//!
//! Канон [`StyleRecord`]: имя записи ДОСЛОВНО (стандартное `FormBackColor` либо
//! `StyleItem.<Имя>` — префикс несут ОБА диалекта) + значение в канонах стиль-значений
//! (`ColorStyle`/`FontStyle`; Border — ССЫЛКА `Style.<Имя>`, Def в составе стиля не
//! witnessed). Порядок записей структурен (== doc-порядку; ERP: оба диалекта идентичны
//! 231/231 — §1.6: оба дают РАВНЫЙ канон).
//!
//! # §1.0-самопроверка на read
//! Пере-сериализация канона в ИСХОДНЫЙ диалект (Designer — ВЕРСИЕЙ ИСТОЧНИКА) ОБЯЗАНА
//! воспроизвести исходные байты — иначе ГРОМКИЙ отказ (незамоделированный
//! атрибут/элемент/порядок сразу виден), как у [`crate::schedule_read`].

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use formats_xml::Element;
use morph1c_core::ir::value::{ColorStyle, FontFace, FontFlags, FontStyle};
use morph1c_core::ir::{MetadataObject, StyleRecord, StyleRecordValue};
use morph1c_core::version::FormatVersion;

use crate::ConvertError;

/// Вид, несущий состав стиля (единственный — сайдкар есть только у `Styles/`).
const STYLE_KIND: &str = "Style";
/// Имя EDT-сайдкара (рядом с `.mdo`).
const EDT_FILE: &str = "Style.style";
/// Имя Designer-сайдкара (внутри `Ext/`).
const DESIGNER_FILE: &str = "Style.xml";
/// UTF-8 BOM — Designer-сайдкар его несёт, EDT нет.
const BOM: char = '\u{FEFF}';

// ---------------------------------------------------------------------------------------------
// ATTACH / WRITE (конвейерные хуки)
// ---------------------------------------------------------------------------------------------

/// Подгрузить состав стиля (если сайдкар существует) в `obj.style_records`. Не-`Style` виды и
/// cf (контейнер) — no-op; Style БЕЗ сайдкара — тоже no-op (coverage-стаб — дескриптор-только).
///
/// §1.0: сайдкар, чья пере-сериализация не воспроизводит исходные байты → ГРОМКИЙ отказ
/// (см. модульный docstring). Empty current item collections are valid SDK styles.
pub fn attach_style(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if kind != STYLE_KIND || format == Format::Cf {
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
    let doc = formats_xml::parse(&bytes)
        .map_err(|e| read_err(format!("style {} XML: {e}", path.display())))?;
    let ctx = format!("style {}", path.display());
    // §1.0-самопроверка: канон обязан пере-сериализоваться в ИСХОДНЫЕ байты (Designer —
    // ВЕРСИЕЙ ИСТОЧНИКА: версия — свойство файла, детектится из его корня, §1.6).
    let (records, back, native_palette) = match format {
        Format::Edt => {
            let r = parse_edt(&doc.root, &ctx).map_err(read_err)?;
            let back = serialize_edt(&r).map_err(read_err)?;
            (r, back, None)
        }
        Format::Designer => {
            let (r, version) = parse_designer(&doc.root, &ctx).map_err(read_err)?;
            let palette = match doc.root.attr("xmlns:pal") {
                Some(attr) if version >= FormatVersion::new(2, 21)
                    && attr.value == "http://v8.1c.ru/8.1/data/ui/colors/palette" => true,
                Some(_) => return Err(read_err("style has an invalid palette namespace binding for its source version".into())),
                None => false,
            };
            // Validate both registered native namespace spellings against the
            // source, without changing values or accepting unknown attributes.
            let back = serialize_designer_with_palette(&r, version, palette).map_err(read_err)?;
            (r, back, Some((version, palette)))
        }
        Format::Cf => unreachable!("cf returned above"),
    };
    let empty_root = if records.is_empty() {
        Some((format == Format::Designer, bytes.ends_with(b"/>") || bytes.ends_with(b"/>\r\n")))
    } else {
        None
    };
    let back = match empty_root {
        Some((_, self_closing)) => empty_root_spelling(back, format, self_closing).map_err(read_err)?,
        None => back,
    };
    if back != bytes {
        return Err(read_err(format!(
            "style {} does not round-trip byte-exactly through the IR (the sidecar carries a \
             shape this codec does not model — refusing to silently drop it, §1.0)",
            path.display()
        )));
    }
    // §1.0: дескриптор-read состав НЕ заполняет — только этот проход.
    if !obj.style_records.is_empty() {
        return Err(read_err(
            "object already carries style records before the sidecar attach (unexpected — the \
             descriptor projection must not populate them)"
                .into(),
        ));
    }
    obj.style_records = records;
    obj.style_native_palette = native_palette;
    obj.style_sidecar_present = true;
    obj.style_empty_root = empty_root;
    Ok(())
}

/// Write-side mirror of [`attach_style`]: эмитить `obj.style_records` рядом с только что
/// записанным дескриптором в раскладке/кодировке ЦЕЛЕВОГО формата. cf (тело собирает
/// cf-ассемблер) и объект без состава — no-op. §1.0: состав у вида БЕЗ раскладки сайдкара —
/// типизированный отказ (тихий дроп запрещён).
pub fn write_style(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if obj.style_records.is_empty() && !obj.style_sidecar_present {
        return Ok(());
    }
    if format == Format::Cf {
        return Ok(());
    }
    let write_err = |reason: String| ConvertError::Write {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason,
    };
    if kind != STYLE_KIND {
        return Err(write_err(
            "object carries style records but only Style has a witnessed style sidecar layout \
             (§1.0 — silent drop forbidden)"
                .into(),
        ));
    }
    let path = sidecar_path(format, descriptor_out).ok_or_else(|| {
        write_err("descriptor path has no parent/stem to anchor the style sidecar at".into())
    })?;
    let bytes = match format {
        Format::Edt => serialize_edt(&obj.style_records).map_err(write_err)?,
        Format::Designer => {
            let target = crate::sidecar_version::write_target();
            match obj.style_native_palette.filter(|(source, _)| *source == target) {
                Some((_, present)) => serialize_designer_with_palette(&obj.style_records, target, present),
                None => serialize_designer(&obj.style_records, target),
            }.map_err(write_err)?
        }
        Format::Cf => unreachable!("cf returned above"),
    };
    let bytes = if obj.style_records.is_empty() {
        let self_closing = obj.style_empty_root
            .filter(|(native, _)| *native == (format == Format::Designer))
            .is_some_and(|(_, closing)| closing);
        empty_root_spelling(bytes, format, self_closing).map_err(write_err)?
    } else {
        bytes
    };
    crate::form_write::write_file(&path, &bytes)
}

/// Rebuild only the empty root's closing token from newly emitted current data.
fn empty_root_spelling(mut bytes: Vec<u8>, format: Format, self_closing: bool) -> Result<Vec<u8>, String> {
    if !self_closing {
        return Ok(bytes);
    }
    let (suffix, replacement): (&[u8], &[u8]) = match format {
        Format::Designer => (b">\r\n</Style>", b"/>"),
        Format::Edt => (b">\r\n</style:Style>\r\n", b"/>\r\n"),
        Format::Cf => return Err("container format has no XML style root".into()),
    };
    if !bytes.ends_with(suffix) {
        return Err("empty style writer produced an unexpected root closure".into());
    }
    bytes.truncate(bytes.len() - suffix.len());
    bytes.extend_from_slice(replacement);
    Ok(bytes)
}

/// Путь сайдкара состава стиля относительно дескриптора: EDT `<obj-dir>/Style.style`,
/// Designer `<dir>/<Name>/Ext/Style.xml`.
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

// ---------------------------------------------------------------------------------------------
// READ — Designer
// ---------------------------------------------------------------------------------------------

/// Designer: `<Style … version="…">` → (канон, witnessed-версия формата источника).
fn parse_designer(root: &Element, ctx: &str) -> Result<(Vec<StyleRecord>, FormatVersion), String> {
    if root.local != "Style" {
        return Err(format!("{ctx}: root is <{}>, expected <Style>", root.local));
    }
    let version = root
        .attr("version")
        .ok_or_else(|| format!("{ctx}: <Style> root has no version attribute (§1.0)"))?;
    let version = crate::sidecar_version::parse_witnessed(&version.value, ctx)?;
    let mut records = Vec::with_capacity(root.children.len());
    for item in &root.children {
        if item.local != "Item" {
            return Err(format!("{ctx}: unknown Designer style element <{}> (§1.0)", item.local));
        }
        let name = item
            .attr("name")
            .ok_or_else(|| format!("{ctx}: <Item> without name attribute (§1.0)"))?
            .value
            .clone();
        let ictx = format!("{ctx}: Item {name:?}");
        let value = match item.children.as_slice() {
            [v] => v,
            other => {
                return Err(format!(
                    "{ictx}: expected exactly one value child, got {} (§1.0)",
                    other.len()
                ))
            }
        };
        let value = match value.local.as_str() {
            "Color" => StyleRecordValue::Color(parse_designer_color(&value.text, &ictx)?),
            "Font" => StyleRecordValue::Font(parse_designer_font(value, &ictx)?),
            "Border" => StyleRecordValue::BorderRef(parse_designer_border(value, &ictx)?),
            other => return Err(format!("{ictx}: unknown value element <{other}> (§1.0)")),
        };
        records.push(StyleRecord { name, value });
    }
    Ok((records, version))
}

/// Designer-цвет: текст `#RRGGBB` (абсолют) | `web:X` | `style:X` (ссылки).
fn parse_designer_color(text: &str, ctx: &str) -> Result<ColorStyle, String> {
    if let Some(hex) = text.strip_prefix('#') {
        let byte = |i: usize| -> Result<u8, String> {
            u8::from_str_radix(hex.get(i..i + 2).unwrap_or(""), 16)
                .map_err(|e| format!("{ctx}: color {text:?} is not #RRGGBB: {e} (§1.0)"))
        };
        if hex.len() != 6 {
            return Err(format!("{ctx}: color {text:?} is not #RRGGBB (§1.0)"));
        }
        return Ok(ColorStyle::Def {
            red: byte(0)?,
            green: byte(2)?,
            blue: byte(4)?,
        });
    }
    if let Some(name) = text.strip_prefix("web:") {
        return Ok(ColorStyle::Ref(format!("Web.{name}")));
    }
    if let Some(name) = text.strip_prefix("style:") {
        return Ok(ColorStyle::Ref(format!("Style.{name}")));
    }
    Err(format!(
        "{ctx}: color {text:?} carries an unwitnessed prefix (only #/web:/style: witnessed in a \
         style body — §1.0)"
    ))
}

/// `"true"|"false"` → bool (§1.0: иное — отказ).
fn parse_bool(v: &str, ctx: &str, what: &str) -> Result<bool, String> {
    match v {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("{ctx}: {what}={other:?} is not true/false (§1.0)")),
    }
}

/// Designer-шрифт `<Font …/>` (самозакрытый; атрибутные формы — те же, что у Designer
/// StyleItem-значений `v8ui:Font`): `ref=… kind="StyleItem|WindowsFont"` — ссылка;
/// `faceName=… kind="Absolute"` — абсолют.
fn parse_designer_font(el: &Element, ctx: &str) -> Result<FontStyle, String> {
    if !el.children.is_empty() {
        return Err(format!("{ctx}: <Font> with children is not witnessed (§1.0)"));
    }
    let mut r_ref: Option<String> = None;
    let mut face_name: Option<String> = None;
    let mut kind: Option<String> = None;
    let mut height: Option<u32> = None;
    let mut scale: Option<u32> = None;
    let mut flags = FontFlags::default();
    for a in &el.attrs {
        if a.name.starts_with("xmlns") {
            continue;
        }
        match a.name.as_str() {
            "ref" => r_ref = Some(a.value.clone()),
            "faceName" => face_name = Some(a.value.clone()),
            "kind" => kind = Some(a.value.clone()),
            "height" => {
                height = Some(a.value.parse().map_err(|e| {
                    format!("{ctx}: height={:?} is not an integer: {e} (§1.0)", a.value)
                })?)
            }
            "scale" => {
                scale = Some(a.value.parse().map_err(|e| {
                    format!("{ctx}: scale={:?} is not an integer: {e} (§1.0)", a.value)
                })?)
            }
            "bold" => flags.bold = Some(parse_bool(&a.value, ctx, "bold")?),
            "italic" => flags.italic = Some(parse_bool(&a.value, ctx, "italic")?),
            "underline" => flags.underline = Some(parse_bool(&a.value, ctx, "underline")?),
            "strikeout" => flags.strikeout = Some(parse_bool(&a.value, ctx, "strikeout")?),
            other => {
                return Err(format!(
                    "{ctx}: unknown <Font> attribute {other:?} (§1.0 — a field this codec does \
                     not model would be silently lost)"
                ))
            }
        }
    }
    match kind.as_deref() {
        Some("StyleItem") | Some("WindowsFont") => {
            let r = r_ref.ok_or_else(|| format!("{ctx}: font ref without ref= (§1.0)"))?;
            let font = match (kind.as_deref(), r.split_once(':')) {
                (Some("StyleItem"), Some(("style", n))) => format!("Style.{n}"),
                (Some("WindowsFont"), Some(("sys", n))) => format!("System.{n}"),
                _ => {
                    return Err(format!(
                        "{ctx}: font ref {r:?} does not pair with kind={kind:?} \
                         (style:↔StyleItem / sys:↔WindowsFont — §1.0)"
                    ))
                }
            };
            if face_name.is_some() {
                return Err(format!("{ctx}: font ref with faceName= (§1.0)"));
            }
            Ok(FontStyle::Ref {
                font,
                height,
                flags,
                scale,
            })
        }
        Some("Absolute") => {
            let face_name =
                face_name.ok_or_else(|| format!("{ctx}: absolute font without faceName= (§1.0)"))?;
            let height =
                height.ok_or_else(|| format!("{ctx}: absolute font without height= (§1.0)"))?;
            if r_ref.is_some() {
                return Err(format!("{ctx}: absolute font with ref= (§1.0)"));
            }
            if scale != Some(100) {
                return Err(format!(
                    "{ctx}: absolute font scale {scale:?} != 100 is not witnessed (§1.0)"
                ));
            }
            // Designer эмитит все четыре флага абсолюта ДЕНСОВО (как у StyleItem-значений);
            // самопроверка byte-round-trip ловит любую разреженную форму.
            let face = FontFace {
                bold: flags.bold.unwrap_or(false),
                italic: flags.italic.unwrap_or(false),
                underline: flags.underline.unwrap_or(false),
                strikeout: flags.strikeout.unwrap_or(false),
            };
            Ok(FontStyle::Def {
                face_name,
                height,
                face,
            })
        }
        other => Err(format!(
            "{ctx}: font kind {other:?} is not witnessed (StyleItem/WindowsFont/Absolute — §1.0)"
        )),
    }
}

/// Designer-рамка `<Border ref="style:X"/>` — ТОЛЬКО ссылка (Def в составе стиля не witnessed).
fn parse_designer_border(el: &Element, ctx: &str) -> Result<String, String> {
    if !el.children.is_empty() {
        return Err(format!("{ctx}: <Border> with children is not witnessed (§1.0)"));
    }
    let mut target: Option<String> = None;
    for a in &el.attrs {
        if a.name.starts_with("xmlns") {
            continue;
        }
        match a.name.as_str() {
            "ref" => {
                let n = a.value.strip_prefix("style:").ok_or_else(|| {
                    format!("{ctx}: border ref {:?} is not style:* (§1.0)", a.value)
                })?;
                target = Some(format!("Style.{n}"));
            }
            other => {
                return Err(format!(
                    "{ctx}: unknown <Border> attribute {other:?} (only ref= witnessed — §1.0)"
                ))
            }
        }
    }
    target.ok_or_else(|| format!("{ctx}: <Border> without ref= (§1.0)"))
}

// ---------------------------------------------------------------------------------------------
// READ — EDT
// ---------------------------------------------------------------------------------------------

/// EDT: `<style:Style>` → канон (симметрия [`parse_designer`], §1.6).
fn parse_edt(root: &Element, ctx: &str) -> Result<Vec<StyleRecord>, String> {
    if root.local != "Style" {
        return Err(format!("{ctx}: root is <{}>, expected <Style>", root.local));
    }
    let mut records = Vec::with_capacity(root.children.len());
    for item in &root.children {
        if item.local != "items" {
            return Err(format!("{ctx}: unknown EDT style element <{}> (§1.0)", item.local));
        }
        let ty = item
            .attr("xsi:type")
            .ok_or_else(|| format!("{ctx}: <items> without xsi:type (§1.0)"))?
            .value
            .clone();
        let name = item
            .child("name")
            .ok_or_else(|| format!("{ctx}: <items> without <name> (§1.0)"))?
            .text
            .clone();
        let ictx = format!("{ctx}: items {name:?}");
        if item.children.len() != 2 {
            return Err(format!(
                "{ictx}: expected exactly <name> + one value child, got {} (§1.0)",
                item.children.len()
            ));
        }
        let value = match ty.as_str() {
            "style:ColorStyleItem" => {
                let el = item
                    .child("color")
                    .ok_or_else(|| format!("{ictx}: ColorStyleItem without <color> (§1.0)"))?;
                StyleRecordValue::Color(parse_edt_color(el, &ictx)?)
            }
            "style:FontStyleItem" => {
                let el = item
                    .child("font")
                    .ok_or_else(|| format!("{ictx}: FontStyleItem without <font> (§1.0)"))?;
                StyleRecordValue::Font(parse_edt_font(el, &ictx)?)
            }
            "style:BorderStyleItem" => {
                let el = item
                    .child("border")
                    .ok_or_else(|| format!("{ictx}: BorderStyleItem without <border> (§1.0)"))?;
                StyleRecordValue::BorderRef(parse_edt_border(el, &ictx)?)
            }
            other => return Err(format!("{ictx}: unknown items xsi:type {other:?} (§1.0)")),
        };
        records.push(StyleRecord { name, value });
    }
    Ok(records)
}

/// EDT-цвет: `core:ColorRef` (`<color>Web.X|Style.X</color>`) | `core:ColorDef`
/// (`<red>/<green>/<blue>`, опущенная компонента = 0).
fn parse_edt_color(el: &Element, ctx: &str) -> Result<ColorStyle, String> {
    let ty = el
        .attr("xsi:type")
        .ok_or_else(|| format!("{ctx}: <color> without xsi:type (§1.0)"))?;
    match ty.value.as_str() {
        "core:ColorRef" => {
            let name = match el.children.as_slice() {
                [c] if c.local == "color" => c.text.clone(),
                _ => return Err(format!("{ctx}: ColorRef must carry exactly <color> (§1.0)")),
            };
            if !name.starts_with("Web.") && !name.starts_with("Style.") {
                return Err(format!(
                    "{ctx}: color ref {name:?} carries an unwitnessed prefix (only Web./Style. \
                     witnessed in a style body — §1.0)"
                ));
            }
            Ok(ColorStyle::Ref(name))
        }
        "core:ColorDef" => {
            let mut rgb = [0u8; 3];
            for c in &el.children {
                let slot = match c.local.as_str() {
                    "red" => 0,
                    "green" => 1,
                    "blue" => 2,
                    other => {
                        return Err(format!("{ctx}: unknown ColorDef element <{other}> (§1.0)"))
                    }
                };
                rgb[slot] = c.text.parse().map_err(|e| {
                    format!("{ctx}: {}={:?} is not a u8: {e} (§1.0)", c.local, c.text)
                })?;
            }
            Ok(ColorStyle::Def {
                red: rgb[0],
                green: rgb[1],
                blue: rgb[2],
            })
        }
        other => Err(format!("{ctx}: color xsi:type {other:?} is not witnessed (§1.0)")),
    }
}

/// EDT-высота шрифта: `"H.0"` → `H` (witnessed-форма EDT-высот стиль-значений).
fn parse_edt_height(t: &str, ctx: &str) -> Result<u32, String> {
    t.strip_suffix(".0")
        .and_then(|h| h.parse().ok())
        .ok_or_else(|| format!("{ctx}: height {t:?} is not the witnessed `H.0` shape (§1.0)"))
}

/// EDT-шрифт: `core:FontRef` (`<font>Style.X|System.X</font>` + опц. height/флаги/scale) |
/// `core:FontDef` (`<faceName>`+`<height>`+true-флаги).
fn parse_edt_font(el: &Element, ctx: &str) -> Result<FontStyle, String> {
    let ty = el
        .attr("xsi:type")
        .ok_or_else(|| format!("{ctx}: <font> without xsi:type (§1.0)"))?;
    match ty.value.as_str() {
        "core:FontRef" => {
            let mut font: Option<String> = None;
            let mut height: Option<u32> = None;
            let mut scale: Option<u32> = None;
            let mut flags = FontFlags::default();
            for c in &el.children {
                match c.local.as_str() {
                    "font" => font = Some(c.text.clone()),
                    "height" => height = Some(parse_edt_height(&c.text, ctx)?),
                    "scale" => {
                        scale = Some(c.text.parse().map_err(|e| {
                            format!("{ctx}: scale {:?} is not an integer: {e} (§1.0)", c.text)
                        })?)
                    }
                    "bold" => flags.bold = Some(parse_bool(&c.text, ctx, "bold")?),
                    "italic" => flags.italic = Some(parse_bool(&c.text, ctx, "italic")?),
                    "underline" => flags.underline = Some(parse_bool(&c.text, ctx, "underline")?),
                    "strikeout" => flags.strikeout = Some(parse_bool(&c.text, ctx, "strikeout")?),
                    other => {
                        return Err(format!("{ctx}: unknown FontRef element <{other}> (§1.0)"))
                    }
                }
            }
            let font = font.ok_or_else(|| format!("{ctx}: FontRef without <font> (§1.0)"))?;
            if !font.starts_with("Style.") && !font.starts_with("System.") {
                return Err(format!(
                    "{ctx}: font ref {font:?} is not Style.*/System.* (§1.0)"
                ));
            }
            Ok(FontStyle::Ref {
                font,
                height,
                flags,
                scale,
            })
        }
        "core:FontDef" => {
            let mut face_name: Option<String> = None;
            let mut height: Option<u32> = None;
            let mut face = FontFace::default();
            for c in &el.children {
                match c.local.as_str() {
                    "faceName" => face_name = Some(c.text.clone()),
                    "height" => height = Some(parse_edt_height(&c.text, ctx)?),
                    // EDT несёт только true-флаги (как у StyleItem-значений).
                    "bold" => face.bold = parse_bool(&c.text, ctx, "bold")?,
                    "italic" => face.italic = parse_bool(&c.text, ctx, "italic")?,
                    "underline" => face.underline = parse_bool(&c.text, ctx, "underline")?,
                    "strikeout" => face.strikeout = parse_bool(&c.text, ctx, "strikeout")?,
                    other => {
                        return Err(format!("{ctx}: unknown FontDef element <{other}> (§1.0)"))
                    }
                }
            }
            Ok(FontStyle::Def {
                face_name: face_name
                    .ok_or_else(|| format!("{ctx}: FontDef without <faceName> (§1.0)"))?,
                height: height.ok_or_else(|| format!("{ctx}: FontDef without <height> (§1.0)"))?,
                face,
            })
        }
        other => Err(format!("{ctx}: font xsi:type {other:?} is not witnessed (§1.0)")),
    }
}

/// EDT-рамка: ТОЛЬКО `core:BorderRef` (`<border>Style.X</border>`).
fn parse_edt_border(el: &Element, ctx: &str) -> Result<String, String> {
    let ty = el
        .attr("xsi:type")
        .ok_or_else(|| format!("{ctx}: <border> without xsi:type (§1.0)"))?;
    if ty.value != "core:BorderRef" {
        return Err(format!(
            "{ctx}: border xsi:type {:?} is not witnessed in a style body (only core:BorderRef \
             — §1.0)",
            ty.value
        ));
    }
    let target = match el.children.as_slice() {
        [c] if c.local == "border" => c.text.clone(),
        _ => return Err(format!("{ctx}: BorderRef must carry exactly <border> (§1.0)")),
    };
    if !target.starts_with("Style.") {
        return Err(format!("{ctx}: border ref {target:?} is not Style.* (§1.0)"));
    }
    Ok(target)
}

// ---------------------------------------------------------------------------------------------
// WRITE (byte-exact — см. §1.0-самопроверку на read)
// ---------------------------------------------------------------------------------------------

/// Designer `Ext/Style.xml`: С BOM, CRLF, ТАБ-отступ, БЕЗ хвостового перевода строки. Корень
/// штампуется `version` заданной версии формата (ns-блок — побайтно witnessed ERP).
fn serialize_designer(records: &[StyleRecord], version: FormatVersion) -> Result<Vec<u8>, String> {
    serialize_designer_with_palette(records, version, version >= FormatVersion::new(2, 21))
}

fn serialize_designer_with_palette(records: &[StyleRecord], version: FormatVersion, palette: bool) -> Result<Vec<u8>, String> {
    let mut out = String::new();
    out.push(BOM);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    out.push_str(&format!(
        "<Style xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" {palette_namespace}\
         xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" \
         xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" \
         xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" \
         xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" \
         xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" \
         xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
         xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"{version}\">\r\n",
        palette_namespace = if palette { "xmlns:pal=\"http://v8.1c.ru/8.1/data/ui/colors/palette\" " } else { "" },
    ));
    for r in records {
        out.push_str(&format!("\t<Item name=\"{}\">\r\n", r.name));
        match &r.value {
            StyleRecordValue::Color(c) => {
                out.push_str(&format!("\t\t<Color>{}</Color>\r\n", designer_color_text(c)));
            }
            StyleRecordValue::Font(f) => {
                out.push_str(&format!("\t\t<Font {}/>\r\n", designer_font_attrs(f)?));
            }
            StyleRecordValue::BorderRef(target) => {
                let short = target.strip_prefix("Style.").ok_or_else(|| {
                    format!("style record {:?}: border ref {target:?} is not Style.* (§1.0)", r.name)
                })?;
                out.push_str(&format!("\t\t<Border ref=\"style:{short}\"/>\r\n"));
            }
        }
        out.push_str("\t</Item>\r\n");
    }
    out.push_str("</Style>");
    Ok(out.into_bytes())
}

/// Designer-текст цвета: Def → `#RRGGBB` (верхний регистр), `Web.X` → `web:X`, `Style.X` →
/// `style:X` (домен ссылок гарантирует ридер).
fn designer_color_text(c: &ColorStyle) -> String {
    match c {
        ColorStyle::Def { red, green, blue } => format!("#{red:02X}{green:02X}{blue:02X}"),
        ColorStyle::Ref(name) => match name.split_once('.') {
            Some(("Web", n)) => format!("web:{n}"),
            Some(("Style", n)) => format!("style:{n}"),
            _ => unreachable!("reader admits only Web./Style. color refs"),
        },
    }
}

/// Designer-атрибуты `<Font …/>` в witnessed-порядке: `ref|faceName, height, флаги, kind,
/// scale` (порядок — как у Designer StyleItem-значений `v8ui:Font`).
fn designer_font_attrs(f: &FontStyle) -> Result<String, String> {
    let flag = |name: &str, v: Option<bool>| -> String {
        v.map(|b| format!(" {name}=\"{b}\"")).unwrap_or_default()
    };
    match f {
        FontStyle::Ref {
            font,
            height,
            flags,
            scale,
        } => {
            let (r, kind) = match font.split_once('.') {
                Some(("Style", n)) => (format!("style:{n}"), "StyleItem"),
                Some(("System", n)) => (format!("sys:{n}"), "WindowsFont"),
                _ => return Err(format!("font ref {font:?} is not Style.*/System.* (§1.0)")),
            };
            let mut attrs = format!("ref=\"{r}\"");
            if let Some(h) = height {
                attrs.push_str(&format!(" height=\"{h}\""));
            }
            attrs.push_str(&flag("bold", flags.bold));
            attrs.push_str(&flag("italic", flags.italic));
            attrs.push_str(&flag("underline", flags.underline));
            attrs.push_str(&flag("strikeout", flags.strikeout));
            attrs.push_str(&format!(" kind=\"{kind}\""));
            if let Some(s) = scale {
                attrs.push_str(&format!(" scale=\"{s}\""));
            }
            Ok(attrs)
        }
        FontStyle::Def {
            face_name,
            height,
            face,
        } => Ok(format!(
            "faceName=\"{face_name}\" height=\"{height}\" bold=\"{}\" italic=\"{}\" \
             underline=\"{}\" strikeout=\"{}\" kind=\"Absolute\" scale=\"100\"",
            face.bold, face.italic, face.underline, face.strikeout
        )),
    }
}

/// EDT `Style.style`: БЕЗ BOM, CRLF, отступ 2 пробела, хвостовой перевод строки ЕСТЬ.
fn serialize_edt(records: &[StyleRecord]) -> Result<Vec<u8>, String> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    out.push_str(
        "<style:Style xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" \
         xmlns:style=\"http://g5.1c.ru/v8/dt/style\">\r\n",
    );
    for r in records {
        let ty = match &r.value {
            StyleRecordValue::Color(_) => "ColorStyleItem",
            StyleRecordValue::Font(_) => "FontStyleItem",
            StyleRecordValue::BorderRef(_) => "BorderStyleItem",
        };
        out.push_str(&format!("  <items xsi:type=\"style:{ty}\">\r\n"));
        out.push_str(&format!("    <name>{}</name>\r\n", r.name));
        match &r.value {
            StyleRecordValue::Color(ColorStyle::Ref(name)) => {
                out.push_str("    <color xsi:type=\"core:ColorRef\">\r\n");
                out.push_str(&format!("      <color>{name}</color>\r\n"));
                out.push_str("    </color>\r\n");
            }
            StyleRecordValue::Color(ColorStyle::Def { red, green, blue }) => {
                out.push_str("    <color xsi:type=\"core:ColorDef\">\r\n");
                // EDT опускает нулевые компоненты (канон EDT ColorDef, §1.6).
                for (tag, v) in [("red", *red), ("green", *green), ("blue", *blue)] {
                    if v != 0 {
                        out.push_str(&format!("      <{tag}>{v}</{tag}>\r\n"));
                    }
                }
                out.push_str("    </color>\r\n");
            }
            StyleRecordValue::Font(f) => {
                edt_font(&mut out, f);
            }
            StyleRecordValue::BorderRef(target) => {
                out.push_str("    <border xsi:type=\"core:BorderRef\">\r\n");
                out.push_str(&format!("      <border>{target}</border>\r\n"));
                out.push_str("    </border>\r\n");
            }
        }
        out.push_str("  </items>\r\n");
    }
    out.push_str("</style:Style>\r\n");
    Ok(out.into_bytes())
}

/// EDT-элемент `<font xsi:type="core:FontRef|FontDef">` (формы — как у EDT
/// StyleItem-значений: Ref несёт тристейт-флаги, Def — только true-флаги).
fn edt_font(out: &mut String, f: &FontStyle) {
    let flag = |out: &mut String, name: &str, v: Option<bool>| {
        if let Some(b) = v {
            out.push_str(&format!("      <{name}>{b}</{name}>\r\n"));
        }
    };
    match f {
        FontStyle::Ref {
            font,
            height,
            flags,
            scale,
        } => {
            out.push_str("    <font xsi:type=\"core:FontRef\">\r\n");
            out.push_str(&format!("      <font>{font}</font>\r\n"));
            if let Some(h) = height {
                out.push_str(&format!("      <height>{h}.0</height>\r\n"));
            }
            flag(out, "bold", flags.bold);
            flag(out, "italic", flags.italic);
            flag(out, "underline", flags.underline);
            flag(out, "strikeout", flags.strikeout);
            if let Some(s) = scale {
                out.push_str(&format!("      <scale>{s}</scale>\r\n"));
            }
            out.push_str("    </font>\r\n");
        }
        FontStyle::Def {
            face_name,
            height,
            face,
        } => {
            out.push_str("    <font xsi:type=\"core:FontDef\">\r\n");
            out.push_str(&format!("      <faceName>{face_name}</faceName>\r\n"));
            out.push_str(&format!("      <height>{height}.0</height>\r\n"));
            // EDT несёт только true-флаги абсолюта (false = омиссия).
            flag(out, "bold", face.bold.then_some(true));
            flag(out, "italic", face.italic.then_some(true));
            flag(out, "underline", face.underline.then_some(true));
            flag(out, "strikeout", face.strikeout.then_some(true));
            out.push_str("    </font>\r\n");
        }
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::version::ERP;

    /// Витнессированные дисковые байты ОДНОГО И ТОГО ЖЕ среза состава ERP `Основной` в двух
    /// диалектах (дословно из корпуса: стандартный web-цвет, стандартный шрифт, рамка,
    /// кастом style-цвет, кастом абсолют-цвет, кастом флаговый шрифт).
    const DESIGNER_SRC: &str = concat!(
        "\u{FEFF}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<Style xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" ",
        "xmlns:style=\"http://v8.1c.ru/8.1/data/ui/style\" ",
        "xmlns:sys=\"http://v8.1c.ru/8.1/data/ui/fonts/system\" ",
        "xmlns:v8ui=\"http://v8.1c.ru/8.1/data/ui\" ",
        "xmlns:web=\"http://v8.1c.ru/8.1/data/ui/colors/web\" ",
        "xmlns:win=\"http://v8.1c.ru/8.1/data/ui/colors/windows\" ",
        "xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" ",
        "xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" ",
        "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n",
        "\t<Item name=\"FormBackColor\">\r\n\t\t<Color>web:Cream</Color>\r\n\t</Item>\r\n",
        "\t<Item name=\"TextFont\">\r\n",
        "\t\t<Font ref=\"style:TextFont\" kind=\"StyleItem\"/>\r\n\t</Item>\r\n",
        "\t<Item name=\"ControlBorder\">\r\n",
        "\t\t<Border ref=\"style:ControlBorder\"/>\r\n\t</Item>\r\n",
        "\t<Item name=\"StyleItem.БыстрыеОтборыФонГруппы\">\r\n",
        "\t\t<Color>style:FormBackColor</Color>\r\n\t</Item>\r\n",
        "\t<Item name=\"StyleItem.ПоясняющийТекст\">\r\n",
        "\t\t<Color>#807A59</Color>\r\n\t</Item>\r\n",
        "\t<Item name=\"StyleItem.ЗаголовокУдаленногоРеквизитаШрифт\">\r\n",
        "\t\t<Font ref=\"style:TextFont\" bold=\"false\" italic=\"false\" ",
        "underline=\"false\" strikeout=\"true\" kind=\"StyleItem\"/>\r\n\t</Item>\r\n",
        "</Style>"
    );

    const EDT_SRC: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<style:Style xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
        "xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:style=\"http://g5.1c.ru/v8/dt/style\">\r\n",
        "  <items xsi:type=\"style:ColorStyleItem\">\r\n",
        "    <name>FormBackColor</name>\r\n",
        "    <color xsi:type=\"core:ColorRef\">\r\n",
        "      <color>Web.Cream</color>\r\n",
        "    </color>\r\n",
        "  </items>\r\n",
        "  <items xsi:type=\"style:FontStyleItem\">\r\n",
        "    <name>TextFont</name>\r\n",
        "    <font xsi:type=\"core:FontRef\">\r\n",
        "      <font>Style.TextFont</font>\r\n",
        "    </font>\r\n",
        "  </items>\r\n",
        "  <items xsi:type=\"style:BorderStyleItem\">\r\n",
        "    <name>ControlBorder</name>\r\n",
        "    <border xsi:type=\"core:BorderRef\">\r\n",
        "      <border>Style.ControlBorder</border>\r\n",
        "    </border>\r\n",
        "  </items>\r\n",
        "  <items xsi:type=\"style:ColorStyleItem\">\r\n",
        "    <name>StyleItem.БыстрыеОтборыФонГруппы</name>\r\n",
        "    <color xsi:type=\"core:ColorRef\">\r\n",
        "      <color>Style.FormBackColor</color>\r\n",
        "    </color>\r\n",
        "  </items>\r\n",
        "  <items xsi:type=\"style:ColorStyleItem\">\r\n",
        "    <name>StyleItem.ПоясняющийТекст</name>\r\n",
        "    <color xsi:type=\"core:ColorDef\">\r\n",
        "      <red>128</red>\r\n",
        "      <green>122</green>\r\n",
        "      <blue>89</blue>\r\n",
        "    </color>\r\n",
        "  </items>\r\n",
        "  <items xsi:type=\"style:FontStyleItem\">\r\n",
        "    <name>StyleItem.ЗаголовокУдаленногоРеквизитаШрифт</name>\r\n",
        "    <font xsi:type=\"core:FontRef\">\r\n",
        "      <font>Style.TextFont</font>\r\n",
        "      <bold>false</bold>\r\n",
        "      <italic>false</italic>\r\n",
        "      <underline>false</underline>\r\n",
        "      <strikeout>true</strikeout>\r\n",
        "    </font>\r\n",
        "  </items>\r\n",
        "</style:Style>\r\n"
    );

    fn parse(format: Format, src: &str) -> Vec<StyleRecord> {
        let doc = formats_xml::parse(src.as_bytes()).expect("xml");
        match format {
            Format::Edt => parse_edt(&doc.root, "t").expect("edt"),
            Format::Designer => parse_designer(&doc.root, "t").expect("designer").0,
            Format::Cf => unreachable!(),
        }
    }

    /// §1.6: ОБА диалекта дают РАВНЫЙ канон (тот же порядок, те же значения).
    #[test]
    fn both_dialects_yield_the_same_canonical_records() {
        let e = parse(Format::Edt, EDT_SRC);
        let d = parse(Format::Designer, DESIGNER_SRC);
        assert_eq!(e, d, "edt canon == designer canon (§1.6)");
        assert_eq!(e.len(), 6);
        assert_eq!(e[0].name, "FormBackColor");
        assert_eq!(
            e[0].value,
            StyleRecordValue::Color(ColorStyle::Ref("Web.Cream".into()))
        );
        assert_eq!(
            e[2].value,
            StyleRecordValue::BorderRef("Style.ControlBorder".into())
        );
        assert_eq!(
            e[4].value,
            StyleRecordValue::Color(ColorStyle::Def {
                red: 128,
                green: 122,
                blue: 89
            })
        );
        match &e[5].value {
            StyleRecordValue::Font(FontStyle::Ref { flags, .. }) => {
                assert_eq!(flags.strikeout, Some(true));
                assert_eq!(flags.bold, Some(false));
            }
            other => panic!("font record expected, got {other:?}"),
        }
    }

    /// Byte-exact round-trip в ОБОИХ диалектах И кросс-диалектно (это же — §1.0-самопроверка
    /// на read).
    #[test]
    fn serialize_reproduces_the_witnessed_bytes() {
        let d = parse(Format::Designer, DESIGNER_SRC);
        assert_eq!(
            String::from_utf8(serialize_designer(&d, ERP).unwrap()).unwrap(),
            DESIGNER_SRC,
            "Designer byte-exact (2.20)"
        );
        let e = parse(Format::Edt, EDT_SRC);
        assert_eq!(
            String::from_utf8(serialize_edt(&e).unwrap()).unwrap(),
            EDT_SRC,
            "EDT byte-exact"
        );
        // Кросс-диалектно: канон из EDT пишется в байт-точный Designer и наоборот (§1.6).
        assert_eq!(
            String::from_utf8(serialize_designer(&e, ERP).unwrap()).unwrap(),
            DESIGNER_SRC,
            "edt canon → designer bytes"
        );
        assert_eq!(
            String::from_utf8(serialize_edt(&d).unwrap()).unwrap(),
            EDT_SRC,
            "designer canon → edt bytes"
        );
    }

    /// §1.0: незамоделированный атрибут / чужой префикс ссылки — ГРОМКИЙ отказ.
    #[test]
    fn unwitnessed_shapes_are_refused() {
        let bad = DESIGNER_SRC.replace("web:Cream", "win:Window");
        let doc = formats_xml::parse(bad.as_bytes()).expect("xml");
        let err = parse_designer(&doc.root, "t").unwrap_err();
        assert!(err.contains("unwitnessed prefix"), "{err}");

        let bad = DESIGNER_SRC.replace(
            "<Border ref=\"style:ControlBorder\"/>",
            "<Border ref=\"style:ControlBorder\" width=\"1\"/>",
        );
        let doc = formats_xml::parse(bad.as_bytes()).expect("xml");
        let err = parse_designer(&doc.root, "t").unwrap_err();
        assert!(err.contains("Border"), "{err}");

        let bad = EDT_SRC.replace("core:BorderRef", "core:BorderDef");
        let doc = formats_xml::parse(bad.as_bytes()).expect("xml");
        let err = parse_edt(&doc.root, "t").unwrap_err();
        assert!(err.contains("BorderRef"), "{err}");
    }
}
