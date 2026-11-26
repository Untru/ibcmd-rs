# ERP УХ differing-file root-cause map, 20260825

Status: measurement of the `uh` gate's `differing` set — the files we write
that do not byte-match the platform — and of what four fixes in this pass
closed. Companion to `uh-missing-root-cause-map-20260825.md`, which does the
same for files we do not write at all.

## Result of this pass

| | exact | differing | missing | percent |
| --- | ---: | ---: | ---: | ---: |
| before | 120,504 | 18,429 | 1,478 | 85.82 % |
| after the localized-text and mask fixes | 128,104 | 10,829 | 1,478 | 91.24 % |
| after the row-table fixes | 131,584 | **7,349** | 1,478 | **93.71 %** |
| Δ | **+11,080** | **−11,080** | 0 | +7.89 pp |

**Broken: 0** on every step, on `uh`, `ut` and the five fast gates.

## Method

The first byte at which two files differ is a poor classifier: it names
whichever cosmetic difference happens to come first and hides everything
behind it. Measured directly — applying the `<description/>` fix textually to
our own output made **23** files exact out of the **7,570** whose first diff
it was.

So each differing file is classified by its *whole* content instead: take the
multiset difference of its lines both ways, native-only and ours-only, and
record the set of element names appearing in each. A cause is credited with a
file if it appears anywhere in that file's difference, and credited as *sole
cause* only when it is the only element name involved — the second number is
what a fix can actually be expected to close on its own.

## What the four fixes closed

| fix | files made exact |
| --- | ---: |
| every declared translation of a cell's text is read, not just the first | +7,145 |
| the root's tail table is anchored on data, not on a fixed six-field trailer (input masks) | +455 |
| empty `<languageSettings>` texts written as empty elements | included above |

The first was the single largest defect class on the gate: the cell text
container is `{1, <count>, {<lang>,<content>}, …}`, the count was read and one
pair consumed, and the writer stamped `ru` on the survivor. `v8:item` was
native-only in **14,189 of the 18,429** differing files.

## The remaining 7,349

By file role: `Ext/Template.xml` 4,148 (of which 2,728 MXL `<document>` and
1,420 `<DataCompositionSchema>`), `Ext/Form.xml` 2,776, top-level object XML
288, the rest under 40 each.

Ranked by the number of differing files each element name appears in:

| element | native-only in | ours-only in | sole cause |
| --- | ---: | ---: | ---: |
| `v8:content` / `v8:lang` / `v8:item` | 4,786 | 150 | 1 |
| `f`, `tl`, `c`, `i`, `v`, `index`, `row`, `rowsItem` | ~3,530 | ~250 | 0 |
| `defaultFormatIndex` | 2,897 | 2,897 | 0 |
| `formatIndex` | 2,834 | 1,634 | 0 |
| `columnsID` | 2,676 | 0 | 1 |
| `empty` | 552 | 1,751 | 0 |
| `appearance` | 0 | 1,409 | 47 |
| `dcsset:outputParameters` | 0 | 1,350 | 9 |
| `beginRow` / `endRow` / `beginColumn` / `endColumn` | 1,038 | 0 | 0 |
| `Shortcut` | 737 | 713 | **634** |

### 1. The MXL row table — closed, 3,701 mismatches → 7

Three defects, all of them found by extracting the template's storage element
and walking its declared row block by hand rather than by reading the XML.

**A refused cell fails its row, and a failed row truncates the rest of the
stream.** That is what made this look like one huge cluster: two small reader
gaps cost thousands of files each.

* **Cell value type `"B"`.** The reader knew `{"U"}`, `{"S"}`, `{"N"}`,
  `{"D"}` and `{"#"}` but not the boolean.
  `Report.РегламентированноеУведомлениеВозвратНДФЛНПДБиоресурсы.Template.
  Титульная_2026` declares 41 rows and published 21 — it stops on
  `{2,32,{"B",0}}` at row 21, where native publishes
  `<v xsi:type="xs:boolean">false</v>`. ERP УХ publishes 1,346 booleans, all
  `false`. **+445 files.**
