//! Base-free codecs and fail-closed passthrough for `Ext/Predefined.xml` bodies.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ibcmd_core::artifact::ProfileId;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_core::profile::EffectiveProfile;

use super::{BodyProfileError, SelectedBodyProfile};
use crate::compiler::families::native::{
    NativeError, NativeValue, exact_list, exact_token, formatted_list, inflate_and_parse,
    inline_list, parse_bool_token, raw_deflate, required_list, required_text, required_token,
    serialize, text, token,
};

const LAYOUT_KEY: &str = "bootstrap.body.predefined.layout";
const LAYOUT: &str = "predefined-v1-raw-deflate-utf8-bom";
const PREDEFINED_TYPE_UUID: &str = "ae135932-4f94-44df-92c1-c91f15a92848";
const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
const MAX_ITEMS: usize = 1_000_000;
const MAX_STRING_LENGTH: u32 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PredefinedFamily {
    Catalog,
    ChartOfCharacteristicTypes,
    ChartOfAccounts,
    ChartOfCalculationTypes,
}

impl PredefinedFamily {
    const fn root_marker(self) -> &'static str {
        match self {
            Self::Catalog => "0",
            Self::ChartOfCharacteristicTypes => "1",
            Self::ChartOfAccounts => "2",
            Self::ChartOfCalculationTypes => "9",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredefinedCodecProfile(SelectedBodyProfile);

impl PredefinedCodecProfile {
    pub fn from_effective(profile: &EffectiveProfile) -> Result<Self, BodyProfileError> {
        SelectedBodyProfile::from_effective(profile, LAYOUT_KEY, LAYOUT).map(Self)
    }

    pub const fn profile_id(&self) -> &ProfileId {
        self.0.profile_id()
    }

    #[cfg(test)]
    fn fixture() -> Self {
        Self(SelectedBodyProfile::fixture("platform-8.3.27.1989"))
    }
}

/// Validated native data. Bodies decoded from storage retain their source
/// profile and may only be passed through to that exact profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredefinedBody {
    native: NativeValue,
    family: PredefinedFamily,
    source_profile: ProfileId,
    original_blob: Option<Vec<u8>>,
}

impl PredefinedBody {
    pub const fn family(&self) -> PredefinedFamily {
        self.family
    }

    pub const fn source_profile(&self) -> &ProfileId {
        &self.source_profile
    }

