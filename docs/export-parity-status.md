# Export Parity Status

Generated from:

```powershell
target\release\ibcmd-rs.exe mssql-dump-config `
  --database ut_ibcmd `
  -o E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\ibcmd_rs_dump `
  --overwrite `
  --inflate `
  --extract-module-text `
  --extract-metadata-xml `
  --source-version 2.20 `
  --no-binary-rows `
  > E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\report.json

robocopy `
  E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\ibcmd_rs_dump `
  E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\ibcmd_rs_source_only `
  /E /XD Config_inflated Config_raw ConfigSave_inflated ConfigSave_raw `
  /XF manifest.json *.json

target\release\ibcmd-rs.exe source-diff `
  -o E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\diff_full_source_only.json `
  D:\ibcmd-rs\lab\ut_ibcmd_20260629_164647\ibcmd `
  E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\ibcmd_rs_source_only

target\release\ibcmd-rs.exe mssql-dump-timing-summary `
  E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\report.json `
  -o E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\timing-summary.json
```

The full JSON report is retained at
`E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\diff_full_source_only.json`.
The full dump timing report is retained at
`E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\report.json`, with the derived
timing summary at
`E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1\timing-summary.json`.
The latest retained DataProcessors-focused signature-mining report remains at
`E:\ibcmd_lab\selected_dp_full_20260706_0026_command_interface_release\signatures_dp_only_all.json` to rank
repeated XML path differences for the next agent batches.
The latest full DataProcessors scoped verification is retained at
`E:\ibcmd_lab\selected_dp_full_20260706_0026_command_interface_release\diff_dp_only_all.json`, generated
from `E:\ibcmd_lab\selected_dp_full_20260706_0026_command_interface_release`; this
retained run does not have a separate saved `report.json` or
`timing-summary.json`, so the timing figures below are taken from the verified
dump stdout captured during the retained run.

Reference: native `ibcmd` export from `ut_ibcmd`.
Candidate: `ibcmd-rs` full export snapshot generated from the current direct
MSSQL export path on July 3, 2026.

Direct source-only summary against the native export for the current tree:

| left_only | right_only | different | unchanged |
|---:|---:|---:|---:|
| 12499 | 0 | 8871 | 28253 |

`ConfigDumpInfo.xml` remains one deliberate `left_only` scope exclusion, but the
current tree also has broader structural drift beyond that baseline.

Overall full-snapshot readiness excluding `ConfigDumpInfo.xml`: **56.9%**
(`28253 / 49622` byte-identical files; 56.94% exact), with **21369** files still remaining.

Latest full-run timing on the current tree was captured from the release dump
that produced `E:\ibcmd_lab\full_diff_20260703_102243_zeroemptyrow1`:
`40,576` rows / `927,826,268` bytes (`~884.8 MiB`) exported in `124.419 s`
on the dump critical path (`process_rows_wall`), which is `326.12 rows/s` and
`7.11 MiB/s` overall. Raw BCP fetch took `198.186 s` (`4.46 MiB/s`).

Latest full `DataProcessors` scoped verification on the current tree used
`selected_dp_full_20260706_0026_command_interface_release`: `5,737` rows /
`39,889,431` bytes (`~38.0 MiB`) exported in `12.863 s` on the dump critical
path, which is `446.01 rows/s` and `2.96 MiB/s` overall. Raw BCP fetch took
`1.089 s` (`34.93 MiB/s`). The resulting `source-diff` summary still stands at
`1572 different / 5486 unchanged`, with `0 left-only / 0 right-only`
(`77.7274%`, `5486 / 7058`). The same retained run reports
`fetch_headers_ms=7976`, `prepare_indexes_ms=40783`,
`prepare_metadata_fetch_ms=20589`, `prepare_reference_indexes_ms=13215`,
`fetch_rows_ms=1089`, `process_rows_wall_ms=12863`, and
`source_asset_cpu_ms=58406`; the hottest source-asset buckets are
`form_xml=34033`, `child_items=13928`, and `help=10325`. Latest signature
mining on the same retained run keeps `document/rowsItem/row/c/c/f=464` in `1`
file (`РаботаСНоменклатурой/Templates/ПФ_MXL_КарточкаНоменклатуры`), with the
largest remaining color tails at `document/format/borderColor=368` and
`document/format/backColor=324`, while the leading form tails are now
`Form/Commands/Command/Picture=234`,
`Form/ChildItems/Table/CommandSet/ExcludedCommand=211`, and
`Form/CommandInterface/CommandBar/Item=140`. This rerun keeps the same
`77.7274%` top-line parity while lowering
`Form/CommandInterface/CommandBar/Item` from `195` across `25` files to `140`
across `18` files after falling back from a fixed `body.trailing[3]` lookup to
the first real form `CommandInterface` container.

Verification history:

| Object | Verification | Result |
|---|---|---|
| Round 110 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `0026_command_interface_release` keeps `DataProcessors` at `1572` different / `5486` unchanged with `0` left-only / `0` right-only after falling back from a fixed `body.trailing[3]` slot to the first real form `CommandInterface` container; this does not close additional files yet, but it reduces the `Form/CommandInterface/CommandBar/Item` tail from `195` across `25` files to `140` across `18` files and lands at `12.863 s` critical path (`446.01 rows/s`, `2.96 MiB/s`) |
| Round 109 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `1947_filter_picture_release` keeps `DataProcessors` at `1572` different / `5486` unchanged with `0` left-only / `0` right-only after remapping filter-command standard picture UUIDs back to `StdPicture.FilterCriterion`, `StdPicture.FilterByCurrentValue`, and `StdPicture.ClearFilter`; this does not close additional files yet, but it reduces the `Form/Commands/Command/Picture` tail from `248` across `97` files to `234` across `95` files and lands at `22.574 s` critical path (`254.15 rows/s`, `1.69 MiB/s`) |
| Round 108 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `1735_root_order_release` improves `DataProcessors` to `1572` different / `5486` unchanged with `0` left-only / `0` right-only after restoring native root-form `CommandBarLocation` / `CommandSet` order and moving `CustomizeForm` back into the standard dialog command order; this closes `6` more files, shifts the leading form tail to `Form/ChildItems/Table/CommandSet/ExcludedCommand=268`, and lands at `16.920 s` critical path (`339.07 rows/s`, `2.25 MiB/s`) |
| Round 107 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `144331_event_uuid_release` improves `DataProcessors` to `1578` different / `5480` unchanged with `0` left-only / `0` right-only after remapping confirmed form-event UUID aliases back to native event names, keeps the dominant template `document/rowsItem/row/c/c/f` signature at `464` in `1` file, preserves the leading form tail at nested `UsualGroup/.../Width=307`, and lands at `28.909 s` critical path (`198.45 rows/s`, `1.32 MiB/s`) |
| Round 106 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `141710_readme_refresh_release` reconfirms `DataProcessors` at `1580` different / `5478` unchanged with `0` left-only / `0` right-only after a full current-release rerun, keeps the dominant template `document/rowsItem/row/c/c/f` signature at `464` in `1` file, shifts the leading form tail to nested `UsualGroup/.../Width=307`, and lands at `28.129 s` critical path (`203.95 rows/s`, `1.35 MiB/s`) |
| Round 105 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `122017_purchase_line_release` improves `DataProcessors` to `1580` different / `5478` unchanged with `0` left-only / `0` right-only after remapping leading `f527` palette-offset MXL style refs and expanding the two-entry `{None, Solid}` line table into the native three-width solid set, which closes `ОбменСБанками/Templates/ПоручениеНаПокупкуВалюты/Ext/Template.xml`, `ОбменСБанками/Templates/ПоручениеНаПереводВалюты/Ext/Template.xml`, and `ОбменСБанками/Templates/РаспоряжениеНаОбязательнуюПродажуВалюты/Ext/Template.xml`, keeps the dominant template `document/rowsItem/row/c/c/f` signature at `464` in `1` file, and lands at `29.071 s` critical path (`197.34 rows/s`, `1.31 MiB/s`) |
| Round 104 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `110226_slotfix_release` improves `DataProcessors` to `1583` different / `5475` unchanged with `0` left-only / `0` right-only after resolving MXL style-ref slots `{-1}` / `{-3}` to `FormBackColor` / `FormTextColor` instead of dropping them to compact-color fallback, which removes the broad `ToolTipBackColor` misclassification tail, keeps the dominant template `document/rowsItem/row/c/c/f` signature at `464` in `1` file, and lands at `12.943 s` critical path (`443.25 rows/s`, `2.94 MiB/s`) |
| Round 103 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `101148_eisheader_release` improves `DataProcessors` to `1584` different / `5474` unchanged with `0` left-only / `0` right-only after making single-set sparse MXL header/footer refs prefer the explicit external source slot ahead of the `font=0,width=72` sparse-default heuristic, which closes `ЭлектронноеАктированиеЕИС/Templates/ЭД_ПриложениеЕИС_ru/Ext/Template.xml`, drops the dominant template `document/rowsItem/row/c/c/f` signature from `872` across `2` files to `464` in `1` file, and lands at `11.219 s` critical path (`511.36 rows/s`, `3.39 MiB/s`) |
| Round 102 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `093104_contracttypical_release` improves `DataProcessors` to `1585` different / `5473` unchanged with `0` left-only / `0` right-only after restoring single-set shared-empty MXL default handling without reopening the `UKD` / `СоответствияПараметровИМеток` cases, adding a new baseline regression for `ФорматДоговорнойДокумент101XML/Templates/ПереченьТиповыхНаименованийЭлементовДоговоров/Ext/Template.xml`, and closing `ОбменСКонтрагентами/Templates/УКД_ИнформацияПокупателя_2020/Ext/Template.xml` plus `ФорматДоговорнойДокумент101XML/Templates/СоответствияПараметровИМеток/Ext/Template.xml`; latest signature mining now concentrates `document/rowsItem/row/c/c/f` at `872` across `2` files and lands at `19.508 s` critical path (`294.08 rows/s`, `2.05 MiB/s`) |
| Round 101 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `080819_true_scope_release` improves `DataProcessors` to `1587` different / `5471` unchanged with `0` left-only / `0` right-only after batching `sqlcmd` row-header fetches to avoid SQL Server `8623`, resolving sparse shared-header/default MXL slots through explicit source refs only when no native sparse-default format exists, and suppressing the shifted row-level `formatIndex=2` tails that appear after the leading shared default reorder; this closes `ПечатьОтчетовПоКомиссии/Templates/ПФ_MXL_ОтчетПоКомиссии/Ext/Template.xml`, `ПечатьОтчетовПоКомиссии/Templates/ПФ_MXL_ОтчетПоКомиссииОСписании/Ext/Template.xml`, and `ФорматДоговорнойДокумент101XML/Templates/ПереченьТиповыхНаименованийЭлементовДоговоров/Ext/Template.xml`, drops the dominant template `document/rowsItem/row/c/c/f` signature from `1041` (`4` files) to `594` (`3` files), and lands at `11.498 s` critical path (`498.96 rows/s`, `3.31 MiB/s`) |
| Round 100 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `055344_compactstyle_release` improves `DataProcessors` to `1592` different / `5466` unchanged with `0` left-only / `0` right-only after teaching compact MOXEL style-ref indices to resolve directly to tooltip/form/field colors when the style-ref table omits those slots; this closes the remaining invoice template tails in `ПФ_MXL_СчетФактура451_ru`, `ПФ_MXL_СчетФактура981_ru`, `ПФ_MXL_СчетФактура1137_ru`, and `ПФ_MXL_СчетФактура1137_625_ru`, and lands at `12.925 s` critical path (`443.87 rows/s`, `2.94 MiB/s`) |
| Round 99 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `052811_exactslot_release` improves `DataProcessors` to `1603` different / `5455` unchanged with `0` left-only / `0` right-only after reading declared sparse-sheet height from the live MXL column-set block, restoring leading `{129,0,width}` default-width detection for sparse source-slot templates without reclassifying the whole width table, and preferring exact existing `font=0 + width` default-format matches before normalized width-only fallbacks; this closes two more files on top of the previous retained run, fixes the `СчетФактура451` `defaultFormatIndex=102` regression while preserving the `1096` and `request_offer` native baselines, and leaves the three invoice tails narrowed to the remaining drawing `backColor=style:FieldBackColor` gap plus their paired native `font=0` default-slot exactness checks; latest release timing is `11.417 s` critical path (`502.50 rows/s`, `3.33 MiB/s`) |
| Round 98 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `023103_zero_offset_sparse_release` improves `DataProcessors` to `1609` different / `5449` unchanged with `0` left-only / `0` right-only after detecting zero-offset sparse MXL source-format refs, splitting the format table by real source refs, remapping row/cell refs on that path, and recomputing the fallback default-format slot after remap; this closes `ОбменСКонтрагентами/Templates/ПользовательскоеПредставлениеОбязательныхПолей/Ext/Template.xml`, drops the dominant `document/rowsItem/row/c/c/f` signature from `2029` to `1679` (`9 -> 7` files), and lands at `16.033 s` critical path (`357.82 rows/s`, `2.37 MiB/s`) |
| Round 97 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `225500_zero_colfmt_release` improves `DataProcessors` to `1610` different / `5448` unchanged with `0` left-only / `0` right-only after reusing matching MXL default formats instead of always appending a synthetic tail format and preserving zero-valued MXL column `formatIndex`; this closes `5` more files on top of the previous retained run, drops the dominant `document/rowsItem/row/c/c/f` signature from `3001` to `2857` (`17 -> 9` files), and collapses `ЗаявлениеНаВыпускНовогоКвалифицированногоСертификата/Templates/ЮридическоеЛицо/Ext/Template.xml` from `143` differences to `25`; latest release timing is `15.075 s` critical path (`380.56 rows/s`, `2.52 MiB/s`) |
| Round 96 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `011557_command_no_action_release` reconfirms `DataProcessors` at `1615` different / `5443` unchanged with `0` left-only / `0` right-only after keeping form `Command` entries with empty `Action` and omitting empty `<Action>` tags; this does not close additional files yet, but narrows `Form/Commands/Command/Picture` from `269` to `252` and trims representative form diffs such as `БизнесСеть/ПрофильУчастника` (`24 -> 18`) and `БизнесСеть/ОтправкаПриглашенийКонтрагентам` (`116 -> 49`); latest release timing is `12.438 s` critical path (`461.25 rows/s`, `3.06 MiB/s`) |
| Round 95 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `010452_change_row2_release` improves `DataProcessors` to `1615` different / `5443` unchanged with `0` left-only / `0` right-only after refining wrapper-55 ordinary-table `ChangeRowOrder=false` heuristics; this closes `2` more files on top of the radio-button work and lands at `11.079 s` critical path (`517.83 rows/s`, `3.43 MiB/s`) |
| Round 94 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `004400_radio_release` improves `DataProcessors` to `1617` different / `5441` unchanged with `0` left-only / `0` right-only after adding `RadioButtonField` extraction plus `ChoiceList`, `RadioButtonType`, and `ColumnsCount` support for managed forms; latest release timing is `15.515 s` critical path (`369.76 rows/s`, `2.45 MiB/s`) |
| Round 93 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `000112_dp_child_order_release` improves `DataProcessors` to `1619` different / `5439` unchanged with `0` left-only / `0` right-only after restoring wrapped-child `EditFormat`, preserving boolean `{"U"}` fill values as `xsi:nil`, separating `TabularSection.FillChecking` from the `LineNumber` standard attribute, and ordering top-level `DataProcessor` attributes before tabular sections; this closes `18` more files and lands at `19.862 s` critical path (`288.84 rows/s`, `1.92 MiB/s`) |
| Round 92 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `220341_dp_childobjects_release` improves `DataProcessors` to `1637` different / `5421` unchanged with `0` left-only / `0` right-only after rebuilding release and emitting explicit empty `DataProcessor/ChildObjects` plus empty child `Attribute/Type` nodes; this closes `29` files on top of the earlier template palette and dynamic-list fixes, while latest release timing lands at `25.825 s` critical path (`222.15 rows/s`, `1.47 MiB/s`) |
| Round 91 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `214500_listsettings_rework` improves `DataProcessors` to `1666` different / `5392` unchanged with `0` left-only / `0` right-only after reworking dynamic-list `ListSettings` normalization, restoring implicit defaults only when the native body omits them and reading `Appearance` / `GroupSelectedSettingId`-backed settings without reintroducing wrong default IDs; latest release timing is `14.799 s` critical path (`387.66 rows/s`, `2.57 MiB/s`), and the old `Form/.../ListSettings/...` signature family drops out of the current top block entirely |
| Round 89 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `220025_valuetreefont` improves `DataProcessors` to `1670` different / `5388` unchanged with `0` left-only / `0` right-only after restoring built-in form attribute types `v8ui:Font` and `v8:ValueTree` on top of the latest `SpreadsheetDocument` / bare-number / `v8ui:Color` fixes; latest signature mining keeps `document/rowsItem/row/c/c/f=3001`, shifts the next template color tails to `document/format/backColor=337` and `document/format/textColor=327`, and pushes the old `Form/Attributes/Attribute/Type` cluster out of the current top signature block; latest release timing is `16.624 s` critical path (`345.10 rows/s`, `2.29 MiB/s`) |
| Round 88 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `195904_btnchoice` improves `DataProcessors` to `1671` different / `5387` unchanged with `0` left-only / `0` right-only after restoring button `AutoMaxWidth` / `MaxWidth` and input-field `ChoiceFoldersAndItems`; latest signature mining keeps `document/rowsItem/row/c/c/f=3001`, `document/format/textColor=1188`, and reduces the remaining form-level table `ExcludedCommand` gap to `876` combined occurrences (`82` files); latest release timing is `13.647 s` critical path (`420.39 rows/s`, `2.79 MiB/s`) |
| Round 87 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `192500_wrapper55` reconfirms `DataProcessors` at `1673` different / `5385` unchanged with `0` left-only / `0` right-only, but narrows the remaining form-level `Form/ChildItems/Table/CommandSet/ExcludedCommand` signature cluster from `964` to `948` occurrences (`94 -> 92` files) after broadening ordinary wrapper-55 table detection, restoring 8-command table `CommandSet` variants, and emitting table row-picture metadata; latest release timing is `13.809 s` critical path (`415.45 rows/s`, `2.75 MiB/s`) |
| Round 86 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `185000_checkall` improves `DataProcessors` to `1673` different / `5385` unchanged with `0` left-only / `0` right-only after restoring more form/root `CommandSet` patterns, wrapper-55 table `ExcludedCommand` variants, and `UncheckAll` standard-command binding; latest release timing is `12.632 s` critical path (`454.16 rows/s`, `3.01 MiB/s`) |
| Round 84 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `233500_mxlpictext` improves `DataProcessors` to `1680` different / `5378` unchanged with `0` left-only / `0` right-only after preserving embedded MXL picture payloads, `textPosition`, `widthWeightFactor`, negative format heights, and row-level `columnsID` ordering; latest release timing is `15.584 s` critical path (`368.13 rows/s`, `2.44 MiB/s`) |
| Round 83 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `190500_clientbank_settlement` improves `DataProcessors` to `1683` different / `5375` unchanged with `0` left-only / `0` right-only after restoring standalone MXL default-format tails, `detailsUse=WithoutProcessing`, non-integer font heights, indexed style overrides / `auto`, and aggregating multiple merge-region lists so both `КлиентБанк/Templates/ОтчетОЗагрузке/Ext/Template.xml` and `ЗаполнениеРегистровВзаиморасчетов/Templates/Макет/Ext/Template.xml` become byte-identical; latest release timing is `13.401 s` critical path (`428.10 rows/s`, `2.84 MiB/s`) |
| Round 82 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `140030_hiddenmerge` improves `DataProcessors` to `1700` different / `5358` unchanged with `0` left-only / `0` right-only after preserving explicit empty `format` / `editFormat`, `hidden=false`, legacy `Bottom` alignment, wrapped MXL style refs, and narrowing the sparse default-format heuristic so `request_offer` becomes byte-identical; latest release timing is `16.896 s` critical path (`339.55 rows/s`, `2.25 MiB/s`) |
| Round 81 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `113500_emptycols` reconfirms `DataProcessors` at `1703` different / `5355` unchanged with `0` left-only / `0` right-only after restoring sparse-source MXL absolute font flags, `{-11}` line-style handling, and removing the extra sparse row/cell `formatIndex` `+1` remap; signature mining collapses template `document/rowsItem/row/c/c/f` from `65299` to `3001`; latest release timing is `13.170 s` critical path (`435.61 rows/s`, `2.89 MiB/s`) |
| Round 80 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `113500_emptycols` improves `DataProcessors` to `1703` different / `5355` unchanged with `0` left-only / `0` right-only after restoring native sparse source-slot MXL column format selection, `request_offer`/`load_goods` default-format placement, and `ReportHeaderBackColor` color normalization; latest release timing is `11.978 s` critical path (`478.96 rows/s`, `3.18 MiB/s`) |
| Round 79 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `113500_emptycols` improves `DataProcessors` to `1706` different / `5352` unchanged with `0` left-only / `0` right-only after normalizing `LockOwnerWindow`, `BeforeRowChange`, and additional table `ExcludedCommand` patterns; latest release timing is `14.977 s` critical path (`383.05 rows/s`, `2.54 MiB/s`) |
| Round 78 full snapshot refresh | full source-only export and diff verification | fresh snapshot `102243_zeroemptyrow1` captures the current tree at `12499` left-only / `8871` different / `28253` unchanged, with `DataProcessors` improved to `1709` different / `5349` unchanged; latest release timing is `124.419 s` critical path (`326.12 rows/s`, `7.11 MiB/s`) |
| Round 77 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `113500_emptycols` improves `DataProcessors` to `1709` different / `5349` unchanged with `0` left-only / `0` right-only after emitting zero-column-slot row `formatIndex=1` and preserving native empty default/picture slot handling; latest release timing is `22.464 s` critical path (`255.39 rows/s`, `1.69 MiB/s`) |
| Round 76 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `092028_mxlrowmap2` improves `DataProcessors` to `1710` different / `5348` unchanged with `0` left-only / `0` right-only after restoring sparse source-derived MXL row/cell remap, `BorderColor` slot 0, `WindowsFont`, and native spreadsheet XML ordering; latest release timing is `12.042 s` critical path (`476.42 rows/s`, `3.16 MiB/s`) |
| Round 75 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `085640_mxlslots` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after preserving source-derived MXL style-slot ordering and native default format tails; latest release timing is `14.397 s` critical path (`398.49 rows/s`, `2.64 MiB/s`) |
| Round 74 full snapshot refresh | full source-only export and diff verification | fresh snapshot `081405_sourcefix` reconfirms overall parity at `12226` different / `37396` unchanged, reconfirms `Reports` at `437` remaining, and reconfirms `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `83.685 s` critical path (`484.87 rows/s`, `10.57 MiB/s`) |
| Round 73 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `081017_17rc17lo5` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after remapping source-derived MXL column format refs; latest release timing is `13.050 s` critical path (`439.62 rows/s`, `2.92 MiB/s`) |
| Round 72 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `013406_zerosize` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after preserving zero-sized default MXL column sets; latest release timing is `14.493 s` critical path (`395.85 rows/s`, `2.62 MiB/s`) |
| Round 71 DataProcessors scoped refresh | selected export, source-only diff, signature mining, and timing verification | fresh selected export `012824_negcol` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after accepting negative MXL column indexes; latest release timing is `14.045 s` critical path (`408.47 rows/s`, `2.71 MiB/s`) |
| Round 70 full snapshot refresh | full source-only export and diff verification | fresh snapshot `010516_vg` reconfirms overall parity at `12226` different / `37396` unchanged, reconfirms `Reports` at `437` remaining, and reconfirms `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.186 s` critical path (`470.80 rows/s`, `10.27 MiB/s`) |
| Round 69 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `005936_vg` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after the `vg` MXL roundtrip fix; latest release timing is `13.730 s` critical path (`417.84 rows/s`, `2.77 MiB/s`) |
| Round 68 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `003832_editformat` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only after the `editFormat` MXL roundtrip fix; latest release timing is `12.405 s` critical path (`462.47 rows/s`, `3.07 MiB/s`) |
| Round 67 full snapshot refresh | full source-only export and diff verification | fresh snapshot `001609` improves overall parity to `12226` different / `37396` unchanged, improves `Reports` to `437` remaining, and reconfirms `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.359 s` critical path (`469.85 rows/s`, `10.25 MiB/s`) |
| Round 66 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `001157` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `13.193 s` critical path (`434.85 rows/s`, `2.88 MiB/s`) |
| Round 65 full snapshot refresh | full source-only export and diff verification | fresh snapshot `235827` reconfirms overall parity at `12227` different / `37395` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.750 s` critical path (`467.73 rows/s`, `10.20 MiB/s`) |
| Round 64 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `235010` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `13.324 s` critical path (`430.58 rows/s`, `2.86 MiB/s`) |
| Round 63 full snapshot refresh | full source-only export and diff verification | fresh snapshot `232306` reconfirms overall parity at `12227` different / `37395` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.922 s` critical path (`466.81 rows/s`, `10.18 MiB/s`) |
| Round 62 full snapshot refresh | full source-only export and diff verification | fresh snapshot `230839` reconfirms overall parity at `12227` different / `37395` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `86.967 s` critical path (`466.57 rows/s`, `10.17 MiB/s`) |
| Round 61 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `230510` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `13.293 s` critical path (`431.58 rows/s`, `2.86 MiB/s`) |
| Round 60 full snapshot refresh | full source-only export and diff verification | fresh snapshot `2252236` reconfirms overall parity at `12227` different / `37395` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `89.698 s` critical path (`452.36 rows/s`, `9.86 MiB/s`) |
| Round 59 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `222918` reconfirms `DataProcessors` at `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `13.722 s` critical path (`418.09 rows/s`, `2.77 MiB/s`) |
| Round 58 full snapshot refresh | full source-only export and diff verification | fresh snapshot `214826` updates overall parity to `12227` different / `37395` unchanged, reconfirms `Reports` at `438` remaining, and captures `DataProcessors` at `1714` different / `5344` unchanged; latest release timing is `89.419 s` critical path (`453.77 rows/s`, `9.90 MiB/s`) |
| Round 57 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `214301` reduces `DataProcessors` to `1714` different / `5344` unchanged with `0` left-only / `0` right-only; latest release timing is `15.957 s` critical path (`359.53 rows/s`, `2.38 MiB/s`) |
| Round 56 full snapshot refresh | full source-only export and diff verification | fresh snapshot `212531` updates overall parity to `12238` different / `37384` unchanged, reconfirms `Reports` at `438` remaining, and reconfirms `DataProcessors` at `1725` different / `5333` unchanged; latest release timing is `90.265 s` critical path (`449.52 rows/s`, `9.80 MiB/s`) |
| Round 55 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `212150` reduces `DataProcessors` to `1725` different / `5333` unchanged with `0` left-only / `0` right-only; latest release timing is `14.474 s` critical path (`396.37 rows/s`, `2.63 MiB/s`) |
| Round 54 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `211339` reduces `DataProcessors` to `1745` different / `5313` unchanged with `0` left-only / `0` right-only; latest release timing is `12.541 s` critical path (`457.46 rows/s`, `3.03 MiB/s`) |
| Round 53 full snapshot refresh | full source-only export and diff verification | fresh snapshot `205558` updates overall parity to `12264` different / `37358` unchanged, reconfirms `Reports` at `438` remaining, and reconfirms `DataProcessors` at `1751` different / `5307` unchanged; latest release timing is `89.677 s` critical path (`452.47 rows/s`, `9.87 MiB/s`) |
| Round 52 DataProcessors scoped refresh | selected export, source-only diff, and timing verification | fresh selected export `2015` updates `DataProcessors` to `1751` different / `5307` unchanged with `0` left-only / `0` right-only; latest release timing is `13.309 s` critical path (`431.06 rows/s`, `2.86 MiB/s`) |
| Round 51 full snapshot refresh | full source-only export and diff verification | fresh snapshot `185626` reconfirms overall parity at `12307` different / `37315` unchanged, `Reports` at `438` remaining, and `DataProcessors` at `1789` different / `5269` unchanged; latest release timing is `82.381 s` critical path (`492.54 rows/s`, `10.74 MiB/s`) |
| Round 50 full snapshot refresh | full source-only export and diff verification | fresh snapshot `0022` updates overall parity to `12307` different / `37315` unchanged, improves `Reports` to `438` remaining, and improves `DataProcessors` to `1789` different / `5269` unchanged; latest release timing is `91.909 s` critical path (`441.48 rows/s`, `9.63 MiB/s`) |
| Round 49 full snapshot refresh | full source-only export and diff verification | fresh snapshot `0021` reconfirms overall parity at `10276` different / `39346` unchanged, `Reports` at `505` remaining, and `DataProcessors` at `3042` different / `4016` unchanged; latest release timing is `133.197 s` critical path (`304.63 rows/s`, `6.64 MiB/s`) |
| Round 48 full snapshot refresh | full source-only export and diff verification | fresh snapshot `0020` reconfirms overall parity at `10276` different / `39346` unchanged, `Reports` at `505` remaining, and `DataProcessors` at `3042` different / `4016` unchanged; latest release timing is `128.107 s` critical path (`316.74 rows/s`, `6.91 MiB/s`) |
| Round 47 full snapshot refresh | full source-only export and diff verification | fresh snapshot `0018` reconfirms overall parity at `10276` different / `39346` unchanged, `Reports` at `505` remaining, and `DataProcessors` at `3042` different / `4016` unchanged; latest release timing is `122.111 s` critical path (`332.29 rows/s`, `7.25 MiB/s`) |
| Round 46 full snapshot refresh | full source-only export and diff verification | fresh snapshot `0017` reconfirms overall parity at `10276` different / `39346` unchanged, `Reports` at `505` remaining, and `DataProcessors` at `3042` different / `4016` unchanged; latest release timing is `133.466 s` critical path (`304.02 rows/s`, `6.63 MiB/s`) |
| Round 45 full snapshot refresh | full source-only export and diff verification | fresh snapshot `0016` updates overall parity to `10276` different / `39346` unchanged, `Reports` to `505` remaining, and `DataProcessors` to `3042` different / `4016` unchanged; latest release timing is `246.829 s` critical path (`164.39 rows/s`, `3.58 MiB/s`) |
| Round 44 full snapshot refresh | full source-only export and diff verification | fresh snapshot `0015` reconfirms overall parity at `8956` different / `40666` unchanged and DataProcessors at `1789` different / `5269` unchanged; latest release timing is `127.389 s` critical path (`318.52 rows/s`, `6.95 MiB/s`) |
| Round 43 full snapshot refresh | full source-only export and diff verification | fresh snapshot `0014` reconfirms overall parity at `8956` different / `40666` unchanged and DataProcessors at `1789` different / `5269` unchanged on the current tree |
| Round 43 Form.xml group slice | unit-level extractor verification | property-bag `UsualGroup` items emit native `Behavior=Usual`, `Representation`, and non-default `Group`, while default root `Form/Group=Vertical` is suppressed |
| Round 43 Form.xml command slice | unit-level extractor verification | command `Representation` / `CurrentRowUse` derive from native command flags, picture presence, and action/name shape instead of over-emitting defaults |
| Round 42 Form.xml tooling slice | CLI and unit-level mapping verification | `form-diff-candidates` compares baseline/variant Form.xml and raw Form body blobs, diffs the parsed layout brace tree, and emits candidate `XML path -> layout path` mappings |
| Round 42 Form.xml slice | unit-level packer verification | new `Table` child items preserve XML `<DataPath>` by using the wider native layout shape |
| Round 42 MXL template slice | unit-level extractor/packer verification | MOXCEL vertical alignment code `48` round-trips as `Bottom` |
| Round 42 Catalog metadata slice | unit-level metadata XML verification | `CreateOnInput`, `ChoiceHistoryOnInput`, `DataHistory`, `UpdateDataHistoryImmediatelyAfterWrite`, and `ExecuteAfterWriteDataHistoryVersionProcessing` are parsed from native root fields and emitted after `Explanation` |
| Round 42 source staging readiness slice | unit-level readiness audit verification | metadata XML base-dependency blockers include direct `Properties` child counts plus `ChildObjects` kind breakdowns |
| Round 42 mssql_dump split slice | compile and focused selected-export verification | selected-export planning helpers are isolated in `src/mssql_dump/selected.rs` without changing dump behavior |
| Round 41 Form.xml slice | unit-level packer verification | new `CheckBoxField` child items can carry explicit `ShowInHeader` through the extended layout shape |
| Round 41 MXL template slice | unit-level extractor/packer verification | MOXCEL number-format string tables round-trip through `document/format/format/v8:item` references |
| Round 41 workflow metadata slice | unit-level metadata XML verification | `BusinessProcess` and `Task` metadata XML emit `UseStandardCommands` from the native owner field |
| Round 41 source staging readiness slice | unit-level readiness audit verification | `Predefined.xml` reports precise base-dependent row, nesting, editable-field and native-shape blockers |
| Round 41 mssql_dump split slice | compile and focused fetch/timing verification | SQL/BCP fetch helpers are isolated in `src/mssql_dump/fetch.rs` without changing the dump API |
| Round 40 Configuration.xml slice | unit-level metadata XML verification | root child object tags are detected through the shared metadata classifier for Language, XDTOPackage, SettingsStorage, ScheduledJob, CommandGroup, Style and DocumentNumerator in source XML 2.20/2.21 |
| Round 40 Catalog metadata slice | unit-level metadata XML verification | `QuickChoice` and `ChoiceMode` are read from native root fields while older shorter blobs keep the previous defaults |
| Round 40 Form.xml slice | unit-level extractor verification | `TextDocumentField` uses the shared read-only path and emits explicit `ReadOnly=true` |
| Round 40 source staging readiness slice | unit-level readiness audit verification | sectionless `Form.xml` bodies report the precise native container skeleton blocker instead of a generic base-free note |
| Round 40 performance evidence slice | timing-summary report verification | `mssql-dump-timing-summary` exposes sorted source-asset and Form CPU breakdowns; latest saved full source-only evidence still points at Form XML reconstruction CPU as the proven bottleneck |
| Round 39 Form.xml slice | unit-level extractor verification | default `Table` child item `SkipOnInput=false` is omitted from source XML while explicit `true` remains supported |
| Round 39 MXL template slice | unit-level template body verification | MOXCEL system style refs `-25`, `-26`, `-27`, `-34`, `-35`, `-36`, `-37`, and `-38` map to native report/table/button style color refs |
| Round 39 InformationRegister metadata slice | unit-level metadata XML verification | extended owner tuple field emits `DataLockControlMode` for `InformationRegister` as well as `AccumulationRegister` |
| Round 39 Role Rights.xml slice | unit-level extractor verification | role rights object ordering can follow metadata tree order while preserving serialized order within one owner |
| Round 39 source staging readiness slice | unit-level readiness audit verification | Form body base-dependency audit reports precise counts for root/layout scalars, child items, trailing sections and `Ext/Form/Items/**` assets |
| Round 38 Form.xml slice | unit-level extractor/packer verification | default `WindowOpeningMode=DontBlock` is omitted from source XML while explicit XML still packs into the form layout |
| Round 38 MXL format slice | unit-level extractor/packer verification | native empty format slots `{0}` no longer drop later format-table entries such as width-bearing formats |
| Round 38 MXL named-area slice | unit-level template body verification | mixed `NamedItemCells` / `NamedItemDrawing` lists preserve valid named areas and skip drawing items in the named-area packer |
| Round 38 object metadata slice | unit-level metadata XML verification | Document and Report child attributes use shared property-tail extraction and emit `DataHistory` after `ChoiceForm` |
| Round 38 Subsystem metadata slice | unit-level metadata XML verification | Subsystem scalar tail emits `IncludeHelpInContents`, `IncludeInCommandInterface`, and native `UseOneCommand` |
| Round 38 source staging readiness slice | unit-level readiness/row-generation verification | `XDTOPackages/*/Ext/Package.bin` stages from source bytes without fetching an active Config blob and is counted by source-load coverage |
| Round 37 Form.xml slice | unit-level extractor/packer verification | dynamic-list `Settings/Field` entries now export `dataPath`/`field` and pack back into the serialized settings bag |
| Round 37 MXL text-placement slice | unit-level extractor/packer verification | `textPlacement=Cut` maps to MOXCEL code `1` in both export and import packing |
| Round 37 MXL format-index slice | unit-level template body verification | raw column `formatIndex` values are preserved separately from normalized internal indexes, keeping non-1-based column format references stable |
| Round 37 Configuration.xml slice | unit-level metadata XML verification | root `CommonPicture` child objects are detected by native field shape and emitted for source XML 2.20/2.21 |
| Round 37 object metadata slice | unit-level metadata XML verification | child attribute `ChoiceForm` tails preserve empty values and resolved non-empty design-time refs |
| Round 37 source staging readiness slice | unit-level readiness/row-generation verification | root `Ext/MainSectionPicture.xml` can stage without fetching an active Config blob and is counted by source-load coverage |
| Round 36 diff-mining slice | reusable XML-path signature mining | added `source-diff-signatures`; a bounded run sampled 2,101 XML pairs from the last full diff and confirmed the largest remaining clusters are MXL `document/format/*` and `formatIndex` signatures |
| Round 36 Form.xml slice | unit-level extractor/packer verification | default `Page/ScrollOnCompress=false` is omitted from source XML while explicit `true` remains supported |
| Round 36 Flowchart.xml slice | unit-level source asset verification | BusinessProcess/GraphicalSchema connection-line `Font` now decodes serialized style font tuples instead of always emitting the default GUI font |
| Round 36 MXL template slice | unit-level extractor/packer verification | spreadsheet `textOrientation` format bit 13 exports to `<textOrientation>` and packs back into MOXCEL bodies |
| Round 36 CommonAttribute metadata slice | unit-level metadata XML verification | CommonAttribute property-tail `FillChecking` is parsed from the native tail and emitted as `ShowError` / `DontCheck` after `FillValue` |
| Round 36 source staging readiness slice | unit-level readiness/row-generation verification | `CommonCommand/Ext/CommandInterface.xml` raw command-interface source rows can be staged base-free |
| Round 35 diff-mining slice | sampled XML-path diff mining | added `docs/diff-mining-2026-07-01-round35.md` to prioritize repeated signatures such as MXL format indexes, Catalog child tails and Form default over-emission |
| Round 35 Form.xml slice | unit-level extractor/packer verification | default `ShowCommandBar=true` is omitted from source XML while explicit `false` and explicit packer `true` remain supported |
| Round 35 Catalog metadata slice | unit-level metadata XML verification | Catalog child attributes now pass `object_refs` into the shared property-tail parser, resolving `ChoiceParameters` design-time refs |
| Round 35 Flowchart.xml slice | unit-level source asset verification | BusinessProcess/GraphicalSchema item `ZOrder` is derived from serialized item order instead of hardcoded zero |
| Round 35 MXL template slice | unit-level template body verification | unknown MOXCEL format bits are consumed without dropping known neighboring format properties or format indexes |
| Round 35 source staging readiness slice | unit-level readiness/row-generation verification | root configuration application modules under `Ext/*.bsl` are explicitly covered as base-free staging rows |
| Round 35 Role Rights.xml slice | unit-level extractor/packer verification | Role rights object refs now include common and owned form refs, and source packing resolves `Form` child refs |
| Round 34 source staging readiness slice | unit-level row-generation verification | `Ext/ParentConfigurations.bin` source bytes are treated as inflated raw-deflated payload and re-deflated for the `ConfigSave` row without fetching an active base blob |
| Round 34 Catalog metadata slice | unit-level metadata XML verification | `Catalog` owner refs are parsed from native metadata and emitted as ordered `xr:MDObjectRef` items in `<Owners>` |
| Round 34 DCS template slice | unit-level template body verification | DCS `calculatedField` `TypeId` values normalize to `d4p1` current-config refs instead of falling back to `d5p1` |
| Round 34 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `RowFilter xsi:nil="true"` extracts and packs through property-bag key `10` with the native `{"U"}` marker |
| Round 34 ExchangePlan Content.xml slice | unit-level source asset verification | ExchangePlan content items are ordered by configuration metadata tree order, preserving blob order as fallback for unresolved items |
| Round 34 CommonAttribute metadata slice | unit-level metadata XML verification | CommonAttribute native property-tail `FillValue` emits string, nil, decimal and boolean XML values in native property order |
| CommandGroups | selected export of all 34 `CommandGroups` UUIDs from `ut_ibcmd` | `0 different / 34 unchanged` |
| Languages | selected export of `Languages/Русский.xml` from `ut_ibcmd` | `0 different / 1 unchanged` |
| FunctionalOptionsParameters | selected export of all 9 `FunctionalOptionsParameters` UUIDs from `ut_ibcmd` after selected write-set fix | `0 different / 9 unchanged` |
| SessionParameters | selected export of all 98 `SessionParameters` UUIDs from `ut_ibcmd` via `--file-name-list` | `0 different / 98 unchanged` |
| DocumentNumerators | selected export of all 3 `DocumentNumerators` UUIDs from `ut_ibcmd` via `--file-name-list` | `0 different / 3 unchanged` |
| WSReferences | selected export of `UpdateFilesApiImplService` metadata and `.0` body from `ut_ibcmd` via `--file-name-list` | `0 different / 2 unchanged` |
| IntegrationServices | selected export of `ОбменСообщениями` metadata and `.0` module from `ut_ibcmd` via `--file-name-list` | `0 different / 2 unchanged` |
| Ext/HomePageWorkArea.xml | selected export of `c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.8` from `ut_ibcmd` with metadata indexes | byte-identical to native |
| Ext/ClientApplicationInterface.xml | selected export of `c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.b` from `ut_ibcmd` | byte-identical to native |
| Ext/CommandInterface.xml | selected export of `c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.a` from `ut_ibcmd` with metadata indexes | byte-identical to native |
| Ext/MainSectionCommandInterface.xml | selected export of `c5546fcc-f2dc-43dd-8d40-a6cc2a81b865.9` from `ut_ibcmd` with metadata indexes | byte-identical to native |
| Ext | selected export of configuration-level `.8/.9/.a/.b` from `ut_ibcmd` with metadata indexes | `0 different / 4 unchanged`; all 15 `Ext` files covered by snapshot + selected verification |
| Round 9 Form.xml slice | unit-level packer verification | `TextDocumentField/ReadOnly` now patches existing form layout read-only slot |
| Round 9 DataProcessor metadata slice | unit-level metadata XML verification | owned `DataProcessor` template child refs are emitted in `<ChildObjects>` |
| Round 9 source staging readiness slice | unit-level staging verification | `CommonPicture` and configuration picture rows are prepared without reading active `Config` blobs |
| Round 9 selected command-interface performance slice | unit-level selected export verification | selected `.9` command refs can use targeted owner metadata rows and keep broad fallback |
| Round 10 Form.xml slice | unit-level packer verification | `TextDocumentField/Width` now patches existing and newly created form layout items |
| Round 10 MXL template slice | unit-level formatter verification | Moxel system style `-13` is emitted as `style:FieldTextColor` |
| Round 10 Configuration.xml slice | unit-level metadata XML verification | root `Configuration.xml` emits nested `CommonAttribute` child object headers |
| Round 10 source staging readiness slice | unit-level staging verification | object/form `Help.xml` rows use deterministic body ids without querying active `Config` |
| Round 11 Form.xml slice | unit-level packer verification | new `CheckBoxField` form child items can be compiled from XML |
| Round 11 Role Rights.xml slice | unit-level rights packer verification | omitted false/no-restriction rights are treated as false while preserving base table shape |
| Round 11 InformationRegister metadata slice | unit-level metadata XML verification | `InformationRegister` emits generated type `InternalInfo` entries |
| Round 11 source staging readiness slice | unit-level staging verification | common/object/nested command module bodies are prepared without active module blobs |
| Round 12 AccumulationRegister metadata slice | unit-level metadata XML verification | `AccumulationRegister` emits generated type `InternalInfo` entries |
| Round 12 Form.xml slice | unit-level packer verification | `Pages` / `Page` layout items support additional creation and patch properties |
| Round 12 Configuration.xml slice | unit-level metadata XML verification | root `Configuration.xml` emits nested `CommonModule` child object headers |
| Round 12 source staging readiness slice | unit-level staging verification | `ExchangePlan/Ext/Content.xml` rows are prepared without active Config blobs |
| Round 13 Form.xml slice | unit-level packer verification | new `PictureDecoration` child items can be compiled from XML |
| Round 13 Catalog metadata slice | unit-level metadata XML verification | `Catalog` metadata emits owned `Form` and `Template` child object headers |
| Round 13 MXL template slice | unit-level packer/formatter verification | SpreadsheetDocument `style:ButtonTextColor` is supported in pack and extract paths |
| Round 13 source staging readiness slice | unit-level staging verification | SpreadsheetDocument `Ext/Template.xml` rows are prepared without active Config blobs |
| Round 14 Form.xml slice | unit-level packer verification | new `InputField` items preserve explicit `ReadOnly` values |
| Round 14 BusinessProcess metadata slice | unit-level metadata XML verification | `BusinessProcess` emits generated type `InternalInfo` entries |
| Round 14 Task metadata slice | unit-level metadata XML verification | `Task` metadata emits owned `Form` and `Template` child object headers |
| Round 14 source staging readiness slice | unit-level staging verification | `Style/Ext/Style.xml` rows are prepared without active Config blobs |
| Round 15 Form.xml slice | unit-level packer verification | `InputField` `DataPath` is packed for existing and new layout entries |
| Round 15 partial metadata slice | unit-level metadata XML verification | `ExchangePlan` and related partial families emit owned `Form` and `Template` child object refs |
| Round 15 simple metadata slice | unit-level metadata XML verification | `DocumentJournal` emits owned `Form` and `Template` child object refs |
| Round 15 source staging readiness slice | unit-level staging verification | HTMLDocument template rows are covered as base-free help-style blobs |
| Round 16 Form.xml slice | unit-level packer verification | new `LabelField` items preserve explicit `ShowInHeader` values |
| Round 16 partial metadata slice | unit-level metadata XML verification | `AccountingRegister` and `CalculationRegister` emit generated type `InternalInfo` entries |
| Round 16 Configuration.xml slice | unit-level metadata XML verification | root `Configuration.xml` emits `Constant` child object headers |
| Round 16 source staging readiness slice | unit-level staging verification | raw-id `CommandInterface.xml` rows can be prepared without active Config blobs |
| Round 17 Form.xml slice | unit-level packer verification | new `InputField` items preserve explicit `SkipOnInput` values |
| Round 17 partial metadata slice | unit-level metadata XML verification | `Task` emits generated type `InternalInfo` entries |
| Round 17 MXL template slice | unit-level packer verification | SpreadsheetDocument `style:FieldSelectionBackColor` is supported in pack path |
| Round 17 source staging readiness slice | unit-level staging verification | root configuration raw-id `CommandInterface.xml` rows are covered as base-free |
| Round 18 Form.xml slice | unit-level packer verification | new top-level `Button` items preserve explicit `DefaultButton` values |
| Round 18 partial metadata slice | unit-level metadata XML verification | `ExchangePlan` emits `UseStandardCommands` from the metadata blob |
| Round 18 Configuration.xml slice | unit-level metadata XML verification | root `Configuration.xml` emits `Catalog` child object headers |
| Round 18 source staging readiness slice | unit-level staging verification | root configuration raw-id `MainSectionCommandInterface.xml` rows are covered as base-free |
| Round 19 Form.xml slice | unit-level packer verification | new `InputField` items preserve extended options (`Width`, `HorizontalStretch`, `AutoMaxWidth`, `MaxWidth`) |
| Round 19 partial metadata slice | unit-level metadata XML verification | `Subsystem` emits `UseStandardCommands` from the metadata blob |
| Round 19 source staging readiness slice | unit-level staging verification | `WSReference` definition rows are covered as base-free raw-deflated bodies |
| Round 20 Form.xml slice | unit-level packer verification | new `UsualGroup` items preserve extended `Behavior`, `Representation`, `ShowTitle=false`, and nested children |
| Round 20 partial metadata slice | unit-level metadata XML verification | register-family metadata emits `UseStandardCommands` from the metadata blob |
| Round 20 source staging readiness slice | unit-level staging verification | configuration `Ext/StandaloneConfigurationContent.bin` rows are covered as base-free raw-deflated bodies |
| Round 21 Form.xml slice | unit-level packer verification | nested `AutoCommandBar` items preserve `HorizontalAlign` and `Autofill` settings |
| Round 21 object metadata slice | unit-level metadata XML verification | `DataProcessor` emits `UseStandardCommands` from the metadata blob |
| Round 21 source staging readiness slice | unit-level staging verification | configuration `Ext/MobileClientSignature.bin` rows are covered as base-free raw-deflated bodies |
| Round 22 Form.xml slice | unit-level extractor/packer verification | form command `ModifiesSavedData=true` is extracted from slot 10 and packed back into existing/new command rows |
| Round 22 root/common attribute metadata slice | unit-level metadata XML verification | root `Configuration.xml` emits additional child families; `CommonAttribute` emits native-shaped `Content` and `AutoUse` enum values |
| Round 22 source staging readiness slice | unit-level readiness audit verification | form body staging remains base-dependent, but readiness reports precise blockers for layout, trailing sections, modules and item assets |
| Round 22 V8 container slice | unit-level module blob/container verification | shared `v8_container` parser/builder preserves module blob behavior and tests multi-element, round-trip and multi-page rejection behavior |
| Round 23 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table items extract `AutoRefresh` / `AutoRefreshPeriod` from property bag keys `5`/`6` and pack them back into existing rows |
| Round 23 object metadata slice | unit-level metadata XML verification | `Document` metadata emits owned `Form` and `Template` child refs from generic form/template indexes |
| Round 23 partial metadata slice | unit-level metadata XML verification | `InformationRegister` reads `UseStandardCommands` from the extended native owner tuple while keeping simplified-shape fallback |
| Round 23 MXL template slice | unit-level extractor/packer verification | SpreadsheetDocument MOXCEL merge region kind `2` is preserved as `verticalUnmerge` and packs back into the same region list |
| Round 23 source staging readiness slice | unit-level readiness audit verification | `Role/Ext/Rights.xml` remains base-dependent, but readiness now reports exact blockers around role object order, right UUID slots, restrictions and template/trailing layout |
| Round 23 native dump performance slice | unit-level fetch/timing verification | streamed `mssql-dump-config` routes blob-bearing metadata/direct prepare reads through the existing native `bcp` parser and reports `sqlcmd`/`bcp` timing split |
| Round 24 Role Rights.xml slice | unit-level extractor/packer verification | role rights objects are sorted by object UUID for export, and packer maps XML objects back to base rights slots using the same UUID order |
| Round 24 Form.xml slice | unit-level extractor/packer verification | table child items extract and pack `SkipOnInput`, including wrapper `55` layouts |
| Round 24 object metadata slice | unit-level metadata XML verification | `Document` metadata emits `StandardAttributes`; the `Number` standard attribute fill value follows native number type |
| Round 24 CommonAttribute metadata slice | unit-level metadata XML verification | native `Content` pairs of metadata UUID and `{2,use_flag,...}` settings are parsed, including `use_flag=2` as `DontUse`, while simplified shape remains supported |
| Round 24 DCS template slice | unit-level template body verification | DataCompositionSchema template bodies rewrite known `v8:TypeId` values through the metadata type index to source-style current-config `v8:Type` references |
| Round 24 ExchangePlan Content.xml slice | unit-level source asset verification | `AutoRecord` values now map as `0=Deny`, `1=Allow`, `2=Auto` |
| Round 24 source staging readiness slice | unit-level readiness audit verification | `BusinessProcess/Ext/Flowchart.xml` remains base-dependent, but readiness now reports precise blockers for item table order, type-code slots, events, explanations and nested payload shape |
| Round 25 Role Rights.xml slice | unit-level extractor/packer verification | explicit disabled `false` rights without restrictions are preserved instead of being dropped |
| Round 25 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table items extract and pack `UseAlternationRowColor` through property bag key `9` |
| Round 25 object metadata slice | unit-level metadata XML verification | `Document` metadata emits numbering settings, standard command flag and default object/list/choice form refs |
| Round 25 InformationRegister metadata slice | unit-level metadata XML verification | `DefaultRecordForm` and `DefaultListForm` refs are resolved through owner metadata form indexes |
| Round 25 MXL template slice | unit-level template body verification | SpreadsheetDocument text-node quotes remain literal while XML structural characters stay escaped |
| Round 25 Configuration.xml slice | unit-level metadata XML verification | root scalar application/compatibility properties are emitted in native-shaped metadata XML |
| Round 25 source staging readiness slice | unit-level readiness audit verification | `Catalog/Ext/Predefined.xml` and `ChartOfCharacteristicTypes/Ext/Predefined.xml` remain base-dependent, but readiness reports precise blockers for row order, parent/type slots and trailing fields |
| Round 26 native dump performance slice | unit-level timing/batching verification | native `bcp` row batch cap is reduced to 256 MiB and timing JSON reports batch count/max rows/max binary bytes |
| Round 26 Role Rights.xml slice | unit-level extractor/packer verification | role object refs use UUID plus serialized tail fields, preserving repeated owner refs and standard-attribute refs |
| Round 26 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table items extract and pack observed `UpdateOnDataChange=Auto` through property bag key `14` |
| Round 26 object metadata slice | unit-level metadata XML verification | Catalog standard attribute labels and DataProcessor object/presentation scalars are parsed from metadata blobs |
| Round 26 register/exchange metadata slice | unit-level metadata XML verification | AccumulationRegister and ExchangePlan generated type `InternalInfo` coverage is expanded |
| Round 26 Configuration.xml slice | unit-level metadata XML verification | root `DefaultStyle` and `DefaultLanguage` refs are emitted from UUID-backed metadata fields |
| Round 26 DCS template slice | unit-level template body verification | observed DCS body `AnyIBRef`/`TypeSet` and settings `xsi:type` values are canonicalized |
| Round 26 source staging readiness slice | unit-level readiness audit verification | readable `CommandInterface.xml` refs now report precise base-free blockers while raw `kind:uuid` refs remain base-free |
| Round 27 Role Rights.xml slice | unit-level extractor verification | nested subsystem and integration service channel refs resolve to fuller native-style child object names |
| Round 27 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table items extract and pack existing `AllowGettingCurrentRowURL` boolean slots |
| Round 27 object metadata slice | unit-level metadata XML verification | Report command child headers and Catalog/DataProcessor Attribute/TabularSection child headers are emitted |
| Round 27 workflow/register metadata slice | unit-level metadata XML verification | BusinessProcess/Task generated types and AccumulationRegister `IncludeHelpInContents=false` are emitted |
| Round 27 Configuration.xml slice | unit-level metadata XML verification | root `<DefaultRoles>` is emitted from resolved role refs in metadata field `39` |
| Round 27 DCS template slice | unit-level template body verification | TypeId normalization can use `xr:GeneratedType` entries from XML-shaped metadata text |
| Round 27 source staging readiness slice | unit-level readiness audit verification | configuration `Ext/HomePageWorkArea.xml` has explicit base-free readiness classification |
| Round 27 native dump performance slice | real selected-run timing plus unit-level summary verification | `mssql-dump-timing-summary` added; selected 663,776,134-byte blob run used 3 BCP batches with `fetch_rows_ms=3532` |
| Round 28 standalone-content source asset slice | unit-level source-only extraction verification plus real selected repro | `Ext/StandaloneConfigurationContent.bin` now builds targeted metadata refs when metadata XML extraction is disabled; the previous `0014cc2a-b5ed-427d-8ac6-116e92aaa9a4` blocker resolves as a role ref |
| Round 28 Role Rights.xml slice | unit-level extractor/packer verification | string-backed restriction fields export/import through `<restrictionByCondition><field>` while preserving base layout |
| Round 28 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `UserSettingsGroup` extracts and packs existing numeric property-bag slots |
| Round 28 object metadata slice | unit-level metadata XML verification | Catalog/DataProcessor child attribute type refs and Report auxiliary-variant placeholder behavior improved |
| Round 28 register/exchange slice | unit-level metadata/asset verification | AccumulationRegister `RegisterType` and ExchangePlan `Content.xml` BOM/final-newline framing are covered |
| Round 28 CommonAttribute slice | unit-level metadata XML verification | content item `xr:ConditionalSeparation` refs are resolved from native settings records |
| Round 28 DCS template slice | unit-level selected-index verification | selected template owners build `type_index` so DCS TypeIds can normalize in selected/source-only flows |
| Round 28 source staging readiness slice | unit-level readiness audit verification | metadata XML rows now report precise base-dependent blockers for incomplete native metadata compilation |
| Round 29 ExchangePlan content/performance slice | unit-level source-only extraction verification plus real selected repro | source asset extraction builds source-layout refs without metadata XML; ExchangePlan content failures now name the exact unsupported metadata id, currently a Constant ref not present in the selected/full source-layout index |
| Round 29 Role Rights.xml slice | unit-level extractor/packer verification | Role object rights preserve serialized object order instead of UUID/tail sorting |
| Round 29 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `DefaultItem=true` extracts and packs through property-bag key `11` |
| Round 29 object metadata slice | unit-level metadata XML verification | child `Attribute` property tails emit format/edit/fill/history details from native metadata blobs |
| Round 29 register metadata slice | unit-level metadata XML verification | AccumulationRegister `DataLockControlMode` and `FullTextSearch` owner properties are emitted |
| Round 29 CommonAttribute slice | unit-level metadata XML verification | native property detail defaults are emitted before `Content` for source XML 2.20/2.21 |
| Round 29 MXL template slice | unit-level template body verification | default-only spreadsheet `printSettings` blocks are suppressed to match native source output |
| Round 29 source staging readiness slice | unit-level readiness audit verification | `versions` rows now report precise base-free blockers for generation UUID maps, entry order, unchanged row sets, and platform-owned standard entries |
| Round 30 ExchangePlan content/performance slice | unit-level source-only extraction verification plus real selected/full repro | selected ExchangePlan content now requests `object_refs`; selected and full source-layout exports completed without `ConfigDumpInfo.xml` |
| Round 30 Role Rights.xml slice | unit-level packer verification | Role rights import packing maps objects by source-resolved refs instead of XML order, including commands, child dimensions/resources/attributes, standard attributes, and direct metadata refs |
| Round 30 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `RowPictureDataPath` extracts and packs through string property-bag key `19` |
| Round 30 object metadata slice | unit-level metadata XML verification | child `Attribute` tail properties now include choice/use/indexing/full-text/data-history scalar details, and Report/DataProcessor child commands emit command property tails |
| Round 30 register metadata slice | unit-level metadata XML verification | InformationRegister and AccumulationRegister standard attributes are emitted; AccumulationRegister type value `1` maps to `Turnovers` |
| Round 30 CommonAttribute slice | unit-level metadata XML verification | native separation tail properties and UUID-backed separation refs are emitted |
| Round 30 DCS template slice | unit-level template body verification | current-config `v8:Type` prefixes normalize by DCS context (`d4p1`, `d5p1`, `d6p1`) |
| Round 30 source staging readiness slice | unit-level readiness/row-generation verification | FilterCriterion manager modules can stage base-free from `FilterCriteria/<Name>/Ext/ManagerModule.bsl` |
| Round 31 native dump performance slice | real full source-layout timing plus unit-level extractor verification | `InputField` option bags are parsed once per form item; after-run completed without `ConfigDumpInfo.xml`, with `source_asset_form_child_items_cpu_ms` reduced from 70,750 to 68,886 and `source_asset_form_cpu_ms` from 124,212 to 123,507 |
| Round 31 Role Rights.xml slice | unit-level extractor/packer verification | Task child refs map as `Task.<name>.AddressingAttribute.<child>` and source-based rights packing resolves `AddressingAttribute` children |
| Round 31 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `ChoiceFoldersAndItems` extracts and packs through the existing typed table slot |
| Round 31 object metadata slice | unit-level metadata XML verification | tabular-section property tails are emitted, and generic child property tails include empty `<LinkByType/>` before `ChoiceHistoryOnInput` |
| Round 31 register metadata slice | unit-level metadata XML verification | register dimensions/resources/attributes reuse generic child object parsing and emit decoded types plus common property tails |
| Round 31 Configuration.xml slice | unit-level metadata XML verification | root settings-storage refs (`CommonSettingsStorage`, report settings/variants storage, form data settings storage) are emitted from UUID-backed metadata fields |
| Round 31 DCS template slice | unit-level template body verification | generated type id lookup is case-insensitive for DataCompositionSchema template bodies |
| Round 31 source staging readiness slice | unit-level readiness/row-generation verification | WebService `Ext/Module.bsl` is covered as a base-free source staging body |
| Round 32 native dump performance slice | real full source-layout timing plus unit-level form verification | form child-item support indexes are built in one traversal; after-run completed without `ConfigDumpInfo.xml`, with `source_asset_form_child_items_cpu_ms` reduced from 68,886 to 40,598 and `source_asset_form_xml_cpu_ms` from 118,673 to 102,296 |
| Round 32 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `RestoreCurrentRow` extracts and packs through property-bag key `12` |
| Round 32 object metadata slice | unit-level metadata XML verification | Document owner `<IncludeHelpInContents>` is emitted after default/auxiliary form properties |
| Round 32 ExchangePlan metadata slice | unit-level metadata XML verification | ExchangePlan code-4/code-27 child attributes are emitted with value types and property tails |
| Round 32 Configuration.xml slice | unit-level metadata XML verification | localized root information fields are emitted: `BriefInformation`, `DetailedInformation`, `Copyright`, `VendorInformationAddress`, and `ConfigurationInformationAddress` |
| Round 32 source staging readiness slice | unit-level readiness audit verification | unsupported `AdditionalIndexes.xml` families now report a precise `requires_base_blob` blocker instead of being silently omitted |
| Round 33 Role Rights.xml slice | unit-level extractor/packer verification | HTTPService URL template method refs resolve as `HTTPService.<service>.URLTemplate.<template>.Method.<method>` and pack back from source XML |
| Round 33 DCS template slice | unit-level template body verification | unqualified DCS core `xsi:type` values normalize to `dcscor:*`; data-core `StandardPeriod` / `StandardPeriodVariant` normalize to `v8:*` |
| Round 33 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `Period` extracts and packs through property-bag key `7` as `v8:StandardPeriodVariant=Custom` |
| Round 33 object metadata slice | unit-level metadata XML verification | DataProcessor child metadata emits non-empty `ChoiceParameters`, resolving design-time UUIDs to stable refs and fixed arrays |
| Round 33 ExchangePlan metadata slice | unit-level metadata XML verification | optional `xr:ThisNode` is emitted in `InternalInfo`, and `UseStandardCommands` is read relative to the detected header slot |
| Round 33 source staging readiness slice | unit-level row-generation verification | module `.bin` source assets containing inflated V8 containers can stage base-free for common/object/nested module bodies |
| Round 34 source staging readiness slice | unit-level row-generation verification | `Ext/ParentConfigurations.bin` source bytes are re-deflated into the staged `ConfigSave` body instead of being written directly |
| Round 34 Catalog metadata slice | unit-level metadata XML verification | Catalog `<Owners>` now emits resolved owner refs before `<SubordinationUse>` |
| Round 34 DCS template slice | unit-level template body verification | DCS `calculatedField` current-config type refs use the native `d4p1` namespace context |
| Round 34 Form.xml slice | unit-level extractor/packer verification | wrapper `55` table `RowFilter xsi:nil="true"` maps to property-bag key `10` / `{"U"}` |
| Round 34 ExchangePlan Content.xml slice | unit-level source asset verification | `ExchangePlan/Ext/Content.xml` ordering follows configuration metadata child order when that index is available |
| Round 34 CommonAttribute metadata slice | unit-level metadata XML verification | CommonAttribute property-tail `FillValue` supports string, nil, decimal and boolean encodings |
| Round 35 diff-mining slice | sampled XML-path diff mining | planning now targets repeated signatures instead of broad object families; full all-file pass still needs a faster reusable diagnostic |
| Round 35 Form.xml slice | unit-level extractor/packer verification | default `ShowCommandBar=true` is suppressed in export, and explicit XML still packs into the form layout |
| Round 35 Catalog metadata slice | unit-level metadata XML verification | Catalog child attributes reuse generic native-order property tails and resolve choice-parameter refs through `object_refs` |
| Round 35 Flowchart.xml slice | unit-level source asset verification | Flowchart item `ZOrder` is emitted from serialized item position rather than fixed zero |
| Round 35 MXL template slice | unit-level template body verification | unknown MOXCEL format bits no longer invalidate the entire format table |
| Round 35 source staging readiness slice | unit-level readiness/row-generation verification | configuration application modules have explicit base-free readiness and row-generation coverage |
| Round 35 Role Rights.xml slice | unit-level extractor/packer verification | role rights extraction and packing support form refs including owned forms |
| Round 36 diff-mining slice | reusable XML-path signature mining | `source-diff-signatures` turns a `source-diff` JSON into ranked repeated XML path signatures with per-kind sampling |
| Round 36 Form.xml slice | unit-level extractor/packer verification | `Page/ScrollOnCompress=false` is treated as a native omitted default; explicit `true` still round-trips |
| Round 36 Flowchart.xml slice | unit-level source asset verification | flowchart connection-line fonts decode from serialized style tuples, including standard style-item refs |
| Round 36 MXL template slice | unit-level extractor/packer verification | MXL `textOrientation` format bit 13 is preserved in export and import packing |
| Round 36 CommonAttribute metadata slice | unit-level metadata XML verification | CommonAttribute `FillChecking` is emitted from native property details in native XML order |
| Round 36 source staging readiness slice | unit-level readiness/row-generation verification | raw `CommonCommand/Ext/CommandInterface.xml` staging no longer needs an active base blob |
| Round 37 Form.xml slice | unit-level extractor/packer verification | dynamic-list `Settings/Field` entries round-trip through the form settings bag |
| Round 37 MXL text-placement slice | unit-level extractor/packer verification | MOXCEL text placement code `1` is emitted as `Cut` and packs back as code `1` |
| Round 37 MXL format-index slice | unit-level template body verification | non-1-based column `formatIndex` values are preserved in XML while internal decoding can still normalize indexes |
| Round 37 Configuration.xml slice | unit-level metadata XML verification | root `CommonPicture` child objects are emitted without misclassifying other code-4 root children |
| Round 37 object metadata slice | unit-level metadata XML verification | shared child attribute property tails now include resolved non-empty `ChoiceForm` refs |
| Round 37 source staging readiness slice | unit-level readiness/row-generation verification | `Ext/MainSectionPicture.xml` is covered by base-free row generation and source-load audit classification |
| Round 38 Form.xml slice | unit-level extractor/packer verification | `WindowOpeningMode=DontBlock` is treated as a native omitted default while explicit XML still round-trips |
| Round 38 MXL format slice | unit-level extractor/packer verification | empty format slots pack as native `{0}` and extract as valid empty slots |
| Round 38 MXL named-area slice | unit-level template body verification | named-area parsing skips drawing named items without losing later cell named areas |
| Round 38 object metadata slice | unit-level metadata XML verification | Document/Report child attribute metadata now includes shared property tails such as `DataHistory` |
| Round 38 Subsystem metadata slice | unit-level metadata XML verification | Subsystem native scalar tail no longer emits the obsolete `UseStandardCommands` property |
| Round 38 source staging readiness slice | unit-level readiness/row-generation verification | XDTO package `Ext/Package.bin` source bytes are prepared as base-free raw-deflated body rows |

