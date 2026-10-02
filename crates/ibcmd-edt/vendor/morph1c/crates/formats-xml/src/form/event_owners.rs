//! Root event ownership from the installed EDT2025.2.3.30 runtime type models.
//! Native XML combines two ordered handler containers. Partition them stably;
//! never sort semantic arrays or retain handler values in source facets.
use std::collections::{HashMap, HashSet};

use super::FormError;
use morph1c_core::ir::form::NativeFormEventOrder;
use morph1c_core::ir::{FormBody, FormEvent, FormItem};

const ROOT: &[&str] = &[
    "ActivationProcessing",
    "AddInDetachmentOnError",
    "BeforeClose",
    "BeforeLoadDataFromSettingsAtServer",
    "BeforeReopenFromOtherServer",
    "ChoiceProcessing",
    "CollaborationSystemUsersAutoComplete",
    "CollaborationSystemUsersChoiceFormGetProcessing",
    "ExternalEvent",
    "FillCheckProcessingAtServer",
    "NavigationProcessing",
    "NewWriteProcessing",
    "NotificationProcessing",
    "OnChangeDisplaySettings",
    "OnClose",
    "OnCreateAtServer",
    "OnLoadDataFromSettingsAtServer",
    "OnMainServerAvailabilityChange",
    "OnOpen",
    "OnPasteFromClipboard",
    "OnReopen",
    "OnReopenFromOtherServer",
    "OnSaveDataInSettingsAtServer",
    "URLGetProcessing",
    "URLListGetProcessing",
    "URLProcessing",
    "OnClientApplicationSuspend",
    "OnClientApplicationResume",
];
const WRITE: &[&str] = &[
    "AfterWrite",
    "AfterWriteAtServer",
    "BeforeWrite",
    "BeforeWriteAtServer",
    "OnReadAtServer",
    "OnWriteAtServer",
];
const REPORT: &[&str] = &[
    "BeforeLoadUserSettingsAtServer",
    "BeforeLoadVariantAtServer",
    "OnLoadUserSettingsAtServer",
    "OnLoadVariantAtServer",
    "OnSaveUserSettingsAtServer",
    "OnSaveVariantAtServer",
    "OnUpdateUserSettingSetAtServer",
    "AfterComposeResult",
    "OnComposeResult",
    "OnSettingsChange",
];

fn error(reason: impl Into<String>) -> FormError {
    FormError::Frame(format!("root event ownership: {}", reason.into()))
}
// SDK root EventHandlerContainer can retain an unresolved symbolic Event
// reference. Its name is an open nonempty string, not a UUID grammar. Preserve
// the typed identity verbatim; recognized extension-only names retain their
// declared ownership even when this root container allows unresolved proxies.
fn root_event_name(kind: Option<&str>, name: &str) -> bool {
    ROOT.contains(&name) || (!name.is_empty() && !extension_allows(kind, name))
}

fn extension_allows(kind: Option<&str>, name: &str) -> bool {
    // SDK EventHandlerXmlPartReader excludes base Form event names from ExtInfo,
    // even when a runtime extension repeats that name (ActivationProcessing).
    if ROOT.contains(&name) {
        return false;
    }
    match kind {
        Some(
            "form:CatalogFormExtInfo"
            | "form:DocumentFormExtInfo"
            | "form:ChartOfCharacteristicTypesFormExtInfo"
            | "form:ObjectFormExtInfo"
            | "form:TableObjectFormExtInfo",
        ) => WRITE.contains(&name) || name == "ValueChoice",
        Some(
            "form:ConstantsFormExtInfo"
            | "form:InformationRegisterManagerFormExtInfo"
            | "form:RecordSetFormExtInfo"
            | "form:TableRecordFormExtInfo",
        ) => WRITE.contains(&name),
        Some("form:BusinessProcesFormExtInfo") => {
            WRITE.contains(&name) || matches!(name, "ValueChoice" | "BeforeStart")
        }
        Some("form:TaskFormExtInfo") => {
            WRITE.contains(&name) || matches!(name, "ValueChoice" | "BeforeExecute")
        }
        Some("form:ReportFormExtInfo") => REPORT.contains(&name),
        Some("form:SettingsComposerFormExtInfo") => name == "OnUpdateUserSettingSetAtServer",
        Some("form:CubeRecordFormExtInfo" | "form:CubeRecordSetFormExtInfo") => {
            name == "OnReadAtServer"
        }
        _ => false,
    }
}
fn names(events: &[FormEvent]) -> Vec<String> {
    events.iter().map(|event| event.name.clone()).collect()
}

