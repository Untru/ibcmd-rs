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

**Статус:** `[x]`

- [x] Доказанный typed minimum (`itemsViewMode`, `itemsUserSettingID`) в
  standalone Settings и Form ListSettings строит один `DcsSettingsEnvelope` и
  использует один evidence-gated production emitter с явными source/target
  profiles; native DCS normalization больше не откатывается fail-open на
  необработанное three-document body.
- [x] Picture/color/type qualification не угадывается.
- [x] Physical wrapper QName и отсутствие TypeId подтверждены для EDT
  2025.2.3+30; opaque-placement явно unsupported (lossless placement отсутствует),
  поэтому emission с opaque facets fail-closed.

## Задача 4: Расширить schema/template

**Статус:** `[ ]`

- [ ] DCS schema/settings/template покрыты общим layer.

## Задача 5: Проверить evidence

**Статус:** `[ ]`

- [x] DCS body layout и raw -> native `Template.xml` byte parity подтверждены
  двумя изолированными roundtrip на 8.3.27.2214 / XML 2.20; micro-CF проходит
  production `cf export` offline.
- [x] Обратное тело production compiler принято и применено третьей чистой
  базой 8.3.27.2214; рекурсивная выгрузка всех пяти файлов отчёта побайтно
  совпадает с native round 2. Отличия лексики namespace внутри storage body не
  являются отдельным форматом или блокером реализации.
- [ ] EDT roundtrip fixtures проходят.
- [ ] IBCMD parity fixtures проходят.
- [x] GitHub #283 получает evidence.
