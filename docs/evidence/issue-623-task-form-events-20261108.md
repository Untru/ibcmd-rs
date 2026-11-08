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

The initial evidence will be tracked before full CI. Clean iter provenance,
default verified imports, native apply/re-export, full staged CI and fresh
independent review remain pending. Pending, skipped or failed runs are not
counted as successful gates.
