//! READ · unit tests moved out of `read.rs`: X-comparability of the EDT vs Designer DCS
//! readers (they must decode to IDENTICAL IR) plus loud-refusal (§1.0) coverage.

use super::*;

#[cfg(any())]
mod dcs_param_type_value_tests {
    //! ПОД-КЛАСС 3: значение-ТИП DCS-параметра (`core:TypeValue` EDT ⟺ `v8:Type` Designer).
    //! Витнесс ERP Catalog.КлючиРеестраДокументов.ФормаВыбора DCS-параметр `ТипЗначенияКлюча`.
    use super::read_designer_dcs_parameter;
    use super::read_edt_dcs_parameter;
    use morph1c_core::ir::form::DcsParamValue;

    const EDT_PARAM: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<parameters xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\">\
<name>ТипЗначенияКлюча</name>\
<values xsi:type=\"core:TypeValue\"><value>Undefined</value></values>\
<useRestriction>true</useRestriction>\
</parameters>";

    const DES_PARAM: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Parameter xmlns:dcssch=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dcssch:name>ТипЗначенияКлюча</dcssch:name>\
<dcssch:value xmlns:d6p1=\"http://v8.1c.ru/8.2/data/types\" xsi:type=\"v8:Type\">d6p1:Undefined</dcssch:value>\
<dcssch:useRestriction>true</dcssch:useRestriction>\
</Parameter>";

    #[test]
    fn both_dialects_read_the_same_bare_type_name() {
        let edt_root = crate::parse(EDT_PARAM.as_bytes()).expect("parse edt").root;
        let edt = read_edt_dcs_parameter(&edt_root).expect("read edt param");
        let des_root = crate::parse(DES_PARAM.as_bytes()).expect("parse des").root;
        let des = read_designer_dcs_parameter(&des_root).expect("read designer param");
        // X-канон: беспрефиксное имя типа, ИДЕНТИЧНОЕ по обоим диалектам.
        assert_eq!(edt.value, Some(DcsParamValue::TypeValue("Undefined".into())));
        assert_eq!(des.value, edt.value, "edt ≡ designer (X-сравнимо)");
    }

    #[test]
    fn edt_write_reproduces_core_type_value() {
        let root = crate::parse(EDT_PARAM.as_bytes()).expect("parse").root;
        let param = read_edt_dcs_parameter(&root).expect("read");
        let out = crate::form::write::edt_dcs_parameter(&param).expect("write edt");
        let values = out
            .children
            .iter()
            .find(|c| c.local == "values")
            .expect("<values>");
        assert!(values
            .attrs
            .iter()
            .any(|(n, v)| n == "xsi:type" && v == "core:TypeValue"));
        let inner = values.children.iter().find(|c| c.local == "value").unwrap();
        assert_eq!(inner.text.as_deref(), Some("Undefined"));
    }

    #[test]
    fn designer_write_reproduces_locus_prefixed_v8_type() {
        let root = crate::parse(DES_PARAM.as_bytes()).expect("parse").root;
        let param = read_designer_dcs_parameter(&root).expect("read");
        let out = crate::form::write::designer_dcs_parameter(&param).expect("write des");
        let value = out
            .children
            .iter()
            .find(|c| c.local == "value" && c.prefix == "dcssch")
            .expect("<dcssch:value>");
        // ИНЛАЙН types-ns под авто-префиксом ЛОКУСА (d6p1), ПЕРЕД xsi:type; текст — QName.
        assert_eq!(
            value.attrs,
            vec![
                (
                    "xmlns:d6p1".to_string(),
                    "http://v8.1c.ru/8.2/data/types".to_string()
                ),
                ("xsi:type".to_string(), "v8:Type".to_string()),
            ]
        );
        assert_eq!(value.text.as_deref(), Some("d6p1:Undefined"));
    }
}

#[cfg(any())]
mod dcs_schema_field_extras_tests {
    //! DCS-schema field/calc extras (форм-локус), оба диалекта → ИДЕНТИЧНЫЙ IR (X-сравнимо):
    //! SC1 availableValues, SC2 field appearance, SC3 calc presentationExpression+orderExpression.
    use super::{
        read_designer_dcs_calculated_field, read_designer_dcs_field, read_edt_dcs_calculated_field,
        read_edt_dcs_field,
    };
    use crate::form::write::{
        designer_dcs_calculated_field, designer_dcs_field, edt_dcs_calculated_field, edt_dcs_field,
    };
    use morph1c_core::ir::form::{DcsCorValue, DcsParamValue};
    use morph1c_core::ir::PropertyValue;