// FormTable / FormTableExtensionForDynamicList runtime type models, 8.3.27
// and 8.5.1. The latter adds the two base events at the end of this list.
const TABLE: &[&str] = &[
    "AfterDeleteRow",
    "BeforeAddRow",
    "BeforeCollapse",
    "BeforeDeleteRow",
    "BeforeEditEnd",
    "BeforeExpand",
    "BeforeRowChange",
    "ChoiceProcessing",
    "Drag",
    "DragCheck",
    "DragEnd",
    "DragStart",
    "NewWriteProcessing",
    "OnActivateCell",
    "OnActivateField",
    "OnActivateRow",
    "OnChange",
    "OnCurrentParentChange",
    "OnEditEnd",
    "OnStartEdit",
    "RefreshRequestProcessing",
    "Selection",
    "ValueChoice",
    "OnHover",
    "OnSelectedRowsSetChange",
];
const TABLE_EXTENSION: &[&str] = &[
    "BeforeLoadUserSettingsAtServer",
    "OnGetDataAtServer",
    "OnLoadUserSettingsAtServer",
    "OnSaveUserSettingsAtServer",
    "OnUpdateUserSettingSetAtServer",
    "URLGetProcessing",
    "URLListGetProcessing",
];
const TABLE_EXTENSION_KIND: &str = "form:DynamicListTableExtInfo";

// Like Form, Table is an EventHandlerContainer. The SDK retains an open
// symbolic Event reference in its own handlers; its nested ExtInfo excludes
// those handlers and accepts only the declared extension event names.
fn table_body_event(has_extension: bool, name: &str) -> bool {
    TABLE.contains(&name)
        || (!name.is_empty() && (!has_extension || !TABLE_EXTENSION.contains(&name)))
}

pub(crate) fn partition_native_table(item: &mut FormItem) -> Result<(), FormError> {
    let merged = names(&item.events);
    let mut root = Vec::new();
    let mut extension = Vec::new();
    let mut seen = HashSet::new();
    for event in std::mem::take(&mut item.events) {
        if event.name.is_empty() || event.handler.is_empty() || !seen.insert(event.name.clone()) {
            return Err(error(format!("duplicate Table event {}", event.name)));
        }
        if table_body_event(item.dynamic_list_ext.is_some(), &event.name) {
            root.push(event);
        } else if item.dynamic_list_ext.is_some() && TABLE_EXTENSION.contains(&event.name.as_str())
        {
            extension.push(event);
        } else {
            return Err(error(format!(
                "unknown or wrong-owner Table event {}",
                event.name
            )));
        }
    }
    item.native_table_event_order = Some(NativeFormEventOrder {
        extension_kind: item
            .dynamic_list_ext
            .as_ref()
            .map(|_| TABLE_EXTENSION_KIND.into()),
        merged,
        root: names(&root),
        extension: names(&extension),
    });
    item.events = root;
    if let Some(info) = &mut item.dynamic_list_ext {
        info.events = extension;
    }
    Ok(())
}

pub(crate) fn validate_table_owned(item: &FormItem) -> Result<(), FormError> {
    if item.kind.as_str() != "Table" {
        return Err(error("Table event owner has a different control kind"));
    }
    let mut seen = HashSet::new();
    for event in &item.events {
        if event.handler.is_empty() || !table_body_event(item.dynamic_list_ext.is_some(), &event.name)
            || !seen.insert(event.name.as_str())
        {
            return Err(error(format!(
                "unknown, duplicate or wrong-owner Table event {}",
                event.name
            )));
        }
    }
    if let Some(info) = &item.dynamic_list_ext {
        for event in &info.events {
            if event.handler.is_empty() || !TABLE_EXTENSION.contains(&event.name.as_str()) || !seen.insert(event.name.as_str())
            {
                return Err(error(format!(
                    "unknown, duplicate or wrong-owner DynamicList event {}",
                    event.name
                )));
            }
        }
    }
    Ok(())
}

pub(crate) fn native_table_order(item: &FormItem) -> Result<Vec<&FormEvent>, FormError> {
    validate_table_owned(item)?;
    let extension = item
        .dynamic_list_ext
        .as_ref()
        .map(|info| info.events.as_slice())
        .unwrap_or(&[]);
    if let Some(source) = &item.native_table_event_order {
        if source.extension_kind.as_deref()
            == item.dynamic_list_ext.as_ref().map(|_| TABLE_EXTENSION_KIND)
            && source.root == names(&item.events)
            && source.extension == names(extension)
        {
            let current: HashMap<_, _> = item
                .events
                .iter()
                .chain(extension)
                .map(|event| (event.name.as_str(), event))
                .collect();
            if current.len() == source.merged.len() {
                let mut used = HashSet::new();
                let ordered: Option<Vec<_>> = source
                    .merged
                    .iter()
                    .map(|name| {
                        if !used.insert(name.as_str()) {
                            return None;
                        }
                        current.get(name.as_str()).copied()
                    })
                    .collect();
                if let Some(ordered) = ordered {
                    return Ok(ordered);
                }
            }
        }
    }
    Ok(morph1c_core::ir::merge_table_events(
        &item.events,
        extension,
    ))
}

