# Select chart automatic-series-name representation

## Why

Refs #381. Five retained UT Template files contain 15 unchanged automatic extra-series names: native English publication uses `Pivot`, while stored caches contain Russian text. No reliable data/OS/build selector has been established. An explicit output representation makes this choice reproducible without claiming native autodetection.

## What changes

- Add `--chart-series-names stored|russian|english` to configuration filesystem export; default `stored`.
- Pass a typed immutable policy through the export and nested chart wrappers. Never infer it from BSL, ScriptVariant, host locale or object names.
- Explicit Russian/English publication changes generated labels only. Preserve user-changed labels and item language identifiers.
- Mandatory extra-series publication does not depend on real-series count or palette. Keep other existing eligibility behavior unchanged.
- Refuse unsupported values, non-stored incremental/base exports and unsupported extension overrides before output publication.
- Keep memory/editor/compiler source validation Stored. Record explicit policy in comparison evidence; it is not default-mode parity.
