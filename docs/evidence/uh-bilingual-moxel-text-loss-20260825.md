# ERP УХ bilingual text loss in MOXCEL cell/drawing text lists, 20260825

Status: root cause found and fixed for the largest single component of the
`uh` (ERP Управление холдингом) `differing` set. `uh` is the only bundled
corpus that is genuinely bilingual (`Languages/` carries both `ru` and
`en`; 76,268 of 140,411 files carry English strings) -- УТ and БСП are
effectively single-language, so this class of defect could not surface on
any other gate corpus.

## The measurement that framed the search

Baseline `uh` run at `d0457a6` (release binary, `zsh $D/kit/run.sh uh
<worktree> <out>`, compared with `$D/kit/parity2.py` against
`$D/cap/uh-r1/src`):

```
native_files 140411, exact 120592, differing 18487, missing 1332, extra 64
percent 85.885
```

Categorizing the `differing` set by counting `<v8:lang>en</v8:lang>`
occurrences native vs. ours (native strictly greater = we dropped an
English item) and, for each dropped occurrence, walking backward to the
nearest non-`v8:*` enclosing tag:

```
   5548  Title
   3960  mask       (moxel spreadsheet cell/drawing member records)
   1666  text        (moxel <tl>/<tfl> cell text list)
    934  ToolTip
    608  editFormat
    282  Synonym
    186  Presentation
     73  dcssch:title
     51  InputHint
     50  tfl
     42  Format
     26  EditFormat
      8  d4p1:text
      ... (long tail, each under 5 files)
```

By path family (first two path segments), the loss concentrates overwhelmingly
in `Reports/РегламентированныйОтчет*/Templates/*/Ext/Template.xml` --
regulated-report spreadsheet layouts, e.g. 610 files under
`Reports/РегламентированныйОтчетПрибыль`, 520 under
`.../РегламентированныйОтчет3НДФЛ`, 448 under `.../РегламентированныйОтчетНДС`,
continuing down a long tail of the same shape. These are all MOXCEL
(`.mxl`-derived) spreadsheet documents -- the `document` root with
`xmlns="http://v8.1c.ru/8.2/data/spreadsheet"` that `src/mssql_dump/moxel.rs`
reads and writes.

Direct byte inspection of a real native file,
`Reports/РегламентированныйОтчетНДДУ/Templates/ФормаОтчета2022Кв4_Раздел2/Ext/Template.xml`
(179 KB, from `$D/cap/uh-r1/src`): 576 `<v8:item>` elements, 144
`<v8:lang>ru</v8:lang>` and 144 `<v8:lang>en</v8:lang>` -- every cell text
list (`<tl>`) in the file carries both languages, one `<v8:item>` per
language, e.g.:

```xml
<c><f>19</f><tl>
    <v8:item><v8:lang>ru</v8:lang><v8:content>Добавить страницу</v8:content></v8:item>
    <v8:item><v8:lang>en</v8:lang><v8:content>Добавить страницу</v8:content></v8:item>
</tl></c>
```

(The `en` content here happens to duplicate the `ru` text verbatim -- this
report template was never translated -- but the platform still declares
and publishes both `v8:item` entries; the defect is at the item-count
level, not at translating content.)

## Root cause 1 (the `text`/`mask` buckets, ~5,600+ files): first-item-only read

`src/mssql_dump/moxel.rs`'s `parse_moxel_localized_cell_value` is the
shared primitive both the cell text list (`<tl>`/`<tfl>`) and the drawing
`<text>` member go through. Before this fix:

```rust
pub(super) fn parse_moxel_localized_cell_value(text: &str) -> Option<Option<MoxelLocalizedValue>> {
    let fields = split_1c_braced_fields(text, 0)?;
    let count = fields.get(1)?.trim().parse::<usize>().ok()?;
    if count == 0 {
        return Some(None);
    }
    let pair = split_1c_braced_fields(fields.iter().skip(2).take(count).next()?, 0)?;
    ...
    Some(Some(MoxelLocalizedValue { lang, content }))
}
```

It reads the declared `count` (how many `{lang, content}` pairs the record
carries) but then `.skip(2).take(count).next()` -- takes only the *first*
pair regardless of `count`, discarding every subsequent language. On a
mono-language corpus (УТ, БСП) `count` is always 1, so this never showed
up; on `uh`'s bilingual corpus `count` is routinely 2 (ru + en) and the
second item silently vanished.

The write side compounded it for the cell case specifically
(`push_moxel_row_xml`, formerly): having kept only one `MoxelLocalizedValue`
in `MoxelCell.text: Option<String>` (the type itself could not hold a
second language), the XML writer then hard-coded the language tag rather
than using the parsed one:

```rust
xml.push_str("\t\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n");
```

i.e. even the single surviving item's language was never read from the
data.

`MoxelDrawingMembers.text: Option<MoxelLocalizedValue>` (the drawing
`<text>` member -- "the same container the cell record uses", per the
pre-existing code comment) had the identical shape and the identical loss,
though its writer at least used the correct `.lang` for the one item it
kept.

The formatted-tail case (`<tfl>`, the `count`-4/5-field record whose fourth
field is a `0`/`1` flag opening a fifth field with markup) went through the
same primitive a third time (`parse_moxel_formatted_cell_text`), with the
same first-item-only bug.

### The fix

- `parse_moxel_localized_cell_value` now collects all `count` declared
  items into a `Vec<MoxelLocalizedValue>` instead of stopping at the
  first. Tolerance for malformed/short input is unchanged (still no
  `fields.first() == "1"` marker check, still no strict
  `fields.len() == count + 2` requirement) -- only the truncation to one
  item is removed.
- `MoxelCell.text` and `MoxelDrawingMembers.text` changed from
  `Option<String>` / `Option<MoxelLocalizedValue>` to
  `Vec<MoxelLocalizedValue>`, so every declared language survives from
  parse through to the struct.
- `parse_moxel_formatted_cell_text` returns `Vec<MoxelLocalizedValue>` too,
  same reasoning; the parameter-vs-text discriminator (an empty leading
  `lang` marks a parameter reference, never mixed with a real text list)
  is preserved by inspecting only the first item, exactly as before.
- Both writers (`push_moxel_row_xml`'s cell-text branch,
  `push_moxel_drawing_xml`'s drawing-text branch) now loop over every item
  and emit one `<v8:item>` per language, using that item's own `lang`
  instead of a hard-coded `"ru"`.

New regression test,
`mssql_dump::moxel::moxel_exact_parity_tests::bilingual_cell_text_list_publishes_every_declared_language`,
constructs the exact real-corpus shape (`{16,2,{1,2,{"ru",...},{"en",...}},0}`)
and asserts both `<v8:item>` blocks are published with their correct
`<v8:lang>`.

## Root cause 2 (a slice of the `Title`/general-loss overlap, self-closing element): empty `<description>`

Separately, the same `languageSettings` header every MOXCEL document
carries writes one `<languageInfo>` block per configured language. When a
language has no translated display name (e.g. `en`'s `<description>` is
often empty even though the language itself is configured and its cell
text is populated), the platform self-closes:

```xml
<languageInfo>
    <id>en</id>
    <code>Английский</code>
    <description/>
</languageInfo>
```

`push_moxel_language_settings_xml` unconditionally wrote
`<description>{}</description>` regardless of whether `info.description`
was empty, producing `<description></description>` instead. This is the
same defect class already fixed twice elsewhere in this project (77e3069,
empty MOXCEL drawing string `<value>`; 1b69a37, empty `DesignTimeRef`
`<v8:Value>`) -- an unconditional open/close write where the platform
self-closes on empty content. `grep -rn '<description>{}</description>'
src/` confirmed this was the *only* unconditional (non-empty-checked)
`<description>` writer in the codebase, so it is very likely the dominant
source of the "we also write `<description></description>` instead of
`<description/>`" defect class the wave's framing measured (7,570 of the
18,487 `uh`-differing files) -- every MOXCEL document with a
`languageSettings` header is a candidate, and MOXCEL documents are exactly
where the bulk of `uh`'s `differing` set lives.

### The fix

`push_moxel_language_settings_xml`'s per-`languageInfo` loop now
self-closes `<description/>` when `info.description.is_empty()`, matching
the pattern the adjacent `push_moxel_language_text` helper (used for
`<currentLanguage>`/`<defaultLanguage>`) already followed two lines above
it in the same function.

## Verification

- `cargo test --lib`: 2251 passed / 33 failed (was 2250/33 before this
  change; the one new pass is the new regression test above). The 33
  failing names are byte-for-byte identical to `$D/fail-base.txt` -- no
  name-diff, pre-existing and unrelated to this change.
- `cargo fmt --check` / `git diff --check`: clean.
- `zsh $D/kit/bundled9.sh <worktree>`: 9/9.
- Exact-set diff against `$D/base789/<key>.parity.json` (`BROKEN` = was
  exact, is no longer exact -- the only regression signal that counts):
  `ws` 0/0 broken (29/29 exact, 100%), `mdm` 0 broken (160/164, 97.561%,
  unchanged), `wms` 0 broken (226/226, 100%), `sslbase` 0 broken
  (9573/9617, 99.5425%, unchanged), `ssl` 0 broken (12644/12701, 99.5512%,
  unchanged). All five percentages match the brief's table exactly --
  this fix does not move any of the five fast/mono-language gates, as
  expected for a defect specific to bilingual MOXCEL documents.
