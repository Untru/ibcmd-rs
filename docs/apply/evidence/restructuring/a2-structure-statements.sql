-- 06:42:54.354000 sid=61 sql_batch_completed rows=0 dur=12143us
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
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference20NG SET (LOCK_ESCALATION = DISABLE);

-- 06:42:54.360000 sid=61 sql_batch_completed rows=0 dur=4126us
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

-- 06:42:54.378000 sid=61 sql_batch_completed rows=0 dur=16193us
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

-- 06:42:54.463000 sid=61 sql_batch_completed rows=0 dur=4268us
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.466000 sid=61 sql_batch_completed rows=0 dur=1221us
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.468000 sid=61 sql_batch_completed rows=0 dur=2167us
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.472000 sid=61 sql_batch_completed rows=0 dur=1551us
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.492000 sid=61 sql_batch_completed rows=0 dur=19175us
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.498000 sid=61 sql_batch_completed rows=0 dur=5172us
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:54.502000 sid=61 sql_batch_completed rows=0 dur=3071us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:54.507000 sid=61 sql_batch_completed rows=0 dur=4647us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:54.509000 sid=61 sql_batch_completed rows=0 dur=1127us
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.511000 sid=61 sql_batch_completed rows=0 dur=1022us
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.515000 sid=61 rpc_completed rows=1 dur=1564us
-- args: ,N'@P1 varbinary(max)',0x<5174 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 06:42:54.532000 sid=61 sql_batch_completed rows=0 dur=2732us
drop index _Reference20_1NG on dbo._Reference20NG;

-- 06:42:54.536000 sid=61 sql_batch_completed rows=0 dur=1729us
drop index _Reference20_2NG on dbo._Reference20NG;

-- 06:42:54.539000 sid=61 sql_batch_completed rows=0 dur=2469us
drop index _Reference20_3NG on dbo._Reference20NG;

-- 06:42:54.543000 sid=61 sql_batch_completed rows=0 dur=1770us
drop index _Reference20_4NG on dbo._Reference20NG;

-- 06:42:54.545000 sid=61 sql_batch_completed rows=0 dur=1843us
drop index _Reference20_5NG on dbo._Reference20NG;

-- 06:42:54.548000 sid=61 sql_batch_completed rows=0 dur=2256us
drop index _Reference20_S_HPKNG on dbo._Reference20NG;

-- 06:42:54.551000 sid=61 sql_batch_completed rows=0 dur=1525us
drop index _Reference20_VT155_SKNG on dbo._Reference20_VT155NG;

-- 06:42:54.555000 sid=61 sql_batch_completed rows=0 dur=1239us
drop index _Reference20_VT159_1NG on dbo._Reference20_VT159NG;

-- 06:42:54.557000 sid=61 sql_batch_completed rows=0 dur=1047us
drop index _Reference20_VT159_2NG on dbo._Reference20_VT159NG;

-- 06:42:54.558000 sid=61 sql_batch_completed rows=0 dur=1145us
drop index _Reference20_VT159_SKNG on dbo._Reference20_VT159NG;

-- 06:42:54.563000 sid=61 rpc_completed rows=14 dur=4121us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Reference20NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Code, _Description, _Fld151, _Fld152, _Fld153, _Fld154, _Fld949RRef, _Fld6357RRef, _Fld11034, _Fld2683) SELECT
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
CAST(CASE WHEN T1._Folder = 0x01 THEN @P1 END AS NVARCHAR(50)),
T1._Fld2683
FROM dbo._Reference20 T1 WITH(NOLOCK);

-- 06:42:54.569000 sid=61 sql_batch_completed rows=29 dur=5589us
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

-- 06:42:54.575000 sid=61 sql_batch_completed rows=18 dur=5001us
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

-- 06:42:54.583000 sid=61 sql_batch_completed rows=14 dur=6366us
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.587000 sid=61 sql_batch_completed rows=14 dur=2902us
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.590000 sid=61 sql_batch_completed rows=14 dur=2492us
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.595000 sid=61 sql_batch_completed rows=14 dur=3962us
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.601000 sid=61 sql_batch_completed rows=14 dur=5022us
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.619000 sid=61 sql_batch_completed rows=84 dur=17182us
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:54.624000 sid=61 sql_batch_completed rows=29 dur=4990us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:54.632000 sid=61 sql_batch_completed rows=18 dur=7143us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:54.637000 sid=61 sql_batch_completed rows=18 dur=3132us
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:54.643000 sid=61 sql_batch_completed rows=18 dur=4573us
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:55.255000 sid=61 sql_batch_completed rows=0 dur=3408us
create table dbo._DbCopiesUpdatesNG (
_CopyId binary(16) not null,
_TrNum int,
_TrTime datetime2(0),
_UpdateId binary(16),
_LastUpdateResult numeric(2, 0),
_LastUpdateError varbinary(max)
)
;alter table dbo._DbCopiesUpdatesNG SET (LOCK_ESCALATION = DISABLE);

