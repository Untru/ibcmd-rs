# Parallel Agent Tasks - 2026-07-01 Round 40

Base branch: `master` after commit `58386ac`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`.

The current import target remains staging over an existing compatible
infobase. Do not redesign this round as bootstrap into a blank database.

`src/mssql_dump.rs` has been moved to `src/mssql_dump/mod.rs`; timing report
types live in `src/mssql_dump/timing.rs`. Agents should account for this path
change and should not undo the split.

## Task A - Issue #22, CommonAttributes / Configuration.xml

Goal: reduce the still-zero full snapshot rows for `CommonAttributes` or root
`Configuration.xml`.

Ownership:

- root metadata / CommonAttribute metadata XML extraction in
  `src/mssql_dump/mod.rs`;
- focused tests for source XML 2.20/2.21 if versioning matters.

Constraints:

- no database-specific GUID or object-name literals;
- derive the layer from metadata blob structure;
- avoid form/template/right code.

## Task B - Issue #15, Object Metadata XML

Goal: reduce one high-volume owner metadata XML layer for Catalog, Document,
DataProcessor, or Report.

Ownership:

- object-family metadata formatter/parser code in `src/mssql_dump/mod.rs`;
- focused tests from a real or synthetic native blob shape.

Prefer root/child metadata XML properties and refs. Avoid `Ext/Form.xml`,
template body content, and Role Rights.

## Task C - Issue #16, Shared Form.xml Parity

Goal: reduce one generic `Ext/Form.xml` mismatch class across owner groups.

Ownership:

- shared form body parser/formatter/packer code in `src/mssql_dump/mod.rs`;
- focused tests for the selected form item/property shape.

Do not implement one-object special cases.

## Task D - Issue #21, Base-Free Staging Readiness

Goal: reduce or precisely audit one remaining base-blob dependency in the
existing-base staging path.

Ownership:

- staging/readiness code in `src/mssql.rs`, `src/module_blob.rs`,
  `src/source_audit.rs`, or adjacent modules;
- focused tests for row generation/readiness.

Do not touch export-only metadata XML unless the selected source asset class
requires it.

## Task E - Issue #19, Performance Evidence

Goal: produce actionable performance evidence without broad code changes.

Ownership:

- timing/report analysis, docs, or narrow instrumentation;
- lab artifacts under `E:\ibcmd_lab\perf`.

Prefer reading existing timing reports and adding a narrow test/report helper
over running a long full export unless credentials and disk space are already
available.

## Integrated Results

| Task | Issue | Result |
|---|---:|---|
| A | #22 | generic `Configuration.xml` root-child classification for additional families |
| B | #15 | Catalog `QuickChoice` and `ChoiceMode` extracted from native root fields |
| C | #16 | Form `TextDocumentField` explicit `ReadOnly=true` extraction |
| D | #21 | precise sectionless Form body base-blob blocker audit |
| E | #19 | timing-summary CPU breakdown plus existing full-run evidence audit |
