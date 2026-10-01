//! Explicit adapter resource for semantic per-use PictureRef transparency.
//! It never substitutes pictures or replays source XML; every record resolves
//! against the current fully claimed typed form and its declared metadata UUID.
use std::collections::{HashMap, HashSet};

use morph1c_core::ir::form::*;
use morph1c_core::ir::{PropertyValue, Uuid};
use morph1c_core::spec::forms::command as fc;
use serde::{Deserialize, Serialize};

use super::{FormError, fields, pictures};

pub const PICTURE_SEMANTICS_RESOURCE: &str = "ibcmd-picture-semantics.v1.json";
const SCHEMA: &str = "urn:ibcmd:source-extension:picture-semantics:1";

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    schema: String,
    version: u32,
    form: FormPictureSemantics,
}
fn error(reason: impl Into<String>) -> FormError {
    FormError::Frame(format!("picture semantics: {}", reason.into()))
}
fn key(binding: &PictureSemanticBinding) -> Result<String, FormError> {
    serde_json::to_string(binding).map_err(|e| error(e.to_string()))
}
fn decode(bytes: &[u8]) -> Result<Resource, FormError> {
    let resource: Resource = serde_json::from_slice(bytes).map_err(|e| error(e.to_string()))?;
    if resource.schema != SCHEMA || resource.version != 1 {
        return Err(error("unknown schema/version"));
    }
    let mut bindings = HashSet::new();
    for record in &resource.form.records {
        if record.pixel.is_some() && !record.load_transparent {
            return Err(error("pixel requires LoadTransparent=true"));
        }
        if !bindings.insert(key(&record.binding)?) {
            return Err(error("duplicate picture binding"));
        }
    }
    Ok(resource)
}

/// Decode only this closed resource grammar. Binding validation follows after
/// complete metadata and typed form attachment; unknown properties reject here.
pub fn read_picture_semantics_resource(bytes: &[u8]) -> Result<FormPictureSemantics, FormError> {
    Ok(decode(bytes)?.form)
}

/// JSON lexical whitespace/order does not carry semantics. This comparison is
/// restricted to the versioned, fully decoded resource, including every value.
pub fn same_picture_semantics_resource(a: &[u8], b: &[u8]) -> bool {
    match (decode(a), decode(b)) {
        (Ok(a), Ok(b)) if a.form.form_uuid == b.form.form_uuid => {
            let map = |resource: Resource| {
                resource
                    .form
                    .records
                    .into_iter()
                    .map(|row| key(&row.binding).map(|address| (address, row)))
                    .collect::<Result<HashMap<_, _>, _>>()
            };
            match (map(a), map(b)) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            }
        }
        _ => false,
    }
}

fn record(
    binding: PictureSemanticBinding,
    value: &PropertyValue,
) -> Result<Option<PictureSemanticRecord>, FormError> {
    let (reference, load_transparent) = fields::picture_ref_lt(value)?;
    let pixel = fields::picture_pixel(value).map(|(x, y)| PictureSemanticPixel { x, y });
    if let PropertyValue::List(parts) = value {
        if parts.len() == 3 && pixel.is_none() {
            return Err(error("malformed typed pixel"));
        }
    }
    if pixel.is_some() && !load_transparent {
        return Err(error("pixel requires LoadTransparent=true"));
    }
    if reference.is_empty() || reference.starts_with("abs:") {
        return Ok(None);
    }
    Ok(Some(PictureSemanticRecord {
        binding,
        reference: reference.into(),
        load_transparent,
        pixel,
    }))
}
fn value_of(record: &PictureSemanticRecord) -> PropertyValue {
    match record.pixel {
        Some(pixel) => fields::picture_canon_px(record.reference.clone(), (pixel.x, pixel.y)),
        None => fields::picture_canon(record.reference.clone(), record.load_transparent),
    }
}

