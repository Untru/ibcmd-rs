//! Base-free serializers for the configuration's interface assets that sit
//! beside its command interfaces: `Ext/HomePageWorkArea.xml` (row `.8`),
//! `Ext/ClientApplicationInterface.xml` (row `.b`) and
//! `Ext/StandaloneConfigurationContent.bin` (row `.f`).
//!
//! Each is the inverse of the exporter's reader for the same row
//! (`parse_home_page_work_area_text`, `parse_client_application_interface_text`
//! and `extract_standalone_content_xml`), laid out the way the platform lays out
//! its own plain text.

use ibcmd_core::identity::ObjectUuid;

use crate::compiler::bodies::command_interface::{VisibilityValue, native_visibility};
use crate::compiler::families::native::{
    NativeError, NativeValue, platform_list, serialize, text, token,
};

const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";

/// `{1,<template>,<left count>,<items>,<right count>,<items>,2}`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomePageWorkAreaModel {
    /// The stored template code; `2` is `TwoColumnsVariableWidth`.
    pub template: &'static str,
    pub left_column: Vec<HomePageWorkAreaItem>,
    pub right_column: Vec<HomePageWorkAreaItem>,
}

/// `{0,{0,<form uuid>},<height>,<visibility>}`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HomePageWorkAreaItem {
    pub form: ObjectUuid,
    /// The height exactly as the source spells it: a non-negative integer.
    pub height: String,
    pub common: bool,
    pub values: Vec<VisibilityValue>,
}

pub(crate) fn home_page_work_area_plaintext(
    model: &HomePageWorkAreaModel,
) -> Result<Vec<u8>, NativeError> {
    let mut fields = vec![token("1"), token(model.template)];
    for column in [&model.left_column, &model.right_column] {
        fields.push(token(column.len().to_string()));
        for item in column {
            fields.push(platform_list(vec![
                token("0"),
                platform_list(vec![token("0"), token(item.form.to_string())]),
                token(item.height.clone()),
                native_visibility(item.common, &item.values),
            ]));
        }
    }
    // Every stored record of both corpora closes on `2`; the exporter does
    // not read it and the source has no element for it.
    fields.push(token("2"));
    serialize(&platform_list(fields))
}

/// The platform's own class ids for the two kinds of node an area holds,
/// written before each node. Constant over every stored record of both
/// corpora; the exporter skips them.
pub(crate) const CLIENT_APPLICATION_GROUP_MARKER: &str = "f1d2a809-46ac-46ab-8b37-b2135ac5b323";
pub(crate) const CLIENT_APPLICATION_PANEL_MARKER: &str = "8b252cb5-d895-4c40-8ccb-581ce795fb80";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientApplicationInterfaceModel {
    pub top: Vec<ClientApplicationNode>,
    pub left: Vec<ClientApplicationNode>,
    pub bottom: Vec<ClientApplicationNode>,
    pub panel_defs: Vec<ClientApplicationPanelDef>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientApplicationNode {
    /// `{0,<id>,0,{0,<count>,<marker>,<child>,…}}`.
    Group {
        id: ObjectUuid,
        children: Vec<ClientApplicationNode>,
    },
    /// `{0,<id>,<panel uuid>,2,0}`.
    Panel { id: ObjectUuid, uuid: ObjectUuid },
}

/// One `<panelDef>`: `2,{<id>,0}` for one of the platform's standard
/// definitions, `1,{<id>,1,""}` for any other.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientApplicationPanelDef {
    pub id: ObjectUuid,
    pub standard: bool,
}

pub(crate) fn client_application_interface_plaintext(
    model: &ClientApplicationInterfaceModel,
) -> Result<Vec<u8>, NativeError> {
    let mut fields = vec![token("1")];
    // Areas are stored in code order: 1 top, 2 bottom, 3 left, 4 (never
    // populated on the stand).
    for (code, nodes) in [
        ("1", model.top.as_slice()),
        ("2", model.bottom.as_slice()),
        ("3", model.left.as_slice()),
        ("4", &[][..]),
    ] {
        fields.push(platform_list(vec![
            token("0"),
            token(code),
            client_application_children(nodes),
        ]));
    }
    for panel_def in &model.panel_defs {
        if panel_def.standard {
            fields.push(token("2"));
            fields.push(platform_list(vec![
                token(panel_def.id.to_string()),
                token("0"),
            ]));
        } else {
            fields.push(token("1"));
            fields.push(platform_list(vec![
                token(panel_def.id.to_string()),
                token("1"),
                text(""),
            ]));
        }
    }
    fields.push(token("0"));
    serialize(&platform_list(fields))
}

fn client_application_children(nodes: &[ClientApplicationNode]) -> NativeValue {
    let mut fields = vec![token("0"), token(nodes.len().to_string())];
    for node in nodes {
        match node {
            ClientApplicationNode::Group { id, children } => {
                fields.push(token(CLIENT_APPLICATION_GROUP_MARKER));
                fields.push(platform_list(vec![
                    token("0"),
                    token(id.to_string()),
                    token("0"),
                    client_application_children(children),
                ]));
            }
            ClientApplicationNode::Panel { id, uuid } => {
                fields.push(token(CLIENT_APPLICATION_PANEL_MARKER));
                fields.push(platform_list(vec![
                    token("0"),
                    token(id.to_string()),
                    token(uuid.to_string()),
                    token("2"),
                    token("0"),
                ]));
            }
        }
    }
    platform_list(fields)
}

/// `{2,<used count>,<used>…}` alone, or followed by the extended sections
/// `<unused count>,<unused>…,<priority count>,(<uuid>,2)…` and the data
/// exchange settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneContentModel {
    pub used: Vec<ObjectUuid>,
    pub extended: Option<StandaloneContentExtended>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StandaloneContentExtended {
    pub unused: Vec<ObjectUuid>,
    /// Items whose priority is `LocalServer`, stored as `<uuid>,2`.
    pub local_server_priority: Vec<ObjectUuid>,
}

pub(crate) fn standalone_content_plaintext(
    model: &StandaloneContentModel,
) -> Result<Vec<u8>, NativeError> {
    let mut fields = vec![token("2"), token(model.used.len().to_string())];
    fields.extend(model.used.iter().map(|uuid| token(uuid.to_string())));
    if let Some(extended) = &model.extended {
        fields.push(token(extended.unused.len().to_string()));
        fields.extend(extended.unused.iter().map(|uuid| token(uuid.to_string())));
        fields.push(token(extended.local_server_priority.len().to_string()));
        for uuid in &extended.local_server_priority {
            fields.push(token(uuid.to_string()));
            fields.push(token("2"));
        }
        // `<DataExchangeSettings>` as both corpora store it, which is also
        // the only one the exporter writes: exchange on change `true`, period
        // 300, 1000 records per transaction, cleanup timeout 0, between a nil
        // uuid and a trailing 0 the source has no element for.
        for value in [NIL_UUID, "1", "300", "1000", "0", "0"] {
            fields.push(token(value));
        }
    }
    serialize(&platform_list(fields))
}
