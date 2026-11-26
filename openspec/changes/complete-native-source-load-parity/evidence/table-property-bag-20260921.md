# The table record's keyed property bag

2026-09-21, ERP УХ, 10 738 `{55,…}` records whose bag splits.

The bag is `<count>` followed by that many `(key, value)` pairs, where a value
is a typed 1C value — `{"B",0}`, `{"N",60}`, `{"S",""}`, `{"U"}` or
`{"#",<type uuid>[,<ordinal>]}`. Which keys a table carries is decided by the
**type of the attribute its `<DataPath>` binds to**:

| attribute type | tables | key set |
|---|---|---|
| `cfg:DynamicList` | 4 333 | 5–12, 14, 15, 16, 19, 20 |
| a tabular section (`v8:ValueTable`, `cfg:DocumentObject`, `cfg:CatalogObject`, …) | 6 228 | 13 when `<RowFilter>` is named, and 19 |
| `v8:ValueTree`, `v8:ValueListType` | 1 648 | 19, or nothing |
| `dcsset:SettingsComposer` | 177 | 0, 1, 3, 4 |

## What each key holds

| key | property | value |
|---|---|---|
| 0 | constant | `{"#",54ea12c9-d306-418f-93…}` |
| 1 | constant | `{"#",f33178f9-6060-48d3-bc…}` |
| 3 | `<ViewMode>` | `{"#",c04ead79-…,0}` `All`, `,1}` `QuickAccess` |
| 4 | `<SettingsNamedItemDetailedRepresentation>` | `{"B",0|1}` |
| 5 | `<AutoRefresh>` | `{"B",0|1}` |
| 6 | `<AutoRefreshPeriod>` | `{"N",n}`, 60 when unnamed |
| 7 | constant | `{"#",2fdc88ec-7c9b-43cd-8b…}` |
| 8 | `<ChoiceFoldersAndItems>` | `{"#",59ef2b80-…,0|1|2}` for `Items`, `Folders`, `FoldersAndItems` |
| 9 | `<RestoreCurrentRow>` | `{"B",0|1}` |
| 10 | constant | `{"U"}` |
| 11 | `<ShowRoot>` | `{"B",0|1}`, 1 when unnamed |
| 12 | `<AllowRootChoice>` | `{"B",0|1}` |
| 13 | `<RowFilter>` is named | `{"U"}` — pure over all 6 228 |
| 14 | `<UpdateOnDataChange>` | `{"#",eac7bfa0-…,0}` `Auto`, `,1}` `DontUpdate` |
| 15 | constant | `{"U"}` |
| 16 | the id of the item `<UserSettingsGroup>` names | `{"N",n}`, 0 when unnamed |
| 19 | constant | `{"S",""}` |
| 20 | `<AllowGettingCurrentRowURL>` | `{"B",0|1}`, 1 when unnamed |

Every non-constant key is **pure**: each spelling of its property, and its
absence, maps to exactly one stored value over every record that carries it.

Key 16 took a second pass. It is 0 in all 1 285 tables that name no
`<UserSettingsGroup>`, and in the other 2 764 it is the id of the item the
group names — in **2 756** directly, and in the remaining 8 the group is
spelled `<id>:<namespace>` and the id part is what the bag carries. So the
rule holds in all 2 764.

## The presence of a key is not always in the source

451 tables name `<AllowGettingCurrentRowURL>true</AllowGettingCurrentRowURL>`
and carry **no key 20** at all; 926 dynamic lists carry no key 15 and 938 no
key 19, and no property of the table decides which. The same class of fact as
the navigator. It is harmless for the round trip: `true` is the default, so
the export reads it back whether the key is there or not, and the writer emits
the key whenever the property is named.
