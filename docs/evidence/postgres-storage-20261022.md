# Конфигурация в PostgreSQL: формат хранения (#367) и выгрузка (#369)

Дата: 22.10.2026. Стенд: Windows 11, PostgreSQL 17.2-4.1C (сборка для 1С),
1С:Предприятие 8.5.1.1529 (`ibcmd`). Базы созданы штатным ibcmd:

```
ibcmd infobase create --dbms=PostgreSQL --db-server=127.0.0.1 --db-name=<база> \
  --db-user=postgres --create-database --import=<XML> --apply --force
```

## Таблицы

Конфигурация лежит в тех же таблицах, что и на SQL Server, только имена
таблиц и колонок в нижнем регистре (`information_schema.columns` базы
`tiny`):

| Таблица | Колонки |
|---|---|
| `config`, `configsave`, `configcas`, `configcassave`, `params` | `filename` mvarchar, `creation` timestamp, `modified` timestamp, `attributes` int4, `datasize` int8, `binarydata` bytea, `partno` int4 |
| `dbschema` | `serializeddata` bytea |
| `ibversion` | `ibversion` int4, `platformversionreq` int4 |

- Первичный ключ `config` и `configsave`: уникальный индекс `(filename, partno)`.
- `filename` имеет тип `mvarchar` сборки PostgreSQL для 1С, поэтому
  читается как `filename::text`.
- `binarydata` хранит те же raw-deflate строки, что и `BinaryData` на SQL
  Server. `partno` нумерует части строки, как на SQL Server и в файловой базе.
- После `--import ... --apply` таблица `configsave` пуста, в `config` у `tiny`
  7 строк: `root`, `version`, `versions`, корень конфигурации, язык, общий
  модуль и его текст.
- Законченная стадия загрузки в `configsave` устроена так же, как в файловой
  базе и на SQL Server (строка `versions` без маркеров незаконченной
  загрузки), и публикуется поверх `config` той же логикой
  (`mssql_dump::file_infobase::publish`).

## Выгрузка

`ibcmd-rs infobase config export --dbms=PostgreSQL --db-server=<host> --db-name=<база> --db-user=<user> [--db-pwd=...] <папка>`.

- Свой клиент протокола PostgreSQL 3.0 (`src/sql/postgres.rs`), без
  внешних библиотек и без TLS.
- Вход: без пароля (`trust`), cleartext, MD5, SCRAM-SHA-256. Пароль
  необязателен: его спрашивает сервер.
- Строки `config` (и законченной стадии `configsave`) читаются один раз
  (`encode(binarydata, 'hex')`, от `bytea_output` не зависит). Дальше работает
  общий конвейер выгрузки, как для `--rows-dir` и файловых баз.
- Сервер в записи 1С: `host`, `host:port` или `host port=N`; порт по
  умолчанию 5432, пользователь по умолчанию `postgres`.

## Замеры

| База | Файлов | Совпало с выгрузкой штатного ibcmd | Вход |
|---|---|---|---|
| `tiny` (`tests/fixtures/file-infobase/tiny-source`) | 4 | 4 из 4 | trust, MD5, SCRAM-SHA-256 (временная роль, удалена после замера) |
| БСП 3.1.11 (`ssl`, XML 8.3.27, `--platform=8.5.1`) | 9 156 | 8 710 из 9 156 | trust |

446 различий БСП не связаны с PostgreSQL. Та же БСП, загруженная тем же
ibcmd 8.5.1.1529 в файловую базу, даёт при выгрузке ibcmd-rs те же 446
различий. Наши выгрузки из PG и из файловой базы совпадают между собой,
нативные тоже, кроме `ConfigDumpInfo.xml`: у каждой базы свои версии. Это
пробел выгрузки 8.5 для конфигурации в режиме совместимости 8.3.27:
`ButtonImportance`, картинки в настройках СКД. Время выгрузки БСП: штатный
ibcmd 17 с, ibcmd-rs 40 с; строки читаются одним соединением в
hex-представлении.

Нативное `infobase create ... --apply` БСП на этом стенде завершилось с rc=-1
(`Ошибка SDBL: Использование быстрой вставки недопустимо без удаления
индексов`) на реструктуризации данных. Конфигурация при этом записана в
`config` (7 212 строк, `configsave` пуст), и штатная выгрузка её читает.

Неверный пароль дал отказ `FATAL 28P01: password authentication failed`,
выгрузка не началась.

## Чего нет

Выгрузки расширений, `import files`, `config save`, применения и TLS.
Команды, кроме выгрузки и загрузки, отвечают на `--dbms=PostgreSQL` отказом
«не поддерживается».
