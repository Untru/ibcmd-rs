use std::cell::Cell;

use clap::{CommandFactory, FromArgMatches};

use super::*;
use crate::cli::{Cli, Commands};
use crate::settings::tests::{Layers, Scratch};

/// The parsed command and its matches.
fn parse(arguments: &[&str]) -> (Commands, ArgMatches) {
    let mut command_line = vec!["ibcmd-rs"];
    command_line.extend_from_slice(arguments);
    let matches = Cli::command().try_get_matches_from(command_line).unwrap();
    let cli = Cli::from_arg_matches(&matches).unwrap();
    (cli.command, matches.subcommand().unwrap().1.clone())
}

fn tree(scratch: &Scratch, folder: &str, xml: &str) -> String {
    scratch.write(
        &format!("{folder}/Configuration.xml"),
        &format!(
            "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"{xml}\"><Configuration uuid=\"66193438-abc5-410b-a1f1-a204102d1a62\"><Properties/></Configuration></MetaDataObject>"
        ),
    );
    scratch.path().join(folder).to_string_lossy().into_owned()
}

const BINDINGS: &str = r#"db-server = "sql-from-file"

[[database]]
name = "ibcmd_rs_bsp_85*"
platform = "8.5.1.1150"

[[database]]
name = "ibcmd_rs_bsp_8327*"
platform = "8.3.27.2214"
"#;

#[test]
fn one_export_command_follows_each_database_binding() {
    let scratch = Scratch::new("export-bindings");
    scratch.write("cwd/ibcmd-rs.toml", BINDINGS);
    let mut layers = Layers::new(&scratch);
    // The binding beats the variable, so one variable cannot override it.
    layers
        .env
        .insert(crate::settings::ENV_PLATFORM, "8.3.27".into());
    let rows = scratch.path().join("rows").to_string_lossy().into_owned();
    let load = || layers.load();
    for (database, xml) in [
        (
            "ibcmd_rs_bsp_85_src_20260922",
            InfobaseConfigSourceVersion::V2_21,
        ),
        (
            "ibcmd_rs_bsp_8327_native_20260919",
            InfobaseConfigSourceVersion::V2_20,
        ),
        // No entry: the variable.
        ("ibcmd_rs_uha_8327", InfobaseConfigSourceVersion::V2_20),
    ] {
        let (command, matches) = parse(&[
            "mssql-dump-config",
            "--rows-dir",
            &rows,
            "--database",
            database,
            "-o",
            "out",
        ]);
        let Commands::MssqlDumpConfig(mut args) = command else {
            unreachable!()
        };
        prepare_dump_config_with(&mut args, &matches, &load).unwrap();
        assert_eq!(args.source_version, xml, "{database}");
        // An offline run connects nowhere: the server stays as given.
        assert_eq!(args.server, "localhost");
    }
}

#[test]
fn settings_are_read_only_when_something_is_left_open() {
    let calls = Cell::new(0);
    let load = || -> Result<Settings> {
        calls.set(calls.get() + 1);
        Ok(Settings::default())
    };
    let (command, matches) = parse(&[
        "mssql-dump-config",
        "--rows-dir",
        "rows",
        "-o",
        "out",
        "--source-version",
        "2.21",
    ]);
    let Commands::MssqlDumpConfig(mut args) = command else {
        unreachable!()
    };
    prepare_dump_config_with(&mut args, &matches, &load).unwrap();
    assert_eq!(args.source_version, InfobaseConfigSourceVersion::V2_21);
    let (command, matches) = parse(&[
        "mssql-dump-config",
        "--server",
        "sql01",
        "--sql-user",
        "sa",
        "--database",
        "x",
        "-o",
        "out",
        "--platform",
        "8.5.1.1150",
    ]);
    let Commands::MssqlDumpConfig(mut args) = command else {
        unreachable!()
    };
    prepare_dump_config_with(&mut args, &matches, &load).unwrap();
    assert_eq!(args.source_version, InfobaseConfigSourceVersion::V2_21);
    assert_eq!(calls.get(), 0);
}

#[test]
fn an_export_takes_its_server_and_login_from_the_settings_unless_given() {
    let scratch = Scratch::new("export-connection");
    scratch.write("cwd/ibcmd-rs.toml", BINDINGS);
    let mut layers = Layers::new(&scratch);
    layers
        .env
        .insert(crate::settings::ENV_DB_USER, "reader".into());
    let load = || layers.load();
    let (command, matches) = parse(&[
        "mssql-dump-config",
        "--database",
        "ibcmd_rs_bsp_85_x",
        "-o",
        "out",
    ]);
    let Commands::MssqlDumpConfig(mut args) = command else {
        unreachable!()
    };
    prepare_dump_config_with(&mut args, &matches, &load).unwrap();
    assert_eq!(args.server, "sql-from-file");
    assert_eq!(args.sql_user.as_deref(), Some("reader"));
    assert_eq!(args.source_version, InfobaseConfigSourceVersion::V2_21);

    let (command, matches) = parse(&[
        "mssql-dump-config",
        "--server",
        "sql01",
        "--sql-user",
        "sa",
        "--database",
        "ibcmd_rs_bsp_85_x",
        "-o",
        "out",
    ]);
    let Commands::MssqlDumpConfig(mut args) = command else {
        unreachable!()
    };
    prepare_dump_config_with(&mut args, &matches, &load).unwrap();
    assert_eq!(args.server, "sql01");
    assert_eq!(args.sql_user.as_deref(), Some("sa"));
}

