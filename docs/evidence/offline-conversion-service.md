# Offline conversion service evidence

APP-001 connects the existing canonical XML, migration, storage, and CF
adapters behind one platform-independent `convert` command. The production
path is Rust-only: it does not probe `PATH`, locate an installed 1C platform,
load EDT/JVM components, or start a subprocess.

## Pipeline

Every successful route reports these ordered phases:

1. `decode`
2. `validate`
3. `migration_plan`
4. `migrate`
5. `encode_preflight`
6. `atomic_encode`

For `--dry-run`, the first five phases complete and `atomic_encode` is reported
as `skipped_dry_run`. The requested destination remains absent. A separately
requested `--report` file is diagnostic output and contains the same versioned
JSON report as stdout.

Format and profile are independent mandatory inputs. An XML endpoint requires
an exact profile with an `xml_dialect` coordinate. A CF endpoint requires an
exact platform profile with explicit `platform_build` and `storage_profile`
coordinates. Mismatched coordinates fail before decoding the artifact.

## Supported route contracts

| Route | Planning and preflight contract |
|---|---|
| XML -> XML | The deterministic migration graph contains only the verified 2.20 -> 2.21 and guarded 2.21 -> 2.20 edges. Same-profile plans are empty. Every metadata envelope is decoded, canonically validated, migrated transactionally, encoded by the exact target adapter, reparsed, and published as one new source tree. |
| XML -> CF | The complete source tree is decoded and validated first. A direct adapter plan then invokes the base-free compiler, patch preflight, in-memory CF write, and reopen validation before create-new atomic publication. |
| CF -> XML | The CF is decoded into a neutral `StorageImage`, exported into private staging, reparsed as a bounded source tree, and atomically published. Failed records and unknown opaque records block the operation. The exact structural records `root`, `version`, and `versions` are classified separately because the XML -> CF adapter regenerates them deterministically. |
| CF -> CF | Only an exact same-profile lossless repack is supported. Packed bytes, logical headers, layout metadata, and entry order are preflighted and reopen-validated before atomic publication. Cross-profile CF migration has no verified edge and fails closed. |

During cross-profile XML conversion, only BSL modules and recognized binary
assets may pass through byte-exactly. Forms, templates, other XML, and unknown
file kinds require a verified adapter and otherwise block before publication.
The destination and report paths are also rejected when they could replace or
modify either artifact.

The `error` loss policy is the default. `warn` and `drop` are accepted only
through the migration core's codec-owned declarations. Migration reports retain
the stable loss code, object/property path, exact source/target profiles,
reason, requested policy, and actual disposition.

## Extensibility boundary

No central version enum selects behavior. A new version is added through an
effective JSON profile plus the required adapter evidence. A cross-profile
conversion additionally needs an explicit migration edge. Unknown profiles,
missing coordinates, missing adapters, ambiguous paths, unverified downgrades,
and unsupported source files all stop before destination publication.

This orchestration did not require replacing the canonical core. It uses the
existing `ProfileRegistry`, `MigrationGraph`, `MigrationExecutor`,
`SourceTree`, `StorageImage`, bootstrap compiler, and atomic CF/XML writers.

## Executable checks

The integration test runs the release-shaped binary with an empty `PATH` and
proves:

- XML 2.20 -> XML 2.21 dry-run completes migration and encode preflight without
  creating the destination;
- the stable JSON report can be written separately;
- XML 2.20 -> Format16 CF -> XML 2.20 preserves the exact relative source-file
  inventory;
- same-profile CF -> CF repack completes through the lossless writer;
- an existing destination is never overwritten;
- format/profile mismatch, unknown cross-profile assets, and unsafe path
  overlap fail before output.

Run:

```powershell
cargo test --locked --test conversion_cli
cargo check --locked --workspace --all-targets
```
