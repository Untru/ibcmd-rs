# Canonical DCS — план реализации

## Задача 1: Выделить DCS schema rules

**Статус:** `[x]`

- [x] QName/TypeId/order/delegates имеют verified или pending status.
- [x] Unsupported rule не становится runtime fallback.

## Задача 2: Добавить bounded canonical DCS IR

**Статус:** `[x]`

- [x] Settings/ListSettings typed minimum реализован.
- [x] Unknown extensions сохраняются opaque-lossless с placement.

## Задача 3: Реализовать единый serializer

**Статус:** `[ ]`

- [ ] Standalone settings и Form ListSettings используют один production-путь
  (bounded emitter готов, интеграция production callers остаётся отдельным slice).
- [x] Picture/color/type qualification не угадывается.
- [x] Physical wrapper QName и отсутствие TypeId подтверждены для EDT
  2025.2.3+30; opaque-placement явно unsupported (lossless placement отсутствует),
  поэтому emission с opaque facets fail-closed.

## Задача 4: Расширить schema/template

**Статус:** `[ ]`

- [ ] DCS schema/settings/template покрыты общим layer.

## Задача 5: Проверить evidence

**Статус:** `[ ]`

- [ ] EDT roundtrip fixtures проходят.
- [ ] IBCMD parity fixtures проходят.
- [ ] GitHub #283 получает evidence.
