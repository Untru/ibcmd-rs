//! Native SQL transport settings, resolved once before opening connections.

use std::ffi::OsString;

use anyhow::{Result, bail};
use tiberius::EncryptionLevel;

const ENCRYPTION_SETTING: &str = "IBCMD_RS_MSSQL_ENCRYPTION";
const COMPATIBILITY_SETTING: &str = "IBCMD_RS_MSSQL_COMPATIBILITY";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MssqlEncryption {
    Required,
    LoginOnly,
    Disabled,
}

impl MssqlEncryption {
    fn parse(setting: Option<OsString>) -> Result<Self> {
        match setting.as_ref().map(|setting| setting.to_str()) {
            None | Some(Some("required")) => Ok(Self::Required),
            Some(Some("login-only")) => Ok(Self::LoginOnly),
            Some(Some("disabled")) => Ok(Self::Disabled),
            _ => bail!("{ENCRYPTION_SETTING} must be required, login-only or disabled"),
        }
    }

    pub(super) fn tds_level(self) -> EncryptionLevel {
        match self {
            Self::Required => EncryptionLevel::Required,
            Self::LoginOnly => EncryptionLevel::Off,
            Self::Disabled => EncryptionLevel::NotSupported,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MssqlConnectionPolicy {
    pub(super) sql_server_2012: bool,
    pub(super) encryption: MssqlEncryption,
}

impl MssqlConnectionPolicy {
    pub(super) fn from_env() -> Result<Self> {
        Self::parse(
            std::env::var_os(COMPATIBILITY_SETTING),
            std::env::var_os(ENCRYPTION_SETTING),
        )
    }

    fn parse(compatibility: Option<OsString>, encryption: Option<OsString>) -> Result<Self> {
        let sql_server_2012 = match compatibility.as_ref().map(|setting| setting.to_str()) {
            None => false,
            Some(Some("2012")) => true,
            _ => bail!("{COMPATIBILITY_SETTING} must be 2012 when set"),
        };
        let encryption = MssqlEncryption::parse(encryption)?;
        if !sql_server_2012 && encryption != MssqlEncryption::Required {
            bail!(
                "{ENCRYPTION_SETTING} login-only or disabled requires {COMPATIBILITY_SETTING}=2012"
            );
        }
        Ok(Self {
            sql_server_2012,
            encryption,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::{MssqlConnectionPolicy, MssqlEncryption};

    #[test]
    fn compatibility_is_explicit_and_transport_defaults_to_required() {
        let standard = MssqlConnectionPolicy::parse(None, None).unwrap();
        assert!(!standard.sql_server_2012);
        assert_eq!(standard.encryption, MssqlEncryption::Required);

        let legacy = MssqlConnectionPolicy::parse(Some("2012".into()), None).unwrap();
        assert!(legacy.sql_server_2012);
        assert_eq!(legacy.encryption, MssqlEncryption::Required);
    }

    #[test]
    fn non_required_transport_requires_sql2012_opt_in() {
        for setting in ["login-only", "disabled"] {
            let error = MssqlConnectionPolicy::parse(None, Some(setting.into()))
                .unwrap_err()
                .to_string();
            assert!(error.contains("IBCMD_RS_MSSQL_COMPATIBILITY"));

            let legacy =
                MssqlConnectionPolicy::parse(Some("2012".into()), Some(setting.into())).unwrap();
            assert!(legacy.sql_server_2012);
        }
    }

    #[test]
    fn invalid_compatibility_is_rejected_without_echoing_the_value() {
        for setting in ["", "private-invalid-compatibility", "110", "2019", " 2012"] {
            let error = MssqlConnectionPolicy::parse(Some(setting.into()), None)
                .unwrap_err()
                .to_string();
            assert!(error.contains("IBCMD_RS_MSSQL_COMPATIBILITY"));
            assert!(!error.contains("private-invalid-compatibility"));
        }
    }

    #[test]
    fn absent_setting_requires_encryption() {
        assert_eq!(
            MssqlEncryption::parse(None).unwrap(),
            MssqlEncryption::Required
        );
    }

    #[test]
    fn explicit_transport_modes_are_parsed() {
        for (setting, expected) in [
            ("required", MssqlEncryption::Required),
            ("login-only", MssqlEncryption::LoginOnly),
            ("disabled", MssqlEncryption::Disabled),
        ] {
            assert_eq!(
                MssqlEncryption::parse(Some(setting.into())).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn invalid_settings_are_rejected_without_echoing_the_value() {
        for setting in [
            "",
            "private-invalid-mode",
            "REQUIRED",
            " required",
            "disabled ",
        ] {
            let error = MssqlEncryption::parse(Some(setting.into()))
                .unwrap_err()
                .to_string();
            assert!(error.contains("IBCMD_RS_MSSQL_ENCRYPTION"));
            assert!(!error.contains("private-invalid-mode"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_setting_is_rejected() {
        use std::os::unix::ffi::OsStringExt;

        let setting = OsString::from_vec(vec![b'p', b'r', b'i', b'v', 0xff]);
        let error = MssqlEncryption::parse(Some(setting))
            .unwrap_err()
            .to_string();
        assert!(error.contains("IBCMD_RS_MSSQL_ENCRYPTION"));
        assert!(!error.contains("priv"));
    }

    #[cfg(windows)]
    #[test]
    fn non_unicode_setting_is_rejected() {
        use std::os::windows::ffi::OsStringExt;

        let setting = OsString::from_wide(&[b'p' as u16, b'r' as u16, 0xd800]);
        let error = MssqlEncryption::parse(Some(setting))
            .unwrap_err()
            .to_string();
        assert!(error.contains("IBCMD_RS_MSSQL_ENCRYPTION"));
        assert!(!error.contains("pr"));
    }
}
