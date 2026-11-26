//! A file a form's items carry beside `Form.xml` (from
//! `src/mssql_dump/source_assets.rs`).

pub struct FormItemAsset {
    pub item_name: String,
    pub file_name: String,
    pub content: Vec<u8>,
}
