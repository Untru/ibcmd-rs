//! Type descriptions, typed values, design-time references and data paths:
//! the value encoders every family shares. Owned by the simple-objects track.
//!
//! Public API (stable; extended additively):
//! - [`type_pattern`]: a `v8:TypeDescription` element (`<Type>`, an event
//!   subscription's `<Source>`, a command's `<CommandParameterType>`, ...) ->
//!   `{"Pattern",<item>...}`, items in the order the platform stores them.
//! - [`typed_value`]: a value element carrying `xsi:type` / `xsi:nil`
//!   (`<MinValue>`, `<FillValue>`, a choice parameter's `<value>`) -> `{"U"}`,
//!   `{"S","..."}`, `{"B",1}`, `{"N",5}`, `{"D",20200101000000}`, a
//!   design-time reference or a fixed array of those.
//! - [`design_time_ref`]: `Catalog.X.EmptyRef`, `Enum.E.EnumValue.V`,
//!   `Catalog.X.<predefined item>`, `<type uuid>.<value uuid>` ->
//!   `{"#",5c14e26f-...,{0,<ref type id>,<value uuid>}}`.
//! - [`metadata_ref`]: an `xr:MDObjectRef` full name ->
//!   `{"#",157fa490-...,{1,<uuid>}}`.
//! - [`data_path`]: a field data path (`Catalog.X.Attribute.Y`,
//!   `Document.X.TabularSection.T.Attribute.A`,
//!   `Catalog.X.StandardAttribute.Owner`, `-8`, `0:<uuid>/0:<uuid>`, `0`) ->
//!   its segments (`{0,<uuid>}`, `{-5}`, `{0}`).
//! - [`object_uuid`]: a full name (`Catalog.X`, `Catalog.X.Form.F`,
//!   `Catalog.X.TabularSection.T.Attribute.A`, `Enum.E.EnumValue.V`) -> uuid.
//! - [`ref_type_id`]: `Catalog.X` -> the TypeId of `CatalogRef.X`.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Result, anyhow, bail};

use super::brace::{Brace, NIL_UUID};
use super::xml::{Element, parse_element_tree};
use super::{DescriptorContext, native_text};
use crate::brace_list;

#[path = "types_export.rs"]
pub(crate) mod export;

/// `{"#",<this>,{0,<type>,<value>}}`: a design-time reference value.
pub const DESIGN_TIME_REF_TYPE: &str = "5c14e26f-099b-4d37-84a6-b433d87400da";
/// `{"#",<this>,{<count>,<value>...}}`: `v8:FixedArray`.
pub const FIXED_ARRAY_TYPE: &str = "4500381b-db30-4a10-9db4-990038032acf";
/// `{"#",<this>,{1,<uuid>}}`: a metadata object reference (`xr:MDObjectRef`).
pub const METADATA_OBJECT_REF_TYPE: &str = "157fa490-4ce9-11d4-9415-008048da11f9";
/// `{"#",<this>,<pattern>}`: a `v8:TypeDescription` value.
pub const TYPE_DESCRIPTION_TYPE: &str = "f5c65050-3bbb-11d5-b988-0050bae0a95d";
/// `{"#",<this>,<0 active | 1 passive | 2 active-passive>}`: `ent:AccountType`.
pub const ACCOUNT_TYPE_TYPE: &str = "872f7198-7083-4e3e-b57e-a2a9802c769e";

/// Where the platform sorts its primitive pattern items among the uuid-typed
/// ones. A pattern is ordered by type id and the primitives sort as if their
/// ids were these. Every one of the 15 132 multi-item patterns stored in the
/// four corpora (every Config row, forms included) agrees, and bounds each
/// primitive to a narrow window: `B` in (5d12bac6, 5ddff675), `S` in
/// (9b601b75, 9c5f90e5), `D` in (a8154859, abb75494), `N` in (ac606d60,
/// b165a6f6), `L` above aec47505.
const BOOLEAN_KEY: &str = "5d800000-0000-0000-0000-000000000000";
const STRING_KEY: &str = "9c000000-0000-0000-0000-000000000000";
const DATE_KEY: &str = "aa000000-0000-0000-0000-000000000000";
const NUMBER_KEY: &str = "af000000-0000-0000-0000-000000000000";
const NULL_KEY: &str = "f0000000-0000-0000-0000-000000000000";
const BINARY_KEY: &str = "40000000-0000-0000-0000-000000000000";

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
    ("d0p1:Chart", "3543ef08-3316-4f7e-9447-0cd0a1cbf1d5"),
    ("d0p1:GanttChart", "3a6e63bf-16aa-42eb-b48c-2fff9670ad2f"),
    ("d0p1:TextDocument", "ebf766b1-f32c-11d3-9851-008048da1252"),
    (
        "d0p1:GeographicalSchema",
        "95de81b0-81c3-4936-9dbb-6400e5c90378",
    ),
    (
        "d0p1:FlowchartContextType",
        "4af83795-fc2a-48cd-9bea-ce665789a62c",
    ),
    (
        "d0p1:DataAnalysisTimeIntervalUnitType",
        "77a01c71-e9b2-4617-af07-c95a4b74548a",
    ),
    (
        "d0p1:ConditionalAppearance",
        "7dd764b6-b22f-4712-8edc-c0d634340e60",
    ),
];

pub(crate) fn builtin_type_id(name: &str) -> Option<&'static str> {
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

