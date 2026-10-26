//! The export direction for the reference objects: a stored row -> the
//! object's XML DOM, walking the kind's `Layout` backwards, and what the row
//! contributes to a name index.
//!
//! The row stores properties in slot order; the XML writes them in the
//! order of [`Spec::properties`] (measured over the four reference trees:
//! every file of a kind uses the same order). Child objects are written per
//! [`Spec::children`], each collection in stored order.

use std::collections::HashMap;

use anyhow::{Context, Result, anyhow, bail};

use super::parts::{AttributeWrapper, Coll, CommandWrapper, Compat, Layout, Slot, TsWrapper};
use super::{layout, line_number_marker};
use crate::metadata_model::attribute::export::attribute_has_nondefault_fill;
use crate::metadata_model::brace::Brace;
use crate::metadata_model::export::values::{
    Header, Owner, attribute_header, attribute_properties, bool_text, code_text, field_text,
    header, header_elements, localized_element, reference_name, standard_attribute_elements,
    standard_attributes_element, type_element, value_element,
};
use crate::metadata_model::export::{
    Build, ExportContext, GeneratedTypeName, ObjectNames, atom, el, item, leaf, list, number,
    short, string,
};
use crate::metadata_model::standard_pictures::standard_picture_name;
use crate::metadata_model::xml::Element;

/// How a kind writes its XML.
struct Spec {
    /// `<Properties>` children in order.
    properties: &'static [&'static str],
    /// `<ChildObjects>` element kinds in order.
    children: &'static [&'static str],
    /// What follows `ChoiceHistoryOnInput` in a root attribute.
    attribute_tail: &'static [&'static str],
    /// Root attributes write `FillFromFillingValue` and `FillValue`.
    attribute_fill: bool,
    /// Tabular-section attributes write them.
    section_attribute_fill: bool,
    /// What follows `ChoiceHistoryOnInput` in a tabular-section attribute.
    section_attribute_tail: &'static [&'static str],
    /// What follows `StandardAttributes` in a tabular section.
    section_tail: &'static [&'static str],
}

const REFERENCE_TAIL: &[&str] = &["Indexing", "FullTextSearch", "DataHistory"];

#[cfg(test)]
mod fill_export_tests {
    use super::*;
    use crate::brace_list;
    use crate::metadata_model::attribute::attribute_body;
    use crate::metadata_model::export::NameIndex;
    use crate::metadata_model::types::tests::{context, element};

    fn attribute_with_fill(fill: &str) -> Brace {
        let properties = element(&format!(
            r#"<Properties xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><Name>Amount</Name><Synonym/><Comment/><Type><v8:Type>xs:decimal</v8:Type><v8:NumberQualifiers><v8:Digits>10</v8:Digits><v8:FractionDigits>2</v8:FractionDigits><v8:AllowedSign>Any</v8:AllowedSign></v8:NumberQualifiers></Type>{fill}</Properties>"#
        ));
        attribute_body(
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            &properties,
            &context(),
        )
        .unwrap()
    }

    fn section_attribute_properties(kind: &str, body: Brace) -> Result<Element> {
        let section_properties = element("<Properties><Name>Lines</Name></Properties>");
        let section = brace_list![
            Brace::num(11),
            Brace::nil_uuid(),
            Brace::nil_uuid(),
            Brace::nil_uuid(),
            Brace::nil_uuid(),
            brace_list![
                Brace::num(0),
                crate::metadata_model::md_base(
                    "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
                    &section_properties
                ),
            ],
            Brace::num(0),
            brace_list![Brace::num(0)],
            brace_list![Brace::num(0)],
        ];
        let (wrapper, wrapped) = if kind == "Catalog" {
            (
                TsWrapper::UseAndLength(2),
                brace_list![Brace::num(2), section, Brace::num(0), Brace::num(5)],
            )
        } else {
            (
                TsWrapper::Length(2),
                brace_list![Brace::num(2), section, Brace::num(5)],
            )
        };
        let stored = brace_list![
            wrapped,
            Brace::num(1),
            brace_list![
                Brace::uuid("88888888-8888-4888-8888-888888888888"),
                Brace::num(1),
                brace_list![
                    brace_list![
                        Brace::num(8),
                        body,
                        Brace::num(0),
                        Brace::num(1),
                        Brace::num(0)
                    ],
                    Brace::num(0),
                ],
            ],
        ];
        let full_name = format!("{kind}.Probe");
        let context = ExportContext {
            names: NameIndex::default(),
            version: "2.20".to_string(),
            compat: Compat(8, 3, 27),
        };
        let section = tabular_section(
            kind,
            &spec(kind)?,
            &stored,
            wrapper,
            AttributeWrapper::TabularSection,
            Owner {
                kind,
                full_name: &full_name,
            },
            &context,
        )?;
        Ok(section
            .path(&["ChildObjects", "Attribute", "Properties"])
            .unwrap()
            .clone())
    }

    fn assert_decimal_qualifiers(properties: &Element) {
        let value_type = properties.child("Type").unwrap();
        assert_eq!(value_type.child_text("Type"), Some("xs:decimal"));
        let qualifiers = value_type.child("NumberQualifiers").unwrap();
        assert_eq!(qualifiers.child_text("Digits"), Some("10"));
        assert_eq!(qualifiers.child_text("FractionDigits"), Some("2"));
        assert_eq!(qualifiers.child_text("AllowedSign"), Some("Any"));
    }

    #[test]
    fn document_section_legacy_empty_string_false_fill_remains_omitted() {
        // Absence compiles to the exact legacy stored pair {"S",""},0.
        let properties = section_attribute_properties("Document", attribute_with_fill("")).unwrap();
        assert!(Element::child(&properties, "FillFromFillingValue").is_none());
        assert!(Element::child(&properties, "FillValue").is_none());
        assert_decimal_qualifiers(&properties);
    }

    #[test]
    fn document_section_explicit_nil_false_fill_is_preserved() {
        let body = attribute_with_fill(
            r#"<FillFromFillingValue>false</FillFromFillingValue><FillValue xsi:nil="true"/>"#,
        );
        let properties = section_attribute_properties("Document", body).unwrap();
        assert_eq!(properties.child_text("FillFromFillingValue"), Some("false"));
        let value = Element::child(&properties, "FillValue").unwrap();
        assert_eq!(value.attr("nil"), Some("true"));
        assert!(value.attr("type").is_none());
        assert_decimal_qualifiers(&properties);
    }

    #[test]
    fn document_section_explicit_number_true_fill_is_preserved() {
        let body = attribute_with_fill(
            r#"<FillFromFillingValue>true</FillFromFillingValue><FillValue xsi:type="xs:decimal">42</FillValue>"#,
        );
        let properties = section_attribute_properties("Document", body).unwrap();
        assert_eq!(properties.child_text("FillFromFillingValue"), Some("true"));
        let value = Element::child(&properties, "FillValue").unwrap();
        assert_eq!(value.attr("type"), Some("xs:decimal"));
        assert_eq!(value.text, "42");
        assert_decimal_qualifiers(&properties);
    }

    #[test]
    fn catalog_section_legacy_fill_omission_policy_is_unchanged() {
        let properties = section_attribute_properties("Catalog", attribute_with_fill("")).unwrap();
        assert!(Element::child(&properties, "FillFromFillingValue").is_none());
        assert!(Element::child(&properties, "FillValue").is_none());
        assert_decimal_qualifiers(&properties);
    }

    #[test]
    fn document_root_attribute_still_emits_legacy_fill_properties() {
        let body = attribute_with_fill("");
        let context = ExportContext {
            names: NameIndex::default(),
            version: "2.20".to_string(),
            compat: Compat(8, 3, 27),
        };
        let properties = attribute_properties(
            &body,
            spec("Document").unwrap().attribute_fill,
            Vec::new(),
            Owner {
                kind: "Document",
                full_name: "Document.Probe",
            },
            &context,
        )
        .unwrap();
        assert_eq!(properties.child_text("FillFromFillingValue"), Some("false"));
        let value = Element::child(&properties, "FillValue").unwrap();
        assert_eq!(value.attr("type"), Some("xs:string"));
        assert!(value.text.is_empty());
        assert_decimal_qualifiers(&properties);
    }

    #[test]
    fn document_section_unsupported_fill_flag_is_refused() {
        let mut invalid_flag = attribute_with_fill("");
        invalid_flag.as_list_mut().unwrap()[20] = Brace::num(2);
        assert!(section_attribute_properties("Document", invalid_flag).is_err());
    }