-- 06:42:55.259000 sid=61 sql_batch_completed rows=0 dur=3770us
CREATE UNIQUE CLUSTERED INDEX _DbCopiesUpdates_1NG ON dbo._DbCopiesUpdatesNG (_CopyId, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:55.260000 sid=61 rpc_completed rows=1 dur=414us
-- args: ,N'@P1 varbinary(max)',0x<6062 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 06:42:55.265000 sid=61 sql_batch_completed rows=0 dur=3131us
INSERT INTO dbo._DbCopiesUpdatesNG WITH(TABLOCK) (_CopyId, _TrNum, _TrTime) SELECT
T1._CopyId,
(T1._TrNum + 0.0),
T1._TrTime
FROM dbo._DbCopiesUpdates T1 WITH(NOLOCK);

-- 06:42:55.272000 sid=61 sql_batch_completed rows=0 dur=4844us
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

-- 06:42:55.278000 sid=61 sql_batch_completed rows=0 dur=4867us
CREATE UNIQUE CLUSTERED INDEX _DbCopies_1NG ON dbo._DbCopiesNG (_CopyId, _CopyName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:55.279000 sid=61 rpc_completed rows=1 dur=337us
-- args: ,N'@P1 varbinary(max)',0x<7590 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 06:42:55.283000 sid=61 rpc_completed rows=0 dur=2114us
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

-- 06:42:55.820000 sid=61 sql_batch_completed rows=0 dur=11771us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 06:42:55.824000 sid=61 sql_batch_completed rows=0 dur=2822us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 06:42:55.829000 sid=61 sql_batch_completed rows=0 dur=4563us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:55.833000 sid=61 sql_batch_completed rows=0 dur=1446us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:55.838000 sid=61 sql_batch_completed rows=0 dur=4225us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:55.841000 sid=61 rpc_completed rows=1 dur=2522us
-- args: ,N'@P1 varbinary(max)',0x<8654 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 06:42:55.851000 sid=61 sql_batch_completed rows=0 dur=1998us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 06:42:55.854000 sid=61 sql_batch_completed rows=0 dur=1431us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 06:42:55.861000 sid=61 sql_batch_completed rows=0 dur=3055us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 06:42:56.028000 sid=61 sql_batch_completed rows=10000 dur=29590us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 06:42:56.052000 sid=61 sql_batch_completed rows=10000 dur=22367us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 06:42:56.055000 sid=61 sql_batch_completed rows=115 dur=2702us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 06:42:56.249000 sid=61 sql_batch_completed rows=10000 dur=29221us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 06:42:56.264000 sid=61 sql_batch_completed rows=10000 dur=13743us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 06:42:56.267000 sid=61 sql_batch_completed rows=575 dur=3014us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 06:42:56.351000 sid=61 sql_batch_completed rows=685 dur=5114us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=685);

-- 06:42:56.356000 sid=61 sql_batch_completed rows=667 dur=2911us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=667);

-- 06:42:56.389000 sid=61 sql_batch_completed rows=20685 dur=31653us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:56.425000 sid=61 sql_batch_completed rows=20685 dur=34373us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 06:42:56.457000 sid=61 sql_batch_completed rows=21357 dur=31375us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:42:58.361000 sid=61 rpc_completed rows=1 dur=403us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 06:42:58.391000 sid=61 rpc_completed rows=1 dur=416us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 06:42:58.950000 sid=61 rpc_completed rows=1 dur=154us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<888 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 06:42:58.981000 sid=61 rpc_completed rows=1 dur=155us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF00EFAB36899F1118F4F00E04C680093,0x<7728 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 06:43:03.297000 sid=61 sql_batch_completed rows=1 dur=3030us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 06:43:03.333000 sid=61 sql_batch_completed rows=0 dur=27916us
drop table dbo._Reference20;