/// A type the configuration generates (`CatalogRef.X`, `DefinedType.Y`,
/// `Characteristic.Z`, `CatalogObject.X`, ...) -> its TypeId.
pub fn generated_type_id(name: &str, context: &DescriptorContext) -> Option<String> {
    context
        .index
        .generated_type(name)
        .map(|generated| generated.type_id.clone())
}

/// A `v8:Type` / `v8:TypeSet` text -> the uuid its pattern item names.
pub fn named_type_id(name: &str, context: &DescriptorContext) -> Result<String> {
    if let Some(id) = builtin_type_id(name) {
        return Ok(id.to_string());
    }
    if let Some(local) = name.strip_prefix("cfg:")
        && let Some(id) = generated_type_id(local, context)
    {
        return Ok(id);
    }
    bail!("unknown type {name}")
}

/// `<Type>` -> `{"Pattern",<item>...}`. `None` (no type description at all)
/// is the empty pattern `{"Pattern"}`.
pub fn type_pattern(type_el: Option<&Element>, context: &DescriptorContext) -> Result<Brace> {
    let mut items = type_pattern_items(type_el, context)?;
    let mut out = Vec::with_capacity(items.len() + 1);
    out.push(Brace::str("Pattern"));
    out.append(&mut items);
    Ok(Brace::List(out))
}

/// The pattern's items without the `"Pattern"` head, in stored order.
pub fn type_pattern_items(
    type_el: Option<&Element>,
    context: &DescriptorContext,
) -> Result<Vec<Brace>> {
    let Some(type_el) = type_el else {
        return Ok(Vec::new());
    };
    let string_q = type_el.child("StringQualifiers");
    let number_q = type_el.child("NumberQualifiers");
    let date_q = type_el.child("DateQualifiers");
    let binary_q = type_el.child("BinaryDataQualifiers");
    let mut keyed: Vec<(String, Brace)> = Vec::new();
    for child in &type_el.children {
        let name = child.text.trim();
        match child.name.as_str() {
            "Type" => keyed.push(match name {
                "xs:boolean" => (BOOLEAN_KEY.to_string(), brace_list![Brace::str("B")]),
                "xs:string" => (STRING_KEY.to_string(), string_item(string_q)?),
                "xs:decimal" => (NUMBER_KEY.to_string(), number_item(number_q)?),
                "xs:dateTime" => (DATE_KEY.to_string(), date_item(date_q)),
                "v8:Null" => (NULL_KEY.to_string(), brace_list![Brace::str("L")]),
                "xs:base64Binary" | "v8:BinaryData" => {
                    (BINARY_KEY.to_string(), binary_item(binary_q)?)
                }
                other => type_id_item(named_type_id(other, context)?),
            }),
            "TypeSet" => keyed.push(type_id_item(named_type_id(name, context)?)),
            "TypeId" => keyed.push(type_id_item(name.to_ascii_lowercase())),
            _ => {}
        }
    }
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    keyed.dedup();
    Ok(keyed.into_iter().map(|(_, item)| item).collect())
}

fn type_id_item(id: String) -> (String, Brace) {
    let item = brace_list![Brace::str("#"), Brace::uuid(&id)];
    (id, item)
}

fn qualifier_number(qualifiers: &Element, name: &str) -> Result<i64> {
    let text = qualifiers.child_text(name).unwrap_or("0").trim();
    text.parse::<i64>()
        .map_err(|_| anyhow!("bad <{name}> qualifier {text:?}"))
}

fn allowed_length_flag(qualifiers: &Element) -> i64 {
    match qualifiers.child_text("AllowedLength") {
        Some("Fixed") => 0,
        _ => 1,
    }
}

/// `{"S"}` (unlimited) or `{"S",<length>,<1 variable | 0 fixed>}`.
fn string_item(qualifiers: Option<&Element>) -> Result<Brace> {
    let Some(qualifiers) = qualifiers else {
        return Ok(brace_list![Brace::str("S")]);
    };
    let length = qualifier_number(qualifiers, "Length")?;
    if length == 0 {
        return Ok(brace_list![Brace::str("S")]);
    }
    Ok(brace_list![
        Brace::str("S"),
        Brace::num(length),
        Brace::num(allowed_length_flag(qualifiers)),
    ])
}

/// `{"N",<digits>,<fraction digits>,<1 nonnegative | 0 any>}`; `{"N"}` for
/// the unqualified number (`0,0,Any` or no qualifiers at all, as a form's
/// `ВыгрузкаЗагрузкаДанныхXML` attribute stores it).
fn number_item(qualifiers: Option<&Element>) -> Result<Brace> {
    let Some(qualifiers) = qualifiers else {
        return Ok(brace_list![Brace::str("N")]);
    };
    let digits = qualifier_number(qualifiers, "Digits")?;
    let fraction = qualifier_number(qualifiers, "FractionDigits")?;
    let nonnegative = qualifiers.child_text("AllowedSign") == Some("Nonnegative");
    if digits == 0 && fraction == 0 && !nonnegative {
        return Ok(brace_list![Brace::str("N")]);
    }
    Ok(brace_list![
        Brace::str("N"),
        Brace::num(digits),
        Brace::num(fraction),
        Brace::flag(nonnegative),
    ])
}