Performance note for selected extraction:

- `FunctionalOptionsParameters` selected export now writes only 9 rows / 9 metadata XML files / 0 source assets; output size is about 0.02 MB.
- The previous accidental broad run processed 40,576 rows and 12,647 source assets. The remaining selected-export cost is now `prepare_indexes_ms` for the broad metadata reference index, not row fetching or asset writing.
- `--file-name-list` is available for larger selected sets; long generated SQL is sent to `sqlcmd` through a temporary `.sql` file to avoid Windows command-line length limits.
- `DocumentNumerators` selected export writes 3 rows / 3 metadata XML files / 0 source assets; XML generation is about 3 ms. After skipping the broad metadata reference index for self-contained selected types, `prepare_indexes_ms` dropped from about 20 s to 77 ms on this set.
- `WSReferences` selected export writes 2 rows / 1 metadata XML file / 1 source asset. After extracting only the embedded WSDL XML and skipping broad indexes for `WSReference.0`, `prepare_indexes_ms` dropped from about 19.7 s to 81 ms on this set.
- `IntegrationServices` selected export writes 2 rows / 1 metadata XML file / 1 module text file. After parsing child channels and skipping broad indexes for `IntegrationService.0` module text, `prepare_indexes_ms` dropped from about 19 s to 89 ms on this set.
- Configuration-level `Ext` selected export maps `.8/.9/.a/.b` rows to source paths without requiring the `.0/.5/.6/.7` module group, so quick diagnostics can write these files from a `--file-name-list`.
- `Ext` `.8/.9/.a/.b` selected source-mode run is byte-identical to native and no longer performs a broad metadata fetch. Latest timing: `prepare_indexes_ms=1552`, split into `prepare_metadata_fetch_ms=211`, `prepare_metadata_texts_ms=768`, and `prepare_reference_indexes_ms=573`. Detailed reference-index timings were `prepare_command_refs_ms=392`, `prepare_form_refs_ms=163`, `prepare_metadata_refs_ms=14`; all other reference-index builders were skipped.
- Broader selected command-interface diagnostics can still need a targeted/cache reference index when command UUIDs only exist inside owner metadata blobs rather than as direct `FileName` rows.
- Issue #19 bottleneck note: `docs/issue-19-selected-export-bottleneck.md` pinpoints selected command-interface `command_refs` as the next safe optimization target.
- Round 23 #19 follow-up: streamed `mssql-dump-config` no longer uses `sqlcmd` for blob-bearing metadata/direct prepare reads; those paths now use the existing native `bcp` parser. `sqlcmd` still remains for lightweight row headers/control queries. The latest full-run evidence before this change had `fetch_rows_ms=11176` against `process_rows_wall_ms=85887` and `prepare_indexes_ms=33210`, so the primary remaining bottleneck is CPU/source generation and reference/index preparation, not SQL blob transfer.
- Round 26 #19 memory/timing follow-up: native `bcp` row fetch batches are capped at 256 MiB instead of 1 GiB to reduce peak temp-file plus parsed-row memory pressure. The dump report now includes `timings.fetch_row_batches`, `timings.fetch_row_batch_max_rows`, and `timings.fetch_row_batch_max_binary_bytes` to isolate whether the next full-run bottleneck is batch memory pressure or row-processing wall time.
- Round 27 #19 timing follow-up: added `mssql-dump-timing-summary` for saved dump JSON reports and ran a real selected blob fetch on `ut_ibcmd` under `E:\ibcmd_lab\perf`. The selected run fetched 663,776,134 bytes in 3 BCP batches, with `fetch_row_batch_max_binary_bytes=268302217` and `fetch_rows_ms=3532`.
- Round 28 #19 standalone-content follow-up: source-asset-only dumps now resolve `Ext/StandaloneConfigurationContent.bin` references without requiring `--extract-metadata-xml`. A real selected repro on `ut_ibcmd` wrote 6 rows / 40,421 bytes / 2 source assets under `E:\ibcmd_lab\perf\issue-19-standalone-content-v6-standalone-repro-targeted`, resolving `0014cc2a-b5ed-427d-8ac6-116e92aaa9a4` as `Role.РазделОтчетыИМониторингЦелевыеПоказатели`. A full source-layout timing retry passed standalone content and then stopped at `ExchangePlans\Полный\Ext\Content.xml`; no `ConfigDumpInfo.xml` was generated.
- Round 29 #19 ExchangePlan content follow-up: source-layout extraction now builds metadata object/type indexes when source assets are extracted without metadata XML, and `ExchangePlanContent` parsing reports precise unsupported ids. A selected repro for `ExchangePlans\Полный\Ext\Content.xml` now stops at `ExchangePlanContent item 0 references unsupported metadata id ff76e85a-6d29-41d3-a83e-f4a34139c6b2`; the id is a direct Constant row named `ВыгружатьВнутренниеШтрихкодыШтучныхТоваров`. Artifacts are under `E:\ibcmd_lab\perf\issue-19-exchange-content-v1-*`. No `ConfigDumpInfo.xml` was generated.
- Round 30 #19 ExchangePlan content follow-up: selected ExchangePlan owner metadata now requests `object_refs`, so the Constant referenced from `ExchangePlans\Полный\Ext\Content.xml` resolves. The selected repro passed, and a full release source-layout run completed under `E:\ibcmd_lab\perf\issue-19-exchange-constant-ref-v2-full-source`, writing 13,428 files from 40,576 rows / 10,277 source assets without `ConfigDumpInfo.xml`. Full timing: `prepare_indexes_ms=17461`, `prepare_object_refs_ms=10621`, `fetch_rows_ms=5043`, `process_rows_wall_ms=15598`, `source_asset_cpu_ms=191976`, `source_asset_form_cpu_ms=124212`.
- Round 31 #19 form CPU follow-up: `InputField` extended option bags are cached per form child item instead of reparsed for each option probe. The full source-layout after-run completed under `E:\ibcmd_lab\perf\issue-19-form-cpu-v1-full-source-after` without `ConfigDumpInfo.xml`; `source_asset_form_child_items_cpu_ms` changed from 70,750 to 68,886 and `source_asset_form_cpu_ms` from 124,212 to 123,507.
- Round 32 #19 form CPU follow-up: form child-item support indexes for table names, table columns, child item names, and `UserSettingsGroup` resolution are now built in one traversal. The full source-layout after-run completed under `E:\ibcmd_lab\perf\issue-19-source-layout-perf-v2-full-source-after` without `ConfigDumpInfo.xml`; `source_asset_form_child_items_cpu_ms` changed from 68,886 to 40,598, `source_asset_form_xml_cpu_ms` changed from 118,673 to 102,296, and `source_asset_form_cpu_ms` changed from 123,507 to 106,971.
- Scope decision: `ConfigDumpInfo.xml` is intentionally not generated. The native file is derived from the `versions` row, but it is not needed for our export/import target and should not be treated as remaining work.

