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
- The high-level command now resolves the source owner UUID before SQL access and reads exactly the owner and body rows instead of the full BSP tree.
- No-op on CommonModules/_ДемоЗаметки selected exactly ab132638-5188-470d-9432-de85f2b2c7d8 and .0 and completed in 4.685 s before any dynamic history existed.
- Online apply published generation 719baa18-69ed-439a-8962-1de53d98e05e -> 968a0bc0-969b-4bf2-b9ef-19d0e8bf8ce4 in 5.500 s and produced a recovery artifact.
- Online publication intentionally retains ordinary Config rows and publishes generation aliases. A raw ordinary-row export therefore shows the old body; the activation SQL verifies the staged hashes under the aliases.
- The high-level reader now resolves the newest alias for the selected object across the complete Config/Params dynamic history. Repeating the applied source returned no-op in 3.058 s.
- A different module absent from the newest generation correctly fell back to its ordinary row and returned no-op in 5.185 s.

## Main form module

- CommonForms/_ДемоПримечание selected exactly a627e390-8fad-4a95-afe6-674f54813188 and .0.
- The initial selected dump exposed global form-index uncertainty caused by unrelated forms and the prior module alias. Exact selected-row, owner-identity, and emitted-path checks now admit only the evidenced target form note and reject every other target diagnostic.
- A changed Form/Module.bsl initially attempted to rebuild Form.xml and hit unsupported Form.StandardCommand.Cancel. Module-only compile now removes Form.xml from the temporary compiler tree after classification, preserving the active layout and replacing only module text.
- The resulting dry-run prepared one metadata object, one body row, one batch, and zero prepare failures. No SQL write occurred.
- A live online form-module write was requested but the external approval reviewer required a new explicit user confirmation for that mutating command, so it was not retried.
- Full Form.xml changes remain fail-closed on unsupported compiler facets.

## Tests and build

- Focused extension load tests: 4 passed.
- Focused extension activation tests: 5 passed.
- CLI tests: 56 passed after adding the high-level parser case.
- DynamicallyUpdated inventory regression test: passed.
- Full suite: 2536 passed, 16 pre-existing form-decoder tests failed; one failure reproduces in isolation.
- cargo build --release --locked: passed.

## Remaining closure blockers

- Live online main-form module write and repeated alias no-op after explicit approval.
- Full native XML parity for extension-only properties/adopted mappings remains false in mssql-dump-extension.
- Full Form.xml and extension-form high-level live smoke.
- Explicit SQL-auth/trust review for the legacy main staging path.
- Existing unrelated red form-decoder regression tests.