pub(crate) fn partition_native(body: &mut FormBody) -> Result<(), FormError> {
    let kind = body.root_ext_info.as_ref().map(|info| info.kind.clone());
    let merged = names(&body.events);
    let mut seen = HashSet::new();
    let mut root = Vec::new();
    let mut extension = Vec::new();
    for event in std::mem::take(&mut body.events) {
        if event.name.is_empty() || event.handler.is_empty() || !seen.insert(event.name.clone()) {
            return Err(error(format!("duplicate event {}", event.name)));
        }
        if root_event_name(kind.as_deref(), &event.name) {
            root.push(event);
        } else if extension_allows(kind.as_deref(), &event.name) {
            extension.push(event);
        } else {
            return Err(error(format!(
                "unknown event {} for {:?}",
                event.name, kind
            )));
        }
    }
    body.native_event_order = Some(NativeFormEventOrder {
        extension_kind: kind,
        merged,
        root: names(&root),
        extension: names(&extension),
    });
    body.events = root;
    if let Some(info) = &mut body.root_ext_info {
        info.events = extension;
    }
    Ok(())
}

pub(crate) fn validate_owned(body: &FormBody) -> Result<(), FormError> {
    let mut seen = HashSet::new();
    for event in &body.events {
        if event.handler.is_empty() || !root_event_name(body.root_ext_info.as_ref().map(|info| info.kind.as_str()), &event.name) {
            return Err(error(format!(
                "event {} does not belong to Form",
                event.name
            )));
        }
        if !seen.insert(event.name.as_str()) {
            return Err(error(format!("duplicate event {}", event.name)));
        }
    }
    if let Some(info) = &body.root_ext_info {
        for event in &info.events {
            if event.handler.is_empty() || !extension_allows(Some(&info.kind), &event.name) {
                return Err(error(format!(
                    "event {} does not belong to {}",
                    event.name, info.kind
                )));
            }
            if !seen.insert(event.name.as_str()) {
                return Err(error(format!("duplicate event {}", event.name)));
            }
        }
    }
    Ok(())
}

pub(crate) fn native_order(body: &FormBody) -> Result<Vec<&FormEvent>, FormError> {
    validate_owned(body)?;
    let extension = body
        .root_ext_info
        .as_ref()
        .map(|info| info.events.as_slice())
        .unwrap_or(&[]);
    if let Some(source) = &body.native_event_order {
        // Editing either owner's name/order invalidates lexical replay. Values
        // always come from current typed events, including changed handlers.
        if source.extension_kind.as_deref()
            == body.root_ext_info.as_ref().map(|info| info.kind.as_str())
            && source.root == names(&body.events)
            && source.extension == names(extension)
        {
            let current: HashMap<_, _> = body
                .events
                .iter()
                .chain(extension)
                .map(|event| (event.name.as_str(), event))
                .collect();
            if current.len() == source.merged.len() {
                let mut used = HashSet::new();
                let ordered: Option<Vec<_>> = source
                    .merged
                    .iter()
                    .map(|name| {
                        if !used.insert(name.as_str()) {
                            return None;
                        }
                        current.get(name.as_str()).copied()
                    })
                    .collect();
                if let Some(ordered) = ordered {
                    return Ok(ordered);
                }
            }
        }
    }
    Ok(morph1c_core::ir::merge_form_events(
        &body.events,
        extension,
        body.root_ext_info
            .as_ref()
            .is_some_and(|info| info.kind == "form:DocumentFormExtInfo"),
    ))
}

