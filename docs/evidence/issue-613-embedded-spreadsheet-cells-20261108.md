# Embedded spreadsheet text cells

Refs #613. Base `54b796f64da5daed3714acdf7810581fe0e58778`; public
clean-room corpus gitlink `8d138b896f462c3053d16076cbabf0ffe5030c1c`.

## Stored rule and scope

The exact public embedded-cell form bodies saved by 8.3.27.2214 and
8.5.1.1150 carry the same text-cell rule: a row supplies its index, format
and counted column/cell pairs; the text cell is `{16,format,localized,0}`.
The localized list is `{1,count,{language,text}...}`. The form's default
format and format table use font and text-fill mask 32769. The absolute
font and color revisions already follow the existing 8.5 primitive adapter.
The raw bodies were extracted and inspected before changing the writer.
No raw body or export tree is committed.

The shared embedded writer now handles this text-cell cohort with row and
column positions, localized text, absolute fonts and font/text-fill/width
formats. Its namespace-aware XML input checks complete admitted structures;
unknown attributes, nested content, format references, unsupported cell
values or formatting refuse. It checks duplicate and unordered positions,
row/column bounds, language declarations, duplicate settings and numeric
bounds. Schema-owned encoders own the counted row, cell, font, format and
complete document records. Before returning a record, the writer reads it
through the existing embedded-spreadsheet decoder and compares every
namespace-resolved Settings node, attribute and scalar. Only structural
indentation and prefix spelling are irrelevant to this internal check;
localized text spaces, quotes and entities remain significant. Existing documents without cells retain their
previous writer. No dependency, generated JSON, vendor or submodule changes
are made.

## Failing first and portable tests

The new public coverage regression first failed with the original
`an embedded mxl:SpreadsheetDocument with cells has no writer yet` refusal.
It now compiles all nine public target forms across saved 2214, 1150 and
1529 trees. Every XML 2.20 case additionally compares the complete re-read
Form.xml bytes, including its spreadsheet settings; no XML normalization
is applied.

Schema tests compare complete native row, cell, font and format values.
Codec tests retain spaces, entities and quotes, accept equivalent namespace
prefixes, and reject unclassified content, unknown namespaces/attributes,
duplicate settings/languages/positions and invalid references. Comments
outside the selected cell settings do not alter the pre-existing empty
cohort.

The physical-adapter inventory of the new writer module has zero guarded
occurrences. Its only policy-baseline addition registers that file with
`occurrences: []`; every old record is preserved. This accepts no new
literal exception or policy rule. The policy guard passes.

## Compatibility and loss guards

An additional red-first test found a regression of the initial fix: a
merge's column element also spells `c`, and the broad prefilter reached the
new parser on the old empty document/named-area cohort. The prefilter now
requires the actual Settings/rowsItem/row/c schema path. Both native builds
accepted and applied an empty document with a merge and named area (eight
commands returned 0); the exact base also bootstrapped and re-exported its
2214 control with all four files equal. The legacy regression is green.

A second red-first guard showed that an unused font table member could be
admitted and then disappear in projection. The complete Settings readback
check now refuses that loss before publishing the record. Six codec tests
and the complete native-record schema test pass, as does strict clippy.
All nine public compile cases and all three XML 2.20 whole-form comparisons
also pass with these guards enabled.

## Matched offline measurements

Both merge-base and fixed 242-set CF runs pass: generated 13285,
native-proven 13236, native-refused 49, equal 8060, different 5039,
exporter refusals 137. The change affects import, so no CF equality baseline
is added or removed. Both matched 124-set stored-row inventories pass with
7449 complete equal files and identical equal/difference lists. Existing
storage expectations are preserved.

Round-trip tests (four PASS) report exactly eight #613 refusal allowances
resolved; only those eight obsolete allowances are removed. The observed
problem suite passes. The final repeat after removing those allowances also passes all four
round-trip tests. The final check passes seven steps, including all 570
form-body tests, all 17 form-schema tests, all 44 form-writer tests and the
complete coverage suite. No test is disabled or relabelled.

## Fresh native controls

Installed native builds are 8.3.27.2214 and authorized 8.5.1.1529. Saved
1150 remains an exact offline oracle, not a claim of an installed 1150
runtime. Both native source configurations were loaded from their exact
public corpus CF and freshly exported: four native commands returned 0.

Three additional isolated configurations on each installed build cover the
original cell, multiple row/column positions with quoted and spaced text,
and multiple languages with changed absolute font and width. Native create,
import, apply and export returned 0 for all 24 commands. Preliminary CF
bootstrap/re-export controls also match all 24 complete files, excluding
ConfigDumpInfo.xml. These preliminary controls are not the final binary
measurement; final clean-commit provenance and verified import results
will be recorded after the full gate.

## Acceptance limits and remaining gates

The complete corpus-managed-forms configuration also contains independent
picture, flowchart-border and PeriodField refusals (#598, #600 and #617).
At clean merge base, full default-verified imports of both fresh native
sets return 255 before writing: eleven rows refuse (#613 plus ten other
rows belonging to #598, #600 and #617); both disposable database hashes
remain identical. The base debug binary SHA-256 is
`4ee1753a8c8af7a617e9b3b292cc5c0c333d26ae4279bc276576d2f113421756`.
Commands were `cargo build --locked` and `ibcmd-rs infobase config import
--db-path=<new-empty-copy> --platform=<installed-build> <fresh-native-set>`
with empty candidate PATH. Candidate comparison remains pending; full-set
import success is not claimed. No unrelated
form fix is bundled into this change. SQL Server, exact 1150 runtime,
release-build and release-audit checks are not claimed.

Final matched corpus/storage results, the clean code commit and iter binary
hash, default-verified import/native apply/re-export, full staged CI and
independent review are pending. Failed or interrupted invocations are not
passing evidence.
