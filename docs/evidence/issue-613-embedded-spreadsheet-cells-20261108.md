# Embedded spreadsheet text cells

Refs #613. Base `54b796f64da5daed3714acdf7810581fe0e58778`; pinned public
corpus `8d138b896f462c3053d16076cbabf0ffe5030c1c`. Final measured clean code
commit `4cbfd567563712684a2cb7b86486432960f7d726`; Rust 1.95.0, macOS ARM64.
Iter version `ibcmd-rs 0.4.1`, SHA-256
`f0726185ccc634feee7ba510a96acd6119e0ab684d8e5cb0c8362f35a92d14a9`. The version output does not embed the commit;
clean checkout status and binary hash bind these measurements.

## Rule and implementation

The exact public 2214 and 1150 embedded-cell bodies use the same counted
row/column and localized text-cell grammar. Schema-owned builders write
cell mask16, localized lists, absolute fonts and font/text-fill/width formats.
A namespace-resolved Settings parser validates complete admitted structures,
positions, references, languages and numeric bounds. An existing embedded
spreadsheet decoder re-reads each result and compares every Settings node,
attribute and scalar before the record is returned. Structural indentation
and namespace prefixes are irrelevant only to that internal check; text
spaces, entities and quotes remain significant. Unknown cell values or
formatting refuse. Existing no-cell documents retain their previous writer.
No dependency, generated JSON, vendor or gitlink change is made.

The cell document's ten repeat/print/palette scalars are
`0,0,0,0,0,0,0,1,0,1`, measured on all eight native control bodies across both
installed builds. The initial implementation incorrectly borrowed an empty
cohort's sentinel containing a nil UUID. Compatible readback did not reveal
it, but native export did. The corrected sequence is owned by the schema.

## Failing first and development failures

The public coverage test first failed with the original embedded-cells
writer refusal. It now compiles all nine pinned target forms; all three
XML2.20 cases additionally compare the complete re-read Form.xml bytes.
A broad initial prefilter confused a merge column with a row cell; its
red-first regression now selects only Settings/rowsItem/row/c. Another
red-first guard rejects an unused font entry instead of silently dropping
it. Six codec tests cover these boundaries, spaces/quotes/entities,
namespace aliases, unknown attributes/content/namespaces, duplicated or
unordered positions and invalid references.

The first clean iter code8735a45a (SHA70d3d755...) passed own verified import
and native apply but native export returned255, stream-format error. This
was a real defect of this change, not a pre-existing failure, and is not a
passing native route. Exact native/candidate CF saves and raw extraction
isolated the ten-scalar difference. A complete native-saved MOXCEL value
regression was observed red, then passed after the schema fix. The final
binary above was rebuilt from clean4cbfd567 and repeated the entire matrix.

The no-cell legacy control also completes native import/apply/export on
clean base, with all four files equal on both builds. An exploratory script
incorrectly expected a failure there; that assertion is not evidence of a
base failure. The subsequent positive comparison and final matrix preserve
this existing cohort without an unrelated production change.

## Recovered portable evidence

Old temporary worktrees, tools and raw receipts were externally removed
before final verification. Those lost files are not claimed available.
The retained source commits were checked out in durable worktrees and all
following evidence was regenerated: exact raw2214/1150 extraction,
public red-first failure, initial41993 legacy/loss red-first failures,
clean-base binary, matched corpus/storage, policy/check, native references,
final binary and the final matrix. The recovery reproduction script's
last checkout initially failed on a stale deleted-worktree registration;
tests had completed and the feature branch was manually restored clean.
That script's overall exit1 is not a passing gate.

`cargo xtask check`: seven PASS after the native scalar fix, including
570 form-body,18 form-schema and44 form-writer tests and the full coverage
suite. Exact whole native record assertions cover the scalar sequence.
Matched base/fixed CF:242 sets,13285 generated,13236 native-proven,49 native
refused; equal8060/different5039/exporter-refused137 on both sides. Matched
124-set stored-row inventories retain7449 equal complete files and identical
per-path equal/difference lists. No equality baseline changes. Exactly eight
measured obsolete #613 writer-refusal allowances were removed; round-trip
four PASS and observed-problems one PASS after the scalar fix.

The new writer module has zero physical-policy occurrences. Its only
policy-baseline addition is an empty file registry entry, `occurrences:[]`;
all previous records are unchanged. Policy passes; no literal exception or
policy rule is accepted.

## Native controls and final result

Fresh native builds:8.3.27.2214 and authorized8.5.1.1529. The exact saved1150
CF remains the offline input for its route; exact1150 runtime is not claimed.
Both source corpus configurations are freshly loaded/applied/exported from
their pinned CFs. Eight isolated native configurations cover original text,
multiple row/column positions, localized strings and changed font/width,
and the pre-existing empty document with merge/named area. Native reference
preparation:36/36 commands return0. Every native call has unique --data;
both sides LANG=ru_RU.UTF-8. Writes use new disposable bases only. Candidate
children have empty PATH and import retains its default verification.

