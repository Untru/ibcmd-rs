//! Base-free writer for a role's `Ext/Rights.xml`.
//!
//! Compiles the native XML into the stored Role rights row (`<role uuid>.0`,
//! raw-deflated brace text starting `{10,`) without reading a base blob. It
//! is the inverse of the exporter's `mssql_dump::role_rights` decoder, and
//! it writes what the platform writes when it loads a configuration from XML
//! files. Every rule below was measured on the two 8.3.27 corpora (БСП демо,
//! 160 roles; ERP УХ 3.2.12.6, 2,114 roles), comparing the stored rows with
//! the native `Rights.xml` beside them (2026-09-23):
//!
//! * **Rights of an object** are the ones the XML names, in XML order, `1`
//!   for true and `-1` for false. БСП stores exactly that on 2,744 of 2,745
//!   objects; the one exception is the Configuration root below. (ERP УХ was
//!   saved by Designer, which writes an object's whole right list with `-1`
//!   for every right not granted -- a list the XML cannot carry, since the
//!   export hides a right equal to `setForNewObjects`.)
//! * **The Configuration root** always carries the six launch-mode rights
//!   (`MainWindowMode*`, `AnalyticsSystemClient`), whose type default is
//!   `true`: the full-rights role, whose XML omits them because they equal
//!   its `setForNewObjects`, stores all six as `1` at their place in the
//!   platform's right order. A missing one is written with the value that
//!   makes the export omit it again: the role's `setForNewObjects`.
//! * **Object references** are `{1,<uuid>,0,1}` for a field (attribute,
//!   dimension, resource, tabular section and its attributes, accounting
//!   flag, addressing attribute) and `{1,<uuid>,0,0}` for everything else
//!   (commands, operations, methods, channels, subsystems, recalculations,
//!   top-level objects); 0 exceptions over the 319,452 such references of
//!   both corpora that name a live object. A standard attribute is
//!   `{1,<owner>,1,\r\n{<slot>,<family>},1}`, a nested one
//!   `{1,<owner>,2,\r\n{<slot>,<family>},\r\n{<slot>,<family>},1}`, with the
//!   exporter's slot tables read backwards.
//! * **Object order** is the iteration order of a `boost::unordered_map`
//!   (the 64-bit `mix64_policy`: power-of-two buckets, 16 to start, load
//!   factor 1, growth to `max(size, 1.5 * size)`, a stable rehash that moves
//!   a node to the front of its bucket's group) keyed by the object uuid and
//!   hashed by its first eight hex digits, the objects inserted in XML order.
//!   It reproduces the stored order of 160/160 БСП roles and 1,880/2,114 ERP
//!   УХ roles (the rest carry Designer's edit history).
//! * **Restrictions** of an object are sorted by right uuid (154/154 БСП and
//!   4,395/4,395 ERP УХ objects), each `{<right>,\r\n{<n>,\r\n<block>…\r\n}\r\n}`
//!   with a block `{1,"<condition>",0}` or `{1,"<condition>",1,\r\n<field>\r\n}`.
//! * **Text** is stored with every line break the XML carries as `\n` written
//!   `\r\n`, which inverts the export's single `\r\n` -> `\n` replacement: no
//!   stored condition on either corpus holds a bare `\n` or a bare `\r`.
//! * **Flags**: `setForNewObjects` and `setForAttributesByDefault` are `1` or
//!   `4294967295`, `independentRightsOfChildObjects` is `1` or `0`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{self, Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use quick_xml::Reader;
use quick_xml::escape::resolve_xml_entity;
use quick_xml::events::Event;

use crate::mssql_dump::{
    CONFIGURATION_MODE_RIGHT_NAMES, ROLE_RIGHTS_EXT_DIMENSION_FAMILY,
    ROLE_RIGHTS_EXT_DIMENSION_TYPE_FAMILY, ROLE_RIGHTS_STANDARD_ATTRIBUTE_FAMILY,
    ROLE_RIGHTS_STANDARD_TABULAR_SECTION_FAMILY, role_right_uuid, role_standard_attribute_slot,
    role_standard_tabular_section_attribute_slot, role_standard_tabular_section_slot,
};

/// Why a role was not written. `class` is a stable key an audit can count
/// by; `detail` names the object or the construct.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoleRightsRefusal {
    pub class: &'static str,
    pub detail: String,
}

impl RoleRightsRefusal {
    fn new(class: &'static str, detail: impl Into<String>) -> Self {
        Self {
            class,
            detail: detail.into(),
        }
    }
}

impl Display for RoleRightsRefusal {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.class, self.detail)
    }
}

impl std::error::Error for RoleRightsRefusal {}

/// Where the writer looks metadata names up.
pub trait RoleRightsSource {
    /// The uuid of the metadata object `reference` names: a top-level object
    /// (`Catalog.X`), the configuration (`Configuration.X`), an object kept in
    /// a file of its own below its owner (`Subsystem.A.Subsystem.B`,
    /// `CalculationRegister.R.Recalculation.X`), or a child its owner's file
    /// declares (`Catalog.X.TabularSection.T.Attribute.A`, `.Command.C`, …).
    fn metadata_object_uuid(&self, reference: &str) -> Result<String, String>;

    /// The uuid of the field `object` declares directly under `field` (an
    /// attribute, dimension or resource), or `None` when it declares none.
    fn declared_field_uuid(&self, object: &str, field: &str) -> Result<Option<String>, String>;
}

