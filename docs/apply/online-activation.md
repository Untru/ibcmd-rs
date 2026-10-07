# Online, live and worker activation of one main-configuration change: what each mode writes, who sees it and when

Issue [#344](https://github.com/Untru/ibcmd-rs/issues/344) ("Довести direct-mssql-online-activation: свидетельства,
ревью, документация"), track "trace", milestone 0.5. OpenSpec changes `direct-mssql-online-activation` (tasks 1 and 7)
and `add-mssql-live-generation-switch`. Companion of `native-apply-trace.md` (#336: what the *native* apply writes).

Scope: `ibcmd-rs mssql-apply-source-change` and `mssql-activate-staged-main` with `--mode exclusive|online|live|worker`,
one existing module body (`CommonModules/<name>/Ext/Module.bsl`) of the **main** configuration, platform 1C:Enterprise
8.3.27.2214, Microsoft SQL Server 2025 (17.0.1135.8), the БСП 3.1.11 demo configuration with its four extensions (2 234
tables), two disposable clones of the lab corpus, 2026-09-29. The binary was built from this branch
(`feat/0.5-online-evidence`, base `3dd0b4db`). Extensions (online/exclusive only) are **not** covered here; their
evidence is `evidence/extension-live-validation.md` and `evidence/extension/protocol.md`.

Every number is **measured** unless the text says **hypothesis**. The workstation was shared with six other tracks during
every run: total CPU 98 % on average (minimum 65 %), free RAM between 3.3 and 26 GB
(`evidence/online-live-8327-20260929/timelines/*-load.tsv.gz`). Timings of client start-up and of every SQL step are
therefore upper bounds of what an idle machine gives; where the load matters for the *outcome* (section 4.3) it is said.
**WORKER was not run in this issue** (rule: it restarts a working process of the shared 8.3.27 cluster); section 4.4 is
the 2026-08-30 evidence, re-read, nothing new.

The evidence (journals of the observer sessions, timelines, SQL scripts as rendered, snapshot diffs, traces) is in
`openspec/changes/direct-mssql-online-activation/evidence/online-live-8327-20260929/` (index there); the scripts that
made it are `scripts/apply-trace/lab/online-live/`.

## 1. Summary

1. **ONLINE works as designed, and is invisible to running sessions.** One `mssql-apply-source-change --mode online`
   wrote 4 new `Config` rows (module owner and body under `<uuid>_dynupdate_<generation>`, `versions_dynupdate_<generation>`,
   the `DynamicallyUpdated` marker), 1 new `Params` row (`DynamicallyUpdated`) and replaced `root`/`version` with the same
   bytes; the ordinary module rows were not touched. The database stayed `ONLINE`/`MULTI_USER` in all 703 samples of the run.
   An already-open session (used **or not yet used** the module) kept the old code on both the client and the server side for
   as long as it was watched (34 minutes after the first generation, 11 minutes after the second); a session opened after
   the commit ran the new code on both sides at its first call. A 60-second server transaction that spanned the commit
   committed normally. Details and the matrix: section 4.1/4.2.
2. **LIVE did not complete once in five attempts** (section 4.3). Every attempt was refused by the readiness gate
   (`57234`, `src/mssql_main_activation.rs:607-609`) after the **first** recovery cycle, with the ordinary promotion already
   committed. What the sessions then see depends on the attempt: no error and a half-switched configuration (client and
   server disagree), or a modal DB error that needs "Перезапустить"/"Завершить" and loses the client's unsaved data; an
   in-flight server transaction is rolled back and its client gets an unrecoverable error. **A retry of the same command is a
   no-op** (`executed=false`, 2.0 s) and never runs the second cycle. The historical success
   (`add-mssql-live-generation-switch/evidence/live-stable-v7.md`, 2026-08-30) predates the gate and was one idle client.
3. **Two defects block the second apply on a database that has an online generation** (section 6): a re-apply of the *same*
   object fails after 250-300 s under load with "selected storage closure did not emit required source body"
   (`mssql_dump/mod.rs:3762-3765`, `44820-44834`); an apply of a *different* object stages and is then refused with "Params
   dynamic ordinary generation disagrees with versions" because the activation reads `Config` through a process-global
   dynamic overlay (`mssql_dump/dynamic_generation.rs:139-157`, `mssql.rs:957`). The workaround for the second one is to run
   `mssql-activate-staged-main` in a fresh process.
4. **`exclusive`, `live` and `worker` silently discarded earlier online generations** (measured for `live`; the three modes
   share `render_ordinary_transition`). Their ordinary promotion replaces
   only the staged rows and deletes both markers; the `_dynupdate_` alias rows stay in `Config` as orphans, so what the
   earlier online changes published is gone from the effective configuration. Measured on a session (section 6, F-4): after a
   `live` promotion of an unrelated module, a new session sees the original text of the module changed by the first online
   generation and no longer finds the function that the second online generation added. Native `exclusive` apply *merges* the
   overlay (`native-apply-trace.md`, section 5); ours did not. **Fail-closed by #408 step 1:** these modes refuse
   before any write when a marker or a `_dynupdate_` row exists (`mssql_main_activation.rs`, `online_history_refusal`,
   SQL code `57208`). **Fixed for `exclusive` by #408 step 2 (0.5):** `mssql-activate-staged-main` and `mssql-apply-source-change`
   hand the `exclusive` mode to `mssql_config_apply`, which folds the `_dynupdate_` rows into the ordinary rows as the native apply does (section 6.6).
   `live`, `worker` and `exclusive` with `--sqlcmd` keep the refusal (the worker lab cluster now exists, `docs/apply/worker-lab.md`; the fold for `live` and `worker` is still to be done, and their lab runs use a marker-free clone).
5. **`exclusive` cannot run against an infobase that has users** (F-3; fixed in 0.5, section 6.2): the tool's own RAS
   verification (`rac infobase info --infobase-user=...`, `mssql_platform_profile.rs:211-227`) opens two `1CV83 Server` SQL
   sessions that the gate (`57209`) then refused.
6. **The `live` preflight did not protect the commit** (F-9) and **`live` interrupted sessions without asking** (F-10): with a broken log chain, a missing or unwritable tail directory the promotion was
   committed and only then `BACKUP LOG` failed (`4214`, `3201`); sessions with open transactions were rolled back with no word. **Fixed in 0.5 (#409), section 6.7:** the gate of the live mode runs before the
   stage, before the script's transaction and again before its `COMMIT`; `--interrupt-sessions` is the operator's acceptance.
7. **Timing (ONLINE, measured):** 10.4 s in the tool (active export 2.0, classification 0.07, staging 1.0, activation
   2.7 s), 11.8 s wall; the activation transaction itself is 190.8 ms; activation alone in a fresh process 2.1 s.

## 2. The four modes on one page

| | `exclusive` | `online` | `live` | `worker` |
|---|---|---|---|---|
| ordinary `Config` rows of the staged names | replaced (`DELETE`+`INSERT`) | **kept**; `root`, `version` replaced (same bytes) | replaced | replaced |
| what is written instead | | `<name>_dynupdate_<gen>` alias rows, `versions_dynupdate_<gen>` | | |
| `Config`/`Params` `DynamicallyUpdated` markers | both deleted (exactly one each if present) | both written/extended | both deleted | both deleted |
| `_dynupdate_` alias rows of earlier generations | **left as orphans** | kept, listed in the marker | **left as orphans** | **left as orphans** |
| gate before writing | no other user session in the database (`57209`) | none | recovery model FULL/BULK_LOGGED, database ONLINE, tail-log file absent | dedicated worker process (exactly one process serves the infobase, no foreign infobase on it) |
| after the commit | nothing | nothing | two `SINGLE_USER` -> `BACKUP LOG ... NORECOVERY` -> `RESTORE ... WITH RECOVERY` -> `MULTI_USER` cycles | `rac process turn-off` of the old working process, wait for a replacement (10 s) |
| open sessions | none may exist | keep their generation (measured) | expected to switch without reconnecting (design); see 4.3 | expected to reattach through the new process (2026-08-30 evidence) |
| extensions | yes (CAS path) | yes (CAS path) | **rejected** | **rejected** |
| status in this issue | cannot run on a base with users (F-3) | measured | measured, 0/5 completed | not run |

All modes share the classifier and the staging: only an existing module body (`CommonModules/*/Ext/Module.bsl`) or an
existing common-form body is admitted (`mssql_apply.rs:591-619`), the change must not touch any other source path, the
staged rows are `<owner uuid>`, `<owner uuid>.0`, `root`, `version`, `versions` (5 rows; `versions` is 344 KB on the
БСП), and the whole publication is one `SERIALIZABLE` transaction under `sp_getapplock 'ibcmd-rs:main-activation'`.

## 3. What each mode writes

### 3.1 The transaction (all modes)

`render_main_activation_sql` (`mssql_main_activation.rs:316-419`), rendered script of the measured ONLINE run:
`evidence/online-live-8327-20260929/online/online-v1.sql`. In order:

1. `SET TRANSACTION ISOLATION LEVEL SERIALIZABLE; BEGIN TRANSACTION; sp_getapplock(... Exclusive, LockTimeout=0)` (`57200`
   when busy).
2. Two expected tables: the staged rows (`ExpectedStage`) and the **current** ordinary rows of the same names
   (`ExpectedActive`), each with `DataSize` and `SHA2_256`. `ConfigSave` must equal `ExpectedStage` exactly (`57201`); every
   `ExpectedActive` row must still be in `Config` with the same size and hash (`57202`); the two markers must be exactly as
   read (`57203`/`57204`: present with the same bytes, or absent).
3. The mode's transition (3.2 - 3.4), each row checked with `@@ROWCOUNT` (`57210`, `57212`, `57215`, `57216`).
4. `DELETE FROM dbo.ConfigSave` (`57220`), then the postconditions: the published rows read back with size and hash
   (`57222`), the markers as expected (`57223`-`57226`), `ConfigSave` empty (`57221`); `COMMIT`. Any error rolls back
   (`XACT_ABORT`, `CATCH`).

The staged rows come from `ConfigSave`, written earlier by the staging step in its own transaction (368.8 ms in the
measured run); a failure after that leaves the stage in `ConfigSave` (F-2 left exactly that; since 0.5 the checks that
need no staged row run before the stage, section 6.1).

### 3.2 `online` (`render_online_transition`, `mssql_main_activation.rs:448-478`)

Measured on a database without dynamic history (`online/online-v1.diff.md`, `online/online-v1.transactions.tsv`):

| `Config` | change |
|---|---|
| `313d9858-...b4_dynupdate_4832dd20-...` | **inserted** (165 B): the owner descriptor |
| `313d9858-...b4_dynupdate_4832dd20-....0` | **inserted** (704 B): the module body |
| `versions_dynupdate_4832dd20-...` | **inserted** (344 212 B): the staged `versions` |
| `DynamicallyUpdated` | **inserted** (45 B) `{1,1,4832dd20-...}` (with the UTF-8 BOM) |
| `root`, `version` | deleted and inserted again, **same content**, new `Creation`/`Modified` |
| `313d9858-...b4`, `.0`, `versions` | **untouched** (the previous generation stays) |

| `Params` | change |
|---|---|
| `DynamicallyUpdated` | **inserted** (82 B) `{0,2,848a0a59-...,4832dd20-...}` |

`<gen>` is the header GUID of the staged `versions` row (`4832dd20-...` here); `848a0a59-...` is the header GUID of the
ordinary `versions` (the "ordinary generation"). The alias name keeps the storage suffix last (`<uuid>_dynupdate_<gen>.0`).
After a second generation (measured, `online/online-v2b.diff.md`) the markers grow: `Config` `{1,2,<g1>,<g2>}` (82 B),
`Params` `{0,3,<ordinary>,<g1>,<g2>}` (119 B); every generation adds its aliases and never removes the earlier ones.

What native `--dynamic=force` writes *besides* this (16 `Params <guid>_dynupdate_<gen>.si` overlay rows, `dbStruFinal`,
`dynamicCommit`, `_ConfigChngR._MessageNo = NULL`, see `native-apply-trace.md` section 5) the direct mode does **not**
write; for a non-structural module change new sessions got the new code without them (sections 4.1, 4.2), and the
2026-08-29 native control kept the old generation in the open session the same way. Whether data exchange or other consumers
of the change register need those rows is **not measured**.

### 3.3 `exclusive`, `live`, `worker` (`render_ordinary_transition`, `mssql_main_activation.rs:421-446`)

For each staged name: `DELETE FROM Config WHERE FileName=... AND PartNo=...`, then `INSERT ... SELECT FROM ConfigSave`
(the module owner and body, `root`, `version`, `versions`); then `DELETE FROM Config WHERE FileName='DynamicallyUpdated'`
and the same on `Params`, each expecting exactly one row if the marker was read and none if not
(`render_marker_delete`, `mssql_main_activation.rs:525-537`). Measured for `live`
(`live/live-v1.diff.md`): `Config` -1 (`DynamicallyUpdated`), 3 rows updated (module owner, body, `versions`), `root`/`version`
same content with new dates; `Params` -1; `ConfigSave` 5 -> 0. `exclusive` adds before this, inside the transaction, the gate
`IF EXISTS (SELECT 1 FROM sys.dm_exec_sessions WHERE is_user_process=1 AND session_id<>@@SPID AND database_id=DB_ID()) THROW
57209` (`mssql_main_activation.rs:427`).

**Not written by any mode:** rows of `Params`/`Files` other than the marker, the change register, the schema tables, data
tables. The diffs of the runs show no other service table changed by the tool; the only other differences are the user-settings
registers of the observer sessions (`_CommonSettings`, `_UsersWorkHistory`) and the constant the transaction probe wrote.

### 3.4 `live` after the commit (`render_live_recovery`, `mssql_main_activation.rs:580-628`)

Preflight, before the transaction (rendered lines 3-7 of `live/live-v5.sql`): the database exists (`57230`), recovery model
FULL or BULK_LOGGED (`57231`), state `ONLINE` (`57232`), the tail-log file does not exist and is not a directory
(`57233`, through `sys.dm_os_file_exists`, i.e. on the SQL Server host). Since 0.5 (section 6.7) the same gate also checks the log backup chain (`57235`), the tail directory
(`57236`), writes and removes a probe backup there, and refuses sessions with open work (`57238`) unless `--interrupt-sessions` was given; it runs before the stage as well.

After the `COMMIT`: `@LiveExpected1cConnections` = number of SQL sessions of program `1CV83 Server` bound to the database
(line 587); `CHECKPOINT`; then, from `master`: **cycle 1** `ALTER DATABASE SET SINGLE_USER WITH ROLLBACK IMMEDIATE` ->
`BACKUP LOG ... TO DISK=<tail> WITH NORECOVERY, INIT, COMPRESSION, CHECKSUM` -> `RESTORE DATABASE ... WITH RECOVERY` ->
`SET MULTI_USER`; the **readiness gate** (a loop of 100 ms until the number of `1CV83 Server` sessions is at least the
expected one and has stayed so for 500 ms, at most 4 000 ms after `MULTI_USER`; otherwise `THROW 57234`, lines 607-609);
**cycle 2** (`NOINIT`, appended to the same file). The catch block restores `MULTI_USER` when the database is not
`RESTORING` and otherwise throws `57250` with the mandatory `RESTORE DATABASE ... WITH RECOVERY` (lines 623-627).

### 3.5 `worker` (`mssql_worker_switch.rs`, `mssql.rs:1053-1065`)

Ordinary promotion as in 3.3 (no recovery cycle), then `rac process turn-off --cluster=... --process=<old>`, polling
`rac connection list` every 100 ms until the infobase is served by another process (timeout 10 s,
`mssql.rs:1025`, `mssql_apply.rs:111`). Before staging and again before the SQL commit exactly one process must serve the
infobase and no foreign infobase (`infobase != 0000...`) may be on it (`prepare_dedicated_worker`, `mssql_worker_switch.rs:37-55`,
`validate_process_is_dedicated`, `130-150`); the assignment is checked once more after the commit (`57-77`). Killing the
process was tried and removed (2026-08-30). Not run in this issue.

## 4. When do sessions see the change

The instrument: an external data processor (`observer/IbcmdRsObserver.epf`, sources in
`scripts/apply-trace/lab/online-live/observer/`) opened with `/Execute` in a thin client (`1cv8c.exe`) on the registered
clone. Once a second it reads `ОбсужденияСлужебныйКлиентСервер.ТипыВнешнихСистем().Telegram` **on the client** and through an
`&НаСервереБезКонтекста` function **on the server** (the module is a client-server module, so each side runs its own compiled
copy), and probes a second module, `РаботаСКлассификаторамиКлиентСервер.IbcmdRsMarkerB()`, which exists only in the second
online generation. Every line is `ms-since-0001-01-01 UTC | label | event | 1C session number | client value | server
value | details` (details: the value of the client-side "unsaved draft" form field and its modified flag). Modes: `poll`;
`lazy` (session opened but the module untouched until a flag file appears); `txn` (on a flag file, opens a server
transaction, writes a constant, holds it for 60 s). The change is the string stored under the key "Telegram" (original
value `Telegram`). The thin client polls every 1.02 s (median); it has a periodic stall of up to 9.4 s every ~30 s, so
gaps below 10 s in the journals are baseline (`analysis/poll-gaps.txt`).

"WARM" = a session opened before the change and kept open; "NEW" = opened after the commit; times are relative to the
commit of the activation transaction (from the Extended Events trace).

### 4.1 ONLINE, first generation (commit 17:08:35.690 UTC, clone `ibcmd_rs_05_online_a1`, no dynamic history)

| session | opened | what it saw | until |
|---|---|---|---|
| WARM-A (`poll`, session 1) | 17:00:47 | old on client and server, 1 320 polls in all, no error, no gap beyond baseline | its client process ended on its own at 17:28:56 (+20 min 20 s), not caused by the change (no crash event was found) |
| WARM-B (`txn`, session 4) | 17:01:37 | server transaction opened 20 s **before** the commit, held 60 s: `ЗАФИКСИРОВАНО за 60023 мс`; the write is in the database (`_ConstChngR9527` +3 rows) | after it: old code, until stopped at 17:29 |
| LAZY (`lazy`, session 5) | 17:01:44 | never touched the module before the change; armed 1.06 s after the commit, first read at **+1.43 s: old** on client and server | old code until stopped at 17:43:18 (**+34 min 43 s**, 1 639 polls) |
| NEW-1 (`poll`, session 8) | launched 17:08:36.4 (+0.7 s) | first poll at +29.5 s (start-up under load): `ibcmd-online-v1` on client and server | v1 to the end (+35 min) |
| NEW-2 (`poll`, session 9) | launched 17:11:14 | first poll 8 s later: v1 on both sides | v1 to the end |

So: **the generation is bound when the session is created, not when the module is first used** (LAZY). An idle or busy
session is not disturbed: the largest gap of every session in the ±30 s around the commit is inside the baseline stall
(7.4 s at -21.7 s for LAZY; 1.2 s after the commit for NEW-1). The `SQL` timeline (`timelines/online-timeline-sql.tsv.gz`,
100 ms) never left `ONLINE`/`MULTI_USER`. The ordinary module rows are unchanged, so `mssql-dump-config`-style readers that do
not know the overlay see the old text (F-1, F-15).

### 4.2 ONLINE, second generation (another module; commit 17:32:07.480 UTC)

The second generation could not be made for the same module (F-1) and, for another module, needed the two-step path (F-2);
both are fixed in 0.5 (section 6.1).
The change added an exported function `IbcmdRsMarkerB` to `РаботаСКлассификаторамиКлиентСервер`.

| session | opened | first module A / module B values |
|---|---|---|
| WARM-C (`poll`, session 10) | 17:29:28 (between the generations) | A = v1 (generation 1); B = "нет" (function absent); **B stayed absent for the 11 min 13 s watched after the commit** |
| NEW-3 (`poll`, session 16) | launched 17:32:08 (+0.6 s) | A = v1; B = `ibcmd-online-B-v2` on client **and** server |
| LAZY, NEW-1, NEW-2 | earlier | unchanged (A only) |

Markers after it: `Config` `{1,2,4832dd20-...,96eee589-...}`, `Params` `{0,3,848a0a59-...,4832dd20-...,96eee589-...}`.

### 4.3 LIVE (clone `ibcmd_rs_05_online_b1`, FULL recovery, base backup taken; native dynamic history present)

Five attempts, all ended by the gate `57234` ("1C SQL connections did not recover before the live activation deadline;
database is online and the second recovery was not started"). The promotion was committed each time; the tail file holds
cycle 1 only. Table (times UTC; window = `SINGLE_USER` until `MULTI_USER`, from `timelines/*-timeline-sql.tsv.gz`):

| # | run | sessions | window | first 1C SQL connection back | what the sessions saw |
|---|---|---|---|---|---|
| 1 | `live-v1`, capture kit | 3 WARM (poll, txn, lazy), then NEW-1, NEW-2 | 9.72 s (17:56:07.456 - 17:56:17.180); `RESTORING` 1.05 s | none within 5 s | all three WARM clients: modal **"Ошибка СУБД ... пользователя sa ... native=18456"** (poll, lazy) and **"Невосстановимая ошибка ... POST /e1cib/logForm"** (txn client; the transaction was rolled back, no constant written); NEW-1/NEW-2: client `ibcmd-live-v1` (new), server `Telegram` (old), for 9 minutes |
| 2 | `live-v2`, capture kit | 1 WARM (poll, session 1), NEW-1 | 0.87 s (18:08:22.356 - 23.222) | +3.5 s | WARM: **no error, same session, draft kept**; from +7.5 s server = v2, **client still v1**; NEW (opened +12.9 s): client v1, server v2 |
| 3 | `live-v3`, lean | 1 WARM | 2.41 s (18:13:39.054 - 41.460) | none in 25 s | WARM stopped polling 14 s earlier (before the window) and shows the DB error dialog |
| 4 | `h1-live-c1` (clone `a1`) | 1 WARM, NEW | 4.95 s (18:21:14.245 - 19.195) | n/a | unchanged views for 65 s (see F-4) |
| 5 | `live-v5`, lean (only the SQL sampler) | 1 WARM | 2.09 s (18:25:00.272 - 02.356) | +10.2 s | WARM: no error, same session, **no change of view** (client v1, server v3; v5 expected) |

Manual completion of the missing second cycle (the tool cannot do it, see F-5): after attempt 1 (window 2.05 s at 18:03:05)
`BACKUP LOG` failed with `924` ("database is already open and can only have one user at a time", i.e. a 1C connection took
the single-user slot between `ALTER` and `BACKUP`); after attempt 2 (window 3.21 s at 18:10:37.6) it succeeded, and **both**
open sessions then showed the DB error dialog. Pressing the default button ("Перезапустить") of such a dialog makes the client
process start **another client process** (`1cv8c.exe MNG_ENTERPRISE`) and exit: a new session, no external processing, the
draft is lost.

What can be said from these five runs (**measured**): a session survives a cycle when it makes no call that needs the
database during the window (attempts 2 and 5: 0.9 s and 2.1 s); it gets a modal DB error when it does (attempts 1, 3 and the
3.2 s manual cycle); the platform does not retry silently for longer than about a second or two. The 1C SQL connections
came back 3.5 s, 10.2 s and never after the first cycle, against a gate that allows 4 s, so under this load the gate
aborts. **Hypothesis:** with an idle machine and one polling client the gate passes (as on 2026-08-30); the constants
4 000 ms/500 ms and the expected count (which includes the two sessions that the tool's own RAS verification opens, F-3) are
not robust against a busy machine or several sessions.

After an abort, the state is a *mixed generation*: attempt 1 left new sessions with new client code and old server code until
every session had ended (a fresh session at 18:06:27 saw both new); attempt 2 the other way round (client old, server
new). Which side lags was not the same in the two attempts (**not explained**).

### 4.4 WORKER (2026-08-30 evidence, not repeated)

`evidence`: `add-mssql-live-generation-switch/evidence/worker-watch-20260830.md`. Same client PID and 1C session UUID; the open
client saw the new client and server code 3.3 s after an already staged promotion (promotion + signal 323 ms); the complete
save path (export 400-734 ms, classification 16-32 ms, staging 1.2-1.7 s, activation 5.8-7.3 s of which the worker hand-off
4.3-6.2 s) took 7.5-9.0 s; a shared process was refused in 108 ms with `ConfigSave` untouched. Under the load of this issue
each `rac` call took 1.7 s (`infobase info`) to 5.4 s (`session list`), against a 100 ms poll and a 10 s handoff timeout
(`mssql_worker_switch.rs:91-116`): on a busy machine the timeout is tight (**hypothesis**, not run).

## 5. Timing

ONLINE, clone without history, report of the tool (`online/online-v1.json`): active export 2 021 ms, classification 66 ms,
staging 1 049 ms, activation 2 737 ms (includes the second RAS verification and the three reads of the snapshot), total 10 419 ms;
wall of the process 11 838 ms; Extended Events: staging transaction 368.8 ms, activation transaction 190.8 ms
(`online/online-v1.transactions.tsv`). Activation alone in a fresh process (`online/online-v2b.json`): 2 142 ms wall. Dry-run
of a no-op on a database that has dynamic history but not for the object: 10.6 s wall, export 2.0 s. The same export for an
object that *has* an alias: 250-300 s (F-1/F-15). LIVE: the tool ran 13.8-27.8 s before the gate aborted; the window of
cycle 1 was 0.87-9.72 s (section 4.3). `rac`: `infobase info --infobase-user` 1.7 s, `session list` 5.4 s (load).

## 6. Safety: what fails closed and what does not

Fails closed (all measured or read from the SQL): stale stage or drifted `Config`/marker (`57201`-`57204`), busy lock
(`57200`), alias that already exists (`57211`), any row count mismatch, unsupported target/extension mode, missing
`--allow-non-lab`/`--sqlcmd-trust-cert`, platform-profile mismatch (RAS agent build, registered infobase, live column
layout), unreadable markers ("Config and Params dynamic markers must both be present or both absent"), a stage that reuses
the active generation. Recovery artifacts are written **before** the SQL runs (`mssql.rs:1031-1051`).

Review findings (2026-09-29, "report, do not fix"; severity = effect on a user of the mode; **file:line** are of this branch):

| id | severity | finding | where | evidence |
|---|---|---|---|---|
| F-4 | **critical** | An ordinary promotion (`exclusive`, `live`, `worker`) replaces only the staged rows and deletes both markers. The `_dynupdate_` rows of the earlier online generations stay as orphans, so **what earlier online generations published is silently gone**. Native `exclusive` merges the overlay. **Mitigation (#408 step 1): the three modes refuse before any write when the database holds markers or `_dynupdate_` rows. Fix (#408 step 2): the `exclusive` mode is carried out by `mssql_config_apply`, which folds the aliases (section 6.6); `live`, `worker` and `--sqlcmd` `exclusive` keep the refusal.** Measured (before the mitigation): after a `live` promotion of an unrelated module on a base with two online generations and once every old session had ended, a new session saw the original `Telegram` (generation 1 gone) and no `IbcmdRsMarkerB` (generation 2 gone); on the corpus base the storage shows 5 alias rows, no marker, ordinary bodies of 1 522/2 244 B against 1 533/2 262 B in the aliases. | `mssql_main_activation.rs:421-446`, `525-537` | `analysis/h1-sessions.txt`, `live/h1-storage-b1.txt` |
| F-1 | high | **Fixed in 0.5 (#409), section 6.1.** The second apply of an object that already has an online alias fails ("selected storage closure did not emit required source body") after 250-300 s. By the code (the symptom is measured, the mechanism was not tested separately): the headers are read for the plain names, then the overlay hides the plain rows and the aliases were never selected, so nothing remains. The dev loop "edit, apply online, edit, apply online" cannot be repeated. | `mssql_dump/mod.rs:3762-3765`, `44820-44834`; `mssql_apply.rs:1020-1030` | `online/online-v2-single.err`, `online-v2-dry.err` |
| F-2 | high | **Fixed in 0.5 (#409), section 6.1.** The dynamic overlay is process-global (`STORAGE_GENERATION_OVERLAYS`), installed by the active export and cleared only by the next `dump_config`. `activate_staged_main`, run in the same process, reads the ordinary `Config` rows **through the overlay** (`qualified_storage_table`), takes the alias generation as the ordinary one, and is refused with "Params dynamic ordinary generation disagrees with versions" after staging, leaving a dirty `ConfigSave` (the same staged rows on the same database pass in a fresh process: `mssql-activate-staged-main --dry-run` on the corpus clone, and the real `online` run of generation 2). Any real apply on a base that has markers (native leftover of the corpus, or an earlier online generation) hits it. Workaround: `mssql-activate-staged-main` in a fresh process (measured). | `mssql_dump/dynamic_generation.rs:139-157`, `mssql_dump/mod.rs:2129`, `44845-44851`, `1008-1027`; `mssql.rs:957-976`; refusal `mssql_main_activation.rs:826-829` | `online/online-v2b-single.err`, `online/baseline-exclusive.err`, `live/live-v1-single.err` |
| F-3 | high | **Fixed in 0.5 (#409), section 6.2.** `exclusive` is refused on every registered infobase that has users: the RAS verification with `--infobase-user` makes the cluster open two `1CV83 Server` SQL sessions (the RAS connection that holds them stayed for the whole observation, 56+ minutes; `rac connection disconnect` with the infobase user just creates another RAS connection, without it is refused) and the gate counts them. | `mssql_platform_profile.rs:211-227`; `mssql_main_activation.rs:427` | `online/baseline-act2.err`, `.sql` |
| F-5 | high | `live` aborts half-way when the 1C connections do not come back within 4 s (0 of 5 completed here). The promotion and markers are already committed, sessions may be left in a mixed generation or in a modal DB error, and **no command can run the second cycle**: a retry is a no-op (`executed=false`, 2.0 s). The manual second cycle is racy (`924`). The error text does not tell the operator any of this. | `mssql_main_activation.rs:587`, `607-609`; no-op `mssql_apply.rs:246-274`; `mssql.rs:1053-1065` | section 4.3, `live/live-v*.err`, `live/live-v2-retry.json` |
| F-6 | low | **Fixed in 0.5 (#409):** `apply_source_change` checks its arguments (`--allow-non-lab`, `--sqlcmd-trust-cert`, the extension mode) before the verification and hands the verification to the activation (`activate_staged_main_verified`, `activate_staged_extension_verified`), so it runs once per command; `mssql-activate-staged-main` and the extension activation also check `--allow-non-lab` first. `--dry-run` still verifies (it reads the target). Was: the platform-profile verification (two `rac` calls, RAS authentication, SQL schema probe) runs **twice** in one high-level apply and also for `--dry-run` and before the `--allow-non-lab` check; each `rac infobase info --infobase-user` opens a cluster connection and SQL sessions on the infobase (1.7 s each under load). | `mssql_apply.rs:70-95`; `mssql.rs:910-931` | `online/online-v1.trace-summary.md` (statements of `1CV83 Server` sessions before the staging) |
| F-9 | medium-high | The `live` preflight checks the recovery model, the state and the tail-file name, not the log chain or the destination directory. With `FULL` but no full backup the transaction commits and `BACKUP LOG` then fails with `4214`; the database is online with the new generation in the ordinary rows and no session switch. The design says a failure before a successful tail backup "leaves the database online and returns the bounded row recovery artifact"; the artifact path is not in the error. **Fixed in 0.5 (#409), section 6.7:** the log chain, the tail directory and the account's right to write there are checked before the stage, and again at the head of the script. | `mssql_main_activation.rs:342-348`, `402-406`; `mssql_live_gate.rs` | `live/live-v6-nochain.err`, `live/break-log-chain.sql`; `evidence/live-gate/f9f10-red.log`, `f9f10-green.log` |
| F-10 | medium | `live` kills every connection of the database with `ROLLBACK IMMEDIATE`: no session preflight, no warning. In-flight server transactions are rolled back and their clients get an unrecoverable error; a client that touches the database in the window shows a modal error and loses unsaved data on restart. It is the documented design, but the command has no `--force`-style acknowledgement beyond `--allow-non-lab`. **Fixed in 0.5 (#409), section 6.7:** sessions with an open transaction or a running request are refused (named in the report) unless the operator gives `--interrupt-sessions`; checked again inside the promotion transaction before its `COMMIT`. | `mssql_main_activation.rs:595-599`, `610-614`; `mssql_live_gate.rs` | section 4.3; `evidence/live-gate/f9f10-red.log`, `f9f10-green.log` |
| F-15 | medium | **Fixed in 0.5 (#409), section 6.5.** With an active generation every read of `Config` is a derived table over the **whole** table (`CASE` over the file name), so a bounded read costs a full scan and a memory grant: 4 762 logical reads and 34.5 s elapsed under load against 3 reads and 1 ms for the plain read (`RESOURCE_SEMAPHORE` wait); the bounded export of one object took 250-300 s instead of 2 s. | `mssql_dump/dynamic_generation.rs:167-190` | `online/overlay-query.sql`, `online/online-v2-dry.meta.txt` |
| F-8 | low-medium | Historical row/header and publication defects are repaired: full physical preimages/CAS, actual overwritten versus retained ONLINE rows, synchronized no-clobber standalone compact package and opt-in compact LIVE envelope are included. Legacy LIVE tokens remain compatible. Actual compact LIVE continuation control and generic guarded undo are still open; no power-loss durability claim. | `mssql_dump/mod.rs`; `mssql_recovery_artifact.rs`; `mssql_live_artifact.rs` | [Standalone recovery](evidence/live-gate/compact-recovery-2026-10-01.md), [compact LIVE](evidence/live-gate/compact-live-recovery-2026-10-01.md) |
| F-7 | low | Historical timestamp difference: the merged plain dynamic marker writer now uses the platform year offset of 2000; verified platform stamps remain authoritative for staged rows. This source correction requires current merged-code verification and does not turn historical session observations into a new native parity measurement. | `mssql_main_activation.rs`; staging | `online/online-v1.diff.md`; current integration gates pending |
| F-11 | low | Generic WORKER execution now refuses incomplete loaded/history ownership before staging or artifact publication. No RAC preparation/signal is performed. Source no-op and dry-run behavior remain available; worker watch refuses at startup. Positive general worker ownership remains open; earlier private-cluster measurements do not authorize this product path. | `mssql_worker_switch.rs`; `mssql_source_change.rs`; `mssql.rs` | [Current refusal boundary](evidence/live-gate/worker-f11-refusal-2026-10-01.md); section 4.4 is historical |
| F-12 | low | Partial: direct staged-main activation and high-level source apply carry the requested certificate policy through capture, active export, staging, dry-run preparation and execution; see [policy propagation](evidence/live-gate/certificate-policy-2026-10-01.md). Source apply no longer requires trust. Standalone legacy export/stage defaults and RAC password command-line handling remain open. | `mssql.rs`; `mssql_apply.rs`; `mssql_dump/mod.rs`; `mssql_platform_profile.rs` | backend/argument and actual preparation read-handle regressions; no live certificate-chain matrix |
| F-13 | low | Historical identical-branch defect is absent from the current code. Exclusive promotion carries the own apply's actual `tables_touched`, including registration and Files tables. Online reporting remains at table granularity (`ConfigSave`, `Config`, `Params`); selected source names are not a complete per-row alias/marker mutation ledger. | `mssql_apply.rs`, `tables_touched_by_config_apply` and report construction | Existing exclusive-report regression passed in the current 3729-test run; full per-row reporting is unclaimed |
| F-14 | low | Historical: two path comparisons disagreed (ASCII case fold against Unicode lower-case), and watch hashed the whole closure every 100 ms. Root containment now uses the existing Unicode inventory key. Watch polls metadata every 100 ms and reads content on hints or a mandatory one-second interval; preserved size/time edits are still detected. See [path consistency](evidence/live-gate/online-recovery-preimages-2026-10-01.md) and [watch cadence evidence](evidence/live-gate/watch-read-cadence-2026-10-01.md). | `mssql_source_change.rs`; `mssql_apply.rs` | focused regressions; no native watch throughput claim |
| F-16 | info | Historical duplicate body overlay removed: `active_dynamic_generation` only names the active generation, while the bounded export supplies the effective body. It does not rewrite the exported selected body. | `mssql_apply.rs`, `active_dynamic_generation` | Current source review; see the F-1/F-16 explanation below |
| F-17 | info | OpenSpec `add-mssql-live-generation-switch` task 7 (readiness gate) is checked but has no evidence with the gate; the recorded live run is the fixed five-second delay it replaced. | `add-mssql-live-generation-switch/tasks.md` | section 4.3 |

### 6.1 Fixed in 0.5 (#409): F-2 and F-1

Both were reproduced first, by tests on the unfixed code (an export leaves its overlay behind; another database reads the
overlay; a selected aliased object lists no row) and on a corpus clone with the unfixed binary, then fixed and proven on the
same clone with the fixed one (`ibcmd_rs_05_ui_rf_a`, restored from the БСП 8.3.27 corpus backup, which already holds a native
online generation of two objects: the common form `_ДемоПримечание` and the common module `_ДемоЗаметки`).

**F-2, what was wrong and what is now.** `STORAGE_GENERATION_OVERLAYS` was keyed by table name only and cleared only by the
next `dump_config`. The activation that follows the active export in the same process read its snapshot (`fetch_main_activation_rows`)
through it, took the alias generation for the ordinary one and was refused after the stage. Now:

- the overlay is kept per (database, table) and lives in a `StorageViewScope`; `dump_config` and `export_staged_state` open
  one, so the overlay ends with the export, and another database or a step that opens its own scope never reads it;
- `fetch_main_activation_rows` (the activation's reads: staged rows, the rows they replace, both markers) always reads the rows
  as stored, whatever scope is open: the transaction compares them with `dbo.Config`, not with a view;
- the checks that need no staged row are made **before** anything is staged, dry runs included: an ordinary mode (`exclusive`,
  `live`, `worker`) on a database that holds markers (the #408 refusal), markers that are not one pair or do not agree with the
  ordinary `versions` row, and the `--tail-log-output` argument (required for a real `live` run, absolute, no control
  characters; refused for the other modes). The plan of the activation makes the same checks through the same function
  (`check_publication_state`), so the two cannot disagree; a test runs both on the same input. The state preflight runs after the
  no-op decision (a no-op is not refused) and before the compile tree and the stage; the tail-log check runs before the first `rac` call;
- an `online` apply on a base with markers is now one step in one process: no `mssql-activate-staged-main` in a fresh process.

`overlay_active_dynamic_module` (F-16) no longer rewrites the exported file from the alias row of the selected body: the export's
view already publishes that row, and the function would have come back to life now that the view does not outlive the export.
It is `active_dynamic_generation` and only names the generation the change applies to.

**F-1, what was wrong and what is now.** A run that selects names lists the headers of the *stored* rows with those names. For an
object with an alias that is the plain row, which the overlay hides, and none of the alias rows, so nothing was left to export
("selected storage closure did not emit required source body"). The published headers of a selected run now come from the
inventory of the table (already read to resolve the overlay), filtered by the published names.

Measured on the clone (this machine, no other load on the database; times of the tool's own report, wall of the process):

| step | unfixed binary | fixed binary |
|---|---|---|
| `online`, common module `ОбсужденияСлужебныйКлиентСервер` (no alias), base with the native marker | real run: refused after the stage, `Params dynamic ordinary generation disagrees with versions`, wall 2.4 s, `ConfigSave` holds 5 rows | real run: applied, generation 2 written, wall 2.8 s (export 1.1 s, stage 0.2 s, activation 0.7 s), `ConfigSave` empty |
| `exclusive`, same change, real run | refused after the stage (the #408 refusal, raised by the plan), wall 4.4 s, `ConfigSave` holds 5 rows | refused **before** the stage, wall 1.5 s; row counts and checksums of `Config`, `ConfigSave` and `Params` identical to the state before |
| `worker`, `live`, `exclusive`, dry run | not run | refused, wall 1.1-1.7 s, nothing written |
| `online`, dry run, module with an alias | fails, `did not emit required source body`, wall 40.6 s | passes, wall 4.2 s (export 3.0 s) |
| `online`, real run, module with an alias | (fails as above) | generation 3: wall 2.4 s (export 0.7 s, activation 0.9 s); the alias body holds the new text |
| the same module again with another edit | (fails) | generation 4: wall 1.9 s; history `{1,4,...}` / `{0,5,...}` chains the four generations |
| the same source once more | (fails) | `no_op`, wall 1.1 s: the active export of the module is the text just applied |

The header rows of the new generations are byte-identical (inflated) to the native alias header of the same object. The 250-300 s
of F-1 were measured under the load of the other runs; the unfixed binary needed 40.6 s here, on an idle database. F-15 is
untouched: a bounded read of an object with an alias is still a scan of the table under a derived table (the export took 3.0 s
cold and 0.7 s warm here), and would take longer under load.

**Found while proving F-1; fixed in section 6.4 (#416).** The stage reads its base rows and the `versions` blob with its own SQL on the
ordinary `Config` rows (`fetch_config_blob`, `fetch_config_blobs_for_files`), never through the overlay. On a base with earlier
generations the new `versions_dynupdate_<g>` is therefore built on the ordinary `versions`, not on the active generation's:
in the clone, `versions_dynupdate_<native g1>` lists the form `a627e390-...` at `226957a7-...` and its body at `9fab40a2-...`,
while the three generations written by this tool list `9947107e-...` and `f2422a32-...` (the ordinary values). Bodies are read from
the aliases whatever `versions` says, and a session has not been opened on such a state, so the effect on a session (a reload of
the objects whose stamp went back) is **unverified**.

### 6.2 Fixed in 0.5 (#409): F-3

**What was wrong, measured.** `rac infobase info` without the infobase user is refused ("Недостаточно прав пользователя на
информационную базу"); with it the cluster's worker process loads the infobase and keeps **two idle `1CV83 Server` SQL sessions**
(`sleeping`, `open_transaction_count = 0`, `host_process_id` = the pid `rac process list` gives for the worker) for as long as its
RAS connection lives. `rac connection list` shows that connection: application `RAS`, session number 0. The gate `57209`
(`57212` for an extension) counts every session, so `exclusive` was refused on every infobase that has users. On the marker-free
clone `ibcmd_rs_05_ui_rf_b` the unfixed binary stops with `57209` after the stage, with exactly those two sessions on the database.

**What is now.**

- `read_infobase_clients` asks the cluster (`rac connection list`, `rac session list`, `rac process list`, no infobase login,
  which would open another RAS connection). Every connection whose application is not `RAS` (or has none) and every session is a
  client; a worker process that carries RAS connections of the infobase and no client is a RAS-only process (host, pid).
- An `exclusive` apply is refused **before the stage** when the cluster lists a client, and the activation asks again before it
  renders its script (a dry run included).
- `exclusive_session_gate` builds the one statement both gates use. It leaves out the sessions that are `1CV83 Server`, `sleeping`,
  without an open transaction, on a RAS-only process (host and pid match). A session of another program, another process, a running
  one or one that holds a transaction still refuses. With no process named the text is the old one, and the dry-run report of the
  activation lists the processes it left out (`own_ras_processes`).
- The extension activation uses the same helper for its gate (`57212`). Unit tests cover it; it was not run on an extension in the lab.

**What it does not cover.** A user who signs in through the same worker process in the seconds between the cluster query and the
transaction, while the sessions stay idle, is not seen: the exemption is by process, not by session. The window is the time from the
last `rac` call to the transaction (a few seconds in the runs below); the cluster's own list and the running/transaction condition
narrow it, an infobase lock (`sessions-deny`) would close it and is not done here.

Measured on the marker-free clone (the corpus backup with its markers and aliases deleted, so that the #408 refusal does not come
first; the worker process 22608 held the RAS connection):

| run | unfixed binary | fixed binary |
|---|---|---|
| `exclusive`, no users | refused `57209` after the stage, 6.9 s; `Config` unchanged, `ConfigSave` 5 rows; two idle `1CV83 Server` sessions of pid 22608 | applied, 13.4 s (activation 4.1 s); the report names `DESKTOP-SMI5N4O` / 22608 as left out; `ConfigSave` empty |
| the same, with an idle non-1C SQL session on the database | - | refused `57209` after the stage, 3.1 s; `Config` unchanged (the gate still counts it) |
| the same, with a thin client (`1cv8c`) connected | - | refused before the stage on the cluster's word, 2.0 s, five connections and sessions listed; `Config` and `ConfigSave` unchanged; the dry run is refused as well; `online` (dry run) passes |
| the client killed, its session left in the cluster | - | refused the same way until the session was ended with `rac session terminate` (the cluster keeps the session of a killed client) |
| after that | - | applied, 5.7 s, three idle sessions of the worker left out |

A connected client also writes `Params` on its own (a row went away while the tool was refusing before any write), so a fingerprint
of the whole database is only comparable without one.

### 6.3 The export path after the scope guard (#409)

`StorageViewScope` wraps `dump_config` and `export_staged_state`, the export every user runs and the guard of the import. Checked
against the native platform, with the fixed binary (merged with feat/0.4 5e146f00):

- `ibcmd_rs_04_rcheck_bsp_a` (active generations): the drop-in `infobase config export` (37 s) against the native `ibcmd infobase
  config export` (40 s) of the same database: **12 198 of 12 198 files identical**.
- `ibcmd_rs_05_ui_rf_a` after the three generations this tool wrote (F-1, F-2): ours 55 s, native 20 s, **12 198 of 12 198 identical**;
  the native platform reads the text of the last generation (both markers are in `_ДемоЗаметки`).
- БСП 8.3.27 from rows (`mssql-dump-config --rows-dir`), the binary before (feat/0.4 e672908c) against the binary after:
  **12 199 of 12 199 identical**.
- Tests: the whole `cargo test --locked -p ibcmd-rs --no-default-features` (lib and the integration tests, 46 binaries): 3 643 passed, 0 failed, 12 ignored; the lib alone 3 435, the import guard's `mssql::stage_guard` tests among them.

### 6.4 Fixed in 0.5 (#409 follow-up, #416): the pre-stage session count and the stage's base rows

**An exclusive apply is refused on a foreign SQL session before the stage.** The gate inside the activation's transaction refuses a
session of another program or process, but after the stage, and left `ConfigSave` filled. The apply now runs the gate's own condition
as a query before the stage (`foreign_sessions_query`, with the same exemption for the tool's RAS-held sessions) and names what it
found; the gate in the transaction stays the last word, and a `--sqlcmd` connection, which cannot ask, is left to it. Measured on a
marker-free clone with one idle non-1C session on the database: unfixed, refused after the stage (7.1 s, `ConfigSave` 5 rows); now
refused in 3.4 s with the session named (login, host, program) and the row counts and checksums of `Config`, `ConfigSave` and `Params`
identical to before. Without that session the same apply goes through (3.4 s).

**F-18 (#416): the stage built `versions` on the plain row.** Found while proving F-1 (section 6.1). The stage reads the base rows it
patches with its own SQL on the ordinary `Config` rows, so the `versions_dynupdate_<g>` it writes carried the ordinary stamp of every
object an earlier generation had changed. Nothing else was wrong with the rows: the bodies stay in their aliases.

*What a client saw, measured before the fix* (the observer client of the #344 kit, extended to read the title of the common form
`_ДемоПримечание`, which the native generation changed, next to the client and server value of the module marker):

- Marker-free clone. A client warmed on the ordinary configuration and killed (so its disk cache holds the ordinary stamps);
  generation 1 of ours (module A marker `v1`); generation 2 of ours (module B), whose `versions` had A back at its ordinary stamp
  (`151cc9e7`, generation 1 had `3ac2adf7`). A new session: A `ibcmd-online-v1` on the client **and** the server, B present. Correct.
- Native generation, then ours (the native rows taken out while a session was opened on the ordinary configuration and put back,
  then generation 2 of ours over them). The old session stays alive with the ordinary form; a new session shows the form title of the
  native generation and A `v1`, while the old one keeps the ordinary title, as an old session should. Correct.

So no visible effect on the content a session gets, in these two runs. The row is wrong all the same: in the second run
`versions_dynupdate_<ours>` differs from `versions_dynupdate_<native>` in 9 stamps, four of them (the form and its body, the module
`_ДемоЗаметки` and its body) only because they went back. Whatever compares version stamps -- the `ConfigDumpInfo.xml` an export
writes lists them -- takes those objects for unchanged since before the native generation. Not tested with a native tool.

*The fix.* `fetch_config_blob` and `fetch_config_blobs_for_files` (the stage's base-row readers) ask for the generation history once
per process and database; when generations are active they read the row the platform reads: the plain row and every alias of it in
one seek (`FileName = @name OR FileName LIKE '<stem>\_dynupdate\_<36 wildcards><suffix>' ESCAPE '\'`), and the alias of the newest
generation in the history wins (`mssql_effective_row`, the rule the export's overlay applies). It covers every base row of the
per-row stage -- headers, bodies, `versions`, `root`, the constants.

*One rule with the import's stage (#388 step 2).* The import's stage reads the whole table first (the bulk prefetch) and bases its
rows on the aliases of a pending online update (`dynamic_generation_aliases`, `docs/import/override.md` section 1, item 6); the
apply's stage asks row by row. Both apply the export's rule (`StorageGenerationOverlay`: the alias of the newest generation of the
history that carries the name, an unlisted generation ignored, the plain row when none does). The row-by-row reader reaches it
through `mssql_dump::stored_row_name` and adds only the seek that finds the candidates (`mssql_effective_row`); it does not keep a
rule of its own, and a test gives both readers the same stored names and asserts that they name the same row for every name (an
object the update added, an unlisted generation and a missing name included). Which one runs is decided by the prefetch:
`fetch_config_blob` answers from the prefetched rows first (the import), and asks the database only when there are none (the
apply); the two never read the same row in one process.

Measured on a clone restored from the corpus (native generation of two objects), then generation 2 of ours (module A) and generation
3 of ours (module B), with the fixed binary:

| comparison of `versions` rows | stamps that differ |
|---|---|
| native generation -> ours (unfixed binary) | 9: A, A.0, the form, its body, `_ДемоЗаметки`, its body, `root`, `version`, `versions` |
| native generation -> ours (fixed binary) | 5: A, A.0, `root`, `version`, `versions` |
| ours 2 -> ours 3 (fixed) | 5: B, B.0, `root`, `version`, `versions` |
| native generation -> ours 3 (fixed) | 7: A, A.0, B, B.0 and the three service rows |

The export of that clone against the native platform: **12 198 of 12 198 identical**. A new session shows A `v1`, B `v2` and the form
title of the native generation. The reproducing test is a live test (`the_stage_reads_the_versions_row_the_platform_reads`,
`IBCMD_RS_DYNGEN_DB=<lab database>`, feature `mssql-live-tests`): on the commit before the fix it fails ("the stage reads the plain
`versions` row"), on the fix it passes; the rest is pure (`mssql_effective_row`: alias names, the `LIKE` pattern, which row wins).

*After the reconciliation with the import (feat/0.4 029b4a2b merged).* The import track's kit writes into its own lab folder, takes the
native lock as `import` and accepts only its own databases, so the same steps were run with a script of this lab: the drop-in
`infobase config import`, the drop-in `infobase config apply`, the platform's `config export`, `source-diff` of the tree against the
export. Clones of the corpus backup, which carries the native generation:

| case | result |
|---|---|
| F-18 proof again, on the merged binary: native generation, ours 2 (module A), ours 3 (module B) | stamps that differ: 5, 5 and 7, as before; export against the native platform **12 198 of 12 198** |
| import on the marker base: a comment appended to `CommonModule._ДемоЗаметки` (the object of the native generation), our import 39.8 s, our apply 130 s (folds the generation: no dynamic row left, Config 9 841, ConfigSave empty), native export | **the export equals the tree** (12 197 files identical; `ConfigDumpInfo.xml` differs, as it does for the native import) |
| the same after our own online generation (module A, `v1`) was put on top of the native one, then the import of the same tree | the same: 12 197 identical; the import overwrote A with the tree's text and its stage was based on our alias rows |
| the import of `rem2` (a form and a template removed): our import 53.5 s | our apply refuses ("требуется штатный config apply": the removals need the platform's), as the import's own acceptance applies that case natively; a descriptor-changing tree (`syn`, `prop`, `confver`, rights, command interface, predefined) is refused the same way ("possibly structural") |

Tests, on the merged tree: the whole `cargo test --locked -p ibcmd-rs --no-default-features` (lib and the integration tests, 46 binaries)
**3 699 passed, 0 failed, 12 ignored**.

### 6.5 Fixed in 0.5 (#409): F-15, a bounded read through the overlay seeks

**The query.** With an active generation every read of `Config` goes through the derived table of `storage_table_expression`: the scan
computes the published name of every stored row, the aggregate keeps the newest generation per name, and only then does the
caller's filter (`FileName IN (...)`, the owner `LIKE`) run. A filter on the aggregated name cannot go below the aggregate, so a read
of one module scanned the whole table and asked for a memory grant of about 6 MB. (The evidence query of the review, a `CASE` per
alias, was an older shape of the same thing.)

**The change.** The builders that keep a known set of names say so (`Selection::Names` for `IN`, `Selection::Owners` for the owner
`LIKE`; `qualified_storage_table_for`), and the scan under the aggregate is limited to the stored rows that can publish them: the
names themselves and, per stem (what precedes the first dot), the prefix range `<stem>\_dynupdate\_%` of the clustered key. That is
a narrowing only, the outer filter is unchanged, so the rows are the same. Not narrowed: an unbounded read (the whole table is the
point), a range filter (the batches of a full export), a list of more than 64 names (it is read as before), a table whose overlay has
no alias (no aggregate to narrow). The only places that build such a query are `fetch.rs`'s five builders.

**Measured** on a clone restored from the corpus backup (native generation of two objects, 9 847 rows), five dry-run applies of the
aliased module `_ДемоЗаметки` per binary, cost of the statements that read the overlay (`sys.dm_exec_query_stats`, per execution):

| statement | before | after |
|---|---|---|
| the rows of the selected module (`IN`) | 5 118 logical reads, 921 ms (first execution cold), grant 6 064 KB (2 792 used) | **12** reads, 1.1 ms, grant 1 024 KB (24 used) |
| the two other bounded reads of a run (two executions per run) | 5 122 reads, 79 ms, grant 5 776 KB | **20** reads, 1.7 ms, grant 1 024 KB (24 used) |
| the read of every metadata row (the name index of the model export; unbounded by nature) | 28 729 reads, 291 ms, grant 11 000 KB | unchanged (28 472 reads, 318 ms; the same rows read from the plain table took 174-242 ms) |

The bounded reads of one apply cost about 15 400 logical reads before and 44 after. On an idle machine the whole `active_export_ms`
hardly moves (median 1 055 ms before, 743 ms after; the first process of the "before" series took 6.5 s on a cold database; the
"after" series ran second, on a warm one, and its first took 1.0 s), because the name index and the model export take most of it. What the change removes is the part that
grows with the table and needs a memory grant, which is what waited (`RESOURCE_SEMAPHORE`, 34.5 s) in the review's run on a loaded
machine. **That wait was not reproduced here**: the server was not loaded and its resource governor is not this issue's to change;
the reads and the grant are the measurable part.

The rows are unchanged, proven three ways: the unit tests give the text of every selection (names, owners, a long list, an empty
one, a wildcard in a name); the live test of the overlay now also reads the same rows through a selection and without one on the
clone (`IBCMD_RS_DYNGEN_DB=<lab db>`, feature `mssql-live-tests`: 11 rows through 12 names and 7 owners, identical); and two real
online applies of the module with the new binary (2.5 s and 1.7 s) leave a base whose export equals the native platform's,
**12 198 of 12 198 files**.

Tests, on this tree: the whole `cargo test --locked -p ibcmd-rs --no-default-features` (lib and the integration tests, 46 binaries): 3 703 passed, 0 failed, 12 ignored; the lib alone 3 495.

### 6.6 #408 step 2: the exclusive mode of the old commands is carried out by `mssql_config_apply`

**The problem that was left.** Step 1 of #408 made `exclusive`, `live` and `worker` refuse a database that holds markers or
`_dynupdate_` rows (F-4), and that includes the БСП corpus, whose native tail is such a state: on it the three modes did not run
at all. Step 2 is the fold: the promotion keeps what the earlier online generations published.

**The decision.** The `exclusive` mode of `mssql-activate-staged-main` and `mssql-apply-source-change` is handed to
`mssql_config_apply`, which folds the rows of every online generation into the ordinary rows as the native apply does (twins of
the dynamic-update cases E1-E4 and of the БСП stages: `docs/apply/own-apply.md`, "Removals" and "Dynamic-update rows go even when the
stage omits them"). `online` keeps the script of `mssql_main_activation.rs` (it *adds* a generation; nothing to fold). `live` and
`worker` keep the script and keep refusing on a marker database until the worker lab cluster exists: their transaction is followed by
a recovery cycle or a worker hand-off that the apply does not do.

**The code path (as built).** Both commands end in one function, so there is one place that decides:

| Entry point | Route |
|---|---|
| `mssql-apply-source-change` | `mssql_apply::apply_source_change`: export the active object, classify, `preflight_main_publication`, the pre-stage session count, stage, then `mssql::activate_staged_main`. The preflight asks `preflight_publication` with the executor of the mode, so `exclusive` no longer refuses markers when the apply will carry it out |
| `mssql-activate-staged-main` | `mssql::activate_staged_main` |
| `activate_staged_main` | profile verification by `rac`, `--allow-non-lab`, read the stage, the rows it replaces and both markers; `MainActivationExecutor::for_mode(mode, built-in client)`; `prepare_main_activation_for` validates the stage and builds the plan and the report (markers are accepted only when the executor folds them); the cluster's word about clients (`own_ras_processes_for_exclusive`); **then**, for a plan that changes something and names the config apply, `activate_by_config_apply` calls `mssql_config_apply::apply_staged_configuration` in place of render + run. `render_main_activation_sql` refuses such a plan (`an exclusive promotion is carried out by mssql_config_apply`), so the two executors cannot both run |

| Route | Executor | Markers or `_dynupdate_` rows on the base |
|---|---|---|
| `exclusive`, built-in SQL client | `mssql_config_apply` (default gate `apply-check`) | folded, as the native apply does |
| `exclusive` with `--sqlcmd` | the script of `mssql_main_activation.rs` | refused before any write (step 1) |
| `live`, `worker` | the script | refused before any write (step 1) |
| `online` | the script | extended by one generation |

What the preamble keeps, because it is the contract of these commands and not of the apply: the platform-profile verification, the
acknowledgement, the cluster's word about clients, the stage shape (service rows and bodies of existing objects, nothing new, one new
generation), `--script-output` and `--recovery-output`.

**What `mssql_config_apply` got.** `ConfigApplyOptions.own_ras_processes` (default empty): the worker processes whose idle `1CV83 Server` sessions
the tool's own RAS verification opened (#409 F-3). Both places that count sessions leave them out with the predicate the old gate uses
(`mssql_platform_profile::session_exemption`, now `pub(crate)`): `other_sessions` (the plan; its query is `other_sessions_query`) and the assertion
inside the transaction (`ScriptInputs.session_exemption`). Without them the apply counts every session, as it did. Nothing else: the fold, the versions of
the folded generation, the `Params` marker, the register and `MobileVersions.dat` were already the apply's.

**The options of the call** (`mssql::config_apply_options`). `database`, the profile of the command, `dry_run` from `--dry-run`, `exclusivity` = SQL sessions, the
default gate (the restructure check reads `Config` with the overlay folded in; the conservative gate compares the staged descriptor of an
aliased object with the raw row and would refuse it), no restructuring, `script_output` from `--script-output` (the apply writes the script there, also in a dry run),
`own_ras_processes` from the plan. `--recovery-output` keeps its meaning (the JSON snapshot of the rows the stage replaces, written before the run, as before); the apply's own
artifact (`rows.pack`, with the markers and the alias rows, the only one that can take a fold back) is named in the report (`config_apply.recovery_dir`).
`--tail-log-output` stays refused for `exclusive`.

**The report.** `MssqlActivateStagedMainReport.activation` keeps its shape and gains `executor` (`script` or `config_apply`): `old_generation` is the generation the promotion starts
from (the last online one), `new_generation` the staged one, both equal to the apply's `active_generation` and `new_generation`. `apply_source_change` still reads
`/activation/new_generation`, `/activation/old_generation` and `/activation/recovery_token`, and reports the apply's `tables_touched` as `tables_changed`. A new field `config_apply` carries the apply's whole
report (gate, generations folded, registrations, recovery directory); it is absent when the script ran.

**Decisions and their reasons.**

- *No-op.* A promotion of a stage equal to the ordinary rows is a no-op only on a database without markers. With markers the ordinary rows are not the configuration (the aliases are): the stage takes
  an online change back to the original text, so the apply folds (`exclusive`), and the script refuses (`live`, `worker`, `--sqlcmd`). The online mode compares with the generation it extends and is never refused for a stage that changes nothing.
  The step-1 test that expected a no-op on a marker base changed with this rule.
- *What changes for a database without markers.* The promotion is now the apply's: it also resets `_ConfigChngR._MessageNo` and registers for the
  nodes of exchange plans, gives `Files.MobileVersions.dat` its new head, and keeps a recovery artifact of its own. The old exclusive script
  did none of that; the native apply does all of it, so this is the intended difference (the apply's report says in `not_written` what it still does not write).
- *`--sqlcmd` (the legacy runner).* The apply needs the built-in SQL Server client. With `--sqlcmd` the `exclusive` mode keeps the script and keeps refusing markers.
- *Extensions* (`mssql-activate-staged-extension`, `exclusive` on `ConfigCAS`) are another table set and are not touched.
- *Locks.* The script held the application lock `ibcmd-rs:main-activation`, the apply holds `ibcmd-rs:config-apply`. An `online` activation running at the same time is not excluded by the lock, but it holds a session of its own, which the exclusivity check of the apply counts.
- *A JobScheduler connection.* The cluster's word about clients (F-3) also counts the `JobScheduler` connection that the working process keeps for a moment after a session of the infobase ends; the lab script waits until the cluster lists none.

**Tests.**

1. `mssql_main_activation::tests::f4_*` (the two that were red at checkpoint 1; no `ignore` now): the F-4 database of the #344 evidence (an ordinary generation, two online
   generations) and a promotion of an unrelated module: the plan is accepted, the executor is the config apply, the generation it starts from is the last online one, the module renders no script for it, `live` and `worker` still refuse, `online` still extends the
   history; a stage equal to the ordinary rows is not a no-op. Beside them: `for_mode`, the plan of the config apply for `exclusive` only, a marker-free base, the preflight and the plan making the same checks for both executors, the step-1 refusals of the script.
2. `mssql::config_apply_options_tests` (the options of the call), `mssql_config_apply` (`other_sessions_query`; the exemption in the script's exclusivity assertion), `mssql_apply::tests` (the tables of the report).
3. `scripts/apply-lab/f4_repro.ps1`, on lab clones of the corpus (it carries a native online generation): our `online` generation (clone A), a byte-equal copy of that state (C), then **route 2** `mssql-apply-source-change --mode exclusive` on A and **route 1**
   `mssql-stage-source-objects` + `mssql-activate-staged-main --mode exclusive` on C, of another module; the native twin D (a copy of C's staged state, the platform's `config apply`). Checked after each route: no marker, no `_dynupdate_` row, `ConfigSave` empty, every alias row's bytes in the
   ordinary row, and a new external-connection session that sees the online change and the promoted function. Then the native exports of A, C and D compared, and a native apply after ours (it says "не требуется").

**Acceptance of #408** (from the issue): the F-4 repro keeps the earlier online changes and a new session sees both generations; the native export equals the native apply's on the same stage.

**Measured** (`docs/apply/evidence/online-activation/f4-repro-green.txt`, 2026-09-30): before the step, `f4-repro-red.txt`. After it, on the corpus clone with the native generation (5 alias rows plus the versions row) and our generation: route 2 wall 38 s
(export, stage and the apply's 11 s; the apply itself: gate 4.8 s, SQL 4.4 s), route 1 12.7 s; each folded 6 alias rows into the ordinary rows (0 of 6 differ), left no marker and no `_dynupdate_` row, and a new session read `telegram=G2MARK probe=F4`.

### 6.7 Fixed in 0.5 (#409): F-9 and F-10, the gate of the live mode

**What was wrong.** `live` commits the promotion and only then interrupts every connection of the database (`SET SINGLE_USER WITH ROLLBACK IMMEDIATE`) and takes the tail-log
backup. Its preflight (section 3.4) knew the recovery model, the state and the name of the tail file, and nothing else.

- **F-9** With `FULL` recovery but no log backup chain (no full backup since the model was set), a tail directory that does not exist, or one the SQL Server account cannot write to, the promotion
  was committed and the first `BACKUP LOG` then failed (`4214`, `3201`): the new generation was already in the ordinary rows, the operator had an SQL error.
- **F-10** Sessions with a transaction open or a request running were rolled back and disconnected with no question asked; their clients got a transport error and lost the work.

**Measured before** (`docs/apply/evidence/live-gate/f9f10-red.log`; the worker lab cluster (`docs/apply/worker-lab.md`), clone `ibcmd_rs_05_apply_wlab1_20260930`, made marker-free
(`scripts/apply-lab/live/marker_free.ps1`: `live` still refuses a database with online generations, section 6.6), FULL recovery; the old tool):

| Case | What the old tool did |
|---|---|
| no log chain (`SIMPLE` -> `FULL`, no full backup) | promotion committed (`versions` changed), then `BACKUP LOG` failed: error 4214 |
| the tail directory does not exist | promotion committed, then error 3201 (operating system error 3) |
| the account cannot write (`C:\Windows\System32\config`) | promotion committed, then error 3201 (operating system error 5, access denied) |
| a session holds an open transaction with one row written | the switch went on; the session got `A transport-level error ...` at its `COMMIT`, its row is gone; no word from the tool |

**The gate.** `mssql_live_gate.rs`. From `master` (never connecting into the database: a connection there could take the single-user slot), in this order, a `THROW` for the first condition
that does not hold: the database exists (`57230`), FULL or BULK_LOGGED (`57231`), ONLINE (`57232`), the tail file is not there (`57233`) -- these four as before -- then

| Code | Check |
|---|---|
| `57235` | a log backup chain exists: `sys.database_recovery_status.last_log_backup_lsn` is not NULL (NULL after a switch through `SIMPLE`, or with no full backup: exactly when `BACKUP LOG` fails with `4214`) |
| `57236` | the directory of the tail-log path exists on the SQL Server host (`sys.dm_os_file_exists`) |
| (`3201`) | the account can write there: a `COPY_ONLY` backup of `model` (about 0.5 MB compressed) is written next to the future tail file and removed with `xp_delete_file` by its own extension (`.ibcmdrsprobe`). The SQL error of the backup is left as it is, with the operating system's reason; a hint says that the tail-log backup would fail the same way. Not in a dry run |
| `57238` | no session of the database has an open transaction (`open_transaction_count > 0`) or a running request (`sys.dm_exec_requests`), unless the operator accepted it with **`--interrupt-sessions`** (live only; refused for the other modes) |

The same gate runs in four places, so that no route skips it: (1) **before the stage** in `mssql-apply-source-change`, from the built-in SQL client (a refusal leaves `ConfigSave` as it was; the probe is skipped in a
dry run); (2) in `mssql-activate-staged-main` before the script is rendered (the report names the sessions: id, login, host, program, status, open transactions, request); (3) at the head of the
activation script, before its transaction, which is the last word and the only one on the `--sqlcmd` route; (4) **inside the promotion transaction, just before its `COMMIT`** (`57239`): work that
started after the gate ran rolls the promotion back instead of being rolled back by the interruption. What is left is the window between the `COMMIT` and the `ALTER DATABASE` (the `CHECKPOINT` between
them). The report gains `live_gate` (recovery model, chain, tail directory, whether the probe was written, connections, connections of `1CV83 Server`, sessions with open work).

**What the sessions check does not see:** 1C sessions that are inside a server call but have no SQL request at the moment of the sample; the connections that are only idle. The latter are interrupted by design (the 1C
processes reconnect). `--interrupt-sessions` is the operator's word that the rest may be rolled back.

**Measured after** (`docs/apply/evidence/live-gate/f9f10-green.log`; the same cases, the new tool): 

| Case | What the new tool does |
|---|---|
| no log chain | refused in 4.5 s, before the stage: `57235`; `ConfigSave` empty, `versions` unchanged |
| the tail directory does not exist | refused in 5.5 s: `57236` |
| the account cannot write | refused in 5.2 s: `3201` with the operating system's error 5 (access denied) and the hint; the probe file is not left behind |
| a session holds an open transaction | refused in 6.1 s: `57238`, naming session 146 (`ibcmd-lab-writer`, 1 open transaction); the session then commits and its row survives |
| the same with `--interrupt-sessions` | not refused: the promotion is committed, the session is cut off (`A transport-level error`) and its row is gone, as accepted; the run then ends at the F-5 gate (`57234`) |
| `mssql-activate-staged-main` (direct route), no chain | refused by the same gate before the script (`57235`); the stage stays where it was put |
| `--dry-run` | exit 0; the report has `live_gate` (chain, directory, connections, sessions), no probe is written, nothing changes |
| nothing wrong (`clean`) | the gate passes, the promotion is committed, cycle 1 runs (one tail backup set), and the F-5 gate `57234` ends the run: **the old tool does the same on this idle cluster** (see F-5, next checkpoint) |

**Tests.** `mssql_live_gate` (the order of the checks, the probe and its removal, quoting, the acceptance switching the sessions check off in the gate and in the transaction, the report, the refusal texts with the sessions, the
probe hint), `mssql_main_activation` (the gate before the transaction, the check before the `COMMIT`, no gate in the other modes, the acceptance is not part of the plan), the CLI (`--interrupt-sessions` for both commands).

**Not fixed here (F-5, next).** On the worker lab cluster the switch ends at the readiness gate (`57234`) even with no user session and no load, in the old tool and in the new one (the `clean` case of both logs; also `sessions` and `accepted`): the gate expects back the `1CV83 Server` connections it counted before cycle 1, that includes the idle ones the working process holds for the infobase (opened by the tool's own RAS verification, F-3), and an idle process does not reconnect without a call. Section 4.3 measured the same abort under load; F-5 gets its own section.

### 6.8 Experimental 0.5 checkpoint (#409 F-5): explicit staged activation and continuation

The existing `--mode live` default keeps its legacy two-cycle SQL reconnection wait, including error `57234`. The new split route is **opt-in**: `mssql-activate-staged-main --mode live --live-checkpoint`. It requires the built-in SQL client, SQL Server build `17.0.1135.8` (the locally measured 59-column `RESTORE HEADERONLY` layout), and verified platform `8.3.27.2214`. These restrictions are checked before promotion/cycle 1. `mssql-apply-source-change` and its watch route refuse `--live-checkpoint` before processes, export, compilation or staging; a pinned source-to-promotion executor has not been implemented.

The default legacy LIVE format-1 manifest must fit the same 64 MiB bound used by standalone continuation; an oversized pretty-JSON snapshot refuses before promotion/cycle 1. The opt-in route saves its ordinary recovery snapshot plus `<recovery-stem>.live.json` **before** SQL execution. The manifest binds the SQL server, database GUID, family and recovery fork, exact SQL engine build, original verified RAS cluster/infobase, supported platform profile and storage fingerprint, tail-log path, staged row hashes, generation snapshot and SHA-256 recovery token. Promotion and cycle 1 run under a session-owned exclusive application lock in `master`, shared with continuation. Both log sets carry token-bound cycle names. The ordinary F-9/F-10 and marker/alias refusals still apply.

Direct staged-main activation additionally accepts `--live-compact-recovery`
with `--live-checkpoint`. Preserve the format-2 LIVE envelope, its adjacent
`ibcmd-live-<token>.recovery.json`, the content-addressed binary pack and the
tail file together. The envelope and sidecar are each bounded to 2 MiB; the
pack is bounded to 96 MiB and existing per-row/row-set budgets remain in force.
Continuation verifies and reconstructs the same token-bound artifact before
connecting to SQL; it also still reads legacy format1. High-level source apply
does not expose this option. File/dispatch compatibility is independently
reviewed, but actual compact LIVE continuation/recovery remains unmeasured.
See [the format and acceptance boundary](evidence/live-gate/compact-live-recovery-2026-10-01.md).

Readiness currently accepts only an **empty RAS user-session inventory**. The current RAS agent build must still be exactly `8.3.27.2214`, the storage profile must match the saved fingerprint, and the original MSSQL registration is rechecked against the recorded SQL database before inventory and twice before attempting cycle 2; each RAS call has a five-second deadline. Version and identity responses remain strict UTF-8. Session inventory is bounded raw output: only a successful response with empty stderr and ASCII-whitespace-only stdout admits cycle 2. Any nonempty output, including OEM user names and hibernating sessions, refuses. Idle SQL handles of the cluster are not users and do not drive this decision. Any user session, ambiguous output, wrong binding or timeout leaves cycle 1 retained and reports `continuation_required`. The SQL active-work check repeats under the master lock immediately before the second interruption; refusal correctly says the committed promotion/cycle 1 is retained. Active/warm cohort readiness remains unimplemented: this is not an acceptance of F-5 under load.

The boundary is explicit: active SQL work refuses before artifact publication and cycle 1; an SQL-idle but connected RAS user can pass promotion/cycle 1 and then block cycle 2. Closing a thin-client process can leave its RAS session hibernating. End the exact owned session through normal administration before continuation; the tool does not terminate it automatically. Recovery tokens compare the exact SHA-256 independently of hex letter case, retaining the original token spelling and backup names produced by the renderer.

After ending the owned user sessions, resume with the saved manifest:

```powershell
ibcmd-rs mssql-live-continue --artifact F:\lab\recovery.live.json `
  --server localhost --database <same-database> --allow-non-lab `
  --rac '<same-build-rac>' --ras-endpoint <verified-endpoint>
```

Continuation validates identity, clean `ConfigSave`, unchanged published staged bytes and marker-free storage. Under the same lock, `RESTORE HEADERONLY ... WITH CHECKSUM` must show exactly one owned cycle-1 log set, with valid checksums, GUIDs, fork, completion and LSNs, before an append. An intervening log backup refuses append. Exactly two sets are accepted only when both token-bound names and their contiguous backup chain match; this is a validated `already_complete` result and executes no third cycle. Missing, foreign, malformed, unsupported or ambiguous state refuses. A preflight refusal does not change database access mode; cleanup changes it only after this command's own transition. After append the final header/state validation repeats. A transport or verification failure reports completion as uncertain (`cycle_2_executed: null`), never falsely as `false`; inspect the retained files before retry.

A named unfinished checkpoint also blocks default LIVE activation and the source-tree bulk/per-row mutation scripts **under that lock, before target writes**. The pending-history guard looks for the matching named cycle 2, so an unrelated later log backup does not hide cycle 1. This protects these wrapped paths, not every storage-import, standalone stage or administrative command. Preserve SQL backup history and the retained manifest/tail; arbitrary writes, restore, backup-history deletion or other tools invalidate the checkpoint's assumptions. The application lock coordinates this tool's covered paths and cannot prevent an administrator from acting outside them.

The restored research kit is in `scripts/apply-lab/live`; new output defaults to `F:\ibcmd\lab\05\wave1\live`. `obs.ps1 -Srvr localhost:2541 -LabRoot <own-lab>` also supports the existing service83 cluster. Read [checkpoint evidence and remaining work](evidence/live-gate/checkpoint-2026-10-01.md), [real native/RAS measurements](evidence/live-gate/native-ras-2026-10-01.md) and [historical F-5 measurements](evidence/live-gate/f5-findings-2026-09-30.md). The historical measurements establish **client=old/server=new** after cycle 2 until client restart; neither route promises client-code refresh or zero database dialogs. L2 is partial, L5 under warm/load readiness remains open, and #409 remains open.

## 7. Recovery

The standalone recovery output now uses a format-2 JSON manifest and its adjacent
`ibcmd-recovery-<sha256>.pack`. Preserve both files. The manifest names every row,
its full header, offset, length and digest; the pack retains the original bytes.
It is published and checked before SQL can run. Historical files are retained;
the separate `.live.json` checkpoint and its token encoding remain unchanged.
See [compact recovery evidence and limits](evidence/live-gate/compact-recovery-2026-10-01.md).

**ONLINE** recovery must account for new aliases, the overwritten ordinary
`root`/`version` rows, and both history markers. Only `overwritten_config_rows`
are replacement preimages; `retained_config_rows` describe ordinary rows that
publication preserved. Never restore those retained rows over a later
configuration. A safe undo must first verify the exact published generation,
headers and bytes, then remove only its aliases and restore replacement
preimages and prior markers in one transaction. Checksums alone do not establish
database ownership or authorize this operation. A generic guarded undo remains
open (F-8); this manual recovery has not been run.

**A refused apply leaves `ConfigSave` staged** (F-3, any failure after staging; the refusals that need no staged row
- a marker base with an ordinary mode, disagreeing markers, the tail-log argument - come before the stage since 0.5, F-2).
Run `mssql-activate-staged-main` with the same mode (for exclusive see F-3), or empty the stage by staging the next change (the
staging replaces `ConfigSave`).

**LIVE after `57234`** (database `ONLINE`, promotion committed, tail file holds cycle 1): the sessions are in a mixed
state until every old session ends. The historical manual second-cycle experiment can show a database dialog; blindly repeating it caused an unknown cluster/COM hang. Legacy artifacts lack the new token-bound manifest and are not automatically resumable by section 6.8. Inspect the retained state before any manual recovery. **After `57250`** (database `RESTORING`): `RESTORE DATABASE [<db>] WITH RECOVERY;`. The `.trn` is part of the
log chain: keep it (62 MB for the first cycle after a full backup in the measured runs, 3-6 MB for later ones).

**Split or stale generations after any aborted live/worker:** end all sessions of the infobase (a fresh session then sees
one generation, measured), or restart the working process (worker: the tool's own signal).

## 8. Limits

- One existing module body or common-form body of the **main** configuration per call; everything else is rejected before
  any write (`mssql_apply.rs:591-619`); extensions: online/exclusive only.
- Row limits: 128 staged rows, 16 MiB per row, 32 MiB per plan, 16 MiB inflated `versions` (`mssql_main_activation.rs:15-18`);
  `PartNo` must be 0 (multi-part rows are refused).
- A base with a `DynamicallyUpdated` marker: `online` applies in one step, also to an object that already has an alias (F-1,
  F-2, fixed in 0.5); `exclusive` on it is carried out by the own apply, which folds the earlier generations (F-4, #408 step 2);
  `live`/`worker` (and `exclusive` with `--sqlcmd`) are refused before the stage, because their script would discard them.
- `exclusive` needs an infobase on which the cluster lists no client connection or session; the idle SQL sessions of the tool's
  own RAS verification are left out of its gate (F-3, fixed in 0.5). A session of another program, or a running one, still refuses.
- `live` needs FULL/BULK_LOGGED recovery, a full backup taken after that, a tail-log path writable by the SQL Server account
  (all three are checked before the stage since 0.5, F-9), and a machine on which the 1C SQL connections return within 4 s (F-5, open); it interrupts every
  database connection: sessions with open work are refused unless `--interrupt-sessions` is given (F-10).
- `worker` needs exactly one dedicated `rphost` for the infobase, RAS and `rac` on the same host.
- The observed BSP client shows a modal message after a lost database connection; nothing here changes that.
- The drop-in's `infobase config apply --dynamic=force` (#347, [`dropin-dynamic.md`](dropin-dynamic.md)) runs the `online`
  transition unchanged with two additions a plan can carry: the writes the platform's `force` makes besides the rows (the
  change registrations and `MobileVersions.dat`, `MainActivationPlan::with_parity_sql`) and the platform's timestamps for the
  markers (`with_platform_timestamps`; the plans of this file's own commands keep UTC). Its twin against the platform's `force`
  is in `evidence/dropin-dynamic/acceptance.md`.

## 9. Evidence and how to repeat

Index and file list: `openspec/changes/direct-mssql-online-activation/evidence/online-live-8327-20260929/README.md`. The lab kit,
`scripts/apply-trace/lab/online-live/` (paths are the lab's `F:\ibcmd\lab\05\online`; `lab-env.ps1` holds them):
`apply.ps1`/`act.ps1` run one command with wall-clock bookends, `cap-apply.ps1` wraps them in the capture kit (snapshots,
trace, diff) with hooks that arm the transaction observer before the command and start NEW/lazy observers after it,
`obs-start.ps1`/`obs-stop.ps1` start and stop observer clients, `sql-timeline.ps1` (database state from `master`, never
connecting into the database), `rac-timeline.ps1`, `load-sampler.ps1`, `timelines.ps1`, `gen-state.ps1` (generations and
markers), `sessions-clean.ps1`, `make_versions.py` (source trees), `analyze_obs.py` (segments, gaps, errors relative to a
commit time), `build-observer.ps1` (Designer batch on a throw-away file infobase). The first client start on a restored
БСП clone shows "Информационная база была перемещена или восстановлена из резервной копии" and blocks the session; on `a1`
the dialog was simply not shown again after the first client was closed, on `b1` an Enter posted to it (`uia-click.ps1`)
closed it and the windows kept the "[КОПИЯ]" title. `winshot.ps1` and `uia-click.ps1` touch only the windows of the lab's
own client process.

Before repeating: register the clone with `register-ib.ps1` (8.3.27), for `live` make it FULL and take a full backup, and
unregister it afterwards. Do not run `worker`.
