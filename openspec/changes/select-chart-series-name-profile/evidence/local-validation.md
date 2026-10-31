# Explicit representation correction and remaining selector gap

Refs #381. The source-owned zero-series CF input is pinned by SHA256
`b2fa2d388face5e7281711d3714815cc304e12bd220f7495be15abfa0a613b8b`.
A retained native 8.3.27.2342 observation published `Pivot` for its unchanged
extra series with no real series. The historical run remains
`FINAL_AUDIT_INCOMPLETE`; the name observation is not full native acceptance.

The explicit `RepublishedInEnglish` representation now permits that case. The
private mode is passed through the chart renderer without mutable global state.
The compatibility default remains `Stored`; no export selector or CLI option
is added. User-changed names, the existing extended-scale exclusion and the
compiler's stored-cache validation remain intact.

A mechanical seam plus four public-source tests first produced exactly one
failure and three passes. Comparing complete assertion values confirmed only
the cached-name token differed. The reviewed predicate correction then passed
all four tests. The suite checks the English zero-series case, unchanged default
and Stored output, user-changed/excluded inputs, and actual compiler roundtrip.
See the fixture's `evidence.json` for source and log hashes. Later changes were
formatting only. No private input, native process, VM or SQL was used by these
unit tests. Fresh independent source review approved the three-patch sequence.

The causal selector for normal UT export remains unproven: source ScriptVariant,
opaque language flags/load origin and runtime context are competing explanations.
No OS/build/corpus heuristic is introduced. Paired native controls, final data
accounting, selector integration and exact-head native byte/inventory acceptance
remain pending. This correction does not claim the five UT Template differences
or all six UT differences are fixed. Full local xtask check/CI is not claimed.