* **The trailing "formatted" flag is optional.** It is the cell record's last
  member and a record may stop in front of it; requiring it refused every cell
  of `Catalog.ВариантыНаладки.Template.Палитра`, whose stream then yielded one
  row against the ten it declares. Native publishes those cells as plain
  `<tl>` — the file carries ten `<tl>` and no `<tfl>`. **+2,751 files.**
  (This overturned a unit test that asserted the strict arity; the corpus
  decides — doctrine rule 8.)
* **A skipped row is a row at the default format.** Rows the stream skips were
  manufactured with no `source_format_index`, so
  `compact_moxel_empty_row_ranges` — the corpus-proven rule that folds
  adjacent equal cell-less rows into one `<indexTo>` item — never compared
  them equal to the stored empty row in front of them, and the run was
  published one item per index. Giving them the shape a stored `0` format
  field means (`format_index` 1, `source_format_index` `Some(1)`) lets the
  existing rule do it. **+284 files.**

  Measured over every `<rowsItem>` in the native tree — 3,739,968 of them,
  1,240 carrying an `<indexTo>` — **every one is an empty row** (1,117 without
  a `<formatIndex>`, 123 with), and **none** has `indexTo` equal to its
  `index`, so a one-wide run carries none. Where the row in front of a gap
  does carry a format the run must stay separate, and the same comparison
  keeps it separate: `Catalog.АналитическиеПанели.Template.ШаблонВиджета`
  publishes its formatted empty row 7 alone and the gap 8 as its own item. An
  earlier attempt that absorbed gaps into the preceding empty row directly,
  bypassing that comparison, broke exactly those three files and was replaced.

Row-count mismatches: **3,701 → 7**, and the seven remaining are all
"ours fewer", i.e. a different refusal still to be found.

### 2. Remaining localized values — 4,786 files

`v8:item` is still native-only in 4,786 files after the cell-text fix, so
other localized containers drop translations the same way. The cell text
container is fixed; the ones behind these are not yet identified
individually.

### 3. DCS empty elements — 1,409 and 1,350 files, 47 and 9 sole

We write `<appearance/>` and `<dcsset:outputParameters/>`; **native never
writes either** — 0 self-closing occurrences against 8,165 and 5,266
non-empty ones on `uh`, and 0 against 2,752 / 1,762 on `ut`, 140 / 181 on
`ssl`, 57 / 158 on `sslbase`. The rule is "omit when empty", not "write it
self-closing", and it is measured with 0 counterexamples across four corpora.
It was not applied in this pass because the DCS output is produced by a
generic tree serializer under the schema crate's cohort policies rather than
by a literal writer, so the change belongs with that subsystem. Note the
other half is a separate gap: we publish 6,988 non-empty `<appearance>`
against native's 8,165.

### 4. `Shortcut` — 634 files, and not a defect

The blob stores a neutral modifier bitmask (`CTRL = 8`) and the platform
renders it per host: `Ctrl+T` on Windows, `Cmd+T` on macOS. The stand's `uh`
tree was captured on macOS and we render `Ctrl`, so all 634 differ on that
line alone. This is the same host dependency already recorded for `ut` in
`host-dependent-export-2214-20260823.md` (437 files there), where the project
settled that these are not ours to fix. Counting them as defects would mean
making our output depend on the machine it runs on.

**So the honest residual is 6,715 files.** With the row table closed, the
largest pieces are now forms (2,776 files — `ExtendedTooltip`, `DataPath`,
`ChildItems`, `ContextMenu`, `Representation`, and a `Type` cluster), the
1,420 DCS templates, and what is left of the MXL templates: the named-item /
`area` block (`beginRow`/`endRow`/`beginColumn`/`endColumn`, ~1,038 files),
the vertical-group block (`vg`/`vgLevels`, 486), and the localized containers
that still drop translations.
