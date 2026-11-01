//! The root `Configuration` object of an extension.
//!
//! The record is the ordinary configuration properties tuple with the same
//! member positions (`{63,...}` since 8.3.21, `{68,...}` since 8.3.24). The
//! members that matter here, each set apart by a platform probe that changed
//! one value (upstream PR 387, `external/extension_roots`):
//!
//! | member | holds                                                      |
//! |--------|------------------------------------------------------------|
//! | 3      | `ScriptVariant`: 0 English, 1 Russian                      |
//! | 4-8    | detailed, brief, copyright, vendor and configuration texts |
//! | 10     | the default language                                       |
//! | 14, 15 | `Vendor`, `Version`                                        |
//! | 21     | `DefaultRunMode`: 0 ordinary, 1 managed, 2 auto            |
//! | 26     | the configuration's compatibility mode                     |
//! | 33     | `UsePurposes`                                              |
//! | 36     | `ModalityUseMode`: 0 use, 1 use with warnings, 2 don't use |
//! | 38     | `InterfaceCompatibilityMode`                               |
//! | 39     | `DefaultRoles`                                             |
//! | 42     | `NamePrefix`                                               |
//! | 43     | `ConfigurationExtensionCompatibilityMode` (packed version) |
//! | 44     | purpose: 0 patch, 1 customization, 2 add-on                |
//! | 49     | keep the mapping to the extended objects by identity: 0, 1 |
//!
//! (Member 41 is `2` in every extension on record and in every probe; the
//! first pass took it for the mapping flag and read the run mode from member 3,
//! which happen to agree with members 49 and 21 on the two extensions and the
//! two platforms measured.)
//!
//! The platform prints a fixed set of them beside the header (the properties
//! of an extension of its own) and, from the adopted header of the root, the
//! properties the extension controls (a listed property at state 0 is not
//! controlled and prints nothing).
//!
//! Platform 8.5.1 stores an extension it has converted in the `{76,...}` tuple:
//! the `{68,...}` members at the same positions, then the 16 members of the 8.5
//! properties (61-68 the enumerations, the captions in 64 and 65, the eight
//! auxiliary forms in 69-76). It prints `Caption` and `ShortCaption` after the
//! version for every extension, converted or not (an old tuple has none: both
//! empty), and spells the compatibility of `80501` as `Version8_5_1`.

use std::collections::BTreeMap;

use super::properties::{Meaning, meaning};
use super::{ExtensionContext, STATE_EXTENDED};
use crate::legacy_version::InfobaseConfigSourceVersion;
use crate::mssql_dump::{
    format_full_metadata_source_xml, insert_configuration_internal_info_xml,
    insert_configuration_root_child_objects_xml, insert_metadata_properties_xml,
    parse_1c_quoted_string, parse_1c_synonyms, push_localized_property,
    push_optional_simple_property_xml, refs,
};

