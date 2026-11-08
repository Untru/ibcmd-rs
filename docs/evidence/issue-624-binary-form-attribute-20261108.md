# Binary form attribute writer — 2026-11-08

Refs #624.

## Candidate and inputs

- Code commit: `af6a4cc26d5d09ec620daddcde9edd84946b39b4`; base `54b796f64da5daed3714acdf7810581fe0e58778`.
- `cargo build --locked --profile iter --no-default-features`; `ibcmd-rs 0.4.1`, SHA-256 `0b06463b39eb340c6d882a801a54f727c208788b26cd3cd3f52cf31451c83d82`.
- macOS 26.6.2 arm64; Rust 1.95.0; `LANG=ru_RU.UTF-8` on both sides.
- Pinned `1c-fixtures`: `8d138b896f462c3053d16076cbabf0ffe5030c1c`. The existing `corpus-form-settings` variant `CovPictureTitleFontEnterOnInput` supplies the native XML and the binary attribute; it is already part of the combined configurations.
- Saved 8.3.27.2214 / 8.5.1.1150 XML remains the offline oracle. Fresh native runs use 8.3.27.2214 and 8.5.1.1529; no fresh 8.5.1.1150 runtime measurement is claimed.

- Native `8.3.27.2214` executable SHA-256: `1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`.
- Native `8.5.1.1529` executable SHA-256: `40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb`.

## Failing first and local checks

- On the base plus the new coverage regression, `cargo test --locked -p ibcmd-rs --test integration coverage_corpus::native_binary_form_attribute_compiles_for_both_dialects --no-default-features` failed: `Form Attribute type is not supported yet: xs:base64Binary` (0 passed, 1 failed). It now passes for both saved dialects.
- `cargo test --locked -p ibcmd-form-writer binary_form_attribute_qualif` — 5 passed. The tests check binary lengths, Variable/Fixed coding, mixed types, CDATA, unknown/empty/duplicate qualifiers, nested nodes and direct text.
- An added `<Length/>` case first failed with `accepted invalid binary qualifiers: <v8:Length/>`; the nested case first failed with `accepted invalid binary qualifiers: <v8:Length>1<v8:Extra/>6</v8:Length>`. Both now refuse before encoding.
- A new `Length unexpected="x"` test first failed with `accepted unsupported binary qualifier attribute`. Unknown attributes and `xsi:nil` on Length, AllowedLength and BinaryDataQualifiers now refuse before encoding, including an empty container. Namespace declarations remain allowed; their positive tests preserve the encoded qualifier values. This check applies only to the newly admitted binary qualifier subtree.
- A `Length xmlns:v8="urn:unsupported"` case first failed with `accepted unsupported binary qualifier namespace`. The new subtree now resolves element namespaces and requires the core URI for its container and leaves, rejecting foreign and unbound names. Correct core names with an arbitrary prefix or default namespace, and unused namespace declarations, preserve the same pattern. Other form qualifiers retain their previous namespace behavior.
- `cargo xtask affected` selected the form-writer crate, schema crate and `coverage_corpus` suite.
- `cargo xtask check` — 6 steps passed. Form writer: 42 passed, 1 existing ignored; schema: 130 passed; coverage: 6 passed. No existing expectation was weakened.
- Physical-adapter policy passes without a baseline change.
- `cargo test --locked -p ibcmd-rs --test integration coverage_round_trip:: --no-default-features -- --nocapture` — 4 passed. It reported eight #624 entries as no longer refused. Only these eight entries were removed from `round-trip.json`; the repeated audit passed with the remaining gaps unchanged.

## Fresh native import / apply / export

For each native build, copy its corpus-form-settings XML tree and retain only `CovPictureTitleFontEnterOnInput` in CommonForms and Configuration.xml/ChildObjects. The corpus helper `tools/coverage/native_round_trip.py::subset` can reconstruct this subset. The original case has Length 0 / Variable; two further disposable copies set Length 16 / Variable and Length 16 / Fixed. No public corpus bytes were changed.

Each control is created with native `--import`, `--apply` and `--force`, then exported. A separate empty candidate database receives the control XML through ibcmd-rs with verification enabled; native apply follows. Both readers then export the candidate. Every native call has its own `--data` directory.

