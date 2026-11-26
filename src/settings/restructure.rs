//! The restructure limit of the own apply in the settings chain (S1-J, #406).
//!
//! `mssql-config-apply` and `mssql-restructure` take the limit from the flags
//! `--restructure-limit-rows` / `--restructure-limit-bytes`; the drop-in `ibcmd infobase config apply` takes
//! no such flag (it is the platform's syntax and gains nothing native-looking) and reads the same two
//! values from here. Highest first:
//!
//! 1. the flag (the two commands above only);
//! 2. the environment: `IBCMD_RS_RESTRUCTURE_LIMIT_ROWS` and `IBCMD_RS_RESTRUCTURE_LIMIT_BYTES`;
//! 3. the settings files: the top-level keys `restructure-limit-rows` and `restructure-limit-bytes` of
//!    `ibcmd-rs.toml` (the highest layer that sets one);
//! 4. the measured default (`restructure::size_guard`).
//!
//! Rows are a whole number (`2000000`, `2_000_000`). Bytes are the bytes the rebuild writes to the log under
//! full recovery (the data of the rebuilt tables twice, their other indexes once), a whole number or a number with a
//! unit, every unit a power of 1024 (`4GB`, `4 GiB`, `512MB`, `1.5g`); in the TOML file either a number or
//! a string (`restructure-limit-bytes = "4GB"`). A malformed value fails when a command asks for the
//! limit, naming the variable or the file and line; the default is used only when nothing names the value.

use anyhow::{Context, Result, bail};

use super::{
    ENV_RESTRUCTURE_LIMIT_BYTES, ENV_RESTRUCTURE_LIMIT_ROWS, SettingSource, Settings, Sourced,
};
use crate::restructure::size_guard::parse_byte_size;

/// A whole number of rows: `2000000` or `2_000_000`.
pub(super) fn parse_count(text: &str) -> Result<u64> {
    let text = text.trim();
    let digits = text.replace('_', "");
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("`{text}` is not a whole number");
    }
    digits
        .parse()
        .with_context(|| format!("`{text}` is more than a 64-bit count"))
}

impl Settings {
    /// The row limit of a restructure: `IBCMD_RS_RESTRUCTURE_LIMIT_ROWS`, `restructure-limit-rows` of the
    /// settings files. `None` when nothing names it.
    pub fn restructure_limit_rows(&self) -> Result<Option<Sourced<u64>>> {
        if let Some(value) = self.env_value(ENV_RESTRUCTURE_LIMIT_ROWS) {
            let rows = parse_count(&value.value).with_context(|| {
                format!("{ENV_RESTRUCTURE_LIMIT_ROWS}=`{}`", value.value.trim())
            })?;
            return Ok(Some(Sourced::new(rows, value.source)));
        }
        Ok(self.files.iter().rev().find_map(|file| {
            file.restructure_limit_rows.as_ref().map(|located| {
                Sourced::new(
                    located.value,
                    SettingSource::File {
                        path: file.path.clone(),
                        line: located.line,
                    },
                )
            })
        }))
    }