fn identity(edge: PictureControlEdge, kind: &str, id: i64, name: &str) -> PictureControlIdentity {
    PictureControlIdentity {
        edge,
        kind: kind.into(),
        id,
        name: name.into(),
    }
}
fn visit(
    body: &mut FormBody,
    f: &mut impl FnMut(PictureSemanticBinding, &mut PropertyValue) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for command in &mut body.commands {
        if let Some((_, value)) = command
            .properties
            .iter_mut()
            .find(|(id, _)| *id == fc::F_PICTURE)
        {
            f(
                PictureSemanticBinding::Command {
                    id: command.id,
                    name: command.name.clone(),
                },
                value,
            )?;
        }
    }
    items(&mut body.items, &[], PictureControlEdge::Root, f)?;
    if let Some(bar) = &mut body.auto_command_bar {
        let path = [identity(
            PictureControlEdge::AutoCommandBar,
            "AutoCommandBar",
            bar.id,
            &bar.name,
        )];
        items(&mut bar.items, &path, PictureControlEdge::Child, f)?;
    }
    Ok(())
}
fn items(
    list: &mut [FormItem],
    ancestors: &[PictureControlIdentity],
    edge: PictureControlEdge,
    f: &mut impl FnMut(PictureSemanticBinding, &mut PropertyValue) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for item in list {
        let mut path = ancestors.to_vec();
        path.push(identity(edge, item.kind.as_str(), item.id, &item.name));
        for slot in pictures::picture_slots(&item.kind) {
            let bag = if slot.ext {
                &mut item.ext_info
            } else {
                &mut item.properties
            };
            if let Some((_, value)) = bag.iter_mut().find(|(id, _)| *id == slot.id) {
                f(
                    PictureSemanticBinding::Control {
                        path: path.clone(),
                        slot: slot.stem.into(),
                        choice: None,
                    },
                    value,
                )?;
            }
        }
        for slot in pictures::choice_picture_slots(&item.kind) {
            let bag = if slot.ext {
                &mut item.ext_info
            } else {
                &mut item.properties
            };
            if let Some((_, value)) = bag.iter_mut().find(|(id, _)| *id == slot.id) {
                let PropertyValue::List(choices) = value else {
                    return Err(error("malformed typed ChoiceList"));
                };
                let mut occurrences = HashMap::<String, usize>::new();
                for choice in choices {
                    let PropertyValue::List(parts) = choice else {
                        return Err(error("malformed typed choice"));
                    };
                    if parts.len() != 2 && parts.len() != 3 {
                        return Err(error("malformed choice arity"));
                    }
                    let (PropertyValue::Localized(presentation), PropertyValue::Value(value)) =
                        (&parts[0], &parts[1])
                    else {
                        return Err(error("malformed typed choice identity"));
                    };
                    let canonical =
                        serde_json::to_string(&parts[1]).map_err(|e| error(e.to_string()))?;
                    let projected = serde_json::to_string(&(presentation, &canonical))
                        .map_err(|e| error(e.to_string()))?;
                    let ordinal = occurrences.entry(projected).or_default();
                    let binding = PictureSemanticBinding::Control {
                        path: path.clone(),
                        slot: slot.stem.into(),
                        choice: Some(PictureChoiceIdentity {
                            presentation: presentation
                                .iter()
                                .map(|(lang, text)| (lang.as_str().into(), text.clone()))
                                .collect(),
                            value_kind: value.kind,
                            value_canonical: canonical,
                            duplicate_ordinal: *ordinal,
                        }),
                    };
                    *ordinal = ordinal
                        .checked_add(1)
                        .ok_or_else(|| error("choice occurrence overflow"))?;
                    if let Some(picture) = parts.get_mut(2) {
                        f(binding, picture)?;
                    }
                }
            }
        }
        items(&mut item.children, &path, PictureControlEdge::Child, f)?;
        items(&mut item.additions, &path, PictureControlEdge::Addition, f)?;
        if let Some(table) = &mut item.auto_table {
            items(
                std::slice::from_mut(table.as_mut()),
                &path,
                PictureControlEdge::AutoTable,
                f,
            )?;
        }
        if let Some(bar) = &mut item.auto_command_bar {
            let mut bar_path = path.clone();
            bar_path.push(identity(
                PictureControlEdge::AutoCommandBar,
                "AutoCommandBar",
                bar.id,
                &bar.name,
            ));
            items(&mut bar.items, &bar_path, PictureControlEdge::Child, f)?;
        }
        if let Some(menu) = &mut item.context_menu {
            if let DecoratorBody::ContextMenu(body) = &mut menu.body {
                let mut menu_path = path.clone();
                menu_path.push(identity(
                    PictureControlEdge::ContextMenu,
                    "ContextMenu",
                    menu.id,
                    &menu.name,
                ));
                items(&mut body.items, &menu_path, PictureControlEdge::Child, f)?;
            }
        }
    }
    Ok(())
}

