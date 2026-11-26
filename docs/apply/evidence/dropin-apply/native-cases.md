# Native `ibcmd infobase config apply` (8.3.27.2214): what it printed

Every case of the dropin-apply lab (`F:\ibcmd\lab\04\dropin-apply\native\<case>`), one native command each, run under the lab's
`native` lock. Streams are kept as raw bytes there; the platform ends its lines with CRLF and writes UTF-8 without a byte order
mark; the texts below have LF. `<db>` is a БСП 8.3.27 clone. "Direct mode" is the platform run against a database
(`--dbms=MSSQLServer --db-server ... --db-name ...`), "server mode" is `--pid=<ibsrv>`.

## n01_noop_default

Nothing staged, default options. ConfigSave is empty. The metadata check still runs (38 s on the first run, then cached).

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор`

`exit=0 seconds=38.5 started=2026-09-29T17:04:47`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Обновление конфигурации базы данных не требуется
```
stderr:
```

```

## n02_noop_force_disable

Nothing staged, `--force --dynamic=disable`. Same words, same exit code.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --force --dynamic=disable`

`exit=0 seconds=5.1 started=2026-09-29T17:05:49`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Обновление конфигурации базы данных не требуется
```
stderr:
```

```

## n10_default_success

Module change staged, default options (`--dynamic=auto`). No session anywhere. Rows are promoted in place: no `_dynupdate_` rows, no `DynamicallyUpdated` marker.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор`

`exit=0 seconds=8.3 started=2026-09-29T17:18:24`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Принятие изменений...
[INFO] Обработка данных
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Создано поколение конфигурации: a3d987b5e2d06e47b11e2b29ebd4704000000000
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0

## n11_disable

Module change staged, `--dynamic=disable`. The same words and the same end state as the default.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --dynamic=disable`

`exit=0 seconds=7.5 started=2026-09-29T17:18:57`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Принятие изменений...
[INFO] Обработка данных
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Создано поколение конфигурации: 69fced0420b2934dbd0a198fab76f20800000000
[INFO] Обновление конфигурации базы данных успешно завершено
[INFO] Обработка данных Регистрация изменений в планах обмена
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0

## n12_force

Module change staged, `--dynamic=force`. Dynamic update: four alias rows and the `DynamicallyUpdated` markers appear in `Config` and `Params`.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --dynamic=force`

`exit=0 seconds=6 started=2026-09-29T17:19:25`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Принятие изменений...
[INFO] Обработка данных
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Создано поколение конфигурации: de278920eeded44db9dc7e6af4c6097c00000000
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: config_rows 9838 -> 9843; config_alias_rows 0 -> 4; configsave_rows 6 -> 0; config_markers  -> DynamicallyUpdated; params_dynamically_updated 0 -> 1

## n20_sessions_default

Module change staged, a 1C cluster session is connected, default options. The platform run against the database (`--dbms=...`) does not see the cluster: it applies as if nobody were there.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор`

`exit=0 seconds=8.1 started=2026-09-29T17:21:00`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Принятие изменений...
[INFO] Обработка данных
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Создано поколение конфигурации: cfffc970d79c18428aaa67bc769d6bd700000000
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0

## n21_server_nosession

A stand-alone server holds the data directory. The platform run against a database cannot share `--data` with a running stand-alone server.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор`

`exit=-2 seconds=0.3 started=2026-09-29T17:23:34`

stdout:
```

```
stderr:
```
Ошибка блокировки каталога данных сервера.
Рабочий каталог заблокирован процессом: 94520
```
storage, staged -> after: no change in the rows counted

## n22_server_pid_nosession

Through `--pid` of a stand-alone server started BEFORE the stage was written. The server does not see rows written behind its back: "nothing to apply" (and `[INFO ]` with a space, the server mode).

`ibcmd infobase config apply --user=Администратор --pid=94520`

`exit=0 seconds=1.4 started=2026-09-29T17:24:02`

stdout:
```
[INFO ] Обновление конфигурации базы данных...
[INFO ] Проверка корректности метаданных...
[INFO ] Обновление конфигурации базы данных не требуется
```
stderr:
```

