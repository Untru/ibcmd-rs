//! Якорные проверки XDTO-таблиц свойств форм.
//!
//! Значения взяты из модели `mngbase_root.res` и сверены с нативными
//! последовательностями ID (писатель пишет `0xc, 0xb` — сначала `name`,
//! затем `id`; `0x1f` у `UsualGroup` — `Group` с умолчанием `Vertical`).

use ibcmd_schema::{xdto_logform_v8327 as v8327, xdto_logform_v851 as v851};

#[test]
fn ids_are_contiguous_and_ordered() {
    for module in [&v851::PLATFORM, &v8327::PLATFORM] {
        assert!(!module.is_empty());
    }
    for type_name in ["UsualGroup", "Table", "Button", "Field", "Form"] {
        let properties = v851::properties(type_name).expect("тип есть в модели 8.5.1");
        for (index, property) in properties.iter().enumerate() {
            assert_eq!(property.id as usize, index, "{type_name}: ID не совпадает с позицией");
        }
    }
}

#[test]
fn form_item_base_anchors() {
    let id = v851::property("UsualGroup", 0xb).expect("id");
    assert_eq!(id.name, "id");
    assert_eq!(id.owner, "FormItemBase");

    let name = v851::property("UsualGroup", 0xc).expect("name");
    assert_eq!(name.name, "name");
    assert_eq!(name.owner, "FormItemBase");
}

#[test]
fn usual_group_anchors() {
    let group = v851::property("UsualGroup", 0x1f).expect("Group");
    assert_eq!(group.name, "Group");
    assert_eq!(group.default, Some("Vertical"));
    assert_eq!(group.owner, "UsualGroup");

    let align = v851::property("UsualGroup", 0x20).expect("ChildrenAlign");
    assert_eq!(align.name, "ChildrenAlign");
    assert_eq!(align.default, Some("Auto"));

    // В 8.3.27 цепочка короче: Group стоит на 0x1d.
    let old = v8327::property("UsualGroup", 0x1d).expect("Group в 8.3.27");
    assert_eq!(old.name, "Group");
}

#[test]
fn table_writer_order_matches_name_then_id() {
    // Нативный писатель 8.5.1 пишет сначала 0xc, затем 0xb.
    let first = v851::property("Table", 0xc).expect("name");
    let second = v851::property("Table", 0xb).expect("id");
    assert_eq!(first.name, "name");
    assert_eq!(second.name, "id");
}

#[test]
fn platform_851_extends_8327() {
    let old = v8327::properties("UsualGroup").expect("UsualGroup 8.3.27");
    let new = v851::properties("UsualGroup").expect("UsualGroup 8.5.1");

    // Цепочка выросла на 10: два свойства добавились в корень
    // PredefinedChildItems (7 -> 9) и восемь — в сам UsualGroup (22 -> 30).
    assert_eq!(new.len() - old.len(), 10);
    let old_own = old.iter().filter(|prop| prop.owner == "UsualGroup").count();
    let new_own = new.iter().filter(|prop| prop.owner == "UsualGroup").count();
    assert_eq!(new_own - old_own, 8);

    let old_root = old
        .iter()
        .filter(|prop| prop.owner == "PredefinedChildItems")
        .count();
    let new_root = new
        .iter()
        .filter(|prop| prop.owner == "PredefinedChildItems")
        .count();
    assert_eq!(new_root - old_root, 2, "рост корня сдвигает ID после него");
}
