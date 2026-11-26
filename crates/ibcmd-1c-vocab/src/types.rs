//! Platform type identifiers and the XML names they are written as: the
//! built-in type tables, the reference-family type sets, and the value-type
//! model (`ConstantValueType`) the pattern readers produce.

use std::collections::{BTreeMap, BTreeSet};

pub const CONSTANTS_SET_TYPE_UUID: &str = "dcfc3784-a14f-4786-ac7b-c82db5ba275f";
const CHART_TYPE_UUID: &str = "3543ef08-3316-4f7e-9447-0cd0a1cbf1d5";
/// The Gantt chart. Named by every Gantt-chart-typed form attribute the eight
/// stand corpora carry -- 19 of them, one in Documentooborot KORP 3.0.21.3 and
/// 18 in ERP UH 3.2.12.6: each stores this type id and the platform writes this
/// QName both in the attribute's `<v8:Type>` and in the `xsi:type` of its
/// `<Settings>` block. The id had no entry at all, so neither element was
/// written on any of them.
pub const GANTT_CHART_TYPE_UUID: &str = "3a6e63bf-16aa-42eb-b48c-2fff9670ad2f";
/// Русское имя — `ДиаграммаГанта`.
pub const GANTT_CHART_TYPE_REFERENCE: &str = "d5p1:GanttChart";
/// The data-analysis time-interval unit. Named by every form attribute of the
/// eight stand corpora that declares it -- 7 of them, all in ERP UH 3.2.12.6
/// and all in forms that also carry a Gantt chart: each stores this type id
/// with an empty settings slot, and the platform writes this QName in the
/// attribute's `<v8:Type>` and no `<Settings>` block. The id had no entry at
/// all, so the element was written on none of them. The QName occurs nowhere
/// else in the eight native trees.
const DATA_ANALYSIS_TIME_INTERVAL_UNIT_TYPE_UUID: &str = "77a01c71-e9b2-4617-af07-c95a4b74548a";
/// Русское имя — `ТипЕдиницыИнтервалаВремениАнализаДанных`.
const DATA_ANALYSIS_TIME_INTERVAL_UNIT_TYPE_REFERENCE: &str =
    "d5p1:DataAnalysisTimeIntervalUnitType";
const REPORT_BUILDER_TYPE_UUID: &str = "0dda99d9-ae9f-43d2-b7ac-44f3fb0d4059";
pub const DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_UUID: &str = "cab0d12b-3c88-4993-8edc-8c3827cadc7d";
pub const DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_NAME: &str = "dcsset:SettingsComposer";

