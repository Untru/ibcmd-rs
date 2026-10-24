# Explicit English zero-series correction

The reviewed three patches were applied and the four focused Rust tests passed
after the zero-series regression first failed for the intended name mismatch.
This is a tested correction to the explicit English projection only. It does not
select that projection for default exports or fix default UT parity. The frozen
external proposal remains unchanged as a historical UNAPPLIED / UNRUN artifact.

## Scoped correction

The existing enum RepublishedInEnglish is an explicit output representation, not
a detected OS/build/source context. For a no-extended-scales chart with an
unchanged mandatory extra series, this representation must be able to publish
Pivot even at realSeriesCount 0: the public source-owned baseline actually did so.
The scoped semantic correction is:

```rust
let automatic_names_apply = !chart.has_extended_scales
    && (!chart.real_series.is_empty()
        || automatic_name == MoxelChartSeriesAutomaticName::RepublishedInEnglish);
```

Inside the ordinary real-series loop a positive count is already tautological;
only the mandatory realExSeriesData needs this zero-count correction. Default
host()/Stored output remains identical. The existing changed-name early return
and localized item's language remain unchanged. No caller automatically selects
English; the current default UT export mismatch is therefore NOT fixed by this
latent-rule correction. No public profile/CLI/editor entrypoint is added.

## Exact ownership and evidence

source.cf is the SAME 7505-byte source-owned input CF used by the actual public
Windows native baseline: SHA b2fa2d388face5e7281711d3714815cc304e12bd220f7495be15abfa0a613b8b.
It was built from the reviewed small hand-authored wrapper, synthetic metadata
UUIDs/names/title and already-public structural spreadsheet/chart sources. It was
NOT extracted from an original ERP CF, database row or private application. The
wrapper/template/manifest and candidate-compiler hashes are pinned in evidence.json.
It is an INPUT, not a fabricated native expected artifact.

Actual native 8.3.27.2342 input/output observation: source ScriptVariant English,
extra name cache Сводная, localized item ru, output Pivot, count 0, strIsChanged false,
XML scales absent. Source CF hash and four successful command phases bind that
observation. The original receipt hash f65179de7a49cdb7acad0225a1a3853bb0215b70a3404221e6db26f1e7b3c493
remains FINAL_AUDIT_INCOMPLETE because of unrelated data accounting. This correction
uses the observed NAME fact only; it claims no full native-byte/runtime/SQL/UT
acceptance, and never changes that historical status.

Known Windows Stored captures also exist (the current public renderer documents
36 series/16 files on 8.3.27.2214), alongside earlier Windows Pivot captures and
this public 8.3.27.2342 Pivot case. Windows alone cannot select behavior. Those
observations are not controlled build or ScriptVariant experiments. Stored mode
and all default/compiler behavior must stay intact, not be overwritten to fit
one newer case.

## Three-patch test-first sequence

The independently reviewed external proposal supplied these patches, applied in order:

1. 01-private-projection-threading.patch: private whole-chart explicit-mode seam,
   threading the SAME mode through series/text helpers; old predicate retained.
   Default wrapper still calls the unchanged host() => Stored. No semantic fix yet.
2. 02-public-zero-series-regression.patch: include the four unit tests from this
   directory into moxel_exact_parity_tests. They decode source.cf through the
   EXISTING bounded CF archive + MXL codecs and typed spreadsheet/drawing IR;
   no new raw slot parser or guessed positional field is introduced.
3. 03-explicit-english-zero-correction.patch: the scoped predicate above plus
   factual prose correction. User-changed guard, legacy scale exclusion and
   default/compiler Stored behavior remain unchanged.

The focused test command was run in both phases, with unchanged assertions:

```text
CARGO_BUILD_JOBS=1 cargo test --locked --no-default-features --lib explicit_english_projection_
```

With patches 1+2, source SHA256
7591ae959f0a2c79aeb487a81a2b2e1c271e686321dcf1c3d38c565fc14fdcdf:
3 passed and 1 failed. The failing test was
`explicit_english_projection_republishes_zero_series_extra_name`. Comparison of
its full actual/expected XML panic strings confirmed that the only difference
was the cached name Сводная versus Pivot, rather than a compile or parser failure.

After patch 3, source SHA256
8133ea9735d803fd7d947895ef3684e5a1bfb68a33b5488bb76cd146a4b455f2:
4 passed, 0 failed, 0 ignored; exit 0, test execution 0.02 seconds. These are
in-memory source-owned unit checks, not native execution or a performance claim.
Log and sanitized failure-summary hashes are recorded in evidence.json; raw logs
are retained externally and are not bundled with the fixture. No private inputs
were used. Formatting and broader gates are recorded separately when completed;
these focused results alone do not claim them.

The four tests cover:

- Actual zero-series input parsed from pinned source.cf; explicit English changes
  exactly the cached content to Pivot and preserves ru/other bytes. The derived
  comparison is a PROJECTION invariant, not synthesized native full XML.
- Default host()/Stored equality for the same typed chart.
- User-changed synthetic name and existing excluded-flag controls remain cached.
- The real compiler template source-roundtrip still accepts the valid cached-name
  XML and exports Stored, independent of an explicit English test call.

The extended-scale negative is a typed synthetic preservation control ONLY.
Mac Stored seeds confound representation with shape and cannot prove this scale
exclusion necessary inside English projection. Keep the predicate conservatively;
its broader native necessity remains unknown. Native raw internal flags were not
observed by the sanitized baseline. No claim is made that XML scale absence
fully identifies an internal native flag.

## Exact remaining selector gap

Moxel asset extraction currently receives bytes/object refs/generated types, not
root ScriptVariant or a native target-name policy. Its language IR also omits two
opaque storage-language flags that the existing template preservation code admits
as 0/1; XML does not publish them. Thus source syntax, storage/load origin, native
build and runtime context are still competing explanations. No OS/build/corpus
heuristic is justified.

A later controlled ScriptVariant English/Russian pair must hold chart compiled
row bytes and all other source inputs constant under the SAME native build,
OS/process invocation/state, with verified input-level readback. If the outcome
changes reproducibly, that is evidence for this source causal axis in that scope;
if not, it refutes it there. No such paired result exists yet, and no grant for it
is requested here. Hidden storage-flag/target/version questions remain separate.

Compiler mxl_native::chart uses the renderer to validate its stored source cache.
It MUST remain Stored; a global English default would reject valid Сводная XML or
regress cached outputs. The future export selector needs an explicit immutable
caller context and review; this patch intentionally does not invent that selector.
