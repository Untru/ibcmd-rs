# Exact-build SQL evidence for issue #361

`acceptance.py` is a read-only inventory and journal audit. It runs no executable,
connects to no database, installs nothing, writes no files, and removes nothing.
It does not change the production profile registry, compatibility matrix, or
write policy. Its successful result is **ready for review**, with
`native_acceptance: not_determined`; hashes establish artifact integrity, not
that a journal truthfully describes a native execution. Review the commands,
SQL initialization/fingerprints, inventories, census and full diffs before any
acceptance decision. Unknown layouts, write limits and nonzero diffs remain
visible in the result and require an explicit reviewer disposition.

## Current state

See [the dated inventory and blockers](../../docs/evidence/platform-8.3.24-26/README.md).
All three releases are blocked. Profile presence, directory names, CF round
trips, newer platforms and synthetic harness tests do not establish native SQL
export/import acceptance.

```sh
source /Volumes/Work/Dev/ibcmd-rs-lab/lab-env.sh
export PYTHONDONTWRITEBYTECODE=1 IBCMD_TEST_KEEP_FIXTURES=1
python3 scripts/platform-lab/acceptance.py inventory --platform-root /opt/1cv8
python3 scripts/platform-lab/acceptance.py check \
  docs/evidence/platform-8.3.24-26/acceptance-2026-10-23.json \
  --evidence-root /Volumes/Work/Dev/ibcmd-rs-lab
# Exit 2 means blocked, 1 invalid/missing evidence, 0 complete journals for review.
python3 tests/scripts/platform_acceptance_test.py -v
```

Tests require `TMPDIR` from `lab-env.sh` and the keep-fixtures flag. Every run
allocates a new directory; no teardown or path reuse removes previous results.
Only synthetic fixtures are written. Native runs must be scheduled with the
lab coordinator and #359, with `CARGO_BUILD_JOBS=2` for candidate builds, the
external Rust 1.95 toolchain, and an explicit `RUSTC` where needed.

## Native procedure once prerequisites are available

1. Obtain authorized official installers for one exact build in each release
   (8.3.24.1819 is the build named by the issue). Download/extract/install only
   into the external lab. Preserve vendor URL, acquisition time, package hash,
   signature verification and license/authorization provenance locally. Do not
   replace the user's `/opt` installation. A minor release name is insufficient.
2. Coordinate an authorized SQL source snapshot with #359. Record corpus name,
   configuration version, snapshot hash, origin, permission and metadata/body
   inventory. Public clean-room fixtures can be used for probes but cannot
   substitute for the agreed corpus. Original ERP is never a write target;
   private CF, XML, SQL data and credentials stay out of Git/PR artifacts.
3. Build a clean, exact candidate commit. Capture its binary hash, version and
   build command/toolchain locally. Probe each native binary's version; retain
   stdout/stderr, exit status and executable hash. Do not infer the build from
   its directory name or reuse a newer native oracle.
4. Freeze the SQL source. Record a before fingerprint. With the same snapshot,
   capture the native and candidate full XML exports. Retain both per-file
   inventories and the full `source-diff` report (including unchanged entries).
   Run the existing `scripts/export-ibcmd-vs-ours.ps1`/`run-parity-matrix.ps1`
   where applicable, after inspecting its lab/write/cleanup guards. Those
   scripts provide export evidence only; they do not prove the import route.
5. Natively initialize two distinct disposable empty SQL targets with this
   exact build. Capture their schema/config inventories and empty-state proof.
   Import the **same native source export** into the native and candidate
   targets respectively. Preserve stage inventories, command logs and statuses.
   If native activation is needed before export, capture it in the corresponding
   output inventory/journal; a stage-only import is insufficient.
6. Use the same exact native executable to export both targets. Compare their
   entire trees using `source-diff`. Capture every missing, extra, changed and
   unchanged path. A failed/unsupported native route is a blocker, never a zero
   diff. Record source fingerprint after the run and prove it unchanged.
7. Inventory observed layouts and unknown/refused layouts, then record the
   exact production write policy and each unsupported write route. Empty lists
   mean explicitly observed none, not skipped analysis. Publish only scrubbed
   evidence/aggregates; preserve raw artifacts externally. Submit journals to
   this audit and independently review them. Do not close #361 while any release
   lacks actual native SQL export/import acceptance.