```
storage, staged -> after: no change in the rows counted

## n23_server_session_default

Through `--pid`, a session is connected, default options, stdin closed. The prompt answers itself with its default, `[2]` (retry), forever: 95 372 rounds in 8 minutes, 78 MB. Killed by the harness.

`ibcmd infobase config apply --user=Администратор --pid=90928`

`exit=-1 seconds=491.8 started=2026-09-29T17:33:29`

stdout:
```
[INFO ] Обновление конфигурации базы данных...
[INFO ] Проверка корректности метаданных...
Ошибка исключительной блокировки информационной базы.
Активные сеансы и соединения:
компьютер: DESKTOP-SMI5N4O, сеанс начат: 29.09.2026 в 17:24:44, приложение: Тонкий клиент

Выберите "Отмена" для прекращения обновления.
Выберите "Повторить" для повторения попытки установки монопольного режима.
Выберите "Обновить динамически" для обновления без завершения работы пользователей.
1 Отмена
2 Повторить
3 Обновить динамически
Пожалуйста, выберите один из вариантов [2] : Ошибка исключительной блокировки информационной базы.
Активные сеансы и соединения:
компьютер: DESKTOP-SMI5N4O, сеанс начат: 29.09.2026 в 17:24:44, приложение: Тонкий клиент

Выберите "Отмена" для прекращения обновления.
Выберите "Повторить" для повторения попытки установки монопольного режима.
Выберите "Обновить динамически" для обновления без завершения работы пользователей.
1 Отмена
2 Повторить
3 Обновить динамически
Пожалуйста, выберите один из вариантов [2] : Ошибка исключительной блокировки информационной базы.
Активные сеансы и соединения:
компьютер: DESKTOP-SMI5N4O, сеанс начат: 29.09.2026 в 17:24:44, приложение: Тонкий клиент

Выберите "Отмена" для прекращения обновления.
Выберите "Повторить" для повторения попытки установки монопольного режима.
Выберите "Обновить динамически" для обновления без завершения работы пользо
... (cut; 1717 characters in all)
```
stderr:
```

```
storage, staged -> after: no change in the rows counted

## n24_server_session_cancel

Through `--pid`, a session is connected, answer `1` (cancel). The prompt in full, and the cancel: a warning, exit 0.

`ibcmd infobase config apply --user=Администратор --pid=92696`

`exit=0 seconds=19.7 started=2026-09-29T17:48:22 stdout_bytes=1115 stderr_bytes=93`

stdout:
```
[INFO ] Обновление конфигурации базы данных...
[INFO ] Проверка корректности метаданных...
Ошибка исключительной блокировки информационной базы.
Активные сеансы и соединения:
компьютер: DESKTOP-SMI5N4O, сеанс начат: 29.09.2026 в 17:48:32, приложение: Фоновое задание;
компьютер: DESKTOP-SMI5N4O, сеанс начат: 29.09.2026 в 17:47:29, приложение: Тонкий клиент

