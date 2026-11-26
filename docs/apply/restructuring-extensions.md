# The own restructure and the extensions (S1-I, #405)

Part of #391 (the minimal own restructuring, S1), sub-issue S1-I, track "ext". Everything below is **measured**
on the БСП 8.3.27 corpus clone (four extensions, platform 8.3.27.2214, SQL Server 2025) unless it says
**inference**. Code: `src/restructure/extensions.rs`, `mssql_dump::extension::adopted_objects`; the hooks in
`plan.rs`, `reader.rs`, `s1.rs`, `script.rs`, `exec.rs`, `command.rs`. Evidence:
`docs/apply/evidence/restructuring/s1i-*.txt`. The design of S1 itself is `docs/apply/restructuring.md`,
section 12; the route is `mssql-config-apply --allow-restructure s1` (12.12).

## 1. Summary

1. **An extension keeps its own storage beside the main one.** `SchemaStorage(1)` holds the schema of the
   extensions (243 table entries on the clone, physical tables with the suffix `X1`), the `Params` rows
   `DBNames-Ext-<registry id>` number their objects, and `_ExtensionsRestruct` / `_ExtensionsRestructNGS` keep
   bookkeeping. The counter of numbers is shared with the main `DBNames` (the allocation rule of the prototype,
   "one more than the largest header of all of them", was right: `Fld11034` after the headers 10824 / 10989 /
   7841 / 11033).
2. **An adopted object is paired with its base by identity, not by uuid** (kind and name): 47 of the 75 adopted
   objects of the four extensions carry the nil uuid where the header would name the base object, and the
   object has a uuid of its own (`Catalog._ДемоПартнеры` is `3014d9c1-...` in `_ДемоРасширение` and
   `5eab8a1b-...` in the configuration). A check by uuid finds none of them.
3. **What a change of an adopted object does natively depends on whether the extension extends it with data**
   (section 3):
   - an object **no extension adopts**: the platform rebuilds the main tables only; every table and row of the
     extensions is as it was (case n);
   - an object **adopted with no table of the extension's own**: the same (case a; also the earlier a2);
   - an object **adopted and extended with data** (the extension keeps `_Reference18X1` for it): the platform
     rebuilds the main table **and** the extension's table, adds the new field to the extension's schema entry
     and rewrites `SchemaStorage(1)` (case c). A rebuild of the main table alone leaves the extension out of step.
4. **What S1-I does**: the S1 gate reads the objects the extensions adopt and the plan **refuses** a change of an
   object an extension adopts (whatever the case above: the acceptance criterion is a refusal, section 4); it
   refuses when the extension schema is not idle or when the objects were not read; and the structure phase
   **proves** inside the apply's transaction that nothing the extensions keep changed, and rolls back otherwise
   (`THROW 57406`).
5. **On a twin of the БСП clone with four extensions** (section 5), through `mssql-config-apply
   --allow-restructure s1 --i-have-a-backup`: an object no extension adopts passes and the result equals the
   native apply (tables, data, `Config`, `DBNames`, `DBSchema`, 16 of 16 `.si` rows, native export 12 198 / 12 198,
   native apply afterwards "не требуется"), and the four extensions are as they were (243 `X1` tables,
   `SchemaStorage(1)`, `DBNames-Ext-*`, `_ExtensionsInfo`, `_ExtensionsRestruct`, and their native export
   737 / 737); an adopted object is refused, dry run, rehearsal and real run alike, and the database is exactly
   as it was; an injected failure in the script takes everything back.

## 2. The storage of the extensions

| what | measured |
|---|---|
| `SchemaStorage` | two rows, `SchemaID` 0 (main) and 1 (extensions), both `Status 100`, empty generations |
| the extension schema | 243 table entries; the tables `_<Name>X1`; a table an extension **owns** has a number from that extension's `DBNames-Ext-<id>`; a table of an **adopted object the extension extends** has the number of the main object (`Reference18` is `_ДемоНоменклатура`; counted by name over 221 of the entries: 142 owned, 78 of adopted objects, mostly registers and their change-registration tables, 1 of neither) |
| `Params` | `DBNames-Ext-<registry id>` and `DBNamesVersion-DBNames-Ext-<registry id>` only for the extensions that own tables (`_ДемоРасширение` header 7841 with 41 entries, `ServiceDesk` header 11033 with 753); `DBNames-Ext-1` is an empty list with the header 10989 |
| the attribute an extension adds to an adopted catalog | `Fld7840` of `_ДемоНоменклатура`: its number is in `DBNames-Ext-347eea02...`, its field sits in the entry `Reference18` of **`SchemaStorage(1)`** (`_Reference18X1`), never in the main entry |
| `_ExtensionsInfo` | four rows (the registry), unchanged by any apply |
| `_ExtensionsRestruct` | 57 rows, unchanged by a native apply |
| `_ExtensionsRestructNGS` | empty on the corpus clone; after the first native apply **three rows** (one per extension that owns tables: 13 890, 444 and 3 864 bytes, keyed by the registry id) which stay; the platform writes them again at every apply. They are derived state, not a marker of a restructure in flight (an earlier version of this check took them for one and would have refused every database that had ever been applied) |

