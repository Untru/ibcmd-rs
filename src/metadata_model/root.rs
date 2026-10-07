//! The Configuration object's own row, and the `root`, `version` and
//! `versions` rows an empty infobase needs beside it.
//!
//! The Configuration row is `{2,{<uuid>},7,<seven sections>,{{0,"",""}}}`.
//! The first section carries the `<Properties>` tuple and every section
//! lists the uuids of the configuration's child objects, one slot per
//! metadata class, in `<ChildObjects>` order. The tuple comes in three
//! stored shapes, all read by the platform:
//!
//! * `{67,...}`, 60 fields: a configuration kept in compatibility 8.3.24 or
//!   lower (БСП 3.1.11 as 8.3.27 stores it);
//! * `{68,...}`, 61 fields: 8.3.25 .. 8.3.27 (ERP УХ 3.3.3.3 under 8.3.27 and
//!   under 8.5, and every configuration 8.3.27.2214 builds from XML in the
//!   bundled native evidence);
//! * `{76,...}`, 77 fields: compatibility 8.5 (БСП 3.2.1 under 8.5.1).
//!
//! Every field was placed by comparing those rows with their `Configuration.xml`
//! and with the evidence corpora; a property whose place no corpus shows is
//! only accepted at the value every corpus has, and refused otherwise.

#[path = "root_export.rs"]
pub mod export;

use std::collections::BTreeSet;

use anyhow::{Context, Result, anyhow, bail};

use super::brace::{Brace, serialize_row};
use super::xml::{Element, MetadataXml};
use super::{DescriptorContext, ObjectXml, localized, md_base, not_yet, parse_bool};
use crate::brace_list;

pub fn compile(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    match object.kind {
        "Configuration" => configuration(object, context),
        _ => Err(not_yet(object)),
    }
}

/// The stored shape of the Configuration `<Properties>` tuple, ordered from
/// the oldest shape to the newest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigurationShape {
    /// `{67,...}`: 60 fields.
    V67,
    /// `{68,...}`: 61 fields.
    V68,
    /// `{76,...}`: 77 fields, platform 8.5.
    V76,
}

impl ConfigurationShape {
    /// The shape a configuration of this compatibility is stored in.
    pub fn for_compatibility(compatibility: u32) -> Self {
        if compatibility >= 80500 {
            Self::V76
        } else if compatibility <= 80324 {
            Self::V67
        } else {
            Self::V68
        }
    }

    fn tag(self) -> i64 {
        match self {
            Self::V67 => 67,
            Self::V68 => 68,
            Self::V76 => 76,
        }
    }
}

/// What the service rows need to know about a tree's configuration.
#[derive(Clone, Debug)]
pub struct ConfigurationFacts {
    pub uuid: String,
    pub compatibility: u32,
    pub shape: ConfigurationShape,
    /// The restricted-compatibility features the configuration uses, the
    /// uuids `version` lists (see [`version_row`]).
    pub features: Vec<&'static str>,
}

/// Reads `Configuration.xml` for the service rows.
pub fn configuration_facts(xml: &[u8]) -> Result<ConfigurationFacts> {
    let doc = MetadataXml::parse(xml)?;
    let object = doc.object()?;
    if object.name != "Configuration" {
        bail!("expected <Configuration>, found <{}>", object.name);
    }
    let uuid = object
        .attr("uuid")
        .ok_or_else(|| anyhow!("<Configuration> has no uuid"))?
        .to_ascii_lowercase();
    let properties = object
        .child("Properties")
        .ok_or_else(|| anyhow!("<Configuration> has no <Properties>"))?;
    let compatibility = compatibility_of(properties)?;
    // The platform's feature "palette colors" (since 8.5.1; its uuid comes
    // from the platform registry). The platform keeps a registry of features
    // an older platform cannot use -- backend.dll 8.5.1.1150 builds it from
    // thirteen (uuid, packed version) pairs, twelve of them 8.3.9 .. 8.3.26
    // and this one 8.5.1 -- and, on save, lists in `version` those the
    // configuration's objects report using (its
    // `RestrictedCompatibilityUsedFeatures`). A platform that does not know a
    // listed uuid refuses the configuration ("Для использования этой
    // конфигурации требуется более новая версия платформы"). Every
    // PaletteColor object reports this one, unconditionally (the palette
    // colour class's feature method is the only code that names it). The 8.3
    // features are reported by other objects (web service operation
    // parameters, templates, ...) under conditions not worked out; no
    // measured configuration (БСП 8.3.24 and 8.5, ERP УХ 8.3.27) lists one.
    let uses_palette_colors = object
        .child("ChildObjects")
        .is_some_and(|children| children.children_named("PaletteColor").next().is_some());
    let features = if uses_palette_colors {
        vec![crate::platform::feature_uuid(
            crate::platform::FEATURE_PALETTE_COLORS,
        )?]
    } else {
        Vec::new()
    };
    Ok(ConfigurationFacts {
        uuid,
        compatibility,
        shape: ConfigurationShape::for_compatibility(compatibility),
        features,
    })
}

/// `root`: `{2,<configuration uuid>,}`. A configuration 8.5 signed keeps a
/// signature in the third field; a load from XML has none to give, and an
/// unsigned root is what every 8.3.27 and 8.5 row of ERP УХ carries.
pub fn root_row(facts: &ConfigurationFacts) -> Vec<u8> {
    serialize_row(&brace_list![
        Brace::num(2),
        Brace::uuid(&facts.uuid),
        Brace::atom(""),
    ])
}

/// `version`: `{{<format>,0,{<compatibility>,<n>,{<feature uuid>}×n}}}`,
/// format 216 for the 8.3 shapes and 217 for 8.5. The list is the
/// restricted-compatibility features the configuration uses
/// ([`ConfigurationFacts::features`]): fixed uuids from the platform's
/// registry, not values generated on save. БСП 8.5 lists the palette colour
/// feature and every measured 8.3 row lists none. Apply refused a generated
/// uuid there as a feature of a newer platform. An 8.5 row without palette
/// colours (`{80501,0}`) follows from the platform's code, not from a
/// stored row; a compatibility other than 8.5.1 has no stored row at all and
/// is refused rather than guessed.
pub fn version_row(facts: &ConfigurationFacts) -> Result<Vec<u8>> {
    // The format follows the compatibility, not the tuple: the configuration
    // 8.5.1.1529 saved in the `{76,...}` tuple at compatibility 8.3.27 stores
    // `{216,0,{80327,0}}` (`home_page/one_column_v85/input.cf`).
    let format = match facts.compatibility {
        compatibility if compatibility < 80500 => 216,
        80501 => 217,
        _ => bail!(
            "no stored `version` row of compatibility {} is measured (only 8.5.1, БСП 8.5): its format number and feature list are unknown",
            facts.compatibility
        ),
    };
    let mut record = vec![
        Brace::num(facts.compatibility as i64),
        Brace::num(facts.features.len() as i64),
    ];
    record.extend(
        facts
            .features
            .iter()
            .map(|uuid| brace_list![Brace::uuid(uuid)]),
    );
    Ok(serialize_row(&brace_list![brace_list![
        Brace::num(format),
        Brace::num(0),
        Brace::List(record),
    ]]))
}

