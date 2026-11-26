//! What the descriptor rows of one configuration say about themselves: the
//! kind of every row, the names of the objects, and how to decode a row into
//! the element tree the tree diff compares.
//!
//! The rows alone are enough: the `root` row names the Configuration row, the
//! Configuration row lists every top-level object by kind, and an owner's row
//! lists its forms, templates and nested subsystems. This is the plan the
//! model export builds (`mssql_dump::model_export`), without the export.

use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result, anyhow};
use rayon::prelude::*;

use crate::metadata_model::brace::{Brace, parse_row};
use crate::metadata_model::export::names::{
    has_decoder, has_names, has_owned_names, may_own_objects, own_header, owned_objects,
    predefined_items, predefined_suffix, root_kinds,
};
use crate::metadata_model::export::{
    ExportContext, NameIndex, decode_object, export_descriptor, object_names, owned_object_names,
};
use crate::metadata_model::objects::parts::Compat;
use crate::metadata_model::root::ConfigurationShape;
use crate::metadata_model::root::export::{
    fold_split_compatibility, root_configuration_uuid, stored_shape, version_text,
};
use crate::metadata_model::xml::Element;

/// One configuration's descriptor rows, read.
pub struct Plan {
    /// The uuid of the Configuration row.
    pub configuration: Option<String>,
    /// The uuid the configuration's own assets (modules, help, pictures) are
    /// stored under.
    pub module_group: Option<String>,
    /// uuid -> kind of every row the plan could place.
    pub kinds: HashMap<String, &'static str>,
    /// uuid -> full name of an object inside a row (attribute, tabular
    /// section, command, ...), by owner.
    pub children: HashMap<String, String>,
    pub names: NameIndex,
    pub version: String,
    pub compat: Compat,
    /// What went wrong while planning (a row that could not be read).
    pub errors: Vec<String>,
}

/// The kind of the object a body row `<owner>.<suffix>` belongs to: a top-level or owned object, the
/// configuration's module group, or a nested command (a child of some object, which has no row of its own
/// but keeps its module under its own uuid).
pub fn owner_kind(
    kinds: &HashMap<String, &'static str>,
    module_group: Option<&str>,
    children: &HashMap<String, String>,
    owner: &str,
) -> Option<&'static str> {
    if let Some(kind) = kinds.get(owner) {
        return Some(kind);
    }
    if module_group == Some(owner) {
        return Some("Configuration");
    }
    let name = children.get(owner)?;
    // `Catalog.X.Command.C`, `Catalog.X.TabularSection.T`, ...
    let parts = name.split('.').collect::<Vec<_>>();
    match parts.get(parts.len().checked_sub(2)?) {
        Some(&"Command") => Some("Command"),
        _ => None,
    }
}

/// The XML dialect to decode with: named, else the one the Configuration
/// row's shape implies.
fn dialect_of(configuration: Option<&Brace>, named: Option<&str>) -> String {
    if let Some(named) = named {
        return named.to_string();
    }
    match configuration.map(stored_shape) {
        Some(Ok(ConfigurationShape::V76)) => "2.21".to_string(),
        _ => "2.20".to_string(),
    }
}

