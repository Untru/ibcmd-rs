//! Rows mixing member revisions: a real extension keeps members not saved
//! since an older platform beside the newer ones, or in an order the
//! platform does not print.

use super::{catalog_wrapper_code_matches, parse_information_register_standard_attribute_bag};

/// The 25-key bag a tabular section's `LineNumber` stores, at `revision`.
fn bag(revision: &str) -> String {
    let pairs = [
        (
            "1183c14f-f814-49c6-9233-a3c26b3f64cf",
            r##"{"#",9ad557b1-249e-48dc-824b-3e149ecf10a6,{3,0,0}}"##,
        ),
        (
            "2723eb98-b4c1-498a-a6f3-70444757902f",
            r##"{"#",98ea8e5a-b586-442b-b944-6e3447734aa7,0}"##,
        ),
        ("2bbba66b-fabf-4863-8ba3-54b3c64c896e", r#"{"B",0}"#),
        ("2c8143d5-4248-4c43-8bfb-307c0be2e415", r#"{"B",0}"#),
        (
            "33c74a4d-561f-4bc0-9eaa-8d21c893c0a9",
            r##"{"#",ad3615c5-aae6-4725-89be-91827523abd9,{ad3615c5-aae6-4725-89be-91827523abd9,0}}"##,
        ),
        ("3b10624f-1e3d-495d-8093-25225efc5313", r#"{"U"}"#),
        ("3eaf5a8b-06d6-47b0-ac7d-a9698247f499", r#"{"U"}"#),
        (
            "4690ff70-e3fa-4914-9127-6a9acc5fc949",
            r##"{"#",87024738-fc2a-4436-ada1-df79d395c424,{0}}"##,
        ),
        ("4de03908-56f4-4396-a61e-17253afca9ac", r#"{"B",0}"#),
        (
            "580c29e2-8af4-4258-882a-7cf8073e61c8",
            r##"{"#",87024738-fc2a-4436-ada1-df79d395c424,{0}}"##,
        ),
        (
            "6c4f7074-e7d4-48eb-b31b-132873666262",
            r##"{"#",157fa490-4ce9-11d4-9415-008048da11f9,{1,00000000-0000-0000-0000-000000000000}}"##,
        ),
        (
            "6e3a1131-37a3-4da5-8895-572d9d0c9db6",
            r##"{"#",ace3fd07-11b2-477e-ab7f-36f0ea37c8dd,{ace3fd07-11b2-477e-ab7f-36f0ea37c8dd,2}}"##,
        ),
        (
            "7ba608f2-e654-42a3-8885-334fe88ca910",
            r##"{"#",12ca4003-ac70-450e-b897-37faf86bd313,0}"##,
        ),
        (
            "88149a78-9448-4767-867b-0e650d165d2e",
            r##"{"#",87024738-fc2a-4436-ada1-df79d395c424,{0}}"##,
        ),
        ("90ae4b5d-e0fd-49ef-a008-d67c1e75038c", r#"{"B",0}"#),
        (
            "9288a8ed-b259-46d0-a8e3-70d87956ff2d",
            r##"{"#",d46ea122-3201-4e5e-bed4-e669c6e463c8,{d46ea122-3201-4e5e-bed4-e669c6e463c8,1}}"##,
        ),
        ("b02800e9-a8d1-42ab-9a12-f673e92be968", r#"{"B",0}"#),
        ("c65a541f-0b91-4f33-bc88-fbaaa57f9992", r#"{"U"}"#),
        (
            "cf4abea3-37b2-11d4-940f-008048da11f9",
            r##"{"#",87024738-fc2a-4436-ada1-df79d395c424,{0}}"##,
        ),
        ("cf4abea4-37b2-11d4-940f-008048da11f9", r#"{"S",""}"#),
        (
            "d4232326-022b-421e-b6d3-88e418f74327",
            r##"{"#",3b8e6bdd-d648-49d5-af2f-d46d84f87dd5,{3b8e6bdd-d648-49d5-af2f-d46d84f87dd5,1}}"##,
        ),
        (
            "e3da683b-c54a-457a-a243-b9b4f9bf76dd",
            r##"{"#",b76a58b9-2a56-4e46-bb31-8e04ad9f31ae,{5006,0}}"##,
        ),
        ("e6b3f5f3-bdf3-4ad0-bc60-7323b3feb208", r#"{"U"}"#),
        ("f49e4ced-4033-4e6c-8755-9fbaaccd6078", r#"{"S",""}"#),
        (
            "fcf503b8-1c06-454a-970c-06413e64aee5",
            r##"{"#",f2eaae14-91a7-47b9-9d69-097877f41580,{0,0}}"##,
        ),
    ];
    let body = pairs
        .iter()
        .map(|(key, value)| format!("{key},{value}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{revision},25,{body}}}")
}

#[test]
fn revision_14_with_type_reduction_reads() {
    let text = bag("14");
    let parsed = parse_information_register_standard_attribute_bag(&text).unwrap();
    assert!(parsed.has_type_reduction_mode);
}

#[test]
fn revision_13_with_the_type_reduction_key_reads_like_revision_14() {
    // A real extension stores 18 tabular sections so (397 at revision 14);
    // 8.3.27.2214 prints their `LineNumber` with `TypeReductionMode`, and the
    // whole catalog used to be refused.
    let text = bag("13");
    let parsed = parse_information_register_standard_attribute_bag(&text).unwrap();
    assert!(parsed.has_type_reduction_mode);
}

#[test]
fn a_revision_57_catalog_reads_a_wrapper_5_attribute() {
    assert!(catalog_wrapper_code_matches(6, 6));
    assert!(catalog_wrapper_code_matches(5, 6));
    assert!(catalog_wrapper_code_matches(5, 5));
    // Not the other way: a revision-56 catalog never holds a newer attribute.
    assert!(!catalog_wrapper_code_matches(6, 5));
    assert!(!catalog_wrapper_code_matches(8, 6));
}

#[test]
fn configuration_mode_rights_print_in_canonical_order_whatever_the_stored_one() {
    // A real extension's role stores AnalyticsSystemClient before
    // MobileClient and the window modes after Output; 8.3.27.2214 prints
    // ... MobileClient, the five window modes, AnalyticsSystemClient,
    // SaveUserData, Output.
    let pairs = [
        ("3c00c6ee-844e-4620-85e4-671e72f114d9", "ThinClient"),
        (
            "f7c6a0bb-bca6-4cd3-9146-832971cd7073",
            "AnalyticsSystemClient",
        ),
        ("1e50809b-73ed-4935-bb77-2616c4cabdf5", "MobileClient"),
        ("d8682bbb-7800-4aa0-8590-d3cb11fe2a29", "SaveUserData"),
        ("31c3d4f6-7d02-4654-a14e-06aacafcb4fa", "Output"),
        (
            "d066966a-ff6a-4a41-bd68-6191cab083bc",
            "MainWindowModeNormal",
        ),
        (
            "f6168734-8b8d-4a88-ab39-ef6b51758e83",
            "MainWindowModeWorkplace",
        ),
        (
            "b9b44b51-3ac9-47cd-8b5a-df51afdcceb0",
            "MainWindowModeEmbeddedWorkplace",
        ),
        (
            "818fc6c3-4691-44e3-a80c-e8d424730ead",
            "MainWindowModeFullscreenWorkplace",
        ),
        (
            "155a0b35-4343-4047-989b-d385373b063e",
            "MainWindowModeKiosk",
        ),
    ];
    let blob = format!(
        "{{0,{}}}",
        pairs
            .iter()
            .map(|(uuid, _)| format!("{uuid},1"))
            .collect::<Vec<_>>()
            .join(",")
    );
    let rights = super::role_rights::parse_configuration_root_object_rights(&blob, false).unwrap();
    let names = rights
        .iter()
        .map(|right| right.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "ThinClient",
            "MobileClient",
            "MainWindowModeNormal",
            "MainWindowModeWorkplace",
            "MainWindowModeEmbeddedWorkplace",
            "MainWindowModeFullscreenWorkplace",
            "MainWindowModeKiosk",
            "AnalyticsSystemClient",
            "SaveUserData",
            "Output",
        ]
    );
}

#[test]
fn an_unknown_revision_is_refused() {
    assert!(parse_information_register_standard_attribute_bag(&bag("12")).is_none());
}
