# Write phases

185 writes/DDL statements in 16 blocks. A block is a run of consecutive writes to one table. Reads, temporary-table writes and transaction control statements are left out (see timeline.md).

| # | time | sessions | table | statements | rows | user tx | what (operation, row-name shape x statements) | first .. last row name |
|---|---|---|---|---|---|---|---|---|
| 1 | +00:23.128 .. +00:23.148 | 185 | Params | 2 | 1 | begin 1, commit 1 | DELETE <guid>.ui x1; UPDATE <guid>.ui x1 | `54ba7662-8bac-49ee-b00f-0f5125fbfeab.ui` |
| 2 | +00:23.353 .. +00:23.357 | 185 | ConfigSave | 1 | 0 |  | DELETE LIKE %.new x1 | `LIKE %.new` |
| 3 | +00:23.362 .. +00:24.310 | 185 | Config | 19 | 9 |  | DELETE LIKE %.new x1; DELETE <guid>.new <- ConfigSave x2; INSERT <guid>.new <- ConfigSave x2; DELETE <guid>.<n>.new <- ConfigSave x4; INSERT <guid>.<n>.new <- ConfigSave x4; DELETE root.new <- ConfigSave x1; INSERT root.new <- ConfigSave x1; DELETE version.new <- ConfigSave x1; ... +3 more | `LIKE %.new` .. `versions.new` |
| 4 | +00:24.350 .. +00:24.484 | 185 | Files | 3 | 2 | begin 1, commit 1 | INSERT MobileVersions.datNEW x1; DELETE MobileVersions.datNEW x1; UPDATE MobileVersions.datNEW x1 | `MobileVersions.datNEW` |
| 5 | +00:34.193 .. +00:36.915 | 185 | Params | 48 | 32 | begin 16, commit 16 | INSERT <guid>.sinew x16; DELETE <guid>.sinew x16; UPDATE <guid>.sinew x16 | `42ed49cc-765d-4314-bc2d-af425af7bf13.sinew` .. `e05c0074-0404-4b7a-835e-9cacd405960e.sinew` |
| 6 | +00:37.538 .. +00:37.545 | 148 | _ExtensionsRestructNGS | 1 | 0 |  | DELETE x1 |  |
| 7 | +00:37.629 .. +00:37.656 | 148 | Config | 3 | 2 | begin 1, commit 1 | INSERT commit x1; DELETE commit x1; UPDATE commit x1 | `commit` |
| 8 | +00:37.675 .. +00:40.873 | 148 | Params | 35 | 33 | begin 1, commit 1 | DELETE <guid>.si <- Params x16; UPDATE <guid>.sinew -> <guid>.si x16; DELETE siVersions x1; UPDATE siVersions x1; DELETE DynamicallyUpdated x1 | `0b698dcd-501d-42d9-892d-5a9157bc996a.si` .. `DynamicallyUpdated` |
| 9 | +00:40.877 .. +00:40.886 | 148 | Config | 3 | 2 | begin 1, commit 1 | INSERT dbStruFinal x1; DELETE dbStruFinal x1; UPDATE dbStruFinal x1 | `dbStruFinal` |
| 10 | +00:41.189 .. +00:41.243 | 148 | _ConfigChngR | 1 | 7 |  | UPDATE x1 |  |
| 11 | +00:41.369 .. +00:41.818 | 148 | Config | 29 | 26 |  | DELETE <guid> <- Config x4; UPDATE <guid>_dynupdate_<guid> -> <guid> x2; DELETE <guid>.<n> <- Config x6; UPDATE <guid>_dynupdate_<guid>.<n> -> <guid>.<n> x2; DELETE versions_dynupdate_<guid> x1; DELETE deleted_dynupdate_<guid> x1; UPDATE <guid>.<n>.new -> <guid>.<n> x4; UPDATE <guid>.new -> <guid> x2; ... +7 more | `a627e390-8fad-4a95-afe6-674f54813188` .. `DynamicallyUpdated` |
| 12 | +00:41.819 .. +00:41.833 | 148 | ConfigSave | 1 | 0 |  | DELETE LIKE % x1 | `LIKE %` |
| 13 | +00:41.839 .. +00:41.855 | 148 | Files | 2 | 2 |  | DELETE MobileVersions.dat <- Files x1; UPDATE MobileVersions.datNEW -> MobileVersions.dat x1 | `MobileVersions.dat` .. `MobileVersions.datNEW` |
| 14 | +00:41.858 .. +00:41.872 | 148 | Config | 3 | 0 |  | DELETE commit x1; DELETE dynamicCommit x1; DELETE dbStruFinal x1 | `commit` .. `dbStruFinal` |
| 15 | +00:41.966 .. +00:42.005 | 148 | Params | 4 | 2 | begin 2, commit 2 | DELETE <guid>.ui x2; UPDATE <guid>.ui x2 | `789702c6-d272-4008-9e55-db6f10687ae0.ui` .. `97f2c291-1aa3-4d96-8266-82a958c7dfaf.ui` |
| 16 | +00:42.538 .. +00:43.564 | 148 | Files | 30 | 18 | begin 3, commit 3 | DELETE userDocs_ru.new x2; INSERT userDocs_ru.new x1; UPDATE userDocs_ru.new x4; DELETE userVocabulary_ru.new x2; INSERT userVocabulary_ru.new x1; UPDATE userVocabulary_ru.new x4; DELETE userPostings_ru.new x2; INSERT userPostings_ru.new x1; ... +10 more | `userDocs_ru.new` .. `userPostings_ru.new` |