/// `versions`: `{1,<count>,"",<uuid>,"<file>",<uuid>,...}` -- one generation
/// uuid for the configuration as a whole, then one per stored file, names in
/// byte order (`root`, `version` and `versions` included). Every stored
/// `versions` row lists exactly the Config rows beside it (dynamic-update
/// rows aside) and gives each its own uuid; the platform replaces a file's
/// uuid whenever it writes that file, so a freshly loaded configuration gets
/// a fresh uuid for every file.
pub fn versions_row(names: &[String], mut generation: impl FnMut() -> String) -> Vec<u8> {
    let mut sorted = names
        .iter()
        .map(String::as_str)
        .chain(["root", "version", "versions"])
        .collect::<Vec<_>>();
    sorted.sort_unstable();
    sorted.dedup();
    let mut items = Vec::with_capacity(sorted.len() * 2 + 4);
    items.push(Brace::num(1));
    items.push(Brace::num(sorted.len() as i64 + 1));
    items.push(Brace::str(""));
    items.push(Brace::uuid(&generation()));
    for name in sorted {
        items.push(Brace::str(name));
        items.push(Brace::uuid(&generation()));
    }
    serialize_row(&Brace::List(items))
}

/// The section a class of child objects is listed in, and its slots in
/// stored order (ascending class uuid).
struct Section {
    class_id: &'static str,
    wrapper: Wrapper,
    slots: &'static [Slot],
}

#[derive(Clone, Copy)]
enum Wrapper {
    /// `{1,<properties>,<n>,<slots>}`
    Properties,
    /// `{6,{1,{{1,0,<id>},<nil>},<n>,<slots>}}`
    Directory,
    /// `{1,{0,{1,0,<id>}},<n>,<slots>}`
    Zero,
    /// `{1,{{1,0,<id>}},<n>,<slots>}`
    Direct,
}

