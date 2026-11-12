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

The red-test commit is `2cf88cda`; the production fix is
`9ba727bc7d21216b0290353eec29943fdaf433ee`.

After the fix, the same commands passed: **2/2 unit tests and 1/1 CF test**.
All twelve merge sequences and both complete control file maps matched.
The physical-adapter policy guard also passed without a baseline change:

```text
pwsh -NoProfile -File tools/validate-physical-adapter-policy.ps1 -RepositoryRoot .
```

Rust 1.95.0; one Cargo job per target; test threads 1; development debug
information limited to line tables. No existing test, ignore or baseline was
removed or weakened.

## Acceptance limits

These results use the immutable public native CF/XML oracle. Fresh native
platform runs have not been performed on this host. They are not evidence
of live SQL parity. Complete-file equality of the other five target
templates is a separate assertion from equality of their merge sequences:
other stored spreadsheet facets have independent issues. The PR keeps
`Refs #637` while the issue's full acceptance routes remain open.
