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

1. Finish the metadata registry in `module_blob.rs` so every supported family
   has an explicit source-folder mapping, type-prefix mapping, and UUID/source
   resolution path.
   - Keep the current families covered: `Catalogs`, `Documents`,
     `InformationRegisters`, `AccumulationRegisters`, `AccountingRegisters`,
     `CalculationRegisters`, `ChartsOfCharacteristicTypes`, `ChartsOfAccounts`,
     `ChartsOfCalculationTypes`, `ChartsOfCalculationRegisters`,
     `CommonModules`, `CommonForms`, `CommonPictures`, `CommonTemplates`,
     `CommonAttributes`, `CommandGroups`, `DocumentJournals`, `Reports`,
     `DataProcessors`, `Enums`, `ExchangePlans`, `EventSubscriptions`,
     `FilterCriteria`, `FunctionalOptions`, `FunctionalOptionsParameters`,
     `HTTPServices`, `Languages`, `ScheduledJobs`, `SessionParameters`,
     `SettingsStorages`, `StyleItems`, `Subsystems`, `Roles`,
     `CommonCommands`, `Tasks`, `Constants`, `WebServices`, and
     `XDTOPackages`.
   - Extend cross-object resolution for `CommonPicture`, `DefinedType`,
     `CommandGroup`, `CommonCommand`, `SettingsStorage`, `FunctionalOption`,
     `FunctionalOptionsParameter`, `EventSubscription`, `FilterCriterion`,
     `HTTPService`, `WebService`, `Language`, `Role`, `ScheduledJob`,
     `StyleItem`, and `XDTOPackage`.
   - Cover nested service subtrees and object-owned children, including
     `Forms`, `Commands`, `Templates`, `Ext`, `Rights`, `Schedule`,
     `CommandInterface`, `Package.bin`, `Picture.svg`, `Picture.zip`, and
     object-specific XML assets.
2. Add family-specific blob packers for the layouts that are not just header
   patches.
   - Keep the simple patchers for `Constant`, `SessionParameter`,
     `SettingsStorage`, `DefinedType`, `CommonCommand`, and `CommandGroup`.
   - Add the remaining object-shaped packers for form/template/rights-style
     subtrees and any metadata family whose binary payload is versioned or
     structurally encoded.
   - Preserve untouched fields exactly so the output stays stable across
     round-trips.
3. Add round-trip tests for every supported family.
   - Use one real XML sample from `lab/` per family.
   - Use one synthetic base blob per layout to prove header patching is
     independent of the source content.
   - Add negative tests for unsupported combinations so regressions fail fast.
4. Expand source-manifest coverage to every remaining folder layout.
   - Include nested `Ext/` subfiles, object-owned `Forms`, `Commands`, and
     `Templates` folders, service subtrees, and binary assets.
   - Add regressions for roots that can exist both as standalone objects and as
     subtrees under another object.
5. Lock the write path with trace-analysis regressions for the SQL patterns that
   matter to the load experiments:
   - no-op load
   - module-body-only change
   - metadata-attribute change
   - new object insert
6. Run the four experiments against disposable databases and record the SQL
   table-touch set for each platform build we want to support.
7. Compare the SQL traces and timings against `ibcmd` and mark the remaining
   divergence by family, operation, and platform version.
8. Turn the experimental results into an implementation matrix so future
   platform upgrades only require filling specific gaps, not rediscovering the
   whole model.
9. Add performance work after correctness is stable:
   - multithreaded scan/pack stages where the data model is independent per
     object;
   - bounded worker pools for expensive source parsing;
   - explicit safety gates before any non-lab destructive write path.
10. Keep the compatibility matrix current for platform build, DBMS, source tree
    shape, and supported operation set.

### Delivery Order

1. Finish the type-prefix map and source resolution.
2. Add the remaining blob packers.
3. Lock the coverage with tests and fixtures.
4. Re-run the trace experiments and compare against `ibcmd`.
5. Fill the compatibility matrix and performance notes.
6. Commit the validated changes in small atomic steps.
