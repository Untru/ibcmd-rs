# Opt-in compact LIVE recovery envelope

Already-staged `mssql-activate-staged-main --mode live --live-checkpoint`
can additionally select `--live-compact-recovery`. The option requires the
explicit checkpoint and the existing exact platform/SQL/readiness contract.
It does not enable source orchestration, warm readiness or worker activation.
Without the option the historical embedded LIVE format-1 bytes are unchanged.

Format 2 separates the LIVE identity envelope from the snapshot. Preserve the
LIVE manifest, its exact adjacent `ibcmd-live-<token>.recovery.json` sidecar,
the content-addressed binary pack, the retained SQL and the tail log together.
The ordinary standalone recovery manifest can share the same immutable pack.
The sidecar uses the existing compact recovery representation and retains all
physical row headers, binary lengths and digests. No binary JSON arrays are
stored in the LIVE envelope or compact sidecar.

Before cycle 1, publication validates the LIVE artifact and snapshot budgets,
publishes the synchronized complete pack and recovery sidecar, then publishes
the envelope with no-clobber semantics. The exact readback must reconstruct
the admitted artifact before the caller executes SQL. Failed publication can
leave complete unreferenced sidecars; no automatic deletion removes them.
Directory power-loss durability is not claimed.

The continuation reader accepts both legacy format 1 and compact format 2.
For format 2 it checks the envelope digest, strict fields, exact token-derived
adjacent sidecar name, sidecar digest, database/mode binding, bounded regular
files, complete pack/row digests and ranges, and the reconstructed historical
snapshot token. The in-memory artifact remains format 1, so backup names,
recovery token spelling and the existing continuation SQL remain unchanged.
File integrity checks finish before a SQL connection or RAC command starts;
the existing server/database/registration/log-chain checks still govern execution.

The envelope and recovery sidecar are each bounded at 2 MiB. Legacy input remains
bounded at 64 MiB. The compact snapshot retains the existing 128-row, 16-MiB-row,
32-MiB-per-set and 96-MiB-pack budgets. No stage admission budget is expanded.

Focused regressions cover exact legacy/compact reconstruction and unchanged SQL,
headers and backup names, identical publication, missing/altered sidecars,
rechecksummed foreign paths/database/mode, unknown identity fields, corrupt packs
refused by the actual continuation entry point before connection, conflicting
output preservation, invalid input before publication, and binary envelope size.
The first connected test run safely refused an oversized replacement pack at
the file-budget boundary, while its assertion expected the later digest error;
the regression now checks that the entry point returns the same file refusal.

These are representation and dispatch regressions. Actual format-2 LIVE cycle
acceptance, operational recovery, power-loss controls and generic guarded undo
remain unproved until their own reviewed laboratory evidence is recorded.
