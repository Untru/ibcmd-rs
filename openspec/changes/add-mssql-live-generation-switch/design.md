# MSSQL live generation switching

## Decision

`live` is a separate activation mode.  It does not redefine `online`, whose
contract remains generation pinning for existing sessions.

The live transition runs only for the main configuration on the evidenced
MSSQL 8.3.27 profile:

1. Compile and stage the bounded existing module or managed-form cohort.
2. Preflight FULL or BULK_LOGGED recovery, absence of dirty staging and an
   explicitly named tail-log destination that does not already exist.
3. Recheck the optimistic snapshot while the database remains online, promote
   ordinary `Config` rows, delete the
   paired `Config.DynamicallyUpdated` and `Params.DynamicallyUpdated` markers,
   clear `ConfigSave`, and commit.
4. Perform a first tail-log `NORECOVERY`/`RECOVERY` cycle so rphost observes
   the committed ordinary generation.
5. Perform a second cycle that advances existing sessions to the prepared
   generation. Both log backup sets use `CHECKSUM` and compression and are
   retained in the same operator-owned artifact.
6. Restore `MULTI_USER` access after each cycle without copying a full backup.

The interval between cycles is readiness-driven, not a fixed sleep. The SQL
script records the target database's pre-transition 1C connection cohort and
waits only until the cohort has reconnected and remained stable. If it does not,
the script throws while the database is online and does not start the second
cycle. Testing showed that a hibernated client may hold no SQL connection to
reconnect, and an active client can race the first `RESTORING` interval. Thus
`live` remains an evidenced recovery protocol, not the editor-watch default.

`worker` is the editor-watch activation mode. It writes the already guarded
ordinary promotion without a recovery cycle, then asks the cluster to turn off
the old working process. Before staging and again before the SQL commit, the
tool requires exactly one process for the target infobase and rejects any
non-zero foreign infobase on that process. The old assignment is checked once
more after the commit before signalling it. The command polls for the target
infobase on a replacement process and reports both process UUIDs and handoff
latency. A shared process is refused before source or SQL mutation.

Editor watch fingerprints only the selected existing source closure, debounces
atomic-save rename/write bursts, serializes activations, and reports each JSON
result. A failed activation is not retried until another save. Extensions are
rejected in `worker` mode because their dynamic load path has a different
contract.

The log artifact is part of the database backup chain and MUST NOT be silently
deleted or overwritten.  Failure before a successful tail backup leaves the
database online and returns the bounded row recovery artifact.  Failure after
`NORECOVERY` reports `RESTORE DATABASE <db> WITH RECOVERY` as the mandatory
operator action.

Database snapshots are rejected: snapshot revert can discard concurrent data,
rebuilds the log, breaks the log-backup chain, and removes full-text catalogs.
Direct mutation of SQL boot pages or recovery identifiers is also rejected.

## Verification

Unit tests cover CLI parsing, mode rejection, SQL ordering and escaping,
marker normalization, existing-file refusal, recovery reporting, and unchanged
online behavior.  The disposable 8.3.27 integration test proves that PID and
1C session UUID remain stable while both client and server markers change, and
records live versus full-restore timings.
