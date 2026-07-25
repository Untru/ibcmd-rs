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

## Задача 3: Разметить существующие canonical families

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/data/edt-2025.2.3-canonical-coverage.json`

**Проверка:**
- [ ] Metadata/forms/DCS/MXL имеют отдельные агрегаты.
- [ ] Opaque status связан с provenance/placement.

## Задача 4: Подключить strict coverage gate

**Статус:** `[ ]`

**Проверка:**
- [x] Полный join Xcore ↔ coverage не имеет unmapped keys.
- [x] Schema tests и governance CI проходят.

## Задача 5: Опубликовать coverage

**Статус:** `[ ]`

**Проверка:**
- [ ] GitHub #280 содержит totals по статусам и families.
