# Existing synonyms: plain and S1 twin acceptance

Measured 2026-10-01 on SQL Server / БСП 8.3.27.2214, rcheck track. The implementation is
`5671a910`; the measured build is `d011d50e` (that commit merged the integration base
`9d1d6158`). This evidence covers the registry fix; it does not claim a separate 8.5
synonym experiment.

## Cases and protocol

- `syn2`: only the Russian synonyms of `Catalog.Валюты` and
  `Document._ДемоЗаказПокупателя` gain ` (проверка)`.
- `mix2`: S1 `b1` (11 attributes removed, two added, six existing objects) plus those
  two synonyms and the synonym of `Subsystem.Администрирование`.

The platform staged each case with `config import files --partial`. The native and own
clones were restored from the same COPY_ONLY backup of that staged state with the lab's
`restore-clone.ps1`; native `config apply --dynamic=disable` ran on one and the direct
drop-in apply on the other. No old survivor was written. New disposable names are
`ibcmd_rs_04_rcheck_dn_<case>_nat20261001` and
`ibcmd_rs_04_rcheck_dn_<case>_ownnew20261001`. The old-build control is
`ibcmd_rs_04_rcheck_dn_syn2_ownold20261001`.

## Measured result

| Check | syn2 | mix2 |
|---|---|---|
| Apply | exit 0, 2.5 s, gate `apply-check`, `synonym_records=2` | exit 0, 10.5 s, gate `s1`, `synonym_records=3`, recovery backup supplied |
| 4 Config, including timestamps, size, attributes and bytes | EXCEPT both ways: 0 | EXCEPT both ways: 0 |
| 3 rebuilt tables | no restructuring | eight tables, all data equal, EXCEPT both ways: 0 |
| 5 DBNames | exact text equal | exact text equal |
| 5 DBSchema | 1761 entries equal, same order | only `DbCopies`/`DbCopiesUpdates` known native drift; entry order differs (pre-order-fix base) |
| 6 Params .si, inflated text | **16/16 exact**, including `1a621f0f` | **16/16 exact**, including `1a621f0f` |
| 7 native re-apply | exit 0, «Обновление конфигурации базы данных не требуется», 12.1 s | same, 2.9 s |
| 8 native exports / source-diff | **12198/12198 unchanged**, no missing or different files | **12198/12198 unchanged**, no missing or different files |
| 9 fresh cluster sessions | metadata, presentations and section probe equal | metadata, presentations and section probe equal |

`ConfigDumpInfo.xml` is compared with the per-export `configVersion` ignored by source-diff.
The rebuilt table row counts are 2, 4840, 716, 0, 0, 1, 1, 1 (`mix2_check3_tables.txt`).
The registry content is exact; the independently generated `siVersions` GUID and native
rewrites of otherwise unchanged cache versions are not required to equal.

## Old-build control and sessions

The older `ibcmd-rs-gate.exe` applies `syn2` successfully but retains the two old synonyms
in `1a621f0f`: only **15/16** cache texts match. Its Config rows and DBSchema/DBNames match
already. The control diff is saved in `synonyms/syn2_old_check6_text.txt`.

All sessions used `register-ib.ps1` (scheduled jobs denied), the 8.3.27 cluster, and
`session_job.ps1 -Job dn/syn_session.bsl`; every clone was unregistered afterwards.
The native, old-build and fixed `syn2` sessions gave identical output. The native and fixed
`mix2` sessions also gave identical output (`synonyms/{syn2,mix2}_session.txt`): the changed
catalog/document synonyms and presentations, `НайтиПоТипу`, the unchanged currency type
presentation `Валюта`, and the subsystem section presentation (changed only in `mix2`).

Thus the stale registry was a proven storage mismatch, but these session operations did
not expose it. Search-by-synonym is not measured here. The fix is justified by exact native
cache parity, not by a claimed user-visible failure of the tested presentation calls.

## Validation and evidence

