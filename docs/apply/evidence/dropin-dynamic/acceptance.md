# Acceptance of `--dynamic=force` (issue #347, checkpoint 2): the twin of the platform's `force`

2026-09-30, platform 8.3.27.2214, SQL Server 2025, the БСП 8.3.27 corpus (`bsp_native_20260923.bak`, a clone that already
carries one dynamic generation `06cb0442-...` with two objects), a cluster at `localhost:2541`. Native runs in direct mode
(`ibcmd infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=<db> --dynamic=force`, stdin from an empty
file, the native lock), ours through the drop-in of this branch (`ibcmd-rs infobase config apply ... --dynamic=force`).
The lab kit is `F:\ibcmd\lab\05\ui\tools` (`nat-apply.ps1`, `ours-apply.ps1`, `dyn_state.py`, `content_sums.py`,
`tblsnap.py`, `obs-start.ps1`, `client-start.ps1`, `run_native.ps1`, `compare_trees.py`); the runs are in `runs\dy\`
(`dy_*`, `fold_*`, `st_*.json`).

## 1. The stage and the twins

The stage is what the source-driven route makes for one common module (`mssql-stage-source-objects --per-row --path-prefix
CommonModules/ОбсужденияСлужебныйКлиентСервер`): **5 rows**, 345 162 bytes -- the module's descriptor (165 B), its `.0` body
(704 B), `root` (45 B), `version` (28 B) and `versions` (344 220 B) -- staged once on a clone, the change register of the
module's owner seeded (`_MessageNo = 5` on its three rows), then the database backed up (`COPY_ONLY`) and restored under a
second name, so that native and ours start from byte-identical rows. Pair 2 differs by one thing: **all the register rows
of one exchange-plan node were removed** (5 501 rows and their file lists), the state of a node with an initial image
(#412).

| Twin | Start | Run |
|---|---|---|
| `dy_p` (dropped) / `dy_q` (dropped) | base + stage + seed | native `force` / ours `force`, **a `1cv8c` client and a polling observer session connected** |
| `dy_p2` / `dy_q2` | base + stage + seed + imaged node | native `force` / ours `force`, no client |
| `dy_p3` (dropped) | as `dy_p` | native `force` with a client and an observer connected |
| `dy_p5` / `dy_q4` | as pair 2 | native `force` / ours `force` (final binary), no client |

## 2. The state after the apply: equal

Compared with `dyn_state.py --diff` (row counts and bytes of `Config`, `Params`, `Files`, the register and its file lists;
name, part, size and SHA-256 of every alias row, `root`, `version`, `versions`, the module's rows and the markers; the
markers' bytes; `MobileVersions.dat` behind its head; every `_ConfigChngR` row by node, object and message number):

| Compared | Result |
|---|---|
| native `force` with a client (`dy_p3`) against ours with a client (`dy_q`) | **0 differences** |
| native `force` (`dy_p2`) against ours (`dy_q2`), imaged node | **0 differences** |
| native `force` (`dy_p5`) against ours (`dy_q4`), imaged node, final binary | **0 differences** |

What that state is (`dy_q`, ours; the same for native): `Config` 9 847 -> **9 850 rows**, 136 849 370 -> **137 194 496
bytes** (the three alias rows and the marker growing from one generation to two); the alias rows
`313d9858-3995-4a4c-b2b0-15d2350417b4_dynupdate_85881c00-a93f-417f-bf0f-bce9a97535fb` (165 B), `....0` (704 B) and
`versions_dynupdate_85881c00-a93f-417f-bf0f-bce9a97535fb` (344 220 B); `Config.DynamicallyUpdated`
`{1,2,06cb0442-...,85881c00-...}` (82 B) and `Params.DynamicallyUpdated` `{0,3,848a0a59-...,06cb0442-...,85881c00-...}` (119 B);
`root` and `version` replaced in place; `ConfigSave` empty; `_MessageNo` of the module's three rows 5 -> NULL; the ring of
`MobileVersions.dat` with a new head GUID and the same 999 behind it. The line printed is the same as native's:
`Создано поколение конфигурации: 001c88853fa97f41bf0fbce9a97535fb00000000`.

The content of **every service table** (`content_sums.py`: `Config` 9 850 rows, `Params`, `Files` but `MobileVersions.dat`,
`_ConfigChngR` by node/object/message number, `_ConfigChngR_ExtProps` by object/key/file, `ConfigSave`, `SchemaStorage`,
`DBSchema`, `_YearOffset`, `IBVersion`) is equal between `dy_p5` (native) and `dy_q4` (ours), **except the `Params` `.ui`
rows**, six rows the native `force` re-encrypts with a fresh vector (the known gap `not_written`: the own applies never
write them, #340). The timestamps are the platform's now: the markers carry `Modified` in local time shifted by the year
offset (`4026-09-30 13:14:51`, as native's `4026-09-30 12:43:34`); the first build stamped them in UTC and the twin showed it.

The **imaged node**: with all register rows of one node removed, the platform's `force` inserts the missing row of the
module and its file list (`_ConfigChngR` 15 184 -> 15 185, `_ConfigChngR_ExtProps` 15 882 -> 15 883), as the own exclusive
apply does (#412), and ours does the same (`registrations.rows_added: 1`); only the opaque `_IDRRef` of the inserted row
differs (native: a time-based id, ours: the next of the table's sequence, as in the exclusive apply).

The client removes one `Params` row (`ecsreg_...`, 306 B) when it connects; the twins with a client (`dy_p3`, `dy_q`) both
lack it, the twins without one (`dy_p2`, `dy_q2`, `dy_p5`, `dy_q4`) both have it. The apply writes nothing there.

## 3. Sessions

A `1cv8c` thin client and an observer session (`IbcmdRsObserver.epf`, polls the marker of the changed module once a second)
were connected before the apply.

| Session | Before | After ours (`dy_q`) | After native (`dy_p3`) |
|---|---|---|---|
| open before the apply | `Telegram` | `Telegram` for the 878 further lines, no error | `Telegram` for the 712 further lines, no error |
| opened after the apply | -- | `ibcmd-online-v3` from its first line, no error, also across a **second** dynamic apply (`v2`) made while it ran | `ibcmd-online-v3` |

Ours and native's behave the same, and as the source-driven online apply did (`online-activation.md` section 4): a session
keeps the generation it loaded, a session opened afterwards reads the new one. Neither run tells the cluster (the platform
against a database does not either).

A second dynamic apply of the same module (`v2`), with the first generation's aliases in place, published a third
generation (history `06cb0442-...`, `85881c00-...`, `be969279-...`), 6.5 s with the client and the observers running; the
descriptor was compared with the row of the earlier generation, not with the plain one.

## 4. The refusals, on `dy_q` with the client connected

| Command | Result |
|---|---|
| `--dynamic=auto` | exit -1, `Ошибка исключительной блокировки информационной базы.`, the five sessions, `Закройте их ... и повторите`, **`можно применить динамически: --dynamic=force`**; nothing written, `ConfigSave` still 5 rows |
| `--dynamic=prompt` | the same, hint included |
| `--dynamic=disable` | the same **without** the hint |
| `--dynamic=force` | exit 0, 1.9 s (SQL 0.54 s), the generation line above |
| a stage of 130 rows beside the module's (`auto`) | the same refusal for the sessions, **no hint** (the stage would not qualify) |
| the same stage (`force`) | exit 1, `требуется штатный config apply: the stage holds 135 rows and a dynamic update takes at most 128 (the rows of a few objects and the service rows); a whole-tree stage of `config import` is not one (#395)` |
| a stage of a catalog (descriptor and `.0` copied from `Config`) | exit 1, `требуется штатный config apply: 0a136ad9-...: the object is a Catalog; only common modules and common forms are published dynamically (other kinds are not measured with sessions connected yet)`, one reason for the object |
| `--dynamic=force --platform=8.5.1` | exit 1, ``Параметр `--dynamic=force` команды `infobase config apply` не поддерживается для платформы platform-8.5.1.1150: ... (capability `mssql.config.apply.dynamic` is explicitly unsupported ...)`` |
| `--dynamic=force --platform=8.3.27.1989` | exit 1, the same words, `... is not declared for platform profile ...` |

Timing: ours **1.6-2.0 s** on a quiet machine (`sql_ms` 542-572; 6-10 s while native runs and exports were competing for
the CPU), native 26-72 s.

### 4a. Locks (`tools\lockprobe.py`, three runs on fresh clones of pair 2, no client)

`sys.dm_tran_locks` of the database sampled every few milliseconds while `--dynamic=force` ran, and the lock counters of
the indexes (`sys.dm_db_index_operational_stats`) read before and after (`runs\dy\lockprobe1-3.txt`):

* **no lock escalation attempted** on `Config`, `ConfigSave`, `Params`, `Files`, `_ConfigChngR`, `_ConfigChngR_ExtProps`
  (`index_lock_promotion_attempt_count` 0 on every index after the run);
* what is held: key locks (`U`, `X`, `RangeX-X`, a handful) on the rows it writes; `PAGE X` on `Config` while the 344 KB
  `versions` alias is written (44 pages); key-range locks `RangeS-S`/`RangeS-U` on `ConfigSave` (5 rows) and on the register
  it scans for the reset (11 775 at the peak on `_ConfigChngR`, 2 013 on its file list) for the ~0.5 s of the transaction;
  intent locks on the tables; **a shared table lock on `_YearOffset`** (a one-row heap read for the year offset);
* no table lock on `Config`, `ConfigSave`, `Params` or `Files`. In one of the three runs a single sample (a few
  milliseconds) showed an exclusive table lock on the two register tables at once, and in another one on `Config`; none was
  held for more than a sample, and the counters above say it was not an escalation. The register tables are read and written
  only by configuration operations.

The observers of section 3 polled a common module and a common form once a second through all of it: every journal has the
same gap of about 7 s every 30 s (the observer's own, present before, during and after each apply and in the native run),
and none that belongs to the apply.

## 5. Export and fold

`ibcmd infobase config export` (native) of the two twins after the dynamic apply: **12 198 files, 12 198 identical**
(`dy_p2` native `force` against `dy_q2` ours); the exported module carries `ibcmd-online-v3` in both.

The next exclusive apply (a second 5-row stage of the same module, `v2`, copied byte for byte into the three databases):

| Database | History | Apply | Result |
|---|---|---|---|
| `dy_p2` | native `force` | native `--dynamic=disable` | `Config` 9 850 -> **9 841 rows, 136 508 294 bytes**, generation `48b1b476-...` |
| `dy_q3` (a copy of `dy_q2` after ours) | ours `force` | **native** `--dynamic=disable` | 9 841 rows, 136 508 294 bytes, the same generation |
| `dy_q2` | ours `force` | **ours** `--dynamic=disable` (the drop-in) | 9 841 rows, 136 508 294 bytes, the same generation, 7.7 s |

Native exports after the three folds (the module carries `ibcmd-online-v2`): `dy_p2` against `dy_q2` **12 198 of 12 198
identical**; `dy_p2` against `dy_q3` **12 198 of 12 198 identical**. `content_sums.py` of the three: `Config`, `Params`
(ordinary rows), `Files`, the register and its file lists **equal**; the `.ui` rows differ (native re-encrypts them,
random); and one real difference: **the native exclusive apply leaves `Params.DynamicallyUpdated`** (`{0,3,...}`, the
history that was folded) while the own exclusive apply deletes it, whatever generation made the overlay. The own rule ("the `Params` marker goes when the stage carries a descriptor row")
was written from stages whose descriptor text changed; the stage here has a descriptor row of unchanged text, and native
keeps the marker, as it does for body rows alone (S5C). This is the exclusive apply's, not the dynamic path's, and it is
handed to the apply track (`dropin-dynamic.md` section 5).

## 6. What the platform's `force` does with a structural stage (from checkpoint 1, for the record)

The stage of this program's own `config import` for an added attribute (9 520 rows): without `--force` it restructures,
asks `[y/n]`, and with a closed stdin exits **102** after 219 s having written nothing; with `--force` it publishes
**every staged row as an alias** (`Config` 9 841 -> 19 360 rows) in 208 s. This apply refuses such a stage by its size
(#395) and, whatever its size, by the rules of `dropin-dynamic.md` section 2.

## 7. Repeating it

```
restore-clone.ps1 -Corpus bsp8327 -Name ibcmd_rs_05_ui_dy_p ...
ibcmd-rs mssql-stage-source-objects --database ibcmd_rs_05_ui_dy_p --source-root <tree> --path-prefix CommonModules/<module> --replace-config-save --allow-non-lab --per-row
UPDATE dbo._ConfigChngR SET _MessageNo = 5 WHERE _MDObjID = <the module's object id, little-endian>   -- the seed
BACKUP DATABASE ... COPY_ONLY; restore-clone.ps1 -Corpus bak -Bak ... -Name ibcmd_rs_05_ui_dy_q ...
nat-apply.ps1 -Database ..._p -Tag <t> -ApplyArgs '--dynamic=force'        (under the native lock)
ours-apply.ps1 -Database ..._q -Tag <t> -ApplyArgs '--dynamic=force'
dyn_state.py <db> <json>; dyn_state.py --diff <a.json> <b.json>; content_sums.py <db> <db>
```

## 8. Lab resources

All eight databases (`ibcmd_rs_05_ui_dy_p`, `_q`, `_p2`, `_q2`, `_q3`, `_q4`, `_p3`, `_p5`) were dropped with
`drop-lab-dbs.ps1` once measured; the export trees, the backups and the run folders were deleted. No process, lock or
cluster registration of this run is left.

## 7. The import route (#395, feat/0.4 1c63b763 merged): our import, then `--dynamic=force`

Since #395 `infobase config import` stages only the rows that change (`docs/import/delta.md`). A one-comment change of the
module (`ibcmd-online-imp*`), exported natively from the clone, edited, imported by the merged build:

| Base | Rows staged | Ours `--dynamic=force` | Native `--dynamic=force` on a byte-identical twin |
|---|---|---|---|
| clean (no pending online update; `dy_j`, `dy_j2`) | **4**: the module's `.0` body, `root`, `version`, `versions` | done, 2.5 s, generation `5b5d4587...` | same generation; **`dyn_state` 0 differences, `content_sums` equal** in `Config`, `Params` (all but `.ui`), `Files`, the register and its file lists; native 79 s |
| with the БСП clone's pending update (`dy_i`, `dy_i2`) | **7**: the same four, the effective `.0` bodies of the two objects the update changed, and a `deleted` list naming the update's six rows | refused (see below) | done, 46 s; generation `b7c76e99...` |

The clean base is the route the acceptance asked for and it is equal to the platform's. The base with a pending update is
not, and the measurement shows why: the `deleted` list tells the platform to promote the update, and its `force` does more
than this apply does -- `Config` 9 852 rows against ours 9 851 (a `deleted_dynupdate_<g>` row of 151 bytes), six more rows in
the register's file lists (the old aliases `<uuid>_dynupdate_06cb0442-....0` appended to the lists of the two objects, as the
exclusive apply appends the bodies a `deleted` list names), and the service information collected into `Params` (16 `.si`
alias rows, `Params` 4.2 -> 8.3 MB, «Сбор служебной информации...» in its output). This apply therefore **refuses** such a
stage (`deleted: the stage lists 6 row(s) of the online update ... apply it exclusively`), and consumes only an empty list
(unmeasured for the platform's `force`; the clean import stage carries none). What a native-faithful publication of it
needs: the engine can publish `deleted` as `deleted_dynupdate_<g>` (its alias naming does), the register part is
`registrations::plan(.., dropped)` of the exclusive apply, the `Params` part is the `.si` collection that the exclusive
apply also does not do (`own-apply.md`, `not_written`). The import-then-force loop works until the first `force` has left
an update; from then on every import stage names it. **Open (next step in `dropin-dynamic.md` section 4).**