Diff by file kind:

| kind | different |
|---|---:|
| form | 10690 |
| metadata_xml | 3902 |
| template | 1101 |
| other | 1170 |
| configuration_root | 1 |

## Top-Level Objects

| Object | State | Total | Unchanged | Different | Ready % |
|---|---|---:|---:|---:|---:|
| CommandGroups | done | 34 | 34 | 0 | 100.0 |
| CommonCommands | done | 612 | 612 | 0 | 100.0 |
| CommonModules | done | 5594 | 5594 | 0 | 100.0 |
| CommonPictures | done | 5505 | 5505 | 0 | 100.0 |
| Constants | done | 1235 | 1235 | 0 | 100.0 |
| DefinedTypes | done | 456 | 456 | 0 | 100.0 |
| DocumentNumerators | done | 3 | 3 | 0 | 100.0 |
| EventSubscriptions | done | 312 | 312 | 0 | 100.0 |
| Ext | done | 15 | 15 | 0 | 100.0 |
| FunctionalOptions | done | 567 | 567 | 0 | 100.0 |
| FunctionalOptionsParameters | done | 9 | 9 | 0 | 100.0 |
| IntegrationServices | done | 2 | 2 | 0 | 100.0 |
| Languages | done | 1 | 1 | 0 | 100.0 |
| ScheduledJobs | done | 402 | 402 | 0 | 100.0 |
| SessionParameters | done | 98 | 98 | 0 | 100.0 |
| StyleItems | done | 400 | 400 | 0 | 100.0 |
| WSReferences | done | 2 | 2 | 0 | 100.0 |
| XDTOPackages | done | 814 | 814 | 0 | 100.0 |
| Enums | partial | 1195 | 1163 | 32 | 97.3 |
| CommonTemplates | partial | 495 | 414 | 81 | 83.6 |
| Reports | partial | 2362 | 1925 | 437 | 81.5 |
| AccumulationRegisters | partial | 449 | 269 | 180 | 59.9 |
| Roles | done | 2220 | 2220 | 0 | 100.0 |
| ExchangePlans | partial | 366 | 221 | 145 | 60.4 |
| HTTPServices | partial | 10 | 8 | 2 | 80.0 |
| WebServices | partial | 36 | 18 | 18 | 50.0 |
| DocumentJournals | partial | 121 | 72 | 49 | 59.5 |
| DataProcessors | partial | 7058 | 5486 | 1572 | 77.73 |
| Tasks | partial | 49 | 26 | 23 | 53.1 |
| Catalogs | partial | 6705 | 3486 | 3219 | 52.0 |
| Documents | partial | 6219 | 3231 | 2988 | 52.0 |
| ChartsOfCharacteristicTypes | partial | 167 | 81 | 86 | 48.5 |
| InformationRegisters | partial | 3978 | 1855 | 2123 | 46.6 |
| CommonAttributes | partial | 7 | 3 | 4 | 42.9 |
| BusinessProcesses | partial | 152 | 66 | 86 | 43.4 |
| SettingsStorages | partial | 82 | 36 | 46 | 43.9 |
| CommonForms | partial | 1116 | 677 | 439 | 60.7 |
| Subsystems | partial | 766 | 219 | 547 | 28.6 |
| FilterCriteria | partial | 7 | 1 | 6 | 14.3 |
| Configuration.xml | partial | 1 | 0 | 1 | 0.0 |
| **Overall full snapshot** | **partial** | **49622** | **37396** | **12226** | **75.4** |

