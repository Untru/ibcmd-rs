# CF-015: corruption/property/fuzz suite контейнера

Дата фиксации: 2026-07-23.

## Результат

Контейнерные readers, writers, payload codecs и bounded tree visitor теперь
имеют общий набор негативных, генеративных и fuzz-проверок. Все обязательные
regressions выполняются обычным `cargo test` на Linux и Windows без платформы
1С, `ibcmd`, Designer, EDT, Java/JVM, SQL-сервера или внешнего корпуса.

## Clean-room malformed corpus

`tests/fixtures/malformed/cf/` содержит маленькие reviewable `.hex` seeds,
созданные вручную и не содержащие vendor configuration data. Они закрепляют
точные типизированные ошибки для следующих границ:

- усечённый и неизвестный file header;
- не-hex символ в block header;
- TOC offset за пределами входа;
- self-cycle цепочки страниц;
- нечётная длина и unpaired surrogate в UTF-16LE имени;
- структурно допустимая запись с повреждённым raw-DEFLATE payload;
- усечённый вложенный stored container с сохранением его logical path.

Парсер тестового корпуса игнорирует комментарии после `#`, поэтому каждый seed
остаётся читаемым byte-level описанием и может быть минимизирован вручную.

## Детерминированные свойства

`tests/cf_corruption.rs` выполняет четыре переносимые группы проверок:

1. Все сохранённые malformed seeds доходят до ожидаемой ветви typed error.
2. 512 псевдослучайных входов длиной до 4096 байт проходят bounded open и
   выборочное чтение без panic и обхода лимита числа entries.
3. 128 сгенерированных пар Format15/Format16 покрывают страницы
   `31/64/511/512/1024`, payload lengths на границах страниц, UTF-16 names и
   различие absent/empty; `parse(build(x))` сохраняет всю семантику.
4. Stored trees глубиной `0..=6` собираются writers и обходятся visitor с
   точным числом containers/entries и исходным leaf payload.

Генератор использует фиксированный seed: падение воспроизводится без сети,
часов, entropy source и сторонних binaries.

## Fuzz boundary

`fuzz/fuzz_targets/cf_parse.rs` — отдельный `cargo-fuzz` workspace. Один вход
ограничен 1 MiB; общие `ResourceLimits` задают depth 8, 64 entries, по 1 MiB
encoded/decoded bytes и ratio 200. Target независимо вызывает:

- unified Format15/Format16 open и чтение каждого разрешённого entry;
- stored и strict raw-DEFLATE payload decoding;
- nested traversal со всеми вариантами `Skip`, `Leaf` и `Container`.

Он не входит в default workspace, release graph или обязательную установку.
После установки nightly и `cargo-fuzz` разработчик запускает:

```text
cargo fuzz run cf_parse -- -max_len=1048576
```

Найденный и минимизированный сбой переносится в clean-room `.hex` corpus и
получает точное утверждение в обычном integration test.

## Проверки

- `cargo test --locked --test cf_corruption` — 4 passed;
- `cargo check --locked --manifest-path fuzz/Cargo.toml --bin cf_parse` — passed;
- `cargo clippy --locked --manifest-path fuzz/Cargo.toml --bin cf_parse -- -D warnings` — passed;
- `cargo test --locked --workspace --exclude ibcmd-rs` — portable crates;
- `cargo test --locked --test cf_corpus` — checked-in valid corpus;
- `cargo clippy --locked --workspace --exclude ibcmd-rs --all-targets -- -D warnings` — portable strict lint.

Обычный `cargo run` не является способом запуска libFuzzer target: на Windows
он закономерно не получает linker entry point без `cargo-fuzz`. Полный root
`clippy -D warnings` также остаётся заблокирован прежним legacy lint debt вне
CF-015; строгий lint нового независимого fuzz workspace проходит.
