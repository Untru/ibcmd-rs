//! Editor apply uses the existing own backend only. File databases return a
//! descriptor for an explicit editor-side native operation; the server never
//! discovers or executes a platform tool.
use super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ApplyParams {
    infobase: String,
    mode: String,
    #[serde(default)]
    confirm: bool,
    /// Existing backend recovery artifacts must remain available for inspection.
    recovery_dir: PathBuf,
}

impl State {
    pub(super) fn prepare_apply(&mut self, params: InfobaseParams) -> Result<Value, RpcError> {
        let session = self.session(&params.infobase)?;
        match session.configuration.source() {
            ConfigSource::FileDb(path) => {
                if !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.eq_ignore_ascii_case("1Cv8.1CD"))
                {
                    return Err(RpcError::invalid_params(
                        "native apply requires a standard 1Cv8.1CD database filename",
                    ));
                }
                Ok(json!({
                    "backend": "external-native", "modes": ["exclusive"],
                    "dbPath": path.parent(), "platform": session.entry.platform,
                    "warning": "Close all sessions and retain a backup. Native cancellation can leave an uncertain outcome; inspect pending state before retrying."
                }))
            }
            ConfigSource::Database { .. } => {
                crate::dropin::apply::profile_of(session.configuration.platform())
                    .map_err(failure)?;
                Ok(json!({"backend": "own", "modes": ["exclusive"],
                    "warning": "Exclusive access is required. Structural changes and dynamic mode are not enabled by this editor command. Cancellation after SQL starts cannot guarantee rollback; inspect pending state before retrying."}))
            }
            _ => Err(offline("config/apply", session)),
        }
    }

    pub(super) fn apply_config(&mut self, params: ApplyParams) -> Result<Value, RpcError> {
        if params.mode != "exclusive" {
            return Err(RpcError::invalid_params(
                "only exclusive apply is supported; dynamic compatibility is tracked in #347",
            ));
        }
        if !params.confirm {
            return Err(RpcError::new(
                codes::CONFIRMATION_REQUIRED,
                "explicit apply consent is required",
            ));
        }
        if !params.recovery_dir.is_absolute() {
            return Err(RpcError::invalid_params("recoveryDir must be absolute"));
        }
        let output = self.output.clone();
        let session = self.session(&params.infobase)?;
        let ConfigSource::Database { sql, database } = session.configuration.source() else {
            return Err(offline(
                "config/apply (use the explicit external-native editor route for file-db)",
                session,
            ));
        };
        let profile =
            crate::dropin::apply::profile_of(session.configuration.platform()).map_err(failure)?;
        let mut options = crate::mssql_config_apply::ConfigApplyOptions::new(
            database,
            profile,
            crate::restructure::extensions::read_adoptions,
            &crate::mssql_dump::StoredActivationRows,
        );
        options.recovery_dir = Some(params.recovery_dir.join(uuid::Uuid::new_v4().to_string()));
        // Never prune operator artifacts from this editor operation.
        options.recovery_keep = 0;
        crate::cancel::check().map_err(|e| failure(e.into()))?;
        output.log("info", "config/apply: exclusive apply started");
        let report = objects::exclusive(|| {
            crate::cancel::check()?;
            crate::mssql_config_apply::apply_staged_configuration(sql, &options)
        })
        .map_err(failure)?;
        // Do not turn a successful publication into a cancellation afterwards.
        self.import_plans.clear();
        output.log("info", "config/apply: exclusive apply completed");
        Ok(json!({"backend": "own", "report": report}))
    }
}
