# Governance EDT corpus — план реализации

**Цель:** превратить clean-room/provenance policy в обязательный CI gate.

**Дизайн:** `openspec/changes/enforce-edt-corpus-governance/design.md`

## Задача 1: Реализовать repository validator

**Статус:** `[x]`

**Файлы:** `tools/validate-edt-corpus.ps1`

**Проверка:**
- [x] Запрещённые binaries/source formats отклоняются.
- [x] Absolute paths и file URI отклоняются.
- [x] Проверка детерминирована и не требует EDT.

## Задача 2: Добавить negative fixtures/tests

**Статус:** `[x]`

**Файлы:** `tests/fixtures/schema-governance/`, validator self-test

**Проверка:**
- [x] Есть path, binary, missing provenance и valid cases.

## Задача 3: Подключить CI

**Статус:** `[x]`

**Файлы:** `.github/workflows/offline-e2e.yml`

**Проверка:**
- [x] Gate запускается на Windows и Linux-compatible PowerShell.
- [x] Default CI не требует EDT/Java.

## Задача 4: Документировать policy

**Статус:** `[x]`

**Файлы:** `docs/architecture-edt-derived-schema.md`

**Проверка:**
- [x] Описаны разрешённые evidence и запрещённые материалы.
- [x] Один XML diff явно недостаточен.

## Задача 5: Финальная валидация

**Статус:** `[x]`

**Проверка:**
- [x] Validator, OpenSpec strict и offline checks проходят.
- [x] GitHub #279 получает evidence и закрывается после зелёного CI.
