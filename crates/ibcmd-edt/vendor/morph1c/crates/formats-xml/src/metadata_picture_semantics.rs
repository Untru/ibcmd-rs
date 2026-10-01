//! Typed adapter carrier for metadata-command PictureRef transparency.
//! All values live in current command properties; selection contains only UUIDs.
use crate::picture;
use morph1c_core::ir::{FieldId, MetadataObject, Uuid};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
};

pub const RESOURCE: &str = "ibcmd-metadata-picture-semantics.v1.json";
const SCHEMA: &str = "urn:ibcmd:source-extension:metadata-picture-semantics:1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pixel {
    x: i64,
    y: i64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    command_uuid: Uuid,
    command_kind: String,
    slot: Slot,
    reference: String,
    load_transparent: bool,
    pixel: Option<Pixel>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Slot {
    Picture,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    schema: String,
    version: u32,
    owner_uuid: Uuid,
    owner_kind: String,
    pictures: Vec<Row>,
}
fn field(obj: &MetadataObject) -> Option<FieldId> {
    let kind = obj.kind.as_str();
    if !kind.ends_with(".Command") && kind != "CommonCommand" && kind != "CommandGroup" {
        return None;
    }
    morph1c_core::spec::spec_for(kind)?
        .fields
        .iter()
        .find(|f| f.name == "picture")
        .map(|f| f.id)
}
fn targets(obj: &MetadataObject) -> Vec<&MetadataObject> {
    std::iter::once(obj)
        .filter(|c| matches!(c.kind.as_str(), "CommonCommand" | "CommandGroup"))
        .chain(obj.children.iter())
        .filter(|c| field(c).is_some())
        .collect()
}
fn validate_resource(resource: &Resource) -> Result<(), String> {
    if resource.schema != SCHEMA || resource.version != 1 {
        return Err("metadata picture resource: unknown schema/version".into());
    }
    if resource.pictures.is_empty() {
        return Err("metadata picture resource: empty carrier".into());
    }
    let mut seen = BTreeSet::new();
    for row in &resource.pictures {
        picture::unpack(&picture::pack(
            row.reference.clone(),
            row.load_transparent,
            row.pixel.as_ref().map(|p| (p.x, p.y)),
        ))?;
        if !seen.insert(row.command_uuid) {
            return Err("metadata picture resource: duplicate command binding".into());
        }
    }
    Ok(())
}
fn decode(bytes: &[u8]) -> Result<Resource, String> {
    let r: Resource =
        serde_json::from_slice(bytes).map_err(|e| format!("metadata picture resource: {e}"))?;
    validate_resource(&r)?;
    Ok(r)
}
fn tuple(obj: &MetadataObject, id: FieldId) -> Result<(String, bool, Option<(i64, i64)>), String> {
    if obj.properties.iter().filter(|(f, _)| *f == id).count() > 1 {
        return Err("metadata picture: duplicate Picture field".into());
    }
    if let Some(value) = obj.get(id) {
        let (reference, flag, pixel) = picture::unpack(value)?;
        Ok((reference.into(), flag, pixel))
    } else {
        // The registered PictureRef default is the empty reference and true flag.
        // Read it from the authoritative spec rather than inventing a root default.
        let spec = morph1c_core::spec::spec_for(obj.kind.as_str())
            .ok_or("metadata picture: missing registered spec")?;
        let value = spec
            .fields
            .iter()
            .find(|f| f.id == id)
            .and_then(|f| f.default.as_ref())
            .ok_or("metadata picture: missing Picture default")?;
        let (reference, flag, pixel) = picture::unpack(value)?;
        Ok((reference.into(), flag, pixel))
    }
}
fn resource(obj: &MetadataObject) -> Result<Option<Resource>, String> {
    let declared = targets(obj);
    let mut ids = BTreeSet::new();
    for command in &declared {
        if !ids.insert(command.uuid) {
            return Err("metadata picture: ambiguous command UUID".into());
        }
    }
    let mut selected = BTreeSet::new();
    for id in &obj.metadata_picture_resource_commands {
        if !selected.insert(*id) {
            return Err("metadata picture: duplicate resource selection".into());
        }
        if !ids.contains(id) {
            return Err(
                "metadata picture: resource selection names deleted/unknown command".into(),
            );
        }
    }
    let mut pictures = Vec::new();
    for command in declared {
        let (reference, load_transparent, pixel) =
            tuple(command, field(command).expect("registered picture"))?;
        if selected.contains(&command.uuid)
            || pixel.is_some()
            || load_transparent != picture::default_load_transparent(&reference)
        {
            pictures.push(Row {
                command_uuid: command.uuid,
                command_kind: command.kind.as_str().into(),
                slot: Slot::Picture,
                reference,
                load_transparent,
                pixel: pixel.map(|(x, y)| Pixel { x, y }),
            });
        }
    }
    if pictures.is_empty() {
        return Ok(None);
    }
    Ok(Some(Resource {
        schema: SCHEMA.into(),
        version: 1,
        owner_uuid: obj.uuid,
        owner_kind: obj.kind.as_str().into(),
        pictures,
    }))
}
/// Validate the entire binding set before mutating any current semantic value.
pub fn apply(obj: &mut MetadataObject, bytes: &[u8]) -> Result<(), String> {
    let r = decode(bytes)?;
    if r.owner_uuid != obj.uuid || r.owner_kind != obj.kind.as_str() {
        return Err("metadata picture resource: owner UUID/kind mismatch".into());
    }
    let declared = targets(obj);
    let mut map = BTreeMap::new();
    for command in declared {
        if map.insert(command.uuid, command).is_some() {
            return Err("metadata picture resource: ambiguous declared command UUID".into());
        }
    }
    for row in &r.pictures {
        let command = map
            .get(&row.command_uuid)
            .ok_or("metadata picture resource: command deleted/unknown")?;
        if command.kind.as_str() != row.command_kind {
            return Err("metadata picture resource: command kind mismatch".into());
        }
        let id = field(command).ok_or("metadata picture resource: unknown slot")?;
        if tuple(command, id)?.0 != row.reference {
            return Err("metadata picture resource: stale current PictureRef binding".into());
        }
    }
    drop(map);
    for row in &r.pictures {
        let command = if obj.uuid == row.command_uuid && field(obj).is_some() {
            &mut *obj
        } else {
            obj.children
                .iter_mut()
                .find(|c| c.uuid == row.command_uuid)
                .expect("validated command")
        };
        let id = field(command).expect("validated picture field");
        let value = picture::pack(
            row.reference.clone(),
            row.load_transparent,
            row.pixel.as_ref().map(|p| (p.x, p.y)),
        );
        if let Some(entry) = command.properties.iter_mut().find(|(f, _)| *f == id) {
            entry.1 = value;
        } else {
            command.properties.push((id, value));
        }
    }
    obj.metadata_picture_resource_commands = r.pictures.iter().map(|r| r.command_uuid).collect();
    Ok(())
}
/// Project only descriptor PictureRef values to the ordinary SDK shape. The
/// returned resource is rebuilt from the same CURRENT typed values each time.
pub fn project(obj: &MetadataObject) -> Result<(Cow<'_, MetadataObject>, Option<Vec<u8>>), String> {
    let Some(r) = resource(obj)? else {
        return Ok((Cow::Borrowed(obj), None));
    };
    let bytes = serde_json::to_vec_pretty(&r).map_err(|e| e.to_string())?;
    let mut projected = obj.clone();
    for row in r.pictures {
        let command = if projected.uuid == row.command_uuid && field(&projected).is_some() {
            &mut projected
        } else {
            projected
                .children
                .iter_mut()
                .find(|c| c.uuid == row.command_uuid)
                .expect("validated command")
        };
        let id = field(command).expect("registered field");
        let value = picture::pack(
            row.reference.clone(),
            picture::default_load_transparent(&row.reference),
            None,
        );
        if let Some(entry) = command.properties.iter_mut().find(|(f, _)| *f == id) {
            entry.1 = value;
        } else {
            command.properties.push((id, value));
        }
    }
    Ok((Cow::Owned(projected), Some(bytes)))
}
pub fn resource_count(obj: &MetadataObject) -> Result<Option<usize>, String> {
    Ok(resource(obj)?.map(|r| r.pictures.len()))
}
/// Closed carrier records are a mapping by declared command identity. This does
/// not normalize source metadata, picture values, or any semantic ordered array.
pub fn same_resource(a: &[u8], b: &[u8]) -> bool {
    let (Ok(a), Ok(b)) = (decode(a), decode(b)) else {
        return false;
    };
    if a.owner_uuid != b.owner_uuid || a.owner_kind != b.owner_kind {
        return false;
    }
    let map = |r: Resource| {
        r.pictures
            .into_iter()
            .map(|r| (r.command_uuid, r))
            .collect::<BTreeMap<_, _>>()
    };
    map(a) == map(b)
}
