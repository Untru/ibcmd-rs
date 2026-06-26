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
- Storage mapping now groups observed SQL by mutation kind, stage role,
  operation family, signal, and table name.

### Target Surface

The replacement tool should eventually cover the full set of observed 1C
metadata and object families below, both in source scanning and in SQL staging
or blob packing where applicable:

- common object families: `CommonModules`, `CommonForms`, `CommonPictures`,
  `CommonTemplates`, `CommonAttributes`, `CommonCommands`, `CommandGroups`;
- simple metadata families: `Constants`, `SessionParameters`,
  `SettingsStorages`, `DefinedTypes`, `Languages`, `StyleItems`,
  `FunctionalOptions`, `FunctionalOptionsParameters`, `EventSubscriptions`,
  `HTTPServices`, `WebServices`, `ScheduledJobs`, `Subsystems`, `Roles`,
  `Tasks`, `XDTOPackages`;
- business object families: `Catalogs`, `Documents`, `InformationRegisters`,
  `AccumulationRegisters`, `AccountingRegisters`, `CalculationRegisters`,
  `ChartsOfCharacteristicTypes`, `ChartsOfAccounts`,
  `ChartsOfCalculationTypes`, `ChartsOfCalculationRegisters`,
  `DocumentJournals`, `Reports`, `DataProcessors`, `Enums`, `ExchangePlans`,
  `FilterCriteria`, `BusinessProcesses`.

For each family, the implementation should cover:

- source-folder classification;
- object identity and UUID resolution;
- owned subtrees such as `Ext`, `Forms`, `Commands`, `Templates`, `Rights`,
  `Schedule`, `CommandInterface`, `Flowchart`, and package/blob assets;
- blob packing or header patching where the platform stores generated binary
  payloads instead of plain XML.

### Remaining Work

1. Finish the source and UUID registry in `module_blob.rs`.
   - Keep the current mappings for `Constant`, `SessionParameter`,
     `SettingsStorage`, `DefinedType`, `CommonCommand`, `CommandGroup`,
     `CommonPicture`, `FunctionalOption`, `FunctionalOptionsParameter`,
     `EventSubscription`, `HTTPService`, `WebService`, `ScheduledJob`,
     `StyleItem`, `Role`, `Language`, and `XDTOPackage`.
   - Fill the remaining family-to-folder and family-to-reference mappings for
     `Catalogs`, `Documents`, `Registers`, `Charts`, `Reports`,
     `DataProcessors`, `Enums`, `ExchangePlans`, `FilterCriteria`,
     `BusinessProcesses`, `Tasks`, `Subsystems`, and the remaining owned
     children under `Forms`, `Commands`, `Templates`, `Ext`, `Rights`,
     `Schedule`, `CommandInterface`, `Flowchart`, and binary assets.
2. Add the missing blob packers and header patchers.
   - Keep the current packers for `CommonModule`, `Constant`,
     `SessionParameter`, `DefinedType`, `CommonCommand`, and `CommandGroup`.
   - Add packers for object-shaped metadata whose payload is more than a flat
     header patch, including form/template/rights-like subtrees and binary
     payload owners such as `CommonPicture`, `WebService`, `HTTPService`,
     `ScheduledJob`, `SettingsStorage`, `XDTOPackage`, and service-specific
     assets.
   - Preserve untouched fields exactly so round-trips stay stable.
3. Expand source-manifest coverage to every remaining folder layout.
   - Cover nested `Ext/` files, object-owned `Forms`, `Commands`,
     `Templates`, `Rights`, `Schedule`, `CommandInterface`, `Flowchart`,
     package binaries, and picture assets.
   - Add regressions for roots that appear both as standalone objects and as
     subtrees under another object.
4. Add stage builders for the remaining object families in `mssql.rs`.
   - Build explicit staging paths for the business-object families:
     `Catalogs`, `Documents`, `InformationRegisters`, `AccumulationRegisters`,
     `AccountingRegisters`, `CalculationRegisters`,
     `ChartsOfCharacteristicTypes`, `ChartsOfAccounts`,
     `ChartsOfCalculationTypes`, `ChartsOfCalculationRegisters`,
     `DocumentJournals`, `Reports`, `DataProcessors`, `Enums`,
     `ExchangePlans`, `FilterCriteria`, and `BusinessProcesses`.
   - Add per-family logic for the metadata-only objects that still need
     staging coverage beyond the current common-module and generic metadata
     paths.
5. Lock the write path with trace-analysis regressions for the SQL patterns
   that matter to the load experiments.
   - no-op load
   - module-body-only change
   - metadata-attribute change
   - new object insert
6. Run the four experiments against disposable databases and record the SQL
   table-touch set for each platform build we want to support.
7. Compare the SQL traces and timings against `ibcmd` and mark divergence by
   family, operation, and platform version.
8. Turn the experimental results into an implementation matrix so future
   platform upgrades only require filling specific gaps, not rediscovering the
   whole model.
9. Add performance work after correctness is stable.
   - multithreaded scan/pack stages where the data model is independent per
     object;
   - bounded worker pools for expensive source parsing;
   - explicit safety gates before any non-lab destructive write path.
   - current implementation already parallelizes source scanning and staging
     preparation with Rayon; the next step is to profile the remaining serial
     hot paths before changing the worker model.
10. Keep the compatibility matrix current for platform build, DBMS, source tree
    shape, and supported operation set.

### Delivery Order

1. Finish the family-to-folder and family-to-reference maps.
2. Add the remaining blob packers and staging builders.
3. Lock the coverage with tests and fixtures.
4. Re-run the trace experiments and compare against `ibcmd`.
5. Fill the compatibility matrix and performance notes.
6. Commit the validated changes in small atomic steps.

### Current Parallelization Notes

- `src/source.rs` already uses Rayon to scan files in parallel.
- `src/mssql.rs` already prepares common-module and metadata staging inputs in
  parallel before SQL generation.
- The remaining work is to identify which parsing or blob-building steps still
  justify a bounded worker pool rather than widening the current fan-out.