/// `{"D"}` (date and time), `{"D","D"}` (date), `{"D","T"}` (time).
fn date_item(qualifiers: Option<&Element>) -> Brace {
    match qualifiers.and_then(|qualifiers| qualifiers.child_text("DateFractions")) {
        Some("Date") => brace_list![Brace::str("D"), Brace::str("D")],
        Some("Time") => brace_list![Brace::str("D"), Brace::str("T")],
        _ => brace_list![Brace::str("D")],
    }
}

/// `{"R"}` or `{"R",<length>,<1 variable | 0 fixed>}` (binary data).
fn binary_item(qualifiers: Option<&Element>) -> Result<Brace> {
    let Some(qualifiers) = qualifiers else {
        return Ok(brace_list![Brace::str("R")]);
    };
    let length = qualifier_number(qualifiers, "Length")?;
    if length == 0 {
        return Ok(brace_list![Brace::str("R")]);
    }
    Ok(brace_list![
        Brace::str("R"),
        Brace::num(length),
        Brace::num(allowed_length_flag(qualifiers)),
    ])
}

/// A value element (`<FillValue xsi:type="xs:decimal">5</FillValue>`,
/// `<MinValue xsi:nil="true"/>`, a choice parameter's `<value>`) -> its
/// typed value. An absent element is `{"U"}`.
pub fn typed_value(value: Option<&Element>, context: &DescriptorContext) -> Result<Brace> {
    let Some(value) = value else {
        return Ok(undefined());
    };
    if value.is_nil() {
        return Ok(undefined());
    }
    let text = value.text.as_str();
    match value.attr("type") {
        Some("xs:string") => Ok(brace_list![Brace::str("S"), Brace::str(native_text(text))]),
        Some("xs:boolean") => Ok(brace_list![
            Brace::str("B"),
            Brace::flag(text.trim() == "true")
        ]),
        Some("xs:decimal") => Ok(brace_list![Brace::str("N"), Brace::atom(text.trim())]),
        Some("xs:dateTime") => Ok(brace_list![
            Brace::str("D"),
            Brace::atom(date_digits(text)?)
        ]),
        Some("xr:DesignTimeRef") => design_time_ref(text, context),
        Some("xr:MDObjectRef") => metadata_ref(text, context),
        // A characteristic chart's `ValueType`: `{"#",<type description>,<pattern>}`.
        Some("v8:TypeDescription") => Ok(brace_list![
            Brace::str("#"),
            Brace::uuid(TYPE_DESCRIPTION_TYPE),
            type_pattern(Some(value), context)?,
        ]),
        // A chart of accounts' `Type`: `{"#",<account type>,<code>}`.
        Some("ent:AccountType") => {
            let code = match text.trim() {
                "Active" => 0,
                "Passive" => 1,
                "ActivePassive" => 2,
                other => bail!("unsupported account type {other}"),
            };
            Ok(brace_list![
                Brace::str("#"),
                Brace::uuid(ACCOUNT_TYPE_TYPE),
                Brace::num(code)
            ])
        }
        Some("v8:FixedArray") => {
            let mut items = vec![Brace::num(0)];
            for member in value.children_named("Value") {
                items.push(typed_value(Some(member), context)?);
            }
            items[0] = Brace::num((items.len() - 1) as i64);
            Ok(brace_list![
                Brace::str("#"),
                Brace::uuid(FIXED_ARRAY_TYPE),
                Brace::List(items)
            ])
        }
        Some(other) => bail!("unsupported value type {other}"),
        None if value.children.is_empty() && text.trim().is_empty() => Ok(undefined()),
        None => bail!("value without xsi:type: {text:?}"),
    }
}

/// `{"U"}`: Undefined.
pub fn undefined() -> Brace {
    brace_list![Brace::str("U")]
}

/// `{"S",""}`: the empty string.
pub fn empty_string_value() -> Brace {
    brace_list![Brace::str("S"), Brace::str("")]
}

/// `0001-01-01T00:00:00` -> `00010101000000`.
pub fn date_digits(text: &str) -> Result<String> {
    let digits: String = text.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() != 14 {
        bail!("bad dateTime value {text:?}");
    }
    Ok(digits)
}

/// Reference families: the metadata kind and its `...Ref` generated type
/// prefix.
const REF_FAMILIES: &[(&str, &str)] = &[
    ("Catalog", "CatalogRef"),
    ("Document", "DocumentRef"),
    ("Enum", "EnumRef"),
    ("ExchangePlan", "ExchangePlanRef"),
    (
        "ChartOfCharacteristicTypes",
        "ChartOfCharacteristicTypesRef",
    ),
    ("ChartOfAccounts", "ChartOfAccountsRef"),
    ("ChartOfCalculationTypes", "ChartOfCalculationTypesRef"),
    ("BusinessProcess", "BusinessProcessRef"),
    ("Task", "TaskRef"),
];

/// `Catalog` -> `CatalogRef`.
pub fn ref_type_prefix(kind: &str) -> Option<&'static str> {
    REF_FAMILIES
        .iter()
        .find_map(|(family, prefix)| (*family == kind).then_some(*prefix))
}

/// The inverse: `CatalogRef` -> `Catalog`.
pub fn ref_family(prefix: &str) -> Option<&'static str> {
    REF_FAMILIES
        .iter()
        .find_map(|(family, candidate)| (*candidate == prefix).then_some(*family))
}

