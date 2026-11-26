//! Current enum identities; native XDTO names cannot recover an EDT EPackage.
//! Protocol map derived from original XdtoTypeMap (mapping.json SHA-256
//! 6ad6f93752dd21747da8cc90385d5e075aa1cccc88a81b59ac9e0162ed35cb15).
use super::*;
use morph1c_core::ir::form::ChartTypedValue;
use morph1c_core::ir::value::{PropertyValue, ValueScalarKind, ValueSpec};

const CFG: &str = "http://v8.1c.ru/8.1/data/enterprise/current-config";
const UI: &str = "http://v8.1c.ru/8.1/data/ui";
const COMMON: &str = "http://g5.1c.ru/v8/dt/metadata/common";
const LINE: &[&str] = &[
    "None",
    "Solid",
    "Dotted",
    "Dashed",
    "DashDotted",
    "DashDottedDotted",
];
const NAMESPACES: &[&str] = &[
    "http://v8.1c.ru/8.1/data-composition-system/core",
    "http://v8.1c.ru/8.1/data-composition-system/details",
    "http://v8.1c.ru/8.1/data-composition-system/settings",
    "http://v8.1c.ru/8.1/data/core",
    "http://v8.1c.ru/8.1/data/enterprise",
    "http://v8.1c.ru/8.1/data/txtedt",
    "http://v8.1c.ru/8.1/data/ui",
    "http://v8.1c.ru/8.2/data/chart",
    "http://v8.1c.ru/8.2/data/data-analysis",
    "http://v8.1c.ru/8.2/data/formatted-document",
    "http://v8.1c.ru/8.2/data/geo",
    "http://v8.1c.ru/8.2/data/graphscheme",
    "http://v8.1c.ru/8.2/data/spreadsheet",
    "http://v8.1c.ru/8.2/managed-application/core",
    "http://v8.1c.ru/8.2/misc",
    "http://v8.1c.ru/8.3/data/entext",
    "http://v8.1c.ru/8.3/data/pdf",
    "http://v8.1c.ru/8.3/data/planner",
    "http://www.w3.org/2001/XMLSchema",
];
const XDTO: &[(&str, &str, &str, usize)] = &[
    ("AccountType", "ent", "AccountType", 4),
    ("AccountingRecordType", "ent", "AccountingRecordType", 4),
    ("AccumulationRecordType", "ent", "AccumulationRecordType", 4),
    (
        "AccumulationRegisterAggregatePeriodicity",
        "ent",
        "AccumulationRegisterAggregatePeriodicity",
        4,
    ),
    (
        "AccumulationRegisterAggregateUse",
        "ent",
        "AccumulationRegisterAggregateUse",
        4,
    ),
    ("Alias", "app", "Alias", 13),
    ("AllowedLength", "v8", "AllowedLength", 3),
    ("AllowedSign", "v8", "AllowedSign", 3),
    ("AnalysisDataType", "data-analysis", "AnalysisDataType", 8),
    ("AppearanceArea", "entext", "AppearanceArea", 15),
    ("AppearanceAreaItem", "entext", "AppearanceAreaItem", 15),
    ("AppearanceAreaType", "entext", "AppearanceAreaType", 15),
    ("AppearanceSetting", "entext", "AppearanceSetting", 15),
    (
        "AppearanceSettingItem",
        "entext",
        "AppearanceSettingItem",
        15,
    ),
    ("ApplicationUsePurpose", "app", "ApplicationUsePurpose", 13),
    (
        "AssociationRulesDataSourceType",
        "data-analysis",
        "AssociationRulesDataSourceType",
        8,
    ),
    (
        "AssociationRulesPruneType",
        "data-analysis",
        "AssociationRulesPruneType",
        8,
    ),
    ("AutoTimeMode", "ent", "AutoTimeMode", 4),
    ("BinaryData", "xs", "base64Binary", 18),
    ("Boolean", "xs", "boolean", 18),
    ("Border", "v8ui", "Border", 6),
    (
        "CalculationRegisterPeriodType",
        "ent",
        "CalculationRegisterPeriodType",
        4,
    ),
    ("Chart", "chr", "Chart", 7),
    ("ChartAxis", "chr", "ChartAxis", 7),
    (
        "ChartBoundaryDetectionMethod",
        "chr",
        "ChartBoundaryDetectionMethod",
        7,
    ),
    ("ChartColorPalette", "v8ui", "ChartColorPalette", 6),
    (
        "ChartColorPaletteDescription",
        "chr",
        "ChartColorPaletteDescription",
        7,
    ),
    ("ChartGridLinesShowMode", "chr", "ChartGridLinesShowMode", 7),
    ("ChartLabelArea", "chr", "ChartLabelArea", 7),
    ("ChartLabelLocation", "chr", "ChartLabelLocation", 7),
    (
        "ChartLabelsOrientation",
        "v8ui",
        "ChartLabelsOrientation",
        6,
    ),
    ("ChartLegendPlacement", "chr", "ChartLegendPlacement", 7),
    ("ChartMarkerType", "chr", "ChartMarkerType", 7),
    ("ChartPlotAreaPlacement", "chr", "ChartPlotAreaPlacement", 7),
    ("ChartReferenceBand", "chr", "ChartReferenceBand", 7),
    (
        "ChartReferenceBandBorderPosition",
        "chr",
        "ChartReferenceBandBorderPosition",
        7,
    ),
    ("ChartReferenceLine", "chr", "ChartReferenceLine", 7),
    (
        "ChartReferenceLinePosition",
        "chr",
        "ChartReferenceLinePosition",
        7,
    ),
    ("ChartScale", "chr", "ChartScale", 7),
    (
        "ChartScaleLabelLocation",
        "chr",
        "ChartScaleLabelLocation",
        7,
    ),
    ("ChartScaleLocation", "chr", "ChartScaleLocation", 7),
    ("ChartScaleMarkLocation", "chr", "ChartScaleMarkLocation", 7),
    (
        "ChartScaleTitlePlacement",
        "chr",
        "ChartScaleTitlePlacement",
        7,
    ),
    (
        "ChartScaleTitleTextSource",
        "chr",
        "ChartScaleTitleTextSource",
        7,
    ),
    (
        "ChartSeriesGraphicalRepresentationType",
        "chr",
        "ChartSeriesGraphicalRepresentationType",
        7,
    ),
    ("ChartSeriesStackType", "chr", "ChartSeriesStackType", 7),
    (
        "ChartTitleAreaPlacement",
        "chr",
        "ChartTitleAreaPlacement",
        7,
    ),
    (
        "ChartTrendLineApproximationType",
        "chr",
        "ChartTrendlineApproximationType",
        7,
    ),
    ("ChartTrendlineFactor", "chr", "ChartTrendlineFactor", 7),
    ("ChartType", "v8ui", "ChartType", 6),
    ("ChartValueEditState", "chr", "ChartValueEditState", 7),
    ("ChartValuesEditMode", "chr", "ChartValuesEditMode", 7),
    ("ClientConnectionSpeed", "ent", "ClientConnectionSpeed", 4),
    ("ClientRunMode", "app", "ClientRunMode", 13),
    (
        "ClusterizationMethod",
        "data-analysis",
        "ClusterizationMethod",
        8,
    ),
    ("Color", "v8ui", "Color", 6),
    ("CommandGroupCategory", "app", "CommandGroupCategory", 13),
    ("ComparisonType", "ent", "ComparisonType", 4),
    ("CompositeID", "v8", "CompositeID", 3),
    (
        "ConditionalAppearance",
        "entext",
        "ConditionalAppearance",
        15,
    ),
    (
        "ConditionalAppearanceItem",
        "entext",
        "ConditionalAppearanceItem",
        15,
    ),
    (
        "DataAnalysisAssociationRulesOrderType",
        "data-analysis",
        "DataAnalysisAssociationRulesOrderType",
        8,
    ),
    (
        "DataAnalysisColumnType",
        "data-analysis",
        "DataAnalysisColumnType",
        8,
    ),
    (
        "DataAnalysisColumnTypeAssociationRules",
        "data-analysis",
        "DataAnalysisColumnTypeAssociationRules",
        8,
    ),
    (
        "DataAnalysisColumnTypeClusterization",
        "data-analysis",
        "DataAnalysisColumnTypeClusterization",
        8,
    ),
    (
        "DataAnalysisColumnTypeDecisionTree",
        "data-analysis",
        "DataAnalysisColumnTypeDecisionTree",
        8,
    ),
    (
        "DataAnalysisColumnTypeSequentialPatterns",
        "data-analysis",
        "DataAnalysisColumnTypeSequentialPatterns",
        8,
    ),
    (
        "DataAnalysisColumnTypeSummaryStatistics",
        "data-analysis",
        "DataAnalysisColumnTypeSummaryStatistics",
        8,
    ),
    (
        "DataAnalysisDistanceMetricType",
        "data-analysis",
        "DataAnalysisDistanceMetricType",
        8,
    ),
    (
        "DataAnalysisNumericValueUseType",
        "data-analysis",
        "DataAnalysisNumericValueUseType",
        8,
    ),
    (
        "DataAnalysisResultTableFillType",
        "data-analysis",
        "DataAnalysisResultTableFillType",
        8,
    ),
    (
        "DataAnalysisSequentialPatternsOrderType",
        "data-analysis",
        "DataAnalysisSequentialPatternsOrderType",
        8,
    ),
    (
        "DataAnalysisStandardizationType",
        "data-analysis",
        "DataAnalysisStandardizationType",
        8,
    ),
    (
        "DataAnalysisTimeIntervalUnitType",
        "data-analysis",
        "DataAnalysisTimeIntervalUnitType",
        8,
    ),
    ("DataCompositionAppearance", "dcscor", "Appearance", 0),
    (
        "DataCompositionAppearanceFields",
        "dcsset",
        "AppearanceFields",
        2,
    ),
    (
        "DataCompositionAttributesPlacement",
        "dcsset",
        "DataCompositionAttributesPlacement",
        2,
    ),
    (
        "DataCompositionChartLegendPlacement",
        "dcsset",
        "DataCompositionChartLegendPlacement",
        2,
    ),
    (
        "DataCompositionComparisonType",
        "dcsset",
        "DataCompositionComparisonType",
        2,
    ),
    (
        "DataCompositionConditionalAppearance",
        "dcsset",
        "ConditionalAppearance",
        2,
    ),
    (
        "DataCompositionEditParameters",
        "dcscor",
        "InputParameters",
        0,
    ),
    ("DataCompositionField", "dcscor", "Field", 0),
    (
        "DataCompositionFieldPlacement",
        "dcsset",
        "DataCompositionFieldPlacement",
        2,
    ),
    (
        "DataCompositionFieldsTitleType",
        "dcscor",
        "DataCompositionFieldsTitleType",
        0,
    ),
    ("DataCompositionFilter", "dcsset", "Filter", 2),
    (
        "DataCompositionFilterApplicationType",
        "dcsset",
        "DataCompositionFilterApplicationType",
        2,
    ),
    (
        "DataCompositionFilterItemsGroupType",
        "dcsset",
        "DataCompositionFilterItemsGroupType",
        2,
    ),
    ("DataCompositionGroupFields", "dcsset", "GroupItems", 2),
    (
        "DataCompositionGroupFieldsPlacement",
        "dcsset",
        "DataCompositionGroupFieldsPlacement",
        2,
    ),
    (
        "DataCompositionGroupPlacement",
        "dcsset",
        "DataCompositionGroupPlacement",
        2,
    ),
    (
        "DataCompositionGroupTemplateType",
        "dcsset",
        "DataCompositionGroupTemplateType",
        2,
    ),
    (
        "DataCompositionGroupType",
        "dcscor",
        "DataCompositionGroupType",
        0,
    ),
    (
        "DataCompositionGroupUseVariant",
        "dcsset",
        "DataCompositionGroupUseVariant",
        2,
    ),
    ("DataCompositionOrder", "dcsset", "Order", 2),
    (
        "DataCompositionPeriodAdditionType",
        "dcscor",
        "DataCompositionPeriodAdditionType",
        0,
    ),
    (
        "DataCompositionPredefinedValue",
        "dcscor",
        "DesignTimeValue",
        0,
    ),
    (
        "DataCompositionResourcesAutoPosition",
        "dcsset",
        "DataCompositionResourcesAutoPosition",
        2,
    ),
    (
        "DataCompositionResourcesPlacement",
        "dcsset",
        "DataCompositionResourcesPlacement",
        2,
    ),
    (
        "DataCompositionResourcesPlacementInChart",
        "dcsset",
        "DataCompositionResourcesPlacementInChart",
        2,
    ),
    ("DataCompositionSelectedFields", "dcsset", "Selection", 2),
    ("DataCompositionSettingStructure", "dcsset", "Structure", 2),
    (
        "DataCompositionSettingsComposer",
        "dcsset",
        "SettingsComposer",
        2,
    ),
    (
        "DataCompositionSettingsItemState",
        "dcsset",
        "DataCompositionSettingsItemState",
        2,
    ),
    (
        "DataCompositionSettingsItemViewMode",
        "dcsset",
        "DataCompositionSettingsItemViewMode",
        2,
    ),
    (
        "DataCompositionSettingsViewMode",
        "dcsset",
        "DataCompositionSettingsViewMode",
        2,
    ),
    (
        "DataCompositionSortDirection",
        "dcscor",
        "DataCompositionSortDirection",
        0,
    ),
    (
        "DataCompositionTextOutputType",
        "dcsset",
        "DataCompositionTextOutputType",
        2,
    ),
    (
        "DataCompositionTextPlacementType",
        "dcscor",
        "DataCompositionTextPlacementType",
        0,
    ),
    (
        "DataCompositionTotalPlacement",
        "dcscor",
        "DataCompositionTotalPlacement",
        0,
    ),
    ("Date", "xs", "dateTime", 18),
    ("DateFractions", "v8", "DateFractions", 3),
    (
        "DecisionTreeSimplificationType",
        "data-analysis",
        "DecisionTreeSimplificationType",
        8,
    ),
    ("Dendrogram", "chr", "Dendrogram", 7),
    (
        "DetailsProcessDescription",
        "dcsdet",
        "DetailsProcessDescription",
        1,
    ),
    ("DocumentPostingMode", "ent", "DocumentPostingMode", 4),
    ("DocumentWriteMode", "ent", "DocumentWriteMode", 4),
    ("FillCheckErrorStatus", "v8", "FillCheckErrorStatus", 3),
    ("FillChecking", "v8", "FillChecking", 3),
    ("Filter", "misc", "Filter", 14),
    ("FilterItem", "ent", "FilterItem", 4),
    ("FixedArray", "v8", "FixedArray", 3),
    ("FixedMap", "v8", "FixedMap", 3),
    ("FixedStructure", "v8", "FixedStructure", 3),
    ("Font", "v8ui", "Font", 6),
    ("FormattedDocument", "fd", "FormattedDocument", 9),
    ("FormattedString", "v8ui", "FormattedString", 6),
    ("GanttChart", "chr", "GanttChart", 7),
    (
        "GanttChartIntervalTextRepresentation",
        "chr",
        "GanttChartIntervalTextRepresentation",
        7,
    ),
    (
        "GanttChartTextPlacementType",
        "chr",
        "GanttChartTextPlacementType",
        7,
    ),
    ("GeographicalSchema", "geo", "GeographicalSchema", 10),
    ("GraphicalSchema", "sch", "FlowchartContextType", 11),
    ("HorizontalAlign", "v8ui", "HorizontalAlign", 6),
    ("Line", "v8ui", "Line", 6),
    ("LinkedValueChangeMode", "ent", "LinkedValueChangeMode", 4),
    ("LocalString", "v8", "LocalStringType", 3),
    ("MessageStatus", "ent", "MessageStatus", 4),
    ("Null", "v8", "Null", 3),
    ("Number", "xs", "decimal", 18),
    ("Order", "misc", "Order", 14),
    ("PDFDocument", "pdfdoc", "PDFDocument", 16),
    ("Picture", "v8ui", "Picture", 6),
    ("PivotChartType", "v8ui", "PivotChartType", 6),
    ("Planner", "pl", "Planner", 17),
    (
        "PredictionModelColumnType",
        "data-analysis",
        "PredictionModelColumnType",
        8,
    ),
    (
        "ReportBuilderDimensionType",
        "entext",
        "ReportBuilderDimensionType",
        15,
    ),
    (
        "RequiredMobileApplicationPermissions",
        "app",
        "RequiredMobileApplicationPermissions",
        13,
    ),
    ("SearchDirection", "v8", "SearchDirection", 3),
    (
        "SectionPanelRepresentation",
        "app",
        "SectionPanelRepresentation",
        13,
    ),
    ("ShowChartScaleTitle", "chr", "ShowChartScaleTitle", 7),
    ("ShowInChart", "chr", "ShowInChart", 7),
    ("ShowInChartLegend", "chr", "ShowInChartLegend", 7),
    ("ShowInGanttChart", "chr", "ShowInGanttChart", 7),
    ("SizeChangeMode", "v8ui", "SizeChangeMode", 6),
    ("SliceUse", "ent", "SliceUse", 4),
    ("SortDirection", "ent", "SortDirection", 4),
    ("SpreadsheetDocument", "mxl", "SpreadsheetDocument", 12),
    ("StandardBeginningDate", "v8", "StandardBeginningDate", 3),
    (
        "StandardBeginningDateVariant",
        "v8",
        "StandardBeginningDateVariant",
        3,
    ),
    ("StandardPeriod", "v8", "StandardPeriod", 3),
    ("StandardPeriodVariant", "v8", "StandardPeriodVariant", 3),
    (
        "StockChartUsedPointValue",
        "chr",
        "StockChartUsedPointValue",
        7,
    ),
    ("String", "xs", "string", 18),
    ("StyleEntryKind", "app", "StyleEntryKind", 13),
    ("TextDocument", "txtedt", "TextDocument", 5),
    ("Type", "v8", "Type", 3),
    ("TypeDescription", "v8", "TypeDescription", 3),
    ("UUID", "v8", "UUID", 3),
    ("UsedChartValuesAxis", "chr", "UsedChartValuesAxis", 7),
    ("ValueList", "v8", "ValueListType", 3),
    ("ValueStorage", "v8", "ValueStorage", 3),
    ("ValueTable", "v8", "ValueTable", 3),
    ("ValueTree", "v8", "ValueTree", 3),
    ("VerticalAlign", "v8ui", "VerticalAlign", 6),
];

