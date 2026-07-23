# Протокол побайтовой совместимости нативной выгрузки

Этот документ задаёт единственный воспроизводимый способ измерять совместимость
`ibcmd-rs` с нативным `ibcmd`. Он выполняет только чтение исходной MSSQL-базы:
скрипты не вызывают команды записи, загрузки, stage/import или изменения схемы.

## Предусловия

- Нужен релизный бинарник с платформенным оракулом:

  ```powershell
  cargo build --release --features platform-oracle
  ```

- `IBCMD_DB_PSW` задаётся в окружении текущего процесса. Пароль не попадает в
  аргументы, журналы или `parity-manifest.json`.
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

Каталог `E:\ibcmd_lab\parity\ut_ibcmd_20260723_ut_full` создаётся ровно один
раз; повторное использование идентификатора — ошибка. Внутри всегда находятся:

- `native/` — выгрузка нативного `ibcmd`;
- `candidate_dump/` — служебная выгрузка строк MSSQL;
- `candidate/` — только реконструированное дерево исходников;
- `raw-diff.json`, `signatures.json`, `matrix.json` и `matrix.md`;
- `logs/` и `parity-manifest.json` со статусами, временем и кодами завершения.

В манифесте сохраняются SHA Git, версия формата, режим, обезличенный источник
пароля и точные имена CLI-команд. Значения паролей и переменная окружения целиком
не сериализуются.

## Матрица УТ + БСП

```powershell
powershell -ExecutionPolicy Bypass -File scripts\run-parity-matrix.ps1 `
  -UtDbName ut_ibcmd -BspDbName bsp -RunId 20260723_full
```

Оркестратор запускает два независимых неизменяемых прогона, затем командой
`source-diff-matrix-merge` объединяет полные матрицы в
`matrix_<RunId>\parity-matrix.json` и `parity-matrix.md`. Имена баз и сервер
передаются явно; скрипт не ищет и не выбирает рабочие базы автоматически.

`RunId` должен начинаться с буквы или цифры, состоять не более чем из 128
символов `[A-Za-z0-9._-]` и не содержать `..` либо разделителей пути. Проверка
выполняется до создания любого каталога.

## Полный и ограниченный режимы

`-Scope full` (по умолчанию) означает сравнение всего дерева, запрещает
`-PathPrefix` и только он годится для заявления о полной совместимости.
В этом режиме кандидат запускается со строгой проверкой
`--require-complete-root-metadata`: отсутствие хотя бы одного ожидаемого
корневого XML прерывает прогон до построения матрицы.
`-Scope scoped` требует хотя бы один `-PathPrefix` и нужен для исследования
одного семейства файлов; его результат всегда диагностический и не изменяет
общий процент готовности.

Старые `01-export-ibcmd.bat`, `02-export-ibcmd-rs.bat` и
`03-diff-ibcmd-vs-ibcmd-rs.bat` намеренно отключены: они позволяли записывать в
известный каталог и тем самым смешивать baseline и candidate.
