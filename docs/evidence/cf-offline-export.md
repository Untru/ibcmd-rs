# CF-012: автономный CF → XML export

Дата фиксации: 2026-07-23.

## Результат

Команда `cf export` читает CF напрямую, строит нейтральный `StorageImage` и
передаёт его существующим Rust-декодерам семейств исходного XML. Для запуска
не нужны установленная платформа 1С, `ibcmd`, Designer, EDT, Java/JVM,
`sqlcmd` или любой другой subprocess.

```powershell
cargo run -- cf export configuration.cf source --source-version 2.20
```

Формат payload задаётся явно через `--compression raw-deflate|stored`, а XML
dialect — независимо через `--source-version 2.20|2.21`. Никакая версия не
выводится из имени файла, содержимого payload или версии контейнера.

## Граница провайдеров

`crates/ibcmd-cf/src/export.rs` не зависит от XML, SQL или файловой системы.
Он группирует физические multipart entries по logical key, сохраняет порядок
первого появления и отдаёт точные packed bytes. Односоставная запись не
копируется; multipart собирается один раз с проверкой переполнения размера.

Корневой `mssql_dump` остаётся legacy-адаптером семейных декодеров. Новый
`export_storage_image_to_source` принимает только `StorageImage`, output path,
overwrite policy и XML dialect. SQL-получение строк не входит в этот API.
Обычный MSSQL export продолжает завершаться на первой ошибке; режим накопления
per-entry failures включён только для нейтрального storage export.

## Отчёт и fail-closed поведение

JSON-отчёт имеет schema version 1 и для каждой logical entry содержит:

- logical name/key, количество частей и packed byte count;
- `supported` и список созданных source paths, если семейный декодер сработал;
- `opaque`, если ни один доказанный декодер не распознал запись;
- `failed` и диагностическое сообщение, если распознанная запись не смогла
  экспортироваться.

Opaque не считается успешным декодированием и не превращается в синтетический
XML. Наличие хотя бы одного `failed` делает CLI-команду неуспешной и приводит к
ненулевому exit code; top-level ошибки открытия, CF decode и подготовки output
также возвращаются как стабильные JSON diagnostics на stderr.

## Переносимые проверки

- `cargo test --locked -p ibcmd-cf export::tests`
  - multipart grouping и source order;
  - агрегирование disposition counters.
- `cargo test --locked --test cf_export`
  - checked-in Format15 clean-room CF экспортирует доказанный `Language`, а
    минимальная структурная Configuration-запись честно остаётся opaque;
  - отдельная полная clean-room Configuration-запись проходит тем же семейным
    декодером и создаёт `Configuration.xml`;
  - оба CLI-процесса работают с пустым `PATH`.
- `cargo test --locked commands::cf::tests`
  - прежние `inspect`/`verify` контракты не изменены.
- `cargo test --locked --test cf_cli`
  - прежние CLI success/error JSON envelopes не изменены.
- `cargo test --locked writes_exchange_plan_content_without_metadata_xml_indexes`
  - прежний eager MSSQL source-layout путь по умолчанию остаётся рабочим и
    использует исходный fail-fast режим.
- `cargo clippy --locked --workspace --exclude ibcmd-rs --all-targets -- -D warnings`
  - переносимые crates проходят строгий lint gate.

Полный root `clippy -D warnings` по-прежнему блокируется ранее существовавшим
legacy lint debt вне CF-012; это не скрывается как успех новой реализации.