/// `Catalog`, `X` -> the TypeId of `CatalogRef.X`.
pub fn ref_type_id(kind: &str, name: &str, context: &DescriptorContext) -> Result<String> {
    let prefix = ref_type_prefix(kind).ok_or_else(|| anyhow!("{kind} has no reference type"))?;
    generated_type_id(&format!("{prefix}.{name}"), context)
        .ok_or_else(|| anyhow!("no generated type {prefix}.{name}"))
}

pub fn is_uuid(text: &str) -> bool {
    text.len() == 36
        && text.chars().enumerate().all(|(index, ch)| match index {
            8 | 13 | 18 | 23 => ch == '-',
            _ => ch.is_ascii_hexdigit(),
        })
}

/// `{"#",5c14e26f-...,{0,<ref type id>,<value uuid>}}`.
pub fn design_time_ref(text: &str, context: &DescriptorContext) -> Result<Brace> {
    let (type_id, value_id) = design_time_ref_ids(text, context)?;
    Ok(brace_list![
        Brace::str("#"),
        Brace::uuid(DESIGN_TIME_REF_TYPE),
        brace_list![Brace::num(0), Brace::uuid(&type_id), Brace::uuid(&value_id)],
    ])
}

/// The (ref type id, value id) pair a design-time reference text names.
pub fn design_time_ref_ids(text: &str, context: &DescriptorContext) -> Result<(String, String)> {
    let text = text.trim();
    if text.is_empty() {
        return Ok((NIL_UUID.to_string(), NIL_UUID.to_string()));
    }
    if let Some((left, right)) = text.split_once('.')
        && is_uuid(left)
        && is_uuid(right)
    {
        return Ok((left.to_ascii_lowercase(), right.to_ascii_lowercase()));
    }
    let parts = text.split('.').collect::<Vec<_>>();
    if parts.len() < 3 {
        bail!("unsupported design-time reference {text}");
    }
    let (kind, name) = (parts[0], parts[1]);
    let type_id = ref_type_id(kind, name, context)?;
    let value_id = match &parts[2..] {
        ["EmptyRef"] => NIL_UUID.to_string(),
        ["EnumValue", value] => context
            .index
            .uuid_of(&format!("{kind}.{name}.EnumValue.{value}"))
            .ok_or_else(|| anyhow!("unknown enum value {text}"))?
            .to_string(),
        [item] => predefined_item_id(kind, name, item, context)?,
        _ => bail!("unsupported design-time reference {text}"),
    };
    Ok((type_id, value_id))
}

type PredefinedMap = Arc<HashMap<String, String>>;

fn predefined_cache() -> &'static Mutex<HashMap<PathBuf, PredefinedMap>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, PredefinedMap>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A predefined item of a catalog or chart by name, from the object's
/// `Ext/Predefined.xml`.
pub fn predefined_item_id(
    kind: &str,
    name: &str,
    item: &str,
    context: &DescriptorContext,
) -> Result<String> {
    let entry = context
        .index
        .objects
        .get(&format!("{kind}.{name}"))
        .ok_or_else(|| anyhow!("unknown object {kind}.{name}"))?;
    let path = entry
        .path
        .with_extension("")
        .join("Ext")
        .join("Predefined.xml");
    let cached = predefined_cache()
        .lock()
        .map_err(|_| anyhow!("predefined cache poisoned"))?
        .get(&path)
        .cloned();
    let items = match cached {
        Some(items) => items,
        None => {
            let items = Arc::new(read_predefined(&path)?);
            predefined_cache()
                .lock()
                .map_err(|_| anyhow!("predefined cache poisoned"))?
                .insert(path, items.clone());
            items
        }
    };
    items
        .get(item)
        .cloned()
        .ok_or_else(|| anyhow!("unknown predefined item {kind}.{name}.{item}"))
}

fn read_predefined(path: &PathBuf) -> Result<HashMap<String, String>> {
    fn visit(element: &Element, out: &mut HashMap<String, String>) {
        for item in element.children_named("Item") {
            if let (Some(id), Some(name)) = (item.attr("id"), item.child_text("Name")) {
                out.insert(name.to_string(), id.to_ascii_lowercase());
            }
            if let Some(children) = item.child("ChildItems") {
                visit(children, out);
            }
        }
    }
    let mut out = HashMap::new();
    let Ok(bytes) = fs::read(path) else {
        return Ok(out);
    };
    visit(&parse_element_tree(&bytes)?, &mut out);
    Ok(out)
}

/// `{"#",157fa490-...,{1,<uuid>}}`: a metadata object reference by full name.
pub fn metadata_ref(full_name: &str, context: &DescriptorContext) -> Result<Brace> {
    Ok(metadata_ref_uuid(&object_uuid(full_name, context)?))
}

/// `{"#",157fa490-...,{1,<uuid>}}` for a known uuid.
pub fn metadata_ref_uuid(uuid: &str) -> Brace {
    brace_list![
        Brace::str("#"),
        Brace::uuid(METADATA_OBJECT_REF_TYPE),
        brace_list![Brace::num(1), Brace::uuid(uuid)],
    ]
}

/// uuid of an object or a child object by full name; a bare uuid is itself.
pub fn object_uuid(full_name: &str, context: &DescriptorContext) -> Result<String> {
    let full_name = full_name.trim();
    if is_uuid(full_name) {
        return Ok(full_name.to_ascii_lowercase());
    }
    context
        .index
        .uuid_of(full_name)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("unknown metadata object {full_name}"))
}

