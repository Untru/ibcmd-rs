# Write phases

228 writes/DDL statements in 20 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +04:35.498 .. +04:35.519 | 279 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 2 | +04:35.550 .. +04:39.258 | 279 | Config | 47 | 23 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x10; INSERT <guid>.new <- ConfigSave x10; DELETE <guid>.<n>.new <- ConfigSave x10; INSERT <guid>.<n>.new <- ConfigSave x10; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 3 | +04:39.286 .. +04:39.311 | 279 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 4 | +04:39.809 .. +04:41.257 | 279 | Params | 4 | 2 | begin 1, commit 1 | INSERT DBNames.New x1; DELETE DBNames.New x2; UPDATE DBNames.New x1 | `DBNames.New` |
| 5 | +05:44.376 .. +05:44.498 | 195 | _DbCopiesInfoBaseUse | 1 | 2 | begin 1 | UPDATE x1 |  |
| 6 | +05:44.504 .. +05:44.522 | 195 | _DbCopiesInitialLast | 1 | 0 |  | DELETE x1 |  |
| 7 | +05:44.530 .. +05:44.556 | 195 | _DbCopiesUpdates | 1 | 0 | commit 1 | DELETE x1 |  |
| 8 | +05:44.593 .. +05:44.644 | 195 | _DbCopiesTrChObj | 2 | 0 |  | TRUNCATE x2 |  |
| 9 | +05:44.646 .. +05:44.649 | 195 | _DbCopiesTrChanges | 2 | 0 |  | TRUNCATE x2 |  |
| 10 | +05:44.688 .. +05:44.689 | 195 | _ExtensionsRestructNGS | 1 | 0 |  | DELETE x1 |  |
| 11 | +05:44.998 .. +05:46.084 | 195 | Params | 3 | 2 | begin 1, commit 1 | INSERT DBNames.New x1; DELETE DBNames.New x1; UPDATE DBNames.New x1 | `DBNames.New` |
| 12 | +08:12.517 .. +08:24.006 | 195 | Params | 48 | 32 | begin 16, commit 16 | INSERT <guid>.sinew x16; DELETE <guid>.sinew x16; UPDATE <guid>.sinew x16 | `42ed49cc-765d-4314-bc2d-af425af7bf13.sinew` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 13 | +08:24.622 .. +08:24.635 | 195 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 14 | +08:24.785 .. +08:27.758 | 195 | Params | 36 | 33 | begin 1, commit 1 | DELETE DBNames.New x1; DELETE <guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>.si x16; DELETE siVersions x1; UPDATE siVersions x1; DELETE DynamicallyUpdated x1 | `DBNames.New` .. `DynamicallyUpdated` |
| 15 | +08:28.058 .. +08:28.848 | 195 | Config | 47 | 46 |  | DELETE <guid>.<n> <- Config x10; UPDATE <guid>.<n>.new -> <guid>.<n> x10; DELETE <guid> <- Config x10; UPDATE <guid>.new -> <guid> x10; DELETE root <- Config x1; UPDATE root.new -> root x1; DELETE version <- Config x1; UPDATE version.new -> version x1; ... +3 more | `323997ca-488d-4d84-aa44-da60b6b4d414.0` .. `DynamicallyUpdated` |
| 16 | +08:28.850 .. +08:29.112 | 195 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 17 | +08:29.114 .. +08:29.125 | 195 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 18 | +08:29.128 .. +08:29.134 | 195 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 19 | +08:30.360 .. +08:30.797 | 195 | Params | 10 | 6 | begin 4, commit 4 | INSERT <guid>.ui x2; DELETE <guid>.ui x2; UPDATE <guid>.ui x2; DELETE ibparams.inf x1; UPDATE ibparams.inf x1; DELETE locale.inf x1; UPDATE locale.inf x1 | `41d98f83-fc5d-4f0b-9709-a32839d47a93.ui` .. `locale.inf` |
| 20 | +08:32.836 .. +08:32.909 | 195 | Files | 12 | 0 |  | DELETE userDocs_ru.bin <- Files x1; UPDATE userDocs_ru.new -> userDocs_ru.bin x1; DELETE userVocabulary_ru.bin <- Files x1; UPDATE userVocabulary_ru.new -> userVocabulary_ru.bin x1; DELETE userPostings_ru.bin <- Files x1; UPDATE userPostings_ru.new -> userPostings_ru.bin x1; DELETE userDocs_en.bin <- Files x1; UPDATE userDocs_en.new -> userDocs_en.bin x1; ... +4 more | `userDocs_ru.bin` .. `userPostings_en.new` |
