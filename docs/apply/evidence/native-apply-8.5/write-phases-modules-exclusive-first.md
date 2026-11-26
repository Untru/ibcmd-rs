# Write phases

19599 writes/DDL statements in 304 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:19.073 .. +00:19.092 | 144 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:20.464 .. +00:20.467 | 144 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:20.473 .. +00:20.767 | 144 | Config | 83 | 41 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x15; INSERT <guid>.new <- ConfigSave x15; DELETE <guid>.<n>.new <- ConfigSave x23; INSERT <guid>.<n>.new <- ConfigSave x23; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 4 | +00:20.792 .. +00:20.812 | 144 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +00:21.007 .. +00:21.100 | 144 | Params | 16 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x2; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x4; ... +1 more | `DBNames.New` .. `DBNames-Ext-d0d081f9-a401-11f1-8f51-00e04c68009...` |
| 6 | +00:25.158 .. +00:25.160 | 102 | _DbCopiesInfoBaseUse | 1 | 1 | begin 1 | UPDATE x1 |  |
| 7 | +00:25.161 .. +00:25.189 | 102 | _DbCopiesInitialLast | 1 | 0 |  | DELETE x1 |  |
| 8 | +00:25.190 .. +00:25.206 | 102 | _DbCopiesUpdates | 1 | 0 |  | DELETE x1 |  |
| 9 | +00:25.217 .. +00:25.224 | 102 | _DbCopiesTrChObj | 2 | 0 |  | TRUNCATE x2 |  |
| 10 | +00:25.224 .. +00:25.227 | 102 | _DbCopiesTrChanges | 2 | 0 | commit 1 | TRUNCATE x2 |  |
| 11 | +00:25.232 .. +00:25.236 | 102 | _ExtensionsRestructNGS | 1 | 0 |  | DELETE x1 |  |
| 12 | +00:25.323 .. +00:25.360 | 102 | _ConfigChngRNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 13 | +00:25.366 .. +00:25.383 | 102 | _ConfigChngR_ExtPropsNG | 2 | 0 |  | CREATE TABLE x1; ALTER TABLE x1 |  |
| 14 | +00:25.575 .. +00:25.584 | 102 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 15 | +00:25.587 .. +00:25.594 | 102 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 16 | +00:25.596 .. +00:25.651 | 102 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 17 | +00:25.667 .. +00:25.682 | 102 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 18 | +00:25.703 .. +00:25.711 | 102 | _ConfigChngR_1NG | 1 | 0 |  | DROP INDEX x1 |  |
| 19 | +00:25.727 .. +00:25.734 | 102 | _ConfigChngR_2NG | 1 | 0 |  | DROP INDEX x1 |  |
| 20 | +00:25.770 .. +00:25.783 | 102 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | DROP INDEX x1 |  |
| 21 | +00:26.172 .. +00:26.370 | 102 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 22 | +00:26.377 .. +00:26.455 | 102 | _ConfigChngR_ExtPropsNG | 2 | 10007 |  | INSERT BULK x2 |  |
| 23 | +00:26.697 .. +00:26.738 | 102 | _ConfigChngRNG | 1 | 10000 |  | INSERT BULK x1 |  |
| 24 | +00:26.741 .. +00:26.931 | 102 | _ConfigChngR_ExtPropsNG | 2 | 10469 |  | INSERT BULK x2 |  |
| 25 | +00:27.285 .. +00:27.308 | 102 | _ConfigChngRNG | 1 | 1219 |  | INSERT BULK x1 |  |
| 26 | +00:27.312 .. +00:27.326 | 102 | _ConfigChngR_ExtPropsNG | 1 | 1387 |  | INSERT BULK x1 |  |
| 27 | +00:27.339 .. +00:27.436 | 102 | _ConfigChngR_1NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 28 | +00:27.443 .. +00:27.508 | 102 | _ConfigChngR_2NG | 1 | 0 |  | CREATE INDEX x1 |  |
| 29 | +00:27.510 .. +00:27.569 | 102 | _ConfigChngR_ExtProps_SKNG | 1 | 0 |  | CREATE INDEX x1 |  |
| 30 | +00:27.591 .. +00:27.611 | 102 | _ExtensionsRestruct | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 31 | +00:28.756 .. +00:28.769 | 102 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 32 | +00:28.988 .. +00:29.092 | 102 | _ExtensionsRestructNGS | 4 | 2 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 33 | +00:30.917 .. +00:30.918 | 102 | Files | 2 | 1 | begin 1, commit 1 | DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 34 | +00:30.991 .. +00:31.044 | 102 | _ExtensionsRestructNGS | 4 | 3 | begin 2, commit 2 | DELETE x2; INSERT x2 |  |
| 35 | +00:31.118 .. +00:31.128 | 102 | Files | 4 | 2 | begin 1, commit 1 | DELETE extd_props_cached/gc.mrk x2; INSERT extd_props_cached/gc.mrk x1; UPDATE extd_props_cached/gc.mrk x1 | `extd_props_cached/gc.mrk` |
| 36 | +00:31.320 .. +04:10.531 | 102 | ConfigCAS | 18629 | 0 |  | DELETE <sha1> x18627; DELETE dbStruFinal<guid> x2 | `0000d5fcfc5b2b5802c550c295f213a27b6a1e2e` .. `fffc5eb0c658c10c91390792cf8d395038767388` |
| 37 | +04:10.550 .. +04:12.325 | 102 | Files | 329 | 1 | begin 1, commit 1 | DELETE userDocs_ru_<sha1>.bin x109; DELETE userPostings_ru_<sha1>.bin x109; DELETE userVocabulary_ru_<sha1>.bin x109; DELETE CAS_GC_Info x1; UPDATE CAS_GC_Info x1 | `userDocs_ru_0572c8243652bf309b7cac6a78c840f14c7...` .. `CAS_GC_Info` |
| 38 | +04:12.338 .. +04:12.381 | 102 | Params | 12 | 8 | begin 4, commit 4 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1; INSERT DBNames-Ext-1.New x1; DELETE DBNames-Ext-1.New x1; UPDATE DBNames-Ext-1.New x1; INSERT DBNames-Ext-<guid>.New x2; DELETE DBNames-Ext-<guid>.New x2; ... +1 more | `DBNames.New` .. `DBNames-Ext-d0d081f9-a401-11f1-8f51-00e04c68009...` |
| 39 | +04:18.478 .. +04:21.364 | 102 | Params | 48 | 32 | begin 16, commit 16 | INSERT <guid>.sinew x16; DELETE <guid>.sinew x16; UPDATE <guid>.sinew x16 | `42ed49cc-765d-4314-bc2d-af425af7bf13.sinew` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 40 | +04:21.637 .. +04:21.656 | 102 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 41 | +04:21.676 .. +04:21.723 | 102 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 42 | +04:21.811 .. +04:21.974 | 102 | _ConfigChngR | 1 | 0 |  | DROP TABLE x1 |  |
| 43 | +04:21.994 .. +04:22.024 | 102 | _ConfigChngR_ExtProps | 1 | 0 |  | DROP TABLE x1 |  |
| 44 | +04:22.028 .. +04:22.033 | 102 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 45 | +04:22.065 .. +04:28.044 | 102 |  | 5 | 0 | begin 5, commit 5 | RENAME exec sp_rename '_ConfigChngRNG', '_ConfigChngR', 'object' x1; RENAME exec sp_rename '_ConfigChngR_ExtPropsNG', '_ConfigChngR_ExtProps', 'object' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_1NG', '_ConfigChngR_1', 'index' x1; RENAME exec sp_rename '_ConfigChngR._ConfigChngR_2NG', '_ConfigChngR_2', 'index' x1; RENAME exec sp_rename '_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG', '_ConfigChngR_ExtPro... x1 |  |
| 46 | +04:38.664 .. +04:38.685 | 102 | IBVersion | 1 | 0 |  | ALTER TABLE x1 |  |
| 47 | +04:38.689 .. +04:38.719 | 102 | _YearOffset | 1 | 0 |  | ALTER TABLE x1 |  |
| 48 | +04:38.726 .. +04:38.738 | 102 | DBSchema | 1 | 0 |  | ALTER TABLE x1 |  |
| 49 | +04:38.744 .. +04:38.855 | 102 | Config | 1 | 0 |  | ALTER TABLE x1 |  |
| 50 | +04:38.861 .. +04:38.908 | 102 | ConfigCASSave | 1 | 0 |  | ALTER TABLE x1 |  |
| 51 | +04:38.909 .. +04:39.096 | 102 | ConfigCAS | 1 | 0 |  | ALTER TABLE x1 |  |
| 52 | +04:39.103 .. +04:39.139 | 102 | ConfigSave | 1 | 0 |  | ALTER TABLE x1 |  |
| 53 | +04:39.143 .. +04:39.177 | 102 | DepotFiles | 1 | 0 |  | ALTER TABLE x1 |  |
| 54 | +04:39.184 .. +04:39.191 | 102 | Files | 1 | 0 |  | ALTER TABLE x1 |  |
| 55 | +04:39.200 .. +04:39.207 | 102 | Params | 1 | 0 |  | ALTER TABLE x1 |  |
| 56 | +04:39.554 .. +04:39.588 | 102 | PK___Enum427__AC8ED0C4E471CE32 | 1 | 0 |  | ALTER INDEX x1 |  |
| 57 | +04:39.589 .. +04:39.620 | 102 | PK___STTMode__AC8ED0C49C44B251 | 1 | 0 |  | ALTER INDEX x1 |  |
| 58 | +04:39.627 .. +04:39.637 | 102 | PK___Enum100__AC8ED0C45E0342FD | 1 | 0 |  | ALTER INDEX x1 |  |
| 59 | +04:39.642 .. +04:39.663 | 102 | PK___STTMode__AC8ED0C45F7FB97C | 1 | 0 |  | ALTER INDEX x1 |  |
| 60 | +04:39.667 .. +04:39.695 | 102 | PK___Enum100__AC8ED0C49A4E6A63 | 1 | 0 |  | ALTER INDEX x1 |  |
| 61 | +04:39.697 .. +04:39.712 | 102 | PK___Enum439__AC8ED0C4B1DC8364 | 1 | 0 |  | ALTER INDEX x1 |  |
| 62 | +04:39.717 .. +04:39.783 | 102 | PK___Referen__AC8ED0C414E25D5C | 1 | 0 |  | ALTER INDEX x1 |  |
| 63 | +04:39.787 .. +04:39.818 | 102 | PK___Enum816__AC8ED0C4605B71D8 | 1 | 0 |  | ALTER INDEX x1 |  |
| 64 | +04:39.823 .. +04:39.903 | 102 | PK___Documen__AC8ED0C4B56045C7 | 1 | 0 |  | ALTER INDEX x1 |  |
| 65 | +04:39.907 .. +04:39.945 | 102 | PK___Enum157__AC8ED0C4AB4B0BA2 | 1 | 0 |  | ALTER INDEX x1 |  |
| 66 | +04:39.949 .. +04:40.016 | 102 | PK___Referen__AC8ED0C4B06D7351 | 1 | 0 |  | ALTER INDEX x1 |  |
| 67 | +04:40.018 .. +04:40.037 | 102 | PK___Referen__AC8ED0C4954F487F | 1 | 0 |  | ALTER INDEX x1 |  |
| 68 | +04:40.041 .. +04:40.067 | 102 | PK___Referen__AC8ED0C47A83FBDD | 1 | 0 |  | ALTER INDEX x1 |  |
| 69 | +04:40.074 .. +04:40.145 | 102 | PK___Enum371__AC8ED0C4D7BE94C0 | 1 | 0 |  | ALTER INDEX x1 |  |
| 70 | +04:40.150 .. +04:40.215 | 102 | PK___Enum260__AC8ED0C48FED8814 | 1 | 0 |  | ALTER INDEX x1 |  |
| 71 | +04:40.222 .. +04:40.307 | 102 | PK___Enum428__AC8ED0C42D71A7A1 | 1 | 0 |  | ALTER INDEX x1 |  |
| 72 | +04:40.312 .. +04:40.325 | 102 | PK___Enum476__AC8ED0C490181B18 | 1 | 0 |  | ALTER INDEX x1 |  |
| 73 | +04:40.330 .. +04:40.462 | 102 | PK___Referen__AC8ED0C447F49EB3 | 1 | 0 |  | ALTER INDEX x1 |  |
| 74 | +04:40.467 .. +04:40.490 | 102 | PK___Enum102__AC8ED0C4D7D85CAC | 1 | 0 |  | ALTER INDEX x1 |  |
| 75 | +04:40.494 .. +04:40.552 | 102 | PK___Enum844__AC8ED0C450326831 | 1 | 0 |  | ALTER INDEX x1 |  |
| 76 | +04:40.556 .. +04:40.605 | 102 | PK___Referen__AC8ED0C432417EF2 | 1 | 0 |  | ALTER INDEX x1 |  |
| 77 | +04:40.610 .. +04:40.677 | 102 | PK___Enum102__AC8ED0C43DA522BD | 1 | 0 |  | ALTER INDEX x1 |  |
| 78 | +04:40.683 .. +04:40.703 | 102 | PK___Node259__AC8ED0C4743321F0 | 1 | 0 |  | ALTER INDEX x1 |  |
| 79 | +04:40.708 .. +04:40.775 | 102 | PK___Enum744__AC8ED0C44A16F866 | 1 | 0 |  | ALTER INDEX x1 |  |
| 80 | +04:40.779 .. +04:40.817 | 102 | PK___Referen__AC8ED0C40178721D | 1 | 0 |  | ALTER INDEX x1 |  |
| 81 | +04:40.825 .. +04:40.863 | 102 | PK___Enum775__AC8ED0C40D616430 | 1 | 0 |  | ALTER INDEX x1 |  |
| 82 | +04:40.867 .. +04:40.901 | 102 | PK___Enum744__AC8ED0C4CF6A1B96 | 1 | 0 |  | ALTER INDEX x1 |  |
| 83 | +04:40.905 .. +04:40.957 | 102 | PK___Referen__AC8ED0C49CE37484 | 1 | 0 |  | ALTER INDEX x1 |  |
| 84 | +04:40.960 .. +04:41.008 | 102 | PK___Referen__AC8ED0C4D042F776 | 1 | 0 |  | ALTER INDEX x1 |  |
| 85 | +04:41.014 .. +04:41.068 | 102 | PK___Enum775__AC8ED0C4FF350F48 | 1 | 0 |  | ALTER INDEX x1 |  |
| 86 | +04:41.075 .. +04:41.140 | 102 | PK___Documen__AC8ED0C466BAE6BE | 1 | 0 |  | ALTER INDEX x1 |  |
| 87 | +04:41.145 .. +04:41.180 | 102 | PK___Enum557__AC8ED0C4FB9D4931 | 1 | 0 |  | ALTER INDEX x1 |  |
| 88 | +04:41.201 .. +04:41.243 | 102 | PK___Referen__AC8ED0C486060C1C | 1 | 0 |  | ALTER INDEX x1 |  |
| 89 | +04:41.248 .. +04:41.336 | 102 | PK___Documen__AC8ED0C4BEEC0646 | 1 | 0 |  | ALTER INDEX x1 |  |
| 90 | +04:41.341 .. +04:41.413 | 102 | PK___Enum775__AC8ED0C4A25B817F | 1 | 0 |  | ALTER INDEX x1 |  |
| 91 | +04:41.418 .. +04:41.446 | 102 | PK___Referen__AC8ED0C4707A8E14 | 1 | 0 |  | ALTER INDEX x1 |  |
| 92 | +04:41.451 .. +04:41.487 | 102 | PK___Enum371__AC8ED0C4E0AD3C82 | 1 | 0 |  | ALTER INDEX x1 |  |
| 93 | +04:41.506 .. +04:41.511 | 102 | PK___Referen__AC8ED0C4BFEB909A | 1 | 0 |  | ALTER INDEX x1 |  |
| 94 | +04:41.518 .. +04:41.547 | 102 | PK___Referen__AC8ED0C420549A80 | 1 | 0 |  | ALTER INDEX x1 |  |
| 95 | +04:41.552 .. +04:41.599 | 102 | PK___Documen__AC8ED0C43F235CA4 | 1 | 0 |  | ALTER INDEX x1 |  |
| 96 | +04:41.605 .. +04:41.631 | 102 | PK___Enum695__AC8ED0C4095C2F52 | 1 | 0 |  | ALTER INDEX x1 |  |
| 97 | +04:41.633 .. +04:41.686 | 102 | PK___Enum101__AC8ED0C40096B7E7 | 1 | 0 |  | ALTER INDEX x1 |  |
| 98 | +04:41.690 .. +04:41.706 | 102 | PK___Referen__AC8ED0C4C6104A2F | 1 | 0 |  | ALTER INDEX x1 |  |
| 99 | +04:41.712 .. +04:41.725 | 102 | PK___Enum101__AC8ED0C4FAF2C1BA | 1 | 0 |  | ALTER INDEX x1 |  |
| 100 | +04:41.731 .. +04:41.757 | 102 | PK___Enum744__AC8ED0C4E3D2105D | 1 | 0 |  | ALTER INDEX x1 |  |
| 101 | +04:41.761 .. +04:41.779 | 102 | PK___Referen__AC8ED0C415382F6E | 1 | 0 |  | ALTER INDEX x1 |  |
| 102 | +04:41.784 .. +04:41.873 | 102 | PK___Documen__AC8ED0C445F42025 | 1 | 0 |  | ALTER INDEX x1 |  |
| 103 | +04:41.879 .. +04:41.896 | 102 | PK___Enum101__AC8ED0C477EDCE7B | 1 | 0 |  | ALTER INDEX x1 |  |
| 104 | +04:41.898 .. +04:41.920 | 102 | PK___Enum100__AC8ED0C444C759B6 | 1 | 0 |  | ALTER INDEX x1 |  |
| 105 | +04:41.926 .. +04:41.953 | 102 | PK___Enum393__AC8ED0C4DA2D1B43 | 1 | 0 |  | ALTER INDEX x1 |  |
| 106 | +04:41.957 .. +04:41.974 | 102 | PK___Referen__AC8ED0C4C8187142 | 1 | 0 |  | ALTER INDEX x1 |  |
| 107 | +04:41.980 .. +04:42.113 | 102 | PK___Documen__AC8ED0C4B2C8F0DD | 1 | 0 |  | ALTER INDEX x1 |  |
| 108 | +04:42.118 .. +04:42.143 | 102 | PK___Enum660__AC8ED0C4A78090A2 | 1 | 0 |  | ALTER INDEX x1 |  |
| 109 | +04:42.148 .. +04:42.172 | 102 | PK___Enum512__AC8ED0C4BD22C8BB | 1 | 0 |  | ALTER INDEX x1 |  |
| 110 | +04:42.180 .. +04:42.196 | 102 | PK___Referen__AC8ED0C41A45480E | 1 | 0 |  | ALTER INDEX x1 |  |
| 111 | +04:42.202 .. +04:42.410 | 102 | PK___Referen__AC8ED0C47F4F73B1 | 1 | 0 |  | ALTER INDEX x1 |  |
| 112 | +04:42.414 .. +04:42.453 | 102 | PK___Enum952__AC8ED0C4933C2045 | 1 | 0 |  | ALTER INDEX x1 |  |
| 113 | +04:42.462 .. +04:42.477 | 102 | PK___Documen__AC8ED0C4E88010F8 | 1 | 0 |  | ALTER INDEX x1 |  |
| 114 | +04:42.479 .. +04:42.506 | 102 | PK___Enum341__AC8ED0C41B52AE26 | 1 | 0 |  | ALTER INDEX x1 |  |
| 115 | +04:42.512 .. +04:42.546 | 102 | PK___Enum540__AC8ED0C4C2E432B9 | 1 | 0 |  | ALTER INDEX x1 |  |
| 116 | +04:42.551 .. +04:42.585 | 102 | PK___Referen__AC8ED0C4FC42C178 | 1 | 0 |  | ALTER INDEX x1 |  |
| 117 | +04:42.595 .. +04:42.637 | 102 | PK___Enum952__AC8ED0C49930488C | 1 | 0 |  | ALTER INDEX x1 |  |
| 118 | +04:42.641 .. +04:42.657 | 102 | PK___Enum289__AC8ED0C4711BA67F | 1 | 0 |  | ALTER INDEX x1 |  |
| 119 | +04:42.665 .. +04:42.692 | 102 | PK___Enum381__AC8ED0C43792291B | 1 | 0 |  | ALTER INDEX x1 |  |
| 120 | +04:42.703 .. +04:42.729 | 102 | PK___Referen__AC8ED0C4EF090EEF | 1 | 0 |  | ALTER INDEX x1 |  |
| 121 | +04:42.733 .. +04:42.746 | 102 | PK___Referen__AC8ED0C4E54CE2E1 | 1 | 0 |  | ALTER INDEX x1 |  |
| 122 | +04:42.751 .. +04:42.787 | 102 | PK___Enum952__AC8ED0C408322C5B | 1 | 0 |  | ALTER INDEX x1 |  |
| 123 | +04:42.789 .. +04:42.797 | 102 | PK___Enum643__AC8ED0C4E21F4DCD | 1 | 0 |  | ALTER INDEX x1 |  |
| 124 | +04:42.797 .. +04:42.810 | 102 | PK___Enum143__AC8ED0C4B1AB9667 | 1 | 0 |  | ALTER INDEX x1 |  |
| 125 | +04:42.816 .. +04:42.853 | 102 | PK___Enum952__AC8ED0C4F51F6F1F | 1 | 0 |  | ALTER INDEX x1 |  |
| 126 | +04:42.857 .. +04:42.915 | 102 | PK___Referen__AC8ED0C4C2C277D3 | 1 | 0 |  | ALTER INDEX x1 |  |
| 127 | +04:42.921 .. +04:42.955 | 102 | PK___Referen__AC8ED0C480089D49 | 1 | 0 |  | ALTER INDEX x1 |  |
| 128 | +04:42.960 .. +04:42.967 | 102 | PK___Enum100__AC8ED0C4DF38B294 | 1 | 0 |  | ALTER INDEX x1 |  |
| 129 | +04:42.972 .. +04:42.995 | 102 | PK___Enum289__AC8ED0C44A08678E | 1 | 0 |  | ALTER INDEX x1 |  |
| 130 | +04:42.999 .. +04:43.022 | 102 | PK___Referen__AC8ED0C49DBC926B | 1 | 0 |  | ALTER INDEX x1 |  |
| 131 | +04:43.031 .. +04:43.123 | 102 | PK___Documen__AC8ED0C436C2A1BB | 1 | 0 |  | ALTER INDEX x1 |  |
| 132 | +04:43.128 .. +04:43.167 | 102 | PK___Enum101__AC8ED0C4A90A412D | 1 | 0 |  | ALTER INDEX x1 |  |
| 133 | +04:43.172 .. +04:43.233 | 102 | PK___Enum788__AC8ED0C4DDA39298 | 1 | 0 |  | ALTER INDEX x1 |  |
| 134 | +04:43.241 .. +04:43.273 | 102 | PK___Enum659__AC8ED0C40AD9652D | 1 | 0 |  | ALTER INDEX x1 |  |
| 135 | +04:43.278 .. +04:43.433 | 102 | PK___Documen__AC8ED0C4908410FC | 1 | 0 |  | ALTER INDEX x1 |  |
| 136 | +04:43.440 .. +04:43.468 | 102 | PK___Enum101__AC8ED0C4E596713A | 1 | 0 |  | ALTER INDEX x1 |  |
| 137 | +04:43.473 .. +04:43.492 | 102 | PK___Enum223__AC8ED0C43FC7F036 | 1 | 0 |  | ALTER INDEX x1 |  |
| 138 | +04:43.503 .. +04:43.661 | 102 | PK___Enum157__AC8ED0C4ABE06CDB | 1 | 0 |  | ALTER INDEX x1 |  |
| 139 | +04:43.666 .. +04:43.881 | 102 | PK___Enum101__AC8ED0C4157059D4 | 1 | 0 |  | ALTER INDEX x1 |  |
| 140 | +04:43.892 .. +04:43.914 | 102 | PK___Enum584__AC8ED0C47A289450 | 1 | 0 |  | ALTER INDEX x1 |  |
| 141 | +04:43.918 .. +04:43.933 | 102 | PK___Enum43__AC8ED0C48A7C72CE | 1 | 0 |  | ALTER INDEX x1 |  |
| 142 | +04:43.940 .. +04:44.154 | 102 | PK___Enum101__AC8ED0C4215FAD30 | 1 | 0 |  | ALTER INDEX x1 |  |
| 143 | +04:44.166 .. +04:44.174 | 102 | PK___Enum109__AC8ED0C48C9F385C | 1 | 0 |  | ALTER INDEX x1 |  |
| 144 | +04:44.181 .. +04:44.200 | 102 | PK___Enum223__AC8ED0C4A06BEF88 | 1 | 0 |  | ALTER INDEX x1 |  |
| 145 | +04:44.209 .. +04:44.245 | 102 | PK___Enum44__AC8ED0C47F1B6168 | 1 | 0 |  | ALTER INDEX x1 |  |
| 146 | +04:44.254 .. +04:44.319 | 102 | PK___Referen__AC8ED0C4892BB189 | 1 | 0 |  | ALTER INDEX x1 |  |
| 147 | +04:44.325 .. +04:44.391 | 102 | PK___Enum109__AC8ED0C4F0A64B9A | 1 | 0 |  | ALTER INDEX x1 |  |
| 148 | +04:44.395 .. +04:44.400 | 102 | PK___Enum431__AC8ED0C4E6070034 | 1 | 0 |  | ALTER INDEX x1 |  |
| 149 | +04:44.404 .. +04:44.410 | 102 | PK___Enum636__AC8ED0C4151A36E1 | 1 | 0 |  | ALTER INDEX x1 |  |
| 150 | +04:44.415 .. +04:44.429 | 102 | PK___CKinds7__AC8ED0C42511C355 | 1 | 0 |  | ALTER INDEX x1 |  |
| 151 | +04:44.430 .. +04:44.488 | 102 | PK___Referen__AC8ED0C46BC7FBC2 | 1 | 0 |  | ALTER INDEX x1 |  |
| 152 | +04:44.501 .. +04:44.522 | 102 | PK___Enum431__AC8ED0C4E22A930D | 1 | 0 |  | ALTER INDEX x1 |  |
| 153 | +04:44.527 .. +04:44.535 | 102 | PK___Enum501__AC8ED0C414F59229 | 1 | 0 |  | ALTER INDEX x1 |  |
| 154 | +04:44.540 .. +04:44.577 | 102 | PK___Documen__AC8ED0C45A3711AA | 1 | 0 |  | ALTER INDEX x1 |  |
| 155 | +04:44.582 .. +04:44.603 | 102 | PK___Enum781__AC8ED0C44EAEE3FF | 1 | 0 |  | ALTER INDEX x1 |  |
| 156 | +04:44.604 .. +04:44.619 | 102 | PK___Enum50__AC8ED0C4AA9CA5A7 | 1 | 0 |  | ALTER INDEX x1 |  |
| 157 | +04:44.623 .. +04:44.648 | 102 | PK___Enum842__AC8ED0C43AE501DB | 1 | 0 |  | ALTER INDEX x1 |  |
| 158 | +04:44.655 .. +04:44.677 | 102 | PK___Enum158__AC8ED0C41F4FF7B9 | 1 | 0 |  | ALTER INDEX x1 |  |
| 159 | +04:44.682 .. +04:44.696 | 102 | PK___Enum842__AC8ED0C4FD0DE18E | 1 | 0 |  | ALTER INDEX x1 |  |
| 160 | +04:44.700 .. +04:44.784 | 102 | PK___Referen__AC8ED0C43B08EA6E | 1 | 0 |  | ALTER INDEX x1 |  |
| 161 | +04:44.790 .. +04:44.902 | 102 | PK___Enum260__AC8ED0C4A2531B38 | 1 | 0 |  | ALTER INDEX x1 |  |
| 162 | +04:44.905 .. +04:44.919 | 102 | PK___Enum998__AC8ED0C4F71169A3 | 1 | 0 |  | ALTER INDEX x1 |  |
| 163 | +04:44.923 .. +04:44.969 | 102 | PK___Acc7810__AC8ED0C4BF01D6AB | 1 | 0 |  | ALTER INDEX x1 |  |
| 164 | +04:44.977 .. +04:45.086 | 102 | PK___Enum796__AC8ED0C46A7EA09A | 1 | 0 |  | ALTER INDEX x1 |  |
| 165 | +04:45.091 .. +04:45.104 | 102 | PK___Enum157__AC8ED0C49406420C | 1 | 0 |  | ALTER INDEX x1 |  |
| 166 | +04:45.107 .. +04:45.134 | 102 | PK___Enum111__AC8ED0C42A10CBE5 | 1 | 0 |  | ALTER INDEX x1 |  |
| 167 | +04:45.137 .. +04:45.188 | 102 | PK___Enum100__AC8ED0C48E6C8A24 | 1 | 0 |  | ALTER INDEX x1 |  |
| 168 | +04:45.194 .. +04:45.273 | 102 | PK___Enum926__AC8ED0C427FE9BA6 | 1 | 0 |  | ALTER INDEX x1 |  |
| 169 | +04:45.281 .. +04:45.333 | 102 | PK___Enum260__AC8ED0C421C5E69D | 1 | 0 |  | ALTER INDEX x1 |  |
| 170 | +04:45.342 .. +04:45.377 | 102 | PK___Referen__AC8ED0C4E813126A | 1 | 0 |  | ALTER INDEX x1 |  |
| 171 | +04:45.381 .. +04:45.490 | 102 | PK___Enum289__AC8ED0C404AC9CBA | 1 | 0 |  | ALTER INDEX x1 |  |
| 172 | +04:45.495 .. +04:45.564 | 102 | PK___Enum581__AC8ED0C4A799006B | 1 | 0 |  | ALTER INDEX x1 |  |
| 173 | +04:45.611 .. +04:45.660 | 102 | PK___Enum999__AC8ED0C400399FC4 | 1 | 0 |  | ALTER INDEX x1 |  |
| 174 | +04:45.666 .. +04:45.728 | 102 | PK___Enum338__AC8ED0C4871E1447 | 1 | 0 |  | ALTER INDEX x1 |  |
| 175 | +04:45.732 .. +04:45.743 | 102 | PK___Enum218__AC8ED0C4BBB6F3A4 | 1 | 0 |  | ALTER INDEX x1 |  |
| 176 | +04:45.748 .. +04:45.770 | 102 | PK___Referen__AC8ED0C44AD616EF | 1 | 0 |  | ALTER INDEX x1 |  |
| 177 | +04:45.774 .. +04:45.795 | 102 | PK___Enum585__AC8ED0C4366621BE | 1 | 0 |  | ALTER INDEX x1 |  |
| 178 | +04:45.799 .. +04:45.811 | 102 | PK___Enum643__AC8ED0C4391D26D2 | 1 | 0 |  | ALTER INDEX x1 |  |
| 179 | +04:45.815 .. +04:45.832 | 102 | PK___Documen__AC8ED0C4268F0EF4 | 1 | 0 |  | ALTER INDEX x1 |  |
| 180 | +04:45.837 .. +04:45.863 | 102 | PK___Referen__AC8ED0C4C7A5F18A | 1 | 0 |  | ALTER INDEX x1 |  |
| 181 | +04:45.867 .. +04:45.888 | 102 | PK___Enum291__AC8ED0C4832575E5 | 1 | 0 |  | ALTER INDEX x1 |  |
| 182 | +04:45.890 .. +04:45.899 | 102 | PK___Enum100__AC8ED0C4512B7158 | 1 | 0 |  | ALTER INDEX x1 |  |
| 183 | +04:45.900 .. +04:45.913 | 102 | PK___Documen__AC8ED0C4C752E7D2 | 1 | 0 |  | ALTER INDEX x1 |  |
| 184 | +04:45.917 .. +04:45.939 | 102 | PK___Documen__AC8ED0C4267F5601 | 1 | 0 |  | ALTER INDEX x1 |  |
| 185 | +04:45.943 .. +04:45.964 | 102 | PK___Enum588__AC8ED0C48EA995D4 | 1 | 0 |  | ALTER INDEX x1 |  |
| 186 | +04:45.967 .. +04:45.971 | 102 | PK__Config__3F5D91869047AD60 | 1 | 0 |  | ALTER INDEX x1 |  |
| 187 | +04:45.975 .. +04:45.980 | 102 | PK___ConfigC__AC8ED0C44B813941 | 1 | 0 |  | ALTER INDEX x1 |  |
| 188 | +04:45.984 .. +04:45.999 | 102 | PK___Enum51__AC8ED0C41C98B58F | 1 | 0 |  | ALTER INDEX x1 |  |
| 189 | +04:46.003 .. +04:46.009 | 102 | PK___Enum100__AC8ED0C442A7AED4 | 1 | 0 |  | ALTER INDEX x1 |  |
| 190 | +04:46.013 .. +04:46.023 | 102 | PK___Documen__AC8ED0C4A7AD6483 | 1 | 0 |  | ALTER INDEX x1 |  |
| 191 | +04:46.026 .. +04:46.030 | 102 | PK___Enum586__AC8ED0C4E9456087 | 1 | 0 |  | ALTER INDEX x1 |  |
| 192 | +04:46.030 .. +04:46.031 | 102 | PK__ConfigSa__3F5D918601F224F0 | 1 | 0 |  | ALTER INDEX x1 |  |
| 193 | +04:46.033 .. +04:46.039 | 102 | PK___Enum658__AC8ED0C4BC704C55 | 1 | 0 |  | ALTER INDEX x1 |  |
| 194 | +04:46.043 .. +04:46.044 | 102 | PK___Documen__AC8ED0C4F1423276 | 1 | 0 |  | ALTER INDEX x1 |  |
| 195 | +04:46.049 .. +04:46.066 | 102 | PK___Referen__AC8ED0C49E6F2422 | 1 | 0 |  | ALTER INDEX x1 |  |
| 196 | +04:46.074 .. +04:46.108 | 102 | PK___Referen__AC8ED0C485EE5BF4 | 1 | 0 |  | ALTER INDEX x1 |  |
| 197 | +04:46.112 .. +04:46.139 | 102 | PK___Enum144__AC8ED0C49BEDAFCB | 1 | 0 |  | ALTER INDEX x1 |  |
| 198 | +04:46.145 .. +04:46.158 | 102 | PK___Enum100__AC8ED0C4A41DF8A9 | 1 | 0 |  | ALTER INDEX x1 |  |
| 199 | +04:46.162 .. +04:46.167 | 102 | PK__Params__3F5D91866FE25E21 | 1 | 0 |  | ALTER INDEX x1 |  |
| 200 | +04:46.171 .. +04:46.223 | 102 | PK___Documen__AC8ED0C419CBF8F3 | 1 | 0 |  | ALTER INDEX x1 |  |
| 201 | +04:46.228 .. +04:46.394 | 102 | PK___Enum901__AC8ED0C4C94E5A54 | 1 | 0 |  | ALTER INDEX x1 |  |
| 202 | +04:46.398 .. +04:46.445 | 102 | PK___Enum909__AC8ED0C46A4C00B5 | 1 | 0 |  | ALTER INDEX x1 |  |
| 203 | +04:46.454 .. +04:46.489 | 102 | PK___Enum775__AC8ED0C4AC358A46 | 1 | 0 |  | ALTER INDEX x1 |  |
| 204 | +04:46.494 .. +04:46.519 | 102 | PK___BPrPoin__AC8ED0C41E4B4687 | 1 | 0 |  | ALTER INDEX x1 |  |
| 205 | +04:46.532 .. +04:46.540 | 102 | PK__Files__3F5D91861B5AC81B | 1 | 0 |  | ALTER INDEX x1 |  |
| 206 | +04:46.558 .. +04:46.610 | 102 | PK___Enum45__AC8ED0C4DBB8CE23 | 1 | 0 |  | ALTER INDEX x1 |  |
| 207 | +04:46.616 .. +04:46.681 | 102 | PK___Referen__AC8ED0C465132DCF | 1 | 0 |  | ALTER INDEX x1 |  |
| 208 | +04:46.697 .. +04:46.798 | 102 | PK___Referen__AC8ED0C43608E039 | 1 | 0 |  | ALTER INDEX x1 |  |
| 209 | +04:46.800 .. +04:46.812 | 102 | PK___Enum849__AC8ED0C4298FB2F5 | 1 | 0 |  | ALTER INDEX x1 |  |
| 210 | +04:46.816 .. +04:46.821 | 102 | PK___BPrPoin__AC8ED0C4BAAED516 | 1 | 0 |  | ALTER INDEX x1 |  |
| 211 | +04:46.823 .. +04:46.828 | 102 | PK___Enum393__AC8ED0C401623E0E | 1 | 0 |  | ALTER INDEX x1 |  |
| 212 | +04:46.832 .. +04:46.837 | 102 | PK__DepotFil__3F5D9186A5805172 | 1 | 0 |  | ALTER INDEX x1 |  |
| 213 | +04:46.838 .. +04:46.848 | 102 | PK___Enum989__AC8ED0C4097A3882 | 1 | 0 |  | ALTER INDEX x1 |  |
| 214 | +04:46.853 .. +04:46.872 | 102 | PK___Enum334__AC8ED0C4ACA8F432 | 1 | 0 |  | ALTER INDEX x1 |  |
| 215 | +04:46.874 .. +04:46.885 | 102 | PK___Enum100__AC8ED0C41B3B4ECE | 1 | 0 |  | ALTER INDEX x1 |  |
| 216 | +04:46.891 .. +04:46.900 | 102 | PK___Enum302__AC8ED0C4421F86F8 | 1 | 0 |  | ALTER INDEX x1 |  |
| 217 | +04:46.904 .. +04:46.910 | 102 | PK___Referen__AC8ED0C40841DA66 | 1 | 0 |  | ALTER INDEX x1 |  |
| 218 | +04:46.914 .. +04:46.933 | 102 | PK___Enum102__AC8ED0C495C87DC5 | 1 | 0 |  | ALTER INDEX x1 |  |
| 219 | +04:46.938 .. +04:46.949 | 102 | PK___Enum587__AC8ED0C441780545 | 1 | 0 |  | ALTER INDEX x1 |  |
| 220 | +04:46.954 .. +04:46.962 | 102 | PK___Enum100__AC8ED0C46316DEF2 | 1 | 0 |  | ALTER INDEX x1 |  |
| 221 | +04:46.963 .. +04:46.968 | 102 | PK__ConfigCA__3F5D9186C400D87E | 1 | 0 |  | ALTER INDEX x1 |  |
| 222 | +04:46.969 .. +04:46.974 | 102 | PK___Enum100__AC8ED0C4681DA8F1 | 1 | 0 |  | ALTER INDEX x1 |  |
| 223 | +04:46.978 .. +04:46.989 | 102 | PK___Enum102__AC8ED0C45700DF0F | 1 | 0 |  | ALTER INDEX x1 |  |
| 224 | +04:46.993 .. +04:47.005 | 102 | PK___Enum52__AC8ED0C4409F08D1 | 1 | 0 |  | ALTER INDEX x1 |  |
| 225 | +04:47.009 .. +04:47.014 | 102 | PK___Enum420__AC8ED0C4578FD704 | 1 | 0 |  | ALTER INDEX x1 |  |
| 226 | +04:47.019 .. +04:47.030 | 102 | PK___Referen__AC8ED0C41A143985 | 1 | 0 |  | ALTER INDEX x1 |  |
| 227 | +04:47.034 .. +04:47.037 | 102 | PK__ConfigCA__3F5D918632D9FC3E | 1 | 0 |  | ALTER INDEX x1 |  |
| 228 | +04:47.037 .. +04:47.054 | 102 | PK___Enum102__AC8ED0C42BA53A18 | 1 | 0 |  | ALTER INDEX x1 |  |
| 229 | +04:47.060 .. +04:47.079 | 102 | PK___Enum745__AC8ED0C4C064A176 | 1 | 0 |  | ALTER INDEX x1 |  |
| 230 | +04:47.083 .. +04:47.112 | 102 | PK___Enum260__AC8ED0C4F170550B | 1 | 0 |  | ALTER INDEX x1 |  |
| 231 | +04:47.116 .. +04:47.139 | 102 | PK___Enum102__AC8ED0C4BDA2ADBA | 1 | 0 |  | ALTER INDEX x1 |  |
| 232 | +04:47.140 .. +04:47.170 | 102 | PK___Enum289__AC8ED0C407750F87 | 1 | 0 |  | ALTER INDEX x1 |  |
| 233 | +04:47.175 .. +04:47.183 | 102 | PK__v8users__3214EC2787E20B9B | 1 | 0 |  | ALTER INDEX x1 |  |
| 234 | +04:47.186 .. +04:47.192 | 102 | ByEAuth | 1 | 0 |  | ALTER INDEX x1 |  |
| 235 | +04:47.193 .. +04:47.198 | 102 | ByEmail_V8USERS | 1 | 0 |  | ALTER INDEX x1 |  |
| 236 | +04:47.200 .. +04:47.210 | 102 | ByName | 1 | 0 |  | ALTER INDEX x1 |  |
| 237 | +04:47.215 .. +04:47.225 | 102 | ByOSName | 1 | 0 |  | ALTER INDEX x1 |  |
| 238 | +04:47.227 .. +04:47.234 | 102 | ByRolesID | 1 | 0 |  | ALTER INDEX x1 |  |
| 239 | +04:47.237 .. +04:47.242 | 102 | ByShow | 1 | 0 |  | ALTER INDEX x1 |  |
| 240 | +04:47.248 .. +04:47.254 | 102 | ByUSDescr | 1 | 0 |  | ALTER INDEX x1 |  |
| 241 | +04:47.259 .. +04:47.263 | 102 | ByUSEmail_V8USERS | 1 | 0 |  | ALTER INDEX x1 |  |
| 242 | +04:47.265 .. +04:47.276 | 102 | ByUSName | 1 | 0 |  | ALTER INDEX x1 |  |
| 243 | +04:47.279 .. +04:47.283 | 102 | ByUSOSName | 1 | 0 |  | ALTER INDEX x1 |  |
| 244 | +04:47.286 .. +04:47.291 | 102 | PK___Enum774__AC8ED0C4B572C073 | 1 | 0 |  | ALTER INDEX x1 |  |
| 245 | +04:47.295 .. +04:47.312 | 102 | PK___Enum102__AC8ED0C4F7AA3B58 | 1 | 0 |  | ALTER INDEX x1 |  |
| 246 | +04:47.317 .. +04:47.332 | 102 | PK___Enum261__AC8ED0C4D3D4AB24 | 1 | 0 |  | ALTER INDEX x1 |  |
| 247 | +04:47.332 .. +04:47.341 | 102 | PK___Enum501__AC8ED0C4CD7D2061 | 1 | 0 |  | ALTER INDEX x1 |  |
| 248 | +04:47.345 .. +04:47.361 | 102 | PK___Referen__AC8ED0C41BC08E01 | 1 | 0 |  | ALTER INDEX x1 |  |
| 249 | +04:47.366 .. +04:47.384 | 102 | ByName_V8USERPWDPLCS | 1 | 0 |  | ALTER INDEX x1 |  |
| 250 | +04:47.388 .. +04:47.404 | 102 | PK___Enum102__AC8ED0C42781C133 | 1 | 0 |  | ALTER INDEX x1 |  |
| 251 | +04:47.409 .. +04:47.424 | 102 | PK___Enum261__AC8ED0C4B3E42B54 | 1 | 0 |  | ALTER INDEX x1 |  |
| 252 | +04:47.429 .. +04:47.451 | 102 | PK___Enum918__AC8ED0C413C4AEF5 | 1 | 0 |  | ALTER INDEX x1 |  |
| 253 | +04:47.456 .. +04:47.478 | 102 | PK__V8CMSDPW__4575608DC7866895 | 1 | 0 |  | ALTER INDEX x1 |  |
| 254 | +04:47.485 .. +04:47.502 | 102 | PK___Enum102__AC8ED0C4BB442535 | 1 | 0 |  | ALTER INDEX x1 |  |
| 255 | +04:47.505 .. +04:47.524 | 102 | PK___Enum158__AC8ED0C46C147B57 | 1 | 0 |  | ALTER INDEX x1 |  |
| 256 | +04:47.529 .. +04:47.535 | 102 | PK___Enum223__AC8ED0C4EA2126BF | 1 | 0 |  | ALTER INDEX x1 |  |
| 257 | +04:47.540 .. +04:47.564 | 102 | PK___Enum102__AC8ED0C41BE4B744 | 1 | 0 |  | ALTER INDEX x1 |  |
| 258 | +04:47.570 .. +04:47.576 | 102 | PK___Referen__AC8ED0C4FB25C678 | 1 | 0 |  | ALTER INDEX x1 |  |
| 259 | +04:47.580 .. +04:47.605 | 102 | PK___Enum224__AC8ED0C479948BAF | 1 | 0 |  | ALTER INDEX x1 |  |
| 260 | +04:47.614 .. +04:47.630 | 102 | PK___Enum774__AC8ED0C4942629CD | 1 | 0 |  | ALTER INDEX x1 |  |
| 261 | +04:47.634 .. +04:47.660 | 102 | PK___Enum101__AC8ED0C43D55E19A | 1 | 0 |  | ALTER INDEX x1 |  |
| 262 | +04:47.664 .. +04:47.682 | 102 | PK___Enum100__AC8ED0C4CD6C11BC | 1 | 0 |  | ALTER INDEX x1 |  |
| 263 | +04:47.687 .. +04:47.704 | 102 | PK___Enum439__AC8ED0C4BB089C69 | 1 | 0 |  | ALTER INDEX x1 |  |
| 264 | +04:47.709 .. +04:47.740 | 102 | PK___ExtsChn__AC8ED0C4357C819B | 1 | 0 |  | ALTER INDEX x1 |  |
| 265 | +04:47.744 .. +04:47.757 | 102 | PK___Enum968__AC8ED0C40747BD4F | 1 | 0 |  | ALTER INDEX x1 |  |
| 266 | +04:47.762 .. +04:47.767 | 102 | PK___Enum102__AC8ED0C4E2E5A027 | 1 | 0 |  | ALTER INDEX x1 |  |
| 267 | +04:47.771 .. +04:47.789 | 102 | PK___Referen__AC8ED0C4445E7E8F | 1 | 0 |  | ALTER INDEX x1 |  |
| 268 | +04:47.793 .. +04:47.798 | 102 | PK__SchemaSt__95006FDAA60839A1 | 1 | 0 |  | ALTER INDEX x1 |  |
| 269 | +04:47.802 .. +04:47.811 | 102 | PK___Enum605__AC8ED0C460D05D15 | 1 | 0 |  | ALTER INDEX x1 |  |
| 270 | +04:47.815 .. +04:47.837 | 102 | PK___Chrc781__AC8ED0C413E87D9F | 1 | 0 |  | ALTER INDEX x1 |  |
| 271 | +04:47.841 .. +04:47.859 | 102 | PK___Enum102__AC8ED0C49AABBDFF | 1 | 0 |  | ALTER INDEX x1 |  |
| 272 | +04:47.863 .. +04:47.885 | 102 | PK___Enum319__AC8ED0C4320D8974 | 1 | 0 |  | ALTER INDEX x1 |  |
| 273 | +04:47.886 .. +04:47.902 | 102 | PK___Enum102__AC8ED0C457002C80 | 1 | 0 |  | ALTER INDEX x1 |  |
| 274 | +04:47.907 .. +04:47.934 | 102 | PK___Enum100__AC8ED0C40B3E1BE0 | 1 | 0 |  | ALTER INDEX x1 |  |
| 275 | +04:47.938 .. +04:47.943 | 102 | PK___Referen__AC8ED0C4719110B2 | 1 | 0 |  | ALTER INDEX x1 |  |
| 276 | +04:47.948 .. +04:47.975 | 102 | PK___Enum102__AC8ED0C4F6566551 | 1 | 0 |  | ALTER INDEX x1 |  |
| 277 | +04:47.979 .. +04:47.988 | 102 | PK___Enum967__AC8ED0C4E008E97C | 1 | 0 |  | ALTER INDEX x1 |  |
| 278 | +04:47.993 .. +04:48.016 | 102 | PK___Enum100__AC8ED0C4B6789FB7 | 1 | 0 |  | ALTER INDEX x1 |  |
| 279 | +04:48.020 .. +04:48.034 | 102 | PK___Chrc158__AC8ED0C4EFC1C642 | 1 | 0 |  | ALTER INDEX x1 |  |
| 280 | +04:48.038 .. +04:48.052 | 102 | PK___Enum102__AC8ED0C4D3B0B0FF | 1 | 0 |  | ALTER INDEX x1 |  |
| 281 | +04:48.056 .. +04:48.061 | 102 | PK___Enum967__AC8ED0C43F065962 | 1 | 0 |  | ALTER INDEX x1 |  |
| 282 | +04:48.063 .. +04:48.065 | 102 | PK___Enum643__AC8ED0C4EF5F3160 | 1 | 0 |  | ALTER INDEX x1 |  |
| 283 | +04:48.069 .. +04:48.076 | 102 | PK___Referen__AC8ED0C4FC915B0D | 1 | 0 |  | ALTER INDEX x1 |  |
| 284 | +04:48.080 .. +04:48.108 | 102 | PK___Enum102__AC8ED0C44FCAE24F | 1 | 0 |  | ALTER INDEX x1 |  |
| 285 | +04:48.112 .. +04:48.117 | 102 | PK___Enum968__AC8ED0C4F3F5F7F6 | 1 | 0 |  | ALTER INDEX x1 |  |
| 286 | +04:48.121 .. +04:48.137 | 102 | PK___Enum157__AC8ED0C4B82FE4C9 | 1 | 0 |  | ALTER INDEX x1 |  |
| 287 | +04:48.143 .. +04:48.175 | 102 | PK___Enum968__AC8ED0C4F802A606 | 1 | 0 |  | ALTER INDEX x1 |  |
| 288 | +04:48.178 .. +04:48.201 | 102 | PK___Enum796__AC8ED0C41AB8560A | 1 | 0 |  | ALTER INDEX x1 |  |
| 289 | +04:48.206 .. +04:48.213 | 102 | PK___Enum844__AC8ED0C42092ED08 | 1 | 0 |  | ALTER INDEX x1 |  |
| 290 | +04:48.219 .. +04:48.230 | 102 | PK___Enum968__AC8ED0C4361A4211 | 1 | 0 |  | ALTER INDEX x1 |  |
| 291 | +04:48.234 .. +04:48.256 | 102 | PK___Enum796__AC8ED0C4E7DB46EC | 1 | 0 |  | ALTER INDEX x1 |  |
| 292 | +04:48.257 .. +04:48.262 | 102 | PK___Enum260__AC8ED0C42B168BAC | 1 | 0 |  | ALTER INDEX x1 |  |
| 293 | +04:48.263 .. +04:48.265 | 102 | PK___Enum922__AC8ED0C4CF091A9E | 1 | 0 |  | ALTER INDEX x1 |  |
| 294 | +04:48.266 .. +04:48.269 | 102 | PK___Referen__AC8ED0C419529121 | 1 | 0 |  | ALTER INDEX x1 |  |
| 295 | +04:48.270 .. +04:48.272 | 102 | PK___Enum850__AC8ED0C4C6223001 | 1 | 0 |  | ALTER INDEX x1 |  |
| 296 | +04:48.300 .. +04:48.334 | 102 | SchemaStorage | 1 | 1 |  | UPDATE x1 |  |
| 297 | +04:48.358 .. +04:48.377 | 102 | DBSchema | 1 | 1 |  | UPDATE x1 |  |
| 298 | +04:48.428 .. +04:48.779 | 102 | Params | 39 | 33 | begin 1, commit 1 | DELETE DBNames.New x1; DELETE DBNames-Ext-1.New x1; DELETE DBNames-Ext-<guid>.New x2; DELETE <guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>.si x16; DELETE siVersions x1; UPDATE siVersions x1; DELETE DynamicallyUpdated x1 | `DBNames.New` .. `DynamicallyUpdated` |
| 299 | +04:48.818 .. +04:48.913 | 102 | Config | 83 | 82 |  | DELETE <guid>.<n> <- Config x23; UPDATE <guid>.<n>.new -> <guid>.<n> x23; DELETE <guid> <- Config x15; UPDATE <guid>.new -> <guid> x15; DELETE root <- Config x1; UPDATE root.new -> root x1; DELETE version <- Config x1; UPDATE version.new -> version x1; ... +3 more | `06427cd3-a661-4ec9-a4e4-204b9c6b9d6c.0` .. `DynamicallyUpdated` |
| 300 | +04:48.913 .. +04:48.940 | 102 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 301 | +04:48.949 .. +04:48.961 | 102 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 302 | +04:48.962 .. +04:48.965 | 102 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 303 | +04:48.993 .. +04:48.996 | 102 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 304 | +04:49.544 .. +04:49.801 | 102 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
