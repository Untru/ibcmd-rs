# EDT XML Layer Notes

Date: 2026-06-30.

Local EDT XML plugins inspected:

- `C:/Users/Pavel/.p2/pool/plugins/com._1c.g5.v8.dt.md.export.xml_10.0.102.v202605141817.jar`
- `C:/Users/Pavel/.p2/pool/plugins/com._1c.g5.v8.dt.md.import.xml_6.0.1.v202605141817.jar`

## Practical Conclusion

The EDT XML layer is useful as an authoritative behavioral reference, not as a
runtime dependency for `ibcmd-rs`.

The exporter/importer classes depend on Eclipse/OSGi, EMF and EDT metadata/BM
models. They do not work directly with SQL `Config`/`ConfigSave` blobs. Calling
them from the Rust tool would require reconstructing an EDT model first and
would add a heavy Java runtime path. That conflicts with the direct-SQL
replacement goal.

## Useful Export References

The export plugin exposes these useful implementation points:

- `MetadataObjectWriter`: generic XML shape: object element with `uuid`,
  optional `InternalInfo`, `Properties`, and optional `ChildObjects`.
- `MetadataObjectFeatureOrderProvider`: authoritative feature ordering by
  metadata class.
- `ProducedTypesOrderProvider`: ordering for generated/produced type
  references.
- `ExportMdFilesSupport`: source tree file names and layout rules.
- Specific writers for standard attributes, fields, contained objects,
  exchange plan content, templates, help, pictures, forms and similar bodies.

For `ibcmd-rs`, this means the next export parity work should move remaining
manual guesses toward generated/verified order tables and source path rules
derived from these behaviors.

## Useful Import References

The import plugin exposes:

- `MdXmlFileReaderProvider`
- object readers such as `CatalogXmlFileReader`, `DocumentXmlFileReader`,
  `EnumXmlFileReader`, `ReportXmlFileReader`, `RoleXmlFileReader`,
  `TemplateXmlFileReader`
- hierarchy importers such as `MetadataObjectImporter`, `TemplateImporter`,
  `FormMetadataImporter`, `PredefinedImporter`,
  `ExchangePlanContentImporter`
- `ConfigurationImportValidator`

For `ibcmd-rs`, this maps well to the existing staging architecture:

- source tree scan
- XML reader / identity extraction
- metadata body packers
- `ConfigSave` staging
- versions blob patching

The current importer is still a SQL staging updater over an existing compatible
database, not a full bootstrap compiler for an empty infobase.

## Current Code Alignment

Implemented after this analysis:

- high-level `infobase config import` now passes the resolved source version
  into SQL staging;
- `mssql-stage-source-objects` accepts optional `--source-version`;
- `mssql-audit-source-parity` accepts optional `--source-version`;
- selected metadata/common-module XML files are preflight-checked against that
  version before staging/audit preparation;
- staging and parity reports include the checked source version;
- export tests now explicitly cover `2.21` output for form/help/template XML
  where the source version matters.

## Remaining Work

Highest-value next steps:

1. Extract or manually transcribe feature-order expectations from
   `MetadataObjectFeatureOrderProvider` into Rust order tables.
2. Compare `ExportMdFilesSupport` path decisions against `SourceAsset` layout
   in `src/mssql_dump.rs`.
3. Expand metadata XML decompile for partial families in
   `docs/export-parity-status.md`, especially `Catalogs`, `Documents`,
   `DataProcessors`, `Reports`, `Registers`, `Subsystems`, and
   `CommonAttributes`.
4. Use import readers/hierarchy importers as the architecture checklist for a
   future full source compiler. The existing staging path can continue to grow
   incrementally, but bootstrap import into an empty database is a separate
   milestone.
