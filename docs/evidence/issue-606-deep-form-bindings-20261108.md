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
Final binary provenance, native candidate routes, full staged CI and
independent review are pending. No pending item is counted
as passing.

Fresh native builds are installed 8.3.27.2214 and approved 8.5.1.1529;
saved 1150 remains an honest offline oracle. Both builds have loaded and
exported the exact public deep-binding and owned-catalog CFs: eight commands,
all exit 0. For an isolated import control the owned set retains all metadata
dependencies and its three issue-target forms, removing other form entries.
Both native builds re-import/export this control: four further commands exit
0, and all six target Form.xml files remain byte-identical to the full fresh
native output. Candidate verification is a separate subsequent measurement.
All writes use disposable bases, each native call uses its own data directory
and LANG=ru_RU.UTF-8. No dependency, vendor edit, generated schema JSON edit,
platform-version heuristic, MSSQL measurement or exact1150 runtime claim.
