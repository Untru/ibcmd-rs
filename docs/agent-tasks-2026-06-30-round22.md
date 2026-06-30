# Parallel Agent Tasks - 2026-06-30 Round 22

Base branch: `master` after commit `6544df9`.

Lab artifacts must go under `E:\ibcmd_lab`.

Do not generate `ConfigDumpInfo.xml`. It is an explicit scope exclusion.

Current import target remains staging over an existing compatible infobase. Do
not redesign this round as bootstrap into a blank database. For Issue #21,
continue reducing base-blob dependencies by adding base-free row generation or
audits for narrow source asset classes.

Latest full source-only diff:

`E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`

Current full snapshot excluding `ConfigDumpInfo.xml`: 65.8%
(`32638 / 49622` byte-identical), 16984 files still different.

## Task A - Issue #22, CommonAttributes / Configuration.xml

Worktree: `E:\ibcmd_lab\worktrees\issue-22-root-v1`

Goal: reduce the currently 0% `CommonAttributes` / `Configuration.xml` full
snapshot rows.

Scope:

- primary files: `src/mssql_dump.rs`;
- tests in the existing `mssql_dump` test module;
- no docs update unless the full/selected evidence clearly changes a status.

Acceptance:

- implement one real, generic root/CommonAttribute metadata XML layer that is
  derived from metadata blob contents, not from hardcoded database GUIDs or
  object names;
- add focused unit tests for both source versions where XML versioning matters;
- if using lab checks, write all output under `E:\ibcmd_lab`;
- keep `ConfigDumpInfo.xml` excluded.

Suggested evidence:

- targeted unit test names in final report;
- optional selected `source-diff` against one CommonAttribute or
  `Configuration.xml` path from the full snapshot.

## Task B - Issue #16, Shared Form.xml Parity

Worktree: `E:\ibcmd_lab\worktrees\issue-16-forms-v23`

Goal: reduce the largest remaining diff class, `form` (`10690 different`).

Scope:

- shared form parser/formatter/packer code in `src/mssql_dump.rs`;
- do not implement a one-object special case;
- avoid touching object metadata formatters unless directly required by form
  source paths.

Acceptance:

- choose one missing high-confidence `Form.xml` element/property class by
  inspecting `E:\ibcmd_lab\full_diff_20260630_184120\diff_full_source_only.json`;
- implement generic decompile and, where the existing packer supports the same
  area, compile/patch coverage;
- add focused unit tests plus at least one regression-style fixture/assertion
  using a real shape observed in the diff or existing lab files;
- no broad runtime regression in form processing.

Suggested evidence:

- test names in final report;
- before/after selected diff for one affected form if practical.

## Task C - Issue #21, Base-Free Staging Readiness

Worktree: `E:\ibcmd_lab\worktrees\issue-21-basefree-v8`

Goal: reduce staging dependencies on active base blobs without changing the
import model into empty-database bootstrap.

Scope:

- staging/audit code in `src/mssql.rs`, `src/module_blob.rs`, and related
  tests;
- avoid touching export-only metadata XML unless a source asset class needs it.

Acceptance:

- identify one remaining source asset class that currently requires an active
  base blob but can be generated as a base-free row;
- implement the narrow base-free row path or a precise audit that proves why it
  is not yet safe;
- add unit tests around staging readiness / row generation;
- keep destructive SQL writes guarded.

Suggested evidence:

- targeted tests such as `bootstrap_readiness`, `source_stage`, or a new narrow
  test;
- report exact source asset class and whether it is now base-free or remains a
  documented blocker.

