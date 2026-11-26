# Native extension restructuring, 2026-10-25

Issue #582; product PR #584; public corpus PR Untru/1c-fixtures#2.
All inputs are independently generated Coverage metadata. SQL writes and
platform sessions used disposable laboratory copies only.

## Exact controls and inputs

| Platform | XML | active/staged configinfo | Extension versions |
| --- | --- | --- | --- |
| 8.3.27.2214 | 2.20 | 80327 | 1.2.0.0 -> 1.2.0.1 |
| 8.5.1.1150 | 2.21 | 80501 | 1.2.0.0 -> 1.2.0.1 |

The update adds two own catalogs, four independent nonperiodical registers,
a decimal(15,2) attribute of an existing own document, an enum value, a common
module, HTTP service, subsystem and non-predefined scheduled job. Before the
update, one catalog record, one document and one register row contain fixed
control values and references to each other. Version in built native CFE and
source XML matches. Additional version 1.2.0.3 creates two non-predefined jobs;
1.2.0.2 records the unsupported predefined-instance case.

Published corpus: `cc0b9072b8fd49a01d272860d09333188fd6aba9`.
Native executable SHA-256 values, CFE hashes and full native SQL before/after
facts are in each pinned `extensions/restructuring/manifest.json` and
`native-plan*.json`. Both fresh native file rounds reproduce the committed XML.
The independent corpus review approved all ten CFE inputs and their provenance.

## Results

| Check | 8.3.27.2214 | 8.5.1.1150 |
| --- | --- | --- |
| Standard own import/apply, full update | pass | pass |
| Old data and references, fresh session | pass | pass |
| New 2 catalogs + 4 registers, fresh writes/reads | pass | pass |
| New enum, document default, module and metadata | pass | pass |
| Native re-export outside ConfigDumpInfo.xml | 33/33 equal | 33/33 equal |
| Native/own columns and data, EXCEPT both directions | 11 tables, zero differences | 11 tables, zero differences |
| Repeated own apply | no-op | no-op |
| Main data-separation base, own import/apply + fresh session | pass | pass |
| Separated-base native re-export | 33/33 equal | 33/33 equal |
| Injected failure immediately before COMMIT | all 20 captured snapshots restored | all 20 captured snapshots restored |
| Generated-script rollback rehearsal | all 20 snapshots unchanged | all 20 snapshots unchanged |
| Stale registry rowversion | refused before DDL, snapshots unchanged | refused before DDL, snapshots unchanged |
| Fresh old-generation session after rollback | pass | pass |

The 11-table comparison excludes SQL timestamp/rowversion columns: native and
own rebuilds generate new concurrency stamps. All user values, record IDs and
references are compared. ConfigDumpInfo.xml is reported separately as derived
storage hashes, not counted as a source-code match.

Execution snapshots: full ordinary matrix source `d1863f17083b7ff36e87620c39f5134787b13ea3`,
binary SHA-256 `fa3d228ce2f0e15c8779815cf1d9469260e810529c09df147535f671df0e8cb9`; separated-base context-fix matrix source
`e809ce1fe5e01afc4e52b662ea32adc3aa4bfdd7`, binary SHA-256 `3d6bd0ae0ab21f50153cd442a0dc3a542b837c7a4eb17e8200638fc0edae6295`.
The subsequent format-admission gate changes no accepted writer/DDL rule and
is covered by exact positive and legacy-negative tests.

## Failure-first evidence and rules

- The old product refuses this update at the structural guard on both exact
  profiles with valid RAC bindings; it leaves the active database unchanged.
- The export-only Coverage gate was already equal. The new Coverage SQL-plan
  consumer first failed on the real scheduled-job path, then matched complete
  native SchemaStorage(1), target/system DBNames and service payloads.
- Adding a job creates a SQL table even with Use=false. Fresh one/two-job
  controls establish allocation order and the global mapping format.
- Main ordinary common attributes and active main separators do not inherit
  into own extension data tables in Infobase scope. They add separator context
  columns to the registry/service tables. The initial service INSERT failed
  SQL515 on the separated-base control; all DDL, metadata and old data rolled
  back. The corrected plan derives columns from SchemaStorage(0) and copies
  actual values from the selected locked registry row. Empty string and NULL
  are preserved. An offline Coverage regression failed before this fix.
- Replacement of an old object UUID at the same path is rejected before
  staging. Existing field/type changes and destructive plans are rejected.
- Physical schema/index drift and unmodeled SQL features are checked before
  DDL; failed execution explicitly rolls back the whole publication.
- Replanning creates fresh DBNames version GUIDs. Default immutable artifact
  names include the script digest so a dry run cannot block the real apply.

## Reproduction and verification

The offline consumer reads the pinned native matrices, decodes the native CFE
pair, plans their full source transition and compares the entire resulting
schema, all name mappings and the scheduled-job service payload. It covers one
job, two jobs, a separated base and the predefined-job refusal on both builds:

```text
cargo test --locked -p ibcmd-rs --test integration coverage_corpus:: --no-default-features
cargo test --locked -p ibcmd-rs --lib mssql_extension_restructure:: --no-default-features
cargo test --locked -p ibcmd-rs --lib mssql_extension_load:: --no-default-features
cargo test --locked -p ibcmd-rs --lib mssql_extension_activation:: --no-default-features
cargo test --locked -p ibcmd-restructure
cargo xtask check
cargo xtask ci
```

Local immutable laboratory manifests, SQL snapshots, source comparisons and
process logs are retained outside the repository. Native follow-up apply is
checked for success and unchanged physical schemas; final gate/review results
are recorded in the PR evidence.

## Admission boundaries

This is measured additive SQL restructuring, not destructive migration or a
format upgrade. Whole-infobase installation scope and the format/profile pairs
above are admitted. Legacy body-only edits retain their existing support.
Predefined job instances require an unimplemented ActivationCondition writer;
new history/other register modes, structural edits of adopted objects and
unmodeled own-common-attribute contributions to registers remain explicit
refusals before staging. A registry-only recovery artifact cannot reverse a
committed schema update. These boundaries do not remove existing data or
silently publish an incomplete database.

## Final local gates and independent review

Historical gated source b6cc4db04e84fc681a240e2e7c3e71b9bddb4b65 (master PR #585 integration; documentation-only handoff 25a332dac398ade1fe12c7702f5a6d921e9f0b2f): `cargo xtask ci` passed all 28 steps,
including the workspace crates, integration suites and 4,112 root-library tests
(0 failures, 16 existing ignored tests). `cargo xtask check` passed all 6 steps.
A fresh-context critic independently reran check, recomputed all four native/own
source comparisons and reviewed the merge delta: APPROVE, no blocking findings.
The shared text primitives from #585 and the portable brace/schema planner are
both retained; no extension restructuring behavior changed during the merge.

Local gates do not replace the hosted release-build, native micro-corpus,
SBOM or release-audit checks. Their current result remains visible in PR #584.
At that historical handoff, product PR584 and corpus PR Untru/1c-fixtures#2 were both unmerged.
The ready dev debug=0 binary for that gated source has SHA-256
2a7a2a0abc5d8d6c28c5f756cfad0927be072cf5d22092c22560a196919022cd.
It is distinct from the native/own SQL-execution binaries identified above.
Later dialog/account-free checks and their different test counts are labelled
by revision in dialog-and-account-free-20261026.md.
