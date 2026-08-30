# Tasks: direct MSSQL online activation

- [ ] 1. Build bounded row/value snapshots and trace correlation for activation-relevant MSSQL tables, then capture main-module, main-form, extension-module, and extension-form oracle evidence on disposable 8.3.27 clones.
- [x] 2. Add a fail-closed source-change classifier and typed activation plan that admits only existing BSL bodies and existing managed-form bodies, detects no-ops, and rejects structural/source-closure drift.
- [x] 3. Implement and test exclusive main-configuration promotion with serializable locking, exact optimistic predicates, recovery artifacts, postcondition verification, and no native-tool invocation.
- [x] 4. Implement and test exclusive extension publication with immutable `ConfigCAS` inserts, selected-root registry transition, prefix-scoped staging cleanup, rollback, and no native-tool invocation.
- [x] 5. Derive and implement the evidenced 8.3.27 online generation/cache invalidation protocol for main configuration and extensions; prove it in an already-connected session.
- [x] 6. Add `mssql-apply-source-change` with `online`/`exclusive`, `--dry-run`, explicit trust/auth/safety flags, stable JSON reporting, recovery token, and unchanged no-op behavior.
- [ ] 7. Run independent security/correctness reviews, focused and regression tests, main/extension parity smoke tests, release build, and document compatibility, operation, and recovery.
