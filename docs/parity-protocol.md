# Протокол побайтовой совместимости нативной выгрузки

Этот документ задаёт единственный воспроизводимый способ измерять совместимость
`ibcmd-rs` с нативным `ibcmd`. Он выполняет только чтение исходной MSSQL-базы:
скрипты не вызывают команды записи, загрузки, stage/import или изменения схемы.

## Предусловия

- Нужен релизный бинарник с платформенным оракулом:

  ```powershell
  cargo build --release --features platform-oracle
  ```

- Для SQL-аутентификации `IBCMD_DB_PSW` задаётся в окружении текущего процесса.
  Пароль и имя переменной не попадают в журналы или `parity-manifest.json`.
- Для Windows-аутентификации используйте `-IntegratedAuth` (либо пустой
  `-DbUser`). В этом режиме скрипт не передаёт SQL-реквизиты и на время
  нативной выгрузки изолирует унаследованные `IBCMD_DB_USR`/`IBCMD_DB_PSW`.
- Доступны нативный `ibcmd`, `sqlcmd`/`bcp` и тестовая база. Используйте только
  одноразовые тестовые копии.

Перед запуском `export-ibcmd-vs-ours.ps1` проверяет наличие команд
`dump-sources`, `mssql-dump-config`, `source-diff`, `source-diff-signatures`,
`source-diff-matrix` и `source-diff-matrix-merge`.
Если сборка не содержит `dump-sources`, она прекращается с подсказкой собрать
бинарник с `--features platform-oracle`.

## Один прогон

```powershell
$env:IBCMD_DB_PSW = "<пароль>"
powershell -ExecutionPolicy Bypass -File scripts\export-ibcmd-vs-ours.ps1 `
  -DbName ut_ibcmd -DbServer localhost -DbUser sa `
  -RunId 20260723_ut_full -LabRoot E:\ibcmd_lab\parity
```

Вариант с интегрированной аутентификацией:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\export-ibcmd-vs-ours.ps1 `
  -DbName ut_ibcmd -DbServer localhost -IntegratedAuth `
  -RunId 20260723_ut_full -LabRoot E:\ibcmd_lab\parity
```

Каталог `E:\ibcmd_lab\parity\ut_ibcmd_20260723_ut_full` создаётся ровно один
раз; повторное использование идентификатора — ошибка. Внутри всегда находятся:

- `native/` — выгрузка нативного `ibcmd`;
- `candidate_dump/` — служебная выгрузка строк MSSQL;
- `candidate/` — только реконструированное дерево исходников;
- `raw-diff.json`, `signatures.json`, `matrix.json` и `matrix.md`;
- `logs/` и `parity-manifest.json` со статусами, временем и кодами завершения.

Манифест создаётся до первого внешнего процесса и атомарно обновляется до и
после каждого шага. В нём сохраняются SHA и clean/dirty-состояние Git, SHA-256
бинарников, версии `ibcmd-rs`, `ibcmd`, `sqlcmd`, `bcp` и `robocopy`, версия
XML, точные обезличенные аргументы, журналы и выходные артефакты. Отдельно
фиксируются точные отпечатки таблиц `Config`/`ConfigSave` до и после выгрузки,
а также детерминированные SHA-256 нативного и кандидатного деревьев.

Полный прогон не может завершиться со статусом `passed`, если репозиторий
грязный, отпечаток БД отсутствует или изменился во время работы. Значения
паролей и имя парольной переменной не сериализуются.

## Offline three-way oracle (native, EDT, ibcmd-rs)

`source-three-way-oracle` is a research diagnostic, not a release gate. It
only reads three already-created source trees and writes a new JSON and Markdown
report outside those trees. It never starts EDT, Java/JVM, `ibcmd`, or a database
client, so the default workflow has no EDT runtime dependency.

Each input requires an explicit exact version string: the common source version,
native `ibcmd`, the EDT import/export route, and `ibcmd-rs`. The report preserves
per-path raw SHA-256/size values plus a deterministic tree SHA-256 using
`path + NUL + file SHA-256 + NUL + size + LF`. It has bounded file-count, total
byte, and per-file byte limits; existing reports are never overwritten.

```powershell
powershell -ExecutionPolicy Bypass -File scripts\run-three-way-source-oracle.ps1 `
  -NativeRoot E:\oracle\native -EdtRoot E:\oracle\edt -OursRoot E:\oracle\ours `
  -SourceVersion '8.3.27 / XML 2.20' `
  -NativeToolVersion 'ibcmd 8.3.27.1989' `
  -EdtToolVersion 'EDT 2025.2.3+30 import/export' `
  -OursToolVersion 'ibcmd-rs 0.1.0 @ <commit>' `
  -Output E:\oracle\report.json -Markdown E:\oracle\report.md
