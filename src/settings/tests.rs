use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::*;

/// A scratch folder removed on drop.
pub(crate) struct Scratch(PathBuf);

impl Scratch {
    pub(crate) fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-settings-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }

    /// Writes `text` at `relative`, creating its folders.
    pub(crate) fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The layers of a scratch folder: `exe/`, `appdata/ibcmd-rs/`, `cwd/`.
pub(crate) struct Layers<'a> {
    pub(crate) scratch: &'a Scratch,
    pub(crate) env: HashMap<&'static str, String>,
    pub(crate) native: Option<PathBuf>,
    pub(crate) executable: Option<PathBuf>,
}

impl<'a> Layers<'a> {
    pub(crate) fn new(scratch: &'a Scratch) -> Self {
        Self {
            scratch,
            env: HashMap::new(),
            native: None,
            executable: Some(scratch.path().join("exe").join("ibcmd-rs.exe")),
        }
    }

    pub(crate) fn load(&self) -> Result<Settings> {
        let env = |name: &str| self.env.get(name).cloned();
        Settings::from_sources(&SettingsSources {
            env: &env,
            executable: self.executable.clone(),
            current_dir: Some(self.scratch.path().join("cwd")),
            app_data: Some(self.scratch.path().join("appdata")),
            native_config: self.native.as_deref(),
        })
    }
}

fn target<'a>(server: Option<&'a str>, name: &'a str) -> Option<DatabaseTarget<'a>> {
    DatabaseTarget::new(server, name)
}

fn resolved(
    settings: &Settings,
    explicit: Option<&str>,
    target: Option<DatabaseTarget<'_>>,
    hint: PlatformHint<'_>,
) -> (String, SettingSource) {
    let resolved = resolve_platform_with_source(explicit, settings, target, hint).unwrap();
    (resolved.value.to_string(), resolved.source)
}

#[test]
fn masks_match_whole_names_ignoring_case() {
    for (mask, name) in [
        ("erp_prod", "erp_prod"),
        ("erp_prod", "ERP_Prod"),
        ("bsp_*", "bsp_85"),
        ("bsp_*", "bsp_"),
        ("*_85", "ibcmd_rs_bsp_85"),
        ("*bsp*", "ibcmd_rs_bsp_8327_native"),
        ("ibcmd_rs_*_85_*", "ibcmd_rs_uha_85_src_20260922"),
        ("*", "anything"),
        ("a*b*c", "aXXbYYc"),
        ("a*b*c", "abc"),
        ("БСП_*", "бсп_тест"),
    ] {
        assert!(mask_matches(mask, name), "{mask} ~ {name}");
    }
    for (mask, name) in [
        ("erp_prod", "erp_prod2"),
        ("erp_prod", "xerp_prod"),
        ("bsp_*", "bs_85"),
        ("*_85", "bsp_851"),
        ("a*b*c", "aXXbYY"),
        ("ibcmd_rs_*_85_*", "ibcmd_rs_uha_8327_native"),
    ] {
        assert!(!mask_matches(mask, name), "{mask} !~ {name}");
    }
}

#[test]
fn an_entry_without_a_server_matches_every_server() {
    let binding = |server: Option<&str>| DatabaseBinding {
        server: server.map(str::to_string),
        name: "bsp_*".to_string(),
        platform: crate::platform::parse("8.5.1").unwrap(),
        line: 1,
        rows_dir: None,
    };
    let any = binding(None);
    assert!(any.matches(target(Some("sql01"), "bsp_1").unwrap()));
    assert!(any.matches(target(None, "bsp_1").unwrap()));
    let sql02 = binding(Some("SQL02"));
    assert!(sql02.matches(target(Some("sql02"), "bsp_1").unwrap()));
    assert!(!sql02.matches(target(Some("sql01"), "bsp_1").unwrap()));
    // A command that names no server cannot prove it is on sql02.
    assert!(!sql02.matches(target(None, "bsp_1").unwrap()));
    assert_eq!(target(Some(" "), " x ").unwrap().server, None);
    assert_eq!(target(Some("s"), " x ").unwrap().name, "x");
    assert!(target(Some("s"), "  ").is_none());
}

