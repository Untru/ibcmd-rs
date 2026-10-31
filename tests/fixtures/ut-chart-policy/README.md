# Chart automatic-name discriminator experiment

Status: **design inputs only; no native run or expected output**. This directory
contains a standard-library generator, not a new export policy or a parity
fixture. No private XML, source blob, application name or business value is used.
The only target name literals are the previously public `Pivot` and `Сводная`.

## What is established

Sanitized receipt SHA-256
`ad631a807fe91fefa90a0e1655491ca4012cb53ad32f9deb213b1dc9a891d9fd`
reports 15 native `Pivot` / candidate `Сводная` differences in the extra-series
localized content of five spreadsheet templates (native 8.3.27.2342, XML 2.20).
All affected names have `strIsChanged=false`; three have zero real series.
Scale-element presence is mixed. The internal binary `has_extended_scales`
value was not observed. Consequently neither positive real-series count nor
XML scale presence establishes a portable gate for English republication.
These are observations of the selected changed nodes, not an exhaustive chart
inventory or a controlled causal experiment.

The public structural fixtures under
`../native-evidence/8.3.27.2214/moxel-chart-series-count-zero/` were captured on
macOS and retain stored extra-series text. Their historical expected outputs and
manifest remain unchanged. Their local shape observations do not explain the
new Windows receipt. Existing native Windows captures also retain stored names;
OS alone is insufficient. English executable resources/help and observed en-US
cultures are observational evidence only. No supported language switch or
installation-setting discriminator has yet been established.

## Source-owned inputs

Run the light, in-memory design validator from the repository root:

```sh
python3 tests/fixtures/ut-chart-policy/generate_seeds.py --check
```

`--out /absolute/new/external-lab-directory` writes the fragments and a manifest
into a new directory only; it refuses an existing path and never removes files.
There are 11 inputs: eight factorial cases, one literal control and two identical
baseline repeats. Generation accesses only three pinned public object fragments;
the manifest records their SHA-256 values and each generated fragment's hash.
Every localized content is replaced with a synthetic title, then only the target
extra-series content is set to the approved public automatic-name literal.
The real-series structural control has an empty localized text, as in its public
source. No original scaffold title is carried forward.

These are well-formed **chart object XML fragments**, not complete spreadsheet
sources, importable configurations, raw Form bodies or native expected outputs.
An operator must first establish a source-owned minimal spreadsheet/configuration
wrapper and verify native import/compile/export roundtrip. A successful generator
check is not proof of platform acceptance. Pin that wrapper and the final imported
source representation before interpreting any result. Do not insert these files
into a private application or treat candidate-generated output as an oracle.

| Axis | Levels | Controlled change |
| --- | --- | --- |
| Extra-series name state | unchanged / changed | Only `realExSeriesData/strIsChanged` |
| Source scales profile | extended / without extended | Three elements absent (`pointsScale`, `valuesScale`, `colorPaletteDescription`); `xLabelsOrientation` and `paletteKind` reset to public control values |
| Actual series count | 0 / 1 | Coherent `realSeriesCount` plus zero/one `realSeriesData` subtree |
| Name literal control | public automatic / synthetic user literal | Only target localized `content`, paired at changed=true, without-extended, count=1 |
| Repeat control | baseline / repeat 1 / repeat 2 | Identical source bytes; unchanged, without-extended, count=1 |

The scales profile is intentionally a **multi-field structural contrast**, not a
claim that any one field or the hidden binary flag changed. If a contrast affects
the result, split its five changes into individually valid native controls before
attributing causality. The count axis also changes a coherent record group;
invalid count/subtree mismatches are not useful experiments. Factorial pairs
isolate source contrasts; do not compare diagonal cases as one-variable tests.

The generator checks pinned input hashes, XML roundtrips, mandatory extra-series
presence, real-series count/subtree consistency, and flag/count edge invariants.
Its XML formatting is a source construction choice. Its internal formatting-
insensitive invariant comparison must never be reused to normalize native parity.

## Operator execution plan (pending review and allocation)

1. Use only a disposable source-owned configuration and external lab output paths.
   Record native executable/build/XML version, hashes of the configuration and
   imported spreadsheet, exact documented invocation, and observable environment.
   Keep source/import/export artifacts local; return only approved aggregate facts.
2. Verify all eight cases survive native import/compile. Read back the source
   representation so auto-repair, changed flags, counts or scales do not silently
   collapse input levels. Refuse causal classification if levels collapse.
3. Export the baseline, traverse paired contrasts, then repeat the baseline
   (A-B-A) under the same installation, process invocation and configuration state.
   Record fresh versus reused process/state explicitly; retain immutable hashes.
   The two identical repeat fragments permit separate disposable runs; identity of
   input bytes alone cannot establish independence of native internal state.
4. Observe the exact `realExSeriesData/text/item/content` path, locale tags,
   `strIsChanged`, actual series count and source scales profile. Classify only
   `Pivot`, `Сводная`, unchanged synthetic literal, or OTHER; no OTHER values may
   leave the lab. Record unexpected shape, omitted node and failed import as
   separate outcomes, never substitute a guessed name. Ordinary series text is
   a separate observation. Preserve byte and inventory diffs, without normalizing.
5. Compare the candidate under the identical source and record its exact head and
   binary hash. Native and candidate timings are secondary to verified input
   identity and exact output bytes; exclude import/seed setup from export timing.
6. Run a separate wrapper generalization control only after the spreadsheet
   result is understood. Existing public Form chart and embedded Gantt fixtures
   are different serialization paths; their behavior must not be inferred from
   a spreadsheet result or folded into this factorial matrix.

No environment change is authorized or specified here. A language/installation
experiment requires a documented supported setting, an approved operator action,
and a measurable setting value first. Then use the exact same source bytes and
native build for an A-B-A setting contrast, with disposable state and no other
changes. Do not invent CLI switches, edit system locale, infer behavior from
resource language, or select production behavior from the host OS/environment.

## Decision and overlap boundaries

A result may establish a source discriminator, require an explicit target profile,
or remain unresolved. The current evidence supports **neither a new default nor
the existing shape predicate as a universal English gate**. The proposal under
`openspec/changes/select-chart-series-name-profile/` keeps the public entrypoint
unselected. User-changed names are a required control, not a platform behavior
already measured by this experiment.

PR #521's retained public diff was inspected: it touches chart series parsing and
other Moxel formatting/compaction. No code is copied or merged here. Any eventual
integration must pin the actual parser/writer head, rerun this matrix and review
that overlap. Form marker diagnostics remain independently owned and unrelated.

Remaining evidence gaps: complete native-accepted source-owned wrapper; native
roundtrip stability of each source level; controlled name-state/source-shape
results; supported observable target setting if environment remains relevant;
independent native result review; wrapper generalization; exact-head candidate
byte/inventory parity. No private corpus rule, environment heuristic or production
fix is justified until those gaps are resolved.
