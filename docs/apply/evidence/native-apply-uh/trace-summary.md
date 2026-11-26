# Trace report: uh-case1-forms-exclusive

- database: `ibcmd_rs_04_trace_uha_c1` on `localhost`, Microsoft SQL Server 2025 (RTM-GDR) (KB5122770) - 17.0.1135.8 (X64)
- session: `ibcmd_rs_04_trace_1`, predicate `sqlserver.database_id = 41`
- command: `C:\Program Files\1cv8\8.3.27.2214\bin\ibcmd.exe infobase config apply --dbms=MSSQLServer --db-server=localhost --db-name=ibcmd_rs_04_trace_uha_c1 --data=F:\ibcmd\lab\04\trace\ibdata\ibcmd_rs_04_trace_uha_c1 --force --dynamic=disable`
- exit code 0, command 522.1 s; trace window 518.2 s (first statement start to last statement end)
- events 28753; dropped by XE: 0
- statements cut by the kit (over 32 KB): 109; rpc statements truncated by SQL Server at 2,000,000 chars (binary parameter over ~1,000,000 bytes): 6

## Volume

- 28701 statement events (28428 rpc, 273 batch), 208 normalized groups, total statement time 187.25 s
- by verb: SELECT 28326, DELETE 107, SET 97, UPDATE 72, INSERT 45, BEGIN 26, COMMIT 26, TRUNCATE 2
- user transactions 26 (commit 26, rollback 0, unfinished 0); statements outside a user transaction 28595
- sessions: spid 195 `-` 7237 statements; spid 197 `-` 100 statements; spid 201 `-` 11 statements; spid 203 `-` 11 statements; spid 221 `-` 11 statements; spid 224 `-` 11 statements; spid 230 `-` 11 statements; spid 236 `-` 11 statements; spid 250 `-` 11 statements; spid 253 `-` 11 statements; spid 255 `-` 11 statements; spid 256 `-` 11 statements
- SQL errors reported 0, statements with a non-OK result 0

## Writes to service tables (name shapes)

