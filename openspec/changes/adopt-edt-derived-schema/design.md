# Дизайн: независимый EDT-derived model и writer corpus

## 1. Слои

```text
MSSQL / Config blobs
        |
        v
raw adapters and decoders
        |
        v
ibcmd-core canonical semantic model
        |
        +--------------------------+
        |                          |
        v                          v
ibcmd-schema                 opaque provenance
EDT-derived rules            for unknown fields
        |
        v
ibcmd-xml serializers
        |
        v
configuration source tree
```

Зависимости направлены только вниз:

- `ibcmd-core` не знает о SQL, XML или EDT;
- `ibcmd-schema` содержит декларативные производные знания и не зависит от EDT;
- `ibcmd-xml` использует `ibcmd-core` и `ibcmd-schema`;
- MSSQL/blob adapters преобразуют физические представления в `ibcmd-core`.

## 2. Состав corpus

### Model inventory

Версионированный JSON содержит:

- symbolic name и версию bundle;
- имена model types;
- имена XML importers;
- имена XML exporters;
- контрольные количества.

Из исходного исследовательского inventory удаляются абсолютные пути. JAR и
class-файлы в репозиторий не копируются.

### Writer rules

Каждое правило содержит:

- стабильный идентификатор;
- исходный EDT writer class;
- модельный тип и feature;
- последовательность XML-операций;
- условия default/version;
- делегируемый writer/serializer;
- provenance и состояние проверки.

Правило принимается только после анализа writer/reader либо трёхстороннего
эксперимента. Наблюдение одного IBCMD-файла недостаточно.

## 3. Независимость

Default build и все portable tests:

- не ищут EDT;
- не открывают JAR;
- не запускают Java/OSGi/native libraries;
- используют только committed JSON corpus.

Research importer является необязательным инструментом обновления. Его результат
должен быть детерминированным и проходить `ibcmd-schema` validation.

## 4. Первая вертикаль

Первая поставка включает:

1. полный очищенный inventory EDT 2025.2.3;
2. правила `FormChoiceListDesTimeValueWriter`;
3. правило делегирования `ListSettingsWriter`;
4. правило структурного копирования `SpreadsheetContentWriter`;
5. API загрузки, поиска и строгой проверки corpus;
6. wiring `ibcmd-xml -> ibcmd-schema`.

Это инфраструктурная вертикаль. Следующие изменения переводят реальные writers
на декларативные правила по одному семейству, не добавляя новые эвристики в
`form_body.rs`, `module_blob.rs` или `moxel.rs`.

## 5. Трёхсторонний oracle

Для одного исходного объекта сохраняются три результата:

- native IBCMD;
- EDT import -> export;
- ibcmd-rs.

Классификация:

- `IBCMD == EDT != ibcmd-rs`: ошибка schema/writer;
- `IBCMD != EDT`, но модель эквивалентна: лексическое правило IBCMD;
- оба эталона отличаются от ibcmd-rs семантически: decoder/model defect;
- EDT не принимает источник: отдельный platform-specific исследовательский пакет.

## 6. Ограничения для новых исправлений

- Нельзя добавлять XML-ветку без ссылки на corpus rule или полный
  counterexample corpus.
- SQL/blob parser не определяет порядок XML.
- XML writer не интерпретирует числовые raw slots.
- Реальные UUID/имена объектов не используются как production-условия.
- Процент меняется только после полного raw parity run.
