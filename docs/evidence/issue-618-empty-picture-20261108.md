# Empty DCS pictures

Refs #618. Base `54b796f64da5daed3714acdf7810581fe0e58778`.
Pinned public corpus `8d138b896f462c3053d16076cbabf0ffe5030c1c`.

## Native rule and legacy boundary

An unreferenced empty Picture is a valid value, not a missing named reference.
Native 8.5.1.1529 emits a self-closing readable Picture, with its generated
prefix numbered by value depth. The stored primitive and native 8.3.27 XML
use the empty `v8ui:Picture` spelling. No Ref or LoadTransparent child is
invented. Named pictures retain their existing representation and context
requirements.

The exact target storage body from the pinned 1150 nested set and the 1529
combined set is identical: 2860 bytes, SHA-256
`c30c923d97ceab2eb00794a281de4b64b3fb42aabc5d04f9e7fd3345aba0a1c6`.
Freshly loading that 1150 CF in native 1529 produces the readable empty value,
as does the saved 1529 oracle. The saved 1150 oracle instead retains the
classic empty spelling. The new previously refused XML 2.21 case follows the
current measured 1529 writer; the 1150 fixture is unchanged and is not declared
byte-equal. No build inference from archive version/count, no CLI selector,
and no fixture replacement is introduced.

The schema crate owns the new empty-value spelling. The mature compiler's
existing depth traversal, attribute validation and line edits are reused.
Forward and inverse transformations accept only the measured empty shape;
unknown attributes, malformed/empty references and neighboring inline XML
still refuse before a rewritten document is returned. A numeric depth cannot
inject an arbitrary generated XML prefix. No new parser slot, dependency,
vendor adaptation or generated JSON edit.

## Failing first and offline checks

The new forward empty, mixed named/empty and inverse empty tests all failed
before the fix. The public coverage regression failed because the target
Template.xml was not emitted, with the exact missing-picture-reference
refusal. The old blanket refusal assertion was replaced by strict rejection
cases after the native positive case was established; no test was ignored.

- Picture codec tests: 17 PASS, including whole-document forward/inverse
  assertions and strict negative boundaries.
- Public regression: all nine pinned CF archives containing the target are
  checked, including four 2.20 archives, three legacy 1150 inputs and two
  1529 archives. The three complete empty-value elements agree with native
  2.20 or the current saved 1529 2.21 spelling, including exact depth prefixes.
- `cargo xtask check`: six PASS, compiler 137 tests and schema 130 tests,
  including the complete coverage suite.
- Physical-adapter policy PASS, baseline unchanged.
- Matched base/fixed corpus: both complete 242 sets pass. Counts unchanged:
  generated 13285, native-proven 13236, native-refused 49, complete equal 8060,
  different 5039, exporter-refused variants 137. The newly written target
  assets still differ by independent #597 names and legacy syntax, so no
  unproven full-file equality baseline is added.
- Matched stored-row suite: four PASS on each version of the code, 124 sets,
  no new complete-file equality entries and no baseline removal.
- Round-trip: four PASS after replacing the obsolete whole-set #618 refusal
  with a single exact legacy target-file gap. The rest of that set is now
  compared instead of exempted by a broad refusal. The first run that exposed
  this changed failure class is retained, not counted as passing.
- Observed-problems: one PASS, all 1246 historical symptoms retained; the
  separate current-profile empty-value regression requires equality.

## Fresh native controls and commands

Every native invocation uses a distinct `--data`, both sides use
`LANG=ru_RU.UTF-8`, and all writes target new disposable bases. Fresh 1529
create/load/apply/export of the exact pinned 1150 CF succeeds. Minimal
Russian-script controls isolate this picture behavior from #597's independent
English parameter-name problem. On 2214 the control uses the existing 2.20
configuration envelope and classic empty input; on 1529 it uses the pinned
1150 envelope and the same classic empty input. Both create/import/apply,
export and save successfully (six commands). Native 2214 rejects the readable
2.21 type; that cross-syntax attempt is recorded separately and is not counted
as a successful control.

