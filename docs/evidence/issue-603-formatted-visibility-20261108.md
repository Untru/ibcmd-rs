# Formatted document field visibility

Issue: Refs #603. Base: `54b796f64da5daed3714acdf7810581fe0e58778`.
Pinned public corpus: `8d138b896f462c3053d16076cbabf0ffe5030c1c`.

A FormattedDocumentField's conditional rights prefix uses the same exact
60-member wrapper-37 shape as the admitted document fields. The schema now
admits that field kind. The caller delegates field-kind admission to the
schema, removing a duplicate whitelist. A recognized default remains
`Some(None)`, so it cannot become the positional fallback's fabricated false.
False and role-specific payloads pass through the unchanged rights reader.
No emitter, slot index, dependency, vendor file or generated JSON changes.

## Failing first

- `cargo test --locked -p ibcmd-form-schema formatted_document_visibility -- --nocapture`:
  the new positive schema test failed (`None` instead of `Some(None)`);
  invalid wrapper/count/offset/marker boundaries stayed rejected.
- `cargo test --locked -p ibcmd-rs --test integration coverage_corpus::formatted_document_visibility_matches_native_default_and_false --no-default-features -- --nocapture`:
  the native absence was replaced by `<UserVisible>` carrying false on
  `CovFormattedRootUserVisibleTrue` before the fix.

## Portable evidence

Rust 1.95.0, macOS ARM64; canonical short `TMPDIR=/private/tmp`,
`CARGO_PROFILE_DEV_DEBUG=line-tables-only`, `CARGO_INCREMENTAL=0`.

- Both schema boundary tests pass: default, false, opaque role payload,
  existing field kinds and structural rejection cases.
- Public property regression passes twelve conditions: true/false, root/nested,
  exact native CF profiles 8.3.27.2214 / 8.5.1.1150 / 8.5.1.1529. It requires
  the formatted field to survive and compares the complete rights element,
  including rejection of an unexpected empty or duplicated rights element.
- `cargo xtask affected`: selects form-body, form-schema and coverage_corpus.
- `cargo xtask check`: 6 PASS, including the full coverage suite.
- Staged-tree `cargo xtask ci` on
  `cd0b1721a2f76492aa465471959571f309fb5cc8`: 29 PASS, no failed or skipped
  steps. Workspace crates: 4708 passed / 89 existing ignored; root integration
  suites: 554 passed / 4 existing ignored; root library: 904 passed / 7 existing
  ignored. Temporary directories are empty. Release build, SBOM and release
  audit of the offline matrix were not run.
- Physical-adapter policy: PASS, without changing its baseline. Duplicating
  the new field kind in the caller was rejected; schema-owned admission
  eliminates the duplicate rule instead.
- Full coverage measurement: PASS, no existing baseline regression. Seven
  proven root-true entries are added to CF baselines and three Form.xml
  paths to stored-row baselines. Other newly equal entries are left alone.
- `coverage_storage_records::`: 4 PASS.
- `coverage_observed_problems::`: PASS, `equal=6, known-symptom=1240`.
  Running just the six formatted-visibility witnesses also reports `equal=6`.
  The historical ledger is unchanged; the dedicated regression requires
  equality for those six conditions, rather than permitting their old symptom.

## Native evidence

Fresh native controls use 8.3.27.2214 and 8.5.1.1529. Exact 8.5.1.1150 remains
an offline CF/native oracle. All writes use new disposable infobases, every
native call has a distinct `--data`, and both sides use `LANG=ru_RU.UTF-8`.

The first controls load the pinned 2214/1150 visibility CFs into their native
platforms and export fresh references. Candidate CF and file-infobase exports
run with an empty child PATH. All sixteen UserVisible conditions agree with
native, including explicit false controls. All eight complete root target
Form.xml comparisons agree; the eight nested target files retain unrelated
companion-ID differences tracked by #596. Source `.1CD` hashes stay unchanged
after candidate exports.

Final measured code: `cd0b1721a2f76492aa465471959571f309fb5cc8` (clean checkout).
`cargo build --locked --profile iter` passed; binary reports
`ibcmd-rs 0.4.1`, SHA-256
`0c8be8bc9fac50925f7f5fe6b51d7f2fa03f017e9816efecfa3b2d444ce8c490`.
The final native replay uses matching public 2214/1529 source profiles on
2214/1529 runtimes respectively. Four native create/export commands and four
candidate CF/DB exports exit zero. All sixteen complete UserVisible values
agree and all eight full root target files agree byte for byte. All eight
nested full target files retain the documented #596 companion-ID differences.
Both source database SHA-256 values are unchanged. Candidate children again
have empty PATH. This replay supplements the earlier 1150-input/1529-runtime
compatibility control.

Commands, with fresh `<scratch>` directories:

```text
ibcmd infobase create --db-path=<scratch>/ib --create-database --locale=ru_RU --load=<public-visibility.cf> --apply --force --data=<scratch>/d1
ibcmd infobase config export --db-path=<scratch>/ib <scratch>/native --data=<scratch>/d2
ibcmd-rs cf export <public-visibility.cf> <scratch>/cf --source-version=<2.20|2.21>
ibcmd-rs infobase config export --db-path=<scratch>/ib --platform=<native-build> <scratch>/ours
```

## Acceptance limits

Whole nested forms remain unequal because of #596. No ID-normalization fix
is included. Refs #603 does not claim complete whole-form acceptance. MSSQL
and the exact 8.5.1.1150 runtime were not run.