/// A parsed `Rights.xml`, text exactly as the file spells it (entities
/// resolved, line breaks untouched).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RightsDocument {
    pub set_for_new_objects: bool,
    pub set_for_attributes_by_default: bool,
    pub independent_rights_of_child_objects: bool,
    pub objects: Vec<RightsObject>,
    pub templates: Vec<RightsTemplate>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RightsObject {
    pub name: String,
    pub rights: Vec<RightsRight>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RightsRight {
    pub name: String,
    pub value: bool,
    pub restrictions: Vec<RightsRestriction>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RightsRestriction {
    pub field: Option<String>,
    pub condition: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RightsTemplate {
    pub name: String,
    pub condition: String,
}

/// A written row: the plain text (with its BOM) and the names the exporter
/// needs to read it back -- the metadata name of every object uuid it
/// references and the name of every restriction field uuid.
#[derive(Clone, Debug)]
pub struct WrittenRoleRights {
    pub plain: Vec<u8>,
    pub object_refs: BTreeMap<String, String>,
    pub field_refs: BTreeMap<String, String>,
}

/// The platform's order of the Configuration root's rights, as Designer
/// writes the whole list (ERP УХ, 251 roles carry it complete). Used only to
/// place a launch-mode right the XML omits; the uuid `4df6d046-…` that sits
/// between `AnalyticsSystemClient` and `SaveUserData` there has no name and
/// is never written from XML (БСП's full-rights role stores none).
const CONFIGURATION_RIGHT_ORDER: [&str; 25] = [
    "Administration",
    "DataAdministration",
    "UpdateDataBaseConfiguration",
    "ExclusiveMode",
    "ActiveUsers",
    "EventLog",
    "ThinClient",
    "WebClient",
    "MobileClient",
    "ThickClient",
    "ExternalConnection",
    "Automation",
    "TechnicalSpecialistMode",
    "CollaborationSystemInfoBaseRegistration",
    "MainWindowModeNormal",
    "MainWindowModeWorkplace",
    "MainWindowModeEmbeddedWorkplace",
    "MainWindowModeFullscreenWorkplace",
    "MainWindowModeKiosk",
    "AnalyticsSystemClient",
    "SaveUserData",
    "ConfigurationExtensionsAdministration",
    "InteractiveOpenExtDataProcessors",
    "InteractiveOpenExtReports",
    "Output",
];

/// Compiles a `Rights.xml` into the stored row's plain text.
pub fn write_role_rights(
    xml: &[u8],
    source: &dyn RoleRightsSource,
) -> Result<WrittenRoleRights, RoleRightsRefusal> {
    let document = parse_rights_xml(xml)?;
    write_role_rights_document(&document, source)
}

/// Compiles an already parsed `Rights.xml`.
pub fn write_role_rights_document(
    document: &RightsDocument,
    source: &dyn RoleRightsSource,
) -> Result<WrittenRoleRights, RoleRightsRefusal> {
    let mut names = HashSet::new();
    let mut object_refs = BTreeMap::<String, String>::new();
    let mut field_refs = BTreeMap::<String, String>::new();
    let mut entries = Vec::with_capacity(document.objects.len());
    let mut hashes = Vec::with_capacity(document.objects.len());
    for object in &document.objects {
        if !names.insert(object.name.as_str()) {
            return Err(RoleRightsRefusal::new(
                "duplicate object",
                object.name.clone(),
            ));
        }
        let reference = resolve_object(&object.name, source)?;
        match object_refs.get(&reference.uuid) {
            Some(existing) if existing != &reference.base_name => {
                return Err(RoleRightsRefusal::new(
                    "two names for one uuid",
                    format!("{existing} and {}", reference.base_name),
                ));
            }
            Some(_) => {}
            None => {
                object_refs.insert(reference.uuid.clone(), reference.base_name.clone());
            }
        }
        let rights = write_object_rights(document, object, source, &mut field_refs)?;
        hashes.push(uuid_first_group(&reference.uuid)?);
        entries.push(format!("{{\r\n{},\r\n{}\r\n}}", reference.text(), rights));
    }

    let mut plain = String::with_capacity(entries.iter().map(String::len).sum::<usize>() + 256);
    plain.push('\u{feff}');
    plain.push_str("{10,\r\n");
    if entries.is_empty() {
        plain.push_str("{0}");
    } else {
        plain.push('{');
        plain.push_str(&entries.len().to_string());
        for index in boost_unordered_iteration_order(&hashes) {
            plain.push_str(",\r\n");
            plain.push_str(&entries[index]);
        }
        plain.push_str("\r\n}");
    }
    plain.push_str(",\r\n");
    if document.templates.is_empty() {
        plain.push_str("{0}");
    } else {
        plain.push('{');
        plain.push_str(&document.templates.len().to_string());
        for template in &document.templates {
            if template.name.is_empty() {
                return Err(RoleRightsRefusal::new(
                    "restriction template without a name",
                    String::new(),
                ));
            }
            plain.push_str(",\r\n{");
            push_brace_text(&mut plain, &template.name);
            plain.push(',');
            push_brace_text(&mut plain, &stored_text(&template.condition));
            plain.push('}');
        }
        plain.push_str("\r\n}");
    }
    plain.push(',');
    plain.push_str(if document.set_for_new_objects {
        "1"
    } else {
        "4294967295"
    });
    plain.push(',');
    plain.push_str(if document.set_for_attributes_by_default {
        "1"
    } else {
        "4294967295"
    });
    plain.push(',');
    plain.push_str(if document.independent_rights_of_child_objects {
        "1"
    } else {
        "0"
    });
    plain.push_str(",4294967295}");

    Ok(WrittenRoleRights {
        plain: plain.into_bytes(),
        object_refs,
        field_refs,
    })
}

/// One object's rights table: `{0,<uuid>,<value>,…}`, or with restrictions
/// `{1,<count>,<uuid>,<value>,…,<restricted>,\r\n<entry>,…\r\n}`.
fn write_object_rights(
    document: &RightsDocument,
    object: &RightsObject,
    source: &dyn RoleRightsSource,
    field_refs: &mut BTreeMap<String, String>,
) -> Result<String, RoleRightsRefusal> {
    if object.rights.is_empty() {
        return Err(RoleRightsRefusal::new(
            "object without rights",
            object.name.clone(),
        ));
    }
    let mut seen = HashSet::new();
    let mut rights = Vec::with_capacity(object.rights.len() + 6);
    for right in &object.rights {
        let uuid = role_right_uuid(&right.name).ok_or_else(|| {
            RoleRightsRefusal::new(
                "unknown right",
                format!("{} on {}", right.name, object.name),
            )
        })?;
        if !seen.insert(uuid) {
            return Err(RoleRightsRefusal::new(
                "duplicate right",
                format!("{} on {}", right.name, object.name),
            ));
        }
        rights.push((
            right.name.as_str(),
            uuid,
            right.value,
            right.restrictions.as_slice(),
        ));
    }
    if is_configuration_root(&object.name) {
        if rights
            .iter()
            .any(|(_, _, _, restrictions)| !restrictions.is_empty())
        {
            // The exporter reads the root's rights as plain pairs only.
            return Err(RoleRightsRefusal::new(
                "restricted Configuration right",
                object.name.clone(),
            ));
        }
        rights = with_configuration_mode_rights(rights, document.set_for_new_objects).map_err(
            |detail| {
                RoleRightsRefusal::new(
                    "Configuration rights out of the platform order",
                    format!("{}: {detail}", object.name),
                )
            },
        )?;
    }

    let mut pairs = String::with_capacity(rights.len() * 40);
    for (_, uuid, value, _) in &rights {
        pairs.push(',');
        pairs.push_str(uuid);
        pairs.push(',');
        pairs.push_str(if *value { "1" } else { "-1" });
    }
    let mut restricted = rights
        .iter()
        .filter(|(_, _, _, restrictions)| !restrictions.is_empty())
        .collect::<Vec<_>>();
    if restricted.is_empty() {
        return Ok(format!("{{0{pairs}}}"));
    }
    restricted.sort_by_key(|(_, uuid, _, _)| *uuid);
    let mut text = format!("{{1,{}{pairs},{}", rights.len(), restricted.len());
    for (_, uuid, _, restrictions) in restricted {
        // A right's condition on the whole record comes first even when it
        // is empty: a right the XML restricts by field only is stored with
        // an empty field-less block ahead of its field blocks, which the
        // export then skips. All 7 field restrictions of both corpora
        // follow a field-less block; the one whose XML has none
        // (`Roles/ЧтениеИнформацииОВерсияхОбъектов`, in БСП and ERP УХ
        // alike) stores it as `{1,"",0}`.
        let whole_record_missing = restrictions
            .iter()
            .all(|restriction| restriction.field.is_some());
        text.push_str(",\r\n{");
        text.push_str(uuid);
        text.push_str(",\r\n{");
        text.push_str(&(restrictions.len() + usize::from(whole_record_missing)).to_string());
        if whole_record_missing {
            text.push_str(",\r\n{1,\"\",0}");
        }
        for restriction in *restrictions {
            text.push_str(",\r\n{1,");
            push_brace_text(&mut text, &stored_text(&restriction.condition));
            match &restriction.field {
                None => text.push_str(",0}"),
                Some(field) => {
                    text.push_str(",1,\r\n");
                    text.push_str(&restriction_field(&object.name, field, source, field_refs)?);
                    text.push_str("\r\n}");
                }
            }
        }
        text.push_str("\r\n}\r\n}");
    }
    text.push_str("\r\n}");
    Ok(text)
}

/// A restriction's `<field>`: a field the object declares, by uuid
/// (`{\r\n{0},\r\n{0,<uuid>}\r\n}`), or one of its standard attributes, by
/// slot (`{\r\n{0},\r\n{<slot>}\r\n}`) -- the two payloads the stores carry.
fn restriction_field(
    object: &str,
    field: &str,
    source: &dyn RoleRightsSource,
    field_refs: &mut BTreeMap<String, String>,
) -> Result<String, RoleRightsRefusal> {
    if let Some(uuid) = source
        .declared_field_uuid(object, field)
        .map_err(|detail| RoleRightsRefusal::new("unresolved restriction field", detail))?
    {
        if let Some(existing) = field_refs.get(&uuid)
            && existing != field
        {
            return Err(RoleRightsRefusal::new(
                "two names for one field uuid",
                format!("{existing} and {field}"),
            ));
        }
        field_refs.insert(uuid.clone(), field.to_string());
        return Ok(format!("{{\r\n{{0}},\r\n{{0,{uuid}}}\r\n}}"));
    }
    let kind = object.split('.').next().unwrap_or_default();
    if object.matches('.').count() == 1
        && let Some((slot, _)) = role_standard_attribute_slot(kind, field)
    {
        return Ok(format!("{{\r\n{{0}},\r\n{{{slot}}}\r\n}}"));
    }
    Err(RoleRightsRefusal::new(
        "unresolved restriction field",
        format!("{field} of {object}"),
    ))
}

fn is_configuration_root(name: &str) -> bool {
    name.starts_with("Configuration.") && name.matches('.').count() == 1
}

type RightEntry<'a> = (&'a str, &'static str, bool, &'a [RightsRestriction]);

/// Adds the launch-mode rights the XML omits, at the place the platform's
/// order gives them, valued so that the export omits them again.
fn with_configuration_mode_rights<'a>(
    rights: Vec<RightEntry<'a>>,
    set_for_new_objects: bool,
) -> Result<Vec<RightEntry<'a>>, String> {
    let missing = CONFIGURATION_MODE_RIGHT_NAMES
        .iter()
        .filter(|mode| !rights.iter().any(|(name, _, _, _)| name == *mode))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(rights);
    }
    let rank = |name: &str| {
        CONFIGURATION_RIGHT_ORDER
            .iter()
            .position(|known| *known == name)
    };
    let mut previous = None;
    for (name, _, _, _) in &rights {
        let position = rank(name).ok_or_else(|| format!("{name} has no known place"))?;
        if previous.is_some_and(|previous| previous >= position) {
            return Err(format!("{name} is out of order"));
        }
        previous = Some(position);
    }
    let mut merged = rights;
    for mode in missing {
        let uuid = role_right_uuid(mode).ok_or_else(|| format!("{mode} has no uuid"))?;
        merged.push((mode, uuid, set_for_new_objects, &[]));
    }
    merged.sort_by_key(|(name, _, _, _)| rank(name));
    Ok(merged)
}