#[derive(Clone, Copy)]
struct Slot {
    class_id: &'static str,
    /// `None`: a slot no XML kind fills (always empty in every corpus).
    kind: Option<&'static str>,
    /// Only in these shapes.
    since: SlotSince,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SlotSince {
    Always,
    V68,
    V76,
}

impl Slot {
    fn in_shape(&self, shape: ConfigurationShape) -> bool {
        match self.since {
            SlotSince::Always => true,
            SlotSince::V68 => shape != ConfigurationShape::V67,
            SlotSince::V76 => shape == ConfigurationShape::V76,
        }
    }
}

const fn slot(class_id: &'static str, kind: &'static str) -> Slot {
    Slot {
        class_id,
        kind: Some(kind),
        since: SlotSince::Always,
    }
}

const SECTION_1: &[Slot] = &[
    slot("09736b02-9cac-4e3f-b4f7-d3e9576ab948", "Role"),
    slot("0c89c792-16c3-11d5-b96b-0050bae0a95d", "CommonTemplate"),
    slot("0fe48980-252d-11d6-a3c7-0050bae0a776", "CommonModule"),
    slot("0fffc09c-8f4c-47cc-b41c-8d5c5a221d79", "HTTPService"),
    Slot {
        class_id: "102f6202-43fa-40b0-8898-acd3876daacb",
        kind: Some("PaletteColor"),
        since: SlotSince::V76,
    },
    slot("11bdaf85-d5ad-4d91-bb24-aa0eee139052", "ScheduledJob"),
    slot("15794563-ccec-41f6-a83c-ec5f7b9a5bc1", "CommonAttribute"),
    slot("24c43748-c938-45d0-8d14-01424a72b11e", "SessionParameter"),
    slot(
        "30d554db-541e-4f62-8970-a1c6dcfeb2bc",
        "FunctionalOptionsParameter",
    ),
    slot("37f2fa9a-b276-11d4-9435-004095e12fc7", "Subsystem"),
    slot("39bddf6a-0c3c-452b-921c-d99cfa1c2f1b", "Interface"),
    slot("3e5404af-6ef8-4c73-ad11-91bd2dfac4c8", "Style"),
    slot("3e7bfcc0-067d-11d6-a3c7-0050bae0a776", "FilterCriterion"),
    slot("46b4cd97-fd13-4eaa-aba2-3bddd7699218", "SettingsStorage"),
    slot("4e828da6-0f44-4b5b-b1c0-a2b3cfe7bdcc", "EventSubscription"),
    slot("58848766-36ea-4076-8800-e91eb49590d7", "StyleItem"),
    slot("6e6dc072-b7ac-41e7-8f88-278d25b6da2a", "Bot"),
    slot("7dcd43d9-aca5-4926-b549-1842e6a4e8cf", "CommonPicture"),
    slot("857c4a91-e5f4-4fac-86ec-787626f1c108", "ExchangePlan"),
    slot("8657032e-7740-4e1d-a3ba-5dd6e8afb78f", "WebService"),
    slot("9cd510ce-abfc-11d4-9434-004095e12fc7", "Language"),
    Slot {
        class_id: "a7641777-7813-45c6-96ef-9d51587a6ac6",
        kind: None,
        since: SlotSince::V68,
    },
    slot("af547940-3268-434f-a3e7-e47d6d2638c3", "FunctionalOption"),
    slot("c045099e-13b9-4fb6-9d50-fca00202971e", "DefinedType"),
    slot("cc9df798-7c94-4616-97d2-7aa0b7bc515e", "XDTOPackage"),
    slot("d26096fb-7a5d-4df9-af63-47d04771fa9b", "WSReference"),
];

const SECTION_2: &[Slot] = &[
    slot("0195e80c-b157-11d4-9435-004095e12fc7", "Constant"),
    slot("061d872a-5787-460e-95ac-ed74ea3a3e84", "Document"),
    slot("07ee8426-87f1-11d5-b99c-0050bae0a95d", "CommonForm"),
    slot(
        "13134201-f60b-11d5-a3c7-0050bae0a776",
        "InformationRegister",
    ),
    slot("1c57eabe-7349-44b3-b1de-ebfeab67b47d", "CommandGroup"),
    slot("2f1a5187-fb0e-4b05-9489-dc5dd6412348", "CommonCommand"),
    slot("36a8e346-9aaa-4af9-bdbd-83be3c177977", "DocumentNumerator"),
    slot("4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d", "DocumentJournal"),
    slot("631b75a0-29e2-11d6-a3c7-0050bae0a776", "Report"),
    slot(
        "82a1b659-b220-4d94-a9bd-14d757b95a48",
        "ChartOfCharacteristicTypes",
    ),
    slot(
        "b64d9a40-1642-11d6-a3c7-0050bae0a776",
        "AccumulationRegister",
    ),
    slot("bc587f20-35d9-11d6-a3c7-0050bae0a776", "Sequence"),
    slot("bf845118-327b-4682-b5c6-285d2a0eb296", "DataProcessor"),
    slot("cf4abea6-37b2-11d4-940f-008048da11f9", "Catalog"),
    slot("f6a80749-5ad7-400b-8519-39dc5dff2542", "Enum"),
];

const SECTION_3: &[Slot] = &[
    slot("238e7e88-3c5f-48b2-8a3b-81ebbecb20ed", "ChartOfAccounts"),
    slot("2deed9b8-0056-4ffe-a473-c20a6c32a0bc", "AccountingRegister"),
];

const SECTION_4: &[Slot] = &[
    slot(
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767",
        "ChartOfCalculationTypes",
    ),
    slot(
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7",
        "CalculationRegister",
    ),
];

const SECTION_5: &[Slot] = &[
    slot("3e63355c-1378-4953-be9b-1deb5fb6bec5", "Task"),
    slot("fcd3404e-1523-48ce-9bc0-ecdb822684a1", "BusinessProcess"),
];

const SECTION_6: &[Slot] = &[slot(
    "5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5",
    "ExternalDataSource",
)];

const SECTION_7: &[Slot] = &[slot(
    "bf3420b0-f6f9-41a0-b83a-fe9d4ab0b65d",
    "IntegrationService",
)];

const SECTIONS: [Section; 7] = [
    Section {
        class_id: MODULE_GROUP_CLASS_ID,
        wrapper: Wrapper::Properties,
        slots: SECTION_1,
    },
    Section {
        class_id: "9fcd25a0-4822-11d4-9414-008048da11f9",
        wrapper: Wrapper::Directory,
        slots: SECTION_2,
    },
    Section {
        class_id: "e3687481-0a87-462c-a166-9f34594f9bba",
        wrapper: Wrapper::Zero,
        slots: SECTION_3,
    },
    Section {
        class_id: "9de14907-ec23-4a07-96f0-85521cb6b53b",
        wrapper: Wrapper::Direct,
        slots: SECTION_4,
    },
    Section {
        class_id: "51f2d5d8-ea4d-4064-8892-82951750031e",
        wrapper: Wrapper::Zero,
        slots: SECTION_5,
    },
    Section {
        class_id: "e68182ea-4237-4383-967f-90c1e3370bc7",
        wrapper: Wrapper::Direct,
        slots: SECTION_6,
    },
    Section {
        class_id: "fb282519-d103-4dd3-bc12-cb271d631dfc",
        wrapper: Wrapper::Direct,
        slots: SECTION_7,
    },
];

/// The class uuid of the section that holds the configuration's own
/// properties; its contained object is the managed-application module group
/// that the configuration's modules, help and interface assets are stored
/// under.
pub const MODULE_GROUP_CLASS_ID: &str = "9cd510cd-abfc-11d4-9434-004095e12fc7";

const DESIGN_TIME_REFERENCE_CLASS_ID: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
const USE_PURPOSE_CLASS_ID: &str = "1708fdaa-cbce-4289-b373-07a5a74bee91";
const PERMISSION_CLASS_ID: &str = "e4c53f94-e5f7-4a34-8c10-218bd811cae1";
const SHARE_REQUEST_TYPE_CLASS_ID: &str = "f251d17e-94e0-4f9b-974e-d642cf9cb6e4";

fn configuration(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    let properties = object.properties()?;
    let compatibility = compatibility_of(properties)?;
    // An 8.3-compatible configuration that platform 8.5 stores in its own
    // layout keeps the 8.5 tuple (`common::tree_stores_layout_8_5_1`).
    let shape = if super::common::stores_layout_8_5_1(context) {
        ConfigurationShape::V76
    } else {
        ConfigurationShape::for_compatibility(compatibility)
    };
    let contained = contained_objects(object.element)?;
    let children = child_objects(object, context)?;

    // Every child kind must have a slot in this shape.
    let known = SECTIONS
        .iter()
        .flat_map(|section| section.slots.iter())
        .filter(|slot| slot.in_shape(shape))
        .filter_map(|slot| slot.kind)
        .collect::<BTreeSet<_>>();
    if let Some((kind, _)) = children
        .iter()
        .find(|(kind, _)| !known.contains(kind.as_str()))
    {
        bail!("Configuration lists {kind} objects, which a {shape:?} row has no slot for");
    }

    let mut row = vec![
        Brace::num(2),
        brace_list![Brace::uuid(&object.uuid)],
        Brace::num(SECTIONS.len() as i64),
    ];
    for section in &SECTIONS {
        let contained_id = contained
            .iter()
            .find(|(class, _)| class == section.class_id)
            .map(|(_, id)| id.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "Configuration has no <xr:ContainedObject> of class {}",
                    section.class_id
                )
            })?;
        let slots = section
            .slots
            .iter()
            .filter(|slot| slot.in_shape(shape))
            .map(|slot| {
                let uuids = slot
                    .kind
                    .and_then(|kind| children.iter().find(|(k, _)| k == kind))
                    .map(|(_, uuids)| uuids.as_slice())
                    .unwrap_or_default();
                let mut items = vec![Brace::atom(slot.class_id), Brace::num(uuids.len() as i64)];
                items.extend(uuids.iter().map(|uuid| Brace::uuid(uuid)));
                Brace::List(items)
            })
            .collect::<Vec<_>>();
        let count = Brace::num(slots.len() as i64);
        let identity = brace_list![Brace::num(1), Brace::num(0), Brace::uuid(contained_id)];
        let body = match section.wrapper {
            Wrapper::Properties => {
                let tuple = properties_tuple(object, context, shape, compatibility, contained_id)?;
                let mut items = vec![Brace::num(1), tuple, count];
                items.extend(slots);
                Brace::List(items)
            }
            Wrapper::Directory => {
                let mut items = vec![
                    Brace::num(1),
                    brace_list![identity, Brace::nil_uuid()],
                    count,
                ];
                items.extend(slots);
                brace_list![Brace::num(6), Brace::List(items)]
            }
            Wrapper::Zero => {
                let mut items = vec![Brace::num(1), brace_list![Brace::num(0), identity], count];
                items.extend(slots);
                Brace::List(items)
            }
            Wrapper::Direct => {
                let mut items = vec![Brace::num(1), brace_list![identity], count];
                items.extend(slots);
                Brace::List(items)
            }
        };
        row.push(brace_list![Brace::atom(section.class_id), body]);
    }
    // Every Config-stored 8.3 row ends with the bare footer; a CF-built one
    // and БСП 8.5's carry `{1,"",""},{<n>}` instead, a counter no source
    // holds.
    row.push(brace_list![brace_list![
        Brace::num(0),
        Brace::str(""),
        Brace::str(""),
    ]]);
    Ok(Brace::List(row))
}

