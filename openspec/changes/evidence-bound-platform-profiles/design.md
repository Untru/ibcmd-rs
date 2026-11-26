# Evidence-bound platform profiles

## Decision

A platform build is admitted to a native write only by its own bundled
declaration. The CLI keeps a closed enum so an unknown string cannot be passed,
but the enum carries no policy: `require_main_write_supported` and
`require_extension_write_supported` resolve the bundled profile and demand an
explicit `supported` capability. An absent capability is treated exactly like an
explicit `unsupported` one, so adding a build without evidence cannot silently
enable writes.

`platform-8.3.27.2214` extends `platform-8.3.27.1989`. The parent keeps the
June bootstrap layout constants and the extension registry read evidence; the
child declares the build, the write capabilities and the activation, live,
worker and parity evidence measured on 2214. Provenance stays auditable because
every effective entry keeps `declared_by`.

## Live verification

The live probe keeps the exact five-table column shape check, which is the same
on every evidenced build, and then compares two observations against the values
the selected profile declares:

- `mssql.ibversion`: `IBVersion|PlatformVersionReq`;
- `mssql.config-schema.sha256`: the canonical digest of the identity row and the
  ordered column rows.

A profile that declares neither cannot be used for a write. The RAS agent build
must still equal the profile build exactly, because the SQL shape alone does not
identify a platform: 8.3.27 and 8.5.1 databases share it byte for byte.

## Ordering

Policy is cheap and local, so it runs first. An unsupported build now fails
before rac, sqlcmd, the source tree or any temporary directory is touched, and
the diagnostic names the capability instead of an external tool failure.

## Extension staging atomicity

`mssql-load-extension` may legitimately stage rows for a later native
`config apply`, so it keeps its current contract. The high-level
`mssql-apply-source-change` stages and publishes in one command, so it first
reads the registry envelope and requires the same transition the publisher will
demand. The check is read-only and runs before the first staged row.

## Verification

- Unit tests: capability resolution from the bundled registry, undeclared and
  explicitly unsupported builds, fingerprint mismatch, agent-build mismatch,
  policy-before-runtime ordering.
- Live 8.3.27.2214: a read-only dry run reports
  `verified_platform_profile = platform-8.3.27.2214`, and the loads recorded in
  `evidence/parity-8327-20260919.md` are reproduced with the new profile.
