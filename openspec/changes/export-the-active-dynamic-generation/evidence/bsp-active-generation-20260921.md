# BSP with an active dynamic generation, 21.09.2026

Read-only measurement against `ibcmd_rs_bsp_8327_native_20260919` on
localhost, platform 8.3.27.2214, with
`E:\ibcmd_lab\parity\tools\manual-parity-with-ibuser.ps1` (native export +
this export + `source-diff`, no writes).

Run directory:
`E:\ibcmd_lab\parity\ibcmd_rs_bsp_8327_native_20260919_20260921_bsp_recheck`.

## What the database holds

```
Config.DynamicallyUpdated = {1,1,06cb0442-0c47-4fad-986a-f08f28287c1b}
```

Five rows carry that generation:

| row | bytes |
|---|---|
| `a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-…` | 180 |
| `a627e390-8fad-4a95-afe6-674f54813188_dynupdate_06cb0442-….0` | 2 262 |
| `ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_06cb0442-…` | 123 |
| `ab132638-5188-470d-9432-de85f2b2c7d8_dynupdate_06cb0442-….0` | 1 533 |
| `versions_dynupdate_06cb0442-…` | 340 598 |

`a627e390-…` is `CommonForm._ДемоПримечание`, `ab132638-…` is
`CommonModule._ДемоЗаметки`. An earlier load test of this project performed
that online update.

## Result

| | files |
|---|---|
| unchanged | 12 194 |
| different | 4 |
| missing | 0 |

The four:

- `CommonModules/_ДемоЗаметки/Ext/Module.bsl`
- `CommonForms/_ДемоПримечание/Ext/Form.xml`
- `CommonForms/_ДемоПримечание/Ext/Form/Module.bsl`
- `ConfigDumpInfo.xml`

Both modules differ by exactly one line — the native export carries
`// ibcmd-rs load parity 2026-09-19 x2 online`, which the dynamic generation
added, and this export does not. `ConfigDumpInfo.xml` differs in the
`configVersion` of those two objects and in the entries the `versions` record
supplies.

## Contrast: ERP УХ

`ibcmd_rs_uha_8327_parity2_20260920` carries 156 rows under two generations,
`15bcc426-54ca-410a-9543-768987b832ac` and
`17894f1a-0404-4132-9792-15816a396671`, and **no** `DynamicallyUpdated`
record: both updates were committed and the plain rows are the configuration.
Against a native export of that database this export is at 140 703 of 140 709
files identical, with none missing, which is the state the current
`is_superseded_generation_owner` rule produces.
