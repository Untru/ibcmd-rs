//! The platform ibcmd's `infobase` mode, parsed the way the platform's ibcmd
//! parses it (measured against 8.3.27.2214):
//!
//! - after the mode come command words (`config`, `export`, ...), options
//!   and arguments in any order;
//! - an option is `--name=value`, `--name value` or `-x value` (`-xvalue`
//!   is an error), names are case-sensitive;
//! - an option is known only from its own command on: the mode's options
//!   (`--dbms`, `--db-name`, ...) anywhere after the mode, `config`'s
//!   (`--user`, `--password`) after `config`, `export`'s after `export`.
//!
//! The whole native command tree is known, so every native command is
//! recognized: `config export`, `config import`, `config import files`,
//! `config apply` and `config save` are served, everything else is refused
//! by name (see `super`).

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use crate::cli::InfobaseConfigSourceVersion;
use crate::mssql_config_apply::BackupPolicy;
use crate::platform::PlatformSpec;

/// One option of the grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Opt {
    // common to every mode
    Help,
    Version,
    Pid,
    Remote,
    // the infobase mode
    Config,
    System,
    Dbms,
    DbServer,
    DbName,
    DbUser,
    DbPwd,
    RequestDbPwd,
    DbPath,
    Data,
    Lock,
    Temp,
    UsersData,
    SessionData,
    SttData,
    LogData,
    FtextData,
    Ftext2Data,
    OpenidData,
    BinDataStrg,
    ClntNotifData,
    WebsocketData,
    // `config`
    User,
    Password,
    // `config export`
    Base,
    File,
    Extension,
    Sync,
    Force,
    Threads,
    Archive,
    IgnoreUnresolvedRefs,
    Short,
    Recursive,
    // `config import`
    Out,
    // `config import files`
    BaseDir,
    Partial,
    NoCheck,
    // `config save`
    Db,
    // `config apply`
    Dynamic,
    SessionTerminate,
    SessionTerminateMessage,
    // ibcmd-rs's own (no native spelling)
    Report,
    Platform,
    Sqlcmd,
    Settings,
    SourceVersion,
    ChartSeriesNames,
    DbPwdEnv,
    BaseFree,
    Exclusivity,
    RecoveryBackup,
    IHaveABackup,
    Verify,
    NoVerify,
    Rac,
    RasEndpoint,
    ClusterId,
    InfobaseId,
}

/// How an option is spelled and whether it takes a value.
#[derive(Debug, Clone, Copy)]
pub struct OptSpec {
    pub opt: Opt,
    /// Long names, the first is the one messages use.
    pub long: &'static [&'static str],
    pub short: Option<char>,
    pub value: bool,
}

const fn flag(opt: Opt, long: &'static [&'static str], short: Option<char>) -> OptSpec {
    OptSpec {
        opt,
        long,
        short,
        value: false,
    }
}

const fn valued(opt: Opt, long: &'static [&'static str], short: Option<char>) -> OptSpec {
    OptSpec {
        opt,
        long,
        short,
        value: true,
    }
}

/// `ibcmd --help`, `--version`, `--pid`, `--remote`: every mode has them.
pub const GLOBAL_OPTIONS: &[OptSpec] = &[
    flag(Opt::Help, &["help"], Some('h')),
    flag(Opt::Help, &[], Some('?')),
    flag(Opt::Version, &["version"], Some('v')),
    valued(Opt::Pid, &["pid"], Some('p')),
    valued(Opt::Remote, &["remote"], Some('r')),
];

/// The infobase mode's own parameters, plus ibcmd-rs's.
pub const MODE_OPTIONS: &[OptSpec] = &[
    valued(Opt::Config, &["config"], Some('c')),
    valued(Opt::System, &["system"], None),
    valued(Opt::Dbms, &["dbms"], None),
    valued(Opt::DbServer, &["database-server", "db-server"], None),
    valued(Opt::DbName, &["database-name", "db-name"], None),
    valued(Opt::DbUser, &["database-user", "db-user"], None),
    valued(Opt::DbPwd, &["database-password", "db-pwd"], None),
    flag(
        Opt::RequestDbPwd,
        &["request-database-password", "request-db-pwd"],
        Some('W'),
    ),
    valued(Opt::DbPath, &["database-path", "db-path"], None),
    valued(Opt::Data, &["data"], Some('d')),
    valued(Opt::Lock, &["lock"], None),
    valued(Opt::Temp, &["temp"], Some('t')),
    valued(Opt::UsersData, &["users-data"], None),
    valued(Opt::SessionData, &["session-data"], None),
    valued(Opt::SttData, &["stt-data"], None),
    valued(Opt::LogData, &["log-data"], None),
    valued(Opt::FtextData, &["ftext-data"], None),
    valued(Opt::Ftext2Data, &["ftext2-data"], None),
    valued(Opt::OpenidData, &["openid-data"], None),
    valued(Opt::BinDataStrg, &["bin-data-strg"], None),
    valued(Opt::ClntNotifData, &["clnt-notif-data"], None),
    valued(Opt::WebsocketData, &["websocket-data"], None),
    valued(Opt::Report, &["report"], None),
    valued(Opt::Sqlcmd, &["sqlcmd"], None),
    valued(Opt::Settings, &["settings"], None),
    valued(Opt::Platform, &["platform"], None),
    valued(Opt::SourceVersion, &["source-version"], None),
    valued(Opt::DbPwdEnv, &["db-pwd-env"], None),
    valued(Opt::Rac, &["rac"], None),
    valued(Opt::RasEndpoint, &["ras-endpoint"], None),
    valued(Opt::ClusterId, &["cluster-id"], None),
    valued(Opt::InfobaseId, &["infobase-id"], None),
];

const CONFIG_OPTIONS: &[OptSpec] = &[
    valued(Opt::User, &["user"], Some('u')),
    valued(Opt::Password, &["password"], Some('P')),
];

const EXPORT_OPTIONS: &[OptSpec] = &[
    valued(Opt::ChartSeriesNames, &["chart-series-names"], None),
    valued(Opt::Base, &["base"], Some('b')),
    valued(Opt::File, &["file"], Some('f')),
    valued(Opt::Extension, &["extension"], Some('e')),
    flag(Opt::Sync, &["sync"], None),
    flag(Opt::Force, &["force"], None),
    valued(Opt::Threads, &["threads"], Some('T')),
    flag(Opt::Archive, &["archive"], Some('A')),
    flag(Opt::IgnoreUnresolvedRefs, &["ignore-unresolved-refs"], None),
];

const EXPORT_INFO_OPTIONS: &[OptSpec] = &[valued(Opt::Out, &["out"], Some('o'))];
const EXPORT_STATUS_OPTIONS: &[OptSpec] = &[
    valued(Opt::Out, &["out"], Some('o')),
    flag(Opt::Short, &["short"], Some('s')),
];
const EXPORT_OBJECTS_OPTIONS: &[OptSpec] = &[
    valued(Opt::Out, &["out"], Some('o')),
    flag(Opt::Recursive, &["recursive"], Some('r')),
];

const IMPORT_OPTIONS: &[OptSpec] = &[
    valued(Opt::Out, &["out"], Some('o')),
    valued(Opt::Extension, &["extension"], Some('e')),
    flag(Opt::BaseFree, &["base-free"], None),
    flag(Opt::Verify, &["verify"], None),
    flag(Opt::NoVerify, &["no-verify"], None),
];

/// `config import files --base-dir=<dir> [--partial] [--no-check] <files>`:
/// the options the lab's scripts give the platform
/// (`scripts/apply-trace/lab/validate_sparse_import.ps1`,
/// `scripts/restructure-lab/import_files.ps1`) and the issue's syntax
/// (Untru/ibcmd-rs#363). The platform's own help for them is not recorded.
const IMPORT_FILES_OPTIONS: &[OptSpec] = &[
    valued(Opt::BaseDir, &["base-dir"], None),
    flag(Opt::Partial, &["partial"], None),
    flag(Opt::NoCheck, &["no-check"], None),
];

/// `config save [--db] [--extension=<name>] <path>`: the native options (the
/// issue's syntax, Untru/ibcmd-rs#352); `--extension` is refused by name.
const SAVE_OPTIONS: &[OptSpec] = &[
    flag(Opt::Db, &["db"], None),
    valued(Opt::Extension, &["extension"], Some('e')),
];

/// `config load`: the main configuration replaced by a `.cf`; an
/// extension's is refused.
const LOAD_OPTIONS: &[OptSpec] = &[valued(Opt::Extension, &["extension"], Some('e'))];

const APPLY_OPTIONS: &[OptSpec] = &[
    valued(Opt::Extension, &["extension"], Some('e')),
    flag(Opt::Force, &["force"], Some('F')),
    valued(Opt::Dynamic, &["dynamic"], None),
    valued(Opt::SessionTerminate, &["session-terminate"], None),
    valued(
        Opt::SessionTerminateMessage,
        &["session-terminate-message"],
        None,
    ),
    valued(Opt::Exclusivity, &["exclusivity"], None),
    valued(Opt::RecoveryBackup, &["recovery-backup"], None),
    flag(Opt::IHaveABackup, &["i-have-a-backup"], None),
];

/// What a command word leads to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Needs one of its subcommands.
    Group,
    /// `infobase config export`.
    Export,
    ExportInfo,
    ExportStatus,
    ExportObjects,
    /// `infobase config import`.
    Import,
    /// `infobase config import files`.
    ImportFiles,
    /// `infobase config apply`.
    Apply,
    /// `infobase config save`.
    Save,
    /// `infobase config load`.
    Load,
    /// A native command ibcmd-rs does not implement yet.
    Unsupported,
}

/// One command of the tree.
#[derive(Debug)]
pub struct Node {
    pub name: &'static str,
    /// The platform's own one-line description.
    pub summary: &'static str,
    pub kind: NodeKind,
    pub options: &'static [OptSpec],
    pub children: &'static [Node],
}

const fn unsupported(name: &'static str, summary: &'static str) -> Node {
    Node {
        name,
        summary,
        kind: NodeKind::Unsupported,
        options: &[],
        children: &[],
    }
}

