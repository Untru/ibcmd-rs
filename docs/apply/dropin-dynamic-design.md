# `--dynamic` in the drop-in `infobase config apply` (issue #347): design, checkpoint 1

Design only, no code. What the platform does with each value (measured on 8.3.27.2214 with a real client connected),
what this program has and lacks to serve it, the mapping proposed, the limits that apply, the words and the exit codes.
The decisions that are the coordinator's are listed at the end.

> **Decided and built (checkpoint 2):** `auto` with sessions keeps the refusal (a dynamic update only for an explicit
> `force`, plus the hint line when the stage would qualify); the delta stage comes from the source-driven route now and from
> the import once #395 lands; more kinds are #345's; the report says `mode: "dynamic"`, the alias rows and the history; no cap
> on generations, a warning past 50. What was built and how it was proven: [`dropin-dynamic.md`](dropin-dynamic.md).
> The text below is the design as it was put to the decision, unchanged.

Today the drop-in serves the exclusive apply only (`dropin-apply.md`): `--dynamic=auto|disable|prompt` are that apply,
`--dynamic=force` is refused by name at parsing (`Параметр --dynamic=force ... не поддерживается`, exit 1), and the apply is
**stricter than the platform** on purpose: it refuses while SQL Server shows another user process on the database.

## 1. What the platform does (measured)

