# Trace report: online-v2b

- database: `ibcmd_rs_05_online_a1` on `localhost`, Microsoft SQL Server 2025 (RTM-GDR) (KB5122770) - 17.0.1135.8 (X64)
- session: `ibcmd_rs_04_online_1`, predicate `sqlserver.database_id = 45`
- command: `& pwsh @argsList`
- exit code 0, command 5.2 s; trace window 4.4 s (first statement start to last statement end)
- events 22; dropped by XE: 0

## Volume

- 20 statement events (15 rpc, 5 batch), 18 normalized groups, total statement time 710.0 ms
- by verb: SELECT 17, SET 1, EXEC 1, INSERT 1
- user transactions 1 (commit 1, rollback 0, unfinished 0); statements outside a user transaction 19
- sessions: spid 280 `1CV83 Server` 1 statements; spid 197 `1CV83 Server` 15 statements; spid 255 `ibcmd-rs` 3 statements; spid 267 `ibcmd-rs` 1 statements
- SQL errors reported 0, statements with a non-OK result 0

## Writes to service tables (name shapes)

| first seq | table | op | name shape | statements | rows | declared bytes |
|---|---|---|---|---|---|---|
| 22 |  | INSERT | `` | 15 | 0 | 0 |
| 22 | Config | INSERT | `<guid>_dynupdate_<guid>` | 1 | 0 | 0 |
| 22 | Config | INSERT | `<guid>_dynupdate_<guid>.<n>` | 1 | 0 | 0 |
| 22 | Config | DELETE | `root` | 1 | 0 | 0 |
| 22 | Config | INSERT | `root` | 1 | 0 | 0 |
| 22 | Config | DELETE | `version` | 1 | 0 | 0 |
| 22 | Config | INSERT | `version` | 1 | 0 | 0 |
| 22 | Config | INSERT | `versions_dynupdate_<guid>` | 1 | 0 | 0 |
| 22 | ConfigSave | DELETE | `` | 1 | 0 | 0 |

Every write in order: `service-writes.tsv` (23 lines), decoded small payloads: `payloads.jsonl` (0).

## Writes to object (data) tables

none

## DDL

0 DDL statements (verbatim in `ddl.sql`), 0 temporary-table DDL statements not listed.

## Transactions

| tx | spid | begin | duration | state | statements | writes to service tables |
|---|---|---|---|---|---|---|
| 3155464891 | 267 | +00:04.180 | 209.0 ms | Commit | 1 | Config 7, ConfigSave 1 |

## Slowest statements

- 328.5 ms at +00:04.067 (seq 22, G0018): `SET NOCOUNT ON; SET XACT_ABORT ON; USE [ibcmd_rs_05_online_a1]; SET TRANSACTION ISOLATION LEVEL SERIALIZABLE; BEGIN TRY BEGIN TRANSACTION; DECLARE @LockResul...`
- 283.9 ms at +00:03.555 (seq 19, G0017): `exec sp_executesql stmt='?',params='?'`
- 90.3 ms at +00:03.222 (seq 15, G0015): `SELECT CurrentSchema, NewGenCreated, NewGenDropped FROM SchemaStorage WHERE SchemaID = ?`
- 1.5 ms at +00:03.212 (seq 13, G0013): `SELECT TOP ? ID, Name, Descr, OSName, Changed, RolesID, Show, Data, EAuth, AdmRole, UsSprH, Email FROM v8users WITH(READCOMMITTED) WHERE Name = @P1 AND ID <>...`
- 1.3 ms at +00:03.217 (seq 14, G0014): `select xtype from sys.sysobjects where id = object_id(@P1)`
- 0.7 ms at +00:00.033 (seq 4, G0004): `SELECT TOP ? T1.IDRRef, T1.Marked_, T1.Date_Time_, T1.BusinessProcess_TYPE, T1.BusinessProcess_RTRef, T1.BusinessProcess_RRRef, T1.Name_, T1.Executed_, T1.Fl...`
- 0.6 ms at +00:00.000 (seq 1, G0001): `SELECT T1.IDRRef, T1.Fld552_, T6._EnumOrder FROM (SELECT T2.Fld2179RRef AS Fld2179RRef, T2.Marked_ AS Marked_, T2.Executed_ AS Executed_, T2.IDRRef AS IDRRef...`
- 0.5 ms at +00:00.012 (seq 3, G0003): `SELECT TOP ? T1.IDRRef, T1.Marked_, T1.Date_Time_, T1.BusinessProcess_TYPE, T1.BusinessProcess_RTRef, T1.BusinessProcess_RRRef, T1.Name_, T1.Executed_, T1.Fl...`
- 0.4 ms at +00:00.060 (seq 5, G0005): `SELECT T1._IDRRef, T1._Fld536, T1._Date_Time FROM dbo._BPr9X1 T1 WHERE ((T1._Fld2683 = @P1)) AND ((T1._IDRRef IN (@P2, @P3))) ORDER BY T1._IDRRef`
- 0.3 ms at +00:03.533 (seq 18, G0017): `exec sp_executesql stmt='?',params='?'`

## Files

`groups.md` `groups.tsv` `timeline.md` `service-writes.tsv` `payloads.jsonl` `data-writes.tsv` `ddl.sql` `transactions.tsv` `slowest.tsv` `sessions.tsv` `events.tsv` `trace-meta.json` `session.sql` `command.log`