| first seq | table | op | name shape | statements | rows | declared bytes |
|---|---|---|---|---|---|---|
| 6796 | ConfigSave | DELETE | `LIKE %.new` | 1 | 0 | 0 |
| 6797 | Config | DELETE | `LIKE %.new` | 1 | 0 | 0 |
| 6799 | Config | DELETE | `<guid>.new` | 10 | 0 | 0 |
| 6800 | Config | INSERT | `<guid>.new` | 10 | 10 | 0 |
| 6801 | Config | DELETE | `<guid>.<n>.new` | 10 | 0 | 0 |
| 6802 | Config | INSERT | `<guid>.<n>.new` | 10 | 10 | 0 |
| 6839 | Config | DELETE | `root.new` | 1 | 0 | 0 |
| 6840 | Config | INSERT | `root.new` | 1 | 1 | 0 |
| 6841 | Config | DELETE | `version.new` | 1 | 0 | 0 |
| 6842 | Config | INSERT | `version.new` | 1 | 1 | 0 |
| 6843 | Config | DELETE | `versions.new` | 1 | 0 | 0 |
| 6844 | Config | INSERT | `versions.new` | 1 | 1 | 0 |
| 6849 | Files | INSERT | `MobileVersions.datNEW` | 1 | 1 | 0 |
| 6852 | Files | DELETE | `MobileVersions.datNEW` | 1 | 0 | 0 |
| 6853 | Files | UPDATE | `MobileVersions.datNEW` | 2 | 2 | 80 |
| 6863 | Params | INSERT | `DBNames.New` | 2 | 2 | 0 |
| 6866 | Params | DELETE | `DBNames.New` | 4 | 0 | 0 |
| 6867 | Params | UPDATE | `DBNames.New` | 2 | 2 | 6064656 |
| 21485 | _DbCopiesInfoBaseUse | UPDATE | `` | 1 | 2 | 0 |
| 21486 | _DbCopiesInitialLast | DELETE | `` | 1 | 0 | 0 |
| 21487 | _DbCopiesUpdates | DELETE | `` | 1 | 0 | 0 |
| 21494 | _DbCopiesTrChObj | TRUNCATE | `` | 1 | 0 | 0 |
| 21495 | _DbCopiesTrChanges | TRUNCATE | `` | 1 | 0 | 0 |
| 21499 | _ExtensionsRestructNGS | DELETE | `` | 1 | 0 | 0 |
| 28415 | Params | INSERT | `<guid>.sinew` | 16 | 16 | 0 |
| 28418 | Params | DELETE | `<guid>.sinew` | 16 | 0 | 0 |
| 28419 | Params | UPDATE | `<guid>.sinew` | 32 | 32 | 25480559 |
| 28560 | Config | INSERT | `commit` | 1 | 1 | 0 |
| 28563 | Config | DELETE | `commit` | 2 | 0 | 0 |
| 28564 | Config | UPDATE | `commit` | 1 | 1 | 0 |
| 28574 | Params | DELETE | `<guid>.si` | 16 | 16 | 0 |
| 28608 | Params | DELETE | `siVersions` | 1 | 0 | 0 |
| 28609 | Params | UPDATE | `siVersions` | 1 | 1 | 1273 |
| 28612 | Params | DELETE | `DynamicallyUpdated` | 1 | 0 | 0 |
| 28616 | Config | DELETE | `<guid>.<n>` | 10 | 10 | 0 |
| 28617 | Config | UPDATE | `<guid>.<n>.new` | 10 | 10 | 0 |
| 28618 | Config | DELETE | `<guid>` | 10 | 10 | 0 |
| 28619 | Config | UPDATE | `<guid>.new` | 10 | 10 | 0 |
| 28644 | Config | DELETE | `root` | 1 | 1 | 0 |
| 28645 | Config | UPDATE | `root.new` | 1 | 1 | 0 |
| 28646 | Config | DELETE | `version` | 1 | 1 | 0 |
| 28647 | Config | UPDATE | `version.new` | 1 | 1 | 0 |
| 28648 | Config | DELETE | `versions` | 1 | 1 | 0 |
| 28649 | Config | UPDATE | `versions.new` | 1 | 1 | 0 |
| 28662 | Config | DELETE | `DynamicallyUpdated` | 1 | 0 | 0 |
| 28663 | ConfigSave | DELETE | `LIKE %` | 1 | 0 | 0 |
| 28665 | Files | DELETE | `MobileVersions.dat` | 1 | 1 | 0 |
| 28668 | Config | DELETE | `dynamicCommit` | 1 | 0 | 0 |
| 28669 | Config | DELETE | `dbStruFinal` | 1 | 0 | 0 |
| 28677 | Params | INSERT | `<guid>.ui` | 2 | 2 | 0 |
| 28680 | Params | DELETE | `<guid>.ui` | 2 | 0 | 0 |
| 28681 | Params | UPDATE | `<guid>.ui` | 2 | 2 | 24753 |
| 28697 | Params | DELETE | `ibparams.inf` | 1 | 0 | 0 |
| 28698 | Params | UPDATE | `ibparams.inf` | 1 | 1 | 324 |
| 28706 | Params | DELETE | `locale.inf` | 1 | 0 | 0 |
| 28707 | Params | UPDATE | `locale.inf` | 1 | 1 | 112 |
| 28731 | Files | DELETE | `userDocs_ru.bin` | 1 | 0 | 0 |
| 28732 | Files | UPDATE | `userDocs_ru.new` | 1 | 0 | 0 |
| 28733 | Files | DELETE | `userVocabulary_ru.bin` | 1 | 0 | 0 |
| 28734 | Files | UPDATE | `userVocabulary_ru.new` | 1 | 0 | 0 |
| 28735 | Files | DELETE | `userPostings_ru.bin` | 1 | 0 | 0 |
| 28736 | Files | UPDATE | `userPostings_ru.new` | 1 | 0 | 0 |
| 28737 | Files | DELETE | `userDocs_en.bin` | 1 | 0 | 0 |
| 28738 | Files | UPDATE | `userDocs_en.new` | 1 | 0 | 0 |
| 28739 | Files | DELETE | `userVocabulary_en.bin` | 1 | 0 | 0 |
| 28740 | Files | UPDATE | `userVocabulary_en.new` | 1 | 0 | 0 |
| 28741 | Files | DELETE | `userPostings_en.bin` | 1 | 0 | 0 |
| 28742 | Files | UPDATE | `userPostings_en.new` | 1 | 0 | 0 |

Every write in order: `service-writes.tsv` (226 lines), decoded small payloads: `payloads.jsonl` (28).

## Writes to object (data) tables

none

## DDL

2 DDL statements (verbatim in `ddl.sql`), 0 temporary-table DDL statements not listed.
- TRUNCATE `_DbCopiesTrChObj` x1
- TRUNCATE `_DbCopiesTrChanges` x1

## Transactions