#[test]
fn the_platform_follows_the_documented_order() {
    let scratch = Scratch::new("order");
    scratch.write(
        "cwd/ibcmd-rs.toml",
        r#"platform = "8.3.27.2214"

[[database]]
name = "bsp_*"
platform = "8.5.1.1150"
"#,
    );
    scratch.write(
        "tree85/Configuration.xml",
        "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"2.21\"><Configuration uuid=\"66193438-abc5-410b-a1f1-a204102d1a62\"><Properties/></Configuration></MetaDataObject>",
    );
    let mut layers = Layers::new(&scratch);
    layers.env.insert(ENV_PLATFORM, "8.3.27.1989".to_string());
    let settings = layers.load().unwrap();
    let bsp = target(Some("localhost"), "bsp_85");
    let other = target(Some("localhost"), "erp");
    let none = PlatformHint::None;

    // 1. the flag
    let (platform, source) = resolved(&settings, Some("8.3.27"), bsp, none);
    assert_eq!(platform, "8.3.27");
    assert_eq!(
        source,
        SettingSource::Flag {
            flag: "--platform".into()
        }
    );
    // 2. the [[database]] entry, above the environment
    let (platform, source) = resolved(&settings, None, bsp, none);
    assert_eq!(platform, "8.5.1.1150");
    assert!(
        matches!(&source, SettingSource::Database { line: 5, name, server: None, .. } if name == "bsp_*"),
        "{source:?}"
    );
    // 3. the environment for a database no entry names
    let (platform, source) = resolved(&settings, None, other, none);
    assert_eq!(platform, "8.3.27.1989");
    assert_eq!(
        source,
        SettingSource::Environment {
            variable: ENV_PLATFORM.into()
        }
    );
    // 4. the file's own platform without the variable
    layers.env.clear();
    let settings = layers.load().unwrap();
    let (platform, source) = resolved(&settings, None, other, none);
    assert_eq!(platform, "8.3.27.2214");
    assert!(
        matches!(&source, SettingSource::File { line: 1, path } if path.ends_with("cwd/ibcmd-rs.toml")),
        "{source:?}"
    );
    // 5. auto-detection without the file
    std::fs::remove_file(scratch.path().join("cwd/ibcmd-rs.toml")).unwrap();
    let settings = layers.load().unwrap();
    let tree = scratch.path().join("tree85");
    let (platform, source) = resolved(
        &settings,
        None,
        bsp,
        PlatformHint::Import { source_root: &tree },
    );
    assert_eq!(platform, "8.5.1");
    assert!(
        matches!(source, SettingSource::SourceTree { .. }),
        "{source:?}"
    );
    let yes = || Ok(true);
    let (platform, source) = resolved(
        &settings,
        None,
        bsp,
        PlatformHint::Export {
            compatibility_8_5_or_later: &yes,
        },
    );
    assert_eq!(platform, "8.5.1");
    assert_eq!(source, SettingSource::Compatibility);
    // 6. the default
    let no = || Ok(false);
    let (platform, source) = resolved(
        &settings,
        None,
        bsp,
        PlatformHint::Export {
            compatibility_8_5_or_later: &no,
        },
    );
    assert_eq!(platform, "8.3.27");
    assert_eq!(source, SettingSource::Default);
    let (platform, source) = resolved(
        &settings,
        None,
        None,
        PlatformHint::Import {
            source_root: scratch.path(),
        },
    );
    assert_eq!(
        (platform.as_str(), source),
        ("8.3.27", SettingSource::Default)
    );
}

