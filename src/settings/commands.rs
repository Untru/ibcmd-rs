//! The settings for the database commands: what `--platform` (or the hidden
//! `--source-version`), `--server` and `--sql-user` leave open is taken from
//! [`Settings`], which are read only when something is left open.
//!
//! - `mssql-dump-config` is an export: after the settings, a configuration
//!   kept in compatibility 8.5 or later is 8.5.1's. An offline `--rows-dir`
//!   run connects nowhere; its `--database` (and an explicit `--server`) only
//!   pick its `[[database]]` entry.
//! - `mssql-stage-source-objects` and `mssql-audit-source-parity` are loads:
//!   after the settings, the tree's `Configuration.xml` names the format, and
//!   a platform whose XML format is not the tree's is refused before anything
//!   is staged. When nothing names the platform and the tree has no
//!   `Configuration.xml`, each file is read in the format it declares, as
//!   before.
//! - `mssql-dump-extension`: the settings only.
//! - `mssql-load-extension` and `mssql-apply-source-change` name their exact
//!   build with `--platform-profile`: without `--platform` or
//!   `--source-version` their XML format is that build's, and a `--platform`
//!   that is neither that build nor its release is refused.

use std::path::Path;

use anyhow::{Result, bail};
use clap::ArgMatches;
use clap::parser::ValueSource;

use super::{
    DatabaseTarget, PlatformHint, SettingSource, Settings, resolve_platform,
    resolve_platform_with_source,
};
use crate::cli::{
    InfobaseConfigSourceVersion, MssqlApplySourceChangeArgs, MssqlAuditSourceParityArgs,
    MssqlDumpConfigArgs, MssqlDumpExtensionArgs, MssqlLoadExtensionArgs,
    MssqlStageSourceObjectsArgs,
};
use crate::mssql_platform_profile::MssqlNativePlatformProfile;
use crate::platform::PlatformSpec;

/// Reads the settings when a command needs them ([`Settings::load`] outside
/// tests).
pub type Loader<'a> = &'a dyn Fn() -> Result<Settings>;

fn load_settings() -> Result<Settings> {
    Settings::load(None)
}

/// Whether the command line gave the argument (a default does not count).
fn given(matches: &ArgMatches, id: &str) -> bool {
    matches.value_source(id) == Some(ValueSource::CommandLine)
}

/// What a database command left open for the settings.
struct Open {
    platform: bool,
    server: bool,
    user: bool,
}

impl Open {
    fn any(&self) -> bool {
        self.platform || self.server || self.user
    }
}

/// Fills `--server` and `--sql-user` from the settings where the command
/// line left them open.
fn connection(settings: &Settings, open: &Open, server: &mut String, user: &mut Option<String>) {
    if open.server
        && let Some(value) = settings.db_server()
    {
        *server = value.value;
    }
    if open.user
        && let Some(value) = settings.db_user()
    {
        *user = Some(value.value);
    }
}

/// `mssql-dump-config`.
pub fn prepare_dump_config_with(
    args: &mut MssqlDumpConfigArgs,
    matches: &ArgMatches,
    load: Loader<'_>,
) -> Result<()> {
    if let Some(platform) = args.platform {
        args.source_version = platform.xml_version();
    }
    let offline = args.rows_dir.is_some();
    if !offline && args.database.trim().is_empty() {
        // The export refuses it: no database, nothing to settle.
        return Ok(());
    }
    let open = Open {
        platform: args.platform.is_none() && !given(matches, "source_version"),
        server: !offline && !given(matches, "server"),
        user: !offline && args.sql_user.is_none(),
    };
    if !open.any() {
        return Ok(());
    }
    let settings = load()?;
    connection(&settings, &open, &mut args.server, &mut args.sql_user);
    if open.platform {
        let platform = {
            let probe_args: &MssqlDumpConfigArgs = args;
            let probe = || {
                crate::mssql_dump::model_export::configuration_compatibility_8_5_or_later(
                    probe_args,
                )
            };
            let server = (!offline || given(matches, "server")).then_some(args.server.as_str());
            resolve_platform(
                None,
                &settings,
                DatabaseTarget::new(server, &args.database),
                PlatformHint::Export {
                    compatibility_8_5_or_later: &probe,
                },
            )?
        };
        crate::platform::note_export_platform(platform);
        args.source_version = platform.xml_version();
    }
    Ok(())
}

/// Refuses a load whose tree is not in the platform's XML format (also the
/// drop-in `infobase config import`, `crate::infobase`).
pub(crate) fn check_tree(platform: PlatformSpec, source_root: &Path, database: &str) -> Result<()> {
    let Some(tree) = crate::metadata_model::export::tree_version(source_root) else {
        return Ok(());
    };
    if tree != platform.xml_version().as_str() {
        bail!(
            "{} is XML {tree}, but the platform of {database} is {platform} (XML {}): export the tree for that platform, or name the platform with --platform",
            source_root.join("Configuration.xml").display(),
            platform.xml_version().as_str()
        );
    }
    Ok(())
}

/// The parts of a load command the settings may fill.
struct Load<'a> {
    platform: Option<PlatformSpec>,
    explicit_xml: bool,
    explicit_server: bool,
    source_version: &'a mut Option<InfobaseConfigSourceVersion>,
    server: &'a mut String,
    /// `None` for a command without a login.
    user: Option<&'a mut Option<String>>,
    database: &'a str,
    source_root: &'a Path,
}

