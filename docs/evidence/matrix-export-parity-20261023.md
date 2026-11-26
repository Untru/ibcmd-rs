# Matrix `1Cv8.cf` export parity

Date: 2026-10-23. Working tree: `feat/config-attribute-type-matrix`.
Platform oracle: native `ibcmd.exe 8.3.27.2214`.

Input: `TestData/TEST123/РеквизитМатрица/1Cv8.cf`
(checked in as `tests/fixtures/eds-matrix/configuration.cf`, SHA-256
`E66C3863907DF1D059D891024289C4305864C74E7479007879B3D2295B6E11B8`).

## Result

Final tree diff against `_УДАЛИТЬ_cf_work/plat/Configuration`:

```
identical 77
different 0
missing   1  (ConfigDumpInfo.xml, known exception)
extra     0
```

The ten matrix roots below are byte-identical to the platform, including their
tabular sections and nested attributes. The EDS tree is listed separately:

- `Catalogs/Справочник1.xml` — cross-object `ChoiceForm`;
- `Documents/Документ1.xml` — same;
- `BusinessProcesses/БизнесПроцесс1.xml` — cross-object choice form;
- `ChartsOfCharacteristicTypes/ПланВидовХарактеристик1.xml` — same;
- `Tasks/Задача1.xml` — empty `<AddressingDimension/>` and no root `<Addressing>`;
- `ChartsOfAccounts/ПланСчетов1.xml` — `4c7fec95-…` is the tabular-section
  collection (7 sections) and `-16` is its `LineNumber` standard attribute;
- `ExchangePlans/ПланОбмена1.xml` — tabular sections through the shared
  code-11/nested-attribute reader;
- `CalculationRegisters/РегистрРасчета1.xml` — `ScheduleLink` is accepted even
  when the register root `<Schedule>` is empty, so the full attribute/dimension
  property block is emitted;
- `HTTPServices/HTTPСервис1.xml` — the complete 18-value HTTPMethod enum and
  empty `<ChildObjects/>` for templates without methods;
- `IntegrationServices/СервисИнтеграции1.xml` — self-closing empty
  `ExternalIntegrationServiceChannelName`;
- `ExternalDataSources/...` — root, cube, table and dimension table (see
  `eds-children-export-20261023.md`).

`ConfigDumpInfo.xml` / `versions` are out of scope and treated as the known
exception, matching `docs/export-parity-status.md`.

## Checks

```bash
cargo test --lib                 # 4061 passed, 0 failed, 16 ignored
cargo test --test cf_eds_matrix  # 2 passed
cargo fmt --all --check          # clean
```

The integration test exports the attached `configuration.cf` and compares
`Catalogs`, `Documents`, `BusinessProcesses`, `ChartsOfCharacteristicTypes`,
`Tasks`, `ChartsOfAccounts`, `ExchangePlans`, `HTTPServices`,
`CalculationRegisters`, `IntegrationServices`, the two marker `Constants`
(`КнстРеквизитОтрицательные`, `КнстРеквизитРасшрПравка`) and the four EDS
files byte-for-byte; it also asserts that no false
`IntegrationServices/Куб1.xml` or `IntegrationServices/ТаблицаИзмерения1.xml`
is produced.

`cargo clippy --lib` still stops on the pre-existing
`mssql_apply.rs:1387` `never_loop` denial, not on this work.
