# Dynamic-list `~`: one claim per name, and a query that cannot compile, 20260826

Status: two more residues of the `~` class closed, on top of
[`form-dynamic-list-declared-fields-20260826.md`](form-dynamic-list-declared-fields-20260826.md).
Base `7bc966c`, measured over every dynamic-list record `cf export` walks on the
stand ($D = `/Users/untru/Documents/ChatGPT/ibcmd-stand`), all eight
configurations.

## One name, one claim

A dynamic list's field map can remember the same name under two different item
ids. `Catalogs/Лоты/Forms/ФормаСписка` and `Documents/Лот/Forms/ФормаСписка`
(ERP УХ 3.2.12.6) both remember

```
FieldsMapItemId17 {"N",534}  FieldsMapItemName17 {"S","Ref"}          SecondaryName17 {"S","Ссылка"}
FieldsMapItemId20 {"N",645}  FieldsMapItemName20 {"S","DeletionMark"} SecondaryName20 {"S","ПометкаУдаления"}
FieldsMapItemId22 {"N",756}  FieldsMapItemName22 {"S","Ref"}          SecondaryName22 {"S","Ссылка"}
FieldsMapItemId23 {"N",807}  FieldsMapItemName23 {"S","DeletionMark"} SecondaryName23 {"S","ПометкаУдаления"}
```

and require all four ids. The native `<UseAlways>` of `Catalogs/Лоты` is

```
~Список.DeletionMark~Список.ПометкаУдаления
~Список.Ref~Список.Ссылка
~Список.Статус
Список.DeletionMark
Список.Owner
Список.Ref
Список.МетодОценкиПредложенийПоставщиков
Список.СтрокаПланаЗакупок
```

— the plain spelling and the doubled marked spelling of the same name side by
side. The list's available-field collection holds one entry per name: the first
remembered id binds to it, and a later id remembering the same name binds to
nothing, so the platform marks it. The reader deduplicated the two identical
outputs and wrote one entry.

Census over the whole stand: of 7 728 dynamic lists, six remember a name under
two ids, and exactly these two require both. The other four write one entry and
are untouched. `Documents/Лот` differs from `Catalogs/Лоты` only in
`~Список.Owner~Список.Владелец` — a document family has no `Owner` standard
attribute, which the twin reader already handled.

The order read is the map's own suffix numbering. In both observations the map
is stored with ascending item ids, so suffix order and id order name the same
first entry; nothing here distinguishes the two readings.

## A query that names metadata the configuration does not declare

`Documents/ЗаявкаНаИзменениеНСИ/Forms/ФормаСписка` and `.../ФормаВыбора` of ERP
УХ MDM_Management 3.2.12.6 carry a manual query with
`AutoFillAvailableFields=1` whose result columns are exactly the eight names
their field maps remember — and the platform marks every one of them on its item
`<DataPath>`. The query selects

```
ВЫБОР КОГДА СоответствиеЗаявокНаИзменениеНСИ.Состояние ЕСТЬ NULL
        ТОГДА ЗНАЧЕНИЕ(Перечисление.СостоянияСогласования.Черновик)
      ИНАЧЕ СоответствиеЗаявокНаИзменениеНСИ.Состояние
КОНЕЦ КАК СостояниеОбъекта
```

while the configuration declares `Enum.СостоянияСогласованияНСИ` and no
`СостоянияСогласования` at all. The query cannot compile, so the list has no
available fields and every remembered field is unresolvable.

Census: over the stand, 2 226 ERP УХ, 1 025 UT, 497 Документооборот, 173 БСП
demo and 148 БСП base manual-query lists name metadata in a `ЗНАЧЕНИЕ(...)`
literal; these two are the only ones naming something the configuration does not
declare. Ten lists look like counter-examples on a case-sensitive reading —
`ЗНАЧЕНИЕ(Перечисление.СтатусызаданийТорговымПредставителям…)` against the
declared `СтатусыЗаданийТорговымПредставителям` — and the platform writes every
one of their fields plain, because the query language names metadata
case-insensitively. The index folds both sides to lower case for exactly that
reason.

Only the `ЗНАЧЕНИЕ(...)` literal is read. A `FROM` source naming undeclared
metadata is the same failure in principle and has no observation anywhere on the
stand, so it is left alone rather than guessed at.

## Result

Exact-set difference against `$D/baselines/7bc966c/<key>.parity.json`, for the
three packages together (declared fields, one claim per name, undeclared
metadata):

| key       |    было |    стало | прибавилось | сломано | лишних |
|-----------|--------:|---------:|------------:|--------:|-------:|
| `ws`      |      29 |       29 |           0 |       0 |      0 |
| `wms`     |     226 |      226 |           0 |       0 |      0 |
| `mdm`     |     160 |      162 |           2 |       0 |      0 |
| `sslbase` |   9 617 |    9 617 |           0 |       0 |      0 |
| `ssl`     |  12 697 |   12 697 |           0 |       0 |      0 |
| `do`      |  25 322 |   25 323 |           1 |       0 |      0 |
| `ut`      |  50 898 |   50 898 |           0 |       0 |      0 |
| `uh`      | 139 107 |  139 118 |          11 |       0 |      0 |

`нет файла` unchanged on every key.