Final isolated matrix:8/8 default-verified imports,8/8 native applies,
8/8 native exports,8/8 own exports and8/8 offline CF bootstrap/re-export
pairs return0. Native, own and CF exports match96/96 complete files across
24 four-file trees, zero differences. Native XML inputs, source databases
and candidate databases around own read-only exports retain their hashes.
No --no-verify or namespace/whitespace normalization is used for comparisons.

| Build | Case | Full files | Expected and all three result tree SHA-256 |
| --- | --- | --- | --- |
| 8.3.27.2214 | original | 4/4 × 3 | 4352394d643dd4936daae5e6d63b34ffdf381b46c719289d5562a4cbbf841efd |
| 8.3.27.2214 | positions | 4/4 × 3 | f9cdde37cdc02826850b441ce4e840cccab08abed8665711c036b17984065175 |
| 8.3.27.2214 | localized-font | 4/4 × 3 | 98b3b431019f4e44f3225264be75f53f90f24aadb582a1fec065f4b39f3768d7 |
| 8.3.27.2214 | legacy-empty | 4/4 × 3 | adba95b24e191d8dfd65bb40410d7ad5be9b667928cf9924cb1fd8e83dbc4144 |
| 8.5.1.1529 | original | 4/4 × 3 | 608975ada870103ffe27d704e9b0938e268dc64fe3400a4708a5673d29b8a105 |
| 8.5.1.1529 | positions | 4/4 × 3 | 372993ddc56832ca385c219f7356b0fe5925618b9fca256dbd7676c329fce077 |
| 8.5.1.1529 | localized-font | 4/4 × 3 | 7f13637bf20b4e4caf15a930bc21c2ae71ef348e1633de2cfc3c88a27377bd96 |
| 8.5.1.1529 | legacy-empty | 4/4 × 3 | 52ce8f040adfa47f95badffa52a69a9818ed66de4ce84a7951be3bd83efd99cd |

Tree hash is sorted path + NUL + fileSHA256 + NUL + byte length + LF.
ConfigDumpInfo.xml is excluded from tree comparisons. Native executable
SHA-256:2214 `1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`;
1529 `40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb`.

## Full-set acceptance limits

Whole original corpus-managed-forms import does not yet satisfy its global
acceptance on this independent branch: clean base refuses11 rows, final
binary refuses10. The #613 target refusal is gone; the remaining ten exact
CommonForms diagnostics match base (#598 pictures7,#600 borders2,#617 period
field1). Both full imports return255 before mutation; both database hashes
and XML inputs stay unchanged. Base debug SHA-256
`bb7d2e0495455fa85f6420d71f13575a22de8f3b1e49856741d67c0b97a38b48`.

Whole original readers also retain exact reproduced base failures:
2214 CF returns2 for a versions row declaring48 pairs but containing100
fields;1529 CF returns2 and fileDB255 for PeriodField revision48/kind18
unknown14-member property bag. Those refused full routes are not PASS.
Both partial CFs and both full fileDB routes emit the complete byte-equal
target Form.xml. Only the2214 full fileDB route and both selected target
exports return0; the1529 full fileDB route still returns255 for the separate
PeriodField after writing the target. Target equality does not make that
full route PASS. Source database hashes stay unchanged.

The final matrix records59 zero-code commands and three exact known base
reader refusals. Its96-file isolated proof is distinct from these full-set
limits. Thus Refs #613, not Closes; no whole-set compatibility, MSSQL,
exact1150 runtime, release-build or release-audit claim is made.

## Reproduction commands and remaining gate

Build: `cargo build --locked --profile iter`. Native reference control:
`ibcmd infobase create --db-path=<new> --create-database --locale=ru_RU`,
then `infobase config import`, `config apply --force`, `config export`,
each with unique --data. Candidate: `ibcmd-rs infobase config import
--db-path=<another-new> --platform=<build> <native-XML>`, native apply/export,
then own `infobase config export`. Offline route: `cf bootstrap --base-free
--platform=<build>` then `cf export --source-version=<2.20|2.21>`.

Full staged `cargo xtask ci` completed with29 PASS and no skipped local step.
Workspace:4714 passed/0 failed/89 existing ignored;
root integration:554 passed/0 failed/4 existing ignored;
root library:904 passed/0 failed/7 existing ignored.
The temporary directory check passes. Release build, native micro-corpus parity,
SBOM and release audit are not run locally and are not claimed. Independent
fresh-context review remains pending. Failed or interrupted development
invocations above are not passing gates.
