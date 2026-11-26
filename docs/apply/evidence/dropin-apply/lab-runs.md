# The drop-in `infobase config apply` in the lab

The runs of `ibcmd-rs infobase config apply` (debug build of the commit that adds the command) with the command line of the
platform's own apply (`--dbms=MSSQLServer --db-server=localhost --db-name=<db> --data=<dir> --user=Администратор` and the options
shown), on БСП 8.3.27 clones of the rcheck lab; one line of the platform's own apply follows in o03. `<db>` is the clone.
Raw streams are in `F:\ibcmd\lab\04\dropin-apply\native\<case>`. Lines end with LF here (the platform's CRLF).

## o01_noop

Nothing staged. No metadata check, so no wait: 0.9 s against the platform's 5-38 s.

`infobase config apply <connection> --user=Администратор --force --dynamic=disable`

`exit=0 seconds=0.9 started=2026-09-29T18:36:02 stdout_bytes=177 stderr_bytes=0`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Обновление конфигурации базы данных не требуется
```
stderr:
```

```
storage, staged -> after: no change in the rows counted

## o02_success

A module change staged (a 6-row delta stage of `mssql-stage-source-objects`), default options. The staged rows replace the `Config` rows in place, `ConfigSave` is emptied, `versions` names the new generation.

`infobase config apply <connection> --user=Администратор`

`exit=0 seconds=29.7 started=2026-09-29T18:36:46 stdout_bytes=295 stderr_bytes=0`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Создано поколение конфигурации: 00463caebd242b4dbb4403a53a596bae00000000
[INFO] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0

## o03_native_after_ours

The platform's own apply on the same database, right after o02. Its metadata check (17.8 s) finds nothing to update: what the drop-in applied is a configuration the platform accepts as applied.

`ibcmd infobase config apply <connection> --user=Администратор --force --dynamic=disable`

`exit=0 seconds=17.8 started=2026-09-29T18:37:55 stdout_bytes=253 stderr_bytes=0`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Проверка корректности метаданных...
[INFO] Обновление конфигурации базы данных не требуется
```
stderr:
```

```
storage, staged -> after: no change in the rows counted

## o04_descriptor_refused

A staged descriptor differs (the synonym of the module). Refused by the apply's conservative gate, nothing written. The restructuring check calls this change harmless (see below), so it passes once `ApplyCheckGate` is the apply's gate.

`infobase config apply <connection> --user=Администратор`

`exit=1 seconds=4 started=2026-09-29T18:38:47 stdout_bytes=78 stderr_bytes=184`

stdout:
```
[INFO] Обновление конфигурации базы данных...
```
stderr:
```
[ERROR] требуется штатный config apply: ab132638-5188-470d-9432-de85f2b2c7d8: the descriptor's text differs from the active one: a metadata change, possibly structural
```
storage, staged -> after: no change in the rows counted

`ibcmd-rs mssql-apply-check` on the same stage:
```
Реструктуризация не нужна.
Файлов в конфигурации: 9838 -> 9838; строк в ConfigSave: 6; добавлено 0, удалено 0; описаний сравнено 1, тел с данными сравнено 0.
Объектов с отличиями: 1 (из них с причинами: 0).
Тела без влияния на структуру: Module 1.
Безопасные изменения (1):
  CommonModule._ДемоЗаметки: Properties/Synonym/item/content: Демо: Заметки -> Демо: Заметки (изм.)
```

## o05_sessions_refused

A thin client of the 1C cluster is connected; default options. SQL Server shows the working process's connections. Nothing written.

`infobase config apply <connection> --user=Администратор`

`exit=-1 seconds=3 started=2026-09-29T18:40:37 stdout_bytes=78 stderr_bytes=866`

stdout:
```
[INFO] Обновление конфигурации базы данных...
```
stderr:
```
[ERROR] Ошибка исключительной блокировки информационной базы.
[ERROR] Активные сеансы и соединения:
[ERROR] компьютер: DESKTOP-SMI5N4O, приложение: 1CV83 Server, соединение с СУБД: 66 (sa, sleeping);
[ERROR] компьютер: DESKTOP-SMI5N4O, приложение: 1CV83 Server, соединение с СУБД: 77 (sa, running);
[ERROR] компьютер: DESKTOP-SMI5N4O, приложение: 1CV83 Server, соединение с СУБД: 255 (sa, sleeping)
[ERROR] Закройте их (если рабочий процесс лишь держит соединение из пула, остановите его) и повторите
[ERROR] Обновление конфигурации базы данных завершено с ошибкой
```
storage, staged -> after: no change in the rows counted

## o06_terminate_force_refused

The same with `--force --dynamic=disable --session-terminate=force`. This version ends no session: a refusal by name, the sessions listed.

`infobase config apply <connection> --user=Администратор --force --dynamic=disable --session-terminate=force`

`exit=1 seconds=2.4 started=2026-09-29T18:41:07 stdout_bytes=78 stderr_bytes=1089`

stdout:
```
[INFO] Обновление конфигурации базы данных...
```
stderr:
```
[ERROR] Параметр `--session-terminate=force` команды `infobase config apply` не поддерживается в этой версии ibcmd-rs (планируется в следующих): к базе подключены сеансы, а ibcmd-rs не завершает сеансы, закройте их сами
[ERROR] Ошибка исключительной блокировки информационной базы.
[ERROR] Активные сеансы и соединения:
[ERROR] компьютер: DESKTOP-SMI5N4O, приложение: 1CV83 Server, соединение с СУБД: 66 (sa, sleeping);
[ERROR] компьютер: DESKTOP-SMI5N4O, приложение: 1CV83 Server, соединение с СУБД: 77 (sa, sleeping);
[ERROR] компьютер: DESKTOP-SMI5N4O, приложение: 1CV83 Server, соединение с СУБД: 255 (sa, running)
[ERROR] Закройте их (если рабочий процесс лишь держит соединение из пула, остановите его) и повторите
```
storage, staged -> after: no change in the rows counted

## o07_assumed_applies

The same with `--exclusivity=assumed`. The operator answers for exclusivity: applied with the session connected.

`infobase config apply <connection> --user=Администратор --exclusivity=assumed`

`exit=0 seconds=4.1 started=2026-09-29T18:41:19 stdout_bytes=295 stderr_bytes=0`

stdout:
```
[INFO] Обновление конфигурации базы данных...
[INFO] Создано поколение конфигурации: 2c2616055b42134fbbc7e17552ecc29800000000
[INFO] Обновление конфигурации базы данных успешно завершено
```
stderr:
```

```
storage, staged -> after: configsave_rows 6 -> 0

## o08_native_stage_refused

A stage written by the platform's own `config import` (9 842 rows, a `deleted` row). Refused at once and read-only.

`infobase config apply <connection> --user=Администратор --force --dynamic=disable`

`exit=1 seconds=15 started=2026-09-29T18:42:25 stdout_bytes=78 stderr_bytes=450`

stdout:
```
[INFO] Обновление конфигурации базы данных...
```
stderr:
```
[ERROR] требуется штатный config apply: the stage carries a `deleted` row, the list of removals: it is empty (the platform's own import writes one to every stage). This apply takes the stage of this repository's `infobase config import`, which writes no such row yet, and removes nothing that a staged row does not replace; the stage of the platform's own `config import` and any removal need the native `ibcmd infobase config apply`
```
storage, staged -> after: no change in the rows counted

