# Empty nested command bars — 2026-11-08

Issue: #594. Code revision: `0253193455a70aa05eeb6fbec1a4682c4272aa4b`; pinned public corpus:
`8d138b896f462c3053d16076cbabf0ffe5030c1c`.

The nested panel reader now accepts the measured revision-one marker through
the same schema parser as the root panel. The marker's revision, length,
reserved member, alignment and autofill remain checked. An empty panel still
requires the existing exact record shape; configured panels remain open.
No object-name rule, numeric parser slot, dependency or vendor change was added.

## Failing-first and targeted checks

```text
cargo test --locked -p ibcmd-rs --test integration coverage_corpus::empty_85_table_command_bar --no-default-features
cargo test --locked -p ibcmd-form-schema revision_one_nested_command_bars
cargo test --locked -p ibcmd-form-schema
cargo xtask affected
cargo xtask check
```

Before the fix the two coverage tests failed on the missing native empty tag,
for XML 2.20 and 2.21. The schema test failed because the revision-one marker
was rejected. After the fix: coverage **2 passed**, schema **17 passed**;
`xtask check` **5 steps passed**, including **7 coverage tests**.

## Complete pinned corpus

The same `coverage_corpus::coverage_sets_hold_their_baseline` command with
`-- --nocapture` was run with the production schema restored from
`54b796f64da5daed3714acdf7810581fe0e58778`, then with the fix restored.
Both runs passed the old baselines, over all **242 sets**.

| Result | Base | Fixed |
|---|---:|---:|
| Generated variants | 13285 | 13285 |
| Platform-proven variants | 13236 | 13236 |
| Platform-refused variants | 49 | 49 |
| Byte-equal variants | 8060 | 8323 |
| Different variants | 5039 | 4776 |
| Exporter refusals | 137 | 137 |

Only the **263 newly equal variants in 10 sets** introduced by this fix were
added with `tools/coverage/update_baseline.py`. Existing expectations remain;
pre-existing newly-equal entries in the base report were excluded.

A final `iter` audit selected configuration CF sets containing native empty
panels: **71 sets**,
**12653 exported forms** and **14354 native empty panels**: **0 remaining
open/close versus self-closing mismatches**. **93 native forms were not
exported and are excluded from that lexical result.** Sets without a
configuration CF were not part of this audit.

## Fresh native comparison

Host: macOS ARM64; Rust 1.95.0; `LANG=ru_RU.UTF-8`. Fresh native runs used
**8.3.27.2214 and 8.5.1.1529**. Saved 8.5.1.1150 CF/XML remains the exact offline
oracle; this is not a fresh runtime measurement on 8.5.1.1150.

The final `cargo build --locked --profile iter` binary reported
`ibcmd-rs 0.4.1`; SHA-256:
`96a6b4eb433e4e916e8468715c510a78d184b61416819c06e86b791d028aa6f5`.
The source checkout was clean at the code revision above. The version output
does not embed that Git revision, so the binding is the clean checkout and
binary hash.

| Native build | Public input | CF export | File-infobase export |
|---|---|---|---|
| 8.3.27.2214 | 8.3.27.2214/corpus-cross-forms-from85 | target exact; 4/4 empty panel lines exact | target exact; 4/4 exact |
| 8.5.1.1529 | 8.5.1.1150/corpus-dynamic-list-table-values | target exact; 3/3 empty panel lines exact | target exact; 3/3 exact |

All four native create/export commands and all four final candidate exports
returned 0. Each native invocation had a separate `--data` directory. Bases
were newly created disposable copies. Candidate exports ran with an empty
child-process `PATH`. The two source 1CD hashes were unchanged by the final
candidate reads:

- 8.3.27.2214: `1878ea0a8bacc10497ecc67684ba8f4b82d02fc70ab6bf6f272c097feb0f26f0` before and after.
- 8.5.1.1529: `22771a4c03e72a81030fd4c6d270ae8ef5d58c8626a00799a3facfb50fcba6aa` before and after.