/// The infobase mode as the platform's `ibcmd help infobase` lists it.
pub static INFOBASE: Node = Node {
    name: "infobase",
    summary: "Режим управления информационной базой",
    kind: NodeKind::Group,
    options: MODE_OPTIONS,
    children: &[
        unsupported("create", "Создание информационной базы"),
        unsupported("dump", "Выгрузка данных информационной базы"),
        unsupported("restore", "Загрузка данных информационной базы"),
        unsupported("clear", "Очистка информационной базы"),
        unsupported("replicate", "Репликация информационной базы"),
        Node {
            name: "config",
            summary: "Управление конфигурацией информационной базы",
            kind: NodeKind::Group,
            options: CONFIG_OPTIONS,
            children: &[
                Node {
                    name: "load",
                    summary: "Загрузка конфигурации",
                    kind: NodeKind::Load,
                    options: LOAD_OPTIONS,
                    children: &[],
                },
                Node {
                    name: "save",
                    summary: "Выгрузка конфигурации",
                    kind: NodeKind::Save,
                    options: SAVE_OPTIONS,
                    children: &[],
                },
                unsupported("check", "Проверка конфигурации"),
                Node {
                    name: "apply",
                    summary: "Обновление конфигурации базы данных",
                    kind: NodeKind::Apply,
                    options: APPLY_OPTIONS,
                    children: &[],
                },
                unsupported("reset", "Возврат к конфигурации базы данных"),
                unsupported(
                    "repair",
                    "Восстановление конфигурации после незавершенной операции",
                ),
                Node {
                    name: "export",
                    summary: "Экспорт конфигурации в XML",
                    kind: NodeKind::Export,
                    options: EXPORT_OPTIONS,
                    children: &[
                        Node {
                            name: "info",
                            summary: "Вывести информацию о состоянии конфигурации (ConfigDumpInfo)",
                            kind: NodeKind::ExportInfo,
                            options: EXPORT_INFO_OPTIONS,
                            children: &[],
                        },
                        Node {
                            name: "status",
                            summary: "Вывести информацию о изменениях конфигурации относительно переданного состояния (ConfigDumpInfo)",
                            kind: NodeKind::ExportStatus,
                            options: EXPORT_STATUS_OPTIONS,
                            children: &[],
                        },
                        Node {
                            name: "objects",
                            summary: "Экспорт выбранных объектов конфигурации в XML",
                            kind: NodeKind::ExportObjects,
                            options: EXPORT_OBJECTS_OPTIONS,
                            children: &[],
                        },
                        unsupported(
                            "all-extensions",
                            "Экспорт всех расширений конфигурации в XML",
                        ),
                    ],
                },
                Node {
                    name: "import",
                    summary: "Импорт конфигурации из XML",
                    kind: NodeKind::Import,
                    options: IMPORT_OPTIONS,
                    children: &[
                        Node {
                            name: "files",
                            summary: "Импорт выбранных файлов конфигурации из XML",
                            kind: NodeKind::ImportFiles,
                            options: IMPORT_FILES_OPTIONS,
                            children: &[],
                        },
                        unsupported(
                            "all-extensions",
                            "Импорт всех расширений конфигурации из XML",
                        ),
                    ],
                },
                Node {
                    name: "support",
                    summary: "Настройка поддержки конфигурации",
                    kind: NodeKind::Unsupported,
                    options: &[],
                    children: &[unsupported("disable", "Снять конфигурацию с поддержки")],
                },
                Node {
                    name: "data-separation",
                    summary: "Получение информации о разделителях информационной базы",
                    kind: NodeKind::Unsupported,
                    options: &[],
                    children: &[unsupported(
                        "list",
                        "Вывод списка разделителей информационной базы",
                    )],
                },
                Node {
                    name: "extension",
                    summary: "Управление расширениями конфигурации",
                    kind: NodeKind::Unsupported,
                    options: &[],
                    children: &[
                        unsupported("create", "Создание расширения"),
                        unsupported("info", "Получить информацию о расширении"),
                        unsupported("list", "Получить список расширений"),
                        unsupported("update", "Установить указанные значения свойств расширения"),
                        unsupported("delete", "Удаление расширения"),
                    ],
                },
                unsupported(
                    "generation-id",
                    "Получить идентификатор поколения конфигурации",
                ),
                unsupported("sign", "Цифровая подпись конфигурации/расширения"),
            ],
        },
    ],
};

/// The platform ibcmd's other modes (`ibcmd help`), none served yet.
pub const OTHER_MODES: &[(&str, &str)] = &[
    ("server", "Режим настройки автономного сервера"),
    ("eventlog", "Режим работы с журналом регистрации"),
    ("config", "Режим работы с конфигурациями и расширениями"),
    ("extension", "Режим работы с расширениями"),
    ("mobile-app", "Режим работы с мобильным приложением"),
    ("mobile-client", "Режим работы с мобильным клиентом"),
    (
        "session",
        "Режим администрирования сеансов информационных баз",
    ),
    ("lock", "Режим администрирования блокировок"),
    (
        "binary-data-storage",
        "Режим администрирования хранилищ двоичных данных",
    ),
];

/// A command line refused before anything runs; `super::refusal_exit_code`
/// gives its exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// An unknown option, a value where none is taken, an extra argument.
    Parse(String),
    /// An option without its value, a command without its path.
    MissingValue(String),
    /// A command word that needs one of its subcommands.
    Incomplete {
        path: Vec<&'static str>,
    },
    /// A native command ibcmd-rs does not implement yet.
    UnsupportedCommand(String),
    /// A native option of a served command that ibcmd-rs does not implement.
    UnsupportedOption {
        option: String,
        command: String,
    },
    /// `--pid`/`--remote`: a running standalone server.
    UnsupportedServer(String),
    /// A DBMS the platform knows and ibcmd-rs does not serve.
    UnsupportedDbms(String),
    /// A DBMS the platform does not know either.
    UnknownDbms(String),
    /// `--db-path` with a command ibcmd-rs does not run on a file infobase
    /// yet (only `config export` and `config import` do).
    FileInfobase,
    /// An import from a `.zip`.
    ImportArchive(PathBuf),
    /// A value of the wrong shape.
    InvalidValue {
        option: String,
        value: String,
    },
    /// An option of a fixed set of words given another one (or none), as the
    /// platform words it: `Некорректное значение параметра: dynamic`.
    BadValue(String),
    /// Two options that exclude each other.
    Conflict {
        first: String,
        second: String,
    },
    /// A line ibcmd-rs will not run, with the reason in its own words.
    Unsupported(String),
    NativeMalformed(String),
}

/// The connection and the other options every served command shares.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Common {
    /// `--config`/`-c`: the platform ibcmd's configuration file.
    pub native_config: Option<PathBuf>,
    /// `--db-path`/`--database-path`: a file infobase, in place of a DBMS.
    pub db_path: Option<PathBuf>,
    pub dbms: Option<String>,
    pub db_server: Option<String>,
    pub db_name: Option<String>,
    pub db_user: Option<String>,
    pub db_pwd: Option<String>,
    /// `-W`: read the database password from STDIN.
    pub request_db_pwd: bool,
    pub data: Option<PathBuf>,
    pub temp: Option<PathBuf>,
    pub user: Option<String>,
    pub password: Option<String>,
    pub report: Option<PathBuf>,
    pub sqlcmd: Option<PathBuf>,
    pub settings: Option<PathBuf>,
    /// `--platform` (ibcmd-rs's own): the platform whose XML format is read
    /// or written, a release (`8.3.27`) or an exact build of the registry.
    pub platform: Option<PlatformSpec>,
    pub source_version: Option<InfobaseConfigSourceVersion>,
    pub db_pwd_env: Option<String>,
    pub extension: Option<String>,
    pub rac: Option<PathBuf>,
    pub ras_endpoint: Option<String>,
    pub cluster_id: Option<uuid::Uuid>,
    pub infobase_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRequest {
    pub chart_series_names: crate::mssql_dump::MoxelChartSeriesAutomaticName,
    pub common: Common,
    pub threads: Option<usize>,
    /// The extension to export (`--extension`, `-e`); the configuration
    /// itself without it.
    pub extension: Option<String>,
    /// `--base`, `-b`: the ConfigDumpInfo.xml of an earlier export; only
    /// what changed since is written.
    pub base: Option<PathBuf>,
    /// `--sync`: the directory is brought in line with the configuration.
    pub sync: bool,
    /// The directory as given.
    pub path: OsString,
}

/// Native export queries share connection resolution and the export pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportQueryRequest {
    pub export: ExportRequest,
    pub kind: ExportQueryKind,
    pub out: Option<PathBuf>,
    pub short: bool,
    pub recursive: bool,
    pub objects: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportQueryKind {
    Info,
    Status,
    Objects,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRequest {
    pub common: Common,
    pub base_free: bool,
    /// `--verify`: check the staged state against the tree (the default).
    pub verify: bool,
    /// `--no-verify`: check nothing.
    pub no_verify: bool,
    /// The directory as given.
    pub path: OsString,
}

/// `--dynamic`: whether the platform may update the running infobase
/// dynamically. The default is `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DynamicMode {
    Auto,
    Disable,
    Prompt,
    Force,
}

impl DynamicMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Disable => "disable",
            Self::Prompt => "prompt",
            Self::Force => "force",
        }
    }
}

/// `--session-terminate`: what to do with the sessions that keep the
/// exclusive lock from being taken. The default is `disable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionTerminate {
    Disable,
    Prompt,
    Force,
}

impl SessionTerminate {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Prompt => "prompt",
            Self::Force => "force",
        }
    }
}

/// `--exclusivity` (ibcmd-rs's own): how the apply learns that nobody else
/// works with the infobase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusivityMode {
    /// Other sessions on the database as SQL Server sees them (the default).
    Sql,
    /// The caller guarantees it; nothing is looked at.
    Assumed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyRequest {
    pub common: Common,
    /// `--force`/`-F`: confirm the warnings.
    pub force: bool,
    pub dynamic: DynamicMode,
    pub session_terminate: SessionTerminate,
    /// `--session-terminate-message`: the text a terminated session shows.
    pub session_terminate_message: Option<String>,
    pub exclusivity: ExclusivityMode,
    /// `--recovery-backup <file>` / `--i-have-a-backup` (ibcmd-rs's own): what
    /// the operator says about a way back before a restructuring. The
    /// platform has no such thing; it matters only for a stage the own apply
    /// restructures.
    pub backup: BackupPolicy,
}