/// Platform types the writer spells with a prefix **bound at the document
/// root**, so their QName is one fact and not a per-document one.
///
/// The split of platform type names into per-owner tables was never a platform
/// fact. A `<v8:Type>` names a platform type by *namespace and local name*; the
/// prefix is chosen by the document being written. When the namespace is
/// already bound at the root element, the writer uses the root prefix and
/// declares nothing; when it is not, it declares the namespace inline on the
/// `<v8:Type>` element itself, either with the canonical prefix it holds for
/// that namespace (`mxl`, `fd`, `dcsset`, `pdfdoc`, `pl`) or with a generated
/// `d<N>p1` where **N is the 1-based element depth** of that very element.
///
/// The depth rule is measured, not assumed: across all eight stand corpora
/// there are 17 495 inline `xmlns:d<N>p1` declarations and in every one of them
/// N equals the depth of the element carrying it -- 0 counterexamples. It is
/// what makes `d5p1:Chart` and `d7p1:Chart` the same name: type id
/// `3543ef08-…` sits at depth 5 in a form body
/// (`<Form><Attributes><Attribute><Type><v8:Type>`, 30 occurrences) and at
/// depth 7 in a metadata root
/// (`<MetaDataObject><DataProcessor><ChildObjects><Attribute><Properties>
/// <Type><v8:Type>`, 5 occurrences).
///
/// The names below carry `cfg:`, `v8:` and `v8ui:`, and every document that can
/// carry a `<v8:Type>` for them binds those three at its own root -- both the
/// `MDClasses` root and the `logform` root do. Census of the eight corpora:
/// `v8:TypeDescription` 695, `v8ui:Picture` 558, `cfg:ConstantsSet` 227,
/// `cfg:ReportObject` 24, `v8ui:VerticalAlign` 11, `cfg:ReportBuilder` 5 --
/// 1 520 occurrences, and **not one of them carries an inline `xmlns`**. So one
/// table answers every reader for them, and the per-shape tables below keep
/// only the names whose spelling really does depend on the document.
const ROOT_BOUND_PLATFORM_TYPE_REFERENCES: &[(&str, &str)] = &[
    // КонстантыНабор
    (CONSTANTS_SET_TYPE_UUID, "cfg:ConstantsSet"),
    // ОписаниеТипов
    (TYPE_DESCRIPTION_TYPE_UUID, "v8:TypeDescription"),
    // Картинка
    ("e6f51714-91cb-4dce-94fe-90ae3e3e1ad1", "v8ui:Picture"),
    // ОтчетОбъект
    ("1dd6fdb9-553d-40d4-b2d1-c7fc31f497bb", "cfg:ReportObject"),
    // ВертикальноеПоложение
    ("52616226-8ccf-4d1d-a3da-827eeb4f9cf9", "v8ui:VerticalAlign"),
    // Named by `DataProcessors/УниверсальныйОбменДаннымиXML` in ух, ут, до and
    // БСП демо and by `Reports/ПланПоказателей` in ух -- five records of one
    // type id, one spelling, in two different owner families.
    (REPORT_BUILDER_TYPE_UUID, "cfg:ReportBuilder"),
    // The `ent:` and `v8ui:` namespaces are bound at the same roots as `cfg:`,
    // so these names carry no inline declaration either.
    // ДинамическийСписок
    ("65abad24-838b-4987-8b35-ed9e2bd4d9c8", "cfg:DynamicList"),
    // ВидДвиженияНакопления
    (
        "5a49abd5-936d-4f46-8499-9cb204efe378",
        "ent:AccumulationRecordType",
    ),
    // ВидСчета
    ("872f7198-7083-4e3e-b57e-a2a9802c769e", "ent:AccountType"),
    // ВидДвиженияБухгалтерии
    (
        "741ae838-6e42-4ac0-b6a4-17e5604b0669",
        "ent:AccountingRecordType",
    ),
    // ГоризонтальноеПоложение
    (
        "43f9c095-40e8-441a-8fad-20a45798c71b",
        "v8ui:HorizontalAlign",
    ),
    // РежимИзмененияРазмера
    (
        "7a0135c7-5b77-4de2-839e-25c5e63a2142",
        "v8ui:SizeChangeMode",
    ),
];
// Platform type IDs used by serialized Form type patterns. Only the names whose
// prefix the form document itself fixes stay here: the `d5p1:` names by the
// depth rule above, and the four inline-declared canonical prefixes the form
// writer knows how to spell (`form_metadata_type_xml_namespace_attr`). Names
// bound at the root live in `ROOT_BOUND_PLATFORM_TYPE_REFERENCES`.
const FORM_BUILTIN_TYPE_REFERENCES: &[(&str, &str)] = &[
    // КомпоновщикНастроекКомпоновкиДанных
    (
        DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_UUID,
        DATA_PROCESSOR_SETTINGS_COMPOSER_TYPE_NAME,
    ),
    // ФорматированныйДокумент
    (
        "151f8778-e2d0-496a-9f02-d9ffd93b57ec",
        "fd:FormattedDocument",
    ),
    // ТекстовыйДокумент
    ("ebf766b1-f32c-11d3-9851-008048da1252", "d5p1:TextDocument"),
    // Диаграмма
    (CHART_TYPE_UUID, "d5p1:Chart"),
    // ДиаграммаГанта
    (GANTT_CHART_TYPE_UUID, GANTT_CHART_TYPE_REFERENCE),
    // ТипЕдиницыИнтервалаВремениАнализаДанных
    (
        DATA_ANALYSIS_TIME_INTERVAL_UNIT_TYPE_UUID,
        DATA_ANALYSIS_TIME_INTERVAL_UNIT_TYPE_REFERENCE,
    ),
    // ГрафическаяСхема
    (
        "4af83795-fc2a-48cd-9bea-ce665789a62c",
        "d5p1:FlowchartContextType",
    ),
    // ДокументPDF
    (
        "48510817-200c-48c2-9973-06cf90840514",
        PDF_DOCUMENT_TYPE_REFERENCE,
    ),
    // ГеографическаяСхема
    (
        "95de81b0-81c3-4936-9dbb-6400e5c90378",
        GEOGRAPHICAL_SCHEMA_TYPE_REFERENCE,
    ),
    // Планировщик
    (PLANNER_TYPE_UUID, PLANNER_TYPE_REFERENCE),
];
// Both carry their namespace inline on the emitted `<v8:Type>` element; see
// `form_metadata_type_xml_namespace_attr`.
/// Русские имена — `ДокументPDF` и `ГеографическаяСхема`.
const PDF_DOCUMENT_TYPE_REFERENCE: &str = "pdfdoc:PDFDocument";
const GEOGRAPHICAL_SCHEMA_TYPE_REFERENCE: &str = "d5p1:GeographicalSchema";
/// The planner. Named by the five graphical-planner attributes of
/// Документооборот КОРП 3.0.21.3, the only ones the eight stand corpora carry:
/// each stores this type id and the platform writes this QName both in the
/// attribute's `<v8:Type>` and in the `xsi:type` of its `<Settings>` block.
const PLANNER_TYPE_UUID: &str = "43dc7f37-5b1d-42a7-8f28-f545080d0255";
/// Русское имя — `Планировщик`.
const PLANNER_TYPE_REFERENCE: &str = "pl:Planner";

