//! Nullable typed chart Picture values. Descriptor codecs never invent bitmap bytes.
use super::*;
use crate::picture::{PictureDialect, decode, encode, pack, unpack};
use morph1c_core::engine::Decoded;
use morph1c_core::ir::form::ChartPicture;

pub(super) fn read(el: &Element, path: &str, native: bool) -> Result<ChartValue, FormError> {
    el.claim();
    if el.attrs.is_empty() && el.children.is_empty() && el.text.is_empty() {
        return Ok(ChartValue::Absent);
    }
    if (native
        && el
            .children
            .iter()
            .any(|c| c.prefix == "xr" && c.local == "Abs"))
        || (!native
            && el
                .attr("xsi:type")
                .is_some_and(|a| a.value == "core:PictureDef"))
    {
        return read_definition(el, path, native);
    }
    let dialect = if native {
        PictureDialect::Designer
    } else {
        PictureDialect::Edt
    };
    match decode(dialect, el) {
        Decoded::Present(v) => {
            let (reference, load_transparent, pixel) = unpack(&v).map_err(frame)?;
            if pixel.is_some() {
                return Err(frame(format!(
                    "chart {path}: PictureRef has unexpected per-use Pixel"
                )));
            }
            if reference.is_empty() {
                return Err(frame(format!(
                    "chart {path}: nonnull PictureRef requires current reference"
                )));
            }
            Ok(ChartValue::Picture(Box::new(ChartPicture::Reference {
                reference: reference.to_owned(),
                load_transparent,
            })))
        }
        Decoded::Error(e) => Err(frame(format!("chart {path}: {e}"))),
        _ => Err(frame(format!(
            "chart {path}: current Picture could not be decoded"
        ))),
    }
}
pub(super) fn write(
    name: &str,
    value: &ChartValue,
    path: &str,
    native: bool,
) -> Result<OutElement, FormError> {
    let ns = if native { "d4p1" } else { "" };
    match value {
        ChartValue::Absent => Ok(OutElement::self_closing(ns, name)),
        ChartValue::Picture(p) => match p.as_ref() {
            ChartPicture::Reference {
                reference,
                load_transparent,
            } => {
                if reference.is_empty() {
                    return Err(frame(format!(
                        "chart {path}: nonnull PictureRef requires current reference"
                    )));
                }
                let current = pack(reference.clone(), *load_transparent, None);
                encode(
                    if native {
                        PictureDialect::Designer
                    } else {
                        PictureDialect::Edt
                    },
                    ns,
                    name,
                    &current,
                )
                .map_err(frame)
            }
            ChartPicture::Definition {
                file_name,
                transparent_pixel,
                glyph,
                ..
            } => {
                if native {
                    if glyph.is_some() {
                        return Err(frame(format!(
                            "chart {path}: current PictureDef glyph requires bound typed transport"
                        )));
                    }
                    super::super::picture_semantics::validate_asset_path(file_name)?;
                    let mut host = OutElement::branch("d4p1", name);
                    host.push(OutElement::leaf("xr", "Abs", file_name.clone()));
                    host.push(OutElement::leaf(
                        "xr",
                        "LoadTransparent",
                        if transparent_pixel.is_some() {
                            "true"
                        } else {
                            "false"
                        },
                    ));
                    if let Some(point) = transparent_pixel.filter(|point| *point != (-1, -1)) {
                        host.push(native_point("TransparentPixel", point, path)?);
                    }
                    Ok(host)
                } else {
                    let mut host = OutElement::branch("", name).attr("xsi:type", "core:PictureDef");
                    for (tag, point) in [("transparentPixel", transparent_pixel), ("glyph", glyph)]
                    {
                        if let Some(point) = point {
                            let point = checked_point(*point, path)?;
                            let value = morph1c_core::ir::value::PropertyValue::List(vec![
                                morph1c_core::ir::value::PropertyValue::Int(point.0),
                                morph1c_core::ir::value::PropertyValue::Int(point.1),
                            ]);
                            host.push(
                                crate::transparent_pixel::encode("", tag, &value).map_err(frame)?,
                            );
                        }
                    }
                    host.self_closing = host.children.is_empty();
                    Ok(host)
                }
            }
        },
        _ => Err(frame(format!(
            "chart {path}: expected nullable typed Picture"
        ))),
    }
}

fn checked_point(point: (i64, i64), path: &str) -> Result<(i64, i64), FormError> {
    i32::try_from(point.0).map_err(|_| frame(format!("chart {path}: Point.x outside EInt")))?;
    i32::try_from(point.1).map_err(|_| frame(format!("chart {path}: Point.y outside EInt")))?;
    Ok(point)
}
pub(super) fn native_point(
    name: &str,
    point: (i64, i64),
    path: &str,
) -> Result<OutElement, FormError> {
    let point = checked_point(point, path)?;
    Ok(OutElement::self_closing("xr", name)
        .attr("x", point.0.to_string())
        .attr("y", point.1.to_string()))
}
fn read_definition(el: &Element, path: &str, native: bool) -> Result<ChartValue, FormError> {
    let mut seen = std::collections::HashSet::new();
    let (mut file_name, mut pixel, mut glyph, mut lt) = (None, None, None, None);
    if !el.text.is_empty() {
        return Err(frame(format!(
            "chart {path}: text outside PictureDef children"
        )));
    }
    if !native {
        claim_xsi(el, "core:PictureDef", path)?;
    }
    expect_attrs_claimed(el, path)?;
    for child in &el.children {
        if !seen.insert(child.local.as_str()) {
            return Err(frame(format!("chart {path}: duplicate PictureDef field")));
        }
        match (native, child.prefix.as_str(), child.local.as_str()) {
            (true, "xr", "Abs") => {
                let name = leaf_text(child, path)?;
                super::super::picture_semantics::validate_asset_path(&name)?;
                file_name = Some(name);
            }
            (true, "xr", "LoadTransparent") => {
                lt = Some(parse_bool(&leaf_text(child, path)?, path)?)
            }
            (true, "xr", "TransparentPixel") => pixel = Some(read_point(child, path)?),
            (false, "", "transparentPixel") => pixel = Some(read_point(child, path)?),
            (false, "", "glyph") => glyph = Some(read_point(child, path)?),
            _ => return Err(frame(format!("chart {path}: unknown PictureDef field"))),
        }
    }
    let file_name = if native {
        let flag = lt.ok_or_else(|| {
            frame(format!(
                "chart {path}: PictureDef missing current LoadTransparent"
            ))
        })?;
        if !flag && pixel.is_some() {
            return Err(frame(format!(
                "chart {path}: PictureDef Pixel contradicts nullable-point LoadTransparent"
            )));
        }
        if flag && pixel.is_none() {
            pixel = Some((-1, -1));
        }
        file_name.ok_or_else(|| frame(format!("chart {path}: PictureDef missing declared Abs")))?
    } else {
        String::new()
    };
    Ok(ChartValue::Picture(Box::new(ChartPicture::Definition {
        file_name,
        bytes: Vec::new(),
        transparent_pixel: pixel,
        glyph,
    })))
}
fn read_point(el: &Element, path: &str) -> Result<(i64, i64), FormError> {
    match crate::md_picture::decode_point(el) {
        Decoded::Present(value) => checked_point(
            crate::transparent_pixel::pixel_of(&value).map_err(frame)?,
            path,
        ),
        Decoded::Error(error) => Err(frame(format!("chart {path}: {error}"))),
        _ => Err(frame(format!("chart {path}: Point was not fully decoded"))),
    }
}
