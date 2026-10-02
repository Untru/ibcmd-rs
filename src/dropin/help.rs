//! The Russian help of the drop-in modes, in the layout of the platform's
//! `ibcmd help infobase`.

use super::parse::{INFOBASE, NodeKind, OTHER_MODES, command_paths};

/// `ibcmd infobase --help` (and `ibcmd help infobase`).
pub fn infobase_help(program: &str) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let unsupported = command_paths()
        .into_iter()
        .filter(|(path, kind)| {
            *kind == NodeKind::Unsupported
                // a command whose parent is already listed as unsupported
                && super::parse::node_at(&path[..path.len() - 1])
                    .is_none_or(|parent| parent.kind != NodeKind::Unsupported)
        })
        .map(|(path, _)| format!("        {program} infobase {}", path.join(" ")))
        .collect::<Vec<_>>()
        .join("\n");
    let modes = OTHER_MODES
        .iter()
        .map(|(mode, _)| *mode)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "ibcmd-rs {version}: выгрузка и загрузка конфигурации информационной базы Microsoft SQL Server \
без платформы 1С:Предприятия, с командной строкой ibcmd
{summary}

Использование:

\t{program} infobase [command] [options] [arguments]

Общие параметры:

    --version | -v
        получение версии утилиты

    --help | -? | -h
        отображение краткой информации об утилите

Режим:

    infobase
        {summary}

Параметры:

    --config=<path> | -c <path>
        Путь к конфигурационному файлу ibcmd. Из его раздела database:
        берутся СУБД, сервер, база, пользователь и пароль, не заданные
        параметрами

    --dbms=<kind>
        Тип СУБД, в которой размещается информационная база.
        Поддерживается только MSSQLServer

    --database-server=<server> | --db-server=<server>
        Имя сервера СУБД: server, server\\instance, server/instance или
        server,port (по умолчанию из настроек, иначе localhost)

    --database-name=<name> | --db-name=<name>
        Имя базы данных

    --database-user=<name> | --db-user=<name>
        Имя пользователя сервера СУБД. Без него (и без настроек) используется
        проверка подлинности Windows

    --database-password=<password> | --db-pwd=<password>
        Пароль пользователя сервера СУБД. Без него берется из переменной
        окружения IBCMD_DB_PSW или из раздела database: файла --config

    --request-database-password | --request-db-pwd | -W
        Запрос пароля пользователя сервера СУБД через стандартный поток ввода (STDIN)

    --data=<path> | -d <path>
        Путь к каталогу данных сервера. Импорт оставляет служебные файлы
        в его подкаталоге temp

    --temp=<path> | -t <path>
        Путь к каталогу временных файлов (относительный путь считается от
        каталога данных сервера)

    --system, --lock, --users-data, --session-data, --stt-data, --log-data,
    --ftext-data, --ftext2-data, --openid-data, --bin-data-strg,
    --clnt-notif-data, --websocket-data
        Принимаются для совместимости и не используются

    --database-path=<path> | --db-path=<path>
        Файловые информационные базы не поддерживаются