pub const TYPE_DESCRIPTION_TYPE_UUID: &str = "f5c65050-3bbb-11d5-b988-0050bae0a95d";

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ConstantValueType {
    Boolean,
    String {
        length: Option<u32>,
        allowed_length_flag: u8,
    },
    Number {
        digits: u32,
        fraction_digits: u32,
        allowed_sign_flag: u8,
    },
    DateTime {
        date_fractions: &'static str,
    },
    /// `{"R"}` / `{"R",<length>,<allowed length>}`: `xs:base64Binary`, the 8.5
    /// binary data type (БСП 8.5 extension ServiceDesk, `{"R"}` = length 0,
    /// variable).
    BinaryData {
        length: u32,
        allowed_length_flag: u8,
    },
    Reference {
        reference: String,
    },
    ReferenceTypeSet {
        reference: String,
    },
    /// A form pattern can legally name a platform type that neither the
    /// configuration index nor the platform-QName table knows. The platform
    /// preserves that identifier instead of discarding the whole type block.
    TypeId {
        type_id: String,
    },
}

// Platform reference-family TypeIds are protocol identifiers, not metadata UUIDs.
pub const DCS_BUILTIN_REFERENCE_TYPE_SETS: &[(&str, &str)] = &[
    (
        "0a52f9de-73ea-4507-81e8-66217bead73a",
        "cfg:ExchangePlanRef",
    ),
    (
        "11e5f865-1501-40c6-b4d4-022095a296a5",
        "cfg:BusinessProcessRoutePointRef",
    ),
    (
        "214fa4d8-6ba4-4748-a5e1-6332b5887780",
        "cfg:BusinessProcessRef",
    ),
    ("38bfd075-3e63-4aaa-a93e-94521380d579", "cfg:DocumentRef"),
    (
        "474c3bf6-08b5-4ddc-a2ad-989cedf11583",
        // `builtin_type_reference` below has carried this protocol identifier
        // all along; this table did not, so a DCS `valueType` naming the
        // enumeration family failed closed on a name the storage side already
        // knew. The two tables now agree on it.
        "cfg:EnumRef",
    ),
    (
        "593cd424-0877-470d-91f9-b90a982059b4",
        "cfg:ChartOfCalculationTypesRef",
    ),
    ("6291e9b3-8df5-44e1-b6b2-d9fe008016c0", "cfg:TaskRef"),
    (
        "99892482-ed55-4fb5-a7f7-20888820a758",
        "cfg:ChartOfCharacteristicTypesRef",
    ),
    (
        "ac606d60-0209-4159-8e4c-794bc091ce38",
        "cfg:ChartOfAccountsRef",
    ),
    ("e61ef7b8-f3e1-4f4b-8ac7-676e90524997", "cfg:CatalogRef"),
];

