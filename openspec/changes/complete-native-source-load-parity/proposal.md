# Complete native source load parity

## Why

The export side now reproduces the native 8.3.27.2214 export byte for byte on
both measured corpora — ERP УХ 3.3.3.3 at 140 709 of 140 709 files and the BSP
demo at 12 198 of 12 198. The load side does not: the source compiler stages a
form's metadata and its modules, but it does not rebuild the form *body* blob
the platform stores, so a source tree cannot be loaded back into a storage
table and re-exported unchanged.

`mssql-audit-source-parity` fails closed on the first form it cannot compile,
for example

```
unsupported Form ListSettings filter: explicit filter use is outside the
platform-evidenced cohort
```

and `audit-source-load-coverage` puts a number on the rest.

Measured 21.09.2026 against the same two native trees the export parity is
measured against:

| | ERP УХ | BSP |
|---|---|---|
| files in the tree | 140 709 | 12 198 |
| `Form.xml` bodies only partially compiled | 13 044 | 1 108 |
| bytes in those bodies | 583 839 162 | 24 848 474 |
| of those, stageable through their module | 12 216 | 1 045 |
| of those, with no stageable module | 828 | 63 |
| modules the loader supports | 29 818 of 29 818 | 2 985 of 2 985 |

Everything else the audit reports as covered: metadata XML, modules, templates
and binary bodies. The gap is the form body alone, and it is the inverse of
the reader the export side finished.

## What changes

- The form-body compiler learns the shapes the reader already proves, so a
  `Form.xml` the export writes compiles back to the blob it was read from.
- `mssql-audit-source-parity` runs to completion on both corpora.
- A full cycle — native export, load into a clone, export again — is
  byte-identical to the native export it started from.

## Impact

- `src/compiler/` (the form-body writer), `src/mssql_apply.rs` and the
  `mssql-stage-*` commands that carry a staged body.
- No change to the export path, which is measured and complete.

## Status

Not started. This records the measured gap so it is not mistaken for parity:
**export is complete on both corpora, load is not.**
