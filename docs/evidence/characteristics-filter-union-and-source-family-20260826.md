# `<Characteristics>`: союз `TypesFilterValue` и семья маркеров стандартных реквизитов

База: `911e86e`. Стенд: `$D = /Users/untru/Documents/ChatGPT/ibcmd-stand`,
эталоны `$D/cap/<ключ>/src`, неизменяемые снимки `$D/baselines/911e86e/`.

## Опровергнутая гипотеза

Наблюдение, с которого начался участок, звучало так: «каждый объект
метаданных, чей XML несёт блок `<Characteristics>`, отсутствует в нашей
выгрузке целиком». Перепись по корпусу это **опровергает**.

`do` (Документооборот КОРП 3.0.21.3), объекты верхнего уровня, у которых
платформа вообще пишет свойство `Characteristics` (в любой форме):

```
BusinessProcesses            open       MISSING  10
Catalogs                     open       MISSING   1
Catalogs                     open       exact    23
Catalogs                     selfclose  MISSING   2
Catalogs                     selfclose  exact   252
ChartsOfCharacteristicTypes  selfclose  MISSING   1
ChartsOfCharacteristicTypes  selfclose  exact     2
Documents                    open       MISSING   1
Documents                    open       exact     3
Documents                    selfclose  MISSING   3
Documents                    selfclose  exact    25
Enums                        selfclose  exact   485
ExchangePlans                selfclose  exact    12
Tasks                        open       MISSING   1
всего объектов со свойством: 821, из них не exact: 19
```

802 из 821 объектов со свойством совпадают побайтово, включая 26 объектов
с **непустым** блоком. Значит, наличие блока само по себе ничего не решает.

На `uh` наблюдение опровергается ещё жёстче: все 20 `BusinessProcesses`
несут свойство, и все 20 — в одной и той же форме `<Characteristics/>`
(пустое, самозакрывающееся). 14 из них совпадают, 6 отсутствуют. Форма
свойства у совпадающих и у отсутствующих **одинаковая**, то есть на `uh`
блок `Characteristics` вообще не является различителем.

Функция `decode_characteristics` в `src/mssql_dump/mod.rs`, которую сборка
метит как неиспользуемую, — это тонкая обёртка над
`decode_characteristics_with_owner_code`, и вот она вызывается из трёх
разборщиков (`Catalog`, `Document`, `ChartOfCharacteristicTypes`).
Читатель подключён; неиспользуемой осталась только обёртка с умолчанием.

## Что на самом деле ломается: два члена союза `TypesFilterValue`

Перепись форм элемента `<xr:TypesFilterValue>` по восьми корпусам стенда,
с раскладкой по статусу паритета (снимок `911e86e`):

```
форма                 статус    вхождений
DesignTimeRef         MISSING          36
DesignTimeRef         exact            58
xsi:type="xs:boolean" MISSING           2
xsi:type="xs:boolean" diff              1
xsi:nil="true"        MISSING           8
xs:string             MISSING          20
xs:string             diff              9
xs:string             exact          1729
```

`xs:string` и `xr:DesignTimeRef` встречаются среди совпадающих файлов —
эти два члена союза разбираются. `xsi:nil="true"` и `xsi:type="xs:boolean"`
**ни разу** не встречаются среди совпадающих: 8 и 3 вхождения соответственно,
и все в отсутствующих или расходящихся файлах.

Обе формы записаны в корпусе ровно одним написанием:

```
8 вхождений:  <xr:TypesFilterValue xsi:nil="true"/>
3 вхождения:  <xr:TypesFilterValue xsi:type="xs:boolean">true</xr:TypesFilterValue>
```

Физическая сторона (реальные байты, `ibcmd-rs cf extract`):

| член союза | физическая запись | XML |
|---|---|---|
| строка | `{"S","Документ_ЕжедневныйОтчет"}` | `xsi:type="xs:string">…` |
| design-time ref | `{"#",<type-uuid>,{0,<owner>,<value>}}` | `xsi:type="xr:DesignTimeRef">…` |
| булево | `{"B",1}` | `xsi:type="xs:boolean">true` |
| без значения | `{"U"}` | `xsi:nil="true"/` (самозакрывающийся) |

Пары получены сведением физической записи объекта с XML платформы для того
же объекта, элемент к элементу, по всем 39 объектам `do` с непустым блоком
(скрипт переписи сводит `<xr:Characteristic>` XML с членами полезной
нагрузки слота по объявленному счётчику, не по позиции литерала).

Тег `"U"` не несёт полезной нагрузки: `fields.len() == 1`. Это **член
союза**, а не отсутствие свойства — платформа всё равно пишет элемент.

Тег `"B"` несёт один член, декодируемый общим для проекта физическим
булевым чтением (`information_register_bool`: `0`/`1`). В корпусе
наблюдается только `1`; `0` не запрещён, но и не встречен.

## Что ломается вторым: таблица маркеров стандартных реквизитов

Маркер вида `{1,{-8},0}` в поле характеристики называет стандартный
реквизит объекта, на который указывает ссылка. Прежний код выбирал таблицу
маркеров по семье **владельца** свойства:

```rust
let attributes = match owner_family {
    OwnerGraphFamily::Catalog => CATALOG_STANDARD_ATTRIBUTES.as_slice(),
    OwnerGraphFamily::ChartOfCharacteristicTypes => CCT_STANDARD_ATTRIBUTES.as_slice(),
    _ => [].as_slice(),
};
```

Реальные байты `BusinessProcess.Исполнение` (`do`, uuid
`679bd72a-4cf8-4c2e-ae00-3f01f48050c4`, слот 41) показывают, что одна
характеристика смешивает две таблицы:

```
 1 types_from       {1,b456d820-…}  → Catalog.НаборыДополнительныхРеквизитовИСведений.TabularSection.ДополнительныеРеквизиты
 2 values_from      {1,05cef2f3-…}  → BusinessProcess.Исполнение.TabularSection.ДополнительныеРеквизиты
 3 ObjectField      {1,{-5},0}      → BusinessProcess.…ДополнительныеРеквизиты.StandardAttribute.Ref
 6 TypesFilterField {1,{-8},0}      → Catalog.…ДополнительныеРеквизиты.StandardAttribute.Ref
```

`-8` и `-5` в одном элементе означают одно и то же — `Ref`, — потому что
указывают на объекты разных семей. Перепись сведения «физический маркер ↔
имя в XML платформы» по всем 39 объектам `do` с непустым блоком:

```
источник                     форма          маркер  суффикс в XML
BusinessProcess              TabularSection  -5     .StandardAttribute.Ref
Catalog                      Object          -4     .StandardAttribute.Parent
Catalog                      Object          -8     .StandardAttribute.Ref
Catalog                      TabularSection  -8     .StandardAttribute.Ref
Document                     TabularSection  -5     .StandardAttribute.Ref
Task                         TabularSection  -5     .StandardAttribute.Ref
```

Все шесть совпадают с уже существующими в дереве таблицами
`CATALOG_STANDARD_ATTRIBUTES` (`-8` Ref, `-4` Parent),
`DOCUMENT_STANDARD_ATTRIBUTES` (`-5` Ref),
`BUSINESS_PROCESS_STANDARD_ATTRIBUTES` (`-5` Ref),
`TASK_STANDARD_ATTRIBUTES` (`-5` Ref). Новая таблица не заводится: выбор
идёт по семье, объявленной в разрешённом пути ссылки.

Седьмая пара, `ChartOfCharacteristicTypes` `Object` `-2` → `.Ref`, взята с
`uh` (`Catalog.Должности`, uuid `66a0aa7f-f55e-4b51-9099-d10bdca7226d`,
элемент 3: `KeyField {1,{-2},0}` при
`from="ChartOfCharacteristicTypes.ДолжностиПодключаемыеХарактеристики"`,
XML пишет `.StandardAttribute.Ref`) и совпадает с
`CCT_STANDARD_ATTRIBUTES` (`-2` Ref). При прежнем ключе по владельцу
(`Catalog`) тот же `-2` дал бы `.StandardAttribute.Code` — не отказ,
а **неверный путь**.

XML-перепись `.StandardAttribute.` внутри `<Characteristics>` по всем
восьми корпусам показывает, что комбинация «источник — семья, отличная от
владельца» встречается только среди отсутствующих и расходящихся файлов:
`ChartOfCharacteristicTypes Object Ref` — 3 MISSING + 1 diff, ни одного
exact. Смена ключа поэтому не может испортить ни один уже совпадающий файл
и в переписи ничего, кроме `Ref` (и `Parent` для объектной формы каталога),
не встречается вовсе.

Правило, которое кодифицируется:

* словарь маркеров объявлен семьёй объекта, **на который указывает ссылка**;
* табличная часть открывает только `Ref` своего владельца; объектная форма
  открывает всю таблицу семьи;
* семья без собственной таблицы (например `InformationRegister`) —
  типизированный отказ, а не заимствование чужой нумерации.

Роль поля (`ObjectField`/`TypeField`/`KeyField`/…) в выборе не участвует:
прежняя привязка `role == ObjectField && marker == -5` для `Document` была
артефактом ключа по владельцу. Тест
`characteristics_document_schema_marker_is_closed_to_object_field`
кодифицировал именно этот артефакт и переписан в
`characteristics_standard_attribute_marker_follows_the_source_family`.

## Измерение

`do`, разность exact-множеств против `$D/baselines/911e86e/do.parity.json`:

```
base exact  : 25185
now  exact  : 25187
BROKEN      : 0
gained      : 2
   + Catalogs/КлассификаторЕдиницИзмерения.xml
   + Documents/ЕжедневныйОтчет.xml
extra files : 0
```

`ws` 29/29, `wms` 226/226, `mdm` 160/164 — BROKEN=0, лишних 0, прибавки 0
(ни один из трёх корпусов не содержит ни `xsi:nil`, ни `xs:boolean`, ни
разносемейного маркера).

`cargo test --lib`: 2328 passed / 33 failed, множество имён падающих
совпадает с `$D/baselines/911e86e/fail-base.txt` посимвольно (+1 тест
относительно 2327 — добавлен
`characteristics_filter_value_union_covers_undefined_and_boolean_members`).

## Что осталось открытым в этом же участке

`BusinessProcess` и `Task` с **непустым** слотом `Characteristics`
по-прежнему выбрасываются целиком, и это отдельный дефект — см.
`characteristics-business-process-task-20260826.md`.
