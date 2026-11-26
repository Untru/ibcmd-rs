# Typed ConfigDumpInfo integration

Narrow port of reviewed #362 30b0461cf1e7b2291577aa9748e88f2d5e87ba43 on top of
the existing b46 snapshot and a5 owner helpers. The public snapshot/owners API and
#380 SQL main-view adaptation stay stable. The codec owns grammar validation,
row/part XML types, attribute escaping and canonical native BOM/CRLF spelling;
this is not lossless arbitrary XML formatting/comment preservation.

The intervening #362 CLI native_versions mode and its empty-CONFIG entry point
are excluded. Only the six public XML captures referenced by codec tests are
ported, with the matching scoped -text fixture attribute to preserve native CRLF
bytes across checkouts, and no private ERP files or unrelated CLI behavior. InventorySink moves
into the existing config_dump_info adapter instead of widening a policy baseline.
Repeated read-only integration review confirmed the codec/tests/captures are
byte-identical to 30b0461 and found no confirmed P1/P2 in the adaptation.

Seven pure codec tests pass (the private-corpus opt-in remains ignored). The first
physical-policy read-only run passes; baseline/scanner/policy documentation are
unchanged. Final exact-head gate results and native owner/six-state host evidence
are recorded outside the repository under the authorized external lab logs.
Full lib after reviewed b63a6bbf portability fix previously passed 4058 tests;
the additional root-clippy never_loop failure was reproduced on the unchanged
f9524b27 base source archive. No lint suppression or unrelated behavior change.

PR publication remains gated on the #362 owner's exact-head results and parent
review; no Ready, merge, release or complete-ERP/performance claim is made.
