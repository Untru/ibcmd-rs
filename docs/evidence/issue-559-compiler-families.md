# Compiler-family extraction evidence

Historical measurement on 29 October 2026, base `8b54d2ed`, extracted code `f6df6905`.
The root has 463,458 Rust lines before and 431,432 after this change (32,026 fewer). The new crate has 32,069 lines.

## Inventory

The 24 `compiler/families` files contain 29,706 lines and 98 statically named tests. The family callers occupy 30 root files. The family code reaches `CompileAxes`, `compiler::{graph, identity}`, the bundled profile loader (tests), and two module-codec functions. `graph` and `identity` also reach `ArtifactScope`; its external kind is already an alias of `ibcmd_schema::external_artifact::ExternalArtifactKind`.

All families, graph, identity, artifact admission, compiler axes and the profile loader move. Their internal compiler namespace is retained. The root keeps facades: family public modules, crate-private `families::native`, public identity items and crate-private `derive_generation_uuid_v8`, graph/artifact module aliases, `CompileAxes`, and the profile registry. Every root caller keeps its text. The reserved files of the metadata-blob parallel track remain unchanged.

The ten bundled JSON profiles stay in the canonical workspace `profiles/` directory. Only their compile-time include paths change; their bytes and provenance names do not. The new crate is workspace-internal (`publish = false`). No platform Rust module moves: its only apparent dependency was a comment in the profile-loader tests.

The move is commit `77bbdb8d`; the separate mechanical lint cleanup is `f6df6905`. There is no new registry dependency or lockfile version update.

## Guard

All 87 previous baseline file records are unchanged, including the existing records in `config_dump_info.rs` and `incremental.rs`. The new slice contains 31 files and 798 production fingerprint records. Every fingerprint and count matched the pre-move code at `77bbdb8d`. No baseline-refresh command was used.

The later short-circuiting let-chain cleanup changes one name-policy fingerprint, count one:

- Before: `06f20cdf6d98a912cc6698719fda5b234053dbc39bd3a4a6708ba1332a9289fb`.
- After: `a1eb8c22ec2bfbf1d9159e97d2da8911a0a0d134b01e0e3abca8a2a4dc644539`.

[All joined records](issue-559-compiler-families-policy.csv).

| Scratch-copy probe | Expected | Observed |
| --- | --- | --- |
| unchanged | pass | exit 0, as expected |
| uuid | reject | exit 1, as expected |
| name | reject | exit 1, as expected |
| xml | reject | exit 1, as expected |
| test-only | pass | exit 0, as expected |
| new-file | reject | exit 1, as expected |
| missing-file | reject | exit 1, as expected |
| missing-directory | reject | exit 1, as expected |
| crlf | pass | exit 0, as expected |

## Mechanical lint changes

Twelve findings: one `ptr_arg` (a non-resizing writer receives a slice), two `manual_is_multiple_of` (divisor four), one `collapsible_if` (short-circuiting identity check), two `assertions_on_constants` (assertions retained in a const block), four `obfuscated_if_else` in test fixtures, one `items_after_test_module` (the test module moves after the final impl), and one `redundant_pattern_matching` in a test. Test-only root imports move into the test module.

## Local build measurements

Rust 1.95.0, Windows x86_64, 24 logical CPUs, 128-class GB RAM, eight build jobs, dev debug `line-tables-only`. Each worktree has a separate target. Dependencies are cached. The full measurement cleans only `ibcmd-rs`, then builds its complete library. A comment invalidates a neighboring root module or a family source; each edit is restored and rebuilt. No other Cargo command ran during these rounds.

| Case, seconds | Base rounds 1, 2 | Extracted rounds 1, 2 |
| --- | --- | --- |
| full | 90.393, 70.691 | 81.166, 72.435 |
| neighbor | 17.402, 25.762 | 13.415, 11.815 |
| neighbor-restore | 13.606, 14.123 | 13.738, 10.955 |
| family | 14.237, 14.065 | 12.978, 11.735 |
| family-restore | 14.423, 19.995 | 13.023, 13.155 |

The complete-build samples overlap the between-round spread; these two rounds do not establish a repeatable complete-build speedup. The incremental samples are smaller after extraction. Linux CI durations must be compared separately in the PR once its checks complete.

Memory is measured with Windows process counters: sampled summed process working set and individual process `PeakWorkingSetSize`. It is not a Linux RSS measurement; shared resident pages can be counted in more than one process. Raw byte values and commands are in [the measurement JSON](issue-559-compiler-families-measurements.json).

The base root library reports 81 warnings. The extraction leaves one additional unused import, `unpack_module_container_text`, in the reserved `src/module_blob.rs`: its family caller now reads the codec crate directly. The reserved file is not edited or its warning suppressed. Test warning counts and complete named outcomes are recorded in the PR after its full checks.

## Next independent candidates

These are direct lexical root references after excluding comments, strings, char/byte-char literals and whole-module self references; tests are included. Counts and module-name maps agree with an independent scan using the physical-policy guard's Rust tokenizer. They are not a transitive dependency closure. `super` aliases and wildcard imports require name analysis before another extraction.

| Candidate | Lines | External root modules | Direct references | Parent references to resolve |
| --- | ---: | ---: | ---: | ---: |
| `src/restructure` | 22200 | 13 | 95 | 15 |
| `src/metadata_model` | 27024 | 7 | 56 | 264 |
| `src/mssql_dump/source_assets.rs` | 6108 | 3 | 44 | 19 |
| `src/mssql_dump/refs.rs` | 5550 | 2 | 8 | 13 |
| `src/mssql_dump/dcs.rs` | 1353 | 1 | 54 | 2 |
| `src/sql` | 2530 | 0 | 0 | 10 |

`src/sql` is the clearest independent leaf: its six `crate::sql` references and parent imports remain inside that module. It needs its platform-specific client features preserved. `metadata_model` overlaps the reserved XML adapter of the parallel track. The source-assets, refs and DCS files still need their parent-module names resolved; small direct-reference counts alone do not make them root-free.

[Dependency counts and module names](issue-559-compiler-families-next.json).
