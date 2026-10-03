//! Closed semantic transport for valid absent FieldExtInfo native projections.
//! Top-level bindings name CURRENT native events and never store their handlers.
//! An independently typed extension subtree absent from the native projection
//! owns its complete CURRENT values (including nested handlers) exactly once.
use super::FormError;
use morph1c_core::ir::form::{DecoratorBody, FieldEventOwners, NativeFormEventOrder};
use morph1c_core::ir::{FormBody, FormItem, Uuid};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const EVENT_SEMANTICS_RESOURCE: &str = "ibcmd-form-event-semantics.v1.json";
const SCHEMA: &str = "urn:ibcmd:source-extension:form-event-semantics:1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Edge {
    Child,
    Addition,
    AutoTable,
    AutoCommandBar,
    ContextMenu,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    edge: Edge,
    kind: String,
    id: i64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionPayload {
    properties: Vec<(morph1c_core::ir::FieldId, morph1c_core::ir::PropertyValue)>,
    font: Option<morph1c_core::ir::form::FontRef>,
    auto_table: Option<Box<FormItem>>,
    additions: Vec<FormItem>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    path: Vec<Identity>,
    owner: FieldEventOwners,
    field_type_none: bool,
    actual_extension: Option<String>,
    extension_payload: Option<ExtensionPayload>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    schema: String,
    version: u32,
    form_uuid: Uuid,
    records: Vec<Record>,
}
fn error(reason: impl Into<String>) -> FormError {
    FormError::Frame(format!("event semantics: {}", reason.into()))
}
fn key(path: &[Identity]) -> Result<String, FormError> {
    serde_json::to_string(path).map_err(|e| error(e.to_string()))
}
fn decode(bytes: &[u8]) -> Result<Resource, FormError> {
    let value: Resource = super::strict_resource::parse(bytes).map_err(|e| error(e.to_string()))?;
    if value.schema != SCHEMA || value.version != 1 || value.records.is_empty() {
        return Err(error("unknown/empty schema/version"));
    }
    let mut seen = HashSet::new();
    for row in &value.records {
        if row.path.is_empty()
            || (row.owner.extension_attached
                && !row.field_type_none
                && row.actual_extension.is_none())
            || !seen.insert(key(&row.path)?)
        {
            return Err(error("duplicate/unknown event owner topology"));
        }
        let tail = row.path.last().unwrap();
        let kind = super::tables::FIELD_KINDS
            .iter()
            .find(|kind| kind.ext_xsi == row.owner.extension_kind)
            .ok_or_else(|| error("resource owner is not a known field extension"))?;
        if super::tables::field_kind(&tail.kind).is_none() {
            return Err(error("resource field extension kind mismatch"));
        }
        if row.actual_extension.as_deref()
            != row
                .extension_payload
                .as_ref()
                .map(|_| row.owner.extension_kind.as_str())
        {
            return Err(error("actual extension class/payload binding mismatch"));
        }
        if let Some(payload) = &row.extension_payload {
            validate_payload(kind, payload)?;
        }
        if row.owner.owners.iter().any(|(name, owner)| {
            name.is_empty()
                || (!row.owner.extension_attached
                    && *owner != morph1c_core::ir::form::FieldEventOwner::Body)
        }) {
            return Err(error("absent extension requires current body ownership"));
        }
    }
    Ok(value)
}
fn items(
    items: &mut [FormItem],
    parent: &[Identity],
    edge: Edge,
    f: &mut impl FnMut(&[Identity], &mut FormItem) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for item in items {
        let mut path = parent.to_vec();
        path.push(Identity {
            edge: edge.clone(),
            kind: item.kind.as_str().into(),
            id: item.id,
        });
        f(&path, item)?;
        self::items(&mut item.children, &path, Edge::Child, f)?;
        self::items(&mut item.additions, &path, Edge::Addition, f)?;
        if let Some(table) = &mut item.auto_table {
            self::items(
                std::slice::from_mut(table.as_mut()),
                &path,
                Edge::AutoTable,
                f,
            )?;
        }
        if let Some(bar) = &mut item.auto_command_bar {
            let mut p = path.clone();
            p.push(Identity {
                edge: Edge::AutoCommandBar,
                kind: "AutoCommandBar".into(),
                id: bar.id,
            });
            self::items(&mut bar.items, &p, Edge::Child, f)?;
        }
        if let Some(menu) = &mut item.context_menu {
            if let DecoratorBody::ContextMenu(body) = &mut menu.body {
                let mut p = path.clone();
                p.push(Identity {
                    edge: Edge::ContextMenu,
                    kind: "ContextMenu".into(),
                    id: menu.id,
                });
                self::items(&mut body.items, &p, Edge::Child, f)?;
            }
        }
    }
    Ok(())
}
fn visit(
    body: &mut FormBody,
    f: &mut impl FnMut(&[Identity], &mut FormItem) -> Result<(), FormError>,
) -> Result<(), FormError> {
    items(&mut body.items, &[], Edge::Child, f)?;
    if let Some(bar) = &mut body.auto_command_bar {
        items(
            &mut bar.items,
            &[Identity {
                edge: Edge::AutoCommandBar,
                kind: "AutoCommandBar".into(),
                id: bar.id,
            }],
            Edge::Child,
            f,
        )?;
    }
    Ok(())
}
/// Emit only current nondefault semantic topology. No cached resource values.
pub fn write_event_semantics_resource(
    body: &FormBody,
    form_uuid: Uuid,
) -> Result<Option<Vec<u8>>, FormError> {
    // Reuse the typed walker through a shallow semantic-only item projection.
    fn collect(
        items: &[FormItem],
        parent: &[Identity],
        edge: Edge,
        rows: &mut Vec<Record>,
        seen: &mut HashSet<String>,
    ) -> Result<(), FormError> {
        for item in items {
            let mut path = parent.to_vec();
            path.push(Identity {
                edge: edge.clone(),
                kind: item.kind.as_str().into(),
                id: item.id,
            });
            if super::tables::field_kind(item.kind.as_str()).is_some() {
                super::event_owners::field_owned_events(item)?;
                if item.field_extension_kind.is_some()
                    || item.field_type_none
                    || item
                        .field_event_owners
                        .as_ref()
                        .is_some_and(|owner| !owner.extension_attached)
                {
                    if !seen.insert(key(&path)?) {
                        return Err(error("ambiguous current field identity"));
                    }
                    let (body, extension, attached) =
                        super::event_owners::field_owned_events(item)?;
                    let owners = body
                        .into_iter()
                        .map(|e| {
                            (
                                e.name.clone(),
                                morph1c_core::ir::form::FieldEventOwner::Body,
                            )
                        })
                        .chain(extension.into_iter().map(|e| {
                            (
                                e.name.clone(),
                                morph1c_core::ir::form::FieldEventOwner::Extension,
                            )
                        }))
                        .collect();
                    let kind = super::event_owners::field_extension_kind(item)?;
                    let extension_payload = if item.field_extension_kind.is_some() {
                        let payload = ExtensionPayload {
                            properties: item.ext_info.clone(),
                            font: item.font.clone(),
                            auto_table: item.auto_table.clone(),
                            additions: item.additions.clone(),
                        };
                        validate_payload(kind, &payload)?;
                        Some(payload)
                    } else {
                        None
                    };
                    rows.push(Record {
                        path: path.clone(),
                        owner: FieldEventOwners {
                            extension_kind: kind.ext_xsi.into(),
                            extension_attached: attached,
                            owners,
                        },
                        field_type_none: item.field_type_none,
                        actual_extension: item.field_extension_kind.clone(),
                        extension_payload,
                    });
                }
            }
            collect(&item.children, &path, Edge::Child, rows, seen)?;
            // The transported extension payload already owns these complete
            // CURRENT typed subtrees; they have no separate native projection.
            if item.field_extension_kind.is_none() {
                collect(&item.additions, &path, Edge::Addition, rows, seen)?;
            }
            if let Some(table) = &item
                .auto_table
                .as_ref()
                .filter(|_| item.field_extension_kind.is_none())
            {
                collect(
                    std::slice::from_ref(table.as_ref()),
                    &path,
                    Edge::AutoTable,
                    rows,
                    seen,
                )?;
            }
            if let Some(bar) = &item.auto_command_bar {
                let mut p = path.clone();
                p.push(Identity {
                    edge: Edge::AutoCommandBar,
                    kind: "AutoCommandBar".into(),
                    id: bar.id,
                });
                collect(&bar.items, &p, Edge::Child, rows, seen)?;
            }
            if let Some(menu) = &item.context_menu {
                if let DecoratorBody::ContextMenu(body) = &menu.body {
                    let mut p = path.clone();
                    p.push(Identity {
                        edge: Edge::ContextMenu,
                        kind: "ContextMenu".into(),
                        id: menu.id,
                    });
                    collect(&body.items, &p, Edge::Child, rows, seen)?;
                }
            }
        }
        Ok(())
    }
    let mut records = Vec::new();
    let mut seen = HashSet::new();
    collect(&body.items, &[], Edge::Child, &mut records, &mut seen)?;
    if let Some(bar) = &body.auto_command_bar {
        collect(
            &bar.items,
            &[Identity {
                edge: Edge::AutoCommandBar,
                kind: "AutoCommandBar".into(),
                id: bar.id,
            }],
            Edge::Child,
            &mut records,
            &mut seen,
        )?;
    }
    if records.is_empty() {
        return Ok(None);
    }
    serde_json::to_vec_pretty(&Resource {
        schema: SCHEMA.into(),
        version: 1,
        form_uuid,
        records,
    })
    .map(Some)
    .map_err(|e| error(e.to_string()))
}
/// Validate all bindings against the fully decoded current native form before
/// changing any owner. Restore order by CURRENT handlers and native names only.
pub fn apply_event_semantics_resource(
    body: &mut FormBody,
    form_uuid: Uuid,
    bytes: &[u8],
) -> Result<(), FormError> {
    let value = decode(bytes)?;
    if value.form_uuid != form_uuid {
        return Err(error("declared form UUID differs"));
    }
    let mut pending: BTreeMap<_, _> = value
        .records
        .into_iter()
        .map(|row| key(&row.path).map(|key| (key, row)))
        .collect::<Result<_, _>>()?;
    let mut plans = HashMap::new();
    let mut seen = HashSet::new();
    visit(body, &mut |path, item| {
        let address = key(path)?;
        if let Some(record) = pending.get(&address) {
            let owner = &record.owner;
            if !seen.insert(address.clone()) {
                return Err(error("ambiguous current resource owner"));
            }
            if record.actual_extension.is_some() {
                let base = super::tables::field_kind(item.kind.as_str())
                    .ok_or_else(|| error("unknown current native base kind"))?;
                let mut defaults = Vec::new();
                super::fields::read_fields_edt(
                    base.kind,
                    None,
                    base.ext,
                    super::fields::Region::Ext,
                    &mut defaults,
                )?;
                if item.ext_info != defaults
                    || item.font.is_some()
                    || item.auto_table.is_some()
                    || !item.additions.is_empty()
                {
                    return Err(error(
                        "current native extension projection was edited; resource binding is stale",
                    ));
                }
            }
            let current: HashMap<_, _> = item
                .events
                .iter()
                .map(|event| (event.name.as_str(), event))
                .collect();
            if current.len() != item.events.len()
                || current.len() != owner.owners.len()
                || owner
                    .owners
                    .keys()
                    .any(|name| !current.contains_key(name.as_str()))
            {
                return Err(error(
                    "resource names differ from current complete event identities",
                ));
            }
            let ordered = if let Some(source) = &item.native_field_event_order {
                &source.merged
            } else {
                return Err(error("resource is only valid for a decoded native field"));
            };
            if ordered.len() != current.len() {
                return Err(error("current native event order is incomplete"));
            }
            let mut used = HashSet::new();
            let mut events = Vec::new();
            for name in ordered {
                if !used.insert(name.as_str()) {
                    return Err(error("duplicate current native event order"));
                }
                events.push(
                    (*current
                        .get(name.as_str())
                        .ok_or_else(|| error("unknown current native ordered event"))?)
                    .clone(),
                );
            }
            let mut candidate = FormItem::new(item.kind.clone(), item.name.clone(), item.id);
            candidate.events = events.clone();
            candidate.field_event_owners = Some(owner.clone());
            candidate.field_type_none = record.field_type_none;
            candidate.field_extension_kind = record.actual_extension.clone();
            super::event_owners::field_owned_events(&candidate)?;
            let merged = events.iter().map(|e| e.name.clone()).collect::<Vec<_>>();
            let (base, ext): (Vec<_>, Vec<_>) = events.into_iter().partition(|e| {
                owner.owners[&e.name] == morph1c_core::ir::form::FieldEventOwner::Body
            });
            let events = base.into_iter().chain(ext).collect();
            plans.insert(
                address,
                (
                    owner.clone(),
                    record.field_type_none,
                    record.actual_extension.clone(),
                    record.extension_payload.clone(),
                    events,
                    merged,
                ),
            );
        }
        Ok(())
    })?;
    for address in &seen {
        pending.remove(address);
    }
    if !pending.is_empty() {
        return Err(error("deleted/unknown event resource owner"));
    }
    visit(body, &mut |path, item| {
        if let Some((owner, none_type, actual_extension, extension_payload, events, merged)) =
            plans.remove(&key(path)?)
        {
            item.events = events;
            item.field_type_none = none_type;
            item.field_extension_kind = actual_extension;
            if let Some(payload) = extension_payload {
                item.ext_info = payload.properties;
                item.font = payload.font;
                item.auto_table = payload.auto_table;
                item.additions = payload.additions;
            }
            item.field_event_owners = if owner.extension_attached {
                None
            } else {
                Some(owner.clone())
            };
            item.native_field_event_order = Some(NativeFormEventOrder {
                extension_kind: Some(owner.extension_kind),
                merged,
                root: item
                    .events
                    .iter()
                    .filter(|e| {
                        owner.owners[&e.name] == morph1c_core::ir::form::FieldEventOwner::Body
                    })
                    .map(|e| e.name.clone())
                    .collect(),
                extension: item
                    .events
                    .iter()
                    .filter(|e| {
                        owner.owners[&e.name] == morph1c_core::ir::form::FieldEventOwner::Extension
                    })
                    .map(|e| e.name.clone())
                    .collect(),
            });
        }
        Ok(())
    })?;
    Ok(())
}
pub fn same_event_semantics_resource(a: &[u8], b: &[u8]) -> bool {
    matches!((decode(a),decode(b)),(Ok(a),Ok(b)) if a==b)
}
pub fn event_semantics_resource_count(body: &FormBody) -> Result<Option<usize>, FormError> {
    write_event_semantics_resource(body, Uuid([0; 16]))?
        .map(|bytes| decode(&bytes).map(|value| value.records.len()))
        .transpose()
}

