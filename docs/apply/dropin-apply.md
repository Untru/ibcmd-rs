# `ibcmd infobase config apply` in the drop-in mode (milestone 0.4, issue #343)

`ibcmd infobase config apply [--force] [--dynamic=...] [--session-terminate=...]`, in the
platform's syntax, served by the own exclusive apply (`ibcmd-rs mssql-config-apply`, #337).
This page records what the platform's command does (measured on 8.3.27.2214, every case in
[`evidence/dropin-apply/native-cases.md`](evidence/dropin-apply/native-cases.md)), which of it
this version serves and how, and the words and exit codes it answers with.

**Stricter than the platform, on purpose.** Run against a database (`--dbms=...`, the way the
drop-in and every script of the lab run), the platform does not see other sessions and applies
whatever is connected; in server mode (`--pid`, `--remote`) it cancels with a warning and **exit
0**. This apply refuses while SQL Server shows another user process on the database, with exit
-1, so that a script never takes "not applied" for success (`--exclusivity=assumed` switches the
look off). It also refuses, with exit 1, every stage that would restructure the database **unless** the
restructuring is in the S1 set and the operator names a way back ([below](#the-way-back-before-a-restructuring)).

Contents: [the platform's command](#the-platforms-command) -
[what is served](#what-is-served) - [what the drop-in prints](#what-the-drop-in-prints) -
[differences from the platform](#differences-from-the-platform) -
[the seam](#the-seam) - [tests and lab evidence](#tests-and-lab-evidence) -
[open points](#open-points).

## The platform's command

### Options (from `ibcmd help infobase`, `config` > `apply`)

| Option | Meaning | Default |
|---|---|---|
| `--extension=<name>`, `-e` | the configuration extension to update | the main configuration |
| `--force`, `-F` | "confirm the operation when there are warnings" | off |
| `--dynamic=<auto\|disable\|prompt\|force>` | use of the dynamic (online) update: `auto` decides, `disable` forbids it, `prompt` asks the user, `force` uses the dynamic update only | `auto` |
| `--session-terminate=<disable\|prompt\|force>` | end the active sessions when the exclusive lock of the infobase is needed: forbidden, ask, end them | `disable` |
| `--session-terminate-message=<message>` | the text a terminated session shows | none |
| `--user`, `-u`, `--password`, `-P` (of `config`) | the infobase user | none |

What `--force` confirms: the warnings of an apply that restructures data, such as
`[WARN] Код справочника стал неуникальным: _ДемоКонтрагенты (0)` or `[WARN] Номер документа стал
неуникальным в заданном периоде ЭлектронноеПисьмоИсходящее (00000000001)` (printed and gone past
in the lab's structural applies with `--force`, `restructure-check/runs/probe_auto_a`). A clean
module apply prints the same with and without it (n10 against n30). What the platform does
*without* `--force` when such a warning comes was not measured; the own apply never restructures,
so it has nothing to confirm.

The words are case-sensitive; a wrong or missing word is `Некорректное значение параметра: dynamic`
(or `session-terminate`) on stderr and exit 2 (cases p01-p03, p08, p10, p11). A flag given a value
(`--force=yes`) is `Ошибка разбора параметра: force` (exit 2, the name without dashes; the same for
`export --sync=1`), an unknown option `Ошибка разбора параметра: --bogus`. A stray argument is
ignored. `ibcmd infobase config apply --help` prints the top-level help, not a help of the command.

### Two ways to run it

* **Direct** (`--dbms=MSSQLServer --db-server ... --db-name ...`, the way every script of the lab
  and the drop-in's export and import run): the platform opens the database itself. It does
  **not look for other sessions at all**: with a 1C cluster session connected to the database
  (a thin client of a cluster infobase, `rac session list` shows it) the apply runs as if nobody
  were there (n20). Only a running stand-alone server on the same `--data` stops it
  (`Ошибка блокировки каталога данных сервера.` / `Рабочий каталог заблокирован процессом: <pid>`,
  exit -2, n21).
* **Server** (`--pid=<pid>` or `--remote=<url>` of a stand-alone server, `ibsrv`): the command
  runs inside the server, which knows its sessions. Its lines start `[INFO ]` (a space before the
  bracket). The server keeps its own picture of the infobase: rows written to `ConfigSave` behind
  its back are not seen (n22: "не требуется" with six rows staged).

### What it printed (direct mode, БСП 8.3.27 clone)

| Case | stdout | stderr | Exit | Time |
|---|---|---|---|---|
| Nothing staged (n01, n02) | `[INFO] Обновление конфигурации базы данных...` / `[INFO] Проверка корректности метаданных...` / `[INFO] Обновление конфигурации базы данных не требуется` | empty | 0 | 5-38 s (the check) |
| A module changed, any of `auto` (default), `disable`, `prompt` (n10, n11, n31) | `[INFO] Обновление конфигурации базы данных...` / `Проверка корректности метаданных...` / `Принятие изменений...` / `Обработка данных` / `Обработка данных Регистрация изменений в планах обмена` / `Создано поколение конфигурации: <32 hex><00000000>` / `Обработка данных Регистрация изменений в планах обмена` / `[INFO] Обновление конфигурации базы данных успешно завершено` (the last two lines swap places from run to run) | empty | 0 | 8-34 s |
| The same with `--dynamic=force` (n12) | the same lines | empty | 0 | 6 s |
| `--session-terminate=force` or `prompt`, a message (n30, n31) | as a plain apply: nobody to end | empty | 0 | |
| A staged descriptor breaks the metadata check (n46) | the first two INFO lines | `[ERROR] ОбщийМодуль._ДемоЛокализация: Дублирование имени объекта метаданных: ` / `[ERROR] Операция невозможна: при выполнении проверки корректности метаданных обнаружены ошибки` | **1** | 5 s |
| A staged descriptor is not readable (n45) | the first two INFO lines | `Ошибка формата потока` | -1 | 10 s |
| The database does not exist (n42) | empty | `База данных отсутствует в сервере баз данных` / `Не найдена база данных '<db>' в SQL-сервере 'localhost'` | -1 | 1 s |
| The SQL server cannot be reached (n43) | empty | `Соединение с сервером баз данных разорвано администратором` / the OLE DB text | -1 | 17 s |
| Unknown infobase user (n40) | `Для выполнения операции требуется аутентификация в информационной базе` / `Пароль для '<user>': ` | `Идентификация пользователя не выполнена` | -1 | 5 s |
| No `--user` on a database that has users, stdin closed (n41) | the same first line, then `Имя пользователя: ` forever (12 MB a minute) | empty | never ends | |
| A command-line error (p01-p12) | empty | see above | 2 | 0.3 s |

What the apply changes in the tables: the staged rows replace the `Config` rows in place,
`ConfigSave` is emptied, `versions` names a new generation. `auto` leaves exactly the state
`disable` leaves (n10 against n11: no `_dynupdate_` row, no `DynamicallyUpdated` marker);
`force` writes the dynamic overlay (n12: four alias rows, the markers in `Config` and `Params`).
`Создано поколение конфигурации:` prints the head of the new `versions` row: the 16 bytes of the
GUID as stored (little-endian fields) in hex, then `00000000` (`40cfe0ac-6f3b-4851-85ba-295e568663f6`
is printed `ace0cf403b6f514885ba295e568663f600000000`).

### Sessions connected (server mode: the only mode that sees them)

* Default options, stdin closed (n23): the platform prints
  `Ошибка исключительной блокировки информационной базы.` / `Активные сеансы и соединения:` / one
  line per session (`компьютер: <host>, сеанс начат: <time>, приложение: <application>`) and a
  three-way prompt (1 Отмена, 2 Повторить, 3 Обновить динамически, default `[2]`). With no input
  the prompt takes its default and asks again, without end: 95 372 rounds in eight minutes.
* Answer `1` (n24): `[WARN ] Обновление конфигурации базы данных отменено` on stderr, **exit 0**.
* `--dynamic=disable`, stdin closed (n26): two INFO lines, the same WARN, exit 0, in two seconds.
  This is the non-interactive refusal because sessions are connected. Nothing was applied.
* `--dynamic=disable --session-terminate=force` (n27): the session is ended and the stage applied.

## What is served

The drop-in serves the exclusive apply and, for `--dynamic=force`, the dynamic apply of a small stage
([`dropin-dynamic.md`](dropin-dynamic.md)). The default of the platform, `--dynamic=auto`, is
served because it *is* the exclusive apply whenever the exclusive lock can be taken (n10 against
n11), and the platform run against a database always takes it, since it sees no session.

| Platform | ibcmd-rs |
|---|---|
| default options | exclusive apply |
| `--dynamic=auto`, `disable`, `prompt` | exclusive apply (the platform, run against a database, never asks and never updates dynamically when nobody blocks the lock); with sessions connected: the refusal below, never a dynamic update the user did not ask for. `auto` and `prompt` add the line `можно применить динамически: --dynamic=force` when the stage would qualify |
| `--dynamic=force` | the dynamic apply (`dropin-dynamic.md`): a small stage of common-module and common-form bodies is published as a generation beside the active rows while sessions stay connected; any other stage `требуется штатный config apply: <reasons>` (exit 1); a platform without `mssql.config.apply.dynamic` (8.5.1.1150, 8.3.27.1989) `Параметр `--dynamic=force` ... не поддерживается для платформы ...` (exit 1) |
| `--force`, `-F` | accepted; the own apply raises no warning that needs confirming |
| `--session-terminate=disable` (default) | with other sessions connected: the refusal below; else the apply |
| `--session-terminate=force`, `prompt` | with nobody connected: accepted, nothing to end. With sessions connected: `Параметр `--session-terminate=force` команды ... не поддерживается в этой версии ibcmd-rs`, the list of sessions, exit 1 (this version ends no session) |
| `--session-terminate-message` | accepted, unused |
| `--extension`, `-e` | not supported (exit 1), as for export and import |
| `--pid`, `--remote` (server mode) | not supported (exit 1), as for export and import |
| `--user`, `--password` | accepted, not needed |
| a stray argument | ignored, as the platform ignores it |

Own options (the platform has none of them): `--report=<file>` (the JSON report, also for a
refusal), `--platform=<version>`, `--settings`, `--db-pwd-env`, `--exclusivity=<sql|assumed>`
(`sql`, the default: look at the sessions SQL Server shows, needs `VIEW SERVER STATE`; `assumed`:
the operator answers for it), `--recovery-backup=<file>` and `--i-have-a-backup` (below). `--sqlcmd` is
refused: the apply runs on the built-in client.

### The way back before a restructuring

The structural gate of the drop-in is always the restructuring track's S1 gate (#391, `AllowRestructure::S1`).
The platform has no option to choose it, and the backup option is the operator's consent. What the gate does with a
stage:

| the stage | the drop-in | exit |
|---|---|---|
| changes no table: bodies, harmless properties of descriptors (a synonym), whatever the restructure check passes | applied as it always was; no backup option needed; the report's `gate` is `apply-check` | 0 |
| a restructuring of the S1 set (add, delete an attribute, widen a variable string, switch the index of an attribute; catalogs and documents) **with** `--recovery-backup=<file>` or `--i-have-a-backup` | done by the own restructuring inside the apply's transaction; the report's `gate` is `s1` and `structure` lists the objects and tables | 0 |
| the same **without** a backup option | `BackupRequired`: the Russian words naming both options; nothing written | 1 |
| any other restructuring, or an S1 operation that is designed and not built yet (add a tabular section, add an object) | `требуется штатный config apply: <the reasons>`, the S1 reasons among them; nothing written | 1 |

A restructuring drops the old tables of the objects inside the transaction, and the apply's recovery artifact keeps
the `Config` rows and the caches, not the tables (`own-apply.md`, "Backup policy"), so the way back is a SQL Server
backup: `--recovery-backup=<file>` (the apply takes `BACKUP DATABASE ... WITH COPY_ONLY, COMPRESSION` to that file,
a path the SQL Server service can write and that does not exist yet, before the transaction, and names it in the
report) or `--i-have-a-backup` (the operator has one; recorded in the report). Both: the file. The platform has
neither and needs no backup, so this is stricter than it.

The S1 gate sits **behind** the restructure check (`gate::FirstThen`): the check judges the stage first, and only
what it refuses is handed to the S1 gate. Alone, the S1 gate wraps the conservative rule, which refuses every
descriptor whose text differs, harmless or not: a stage that changed a synonym was refused with "S1: the conservative
gate refuses descriptors, the restructuring check names no change in them" where the default gate applies it. The same rule
would have held a synonym changed in *another* object against an S1 change: the S1 gate withdraws the conservative "descriptor's
text differs" blocker of a row that the check has read and no reason of it names (as it does for the planned objects, `root`, the
created rows and the listing), so an attribute change with a synonym change elsewhere is applied
(`evidence/dropin-apply/s1-mixed-stage.md`). The apply now also updates each changed synonym in the registry
row `1a621f0f`, for both the plain and S1 gates; the former stale-cache gap is covered by
[`evidence/dropin-apply/synonyms.md`](evidence/dropin-apply/synonyms.md). When both gates refuse, the reasons are the first gate's, then the S1 gate's own (`S1: ...`); the
conservative lines are kept only where they are what refuses the stage.

The platform of the database is `--platform`, else the settings, else 8.3.27 (a release stands
for the build the apply was measured on, `8.3.27.2214`). The storage layout is verified against
the database by the apply. 8.5 is served like 8.3.27 (the apply is measured on both, `own-apply.md`; the S1 restructurings on both
БСП, `evidence/dropin-apply/s1-acceptance.md` and `s1-acceptance-85.md`); the drop-in has no refusal of its own for it.

### Refusals

* **A stage that needs a restructuring**, or anything else the own apply does not do (a stage of
  the platform's own `config import`, removals, a dynamic overlay in `Params`, an interrupted
  operation, a new object of a kind the apply does not create): nothing is written, exit 1,
  stderr `[ERROR] требуется штатный config apply: <row or object>: <reason>; ...` (eight reasons
  at most, then `и ещё N`). The reasons are the gate's; today the apply's conservative gate,
  and `apply_check::Verdict::refusal()` (#338) once `ApplyCheckGate` is its default.
* **Other sessions connected** (SQL Server shows another user process on the database): exit -1,
  `Ошибка исключительной блокировки информационной базы.` / `Активные сеансы и соединения:` / one
  line per session as SQL Server knows it (`компьютер: <host>, приложение: <program>, соединение
  с СУБД: <id> (<login>, <status>)`; the 1C session's own start time is not known here) and the
  advice to close them, then the closing `[ERROR]` line. **This is stricter than the platform**:
  run against a database it applies with sessions connected (n20), and its server mode cancels
  with a warning and exit 0 (n26). Here it is a failure (exit -1): a script must never take "not
  applied" for success. `--exclusivity=assumed` (this program's option, default `sql`) does not
  look at the sessions: for a login without `VIEW SERVER STATE`, or when only a working
  process's pooled connections remain and the operator knows nobody is working.

## What the drop-in prints

Success:

```
[INFO] Обновление конфигурации базы данных...
[INFO] Создано поколение конфигурации: <hex>
[INFO] Обновление конфигурации базы данных успешно завершено
```

Nothing staged (exit 0):

```
[INFO] Обновление конфигурации базы данных...
[INFO] Обновление конфигурации базы данных не требуется
```

Failure (exit -1): every line of the message as `[ERROR] ...`, then
`[ERROR] Обновление конфигурации базы данных завершено с ошибкой`. Refusal of a stage (exit 1):
`[ERROR] требуется штатный config apply: ...`, nothing else. A command line refused before
anything runs: one bare line on stderr, stdout empty (exit 2 for what the platform refuses too,
exit 1 for what this version does not serve).

| Exit | When |
|---|---|
| 0 | applied; nothing to apply |
| 2 | a command-line error, the platform's words |
| 1 | `требуется штатный config apply` (or `config repair`); a restructuring without `--recovery-backup` or `--i-have-a-backup`; an option this version does not serve; sessions connected together with `--session-terminate=force\|prompt` |
| -1 | a failed operation: no database, no connection, no password, other sessions connected, the apply's own failure (255 in a POSIX shell) |

`--report=<file>` receives `{"operation": "infobase config apply", "ok": ..., "nothing_to_apply": ...,
"apply": <the apply's report>}` on success and `{"operation", "ok": false, "error"}` otherwise.
The recovery artifact of the apply (the bytes it overwrote) goes to its default folder,
`%TEMP%\ibcmd-rs\config-apply-recovery\<db>-<token>`; the path is in the report.

## Differences from the platform

1. **No metadata check.** The platform checks the whole configuration first
   (`Проверка корректности метаданных...`, 5-38 s, exit 1 on errors). The own apply does not;
   the stage it takes is that of `infobase config import`, whose input was checked when it was
   staged, and its gate refuses whatever changes the structure. The drop-in prints no line for
   the steps it does not do.
2. **Sessions are looked for.** The platform run against a database is blind to them; this apply
   is not (see above). It is stricter than the platform on purpose: it moves rows in one
   transaction, but a working process holds the configuration in memory.
3. **Only stages of this repository's importers**; the stage of the platform's own `config import`
   (records in another format, a `{68}` Configuration row, a `deleted` list this apply cannot account for name by name:
   the rows of a removed form or template it can, docs/apply/own-apply.md "Removals") is exit 1.
4. **LF line ends** (the platform writes CRLF), like the drop-in's export and import.
5. **Exit 1** is the platform's code for a failed metadata check and this version's code for what
   it does not serve. A script that tells them apart reads stderr.
6. The `Params` `.ui` rows, the help index, the extension CAS garbage collection and other
   derived state the platform rewrites on every apply are not written (see
   `own-apply.md`, "What it does not write").

## The seam

`src/dropin/apply.rs` has one call into the apply, `call_apply`, which is
`crate::mssql_config_apply::apply_staged_configuration(sql, &options)`. Everything above it is the
platform's command line (`src/dropin/parse.rs`: options, words, exit 2) and the mapping of the
options (`apply_options`, `profile_of`); everything below it is the mapping of what comes back
(`classify`, `structural_text`, `sessions_text`, `native_generation`). The gate is the apply's
default: its conservative gate today, `ApplyCheckGate` (a `StructuralGate` around
`apply_check::check_staged`) after track apply's swap, with no change here.

`classify` sorts an error by the type the apply gives it (`mssql_config_apply::errors`, through any context
around it) and reads no message:

| error of the apply | the drop-in says | exit |
|---|---|---|
| `StructuralRefusal` | `требуется штатный config apply: <the gate's reasons>` | 1 |
| `NeedsNativeApply` (`Apply`) | `требуется штатный config apply: <reason>` | 1 |
| `NeedsNativeApply` (`Repair`, an interrupted operation) | `требуется штатный config repair: <reason>` | 1 |
| `BackupRequired` | its words, naming `--recovery-backup` and `--i-have-a-backup` | 1 |
| `ExclusiveAccessRefused` | the platform's lock words and its own list of sessions (the error carries them) | -1 (1 with `--session-terminate=prompt\|force`) |
| `ExclusiveAccessUnprovable` | the reason and `укажите --exclusivity=assumed` | -1 |
| any other | the error and its context | -1 |

An error of no type is a failure, whatever its words say (a test holds the old marker phrases to it): a new
refusal of the apply has to be a type to be told from a failure. `config repair` in the second row is new:
the words used to say `config apply` for an operation that was never finished.

### Where the drop-in stays stricter than the platform

* **Sessions are looked for** (above), and a connected session is a failure, not a warning.
* **A restructuring is refused** (`требуется штатный config apply`) unless it is in the S1 set, and then it needs a
  backup option (`BackupRequired`, exit 1); the platform carries every restructuring out and needs no backup.
* **`--dynamic=force`, `--extension`, `--sqlcmd`, `--pid`, `--remote`** are refused by name (exit 1).
* **Stages of the platform's own `config import`** are exit 1 unless the check proves them (`restructuring-check.md`
  3.6): the record-format noise of a whole native image is proven; a `deleted` list of removed attributes is judged
  by the S1 gate, the rows of a removed form or template are deleted by the apply itself, any other list is not read.
* **Stages of this program's own import** do not carry a restructuring today (evidence:
  [`s1-acceptance.md`](evidence/dropin-apply/s1-acceptance.md)): the patch mode transfers no attribute, and the
  base-free stage of a changed descriptor is refused by S1 for the rows it rewrites (the business process flowchart, the
  `root` row) and cannot compile a form that binds a deleted attribute. A restructuring is staged by the platform's
  `import files --partial` (or a tree the platform imports) until that changes.
* It is **less** strict in one place: no metadata check (difference 1 below).

## Tests and lab evidence

* Unit tests (`src/dropin/parse.rs`, `apply.rs`, `mod.rs`, `help.rs`; `cargo test --lib dropin`,
  39 tests): the platform's line of the lab scripts, the defaults, every word, the errors as
  measured, what is not served, the mapping of options and platform (the backup options too), the sorting of
  the apply's typed errors and the messages (sessions, structural reasons, native apply and repair, backup), the
  generation as the platform prints it, the report file.
* `tests/dropin_cli.rs` (10 tests): the process, without a database: every native command served
  or refused by name, the words and refusals before anything runs (exit codes and streams), the
  accepted spellings, the failure shape and the `--report`.
* Lab, [`evidence/dropin-apply/lab-runs.md`](evidence/dropin-apply/lab-runs.md): the drop-in on
  БСП 8.3.27 clones with the platform's command line. Nothing staged (0.9 s, "не требуется");
  a module change applied (exit 0), and the platform's own apply right after finds nothing
  to update; a descriptor change refused by the conservative gate (exit 1, nothing written);
  a connected cluster session refused with the list (exit -1), `--session-terminate=force`
  refused by name (exit 1), `--exclusivity=assumed` applied; a stage written by the platform's
  own import refused read-only (exit 1).
* The platform's help of the command: [`evidence/dropin-apply/native-help-apply.txt`](evidence/dropin-apply/native-help-apply.txt).
* The whole user-facing flow, [`evidence/dropin-apply/e2e-0.4.md`](evidence/dropin-apply/e2e-0.4.md): our exe as
  `ibcmd.exe` without `sqlcmd`/`bcp` on `PATH`; `config import` of a tree with 25 edited files (modules of 17
  kinds of object, forms, templates, a help page), `config apply`, the platform's export, `source-diff`: 12 197
  files identical and `ConfigDumpInfo.xml` identical with `configVersion` blanked, twice; the times against the
  platform's own import and apply (`scripts/dropin-e2e/run_e2e.ps1`).

## Open points

0. **Staging a restructuring with the program's own import** (import track): see the difference above. The
   drop-in apply is measured on the platform's stage (`evidence/dropin-apply/s1-acceptance.md`).

1. **Exit code of "sessions connected"**: -1 here; the platform's server mode exits 0 after
   cancelling. Its direct mode has no such case.
2. **Typed refusals in `mssql_config_apply`**: done (rcheck-6): `NeedsNativeApply`, `ExclusiveAccessRefused`,
   `ExclusiveAccessUnprovable` and `BackupRequired` next to `StructuralRefusal`; `classify` reads types only.
3. **`VIEW SERVER STATE`**: without the right the apply cannot see sessions and refuses;
   `--exclusivity=assumed` is the way out. A database login of an ordinary installation is not
   a sysadmin.
4. **Sessions of the 1C cluster** hold SQL connections that a pooled working process keeps
   after the user left; the refusal then names the working process (`1CV8`), and stopping it
   is the operator's.
5. **8.5**: served (rcheck-6). Evidence on an 8.5 БСП clone (`--platform=8.5.1`): nothing staged, `не требуется`,
   exit 0; a stage made by the drop-in's own import (a module comment, 9 622 rows in patch mode) reaches the apply and
   is refused by the check for the known reason of the 8.5 stage (`restructuring-check.md`, finding 10: the 99 MB
   body row of the configuration differs), exit 1, in the check's words and not in a refusal of 8.5. A neutral 8.5
   stage that our importer can make and the apply can take is the import track's. **S1 on 8.5 is proven** on the
   platform's own stages `b1`, `c1`, `d1` of the БСП 8.5.1.1150 (checks 3, 4, 5, 7, 8 equal to the native twin, the `*.si`
   rows the same up to order): `evidence/dropin-apply/s1-acceptance-85.md`. What it needed was the `root` row: the 8.5
   platform re-stamps the final block of its payload on every write, and the check reads the row for that. **S1 on the ERP
   УХ 8.3.27** is proven on two cases (`b1` deletes and adds attributes, `c1` widens strings; checks 2, 3, 4, 5, 7, 8 equal to the
   native twin, the 16 `*.si` rows identical): `evidence/dropin-apply/s1-acceptance-uha.md`. The УХ backup holds the configuration
   only, so the rebuilt tables are empty: it proves the structure path at УХ's metadata scale, not the copy of data. One
   difference from the platform: it appends the rebuilt tables at the end of the `DBSchema` list, ours are placed before
   `ConfigChngR`, which is the end only on the БСП. ERP УХ 8.5 is not measured.
6. **Dynamic update (`--dynamic=force`)** and **ending sessions** are the natural next steps
   (0.5, "смена поколения").
