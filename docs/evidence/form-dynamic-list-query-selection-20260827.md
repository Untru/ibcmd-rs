# `<UseAlways>`: what the query itself says its fields are, 20260827

Base: `9b58354`. Snapshots: `$D/baselines/9b58354/`
($D = `/Users/untru/Documents/ChatGPT/ibcmd-stand`). Own export reproduced the
`uh` snapshot byte for byte before any change (exact 139 626 / differing 723 /
missing 62 / extra 0), and every other key likewise.

This closes the last named class of the `~` marker on `<Attribute>` /
`<Field>` — the block a dynamic-list attribute writes as `<UseAlways>`.

## 1. Method

Two joined censuses, both over the whole corpus, never a sample.

* **Platform side.** Every `<Attribute name=…>` of every `Ext/Form.xml` of the
  eight native trees, with the `<Field>` list of the `<UseAlways>` block that
  belongs to it. 4 166 blocks on `uh`, 1 766 on `ut`, 533 on `do`, 280 on
  `ssl`, 206 on `sslbase`, 2 on `mdm`, 2 on `wms`, none on `ws`.
* **Reader side.** A temporary probe in
  `parse_form_attribute_with_dcs_type_index` appended, per dynamic-list
  attribute with a required-field list, the form's own source path, the field
  map (ids, names, localized twins), the required and shadowed ids, the parsed
  query selection (aliases, `*` items with their qualifier, FROM sources), the
  computed resolvable-field universe and the emitted `<UseAlways>` list. The
  probe is not part of this package.

Joining the two on `(form path, attribute name)` gives one row per dynamic-list
record the export walks, with the platform's own answer beside the reader's.
The remaining native blocks — 1 305 on `uh`, 402 on `ut`, 78 on `do` — belong
to attributes that are not dynamic lists and are written by
`parse_form_attribute_direct_use_always`, a different code path this package
does not touch.

## 2. The class at the base

| key | blocks | `<Field>` entries | `~`-marked | joined records | agree | disagree |
|---|---:|---:|---:|---:|---:|---:|
| `uh` | 4 166 | 12 918 | 1 438 | 2 861 | 2 786 | **75** |
| `ut` | 1 766 | 5 392 | 167 | 1 364 | 1 364 | 0 |
| `do` | 533 | 2 336 | 58 | 455 | 455 | 0 |
| `ssl` | 280 | 766 | 14 | 229 | 229 | 0 |
| `sslbase` | 206 | 667 | 14 | 171 | 171 | 0 |
| `mdm` | 2 | 2 | 0 | 1 | 1 | 0 |
| `wms` | 2 | 2 | 0 | 1 | 1 | 0 |
| `ws` | 0 | 0 | 0 | 0 | 0 | 0 |

The 75 disagreeing blocks live on 35 ERP УХ form bodies, and 34 of those files
differ from native in nothing but `<Field>` lines. Every one of the 75 is the
same direction: the platform marks a field, the reader writes it plain. That is
what a **refused universe** produces — with no universe no marker is ever
added — so the whole class is the reader declining to say what the list's
fields are, in three distinct places.

## 3. Rule 1: a `*` names the fields of the source it is qualified with

`form_dynamic_list_use_always_universe` refused outright when the final
selection carried a `*`. ERP УХ 3.2.12.6 disagrees on 71 of those blocks, all
on regulated-report forms whose list reads

```
ВЫБРАТЬ
	РегистрСведений.*,
	&ПустаяКартинкаСтрок КАК ПустаяКартинка
ИЗ
	РегистрСведений.СведенияРеглОтчетАлкоПрил25Раздел2Поступления КАК РегистрСведений
ГДЕ РегистрСведений.ИДДокИндСтраницы = &ИДДокИндСтраницы
```

`<alias>.*` names the source declared under that alias, so its result columns
are that table's own fields. Reading them off the same two declarations the
rest of the decoder already reads — the family's standard attributes filtered
by what the table declares (`form_dynamic_list_declared_std_attribute_pairs`,
Russian spelling only, the rule
`form_dynamic_list_main_table_auto_fields` already states for a manual query)
plus the table's declared top-level children — reproduces the platform's
markers exactly.

