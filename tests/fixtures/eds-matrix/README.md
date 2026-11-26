# EDS children export fixtures

Provenance of every file in this directory is recorded here so the parity
claim can be reproduced without the source workspace.

## `eds-matrix/`

Target configuration: `TestData/TEST123/РеквизитМатрица/1Cv8.cf`
(`configuration.cf` in this directory), exported by native
`ibcmd.exe 8.3.27.2214` to the platform tree used for the expected files.

| File | SHA-256 |
|---|---|
| `configuration.cf` | `E66C3863907DF1D059D891024289C4305864C74E7479007879B3D2295B6E11B8` |

Extracted bodies (inflated CF payloads) and the platform XML:

- `root.txt` / `root.xml` — `ExternalDataSource` `35f0f3eb-6bc6-455e-8200-2a8c1664122b`;
- `cube.txt` / `cube.xml` — `Cube` `959bb0a7-39af-49f7-97cd-b9702bcf6cf0`;
- `table.txt` / `table.xml` — `Table` `87f7ae12-3120-4257-92a1-11beacd7b790`;
- `dimension.txt` / `dimension.xml` — `DimensionTable` `79581bb0-b72d-43bb-b988-8f45c602139a`.

Expected paths:

```
ExternalDataSources/ВнешнийИсточникДанных1.xml
ExternalDataSources/ВнешнийИсточникДанных1/Tables/Таблица1.xml
ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1.xml
ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1/DimensionTables/ТаблицаИзмерения1.xml
```

## `eds-corpus2/`

Second independent corpus: the final step of
`TestData/TEST123/_ВнешнийИсточникДанных` (`Функция.cf`), its extracted bodies
and the native platform XML tree. Root/cube/table/dimension UUIDs:

- root `7a9885f0-eba3-4020-b10e-4f2784b179cb`;
- cube `2a643a64-766d-4f18-a3a6-718bee907cef`;
- table `ef3f4e83-a764-4ae6-9877-d962e074dcf0`;
- dimension table `59945b31-4421-4845-b868-8eefd291dc8c`.

It covers the same XML shapes without the matrix's non-empty inline
`Dimension`/`Resource`/`Field` children, so it confirms that the rules are not
specific to the matrix.

## Reproduction

```bash
cargo test --locked -p ibcmd-dump --lib mssql_dump::eds_export::tests
```

The test feeds the four stored bodies through `EdsExport::build` and asserts
byte equality with `*.xml`, including the full nested path.

A full export of the attached configuration can be reproduced with:

```bash
ibcmd-rs cf export --overwrite --platform 8.3.27.2214 \
  tests/fixtures/eds-matrix/configuration.cf /tmp/eds-matrix-out
```

The four expected files above must be byte-identical to
`tests/fixtures/eds-matrix/*.xml`; no file under `IntegrationServices/` may be
named `Куб1.xml` or `ТаблицаИзмерения1.xml`.

The native XML oracle files in eds-matrix and eds-corpus2 retain CRLF bytes
in Git via the scoped .gitattributes rules. The tests compare the complete
bytes on every OS; they do not normalize XML text or string values.

Unrendered EDS properties are admitted only at their exact schema-owned
measured defaults. Nonempty presentation data, changed unmodeled scalars,
malformed child collections and nonempty unsupported form/template/command
collections are refused rather than exported as empty/default XML.
