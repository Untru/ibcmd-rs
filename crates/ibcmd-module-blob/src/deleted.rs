//! The `deleted` row of the platform's import, which a stage writes into
//! ConfigSave and the export of a file infobase compares a stored one with.
//! It moved here from the root's `mssql/override_stage.rs`, which re-exports
//! it at its old path; its test stays there.

use anyhow::{Result, bail};

use crate::deflate_raw;

/// The `deleted` row of the platform's import: a BOM, the number of names,
/// then each name in quotes followed by `0` (the flag of a Config row), raw
/// deflate.
pub fn deleted_row(names: &[String]) -> Result<Vec<u8>> {
    let mut text = format!("\u{feff}{}", names.len());
    for name in names {
        if name.contains('"') {
            bail!("a row name with a quote cannot be listed in the deleted row: {name}");
        }
        text.push_str(&format!(",\"{name}\",0"));
    }
    deflate_raw(text.as_bytes())
}
