# Orphan characteristic fields — #626

## Result and scope

The complete `corpus-orphan-characteristics` application tree exports and
imports on 8.3.27.2214 and 8.5.1.1529. Archived native CF/XML from
8.3.27.2214 and 8.5.1.1150 remain separate byte-for-byte offline oracles.
The pinned public corpus is `8d138b896f462c3053d16076cbabf0ffe5030c1c`.
No private configuration or new corpus bytes are introduced.

A removed characteristic source leaves `from=""`, a standard-field marker
`-2`, and dangling `0:<uuid>` field identities. The former source family
cannot be inferred from the owner. The canonical model now retains the
measured marker; XML and storage writers preserve it without inventing a path.
The closed schema domain rejects other markers. Resolved fields without a
source, nil field UUIDs, missing sources for numeric markers, and numeric XML
markers attached to a resolved source remain refusals.

## Failing first and offline evidence

Before the fix, both new complete-tree regressions failed: CF export omitted
the Catalog descriptor; base-free bootstrap refused `unresolved reference -2`.
After the fix, both editions export and bootstrap/re-export all three
application files exactly. Seventy-six malformed marker/source/structure/UUID
combinations refuse bootstrap before publishing a CF. Lower-level controls cover the exact marker
domain and the existing source-bound `-2` → Catalog.Code interpretation.

| Measurement against base `54b796f64da5daed3714acdf7810581fe0e58778` | Before | After |
| --- | ---: | ---: |
| CF sets measured | 242 | 242 |
| Equal CF variants | 8,060 | 8,069 |
| Different CF variants | 5,039 | 5,030 |
| Exporter refusals | 137 | 137 |
| Stored-row sets measured | 124 | 124 |
| Equal complete stored-row files | 7,449 | 7,451 |

The full fixed-minus-base measurement used code commit `2e028962591a2f142b57fa201a365eda0f244461`. Subsequent changes tighten admission only for the new orphan-marker profile; the final merge checks rerun all baselines. The base measurement restored production files directly from the base commit
under a `try/finally` guard, ran the same corpus commands, then restored the
implementation. Only the nine newly equal OrphanCharacteristics variants and
two newly equal stored Catalog files enter equality baselines. No equal entry
was lost. Eight measured resolved #626 round-trip refusals were removed.
Existing line endings and physical-policy inventory are preserved.

Commands:

```text
cargo test --locked -p ibcmd-rs --test integration coverage_corpus::orphan_characteristics_ --no-default-features
cargo test --locked -p ibcmd-rs --test integration coverage_corpus:: --no-default-features -- --nocapture
cargo test --locked -p ibcmd-rs --test integration coverage_storage_records:: --no-default-features -- --nocapture
cargo test --locked -p ibcmd-rs --test integration coverage_round_trip:: --no-default-features -- --nocapture
cargo xtask affected
cargo xtask check
```

The focused regressions passed 5/5, the CF suite 10/10, stored-row suite 4/4,
and round-trip suite 4/4. `check` passed all ten steps. An initial sandbox
run could not bind seven fake PostgreSQL sockets; the same socket family
passed 9/9 with local IPC permission, followed by the complete green check.
No test was disabled. The physical-adapter policy guard passed unchanged.

## Native evidence

The clean iter build is code commit
`f7603a61b21be925f2009245bf00e1f0751a8f6a`, binary SHA-256
`84939399e33d584815c4fb7f6a9cea906204e057798cca9219e58b6f266e5875`.
It was built with Rust 1.95.0 using `cargo build --locked --profile iter`.
The [native receipt](orphan-characteristics-native-20261108.json) binds the
native tool versions/hashes, CF inputs, twenty command exits and all tree hashes.

For each runtime, with `LANG=ru_RU.UTF-8` and a unique native `--data` directory
per invocation:

1. Native create/load/apply of the public CF, then native export.
2. Own export from that disposable database: all four files, including
   ConfigDumpInfo.xml, equal the native output exactly.
3. Native creation of another empty disposable database, own XML import with
   default verification enabled, native apply (`--force --dynamic=disable`),
   native export, then own export of the same database.
4. The two exports of the imported database are byte-identical across all four
   files. Its three application files equal the archived oracle.
5. A separate native XML import/apply/export control produces the same three
   application files as the own import. Its ConfigDumpInfo.xml differs only
   in three `configVersion` values on each runtime; byte comparison after
   masking just those values confirms no other changes. This is diagnostic
   classification; application comparisons and same-database complete-tree
   comparisons use untouched bytes.

All twenty commands succeeded. All eight raw tree comparisons succeeded:
four complete four-file comparisons and four three-file application
comparisons. Tree SHA-256 uses sorted `path + NUL + file SHA-256 + NUL + size + LF`.
Native 8.5.1.1529 is the live runtime; no fresh 8.5.1.1150 run is claimed.
MSSQL was not exercised; database checks use the issue's disposable file-base
route. Whole private-configuration compatibility and other marker domains
are outside this result.

## Admission and merge checks

The orphan-marker profile is admitted from original XML by the existing strict
Characteristics codec before the compact DOM drops qualified attributes or
mixed-content structure, both for ordinary descriptors and extension intake.
Its marker, sources and UUID fields must be exact; nil and already known
metadata UUIDs cannot masquerade as detached identities. Older profiles retain
their existing admission boundary. No dependency edge or policy baseline changed.

Failing-first controls reproduced whitespace normalization, unknown child and
attribute loss, and nil/known UUID publication. The final clean iter binary
refuses all 66 additional actual-process controls (33 per runtime) without a CF.
All negative process controls run with an empty PATH.

A follow-up failing-first regression reproduced twelve silent publications on
the previous code: orphan blocks hidden behind duplicate Properties or owners,
foreign namespace wrappers, and unknown wrappers with or without an empty
canonical block. Detection now traverses the entire original document before
selecting an owner or Properties. A further regression reproduced publication
when the compact parser selected a final clean XML root and discarded the
preceding orphan root. The bounded detector now uses a compact forest retaining
all top-level roots; the legacy tree API keeps its original last-root selection.
Strict reading of the detected profile rejects multiple roots. Both root orders
are process-tested on both archived editions. A further EOF regression showed
that an unfinished second envelope could disappear from the compact forest.
Admission now reconstructs the entire unfinished element stack for detection,
while the legacy tree API still selects the final closed root. Seven EOF
positions, from the outer root through the marker field, are refused on both
archived editions. Strict admission requires every candidate
Characteristics block to be the unique canonical block consumed by compilation.

The permanent integration regressions exercise 76 negative combinations on
the two archived editions. No exception from strict admission has a fallback.

The initial CI attempt used an overlong TMPDIR and was superseded. Final
merge-gate results are reported against the immutable evidence commit in the
PR; no earlier run is claimed as validation of that HEAD. Test environments
use a short temporary root to satisfy Unix-domain socket length limits.
