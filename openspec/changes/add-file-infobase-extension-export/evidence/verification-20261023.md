# Named file extension export verification, 2026-10-23

## Revisions and environment

Base: `1688c73b9bef00df31aa87aa0f259a1133fc3624`.
Implementation reviewed and tested: `52b57c3f`.
macOS arm64, Rust `1.95.0 (59807616e 2026-04-14)`,
Cargo `1.95.0 (f2d3ce0bd 2026-03-21)`.
Native oracle: 1C 8.3.27.2214; binary hashes and disposable database recipe
are in `native-8327-preflight-20261023.md`.

The installed Homebrew compiler was 1.96.0. After the base branch pinned
1.95.0, the final gate used an official temporary 1.95.0 toolchain without
changing the system installation. Python 3.14 and portable official
PowerShell 7.6.8 supplied the guard prerequisites. TMPDIR/TEMP/TMP used a
physical directory under `/private/tmp`, because the guard intentionally
rejects a symlinked path such as `/var` on macOS. No guard was weakened and
no Cargo dependency was added.

## Failing-first evidence and independent review

The feature regressions first failed on the unsupported-route error (rc255)
and missing-name rc1 assertion. Independent review used the repository
`review-change` workflow and `rust-expert-best-practices-code-review` skill.
It found four reader boundaries; all were reproduced before fixing:

| boundary | failing assertion | final regression |
| --- | --- | --- |
| physical PARTNO 4294967296 narrowed to zero | `an invalid physical part was published` | `an_oversized_physical_part_number_cannot_wrap_into_part_zero` |
| repeated references to one blob bypassed retained-memory limit | `2 MiB budget accepted 33554432 retained bytes` | `bounded_reader_counts_shared_blob_chains_before_copying` |
| unknown CONFIGCASSAVE free flag hid staging and selected the active image | `a corrupt prepared image fell back to active` | `unknown_staging_record_flags_cannot_hide_a_prepared_image` |
| unmeasured numeric precision was decoded as an integer | `unmeasured part-precision profile was published` | `unmeasured_config_field_precision_and_collation_are_refused` |

The final critic verdict on `1688c73b..52b57c3f` is **APPROVE**: all four
findings are fixed, no further concrete defects found. The reviewer reran
filedb tests with Rust 1.95.0 (17 unit + 9 integration), the root extension
executable (11 passed), diff checks and rebase equivalence checks. Its prior
independent inner-loop run passed all five steps; the final full gate was
run by the implementer below.

## Targeted and required checks

- `npx --yes @fission-ai/openspec@1.6.0 validate add-file-infobase-extension-export --strict --no-interactive`: valid.
- `cargo xtask affected`: selects ibcmd-filedb, file_extension_export and the full gate for the shared/root/OpenSpec changes.
- `cargo test --locked -p ibcmd-filedb`: 17 unit + 9 integration passed.
- `cargo test --locked -p ibcmd-rs --test file_extension_export --no-default-features`: 11 passed on Rust 1.95.0.
- `cargo xtask check`: 5 steps passed on Rust 1.95.0.
- `cargo test --locked -p ibcmd-rs --lib mssql_extensions::tests --no-default-features`: 11 passed on Rust 1.95.0.
- `cargo test --locked -p ibcmd-rs --lib mssql_extension_export::tests --no-default-features`: 6 passed on Rust 1.95.0.

- `cargo xtask ci`: **19 steps passed**, no missing-tool SKIP. Workspace: 1249 passed / 0 failed / 79 ignored (102 harnesses); root integration: 412 passed / 0 failed / 3 ignored in standard harness summaries; root configuration contracts: 112 passed / 0 failed / 1 ignored.

The final full gate ran after staging all implementation and fixture files.
Only this verification evidence and completion checkboxes were added later;
corpus/policy guards and strict OpenSpec were repeated after staging them.

## Final production-binary comparison

`cargo build --locked --profile iter` passed with default features empty
(no platform-oracle). No Rust source changed during or after the build.

Commit: `52b57c3f54ecb6d633a3350cc4b585b25c4a3d9d`. Binary SHA-256:
`235ee2988eaf45863ef23a0da15d43d96cb932e095dd58f8ae690c2d00d1d154`.

For each committed gzip fixture, unpack to a fresh disposable directory,
then run with `LANG=ru_RU.UTF-8`:

```text
ibcmd-rs infobase config export --platform=8.3.27 --db-path=<fresh-db> --extension=<name> <fresh-output>
```

Compare complete path sets and raw bytes with the retained native ibcmd
output trees from the preflight, without normalization or exclusions.
All six commands returned 0: **33/33 identical files**; whole database
bytes remained identical after every export. The portable integration suite
independently compares each file with the committed native SHA-256 oracle.

| state | extension | files | bytes | common native/candidate tree SHA-256 |
| --- | --- | --- | --- | --- |
| staged | ТестРасширение | 6 | 9431 | `dc9a5f3088fe560aba18ca6a29653d4111812c80dd54d3c5be5338e941bd40b8` |
| staged | ДругоеРасширение | 5 | 7669 | `8adb09b2186aebac0ee61aca99445cc83e1c604d2568ca1fa41f7932031ae69f` |
| applied | ТестРасширение | 6 | 9431 | `dc9a5f3088fe560aba18ca6a29653d4111812c80dd54d3c5be5338e941bd40b8` |
| applied | ДругоеРасширение | 5 | 7669 | `8f915271ee379fb134f00cc2be256321ac8692a489b80276cfe2c1984a6ddd9e` |
| mixed | ТестРасширение | 6 | 9496 | `8b4b4d53d91fc9a002ce260700988b61d1c54b0ff265cff4f1ce4b3806c6b365` |
| mixed | ДругоеРасширение | 5 | 7669 | `8f915271ee379fb134f00cc2be256321ac8692a489b80276cfe2c1984a6ddd9e` |

Tree hashes use the preflight's sorted path/NUL/file-hash/NUL/size/LF formula.
The prepared second-extension snapshot preserves the documented early
adopted-language mismatch; it proves export parity, not runtime applicability.
Applied and mixed snapshots contain the corrected adopted-language reference.

## Limits and hand-off

No live SQL Server, private application corpus, 8.5, import/apply or EDT
capability was exercised by the native acceptance. Shared SQL/CFE and file
regressions ran through the portable tests. The additional offline-matrix
release build, native micro-corpus parity, SBOM and release audit were not
run locally; `xtask ci` explicitly reports those separate lanes.

Project 9 Status/Agent remain pending owner access. The implementation
proceeded under an explicitly authorized temporary tracking exception.
The PR requires maintainer review and merge; the issue is not closed by a
draft or a local green gate.
