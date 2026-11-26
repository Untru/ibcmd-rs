//! Compatibility exports and the root extension-image adapter.

use crate::cli::MssqlExtensionImage;
use crate::mssql_dump::extension::adopted_objects;
use crate::mssql_extension_export::fetch_extension_image;
use crate::mssql_extensions::list_extensions_on;
use crate::sql::SqlExec;
use anyhow::{Context, Result};
pub use ibcmd_restructure_engine::restructure::extensions::*;

/// The objects every extension adopts, read from its active image and from its staged one (when it has
/// one), through `sql` (the own apply's client, the research command's). Any extension that cannot be read
/// fails the whole read.
pub fn read_adoptions(sql: &SqlExec, database: &str) -> Result<Vec<Adoption>> {
    let listed = list_extensions_on(sql, database).context("the registry of the extensions")?;
    let mut adoptions = Vec::new();
    for extension in &listed.extensions {
        let mut read_active = false;
        for choice in [MssqlExtensionImage::Active, MssqlExtensionImage::Auto] {
            let (image, source) = fetch_extension_image(sql, database, extension, choice)
                .with_context(|| {
                    format!("the {choice:?} image of the extension {:?}", extension.name)
                })?;
            // `Auto` is the active image again when nothing is staged.
            if read_active && source.image == "active" {
                continue;
            }
            read_active |= source.image == "active";
            for object in adopted_objects(&image)
                .with_context(|| format!("the objects of the extension {:?}", extension.name))?
            {
                adoptions.push(Adoption {
                    extension: extension.name.clone(),
                    image: source.image,
                    object,
                });
            }
        }
    }
    Ok(adoptions)
}
