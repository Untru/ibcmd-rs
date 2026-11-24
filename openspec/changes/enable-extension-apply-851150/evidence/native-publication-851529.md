# SQL extension import and apply on 8.5.1.1529

On 2026-11-24 native Windows x64 ibcmd 8.5.1.1529 and local SQL Server were
used with two disposable databases restored from the same initialized state.
The native tool created the customization MeasureExt with its default role.
Both initial exports had identical paths and bytes for Configuration.xml,
Roles/ОсновнаяРоль.xml and ConfigDumpInfo.xml. No BSP database was used.

## Reproduction

1. On the seed, use native infobase config extension create with
   --name=MeasureExt --name-prefix=MeasureExt_, then config apply
   --extension=MeasureExt. Back up the database and restore the twin.
   Use a separate --data directory for each database.
2. Export MeasureExt. In a copy, replace the empty Version element with
   <Version>1.1</Version>, preserving all other bytes, including dump info.
3. On the reference database, run native infobase config import
   --extension=MeasureExt with the whole directory, export to a fresh
   directory, run config apply --extension=MeasureExt, then export again.
4. On the other database, pass the same directory to ibcmd-rs infobase
   config import and run config apply with --extension=MeasureExt,
   --platform=8.5.1.1529, the SQL target and syntactically valid cluster
   and infobase IDs. Profile selection refuses before contacting RAS.
   Also stage the same directory with native import and repeat OWN apply.

## Observations

| Native reference | ConfigCASSave rows | ConfigCAS rows | SHA-256 of MeasureExt registry payload (prefix) |
| --- | ---: | ---: | --- |
| Before import | 5 | 3 | 5aa5b0dc335d |
| After import | 8 | 3 | 5aa5b0dc335d |
| After apply | 5 | 6 | f2be13b29be5 |

The five pre-existing staging rows belong to a separate extension left
staged after a native validation failure. The counts cover both extensions.
Native import and apply exited successfully. Both exports contained the
edited version and were byte-identical across all three files. Compared
with the input directory, native regenerated only ConfigDumpInfo.xml.
The published generation was e5a6e82e67163e82b4832b7f14ab4903ea8351d9.

The OWN commands refused the exact 1529 target. The native export after
the refusals matched the baseline across all three files. The sorted
filename, part number and SHA-256 of every staging and active CAS payload
and the hash of the target registry payload were unchanged. With a native
stage present, OWN apply also refused and retained the staged data.

This measures one native metadata-only update and the OWN refusal. The
SQL writer on 1529 still needs a measured storage profile, successful OWN
staging and publication, whole-tree comparison and session visibility.
The larger adopted-form regression corpus is tracked separately in #414.
