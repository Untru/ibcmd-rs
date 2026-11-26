# Final 0.4.0 release candidate — 2026-10-01

The final production implementation is integrated at `52ffd10fd76287e457c5c50da87f379e54eb9fd8`.
Subsequent release-candidate changes are documentation and acceptance evidence.
The live runner used the default-feature-disabled `ibcmd-rs 0.4.0` Windows release binary:
SHA-256 `22a40259f930342590c1d8b1fc96fdc17da04e12374fae094ecf8a79c2a3542e`.

## Four-corpus live acceptance

| Corpus | Native tree files | DB export | Offline rows export | Default import into fresh empty database, native apply, native export |
|---|---:|---|---|---|
| BSP 8.3.27 | 12198 | PASS | PASS | PASS |
| BSP 8.5.1 | 12337 | PASS | PASS | PASS |
| ERP UH 8.3.27 | 140709 | PASS | PASS | PASS |
| ERP UH 8.5.1 | 140709 | PASS | PASS | PASS |

Both export modes match every source file and raw `ConfigDumpInfo.xml` byte for byte.
Offline rows export adds only its documented `manifest.json`.
Empty import uses the default nonempty verification guard. Native apply and native
export must exit successfully. The resulting full tree and `ConfigDumpInfo.xml`
inventory match the independent native source; only `configVersion` values in
`ConfigDumpInfo.xml` are normalized after import. Missing, extra, unclassified or
incomplete source-diff entries fail the runner. BSP 8.3.27 native apply returned
its known first-empty-apply SDBL failure; the single bounded retry succeeded,
followed by the complete export comparison. Stage exits remain visible in the TSV.
All SQL databases are disposable
trace clones; original corpora and reference trees remain unchanged.

[Export measurements](exports.tsv), [empty import stages](empty-import.tsv).
Existing-database edits, S1 combinations, native-twin cache and data comparisons
are separately documented in the [milestone audit](../../0.4.0-audit.md),
[8.5 acceptance](../../../import/8.5-acceptance.md) and
[synonym acceptance](../../../apply/evidence/dropin-apply/synonyms.md).
UH schema experiments use configuration-only databases, with empty application tables.

## Local and hosted gates

All 16 [local gates](local-gates.txt) passed. Root library: 3598 passed,
0 failed, 9 intentionally ignored. Strict OpenSpec checks passed for the offline
converter, CF extraction and fast parity specifications. All four required merge
contexts passed on the integrated candidate on both operating systems.

The [manual release dry run](https://github.com/Untru/ibcmd-rs/actions/runs/36815209249)
passed Gate and both target builds. Its six downloaded assets independently
passed the archive allowlist, binary markers, exact archived SBOM bytes,
CycloneDX 1.5/version checks and SHA-256 sidecars; Windows binary reports 0.4.0.
[Downloaded asset audit](dry-run-assets.json). Linux execution/boundary checks run
on the native Linux workflow runner.

Raw source-diff reports, strict scripts, logs, CPU samples, native trees and
binary hash are retained under `F:/ibcmd/lab/04/release-20261001`.
This evidence precedes final merge/tag publication; actual publication is recorded
in the tag workflow and issue #333, rather than inferred from the manual dry run.
