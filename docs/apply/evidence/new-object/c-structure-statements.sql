-- 07:44:59.950000 sid=58 sql_batch_completed rows=0 dur=11103us
create table dbo._Reference11036NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_Code nvarchar(9) not null,
_Description nvarchar(25) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference11036NG SET (LOCK_ESCALATION = DISABLE);

-- 07:44:59.955000 sid=58 sql_batch_completed rows=0 dur=3092us
create table dbo._DbCopiesInfoBaseUseNG (
_Id binary(16) not null,
_Description nvarchar(256) not null
)
;alter table dbo._DbCopiesInfoBaseUseNG SET (LOCK_ESCALATION = DISABLE);

-- 07:44:59.959000 sid=58 sql_batch_completed rows=0 dur=2666us
create table dbo._DbCopiesUpdateTableStatNG (
_CopyId binary(16) not null,
_TableName nvarchar(256) not null,
_UpdateTime datetime2(0) not null,
_TransferTime numeric(10, 0) not null,
_IsPortion binary(1) not null
)
;alter table dbo._DbCopiesUpdateTableStatNG SET (LOCK_ESCALATION = DISABLE);

-- 07:44:59.964000 sid=58 sql_batch_completed rows=0 dur=4500us
create table dbo._DbCopiesUpdateStatNG (
_CopyId binary(16) not null,
_UpdateTime datetime2(0) not null,
_TranPerSec numeric(16, 4) not null
)
;alter table dbo._DbCopiesUpdateStatNG SET (LOCK_ESCALATION = DISABLE);

-- 07:44:59.968000 sid=58 sql_batch_completed rows=0 dur=3163us
create table dbo._WebSocketClientsNG (
_ID binary(16) not null,
_WSCKey nvarchar(100) not null,
_MetadataID binary(16) not null,
_ServerURL nvarchar(255) not null,
_Predefined binary(1) not null,
_ConnectionParameters varbinary(max) not null,
_IBUserName nvarchar(100),
_AutoConnect binary(1) not null
)
;alter table dbo._WebSocketClientsNG SET (LOCK_ESCALATION = DISABLE);

-- 07:45:00.078000 sid=58 sql_batch_completed rows=0 dur=58500us
CREATE INDEX _Reference11036_1NG ON dbo._Reference11036NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:00.081000 sid=58 sql_batch_completed rows=0 dur=1172us
CREATE UNIQUE INDEX _Reference11036_2NG ON dbo._Reference11036NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:00.083000 sid=58 sql_batch_completed rows=0 dur=1181us
CREATE UNIQUE INDEX _Reference11036_3NG ON dbo._Reference11036NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:00.088000 sid=58 sql_batch_completed rows=0 dur=4559us
CREATE UNIQUE CLUSTERED INDEX _Reference11036_S_HPKNG ON dbo._Reference11036NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 07:45:00.094000 sid=58 rpc_completed rows=1 dur=2029us
-- args: ,N'@P1 varbinary(max)',0x<3862 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 07:45:03.223000 sid=58 sql_batch_completed rows=0 dur=91899us
create table dbo._BPrPoints10NG (
_IDRRef binary(16) not null primary key,
_RoutePointOrder numeric(10, 0) not null
)
;alter table dbo._BPrPoints10NG SET (LOCK_ESCALATION = DISABLE);

-- 07:45:03.232000 sid=58 sql_batch_completed rows=0 dur=6581us
CREATE UNIQUE INDEX _BPrPoints10_1NG ON dbo._BPrPoints10NG (_RoutePointOrder, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:03.233000 sid=58 rpc_completed rows=1 dur=1002us
-- args: ,N'@P1 varbinary(max)',0x<4348 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 07:45:03.257000 sid=58 sql_batch_completed rows=0 dur=3681us
drop index _BPrPoints10_1NG on dbo._BPrPoints10NG;

-- 07:45:03.266000 sid=58 sql_batch_completed rows=6 dur=5517us
insert bulk dbo._BPrPoints10NG([_IDRRef] binary(16),[_RoutePointOrder] numeric(10,0))with(TABLOCK, ROWS_PER_BATCH=6);

-- 07:45:03.362000 sid=58 sql_batch_completed rows=6 dur=88733us
CREATE UNIQUE INDEX _BPrPoints10_1NG ON dbo._BPrPoints10NG (_RoutePointOrder, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:03.780000 sid=58 sql_batch_completed rows=0 dur=82659us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 07:45:03.971000 sid=58 sql_batch_completed rows=0 dur=186782us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 07:45:04.056000 sid=58 sql_batch_completed rows=0 dur=83241us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:04.143000 sid=58 sql_batch_completed rows=0 dur=83873us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:04.240000 sid=58 sql_batch_completed rows=0 dur=94826us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 07:45:04.243000 sid=58 rpc_completed rows=1 dur=940us
-- args: ,N'@P1 varbinary(max)',0x<5412 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 07:45:04.251000 sid=58 sql_batch_completed rows=0 dur=3237us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 07:45:04.258000 sid=58 sql_batch_completed rows=0 dur=3521us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 07:45:04.264000 sid=58 sql_batch_completed rows=0 dur=3025us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 07:45:04.550000 sid=58 sql_batch_completed rows=10000 dur=28737us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 07:45:04.580000 sid=58 sql_batch_completed rows=9995 dur=28161us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=9995);

-- 07:45:05.021000 sid=58 sql_batch_completed rows=10000 dur=148338us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 07:45:05.040000 sid=58 sql_batch_completed rows=10000 dur=17876us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 07:45:05.051000 sid=58 sql_batch_completed rows=657 dur=11093us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 07:45:05.332000 sid=58 sql_batch_completed rows=688 dur=52545us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=688);