`Reports/РегламентированныйОтчетАлкоПриложение25/Forms/ФормаОтчета2021Кв1`
carries four such lists and settles the rule on its own:

* the `Раздел2Поступления` list writes `П000020000301`…`П000020000313`,
  `Активно`, `Документ` and `ИндексСтроки` plain — the register declares every
  one — and marks `ИндексСтраницы` (the register spells that field
  `ИДДокИндСтраницы`) together with `П000020000300`, `П000020000314`,
  `П000020000315`, `П000020000316`, `П000020000391` and `П000020000392`, none
  of which it declares;
* the `Раздел3Возвраты` list remembers the *`Раздел2`* names — the map was
  filled when the list pointed at the other register — and the platform marks
  every single one of them while leaving `Активно`, `Документ` and
  `ИндексСтроки` plain.

### Census

The reader's own parse splits the 192 manual-query dynamic lists of the stand
whose final selection carries a star into four shapes:

| shape | records | lists the platform marks a field on |
|---|---:|---:|
| `<alias>.*` over a base table | 71 (`uh`) | **71** |
| `<alias>.<section>.*` | 111 (`uh` 56, `ut` 48, `do` 7) | 0 |
| qualifier names no FROM source of that select | 9 (`uh` 2, `ut` 2, `do` 1, `ssl` 2, `sslbase` 2) | 0 |
| bare `*` | 1 (`uh`) | 0 |

