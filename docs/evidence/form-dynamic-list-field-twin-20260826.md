# Dynamic-list `~`: the localized twin the field map declares, 20260826

Status: root cause and partial fix for the `~` marker on `<Field>` and
`<DataPath>` in ERP УХ form bodies. Commits `71775f3`, `9edd076`, `7290326`. Measured over every dynamic-list record
`cf export` walks on the stand ($D = `/Users/untru/Documents/ChatGPT/ibcmd-stand`),
all eight configurations.

## Symptom

The marker model was fitted on UT 11.5.27.75, where it is nearly exact, and
ERP УХ 3.2.12.6 broke it in both directions. In the native ERP УХ tree the
marker appears 1 438 times on `<UseAlways><Field>`, 203 times on an item
`<DataPath>` and 180 times on `<RowPictureDataPath>`, and the export
disagreed with the platform on 132 of the 4 432 dynamic-list records that
carry a required-field list. 33 form bodies differed from native in nothing
but this marker.

Two observations made a language rule impossible. Standard attributes were
written plain under their **English** names on some lists and marked on
others; and the platform can write **both spellings glued into one element**:

```
<Field>~Список.Ref~Список.Ссылка</Field>
<DataPath>~Список.Code~Список.Код</DataPath>
```

with, in `Catalogs/Лоты/Forms/ФормаСписка`, a plain `Список.Ref` standing
next to the glued one in the same `<UseAlways>` block. Nothing keyed on the
name alone can produce that.

## What the record actually declares

The dynamic-list settings bag carries the field map twice — once under the
platform's own misspelling `FiledsMapItem*` and once under `FieldsMapItem*`
(the two never disagree: 0 clashes in 15 456 records across the stand) — and,
next to the second spelling, a **third** slot that nothing read:

```
"FieldsMapItemId0"             {"N",1}
"FieldsMapItemName0"           {"S","Code"}
"FieldsMapItemSecondaryName0"  {"S","Код"}      <- never read
"FiledsMapItemId0"             {"N",1}
"FiledsMapItemName0"           {"S","Code"}
"FieldsMapSecondaryNamesLoaded" {"B",1}
```

The map remembers a field of the list by **two names**: its own, and a
localized twin. A standard attribute is remembered under the English name
with the Russian spelling beside it (`Code`/`Код`, `Ref`/`Ссылка`,
`Owner`/`Владелец`); a field that is not a standard attribute — a declared
attribute, a query alias — is remembered under its single name and carries no
twin.

Both names name the same field, which settles both halves of the symptom:

* a remembered field is resolvable when **either** spelling is a field of the
  list, which is why an English name can be written plain on a list whose
  query spells everything in Russian;
* when neither is, the platform writes the **pair** out in full, each half
  carrying its own marker — the glued spelling.

### Census

Over the whole stand, per dynamic-list record with a required-field list:

| corpus    | lists | required fields | remember a twin | glued outputs |
|-----------|------:|----------------:|----------------:|--------------:|
| `uh`      | 4 432 |          10 168 |           1 474 |             6 |
| `ut`      | 1 936 |           4 379 |             744 |             0 |
| `do`      |   813 |           1 851 |             283 |             0 |
| `ssl`     |   305 |             595 |             174 |             0 |
| `sslbase` |   234 |             517 |             135 |             0 |
| `mdm`     |     6 |               1 |               1 |             0 |
| `wms`     |     2 |               1 |               1 |             0 |
| **total** | 7 728 |          17 512 |           2 812 |             6 |

Every required-field id produces exactly one output entry on every
configuration (`required == emitted` with no exception), so the two columns
align entry by entry.

On the 7 624 lists the corrected model reproduces byte for byte, 2 790 of the
2 791 twin-carrying required fields on them are written **plain** and exactly one is
written marked: `Documents/Лот/Forms/ФормаСписка`, whose remembered
`Owner`/`Владелец` pair names a standard attribute no document family has.
The remaining glued outputs live on `Catalogs/Лоты` and `Documents/Лот`,
where the map remembers the same pair twice under two different ids and the
platform resolves only one of them — the residue described below.

## Second correction: a manual query has no English spellings of its own

`form_dynamic_list_main_table_auto_fields` added the main table's standard
attributes to a manual query's field universe under **both** spellings, and
`form_dynamic_list_selected_standard_twins` added the English twin of every
standard attribute the query selected under its own Russian name. Both are
wrong: a manual query's available-field list is named by what the query
produces, so the English spelling of a standard attribute is not a name of
that list at all. A remembered field that carries the English one reaches the
list through its twin, not through these sets.

Evidence: 39 ERP УХ lists whose `<UseAlways>` those English spellings
resolved and the platform marked. The clearest is
`DocumentJournals/ПротоколыЗакупочныхПроцедур/Forms/ФормаСписка`, whose query
selects `ЖурналДокументов….Ссылка КАК Ссылка` and `….Дата КАК Дата` and whose
native block is

```
<Field>~Список.Date</Field>
<Field>~Список.Ref</Field>
…
<Field>Список.Дата</Field>
<Field>Список.Ссылка</Field>
```

