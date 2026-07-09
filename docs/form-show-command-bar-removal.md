# Form ShowCommandBar Removal

Date: 2026-07-09

Removal commit: `1877326` (`Disable form ShowCommandBar XML support`)

What changed:

- `Form.xml` export no longer emits `<ShowCommandBar>`.
- `Form.xml` import no longer parses `<ShowCommandBar>` as a root form property.
- The form layout patcher no longer writes the former `fields[18]` / property-bag slot from this XML tag.

Reason:

The field was inferred from layout slots, but real `ibcmd` exports inspected so far do not show a `ShowCommandBar` XML property. Emitting it can create non-native diffs and may change form command bar visibility.

Restore:

```powershell
git revert 1877326
```

Earlier related commits:

- `c40ef11` - introduced fixed-form command bar visibility round-trip.
- `8566b99` - added report-form command bar visibility round-trip.
- `dab6c0f` - suppressed default `ShowCommandBar=true` while still supporting explicit `false`.
