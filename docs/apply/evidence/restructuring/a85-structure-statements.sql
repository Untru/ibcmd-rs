-- The structure statements of the NATIVE apply of case a on the 8.5.1.1150 БСП (clone ibcmd_rs_04_ddl_bsp85_a, catalog _ДемоПартнеры,
-- a new String(50) attribute, field _Fld11262), from the Extended Events trace, in execution order. Compare with a2-structure-statements.sql
-- (8.3.27): the same three NG tables, ten indexes, copy and renames. 240 `ALTER INDEX ... SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)`
-- statements on primary keys of older tables (the 8.5 build drift) are cut after the first three.

-- 11:33:34.550000 sid=81 sql_batch_completed rows=0 dur=14414us
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
_Fld11262 nvarchar(50),
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference20NG SET (LOCK_ESCALATION = DISABLE);

-- 11:33:34.562000 sid=81 sql_batch_completed rows=0 dur=8734us
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

-- 11:33:34.571000 sid=81 sql_batch_completed rows=0 dur=6063us
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

-- 11:33:34.645000 sid=81 sql_batch_completed rows=0 dur=3575us
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.651000 sid=81 sql_batch_completed rows=0 dur=3036us
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.654000 sid=81 sql_batch_completed rows=0 dur=2206us
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.658000 sid=81 sql_batch_completed rows=0 dur=2249us
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.660000 sid=81 sql_batch_completed rows=0 dur=1182us
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.668000 sid=81 sql_batch_completed rows=0 dur=7892us
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:33:34.673000 sid=81 sql_batch_completed rows=0 dur=4578us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:33:34.681000 sid=81 sql_batch_completed rows=0 dur=7613us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:33:34.685000 sid=81 sql_batch_completed rows=0 dur=1864us
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.687000 sid=81 sql_batch_completed rows=0 dur=1544us
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.692000 sid=81 rpc_completed rows=1 dur=2303us
-- args: ,N'@P1 varbinary(max)',0x<5174 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 11:33:34.703000 sid=81 sql_batch_completed rows=0 dur=1965us
drop index _Reference20_1NG on dbo._Reference20NG;

-- 11:33:34.705000 sid=81 sql_batch_completed rows=0 dur=1622us
drop index _Reference20_2NG on dbo._Reference20NG;

-- 11:33:34.707000 sid=81 sql_batch_completed rows=0 dur=2042us
drop index _Reference20_3NG on dbo._Reference20NG;

-- 11:33:34.714000 sid=81 sql_batch_completed rows=0 dur=5007us
drop index _Reference20_4NG on dbo._Reference20NG;

-- 11:33:34.719000 sid=81 sql_batch_completed rows=0 dur=1798us
drop index _Reference20_5NG on dbo._Reference20NG;

-- 11:33:34.728000 sid=81 sql_batch_completed rows=0 dur=7367us
drop index _Reference20_S_HPKNG on dbo._Reference20NG;

-- 11:33:34.736000 sid=81 sql_batch_completed rows=0 dur=2314us
drop index _Reference20_VT155_SKNG on dbo._Reference20_VT155NG;

-- 11:33:34.742000 sid=81 sql_batch_completed rows=0 dur=2004us
drop index _Reference20_VT159_1NG on dbo._Reference20_VT159NG;

-- 11:33:34.746000 sid=81 sql_batch_completed rows=0 dur=1621us
drop index _Reference20_VT159_2NG on dbo._Reference20_VT159NG;

-- 11:33:34.749000 sid=81 sql_batch_completed rows=0 dur=2855us
drop index _Reference20_VT159_SKNG on dbo._Reference20_VT159NG;

-- 11:33:34.764000 sid=81 rpc_completed rows=14 dur=13871us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Reference20NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Code, _Description, _Fld151, _Fld152, _Fld153, _Fld154, _Fld949RRef, _Fld6357RRef, _Fld11262, _Fld2683) SELECT
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

-- 11:33:34.775000 sid=81 sql_batch_completed rows=29 dur=9667us
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

-- 11:33:34.784000 sid=81 sql_batch_completed rows=18 dur=8973us
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

-- 11:33:34.822000 sid=81 sql_batch_completed rows=14 dur=35004us
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.828000 sid=81 sql_batch_completed rows=14 dur=5248us
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.841000 sid=81 sql_batch_completed rows=14 dur=12002us
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.848000 sid=81 sql_batch_completed rows=14 dur=6169us
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.855000 sid=81 sql_batch_completed rows=14 dur=5259us
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.893000 sid=81 sql_batch_completed rows=84 dur=37364us
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:33:34.901000 sid=81 sql_batch_completed rows=29 dur=7369us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:33:34.911000 sid=81 sql_batch_completed rows=18 dur=9694us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:33:34.916000 sid=81 sql_batch_completed rows=18 dur=4180us
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:34.921000 sid=81 sql_batch_completed rows=18 dur=3583us
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:35.796000 sid=81 sql_batch_completed rows=0 dur=14724us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 11:33:35.801000 sid=81 sql_batch_completed rows=0 dur=3430us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 11:33:35.805000 sid=81 sql_batch_completed rows=0 dur=3834us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:35.812000 sid=81 sql_batch_completed rows=0 dur=4177us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:35.815000 sid=81 sql_batch_completed rows=0 dur=3167us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:33:35.816000 sid=81 rpc_completed rows=1 dur=389us
-- args: ,N'@P1 varbinary(max)',0x<6238 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 11:33:35.825000 sid=81 sql_batch_completed rows=0 dur=3255us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 11:33:35.827000 sid=81 sql_batch_completed rows=0 dur=1166us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 11:33:35.830000 sid=81 sql_batch_completed rows=0 dur=2153us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 11:33:36.073000 sid=81 sql_batch_completed rows=10000 dur=33430us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 11:33:36.099000 sid=81 sql_batch_completed rows=10000 dur=24509us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 11:33:36.127000 sid=81 sql_batch_completed rows=7 dur=23801us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 11:33:36.345000 sid=81 sql_batch_completed rows=10000 dur=33050us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 11:33:36.380000 sid=81 sql_batch_completed rows=10000 dur=33433us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 11:33:36.383000 sid=81 sql_batch_completed rows=469 dur=3730us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 11:33:36.464000 sid=81 sql_batch_completed rows=1219 dur=6140us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=1219);

