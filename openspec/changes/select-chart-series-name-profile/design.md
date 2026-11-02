# Design

## Contract

`MoxelChartSeriesAutomaticName` is the existing typed representation policy. Expose its three CLI values `stored`, `russian` and `english`, with Stored as the default. Russian selects the public automatic label `Сводная`; English selects `Pivot`. The localized item identifier is unchanged. This is publication representation, not translation of BSL, ScriptVariant, user text or the program UI.

Pass the value as an ordinary per-export argument into source-asset rendering, including nested Gantt chart wrappers. No mutable global, thread-local override, environment or profile inference is used. Old decoder/compiler entrypoints remain Stored so source validation and caches do not depend on presentation policy.

## Eligibility and retained evidence

The retained 15-node observation (SHA256 `4d7c36cd6f6b9e22436b5e17fe9ceb625b98e892f1c5375bfba6ebd075b7ab55`) found all names unchanged, 14 Auto palettes and one Palette8. Palette kind and palette-description child agree. This identifies the derived Rust palette guard, not a separate raw flag. Native English publication includes the Palette8 mandatory extra-series. Public native evidence also includes zero-real-series English publication. Therefore explicit mandatory extra-series publication excludes neither case. User `strIsChanged` remains authoritative. Existing real-series eligibility remains unchanged; no broader native rule is inferred.

Global `isAutoSeriesName` is true on 4 and false on 11 target nodes; it does not select the observed language. Existing native commands have no explicit language/settings flags. Native language causality remains unestablished. The explicit option replaces guessing; selecting English does not prove automatic parity for other native installations.

## Refusals and compatibility

The default produces existing Stored bytes. Unsupported CLI values fail during parsing. Non-Stored with base/sync is rejected before output because incremental inventories do not encode the presentation policy; silently retaining old differently formatted files would be incorrect. Unsupported extension overrides are also refused. Fresh full exports avoid mixing policies. Memory/editor reads and compiler source-roundtrips remain unchanged. No native recapture is required for comparison against the five frozen references.