```

At every path, exactly one deterministic branch is reported: all equal;
native=EDT≠ours; native=ours≠EDT; EDT=ours≠native; or all different. The latter
four branches are candidate hypotheses only. In particular, matching hashes do
not by themselves prove a decoder/model/schema/writer, EDT, native/storage, or
version cause. Do not commit production trees, application XML/BSL, credentials,
or reports containing application paths/content; committed tests use synthetic
hash-only evidence.

## Матрица УТ + БСП

```powershell
powershell -ExecutionPolicy Bypass -File scripts\run-parity-matrix.ps1 `
  -UtDbName ut_ibcmd -BspDbName bsp -RunId 20260723_full
```

Оркестратор запускает два независимых неизменяемых прогона, затем командой
`source-diff-matrix-merge` объединяет полные матрицы в
`matrix_<RunId>\parity-matrix.json` и `parity-matrix.md`. Имена баз и сервер
передаются явно; скрипт не ищет и не выбирает рабочие базы автоматически.
До объединения он машинно проверяет одинаковые Git SHA, версии XML и фактически
разрешённые версии нативного `ibcmd`. Дочерние прогоны и merge журналируются как
отдельные шаги верхнего манифеста.

`RunId` должен начинаться с буквы или цифры, состоять не более чем из 128
символов `[A-Za-z0-9._-]` и не содержать `..` либо разделителей пути. Проверка
выполняется до создания любого каталога.

## Полный и ограниченный режимы

`-Scope full` (по умолчанию) означает сравнение всего дерева, запрещает
`-PathPrefix` и только он годится для заявления о полной совместимости.
Во время доведения совместимости такой прогон сохраняет инвентарь пропущенных
корневых XML и продолжает строить матрицу. Для выпускной проверки добавьте
`-RequireCompleteRootMetadata`: тогда отсутствие хотя бы одного ожидаемого
корневого XML прервёт прогон. Этот переключатель несовместим с `-Scope scoped`.
`-Scope scoped` требует хотя бы один `-PathPrefix` и нужен для исследования
одного семейства файлов; его результат всегда диагностический и не изменяет
общий процент готовности. Оркестратор поддерживает объединение scoped-матриц,
но помечает результат `result_class=diagnostic` и
`release_eligible=false`.

Старые `01-export-ibcmd.bat`, `02-export-ibcmd-rs.bat` и
`03-diff-ibcmd-vs-ibcmd-rs.bat` намеренно отключены: они позволяли записывать в
известный каталог и тем самым смешивать baseline и candidate.

## Трасса линий MXL

Для исследования итоговой палитры линий в уже сохранённом прогоне используйте
только его неизменяемый `candidate_dump`:

```powershell
cargo run -- mxl-line-provenance-corpus --run-root <run-root> --output <trace.jsonl>
```

Для одного сохранённого сырого asset (обычный режим для UKD) укажите его
относительно `candidate_dump`, не передавая абсолютный путь:

```powershell
cargo run -- mxl-line-provenance-corpus --run-root <run-root> --asset <asset-relative-to-candidate_dump> --output <trace.jsonl>
```

Команда прогоняет каждый сырой asset через обычный compatible-MXL extractor и
записывает ровно одно JSONL-событие на каждую финальную line slot. Событие не
содержит runtime-имён, UUID или путей: в нём есть только raw entry/line spans,
упорядоченная цепочка преобразований, format index/border slot и финальные
style/type/width с флагами `ambiguous` и `fail_closed`. Нераспознанные assets
пропускаются; команда не изменяет дерево прогона.
