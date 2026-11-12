# Stored spreadsheet merge order and uniqueness — 2026-11-12

Refs #637. This is an export normalization fix for stored MOXCEL revisions
6 and 12, using the existing public `corpus-stored-merge-rules` fixtures.
It changes no fixture, physical layout, schema, compiler or dependency.

## Inputs and rule

- Converter base: `54b796f64da5daed3714acdf7810581fe0e58778`.
- Pinned `1c-fixtures`: `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
- Public native profiles: 8.3.27.2214 / XML 2.20 and 8.5.1.1150 / XML 2.21.
- Fixture paths: `coverage/<platform>/corpus-stored-merge-rules` in that
  submodule. CF and expected native file SHA-256 values are checked against
  each manifest before the regression compares output.

| Profile | Decoded CF SHA-256 |
| --- | --- |
| 8.3.27.2214 | `4142563d60895a76134d1cf37969e29b92c7854e50c4aa68e81fb98fd66d63f0` |
| 8.5.1.1150 | `9463b9c1117ff40e98cef4f9b6a64df55dbcf9817d6af7e2ca132b75316a6f79` |

Native XML lists finite merges by ascending row, then column, and contains
one copy of an identical region. The fix sorts only the existing finite
phase. It retains the existing order and relative positions of the three
unbounded-axis phases. Stable ties retain distinct extents or column systems;
deduplication compares all five decoded fields: row, column, height, width,
and optional column-system identifier.

The six target templates have byte-identical stored body rows across the
two profiles. Three rows use revision 6 and three revision 12. Exact storage
extraction for this inspection used a separate converter from source
`3fb85dc3ba02885a4194d9a7e7fbbf448e7e330d`, binary SHA-256
`b80159b0cb8f310a7b3c949a4c1bd1ee3c382036d4557309fd5d871a3979d15c`.
That extraction establishes raw input provenance; it is not a measurement
of the #637 fix.

A read-only scan of the pinned native spreadsheet XML found 574 documents,
420 containing merges and 669 merge records, with no coordinate-order
exceptions or exact duplicate merge records. This does not establish a new
ordering rule for the unbounded-axis phases; their previous order is retained.

## Failing-first regression

Before the production fix:

```text
cargo test --locked -p ibcmd-moxel merge_order_tests::
  0 passed; 2 failed
cargo test --locked -p ibcmd-rs --test integration coverage_corpus::stored_spreadsheet_merges_match_the_native_coordinate_order --no-default-features
  0 passed; 1 failed
