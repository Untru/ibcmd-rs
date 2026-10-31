use clap::ValueEnum;

use super::*;

#[test]
fn every_known_build_and_release_declares_its_xml_layout_and_features() {
    let palette = "2dd2d9e1-40c8-430b-a433-a81ec6856ab0";
    for (name, xml, layout, exact, features) in [
        (
            "8.3.27",
            InfobaseConfigSourceVersion::V2_20,
            FormLayout::V8_3,
            false,
            &[][..],
        ),
        (
            "8.3.27.1989",
            InfobaseConfigSourceVersion::V2_20,
            FormLayout::V8_3,
            true,
            &[][..],
        ),
        (
            "8.3.27.2214",
            InfobaseConfigSourceVersion::V2_20,
            FormLayout::V8_3,
            true,
            &[][..],
        ),
        (
            "8.5.1",
            InfobaseConfigSourceVersion::V2_21,
            FormLayout::V8_5_1,
            false,
            &[("palette-colors", palette)][..],
        ),
        (
            "8.5.1.1150",
            InfobaseConfigSourceVersion::V2_21,
            FormLayout::V8_5_1,
            true,
            &[("palette-colors", palette)][..],
        ),
        (
            "8.5.1.1529",
            InfobaseConfigSourceVersion::V2_21,
            FormLayout::V8_5_1,
            true,
            &[("palette-colors", palette)][..],
        ),
    ] {
        let spec = parse(name).unwrap();
        assert_eq!(spec.display(), name);
        assert_eq!(spec.to_string(), name);
        assert_eq!(spec.xml_version(), xml, "{name}");
        assert_eq!(spec.form_layout(), layout, "{name}");
        assert_eq!(spec.is_exact_build(), exact, "{name}");
        let declared = spec
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
        palette
    );
    assert!(
        parse("8.3.27")
            .unwrap()
            .feature(FEATURE_PALETTE_COLORS)
            .is_none()
    );
    assert_eq!(feature_uuid(FEATURE_PALETTE_COLORS).unwrap(), palette);
    assert!(feature_uuid("no-such-feature").is_err());
}

/// The builds agree on the XML format, the layout and the features, so
/// `8.5.1` names them all; they differ on the importance of a default button,
/// and the release keeps the spelling of the oldest.
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
fn a_platform_maps_to_its_xml_format_and_back() {
    // The explicit mapping: 8.3.x -> 2.20, 8.5.x -> 2.21, for the known
    // releases only.
    assert_eq!(
        for_xml_version(InfobaseConfigSourceVersion::V2_20)
            .unwrap()
            .display(),
        "8.3.27"
    );
    assert_eq!(
        for_xml_version(InfobaseConfigSourceVersion::V2_21)
            .unwrap()
            .display(),
        "8.5.1"
    );
    // The platform-shaped aliases `--source-version` has always accepted
    // agree with the registry.
    for alias in ["8.3.27", "8.5.1"] {
        let selector = InfobaseConfigSourceVersion::from_str(alias, false).unwrap();
        assert_eq!(parse(alias).unwrap().xml_version(), selector, "{alias}");
    }
    for spec in known().unwrap() {
        assert_eq!(
            for_xml_version(spec.xml_version()).unwrap(),
            spec.release_spec().unwrap(),
            "{spec}"
        );
    }
}

#[test]
fn unknown_and_unsupported_versions_are_refused_by_name() {
    let message = |text: &str| format!("{:#}", parse(text).unwrap_err());

    let unsupported = message("8.3.24.1819");
    assert!(
        unsupported.contains("8.3.24.1819 is not supported"),
        "{unsupported}"
    );
    assert!(unsupported.contains("8.3.27.2214"), "{unsupported}");

    let build = message("8.3.27.9999");
    assert!(
        build.contains("unknown platform build 8.3.27.9999"),
        "{build}"
    );
    assert!(build.contains("releases 8.3.27, 8.5.1"), "{build}");
    assert!(
        build.contains("builds 8.3.27.1989, 8.3.27.2214, 8.5.1.1150, 8.5.1.1529"),
        "{build}"
    );

    let release = message("8.5.4");
    assert!(
        release.contains("unknown platform release 8.5.4"),
        "{release}"
    );

    for malformed in [
        "8.5",
        "2.21",
        "8.3.27.2214.1",
        "8.3.x",
        "",
        "v8.3.27",
        "8..27",
    ] {
        let text = message(malformed);
        assert!(
            text.contains("is not a platform version"),
            "{malformed}: {text}"
        );
    }
    assert_eq!(parse(" 8.5.1.1150 ").unwrap().display(), "8.5.1.1150");
    assert!(parse_flag("8.4.0").is_err());
    assert_eq!(parse_flag("8.3.27").unwrap().display(), "8.3.27");
}

