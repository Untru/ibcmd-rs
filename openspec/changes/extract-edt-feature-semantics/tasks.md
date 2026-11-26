# Xcore-derived feature semantics — план реализации

> **Для субагента:** выполнять задачи по одной с ревью между ними.

**Цель:** реализовать первый воспроизводимый vertical slice задачи GitHub #278.

**Дизайн:** `openspec/changes/extract-edt-feature-semantics/design.md`

## Задача 1: Определить строгую модель feature semantics

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/src/lib.rs`

**Проверка:**
- [x] `pending` и `verified` представлены типобезопасно.
- [x] Неполное verified XML-правило отклоняется.
- [x] Дубликаты semantic keys отклоняются.

## Задача 2: Реализовать Xcore research importer

**Статус:** `[x]`

**Файлы:** `tools/import-edt-xcore-semantics.ps1`

**Проверка:**
- [x] Источник читается из inventory, но абсолютные пути не пишутся в output.
- [x] Извлекаются kind, qualifiers, model type, lower/upper bounds и explicit default.
- [x] Комментарии/annotations/operation bodies не распознаются как features.
- [x] Неизвестные classifier/feature/multiplicity отклоняются fail-closed.
- [x] Результат детерминирован и не содержит Xcore/JAR/class bytes.

## Задача 3: Добавить form semantics corpus

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/data/edt-2025.2.3-feature-semantics.json`

**Проверка:**
- [x] Corpus создан повторяемым importer.
- [x] XML behaviour без подтверждения помечен `pending`.
- [x] Есть representative attributes, references, containment и defaults.

## Задача 4: Подключить bundled API и тесты

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/src/lib.rs`

**Проверка:**
- [x] Corpus загружается через публичный bundled API.
- [x] Тесты проверяют реальные form features и независимость от EDT.
- [x] `cargo test -p ibcmd-schema --locked` проходит.

## Задача 5: Проверить и опубликовать первый инкремент

**Статус:** `[x]`

**Проверка:**
- [x] `openspec validate extract-edt-feature-semantics --strict`.
- [x] `cargo fmt --all -- --check`.
- [x] `cargo test -p ibcmd-schema --locked`.
- [x] Повторная генерация даёт тот же SHA-256.
