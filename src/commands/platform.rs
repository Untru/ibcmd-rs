//! `--platform` on the commands that read or write XML: the XML format the
//! platform registry gives the platform (8.3.x -> 2.20, 8.5.x -> 2.21)
//! replaces the hidden `--source-version`, which the flag conflicts with.

use crate::cli::{
    AuditEmptyStageArgs, AuditInterfaceWriterArgs, AuditMetadataCompilerArgs,
    AuditMetadataExportArgs, AuditNameIndexArgs, CfBootstrapArgs, CfExportArgs, CfLoadArgs,
    CfOverlayArgs, MssqlApplySourceChangeArgs, MssqlAuditSourceParityArgs, MssqlDumpConfigArgs,
    MssqlDumpExtensionArgs, MssqlLoadExtensionArgs, MssqlStageSourceObjectsArgs,
};

/// A command whose `--platform` names its XML format.
pub trait PlatformFlag {
    /// Replaces the command's XML selection with the format of the platform
    /// `--platform` names, when it names one.
    fn apply_platform_flag(&mut self);
}

/// `source_version: InfobaseConfigSourceVersion`.
macro_rules! xml_selector {
    ($($args:ty),* $(,)?) => {$(
        impl PlatformFlag for $args {
            fn apply_platform_flag(&mut self) {
                if let Some(platform) = self.platform {
                    crate::platform::note_export_platform(platform);
                    self.source_version = platform.xml_version();
                }
            }
        }
    )*};
}

/// `source_version: Option<InfobaseConfigSourceVersion>`.
macro_rules! optional_xml_selector {
    ($($args:ty),* $(,)?) => {$(
        impl PlatformFlag for $args {
            fn apply_platform_flag(&mut self) {
                if let Some(platform) = self.platform {
                    crate::platform::note_export_platform(platform);
                    self.source_version = Some(platform.xml_version());
                }
            }
        }
    )*};
}

/// `source_version: Option<String>`, the dialect as text.
macro_rules! optional_dialect_text {
    ($($args:ty),* $(,)?) => {$(
        impl PlatformFlag for $args {
            fn apply_platform_flag(&mut self) {
                if let Some(platform) = self.platform {
                    self.source_version = Some(platform.xml_version().as_str().to_string());
                }
            }
        }
    )*};
}

xml_selector!(
    CfExportArgs,
    CfOverlayArgs,
    CfBootstrapArgs,
    CfLoadArgs,
    AuditInterfaceWriterArgs,
    AuditNameIndexArgs,
    MssqlDumpConfigArgs,
    MssqlDumpExtensionArgs,
    MssqlLoadExtensionArgs,
    MssqlApplySourceChangeArgs,
);
optional_xml_selector!(MssqlAuditSourceParityArgs, MssqlStageSourceObjectsArgs);
optional_dialect_text!(AuditMetadataExportArgs, AuditEmptyStageArgs);

impl PlatformFlag for AuditMetadataCompilerArgs {
    fn apply_platform_flag(&mut self) {
        if let Some(platform) = self.platform {
            self.source_version = platform.xml_version().as_str().to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::cli::{Cli, Commands, InfobaseConfigSourceVersion};

    fn dump_config(arguments: &[&str]) -> MssqlDumpConfigArgs {
        let mut command_line = vec!["ibcmd-rs", "mssql-dump-config", "-o", "out"];
        command_line.extend_from_slice(arguments);
        match Cli::try_parse_from(command_line).unwrap().command {
            Commands::MssqlDumpConfig(mut args) => {
                args.apply_platform_flag();
                args
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn platform_names_the_xml_format() {
        for (platform, xml) in [
            ("8.3.27", InfobaseConfigSourceVersion::V2_20),
            ("8.3.27.2214", InfobaseConfigSourceVersion::V2_20),
            ("8.5.1", InfobaseConfigSourceVersion::V2_21),
            ("8.5.1.1150", InfobaseConfigSourceVersion::V2_21),
        ] {
            let args = dump_config(&["--platform", platform]);
            assert_eq!(args.source_version, xml, "{platform}");
            assert_eq!(args.platform.unwrap().display(), platform);
        }
        // The hidden alias still selects the format itself.
        let args = dump_config(&["--source-version", "2.21"]);
        assert_eq!(args.source_version, InfobaseConfigSourceVersion::V2_21);
        assert_eq!(args.platform, None);
        let args = dump_config(&[]);
        assert_eq!(args.source_version, InfobaseConfigSourceVersion::V2_20);
    }

    #[test]
    fn platform_refuses_unknown_versions_and_conflicts_with_the_alias() {
        let error = Cli::try_parse_from([
            "ibcmd-rs",
            "mssql-dump-config",
            "-o",
            "out",
            "--platform",
            "8.4.1",
        ])
        .unwrap_err()
        .to_string();
        assert!(error.contains("unknown platform release 8.4.1"), "{error}");
        let error = Cli::try_parse_from([
            "ibcmd-rs",
            "mssql-dump-config",
            "-o",
            "out",
            "--platform",
            "8.5.1",
            "--source-version",
            "2.20",
        ])
        .unwrap_err()
        .to_string();
        assert!(error.contains("cannot be used with"), "{error}");
    }

    #[test]
    fn every_command_with_the_flag_takes_it() {
        let parse = |arguments: &[&str]| Cli::try_parse_from(arguments).unwrap().command;
        match parse(&[
            "ibcmd-rs",
            "mssql-stage-source-objects",
            "--database",
            "x",
            "--source-root",
            "tree",
            "--platform",
            "8.5.1.1150",
        ]) {
            Commands::MssqlStageSourceObjects(mut args) => {
                args.apply_platform_flag();
                assert_eq!(
                    args.source_version,
                    Some(InfobaseConfigSourceVersion::V2_21)
                );
            }
            _ => unreachable!(),
        }
        match parse(&[
            "ibcmd-rs",
            "audit-empty-stage",
            "tree",
            "rows",
            "--platform",
            "8.5.1",
        ]) {
            Commands::AuditEmptyStage(mut args) => {
                args.apply_platform_flag();
                assert_eq!(args.source_version.as_deref(), Some("2.21"));
            }
            _ => unreachable!(),
        }
        match parse(&[
            "ibcmd-rs",
            "audit-metadata-compiler",
            "tree",
            "rows",
            "--platform",
            "8.5.1",
        ]) {
            Commands::AuditMetadataCompiler(mut args) => {
                args.apply_platform_flag();
                assert_eq!(args.source_version, "2.21");
            }
            _ => unreachable!(),
        }
    }
}
