# Автономный многоверсионный конвертер XML/CF — план реализации

> **Для субагента:** выполнять одну задачу/issue за раз, отдельным PR, с ревью между задачами.

**Цель:** построить полностью автономный Rust-конвертер XML/CF с профилями версий, lossless-моделью и контролируемыми межверсионными миграциями.

**Дизайн:** `openspec/changes/build-offline-converter/design.md`

**GitHub:** [epic #178](https://github.com/Untru/ibcmd-rs/issues/178); 56 дочерних задач — #179–#234.

**Порядок:** зависимости ниже обязательны; задачи одной фазы без общей зависимости можно выполнять параллельно.

**Общая проверка каждого PR:** `cargo fmt --all --check`, `cargo check --locked --all-targets`, релевантные targeted tests, `git diff --check`. Production-код не должен запускать компоненты 1С/EDT.

---

## Фаза 0 — безопасная стартовая точка

### BASE-001 — Восстановить зелёный Rust baseline

- **Статус:** `[x]`; **зависимости:** нет
- **Файлы:** изменить `src/mssql_dump/tests.rs`; при необходимости `src/mssql_dump/form_body.rs`
- **Действия:** добавить `fixing_in_table` в три устаревших fixture-конструктора; убрать/обосновать warnings.
- **Проверка:** `cargo test --locked --lib --no-run` и CI-safe suite проходят; production behavior не меняется.
- **Коммит:** `test: restore green all-target baseline`

### BASE-002 — Зафиксировать standalone и lossless contracts

- **Статус:** `[x]`; **зависимости:** нет
- **Файлы:** создать `docs/architecture/standalone-core.md`; изменить `README.md`
- **Действия:** описать запрещённые runtime dependencies, version axes, opaque policy, capability levels и loss semantics.
- **Проверка:** документ явно отличает repack, overlay, bootstrap и conversion; release path запрещает `ibcmd`, `1cv8`, EDT/JVM.
- **Коммит:** `docs: define standalone conversion contract`

### BASE-003 — Создать workspace и пустые layer crates

- **Статус:** `[x]`; **зависимости:** BASE-001, BASE-002
- **Файлы:** изменить `Cargo.toml`, `Cargo.lock`; создать `crates/ibcmd-core/`, `crates/ibcmd-xml/`, `crates/ibcmd-v8/`, `crates/ibcmd-cf/`
- **Действия:** добавить workspace packages и dependency direction; root package остаётся CLI/legacy layer.
- **Проверка:** `cargo test --workspace`; `ibcmd-core` не зависит от clap/XML/SQL/process crates; текущий CLI собирается без изменения поведения.
- **Коммит:** `refactor: add standalone codec workspace`

### BASE-004 — Разделить portable/live/research tests и расширить CI

- **Статус:** `[x]`; **зависимости:** BASE-001, BASE-003
- **Файлы:** изменить `.github/workflows/ci.yml`, `Cargo.toml`; создать `tests/common/`
- **Действия:** заменить skip-by-name на явные test tiers; добавить Windows/Linux, clippy и all-target portable suite.
- **Проверка:** portable suite не требует MSSQL, абсолютных lab-путей или компонентов 1С; Windows legacy lane сохранён.
- **Коммит:** `ci: add portable workspace test lanes`

## Фаза 1 — версии, профили и модели ядра

### CORE-001 — Реализовать открытые типы версий и независимые оси

- **Статус:** `[x]`; **зависимости:** BASE-003
- **Файлы:** создать `crates/ibcmd-core/src/version.rs`; изменить `crates/ibcmd-core/src/lib.rs`
- **Действия:** реализовать dotted version, `PlatformBuild`, `XmlDialect`, `CompatibilityMode`, `StorageVersion`, `ContainerRevision` без неявных преобразований.
- **Проверка:** parse/order/serde tests для `2.17`, `2.20`, `2.21`, `8.3.24.1819`, `8.3.27.1989`, `8.5.1.1150` и неизвестной будущей версии.
- **Коммит:** `feat(core): separate version axes`

### CORE-002 — Описать artifact и storage identities

- **Статус:** `[x]`; **зависимости:** CORE-001
- **Файлы:** создать `crates/ibcmd-core/src/artifact.rs`
- **Действия:** добавить `ArtifactFormat`, `ArtifactKind`, `DbmsKind`, `ProfileId`, `StorageProfileId`; формат не содержит версию.
- **Проверка:** CF/XML/infobase и Configuration/Extension/EPF/ERF сериализуются независимо; неизвестные IDs не ломают чтение.
- **Коммит:** `feat(core): model artifact identities`

### CORE-003 — Реализовать schema профиля, inheritance и loader

- **Статус:** `[x]`; **зависимости:** CORE-001, CORE-002
- **Файлы:** создать `crates/ibcmd-core/src/profile.rs`, `profiles/schema.json`, `src/profile_registry.rs`, `profiles/README.md`
- **Действия:** capability schema, один `extends`, deterministic delta merge, bundled/external loading и provenance effective values.
- **Проверка:** cycle/missing parent/duplicate/conflicting override отвергаются; результат не зависит от порядка файлов.
- **Коммит:** `feat(core): add extensible version profiles`

### CORE-004 — Добавить начальные experimental profiles и detection evidence

- **Статус:** `[x]`; **зависимости:** CORE-003
- **Файлы:** создать `profiles/xml/2.17.json`, `2.20.json`, `2.21.json`, `profiles/platform/8.3.24.1819.json`, `8.3.27.1989.json`, `8.5.1.1150.json`, `crates/ibcmd-core/src/detection.rs`
- **Действия:** seed только доказанные coordinates/fingerprints; реализовать `exact|ambiguous|unknown` с evidence.
- **Проверка:** platform build не выводится только из XML version; encode запрещён при ambiguous/unknown target.
- **Коммит:** `feat(core): seed profiles and artifact detection`

### CORE-005 — Ввести структурированные diagnostics и loss policy

- **Статус:** `[x]`; **зависимости:** CORE-002
- **Файлы:** создать `crates/ibcmd-core/src/diagnostic.rs`
- **Действия:** stable code, severity, object/property path, profiles, recovery hint; `Error|Warn|DropExplicitly`.
- **Проверка:** deterministic JSON; default loss policy — error; drop невозможен без codec declaration.
- **Коммит:** `feat(core): add structured loss diagnostics`

### CORE-006 — Выделить ordered `StorageImage`

- **Статус:** `[x]`; **зависимости:** CORE-002, CORE-005
- **Файлы:** создать `crates/ibcmd-core/src/storage.rs`; изменить `src/mssql_dump/config_rows.rs`
- **Действия:** entry name/key, multipart identity, attributes, raw header, packed/unpacked payload, compression and provenance.
- **Проверка:** модель не содержит SQL/CF types; multipart ordering и hashes детерминированы.
- **Коммит:** `refactor(core): extract neutral storage image`

### CORE-007 — Реализовать ordered canonical values, provenance и opaque facets

- **Статус:** `[x]`; **зависимости:** CORE-002, CORE-005
- **Файлы:** создать `crates/ibcmd-core/src/value.rs`, `provenance.rs`, `opaque.rs`, `asset.rs`
- **Действия:** typed scalar/reference/record/sequence/binary values; anchored opaque data с source profile и digest.
- **Проверка:** порядок сохраняется; unknown enum token допустим; cross-profile opaque emit выдаёт diagnostic.
- **Коммит:** `feat(core): add lossless canonical values`

### CORE-008 — Добавить identity graph, validation и semantic digest

- **Статус:** `[x]`; **зависимости:** CORE-007
- **Файлы:** создать `crates/ibcmd-core/src/identity.rs`, `model.rs`, `graph.rs`, `validate.rs`, `semantic.rs`
- **Действия:** UUID/logical IDs, ownership, refs, generated types, graph indexes и semantic equality.
- **Проверка:** duplicate UUID/path, dangling ref и ownership cycle выявляются; digest одинаков на Windows/Linux.
- **Коммит:** `feat(core): model and validate metadata graph`

### CORE-009 — Определить adapter, preflight и `FamilyCodec` contracts

- **Статус:** `[x]`; **зависимости:** CORE-004–CORE-008
- **Файлы:** создать `crates/ibcmd-core/src/adapter.rs`, `family.rs`, `capability.rs`
- **Действия:** decode/encode request/outcome, mandatory preflight, codec registry, capability/preservation levels.
- **Проверка:** fatal diagnostics запрещают writer start; duplicate codec registration отвергается; API не содержит path к platform executable.
- **Коммит:** `feat(core): define codec adapter contracts`

### CORE-010 — Добавить legacy bridge без смешения версий

- **Статус:** `[x]`; **зависимости:** CORE-004, CORE-009
- **Файлы:** создать `src/adapters/mssql_legacy.rs`, `src/legacy_version.rs`; изменить `src/cli.rs`, `src/infobase.rs`, `src/mssql_dump/mod.rs`
- **Действия:** преобразовать старый enum в отдельные axes; оформить MSSQL path как provider/legacy adapter; отметить `requires_base_blob` capability.
- **Проверка:** старые команды работают; core не импортирует `InfobaseConfigSourceVersion`; строковая смена XML version не используется новым путём.
- **Коммит:** `refactor: wrap legacy storage and version paths`

## Фаза 2 — автономный XCF adapter

### XML-001 — Реализовать ordered lossless XML parser/emitter

- **Статус:** `[x]`; **зависимости:** BASE-003, CORE-007
- **Файлы:** создать `crates/ibcmd-xml/src/node.rs`, `reader.rs`, `writer.rs`
- **Действия:** сохранить QName, namespace declarations, attribute/child order, text, CDATA и comments; configurable lexical policy.
- **Проверка:** parse→emit→parse сохраняет дерево; malformed XML даёт position-aware error; escaping покрыт тестами.
- **Коммит:** `feat(xml): add ordered lossless XML layer`

### XML-002 — Реализовать безопасный source-tree reader/writer

- **Статус:** `[x]`; **зависимости:** XML-001, CORE-008
- **Файлы:** создать `crates/ibcmd-xml/src/source_tree/reader.rs`, `writer.rs`; переиспользовать правила из `src/source.rs`
- **Действия:** inventory modules/assets, normalized paths, traversal/duplicate rejection, preflight и atomic directory publish.
- **Проверка:** ошибка не заменяет destination; conflicting UUID/path диагностируется до записи.
- **Коммит:** `feat(xml): add atomic source tree adapter`

### XML-003 — Добавить detection и delta descriptors диалектов

- **Статус:** `[x]`; **зависимости:** CORE-004, XML-001
- **Файлы:** создать `crates/ibcmd-xml/src/dialect.rs`, `profiles/xml/*.json`
- **Действия:** root/namespaces/feature evidence, baseline+delta rules, lexical policies.
- **Проверка:** 2.17 не отвергается closed enum; conflicting evidence возвращает ambiguity; build платформы не угадывается.
- **Коммит:** `feat(xml): detect and describe XCF dialects`

### XML-004 — Реализовать common metadata envelope и opaque fallback

- **Статус:** `[x]`; **зависимости:** XML-001, XML-003, CORE-008
- **Файлы:** создать `crates/ibcmd-xml/src/metadata/common.rs`, `fallback.rs`, `registry.rs`
- **Действия:** UUID/kind/name/synonym/generated types/children в typed IR; остальные узлы anchored opaque.
- **Проверка:** unknown family same-profile roundtrip; cross-profile encode блокируется с object path.
- **Коммит:** `feat(xml): add metadata envelope and fallback`

### XML-005 — Перевести Constant как pilot codec

- **Статус:** `[x]`; **зависимости:** XML-004, CORE-010
- **Файлы:** создать `crates/ibcmd-xml/src/metadata/constant.rs`; изменить Constant routing в `src/mssql_dump/mod.rs`
- **Действия:** XML 2.20/2.21 ↔ IR без base blob; shadow comparison со старым formatter.
- **Проверка:** semantic equality, сохранение unknown sibling, отсутствие regressions в существующих Constant fixtures.
- **Коммит:** `feat(xml): route Constant through canonical IR`

### XML-006 — Добавить versioned XML corpus и 2.20/2.21 delta tests

- **Статус:** `[x]`; **зависимости:** XML-003–XML-005
- **Файлы:** создать `tests/fixtures/xml/{2.17,2.20,2.21}/`, `tests/xml_profiles.rs`
- **Действия:** минимальная configuration/module/object-with-child fixture на диалект; expected namespaces/order/defaults.
- **Проверка:** parse→emit→parse, UUID/order/opaque preservation; root-version-only conversion тестом запрещена.
- **Коммит:** `test(xml): add dialect corpus and delta matrix`

## Фаза 3 — native CF container и архив

### CF-001 — Добавить versioned CF fixture corpus

- **Статус:** `[ ]`; **зависимости:** BASE-002
- **Файлы:** создать `tests/fixtures/cf/README.md`, `manifest.json`, `tests/cf_corpus.rs`
- **Действия:** минимальные Format15/Format16 fixtures, hashes, provenance, exact coordinates; optional external corpus env.
- **Проверка:** обычные тесты не запускают 1С; corpus не содержит сторонний прикладной код или секреты.
- **Коммит:** `test(cf): add versioned offline fixture corpus`

### CF-002 — Поддержать storage versions и raw headers Format15

- **Статус:** `[ ]`; **зависимости:** BASE-003, CF-001
- **Файлы:** перенести/изменить `src/v8_container.rs` в `crates/ibcmd-v8/src/format15.rs`
- **Действия:** версии 0/1/2/5, порядок, raw headers, absent-data sentinel; сохранить module-blob behavior.
- **Проверка:** fixtures перечисляют ожидаемые entries; существующие `module_blob` tests зелёные.
- **Коммит:** `feat(v8): preserve Format15 headers and versions`

### CF-003 — Реализовать безопасное чтение chained pages

- **Статус:** `[ ]`; **зависимости:** CF-002
- **Файлы:** создать `crates/ibcmd-v8/src/block.rs`
- **Действия:** собрать 2+/3+ page chain; detect cycle, overlap, repeated/out-of-range address, size mismatch.
- **Проверка:** boundary payloads и corrupt synthetic chains возвращают точные typed errors.
- **Коммит:** `feat(v8): read chained block pages`

### CF-004 — Реализовать autodetect и parser Format16

- **Статус:** `[ ]`; **зависимости:** CF-002, CF-003
- **Файлы:** создать `crates/ibcmd-v8/src/format16.rs`, `format.rs`
- **Действия:** 64-bit TOC/block fields, 55-byte headers, sentinel, versioned/base-offset preamble detection.
- **Проверка:** Format15/16 autodetect; golden fixture даёт ожидаемые names/counts; truncation fail-closed.
- **Коммит:** `feat(v8): parse Format16 containers`

### CF-005 — Реализовать strict raw-DEFLATE и resource limits

- **Статус:** `[ ]`; **зависимости:** CF-001, CORE-005
- **Файлы:** создать `crates/ibcmd-cf/src/payload.rs`, `crates/ibcmd-core/src/limits.rs`
- **Действия:** `Stored|RawDeflate`, full input consumption/StreamEnd, max depth/count/bytes/ratio.
- **Проверка:** truncated/trailing stream и decompression bomb контролируемо отклоняются без panic/OOM.
- **Коммит:** `feat(cf): add bounded payload decoding`

### CF-006 — Добавить bounded streaming и nested traversal

- **Статус:** `[ ]`; **зависимости:** CF-003–CF-005
- **Файлы:** создать `crates/ibcmd-v8/src/reader.rs`, `crates/ibcmd-cf/src/tree.rs`
- **Действия:** `Read + Seek`, lazy selected entry, nested traversal, limits propagation.
- **Проверка:** large/sparse fixture не загружается целиком; recursive bomb получает typed limit error.
- **Коммит:** `feat(cf): stream nested container entries`

### CF-007 — Реализовать deterministic Format15 writer

- **Статус:** `[ ]`; **зависимости:** CF-002, CF-003
- **Файлы:** создать `crates/ibcmd-v8/src/writer.rs`
- **Действия:** TOC, addresses, page chains, absent sentinel, preserved headers и deterministic allocation.
- **Проверка:** sizes 0/1/511/512/513/large проходят write→parse; повторная запись byte-identical.
- **Коммит:** `feat(v8): write deterministic Format15 containers`

### CF-008 — Смоделировать preamble и реализовать Format16 writer

- **Статус:** `[ ]`; **зависимости:** CF-004, CF-006, CF-007, CORE-004
- **Файлы:** создать `crates/ibcmd-cf/src/preamble.rs`; расширить `crates/ibcmd-v8/src/writer.rs`, `format16.rs`
- **Действия:** semantic preamble model, 64-bit streaming offsets, preserve/generate modes.
- **Проверка:** parser принимает output; writer не держит все payloads в RAM; preamble не копируется как недокументированная константа.
- **Коммит:** `feat(cf): write Format16 archives`

### CF-009 — Отобразить CF archive в `StorageImage`

- **Статус:** `[ ]`; **зависимости:** CORE-006, CF-004–CF-006
- **Файлы:** создать `crates/ibcmd-cf/src/archive.rs`
- **Действия:** ordered logical entries, suffix/multipart identity, raw/packed payload and source profile; duplicate rejection.
- **Проверка:** CF→StorageImage deterministic; expected root/version/versions and object entries доступны по logical keys.
- **Коммит:** `feat(cf): decode archive into storage image`

### CF-010 — Добавить offline `cf inspect` и `cf verify`

- **Статус:** `[ ]`; **зависимости:** CF-009
- **Файлы:** изменить `src/cli.rs`, `src/main.rs`; создать `src/commands/cf.rs`
- **Действия:** JSON layout/profile/elements/compression/errors; selective hash/list verification.
- **Проверка:** команды работают при пустом PATH; corrupt fixture возвращает non-zero и machine-readable error.
- **Коммит:** `feat(cli): add offline CF inspection`

### CF-011 — Реализовать lossless CF repack и atomic output

- **Статус:** `[ ]`; **зависимости:** CF-007–CF-009
- **Файлы:** создать `crates/ibcmd-cf/src/writer.rs`, `tests/cf_roundtrip.rs`
- **Действия:** preserve order/header/unpacked bytes/compression; temporary file, re-open validation, atomic publish.
- **Проверка:** parse(write(parse(cf))) semantic-equal; unchanged entries byte-preserved; два outputs одинаковы.
- **Коммит:** `feat(cf): add deterministic lossless repack`

### CF-012 — Подключить автономный CF → XML export

- **Статус:** `[ ]`; **зависимости:** CF-009, CORE-010, XML-004
- **Файлы:** создать `crates/ibcmd-cf/src/export.rs`; изменить `src/mssql_dump/mod.rs`, `src/cli.rs`
- **Действия:** source exporter принимает `StorageImage`, SQL становится отдельным provider; report supported/opaque/failed.
- **Проверка:** CF fixture экспортируется теми же family decoders без subprocess; MSSQL parity не ухудшается.
- **Коммит:** `feat(cf): export CF to XML offline`

### CF-013 — Выделить pure source → `StoragePatch`

- **Статус:** `[x]`; **зависимости:** CORE-006, CORE-009, CORE-010
- **Файлы:** создать `src/compiler/mod.rs`, `src/compiler/overlay.rs`; изменить `src/mssql.rs`, `src/module_blob.rs`
- **Действия:** packers возвращают `Compiled|NeedsBase|Unsupported` entries без SQL; MSSQL staging использует тот же API.
- **Проверка:** pure unit tests; ни один compiler API не принимает connection/process types.
- **Коммит:** `refactor: extract pure storage patch compiler`

### CF-014 — Реализовать XML overlay поверх base CF

- **Статус:** `[ ]`; **зависимости:** CF-011, CF-013, XML-002
- **Файлы:** создать `crates/ibcmd-cf/src/overlay.rs`; изменить CLI command layer
- **Действия:** replace compiled entries, update versions, preserve unknown entries, atomic write.
- **Проверка:** module и raw asset smoke; only intended entries change; NeedsBase uses base, Unsupported blocks operation.
- **Коммит:** `feat(cf): overlay XML changes on base archive`

### CF-015 — Добавить corruption/property/fuzz suite контейнера

- **Статус:** `[ ]`; **зависимости:** CF-005–CF-011
- **Файлы:** создать `tests/fixtures/malformed/cf/`, `tests/cf_corruption.rs`, `fuzz/fuzz_targets/cf_parse.rs`
- **Действия:** headers/hex/offsets/chains/UTF-16/DEFLATE/nesting corpus; parse(build(x)) properties.
- **Проверка:** arbitrary input не panic/OOM/hang; regression seeds сохраняются; valid generated trees roundtrip.
- **Коммит:** `test(cf): harden codecs with fuzz corpus`

## Фаза 4 — bootstrap compiler без base CF

### BOOT-001 — Сгенерировать bootstrap readiness manifest

- **Статус:** `[x]`; **зависимости:** CF-013, CORE-009
- **Файлы:** создать `src/compiler/readiness.rs`, `compatibility/bootstrap.json`
- **Действия:** для каждого source artifact указать expected entries, codec и blocker; инвентаризировать все base reads.
- **Проверка:** manifest покрывает все маршруты source scan; build запрещён при любом NeedsBase/Unsupported.
- **Коммит:** `feat(compiler): inventory bootstrap blockers`

### BOOT-002 — Построить identity graph и special entries

- **Статус:** `[ ]`; **зависимости:** BOOT-001, CORE-008, CORE-004
- **Файлы:** создать `src/compiler/identity.rs`, `graph.rs`, `root.rs`, `version.rs`, `versions.rs`
- **Действия:** IDs, collections, generated types, suffixes, root/version/versions/configuration body без base.
- **Проверка:** повторная сборка deterministic; все special-entry ссылки разрешаются и соответствуют storage inventory.
- **Коммит:** `feat(compiler): generate root and identity entries`

### BOOT-003 — Base-free codecs простых metadata families

- **Статус:** `[x]`; **зависимости:** BOOT-002, XML-004
- **Файлы:** создать `src/compiler/families/simple.rs`; адаптировать соответствующие парсеры `src/module_blob.rs`
- **Действия:** Constant, Language, SessionParameter, DefinedType, FunctionalOption/Parameter.
- **Проверка:** XML→blob→IR/XML без base для каждой family; readiness становится Compiled только по зелёным fixtures.
- **Коммит:** `feat(compiler): bootstrap simple metadata`

### BOOT-004 — Base-free codecs service metadata

- **Статус:** `[x]` (7/7: ScheduledJob, EventSubscription, HTTPService, WebService, IntegrationService, WSReference, XDTOPackage); **зависимости:** BOOT-002, XML-004
- **Файлы:** создать `src/compiler/families/services.rs`
- **Действия:** ScheduledJob, EventSubscription, HTTP/Web/Integration services, WSReference, XDTOPackage.
- **Проверка:** family fixtures roundtrip без base; unknown service fields fail-closed/preserved by profile rules.
- **Коммит:** `feat(compiler): bootstrap service metadata`

### BOOT-005 — Base-free codecs Catalog и Document

- **Статус:** `[x]`; **зависимости:** BOOT-002, CORE-008
- **Файлы:** создать `src/compiler/families/catalog.rs`, `document.rs`
- **Действия:** roots, attributes, tabular sections, commands/forms/templates refs и generated types.
- **Проверка:** minimal + child-rich fixtures на family; object graph и entry inventory exact, без base reads.
- **Коммит:** `feat(compiler): bootstrap catalog and document`

### BOOT-006 — Base-free codecs Report/DataProcessor/Enum/Settings

- **Статус:** `[x]` (4/4: Report, DataProcessor, Enum, SettingsStorage); **зависимости:** BOOT-002
- **Файлы:** создать `src/compiler/families/report.rs`, `data_processor.rs`, `enum.rs`, `settings.rs`
- **Действия:** metadata roots/children/assets refs для cohort.
- **Проверка:** XML→storage→XML semantic roundtrip; generated types и child order стабильны.
- **Коммит:** `feat(compiler): bootstrap report processor and enum families`

### BOOT-007 — Base-free codecs Subsystem/ExchangePlan/BusinessProcess/Task

- **Статус:** `[ ]`; **зависимости:** BOOT-002
- **Файлы:** создать `src/compiler/families/subsystem.rs`, `exchange_plan.rs`, `business_process.rs`, `task.rs`
- **Действия:** hierarchical content, routes, tables, child refs and generated types.
- **Проверка:** nested subsystem и BP/Task fixtures; ownership/reference validation; no base dependency.
- **Коммит:** `feat(compiler): bootstrap hierarchical metadata`

### BOOT-008 — Base-free codecs registers и charts

- **Статус:** `[ ]`; **зависимости:** BOOT-002
- **Файлы:** создать `src/compiler/families/registers.rs`, `charts.rs`, `recalculation.rs`
- **Действия:** information/accumulation/accounting/calculation registers, recalculations, charts of types/accounts.
- **Проверка:** fixture на каждый layout cohort; standard fields/generated types deterministic; unsupported tail blocks output.
- **Коммит:** `feat(compiler): bootstrap registers and charts`

### BOOT-009 — Base-free codecs modules, commands и source assets

- **Статус:** `[ ]`; **зависимости:** BOOT-002, CF-005
- **Файлы:** создать `src/compiler/families/modules.rs`, `commands.rs`, `assets.rs`; адаптировать `src/mssql_dump/source_assets.rs`
- **Действия:** CommonModule, CommonCommand/Group, pictures, help and binary assets.
- **Проверка:** text encoding/compression/assets hashes roundtrip; no source-path heuristics outside registry.
- **Коммит:** `feat(compiler): bootstrap modules commands and assets`

### BOOT-010 — Base-free codecs Rights/Predefined/support data

- **Статус:** `[ ]`; **зависимости:** BOOT-002, CORE-005
- **Файлы:** создать `src/compiler/bodies/rights.rs`, `predefined.rs`, `support.rs`; адаптировать `src/mssql_dump/role_rights.rs`
- **Действия:** encode without base, preserve unsupported signature/support facets or report blocker.
- **Проверка:** role and predefined fixtures roundtrip; signatures никогда не подделываются и не теряются молча.
- **Коммит:** `feat(compiler): bootstrap rights and support data`

### BOOT-011 — Base-free managed Form и CommandInterface codecs

- **Статус:** `[ ]`; **зависимости:** BOOT-002, CORE-008
- **Файлы:** создать `src/compiler/bodies/form.rs`, `command_interface.rs`; адаптировать `src/mssql_dump/form_body.rs`, `command_interface.rs`
- **Действия:** full typed form tree/refs/commands, deterministic IDs/order, no base blob.
- **Проверка:** representative element matrix; XML→blob→XML semantic equality; unknown layouts block bootstrap.
- **Коммит:** `feat(compiler): bootstrap managed forms`

### BOOT-012 — Base-free template codecs MXL/DCS и прочих bodies

- **Статус:** `[ ]`; **зависимости:** BOOT-002, CF-005
- **Файлы:** создать `src/compiler/bodies/mxl.rs`, `dcs.rs`, `template.rs`; адаптировать `src/mssql_dump/moxel.rs`, `dcs.rs`
- **Действия:** compile supported template types, preserve binary identity and nested containers.
- **Проверка:** fixture на каждый заявленный template kind; semantic roundtrip; unknown template blocks bootstrap.
- **Коммит:** `feat(compiler): bootstrap template bodies`

### BOOT-013 — Собрать новый CF из полного XML tree

- **Статус:** `[ ]`; **зависимости:** BOOT-001–BOOT-012, CF-008, CF-011
- **Файлы:** создать `crates/ibcmd-cf/src/bootstrap.rs`; изменить CLI command layer
- **Действия:** source tree→IR→complete StorageImage→Format15/16; enforce readiness and atomic validation.
- **Проверка:** без `--base`; all entries reachable from root/versions; CF→XML не имеет missing/extra для verified corpus; unsupported feature prevents output.
- **Коммит:** `feat(cf): bootstrap configuration from XML`

## Фаза 5 — миграции, CLI и доказательство готовности

### MIG-001 — Определить migration steps и compatibility analyzer

- **Статус:** `[x]`; **зависимости:** CORE-005, CORE-008
- **Файлы:** создать `crates/ibcmd-core/src/migration/step.rs`, `compatibility.rs`
- **Действия:** source/target constraints, analyze/apply split, touched capabilities, possible losses.
- **Проверка:** каждый incompatible path получает stable diagnostic; analyze не мутирует модель.
- **Коммит:** `feat(core): define migration contracts`

### MIG-002 — Реализовать graph planner, transactional executor и report

- **Статус:** `[x]`; **зависимости:** MIG-001, CORE-004
- **Файлы:** создать `crates/ibcmd-core/src/migration/graph.rs`, `executor.rs`, `report.rs`
- **Действия:** deterministic path, clone/apply/validate, composed loss report.
- **Проверка:** no-path/ambiguity/cycle errors; fatal step leaves source unchanged; stable JSON route report.
- **Коммит:** `feat(core): execute deterministic migrations`

### MIG-003 — Реализовать доказанный 2.20 → 2.21 edge

- **Статус:** `[ ]`; **зависимости:** MIG-002, XML-006 и соответствующие family codecs
- **Файлы:** создать `crates/ibcmd-core/src/migration/v2_20_to_v2_21.rs`, fixtures `tests/fixtures/migrations/2.20-to-2.21/`
- **Действия:** только подтверждённые feature deltas; unchanged/changed/newly-supported cases.
- **Проверка:** целевой adapter читает output; переносимые fixtures имеют пустой loss report; неизвестная delta блокируется.
- **Коммит:** `feat(migration): add XCF 2.20 to 2.21 edge`

### MIG-004 — Реализовать 2.21 → 2.20 downgrade с loss policies

- **Статус:** `[ ]`; **зависимости:** MIG-003
- **Файлы:** создать `crates/ibcmd-core/src/migration/v2_21_to_v2_20.rs`, downgrade fixtures
- **Действия:** error/warn/drop rules и path-addressed losses.
- **Проверка:** default error; explicit safe drop записан в report; unsupported property не исчезает молча.
- **Коммит:** `feat(migration): add guarded XCF downgrade`

### APP-001 — Добавить platform-independent conversion service и CLI

- **Статус:** `[ ]`; **зависимости:** CORE-009, MIG-002, XML adapter, CF adapter
- **Файлы:** создать `src/conversion.rs`; изменить `src/cli.rs`, `src/main.rs`
- **Действия:** `decode→validate→plan→migrate→preflight→atomic encode`; options formats/profiles/dry-run/loss/report.
- **Проверка:** dry-run не пишет; format/version задаются раздельно; никакой subprocess 1С не запускается.
- **Коммит:** `feat(cli): add offline configuration conversion`

### APP-002 — Генерировать compatibility matrix из evidence

- **Статус:** `[ ]`; **зависимости:** CORE-003, CORE-009, APP-001
- **Файлы:** создать `compatibility/matrix.json`, schema и `tests/compatibility_matrix.rs`; изменить `src/compatibility.rs`
- **Действия:** operation×artifact×source×target×family×evidence; experimental/verified status.
- **Проверка:** verified без green evidence невозможен; unknown profile unsupported; CLI читает тот же источник истины.
- **Коммит:** `feat: derive compatibility from test evidence`

### APP-003 — Изолировать platform-oracle code от default/release build

- **Статус:** `[ ]`; **зависимости:** APP-001, CORE-010
- **Файлы:** изменить `Cargo.toml`, `src/infobase.rs`, `src/dump_sources.rs`, `src/probe.rs`, CLI routing
- **Действия:** legacy research calls только под non-default `platform-oracle`; release binary не содержит probe/run path.
- **Проверка:** default binary builds/runs on Linux; binary/dependency audit не находит EDT/JNI/vendor executable payloads.
- **Коммит:** `refactor: isolate platform oracle integrations`

### APP-004 — Добавить offline E2E, readiness gate и reproducible release

- **Статус:** `[ ]`; **зависимости:** BOOT-013, MIG-004, APP-002, APP-003, CF-015
- **Файлы:** создать `tests/offline_conversion_matrix.rs`, `.github/workflows/offline-e2e.yml`, `.github/workflows/release.yml`, `docs/release-criteria.md`
- **Действия:** XML↔CF, same/cross-profile, malformed, clean environment; SHA-256/SBOM/release smoke.
- **Проверка:** merge/release запрещены при missing evidence/unexpected loss; release archive не содержит компонентов 1С/EDT; финальная валидация всех требований design/specs.
- **Коммит:** `ci: gate standalone converter releases`

---

## Критический путь

```text
BASE-001/002 -> BASE-003 -> CORE-001..009
                              |          \
                              |           -> XML-001..006
                              -> CF-001..011 -> CF-012/013/014
                                                   |
                                                   -> BOOT-001..013
CORE model + adapters -> MIG-001..004 ----------------|
                                                        -> APP-001..004
```

## Финальная валидация

Финальная задача `APP-004` считается завершённой только если выполнены все предыдущие задачи, строгая OpenSpec-проверка, workspace tests, offline E2E, migration matrix, malformed/fuzz smoke и release artifact audit.