/// A resolved object reference.
struct ResolvedObject {
    uuid: String,
    tail: ReferenceTail,
    /// The name the exporter looks the uuid up under: the object's own name,
    /// or its owner's for a standard attribute.
    base_name: String,
}

enum ReferenceTail {
    Plain { field: bool },
    Nested(Vec<(isize, &'static str)>),
}

impl ResolvedObject {
    fn text(&self) -> String {
        match &self.tail {
            ReferenceTail::Plain { field } => {
                format!("{{1,{},0,{}}}", self.uuid, if *field { 1 } else { 0 })
            }
            ReferenceTail::Nested(slots) => {
                let mut text = format!("{{1,{},{}", self.uuid, slots.len());
                for (slot, family) in slots {
                    text.push_str(&format!(",\r\n{{{slot},{family}}}"));
                }
                text.push_str(",1}");
                text
            }
        }
    }
}

/// Field-like children: the reference's trailing member is `1` for these.
const FIELD_CATEGORIES: [&str; 7] = [
    "Attribute",
    "Dimension",
    "Resource",
    "TabularSection",
    "AccountingFlag",
    "ExtDimensionAccountingFlag",
    "AddressingAttribute",
];

/// Children whose reference ends `0,0`, like a top-level object's.
const PLAIN_CATEGORIES: [&str; 6] = [
    "Command",
    "Operation",
    "Method",
    "IntegrationServiceChannel",
    "Subsystem",
    "Recalculation",
];

fn resolve_object(
    name: &str,
    source: &dyn RoleRightsSource,
) -> Result<ResolvedObject, RoleRightsRefusal> {
    let parts = name.split('.').collect::<Vec<_>>();
    if parts.len() < 2 || parts.len() % 2 != 0 || parts.iter().any(|part| part.is_empty()) {
        return Err(RoleRightsRefusal::new("malformed object name", name));
    }
    let kind = parts[0];
    let owner = format!("{}.{}", parts[0], parts[1]);
    let unresolved = |detail: String| RoleRightsRefusal::new("unresolved object", detail);
    match parts.get(2).copied() {
        Some("StandardAttribute") => {
            if parts.len() != 4 {
                return Err(RoleRightsRefusal::new("malformed object name", name));
            }
            let attribute = parts[3];
            let slots = if kind == "AccountingRegister"
                && let Some((index, family)) = ext_dimension(attribute)
            {
                vec![
                    (index, ROLE_RIGHTS_STANDARD_ATTRIBUTE_FAMILY),
                    (index, family),
                ]
            } else {
                let (slot, _) = role_standard_attribute_slot(kind, attribute)
                    .ok_or_else(|| RoleRightsRefusal::new("unknown standard attribute", name))?;
                vec![(slot, ROLE_RIGHTS_STANDARD_ATTRIBUTE_FAMILY)]
            };
            Ok(ResolvedObject {
                uuid: source.metadata_object_uuid(&owner).map_err(unresolved)?,
                tail: ReferenceTail::Nested(slots),
                base_name: owner,
            })
        }
        Some("StandardTabularSection") => {
            let (section, _) = role_standard_tabular_section_slot(kind, parts[3])
                .ok_or_else(|| RoleRightsRefusal::new("unknown standard tabular section", name))?;
            let mut slots = vec![(section, ROLE_RIGHTS_STANDARD_TABULAR_SECTION_FAMILY)];
            match parts.len() {
                4 => {}
                6 if parts[4] == "StandardAttribute" => {
                    let (attribute, _) = role_standard_tabular_section_attribute_slot(
                        kind, parts[5],
                    )
                    .ok_or_else(|| RoleRightsRefusal::new("unknown standard attribute", name))?;
                    slots.push((attribute, ROLE_RIGHTS_STANDARD_ATTRIBUTE_FAMILY));
                }
                _ => return Err(RoleRightsRefusal::new("malformed object name", name)),
            }
            Ok(ResolvedObject {
                uuid: source.metadata_object_uuid(&owner).map_err(unresolved)?,
                tail: ReferenceTail::Nested(slots),
                base_name: owner,
            })
        }
        _ => {
            let field = if parts.len() == 2 {
                false
            } else {
                let category = parts[parts.len() - 2];
                if FIELD_CATEGORIES.contains(&category) {
                    true
                } else if PLAIN_CATEGORIES.contains(&category) {
                    false
                } else {
                    return Err(RoleRightsRefusal::new(
                        "unsupported object category",
                        format!("{category} in {name}"),
                    ));
                }
            };
            Ok(ResolvedObject {
                uuid: source.metadata_object_uuid(name).map_err(unresolved)?,
                tail: ReferenceTail::Plain { field },
                base_name: name.to_string(),
            })
        }
    }
}

/// An accounting register's `ExtDimension<n>` / `ExtDimensionType<n>`: the
/// zero-based index and the family of the inner slot.
fn ext_dimension(attribute: &str) -> Option<(isize, &'static str)> {
    let (digits, family) = if let Some(digits) = attribute.strip_prefix("ExtDimensionType") {
        (digits, ROLE_RIGHTS_EXT_DIMENSION_TYPE_FAMILY)
    } else {
        (
            attribute.strip_prefix("ExtDimension")?,
            ROLE_RIGHTS_EXT_DIMENSION_FAMILY,
        )
    };
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || digits.starts_with('0')
    {
        return None;
    }
    let number = digits.parse::<isize>().ok()?;
    Some((number - 1, family))
}

fn uuid_first_group(uuid: &str) -> Result<u64, RoleRightsRefusal> {
    uuid.get(..8)
        .and_then(|group| u32::from_str_radix(group, 16).ok())
        .map(u64::from)
        .ok_or_else(|| RoleRightsRefusal::new("unresolved object", format!("bad uuid {uuid}")))
}

/// Text as the row stores it: the export writes a stored `\r\n` as `\n`, so
/// every `\n` the XML carries goes back as `\r\n`.
fn stored_text(text: &str) -> String {
    text.replace('\n', "\r\n")
}

fn push_brace_text(output: &mut String, text: &str) {
    output.push('"');
    for ch in text.chars() {
        output.push(ch);
        if ch == '"' {
            output.push('"');
        }
    }
    output.push('"');
}

/// The order a `boost::unordered_map` (1.5x-1.79, 64-bit `mix64_policy`)
/// iterates its nodes in, after inserting keys with these hashes in order.
///
/// Nodes sit in one singly linked list; a bucket stores the node *before*
/// its first node. A node for an empty bucket goes to the front of the list,
/// one for an occupied bucket to the front of that bucket's group. A rehash
/// walks the list once and leaves a node where it is when its new bucket is
/// still empty, else moves it to the front of that bucket's group.
fn boost_unordered_iteration_order(hashes: &[u64]) -> Vec<usize> {
    const NONE: usize = usize::MAX;
    let count = hashes.len();
    let sentinel = count;
    let mixed = hashes.iter().map(|hash| mix64(*hash)).collect::<Vec<_>>();
    let mut next = vec![NONE; count + 1];
    let mut bucket_count = boost_bucket_count(11);
    let mut buckets = Vec::new();
    let mut size = 0usize;
    for node in 0..count {
        let new_size = size + 1;
        if buckets.is_empty() {
            bucket_count = bucket_count.max(boost_bucket_count(new_size + 1));
            buckets = vec![NONE; bucket_count];
        } else if new_size > bucket_count {
            let wanted = boost_bucket_count(new_size.max(size + (size >> 1)) + 1);
            if wanted != bucket_count {
                bucket_count = wanted;
                buckets = vec![NONE; bucket_count];
                let mut previous = sentinel;
                while next[previous] != NONE {
                    let current = next[previous];
                    let bucket = (mixed[current] as usize) & (bucket_count - 1);
                    if buckets[bucket] == NONE {
                        buckets[bucket] = previous;
                        previous = current;
                    } else {
                        next[previous] = next[current];
                        let before = buckets[bucket];
                        next[current] = next[before];
                        next[before] = current;
                    }
                }
            }
        }
        let bucket = (mixed[node] as usize) & (bucket_count - 1);
        if buckets[bucket] == NONE {
            let first = next[sentinel];
            if first != NONE {
                buckets[(mixed[first] as usize) & (bucket_count - 1)] = node;
            }
            buckets[bucket] = sentinel;
            next[node] = first;
            next[sentinel] = node;
        } else {
            let before = buckets[bucket];
            next[node] = next[before];
            next[before] = node;
        }
        size = new_size;
    }
    let mut order = Vec::with_capacity(count);
    let mut current = next[sentinel];
    while current != NONE {
        order.push(current);
        current = next[current];
    }
    order
}

/// `mix64_policy::new_bucket_count`: the next power of two, at least 4.
fn boost_bucket_count(minimum: usize) -> usize {
    minimum.max(4).next_power_of_two()
}

/// `mix64_policy::apply_hash`.
fn mix64(key: u64) -> u64 {
    let mut key = (!key).wrapping_add(key << 21);
    key ^= key >> 24;
    key = key.wrapping_add(key << 3).wrapping_add(key << 8);
    key ^= key >> 14;
    key = key.wrapping_add(key << 2).wrapping_add(key << 4);
    key ^= key >> 28;
    key.wrapping_add(key << 31)
}

/// Reads a `Rights.xml` exactly: only the elements the format has, text
/// with its line breaks as the file spells them (quick-xml's `decode`, not
/// the end-of-line normalising `xml_content`), entities resolved.
pub fn parse_rights_xml(xml: &[u8]) -> Result<RightsDocument, RoleRightsRefusal> {
    let refuse = |detail: String| RoleRightsRefusal::new("unreadable Rights.xml", detail);
    let xml = xml.strip_prefix(b"\xef\xbb\xbf").unwrap_or(xml);
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    let mut path = Vec::<String>::new();
    let mut text = String::new();
    let mut flags = [None::<bool>; 3];
    let mut document = RightsDocument::default();
    let mut object = None::<RightsObject>;
    let mut right = None::<RightsRight>;
    let mut value_seen = false;
    let mut restriction = None::<(Option<String>, Option<String>)>;
    let mut template = None::<(Option<String>, Option<String>)>;

    loop {
        let event = reader
            .read_event_into(&mut buffer)
            .map_err(|error| refuse(error.to_string()))?;
        let (local, closes) = match &event {
            Event::Start(start) => (Some(local_name(start.local_name().as_ref())), false),
            Event::Empty(start) => (Some(local_name(start.local_name().as_ref())), true),
            Event::End(_) => (None, true),
            Event::Text(chunk) => {
                text.push_str(&chunk.decode().map_err(|error| refuse(error.to_string()))?);
                buffer.clear();
                continue;
            }
            Event::CData(chunk) => {
                text.push_str(&chunk.decode().map_err(|error| refuse(error.to_string()))?);
                buffer.clear();
                continue;
            }
            Event::GeneralRef(reference) => {
                if let Some(ch) = reference
                    .resolve_char_ref()
                    .map_err(|error| refuse(error.to_string()))?
                {
                    text.push(ch);
                } else {
                    let name = reference
                        .decode()
                        .map_err(|error| refuse(error.to_string()))?;
                    text.push_str(
                        resolve_xml_entity(&name)
                            .ok_or_else(|| refuse(format!("unknown entity &{name};")))?,
                    );
                }
                buffer.clear();
                continue;
            }
            Event::Eof => break,
            _ => {
                buffer.clear();
                continue;
            }
        };
        if let Some(local) = local {
            // An element opens: whatever text came before it was layout.
            if !text.trim().is_empty() {
                return Err(refuse(format!("text before <{local}>")));
            }
            text.clear();
            let parent = path.iter().map(String::as_str).collect::<Vec<_>>();
            match (parent.as_slice(), local.as_str()) {
                ([], "Rights") => {}
                (
                    ["Rights"],
                    "setForNewObjects"
                    | "setForAttributesByDefault"
                    | "independentRightsOfChildObjects",
                ) => {}
                (["Rights"], "object") => {
                    object = Some(RightsObject::default());
                }
                (["Rights"], "restrictionTemplate") => template = Some((None, None)),
                (["Rights", "object"], "name") => {}
                (["Rights", "object"], "right") => {
                    right = Some(RightsRight::default());
                    value_seen = false;
                }
                (["Rights", "object", "right"], "name" | "value") => {}
                (["Rights", "object", "right"], "restrictionByCondition") => {
                    restriction = Some((None, None));
                }
                (
                    ["Rights", "object", "right", "restrictionByCondition"],
                    "field" | "condition",
                ) => {}
                (["Rights", "restrictionTemplate"], "name" | "condition") => {}
                _ => {
                    return Err(refuse(format!(
                        "unexpected <{local}> in {}",
                        if parent.is_empty() {
                            "the document".to_string()
                        } else {
                            parent.join("/")
                        }
                    )));
                }
            }
            path.push(local);
            if !closes {
                buffer.clear();
                continue;
            }
        }
        // An element closes (an `Empty` one closes right after it opens).
        let Some(local) = path.pop() else {
            return Err(refuse("unbalanced document".to_string()));
        };
        let parent = path.iter().map(String::as_str).collect::<Vec<_>>();
        let value = std::mem::take(&mut text);
        let leaf = matches!(
            local.as_str(),
            "setForNewObjects"
                | "setForAttributesByDefault"
                | "independentRightsOfChildObjects"
                | "name"
                | "value"
                | "field"
                | "condition"
        );
        if !leaf && !value.trim().is_empty() {
            return Err(refuse(format!("text in <{local}>")));
        }
        match (parent.as_slice(), local.as_str()) {
            (
                ["Rights"],
                flag @ ("setForNewObjects"
                | "setForAttributesByDefault"
                | "independentRightsOfChildObjects"),
            ) => {
                let index = match flag {
                    "setForNewObjects" => 0,
                    "setForAttributesByDefault" => 1,
                    _ => 2,
                };
                if flags[index]
                    .replace(
                        parse_bool(&value).ok_or_else(|| refuse(format!("{flag} is {value}")))?,
                    )
                    .is_some()
                {
                    return Err(refuse(format!("{flag} twice")));
                }
            }
            (["Rights", "object"], "name") => {
                let object = object
                    .as_mut()
                    .ok_or_else(|| refuse("name outside an object".into()))?;
                if !object.name.is_empty() || value.is_empty() {
                    return Err(refuse("object name repeated or empty".into()));
                }
                object.name = value;
            }
            (["Rights", "object", "right"], "name") => {
                let right = right
                    .as_mut()
                    .ok_or_else(|| refuse("name outside a right".into()))?;
                if !right.name.is_empty() || value.is_empty() {
                    return Err(refuse("right name repeated or empty".into()));
                }
                right.name = value;
            }
            (["Rights", "object", "right"], "value") => {
                let right = right
                    .as_mut()
                    .ok_or_else(|| refuse("value outside a right".into()))?;
                if value_seen {
                    return Err(refuse("right value repeated".into()));
                }
                right.value =
                    parse_bool(&value).ok_or_else(|| refuse(format!("right value {value}")))?;
                value_seen = true;
            }
            (["Rights", "object", "right", "restrictionByCondition"], "field") => {
                let restriction = restriction
                    .as_mut()
                    .ok_or_else(|| refuse("field outside a restriction".into()))?;
                if restriction.0.replace(value).is_some() {
                    return Err(refuse("restriction field repeated".into()));
                }
            }
            (["Rights", "object", "right", "restrictionByCondition"], "condition") => {
                let restriction = restriction
                    .as_mut()
                    .ok_or_else(|| refuse("condition outside a restriction".into()))?;
                if restriction.1.replace(value).is_some() {
                    return Err(refuse("restriction condition repeated".into()));
                }
            }
            (["Rights", "object", "right"], "restrictionByCondition") => {
                let (field, condition) = restriction
                    .take()
                    .ok_or_else(|| refuse("unbalanced restriction".into()))?;
                let condition =
                    condition.ok_or_else(|| refuse("restriction without a condition".into()))?;
                right
                    .as_mut()
                    .ok_or_else(|| refuse("restriction outside a right".into()))?
                    .restrictions
                    .push(RightsRestriction { field, condition });
            }
            (["Rights", "object"], "right") => {
                let right = right
                    .take()
                    .ok_or_else(|| refuse("unbalanced right".into()))?;
                if right.name.is_empty() || !value_seen {
                    return Err(refuse("right without a name or a value".into()));
                }
                object
                    .as_mut()
                    .ok_or_else(|| refuse("right outside an object".into()))?
                    .rights
                    .push(right);
            }
            (["Rights"], "object") => {
                let object = object
                    .take()
                    .ok_or_else(|| refuse("unbalanced object".into()))?;
                if object.name.is_empty() {
                    return Err(refuse("object without a name".into()));
                }
                document.objects.push(object);
            }
            (["Rights", "restrictionTemplate"], "name") => {
                let template = template
                    .as_mut()
                    .ok_or_else(|| refuse("name outside a template".into()))?;
                if template.0.replace(value).is_some() {
                    return Err(refuse("template name repeated".into()));
                }
            }
            (["Rights", "restrictionTemplate"], "condition") => {
                let template = template
                    .as_mut()
                    .ok_or_else(|| refuse("condition outside a template".into()))?;
                if template.1.replace(value).is_some() {
                    return Err(refuse("template condition repeated".into()));
                }
            }
            (["Rights"], "restrictionTemplate") => {
                let (name, condition) = template
                    .take()
                    .ok_or_else(|| refuse("unbalanced template".into()))?;
                document.templates.push(RightsTemplate {
                    name: name.ok_or_else(|| refuse("template without a name".into()))?,
                    condition: condition
                        .ok_or_else(|| refuse("template without a condition".into()))?,
                });
            }
            _ => {}
        }
        buffer.clear();
    }
    if !path.is_empty() {
        return Err(refuse("document ends inside an element".into()));
    }
    document.set_for_new_objects = flags[0].unwrap_or(false);
    document.set_for_attributes_by_default = flags[1].unwrap_or(true);
    document.independent_rights_of_child_objects = flags[2].unwrap_or(false);
    Ok(document)
}

fn local_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

/// Metadata names looked up in a native XML source tree, each file read once.
///
/// A file is parsed into the tree of its uuid-bearing elements, each with the
/// `<Properties><Name>` it declares; a child path (`TabularSection.T.
/// Attribute.A`) is walked down that tree. Top-level objects, the
/// configuration, nested subsystems and recalculations are files of their
/// own.
#[derive(Debug)]
pub struct SourceTreeRoleRightsSource {
    root: PathBuf,
    files: Mutex<HashMap<PathBuf, Option<Arc<UuidNode>>>>,
}

#[derive(Debug)]
struct UuidNode {
    tag: String,
    uuid: String,
    name: String,
    children: Vec<UuidNode>,
}

impl SourceTreeRoleRightsSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            files: Mutex::new(HashMap::new()),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn file(&self, path: PathBuf) -> Option<Arc<UuidNode>> {
        if let Ok(files) = self.files.lock()
            && let Some(found) = files.get(&path)
        {
            return found.clone();
        }
        let parsed = std::fs::read(&path)
            .ok()
            .and_then(|xml| parse_uuid_tree(&xml))
            .map(Arc::new);
        if let Ok(mut files) = self.files.lock() {
            files.insert(path, parsed.clone());
        }
        parsed
    }

    /// The node an object's own file declares, and the remaining child path.
    fn object_node<'a>(
        &self,
        parts: &'a [&'a str],
    ) -> Result<(Arc<UuidNode>, &'a [&'a str]), String> {
        let kind = parts[0];
        let (path, consumed) = if kind == "Configuration" {
            (self.root.join("Configuration.xml"), 2)
        } else if kind == "Subsystem" {
            // Subsystem.A.Subsystem.B… lives at Subsystems/A/Subsystems/B.xml.
            let mut consumed = 2;
            while parts.get(consumed) == Some(&"Subsystem") && parts.len() > consumed + 1 {
                consumed += 2;
            }
            let mut path = self.root.join("Subsystems");
            let names = parts[..consumed]
                .iter()
                .skip(1)
                .step_by(2)
                .collect::<Vec<_>>();
            for (index, name) in names.iter().enumerate() {
                if index + 1 == names.len() {
                    path = path.join(format!("{name}.xml"));
                } else {
                    path = path.join(name).join("Subsystems");
                }
            }
            (path, consumed)
        } else if kind == "CalculationRegister"
            && parts.get(2) == Some(&"Recalculation")
            && parts.len() == 4
        {
            (
                self.root
                    .join("CalculationRegisters")
                    .join(parts[1])
                    .join("Recalculations")
                    .join(format!("{}.xml", parts[3])),
                4,
            )
        } else {
            let folder = metadata_family_folder(kind)
                .ok_or_else(|| format!("no source folder for {kind}"))?;
            (self.root.join(folder).join(format!("{}.xml", parts[1])), 2)
        };
        let node = self
            .file(path.clone())
            .ok_or_else(|| format!("no readable metadata file {}", path.display()))?;
        let expected_tag = parts[consumed - 2];
        let expected_name = parts[consumed - 1];
        if node.tag != expected_tag || node.name != expected_name {
            return Err(format!(
                "{} declares {}.{}, not {expected_tag}.{expected_name}",
                path.display(),
                node.tag,
                node.name
            ));
        }
        Ok((node, &parts[consumed..]))
    }
}

