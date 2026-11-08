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
The final binary and native candidate results are recorded below. The first
full staged CI run was interrupted; a complete retry and independent review
remain pending. No pending item is counted
as passing.

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


## Final binary and native routes

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
before a clean retry. The retry and subsequent fresh-context review remain
pending. Release builds, SBOM, the release audit and MSSQL are outside
these local results.