-- 06:43:03.342000 sid=61 sql_batch_completed rows=0 dur=5974us
drop table dbo._Reference20_VT155;

-- 06:43:03.347000 sid=61 sql_batch_completed rows=0 dur=3828us
drop table dbo._Reference20_VT159;

-- 06:43:03.353000 sid=61 sql_batch_completed rows=0 dur=5172us
drop table dbo._DbCopiesUpdates;

-- 06:43:03.363000 sid=61 sql_batch_completed rows=0 dur=7459us
drop table dbo._DbCopies;

-- 06:43:03.376000 sid=61 sql_batch_completed rows=0 dur=10700us
drop table dbo._ConfigChngR;

-- 06:43:03.381000 sid=61 sql_batch_completed rows=0 dur=3560us
drop table dbo._ConfigChngR_ExtProps;

-- 06:43:03.383000 sid=61 sql_batch_completed rows=1 dur=1528us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 06:43:03.499000 sid=61 rpc_completed rows=25 dur=99840us
-- args: N'_Reference20NG',N'_Reference20'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 06:43:03.505000 sid=61 rpc_completed rows=25 dur=3201us
-- args: N'_Reference20_VT155NG',N'_Reference20_VT155'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 06:43:03.513000 sid=61 rpc_completed rows=25 dur=5517us
-- args: N'_Reference20_VT159NG',N'_Reference20_VT159'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 06:43:03.517000 sid=61 rpc_completed rows=16 dur=1724us
-- args: N'_Reference20._Reference20_1NG',N'_Reference20_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.523000 sid=61 rpc_completed rows=16 dur=2276us
-- args: N'_Reference20._Reference20_2NG',N'_Reference20_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.529000 sid=61 rpc_completed rows=16 dur=1939us
-- args: N'_Reference20._Reference20_3NG',N'_Reference20_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.538000 sid=61 rpc_completed rows=16 dur=6531us
-- args: N'_Reference20._Reference20_4NG',N'_Reference20_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.547000 sid=61 rpc_completed rows=16 dur=1827us
-- args: N'_Reference20._Reference20_5NG',N'_Reference20_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.556000 sid=61 rpc_completed rows=16 dur=1863us
-- args: N'_Reference20._Reference20_S_HPKNG',N'_Reference20_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.560000 sid=61 rpc_completed rows=16 dur=2026us
-- args: N'_Reference20_VT155._Reference20_VT155_SKNG',N'_Reference20_VT155_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.563000 sid=61 rpc_completed rows=16 dur=1307us
-- args: N'_Reference20_VT159._Reference20_VT159_SKNG',N'_Reference20_VT159_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.568000 sid=61 rpc_completed rows=16 dur=2852us
-- args: N'_Reference20_VT159._Reference20_VT159_1NG',N'_Reference20_VT159_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.571000 sid=61 rpc_completed rows=16 dur=1225us
-- args: N'_Reference20_VT159._Reference20_VT159_2NG',N'_Reference20_VT159_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.574000 sid=61 rpc_completed rows=25 dur=1576us
-- args: N'_DbCopiesUpdatesNG',N'_DbCopiesUpdates'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 06:43:03.578000 sid=61 rpc_completed rows=16 dur=1641us
-- args: N'_DbCopiesUpdates._DbCopiesUpdates_1NG',N'_DbCopiesUpdates_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.583000 sid=61 rpc_completed rows=25 dur=3242us
-- args: N'_DbCopiesNG',N'_DbCopies'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 06:43:03.588000 sid=61 rpc_completed rows=16 dur=3563us
-- args: N'_DbCopies._DbCopies_1NG',N'_DbCopies_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.597000 sid=61 rpc_completed rows=25 dur=5245us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 06:43:03.605000 sid=61 rpc_completed rows=25 dur=3797us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 06:43:03.611000 sid=61 rpc_completed rows=16 dur=4675us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.616000 sid=61 rpc_completed rows=16 dur=2217us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.621000 sid=61 rpc_completed rows=16 dur=1535us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 06:43:03.811000 sid=61 sql_batch_completed rows=0 dur=2363us
ALTER INDEX PK___ConfigC__AC8ED0C40D1D879F ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 06:43:03.866000 sid=61 rpc_completed rows=1 dur=25631us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1955502 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 06:43:03.956000 sid=61 rpc_completed rows=1 dur=45322us
-- args: ,N'@P1 varbinary(max)',0x<1955502 hex>
UPDATE DBSchema SET SerializedData = @P1;