/// `<xr:ContainedObject>` pairs: (class id, object id).
fn contained_objects(element: &Element) -> Result<Vec<(String, String)>> {
    let info = element
        .child("InternalInfo")
        .ok_or_else(|| anyhow!("Configuration has no <InternalInfo>"))?;
    Ok(info
        .children_named("ContainedObject")
        .map(|contained| {
            (
                contained
                    .child_text("ClassId")
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase(),
                contained
                    .child_text("ObjectId")
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_lowercase(),
            )
        })
        .collect())
}

/// `<ChildObjects>` by kind, uuids in document order.
fn child_objects(
    object: &ObjectXml<'_>,
    context: &DescriptorContext,
) -> Result<Vec<(String, Vec<String>)>> {
    let mut by_kind: Vec<(String, Vec<String>)> = Vec::new();
    let Some(children) = object.child_objects() else {
        return Ok(by_kind);
    };
    for child in &children.children {
        let full = format!("{}.{}", child.name, child.text.trim());
        let uuid = context
            .index
            .objects
            .get(&full)
            .map(|entry| entry.uuid.clone())
            .ok_or_else(|| anyhow!("Configuration lists {full}, which the tree has no XML for"))?;
        match by_kind.iter_mut().find(|(kind, _)| *kind == child.name) {
            Some((_, uuids)) => uuids.push(uuid),
            None => by_kind.push((child.name.clone(), vec![uuid])),
        }
    }
    Ok(by_kind)
}

/// `Version8_3_24` -> `80324`.
fn compatibility_of(properties: &Element) -> Result<u32> {
    // An extension's root has no CompatibilityMode; its objects follow its
    // own ConfigurationExtensionCompatibilityMode (`cf load` of an object
    // added to an extension).
    let text = properties
        .child_text("CompatibilityMode")
        .or_else(|| properties.child_text("ConfigurationExtensionCompatibilityMode"))
        .ok_or_else(|| anyhow!("Configuration has no <CompatibilityMode>"))?
        .trim();
    pack_version(text)
        .ok_or_else(|| anyhow!("Configuration <CompatibilityMode> {text:?} is not VersionX_Y_Z"))
}

fn pack_version(text: &str) -> Option<u32> {
    let rest = text.strip_prefix("Version")?;
    let mut parts = rest.split('_').map(|part| part.parse::<u32>().ok());
    let major = parts.next()??;
    let minor = parts.next()??;
    let patch = parts.next()??;
    if parts.next().is_some() || minor > 99 || patch > 99 {
        return None;
    }
    Some(major * 10000 + minor * 100 + patch)
}

fn text_of<'a>(properties: &'a Element, name: &str) -> &'a str {
    properties
        .child_text(name)
        .map(str::trim)
        .unwrap_or_default()
}

/// An element that may be absent or empty, or else must hold one of
/// `expected`.
fn require_default(properties: &Element, name: &str, expected: &[&str]) -> Result<()> {
    let Some(element) = properties.child(name) else {
        return Ok(());
    };
    let text = element.text.trim();
    if element.children.is_empty() && (text.is_empty() || expected.contains(&text)) {
        return Ok(());
    }
    let found = if element.children.is_empty() {
        text.to_string()
    } else {
        format!("<{} children>", element.children.len())
    };
    bail!(
        "Configuration <{name}> is {found:?}; no corpus shows where a value other than {expected:?} is stored"
    )
}

/// A metadata reference (`Role.X`, `CommonForm.Y`) -> its uuid; the nil uuid
/// when empty.
fn reference(
    properties: &Element,
    name: &str,
    kind: &str,
    context: &DescriptorContext,
) -> Result<Brace> {
    let text = text_of(properties, name);
    if text.is_empty() {
        return Ok(Brace::nil_uuid());
    }
    resolve(text, kind, context)
        .with_context(|| format!("Configuration <{name}>"))
        .map(|uuid| Brace::uuid(&uuid))
}

fn resolve(text: &str, kind: &str, context: &DescriptorContext) -> Result<String> {
    if !text.starts_with(&format!("{kind}.")) {
        bail!("{text} is not a {kind}");
    }
    context
        .index
        .objects
        .get(text)
        .map(|entry| entry.uuid.clone())
        .ok_or_else(|| anyhow!("{text} is not in the tree"))
}

fn flag_of(properties: &Element, name: &str) -> Result<Brace> {
    let text = text_of(properties, name);
    let value = if text.is_empty() {
        false
    } else {
        parse_bool(text).with_context(|| format!("Configuration <{name}>"))?
    };
    Ok(Brace::flag(value))
}

fn enum_of(
    properties: &Element,
    name: &str,
    values: &[(&str, i64)],
    default: &str,
) -> Result<Brace> {
    let text = match text_of(properties, name) {
        "" => default,
        text => text,
    };
    values
        .iter()
        .find(|(candidate, _)| *candidate == text)
        .map(|(_, code)| Brace::num(*code))
        .ok_or_else(|| anyhow!("Configuration <{name}> {text:?} has no known stored code"))
}

