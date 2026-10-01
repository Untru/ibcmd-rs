use formats_xml::{parse, source_extensions as codec};
#[test]
fn form_command_data_path_parameter_is_typed_and_preserved() {
    use formats_xml::form::{FormDialect, read_form, write_form};
    let mut body = morph1c_core::ir::FormBody::new();
    body.command_interface = true;
    body.form_ci_navigation_panel
        .push(morph1c_core::ir::FormCiItem {
            command: "CommonCommand.Notes".into(),
            ty: "Added".into(),
            command_parameter: Some("Объект.Ref".into()),
            group: Some("FormNavigationPanelGoTo".into()),
            index: Some(0),
            user_visible: Some(false),
            user_visible_roles: vec![("Role.Editor".into(), false), ("Role.Admin".into(), true)],
        });
    let edt = write_form(FormDialect::Edt, &body).unwrap();
    let read = read_form(FormDialect::Edt, &edt).unwrap();
    assert_eq!(read.form_ci_navigation_panel, body.form_ci_navigation_panel);
    let xml = write_form(FormDialect::Designer, &read).unwrap();
    assert!(
        std::str::from_utf8(&xml)
            .unwrap()
            .contains("<Attribute>Объект.Ref</Attribute>")
    );
    assert_eq!(
        read_form(FormDialect::Designer, &xml)
            .unwrap()
            .form_ci_navigation_panel,
        body.form_ci_navigation_panel
    );
    let text = std::str::from_utf8(&edt).unwrap();
    for bad in [
        text.replace("form:DataPath", "core:StringValue"),
        text.replace(
            "</commandParameter>",
            "<segments>Other.Ref</segments></commandParameter>",
        ),
        text.replace(
            "</commandParameter>",
            "<unknown>keep</unknown></commandParameter>",
        ),
        text.replace("<segments>", "<segments extra=\"keep\">"),
        text.replace("http://g5.1c.ru/v8/dt/form", "urn:wrong:form"),
    ] {
        assert!(read_form(FormDialect::Edt, bad.as_bytes()).is_err());
    }
}