    #[test]
    fn document_section_unsupported_fill_value_is_refused() {
        let mut invalid_value = attribute_with_fill("");
        invalid_value.as_list_mut().unwrap()[19] = brace_list![Brace::str("Unknown")];
        assert!(section_attribute_properties("Document", invalid_value).is_err());
    }

    // A clean-room complete Document descriptor, not a private source fixture.
    // These tests use the same base-patch entry point as metadata staging.
    fn patch_document_xml(fill: &str) -> String {
        let generated = |category: &str, index: usize| {
            format!(
                r#"<xr:GeneratedType name="Document{category}.Probe" category="{category}"><xr:TypeId>11111111-1111-4111-8111-{index:012}</xr:TypeId><xr:ValueId>22222222-2222-4222-8222-{index:012}</xr:ValueId></xr:GeneratedType>"#
            )
        };
        let internal = ["Object", "Ref", "Selection", "List", "Manager"]
            .iter()
            .enumerate()
            .map(|(index, category)| generated(category, index + 1))
            .collect::<String>();
        format!(
            r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
<Document uuid="dddddddd-dddd-4ddd-8ddd-dddddddddddd"><InternalInfo>{internal}</InternalInfo>
<Properties><Name>Probe</Name><Synonym/><Comment>untouched owner</Comment>
<UseStandardCommands>true</UseStandardCommands><NumberType>String</NumberType><NumberLength>9</NumberLength><NumberAllowedLength>Variable</NumberAllowedLength><NumberPeriodicity>Nonperiodical</NumberPeriodicity><CheckUnique>true</CheckUnique><Autonumbering>true</Autonumbering>
<Posting>Deny</Posting><RealTimePosting>Deny</RealTimePosting><RegisterRecordsDeletion>AutoDelete</RegisterRecordsDeletion><RegisterRecordsWritingOnPost>WriteSelected</RegisterRecordsWritingOnPost><SequenceFilling>AutoFill</SequenceFilling><PostInPrivilegedMode>true</PostInPrivilegedMode><UnpostInPrivilegedMode>true</UnpostInPrivilegedMode><IncludeHelpInContents>false</IncludeHelpInContents>
<DataLockControlMode>Managed</DataLockControlMode><FullTextSearch>DontUse</FullTextSearch><CreateOnInput>Auto</CreateOnInput><SearchStringModeOnInputByString>Begin</SearchStringModeOnInputByString><FullTextSearchOnInputByString>DontUse</FullTextSearchOnInputByString><ChoiceDataGetModeOnInputByString>Directly</ChoiceDataGetModeOnInputByString><ChoiceHistoryOnInput>Auto</ChoiceHistoryOnInput><DataHistory>DontUse</DataHistory><UpdateDataHistoryImmediatelyAfterWrite>false</UpdateDataHistoryImmediatelyAfterWrite><ExecuteAfterWriteDataHistoryVersionProcessing>false</ExecuteAfterWriteDataHistoryVersionProcessing></Properties>
<ChildObjects><TabularSection uuid="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"><InternalInfo>
<xr:GeneratedType name="DocumentTabularSection.Probe.Lines" category="TabularSection"><xr:TypeId>11111111-1111-4111-8111-000000000006</xr:TypeId><xr:ValueId>22222222-2222-4222-8222-000000000006</xr:ValueId></xr:GeneratedType>
<xr:GeneratedType name="DocumentTabularSectionRow.Probe.Lines" category="TabularSectionRow"><xr:TypeId>11111111-1111-4111-8111-000000000007</xr:TypeId><xr:ValueId>22222222-2222-4222-8222-000000000007</xr:ValueId></xr:GeneratedType></InternalInfo>
<Properties><Name>Lines</Name><Synonym/><Comment>untouched section</Comment><ToolTip/><FillChecking>DontCheck</FillChecking><LineNumberLength>5</LineNumberLength></Properties>
<ChildObjects><Attribute uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"><Properties><Name>Amount</Name><Synonym/><Comment>untouched attribute</Comment><Type><v8:Type>xs:decimal</v8:Type><v8:NumberQualifiers><v8:Digits>10</v8:Digits><v8:FractionDigits>2</v8:FractionDigits><v8:AllowedSign>Any</v8:AllowedSign></v8:NumberQualifiers></Type>{fill}<Indexing>DontIndex</Indexing><FullTextSearch>Use</FullTextSearch><DataHistory>DontUse</DataHistory></Properties></Attribute></ChildObjects>
</TabularSection></ChildObjects></Document></MetaDataObject>"#
        )
    }

    fn patch_document_fixture() -> Result<(Brace, ExportContext)> {
        use crate::metadata_model::{DescriptorContext, compile_descriptor};
        use std::{path::Path, sync::Arc};

        let root = Path::new("/offline-document-fill");
        let path = root.join("Documents/Probe.xml");
        let xml = patch_document_xml("");
        let context = DescriptorContext::with_files(
            root,
            "2.20",
            &[(path.clone(), Arc::new(xml.as_bytes().to_vec()))],
        )?;
        let plain = compile_descriptor("Document", &path, xml.as_bytes(), &context)?;
        let base = crate::metadata_model::brace::parse_row(&plain)?;
        let export = ExportContext {
            names: NameIndex::from_config_index(&context.index),
            version: "2.20".to_string(),
            compat: Compat(8, 3, 27),
        };
        // Validate the fixture before exercising the selected stage patcher.
        crate::metadata_model::export::decode_object("Document", &base, &export)?;
        Ok((base, export))
    }

    fn patch_document(base: &Brace, xml: &str) -> Result<Brace> {
        use crate::metadata_model::brace::{parse_row, serialize_row};
        use crate::module_blob::{
            deflate_raw, inflate_raw, pack_simple_metadata_blob_from_xml_with_source,
        };

        let blob = deflate_raw(&serialize_row(base))?;
        let staged = pack_simple_metadata_blob_from_xml_with_source(&blob, xml.as_bytes(), None)?;
        parse_row(&inflate_raw(&staged.blob)?)
    }

