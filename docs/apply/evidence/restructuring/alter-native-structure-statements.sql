-- The structure statements of the NATIVE apply of a third attribute (_Fld11036) on our twin ibcmd_rs_04_ddl_a2_own2 whose catalog table had got the second
-- attribute (_Fld11035) by ALTER TABLE ADD (last in the physical order): the object is rebuilt and the column is put before _Fld2683.
-- From the Extended Events trace xe/alter_native, execution order; the binary parameters are cut.

-- 12:47:00.872000 sid=68 sql_batch_completed rows=0 dur=15783us
create table dbo._Reference20NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_ParentIDRRef binary(16) not null,
_Folder binary(1) not null,
_Code nvarchar(9) not null,
_Description nvarchar(150) not null,
_Fld151 binary(1),
_Fld152 binary(1),
_Fld153 binary(1),
_Fld154 binary(1),
_Fld949RRef binary(16),
_Fld6357RRef binary(16),
_Fld11034 nvarchar(50),
_Fld11035 nvarchar(20),
_Fld11036 nvarchar(30),
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference20NG SET (LOCK_ESCALATION = DISABLE);

-- 12:47:00.880000 sid=68 sql_batch_completed rows=0 dur=5400us
create table dbo._Reference20_VT155NG (
_Reference20_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo156 numeric(5, 0) not null,
_Fld157RRef binary(16) not null,
_Fld158_TYPE binary(1) not null,
_Fld158_L binary(1) not null,
_Fld158_N numeric(17, 5) not null,
_Fld158_T datetime2(0) not null,
_Fld158_S nvarchar(1024) not null,
_Fld158_RTRef binary(4) not null,
_Fld158_RRRef binary(16) not null,
_Fld1473 nvarchar(max) not null
)
;alter table dbo._Reference20_VT155NG SET (LOCK_ESCALATION = DISABLE);

-- 12:47:00.887000 sid=68 sql_batch_completed rows=0 dur=5294us
create table dbo._Reference20_VT159NG (
_Reference20_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo160 numeric(5, 0) not null,
_Fld161RRef binary(16) not null,
_Fld162RRef binary(16) not null,
_Fld163 nvarchar(500) not null,
_Fld164 nvarchar(max) not null,
_Fld165 nvarchar(100) not null,
_Fld166 nvarchar(50) not null,
_Fld167 nvarchar(50) not null,
_Fld168 nvarchar(100) not null,
_Fld169 nvarchar(100) not null,
_Fld170 nvarchar(20) not null,
_Fld171 nvarchar(20) not null,
_Fld5024RRef binary(16) not null,
_Fld6753 nvarchar(max) not null
)
;alter table dbo._Reference20_VT159NG SET (LOCK_ESCALATION = DISABLE);

-- 12:47:00.947000 sid=68 sql_batch_completed rows=0 dur=4788us
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:00.950000 sid=68 sql_batch_completed rows=0 dur=2316us
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:00.953000 sid=68 sql_batch_completed rows=0 dur=1319us
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:00.955000 sid=68 sql_batch_completed rows=0 dur=1632us
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:00.958000 sid=68 sql_batch_completed rows=0 dur=2533us
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:00.965000 sid=68 sql_batch_completed rows=0 dur=6234us
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:00.974000 sid=68 sql_batch_completed rows=0 dur=9031us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:00.983000 sid=68 sql_batch_completed rows=0 dur=8521us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:00.986000 sid=68 sql_batch_completed rows=0 dur=1354us
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:00.988000 sid=68 sql_batch_completed rows=0 dur=2051us
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:00.993000 sid=68 rpc_completed rows=1 dur=2050us
-- args: ,N'@P1 varbinary(max)',0x<5398 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 12:47:01.002000 sid=68 sql_batch_completed rows=0 dur=1198us
drop index _Reference20_1NG on dbo._Reference20NG;

-- 12:47:01.004000 sid=68 sql_batch_completed rows=0 dur=1264us
drop index _Reference20_2NG on dbo._Reference20NG;

-- 12:47:01.007000 sid=68 sql_batch_completed rows=0 dur=2849us
drop index _Reference20_3NG on dbo._Reference20NG;

-- 12:47:01.009000 sid=68 sql_batch_completed rows=0 dur=797us
drop index _Reference20_4NG on dbo._Reference20NG;

-- 12:47:01.011000 sid=68 sql_batch_completed rows=0 dur=1674us
drop index _Reference20_5NG on dbo._Reference20NG;

-- 12:47:01.013000 sid=68 sql_batch_completed rows=0 dur=1964us
drop index _Reference20_S_HPKNG on dbo._Reference20NG;

-- 12:47:01.017000 sid=68 sql_batch_completed rows=0 dur=2414us
drop index _Reference20_VT155_SKNG on dbo._Reference20_VT155NG;

