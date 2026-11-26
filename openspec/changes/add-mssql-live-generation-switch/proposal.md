# Change: add MSSQL live generation switching

## Why

The existing `online` publication stores a dynamic generation but intentionally
keeps already-connected sessions on their loaded code.  Controlled 8.3.27
tests show that publishing the ordinary generation followed by a lossless
tail-log `NORECOVERY`/`RECOVERY` cycle makes the same 1C session observe the new
client and server code substantially faster than restoring a full database.

## What Changes

- Add `live` as an explicit main-configuration activation mode.
- Promote the bounded ordinary `Config` cohort and normalize matching
  `DynamicallyUpdated` markers before the recovery cycle.
- Require a new, retained SQL Server tail-log artifact for every live write.
- Force a bounded `SINGLE_USER -> BACKUP LOG WITH NORECOVERY -> RESTORE WITH
  RECOVERY -> MULTI_USER` transition and report its recovery instructions.
- Keep existing `online` and `exclusive` behavior compatible; reject `live`
  for extensions until independently evidenced.
- Replace the conservative fixed inter-recovery delay with a bounded readiness
  gate, and fail online before the second recovery when the original SQL
  connection cohort has not reappeared.
- Add a `worker` activation mode that promotes the bounded ordinary generation
  and hands a verified dedicated development `rphost` over to a replacement,
  without putting the database into `RESTORING`.
- Add an editor-watch workflow for `worker` mode that debounces a selected
  source closure and serializes guarded activation after every stable save.

## Impact

The CLI, main activation plan/report, SQL renderer, recovery artifact, and live
integration evidence change.  Native `ibcmd` and Designer remain absent from
the product path.