    /// The byte limit of a restructure: `IBCMD_RS_RESTRUCTURE_LIMIT_BYTES`, `restructure-limit-bytes` of the
    /// settings files. `None` when nothing names it.
    pub fn restructure_limit_bytes(&self) -> Result<Option<Sourced<u64>>> {
        if let Some(value) = self.env_value(ENV_RESTRUCTURE_LIMIT_BYTES) {
            let bytes = parse_byte_size(&value.value).with_context(|| {
                format!("{ENV_RESTRUCTURE_LIMIT_BYTES}=`{}`", value.value.trim())
            })?;
            return Ok(Some(Sourced::new(bytes, value.source)));
        }
        Ok(self.files.iter().rev().find_map(|file| {
            file.restructure_limit_bytes.as_ref().map(|located| {
                Sourced::new(
                    located.value,
                    SettingSource::File {
                        path: file.path.clone(),
                        line: located.line,
                    },
                )
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::restructure::size_guard::{DEFAULT_LIMIT_BYTES, DEFAULT_LIMIT_ROWS, resolve_limit};
    use crate::settings::SettingsSources;

    fn load(
        env: &[(&str, &str)],
        toml: Option<&str>,
    ) -> (Settings, Option<crate::test_scratch::TestScratch>) {
        let dir = toml.map(|text| {
            let dir = crate::test_scratch::TestScratch::adopt(
                std::env::temp_dir().join(format!("ibcmd-rs-limit-{}", uuid::Uuid::new_v4())),
            );
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("ibcmd-rs.toml"), text).unwrap();
            dir
        });
        let env: Vec<(String, String)> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let lookup = move |name: &str| {
            env.iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        };
        let loaded = Settings::from_sources(&SettingsSources {
            env: &lookup,
            executable: None,
            current_dir: dir.as_ref().map(|dir| dir.to_path_buf()),
            app_data: None,
            native_config: None,
        })
        .unwrap();
        (loaded, dir)
    }

    /// Removes the settings folder of `load`, if any.
    fn cleanup(dir: Option<crate::test_scratch::TestScratch>) {
        drop(dir);
    }

    #[test]
    fn nothing_named_is_the_default() {
        let (settings, dir) = load(&[], None);
        assert!(settings.restructure_limit_rows().unwrap().is_none());
        assert!(settings.restructure_limit_bytes().unwrap().is_none());
        let resolved = resolve_limit(&settings, None, None).unwrap();
        assert_eq!(resolved.limit.rows, DEFAULT_LIMIT_ROWS);
        assert_eq!(resolved.limit.bytes, DEFAULT_LIMIT_BYTES);
        assert_eq!(resolved.rows_source, "the default");
        cleanup(dir);
    }

    #[test]
    fn the_environment_names_both_halves_with_units() {
        let (settings, dir) = load(
            &[
                (ENV_RESTRUCTURE_LIMIT_ROWS, " 5_000_000 "),
                (ENV_RESTRUCTURE_LIMIT_BYTES, "8 GiB"),
            ],
            None,
        );
        let resolved = resolve_limit(&settings, None, None).unwrap();
        assert_eq!(resolved.limit.rows, 5_000_000);
        assert_eq!(resolved.limit.bytes, 8 << 30);
        assert_eq!(resolved.rows_source, ENV_RESTRUCTURE_LIMIT_ROWS);
        assert_eq!(resolved.bytes_source, ENV_RESTRUCTURE_LIMIT_BYTES);
        cleanup(dir);
    }

    #[test]
    fn the_settings_file_names_them_as_numbers_or_strings() {
        let (settings, dir) = load(
            &[],
            Some("restructure-limit-rows = 123456\nrestructure-limit-bytes = \"2GB\"\n"),
        );
        let rows = settings.restructure_limit_rows().unwrap().unwrap();
        assert_eq!(rows.value, 123_456);
        assert!(matches!(rows.source, SettingSource::File { line: 1, .. }));
        let bytes = settings.restructure_limit_bytes().unwrap().unwrap();
        assert_eq!(bytes.value, 2 << 30);
        assert!(matches!(bytes.source, SettingSource::File { line: 2, .. }));
        cleanup(dir);
        let (settings, dir) = load(&[], Some("restructure-limit-bytes = 1048576\n"));
        assert_eq!(
            settings.restructure_limit_bytes().unwrap().unwrap().value,
            1 << 20
        );
        cleanup(dir);
    }

    #[test]
    fn the_flag_beats_the_environment_beats_the_file_and_each_half_is_its_own() {
        let (settings, dir) = load(
            &[(ENV_RESTRUCTURE_LIMIT_ROWS, "700")],
            Some("restructure-limit-rows = 100\nrestructure-limit-bytes = \"1GB\"\n"),
        );
        // rows: the environment over the file; bytes: the file
        let resolved = resolve_limit(&settings, None, None).unwrap();
        assert_eq!(resolved.limit.rows, 700);
        assert_eq!(resolved.limit.bytes, 1 << 30);
        // the flags over both
        let resolved = resolve_limit(&settings, Some(9), Some("3MB")).unwrap();
        assert_eq!(resolved.limit.rows, 9);
        assert_eq!(resolved.limit.bytes, 3 << 20);
        assert_eq!(resolved.rows_source, "--restructure-limit-rows");
        assert_eq!(resolved.bytes_source, "--restructure-limit-bytes");
        cleanup(dir);
    }

    #[test]
    fn a_malformed_value_names_where_it_is() {
        let (settings, dir) = load(&[(ENV_RESTRUCTURE_LIMIT_BYTES, "lots")], None);
        let error = settings.restructure_limit_bytes().unwrap_err();
        assert!(
            format!("{error:#}").contains(ENV_RESTRUCTURE_LIMIT_BYTES),
            "{error:#}"
        );
        cleanup(dir);
        let (settings, dir) = load(&[(ENV_RESTRUCTURE_LIMIT_ROWS, "12x")], None);
        assert!(settings.restructure_limit_rows().is_err());
        cleanup(dir);
        // a limit of 0 would refuse everything
        let (settings, dir) = load(&[], None);
        assert!(resolve_limit(&settings, Some(0), None).is_err());
        assert!(resolve_limit(&settings, None, Some("0")).is_err());
        cleanup(dir);
    }
}