Quick gates on the merged source pass: fmt, physical-adapter guard, workspace-layer clippy,
root library tests **3581 passed, 0 failed, 9 ignored**. Unit tests cover nested headers,
multiple languages, removed/empty synonym text, embedded quotes, missing registry records,
row/version rewrites, and composing an edit on top of another cache edit.

Committed apply reports and comparator output are in [synonyms/](synonyms/). Export reports retain the
summary and changed-object/ConfigDumpInfo entries; unchanged unrelated entries are omitted, with the
complete-report path recorded in each JSON. Full lab runs
and reusable scripts remain under `F:\ibcmd\lab\04\restructure-check\dn`:
`make_syn2.py`, `make_mix2.py`, `prepare_resume.ps1`, `own_case_resume.ps1`,
`logs/*20261001*`, `bak/{syn2,mix2}_staged.bak`, `ddl_{syn2,mix2}/snap`.
The executable used is `suha/bin/ibcmd-rs-syn20261001.exe`.

## Dynamic-alias reversion regression (079b75ad)

Independent review found a missed case: physical Config descriptor A, effective dynamic alias B and
registry B, then a stage that reverts to the exact physical A bytes. The former SHA filter excluded
that descriptor, and the SQL reader joined physical Config instead of the effective alias. The apply
could fold the alias and publish A while retaining synonym B in the registry.

The corrected reader reconciles **all staged header synonyms with the registry**, including unchanged
physical descriptors and headers of objects present only as aliases. No physical Config read is needed.
Parts are joined before inflation; contradictory headers fail closed. Matching registry records and
siVersions stay unchanged. The unit regression splits the staged compressed descriptor into two parts,
provides no physical Config answer, reverts registry B to A, and verifies the version changes. A second
test verifies an unchanged stage produces no registry/version rewrite.

The live disposable regression used actual aliases produced by native `--dynamic=force` on a fresh
clone of `syn2_staged.bak`. That created eight Config aliases (including an older module generation)
and sixteen Params aliases. **The raw native state is refused by our apply because Params aliases are
unsupported.** To exercise its supported Config-only folding path, the sixteen native-generated cache
aliases were promoted to ordinary Params rows in this clone, retaining all Config aliases and markers.
This normalization is explicit: it is a constructed cache-level twin, not a claim that the raw native
online state is accepted. Native partial import then staged the original A XML. Its compiler re-encoded
the two descriptors, so the regression replaced their staged bytes/size with the clone's own physical
base A payloads to enforce exact digest equality. All other staged rows/versions remained native.

Both native and own twins were restored from that same `alias2_staged.bak`. Logs of normalization and
exact-base preconditions are committed alongside the results.

| Check | Prior synonym build | Corrected reader |
|---|---|---|
| apply | exit 0, registry left B | exit 0, 2.6 s, two records rewritten |
| Config against native twin | EXCEPT both ways 0 | EXCEPT both ways 0 |
| DBSchema / DBNames | unchanged/equal | unchanged/equal |
| inflated .si text | 15/16, two old B synonyms remain | **16/16 exact**, registry now A |
| native re-apply | not needed for this negative control | exit 0, «не требуется», 2.2 s |
| native exports | not needed for this negative control | **12198/12198 unchanged**, no missing/different files |

Quick gates pass again: fmt, policy guard, layer clippy, **3583 root tests passed, 0 failed,
9 ignored**; all ten synonym tests pass. Evidence is `synonyms/alias2_*`, with the full-export
report's original path and SHA256 recorded in the compact JSON. The saved corrected executable is
`F:\ibcmd\lab\04\restructure-check\suha\bin\ibcmd-rs-syn-alias20261001.exe`.

The corrected reader was also rerun on fresh `syn2` and structural `mix2` clones from their original
staged backups: Config matches native, every rebuilt-table EXCEPT is zero on `mix2`, and both cases
still have all sixteen cache texts exact. These final-reader checks are `synonyms/{syn2,mix2}_final_*`.
Fresh cluster sessions on the alias native/fixed twins gave identical original A synonyms and
presentations (including type lookup and section title); `synonyms/alias2_session.txt`. Both were
unregistered after the session probe.