/// Called after CommonPicture metadata BOOL resolution. An input resource is
/// consumed once, bound to exact current Ref/known slot/typed identity, and then
/// ALL reference records are recomputed from current body values in both origins.
pub fn bind_picture_semantics(
    body: &mut FormBody,
    form_uuid: Uuid,
    edt_origin: bool,
) -> Result<(), FormError> {
    let pending = if edt_origin && body.picture_resource_selection.is_some() {
        body.picture_semantics.take()
    } else {
        None
    };
    let mut remaining = HashMap::new();
    if let Some(pending) = pending {
        if pending.form_uuid != form_uuid {
            return Err(error("form UUID does not match declared metadata"));
        }
        for row in pending.records {
            if remaining.insert(key(&row.binding)?, row).is_some() {
                return Err(error("duplicate picture binding"));
            }
        }
    }
    let mut rows = Vec::new();
    let mut seen = HashSet::new();
    visit(body, &mut |binding, value| {
        if let Some(current) = record(binding, value)? {
            let address = key(&current.binding)?;
            if !seen.insert(address.clone()) {
                return Err(error("ambiguous current picture binding"));
            }
            if let Some(row) = remaining.remove(&address) {
                if row.reference != current.reference {
                    return Err(error("resource Ref differs from current typed picture"));
                }
                if row.pixel.is_some() && !row.load_transparent {
                    return Err(error("pixel requires LoadTransparent=true"));
                }
                *value = value_of(&row);
            }
            rows.push(record(current.binding, value)?.expect("reference remains bound"));
        }
        Ok(())
    })?;
    if !remaining.is_empty() {
        return Err(error(format!(
            "{} stale/unknown picture bindings",
            remaining.len()
        )));
    }
    body.picture_semantics = if rows.is_empty() {
        None
    } else {
        Some(FormPictureSemantics {
            form_uuid,
            records: rows,
        })
    };
    Ok(())
}

fn sdk_default(body: &FormBody, reference: &str) -> Result<bool, FormError> {
    if reference.starts_with("CommonPicture.") {
        body.common_picture_transparency
            .get(reference)
            .copied()
            .ok_or_else(|| error("CommonPicture metadata not resolved"))
    } else {
        Ok(reference.starts_with(fc::PICTURE_STD_PREFIX))
    }
}
fn needed(body: &FormBody, row: &PictureSemanticRecord) -> Result<bool, FormError> {
    Ok(row.pixel.is_some() || row.load_transparent != sdk_default(body, &row.reference)?)
}

/// Build EDT-compatible descriptor values plus a resource from CURRENT typed
/// values. A source selection stores only bindings/file presence, never values.
pub fn project_picture_semantics(
    body: &FormBody,
    form_uuid: Uuid,
) -> Result<(FormBody, Option<Vec<u8>>), FormError> {
    let mut projected = body.clone();
    bind_picture_semantics(&mut projected, form_uuid, false)?;
    let mut selected = HashSet::new();
    if let Some(selection) = &body.picture_resource_selection {
        for binding in selection {
            if !selected.insert(key(binding)?) {
                return Err(error("duplicate transport selection"));
            }
        }
    }
    let mut rows = Vec::new();
    if let Some(model) = &projected.picture_semantics {
        for row in &model.records {
            if selected.remove(&key(&row.binding)?) || needed(body, row)? {
                rows.push(row.clone());
            }
        }
    }
    if !selected.is_empty() {
        return Err(error("stale resource selection"));
    }
    let bytes = if rows.is_empty() && body.picture_resource_selection.is_none() {
        None
    } else {
        let resource = Resource {
            schema: SCHEMA.into(),
            version: 1,
            form: FormPictureSemantics {
                form_uuid,
                records: rows,
            },
        };
        let mut bytes = serde_json::to_vec_pretty(&resource).map_err(|e| error(e.to_string()))?;
        bytes.push(b'\n');
        Some(bytes)
    };
    let defaults = body.common_picture_transparency.clone();
    visit(&mut projected, &mut |binding, value| {
        if let Some(row) = record(binding, value)? {
            let flag = if row.reference.starts_with("CommonPicture.") {
                defaults
                    .get(&row.reference)
                    .copied()
                    .ok_or_else(|| error("CommonPicture metadata not resolved"))?
            } else {
                row.reference.starts_with(fc::PICTURE_STD_PREFIX)
            };
            *value = fields::picture_canon(row.reference, flag);
        }
        Ok(())
    })?;
    Ok((projected, bytes))
}

pub fn picture_semantics_resource_count(body: &FormBody) -> Result<Option<usize>, FormError> {
    let Some(model) = &body.picture_semantics else {
        return Ok(body.picture_resource_selection.as_ref().map(|_| 0));
    };
    let mut selected = HashSet::new();
    for binding in body
        .picture_resource_selection
        .as_deref()
        .unwrap_or_default()
    {
        if !selected.insert(key(binding)?) {
            return Err(error("duplicate transport selection"));
        }
    }
    let mut count: usize = 0;
    for row in &model.records {
        if selected.remove(&key(&row.binding)?) || needed(body, row)? {
            count = count
                .checked_add(1)
                .ok_or_else(|| error("picture resource count overflow"))?;
        }
    }
    if !selected.is_empty() {
        return Err(error("stale resource selection"));
    }
    Ok(if count == 0 && body.picture_resource_selection.is_none() {
        None
    } else {
        Some(count)
    })
}