| tx | spid | begin | duration | state | statements | writes to service tables |
|---|---|---|---|---|---|---|
| 3130115493 | 279 | +04:39.305 | 49.1 ms | Commit | 4 | Files 2 |
| 3130117174 | 279 | +04:40.276 | 188.3 ms | Commit | 4 | Params 2 |
| 3130269240 | 195 | +05:44.359 | 202.1 ms | Commit | 6 | _DbCopiesInfoBaseUse 1, _DbCopiesInitialLast 1, _DbCopiesUpdates 1 |
| 3130270517 | 195 | +05:45.367 | 741.1 ms | Commit | 4 | Params 2 |
| 3130525981 | 195 | +08:12.556 | 99.5 ms | Commit | 4 | Params 2 |
| 3130526233 | 195 | +08:12.702 | 21.5 ms | Commit | 4 | Params 2 |
| 3130526412 | 195 | +08:12.795 | 35.4 ms | Commit | 4 | Params 2 |
| 3130527598 | 195 | +08:13.244 | 819.7 ms | Commit | 4 | Params 2 |
| 3130530351 | 195 | +08:14.163 | 82.2 ms | Commit | 4 | Params 2 |
| 3130531045 | 195 | +08:14.630 | 251.3 ms | Commit | 4 | Params 2 |
| 3130532951 | 195 | +08:15.699 | 376.2 ms | Commit | 4 | Params 2 |
| 3130533752 | 195 | +08:16.202 | 193.1 ms | Commit | 4 | Params 2 |
| 3130542457 | 195 | +08:21.132 | 500.5 ms | Commit | 4 | Params 2 |
| 3130544779 | 195 | +08:21.723 | 47.4 ms | Commit | 4 | Params 2 |
| 3130545016 | 195 | +08:21.784 | 4.3 ms | Commit | 4 | Params 2 |
| 3130545347 | 195 | +08:21.852 | 28.8 ms | Commit | 4 | Params 2 |
| 3130550888 | 195 | +08:23.345 | 352.2 ms | Commit | 4 | Params 2 |
| 3130551878 | 195 | +08:23.814 | 79.9 ms | Commit | 4 | Params 2 |
| 3130552091 | 195 | +08:23.940 | 32.7 ms | Commit | 4 | Params 2 |
| 3130552220 | 195 | +08:23.998 | 8.1 ms | Commit | 4 | Params 2 |
| 3130554239 | 195 | +08:24.628 | 7.1 ms | Commit | 4 | Config 2 |
| 3130562472 | 195 | +08:27.749 | 4.0 ms | Commit | 4 | Params 2 |
| 3130568862 | 195 | +08:30.386 | 119.0 ms | Commit | 4 | Params 2 |
| 3130569197 | 195 | +08:30.594 | 27.1 ms | Commit | 4 | Params 2 |
| 3130569259 | 195 | +08:30.638 | 17.6 ms | Commit | 4 | Params 2 |
| 3130569351 | 195 | +08:30.726 | 86.7 ms | Commit | 4 | Params 2 |

## Slowest statements

- 12.51 s at +04:15.049 (seq 6783, G0033): `SELECT FileName,Creation,Modified,Attributes,DataSize FROM Config WHERE PartNo = ? and FileName LIKE @P1`
- 10.64 s at +03:44.406 (seq 6776, G0033): `SELECT FileName,Creation,Modified,Attributes,DataSize FROM Config WHERE PartNo = ? and FileName LIKE @P1`
- 3.48 s at +00:10.378 (seq 103, G0033): `SELECT FileName,Creation,Modified,Attributes,DataSize FROM Config WHERE PartNo = ? and FileName LIKE @P1`
- 3.03 s at +04:35.550 (seq 6797, G0041): `DELETE FROM Config WHERE FileName LIKE '?' SELECT TOP ? FileName FROM Config WHERE FileName LIKE '?'`
- 2.65 s at +08:09.217 (seq 28412, G0033): `SELECT FileName,Creation,Modified,Attributes,DataSize FROM Config WHERE PartNo = ? and FileName LIKE @P1`
- 2.47 s at +04:31.355 (seq 6788, G0016): `SELECT Creation,Modified,Attributes,DataSize,BinaryData FROM Params WHERE FileName = @P1 ORDER BY PartNo`
- 1.74 s at +05:05.060 (seq 9029, G0033): `SELECT FileName,Creation,Modified,Attributes,DataSize FROM Config WHERE PartNo = ? and FileName LIKE @P1`
- 1.53 s at +08:32.933 (seq 28746, G0019): `SELECT Creation,Modified,Attributes,DataSize,BinaryData FROM Config WHERE FileName = @P1 ORDER BY PartNo`
- 1.39 s at +08:24.847 (seq 28576, G0199): `DELETE FROM params WHERE FileName = @P1 AND EXISTS(SELECT ? FROM Params WHERE FileName = @P2)`
- 1.25 s at +02:46.208 (seq 5486, G0019): `SELECT Creation,Modified,Attributes,DataSize,BinaryData FROM Config WHERE FileName = @P1 ORDER BY PartNo`

## Files

`groups.md` `groups.tsv` `timeline.md` `service-writes.tsv` `payloads.jsonl` `data-writes.tsv` `ddl.sql` `transactions.tsv` `slowest.tsv` `sessions.tsv` `events.tsv` `trace-meta.json` `session.sql` `command.log`
