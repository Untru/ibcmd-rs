//! The collections of the configuration's root row, in file order.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};

use crate::metadata_model::brace::Brace;

/// `{<class uuid>,<count>,<uuid>...}` of the root: the objects of one kind, in the root's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collection {
    pub class: String,
    pub objects: Vec<String>,
}

pub fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.char_indices().all(|(index, ch)| match index {
            8 | 13 | 18 | 23 => ch == '-',
            _ => ch.is_ascii_hexdigit(),
        })
}

/// Every `{<class>,<count>,<uuid>...}` of the root row in depth-first file order. A list counts
/// as a collection when its first element is a uuid, its second is the number of the uuids that
/// follow, and it has at least one.
pub fn collections(root: &Brace) -> Vec<Collection> {
    let mut out = Vec::new();
    collect(root, &mut out);
    out
}

fn collect(node: &Brace, out: &mut Vec<Collection>) {
    let Some(items) = node.as_list() else { return };
    if let [class, count, objects @ ..] = items
        && let (Some(class), Some(count)) = (class.as_atom(), count.as_atom())
        && is_uuid(class)
        && count.parse::<usize>().ok() == Some(objects.len())
        && !objects.is_empty()
        && objects
            .iter()
            .all(|object| object.as_atom().is_some_and(is_uuid))
    {
        out.push(Collection {
            class: class.to_ascii_lowercase(),
            objects: objects
                .iter()
                .filter_map(Brace::as_atom)
                .map(str::to_ascii_lowercase)
                .collect(),
        });
        return;
    }
    for item in items {
        collect(item, out);
    }
}

/// The objects of the root in the order the caches are filled in: the collections in file order,
/// each in its own order, as `(class, uuid)`.
pub fn flat_order(collections: &[Collection]) -> Vec<(&str, &str)> {
    collections
        .iter()
        .flat_map(|collection| {
            collection
                .objects
                .iter()
                .map(|object| (collection.class.as_str(), object.as_str()))
        })
        .collect()
}

/// Which collection (class) every object is listed under.
pub fn class_of(collections: &[Collection]) -> BTreeMap<&str, &str> {
    let mut map = BTreeMap::new();
    for collection in collections {
        for object in &collection.objects {
            map.entry(object.as_str())
                .or_insert(collection.class.as_str());
        }
    }
    map
}

/// The collection of a class.
pub fn collection_of<'a>(collections: &'a [Collection], class: &str) -> Result<&'a Collection> {
    let mut found = collections
        .iter()
        .filter(|collection| collection.class == class);
    let first = found
        .next()
        .with_context(|| format!("the root has no collection {class}"))?;
    if found.next().is_some() {
        bail!("the root lists collection {class} twice");
    }
    Ok(first)
}

/// The collection of the catalogs.
pub const CATALOG_CLASS: &str = "cf4abea6-37b2-11d4-940f-008048da11f9";
/// The collection of the documents.
pub const DOCUMENT_CLASS: &str = "061d872a-5787-460e-95ac-ed74ea3a3e84";

/// The root collection of a kind (the inverse of [`kind_of_class`] for the kinds the caches know).
pub fn class_of_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "Catalog" => Some(CATALOG_CLASS),
        "Document" => Some(DOCUMENT_CLASS),
        _ => None,
    }
}

