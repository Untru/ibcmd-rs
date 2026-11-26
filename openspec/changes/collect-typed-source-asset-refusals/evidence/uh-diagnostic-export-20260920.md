# ERP UH diagnostic export, 2026-09-20

Platform 8.3.27.2214, disposable clone `ibcmd_rs_uha_8327_parity2_20260920`
restored from a `COPY_ONLY` backup of the registered `uha` infobase
(`1C:ERP. Управление холдингом` 3.3.3.3, compatibility `Version8_3_27`).
Native reference: the 140,709-file `ibcmd config export` tree taken from the
same pristine backup on 2026-09-19.

## Before this change

```text
ibcmd-rs mssql-dump-config ... --collect-all-source-asset-diagnostics
Error: failed to normalize native data-composition source asset
Reports/РегистрыНалоговогОУчета/Templates/РегистрыДанныхПрочиеДоходыИРасходы/Ext/Template.xml
```

The run stopped after 229 s with 19,393 files; 121,316 native files were never
produced, so nothing could be said about them.

## After this change

The data-composition refusal is collected, and a second classified refusal
(`standalone-content.reference-unresolved`) no longer aborts the traversal
either. The export now walks every row in 488 s and writes 140,701 files.

Raw diff against the native tree:

| | files |
|---|---:|
| byte-identical | 140,667 |
| different | 34 |
| native-only | 8 |
| custom-only | 0 |

The differing files are 22 `Form.xml`, 8 `Template.xml`, 3 metadata XML and
`Styles/Основной/Ext/Style.xml`. The eight native-only files are three
data-composition templates of `Reports/РегистрыНалоговогоУчета`, one
`Form.xml`, one metadata XML, `Ext/ParentConfigurations.bin`,
`Ext/StandaloneConfigurationContent.bin` and `ConfigDumpInfo.xml`.

## Remaining hard failure

The run still ends with a non-zero exit after the traversal:

```text
Error: ConfigDumpInfo has 4 entries without canonical routes
[0312ddd4-7ca2-4081-913f-75999b1f03fd.0, 127eceaa-f3e0-4e51-b2b4-f54ef3de6da9.1,
 7d97c576-0e03-42fc-b939-55e54afa4218.0, 9362882b-e579-4e29-9e17-00a9baf0d986.0]
```

That check runs after every row is written, so the tree above is complete. It
also prevents `manifest.json` from being written, so the collected refusal
clusters are not persisted for this run; both are tracked as follow-up work.