impl RoleRightsSource for SourceTreeRoleRightsSource {
    fn metadata_object_uuid(&self, reference: &str) -> Result<String, String> {
        let parts = reference.split('.').collect::<Vec<_>>();
        if parts.len() < 2 || parts.len() % 2 != 0 {
            return Err(format!("malformed reference {reference}"));
        }
        let (node, rest) = self.object_node(&parts)?;
        let mut current = node.as_ref();
        for pair in rest.chunks(2) {
            current = unique_child(current, |child| {
                child.tag == pair[0] && child.name == pair[1]
            })
            .ok_or_else(|| format!("{reference}: no single {} {}", pair[0], pair[1]))?;
        }
        Ok(current.uuid.clone())
    }

    fn declared_field_uuid(&self, object: &str, field: &str) -> Result<Option<String>, String> {
        let parts = object.split('.').collect::<Vec<_>>();
        if parts.len() < 2 || parts.len() % 2 != 0 {
            return Err(format!("malformed reference {object}"));
        }
        let (node, rest) = self.object_node(&parts)?;
        let mut current = node.as_ref();
        for pair in rest.chunks(2) {
            current = unique_child(current, |child| {
                child.tag == pair[0] && child.name == pair[1]
            })
            .ok_or_else(|| format!("{object}: no single {} {}", pair[0], pair[1]))?;
        }
        let matches = current
            .children
            .iter()
            .filter(|child| {
                child.name == field
                    && matches!(
                        child.tag.as_str(),
                        "Attribute"
                            | "Dimension"
                            | "Resource"
                            | "AccountingFlag"
                            | "ExtDimensionAccountingFlag"
                            | "AddressingAttribute"
                    )
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Ok(None),
            [only] => Ok(Some(only.uuid.clone())),
            _ => Err(format!("{object} declares {field} more than once")),
        }
    }
}

fn unique_child<'a>(
    node: &'a UuidNode,
    matches: impl Fn(&UuidNode) -> bool,
) -> Option<&'a UuidNode> {
    let mut found = None;
    for child in &node.children {
        if matches(child) {
            if found.is_some() {
                return None;
            }
            found = Some(child);
        }
    }
    found
}

