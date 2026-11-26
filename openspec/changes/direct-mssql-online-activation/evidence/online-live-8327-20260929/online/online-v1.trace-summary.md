# Trace report: online-v1

- database: `ibcmd_rs_05_online_a1` on `localhost`, Microsoft SQL Server 2025 (RTM-GDR) (KB5122770) - 17.0.1135.8 (X64)
- session: `ibcmd_rs_04_online_1`, predicate `sqlserver.database_id = 45`
- command: `& pwsh @argsList`
- exit code 0, command 17.1 s; trace window 20.9 s (first statement start to last statement end)
- events 130; dropped by XE: 0

## Volume

- 117 statement events (90 rpc, 27 batch), 52 normalized groups, total statement time 3.90 s
- by verb: SELECT 97, BEGIN 5, COMMIT 4, INSERT 4, UPDATE 2, SET 2, EXEC 2, DELETE 1
- user transactions 7 (commit 6, rollback 0, unfinished 1); statements outside a user transaction 46
- sessions: spid 197 `1CV83 Server` 64 statements; spid 280 `1CV83 Server` 43 statements; spid 146 `ibcmd-rs` 3 statements; spid 77 `ibcmd-rs` 2 statements; spid 80 `ibcmd-rs` 1 statements; spid 99 `ibcmd-rs` 3 statements; spid 111 `ibcmd-rs` 1 statements
- SQL errors reported 0, statements with a non-OK result 0

## Writes to service tables (name shapes)

| first seq | table | op | name shape | statements | rows | declared bytes |
|---|---|---|---|---|---|---|
| 117 | ConfigSave | DELETE | `` | 2 | 0 | 0 |
| 117 | ConfigSave | INSERT | `FileName` | 1 | 0 | 0 |
| 117 | ConfigSave | INSERT | `<guid>` | 1 | 0 | 0 |
| 117 | ConfigSave | INSERT | `<guid>.<n>` | 1 | 0 | 0 |
| 117 | ConfigSave | INSERT | `versions` | 1 | 0 | 0 |
| 130 |  | INSERT | `` | 15 | 0 | 0 |
| 130 | Config | INSERT | `<guid>_dynupdate_<guid>` | 1 | 0 | 0 |
| 130 | Config | INSERT | `<guid>_dynupdate_<guid>.<n>` | 1 | 0 | 0 |
| 130 | Config | DELETE | `root` | 1 | 0 | 0 |
| 130 | Config | INSERT | `root` | 1 | 0 | 0 |
| 130 | Config | DELETE | `version` | 1 | 0 | 0 |
| 130 | Config | INSERT | `version` | 1 | 0 | 0 |
| 130 | Config | INSERT | `versions_dynupdate_<guid>` | 1 | 0 | 0 |

Every write in order: `service-writes.tsv` (28 lines), decoded small payloads: `payloads.jsonl` (0).

## Writes to object (data) tables

| table | op | statements | rows | total ms |
|---|---|---|---|---|
| _Const9525 | UPDATE | 1 | 1 | 8.1 |
| _ConstChngR9527 | UPDATE | 1 | 0 | 76.2 |
| _ConstChngR9527 | INSERT | 3 | 3 | 31.1 |

## DDL

0 DDL statements (verbatim in `ddl.sql`), 0 temporary-table DDL statements not listed.

## Transactions

| tx | spid | begin | duration | state | statements | writes to service tables |
|---|---|---|---|---|---|---|
| 3149548535 | 197 | +00:00.012 | 3.8 ms | Commit | 3 |  |
| 3149549582 | 197 | +00:00.843 | 0.0 ms | open | 57 |  |
| 3149552992 | 280 | +00:02.245 | 9.5 ms | Commit | 3 |  |
| 3149555594 | 280 | +00:03.261 | 13.9 ms | Commit | 3 |  |
| 3149559263 | 280 | +00:05.543 | 10.6 ms | Commit | 3 |  |
| 3149592718 | 80 | +00:17.728 | 368.8 ms | Commit | 1 | ConfigSave 5 |
| 3149606340 | 111 | +00:20.668 | 190.8 ms | Commit | 1 | Config 7, ConfigSave 1 |

## Slowest statements

- 831.0 ms at +00:14.099 (seq 112, G0050): `exec sp_executesql stmt='?',params='?'`
- 571.9 ms at +00:17.530 (seq 117, G0051): `SET NOCOUNT ON; SET XACT_ABORT ON; USE [ibcmd_rs_05_online_a1]; BEGIN TRAN; DELETE FROM ConfigSave; INSERT INTO ConfigSave (FileName, Creation, Modified, Att...`
- 445.6 ms at +00:13.228 (seq 108, G0048): `SELECT CurrentSchema, NewGenCreated, NewGenDropped FROM SchemaStorage WHERE SchemaID = ?`
- 431.7 ms at +00:20.446 (seq 130, G0052): `SET NOCOUNT ON; SET XACT_ABORT ON; USE [ibcmd_rs_05_online_a1]; SET TRANSACTION ISOLATION LEVEL SERIALIZABLE; BEGIN TRY BEGIN TRANSACTION; DECLARE @LockResul...`
- 412.4 ms at +00:01.225 (seq 28, G0018): `SELECT FileName,Creation,Modified,Attributes,DataSize FROM Config WHERE PartNo = ? and FileName LIKE @P1`
- 227.8 ms at +00:19.394 (seq 123, G0048): `SELECT CurrentSchema, NewGenCreated, NewGenDropped FROM SchemaStorage WHERE SchemaID = ?`
- 76.2 ms at +00:02.389 (seq 79, G0038): `UPDATE T1 SET _MessageNo = CAST(NULL AS NUMERIC(?...)) FROM dbo._ConstChngR9527 T1 WHERE (T1._ConstID = @P1 AND (T1._NodeTRef = 0x? AND T1._NodeRRef IN (@P2,...`
- 64.9 ms at +00:01.818 (seq 39, G0023): `SELECT 0x?, T1._IDRRef, T1._Fld2625, T1._Fld2621, T1._Description, T1._Fld2621, T1._Marked, CAST(@P1 AS NVARCHAR(?)), CAST(@P2 AS NVARCHAR(?)), CAST(@P3 AS N...`
- 62.6 ms at +00:02.211 (seq 63, G0033): `SELECT T6._LineNo1325, T6._Fld1326RRef, ? AS SDBL_IDENTITY FROM dbo._Node989_VT1324 T6 INNER JOIN dbo._Node989 T7 ON T7._IDRRef = T6._Node989_IDRRef WHERE T6...`
- 55.9 ms at +00:02.154 (seq 53, G0032): `SELECT T4._LineNo1321, T4._Fld1322RRef, T4._Fld1323RRef, ? AS SDBL_IDENTITY FROM dbo._Node989_VT1320 T4 INNER JOIN dbo._Node989 T5 ON T5._IDRRef = T4._Node98...`

## Files

`groups.md` `groups.tsv` `timeline.md` `service-writes.tsv` `payloads.jsonl` `data-writes.tsv` `ddl.sql` `transactions.tsv` `slowest.tsv` `sessions.tsv` `events.tsv` `trace-meta.json` `session.sql` `command.log`
