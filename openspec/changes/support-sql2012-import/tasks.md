## 1. Transport policy (#572)
- [x] 1.1 Add failing policy/consumer tests.
- [x] 1.2 Implement explicit immutable SQL2012 compatibility/transport policy with unchanged default.
- [x] 1.3 Verify pooled and dedicated consumers and invalid-input refusal, first-query version checks and rejection of other versions.

## 2. Native import reads (#572)
- [x] 2.1 Add failing typed-client compatibility tests.
- [x] 2.2 Replace the three native import metadata reads and preserve Tools and default/modern client JSON behavior.
- [x] 2.3 Verify empty/NULL/large-count and dynamic-generation semantics.

## 3. Acceptance (#572)
- [x] 3.1 Validate this specification and document settings/limitations.
- [ ] 3.2 Run targeted checks, normal check and merge gate with real results.
- [x] 3.3 Complete read-only target acceptance and independent review.
- [x] 3.4 Complete an authorized full import/apply benchmark with full staged-tree verification and record native database apply separately.

Local evidence: 302 selected root tests and 382 XML tests passed; all four previously refused stage-guard pairs passed. `cargo xtask check` completed five steps; physical-adapter policy and strict OpenSpec validation passed. The independent final source review approved the draft. Full `cargo xtask ci` remains unverified: the acceptance environment prohibits its file-infobase fixture operations. This is not a passing merge gate.

Observed acceptance on SQL Server 11.0.7001.0 used an existing, explicitly authorized test database. Full-folder import in auto/patch mode verified 122,550 of 122,550 files as identical and completed in 557.253 seconds. Native Designer 8.3.27.1859 `UpdateDBCfg` completed in 316.201 seconds, exit 0 / platform result 0; ConfigSave was empty and the active configuration generation changed. The operations sum was 873.454 seconds; wall time including checks between commands was 903.502 seconds. This measures an update of an existing database with unchanged rows reused, not a cold rebuild or a controlled comparison against the reported native baseline. No force or disabled verification was used. The native platform apply is measured separately from the utility's staged-tree comparison; a full post-apply native XML export was not measured.

The same guarded-import path required the separately specified MXL, DCS, panel and typed metadata/form round-trip repairs included with this change. Their format-scoped behavior is separate from the explicit SQL Server 2012 transport/query route. Public fixtures remain clean-room; private application sources and environment details are excluded. The existing-test-server acceptance does not establish compliance with upstream's disposable-copy policy; that remains a maintainer review item.
