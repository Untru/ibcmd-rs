//! Typed source-only features witnessed in native BSP 8.3.27/8.5.1 exports.
//! Every value is modeled; no retained XML tree or raw descriptor passthrough.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SourceExtensions {
    pub aggregates: Option<Vec<Aggregate>>,
    pub recalculations: Vec<Recalculation>,
    pub recalculation_refs: Vec<String>,
    pub calculation_predefined: Option<Vec<CalculationPredefined>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aggregate {
    pub id: String,
    pub usage: String,
    pub periodicity: String,
    pub dimensions: Vec<(String, bool)>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recalculation {
    pub id: String,
    pub name: String,
    pub synonym: Vec<(String, String)>,
    pub comment: String,
    pub data_lock_mode: String,
    pub generated_types: Vec<RecalculationType>,
    pub dimensions: Vec<RecalculationDimension>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecalculationType {
    pub category: String,
    pub type_id: String,
    pub value_id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecalculationDimension {
    pub id: String,
    pub name: String,
    pub synonym: Vec<(String, String)>,
    pub comment: String,
    pub register_dimension: String,
    pub leading_data: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalculationPredefined {
    pub id: String,
    pub name: String,
    pub code: String,
    pub numeric_code: bool,
    pub description: String,
    pub action_period_is_base: bool,
    pub displaced: Vec<String>,
    pub base: Vec<String>,
    pub leading: Vec<String>,
}
