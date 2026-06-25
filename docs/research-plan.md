# Research Plan

## Principle

We are not copying executable code. We are building an independently implemented
tool by observing behavior on databases we control, starting with read-only
analysis and reproducible traces.

## Data To Capture Per Run

- Platform version and build.
- DBMS version and compatibility settings.
- Infobase creation path and configuration type.
- Exact command line used for `ibcmd` or `1cv8`.
- Source manifest before the run.
- Source manifest after any source changes.
- `profile-run` JSON result.
- `trace-analyze` JSON result.
- SQL Server Extended Events output.
- 1C technical log output.

## Initial Questions To Answer

1. Which SQL tables are touched during a no-op load?
2. Which tables are touched when only one common module changes?
3. Which tables are touched when a metadata object is added?
4. Which platform-side phases dominate total time?
5. How much of the cost is SQL execution, lock waiting, client round-trips or
   source parsing?

## Experiments

### E01: No-Op Load

Run a load when the database already matches the source tree. This reveals
baseline overhead and validation queries.

### E02: Module Body Change

Change one common module body and run the load. This should isolate text/module
storage updates without schema restructuring.

### E03: Metadata Attribute Change

Change a catalog attribute in a disposable copy. This introduces metadata
storage changes and likely database configuration update work.

### E04: New Object

Add a small catalog or data processor. This helps identify insert patterns,
UUID handling and dependent metadata records.

## Early Stop Conditions

- The observed SQL pattern changes across minor platform builds for the same
  operation.
- The platform performs non-SQL validation or cache regeneration that cannot be
  reproduced safely.
- A write experiment requires production data or a non-disposable database.

## Next Implementation Milestone

Improve the trace analyzer so it can enrich grouped queries with row count,
client session, transaction boundaries and object/table names.