    fn edt(root: &str) -> crate::descriptor::Element {
        crate::parse(root.as_bytes()).expect("parse edt").root
    }
    fn des(root: &str) -> crate::descriptor::Element {
        crate::parse(root.as_bytes()).expect("parse des").root
    }

    // ── SC1: availableValues (witness СчетНаОплатуКлиенту `Состояние`) ──────────────────────
    const SC1_EDT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<fields xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:schema=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xsi:type=\"schema:DataCompositionSchemaDataSetField\">\
<dataPath>Состояние</dataPath><field>Состояние</field>\
<availableValues>\
<value xsi:type=\"core:StringValue\"><value>Выставлен</value></value>\
<presentation><localValue><content><key>ru</key><value>Выставлен</value></content></localValue></presentation>\
</availableValues>\
</fields>";
    const SC1_DES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Field xmlns:dcssch=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"dcssch:DataSetFieldField\">\
<dcssch:dataPath>Состояние</dcssch:dataPath><dcssch:field>Состояние</dcssch:field>\
<dcssch:availableValue>\
<dcssch:value xsi:type=\"xs:string\">Выставлен</dcssch:value>\
<dcssch:presentation xsi:type=\"v8:LocalStringType\"><v8:item><v8:lang>ru</v8:lang><v8:content>Выставлен</v8:content></v8:item></dcssch:presentation>\
</dcssch:availableValue>\
</Field>";

    #[test]
    fn sc1_available_values_x_equal_and_write() {
        let ef = read_edt_dcs_field(&edt(SC1_EDT)).expect("read edt field");
        let df = read_designer_dcs_field(&des(SC1_DES)).expect("read des field");
        assert_eq!(ef, df, "SC1: edt ≡ designer (X-сравнимо)");
        assert_eq!(ef.available_values.len(), 1);
        assert_eq!(
            ef.available_values[0].value,
            DcsParamValue::Str("Выставлен".into())
        );
        assert_eq!(
            ef.available_values[0].presentation,
            Some(PropertyValue::Localized(vec![(
                morph1c_core::ir::Lang::new("ru"),
                "Выставлен".into()
            )]))
        );
        assert!(edt_dcs_field(&ef)
            .unwrap()
            .children
            .iter()
            .any(|c| c.local == "availableValues"));
        assert!(designer_dcs_field(&df)
            .unwrap()
            .children
            .iter()
            .any(|c| c.local == "availableValue"));
    }

    #[test]
    fn sc1_available_value_without_value_refuses() {
        let bad = SC1_EDT.replace(
            "<value xsi:type=\"core:StringValue\"><value>Выставлен</value></value>",
            "",
        );
        let e = read_edt_dcs_field(&edt(&bad)).expect_err("no <value> must refuse");
        assert!(e.to_string().contains("§1.0"), "loud refusal: {e}");
    }

    // ── SC2: field appearance (witness СостоянияCDNПлощадокИСМП `Формат=ДЛФ=DT`) ─────────────
    const SC2_EDT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<fields xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:schema=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:settings=\"http://g5.1c.ru/v8/dt/data-composition-system/settings\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xsi:type=\"schema:DataCompositionSchemaDataSetField\">\
<dataPath>Дата</dataPath><field>Дата</field>\
<appearance><items xsi:type=\"settings:SettingsParameterValue\"><parameter><value>Формат</value></parameter><values xsi:type=\"core:StringValue\"><value>ДЛФ=DT</value></values></items></appearance>\
</fields>";
    const SC2_DES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<Field xmlns:dcssch=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:dcscor=\"http://v8.1c.ru/8.1/data-composition-system/core\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"dcssch:DataSetFieldField\">\
<dcssch:dataPath>Дата</dcssch:dataPath><dcssch:field>Дата</dcssch:field>\
<dcssch:appearance><dcscor:item xsi:type=\"dcsset:SettingsParameterValue\"><dcscor:parameter>Формат</dcscor:parameter><dcscor:value xsi:type=\"xs:string\">ДЛФ=DT</dcscor:value></dcscor:item></dcssch:appearance>\
</Field>";

    #[test]
    fn sc2_field_appearance_x_equal_and_write() {
        let ef = read_edt_dcs_field(&edt(SC2_EDT)).expect("read edt field");
        let df = read_designer_dcs_field(&des(SC2_DES)).expect("read des field");
        assert_eq!(ef, df, "SC2: edt ≡ designer (X-сравнимо)");
        assert_eq!(ef.appearance.len(), 1);
        assert_eq!(ef.appearance[0].parameter, "Формат");
        assert_eq!(
            ef.appearance[0].value,
            Some(DcsCorValue::Str("ДЛФ=DT".into()))
        );
        assert!(edt_dcs_field(&ef)
            .unwrap()
            .children
            .iter()
            .any(|c| c.local == "appearance"));
        assert!(designer_dcs_field(&df)
            .unwrap()
            .children
            .iter()
            .any(|c| c.local == "appearance"));
    }