    pub fn plaintext(&self) -> Result<Vec<u8>, PredefinedCodecError> {
        serialize(&self.native).map_err(Into::into)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogPredefinedLayout {
    pub code_length: u32,
    pub description_length: u32,
    pub synthetic_root_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogPredefinedData {
    pub layout: CatalogPredefinedLayout,
    pub items: Vec<CatalogPredefinedItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogPredefinedItem {
    pub id: ObjectUuid,
    pub name: String,
    pub code: String,
    pub description: String,
    pub is_folder: bool,
    pub children: Vec<CatalogPredefinedItem>,
}

pub fn decode_predefined(
    profile: &PredefinedCodecProfile,
    family: PredefinedFamily,
    blob: &[u8],
) -> Result<PredefinedBody, PredefinedCodecError> {
    let native = inflate_and_parse(blob).map_err(PredefinedCodecError::from)?;
    validate_family_root(&native, family)?;
    Ok(PredefinedBody {
        native,
        family,
        source_profile: profile.profile_id().clone(),
        original_blob: Some(blob.to_vec()),
    })
}

/// Re-emits an unchanged native body only for its exact source profile.
/// This is the preservation path for chart layouts not yet represented by
/// the typed Catalog model.
pub fn encode_predefined(
    profile: &PredefinedCodecProfile,
    body: &PredefinedBody,
) -> Result<Vec<u8>, PredefinedCodecError> {
    if profile.profile_id() != body.source_profile() {
        return Err(PredefinedCodecError::OpaqueProfileMismatch {
            source: body.source_profile().clone(),
            target: profile.profile_id().clone(),
            family: body.family,
        });
    }
    match &body.original_blob {
        Some(blob) => Ok(blob.clone()),
        None => raw_deflate(&body.native).map_err(Into::into),
    }
}

pub fn compile_catalog_predefined(
    profile: &PredefinedCodecProfile,
    model: &CatalogPredefinedData,
) -> Result<Vec<u8>, PredefinedCodecError> {
    validate_catalog_model(model)?;
    let mut row_index = 1usize;
    let children = native_item_list(&model.items, &mut row_index)?;
    let synthetic_values = if model.layout.code_length == 0 {
        vec![
            uuid_value(NIL_UUID),
            bool_value(true),
            uuid_value(NIL_UUID),
            string_value(&model.layout.synthetic_root_name),
            string_value(""),
        ]
    } else {
        vec![
            uuid_value(NIL_UUID),
            bool_value(true),
            uuid_value(NIL_UUID),
            string_value(&model.layout.synthetic_root_name),
        ]
    };
    let mut synthetic = vec![
        token("2"),
        token("0"),
        token(synthetic_values.len().to_string()),
    ];
    synthetic.extend(synthetic_values);
    synthetic.push(token("1"));
    synthetic.push(children);

    let root_list = formatted_list(
        vec![
            token("1"),
            token("1"),
            formatted_list(synthetic, false, vec![3], false),
        ],
        false,
        vec![2],
        false,
    );
    let mut rowset = vec![token("2"), token("7")];
    for index in 0..7 {
        rowset.push(token(index.to_string()));
        rowset.push(token(index.to_string()));
    }
    rowset.push(root_list);
    let table = formatted_list(
        vec![
            token("1"),
            catalog_schema(&model.layout),
            formatted_list(rowset, false, vec![16], false),
            token("-1"),
            token("1"),
        ],
        false,
        vec![1, 2],
        false,
    );
    let native = formatted_list(
        vec![token(PredefinedFamily::Catalog.root_marker()), table],
        false,
        vec![1],
        false,
    );
    let body = PredefinedBody {
        native,
        family: PredefinedFamily::Catalog,
        source_profile: profile.profile_id().clone(),
        original_blob: None,
    };
    encode_predefined(profile, &body)
}

pub fn decode_catalog_predefined(
    profile: &PredefinedCodecProfile,
    blob: &[u8],
    layout: CatalogPredefinedLayout,
) -> Result<CatalogPredefinedData, PredefinedCodecError> {
    let body = decode_predefined(profile, PredefinedFamily::Catalog, blob)?;
    let root = exact_list(&body.native, 2, "Catalog Predefined root")?;
    let table = exact_list(&root[1], 5, "Catalog Predefined table")?;
    exact_token(&table[0], "1", "Catalog Predefined table marker")?;
    exact_token(&table[3], "-1", "Catalog Predefined table tail")?;
    exact_token(&table[4], "1", "Catalog Predefined table tail")?;
    validate_catalog_schema(&table[1], &layout)?;
    let rowset = exact_list(&table[2], 17, "Catalog Predefined rowset")?;
    exact_token(&rowset[0], "2", "Catalog Predefined rowset marker")?;
    exact_token(&rowset[1], "7", "Catalog Predefined column count")?;
    for index in 0..7 {
        exact_token(
            &rowset[2 + index * 2],
            &index.to_string(),
            "Catalog Predefined column offset",
        )?;
        exact_token(
            &rowset[3 + index * 2],
            &index.to_string(),
            "Catalog Predefined column id",
        )?;
    }
    let roots = exact_list(&rowset[16], 3, "Catalog Predefined root list")?;
    exact_token(&roots[0], "1", "Catalog Predefined list marker")?;
    exact_token(&roots[1], "1", "Catalog Predefined synthetic root count")?;
    let synthetic = required_list(&roots[2], "Catalog Predefined synthetic root")?;
    if synthetic.len() < 8 {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined synthetic root is truncated",
        ));
    }
    exact_token(&synthetic[0], "2", "Catalog Predefined row marker")?;
    exact_token(&synthetic[1], "0", "Catalog Predefined synthetic row index")?;
    let value_count = parse_count(&synthetic[2], "Catalog Predefined synthetic value count")?;
    let child_marker = 3usize
        .checked_add(value_count)
        .ok_or(PredefinedCodecError::CountOverflow)?;
    if synthetic.len() != child_marker.saturating_add(2) {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined synthetic row has unsupported fields",
        ));
    }
    validate_synthetic_root(&synthetic[3..child_marker], &layout)?;
    exact_token(
        &synthetic[child_marker],
        "1",
        "Catalog Predefined synthetic child marker",
    )?;
    let mut expected_index = 1usize;
    let items = parse_item_list(&synthetic[child_marker + 1], &mut expected_index)?;
    let model = CatalogPredefinedData { layout, items };
    validate_catalog_model(&model)?;
    Ok(model)
}

fn validate_synthetic_root(
    values: &[NativeValue],
    layout: &CatalogPredefinedLayout,
) -> Result<(), PredefinedCodecError> {
    let expected_count = if layout.code_length == 0 { 5 } else { 4 };
    if values.len() != expected_count {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined synthetic root value count",
        ));
    }
    if parse_uuid_value(&values[0])?.to_string() != NIL_UUID {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined synthetic UUID is non-zero",
        ));
    }
    if !parse_typed_bool(&values[1])? {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined synthetic root is not a folder",
        ));
    }
    if parse_uuid_value(&values[2])?.to_string() != NIL_UUID {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined synthetic parent reference is non-zero",
        ));
    }
    if parse_typed_string(&values[3])? != layout.synthetic_root_name {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined synthetic root name differs from the selected layout",
        ));
    }
    if values.len() == 5 && !parse_typed_string(&values[4])?.is_empty() {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined synthetic root code is non-empty",
        ));
    }
    Ok(())
}