/// Exact native event scalar roles in the supported typed form grammar.
/// Namespace resolution and complete XML validation remain the caller's job.
#[doc(hidden)]
pub fn is_native_form_event_path(names: &[String]) -> bool {
    if names.first().map(String::as_str) != Some("Form")
        || names.last().map(String::as_str) != Some("Events")
    {
        return false;
    }
    let mut owner = "Form";
    let mut index = 1;
    while index < names.len() - 1 {
        let tag = names[index].as_str();
        match tag {
            "ChildItems" => {
                if !(owner == "Form"
                    || owner == "Table" || owner == "ContextMenu"
                    || super::tables::group_kind(owner).is_some())
                    || index + 1 >= names.len() - 1
                {
                    return false;
                }
                owner = names[index + 1].as_str();
                if !(owner == "Table"
                    || owner == "Button" || super::tables::addition_kind(owner).is_some()
                    || super::tables::field_kind_by_des_tag(owner).is_some()
                    || super::tables::group_kind(owner).is_some()
                    || super::tables::decoration_kind(owner).is_some())
                {
                    return false;
                }
                index += 2;
            }
            "AutoCommandBar" if owner == "Form" || owner == "Table" => {
                owner = "CommandBar";
                index += 1;
            }
            "ContextMenu" if owner != "ContextMenu" && owner != "CommandBar" => {
                owner = "ContextMenu"; index += 1;
            }
            tag if (owner == "Table" || owner == "PDFDocumentField")
                && super::tables::addition_kind(tag).is_some() => {
                owner = tag; index += 1;
            }
            "ExtendedTooltip" if owner != "ExtendedTooltip" => {
                owner = "ExtendedTooltip";
                index += 1;
            }
            "Table" if owner == "GanttChartField" => {
                owner = "Table";
                index += 1;
            }
            _ => return false,
        }
    }
    owner == "Form"
        || owner == "Table"
        || owner == "ExtendedTooltip" || super::tables::addition_kind(owner).is_some()
        || super::tables::field_kind_by_des_tag(owner).is_some()
        || super::tables::group_kind(owner).is_some()
        || super::tables::decoration_kind(owner).is_some()
}

pub(crate) fn field_extension_kind(item: &FormItem) -> Result<&'static super::tables::FieldKind, FormError> {
    let base = super::tables::field_kind(item.kind.as_str())
        .ok_or_else(|| error("unknown field base type"))?;
    if item.field_type_none && base.kind != "InputField" {
        return Err(error("None base type must use its InputField native projection"));
    }
    match item.field_extension_kind.as_deref() {
        Some(name) => {
            let actual = super::tables::FIELD_KINDS.iter().find(|kind| kind.ext_xsi == name)
                .ok_or_else(|| error("unknown actual FieldExtInfo kind"))?;
            // Official FieldValidator.checkCorrectExtInfoType permits arbitrary
            // actual extension only for None; explicit types require their class.
            if !item.field_type_none && actual.ext_xsi != base.ext_xsi {
                return Err(error("explicit field type has a different actual extension (SDK200)"));
            }
            Ok(actual)
        },
        None => Ok(base),
    }
}

fn field_extension_names(kind: &str) -> &'static [&'static str] {
    use morph1c_core::version::{FormatVersion, current_roundtrip_target, current_source_version};
    let version = current_roundtrip_target()
        .or(current_source_version())
        .unwrap_or(FormatVersion::new(2, 21));
    super::event_catalog::field_extension_events(kind, version >= FormatVersion::new(2, 21))
}

pub(crate) fn field_owned_events(
    item: &FormItem,
) -> Result<(Vec<&FormEvent>, Vec<&FormEvent>, bool), FormError> {
    use morph1c_core::ir::form::FieldEventOwner;
    let kind = field_extension_kind(item)?;
    let ownership = item.field_event_owners.as_ref();
    if let Some(ownership) = ownership {
        if ownership.extension_kind != kind.ext_xsi
            || ownership.owners.len() != item.events.len()
        {
            return Err(error("field owner kind/count differs from current events"));
        }
    }
    let attached = ownership.is_none_or(|owner| owner.extension_attached);
    let allowed = field_extension_names(kind.kind);
    let mut seen = HashSet::new();
    let mut body = Vec::new();
    let mut extension = Vec::new();
    for event in &item.events {
        if event.name.is_empty() || event.handler.is_empty() || !seen.insert(event.name.as_str()) {
            return Err(error("empty or duplicate field event identity/handler"));
        }
        let owner = match ownership {
            Some(ownership) => *ownership.owners.get(&event.name)
                .ok_or_else(|| error("field owner binding misses a current event"))?,
            None if event.name != "OnChange" && allowed.contains(&event.name.as_str()) =>
                FieldEventOwner::Extension,
            None => FieldEventOwner::Body,
        };
        match owner {
            FieldEventOwner::Body => {
                if event.name != "OnChange" && attached && allowed.contains(&event.name.as_str()) {
                    return Err(error("known field extension event is assigned to its body"));
                }
                body.push(event);
            }
            FieldEventOwner::Extension => {
                if !attached || event.name == "OnChange" || !allowed.contains(&event.name.as_str()) {
                    return Err(error("unknown or wrong-owner nested field extension event"));
                }
                extension.push(event);
            }
        }
    }
    Ok((body, extension, attached))
}

