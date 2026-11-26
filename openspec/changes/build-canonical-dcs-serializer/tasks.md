# Canonical DCS — план реализации

## Задача 1: Выделить DCS schema rules

**Статус:** `[x]`

- [x] QName/TypeId/order/delegates имеют verified или pending status.
- [x] Unsupported rule не становится runtime fallback.

## Задача 2: Добавить bounded canonical DCS IR

**Статус:** `[ ]`

- [x] Settings/ListSettings typed minimum реализован.
- [x] Exact EDT evidence разделяет распознаваемые settings features и
  truly-unknown QName, для которых reader вызывает `throwWrongElement`.
- [ ] Profile-recognized, но ещё не типизированные ветви сохраняются
  source-owned с точным placement/provenance в production decoder.
- [ ] Ветви с positive opaque-lossless rule проходят same-profile replay;
  arbitrary unknown без такого rule остаётся fail-closed.

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

- [x] Physical schema/template envelope подтверждён для одного и двух прямых
  root `settingsVariant`: `u32@4` — число внешних `Settings`, далее идут
  `settings_count + 1` длин `u64`, а внешние documents связываются с variants
  позиционно и возвращаются inline в source XML.
- [x] Общий evidence-gated schema/template envelope API валидирует document
  roles/QName/BOM/empty-terminal, связывает внешние `Settings` с direct
  variants namespace-aware и обслуживает reverse compiler для доказанных
  одного-двух variants; доказанный production MSSQL route использует decoder
  ranges и не пересканирует plaintext по `<?xml`.
- [x] Source-to-native document construction для этого bounded cohort также
  принадлежит общему XML layer: compiler больше не владеет BOM/declaration,
  `SchemaFile`, root-case mapping или empty terminal document; получаемое тело
  совпадает с уже принятым pinned-платформой compiler artifact.
- [x] Первый platform-attested typed inner-schema cohort реализован общими
  `ibcmd-core` / `ibcmd-schema` / `ibcmd-xml`: Local data source,
  `DataSetObject`, доказанные simple (one string field) и rich
  (string/decimal fields, calculated field, два ungrouped `Sum` total,
  scalar string parameter) cohorts и один-два positional
  settings-variant shell. Native `SchemaFile` parse и canonical source emit
  проходят один codec; production MSSQL route больше не откатывается на
  plaintext/legacy Schema writer для этого cohort.
- [x] Первый exact `TypeId`/current-config reference coordinate реализован
  семантически: storage UUID разрешается в `CatalogRef.FilterProbe`, source
  QName строится общим XML codec, а собранный offline CF принят, применён,
  сохранён и повторно выгружен чистой базой 8.3.27.2214.
- [x] Exact bounded `DataSetQuery` + one-item `DataSetUnion` + direct
  `dataSetLink` cohort реализован одним semantic IR и namespace-aware XML
  codec. Два свежих платформенных roundtrip byte-stable; production compiler
  body загружен и применён свежей базой 8.3.27.2214, обратный `Template.xml`
  побайтно совпадает с native evidence.
- [ ] Полный typed schema API ещё не реализован: остальные Query/Union/link
  варианты, TypeId/type families и AreaTemplate требуют evidence-срезов;
  неподтверждённые schema shapes fail-closed.
- [ ] `defaultSettings`, nested variants, `AreaTemplate` и остальные schema
  branches остаются вне доказанного общего layer.
- [x] Физический style-free `AreaTemplate` cohort снят двумя свежими циклами
  8.3.27.2214: primary schema и внешний Settings остаются byte-identical
  `dcs-core`, а один TableRow/tableCell/Field + expression parameter хранится
  отдельным trailing `SchemaFile` без root appearance и `appIndex`. Production
  codec/reverse compiler для этого документа остаются следующим подэтапом.

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
