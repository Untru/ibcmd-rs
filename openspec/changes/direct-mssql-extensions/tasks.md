# Tasks: direct MSSQL configuration extensions

- [x] 1. Add typed `_ExtensionsInfo` registry discovery/decoding and a stable `mssql-extension-list` CLI/API with fixtures and live read evidence.
- [x] 2. Generalize MSSQL storage fetching to `ConfigCAS` / `ConfigCASSave`, resolve one extension's reachable graph, and add unit tests for isolation and malformed graphs.
- [ ] 3. Add `mssql-dump-extension` for one or all extensions, reuse the source writer, and verify native-export parity on the four `BSP_Service` extensions.
- [x] 4. Add bounded source compilation and transactional `ConfigCASSave` staging primitives with non-lab, dirty-staging, and rollback safety gates; keep activation explicitly separate.
- [x] 5. Add `mssql-load-extension` for one or all extension source trees and verify round-trip behavior on a disposable SQL clone using native `config apply` only for the activation step.
- [x] 6. Update compatibility documentation, CLI examples, and regression tests; run formatting, release build, focused tests, and the main-configuration parity smoke check.

Task 3 remains open deliberately. Selection, CAS isolation, atomic publication,
and export of all four extensions work, but native XML parity is not complete:
the ordinary configuration writer does not yet project extension-only metadata
semantics and some storage rows remain opaque. The CLI reports this explicitly
instead of claiming a complete export.