— the aliases plain, the English spellings marked, in the same block. On the
other side, `Tasks/ЗадачаИсполнителя/Forms/МоиЗадачи` writes `Список.Ref` and
`Список.BusinessProcess` plain: its map remembers those two with the twins
`Ссылка` and `БизнесПроцесс`, and the query selects both aliases. Not one
list on any of the eight configurations needs an English spelling from the
main-table set.

The twins helper is deleted; the main-table set keeps the Russian spelling
only. The auto-list branch is untouched — an auto list's fields *are* the
main table's, in both spellings.

## Result

Exact-set difference against `$D/baselines/8cc12dc/<key>.parity.json`:

| key       |    было |    стало | прибавилось | сломано | лишних |
|-----------|--------:|---------:|------------:|--------:|-------:|
| `ws`      |      29 |       29 |           0 |       0 |      0 |
| `wms`     |     226 |      226 |           0 |       0 |      0 |
| `mdm`     |     160 |      160 |           0 |       0 |      0 |
| `sslbase` |   9 614 |    9 614 |           0 |       0 |      0 |
| `ssl`     |  12 692 |   12 692 |           0 |       0 |      0 |
| `do`      |  25 201 |   25 210 |           9 |       0 |      0 |
| `ut`      |  50 896 |   50 896 |           0 |       0 |      0 |
| `uh`      | 138 467 |  138 494 |          27 |       0 |      0 |

Marker by marker on ERP УХ, over the 12 473 form bodies whose `<Field>` /
`<DataPath>` / `<RowPictureDataPath>` sequence lines up with native: 945 → 1 052
markers agree, 89 → 40 disagree. Form bodies whose *only* difference from
native is this marker: 33 → 12.

Model agreement per dynamic-list record, before and after: `uh` 4 300 → 4 342
of 4 432, `do` 808 → 812 of 813, `ut` 1 931 of 1 936 unchanged, `ssl` and
`sslbase` all of theirs, unchanged.

## What is left

40 marker positions on 21 ERP УХ form bodies still disagree, in two named
residues plus the query reader's own.

1. **Property-blind standard attributes** — 7 positions on 6 form bodies.
   `form_dynamic_list_std_attribute_pairs` admits a family's whole
   standard-attribute table regardless of what the object declares, so on an
   auto list `Code` stays resolvable on `Catalog.КатегорииЗакупок`
   (`CodeLength` 0) and `Catalog.УдалитьПанелиОтчетов`, `Description` on
   `Catalog.КодыВидовРасхода` (`DescriptionLength` 0), `Owner` on
   `Catalog.ИнтервалыЗадолженностей` and
   `Catalog.РазделыИнвестиционныхПрограмм` (empty `<Owners/>`), and `Period`
   on the non-periodical `InformationRegister.НоменклатураАккредитованыхПоставщиков`
   — every one of which the platform marks, two of them in the doubled
   spelling. Reading the declared properties needs a per-family metadata index
   the form decoder does not have yet.

2. **Common attributes admitted everywhere** — 5 positions on 5 form bodies.
   The universe admits every `CommonAttribute.<name>` in the configuration,
   because the object-reference index carries the common attributes but not
   their `<Content>`. ERP УХ marks `~Список.КлассВНА` on
   `Catalog.ГруппыВНАМСФО`, `Document.ИзменениеПараметровВНАМСФО` and
   `Document.ВводНачальныхОстатковВНАМСФО`, and `~Список.НСИ_НеАктивный` on
   `Catalog.ПроизвольныйКлассификаторУХ`: none of those tables is in the
   common attribute's content list, whose `AutoUse` is `DontUse`.

Outside the marker positions, one shape residue is worth naming.
`Catalogs/Лоты` and `Documents/Лот` remember `Ref`/`Ссылка` and
`DeletionMark`/`ПометкаУдаления` under **two different field-map ids each**;
the platform resolves one of each pair and marks the other, writing both a
plain `Список.Ref` and a doubled `~Список.Ref~Список.Ссылка` in the same
`<UseAlways>` block. Resolution there is by the id's position in the rebuilt
available-field collection, not by name, so the reader emits one entry where
the platform emits two. Reconstructing that numbering is unattempted.

The rest is the pre-existing residue of the query reader: nine data paths on
`DataProcessors/ЛичныйКабинетПоставщика/Forms/ПретензииПоставщика` and eleven
on `InformationRegisters/ОповещенияПользователей/Forms/ФормаСписка`, whose
queries the reader resolves where the platform does not, and two lone
over-marks (`Documents/ПлановаяКалькуляция`, `Documents/НаработкаОбъектовЭксплуатации`).
None of it is touched here.

## Reproduction

The census was taken with a temporary probe in
`parse_form_attribute_with_dcs_type_index` that appended, per dynamic-list
attribute, the settings bag's top-level entries plus the parsed query
selection to a JSON-lines file, joined against the `<UseAlways>` blocks of
the native tree. The probe is not part of this package.