#[test]
fn a_load_is_checked_against_its_tree() {
    let scratch = Scratch::new("load");
    scratch.write("cwd/ibcmd-rs.toml", BINDINGS);
    let tree_85 = tree(&scratch, "tree85", "2.21");
    let tree_8327 = tree(&scratch, "tree8327", "2.20");
    let layers = Layers::new(&scratch);
    let load = || layers.load();
    let stage = |database: &str, tree: &str, extra: &[&str]| {
        let mut arguments = vec![
            "mssql-stage-source-objects",
            "--database",
            database,
            "--source-root",
            tree,
        ];
        arguments.extend_from_slice(extra);
        let (command, matches) = parse(&arguments);
        let Commands::MssqlStageSourceObjects(mut args) = command else {
            unreachable!()
        };
        prepare_stage_source_objects_with(&mut args, &matches, &load).map(|()| args)
    };

    let args = stage("ibcmd_rs_bsp_85_empty", &tree_85, &[]).unwrap();
    assert_eq!(
        args.source_version,
        Some(InfobaseConfigSourceVersion::V2_21)
    );
    assert_eq!(args.server, "sql-from-file");
    // No entry, no platform in the file: the tree's own format.
    let args = stage("elsewhere", &tree_85, &[]).unwrap();
    assert_eq!(
        args.source_version,
        Some(InfobaseConfigSourceVersion::V2_21)
    );

    let error = format!(
        "{:#}",
        stage("ibcmd_rs_bsp_8327_empty", &tree_85, &[]).unwrap_err()
    );
    assert!(error.contains("is XML 2.21"), "{error}");
    assert!(
        error.contains("the platform of ibcmd_rs_bsp_8327_empty is 8.3.27.2214 (XML 2.20)"),
        "{error}"
    );
    let error = format!(
        "{:#}",
        stage("elsewhere", &tree_8327, &["--platform", "8.5.1"]).unwrap_err()
    );
    assert!(error.contains("is XML 2.20"), "{error}");
    // The hidden alias is taken as given, unchecked, as before.
    let args = stage("elsewhere", &tree_8327, &["--source-version", "2.21"]).unwrap();
    assert_eq!(
        args.source_version,
        Some(InfobaseConfigSourceVersion::V2_21)
    );

    // Nothing configured and no Configuration.xml: each file keeps its own
    // format, as before the settings.
    let bare = Scratch::new("load-bare");
    let parts = bare.path().join("parts").to_string_lossy().into_owned();
    std::fs::create_dir_all(&parts).unwrap();
    let nothing = || Ok(Settings::default());
    let (command, matches) = parse(&[
        "mssql-stage-source-objects",
        "--database",
        "x",
        "--source-root",
        &parts,
    ]);
    let Commands::MssqlStageSourceObjects(mut args) = command else {
        unreachable!()
    };
    prepare_stage_source_objects_with(&mut args, &matches, &nothing).unwrap();
    assert_eq!(args.source_version, None);
    assert_eq!(args.server, "localhost");
}

#[test]
fn a_platform_profile_names_the_xml_format_of_its_build() {
    let load = |extra: &[&str]| {
        let mut arguments = vec![
            "mssql-load-extension",
            "--platform-profile",
            "platform-8.3.27.2214",
            "--cluster-id",
            "5c1b0ca0-7e8b-4e2d-9a44-2f2f3ad3f6a1",
            "--infobase-id",
            "6d2c1db1-8f9c-4f3e-8b55-303f4be4a7b2",
            "--database",
            "x",
            "--all-extensions",
            "-i",
            "in",
        ];
        arguments.extend_from_slice(extra);
        let (command, matches) = parse(&arguments);
        let Commands::MssqlLoadExtension(mut args) = command else {
            unreachable!()
        };
        prepare_load_extension(&mut args, &matches).map(|()| args.source_version)
    };
    assert_eq!(load(&[]).unwrap(), InfobaseConfigSourceVersion::V2_20);
    assert_eq!(
        load(&["--platform", "8.3.27"]).unwrap(),
        InfobaseConfigSourceVersion::V2_20
    );
    assert_eq!(
        load(&["--source-version", "2.21"]).unwrap(),
        InfobaseConfigSourceVersion::V2_21
    );
    let error = format!("{:#}", load(&["--platform", "8.5.1"]).unwrap_err());
    assert!(
        error.contains(
            "--platform 8.5.1 is not the build of --platform-profile platform-8.3.27.2214"
        ),
        "{error}"
    );
}