No cleanup is automatic. Retained outputs may later be moved to Trash under the
user's policy. Never remove original corpora or production installations.

## Journal contract (schema version 1)

The manifest has exactly three `rows`, one for each `release` 8.3.24/25/26.
A row is `state: blocked` with a nonempty `blockers` list, or `state: measured`
with the fields below. Mixed states retain `blocked` overall. Additional exact
builds/corpora should use separate manifests; never generalize a result to all
builds of a release. JSON objects reject duplicate keys.

Every artifact reference is `{"path":"relative/file.json","sha256":"<64 lowercase hex>"}`.
References resolve under the explicit evidence root, forbid symlinks/traversal,
and are hash checked. JSON reports are bounded to 128 MiB; binary/snapshot
hashing streams data. Do not point the evidence root at production data.

| Measured row field | Required contents |
| --- | --- |
| `platform_build` | Exact `8.3.24.N`, `8.3.25.N` or `8.3.26.N` matching `release` |
| `native` | `binary_sha256`, `version_probe` artifact reference |
| `candidate` | `binary_sha256`, full 40-hex `git_commit`, `clean_worktree: true` |
| `corpus` | Nonempty `id`, `version`, `origin`, `authorization`; `snapshot` reference |
| `database` | `engine: mssql`; distinct `source_id`, `native_target_id`, `candidate_target_id`; `source_before`, `source_after`, `empty_targets_proof` references |
| `observations` | Explicit `unknown_layouts` and `write_limits` string lists; `layout_census` and `write_policy` references |
| `phases` | Exactly the six phase names below, each referencing a command journal |
| `comparisons` | `export` and `import`, each referencing a comparison report |

The native version probe contains `platform_build`, `binary_sha256`, integer
`exit_code: 0`, and `stdout`/`stderr` references. Stdout must name exactly one
build equal to the row. Before/after source fingerprints must hash identically.
The structured empty-target proof contains `engine`, `platform_build`, both
`*_target_id` fields, `native_empty: true`, `candidate_empty: true`, and
`native_inventory`/`candidate_inventory` references. Review its SQL queries and
underlying inventories; booleans alone do not prove an empty native database.

Each command journal contains `phase`, `platform_build`, `binary_sha256`,
`candidate_commit`, `corpus_snapshot_sha256`, `database_id`,
`input_inventory_sha256`, nonempty `argv_redacted` array, `started_utc`,
`ended_utc`, integer `exit_code: 0`, plus `stdout`, `stderr`, `output_inventory`
artifact references. Redact secret values, keep actual flags, logical database
IDs and source/target roles reviewable. Phase bindings are checked as follows:

| Phase | Database | Input inventory |
| --- | --- | --- |
| `native_export_source` | source | corpus snapshot |
| `candidate_export_source` | source | corpus snapshot |
| `native_import_empty` | native target | native source export output |
| `candidate_import_empty` | candidate target | native source export output |
| `native_reexport_native_target` | native target | native import output |
| `native_reexport_candidate_target` | candidate target | candidate import output |

Native phases use the native binary hash; candidate phases use the candidate
hash. Comparison reports contain `scope: full`, `platform_build`,
`candidate_commit`, `corpus_snapshot_sha256`, `left_inventory_sha256`,
`right_inventory_sha256`, `summary` and a `full_diff` artifact reference.
Export binds to the two source export outputs; import binds to the two native
reexport outputs. The full diff must repeat the same `left_inventory_sha256` and
`right_inventory_sha256` bindings and `summary`. Each compared export output
inventory has a complete `files` array of unique safe relative `path`, lowercase
`sha256`, and nonnegative integer `size_bytes` entries. The detailed diff has a
`differences` array covering the exact union of those paths, with `status`,
`left_sha256`, `right_sha256`, `left_size_bytes`, `right_size_bytes` matching the
inventories (explicit nulls for the absent side). Status follows file presence
and SHA equality. Reusing an unrelated nested diff is rejected even if summary
counts match or its inventory labels were rewritten. Both summary and per-file counts include
`left_only`, `right_only`, `different`, `unchanged`. Counts must be nonnegative
integers and total at least one. Preserve the original detailed source-diff
fields and document comparison rules/normalization in its journal: the audit
checks byte-hash/size/path/status/count integrity and provenance; it does not
independently establish XML semantic equivalence or authenticate native logs.