-- 12:47:01.020000 sid=68 sql_batch_completed rows=0 dur=1586us
drop index _Reference20_VT159_1NG on dbo._Reference20_VT159NG;

-- 12:47:01.022000 sid=68 sql_batch_completed rows=0 dur=1422us
drop index _Reference20_VT159_2NG on dbo._Reference20_VT159NG;

-- 12:47:01.024000 sid=68 sql_batch_completed rows=0 dur=1325us
drop index _Reference20_VT159_SKNG on dbo._Reference20_VT159NG;

-- 12:47:01.033000 sid=68 rpc_completed rows=15 dur=8375us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Reference20NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Code, _Description, _Fld151, _Fld152, _Fld153, _Fld154, _Fld949RRef, _Fld6357RRef, _Fld11034, _Fld11035, _Fld11036, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._ParentIDRRef,
T1._Folder,
T1._Code,
T1._Description,
T1._Fld151,
T1._Fld152,
T1._Fld153,
T1._Fld154,
T1._Fld949RRef,
T1._Fld6357RRef,
T1._Fld11034,
T1._Fld11035,
CAST(CASE WHEN T1._Folder = 0x01 THEN @P1 END AS NVARCHAR(30)),
T1._Fld2683
FROM dbo._Reference20 T1 WITH(NOLOCK);

-- 12:47:01.047000 sid=68 sql_batch_completed rows=29 dur=13461us
INSERT INTO dbo._Reference20_VT155NG WITH(TABLOCK) (_LineNo156, _Fld157RRef, _Fld158_TYPE, _Fld158_L, _Fld158_N, _Fld158_T, _Fld158_S, _Fld158_RTRef, _Fld158_RRRef, _Fld1473, _Fld2683, _Reference20_IDRRef, _KeyField) SELECT
T2._LineNo156,
T2._Fld157RRef,
T2._Fld158_TYPE,
T2._Fld158_L,
T2._Fld158_N,
T2._Fld158_T,
T2._Fld158_S,
T2._Fld158_RTRef,
T2._Fld158_RRRef,
T2._Fld1473,
T2._Fld2683,
T2._Reference20_IDRRef,
T2._KeyField
FROM dbo._Reference20_VT155 T2 WITH(NOLOCK);

-- 12:47:01.055000 sid=68 sql_batch_completed rows=18 dur=7999us
INSERT INTO dbo._Reference20_VT159NG WITH(TABLOCK) (_LineNo160, _Fld161RRef, _Fld162RRef, _Fld163, _Fld164, _Fld165, _Fld166, _Fld167, _Fld168, _Fld169, _Fld170, _Fld171, _Fld5024RRef, _Fld6753, _Fld2683, _Reference20_IDRRef, _KeyField) SELECT
T3._LineNo160,
T3._Fld161RRef,
T3._Fld162RRef,
T3._Fld163,
T3._Fld164,
T3._Fld165,
T3._Fld166,
T3._Fld167,
T3._Fld168,
T3._Fld169,
T3._Fld170,
T3._Fld171,
T3._Fld5024RRef,
T3._Fld6753,
T3._Fld2683,
T3._Reference20_IDRRef,
T3._KeyField
FROM dbo._Reference20_VT159 T3 WITH(NOLOCK);

-- 12:47:01.069000 sid=68 sql_batch_completed rows=15 dur=12274us
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:01.072000 sid=68 sql_batch_completed rows=15 dur=2188us
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:01.076000 sid=68 sql_batch_completed rows=15 dur=3713us
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:01.079000 sid=68 sql_batch_completed rows=15 dur=2278us
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:01.083000 sid=68 sql_batch_completed rows=15 dur=3399us
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:01.115000 sid=68 sql_batch_completed rows=90 dur=31311us
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:01.123000 sid=68 sql_batch_completed rows=29 dur=7998us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:01.132000 sid=68 sql_batch_completed rows=18 dur=8457us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:01.137000 sid=68 sql_batch_completed rows=18 dur=4393us
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:01.141000 sid=68 sql_batch_completed rows=18 dur=2675us
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:02.170000 sid=68 sql_batch_completed rows=0 dur=8152us
create table dbo._DbCopiesUpdatesNG (
_CopyId binary(16) not null,
_TrNum int,
_TrTime datetime2(0),
_UpdateId binary(16),
_LastUpdateResult numeric(2, 0),
_LastUpdateError varbinary(max)
)
;alter table dbo._DbCopiesUpdatesNG SET (LOCK_ESCALATION = DISABLE);

