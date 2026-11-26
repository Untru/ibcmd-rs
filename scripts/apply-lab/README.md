# Apply lab: the scripts of the removals twins (#393)

Scripts of track apply for the twins that compare `ibcmd-rs mssql-config-apply` with the platform's `ibcmd infobase config apply`
on **the same stage** (`docs/apply/own-apply.md`, "Removals"). They run on the lab of the 0.4 tracks only
(`F:\ibcmd\lab\04\apply`, databases `ibcmd_rs_04_apply_*`, the native lock of `F:\ibcmd\lab\04\tools\heavy-lock.ps1`), and use the
ddl track's kit (`scripts/restructure-lab`: `snapshot.py`, `snapdiff.py`, `si_diff.py`, `dbschema_cmp.py`, `compare_tables.ps1`) with
`DDL_LAB=F:\ibcmd\lab\04\apply\ddlkit`.

| Script | What |
|---|---|
| `native_import.ps1` | the platform's `config import` of a tree into a clone, with a `--data` directory **of its own for the database** (a shared one made the import fail or stage a wrong set), under the native lock |
| `native_cmd.ps1` | any other native `ibcmd` command on a clone (`infobase config apply --force --dynamic=disable`, `config export`, `config check`), the lock taken for the writes |
| `native_twin.ps1` | a fresh clone of the БСП 8.3.27 corpus, native `config import` repeated until the stage is whole (the first import of a fresh clone stages a subset or fails), snapshot, COPY_ONLY backup, native apply, snapshot |
| `refusal_cases.ps1` | five lists that `mssql-config-apply` must refuse as a whole, planned with `--dry-run` on the clone that holds a removal stage |
| `evidence_393.ps1` | collects the checks of the two twins into `docs/apply/evidence/own-apply/removals-393-twin-checks.txt` |
| `deleted_cmp.py`, `deleted_row.py`, `deleted_classify.py` | the `deleted` row of a stage: format and set of names against another row, its text, its names against `Config` |
| `reg_cmp.py` | the change registers of two clones: message numbers and file lists per (node, object) |
| `reg_state.py` | the register rows of the removed form and template: `show`, and `setup` (message numbers and missing rows) |
| `final_acceptance.ps1` | our `infobase config import` and drop-in `infobase config apply` of a tree on a fresh clone, the platform's apply of the same stage on a twin, the platform's re-apply and export of our result, and the comparisons |
| `stage_tools.py` | helpers for stages in disposable clones (copy a stage, add or drop the `deleted` row, ...) |
| `tree_edit.py`, `unused_modules.py`, `module_mentions.py` | edits of a working copy of the exported tree, and which common modules the rest of the tree mentions |

The stage of the first twin is what `ibcmd-rs infobase config import` (branch `feat/0.4-import-override`) writes for the tree of the
import lab with the form `ВсеЗаметки` and the template `ДатыПасха` removed; the second adds the removal of the first attribute of
`Catalog._ДемоГруппыДоступаНоменклатуры` (`edits.py attrdel`).

## #408 step 2 (the exclusive mode of the old activation commands)

| Script | What |
|---|---|
| `f4_repro.ps1` | the F-4 repro and its acceptance (green since #408 step 2, `docs/apply/evidence/online-activation/f4-repro-green.txt`): clones of the corpus (it carries a native online generation), our `online` generation, a byte-equal copy, then route 2 (`mssql-apply-source-change --mode exclusive`) and route 1 (`mssql-stage-source-objects` + `mssql-activate-staged-main --mode exclusive`) of another module, the native twin (the platform's `config apply` of the same stage), the native exports compared, and a native apply after ours ("не требуется"); `-Steps` continues a stopped run |
| `f4_session.ps1` | a NEW external-connection session (COM, Windows PowerShell 5.1) that reads `ТипыВнешнихСистем().Telegram` and `ПроверкаIbcmdRsF4()` |
| `f4_trees.py` | the two source trees of the repro (one common module each, ExternalConnection = true) |

## The worker lab cluster (0.5; `docs/apply/worker-lab.md`)

`cluster/`: a separate console `ragent` + `rmngr` + `ras` of platform 8.3.27.2214 on the ports 5540/5541/5545 and 5560-5591, its registry in
`F:\ibcmd\lab\05\cluster`, started for a run and stopped after it (no Windows service, no existing cluster touched).

| Script | What |
|---|---|
| `cluster/start.ps1 -Track <t>` | start it (about a minute on a fresh registry); refuses when it already runs or a port is taken |
| `cluster/stop.ps1 [-Purge]` | stop every process of it and show that none is left and no port listens |
| `cluster/status.ps1` | state, processes, listeners, cluster, infobases, working processes, sessions |
| `cluster/with-cluster.ps1 -Track <t> -Run <script>` | start, run a script, stop whatever happens |
| `cluster/session.ps1` | one COM session of a clone, held until a release file appears |
| `cluster/smoke.ps1` | start, register one clone (`register-ib.ps1 -Cluster worker`), one COM session and one 1cv8c session listed by `rac session list`, unregister, stop, no process left, the other clusters unchanged |

## The live gate: F-9 and F-10 of #409 (`live/`, `docs/apply/online-activation.md` 6.7)

Run against the worker lab cluster (`cluster/`), on a marker-free FULL-recovery clone; the tail files go to `F:\ibcmd\lab\05\live\bak`.

| Script | What |
|---|---|
| `live/f9_f10.ps1` | the cases: no log chain, no tail directory, a directory the SQL Server account cannot write, a session with an open transaction, the same accepted with `--interrupt-sessions`, the direct `activate-staged-main` route, `--dry-run`, and a clean run. `-Label red` runs the old tool (the defect is reproduced), `-Label green` the fixed one |
| `live/hold_txn.ps1` | a session that holds an open transaction with a row written (`dbo.IbcmdRsLiveProbe`) until a release file appears: the work in flight the live switch would roll back |
| `live/marker_free.ps1` | a marker-free clone: the two `DynamicallyUpdated` markers and the `_dynupdate_` rows deleted (`live` refuses a database with online generations) |
| `live/live_trees.py` | one source tree per tag (a module with the marker `LIVE-<tag>`), so that every run promotes something new |
