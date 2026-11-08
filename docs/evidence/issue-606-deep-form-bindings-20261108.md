# Deep form binding chains

Refs #606. Base `54b796f64da5daed3714acdf7810581fe0e58778`;
public corpus `8d138b896f462c3053d16076cbabf0ffe5030c1c`.

## Rules and failing-first evidence

The exact public `corpus-form-deep-bindings` form records on both saved
editions store the complete chain
`{5,{1},{0,b9754f01-29e9-11d6-a3c7-0050bae0a776},{0},{2},{10000}}`.
The last member is `Use` under `Selection`, not `Use` under `Settings` or
another collection. Both reader and inverse vocabulary omitted that member.
The reader now uses a schema-owned named entry; the inverse extends its
existing native member table. No numeric slot index is introduced.

The other two refused bindings name a tabular-section attribute whose declared
type is a DefinedType. The existing expansion helper was used for a form
attribute but not after resolving a configuration metadata field. That
transition now expands the declared type before walking the next member.
The ordinary direct-reference variant already compiled and remains covered.
Unknown members and unavailable metadata still refuse.

Before production changes, the complete public Form.xml comparison failed
on the missing DataPath. The public compiler test separately listed six
refusals: CheckBoxField on each edition, and LabelField/InputField through a
defined type on each edition. Two unit tests returned None instead of the
complete native chain. These failures were observed and retained; initial
fixed-build compile errors are not counted as red-first evidence or success.

Exact uncompressed composer bodies SHA-256:

| Saved edition | SHA-256 |
| --- | --- |
| 8.3.27.2214 | fd1b438a9362965254e10ae93b4bdffdd752ee6a7bea95ce25dc86f1c0977d4f |
| 8.5.1.1150 | 545df3c4f02539559e9f5c5638d04f211d5a4e5395d68cc3acda3ace450ac23b |

Six further exact owned-form bodies were extracted from their public CF
records before changing the writer. No raw bodies or native trees are added
to this repository.

## Verification status

Base: full 242-set CF corpus PASS; stored-row route four tests PASS over
124 sets. Fixed targeted tests pass: two complete-chain units, two public tests
covering ten forms and two complete Form.xml files; unknown members refuse.
The physical-adapter guard passes without baseline changes. `cargo xtask check`
passes all seven steps, including body 570, native 44 and schema 16 tests and
the complete coverage suite. Matched fixed/base CF runs both pass all 242 sets: equal 8060 -> 8066 (+6),
different 5039 -> 5033, exporter refusals remain 137; native-proven count
13236 and native-refused count 49 are unchanged. Stored-row runs both pass
four tests over 124 sets: 7449 -> 7451 equal files (+2). Baselines add only
those measured differences, preserving all previous expectations and newlines.
Exactly 24 resolved #606 writer-refusal allowances are removed; the strict
round-trip suite still passes all four tests afterward. A broader profile
audit finds exact complete DataPath vectors in all 49 public forms carrying
one of the two issue paths, over 13 archives. Two same-named legacy aliases
bind only Object.Code and are outside those two target paths; that separate
gap is not claimed fixed or labelled pre-existing by this measurement.
The recovered binary, native replay and successful full gate are recorded
below. Earlier interrupted/failed gates are retained as history and are not
counted as success. Independent review remains pending.

Fresh native builds are installed 8.3.27.2214 and approved 8.5.1.1529;
saved 1150 remains an honest offline oracle. Both builds have loaded and
exported the exact public deep-binding and owned-catalog CFs: eight commands,
all exit 0. For an isolated import control the owned set retains all metadata
dependencies and its three issue-target forms, removing other form entries.
Both native builds re-import/export this control: four further commands exit
0, and all six target Form.xml files remain byte-identical to the full fresh
native output. The final candidate matrix below is measured separately from those controls.
All writes use disposable bases, each native call uses its own data directory
and LANG=ru_RU.UTF-8. No dependency, vendor edit, generated schema JSON edit,
platform-version heuristic, MSSQL measurement or exact1150 runtime claim.


## Initial completed binary and native routes

Clean code commit `4873840bfadf58504dc4cf4e7dd722741adbdcd8`, Rust 1.95.0, macOS ARM64,
iter profile, version `ibcmd-rs 0.4.1`. Binary SHA-256:
`568cd78ac649b1964e0bcf77937a29dfa8b01397685dee2dee37046cdd7b418d`. The version output does not embed a commit;
the clean checkout recorded before the matrix and this hash bind its source.

