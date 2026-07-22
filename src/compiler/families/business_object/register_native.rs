//! Evidenced 8.3.27 native layouts for registers, recalculations, and charts.
//!
//! This first base-free cohort intentionally accepts root metadata, forms,
//! calculation-register recalculation references, and recalculation dimensions.
//! Unevidenced embedded register fields fail closed.

use super::*;

const INFORMATION_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "MainFilterOnPeriod",
    "IncludeHelpInContents",
    "EnableTotalsSliceFirst",
    "EnableTotalsSliceLast",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "EditType",
    "InformationRegisterPeriodicity",
    "WriteMode",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "DefaultRecordForm",
    "DefaultListForm",
    "AuxiliaryRecordForm",
    "AuxiliaryListForm",
    "RecordPresentation",
    "ExtendedRecordPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];
const ACCUMULATION_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "EnableTotalsSplitting",
    "RegisterType",
    "DataLockControlMode",
    "FullTextSearch",
    "DefaultListForm",
    "AuxiliaryListForm",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];
const ACCOUNTING_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "Correspondence",
    "EnableTotalsSplitting",
    "PeriodAdjustmentLength",
    "DataLockControlMode",
    "FullTextSearch",
    "ChartOfAccounts",
    "DefaultListForm",
    "AuxiliaryListForm",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];
const CALCULATION_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "ActionPeriod",
    "BasePeriod",
    "IncludeHelpInContents",
    "Periodicity",
    "DataLockControlMode",
    "FullTextSearch",
    "DefaultListForm",
    "AuxiliaryListForm",
    "Schedule",
    "ScheduleValue",
    "ScheduleDate",
    "ChartOfCalculationTypes",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
    "ChildRecalculations",
];
const RECALCULATION_SCHEMA: &[&str] = &["Name", "Synonym", "Comment", "DataLockControlMode"];
const RECALCULATION_DIMENSION_SCHEMA: &[&str] =
    &["Name", "Synonym", "Comment", "RegisterDimension"];
const CCT_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "Hierarchical",
    "FoldersOnTop",
    "CheckUnique",
    "Autonumbering",
    "QuickChoice",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "CodeLength",
    "DescriptionLength",
    "CodeAllowedLength",
    "CodeSeries",
    "DefaultPresentation",
    "PredefinedDataUpdate",
    "EditType",
    "ChoiceMode",
    "CreateOnInput",
    "SearchStringModeOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceHistoryOnInput",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "CharacteristicExtValues",
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
    "Types",
    "InputByString",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];
const COA_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "IncludeHelpInContents",
    "CheckUnique",
    "QuickChoice",
    "AutoOrderByCode",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "MaxExtDimensionCount",
    "CodeLength",
    "DescriptionLength",
    "OrderLength",
    "CodeSeries",
    "DefaultPresentation",
    "PredefinedDataUpdate",
    "EditType",
    "ChoiceMode",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "ExtDimensionTypes",
    "CodeMask",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "InputByString",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];
const COT_SCHEMA: &[&str] = &[
    "Name",
    "Synonym",
    "Comment",
    "UseStandardCommands",
    "QuickChoice",
    "ActionPeriodUse",
    "IncludeHelpInContents",
    "UpdateDataHistoryImmediatelyAfterWrite",
    "ExecuteAfterWriteDataHistoryVersionProcessing",
    "CodeLength",
    "DescriptionLength",
    "CodeType",
    "CodeAllowedLength",
    "DefaultPresentation",
    "EditType",
    "ChoiceMode",
    "SearchStringModeOnInputByString",
    "FullTextSearchOnInputByString",
    "ChoiceDataGetModeOnInputByString",
    "CreateOnInput",
    "ChoiceHistoryOnInput",
    "DependenceOnCalculationTypes",
    "PredefinedDataUpdate",
    "DataLockControlMode",
    "FullTextSearch",
    "DataHistory",
    "DefaultObjectForm",
    "DefaultListForm",
    "DefaultChoiceForm",
    "AuxiliaryObjectForm",
    "AuxiliaryListForm",
    "AuxiliaryChoiceForm",
    "InputByString",
    "BaseCalculationTypes",
    "ObjectPresentation",
    "ExtendedObjectPresentation",
    "ListPresentation",
    "ExtendedListPresentation",
    "Explanation",
    "ChildForms",
    "ChildTemplates",
];

pub(super) fn build_register_family(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
    family: BusinessObjectFamily,
) -> Result<NativeValue, BusinessObjectBuildError> {
    match family {
        BusinessObjectFamily::InformationRegister => build_information(validated, object, indexes),
        BusinessObjectFamily::AccumulationRegister => {
            build_accumulation(validated, object, indexes)
        }
        BusinessObjectFamily::AccountingRegister => build_accounting(validated, object, indexes),
        BusinessObjectFamily::CalculationRegister => build_calculation(validated, object, indexes),
        BusinessObjectFamily::Recalculation => build_recalculation(validated, object, indexes),
        BusinessObjectFamily::ChartOfCharacteristicTypes => build_cct(validated, object, indexes),
        BusinessObjectFamily::ChartOfAccounts => build_coa(validated, object, indexes),
        BusinessObjectFamily::ChartOfCalculationTypes => build_cot(validated, object, indexes),
        _ => native("register native builder received another family"),
    }
}

fn validate_top_level(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    schema: &[&str],
) -> Result<(), BusinessObjectBuildError> {
    validate_root_object(validated, object, schema)?;
    let uuid = object.identity().uuid();
    for child in validated.configuration().objects() {
        if child.owner() != Some(uuid) {
            continue;
        }
        if child.kind().as_str() != "Form" && child.kind().as_str() != "Template" {
            return invalid_model(
                uuid,
                "register/chart embedded child is not in the supported cohort",
            );
        }
    }
    Ok(())
}

