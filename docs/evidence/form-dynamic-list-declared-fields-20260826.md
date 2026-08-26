# Dynamic-list `~`: the fields the main table actually declares, 20260826

Status: the two named residues of
[`form-dynamic-list-field-twin-20260826.md`](form-dynamic-list-field-twin-20260826.md)
are closed. Base `dd1fd72`, measured over every dynamic-list record `cf export`
walks on the stand ($D = `/Users/untru/Documents/ChatGPT/ibcmd-stand`), all eight
configurations.

## What was wrong

A dynamic list's resolvable-field universe was built without ever asking what
the main table declares.

* `form_dynamic_list_std_attribute_pairs` admitted a family's whole
  standard-attribute table. A catalog with `<CodeLength>0</CodeLength>` has no
  `Code`; one with an empty `<Owners/>` has no `Owner`; one whose
  `<HierarchyType>` is `HierarchyOfItems` has no `IsFolder`; a `Nonperiodical`
  information register has no `Period`. The platform marks every one of them,
  the reader wrote them plain.
* The universe admitted every `CommonAttribute.<name>` of the configuration,
  because the object-reference index carries the common attributes but not their
  `<Content>`. A common attribute is a field only of the tables its content puts
  it on.

## What the record declares

Both facts are already read when the objects themselves are written — the
catalog's `<CodeLength>`, `<DescriptionLength>`, `<Owners>`, `<Hierarchical>`
and `<HierarchyType>`, the register's `<InformationRegisterPeriodicity>`, the
common attribute's `<AutoUse>` and `<Content>`. The package does not copy them:
the owner-field slots are named once
(`CATALOG_OWNER_FIELD_*`, `INFORMATION_REGISTER_OWNER_FIELD_PERIODICITY`) and
read from both the family's own properties parser and
`build_metadata_field_declaration_index_from_texts`, which hands the same
declaration to the form decoder through `FormParseContext`.

`MetadataTableStandardAttributes` keeps one `Option` per property. `None` means
this reader could not name the property, and an unread property withholds
nothing: the standard attribute stays in the universe exactly as before, so
every family the index does not decode behaves as it did. Same for a common
attribute whose `<AutoUse>` does not read: the declaration is left out of the
index and the name is admitted everywhere.

## Census

Per rule, the number of dynamic lists whose universe the rule takes a name out
of, and what that changed against the platform's own markers:

| rule | uh | ut | do | ssl | sslbase | markers gained | markers lost |
|---|---:|---:|---:|---:|---:|---:|---:|
| `<CodeLength>0` -> no `Code` | fires | 464 | 162 | 73 | 65 | 4 | 0 |
| `<DescriptionLength>0` -> no `Description` | fires | 33 | 19 | 2 | 2 | 1 | 0 |
| empty `<Owners>` -> no `Owner` | fires | 552 | 391 | 101 | 81 | 3 | 0 |
| not hierarchical -> no `Parent` | fires | 448 | 263 | 72 | 57 | 0 | 0 |
| no folder hierarchy -> no `IsFolder` | fires | 502 | 325 | 93 | 75 | 2 | 0 |
| `Nonperiodical` -> no `Period` | fires | 408 | 218 | 90 | 81 | 1 | 0 |
| common attribute content | fires | — | — | — | — | 5 | 0 |

Over 40 454 ERP УХ, 18 462 UT, 6 264 Документооборот, 2 295 БСП demo and 1 754
БСП base written marker positions the rules move exactly 17, every one of them
towards the platform, and none away from it.

The 15 ERP УХ positions: `Catalog.КатегорииЗакупок` (`ФормаСписка`,
`ФормаВыбора`), `Catalog.УдалитьПанелиОтчетов`,
`Catalog.ЭлектронныеТорговыеПлощадки` — `Code`, two of them in the doubled
`~Список.Code~Список.Код` spelling; `Catalog.КодыВидовРасхода` —
`~Список.Description~Список.Наименование`; `Catalog.ИнтервалыЗадолженностей`,
`Catalog.РазделыИнвестиционныхПрограмм`,
`Catalog.ВерсииРегламентовПодготовкиОтчетности` — `Owner`;
`Catalog.ОбщероссийскийКлассификаторОсновныхФондов` (`HierarchyOfItems`) —
`IsFolder`; `InformationRegister.НоменклатураАккредитованыхПоставщиков` —
`Period`; and five common-attribute positions, `~Список.КлассВНА` on
`Catalog.ГруппыВНАМСФО` (two forms), `Document.ИзменениеПараметровВНАМСФО` and
`Document.ВводНачальныхОстатковВНАМСФО`, and `~Список.НСИ_НеАктивный` on
`Catalog.ПроизвольныйКлассификаторУХ`. `CommonAttribute.КлассВНА` lists seven
tables in its content and none of those three;
`CommonAttribute.НСИ_НеАктивный` does not list that catalog either. The one
Документооборот position is `Catalog.СостоянияЧатБота` (`HierarchyOfItems`),
`~Список.ЭтоГруппа`.

## Refused rather than guessed

`Number` on a document, business process or task whose `<NumberLength>` is zero,
and `Recorder`/`LineNumber` on an independent information register, are the same
shape as the rules above and have no platform observation anywhere on the stand:
enabling them would have withdrawn a name from 446 ERP УХ, 240 Документооборот,
99 БСП demo and 87 БСП base lists without changing a single written marker. They
are named in `MetadataTableStandardAttributes` as a refusal instead of being
implemented.

The families the index decodes are `Catalog` and `InformationRegister`, the two
the evidence lives on. A `ChartOfCharacteristicTypes` or `ChartOfAccounts` with a
zero code length is not in the index at all, so it keeps the whole family table
— the pre-existing behaviour, not a claim.

## Result

Exact-set difference against `$D/baselines/dd1fd72/<key>.parity.json`:

| key       |    было |    стало | прибавилось | сломано | лишних |
|-----------|--------:|---------:|------------:|--------:|-------:|
| `ws`      |      29 |       29 |           0 |       0 |      0 |
| `wms`     |     226 |      226 |           0 |       0 |      0 |
| `mdm`     |     160 |      160 |           0 |       0 |      0 |
| `sslbase` |   9 617 |    9 617 |           0 |       0 |      0 |
| `ssl`     |  12 697 |   12 697 |           0 |       0 |      0 |
| `do`      |  25 322 |   25 323 |           1 |       0 |      0 |
| `ut`      |  50 898 |   50 898 |           0 |       0 |      0 |
| `uh`      | 138 899 |  138 908 |           9 |       0 |      0 |

`нет файла` unchanged on every key.

## Reproduction

The census was taken with a temporary probe in
`parse_form_attribute_with_dcs_type_index` that appended, per dynamic-list
attribute, the form's own source path, the settings bag's top-level entries, the
parsed field map and the computed universe to a JSON-lines file, joined against
the `<UseAlways>` blocks and item `<DataPath>`s of the native tree and against
the `<CodeLength>`/`<Owners>`/`<AutoUse>`/`<Content>` the native tree spells out
for each object. The probe is not part of this package.
