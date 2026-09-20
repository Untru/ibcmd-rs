# Change: bind MSSQL write policy to evidenced platform profiles

## Why

The fail-closed platform gate pins native writes to the exact RAS agent build of
the selected profile, but the only 8.3 profile is `platform-8.3.27.1989` while
every activation, live and worker measurement was taken on `8.3.27.2214`. On a
workstation whose cluster runs 2214 the gate therefore rejects the whole
evidenced 8.3.27 path before any write:

```text
claimed platform profile `platform-8.3.27.1989` does not match RAS agent build `8.3.27.2214`
```

The policy itself also lived in Rust match arms while the profile JSON declared
capabilities that nothing read, so the declaration and the behaviour could drift.

A second defect breaks the atomicity promise: the high-level extension apply
stages `ConfigCASSave` rows first and only then discovers that the registry
envelope is not publishable, leaving a staged namespace behind.

## What Changes

- Add the evidenced `platform-8.3.27.2214` profile, inheriting the 8.3.27
  storage constants and declaring the capabilities its evidence supports.
- Read `mssql.main.write` and `mssql.extension.write` from the bundled profile
  instead of Rust match arms; an undeclared capability fails closed.
- Verify the live `IBVersion`/schema fingerprints against the values the profile
  declares instead of hard-coded constants.
- Check the declared policy before starting rac, sqlcmd or reading sources.
- Verify the extension registry transition before the first staged row.

## Impact

Platform profiles, the MSSQL profile gate, the high-level apply path, extension
activation and their tests. `platform-8.3.27.1989` keeps its read evidence and
now fails closed for writes, because no write protocol was ever measured on that
build.
