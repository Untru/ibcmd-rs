# Effective information-register period filtering

Refs #631. Base54b796f64da5daed3714acdf7810581fe0e58778;
public corpus8d138b896f462c3053d16076cbabf0ffe5030c1c.

## Rule and failing-first proof

Database descriptor output previously copied the stored MainFilterOnPeriod
flag even for a nonperiodic register. The existing CF exporter already
normalizes it. The schema now owns the existing periodicity and write-mode
enum vocabulary and the effective rule: stored true AND periodic AND
independent writing. Unknown enum spellings refuse. Existing slot accesses
are reused; no production parser numeric index, dependency, generated JSON
or vendor change is introduced. The unrelated CF exporter is unchanged.

The public stored-descriptor regression compares complete XML for both exact
2342 and1150 descriptors. Before the fix it failed solely at false versus
true; afterwards both full files match. The schema test covers all seven
periodicities, both write modes and both stored booleans, plus unknowns.
`cargo xtask affected` and `cargo xtask check`: six PASS; physical policy
PASS without a baseline change.

## Matched corpus

Both clean-base and fixed full CF runs:242 sets,13285 generated,13236
native-proven,49 native-refused;8060 equal,5039 different,137 exporter
refusals. Stored inventory124 sets:7449 complete equal files on each side,
identical equality/difference lists. No baseline is changed. Fixed stored
suite five PASS (including the red-first regression); round-trip four PASS.

## Fresh native evidence

Code 86634691a2e3a7a6f62169c99dd29d70c37cb6ac; iter SHA-256
a8d66ccd71dedb977a9fb35e6a9875b30654113408475ebe26745477fc9e150c. Clean checkout bound the binary; version0.4.1 does
not embed the commit. Rust1.95.0/macOS ARM64, incremental off, canonical
TMPDIR=/private/tmp. Candidate processes use empty PATH. Native calls have
LANG=ru_RU.UTF-8 and unique --data; all writes target disposable databases.

Installed2214 loads the exact public2342 informationregisters CF (there is
no2214 CF for this set). Approved1529 loads the exact saved1150 CF. This is
not a fresh2342/1150 runtime claim. Native create/export:four commands exit0.
Both complete21-file trees match through both CF and fileDB exports:84/84
comparisons. Two positive controls change only the target periodicity toDay
and its MainFilterOnPeriod totrue; native create/import/apply/export:eight
commands exit0. Both candidate database exports retaintrue and match their
complete21-file native trees:42/42 comparisons. All six candidate routes
exit0, all126/126 full-file comparisons match, and all four databases retain
their hashes around reads. No SQL Server evidence is claimed.

## Final gate and review

The initial report was tracked before the full gate. Final staged CI
completed29PASS with no skipped local stage: workspace, root integration,
root library and cleanup all passed. Independent fresh-context review approved27c342d5: check six PASS,
metadata68/schema131/stored5 tests and all five coverage_corpus tests PASS.
The reviewer repeated four native exports and six candidate routes using
retained disposable controls, with fresh output directories:132/132 complete
files including ConfigDumpInfo.xml matched; all four database hashes stable.
No additional native create was claimed for that reviewer run.

## Reproduction routes and tree hashes

After decoding the exact public `configuration.cf.b64` named above, repeat
with each installed build and a fresh output/database directory for every run:

```text
native ibcmd infobase create --db-path=<copy> --create-database --locale=ru_RU --load=<exact-public.cf> --apply --force --data=<unique>
native ibcmd infobase config export --db-path=<copy> <native-output> --data=<unique>
target/iter/ibcmd-rs cf export <exact-public.cf> <candidate-cf-output> --source-version <2.20-or-2.21>
target/iter/ibcmd-rs infobase config export --db-path=<copy> --platform=<installed-build> <candidate-file-output>
```

Run both sides with LANG=ru_RU.UTF-8 and candidate PATH empty. Positive
controls clone the native XML, change only the target Periodicity toDay and
MainFilterOnPeriod totrue, then native create/import/apply/export before the
candidate read. Compare every complete file, excluding ConfigDumpInfo.xml.
Hashes cover sorted relative path,NUL,fileSHA256,NUL,byte length,LF records.

| Build | Control | Route | Files | Equal tree SHA-256 |
|---|---|---|---:|---|
| 8.3.27.2214 | original | file | 21 | 4b83aefdb9b8e73be11a5c499962a22365f90094edd0af1c037c20ff715013b3 |
| 8.3.27.2214 | original | cf | 21 | 4b83aefdb9b8e73be11a5c499962a22365f90094edd0af1c037c20ff715013b3 |
| 8.3.27.2214 | periodic | file | 21 | 17db08de585c96102a668f32142edc6ae84d2f2b1d521c082bb35cc4d47d32c4 |
| 8.5.1.1529 | original | file | 21 | 387ad1dd5916705a1d2803cc746395f97727a50fcad2b58a21eecdfbc813ee54 |
| 8.5.1.1529 | original | cf | 21 | 387ad1dd5916705a1d2803cc746395f97727a50fcad2b58a21eecdfbc813ee54 |
| 8.5.1.1529 | periodic | file | 21 | e6db0ebb752d3a663fd5bfc7b87773f03855998fd9836536c99598b11254263f |
