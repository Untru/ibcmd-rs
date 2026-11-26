//! `copyinfo` of an external object: the configuration objects it references
//! (id → class + name path) and the type ids it uses (type id → object +
//! generated-type index). The platform uses it to show `cfg:CatalogRef.X`
//! without the configuration; so does this adapter. Layout:
//! `{4,{N,{<id>,<id>,k,{<class>,"name"}…}…},{M,{<type>,<id>,<index>}…},…}`.

use anyhow::{Context, Result, bail};

use super::brace;

/// Metadata class id → generated types in InternalInfo order; the copyinfo
/// type index is a position in this list. Class ids: `src/compiler/root.rs`;
/// orders: every object of the kind in the native 8.3.27.2214 dump of a
/// BSP-based configuration (one order per kind, no exceptions). Kinds not
/// listed stay unresolved (`<v8:TypeId>`) until evidenced.
const CLASSES: &[(&str, &[(&str, &str)])] = &[
    (
        "cf4abea6-37b2-11d4-940f-008048da11f9",
        &[
            ("CatalogObject", "Object"),
            ("CatalogRef", "Ref"),
            ("CatalogSelection", "Selection"),
            ("CatalogList", "List"),
            ("CatalogManager", "Manager"),
        ],
    ),
    (
        "061d872a-5787-460e-95ac-ed74ea3a3e84",
        &[
            ("DocumentObject", "Object"),
            ("DocumentRef", "Ref"),
            ("DocumentSelection", "Selection"),
            ("DocumentList", "List"),
            ("DocumentManager", "Manager"),
        ],
    ),
    (
        "f6a80749-5ad7-400b-8519-39dc5dff2542",
        &[
            ("EnumRef", "Ref"),
            ("EnumManager", "Manager"),
            ("EnumList", "List"),
        ],
    ),
    (
        "13134201-f60b-11d5-a3c7-0050bae0a776",
        &[
            ("InformationRegisterRecord", "Record"),
            ("InformationRegisterManager", "Manager"),
            ("InformationRegisterSelection", "Selection"),
            ("InformationRegisterList", "List"),
            ("InformationRegisterRecordSet", "RecordSet"),
            ("InformationRegisterRecordKey", "RecordKey"),
            ("InformationRegisterRecordManager", "RecordManager"),
        ],
    ),
    (
        "b64d9a40-1642-11d6-a3c7-0050bae0a776",
        &[
            ("AccumulationRegisterRecord", "Record"),
            ("AccumulationRegisterManager", "Manager"),
            ("AccumulationRegisterSelection", "Selection"),
            ("AccumulationRegisterList", "List"),
            ("AccumulationRegisterRecordSet", "RecordSet"),
            ("AccumulationRegisterRecordKey", "RecordKey"),
        ],
    ),
    (
        "82a1b659-b220-4d94-a9bd-14d757b95a48",
        &[
            ("ChartOfCharacteristicTypesObject", "Object"),
            ("ChartOfCharacteristicTypesRef", "Ref"),
            ("ChartOfCharacteristicTypesSelection", "Selection"),
            ("ChartOfCharacteristicTypesList", "List"),
            ("Characteristic", "Characteristic"),
            ("ChartOfCharacteristicTypesManager", "Manager"),
        ],
    ),
    (
        "857c4a91-e5f4-4fac-86ec-787626f1c108",
        &[
            ("ExchangePlanObject", "Object"),
            ("ExchangePlanRef", "Ref"),
            ("ExchangePlanSelection", "Selection"),
            ("ExchangePlanList", "List"),
            ("ExchangePlanManager", "Manager"),
        ],
    ),
    (
        "3e63355c-1378-4953-be9b-1deb5fb6bec5",
        &[
            ("TaskObject", "Object"),
            ("TaskRef", "Ref"),
            ("TaskSelection", "Selection"),
            ("TaskList", "List"),
            ("TaskManager", "Manager"),
        ],
    ),
    (
        "fcd3404e-1523-48ce-9bc0-ecdb822684a1",
        &[
            ("BusinessProcessObject", "Object"),
            ("BusinessProcessRef", "Ref"),
            ("BusinessProcessSelection", "Selection"),
            ("BusinessProcessList", "List"),
            ("BusinessProcessManager", "Manager"),
            ("BusinessProcessRoutePointRef", "RoutePointRef"),
        ],
    ),
    (
        "bf845118-327b-4682-b5c6-285d2a0eb296",
        &[
            ("DataProcessorObject", "Object"),
            ("DataProcessorManager", "Manager"),
        ],
    ),
    (
        "631b75a0-29e2-11d6-a3c7-0050bae0a776",
        &[("ReportObject", "Object"), ("ReportManager", "Manager")],
    ),
    (
        "0195e80c-b157-11d4-9435-004095e12fc7",
        &[
            ("ConstantManager", "Manager"),
            ("ConstantValueManager", "ValueManager"),
            ("ConstantValueKey", "ValueKey"),
        ],
    ),
    (
        "4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d",
        &[
            ("DocumentJournalSelection", "Selection"),
            ("DocumentJournalList", "List"),
            ("DocumentJournalManager", "Manager"),
        ],
    ),
    (
        "c045099e-13b9-4fb6-9d50-fca00202971e",
        &[("DefinedType", "DefinedType")],
    ),
    (
        "3e7bfcc0-067d-11d6-a3c7-0050bae0a776",
        &[
            ("FilterCriterionManager", "Manager"),
            ("FilterCriterionList", "List"),
        ],
    ),
    (
        "46b4cd97-fd13-4eaa-aba2-3bddd7699218",
        &[("SettingsStorageManager", "Manager")],
    ),
    (
        "c3831ec8-d8d5-4f93-8a22-f9bfae07327f",
        &[("ExternalDataProcessorObject", "Object")],
    ),
    (
        "e41aff26-25cf-4bb6-b6c1-3f478a75f374",
        &[("ExternalReportObject", "Object")],
    ),
];

