# Tasks: direct MSSQL configuration extensions

- [x] 1. Add typed `_ExtensionsInfo` registry discovery/decoding and a stable `mssql-extension-list` CLI/API with fixtures and live read evidence.
- [x] 2. Generalize MSSQL storage fetching to `ConfigCAS` / `ConfigCASSave`, resolve one extension's reachable graph, and add unit tests for isolation and malformed graphs.
- [x] 3. Add `mssql-dump-extension` for one or all extensions, reuse the source writer, and verify native-export parity on the four `BSP_Service` extensions.
- [x] 4. Add bounded source compilation and transactional `ConfigCASSave` staging primitives with non-lab, dirty-staging, and rollback safety gates; keep activation explicitly separate.
- [x] 5. Add `mssql-load-extension` for one or all extension source trees and verify round-trip behavior on a disposable SQL clone using native `config apply` only for the activation step.
- [x] 6. Update compatibility documentation, CLI examples, and regression tests; run formatting, release build, focused tests, and the main-configuration parity smoke check.

Task 3 was left open until native XML parity was measured. On 2026-09-29, on a
disposable clone of the БСП 8.3.27 database with the same four extensions,
`ibcmd-rs source-diff` between the native `config export --extension=<name>`
tree and ours finds no difference in any of the 737 files (`ConfigDumpInfo.xml`
included), and the offline export of the main configuration is unchanged
(12198 of 12198 files). The differences found and how each was closed are in
`docs/extensions/parity.md`. The БСП 8.5 extensions and the load and activation
round trip of an exported extension are tracked in issue #348.
