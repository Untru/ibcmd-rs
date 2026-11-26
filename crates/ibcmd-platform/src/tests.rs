use super::*;

const PALETTE: &str = "2dd2d9e1-40c8-430b-a433-a81ec6856ab0";

#[test]
fn every_known_build_and_release_declares_its_xml_layout_and_features() {
    let names = known()
        .unwrap()
        .iter()
        .map(|platform| platform.display())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "8.3.27.1989",
            "8.3.27.2214",
            "8.5.1.1150",
            "8.5.1.1529",
            "8.3.27",
            "8.5.1"
        ]
    );
    for (name, xml, layout, exact, features) in [
        ("8.3.27", XmlFormat::V2_20, FormLayout::V8_3, false, &[][..]),
        (
            "8.3.27.1989",
            XmlFormat::V2_20,
            FormLayout::V8_3,
            true,
            &[][..],
        ),
        (
            "8.3.27.2214",
            XmlFormat::V2_20,
            FormLayout::V8_3,
            true,
            &[][..],
        ),
        (
            "8.5.1",
            XmlFormat::V2_21,
            FormLayout::V8_5_1,
            false,
            &[("palette-colors", PALETTE)][..],
        ),
        (
            "8.5.1.1150",
            XmlFormat::V2_21,
            FormLayout::V8_5_1,
            true,
            &[("palette-colors", PALETTE)][..],
        ),
        (
            "8.5.1.1529",
            XmlFormat::V2_21,
            FormLayout::V8_5_1,
            true,
            &[("palette-colors", PALETTE)][..],
        ),
    ] {
        let platform = parse(name).unwrap();
        assert_eq!(platform.display(), name);
        assert_eq!(platform.to_string(), name);
        assert_eq!(platform.xml_format(), xml, "{name}");
        assert_eq!(platform.form_layout(), layout, "{name}");
        assert_eq!(platform.is_exact_build(), exact, "{name}");
        assert_eq!(
            platform
                .profile()
                .map(|profile| profile.id.as_str().to_owned()),
            exact.then(|| format!("platform-{name}")),
            "{name}"
        );
        let declared = platform
            .features()
            .iter()
            .map(|feature| (feature.name(), feature.uuid()))
            .collect::<Vec<_>>();
        assert_eq!(declared, features, "{name}");
    }
    assert_eq!(
        parse("8.5.1")
            .unwrap()
            .feature(FEATURE_PALETTE_COLORS)
            .unwrap()
            .uuid(),
        PALETTE
    );
    assert!(
        parse("8.3.27")
            .unwrap()
            .feature(FEATURE_PALETTE_COLORS)
            .is_none()
    );
}

#[test]
fn a_feature_uuid_comes_from_the_builds_that_declare_it() {
    assert_eq!(feature_uuid(FEATURE_PALETTE_COLORS).unwrap(), PALETTE);
    assert_eq!(
        format!("{:#}", feature_uuid("no-such-feature").unwrap_err()),
        "no known platform declares the feature `no-such-feature`"
    );
}

/// The metadata contexts read their platform from the dialect alone: `2.20`
/// is 8.3.27, any other dialect 8.5.1.
#[test]
fn a_dialect_maps_to_the_release_that_writes_it() {
    for (dialect, name, release, packed, layout) in [
        ("2.20", "8.3.27", [8, 3, 27], 80327, FormLayout::V8_3),
        ("2.21", "8.5.1", [8, 5, 1], 80501, FormLayout::V8_5_1),
        ("", "8.5.1", [8, 5, 1], 80501, FormLayout::V8_5_1),
    ] {
        let platform = of_xml_dialect(dialect);
        assert_eq!(platform, parse(name).unwrap(), "{dialect}");
        assert_eq!(platform.to_string(), name, "{dialect}");
        assert_eq!(platform.release(), release, "{dialect}");
        assert_eq!(platform.compatibility_packed(), packed, "{dialect}");
        assert_eq!(platform.form_layout(), layout, "{dialect}");
        assert!(!platform.is_exact_build(), "{dialect}");
    }
    for platform in known().unwrap() {
        assert_eq!(
            for_xml_format(platform.xml_format()).unwrap(),
            platform.release_platform().unwrap(),
            "{platform}"
        );
    }
}

#[test]
fn only_the_1529_build_writes_the_importance_of_a_default_button() {
    for (name, writes) in [
        ("8.3.27", false),
        ("8.3.27.2214", false),
        ("8.5.1", false),
        ("8.5.1.1150", false),
        ("8.5.1.1529", true),
    ] {
        assert_eq!(
            parse(name).unwrap().writes_default_button_importance(),
            writes,
            "{name}"
        );
    }
}

#[test]
fn unknown_and_unsupported_versions_are_refused_by_name() {
    let message = |text: &str| format!("{:#}", parse(text).unwrap_err());
    let known = "releases 8.3.27, 8.5.1; builds 8.3.27.1989, 8.3.27.2214, 8.5.1.1150, 8.5.1.1529";
    assert_eq!(
        message("8.3.24.1819"),
        format!(
            "platform 8.3.24.1819 is not supported: ibcmd-rs reads and writes only the XML of the platforms it knows ({known})"
        )
    );
    assert_eq!(
        message("8.3.27.9999"),
        format!("unknown platform build 8.3.27.9999: ibcmd-rs knows {known}")
    );
    assert_eq!(
        message("8.5.4"),
        format!("unknown platform release 8.5.4: ibcmd-rs knows {known}")
    );
    assert_eq!(
        message("8.5"),
        format!(
            "`8.5` is not a platform version: name a release such as 8.3.27 or an exact build such as 8.3.27.2214 ({known})"
        )
    );
    assert_eq!(parse(" 8.5.1.1150 ").unwrap().display(), "8.5.1.1150");
}

#[test]
fn compatibility_of_a_platform_is_its_release() {
    let v8_3 = parse("8.3.27.2214").unwrap();
    assert_eq!(v8_3.release(), [8, 3, 27]);
    assert_eq!(v8_3.compatibility_packed(), 80327);
    assert_eq!(v8_3.compatibility_mode(), "Version8_3_27");
    assert_eq!(release_name(v8_3.release()), "8.3.27");
    let v8_5 = parse("8.5.1").unwrap();
    assert_eq!(v8_5.compatibility_packed(), 80501);
    assert_eq!(v8_5.compatibility_mode(), "Version8_5_1");
    assert_eq!(
        parse("8.5.1.1150").unwrap().release_platform().unwrap(),
        v8_5
    );
    assert_eq!(v8_5.release_platform().unwrap(), v8_5);
}

#[test]
fn platforms_serialize_as_their_version() {
    assert_eq!(
        serde_json::to_string(&parse("8.5.1.1150").unwrap()).unwrap(),
        "\"8.5.1.1150\""
    );
    assert_eq!(
        format!("{:?}", parse("8.3.27").unwrap()),
        "Platform(\"8.3.27\")"
    );
}