/// uuid of a form reference (`Catalog.X.Form.F`, `CommonForm.F`), nil uuid
/// when empty.
pub fn form_uuid(full_name: &str, context: &DescriptorContext) -> Result<String> {
    let full_name = full_name.trim();
    if full_name.is_empty() {
        return Ok(NIL_UUID.to_string());
    }
    object_uuid(full_name, context)
}

/// A family's standard attributes by the code a data path or a
/// standard-attribute block names them with.
pub fn standard_attribute_codes(kind: &str) -> Option<&'static [(&'static str, i64)]> {
    Some(match kind {
        "Catalog" => crate::metadata_model::objects::CATALOG_STANDARD,
        "Document" => crate::metadata_model::objects::DOCUMENT_STANDARD,
        "ExchangePlan" => crate::metadata_model::objects::EXCHANGE_PLAN_STANDARD,
        "ChartOfCharacteristicTypes" => crate::metadata_model::objects::CCT_STANDARD,
        "ChartOfAccounts" => crate::metadata_model::objects::COA_STANDARD,
        "ChartOfCalculationTypes" => crate::metadata_model::objects::CCALC_STANDARD,
        "BusinessProcess" => crate::metadata_model::objects::BUSINESS_PROCESS_STANDARD,
        "Task" => crate::metadata_model::objects::TASK_STANDARD,
        "Enum" => crate::metadata_model::objects::ENUM_STANDARD,
        "InformationRegister" => &[
            ("Active", -5),
            ("LineNumber", -4),
            ("Recorder", -3),
            ("Period", -2),
        ],
        "AccumulationRegister" => &[
            ("RecordType", -9),
            ("Active", -5),
            ("LineNumber", -4),
            ("Recorder", -3),
            ("Period", -2),
        ],
        "AccountingRegister" => &[
            ("PeriodAdjustment", -30),
            ("Account", -10),
            ("RecordType", -9),
            ("Active", -5),
            ("LineNumber", -4),
            ("Recorder", -3),
            ("Period", -2),
        ],
        "CalculationRegister" => &[
            ("RegistrationPeriod", -13),
            ("ReversingEntry", -11),
            ("Active", -10),
            ("EndOfBasePeriod", -9),
            ("BegOfBasePeriod", -8),
            ("EndOfActionPeriod", -7),
            ("BegOfActionPeriod", -6),
            ("ActionPeriod", -5),
            ("CalculationType", -4),
            ("LineNumber", -3),
            ("Recorder", -2),
        ],
        "DocumentJournal" => &[
            ("Type", -60003),
            ("Ref", -101),
            ("Date", -100),
            ("Posted", -7),
            ("DeletionMark", -4),
            ("Number", -2),
        ],
        _ => return None,
    })
}

/// The negative code a data path names a standard attribute by, per owner
/// kind (`LineNumber` of a tabular section: the family's marker).
pub fn standard_attribute_code(owner_kind: &str, name: &str) -> Option<i64> {
    standard_attribute_codes(owner_kind)?
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, code)| *code)
}

/// An accounting register's `ExtDimensionN` / `ExtDimensionTypeN` standard
/// attribute: `{N-1,<class>}`.
fn ext_dimension_segment(owner_kind: &str, name: &str) -> Option<Brace> {
    const EXT_DIMENSION: &str = "91162600-3161-4326-89a0-4a7cecd5092a";
    const EXT_DIMENSION_TYPE: &str = "b3b48b29-d652-47ab-9d21-7e06768c31b5";
    if owner_kind != "AccountingRegister" {
        return None;
    }
    let (class, number) = match name.strip_prefix("ExtDimensionType") {
        Some(number) => (EXT_DIMENSION_TYPE, number),
        None => (EXT_DIMENSION, name.strip_prefix("ExtDimension")?),
    };
    let number: i64 = number.parse().ok()?;
    Some(brace_list![Brace::num(number - 1), Brace::uuid(class)])
}

/// A field data path -> its segments.
pub fn data_path(text: &str, context: &DescriptorContext) -> Result<Vec<Brace>> {
    let text = text.trim();
    if text == "0" || text.is_empty() {
        return Ok(vec![brace_list![Brace::num(0)]]);
    }
    if let Ok(code) = text.parse::<i64>() {
        return Ok(vec![brace_list![Brace::num(code)]]);
    }
    if text.contains('/') {
        let mut segments = Vec::new();
        for segment in text.split('/') {
            segments.extend(data_path(segment, context)?);
        }
        return Ok(segments);
    }
    if let Some((kind, uuid)) = text.split_once(':') {
        if !is_uuid(uuid) {
            bail!("unsupported data path segment {text:?}");
        }
        return Ok(vec![brace_list![Brace::atom(kind), Brace::uuid(uuid)]]);
    }
    let parts = text.split('.').collect::<Vec<_>>();
    if parts.len() < 2 || parts.len() % 2 != 0 {
        bail!("unsupported data path {text}");
    }
    let owner = format!("{}.{}", parts[0], parts[1]);
    if parts.len() == 2 {
        return Ok(vec![brace_list![
            Brace::num(0),
            Brace::uuid(&object_uuid(&owner, context)?)
        ]]);
    }
    let mut segments = Vec::new();
    let mut prefix = owner;
    let owner_kind = parts[0];
    let mut in_section = false;
    for pair in parts[2..].chunks(2) {
        let (member_kind, member) = (pair[0], pair[1]);
        if member_kind == "StandardAttribute" {
            if let Some(segment) = ext_dimension_segment(owner_kind, member) {
                segments.push(segment);
                continue;
            }
            let code = if in_section {
                (member == "LineNumber")
                    .then(|| crate::metadata_model::objects::line_number_marker(owner_kind))
            } else {
                standard_attribute_code(owner_kind, member)
            }
            .ok_or_else(|| {
                anyhow!("unknown standard attribute {member} of {owner_kind} in {text}")
            })?;
            segments.push(brace_list![Brace::num(code)]);
        } else {
            prefix = format!("{prefix}.{member_kind}.{member}");
            segments.push(brace_list![
                Brace::num(0),
                Brace::uuid(&object_uuid(&prefix, context)?)
            ]);
            if member_kind == "TabularSection" {
                in_section = true;
            }
        }
    }
    Ok(segments)
}