Parameterized command sequence used for each case (`native`, `platform`, `work` and `candidate` name that case's exact tools and disposable directories):

```sh
LANG=ru_RU.UTF-8 "$native" infobase create --db-path="$work/control-db" --create-database --locale=ru_RU --import="$work/source" --apply --force --data="$work/control-create-data"
LANG=ru_RU.UTF-8 "$native" infobase config export --db-path="$work/control-db" "$work/native" --data="$work/control-export-data"
LANG=ru_RU.UTF-8 "$native" infobase create --db-path="$work/final-db" --create-database --locale=ru_RU --data="$work/final-create-data"
LANG=ru_RU.UTF-8 "$candidate" infobase config import --db-path="$work/final-db" --platform="$platform" "$work/native"
LANG=ru_RU.UTF-8 "$native" infobase config apply --db-path="$work/final-db" --force --data="$work/final-apply-data"
LANG=ru_RU.UTF-8 "$native" infobase config export --db-path="$work/final-db" "$work/final-native" --data="$work/final-native-export-data"
LANG=ru_RU.UTF-8 "$candidate" infobase config export --db-path="$work/final-db" --platform="$platform" "$work/final-ours"
```

All commands returned 0. Each tree holds four compared files: Configuration.xml, the language descriptor, the CommonForm descriptor and Form.xml. ConfigDumpInfo.xml is excluded because its generated versions are outside this comparison. No XML or whitespace normalization is applied. Both readers match the fresh native control for all six cases: **24/24 files per reader**, zero differences.

The deterministic tree hash is SHA-256 of sorted `path + NUL + file SHA-256 + NUL + byte length + LF` records. The table hash is identical for the control, the native post-apply export and the ibcmd-rs post-apply export.

| Native build | Length | AllowedLength | Files per reader | Tree SHA-256 |
| --- | ---: | --- | ---: | --- |
| 8.3.27.2214 | 0 | Variable | 4/4 | `3a857365cbfd4d4d7d09a13f42437215886f468c625c8cbb01cd0d7fe67d2b0c` |
| 8.3.27.2214 | 16 | Variable | 4/4 | `de80096d7c86db8d489e9f0bd5688b89a1cac1551ffd52f886b678b3177437f2` |
| 8.3.27.2214 | 16 | Fixed | 4/4 | `e4ea68cb0b9ef21eb68f36d5a4a9a35d9ab1a5eeedf9338fa25058e83e949008` |
| 8.5.1.1529 | 0 | Variable | 4/4 | `384b7d7ceb02c1a186d5c9fd7c1d32e16b8406fa017cb5efb1fa30548533dd1e` |
| 8.5.1.1529 | 16 | Variable | 4/4 | `f6039b727948ec532e6f7a41c79aac63847375351d8f8c9e1b3cebc951f7b3af` |
| 8.5.1.1529 | 16 | Fixed | 4/4 | `5299be8fb769477f3cfcd59b861fe738cb2973cde622f1e09b8a668eacb55a4a` |

## Remaining boundaries

- Native acceptance isolates the target variant. The whole corpus-form-settings set also contains the independently tracked VerticalScroll case #604; completion of that full-set import is not claimed. The combined writer audit keeps all unrelated refusals.
- The new binary support is for form attributes. Other type-bearing contexts are outside #624 and retain their prior behavior.
- SQL Server, live application corpora and release/SBOM audits were not part of these runs. The fresh 8.5 measurement is specifically 8.5.1.1529.

The full local gate is run after staging the change with the hosted CI compilation settings and a short canonical temp path:

```sh
CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 TMPDIR=/private/tmp cargo xtask ci
```

Final staged gate on `78d80f04011b084aaee205bd7742a3743a613e1d`: **29 PASS**, no skipped steps. Workspace 4711 passed / 89 existing ignored; root integration 554 passed / 4 existing ignored; root library 904 passed / 7 existing ignored; temporary-directory check passed. An earlier sandboxed attempt failed resolving npm's registry and is not counted as passing; the final gate used network permission.

Fresh-context follow-up review of that exact head against the stated base: **APPROVE**. The reviewer independently reran all six `cargo xtask check` steps and checked the final binary hash, all six receipt trees/hashes, failing-first logs and CI log. The 42 native commands were recorded by the author, not repeated by the follow-up reviewer. Parent verification additionally reran all five binary-qualifier tests and the exact public two-dialect corpus regression: 5/5 and 1/1 passed. This final documentation commit changes no production code, tests or baseline.
