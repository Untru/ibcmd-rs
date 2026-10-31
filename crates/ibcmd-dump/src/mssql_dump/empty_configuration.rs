//! The export of an infobase whose `Config` table holds no row (#469): one
//! right after `ibcmd infobase create`. The platform exports the default
//! configuration it makes up for it -- `Configuration.xml`, one language and
//! an empty `ConfigDumpInfo.xml` -- with uuids it draws anew at each export
//! (two exports of one empty infobase differ in them alone).
//!
//! The templates are those exports with the uuids and the language taken
//! out: 8.3.27.2214 (Designer `/DumpConfigToFiles`) and 8.5.1.1529 (`ibcmd
//! infobase config export`), interface language Russian; the English
//! variant is 8.5.1.1150's on Linux (`tests/fixtures/file-infobase`).
//! Neither the locale of the infobase nor anything stored in it picks the
//! language: `--locale=en_US` and `--locale=de_DE` infobases exported from a
//! Russian interface give `Русский` as well.

use std::path::Path;

use anyhow::Result;

use super::output_writer::OutputWriter;
use crate::legacy_version::InfobaseConfigSourceVersion;

struct Templates {
    configuration: &'static str,
    language: &'static str,
    config_dump_info: &'static str,
}

const V2_20: Templates = Templates {
    configuration: include_str!("empty_configuration/Configuration_8_3_27.xml"),
    language: include_str!("empty_configuration/Language_8_3_27.xml"),
    config_dump_info: include_str!("empty_configuration/ConfigDumpInfo_8_3_27.xml"),
};

const V2_21: Templates = Templates {
    configuration: include_str!("empty_configuration/Configuration_8_5_1.xml"),
    language: include_str!("empty_configuration/Language_8_5_1.xml"),
    config_dump_info: include_str!("empty_configuration/ConfigDumpInfo_8_5_1.xml"),
};

/// The default configuration's words in one interface language.
struct Language {
    configuration_name: &'static str,
    script_variant: &'static str,
    name: &'static str,
    code: &'static str,
}

const RUSSIAN: Language = Language {
    configuration_name: "Конфигурация",
    script_variant: "Russian",
    name: "Русский",
    code: "ru",
};

const ENGLISH: Language = Language {
    configuration_name: "Configuration",
    script_variant: "English",
    name: "English",
    code: "en",
};

/// Writes the default configuration into `output_dir`.
pub(super) fn write(
    output: &OutputWriter,
    output_dir: &Path,
    source_version: InfobaseConfigSourceVersion,
) -> Result<()> {
    let templates = match source_version {
        InfobaseConfigSourceVersion::V2_20 => &V2_20,
        InfobaseConfigSourceVersion::V2_21 => &V2_21,
    };
    let language = interface_language();
    let mut configuration = templates
        .configuration
        .replace("@CONFIGURATION@", &fresh_uuid())
        .replace("@NAME@", language.configuration_name)
        .replace("@SCRIPT@", language.script_variant)
        .replace("@LANGUAGE@", language.name);
    // Seven contained objects (the configuration's own modules and
    // interfaces), each with an object id of its own.
    for index in 1..=7 {
        configuration = configuration.replace(&format!("@OBJECT{index}@"), &fresh_uuid());
    }
    let language_xml = templates
        .language
        .replace("@UUID@", &fresh_uuid())
        .replace("@LANGUAGE@", language.name)
        .replace("@CODE@", language.code);
    output.write(
        output_dir.join("Configuration.xml"),
        configuration.into_bytes(),
    )?;
    output.write(
        output_dir
            .join("Languages")
            .join(format!("{}.xml", language.name)),
        language_xml.into_bytes(),
    )?;
    output.write(
        output_dir.join("ConfigDumpInfo.xml"),
        templates.config_dump_info.as_bytes().to_vec(),
    )?;
    Ok(())
}

fn fresh_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// The interface language the platform would run in: `IBCMD_RS_LANGUAGE`
/// (`ru`/`en`) when set, else the system's -- the user's UI language on
/// Windows, `LC_ALL`/`LC_MESSAGES`/`LANG` elsewhere. Russian when it is
/// Russian, English otherwise.
fn interface_language() -> &'static Language {
    let russian = match std::env::var("IBCMD_RS_LANGUAGE") {
        Ok(value) if !value.trim().is_empty() => {
            value.trim().to_ascii_lowercase().starts_with(RUSSIAN.code)
        }
        _ => system_language_is_russian(),
    };
    if russian { &RUSSIAN } else { &ENGLISH }
}

#[cfg(windows)]
fn system_language_is_russian() -> bool {
    // LANG_RUSSIAN, the primary language of the LANGID.
    const LANG_RUSSIAN: u16 = 0x19;
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    id & 0x3ff == LANG_RUSSIAN
}

#[cfg(not(windows))]
fn system_language_is_russian() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with(RUSSIAN.code))
}
