# Schema-driven Form slice — план реализации

## Задача 1: Зафиксировать exact schema rules

**Статус:** `[ ]`

- [x] ChoiceList order/default имеют verified evidence.
- [ ] ChoiceList picture version gate подключён к target-version path (blocker: formatter не содержит picture/target-version boundary).
- [x] ListSettings delegate boundary имеет verified evidence.

## Задача 2: Добавить canonical Form values

**Статус:** `[x]`

- [x] Отсутствие, empty, typed и opaque различаются.
- [x] Raw provenance не содержит XML policy.

## Задача 3: Подключить schema-driven writer

**Статус:** `[ ]`

- [x] ChoiceList writer не использует raw slot для XML order.
- [ ] ListSettings сериализуется через DCS layer.
- [x] Pending/unsupported rule fail-closed.

## Задача 4: Удалить перекрытые эвристики

**Статус:** `[ ]`

- [ ] Удалены только ветки, покрытые новыми rule-level tests.

## Задача 5: Проверить cohort

**Статус:** `[ ]`

- [x] Rule-level fixtures побайтовы.
- [ ] Scoped УТ/БСП diff не регрессирует.
- [ ] GitHub #281 получает evidence.
