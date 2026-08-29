# Direct MSSQL online activation for non-structural source changes

## Goal

Apply a changed BSL module or an existing managed form directly to an MSSQL
infobase on platform 8.3.27 without invoking native `ibcmd`, both with active
users (`online`) and without them (`exclusive`). The application owns source
compilation, staging, generation publication, cache invalidation, verification,
and recovery.

## Supported boundary

The first profile is MSSQL plus the evidenced 8.3.27 storage format. Accepted
changes are existing BSL bodies and the body/layout of an existing managed
form. The command must reject additions/deletions of metadata, root object XML,
attributes, tabular sections, registers, types, configuration properties, new
forms, extension adoption mappings, and every change that can require SQL
schema restructuring.

Main-configuration staging uses `ConfigSave`; extension staging uses
`ConfigCASSave`. Runtime activation must not launch native `ibcmd`, Designer,
or another 1C configuration tool. Native behavior may be used only as a lab
oracle while deriving and validating the protocol on disposable clones.

## CLI contract

Introduce one high-level command:

```text
ibcmd-rs mssql-apply-source-change \
  --database <db> --source-root <dir> --path <source path> \
  [--extension <name>] --mode online|exclusive \
  [--dry-run] --allow-non-lab
```

The report identifies the selected logical object, active and proposed
generation/root, compiled and retained storage targets, exact SQL tables
changed, timings, activation verification, and a recovery token. An unchanged
compiled payload is a successful no-op.

## Architecture

1. **Classifier and safety gate.** Canonicalize the source path under the held
   source root, map it to one existing metadata identity, and permit only the
   supported body cohort. Compare the active storage graph and reject any
   source tree mutation outside the selected dependency closure.
2. **Compiler and staging.** Reuse the existing bounded module/form compiler.
   Produce a typed immutable activation plan from an optimistic snapshot. Main
   and extension staging remain separate adapters.
3. **Protocol evidence.** Capture row-level values, hashes, timestamps, and SQL
   traces before/during/after controlled native dynamic updates on disposable
   8.3.27 clones. Evidence must cover main module, main form, extension module,
   extension form, online and exclusive paths. No production write is allowed.
4. **Publication.** Publish a new generation in one serializable transaction.
   Main configuration promotion owns the evidenced `Config`/`ConfigSave` and
   service-row transition. Extension promotion owns new immutable `ConfigCAS`
   content, the selected extension registry root/version, and removal of only
   its staged namespace. Unknown or drifting rows abort before mutation.
5. **Online publication.** Reproduce the evidenced 8.3.27 generation transition
   while sessions remain connected. Controlled tests show that both native
   dynamic apply and the direct protocol retain the generation already loaded
   by an existing session; a session opened after publication observes the new
   code. `exclusive` uses ordinary rows and a stricter no-session preflight.
6. **Recovery.** Retain the prior generation/root and all overwritten row
   images in a bounded recovery artifact. A failed transaction changes nothing;
   a failed postcondition triggers a documented rollback command or automatic
   rollback when the evidence proves that is safe.

## Safety invariants

- Fail closed on an unknown platform/storage profile or unrecognized service
  row.
- Require an explicit non-lab acknowledgement for every write.
- Hold application locks and use serializable transactions plus exact
  optimistic row-version/blob predicates.
- Never interpolate SQLCMD variables; use UTF-8 scripts, bounded payloads, and
  explicit certificate policy.
- Do not overwrite immutable CAS content with different bytes.
- Do not activate a partial compile or a structurally changed source tree.
- Do not claim online support until a live session remains connected during
  publication and a fresh session observes the new code on the same 8.3.27
  profile.

## Verification

- Unit tests for classification, plan construction, SQL rendering, limits,
  optimistic concurrency, rollback, no-op, and wrong-profile rejection.
- Snapshot and trace fixtures for each protocol cohort.
- Integration tests on a disposable clone for main/extension module and form.
- Online tests prove the existing session is not interrupted and retains its
  loaded generation, while a fresh session receives the new generation.
  Exclusive tests prove the no-session gate and resulting source/storage parity.
- Main configuration export parity and the existing extension round-trip tests
  must remain unchanged.

## Rejected approaches

- Calling native `ibcmd config apply`: violates the runtime-independence goal.
- Blind `ConfigSave -> Config` copying: omits generation/service state and cache
  invalidation and can expose a mixed configuration.
- Supporting structural metadata in the first version: requires schema update,
  data conversion, locks, and a much larger rollback protocol.
- Advertising online mode based only on SQL row parity: server processes can
  retain old code in memory, so a live-session observation is mandatory.