fn properties_tuple(
    object: &ObjectXml<'_>,
    context: &DescriptorContext,
    shape: ConfigurationShape,
    compatibility: u32,
    module_group: &str,
) -> Result<Brace> {
    let p = object.properties()?;
    // Properties no corpus shows at another value: accepted at that value only.
    for (name, expected) in [
        ("AdditionalFullTextSearchDictionaries", &[][..]),
        ("DynamicListsUserSettingsStorage", &[][..]),
        ("URLExternalDataStorage", &[][..]),
        ("Content", &[][..]),
        ("DefaultReportAppearanceTemplate", &[][..]),
        ("DefaultDynamicListSettingsForm", &[][..]),
        ("DefaultDataHistoryChangeHistoryForm", &[][..]),
        ("DefaultDataHistoryVersionDataForm", &[][..]),
        ("DefaultDataHistoryVersionDifferencesForm", &[][..]),
        ("DefaultCollaborationSystemUsersChoiceForm", &[][..]),
        ("RequiredMobileApplicationPermissions", &[][..]),
        ("StandaloneConfigurationRestrictionRoles", &[][..]),
        ("MobileApplicationURLs", &[][..]),
        ("MainClientApplicationWindowMode", &["Normal"][..]),
        ("DefaultInterface", &[][..]),
        ("DataLockControlMode", &["Managed"][..]),
        ("ObjectAutonumerationMode", &["NotAutoFree"][..]),
        ("DatabaseTablespacesUseMode", &["DontUse"][..]),
        ("DefaultConstantsForm", &[][..]),
    ] {
        require_default(p, name, expected)?;
    }
    if shape == ConfigurationShape::V76 {
        for (name, expected) in [
            ("AuxiliaryReportForm", &[][..]),
            ("AuxiliaryReportVariantForm", &[][..]),
            ("AuxiliaryReportSettingsForm", &[][..]),
            ("AuxiliaryDynamicListSettingsForm", &[][..]),
            ("AuxiliaryDataHistoryChangeHistoryForm", &[][..]),
            ("AuxiliaryDataHistoryVersionDataForm", &[][..]),
            ("AuxiliaryDataHistoryVersionDifferencesForm", &[][..]),
            ("AuxiliaryCollaborationSystemUsersChoiceForm", &[][..]),
            (
                "MainClientApplicationWindowInterfaceVariant",
                &["NavigationLeft"][..],
            ),
            ("ClientApplicationTheme", &["Auto"][..]),
            (
                "ClientApplicationWindowsOpenVariant",
                &["OpenDataInTabs", "OpenDataInDialogs"][..],
            ),
            ("Version85InterfaceMigrationMode", &["Use", "DontUse"][..]),
        ] {
            require_default(p, name, expected)?;
        }
    }

    let roles = default_roles(p, context)?;
    let functionalities = mobile_functionalities(p)?;
    let permissions = permissions_of(&functionalities);
    let compatibility = Brace::num(compatibility as i64);
    let (interface_8_3, interface_8_5_1) = interface_compatibility(p, shape)?;
    let nil = Brace::nil_uuid;
    let pair = || brace_list![Brace::num(0), Brace::num(0)];

    let mut fields = vec![
        // 0
        Brace::num(shape.tag()),
        // 1: the header, keyed by the module group.
        brace_list![Brace::num(0), md_base(module_group, p)],
        // 2
        Brace::str(text_of(p, "NamePrefix")),
        // 3: `ScriptVariant`, not `DefaultRunMode`. Over 21 corpora the
        // field is `0` on the one English configuration (ERP WE English) and
        // `1` on the 20 Russian ones, while the run mode is `Managed` on that
        // English one and `Ordinary` on one Russian (acc) -- which field 21
        // separates.
        enum_of(
            p,
            "ScriptVariant",
            &[("English", 0), ("Russian", 1)],
            "Russian",
        )?,
        // 4, 5: detailed before brief (WMS5 tells the two apart).
        localized(p.child("DetailedInformation")),
        localized(p.child("BriefInformation")),
        // 6, 7, 8
        localized(p.child("Copyright")),
        localized(p.child("VendorInformationAddress")),
        localized(p.child("ConfigurationInformationAddress")),
        // 9, 10
        reference(p, "DefaultStyle", "Style", context)?,
        reference(p, "DefaultLanguage", "Language", context)?,
        // 11
        nil(),
        // 12: the main role of the 8.2 era, which the XML no longer prints;
        // every corpus holds its first default role here.
        roles.first().map_or_else(nil, |uuid| Brace::uuid(uuid)),
        // 13
        flag_of(p, "IncludeHelpInContents")?,
        // 14, 15, 16
        Brace::str(text_of(p, "Vendor")),
        Brace::str(text_of(p, "Version")),
        Brace::str(text_of(p, "UpdateCatalogAddress")),
        // 17 .. 21
        Brace::num(1),
        pair(),
        Brace::num(1),
        pair(),
        // 21: `DefaultRunMode` (`0` on acc, the one `OrdinaryApplication`
        // corpus, `1` on the other 20).
        enum_of(
            p,
            "DefaultRunMode",
            &[("OrdinaryApplication", 0), ("ManagedApplication", 1)],
            "ManagedApplication",
        )?,
        // 22 .. 25
        reference(p, "CommonSettingsStorage", "SettingsStorage", context)?,
        reference(p, "ReportsUserSettingsStorage", "SettingsStorage", context)?,
        reference(p, "ReportsVariantsStorage", "SettingsStorage", context)?,
        reference(p, "FormDataSettingsStorage", "SettingsStorage", context)?,
        // 26
        compatibility.clone(),
        // 27
        pair(),
        // 28, 29
        flag_of(p, "UseManagedFormInOrdinaryApplication")?,
        flag_of(p, "UseOrdinaryFormInManagedApplication")?,
        // 30, 31, 32
        reference(p, "DefaultReportForm", "CommonForm", context)?,
        reference(p, "DefaultReportVariantForm", "CommonForm", context)?,
        reference(p, "DefaultReportSettingsForm", "CommonForm", context)?,
        // 33
        use_purposes(p)?,
        // 34, 35
        nil(),
        nil(),
        // 36
        enum_of(
            p,
            "ModalityUseMode",
            &[("Use", 0), ("UseWithWarnings", 1), ("DontUse", 2)],
            "DontUse",
        )?,
        // 37: `<DefaultSearchForm>`, read back by the exporter from this
        // tuple field on the same terms as 30..32 (Управление задачами
        // names `CommonForm.ФормаПоиска` there).
        reference(p, "DefaultSearchForm", "CommonForm", context)?,
        // 38
        interface_8_3,
        // 39
        roles_list(&roles),
        // 40
        permission_table(&permissions),
        // 41
        enum_of(
            p,
            "SynchronousPlatformExtensionAndAddInCallUseMode",
            &[("Use", 0), ("UseWithWarnings", 1), ("DontUse", 2)],
            "DontUse",
        )?,
        // 42
        Brace::str(""),
        // 43
        compatibility,
        // 44, 45
        Brace::num(1),
        Brace::num(0),
        // 46, 47, 48
        nil(),
        nil(),
        nil(),
        // 49, 50
        Brace::num(1),
        nil(),
        // 51
        permission_flags(&permissions),
        // 52
        pair(),
        // 53
        functionality_table(&functionalities, shape),
        // 54 .. 58
        brace_list![Brace::num(0)],
        nil(),
        Brace::str(""),
        Brace::num(0),
        Brace::num(1),
        // 59
        share_request_types(p)?,
    ];
    match shape {
        ConfigurationShape::V67 => {}
        ConfigurationShape::V68 => fields.push(Brace::num(1)),
        ConfigurationShape::V76 => {
            fields.push(Brace::num(1));
            // 61 .. 68: the 8.5 interface properties, at the only values the
            // one 8.5 corpus shows (checked above); 62 is the 8.5 code of
            // the interface compatibility mode.
            fields.push(Brace::num(0));
            fields.push(interface_8_5_1);
            fields.push(Brace::num(0));
            fields.push(localized(p.child("Caption")));
            fields.push(localized(p.child("ShortCaption")));
            fields.push(Brace::num(0));
            let (dialogs, dont_use) = v76_window_and_migration(p)?;
            fields.push(Brace::num(dialogs));
            fields.push(Brace::num(dont_use));
            // 69 .. 76: the eight auxiliary forms (all empty, checked above).
            for _ in 0..8 {
                fields.push(nil());
            }
        }
    }
    Ok(Brace::List(fields))
}