Reproduction, once the public fixture has been decoded to `set.cf` in a new
scratch directory (use a fresh `--data` directory for each invocation):

```sh
LANG=ru_RU.UTF-8 "$NATIVE" infobase create --db-path="$SCRATCH/ib" --create-database --locale=ru_RU --load="$SCRATCH/set.cf" --apply --force --data="$SCRATCH/d1"
LANG=ru_RU.UTF-8 "$NATIVE" infobase config export --db-path="$SCRATCH/ib" "$SCRATCH/native" --data="$SCRATCH/d2"
LANG=ru_RU.UTF-8 target/iter/ibcmd-rs cf export "$SCRATCH/set.cf" "$SCRATCH/ours-cf" --source-version "$XML_VERSION"
LANG=ru_RU.UTF-8 target/iter/ibcmd-rs infobase config export --db-path="$SCRATCH/ib" --platform="$BUILD" "$SCRATCH/ours-file"
```

Whole-tree equality is not claimed. Excluding ConfigDumpInfo.xml, both 8.3.27
candidate routes retain the missing Configuration.xml (#638) and the
CovCompatibleTable form's RowFilter difference (#595). Both 8.5 routes retain
the CovDynamicTableFolders dynamic-list property differences (#601).
These files were inspected separately from the fixed panel atom.

Native binary SHA-256 values:

- 8.3.27.2214: `1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`; decoded input CF `927b6023bc6b4387d530e230d754ed142b93798273a624005e4b493b9dbf280e`.
- 8.5.1.1529: `40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb`; decoded input CF `5a64438dc6e9f7787347ab32e6c28bfb290008ada50b9c9114b214defa296aa4`.

Tree hashes exclude ConfigDumpInfo.xml and use sorted NFC paths with
`path + NUL + SHA256(bytes) + NUL + size + LF`:

| Build | Native | Candidate CF and file (identical trees) |
|---|---|---|
| 8.3.27.2214 | `75bf1d4542b3e0893a16b260ea72862555e8c953e51a9a4c1a2d1bc391755e35` | `bb7057b8a129fcc7d35dd8c4df0ae72175510ab78f3437d0da3f98052a2b65a4` |
| 8.5.1.1529 | `2aa127cf42a84e6b5056119c16094cd28418f2c8f110d823d82568cf7d371262` | `29b275280bc9f6103201ab05e1593666fd245607b79747928ae04111cad2739c` |

## Full gate and environment

`TMPDIR=/private/tmp CARGO_PROFILE_DEV_DEBUG=line-tables-only cargo xtask ci`
completed: **29 steps passed**, no failed or SKIP steps. Test summaries:
- workspace: 4707 passed, 0 failed, 89 existing ignored.
- integration: 555 passed, 0 failed, 4 existing ignored.
- root library: 904 passed, 0 failed, 7 existing ignored.


The first local CI stopped at `python3 tests/scripts/edt_oracle_test.py`:
macOS's default `/var` temporary path is a symlink rejected by that test's
path custody checks. The identical command on the unchanged merge base
`54b796f6` reproduced the same 48-test result (19 failures, 72 subtest errors,
one case-insensitive-filesystem skip). On that same base, a canonical TMPDIR passed (48 tests,
one case-insensitive-filesystem skip). The final gate uses short canonical `TMPDIR=/private/tmp`
and `CARGO_PROFILE_DEV_DEBUG=line-tables-only`, as the hosted build does.
No test, refusal or gate was disabled.

The release build, hosted native micro-corpus, SBOM and release audit are
separate hosted checks, not part of the local result above. No SQL Server
comparison was performed.

An independently added baseline at the same path must retain the union of
passing `equal` entries when integrated. This fix does not depend on another
PR's code. Native trees, infobases and decoded CFs are local disposable
artifacts and are not distributed in the repository.
