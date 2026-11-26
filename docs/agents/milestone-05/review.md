# Independent first-wave review

Reviewers: coordinator for dynamic; an independent peer for extension/live
and the coordinator's integration resolutions.
Review combines specification and code correctness; actual source and raw
test logs were inspected, not only implementer summaries.

## Dynamic checkpoint: PASS

Base `2a55cb3462aae8ce7cb8e08e8837bb404154bd68`,
implementation `d807ec00f0f3bdfef316549b0b7c3e45500c4ec6`.

- Explicit force is the only dynamic route. Exclusive modes/session refusal
  remain unchanged; unsupported platforms, structural targets and nonempty
  overlay deletion lists refuse before publication.
- The recovered feature diff retains v0.4 synonym/import/restructure fixes.
  The extracted parity writer preserves exclusive SQL statements/order, and
  only the new dynamic capability changes in the platform profiles.
- Descriptor/root/deleted semantic reads bind to their initial digests; the
  final engine snapshot and both markers bind to that judged inventory.
  Transient safe deleted content cannot authorize different published bytes.
- SQL assertions, marker/parity writes and stage consumption remain in one
  serializable transaction. No-op consumes staging without publication/parity.
- Unknown commit outcome retains recovery location/token and never promises
  rollback. Post-commit verification failure is distinguished explicitly.
- Raw final quick logs confirm 3624 passed/0 failed/9 ignored; focused suites
  and CLI integration match the versioned checkpoint evidence. Diff whitespace
  check passes. Current native/session proof is expressly unclaimed.

No outstanding P1/P2 findings in this bounded checkpoint. This review does
not close #347 or approve the wider pending-overlay/8.5/load boundaries.

## Extension checkpoint: PASS

Reviewer: `release_review`, independent of the extension implementation author.
Base `2a55cb3462aae8ce7cb8e08e8837bb404154bd68`, implementation
`c2cccffc061578e3f14cb7d0a8aa9219eaeffcb6`.

- Stored primary/additional event codes produce native Before/After/Override
  forms. Unsupported or ambiguous shapes refuse, retaining the BaseForm path
  and the measured Before command behavior.
- The lexical XML finding is repaired: semantic attributes/dispatch, comment
  handling and after-fold checks prevent silent loss of interceptors.
- The reviewer verified native row hashes/blocks, raw trees and thirteen
  versioned comparisons. All source bytes match; the only CDI normalization
  is configVersion values. Applied own CAS roots match the staged reports.
- Final sequential raw logs on the fixed source report 3607 passed/0 failed/
  9 ignored, with fmt/policy/clippy passing. The policy baseline adds exactly
  two reviewed name-special-case occurrences and preserves old allowances.
- The evidence identifies the actual staging binary, subsequent parser
  repairs, native Version seed 1.7.0.0 → 1.7.0.1, no built CFE, and the
  unchanged direct root-Version refusal. Scope does not close all of #348.

No outstanding P1/P2 findings. Coordinator cherry-pick applied without a
source conflict; the shared mssql.rs change remains the reviewed dispatch.

## Live checkpoint: PASS in its bounded scope

Reviewer: `release_review`. Base `2a55cb34`, implementation
`5818be39f1fc5b4483979e53660e3ab09fc21d33`.

- The explicit staged checkpoint preserves the default two-cycle SQL body,
  refuses the high-level source option and never infers warm readiness from
  idle SQL handles. Real active/warm session acceptance remains open.
- Canonical database-GUID master/session locks and backup-history pending
  guards cover the declared source-stage/default LIVE paths, including
  no-op stage deletion. Other standalone import/stage/admin paths are outside
  this coverage and changed state refuses continuation.
- Actual SQL-only logs prove first-state 1/0, append 1/1, completion 2/0,
  repeated 2/0 without a third backup set, and pending/held-lock/foreign-LSN
  refusals. These fixtures do not promote metadata or verify RAS/client code.
- Exact SQL 17.0.1135.8 and its measured header layout are required; the
  earlier survey's SQL 16 attribution was corrected. Unsupported engines and
  changed RAC agent build/storage/registration refuse before cycle 2.
- Token/identity/fork/generation/header/chain checks share the append's SQL
  session. Unknown completion remains unknown; error cleanup cannot undo
  committed promotion. Serialized artifact bytes satisfy the same 64 MiB
  read bound before writing the manifest or beginning phase 1.
- Final targeted 49/0/1, all-targets/fmt/policy logs pass; the preceding full
  3611/0/10 run and the final narrow regressions are identified separately.
  Both owned SQL clones were dropped using the guarded shared helper.

No outstanding P1/P2. This does not close #409 F-5 or the native/session/load
acceptance, and does not enable 1C 8.5 or unmeasured SQL 16.

## Integration resolution: PASS

Independent peer review at `e8e88586` compared complete added/deleted patch
payloads with all three accepted implementation commits. The entire LIVE
delta is retained, and the additional activation delta exactly matches the
accepted dynamic changes. Extension adapters, dynamic owned files and v0.4
synonym/state behavior are preserved. Preflight precedes the recovered input
reader; both full test groups remain and no production functions duplicate.
Integrated compilation/tests and current CI are separate from this review.

## Release audit alignment: PASS

Reviewer `release_review` independently checked the CI-discovered stale
dynamic-force refusal. Replacing that case with unsupported extension apply
matches the Rust boundary suite and refuses during parsing before SQL/profile
or executor dispatch. Exit 1/message and empty-PATH checks remain; binary,
archive, oracle-command, SBOM and checksum guards are unchanged. Local saved
CLI audit PASS; combined release binaries remain subject to current CI.
