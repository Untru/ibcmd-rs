# An S1 change next to a harmless change elsewhere: the drop-in apply on a БСП 8.3.27 twin (rcheck, gate reasons)

The stage of `s1-acceptance.md` case `b1` (11 attributes deleted, two added, six objects) **plus the synonym changed in two other
objects** (`Catalog.Валюты`, `Document._ДемоЗаказПокупателя`). By construction, before the change of this branch the stage was refused
by the S1 gate: `decide()` withdrew the conservative blockers of the objects the plan changes, of `root`, of the created rows and of the
listing, and no others; the conservative rule lists the two synonym descriptors as "a metadata change, possibly structural", and the
gate held them against the change it otherwise let through. Now `decide()` also withdraws the "descriptor's text differs" blocker of a
row that the restructuring check has read and that no reason of it names, and the drop-in applies the stage.

## The twin run (the protocol of `s1-acceptance.md`)

A БСП 8.3.27 clone staged with the platform's `import files --partial` (8 files, 12 `ConfigSave` rows), a `BACKUP ... COPY_ONLY` of that state restored
as the own twin, the platform's `config apply --dynamic=disable` on the native twin (554 s, exit 0, the machine busy), the drop-in
`infobase config apply --recovery-backup=<file>` on the own twin.

| | |
|---|---|
| without a backup option | exit 1, `Нужна резервная копия SQL Server ...`, the fingerprint of the database unchanged |
| drop-in apply with `--recovery-backup` | exit 0 in 53.7 s, gate `s1`, 8 tables of 6 objects rebuilt; the two synonym descriptors are not in the structure report (they are copied, not rebuilt) |
| 4 `Config` rows | 0 rows differ both ways (9 841 rows; `Creation` and `Modified` included) |
| 3 data of the rebuilt tables | `_Reference23` 2/2, `_Reference2565` 4 840/4 840, `_Reference2598` 716/716, `_Reference7444` 0/0, `_Reference3408` 0/0, `_Document1564` 1/1 and its two sub-tables 1/1, 1/1: `EXCEPT` both ways 0 rows |
| 7 the platform's `config apply` on our twin | «Обновление конфигурации базы данных не требуется» |
| 8 the platform's `config export` of both twins | 12 198 of 12 198 files identical (the synonyms of the two objects included) |
| 5 `DBSchema` entries and `DBNames` | `DBNames` equal; the entries equal but `DbCopies` and `DbCopiesUpdates` (the platform's own drift list of `restructuring.md` 12.6) |
| 6 the 16 `*.si` rows | 15 identical; `1a621f0f` (the object registry) differs, see below |

Logs: `F:\ibcmd\lab\04\restructure-check\dn\logs\{prepare,own}_mix.out`, `mix_check5.txt`, `mix_check6.txt`.

## Historical finding: the synonym in the object registry

**Resolved by the synonym rewrite.** This page retains the original measurement of the older build.
The fresh `syn2` / `mix2` twin runs and cluster-session comparison are in [synonyms.md](synonyms.md).
The following 15/16 result describes the pre-fix build, not the current apply.

The registry row `1a621f0f` lists every metadata object with its synonym (`derived-caches.md`, sections 4 and 5). The native apply
rewrites the records of the two objects whose synonym changed: `{"ru","Валюты (проверка)"}` and `{"ru","Демо: Заказ покупателя (проверка)"}`;
after the drop-in apply they still read `Валюты` and `Демо: Заказ покупателя`. Nothing else in the row differs (the diff is those two lines).

This is not made by this change. A stage that only changes a synonym goes through the apply without a restructuring (`gate` `apply-check`),
which writes the registry for new forms and templates only (`si.rs`: "the contents only change with the set of metadata objects") and
leaves a changed synonym in it as it was; the platform's own apply on our twin accepts the state (check 7) and the export
is equal (check 8). With an S1 change in the stage the registry is recomputed for the objects the plan changes, not for the ones
that only got a new synonym. What the stale text is used for (search by synonym, the presentation of a type) was not measured. The
fix would be one more cache update in the apply, for both gates: the record of an object whose synonym changed takes the new text.

## A harmful change elsewhere is still refused

The check names every row it refuses, and a row it names is not withdrawn. The unit tests
(`an_s1_change_and_a_harmful_change_elsewhere_are_still_refused`) hold it for a structural change of a kind S1 does not do and for
an S1 operation that the plan does not make; on the twin, the stage of `s1-acceptance.md` "an attribute added (S1) and `CodeLength` 9 -> 12 (outside S1) in one
stage" is refused as before (exit 1 with the S1 reason, database untouched): `logs/out2_codelength.out` in the same folder.

## What the change does not withdraw

Only the conservative blocker "the descriptor's text differs from the active one" of a row no reason of the check names. The other
blockers stay: a new object the apply cannot create, a row it does not know, a body row of a role the conservative rule does not
pass (rights, command interface, predefined data ...), an omitted blocker. So an S1 change next to a **body** of such a role
(a right of a role, say) is still refused by the conservative rule, although the check accepts the row; the same withdrawal for
bodies is a separate step (the reasons of the conservative rule for bodies are of several kinds, some of them limits of this apply).
