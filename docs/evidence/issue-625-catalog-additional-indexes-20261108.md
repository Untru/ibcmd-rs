# Catalog additional-index storage mapping

Refs #625. Base54b796f64da5daed3714acdf7810581fe0e58778;
public corpus8d138b896f462c3053d16076cbabf0ffe5030c1c.

The schema owns the closed owner-family storage suffix map: Catalog1d,
Document3, AccumulationRegister4. The loader delegates to that rule; other
families retain the precise unmapped refusal. Existing index-body encoding
and standard-field rules are reused. No dependency, vendor/generated-data
change or new parser slot is introduced.

Both exact public catalog-index CFs,2214 and1150, contain the owner-relative
.1d element with identical inflated SHA-256
7f8c3e87f5a7bccdeb1a5cf047bafe718832bda78d1338a010ae67ffcf2a73cb.
Fresh installed2214 and authorized1529 create/export controls completed four
native commands with exit0. The public base-free bootstrap regression first
failed at the unmapped .1d target. After the fix both complete
AdditionalIndexes.xml files re-export byte-exactly; the existing suffix unit
also passes and retains InformationRegister rejection. The earlier limited
legacy bootstrap invocation failed for another rule and is not the red proof.

Initial affected/check completed five PASS and policy passed unchanged.
After recovery the complete public index regression and suffix unit pass
again. The staged scope retains exactly eight resolved #625 refusals less;
round-trip passes four tests, observed-problems one, final check five steps
and physical policy passes without a baseline change.

Fresh matched measurements use the same feature integration harness for
both the verified clean-base binary (54b796f64d, SHA-256
bb7d2e0495455fa85f6420d71f13575a22de8f3b1e49856741d67c0b97a38b48)
and the fixed binary (SHA-256
7281d0888da5487cd5ae13d39aa7d178775c87b697c371f05280deb8d27f3e65).
Only the product executable is exchanged inside a try/finally restoration;
no checkout source is changed. Each side passes all242 CF sets:
8060 equal,5039 different,137 exporter refusals, with identical verdict
and first-difference lists. All124 stored sets retain7449 equal files;
equality/difference/exit-code lists are identical. No equality baseline changes.

Fresh native source controls again complete four create/export commands
with exit0. Separate base audits of both native trees reproduce exactly
one unmapped AdditionalIndexes refusal each (the audit command itself returns0;
its report contains the failure). These are offline diagnostic controls,
not evidence of successful import.

## Final clean binary and native acceptance

Clean build commit85325dfcb24f7930994c7c479070986e66e6817e,
Rust1.95.0/macOS ARM64; iter SHA-256
`d871495eaeffc7392ab6927c7c92882e7ff1499931155edf3d494daf7147638c`.
The executable reports `ibcmd-rs 0.4.1`; commit binding is by the clean
checkout/hash, not an embedded commit. Both public .1d records were
re-extracted after recovery:347 bytes, the identical SHA-256 stated above.

The complete minimal class now passes both installed runtimes:
8.3.27.2214 loads its exact public2214 CF; authorized8.5.1.1529 loads the
saved1150 CF. Sixteen final workflow commands return0, including both
default-verified imports and native applies. Each audit compiles all three
descriptors/seven rows with zero failures. Four application files per build
match in four routes (CF,fileDB,native post-apply,own post-apply):32/32
complete-file comparisons without XML or whitespace normalization. Both
input CF/XML trees are unchanged; four source/candidate own-read database
hash checks retain exact bytes. No --no-verify is used, candidate PATH is empty,
LANG is ru_RU.UTF-8 and each native call has unique --data.

| Runtime | Source application-tree SHA-256 | Equal routes |
|---|---|---:|
| 8.3.27.2214 | 0a2a73e99ab5e0275a545c20da986c221577c242f15f3ad1a5faf67fbfe96975 | 4/4 |
| 8.5.1.1529 | f4d06d8dbe6d28d36923cfbb74ff805137bfeb270e9043559d58b4bea2eb1059 | 4/4 |

Tree hashes cover sorted relative path,NUL,fileSHA256,NUL,length,LF and
exclude generated ConfigDumpInfo.xml. CF/file readers additionally match
all five source files, including that manifest. Same-candidate native/own
exports match all five files per build:10/10 additional direct comparisons.
Against the original source manifest, exactly four configVersion values
change; every other byte remains identical. A separate official native
XML-import/create/apply/export control repeats eight successful commands,
compares all eight application files exactly and changes only the same four
manifest attributes per build. This classifies generation tokens; the
post-import manifests are not claimed equal to the originals. Including
source and manifest controls,28 native/candidate workflow commands return0.

```text
cargo xtask affected
cargo xtask check
cargo test --locked -p ibcmd-rs --test integration coverage_storage_records::catalog_additional_indexes_compile_and_reexport_the_complete_native_body --no-default-features
cargo test --locked -p ibcmd-rs --lib maps_additional_indexes_body_suffixes_for_load --no-default-features
cargo test --locked -p ibcmd-rs --test integration coverage_round_trip:: --no-default-features
cargo test --locked -p ibcmd-rs --test integration coverage_observed_problems:: --no-default-features
pwsh -NoProfile -File tools/validate-physical-adapter-policy.ps1
cargo build --locked --profile iter
native ibcmd infobase create --db-path=<source-copy> --create-database --locale=ru_RU --load=<public.cf> --apply --force --data=<unique>
native ibcmd infobase config export --db-path=<source-copy> <native> --data=<unique>
ibcmd-rs audit-empty-stage <native> <empty-rows> --platform=<build> --output=<audit.json>
ibcmd-rs cf export <public.cf> <cf-output> --source-version=<2.20-or-2.21>
ibcmd-rs infobase config export --db-path=<source-copy> --platform=<build> <file-output>
native ibcmd infobase create --db-path=<candidate-copy> --create-database --locale=ru_RU --data=<unique>
ibcmd-rs infobase config import --db-path=<candidate-copy> --platform=<build> <native>
native ibcmd infobase config apply --db-path=<candidate-copy> --force --data=<unique>
native ibcmd infobase config export --db-path=<candidate-copy> <post-native> --data=<unique>
ibcmd-rs infobase config export --db-path=<candidate-copy> --platform=<build> <post-ours>
```

Native binaries:2214 SHA-256
`1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25`;
1529 SHA-256
`40f74bfb8237465acc934aaea4b8d40557602bdc0ab37fc5bb389023166d2ffb`.
Full staged `cargo xtask ci --keep-going` completes with terminal exit0:
29 PASS, no failed or skipped local step. Workspace4706, root integration554
and root library904 tests pass; existing ignored89/4/7 remain unchanged.
Temporary-directory cleanup passes. All source/tests were tracked before
the gate; later edits only extend this evidence Markdown. No test was
ignored, relabelled or removed. Fresh-context independent review is APPROVE with no actionable findings.
The reviewer independently passes check5, acceptance coverage5, strict
round-trip4, suffix unit1, physical policy and diff whitespace. Its isolated
clean-base bootstrap reproduces the intended unmapped refusal for both
exact public sources without publishing a CF. A fresh native replay passes
16 commands,32 complete application-file comparisons and10 same-candidate
full files including the manifest; both audits have zero failures. All
reviewer processes are terminal and the source remains unchanged. No exact1150 runtime,
SQL Server, full combined import or local release audit claim is made.
