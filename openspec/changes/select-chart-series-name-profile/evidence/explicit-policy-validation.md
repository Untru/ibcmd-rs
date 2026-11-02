# Explicit chart publication policy validation

## Contract and evidence

The output option is `--chart-series-names stored|russian|english`, default
Stored. It changes supported generated chart labels only, preserving localized
item identifiers, user-changed text, BSL and Configuration ScriptVariant. The
choice is explicit and operation-local, not native language autodetection.

A bounded read-only pass of five already retained Template pairs classified all
15 target extra-series names: native Pivot versus stored Russian; all paired
strIsChanged flags false. Fourteen palettes are Auto and one is Palette8, with
consistent paletteKind/description signals. The derived Rust palette guard is
therefore known, not an independently measured raw bit. isAutoSeriesName is true
on 4 and false on 11 nodes. Receipt SHA256:
`4d7c36cd6f6b9e22436b5e17fe9ceb625b98e892f1c5375bfba6ebd075b7ab55`.
Native commands recorded no explicit language/settings argument. No new native
capture, SQL write or raw XML transfer was used.

## Local regressions

The public typed Palette8 extra-series regression failed on unchanged production
(expected Pivot versus stored Russian), then passed after the correction.
Existing positive-real-series eligibility is retained. Explicit mandatory extra-
series publication does not require a real series or unnamed palette; user
strIsChanged remains authoritative.

Rust 1.95, one build job, external target/TMP and retained fixtures:

- `cargo test --locked --no-default-features --lib chart_policy_`: 4 passed.
- `... --lib explicit_english_projection_`: 4 passed, including compiler Stored
  source-roundtrip and default compatibility.
- `... --lib chart_series_names_`: 2 passed (typed/default/unknown CLI parsing).
- `... --lib changed_chart_names_`: 2 passed (drop-in/direct preflight refusals).
- `... --lib nonstored_chart_labels_`: 2 passed (incremental/raw export refusals
  before database/journal/output).

The tests compare complete output except the deliberately selected generated
contents, including Russian/English localized items and escaped user text. Full
packed decoding and nested Gantt propagation are covered. Formatting, diff check
and strict OpenSpec validation passed. Two independent source reviews found no
blocker; a test-only compatibility wrapper was cfg-gated to avoid a new production
warning. Logs and RED are retained on the external lab disk.

Production library and CLI check passed in 13.37 seconds with the unchanged 97
baseline lib warnings. Both merge and connection workflows now run the 14
focused controls; the accepted specification is in the strict OpenSpec gate.
Exact candidate CI and the five-file explicit-English runtime comparison remain
pending. No five-Template parity claim is made.

## Separate status/selected loop closed at e93

Exact product `e93c50cc8633220bfe59d7bf093f2145775eb693`, PE SHA256
`ab387b16009164ff669e092e9fdb9b776738370d38baed343eee00f8818aa0d4`,
passed automatic Windows/Linux CI run 40156669721 and serialized public runtime
acceptance: clean module Same; selected and stage module bytes equal the 43-byte
native reference; raw memory remains 38 bytes. A dirty public module is Modified,
unchanged by status/exportPlan, and its plan.expected records the exact raw SHA.
Full export remains 5/6 byte-exact with the known Template mismatch. Both product
processes exited naturally with 0 and source fingerprints remained unchanged.
Full-release receipt SHA256:
`fc426e45bf6559ff338a7e54c002e4a54a05bc35adf5a2d4b49311141b68439f`.

The lab VM did not execute editor guarded publication. Dirty publication refusal
coverage is exact-head CI unit and real Linux editor-host evidence; the direct
objects/export RPC does not itself promise that guard. No runtime dirty-refusal
claim is inferred from staging alone.