```

The unit failures showed row 2 preceding rows 0 and 1, and a nonadjacent
duplicate surviving. The tests compare complete decoded values and preserve
controls with different widths, heights, column-system identities, stable
equal anchors, and all unbounded-axis phases.

The CF regression reported all twelve merge sequences different: six
templates on each profile. It also reported both complete owned native file
maps different for `CovSpreadsheetFacetTemplate`. The six templates are
`CovSpreadsheetFacetTemplate`, `CovSheetAutoFontHeight`, `CovSheetOEMFixedFont`,
`CovSheetVerticalPageBreak`, `CovSheetPageSizeA4`, and `CovSheetSandyBrown`.
The test compares the complete serialized `<merge>` elements, including
their order and multiplicity, against the pinned native files.

The local red-test commit is `2cf88cda`; the local production fix is
`9ba727bc7d21216b0290353eec29943fdaf433ee`. These local history anchors are
retained in the contributor checkout. Publication through the fork's web
interface gives the equivalent source different commit identifiers: the
production change and unit controls are published in `5cd16659656111180eefb1f1b4c3bad334e4b6e1`.

After the fix, the same commands passed: **2/2 unit tests and 1/1 CF test**.
All twelve merge sequences and both complete control file maps matched.
The physical-adapter policy guard also passed without a baseline change:

```text
pwsh -NoProfile -File tools/validate-physical-adapter-policy.ps1 -RepositoryRoot .
```

Rust 1.95.0; one Cargo job per target; test threads 1; development debug
information limited to line tables. No existing test, ignore or baseline was
removed or weakened.

## Product build and both offline export routes

`cargo xtask check --base upstream/master` passed all **5 steps**, including
the complete Moxel crate (**40 passed, 0 failed**) and the complete coverage
suite (1,798 seconds). An earlier check accidentally used the stale fork
default base and was interrupted while testing unrelated crates; it is not
counted as a completed gate.

The subsequent product build used `cargo build --locked --profile iter`
and finished successfully in 38m 57s from a clean checkout:

- Source commit: `5f2aa90ea40a5f0dc291125783ad630970ff6a89`.
- Source tree: `a0991ccfa6198214291fae5f5c04a586b72e1cdf`.
- Executable SHA-256:
  `2cea8045ad0d3d8a5d0383b030b687d3002b229cfc19c6b97c406123e5eea008`.
- Decompressed public `tests/fixtures/file-infobase/empty.1CD.gz` SHA-256:
  `c79a7752a6afe4120316cd5fca7941be18ac840b6a836118f7beab4601d1f891`.

For each profile, decode the public `configuration.cf.b64` and verify the
manifest hash above. The measured commands are:

```text
ibcmd-rs cf export <input.cf> <cf-output> --source-version <2.20-or-2.21>
ibcmd-rs infobase config load --db-path=<disposable-db> --data=<scratch/load-data> <input.cf>
ibcmd-rs infobase config export --db-path=<disposable-db> --data=<scratch/export-data> --platform=<exact-build> <db-output>
```

The database contains a fresh decompressed copy of the public empty fixture.
Loading the original CF stages its stored records directly; the route does
not pre-normalize them through XML import. It measures CONFIGSAVE export,
without a native platform apply or an active configuration claim. All six
commands exited **0**, and every scratch database, backup, data directory and
output tree was removed after measurement.

Both routes matched all six target templates on both profiles: **24 complete
owned file maps / 48 files byte-equal**, with no extra owned files. Their
**24 complete serialized merge sequences** also matched byte-for-byte.
The measurement revealed that the other five target templates are complete
matches in this stored-merge fixture too, so the regression was strengthened
to require complete native file maps for all six, rather than the control
template alone.

The official `tools/coverage/update_baseline.py` consumed a targeted report
derived from the completed CF measurement and added all six newly equal
target variants to each profile's converter baseline (**12 additions**).
The report is a product measurement, not purported Cargo output. These are
new baseline files; no earlier baseline entry was removed or changed.

## Full local merge gate

`cargo xtask ci` completed **29/29 steps**, with no skipped local step and an
empty test temporary directory. It ran on tree
`d71b39b6b084c1a9234230227f502708daf2a861`, initially local commit
`4fbe7a2c38293154200d2ca2725c757f1b2c5310`, verified identical to published
`d39d91d94880bd2181344d5b29765e8447b29e0f`:

- Workspace crates: **4,709 passed, 0 failed, 90 ignored**.
- Root integration suites: **518 passed, 0 failed, 4 ignored**.
- Root library: **895 passed, 0 failed, 7 ignored**.

The fresh-native evidence below is a subsequent documentation-only update.

## Fresh native 8.5.1.1529 acceptance — 2026-11-12

Installed native Windows x86-64 `ibcmd` version **8.5.1.1529**, executable
SHA-256
`4dde71c1d061cc14c619255ec524b4ca8aca1f9bc38a92d1d766eb7d57721d20`,
loaded and applied the original, hash-checked 8.5.1.1150 stored-merge CF.
Its SHA-256 is `9463b9c1117ff40e98cef4f9b6a64df55dbcf9817d6af7e2ca132b75316a6f79`.
The converter is the preserved iter executable from `5f2aa90e`, with the
SHA-256 recorded above. All subsequent changes through `d39d91d9` affect only
the test, baselines and evidence; product source is unchanged.

Both processes use `LANG=ru_RU.UTF-8`; create the disposable file infobase
with `--locale=ru_RU`. Each invocation has its own scratch data directory:

```text
<native> infobase create --db-path=<db> --create-database --locale=ru_RU --load=<original.cf> --apply --force --data=<data-1>
<native> infobase config export --db-path=<db> --data=<data-2> <native-xml>
<ours> cf export <original.cf> <ours-cf-xml> --source-version 2.21
<ours> infobase config export --db-path=<db> --platform 8.5.1.1529 --data=<data-3> <ours-db-xml>
```

All **four commands exited 0**. All six complete native target maps match
the pinned native files (**12 file comparisons**). The converter CF export
and export from that native-created, applied file infobase each match all
six complete fresh-native target maps: **12 product maps / 24 file
comparisons byte-equal**, with no extra owned files. This file-infobase
measurement reads the native-applied configuration, in addition to the
earlier offline CONFIGSAVE measurement. Every temporary infobase, data
directory and export was removed on exit.

## Acceptance limits

The complete-file counts cover the six target templates in
`corpus-stored-merge-rules`, not similarly named templates in the separate
facets set. The four other template variants still differ in independent
facets; whole-configuration equality is not claimed. Fresh native evidence
covers 8.5.1.1529. The 8.3.27.2214 and 8.5.1.1150 results above use the
immutable public native CF/XML oracle. No fresh 8.3 executable or live SQL
route was run. The PR retains `Refs #637` for those acceptance limits.
