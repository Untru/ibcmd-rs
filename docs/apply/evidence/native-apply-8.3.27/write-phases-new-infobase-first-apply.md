# Write phases

53466 writes/DDL statements in 6650 blocks (small blocks merged to 80). A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:13.589 .. +00:13.678 | 56 | (several) | 3 | 1 | begin 1, commit 1 | _DbCopiesInfoBaseUse x1; _ExtensionsRestructNGS x1; ConfigSave x1 |  |
| 2 | +00:13.679 .. +00:36.789 | 56 | Config | 19677 | 9853 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x4929; INSERT <guid>.new <- ConfigSave x4929; DELETE <guid>.<n>.new <- ConfigSave x4888; INSERT <guid>.<n>.new <- ConfigSave x4888; DELETE <guid>.1c.new <- ConfigSave x14; INSERT <guid>.1c.new <- ConfigSave x14; DELETE <guid>.a.new <- ConfigSave x1; ... +13 more | `LIKE %.new` .. `versions.new` |
| 3 | +00:36.792 .. +00:36.796 | 56 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 4 | +00:36.802 .. +00:36.810 | 56 | Params | 4 | 2 | begin 1, commit 1 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1 | `DBNames.New` |
| 5 | +00:37.281 .. +01:08.683 | 56 | (several) | 7013 | 2 | begin 3, commit 3 | _Node57NG x3; _Reference60NG x2; _ReferenceChngR360NG x2; _Reference61NG x2; _ReferenceChngR365NG x2; _Reference62NG x2 |  |
| 6 | +01:08.813 .. +01:08.818 | 56 | _RefSInf848NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 7 | +01:08.832 .. +01:08.851 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 8 | +01:08.886 .. +01:08.890 | 56 | _RefSInf955NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 9 | +01:08.913 .. +01:08.932 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 10 | +01:08.949 .. +01:08.955 | 56 | _RefSInf975NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 11 | +01:08.981 .. +01:09.004 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 12 | +01:09.024 .. +01:09.027 | 56 | _RefSInf985NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 13 | +01:09.046 .. +01:09.066 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 14 | +01:09.099 .. +01:09.101 | 56 | _RefSInf1101NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 15 | +01:09.117 .. +01:09.132 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 16 | +01:09.143 .. +01:09.146 | 56 | _RefSInf1125NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 17 | +01:09.160 .. +01:09.179 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 18 | +01:09.232 .. +01:09.240 | 56 | _RefSInf1390NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 19 | +01:09.260 .. +01:09.276 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 20 | +01:09.315 .. +01:09.319 | 56 | _RefSInf1476NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 21 | +01:09.341 .. +01:09.361 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 22 | +01:09.455 .. +01:09.459 | 56 | _RefSInf1721NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 23 | +01:09.481 .. +01:09.501 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 24 | +01:09.521 .. +01:09.525 | 56 | _RefSInf1781NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 25 | +01:09.542 .. +01:09.566 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 26 | +01:09.582 .. +01:09.585 | 56 | _RefSInf1794NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 27 | +01:09.603 .. +01:09.622 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 28 | +01:09.659 .. +01:09.662 | 56 | _RefSInf1887NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 29 | +01:09.681 .. +01:09.699 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 30 | +01:09.717 .. +01:09.720 | 56 | _RefSInf1899NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 31 | +01:09.739 .. +01:09.752 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 32 | +01:09.792 .. +01:09.797 | 56 | _RefSInf2000NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 33 | +01:09.818 .. +01:10.783 | 56 | (several) | 74 | 10 |  | SchemaStorage x10; _DataHistoryLatestVersions1_1NG x3; _DataHistoryLatestVersions2_1NG x3; _DataHistorySettings_1NG x3; _DataHistoryMetadataNG x3; _DataHistoryMetadata_1NG x3 |  |
| 34 | +01:10.792 .. +01:10.797 | 56 | _CKindsSInf2857NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 35 | +01:10.818 .. +01:10.836 | 56 | (several) | 1 | 1 |  | SchemaStorage x1 |  |
| 36 | +01:10.853 .. +01:10.858 | 56 | _SystemSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 37 | +01:10.859 .. +01:10.924 | 56 | (several) | 8 | 1 |  | _SystemSettings_1NG x3; _SystemSettings_2NG x3; SchemaStorage x1; _SystemSettingsNG x1 |  |
| 38 | +01:10.926 .. +01:10.931 | 56 | _CommonSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 39 | +01:10.932 .. +01:10.995 | 56 | (several) | 8 | 1 |  | _CommonSettings_1NG x3; _CommonSettings_2NG x3; SchemaStorage x1; _CommonSettingsNG x1 |  |
| 40 | +01:10.997 .. +01:11.001 | 56 | _RepSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 41 | +01:11.002 .. +01:11.076 | 56 | (several) | 8 | 1 |  | _RepSettings_1NG x3; _RepSettings_2NG x3; SchemaStorage x1; _RepSettingsNG x1 |  |
| 42 | +01:11.077 .. +01:11.080 | 56 | _RepVarSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 43 | +01:11.081 .. +01:11.130 | 56 | (several) | 8 | 1 |  | _RepVarSettings_1NG x3; _RepVarSettings_2NG x3; SchemaStorage x1; _RepVarSettingsNG x1 |  |
| 44 | +01:11.131 .. +01:11.135 | 56 | _FrmDtSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 45 | +01:11.136 .. +01:11.183 | 56 | (several) | 8 | 1 |  | _FrmDtSettings_1NG x3; _FrmDtSettings_2NG x3; SchemaStorage x1; _FrmDtSettingsNG x1 |  |
| 46 | +01:11.184 .. +01:11.187 | 56 | _DynListSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 47 | +01:11.188 .. +01:11.239 | 56 | (several) | 8 | 1 |  | _DynListSettings_1NG x3; _DynListSettings_2NG x3; SchemaStorage x1; _DynListSettingsNG x1 |  |
| 48 | +01:11.241 .. +01:11.244 | 56 | _ErrorProcessingSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 49 | +01:11.245 .. +01:11.293 | 56 | (several) | 8 | 1 |  | _ErrorProcessingSettings_1NG x3; _ErrorProcessingSettings_2NG x3; SchemaStorage x1; _ErrorProcessingSettingsNG x1 |  |
| 50 | +01:11.295 .. +01:11.299 | 56 | _URLExternalDataNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 51 | +01:11.300 .. +01:11.367 | 56 | (several) | 8 | 1 |  | _URLExternalData_1NG x3; _URLExternalData_2NG x3; SchemaStorage x1; _URLExternalDataNG x1 |  |
| 52 | +01:11.369 .. +01:11.371 | 56 | _InternalSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 53 | +01:11.372 .. +01:11.417 | 56 | (several) | 8 | 1 |  | _InternalSettings_1NG x3; _InternalSettings_2NG x3; SchemaStorage x1; _InternalSettingsNG x1 |  |
| 54 | +01:11.417 .. +01:11.420 | 56 | _DefaultSystemSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 55 | +01:11.421 .. +01:11.474 | 56 | (several) | 8 | 1 |  | _DefaultSystemSettings_1NG x3; _DefaultSystemSettings_2NG x3; SchemaStorage x1; _DefaultSystemSettingsNG x1 |  |
| 56 | +01:11.475 .. +01:11.479 | 56 | _DefaultInternalSettingsNG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 57 | +01:11.481 .. +01:21.298 | 56 | (several) | 949 | 779 | begin 127, commit 127 | SchemaStorage x73; _DefaultInternalSettings_1NG x3; _DefaultInternalSettings_2NG x3; _ScheduledJobs6346NG x3; _ScheduledJobs6346_1NG x3; _ScheduledJobs6347NG x3 |  |
| 58 | +01:21.312 .. +01:21.315 | 56 | _ChrcChngR6056NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 59 | +01:21.317 .. +01:21.357 | 56 | (several) | 3 | 1 |  | _ChrcChngR6056_1NG x1; _ChrcChngR6056_2NG x1; SchemaStorage x1 |  |
| 60 | +01:21.368 .. +01:21.374 | 56 | _ChrcChngR6080NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 61 | +01:21.374 .. +01:21.428 | 56 | (several) | 3 | 1 |  | _ChrcChngR6080_1NG x1; _ChrcChngR6080_2NG x1; SchemaStorage x1 |  |
| 62 | +01:21.438 .. +01:21.441 | 56 | _ChrcChngR6128NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 63 | +01:21.442 .. +01:21.479 | 56 | (several) | 3 | 1 |  | _ChrcChngR6128_1NG x1; _ChrcChngR6128_2NG x1; SchemaStorage x1 |  |
| 64 | +01:21.492 .. +01:21.495 | 56 | _ChrcChngR6130NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 65 | +01:21.496 .. +01:21.628 | 56 | (several) | 15 | 13 | begin 1, commit 2 | SchemaStorage x2; _BPrPoints329NG x2; _BPrPoints329_1NG x2; _BPrPoints331NG x2; _BPrPoints331_1NG x2; _ODataSettingsNG x2 |  |
| 66 | +01:21.639 .. +01:21.642 | 56 | _AccSInf2878NG | 3 | 0 |  | DROP TABLE x1; CREATE TABLE x1; ALTER TABLE x1 |  |
| 67 | +01:21.666 .. +01:22.101 | 56 | (several) | 29 | 7 | begin 6, commit 6 | SchemaStorage x5; _ExtensionsRestructNG x3; _ExtensionsRestructNGSNG x3; _ExtensionsInfoNG x3; _ExtensionsInfoNGSNG x3; _AccRgOpt2931NG x2 |  |
| 68 | +01:22.117 .. +01:22.131 | 56 | Files | 6 | 4 | begin 2, commit 2 | INSERT extd_props_cached/gc.mrk x1; DELETE extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1; INSERT CAS_GC_Info x1; DELETE CAS_GC_Info x1; UPDATE CAS_GC_Info x1 | `extd_props_cached/gc.mrk` .. `CAS_GC_Info` |
| 69 | +01:22.133 .. +01:22.154 | 56 | Params | 3 | 2 | begin 1, commit 1 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1 | `DBNames.New` |
| 70 | +01:22.162 .. +01:22.652 | 56 |  | 19 | 0 | begin 19, commit 19 | RENAME exec sp_rename 'v8users', 'v8usersOG', 'object' x1; RENAME exec sp_rename 'v8usersOG.ByName', 'ByNameOG', 'index' x1; RENAME exec sp_rename 'v8usersOG.ByDescr', 'ByDescrOG', 'index' x1; RENAME exec sp_rename 'v8usersOG.ByOSName', 'ByOSNameOG', 'index' x1; RENAME exec sp_rename 'v8usersOG.ByRolesID', 'ByRolesIDOG', 'index' x1; RENAME exec sp_rename 'v8usersOG.ByShow', 'ByShowOG', 'index' x1; RENAME exec sp_rename 'v8usersOG.ByEAuth', 'ByEAuthOG', 'index' x1; RENAME exec sp_rename 'v8usersOG.ByEmail_V8USERS', 'ByEmail_V8USERSOG', 'index' x1; ... +11 more |  |
| 71 | +01:25.348 .. +01:27.035 | 56 | Params | 48 | 32 | begin 16, commit 16 | INSERT <guid>.sinew x16; DELETE <guid>.sinew x16; UPDATE <guid>.sinew x16 | `42ed49cc-765d-4314-bc2d-af425af7bf13.sinew` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 72 | +01:27.224 .. +01:27.230 | 56 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 73 | +01:27.239 .. +01:27.344 | 56 | (several) | 31 | 2 |  | SchemaStorage x2; _STTSettings x1; _STTGrammar x1; _STTGrammarChecksum x1; _STTModels x1; _STTModelsDesc x1 |  |
| 74 | +01:27.348 .. +01:42.537 | 56 |  | 5488 | 0 | begin 5488, commit 5488 | RENAME exec sp_rename '_Reference60NG', '_Reference60', 'object' x1; RENAME exec sp_rename '_Reference60._Reference60_1NG', '_Reference60_1', 'index' x1; RENAME exec sp_rename '_Reference60._Reference60_2NG', '_Reference60_2', 'index' x1; RENAME exec sp_rename '_Reference60._Reference60_3NG', '_Reference60_3', 'index' x1; RENAME exec sp_rename '_Reference60._Reference60_4NG', '_Reference60_4', 'index' x1; RENAME exec sp_rename '_Reference60._Reference60_5NG', '_Reference60_5', 'index' x1; RENAME exec sp_rename '_Reference60._Reference60_6NG', '_Reference60_6', 'index' x1; RENAME exec sp_rename '_Reference60._Reference60_7NG', '_Reference60_7', 'index' x1; ... +5480 more |  |
| 75 | +01:42.538 .. +01:46.715 | 56 | (several) | 179 | 2 |  | DBSchema x2; v8usersOG x1; IBVersion x1; Config x1; ConfigSave x1; Params x1 |  |
| 76 | +01:46.752 .. +01:46.841 | 56 | Params | 42 | 22 | begin 3, commit 3 | DELETE DBNames <- Params x1; UPDATE DBNames.New -> DBNames x1; DELETE DBNamesVersion-DBNames x1; UPDATE DBNamesVersion-DBNames x1; INSERT siVersions x1; DELETE siVersions x2; UPDATE siVersions x2; DELETE <guid>.si <- Params x16; ... +2 more | `DBNames` .. `DynamicallyUpdated` |
| 77 | +01:46.949 .. +01:58.584 | 56 | Config | 19677 | 9846 |  | DELETE <guid> <- Config x4929; UPDATE <guid>.new -> <guid> x4929; DELETE <guid>.<n> <- Config x4888; UPDATE <guid>.<n>.new -> <guid>.<n> x4888; DELETE <guid>.1c <- Config x14; UPDATE <guid>.1c.new -> <guid>.1c x14; DELETE <guid>.a <- Config x1; UPDATE <guid>.a.new -> <guid>.a x1; ... +13 more | `000932fc-f508-4326-93c6-e2351be1cf25` .. `DynamicallyUpdated` |
| 78 | +01:58.591 .. +02:02.457 | 56 | (several) | 3 | 1 |  | Files x2; ConfigSave x1 |  |
| 79 | +02:02.458 .. +02:02.461 | 56 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 80 | +02:02.592 .. +02:02.593 | 56 | (several) | 1 | 1 |  | _Const3050 x1 |  |