```text
ibcmd infobase create --db-path=<scratch>/control-db --create-database --locale=ru_RU --import=<minimal-native-source> --apply --force --data=<scratch>/create-data
ibcmd infobase config export --db-path=<scratch>/control-db <scratch>/native --data=<scratch>/export-data
ibcmd infobase config save --db-path=<scratch>/control-db <scratch>/saved.cf --data=<scratch>/save-data
ibcmd-rs cf export <scratch>/saved.cf <scratch>/ours-cf --source-version=<2.20|2.21>
ibcmd-rs infobase config export --db-path=<scratch>/control-db --platform=<build> <scratch>/ours-file
ibcmd infobase create --db-path=<scratch>/candidate-db --create-database --locale=ru_RU --data=<scratch>/candidate-create-data
ibcmd-rs infobase config import --db-path=<scratch>/candidate-db --platform=<build> <scratch>/native
ibcmd infobase config apply --db-path=<scratch>/candidate-db --force --data=<scratch>/candidate-apply-data
ibcmd infobase config export --db-path=<scratch>/candidate-db <scratch>/after-native --data=<scratch>/post-native-export-data
ibcmd-rs infobase config export --db-path=<scratch>/candidate-db --platform=<build> <scratch>/after-ours
```

Final binary binding, reader/import results and the full local gate are
recorded below; independent review follows the completed checks. No native trees, databases,
raw blobs, application source or private paths are committed. No MSSQL,
exact 1150 runtime or whole legacy-1150 template equality is claimed.

## Final binary binding

Clean code commit `4d35b4628106f091ec21fdbe729207925123cf76`, `ibcmd-rs 0.4.1`; iter SHA-256
`d7d5e32818f1d4f4ba3779f2f893fdad2806b88cbb4243cb0d162c977f16721a`. Version output does not embed the commit;
the clean-checkout receipt and hash bind the measured binary. The evidence
file is staged before the full local gate so tracked-file guards include it.

## Completed final native run

The final iter binary completed sixteen candidate/native steps with code 0.
For each Russian control, CF and fileDB exports match all five native files.
Both verified imports pass, followed by native apply and native/ibcmd-rs
exports that each match the same five files. Across eight complete trees,
40/40 file comparisons agree without XML or whitespace normalization.
Both original control database hashes remain unchanged by candidate reads.

| Native build | Files per tree | Expected / CF / fileDB / post-apply tree SHA-256 |
| --- | ---: | --- |
| 8.3.27.2214 | 5/5 | `821db0bf0ebe1a3edad5e4ea2b00f89054e93d26f3f8c054df8d456b4590e7f6` |
| 8.5.1.1529 | 5/5 | `f515a9449672f266f1f7a4dba821b685832255c34bf38a18067920751806f3ad` |

The English 1529 reference additionally passes CF and fileDB export with
all three complete readable empty-value elements matching native exactly.
Its complete Template.xml still differs at the three parameter names
(Picture versus Картинка, #597); these differences are reported and never
normalized into a byte-equality claim. That source database hash also
remains unchanged. The saved 1150 legacy oracle remains unchanged and
retains its explicit round-trip target gap.

A separate source control changes only the three current readable empty
values back to the classic spelling, retaining the current Russian minimal
configuration. Its default-verified import into profile 1529 refuses rather
than pretending the legacy source is byte-reproduced. The disposable database
SHA-256 before/after that refusal is identical. This negative boundary is
recorded separately from the sixteen successful primary steps and preserves
Refs #618; no verification bypass is used.

## Full local gate and acceptance record

The first `cargo xtask ci` on code commit
`4d35b4628106f091ec21fdbe729207925123cf76`, with the evidence file staged,
completed with 29 PASS and no skipped local step. Workspace
crates: 4710 passed, zero failed, 89 existing ignored; root integration and
root library checks also passed. The command explicitly excludes release
builds, native micro-corpus parity, SBOM and the offline release audit; these
are not claimed. The targeted fresh native controls above are separate.

The narrow OpenSpec change `fix-empty-dcs-pictures` records the current-build
acceptance boundary and leaves the legacy equality task open. Strict
validation passed with pinned `@fission-ai/openspec@1.6.0`; no workflow gate
is added. The record was written after measuring the discrepancy and code
fix, rather than presented as a prior proposal.

The complete gate is being repeated with the final evidence and new OpenSpec
record tracked. At author handoff that second run is still active; its result
is pending and is not counted as a second successful run. The measured source
and iter binary are unchanged. Independent review must wait for that run to
finish.
