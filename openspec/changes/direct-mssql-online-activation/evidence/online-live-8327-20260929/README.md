# Online and live activation on 8.3.27.2214: evidence of 2026-09-29 (issue #344, track "trace")

Read `docs/apply/online-activation.md` first: it states the results, the measured/hypothesis split and the review findings
(F-1 ... F-17). This folder holds the files behind it. Everything was made with the binary built from branch
`feat/0.5-online-evidence` (base `3dd0b4db`) on two disposable clones of the БСП 8.3.27 lab corpus, registered in the
8.3.27 cluster (`localhost:2541`, RAS `localhost:2545`) and unregistered afterwards. `worker` was not run.

| clone | use | start state |
|---|---|---|
| `ibcmd_rs_05_online_a1` | ONLINE evidence (two generations), then the platform-level check of finding F-4 (`live` on it, FULL recovery) | the corpus with its native dynamic leftover (markers + 5 alias rows) removed by `online/prep-normalize.sql`, so that the first online generation runs in one process |
| `ibcmd_rs_05_online_b1` | LIVE evidence (FULL recovery, base backup) | the corpus as it is (native dynamic history present) |

## Chronology (UTC)

| time | what | files |
|---|---|---|
| 15:48 - 16:22 | first attempts on `a1` as restored: dry run OK; `exclusive` refused after staging (F-2), then twice by the session gate (F-3) | `online/baseline-*` |
| 16:23 - 16:27 | `a1` normalized (no markers, no aliases, empty `ConfigSave`) | `online/prep-normalize.sql` |
| 17:00 - 17:08 | observer sessions: WARM-A (poll), WARM-B (txn), LAZY (lazy) | `journals/on-*.log.gz` |
| 17:08:35.690 | **ONLINE generation 1 committed** (`apply --mode online`, one process); NEW-1 launched 17:08:36.4, NEW-2 17:11:14 | `online/online-v1.*` |
| 17:12 - 17:23 | re-apply of the same module: fails (F-1), 250 s and 303 s | `online/online-v2-single.*`, `online-v2-dry.*` |
| 17:25 | no-op dry run of another module on a base with history: fine, 10.6 s | `online/online-noop-B-dry.*` |
| 17:29 | WARM-C started; apply of another module, single process: refused after staging (F-2) | `online/online-v2b-single.*` |
| 17:32:07.480 | **ONLINE generation 2 committed** (`mssql-activate-staged-main` in a fresh process); NEW-3 launched 17:32:08 | `online/online-v2b.*` |
| 17:43 | observers stopped, sessions terminated, samplers stopped | |
| 17:44 - 18:27 | **LIVE** on `b1`, five attempts (all refused by the readiness gate), two manual second cycles, the no-op retry, and the broken-log-chain check | `live/*` |
| 18:17 - 18:23 | `a1` set to FULL, `live` of an unrelated module, then a fresh session: F-4 | `live/h1-*`, `analysis/h1-sessions.txt` |
| ~18:29 | both clones unregistered from the cluster | |

## Files

- `online/online-v1.{json,sql,meta.txt}`, `online-v2b.*`: the report of the tool, the SQL script as rendered, wall-clock
  bookends (`meta.txt`). `*.recovery-summary.txt`: the recovery artifact reduced to its structure (the real ones carry the
  row bytes as JSON number arrays, 1 MB each, and stay in the lab folder). `*.diff.md`: the before/after snapshot diff of
  the kit (`scripts/apply-trace`), `*.trace-summary.md`, `*.transactions.tsv`, `*.service-writes.tsv`: the Extended Events
  trace of the command (database-filtered).
- `online/*.err`, `*.meta.txt`: the refusals (F-1, F-2, F-3); `overlay-query.sql`: the overlay query and the plain one
  measured with `SET STATISTICS IO, TIME` (4 762 logical reads and 34.5 s against 3 reads and 1 ms, under load).
- `live/live-v*.{err,meta.txt,sql}`: the five LIVE attempts and the broken-chain check (`live-v6-nochain`); `live-v1.diff.md`,
  `live-v1.trace-summary.md`, `live-v1.transactions.tsv`, `live-v2.diff.md`; `live-v2-retry.json` (the no-op retry);
  `manual-second-cycle*.sql` (the statements of the second cycle, run by hand); `h1-*`: the F-4 check; `gen-state-final-b1.txt`,
  `h1-storage-b1.txt`: the generation state and the orphaned alias rows after the LIVE runs on `b1`.
- `journals/<label>.log.gz`: the observer journals, one line per second per session:
  `ms since 0001-01-01 UTC | label | event | 1C session number | client value | server value | details`
  (`details`: `B=<client>/<server>` of the second module, the client-side draft, its modified flag, or the error text).
  Convert with `datetime(1,1,1) + timedelta(milliseconds=ms)`. Labels: `on-*` ONLINE, `lv-*`, `lv2-*`, `lv3-*`, `lv5-*` LIVE
  attempts 1, 2, 3, 5; `h1-*` F-4.
- `timelines/<run>-timeline-sql.tsv.gz` (database state and the number of `1CV83 Server` and other SQL sessions, from
  `master`, change-driven, 100 ms), `-timeline-rac.tsv.gz` (sessions and connections of the infobase through RAS, 1 s),
  `-load.tsv.gz` (CPU %, free RAM, 5 s). Runs: `online`, `live` (attempts 1-3), `live5`, `h1`.
- `analysis/*.txt`: the output of `scripts/apply-trace/lab/online-live/analyze_obs.py` per attempt (marker segments, gaps,
  errors, relative to the commit or the start of the single-user window).
- `observer/IbcmdRsObserver.epf`: the external data processor (sources and build script:
  `scripts/apply-trace/lab/online-live/observer/`).

## Not in this folder

The full capture folders (`before`/`after` snapshots, blob store, raw events) stay in `F:\ibcmd\lab\05\online\runs\*\cap\`;
the tail-log files (`*.trn`, 62 MB for the first cycle) and the base backups are deleted at the end of the task.