pub fn unique_metadata_type_reference<'a>(
    type_index: &'a BTreeMap<String, String>,
    reference_collisions: &BTreeSet<String>,
    type_id: &str,
) -> Option<&'a str> {
    let normalized = type_id.to_ascii_lowercase();
    if reference_collisions.contains(&normalized) {
        return None;
    }
    let mut matches = type_index
        .iter()
        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(type_id));
    let (_, reference) = matches.next()?;
    matches.next().is_none().then_some(reference.as_str())
}

pub fn metadata_reference_is_type_set(reference: &str) -> bool {
    reference.starts_with("cfg:DefinedType.")
        || reference.starts_with("cfg:Characteristic.")
        || reference == "cfg:AnyIBRef"
        || (reference.starts_with("cfg:") && reference.ends_with("Ref") && !reference.contains('.'))
}

pub fn builtin_type_reference(type_id: &str) -> Option<&'static str> {
    match type_id {
        "acf6192e-81ca-46ef-93a6-5a6968b78663" => Some("v8:ValueTable"),
        "140b5ff4-37b1-4df5-b5ec-a0bfd2b94f8f" => Some("v8ui:FormattedString"),
        "9cd510c7-abfc-11d4-9434-004095e12fc7" => Some("v8ui:Color"),
        "9cd510c8-abfc-11d4-9434-004095e12fc7" => Some("v8ui:Font"),
        "e199ca70-93cf-46ce-a54b-6edc88c3a296" => Some("v8:ValueStorage"),
        "e603c0f2-92fb-4d47-8f38-a44a381cf235" => Some("v8:ValueTree"),
        "fc01b5df-97fe-449b-83d4-218a090e681e" => Some("v8:UUID"),
        "3ee983d7-ace7-40f9-bb7e-2e916fcddd56" => Some("v8:FixedStructure"),
        "4500381b-db30-4a10-9db4-990038032acf" => Some("v8:FixedArray"),
        "220455ea-6c85-4513-996f-bbe79ed07774" => Some("v8:FixedMap"),
        "2fdc88ec-7c9b-43cd-8ba5-873f043bdd88" => Some("v8:StandardPeriod"),
        "0387f3a2-7df5-4804-948b-4580a51e4a15" => Some("v8:StandardBeginningDate"),
        "4772b3b4-f4a3-49c0-a1a5-8cb5961511a3" => Some("v8:ValueListType"),
        "e603103e-a318-4edc-a014-b1c6cf94d49f" => Some("mxl:SpreadsheetDocument"),
        "0a52f9de-73ea-4507-81e8-66217bead73a" => Some("cfg:ExchangePlanRef"),
        "280f5f0e-9c8a-49cc-bf6d-4d296cc17a63" => Some("cfg:AnyIBRef"),
        "0195e80c-b157-11d4-9435-004095e12fc7" => Some("cfg:ConstantValueManager"),
        "061d872a-5787-460e-95ac-ed74ea3a3e84" => Some("cfg:DocumentObject"),
        // Three object-family identifiers were rotated among themselves.
        // Event subscriptions decide them: an `<Event>` whose `Source` names a
        // single family pins that family's identifier outright. Census of the
        // subscriptions of `do`, `ssl`, `sslbase`, `mdm`, `ws` and `wms`,
        // pattern against the XML the platform writes for the same object:
        // `fcd3404e` is alone in the pattern of twelve subscriptions and every
        // one of them is written `cfg:BusinessProcessObject`; `do`
        // `ОбменДаннымиДО20ДО21ПередЗаписью` carries exactly
        // `{3e63355c, cf4abea6, fcd3404e}` against
        // `{BusinessProcessObject, CatalogObject, TaskObject}`, which leaves
        // `3e63355c` = `cfg:TaskObject`; and `do`
        // `ОбработкаПредопределенныхЭлементовПередЗаписью` carries
        // `{238e7e88, 30b100d6, 82a1b659, cf4abea6}` against
        // `{CatalogObject, ChartOfAccountsObject, ChartOfCalculationTypesObject,
        // ChartOfCharacteristicTypesObject}`, which leaves `238e7e88` =
        // `cfg:ChartOfAccountsObject`.
        //
        // The rotation was invisible for a long time because the platform
        // writes a subscription's type sets in one canonical order, so a
        // permutation of the same three names still produced the same bytes.
        // It shows only where the pattern carries some of the three and not
        // all: nine subscriptions of `do` and four of `uh`.
        "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed" => Some("cfg:ChartOfAccountsObject"),
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767" => Some("cfg:ChartOfCalculationTypesObject"),
        "3e63355c-1378-4953-be9b-1deb5fb6bec5" => Some("cfg:TaskObject"),
        "82a1b659-b220-4d94-a9bd-14d757b95a48" => Some("cfg:ChartOfCharacteristicTypesObject"),
        "857c4a91-e5f4-4fac-86ec-787626f1c108" => Some("cfg:ExchangePlanObject"),
        "cf4abea6-37b2-11d4-940f-008048da11f9" => Some("cfg:CatalogObject"),
        "fcd3404e-1523-48ce-9bc0-ecdb822684a1" => Some("cfg:BusinessProcessObject"),
        "13134201-f60b-11d5-a3c7-0050bae0a776" => Some("cfg:InformationRegisterRecordSet"),
        "2deed9b8-0056-4ffe-a473-c20a6c32a0bc" => Some("cfg:AccountingRegisterRecordSet"),
        "b64d9a40-1642-11d6-a3c7-0050bae0a776" => Some("cfg:AccumulationRegisterRecordSet"),
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7" => Some("cfg:CalculationRegisterRecordSet"),
        "274bf899-db0e-4df6-8ab5-67bf6371ec0b" => Some("cfg:SequenceRecordSet"),
        "bc587f20-35d9-11d6-a3c7-0050bae0a776" => Some("cfg:RecalculationRecordSet"),
        "0dee6ca3-50a1-4f94-8c34-e70eeb802d81" => Some("cfg:AccumulationRegisterManager"),
        "1aa09f48-f6d5-4999-a7f5-02a15794c795" => Some("cfg:InformationRegisterManager"),
        "2066866d-9d38-47fe-a272-3cd416eb9c85" => Some("cfg:ChartOfAccountsManager"),
        "26dd1dee-252a-4942-b4b5-62ea44ed8030" => Some("cfg:DocumentManager"),
        "2d0abc8e-dede-4184-afd7-7ae8da588d47" => Some("cfg:CalculationRegisterManager"),
        "38f1038d-8b0b-438b-bfbe-830a60a1153a" => Some("cfg:BusinessProcessManager"),
        "38bfd075-3e63-4aaa-a93e-94521380d579" => Some("cfg:DocumentRef"),
        "3ab47eda-6a5c-4590-9b08-0e633aa2f376" => Some("cfg:AccountingRegisterManager"),
        "3eab4ff4-f2d1-4c96-831c-04711b093999" => Some("cfg:ChartOfCalculationTypesManager"),
        "5e268c17-8035-458f-8041-daf9b15d05c9" => Some("cfg:TaskManager"),
        "7612de75-8b10-466a-b235-68572c605d92" => Some("cfg:ChartOfCharacteristicTypesManager"),
        "82faabf3-7f9b-4b2e-b499-98876415f270" => Some("cfg:CatalogManager"),
        "92e7f73f-bd66-4d9e-bc43-bae2acfadfd5" => Some("cfg:DocumentJournalManager"),
        "e61ef7b8-f3e1-4f4b-8ac7-676e90524997" => Some("cfg:CatalogRef"),
        // Same reference family as `cfg:CatalogRef` above.
        // `information_register_builtin_reference` (mod.rs:~16452) already
        // resolved this identifier for register children only; keeping the
        // mapping here instead makes the one table serve every pattern reader,
        // which is what the register reader itself falls back to.
        "474c3bf6-08b5-4ddc-a2ad-989cedf11583" => Some("cfg:EnumRef"),
        // The reference-family protocol identifiers live in one table. This
        // reader carried five of them and the DCS reader ten, and the two
        // overlapping copies were already reconciled once, for `cfg:EnumRef`
        // just above. `do` `BusinessProcess.КомплексныйПроцесс` writes
        // `{"Pattern",{"#",11e5f865-…}}` for its `ТочкаМаршрута` attribute --
        // `cfg:BusinessProcessRoutePointRef`, a name only the other copy knew,
        // so the object failed closed on an identifier the project already
        // resolves elsewhere. One table now answers both readers.
        _ => ROOT_BOUND_PLATFORM_TYPE_REFERENCES
            .iter()
            .find_map(|(candidate, reference)| {
                candidate
                    .eq_ignore_ascii_case(type_id)
                    .then_some(*reference)
            })
            .or_else(|| {
                DCS_BUILTIN_REFERENCE_TYPE_SETS
                    .iter()
                    .find_map(|(candidate, reference)| {
                        (*candidate == type_id).then_some(*reference)
                    })
            })
            .or_else(|| {
                ibcmd_schema::metadata_storage_facts::data_processor_builtin_type_reference(type_id)
            }),
    }
}