/// `Configuration.xml` of an extension, or `None` (the row stays opaque) when
/// the record is not one this reader can name every printed member of.
pub(crate) fn extension_root_xml(
    context: &ExtensionContext,
    text: &str,
    uuid: &str,
    object_refs: &BTreeMap<String, String>,
    source_version: InfobaseConfigSourceVersion,
) -> Option<String> {
    let parts = refs::extension_root_parts(text, uuid, object_refs)?;
    let adopted = context.adopted(&parts.header_uuid)?;
    let fields = &parts.fields;
    let field = |index: usize| fields.get(index).map(String::as_str);

    // What the header lists.
    let mut adopted_names = Vec::<&'static str>::new();
    let mut states = Vec::<(&'static str, &'static str)>::new();
    let mut asset_members = 0usize;
    let mut unknown = Vec::new();
    for (guid, state) in &adopted.properties {
        let Some(meaning) = meaning("Configuration", guid) else {
            // Keep the fields whose identities are known, but never report
            // this partial root as a successful, completely decoded object.
            unknown.push(guid.as_str());
            continue;
        };
        match meaning {
            Meaning::Property(name) => {
                if *state != STATE_NOT_CONTROLLED {
                    adopted_names.push(name);
                }
            }
            Meaning::Block(name) => {
                if *state == STATE_EXTENDED {
                    states.push((name, "Extended"));
                }
            }
            Meaning::Hidden(_) => {}
            Meaning::Group("RootExtAssets") => {
                // Each of the three is a changed block (state 3).
                if *state != STATE_EXTENDED {
                    return None;
                }
                asset_members += 1;
            }
            Meaning::Group(_) | Meaning::Multi(_) | Meaning::ExtendedObject => return None,
        }
    }
    // Whatever the order of the header list, the platform writes the states of
    // the root's modules and command interfaces in one order.
    sort_root_states(&mut states);
    // The three ids of the asset group appear whole or not at all; whole, the
    // platform prints their states after the module's, in this order.
    match asset_members {
        0 => {}
        3 => states.extend([
            ("HomePageWorkArea", "Extended"),
            ("Logo", "Extended"),
            ("Splash", "Extended"),
        ]),
        _ => return None,
    }

    let controlled = |name: &str| adopted_names.contains(&name);
    let mut insert = String::new();
    let purpose = match field(44)? {
        "0" => "Patch",
        "1" => "Customization",
        "2" => "AddOn",
        _ => return None,
    };
    push_optional_simple_property_xml(&mut insert, "ConfigurationExtensionPurpose", Some(purpose));
    let keep_mapping = match field(49)? {
        "0" => "false",
        "1" => "true",
        _ => return None,
    };
    push_optional_simple_property_xml(
        &mut insert,
        "KeepMappingToExtendedConfigurationObjectsByIDs",
        Some(keep_mapping),
    );
    let name_prefix = parse_1c_quoted_string(field(42)?)?;
    push_optional_simple_property_xml(&mut insert, "NamePrefix", Some(&name_prefix));
    let compatibility = refs::configuration_compatibility_mode_xml_for(field(43)?, source_version)?;
    let tuple_8_5_1 = source_version == InfobaseConfigSourceVersion::V2_21 && fields.len() == 77;
    if let Ok(packed_version) = field(43)?.parse::<u32>() {
        context.note_compatibility(packed_version);
    }
    push_optional_simple_property_xml(
        &mut insert,
        "ConfigurationExtensionCompatibilityMode",
        Some(&compatibility),
    );
    if controlled("DefaultRunMode") {
        let default_run_mode = match field(21)? {
            "0" => "OrdinaryApplication",
            "1" => "ManagedApplication",
            "2" => "Auto",
            _ => return None,
        };
        push_optional_simple_property_xml(&mut insert, "DefaultRunMode", Some(default_run_mode));
    }
    if controlled("UsePurposes") {
        insert.push_str(&use_purposes_xml(field(33)?)?);
    }
    let script_variant = match field(3)? {
        "0" => "English",
        "1" => "Russian",
        _ => return None,
    };
    push_optional_simple_property_xml(&mut insert, "ScriptVariant", Some(script_variant));
    if controlled("DefaultRoles") {
        let roles = default_roles(field(39)?, object_refs)?;
        if roles.is_empty() {
            insert.push_str("\t\t\t<DefaultRoles/>\r\n");
        } else {
            insert.push_str("\t\t\t<DefaultRoles>\r\n");
            for role in &roles {
                insert.push_str(&format!(
                    "\t\t\t\t<xr:Item xsi:type=\"xr:MDObjectRef\">{role}</xr:Item>\r\n"
                ));
            }
            insert.push_str("\t\t\t</DefaultRoles>\r\n");
        }
    }
    push_optional_simple_property_xml(
        &mut insert,
        "Vendor",
        Some(&parse_1c_quoted_string(field(14)?)?),
    );
    push_optional_simple_property_xml(
        &mut insert,
        "Version",
        Some(&parse_1c_quoted_string(field(15)?)?),
    );
    if source_version == InfobaseConfigSourceVersion::V2_21 {
        let (caption, short_caption) = if tuple_8_5_1 {
            (parse_1c_synonyms(field(64)?), parse_1c_synonyms(field(65)?))
        } else {
            (Vec::new(), Vec::new())
        };
        push_localized_property(&mut insert, "\t\t\t", "Caption", &caption);
        push_localized_property(&mut insert, "\t\t\t", "ShortCaption", &short_caption);
    }
    if controlled("DefaultLanguage") {
        let language = object_refs.get(field(10)?)?;
        if !language.starts_with("Language.") {
            return None;
        }
        push_optional_simple_property_xml(&mut insert, "DefaultLanguage", Some(language));
    }
    // The localized texts: members 4 (detailed), 5 (brief), 6 (copyright),
    // 7 (vendor address), 8 (configuration address).
    for (name, index) in [
        ("BriefInformation", 5),
        ("DetailedInformation", 4),
        ("Copyright", 6),
        ("VendorInformationAddress", 7),
        ("ConfigurationInformationAddress", 8),
    ] {
        push_localized_property(
            &mut insert,
            "\t\t\t",
            name,
            &parse_1c_synonyms(field(index)?),
        );
    }
    if controlled("ModalityUseMode") {
        let modality = match field(36)? {
            "0" => "Use",
            "1" => "UseWithWarnings",
            "2" => "DontUse",
            _ => return None,
        };
        push_optional_simple_property_xml(&mut insert, "ModalityUseMode", Some(modality));
    }
    if controlled("InterfaceCompatibilityMode") {
        // 8.5 reads both stored interface coordinates of a converted tuple;
        // the 8.3 spelling follows member 38 alone.
        let interface = if tuple_8_5_1 {
            // Native V76 stores a pair: (3,3) Taxi, (3,6) 8.5+Taxi,
            // (2,2) Taxi+8.2. Use the same evidenced authority as CF roots.
            refs::configuration_v76_interface_mode(text, uuid)
                .ok()??
                .xml_name()
        } else {
            match field(38)? {
                "0" => "Version8_2",
                "1" => "Version8_2EnableTaxi",
                "2" => "TaxiEnableVersion8_2",
                "3" => "Taxi",
                _ => return None,
            }
        };
        push_optional_simple_property_xml(
            &mut insert,
            "InterfaceCompatibilityMode",
            Some(interface),
        );
    }
    if controlled("CompatibilityMode") {
        // The configuration's own compatibility mode, member 26.
        let own = refs::configuration_compatibility_mode_xml_for(field(26)?, source_version)?;
        push_optional_simple_property_xml(&mut insert, "CompatibilityMode", Some(&own));
    }

    let mut xml = format_full_metadata_source_xml("Configuration", &parts.header, source_version);
    // ObjectBelonging opens the properties.
    let properties_open = xml.find("\t\t<Properties>\r\n")? + "\t\t<Properties>\r\n".len();
    xml.insert_str(
        properties_open,
        "\t\t\t<ObjectBelonging>Adopted</ObjectBelonging>\r\n",
    );
    insert_metadata_properties_xml(&mut xml, &insert);
    insert_configuration_internal_info_xml(&mut xml, &parts.contained_objects).ok()?;
    if !states.is_empty() {
        let mut property_states = String::new();
        for (name, state) in &states {
            property_states.push_str(&format!(
                "\t\t\t<xr:PropertyState>\r\n\t\t\t\t<xr:Property>{name}</xr:Property>\r\n\t\t\t\t<xr:State>{state}</xr:State>\r\n\t\t\t</xr:PropertyState>\r\n"
            ));
        }
        let close = xml.find("\t\t</InternalInfo>\r\n")?;
        xml.insert_str(close, &property_states);
    }
    insert_configuration_root_child_objects_xml(&mut xml, parts.child_objects.as_deref()?);
    if !unknown.is_empty() {
        context.note_diagnostic(uuid, format!(
            "extension root {uuid}: controlled properties {} are not known; only known root properties retained",
            unknown.join(", ")
        ));
    }
    Some(xml)
}

