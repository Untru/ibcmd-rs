//! Drop-in `ibcmd`: the platform ibcmd's command line, so that scripts
//! written for it run against ibcmd-rs renamed to `ibcmd`.
//!
//! - `infobase config export` and `infobase config import` are served
//!   (`crate::infobase`), for Microsoft SQL Server infobases, and so is
//!   `infobase config import files` (the partial import of listed files,
//!   `crate::mssql::files_stage`, Untru/ibcmd-rs#363);
//! - `infobase config apply` is served by the own exclusive apply
//!   (`crate::mssql_config_apply`, [`apply`]): it moves the staged
//!   configuration into the active one, refuses what needs a restructuring
//!   (`требуется штатный config apply: ...`) and what a connected session
//!   keeps from an exclusive lock; `--dynamic=force` is served by the dynamic
//!   apply (`crate::mssql_config_apply::dynamic`), which publishes a small
//!   stage as a generation while sessions stay connected;
//! - `infobase config save [--db] <file>` writes the configuration's rows as
//!   a `.cf` (`crate::infobase::save_config`, Untru/ibcmd-rs#352);
//! - every other native mode, command and option is recognized and refused
//!   by name with exit code 1 (`Команда `infobase config check` не
//!   поддерживается в этой версии ibcmd-rs (планируется в следующих)`);
//!   the platform is never launched in its place.
//!
//! Output follows the platform's: `[INFO] ...` lines on stdout when an
//! operation starts and when it succeeds, `[ERROR] ...` lines on stderr when
//! it fails, a bare line on stderr for a refused command line (the list of
//! commands for an incomplete one goes to stdout, as natively). The JSON
//! report is written only to `--report <file>`. Exit codes are the
//! platform's (measured on 8.3.27.2214): 0 success, 2 a malformed or
//! incomplete command line, -1 a failed operation (255 to a POSIX shell).
//! 1 is ibcmd-rs's own: a mode, command, option or DBMS this version does
//! not serve, which the platform would have run (the platform itself exits
//! 1 for `config apply` only when its metadata check finds errors).

pub mod apply;
pub mod help;
pub mod parse;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::{
    InfobaseConfigExportArgs, InfobaseConfigImportArgs, InfobaseConfigSaveArgs,
    InfobaseImportStageMode, InfobaseImportVerify,
};
use crate::infobase::OutputDirectoryNotEmpty;
pub use parse::{
    ApplyRequest, Common, ExportRequest, ImportFilesRequest, ImportRequest, Invocation, Refusal,
    SaveRequest,
};

const PLANNED: &str = "не поддерживается в этой версии ibcmd-rs (планируется в следующих)";

/// Exit code of what this version does not serve (no platform equivalent).
pub const EXIT_UNSUPPORTED: i32 = 1;
/// Exit code of a malformed or incomplete command line, as the platform's.
pub const EXIT_MALFORMED: i32 = 2;
/// Exit code of a failed operation, as the platform's: -1, which Windows
/// shows as -1 (`%ERRORLEVEL%`, `$LASTEXITCODE`) and a POSIX shell as 255.
pub const EXIT_FAILED: i32 = -1;

/// The exit code of a refused command line: the platform's for a line it
/// would refuse too, ibcmd-rs's own for one the platform would run.
pub fn refusal_exit_code(refusal: &Refusal) -> i32 {
    match refusal {
        Refusal::Parse(_)
        | Refusal::MissingValue(_)
        | Refusal::Incomplete { .. }
        | Refusal::UnknownDbms(_)
        | Refusal::InvalidValue { .. }
        | Refusal::BadValue(_)
        | Refusal::Conflict { .. } => EXIT_MALFORMED,
        Refusal::UnsupportedCommand(_)
        | Refusal::UnsupportedOption { .. }
        | Refusal::UnsupportedServer(_)
        | Refusal::UnsupportedDbms(_)
        | Refusal::FileInfobase
        | Refusal::ImportArchive(_)
        | Refusal::Unsupported(_) => EXIT_UNSUPPORTED,
    }
}

