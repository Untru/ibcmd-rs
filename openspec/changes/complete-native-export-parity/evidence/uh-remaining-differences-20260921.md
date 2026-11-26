# ERP УХ 3.3.3.3: full export parity, 21.09.2026

Measured on branch `fix/8-3-27-parity` at `47313273`, against the native
8.3.27.2214 export of the same database
(`ibcmd_rs_uha_8327_parity2_20260920`, read-only):

| | files |
|---|---|
| byte-identical | 140 709 |
| different | 0 |
| native-only | 0 |
| candidate-only | 0 |

Run: `F:\ibcmd\lab\uha_collect_r20_20260921`
(`mssql-dump-config --extract-metadata-xml --extract-module-text
--no-binary-rows`, then `source-diff` against
`E:\ibcmd_lab\parity\ibcmd_rs_uha_8327_native_20260919_20260919_uha_8327_85head\native`).

At the start of 20.09.2026 the same comparison stood at 140 672 identical,
30 different and 7 missing; at the start of 21.09.2026 at 140 704 and 5.

## What the last five were

### 1. `AccumulationRegisters/ПланированиеПотребностей/Forms/ФормаСписка`

Eight nested data paths the platform marked and this export wrote plain:
`~Список.АналитикаИсточника.Этап`, `.ОбъектПланирования`, `.ДатаОперации`,
`.Цена`, and the same four under `АналитикаПланирования`. Both heads are
dimensions of the register typed `cfg:CatalogRef.КлючиАналитикиПланирования`,
and the four marked terminals are not fields of that catalogue while
`.АналитикаСтруктуры`, written plain beside them, is.

Closed by giving `MetadataFieldDeclarationIndex` the one reference owner each
data field declares, and letting a nested path resolve against it
(`ea42c152`).

### 2. `Documents/Лот/Forms/ВыигранныеЛоты`

Three data paths the platform marks, on a manual-query list with no main
table; two of them are also `<UseAlways>` fields, so five marks in all. Its
near-twin `Catalogs/Лоты/Forms/ВыигранныеЛоты` carries the same three paths,
the same stored field map (ids 1..5 named `Ref`, `ДоговорКонтрагента`,
`ПериодЗакупок`, `СпецификацияДоговора`, `Лот`) and almost the same query, and
is written plain -- so neither the stored map nor the form decides.

The difference is one extra union part whose join reads
`ПланированиеПотребностей.АналитикаПланирования.Этап` and
`…АналитикаПланирования.ОбъектПланирования.Объект` after
`Catalog.КлючиАналитикиПланирования` renamed both attributes to
`Удалить_Этап` and `Удалить_ОбъектПланирования`. The query no longer compiles,
so the platform builds no available-field list at all and marks every path
onto the list.

Two earlier hypotheses were tested against the corpus and both were disproved
before this one was found:

- *the source names an undeclared tabular section* -- it does not;
- *a union takes its result names from the last batch* -- 234 fields of this
  tree are named only by the first batch and the platform writes all of them
  plain.

Closed by `47313273`. Over all 2 229 manual-query dynamic lists of this export
that carry a data path, exactly one has such a dereference, all of its paths
are marked, and no other list is emptied by the rule.

### 3. `Catalogs/ШаблонЦепочкиПлатежей/Forms/ПомощникСозданияШаблонов`

One `<ExcludedCommand>SearchHistory</ExcludedCommand>` this export wrote on
`ТаблицаЦепочекПлатежей` that the platform does not. Both tables of that form
store the identical 19-uuid excluded list, so the list is not what decides.

The platform names an excluded command only where the item owns it. The search
history belongs to the search string: a table showing neither a search string
nor a search control does not own it, and a dynamic list owns it whatever the
table shows. Over all 3 140 tables of this export that carry a command set,
162 name the command and every one passes that test, 624 fail it and none
names it. Closed by `0dd002a2`.

### 4-5. Two MXL templates

`Catalogs/ШаблоныНазначенийПлатежей/Templates/МакетТегов` lost five `backColor`
values because the web-colour table had no name for codes 1 and 77
(`AliceBlue`, `LightSlateGray`); closed by `28ebe2f9`.

`DataProcessors/РасшифровкаРассчитанныхЗначений/Templates/МакетРасшифровкиУсловийОплаты`
stored one border whose line descriptor carries the nil kind with a zero width,
which the line table refused, dropping the whole template to the opaque path
and shifting every later style reference; closed by `bea419f1`.

## Known gap outside this corpus

A database left holding an *active* dynamic (online) generation still exports
the previous configuration for the objects that generation touched -- see
`openspec/changes/export-the-active-dynamic-generation`. ERP УХ carries two
*superseded* generations and no `DynamicallyUpdated` record, so it is not
affected; the BSP demo database, which an earlier load test of this project
left with an active generation, is.