/// Top-level metadata class id → kind, as in the configuration root sections
/// (`src/compiler/root.rs`, `CONFIGURATION_SECTION_1..7`).
const KINDS: &[(&str, &str)] = &[
    ("09736b02-9cac-4e3f-b4f7-d3e9576ab948", "Role"),
    ("0c89c792-16c3-11d5-b96b-0050bae0a95d", "CommonTemplate"),
    ("0fe48980-252d-11d6-a3c7-0050bae0a776", "CommonModule"),
    ("0fffc09c-8f4c-47cc-b41c-8d5c5a221d79", "HTTPService"),
    ("11bdaf85-d5ad-4d91-bb24-aa0eee139052", "ScheduledJob"),
    ("15794563-ccec-41f6-a83c-ec5f7b9a5bc1", "CommonAttribute"),
    ("24c43748-c938-45d0-8d14-01424a72b11e", "SessionParameter"),
    (
        "30d554db-541e-4f62-8970-a1c6dcfeb2bc",
        "FunctionalOptionsParameter",
    ),
    ("37f2fa9a-b276-11d4-9435-004095e12fc7", "Subsystem"),
    ("39bddf6a-0c3c-452b-921c-d99cfa1c2f1b", "Interface"),
    ("3e5404af-6ef8-4c73-ad11-91bd2dfac4c8", "Style"),
    ("3e7bfcc0-067d-11d6-a3c7-0050bae0a776", "FilterCriterion"),
    ("46b4cd97-fd13-4eaa-aba2-3bddd7699218", "SettingsStorage"),
    ("4e828da6-0f44-4b5b-b1c0-a2b3cfe7bdcc", "EventSubscription"),
    ("58848766-36ea-4076-8800-e91eb49590d7", "StyleItem"),
    ("6e6dc072-b7ac-41e7-8f88-278d25b6da2a", "Bot"),
    ("7dcd43d9-aca5-4926-b549-1842e6a4e8cf", "CommonPicture"),
    ("857c4a91-e5f4-4fac-86ec-787626f1c108", "ExchangePlan"),
    ("8657032e-7740-4e1d-a3ba-5dd6e8afb78f", "WebService"),
    ("9cd510ce-abfc-11d4-9434-004095e12fc7", "Language"),
    ("af547940-3268-434f-a3e7-e47d6d2638c3", "FunctionalOption"),
    ("c045099e-13b9-4fb6-9d50-fca00202971e", "DefinedType"),
    ("cc9df798-7c94-4616-97d2-7aa0b7bc515e", "XDTOPackage"),
    ("d26096fb-7a5d-4df9-af63-47d04771fa9b", "WSReference"),
    ("0195e80c-b157-11d4-9435-004095e12fc7", "Constant"),
    ("061d872a-5787-460e-95ac-ed74ea3a3e84", "Document"),
    ("07ee8426-87f1-11d5-b99c-0050bae0a95d", "CommonForm"),
    (
        "13134201-f60b-11d5-a3c7-0050bae0a776",
        "InformationRegister",
    ),
    ("1c57eabe-7349-44b3-b1de-ebfeab67b47d", "CommandGroup"),
    ("2f1a5187-fb0e-4b05-9489-dc5dd6412348", "CommonCommand"),
    ("36a8e346-9aaa-4af9-bdbd-83be3c177977", "DocumentNumerator"),
    ("4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d", "DocumentJournal"),
    ("631b75a0-29e2-11d6-a3c7-0050bae0a776", "Report"),
    (
        "82a1b659-b220-4d94-a9bd-14d757b95a48",
        "ChartOfCharacteristicTypes",
    ),
    (
        "b64d9a40-1642-11d6-a3c7-0050bae0a776",
        "AccumulationRegister",
    ),
    ("bc587f20-35d9-11d6-a3c7-0050bae0a776", "Sequence"),
    ("bf845118-327b-4682-b5c6-285d2a0eb296", "DataProcessor"),
    ("cf4abea6-37b2-11d4-940f-008048da11f9", "Catalog"),
    ("f6a80749-5ad7-400b-8519-39dc5dff2542", "Enum"),
    ("238e7e88-3c5f-48b2-8a3b-81ebbecb20ed", "ChartOfAccounts"),
    ("2deed9b8-0056-4ffe-a473-c20a6c32a0bc", "AccountingRegister"),
    (
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767",
        "ChartOfCalculationTypes",
    ),
    (
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7",
        "CalculationRegister",
    ),
    ("3e63355c-1378-4953-be9b-1deb5fb6bec5", "Task"),
    ("fcd3404e-1523-48ce-9bc0-ecdb822684a1", "BusinessProcess"),
    ("5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5", "ExternalDataSource"),
    ("bf3420b0-f6f9-41a0-b83a-fe9d4ab0b65d", "IntegrationService"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyInfoObject {
    pub id: String,
    /// `(class id, name)` from the configuration root down to the object.
    pub path: Vec<(String, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyInfoType {
    pub type_id: String,
    pub object_id: String,
    pub index: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CopyInfo {
    pub objects: Vec<CopyInfoObject>,
    pub types: Vec<CopyInfoType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedType {
    pub type_id: String,
    /// Generated type name without the `cfg:` prefix, e.g. `CatalogRef.X`.
    pub name: String,
    pub category: &'static str,
}

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').replace("\"\"", "\"")
}

pub fn parse(text: &str) -> Result<CopyInfo> {
    let text = brace::strip_bom(text).trim();
    let (top, _) = brace::fields(text, 0).context("copyinfo is not a brace list")?;
    if top.first() != Some(&"4") || top.len() < 3 {
        bail!("unsupported copyinfo layout");
    }
    let mut info = CopyInfo::default();
    let (objects, _) = brace::fields(top[1], 0).context("copyinfo objects")?;
    for object in objects.iter().skip(1) {
        let (f, _) = brace::fields(object, 0).context("copyinfo object")?;
        let depth: usize = f
            .get(2)
            .context("copyinfo object depth")?
            .parse()
            .context("copyinfo object depth")?;
        let mut path = Vec::with_capacity(depth);
        for step in f.iter().skip(3).take(depth) {
            let (s, _) = brace::fields(step, 0).context("copyinfo path step")?;
            let name = s.get(1).context("copyinfo path name")?;
            path.push((s[0].to_ascii_lowercase(), unquote(name)));
        }
        info.objects.push(CopyInfoObject {
            id: f[0].to_ascii_lowercase(),
            path,
        });
    }
    let (types, _) = brace::fields(top[2], 0).context("copyinfo types")?;
    for ty in types.iter().skip(1) {
        let (f, _) = brace::fields(ty, 0).context("copyinfo type")?;
        info.types.push(CopyInfoType {
            type_id: f.first().context("copyinfo type id")?.to_ascii_lowercase(),
            object_id: f
                .get(1)
                .context("copyinfo type object")?
                .to_ascii_lowercase(),
            index: f
                .get(2)
                .context("copyinfo type index")?
                .parse()
                .context("copyinfo type index")?,
        });
    }
    Ok(info)
}

const DEFINED_TYPE_CLASS: &str = "c045099e-13b9-4fb6-9d50-fca00202971e";
/// The unnamed "all catalogs" owner: index 0 is written `cfg:CatalogRef`
/// (corpus: 2 of 2). Other indexes are not evidenced yet.
const ALL_CATALOGS_CLASS: &str = "9fcd25a0-4822-11d4-9414-008048da11f9";

/// Types whose owner is a top-level object of a known class.
pub fn resolve(info: &CopyInfo) -> Vec<ResolvedType> {
    let mut out = Vec::new();
    for ty in &info.types {
        let Some(object) = info.objects.iter().find(|o| o.id == ty.object_id) else {
            continue;
        };
        let [(class, name)] = object.path.as_slice() else {
            continue;
        };
        // A defined type carries index 27 rather than a position (14 of 14).
        if class == DEFINED_TYPE_CLASS {
            out.push(ResolvedType {
                type_id: ty.type_id.clone(),
                name: format!("DefinedType.{name}"),
                category: "DefinedType",
            });
            continue;
        }
        if class == ALL_CATALOGS_CLASS && name.is_empty() && ty.index == 0 {
            out.push(ResolvedType {
                type_id: ty.type_id.clone(),
                name: "CatalogRef".into(),
                category: "Ref",
            });
            continue;
        }
        let Some((_, generated)) = CLASSES.iter().find(|(c, _)| c == class) else {
            continue;
        };
        let Some((prefix, category)) = generated.get(ty.index) else {
            continue;
        };
        out.push(ResolvedType {
            type_id: ty.type_id.clone(),
            name: format!("{prefix}.{name}"),
            category,
        });
    }
    out
}

/// Owner class → class of its form collection
/// (`crates/ibcmd-dump-readers/src/metadata_owner_graph.rs`,
/// `src/compiler/families/business_object.rs`); a two-step copyinfo path
/// through it names a form of a configuration object.
const FORM_COLLECTIONS: &[(&str, &str)] = &[
    (
        "cf4abea6-37b2-11d4-940f-008048da11f9",
        "fdf816d2-1ead-11d5-b975-0050bae0a95d",
    ),
    (
        "061d872a-5787-460e-95ac-ed74ea3a3e84",
        "fb880e93-47d7-4127-9357-a20e69c17545",
    ),
    (
        "82a1b659-b220-4d94-a9bd-14d757b95a48",
        "eb2b78a8-40a6-4b7e-b1b3-6ca9966cbc94",
    ),
    (
        "fcd3404e-1523-48ce-9bc0-ecdb822684a1",
        "3f7a8120-b71a-4265-98bf-4d9bc09b7719",
    ),
    (
        "3e63355c-1378-4953-be9b-1deb5fb6bec5",
        "3f58cbfb-4172-4e54-be49-561a579bb38b",
    ),
    (
        "3e7bfcc0-067d-11d6-a3c7-0050bae0a776",
        "00867c40-06b1-11d6-a3c7-0050bae0a776",
    ),
    (
        "bf845118-327b-4682-b5c6-285d2a0eb296",
        "d5b0e5ed-256d-401c-9c36-f630cafd8a62",
    ),
    (
        "631b75a0-29e2-11d6-a3c7-0050bae0a776",
        "a3b368c0-29e2-11d6-a3c7-0050bae0a776",
    ),
];

/// Configuration objects by id → `Kind.Name` (and forms of them →
/// `Kind.Name.Form.FormName`), the spelling of the pipeline's object
/// reference index (pictures, style items, help links, form slots …).
pub fn object_references(info: &CopyInfo) -> std::collections::BTreeMap<String, String> {
    info.objects
        .iter()
        .filter_map(|object| match object.path.as_slice() {
            [(class, name)] => {
                let (_, kind) = KINDS.iter().find(|(c, _)| c == class)?;
                Some((object.id.clone(), format!("{kind}.{name}")))
            }
            [(class, name), (child_class, form)] => {
                let (_, kind) = KINDS.iter().find(|(c, _)| c == class)?;
                FORM_COLLECTIONS
                    .iter()
                    .find(|(owner, forms)| owner == class && forms == child_class)?;
                Some((object.id.clone(), format!("{kind}.{name}.Form.{form}")))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // `copyinfo` of the corpus processor КартаМаршрутаБизнесПроцесса, cut to
    // three objects (one with a two-step path) and two types.
    const SAMPLE: &str = "{4,{3,{579baaa4-6493-4d99-8744-6399757295c7,579baaa4-6493-4d99-8744-6399757295c7,1,{cf4abea6-37b2-11d4-940f-008048da11f9,\"Пользователи\"}},{b85b3a58-95e4-4698-b1ae-037901ecc1b1,b85b3a58-95e4-4698-b1ae-037901ecc1b1,1,{f6a80749-5ad7-400b-8519-39dc5dff2542,\"СостоянияБизнесПроцессов\"}},{76389fa9-b9e0-492e-9c93-24b5f3bf094c,76389fa9-b9e0-492e-9c93-24b5f3bf094c,2,{3e63355c-1378-4953-be9b-1deb5fb6bec5,\"Задача\"},{3f58cbfb-4172-4e54-be49-561a579bb38b,\"ФормаЗадачи\"}}},{2,{645ed368-b47a-4aaa-9775-2a2cc3ab2ba4,b85b3a58-95e4-4698-b1ae-037901ecc1b1,0},{c54edff0-c3a1-44d7-9707-1fe05700b055,579baaa4-6493-4d99-8744-6399757295c7,1}},{0},{0,0},{0}}";

    #[test]
    fn parses_objects_and_types() {
        let info = parse(SAMPLE).unwrap();
        assert_eq!(info.objects.len(), 3);
        assert_eq!(info.objects[2].path.len(), 2);
        assert_eq!(info.objects[0].path[0].1, "Пользователи");
        assert_eq!(info.types.len(), 2);
        assert_eq!(info.types[1].index, 1);
    }

    #[test]
    fn resolves_ref_types_by_generated_type_index() {
        let resolved = resolve(&parse(SAMPLE).unwrap());
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].name, "EnumRef.СостоянияБизнесПроцессов");
        assert_eq!(resolved[1].name, "CatalogRef.Пользователи");
        assert_eq!(resolved[1].category, "Ref");
    }

    #[test]
    fn empty_copyinfo_has_no_types() {
        assert!(resolve(&parse("{4,{0},{0},{0},{0,0},{0}}").unwrap()).is_empty());
    }

    #[test]
    fn unknown_class_is_left_unresolved() {
        let text = SAMPLE.replace(
            "cf4abea6-37b2-11d4-940f-008048da11f9",
            "00000000-1111-2222-3333-444444444444",
        );
        let resolved = resolve(&parse(&text).unwrap());
        assert_eq!(resolved.len(), 1);
    }

    #[test]
    fn resolves_defined_types_whatever_their_index() {
        // corpus: every DefinedType reference carries index 27 (14 of 14)
        let text = "{4,{1,{ea2c4c8e-23cc-4884-812e-1e959da48afc,ea2c4c8e-23cc-4884-812e-1e959da48afc,1,{c045099e-13b9-4fb6-9d50-fca00202971e,\"Респондент\"}}},{1,{f0d7bb9f-e04b-4261-bbbf-0fe067eb2095,ea2c4c8e-23cc-4884-812e-1e959da48afc,27}},{0},{0,0},{0}}";
        let resolved = resolve(&parse(text).unwrap());
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "DefinedType.Респондент");
        assert_eq!(resolved[0].category, "DefinedType");
    }

    #[test]
    fn resolves_the_all_catalogs_type_set() {
        let text = "{4,{1,{0f7a6c2d-0000-0000-0000-000000000001,0f7a6c2d-0000-0000-0000-000000000001,1,{9fcd25a0-4822-11d4-9414-008048da11f9,\"\"}}},{1,{11111111-0000-0000-0000-000000000009,0f7a6c2d-0000-0000-0000-000000000001,0}},{0},{0,0},{0}}";
        let resolved = resolve(&parse(text).unwrap());
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].name, "CatalogRef");
    }

    #[test]
    fn maps_top_level_objects_to_kind_and_name() {
        let refs = object_references(&parse(SAMPLE).unwrap());
        assert_eq!(
            refs["579baaa4-6493-4d99-8744-6399757295c7"],
            "Catalog.Пользователи"
        );
        assert_eq!(
            refs["b85b3a58-95e4-4698-b1ae-037901ecc1b1"],
            "Enum.СостоянияБизнесПроцессов"
        );
        // a two-step path through the task form collection names the form
        assert_eq!(
            refs["76389fa9-b9e0-492e-9c93-24b5f3bf094c"],
            "Task.Задача.Form.ФормаЗадачи"
        );
    }

    #[test]
    fn maps_forms_of_configuration_objects() {
        let text = "{4,{2,{f829e2a9-5bdb-4171-8b32-8d10d5968813,f829e2a9-5bdb-4171-8b32-8d10d5968813,2,{061d872a-5787-460e-95ac-ed74ea3a3e84,\"Анкета\"},{fb880e93-47d7-4127-9357-a20e69c17545,\"ФормаДокумента\"}},{aaaaaaaa-5bdb-4171-8b32-8d10d5968813,aaaaaaaa-5bdb-4171-8b32-8d10d5968813,2,{cf4abea6-37b2-11d4-940f-008048da11f9,\"Пользователи\"},{cf4abea7-37b2-11d4-940f-008048da11f9,\"Реквизит\"}}},{0},{0},{0,0},{0}}";
        let refs = object_references(&parse(text).unwrap());
        assert_eq!(
            refs["f829e2a9-5bdb-4171-8b32-8d10d5968813"],
            "Document.Анкета.Form.ФормаДокумента"
        );
        // an attribute of a catalog is not a form
        assert!(!refs.contains_key("aaaaaaaa-5bdb-4171-8b32-8d10d5968813"));
    }

    #[test]
    fn maps_common_pictures_and_style_items() {
        let text = "{4,{2,{b5e73fbe-499c-4666-a482-0ef399c97c1e,b5e73fbe-499c-4666-a482-0ef399c97c1e,1,{7dcd43d9-aca5-4926-b549-1842e6a4e8cf,\"Предупреждение32\"}},{11111111-2222-3333-4444-555555555555,11111111-2222-3333-4444-555555555555,1,{58848766-36ea-4076-8800-e91eb49590d7,\"ПоясняющийТекст\"}}},{0},{0},{0,0},{0}}";
        let refs = object_references(&parse(text).unwrap());
        assert_eq!(
            refs["b5e73fbe-499c-4666-a482-0ef399c97c1e"],
            "CommonPicture.Предупреждение32"
        );
        assert_eq!(
            refs["11111111-2222-3333-4444-555555555555"],
            "StyleItem.ПоясняющийТекст"
        );
    }
}
