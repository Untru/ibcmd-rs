# PeriodField native parity — 2026-11-08

Issue: #617. Base: `54b796f64da5daed3714acdf7810581fe0e58778`.
Measured implementation: `370b85c48b0abe3d359ac49c7b6439dd870d1a55`.
Public clean-room corpus submodule: `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
No dependencies, vendor adaptations or generated schema JSON changed.

## Failing-first coverage

The existing public `corpus-managed-forms` CovPeriodField reproduces the issue;
no private configuration was converted into a fixture. Before implementation,
`coverage_corpus::period_field_matches_complete_native_form_in_both_editions`
failed on 8.3.27.2214 with a missing PeriodField and
`period_field_native_xml_compiles_in_both_editions` failed with
`<PeriodField> has no writer yet`. The saved 8.5.1.1150 profile refused its
kind-18, revision-2, 14-member property bag.

Additional failing-first controls exposed the stored layout, an unknown UI class,
and malformed scalar XML. A product CLI probe accepted `<Width/>` and published
CF before the scalar guard; the final CLI replay refuses it before publication.

## Inherited namespace regression

Fresh review found that a foreign default namespace declared on ChildItems
was inherited by an unprefixed PeriodField yet accepted. The new regression
ran red on both saved XML editions: ten foreign/unbound element and service
property cases were accepted. The fixed admission resolves element namespaces
through the existing quick-xml namespace reader, including ancestor rebindings,
and requires the measured logform namespace for PeriodField and its direct
properties. A nearer ancestor restoring that namespace remains accepted.
Scalar text/end events pass through the namespace reader so its sibling scope
is restored. No dependency edge changed and older profiles were not rewritten.

A product CLI replay with PATH empty now exits 2 and publishes no CF for the
inherited foreign namespace on both 8.3.27.2214 and 8.5.1.1529. Positive native
import/apply/export was repeated on the namespace-fixed implementation:
20 commands succeeded and all 20 complete files matched again. The binding
and per-run tree hashes below refer to this final-source replay.

## Measured rule and boundaries

The reader/writer admit kind 18 revision 2, 14-member bags inside the measured
37/59 and 48/74 frames. Schema-owned fields cover width, height, maximum width
and height, horizontal/vertical stretch and automatic maximum flags. Native
controls set width 35, height 3, maximum width 40 and maximum height 5; each of
the four flags was independently switched false on both platform builds.
All 24 native flag-control commands succeeded, and each switch changed exactly
one of the fourteen bag members. Offline tests independently pin those physical
flag coordinates; they do not derive expected coordinates from the writer.

Unknown headers, revisions, conditional prefixes, event payloads and nondefault
appearance fail closed. XML scalar values must be canonical measured integers
or booleans; unsupported, empty, nested, qualified, duplicate or attributed
properties are refused before XML version projection. Traversal keeps the
existing core nesting bound. ContextMenu and ExtendedTooltip use existing
service-item writers.

## Portable verification

Commands used Rust 1.95.0, locked dependencies, PowerShell 7.6.9 and a short
isolated temporary directory. Results on the measured implementation:

- `cargo xtask affected`: four form crates plus `coverage_corpus::`.
- `cargo test --locked -p ibcmd-rs --test integration coverage_corpus::period_field --no-default-features`: 9 passed.
- `cargo xtask check`: 8 steps passed, including all 14 coverage-corpus tests.
- `cargo test --locked -p ibcmd-rs --test integration coverage_round_trip:: --no-default-features`: 4 passed.
- `pwsh -NoProfile -File tools/validate-physical-adapter-policy.ps1`: passed without baseline refresh.

Across 242 coverage sets: 8067 equal, 5032 different, 137 exporter-refused.
Exactly seven measured PeriodField equality entries were added and eight
resolved #617 round-trip refusal allowances removed. Both primary platform
families match the complete target Form.xml. All nine public profiles match
the complete PeriodField ChildItems subtree; twelve isolated XML bootstrap
profiles also preserve complete Form.xml (default, geometry and four separate
flag controls per XML edition).

## Native acceptance

Product version `0.4.1`, binary SHA-256 `3c2542877fd8b87acfb70bf33a1c5834a8490d63a4ecf644b27ca8abb1af4dd4`.
All 20 commands succeeded; all 20 full exported files matched, including
ConfigDumpInfo.xml. The four complete target Form.xml hashes also matched
the input source. The malformed empty-width CLI replay exited 2 without
publishing CF.

Native `8.3.27.2214` executable SHA-256 `1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`.

Native `8.5.1.1529` executable SHA-256 `40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb`.



Native runs create disposable empty file infobases, then execute the product's
verified XML import (without `--no-verify`), native apply, native export and
product export. Each native command has its own work directory and both sides
use `LANG=ru_RU.UTF-8`. Default and nondefault geometry cases run for both
platform families. Source-tree hashes are checked unchanged. Tree hashes below
are SHA-256 of compact sorted JSON mapping relative filenames to SHA-256.

| Native build | Case | Equal files | Tree SHA-256 | Form.xml SHA-256 |
|---|---|---:|---|---|
| 8.3.27.2214 | default | 5 | `5a24b7c3430ff4d8945e2d6c32fe7c4797370d16d2da7df624f670ec938fecf4` | `21f53e2d906e7cf7007d619f2c5fd723c447449d543f2e3061fd72debf9afa1d` |
| 8.3.27.2214 | geometry | 5 | `8f00376789266d0a471f19cf6ed56e519e2b35c316c6077916e5d25244156e84` | `25b42fff247b3607da5d26086a2ed1eade216f3c68ecf3c4a7a0eb511b108e33` |
| 8.5.1.1529 | default | 5 | `d8060097ecb4dd0f33c3ab4a70e5a76eebf7d98be91d5bb57f13489969590ac8` | `086146c1f674483235b0c03cc3e567f08a49bd5dd5b78e4ec5fddbc726650123` |
| 8.5.1.1529 | geometry | 5 | `a084699e7f337fec69b12fef329c608a0bc574570bf038ec05a87ef35c617277` | `dd1b048c2398a3e82262f0eb502aeafbc33b174a58f6363cebf696556be24d76` |

## Remaining scope

Fresh native 8.5 measurements use 8.5.1.1529; saved 8.5.1.1150 corpus remains an
offline oracle. No fresh 1150, MSSQL or private typical-configuration run was
performed. Two 8.5 stored-command-panel whole forms retain unrelated root
WindowOpeningMode/Group differences tracked by #632; their PeriodField child
trees match. Other combined-configuration refusals remain outside #617. This
measurement makes no full-corpus compatibility or README progress claim.

Raw CF/1CD files, native trees, blobs and local run paths are excluded from this
report and from the commit. Final merge checks and independent review are
reported by the PR separately.
