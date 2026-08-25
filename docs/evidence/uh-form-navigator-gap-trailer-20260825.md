# ERP УХ form-root Navigator-gap trailer, 20260825

Status: root cause found and fixed for six of this pass's assigned tag
buckets (`MobileDeviceCommandBarContent`, `SaveWindowSettings`, `Group`,
`VerticalSpacing`, plus the closely related `ConversationsRepresentation`/
`VerticalAlign`/`ChildrenAlign`/`ScalingMode`/`HorizontalAlign`/
`ShowCloseButton`), shipped in three commits (`fc9d418`, `68dc327`,
`d70ef54`). `Field` and `DataPath` are **not** fixed by this pass -- see
"What is still open" below for their (different, more entangled) causes.

## Method

Snapshot of the `uh` `differing` set at `d0457a6` (`cf export` + `parity2.py`
against `$D/cap/uh-r1/src`): 18,487 differing files total, 2,784 of them
form bodies (`**/Ext/Form.xml`, verified against the task's stated count
exactly once the file filter was corrected to `endswith("/Ext/Form.xml")`
rather than `"/Forms/" in path` -- the latter silently excludes `CommonForms/*`,
which has no `Forms/` path segment). For each differing form, diffed native
vs. ours line-by-line (`difflib.SequenceMatcher`) and grouped by the set of
XML tags touched. Files whose native/ours line-count ratio fell outside
`[0.5, 2.0]` (`LARGE_STRUCTURAL`, ~261 files) or whose only touched tag was
`AutoURL` (195 files) were excluded -- both are separate, explicitly
out-of-scope defect classes owned by other sessions.

## Root cause

Fourteen-plus `mssql_dump::form_body` functions locate the form root's
24-member trailer via `form_root_child_items_tail_start` (root `"50"` only,
trailer length exactly `24`), then read fixed slots from it. Forms whose
root carries a built-in Navigator/quick-search child item (present whenever
the platform gives a list/choice form its default search bar) write **one
extra field ahead of the trailer's own start** -- a 25-member trailer, not
24. The strict `[24]`-only search validates its result by requiring the
root child-items count-list (`{count, uuid1,value1, uuid2,value2, ...}`) to
end *exactly* at the trailer's own start; for these forms the count-list
actually ends one field earlier, so the search finds no valid count-list at
all and returns `None` -- silently dropping every property it gates, no
error (doctrine point 2/6: a silent default, not a typed refusal).

