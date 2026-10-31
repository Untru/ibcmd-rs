//! Compatibility exports, the root settings adapter and the apply gate adapter.

use crate::settings::Settings;
use anyhow::{Context, Result, bail};
pub use ibcmd_restructure_engine::restructure::size_guard::*;

// The guard of the S1 gate's structure phase reads the config apply's gate types and goes with it.
mod phase;
pub use phase::guard_phase;

/// The limit a command runs with: the flag, else the settings chain (variable, settings file), else the
/// default; rows and bytes each on their own. A limit of 0 is refused: it would refuse every rebuild.
pub fn resolve_limit(
    settings: &Settings,
    flag_rows: Option<u64>,
    flag_bytes: Option<&str>,
) -> Result<LimitSetting> {
    let default = RestructureLimit::default();
    let (rows, rows_source) = match flag_rows {
        Some(rows) => (rows, "--restructure-limit-rows".to_owned()),
        None => match settings.restructure_limit_rows()? {
            Some(setting) => (setting.value, setting.source.to_string()),
            None => (default.rows, DEFAULT_SOURCE.to_owned()),
        },
    };
    let (bytes, bytes_source) = match flag_bytes {
        Some(text) => (
            parse_byte_size(text).context("--restructure-limit-bytes")?,
            "--restructure-limit-bytes".to_owned(),
        ),
        None => match settings.restructure_limit_bytes()? {
            Some(setting) => (setting.value, setting.source.to_string()),
            None => (default.bytes, DEFAULT_SOURCE.to_owned()),
        },
    };
    if rows == 0 || bytes == 0 {
        bail!(
            "a restructure limit of 0 would refuse every rebuild (rows {rows} from {rows_source}, bytes {bytes} from {bytes_source})"
        );
    }
    Ok(LimitSetting {
        limit: RestructureLimit { rows, bytes },
        rows_source,
        bytes_source,
    })
}
