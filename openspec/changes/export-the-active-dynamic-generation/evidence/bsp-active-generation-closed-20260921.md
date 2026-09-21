# BSP with an active dynamic generation: closed, 21.09.2026

Read-only measurement against `ibcmd_rs_bsp_8327_native_20260919` on
localhost, platform 8.3.27.2214, compared with the native export in
`E:\ibcmd_lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_bsp_recheck\native`.

Run: `F:\ibcmd\lab\bsp_check_r2_20260921`
(`mssql-dump-config --extract-metadata-xml --extract-module-text
--no-binary-rows`, then `source-diff`).

| | before | after |
|---|---|---|
| byte-identical | 12 193 | **12 198** |
| different | 5 | **0** |
| native-only | 0 | 0 |
| candidate-only | 0 | 0 |

The five were `CommonModules/_ДемоЗаметки/Ext/Module.bsl`,
`CommonForms/_ДемоПримечание/Ext/Form.xml` with its form module,
`ConfigDumpInfo.xml` and `Ext/ParentConfigurations.bin`.

## What the database holds

```
Config.DynamicallyUpdated = {1,1,06cb0442-0c47-4fad-986a-f08f28287c1b}
```

Five rows carry that generation: the two bodies and headers of
`CommonForm._ДемоПримечание` (`a627e390-…`) and `CommonModule._ДемоЗаметки`
(`ab132638-…`), and `versions_dynupdate_06cb0442-…`. An earlier load test of
this project performed that online update.

`Ext/ParentConfigurations.bin` was the fifth difference for a second reason:
the configuration root's own uuid is claimed by both the plain row and the
aliased one, and the collision rule withheld the path from the output rather
than pick one. With the alias published under the plain name there is one
claimant again, and the file is written.

## No change where no generation is active

ERP УХ `ibcmd_rs_uha_8327_parity2_20260920` carries 156 rows under the two
*superseded* generations `15bcc426-54ca-410a-9543-768987b832ac` and
`17894f1a-0404-4132-9792-15816a396671`, and no `DynamicallyUpdated` record at
all. Re-measured after this change (`F:\ibcmd\lab\uha_collect_r21_20260921`):

| | files |
|---|---|
| byte-identical | 140 709 |
| different | 0 |
| native-only | 0 |
| candidate-only | 0 |

— the same numbers as before it. A table whose row inventory has no
`DynamicallyUpdated` row installs no overlay, runs no extra query and builds
every statement on the bare table name, exactly as it did before.