- Full `uh` re-run after the fix (via the now-locked `$D/kit/run.sh uh`,
  serialized against the other seven parallel exporters sharing this
  host): `native_files 140411, exact 127753, differing 11326, missing 1332,
  extra 64, percent 90.985` (was `exact 120592, differing 18487, percent
  85.885`). Exact-set diff against `$D/base789/uh.parity.json`:
  `BROKEN=0` (zero previously-exact files lost), `gained=7161` newly-exact
  files -- matching the before/after differing delta exactly
  (18487 − 11326 = 7161). `missing` (1332) and `extra` (64) are unchanged,
  confirming this fix touches only files that were already present and
  differing, nothing in the missing/extra buckets that a separate,
  unrelated investigation (`uh-missing-root-cause-map-20260825.md`) owns.

Instrumentation check (`grep -rn "PROBE\|std::env::var\|eprintln!" src
crates | grep -v tests.rs`): only the four pre-existing, unrelated uses
(password env vars in `mssql.rs`/`fetch.rs`, `IBCMD_RS_WORKERS` in
`parallel.rs`, `MEMORY_BUDGET_ENV` in `commands/cf.rs`, `eprintln!` in
`main.rs`'s top-level error renderer) -- nothing added by this change.

## Files touched

- `src/mssql_dump/moxel.rs`: `MoxelLocalizedValue` gains `Debug`;
  `MoxelCell.text` and `MoxelDrawingMembers.text` become
  `Vec<MoxelLocalizedValue>`; `parse_moxel_localized_cell_value` and
  `parse_moxel_formatted_cell_text` collect every declared item instead of
  the first; `parse_moxel_cell` and `parse_moxel_drawing_format_record`
  updated for the new shapes; `push_moxel_row_xml` (cell `<tl>`/`<tfl>`)
  and `push_moxel_drawing_xml` (drawing `<text>`) loop over every item with
  its own `<v8:lang>` instead of one hard-coded item;
  `push_moxel_language_settings_xml` self-closes empty `<description>`.
- `src/mssql_dump/tests.rs`: construction sites updated for the new field
  types (`Option<String>`/`Option<MoxelLocalizedValue>` ->
  `Vec<MoxelLocalizedValue>`); two direct assertions
  (`parses_moxel_detail_parameter_cell_variants`) updated to compare
  against the vec shape.

## What is still open

- The `Title`/`ToolTip`/`editFormat`/`Synonym`/`Presentation`/
  `dcssch:title`/`InputHint`/`Format`/`EditFormat` buckets from the
  categorization above are **out of this fix's scope, confirmed by
  location, not just by name**: `grep -rl` for these tags under a sampled
  `Reports/РегламентированныйОтчетПрибыль/` shows every one of them lives
  in `Forms/*.xml` or `Forms/*/Ext/Form.xml` (form descriptors and form
  bodies) -- explicitly "исходники форм", owned by a different executor
  per this wave's brief, not by this pass. `dcssch:title` is a DCS schema
  element (`настройки СКД в макетах`), also explicitly out of scope.
  `parse_1c_synonyms` (the general metadata-object Synonym reader in
  `src/mssql_dump/mod.rs`) was checked and is *not* first-item-only -- it
  already collects every quoted-string pair via `chunks(2)` -- consistent
  with the `Synonym`/`Presentation` occurrences here being form-property
  writers, not a second instance of this fix's defect class. After
  excluding the forms/DCS buckets, the `mask` (3,960) + `text` (1,666) +
  `tfl` (50) buckets -- all MOXCEL, all inside this fix's scope -- account
  for the overwhelming majority of the *in-scope* English-item loss.
- Post-fix full `uh` numbers landed (see Verification above):
  `BROKEN=0`, 7,161 files moved from `differing` to `exact` -- accounting
  for the whole of the in-scope `mask`/`text`/`tfl` bucket total (5,676)
  plus more, since a single MOXCEL document can carry both a cell-text-list
  loss and other, already-correct content, and the categorization counted
  *occurrences* of a missing `<v8:lang>en</v8:lang>` rather than *files*
  affected -- a file with several affected cells only needed counting
  once here. `uh` remains at 11,326 differing (90.985%, up from 85.885%);
  the still-open buckets above (forms, DCS) account for the remainder.
