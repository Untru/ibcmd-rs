//! The compiler's built-in type table: the platform types a `<v8:Type>` or
//! `<v8:TypeSet>` names that no configuration generates, by name -> TypeId
//! (`builtin_type_id`) and back (`builtin_type_qname`).
//!
//! It moved here verbatim, but for visibility, from the root's
//! `metadata_model::types`, which reaches it by its old paths.
//!
//! The exporter's table, `builtin_type_reference` (id -> the name the export
//! writes), was written apart from this one and is not the same table: it
//! knows ids this one does not, this one knows ids it does not, and it spells
//! a type whose prefix is generated per document with `d7p1:` where this one
//! keeps the `d0p1:` placeholder. Where both know an id they name the same
//! type; `exporter_table_names_the_same_types` holds them to that.

/// `{"#",<this>,{<count>,<value>...}}`: `v8:FixedArray`.
pub const FIXED_ARRAY_TYPE: &str = "4500381b-db30-4a10-9db4-990038032acf";

/// Platform types named in `<v8:Type>` / `<v8:TypeSet>` that the
/// configuration does not generate.
const BUILTIN_TYPES: &[(&str, &str)] = &[
    ("v8:ValueStorage", "e199ca70-93cf-46ce-a54b-6edc88c3a296"),
    ("v8:UUID", "fc01b5df-97fe-449b-83d4-218a090e681e"),
    ("v8:ValueTable", "acf6192e-81ca-46ef-93a6-5a6968b78663"),
    ("v8:ValueTree", "e603c0f2-92fb-4d47-8f38-a44a381cf235"),
    ("v8:FixedStructure", "3ee983d7-ace7-40f9-bb7e-2e916fcddd56"),
    ("v8:FixedArray", FIXED_ARRAY_TYPE),
    ("v8:FixedMap", "220455ea-6c85-4513-996f-bbe79ed07774"),
    ("v8:StandardPeriod", "2fdc88ec-7c9b-43cd-8ba5-873f043bdd88"),
    (
        "v8:StandardBeginningDate",
        "0387f3a2-7df5-4804-948b-4580a51e4a15",
    ),
    ("v8:ValueListType", "4772b3b4-f4a3-49c0-a1a5-8cb5961511a3"),
    ("v8:TypeDescription", "f5c65050-3bbb-11d5-b988-0050bae0a95d"),
    ("v8:FillChecking", "98ea8e5a-b586-442b-b944-6e3447734aa7"),
    (
        "v8ui:FormattedString",
        "140b5ff4-37b1-4df5-b5ec-a0bfd2b94f8f",
    ),
    ("v8ui:Color", "9cd510c7-abfc-11d4-9434-004095e12fc7"),
    ("v8ui:Font", "9cd510c8-abfc-11d4-9434-004095e12fc7"),
    ("v8ui:Picture", "e6f51714-91cb-4dce-94fe-90ae3e3e1ad1"),
    ("v8ui:VerticalAlign", "52616226-8ccf-4d1d-a3da-827eeb4f9cf9"),
    (
        "v8ui:HorizontalAlign",
        "43f9c095-40e8-441a-8fad-20a45798c71b",
    ),
    // РежимИзмененияРазмера
    (
        "v8ui:SizeChangeMode",
        "7a0135c7-5b77-4de2-839e-25c5e63a2142",
    ),
    (
        "mxl:SpreadsheetDocument",
        "e603103e-a318-4edc-a014-b1c6cf94d49f",
    ),
    (
        "fd:FormattedDocument",
        "151f8778-e2d0-496a-9f02-d9ffd93b57ec",
    ),
    ("pdfdoc:PDFDocument", "48510817-200c-48c2-9973-06cf90840514"),
    ("pl:Planner", "43dc7f37-5b1d-42a7-8f28-f545080d0255"),
    (
        "dcsset:SettingsComposer",
        "cab0d12b-3c88-4993-8edc-8c3827cadc7d",
    ),
    ("dcsset:Filter", "f6841c6b-6c71-4c82-ae9e-d08b49db326c"),
    (
        "dcsset:DataCompositionComparisonType",
        "dcbf2698-3c1f-4a22-997f-48070ae9bd64",
    ),
    (
        "dcsset:DataCompositionFieldPlacement",
        "a090004e-b706-453f-aa10-090a77b53757",
    ),
    (
        "dcscor:DataCompositionSortDirection",
        "af4a19b5-da3d-406f-be0c-81143e400452",
    ),
    (
        "dcscor:DataCompositionGroupType",
        "0e0850cf-0634-414e-85ba-9a88a8bd44c4",
    ),
    (
        "dcscor:DataCompositionPeriodAdditionType",
        "c6a52555-d20f-452c-bfc2-1b53e9a56063",
    ),
    ("dcscor:Field", "913e8016-6e90-47a0-b2a0-4513f4edad61"),
    (
        "ent:AccountingRecordType",
        "741ae838-6e42-4ac0-b6a4-17e5604b0669",
    ),
    // ВидСчета
    ("ent:AccountType", "872f7198-7083-4e3e-b57e-a2a9802c769e"),
    //ВидДвиженияНакопления
    (
        "ent:AccumulationRecordType",
        "5a49abd5-936d-4f46-8499-9cb204efe378",
    ),
    ("ent:ComparisonType", "b1b064f3-ae38-49bf-8c6d-390c65fd94af"),
    ("cfg:ConstantsSet", "dcfc3784-a14f-4786-ac7b-c82db5ba275f"),
    ("cfg:ReportBuilder", "0dda99d9-ae9f-43d2-b7ac-44f3fb0d4059"),
    ("cfg:ReportObject", "1dd6fdb9-553d-40d4-b2d1-c7fc31f497bb"),
    ("cfg:DynamicList", "65abad24-838b-4987-8b35-ed9e2bd4d9c8"),
    // Type sets (`<v8:TypeSet>cfg:CatalogRef</v8:TypeSet>`): every object of
    // one family.
    ("cfg:AnyIBRef", "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63"),
    // The same type set as a configuration under an older compatibility
    // mode spells it (`compatibility_mode_spells_any_ref`); read only, the
    // id's name is the one above.
    ("cfg:AnyRef", "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63"),
    ("cfg:CatalogRef", "e61ef7b8-f3e1-4f4b-8ac7-676e90524997"),
    ("cfg:DocumentRef", "38bfd075-3e63-4aaa-a93e-94521380d579"),
    ("cfg:EnumRef", "474c3bf6-08b5-4ddc-a2ad-989cedf11583"),
    (
        "cfg:ExchangePlanRef",
        "0a52f9de-73ea-4507-81e8-66217bead73a",
    ),
    (
        "cfg:ChartOfCharacteristicTypesRef",
        "99892482-ed55-4fb5-a7f7-20888820a758",
    ),
    (
        "cfg:ChartOfAccountsRef",
        "ac606d60-0209-4159-8e4c-794bc091ce38",
    ),
    (
        "cfg:ChartOfCalculationTypesRef",
        "593cd424-0877-470d-91f9-b90a982059b4",
    ),
    (
        "cfg:BusinessProcessRef",
        "214fa4d8-6ba4-4748-a5e1-6332b5887780",
    ),
    (
        "cfg:BusinessProcessRoutePointRef",
        "11e5f865-1501-40c6-b4d4-022095a296a5",
    ),
    ("cfg:TaskRef", "6291e9b3-8df5-44e1-b6b2-d9fe008016c0"),
    ("cfg:CatalogObject", "cf4abea6-37b2-11d4-940f-008048da11f9"),
    ("cfg:DocumentObject", "061d872a-5787-460e-95ac-ed74ea3a3e84"),
    (
        "cfg:ExchangePlanObject",
        "857c4a91-e5f4-4fac-86ec-787626f1c108",
    ),
    (
        "cfg:ChartOfCharacteristicTypesObject",
        "82a1b659-b220-4d94-a9bd-14d757b95a48",
    ),
    (
        "cfg:ChartOfAccountsObject",
        "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed",
    ),
    (
        "cfg:ChartOfCalculationTypesObject",
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767",
    ),
    (
        "cfg:BusinessProcessObject",
        "fcd3404e-1523-48ce-9bc0-ecdb822684a1",
    ),
    ("cfg:TaskObject", "3e63355c-1378-4953-be9b-1deb5fb6bec5"),
    (
        "cfg:ConstantValueManager",
        "0195e80c-b157-11d4-9435-004095e12fc7",
    ),
    (
        "cfg:InformationRegisterRecordSet",
        "13134201-f60b-11d5-a3c7-0050bae0a776",
    ),
    (
        "cfg:AccountingRegisterRecordSet",
        "2deed9b8-0056-4ffe-a473-c20a6c32a0bc",
    ),
    (
        "cfg:AccumulationRegisterRecordSet",
        "b64d9a40-1642-11d6-a3c7-0050bae0a776",
    ),
    (
        "cfg:CalculationRegisterRecordSet",
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7",
    ),
    (
        "cfg:SequenceRecordSet",
        "274bf899-db0e-4df6-8ab5-67bf6371ec0b",
    ),
    (
        "cfg:RecalculationRecordSet",
        "bc587f20-35d9-11d6-a3c7-0050bae0a776",
    ),
    ("cfg:CatalogManager", "82faabf3-7f9b-4b2e-b499-98876415f270"),
    (
        "cfg:DocumentManager",
        "26dd1dee-252a-4942-b4b5-62ea44ed8030",
    ),
    (
        "cfg:DocumentJournalManager",
        "92e7f73f-bd66-4d9e-bc43-bae2acfadfd5",
    ),
    (
        "cfg:ChartOfCharacteristicTypesManager",
        "7612de75-8b10-466a-b235-68572c605d92",
    ),
    (
        "cfg:ChartOfAccountsManager",
        "2066866d-9d38-47fe-a272-3cd416eb9c85",
    ),
    (
        "cfg:ChartOfCalculationTypesManager",
        "3eab4ff4-f2d1-4c96-831c-04711b093999",
    ),
    (
        "cfg:InformationRegisterManager",
        "1aa09f48-f6d5-4999-a7f5-02a15794c795",
    ),
    (
        "cfg:AccumulationRegisterManager",
        "0dee6ca3-50a1-4f94-8c34-e70eeb802d81",
    ),
    (
        "cfg:AccountingRegisterManager",
        "3ab47eda-6a5c-4590-9b08-0e633aa2f376",
    ),
    (
        "cfg:CalculationRegisterManager",
        "2d0abc8e-dede-4184-afd7-7ae8da588d47",
    ),
    (
        "cfg:BusinessProcessManager",
        "38f1038d-8b0b-438b-bfbe-830a60a1153a",
    ),
    ("cfg:TaskManager", "5e268c17-8035-458f-8041-daf9b15d05c9"),
];

