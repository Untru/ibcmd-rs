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

## Coverage Roadmap

### Already Covered

- Source classification for `Configuration.xml`, `.bsl`, forms, templates,
  XML metadata, and binary assets.
- `Ext/` subfiles for metadata objects.
- Manifest coverage for `CommonModules`, `CommonForms`, `CommonPictures`,
  `CommonTemplates`, `CommonAttributes`, `CommandGroups`, `DocumentJournals`,
  `Reports`, `DataProcessors`, `Enums`, `ExchangePlans`, `EventSubscriptions`,
  `FilterCriteria`, `FunctionalOptions`, `FunctionalOptionsParameters`,
  `HTTPServices`, `Languages`, `ScheduledJobs`, `SessionParameters`,
  `SettingsStorages`, `StyleItems`, `Subsystems`, `Roles`,
  `CommonCommands`, `Constants`, `WebServices`, and `XDTOPackages`.
- Trace analysis enrichment for duration, rows, session metadata, object names,
  table names, and transaction boundaries.

### Remaining Work

1. Map every supported metadata family to its concrete generated type prefixes
   in `module_blob.rs`.
2. Extend reference resolution so cross-object links work for all supported
   families, not only the currently special-cased ones.
3. Add family-specific XML packers where the blob layout is not just a header
   patch.
4. Add round-trip tests for each supported family using a real XML sample and a
   synthetic base blob.
5. Add source-manifest tests for every remaining folder layout, especially
   nested `Ext/` subfiles and families that can appear both at object root and
   as service subtrees.
6. Add trace-analysis regression tests for the SQL patterns that matter to the
   load experiments:
   - no-op load
   - module-body-only change
   - metadata-attribute change
   - new object insert
7. Run the four experiments against disposable databases and record the SQL
   table touch set for each platform build we want to support.
8. Compare the SQL traces and timings against `ibcmd` and mark any remaining
   divergence by family, operation, and platform version.
9. Turn the experimental results into an implementation matrix so future
   platform upgrades only require filling specific gaps, not rediscovering the
   whole model.

### Delivery Order

1. Finish the type-prefix map and source resolution.
2. Expand patchers for the remaining blob layouts.
3. Lock the coverage with tests.
4. Re-run the trace experiments.
5. Commit the validated changes in small atomic steps.
