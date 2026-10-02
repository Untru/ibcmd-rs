//! Root MdPicture presence and both CURRENT persisted Point properties.
use crate::{descriptor::Element, emit::OutElement};
use morph1c_core::{
    engine::Decoded,
    ir::{FieldId, MetadataObject, PropertyValue, Uuid},
};
use serde::{Deserialize, Serialize};

fn value_point(point: Option<(i64, i64)>) -> PropertyValue {
    PropertyValue::List(
        point
            .map(|(x, y)| vec![PropertyValue::Int(x), PropertyValue::Int(y)])
            .unwrap_or_default(),
    )
}
fn checked_point(point: (i64, i64)) -> Result<(i64, i64), String> {
    // The original SDK Point EClass declares x/y as EInt, not unbounded integers.
    for coordinate in [point.0, point.1] {
        i32::try_from(coordinate)
            .map_err(|_| "MdPicture Point coordinate exceeds signed EInt range".to_string())?;
    }
    Ok(point)
}
fn optional_point(value: &PropertyValue) -> Result<Option<(i64, i64)>, String> {
    if matches!(value,PropertyValue::List(v) if v.is_empty()) {
        Ok(None)
    } else {
        crate::transparent_pixel::pixel_of(value)
            .and_then(checked_point)
            .map(Some)
    }
}
pub fn present(point: Option<(i64, i64)>) -> PropertyValue {
    present_with_glyph(point, None)
}
pub fn present_with_glyph(point: Option<(i64, i64)>, glyph: Option<(i64, i64)>) -> PropertyValue {
    PropertyValue::List(vec![value_point(point), value_point(glyph)])
}
fn unpack(value: &PropertyValue) -> Result<(Option<(i64, i64)>, Option<(i64, i64)>), String> {
    match value {
        PropertyValue::List(v) => match v.as_slice() {
            [point, glyph] => Ok((optional_point(point)?, optional_point(glyph)?)),
            _ => Err(
                "MdPicture requires exactly its transparentPixel and glyph nullable Points".into(),
            ),
        },
        _ => Err("MdPicture requires typed presence plus two nullable Points".into()),
    }
}
pub fn point(value: &PropertyValue) -> Result<Option<(i64, i64)>, String> {
    unpack(value).map(|v| v.0)
}
pub fn glyph(value: &PropertyValue) -> Result<Option<(i64, i64)>, String> {
    unpack(value).map(|v| v.1)
}
pub fn decode(host: &Element) -> Decoded {
    host.claim_with_text();
    if !host.attrs.is_empty() || !host.text.trim().is_empty() {
        return Decoded::Error("MdPicture contains unknown attributes/text".into());
    }
    let (mut point, mut glyph) = (None, None);
    for child in &host.children {
        if !child.prefix.is_empty() {
            return Decoded::Error("qualified unknown MdPicture field".into());
        }
        let slot = match child.local.as_str() {
            "transparentPixel" => &mut point,
            "glyph" => &mut glyph,
            _ => return Decoded::Error("unknown MdPicture Point field".into()),
        };
        if slot.is_some() {
            return Decoded::Error("duplicate MdPicture Point field".into());
        }
        match decode_point(child) {
            Decoded::Present(value) => match crate::transparent_pixel::pixel_of(&value) {
                Ok(p) => match checked_point(p) {
                    Ok(p) => *slot = Some(p),
                    Err(e) => return Decoded::Error(e),
                },
                Err(e) => return Decoded::Error(e),
            },
            Decoded::Error(e) => return Decoded::Error(e),
            _ => return Decoded::Error("missing MdPicture Point".into()),
        }
    }
    Decoded::Present(present_with_glyph(point, glyph))
}
pub fn claim(host: &Element) {
    let _ = decode(host);
}
pub fn encode(ns: &str, tag: &str, value: &PropertyValue) -> Result<OutElement, String> {
    let (point, glyph) = unpack(value)?;
    let mut out = OutElement::branch(ns, tag);
    // Persisted Ecore feature order, as observed with original FormattingXmlResource.
    for (tag, p) in [("glyph", glyph), ("transparentPixel", point)] {
        if let Some(p) = p {
            out.push(crate::transparent_pixel::encode(
                "",
                tag,
                &value_point(Some(p)),
            )?);
        }
    }
    Ok(out)
}