fn text_of(row: &[u8]) -> &str {
    let text = std::str::from_utf8(row).unwrap_or_default();
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// The plan of `descriptors` (uuid -> inflated row of every object, the
/// Configuration row among them) under `root` (the inflated `root` row).
/// `load_predefined` is given the names of the predefined-data bodies
/// (`<uuid>.1c` and so on) of the objects that store predefined items, and
/// returns the rows it has (name -> inflated): the name index reads the names
/// of predefined items from them.
#[allow(
    clippy::type_complexity,
    reason = "the loader's signature says what it reads and returns"
)]
pub fn build(
    descriptors: &BTreeMap<String, Vec<u8>>,
    root: Option<&[u8]>,
    xml_version: Option<&str>,
    load_predefined: &dyn Fn(&[String]) -> BTreeMap<String, Vec<u8>>,
) -> Plan {
    let mut errors = Vec::new();
    let mut plan = Plan {
        configuration: None,
        module_group: None,
        kinds: HashMap::new(),
        children: HashMap::new(),
        names: NameIndex::default(),
        version: dialect_of(None, xml_version),
        compat: Compat(8, 3, 27),
        errors: Vec::new(),
    };

    // The root row names the Configuration row.
    let configuration_uuid = match root.map(parse_row).transpose() {
        Ok(Some(row)) => match root_configuration_uuid(&row) {
            Ok(uuid) => Some(uuid),
            Err(error) => {
                errors.push(format!("the root row: {error:#}"));
                None
            }
        },
        Ok(None) => {
            errors.push("there is no root row".to_string());
            None
        }
        Err(error) => {
            errors.push(format!("the root row: {error:#}"));
            None
        }
    };
    let configuration_row = configuration_uuid
        .as_ref()
        .and_then(|uuid| descriptors.get(uuid))
        .and_then(|bytes| match parse_row(bytes) {
            Ok(row) => Some(row),
            Err(error) => {
                errors.push(format!("the Configuration row: {error:#}"));
                None
            }
        });
    plan.version = dialect_of(configuration_row.as_ref(), xml_version);
    if let (Some(uuid), Some(row)) = (&configuration_uuid, &configuration_row) {
        plan.configuration = Some(uuid.clone());
        plan.kinds.insert(uuid.clone(), "Configuration");
        plan.module_group = own_header(row).map(|(uuid, _)| uuid);
        for (uuid, kind) in root_kinds(row) {
            plan.kinds.insert(uuid, kind);
        }
    }

    // What the top-level rows own, owners before what they own.
    let owners = plan
        .kinds
        .iter()
        .filter(|(uuid, kind)| **kind != "Configuration" && descriptors.contains_key(*uuid))
        .map(|(uuid, kind)| (uuid.clone(), *kind))
        .collect::<Vec<_>>();
    let owned_lists = owners
        .par_iter()
        .filter_map(|(uuid, _)| {
            let bytes = descriptors.get(uuid)?;
            if !may_own_objects(text_of(bytes)) {
                return None;
            }
            let row = parse_row(bytes).ok()?;
            let list = owned_objects(&row);
            (!list.is_empty()).then(|| (uuid.clone(), list))
        })
        .collect::<Vec<_>>();
    let mut owned = Vec::<(String, &'static str, String)>::new();
    let mut pending = owned_lists;
    pending.reverse();
    while let Some((owner, list)) = pending.pop() {
        for (kind, uuid) in list {
            if !descriptors.contains_key(&uuid) || plan.kinds.contains_key(&uuid) {
                continue;
            }
            plan.kinds.insert(uuid.clone(), kind);
            owned.push((owner.clone(), kind, uuid.clone()));
            if kind == "Subsystem"
                && let Some(bytes) = descriptors.get(&uuid)
                && may_own_objects(text_of(bytes))
                && let Ok(row) = parse_row(bytes)
            {
                let nested = owned_objects(&row);
                if !nested.is_empty() {
                    pending.push((uuid, nested));
                }
            }
        }
    }

    // The names: every top-level row through its kind, then the owned rows
    // under their owners' names.
    let named = plan
        .kinds
        .iter()
        .filter(|(uuid, _)| descriptors.contains_key(*uuid))
        .filter(|(uuid, kind)| {
            owned.iter().all(|(_, _, owned_uuid)| owned_uuid != *uuid) && has_names(kind)
        })
        .map(|(uuid, kind)| (uuid.clone(), *kind))
        .collect::<Vec<_>>();
    let results = named
        .par_iter()
        .map(|(uuid, kind)| {
            let result = descriptors
                .get(uuid)
                .ok_or_else(|| anyhow!("no row"))
                .and_then(|bytes| parse_row(bytes))
                .and_then(|row| object_names(kind, &row));
            (uuid.clone(), *kind, result)
        })
        .collect::<Vec<_>>();
    for (uuid, kind, result) in results {
        match result {
            Ok(names) => {
                for (full_name, child) in &names.children {
                    plan.children.insert(child.clone(), full_name.clone());
                }
                plan.names.add(&names);
            }
            Err(error) => errors.push(format!("the names of {kind} {uuid}: {error:#}")),
        }
    }
    if let Some(uuid) = &plan.configuration {
        plan.names.insert_name(uuid, "Configuration");
    }
    // Top-level kinds without a name reader: the row's own header.
    for (uuid, kind) in plan.kinds.clone() {
        if plan.names.has_name(&uuid) || owned.iter().any(|(_, _, owned_uuid)| *owned_uuid == uuid)
        {
            continue;
        }
        if let Some((_, name)) = descriptors
            .get(&uuid)
            .and_then(|bytes| parse_row(bytes).ok())
            .and_then(|row| own_header(&row))
        {
            plan.names.insert_name(&uuid, &format!("{kind}.{name}"));
        }
    }
    for (owner, kind, uuid) in &owned {
        let Some(owner_name) = plan.names.name(owner).map(str::to_string) else {
            continue;
        };
        let row = descriptors
            .get(uuid)
            .and_then(|bytes| parse_row(bytes).ok());
        let mut named_here = false;
        if has_owned_names(kind)
            && let Some(row) = &row
        {
            match owned_object_names(kind, row, &owner_name) {
                Ok(names) => {
                    plan.names.set_name(&names.uuid, &names.full_name);
                    for (full_name, child) in &names.children {
                        plan.children.insert(child.clone(), full_name.clone());
                    }
                    plan.names.add(&names);
                    named_here = true;
                }
                Err(error) => errors.push(format!("the names of {kind} {uuid}: {error:#}")),
            }
        }
        if !named_here && let Some((_, name)) = row.as_ref().and_then(own_header) {
            plan.names
                .set_name(uuid, &format!("{owner_name}.{kind}.{name}"));
        }
    }

    // The names of predefined items, from the bodies, under their owners.
    let wanted = plan
        .kinds
        .iter()
        .filter(|(uuid, _)| descriptors.contains_key(*uuid))
        .filter_map(|(uuid, kind)| Some(format!("{uuid}.{}", predefined_suffix(kind)?)))
        .collect::<Vec<_>>();
    let predefined = if wanted.is_empty() {
        BTreeMap::new()
    } else {
        load_predefined(&wanted)
    };
    for (row_name, bytes) in &predefined {
        let Some((owner, suffix)) = row_name.split_once('.') else {
            continue;
        };
        let Some(kind) = plan.kinds.get(owner).copied() else {
            continue;
        };
        if predefined_suffix(kind) != Some(suffix) || bytes.is_empty() {
            continue;
        }
        let Some(owner_name) = plan.names.name(owner).map(str::to_string) else {
            continue;
        };
        if let Ok(body) = parse_row(bytes)
            && let Ok(items) = predefined_items(kind, &body)
        {
            for (uuid, name) in items {
                plan.names.insert_predefined(&owner_name, &uuid, &name);
            }
        }
    }

    // The compatibility mode decides the record versions of the rows: read
    // it off the decoded Configuration row.
    if let Some(row) = &configuration_row {
        let context = ExportContext {
            names: NameIndex::default(),
            version: plan.version.clone(),
            compat: plan.compat,
        };
        // Names are needed to decode default roles; use the plan's own.
        let context = ExportContext {
            names: std::mem::take(&mut plan.names),
            ..context
        };
        match decode_tree("Configuration", row, &context) {
            Ok(element) => {
                if let Some(mode) = element
                    .path(&["Properties", "CompatibilityMode"])
                    .and_then(|mode| Compat::parse(&mode.text))
                {
                    plan.compat = mode;
                }
            }
            Err(error) => errors.push(format!("the Configuration row: {error:#}")),
        }
        plan.names = context.names;
    }
    plan.errors = errors;
    plan
}

/// The name of the property the folded extension compatibility mode goes
/// back into.
const EXTENSION_COMPATIBILITY: &str = "ConfigurationExtensionCompatibilityMode";

/// `decode_object`, and a Configuration row of the shape the model refuses
/// (see `fold_split_compatibility`) read the way the comparison needs: the
/// compatibility mode from the field that holds it, the extension
/// compatibility mode from the field that holds that.
fn decode_tree(kind: &str, tree: &Brace, context: &ExportContext) -> Result<Element> {
    if kind == "Configuration"
        && let Some((folded, extension)) = fold_split_compatibility(tree)
    {
        let mut element = decode_object(kind, &folded, context)?;
        let property = element
            .children
            .iter_mut()
            .find(|child| child.name == "Properties")
            .and_then(|properties| {
                properties
                    .children
                    .iter_mut()
                    .find(|child| child.name == EXTENSION_COMPATIBILITY)
            })
            .ok_or_else(|| anyhow!("the decoded Configuration has no {EXTENSION_COMPATIBILITY}"))?;
        property.text = version_text(extension);
        return Ok(element);
    }
    decode_object(kind, tree, context)
}

/// Decodes stored descriptors into element trees. Both sides of a comparison
/// go through one decoder, so a reference is spelled the same on both.
pub struct Decoder {
    context: ExportContext,
    /// Other compatibility modes a row may be written for. A staged row: the
    /// platform that imported it writes the record format of its own
    /// edition, which is not always the one the active configuration's
    /// compatibility mode stores. A stored row: a native apply promotes the
    /// staged rows into `Config` as they are, so an object that was changed
    /// natively once is stored in that newer format from then on.
    others: Vec<ExportContext>,
}

impl Decoder {
    /// A decoder for a comparison: a row, stored or staged, is read in
    /// `compat` or, when that does not read it, in the first of
    /// `staged_compats` that does.
    pub fn for_comparison(
        names: NameIndex,
        version: &str,
        compat: Compat,
        staged_compats: &[Compat],
    ) -> Self {
        let mut others = Vec::<Compat>::new();
        for other in staged_compats {
            if *other != compat && !others.contains(other) {
                others.push(*other);
            }
        }
        let others = others
            .into_iter()
            .map(|other| ExportContext {
                names: names.clone(),
                version: version.to_string(),
                compat: other,
            })
            .collect();
        Self {
            context: ExportContext {
                names,
                version: version.to_string(),
                compat,
            },
            others,
        }
    }

    /// The text of the file an export writes for a stored descriptor, in the
    /// active compatibility mode or, when the row is stored in a newer record
    /// format, in the first other one that reads it.
    pub fn export(&self, kind: &str, row: &[u8]) -> Result<String> {
        if !has_decoder(kind) {
            return Err(anyhow!("the model has no decoder for {kind}"));
        }
        let first = export_descriptor(kind, row, &self.context);
        if first.is_ok() {
            return first;
        }
        for context in &self.others {
            if let Ok(text) = export_descriptor(kind, row, context) {
                return Ok(text);
            }
        }
        first
    }

    pub fn decode(&self, kind: &str, row: &[u8]) -> Result<Element> {
        self.decode_in(kind, row, &self.context)
    }

    /// A row of the active configuration that a native apply promoted from a
    /// stage: in the record format of the platform that staged it, which the
    /// active compatibility mode may not store.
    pub fn decode_stored(&self, kind: &str, row: &[u8]) -> Result<Element> {
        self.decode_staged(kind, row).map(|(element, _)| element)
    }

    /// A staged row: in the active compatibility mode, else in another one;
    /// the flag says it was another (the row is in a record format the
    /// active configuration does not store).
    pub fn decode_staged(&self, kind: &str, row: &[u8]) -> Result<(Element, bool)> {
        let first = self.decode(kind, row);
        if let Ok(element) = first {
            return Ok((element, false));
        }
        for context in &self.others {
            if let Ok(element) = self.decode_in(kind, row, context) {
                return Ok((element, true));
            }
        }
        first.map(|element| (element, false))
    }

    fn decode_in(&self, kind: &str, row: &[u8], context: &ExportContext) -> Result<Element> {
        if !has_decoder(kind) {
            return Err(anyhow!("the model has no decoder for {kind}"));
        }
        let tree = parse_row(row).with_context(|| format!("failed to read the {kind} row"))?;
        decode_tree(kind, &tree, context)
    }
}