/// `infobase config import files --base-dir=<dir> [--partial] [--no-check]
/// <files>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFilesRequest {
    pub common: Common,
    /// `--base-dir`: the directory the files are relative to.
    pub base_dir: PathBuf,
    /// The files as given.
    pub files: Vec<String>,
    /// `--partial`: the directory holds part of the configuration's files.
    pub partial: bool,
    /// `--verify` (ibcmd-rs's own): check the staged files (the default).
    pub verify: bool,
    /// `--no-check` (or ibcmd-rs's `--no-verify`): check nothing before
    /// the write.
    pub no_check: bool,
}

/// `infobase config save [--db] <file>`.
/// `infobase config load <file.cf>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadRequest {
    pub common: Common,
    /// The `.cf` as given.
    pub path: OsString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveRequest {
    pub common: Common,
    /// `--db`: the database configuration (Config) instead of the main one.
    pub database_configuration: bool,
    /// The file as given.
    pub path: OsString,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invocation {
    Help,
    Version,
    Export(ExportRequest),
    ExportQuery(ExportQueryRequest),
    Import(ImportRequest),
    ImportFiles(ImportFilesRequest),
    Apply(ApplyRequest),
    Save(SaveRequest),
    Load(LoadRequest),
}

/// The tokens of one command line, resolved against the tree.
struct Scan {
    /// The command path from the mode down (`["config", "export"]`).
    path: Vec<&'static Node>,
    /// Options in order: which one, how it was spelled, its value.
    options: Vec<(Opt, String, Option<String>)>,
    /// Arguments of the last command.
    arguments: Vec<OsString>,
    /// The first unknown option or option without its value.
    error: Option<Refusal>,
}

impl Scan {
    fn node(&self) -> &'static Node {
        self.path.last().copied().unwrap_or(&INFOBASE)
    }

    fn command(&self) -> String {
        std::iter::once("infobase")
            .chain(self.path.iter().map(|node| node.name))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn has(&self, opt: Opt) -> bool {
        self.options.iter().any(|(found, _, _)| *found == opt)
    }

    /// The last value given for an option.
    fn value(&self, opt: Opt) -> Option<&str> {
        self.options
            .iter()
            .rev()
            .find(|(found, _, _)| *found == opt)
            .and_then(|(_, _, value)| value.as_deref())
    }

    fn spelled(&self, opt: Opt) -> Option<&str> {
        self.options
            .iter()
            .find(|(found, _, _)| *found == opt)
            .map(|(_, spelled, _)| spelled.as_str())
    }
}

/// The options in force at a command: its own, its ancestors', the mode's
/// and the global ones, innermost first.
fn lookup<'a>(
    path: &[&'static Node],
    matches: impl Fn(&OptSpec) -> bool + 'a,
) -> Option<&'static OptSpec> {
    path.iter()
        .rev()
        .flat_map(|node| node.options.iter())
        .chain(INFOBASE.options.iter())
        .chain(GLOBAL_OPTIONS.iter())
        .find(|spec| matches(spec))
}

fn scan(args: &[OsString]) -> Scan {
    let mut scan = Scan {
        path: Vec::new(),
        options: Vec::new(),
        arguments: Vec::new(),
        error: None,
    };
    let mut index = 0;
    while index < args.len() {
        let raw = &args[index];
        index += 1;
        let text = raw.to_string_lossy();
        if text.len() > 1 && text.starts_with('-') {
            let (spec, spelled, attached) = if let Some(long) = text.strip_prefix("--") {
                let (name, attached) = match long.split_once('=') {
                    Some((name, value)) => (name, Some(value.to_string())),
                    None => (long, None),
                };
                let spec = (!name.is_empty())
                    .then(|| lookup(&scan.path, |spec| spec.long.contains(&name)))
                    .flatten();
                (spec, format!("--{name}"), attached)
            } else {
                let mut chars = text.chars().skip(1);
                let short = chars.next();
                let spec = match (short, chars.next()) {
                    (Some(short), None) => lookup(&scan.path, |spec| spec.short == Some(short)),
                    _ => None,
                };
                (spec, text.to_string(), None)
            };
            let Some(spec) = spec else {
                scan.error.get_or_insert(Refusal::Parse(text.to_string()));
                continue;
            };
            let value = if spec.value {
                match attached {
                    Some(value) => Some(value),
                    None if index < args.len() => {
                        index += 1;
                        Some(args[index - 1].to_string_lossy().into_owned())
                    }
                    None => {
                        let name = spec
                            .long
                            .first()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| spelled.clone());
                        scan.error.get_or_insert(Refusal::MissingValue(name));
                        continue;
                    }
                }
            } else {
                if attached.is_some() {
                    // measured on 8.3.27.2214 (`export --force=yes`, `apply --force=yes`):
                    // the option's name without the dashes
                    let name = spec
                        .long
                        .first()
                        .map_or_else(|| text.to_string(), |name| name.to_string());
                    scan.error.get_or_insert(Refusal::Parse(name));
                    continue;
                }
                None
            };
            scan.options.push((spec.opt, spelled, value));
            continue;
        }
        // A command word descends while no argument has been given at this
        // level; anything else is an argument of the command reached.
        let node = scan.node();
        if scan.arguments.is_empty()
            && let Some(child) = node
                .children
                .iter()
                .find(|child| OsStr::new(child.name) == raw)
        {
            scan.path.push(child);
            continue;
        }
        scan.arguments.push(raw.clone());
    }
    scan
}

fn parse_export_query(scan: &Scan) -> Result<Invocation, Refusal> {
    if let Some(error) = scan.error.clone() {
        return Err(error);
    }
    for opt in [Opt::Pid, Opt::Remote] {
        if let Some(spelled) = scan.spelled(opt) {
            return Err(Refusal::UnsupportedServer(spelled.to_owned()));
        }
    }
    for opt in [
        Opt::File,
        Opt::Archive,
        Opt::Sync,
        Opt::Force,
        Opt::IgnoreUnresolvedRefs,
        Opt::Extension,
        Opt::ChartSeriesNames,
    ] {
        if let Some(spelled) = scan.spelled(opt) {
            return Err(Refusal::UnsupportedOption {
                option: spelled.into(),
                command: scan.command(),
            });
        }
    }
    let kind = match scan.node().kind {
        NodeKind::ExportInfo => ExportQueryKind::Info,
        NodeKind::ExportStatus => ExportQueryKind::Status,
        _ => ExportQueryKind::Objects,
    };
    let out = match scan.value(Opt::Out) {
        Some("") => return Err(Refusal::MissingValue("out".into())),
        value => value.map(PathBuf::from),
    };
    if kind == ExportQueryKind::Status && !scan.has(Opt::Base) {
        return Err(Refusal::NativeMalformed(
            "Не указан путь к файлу ConfigDumpInfo.xml".into(),
        ));
    }
    if kind == ExportQueryKind::Objects && out.is_none() {
        return Err(Refusal::MissingValue("out".into()));
    }
    if kind == ExportQueryKind::Objects && scan.arguments.is_empty() {
        return Err(Refusal::NativeMalformed(
            "Не указано ни одного объекта метаданных.".into(),
        ));
    }
    if kind != ExportQueryKind::Objects && !scan.arguments.is_empty() {
        return Err(Refusal::Parse(
            scan.arguments[0].to_string_lossy().into_owned(),
        ));
    }
    if kind != ExportQueryKind::Status && scan.has(Opt::Base) {
        return Err(Refusal::UnsupportedOption {
            option: "--base".into(),
            command: scan.command(),
        });
    }
    let threads = scan
        .value(Opt::Threads)
        .map(|v| {
            v.parse::<usize>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or_else(|| Refusal::InvalidValue {
                    option: "--threads".into(),
                    value: v.into(),
                })
        })
        .transpose()?;
    Ok(Invocation::ExportQuery(ExportQueryRequest {
        export: ExportRequest {
            chart_series_names: crate::mssql_dump::MoxelChartSeriesAutomaticName::Stored,
            common: common(scan)?,
            threads,
            extension: scan.value(Opt::Extension).map(str::to_owned),
            base: scan.value(Opt::Base).map(PathBuf::from),
            sync: false,
            path: out.clone().unwrap_or_default().into_os_string(),
        },
        kind,
        out,
        short: scan.has(Opt::Short),
        recursive: scan.has(Opt::Recursive),
        objects: scan
            .arguments
            .iter()
            .map(|v| v.to_string_lossy().into_owned())
            .collect(),
    }))
}