Confirmed on real ERP УХ 3.2.12.6 bytes across a dozen-plus independent
forms spanning `BusinessProcesses`/`Catalogs`/`CommonForms`/
`DataProcessors`/`Documents`/`InformationRegisters`: `fields.len() - 25`
validates cleanly via the existing shared count-list scan
(`form_root_child_items_tail_start_at`) where `fields.len() - 24` finds
nothing, and every trailer slot the fixed 14+ callers read shifts by
exactly `+1` (confirmed independently at trailer positions 6, 9, 10, 11,
12, 13, 14, 18, 19, 20, 21 -- a uniform, mechanical consequence of one
field being prepended, not something that varies per property). The one
content that is *not* a fixed-offset-from-front read
(`MobileDeviceCommandBarContent`'s own nested tuple) sits at a fixed offset
from the trailer's **end** instead (`trailer.len() - 2`), which is
naturally shift-invariant.

## The fix

Added `form_root_child_items_tail_start_50_with_navigator_gap` (root `"50"`
only, trailer `[24, 25]`) as a narrow, separate entry point -- mirroring
the existing `form_root_child_items_tail_start_49_or_50` precedent's own
rationale (`form_root_child_items_tail_start` has a dozen-plus callers,
most reading a fixed slot with no per-property re-check of their own;
broadening the shared function's admitted shapes activates every caller at
once, which is how `form_root_child_items_tail_start_49_or_50` broke
`Catalogs/СправочникиБД/Forms/ФормаСписка`'s `HorizontalAlign` previously).
A small helper, `form_root_trailer_slot_with_navigator_gap(trailer_len,
slot_in_24_shape)`, centralizes the `+0`/`+1` shift for the two
direct-offset readers (`HorizontalAlign`, `ShowCloseButton`, `ScalingMode`);
the schema-based readers (`FormRootGroupSchema`, `FormRootGroupingSchema`,
`FormRootConversationsRepresentationSchema`, `FormRootVerticalAlignSchema`)
each grew their own `_WITH_NAVIGATOR_GAP` slot constants and now branch on
`trailer_field_count` the same way `FormRootMobileDeviceCommandBarContentSchema`
does for its own (end-anchored) content slot.

Explicitly **not** touched: `FormRootAutoUrlSchema`/`extract_form_auto_url`
(a separate session owns the `AutoURL` bucket and its own `trailer.len() !=
24` defect -- structurally similar but a different code path, left
untouched per the handoff) and any root-`49` behavior (the one root-`49`
sample checked while validating `ShowCloseButton` has a *different*,
unverified extra-field shape and stays out of scope, matching this
codebase's existing precedent of not extending root-`49` behavior without
separate verification -- see `form_root_child_items_tail_start_49_or_50`'s
doc comment).

## Verified

Each commit: `cargo test --lib` fail-base unchanged, name-for-name, against
`$D/baselines/d0457a6/fail-base.txt` (the **pinned**, immutable snapshot --
`$D/base789` is a moving pointer the coordinator re-pins after every merge
and must not be used for comparison); `bundled9` 9/9; `cargo fmt --check`
and `git diff --check` clean; `ws`/`mdm`/`wms`/`sslbase`/`ssl` `BROKEN=0`
against `$D/baselines/d0457a6/<key>.parity.json` (all five `gained=0` --
none of them happen to carry a root-`50` form with this exact shape); `ut`
`BROKEN=0`, `gained=0` (ditto); `uh` (first commit only, re-run for the
later two pending) `BROKEN=0`, `gained=150`.

Real-byte regression tests (fixtures under
`tests/fixtures/native-evidence/8.3.27.2214/form-*-navigator-gap/`, each
with a `manifest.json` evidence trail): 14 tests across
`form-mobile-device-command-bar-content-navigator-gap`,
`form-save-window-settings-navigator-gap`,
`form-root-grouping-navigator-gap` and
`form-root-alignment-navigator-gap`, all passing.

## What is still open

- **`Field`** (104 files, this pass's other assigned bucket): **not** the
  same root cause. Diffing all 104 shows at least four independent
  sub-causes tangled together:
  - `tilde_reorder` (46 files): a dynamic list's `<UseAlways>` English-name
    twin (e.g. `~Список.DeletionMark` for a query that selects the Russian
    `ПометкаУдаления`) is missing its `~` marker, which cascades into an
    apparent reorder purely because `use_always.sort()` is a plain string
    sort and `~` (0x7E) sorts before Cyrillic bytes -- fixing the marker
    would fix the order for free, no separate reorder logic needed. Traced
    the marker bug to `form_dynamic_list_main_table_auto_fields` and
    `form_dynamic_list_selected_standard_twins` (`mssql_dump::form_body`)
    both unconditionally adding the field's Russian *and* English names to
    the tilde-decision "universe" whenever the main table's own standard
    attribute is selected under its own name -- contradicting the native
    evidence here, though the same logic was validated at "4356 of 4357"
    UseAlways observations on the *original* UT corpus. Not fixed: this
    touches shared, heavily-used dynamic-list machinery: any change needs
    verification across a UT corpus this large before it can ship, which
    this pass did not have room for. The distinguishing condition between
    the working UT cases and these failing UH cases is not yet identified
    (candidate: a preceding temp-table `ПОМЕСТИТЬ` batch ahead of the
    query's real `SELECT`, present in at least the one form traced in
    detail, `Catalogs/ЗакупочныеПроцедуры/Forms/ФормаСписка`).
  - `missing_fields` (45 files): fields entirely absent from `<UseAlways>`,
    heavily concentrated in `Константы`/`НаборКонстант` (constant-set
    attribute) names -- likely the same universe-computation family as
    above but not traced further.
  - `order_missing` (5 files): 2 are the dynamic list's own `-1` (`Order`)
    pseudo item id, fixed narrowly in this pass (mirrors the pre-existing
    `-3` (`Group`) convention, see `form_dynamic_list_use_always_field_name`
    in `fc9d418`) but **not independently verified end-to-end** -- the
    verification harness (`extract_form_body_xml` with empty
    `object_refs`) cannot resolve `<MainTable>` for auto-query lists, so
    this fix relies on the `uh` corpus gate rather than a fixture test for
    proof; check the `uh` gate's `gained` set for
    `Catalogs/ПравилаГруппировкиАктивовОбязательств/Forms/ФормаСписка` and
    `Catalogs/ДокументыПодтверждающиеЛьготыПоИмущественнымНалогам/Forms/ФормаСписка`
    specifically before trusting it. The other 3 are an unrelated
    `Настройки.Settings.*` (`ReportSettings`-typed attribute) `UseAlways`
    block missing ~22 entries entirely, in one `InformationRegister`
    family (`НастройкаЗаполненияФормСтатистики`/
    `НастройкаЗаполненияСвободныхСтрокФормСтатистики`) -- not traced.
  - `concat_name` (1 file) and `extra_fields`/`other` (7 files): not
    traced.
- **`DataPath`** (50 files): also multiple sub-causes, at least three
  identified, none fixed:
  - The same `tilde`/twin universe bug as `Field` above, on item
    `<DataPath>` rather than `<UseAlways>`'s `<Field>` (roughly a third of
    the 50).
  - A `<Button>`'s own `<DataPath>` (e.g. a Report command bound to
    `Объект.Ref` as its context) is unconditionally excluded by
    `format_form_child_item_xml`-family code
    (`item.tag != "Table" && item.tag != "Button"`,
    `src/mssql_dump/form_body.rs` around line 23411) with no evidence
    comment attached -- native clearly writes one for at least
    `Catalogs/КлассыВНА/Forms/ФормаЭлемента`'s `AutoCommandBar` button. Not
    investigated further: needs real-byte evidence on how many/which
    buttons *do* carry a `DataPath` (this one's command needs report
    context; most buttons plausibly don't) before loosening the exclusion.
  - Standard-attribute items (`Объект.Description`/`Объект.Code`, the
    platform's auto-generated "code+name" `UsualGroup`) lose their
    `DataPath` specifically when nested one level inside a `UsualGroup` --
    confirmed via raw-byte comparison that the *item's own* wrapper
    (`{37,{id,uuid},...,{2,{1},{-3}},...}`, matching
    `resolve_form_strict_field_model_data_path`'s expected shape exactly)
    is byte-identical whether the item sits at the form root or nested in
    a group, and a *sibling* item using the same owner attribute id `"1"`
    at the top level resolves fine -- so `attribute_metadata_owners_by_id`
    itself is not the problem. The likely culprit is
    `strict_field_data_path`'s `field_schema_and_options.is_some()` gate
    (`src/mssql_dump/form_body.rs` ~line 9520) or how nested group children
    get parsed/threaded recursively, but the actual recursive call site
    that parses a `UsualGroup`'s own `ChildItems` was not located in this
    pass. Confirmed present in at least 3 of the 12 samples checked
    (`Catalogs/ВидыВременныхРазниц/Forms/ФормаГруппы`,
    `Catalogs/ВидыОперацийУчетаМСФО/Forms/ФормаЭлемента`,
    `Catalogs/КатегорииЗакупок/Forms/ФормаЭлемента`).
  - `AccumulationRegisters/ПланыПроизводства/Forms/ФормаРедактированияПолуфабриката`
    is missing table-row-context paths
    (`СписокКорректировок.LineNumber`, `Items.СписокКорректировок.CurrentData.Period`)
    -- a fourth, distinct shape, not traced.
- **`Shortcut`** (615 files, the single largest tag-diff bucket by far):
  **not a defect** -- `docs/evidence/host-dependent-export-2214-20260823.md`
  already documents this as host-dependent noise on `ut` (accelerator-key
  assignment order depends on install history, not configuration content);
  the same applies here and this bucket should stay untouched.
- Long tail beyond the above: ~1,873 files with multi-tag or small-count
  signatures, not yet grouped/triaged (this pass's classification script,
  `classify_form_tags.py`-equivalent logic, groups by touched-tag-set; the
  next pass should start from the signature-size histogram in this
  document's sibling analysis rather than re-deriving it).

## Files

- Code: `src/form_schema.rs`, `src/mssql_dump/form_body.rs`.
- Fixtures: `tests/fixtures/native-evidence/8.3.27.2214/form-mobile-device-command-bar-content-navigator-gap/`,
  `.../form-save-window-settings-navigator-gap/`,
  `.../form-root-grouping-navigator-gap/`,
  `.../form-root-alignment-navigator-gap/` (each with its own
  `manifest.json`).
- Commits: `fc9d418`, `68dc327`, `d70ef54`.