/// Platform types whose prefix is generated per document (`d5p1:Chart`,
/// `d7p1:Chart`): matched by local name; the export writes `d0p1:` and the
/// writer puts the element's depth in.
const BUILTIN_LOCAL_TYPES: &[(&str, &str)] = &[
    ("d0p1:Filter", "4652c4ec-1d1d-4af4-b835-e33fcb43af8c"),
    // Диаграмма
    ("d0p1:Chart", "3543ef08-3316-4f7e-9447-0cd0a1cbf1d5"),
    // ДиаграммаГанта
    ("d0p1:GanttChart", "3a6e63bf-16aa-42eb-b48c-2fff9670ad2f"),
    // Дендрограмма
    ("d0p1:Dendrogram", "2957f638-fc52-4bbb-9bd5-614550730ca6"),
    // ТекстовыйДокумент
    ("d0p1:TextDocument", "ebf766b1-f32c-11d3-9851-008048da1252"),
    // ГеографическаяСхема
    (
        "d0p1:GeographicalSchema",
        "95de81b0-81c3-4936-9dbb-6400e5c90378",
    ),
    // ГрафическаяСхема
    (
        "d0p1:FlowchartContextType",
        "4af83795-fc2a-48cd-9bea-ce665789a62c",
    ),
    // ТипЕдиницыИнтервалаВремениАнализаДанных
    (
        "d0p1:DataAnalysisTimeIntervalUnitType",
        "77a01c71-e9b2-4617-af07-c95a4b74548a",
    ),
    // ВидДанныхАнализа
    (
        "d0p1:AnalysisDataType",
        "0bc96bbe-a6e8-4fdc-aecd-57f493904535",
    ),
    // ТипИсточникаДанныхПоискаАссоциаций
    (
        "d0p1:AssociationRulesDataSourceType",
        "050f918e-cda3-4d92-997a-df4a76f00cd1",
    ),
    // ТипОтсеченияПравилАссоциации
    (
        "d0p1:AssociationRulesPruneType",
        "f8cdf4bc-4411-4b98-b8a3-d6f8e3ddb1c7",
    ),
    // МетодКластеризации
    (
        "d0p1:ClusterizationMethod",
        "df706cce-3392-469c-af4c-cd4a13bf13bb",
    ),
    // ТипУпорядочиванияПравилАссоциацииАнализаДанных
    (
        "d0p1:DataAnalysisAssociationRulesOrderType",
        "e838bbb4-8cdd-4b96-8259-66a1c44ca631",
    ),
    // ТипКолонкиАнализаДанныхПоискАссоциаций
    (
        "d0p1:DataAnalysisColumnTypeAssociationRules",
        "148a98e5-0ad6-42c9-8133-e5ebbff69734",
    ),
    // ТипКолонкиАнализаДанныхКластеризация
    (
        "d0p1:DataAnalysisColumnTypeClusterization",
        "ffdb46a2-3f23-4712-bad7-ad26997fb2d4",
    ),
    // ТипКолонкиАнализаДанныхДеревоРешений
    (
        "d0p1:DataAnalysisColumnTypeDecisionTree",
        "5f290fad-aa2a-4245-b200-cd035ab19571",
    ),
    // ТипКолонкиАнализаДанныхПоискПоследовательностей
    (
        "d0p1:DataAnalysisColumnTypeSequentialPatterns",
        "7d8b2989-2a0a-4334-afe1-82ceb63cf38a",
    ),
    // ТипКолонкиАнализаДанныхОбщаяСтатистика
    (
        "d0p1:DataAnalysisColumnTypeSummaryStatistics",
        "efb63b25-8150-4c82-9687-970924f5f1f8",
    ),
    // ТипМерыРасстоянияАнализаДанных
    (
        "d0p1:DataAnalysisDistanceMetricType",
        "0d3438ab-6412-4d39-91a9-a7d4e8418d27",
    ),
    // ТипИспользованияЧисловыхЗначенийАнализаДанных
    (
        "d0p1:DataAnalysisNumericValueUseType",
        "2ee0f1c6-3781-421d-b7de-c4c359cb159c",
    ),
    // ТипЗаполненияТаблицыРезультатаАнализаДанных
    (
        "d0p1:DataAnalysisResultTableFillType",
        "18ae9d87-950e-44a6-883a-1cd1f63040fb",
    ),
    // ТипУпорядочиванияШаблоновПоследовательностейАнализаДанных
    (
        "d0p1:DataAnalysisSequentialPatternsOrderType",
        "6359f32c-4192-46cf-86c0-2f897b70a339",
    ),
    // ТипСтандартизацииАнализаДанных
    (
        "d0p1:DataAnalysisStandardizationType",
        "afaa3f6e-29a1-4064-a825-73699c7db53b",
    ),
    // ТипУпрощенияДереваРешений
    (
        "d0p1:DecisionTreeSimplificationType",
        "4622a61b-5817-4bf3-bf46-08bed102c02a",
    ),
    // ТипКолонкиМоделиПрогноза
    (
        "d0p1:PredictionModelColumnType",
        "d015cb03-c768-4078-bb5d-b9404d99bc4c",
    ),
    // УсловноеОформление
    (
        "d0p1:ConditionalAppearance",
        "7dd764b6-b22f-4712-8edc-c0d634340e60",
    ),
];