#[test]
fn compatibility_of_a_platform_is_its_release() {
    let v8_3 = parse("8.3.27.2214").unwrap();
    assert_eq!(v8_3.release(), [8, 3, 27]);
    assert_eq!(v8_3.compatibility_packed(), 80327);
    assert_eq!(v8_3.compatibility_mode(), "Version8_3_27");
    let v8_5 = parse("8.5.1").unwrap();
    assert_eq!(v8_5.compatibility_packed(), 80501);
    assert_eq!(v8_5.compatibility_mode(), "Version8_5_1");
    assert_eq!(
        parse("8.5.1.1150").unwrap().release_spec().unwrap(),
        parse("8.5.1").unwrap()
    );
    assert_eq!(v8_5.release_spec().unwrap(), v8_5);
}

#[test]
fn a_configuration_older_than_8_5_keeps_the_8_3_layout() {
    assert_eq!(FormLayout::V8_5_1.stored(80501), FormLayout::V8_5_1);
    assert_eq!(FormLayout::V8_5_1.stored(80500), FormLayout::V8_5_1);
    assert_eq!(FormLayout::V8_5_1.stored(80327), FormLayout::V8_3);
    assert_eq!(FormLayout::V8_3.stored(80501), FormLayout::V8_3);
    assert!(FormLayout::V8_3 < FormLayout::V8_5_1);
}

#[test]
fn native_profiles_and_live_activation_come_from_the_same_declarations() {
    for native in MssqlNativePlatformProfile::value_variants() {
        let build = native.id().strip_prefix("platform-").unwrap();
        let spec = parse(build).unwrap();
        assert_eq!(spec.native_profile(), Some(*native), "{build}");
        assert_eq!(
            spec.live_activation(),
            native.require_main_write_supported().is_ok(),
            "{build}"
        );
    }
    assert!(parse("8.3.27.2214").unwrap().live_activation());
    assert!(!parse("8.3.27.1989").unwrap().live_activation());
    assert!(!parse("8.5.1.1150").unwrap().live_activation());
    // A release stands for several builds: never enough to activate.
    assert!(!parse("8.3.27").unwrap().live_activation());
    assert_eq!(parse("8.3.27").unwrap().native_profile(), None);
}

#[test]
fn specs_serialize_as_their_version() {
    assert_eq!(
        serde_json::to_string(&parse("8.5.1.1150").unwrap()).unwrap(),
        "\"8.5.1.1150\""
    );
    assert_eq!(
        serde_json::to_string(&FormLayout::V8_5_1).unwrap(),
        "\"8.5.1\""
    );
    assert_eq!(
        format!("{:?}", parse("8.3.27").unwrap()),
        "PlatformSpec(\"8.3.27\")"
    );
}

/// A metadata context's `platform()` is the portable `Platform`; the root's
/// spec of it, and back, is one conversion each way.
#[test]
fn a_spec_and_its_portable_platform_convert_both_ways() {
    for spec in known().unwrap() {
        let platform = spec.platform();
        assert_eq!(Platform::from(spec), platform, "{spec}");
        assert_eq!(PlatformSpec::from(platform), spec, "{spec}");
        assert_eq!(platform.display(), spec.display(), "{spec}");
        assert_eq!(
            platform.xml_format().as_str(),
            spec.xml_version().as_str(),
            "{spec}"
        );
    }
    for (dialect, name) in [("2.20", "8.3.27"), ("2.21", "8.5.1")] {
        let context = crate::metadata_model::export::ExportContext {
            names: Default::default(),
            version: dialect.to_owned(),
            compat: crate::metadata_model::objects::parts::Compat(8, 3, 27),
        };
        let spec = PlatformSpec::from(context.platform());
        assert_eq!(spec, parse(name).unwrap(), "{dialect}");
        assert_eq!(spec, of_xml_dialect(dialect), "{dialect}");
    }
}