Команды:

    config
        Управление конфигурацией информационной базы

        --user=<name> | -u <name>
            Имя пользователя информационной базы. Принимается и не нужно:
            ibcmd-rs читает и пишет таблицы конфигурации напрямую

        --password=<password> | -P <password>
            Пароль пользователя информационной базы (принимается)

        Дополнительные команды:
            export
                Экспорт конфигурации в XML

                --extension=<name> | -e <name>
                    Экспортировать расширение конфигурации с этим именем, а не
                    конфигурацию. Выгружается то состояние расширения, которое
                    выгружает ibcmd: подготовленное к применению, если оно есть

                --threads=<n> | -T <n>
                    Количество потоков, используемых при экспорте

                --force
                    Принимается для совместимости. Каталог выгрузки, как и у
                    ibcmd, должен быть пустым или отсутствовать (кроме
                    выгрузки с --base или --sync)

                --ignore-unresolved-refs
                    Принимается для совместимости

                --base=<path> | -b <path>
                    Файл ConfigDumpInfo.xml предыдущей выгрузки. Выгружаются
                    только объекты, версия (configVersion) которых отличается
                    от указанной в файле, и ConfigDumpInfo.xml всей
                    конфигурации; файлы остальных объектов не записываются.
                    Если объект или его реквизит переименован, выгружается вся
                    конфигурация. Каталог может содержать предыдущую выгрузку

                --sync
                    Синхронизировать каталог с конфигурацией: после выгрузки
                    удалить файлы, которых нет в полной выгрузке (удаленных и
                    переименованных объектов). Затрагиваются только
                    Configuration.xml, ConfigDumpInfo.xml, Ext и каталоги
                    объектов метаданных, каталоги с точкой (.git) - никогда.
                    Вместе с --base - инкрементальное обновление выгрузки

                <path>
                    путь к каталогу файлов конфигурации

            import
                Импорт конфигурации из XML в сохраненную конфигурацию базы
                (таблица ConfigSave); применяет ее команда config apply.
                Если в базе нет конфигурации дерева (пустая база), каждая
                строка собирается из дерева, конфигурация базы не читается

                <path>
                    путь к каталогу с файлами конфигурации

                Дополнительные команды:
                    files
                        Импорт выбранных файлов конфигурации из XML: в
                        ConfigSave записываются только строки, которые
                        собираются из перечисленных файлов (описание объекта,
                        модуль, форма, макет, справка...), и root, version,
                        versions; остальные строки остаются как в
                        конфигурации базы. Объект каждого файла должен быть
                        в базе, а файл его описания (Catalogs/X.xml для
                        Catalogs/X/Ext/ObjectModule.bsl) - в каталоге.
                        Удаление файлов и объектов, новые объекты и
                        Configuration.xml не загружаются: код возврата 1

                        --base-dir=<path>
                            Каталог, относительно которого указаны файлы.
                            Файл вне его не загружается

                        --partial
                            Каталог содержит только часть файлов
                            конфигурации. Без него каталог должен быть полной
                            выгрузкой (с Configuration.xml)

                        --no-check
                            Не проверять перед записью, что выгрузка
                            загруженных строк совпадает с перечисленными
                            файлами (по умолчанию проверяется)

                        <file> ...
                            файлы конфигурации для загрузки, относительно
                            --base-dir

            apply
                Обновление конфигурации базы данных: переносит конфигурацию,
                сохраненную командой import (таблица ConfigSave), в действующую
                (таблица Config) в монопольном режиме, одной транзакцией.
                Реструктуризацию реквизитов справочников и документов
                (добавление, удаление, увеличение длины строки, индексирование
                реквизита) и создание нового справочника или документа (без
                форм, макетов, команд и предопределенных элементов) выполняет
                само, в той же транзакции, если указан
                --recovery-backup или --i-have-a-backup; без них ничего не
                меняет и отвечает про резервную копию, код возврата 1. Другую
                реструктуризацию базы (или изменение, которое ibcmd-rs не
                выполняет) не выполняет, ничего не меняет и отвечает
                \"требуется штатный config apply: <причины>\", код возврата 1.
                Если к базе подключены другие сеансы, отказывает словами ibcmd
                об исключительной блокировке, код возврата -1

                --force | -F
                    Подтверждение выполнения операции в случае наличия
                    предупреждений. Принимается: предупреждений, которые нужно
                    подтверждать, это применение не выдает

                --dynamic=<auto|disable|prompt|force>
                    Использование динамического обновления. Применение всегда
                    монопольное: auto (по умолчанию), disable и prompt
                    выполняются как disable, force не поддерживается

                --session-terminate=<disable|prompt|force>
                    Завершение активных сеансов. disable (по умолчанию);
                    prompt и force принимаются, пока к базе никто не подключен,
                    ibcmd-rs сеансы не завершает

                --session-terminate-message=<message>
                    Принимается и не используется

            save
                Выгрузка конфигурации в файл .cf: строки конфигурации
                записываются в файл как есть, без XML. Без --db - основная
                конфигурация (загруженная командой import и еще не
                примененная, если она есть, иначе конфигурация базы данных),
                с --db - конфигурация базы данных (таблица Config).
                Существующий файл заменяется, когда новый записан и проверен

                --db
                    Выгрузить конфигурацию базы данных

                <path>
                    путь к файлу конфигурации