pub fn builtin_type_id(name: &str) -> Option<&'static str> {
    if let Some((_, id)) = BUILTIN_TYPES
        .iter()
        .find(|(candidate, _)| *candidate == name)
    {
        return Some(id);
    }
    let (prefix, local) = name.split_once(':')?;
    if !(prefix.starts_with('d') && prefix.ends_with("p1")) {
        return None;
    }
    BUILTIN_LOCAL_TYPES
        .iter()
        .find(|(candidate, _)| candidate.split_once(':').map(|(_, own)| own) == Some(local))
        .map(|(_, id)| *id)
}

/// The inverse: a platform type's id -> the name `<v8:Type>` writes.
pub fn builtin_type_qname(type_id: &str) -> Option<&'static str> {
    BUILTIN_TYPES
        .iter()
        .chain(BUILTIN_LOCAL_TYPES)
        .find_map(|(name, id)| (*id == type_id).then_some(*name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_ref_reads_as_the_any_ib_ref_type_set() {
        assert_eq!(
            builtin_type_id("cfg:AnyRef"),
            builtin_type_id("cfg:AnyIBRef")
        );
        assert!(builtin_type_id("cfg:AnyRef").is_some());
        assert_eq!(
            builtin_type_qname("280f5f0e-9c8a-49cc-bf6d-4d296cc17a63"),
            Some("cfg:AnyIBRef")
        );
    }

    /// Every type of this table that the exporter's `builtin_type_reference`
    /// also knows, it names the same: by the QName `builtin_type_qname`
    /// writes, or, for a type whose prefix is generated per document, by a
    /// name with the same local part that `builtin_type_id` reads back as the
    /// same id. The types only this table knows are listed whole.
    #[test]
    fn exporter_table_names_the_same_types() {
        let mut compiler_only = Vec::new();
        for (name, id) in BUILTIN_TYPES.iter().chain(BUILTIN_LOCAL_TYPES) {
            assert_eq!(builtin_type_id(name), Some(*id), "{name}");
            let qname = builtin_type_qname(id).unwrap();
            if qname != *name {
                // A second name of the same id (`cfg:AnyRef`), read only.
                continue;
            }
            match crate::builtin_type_reference(id) {
                None => compiler_only.push(qname),
                Some(reference) if reference == qname => {}
                Some(reference) => {
                    assert!(
                        BUILTIN_LOCAL_TYPES.iter().any(|(local, _)| *local == qname),
                        "{qname}: the exporter writes {reference}"
                    );
                    assert_eq!(
                        reference.split_once(':').map(|(_, local)| local),
                        qname.split_once(':').map(|(_, local)| local),
                        "{qname}: {reference}"
                    );
                    assert_eq!(
                        builtin_type_id(reference),
                        Some(*id),
                        "{qname}: {reference}"
                    );
                }
            }
        }
        assert_eq!(
            compiler_only,
            [
                "v8:FillChecking",
                "dcsset:SettingsComposer",
                "dcsset:Filter",
                "dcsset:DataCompositionComparisonType",
                "dcsset:DataCompositionFieldPlacement",
                "dcscor:DataCompositionSortDirection",
                "dcscor:DataCompositionGroupType",
                "dcscor:DataCompositionPeriodAdditionType",
                "dcscor:Field",
                "ent:ComparisonType",
                "d0p1:ConditionalAppearance",
            ]
        );
    }
}
