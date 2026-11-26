# Direct MSSQL configuration-extension lifecycle

## Goal

Provide the extension-oriented read/export/import behavior exposed by native
`ibcmd`, while keeping the existing direct MSSQL path independent of an
installed 1C platform.

The first supported storage profile is SQL Server databases created and served
by platform 8.3.27. Extension discovery uses `_ExtensionsInfo`; extension
content uses the content-addressed `ConfigCAS` / `ConfigCASSave` stores.

## CLI contract

- `mssql-extension-list`: return extension name, version, order, active state,
  purpose, scope, safe-mode flags, distributed-infobase flag, update timestamp,
  and active CAS root. Support human-readable output and stable JSON.
- `mssql-dump-extension --extension <name> -o <path>`: export one extension to
  hierarchical XML sources.
- `mssql-dump-extension --all-extensions -o <path>`: export every extension to
  a child directory named after the extension.
- `mssql-load-extension --extension <name> -i <path>`: compile one source tree
  and stage/publish it as the named extension.
- `mssql-load-extension --all-extensions -i <path>`: load every extension child
  directory.

Selection options are mutually exclusive and one is required. SQL passwords
must be accepted through environment-variable options, consistently with the
existing MSSQL commands.

## Architecture

1. Add an extension registry adapter that reads `_ExtensionsInfo` and decodes
   `_ExtensionZippedInfo` using bounded parsers. Unknown 8.3.27 shapes fail with
   a typed diagnostic; names and blobs are never guessed.
2. Generalize the existing storage-row fetch/export pipeline from the fixed
   `Config`/`ConfigSave` pair to a typed storage boundary. The extension
   boundary traverses only rows reachable from the selected CAS root, so two
   extensions cannot leak files into one another.
3. Reuse the existing source-asset decoder/writer after materializing the
   selected extension as a `StorageImage`. Extension-only assets such as
   `Ext/ParentConfigurations.bin` remain part of the source tree.
4. Reuse the existing source compiler for import and stage namespaced logical
   rows in `ConfigCASSave`, matching native `ibcmd config import` semantics.
   Import does not mutate `ConfigCAS` or `_ExtensionsInfo`; activation is a
   separate `config apply` operation. Existing CAS rows are never overwritten
   in place.
5. Refuse non-lab writes unless `--allow-non-lab` is explicit. Refuse ambiguous
   extension names, output collisions, unsupported registry versions, partial
   CAS graphs, and dirty `ConfigCASSave` unless an explicit replacement option
   is supplied.

## Compatibility and verification

- Unit tests cover CLI selection, registry decoding, CAS reachability,
  deterministic source paths, staging SQL, rollback, and safety gates.
- Live read verification uses all four extensions in `BSP_Service` on platform
  8.3.27.2214 and compares each direct export with native `ibcmd` using
  `source-diff`.
- Live write verification is performed only on a disposable SQL clone. For
  each tested extension: native export -> direct import into `ConfigCASSave` ->
  native `config apply` -> native export -> byte-level source comparison.
- Main-configuration behavior and its benchmark must remain unchanged.

## Non-goals

- Direct support for PostgreSQL or platform storage profiles other than the
  evidenced 8.3.27 MSSQL layout.
- Destructive testing against `BSP_Service`.
- Reimplementation of platform activation, metadata checks, schema updates,
  cache invalidation, or help-index rebuilding. Until that protocol is fully
  evidenced, activation remains the explicit native `config apply` step.
- Silent compatibility fallback to native `ibcmd` during discovery, export, or
  import staging.