/// The order the platform writes the states of the root's blocks in, whatever
/// the order of the header list (fixtures `external/extension_roots/modules`).
const ROOT_STATE_ORDER: [&str; 6] = [
    "ManagedApplicationModule",
    "SessionModule",
    "ExternalConnectionModule",
    "OrdinaryApplicationModule",
    "CommandInterface",
    "MainSectionCommandInterface",
];

fn sort_root_states(states: &mut [(&'static str, &'static str)]) {
    states.sort_by_key(|(name, _)| {
        ROOT_STATE_ORDER
            .iter()
            .position(|known| known == name)
            .unwrap_or(usize::MAX)
    });
}

/// A listed property the extension does not control (see `properties`).
const STATE_NOT_CONTROLLED: u8 = 0;

/// `{N,{"#",1708fdaa-...,<purpose>}...}` as the `UsePurposes` element: `1` is
/// the platform application, `2` the mobile one.
fn use_purposes_xml(field: &str) -> Option<String> {
    let list = crate::mssql_dump::split_1c_braced_fields(field, 0)?;
    let count: usize = list.first()?.trim().parse().ok()?;
    if count == 0 || list.len() != count + 1 {
        return None;
    }
    let mut xml = String::from("\t\t\t<UsePurposes>\r\n");
    for entry in &list[1..] {
        let typed = crate::mssql_dump::split_1c_braced_fields(entry.trim(), 0)?;
        if typed.len() != 3
            || parse_1c_quoted_string(typed[0].trim()).as_deref() != Some("#")
            || typed[1].trim() != "1708fdaa-cbce-4289-b373-07a5a74bee91"
        {
            return None;
        }
        let purpose = match typed[2].trim() {
            "1" => "PlatformApplication",
            "2" => "MobilePlatformApplication",
            _ => return None,
        };
        xml.push_str(&format!(
            "\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">{purpose}</v8:Value>\r\n"
        ));
    }
    xml.push_str("\t\t\t</UsePurposes>\r\n");
    Some(xml)
}