/// Offline measurement of [`type_pattern`] against every type description of
/// a tree: each `<Type>` (any element holding `v8:Type` / `v8:TypeSet` /
/// `v8:TypeId` children) of a metadata XML is compiled and looked up in its
/// object's stored row; each one of an owned form's `Ext/Form.xml` in the
/// form body row (`<form uuid>.0`).
pub mod corpus {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use anyhow::Result;
    use rayon::prelude::*;
    use serde::Serialize;

    use super::super::DescriptorContext;
    use super::super::audit::{descriptor_xmls, read_stored_row};
    use super::super::brace::{parse_row, serialize};
    use super::super::xml::{Element, MetadataXml, parse_element_tree};
    use super::type_pattern;
    use crate::parallel;

    #[derive(Debug, Default, Serialize)]
    pub struct PatternAuditReport {
        pub total: usize,
        pub found: usize,
        pub missing: usize,
        pub failed: usize,
        /// By where the description sits (`Catalog`, `Form`) and why.
        pub missing_by: BTreeMap<String, usize>,
        pub failures: BTreeMap<String, usize>,
        pub samples: Vec<PatternSample>,
    }

    #[derive(Debug, Serialize)]
    pub struct PatternSample {
        pub file: String,
        pub types: String,
        pub compiled: String,
    }

    fn is_description(element: &Element) -> bool {
        element.children.iter().any(|child| {
            child.prefix == "v8" && matches!(child.name.as_str(), "Type" | "TypeSet" | "TypeId")
        }) && element
            .children
            .iter()
            .all(|child| child.children.is_empty() || child.name.ends_with("Qualifiers"))
    }

