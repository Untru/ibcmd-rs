# Space form shortcuts

Issue: #605. Base: `54b796f64da5daed3714acdf7810581fe0e58778`.

The shared shortcut reader and the form record writer recognize `Space` as
virtual-key code 32. Existing modifier masks and unknown-key refusals are
unchanged. The public native CF records for `SpaceForm` contain `{0,32,0}`:
unpacked offsets 1707 (8.3.27.2214) and 1843 (8.5.1.1150 source fixture).
No fixture bytes, schema slots, dependencies or vendor files changed.

## Failing first

- `cargo test --locked -p ibcmd-form-native space_shortcuts -- --nocapture`:
  failed before the fix (`None` instead of `Some("{0,32,0}")`).
- `cargo test --locked -p ibcmd-rs --test integration coverage_corpus::form_space_shortcut_matches_both_native_profiles --no-default-features -- --nocapture`:
  failed before the fix because the full `SpaceForm` XML omitted `Shortcut`.
  After the fix all four full-form byte comparisons pass: `SpaceForm` and
  `CovFacetCommandSpace`, for the exact pinned 8.3.27.2214 and 8.5.1.1150
  CF/native fixtures.

## Portable checks

Rust 1.95.0, macOS ARM64. The pinned corpus is
`8d138b896f462c3053d16076cbabf0ffe5030c1c`.

- Writer boundary test: 1 passed; reader/host-spelling boundary test: 1 passed.
- `cargo xtask affected`: selected both changed crates and `coverage_corpus`.
- `cargo xtask check`: 6 steps passed; coverage suite 6 passed.
- Full coverage measurement with `--nocapture`: 1 passed, no baseline regression.
  Twelve newly equal Space variants are added across eight baselines;
  unrelated newly equal variants are not added. Five newly equal target Form.xml
  paths are also added to stored-row baselines after a machine-readable measurement.
  Eight resolved Space writer-refusal allowances are removed from the round-trip
  ledger, so recurrence is an unlisted failure.
- `cargo test --locked -p ibcmd-rs --test integration coverage_storage_records:: --no-default-features -- --nocapture`:
  4 passed, no storage baseline regression.
- `coverage_round_trip::` on the built integration binary: 4 passed after
  removing the eight resolved Space writer refusal allowances.

## Native checks

Fresh native platforms: 8.3.27.2214 and 8.5.1.1529. The latter reads the
public 8.5.1.1150 fixture; the exact 8.5.1.1150 runtime was not executed.
Each operation uses an isolated disposable infobase and a distinct native
`--data` directory, with `LANG=ru_RU.UTF-8`.

The `corpus-owned-catalog-forms` CF was loaded/applied by the native
platform, exported natively, then exported by ibcmd-rs through CF and file
infobase routes. `SpaceForm/Ext/Form.xml` matches the fresh native file in
all four comparisons. Source `.1CD` hashes remain unchanged after ibcmd-rs
exports.

Target-only projections retain the original public `SpaceForm` or
`CovFacetCommandSpace`, their owner descriptors and required metadata, and
remove unrelated forms. Each projection imports with the default ibcmd-rs
self-check, accepts native `config apply --force`, and re-exports the
complete target Form.xml byte-for-byte on both native platforms: four
successful import/apply/export sequences.

Commands (each `<scratch>` is newly created):

```text
ibcmd infobase create --db-path=<scratch>/ib --create-database --locale=ru_RU --load=<public-fixture.cf> --apply --force --data=<scratch>/d1
ibcmd infobase config export --db-path=<scratch>/ib <scratch>/native --data=<scratch>/d2
ibcmd-rs cf export <public-fixture.cf> <scratch>/cf --source-version=<2.20|2.21>
ibcmd-rs infobase config export --db-path=<scratch>/ib --platform=<native-build> <scratch>/ours
ibcmd infobase create --db-path=<scratch>/target --create-database --locale=ru_RU --data=<scratch>/d3
ibcmd-rs infobase config import --db-path=<scratch>/target --platform=<native-build> <target-only-public-tree>
ibcmd infobase config apply --db-path=<scratch>/target --force --data=<scratch>/d4
ibcmd infobase config export --db-path=<scratch>/target <scratch>/roundtrip --data=<scratch>/d5
```

## Acceptance limits

The whole owned-catalog-form set still refuses deep `DataPath` bindings
tracked by #606. That fix is not included here. Target-only import evidence
does not prove whole-set import, so this change references #605 without
claiming that its entire acceptance is complete. MSSQL was not run.
Fresh native controls above initially used the debug candidate; final iter
binary evidence and merge-gate results are recorded after validation.