#[test]
fn the_export_probe_runs_only_when_nothing_else_decides() {
    let scratch = Scratch::new("probe");
    let mut layers = Layers::new(&scratch);
    let calls = Cell::new(0);
    let probe = || {
        calls.set(calls.get() + 1);
        Ok(true)
    };
    let export = PlatformHint::Export {
        compatibility_8_5_or_later: &probe,
    };
    layers.env.insert(ENV_PLATFORM, "8.3.27".to_string());
    let settings = layers.load().unwrap();
    resolve_platform_with_source(None, &settings, None, export).unwrap();
    resolve_platform_with_source(Some("8.5.1"), &settings, None, export).unwrap();
    assert_eq!(calls.get(), 0);
    layers.env.clear();
    let settings = layers.load().unwrap();
    resolve_platform_with_source(None, &settings, None, export).unwrap();
    assert_eq!(calls.get(), 1);
    let failing = || Err(anyhow::anyhow!("no rows"));
    let error = resolve_platform_with_source(
        None,
        &settings,
        None,
        PlatformHint::Export {
            compatibility_8_5_or_later: &failing,
        },
    )
    .unwrap_err();
    assert!(
        format!("{error:#}").contains("compatibility mode: no rows"),
        "{error:#}"
    );
}

#[test]
fn a_database_entry_may_name_a_rows_folder_relative_to_its_file() {
    let scratch = Scratch::new("rows-dir");
    scratch.write(
        "cwd/ibcmd-rs.toml",
        r#"[[database]]
name = "demo"
platform = "8.3.27"
rows-dir = "rows/demo"

[[database]]
name = "prod"
platform = "8.3.27"
"#,
    );
    let settings = Layers::new(&scratch).load().unwrap();
    let databases = settings.files()[0].databases();
    assert_eq!(
        databases[0].rows_dir.as_deref(),
        Some(scratch.path().join("cwd").join("rows/demo").as_path())
    );
    assert_eq!(databases[1].rows_dir, None);

    scratch.write(
        "cwd/ibcmd-rs.toml",
        "[[database]]\nname = \"demo\"\nplatform = \"8.3.27\"\nrows-dir = \" \"\n",
    );
    let error = Layers::new(&scratch).load().unwrap_err();
    assert!(
        format!("{error:#}").contains(":4: [[database]] rows-dir is empty"),
        "{error:#}"
    );
}

#[test]
fn later_layers_win_and_their_entries_are_searched_first() {
    let scratch = Scratch::new("layers");
    scratch.write(
        "exe/ibcmd-rs.toml",
        r#"platform = "8.3.27.1989"
db-server = "exe-server"
db-user = "exe-user"

[[database]]
name = "shared"
platform = "8.3.27.1989"

[[database]]
name = "exe_only"
platform = "8.3.27.1989"
"#,
    );
    scratch.write(
        "appdata/ibcmd-rs/ibcmd-rs.toml",
        r#"db-server = "appdata-server"

[[database]]
name = "shared"
platform = "8.3.27.2214"
"#,
    );
    scratch.write(
        "cwd/ibcmd-rs.toml",
        r#"db-user = "cwd-user"

[[database]]
name = "sha*"
platform = "8.5.1.1150"

[[database]]
name = "shared"
platform = "8.3.27"
"#,
    );
    let layers = Layers::new(&scratch);
    let settings = layers.load().unwrap();
    assert_eq!(
        settings
            .files()
            .iter()
            .map(|file| file.layer)
            .collect::<Vec<_>>(),
        [
            SettingsLayer::NextToExecutable,
            SettingsLayer::AppData,
            SettingsLayer::CurrentDirectory
        ]
    );
    assert_eq!(settings.db_server().unwrap().value, "appdata-server");
    assert_eq!(settings.db_user().unwrap().value, "cwd-user");
    assert_eq!(
        settings.file_platform().unwrap().value.display(),
        "8.3.27.1989"
    );
    // The current directory's file first, its first match in file order.
    let shared = settings
        .database_binding(target(None, "shared").unwrap())
        .unwrap();
    assert_eq!(shared.value.display(), "8.5.1.1150");
    assert!(
        matches!(&shared.source, SettingSource::Database { line: 5, name, .. } if name == "sha*"),
        "{:?}",
        shared.source
    );
    let exe_only = settings
        .database_binding(target(None, "exe_only").unwrap())
        .unwrap();
    assert_eq!(exe_only.value.display(), "8.3.27.1989");
    assert!(
        settings
            .database_binding(target(None, "other").unwrap())
            .is_none()
    );
}