## Scope Exclusions

| Artifact | Decision | Reason |
|---|---|---|
| ConfigDumpInfo.xml | do not generate | Not required for the replacement export/import workflow; do not count it as parity debt. |

Note: `Configuration.xml` currently covers the root metadata header
(uuid/name/synonym/comment), source XML version selection, and selected root
child object headers (`CommonAttribute`, `CommonModule`, `Constant`, `Catalog`),
plus selected root refs such as default roles/style/language/settings storages.
Localized root information fields are also covered.
Deeper root properties are still tracked as Issue #22 follow-up work.

## Delegation History

| Issue | Scope | Status |
|---|---|---|
| #15 | Catalogs/Documents/DataProcessors/Reports metadata XML | merged to `master` |
| #18 | register/subsystem/exchange-plan metadata and auxiliary assets | merged to `master` |
| #16 | Form.xml `TextDocumentField/ReadOnly` layout packing | merged to `master` in round 9 |
| #15 | DataProcessor owned template child refs | merged to `master` in round 9 |
| #19 | selected command-interface targeted owner refs | merged to `master` in round 9; issue stays open pending real lab timing |
| #21 | CommonPicture/configuration picture staging without base blob fetch | merged to `master` in round 9 |
| #16 | Form.xml `TextDocumentField/Width` layout packing | merged to `master` in round 10 |
| #17 | MXL `style:FieldTextColor` extraction | merged to `master` in round 10 |
| #22 | Configuration.xml nested CommonAttribute child headers | merged to `master` in round 10 |
| #21 | Help.xml staging without active Config query | merged to `master` in round 10 |
| #16 | Form.xml new `CheckBoxField` compilation | merged to `master` in round 11 |
| #13 | omitted false Role Rights.xml entries | merged to `master` in round 11 |
| #18 | InformationRegister generated type InternalInfo | merged to `master` in round 11 |
| #21 | module body staging without active module blobs | merged to `master` in round 11 |
| #18 | AccumulationRegister generated type InternalInfo | merged to `master` in round 12 |
| #16 | Form.xml `Pages` / `Page` layout properties | merged to `master` in round 12 |
| #22 | Configuration.xml nested CommonModule child headers | merged to `master` in round 12 |
| #21 | ExchangePlan Content.xml staging without active Config query | merged to `master` in round 12 |
| #16 | Form.xml new `PictureDecoration` compilation | merged to `master` in round 13 |
| #15 | Catalog owned Form/Template child refs | merged to `master` in round 13 |
| #17 | SpreadsheetDocument `style:ButtonTextColor` pack/extract | merged to `master` in round 13 |
| #21 | SpreadsheetDocument Template.xml staging without active Config query | merged to `master` in round 13 |
| #16 | Form.xml new `InputField/ReadOnly` generation | merged to `master` in round 14 |
| #18 | BusinessProcess generated type InternalInfo | merged to `master` in round 14 |
| #14 | Task owned Form/Template child refs | merged to `master` in round 14 |
| #21 | Style.xml staging without active Config query | merged to `master` in round 14 |
| #16 | Form.xml `InputField/DataPath` packing | merged to `master` in round 15 |
| #18 | partial metadata owned Form/Template child refs | merged to `master` in round 15 |
| #14 | DocumentJournal owned Form/Template child refs | merged to `master` in round 15 |
| #21 | HTMLDocument template staging coverage without base blob fetch | merged to `master` in round 15 |
| #16 | Form.xml new `LabelField/ShowInHeader` generation | merged to `master` in round 16 |
| #18 | AccountingRegister/CalculationRegister generated type InternalInfo | merged to `master` in round 16 |
| #22 | Configuration.xml root Constant child headers | merged to `master` in round 16 |
| #21 | raw-id CommandInterface.xml staging without active Config query | merged to `master` in round 16 |
| #16 | Form.xml new `InputField/SkipOnInput` generation | merged to `master` in round 17 |
| #18 | Task generated type InternalInfo | merged to `master` in round 17 |
| #17 | SpreadsheetDocument `style:FieldSelectionBackColor` pack | merged to `master` in round 17 |
| #21 | configuration CommandInterface.xml raw-id base-free coverage | merged to `master` in round 17 |
| #16 | Form.xml new top-level `Button/DefaultButton` generation | merged to `master` in round 18 |
| #18 | ExchangePlan `UseStandardCommands` metadata XML | merged to `master` in round 18 |
| #22 | Configuration.xml root Catalog child headers | merged to `master` in round 18 |
| #21 | configuration MainSectionCommandInterface.xml raw-id base-free coverage | merged to `master` in round 18 |
| #16 | Form.xml new `InputField` extended options generation | merged to `master` in round 19 |
| #18 | Subsystem `UseStandardCommands` metadata XML | merged to `master` in round 19 |
| #21 | WSReference definition raw-deflated base-free coverage | merged to `master` in round 19 |
| #16 | Form.xml new `UsualGroup` extended properties generation | merged to `master` in round 20 |
| #18 | register-family `UseStandardCommands` metadata XML | merged to `master` in round 20 |
| #21 | configuration StandaloneConfigurationContent raw-deflated base-free coverage | merged to `master` in round 20 |
| #16 | Form.xml nested `AutoCommandBar` settings | merged to `master` in round 21 |
| #15 | DataProcessor `UseStandardCommands` metadata XML | merged to `master` in round 21 |
| #21 | configuration MobileClientSignature raw-deflated base-free coverage | merged to `master` in round 21 |
| #21 | precise form body base-blob blocker audit | merged to `master` in round 22 |
| #22 | root child families and CommonAttribute content/AutoUse metadata XML | merged to `master` in round 22 |
| #16 | Form command `ModifiesSavedData` extract/pack support | merged to `master` in round 22 |
| #23 | shared V8 container parser/builder extraction | merged to `master` in round 22 |
| #15 | Document owned Form/Template child refs | merged to `master` in round 23 |
| #16 | Form.xml wrapper `55` table `AutoRefresh` / `AutoRefreshPeriod` extraction and packing | merged to `master` in round 23 |
| #17 | SpreadsheetDocument `verticalUnmerge` merge-region extraction and packing | merged to `master` in round 23 |
| #18 | InformationRegister extended-tuple `UseStandardCommands` metadata XML | merged to `master` in round 23 |
| #19 | `mssql-dump-config` blob-bearing prepare fetches routed through native `bcp` parser with timing split | merged to `master` in round 23 |
| #21 | precise Role `Rights.xml` base-free staging blocker audit | merged to `master` in round 23 |
| #13 | Role Rights.xml object UUID ordering for export and pack mapping | merged to `master` in round 24 |
| #15 | Document `StandardAttributes` metadata XML | merged to `master` in round 24 |
| #16 | Form.xml table `SkipOnInput` extraction and packing | merged to `master` in round 24 |
| #17 | DataCompositionSchema template `TypeId` to `Type` reference normalization | merged to `master` in round 24 |
| #18 | ExchangePlan `Content.xml` `AutoRecord` mapping | merged to `master` in round 24 |
| #21 | precise BusinessProcess `Flowchart.xml` base-free staging blocker audit | merged to `master` in round 24 |
| #22 | CommonAttribute native content pair parsing | merged to `master` in round 24 |
| #13 | explicit disabled Role Rights.xml entries | merged to `master` in round 25 |
| #15 | Document numbering settings, standard command flag and default form refs | merged to `master` in round 25 |
| #16 | Form.xml wrapper `55` table `UseAlternationRowColor` extraction and packing | merged to `master` in round 25 |
| #17 | SpreadsheetDocument text-node quote escaping | merged to `master` in round 25 |
| #18 | InformationRegister default record/list form refs | merged to `master` in round 25 |
| #21 | precise Predefined.xml base-free staging blocker audit | merged to `master` in round 25 |
| #22 | root Configuration.xml scalar application/compatibility properties | merged to `master` in round 25 |
| #13 | Role Rights.xml tail-aware object refs and standard attribute refs | merged to `master` in round 26 |
| #15 | Catalog standard attribute labels and DataProcessor owner presentation metadata | merged to `master` in round 26 |
| #16 | Form.xml wrapper `55` table `UpdateOnDataChange=Auto` extraction and packing | merged to `master` in round 26 |
| #17 | DCS template body canonicalization for observed TypeSet/xsi:type values | merged to `master` in round 26 |
| #18 | AccumulationRegister and ExchangePlan generated type metadata XML | merged to `master` in round 26 |
| #19 | native dump `bcp` batch memory cap and batch diagnostics | merged to `master` in round 26 |
| #21 | precise readable `CommandInterface.xml` base-free blocker audit | merged to `master` in round 26 |
| #22 | root Configuration.xml default style/language refs | merged to `master` in round 26 |
| #13 | Role Rights.xml nested subsystem/integration-service child refs | merged to `master` in round 27 |
| #15 | Report command child headers and Catalog/DataProcessor Attribute/TabularSection child headers | merged to `master` in round 27 |
| #16 | Form.xml wrapper `55` table `AllowGettingCurrentRowURL` extraction and packing | merged to `master` in round 27 |
| #17 | DCS TypeId readiness from XML-shaped `xr:GeneratedType` metadata | merged to `master` in round 27 |
| #18 | BusinessProcess/Task generated types and AccumulationRegister include-help scalar | merged to `master` in round 27 |
| #19 | `mssql-dump-timing-summary` and selected blob timing evidence | merged to `master` in round 27 |
| #21 | explicit HomePageWorkArea base-free readiness classification | merged to `master` in round 27 |
| #22 | root Configuration.xml default role refs | merged to `master` in round 27 |
| #13 | Role Rights.xml string-backed restriction field export/import | merged to `master` in round 28 |
| #15 | Catalog/DataProcessor child attribute type refs and Report auxiliary-variant placeholder behavior | merged to `master` in round 28 |
| #16 | Form.xml wrapper `55` table `UserSettingsGroup` extraction and packing | merged to `master` in round 28 |
| #17 | selected template-owner `type_index` readiness for DCS TypeId normalization | merged to `master` in round 28 |
| #18 | AccumulationRegister `RegisterType` and ExchangePlan `Content.xml` file framing | merged to `master` in round 28 |
| #19 | standalone-content reference resolution without metadata XML and new ExchangePlan content blocker evidence | merged to `master` in round 28 |
| #21 | precise metadata XML row base-dependent readiness audit | merged to `master` in round 28 |
| #22 | CommonAttribute conditional-separation refs | merged to `master` in round 28 |
| #13 | Role Rights.xml serialized object order preservation | merged to `master` in round 29 |
| #15 | child `Attribute` property tails for object metadata XML | merged to `master` in round 29 |
| #16 | Form.xml wrapper `55` table `DefaultItem` extraction and packing | merged to `master` in round 29 |
| #17 | default-only spreadsheet `printSettings` suppression | merged to `master` in round 29 |
| #18 | AccumulationRegister `DataLockControlMode` and `FullTextSearch` owner properties | merged to `master` in round 29 |
| #19 | ExchangePlan `Content.xml` no-metadata-XML source refs and precise unsupported-id diagnostics | merged to `master` in round 29 |
| #21 | precise `versions` base-free staging blocker audit | merged to `master` in round 29 |
| #22 | CommonAttribute native property detail defaults | merged to `master` in round 29 |
| #13 | Role Rights.xml source-ref-based object mapping for import packing | merged to `master` in round 30 |
| #15 | child `Attribute` scalar tails and Report/DataProcessor child command properties | merged to `master` in round 30 |
| #16 | Form.xml wrapper `55` table `RowPictureDataPath` extraction and packing | merged to `master` in round 30 |
| #17 | DCS current-config type prefix normalization by context | merged to `master` in round 30 |
| #18 | register standard attributes and AccumulationRegister `Turnovers` enum mapping | merged to `master` in round 30 |
| #19 | ExchangePlan content direct object refs and successful full source-layout timing run | merged to `master` in round 30 |
| #21 | base-free FilterCriterion manager module staging | merged to `master` in round 30 |
| #22 | CommonAttribute separation tail properties and refs | merged to `master` in round 30 |
| #13 | Task `AddressingAttribute` Role Rights.xml refs and source packing | merged to `master` in round 31 |
| #15 | tabular-section property tails and child `<LinkByType/>` metadata XML | merged to `master` in round 31 |
| #16 | Form.xml wrapper `55` table `ChoiceFoldersAndItems` extraction and packing | merged to `master` in round 31 |
| #17 | case-insensitive DCS generated type id lookup | merged to `master` in round 31 |
| #18 | register child object decoded types and property tails | merged to `master` in round 31 |
| #19 | form source-asset CPU option-bag caching and after-run timing | merged to `master` in round 31 |
| #21 | WebService module base-free source staging coverage | merged to `master` in round 31 |
| #22 | Configuration.xml root settings-storage refs | merged to `master` in round 31 |
| #23 | shared V8 container parser/builder extraction | closed in round 22; kept as the required dependency for future Config/ConfigSave container work |
| #15 | Document owner `IncludeHelpInContents` metadata XML | merged to `master` in round 32 |
| #16 | Form.xml wrapper `55` table `RestoreCurrentRow` extraction and packing | merged to `master` in round 32 |
| #18 | ExchangePlan child attributes with value types and property tails | merged to `master` in round 32 |
| #19 | form child-item support-index performance optimization and after-run timing | merged to `master` in round 32 |
| #21 | precise unmapped `AdditionalIndexes.xml` base-free staging blocker audit | merged to `master` in round 32 |
| #22 | Configuration.xml localized root information fields | merged to `master` in round 32 |
| #13 | HTTPService URL template method Role Rights.xml refs and source packing | merged to `master` in round 33 |
| #15 | child `ChoiceParameters` metadata XML with design-time refs | merged to `master` in round 33 |
| #16 | Form.xml wrapper `55` table `Period` extraction and packing | merged to `master` in round 33 |
| #17 | DCS core/data-core `xsi:type` normalization | merged to `master` in round 33 |
| #18 | ExchangePlan `xr:ThisNode` and header-relative `UseStandardCommands` | merged to `master` in round 33 |
| #21 | base-free module `.bin` V8 container body staging | merged to `master` in round 33 |
| #15 | Catalog `<Owners>` metadata XML refs | merged to `master` in round 34 |
| #16 | Form.xml wrapper `55` table `RowFilter xsi:nil` extraction and packing | merged to `master` in round 34 |
| #17 | DCS calculated-field current-config namespace normalization | merged to `master` in round 34 |
| #18 | ExchangePlan `Content.xml` metadata-tree ordering | merged to `master` in round 34 |
| #21 | base-free `Ext/ParentConfigurations.bin` raw-deflated staging | merged to `master` in round 34 |
| #22 | CommonAttribute property-tail `FillValue` metadata XML | merged to `master` in round 34 |
| #13 | Role Rights.xml common/owned form refs and source packing | merged to `master` in round 35 |
| #15 | Catalog child attribute property tails and choice-parameter refs | merged to `master` in round 35 |
| #16 | Form.xml `ShowCommandBar=true` default suppression | merged to `master` in round 35 |
| #17 | MOXCEL unknown format-bit tolerance for MXL templates | merged to `master` in round 35 |
| #18 | BusinessProcess Flowchart `ZOrder` export | merged to `master` in round 35 |
| #19 | sampled XML-path diff-mining report for round35 planning | merged to `master` in round 35 |
| #21 | configuration application module base-free staging audit | merged to `master` in round 35 |
| #16 | Form.xml `Table/SkipOnInput=false` default suppression | merged to `master` in round 39 |
| #17 | MOXCEL report/table/button system color style refs | merged to `master` in round 39 |
| #18 | InformationRegister `DataLockControlMode` metadata XML | merged to `master` in round 39 |
| #21 | precise Form body section-count base dependency audit | merged to `master` in round 39 |
| #13 | Role Rights.xml metadata-order object sorting | merged to `master` in round 39 |
| #22 | generic `Configuration.xml` root child classifier for additional families | merged to `master` in round 40 |
| #15 | Catalog root `QuickChoice` / `ChoiceMode` metadata XML | merged to `master` in round 40 |
| #16 | Form.xml `TextDocumentField/ReadOnly=true` extraction | merged to `master` in round 40 |
| #21 | sectionless Form body native skeleton readiness audit | merged to `master` in round 40 |
| #19 | source-asset/Form CPU timing summary breakdown and existing evidence audit | merged to `master` in round 40 |
| #16 | Form.xml new `CheckBoxField/ShowInHeader` generation | merged to `master` in round 41 |
| #17 | MOXCEL number-format string table extraction and packing | merged to `master` in round 41 |
| #18 | BusinessProcess/Task `UseStandardCommands` metadata XML | merged to `master` in round 41 |
| #21 | precise `Predefined.xml` native-shape base dependency audit | merged to `master` in round 41 |
| #24 | SQL/BCP fetch helpers extracted to `src/mssql_dump/fetch.rs` | merged to `master` in round 41 |
| #16 | Form.xml `Table/DataPath` generation and matrix-diff candidate tooling | merged to `master` in round 42 |
| #17 | MOXCEL `verticalAlignment=Bottom` extraction and packing | merged to `master` in round 42 |
| #15 | Catalog root create/history tail metadata XML | merged to `master` in round 42 |
| #21 | precise metadata XML shape evidence for base dependency audit | merged to `master` in round 42 |
| #24 | selected-export planning helpers extracted to `src/mssql_dump/selected.rs` | merged to `master` in round 42 |

Worker result on #18: one selected subsystem `Ext/CommandInterface.xml` is byte-identical now, but the `Subsystems` group is still partial.
