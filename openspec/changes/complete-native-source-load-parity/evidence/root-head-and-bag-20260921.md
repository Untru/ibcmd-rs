# The root record's head and property bag, measured over ERP УХ

Date: 2026-09-21. Same corpus and split as
[root-record-20260921.md](root-record-20260921.md): 12 488 of the 12 507 root
records split into head, children and tail.

Tools: `F:\ibcmd\lab\tools\try-root-head.py`, `census-root-bag.py`. Reports:
`root-head-try.txt`, `root-bag.txt`.

## The head is 18 fixed members, then a keyed bag

```
heads 12469, fixed part exact 12469 (100.00%)
```

**12 469 of 12 469 heads rebuild byte for byte from the source alone**, refusing
only the 19 forms that name a `<SettingsStorage>`: member 8 is then that storage
object's uuid, which the source alone cannot resolve, and the zero uuid every
other form writes would be wrong.

| # | source | absent |
|---|---|---|
| 0 | always `50` | |
| 1 | always 0 | |
| 2 | `<WindowOpeningMode>`: `DontUse` 0, `LockOwnerWindow` 1, `LockWholeInterface` 2 | 0 |
| 3 | `<Width>` | 0 |
| 4 | `<Height>` | 0 |
| 5 | `<EnterKeyBehavior>`: `DefaultButton` → 0 | 1 |
| 6 | `<SaveDataInSettings>`: `UseList` → 1 | 0 |
| 7 | `<AutoSaveDataInSettings>`: `Use` → 1 | 0 |
| 8 | `<SettingsStorage>`'s uuid | the zero uuid |
| 9 | `<AutoTitle>` | 1 |
| 10 | `<Title>` | `{1,0}` |
| 11 | whether `<Group>` is named at all | 0 |
| 12 | `<ChildItemsWidth>`: `Equal` 1, `LeftWide` 2, `LeftWidest` 3, `LeftNarrow` 4, `LeftNarrowest` 5 | 0 |
| 13 | `<AutoFillCheck>` | 1 |
| 14 | `<Customizable>` | 1 |
| 15 | `<Enabled>` | 1 |
| 16 | always 0 | |
| 17 | `<CommandBarLocation>`: `None` 0, `Auto` 1, `Top` 2, `Bottom` 3 | 1 |

Member 15 is `<Enabled>`, not `<WindowOpeningMode>`, although both fit the five
records that moved: `Enabled` appears in exactly those five and only as `false`,
while `LockOwnerWindow` appears in 3 903 forms of which only those five differ.

## What follows is a keyed property bag

After member 17 the record carries a count and that many `(key, value)` pairs,
and then **exactly four members** -- the events, the command set, the command
bar's flag and the bar itself. The count holds in **all 12 488** records that
split, with nothing left over, and the four members after it hold in all 12 488
too. That is what makes the record parseable at all: the head before the bag is
fixed, the bag is self-describing, and the tail is anchored from the other end.

The keys seen, by how often they are used:

| key | value | what moves with it |
|---|---|---|
| 1 | `{"N",0}` (3 114 of 3 122) | not a scalar property |
| 24 | `{"B",0}` | not a scalar property |
| 2 | `{"#",adeb08a0-…,<n>}` | `<AutoTime>` |
| 3 | `{"#",20d89b09-…,<n>}` | `<UsePostingMode>` |
| 4 | `{"B",0\|1}` | `<RepostOnWrite>` |
| 0 | `{"#",59ef2b80-…,<n>}` | `<UseForFoldersAndItems>` |
| 25 | `{"U"}` | not a scalar property |
| 26 | `{"B",1}` | not a scalar property |
| 7 | `{"#",acbc2eeb-…,<n>}` | the report form settings |
| 23 | `{"N",<n>}` | not a scalar property |

Keys 2, 3, 4 and 0 are the document and catalog properties, written as typed
1C values -- `{"#",<enum uuid>,<index>}` for an enumeration, `{"B",<0\|1>}` for
a boolean, `{"N",<n>}` for a number.

## The bag belongs to the main attribute, not to the form

Keys 1, 23, 24, 25 and 26 move with no scalar property of the form because the
bag is not the form's: it is the **form extension** of the main attribute, and
each extension numbers its own keys. Correlating every key with the class of the
main attribute (`F:\ibcmd\lab\tools\probe-bag-keys.py`, over all 12 507 forms)
separates them completely:

| keys | main attribute class |
|---|---|
| 1 | `DynamicList` -- and no other class ever writes it |
| 0 | `CatalogObject`, `ChartOfCharacteristicTypesObject` |
| 2, 3, 4 | `DocumentObject`, and no other class |
| 24 | the object classes: `DocumentObject`, `CatalogObject`, `TaskObject`, `BusinessProcessObject` |
| 25, 26 | the same object classes, in the subset that carries them |
| 5–22, 27, 29 | `ReportObject`, and no other class |
| 28 | `dcsset:SettingsComposer` |

And the whole-bag view is just as sharp: a form with no main attribute carries
**no bag at all** (4 511 forms), a `DynamicList` form carries exactly `1`
(3 122), a `DataProcessorObject` form carries none (1 165), a document form
carries `2,3,4,24` (714) or `2,3,4,24,25,26` (405), a catalog form `0,24` (492)
or `0,24,25,26` (358).

So a writer emits the bag by asking what the form's main attribute is, then
which of that extension's properties the source names. That is the shape the
next measurement fills in, per class.

`NativeRootLayout` now takes the bag as `(key, value)` pairs, so a writer that
knows a key can emit it; until every key is read, a form that needs one it
cannot write is refused rather than written short.