#[test]
fn ibcmd_rs_config_names_the_only_file() {
    let scratch = Scratch::new("explicit");
    scratch.write("cwd/ibcmd-rs.toml", "platform = \"8.5.1\"\n");
    let explicit = scratch.write("elsewhere/team.toml", "platform = \"8.3.27.2214\"\n");
    let mut layers = Layers::new(&scratch);
    layers
        .env
        .insert(ENV_CONFIG, explicit.to_string_lossy().into_owned());
    let settings = layers.load().unwrap();
    assert_eq!(settings.files().len(), 1);
    assert_eq!(settings.files()[0].layer, SettingsLayer::Explicit);
    assert_eq!(
        settings.file_platform().unwrap().value.display(),
        "8.3.27.2214"
    );
    assert!(settings.absent_files().is_empty());

    layers.env.insert(
        ENV_CONFIG,
        scratch
            .path()
            .join("missing.toml")
            .to_string_lossy()
            .into_owned(),
    );
    let error = format!("{:#}", layers.load().unwrap_err());
    assert!(error.contains("IBCMD_RS_CONFIG names"), "{error}");
    assert!(error.contains("missing.toml"), "{error}");
}

#[test]
fn a_bad_file_fails_naming_the_file_and_line() {
    for (text, expected) in [
        ("platform = \"8.4.1\"\n", ":1: platform = \"8.4.1\""),
        (
            "db-server = \"s\"\n\n[[database]]\nname = \"x\"\nplatform = \"8.3.27.9999\"\n",
            ":5: [[database]] platform = \"8.3.27.9999\"",
        ),
        (
            "[[database]]\nname = \"x\"\nplatfrom = \"8.5.1\"\n",
            "unknown field `platfrom`",
        ),
        ("colour = \"red\"\n", "unknown field `colour`"),
        (
            "[[database]]\nplatform = \"8.5.1\"\n",
            "missing field `name`",
        ),
        (
            "[[database]]\nname = \" \"\nplatform = \"8.5.1\"\n",
            ":2: [[database]] name is empty",
        ),
        ("db-password = \"secret\"\n", "unknown field `db-password`"),
        ("platform = 8.5\n", "line 1"),
        ("platform = \"8.5.1\"\nplatform = \"8.3.27\"\n", "line 2"),
    ] {
        let scratch = Scratch::new("bad");
        scratch.write("cwd/ibcmd-rs.toml", text);
        let error = format!("{:#}", Layers::new(&scratch).load().unwrap_err());
        assert!(error.contains("ibcmd-rs.toml"), "{text:?}: {error}");
        assert!(error.contains(expected), "{text:?}: {error}");
    }
    let scratch = Scratch::new("bad-env");
    let mut layers = Layers::new(&scratch);
    layers.env.insert(ENV_PLATFORM, "8.4".to_string());
    let error = format!("{:#}", layers.load().unwrap_err());
    assert!(error.contains("IBCMD_RS_PLATFORM=`8.4`"), "{error}");
    assert!(error.contains("is not a platform version"), "{error}");
}

