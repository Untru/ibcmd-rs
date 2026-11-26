# The worker lab cluster (0.5, checkpoint 1: design, start/stop kit, smoke)

A cluster of our own for the runs that the shared clusters cannot host: the `live` and `worker` activation modes of
`mssql-apply-source-change` / `mssql-activate-staged-main` (`docs/apply/online-activation.md`, sections 3.4, 3.5 and 8). `live` interrupts
**every** connection of the database (`ROLLBACK IMMEDIATE`, F-10) and needs the 1C SQL connections to come back within 4 s (F-5, F-9);
`worker` turns off a working process (`rac process turn-off`, F-11). Neither may run on the service clusters, which carry Pavel's work and the other
tracks' sessions. This document is the design and the record of the kit; **no `live` or `worker` run has been made** (that is the next
checkpoint, after the go).

## 1. What it is

A separate **console** `ragent` (the cluster agent, which starts its own `rmngr`, the cluster manager, and the working processes `rphost`) and a
console `ras`, of platform **8.3.27.2214** (the build the `live`/`worker` code targets), on ports of their own and a cluster registry of their
own. They are ordinary processes started by a script for a run and stopped by the script when the run ends. **No Windows service is installed,
changed, started or stopped.**

| | value | why |
|---|---|---|
| platform | `C:\Program Files\1cv8\8.3.27.2214\bin` (`ragent`, `rmngr`, `rphost`, `ras`, `rac`) | the build of the profile `platform-8.3.27.2214` |
| agent | `-agent -port 5540 -regport 5541 -range 5560:5591 -d F:\ibcmd\lab\05\cluster\srvinfo` | the ports of the services are 1540/1541/1560-1591, 2540/2541/2560-2591, 3540/3541/3560-3591 |
| RAS | `ras cluster --port=5545 localhost:5540` | the services' RAS are 1545, 2545, 3545 |
| clients | `Srvr="localhost:5541";Ref="<db>"` (COM), `1cv8c ENTERPRISE /S"localhost:5541\<db>"` | the same form as the lab uses for `localhost:2541` |
| `rac` | `rac.exe localhost:5545 ...` | |
| cluster registry | `F:\ibcmd\lab\05\cluster\srvinfo` (the agent creates the cluster on the first start) | not `C:\srvinfo_*` |
| state | `F:\ibcmd\lab\05\cluster\state.json` (who started it, the pids, the ports), `logs\` | |

Port 5600 is used by another program; the cluster uses 5540, 5541, 5545 and 5560-5591 only. `start.ps1` refuses to start when any of them is
listening.

## 2. Guarantees, and what enforces each

| The cluster must | How |
|---|---|
| not touch or re-register any existing cluster or infobase | it has its own `-d` registry and its own ports; the scripts call `rac` only at `localhost:5545`; `register-ib.ps1 -Cluster worker` looks only at that RAS. The smoke reads the service cluster (`rac localhost:2545 cluster list`, `infobase summary list`) **before and after**, read only, and compares the server processes (id, name, start time) and the infobase names |
| be a console process, not a service | `Start-Process ragent.exe` / `ras.exe` hidden; no `-srvc`, no `-instsrvc`, no `sc`. The agent and RAS are started without redirected handles, so a caller that captures the output of `start.ps1` through a pipe returns (with a redirect they inherit the pipe and the caller waits for the agent to exit; found in the first smoke run) |
| stop when the run ends | `stop.ps1` kills the working processes, manager, RAS and agent, then checks that no process of the cluster remains and none of its ports listens; `with-cluster.ps1` runs a script between `start.ps1` and `stop.ps1` in a `try/finally` with a time limit |
| never kill anything else | a process is "ours" only if its command line names our data directory or one of our ports (`-port 5540`, `-regport 5541`, `--port=5545`, `-range 5560:5591`), or it descends from such a process. The services run under another account: their command line is **not visible** to us (empty), so they are never candidates, and the account cannot end them anyway |
| refuse to run twice | `start.ps1` exits 2 when processes of the cluster exist, when a port of it is taken, or when the platform is missing; it names the track that started the running one |
| read `.env` nowhere | the kit never opens `D:\EDT\config\.env`. The SQL login the cluster uses to reach a clone is written by `register-ib.ps1`, which stays the only place that reads it and never prints it |

**Licensing.** The extra server and the sessions are started with no licence setting of any kind. If the platform refuses the server or a session
for a licensing reason, `session.ps1` exits 3, `smoke.ps1` stops the cluster and exits 3 with `LICENSE: ...`, and the run ends there: it is reported,
not worked around (no key, licence server, licence file or account setting is looked for or changed).

## 3. The kit (`scripts/apply-lab/cluster/`, branch `feat/0.5-worker-lab`)

| Script | What |
|---|---|
| `lib.ps1` | the constants (ports, paths) and the helpers: which processes are ours, listeners on our ports, `rac` against our RAS, a snapshot of the other clusters' processes, the bounded stop |
| `start.ps1 -Track <t>` | starts the agent, waits for 5540/5541, starts RAS, waits for 5545 and for `rac cluster list` to answer, writes `state.json`. Exit 0 started, 2 refused, 1 failed (it stops what it started) |
| `stop.ps1 [-Purge]` | stops every process of the cluster and proves that none is left; `-Purge` also deletes the registry and the logs (the next start is a new cluster) |
| `status.ps1` | state, processes, listeners, and (running) the cluster, infobases, working processes and sessions from `rac`. Read only |
| `with-cluster.ps1 -Track <t> -Run <script> [-MaxMinutes 90] [-Purge]` | start, run the script with `IBCMD_WORKER_SRVR`, `IBCMD_WORKER_RAS`, `IBCMD_WORKER_RAC`, `IBCMD_WORKER_CLUSTER` in its environment, stop, whatever happens |
| `session.ps1` | one COM session of a clone, held open until a release file appears (so `rac session list` can see it); Windows PowerShell 5.1 |
| `smoke.ps1` | the smoke test of section 4 |

`F:\ibcmd\lab\04\tools\register-ib.ps1` (the coordinator's tool, outside the repository) got a `-Cluster default|worker` parameter (default: as
before, output unchanged): `-Cluster worker` uses `rac` 8.3.27.2214 at `localhost:5545` and prints `Srvr="localhost:5541"`; it refuses when the
worker cluster is not running ("never started from here") and when `-Platform` is not `8.3.27`. `drop-lab-dbs.ps1` looks only at the two service
clusters, so **a clone is unregistered from the worker cluster before it is dropped** (the smoke does it; the registry of a stopped cluster is not
visible to the tool). The clones are named `ibcmd_rs_05_apply_*`.

## 4. Smoke result

`smoke.ps1`, 2026-09-30, **GREEN twice** (13:05 and 13:20, 13 checks each; `docs/apply/evidence/worker-lab/smoke-2026-09-30-run1.txt`, `-run2.txt`); the times below are those of run 1, run 2 was slower because the machine was busier:

| Step | Result |
|---|---|
| start | 60 s (run 2: 86 s) from `start.ps1` to `rac` answering; the cluster has an id of its own (`eb00f098-...`), not the service's (`24c580ef-...`); processes `ragent`, `rmngr`, `rphost` (port 5560), `ras` |
| register | the БСП clone `ibcmd_rs_05_apply_wlab1_20260930` is registered with `register-ib.ps1 -Cluster worker` (infobase `5487dcd9-...`); it is listed by the worker cluster's `rac` and **not** by the service cluster's |
| COM session | connects (about 45 s, the first session of an infobase starts its connections): `config=БиблиотекаСтандартныхПодсистемДемо version=3.1.11.466 user=Администратор`; `rac session list`: `session-id 1`, `app-id COMConnection`, working process `...:5560` |
| thin client | `1cv8c ENTERPRISE /S"localhost:5541\<db>"` connects (about 28 s); `rac session list` shows `app-id 1CV8C` |
| licensing | **no refusal**: both sessions were granted a licence with no setting of any kind changed |
| unregister, stop | no session left, the clone unregistered; `stop.ps1` 8-16 s |
| after the stop | no `ragent`, `rmngr`, `rphost` or `ras` of the cluster remains; none of 5540, 5541, 5545, 5560-5591 listens; the state file is gone |
| the others | the 9 server processes of the other clusters (`ragent`, `rmngr`, `ras`) are the same ones (id, name, start time); the service 8.3.27 cluster `24c580ef-...` still has its 17 infobases; the count of their `rphost` was 7 before and after |

`with-cluster.ps1` was exercised with two dummy scripts: one that exits 7 (the wrapper exits 7, the cluster is stopped, nothing left) and one that hangs
(`-MaxMinutes 1`: the run is ended, exit 124, the cluster is stopped, nothing left); a second `start.ps1` while the cluster runs exits 2 and names the holder.
The first smoke run hung: `start.ps1` had started the agent with redirected output, so a caller that captures the script's output through a pipe
(as `smoke.ps1` does) waited for the agent to exit. Fixed by starting the agent and RAS without redirected handles.

The state after the smoke: the cluster is stopped and purged (`stop.ps1 -Purge`); the clone `ibcmd_rs_05_apply_wlab1_20260930` stays for the next checkpoint.

## 5. What the bring-up showed

- The agent needs about 37 s from the process start until it listens (on a fresh registry and on an existing one alike), RAS about 16-18 s more: `start.ps1`
  takes about a minute. `stop.ps1` takes 8-16 s (up to 40 s when `rac` answers slowly while the processes are being ended).
- Right after the start the manager has already started one working process (`rphost`, port 5560); `rac process list` shows it with no infobase.
- A COM session on a clone starts the infobase's connections: the session itself (`app-id COMConnection`, on that working process) and a
  `JobScheduler` connection of the process, although scheduled jobs are denied for the registered infobase. The `exclusive` mode of the activation commands counts the latter as a client
  (F-3) until it disappears; the F-4 script waits for it.
- The thin client (`1cv8c`) and the COM connector both get a licence on this machine the same way as on the service clusters (see the smoke); the
  build of `V83.COMConnector` is the 8.3.27.2214 one and it reads the configuration name and version of the БСП demo clone.

## 6. Rules for the runs that follow

1. Start the cluster through `with-cluster.ps1` (or `start.ps1` and `stop.ps1` in a `try/finally`) so that it is stopped when the run ends; check `status.ps1` before
   you leave. Nobody else may start it while a run holds it: `start.ps1` refuses and names the holder.
2. One infobase per run when the run relies on the `worker` mode's "exactly one dedicated `rphost`": with a single registered infobase the single
   working process is dedicated; `rac server update` (infobases per process, connections per process) is a setting of this cluster only and is for the run to set and to record.
3. Clones are `ibcmd_rs_05_*`; register, run, **unregister**, stop, then drop with `drop-lab-dbs.ps1`.
4. The tool under test (`mssql-apply-source-change --mode live|worker`) gets `--rac <rac 8.3.27.2214> --ras-endpoint localhost:5545 --cluster-id <id> --infobase-id <id>`
   of this cluster; nothing points at 2545 or 3545.
5. The load of a run (F-5) is a matter for the `heavy` lock of the lab tools when it is heavy; the cluster itself has no lock but its own refusal to start twice.

## 7. Open points

- `drop-lab-dbs.ps1` could also ask the worker cluster (when running) for its infobases, and `heavy-lock.ps1` could get a `worker` lock name so that two tracks queue for the
  cluster instead of the second start being refused. Neither is changed here (both are the coordinator's tools).
- No cluster tuning is done for `live`/`worker`: that belongs to the first run of each (checkpoint 2).