fn prepare_load(load: Load<'_>, read_settings: Loader<'_>) -> Result<()> {
    if let Some(platform) = load.platform {
        check_tree(platform, load.source_root, load.database)?;
        *load.source_version = Some(platform.xml_version());
    }
    let open = Open {
        platform: load.platform.is_none() && !load.explicit_xml,
        server: !load.explicit_server,
        user: load.user.as_ref().is_some_and(|user| user.is_none()),
    };
    if !open.any() {
        return Ok(());
    }
    let settings = read_settings()?;
    let mut no_login = None;
    connection(
        &settings,
        &open,
        load.server,
        load.user.unwrap_or(&mut no_login),
    );
    if open.platform {
        let resolved = resolve_platform_with_source(
            None,
            &settings,
            DatabaseTarget::new(Some(load.server.as_str()), load.database),
            PlatformHint::Import {
                source_root: load.source_root,
            },
        )?;
        // Nothing names the platform and the tree has no Configuration.xml
        // to say it: each file is read in the format it declares, as before
        // the settings existed.
        if resolved.source != SettingSource::Default {
            check_tree(resolved.value, load.source_root, load.database)?;
            *load.source_version = Some(resolved.value.xml_version());
        }
    }
    Ok(())
}

/// `mssql-stage-source-objects`.
pub fn prepare_stage_source_objects_with(
    args: &mut MssqlStageSourceObjectsArgs,
    matches: &ArgMatches,
    load: Loader<'_>,
) -> Result<()> {
    prepare_load(
        Load {
            platform: args.platform,
            explicit_xml: given(matches, "source_version"),
            explicit_server: given(matches, "server"),
            source_version: &mut args.source_version,
            server: &mut args.server,
            user: Some(&mut args.sql_user),
            database: &args.database,
            source_root: &args.source_root,
        },
        load,
    )
}

/// `mssql-audit-source-parity` (it takes no login).
pub fn prepare_audit_source_parity_with(
    args: &mut MssqlAuditSourceParityArgs,
    matches: &ArgMatches,
    load: Loader<'_>,
) -> Result<()> {
    prepare_load(
        Load {
            platform: args.platform,
            explicit_xml: given(matches, "source_version"),
            explicit_server: given(matches, "server"),
            source_version: &mut args.source_version,
            server: &mut args.server,
            user: None,
            database: &args.database,
            source_root: &args.source_root,
        },
        load,
    )
}

/// `mssql-dump-extension`.
pub fn prepare_dump_extension_with(
    args: &mut MssqlDumpExtensionArgs,
    matches: &ArgMatches,
    load: Loader<'_>,
) -> Result<()> {
    if let Some(platform) = args.platform {
        args.source_version = platform.xml_version();
    }
    let open = Open {
        platform: args.platform.is_none() && !given(matches, "source_version"),
        server: !given(matches, "server"),
        user: args.sql_user.is_none(),
    };
    if !open.any() {
        return Ok(());
    }
    let settings = load()?;
    connection(&settings, &open, &mut args.server, &mut args.sql_user);
    if open.platform {
        let platform = resolve_platform(
            None,
            &settings,
            DatabaseTarget::new(Some(args.server.as_str()), &args.database),
            PlatformHint::None,
        )?;
        crate::platform::note_export_platform(platform);
        args.source_version = platform.xml_version();
    }
    Ok(())
}

/// The XML format of a command that names its exact build with
/// `--platform-profile`.
fn profile_xml(
    profile: MssqlNativePlatformProfile,
    platform: Option<PlatformSpec>,
    explicit_xml: bool,
    source_version: &mut InfobaseConfigSourceVersion,
) -> Result<()> {
    let build = profile
        .id()
        .strip_prefix("platform-")
        .unwrap_or(profile.id());
    let profile_platform = crate::platform::parse(build)?;
    match platform {
        Some(platform) => {
            if platform != profile_platform && platform != profile_platform.release_spec()? {
                bail!(
                    "--platform {platform} is not the build of --platform-profile {}",
                    profile.id()
                );
            }
            *source_version = platform.xml_version();
        }
        None if !explicit_xml => *source_version = profile_platform.xml_version(),
        None => {}
    }
    Ok(())
}

/// `mssql-load-extension`.
pub fn prepare_load_extension(
    args: &mut MssqlLoadExtensionArgs,
    matches: &ArgMatches,
) -> Result<()> {
    profile_xml(
        args.platform_profile,
        args.platform,
        given(matches, "source_version"),
        &mut args.source_version,
    )
}

/// `mssql-apply-source-change`.
pub fn prepare_apply_source_change(
    args: &mut MssqlApplySourceChangeArgs,
    matches: &ArgMatches,
) -> Result<()> {
    profile_xml(
        args.platform_profile,
        args.platform,
        given(matches, "source_version"),
        &mut args.source_version,
    )
}

/// `mssql-dump-config`, with the settings of the environment.
pub fn prepare_dump_config(args: &mut MssqlDumpConfigArgs, matches: &ArgMatches) -> Result<()> {
    prepare_dump_config_with(args, matches, &load_settings)
}

/// `mssql-stage-source-objects`, with the settings of the environment.
pub fn prepare_stage_source_objects(
    args: &mut MssqlStageSourceObjectsArgs,
    matches: &ArgMatches,
) -> Result<()> {
    prepare_stage_source_objects_with(args, matches, &load_settings)
}

/// `mssql-audit-source-parity`, with the settings of the environment.
pub fn prepare_audit_source_parity(
    args: &mut MssqlAuditSourceParityArgs,
    matches: &ArgMatches,
) -> Result<()> {
    prepare_audit_source_parity_with(args, matches, &load_settings)
}

/// `mssql-dump-extension`, with the settings of the environment.
pub fn prepare_dump_extension(
    args: &mut MssqlDumpExtensionArgs,
    matches: &ArgMatches,
) -> Result<()> {
    prepare_dump_extension_with(args, matches, &load_settings)
}

#[cfg(test)]
mod tests;