fn validate_family_root(
    native: &NativeValue,
    family: PredefinedFamily,
) -> Result<(), PredefinedCodecError> {
    let fields = required_list(native, "Predefined root")?;
    if fields.len() < 2 {
        return Err(PredefinedCodecError::InvalidShape(
            "Predefined root is truncated",
        ));
    }
    exact_token(&fields[0], family.root_marker(), "Predefined family marker")?;
    required_list(&fields[1], "Predefined native table")?;
    Ok(())
}

fn validate_catalog_model(model: &CatalogPredefinedData) -> Result<(), PredefinedCodecError> {
    if model.layout.code_length > MAX_STRING_LENGTH
        || model.layout.description_length > MAX_STRING_LENGTH
    {
        return Err(PredefinedCodecError::InvalidModel(
            "Catalog string length exceeds the standalone bound",
        ));
    }
    if model.layout.synthetic_root_name.is_empty() {
        return Err(PredefinedCodecError::InvalidModel(
            "Catalog synthetic root name is empty",
        ));
    }
    let mut count = 0usize;
    let mut ids = BTreeSet::new();
    validate_items(&model.items, &model.layout, &mut count, &mut ids)
}

fn validate_items(
    items: &[CatalogPredefinedItem],
    layout: &CatalogPredefinedLayout,
    count: &mut usize,
    ids: &mut BTreeSet<ObjectUuid>,
) -> Result<(), PredefinedCodecError> {
    for item in items {
        *count = count
            .checked_add(1)
            .ok_or(PredefinedCodecError::CountOverflow)?;
        if *count > MAX_ITEMS {
            return Err(PredefinedCodecError::InvalidModel(
                "Catalog Predefined item count exceeds the standalone bound",
            ));
        }
        if item.name.is_empty() {
            return Err(PredefinedCodecError::InvalidModel(
                "Catalog Predefined item name is empty",
            ));
        }
        if item.id.to_string() == NIL_UUID {
            return Err(PredefinedCodecError::InvalidModel(
                "Catalog Predefined item UUID is nil",
            ));
        }
        if !ids.insert(item.id) {
            return Err(PredefinedCodecError::InvalidModel(
                "Catalog Predefined item UUID is duplicated",
            ));
        }
        if layout.code_length == 0 && !item.code.is_empty() {
            return Err(PredefinedCodecError::InvalidModel(
                "Catalog Predefined code is present while CodeLength is zero",
            ));
        }
        if layout.code_length != 0 && item.code.chars().count() > layout.code_length as usize {
            return Err(PredefinedCodecError::InvalidModel(
                "Catalog Predefined code exceeds CodeLength",
            ));
        }
        if layout.description_length == 0 && !item.description.is_empty() {
            return Err(PredefinedCodecError::InvalidModel(
                "Catalog Predefined description is present while DescriptionLength is zero",
            ));
        }
        if layout.description_length != 0
            && item.description.chars().count() > layout.description_length as usize
        {
            return Err(PredefinedCodecError::InvalidModel(
                "Catalog Predefined description exceeds DescriptionLength",
            ));
        }
        if !item.is_folder && !item.children.is_empty() {
            return Err(PredefinedCodecError::InvalidModel(
                "non-folder Catalog Predefined item has children",
            ));
        }
        validate_items(&item.children, layout, count, ids)?;
    }
    Ok(())
}

