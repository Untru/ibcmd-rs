# Lab kit of issue #344: online / live activation with real 1C sessions

What the scripts were used for: `docs/apply/online-activation.md`; what they produced:
`openspec/changes/direct-mssql-online-activation/evidence/online-live-8327-20260929/`. All paths are the lab's
(`F:\ibcmd\lab\05\online`, the 8.3.27.2214 cluster on `localhost:2541`/RAS `2545`); `lab-env.ps1` holds them. Only lab
databases (`ibcmd_rs_04_*`, `ibcmd_rs_05_*`) registered with `F:\ibcmd\lab\04\tools\register-ib.ps1` are accepted.

| script | does |
|---|---|
| `apply.ps1`, `act.ps1` | one `mssql-apply-source-change` / `mssql-activate-staged-main` run: stdout to `<tag>.json`, stderr to `<tag>.err`, the rendered script and the recovery artifact next to them, `<tag>.meta.txt` with UTC start/end, wall ms and exit code |
| `cap-apply.ps1` | the capture kit (`scripts/apply-trace/capture.ps1`: snapshots, Extended Events trace, diff) around `apply.ps1`/`act.ps1`; `-ArmTxn <label>` starts the observer's 60 s server transaction just before the command, `-StartNew a,b` launches NEW observers right after it, `-ArmLazy <label>` lets the lazy observer touch the module for the first time |
| `make_versions.py` | source trees `src/<tag>/` for one common module (owner XML + `Module.bsl` of the reference export with the marker value changed) |
| `build-observer.ps1`, `observer/` | the external data processor `IbcmdRsObserver.epf` (Designer batch on a throw-away file infobase) |
| `obs-start.ps1`, `obs-stop.ps1` | start / stop observer clients (`1cv8c.exe ENTERPRISE ... /Execute ... /C "<label>;<mode>"`, modes `poll`, `lazy`, `txn`); the journal is `obs\<label>.log` |
| `timelines.ps1` | starts / stops `sql-timeline.ps1` (database state from `master`; **never connects into the database**, because a connection there could take the `SINGLE_USER` slot of a live activation), `rac-timeline.ps1` (sessions/connections of one infobase through RAS), `load-sampler.ps1` |
| `sessions-clean.ps1` | terminates every session of one lab infobase (killed clients leave hibernated sessions behind) |
| `gen-state.ps1`, `dump-versions.ps1`, `rowhex.ps1`, `inflate_diff.py` | read-only views: generations and markers, inflated `versions`, raw rows, diff of two raw-deflate rows |
| `analyze_obs.py` | per journal: marker segments, gaps, errors, relative to a time (`--t0`) |
| `winshot.ps1`, `uia-click.ps1` | capture / operate **only the windows of the lab's own client process** (`PrintWindow`, UI Automation); used to read modal errors and to answer the БСП "database moved or restored" dialog once per restored clone |
| `live-try.ps1`, `act-dry.ps1`, `find_marker_modules.py` | one lean live attempt; dry-run of the activation; list of small common modules with their environment flags |

Traps found while building it (each cost a run):

- The thin client rejects `ПодключитьОбработчикОжидания` intervals below 1 s ("Обработчик ожидания с нулевой задержкой...").
- `НомерСеансаИнформационнойБазы()` is not available in the thin client (ask the server); `Новый Файл(...).Существует()` is a
  syntax error (assign the object first).
- A restored БСП clone asks "Информационная база была перемещена или восстановлена из резервной копии" at the first start
  and blocks the session; posting mouse messages (`PostMessage WM_LBUTTONDOWN`) to it is ignored, the UI Automation `Invoke` is
  "not implemented"; an Enter posted to the dialog window closes it (which button is the default was not verified; the
  window title kept "[КОПИЯ]" afterwards, which suggests the copy answer).
- `Start-Process -NoNewWindow` cannot be combined with `-WindowStyle`; a script block cannot be passed through `pwsh -File`;
  in a PowerShell method call a comma separates arguments (`$w.WriteLine("{0}" -f $a, $b)` is two arguments).
- Python heredocs in Git Bash turn `\05`, `\t` in Windows paths into control characters; write raw strings or `chr(92)`.
- `rac ... --infobase-user` opens a cluster connection and SQL sessions that stay for the whole observation.
