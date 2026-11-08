# PeriodField native parity — 2026-11-08

Issue: #617. Base: `54b796f64da5daed3714acdf7810581fe0e58778`.
Measured implementation: `2810310a1a141ca5f14287e3972dd2c5271d0fe7`.
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
- `cargo test --locked -p ibcmd-rs --test integration coverage_corpus::period_field --no-default-features`: 8 passed.
- `cargo xtask check`: 8 steps passed, including all 13 coverage-corpus tests.
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

Product version `0.4.1`, binary SHA-256 `4b959f8e3beec21805745ece46bcef4daf33f7f312dc2c82bef0b90bae49264f`.
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
| 8.3.27.2214 | default | 5 | `f2c855eb3787fe5c932a0e4b29ce6195d6979fbc24ad6df699bd8a356a28c430` | `21f53e2d906e7cf7007d619f2c5fd723c447449d543f2e3061fd72debf9afa1d` |
| 8.3.27.2214 | geometry | 5 | `feff76ee095e3a909e9be6805899e072d39eeb9d173b709eb5c69478713a429b` | `25b42fff247b3607da5d26086a2ed1eade216f3c68ecf3c4a7a0eb511b108e33` |
| 8.5.1.1529 | default | 5 | `1d3851eb2cdc9908b18fd27f99f486a943cd6954922a5ae4fd1ecfb967bc1e43` | `086146c1f674483235b0c03cc3e567f08a49bd5dd5b78e4ec5fddbc726650123` |
| 8.5.1.1529 | geometry | 5 | `0bde24a4116249aae915e8b082845d862cb4819522f4b6d5428cd7649bb1fbe8` | `dd1b048c2398a3e82262f0eb502aeafbc33b174a58f6363cebf696556be24d76` |

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