Direct mode (`--dbms=MSSQLServer --db-server ... --db-name ...`, the way the drop-in and every script of the lab run), on a
БСП 8.3.27 clone that had **a `1cv8c` thin client connected** (`rac session list`: one session, application `1CV8C`, user
Администратор; SQL Server showed the rphost's `1CV83 Server` sessions on the database), stage of 5 rows from our per-row stage
(a module body, `root`, `version`, `versions`), stdin from an empty file, native lock. Files: `F:\ibcmd\lab\05\ui\runs\dy\<tag>.out|err|meta`.

| Run (tag) | Exit | Time | What it printed | What it left |
|---|---|---|---|---|
| `--dynamic=auto` (auto1) | 0 | 5.4 s | the eight INFO lines of a plain apply, `Создано поколение конфигурации: <hex>` | rows replaced **in place**; no alias, no marker; `ConfigSave` empty; the client session still listed |
| `--dynamic=prompt` (prompt1) | 0 | 6.6 s | the same; **no question** although stdin is closed | the same |
| `--dynamic=disable --session-terminate=force --session-terminate-message=...` (term1) | 0 | 11.6 s | the same | in place; **the client session was not ended** (still 1 session after) |
| `--dynamic=force` (force1) | 0 | 8.3 s | the same eight lines | the dynamic overlay: 3 alias rows (descriptor, body, `versions`) and the two `DynamicallyUpdated` markers; the session still listed |

So in direct mode **`auto` is the exclusive apply, whatever sessions exist**: the platform run against a database sees no
session (already n20 of `evidence/dropin-apply/native-cases.md`), never asks, and takes the dynamic path only when told to.
Server mode (`--pid`/`--remote`, the only one that sees sessions) is the earlier evidence: the default prints
`Ошибка исключительной блокировки информационной базы.` / `Активные сеансы и соединения:` and a three-way prompt (1 Отмена,
2 Повторить, **3 Обновить динамически**); `--dynamic=disable` with stdin closed is `[WARN] ... отменено`, exit 0 (n23-n27).
The third choice of that prompt is the platform's own "auto -> dynamic when the lock cannot be taken".

### What `--dynamic=force` writes, against our online activation

Twin clones, one identical `ConfigSave` (5 rows), native `force` on one, `mssql-activate-staged-main --mode online` on the other,
row counts and checksums of every service table before and after (`runs\dy\snap_*.json`):

| | native `force` (12 s, 22 s) | our online activation (2.8 s) |
|---|---|---|
| alias rows | 3: descriptor 165 B, body 704 B, `versions_dynupdate_<g>` 344 213 B | the same three, the same sizes |
| markers | `Config` 45 B, `Params` 82 B | the same sizes |
| `Config` after | 9 845 rows, 136 849 756 bytes | 9 845 rows, 136 849 756 bytes |
| `Files.MobileVersions.dat` | **rewritten** (same size, a new head GUID) | not written |
| `_ConfigChngR._MessageNo` of the changed owner | **set to NULL** (seeded 5, three rows: 5 -> NULL) | left at 5 |
| other tables | unchanged | unchanged |

The shape of the generation is the same; **two writes are missing** in our online path for the drop-in: the mobile versions ring
and the change registration of the exchange plans (the own exclusive apply already writes both: `own-apply.md`,
"Which per-apply writes are required"; #412 for the imaged nodes). Its locking differs as well: the online activation takes
row locks in a short serializable transaction (190.8 ms in the #344 measurement), the own exclusive apply takes `TABLOCKX` on `Config`,
`ConfigSave`, `Params` and `Files` for the whole transaction. A dynamic apply that must not disturb connected sessions has to be the
first kind.

### What `--dynamic=force` does with a stage that is not small or not dynamic

The same command on the stage our `infobase config import` leaves for an added attribute (9 520 rows, a restructuring;
`runs\dy\force_struct*`):

* without `--force`: it **restructures anyway** (`Обработка структуры базы данных...`, `Реструктуризация Справочник._ДемоПартнеры`, the
  20 000-row renumbering of the change-registration table), prints `Изменена структура таблиц базы данных` and asks
  `Принять изменения и продолжить обновление [y/n] :`. With stdin closed: stderr `[WARN] Обновление конфигурации базы данных
  отменено`, **exit 102**, 219 s, nothing written (rolled back: `SchemaStorage` 100, no marker);
* with `--force`: it applies, 208 s, exit 0, and writes **every staged row as an alias**: `Config` 9 841 -> 19 360 rows, `Params`
  38 -> 55, the restructuring done.

So `force` is not "dynamic when possible, else refuse": it publishes whatever `ConfigSave` holds as an overlay, structure change and
all. What makes a native dynamic update small is the platform's own partial stage (a handful of rows).

## 2. What this program has, and what a dynamic apply needs

* **The online transition** (`mssql_main_activation`, mode `online`): aliases + markers, `root`/`version` in place, the short row-level
  transaction; measured against sessions (#344, `online-activation.md`): existing sessions keep their generation, new ones get the new
  one, no session disturbed. Limits: 128 staged rows, 32 MiB a plan, 16 MiB a row, 16 MiB inflated `versions`, `PartNo` 0, only
  **existing** `<uuid>` / `<uuid>.<n>` rows (a new object is refused), profile `platform-8.3.27.2214` (`mssql.main.write`;
  8.5.1.1150 declares it `unsupported`, 8.3.27.1989 not at all). It takes the exact `ConfigSave` set as its plan: the whole stage.
* **The own exclusive apply** (`mssql_config_apply`): plans a stage of any size (fingerprints computed on the server), gates it
  (`ApplyCheckGate`, S1), folds an earlier overlay, writes the exchange-plan registrations and `MobileVersions.dat`, report and
  recovery artifact, the platform's words and exit codes. No RAS, no cluster options.
* **The source-driven online apply** (`mssql-apply-source-change --mode online`): one module or common-form body per call, RAS
  verification of the infobase, the export of the active state and a classification; not reachable from a `ConfigSave` that the
  import staged.
* **The gap that decides the design (#395, open):** our `infobase config import` writes the **whole tree** into `ConfigSave`. For a
  one-comment change of a module: 9 521 staged rows, of which 357 have the bytes of the active rows, 6 098 the same content under
  another deflate, 3 065 different content (the recompiled bodies), 1 new (`runs\dy\import_z.*`, `tools\delta.py`); the apply's report
  says `replaced_rows 9520`. A dynamic apply of that stage would alias 9 500 rows, as the platform's `force` does on the same
  stage, and could not stay inside 128 rows. **A delta stage is a precondition**: either #395 (the import stages only what
  differs, as the platform's partial import does with 25 files -> 45 rows), or a stage made by the source-driven route.

## 3. Mapping proposed

| Value | No other session | Sessions connected (SQL Server shows them) |
|---|---|---|
| `disable` | exclusive apply (as today) | refuse, as today: `Ошибка исключительной блокировки...` and the list, exit -1 |
| `auto` (default) | exclusive apply (as today; = the platform in direct mode) | **dynamic** when the stage qualifies (section 4), else the refusal above. *Alternative:* keep the refusal (`auto` = `disable`) and serve `force` only |
| `prompt` | exclusive apply (the platform never asks in direct mode) | as `disable` (no dialog: stdin is not a terminal for scripts); a terminal prompt is not built |
| `force` | **dynamic** when the stage qualifies, else refuse | dynamic when the stage qualifies, else refuse |

`auto` with sessions is the one choice with a behaviour change: today's refusal becomes an apply. It is the platform's own answer
in server mode (the third choice of its prompt), it is safer for the sessions than what the platform does in direct mode (an in-place
promotion under live sessions), and the exclusive apply folds the overlay it leaves at its next run (`own-apply.md`,
"Dynamic-update rows go even when the stage omits them") so the overlay does not grow without end. The price: a script that used to
be refused gets a generation and an overlay; `--dynamic=disable` keeps the old behaviour.

`--session-terminate` does not change: it concerns the exclusive lock. `force`/`prompt` with sessions connected and the exclusive
path chosen stay «не поддерживается» (this version ends no session); with the dynamic path they are not consulted (the platform's
`force` ends none either).

## 4. Which limits apply to the dynamic path

The stage is judged by its **delta** (the rows that differ, not the bytes of the whole `ConfigSave`; after #395 the two are the same):

1. **What may differ:** bodies (`<uuid>.<n>`) of **common modules and common forms** that already exist, plus `root`, `version`,
   `versions` — the objects the #344 evidence and the source-driven apply are measured on. Descriptors (unless layout-only), new
   or removed objects, object modules and forms of top-level objects, templates, pictures, help pages, rights and command
   interfaces: refused with `требуется штатный config apply: <the reasons>` until each has a session measurement. The structural gate
   (`ApplyCheckGate`, then S1) must say `restructuring_required: false`; a restructuring is never dynamic here (the platform's
   `force` restructures; ours refuses).
2. **Size:** at most 128 rows (125 without the three service rows) and 32 MiB, 16 MiB a row (`mssql_main_activation` limits). More:
   refused, the reason names the count and #395.
3. **Platform:** `platform-8.3.27.2214` only; a new capability `mssql.config.apply.dynamic` declared for it alone. 8.5.1.1150
   (`mssql.main.write: unsupported`) and 8.3.27.1989: refused by name.
4. **Verification without RAS:** the storage-profile check the exclusive apply makes (SQL only). The drop-in has no cluster
   options and the generation is visible to sessions without a cluster call (#344); an optional RAS binding can come later.
5. **Locks:** row locks in a short serializable transaction, never a table lock (sessions stay undisturbed).
6. **Parity writes:** `Files.MobileVersions.dat` and the `_ConfigChngR` registration of the changed owners (with the #412 insert for
   imaged nodes), taken from the exclusive apply, so that a dynamic generation leaves what the platform's `force` leaves.
7. **Exclusive access is not required** and not asked for; the application lock and the exact-stage assertions stay.

## 5. Exit codes and words

| Case | Words | Exit |
|---|---|---|
| dynamic apply done | the platform's lines of an apply, `Создано поколение конфигурации: <hex>` (the same line for `force`) | 0 |
| nothing staged | `Обновление конфигурации базы данных не требуется` | 0 |
| stage does not qualify (`force`, or `auto` with sessions) | `требуется штатный config apply: <the reasons>`; for size, the count and #395 | 1 |
| sessions and the stage does not qualify, `--dynamic=auto|disable|prompt` | the lock refusal and the list, as today | -1 |
| `--session-terminate=force|prompt` with sessions and the exclusive path | «не поддерживается в этой версии ibcmd-rs», the list, as today | 1 |
| wrong value | `Некорректное значение параметра: dynamic` | 2 |
| `--extension`, `--pid`, `--remote`, `--sqlcmd` | «не поддерживается» by name, as today | 1 |
| platform without the capability (8.5, 8.3.27.1989) | «не поддерживается» by name | 1 |
| failure | the error and its context | -1 |

The platform's exit **102** (the confirmation `[y/n]` of a structural change answered by a closed stdin) has no counterpart: ours
never asks.

## 6. Decisions for the coordinator

1. `auto` with sessions: dynamic (proposed) or keep the refusal and serve `force` only.
2. The delta stage: wait for #395, or let the dynamic path judge a full stage by content (the gate's inflated comparison is not
   enough: 3 065 of 9 521 rows of a comment change differ in content; a semantic delta needs the tree).
3. Kinds admitted beyond common modules/forms (object modules, forms of top-level objects, templates): measure before admitting.
4. The `--report` JSON: add `mode: "dynamic"`, the alias rows and the history (proposed), the report of the exclusive apply otherwise.
5. Overlay growth: no limit on generations is proposed; the exclusive apply folds them. A warning when the history passes some
   number (say 50) is cheap.

## 7. Build plan after the decision

Parsing (`force` accepted) -> the delta and its gate -> a dynamic publication in the online engine fed with a delta and the parity
writes -> the report and the words -> tests (unit: mapping, limits, words, exit codes; twin against the platform's `force` as above)
-> lab proof on a clone with a live client (a session before, during and after; `auto` with sessions, `force`, the refusals).
