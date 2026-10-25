//! Controlled properties of adopted objects of several kinds and of the
//! extension root, each set apart by an 8.3.27.2214 probe: `props_all`
//! controls every property the probe offers, `props_b0`..`props_b2` only
//! those whose index has that bit set
//! (`_onecdec/make_adopted_properties_fixture.py`).

use crate::common;

use std::{fs, path::Path};

fn assert_case(case: &str) {
    let dir = common::fixture("adopted").join(case);
    let out = common::temp_dir(&format!("adopted-{case}"));
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}{}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    assert!(!stdout.contains("are not known"), "{case}: {stdout}");
    let mut compared = 0;
    for (rel, expected) in common::files(&dir) {
        if rel == "input.cfe" {
            continue;
        }
        let actual = fs::read(Path::new(&out).join(&rel)).unwrap_or_default();
        assert!(
            expected == actual,
            "{case}: {rel} differs\n--- native\n{}\n--- ours\n{}",
            String::from_utf8_lossy(&expected),
            String::from_utf8_lossy(&actual)
        );
        compared += 1;
    }
    assert!(compared > 0, "{case}: nothing compared");
}

#[test]
fn every_probed_property_controlled() {
    assert_case("props_all");
}

#[test]
fn probed_properties_bit_zero() {
    assert_case("props_b0");
}

#[test]
fn probed_properties_bit_one() {
    assert_case("props_b1");
}

#[test]
fn probed_properties_bit_two() {
    assert_case("props_b2");
}

#[test]
fn a_direct_attribute_links_to_another_documents_tabular_section_attribute() {
    // Two data-path segments, foreign to the linking document, printed raw
    // in both ChoiceParameterLinks and LinkByType: the document used to be
    // refused (`_onecdec/make_foreign_links_fixture.py`).
    assert_case("foreign_links");
}

#[test]
fn adopted_chart_of_characteristic_types_task_and_business_process() {
    // An adopted object stores no standard attributes (`{0}`) and a process
    // no task; each was refused whole (`_onecdec/make_adopted_kinds_fixture.py`).
    assert_case("kinds");
}

#[test]
fn adopted_event_subscription_adding_a_source() {
    // No handler stored (nil module, empty method): the subscription's
    // reader refused it and the source was lost
    // (`_onecdec/make_adopted_subscription_fixture.py`).
    assert_case("subscription");
}

#[test]
fn adopted_exchange_plan_content_lists_the_extensions_objects() {
    // `<ExtensionProperty>` after the content items, from the list stored
    // after them (`_onecdec/make_adopted_exchange_plan_fixture.py`).
    assert_case("exchange_plan");
}

#[test]
fn adopted_catalog_predefined_items_carry_their_extension_state() {
    // `AdoptedCheck` for the base item, `Native` for the extension's own
    // (`_onecdec/make_adopted_predefined_fixture.py`).
    assert_case("predefined");
}

#[test]
fn adopted_role_with_its_rights_extended() {
    // setForAttributesByDefault and the unset rights are stored `2`; the
    // rights were refused whole (`_onecdec/make_adopted_role_fixture.py`).
    assert_case("role");
}

#[test]
fn widened_types_of_a_defined_type_an_attribute_and_a_filter_criterion() {
    // The extend value sits in the adoption header, the checked type in the
    // element's own field; a defined type adopted as is prints `<Type/>`
    // (`_onecdec/make_adopted_widened_fixture.py`).
    assert_case("widened");
}