Only the first shape is read. The other three keep the refusal, which writes
every remembered field plain — exactly what the platform writes for all 121 of
them. A refusal here refuses the whole universe, as the unconditional refusal
it replaces did, and it also covers a source that is a subquery or a temporary
table, a virtual table (whose columns are not the base table's), a table the
object-reference index does not carry and a family whose standard attributes
this reader cannot name.

## 4. Rule 2: a batch that declares no selection is not the final selection

The final selection was taken to be the last batch not stored into a temporary
table. A query that builds temporary tables ends by dropping them, so its last
batch is `УНИЧТОЖИТЬ <таблица>`; reading that as the selection found no
`ВЫБРАТЬ` in it and refused the whole universe.

A batch is the selection when it declares one — a `ВЫБРАТЬ` at the batch's own
nesting level. Two dynamic lists on the whole stand end this way, and both are
settled by the change:

* `DataProcessors/ДокументооборотСКонтролирующимиОрганами/Forms/ОтветыНаТребованияФСС`
  ends `УНИЧТОЖИТЬ ВТСтатус; УНИЧТОЖИТЬ ВТОтветы`. Its real final selection
  names `Ссылка` and no `СостояниеСдачиОтчетности`, and the platform writes
  `<Field>~Ответы.СостояниеСдачиОтчетности</Field>` beside a plain
  `<Field>Ответы.Ссылка</Field>` — the two required fields of that list.
* `Documents/ЗаявлениеОВвозеТоваровПолученное/Forms/ФормаРабочееМесто` ends
  `УНИЧТОЖИТЬ КОформлению`; its final selection names all six remembered
  fields, and the platform writes all six plain.

## 5. Rule 3: the selection list ends at the first clause the batch declares

The selection list was collected up to `ИЗ` or `ПОМЕСТИТЬ` alone. A batch that
declares no source at all never reaches either, so the clause body was read as
further selection items and the item the clause keyword sits behind lost its
alias.

`Documents/ЗаказПереработчику/Forms/РабочееМесто` and
`Documents/ЗаказПереработчику2_5/Forms/РабочееМесто` build their list from a
design-time placeholder query that selects literals and ends

```
	ЛОЖЬ КАК ДинамическаяСтруктура
ГДЕ
	ЛОЖЬ
```

The last item reached the alias reader as
`ЛОЖЬ КАК ДинамическаяСтруктура ГДЕ ЛОЖЬ`, which names nothing, so
`ДинамическаяСтруктура` was missing from the universe and the reader marked a
field the platform writes plain — the one over-mark in the whole class.

Ending the list at the first clause keyword the batch declares at its own
nesting level (`Q_CLAUSE_END`, beside `ИЗ` and `ПОМЕСТИТЬ`) is the whole
change. Census of which keyword a final selection batch reaches first:

| key | source first | clause first | neither |
|---|---:|---:|---:|
| `uh` | 1 739 | **2** | 36 |
| `ut` | 821 | 0 | 20 |
| `do` | 323 | 0 | 7 |
| `ssl` | 136 | 0 | 6 |
| `sslbase` | 111 | 0 | 6 |

Only the two named records reach a clause before a source; a batch that
declares neither is read to its end as before.

## 6. Result

Agreement of the emitted `<UseAlways>` block with the platform's, per corpus:

| key | joined records | agree, base | agree, now |
|---|---:|---:|---:|
| `uh` | 2 861 | 2 786 | **2 860** |
| `ut` | 1 364 | 1 364 | 1 364 |
| `do` | 455 | 455 | 455 |
| `ssl` | 229 | 229 | 229 |
| `sslbase` | 171 | 171 | 171 |
| `mdm` | 1 | 1 | 1 |
| `wms` | 1 | 1 | 1 |

Exact-set difference against `$D/baselines/9b58354/<key>.parity.json`:

| key | было | стало | прибавилось | сломано | лишних | нет файла |
|---|---:|---:|---:|---:|---:|---:|
| `ws` | 29 | 29 | 0 | 0 | 0 | 0 → 0 |
| `wms` | 226 | 226 | 0 | 0 | 0 | 0 → 0 |
| `mdm` | 164 | 164 | 0 | 0 | 0 | 0 → 0 |
| `sslbase` | 9 617 | 9 617 | 0 | 0 | 0 | 0 → 0 |
| `ssl` | 12 701 | 12 701 | 0 | 0 | 0 | 0 → 0 |
| `do` | 25 373 | 25 373 | 0 | 0 | 0 | 0 → 0 |
| `ut` | 50 898 | 50 898 | 0 | 0 | 0 | 0 → 0 |
| `uh` | 139 626 | 139 660 | **+34** | 0 | 0 | 62 → 62 |

The 34 are 34 of the 35 form bodies §2 names: the 31 regulated-report forms of
§3, `DataProcessors/ДокументооборотСКонтролирующимиОрганами/Forms/ОтветыНаТребованияФСС`
(§4) and the two `Documents/ЗаказПереработчику*/Forms/РабочееМесто` (§5). The
35th is `DataProcessors/ЛичныйКабинетПоставщика/Forms/ПретензииПоставщика`,
which §7 names and which differs from native outside this marker as well.

`cargo test --lib` 2 358 passed / 33 failed, the 33 names byte-identical to
`$D/baselines/9b58354/fail-base.txt`. `cargo test -p ibcmd-schema` 111/0.
`bundled9.sh` 9/9. `cargo fmt --check` and `git diff --check` clean.

## 7. Named and open

One `<UseAlways>` disagreement is left on the whole stand:
`DataProcessors/ЛичныйКабинетПоставщика/Forms/ПретензииПоставщика` writes
`Список.АнкетаПоставщика` and `Список.Контрагент` plain where the platform
marks them. It is the pre-existing residue of the query reader named in
[`form-dynamic-list-field-twin-20260826.md`](form-dynamic-list-field-twin-20260826.md)
— the same list's seven item `<DataPath>`s disagree the same way — and that
form differs from native in `<ExcludedCommand>` as well, so closing the marker
alone would not make it byte-exact.

Unattempted, and unchanged by this package: the shape residue of
`Catalogs/Лоты` and `Documents/Лот`, whose field map remembers one name under
two ids and whose resolution is by position in the rebuilt available-field
collection rather than by name.
