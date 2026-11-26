# ExternalDataSource children export: matrix parity

Date: 2026-10-23. Working tree: `feat/config-attribute-type-matrix`.
Scope: `ExternalDataSource` root, `Cube`, `Table`, `DimensionTable` in
`cf export`. Platform oracle: native `ibcmd.exe 8.3.27.2214`.

## Inputs

Target configuration (checked in at
`tests/fixtures/eds-matrix/configuration.cf`):

- `TestData/TEST123/РеквизитМатрица/1Cv8.cf`
- SHA-256 `E66C3863907DF1D059D891024289C4305864C74E7479007879B3D2295B6E11B8`

Platform XML tree: `_УДАЛИТЬ_cf_work/plat/Configuration/ExternalDataSources/`
(the expected files are checked in as `tests/fixtures/eds-matrix/*.xml`).

Second independent corpus: final step of
`TestData/TEST123/_ВнешнийИсточникДанных` (`Функция.cf`) and its native XML,
checked in under `tests/fixtures/eds-corpus2/`.

## Measured row shapes

Inflated target payload lengths and discriminators:

| Object | UUID | Code | Payload arity |
|---|---|---:|---:|
| root | `35f0f3eb-6bc6-455e-8200-2a8c1664122b` | `2` | 10 |
| Cube | `959bb0a7-39af-49f7-97cd-b9702bcf6cf0` | `0` | 27 |
| Table | `87f7ae12-3120-4257-92a1-11beacd7b790` | `11` | 52 |
| DimensionTable | `79581bb0-b72d-43bb-b988-8f45c602139a` | `0` | 33 |

Generated-type slots measured from the bodies and confirmed by the native
`InternalInfo` order:

- Cube body: Manager, RecordSet, Record, RecordKey, List, DimensionTables,
  with RecordManager after the scalar run. XML order: Manager, List,
  RecordSet, Record, RecordKey, RecordManager, DimensionTables.
- Table body: Manager, RecordSet, Record, RecordKey, Object, Ref, List, with
  RecordManager after the scalar run. XML order: Manager, Object, Ref, List,
  Record, RecordSet, RecordKey, RecordManager.
- DimensionTable body/XML: Manager, Object, Ref, List.
- Root: Manager, TablesManager, CubesManager; `ChildObjects` order is
  Table, Cube, Function.

The matrix bodies also measured `TableType=1` -> `Table` and
`TableDataType=0` -> `NonobjectData`; the second corpus carries the same
values. The opposite codes (`Expression`/`ObjectData`) are not yet measured by
a controlled load, so they are not claimed here.

## Result

The export of the attached configuration produces exactly:

```
ExternalDataSources/ВнешнийИсточникДанных1.xml
ExternalDataSources/ВнешнийИсточникДанных1/Tables/Таблица1.xml
ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1.xml
ExternalDataSources/ВнешнийИсточникДанных1/Cubes/Куб1/DimensionTables/ТаблицаИзмерения1.xml
```

All four files are byte-identical to the platform tree:

| File | SHA-256 |
|---|---|
| `root.xml` | `C507E53C20F0F7098F5C3725F41149064BE99A6FF62BC3FC11DD0ACD14CB6F55` |
| `cube.xml` | `B5CA9EF5D4C0148622E336F91ED9449B518EFDC23795C7FC6995AA23EFD38BB1` |
| `table.xml` | `2EBFBAD5309BA4B463DFFE6E8F257D8C85AA79B75D037A82ADAFF66B13C3E136` |
| `dimension.xml` | `55460DCB1327B42EE61B78200FE865F2E47286988BA69147ED2EA074AB4C475A` |

No `IntegrationServices/Куб1.xml` or
`IntegrationServices/ТаблицаИзмерения1.xml` is produced.

The second corpus passes the same assertion, including the root with its inline
`Function`, the cube, the table and the dimension table.

## Checks

```bash
cargo test --lib mssql_dump::eds_export::tests
cargo test --test cf_eds_matrix
```

Both pass in this working tree. The integration test runs the real
`cf export` over `tests/fixtures/eds-matrix/configuration.cf` and checks the
report dispositions, the four nested outputs and the absence of the two wrong
`IntegrationServices/` files.

## Scope and open points

- Property slots `NameInDataSource`, `TableType`, `TableDataType`,
  `UseStandardCommands`, `KeyFields` and `InputByString` are decoded; the
  remaining Table/Cube/Dimension properties are the measured defaults and are
  guarded by `check_row_shape` (a changed unverified slot is refused, not
  silently defaulted).
- The inverse `TableType`/`TableDataType` codes are not exercised by these two
  corpora. `TableType=0` (ServiceDesk 8.5.1) and `TableType=1` (matrix 8.3.27)
  are both the `Table` value; the `Expression` code is not measured.
- Cross-check and adopted unknowns:
  `feat/rea-form-property-inventory-20261022` independently documents Table
  code `11`, its eight generated types, the `c7075688` field collection,
  `KeyFields`/`InputByString` and N/S field types
  (`src/metadata_model/external_data_source_table.rs`,
  `docs/evidence/rea-external-data-source-20261022/README.md`). Its two rows
  are checked in under `tests/fixtures/eds-branch/` (provenance in that
  directory's README) and drive:
  - `metadata_model::external_data_source_children::tests::branch_table_row_names_key_fields_and_input_by_string`;
  - `...::branch_table_row_field_types_are_n_and_s`.
  The branch does not cover Cube/DimensionTable and leaves
  `TableType`/`TableDataType` unknown; this matrix measurement supplies the
  two observed values.
