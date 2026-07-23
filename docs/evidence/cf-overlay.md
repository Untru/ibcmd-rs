# CF-014: автономный XML/source overlay поверх base CF

Дата фиксации: 2026-07-23.

## Результат

Команда `cf overlay` принимает base CF, новый output path и повторяемые явно
типизированные пары `STORAGE_KEY=FILE`. Поддержаны BSL-модули, raw assets,
обычный metadata XML, CommonModule XML и CommandInterface XML. Команда читает,
компилирует и записывает архив только кодом проекта; она не ищет и не запускает
1С, `ibcmd`, Designer, EDT, Java/JVM, SQL-клиент или другой subprocess.

```powershell
cargo run -- cf overlay base.cf result.cf `
  --module 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa.0=Ext/Module.bsl' `
  --raw-asset 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb.0=Ext/Picture.bin' `
  --source-version 2.20
```

CF revision, XML dialect, storage profile и payload compression задаются или
читаются как независимые оси. Storage key всегда указан вызывающей стороной и
не выводится из пути, имени объекта или dotted suffix.

## Граница слоёв

`crates/ibcmd-cf/src/overlay.rs` зависит только от `ibcmd-core` и CF/V8 слоя.
Он принимает нейтральный `StoragePatch` и base `StorageImage`. Legacy XML
packers не перенесены в CF crate и подключаются корневым адаптером через
`OverlayCodec`:

- `Compiled` содержит готовый packed payload и заменяет точный key/part;
- `NeedsBase` получает именно объявленную base entry и вызывает подходящий
  base-capable Rust codec;
- `Unsupported` попадает в полный preflight blocker set и запрещает операцию
  до любого codec call или создания временного файла;
- `update_versions` получает исходную/уже наложенную запись `versions` и
  отсортированный уникальный список изменённых logical keys.

Такое направление зависимостей оставляет core/CF слой независимым от XML,
SQL, CLI и legacy реализации и позволяет позже заменить семейный codec без
изменения физического overlay assembler.

## Lossless и atomic свойства

Overlay сначала проверяет существование точного target key/part, multipart
count, наличие и однозначность required base entry, наличие одиночной
`versions` entry и все `Unsupported` outcomes. Затем он меняет только payloads
заявленных targets и `versions`.

Для каждой нетронутой записи byte-identical сохраняются:

- исходный порядок;
- logical name/key и multipart identity;
- raw element header и address attributes;
- packed/unpacked payload;
- compression, storage profile и provenance.

Новый output публикуется только как новый файл. Он записывается во временный
файл в том же каталоге, flush/sync выполняются до повторного открытия, после
чего проверяются layout и каждая запись. Race-safe публикация не перезаписывает
существующий destination; ошибка удаляет временный файл.

## Machine-readable report

JSON schema version 1 сообщает base/output paths, source dialect/profile,
число requested и preserved entries, факт обновления `versions`, digests patch
и output image, а также для каждой замены:

- logical key и part index;
- источник `compiled`, `needs_base` или `versions`;
- SHA-256 packed payload до и после.

CLI-ошибки используют общий CF JSON envelope на stderr и ненулевой exit code.

## Переносимые проверки

- `cargo test --locked -p ibcmd-cf overlay::tests`
  - compiled и NeedsBase entries заменяются на исходных позициях;
  - required base bytes действительно передаются adapter codec;
  - unknown entry остаётся точной, `versions` обновляется;
  - `Unsupported` блокирует до первого codec call.
- `cargo test --locked --test cf_overlay`
  - бинарная CLI-команда с пустым `PATH` накладывает BSL module, raw asset и
    base-dependent CommandInterface XML;
  - только три targets и `versions` меняются, unknown packed bytes и порядок
    остаются точными;
  - output повторно декодируется, module/raw/XML semantics проверяются;
  - unsupported preflight не создаёт destination или temporary file.
- `cargo test --locked --test cf_roundtrip`
  - общий atomic writer и reopen validator сохраняют прежний контракт.
- `cargo clippy --locked --workspace --exclude ibcmd-rs --all-targets -- -D warnings`
  - переносимые crates проходят строгий lint gate.

Полный root `clippy -D warnings` остаётся заблокирован прежним legacy lint debt
вне CF-014; это не считается успешной проверкой новой реализации.
