//! Standard extension import/apply use the same verified SQL protocol as the
//! direct commands. Build identity is supplied explicitly, never inferred
//! from the XML dialect or from the database name.
use std::path::Path;

use anyhow::{Context, Result, bail};

use super::parse::{
    ApplyRequest, Common, DynamicMode, ExclusivityMode, ImportRequest, SessionTerminate,
};
use super::{APPLY, IMPORT};
use crate::cli::{
    MssqlActivateStagedExtensionArgs, MssqlLoadExtensionArgs, MssqlMainActivationModeArg,
};
use crate::infobase::{
    ConnectionConfig, ConnectionRequest, PlatformNeed, ensure_mssql, resolve_connection,
};
use crate::mssql_platform_profile::MssqlNativePlatformProfile;

fn target(
    common: &Common,
    source: Option<&Path>,
) -> Result<(ConnectionConfig, MssqlNativePlatformProfile)> {
    common
        .cluster_id
        .context("extension writes require --cluster-id")?;
    common
        .infobase_id
        .context("extension writes require --infobase-id")?;
    let server = common.db_server.as_deref().map(super::sql_server_name);
    let pwd_env = common.db_pwd_env.as_deref().unwrap_or("IBCMD_DB_PSW");
    let config = resolve_connection(ConnectionRequest {
        settings: common.settings.as_deref(),
        native_config: common.native_config.as_deref(),
        format: None,
        platform: common.platform,
        source_version: common.source_version,
        db_path: common.db_path.as_deref(),
        dbms: common.dbms.as_deref(),
        db_server: server.as_deref(),
        db_name: common.db_name.as_deref(),
        db_user: common.db_user.as_deref(),
        db_pwd: common.db_pwd.as_deref(),
        db_pwd_env: pwd_env,
        need: source.map_or(PlatformNeed::Given, |source_dir| PlatformNeed::Import {
            source_dir,
        }),
    })?;
    if config.file_infobase.is_some() {
        bail!("extension writes currently require a SQL Server infobase");
    }
    ensure_mssql(&config.dbms)?;
    let settings = crate::settings::Settings::load(common.native_config.as_deref())?;
    let resolved = crate::settings::resolve_platform_with_source(
        common.platform.map(|p| p.display()),
        &settings,
        crate::settings::DatabaseTarget::new(Some(&config.db_server), &config.db_name),
        source.map_or(crate::settings::PlatformHint::None, |source_root| {
            crate::settings::PlatformHint::Import { source_root }
        }),
    )?;
    let profile = super::apply::profile_of(resolved.value)?;
    require_profile(profile)?;
    Ok((config, profile))
}

fn require_profile(profile: MssqlNativePlatformProfile) -> Result<()> {
    if !matches!(
        profile,
        MssqlNativePlatformProfile::Platform8_3_27_2214
            | MssqlNativePlatformProfile::Platform8_5_1_1150
    ) {
        bail!(
            "standard extension import/apply is evidenced only on 8.3.27.2214 and 8.5.1.1150; select --platform explicitly"
        );
    }
    profile.require_extension_write_supported()
}

fn load_args(request: &ImportRequest) -> Result<MssqlLoadExtensionArgs> {
    if request.base_free || request.no_verify {
        bail!(
            "extension import requires a verified overlay; --base-free and --no-verify are not supported"
        );
    }
    let c = &request.common;
    let path = Path::new(&request.path);
    let (config, profile) = target(c, Some(path))?;
    let source_version = config.legacy_source_version()?;
    Ok(MssqlLoadExtensionArgs {
        platform_profile: profile,
        rac: c.rac.clone().unwrap_or_else(|| "rac".into()),
        ras_endpoint: c
            .ras_endpoint
            .clone()
            .unwrap_or_else(|| "localhost:1545".into()),
        cluster_id: c.cluster_id,
        infobase_id: c.infobase_id,
        infobase_user: c.user.clone(),
        infobase_pwd: c.password.clone(),
        sqlcmd: c.sqlcmd.clone(),
        bcp_executable: None,
        server: config.db_server,
        database: config.db_name,
        sql_user: config.db_user,
        sql_pwd: config.db_pwd,
        sql_pwd_env: c
            .db_pwd_env
            .clone()
            .unwrap_or_else(|| "IBCMD_DB_PSW".into()),
        extension: c.extension.clone(),
        all_extensions: false,
        input_dir: path.to_owned(),
        path_prefix: Vec::new(),
        replace_staging: true,
        dry_run: false,
        allow_non_lab: true,
        sqlcmd_trust_cert: true,
        platform: c.platform,
        source_version,
    })
}