pub(crate) fn bind_edt_field_events(
    item: &mut FormItem,
    body_count: usize,
    extension_attached: bool,
) -> Result<(), FormError> {
    use morph1c_core::ir::form::{FieldEventOwner, FieldEventOwners};
    let kind = field_extension_kind(item)?;
    if body_count > item.events.len() { return Err(error("field body event count overflow")); }
    let mut owners = std::collections::BTreeMap::new();
    for (index, event) in item.events.iter().enumerate() {
        let owner = if index < body_count { FieldEventOwner::Body } else { FieldEventOwner::Extension };
        if owners.insert(event.name.clone(), owner).is_some() {
            return Err(error("duplicate field event ownership"));
        }
    }
    item.field_event_owners = Some(FieldEventOwners {
        extension_kind: kind.ext_xsi.into(), extension_attached, owners,
    });
    field_owned_events(item)?;
    // Validate explicit placement first; attached ownership is derivable from
    // CURRENT names and actual kind, so only nondefault topology is retained.
    if extension_attached { item.field_event_owners = None; }
    Ok(())
}

pub(crate) fn partition_native_field(item: &mut FormItem) -> Result<(), FormError> {
    let merged = names(&item.events);
    // Native control type supplies its actual FieldExtInfo; native Events is a
    // flattened projection of that container and the base FormField container.
    let allowed = field_extension_names(item.kind.as_str());
    let mut body = Vec::new();
    let mut extension = Vec::new();
    for event in std::mem::take(&mut item.events) {
        if event.name != "OnChange" && allowed.contains(&event.name.as_str()) {
            extension.push(event);
        } else { body.push(event); }
    }
    let body_count = body.len();
    let root_names = names(&body);
    let extension_names = names(&extension);
    body.extend(extension);
    item.events = body;
    bind_edt_field_events(item, body_count, true)?;
    item.native_field_event_order = Some(NativeFormEventOrder {
        extension_kind: Some(super::tables::field_kind(item.kind.as_str()).unwrap().ext_xsi.into()),
        merged, root: root_names, extension: extension_names,
    });
    Ok(())
}

pub(crate) fn native_field_order(item: &FormItem) -> Result<Vec<&FormEvent>, FormError> {
    let (body, extension, _) = field_owned_events(item)?;
    if let Some(source) = &item.native_field_event_order {
        let body_names: Vec<_> = body.iter().map(|event| event.name.clone()).collect();
        let extension_names: Vec<_> = extension.iter().map(|event| event.name.clone()).collect();
        let kind = field_extension_kind(item)?.ext_xsi;
        if source.extension_kind.as_deref() == Some(kind)
            && source.root == body_names && source.extension == extension_names
            && source.merged.len() == item.events.len()
        {
            let current: HashMap<_, _> = item.events.iter().map(|event| (event.name.as_str(),event)).collect();
            let mut used = HashSet::new();
            let ordered: Option<Vec<_>> = source.merged.iter().map(|name| {
                if !used.insert(name.as_str()) { return None; }
                current.get(name.as_str()).copied()
            }).collect();
            if let Some(ordered) = ordered { return Ok(ordered); }
        }
    }
    Ok(body.into_iter().chain(extension).collect())
}

/// Native cannot carry an independent FieldExtInfo class. Project only that
/// extension to the declared base class; the closed resource owns its CURRENT
/// typed properties and event partition, never source XML or handler copies.
pub(crate) fn native_field_projection(item: &FormItem) -> Result<Option<FormItem>, FormError> {
    let base = super::tables::field_kind(item.kind.as_str()).ok_or_else(|| error("unknown field base type"))?;
    let actual = field_extension_kind(item)?;
    if actual.ext_xsi == base.ext_xsi { return Ok(None); }
    let ordered = native_field_order(item)?;
    let mut projected = item.clone();
    projected.field_extension_kind = None;
    projected.field_event_owners = None;
    projected.field_type_none = false;
    projected.ext_info.clear();
    super::fields::read_fields_edt(base.kind, None, base.ext, super::fields::Region::Ext, &mut projected.ext_info)?;
    projected.font = None; projected.auto_table = None; projected.additions.clear();
    let (body, extension, _) = field_owned_events(&projected)?;
    projected.native_field_event_order = Some(NativeFormEventOrder {
        extension_kind: Some(base.ext_xsi.into()),
        merged: ordered.iter().map(|e| e.name.clone()).collect(),
        root: body.iter().map(|e| e.name.clone()).collect(),
        extension: extension.iter().map(|e| e.name.clone()).collect(),
    });
    Ok(Some(projected))
}
