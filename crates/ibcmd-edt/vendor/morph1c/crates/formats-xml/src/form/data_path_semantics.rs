//! CURRENT ordered DataPath transport for the original serializers' noninjective
//! projections. Values are regenerated from the live IR; no source-value cache.
use super::{FormDialect, FormError, FormProjectionContext};
use morph1c_core::ir::{
    form::{DataPathSpec, DecoratorBody},
    FieldId, FormBody, FormDataAttribute, FormItem, PropertyValue, Uuid,
};
use morph1c_core::version::{with_roundtrip_target, FormatVersion};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const DATA_PATH_SEMANTICS_RESOURCE: &str = "ibcmd-form-data-path-semantics.v1.json";
const SCHEMA: &str = "urn:ibcmd:source-extension:form-data-path-semantics:1";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity {
    edge: Edge,
    kind: String,
    id: i64,
    name: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Edge {
    Root,
    Child,
    Addition,
    AutoTable,
    AutoCommandBar,
    ContextMenu,
    Tooltip,
    Attribute,
    Column,
    AdditionalColumn,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Role {
    Field {
        extension: bool,
        field: FieldId,
        nested: Option<Nested>,
    },
    UseAlways(usize),
    Save(usize),
    TablePath(usize),
    Command {
        panel: Panel,
        ordinal: usize,
        command: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Nested {
    TypeLink,
    ChoiceLink { ordinal: usize, name: String },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum Panel {
    Navigation,
    CommandBar,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    owner: Vec<Identity>,
    role: Role,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    binding: Binding,
    projected_sha256: String,
    current: DataPathSpec,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resource {
    schema: String,
    version: u32,
    form_uuid: Uuid,
    native: bool,
    profile: (u16, u16),
    records: Vec<Record>,
}
fn error(s: impl Into<String>) -> FormError {
    FormError::Frame(format!("DataPath semantics: {}", s.into()))
}
fn key(binding: &Binding) -> Result<String, FormError> {
    serde_json::to_string(binding).map_err(|e| error(e.to_string()))
}
fn digest(binding: &Binding, path: &DataPathSpec) -> Result<String, FormError> {
    let bytes = serde_json::to_vec(&(binding, path)).map_err(|e| error(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
fn validate(path: &DataPathSpec) -> Result<(), FormError> {
    if path.segments.is_empty() {
        return Err(error("segments lower bound is one"));
    }
    Ok(())
}
fn projection(
    binding: &Binding,
    path: &DataPathSpec,
    native: bool,
) -> Result<DataPathSpec, FormError> {
    validate(path)?;
    if native {
        if matches!(binding.role, Role::TablePath(_)) {
            return super::data_path::current(&super::data_path::read_native(&path.primary())?);
        }
        let text = super::data_path::write_native(&PropertyValue::DataPath(path.clone()))?;
        let field = matches!(binding.role, Role::Field { nested: None, .. })
            && binding
                .owner
                .last()
                .is_some_and(|id| super::tables::field_kind(&id.kind).is_some());
        let value = if field {
            super::data_path::read_native_field(&text)?
        } else {
            super::data_path::read_native(&text)?
        };
        super::data_path::current(&value)
    } else {
        Ok(DataPathSpec::from_form_text(&path.primary()))
    }
}
fn compatible_projection(
    binding: &Binding,
    current: &DataPathSpec,
    projected: &DataPathSpec,
    native: bool,
) -> Result<bool, FormError> {
    if !native {
        return Ok(DataPathSpec::from_form_text(&current.primary()) == *projected);
    }
    let field = matches!(binding.role, Role::Field { nested: None, .. })
        && binding
            .owner
            .last()
            .is_some_and(|id| super::tables::field_kind(&id.kind).is_some());
    let decode = |text: &str| {
        let value = if field {
            super::data_path::read_native_field(text)
        } else {
            super::data_path::read_native(text)
        };
        value.and_then(|value| super::data_path::current(&value))
    };
    let primary = current.primary();
    if decode(&primary).is_ok_and(|p| p == *projected) {
        return Ok(true);
    }
    if matches!(binding.role, Role::TablePath(_)) {
        return Ok(false);
    }
    let mut unresolved = format!("~{primary}");
    for extra in &current.extra_paths {
        unresolved.push('~');
        unresolved.push_str(extra);
    }
    Ok(decode(&unresolved).is_ok_and(|p| p == *projected))
}
fn property(
    owner: &[Identity],
    extension: bool,
    field: FieldId,
    codec: super::fields::Codec,
    value: &mut PropertyValue,
    f: &mut impl FnMut(&Binding, &mut DataPathSpec) -> Result<(), FormError>,
) -> Result<(), FormError> {
    use super::fields::Codec;
    let mut one = |value: &mut PropertyValue, nested| {
        let mut path = super::data_path::current(value)?;
        f(
            &Binding {
                owner: owner.to_vec(),
                role: Role::Field {
                    extension,
                    field,
                    nested,
                },
            },
            &mut path,
        )?;
        *value = super::data_path::canonical(path);
        Ok(())
    };
    match codec {
        Codec::DataPath => one(value, None),
        Codec::TypeLink => {
            super::fields::type_link_parts(value)?;
            let PropertyValue::List(values) = value else {
                unreachable!()
            };
            one(&mut values[0], Some(Nested::TypeLink))
        }
        Codec::ChoiceParameterLinks => {
            let PropertyValue::List(values) = value else {
                return Err(error("choice link list shape"));
            };
            for (ordinal, value) in values.iter_mut().enumerate() {
                let (name, _, _) = super::fields::choice_parameter_link_parts(value)?;
                let name = name.to_owned();
                let PropertyValue::List(parts) = value else {
                    unreachable!()
                };
                one(&mut parts[1], Some(Nested::ChoiceLink { ordinal, name }))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn bag(
    owner: &[Identity],
    extension: bool,
    fields: &[super::fields::FieldProj],
    values: &mut [(FieldId, PropertyValue)],
    f: &mut impl FnMut(&Binding, &mut DataPathSpec) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for (id, value) in values {
        if let Some(field) = fields.iter().find(|field| {
            field.id == *id && (field.region == super::fields::Region::Ext) == extension
        }) {
            property(owner, extension, *id, field.codec, value, f)?;
        }
    }
    Ok(())
}
fn controls(
    items: &mut [FormItem],
    parent: &[Identity],
    edge: Edge,
    native: bool,
    f: &mut impl FnMut(&Binding, &mut DataPathSpec) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for item in items {
        let mut owner = parent.to_vec();
        owner.push(Identity {
            edge,
            kind: item.kind.as_str().into(),
            id: item.id,
            name: item.name.clone(),
        });
        if let Some(kind) = super::tables::field_kind(item.kind.as_str()) {
            bag(
                &owner,
                false,
                super::tables::FORM_FIELD_COMMON,
                &mut item.properties,
                f,
            )?;
            // An independent actual FieldExtInfo payload is already CURRENT typed
            // event transport in native; do not bind a second record to an absent
            // native subtree. EDT's actual subtree still needs transient paths.
            if !(native && item.field_extension_kind.is_some()) {
                let actual = item
                    .field_extension_kind
                    .as_ref()
                    .and_then(|name| {
                        super::tables::FIELD_KINDS
                            .iter()
                            .find(|k| k.ext_xsi == name.as_str())
                    })
                    .unwrap_or(kind);
                bag(&owner, true, actual.ext, &mut item.ext_info, f)?;
            }
        } else if let Some(kind) = super::tables::group_kind(item.kind.as_str()) {
            bag(
                &owner,
                false,
                super::tables::FORM_GROUP_BODY,
                &mut item.properties,
                f,
            )?;
            bag(&owner, true, kind.ext, &mut item.ext_info, f)?;
        } else if let Some(kind) = super::tables::decoration_kind(item.kind.as_str()) {
            bag(
                &owner,
                false,
                super::tables::DECORATION_BODY,
                &mut item.properties,
                f,
            )?;
            bag(&owner, true, kind.ext, &mut item.ext_info, f)?;
        } else {
            let fields = match item.kind.as_str() {
                "Button" => super::tables::BUTTON_BODY,
                "Table" => super::tables::TABLE_BODY,
                _ => &[],
            };
            bag(&owner, false, fields, &mut item.properties, f)?;
        }
        controls(&mut item.children, &owner, Edge::Child, native, f)?;
        if !(native && item.field_extension_kind.is_some()) {
            controls(&mut item.additions, &owner, Edge::Addition, native, f)?;
            if let Some(table) = &mut item.auto_table {
                controls(
                    std::slice::from_mut(table.as_mut()),
                    &owner,
                    Edge::AutoTable,
                    native,
                    f,
                )?;
            }
        }
        if let Some(bar) = &mut item.auto_command_bar {
            let mut path = owner.clone();
            path.push(Identity {
                edge: Edge::AutoCommandBar,
                kind: "AutoCommandBar".into(),
                id: bar.id,
                name: bar.name.clone(),
            });
            controls(&mut bar.items, &path, Edge::Child, native, f)?;
        }
        if let Some(menu) = &mut item.context_menu {
            if let DecoratorBody::ContextMenu(body) = &mut menu.body {
                let mut path = owner.clone();
                path.push(Identity {
                    edge: Edge::ContextMenu,
                    kind: "ContextMenu".into(),
                    id: menu.id,
                    name: menu.name.clone(),
                });
                controls(&mut body.items, &path, Edge::Child, native, f)?;
            }
        }
        if let Some(tooltip) = &mut item.ext_tooltip {
            if let DecoratorBody::Tooltip(body) = &mut tooltip.body {
                let mut path = owner.clone();
                path.push(Identity {
                    edge: Edge::Tooltip,
                    kind: "ExtendedTooltip".into(),
                    id: tooltip.id,
                    name: tooltip.name.clone(),
                });
                bag(
                    &path,
                    false,
                    super::tables::TOOLTIP_BODY,
                    &mut body.properties,
                    f,
                )?;
                bag(
                    &path,
                    true,
                    super::tables::TOOLTIP_BODY,
                    &mut body.ext_info,
                    f,
                )?;
            }
        }
    }
    Ok(())
}
fn attributes(
    items: &mut [FormDataAttribute],
    parent: &[Identity],
    edge: Edge,
    f: &mut impl FnMut(&Binding, &mut DataPathSpec) -> Result<(), FormError>,
) -> Result<(), FormError> {
    for item in items {
        let mut owner = parent.to_vec();
        owner.push(Identity {
            edge,
            kind: "FormAttribute".into(),
            id: item.id,
            name: item.name.clone(),
        });
        for (i, path) in item.not_default_use_always.iter_mut().enumerate() {
            f(
                &Binding {
                    owner: owner.clone(),
                    role: Role::UseAlways(i),
                },
                path,
            )?;
        }
        for (i, path) in item.settings_saved_data.iter_mut().enumerate() {
            f(
                &Binding {
                    owner: owner.clone(),
                    role: Role::Save(i),
                },
                path,
            )?;
        }
        attributes(&mut item.columns, &owner, Edge::Column, f)?;
        for (i, group) in item.additional_columns.iter_mut().enumerate() {
            f(
                &Binding {
                    owner: owner.clone(),
                    role: Role::TablePath(i),
                },
                &mut group.table_path,
            )?;
            // Group ordinal distinguishes identical parent/column IDs without
            // smuggling a path value into its own owner identity.
            let mut p = owner.clone();
            p.push(Identity {
                edge: Edge::AdditionalColumn,
                kind: "AdditionalColumns".into(),
                id: i64::try_from(i).map_err(|_| error("actual group ordinal overflow"))?,
                name: String::new(),
            });
            attributes(&mut group.columns, &p, Edge::Column, f)?;
        }
    }
    Ok(())
}
fn visit(
    body: &mut FormBody,
    native: bool,
    f: &mut impl FnMut(&Binding, &mut DataPathSpec) -> Result<(), FormError>,
) -> Result<(), FormError> {
    attributes(&mut body.data_attributes, &[], Edge::Attribute, f)?;
    controls(&mut body.items, &[], Edge::Root, native, f)?;
    if let Some(bar) = &mut body.auto_command_bar {
        let p = [Identity {
            edge: Edge::AutoCommandBar,
            kind: "AutoCommandBar".into(),
            id: bar.id,
            name: bar.name.clone(),
        }];
        controls(&mut bar.items, &p, Edge::Child, native, f)?;
    }
    for (panel, items) in [
        (Panel::Navigation, &mut body.form_ci_navigation_panel),
        (Panel::CommandBar, &mut body.form_ci_command_bar),
    ] {
        for (ordinal, item) in items.iter_mut().enumerate() {
            if let Some(path) = &mut item.command_parameter {
                f(
                    &Binding {
                        owner: Vec::new(),
                        role: Role::Command {
                            panel,
                            ordinal,
                            command: item.command.clone(),
                        },
                    },
                    path,
                )?;
            }
        }
    }
    Ok(())
}

fn may_need_transport(body: &FormBody) -> bool {
    fn path(p: &DataPathSpec) -> bool {
        !p.extra_paths.is_empty()
            || p.primary().contains('~')
            || DataPathSpec::from_form_text(&p.primary()).segments != p.segments
    }
    fn value(v: &PropertyValue) -> bool {
        match v {
            PropertyValue::DataPath(p) => path(p),
            PropertyValue::Ref(s) => {
                s.contains('~') || DataPathSpec::from_form_text(s).primary() != *s
            }
            PropertyValue::List(v) => v.iter().any(value),
            _ => false,
        }
    }
    let mut attributes: Vec<_> = body.data_attributes.iter().collect();
    while let Some(a) = attributes.pop() {
        if a.settings_saved_data
            .iter()
            .chain(&a.not_default_use_always)
            .any(path)
        {
            return true;
        }
        attributes.extend(&a.columns);
        for group in &a.additional_columns {
            if path(&group.table_path) {
                return true;
            }
            attributes.extend(&group.columns);
        }
    }
    if body
        .form_ci_navigation_panel
        .iter()
        .chain(&body.form_ci_command_bar)
        .filter_map(|c| c.command_parameter.as_ref())
        .any(path)
    {
        return true;
    }
    let mut items: Vec<_> = body.items.iter().collect();
    if let Some(bar) = &body.auto_command_bar {
        items.extend(&bar.items);
    }
    while let Some(item) = items.pop() {
        if item
            .properties
            .iter()
            .chain(&item.ext_info)
            .any(|(_, v)| value(v))
        {
            return true;
        }
        items.extend(&item.children);
        items.extend(&item.additions);
        if let Some(t) = &item.auto_table {
            items.push(t);
        }
        if let Some(bar) = &item.auto_command_bar {
            items.extend(&bar.items);
        }
        if let Some(menu) = &item.context_menu {
            if let DecoratorBody::ContextMenu(menu) = &menu.body {
                items.extend(&menu.items);
            }
        }
        if let Some(tip) = &item.ext_tooltip {
            if let DecoratorBody::Tooltip(tip) = &tip.body {
                if tip
                    .properties
                    .iter()
                    .chain(&tip.ext_info)
                    .any(|(_, v)| value(v))
                {
                    return true;
                }
            }
        }
    }
    false
}

/// Emit a closed CURRENT section for a selected standard serializer projection.
/// The returned body retains CURRENT paths for one real serializer invocation:
/// reparsing and serializing its lossy projection again is not idempotent.
/// Native publication uses the existing ConfigDumpInfo container (separate gate).
pub fn project_data_path_semantics(
    body: &FormBody,
    form_uuid: Uuid,
    dialect: FormDialect,
    profile: FormatVersion,
    context: Option<&FormProjectionContext<'_>>,
) -> Result<Option<(FormBody, Vec<u8>)>, FormError> {
    if !may_need_transport(body) {
        return Ok(None);
    }
    let native = dialect == FormDialect::Designer;
    let mut projected = body.clone();
    let mut records = Vec::new();
    let mut seen = BTreeSet::new();
    with_roundtrip_target(profile, || {
        super::availability::with_availability(body, context, || {
            visit(&mut projected, native, &mut |binding, path| {
                let k = key(binding)?;
                if !seen.insert(k) {
                    return Err(error("duplicate current typed owner/slot"));
                }
                let current = path.clone();
                let standard = projection(binding, &current, native)?;
                if standard != current {
                    records.push(Record {
                        binding: binding.clone(),
                        projected_sha256: digest(binding, &standard)?,
                        current,
                    });
                }
                Ok(())
            })
        })
    })?;
    if records.is_empty() {
        return Ok(None);
    }
    let resource = Resource {
        schema: SCHEMA.into(),
        version: 1,
        form_uuid,
        native,
        profile: (profile.major, profile.minor),
        records,
    };
    let bytes = serde_json::to_vec(&resource).map_err(|e| error(e.to_string()))?;
    Ok(Some((projected, bytes)))
}
/// Two passes: every owner/current projection validates before any assignment.
pub fn apply_data_path_semantics_resource(
    body: &mut FormBody,
    form_uuid: Uuid,
    dialect: FormDialect,
    profile: FormatVersion,
    bytes: &[u8],
) -> Result<(), FormError> {
    apply_data_path_semantics_resource_with_context(body, form_uuid, dialect, profile, bytes, None)
}
/// Complete configurations also verify the CURRENT metadata-derived branch.
pub fn apply_data_path_semantics_resource_with_context(
    body: &mut FormBody,
    form_uuid: Uuid,
    dialect: FormDialect,
    profile: FormatVersion,
    bytes: &[u8],
    context: Option<&FormProjectionContext<'_>>,
) -> Result<(), FormError> {
    let resource: Resource =
        super::strict_resource::parse(bytes).map_err(|e| error(e.to_string()))?;
    if resource.schema != SCHEMA
        || resource.version != 1
        || resource.form_uuid != form_uuid
        || resource.native != (dialect == FormDialect::Designer)
        || resource.profile != (profile.major, profile.minor)
        || resource.records.is_empty()
    {
        return Err(error("schema/owner/profile/projection mismatch"));
    }
    let mut pending = BTreeMap::new();
    let mut expected = BTreeMap::new();
    for record in resource.records {
        validate(&record.current)?;
        let k = key(&record.binding)?;
        expected.insert(k.clone(), record.projected_sha256.clone());
        if pending.insert(k, record).is_some() {
            return Err(error("duplicate resource owner/slot"));
        }
    }
    let mut restored = body.clone();
    let mut plans = BTreeMap::new();
    let mut seen = BTreeSet::new();
    visit(&mut restored, resource.native, &mut |binding, path| {
        let k = key(binding)?;
        if !seen.insert(k.clone()) {
            return Err(error("duplicate current typed owner/slot"));
        }
        if let Some(record) = pending.remove(&k) {
            if digest(binding, path)? != record.projected_sha256 {
                return Err(error("current projected path was edited"));
            }
            if !compatible_projection(binding, &record.current, path, resource.native)? {
                return Err(error("CURRENT payload does not project to its bound path"));
            }
            plans.insert(k, record.current);
        }
        Ok(())
    })?;
    if !pending.is_empty() {
        return Err(error("deleted/unknown/wrong typed owner/slot"));
    }
    visit(&mut restored, resource.native, &mut |binding, path| {
        if let Some(value) = plans.remove(&key(binding)?) {
            *path = value;
        }
        Ok(())
    })?;
    if !plans.is_empty() {
        return Err(error("restoration accounting mismatch"));
    }
    if let Some(context) = context {
        let current = restored.clone();
        with_roundtrip_target(profile, || {
            super::availability::with_availability(&current, Some(context), || {
                visit(&mut restored, resource.native, &mut |binding, path| {
                    let k = key(binding)?;
                    if let Some(expected) = expected.get(&k) {
                        if digest(binding, &projection(binding, path, resource.native)?)?
                            != *expected
                        {
                            return Err(error("CURRENT metadata changes the bound projection"));
                        }
                    }
                    Ok(())
                })
            })
        })?;
        restored = current;
    }
    *body = restored;
    Ok(())
}

/// Strict typed resource inspection used by the single configuration container.
pub(super) fn parse_resource(bytes: &[u8]) -> Result<Resource, FormError> {
    let resource: Resource =
        super::strict_resource::parse(bytes).map_err(|e| error(e.to_string()))?;
    if resource.schema != SCHEMA || resource.version != 1 || resource.records.is_empty() {
        return Err(error("resource schema or empty records"));
    }
    let mut seen = BTreeSet::new();
    for row in &resource.records {
        validate(&row.current)?;
        if !seen.insert(key(&row.binding)?) {
            return Err(error("duplicate resource owner/slot"));
        }
    }
    Ok(resource)
}
pub(super) fn resource_uuid(resource: &Resource) -> Uuid {
    resource.form_uuid
}
pub(super) fn resource_profile(resource: &Resource) -> (u16, u16) {
    resource.profile
}
pub(super) fn resource_native(resource: &Resource) -> bool {
    resource.native
}
pub(super) fn resource_bytes(resource: &Resource) -> Result<Vec<u8>, FormError> {
    serde_json::to_vec(resource).map_err(|e| error(e.to_string()))
}
pub fn same_data_path_semantics_resource(a: &[u8], b: &[u8]) -> bool {
    match (parse_resource(a), parse_resource(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
pub fn data_path_semantics_resource_count(body: &FormBody) -> Result<Option<usize>, FormError> {
    let profile = morph1c_core::version::current_roundtrip_target().unwrap_or(morph1c_core::version::SSL);
    let mut bindings = BTreeSet::new();
    for dialect in [FormDialect::Edt, FormDialect::Designer] {
        if let Some((_, bytes)) =
            project_data_path_semantics(body, Uuid([0; 16]), dialect, profile, None)?
        {
            for row in parse_resource(&bytes)?.records {
                bindings.insert(key(&row.binding)?);
            }
        }
    }
    Ok((!bindings.is_empty()).then_some(bindings.len()))
}
/// Recheck already restored CURRENT data once all configuration metadata exists.
pub fn verify_restored_data_path_semantics(
    body: &FormBody,
    form_uuid: Uuid,
    dialect: FormDialect,
    profile: FormatVersion,
    bytes: &[u8],
    context: &FormProjectionContext<'_>,
) -> Result<(), FormError> {
    let resource = parse_resource(bytes)?;
    if resource.form_uuid != form_uuid
        || resource.native != (dialect == FormDialect::Designer)
        || resource.profile != (profile.major, profile.minor)
    {
        return Err(error("restored owner/profile mismatch"));
    }
    let mut pending: BTreeMap<_, _> = resource
        .records
        .into_iter()
        .map(|r| (key(&r.binding).expect("typed key serialization"), r))
        .collect();
    let mut copy = body.clone();
    with_roundtrip_target(profile, || {
        super::availability::with_availability(body, Some(context), || {
            visit(&mut copy, resource.native, &mut |binding, current| {
                if let Some(row) = pending.remove(&key(binding)?) {
                    if *current != row.current
                        || digest(binding, &projection(binding, current, resource.native)?)?
                            != row.projected_sha256
                    {
                        return Err(error(
                            "restored CURRENT payload/metadata projection differs",
                        ));
                    }
                }
                Ok(())
            })
        })
    })?;
    if !pending.is_empty() {
        return Err(error("restored resource has disappeared owner/slot"));
    }
    Ok(())
}
