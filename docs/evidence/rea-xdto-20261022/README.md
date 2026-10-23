# XDTO-модель форм: ID = позиция свойства

Дата: 22.10.2026. Ветка: `feat/rea-form-property-inventory-20261022`.

## Источник истины

Пакет `http://v8.1c.ru/8.3/xcf/logform` лежит **открытым XML** внутри
`mngbase_root.res` платформы. Это не догадка и не EDT: та же модель, которую
использует конфигуратор.

Модель — 66 типов в 8.3.x и 68 в 8.5.1; у каждого типа есть `base` и список
`property` в порядке объявления.

## Правило ID

ID атрибута в писателе XML — **0-based позиция свойства в развёрнутой цепочке
наследования**, начиная с корня:

```
PredefinedChildItems (9)
  → FormVisualEntity (2)
    → FormItemBase (id, name, DisplayImportance)
      → GroupBase (…)
        → UsualGroup (30)
```

Проверка на живых данных:

| ID | Свойство | Владелец | Умолчание |
|---|---|---|---|
| `0xb` | `id` | `FormItemBase` | — |
| `0xc` | `name` | `FormItemBase` | — |
| `0xd` | `DisplayImportance` | `FormItemBase` | — |
| `0x1f` | `Group` | `UsualGroup` | `Vertical` |
| `0x20` | `ChildrenAlign` | `UsualGroup` | `Auto` |
| `0x25` | `Behavior` | `UsualGroup` | `Auto` |

Это ровно те значения, которые пишет нативный писатель: у `UsualGroup`
последовательность начинается `0x1f, 0x20, 0x21…`, у `Table` — `0xc, 0xb, 0xd,
0xe…`.

## Проверка

`tools/rea/xdto_model.py` извлекает модель из `.res`, а команда `verify`
сверяет с ней нативные последовательности.

Для 8.5.1.1529 все ID всех десяти элементов нашлись в модели: `UsualGroup` 30,
`Table` 89, `Button` 49, `Field` 55, `Form` 38, `Pages` 3, `Page` 16,
`CommandBar` 3, `ContextMenu` 1, `ColumnGroup` 12.

Модели выгружены для шести платформ: 8.3.21.1895, 8.3.24.1819, 8.3.27.1989,
8.3.27.2214, 8.5.1.1150, 8.5.1.1529.

## Почему карта из EDT была неверна

EDT-плагин `form.export.xml` задаёт **свой** порядок свойств, а не порядок
XDTO. Сверка с моделью:

| Элемент | Расхождений |
|---|---|
| `UsualGroup` | 13 из 30 |
| `Table` | 89 из 89 |

Поэтому карты `id-name-*-UsualGroup.json` и `id-name-8.5.1-Table.json` в
`docs/evidence/rea-form-writers-20261022/` считаются устаревшими: для имён
используется только XDTO.

## Именованные изменения 8.3.27.2214 → 8.5.1.1529

| Тип | Добавлено | Удалено | Изменены умолчания |
|---|---|---|---|
| UsualGroup | CollapsedRepresentationItem, ChildItemsTitleLocation, ShowAsCard, BackPictureEffect, CardRepresentationType, Hyperlink, BackPicture, ScrollOnCompress | — | ShowTitle |
| Table | HorizontalLinesBWA, VerticalLinesBWA, UseAlternationRowColorBWA, MarkRequiredComplete, InitialRowActivation, RowActionsShowType, HierarchyPanelLocation, SaveColors, IconSaveColorsKey, ShapeSaveColorsKey, ShowCommandBar, MaxCardHeight, CardBehaviorOnVerticalCompression, CellHyperlinksRepresentation | HorizontalLines, VerticalLines, UseAlternationRowColor | — |
| Button | ShowAsCard, BackPicture, BackPictureEffect, CardPictureAndTitleAlign, PictureHeight, ButtonImportance | — | — |
| Field | ShowInCard, FixInCard, MarkRequiredComplete, CellMark, AutoEditMode, AppearanceInCard, ShowTitleInCard, WidthInCard, AutoWidthInTable, CellHyperlinkRepresentation, CellHyperlinkDisplayVariant | — | — |
| Form | Title, ChildItemsTitleLocation, ScaleVariant, CreateButtonsGroupTitle, CreateButtonsGroupPicture, WindowViewMode, ShowCommandBar | — | WindowOpeningMode, ShowTitle |
| Page | ChildItemsTitleLocation | — | ShowTitle, ScrollOnCompress |
| CommandBar | AppearanceMode | — | — |

Отдельно: в 8.5 писатели `Button` и `Table` стали писать `id` и `name` тем же
циклом, что и остальные свойства (в 8.3 их писал базовый код).