/// Fields 67 and 68 of the 8.5 tuple: `0,0` for `OpenDataInTabs` with the
/// 8.5 interface migration (БСП 3.2.1.356 under 8.5.1.1150), `1,1` for
/// `OpenDataInDialogs` with `DontUse` -- what 8.5.1.1529 stored for a tree
/// that names neither property, and what 8.5 prints for a configuration
/// without them (`mssql_dump::refs::configuration_properties_8_5_1`). Which
/// field holds which is not known, so only these two pairs are written.
fn v76_window_and_migration(p: &Element) -> Result<(i64, i64)> {
    let text = |name: &str, default: &'static str| match text_of(p, name) {
        "" => default,
        text => text,
    };
    match (
        text("ClientApplicationWindowsOpenVariant", "OpenDataInTabs"),
        text("Version85InterfaceMigrationMode", "Use"),
    ) {
        ("OpenDataInTabs", "Use") => Ok((0, 0)),
        ("OpenDataInDialogs", "DontUse") => Ok((1, 1)),
        (window, migration) => bail!(
            "Configuration <ClientApplicationWindowsOpenVariant> {window:?} with <Version85InterfaceMigrationMode> {migration:?}: no 8.5 tuple on record stores them"
        ),
    }
}

/// `<InterfaceCompatibilityMode>`: the 8.3 code (field 38) and, for 8.5,
/// the code of field 62.
fn interface_compatibility(p: &Element, shape: ConfigurationShape) -> Result<(Brace, Brace)> {
    let text = match text_of(p, "InterfaceCompatibilityMode") {
        "" => "TaxiEnableVersion8_2",
        text => text,
    };
    if shape == ConfigurationShape::V76 {
        return match text {
            "Version8_5EnableTaxi" => Ok((Brace::num(3), Brace::num(6))),
            // `home_page/one_column_v85/input.cf` (8.5.1.1529, compatibility
            // 8.3.27): field 62 repeats the 8.3 code.
            "TaxiEnableVersion8_2" => Ok((Brace::num(2), Brace::num(2))),
            other => bail!(
                "Configuration <InterfaceCompatibilityMode> {other:?}: the 8.5 tuples on record show only Version8_5EnableTaxi and TaxiEnableVersion8_2"
            ),
        };
    }
    let code = match text {
        "Version8_2" => 0,
        "TaxiEnableVersion8_2" => 2,
        "Taxi" => 3,
        other => {
            bail!("Configuration <InterfaceCompatibilityMode> {other:?} has no known stored code")
        }
    };
    Ok((Brace::num(code), Brace::num(code)))
}

fn default_roles(p: &Element, context: &DescriptorContext) -> Result<Vec<String>> {
    let Some(roles) = p.child("DefaultRoles") else {
        return Ok(Vec::new());
    };
    roles
        .children_named("Item")
        .map(|item| {
            resolve(item.text.trim(), "Role", context).context("Configuration <DefaultRoles>")
        })
        .collect()
}

/// `{0,<n>,{"#",<design-time reference>,{1,<role>}}...}`
fn roles_list(roles: &[String]) -> Brace {
    let mut items = vec![Brace::num(0), Brace::num(roles.len() as i64)];
    for role in roles {
        items.push(brace_list![
            Brace::str("#"),
            Brace::atom(DESIGN_TIME_REFERENCE_CLASS_ID),
            brace_list![Brace::num(1), Brace::uuid(role)],
        ]);
    }
    Brace::List(items)
}