fn apply_args(request: &ApplyRequest) -> Result<MssqlActivateStagedExtensionArgs> {
    if request.dynamic == DynamicMode::Force
        || request.session_terminate != SessionTerminate::Disable
        || request.exclusivity != ExclusivityMode::Sql
    {
        bail!(
            "extension apply supports exclusive publication only, without terminating sessions or bypassing the session guard"
        );
    }
    let c = &request.common;
    let (config, profile) = target(c, None)?;
    Ok(MssqlActivateStagedExtensionArgs {
        platform_profile: profile,
        rac: c.rac.clone().unwrap_or_else(|| "rac".into()),
        ras_endpoint: c
            .ras_endpoint
            .clone()
            .unwrap_or_else(|| "localhost:1545".into()),
        cluster_id: c.cluster_id,
        infobase_id: c.infobase_id,
        infobase_user: c.user.clone(),
        infobase_pwd: c.password.clone(),
        sqlcmd: c.sqlcmd.clone(),
        bcp_executable: None,
        server: config.db_server,
        database: config.db_name,
        sql_user: config.db_user,
        sql_pwd: config.db_pwd,
        sql_pwd_env: c
            .db_pwd_env
            .clone()
            .unwrap_or_else(|| "IBCMD_DB_PSW".into()),
        extension: c.extension.clone().context("extension name is missing")?,
        mode: MssqlMainActivationModeArg::Exclusive,
        dry_run: false,
        allow_non_lab: true,
        sqlcmd_trust_cert: true,
        script_output: None,
        recovery_output: None,
    })
}

pub(super) fn run_import(request: &ImportRequest) -> i32 {
    IMPORT.start();
    match load_args(request).and_then(|args| crate::mssql_extension_load::load_extensions(&args)) {
        Ok(value) => IMPORT.succeed(request.common.report.as_deref(), &value),
        Err(error) => IMPORT.fail_with(&format!("{error:#}"), request.common.report.as_deref()),
    }
}

pub(super) fn run_apply(request: &ApplyRequest) -> i32 {
    match apply_args(request)
        .and_then(|args| crate::mssql_extension_load::activate_staged_extension(&args))
    {
        Ok(value) => {
            if value.activation.no_op {
                println!("[INFO] Обновление конфигурации базы данных не требуется");
            }
            APPLY.succeed(request.common.report.as_deref(), &value)
        }
        Err(error) => APPLY.fail_with(&format!("{error:#}"), request.common.report.as_deref()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse::{Invocation, Refusal, parse_infobase};
    fn parse(args: &[&str]) -> std::result::Result<Invocation, Refusal> {
        parse_infobase(
            &args
                .iter()
                .map(std::ffi::OsString::from)
                .collect::<Vec<_>>(),
        )
    }
    use super::*;

    #[test]
    fn extension_import_and_apply_preserve_explicit_target_binding() {
        let common = [
            "--extension=Example",
            "--platform=8.5.1.1150",
            "--cluster-id=00000000-0000-0000-0000-000000000001",
            "--infobase-id=00000000-0000-0000-0000-000000000002",
            "--rac=must-not-run",
            "--ras-endpoint=explicit:3545",
        ];
        let mut input = vec!["config", "import"];
        input.extend(common);
        input.push("tree");
        let Invocation::Import(import) = parse(&input).unwrap() else {
            panic!("not import")
        };
        assert_eq!(import.common.extension.as_deref(), Some("Example"));
        assert_eq!(import.common.cluster_id.unwrap().as_u128(), 1);
        assert_eq!(import.common.infobase_id.unwrap().as_u128(), 2);
        assert_eq!(import.common.ras_endpoint.as_deref(), Some("explicit:3545"));
        let mut input = vec!["config", "apply"];
        input.extend(common);
        let Invocation::Apply(apply) = parse(&input).unwrap() else {
            panic!("not apply")
        };
        assert_eq!(apply.common, import.common);
    }

    #[test]
    fn extension_writes_require_binding_and_refuse_unmeasured_modes_before_io() {
        let Invocation::Import(import) =
            parse(&["config", "import", "--extension=E", "tree"]).unwrap()
        else {
            panic!()
        };
        assert!(
            load_args(&import)
                .unwrap_err()
                .to_string()
                .contains("--cluster-id")
        );
        let Invocation::Apply(apply) =
            parse(&["config", "apply", "--extension=E", "--dynamic=force"]).unwrap()
        else {
            panic!()
        };
        assert!(
            apply_args(&apply)
                .unwrap_err()
                .to_string()
                .contains("exclusive")
        );
        assert!(require_profile(MssqlNativePlatformProfile::Platform8_3_27_2214).is_ok());
        assert!(require_profile(MssqlNativePlatformProfile::Platform8_5_1_1150).is_ok());
        assert!(require_profile(MssqlNativePlatformProfile::Platform8_3_27_1989).is_err());
        for bad in ["--extension=", "--cluster-id=oops", "--infobase-id=oops"] {
            assert!(parse(&["config", "apply", bad]).is_err());
        }
    }
}