fn catalog_schema(layout: &CatalogPredefinedLayout) -> NativeValue {
    let patterns = [
        uuid_pattern(),
        inline_list(vec![text("B")]),
        uuid_pattern(),
        string_pattern(0),
        string_pattern(layout.code_length),
        string_pattern(layout.description_length),
        inline_list(vec![text("N")]),
    ];
    let mut values = Vec::with_capacity(8);
    values.push(token("7"));
    for (index, pattern) in patterns.into_iter().enumerate() {
        values.push(formatted_list(
            vec![
                token(index.to_string()),
                text(""),
                formatted_list(vec![text("Pattern"), pattern], false, vec![1], true),
                text(""),
                token("0"),
            ],
            false,
            vec![2],
            false,
        ));
    }
    formatted_list(values, false, (1..8).collect(), true)
}

fn validate_catalog_schema(
    value: &NativeValue,
    layout: &CatalogPredefinedLayout,
) -> Result<(), PredefinedCodecError> {
    let actual = required_list(value, "Catalog Predefined schema")?;
    let expected = catalog_schema(layout);
    let expected = required_list(&expected, "Catalog Predefined expected schema")?;
    if !native_semantically_equal(actual, expected) {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined schema differs from the selected layout",
        ));
    }
    Ok(())
}

fn native_semantically_equal(left: &[NativeValue], right: &[NativeValue]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (NativeValue::Token(left), NativeValue::Token(right)) => left == right,
                (NativeValue::Text(left), NativeValue::Text(right)) => left == right,
                (
                    NativeValue::List { values: left, .. },
                    NativeValue::List { values: right, .. },
                ) => native_semantically_equal(left, right),
                _ => false,
            })
}

fn uuid_pattern() -> NativeValue {
    inline_list(vec![text("#"), token(PREDEFINED_TYPE_UUID)])
}

fn string_pattern(length: u32) -> NativeValue {
    if length == 0 {
        inline_list(vec![text("S")])
    } else {
        inline_list(vec![text("S"), token(length.to_string()), token("1")])
    }
}

fn uuid_value(uuid: &str) -> NativeValue {
    formatted_list(
        vec![
            text("#"),
            token(PREDEFINED_TYPE_UUID),
            inline_list(vec![token("1"), token(uuid)]),
        ],
        false,
        vec![2],
        false,
    )
}

