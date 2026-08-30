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
- After explicit user approval, online apply published generation 968a0bc0-969b-4bf2-b9ef-19d0e8bf8ce4 -> ade0166e-82bd-47e6-93b0-cfaa3027d4dd in 36.963 s and produced a recovery artifact. The recovery token is intentionally not copied into this repository.
- The published alias contains the requested BSL comment. The first repeated dry-run exposed that the dynamic-alias overlay omitted the UTF-8 BOM used by canonical source exports; content was equal but byte classification was not.
- After restoring the canonical BOM on alias reads, a repeated read-only run returned no_op=true, changed_paths=[], no staging, and no activation. Both active and proposed generation were ade0166e-82bd-47e6-93b0-cfaa3027d4dd.
- Dynamic alias reads now reconstruct the complete active Form.xml and its sibling Module.bsl. XML classification compares indexed elements, attributes, and values instead of serialization whitespace while malformed XML still fails closed.
- The first live Form.xml title update exposed native 8.3.27 button/command variants 31/9 that the packer did not match, causing two buttons and one command to be appended. The next generation was not published until the exact 20-leaf duplication diff had been explained and the native variants were covered by a regression test.
- The corrected online update published generation e83af6c7-f250-41cb-8907-ae583a2d8b81 -> cb01f23c-deea-4e12-a1bb-0cba4ecbaf12 in 33.192 s. Repeating the same source returned no_op=true in 30.390 s, and the full active/proposed Form.xml leaf diff contained zero differences.
- Existing Form.xml changes are supported when every selected facet has an evidenced codec; unknown references and unsupported facets remain fail-closed.
- The 30.390 s no-op was traced to the generic exporter resolving configuration-wide form reference indexes before reading one selected form. A bounded managed-form fast path now fetches only its descriptor/body, `DynamicallyUpdated` markers, and effective aliases. It accepts the result only after an exact active-blob unpack/repack proof and otherwise falls back to the generic fail-closed exporter. The live target below is a CommonForm; owned-form path eligibility is covered by the focused regression.
- On 2026-08-30 the same 8.3.27.2214 clone published a second Form.xml title update from generation cb01f23c-deea-4e12-a1bb-0cba4ecbaf12 to 269e4f70-7d4c-4c28-b8df-7fd9a147ac76 in 3.156 s: active read 259 ms, classification 4 ms, two-row staging 2.158 s, activation 724 ms. The immediate repeated dry-run was a no-op in 253 ms: active read 235 ms and classification 11 ms. The recovery token remains outside the repository.

## Tests and build

- Focused extension load tests: 4 passed.
- Focused extension activation tests: 5 passed.
- CLI tests: 56 passed after adding the high-level parser case.
- DynamicallyUpdated inventory regression test: passed.
- Focused high-level apply tests: 6 passed, including dynamic form-module BOM reconstruction.
- Form regressions: semantic XML equality/value-change tests and native 31/9 button/command recognition passed.
- Bounded managed-form fast-path eligibility/fallback regression passed; live changed/no-op timings are recorded above.
- Full suite: 2536 passed, 16 pre-existing form-decoder tests failed; one failure reproduces in isolation.
- cargo build --release --locked: passed.

## Remaining closure blockers

- Full native XML parity for extension-only properties/adopted mappings remains false in mssql-dump-extension.
- Extension-form high-level live smoke.
- Explicit SQL-auth/trust review for the legacy main staging path.
- Existing unrelated red form-decoder regression tests.
