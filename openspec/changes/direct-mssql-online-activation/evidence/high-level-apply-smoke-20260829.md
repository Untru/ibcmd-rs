# High-level source apply smoke, 2026-08-29

## Environment

- Platform: 1C 8.3.27.2214.
- SQL Server: localhost; disposable clones only.
- Extension database: ibcmd_rs_activation_ext_20260829.
- Main databases: ibcmd_rs_activation_main_20260829 and ibcmd_rs_apply_main_fix_20260829.
- Native ibcmd/config commands were not invoked by the implementation.

## Extension module

Target: _ДемоРасширение / CommonModules/ОтчетыКлиентПереопределяемый/Ext/Module.bsl.

- Dry-run returned executed=false and exactly one changed path.
- ConfigCASSave stayed at count=0, bytes=0, checksum=NULL before and after dry-run.
- The first live attempt exposed a real defect: compiled storage payloads were deflated twice and the selected .0 row became opaque.
- After removing the second deflate, the selected CAS row is supported and re-exports as Module.bsl.
- Online high-level activation changed root 8498e62c813e7e1398b55e6b485eed133a80b69d -> 18d74730835624845bdc4288ff3858fc454cd129.
- Re-exported target SHA-256 equals the requested source: 57F6B602B9DBBB124D6ABA78FD82DC3905F6E677B37324BA1E53BB886DD910CC.
- All 156 non-target exported files have identical relative paths and SHA-256 before/after.
- Touched tables reported and observed: ConfigCASSave, ConfigCAS, _ExtensionsInfo.

## Main module

- DynamicallyUpdated is now classified as a service row, so it no longer fails the versions object inventory check.
- High-level main dry-run remains fail-closed because the BSP export reports unrelated incomplete source-asset candidates.
- The global require_complete_source_assets gate was not weakened.
- Therefore task 6 must remain open for main until a bounded selected-owner export is implemented or the safety policy is explicitly changed.

## Tests and build

- Focused extension load tests: 4 passed.
- Focused extension activation tests: 5 passed.
- CLI tests: 56 passed after adding the high-level parser case.
- DynamicallyUpdated inventory regression test: passed.
- Full suite: 2536 passed, 16 pre-existing form-decoder tests failed; one failure reproduces in isolation.
- cargo build --release --locked: passed.

## Remaining closure blockers

- Bounded selected-owner active export for high-level main apply.
- Full native XML parity for extension-only properties/adopted mappings remains false in mssql-dump-extension.
- Main-form and extension-form high-level live smoke.
- Explicit SQL-auth/trust review for the legacy main staging path.
- Existing unrelated red form-decoder regression tests.