/// Parses the arguments that follow `infobase`.
pub fn parse_infobase(args: &[OsString]) -> Result<Invocation, Refusal> {
    let scan = scan(args);
    if scan.has(Opt::Help) {
        return Ok(Invocation::Help);
    }
    if scan.has(Opt::Version) {
        return Ok(Invocation::Version);
    }
    let node = scan.node();
    match node.kind {
        NodeKind::Unsupported => return Err(Refusal::UnsupportedCommand(scan.command())),
        NodeKind::Group => {
            if let Some(error) = scan.error {
                return Err(error);
            }
            return Err(Refusal::Incomplete {
                path: scan.path.iter().map(|node| node.name).collect(),
            });
        }
        NodeKind::Export
        | NodeKind::ExportInfo
        | NodeKind::ExportStatus
        | NodeKind::ExportObjects
        | NodeKind::Import
        | NodeKind::ImportFiles
        | NodeKind::Apply
        | NodeKind::Save
        | NodeKind::Load => {}
    }
    if matches!(
        node.kind,
        NodeKind::ExportInfo | NodeKind::ExportStatus | NodeKind::ExportObjects
    ) {
        return parse_export_query(&scan);
    }
    if node.kind == NodeKind::Apply {
        return parse_apply(&scan);
    }
    if node.kind == NodeKind::ImportFiles {
        return parse_import_files(&scan);
    }
    if node.kind == NodeKind::Save {
        return parse_save(&scan);
    }
    if node.kind == NodeKind::Load {
        return parse_load(&scan);
    }
    if let Some(error) = scan.error.clone() {
        return Err(error);
    }
    for opt in [Opt::Pid, Opt::Remote] {
        if let Some(spelled) = scan.spelled(opt) {
            return Err(Refusal::UnsupportedServer(spelled.to_string()));
        }
    }
    let unsupported_options: &[Opt] = match node.kind {
        NodeKind::Export => &[Opt::File, Opt::Archive],
        _ => &[Opt::Out],
    };
    for opt in unsupported_options {
        if let Some(spelled) = scan.spelled(*opt) {
            return Err(Refusal::UnsupportedOption {
                option: spelled.to_string(),
                command: scan.command(),
            });
        }
    }
    if node.kind == NodeKind::Export && scan.has(Opt::Base) && scan.has(Opt::Sync) {
        return Err(Refusal::NativeMalformed(
            "Опции base и sync не могут быть использованы одновременно.".into(),
        ));
    }
    let chart_series_names = match scan.value(Opt::ChartSeriesNames) {
        None => crate::mssql_dump::MoxelChartSeriesAutomaticName::Stored,
        Some(value) => {
            <crate::mssql_dump::MoxelChartSeriesAutomaticName as clap::ValueEnum>::from_str(
                value, false,
            )
            .map_err(|_| Refusal::InvalidValue {
                option: "--chart-series-names".into(),
                value: value.into(),
            })?
        }
    };
    if chart_series_names != crate::mssql_dump::MoxelChartSeriesAutomaticName::Stored
        && (scan.has(Opt::Base) || scan.has(Opt::Sync) || scan.has(Opt::Extension))
    {
        return Err(Refusal::NativeMalformed(
            "--chart-series-names=russian|english requires a full configuration export without --base, --sync or --extension".into(),
        ));
    }
    let common = common(&scan)?;
    let path = match scan.arguments.as_slice() {
        [] => {
            return Err(Refusal::MissingValue(
                match node.kind {
                    NodeKind::Export => "путь к каталогу файлов конфигурации",
                    _ => "путь к каталогу или архиву с файлами конфигурации",
                }
                .to_string(),
            ));
        }
        [path] => path.clone(),
        [_, extra, ..] => return Err(Refusal::Parse(extra.to_string_lossy().into_owned())),
    };
    // `--base` and `--sync` update an export of the configuration; the
    // export of an extension is written in full only.
    if scan.has(Opt::Extension) {
        for opt in [Opt::Base, Opt::Sync] {
            if let Some(spelled) = scan.spelled(opt) {
                return Err(Refusal::UnsupportedOption {
                    option: spelled.to_string(),
                    command: format!("{} --extension", scan.command()),
                });
            }
        }
    }
    Ok(match node.kind {
        NodeKind::Export => Invocation::Export(ExportRequest {
            chart_series_names,
            common,
            extension: match scan.value(Opt::Extension) {
                None => None,
                Some(value) if value.trim().is_empty() => {
                    return Err(Refusal::InvalidValue {
                        option: "--extension".to_string(),
                        value: value.to_string(),
                    });
                }
                Some(value) => Some(value.to_string()),
            },
            base: match scan.value(Opt::Base) {
                None => None,
                Some(value) if value.trim().is_empty() => {
                    return Err(Refusal::InvalidValue {
                        option: "--base".to_string(),
                        value: value.to_string(),
                    });
                }
                Some(value) => Some(PathBuf::from(value)),
            },
            sync: scan.has(Opt::Sync),
            threads: match scan.value(Opt::Threads) {
                None => None,
                Some(value) => Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .filter(|threads| *threads > 0)
                        .ok_or_else(|| Refusal::InvalidValue {
                            option: "--threads".to_string(),
                            value: value.to_string(),
                        })?,
                ),
            },
            path,
        }),
        _ => {
            let as_path = PathBuf::from(&path);
            if as_path.is_file() {
                return Err(Refusal::ImportArchive(as_path));
            }
            if scan.has(Opt::Verify) && scan.has(Opt::NoVerify) {
                return Err(Refusal::Conflict {
                    first: "--verify".to_string(),
                    second: "--no-verify".to_string(),
                });
            }
            Invocation::Import(ImportRequest {
                common,
                base_free: scan.has(Opt::BaseFree),
                verify: scan.has(Opt::Verify),
                no_verify: scan.has(Opt::NoVerify),
                path,
            })
        }
    })
}

/// `infobase config apply`, parsed as the platform's (8.3.27.2214) is: a
/// stray argument is ignored, a value outside the option's words is
/// `Некорректное значение параметра: <name>` (exit 2) whether it is wrong or
/// missing, and the words are case-sensitive.
fn parse_apply(scan: &Scan) -> Result<Invocation, Refusal> {
    let command = scan.command();
    if let Some(error) = scan.error.clone() {
        return Err(match error {
            Refusal::MissingValue(name) if name == "dynamic" || name == "session-terminate" => {
                Refusal::BadValue(name)
            }
            other => other,
        });
    }
    let dynamic = match scan.value(Opt::Dynamic) {
        None => DynamicMode::Auto,
        Some("auto") => DynamicMode::Auto,
        Some("disable") => DynamicMode::Disable,
        Some("prompt") => DynamicMode::Prompt,
        Some("force") => DynamicMode::Force,
        Some(_) => return Err(Refusal::BadValue("dynamic".to_string())),
    };
    let session_terminate = match scan.value(Opt::SessionTerminate) {
        None => SessionTerminate::Disable,
        Some("disable") => SessionTerminate::Disable,
        Some("prompt") => SessionTerminate::Prompt,
        Some("force") => SessionTerminate::Force,
        Some(_) => return Err(Refusal::BadValue("session-terminate".to_string())),
    };
    let exclusivity = match scan.value(Opt::Exclusivity) {
        None | Some("sql") => ExclusivityMode::Sql,
        Some("assumed") => ExclusivityMode::Assumed,
        Some(other) => {
            return Err(Refusal::InvalidValue {
                option: "--exclusivity".to_string(),
                value: other.to_string(),
            });
        }
    };
    // A file names the way back; the word alone only says there is one.
    let backup = match (scan.value(Opt::RecoveryBackup), scan.has(Opt::IHaveABackup)) {
        (Some(""), _) => {
            return Err(Refusal::InvalidValue {
                option: "--recovery-backup".to_string(),
                value: String::new(),
            });
        }
        (Some(file), _) => BackupPolicy::File(PathBuf::from(file)),
        (None, true) => BackupPolicy::Acknowledged,
        (None, false) => BackupPolicy::None,
    };
    for opt in [Opt::Pid, Opt::Remote] {
        if let Some(spelled) = scan.spelled(opt) {
            return Err(Refusal::UnsupportedServer(spelled.to_string()));
        }
    }
    if scan.has(Opt::Sqlcmd) && !scan.has(Opt::Extension) {
        return Err(Refusal::Unsupported(format!(
            "Параметр `--sqlcmd` команды `{command}` не поддерживается: применение основной конфигурации работает только через встроенный клиент SQL Server"
        )));
    }
    let common = common(scan)?;
    // `--dynamic=force` is served (docs/apply/dropin-dynamic.md): the request carries it and the
    // apply decides, from the platform and the stage, whether it can be carried out.
    Ok(Invocation::Apply(ApplyRequest {
        common,
        force: scan.has(Opt::Force),
        dynamic,
        session_terminate,
        session_terminate_message: scan.value(Opt::SessionTerminateMessage).map(str::to_string),
        exclusivity,
        backup,
    }))
}

/// `infobase config import files`: `--base-dir`, the files, `--partial`,
/// `--no-check`, and the connection. The words of a missing `--base-dir` or
/// file list are this program's (the platform's are not measured).
fn parse_import_files(scan: &Scan) -> Result<Invocation, Refusal> {
    if let Some(error) = scan.error.clone() {
        return Err(error);
    }
    for opt in [Opt::Pid, Opt::Remote] {
        if let Some(spelled) = scan.spelled(opt) {
            return Err(Refusal::UnsupportedServer(spelled.to_string()));
        }
    }
    let command = scan.command();
    for opt in [Opt::Out, Opt::Extension] {
        if let Some(spelled) = scan.spelled(opt) {
            return Err(Refusal::UnsupportedOption {
                option: spelled.to_string(),
                command,
            });
        }
    }
    if scan.has(Opt::BaseFree) {
        return Err(Refusal::Unsupported(format!(
            "Параметр `--base-free` не применяется к команде `{command}`: частичная загрузка \
             изменяет строки конфигурации базы, а не собирает ее с нуля"
        )));
    }
    if scan.has(Opt::Verify) {
        for opt in [Opt::NoCheck, Opt::NoVerify] {
            if let Some(spelled) = scan.spelled(opt) {
                return Err(Refusal::Conflict {
                    first: "--verify".to_string(),
                    second: spelled.to_string(),
                });
            }
        }
    }
    let common = common(scan)?;
    let base_dir = match scan.value(Opt::BaseDir) {
        None => return Err(Refusal::MissingValue("base-dir".to_string())),
        Some(value) if value.trim().is_empty() => {
            return Err(Refusal::InvalidValue {
                option: "--base-dir".to_string(),
                value: value.to_string(),
            });
        }
        Some(value) => PathBuf::from(value),
    };
    if scan.arguments.is_empty() {
        return Err(Refusal::MissingValue(
            "файлы конфигурации для загрузки".to_string(),
        ));
    }
    Ok(Invocation::ImportFiles(ImportFilesRequest {
        common,
        base_dir,
        files: scan
            .arguments
            .iter()
            .map(|file| file.to_string_lossy().into_owned())
            .collect(),
        partial: scan.has(Opt::Partial),
        verify: scan.has(Opt::Verify),
        no_check: scan.has(Opt::NoCheck) || scan.has(Opt::NoVerify),
    }))
}

/// `infobase config save`: one file argument, `--db`, and the connection.
/// The words of a missing path are this program's (the platform's own for
/// `config save` are not measured).
fn parse_save(scan: &Scan) -> Result<Invocation, Refusal> {
    if let Some(error) = scan.error.clone() {
        return Err(error);
    }
    for opt in [Opt::Pid, Opt::Remote] {
        if let Some(spelled) = scan.spelled(opt) {
            return Err(Refusal::UnsupportedServer(spelled.to_string()));
        }
    }
    if let Some(spelled) = scan.spelled(Opt::Extension) {
        return Err(Refusal::UnsupportedOption {
            option: spelled.to_string(),
            command: scan.command(),
        });
    }
    let common = common(scan)?;
    let path = match scan.arguments.as_slice() {
        [] => {
            return Err(Refusal::MissingValue(
                "путь к файлу конфигурации".to_string(),
            ));
        }
        [path] => path.clone(),
        [_, extra, ..] => return Err(Refusal::Parse(extra.to_string_lossy().into_owned())),
    };
    Ok(Invocation::Save(SaveRequest {
        common,
        database_configuration: scan.has(Opt::Db),
        path,
    }))
}