/// `{0,N,{"#",157fa490-...,{1,<role uuid>}}...}` as `Role.Name` references.
fn default_roles(field: &str, object_refs: &BTreeMap<String, String>) -> Option<Vec<String>> {
    let list = crate::mssql_dump::split_1c_braced_fields(field, 0)?;
    if list.first()?.trim() != "0" {
        return None;
    }
    let count: usize = list.get(1)?.trim().parse().ok()?;
    if list.len() != count + 2 {
        return None;
    }
    let mut roles = Vec::with_capacity(count);
    for entry in &list[2..] {
        let typed = crate::mssql_dump::split_1c_braced_fields(entry.trim(), 0)?;
        if typed.len() != 3
            || parse_1c_quoted_string(typed[0].trim()).as_deref() != Some("#")
            || typed[1].trim() != "157fa490-4ce9-11d4-9415-008048da11f9"
        {
            return None;
        }
        let target = crate::mssql_dump::split_1c_braced_fields(typed[2].trim(), 0)?;
        if target.len() != 2 || target[0].trim() != "1" {
            return None;
        }
        let name = object_refs.get(target[1].trim())?;
        if !name.starts_with("Role.") {
            return None;
        }
        roles.push(name.clone());
    }
    Some(roles)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The root of the `modules` probe lists its blocks in the order
    /// `7f676314`, `a4a9c1e2`, `9dfcabbf`, `d22e852a`, `a78d9ce3`, `9b7bbbae`;
    /// the platform prints their states in the fixed order.
    #[test]
    fn the_states_of_the_root_blocks_come_out_in_the_platforms_order() {
        let mut states = vec![
            ("CommandInterface", "Extended"),
            ("ExternalConnectionModule", "Extended"),
            ("MainSectionCommandInterface", "Extended"),
            ("ManagedApplicationModule", "Extended"),
            ("OrdinaryApplicationModule", "Extended"),
            ("SessionModule", "Extended"),
        ];
        sort_root_states(&mut states);
        assert_eq!(
            states.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            [
                "ManagedApplicationModule",
                "SessionModule",
                "ExternalConnectionModule",
                "OrdinaryApplicationModule",
                "CommandInterface",
                "MainSectionCommandInterface",
            ]
        );
    }

    #[test]
    fn use_purposes_list_the_applications_in_stored_order() {
        const CLASS: &str = "1708fdaa-cbce-4289-b373-07a5a74bee91";
        let both = format!("{{2,{{\"#\",{CLASS},1}},{{\"#\",{CLASS},2}}}}");
        assert_eq!(
            use_purposes_xml(&both).unwrap(),
            "\t\t\t<UsePurposes>\r\n\
\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">PlatformApplication</v8:Value>\r\n\
\t\t\t\t<v8:Value xsi:type=\"app:ApplicationUsePurpose\">MobilePlatformApplication</v8:Value>\r\n\
\t\t\t</UsePurposes>\r\n"
        );
        // The `spellings` probe stores the mobile application alone.
        let mobile = format!("{{1,{{\"#\",{CLASS},2}}}}");
        assert!(
            use_purposes_xml(&mobile)
                .unwrap()
                .contains(">MobilePlatformApplication<")
        );
        // An empty list, a foreign class and an unknown purpose are refused.
        assert!(use_purposes_xml("{0}").is_none());
        let foreign = "{1,{\"#\",00000000-0000-0000-0000-000000000000,1}}";
        assert!(use_purposes_xml(foreign).is_none());
        let unknown = format!("{{1,{{\"#\",{CLASS},3}}}}");
        assert!(use_purposes_xml(&unknown).is_none());
    }
}
