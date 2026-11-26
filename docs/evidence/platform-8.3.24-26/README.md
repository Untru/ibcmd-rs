# #361 native SQL acceptance — 2026-10-23

**Blocked; no new native export/import measurements.** This records the Mac
external lab inventory, access check and evidence harness, not platform support.
The issue stays open. A profile entry does not establish SQL acceptance.

| Release | Exact-build selection | Native SQL export | Native SQL import / reexport | Layouts and write limits |
| --- | --- | --- | --- | --- |
| 8.3.24 | 8.3.24.1819 named by #361; executable absent from inventory | blocked | blocked | unmeasured |
| 8.3.25 | pending authorized installer; public notes identify 8.3.25.1257 | blocked | blocked | unmeasured |
| 8.3.26 | pending authorized installer; public notes identify 8.3.26.1318 | blocked | blocked | unmeasured |

The [machine-readable rows](acceptance-2026-10-23.json) deliberately contain no
measured status, percentages, invented layout census or production write claims.
The [read-only harness and runbook](../../../scripts/platform-lab/README.md)
validate the eventual evidence contract and preserve blocked states.

## Local and official inventory

[Inventory JSON](inventory-2026-10-23.json) records SHA-256 of the installed
`ibcmd` binaries without executing them. `/opt/1cv8` has 8.3.27.1989,
8.3.27.2170, 8.3.27.2214, 8.3.27.2342 and 8.5.1.1592 binaries. The
8.5.4.1683 directory has no `ibcmd` at its root. These directory labels remain
unverified runtime versions and cannot stand in for 8.3.24–8.3.26.

A bounded filename search on `/Volumes/Work` and `~/Downloads` (depth 5),
`/Applications` release directories (depth 6), and `~/1C` installer filenames
(depth 3) found no matching 8.3.24/25/26 installer or release directory. This is
a scoped inventory, not a claim that the entire computer contains no installer.
No production installation or original ERP file was changed.

The official [release portal](https://releases.1c.ru/project/Platform83)
redirected an unauthenticated request to `login.1c.ru` on 2026-10-23. No
credentials were requested/extracted, no authentication/security bypass was
attempted, and no package was downloaded or installed. Public official
[8.3.25.1257 release notes](https://dl03.1c.ru/content/Platform/8_3_25_1257/1cv8upd_8_3_25_1257.htm)
and [8.3.26.1318 release notes](https://dl03.1c.ru/content/Platform/8_3_26_1318/1cv8upd_8_3_26_1318.htm)
identify builds, not accessible server/ibcmd installers. The official
[8.3.24.1819 thin-client listing](https://online.1c.ru/catalog/programs/program/35869730/)
also does not supply the necessary SQL native oracle. No alternate download
mirror was used. Authenticated official installer access remains a prerequisite.

## SQL and corpora coordination with #359

The #359 owner reported no SQL container in the read-only Docker inventory,
no local TCP 1433 listener, no current authorized SQL endpoint or frozen SQL
source snapshot. Historical Windows localhost SQL measurements are evidence
history, not a current Mac lab connection. Local disposable fixtures are file
bases/ERP references; a file-base/CF round trip cannot establish this SQL route.

The user's authorized UT11 **11.5.27.88** template was copied and hash verified
by the #359 owner into the external fixture lab. Its vendor ReadMe requires
platform **8.3.27.1859+** and recommends 8.3.27.1989+. It is unsuitable for the
8.3.24–8.3.26 acceptance corpus. Obtain an authorized earlier corpus whose
minimum version permits each selected exact build; do not downgrade/retag a
newer template or write to the original ERP. Clean-room fixtures may establish
narrow probes only, with their exact coverage declared.

Remaining inputs: authorized official native tools for all three exact builds;
version-compatible frozen corpus plus provenance/hash and permitted use;
authorized SQL endpoint/auth mechanism; source snapshot and two distinct fresh
native-initialized disposable SQL targets per build. All native writes require
lab ownership and coordinated capacity. Credentials and private CF/XML/SQL
content must remain external and absent from public evidence.

#359's separate [Draft PR #501](https://github.com/Untru/ibcmd-rs/pull/501) at
`86ecbdfa` advances its SQL full-diff collector and also claims no SQL acceptance.
This branch is based directly on master `f9ebb8dd6a4b84d57c72a72d5941931e48fae73b`;
there are no stacked donor commits or dependencies on previous open PRs.

## Verification and limits

The focused Python regression suite uses synthetic journals, retained fixtures
and no database/native process. It verifies exact-build/version binding, phase
binary/commit/corpus/SQL/input provenance, both native imports and reexports,
complete full-diff counts and per-file content/path binding to both compared
output inventories, source immutability, distinct empty targets,
explicit layout/write findings, artifact tampering/path/symlink rejection and
malformed-input diagnostics. A dedicated additive CI job runs these checks.
An independent review found provenance gaps, now fixed with regressions, and
reported no remaining blocking code findings after re-review.

The checker emits `ready_for_review` only for complete journals and always
`native_acceptance: not_determined`. Review authentic native logs, full diffs,
SQL initialization, census and limitations before deciding native acceptance.
It does not execute the measurement pipeline or prove journal authenticity.
All current native rows remain blocked; none were promoted by synthetic tests.
