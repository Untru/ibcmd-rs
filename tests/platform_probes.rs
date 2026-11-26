//! Small configurations built by 8.3.27.2214 beside that platform's own dump
//! of their one form or template: form properties whose spelling follows the
//! configuration's compatibility mode (per mode,
//! `_onecdec/make_form_compat_probe.py`, sources in
//! `_onecdec/fixture_src/probe_forms`) and spreadsheet members
//! (`_onecdec/make_template_probe.py`, sources in
//! `_onecdec/fixture_src/probe_templates`).

use crate::common;

use std::fs;

fn assert_every_mode_matches(probe: &str) {
    let root = common::fixture(probe);
    let mut modes = fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    modes.sort();
    assert!(!modes.is_empty(), "{probe}: no cases");
    for dir in modes {
        let mode = dir.file_name().unwrap().to_string_lossy().into_owned();
        let out = common::temp_dir(&format!("{probe}-{mode}"));
        let run = common::export(&dir.join("input.cf"), &out);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let expected = fs::read(dir.join("Form.xml")).unwrap();
        let actual = fs::read(out.join("CommonForms/Форма/Ext/Form.xml")).unwrap();
        let _ = fs::remove_dir_all(&out);
        assert!(
            expected == actual,
            "{probe} {mode}: Form.xml differs\n--- native\n{}\n--- ours\n{}",
            String::from_utf8_lossy(&expected),
            String::from_utf8_lossy(&actual)
        );
    }
}

/// `<Behavior>Usual</Behavior>` is written from compatibility 8.3.20 on.
#[test]
fn explicit_usual_group_behavior_follows_compatibility_8_3_20() {
    assert_every_mode_matches("group_behavior");
}

/// An extension's forms do not follow its own compatibility mode: under
/// `ConfigurationExtensionCompatibilityMode` 8.3.14 (over a base at 8.3.27)
/// the explicit `Usual` is still written (a real extension at 8.3.9 agrees).
#[test]
fn an_extension_form_writes_usual_group_behavior_under_an_old_extension_mode() {
    let dir = common::fixture("group_behavior_extension").join("Version8_3_14");
    let out = common::temp_dir("group-behavior-extension");
    let run = common::export(&dir.join("input.cfe"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let expected = fs::read(dir.join("Form.xml")).unwrap();
    let actual = fs::read(out.join("CommonForms/ТестРасширение_Форма/Ext/Form.xml")).unwrap();
    let _ = fs::remove_dir_all(&out);
    assert!(
        expected == actual,
        "Form.xml differs\n--- native\n{}\n--- ours\n{}",
        String::from_utf8_lossy(&expected),
        String::from_utf8_lossy(&actual)
    );
}

/// Under compatibility 8.3.18 a dynamic list that declares no main table owns
/// its `DefaultPicture` (written plain); from 8.3.19 on the path is marked
/// `~`. Only the lines naming it are compared.
#[test]
fn no_main_table_default_picture_is_marked_from_compatibility_8_3_19() {
    let root = common::fixture("dynamic_list_default_picture");
    for mode in ["Version8_3_18", "Version8_3_19"] {
        let dir = root.join(mode);
        let out = common::temp_dir(&format!("default-picture-{mode}"));
        let run = common::export(&dir.join("input.cf"), &out);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let lines = |text: String| {
            text.lines()
                .filter(|line| line.contains("DefaultPicture"))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        let expected = lines(fs::read_to_string(dir.join("Form.xml")).unwrap());
        let actual = lines(fs::read_to_string(out.join("CommonForms/Форма/Ext/Form.xml")).unwrap());
        let _ = fs::remove_dir_all(&out);
        assert!(!expected.is_empty(), "{mode}");
        assert_eq!(actual, expected, "{mode}");
    }
}

/// A form loaded without its tables' search/view-status additions stores them
/// without ids; the platform numbers them on writing from one past the
/// greatest id, in document order. Only the lines naming an element's id are
/// compared (the fixture's tables also carry an empty property bag whose
/// defaults the platform writes -- not covered here).
#[test]
fn unnumbered_table_additions_are_numbered_past_the_greatest_id() {
    let dir = common::fixture("unnumbered_additions").join("Version8_3_18");
    let out = common::temp_dir("unnumbered-additions");
    let run = common::export(&dir.join("input.cf"), &out);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let lines = |text: String| {
        text.lines()
            .filter(|line| line.contains(" id=\""))
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let expected = lines(fs::read_to_string(dir.join("Form.xml")).unwrap());
    let actual = lines(fs::read_to_string(out.join("CommonForms/Форма/Ext/Form.xml")).unwrap());
    let _ = fs::remove_dir_all(&out);
    assert!(
        expected
            .iter()
            .any(|line| line.contains("SearchStringAddition"))
    );
    assert_eq!(actual, expected);
}

/// A spreadsheet's vertical and horizontal page breaks
/// (`_onecdec/make_template_probe.py`, sources in
/// `_onecdec/fixture_src/probe_templates`).
#[test]
fn spreadsheet_page_breaks_are_written() {
    for case in ["both", "horizontal"] {
        let dir = common::fixture("page_breaks").join(case);
        let out = common::temp_dir(&format!("page-breaks-{case}"));
        let run = common::export(&dir.join("input.cf"), &out);
        assert!(
            run.status.success(),
            "{}",
            String::from_utf8_lossy(&run.stderr)
        );
        let expected = fs::read(dir.join("Template.xml")).unwrap();
        let actual = fs::read(out.join("CommonTemplates/Макет/Ext/Template.xml")).unwrap();
        let _ = fs::remove_dir_all(&out);
        assert!(
            expected == actual,
            "{case}\n--- native\n{}\n--- ours\n{}",
            String::from_utf8_lossy(&expected),
            String::from_utf8_lossy(&actual)
        );
    }
}

/// A command that carries nothing is written as an empty element.
#[test]
fn an_empty_form_command_is_an_empty_element() {
    assert_every_mode_matches("empty_command");
}