#[test]
fn connection_settings_come_from_the_environment_then_the_files_then_native_config() {
    let scratch = Scratch::new("connection");
    let native = scratch.write(
        "native.yml",
        "server:\n  address: localhost\n  port: 8314\ndatabase:\n  dbms: MSSQLServer\n  server: native-server\n  name: erp_prod\n  user: native-user\n  password: native-secret\ninfobase:\n  id: ef77488f-c437-497f-ab57-2507e5f1fe80\n  distribute-licenses: yes\nhttp:\n  base: /\n",
    );
    let mut layers = Layers::new(&scratch);
    layers.native = Some(native.clone());
    let settings = layers.load().unwrap();
    let native_source = |key: &str| SettingSource::NativeConfig {
        path: native.clone(),
        key: key.into(),
    };
    assert_eq!(
        settings.db_server(),
        Some(Sourced::new(
            "native-server".into(),
            native_source("server")
        ))
    );
    assert_eq!(settings.db_name().unwrap().value, "erp_prod");
    assert_eq!(settings.db_user().unwrap().value, "native-user");
    assert_eq!(settings.db_password().unwrap().value, "native-secret");
    assert_eq!(settings.dbms().unwrap().value, "MSSQLServer");

    scratch.write(
        "cwd/ibcmd-rs.toml",
        "db-server = \"file-server\"\ndb-user = \"file-user\"\n",
    );
    let settings = layers.load().unwrap();
    assert_eq!(settings.db_server().unwrap().value, "file-server");
    assert_eq!(settings.db_user().unwrap().value, "file-user");
    // No settings file holds a password.
    assert_eq!(
        settings.db_password().unwrap().source,
        native_source("password")
    );

    layers.env.insert(ENV_DB_SERVER, "env-server".into());
    layers.env.insert(ENV_DB_USER, "env-user".into());
    layers.env.insert(ENV_DB_PASSWORD, "env-secret".into());
    let settings = layers.load().unwrap();
    for (value, variable) in [
        (settings.db_server(), ENV_DB_SERVER),
        (settings.db_user(), ENV_DB_USER),
        (settings.db_password(), ENV_DB_PASSWORD),
    ] {
        assert_eq!(
            value.unwrap().source,
            SettingSource::Environment {
                variable: variable.into()
            }
        );
    }
    assert_eq!(settings.db_server().unwrap().value, "env-server");
    // An empty variable is not set.
    layers.env.insert(ENV_DB_SERVER, " ".into());
    assert_eq!(
        layers.load().unwrap().db_server().unwrap().value,
        "file-server"
    );
}

#[test]
fn native_config_reads_only_its_database_section() {
    let scratch = Scratch::new("native");
    for (text, name) in [
        ("database:\n  name: 1234\n  server: host\n", Some("1234")),
        ("server:\n  port: 8314\n", None),
        ("database:\n", None),
        ("database: ~\ninfobase:\n  name: x\n", None),
    ] {
        let path = scratch.write("native.yml", text);
        let config = files::read_native_config(&path).unwrap();
        assert_eq!(config.name.as_deref(), name, "{text:?}");
    }
    for (text, expected) in [
        (
            "database:\n  name: [a, b]\n",
            "database.name is not a single value",
        ),
        ("database: [1, 2]\n", "`database` is not a section"),
        ("database:\n  name: \"x\n", "native.yml"),
    ] {
        let path = scratch.write("native.yml", text);
        let error = format!("{:#}", files::read_native_config(&path).unwrap_err());
        assert!(error.contains(expected), "{text:?}: {error}");
    }
}

