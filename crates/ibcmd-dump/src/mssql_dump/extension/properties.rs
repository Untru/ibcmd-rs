//! The property ids an adopted object's md header lists, by the XML element
//! of the object that carries them.
//!
//! Evidence: the four 8.3.27 БСП extensions (`_ДемоПустоеРасширение`,
//! `_ДемоРасширение`, `ServiceDesk`, `VAExtension`) against their native
//! exports. A id whose element it names is *measured* when every object of the
//! corpus that lists it prints exactly that property; an id inside a group that
//! always appears whole (the six flags of a common module, the three localized
//! strings of a common form, ...) is *grouped*: the group is measured, the
//! assignment inside it follows the order of the XML properties and is
//! confirmed by the native round trip only as far as the corpus reaches.

/// What an id in an adopted header stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Meaning {
    /// An ordinary property: the native export prints it in the object's
    /// `<Properties>` (with the value the row stores).
    Property(&'static str),
    /// A module, form, rights or command-interface block. The block is not a
    /// property; the native export prints its state under `<InternalInfo>`.
    Block(&'static str),
    /// A property the extension can add values to. State 3 prints as
    /// `MultiState` with an `xr:ExtendedProperty` value; state 2 as an
    /// ordinary property.
    Multi(&'static str),
    /// `ExtendedConfigurationObject`.
    ExtendedObject,
    /// An overridden block the platform lists in the header but prints
    /// nowhere (its content is empty or it has no XML form).
    Hidden(&'static str),
    /// One member of a property group that always appears whole; the ids of
    /// the group are known, their assignment to the group's properties is not.
    Group(&'static str),
}

const TYPE_PROPERTY: &str = "b1053250-abe6-11d4-9434-004095e12fc7";
const SYNONYM_PROPERTY: &str = "cf4abea3-37b2-11d4-940f-008048da11f9";

/// The meaning of property `guid` on an element called `element`
/// (`Catalog`, `Attribute`, `CommonModule`, ...), if it is known.
pub(crate) fn meaning(element: &str, guid: &str) -> Option<Meaning> {
    use Meaning::{Block, ExtendedObject, Group, Multi, Property};
    match guid {
        "9595ddd6-e72c-47ad-a156-672db811628c" => return Some(ExtendedObject),
        // `Type` of an attribute, resource, dimension, defined type, ...
        TYPE_PROPERTY => return Some(Multi("Type")),
        // One id names the synonym whatever the class (upstream PR 387,
        // `external/adopted/props_all`: a document lists it too).
        SYNONYM_PROPERTY => return Some(Property("Synonym")),
        _ => {}
    }
    Some(match (element, guid) {
        ("AccumulationRegister", "bd533460-4001-11d6-a3c7-0050bae0a776") => {
            Property("RegisterType")
        }
        // Catalog. The ids of the code type and length were read off the
        // stored `{9d, 9e, 9f}` run in the first pass and came out swapped; the
        // platform probes that set one property apart (upstream PR 387, fixtures
        // `external/adopted/props_b0..b2`) show `9d` beside `CodeLength` only
        // and `9e` beside `CodeType` only.
        ("Catalog", "37f2fa9d-b276-11d4-9435-004095e12fc7") => Property("CodeLength"),
        ("Catalog", "37f2fa9e-b276-11d4-9435-004095e12fc7") => Property("CodeType"),
        ("Catalog", "37f2fa9f-b276-11d4-9435-004095e12fc7") => Property("DescriptionLength"),
        ("Catalog", "bf9cc511-eb2a-48b6-a666-7afb78b83f36") => Property("CodeAllowedLength"),
        // Set apart by the same probes (fixtures `external/adopted/props_*`).
        ("Catalog", "b69601f0-4cf6-11d4-9415-008048da11f9") => Property("Owners"),
        ("Catalog", "b0ec341d-eede-4bb4-b303-ed50020b4c7f") => Property("Hierarchical"),
        ("Catalog", "a865d4e5-8584-4894-8e49-4b4206f98f5a") => Property("HierarchyType"),
        // The modules and predefined items an adopted object extends
        // (`external/adopted/{catalog_modules,catalog_object_module,
        // document_children,predefined,exchange_plan}`).
        // Native adopted/predefined and adopted/exchange_plan fixtures:
        // these states belong to their asset blocks, not scalar properties.
        ("Catalog", "f440939a-f130-413b-9c9a-2b18c4af69c6") => Block("Predefined"),
        ("ExchangePlan", "ee705b5d-dd16-46e4-b613-3a7edba5ba84") => Block("Content"),
        ("Catalog", "a637f77f-3840-441d-a1c3-699c8c5cb7e0") => Block("ObjectModule"),
        ("Catalog", "d1b64a2c-8078-4982-8190-8f81aefda192") => Block("ManagerModule"),
        ("Document", "60643a07-120a-4a63-9dba-67369c0f0145") => Property("NumberType"),
        ("Document", "490eb6bd-24c4-4943-82aa-d0c4b666861b") => Property("NumberLength"),
        ("Document", "c6c9689d-2978-42e8-863f-0280c6a85b56") => Property("NumberAllowedLength"),
        ("Document", "a637f77f-3840-441d-a1c3-699c8c5cb7e0") => Block("ObjectModule"),
        ("Form", "32e087ab-1491-49b6-aba7-43571b41ac2b") => Block("Form"),
        ("Form", "e3331ed0-3854-478d-b6b5-4f14acdd6edb") => Property("FormType"),
        ("EventSubscription", "41721fe8-6dcb-4585-9ec2-5577d430cf6b") => Property("Source"),
        ("FilterCriterion", "e1e8ea40-0906-11d6-a3c7-0050bae0a776") => Property("Content"),
        ("StyleItem", "36e0f718-e0ba-4369-af8b-aff9a4ad7a35") => Property("Value"),
        ("CommonModule", "07ddee68-6fc0-4b88-9616-7792446d12b8") => Property("ReturnValuesReuse"),
        ("CommonCommand", "482411f7-457f-4889-a7c9-9adbfb1c7bd4") => Property("Group"),
        ("Command", "482411f7-457f-4889-a7c9-9adbfb1c7bd4") => Property("Group"),
        // 8.5: an adopted command of a register lists it (state 2, printed empty).
        ("Command", "7d14f63a-87e8-4188-a28b-02da93f6bcbd") => Multi("CommandParameterType"),
        ("CommonCommand", "7d14f63a-87e8-4188-a28b-02da93f6bcbd") => Multi("CommandParameterType"),
        ("CommonForm", "32e087ab-1491-49b6-aba7-43571b41ac2b") => Block("Form"),
        ("CommonForm", "e3331ed0-3854-478d-b6b5-4f14acdd6edb") => Property("FormType"),
        ("CommonForm", "c9ba86bf-a1e7-440f-9450-adb61f886f88") => Property("ExtendedPresentation"),
        ("CommonForm", "719346a1-02d5-4311-a141-4d22470a7ac3") => Property("Explanation"),
        ("CommonModule", "d5963243-262e-4398-b4d7-fb16d06484f6") => Block("Module"),
        ("CommonModule", "7dbb2bc7-6bae-4b81-91cb-681317272a0b") => Property("Global"),
        ("CommonModule", "74ce8a02-abd2-46a6-8544-8cfbb4e8c6e0") => {
            Property("ClientManagedApplication")
        }
        ("CommonModule", "6275a02e-96f0-4347-975a-2d661e6a0675") => Property("Server"),
        ("CommonModule", "d12660a6-7298-4ae2-a332-b95a6459a280") => Property("ExternalConnection"),
        ("CommonModule", "436af77a-e846-4084-818b-740a3378518e") => {
            Property("ClientOrdinaryApplication")
        }
        ("CommonModule", "c474bab9-d13a-4fbd-bfb0-9214d6dc2fde") => Property("ServerCall"),
        ("InformationRegister", "9f36fd70-4bf4-47f6-b235-935f73aab43f") => Block("RecordSetModule"),
        ("InformationRegister", "d1b64a2c-8078-4982-8190-8f81aefda192") => Block("ManagerModule"),
        // The same probes: `09c412e0` alone is `WriteMode`, `13134205` is the
        // periodicity (the first pass had them the other way round).
        ("InformationRegister", "09c412e0-0f30-11d6-a3c7-0050bae0a776") => Property("WriteMode"),
        ("InformationRegister", "13134205-f60b-11d5-a3c7-0050bae0a776") => {
            Property("InformationRegisterPeriodicity")
        }
        ("Language", "4b2a2bcf-a845-41ba-a03d-05b09a2c6c11") => Property("LanguageCode"),
        ("Role", "3245b9fe-57f9-43dc-9c11-030f0617ce16") => Block("Rights"),
        ("StyleItem", "1ab4ba03-e4e6-4bf0-93e9-fcc28cc67567") => Property("Type"),
        ("Subsystem", "c6690627-40cf-4741-8719-4e7feb832b84") => Property("Content"),
        ("Subsystem", "7f676314-716a-4d54-8335-a71a3857b21c") => Block("CommandInterface"),
        // The extension's own configuration object.
        ("Configuration", "7f676314-716a-4d54-8335-a71a3857b21c") => Block("CommandInterface"),
        ("Configuration", "9dfcabbf-6a7a-48aa-8721-df5e78b367c6") => {
            Block("MainSectionCommandInterface")
        }
        ("Configuration", "15e3462b-bc9b-40cd-9d9f-dc0922f84560") => Property("DefaultLanguage"),
        // The root's controllable properties and modules, each set apart by a
        // platform probe that changed one value (upstream PR 387, fixtures
        // `external/extension_roots/{values,spellings,modules,roles}`). The
        // first pass took `6a447e3f` for the managed application module and
        // `d22e852a` for a hidden block; they list together in the two
        // extensions on record, so the corpus did not tell them apart: the
        // platform lists `6a447e3f` when the extension sets the default roles
        // and `d22e852a` when it extends the managed application module.
        ("Configuration", "6a447e3f-d9d7-4c97-a239-87e3fe6d8055") => Property("DefaultRoles"),
        ("Configuration", "d22e852a-cf8a-4f77-8ccb-3548e7792bea") => {
            Block("ManagedApplicationModule")
        }
        ("Configuration", "9b7bbbae-9771-46f2-9e4d-2489e0ffc702") => Block("SessionModule"),
        ("Configuration", "a4a9c1e2-1e54-4c7f-af06-4ca341198fac") => {
            Block("ExternalConnectionModule")
        }
        ("Configuration", "a78d9ce3-4e0c-48d5-9863-ae7342eedf94") => {
            Block("OrdinaryApplicationModule")
        }
        ("Configuration", "3a5cca5c-0675-43df-80ca-2cf5163335f2") => Property("ModalityUseMode"),
        ("Configuration", "161bc4c0-4cc0-4382-a045-f58f25e24783") => Property("CompatibilityMode"),
        // HomePageWorkArea, Logo and Splash: an 8.5 extension that changes them
        // lists the three ids, each in state 3 (ServiceDesk, the only one on
        // record). Which id is which is not on record, so they are a group.
        ("Configuration", "d98a8e01-7219-41ce-9f23-dada0860b9bf")
        | ("Configuration", "740eb5f6-e214-4e30-aefa-f15bab91c688")
        | ("Configuration", "3035a9db-d6b2-450e-8b22-4430577f8dab") => Group("RootExtAssets"),
        // DefaultRunMode, UsePurposes and InterfaceCompatibilityMode list
        // together in ServiceDesk; each is set apart by the same probes.
        ("Configuration", "c6c6cdec-9de1-431f-b17a-e442eebf86c1") => Property("DefaultRunMode"),
        ("Configuration", "da648ef9-2f12-418f-8e2f-8956bc10a66f") => Property("UsePurposes"),
        ("Configuration", "cbd1f1ed-72d3-4da3-bd02-c643d9595b7d") => {
            Property("InterfaceCompatibilityMode")
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_ids_resolve_by_element() {
        assert_eq!(
            meaning("Language", "4b2a2bcf-a845-41ba-a03d-05b09a2c6c11"),
            Some(Meaning::Property("LanguageCode"))
        );
        assert_eq!(
            meaning("Attribute", "b1053250-abe6-11d4-9434-004095e12fc7"),
            Some(Meaning::Multi("Type"))
        );
        assert_eq!(
            meaning("Catalog", "9595ddd6-e72c-47ad-a156-672db811628c"),
            Some(Meaning::ExtendedObject)
        );
        assert_eq!(
            meaning("Language", "37f2fa9d-b276-11d4-9435-004095e12fc7"),
            None
        );
    }

    /// HomePageWorkArea, Logo and Splash: the three ids of an 8.5 extension that
    /// changes them appear together, and which is which is not on record.
    /// Each assignment below is a platform probe's, not a corpus reading: the
    /// property printed beside the listed id changes with the one value the
    /// probe changes (`external/adopted/props_*`, `external/extension_roots/*`
    /// of upstream PR 387).
    #[test]
    fn ids_that_the_corpus_could_not_tell_apart_follow_the_probes() {
        for (element, guid, expected) in [
            (
                "Catalog",
                "37f2fa9d-b276-11d4-9435-004095e12fc7",
                "CodeLength",
            ),
            (
                "Catalog",
                "37f2fa9e-b276-11d4-9435-004095e12fc7",
                "CodeType",
            ),
            (
                "InformationRegister",
                "09c412e0-0f30-11d6-a3c7-0050bae0a776",
                "WriteMode",
            ),
            (
                "InformationRegister",
                "13134205-f60b-11d5-a3c7-0050bae0a776",
                "InformationRegisterPeriodicity",
            ),
            (
                "Configuration",
                "6a447e3f-d9d7-4c97-a239-87e3fe6d8055",
                "DefaultRoles",
            ),
            (
                "Configuration",
                "c6c6cdec-9de1-431f-b17a-e442eebf86c1",
                "DefaultRunMode",
            ),
            (
                "Configuration",
                "da648ef9-2f12-418f-8e2f-8956bc10a66f",
                "UsePurposes",
            ),
            (
                "Configuration",
                "cbd1f1ed-72d3-4da3-bd02-c643d9595b7d",
                "InterfaceCompatibilityMode",
            ),
        ] {
            assert_eq!(
                meaning(element, guid),
                Some(Meaning::Property(expected)),
                "{element} {guid}"
            );
        }
        for (guid, expected) in [
            (
                "d22e852a-cf8a-4f77-8ccb-3548e7792bea",
                "ManagedApplicationModule",
            ),
            ("9b7bbbae-9771-46f2-9e4d-2489e0ffc702", "SessionModule"),
            (
                "a4a9c1e2-1e54-4c7f-af06-4ca341198fac",
                "ExternalConnectionModule",
            ),
            (
                "a78d9ce3-4e0c-48d5-9863-ae7342eedf94",
                "OrdinaryApplicationModule",
            ),
        ] {
            assert_eq!(
                meaning("Configuration", guid),
                Some(Meaning::Block(expected)),
                "{guid}"
            );
        }
    }

    #[test]
    fn the_8_5_root_asset_ids_form_one_group() {
        for guid in [
            "d98a8e01-7219-41ce-9f23-dada0860b9bf",
            "740eb5f6-e214-4e30-aefa-f15bab91c688",
            "3035a9db-d6b2-450e-8b22-4430577f8dab",
        ] {
            assert_eq!(
                meaning("Configuration", guid),
                Some(Meaning::Group("RootExtAssets")),
                "{guid}"
            );
            assert_eq!(meaning("Catalog", guid), None, "{guid} outside the root");
        }
    }

    /// An adopted command lists the parameter types it may take (8.5 ServiceDesk
    /// commands of registers).
    #[test]
    fn an_adopted_command_lists_its_parameter_types() {
        assert_eq!(
            meaning("Command", "7d14f63a-87e8-4188-a28b-02da93f6bcbd"),
            Some(Meaning::Multi("CommandParameterType"))
        );
    }
}

/// Inverse of the evidenced property mapping; shared by extension compilation.
#[doc(hidden)]
pub fn guid_for(element: &str, name: &str) -> Option<&'static str> {
    const IDS: &[&str] = &[
        "07ddee68-6fc0-4b88-9616-7792446d12b8",
        "09c412e0-0f30-11d6-a3c7-0050bae0a776",
        "13134205-f60b-11d5-a3c7-0050bae0a776",
        "15e3462b-bc9b-40cd-9d9f-dc0922f84560",
        "161bc4c0-4cc0-4382-a045-f58f25e24783",
        "1ab4ba03-e4e6-4bf0-93e9-fcc28cc67567",
        "3035a9db-d6b2-450e-8b22-4430577f8dab",
        "3245b9fe-57f9-43dc-9c11-030f0617ce16",
        "32e087ab-1491-49b6-aba7-43571b41ac2b",
        "36e0f718-e0ba-4369-af8b-aff9a4ad7a35",
        "37f2fa9d-b276-11d4-9435-004095e12fc7",
        "37f2fa9e-b276-11d4-9435-004095e12fc7",
        "37f2fa9f-b276-11d4-9435-004095e12fc7",
        "3a5cca5c-0675-43df-80ca-2cf5163335f2",
        "41721fe8-6dcb-4585-9ec2-5577d430cf6b",
        "436af77a-e846-4084-818b-740a3378518e",
        "482411f7-457f-4889-a7c9-9adbfb1c7bd4",
        "490eb6bd-24c4-4943-82aa-d0c4b666861b",
        "4b2a2bcf-a845-41ba-a03d-05b09a2c6c11",
        "60643a07-120a-4a63-9dba-67369c0f0145",
        "6275a02e-96f0-4347-975a-2d661e6a0675",
        "6a447e3f-d9d7-4c97-a239-87e3fe6d8055",
        "719346a1-02d5-4311-a141-4d22470a7ac3",
        "740eb5f6-e214-4e30-aefa-f15bab91c688",
        "74ce8a02-abd2-46a6-8544-8cfbb4e8c6e0",
        "7d14f63a-87e8-4188-a28b-02da93f6bcbd",
        "7dbb2bc7-6bae-4b81-91cb-681317272a0b",
        "7f676314-716a-4d54-8335-a71a3857b21c",
        "9595ddd6-e72c-47ad-a156-672db811628c",
        "9b7bbbae-9771-46f2-9e4d-2489e0ffc702",
        "9dfcabbf-6a7a-48aa-8721-df5e78b367c6",
        "9f36fd70-4bf4-47f6-b235-935f73aab43f",
        "a4a9c1e2-1e54-4c7f-af06-4ca341198fac",
        "a637f77f-3840-441d-a1c3-699c8c5cb7e0",
        "a78d9ce3-4e0c-48d5-9863-ae7342eedf94",
        "a865d4e5-8584-4894-8e49-4b4206f98f5a",
        "b0ec341d-eede-4bb4-b303-ed50020b4c7f",
        "b1053250-abe6-11d4-9434-004095e12fc7",
        "b69601f0-4cf6-11d4-9415-008048da11f9",
        "bd533460-4001-11d6-a3c7-0050bae0a776",
        "bf9cc511-eb2a-48b6-a666-7afb78b83f36",
        "c474bab9-d13a-4fbd-bfb0-9214d6dc2fde",
        "c6690627-40cf-4741-8719-4e7feb832b84",
        "c6c6cdec-9de1-431f-b17a-e442eebf86c1",
        "c6c9689d-2978-42e8-863f-0280c6a85b56",
        "c9ba86bf-a1e7-440f-9450-adb61f886f88",
        "cbd1f1ed-72d3-4da3-bd02-c643d9595b7d",
        "cf4abea3-37b2-11d4-940f-008048da11f9",
        "d12660a6-7298-4ae2-a332-b95a6459a280",
        "d1b64a2c-8078-4982-8190-8f81aefda192",
        "d22e852a-cf8a-4f77-8ccb-3548e7792bea",
        "d5963243-262e-4398-b4d7-fb16d06484f6",
        "d98a8e01-7219-41ce-9f23-dada0860b9bf",
        "da648ef9-2f12-418f-8e2f-8956bc10a66f",
        "e1e8ea40-0906-11d6-a3c7-0050bae0a776",
        "e3331ed0-3854-478d-b6b5-4f14acdd6edb",
        "ee705b5d-dd16-46e4-b613-3a7edba5ba84",
        "f440939a-f130-413b-9c9a-2b18c4af69c6",
    ];
    IDS.iter().copied().find(|id| match meaning(element, id) {
        Some(Meaning::Property(value) | Meaning::Block(value) | Meaning::Multi(value)) => {
            value == name
        }
        Some(Meaning::ExtendedObject) => name == "ExtendedConfigurationObject",
        _ => false,
    })
}

#[cfg(test)]
mod inverse_mapping_tests {

    #[test]
    fn inverse_ids_cover_every_guid_known_to_the_exporter() {
        // Includes generic property constants and every element-specific match arm.
        let source = include_str!("properties.rs");
        let forward = source.split("#[cfg(test)]").next().unwrap();
        let inverse = source.split("pub fn guid_for").nth(1).unwrap();
        let ids = inverse.split("IDS.iter()").next().unwrap();
        let ids = ids.split('"').collect::<Vec<_>>();
        for literal in forward.split('"') {
            if uuid::Uuid::parse_str(literal).is_ok() {
                assert!(
                    ids.contains(&literal),
                    "export-only property GUID {literal}"
                );
            }
        }
    }
}