-- 12:47:02.183000 sid=68 sql_batch_completed rows=0 dur=10807us
CREATE UNIQUE CLUSTERED INDEX _DbCopiesUpdates_1NG ON dbo._DbCopiesUpdatesNG (_CopyId, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:02.190000 sid=68 rpc_completed rows=1 dur=4451us
-- args: ,N'@P1 varbinary(max)',0x<6286 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 12:47:02.203000 sid=68 sql_batch_completed rows=0 dur=8645us
INSERT INTO dbo._DbCopiesUpdatesNG WITH(TABLOCK) (_CopyId, _TrNum, _TrTime) SELECT
T1._CopyId,
(T1._TrNum + 0.0),
T1._TrTime
FROM dbo._DbCopiesUpdates T1 WITH(NOLOCK);

-- 12:47:02.209000 sid=68 sql_batch_completed rows=0 dur=3044us
create table dbo._DbCopiesNG (
_CopyId binary(16) not null,
_CopyName nvarchar(256) not null,
_UseIntAccelerator binary(1) not null,
_ReplType int not null,
_DbType int not null,
_DbServer nvarchar(256) not null,
_DbName nvarchar(256) not null,
_DbUser nvarchar(256) not null,
_DbPassword nvarchar(256) not null,
_CreateDb binary(1) not null,
_Version numeric(9, 0) not null,
_StorageVariant numeric(1, 0) not null
)
;alter table dbo._DbCopiesNG SET (LOCK_ESCALATION = DISABLE);

-- 12:47:02.212000 sid=68 sql_batch_completed rows=0 dur=2445us
CREATE UNIQUE CLUSTERED INDEX _DbCopies_1NG ON dbo._DbCopiesNG (_CopyId, _CopyName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:02.213000 sid=68 rpc_completed rows=1 dur=547us
-- args: ,N'@P1 varbinary(max)',0x<7814 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 12:47:02.216000 sid=68 rpc_completed rows=0 dur=1148us
-- args: ,N'@P1 numeric(10)',0
INSERT INTO dbo._DbCopiesNG WITH(TABLOCK) (_CopyId, _CopyName, _UseIntAccelerator, _ReplType, _DbType, _DbServer, _DbName, _DbUser, _DbPassword, _CreateDb, _Version, _StorageVariant) SELECT
T1._CopyId,
T1._CopyName,
T1._UseIntAccelerator,
(T1._ReplType + 0.0),
(T1._DbType + 0.0),
T1._DbServer,
T1._DbName,
T1._DbUser,
T1._DbPassword,
T1._CreateDb,
T1._Version,
@P1
FROM dbo._DbCopies T1 WITH(NOLOCK);

-- 12:47:02.676000 sid=68 sql_batch_completed rows=0 dur=28648us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 12:47:02.681000 sid=68 sql_batch_completed rows=0 dur=4199us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 12:47:02.684000 sid=68 sql_batch_completed rows=0 dur=2435us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:02.687000 sid=68 sql_batch_completed rows=0 dur=1916us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:02.698000 sid=68 sql_batch_completed rows=0 dur=11096us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:02.700000 sid=68 rpc_completed rows=1 dur=806us
-- args: ,N'@P1 varbinary(max)',0x<8878 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 12:47:02.708000 sid=68 sql_batch_completed rows=0 dur=1966us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 12:47:02.710000 sid=68 sql_batch_completed rows=0 dur=1824us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 12:47:02.715000 sid=68 sql_batch_completed rows=0 dur=3319us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 12:47:02.973000 sid=68 sql_batch_completed rows=10000 dur=71121us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 12:47:03.038000 sid=68 sql_batch_completed rows=10000 dur=55838us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 12:47:03.048000 sid=68 sql_batch_completed rows=115 dur=10006us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 12:47:03.341000 sid=68 sql_batch_completed rows=10000 dur=60240us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 12:47:03.375000 sid=68 sql_batch_completed rows=10000 dur=27822us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 12:47:03.385000 sid=68 sql_batch_completed rows=575 dur=10059us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 12:47:03.587000 sid=68 sql_batch_completed rows=685 dur=13178us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=685);

-- 12:47:03.596000 sid=68 sql_batch_completed rows=667 dur=6098us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=667);

-- 12:47:03.634000 sid=68 sql_batch_completed rows=20685 dur=36239us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:03.743000 sid=68 sql_batch_completed rows=20685 dur=106933us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 12:47:03.797000 sid=68 sql_batch_completed rows=21357 dur=53210us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:04.851000 sid=68 rpc_completed rows=1 dur=1779us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 12:47:04.889000 sid=68 rpc_completed rows=1 dur=4731us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 12:47:06.096000 sid=68 rpc_completed rows=1 dur=218us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<888 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 12:47:06.152000 sid=68 rpc_completed rows=1 dur=1792us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF00EFAB36899F1118F4F00E04C680093,0x<7728 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 12:47:12.187000 sid=68 sql_batch_completed rows=1 dur=5719us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 12:47:12.218000 sid=68 sql_batch_completed rows=0 dur=27981us
drop table dbo._Reference20;

