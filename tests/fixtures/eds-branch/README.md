# Branch cross-check fixture

Copied from `feat/rea-form-property-inventory-20261022`
(commit `4356b7eb744e0738123f2bb42ab952b348725164`):

- `external_data_source_table_row.txt` — row of
  `ExternalDataSource.ВнешнийИсточникДанных1.Table.oc_product_to_1c`
  (ServiceDesk 3.10.28, 8.5.1.1529), with non-empty `KeyFields` /
  `InputByString`, fields `product_id` (`N`) and `_c_id` (`S`);
- `external_data_source_row.txt` — the parent external-source row.

Provenance and measurements: the branch file
`docs/evidence/rea-external-data-source-20261022/README.md`.

In this repository the rows are used only by
`metadata_model::external_data_source_children::tests` to confirm the
`KeyFields`/`InputByString` typed-list parsing and the presence of both field
type codes. The matrix and stepwise corpora remain the byte-parity oracle;
the branch does not cover `Cube`/`DimensionTable` and leaves
`TableType`/`TableDataType` unknown.