fn bool_value(value: bool) -> NativeValue {
    inline_list(vec![text("B"), token(if value { "1" } else { "0" })])
}

fn string_value(value: &str) -> NativeValue {
    inline_list(vec![text("S"), text(value)])
}

fn number_zero_value() -> NativeValue {
    inline_list(vec![text("N"), token("0")])
}

fn native_item_list(
    items: &[CatalogPredefinedItem],
    row_index: &mut usize,
) -> Result<NativeValue, PredefinedCodecError> {
    let mut values = Vec::with_capacity(items.len() + 2);
    values.push(token("1"));
    values.push(token(items.len().to_string()));
    for item in items {
        let index = *row_index;
        *row_index = row_index
            .checked_add(1)
            .ok_or(PredefinedCodecError::CountOverflow)?;
        let children = native_item_list(&item.children, row_index)?;
        let mut row = vec![
            token("2"),
            token(index.to_string()),
            token("7"),
            uuid_value(&item.id.to_string()),
            bool_value(item.is_folder),
            uuid_value(NIL_UUID),
            string_value(&item.name),
            string_value(&item.code),
            string_value(&item.description),
            number_zero_value(),
        ];
        if item.children.is_empty() {
            row.push(token("0"));
        } else {
            row.push(token("1"));
            row.push(children);
        }
        values.push(formatted_list(row, false, vec![3], false));
    }
    Ok(if items.is_empty() {
        inline_list(values)
    } else {
        formatted_list(values, false, vec![2], false)
    })
}

fn parse_item_list(
    value: &NativeValue,
    expected_index: &mut usize,
) -> Result<Vec<CatalogPredefinedItem>, PredefinedCodecError> {
    let fields = required_list(value, "Catalog Predefined item list")?;
    if fields.len() < 2 {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined item list is truncated",
        ));
    }
    exact_token(&fields[0], "1", "Catalog Predefined item list marker")?;
    let count = parse_count(&fields[1], "Catalog Predefined item count")?;
    if fields.len() != count.saturating_add(2) {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined item count differs from list length",
        ));
    }
    let mut items = Vec::with_capacity(count);
    for value in &fields[2..] {
        items.push(parse_item(value, expected_index)?);
    }
    Ok(items)
}

fn parse_item(
    value: &NativeValue,
    expected_index: &mut usize,
) -> Result<CatalogPredefinedItem, PredefinedCodecError> {
    let fields = required_list(value, "Catalog Predefined item")?;
    if fields.len() < 11 {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined item is truncated",
        ));
    }
    exact_token(&fields[0], "2", "Catalog Predefined row marker")?;
    exact_token(
        &fields[1],
        &expected_index.to_string(),
        "Catalog Predefined row index",
    )?;
    *expected_index = expected_index
        .checked_add(1)
        .ok_or(PredefinedCodecError::CountOverflow)?;
    exact_token(&fields[2], "7", "Catalog Predefined value count")?;
    let id = parse_uuid_value(&fields[3])?;
    let is_folder = parse_typed_bool(&fields[4])?;
    let actual_parent = parse_uuid_value(&fields[5])?;
    if actual_parent.to_string() != NIL_UUID {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined reserved parent reference is non-zero",
        ));
    }
    let name = parse_typed_string(&fields[6])?;
    let code = parse_typed_string(&fields[7])?;
    let description = parse_typed_string(&fields[8])?;
    let number = exact_list(&fields[9], 2, "Catalog Predefined reserved number")?;
    if required_text(&number[0], "Catalog Predefined number marker")? != "N" {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined number marker",
        ));
    }
    exact_token(&number[1], "0", "Catalog Predefined reserved number")?;
    let child_marker = required_token(&fields[10], "Catalog Predefined child marker")?;
    let children = match child_marker {
        "0" if fields.len() == 11 => Vec::new(),
        "1" if fields.len() == 12 => parse_item_list(&fields[11], expected_index)?,
        _ => {
            return Err(PredefinedCodecError::InvalidShape(
                "Catalog Predefined child layout is unsupported",
            ));
        }
    };
    if !is_folder && !children.is_empty() {
        return Err(PredefinedCodecError::InvalidShape(
            "non-folder Catalog Predefined item has children",
        ));
    }
    Ok(CatalogPredefinedItem {
        id,
        name,
        code,
        description,
        is_folder,
        children,
    })
}