/// The name the program was started as (`ibcmd` once renamed), for the
/// usage lines of the help.
pub fn program_name() -> String {
    std::env::args_os()
        .next()
        .and_then(|path| {
            Path::new(&path)
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "ibcmd".to_string())
}

/// `ibcmd infobase ...`: returns the exit code.
pub fn run_infobase(args: &[OsString]) -> i32 {
    #[cfg(feature = "platform-oracle")]
    if let Some(code) = oracle::run(args) {
        return finish(code);
    }
    let code = match parse::parse_infobase(args) {
        Ok(Invocation::Help) => {
            print!("{}", help::infobase_help(&program_name()));
            0
        }
        Ok(Invocation::Version) => {
            println!("ibcmd-rs {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Ok(Invocation::Export(request)) => run_export(request),
        Ok(Invocation::Import(request)) => run_import(request),
        Ok(Invocation::ImportFiles(request)) => run_import_files(request),
        Ok(Invocation::Apply(request)) => apply::run(request),
        Ok(Invocation::Save(request)) => run_save(request),
        Err(refusal) => {
            print_refusal(&refusal, &program_name());
            refusal_exit_code(&refusal)
        }
    };
    finish(code)
}

/// `ibcmd server ...` and the platform's other modes: refused.
pub fn run_other_mode(mode: &str) -> i32 {
    eprintln!("{}", unsupported_mode_message(mode));
    finish(EXIT_UNSUPPORTED)
}

/// `ibcmd help [MODE]`: the platform's help mode for its modes, this
/// program's own help for its research commands.
pub fn run_help(args: &[OsString]) -> i32 {
    let program = program_name();
    let Some(first) = args.first() else {
        print!("{}", help::overview(&program));
        return finish(0);
    };
    let name = first.to_string_lossy().into_owned();
    if name == "infobase" {
        print!("{}", help::infobase_help(&program));
        return finish(0);
    }
    if parse::OTHER_MODES.iter().any(|(mode, _)| *mode == name) {
        return run_other_mode(&name);
    }
    use clap::Parser;
    match crate::cli::Cli::try_parse_from([program.as_str(), name.as_str(), "--help"]) {
        Err(error) if !matches!(error.kind(), clap::error::ErrorKind::InvalidSubcommand) => {
            let _ = error.print();
            finish(0)
        }
        _ => {
            eprintln!(
                "Неизвестный режим: {name}. Список режимов: {program} help; команды ibcmd-rs: {program} --help"
            );
            finish(EXIT_MALFORMED)
        }
    }
}

pub fn unsupported_mode_message(mode: &str) -> String {
    format!("Режим `{mode}` {PLANNED}")
}

/// The message of a refused command line, and whether it goes to stdout
/// (the platform prints the list of an incomplete command there).
pub fn refusal_message(refusal: &Refusal, program: &str) -> (String, bool) {
    let message = match refusal {
        Refusal::Parse(argument) => format!("Ошибка разбора параметра: {argument}"),
        Refusal::MissingValue(name) => format!("Не указано значение параметра: {name}"),
        Refusal::Incomplete { path } => return (incomplete_message(path, program), true),
        Refusal::UnsupportedCommand(command) => format!("Команда `{command}` {PLANNED}"),
        Refusal::UnsupportedOption { option, command } => {
            format!("Параметр `{option}` команды `{command}` {PLANNED}")
        }
        Refusal::UnsupportedServer(option) => {
            format!("Параметр `{option}` (работа через автономный сервер) {PLANNED}")
        }
        Refusal::UnsupportedDbms(dbms) => format!(
            "СУБД `{dbms}` не поддерживается в этой версии ibcmd-rs: поддерживается только MSSQLServer"
        ),
        Refusal::UnknownDbms(dbms) => format!("Указанный тип СУБД не поддерживается: '{dbms}'"),
        Refusal::FileInfobase => {
            "Файловые информационные базы не поддерживаются в этой версии ibcmd-rs: \
укажите --dbms=MSSQLServer, --db-server и --db-name"
                .to_string()
        }
        Refusal::ImportArchive(path) => format!(
            "Импорт конфигурации из архива ({}) {PLANNED}",
            path.display()
        ),
        Refusal::InvalidValue { option, value } => {
            format!("Недопустимое значение параметра {option}: {value}")
        }
        Refusal::BadValue(name) => format!("Некорректное значение параметра: {name}"),
        Refusal::Conflict { first, second } => {
            format!("Параметры {first} и {second} нельзя указывать вместе")
        }
        Refusal::Unsupported(message) => message.clone(),
    };
    (message, false)
}

fn print_refusal(refusal: &Refusal, program: &str) {
    match refusal_message(refusal, program) {
        (message, true) => println!("{message}"),
        (message, false) => eprintln!("{message}"),
    }
}

/// `Указана неполная команда, возможно Вы имели ввиду:` and the commands
/// under the one given, as the platform lists them.
fn incomplete_message(path: &[&str], program: &str) -> String {
    let node = parse::node_at(path).unwrap_or(&parse::INFOBASE);
    let prefix = std::iter::once(program)
        .chain(std::iter::once("infobase"))
        .chain(path.iter().copied())
        .collect::<Vec<_>>()
        .join(" ");
    let width = node
        .children
        .iter()
        .map(|child| child.name.chars().count())
        .max()
        .unwrap_or(0)
        + 4;
    let mut out = String::from("Указана неполная команда, возможно Вы имели ввиду:\n");
    for (index, child) in node.children.iter().enumerate() {
        let lead = if index == 0 {
            prefix.clone()
        } else {
            " ".repeat(prefix.chars().count())
        };
        let padding = width - child.name.chars().count();
        out.push_str(&format!(
            "\n\t{lead} {}{}- {}",
            child.name,
            " ".repeat(padding),
            child.summary
        ));
    }
    out
}

fn finish(code: i32) -> i32 {
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    code
}

/// `-W`: the database password is the first line of STDIN.
fn read_requested_password(common: &mut Common) -> Result<(), String> {
    if !common.request_db_pwd {
        return Ok(());
    }
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("не удалось прочитать пароль из STDIN: {error}"))?;
    common.db_pwd = Some(line.trim_end_matches(['\r', '\n']).to_string());
    Ok(())
}

/// Where an import leaves its scripts and its bulk rows file: `--temp`
/// (relative to `--data`), else `<--data>/temp`, where the platform keeps
/// its own temporary files, else the system's temporary directory.
pub fn scratch_dir(common: &Common) -> PathBuf {
    match (&common.temp, &common.data) {
        (Some(temp), Some(data)) if temp.is_relative() => data.join(temp),
        (Some(temp), _) => temp.clone(),
        (None, Some(data)) => data.join("temp"),
        (None, None) => std::env::temp_dir(),
    }
    .join("ibcmd-rs")
}

fn file_part(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

/// The platform spells a named instance `server/instance` (the example of
/// its own help); SQL Server's tools read `server\instance`. A server name
/// holds no `/` otherwise, and `lpc:` and the like pass as they are.
pub fn sql_server_name(value: &str) -> String {
    value.replace('/', "\\")
}

pub fn export_args(request: &ExportRequest) -> InfobaseConfigExportArgs {
    let common = &request.common;
    InfobaseConfigExportArgs {
        settings: common.settings.clone(),
        native_config: common.native_config.clone(),
        format: None,
        platform: common.platform,
        source_version: common.source_version,
        dbms: common.dbms.clone(),
        db_server: common.db_server.as_deref().map(sql_server_name),
        db_name: common.db_name.clone(),
        db_user: common.db_user.clone(),
        db_pwd: common.db_pwd.clone(),
        db_pwd_env: common
            .db_pwd_env
            .clone()
            .unwrap_or_else(|| "IBCMD_DB_PSW".to_string()),
        user: common.user.clone(),
        password: common.password.clone(),
        password_env: "IBCMD_USER_PSW".to_string(),
        // the built-in SQL Server client, unless `--sqlcmd` asks for sqlcmd.exe
        sqlcmd: common.sqlcmd.clone(),
        extension: request.extension.clone(),
        overwrite: false,
        count_files: common.report.is_some(),
        output_dir: PathBuf::from(&request.path),
        base: request.base.clone(),
        sync: request.sync,
    }
}

pub fn import_args(request: &ImportRequest) -> InfobaseConfigImportArgs {
    let common = &request.common;
    let script = scratch_dir(common).join(format!(
        "{}_import.sql",
        file_part(common.db_name.as_deref().unwrap_or("import"))
    ));
    InfobaseConfigImportArgs {
        settings: common.settings.clone(),
        native_config: common.native_config.clone(),
        format: None,
        platform: common.platform,
        source_version: common.source_version,
        dbms: common.dbms.clone(),
        db_server: common.db_server.as_deref().map(sql_server_name),
        db_name: common.db_name.clone(),
        db_user: common.db_user.clone(),
        db_pwd: common.db_pwd.clone(),
        db_pwd_env: common
            .db_pwd_env
            .clone()
            .unwrap_or_else(|| "IBCMD_DB_PSW".to_string()),
        user: common.user.clone(),
        password: common.password.clone(),
        password_env: "IBCMD_USER_PSW".to_string(),
        // the built-in SQL Server client, unless `--sqlcmd` asks for sqlcmd.exe
        sqlcmd: common.sqlcmd.clone(),
        // An import replaces the saved configuration, as the platform's does.
        replace_config_save: true,
        allow_non_lab: true,
        batch_size: None,
        path_prefix: Vec::new(),
        files: Vec::new(),
        script_output: Some(script),
        stage_mode: if request.base_free {
            InfobaseImportStageMode::BaseFree
        } else {
            InfobaseImportStageMode::Auto
        },
        verify: if request.no_verify {
            InfobaseImportVerify::Off
        } else if request.verify {
            InfobaseImportVerify::On
        } else {
            InfobaseImportVerify::Auto
        },
        source_dir: PathBuf::from(&request.path),
    }
}

/// The import of `import files`: the patch import of the directory, limited
/// to the rows of the listed files.
pub fn import_files_args(request: &ImportFilesRequest) -> InfobaseConfigImportArgs {
    let mut args = import_args(&ImportRequest {
        common: request.common.clone(),
        base_free: false,
        verify: request.verify,
        no_verify: request.no_check,
        path: request.base_dir.clone().into_os_string(),
    });
    args.stage_mode = InfobaseImportStageMode::Patch;
    args.files = request.files.clone();
    args
}

pub fn save_args(request: &SaveRequest) -> InfobaseConfigSaveArgs {
    let common = &request.common;
    InfobaseConfigSaveArgs {
        settings: common.settings.clone(),
        native_config: common.native_config.clone(),
        dbms: common.dbms.clone(),
        db_server: common.db_server.as_deref().map(sql_server_name),
        db_name: common.db_name.clone(),
        db_user: common.db_user.clone(),
        db_pwd: common.db_pwd.clone(),
        db_pwd_env: common
            .db_pwd_env
            .clone()
            .unwrap_or_else(|| "IBCMD_DB_PSW".to_string()),
        sqlcmd: common.sqlcmd.clone(),
        database_configuration: request.database_configuration,
        // Saving to a file that exists replaces it, as saving a file does;
        // the old one stays whole until the new one is complete. The
        // platform's own behaviour here is not measured.
        overwrite: true,
        output: PathBuf::from(&request.path),
    }
}

/// The failure a `--report` file records.
#[derive(Serialize)]
struct FailureReport<'a> {
    operation: &'a str,
    ok: bool,
    error: String,
}

fn write_json(path: &Path, value: &impl Serialize) -> anyhow::Result<()> {
    use anyhow::Context;
    let json = serde_json::to_string_pretty(value)?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, json)
        .with_context(|| format!("failed to write the report {}", path.display()))
}

/// One operation with the platform's messages around it.
struct Operation {
    /// `infobase config export`, for the report.
    command: &'static str,
    /// `Экспорт конфигурации в XML`.
    title: &'static str,
    /// The participle that agrees with the title: `завершен` or `завершено`.
    ended: &'static str,
}

impl Operation {
    fn start(&self) {
        println!("[INFO] {}...", self.title);
        let _ = std::io::stdout().flush();
    }

    fn succeed(&self, report: Option<&Path>, value: &impl Serialize) -> i32 {
        if let Some(path) = report
            && let Err(error) = write_json(path, value)
        {
            return self.fail_with(&format!("{error:#}"), None);
        }
        println!("[INFO] {} успешно {}", self.title, self.ended);
        0
    }

    fn fail_with(&self, message: &str, report: Option<&Path>) -> i32 {
        for line in message.lines() {
            eprintln!("[ERROR] {line}");
        }
        eprintln!("[ERROR] {} {} с ошибкой", self.title, self.ended);
        self.record_failure(message, report);
        EXIT_FAILED
    }

    /// The operation is not carried out because this version does not serve
    /// what it needs: the reason alone, exit 1.
    fn refuse_with(&self, message: &str, report: Option<&Path>) -> i32 {
        for line in message.lines() {
            eprintln!("[ERROR] {line}");
        }
        self.record_failure(message, report);
        EXIT_UNSUPPORTED
    }

    fn record_failure(&self, message: &str, report: Option<&Path>) {
        if let Some(path) = report {
            let failure = FailureReport {
                operation: self.command,
                ok: false,
                error: message.to_string(),
            };
            if let Err(error) = write_json(path, &failure) {
                eprintln!("[ERROR] {error:#}");
            }
        }
    }
}

const EXPORT: Operation = Operation {
    command: "infobase config export",
    title: "Экспорт конфигурации в XML",
    ended: "завершен",
};

const IMPORT: Operation = Operation {
    command: "infobase config import",
    title: "Импорт конфигурации из XML",
    ended: "завершен",
};

/// The platform's own lines of `import files` (8.3.27 and 8.5, recorded in
/// `docs/import/evidence/419/add85-vs-native.json` and
/// `native-partial-rem-refusal.json`): `[INFO] Импорт файлов конфигурации из
/// XML...`, `... успешно завершен`, `... завершен с ошибкой`.
const IMPORT_FILES: Operation = Operation {
    command: "infobase config import files",
    title: "Импорт файлов конфигурации из XML",
    ended: "завершен",
};

const APPLY: Operation = Operation {
    command: "infobase config apply",
    title: "Обновление конфигурации базы данных",
    ended: "завершено",
};

/// The title is the platform's own name of the command (`ibcmd help
/// infobase`: `save - Выгрузка конфигурации`), as every served command's
/// title is; the lines `config save` itself prints are not measured.
const SAVE: Operation = Operation {
    command: "infobase config save",
    title: "Выгрузка конфигурации",
    ended: "завершена",
};

fn run_export(mut request: ExportRequest) -> i32 {
    let report = request.common.report.clone();
    if let Err(message) = read_requested_password(&mut request.common) {
        EXPORT.start();
        return EXPORT.fail_with(&message, report.as_deref());
    }
    if let Some(threads) = request.threads {
        crate::parallel::request_workers(threads);
    }
    // The platform's lines only; `IBCMD_RS_VERBOSE=1` keeps the export's
    // research summaries on stderr.
    crate::mssql_dump::model_export::set_quiet_summaries(
        std::env::var_os("IBCMD_RS_VERBOSE").is_none_or(|value| value != "1"),
    );
    let args = export_args(&request);
    EXPORT.start();
    match crate::infobase::export_config(&args) {
        Ok(value) => EXPORT.succeed(report.as_deref(), &value),
        Err(error) if error.downcast_ref::<OutputDirectoryNotEmpty>().is_some() => {
            let message = format!(
                "Операция невозможна, при выполнении экспорта конфигурации в XML обнаружены ошибки: Каталог {} не пуст.",
                Path::new(&request.path).display()
            );
            eprintln!("[ERROR] {message}");
            EXPORT.record_failure(&message, report.as_deref());
            EXIT_FAILED
        }
        Err(error) => EXPORT.fail_with(&format!("{error:#}"), report.as_deref()),
    }
}

fn run_import(mut request: ImportRequest) -> i32 {
    let report = request.common.report.clone();
    if let Err(message) = read_requested_password(&mut request.common) {
        IMPORT.start();
        return IMPORT.fail_with(&message, report.as_deref());
    }
    let args = import_args(&request);
    IMPORT.start();
    match crate::infobase::import_config(&args) {
        Ok(value) => IMPORT.succeed(report.as_deref(), &value),
        Err(error) => IMPORT.fail_with(&format!("{error:#}"), report.as_deref()),
    }
}

fn run_import_files(mut request: ImportFilesRequest) -> i32 {
    use crate::mssql::files_stage::{FilesRefused, select};
    let report = request.common.report.clone();
    if let Err(message) = read_requested_password(&mut request.common) {
        IMPORT_FILES.start();
        return IMPORT_FILES.fail_with(&message, report.as_deref());
    }
    IMPORT_FILES.start();
    let refused = |refused: &FilesRefused| {
        if refused.unsupported {
            IMPORT_FILES.refuse_with(&refused.message, report.as_deref())
        } else {
            IMPORT_FILES.fail_with(&refused.message, report.as_deref())
        }
    };
    // The files are checked against the directory before any connection.
    if let Err(refusal) = select(&request.base_dir, &request.files) {
        return refused(&refusal);
    }
    // Without `--partial` the directory is taken for a whole export of the
    // configuration. What the platform does with a directory that is not one
    // is not measured, so such a directory is refused rather than guessed at.
    if !request.partial && !request.base_dir.join("Configuration.xml").is_file() {
        return IMPORT_FILES.refuse_with(
            &format!(
                "Каталог {} не является полной выгрузкой конфигурации (в нем нет Configuration.xml); \
                 для каталога с частью файлов конфигурации укажите --partial",
                request.base_dir.display()
            ),
            report.as_deref(),
        );
    }
    let args = import_files_args(&request);
    match crate::infobase::import_config(&args) {
        Ok(value) => IMPORT_FILES.succeed(report.as_deref(), &value),
        Err(error) => match error.downcast_ref::<FilesRefused>() {
            Some(refusal) => refused(refusal),
            None => IMPORT_FILES.fail_with(&format!("{error:#}"), report.as_deref()),
        },
    }
}

fn run_save(mut request: SaveRequest) -> i32 {
    let report = request.common.report.clone();
    if let Err(message) = read_requested_password(&mut request.common) {
        SAVE.start();
        return SAVE.fail_with(&message, report.as_deref());
    }
    let args = save_args(&request);
    SAVE.start();
    match crate::infobase::save_config(&args) {
        Ok(value) => SAVE.succeed(report.as_deref(), &value),
        Err(error) => SAVE.fail_with(&format!("{error:#}"), report.as_deref()),
    }
}

/// `ibcmd-rs infobase config roundtrip|sweep`: research commands that run
/// the installed platform, only in `platform-oracle` builds.
#[cfg(feature = "platform-oracle")]
mod oracle {
    use std::ffi::OsString;

    use clap::Parser;

    use crate::cli::{InfobaseOracleCli, InfobaseOracleCommands};

    pub(super) fn run(args: &[OsString]) -> Option<i32> {
        let [config, command, ..] = args else {
            return None;
        };
        if config != "config" || !(command == "roundtrip" || command == "sweep") {
            return None;
        }
        let cli = match InfobaseOracleCli::try_parse_from(
            std::iter::once(OsString::from("ibcmd-rs infobase config"))
                .chain(args[1..].iter().cloned()),
        ) {
            Ok(cli) => cli,
            Err(error) => {
                let _ = error.print();
                return Some(error.exit_code());
            }
        };
        let rendered = match cli.command {
            InfobaseOracleCommands::Roundtrip(args) => {
                crate::infobase_oracle::roundtrip_config(&args)
                    .and_then(|report| Ok(serde_json::to_string_pretty(&report)?))
            }
            InfobaseOracleCommands::Sweep(args) => crate::infobase_oracle::sweep_config(&args)
                .and_then(|report| Ok(serde_json::to_string_pretty(&report)?)),
        };
        Some(match rendered {
            Ok(json) => {
                println!("{json}");
                0
            }
            Err(error) => {
                eprintln!("Error: {error:?}");
                1
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn common() -> Common {
        Common {
            db_name: Some("base".to_string()),
            ..Common::default()
        }
    }

    #[test]
    fn the_import_scratch_follows_the_data_and_temp_directories() {
        let mut common = common();
        assert_eq!(scratch_dir(&common), std::env::temp_dir().join("ibcmd-rs"));
        common.data = Some(PathBuf::from(r"F:\run\ibdata"));
        assert_eq!(
            scratch_dir(&common),
            PathBuf::from(r"F:\run\ibdata")
                .join("temp")
                .join("ibcmd-rs")
        );
        common.temp = Some(PathBuf::from("scratch"));
        assert_eq!(
            scratch_dir(&common),
            PathBuf::from(r"F:\run\ibdata")
                .join("scratch")
                .join("ibcmd-rs")
        );
        let absolute = std::env::temp_dir().join("elsewhere");
        common.temp = Some(absolute.clone());
        assert_eq!(scratch_dir(&common), absolute.join("ibcmd-rs"));
    }

    #[test]
    fn requests_become_the_library_arguments() {
        let export = ExportRequest {
            common: Common {
                dbms: Some("MSSQLServer".to_string()),
                db_server: Some("sql01".to_string()),
                report: Some(PathBuf::from("r.json")),
                ..common()
            },
            threads: Some(4),
            extension: Some("Расширение".to_string()),
            base: Some(PathBuf::from("ConfigDumpInfo.xml")),
            sync: true,
            path: OsString::from("out"),
        };
        let args = export_args(&export);
        assert_eq!(args.extension.as_deref(), Some("Расширение"));
        assert_eq!(args.base, Some(PathBuf::from("ConfigDumpInfo.xml")));
        assert!(args.sync);
        assert_eq!(args.db_server.as_deref(), Some("sql01"));
        assert_eq!(sql_server_name("sql01/inst"), r"sql01\inst");
        assert_eq!(sql_server_name(r"lpc:sql01\inst"), r"lpc:sql01\inst");
        assert_eq!(args.db_pwd_env, "IBCMD_DB_PSW");
        assert_eq!(args.sqlcmd, None);
        assert!(!args.overwrite);
        assert!(args.count_files);
        assert_eq!(args.output_dir, PathBuf::from("out"));

        let import = ImportRequest {
            common: Common {
                data: Some(PathBuf::from(r"F:\run\ibdata")),
                ..common()
            },
            base_free: false,
            verify: false,
            no_verify: false,
            path: OsString::from("tree"),
        };
        let args = import_args(&import);
        assert!(args.replace_config_save && args.allow_non_lab);
        assert_eq!(args.stage_mode, InfobaseImportStageMode::Auto);
        assert_eq!(args.verify, InfobaseImportVerify::Auto);
        assert_eq!(
            args.script_output,
            Some(
                PathBuf::from(r"F:\run\ibdata")
                    .join("temp")
                    .join("ibcmd-rs")
                    .join("base_import.sql")
            )
        );
        let args = import_args(&ImportRequest {
            base_free: true,
            ..import.clone()
        });
        assert_eq!(args.stage_mode, InfobaseImportStageMode::BaseFree);
        let args = import_args(&ImportRequest {
            no_verify: true,
            ..import.clone()
        });
        assert_eq!(args.verify, InfobaseImportVerify::Off);
        let args = import_args(&ImportRequest {
            verify: true,
            ..import
        });
        assert_eq!(args.verify, InfobaseImportVerify::On);

        let save = SaveRequest {
            common: Common {
                db_server: Some("sql01/inst".to_string()),
                ..common()
            },
            database_configuration: true,
            path: OsString::from("out.cf"),
        };
        let args = save_args(&save);
        assert!(args.database_configuration && args.overwrite);
        assert_eq!(args.db_server.as_deref(), Some(r"sql01\inst"));
        assert_eq!(args.db_pwd_env, "IBCMD_DB_PSW");
        assert_eq!(args.output, PathBuf::from("out.cf"));
    }

    #[test]
    fn refusals_speak_russian_and_name_what_is_refused() {
        let (message, stdout) = refusal_message(
            &Refusal::UnsupportedCommand("infobase config check".to_string()),
            "ibcmd",
        );
        assert_eq!(
            message,
            "Команда `infobase config check` не поддерживается в этой версии ibcmd-rs (планируется в следующих)"
        );
        assert!(!stdout);
        assert_eq!(
            unsupported_mode_message("server"),
            "Режим `server` не поддерживается в этой версии ibcmd-rs (планируется в следующих)"
        );
        let (message, stdout) = refusal_message(
            &Refusal::Incomplete {
                path: vec!["config"],
            },
            "ibcmd",
        );
        assert!(stdout);
        assert!(message.starts_with("Указана неполная команда, возможно Вы имели ввиду:"));
        assert!(message.contains("\tibcmd infobase config load"));
        assert!(message.contains("export"));
        assert!(message.contains("- Импорт конфигурации из XML"));
        let (message, _) = refusal_message(&Refusal::Parse("--bogus".to_string()), "ibcmd");
        assert_eq!(message, "Ошибка разбора параметра: --bogus");
    }

    #[test]
    fn refusals_exit_as_the_platform_where_it_refuses_too() {
        // 8.3.27.2214: an unknown option, a missing value, an incomplete
        // command and an unknown DBMS all exit 2.
        for refusal in [
            Refusal::Parse("--bogus".to_string()),
            Refusal::MissingValue("path".to_string()),
            Refusal::Incomplete {
                path: vec!["config"],
            },
            Refusal::UnknownDbms("Foo".to_string()),
            Refusal::InvalidValue {
                option: "--threads".to_string(),
                value: "many".to_string(),
            },
            Refusal::BadValue("dynamic".to_string()),
        ] {
            assert_eq!(refusal_exit_code(&refusal), EXIT_MALFORMED, "{refusal:?}");
        }
        // the platform's words for a wrong `--dynamic` or `--session-terminate`
        assert_eq!(
            refusal_message(&Refusal::BadValue("session-terminate".to_string()), "ibcmd").0,
            "Некорректное значение параметра: session-terminate"
        );
        // what the platform would run exits 1, a code it never uses for them
        for refusal in [
            Refusal::UnsupportedCommand("infobase config check".to_string()),
            Refusal::UnsupportedDbms("PostgreSQL".to_string()),
            Refusal::FileInfobase,
            Refusal::Unsupported("Параметр `--sqlcmd` не поддерживается".to_string()),
        ] {
            assert_eq!(refusal_exit_code(&refusal), EXIT_UNSUPPORTED, "{refusal:?}");
        }
    }
}
