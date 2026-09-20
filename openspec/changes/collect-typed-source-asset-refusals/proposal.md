# Change: collect typed non-form source-asset refusals

## Why

`--collect-all-source-asset-diagnostics` only recovers form-writer rejections.
Every other codec aborts the whole export on its first refusal, so one
unsupported body hides the entire remaining backlog. A full ERP UH export stops
after about four minutes on a single data-composition template:

```text
failed to normalize native data-composition source asset
Reports/.../Templates/.../Ext/Template.xml
[error dcs.template-normalize.primary-schema-parse unsupported]
```

121,316 native files were never produced by that run, so nothing can be said
about them. The codec already names a stable diagnostic code and a failure
class, which is exactly the evidence the completeness report is built for.

## What Changes

- In the diagnostic mode, a refusal that carries a stable diagnostic code and a
  source-side failure class becomes a not-emitted entry and the traversal
  continues, starting with the data-composition template codec.
- The completeness report records the family, code, classification, asset path,
  row id, raw length and digest of each refused body.
- Internal invariants, unclassified failures, SQL/BCP and I/O errors stay fatal.
- The strict `--require-complete-source-assets` gate is unchanged: a report with
  any refusal is partial and still fails the command.

## Impact

`src/mssql_dump/source_assets.rs`, `src/mssql_dump/mod.rs` and their tests. The
default export mode keeps failing fast, and no partial `Template.xml` is ever
written.