fn parse_uuid_value(value: &NativeValue) -> Result<ObjectUuid, PredefinedCodecError> {
    let fields = exact_list(value, 3, "Catalog Predefined UUID value")?;
    if required_text(&fields[0], "Catalog Predefined UUID marker")? != "#" {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined UUID marker",
        ));
    }
    exact_token(
        &fields[1],
        PREDEFINED_TYPE_UUID,
        "Catalog Predefined UUID type",
    )?;
    let reference = exact_list(&fields[2], 2, "Catalog Predefined UUID reference")?;
    exact_token(
        &reference[0],
        "1",
        "Catalog Predefined UUID reference marker",
    )?;
    let uuid = required_token(&reference[1], "Catalog Predefined UUID")?;
    ObjectUuid::parse(uuid).map_err(|_| PredefinedCodecError::InvalidUuid(uuid.to_owned()))
}

fn parse_typed_bool(value: &NativeValue) -> Result<bool, PredefinedCodecError> {
    let fields = exact_list(value, 2, "Catalog Predefined boolean")?;
    if required_text(&fields[0], "Catalog Predefined boolean marker")? != "B" {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined boolean marker",
        ));
    }
    parse_bool_token(&fields[1], "Catalog Predefined boolean").map_err(Into::into)
}

fn parse_typed_string(value: &NativeValue) -> Result<String, PredefinedCodecError> {
    let fields = exact_list(value, 2, "Catalog Predefined string")?;
    if required_text(&fields[0], "Catalog Predefined string marker")? != "S" {
        return Err(PredefinedCodecError::InvalidShape(
            "Catalog Predefined string marker",
        ));
    }
    Ok(required_text(&fields[1], "Catalog Predefined string")?.to_owned())
}

fn parse_count(value: &NativeValue, field: &'static str) -> Result<usize, PredefinedCodecError> {
    required_token(value, field)?
        .parse::<usize>()
        .map_err(|_| PredefinedCodecError::InvalidShape(field))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PredefinedCodecError {
    Profile(BodyProfileError),
    Native(String),
    InvalidShape(&'static str),
    InvalidModel(&'static str),
    InvalidUuid(String),
    CountOverflow,
    UnsupportedTypedFamily(PredefinedFamily),
    OpaqueProfileMismatch {
        source: ProfileId,
        target: ProfileId,
        family: PredefinedFamily,
    },
}

impl Display for PredefinedCodecError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Profile(source) => Display::fmt(source, formatter),
            Self::Native(reason) => {
                write!(formatter, "native Predefined codec rejected data: {reason}")
            }
            Self::InvalidShape(field) => write!(formatter, "invalid Predefined body: {field}"),
            Self::InvalidModel(field) => write!(formatter, "invalid Predefined model: {field}"),
            Self::InvalidUuid(value) => write!(formatter, "invalid Predefined UUID `{value}`"),
            Self::CountOverflow => formatter.write_str("Predefined count arithmetic overflow"),
            Self::UnsupportedTypedFamily(family) => write!(
                formatter,
                "typed base-free compilation is not evidenced for {family:?}; exact same-profile passthrough remains available"
            ),
            Self::OpaqueProfileMismatch {
                source,
                target,
                family,
            } => write!(
                formatter,
                "opaque {family:?} Predefined body from `{source}` cannot be reused for `{target}`"
            ),
        }
    }
}

impl Error for PredefinedCodecError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Profile(source) => Some(source),
            _ => None,
        }
    }
}