pub const RESOURCE: &str = "ibcmd-root-picture-semantics.v1.json";
const SCHEMA: &str = "urn:ibcmd:source-extension:root-picture-semantics:1";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum Slot {
    Logo,
    Splash,
    MainSectionPicture,
}
impl Slot {
    fn field(self) -> FieldId {
        use morph1c_core::spec::metadata::configuration as c;
        match self {
            Self::Logo => c::F_LOGO,
            Self::Splash => c::F_SPLASH,
            Self::MainSectionPicture => c::F_MAIN_SECTION_PICTURE,
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Logo => "Logo",
            Self::Splash => "Splash",
            Self::MainSectionPicture => "MainSectionPicture",
        }
    }
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Point {
    x: i64,
    y: i64,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    slot: Slot,
    glyph: Point,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    schema: String,
    version: u8,
    owner_uuid: Uuid,
    owner_kind: String,
    pictures: Vec<Row>,
}
fn decode_resource(bytes: &[u8]) -> Result<Resource, String> {
    let r: Resource =
        serde_json::from_slice(bytes).map_err(|e| format!("root picture resource: {e}"))?;
    if r.schema != SCHEMA
        || r.version != 1
        || r.owner_kind != "Configuration"
        || r.pictures.is_empty()
    {
        return Err("unknown or empty root picture resource".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for row in &r.pictures {
        checked_point((row.glyph.x, row.glyph.y))?;
        if !seen.insert(row.slot) {
            return Err("duplicate root picture resource slot".into());
        }
    }
    Ok(r)
}
fn rows(root: &MetadataObject) -> Result<Vec<Row>, String> {
    if root.kind.as_str() != "Configuration" {
        return Err("root picture resource owner must be Configuration".into());
    }
    let mut out = Vec::new();
    for slot in [Slot::MainSectionPicture, Slot::Logo, Slot::Splash] {
        if root
            .properties
            .iter()
            .filter(|(id, _)| *id == slot.field())
            .count()
            > 1
        {
            return Err("duplicate root picture property".into());
        }
        if let Some(value) = root.get(slot.field()) {
            if let Some((x, y)) = glyph(value)? {
                validate_body(root, slot)?;
                out.push(Row {
                    slot,
                    glyph: Point { x, y },
                });
            }
        }
    }
    Ok(out)
}
fn validate_body(root: &MetadataObject, slot: Slot) -> Result<(), String> {
    if root
        .config_pictures
        .iter()
        .filter(|p| p.slot == slot.name())
        .count()
        != 1
    {
        return Err(format!(
            "root picture resource slot {} has no unique current image body",
            slot.name()
        ));
    }
    let v = root
        .get(slot.field())
        .ok_or_else(|| "root picture resource references an undeclared slot".to_string())?;
    unpack(v)?;
    Ok(())
}
pub fn resource_count(root: &MetadataObject) -> Result<Option<usize>, String> {
    let rows = rows(root)?;
    Ok((!rows.is_empty()).then_some(rows.len()))
}
pub fn resource_bytes(root: &MetadataObject) -> Result<Option<Vec<u8>>, String> {
    let pictures = rows(root)?;
    if pictures.is_empty() {
        return Ok(None);
    }
    serde_json::to_vec_pretty(&Resource {
        schema: SCHEMA.into(),
        version: 1,
        owner_uuid: root.uuid,
        owner_kind: root.kind.as_str().into(),
        pictures,
    })
    .map(Some)
    .map_err(|e| e.to_string())
}
pub fn apply_resource(root: &mut MetadataObject, bytes: &[u8]) -> Result<(), String> {
    let resource = decode_resource(bytes)?;
    if root.kind.as_str() != resource.owner_kind || root.uuid != resource.owner_uuid {
        return Err("root picture resource owner UUID/kind mismatch".into());
    }
    for row in &resource.pictures {
        validate_body(root, row.slot)?;
        if glyph(root.get(row.slot.field()).expect("validated slot"))?.is_some() {
            return Err("root picture glyph has duplicate semantic carriers".into());
        }
    }
    for row in resource.pictures {
        let field = row.slot.field();
        let current = point(root.get(field).expect("validated slot"))?;
        root.properties
            .iter_mut()
            .find(|(id, _)| *id == field)
            .expect("validated slot")
            .1 = present_with_glyph(current, Some((row.glyph.x, row.glyph.y)));
    }
    Ok(())
}
pub fn same_resource(a: &[u8], b: &[u8]) -> bool {
    matches!((decode_resource(a),decode_resource(b)),(Ok(a),Ok(b)) if a==b)
}

// Original FormattingXmlResource accepts both its attributed default save shape
// and the element shape used by genuine EDT descriptors. Claim all coordinates;
// never permit duplicate or mixed representations of one Point.
fn decode_point(host: &Element) -> Decoded {
    if host.attrs.is_empty() {
        return crate::transparent_pixel::decode(host);
    }
    host.claim_with_text();
    if !host.children.is_empty() || !host.text.trim().is_empty() {
        return Decoded::Error(
            "MdPicture Point cannot mix attributes with text/coordinate elements".into(),
        );
    }
    let (mut x, mut y) = (None, None);
    for a in &host.attrs {
        let slot = match a.name.as_str() {
            "x" => &mut x,
            "y" => &mut y,
            _ => return Decoded::Error("unknown MdPicture Point attribute".into()),
        };
        if slot.is_some() {
            return Decoded::Error("duplicate MdPicture Point attribute".into());
        }
        match a.value.parse::<i64>() {
            Ok(v) => {
                *slot = Some(v);
                a.claimed.set(true);
            }
            Err(e) => {
                return Decoded::Error(format!(
                    "MdPicture Point coordinate is not an integer: {e}"
                ));
            }
        }
    }
    match checked_point((x.unwrap_or(0), y.unwrap_or(0))) {
        Ok(point) => Decoded::Present(value_point(Some(point))),
        Err(e) => Decoded::Error(e),
    }
}