Параметры ibcmd-rs (у ibcmd их нет):

    --report=<file>
        Записать отчет о выполнении команды в файл JSON

    --platform=<version>
        Версия платформы базы: 8.3.27 или 8.5.1 (или сборка 8.3.27.2214,
        8.5.1.1150). Задает формат XML: 2.20 для 8.3, 2.21 для 8.5. Без него
        берется из настроек (запись базы в ibcmd-rs.toml, IBCMD_RS_PLATFORM),
        при импорте - из дерева. Прежний --source-version=<2.20|2.21> тоже
        принимается

    --sqlcmd=<path>
        Работать через sqlcmd и bcp, как версия 0.2. По умолчанию ibcmd-rs
        подключается к SQL Server сам

    --settings=<file>
        Файл настроек JSON (ключи vrunner и ibcmd-rs)

    --db-pwd-env=<name>
        Переменная окружения с паролем пользователя СУБД (по умолчанию IBCMD_DB_PSW)

    --base-free
        (import) Собрать каждую строку из дерева, не читая конфигурацию базы

    --exclusivity=<sql|assumed>
        (apply) Как убедиться, что с базой никто не работает. sql (по
        умолчанию): по сеансам, которые видит SQL Server (нужно право VIEW
        SERVER STATE). assumed: сеансы не проверяются; берите его, когда у
        логина СУБД нет этого права или соединение держит лишь пул рабочего
        процесса, а вы знаете, что пользователей в базе нет

    --recovery-backup=<file>
        (apply) Согласие на реструктуризацию, которую применение выполняет
        само: перед транзакцией снять BACKUP DATABASE ... WITH COPY_ONLY в
        этот файл (путь, куда может писать служба SQL Server; файл не должен
        существовать) и назвать его в отчете. Без этого параметра или
        --i-have-a-backup такая реструктуризация не выполняется: отказ, код
        возврата 1. Изменения, которые применение не выполняет само,
        по-прежнему отправляются на штатный config apply

    --i-have-a-backup
        (apply) Подтвердить, что резервная копия базы у вас есть; это
        записывается в отчете

    --verify | --no-verify
        (import) Перед записью в ConfigSave выгрузить моделью конфигурацию,
        какой она станет после загрузки, и сравнить каждый файл с деревом;
        при расхождении загрузка отменяется (код -1), ConfigSave остается как
        был. Проверяется любая загрузка (и в базу с конфигурацией, и --base-free,
        и в пустую базу), --verify это подтверждает; --no-verify пропускает
        проверку

Не поддерживаются в этой версии ibcmd-rs (планируются в следующих):

{unsupported}
        параметры export --file, --archive; --base и --sync с --extension
        параметры import --out, --extension
        параметры apply --extension, --dynamic=force, --sqlcmd
        параметр save --extension
        общие параметры --pid, --remote
        режимы {modes}

Коды возврата: 0 - успех, 2 - ошибка в командной строке, -1 - ошибка
выполнения (как у ibcmd), 1 - команда или параметр не поддерживаются в этой
версии ibcmd-rs.

Справочник по всем командам, настройкам и версиям платформы:
https://github.com/Untru/ibcmd-rs/blob/master/docs/COMMANDS.md
",
        summary = INFOBASE.summary,
    )
}

/// `ibcmd help`: the modes, which one is served.
pub fn overview(program: &str) -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut out = format!(
        "ibcmd-rs {version}: выгрузка и загрузка конфигурации информационной базы Microsoft SQL Server \
без платформы 1С:Предприятия, с командной строкой ibcmd

Использование:

\t{program} [mode] [command] [options] [arguments]

Поддерживаемые режимы:

{infobase:<23}{summary}: config export, config import, config import files, config apply, config save

Не поддерживаются в этой версии ibcmd-rs (планируются в следующих):

",
        infobase = "infobase",
        summary = INFOBASE.summary,
    );
    for (mode, summary) in OTHER_MODES {
        out.push_str(&format!("{mode:<23}{summary}\n"));
    }
    out.push_str(&format!(
        "\nСправка по режиму: {program} help infobase\n\
         Исследовательские команды ibcmd-rs (справка на английском): {program} --help\n"
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_help_names_every_refused_command_and_no_platform_path() {
        let help = infobase_help("ibcmd");
        for command in [
            "ibcmd infobase create",
            "ibcmd infobase config check",
            "ibcmd infobase config export info",
            "ibcmd infobase config import all-extensions",
            "ibcmd infobase config support",
            "ibcmd infobase config extension",
        ] {
            assert!(help.contains(command), "{command}");
        }
        // import files is served (#363): described, not listed as refused
        assert!(!help.contains("ibcmd infobase config import files"));
        for word in ["--base-dir=<path>", "--partial", "--no-check", "<file> ..."] {
            assert!(help.contains(word), "{word}");
        }
        // children of a refused command are covered by it
        assert!(!help.contains("config support disable"));
        // apply is served: described with its words, not listed as refused
        assert!(!help.contains("ibcmd infobase config apply"));
        // so is save; load is not
        assert!(!help.contains("ibcmd infobase config save"));
        assert!(help.contains("ibcmd infobase config load"));
        assert!(help.contains("путь к файлу конфигурации"));
        for word in [
            "--dynamic=<auto|disable|prompt|force>",
            "--session-terminate=<disable|prompt|force>",
            "--session-terminate-message=<message>",
            "требуется штатный config apply: <причины>",
            "--exclusivity=<sql|assumed>",
            "--recovery-backup=<file>",
            "--i-have-a-backup",
        ] {
            assert!(help.contains(word), "{word}");
        }
        assert!(help.contains("--db-server"));
        assert!(help.contains("--request-db-pwd"));
        // the release audit forbids the platform's executable names
        assert!(!help.to_ascii_lowercase().contains(".exe"));
        let overview = overview("ibcmd");
        assert!(overview.contains("binary-data-storage"));
        assert!(overview.contains("ibcmd help infobase"));
    }
}
