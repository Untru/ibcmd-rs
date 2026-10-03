# `--dynamic=force` in the drop-in `infobase config apply` (issue #347)

`ibcmd infobase config apply --dynamic=force` publishes the staged configuration as a **dynamic (online)
generation** while sessions stay connected: the changed rows go beside the active ones under
`<name>_dynupdate_<generation>`, the two `DynamicallyUpdated` markers name the generation, `root` and `version` are
replaced in place, and the writes the platform's own `force` makes besides -- the change registrations of the exchange
plans and `Files.MobileVersions.dat` -- are made in the same transaction. Sessions that are open keep the generation they
loaded; sessions that open afterwards read the new one. Settling collected pending generations requires the
**native exclusive apply**: this binary's direct exclusive path still refuses pending Params SI aliases.

The design and the platform measurements that decided it are in [`dropin-dynamic-design.md`](dropin-dynamic-design.md)
(checkpoint 1); this file is what was built and how it was proven (evidence:
[`evidence/dropin-dynamic/acceptance.md`](evidence/dropin-dynamic/acceptance.md)). The exclusive apply the drop-in served
until now is [`dropin-apply.md`](dropin-apply.md).

## Recovery on released 0.4 — 2026-10-01

The preserved feature diff was ported from `1c63b763..c7ab2131` onto released
0.4.0 (`2a55cb34`), using three-way application instead of restoring old files.
The 0.4 S1, delta-import, PaletteColors and synonym/cache fixes remain present;
only the common report initializer required conflict reconciliation.

The native twins below are historical September measurements, not fresh native
acceptance of this port. The current checkpoint validates bounded Rust gates and
regressions. Fresh native/session acceptance on the integrated 0.5 candidate is
still required before expanding or closing #347. That first-wave checkpoint did not
implement pending-overlay import -> force -> repeat or Params `.si` collection.
The measured bounded extension is described below; active-session acceptance remains open.
The profile change adds only `mssql.config.apply.dynamic`:
8.3.27.2214 supported; exact 8.5.1.1150 now admits only the initial existing
CommonModule cohort described below. Existing 0.4 capabilities stay as released.

## Exact 8.5.1.1150 — initial CommonModule generation

The [fresh native/own evidence](evidence/platform85/initial-common-module-2026-10-01.md)
admits exactly five staged rows: one existing CommonModule descriptor and `.0`
body, plus `root`, `version`, and `versions`. Descriptor and version must be
semantically unchanged; the bounded body must contain the measured `text`/`info`
container with unchanged info and genuinely changed UTF-8 text. The opaque root
may re-stamp only its measured final sixteen bytes. The configuration has no
pending history, markers, deletion list or aliases, and its three touched
registrations must retain the measured NULL-message/existing-file-list shape.
All row, history, registration and stage guards run before publication.

Other 8.5 builds, repeated generations, forms, extensions, structural changes,
SI collection and standalone ONLINE/LIVE/WORKER activation remain unsupported.
`--dynamic=force` remains explicit; `auto`, `prompt` and `disable` keep their
exclusive semantics. SQL-only commands verify storage against the declared
profile; the exact executable build remains the caller's claim. Native/session
acceptance used independently verified 8.5.1.1150 binaries in a fresh private
cluster. Old sessions retaining A and new sessions reading B do not prove
same-session refresh, generic worker ownership or warm LIVE readiness.

The semantic stage inventory is now bound to the exact image the transaction
asserts. A changed `deleted` list or descriptor between judgment and publication
refuses before any write. A transport failure around COMMIT does not prove
rollback: the error names the recovery artifact and token, requires inspection
of ConfigSave and generation markers before retry, and leaves the outcome
uncertain. A final verification failure after successful execution explicitly
states that the transaction committed. The recovery artifact is manual recovery
evidence, not an automatic resume command.

## Pending generations — second-wave bounded extension

The BSP 8.3.27.2214 storage survey and fresh own/native proof are recorded in
[`wave2-pending-generations.md`](evidence/dropin-dynamic/wave2-pending-generations.md).
An imported pending overlay is accepted only when `deleted` exactly inventories
the existing Config aliases and marker, both ordered generation histories agree,
and the Params service collection has the measured sixteen-class shape. The
initial legacy generation has neither collected SI aliases nor a deleted alias;
every later generation must have a complete collection. Missing whole later
collections, partial collections, reordered history, unknown classes and actual
object removal are refused before publication.