/// This witness deliberately has no native-process invocation. The supplied
/// native export and authentic installed-EDT project remain read-only.
#[test]
#[ignore = "requires independently prepared IBCMD_EDT_BSP_EDT / IBCMD_EDT_BSP_XML corpora"]
fn authentic_bsp_common_source_features_agree() {
    let edt = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_BSP_EDT").unwrap());
    let xml = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_BSP_XML").unwrap());
    let lab = std::path::PathBuf::from(std::env::var_os("IBCMD_EDT_LAB").unwrap());
    let read_bytes = |path: &std::path::Path| {
        let snapshot = tempfile::tempdir_in(&lab).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        std::fs::copy(path, snapshot.path().join(name)).unwrap();
        let tree = ibcmd_edt::read_xml_source(snapshot.path(), ibcmd_edt::ReaderLimits::default())
            .unwrap();
        tree.entries()[0].bytes().to_vec()
    };
    let read = |path: &std::path::Path| parse(&read_bytes(path)).unwrap();
    let e = read(&edt.join(
        "src/AccumulationRegisters/_ДемоОборотыПоСчетамНаОплату/_ДемоОборотыПоСчетамНаОплату.mdo",
    ));
    let x =
        read(&xml.join("AccumulationRegisters/_ДемоОборотыПоСчетамНаОплату/Ext/Aggregates.xml"));
    codec::claim_sidecar_envelope(
        &x.root,
        "AccumulationRegisterAggregates",
        &x.root.attr("version").unwrap().value,
    )
    .unwrap();
    let extras = codec::read_edt("AccumulationRegister", &e.root).unwrap();
    assert_eq!(
        extras.aggregates.unwrap(),
        codec::read_aggregates(&x.root, false).unwrap()
    );

    let e =
        read(&edt.join(
            "src/ChartsOfCalculationTypes/_ДемоОсновныеНачисления/_ДемоОсновныеНачисления.mdo",
        ));
    let x = read(&xml.join("ChartsOfCalculationTypes/_ДемоОсновныеНачисления/Ext/Predefined.xml"));
    codec::claim_sidecar_envelope(
        &x.root,
        "PredefinedData",
        &x.root.attr("version").unwrap().value,
    )
    .unwrap();
    let extras = codec::read_edt("ChartOfCalculationTypes", &e.root).unwrap();
    assert_eq!(
        extras.calculation_predefined.unwrap(),
        codec::read_calculation_predefined(&x.root, false).unwrap()
    );

    let e = read(
        &edt.join("src/CalculationRegisters/_ДемоОсновныеНачисления/_ДемоОсновныеНачисления.mdo"),
    );
    let x = read(&xml.join("CalculationRegisters/_ДемоОсновныеНачисления/Recalculations/ПерерасчетОсновныхНачислений.xml"));
    let extras = codec::read_edt("CalculationRegister", &e.root).unwrap();
    assert_eq!(
        extras.recalculations,
        vec![
            codec::read_recalculation(&x.root.children[0], false, Some("_ДемоОсновныеНачисления"))
                .unwrap()
        ]
    );
    for (kind, path) in [
        ("PaletteColor", "PaletteColors/ВниманиеБИПЦветФона"),
        ("Enum", "Enums/УдалитьСостоянияИнтеграцииОбъектов"),
    ] {
        let name = path.rsplit('/').next().unwrap();
        let e = read(&edt.join(format!("src/{path}/{name}.mdo")));
        let x = read(&xml.join(format!("{path}.xml")));
        let registry_e =
            morph1c_pipeline::registry::FormatRegistry::for_format(morph1c_pipeline::Format::Edt)
                .unwrap();
        let registry_x = morph1c_pipeline::registry::FormatRegistry::for_format(
            morph1c_pipeline::Format::Designer,
        )
        .unwrap();
        let er = (registry_e.get(kind).unwrap().read)(&read_bytes(
            &edt.join(format!("src/{path}/{name}.mdo")),
        ))
        .unwrap();
        let xr =
            (registry_x.get(kind).unwrap().read)(&read_bytes(&xml.join(format!("{path}.xml"))))
                .unwrap();
        assert_eq!(
            serde_json::to_value(er).unwrap(),
            serde_json::to_value(xr).unwrap(),
            "{kind} typed properties"
        );
        // Above raw descriptor inputs were independently bounded by the helper.
        assert_eq!(e.root.local, kind);
        assert_eq!(x.root.local, "MetaDataObject");
    }
    for path in [
        "Catalogs/_ДемоКонтрагенты/Forms/ФормаГруппы",
        "Documents/_ДемоПоступлениеТоваров/Forms/ФормаДокумента",
    ] {
        use formats_xml::form::{FormDialect, read_form};
        let e = read_form(
            FormDialect::Edt,
            &read_bytes(&edt.join(format!("src/{path}/Form.form"))),
        )
        .unwrap();
        let x = read_form(
            FormDialect::Designer,
            &read_bytes(&xml.join(format!("{path}/Ext/Form.xml"))),
        )
        .unwrap();
        assert_eq!(
            e.form_ci_navigation_panel, x.form_ci_navigation_panel,
            "{path} CMI navigation semantics"
        );
        assert_eq!(
            e.form_ci_command_bar, x.form_ci_command_bar,
            "{path} CMI command semantics"
        );
    }
}

