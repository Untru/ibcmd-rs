# ERP УХ 3.3.3.3: what still differs, 21.09.2026

Measured on branch `fix/8-3-27-parity` at `89dc2f2f`, against the native
8.3.27.2214 export of the same database
(`ibcmd_rs_uha_8327_parity2_20260920`, read-only):

| | files |
|---|---|
| byte-identical | 140 704 |
| different | 5 |
| missing | 0 |

At the start of 20.09.2026 the same comparison stood at 140 672 identical,
30 different and 7 missing.

## The five

### 1. `AccumulationRegisters/ПланированиеПотребностей/Forms/ФормаСписка`

Eight nested data paths the platform marks and this export writes plain:
`~Список.АналитикаИсточника.Этап`, `.ОбъектПланирования`, `.ДатаОперации`,
`.Цена`, and the same four under `АналитикаПланирования`. Both heads are
dimensions of the register typed `cfg:CatalogRef.КлючиАналитикиПланирования`,
and the four marked terminals are not fields of that catalogue while
`.АналитикаСтруктуры`, written plain beside them, is.

`form_dynamic_list_nested_field_is_declared` answers a nested path only when
its head is the main table's standard `Ref` or one of its tabular sections;
neither covers a dimension whose declared type is a single reference. Closing
it needs a field-to-type index the declaration index does not carry today.

### 2. `Documents/Лот/Forms/ВыигранныеЛоты`

Three data paths the platform marks, on a manual-query list with no main
table. The query is a three-batch union; every table, field and `ЗНАЧЕНИЕ(…)`
literal it names is declared, the tabular-section source
`Документ.ВерсияСоглашенияКоммерческийДоговор.Номенклатура` is a declared
section of a declared document, and the register fields it selects are
declared too. Two hypotheses were tested against the corpus and both are
disproved:

- *the source names an undeclared tabular section* — it does not;
- *a union whose last batch does not alias its columns resolves nothing* — 49
  of that tree's 72 union-query lists have an unaliased last batch and the
  platform writes all of them plain.

Nothing measured so far separates this list from those 49.

### 3. `Catalogs/ШаблонЦепочкиПлатежей/Forms/ПомощникСозданияШаблонов`

One `<ExcludedCommand>SearchHistory</ExcludedCommand>` this export writes on
`ТаблицаЦепочекПлатежей` that the platform does not. Both tables of that form
store the identical 19-uuid excluded list, and the platform writes the command
for one and not for the other, so the difference is not in the list. A
cross-tabulation of `SearchStringLocation` and `SearchControlLocation` against
the exclusion over the whole tree separates nothing: `None`/`None` appears 751
times without the exclusion and 16 times with it.

### 4–5. Two MXL templates

`Catalogs/ШаблоныНазначенийПлатежей/Templates/МакетТегов` and
`DataProcessors/РасшифровкаРассчитанныхЗначений/Templates/
МакетРасшифровкиУсловийОплаты`.

The first: five `<format>` records the platform gives a `backColor`
(`style:ButtonBackColor`, `web:HoneyDew` twice, `web:LightSlateGray`,
`style:FieldSelectionBackColor`, and one with `textColor` beside it) come out
empty, and one the platform writes `style:FormBackColor` comes out
`style:ToolTipBackColor` — the pair
`normalize_moxel_drawing_format_with_pattern_color` rewrites for a format a
*drawing* names, which suggests those records are classified as notes.

The second: 17 `leftBorder`, 13 `backColor` and 3 `bottomBorder` differences
plus one `v8ui:style`, all inside one drawing's format run.