## Где XDTO не применяется: слоты блоба

XDTO задаёт **идентичность и порядок XML-атрибутов**, но не раскладку
внутреннего блоба формы. Проверка на известных парах «слот → свойство» из
`src/form_schema.rs`:

| Свойство | XDTO-ID (8.5.1) | Слот блоба в коде |
|---|---|---|
| `TitleBackColor` (LabelField) | `0x17` (23) | 33 (`TITLE_BACK_COLOR_BASE_SLOT`) |
| `Group` (UsualGroup) | `0x1f` (31) | 1 (в 29-member bag) |
| `ShowTitle` (UsualGroup) | `0x2f`/`0x30` | 4 |

Совпадений нет: блоб — отдельная нумерация (вероятно, унаследованная от
8.2-схемы, найденной в `mngcore_root.res` как XSD `managed-application/logform`).
Поэтому заменять слоты в `form_body.rs` на XDTO-ID нельзя — это сломает
разбор. XDTO-таблицы применяются там, где формируется XML: имена атрибутов,
их порядок и умолчания.

## Контракт порядка для писателя XML

Писатель XML живёт не в `form_body.rs` (тот разбирает блоб), а в
`src/compiler/bodies/form_native.rs`. Порядок атрибутов там задан кодом; XDTO
даёт имена для ID, а нативные последовательности — сам порядок.

Собранный контракт: [attribute-order-8.5.1.json](attribute-order-8.5.1.json) —
для десяти элементов список `{id, name, default}` в порядке, в котором пишет
платформа. Например:

```
Table:      0xc name, 0xb id, 0xd DisplayImportance, 0xe Representation, 0xf Visible, …
UsualGroup: 0x1f Group, 0x20 ChildrenAlign, 0x21 HorizontalSpacing, …
```

Его и должен потреблять `form_native.rs`: имя атрибута и умолчание берутся из
XDTO, порядок — из контракта.

## Проверка на реальном корпусе

Контракт проверен на выгрузке `D:\EDT\artifacts\OneConta85-xml` (формат 2.21,
8.5.1): 40 508 элементов формы, у каждого сверена последовательность дочерних
свойств.

| Элемент | Проверено | Нарушений |
|---|---|---|
| ContextMenu | 19 282 | 0 |
| Button | 8 554 | 0 |
| UsualGroup | 4 335 | 0 |
| Field | 2 485 | 0 |
| Form | 1 390 | 0 |
| Page | 1 296 | 4 |
| Table | 1 181 | 0 |
| ColumnGroup | 1 015 | 0 |
| CommandBar | 539 | 0 |
| Pages | 431 | 0 |

Итог: **40 504 из 40 508** совпали. Первый прогон дал 428 нарушений; они
свелись к трём правилам, которые контракт не описывал: `ShowTitle` и
`ShowCloseButton` у формы идут раньше `CommandSet` и
`MobileDeviceCommandBarContent`, а у группы `CurrentRowUse` — раньше
`ThroughAlign`. После правки остались 4 случая `Page` — и они объясняются
блоком заголовка: если у страницы есть `Title`, то `TitleTextColor` и
`TitleFont` пишутся сразу за ним, до `Group`, а не в конце последовательности
свойств. Это отдельный блок, а не нарушение порядка, поэтому контракт его не
описывает.

## Воспроизведение

```powershell
python tools/rea/xdto_model.py extract `
  --res "C:/Program Files/1cv8/8.5.1.1529/bin/mngbase_root.res" `
  --out docs/evidence/rea-xdto-20261022/8.5.1.1529.json

python tools/rea/xdto_model.py verify `
  --xdto docs/evidence/rea-xdto-20261022/8.5.1.1529.json `
  --native docs/evidence/rea-form-writers-20261022/8.5.1.1529.json

python tools/rea/xdto_model.py map `
  --xdto docs/evidence/rea-xdto-20261022/8.5.1.1529.json `
  --native docs/evidence/rea-form-writers-20261022/8.5.1.1529.json `
  --out docs/evidence/rea-xdto-20261022/8.5.1.1529-mapped.json
```

## Что лежит в репозитории

Здесь только входы генераторов (`8.5.1.1529.json` и `8.3.27.2214.json` для
`tools/rea/gen_xdto_rust.py`, `attribute-order-8.5.1.json` для
`tools/rea/gen_attribute_order_rust.py`) и выводы. Полные дампы 8.3.21.1895,
8.3.24.1819, 8.3.27.1989 и 8.5.1.1150 и их `-mapped` версии не хранятся:
`tools/rea/xdto_model.py` извлекает их заново из установленной платформы.
