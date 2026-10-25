//! Missing names must not silently erase an entire Configuration inventory.
use super::*;

#[test]
fn unresolved_configuration_child_refuses_root_instead_of_omitting_inventory() {
    let (row, context) = super::configuration_interface_pair_tests::fixture("2.21", "Taxi", true);
    let text = crate::metadata_model::brace::serialize(&row);
    let configuration = context
        .index
        .objects
        .values()
        .find(|entry| entry.kind == "Configuration")
        .unwrap();
    let mut names = context
        .index
        .objects
        .iter()
        .map(|(name, entry)| (entry.uuid.clone(), name.clone()))
        .collect::<BTreeMap<_, _>>();
    let version = InfobaseConfigSourceVersion::V2_21;
    let complete =
        extract_configuration_source_xml(&text, &configuration.uuid, &names, version).unwrap();
    assert!(complete.contains("<CommonForm>LayoutWitness</CommonForm>"));
    let child = context
        .index
        .objects
        .values()
        .find(|entry| entry.kind == "CommonForm")
        .unwrap();
    names.remove(&child.uuid);
    assert!(
        extract_configuration_source_xml(&text, &configuration.uuid, &names, version).is_none()
    );
}

#[test]
fn interface_preflight_uses_production_root_classification() {
    use crate::metadata_model::brace::{Brace, serialize};
    use ibcmd_schema::interface::InterfaceLayout as Layout;
    const ID: &str = "10000000-0000-4000-8000-000000000559";
    fn declare(tree: &mut Brace) -> bool {
        let Some(items) = tree.as_list_mut() else {
            return false;
        };
        if items.len() == 2
            && items[0].as_atom() == Some(Layout::FAMILY_UUID)
            && items[1].as_atom() == Some("0")
        {
            items[1] = Brace::num(1);
            items.push(Brace::uuid(ID));
            return true;
        }
        items.iter_mut().any(declare)
    }
    let (mut tree, context) =
        super::configuration_interface_pair_tests::fixture("2.21", "Taxi", true);
    assert!(declare(&mut tree));
    let id = &context.index.configuration_uuid.unwrap();
    let root = metadata_text_row_from_text(id, serialize(&tree)).unwrap();
    let missing = validate_interface_descriptors(&[root.clone()]).unwrap_err();
    assert!(missing.to_string().contains("missing Interface descriptor"));
    let text = format!(
        r#"{{1,{{8,0,{{3,{{1,0,{ID}}},"OwnInterface",{{0}},"",0,0,00000000-0000-0000-0000-000000000000,0}},1}},0}}"#
    );
    let interface = metadata_text_row_from_text(ID, text).unwrap();
    let invalid = validate_interface_descriptors(&[root, interface]).unwrap_err();
    assert!(
        invalid
            .to_string()
            .contains("unsupported Interface descriptor")
    );
}