/// Unknown type names use the original SDK's current-configuration namespace.
/// This is a protocol table, not a whitelist of installed EPackage enums.
pub(super) fn native_type_qname(name: &str) -> (&str, &str, &str) {
    XDTO.iter()
        .find(|(ty, _, _, _)| *ty == name)
        .map(|(_, prefix, local, uri)| (*prefix, *local, NAMESPACES[*uri]))
        .unwrap_or(("cfg", name, CFG))
}
pub(super) fn validate_native_qname(local: &str, actual_uri: &str) -> bool {
    ncname(local)
        && ((local == "ChartLineType" && actual_uri == UI)
            || XDTO
                .iter()
                .any(|(_, _, name, uri)| *name == local && NAMESPACES[*uri] == actual_uri)
            || (actual_uri == CFG && native_type_qname(local).2 == CFG))
}
pub(super) fn is_edt_type(ty: &str) -> bool {
    matches!(
        ty,
        "core:EnumValue" | "core:SysEnumValue" | "common:ChartLineTypeValue"
    )
}
pub(super) fn is_native_type_name(s: &str) -> bool {
    ncname(s)
}
fn ncname(s: &str) -> bool {
    fn start(c: char) -> bool {
        matches!(c, '_' | 'A'..='Z' | 'a'..='z' | '\u{C0}'..='\u{D6}' | '\u{D8}'..='\u{F6}' | '\u{F8}'..='\u{2FF}' | '\u{370}'..='\u{37D}' | '\u{37F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}' | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}' | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}' | '\u{10000}'..='\u{EFFFF}')
    }
    let mut chars = s.chars();
    chars.next().is_some_and(start) && chars.all(|c| start(c) || matches!(c, '-' | '.' | '0'..='9' | '\u{B7}' | '\u{300}'..='\u{36F}' | '\u{203F}'..='\u{2040}'))
}
fn line(s: &str, path: &str) -> Result<(), FormError> {
    if LINE.contains(&s) {
        Ok(())
    } else {
        Err(frame(format!(
            "chart {path}: unknown ChartLineType literal"
        )))
    }
}
fn enum_identity(s: &str, path: &str) -> Result<ChartTypedValue, FormError> {
    let (package_uri, rest) = s
        .split_once('#')
        .ok_or_else(|| frame(format!("chart {path}: invalid EnumValue package identity")))?;
    let (enum_type, literal) = rest.split_once('/').ok_or_else(|| {
        frame(format!(
            "chart {path}: invalid EnumValue type/literal identity"
        ))
    })?;
    if package_uri.is_empty() || !ncname(enum_type) || literal.is_empty() {
        return Err(frame(format!("chart {path}: invalid EnumValue identity")));
    }
    Ok(ChartTypedValue::Enum {
        package_uri: package_uri.into(),
        enum_type: enum_type.into(),
        literal: literal.into(),
    })
}
fn edt_scalar(el: &Element, ty: &str, path: &str) -> Result<Option<String>, FormError> {
    el.claim();
    claim_xsi(el, ty, path)?;
    let (prefix, _) = ty.split_once(':').unwrap();
    if let Some(a) = el.attr(&format!("xmlns:{prefix}")) {
        let expected = if prefix == "common" {
            COMMON
        } else {
            CORE_NS_URI
        };
        if a.value != expected {
            return Err(frame(format!("chart {path}: wrong enum namespace URI")));
        }
        a.claimed.set(true);
    }
    expect_attrs_claimed(el, path)?;
    if !el.text.is_empty() {
        return Err(frame(format!("chart {path}: text outside enum value")));
    }
    match el.children.as_slice() {
        [] => Ok(None),
        [value] if value.prefix.is_empty() && value.local == "value" => {
            Ok(Some(leaf_text(value, path)?))
        }
        _ => Err(frame(format!(
            "chart {path}: unknown/duplicate enum value child"
        ))),
    }
}
/// Match the actual BaseValue reader's AccountType branch without guessing an EPackage.
pub(super) fn binding_value(type_name: &str, literal: &str) -> ChartTypedValue {
    if type_name == "AccountType" && matches!(literal, "Active" | "Passive" | "ActivePassive") {
        ChartTypedValue::Scalar(PropertyValue::Value(ValueSpec {
            kind: ValueScalarKind::AccountType,
            scalar: Some(Box::new(PropertyValue::Str(literal.into()))),
        }))
    } else {
        ChartTypedValue::SysEnum(Some(format!("{type_name}.{literal}")))
    }
}
/// The caller supplies the namespace resolved over the complete ancestor chain.
pub(super) fn read_native_resolved(
    el: &Element,
    path: &str,
    uri: &str,
) -> Result<ChartTypedValue, FormError> {
    let a = el
        .attr("xsi:type")
        .ok_or_else(|| frame(format!("chart {path}: enum value missing xsi:type")))?;
    let (prefix, local) = a.value.split_once(':').unwrap_or(("", a.value.as_str()));
    if (!prefix.is_empty() && !ncname(prefix)) || !validate_native_qname(local, uri) {
        return Err(frame(format!(
            "chart {path}: wrong native enum expanded QName"
        )));
    }
    a.claimed.set(true);
    let declaration = if prefix.is_empty() {
        "xmlns".into()
    } else {
        format!("xmlns:{prefix}")
    };
    if let Some(a) = el.attr(&declaration) {
        if a.value != uri {
            return Err(frame(format!(
                "chart {path}: conflicting enum namespace declaration"
            )));
        }
        a.claimed.set(true);
    }
    let text = leaf_text(el, path)?;
    if local == "ChartLineType" && uri == UI {
        line(&text, path)?;
        return Ok(ChartTypedValue::ChartLineType(text));
    }
    // A unique protocol inverse restores its type name, not an EPackage identity.
    // The SDK BaseValue reader also produces SysEnum rather than an Enum EObject.
    let ty = XDTO
        .iter()
        .find(|(_, _, name, namespace)| *name == local && NAMESPACES[*namespace] == uri)
        .map(|(ty, _, _, _)| *ty)
        .unwrap_or(local);
    if ty == "AccountType" && !matches!(text.as_str(), "Active" | "Passive" | "ActivePassive") {
        return Err(frame(format!("chart {path}: unknown AccountType literal")));
    }
    Ok(binding_value(ty, &text))
}
pub(super) fn read(el: &Element, path: &str, native: bool) -> Result<ChartTypedValue, FormError> {
    let ty = el
        .attr("xsi:type")
        .map(|a| a.value.as_str())
        .ok_or_else(|| frame(format!("chart {path}: enum value missing xsi:type")))?;
    if native {
        if let Some((uri, _)) = el.resolved_type.borrow().as_ref() {
            return read_native_resolved(el, path, uri);
        }
        let (prefix, local) = ty.split_once(':').unwrap_or(("", ty));
        let declaration = if prefix.is_empty() {
            "xmlns".into()
        } else {
            format!("xmlns:{prefix}")
        };
        let uri = el
            .attr(&declaration)
            .map(|a| a.value.as_str())
            .or_else(|| {
                let (known, _, uri) = native_type_qname(local);
                (prefix == known).then_some(uri)
            })
            .ok_or_else(|| {
                frame(format!(
                    "chart {path}: native enum alias requires resolved namespace"
                ))
            })?;
        return read_native_resolved(el, path, uri);
    }
    let value = edt_scalar(el, ty, path)?;
    match ty {
        "core:EnumValue" => enum_identity(
            value
                .as_deref()
                .ok_or_else(|| frame(format!("chart {path}: EnumValue requires nonnull value")))?,
            path,
        ),
        "core:SysEnumValue" => Ok(ChartTypedValue::SysEnum(value)),
        "common:ChartLineTypeValue" => {
            let value = value.unwrap_or_else(|| "None".into());
            line(&value, path)?;
            Ok(ChartTypedValue::ChartLineType(value))
        }
        _ => Err(frame(format!("chart {path}: unknown EDT enum value type"))),
    }
}
pub(super) fn write(
    name: &str,
    value: &ChartTypedValue,
    path: &str,
    native: bool,
) -> Result<OutElement, FormError> {
    let (edt_ty, text) = match value {
        ChartTypedValue::ProjectedEnum { .. } if native => ("", None),
        ChartTypedValue::Enum {
            package_uri,
            enum_type,
            literal,
        } => {
            let text = format!("{package_uri}#{enum_type}/{literal}");
            enum_identity(&text, path)?;
            ("core:EnumValue", Some(text))
        }
        ChartTypedValue::SysEnum(value) => ("core:SysEnumValue", value.clone()),
        ChartTypedValue::ChartLineType(value) => {
            line(value, path)?;
            ("common:ChartLineTypeValue", Some(value.clone()))
        }
        _ => return Err(frame(format!("chart {path}: expected current enum value"))),
    };
    if native {
        let (ty, literal) = match value {
            ChartTypedValue::Enum {
                enum_type, literal, ..
            }
            | ChartTypedValue::ProjectedEnum { enum_type, literal } => {
                (enum_type.as_str(), literal.as_str())
            }
            ChartTypedValue::ChartLineType(value) => ("ChartLineType", value.as_str()),
            ChartTypedValue::SysEnum(Some(value)) => {
                let mut parts = value.split('.');
                let ty = parts.next().unwrap_or("");
                let literal = parts.next().filter(|s| !s.is_empty()).ok_or_else(|| {
                    frame(format!(
                        "chart {path}: SysEnum omission requires semantic transport"
                    ))
                })?;
                (ty, literal)
            }
            _ => {
                return Err(frame(format!(
                    "chart {path}: null SysEnum requires semantic transport"
                )));
            }
        };
        if !ncname(ty) {
            return Err(frame(format!("chart {path}: invalid enum type QName")));
        }
        let (prefix, local, uri) = if matches!(value, ChartTypedValue::ChartLineType(_)) {
            ("v8ui", "ChartLineType", UI)
        } else {
            native_type_qname(ty)
        };
        let mut out = OutElement::leaf("d4p1", name, literal);
        if !super::super::DESIGNER_FORM_NS
            .iter()
            .any(|(key, value)| key.strip_prefix("xmlns:") == Some(prefix) && *value == uri)
        {
            out = out.attr(format!("xmlns:{prefix}"), uri);
        }
        Ok(out.attr("xsi:type", format!("{prefix}:{local}")))
    } else {
        let mut out = OutElement::branch("", name).attr("xsi:type", edt_ty);
        if edt_ty.starts_with("common:") {
            out = out.attr("xmlns:common", COMMON);
        }
        if let Some(text) = text {
            if edt_ty != "common:ChartLineTypeValue" || text != "None" {
                out.push(OutElement::leaf("", "value", text));
            }
        }
        out.self_closing = out.children.is_empty();
        Ok(out)
    }
}
