//! Explicit adapter resource for semantic per-use PictureRef transparency.
//! It never substitutes pictures or replays source XML; every record resolves
//! against the current fully claimed typed form and its declared metadata UUID.
use std::collections::{HashMap, HashSet};

use morph1c_core::ir::form::*;
use morph1c_core::ir::{PropertyValue, Uuid};
use morph1c_core::spec::forms::command as fc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{FormError, fields, pictures};

pub const PICTURE_SEMANTICS_RESOURCE: &str = "ibcmd-picture-semantics.v1.json";
const SCHEMA: &str = "urn:ibcmd:source-extension:picture-semantics:1";

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    schema: String,
    version: u32,
    form: FormPictureSemantics,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    asset_checks: Vec<AssetCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    definition_glyphs: Vec<GlyphRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    definition_points: Vec<PointRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    native_point_presence: Vec<PictureSemanticBinding>,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetCheck {
    binding: PictureSemanticBinding,
    length: u64,
    sha256: String,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GlyphRecord {
    binding: PictureSemanticBinding,
    glyph: PictureSemanticPixel,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointRecord {
    binding: PictureSemanticBinding,
    point: PictureSemanticPixel,
}
fn omitted_point(point: (i64, i64)) -> bool {
    point.0 == -1 || point.1 == -1
}
fn point_transport(binding: &PictureSemanticBinding, point: (i64, i64)) -> bool {
    omitted_point(point)
        && !(matches!(binding, PictureSemanticBinding::Chart { .. }) && point == (-1, -1))
}
fn checks(assets: &[PictureSemanticAsset]) -> Result<Vec<AssetCheck>, FormError> {
    assets
        .iter()
        .map(|asset| {
            Ok(AssetCheck {
                binding: asset.binding.clone(),
                length: u64::try_from(asset.bytes.len())
                    .map_err(|_| error("asset length overflow"))?,
                sha256: format!("{:x}", Sha256::digest(&asset.bytes)),
            })
        })
        .collect()
}
pub(crate) fn validate_asset_path(path: &str) -> Result<(), FormError> {
    if path.is_empty()
        || path.contains(['\\', ':', '\0', '<', '>', '|', '"', '?', '*'])
        || path.starts_with('/')
        || path.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with(['.', ' '])
                || part.chars().any(|c| c.is_control())
        })
    {
        return Err(error("unsafe declared picture asset path"));
    }
    let head = path
        .split('/')
        .next()
        .expect("nonempty path")
        .to_ascii_lowercase();
    if [
        "form.xml",
        "module.bsl",
        "help.xml",
        "ibcmd-picture-semantics.v1.json",
        "ibcmd-event-semantics.v1.json",
        "ibcmd-chart-semantics.v1.json",
    ]
    .contains(&head.as_str())
    {
        return Err(error("asset path collides with a declared form body role"));
    }
    Ok(())
}
fn error(reason: impl Into<String>) -> FormError {
    FormError::Frame(format!("picture semantics: {}", reason.into()))
}
fn key(binding: &PictureSemanticBinding) -> Result<String, FormError> {
    serde_json::to_string(binding).map_err(|e| error(e.to_string()))
}
fn decode(bytes: &[u8]) -> Result<Resource, FormError> {
    let mut resource: Resource =
        super::strict_resource::parse(bytes).map_err(|e| error(e.to_string()))?;
    if resource.schema != SCHEMA || resource.version != 1 {
        return Err(error("unknown schema/version"));
    }
    if checks(&resource.form.assets)? != resource.asset_checks {
        return Err(error("asset length/SHA256 mismatch"));
    }
    validate_assets(&resource.form)?;
    let mut point_keys = HashSet::new();
    for row in &resource.definition_points {
        crate::md_picture::point(&crate::md_picture::present(Some((
            row.point.x,
            row.point.y,
        ))))
        .map_err(error)?;
        if !point_keys.insert(key(&row.binding)?)
            || !omitted_point((row.point.x, row.point.y))
            || !resource
                .form
                .assets
                .iter()
                .any(|a| a.binding == row.binding)
            || !resource
                .form
                .records
                .iter()
                .any(|r| r.binding == row.binding && r.pixel == Some(row.point))
        {
            return Err(error("duplicate/unbound nonprojectable definition Point"));
        }
    }
    let mut presence = HashSet::new();
    for binding in &resource.native_point_presence {
        if !presence.insert(key(binding)?)
            || !resource.form.assets.iter().any(|a| a.binding == *binding)
            || point_keys.contains(&key(binding)?)
        {
            return Err(error("duplicate/unbound native Point presence"));
        }
    }
    resource.form.native_point_presence = resource.native_point_presence.clone();
    let mut glyphs = HashSet::new();
    for row in &resource.definition_glyphs {
        if !glyphs.insert(key(&row.binding)?)
            || !resource
                .form
                .assets
                .iter()
                .any(|asset| asset.binding == row.binding)
        {
            return Err(error("duplicate/unbound definition glyph"));
        }
        let value = fields::picture_with_glyph(
            fields::picture_canon(String::new(), false),
            Some((row.glyph.x, row.glyph.y)),
        )?;
        fields::picture_glyph(&value)?;
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
    let resource = decode(bytes)?;
    if !resource.definition_glyphs.is_empty() || !resource.definition_points.is_empty() {
        return Err(error("native glyph carrier is not an EDT picture resource"));
    }
    Ok(resource.form)
}

/// JSON lexical whitespace/order does not carry semantics. This comparison is
/// restricted to the versioned, fully decoded resource, including every value.
pub fn same_picture_semantics_resource(a: &[u8], b: &[u8]) -> bool {
    match (decode(a), decode(b)) {
        (Ok(a), Ok(b))
            if a.form.form_uuid == b.form.form_uuid
                && a.form.assets == b.form.assets
                && a.definition_glyphs == b.definition_glyphs
                && a.definition_points == b.definition_points
                && a.native_point_presence == b.native_point_presence =>
        {
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
pub(crate) fn visit(
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
    chart_attributes(
        &mut body.data_attributes,
        &mut Vec::new(),
        &|a| PictureChartAttributeIdentity::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        f,
    )?;
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
fn choice_without_pictures(value: &PropertyValue) -> Result<PropertyValue, FormError> {
    let (current, presentation, _) = fields::choice_wrapper_parts(value)?;
    let current = match current {
        PropertyValue::Value(_) => current.clone(),
        PropertyValue::List(values) => PropertyValue::List(
            values
                .iter()
                .map(choice_without_pictures)
                .collect::<Result<_, _>>()?,
        ),
        _ => return Err(error("malformed choice parameter current value")),
    };
    Ok(fields::choice_wrapper_value(
        current,
        presentation.to_vec(),
        None,
    ))
}
fn parameter_wrapper(
    value: &mut PropertyValue,
    path: &[PictureControlIdentity],
    name: &str,
    ancestors: &[PictureParameterIdentity],
    occurrences: &mut HashMap<String, usize>,
    f: &mut impl FnMut(PictureSemanticBinding, &mut PropertyValue) -> Result<(), FormError>,
) -> Result<(), FormError> {
    let (current, presentation, _) = fields::choice_wrapper_parts(value)?;
    let canonical = serde_json::to_string(&choice_without_pictures(current)?)
        .map_err(|e| error(e.to_string()))?;
    let presentation: Vec<_> = presentation
        .iter()
        .map(|(lang, text)| (lang.as_str().to_string(), text.clone()))
        .collect();
    let address = serde_json::to_string(&(name, &canonical, &presentation))
        .map_err(|e| error(e.to_string()))?;
    let ordinal = occurrences.entry(address).or_default();
    let mut wrappers = ancestors.to_vec();
    wrappers.push(PictureParameterIdentity {
        presentation,
        value_canonical: canonical,
        duplicate_ordinal: *ordinal,
    });
    *ordinal = ordinal
        .checked_add(1)
        .ok_or_else(|| error("choice occurrence overflow"))?;
    let current = if let PropertyValue::List(parts) = value {
        if matches!(parts.first(), Some(PropertyValue::Localized(_))) {
            if let Some(picture) = parts.get_mut(2) {
                f(
                    PictureSemanticBinding::ChoiceParameter {
                        path: path.to_vec(),
                        name: name.into(),
                        wrappers: wrappers.clone(),
                    },
                    picture,
                )?;
            }
            &mut parts[1]
        } else {
            value
        }
    } else {
        value
    };
    if let PropertyValue::List(values) = current {
        let mut occurrences = HashMap::new();
        for value in values {
            parameter_wrapper(value, path, name, &wrappers, &mut occurrences, f)?;
        }
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
        for slot in pictures::choice_parameter_picture_slots(&item.kind) {
            let bag = if slot.ext {
                &mut item.ext_info
            } else {
                &mut item.properties
            };
            if let Some((_, value)) = bag.iter_mut().find(|(id, _)| *id == slot.id) {
                let PropertyValue::List(parameters) = value else {
                    return Err(error("malformed choice parameters"));
                };
                let mut occurrences = HashMap::new();
                for parameter in parameters {
                    let PropertyValue::List(parts) = parameter else {
                        return Err(error("malformed choice parameter"));
                    };
                    if parts.len() == 1 {
                        fields::choice_param_item(&PropertyValue::List(parts.clone()))?;
                        continue;
                    }
                    if parts.len() != 2 {
                        return Err(error("choice parameter arity"));
                    }
                    let PropertyValue::Str(name) = &parts[0] else {
                        return Err(error("choice parameter name"));
                    };
                    let name = name.clone();
                    parameter_wrapper(&mut parts[1], &path, &name, &[], &mut occurrences, f)?;
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
    // Validate pending resources atomically against the entire CURRENT typed graph.
    if edt_origin && body.picture_resource_selection.is_some() {
        let mut next = body.clone();
        bind_picture_semantics_inner(&mut next, form_uuid, true)?;
        *body = next;
        return Ok(());
    }
    bind_picture_semantics_inner(body, form_uuid, edt_origin)
}
fn bind_picture_semantics_inner(
    body: &mut FormBody,
    form_uuid: Uuid,
    edt_origin: bool,
) -> Result<(), FormError> {
    let native_point_presence = body
        .picture_semantics
        .as_ref()
        .map(|m| m.native_point_presence.clone())
        .unwrap_or_default();
    let mut assets = body
        .picture_semantics
        .as_ref()
        .map(|model| model.assets.clone())
        .unwrap_or_default();
    let pending_assets = edt_origin && body.picture_resource_selection.is_some();
    let mut seen_chart = HashSet::new();
    chart_definitions(body, &mut |binding, file_name, bytes| {
        let address = key(&binding)?;
        if !seen_chart.insert(address.clone()) {
            return Err(error("duplicate chart definition binding"));
        }
        if pending_assets {
            let asset = assets
                .iter()
                .find(|asset| asset.binding == binding)
                .ok_or_else(|| error("chart definition has no owned asset resource"))?;
            if !file_name.is_empty() && *file_name != asset.path {
                return Err(error("chart descriptor filename differs from resource"));
            }
            if !bytes.is_empty() && *bytes != asset.bytes {
                return Err(error("chart descriptor bytes differ from resource"));
            }
            *file_name = asset.path.clone();
            *bytes = asset.bytes.clone();
        } else if file_name.is_empty() {
            *file_name = format!(
                "ibcmd-picture-assets/{}.bin",
                format!("{:x}", Sha256::digest(address.as_bytes()))
            );
        }
        validate_asset_path(file_name)?;
        Ok(())
    })?;
    if assets.iter().any(|asset| {
        matches!(asset.binding, PictureSemanticBinding::Chart { .. })
            && !seen_chart.contains(&key(&asset.binding).unwrap_or_default())
    }) {
        return Err(error("deleted chart asset owner"));
    }
    assets.retain(|asset| !matches!(asset.binding, PictureSemanticBinding::Chart { .. }));
    chart_definitions(body, &mut |binding, path, bytes| {
        assets.push(PictureSemanticAsset {
            binding,
            path: path.clone(),
            bytes: bytes.clone(),
        });
        Ok(())
    })?;
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
        let address = key(&binding)?;
        if let Some(row) = remaining.get(&address) {
            if row.reference.starts_with("abs-file:") {
                let (reference, _) = fields::picture_ref_lt(value)?;
                if !reference.is_empty() && reference != row.reference {
                    return Err(error("asset resource Ref differs from current picture"));
                }
                if reference.is_empty()
                    && fields::picture_pixel(value) != row.pixel.map(|p| (p.x, p.y))
                {
                    return Err(error(
                        "asset descriptor Point was edited independently from its resource",
                    ));
                }
                if !matches!(
                    binding,
                    PictureSemanticBinding::ChoiceParameter { .. }
                        | PictureSemanticBinding::Chart { .. }
                ) {
                    return Err(error("asset resource is not a declared definition"));
                }
                let glyph = fields::picture_glyph(value)?;
                *value = fields::picture_with_glyph(value_of(row), glyph)?;
            }
        }
        if matches!(binding, PictureSemanticBinding::ChoiceParameter { .. })
            && fields::picture_ref_lt(value)?.0.is_empty()
        {
            return Err(error(
                "choice FormPicture marker has no declared asset resource",
            ));
        }
        if let Some(current) = record(binding, value)? {
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
                let glyph = fields::picture_glyph(value)?;
                *value = fields::picture_with_glyph(value_of(&row), glyph)?;
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
    body.picture_semantics = if rows.is_empty() && assets.is_empty() {
        None
    } else {
        Some(FormPictureSemantics {
            form_uuid,
            records: rows,
            assets,
            native_point_presence,
        })
    };
    if let Some(model) = &body.picture_semantics {
        validate_assets(model)?;
    }
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
    Ok(row.reference.starts_with("abs-file:")
        || row.pixel.is_some()
        || row.load_transparent != sdk_default(body, &row.reference)?)
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
                assets: projected
                    .picture_semantics
                    .as_ref()
                    .map(|model| model.assets.clone())
                    .unwrap_or_default(),
                native_point_presence: Vec::new(),
            },
            asset_checks: checks(
                &projected
                    .picture_semantics
                    .as_ref()
                    .map(|model| model.assets.as_slice())
                    .unwrap_or_default(),
            )?,
            definition_glyphs: Vec::new(),
            definition_points: Vec::new(),
            native_point_presence: projected
                .picture_semantics
                .as_ref()
                .map(|m| m.native_point_presence.clone())
                .unwrap_or_default(),
        };
        let mut bytes = serde_json::to_vec_pretty(&resource).map_err(|e| error(e.to_string()))?;
        bytes.push(b'\n');
        Some(bytes)
    };
    let defaults = body.common_picture_transparency.clone();
    visit(&mut projected, &mut |binding, value| {
        if let Some(row) = record(binding, value)? {
            if row.reference.starts_with("abs-file:") {
                let glyph = fields::picture_glyph(value)?;
                let marker = match fields::picture_pixel(value) {
                    Some(point) => fields::picture_canon_px(String::new(), point),
                    None => fields::picture_canon(String::new(), false),
                };
                *value = fields::picture_with_glyph(marker, glyph)?;
                return Ok(());
            }
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

fn validate_assets(model: &FormPictureSemantics) -> Result<(), FormError> {
    let mut seen = HashSet::new();
    let mut paths = HashMap::new();
    for asset in &model.assets {
        validate_asset_path(&asset.path)?;
        if !matches!(
            asset.binding,
            PictureSemanticBinding::ChoiceParameter { .. } | PictureSemanticBinding::Chart { .. }
        ) {
            return Err(error("asset binding must be a typed definition"));
        }
        if !seen.insert(key(&asset.binding)?) {
            return Err(error("duplicate picture asset binding"));
        }
        if let Some(previous) = paths.insert(&asset.path, &asset.bytes) {
            if previous != &asset.bytes {
                return Err(error("conflicting bytes for one declared asset path"));
            }
        }
        let reference = format!("abs-file:{}", asset.path);
        if !model
            .records
            .iter()
            .any(|row| row.binding == asset.binding && row.reference == reference)
        {
            return Err(error("asset has no current definition binding"));
        }
    }
    for row in &model.records {
        if let Some(path) = row.reference.strip_prefix("abs-file:") {
            validate_asset_path(path)?;
            if let Some(point) = row.pixel {
                crate::md_picture::point(&crate::md_picture::present(Some((point.x, point.y))))
                    .map_err(error)?;
            }
            if !model
                .assets
                .iter()
                .any(|asset| asset.binding == row.binding && asset.path == path)
            {
                return Err(error("current definition has no owned asset bytes"));
            }
        }
    }
    let mut presence = HashSet::new();
    for binding in &model.native_point_presence {
        if !presence.insert(key(binding)?) || !model.assets.iter().any(|a| a.binding == *binding) {
            return Err(error("duplicate/unbound current native Point presence"));
        }
    }
    Ok(())
}
/// Resolve only declared native choice Abs files using the caller's strict regular-file reader.
pub fn attach_choice_picture_assets(
    body: &mut FormBody,
    form_uuid: Uuid,
    mut read: impl FnMut(&str) -> Result<Vec<u8>, FormError>,
) -> Result<(), FormError> {
    chart_definitions(body, &mut |_, path, bytes| {
        validate_asset_path(path)?;
        *bytes = read(path)?;
        Ok(())
    })?;
    let mut assets = Vec::new();
    let chart_presence = native_chart_point_presence(body)?;
    let mut native_point_presence = Vec::new();
    visit(body, &mut |binding, value| {
        let (reference, _) = fields::picture_ref_lt(value)?;
        if let Some(path) = reference.strip_prefix("abs-file:") {
            validate_asset_path(path)?;
            if fields::picture_pixel(value).is_some_and(omitted_point)
                && (!matches!(binding, PictureSemanticBinding::Chart { .. })
                    || chart_presence.contains(&key(&binding)?))
            {
                native_point_presence.push(binding.clone());
            }
            assets.push(PictureSemanticAsset {
                binding,
                path: path.into(),
                bytes: read(path)?,
            });
        }
        Ok(())
    })?;
    body.picture_semantics = Some(FormPictureSemantics {
        form_uuid,
        records: Vec::new(),
        assets,
        native_point_presence,
    });
    Ok(())
}
/// Validate and emit CURRENT definition bytes at their declared native paths.
pub fn choice_picture_assets(
    body: &FormBody,
    form_uuid: Uuid,
) -> Result<Vec<(String, Vec<u8>)>, FormError> {
    if body
        .picture_semantics
        .as_ref()
        .is_some_and(|model| model.form_uuid != form_uuid)
    {
        return Err(error("asset form UUID mismatch"));
    }
    let mut current = body.clone();
    bind_picture_semantics(&mut current, form_uuid, false)?;
    let Some(model) = current.picture_semantics.as_ref() else {
        return Ok(Vec::new());
    };
    validate_assets(model)?;
    let mut paths = HashSet::new();
    Ok(model
        .assets
        .iter()
        .filter(|asset| paths.insert(asset.path.clone()))
        .map(|asset| (asset.path.clone(), asset.bytes.clone()))
        .collect())
}

fn chart_identity(fields: &[(String, ChartValue)]) -> Result<String, FormError> {
    fn strip(values: &mut [(String, ChartValue)]) {
        for (_, value) in values {
            match value {
                ChartValue::Picture(_) => *value = ChartValue::Absent,
                ChartValue::Nested(fields) => strip(fields),
                ChartValue::Items(items) => {
                    for fields in items {
                        strip(fields);
                    }
                }
                _ => {}
            }
        }
    }
    let mut current = fields.to_vec();
    strip(&mut current);
    serde_json::to_string(&current).map_err(|e| error(e.to_string()))
}
fn chart_fields(
    fields: &mut [(String, ChartValue)],
    attribute: &[PictureChartAttributeIdentity],
    kind: &str,
    path: &mut Vec<PictureChartStep>,
    f: &mut impl FnMut(PictureSemanticBinding, &mut PropertyValue) -> Result<(), FormError>,
) -> Result<(), FormError> {
    let identity = chart_identity(fields)?;
    for (name, value) in fields {
        path.push(PictureChartStep::Field(name.clone()));
        match value {
            ChartValue::Picture(picture) => {
                let binding = PictureSemanticBinding::Chart {
                    attribute: attribute.to_vec(),
                    chart_kind: kind.into(),
                    path: path.clone(),
                    current_container_canonical: identity.clone(),
                };
                match picture.as_mut() {
                    ChartPicture::Reference {
                        reference,
                        load_transparent,
                    } => {
                        let mut current =
                            fields::picture_canon(reference.clone(), *load_transparent);
                        f(binding, &mut current)?;
                        if fields::picture_pixel(&current).is_some()
                            || fields::picture_glyph(&current)?.is_some()
                        {
                            return Err(error("ChartPicture Reference cannot contain Point/glyph"));
                        }
                        let (name, flag) = fields::picture_ref_lt(&current)?;
                        *reference = name.into();
                        *load_transparent = flag;
                    }
                    ChartPicture::Definition {
                        file_name,
                        transparent_pixel,
                        glyph,
                        ..
                    } => {
                        let reference = if file_name.is_empty() {
                            String::new()
                        } else {
                            format!("abs-file:{file_name}")
                        };
                        let current = match transparent_pixel {
                            Some(point) => fields::picture_canon_px(reference, *point),
                            None => fields::picture_canon(reference, false),
                        };
                        let mut current = fields::picture_with_glyph(current, *glyph)?;
                        f(binding, &mut current)?;
                        let (reference, flag) = fields::picture_ref_lt(&current)?;
                        if !reference.is_empty() && !reference.starts_with("abs-file:") {
                            return Err(error("chart definition became a reference"));
                        }
                        if flag != fields::picture_pixel(&current).is_some() {
                            return Err(error("chart definition LT must match nullable Point"));
                        }
                        *file_name = reference.strip_prefix("abs-file:").unwrap_or("").into();
                        *transparent_pixel = fields::picture_pixel(&current);
                        *glyph = fields::picture_glyph(&current)?;
                    }
                }
            }
            ChartValue::Nested(fields) => chart_fields(fields, attribute, kind, path, f)?,
            ChartValue::Items(items) => {
                for (index, fields) in items.iter_mut().enumerate() {
                    path.push(PictureChartStep::Item(index));
                    chart_fields(fields, attribute, kind, path, f)?;
                    path.pop();
                }
            }
            _ => {}
        }
        path.pop();
    }
    Ok(())
}
fn chart_attributes(
    attrs: &mut [FormDataAttribute],
    path: &mut Vec<PictureChartAttributeIdentity>,
    edge: &dyn Fn(&FormDataAttribute) -> PictureChartAttributeIdentity,
    f: &mut impl FnMut(PictureSemanticBinding, &mut PropertyValue) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for attr in attrs {
        path.push(edge(attr));
        if let Some(chart) = &mut attr.chart_settings {
            if !["Chart", "GanttChart"].contains(&chart.kind.as_str()) {
                return Err(error("unknown typed chart kind"));
            }
            chart_fields(&mut chart.fields, path, &chart.kind, &mut Vec::new(), f)?;
        }
        chart_attributes(
            &mut attr.columns,
            path,
            &|a| PictureChartAttributeIdentity::Column {
                name: a.name.clone(),
                id: a.id,
            },
            f,
        )?;
        for (index, group) in attr.additional_columns.iter_mut().enumerate() {
            let table = group.table_path.primary();
            chart_attributes(
                &mut group.columns,
                path,
                &|a| PictureChartAttributeIdentity::AdditionalColumn {
                    table_path: table.clone(),
                    group: index,
                    name: a.name.clone(),
                    id: a.id,
                },
                f,
            )?;
        }
        path.pop();
    }
    Ok(())
}

/// Native bitmap wrappers omit Glyph; this explicit resource transports CURRENT
/// glyph only after binding all current filename/byte/Point values.
pub fn write_native_picture_resource(
    body: &FormBody,
    id: Uuid,
) -> Result<Option<Vec<u8>>, FormError> {
    let mut current = body.clone();
    bind_picture_semantics(&mut current, id, false)?;
    let presence = current
        .picture_semantics
        .as_ref()
        .map(|m| m.native_point_presence.clone())
        .unwrap_or_default();
    let mut glyphs = Vec::new();
    let mut points = Vec::new();
    visit(&mut current, &mut |binding, value| {
        if let Some((x, y)) = fields::picture_glyph(value)? {
            glyphs.push(GlyphRecord {
                binding: binding.clone(),
                glyph: PictureSemanticPixel { x, y },
            });
        }
        if let Some((x, y)) = fields::picture_pixel(value).filter(|p| omitted_point(*p)) {
            if point_transport(&binding, (x, y))
                && !presence.contains(&binding)
                && fields::picture_ref_lt(value)?.0.starts_with("abs-file:")
            {
                points.push(PointRecord {
                    binding,
                    point: PictureSemanticPixel { x, y },
                });
            }
        }
        Ok(())
    })?;
    if glyphs.is_empty() && points.is_empty() {
        return Ok(None);
    }
    let form = current
        .picture_semantics
        .ok_or_else(|| error("definition has no owned asset"))?;
    let resource = Resource {
        schema: SCHEMA.into(),
        version: 1,
        asset_checks: checks(&form.assets)?,
        native_point_presence: presence,
        form,
        definition_glyphs: glyphs,
        definition_points: points,
    };
    let mut bytes = serde_json::to_vec_pretty(&resource).map_err(|e| error(e.to_string()))?;
    bytes.push(b'\n');
    Ok(Some(bytes))
}
pub fn apply_native_picture_resource(
    body: &mut FormBody,
    id: Uuid,
    bytes: &[u8],
) -> Result<(), FormError> {
    let resource = decode(bytes)?;
    if resource.form.form_uuid != id
        || (resource.definition_glyphs.is_empty() && resource.definition_points.is_empty())
    {
        return Err(error("wrong form/native definition carrier role"));
    }
    let mut next = body.clone();
    bind_picture_semantics(&mut next, id, false)?;
    let mut expected = resource.form.clone();
    for point in &resource.definition_points {
        let row = expected
            .records
            .iter_mut()
            .find(|r| r.binding == point.binding)
            .ok_or_else(|| error("unknown Point binding"))?;
        row.pixel = if matches!(point.binding, PictureSemanticBinding::Chart { .. }) {
            Some(PictureSemanticPixel { x: -1, y: -1 })
        } else {
            None
        };
    }
    let current = next
        .picture_semantics
        .as_ref()
        .ok_or_else(|| error("native resource has no current pictures"))?;
    if current != &expected {
        return Err(error(
            "native resource filename/bytes/Ref/LT/Point differs from CURRENT source",
        ));
    }
    let mut glyphs = resource
        .definition_glyphs
        .into_iter()
        .map(|row| Ok((key(&row.binding)?, row)))
        .collect::<Result<HashMap<_, _>, FormError>>()?;
    let mut points = resource
        .definition_points
        .into_iter()
        .map(|row| Ok((key(&row.binding)?, row)))
        .collect::<Result<HashMap<_, _>, FormError>>()?;
    visit(&mut next, &mut |binding, value| {
        let address = key(&binding)?;
        if let Some(row) = points.remove(&address) {
            let (reference, _) = fields::picture_ref_lt(value)?;
            *value = fields::picture_canon_px(reference.into(), (row.point.x, row.point.y));
        }
        if let Some(row) = glyphs.remove(&address) {
            if fields::picture_glyph(value)?.is_some() {
                return Err(error("duplicate glyph carrier"));
            }
            *value = fields::picture_with_glyph(value.clone(), Some((row.glyph.x, row.glyph.y)))?;
        }
        Ok(())
    })?;
    if !glyphs.is_empty() || !points.is_empty() {
        return Err(error("stale native definition binding"));
    }
    if let Some(model) = &mut next.picture_semantics {
        model.native_point_presence = resource.native_point_presence;
    }
    bind_picture_semantics(&mut next, id, false)?;
    *body = next;
    Ok(())
}

fn chart_definitions(
    body: &mut FormBody,
    f: &mut impl FnMut(PictureSemanticBinding, &mut String, &mut Vec<u8>) -> Result<(), FormError>,
) -> Result<(), FormError> {
    fn values(
        fields: &mut [(String, ChartValue)],
        attrs: &[PictureChartAttributeIdentity],
        kind: &str,
        path: &mut Vec<PictureChartStep>,
        f: &mut impl FnMut(PictureSemanticBinding, &mut String, &mut Vec<u8>) -> Result<(), FormError>,
    ) -> Result<(), FormError> {
        let identity = chart_identity(fields)?;
        for (name, value) in fields {
            path.push(PictureChartStep::Field(name.clone()));
            match value {
                ChartValue::Picture(picture) => {
                    if let ChartPicture::Definition {
                        file_name, bytes, ..
                    } = picture.as_mut()
                    {
                        f(
                            PictureSemanticBinding::Chart {
                                attribute: attrs.to_vec(),
                                chart_kind: kind.into(),
                                path: path.clone(),
                                current_container_canonical: identity.clone(),
                            },
                            file_name,
                            bytes,
                        )?;
                    }
                }
                ChartValue::Nested(fields) => values(fields, attrs, kind, path, f)?,
                ChartValue::Items(items) => {
                    for (index, fields) in items.iter_mut().enumerate() {
                        path.push(PictureChartStep::Item(index));
                        values(fields, attrs, kind, path, f)?;
                        path.pop();
                    }
                }
                _ => {}
            }
            path.pop();
        }
        Ok(())
    }
    fn attrs(
        items: &mut [FormDataAttribute],
        path: &mut Vec<PictureChartAttributeIdentity>,
        edge: &dyn Fn(&FormDataAttribute) -> PictureChartAttributeIdentity,
        f: &mut impl FnMut(PictureSemanticBinding, &mut String, &mut Vec<u8>) -> Result<(), FormError>,
    ) -> Result<(), FormError> {
        for attr in items {
            path.push(edge(attr));
            if let Some(chart) = &mut attr.chart_settings {
                values(&mut chart.fields, path, &chart.kind, &mut Vec::new(), f)?;
            }
            attrs(
                &mut attr.columns,
                path,
                &|a| PictureChartAttributeIdentity::Column {
                    name: a.name.clone(),
                    id: a.id,
                },
                f,
            )?;
            for (index, group) in attr.additional_columns.iter_mut().enumerate() {
                let table = group.table_path.primary();
                attrs(
                    &mut group.columns,
                    path,
                    &|a| PictureChartAttributeIdentity::AdditionalColumn {
                        table_path: table.clone(),
                        group: index,
                        name: a.name.clone(),
                        id: a.id,
                    },
                    f,
                )?;
            }
            path.pop();
        }
        Ok(())
    }
    attrs(
        &mut body.data_attributes,
        &mut Vec::new(),
        &|a| PictureChartAttributeIdentity::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        f,
    )
}
/// The native descriptor cannot store Glyph. Clear only that selected current
/// value in the projection; write_native_picture_resource carries the original.
pub fn project_native_picture_glyphs(body: &FormBody) -> Result<Option<FormBody>, FormError> {
    let mut current = body.clone();
    let mut changed = false;
    let presence = body
        .picture_semantics
        .as_ref()
        .map(|m| m.native_point_presence.clone())
        .unwrap_or_default();
    visit(&mut current, &mut |binding, value| {
        let (reference, flag) = fields::picture_ref_lt(value)?;
        let reference = reference.to_owned();
        let pixel = fields::picture_pixel(value);
        let glyph = fields::picture_glyph(value)?;
        let omit = reference.starts_with("abs-file:")
            && pixel.is_some_and(omitted_point)
            && !presence.contains(&binding);
        if glyph.is_some() || omit {
            *value = if omit {
                if matches!(binding, PictureSemanticBinding::Chart { .. }) {
                    fields::picture_canon_px(reference, (-1, -1))
                } else {
                    fields::picture_canon(reference, true)
                }
            } else {
                match pixel {
                    Some(p) => fields::picture_canon_px(reference, p),
                    None => fields::picture_canon(reference, flag),
                }
            };
            changed = true;
        }
        Ok(())
    })?;
    changed |= native_presence_layout(&mut current, &presence)?;
    Ok(changed.then_some(current))
}

fn native_chart_point_presence(body: &FormBody) -> Result<HashSet<String>, FormError> {
    fn attrs(
        items: &[FormDataAttribute],
        path: &mut Vec<PictureChartAttributeIdentity>,
        edge: &dyn Fn(&FormDataAttribute) -> PictureChartAttributeIdentity,
        out: &mut HashSet<String>,
    ) -> Result<(), FormError> {
        for attr in items {
            path.push(edge(attr));
            if let Some(chart) = &attr.chart_settings {
                if let Some(layout) = chart
                    .source_layout
                    .as_ref()
                    .filter(|l| l.format == ChartSourceFormat::Designer)
                {
                    let mut fields = chart.fields.clone();
                    chart_fields(
                        &mut fields,
                        path,
                        &chart.kind,
                        &mut Vec::new(),
                        &mut |binding, _| {
                            if let PictureSemanticBinding::Chart { path, .. } = &binding {
                                let lexical = path
                                    .iter()
                                    .map(|p| match p {
                                        PictureChartStep::Field(n) => {
                                            ChartLayoutSegment::Field(n.clone())
                                        }
                                        PictureChartStep::Item(i) => ChartLayoutSegment::Item(*i),
                                    })
                                    .collect::<Vec<_>>();
                                if layout.composites.iter().any(|c| {
                                    c.path == lexical
                                        && c.fields.iter().any(|n| n == "TransparentPixel")
                                }) {
                                    out.insert(key(&binding)?);
                                }
                            }
                            Ok(())
                        },
                    )?;
                }
            }
            attrs(
                &attr.columns,
                path,
                &|a| PictureChartAttributeIdentity::Column {
                    name: a.name.clone(),
                    id: a.id,
                },
                out,
            )?;
            for (index, group) in attr.additional_columns.iter().enumerate() {
                let table = group.table_path.primary();
                attrs(
                    &group.columns,
                    path,
                    &|a| PictureChartAttributeIdentity::AdditionalColumn {
                        table_path: table.clone(),
                        group: index,
                        name: a.name.clone(),
                        id: a.id,
                    },
                    out,
                )?;
            }
            path.pop();
        }
        Ok(())
    }
    let mut out = HashSet::new();
    attrs(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| PictureChartAttributeIdentity::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut out,
    )?;
    Ok(out)
}

fn native_presence_layout(
    body: &mut FormBody,
    presence: &[PictureSemanticBinding],
) -> Result<bool, FormError> {
    fn attrs(
        items: &mut [FormDataAttribute],
        path: &mut Vec<PictureChartAttributeIdentity>,
        edge: &dyn Fn(&FormDataAttribute) -> PictureChartAttributeIdentity,
        presence: &[PictureSemanticBinding],
    ) -> Result<bool, FormError> {
        let mut changed = false;
        for attr in items {
            path.push(edge(attr));
            if let Some(chart) = &mut attr.chart_settings {
                let mut locations = Vec::new();
                let mut fields = chart.fields.clone();
                chart_fields(
                    &mut fields,
                    path,
                    &chart.kind,
                    &mut Vec::new(),
                    &mut |binding, value| {
                        if presence.contains(&binding)
                            && fields::picture_pixel(value) == Some((-1, -1))
                        {
                            if let PictureSemanticBinding::Chart { path, .. } = binding {
                                locations.push(
                                    path.into_iter()
                                        .map(|p| match p {
                                            PictureChartStep::Field(n) => {
                                                ChartLayoutSegment::Field(n)
                                            }
                                            PictureChartStep::Item(i) => {
                                                ChartLayoutSegment::Item(i)
                                            }
                                        })
                                        .collect::<Vec<_>>(),
                                );
                            }
                        }
                        Ok(())
                    },
                )?;
                if !locations.is_empty() {
                    if chart
                        .source_layout
                        .as_ref()
                        .is_none_or(|l| l.format != ChartSourceFormat::Designer)
                    {
                        chart.source_layout = Some(ChartSourceLayout {
                            format: ChartSourceFormat::Designer,
                            composites: Vec::new(),
                            root_namespaces: Vec::new(),
                            common_inline_paths: Vec::new(),
                        });
                    }
                    let layout = chart.source_layout.as_mut().expect("layout initialized");
                    for location in locations {
                        if let Some(row) = layout.composites.iter_mut().find(|c| c.path == location)
                        {
                            if !row.fields.iter().any(|n| n == "TransparentPixel") {
                                row.fields.push("TransparentPixel".into());
                            }
                        } else {
                            layout.composites.push(ChartCompositeLayout {
                                path: location,
                                fields: vec!["TransparentPixel".into()],
                            });
                        }
                    }
                    changed = true;
                }
            }
            changed |= attrs(
                &mut attr.columns,
                path,
                &|a| PictureChartAttributeIdentity::Column {
                    name: a.name.clone(),
                    id: a.id,
                },
                presence,
            )?;
            for (index, group) in attr.additional_columns.iter_mut().enumerate() {
                let table = group.table_path.primary();
                changed |= attrs(
                    &mut group.columns,
                    path,
                    &|a| PictureChartAttributeIdentity::AdditionalColumn {
                        table_path: table.clone(),
                        group: index,
                        name: a.name.clone(),
                        id: a.id,
                    },
                    presence,
                )?;
            }
            path.pop();
        }
        Ok(changed)
    }
    attrs(
        &mut body.data_attributes,
        &mut Vec::new(),
        &|a| PictureChartAttributeIdentity::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        presence,
    )
}
