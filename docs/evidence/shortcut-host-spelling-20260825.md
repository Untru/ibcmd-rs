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