impl From<BodyProfileError> for PredefinedCodecError {
    fn from(source: BodyProfileError) -> Self {
        Self::Profile(source)
    }
}

impl From<NativeError> for PredefinedCodecError {
    fn from(source: NativeError) -> Self {
        Self::Native(source.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_model() -> CatalogPredefinedData {
        CatalogPredefinedData {
            layout: CatalogPredefinedLayout {
                code_length: 0,
                description_length: 150,
                synthetic_root_name: "Элементы".to_owned(),
            },
            items: vec![CatalogPredefinedItem {
                id: ObjectUuid::parse("6c4b0307-43a4-4141-9c35-3dd7e9586d41").unwrap(),
                name: "Администратор".to_owned(),
                code: String::new(),
                description: "Администратор".to_owned(),
                is_folder: false,
                children: Vec::new(),
            }],
        }
    }

    #[test]
    fn catalog_fixture_roundtrips_without_a_base_blob() {
        let profile = PredefinedCodecProfile::fixture();
        let model = fixture_model();
        let blob = compile_catalog_predefined(&profile, &model).unwrap();
        let decoded = decode_catalog_predefined(&profile, &blob, model.layout.clone()).unwrap();
        assert_eq!(decoded, model);
        let opaque = decode_predefined(&profile, PredefinedFamily::Catalog, &blob).unwrap();
        assert_eq!(encode_predefined(&profile, &opaque).unwrap(), blob);
    }

    #[test]
    fn nested_catalog_items_roundtrip_with_deterministic_row_indexes() {
        let profile = PredefinedCodecProfile::fixture();
        let mut model = fixture_model();
        model.items[0].is_folder = true;
        model.items[0].children.push(CatalogPredefinedItem {
            id: ObjectUuid::parse("d9f0e180-87a2-41d7-929c-437c0de47203").unwrap(),
            name: "Оператор".to_owned(),
            code: String::new(),
            description: "Оператор".to_owned(),
            is_folder: false,
            children: Vec::new(),
        });
        let blob = compile_catalog_predefined(&profile, &model).unwrap();
        let decoded = decode_catalog_predefined(&profile, &blob, model.layout.clone()).unwrap();
        assert_eq!(decoded, model);
    }

    #[test]
    fn chart_body_is_preserved_but_cross_profile_reuse_is_blocked() {
        let profile = PredefinedCodecProfile::fixture();
        let native = formatted_list(
            vec![token("2"), inline_list(vec![token("0")])],
            false,
            vec![1],
            false,
        );
        let blob = raw_deflate(&native).unwrap();
        let body = decode_predefined(&profile, PredefinedFamily::ChartOfAccounts, &blob).unwrap();
        assert_eq!(encode_predefined(&profile, &body).unwrap(), blob);

        let other = PredefinedCodecProfile(SelectedBodyProfile::fixture("platform-future"));
        assert!(matches!(
            encode_predefined(&other, &body),
            Err(PredefinedCodecError::OpaqueProfileMismatch { .. })
        ));
    }

    #[test]
    fn catalog_validation_rejects_lossy_tree_and_length_cases() {
        let profile = PredefinedCodecProfile::fixture();
        let mut model = fixture_model();
        let child = model.items[0].clone();
        model.items[0].children.push(child);
        assert!(matches!(
            compile_catalog_predefined(&profile, &model),
            Err(PredefinedCodecError::InvalidModel(_))
        ));

        let mut model = fixture_model();
        model.items.push(model.items[0].clone());
        assert!(matches!(
            compile_catalog_predefined(&profile, &model),
            Err(PredefinedCodecError::InvalidModel(_))
        ));

        let mut model = fixture_model();
        model.items[0].code = "unexpected".to_owned();
        assert!(matches!(
            compile_catalog_predefined(&profile, &model),
            Err(PredefinedCodecError::InvalidModel(_))
        ));
    }
}