An unchanged CommonForm `.1` HELP companion may be carried with `.0`; its inflated
text must equal the effective active row. This does not enable changed help or
other extra parts. Publication preserves every previous alias, adds the new
deleted alias and sixteen raw SI copies, and refreshes raw BOM `siVersions` tokens.
It writes no licensing `.ui` rows. The service inventory, node eligibility and
registration/file-list preimages are checked under transaction locks before even
no-op consumption. An ordinary-row no-op that would discard an effective pending
body revert is refused, including manually prepared stages with absent/empty `deleted`.
Staged rows require measured `Attributes=0`; the dynamic transaction also binds
their Creation/Modified headers, sizes and digests before publication or cleanup.

Stopped-database recovery saves the replaced `siVersions` and exact previously
absent registration-ID/key/filename triples. Those additions may name an **old**
generation's files; deleting only new-generation aliases is insufficient recovery.
Inspect an uncertain COMMIT outcome before using the manual recipe or retrying.
The native comparison covers the measured eligible-node append shape with NULL
message numbers; it does not add an active-session, licensing or arbitrary-node acceptance claim.
The native rebuilds three help/search indexes in Files; this path preserves their
preimage and now reports that known difference in `not_written`. Full storage and
help-search equivalence are not claimed. After these generations, use the platform's
exclusive apply to settle the overlay. Our exclusive guard remains strict; this
checkpoint does not enable its fold of the newly collected Params SI shape.

## 1. What each value of `--dynamic` does

### SQL-guard interactive fallback (new source checkpoint; runtime acceptance pending)

The preserved direct-mode native cases `auto1`, `prompt1` and `term1`
([design](dropin-dynamic-design.md), section 2) remain the basis for noninteractive
behavior: auto/prompt use exclusive apply and the native direct command did not
terminate the connected client. The standalone-server dialog is a distinct mode.

With `--dynamic=prompt`, terminal stdin and stderr, and a typed **pre-write**
SQL-session refusal for this database, this implementation now offers a SQL-guard
interaction when the current stage passes the existing read-only dynamic judgment:
`1` cancels (exit 0, report `cancelled=true, applied=false`), `2` performs one fresh
exclusive apply, and `3` explicitly calls the existing dynamic apply with complete
fresh profile/metadata/stage admission and locked CAS. A second refusal is terminal.
No transaction error, unknown transport outcome, blind census, foreign target,
`--exclusivity=assumed`, or requested session termination can open the interaction.
EOF, invalid/oversized input and input errors dispatch no second apply; closed or
redirected stdin preserves the previous refusal. `--force` confirms warnings and
does not provide dynamic consent. Auto retains its refusal and hint.

This is a user-visible SQL-guard route, **not** native `--pid`/`--remote` prompt
equivalence or authority to kill SQL/RAC sessions. No native runtime is claimed for
this source checkpoint. Issue #347 remains open for current-producer scenarios,
standalone-server integration and exact owned-session termination evidence.

| Value | Nobody else connected | Sessions connected (SQL Server shows them) |
|---|---|---|
| `disable` | exclusive apply (as before) | the lock refusal and the list of sessions, exit -1 (as before) |
| `auto` (the default) | exclusive apply (= the platform run against a database) | the same refusal, exit -1, **plus the line `можно применить динамически: --dynamic=force` when the stage would qualify** |
| `prompt` | exclusive apply (the platform never asks in direct mode) | noninteractive: as `auto`; terminal: explicit SQL-guard fallback above |
| `force` | the dynamic apply if the stage qualifies, else `требуется штатный config apply: <reasons>` (exit 1) | the same: sessions are what it is for |

`auto`, `disable` and noninteractive `prompt` are **never turned into a dynamic update**. Interactive prompt requires
the explicit `3` choice; a user who did not ask for a dynamic update gets the exclusive route. `force` is **never turned into an exclusive
apply** either. `--session-terminate` concerns the exclusive lock only; with `force` it is not consulted (the platform's
`force` ends no session either). `disable` is not pointed at `force` (it said no to dynamic updates).

Words and exit codes:

| Case | Words | Exit |
|---|---|---|
| done | `Обновление конфигурации базы данных...`, `Создано поколение конфигурации: <hex>`, `... успешно завершено` (the same lines as the platform's `force`, the same generation) | 0 |
| nothing staged | `Обновление конфигурации базы данных не требуется` | 0 |
| the stage does not qualify | `требуется штатный config apply: <row>: <reason>; ...` (the first 8, then `и ещё N`) | 1 |
| the profile has no admitted dynamic apply (8.3.27.1989 declares none) | ``Параметр `--dynamic=force` команды `infobase config apply` не поддерживается для платформы ...`` | 1 |
| `auto`/`prompt`/`disable` with sessions | `Ошибка исключительной блокировки информационной базы.`, the sessions, `Закройте их ... и повторите` and, for `auto`/`prompt` when the stage would qualify, `можно применить динамически: --dynamic=force` | -1 (1 with `--session-terminate=force|prompt`, as before) |
| more than 50 generations after the apply | `[WARN] В информационной базе накоплено динамических поколений конфигурации: N; ...` on stderr, and `warnings` in the report | 0 |
| failure | the error and its context | -1 |

The platform's exit 102 (a confirmation `[y/n]` of a structural change answered by a closed stdin) has no counterpart: this
apply never asks and never restructures.

## 2. What qualifies

The platform's `force` publishes whatever `ConfigSave` holds as an overlay, a restructuring included
(`evidence/dropin-dynamic/acceptance.md`, section 6). This apply publishes a **small delta stage** and refuses the rest,
with the reasons named:

1. **Size:** at most 128 rows, 32 MiB, 16 MiB a row, one part each (the online engine's limits, `mssql_main_activation`).
   The stage of this program's own `infobase config import` is a delta since #395 (4 rows for a one-line change of a module
   on a clean base, 7 on a base with a pending online update), as is a stage made by the source-driven route
   (`mssql-stage-source-objects --per-row`) or by the platform's partial import. A whole-tree stage (9 521 rows) is refused
   by its size.
   The `deleted` row of a stage: an empty list is consumed; a list that names the rows of the online update the database
   carries is accepted only for the exact measured pending inventory above. Unknown or structural
   removals remain refused; the September refusal in `acceptance.md`, section 7 is historical.
2. **A generation is made of** `root`, `version` and `versions`; `root` and `version` must have the text of the active rows.
3. **Every other row** is the descriptor or the `.0` body of a **common module or common form** that already exists (the
   kinds a session was measured with, `online-activation.md` section 4; the kind is read from the configuration row the
   `root` row names). Only unchanged-text CommonForm `.1` HELP may accompany the body. A row of another kind,
   a new object, a changed `.1` body, a structural `deleted` list, a name that is not an object's:
   refused. More kinds are #345's, after they are measured with sessions.
4. **A descriptor is published only unchanged in text** (compared with the row the configuration is read from now: an
   alias of an earlier generation counts). A change of an object's properties is not dynamic here.
5. **The restructure check** (`ApplyCheckGate`) finds no restructuring. It is asked last, only for a stage nothing else
   refused; with 1-4 it cannot say yes to a stage that changes a table.
6. **The database is settled:** no unfinished operation (`commit`, `dynamicCommit`, ...), the schema storage at rest, no
   unmeasured overlay in `Params`. The bounded measured sixteen-class collection above is accepted;
   other service shapes require `config repair` / native apply first.
7. **The platform** declares `mssql.config.apply.dynamic`: `platform-8.3.27.2214` only (the build the online transition and
   the twin were measured on). The storage profile is verified against the database as for the exclusive apply (SQL only,
   no RAS; the drop-in has no cluster options).

Nothing of this needs exclusive access, and none of it is asked for.

## 3. How it is built

`src/mssql_config_apply/dynamic.rs` plans and runs; it writes no row itself.

* **The transition** is the online engine's (`mssql_main_activation`, mode `online`, unchanged): an application lock, the
  exact-stage and exact-active assertions, the alias rows and markers, `root` and `version` replaced in place, the
  postconditions, then `ConfigSave` emptied. **Row and key-range locks** in a short serializable transaction (0.5-0.6 s of
  SQL; the exclusive apply takes `TABLOCKX` on `Config`, `ConfigSave`, `Params` and `Files` for its whole run). Measured with
  `sys.dm_tran_locks` sampled every few milliseconds and with the lock counters of the indexes
  (`evidence/dropin-dynamic/acceptance.md`, section 4a): no lock escalation was even attempted on any of the seven tables it
  touches; what it holds is key locks on the rows it writes, key-range locks on `ConfigSave` and on the register while it
  reads them (about 12 000 on `_ConfigChngR` for the length of the transaction, which only configuration operations
  touch), and a shared table lock on the one-row `_YearOffset`. `SET LOCK_TIMEOUT 30000` is set, so a session that holds a
  row for good makes the apply fail instead of wait.
* **The writes the platform's `force` makes besides** are the exclusive apply's own SQL, not a copy:
  `sqlgen::render_parity_writes` (the change registrations: `_MessageNo` reset for every object that owns a staged row and
  the #412 rows for the nodes with an initial image, and the `Files.MobileVersions.dat` ring) is rendered into the
  exclusive script and handed to the online engine (`MainActivationPlan::with_parity_sql`) to run before the stage is
  consumed; the planning is `plan_mobile_versions`, `registrations::plan`, `node_literals` of `mssql_config_apply`.
  `sqlgen::timestamp_declarations` gives both the same `@now` (local time shifted by the year offset), which the plan also
  stamps the markers with (`with_platform_timestamps`; the source-driven online apply keeps UTC).
* **The checks** share `require_no_unfinished_operation`, the schema-settled helper and `blank_report`.
  The exclusive path retains `require_settled_storage`'s strict Params refusal; dynamic adds its measured inventory checks.
* **The recovery artifact** is the exclusive apply's (`recovery::write_recovery`): the rows of `Config` that carry the staged
  names, the markers, `MobileVersions.dat` before, the `_MessageNo` values it resets, the registrations it adds; for a
  dynamic apply the alias rows of earlier generations are not copied (a dynamic apply folds nothing), and the README says how
  to take the generation back.
* **The report** (`--report`) says `"mode": "dynamic"`, `"exclusivity": "not_required"` and `published`: the generation, the
  previous one, the **history** after it, the **alias rows** written and the rows replaced in place.
* **Seam:** `dropin/apply.rs`, `call_apply_dynamic` (the second seam) and `call_would_qualify` (the look that decides
  whether the refusal for the sessions carries the hint).

## 4. What is not done

* **Unmeasured pending inventories** are refused. The second-wave exact deleted/history/SI shape above now supports
  consecutive import -> force operations; arbitrary service collection, structural removal and licensing remain open.
* **Help/search index rebuilding** (`Files.userDocs_ru*`, `userPostings_ru*`, `userVocabulary_ru*`) is not implemented.
  The native-derived caches differ from the preserved own preimage; full storage/help-search equivalence is not claimed.
* **More kinds** (object modules, forms of top-level objects, templates, pictures, help, rights, command interfaces): #345.
* **Native standalone-server prompts** and **ending sessions** (`--session-terminate`): not built.
  The new SQL-guard terminal fallback is a separate bounded route, described above.
* **The cluster is not told.** Like the platform run against a database, this apply writes to SQL Server only; a session that
  is open keeps its generation (measured for both), a new one reads the new generation.
* **8.5 outside the exact initial CommonModule cohort above**, and **8.3.27.1989**.
* **A stage of the platform's own import** (removals, structure): `требуется штатный config apply`.
* **The overlay grows** with every generation. Stage and service inventories retain strict 128-row/32-MiB bounds;
  this checkpoint does not promise an unlimited number of pending generations. The historical growth warning remains.
  Native exclusive apply is the settlement route; this binary's exclusive guard still refuses collected Params aliases.

## 5. Findings for the apply track

1. **The `Params` marker after a fold.** For the same start (a database with dynamic generations) and the same 5-row stage
   -- a common module's body changed, its descriptor present and unchanged in text -- the native exclusive apply **leaves
   `Params.DynamicallyUpdated`** and clears the `Config` one, while the own exclusive apply deletes both, because its rule is
   "the `Params` marker goes when the stage carries a descriptor row" (`own-apply.md`, S5C). The measurement contradicts
   the rule as written: the marker goes with a descriptor whose **text changed** (S1-S5: new forms and templates), and stays
   for body rows alone (S5C) and for an unchanged descriptor (this one). Everything else is equal after the fold, and so is
   the native export. Not changed here; the proposed rule is "a descriptor whose inflated text differs from the active
   one".
2. **The platform's `force` registers the nodes with an initial image** (#412) as its exclusive apply does: with all the
   register rows of one node removed, the dynamic update inserted the missing row and its file list, as the own apply does.
