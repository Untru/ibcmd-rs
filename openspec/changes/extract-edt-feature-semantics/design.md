# Дизайн: Xcore-derived feature semantics

## Источник

Research importer получает inventory с абсолютными путями к локальным EDT JAR.
Из каждого model bundle он перечисляет `model/*.xcore`, читает содержимое через
архивный инструмент и извлекает только декларативную семантику.

Исходные Xcore и JAR не сохраняются в проекте.

## Модель corpus

Каждая запись идентифицируется тройкой:

```text
package namespace URI / classifier / feature
```

Запись содержит:

- Xcore feature kind: attribute, reference или containment;
- model type;
- lower/upper cardinality;
- default value, если явно задан;
- значимые Xcore qualifiers (`container`, `transient`, `unsettable`, `unique`);
- XML QName, order, default-emission, version gate и delegate;
- evidence для каждой группы сведений.

Поля XML behaviour допускают `null` только вместе со статусом `pending`.
`pending`-данные не могут использоваться production writer как подтверждённое
правило.

## Детерминизм

- bundles, Xcore resources, classifiers и features сортируются;
- JSON пишется с фиксированной глубиной и UTF-8 без BOM;
- абсолютные пути и тексты Xcore не попадают в результат;
- повторный импорт одного release должен быть побайтово идентичен.

Importer работает fail-closed: пропускает комментарии, annotations и тела
operations с учётом вложенных скобок, но отклоняет неизвестный classifier,
feature qualifier или multiplicity. Он не разворачивает inheritance и фиксирует
только локально объявленные features.

## Первый вертикальный срез

Первый инкремент извлекает form model Xcore, потому что он одновременно содержит
attributes, references, containment, списки и explicit defaults. Это проверяет
модель corpus до масштабирования на все packages.

## Граница runtime

`ibcmd-schema` включает JSON через `include_str!`. Cargo build/test не запускают
Java, EDT или research importer и не открывают JAR.