-- 12:47:12.222000 sid=68 sql_batch_completed rows=0 dur=2805us
drop table dbo._Reference20_VT155;

-- 12:47:12.225000 sid=68 sql_batch_completed rows=0 dur=2548us
drop table dbo._Reference20_VT159;

-- 12:47:12.230000 sid=68 sql_batch_completed rows=0 dur=4186us
drop table dbo._DbCopiesUpdates;

-- 12:47:12.233000 sid=68 sql_batch_completed rows=0 dur=2067us
drop table dbo._DbCopies;

-- 12:47:12.240000 sid=68 sql_batch_completed rows=0 dur=5748us
drop table dbo._ConfigChngR;

-- 12:47:12.249000 sid=68 sql_batch_completed rows=0 dur=7674us
drop table dbo._ConfigChngR_ExtProps;

-- 12:47:12.250000 sid=68 sql_batch_completed rows=1 dur=568us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 12:47:12.880000 sid=68 rpc_completed rows=30 dur=626916us
-- args: N'_Reference20NG',N'_Reference20'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 12:47:12.885000 sid=68 rpc_completed rows=25 dur=2512us
-- args: N'_Reference20_VT155NG',N'_Reference20_VT155'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 12:47:12.890000 sid=68 rpc_completed rows=25 dur=3016us
-- args: N'_Reference20_VT159NG',N'_Reference20_VT159'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 12:47:12.897000 sid=68 rpc_completed rows=16 dur=2188us
-- args: N'_Reference20._Reference20_1NG',N'_Reference20_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.902000 sid=68 rpc_completed rows=16 dur=3300us
-- args: N'_Reference20._Reference20_2NG',N'_Reference20_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.908000 sid=68 rpc_completed rows=16 dur=1750us
-- args: N'_Reference20._Reference20_3NG',N'_Reference20_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.916000 sid=68 rpc_completed rows=16 dur=2123us
-- args: N'_Reference20._Reference20_4NG',N'_Reference20_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.922000 sid=68 rpc_completed rows=16 dur=2413us
-- args: N'_Reference20._Reference20_5NG',N'_Reference20_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.930000 sid=68 rpc_completed rows=16 dur=1406us
-- args: N'_Reference20._Reference20_S_HPKNG',N'_Reference20_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.935000 sid=68 rpc_completed rows=16 dur=2145us
-- args: N'_Reference20_VT155._Reference20_VT155_SKNG',N'_Reference20_VT155_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.947000 sid=68 rpc_completed rows=16 dur=1354us
-- args: N'_Reference20_VT159._Reference20_VT159_SKNG',N'_Reference20_VT159_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.952000 sid=68 rpc_completed rows=16 dur=2823us
-- args: N'_Reference20_VT159._Reference20_VT159_1NG',N'_Reference20_VT159_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.978000 sid=68 rpc_completed rows=16 dur=11112us
-- args: N'_Reference20_VT159._Reference20_VT159_2NG',N'_Reference20_VT159_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.987000 sid=68 rpc_completed rows=25 dur=2965us
-- args: N'_DbCopiesUpdatesNG',N'_DbCopiesUpdates'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 12:47:12.995000 sid=68 rpc_completed rows=16 dur=2033us
-- args: N'_DbCopiesUpdates._DbCopiesUpdates_1NG',N'_DbCopiesUpdates_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:12.998000 sid=68 rpc_completed rows=25 dur=2490us
-- args: N'_DbCopiesNG',N'_DbCopies'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 12:47:13.004000 sid=68 rpc_completed rows=16 dur=1907us
-- args: N'_DbCopies._DbCopies_1NG',N'_DbCopies_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:13.008000 sid=68 rpc_completed rows=25 dur=2312us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 12:47:13.012000 sid=68 rpc_completed rows=25 dur=2141us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 12:47:13.016000 sid=68 rpc_completed rows=16 dur=1197us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:13.019000 sid=68 rpc_completed rows=16 dur=1553us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:13.024000 sid=68 rpc_completed rows=16 dur=1252us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 12:47:14.189000 sid=68 sql_batch_completed rows=0 dur=1263us
ALTER INDEX PK___ConfigC__AC8ED0C44A9B577C ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 12:47:15.206000 sid=68 rpc_completed rows=1 dur=296170us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1955726 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 12:47:15.700000 sid=68 rpc_completed rows=1 dur=260302us
-- args: ,N'@P1 varbinary(max)',0x<1955726 hex>
UPDATE DBSchema SET SerializedData = @P1;

