# Именованный diff свойств формы: 8.3.27 → 8.5.1 (UsualGroup)

Проверка того, что модель работает для адаптации: одно и то же свойство
сопоставлено с ID в двух версиях платформы, разница посчитана по именам, а не
по числам.

Источники: [id-name-8.3.27-UsualGroup.json](id-name-8.3.27-UsualGroup.json) и
[id-name-8.5.1-UsualGroup.json](id-name-8.5.1-UsualGroup.json).

## Добавлено в 8.5.1

| Свойство | XML | ID 8.5.1 |
|---|---|---|
| `COLLAPSED_REPRESENTATION_ITEM` | `collapsedRepresentationItem` | `0x27` |
| `CHILD_ITEMS_TITLE_LOCATION` | `childItemsTitleLocation` | `0x2b` |
| `SHOW_TITLE851` | `showTitle` | `0x30` |
| `SHOW_AS_CARD` | `showAsCard` | `0x36` |
| `BACK_PICTURE_EFFECT` | `backPictureEffect` | `0x37` |
| `CARD_REPRESENTATION_TYPE` | `cardRepresentationType` | `0x38` |
| `HYPERLINK` | `hyperlink` | `0x39` |
| `BACK_PICTURE` | `backPicture` | `0x3a` |
| `SCROLL_ON_COMPRESS` | `scrollOnCompress` | `0x3b` |

## Не удалено, а переименовано

`SHOW_TITLE` (8.3.27, ID `0x2c`) заменён на `SHOW_TITLE851` (8.5.1, ID `0x30`).
XML-имя в обоих случаях `showTitle`, поэтому для `ibcmd-rs` это не изменение
поведения, а смена внутреннего идентификатора в EDT.

## Вывод для адаптации

1. Сравнивать версии нужно по именам свойств, а не по ID: нумерация сдвигается.
2. Переименования вида `X` → `X851` не означают изменение XML.
3. Добавленные свойства (9 штук) — это ровно то, что надо поддержать в
   выгрузке при переходе на 8.5.

## Ограничение

Diff построен для `UsualGroup`; для `Table`, `Button`, `Field` списки EDT и
платформы пока не совпадают по длине (99/89 и 47/49), поэтому их имена ещё не
сопоставлены.
