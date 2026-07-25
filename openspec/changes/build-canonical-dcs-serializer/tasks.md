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

- [ ] Standalone settings и Form ListSettings используют один путь.
- [x] Picture/color/type qualification не угадывается.
- [ ] Physical wrapper QName, TypeId and opaque-placement evidence are still
  incomplete; `edt-2025.2.3-dcs-writer-evidence.json` records the four exact
  missing keys, so byte emission remains fail-closed.

## Задача 4: Расширить schema/template

**Статус:** `[ ]`

- [ ] DCS schema/settings/template покрыты общим layer.

## Задача 5: Проверить evidence

**Статус:** `[ ]`

- [ ] EDT roundtrip fixtures проходят.
- [ ] IBCMD parity fixtures проходят.
- [ ] GitHub #283 получает evidence.
