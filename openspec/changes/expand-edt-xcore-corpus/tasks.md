# Полный Xcore corpus — план реализации

**Цель:** детерминированно покрыть все model Xcore resources EDT 2025.2.3.

**Дизайн:** `openspec/changes/expand-edt-xcore-corpus/design.md`

## Задача 1: Добавить inventory discovery report

**Статус:** `[x]`

**Файлы:** `tools/import-edt-xcore-semantics.ps1`

**Проверка:**
- [x] Все selected resources учитываются processed/rejected totals.
- [x] Reject содержит portable resource, production и причину.

## Задача 2: Расширить подтверждённую Xcore grammar

**Статус:** `[x]`

**Файлы:** `tools/import-edt-xcore-semantics.ps1`

**Проверка:**
- [x] Все новые productions подтверждены реальным Xcore counterexample.
- [x] Неизвестный синтаксис по-прежнему fail-closed.
- [x] Operation bodies и comments не становятся features.

## Задача 3: Расширить Rust schema при необходимости

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/src/lib.rs`

**Проверка:**
- [x] Все новые kinds/qualifiers типизированы.
- [x] Старый form corpus остаётся совместимым.

## Задача 4: Сгенерировать all-model corpus

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/data/edt-2025.2.3-feature-semantics.json`

**Проверка:**
- [x] Нет молчаливо пропущенных selected resources.
- [x] Повторная генерация даёт одинаковый SHA-256.
- [x] Нет Xcore/JAR/class bytes и absolute paths.

## Задача 5: Подключить totals и representative tests

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/src/lib.rs`

**Проверка:**
- [x] Проверяются Forms, DCS, common и binary model packages; модели без
  packaged Xcore остаются в EPackage inventory и не считаются Xcore-покрытыми.
- [x] `cargo test -p ibcmd-schema --locked` проходит.

## Задача 6: Финальная валидация

**Статус:** `[ ]`

**Проверка:**
- [ ] OpenSpec strict, fmt, clippy и schema/xml tests проходят.
- [ ] GitHub #278 получает totals и evidence.
