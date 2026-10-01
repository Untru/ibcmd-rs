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

pub(crate) fn partition_native_table(item: &mut FormItem) -> Result<(), FormError> {
    let merged = names(&item.events);
    let mut root = Vec::new();
    let mut extension = Vec::new();
    let mut seen = HashSet::new();
    for event in std::mem::take(&mut item.events) {
        if !seen.insert(event.name.clone()) {
            return Err(error(format!("duplicate Table event {}", event.name)));
        }
        if TABLE.contains(&event.name.as_str()) {
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
        if !TABLE.contains(&event.name.as_str()) || !seen.insert(event.name.as_str()) {
            return Err(error(format!(
                "unknown, duplicate or wrong-owner Table event {}",
                event.name
            )));
        }
    }
    if let Some(info) = &item.dynamic_list_ext {
        for event in &info.events {
            if !TABLE_EXTENSION.contains(&event.name.as_str()) || !seen.insert(event.name.as_str())
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
        if !seen.insert(event.name.clone()) {
            return Err(error(format!("duplicate event {}", event.name)));
        }
        if ROOT.contains(&event.name.as_str()) {
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
        if !ROOT.contains(&event.name.as_str()) {
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
            if !extension_allows(Some(&info.kind), &event.name) {
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
