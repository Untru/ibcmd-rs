# Canonical coverage map — план реализации

**Цель:** связать каждый EDT feature с явным canonical preservation status.

## Задача 1: Добавить coverage schema

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/src/lib.rs`

**Проверка:**
- [x] Типизированы четыре статуса и evidence.
- [x] Duplicate/stale/unmapped entries отклоняются.

## Задача 2: Добавить bootstrap generator

**Статус:** `[x]`

**Файлы:** `tools/generate-canonical-coverage.ps1`

**Проверка:**
- [x] Новый feature получает явный `unsupported` placeholder, не пропускается.
- [x] Генерация детерминирована.
- [x] Route/key dictionaries используют ordinal case-sensitive comparison;
  регистровая мутация package name отклоняется.

## Задача 3: Разметить существующие canonical families

**Статус:** `[ ]`

**Файлы:** `crates/ibcmd-schema/data/edt-2025.2.3-canonical-coverage.json`

**Проверка:**
- [x] Metadata/forms/DCS/MXL/common/other имеют отдельные агрегаты; сумма
  `4966` (`0/2314/511/0/0/2141` соответственно).
- [x] `unsupported/schema.unmapped` backlog содержит `152` упорядоченные
  группы по rule/package/classifier-kind/feature-kind и суммарно `4964`
  features, без имён объектов/файлов и UUID.
- [ ] Opaque status связан с provenance/placement.

## Задача 4: Подключить strict coverage gate

**Статус:** `[x]`

**Проверка:**
- [x] Полный join Xcore ↔ coverage не имеет unmapped keys.
- [x] Публичный parser до materialization ограничивает размер JSON, строк,
  entries, evidence sources и backlog; unknown/duplicate fields отклоняются.
- [x] Schema tests и governance CI проходят.

## Задача 5: Опубликовать coverage

**Статус:** `[x]`

**Проверка:**
- [x] GitHub #280 содержит totals по статусам и текущему покрытию; family
  aggregates теперь зафиксированы машинно: `typed=2`,
  `opaque-lossless=0`, `unsupported=4964`, `platform-only=0`.