| Native build | Executable SHA-256 |
| --- | --- |
| 8.3.27.2214 | 1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25 |
| 8.5.1.1529 | 40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb |

All 28 final matrix commands exit 0. Together with the twelve native controls,
all forty calls exit 0. No `--no-verify` is used, and candidate child processes
have an empty PATH. On both builds, the full original CF and fileDB readers
return 0 and match all twenty complete target Form.xml comparisons: composer,
ordinary-reference counterexample and all three owned deep forms.

The full deep-binding XML set and the isolated three-form owned XML set both
pass default verified import, native apply, native export and candidate export
on each build. All eight complete output trees match their canonical native
controls: 124/124 files, excluding ConfigDumpInfo.xml, with zero byte differences.
All four source databases and all four candidate databases preserve their
hashes around candidate read-only exports.

| Build | Import control | Files per reader | Complete tree SHA-256 |
| --- | --- | ---: | --- |
| 8.3.27.2214 | corpus-form-deep-bindings | 12 | 2b6ce0e96f4d68eb2544275dc8a27986d0434ed62e0a36306cb186b90cf21945 |
| 8.3.27.2214 | corpus-owned-catalog-forms | 19 | b057ccc90ae79ef2e99792d19bb7d5f0fad83ceb20a186a7676792521237b8f0 |
| 8.5.1.1529 | corpus-form-deep-bindings | 12 | fa1c4f34c28749e5c80acf8c678ed1121608754584ed4df3672e6318a95c7acd |
| 8.5.1.1529 | corpus-owned-catalog-forms | 19 | 9a018ceff270011b8b922c1e0312b1c1ea57f41f6228730334e0c368ff5df0cf |

Tree hashes cover sorted path, NUL, file SHA-256, NUL, byte length and LF;
there is no XML, whitespace or content normalization.