#[test]
fn metadata_colors_reuse_rgb_refs_and_reject_unknowns() {
    use formats_xml::metadata_color::{self, Dialect};
    let rgb = r#"<color xsi:type="core:ColorDef"><red>255</red><green>236</green><blue>157</blue></color>"#;
    let value = metadata_color::decode(Dialect::Edt, &parse(rgb.as_bytes()).unwrap().root).unwrap();
    assert_eq!(
        metadata_color::decode(
            Dialect::Designer,
            &parse(b"<Color>#FFEC9D</Color>").unwrap().root
        )
        .unwrap(),
        value
    );
    for bad in [
        rgb.replace("</color>", "<alpha>1</alpha></color>"),
        rgb.replace("255", "256"),
        rgb.replace("</blue>", "</blue><blue>0</blue>"),
        rgb.replace("<red>", "<red changed=\"yes\">"),
        rgb.replace("core:ColorDef", "core:Unknown"),
    ] {
        assert!(
            metadata_color::decode(Dialect::Edt, &parse(bad.as_bytes()).unwrap().root).is_err()
        );
    }
    let source = r#"<test xmlns:core="http://g5.1c.ru/v8/dt/mcore" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><color xsi:type="core:ColorRef"><color>Palette.Blue</color></color></test>"#;
    metadata_color::validate_edt_bindings(&parse(source.as_bytes()).unwrap().root).unwrap();
    for bad in [
        source.replace(" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\"", ""),
        source.replace("http://g5.1c.ru/v8/dt/mcore", "urn:wrong"),
        source.replace(
            " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
            "",
        ),
    ] {
        assert!(
            metadata_color::validate_edt_bindings(&parse(bad.as_bytes()).unwrap().root).is_err()
        );
    }
    let value = metadata_color::decode(
        Dialect::Edt,
        &parse(r#"<color xsi:type="core:ColorRef"><color>Palette.Blue</color></color>"#.as_bytes())
            .unwrap()
            .root,
    )
    .unwrap();
    assert_eq!(
        metadata_color::decode(
            Dialect::Designer,
            &parse(b"<Color>pal:Blue</Color>").unwrap().root
        )
        .unwrap(),
        value
    );
}
fn roundtrip(source: &str, kind: &str) -> morph1c_core::ir::source_extensions::SourceExtensions {
    let doc = parse(source.as_bytes()).unwrap();
    let extras = codec::read_edt(kind, &doc.root).unwrap();
    assert_eq!(
        doc.root.unclaimed_count(),
        1,
        "only outer test root remains unclaimed"
    );
    let mut output = formats_xml::OutElement::branch("", "test");
    output
        .attrs
        .push(("xmlns:core".into(), "http://g5.1c.ru/v8/dt/mcore".into()));
    output.attrs.push((
        "xmlns:xsi".into(),
        "http://www.w3.org/2001/XMLSchema-instance".into(),
    ));
    codec::write_edt(&extras, &mut output).unwrap();
    let env = formats_xml::Envelope {
        bom: false,
        eol: "\r\n",
        indent_unit: "  ",
        decl: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        trailing_eol: true,
        escape_gt: true,
        escape_quot: true,
        text_eol: "\r\n",
    };
    let bytes = formats_xml::emit::render(&env, &output);
    let returned = parse(&bytes).unwrap();
    assert_eq!(codec::read_edt(kind, &returned.root).unwrap(), extras);
    extras
}
#[test]
fn aggregate_source_grammar_roundtrips_and_rejects_extra_cells() {
    let input = r#"<test xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:aggregates="http://g5.1c.ru/v8/dt/aggregates"><aggregates xsi:type="aggregates:AccumulationRegisterAggregates"><aggregates id="11111111-1111-1111-1111-111111111111"><use>Always</use><periodicity>Day</periodicity><dimensions>AccumulationRegister.Test.Dimension.A</dimensions></aggregates></aggregates></test>"#;
    let extras = roundtrip(input, "AccumulationRegister");
    let xml = codec::sidecar_envelope(
        codec::emit_aggregates(extras.aggregates.as_ref().unwrap(), false),
        "2.20",
    );
    let bytes = render(&xml);
    let doc = parse(&bytes).unwrap();
    codec::claim_sidecar_envelope(&doc.root, "AccumulationRegisterAggregates", "2.20").unwrap();
    assert_eq!(
        codec::read_aggregates(&doc.root, false).unwrap(),
        extras.aggregates.unwrap()
    );
    let bad = input.replace(
        "</aggregates></aggregates>",
        "<unknown>keep me</unknown></aggregates></aggregates>",
    );
    assert!(codec::read_edt("AccumulationRegister", &parse(bad.as_bytes()).unwrap().root).is_err());
    for bad in [
        input.replace(" xmlns:aggregates=\"http://g5.1c.ru/v8/dt/aggregates\"", ""),
        input.replace("http://g5.1c.ru/v8/dt/aggregates", "urn:unknown:aggregates"),
        input.replace(
            "http://www.w3.org/2001/XMLSchema-instance",
            "urn:unknown:xsi",
        ),
    ] {
        assert!(
            codec::read_edt("AccumulationRegister", &parse(bad.as_bytes()).unwrap().root).is_err()
        );
    }
}
fn render(root: &formats_xml::OutElement) -> Vec<u8> {
    formats_xml::emit::render(
        &formats_xml::Envelope {
            bom: true,
            eol: "\r\n",
            indent_unit: "\t",
            decl: "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
            trailing_eol: false,
            escape_gt: true,
            escape_quot: true,
            text_eol: "\n",
        },
        root,
    )
}
#[test]
fn calculation_predefined_preserves_dependency_lists() {
    let input = r#"<test xmlns:core="http://g5.1c.ru/v8/dt/mcore" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><predefined><items id="11111111-1111-1111-1111-111111111111"><name>A</name><description>Caption</description><code xsi:type="core:StringValue"><value>00001</value></code><actionPeriodIsBase>true</actionPeriodIsBase><displaced>ChartOfCalculationTypes.Test.B</displaced><base>ChartOfCalculationTypes.Test.C</base><leading>ChartOfCalculationTypes.Test.D</leading></items></predefined></test>"#;
    let extras = roundtrip(input, "ChartOfCalculationTypes");
    let mut owner = morph1c_core::ir::MetadataObject::new(
        morph1c_core::ir::ObjectKind::new("ChartOfCalculationTypes"),
        "Test",
        morph1c_core::ir::Uuid([0x99; 16]),
    );
    owner.source_extensions = extras.clone();
    codec::validate_predefined_code_kind(&owner).unwrap();
    owner.properties.push((
        morph1c_core::spec::metadata::chart_of_calculation_types::F_CODE_TYPE,
        morph1c_core::ir::PropertyValue::Enum(morph1c_core::ir::Token::new("Number")),
    ));
    assert!(codec::validate_predefined_code_kind(&owner).is_err());
    owner.source_extensions = codec::read_edt(
        "ChartOfCalculationTypes",
        &parse(
            input
                .replace("core:StringValue", "core:NumberValue")
                .as_bytes(),
        )
        .unwrap()
        .root,
    )
    .unwrap();
    codec::validate_predefined_code_kind(&owner).unwrap();
    let values = extras.calculation_predefined.unwrap();
    let doc = parse(&render(&codec::sidecar_envelope(
        codec::emit_calculation_predefined(&values, false),
        "2.20",
    )))
    .unwrap();
    codec::claim_sidecar_envelope(&doc.root, "PredefinedData", "2.20").unwrap();
    assert_eq!(
        codec::read_calculation_predefined(&doc.root, false).unwrap(),
        values
    );
    let bad = input.replace("</items>", "<unknown>keep</unknown></items>");
    for bad in [
        input.replace(" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\"", ""),
        input.replace("http://g5.1c.ru/v8/dt/mcore", "urn:unknown:core"),
        input.replace(
            " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
            "",
        ),
        input.replace(
            "http://www.w3.org/2001/XMLSchema-instance",
            "urn:unknown:xsi",
        ),
    ] {
        assert!(
            codec::read_edt(
                "ChartOfCalculationTypes",
                &parse(bad.as_bytes()).unwrap().root
            )
            .is_err()
        );
    }
    assert!(
        codec::read_edt(
            "ChartOfCalculationTypes",
            &parse(bad.as_bytes()).unwrap().root
        )
        .is_err()
    );
}
#[test]
fn recalculation_identities_dimensions_and_generated_types_survive() {
    let input = r#"<test><recalculations uuid="11111111-1111-1111-1111-111111111111"><producedTypes><recordType typeId="22222222-2222-2222-2222-222222222222" valueTypeId="33333333-3333-3333-3333-333333333333"/><managerType typeId="44444444-4444-4444-4444-444444444444" valueTypeId="55555555-5555-5555-5555-555555555555"/><recordSetType typeId="66666666-6666-6666-6666-666666666666" valueTypeId="77777777-7777-7777-7777-777777777777"/></producedTypes><name>R</name><synonym><key>ru</key><value>Caption</value></synonym><dataLockControlMode>Managed</dataLockControlMode><dimensions uuid="88888888-8888-8888-8888-888888888888"><name>D</name><registerDimension>CalculationRegister.Test.Dimension.D</registerDimension><leadingRegisterData>CalculationRegister.Test.Dimension.D</leadingRegisterData></dimensions></recalculations></test>"#;
    let extras = roundtrip(input, "CalculationRegister");
    let value = &extras.recalculations[0];
    let doc = parse(&render(&codec::emit_recalculation(
        value,
        false,
        Some("Test"),
    )))
    .unwrap();
    assert_eq!(
        codec::read_recalculation(&doc.root, false, Some("Test")).unwrap(),
        *value
    );
    let bad = input.replace("</dimensions>", "<unknown>keep</unknown></dimensions>");
    assert!(codec::read_edt("CalculationRegister", &parse(bad.as_bytes()).unwrap().root).is_err());
}
