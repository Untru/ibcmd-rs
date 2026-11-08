# Task form write events

Refs #623. Base `54b796f64da5daed3714acdf7810581fe0e58778`;
pinned public corpus `8d138b896f462c3053d16076cbabf0ffe5030c1c`.

## Rule and failing-first proof

The existing native event vocabulary omitted `Form` / `cfg:TaskObject` /
`BeforeWrite`, although it already held that class's `BeforeWriteAtServer`.
Both exact public task-form records store BeforeWrite as
`9cc34712-da5f-4faa-a653-343d2085fbe8`, the already known identity used by
non-document form classes. All eight event identities and handler order
agree between the saved2214 and1150 records. The fix adds only the missing
class/event row to the existing table; it does not widen unknown-class or
unknown-event fallback.

The complete eight-event table regression first returned None instead of
the expected full table. Exact public Form.xml compilation then failed on
both saved editions with `the form names an event the writer cannot place`.
Both regressions were observed before changing production. The table test
now passes, including unknown-class and unknown-name refusals. The public
regression selects Task/BeforeWrite forms by their type and event properties
and compiles all nine profiles, including combined and stored-panel sets.
The complete native crate passes43 tests; `cargo xtask check` passes all five
steps, including six coverage tests. Physical-adapter policy passes with its
baseline unchanged. No new parser index, dependency, vendor adaptation,
generated JSON edit, object-name exception or corpus gitlink is introduced.

Exact public `.0` SHA-256 before decoding:

| Saved build | SHA-256 |
|---|---|
| 8.3.27.2214 | a2e2f40c9a8431220921e9eac8df805d4bc0008e6b0be7b1f5fbd49c2dcfc698 |
| 8.5.1.1150 | 03f93c4512fb2666ec0c8b642396db91ad5545fdded950a7a12466a79c3a91dc |

## Matched corpus and round trip

Clean production-base and fixed runs each cover242 CF sets:13285 generated,
13236 native-proven,49 platform-refused;8060 equal,5039 different,137 exporter
refusals. Stored-row inventory124 sets retains7449 equal complete files on
each side with identical equality/difference/exit-code lists. No equality
baseline changes. The fix affects source compilation, not export decoding.

The base round-trip suite passes four tests with its existing known refusals.
The fixed run also passes four tests: six combined-set reports name the
resolved #623 refusal, and both previously refused class sets now compare
successfully. Exactly eight resolved #623 allowances were removed, retaining
all other expectations and existing JSON formatting/newlines. The stricter
round-trip rerun passes four tests; observed-problems passes one test.

## Native source controls

Installed8.3.27.2214 loads the exact2214 public task-form CF;
approved8.5.1.1529 loads the exact saved1150 CF. Both native create/export
pairs return0. All six application files per build remain byte-equal to the
saved oracle:12/12 comparisons, excluding generated ConfigDumpInfo.xml.
Every native call has unique --data and LANG=ru_RU.UTF-8. Databases and
outputs are disposable; the original fixture trees remain unchanged.
No fresh1150 runtime or SQL Server measurement is claimed.

## Commands and remaining gates

```text
cargo test --locked -p ibcmd-form-native writes_the_complete_native_task_form_event_table
cargo test --locked -p ibcmd-rs --test integration coverage_corpus::native_task_form_events_compile --no-default-features
cargo xtask affected
cargo xtask check
pwsh -NoProfile -File tools/validate-physical-adapter-policy.ps1
cargo test --locked -p ibcmd-rs --test integration coverage_corpus::coverage_sets_hold_their_baseline --no-default-features -- --nocapture
cargo test --locked -p ibcmd-rs --test integration coverage_storage_records:: --no-default-features -- --nocapture
cargo test --locked -p ibcmd-rs --test integration coverage_round_trip:: --no-default-features
```

## Recovered final binary and native matrix

An earlier iter invocation ended101 while external scratch deletion removed
its log and worktree. It supplied neither a valid binary nor a product-failure
diagnosis and is not a passing gate. The committed change and pinned corpus
were restored; all controls below were regenerated in protected directories.
Earlier red-first and matched-corpus results above were observed before that
scratch loss. The final staged gate reruns the fixed suites after recovery.