/// `infobase config load <file>`: as `config save`, without `--db`.
fn parse_load(scan: &Scan) -> Result<Invocation, Refusal> {
    if let Some(error) = scan.error.clone() {
        return Err(error);
    }
    for opt in [Opt::Pid, Opt::Remote] {
        if let Some(spelled) = scan.spelled(opt) {
            return Err(Refusal::UnsupportedServer(spelled.to_string()));
        }
    }
    if let Some(spelled) = scan.spelled(Opt::Extension) {
        return Err(Refusal::UnsupportedOption {
            option: spelled.to_string(),
            command: scan.command(),
        });
    }
    let common = common(scan)?;
    let path = match scan.arguments.as_slice() {
        [] => {
            return Err(Refusal::MissingValue(
                "путь к файлу конфигурации".to_string(),
            ));
        }
        [path] => path.clone(),
        [_, extra, ..] => return Err(Refusal::Parse(extra.to_string_lossy().into_owned())),
    };
    Ok(Invocation::Load(LoadRequest { common, path }))
}

/// The DBMS the platform knows: MSSQLServer is served, the rest refused.
const KNOWN_DBMS: &[&str] = &["MSSQLServer", "PostgreSQL", "IBMDB2", "OracleDatabase"];

fn common(scan: &Scan) -> Result<Common, Refusal> {
    let text = |opt| scan.value(opt).map(str::to_string);
    let path = |opt| scan.value(opt).map(PathBuf::from);
    // `--db-path` names a file infobase; a DBMS value beside it contradicts it.
    if scan.has(Opt::DbPath) {
        for (opt, name) in [
            (Opt::Dbms, "--dbms"),
            (Opt::DbServer, "--db-server"),
            (Opt::DbName, "--db-name"),
        ] {
            if scan.has(opt) {
                return Err(Refusal::Conflict {
                    first: "--db-path".to_string(),
                    second: name.to_string(),
                });
            }
        }
    }
    let dbms = match scan.value(Opt::Dbms) {
        Some(value) => {
            let known = KNOWN_DBMS
                .iter()
                .find(|known| known.eq_ignore_ascii_case(value.trim()));
            match known {
                Some(&"MSSQLServer") => Some("MSSQLServer".to_string()),
                // Exported directly (#369); the other commands refuse it
                // (`dropin::run_infobase`).
                Some(&"PostgreSQL") => Some("PostgreSQL".to_string()),
                Some(other) => return Err(Refusal::UnsupportedDbms(other.to_string())),
                None => return Err(Refusal::UnknownDbms(value.to_string())),
            }
        }
        None => None,
    };
    // Without --dbms the platform opens a file infobase. Here a server
    // database may still come from the settings (a settings file, `--config`,
    // the environment), so a missing one is the connection's to report
    // (`infobase::resolve_connection`), not the parser's.
    let source_version = match scan.value(Opt::SourceVersion) {
        None => None,
        Some(value) => Some(
            <InfobaseConfigSourceVersion as clap::ValueEnum>::from_str(value.trim(), true)
                .map_err(|_| Refusal::InvalidValue {
                    option: "--source-version".to_string(),
                    value: value.to_string(),
                })?,
        ),
    };
    let platform = match scan.value(Opt::Platform) {
        None => None,
        Some(value) => {
            Some(
                crate::platform::parse(value.trim()).map_err(|_| Refusal::InvalidValue {
                    option: "--platform".to_string(),
                    value: format!("{value} (известные версии: {})", known_platforms()),
                })?,
            )
        }
    };
    if platform.is_some() && source_version.is_some() {
        return Err(Refusal::Conflict {
            first: "--platform".to_string(),
            second: "--source-version".to_string(),
        });
    }
    Ok(Common {
        native_config: path(Opt::Config),
        db_path: path(Opt::DbPath),
        dbms,
        db_server: text(Opt::DbServer),
        db_name: text(Opt::DbName),
        db_user: text(Opt::DbUser),
        db_pwd: text(Opt::DbPwd),
        request_db_pwd: scan.has(Opt::RequestDbPwd),
        data: path(Opt::Data),
        temp: path(Opt::Temp),
        user: text(Opt::User),
        password: text(Opt::Password),
        report: path(Opt::Report),
        sqlcmd: path(Opt::Sqlcmd),
        settings: path(Opt::Settings),
        platform,
        source_version,
        db_pwd_env: text(Opt::DbPwdEnv),
        extension: match scan.value(Opt::Extension) {
            Some(value) if value.trim().is_empty() => {
                return Err(Refusal::InvalidValue {
                    option: "--extension".into(),
                    value: value.into(),
                });
            }
            value => value.map(str::to_owned),
        },
        rac: path(Opt::Rac),
        ras_endpoint: text(Opt::RasEndpoint),
        cluster_id: scan
            .value(Opt::ClusterId)
            .map(|s| {
                uuid::Uuid::parse_str(s).map_err(|_| Refusal::InvalidValue {
                    option: "--cluster-id".into(),
                    value: s.into(),
                })
            })
            .transpose()?,
        infobase_id: scan
            .value(Opt::InfobaseId)
            .map(|s| {
                uuid::Uuid::parse_str(s).map_err(|_| Refusal::InvalidValue {
                    option: "--infobase-id".into(),
                    value: s.into(),
                })
            })
            .transpose()?,
    })
}

