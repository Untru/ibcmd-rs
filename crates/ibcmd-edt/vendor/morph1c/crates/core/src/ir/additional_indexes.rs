//! Format-neutral current values of the optional additional-index attachment.
use super::Uuid;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdditionalIndex {
    pub id: Uuid,
    pub name: String,
    pub table: String,
    pub indexed_fields: Vec<String>,
    pub additional_fields: Vec<String>,
}
