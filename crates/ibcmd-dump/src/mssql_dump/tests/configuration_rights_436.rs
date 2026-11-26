use super::*;

#[test]
fn role_rights_termination_follows_synthesized_or_scrambled_window_modes() {
    use ibcmd_schema::configuration_rights::{
        CONFIGURATION_MODE_RIGHT_NAMES, EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START,
    };
    for explicit_modes in [false, true] {
        let mut stored_names = vec![
            "CollaborationSystemInfoBaseRegistration",
            EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START,
        ];
        if explicit_modes {
            stored_names.extend(CONFIGURATION_MODE_RIGHT_NAMES.iter().rev().copied());
        }
        stored_names.push("SaveUserData");
        let pairs = stored_names
            .iter()
            .map(|name| format!(",{},1", role_right_uuid(name).unwrap()))
            .collect::<String>();
        let text = format!(
            "{{10,{{1,{{{{1,dddddddd-dddd-4ddd-dddd-dddddddddddd,0,0}},{{0{pairs}}}}}}},{{0}},0,1,0,4294967295}}"
        );
        let refs = BTreeMap::from([(
            "dddddddd-dddd-4ddd-dddd-dddddddddddd".into(),
            "Configuration.C".into(),
        )]);
        let rights =
            parse_role_rights_blob(&deflate_for_test(text.as_bytes()), &refs, &BTreeMap::new())
                .unwrap();
        let names = rights.objects[0]
            .rights
            .iter()
            .map(|right| right.name.as_str())
            .collect::<Vec<_>>();
        let mut expected = vec!["CollaborationSystemInfoBaseRegistration"];
        expected.extend(CONFIGURATION_MODE_RIGHT_NAMES);
        expected.extend([EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START, "SaveUserData"]);
        assert_eq!(names, expected, "explicit_modes={explicit_modes}");
    }
}