    fn patch_attribute_body(tree: &mut Brace) -> Option<&mut Brace> {
        if attribute_header(tree)
            .is_ok_and(|head| head.uuid == "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
        {
            return Some(tree);
        }
        tree.as_list_mut()?
            .iter_mut()
            .find_map(patch_attribute_body)
    }

    fn assert_document_fill_patch(fill: &str, value: Brace, flag: bool) -> Result<()> {
        let (base, context) = patch_document_fixture()?;
        let mut expected = base.clone();
        let body = patch_attribute_body(&mut expected)
            .unwrap()
            .as_list_mut()
            .unwrap();
        assert_eq!(body[19], brace_list![Brace::str("S"), Brace::str("")]);
        assert_eq!(body[20], Brace::flag(false));
        body[19] = value;
        body[20] = Brace::flag(flag);

        let xml = patch_document_xml(fill);
        let actual = patch_document(&base, &xml)?;
        assert_eq!(
            actual, expected,
            "only the two explicit Fill slots may change"
        );
        let object = crate::metadata_model::export::decode_object("Document", &actual, &context)?;
        let properties = object
            .path(&[
                "ChildObjects",
                "TabularSection",
                "ChildObjects",
                "Attribute",
                "Properties",
            ])
            .unwrap();
        assert_decimal_qualifiers(properties);
        assert!(properties.child("FillValue").is_some());
        assert_eq!(
            properties.child_text("FillFromFillingValue"),
            Some(if flag { "true" } else { "false" })
        );
        assert_eq!(patch_document(&actual, &xml)?, expected, "idempotent patch");
        Ok(())
    }

    #[test]
    fn document_stage_patch_transfers_explicit_nil_false_fill() -> Result<()> {
        assert_document_fill_patch(
            r#"<FillFromFillingValue>false</FillFromFillingValue><FillValue xsi:nil="true"/>"#,
            brace_list![Brace::str("U")],
            false,
        )
    }

    #[test]
    fn document_stage_patch_transfers_explicit_number_true_fill() -> Result<()> {
        assert_document_fill_patch(
            r#"<FillFromFillingValue>true</FillFromFillingValue><FillValue xsi:type="xs:decimal">42</FillValue>"#,
            brace_list![Brace::str("N"), Brace::num(42)],
            true,
        )
    }

    #[test]
    fn document_stage_patch_keeps_legacy_omission_unchanged() -> Result<()> {
        let (base, _) = patch_document_fixture()?;
        assert_eq!(patch_document(&base, &patch_document_xml(""))?, base);
        Ok(())
    }

    #[test]
    fn document_stage_patch_refuses_unsupported_fill_source_values() -> Result<()> {
        let (base, _) = patch_document_fixture()?;
        for fill in [
            "<FillFromFillingValue>unknown</FillFromFillingValue>",
            r#"<FillValue xsi:type="Unknown">value</FillValue>"#,
        ] {
            assert!(
                patch_document(&base, &patch_document_xml(fill)).is_err(),
                "{fill}"
            );
        }
        Ok(())
    }

    fn stage_patch_accepts_fill_xml(base: &Brace, xml: &str) -> Result<bool> {
        use crate::metadata_model::brace::serialize_row;
        use crate::module_blob::{deflate_raw, pack_simple_metadata_blob_from_xml_with_source};

        let blob = deflate_raw(&serialize_row(base))?;
        // Assert refusal at the real pack boundary, before any test-only
        // reparse could hide an invalid or injected native atom.
        Ok(pack_simple_metadata_blob_from_xml_with_source(&blob, xml.as_bytes(), None).is_ok())
    }

    #[test]
    fn document_stage_patch_refuses_invalid_numeric_fill_tokens() -> Result<()> {
        let (base, _) = patch_document_fixture()?;
        let mut accepted = Vec::new();
        for number in [
            "NaN",
            "INF",
            "-INF",
            "not-a-number",
            "1,2",
            "1,{0}",
            "{0}",
            "42}",
            "4 2",
            "1e3",
        ] {
            let xml = patch_document_xml(&format!(
                r#"<FillValue xsi:type="xs:decimal">{number}</FillValue>"#
            ));
            if stage_patch_accepts_fill_xml(&base, &xml)? {
                accepted.push(number);
            }
        }
        assert!(
            accepted.is_empty(),
            "accepted invalid numeric atoms: {accepted:?}"
        );
        Ok(())
    }

    #[test]
    fn document_stage_patch_requires_resolved_known_type_value_namespaces() -> Result<()> {
        let (base, _) = patch_document_fixture()?;
        let mut accepted = Vec::new();
        for (prefix, kind, value, uri) in [
            ("xs", "decimal", "42", "http://www.w3.org/2001/XMLSchema"),
            (
                "xr",
                "DesignTimeRef",
                "33333333-3333-4333-8333-333333333333.44444444-4444-4444-8444-444444444444",
                "http://v8.1c.ru/8.3/xcf/readable",
            ),
            ("v8", "TypeDescription", "", "http://v8.1c.ru/8.1/data/core"),
            (
                "ent",
                "AccountType",
                "Active",
                "http://v8.1c.ru/8.1/data/enterprise",
            ),
        ] {
            let fill = format!(r#"<FillValue xsi:type="{prefix}:{kind}">{value}</FillValue>"#);
            let mut undeclared =
                patch_document_xml(&fill).replace(&format!(r#" xmlns:{prefix}="{uri}""#), "");
            // Keep unrelated element names well bound, so each refusal must
            // protect the xsi:type QName value at FillValue's own scope.
            if prefix == "xr" {
                undeclared = undeclared.replace(
                    "<InternalInfo>",
                    &format!(r#"<InternalInfo xmlns:xr="{uri}">"#),
                );
            }
            if prefix == "v8" {
                undeclared = undeclared.replace("<Type>", &format!(r#"<Type xmlns:v8="{uri}">"#));
            }
            let rebound = patch_document_xml(&fill.replace(
                "<FillValue ",
                &format!(r#"<FillValue xmlns:{prefix}="urn:foreign" "#),
            ));
            for (scope, xml) in [("undeclared", undeclared), ("rebound", rebound)] {
                if stage_patch_accepts_fill_xml(&base, &xml)? {
                    accepted.push(format!("{scope} {prefix}:{kind}"));
                }
            }
        }
        assert!(
            accepted.is_empty(),
            "accepted unresolved/foreign type values: {accepted:?}"
        );
        Ok(())
    }

    #[test]
    fn document_stage_patch_refuses_invalid_calendar_fill_values() -> Result<()> {
        let (base, _) = patch_document_fixture()?;
        let mut accepted = Vec::new();
        for date in [
            "2026-02-30T12:00:00",
            "0000-01-01T00:00:00",
            "2026-01-01T25:00:00",
            "2025-02-29T12:00:00",
            "2026-01-00T12:00:00",
            "2026-13-01T12:00:00",
            "2026-01-01T12:60:00",
            "2026-01-01T12:00:60",
        ] {
            let xml = patch_document_xml(&format!(
                r#"<FillValue xsi:type="xs:dateTime">{date}</FillValue>"#
            ));
            if stage_patch_accepts_fill_xml(&base, &xml)? {
                accepted.push(date);
            }
        }
        assert!(
            accepted.is_empty(),
            "accepted invalid calendar dates: {accepted:?}"
        );
        Ok(())
    }
    #[test]
    fn document_stage_patch_preserves_valid_scalar_fill_values() -> Result<()> {
        // Typed scalar preservation controls; these are not a claim about a
        // platform import's assignment rules for an attribute's declared type.
        for (fill, expected) in [
            (
                r#"<FillValue xsi:type="xs:boolean">true</FillValue>"#,
                brace_list![Brace::str("B"), Brace::flag(true)],
            ),
            (
                r#"<FillValue xsi:type="xs:boolean">false</FillValue>"#,
                brace_list![Brace::str("B"), Brace::flag(false)],
            ),
            (
                r#"<FillValue xsi:type="xs:string">a,&quot;{b}&lt;&gt;&amp;</FillValue>"#,
                brace_list![Brace::str("S"), Brace::str("a,\"{b}<>&")],
            ),
            (
                r#"<FillValue xsi:type="xs:decimal">-42.25</FillValue>"#,
                brace_list![Brace::str("N"), Brace::atom("-42.25")],
            ),
            (
                r#"<FillValue xsi:type="xs:dateTime">2024-02-29T23:59:59</FillValue>"#,
                brace_list![Brace::str("D"), Brace::atom("20240229235959")],
            ),
        ] {
            assert_document_fill_patch(fill, expected, false)?;
        }
        Ok(())
    }
}

fn spec(kind: &str) -> Result<Spec> {
    let reference = |properties, children| Spec {
        properties,
        children,
        attribute_tail: REFERENCE_TAIL,
        attribute_fill: true,
        section_attribute_fill: false,
        section_attribute_tail: REFERENCE_TAIL,
        section_tail: &["LineNumberLength"],
    };
    const STANDARD_CHILDREN: &[&str] =
        &["Attribute", "TabularSection", "Form", "Template", "Command"];
    Ok(match kind {
        "Catalog" => Spec {
            attribute_tail: &["Use", "Indexing", "FullTextSearch", "DataHistory"],
            section_tail: &["Use", "LineNumberLength"],
            ..reference(CATALOG, STANDARD_CHILDREN)
        },
        "Document" => reference(
            DOCUMENT,
            &["Attribute", "Form", "TabularSection", "Template", "Command"],
        ),
        "ExchangePlan" => reference(EXCHANGE_PLAN, STANDARD_CHILDREN),
        "ChartOfCharacteristicTypes" => Spec {
            attribute_tail: &["Indexing", "Use", "FullTextSearch", "DataHistory"],
            section_tail: &["Use", "LineNumberLength"],
            ..reference(CHART_OF_CHARACTERISTIC_TYPES, STANDARD_CHILDREN)
        },
        "ChartOfAccounts" => reference(
            CHART_OF_ACCOUNTS,
            &[
                "Attribute",
                "AccountingFlag",
                "ExtDimensionAccountingFlag",
                "Form",
                "Template",
                "Command",
            ],
        ),
        "ChartOfCalculationTypes" => reference(CHART_OF_CALCULATION_TYPES, STANDARD_CHILDREN),
        "BusinessProcess" => reference(BUSINESS_PROCESS, STANDARD_CHILDREN),
        "Task" => reference(
            TASK,
            &[
                "Attribute",
                "TabularSection",
                "Form",
                "AddressingAttribute",
                "Template",
                "Command",
            ],
        ),
        "Report" | "DataProcessor" => Spec {
            properties: if kind == "Report" {
                REPORT
            } else {
                DATA_PROCESSOR
            },
            children: STANDARD_CHILDREN,
            attribute_tail: &[],
            attribute_fill: false,
            section_attribute_fill: true,
            section_attribute_tail: &[],
            section_tail: &[],
        },
        "Enum" => Spec {
            children: &["EnumValue", "Form", "Template", "Command"],
            ..reference(ENUM, STANDARD_CHILDREN)
        },
        other => bail!("{other} is not a reference object"),
    })
}

const CATALOG: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "Hierarchical",
    "HierarchyType",
    "LimitLevelCount",
    "LevelCount",
    "FoldersOnTop",
    "UseStandardCommands",
    "Owners",
    "SubordinationUse",
    "CodeLength",
    "DescriptionLength",
    "CodeType",
    "CodeAllowedLength",
    "CodeSeries",
    "CheckUnique",
    "Autonumbering",
    "DefaultPresentation",
    "StandardAttributes",
    "Characteristics",
    "PredefinedDataUpdate",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "DefaultObjectForm",
    "DefaultFolderForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "DefaultFolderChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryFolderForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "AuxiliaryFolderChoiceForm",
    "IncludeHelpInContents",
    "BasedOn",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const DOCUMENT: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "Numerator",
    "NumberType",
    "NumberLength",
    "NumberAllowedLength",
    "NumberPeriodicity",
    "CheckUnique",
    "Autonumbering",
    "StandardAttributes",
    "Characteristics",
    "BasedOn",
    "InputByString",
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "Posting",
    "RealTimePosting",
    "RegisterRecordsDeletion",
    "RegisterRecordsWritingOnPost",
    "SequenceFilling",
    "RegisterRecords",
    "PostInPrivilegedMode",
    "UnpostInPrivilegedMode",
    "IncludeHelpInContents",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChoiceHistoryOnInput",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const EXCHANGE_PLAN: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "CodeLength",
    "CodeAllowedLength",
    "DescriptionLength",
    "DefaultPresentation",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "StandardAttributes",
    "Characteristics",
    "BasedOn",
    "DistributedInfoBase",
    "IncludeConfigurationExtensions",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "IncludeHelpInContents",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const CHART_OF_CHARACTERISTIC_TYPES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "CharacteristicExtValues",
    "Type",
    "Hierarchical",
    "FoldersOnTop",
    "CodeLength",
    "CodeAllowedLength",
    "DescriptionLength",
    "CodeSeries",
    "CheckUnique",
    "Autonumbering",
    "DefaultPresentation",
    "StandardAttributes",
    "Characteristics",
    "PredefinedDataUpdate",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceHistoryOnInput",
    "DefaultObjectForm",
    "DefaultFolderForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "DefaultFolderChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryFolderForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "AuxiliaryFolderChoiceForm",
    "BasedOn",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const CHART_OF_ACCOUNTS: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "BasedOn",
    "ExtDimensionTypes",
    "MaxExtDimensionCount",
    "CodeMask",
    "CodeLength",
    "DescriptionLength",
    "CodeSeries",
    "CheckUnique",
    "DefaultPresentation",
    "StandardAttributes",
    "Characteristics",
    "StandardTabularSections",
    "PredefinedDataUpdate",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "AutoOrderByCode",
    "OrderLength",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
];

const CHART_OF_CALCULATION_TYPES: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "CodeLength",
    "DescriptionLength",
    "CodeType",
    "CodeAllowedLength",
    "DefaultPresentation",
    "EditType",
    "QuickChoice",
    "ChoiceMode",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "BasedOn",
    "DependenceOnCalculationTypes",
    "BaseCalculationTypes",
    "ActionPeriodUse",
    "StandardAttributes",
    "Characteristics",
    "StandardTabularSections",
    "PredefinedDataUpdate",
    "IncludeHelpInContents",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const BUSINESS_PROCESS: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "EditType",
    "InputByString",
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "FullTextSearchOnInputByString",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "ChoiceHistoryOnInput",
    "NumberType",
    "NumberLength",
    "NumberAllowedLength",
    "CheckUnique",
    "StandardAttributes",
    "Characteristics",
    "Autonumbering",
    "BasedOn",
    "NumberPeriodicity",
    "Task",
    "CreateTaskInPrivilegedMode",
    "DataLockFields",
    "DataLockControlMode",
    "IncludeHelpInContents",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const TASK: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "NumberType",
    "NumberLength",
    "NumberAllowedLength",
    "CheckUnique",
    "Autonumbering",
    "TaskNumberAutoPrefix",
    "DescriptionLength",
    "Addressing",
    "MainAddressingAttribute",
    "CurrentPerformer",
    "BasedOn",
    "StandardAttributes",
    "Characteristics",
    "DefaultPresentation",
    "EditType",
    "InputByString",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "ChoiceHistoryOnInput",
    "IncludeHelpInContents",
    "DataLockFields",
    "DataLockControlMode",
    "FullTextSearch",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "DataHistory",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
];

const REPORT: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "DefaultForm",
    "AuxiliaryForm",
    "MainDataCompositionSchema",
    "DefaultSettingsForm",
    "AuxiliarySettingsForm",
    "DefaultVariantForm",
    "AuxiliaryVariantForm",
    "VariantsStorage",
    "SettingsStorage",
    "IncludeHelpInContents",
    "ExtendedPresentation",
    "Explanation",
];

const DATA_PROCESSOR: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "DefaultForm",
    "AuxiliaryForm",
    "IncludeHelpInContents",
    "ExtendedPresentation",
    "Explanation",
];

const ENUM: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "StandardAttributes",
    "Characteristics",
    "QuickChoice",
    "ChoiceMode",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChoiceHistoryOnInput",
];

/// Properties only the 2.21 dialect writes.
const XML_2_21_ONLY: &[&str] = &["AuxiliaryVariantForm"];

/// Properties the 2.21 dialect writes although an older row does not store
/// them.
fn xml_only_default(kind: &str, property: &str, context: &ExportContext) -> Option<Element> {
    match (kind, property) {
        ("Report", "AuxiliaryVariantForm") if context.is_xml_2_21() => Some(el(property)),
        _ => None,
    }
}

/// The name of a generated type of an object (or of its tabular section).
pub(crate) fn generated_type_name(
    kind: &str,
    category: &str,
    object: &str,
    section: Option<&str>,
) -> String {
    match (kind, category, section) {
        (_, _, Some(section)) => format!("{kind}{category}.{object}.{section}"),
        ("ChartOfCharacteristicTypes", "Characteristic", _) => format!("Characteristic.{object}"),
        (
            "ChartOfCalculationTypes",
            "DisplacingCalculationTypes"
            | "DisplacingCalculationTypesRow"
            | "BaseCalculationTypes"
            | "BaseCalculationTypesRow"
            | "LeadingCalculationTypes"
            | "LeadingCalculationTypesRow",
            _,
        ) => format!("{category}.{object}"),
        _ => format!("{kind}{category}.{object}"),
    }
}

fn generated_type_element(name: String, category: &str, type_id: &str, value_id: &str) -> Element {
    el("xr:GeneratedType")
        .attr("name", name)
        .attr("category", category)
        .child(leaf("xr:TypeId", type_id))
        .child(leaf("xr:ValueId", value_id))
}

fn modern(compat: Compat) -> bool {
    compat >= Compat(8, 3, 27)
}

fn since_8_5_1(compat: Compat) -> bool {
    compat >= Compat(8, 5, 1)
}

/// How many owner-record values a slot takes.
fn slot_width(slot: &Slot, compat: Compat) -> usize {
    match slot {
        Slot::Generated(_) => 2,
        Slot::Modern(inner) => {
            if modern(compat) {
                slot_width(inner, compat)
            } else {
                0
            }
        }
        Slot::Since8_5_1(inner) => {
            if since_8_5_1(compat) {
                slot_width(inner, compat)
            } else {
                0
            }
        }
        _ => 1,
    }
}

/// The object's md header from its owner record.
fn owner_header(layout: &Layout, owner: &[Brace], compat: Compat) -> Result<Header> {
    let mut position = 0;
    for slot in layout.slots {
        match slot {
            Slot::Header => return header(item(owner, position)?),
            Slot::WrappedHeader => return header(item(list(item(owner, position)?)?, 1)?),
            _ => position += slot_width(slot, compat),
        }
    }
    bail!("layout has no header slot")
}

/// What the owner-record walk collects.
struct Walk {
    properties: HashMap<&'static str, Element>,
    generated: Vec<(&'static str, String, String)>,
    this_node: Option<String>,
}

pub(crate) fn decode(kind: &str, row: &Brace, context: &ExportContext) -> Result<Element> {
    let layout = layout(kind).ok_or_else(|| anyhow!("{kind} has no layout"))?;
    let spec = spec(kind)?;
    let compat = context.compat;
    let root = list(row)?;
    if atom(item(root, 0)?)? != "1" {
        bail!("not a descriptor row: {}", short(row));
    }
    let owner_record = list(item(root, 1)?)?;
    let head = owner_header(layout, owner_record, compat)?;
    let full_name = format!("{kind}.{}", head.name);
    let owner = Owner {
        kind,
        full_name: &full_name,
    };

    let mut walk = Walk {
        properties: HashMap::new(),
        generated: Vec::new(),
        this_node: None,
    };
    let mut position = 0;
    for slot in layout.slots {
        decode_slot(
            slot,
            owner_record,
            &mut position,
            &mut walk,
            &head,
            owner,
            context,
        )
        .with_context(|| format!("slot {position} ({slot:?})"))?;
    }
    if position != owner_record.len() {
        bail!(
            "owner record has {} values, the layout reads {position}",
            owner_record.len()
        );
    }

    // InternalInfo: the exchange plan's node first, then the generated
    // types in stored order.
    let mut internal = el("InternalInfo");
    if let Some(node) = walk.this_node.take() {
        internal.children.push(leaf("xr:ThisNode", node));
    }
    for (category, type_id, value_id) in &walk.generated {
        internal.children.push(generated_type_element(
            generated_type_name(kind, category, &head.name, None),
            category,
            type_id,
            value_id,
        ));
    }

    let mut properties = el("Properties");
    for name in spec.properties {
        match walk.properties.remove(name) {
            Some(element) => properties.children.push(element),
            None if *name == "StandardAttributes" => {}
            None => match xml_only_default(kind, name, context) {
                Some(element) => properties.children.push(element),
                None if !context.is_xml_2_21() && XML_2_21_ONLY.contains(name) => {}
                None => bail!("no slot decodes <{name}>"),
            },
        }
    }
    if let Some(left) = walk.properties.keys().next() {
        bail!("decoded <{left}> has no place in the XML order");
    }

    // Child objects by kind, each collection in stored order.
    let mut collections: HashMap<&str, &Brace> = HashMap::new();
    let count = number(item(root, 2)?)? as usize;
    for collection in root.iter().skip(3).take(count) {
        let fields = list(collection)?;
        collections.insert(atom(item(fields, 0)?)?, collection);
    }
    let mut children = el("ChildObjects");
    for tag in spec.children {
        for (class, content) in layout.collections {
            if collection_tag(content) != Some(*tag) {
                continue;
            }
            let Some(stored) = collections.get(class) else {
                bail!("row has no collection {class}");
            };
            let fields = list(stored)?;
            let declared = number(item(fields, 1)?)? as usize;
            for stored_item in fields.iter().skip(2).take(declared) {
                children.children.push(decode_child(
                    kind,
                    &spec,
                    content,
                    stored_item,
                    owner,
                    context,
                )?);
            }
        }
    }

    Ok(el(kind)
        .attr("uuid", head.uuid.clone())
        .child(internal)
        .child(properties)
        .child(children))
}

/// The XML element kind a collection holds.
fn collection_tag(content: &Coll) -> Option<&'static str> {
    Some(match content {
        Coll::Templates => "Template",
        Coll::Forms => "Form",
        Coll::Commands(_) => "Command",
        Coll::Children(tag, _) => tag,
        Coll::TabularSections { .. } => "TabularSection",
        Coll::EnumValues => "EnumValue",
        Coll::Empty => return None,
    })
}

#[allow(clippy::too_many_arguments)]
fn decode_slot(
    slot: &Slot,
    record: &[Brace],
    position: &mut usize,
    walk: &mut Walk,
    head: &Header,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<()> {
    let names = &context.names;
    let compat = context.compat;
    match *slot {
        Slot::Modern(inner) => {
            if modern(compat) {
                decode_slot(inner, record, position, walk, head, owner, context)?;
            }
            return Ok(());
        }
        Slot::Since8_5_1(inner) => {
            if since_8_5_1(compat) {
                decode_slot(inner, record, position, walk, head, owner, context)?;
            }
            return Ok(());
        }
        Slot::Generated(category) => {
            let type_id = atom(item(record, *position)?)?.to_string();
            let value_id = atom(item(record, *position + 1)?)?.to_string();
            walk.generated.push((category, type_id, value_id));
            *position += 2;
            return Ok(());
        }
        _ => {}
    }
    let value = item(record, *position)?;
    *position += 1;
    if let Slot::ThisNode = slot {
        walk.this_node = Some(atom(value)?.to_string());
        return Ok(());
    }
    let mut put = |name: &'static str, element: Element| {
        walk.properties.insert(name, element);
    };
    match *slot {
        Slot::Tag(old, new, latest) => {
            let expected = if since_8_5_1(compat) {
                latest
            } else if modern(compat) {
                new
            } else {
                old
            };
            let stored = number(value)?;
            if stored != expected {
                bail!("record version {stored}, the compatibility mode stores {expected}");
            }
        }
        Slot::Header | Slot::WrappedHeader => {
            let [name, synonym, comment] = header_elements(head)?;
            put("Name", name);
            put("Synonym", synonym);
            put("Comment", comment);
        }
        Slot::Flag(name) => put(name, leaf(name, bool_text(value)?)),
        Slot::Number(name) => put(name, leaf(name, atom(value)?)),
        Slot::Code(name, table) => put(name, leaf(name, code_text(value, table)?)),
        Slot::Text(name) => put(
            name,
            leaf(
                name,
                crate::metadata_model::export::xml_text(string(value)?),
            ),
        ),
        Slot::Localized(name) => put(name, localized_element(name, value)?),
        Slot::Reference(name) => put(name, leaf(name, reference_name(atom(value)?, names)?)),
        Slot::References(name) => {
            let fields = list(value)?;
            let count = number(item(fields, 1)?)? as usize;
            let mut uuids = Vec::with_capacity(count);
            for reference in fields.iter().skip(2).take(count) {
                let payload = list(item(list(reference)?, 2)?)?;
                uuids.push(atom(item(payload, 1)?)?);
            }
            // A document's register records are a set: the XML lists them
            // in uuid order even where the row kept an append order.
            if name == "RegisterRecords" {
                uuids.sort_unstable();
            }
            let mut element = el(name);
            for uuid in uuids {
                element.children.push(
                    leaf("xr:Item", reference_name(uuid, names)?).attr("type", "xr:MDObjectRef"),
                );
            }
            put(name, element);
        }
        Slot::Fields(name) => {
            let body = list(item(list(value)?, 1)?)?;
            let count = number(item(body, 1)?)? as usize;
            let mut element = el(name);
            for field in body.iter().skip(2).take(count) {
                let segment = item(list(field)?, 2)?;
                element.children.push(leaf(
                    "xr:Field",
                    field_text(segment, owner.full_name, names)?,
                ));
            }
            put(name, element);
        }
        Slot::InputModes => {
            let modes = list(value)?;
            put(
                "SearchStringModeOnInputByString",
                leaf(
                    "SearchStringModeOnInputByString",
                    code_text(item(modes, 0)?, &[("Begin", 1), ("AnyPart", 2)])?,
                ),
            );
            put(
                "FullTextSearchOnInputByString",
                leaf(
                    "FullTextSearchOnInputByString",
                    code_text(item(modes, 1)?, &[("Use", 1), ("DontUse", 2)])?,
                ),
            );
            put(
                "ChoiceDataGetModeOnInputByString",
                leaf(
                    "ChoiceDataGetModeOnInputByString",
                    code_text(item(modes, 2)?, &[("Directly", 0), ("Background", 1)])?,
                ),
            );
        }
        Slot::TypePattern(name) => put(name, type_element(name, value, names)?),
        Slot::StandardAttributes(codes) => {
            if let Some(element) =
                standard_attributes_element("StandardAttributes", value, codes, owner, context)?
            {
                put("StandardAttributes", element);
            }
        }
        Slot::StandardTabularSections(definitions) => {
            put(
                "StandardTabularSections",
                standard_tabular_sections(value, definitions, owner, context)?,
            );
        }
        Slot::Characteristics => put("Characteristics", characteristics(value, context)?),
        // Constants and nil slots carry nothing the XML writes; the
        // lossless check compares them.
        Slot::Const(_) | Slot::Nil => {}
        Slot::Generated(_) | Slot::Modern(_) | Slot::Since8_5_1(_) | Slot::ThisNode => {
            unreachable!()
        }
    }
    Ok(())
}

/// `{1,{0,<n>,<marker>,{3,<synonym>,"comment",<fill>,0,<attrs>,<tooltip>}...}}`.
fn standard_tabular_sections(
    node: &Brace,
    definitions: super::parts::StandardSections,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let mut element = el("StandardTabularSections");
    let fields = list(node)?;
    if atom(item(fields, 0)?)? == "0" {
        return Ok(element);
    }
    let body = list(item(fields, 1)?)?;
    let count = number(item(body, 1)?)? as usize;
    for index in 0..count {
        let marker = number(item(body, 2 + 2 * index)?)?;
        let section = list(item(body, 3 + 2 * index)?)?;
        let (name, _, codes) = definitions
            .iter()
            .find(|(_, candidate, _)| *candidate == marker)
            .ok_or_else(|| anyhow!("unknown standard tabular section {marker}"))?;
        let mut xml = el("xr:StandardTabularSection")
            .attr("name", *name)
            .child(localized_element("xr:Synonym", item(section, 1)?)?)
            .child(leaf(
                "xr:Comment",
                crate::metadata_model::export::xml_text(string(item(section, 2)?)?),
            ))
            .child(localized_element("xr:ToolTip", item(section, 6)?)?)
            .child(leaf(
                "xr:FillChecking",
                code_text(item(section, 3)?, super::parts::FILL_CHECKING)?,
            ));
        let attributes = item(section, 5)?;
        let mut block = el("xr:StandardAttributes");
        if list(attributes)?.len() > 1 {
            block.children.extend(standard_attribute_elements(
                attributes, codes, owner, context,
            )?);
        }
        xml.children.push(block);
        element.children.push(xml);
    }
    Ok(element)
}

/// `{0,{<n>,{"#",<characteristic>,{4,...}}...}}` -> `<Characteristics>`.
fn characteristics(node: &Brace, context: &ExportContext) -> Result<Element> {
    let names = &context.names;
    let mut element = el("Characteristics");
    let body = list(item(list(node)?, 1)?)?;
    let count = number(item(body, 0)?)? as usize;
    for typed in body.iter().skip(1).take(count) {
        let fields = list(item(list(typed)?, 2)?)?;
        let source = |index: usize| -> Result<String> {
            let reference = list(item(fields, index)?)?;
            reference_name(atom(item(reference, 1)?)?, names)
        };
        let types_from = source(1)?;
        let values_from = source(2)?;
        let field = |index: usize, base: &str| -> Result<String> {
            match fields.get(index) {
                // Layouts that stop short write the undefined sentinel.
                None => Ok("-1".to_string()),
                Some(value) => field_text(item(list(value)?, 1)?, base, names),
            }
        };
        let types = el("xr:CharacteristicTypes")
            .attr("from", types_from.clone())
            .child(leaf("xr:KeyField", field(8, &types_from)?))
            .child(leaf("xr:TypesFilterField", field(6, &types_from)?))
            .child(value_element(
                "xr:TypesFilterValue",
                item(fields, 7)?,
                names,
            )?)
            .child(leaf("xr:DataPathField", field(9, &types_from)?))
            .child(leaf("xr:MultipleValuesUseField", field(10, &types_from)?));
        let values = el("xr:CharacteristicValues")
            .attr("from", values_from.clone())
            .child(leaf("xr:ObjectField", field(3, &values_from)?))
            .child(leaf("xr:TypeField", field(4, &values_from)?))
            .child(leaf("xr:ValueField", field(5, &values_from)?))
            .child(leaf("xr:MultipleValuesKeyField", field(11, &values_from)?))
            .child(leaf(
                "xr:MultipleValuesOrderField",
                field(12, &values_from)?,
            ));
        element
            .children
            .push(el("xr:Characteristic").child(types).child(values));
    }
    Ok(element)
}

fn decode_child(
    kind: &str,
    spec: &Spec,
    content: &Coll,
    stored: &Brace,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let names = &context.names;
    let modern = modern(context.compat);
    match *content {
        Coll::Forms | Coll::Templates => {
            let tag = if matches!(content, Coll::Forms) {
                "Form"
            } else {
                "Template"
            };
            let uuid = atom(stored)?;
            let full = names
                .name(uuid)
                .ok_or_else(|| anyhow!("no name for {tag} {uuid}"))?;
            let prefix = format!("{}.{tag}.", owner.full_name);
            let short_name = full
                .strip_prefix(&prefix)
                .ok_or_else(|| anyhow!("{tag} {full} is not owned by {}", owner.full_name))?;
            Ok(leaf(tag, short_name))
        }
        Coll::Commands(wrapper) => {
            let inner = item(list(item(list(stored)?, 0)?)?, 1)?;
            let body = match wrapper {
                CommandWrapper::Owner => item(list(inner)?, 3)?,
                CommandWrapper::Bare => inner,
            };
            command(body, context)
        }
        Coll::Children(tag, wrapper) => {
            let record = list(item(list(stored)?, 0)?)?;
            let wrapper = if modern { wrapper.modern } else { wrapper.old };
            let (with_fill, tail) = match tag {
                "Attribute" => (spec.attribute_fill, spec.attribute_tail),
                "AccountingFlag" | "ExtDimensionAccountingFlag" => (true, &["DataHistory"][..]),
                "AddressingAttribute" => (
                    true,
                    &[
                        "Indexing",
                        "AddressingDimension",
                        "FullTextSearch",
                        "DataHistory",
                    ][..],
                ),
                other => bail!("unknown attribute kind {other}"),
            };
            attribute(tag, record, wrapper, with_fill, tail, owner, context)
        }
        Coll::TabularSections {
            wrapper,
            attribute: attribute_wrapper,
            ..
        } => {
            let wrapper = if modern { wrapper.modern } else { wrapper.old };
            tabular_section(
                kind,
                spec,
                stored,
                wrapper,
                attribute_wrapper,
                owner,
                context,
            )
        }
        Coll::EnumValues => {
            let record = list(item(list(stored)?, 0)?)?;
            let head = header(item(record, 1)?)?;
            let [name, synonym, comment] = header_elements(&head)?;
            let mut properties = el("Properties").child(name).child(synonym).child(comment);
            if context.is_xml_2_21() {
                let color = match record.get(2) {
                    Some(color) => enum_value_color(color)?,
                    None => "auto".to_string(),
                };
                properties.children.push(leaf("Color", color));
            }
            Ok(el("EnumValue").attr("uuid", head.uuid).child(properties))
        }
        Coll::Empty => bail!("an empty collection holds items"),
    }
}

/// `{4,4,{0},4}` -> `auto`; `{4,4,{<index>},5}` -> `pal:<name>`.
fn enum_value_color(node: &Brace) -> Result<String> {
    let fields = list(node)?;
    let index = number(item(list(item(fields, 2)?)?, 0)?)?;
    Ok(match (atom(item(fields, 3)?)?, index) {
        ("4", 0) => "auto",
        ("5", 0) => "pal:FirstBrand",
        ("5", 1) => "pal:SecondBrand",
        ("5", 2) => "pal:Red",
        ("5", 3) => "pal:Orange",
        ("5", 4) => "pal:Yellow",
        ("5", 5) => "pal:Green",
        ("5", 6) => "pal:LightBlue",
        ("5", 7) => "pal:Blue",
        ("5", 15) => "pal:Gray",
        _ => bail!("unsupported enum value colour {}", short(node)),
    }
    .to_string())
}

const INDEXING: &[(&str, i64)] = &[
    ("DontIndex", 0),
    ("Index", 1),
    ("IndexWithAdditionalOrder", 2),
];
const ATTRIBUTE_USE: &[(&str, i64)] = &[("ForItem", 0), ("ForFolder", 1), ("ForFolderAndItem", 2)];
const USE: &[(&str, i64)] = &[("DontUse", 0), ("Use", 1)];

/// An attribute-like child: its wrapper's own slots and the shared body.
fn attribute(
    tag: &str,
    record: &[Brace],
    wrapper: AttributeWrapper,
    with_fill: bool,
    tail_order: &[&str],
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let body = item(record, 1)?;
    let head = attribute_header(body)?;
    let mut tail: HashMap<&str, Element> = HashMap::new();
    let mut indexing = |value: &Brace| -> Result<()> {
        tail.insert("Indexing", leaf("Indexing", code_text(value, INDEXING)?));
        Ok(())
    };
    match wrapper {
        AttributeWrapper::Bare => {}
        AttributeWrapper::Plain(_)
        | AttributeWrapper::PlainModern(_)
        | AttributeWrapper::TabularSection => {
            indexing(item(record, 2)?)?;
            tail.insert(
                "FullTextSearch",
                leaf("FullTextSearch", code_text(item(record, 3)?, USE)?),
            );
            tail.insert(
                "DataHistory",
                leaf("DataHistory", code_text(item(record, 4)?, USE)?),
            );
        }
        AttributeWrapper::Hierarchical(_) | AttributeWrapper::HierarchicalModern(_) => {
            indexing(item(record, 2)?)?;
            tail.insert(
                "Use",
                leaf("Use", code_text(item(record, 3)?, ATTRIBUTE_USE)?),
            );
            tail.insert(
                "FullTextSearch",
                leaf("FullTextSearch", code_text(item(record, 4)?, USE)?),
            );
            tail.insert(
                "DataHistory",
                leaf("DataHistory", code_text(item(record, 5)?, USE)?),
            );
        }
        AttributeWrapper::Addressing(_) => {
            indexing(item(record, 2)?)?;
            tail.insert(
                "AddressingDimension",
                leaf(
                    "AddressingDimension",
                    reference_name(atom(item(record, 3)?)?, &context.names)?,
                ),
            );
            tail.insert(
                "FullTextSearch",
                leaf("FullTextSearch", code_text(item(record, 4)?, USE)?),
            );
            tail.insert(
                "DataHistory",
                leaf("DataHistory", code_text(item(record, 5)?, USE)?),
            );
        }
        AttributeWrapper::Flag(_) => {
            tail.insert(
                "DataHistory",
                leaf("DataHistory", code_text(item(record, 2)?, USE)?),
            );
        }
    }
    let mut ordered = Vec::with_capacity(tail_order.len());
    for name in tail_order {
        ordered.push(
            tail.remove(name)
                .ok_or_else(|| anyhow!("attribute wrapper has no <{name}>"))?,
        );
    }
    let properties = attribute_properties(body, with_fill, ordered, owner, context)?;
    Ok(el(tag).attr("uuid", head.uuid).child(properties))
}

/// `{<wrapped>,1,{<attribute class>,<n>,<attribute>...}}`.
#[allow(clippy::too_many_arguments)]
fn tabular_section(
    kind: &str,
    spec: &Spec,
    stored: &Brace,
    wrapper: TsWrapper,
    attribute_wrapper: AttributeWrapper,
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let fields = list(stored)?;
    let wrapped = list(item(fields, 0)?)?;
    let record = list(item(wrapped, 1)?)?;
    if atom(item(record, 0)?)? != "11" {
        bail!("not a tabular section record: {}", short(item(wrapped, 1)?));
    }
    let head = header(item(list(item(record, 5)?)?, 1)?)?;
    let object_name = owner
        .full_name
        .split_once('.')
        .map(|(_, name)| name)
        .unwrap_or_default();
    let internal = el("InternalInfo")
        .child(generated_type_element(
            generated_type_name(kind, "TabularSection", object_name, Some(&head.name)),
            "TabularSection",
            atom(item(record, 1)?)?,
            atom(item(record, 2)?)?,
        ))
        .child(generated_type_element(
            generated_type_name(kind, "TabularSectionRow", object_name, Some(&head.name)),
            "TabularSectionRow",
            atom(item(record, 3)?)?,
            atom(item(record, 4)?)?,
        ));
    let [name, synonym, comment] = header_elements(&head)?;
    let mut properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(localized_element("ToolTip", item(record, 8)?)?)
        .child(leaf(
            "FillChecking",
            code_text(item(record, 6)?, super::parts::FILL_CHECKING)?,
        ));
    let codes = [("LineNumber", line_number_marker(kind))];
    if let Some(block) = standard_attributes_element(
        "StandardAttributes",
        item(record, 7)?,
        &codes,
        owner,
        context,
    )? {
        properties.children.push(block);
    }
    let (usage, length) = match wrapper {
        TsWrapper::Bare(_) => (None, None),
        TsWrapper::Use(_) => (Some(item(wrapped, 2)?), None),
        TsWrapper::Length(_) => (None, Some(item(wrapped, 2)?)),
        TsWrapper::UseAndLength(_) => (Some(item(wrapped, 2)?), Some(item(wrapped, 3)?)),
    };
    for property in spec.section_tail {
        match *property {
            "Use" => {
                let usage = usage.ok_or_else(|| anyhow!("tabular section wrapper has no Use"))?;
                properties
                    .children
                    .push(leaf("Use", code_text(usage, ATTRIBUTE_USE)?));
            }
            "LineNumberLength" => {
                let length = match length {
                    Some(length) => atom(length)?.to_string(),
                    // Rows before 8.3.27 do not store it; the XML writes the default.
                    None => "5".to_string(),
                };
                properties.children.push(leaf("LineNumberLength", length));
            }
            other => bail!("unknown tabular section property {other}"),
        }
    }
    let attributes = list(item(fields, 2)?)?;
    let count = number(item(attributes, 1)?)? as usize;
    let mut children = el("ChildObjects");
    for stored_attribute in attributes.iter().skip(2).take(count) {
        let record = list(item(list(stored_attribute)?, 0)?)?;
        let with_fill = spec.section_attribute_fill
            || (kind == "Document" && attribute_has_nondefault_fill(item(record, 1)?)?);
        children.children.push(attribute(
            "Attribute",
            record,
            attribute_wrapper,
            with_fill,
            spec.section_attribute_tail,
            owner,
            context,
        )?);
    }
    Ok(el("TabularSection")
        .attr("uuid", head.uuid)
        .child(internal)
        .child(properties)
        .child(children))
}

// ---------------------------------------------------------------------------
// Commands.

fn builtin_command_group(uuid: &str) -> Option<&'static str> {
    Some(match uuid {
        "77ea1b8f-dd79-4717-9dba-5628e7f348cf" => "NavigationPanelOrdinary",
        "bc80566a-86a5-4e87-acd4-872239385a2e" => "NavigationPanelSeeAlso",
        "1af6d528-0b86-4fba-ab95-bd7475db03ba" => "NavigationPanelImportant",
        "4f499c31-050b-47c5-aa84-d0366c0a0da8" => "ActionsPanelCreate",
        "5b360bff-01a1-49b6-93d2-26e7e8e3a038" => "ActionsPanelReports",
        "aabb34e1-98c1-4bd0-bf7f-243f95437b44" => "ActionsPanelTools",
        "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0" => "FormCommandBarCreateBasedOn",
        "cb50f5c0-8013-4262-93a2-f0db379d6b6b" => "FormCommandBarImportant",
        "eacad741-96b9-4b3a-bf79-dde9ecead1a1" => "FormNavigationPanelGoTo",
        "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8" => "FormNavigationPanelSeeAlso",
        "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2" => "FormNavigationPanelImportant",
        _ => return None,
    })
}

fn std_picture_by_code(code: i64) -> Option<&'static str> {
    Some(match code {
        -1 => "InputFieldSelect",
        -2 => "InputFieldClear",
        -3 => "MoveUp",
        -4 => "MoveDown",
        -5 => "InputFieldCalendar",
        -7 => "InputFieldOpen",
        -8 => "MoveLeft",
        -9 => "MoveRight",
        -10 => "CheckAll",
        -11 => "UncheckAll",
        -13 => "Print",
        -14 => "InputFieldChooseType",
        -15 => "ZoomOut",
        -16 => "ZoomIn",
        -100 => "Select",
        _ => return None,
    })
}

/// `{4,<present>,<value>,"",<x>,<y>,<load transparent>,0,""}` -> `<Picture>`.
fn picture(node: &Brace, context: &ExportContext) -> Result<Element> {
    let fields = list(node)?;
    if atom(item(fields, 1)?)? == "0" {
        return Ok(el("Picture"));
    }
    let value = list(item(fields, 2)?)?;
    let reference = match value {
        [code] => {
            let code = number(code)?;
            format!(
                "StdPicture.{}",
                std_picture_by_code(code).ok_or_else(|| anyhow!("unknown picture code {code}"))?
            )
        }
        [_, uuid] => {
            let uuid = atom(uuid)?;
            match standard_picture_name(uuid) {
                Some(name) => name.to_string(),
                None => context
                    .names
                    .name(uuid)
                    .map(str::to_string)
                    .ok_or_else(|| anyhow!("no name for picture {uuid}"))?,
            }
        }
        _ => bail!("unsupported picture {}", short(node)),
    };
    let mut element = el("Picture")
        .child(leaf("xr:Ref", reference))
        .child(leaf("xr:LoadTransparent", bool_text(item(fields, 6)?)?));
    let x = atom(item(fields, 4)?)?;
    let y = atom(item(fields, 5)?)?;
    if x != "-1" || y != "-1" {
        element
            .children
            .push(el("xr:TransparentPixel").attr("x", x).attr("y", y));
    }
    Ok(element)
}

/// `{0,<key>,<modifiers>}` -> `Ctrl+Alt+Shift+Key`; empty for `{0,0,0}`.
pub(crate) fn shortcut(node: &Brace) -> Result<String> {
    let fields = list(node)?;
    let code = number(item(fields, 1)?)?;
    let mask = number(item(fields, 2)?)?;
    if code == 0 {
        return Ok(String::new());
    }
    let key = match code {
        8 => "BackSpace".to_string(),
        9 => "Tab".to_string(),
        13 => "Enter".to_string(),
        27 => "Esc".to_string(),
        32 => "Space".to_string(),
        33 => "PageUp".to_string(),
        34 => "PageDown".to_string(),
        35 => "End".to_string(),
        36 => "Home".to_string(),
        37 => "Left".to_string(),
        38 => "Up".to_string(),
        39 => "Right".to_string(),
        40 => "Down".to_string(),
        45 => "Insert".to_string(),
        46 => "Delete".to_string(),
        48..=57 | 65..=90 => char::from(code as u8).to_string(),
        96..=105 => format!("Num {}", code - 96),
        106 => "Num *".to_string(),
        107 => "Num +".to_string(),
        109 => "Num -".to_string(),
        110 => "Num .".to_string(),
        111 => "Num /".to_string(),
        112..=123 => format!("F{}", code - 111),
        other => bail!("unknown shortcut key {other}"),
    };
    let mut parts = Vec::new();
    if mask & 8 != 0 {
        parts.push("Ctrl".to_string());
    }
    if mask & 16 != 0 {
        parts.push("Alt".to_string());
    }
    if mask & 4 != 0 {
        parts.push("Shift".to_string());
    }
    parts.push(key);
    Ok(parts.join("+"))
}

/// `{1,{2,<uuid>,<command value>},{9,...}}` -> `<Command>`.
pub(crate) fn command(body: &Brace, context: &ExportContext) -> Result<Element> {
    let names = &context.names;
    let fields = list(body)?;
    let identity = list(item(fields, 1)?)?;
    let uuid = atom(item(identity, 1)?)?;
    let record = list(item(fields, 2)?)?;
    if atom(item(record, 0)?)? != "9" {
        bail!("not a command record: {}", short(item(fields, 2)?));
    }
    let head = header(item(record, 9)?)?;
    let [name, synonym, comment] = header_elements(&head)?;
    let group_uuid = atom(item(list(item(record, 7)?)?, 1)?)?;
    let group = match builtin_command_group(group_uuid) {
        Some(group) => group.to_string(),
        None => reference_name(group_uuid, names)?,
    };
    let properties = el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(leaf("Group", group))
        .child(type_element(
            "CommandParameterType",
            item(record, 8)?,
            names,
        )?)
        .child(leaf(
            "ParameterUseMode",
            code_text(item(record, 11)?, &[("Single", 0), ("Multiple", 1)])?,
        ))
        .child(leaf("ModifiesData", bool_text(item(record, 10)?)?))
        .child(leaf(
            "Representation",
            code_text(
                item(record, 2)?,
                &[
                    ("Text", 0),
                    ("Picture", 1),
                    ("PictureAndText", 2),
                    ("Auto", 3),
                ],
            )?,
        ))
        .child(localized_element("ToolTip", item(record, 3)?)?)
        .child(picture(item(record, 1)?, context)?)
        .child(leaf("Shortcut", shortcut(item(record, 5)?)?))
        .child(leaf(
            "OnMainServerUnavalableBehavior",
            code_text(item(record, 12)?, &[("Auto", 0)])?,
        ));
    Ok(el("Command").attr("uuid", uuid).child(properties))
}

// ---------------------------------------------------------------------------
// Names.

/// What a reference object's row contributes to a name index.
pub(crate) fn names(kind: &str, row: &Brace) -> Result<ObjectNames> {
    let layout = layout(kind).ok_or_else(|| anyhow!("{kind} has no layout"))?;
    let root = list(row)?;
    let owner_record = list(item(root, 1)?)?;
    // The record versions only shift slots that follow every name bearer,
    // so the widest reading is exact for the names.
    let compat = Compat(8, 5, 1);
    let head = owner_header(layout, owner_record, compat)?;
    let object_name = head.name.clone();
    let full_name = format!("{kind}.{object_name}");
    let mut out = ObjectNames {
        uuid: head.uuid.clone(),
        full_name: full_name.clone(),
        children: Vec::new(),
        types: Vec::new(),
    };
    let mut position = 0;
    for slot in layout.slots {
        if let Slot::Generated(category) = slot {
            out.types.push(GeneratedTypeName {
                name: generated_type_name(kind, category, &head.name, None),
                category: category.to_string(),
                type_id: atom(item(owner_record, position)?)?.to_string(),
                value_id: atom(item(owner_record, position + 1)?)?.to_string(),
            });
        }
        position += slot_width(slot, compat);
        if position >= owner_record.len() {
            break;
        }
    }
    let count = number(item(root, 2)?)? as usize;
    let collections: HashMap<&str, &Brace> = root
        .iter()
        .skip(3)
        .take(count)
        .filter_map(|collection| {
            let fields = collection.as_list()?;
            Some((fields.first()?.as_atom()?, collection))
        })
        .collect();
    for (class, content) in layout.collections {
        let Some(stored) = collections.get(class) else {
            continue;
        };
        let fields = list(stored)?;
        let declared = number(item(fields, 1)?)? as usize;
        for stored_item in fields.iter().skip(2).take(declared) {
            match content {
                Coll::Children(tag, _) => {
                    let record = list(item(list(stored_item)?, 0)?)?;
                    let head = attribute_header(item(record, 1)?)?;
                    out.children
                        .push((format!("{full_name}.{tag}.{}", head.name), head.uuid));
                }
                Coll::Commands(wrapper) => {
                    let inner = item(list(item(list(stored_item)?, 0)?)?, 1)?;
                    let body = match wrapper {
                        CommandWrapper::Owner => item(list(inner)?, 3)?,
                        CommandWrapper::Bare => inner,
                    };
                    let record = list(item(list(body)?, 2)?)?;
                    let head = header(item(record, 9)?)?;
                    out.children
                        .push((format!("{full_name}.Command.{}", head.name), head.uuid));
                }
                Coll::EnumValues => {
                    let record = list(item(list(stored_item)?, 0)?)?;
                    let head = header(item(record, 1)?)?;
                    out.children
                        .push((format!("{full_name}.EnumValue.{}", head.name), head.uuid));
                }
                Coll::TabularSections { .. } => {
                    let section = list(stored_item)?;
                    let record = list(item(list(item(section, 0)?)?, 1)?)?;
                    let head = header(item(list(item(record, 5)?)?, 1)?)?;
                    let section_full = format!("{full_name}.TabularSection.{}", head.name);
                    for (category, type_index) in [("TabularSection", 1), ("TabularSectionRow", 3)]
                    {
                        out.types.push(GeneratedTypeName {
                            name: generated_type_name(
                                kind,
                                category,
                                &object_name,
                                Some(&head.name),
                            ),
                            category: category.to_string(),
                            type_id: atom(item(record, type_index)?)?.to_string(),
                            value_id: atom(item(record, type_index + 1)?)?.to_string(),
                        });
                    }
                    out.children.push((section_full.clone(), head.uuid));
                    let attributes = list(item(section, 2)?)?;
                    let count = number(item(attributes, 1)?)? as usize;
                    for stored_attribute in attributes.iter().skip(2).take(count) {
                        let record = list(item(list(stored_attribute)?, 0)?)?;
                        let head = attribute_header(item(record, 1)?)?;
                        out.children
                            .push((format!("{section_full}.Attribute.{}", head.name), head.uuid));
                    }
                }
                Coll::Forms | Coll::Templates | Coll::Empty => {}
            }
        }
    }
    Ok(out)
}