/// The kind of the objects a root collection lists (the same table the export uses).
pub fn kind_of_class(class: &str) -> Option<&'static str> {
    Some(match class {
        "2deed9b8-0056-4ffe-a473-c20a6c32a0bc" => "AccountingRegister",
        "b64d9a40-1642-11d6-a3c7-0050bae0a776" => "AccumulationRegister",
        "6e6dc072-b7ac-41e7-8f88-278d25b6da2a" => "Bot",
        "fcd3404e-1523-48ce-9bc0-ecdb822684a1" => "BusinessProcess",
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7" => "CalculationRegister",
        "cf4abea6-37b2-11d4-940f-008048da11f9" => "Catalog",
        "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed" => "ChartOfAccounts",
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767" => "ChartOfCalculationTypes",
        "82a1b659-b220-4d94-a9bd-14d757b95a48" => "ChartOfCharacteristicTypes",
        "1c57eabe-7349-44b3-b1de-ebfeab67b47d" => "CommandGroup",
        "15794563-ccec-41f6-a83c-ec5f7b9a5bc1" => "CommonAttribute",
        "2f1a5187-fb0e-4b05-9489-dc5dd6412348" => "CommonCommand",
        "07ee8426-87f1-11d5-b99c-0050bae0a95d" => "CommonForm",
        "0fe48980-252d-11d6-a3c7-0050bae0a776" => "CommonModule",
        "7dcd43d9-aca5-4926-b549-1842e6a4e8cf" => "CommonPicture",
        "0c89c792-16c3-11d5-b96b-0050bae0a95d" => "CommonTemplate",
        "0195e80c-b157-11d4-9435-004095e12fc7" => "Constant",
        "bf845118-327b-4682-b5c6-285d2a0eb296" => "DataProcessor",
        "c045099e-13b9-4fb6-9d50-fca00202971e" => "DefinedType",
        "061d872a-5787-460e-95ac-ed74ea3a3e84" => "Document",
        "4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d" => "DocumentJournal",
        "36a8e346-9aaa-4af9-bdbd-83be3c177977" => "DocumentNumerator",
        "f6a80749-5ad7-400b-8519-39dc5dff2542" => "Enum",
        "4e828da6-0f44-4b5b-b1c0-a2b3cfe7bdcc" => "EventSubscription",
        "857c4a91-e5f4-4fac-86ec-787626f1c108" => "ExchangePlan",
        "5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5" => "ExternalDataSource",
        "3e7bfcc0-067d-11d6-a3c7-0050bae0a776" => "FilterCriterion",
        "af547940-3268-434f-a3e7-e47d6d2638c3" => "FunctionalOption",
        "30d554db-541e-4f62-8970-a1c6dcfeb2bc" => "FunctionalOptionsParameter",
        "0fffc09c-8f4c-47cc-b41c-8d5c5a221d79" => "HTTPService",
        "13134201-f60b-11d5-a3c7-0050bae0a776" => "InformationRegister",
        "bf3420b0-f6f9-41a0-b83a-fe9d4ab0b65d" => "IntegrationService",
        "39bddf6a-0c3c-452b-921c-d99cfa1c2f1b" => "Interface",
        "9cd510ce-abfc-11d4-9434-004095e12fc7" => "Language",
        "102f6202-43fa-40b0-8898-acd3876daacb" => "PaletteColor",
        "631b75a0-29e2-11d6-a3c7-0050bae0a776" => "Report",
        "09736b02-9cac-4e3f-b4f7-d3e9576ab948" => "Role",
        "11bdaf85-d5ad-4d91-bb24-aa0eee139052" => "ScheduledJob",
        "bc587f20-35d9-11d6-a3c7-0050bae0a776" => "Sequence",
        "24c43748-c938-45d0-8d14-01424a72b11e" => "SessionParameter",
        "46b4cd97-fd13-4eaa-aba2-3bddd7699218" => "SettingsStorage",
        "3e5404af-6ef8-4c73-ad11-91bd2dfac4c8" => "Style",
        "58848766-36ea-4076-8800-e91eb49590d7" => "StyleItem",
        "37f2fa9a-b276-11d4-9435-004095e12fc7" => "Subsystem",
        "3e63355c-1378-4953-be9b-1deb5fb6bec5" => "Task",
        "d26096fb-7a5d-4df9-af63-47d04771fa9b" => "WSReference",
        "8657032e-7740-4e1d-a3ba-5dd6e8afb78f" => "WebService",
        "cc9df798-7c94-4616-97d2-7aa0b7bc515e" => "XDTOPackage",
        _ => return None,
    })
}