    fn descriptions<'a>(element: &'a Element, out: &mut Vec<&'a Element>) {
        for child in &element.children {
            if is_description(child) {
                out.push(child);
            } else {
                descriptions(child, out);
            }
        }
    }

    fn summary(element: &Element) -> String {
        element
            .children
            .iter()
            .map(|child| format!("{}={}", child.name, child.text.trim()))
            .collect::<Vec<_>>()
            .join(",")
    }

    enum Outcome {
        Found,
        Missing(String, String, String, String),
        Failed(String),
    }

    fn check(
        relative: &str,
        kind: &str,
        element: &Element,
        stored: &str,
        context: &DescriptorContext,
    ) -> Outcome {
        match type_pattern(Some(element), context) {
            Err(error) => Outcome::Failed(format!("{error:#}")),
            Ok(pattern) => {
                let text = serialize(&pattern);
                if stored.contains(&text) {
                    Outcome::Found
                } else {
                    Outcome::Missing(
                        kind.to_string(),
                        relative.to_string(),
                        summary(element),
                        text,
                    )
                }
            }
        }
    }

    fn measure_object(
        root: &Path,
        rows: &Path,
        path: &Path,
        context: &DescriptorContext,
    ) -> Result<Vec<Outcome>> {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        let Ok(bytes) = fs::read(path) else {
            return Ok(Vec::new());
        };
        let Ok(doc) = MetadataXml::parse(&bytes) else {
            return Ok(Vec::new());
        };
        let Ok(object) = doc.object() else {
            return Ok(Vec::new());
        };
        let Some(uuid) = object.attr("uuid").map(str::to_ascii_lowercase) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        let mut found = Vec::new();
        descriptions(object, &mut found);
        if !found.is_empty()
            && let Some(stored) = read_stored_row(rows, &uuid)?
            && let Ok(tree) = parse_row(&stored)
        {
            let stored = serialize(&tree);
            for element in found {
                out.push(check(&relative, &object.name, element, &stored, context));
            }
        }
        // An owned form's body: `<...>/Forms/F.xml` + `<...>/Forms/F/Ext/Form.xml`.
        if object.name == "Form" || object.name == "CommonForm" {
            let form = path.with_extension("").join("Ext").join("Form.xml");
            if let Ok(bytes) = fs::read(&form)
                && let Ok(body) = parse_element_tree(&bytes)
            {
                let mut found = Vec::new();
                descriptions(&body, &mut found);
                if !found.is_empty()
                    && let Some(stored) = read_stored_row(rows, &format!("{uuid}.0"))?
                    && let Ok(tree) = parse_row(&stored)
                {
                    let stored = serialize(&tree);
                    for element in found {
                        out.push(check(&relative, "Form.xml", element, &stored, context));
                    }
                }
            }
        }
        Ok(out)
    }

    pub fn audit(
        root: &Path,
        rows: &Path,
        version: &str,
        max_samples: usize,
    ) -> Result<PatternAuditReport> {
        let context = DescriptorContext::new(root, version)?;
        // Owned forms' descriptors sit next to their bodies, so
        // `descriptor_xmls` already yields everything measured here.
        let paths = descriptor_xmls(root);
        let outcomes = parallel::install(|| {
            paths
                .par_iter()
                .map(|path| measure_object(root, rows, path, &context))
                .collect::<Result<Vec<_>>>()
        })??;
        let mut report = PatternAuditReport::default();
        for outcome in outcomes.into_iter().flatten() {
            report.total += 1;
            match outcome {
                Outcome::Found => report.found += 1,
                Outcome::Failed(message) => {
                    report.failed += 1;
                    let key: String = message.chars().take(160).collect();
                    *report.failures.entry(key).or_default() += 1;
                }
                Outcome::Missing(kind, file, types, compiled) => {
                    report.missing += 1;
                    *report.missing_by.entry(kind).or_default() += 1;
                    if report.samples.len() < max_samples {
                        report.samples.push(PatternSample {
                            file,
                            types,
                            compiled,
                        });
                    }
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::brace::serialize;

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
    use super::super::index::{ConfigIndex, GeneratedType};
    use super::super::xml::parse_element_tree;
    use super::*;
    use crate::module_blob::MetadataSourceContext;

    pub(crate) const VALUTA_TYPE: &str = "41dbb66b-ff77-4b8f-aaf8-0ca2012d3a6f";
    pub(crate) const ENUM_TYPE: &str = "31af3c5b-3472-4ad9-86b2-453cc31d397e";
    pub(crate) const ENUM_VALUE: &str = "b8b7b82f-9a9b-4688-8045-dcfa3d202fa6";
    pub(crate) const ATTRIBUTE: &str = "9e7b0920-3d4f-481a-9dce-a789a4e9ef73";
    pub(crate) const SECTION: &str = "34c16381-7743-4b26-a54e-6b35290ab531";
    pub(crate) const SECTION_ATTRIBUTE: &str = "d1280594-c1ac-4b9a-b5ff-9a768102422a";

    /// A context whose index knows a handful of names.
    pub(crate) fn context() -> DescriptorContext {
        let mut index = ConfigIndex::default();
        for (name, type_id) in [
            ("CatalogRef.Валюты", VALUTA_TYPE),
            ("EnumRef.Виды", ENUM_TYPE),
            ("DefinedType.Сумма", "5ddef559-eb73-4233-842e-6548a5404b56"),
        ] {
            index.generated_types.insert(
                name.to_string(),
                GeneratedType {
                    name: name.to_string(),
                    category: String::new(),
                    type_id: type_id.to_string(),
                    value_id: NIL_UUID.to_string(),
                },
            );
        }
        for (name, uuid) in [
            ("Enum.Виды.EnumValue.Первый", ENUM_VALUE),
            ("Catalog.Счета.Attribute.Банк", ATTRIBUTE),
            ("Catalog.Счета.TabularSection.Строки", SECTION),
            (
                "Catalog.Счета.TabularSection.Строки.Attribute.Свойство",
                SECTION_ATTRIBUTE,
            ),
        ] {
            index.children.insert(name.to_string(), uuid.to_string());
        }
        DescriptorContext {
            root: PathBuf::from("."),
            index,
            source: MetadataSourceContext::new(PathBuf::from(".")),
            version: "2.20".to_string(),
        }
    }

    pub(crate) fn element(xml: &str) -> Element {
        parse_element_tree(xml.as_bytes()).unwrap()
    }

    fn text(brace: Result<Brace>) -> String {
        serialize(&brace.unwrap()).replace("\r\n", "")
    }

    #[test]
    fn dates_uuids_and_builtins() {
        assert_eq!(
            date_digits("0001-01-01T00:00:00").unwrap(),
            "00010101000000"
        );
        assert!(is_uuid("5c14e26f-099b-4d37-84a6-b433d87400da"));
        assert!(!is_uuid("Catalog.X"));
        assert_eq!(
            builtin_type_id("d7p1:Chart"),
            Some("3543ef08-3316-4f7e-9447-0cd0a1cbf1d5")
        );
        assert_eq!(
            builtin_type_id("cfg:CatalogRef"),
            Some("e61ef7b8-f3e1-4f4b-8ac7-676e90524997")
        );
        assert_eq!(standard_attribute_code("Catalog", "Owner"), Some(-5));
        assert_eq!(standard_attribute_code("Document", "Date"), Some(-3));
    }

    #[test]
    fn patterns_sort_by_type_id_with_qualified_primitives() {
        let context = context();
        let description = element(
            r##"<Type xmlns:v8="v8"><v8:Type>xs:string</v8:Type><v8:Type>xs:boolean</v8:Type><v8:TypeSet>cfg:CatalogRef</v8:TypeSet><v8:Type>cfg:CatalogRef.Валюты</v8:Type><v8:Type>xs:decimal</v8:Type><v8:Type>xs:dateTime</v8:Type><v8:StringQualifiers><v8:Length>10</v8:Length><v8:AllowedLength>Fixed</v8:AllowedLength></v8:StringQualifiers><v8:NumberQualifiers><v8:Digits>15</v8:Digits><v8:FractionDigits>2</v8:FractionDigits><v8:AllowedSign>Nonnegative</v8:AllowedSign></v8:NumberQualifiers><v8:DateQualifiers><v8:DateFractions>Date</v8:DateFractions></v8:DateQualifiers></Type>"##,
        );
        assert_eq!(
            text(type_pattern(Some(&description), &context)),
            format!(
                r##"{{"Pattern",{{"#",{VALUTA_TYPE}}},{{"B"}},{{"S",10,0}},{{"D","D"}},{{"N",15,2,1}},{{"#",e61ef7b8-f3e1-4f4b-8ac7-676e90524997}}}}"##
            )
        );
        let unlimited = element(
            r##"<Type><v8:Type>xs:string</v8:Type><v8:Type>xs:decimal</v8:Type><v8:StringQualifiers><v8:Length>0</v8:Length><v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers><v8:NumberQualifiers><v8:Digits>0</v8:Digits><v8:FractionDigits>0</v8:FractionDigits><v8:AllowedSign>Any</v8:AllowedSign></v8:NumberQualifiers></Type>"##,
        );
        assert_eq!(
            text(type_pattern(Some(&unlimited), &context)),
            r##"{"Pattern",{"S"},{"N"}}"##
        );
        let defined = element(r##"<Type><v8:TypeSet>cfg:DefinedType.Сумма</v8:TypeSet></Type>"##);
        assert_eq!(
            text(type_pattern(Some(&defined), &context)),
            r##"{"Pattern",{"#",5ddef559-eb73-4233-842e-6548a5404b56}}"##
        );
        assert_eq!(text(type_pattern(None, &context)), r##"{"Pattern"}"##);
    }

    #[test]
    fn typed_values() {
        let context = context();
        let value = |xml: &str| text(typed_value(Some(&element(xml)), &context));
        assert_eq!(value(r##"<FillValue xsi:nil="true"/>"##), r##"{"U"}"##);
        assert_eq!(
            value(r##"<FillValue xsi:type="xs:string"/>"##),
            r##"{"S",""}"##
        );
        assert_eq!(
            serialize(
                &typed_value(
                    Some(&element("<V xsi:type=\"xs:string\">a\nb</V>")),
                    &context
                )
                .unwrap()
            ),
            "{\"S\",\"a\r\nb\"}"
        );
        assert_eq!(
            value(r##"<V xsi:type="xs:decimal">0.5</V>"##),
            r##"{"N",0.5}"##
        );
        assert_eq!(
            value(r##"<V xsi:type="xs:boolean">true</V>"##),
            r##"{"B",1}"##
        );
        assert_eq!(
            value(r##"<V xsi:type="xs:dateTime">0001-01-01T23:59:59</V>"##),
            r##"{"D",00010101235959}"##
        );
        assert_eq!(
            value(r##"<V xsi:type="xr:DesignTimeRef">Catalog.Валюты.EmptyRef</V>"##),
            format!(r##"{{"#",{DESIGN_TIME_REF_TYPE},{{0,{VALUTA_TYPE},{NIL_UUID}}}}}"##)
        );
        assert_eq!(
            value(r##"<V xsi:type="xr:DesignTimeRef"/>"##),
            format!(r##"{{"#",{DESIGN_TIME_REF_TYPE},{{0,{NIL_UUID},{NIL_UUID}}}}}"##)
        );
        assert_eq!(
            value(
                r##"<V xsi:type="v8:FixedArray"><v8:Value xsi:type="xr:DesignTimeRef">Enum.Виды.EnumValue.Первый</v8:Value><v8:Value xsi:type="xs:decimal">30</v8:Value></V>"##
            ),
            format!(
                r##"{{"#",{FIXED_ARRAY_TYPE},{{2,{{"#",{DESIGN_TIME_REF_TYPE},{{0,{ENUM_TYPE},{ENUM_VALUE}}}}},{{"N",30}}}}}}"##
            )
        );
    }

    #[test]
    fn data_paths() {
        let context = context();
        let path = |text: &str| {
            data_path(text, &context)
                .unwrap()
                .iter()
                .map(serialize)
                .collect::<Vec<_>>()
                .join("|")
        };
        assert_eq!(
            path("Catalog.Счета.Attribute.Банк"),
            format!("{{0,{ATTRIBUTE}}}")
        );
        assert_eq!(
            path("Catalog.Счета.TabularSection.Строки.Attribute.Свойство"),
            format!("{{0,{SECTION}}}|{{0,{SECTION_ATTRIBUTE}}}")
        );
        assert_eq!(path("Catalog.Счета.StandardAttribute.Owner"), "{-5}");
        assert_eq!(path("Document.Акт.StandardAttribute.Date"), "{-3}");
        assert_eq!(
            path("AccountingRegister.Хозрасчетный.StandardAttribute.ExtDimension2"),
            "{1,91162600-3161-4326-89a0-4a7cecd5092a}"
        );
        assert_eq!(path("-8"), "{-8}");
        assert_eq!(path("0"), "{0}");
        assert_eq!(
            path(&format!("0:{SECTION}/0:{ATTRIBUTE}")),
            format!("{{0,{SECTION}}}|{{0,{ATTRIBUTE}}}")
        );
    }
}
