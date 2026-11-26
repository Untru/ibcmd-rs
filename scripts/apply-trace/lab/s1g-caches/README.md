# S1-G lab kit (track trace): the platform proof of a cache row and the new-document case

Scripts of the `trace` track for `docs/apply/derived-caches.md`. They are copies of the `ddl` kit
(`F:\ibcmd\lab\04\restructure\tools`, `scripts/restructure-lab`) with another database prefix
(`ibcmd_rs_05_trace_*`), another port (5614) and another data folder (`F:\ibcmd\lab\05\s1g\kit`); the
ddl probe (`ddl_probe.epf`, `probe\jobs\newcat.bsl`) is used from its lab folder as it is. Native commands run
under the lab "native" lock (`DDL_LOCK_TRACK=trace`). Nothing here touches a database of another track.

| script | what it does |
|---|---|
| `make_kit.py`, `make_kit2.py` | make `dbrow.ps1`, `srv.ps1`, `job.ps1`, `snapshot.py`, `db.py` from the ddl kit |
| `dbrow.ps1` | read, put or delete one `Params` row (`list`, `dump`, `put`, `delete`) |
| `swap_row.ps1` | put a stored (deflated) cache row and give it a new guid in `siVersions`, as the apply does |
| `srv.ps1`, `job.ps1` | a stand-alone `ibsrv` on a lab database, and a thin-client session that runs a BSL job |
| `prove.ps1` | the proof: stand-alone server + session, cluster session, native `config check`, native `config apply` |
| `necessity.ps1` | the necessity table of 12.5: one cache row absent or stale at a time, the probe in a session each time |
| `rowdiff.py`, `regdiff.py`, `xdtodiff.py` | which of the 16 rows differ between two snapshots; the registry records and the XDTO lines a case inserted |
| `make_case_d.py` | case d: a new document with a tabular section cloned from a small БСП document |
| `stage_d.ps1` | stage case d with the native partial import, snapshot, apply natively, snapshot |

## The proof of `c4629235` (derived-caches.md 4)

```
restore-clone.ps1 -Corpus bak -Bak ...bsp8327_c2_native_after.bak -Name ibcmd_rs_05_trace_c2_base  ...   # untouched
restore-clone.ps1 -Corpus bak -Bak ...bsp8327_c2_native_after.bak -Name ibcmd_rs_05_trace_c2_ours  ...   # the twin
IBCMD_RS_CACHES_DUMP=<dir> cargo test --lib restructure::caches::tests_cases::dump_the_case_c_rows_for_the_twin_proof
swap_row.ps1 -Database ibcmd_rs_05_trace_c2_ours -Row c4629235-4823-4320-b8b5-1d08f4c6d612.si -Blob <dir>\c4629235-...si.deflated
prove.ps1 -Database ibcmd_rs_05_trace_c2_base -Label base
prove.ps1 -Database ibcmd_rs_05_trace_c2_ours -Label ours
```

Results: `docs/apply/evidence/derived-caches/prove_base.txt` and `prove_ours.txt`.

## Case d

```
restore-clone.ps1 -Corpus bsp8327 -Name ibcmd_rs_05_trace_d_base ...
python make_case_d.py          # needs the native export tree of a БСП clone (F:\ibcmd\lab\04\restructure\export\s2_b1_nat)
stage_d.ps1 -Database ibcmd_rs_05_trace_d_base       # snapshots d_staged and d_after into F:\ibcmd\lab\05\s1g\store
cargo test --lib restructure::caches                  # IBCMD_RS_TRACE_LAB=<store> when the store is elsewhere
```

## The necessity table on our rows (derived-caches.md 8)

```
restore-clone.ps1 -Corpus bak -Bak ...bsp8327_c2_native_after.bak -Name ibcmd_rs_05_trace_n_base  ...   # native rows
restore-clone.ps1 -Corpus bak -Bak ...bsp8327_c2_native_after.bak -Name ibcmd_rs_05_trace_n_ours  ...   # the twin
IBCMD_RS_CACHES_DUMP=<dir> IBCMD_RS_CACHES_CASE=c cargo test --lib restructure::caches::tests_change::dump_the_rows
swap_row.ps1 -Database ibcmd_rs_05_trace_n_ours -Row <each of the eight rows> -Blob <dir>\<row>.deflated
prove.ps1 -Database ibcmd_rs_05_trace_n_ours -Label ours_all -Steps standalone,cluster
necessity.ps1 -Database ibcmd_rs_05_trace_n_ours -Label ours          # the stale bytes: F:\ibcmd\lab\05\s1g\rows\stale (rowdiff.py of the pristine snapshot)
necessity.ps1 -Database ibcmd_rs_05_trace_n_base -Label base
prove.ps1 -Database <each> -Label <x>_native -Steps check,apply
```

Results: `docs/apply/evidence/derived-caches/{prove_*_all,necessity_*,prove_*_all_native}.txt`.

## S1-F: the native cases N1-N7 and their twins (`docs/apply/new-object.md`, section 8)

The stages of the seven cases are made from the native export tree of the base (a database that already had a native apply):
`make_cases_n.py [case ...]` writes `F:\ibcmd\lab\05\s1g\n\<case>\{stage, files.txt}` (the uuids are new at every run: name the
cases to write). One case, end to end:

```
twin_stage.ps1 -Case n5 -Tag tw5 -Native     # staged clone, native partial import, backup n5_staged.bak, the twins _nat and _own,
                                             # the native apply on _nat, full snapshot nat_after
twin_run.ps1 (ddl kit, DDL_LAB=<store>) -Case n5 -Nat ..._tw5_nat -Own ..._tw5_own -Base ..._tw5_st -StagedLabel n5_staged -Exe <ibcmd-rs.exe>
                                             # dry run, rehearsal (check 10), the real run, checks 2-6
python reg_cmp.py <nat> <own> <staged clone> # the change register without the random keys
twin_extra.ps1 -Nat .. -Own .. -Label n5 -Steps noop,session,export   # checks 7, 9, 8
twin_inject.ps1 -Case n5 -Run <store>\out\run_n5 -Own .. -To ..      # check 12
twin_final.ps1                               # all seven cases on fresh own twins with the final binary
python assemble_twin_evidence.py <file>      # the evidence file
```

`f_created.bsl` is the session job (the names `ДемоКатН*`, `ДемоДокН*`): metadata, XDTO types, a write with a value in every
attribute and tabular row, read back, queries, the serialization, deletion. `twin_run.ps1`, `twin_check.ps1`, `inject_failure.ps1` and the
other checks are the ddl kit's (`scripts/restructure-lab`); `twin_extra.ps1` and `twin_inject.ps1` are their trace-track versions
(the ddl scripts accept `ibcmd_rs_04_ddl_*` databases only).