Выберите "Отмена" для прекращения обновления.
Выберите "Повторить" для повторения попытки установки монопольного режима.
Выберите "Обновить динамически" для обновления без завершения работы пользователей.
1 Отмена
2 Повторить
3 Обновить динамически
Пожалуйста, выберите один из вариантов [2] : 
```
stderr:
```
[WARN ] Обновление конфигурации базы данных отменено
```
storage, staged -> after: sessions_other 25 -> 24

## n25_server_session_disable

Through `--pid`, a session is connected, `--dynamic=disable`, answers `1`. No prompt is printed: the operation is cancelled at once.

`ibcmd infobase config apply --user=Администратор --pid=92696 --dynamic=disable`

`exit=0 seconds=2.4 started=2026-09-29T17:49:08 stdout_bytes=153 stderr_bytes=93`

stdout:
```
[INFO ] Обновление конфигурации базы данных...
[INFO ] Проверка корректности метаданных...
```
stderr:
```
[WARN ] Обновление конфигурации базы данных отменено
```
storage, staged -> after: no change in the rows counted

## n26_server_session_disable_eof

Through `--pid`, a session is connected, `--dynamic=disable`, stdin closed. The non-interactive refusal because sessions are connected: two INFO lines, a WARN on stderr, exit 0.

`ibcmd infobase config apply --user=Администратор --pid=92696 --dynamic=disable`

`exit=0 seconds=2 started=2026-09-29T17:49:29 stdout_bytes=153 stderr_bytes=93`

stdout:
```
[INFO ] Обновление конфигурации базы данных...
[INFO ] Проверка корректности метаданных...
```
stderr:
```
[WARN ] Обновление конфигурации базы данных отменено
```
storage, staged -> after: no change in the rows counted

## n27_server_terminate_force

Through `--pid`, a session is connected, `--dynamic=disable --session-terminate=force`. The session is ended (SQL sessions of the database 25 -> 2) and the stage is applied.

`ibcmd infobase config apply --user=Администратор --pid=92696 --dynamic=disable --session-terminate=force`

`exit=0 seconds=2.6 started=2026-09-29T17:50:09 stdout_bytes=683 stderr_bytes=0`

stdout:
```
[INFO ] Обновление конфигурации базы данных...
[INFO ] Проверка корректности метаданных...
[INFO ] Принятие изменений...
[INFO ] Обработка данных
[INFO ] Обработка данных: Регистрация изменений в планах обмена
[INFO ] Создано поколение конфигурации: 8497c704434a86458368d4618e9d9f3600000000
[INFO ] Обработка данных: Регистрация изменений в планах обмена
[INFO ] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0; sessions_other 25 -> 2

## n30_direct_terminate_force

Direct mode, `--force --dynamic=disable --session-terminate=force --session-terminate-message=lab`. Accepted; nobody to end; a plain apply.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --force --dynamic=disable --session-terminate=force --session-terminate-message=lab`

`exit=0 seconds=25.4 started=2026-09-29T17:51:21 stdout_bytes=681 stderr_bytes=0`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Принятие изменений...
[INFO] Обработка данных
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Создано поколение конфигурации: b18d3305f4ec2045ab473fa42b3db63700000000
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0

## n31_direct_prompts

Direct mode, `--dynamic=prompt --session-terminate=prompt`, stdin closed. Nothing to ask: a plain apply.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --dynamic=prompt --session-terminate=prompt`

`exit=0 seconds=34.3 started=2026-09-29T17:52:26 stdout_bytes=681 stderr_bytes=0`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Принятие изменений...
[INFO] Обработка данных
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Создано поколение конфигурации: 16112bfdc4737e46b5c1b5bde7c35f7c00000000
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0

## n40_wrong_user

An unknown infobase user. Asks for the password, stdin is closed.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор`

`exit=-1 seconds=5.4 started=2026-09-29T17:53:21 stdout_bytes=169 stderr_bytes=77`

stdout:
```
Для выполнения операции требуется аутентификация в информационной базе
Пароль для 'NoSuchUser': 
```
stderr:
```
Идентификация пользователя не выполнена
```
storage, staged -> after: no change in the rows counted

## n41_no_user

No `--user` on a database that has users. The user-name prompt repeats forever (cut here); killed after a minute.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --force`

`exit=TIMEOUT seconds=60.4 started=2026-09-29T17:53:38 stdout_bytes=11968145 stderr_bytes=0`

stdout:
```
Для выполнения операции требуется аутентификация в информационной базе
Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользователя: Имя пользовател
... (cut; 109089 characters in all)
```
stderr:
```

```
storage, staged -> after: no change in the rows counted

## n42_no_such_database

The database does not exist.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=ibcmd_rs_04_rcheck_no_such_db --user=Администратор --force`

`exit=-1 seconds=0.8 started=2026-09-29T17:55:00 stdout_bytes=0 stderr_bytes=193`

stdout:
```

```
stderr:
```
База данных отсутствует в сервере баз данных
Не найдена база данных 'ibcmd_rs_04_rcheck_no_such_db' в SQL-сервере 'localhost'
```

## n43_bad_server

The SQL server cannot be reached.

`ibcmd infobase config apply --user=Администратор --dbms=MSSQLServer --db-server=no-such-host-xyz --db-name=x --force`

`exit=-1 seconds=16.9 started=2026-09-29T17:55:29 stdout_bytes=0 stderr_bytes=349`

stdout:
```

