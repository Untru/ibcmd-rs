# Implementation and remaining acceptance

- [x] Verify official protocol revision and current published Rust SDK.
- [x] Separate MCP stdio command with six described tools and schemas.
- [x] Fixed operator policy and deny-by-default import, no model target override.
- [x] Confined bounded offline snapshots through the existing common engine.
- [x] Cancellation bridge, bounded admission and redacted tool errors.
- [x] Independent security/code review and fixes of discovered confinement issues.
- [x] Actual rmcp client: modern discovery and legacy initialize/cancellation; offline
      export/read parity with cf export; policy/target/traversal/symlink failures.
      `cargo test --test mcp_adapter --test editor_server`: 2 MCP + 6 editor tests pass.
      Log: external lab logs/issue-386-regression.log.
- [x] Integrate READ dependency 644684f2 as ea8097f5; common-file [mcp] supported.
      5 actual1CD backend + 6 editor + 2 MCP tests pass after integration.
      Log: external lab logs/issue-386-read-integration-final.log.
- [x] Integrate descriptor READ571e6a13 as7c911837; genuine MCP reads disposable
      filedb module/export/compare without mutation; native8.5.1.1592 re-export
      matches module source. 3 MCP + 6 editor + 6 filedb tests pass.
- [x] Bounded/cancellable shared filedb materialization before blob expansion,
      including repeated CONFIG blob aliases and root/table descriptor aliases.
      19 filedb unit tests + 3 MCP + 6 filedb contracts pass.
      Logs: lab logs/issue-386-budget-lib.log, logs/issue-386-budget-mcp.log.
- [x] Integrate #382 guarded staging API: validate consumed descriptor against
      pinned startup identity, recheck operator policy immediately before atomic
      publication, and propagate cancellation before any visible change.
- [x] Enforce agent-write per fixed database on every mutation and publication.
- [x] Full shared status/compare including local-only paths and selected objects
      absent from the database, with bounded cancellable source snapshots.
      Census revalidation rejects concurrent additions/renames; same-inode
      symlink root aliases are refused. Independent review: no remaining findings.
      4056 root library tests pass (15 ignored),19 filedb unit tests pass;
      3 MCP + 6 editor + 6 filedb integration tests pass. All targets checked;
      filedb all-target Clippy passes with warnings denied.
      Logs: lab logs/issue-386-lib-all-final.log,
      logs/issue-386-full-status-final.log, logs/issue-386-filedb-clippy.log.
- [x] Native acceptance: real MCP client edits disposable filedb module; external
      test orchestrator runs normal apply; native export matches source bytes.
- [ ] Parent review before PR creation. No release/merge/issue closure in this slice.

- [x] Actual shared source custody bounds: empty directories, depth, retained final
      traversal budgets and cancellation; source-onlycommit0c689f35, reviewed.
- [x] Default staging cancellation-test isolation via dependencyc35280e0.
- [x] Physical-adapter-policy guard and selftest pass without baseline edits.
- [x] OpenSpec strict validation passes after delta/scenario formatting.
- [x] Close new publication P1: held parent descriptor for actual create/link/
      rename/unlink and cleanup via253dff7f; independent combined review and
      parent engine review clear; no atomic filesystem CAS claim.
- [x] Final exact-source gates:4065 root library tests (15 ignored),6 MCP tests
      with native roundtrip,25 filedb unit+10 filedb integration,25 default
      editor/staging/filedb regressions,all-targets,strict filedb Clippy,
      physical-adapter guard+selftest and strict OpenSpec validation.
- [ ] Parent exact-head MCP policy/confirmation review before enabled-write push
      or PR. Operator TOML opt-in is the write authorization; model-supplied
      confirm/settings/target/source/auth overrides are not accepted.
