//! Сверка контракта порядка атрибутов с XDTO-моделью.
//!
//! Контракт (`form_attribute_order_v851`) задаёт порядок записи, XDTO — имена
//! и умолчания. Тест требует, чтобы каждая запись контракта совпадала с
//! моделью по имени и умолчанию: если платформа изменит свойство, тест
//! упадёт, а не тихо разойдётся с ней.

use ibcmd_schema::{form_attribute_order_v851 as order, xdto_logform_v851 as xdto};

const ELEMENTS: [&str; 10] = [
    "UsualGroup",
    "Table",
    "Button",
    "Field",
    "Form",
    "Pages",
    "Page",
    "CommandBar",
    "ContextMenu",
    "ColumnGroup",
];

#[test]
fn every_attribute_matches_xdto() {
    for element in ELEMENTS {
        let attributes = order::for_element(element).expect("порядок есть");
        let properties = xdto::properties(element).expect("тип есть в XDTO");
        for attribute in attributes {
            let property = properties
                .iter()
                .find(|property| property.id == attribute.id)
                .unwrap_or_else(|| {
                    panic!("{element}: ID {} отсутствует в XDTO", attribute.id)
                });
            assert_eq!(
                property.name, attribute.name,
                "{element}: ID {} — имя в XDTO не совпало",
                attribute.id
            );
            assert_eq!(
                property.default, attribute.default,
                "{element}: ID {} — умолчание в XDTO не совпало",
                attribute.id
            );
        }
    }
}

#[test]
fn order_is_writer_order_not_xdto_order() {
    // Нативный писатель пишет name раньше id (0xc, затем 0xb), хотя в XDTO
    // id стоит на 0xb, а name на 0xc.
    let table = order::for_element("Table").expect("Table");
    assert_eq!(table[0].id, 0xc);
    assert_eq!(table[0].name, "name");
    assert_eq!(table[1].id, 0xb);
    assert_eq!(table[1].name, "id");
}

#[test]
fn usual_group_starts_with_group() {
    let group = order::for_element("UsualGroup").expect("UsualGroup");
    assert_eq!(group[0].id, 0x1f);
    assert_eq!(group[0].name, "Group");
    assert_eq!(group[0].default, Some("Vertical"));
}

#[test]
fn order_covers_expected_sizes() {
    let sizes: Vec<(usize, &str)> = ELEMENTS
        .iter()
        .map(|element| {
            (
                order::for_element(element).map(<[_]>::len).unwrap_or(0),
                *element,
            )
        })
        .collect();
    for (size, element) in sizes {
        assert!(size > 0, "{element}: пустой контракт");
    }
}
