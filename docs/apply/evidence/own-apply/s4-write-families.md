# S4: the writes of the native apply and of `mssql-config-apply`, family by family

Same stage on two twins of one БСП 8.3.27.2214 clone (16 staged rows: a new form and a new template of one
data processor, the owner's descriptor, ten module/form/template edits, `versions`; no `root`/`version`),
2026-09-29. Traces made with the capture kit of track trace (`scripts/apply-trace/trace.ps1`; the own run with
`-IncludeStatements`, since the whole apply is one batch), compared with
`scripts/apply-trace/compare_traces.py --capture native=... --capture own=...`. A cell is
"write statements / rows they affected / payload bytes" as the trace sees them; the own run's statements are
the parts of ONE batch in ONE transaction, and a statement that moves many rows shows `0 rows` because the
trace cannot count rows of a set-based statement.

Totals: native 195 write statements (of 7 223 statements in 24 user transactions, 75 s), the own apply 17
write statements in one transaction (`Config` 6, `Params` 5, `_ConfigChngR` 2, `Files` 2,
`_ConfigChngR_ExtProps` 1, `ConfigSave` 1). Native wrote nothing but what the table lists; the rows the own
apply leaves out are the `.ui` (3 rows), the `.new` copies and their promotion (`Config` is changed once, by
`INSERT ... SELECT`), the 16 `.sinew` cache rows and their promotion (the own apply rewrites the one
`.si` row that gains records), the markers `commit`/`dbStruFinal`, and the `_ExtensionsRestructNGS` clean-up.

| table | op | row name | native | own |
|---|---|---|---|---|
| Params | DELETE | `<guid>.ui` | 3 st / 0 rows |  |
| Params | UPDATE | `<guid>.ui` | 3 st / 3 rows / 23.8 KB |  |
| ConfigSave | DELETE | `LIKE %.new` | 1 st / 0 rows |  |
| Config | DELETE | `LIKE %.new` | 1 st / 0 rows |  |
| Config | DELETE | `<guid>.new <- ConfigSave` | 3 st / 0 rows |  |
| Config | INSERT | `<guid>.new <- ConfigSave` | 3 st / 3 rows |  |
| Config | DELETE | `<guid>.<n>.new <- ConfigSave` | 12 st / 0 rows |  |
| Config | INSERT | `<guid>.<n>.new <- ConfigSave` | 12 st / 12 rows |  |
| Config | DELETE | `versions.new <- ConfigSave` | 1 st / 0 rows |  |
| Config | INSERT | `versions.new <- ConfigSave` | 1 st / 1 rows |  |
| Files | INSERT | `MobileVersions.datNEW` | 1 st / 1 rows |  |
| Files | DELETE | `MobileVersions.datNEW` | 1 st / 0 rows |  |
| Files | UPDATE | `MobileVersions.datNEW` | 1 st / 1 rows / 36.1 KB |  |
| Params | INSERT | `<guid>.sinew` | 16 st / 16 rows |  |
| Params | DELETE | `<guid>.sinew` | 16 st / 0 rows |  |
| Params | UPDATE | `<guid>.sinew` | 16 st / 16 rows / 1.5 MB |  |
| _ExtensionsRestructNGS | DELETE |  | 1 st / 0 rows |  |
| Config | INSERT | `commit` | 1 st / 1 rows |  |
| Config | DELETE | `commit` | 2 st / 0 rows |  |
| Config | UPDATE | `commit` | 1 st / 1 rows |  |
| Params | DELETE | `<guid>.si <- Params` | 16 st / 16 rows |  |
| Params | UPDATE | `<guid>.sinew -> <guid>.si` | 16 st / 16 rows |  |
| Params | DELETE | `siVersions` | 1 st / 0 rows | 1 st / 0 rows |
| Params | UPDATE | `siVersions` | 1 st / 1 rows / 1.2 KB | 1 st / 0 rows / 1.2 KB |
| ConfigSave | DELETE |  |  | 1 st / 0 rows |
| Params | DELETE | `DynamicallyUpdated` | 1 st / 0 rows |  |
| Config | INSERT | `dbStruFinal` | 1 st / 1 rows |  |
| Config | DELETE | `dbStruFinal` | 2 st / 0 rows |  |
| Config | UPDATE | `dbStruFinal` | 1 st / 1 rows |  |
| _ConfigChngR | UPDATE |  | 1 st / 40 rows | 1 st / 0 rows |
| _ConfigChngR | INSERT |  | 6 st / 6 rows | 1 st / 0 rows |
| _ConfigChngR_ExtProps | INSERT |  | 6 st / 6 rows | 1 st / 0 rows |
| Files | DELETE | `MobileVersions.dat` |  | 1 st / 0 rows |
| Files | UPDATE | `MobileVersions.dat` |  | 1 st / 0 rows / 36.1 KB |
| Params | DELETE | `<guid>.si` |  | 1 st / 0 rows |
| Params | UPDATE | `<guid>.si` |  | 1 st / 0 rows / 439.0 KB |
| Config | DELETE | `<guid> <- Config` | 5 st / 3 rows |  |
| Config | UPDATE | `<guid>_dynupdate_<guid> -> <guid>` | 2 st / 2 rows |  |
| Config | DELETE | `<guid>.<n> <- Config` | 14 st / 12 rows |  |
| Config | UPDATE | `<guid>_dynupdate_<guid>.<n> -> <guid>.<n>` | 2 st / 2 rows |  |
| Config | DELETE | `versions_dynupdate_<guid>` | 1 st / 0 rows |  |
| Config | DELETE | `deleted_dynupdate_<guid>` | 1 st / 0 rows |  |
| Config | UPDATE | `<guid>.<n>.new -> <guid>.<n>` | 12 st / 12 rows |  |
| Config | UPDATE | `<guid>.new -> <guid>` | 3 st / 3 rows |  |
| Config | DELETE | `versions <- Config` | 1 st / 1 rows |  |
| Config | UPDATE | `versions.new -> versions` | 1 st / 1 rows |  |
| Config | DELETE | `DynamicallyUpdated` | 1 st / 0 rows |  |
| ConfigSave | DELETE | `LIKE %` | 1 st / 0 rows |  |
| Files | DELETE | `MobileVersions.dat <- Files` | 1 st / 1 rows |  |
| Files | UPDATE | `MobileVersions.datNEW -> MobileVersions.dat` | 1 st / 1 rows |  |
| Config | DELETE | `dynamicCommit` | 1 st / 0 rows |  |
| Config | DELETE | `(SELECT LEFT(FileName, CHARINDEX(N'_dynupdate_', FileName) - 1) + SUBSTRING(FileName, CHARINDEX(N'_dynupdate_', FileName) + 47, 4000) FROM dbo.Config WHERE FileName LIKE N'%!_dynupdate!_<guid>%' ESCAPE N'!' AND FileName NOT LIKE N'versions!_dynupdate!_%' ESCAPE N'!' AND FileName NOT LIKE N'deleted!_dynupdate!_%' ESCAPE N'!');` |  | 1 st / 0 rows |
| Config | UPDATE | `%!_dynupdate!_<guid>%' ESCAPE N'! -> LEFT(FileName, CHARINDEX(N'_dynupdate_', FileName) - 1) + SUBSTRING(FileName, CHARINDEX(N'_dynupdate_', FileName) + 47, 4000)` |  | 1 st / 0 rows |
| Config | DELETE | `versions_dynupdate_<guid>'` |  | 1 st / 0 rows |
| Config | DELETE | `DynamicallyUpdated'` |  | 1 st / 0 rows |
| Params | DELETE | `DynamicallyUpdated'` |  | 1 st / 0 rows |
| Config | DELETE | `(SELECT FileName FROM dbo.ConfigSave);` |  | 1 st / 0 rows |
| Config | INSERT | `FileName <- ConfigSave` |  | 1 st / 0 rows |

DDL statements by verb:

| verb | native | own |
|---|---|---|