/// The releases and builds of the platform registry, for a refused
/// `--platform`.
fn known_platforms() -> String {
    crate::platform::known()
        .map(|known| {
            known
                .iter()
                .map(|spec| spec.display())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

/// Every command path of the tree, with its kind (for the tests and the
/// help): `["config", "export", "info"]` and so on.
pub fn command_paths() -> Vec<(Vec<&'static str>, NodeKind)> {
    fn walk(
        node: &'static Node,
        prefix: &mut Vec<&'static str>,
        out: &mut Vec<(Vec<&'static str>, NodeKind)>,
    ) {
        for child in node.children {
            prefix.push(child.name);
            out.push((prefix.clone(), child.kind));
            walk(child, prefix, out);
            prefix.pop();
        }
    }
    let mut out = Vec::new();
    walk(&INFOBASE, &mut Vec::new(), &mut out);
    out
}

/// The node a command path leads to.
pub fn node_at(path: &[&str]) -> Option<&'static Node> {
    let mut node = &INFOBASE;
    for name in path {
        node = node.children.iter().find(|child| child.name == *name)?;
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_series_names_default_values_forward_and_refuse_unknown() {
        use crate::mssql_dump::MoxelChartSeriesAutomaticName as Policy;
        assert_eq!(
            export(&["config", "export", "--db-name=lab", "out"]).chart_series_names,
            Policy::Stored
        );
        for (value, expected) in [
            ("stored", Policy::Stored),
            ("russian", Policy::Russian),
            ("english", Policy::RepublishedInEnglish),
        ] {
            let option = format!("--chart-series-names={value}");
            let request = export(&["config", "export", "--db-name=lab", &option, "out"]);
            assert_eq!(request.chart_series_names, expected);
            assert_eq!(
                crate::dropin::export_args(&request).chart_series_names,
                expected
            );
        }
        assert!(
            matches!(parse(&["config", "export", "--db-name=lab", "--chart-series-names=automatic", "out"]),
            Err(Refusal::InvalidValue { option, .. }) if option == "--chart-series-names")
        );
    }

    #[test]
    fn changed_chart_names_refuse_incremental_extension_and_query_commands() {
        for option in ["--base=previous.xml", "--sync", "--extension=Example"] {
            assert!(
                parse(&[
                    "config",
                    "export",
                    "--db-name=lab",
                    "--chart-series-names=russian",
                    option,
                    "out"
                ])
                .is_err()
            );
        }
        assert!(
            parse(&[
                "config",
                "export",
                "info",
                "--db-name=lab",
                "--chart-series-names=english"
            ])
            .is_err()
        );
        let stored = export(&[
            "config",
            "export",
            "--db-name=lab",
            "--chart-series-names=stored",
            "--base=previous.xml",
            "out",
        ]);
        assert_eq!(
            stored.chart_series_names,
            crate::mssql_dump::MoxelChartSeriesAutomaticName::Stored
        );
    }

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    fn parse(list: &[&str]) -> Result<Invocation, Refusal> {
        parse_infobase(&args(list))
    }

    fn export(list: &[&str]) -> ExportRequest {
        match parse(list) {
            Ok(Invocation::Export(request)) => request,
            other => panic!("{list:?}: {other:?}"),
        }
    }

    fn import(list: &[&str]) -> ImportRequest {
        match parse(list) {
            Ok(Invocation::Import(request)) => request,
            other => panic!("{list:?}: {other:?}"),
        }
    }

    #[test]
    fn the_native_export_line_of_the_lab_scripts_parses() {
        // scripts/empty-load/run_empty.ps1 and scripts/timing/run_timing.ps1
        let request = export(&[
            "config",
            "export",
            "--dbms=MSSQLServer",
            "--db-server=localhost",
            "--db-name=ibcmd_rs_empty_bsp8327_ours_20260925_t1",
            r"--data=F:\ibcmd\lab\model\empty\run\ibdata",
            "--user=Администратор",
            "--force",
            r"F:\ibcmd\lab\model\empty\run\export",
        ]);
        assert_eq!(request.common.dbms.as_deref(), Some("MSSQLServer"));
        assert_eq!(request.common.db_server.as_deref(), Some("localhost"));
        assert_eq!(
            request.common.db_name.as_deref(),
            Some("ibcmd_rs_empty_bsp8327_ours_20260925_t1")
        );
        assert_eq!(
            request.common.data,
            Some(PathBuf::from(r"F:\ibcmd\lab\model\empty\run\ibdata"))
        );
        assert_eq!(request.common.user.as_deref(), Some("Администратор"));
        assert_eq!(
            request.path,
            OsString::from(r"F:\ibcmd\lab\model\empty\run\export")
        );
        assert_eq!(request.threads, None);
    }

    #[test]
    fn the_native_import_line_of_the_lab_scripts_parses() {
        // scripts/timing/run_timing.ps1: `infobase config import @na <tree>`
        let request = import(&[
            "config",
            "import",
            "--dbms=MSSQLServer",
            "--db-server=localhost",
            "--db-name=ibcmd_rs_tm_bsp_20260924",
            r"--data=F:\ibcmd\lab\timing\out\native_data_load_bsp8327_t1",
            "--user=Администратор",
            r"F:\ibcmd\lab\parity\bsp\native",
        ]);
        assert_eq!(
            request.common.db_name.as_deref(),
            Some("ibcmd_rs_tm_bsp_20260924")
        );
        assert_eq!(
            request.path,
            OsString::from(r"F:\ibcmd\lab\parity\bsp\native")
        );
        assert!(!request.base_free);
    }

    #[test]
    fn the_native_import_files_line_of_the_lab_scripts_parses() {
        // scripts/restructure-lab/import_files.ps1: `infobase config import
        // files <connection> --base-dir=<dir> --partial <files>`
        let request = match parse(&[
            "config",
            "import",
            "files",
            "--dbms=MSSQLServer",
            "--db-server=localhost",
            "--db-name=base",
            r"--data=C:\ibdata\base",
            "--user=Администратор",
            r"--base-dir=C:\trees\stage",
            "--partial",
            "Catalogs/X.xml",
            "Catalogs/X/Ext/ObjectModule.bsl",
        ]) {
            Ok(Invocation::ImportFiles(request)) => request,
            other => panic!("{other:?}"),
        };
        assert_eq!(request.common.db_name.as_deref(), Some("base"));
        assert_eq!(request.base_dir, PathBuf::from(r"C:\trees\stage"));
        assert_eq!(
            request.files,
            ["Catalogs/X.xml", "Catalogs/X/Ext/ObjectModule.bsl"]
        );
        assert!(request.partial && !request.verify && !request.no_check);
        // `--no-check`, and this program's `--no-verify`, skip the check
        for skip in ["--no-check", "--no-verify"] {
            match parse(&["config", "import", "files", "--base-dir", "d", skip, "f"]) {
                Ok(Invocation::ImportFiles(request)) => {
                    assert!(request.no_check && !request.partial, "{skip}");
                }
                other => panic!("{skip}: {other:?}"),
            }
        }
        // the options of `files` are its own
        assert_eq!(
            parse(&["config", "import", "--partial", "d"]),
            Err(Refusal::Parse("--partial".to_string()))
        );
        assert_eq!(
            parse(&["config", "import", "files", "f"]),
            Err(Refusal::MissingValue("base-dir".to_string()))
        );
    }

    #[test]
    fn the_platform_is_one_the_registry_knows() {
        let request = export(&[
            "config",
            "export",
            "--dbms=MSSQLServer",
            "--db-name=b",
            "--platform=8.5.1",
            "out",
        ]);
        assert_eq!(
            request.common.platform.map(|platform| platform.display()),
            Some("8.5.1")
        );
        let request = import(&["config", "import", "--platform", "8.3.27.2214", "tree"]);
        assert_eq!(
            request.common.platform.map(|platform| platform.display()),
            Some("8.3.27.2214")
        );
        match parse(&["config", "export", "--platform=8.4", "out"]) {
            Err(Refusal::InvalidValue { option, value }) => {
                assert_eq!(option, "--platform");
                assert!(value.starts_with("8.4 (известные версии: "), "{value}");
                assert!(value.contains("8.5.1"), "{value}");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            parse(&[
                "config",
                "export",
                "--platform=8.3.27",
                "--source-version=2.20",
                "out",
            ]),
            Err(Refusal::Conflict {
                first: "--platform".to_string(),
                second: "--source-version".to_string(),
            })
        );
    }

    #[test]
    fn both_spellings_and_both_value_forms_are_accepted() {
        let long = export(&[
            "config",
            "export",
            "--dbms",
            "MSSQLServer",
            "--database-server",
            "sql01",
            "--database-name",
            "base",
            "--database-user",
            "sa",
            "--database-password",
            "secret",
            "out",
        ]);
        let short = export(&[
            "config",
            "export",
            "--dbms=MSSQLServer",
            "--db-server=sql01",
            "--db-name=base",
            "--db-user=sa",
            "--db-pwd=secret",
            "out",
        ]);
        assert_eq!(long, short);
        assert_eq!(long.common.db_user.as_deref(), Some("sa"));
        assert_eq!(long.common.db_pwd.as_deref(), Some("secret"));

        let request = export(&[
            "config",
            "export",
            "-c",
            r"C:\ibcmd\ibcmd.yml",
            "-d",
            r"C:\data",
            "-t",
            r"C:\temp",
            "-W",
            "-T",
            "4",
            "--db-name",
            "base",
            "out",
        ]);
        assert_eq!(
            request.common.native_config,
            Some(PathBuf::from(r"C:\ibcmd\ibcmd.yml"))
        );
        assert_eq!(request.common.data, Some(PathBuf::from(r"C:\data")));
        assert_eq!(request.common.temp, Some(PathBuf::from(r"C:\temp")));
        assert!(request.common.request_db_pwd);
        assert_eq!(request.threads, Some(4));
        for spelling in ["--request-db-pwd", "--request-database-password"] {
            let request = export(&["config", "export", spelling, "--db-name=b", "out"]);
            assert!(request.common.request_db_pwd);
        }
        let request = export(&["config", "export", "--config=cfg.yml", "--db-name=b", "out"]);
        assert_eq!(request.common.native_config, Some(PathBuf::from("cfg.yml")));
        let request = export(&["config", "export", "--threads=8", "--db-name=b", "out"]);
        assert_eq!(request.threads, Some(8));
    }

    #[test]
    fn mode_options_go_anywhere_and_command_options_after_their_command() {
        let request = export(&[
            "--dbms=MSSQLServer",
            "config",
            "--db-name=base",
            "export",
            "out",
            "--db-server=sql01",
        ]);
        assert_eq!(request.common.db_name.as_deref(), Some("base"));
        assert_eq!(request.common.db_server.as_deref(), Some("sql01"));
        // `config`'s own options after `config`
        let request = export(&[
            "config",
            "-u",
            "Админ",
            "-P",
            "",
            "export",
            "--db-name=b",
            "o",
        ]);
        assert_eq!(request.common.user.as_deref(), Some("Админ"));
        assert_eq!(request.common.password.as_deref(), Some(""));
        // an `export` option before `export` is unknown there, as natively
        assert_eq!(
            parse(&["config", "--threads", "4", "export", "--db-name=b", "o"]),
            Err(Refusal::Parse("--threads".to_string()))
        );
        assert_eq!(
            parse(&["--force", "config", "export", "--db-name=b", "o"]),
            Err(Refusal::Parse("--force".to_string()))
        );
    }

    #[test]
    fn the_data_directory_options_are_accepted() {
        let mut list = vec!["config", "export", "--db-name=b"];
        let options = [
            "--system=s",
            "--lock=l",
            "--users-data=u",
            "--session-data=s",
            "--stt-data=s",
            "--log-data=l",
            "--ftext-data=f",
            "--ftext2-data=f",
            "--openid-data=o",
            "--bin-data-strg=b",
            "--clnt-notif-data=c",
            "--websocket-data=w",
            "--ignore-unresolved-refs",
            "--force",
        ];
        list.extend(options);
        list.push("out");
        let request = export(&list);
        assert_eq!(request.path, OsString::from("out"));
    }

    #[test]
    fn parse_errors_name_the_argument_as_natively() {
        assert_eq!(
            parse(&["config", "export", "--bogus", "--db-name=b", "o"]),
            Err(Refusal::Parse("--bogus".to_string()))
        );
        assert_eq!(
            parse(&["config", "export", "--DBMS=MSSQLServer", "--db-name=b", "o"]),
            Err(Refusal::Parse("--DBMS=MSSQLServer".to_string()))
        );
        assert_eq!(
            parse(&["config", "export", "-T4", "--db-name=b", "o"]),
            Err(Refusal::Parse("-T4".to_string()))
        );
        // a flag given a value: the platform names the option without dashes
        assert_eq!(
            parse(&["config", "export", "--force=yes", "--db-name=b", "o"]),
            Err(Refusal::Parse("force".to_string()))
        );
        assert_eq!(
            parse(&["config", "export", "--db-name=b", "o", "--db-server"]),
            Err(Refusal::MissingValue("database-server".to_string()))
        );
        assert_eq!(
            parse(&["config", "export", "--db-name=b", "a", "b"]),
            Err(Refusal::Parse("b".to_string()))
        );
        assert_eq!(
            parse(&["config", "export", "--db-name=b"]),
            Err(Refusal::MissingValue(
                "путь к каталогу файлов конфигурации".to_string()
            ))
        );
        assert_eq!(
            parse(&["config", "export", "--threads=0", "--db-name=b", "o"]),
            Err(Refusal::InvalidValue {
                option: "--threads".to_string(),
                value: "0".to_string()
            })
        );
        assert_eq!(
            parse(&[
                "config",
                "export",
                "--source-version=3.0",
                "--db-name=b",
                "o"
            ]),
            Err(Refusal::InvalidValue {
                option: "--source-version".to_string(),
                value: "3.0".to_string()
            })
        );
    }

    #[test]
    fn the_dbms_is_checked_as_natively() {
        let request = export(&["config", "export", "--dbms=mssqlserver", "--db-name=b", "o"]);
        assert_eq!(request.common.dbms.as_deref(), Some("MSSQLServer"));
        let request = export(&["config", "export", "--dbms=postgresql", "--db-name=b", "o"]);
        assert_eq!(request.common.dbms.as_deref(), Some("PostgreSQL"));
        for dbms in ["IBMDB2", "OracleDatabase"] {
            assert_eq!(
                parse(&["config", "export", &format!("--dbms={dbms}"), "o"]),
                Err(Refusal::UnsupportedDbms(dbms.to_string()))
            );
        }
        assert_eq!(
            parse(&["config", "export", "--dbms=Foo", "o"]),
            Err(Refusal::UnknownDbms("Foo".to_string()))
        );
        // no --dbms and no database: left to the connection, which may find
        // one in the settings (and says so when it does not)
        assert!(matches!(
            parse(&["config", "export", "o"]),
            Ok(Invocation::Export(_))
        ));
        // `--db-path` names a file infobase (#375); a DBMS value beside it
        // contradicts it
        let request = export(&["config", "export", "--db-path=C:\\ib", "o"]);
        assert_eq!(request.common.db_path, Some(PathBuf::from("C:\\ib")));
        let request = import(&["config", "import", "--database-path", "C:\\ib", "o"]);
        assert_eq!(request.common.db_path, Some(PathBuf::from("C:\\ib")));
        for (option, name) in [
            ("--dbms=MSSQLServer", "--dbms"),
            ("--db-server=s", "--db-server"),
            ("--db-name=b", "--db-name"),
        ] {
            assert_eq!(
                parse(&["config", "export", "--db-path=C:\\ib", option, "o"]),
                Err(Refusal::Conflict {
                    first: "--db-path".to_string(),
                    second: name.to_string()
                })
            );
        }
        // a database name alone means the server database
        let request = export(&["config", "export", "--db-name=b", "o"]);
        assert_eq!(request.common.dbms, None);
    }

    #[test]
    fn an_extension_export_names_the_extension() {
        for spelled in [
            vec!["--extension=Расширение"],
            vec!["--extension", "Расширение"],
            vec!["-e", "Расширение"],
        ] {
            let mut list = vec!["config", "export", "--db-name=b"];
            list.extend(spelled);
            list.push("out");
            assert_eq!(
                export(&list).extension.as_deref(),
                Some("Расширение"),
                "{list:?}"
            );
        }
        let request = export(&["config", "export", "--db-name=b", "out"]);
        assert_eq!(request.extension, None);
        assert_eq!(
            parse(&["config", "export", "--db-name=b", "--extension=", "out"]),
            Err(Refusal::InvalidValue {
                option: "--extension".to_string(),
                value: String::new(),
            })
        );
    }

    #[test]
    fn an_incremental_export_names_its_base_and_sync() {
        for spelled in [
            vec!["--base=dump\\ConfigDumpInfo.xml"],
            vec!["--base", "dump\\ConfigDumpInfo.xml"],
            vec!["-b", "dump\\ConfigDumpInfo.xml"],
        ] {
            let mut list = vec!["config", "export", "--db-name=b"];
            list.extend(spelled);
            list.push("out");
            let request = export(&list);
            assert_eq!(
                request.base,
                Some(PathBuf::from("dump\\ConfigDumpInfo.xml")),
                "{list:?}"
            );
            assert!(!request.sync);
        }
        let request = export(&["config", "export", "--db-name=b", "--sync", "out"]);
        assert!(request.sync && request.base.is_none());
        let request = export(&["config", "export", "--db-name=b", "out"]);
        assert!(!request.sync && request.base.is_none());
        assert_eq!(
            parse(&["config", "export", "--db-name=b", "--base=", "out"]),
            Err(Refusal::InvalidValue {
                option: "--base".to_string(),
                value: String::new(),
            })
        );
        // a flag given a value, as the platform words it
        assert_eq!(
            parse(&["config", "export", "--db-name=b", "--sync=1", "out"]),
            Err(Refusal::Parse("sync".to_string()))
        );
        // an extension is exported in full only
        for option in ["--base=i.xml", "--sync"] {
            match parse(&["config", "export", "--db-name=b", "-e", "E", option, "out"]) {
                Err(Refusal::UnsupportedOption { command, .. }) => {
                    assert_eq!(command, "infobase config export --extension")
                }
                other => panic!("{option}: {other:?}"),
            }
        }
    }

    #[test]
    fn unsupported_options_of_served_commands_are_named() {
        for option in ["--file=a.cf", "-f", "--archive", "-A"] {
            let mut list = vec!["config", "export", "--db-name=b"];
            list.push(option);
            if option == "-f" {
                list.push("value");
            }
            list.push("out");
            match parse(&list) {
                Err(Refusal::UnsupportedOption {
                    option: named,
                    command,
                }) => {
                    assert_eq!(command, "infobase config export");
                    assert!(option.starts_with(&named), "{option} named {named}");
                }
                other => panic!("{option}: {other:?}"),
            }
        }
        for option in ["--out=a.cf"] {
            match parse(&["config", "import", "--db-name=b", option, "tree"]) {
                Err(Refusal::UnsupportedOption { command, .. }) => {
                    assert_eq!(command, "infobase config import")
                }
                other => panic!("{option}: {other:?}"),
            }
        }
        for option in ["--pid=12", "--remote=http://host:1545", "-r", "-p"] {
            let mut list = vec!["config", "export", "--db-name=b", option];
            if option.len() == 2 {
                list.push("value");
            }
            list.push("out");
            assert!(
                matches!(parse(&list), Err(Refusal::UnsupportedServer(_))),
                "{option}"
            );
        }
    }

    #[test]
    fn every_native_command_is_recognized() {
        let paths = command_paths();
        for (path, kind) in &paths {
            let result = parse(&path.iter().copied().collect::<Vec<_>>());
            match kind {
                NodeKind::Unsupported => assert_eq!(
                    result,
                    Err(Refusal::UnsupportedCommand(format!(
                        "infobase {}",
                        path.join(" ")
                    ))),
                    "{path:?}"
                ),
                NodeKind::Group => assert!(
                    matches!(result, Err(Refusal::Incomplete { .. })),
                    "{path:?}: {result:?}"
                ),
                // served: without its path (or directory) it asks for one
                NodeKind::ExportInfo => {
                    assert!(matches!(result, Ok(Invocation::ExportQuery(_))))
                }
                NodeKind::ExportStatus | NodeKind::ExportObjects => assert!(result.is_err()),
                NodeKind::Export
                | NodeKind::Import
                | NodeKind::ImportFiles
                | NodeKind::Save
                | NodeKind::Load => {
                    assert!(
                        matches!(result, Err(Refusal::MissingValue(_))),
                        "{path:?}: {result:?}"
                    )
                }
                // served, and needs nothing but the connection
                NodeKind::Apply => assert!(
                    matches!(result, Ok(Invocation::Apply(_))),
                    "{path:?}: {result:?}"
                ),
            }
        }
        let names = paths
            .iter()
            .map(|(path, _)| path.join(" "))
            .collect::<Vec<_>>();
        for expected in [
            "create",
            "dump",
            "restore",
            "clear",
            "replicate",
            "config load",
            "config save",
            "config check",
            "config apply",
            "config reset",
            "config repair",
            "config export info",
            "config export status",
            "config export objects",
            "config export all-extensions",
            "config import files",
            "config import all-extensions",
            "config support",
            "config support disable",
            "config data-separation",
            "config data-separation list",
            "config extension",
            "config extension create",
            "config extension info",
            "config extension list",
            "config extension update",
            "config extension delete",
            "config generation-id",
            "config sign",
        ] {
            assert!(names.iter().any(|name| name == expected), "{expected}");
        }
    }

    #[test]
    fn unsupported_commands_are_refused_whatever_their_options() {
        assert_eq!(
            parse(&[
                "config",
                "check",
                "--dbms=MSSQLServer",
                "--db-server=localhost",
                "--db-name=b",
                "--force",
                "--extension=E",
            ]),
            Err(Refusal::UnsupportedCommand(
                "infobase config check".to_string()
            ))
        );
        assert_eq!(
            parse(&[
                "create",
                "--dbms=MSSQLServer",
                "--db-name=b",
                "--locale=ru_RU",
                "--create-database",
                "--import=tree",
                "--apply",
                "--force",
            ]),
            Err(Refusal::UnsupportedCommand("infobase create".to_string()))
        );
        // load is served (#353): its file is the argument
        assert!(matches!(
            parse(&["config", "load", "--db-name=b", "--user=Админ", "a.cf"]),
            Ok(Invocation::Load(LoadRequest { path, .. })) if path == "a.cf"
        ));
        assert_eq!(
            parse(&["config", "export", "objects", "-r", "Catalog.A"]),
            Err(Refusal::MissingValue("out".to_string()))
        );
    }

    #[test]
    fn a_group_without_its_command_is_incomplete() {
        assert_eq!(parse(&[]), Err(Refusal::Incomplete { path: vec![] }));
        assert_eq!(
            parse(&["config"]),
            Err(Refusal::Incomplete {
                path: vec!["config"]
            })
        );
        assert_eq!(
            parse(&["config", "exprot", "out"]),
            Err(Refusal::Incomplete {
                path: vec!["config"]
            })
        );
        assert_eq!(parse(&["bogus"]), Err(Refusal::Incomplete { path: vec![] }));
    }

    #[test]
    fn help_and_version_win_anywhere() {
        for help in ["--help", "-h", "-?"] {
            assert_eq!(parse(&[help]), Ok(Invocation::Help));
            assert_eq!(parse(&["config", "export", help]), Ok(Invocation::Help));
            assert_eq!(parse(&["config", "apply", help]), Ok(Invocation::Help));
        }
        for version in ["--version", "-v"] {
            assert_eq!(parse(&[version]), Ok(Invocation::Version));
        }
    }

    #[test]
    fn a_directory_named_like_a_subcommand_needs_to_come_later() {
        // `export info` is the subcommand, as natively; an argument given
        // first keeps later words as arguments.
        assert!(matches!(
            parse(&["config", "export", "info"]),
            Ok(Invocation::ExportQuery(_))
        ));
        let request = export(&["config", "export", "--db-name=b", r".\info"]);
        assert_eq!(request.path, OsString::from(r".\info"));
    }

    #[test]
    fn the_ibcmd_rs_options_parse() {
        let request = export(&[
            "config",
            "export",
            "--db-name=b",
            "--report=r.json",
            "--sqlcmd=C:\\sql\\SQLCMD.EXE",
            "--settings=s.json",
            "--source-version=8.5.1",
            "--db-pwd-env=MY_PWD",
            "out",
        ]);
        assert_eq!(request.common.report, Some(PathBuf::from("r.json")));
        assert_eq!(
            request.common.sqlcmd,
            Some(PathBuf::from("C:\\sql\\SQLCMD.EXE"))
        );
        assert_eq!(request.common.settings, Some(PathBuf::from("s.json")));
        assert_eq!(
            request.common.source_version,
            Some(InfobaseConfigSourceVersion::V2_21)
        );
        assert_eq!(request.common.db_pwd_env.as_deref(), Some("MY_PWD"));
        let request = import(&["config", "import", "--settings=s.json", "--base-free", "t"]);
        assert!(request.base_free);
        assert_eq!(request.common.db_name, None);
    }

    fn apply(list: &[&str]) -> ApplyRequest {
        match parse(list) {
            Ok(Invocation::Apply(request)) => request,
            other => panic!("{list:?}: {other:?}"),
        }
    }

    #[test]
    fn the_native_apply_line_of_the_lab_scripts_parses() {
        // the lab README: `infobase config apply <common> --force --dynamic=disable`
        let request = apply(&[
            "config",
            "apply",
            "--dbms=MSSQLServer",
            "--db-server=localhost",
            "--db-name=ibcmd_rs_04_x",
            r"--data=F:\lab\ibdata\x",
            "--user=Администратор",
            "--force",
            "--dynamic=disable",
            "--session-terminate=disable",
        ]);
        assert_eq!(request.common.dbms.as_deref(), Some("MSSQLServer"));
        assert_eq!(request.common.db_name.as_deref(), Some("ibcmd_rs_04_x"));
        assert_eq!(request.common.data, Some(PathBuf::from(r"F:\lab\ibdata\x")));
        assert_eq!(request.common.user.as_deref(), Some("Администратор"));
        assert!(request.force);
        assert_eq!(request.dynamic, DynamicMode::Disable);
        assert_eq!(request.session_terminate, SessionTerminate::Disable);
    }

    #[test]
    fn apply_defaults_are_the_platforms_and_every_word_is_read() {
        // the help of 8.3.27.2214: `--dynamic` auto, `--session-terminate` disable
        let request = apply(&["config", "apply", "--db-name=b"]);
        assert!(!request.force);
        assert_eq!(request.dynamic, DynamicMode::Auto);
        assert_eq!(request.session_terminate, SessionTerminate::Disable);
        assert_eq!(request.session_terminate_message, None);
        assert_eq!(request.exclusivity, ExclusivityMode::Sql);
        assert_eq!(request.backup, BackupPolicy::None);
        for (word, mode) in [
            ("auto", DynamicMode::Auto),
            ("disable", DynamicMode::Disable),
            ("prompt", DynamicMode::Prompt),
            ("force", DynamicMode::Force),
        ] {
            let request = apply(&["config", "apply", &format!("--dynamic={word}")]);
            assert_eq!(request.dynamic, mode, "{word}");
            assert_eq!(request.dynamic.as_str(), word);
        }
        for (word, mode) in [
            ("disable", SessionTerminate::Disable),
            ("prompt", SessionTerminate::Prompt),
            ("force", SessionTerminate::Force),
        ] {
            let request = apply(&["config", "apply", "--session-terminate", word]);
            assert_eq!(request.session_terminate, mode, "{word}");
            assert_eq!(request.session_terminate.as_str(), word);
        }
        let request = apply(&[
            "config",
            "apply",
            "-F",
            "--session-terminate-message=Обновление конфигурации",
            "--exclusivity=assumed",
            "-u",
            "Админ",
            "-P",
            "",
            "--db-name=b",
        ]);
        assert!(request.force);
        assert_eq!(
            request.session_terminate_message.as_deref(),
            Some("Обновление конфигурации")
        );
        assert_eq!(request.exclusivity, ExclusivityMode::Assumed);
        assert_eq!(request.common.user.as_deref(), Some("Админ"));
        // the mode's options go anywhere after the mode; apply's after `apply`
        let request = apply(&["--db-name=b", "config", "apply", "--force"]);
        assert!(request.force);
        assert_eq!(
            parse(&["config", "--force", "apply", "--db-name=b"]),
            Err(Refusal::Parse("--force".to_string()))
        );
    }

    #[test]
    fn the_backup_options_are_the_own_applys() {
        // a file: the apply takes the backup itself
        let request = apply(&[
            "config",
            "apply",
            "--recovery-backup",
            r"F:\backups\before.bak",
        ]);
        assert_eq!(
            request.backup,
            BackupPolicy::File(PathBuf::from(r"F:\backups\before.bak"))
        );
        // the word alone: the operator has one
        assert_eq!(
            apply(&["config", "apply", "--i-have-a-backup"]).backup,
            BackupPolicy::Acknowledged
        );
        // both: the file wins, as it does in `mssql-config-apply`
        assert_eq!(
            apply(&[
                "config",
                "apply",
                "--i-have-a-backup",
                "--recovery-backup=x.bak"
            ])
            .backup,
            BackupPolicy::File(PathBuf::from("x.bak"))
        );
        // a file that is not named is a malformed line, not a backup
        assert_eq!(
            parse(&["config", "apply", "--recovery-backup="]),
            Err(Refusal::InvalidValue {
                option: "--recovery-backup".to_string(),
                value: String::new()
            })
        );
        assert!(matches!(
            parse(&["config", "apply", "--recovery-backup"]),
            Err(Refusal::MissingValue(_))
        ));
        // they belong to `apply`
        assert_eq!(
            parse(&["config", "export", "--i-have-a-backup", "dir"]),
            Err(Refusal::Parse("--i-have-a-backup".to_string()))
        );
    }

    #[test]
    fn apply_errors_are_worded_as_the_platform_does() {
        // measured on 8.3.27.2214, exit 2 each: the words are case-sensitive
        // and a missing word is a wrong one
        for list in [
            vec!["config", "apply", "--dynamic=bogus"],
            vec!["config", "apply", "--dynamic=AUTO"],
            vec!["config", "apply", "--dynamic="],
            vec!["config", "apply", "-F", "--dynamic=bogus"],
            vec!["config", "apply", "--db-name=b", "--dynamic"],
        ] {
            assert_eq!(
                parse(&list),
                Err(Refusal::BadValue("dynamic".to_string())),
                "{list:?}"
            );
        }
        for list in [
            vec!["config", "apply", "--session-terminate=bogus"],
            vec!["config", "apply", "--session-terminate"],
        ] {
            assert_eq!(
                parse(&list),
                Err(Refusal::BadValue("session-terminate".to_string())),
                "{list:?}"
            );
        }
        assert_eq!(
            parse(&["config", "apply", "--force=yes"]),
            Err(Refusal::Parse("force".to_string()))
        );
        assert_eq!(
            parse(&["config", "apply", "--bogus"]),
            Err(Refusal::Parse("--bogus".to_string()))
        );
        // an option of another command is unknown here
        assert_eq!(
            parse(&["config", "apply", "--threads=4"]),
            Err(Refusal::Parse("--threads=4".to_string()))
        );
        // a stray argument is ignored, as the platform ignores it
        let request = apply(&["config", "apply", "--db-name=b", "stray"]);
        assert_eq!(request.common.db_name.as_deref(), Some("b"));
        assert_eq!(
            parse(&["config", "apply", "--exclusivity=maybe"]),
            Err(Refusal::InvalidValue {
                option: "--exclusivity".to_string(),
                value: "maybe".to_string()
            })
        );
        // the connection is checked as for export and import
        assert_eq!(
            parse(&["config", "apply", "--dbms=Foo"]),
            Err(Refusal::UnknownDbms("Foo".to_string()))
        );
        assert_eq!(
            parse(&["config", "apply", "--dbms=IBMDB2"]),
            Err(Refusal::UnsupportedDbms("IBMDB2".to_string()))
        );
        // a file infobase is parsed; `super::run_infobase` refuses to apply it
        let request = apply(&["config", "apply", "--db-path=C:\\ib"]);
        assert_eq!(request.common.db_path, Some(PathBuf::from("C:\\ib")));
    }

    #[test]
    fn what_apply_does_not_serve_is_named() {
        // `--dynamic=force` is served now: it parses, with the other words, and the apply judges the stage
        assert_eq!(
            apply(&["config", "apply", "--dynamic=force"]).dynamic,
            DynamicMode::Force
        );
        for list in [
            vec!["config", "apply", "--extension=E"],
            vec!["config", "apply", "--extension", "E"],
            vec!["config", "apply", "-e", "E"],
        ] {
            assert_eq!(apply(&list).common.extension.as_deref(), Some("E"));
        }
        for option in ["--pid=12", "--remote=http://host:1545"] {
            assert!(
                matches!(
                    parse(&["config", "apply", option]),
                    Err(Refusal::UnsupportedServer(_))
                ),
                "{option}"
            );
        }
        match parse(&["config", "apply", "--sqlcmd=C:\\sql\\SQLCMD.EXE"]) {
            Err(Refusal::Unsupported(message)) => {
                assert!(message.contains("`--sqlcmd`"), "{message}");
                assert!(message.contains("встроенный клиент"), "{message}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn save_takes_one_file_and_db() {
        match parse(&[
            "config",
            "save",
            "--dbms=MSSQLServer",
            "--db-name=b",
            "--db",
            "--user=Админ",
            "out.cf",
        ]) {
            Ok(Invocation::Save(request)) => {
                assert!(request.database_configuration);
                assert_eq!(request.path, OsString::from("out.cf"));
                assert_eq!(request.common.db_name.as_deref(), Some("b"));
                assert_eq!(request.common.user.as_deref(), Some("Админ"));
            }
            other => panic!("{other:?}"),
        }
        match parse(&["config", "save", "--db-name=b", "out.cf"]) {
            Ok(Invocation::Save(request)) => assert!(!request.database_configuration),
            other => panic!("{other:?}"),
        }
        // `--db` is `save`'s own: before the command it is not known yet
        assert_eq!(
            parse(&["config", "--db", "save", "out.cf"]),
            Err(Refusal::Parse("--db".to_string()))
        );
        // a flag takes no value; the platform names it without the dashes
        assert_eq!(
            parse(&["config", "save", "--db=yes", "out.cf"]),
            Err(Refusal::Parse("db".to_string()))
        );
        // export's options are not save's
        assert_eq!(
            parse(&["config", "save", "--force", "out.cf"]),
            Err(Refusal::Parse("--force".to_string()))
        );
        for option in [vec!["--extension=E"], vec!["-e", "E"]] {
            let mut list = vec!["config", "save", "--db-name=b"];
            list.extend(option.iter().copied());
            list.push("out.cf");
            assert_eq!(
                parse(&list),
                Err(Refusal::UnsupportedOption {
                    option: option[0].split('=').next().unwrap().to_string(),
                    command: "infobase config save".to_string(),
                }),
                "{option:?}"
            );
        }
        assert_eq!(
            parse(&["config", "save", "--db-name=b"]),
            Err(Refusal::MissingValue(
                "путь к файлу конфигурации".to_string()
            ))
        );
        assert_eq!(
            parse(&["config", "save", "--db-name=b", "a.cf", "b.cf"]),
            Err(Refusal::Parse("b.cf".to_string()))
        );
        assert!(matches!(
            parse(&["config", "save", "--remote=http://host:1545", "a.cf"]),
            Err(Refusal::UnsupportedServer(_))
        ));
    }
}