This proves the target binding forms, not full original-tree equality. The
original deep-binding fileDB route has one unrelated catalog FillValue
`xsi:nil` versus empty-string difference. The original owned-set CF routes
have four differing non-target forms, and fileDB has those four plus two
catalog FillValue differences, on each build. The forms retain the separate
Space shortcut (#605), table IDs (#596), empty nested command-bar spelling
(#594) and, on 8.5, RowFilter default differences. They are not bundled or
counted as equal here. Full owned-set import is not claimed: its import
control retains every dependency and all three issue-target forms, and its
native rebuild preserves all six target form files byte-for-byte against
the original full native output. Saved 1150 fixtures and all existing equal
baselines remain unchanged apart from the measured additions above.

## Full gate and independent review

All new files and this initial evidence were tracked before `cargo xtask ci`.
The first full gate was interrupted after 25 PASS lines during workspace
crate tests. Its final workspace binary reported four of seven test results;
there is no terminal summary or surviving process, and the original tool
session is unavailable. The cause and exit code are not established. This
run is not a passed gate. Its original CI and workspace logs were retained
before a retry; the later deletion and recovery are recorded next. The final
recovered gate result is below. Release builds, SBOM, the release audit and
MSSQL are outside these local results.

## Durable recovery and apply-check diagnosis

The first full gate was interrupted at 25 PASS without a terminal result.
A subsequent coordinator-owned full gate completed with 28 PASS and one failed
workspace step: `a_stored_row_in_the_newer_format_is_read_like_a_staged_one`
failed at `decoder.decode("Catalog", &twin.before).is_ok()` (128 passed,
one failed, six existing ignored in that harness). Root integration, root
library and the temporary-directory check passed. This is not a passed gate.

During the first attempted base reproduction an external deletion removed
the temporary worktrees, shared toolchain, logs and native/raw artifacts.
The observed missing-standard-library link failures are environmental and
are not a reproduction of the apply-check assertion. Completed measurements
above remain historical results; their deleted raw receipts are not claimed
available. Recovery uses a durable worktree, the same pinned Rust 1.95.0 and
the unchanged public corpus.

After recovery, clean base `54b796f64da5daed3714acdf7810581fe0e58778`
ran the exact failed command `cargo test --locked --workspace --exclude
ibcmd-rs` successfully. Forty default-thread narrow apply-check runs also
passed (129 passed, six existing ignored each). The narrow feature graph
produced another binary, so the exact workspace-built base harness was also
preserved (SHA-256
`528d00b209eae5aae0c2fa4a8277f672f021ffac11b92afef1838661a1e8c844`).
Its 77th default-thread run failed the related, different assertion
`the_twin_rows_are_the_two_record_formats`: the before row requested in
8.3.24 compatibility started with tag 57 instead of the expected 56.
This proves a related fixture-generation failure on unchanged base, not an
exact reproduction of the original decoder assertion.

The fixture names use compatibility label, PID and SystemTime nanos, without
a sequence. A separate pinned-Rust clock probe produced 160000 samples but
only 1303 distinct instants, all microsecond-aligned. Compatibility loading
also defaults to the dialect's version when its Configuration.xml cannot be
read or parsed. These establish a plausible risk, not its causation here.
A bounded 200-run diagnostic with temporary stderr-only lifecycle, row-head
and error output passed. Its final trace scan found one actual overlap:
two threads allocated the identical 8.3.24 scratch path; one removed it,
and the other then observed Configuration.xml ENOENT and an ENOENT Drop.
Both rows happened to compile as tag 56 in that diagnostic run, so the
original assertion's causation is still not established. This demonstrates
the independent fixture-lifecycle risk. Assertions, cleanup and default
threading were retained; all temporary source changes were then restored
byte-for-byte before normal verification. No test was newly
ignored, relabelled or serialized, and no unrelated test workaround enters
this change. The original failure's cause remains unknown; it is not labelled
pre-existing or flaky.

Both base procedures restored the original feature revision in finally.
Twelve recovered native control commands pass; their four complete reference
tree hashes match the table above exactly, and all six isolated owned target
form files match the full native controls. The completed fresh candidate
replay and recovered full gate are recorded below.

Recovered `cargo xtask check` completes all seven steps: format, crate clippy,
root compilation, form-body, form-native, form-schema and the complete
coverage suite. All three issue-specific regressions pass, including all
49 public profiles. The physical-adapter policy guard passes without a
baseline refresh. The clean recovered iter build and its new path-bound
binary provenance are recorded below.


## Recovered candidate replay

The recovered clean build revision is `41b767059249d37172d9edadea35b6f51dff016d`;
implementation files remain byte-identical to code commit
`4873840bfadf58504dc4cf4e7dd722741adbdcd8`. Rust and cargo remain pinned 1.95.0;
the new durable build path changes binary provenance. The current candidate
version is `ibcmd-rs 0.4.1`, SHA-256
`1b5b73dcef1650cedc24d3bbe6e58bf3901d6473b722253217459664ed0b4b85`. The initial binary/hash above describe the
completed historical run, not the recovered candidate.

The recovered matrix completes all 30 commands with exit 0, including two
exact raw CF extractions; together with the twelve regenerated native
controls, 42/42 commands succeed. Both raw hashes match the original saved
records exactly. All eight full CF/file reader routes return 0; all twenty
complete target Form.xml comparisons match. Four default verified imports,
native apply and both readers again produce eight complete output trees:
124/124 files byte-equal, excluding ConfigDumpInfo.xml. All four source and
all four candidate databases retain their hashes around read-only exports.
The four complete tree hashes remain identical to the table above. There is
no --no-verify, normalization, platform fallback or source-tree mutation.

The recovered full `cargo xtask ci --keep-going` completed with exit 0 and
29 PASS, no failed or skipped local step. Workspace crates: 4708 passed,
zero failed, 89 existing ignored; root integration suites: 556 passed,
zero failed, four existing ignored; root library: 904 passed, zero failed,
seven existing ignored. Temporary-directory validation also passed. Both
record-format assertions discussed above passed in this genuine full gate.

All new files were tracked before the gate. Production, tests and baseline
files remain unchanged from code commit 4873840b; only evidence Markdown has
been updated since the clean binary build. No temporary diagnostic source
remains; no unrelated assertion, fixture lifetime or test threading was
changed to obtain this result. The original source-tree limitations above
remain explicit. Fresh-context independent review is APPROVE with no actionable findings.
The reviewer independently passed check7, strict round-trip4, stored rows4,
physical policy and whitespace validation; both new complete-chain tests
failed as intended in isolated scratch code with previous production logic.
Its fresh native replay passes30 commands,20 complete target forms and124
complete-tree files with verified imports/native applies and stable read
hashes. The coordinator read the production diff and independently reran
all three deep-binding regressions (49 public profiles included):three PASS.
All reviewer/parent processes are terminal.