Clean code commit `be3cf39885eec3c1bc1f1b2888e175bfc5f6f1c9`, Rust1.95.0/macOS ARM64.
Fresh iter version `ibcmd-rs 0.4.1`, SHA-256
`595d46930fac7f10bcf700673e013d93e4cf209b8a127488a8227f90280e84ed`. Version output does not embed the commit;
the clean checkout and binary hash bind the run. Both exact public raw
fixture hashes were re-extracted and matched the table above.

Fresh source-control create/export:four commands return0, twelve application
files match their saved native oracles. Final candidate matrix:sixteen
commands return0, including both default-verified imports and both native
applies. Both audits compile four descriptors/eight rows with zero failures.
CF, fileDB, native post-apply and own post-apply exports match all six
application files on each build:48/48 complete-file comparisons, no XML or
whitespace normalization. All four matrix databases retain their hashes
around reads (six recorded native/own checks); input CF/XML remain unchanged.
No --no-verify is used.

| Runtime | Saved CF | Files per route | Equal routes | Application tree SHA-256 |
|---|---|---:|---:|---|
| 8.3.27.2214 | 8.3.27.2214 | 6 | 4/4 | 2637c473e691e7522f570211473b71f8f4855d1aa7cb61856c6032b3425be4d9 |
| 8.5.1.1529 | 8.5.1.1150 | 6 | 4/4 | dfd8e7191382c112b55f0f858bb72e5742a79c1fdbc7bb591241d3daf22635fa |

Tree hashes cover sorted relative path,NUL,fileSHA256,NUL,byte length,LF.
They exclude the generated ConfigDumpInfo.xml, measured separately below.

### Service-version control

CF/file reads also match the complete seven-file source trees. After import,
native and own exports of each same candidate database match all seven files
including ConfigDumpInfo.xml:14/14 additional direct file comparisons.
Against the original source manifest, five configVersion values differ per
build; every other byte is unchanged. These are reported differences, not
normalized whole-file equality.

An independent official native XML-import control on each build creates a
new empty database, imports the same source, applies and exports it. All
eight commands return0; all twelve application files match. It independently
changes the same five configVersion attributes and no other bytes. This
classifies the new-generation service tokens without concealing a data loss.

```text
native ibcmd infobase create --db-path=<control> --create-database --locale=ru_RU --load=<exact-public.cf> --apply --force --data=<unique>
native ibcmd infobase config export --db-path=<control> <native> --data=<unique>
ibcmd-rs audit-empty-stage <native> <empty-rows> --platform=<build> --output=<audit.json>
ibcmd-rs cf export <exact-public.cf> <cf-output> --source-version=<2.20-or-2.21>
ibcmd-rs infobase config export --db-path=<control> --platform=<build> <file-output>
native ibcmd infobase create --db-path=<candidate> --create-database --locale=ru_RU --data=<unique>
ibcmd-rs infobase config import --db-path=<candidate> --platform=<build> <native>
native ibcmd infobase config apply --db-path=<candidate> --force --data=<unique>
native ibcmd infobase config export --db-path=<candidate> <post-native> --data=<unique>
ibcmd-rs infobase config export --db-path=<candidate> --platform=<build> <post-ours>
```

Native binary SHA-256:
- 8.3.27.2214: `1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`.
- 8.5.1.1529: `40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb`.

## Final gate and review

All evidence/new tests were tracked before full CI. The recovered full gate
on `7093bd17753a870bd54818d046962a8d357a79d7` completed with terminal exit0:
29 PASS, no skipped local step. Workspace crates:4707 passed,89 existing
ignored; root integration:554 passed,4 existing ignored; root library:904
passed,7 existing ignored. Temporary-directory cleanup passed. No test was
ignored, relabelled or removed for this change. Fresh independent review
remains pending. No fresh1150 runtime, SQL Server, release build, SBOM or
release audit is claimed by these local targeted controls.