    #[test]
    fn sc2_field_appearance_non_string_value_refuses() {
        let bad = SC2_EDT.replace("core:StringValue", "core:NumberValue");
        let e = read_edt_dcs_field(&edt(&bad)).expect_err("non-StringValue appearance must refuse");
        assert!(e.to_string().contains("§1.0"), "loud refusal: {e}");
    }

    // ── SC3: calc presentationExpression + orderExpression (witness ОперацииСПодключаемымОборудованием) ─
    const SC3_EDT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<calculatedFields xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dataPath>УниверсальнаяДата</dataPath><expression>Дата</expression>\
<presentationExpression>Формат(Дата)</presentationExpression>\
<orderExpression><expression>Дата</expression></orderExpression>\
<orderExpression><expression>НомерОперации</expression></orderExpression>\
</calculatedFields>";
    const SC3_DES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<CalculatedField xmlns:dcssch=\"http://g5.1c.ru/v8/dt/data-composition-system/schema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dcssch:dataPath>УниверсальнаяДата</dcssch:dataPath><dcssch:expression>Дата</dcssch:expression>\
<dcssch:presentationExpression>Формат(Дата)</dcssch:presentationExpression>\
<dcssch:orderExpression><expression xmlns=\"http://v8.1c.ru/8.1/data-composition-system/common\">Дата</expression><orderType xmlns=\"http://v8.1c.ru/8.1/data-composition-system/common\">Asc</orderType><autoOrder xmlns=\"http://v8.1c.ru/8.1/data-composition-system/common\">false</autoOrder></dcssch:orderExpression>\
<dcssch:orderExpression><expression xmlns=\"http://v8.1c.ru/8.1/data-composition-system/common\">НомерОперации</expression><orderType xmlns=\"http://v8.1c.ru/8.1/data-composition-system/common\">Asc</orderType><autoOrder xmlns=\"http://v8.1c.ru/8.1/data-composition-system/common\">false</autoOrder></dcssch:orderExpression>\
</CalculatedField>";

    #[test]
    fn sc3_presentation_and_order_expressions_x_equal_and_write() {
        let ec = read_edt_dcs_calculated_field(&edt(SC3_EDT)).expect("read edt calc");
        let dc = read_designer_dcs_calculated_field(&des(SC3_DES)).expect("read des calc");
        assert_eq!(ec, dc, "SC3: edt ≡ designer (X-сравнимо)");
        assert_eq!(ec.presentation_expression.as_deref(), Some("Формат(Дата)"));
        assert_eq!(ec.order_expressions.len(), 2);
        assert_eq!(ec.order_expressions[0].expression, "Дата");
        assert_eq!(ec.order_expressions[0].order_type, "Asc");
        assert!(!ec.order_expressions[0].auto_order);
        assert_eq!(ec.order_expressions[1].expression, "НомерОперации");
        // EDT-writer опускает дефолты Asc/false: orderExpression несёт лишь <expression>.
        let eo = edt_dcs_calculated_field(&ec).unwrap();
        let oe = eo
            .children
            .iter()
            .find(|c| c.local == "orderExpression")
            .expect("<orderExpression>");
        assert_eq!(oe.children.len(), 1, "EDT: только <expression> (Asc/false опущены)");
        assert!(eo.children.iter().any(|c| c.local == "presentationExpression"));
        // Designer-writer эмитит ВСЕ ТРИ в common-ns.
        let do_ = designer_dcs_calculated_field(&dc).unwrap();
        let doe = do_
            .children
            .iter()
            .find(|c| c.local == "orderExpression")
            .expect("<dcssch:orderExpression>");
        assert_eq!(doe.children.len(), 3, "Designer: expression+orderType+autoOrder");
        assert!(doe.children.iter().all(|c| c
            .attrs
            .iter()
            .any(|(n, v)| n == "xmlns"
                && v == "http://v8.1c.ru/8.1/data-composition-system/common")));
    }

    #[test]
    fn sc3_order_expression_unexpected_child_refuses() {
        let bad = SC3_EDT.replace(
            "<orderExpression><expression>Дата</expression></orderExpression>",
            "<orderExpression><expression>Дата</expression><mystery>1</mystery></orderExpression>",
        );
        let e = read_edt_dcs_calculated_field(&edt(&bad))
            .expect_err("unexpected orderExpression child must refuse");
        assert!(e.to_string().contains("§1.0"), "loud refusal: {e}");
    }
}