The objects an extension adopts are read off its descriptor rows: the first header of a row named by the header's
own uuid, when its belonging is 1 (`mssql_dump::extension::adopted_objects`): the row, the own uuid, the name, and
the uuid of the base object when the header names one. Children (attributes, sections) sit in the same rows.

## 3. What native does when an adopted object changes

Three twins each, restored from one backup of the clone after a native baseline apply (an unchanged file
staged and applied: it folds the `_dynupdate_` rows a corpus clone carries), one string attribute added to a
catalog by a native `config import files --partial`, then the native `config apply --force --dynamic=disable`.
Full diffs: `s1i-native-adopted.txt`, `s1i-case-n-twins.txt`.

| case | object | adopted by | native rebuilt | extension state after |
|---|---|---|---|---|
| n | `Catalog._ДемоСтавкиНДС` (`Reference23`) | nobody | `_Reference23` | unchanged: `SchemaStorage(1)`, 243 `X1` tables, `DBNames-Ext-*`, `_ExtensionsInfo`, `_ExtensionsRestruct`, `_ExtensionsRestructNGS` |
| a | `Catalog._ДемоПартнеры` (`Reference20`) | `_ДемоРасширение`, no table of its own | `_Reference20` | unchanged (as case n; the earlier case a2 said the same) |
| c | `Catalog._ДемоНоменклатура` (`Reference18`) | `_ДемоРасширение`, which keeps `_Reference18X1` (46 rows) | `_Reference18` **and `_Reference18X1`** (`_Fld11034 nvarchar(50) NULL` in both) | **`SchemaStorage(1)` rewritten**: the entry `Reference18` of the extension schema gained `Fld11034` beside the extension's own `Fld7840`; the rows of `DBNames-Ext-*` unchanged |

So a change of an adopted object that the extension extends is **not** confined to the main tables, and the own
restructure has no model of the extension's entry. Whether an adopted object with no table of the extension's
own is harmless is what cases a and a2 say (for the addition of an attribute; the deletion of an attribute the
extension adopts, a change of the type, and every other operation are **not traced**).

## 4. What S1-I does

- **Refuse** (`extensions::check`, called by `plan` for every changed object): the object is adopted when an
  extension's adopted header has its uuid, names its uuid as the base object, or has its name (kind-agnostic:
  the kind is not read off an extension row, so a same-named object of another kind refuses too, which is the
  safe side). Staged images of the extensions count as well as the active ones. The message names the extension,
  the adopted object and image, and the tables of its own the extension keeps for the object (`_Reference18X1`,
  sub-tables included), because those are what a rebuild would leave out of step.
- **Refuse** when the infobase has extensions and the plan was given none of their objects, and when
  `SchemaStorage(1)` is not idle.
- **On the real route** the S1 gate (`S1Gate::check`) reads the state of the extensions with the rest of the
  plan's input (`reader::read_inputs`: `read_state`) and, when the infobase has extensions, the objects they
  adopt (`read_adoptions`, through the apply's own SQL client; a read that fails is a blocker
  `S1: the objects the extensions adopt could not be read`). The plan's error is the gate's blocker
  `S1: catalog X is adopted by the extension ...`; the apply answers with its `StructuralRefusal` (exit 1), and
  the drop-in `ibcmd infobase config apply` words it `требуется штатный config apply: <blockers>` as it does for
  every refusal of the gate.
- **Prove, inside the transaction.** The structure phase (`Plan::phase_sql`) computes a fingerprint of everything
  the extensions keep before its guards, and again after its publication, and compares them (`THROW 57406`,
  "the restructure changed what the extensions keep"). The fingerprint (T-SQL, `script::extension_state_sql`;
  the same in Rust, `extensions::fingerprint`, for the standalone executor): the `SchemaStorage` rows but the
  main one, the `Params` rows `DBNames*-Ext-*`, count and checksum of `_ExtensionsInfo`, `_ExtensionsRestruct`,
  `_ExtensionsRestructNGS` (when they exist), and the row count of every table whose name ends in `X1`. It runs
  inside the apply's SERIALIZABLE transaction with the rest of the phase, so the apply's `CATCH` takes back
  everything.