#[test]
fn the_platform_folder_of_the_executable_names_the_platform() {
    let scratch = Scratch::new("exe");
    let mut layers = Layers::new(&scratch);
    let folder = |path: &str| platform_folder(Path::new(path));
    assert_eq!(
        folder("C:/Program Files/1cv8/8.3.27.2214/bin/ibcmd-rs.exe").as_deref(),
        Some("8.3.27.2214")
    );
    assert_eq!(
        folder("/opt/1cv8/x86_64/8.5.1.1150/bin/ibcmd-rs"),
        None,
        "the release folder must sit right under 1cv8"
    );
    assert_eq!(
        folder("/opt/1CV8/8.5.1/BIN/ibcmd-rs").as_deref(),
        Some("8.5.1")
    );
    assert_eq!(folder("C:/Program Files/1cv8/common/ibcmd-rs.exe"), None);
    assert_eq!(
        folder("C:/Program Files/1cv8/8.3.27.2214/ibcmd-rs.exe"),
        None
    );
    assert_eq!(folder("C:/tools/ibcmd-rs.exe"), None);

    layers.executable = Some(PathBuf::from(
        "C:/Program Files/1cv8/8.5.1.1150/bin/ibcmd-rs.exe",
    ));
    let settings = layers.load().unwrap();
    let (platform, source) = resolved(&settings, None, None, PlatformHint::None);
    assert_eq!(platform, "8.5.1.1150");
    assert!(
        matches!(source, SettingSource::ExecutableDirectory { .. }),
        "{source:?}"
    );

    layers.executable = Some(PathBuf::from(
        "C:/Program Files/1cv8/8.3.24.1819/bin/ibcmd-rs.exe",
    ));
    let settings = layers.load().unwrap();
    let error = format!(
        "{:#}",
        resolve_platform_with_source(None, &settings, None, PlatformHint::None).unwrap_err()
    );
    assert!(error.contains("folder of platform 8.3.24.1819"), "{error}");
    assert!(error.contains("not supported"), "{error}");
}

#[test]
fn a_tree_in_an_unknown_xml_format_is_refused() {
    let scratch = Scratch::new("tree");
    scratch.write(
        "Configuration.xml",
        "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\" version=\"2.17\"><Configuration uuid=\"66193438-abc5-410b-a1f1-a204102d1a62\"><Properties/></Configuration></MetaDataObject>",
    );
    let error = format!(
        "{:#}",
        resolve_platform_with_source(
            None,
            &Settings::default(),
            None,
            PlatformHint::Import {
                source_root: scratch.path()
            }
        )
        .unwrap_err()
    );
    assert!(error.contains("declares XML 2.17"), "{error}");
}

#[test]
fn settings_show_names_every_source_and_never_the_password() {
    let scratch = Scratch::new("show");
    scratch.write(
        "cwd/ibcmd-rs.toml",
        r#"platform = "8.3.27.2214"
db-user = "sa"

[[database]]
server = "sql02"
name = "bsp_*"
platform = "8.5.1.1150"
"#,
    );
    let mut layers = Layers::new(&scratch);
    layers.env.insert(ENV_DB_PASSWORD, "top-secret".into());
    let settings = layers.load().unwrap();
    let report = show::settings_report(&settings, None, Some("sql02"), Some("bsp_85")).unwrap();
    assert_eq!(report.platform.version.display(), "8.5.1.1150");
    assert_eq!(report.platform.xml_version, "2.21");
    assert_eq!(report.databases.len(), 1);
    assert!(report.databases[0].matches);
    assert_eq!(report.files.len(), 3);
    assert_eq!(report.files.iter().filter(|file| file.read).count(), 1);
    let text = show::render(&report);
    assert!(
        text.contains("8.5.1.1150 (XML 2.21, form layout 8.5.1)"),
        "{text}"
    );
    assert!(
        text.contains("[[database]] name = \"bsp_*\", server = \"sql02\""),
        "{text}"
    );
    assert!(text.contains("db-password  (set)"), "{text}");
    assert!(text.contains("--db-server"), "{text}");
    let json = serde_json::to_string(&report).unwrap();
    assert!(!text.contains("top-secret") && !json.contains("top-secret"));

    // Another server: the file's own platform.
    let report = show::settings_report(&settings, None, Some("sql01"), Some("bsp_85")).unwrap();
    assert_eq!(report.platform.version.display(), "8.3.27.2214");
    assert!(!report.databases[0].matches);
    // --platform wins over everything.
    let report =
        show::settings_report(&settings, Some("8.5.1"), Some("sql02"), Some("bsp_85")).unwrap();
    assert_eq!(report.platform.version.display(), "8.5.1");
}