fn owned_forms_and_templates(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<(Vec<ObjectUuid>, Vec<ObjectUuid>), BusinessObjectBuildError> {
    let forms = reference_sequence_targets(object, "ChildForms", indexes)?;
    let templates = reference_sequence_targets(object, "ChildTemplates", indexes)?;
    for (values, kind) in [(&forms, "Form"), (&templates, "Template")] {
        for value in values {
            if indexes.kind(*value) != Some(kind)
                || indexes.owner(*value) != Some(Some(object.identity().uuid()))
            {
                return invalid_model(
                    object.identity().uuid(),
                    "chart form/template is not an owned child",
                );
            }
        }
    }
    Ok((forms, templates))
}

fn require_empty_templates(
    object: &CanonicalObject,
    templates: &[ObjectUuid],
) -> Result<(), BusinessObjectBuildError> {
    if templates.is_empty() {
        Ok(())
    } else {
        invalid_model(
            object.identity().uuid(),
            "register/chart template collection is unsupported",
        )
    }
}

fn generated_or_derived(
    object: &CanonicalObject,
    family: BusinessObjectFamily,
    expected: &[&str],
) -> Result<Vec<(ObjectUuid, ObjectUuid)>, BusinessObjectBuildError> {
    if !object.generated_types().is_empty() {
        return generated_pairs(object, expected);
    }
    if !matches!(
        family,
        BusinessObjectFamily::ChartOfCharacteristicTypes
            | BusinessObjectFamily::ChartOfAccounts
            | BusinessObjectFamily::ChartOfCalculationTypes
    ) {
        return invalid_model(
            object.identity().uuid(),
            "register generated type inventory is absent",
        );
    }
    let owner = object.identity().uuid();
    Ok(expected
        .iter()
        .flat_map(|kind| ["type", "value"].into_iter().map(move |role| (*kind, role)))
        .map(|(kind, role)| {
            let generation = derive_generation_uuid_v8(
                b"metadata-generated-type",
                &[
                    owner.as_bytes(),
                    family.as_str().as_bytes(),
                    kind.as_bytes(),
                    role.as_bytes(),
                ],
            );
            ObjectUuid::parse(&generation.to_string())
                .expect("derived generation UUID text is canonical")
        })
        .collect::<Vec<_>>()
        .chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect())
}

fn form_slot(
    object: &CanonicalObject,
    name: &str,
    forms: &[ObjectUuid],
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    optional_owned_reference(object, name, forms, indexes)
}

fn build_information(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_top_level(validated, object, INFORMATION_SCHEMA)?;
    let (forms, templates) = owned_forms_and_templates(object, indexes)?;
    require_empty_templates(object, &templates)?;
    let generated = generated_pairs(
        object,
        &[
            "Record",
            "Manager",
            "Selection",
            "List",
            "RecordSet",
            "RecordKey",
            "RecordManager",
        ],
    )?;
    let mut fields = vec![token("0"); 39];
    fields[0] = token("33");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 9, 11, 13], &generated);
    fields[15] = list(vec![token("0"), native_header(object)?]);
    fields[16] = form_slot(object, "DefaultRecordForm", &forms, indexes)?;
    fields[17] = form_slot(object, "DefaultListForm", &forms, indexes)?;
    fields[18] = enum_code(
        object,
        "InformationRegisterPeriodicity",
        &[
            ("Nonperiodical", "0"),
            ("Year", "1"),
            ("Quarter", "2"),
            ("Month", "3"),
            ("Day", "4"),
            ("Second", "5"),
            ("RecorderPosition", "6"),
        ],
    )?;
    fields[19] = enum_code(
        object,
        "WriteMode",
        &[("Independent", "0"), ("RecorderSubordinate", "1")],
    )?;
    fields[20] = enum_code(
        object,
        "EditType",
        &[("InList", "0"), ("InDialog", "1"), ("BothWays", "2")],
    )?;
    fields[21] = bool_token(object, "UseStandardCommands")?;
    fields[22] = bool_token(object, "IncludeHelpInContents")?;
    fields[23] = bool_token(object, "MainFilterOnPeriod")?;
    fields[24] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[25] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[26] = standard_attributes(&["-5", "-4", "-3", "-2"])?;
    fields[27] = form_slot(object, "AuxiliaryRecordForm", &forms, indexes)?;
    fields[28] = form_slot(object, "AuxiliaryListForm", &forms, indexes)?;
    for (slot, name) in (29..=33).zip([
        "RecordPresentation",
        "ExtendedRecordPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[34] = bool_token(object, "EnableTotalsSliceLast")?;
    fields[35] = bool_token(object, "EnableTotalsSliceFirst")?;
    fields[36] = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    fields[37] = bool_token(object, "UpdateDataHistoryImmediatelyAfterWrite")?;
    fields[38] = bool_token(object, "ExecuteAfterWriteDataHistoryVersionProcessing")?;
    Ok(list(vec![
        token("1"),
        list(fields),
        token("6"),
        native_collection(INFORMATION_REGISTER_COLLECTION_UUIDS[0], Vec::new()),
        native_collection(INFORMATION_REGISTER_COLLECTION_UUIDS[1], Vec::new()),
        native_collection(INFORMATION_REGISTER_COLLECTION_UUIDS[2], Vec::new()),
        native_collection(INFORMATION_REGISTER_COLLECTION_UUIDS[3], Vec::new()),
        native_collection(INFORMATION_REGISTER_COLLECTION_UUIDS[4], Vec::new()),
        native_collection(
            INFORMATION_REGISTER_COLLECTION_UUIDS[5],
            forms.into_iter().map(uuid_value).collect(),
        ),
    ]))
}

fn build_accumulation(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_top_level(validated, object, ACCUMULATION_SCHEMA)?;
    require_bool_value(object, "IncludeHelpInContents", false)?;
    let (forms, templates) = owned_forms_and_templates(object, indexes)?;
    require_empty_templates(object, &templates)?;
    let generated = generated_pairs(
        object,
        &[
            "Record",
            "Manager",
            "Selection",
            "List",
            "RecordSet",
            "RecordKey",
        ],
    )?;
    let register_type = enum_property(object, "RegisterType")?;
    let markers: &[&str] = match register_type {
        "Balance" => &["-9", "-5", "-4", "-3", "-2"],
        "Turnovers" => &["-5", "-4", "-3", "-2"],
        _ => {
            return invalid_model(
                object.identity().uuid(),
                "AccumulationRegister type is unsupported",
            );
        }
    };
    let mut fields = vec![token("0"); 26];
    fields[0] = token("28");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 9, 11], &generated);
    fields[13] = list(vec![token("0"), native_header(object)?]);
    fields[14] = form_slot(object, "DefaultListForm", &forms, indexes)?;
    fields[15] = token(if register_type == "Balance" { "0" } else { "1" });
    fields[16] = bool_token(object, "UseStandardCommands")?;
    fields[17] = token("0");
    fields[18] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[19] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[20] = bool_token(object, "EnableTotalsSplitting")?;
    fields[21] = standard_attributes(markers)?;
    fields[22] = form_slot(object, "AuxiliaryListForm", &forms, indexes)?;
    for (slot, name) in (23..=25).zip([
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    Ok(list(vec![
        token("1"),
        list(fields),
        token("6"),
        native_collection(ACCUMULATION_REGISTER_COLLECTION_UUIDS[0], Vec::new()),
        native_collection(ACCUMULATION_REGISTER_COLLECTION_UUIDS[1], Vec::new()),
        native_collection(ACCUMULATION_REGISTER_COLLECTION_UUIDS[2], Vec::new()),
        native_collection(ACCUMULATION_REGISTER_COLLECTION_UUIDS[3], Vec::new()),
        native_collection(ACCUMULATION_REGISTER_COLLECTION_UUIDS[4], Vec::new()),
        native_collection(
            ACCUMULATION_REGISTER_COLLECTION_UUIDS[5],
            forms.into_iter().map(uuid_value).collect(),
        ),
    ]))
}

fn build_accounting(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_top_level(validated, object, ACCOUNTING_SCHEMA)?;
    let (forms, templates) = owned_forms_and_templates(object, indexes)?;
    require_empty_templates(object, &templates)?;
    let generated = generated_pairs(
        object,
        &[
            "Record",
            "ExtDimensions",
            "RecordSet",
            "RecordKey",
            "Selection",
            "List",
            "Manager",
        ],
    )?;
    let chart =
        optional_reference_uuid_kind(object, "ChartOfAccounts", "ChartOfAccounts", indexes)?
            .ok_or(BusinessObjectBuildError::InvalidModel {
                object: object.identity().uuid(),
                reason: "AccountingRegister ChartOfAccounts is empty",
            })?;
    let mut fields = vec![token("0"); 30];
    fields[0] = token("21");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 9, 11, 13], &generated);
    fields[15] = list(vec![token("0"), native_header(object)?]);
    fields[16] = bool_token(object, "UseStandardCommands")?;
    fields[17] = bool_token(object, "IncludeHelpInContents")?;
    fields[18] = uuid_value(chart);
    fields[19] = form_slot(object, "DefaultListForm", &forms, indexes)?;
    fields[20] = bool_token(object, "Correspondence")?;
    fields[21] = token(u32_property(object, "PeriodAdjustmentLength")?.to_string());
    fields[22] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[23] = bool_token(object, "EnableTotalsSplitting")?;
    fields[24] = standard_attributes(&["-10", "-5", "-4", "-3", "-2"])?;
    fields[25] = form_slot(object, "AuxiliaryListForm", &forms, indexes)?;
    for (slot, name) in (26..=28).zip([
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[29] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    Ok(list(vec![
        token("1"),
        list(fields),
        token("6"),
        native_collection(ACCOUNTING_REGISTER_COLLECTION_UUIDS[0], Vec::new()),
        native_collection(ACCOUNTING_REGISTER_COLLECTION_UUIDS[1], Vec::new()),
        native_collection(ACCOUNTING_REGISTER_COLLECTION_UUIDS[2], Vec::new()),
        native_collection(ACCOUNTING_REGISTER_COLLECTION_UUIDS[3], Vec::new()),
        native_collection(ACCOUNTING_REGISTER_COLLECTION_UUIDS[4], Vec::new()),
        native_collection(
            ACCOUNTING_REGISTER_COLLECTION_UUIDS[5],
            forms.into_iter().map(uuid_value).collect(),
        ),
    ]))
}

fn schedule_slots(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<[NativeValue; 4], BusinessObjectBuildError> {
    let names = [
        "Schedule",
        "ScheduleValue",
        "ScheduleDate",
        "ChartOfCalculationTypes",
    ];
    let values = names.map(|name| text_property(object, name).map(str::to_owned));
    let values = values.into_iter().collect::<Result<Vec<_>, _>>()?;
    if values.iter().all(String::is_empty) {
        return Ok(std::array::from_fn(|_| token(NIL_UUID)));
    }
    if values.iter().any(String::is_empty) {
        return invalid_model(
            object.identity().uuid(),
            "CalculationRegister schedule tuple is incomplete",
        );
    }
    let schedule = indexes.object(object.identity().uuid(), &values[0])?;
    let resource = indexes.object(object.identity().uuid(), &values[1])?;
    let dimension = indexes.object(object.identity().uuid(), &values[2])?;
    let chart = indexes.object(object.identity().uuid(), &values[3])?;
    if indexes.kind(schedule) != Some("InformationRegister")
        || indexes.kind(resource) != Some("Resource")
        || indexes.owner(resource) != Some(Some(schedule))
        || indexes.kind(dimension) != Some("Dimension")
        || indexes.owner(dimension) != Some(Some(schedule))
        || indexes.kind(chart) != Some("ChartOfCalculationTypes")
    {
        return invalid_model(
            object.identity().uuid(),
            "CalculationRegister schedule ownership is invalid",
        );
    }
    Ok([
        uuid_value(schedule),
        uuid_value(resource),
        uuid_value(dimension),
        uuid_value(chart),
    ])
}

fn child_recalculations(
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<Vec<ObjectUuid>, BusinessObjectBuildError> {
    let values = reference_sequence_targets(object, "ChildRecalculations", indexes)?;
    for value in &values {
        if indexes.kind(*value) != Some("Recalculation")
            || indexes.owner(*value) != Some(Some(object.identity().uuid()))
        {
            return invalid_model(
                object.identity().uuid(),
                "Recalculation is not owned by CalculationRegister",
            );
        }
    }
    Ok(values)
}

fn build_calculation(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_root_object(validated, object, CALCULATION_SCHEMA)?;
    for child in validated.configuration().objects() {
        if child.owner() == Some(object.identity().uuid())
            && !matches!(child.kind().as_str(), "Form" | "Template" | "Recalculation")
        {
            return invalid_model(
                object.identity().uuid(),
                "CalculationRegister embedded child is unsupported",
            );
        }
    }
    let (forms, templates) = owned_forms_and_templates(object, indexes)?;
    require_empty_templates(object, &templates)?;
    let recalculations = child_recalculations(object, indexes)?;
    let generated = generated_pairs(
        object,
        &[
            "Record",
            "Manager",
            "Selection",
            "List",
            "RecordSet",
            "RecordKey",
            "Recalcs",
        ],
    )?;
    let schedule = schedule_slots(object, indexes)?;
    let mut fields = vec![token("0"); 33];
    fields[0] = token("21");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 9, 11, 13], &generated);
    fields[15] = list(vec![token("0"), native_header(object)?]);
    fields[16] = enum_code(object, "Periodicity", &[("Month", "2")])?;
    fields[17] = bool_token(object, "ActionPeriod")?;
    fields[18] = bool_token(object, "BasePeriod")?;
    for (slot, value) in (19..=22).zip(schedule) {
        fields[slot] = value;
    }
    fields[23] = form_slot(object, "DefaultListForm", &forms, indexes)?;
    fields[24] = bool_token(object, "UseStandardCommands")?;
    fields[25] = bool_token(object, "IncludeHelpInContents")?;
    fields[26] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[27] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[28] = standard_attributes(&[
        "-13", "-11", "-10", "-9", "-8", "-7", "-6", "-5", "-4", "-3", "-2",
    ])?;
    fields[29] = form_slot(object, "AuxiliaryListForm", &forms, indexes)?;
    for (slot, name) in (30..=32).zip([
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    Ok(list(vec![
        token("1"),
        list(fields),
        token("7"),
        native_collection(CALCULATION_REGISTER_COLLECTION_UUIDS[0], Vec::new()),
        native_collection(
            CALCULATION_REGISTER_COLLECTION_UUIDS[1],
            recalculations.into_iter().map(uuid_value).collect(),
        ),
        native_collection(CALCULATION_REGISTER_COLLECTION_UUIDS[2], Vec::new()),
        native_collection(CALCULATION_REGISTER_COLLECTION_UUIDS[3], Vec::new()),
        native_collection(
            CALCULATION_REGISTER_COLLECTION_UUIDS[4],
            forms.into_iter().map(uuid_value).collect(),
        ),
        native_collection(CALCULATION_REGISTER_COLLECTION_UUIDS[5], Vec::new()),
        native_collection(CALCULATION_REGISTER_COLLECTION_UUIDS[6], Vec::new()),
    ]))
}

fn build_recalculation(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    let uuid = object.identity().uuid();
    require_property_schema(object, RECALCULATION_SCHEMA)?;
    let owner = object
        .owner()
        .ok_or(BusinessObjectBuildError::InvalidModel {
            object: uuid,
            reason: "Recalculation has no CalculationRegister owner",
        })?;
    if indexes.kind(owner) != Some("CalculationRegister")
        || validated.graph().object_index_by_uuid(uuid).is_none()
        || !object.references().is_empty()
        || !object.assets().is_empty()
    {
        return invalid_model(
            uuid,
            "Recalculation ownership or graph membership is invalid",
        );
    }
    let generated = generated_pairs(object, &["Record", "Manager", "RecordSet"])?;
    let mut dimensions = Vec::new();
    for child in validated.configuration().objects() {
        if child.owner() != Some(uuid) {
            continue;
        }
        if child.kind().as_str() != "Dimension" {
            return invalid_model(uuid, "Recalculation contains a non-Dimension child");
        }
        validate_embedded_object(child, "Dimension")?;
        require_property_schema(child, RECALCULATION_DIMENSION_SCHEMA)?;
        let target = indexes.object(
            child.identity().uuid(),
            text_property(child, "RegisterDimension")?,
        )?;
        if indexes.kind(target) != Some("Dimension") || indexes.owner(target) != Some(Some(owner)) {
            return invalid_model(
                child.identity().uuid(),
                "Recalculation dimension target is not owned by its register",
            );
        }
        dimensions.push(list(vec![
            list(vec![
                token("1"),
                native_header(child)?,
                uuid_value(target),
                list(vec![
                    token("0"),
                    token("1"),
                    list(vec![
                        text("#"),
                        token(METADATA_OBJECT_REF_TYPE_UUID),
                        list(vec![token("1"), uuid_value(target)]),
                    ]),
                ]),
            ]),
            token("0"),
        ]));
    }
    let mut fields = vec![token("0"); 9];
    fields[0] = token("4");
    put_generated_pairs(&mut fields, &[1, 3, 5], &generated);
    fields[7] = list(vec![token("0"), native_header(object)?]);
    fields[8] = token("1");
    Ok(list(vec![
        token("1"),
        list(fields),
        enum_code(
            object,
            "DataLockControlMode",
            &[("Automatic", "0"), ("Managed", "1")],
        )?,
        native_collection(RECALCULATION_DIMENSION_COLLECTION_UUID, dimensions),
    ]))
}

fn build_cct(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_top_level(validated, object, CCT_SCHEMA)?;
    let (forms, templates) = owned_forms_and_templates(object, indexes)?;
    require_empty_templates(object, &templates)?;
    let generated = generated_or_derived(
        object,
        BusinessObjectFamily::ChartOfCharacteristicTypes,
        &[
            "Object",
            "Ref",
            "Selection",
            "List",
            "Characteristic",
            "Manager",
        ],
    )?;
    let mut fields = vec![token("0"); 59];
    fields[0] = token("34");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 9, 11], &generated);
    fields[13] = list(vec![token("0"), native_header(object)?]);
    fields[14] = bool_token(object, "UseStandardCommands")?;
    fields[15] = list(vec![token("0"), token("0")]);
    fields[16] = bool_token(object, "IncludeHelpInContents")?;
    fields[17] =
        optional_metadata_reference_kind(object, "CharacteristicExtValues", "Catalog", indexes)?;
    fields[18] = type_pattern(object, indexes)?;
    fields[19] = bool_token(object, "Hierarchical")?;
    fields[20] = bool_token(object, "FoldersOnTop")?;
    fields[21] = token(u32_property(object, "CodeLength")?.to_string());
    fields[22] = bool_token(object, "Autonumbering")?;
    fields[23] = token(u32_property(object, "DescriptionLength")?.to_string());
    fields[24] = enum_code(object, "CodeSeries", &[("WholeCharacteristicKind", "1")])?;
    fields[25] = enum_code(
        object,
        "DefaultPresentation",
        &[("AsCode", "0"), ("AsDescription", "1")],
    )?;
    for (slot, name) in (26..=30).zip([
        "DefaultObjectForm",
        "DefaultFolderForm",
        "DefaultListForm",
        "DefaultChoiceForm",
        "DefaultFolderChoiceForm",
    ]) {
        fields[slot] = form_slot(object, name, &forms, indexes)?;
    }
    fields[31] = enum_code(object, "EditType", &[("InList", "0"), ("InDialog", "2")])?;
    fields[32] = bool_token(object, "QuickChoice")?;
    fields[33] = field_reference_collection(
        object,
        "InputByString",
        BusinessObjectFamily::ChartOfCharacteristicTypes,
        indexes,
    )?;
    fields[34] = bool_token(object, "CheckUnique")?;
    fields[35] = enum_code(object, "CreateOnInput", &[("DontUse", "0")])?;
    fields[36] = enum_code(
        object,
        "ChoiceMode",
        &[("FromForm", "0"), ("QuickChoice", "1"), ("BothWays", "2")],
    )?;
    fields[37] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[38] = standard_attributes(&["-14", "-11", "-9", "-8", "-7", "-6", "-5", "-4", "-2"])?;
    for (slot, name) in (39..=43).zip([
        "AuxiliaryObjectForm",
        "AuxiliaryFolderForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
        "AuxiliaryFolderChoiceForm",
    ]) {
        fields[slot] = form_slot(object, name, &forms, indexes)?;
    }
    for (slot, name) in (44..=48).zip([
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[49] = enum_code(
        object,
        "CodeAllowedLength",
        &[("Fixed", "0"), ("Variable", "1")],
    )?;
    fields[50] = list(vec![token("0"), list(vec![token("0")])]);
    fields[51] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[52] = list(vec![token("1"), list(vec![token("0"), token("0")])]);
    fields[53] = enum_code(
        object,
        "PredefinedDataUpdate",
        &[("Auto", "0"), ("DontAutoUpdate", "2")],
    )?;
    fields[54] = input_modes(object)?;
    fields[55] = enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?;
    fields[56] = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    fields[57] = bool_token(object, "UpdateDataHistoryImmediatelyAfterWrite")?;
    fields[58] = bool_token(object, "ExecuteAfterWriteDataHistoryVersionProcessing")?;
    Ok(chart_root(
        fields,
        &CHART_OF_CHARACTERISTIC_TYPES_COLLECTION_UUIDS,
        forms,
        5,
    ))
}

fn standard_tabular(
    marker: &str,
    attributes: &[&str],
) -> Result<NativeValue, BusinessObjectBuildError> {
    Ok(list(vec![
        token("1"),
        list(vec![
            token("0"),
            token("1"),
            token(marker),
            list(vec![
                token("3"),
                list(vec![token("0")]),
                text(""),
                token("0"),
                token("0"),
                standard_attributes(attributes)?,
                list(vec![token("0")]),
            ]),
        ]),
    ]))
}

fn standard_tabular_many(
    definitions: &[(&str, &[&str])],
) -> Result<NativeValue, BusinessObjectBuildError> {
    let mut body = vec![token("0"), token(definitions.len().to_string())];
    for (marker, attributes) in definitions {
        body.push(token(*marker));
        body.push(list(vec![
            token("3"),
            list(vec![token("0")]),
            text(""),
            token("0"),
            token("0"),
            standard_attributes(attributes)?,
            list(vec![token("0")]),
        ]));
    }
    Ok(list(vec![token("1"), list(body)]))
}

fn build_coa(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_top_level(validated, object, COA_SCHEMA)?;
    let (forms, templates) = owned_forms_and_templates(object, indexes)?;
    require_empty_templates(object, &templates)?;
    let generated = generated_or_derived(
        object,
        BusinessObjectFamily::ChartOfAccounts,
        &[
            "Object",
            "Ref",
            "Selection",
            "List",
            "Manager",
            "ExtDimensionTypes",
            "ExtDimensionTypesRow",
        ],
    )?;
    let ext_types = optional_reference_uuid_kind(
        object,
        "ExtDimensionTypes",
        "ChartOfCharacteristicTypes",
        indexes,
    )?
    .ok_or(BusinessObjectBuildError::InvalidModel {
        object: object.identity().uuid(),
        reason: "ChartOfAccounts ExtDimensionTypes is empty",
    })?;
    let mut fields = vec![token("0"); 57];
    fields[0] = token("32");
    put_generated_pairs(&mut fields, &[1, 3, 5, 7, 9, 11, 13], &generated);
    fields[15] = list(vec![token("0"), native_header(object)?]);
    fields[16] = bool_token(object, "UseStandardCommands")?;
    fields[17] = bool_token(object, "IncludeHelpInContents")?;
    fields[18] = list(vec![token("0"), token("0")]);
    fields[19] = uuid_value(ext_types);
    fields[20] = token(u32_property(object, "MaxExtDimensionCount")?.to_string());
    fields[21] = text(text_property(object, "CodeMask")?);
    fields[22] = token(u32_property(object, "CodeLength")?.to_string());
    fields[23] = token(u32_property(object, "DescriptionLength")?.to_string());
    fields[24] = enum_code(object, "CodeSeries", &[("WithinSubordination", "1")])?;
    fields[25] = token(u32_property(object, "OrderLength")?.to_string());
    fields[26] = enum_code(
        object,
        "DefaultPresentation",
        &[("AsCode", "0"), ("AsDescription", "1")],
    )?;
    fields[27] = bool_token(object, "CheckUnique")?;
    for (slot, name) in (28..=30).zip(["DefaultObjectForm", "DefaultListForm", "DefaultChoiceForm"])
    {
        fields[slot] = form_slot(object, name, &forms, indexes)?;
    }
    fields[31] = enum_code(object, "EditType", &[("InList", "0"), ("InDialog", "2")])?;
    fields[32] = bool_token(object, "QuickChoice")?;
    fields[33] = field_reference_collection(
        object,
        "InputByString",
        BusinessObjectFamily::ChartOfAccounts,
        indexes,
    )?;
    fields[34] = bool_token(object, "AutoOrderByCode")?;
    fields[35] = enum_code(object, "CreateOnInput", &[("DontUse", "1")])?;
    fields[36] = enum_code(
        object,
        "ChoiceMode",
        &[("FromForm", "0"), ("QuickChoice", "1"), ("BothWays", "2")],
    )?;
    fields[37] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[38] = standard_attributes(&[
        "-28", "-17", "-11", "-10", "-8", "-7", "-6", "-5", "-4", "-2",
    ])?;
    fields[39] = standard_tabular("-12", &["-15", "-14", "-13", "-12"])?;
    for (slot, name) in (40..=42).zip([
        "AuxiliaryObjectForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
    ]) {
        fields[slot] = form_slot(object, name, &forms, indexes)?;
    }
    for (slot, name) in (43..=47).zip([
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[48] = list(vec![token("0"), list(vec![token("0")])]);
    fields[49] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[50] = list(vec![token("1"), list(vec![token("0"), token("0")])]);
    fields[51] = enum_code(
        object,
        "PredefinedDataUpdate",
        &[("Auto", "0"), ("DontAutoUpdate", "2")],
    )?;
    fields[52] = input_modes(object)?;
    fields[53] = enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?;
    fields[54] = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    fields[55] = bool_token(object, "UpdateDataHistoryImmediatelyAfterWrite")?;
    fields[56] = bool_token(object, "ExecuteAfterWriteDataHistoryVersionProcessing")?;
    Ok(chart_root(
        fields,
        &CHART_OF_ACCOUNTS_COLLECTION_UUIDS,
        forms,
        7,
    ))
}

fn one_reference_target(
    object: &CanonicalObject,
    name: &str,
    expected_kind: &str,
    indexes: &ReferenceIndexes,
) -> Result<ObjectUuid, BusinessObjectBuildError> {
    let values = reference_sequence_targets(object, name, indexes)?;
    let [value] = values.as_slice() else {
        return invalid_model(
            object.identity().uuid(),
            "reference collection must contain exactly one item",
        );
    };
    if indexes.kind(*value) != Some(expected_kind) {
        return invalid_model(
            object.identity().uuid(),
            "reference collection resolves to the wrong kind",
        );
    }
    Ok(*value)
}

fn build_cot(
    validated: &ValidatedConfiguration<'_>,
    object: &CanonicalObject,
    indexes: &ReferenceIndexes,
) -> Result<NativeValue, BusinessObjectBuildError> {
    validate_top_level(validated, object, COT_SCHEMA)?;
    let (forms, templates) = owned_forms_and_templates(object, indexes)?;
    require_empty_templates(object, &templates)?;
    let generated = generated_or_derived(
        object,
        BusinessObjectFamily::ChartOfCalculationTypes,
        &[
            "Object",
            "Ref",
            "Selection",
            "List",
            "Manager",
            "DisplacingCalculationTypes",
            "DisplacingCalculationTypesRow",
            "BaseCalculationTypes",
            "BaseCalculationTypesRow",
            "LeadingCalculationTypes",
            "LeadingCalculationTypesRow",
        ],
    )?;
    let base = one_reference_target(
        object,
        "BaseCalculationTypes",
        "ChartOfCalculationTypes",
        indexes,
    )?;
    let mut fields = vec![token("0"); 63];
    fields[0] = token("35");
    fields[1] = list(vec![token("0"), native_header(object)?]);
    put_generated_pairs(
        &mut fields,
        &[2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22],
        &generated,
    );
    fields[24] = bool_token(object, "UseStandardCommands")?;
    fields[25] = token(u32_property(object, "CodeLength")?.to_string());
    fields[26] = enum_code(object, "CodeType", &[("Number", "0"), ("String", "1")])?;
    fields[27] = enum_code(
        object,
        "CodeAllowedLength",
        &[("Fixed", "0"), ("Variable", "1")],
    )?;
    fields[28] = list(vec![
        token("0"),
        token("1"),
        list(vec![
            text("#"),
            token(METADATA_OBJECT_REF_TYPE_UUID),
            list(vec![token("1"), uuid_value(base)]),
        ]),
    ]);
    fields[29] = bool_token(object, "ActionPeriodUse")?;
    fields[30] = token(u32_property(object, "DescriptionLength")?.to_string());
    fields[31] = enum_code(
        object,
        "DefaultPresentation",
        &[("AsCode", "0"), ("AsDescription", "1")],
    )?;
    for (slot, name) in (32..=34).zip(["DefaultObjectForm", "DefaultListForm", "DefaultChoiceForm"])
    {
        fields[slot] = form_slot(object, name, &forms, indexes)?;
    }
    fields[35] = enum_code(
        object,
        "DependenceOnCalculationTypes",
        &[
            ("DontUse", "0"),
            ("OnActionPeriod", "1"),
            ("OnBasePeriod", "2"),
        ],
    )?;
    fields[36] = list(vec![token("0"), token("0")]);
    fields[37] = bool_token(object, "QuickChoice")?;
    fields[38] = enum_code(object, "EditType", &[("InList", "0"), ("InDialog", "2")])?;
    fields[39] = enum_code(object, "CreateOnInput", &[("DontUse", "0")])?;
    fields[40] = field_reference_collection(
        object,
        "InputByString",
        BusinessObjectFamily::ChartOfCalculationTypes,
        indexes,
    )?;
    fields[41] = enum_code(
        object,
        "ChoiceMode",
        &[("FromForm", "0"), ("QuickChoice", "1"), ("BothWays", "2")],
    )?;
    fields[42] = enum_code(object, "FullTextSearch", &[("DontUse", "0"), ("Use", "1")])?;
    fields[43] = standard_attributes(&["-11", "-8", "-6", "-5", "-4", "-3", "-2"])?;
    fields[44] = standard_tabular_many(&[
        ("-30", &["-102", "-101", "-100"]),
        ("-20", &["-102", "-101", "-100"]),
        ("-10", &["-102", "-101", "-100"]),
    ])?;
    for (slot, name) in (45..=47).zip([
        "AuxiliaryObjectForm",
        "AuxiliaryListForm",
        "AuxiliaryChoiceForm",
    ]) {
        fields[slot] = form_slot(object, name, &forms, indexes)?;
    }
    for (slot, name) in (48..=52).zip([
        "ObjectPresentation",
        "ExtendedObjectPresentation",
        "ListPresentation",
        "ExtendedListPresentation",
        "Explanation",
    ]) {
        fields[slot] = localized_value(object, name, "language")?;
    }
    fields[53] = token(if bool_property(object, "IncludeHelpInContents")? {
        "0"
    } else {
        "1"
    });
    fields[54] = list(vec![token("0"), list(vec![token("0")])]);
    fields[55] = enum_code(
        object,
        "DataLockControlMode",
        &[("Automatic", "0"), ("Managed", "1")],
    )?;
    fields[56] = list(vec![token("1"), list(vec![token("0"), token("0")])]);
    fields[57] = enum_code(
        object,
        "PredefinedDataUpdate",
        &[("Auto", "0"), ("DontAutoUpdate", "2")],
    )?;
    fields[58] = input_modes(object)?;
    fields[59] = enum_code(
        object,
        "ChoiceHistoryOnInput",
        &[("Auto", "0"), ("DontUse", "1")],
    )?;
    fields[60] = enum_code(object, "DataHistory", &[("DontUse", "0"), ("Use", "1")])?;
    fields[61] = bool_token(object, "UpdateDataHistoryImmediatelyAfterWrite")?;
    fields[62] = bool_token(object, "ExecuteAfterWriteDataHistoryVersionProcessing")?;
    Ok(chart_root(
        fields,
        &CHART_OF_CALCULATION_TYPES_COLLECTION_UUIDS,
        forms,
        5,
    ))
}

fn chart_root(
    fields: Vec<NativeValue>,
    markers: &[&str],
    forms: Vec<ObjectUuid>,
    collection_count: usize,
) -> NativeValue {
    let mut root = vec![
        token("1"),
        list(fields),
        token(collection_count.to_string()),
    ];
    for (index, marker) in markers.iter().enumerate() {
        let items = if index + 1 == markers.len() {
            forms.iter().copied().map(uuid_value).collect()
        } else {
            Vec::new()
        };
        root.push(native_collection(marker, items));
    }
    list(root)
}

pub(super) fn decode_register_family(
    value: &NativeValue,
    family: BusinessObjectFamily,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    if family == BusinessObjectFamily::Recalculation {
        return decode_recalculation(value);
    }
    let (
        root_len,
        collection_count,
        field_len,
        code,
        header_slot,
        generated_slots,
        markers,
        form_collection,
        recalc_collection,
    ) = match family {
        BusinessObjectFamily::InformationRegister => (
            9,
            "6",
            39,
            "33",
            15,
            &[1, 3, 5, 7, 9, 11, 13][..],
            &INFORMATION_REGISTER_COLLECTION_UUIDS[..],
            5usize,
            None,
        ),
        BusinessObjectFamily::AccumulationRegister => (
            9,
            "6",
            26,
            "28",
            13,
            &[1, 3, 5, 7, 9, 11][..],
            &ACCUMULATION_REGISTER_COLLECTION_UUIDS[..],
            5,
            None,
        ),
        BusinessObjectFamily::AccountingRegister => (
            9,
            "6",
            30,
            "21",
            15,
            &[1, 3, 5, 7, 9, 11, 13][..],
            &ACCOUNTING_REGISTER_COLLECTION_UUIDS[..],
            5,
            None,
        ),
        BusinessObjectFamily::CalculationRegister => (
            10,
            "7",
            33,
            "21",
            15,
            &[1, 3, 5, 7, 9, 11, 13][..],
            &CALCULATION_REGISTER_COLLECTION_UUIDS[..],
            4,
            Some(1usize),
        ),
        BusinessObjectFamily::ChartOfCharacteristicTypes => (
            8,
            "5",
            59,
            "34",
            13,
            &[1, 3, 5, 7, 9, 11][..],
            &CHART_OF_CHARACTERISTIC_TYPES_COLLECTION_UUIDS[..],
            4,
            None,
        ),
        BusinessObjectFamily::ChartOfAccounts => (
            10,
            "7",
            57,
            "32",
            15,
            &[1, 3, 5, 7, 9, 11, 13][..],
            &CHART_OF_ACCOUNTS_COLLECTION_UUIDS[..],
            3,
            None,
        ),
        BusinessObjectFamily::ChartOfCalculationTypes => (
            8,
            "5",
            63,
            "35",
            1,
            &[2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22][..],
            &CHART_OF_CALCULATION_TYPES_COLLECTION_UUIDS[..],
            4,
            None,
        ),
        _ => return native("register decoder received another family"),
    };
    let root = exact_list(value, root_len, "register root")?;
    exact_token(&root[0], "1", "register root discriminator")?;
    exact_token(&root[2], collection_count, "register collection count")?;
    let fields = exact_list(&root[1], field_len, "register owner fields")?;
    exact_token(&fields[0], code, "register owner discriminator")?;
    let wrapper = exact_list(&fields[header_slot], 2, "register owner header wrapper")?;
    exact_token(
        &wrapper[0],
        "0",
        "register owner header wrapper discriminator",
    )?;
    let uuid = parse_header_uuid(&wrapper[1])?;
    let generated_types = generated_slots
        .iter()
        .map(|slot| {
            Ok((
                non_nil_uuid(&fields[*slot], "generated TypeId")?,
                non_nil_uuid(&fields[*slot + 1], "generated ValueId")?,
            ))
        })
        .collect::<Result<Vec<_>, BusinessObjectBuildError>>()?;
    let mut forms = Vec::new();
    let mut recalculations = Vec::new();
    for (index, marker) in markers.iter().enumerate() {
        if index == form_collection {
            forms = parse_uuid_collection(&root[index + 3], marker, "register forms")?;
        } else if Some(index) == recalc_collection {
            recalculations =
                parse_uuid_collection(&root[index + 3], marker, "register recalculations")?;
        } else {
            let values =
                parse_collection(&root[index + 3], marker, "register reserved collection")?;
            if !values.is_empty() {
                return native("register embedded collection is outside the supported cohort");
            }
        }
    }
    validate_register_inventory(uuid, &generated_types, &forms, &recalculations, &[])?;
    Ok(BusinessObjectNativeIr {
        family,
        uuid,
        generated_types,
        attribute_uuids: Vec::new(),
        tabular_sections: Vec::new(),
        command_uuids: Vec::new(),
        form_uuids: forms,
        template_uuids: Vec::new(),
        addressing_attribute_uuids: Vec::new(),
        dimension_uuids: Vec::new(),
        resource_uuids: Vec::new(),
        recalculation_uuids: recalculations,
        content_uuids: Vec::new(),
        child_subsystem_uuids: Vec::new(),
    })
}

fn decode_recalculation(
    value: &NativeValue,
) -> Result<BusinessObjectNativeIr, BusinessObjectBuildError> {
    let root = exact_list(value, 4, "Recalculation root")?;
    exact_token(&root[0], "1", "Recalculation root discriminator")?;
    let fields = exact_list(&root[1], 9, "Recalculation owner fields")?;
    exact_token(&fields[0], "4", "Recalculation owner discriminator")?;
    let wrapper = exact_list(&fields[7], 2, "Recalculation owner header wrapper")?;
    exact_token(
        &wrapper[0],
        "0",
        "Recalculation header wrapper discriminator",
    )?;
    exact_token(&fields[8], "1", "Recalculation owner tail")?;
    let uuid = parse_header_uuid(&wrapper[1])?;
    let generated_types = [1usize, 3, 5]
        .into_iter()
        .map(|slot| {
            Ok((
                non_nil_uuid(&fields[slot], "generated TypeId")?,
                non_nil_uuid(&fields[slot + 1], "generated ValueId")?,
            ))
        })
        .collect::<Result<Vec<_>, BusinessObjectBuildError>>()?;
    let values = parse_collection(
        &root[3],
        RECALCULATION_DIMENSION_COLLECTION_UUID,
        "Recalculation dimensions",
    )?;
    let mut dimensions = Vec::with_capacity(values.len());
    for value in values {
        let item = exact_list(value, 2, "Recalculation dimension item")?;
        exact_token(&item[1], "0", "Recalculation dimension tail")?;
        let payload = exact_list(&item[0], 4, "Recalculation dimension payload")?;
        exact_token(&payload[0], "1", "Recalculation dimension discriminator")?;
        let dimension = parse_header_uuid(&payload[1])?;
        let target = non_nil_uuid(&payload[2], "Recalculation register dimension")?;
        let leading = exact_list(&payload[3], 3, "Recalculation leading data")?;
        exact_token(&leading[0], "0", "Recalculation leading discriminator")?;
        exact_token(&leading[1], "1", "Recalculation leading count")?;
        let typed = exact_list(&leading[2], 3, "Recalculation typed leading reference")?;
        exact_text(&typed[0], "#", "Recalculation reference marker")?;
        exact_token(
            &typed[1],
            METADATA_OBJECT_REF_TYPE_UUID,
            "Recalculation reference type",
        )?;
        let reference = exact_list(&typed[2], 2, "Recalculation reference payload")?;
        exact_token(&reference[0], "1", "Recalculation reference discriminator")?;
        if non_nil_uuid(&reference[1], "Recalculation leading target")? != target {
            return native("Recalculation leading target differs from RegisterDimension");
        }
        dimensions.push(dimension);
    }
    validate_register_inventory(uuid, &generated_types, &[], &[], &dimensions)?;
    Ok(BusinessObjectNativeIr {
        family: BusinessObjectFamily::Recalculation,
        uuid,
        generated_types,
        attribute_uuids: Vec::new(),
        tabular_sections: Vec::new(),
        command_uuids: Vec::new(),
        form_uuids: Vec::new(),
        template_uuids: Vec::new(),
        addressing_attribute_uuids: Vec::new(),
        dimension_uuids: dimensions,
        resource_uuids: Vec::new(),
        recalculation_uuids: Vec::new(),
        content_uuids: Vec::new(),
        child_subsystem_uuids: Vec::new(),
    })
}

fn validate_register_inventory(
    root: ObjectUuid,
    generated: &[(ObjectUuid, ObjectUuid)],
    forms: &[ObjectUuid],
    recalculations: &[ObjectUuid],
    dimensions: &[ObjectUuid],
) -> Result<(), BusinessObjectBuildError> {
    let mut seen = BTreeSet::from([root]);
    for value in generated
        .iter()
        .flat_map(|pair| [pair.0, pair.1])
        .chain(forms.iter().copied())
        .chain(recalculations.iter().copied())
        .chain(dimensions.iter().copied())
    {
        if !seen.insert(value) {
            return native("register native identity inventory contains a duplicate");
        }
    }
    Ok(())
}
