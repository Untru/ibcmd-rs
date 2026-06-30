# Parallel Agent Tasks - 2026-07-01 Round 39

Base branch: `master` after commit `0ecc9b6`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`.

The current import target remains staging over an existing compatible
infobase. Do not redesign this round as bootstrap into a blank database.

`src/mssql_dump.rs` has been moved to `src/mssql_dump/mod.rs`; timing report
types live in `src/mssql_dump/timing.rs`. Agents should account for this path
change and should not undo the split.

## Task A - Issue #16, Shared Form.xml Parity

Goal: reduce one generic, high-confidence class of `Ext/Form.xml` differences.

Ownership:

- shared form extraction/formatting/packing code in `src/mssql_dump/mod.rs`;
- focused tests for the chosen form shape.

Avoid object-family metadata XML unless the chosen form case requires only a
small resolver adjustment.

## Task B - Issue #17, Template Body Parity

Goal: reduce one template subtype mismatch, preferably MXL or DCS/SKD, with a
representative test.

Ownership:

- template/MXL/DCS source asset code in `src/mssql_dump/mod.rs`;
- focused tests for the chosen template body or `Ext/Template.xml` shape.

Avoid form code and object owner metadata unless directly needed for template
reference resolution.

## Task C - Issue #13, Role Rights.xml Parity

Goal: reduce remaining `Roles/*/Ext/Rights.xml` differences through a generic
ordering or flag rule.

Ownership:

- role rights source asset formatter/parser code in `src/mssql_dump/mod.rs`;
- focused tests for the rights object ordering/flag case.

Do not reintroduce the previously reverted two-object order experiment unless a
broader rule proves it.

## Task D - Issue #18, Register/Subsystem/ExchangePlan Parity

Goal: reduce one concrete metadata XML or auxiliary asset layer for
InformationRegisters, Subsystems, or ExchangePlans.

Ownership:

- the selected family formatter/parser in `src/mssql_dump/mod.rs`;
- focused tests for the chosen family.

Prefer a narrow family-specific improvement over touching shared object XML
helpers broadly.

## Task E - Issue #21, Base-Free Staging Readiness

Goal: reduce dependency on active base blobs for the staging import path without
changing the import target to empty-base bootstrap.

Ownership:

- staging/audit code in `src/infobase.rs`, `src/module_blob.rs`,
  `src/v8_container.rs`, or adjacent focused modules;
- focused tests for base-free row generation/readiness.

Avoid export-only metadata XML unless a source asset class requires it.

## Local Task - Issue #24, `mssql_dump` Mechanical Split

Goal: continue reducing the merge-conflict surface by extracting fetch/BCP code
to `src/mssql_dump/fetch.rs`.

This local task must remain behavior-preserving and should not be mixed with
parity fixes.
