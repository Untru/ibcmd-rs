# `<Shortcut>`: the modifier spelling follows the export host, 20260825

Status: root cause and fix for 643 ERP УХ forms (and 437 УТ forms) whose only
difference from the reference tree was a `<Shortcut>` line. They were
previously classified as an unfixable host dependency; they were a one-line
defect.

## What the blob holds

A host-neutral tuple `{0, virtual-key, modifier-mask}` with
`SHIFT = 4`, `CTRL = 8`, `ALT = 16`. The same `.cf` carries the same bits on
every host. Only the *spelling* changes:

| bit | Windows | macOS |
|----:|---------|-------|
| 8 | `Ctrl` | `Cmd` |
| 16 | `Alt` | `Option` |
| 4 | `Shift` | `Shift` |

The key name never changes and the modifier order is identical on both.

## What we were doing

`parse_common_command_shortcut_value` -- the single renderer every
`<Shortcut>` on the export goes through, for common commands, form fields,
`Page`, `Table` and `LabelDecoration` alike -- hardcoded `Ctrl` and `Alt`.

Every reference tree on the stand is a macOS capture: **2 900 `Cmd` and 140
`Option` across uh/ut/ssl/sslbase, and not one `Ctrl` or `Alt`.** We run on
macOS. We were writing the spelling of the host we are not on.

## Proof by substitution

БСП базовая, all 81 forms carrying `<Shortcut>`: 50 were already byte-identical
to the reference, and the remaining 31 become byte-identical under exactly
`Ctrl -> Cmd`, `Alt -> Option`, with **no residual difference in any file**.
The native vocabulary confirms the ordering is untouched -- `Cmd+Option+F7`,
`Cmd+Shift+BackSpace`, `Option+1`, bare `F5` -- each the exact image of what
the Windows-spelled renderer produced.

## Fix

`ShortcutModifierStyle::host()` picks the spelling `ibcmd` itself uses on the
host the export runs on; the renderer takes it as a parameter, so both
spellings are exercised directly by tests rather than depending on which
machine runs the suite.

## Measured effect (against `baselines/b7aa538`, BROKEN = 0 everywhere)

| key | before | after | gained |
|---|---:|---:|---:|
| sslbase | 9 573 | 9 606 | +33 |
| ssl | 12 644 | 12 684 | +40 |
| ut | 50 456 | **50 894** | **+438** |
| uh | 130 127 | **130 819** | **+692** |

УТ now stands at **99.9921 %** -- 4 differing files out of 50 898, none of
them a shortcut: two GanttChart drawings and two MXL sheet-name
localizations (`Сводная` vs `Pivot`), the latter being the *other* construct
`host-dependent-export-2214-20260823.md` documents and very likely curable the
same way.

In uh, 643 forms differed only in `<Shortcut>`; afterwards 11 files still
mention the element at all, each inside a larger structural diff.

## Why the earlier reading was wrong

`host-dependent-export-2214-20260823.md` concluded these were "not our fault"
and should be excluded from parity counting. That followed from measuring
against a Windows-captured reference. Its own framing carries the correction:
both spellings are written by the platform and each is right for its host --
so an exporter that means to reproduce platform bytes must depend on the host
exactly as the platform does. That document has been annotated rather than
replaced; its observation and its capture-manifest requirement both stand.

---

# The second construct: the chart series automatic name, same shape

The chart series name `host-dependent-export-2214-20260823.md` documents turns
out to be the same defect class, and is now closed too.

## What the document holds

A series stores its name beside a `strIsChanged` flag, and while that flag is
clear the stored name **is** the automatic name, cached in the
configuration's own language. `MOXEL_CHART_AUTOMATIC_SERIES_NAME` republished
`Pivot` over that cache whenever the document was in the republishing case --
`has_extended_scales` clear and at least one real series.

That rule was derived from the Windows round-2 capture, where those 14 series
publish `Pivot`. It is right for Windows and wrong here.

## Why the existing seeds could not catch it

The three `moxel-chart-series-count-zero` seeds were built to prove both
conditions necessary, and they do. But all three sit *outside* the
republishing case -- two keep `has_extended_scales` set, the third clears it
while leaving `realSeriesCount` at 0 -- so every one of them could only ever
show the cache, whichever host produced it. The qualifying combination was
never seeded.

## The decisive observation

УТ 11.5.27.75's `DataProcessors/ПроверкаКонтрагента/Templates/ФинансовыйАнализ`
and `Reports/ДосьеКонтрагента/Templates/ФинансовыйАнализ` *are* in the
republishing case, and the stand's macOS capture publishes `Сводная` for both
where the Windows capture published `Pivot`.

Corpus-wide, the string `Pivot` appears **nowhere** in any reference tree --
uh, ut, БСП демо, БСП базовая, WMS, MDM_Management, Web_Service -- while 28 of
their `Template.xml` publish the stored `Сводная`. Nor does it appear in any
native-evidence fixture in this repository: every seed round-tripped through
the platform on this host published the cache.

## Fix

`MoxelChartSeriesAutomaticName::host()`, alongside `ShortcutModifierStyle`:
Windows republishes in English, macOS publishes the cache. The two document
conditions keep their meaning and their evidence untouched -- they still decide
whether the document is in the republishing case; the host decides what is then
written. A test pins both hosts and both branches directly.

Note the two readings of macOS behaviour -- "never republishes" versus
"republishes a name that happens to equal the cache" -- are indistinguishable
on this corpus, because with `strIsChanged` clear the cache is by definition
the automatic name in the configuration's language. Publishing the cache is
correct under either, so the fail-closed choice costs nothing.

## Measured effect (against `baselines/b7aa538`, BROKEN = 0 everywhere)

| key | before | after |
|---|---:|---:|
| ut | 50 894 | **50 896** |
| uh | 130 819 | **130 820** |

УТ now stands at **99.9961 %** -- 2 differing files out of 50 898, both
GanttChart drawings, which are out of scope. Cumulatively on `b7aa538`:
ut +440, uh +954, sslbase +33, ssl +40.