```
stderr:
```
Соединение с сервером баз данных разорвано администратором
Microsoft OLE DB Driver 19 for SQL Server: Named Pipes Provider: Could not open a connection to SQL Server [53]. 
HRESULT=80004005, HRESULT=80004005, HRESULT=80004005, SQLSrvr: SQLSTATE=08001, state=1, Severity=10, native=53, line=0
```

## n44_corrupt_stage

A staged module body row is garbage. The apply does not read module text: it succeeds.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --force`

`exit=0 seconds=13.8 started=2026-09-29T17:56:09 stdout_bytes=681 stderr_bytes=0`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Принятие изменений...
[INFO] Обработка данных
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Создано поколение конфигурации: ace0cf403b6f514885ba295e568663f600000000
[INFO] Обработка данных Регистрация изменений в планах обмена
[INFO] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0

## n45_corrupt_descriptor

A staged descriptor row is garbage. A failure while the metadata are read.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --force`

`exit=-1 seconds=9.8 started=2026-09-29T17:56:49 stdout_bytes=153 stderr_bytes=42`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
```
stderr:
```
Ошибка формата потока
```
storage, staged -> after: no change in the rows counted

## n46_duplicate_name

A staged descriptor takes another common module's name. A metadata check error: exit 1 (the only exit 1 the platform gives).

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --force`

`exit=1 seconds=5.1 started=2026-09-29T17:58:02 stdout_bytes=153 stderr_bytes=312`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
```
stderr:
```
[ERROR] ОбщийМодуль._ДемоЛокализация: Дублирование имени объекта метаданных: 
[ERROR] Операция невозможна: при выполнении проверки корректности метаданных обнаружены ошибки
```
storage, staged -> after: no change in the rows counted

## p01_dynamic_bogus

Command line: `--dynamic=bogus`.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --dynamic=bogus`

`exit=2 seconds=0.3 started=2026-09-29T17:58:43 stdout_bytes=0 stderr_bytes=71`

stdout:
```

```
stderr:
```
Некорректное значение параметра: dynamic
```

## p02_terminate_bogus

Command line: `--session-terminate=bogus`.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --session-terminate=bogus`

`exit=2 seconds=0.2 started=2026-09-29T17:58:46 stdout_bytes=0 stderr_bytes=81`

stdout:
```

```
stderr:
```
Некорректное значение параметра: session-terminate
```

## p03_dynamic_missing

Command line: `--dynamic` with no value (last).

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --dynamic`

`exit=2 seconds=0.2 started=2026-09-29T17:58:49 stdout_bytes=0 stderr_bytes=71`

stdout:
```

```
stderr:
```
Некорректное значение параметра: dynamic
```

## p04_force_value

Command line: `--force=yes`.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --force=yes`

`exit=2 seconds=0.2 started=2026-09-29T17:58:52 stdout_bytes=0 stderr_bytes=55`

stdout:
```

```
stderr:
```
Ошибка разбора параметра: force
```

## p06_bogus_option

Command line: `--bogus`.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --bogus`

`exit=2 seconds=0.1 started=2026-09-29T17:59:05 stdout_bytes=0 stderr_bytes=57`

stdout:
```

```
stderr:
```
Ошибка разбора параметра: --bogus
```

## p08_F_dynamic_bogus

Command line: `-F --dynamic=bogus`.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор -F --dynamic=bogus`

`exit=2 seconds=0.3 started=2026-09-29T18:00:05 stdout_bytes=0 stderr_bytes=71`

stdout:
```

```
stderr:
```
Некорректное значение параметра: dynamic
```

## p10_dynamic_empty

Command line: `--dynamic=`.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --dynamic=`

`exit=2 seconds=0.6 started=2026-09-29T18:00:27 stdout_bytes=0 stderr_bytes=71`

stdout:
```

```
stderr:
```
Некорректное значение параметра: dynamic
```

## p11_dynamic_upper

Command line: `--dynamic=AUTO`. The words are case-sensitive.

`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --user=Администратор --dynamic=AUTO`

`exit=2 seconds=0.3 started=2026-09-29T18:00:34 stdout_bytes=0 stderr_bytes=71`

stdout:
```

```
stderr:
```
Некорректное значение параметра: dynamic
```

