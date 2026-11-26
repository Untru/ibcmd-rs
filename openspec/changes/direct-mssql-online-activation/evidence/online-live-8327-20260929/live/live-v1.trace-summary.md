# Trace report: live-v1

- database: `ibcmd_rs_05_online_b1` on `localhost`, Microsoft SQL Server 2025 (RTM-GDR) (KB5122770) - 17.0.1135.8 (X64)
- session: `ibcmd_rs_04_online_1`, predicate `sqlserver.database_id = 47`
- command: `& pwsh @argsList`
- exit code 1, command 50.1 s; trace window 33.4 s (first statement start to last statement end)
- events 160; dropped by XE: 0

## Volume

- 144 statement events (113 rpc, 31 batch), 60 normalized groups, total statement time 3.90 s
- by verb: SELECT 124, BEGIN 7, COMMIT 6, INSERT 3, UPDATE 2, SET 1, EXEC 1
- user transactions 8 (commit 7, rollback 1, unfinished 0); statements outside a user transaction 68
- sessions: spid 158 `1CV83 Server` 67 statements; spid 159 `1CV83 Server` 45 statements; spid 150 `1CV83 Server` 29 statements; spid 270 `ibcmd-rs` 3 statements
- SQL errors reported 0, statements with a non-OK result 0

## Writes to service tables (name shapes)

| first seq | table | op | name shape | statements | rows | declared bytes |
|---|---|---|---|---|---|---|

Every write in order: `service-writes.tsv` (0 lines), decoded small payloads: `payloads.jsonl` (0).

## Writes to object (data) tables

| table | op | statements | rows | total ms |
|---|---|---|---|---|
| _Const9525 | UPDATE | 1 | 1 | 12.5 |
| _ConstChngR9527 | UPDATE | 1 | 0 | 65.1 |
| _ConstChngR9527 | INSERT | 3 | 3 | 64.5 |

## DDL

0 DDL statements (verbatim in `ddl.sql`), 0 temporary-table DDL statements not listed.

## Transactions

| tx | spid | begin | duration | state | statements | writes to service tables |
|---|---|---|---|---|---|---|
| 3161717578 | 158 | +00:00.403 | 33.86 s | Rollback | 58 |  |
| 3161720842 | 159 | +00:03.058 | 4.5 ms | Commit | 3 |  |
| 3161721050 | 159 | +00:03.278 | 22.8 ms | Commit | 3 |  |
| 3161724474 | 159 | +00:05.379 | 11.2 ms | Commit | 3 |  |
| 3161724801 | 159 | +00:05.595 | 8.8 ms | Commit | 3 |  |
| 3161777416 | 150 | +00:33.198 | 46.9 ms | Commit | 3 |  |
| 3161778147 | 150 | +00:33.360 | 27.2 ms | Commit | 3 |  |
| 3161778264 | 281 | +00:33.455 | 249.3 ms | Commit | 0 |  |

## Slowest statements

- 972.0 ms at +00:01.363 (seq 37, G0022): `SELECT FileName,Creation,Modified,Attributes,DataSize FROM Config WHERE PartNo = ? and FileName LIKE @P1`
- 229.5 ms at +00:27.947 (seq 129, G0053): `select xtype from sys.sysobjects where id = object_id(@P1)`
- 222.4 ms at +00:28.286 (seq 132, G0057): `SELECT TOP ? ID, Name, Descr, OSName, Changed, RolesID, Show, Data, EAuth, AdmRole, UsSprH, Email FROM v8users WHERE OSName = @P1 AND ID <> @P2 ORDER BY OSName`
- 213.2 ms at +00:28.951 (seq 138, G0060): `exec sp_executesql stmt='?',params='?'`
- 208.7 ms at +00:27.409 (seq 125, G0053): `select xtype from sys.sysobjects where id = object_id(@P1)`
- 196.0 ms at +00:03.254 (seq 81, G0042): `SELECT T6._LineNo1325, T6._Fld1326RRef, ? AS SDBL_IDENTITY FROM dbo._Node989_VT1324 T6 INNER JOIN dbo._Node989 T7 ON T7._IDRRef = T6._Node989_IDRRef WHERE T6...`
- 182.0 ms at +00:02.579 (seq 48, G0027): `SELECT 0x?, T1._IDRRef, T1._Fld2625, T1._Fld2621, T1._Description, T1._Fld2621, T1._Marked, CAST(@P1 AS NVARCHAR(?)), CAST(@P2 AS NVARCHAR(?)), CAST(@P3 AS N...`
- 111.6 ms at +00:01.014 (seq 32, G0018): `SELECT T1._Fld1231, T1._Fld1229, T1._Fld1235RRef, T1._Fld8773 FROM dbo._InfoRg1228 T1 WHERE (T1._Fld1230RRef = @P1) AND T1._Fld1239 = 0x? OR (T1._Fld1235RRef...`
- 102.4 ms at +00:00.776 (seq 29, G0015): `SELECT T1._IDRRef FROM dbo._Node3023 T1 WHERE ((T1._Fld2683 = @P1)) AND (T1._PredefinedID = @P2)`
- 96.2 ms at +00:03.142 (seq 73, G0041): `SELECT T4._LineNo1321, T4._Fld1322RRef, T4._Fld1323RRef, ? AS SDBL_IDENTITY FROM dbo._Node989_VT1320 T4 INNER JOIN dbo._Node989 T5 ON T5._IDRRef = T4._Node98...`

## Files

`groups.md` `groups.tsv` `timeline.md` `service-writes.tsv` `payloads.jsonl` `data-writes.tsv` `ddl.sql` `transactions.tsv` `slowest.tsv` `sessions.tsv` `events.tsv` `trace-meta.json` `session.sql` `command.log`
