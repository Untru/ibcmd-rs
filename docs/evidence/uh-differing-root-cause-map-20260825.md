# ERP УХ differing-file root-cause map, 20260825

Status: measurement of the `uh` gate's `differing` set — the files we write
that do not byte-match the platform — and of what four fixes in this pass
closed. Companion to `uh-missing-root-cause-map-20260825.md`, which does the
same for files we do not write at all.

## Result of this pass

| | exact | differing | missing | percent |
| --- | ---: | ---: | ---: | ---: |
| before | 120,504 | 18,429 | 1,478 | 85.82 % |
| after | 128,104 | **10,829** | 1,478 | **91.24 %** |
| Δ | **+7,600** | **−7,600** | 0 | +5.41 pp |

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

## The remaining 10,829

By file role: `Ext/Template.xml` 7,628 (of which 6,208 MXL `<document>` and
1,420 `<DataCompositionSchema>`), `Ext/Form.xml` 2,776, top-level object XML
288, the rest under 40 each. By owner: `Reports` 6,953, `DataProcessors`
1,065, `Documents` 1,042, `Catalogs` 936, `InformationRegisters` 417.

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

### 1. The MXL row table — ~3,700 files, the largest cluster

3,701 templates publish a different number of `<rowsItem>` than native, and
not by a constant: some publish far fewer (5 against 153, 4 against 43), some
more (30 against 22). `columnsID`, `formatIndex`, `defaultFormatIndex`, and
the cell children `c`/`f`/`i`/`v`/`tl` move with it, which is what makes this
one cluster rather than several — e.g.
`Reports/РегламентированноеУведомлениеВозвратНДФЛНПДБиоресурсы/Templates/
Титульная_2026` drops 610 consecutive native lines from `<index>21</index>`
onward and then disagrees on `<defaultFormatIndex>` (58 against 83).

This is the next thing to work and it is a real investigation, not a rule:
the row table's encoding has to be read out of extracted elements before
anything is written.

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

**So the honest residual is 10,195 files**, with the row-table cluster the
largest single piece of it.
