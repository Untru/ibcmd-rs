# Final merge review and corpus integration (2026-10-26)

Issue #582, PR #584. The earlier measurements remain source-labelled in the
other evidence documents; no SQL/application writes were used for these fixes.

## Reproduced review defects

- The real source gate refused removal of a native catalog module, while
  schema classification returned None. Both staging and activation now retain
  the original refusal unless a concrete replacement schema plan exists.
- Structural planning separately establishes SQL ProductVersion >=14 and
  database compatibility >=110 before modern catalog or ordered STRING_AGG
  queries. Reads use legacy-compatible syntax; the preflight refuses if facts
  are missing, ambiguous or incorrectly typed. Code-only updates skip this structural preflight.
- An isolated --exact child that runs zero tests no longer counts as success.
  The completed-stage probe reuses the shared helper; the native dialog helper
  also proves one passing child per platform/case.
- Root references require matching selected descriptors with consistent kind,
  name and UUID. Seven invalid root/identity edits first passed the actual
  source proof; they now refuse. Coordinated tableless add/remove/rename and
  requested ordering remain supported. Selected imports use their effective
  overlay and ignore unselected malformed root input.
- A valid catalog addition no longer overrides a retained tableless UUID
  refusal. The combined native-source control first reached schema-state
  validation despite the substituted CommonForm UUID; it now refuses earlier.
  Both live and pure schema planning reuse the compiler's correspondence proof.

The first three regressions were red, then all five new admission tests passed;
seven root defects were reproduced before the root fix. The subsequent combined
identity regression was separately red, then green. Independent direct harness
verification at 03291c9879cfa6e43fa807796cd639bed47d6bd3 passed tree 19,
admission 6, profile 17 and completed-stage probe 1 (one helper case overlaps).
Parent reruns on 6cd5902e passed tree 19, admission 5, profile 17 and probe 1; the combined regression then passed on 03291c98. The inner check gate passed all
7 steps, including the actual Coverage plan consumer, on that revision.

SQL feature source: [Microsoft STRING_AGG requirements](https://learn.microsoft.com/en-us/sql/t-sql/functions/string-agg-transact-sql?view=sql-server-2017).

## Corpus union required by current master

Master #583 pins the ordinary Coverage changes at 9832bb8b; extension fixtures
were pinned at 3b9a77c8. The integrated pin is
8d138b896f462c3053d16076cbabf0ffe5030c1c, with parents
3b9a77c87b7f8a7bd025a9dceb5114e998c64b2a and
5a98438cdbd93e67b1480202637b4489cece0e8e (which includes 9832bb8b).

Only corpus-index.json and coverage/README.md needed manual resolution.
The README keeps both parents' sections, and the approved verifier rebuilt the
index. A separate 3-way blob/mode audit matched all 39,423 files outside those
two paths exactly to their appropriate parent. An independent critic recomputed
that union and reran the read-only verifier: 39,397 indexed files and 242 native
sets PASS. The 43 generator tests also passed. No native/source/CFE/manifest
bytes or extension versions were regenerated. Corpus PR Untru/1c-fixtures#2
remains unmerged; its published branch contains the integrated pin.

The integration into the product is c0aa7c6ab6847d91d8b4490b172808bceb01a289;
it adds current master's tests/scripts/baselines without changing the reviewed
production code. Complete local CI and final review are recorded separately
with the exact final revision in the PR and local immutable receipts.
## Terminal local gates

Source 8828b498215da13a88655b2d773dcd3af3e3bdb8 passed cargo xtask ci:
28 steps, including the integrated corpus, workspace crates, root integration
suites and 4,108 root-library tests (0 failures, 16 existing ignored tests).
The fresh critic returned APPROVE after verifying the terminal receipt and
independently executing the targeted harness and corpus union checks.
All eight verified CodeRabbit threads were answered and resolved. That
handoff changed only Markdown; strict OpenSpec was revalidated after that edit.
Hosted checks remain mandatory for the final published head before merging.

A subsequent review also reproduced removal of a retained catalog module
alongside the valid native catalog update. The structural classifier now
reuses the existing source refusal policy for moved assets of retained DATA
owners; the explicit form-module exception and assets of new DATA owners
remain admitted. The red/green source probe and path cases cover both the
combined plan boundary and form/template lifecycle refusal. The same shared
source policy also checks skipped table-less paths and root BSL lifecycle
changes, so an unrelated schema delta cannot exempt those paths either.
The original None-fallback proof remains explicit; the actual planner now
refuses a removed retained module earlier. Final gates for these subsequent
code changes are recorded against their published head in the PR.