- **The criterion is a refusal for every adopted object**, as the issue states; the measured harmlessness of
  cases n and a is recorded, not used (the coordinator decided: not in 0.4).

## 5. The twin protocol (12.6) through `mssql-config-apply --allow-restructure s1 --i-have-a-backup`

The clone after a native baseline apply (an unchanged file staged and applied); one string attribute added to a
catalog by a native `config import files --partial`; twins from one backup of the staged state. Case n:
`n3_nat` (native `config apply`) and `n3_own` (dry run, rehearsal and real run of `mssql-config-apply`); case c:
`c3_own` (refused).

| # | check | case n (`_ДемоСтавкиНДС`, adopted by nobody) | case c (`_ДемоНоменклатура`, adopted, `_Reference18X1`) |
|---|---|---|---|
| 1 | the plan made offline equals the native result | `DBNames` inflated equal (347 223 bytes both); the `DBSchema` entry `Reference23` equal | the plan is not made |
| 2 | tables, columns, indexes | identical but `_DbCopies*` (build drift) and the auto-named primary key of `_ConfigChngR` | unchanged: 0 tables changed |
| 3 | the data of every rebuilt table, EXCEPT both ways | `_Reference23`, `Config` and the 243 `X1` tables, `_ExtensionsInfo`, `_ExtensionsRestruct`, `_ExtensionsRestructNGS`: 248 tables, 0 differing | - |
| 4 | `Config` rows | equal (9 841 rows, EXCEPT both ways 0) | unchanged |
| 5 | `DBSchema` entries, `DBNames` | 1 761 entries, equal but `DbCopies`, `DbCopiesUpdates` | unchanged |
| 6 | the 16 `.si` rows | **16 of 16 have the same text** (the registry row `1a621f0f` and the XDTO row included; the XDTO property of the non-nullable attribute has no `lowerBound`, as native's) | unchanged |
| 7 | a native apply afterwards | "Обновление конфигурации базы данных не требуется" | - |
| 8 | native export | main configuration 12 198 / 12 198 identical; the four extensions 737 / 737 identical to the reference exports | - |
| 9 | a session in the cluster | identical output on both twins in the earlier run of this case (wave 0), not repeated | - |
| 10 | a rehearsal changes nothing | `--rehearse` runs the script and rolls it back; the real run followed on the same database | dry run, rehearsal and real run: nothing written (0 tables, all service rows equal) |
| 11 | refusals | - | dry run, rehearsal and real run refuse with `S1: catalog _ДемоНоменклатура is adopted by the extension _ДемоРасширение (...); the extension keeps tables of its own for it (_Reference18X1): ...`, exit 1 |
| 12 | a failure inside the transaction takes everything back | on a fresh twin, the generated script with (a) an `UPDATE dbo._ExtensionsInfo SET _ExtName = _ExtName` between the phase and its extension check: `THROW 57406` "the restructure changed what the extensions keep"; (b) a `THROW` as the last statement before `COMMIT`: 0 tables changed, no `*NG` table, no new column, `SchemaStorage` and every service row as they were, `ConfigSave` 4 rows | - |

The drop-in `ibcmd infobase config apply` at this base does not choose the S1 gate yet (the restructure-check
track is wiring it: a backup option is the consent). With that wiring made locally (not committed), the drop-in
refuses case c with `требуется штатный config apply: bb3d8c09-...: the descriptor's text differs ...; : S1:
catalog _ДемоНоменклатура is adopted by the extension _ДемоРасширение ...` (exit 1; the blocker of the plan has no
row, so the wording shows `; : S1:`), and applies case n (exit 0) with a result equal to the native twin's
(248 tables EXCEPT 0, 16 of 16 `.si` rows).

The earlier run of case n with the standalone `mssql-restructure` (wave 0) is in `s1i-case-n-twins.txt`.

## 6. Not covered

- other kinds than catalogs and documents (the check is by name and works for any kind the gate hands it:
  `ChangedObject::kind`; the gate builds catalogs and documents only), the deletion of an attribute or a widening
  of an object an extension adopts (both are refused with the rest);
- the platform's behaviour for an object the extension adopts when the **extension** carries an attribute of the
  same name, or the extension's form uses the deleted attribute (extension consistency checks of the apply);
- 8.5 (the S1 gate refuses it); an extension whose staged image adopts an object its active image does not is
  read (the union), but no such case was traced;
- the drop-in `ibcmd infobase config apply` on the S1 gate: the wiring is the restructure-check track's; the
  refusal was checked with a local, uncommitted wiring (section 5);
- a cluster session on the result of the route (check 9): equal on the wave-0 twins, not repeated here.