-- 07:45:05.430000 sid=58 sql_batch_completed rows=705 dur=71251us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=705);

-- 07:45:05.528000 sid=58 sql_batch_completed rows=20688 dur=83426us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:05.725000 sid=58 sql_batch_completed rows=20688 dur=194351us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 07:45:05.887000 sid=58 sql_batch_completed rows=21357 dur=160065us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 07:45:12.344000 sid=58 rpc_completed rows=1 dur=2302us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 07:45:12.405000 sid=58 rpc_completed rows=1 dur=1098us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 07:45:14.795000 sid=58 rpc_completed rows=1 dur=1054us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<888 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 07:45:14.891000 sid=58 rpc_completed rows=1 dur=383us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF00EFAB36899F1118F4F00E04C680093,0x<7728 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 07:45:21.056000 sid=58 sql_batch_completed rows=1 dur=12035us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 07:45:21.090000 sid=58 sql_batch_completed rows=0 dur=24675us
drop table dbo._BPrPoints10;

-- 07:45:21.112000 sid=58 sql_batch_completed rows=0 dur=20052us
drop table dbo._ConfigChngR;

-- 07:45:21.121000 sid=58 sql_batch_completed rows=0 dur=7290us
drop table dbo._ConfigChngR_ExtProps;

-- 07:45:21.126000 sid=58 sql_batch_completed rows=1 dur=3334us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 07:45:21.830000 sid=58 rpc_completed rows=30 dur=699349us
-- args: N'_Reference11036NG',N'_Reference11036'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:45:21.838000 sid=58 rpc_completed rows=16 dur=5987us
-- args: N'_Reference11036._Reference11036_1NG',N'_Reference11036_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 07:45:21.842000 sid=58 rpc_completed rows=16 dur=1675us
-- args: N'_Reference11036._Reference11036_2NG',N'_Reference11036_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 07:45:21.845000 sid=58 rpc_completed rows=16 dur=1118us
-- args: N'_Reference11036._Reference11036_3NG',N'_Reference11036_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 07:45:21.855000 sid=58 rpc_completed rows=16 dur=7505us
-- args: N'_Reference11036._Reference11036_S_HPKNG',N'_Reference11036_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 07:45:21.868000 sid=58 rpc_completed rows=25 dur=10835us
-- args: N'_DbCopiesInfoBaseUseNG',N'_DbCopiesInfoBaseUse'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:45:21.876000 sid=58 rpc_completed rows=25 dur=4560us
-- args: N'_DbCopiesUpdateTableStatNG',N'_DbCopiesUpdateTableStat'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:45:21.883000 sid=58 rpc_completed rows=25 dur=3372us
-- args: N'_DbCopiesUpdateStatNG',N'_DbCopiesUpdateStat'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:45:21.889000 sid=58 rpc_completed rows=25 dur=3577us
-- args: N'_WebSocketClientsNG',N'_WebSocketClients'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:45:21.897000 sid=58 rpc_completed rows=25 dur=5833us
-- args: N'_BPrPoints10NG',N'_BPrPoints10'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:45:21.903000 sid=58 rpc_completed rows=16 dur=1867us
-- args: N'_BPrPoints10._BPrPoints10_1NG',N'_BPrPoints10_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 07:45:21.911000 sid=58 rpc_completed rows=25 dur=4680us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:45:21.918000 sid=58 rpc_completed rows=25 dur=5043us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:45:21.928000 sid=58 rpc_completed rows=16 dur=6258us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 07:45:21.933000 sid=58 rpc_completed rows=16 dur=2013us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 07:45:21.940000 sid=58 rpc_completed rows=16 dur=3431us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 07:45:22.196000 sid=58 sql_batch_completed rows=0 dur=2847us
ALTER INDEX PK___BPrPoin__AC8ED0C454253366 ON _BPrPoints10 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 07:45:22.198000 sid=58 sql_batch_completed rows=0 dur=1212us
ALTER INDEX PK___ConfigC__AC8ED0C49974AEC5 ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 07:45:22.259000 sid=58 rpc_completed rows=1 dur=26181us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1959428 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 07:45:22.309000 sid=58 rpc_completed rows=1 dur=23107us
-- args: ,N'@P1 varbinary(max)',0x<1959428 hex>
UPDATE DBSchema SET SerializedData = @P1;