fn validate_payload(
    kind: &super::tables::FieldKind,
    payload: &ExtensionPayload,
) -> Result<(), FormError> {
    let mut seen = HashSet::new();
    for (id, _) in &payload.properties {
        if !seen.insert(*id) || !kind.ext.iter().any(|slot| slot.id == *id) {
            return Err(error("unknown/duplicate actual extension property"));
        }
    }
    let mut candidate = FormItem::new(
        morph1c_core::ir::FormControlKind::new(kind.kind),
        "Extension",
        1,
    );
    candidate.ext_info = payload.properties.clone();
    candidate.font = payload.font.clone();
    candidate.auto_table = payload.auto_table.clone();
    candidate.additions = payload.additions.clone();
    let mut form = FormBody::new();
    form.items.push(candidate);
    use morph1c_core::version::{
        FormatVersion, current_roundtrip_target, current_source_version, with_roundtrip_target,
        with_source_version,
    };
    let explicit = current_roundtrip_target().or(current_source_version());
    let versions = explicit.map_or_else(
        || vec![FormatVersion::new(2, 20), FormatVersion::new(2, 21)],
        |version| vec![version],
    );
    let expected = serde_json::to_vec(payload).map_err(|e| error(e.to_string()))?;
    for version in versions {
        let Ok(bytes) = with_roundtrip_target(version, || {
            super::write_form(super::FormDialect::Edt, &form)
        }) else {
            continue;
        };
        let Ok(returned) = with_source_version(Some(version), || {
            super::read_form(super::FormDialect::Edt, &bytes)
        }) else {
            continue;
        };
        let current = &returned.items[0];
        let decoded = ExtensionPayload {
            properties: current.ext_info.clone(),
            font: current.font.clone(),
            auto_table: current.auto_table.clone(),
            additions: current.additions.clone(),
        };
        if serde_json::to_vec(&decoded).map_err(|e| error(e.to_string()))? == expected {
            return Ok(());
        }
    }
    Err(error(
        "extension payload does not fully match its typed codec",
    ))
}