/// The uuid-bearing elements of one metadata file, nested as the file nests
/// them, each with the `<Properties><Name>` directly under it.
fn parse_uuid_tree(xml: &[u8]) -> Option<UuidNode> {
    let xml = xml.strip_prefix(b"\xef\xbb\xbf").unwrap_or(xml);
    let mut reader = Reader::from_reader(xml);
    let mut buffer = Vec::new();
    // (depth of the element, node)
    let mut open = Vec::<(usize, UuidNode)>::new();
    let mut path = Vec::<String>::new();
    let mut root = None::<UuidNode>;
    let mut name_text = None::<String>;
    loop {
        match reader.read_event_into(&mut buffer).ok()? {
            Event::Start(start) => {
                let local = local_name(start.local_name().as_ref());
                let uuid = start
                    .attributes()
                    .filter_map(Result::ok)
                    .find(|attribute| attribute.key.as_ref() == b"uuid")
                    .map(|attribute| {
                        String::from_utf8_lossy(attribute.value.as_ref()).into_owned()
                    });
                if let Some(uuid) = uuid {
                    open.push((
                        path.len(),
                        UuidNode {
                            tag: local.clone(),
                            uuid: uuid.trim().to_ascii_lowercase(),
                            name: String::new(),
                            children: Vec::new(),
                        },
                    ));
                } else if local == "Name"
                    && path.last().map(String::as_str) == Some("Properties")
                    && open
                        .last()
                        .is_some_and(|(depth, _)| depth + 2 == path.len())
                {
                    name_text = Some(String::new());
                }
                path.push(local);
            }
            Event::Text(chunk) => {
                if let Some(name) = name_text.as_mut() {
                    name.push_str(&chunk.decode().ok()?);
                }
            }
            Event::GeneralRef(reference) => {
                if let Some(name) = name_text.as_mut() {
                    if let Some(ch) = reference.resolve_char_ref().ok()? {
                        name.push(ch);
                    } else {
                        name.push_str(resolve_xml_entity(&reference.decode().ok()?)?);
                    }
                }
            }
            Event::End(_) => {
                let local = path.pop()?;
                if local == "Name"
                    && let Some(name) = name_text.take()
                    && let Some((_, node)) = open.last_mut()
                    && node.name.is_empty()
                {
                    node.name = name.trim().to_string();
                }
                if open.last().is_some_and(|(depth, _)| *depth == path.len()) {
                    let (_, node) = open.pop()?;
                    match open.last_mut() {
                        Some((_, parent)) => parent.children.push(node),
                        None if root.is_none() => root = Some(node),
                        None => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    root
}

/// The folder a top-level metadata kind's files live in.
fn metadata_family_folder(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "AccountingRegister" => "AccountingRegisters",
        "AccumulationRegister" => "AccumulationRegisters",
        "Bot" => "Bots",
        "BusinessProcess" => "BusinessProcesses",
        "CalculationRegister" => "CalculationRegisters",
        "Catalog" => "Catalogs",
        "ChartOfAccounts" => "ChartsOfAccounts",
        "ChartOfCalculationTypes" => "ChartsOfCalculationTypes",
        "ChartOfCharacteristicTypes" => "ChartsOfCharacteristicTypes",
        "CommandGroup" => "CommandGroups",
        "CommonAttribute" => "CommonAttributes",
        "CommonCommand" => "CommonCommands",
        "CommonForm" => "CommonForms",
        "CommonModule" => "CommonModules",
        "CommonPicture" => "CommonPictures",
        "CommonTemplate" => "CommonTemplates",
        "Constant" => "Constants",
        "DataProcessor" => "DataProcessors",
        "DefinedType" => "DefinedTypes",
        "Document" => "Documents",
        "DocumentJournal" => "DocumentJournals",
        "DocumentNumerator" => "DocumentNumerators",
        "Enum" => "Enums",
        "EventSubscription" => "EventSubscriptions",
        "ExchangePlan" => "ExchangePlans",
        "ExternalDataSource" => "ExternalDataSources",
        "FilterCriterion" => "FilterCriteria",
        "FunctionalOption" => "FunctionalOptions",
        "FunctionalOptionsParameter" => "FunctionalOptionsParameters",
        "HTTPService" => "HTTPServices",
        "InformationRegister" => "InformationRegisters",
        "IntegrationService" => "IntegrationServices",
        "Report" => "Reports",
        "ScheduledJob" => "ScheduledJobs",
        "Sequence" => "Sequences",
        "SessionParameter" => "SessionParameters",
        "SettingsStorage" => "SettingsStorages",
        "Task" => "Tasks",
        "WebService" => "WebServices",
        "WSReference" => "WSReferences",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;

    impl RoleRightsSource for Fixed {
        fn metadata_object_uuid(&self, reference: &str) -> Result<String, String> {
            match reference {
                "Catalog.A" => Ok("aaaaaaaa-0000-4000-8000-000000000001".to_string()),
                "Catalog.A.Attribute.B" => Ok("bbbbbbbb-0000-4000-8000-000000000002".to_string()),
                "Configuration.C" => Ok("cccccccc-0000-4000-8000-000000000003".to_string()),
                _ => Err(reference.to_string()),
            }
        }

        fn declared_field_uuid(&self, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
    }

    /// The order the stored row of БСП's `ЧтениеДанныхСервисаDSS` lists its
    /// six objects in, from the uuids' first groups in XML order.
    #[test]
    fn boost_order_reproduces_a_stored_collision() {
        let xml_order = [
            0x041c364f_u64,
            0x440ecfe8,
            0x7406da96,
            0x83e30f0b,
            0xab1efcef,
            0xecc0ac05,
        ];
        let order = boost_unordered_iteration_order(&xml_order)
            .into_iter()
            .map(|index| xml_order[index])
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            [
                0xecc0ac05, 0xab1efcef, 0x83e30f0b, 0x440ecfe8, 0x7406da96, 0x041c364f
            ]
        );
    }

    #[test]
    fn writes_the_platform_layout() {
        let xml = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Rights xmlns=\"http://v8.1c.ru/8.2/roles\" version=\"2.20\">\r\n\t<setForNewObjects>true</setForNewObjects>\r\n\t<setForAttributesByDefault>true</setForAttributesByDefault>\r\n\t<independentRightsOfChildObjects>false</independentRightsOfChildObjects>\r\n\t<object>\r\n\t\t<name>Catalog.A</name>\r\n\t\t<right>\r\n\t\t\t<name>Read</name>\r\n\t\t\t<value>true</value>\r\n\t\t\t<restrictionByCondition>\r\n\t\t\t\t<condition>a &amp; \"b\"\nc</condition>\r\n\t\t\t</restrictionByCondition>\r\n\t\t</right>\r\n\t</object>\r\n\t<object>\r\n\t\t<name>Catalog.A.Attribute.B</name>\r\n\t\t<right>\r\n\t\t\t<name>View</name>\r\n\t\t\t<value>false</value>\r\n\t\t</right>\r\n\t</object>\r\n\t<object>\r\n\t\t<name>Configuration.C</name>\r\n\t\t<right>\r\n\t\t\t<name>Administration</name>\r\n\t\t\t<value>false</value>\r\n\t\t</right>\r\n\t</object>\r\n</Rights>";
        let written = write_role_rights(xml.as_bytes(), &Fixed).unwrap();
        let plain = String::from_utf8(written.plain).unwrap();
        assert!(plain.starts_with("\u{feff}{10,\r\n{3,\r\n{\r\n{1,cccccccc-"));
        assert!(plain.contains(
            "{1,1,1c87578f-9e09-4ec0-a991-5629c87b1588,1,1,\r\n{1c87578f-9e09-4ec0-a991-5629c87b1588,\r\n{1,\r\n{1,\"a & \"\"b\"\"\r\nc\",0}\r\n}\r\n}\r\n}"
        ));
        assert!(plain.contains("{1,bbbbbbbb-0000-4000-8000-000000000002,0,1},\r\n{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,-1}"));
        // The launch-mode rights the full-rights XML leaves out come back as
        // `1`, after the rights the order puts before them.
        assert!(plain.contains(
            "{0,900e3c92-6e18-4874-846a-b28780b5b54c,-1,d066966a-ff6a-4a41-bd68-6191cab083bc,1,"
        ));
        assert!(plain.ends_with("\r\n},\r\n{0},1,1,0,4294967295}"));
    }
}