pub(crate) fn form_builtin_type_reference(type_id: &str) -> Option<&'static str> {
    FORM_BUILTIN_TYPE_REFERENCES
        .iter()
        .find_map(|(candidate, reference)| {
            candidate
                .eq_ignore_ascii_case(type_id)
                .then_some(*reference)
        })
        .or_else(|| platform_reference_family_type_reference(type_id))
}

pub fn data_processor_builtin_type_reference(type_id: &str) -> Option<&'static str> {
    ibcmd_schema::metadata_storage_facts::data_processor_builtin_type_reference(type_id)
        .or_else(|| platform_reference_family_type_reference(type_id))
}

pub fn platform_reference_family_type_reference(type_id: &str) -> Option<&'static str> {
    DCS_BUILTIN_REFERENCE_TYPE_SETS
        .iter()
        .find_map(|(candidate, reference)| {
            candidate
                .eq_ignore_ascii_case(type_id)
                .then_some(*reference)
        })
        .or_else(|| builtin_type_reference(type_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_root_bound_platform_type_name_is_one_fact_for_every_reader() {
        // A `<v8:Type>` names a platform type by namespace and local name; the
        // prefix belongs to the document. Where the namespace is bound at the
        // document root the writer uses the root prefix and declares nothing, and
        // both the `MDClasses` root and the `logform` root bind `cfg`, `v8` and
        // `v8ui`. Census of the eight stand corpora: 1 520 `<v8:Type>` elements
        // spell one of the names below and not one of them carries an inline
        // `xmlns`. So all three readers must answer the same for every one.
        for (identifier, reference) in ROOT_BOUND_PLATFORM_TYPE_REFERENCES {
            assert_eq!(builtin_type_reference(identifier), Some(*reference));
            assert_eq!(form_builtin_type_reference(identifier), Some(*reference));
            assert_eq!(
                data_processor_builtin_type_reference(identifier),
                Some(*reference)
            );
        }
        // And the per-document-shape tables keep no root-bound name of their own:
        // a private second copy of such a row is exactly what let
        // `cfg:ReportBuilder` be known to the data-processor reader and unknown to
        // every other one.
        for (_, reference) in FORM_BUILTIN_TYPE_REFERENCES
            .iter()
            .copied()
            .chain(ibcmd_schema::metadata_storage_facts::data_processor_builtin_type_references())
        {
            assert!(
                !reference.starts_with("cfg:")
                    && !reference.starts_with("v8:")
                    && !reference.starts_with("v8ui:"),
                "{reference} is spelled with a root-bound prefix and belongs to the shared table"
            );
        }
    }
}