/// `{<n>,{"#",<use purpose>,<code>}...}`: `PlatformApplication` = 1.
fn use_purposes(p: &Element) -> Result<Brace> {
    let values = p
        .child("UsePurposes")
        .map(|purposes| {
            purposes
                .children_named("Value")
                .map(|value| value.text.trim().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut items = vec![Brace::num(values.len() as i64)];
    for value in values {
        let code = match value.as_str() {
            "PlatformApplication" => 1,
            other => bail!("Configuration <UsePurposes> {other:?} has no known stored code"),
        };
        items.push(brace_list![
            Brace::str("#"),
            Brace::atom(USE_PURPOSE_CLASS_ID),
            Brace::num(code),
        ]);
    }
    Ok(Brace::List(items))
}

/// The mobile functionalities, by stored id, in stored order.
const FUNCTIONALITIES: [(u32, &str); 38] = [
    (0, "Biometrics"),
    (1, "Location"),
    (2, "BackgroundLocation"),
    (3, "BluetoothPrinters"),
    (4, "WiFiPrinters"),
    (5, "Contacts"),
    (6, "Calendars"),
    (7, "PushNotifications"),
    (8, "LocalNotifications"),
    (9, "InAppPurchases"),
    (10, "PersonalComputerFileExchange"),
    (11, "Ads"),
    (12, "NumberDialing"),
    (13, "CallProcessing"),
    (14, "CallLog"),
    (15, "AutoSendSMS"),
    (16, "ReceiveSMS"),
    (17, "SMSLog"),
    (18, "Camera"),
    (19, "Microphone"),
    (20, "MusicLibrary"),
    (21, "PictureAndVideoLibraries"),
    (22, "AudioPlaybackAndVibration"),
    (23, "BackgroundAudioPlaybackAndVibration"),
    (24, "InstallPackages"),
    (25, "OSBackup"),
    (26, "ApplicationUsageStatistics"),
    (27, "BarcodeScanning"),
    (32, "BackgroundAudioRecording"),
    (33, "AllFilesAccess"),
    (34, "Videoconferences"),
    (35, "NFC"),
    (36, "DocumentScanning"),
    (37, "SpeechToText"),
    (38, "Geofences"),
    (39, "IncomingShareRequests"),
    (40, "AllIncomingShareRequestsTypesProcessing"),
    (41, "TextToSpeech"),
];

/// `<UsedMobileApplicationFunctionalities>`: the ids in use. A name the XML
/// leaves out is off.
fn mobile_functionalities(p: &Element) -> Result<BTreeSet<u32>> {
    let mut used = BTreeSet::new();
    let Some(list) = p.child("UsedMobileApplicationFunctionalities") else {
        return Ok(used);
    };
    for item in list.children_named("functionality") {
        let name = item.child_text("functionality").unwrap_or_default().trim();
        let on = parse_bool(item.child_text("use").unwrap_or("false").trim())
            .with_context(|| format!("mobile functionality {name}"))?;
        let id = FUNCTIONALITIES
            .iter()
            .find(|(_, candidate)| *candidate == name)
            .map(|(id, _)| *id)
            .ok_or_else(|| anyhow!("unknown mobile functionality {name:?}"))?;
        if on {
            used.insert(id);
        }
    }
    if list.children_named("permissionMessage").next().is_some() {
        bail!("Configuration <app:permissionMessage> entries are not compiled yet");
    }
    Ok(used)
}

/// Field 53: `{2,<n>,{<id>,<flag>}...,<tail>}`. The 60-field shape keeps 37
/// pairs and spends its tail scalar on the last functionality
/// (`TextToSpeech`); the longer shapes keep all 38 pairs and a tail of 0
/// (the count of permission messages).
fn functionality_table(used: &BTreeSet<u32>, shape: ConfigurationShape) -> Brace {
    let pairs = match shape {
        ConfigurationShape::V67 => &FUNCTIONALITIES[..FUNCTIONALITIES.len() - 1],
        _ => &FUNCTIONALITIES[..],
    };
    let mut items = vec![Brace::num(2), Brace::num(pairs.len() as i64)];
    for (id, _) in pairs {
        items.push(brace_list![
            Brace::num(*id as i64),
            Brace::flag(used.contains(id))
        ]);
    }
    items.push(match shape {
        ConfigurationShape::V67 => Brace::flag(used.contains(&41)),
        _ => Brace::num(0),
    });
    Brace::List(items)
}

/// The platform's older permission ids a functionality implies (fields 40
/// and 51 carry them; no XML prints them, the platform derives them).
/// Measured on ERP УХ, БСП 8.3.27 and 8.5, the native evidence and 16
/// CF-stored configurations. Proven singly: Biometrics 29, Camera 22,
/// AudioPlaybackAndVibration 26, OSBackup 28, Videoconferences 33, NFC 34,
/// IncomingShareRequests 37, NumberDialing {13,16}. Proven only as groups
/// (always set together in every corpus), split here by name:
/// {BluetoothPrinters, WiFiPrinters, PictureAndVideoLibraries} ->
/// {7,25,30,31}; {PushNotifications, LocalNotifications} -> {5,6,35};
/// {CallProcessing, CallLog, DocumentScanning} -> {14,17,18}; {Microphone,
/// MusicLibrary} -> {23,24}; {BackgroundAudioPlaybackAndVibration,
/// InstallPackages} -> {27}.
fn permissions_of(functionalities: &BTreeSet<u32>) -> BTreeSet<u32> {
    const MAP: &[(u32, &[u32])] = &[
        (0, &[29]),
        (3, &[30]),
        (4, &[31]),
        (7, &[5, 35]),
        (8, &[6]),
        (12, &[13, 16]),
        (13, &[14]),
        (14, &[17]),
        (18, &[22]),
        (19, &[23]),
        (20, &[24]),
        (21, &[7, 25]),
        (22, &[26]),
        (23, &[27]),
        (24, &[27]),
        (25, &[28]),
        (34, &[33]),
        (35, &[34]),
        (36, &[18]),
        (39, &[37]),
    ];
    let mut permissions = BTreeSet::new();
    for (functionality, implied) in MAP {
        if functionalities.contains(functionality) {
            permissions.extend(implied.iter().copied());
        }
    }
    permissions
}

/// Field 51: `{2,<n>,{<id>,<flag>,0}...}` over ids 1..38 but 4 and 15; 13
/// and 14 are listed only when set.
fn permission_flags(permissions: &BTreeSet<u32>) -> Brace {
    let ids = (1..=38u32)
        .filter(|id| !matches!(id, 4 | 15))
        .filter(|id| !matches!(id, 13 | 14) || permissions.contains(id))
        .collect::<Vec<_>>();
    let mut items = vec![Brace::num(2), Brace::num(ids.len() as i64)];
    for id in ids {
        items.push(brace_list![
            Brace::num(id as i64),
            Brace::flag(permissions.contains(&id)),
            Brace::num(0),
        ]);
    }
    Brace::List(items)
}

/// Field 40: the same flags for ids up to 31 as a typed-value map, in the
/// map's own bucket order; `None` is the map's `Undefined` key. 13 (after
/// 17) and 14 (after 9) join it only when set, and either one moves 20
/// before 7 -- every CF and database row shows exactly these three orders.
fn permission_table(permissions: &BTreeSet<u32>) -> Brace {
    const BASE: [Option<u32>; 28] = [
        Some(28),
        None,
        Some(25),
        Some(30),
        Some(24),
        Some(22),
        Some(21),
        Some(19),
        Some(18),
        Some(17),
        Some(12),
        Some(27),
        Some(10),
        Some(31),
        Some(26),
        Some(9),
        Some(8),
        Some(7),
        Some(20),
        Some(29),
        Some(16),
        Some(6),
        Some(5),
        Some(3),
        Some(2),
        Some(11),
        Some(23),
        Some(1),
    ];
    let with_13 = permissions.contains(&13);
    let with_14 = permissions.contains(&14);
    let mut order = Vec::with_capacity(BASE.len() + 2);
    for id in BASE {
        match id {
            Some(7) if with_13 || with_14 => order.push(Some(20)),
            Some(20) if with_13 || with_14 => order.push(Some(7)),
            other => order.push(other),
        }
        if id == Some(17) && with_13 {
            order.push(Some(13));
        }
        if id == Some(9) && with_14 {
            order.push(Some(14));
        }
    }
    let mut items = vec![Brace::num(order.len() as i64)];
    for id in &order {
        let key = match id {
            Some(id) => brace_list![
                Brace::str("#"),
                Brace::atom(PERMISSION_CLASS_ID),
                Brace::num(*id as i64),
            ],
            None => brace_list![Brace::str("U")],
        };
        let on = id.is_some_and(|id| permissions.contains(&id));
        items.push(brace_list![
            key,
            brace_list![Brace::str("B"), Brace::flag(on)]
        ]);
    }
    Brace::List(items)
}

/// Field 59: `{<n>,{"#",<type>,{0,"<mime>","<uti>","<ext>",<variant>,<custom>}}...}`.
fn share_request_types(p: &Element) -> Result<Brace> {
    let values = p
        .child("AllowedIncomingShareRequestTypes")
        .map(|list| list.children_named("Value").collect::<Vec<_>>())
        .unwrap_or_default();
    let mut items = vec![Brace::num(values.len() as i64)];
    for value in values {
        let custom = match value.child_text("isCustom").map(str::trim) {
            None | Some("") => false,
            Some(text) => parse_bool(text)?,
        };
        let variant = value
            .child_text("processingVariant")
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .unwrap_or("0");
        items.push(brace_list![
            Brace::str("#"),
            Brace::atom(SHARE_REQUEST_TYPE_CLASS_ID),
            brace_list![
                Brace::num(0),
                Brace::str(value.child_text("mime").unwrap_or_default()),
                Brace::str(value.child_text("uti").unwrap_or_default()),
                Brace::str(value.child_text("ext").unwrap_or_default()),
                Brace::atom(variant),
                Brace::flag(custom),
            ],
        ]);
    }
    Ok(Brace::List(items))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_compatibility_and_picks_the_shape() {
        assert_eq!(pack_version("Version8_3_24"), Some(80324));
        assert_eq!(pack_version("Version8_5_1"), Some(80501));
        assert_eq!(pack_version("DontUse"), None);
        assert_eq!(
            ConfigurationShape::for_compatibility(80324),
            ConfigurationShape::V67
        );
        assert_eq!(
            ConfigurationShape::for_compatibility(80327),
            ConfigurationShape::V68
        );
        assert_eq!(
            ConfigurationShape::for_compatibility(80501),
            ConfigurationShape::V76
        );
    }

    #[test]
    fn palette_colours_bring_the_8_5_1_feature() {
        let xml = |children: &str| {
            format!(
                "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"2.21\"><Configuration uuid=\"66193438-ABC5-410b-a1f1-a204102d1a62\"><Properties><Name>Б</Name><CompatibilityMode>Version8_5_1</CompatibilityMode></Properties><ChildObjects>{children}</ChildObjects></Configuration></MetaDataObject>"
            )
        };
        let with = configuration_facts(
            xml("<Language>Русский</Language><PaletteColor>Фон</PaletteColor>").as_bytes(),
        )
        .unwrap();
        assert_eq!(with.features, vec!["2dd2d9e1-40c8-430b-a433-a81ec6856ab0"]);
        assert_eq!(with.uuid, "66193438-abc5-410b-a1f1-a204102d1a62");
        let without = configuration_facts(xml("<Language>Русский</Language>").as_bytes()).unwrap();
        assert!(without.features.is_empty());
    }

    #[test]
    fn service_rows_match_the_stored_layout() {
        let facts = ConfigurationFacts {
            uuid: "66193438-abc5-410b-a1f1-a204102d1a62".into(),
            compatibility: 80324,
            shape: ConfigurationShape::V67,
            features: Vec::new(),
        };
        assert_eq!(
            root_row(&facts),
            "\u{feff}{2,66193438-abc5-410b-a1f1-a204102d1a62,}".as_bytes()
        );
        assert_eq!(
            version_row(&facts).unwrap(),
            "\u{feff}{\r\n{216,0,\r\n{80324,0}\r\n}\r\n}".as_bytes()
        );
        // БСП 8.5's stored row: its two palette colours bring the feature.
        let facts_8_5_1 = ConfigurationFacts {
            compatibility: 80501,
            shape: ConfigurationShape::V76,
            features: vec!["2dd2d9e1-40c8-430b-a433-a81ec6856ab0"],
            ..facts.clone()
        };
        assert_eq!(
            version_row(&facts_8_5_1).unwrap(),
            "\u{feff}{\r\n{217,0,\r\n{80501,1,\r\n{2dd2d9e1-40c8-430b-a433-a81ec6856ab0}\r\n}\r\n}\r\n}"
                .as_bytes()
        );
        let bare = ConfigurationFacts {
            features: Vec::new(),
            ..facts_8_5_1.clone()
        };
        assert_eq!(
            version_row(&bare).unwrap(),
            "\u{feff}{\r\n{217,0,\r\n{80501,0}\r\n}\r\n}".as_bytes()
        );
        let unmeasured = ConfigurationFacts {
            compatibility: 80502,
            ..facts_8_5_1.clone()
        };
        assert!(version_row(&unmeasured).is_err());
        // The 8.5 tuple at compatibility 8.3.27 keeps the 8.3 format, as
        // 8.5.1.1529 saved `home_page/one_column_v85/input.cf`.
        let tuple_8_5_at_8_3_27 = ConfigurationFacts {
            compatibility: 80327,
            features: Vec::new(),
            ..facts_8_5_1
        };
        assert_eq!(
            version_row(&tuple_8_5_at_8_3_27).unwrap(),
            "\u{feff}{\r\n{216,0,\r\n{80327,0}\r\n}\r\n}".as_bytes()
        );
        let mut counter = 0;
        let versions = versions_row(&["b".to_string(), "a.0".to_string()], || {
            counter += 1;
            format!("00000000-0000-0000-0000-{counter:012}")
        });
        assert_eq!(
            String::from_utf8(versions).unwrap(),
            "\u{feff}{1,6,\"\",00000000-0000-0000-0000-000000000001,\"a.0\",00000000-0000-0000-0000-000000000002,\"b\",00000000-0000-0000-0000-000000000003,\"root\",00000000-0000-0000-0000-000000000004,\"version\",00000000-0000-0000-0000-000000000005,\"versions\",00000000-0000-0000-0000-000000000006}"
        );
    }
}