-- 11:33:36.474000 sid=81 sql_batch_completed rows=1387 dur=7371us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=1387);

-- 11:33:36.504000 sid=81 sql_batch_completed rows=21219 dur=28345us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:36.536000 sid=81 sql_batch_completed rows=21219 dur=30790us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 11:33:36.560000 sid=81 sql_batch_completed rows=21863 dur=24425us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:33:41.562000 sid=81 rpc_completed rows=1 dur=2750us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 11:33:41.608000 sid=81 rpc_completed rows=1 dur=2931us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 11:33:47.127000 sid=81 rpc_completed rows=1 dur=188us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<1370 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 11:33:47.179000 sid=81 rpc_completed rows=1 dur=322us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF981D0D001A4F1118F5100E04C680093,0x<12096 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 11:37:22.194000 sid=81 sql_batch_completed rows=1 dur=7007us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 11:37:22.367000 sid=81 sql_batch_completed rows=0 dur=75536us
drop table dbo._Reference20;

-- 11:37:22.435000 sid=81 sql_batch_completed rows=0 dur=45101us
drop table dbo._Reference20_VT155;

-- 11:37:22.533000 sid=81 sql_batch_completed rows=0 dur=73250us
drop table dbo._Reference20_VT159;

-- 11:37:22.595000 sid=81 sql_batch_completed rows=0 dur=38122us
drop table dbo._ConfigChngR;

-- 11:37:22.624000 sid=81 sql_batch_completed rows=0 dur=16333us
drop table dbo._ConfigChngR_ExtProps;

-- 11:37:22.633000 sid=81 sql_batch_completed rows=1 dur=861us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 11:37:26.618000 sid=81 rpc_completed rows=31 dur=3927184us
-- args: N'_Reference20NG',N'_Reference20'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 11:37:26.645000 sid=81 rpc_completed rows=25 dur=12473us
-- args: N'_Reference20_VT155NG',N'_Reference20_VT155'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 11:37:26.666000 sid=81 rpc_completed rows=25 dur=8684us
-- args: N'_Reference20_VT159NG',N'_Reference20_VT159'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 11:37:26.699000 sid=81 rpc_completed rows=16 dur=9068us
-- args: N'_Reference20._Reference20_1NG',N'_Reference20_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:26.723000 sid=81 rpc_completed rows=16 dur=4932us
-- args: N'_Reference20._Reference20_2NG',N'_Reference20_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:26.756000 sid=81 rpc_completed rows=16 dur=19205us
-- args: N'_Reference20._Reference20_3NG',N'_Reference20_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:26.768000 sid=81 rpc_completed rows=16 dur=6701us
-- args: N'_Reference20._Reference20_4NG',N'_Reference20_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:26.843000 sid=81 rpc_completed rows=16 dur=8099us
-- args: N'_Reference20._Reference20_5NG',N'_Reference20_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:26.935000 sid=81 rpc_completed rows=16 dur=10025us
-- args: N'_Reference20._Reference20_S_HPKNG',N'_Reference20_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:26.993000 sid=81 rpc_completed rows=16 dur=2640us
-- args: N'_Reference20_VT155._Reference20_VT155_SKNG',N'_Reference20_VT155_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:27.004000 sid=81 rpc_completed rows=16 dur=7669us
-- args: N'_Reference20_VT159._Reference20_VT159_SKNG',N'_Reference20_VT159_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:27.032000 sid=81 rpc_completed rows=16 dur=10909us
-- args: N'_Reference20_VT159._Reference20_VT159_1NG',N'_Reference20_VT159_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:27.071000 sid=81 rpc_completed rows=16 dur=12814us
-- args: N'_Reference20_VT159._Reference20_VT159_2NG',N'_Reference20_VT159_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:27.138000 sid=81 rpc_completed rows=25 dur=44026us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 11:37:27.170000 sid=81 rpc_completed rows=25 dur=10042us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 11:37:27.178000 sid=81 rpc_completed rows=16 dur=5292us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:27.190000 sid=81 rpc_completed rows=16 dur=6045us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:27.212000 sid=81 rpc_completed rows=16 dur=3893us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 11:37:27.763000 sid=81 sql_batch_completed rows=0 dur=6453us
ALTER INDEX PK___Enum427__AC8ED0C4E471CE32 ON _Enum4271 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:37:27.768000 sid=81 sql_batch_completed rows=0 dur=4762us
ALTER INDEX PK___STTMode__AC8ED0C49C44B251 ON _STTModels SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:37:27.773000 sid=81 sql_batch_completed rows=0 dur=4839us
ALTER INDEX PK___Enum100__AC8ED0C45E0342FD ON _Enum1008 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 11:37:29.233000 sid=81 rpc_completed rows=1 dur=29986us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1919130 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 11:37:29.301000 sid=81 rpc_completed rows=1 dur=29479us
-- args: ,N'@P1 varbinary(max)',0x<1919130 hex>
UPDATE DBSchema SET SerializedData = @P1;


-- (237 more ALTER INDEX statements of the same form cut)
