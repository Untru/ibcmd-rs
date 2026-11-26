-- 01:01:09.644000 sid=99 sql_batch_completed rows=0 dur=9594us
create table dbo._Reference21NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_ParentIDRRef binary(16) not null,
_Code nvarchar(9) not null,
_Description nvarchar(150) not null,
_Fld959RRef binary(16) not null,
_Fld11034 nvarchar(20) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference21NG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:09.659000 sid=99 sql_batch_completed rows=0 dur=13619us
create table dbo._Reference21_VT960NG (
_Reference21_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo961 numeric(5, 0) not null,
_Fld962RRef binary(16) not null,
_Fld11035 nvarchar(12) not null
)
;alter table dbo._Reference21_VT960NG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:09.667000 sid=99 sql_batch_completed rows=0 dur=5512us
create table dbo._Reference21_VT11036NG (
_Reference21_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11037 numeric(5, 0) not null,
_Fld11038 binary(1) not null
)
;alter table dbo._Reference21_VT11036NG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:09.738000 sid=99 sql_batch_completed rows=0 dur=17588us
CREATE INDEX _Reference21_1NG ON dbo._Reference21NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.754000 sid=99 sql_batch_completed rows=0 dur=13877us
CREATE UNIQUE INDEX _Reference21_2NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.759000 sid=99 sql_batch_completed rows=0 dur=4023us
CREATE UNIQUE INDEX _Reference21_3NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.764000 sid=99 sql_batch_completed rows=0 dur=2970us
CREATE UNIQUE INDEX _Reference21_4NG ON dbo._Reference21NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.771000 sid=99 sql_batch_completed rows=0 dur=6080us
CREATE UNIQUE INDEX _Reference21_5NG ON dbo._Reference21NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.784000 sid=99 sql_batch_completed rows=0 dur=12445us
CREATE UNIQUE CLUSTERED INDEX _Reference21_S_HPKNG ON dbo._Reference21NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:09.794000 sid=99 sql_batch_completed rows=0 dur=10057us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT960_SKNG ON dbo._Reference21_VT960NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:09.804000 sid=99 sql_batch_completed rows=0 dur=9016us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT11036_SKNG ON dbo._Reference21_VT11036NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:09.810000 sid=99 rpc_completed rows=1 dur=2572us
-- args: ,N'@P1 varbinary(max)',0x<2764 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 01:01:09.821000 sid=99 sql_batch_completed rows=0 dur=2133us
drop index _Reference21_1NG on dbo._Reference21NG;

-- 01:01:09.824000 sid=99 sql_batch_completed rows=0 dur=2104us
drop index _Reference21_2NG on dbo._Reference21NG;

-- 01:01:09.828000 sid=99 sql_batch_completed rows=0 dur=3757us
drop index _Reference21_3NG on dbo._Reference21NG;

-- 01:01:09.831000 sid=99 sql_batch_completed rows=0 dur=1936us
drop index _Reference21_4NG on dbo._Reference21NG;

-- 01:01:09.837000 sid=99 sql_batch_completed rows=0 dur=4339us
drop index _Reference21_5NG on dbo._Reference21NG;

-- 01:01:09.844000 sid=99 sql_batch_completed rows=0 dur=6560us
drop index _Reference21_S_HPKNG on dbo._Reference21NG;

-- 01:01:09.851000 sid=99 sql_batch_completed rows=0 dur=4914us
drop index _Reference21_VT960_SKNG on dbo._Reference21_VT960NG;

-- 01:01:09.858000 sid=99 sql_batch_completed rows=0 dur=5028us
drop index _Reference21_VT11036_SKNG on dbo._Reference21_VT11036NG;

-- 01:01:09.877000 sid=99 rpc_completed rows=4 dur=17678us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Reference21NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Code, _Description, _Fld959RRef, _Fld11034, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._ParentIDRRef,
T1._Code,
T1._Description,
T1._Fld959RRef,
CAST(@P1 AS NVARCHAR(20)),
T1._Fld2683
FROM dbo._Reference21 T1 WITH(NOLOCK);

-- 01:01:09.885000 sid=99 rpc_completed rows=4 dur=6920us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Reference21_VT960NG WITH(TABLOCK) (_LineNo961, _Fld962RRef, _Fld11035, _Fld2683, _Reference21_IDRRef, _KeyField) SELECT
T2._LineNo961,
T2._Fld962RRef,
CAST(@P1 AS NVARCHAR(12)),
T2._Fld2683,
T2._Reference21_IDRRef,
T2._KeyField
FROM dbo._Reference21_VT960 T2 WITH(NOLOCK);

-- 01:01:09.902000 sid=99 sql_batch_completed rows=4 dur=13550us
CREATE INDEX _Reference21_1NG ON dbo._Reference21NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.911000 sid=99 sql_batch_completed rows=4 dur=8326us
CREATE UNIQUE INDEX _Reference21_2NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.918000 sid=99 sql_batch_completed rows=4 dur=6293us
CREATE UNIQUE INDEX _Reference21_3NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.924000 sid=99 sql_batch_completed rows=4 dur=4478us
CREATE UNIQUE INDEX _Reference21_4NG ON dbo._Reference21NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.934000 sid=99 sql_batch_completed rows=4 dur=8563us
CREATE UNIQUE INDEX _Reference21_5NG ON dbo._Reference21NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:09.974000 sid=99 sql_batch_completed rows=24 dur=38124us
CREATE UNIQUE CLUSTERED INDEX _Reference21_S_HPKNG ON dbo._Reference21NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:09.978000 sid=99 sql_batch_completed rows=4 dur=4219us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT960_SKNG ON dbo._Reference21_VT960NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:09.981000 sid=99 sql_batch_completed rows=0 dur=2710us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT11036_SKNG ON dbo._Reference21_VT11036NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.493000 sid=99 sql_batch_completed rows=0 dur=34365us
create table dbo._Document8409NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_Date_Time datetime2(0) not null,
_NumberPrefix datetime2(0) not null,
_Number nvarchar(11) not null,
_Posted binary(1) not null,
_Fld8410RRef binary(16) not null,
_Fld8411 nvarchar(100) not null,
_Fld8412RRef binary(16) not null,
_Fld8413 nvarchar(max) not null,
_Fld8414RRef binary(16) not null,
_Fld11039 nvarchar(20) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Document8409NG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:10.510000 sid=99 sql_batch_completed rows=0 dur=14454us
create table dbo._Document8409_VT8415NG (
_Document8409_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo8416 numeric(5, 0) not null,
_Fld8417RRef binary(16) not null,
_Fld8418 datetime2(0) not null,
_Fld8419 datetime2(0) not null,
_Fld8420 numeric(3, 0) not null,
_Fld8421 binary(1) not null,
_Fld11040 nvarchar(5) not null
)
;alter table dbo._Document8409_VT8415NG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:10.544000 sid=99 sql_batch_completed rows=0 dur=16857us
create table dbo._Document8409_VT11041NG (
_Document8409_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11042 numeric(5, 0) not null,
_Fld11043 nvarchar(10) not null,
_Fld11044 numeric(10, 0) not null
)
;alter table dbo._Document8409_VT11041NG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:10.552000 sid=99 sql_batch_completed rows=0 dur=4562us
create table dbo._Document8409_VT11045NG (
_Document8409_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11046 numeric(5, 0) not null,
_Fld11047 datetime2(0) not null
)
;alter table dbo._Document8409_VT11045NG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:10.556000 sid=99 sql_batch_completed rows=0 dur=1964us
CREATE UNIQUE INDEX _Document8409_1NG ON dbo._Document8409NG (_Fld2683, _NumberPrefix, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:10.558000 sid=99 sql_batch_completed rows=0 dur=1182us
CREATE UNIQUE INDEX _Document8409_2NG ON dbo._Document8409NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:10.564000 sid=99 sql_batch_completed rows=0 dur=5705us
CREATE UNIQUE INDEX _Document8409_3NG ON dbo._Document8409NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:10.573000 sid=99 sql_batch_completed rows=0 dur=7136us
CREATE UNIQUE INDEX _Document8409_4NG ON dbo._Document8409NG (_Fld2683, _Fld8410RRef, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:10.576000 sid=99 sql_batch_completed rows=0 dur=2859us
CREATE UNIQUE CLUSTERED INDEX _Document8409_S_HPKNG ON dbo._Document8409NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.584000 sid=99 sql_batch_completed rows=0 dur=7936us
CREATE UNIQUE CLUSTERED INDEX _Document8409_VT8415_SKNG ON dbo._Document8409_VT8415NG (_Fld2683, _Document8409_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.588000 sid=99 sql_batch_completed rows=0 dur=3731us
CREATE UNIQUE CLUSTERED INDEX _Document8409_VT11041_SKNG ON dbo._Document8409_VT11041NG (_Fld2683, _Document8409_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.592000 sid=99 sql_batch_completed rows=0 dur=3104us
CREATE UNIQUE CLUSTERED INDEX _Document8409_VT11045_SKNG ON dbo._Document8409_VT11045NG (_Fld2683, _Document8409_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.592000 sid=99 rpc_completed rows=1 dur=251us
-- args: ,N'@P1 varbinary(max)',0x<6650 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 01:01:10.602000 sid=99 sql_batch_completed rows=0 dur=4876us
drop index _Document8409_1NG on dbo._Document8409NG;

-- 01:01:10.605000 sid=99 sql_batch_completed rows=0 dur=1943us
drop index _Document8409_2NG on dbo._Document8409NG;

-- 01:01:10.608000 sid=99 sql_batch_completed rows=0 dur=2834us
drop index _Document8409_3NG on dbo._Document8409NG;

-- 01:01:10.611000 sid=99 sql_batch_completed rows=0 dur=2820us
drop index _Document8409_4NG on dbo._Document8409NG;

-- 01:01:10.616000 sid=99 sql_batch_completed rows=0 dur=4343us
drop index _Document8409_S_HPKNG on dbo._Document8409NG;

-- 01:01:10.619000 sid=99 sql_batch_completed rows=0 dur=2188us
drop index _Document8409_VT8415_SKNG on dbo._Document8409_VT8415NG;

-- 01:01:10.622000 sid=99 sql_batch_completed rows=0 dur=1798us
drop index _Document8409_VT11041_SKNG on dbo._Document8409_VT11041NG;

-- 01:01:10.625000 sid=99 sql_batch_completed rows=0 dur=2267us
drop index _Document8409_VT11045_SKNG on dbo._Document8409_VT11045NG;

-- 01:01:10.638000 sid=99 rpc_completed rows=3 dur=12446us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Document8409NG WITH(TABLOCK) (_IDRRef, _Marked, _Date_Time, _NumberPrefix, _Number, _Posted, _Fld8410RRef, _Fld8411, _Fld8412RRef, _Fld8413, _Fld8414RRef, _Fld11039, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._Date_Time,
T1._NumberPrefix,
T1._Number,
T1._Posted,
T1._Fld8410RRef,
T1._Fld8411,
T1._Fld8412RRef,
T1._Fld8413,
T1._Fld8414RRef,
CAST(@P1 AS NVARCHAR(20)),
T1._Fld2683
FROM dbo._Document8409 T1 WITH(NOLOCK);

-- 01:01:10.654000 sid=99 rpc_completed rows=5 dur=14914us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Document8409_VT8415NG WITH(TABLOCK) (_LineNo8416, _Fld8417RRef, _Fld8418, _Fld8419, _Fld8420, _Fld8421, _Fld11040, _Fld2683, _Document8409_IDRRef, _KeyField) SELECT
T2._LineNo8416,
T2._Fld8417RRef,
T2._Fld8418,
T2._Fld8419,
T2._Fld8420,
T2._Fld8421,
CAST(@P1 AS NVARCHAR(5)),
T2._Fld2683,
T2._Document8409_IDRRef,
T2._KeyField
FROM dbo._Document8409_VT8415 T2 WITH(NOLOCK);

-- 01:01:10.660000 sid=99 sql_batch_completed rows=3 dur=4922us
CREATE UNIQUE INDEX _Document8409_1NG ON dbo._Document8409NG (_Fld2683, _NumberPrefix, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:10.664000 sid=99 sql_batch_completed rows=3 dur=3114us
CREATE UNIQUE INDEX _Document8409_2NG ON dbo._Document8409NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:10.666000 sid=99 sql_batch_completed rows=3 dur=1661us
CREATE UNIQUE INDEX _Document8409_3NG ON dbo._Document8409NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:10.670000 sid=99 sql_batch_completed rows=3 dur=3155us
CREATE UNIQUE INDEX _Document8409_4NG ON dbo._Document8409NG (_Fld2683, _Fld8410RRef, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:10.684000 sid=99 sql_batch_completed rows=15 dur=14224us
CREATE UNIQUE CLUSTERED INDEX _Document8409_S_HPKNG ON dbo._Document8409NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.688000 sid=99 sql_batch_completed rows=5 dur=3272us
CREATE UNIQUE CLUSTERED INDEX _Document8409_VT8415_SKNG ON dbo._Document8409_VT8415NG (_Fld2683, _Document8409_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.689000 sid=99 sql_batch_completed rows=0 dur=1373us
CREATE UNIQUE CLUSTERED INDEX _Document8409_VT11041_SKNG ON dbo._Document8409_VT11041NG (_Fld2683, _Document8409_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.691000 sid=99 sql_batch_completed rows=0 dur=1287us
CREATE UNIQUE CLUSTERED INDEX _Document8409_VT11045_SKNG ON dbo._Document8409_VT11045NG (_Fld2683, _Document8409_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.725000 sid=99 sql_batch_completed rows=0 dur=4444us
create table dbo._DbCopiesUpdatesNG (
_CopyId binary(16) not null,
_TrNum int,
_TrTime datetime2(0),
_UpdateId binary(16),
_LastUpdateResult numeric(2, 0),
_LastUpdateError varbinary(max)
)
;alter table dbo._DbCopiesUpdatesNG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:10.728000 sid=99 sql_batch_completed rows=0 dur=2823us
CREATE UNIQUE CLUSTERED INDEX _DbCopiesUpdates_1NG ON dbo._DbCopiesUpdatesNG (_CopyId, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.756000 sid=99 rpc_completed rows=1 dur=27344us
-- args: ,N'@P1 varbinary(max)',0x<7538 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 01:01:10.763000 sid=99 sql_batch_completed rows=0 dur=4570us
INSERT INTO dbo._DbCopiesUpdatesNG WITH(TABLOCK) (_CopyId, _TrNum, _TrTime) SELECT
T1._CopyId,
(T1._TrNum + 0.0),
T1._TrTime
FROM dbo._DbCopiesUpdates T1 WITH(NOLOCK);

-- 01:01:10.773000 sid=99 sql_batch_completed rows=0 dur=7683us
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

-- 01:01:10.775000 sid=99 sql_batch_completed rows=0 dur=2146us
CREATE UNIQUE CLUSTERED INDEX _DbCopies_1NG ON dbo._DbCopiesNG (_CopyId, _CopyName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:10.776000 sid=99 rpc_completed rows=1 dur=419us
-- args: ,N'@P1 varbinary(max)',0x<9066 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 01:01:10.780000 sid=99 rpc_completed rows=0 dur=1391us
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

-- 01:01:11.097000 sid=99 sql_batch_completed rows=0 dur=9816us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:11.101000 sid=99 sql_batch_completed rows=0 dur=3002us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 01:01:11.103000 sid=99 sql_batch_completed rows=0 dur=1331us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:11.104000 sid=99 sql_batch_completed rows=0 dur=771us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:11.108000 sid=99 sql_batch_completed rows=0 dur=3077us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:11.110000 sid=99 rpc_completed rows=1 dur=1615us
-- args: ,N'@P1 varbinary(max)',0x<10130 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 01:01:11.119000 sid=99 sql_batch_completed rows=0 dur=5535us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 01:01:11.121000 sid=99 sql_batch_completed rows=0 dur=1085us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 01:01:11.123000 sid=99 sql_batch_completed rows=0 dur=1026us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 01:01:11.306000 sid=99 sql_batch_completed rows=10000 dur=29589us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 01:01:11.344000 sid=99 sql_batch_completed rows=9996 dur=36102us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=9996);

-- 01:01:11.552000 sid=99 sql_batch_completed rows=10000 dur=55193us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 01:01:11.589000 sid=99 sql_batch_completed rows=10000 dur=36302us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 01:01:11.619000 sid=99 sql_batch_completed rows=658 dur=28951us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 01:01:11.727000 sid=99 sql_batch_completed rows=685 dur=25572us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=685);

-- 01:01:11.773000 sid=99 sql_batch_completed rows=703 dur=43767us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=703);

-- 01:01:11.853000 sid=99 sql_batch_completed rows=20685 dur=79246us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:11.962000 sid=99 sql_batch_completed rows=20685 dur=107563us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 01:01:12.030000 sid=99 sql_batch_completed rows=21357 dur=67480us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:01:13.797000 sid=99 rpc_completed rows=1 dur=69934us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 01:01:13.895000 sid=99 rpc_completed rows=1 dur=12346us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 01:01:17.782000 sid=99 rpc_completed rows=1 dur=222us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<888 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 01:01:17.814000 sid=99 rpc_completed rows=1 dur=171us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF00EFAB36899F1118F4F00E04C680093,0x<7728 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 01:03:01.755000 sid=99 sql_batch_completed rows=1 dur=12344us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 01:03:01.843000 sid=99 sql_batch_completed rows=0 dur=68604us
drop table dbo._Reference21;

-- 01:03:01.859000 sid=99 sql_batch_completed rows=0 dur=7655us
drop table dbo._Reference21_VT960;

-- 01:03:01.921000 sid=99 sql_batch_completed rows=0 dur=45307us
drop table dbo._Document8409;

-- 01:03:01.940000 sid=99 sql_batch_completed rows=0 dur=7153us
drop table dbo._Document8409_VT8415;

-- 01:03:01.990000 sid=99 sql_batch_completed rows=0 dur=28645us
drop table dbo._DbCopiesUpdates;

-- 01:03:02.018000 sid=99 sql_batch_completed rows=0 dur=10249us
drop table dbo._DbCopies;

-- 01:03:02.086000 sid=99 sql_batch_completed rows=0 dur=45856us
drop table dbo._ConfigChngR;

-- 01:03:02.111000 sid=99 sql_batch_completed rows=0 dur=16528us
drop table dbo._ConfigChngR_ExtProps;

-- 01:03:02.116000 sid=99 sql_batch_completed rows=1 dur=4109us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 01:03:02.626000 sid=99 rpc_completed rows=29 dur=492534us
-- args: N'_Reference21NG',N'_Reference21'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.645000 sid=99 rpc_completed rows=25 dur=15638us
-- args: N'_Reference21_VT960NG',N'_Reference21_VT960'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.659000 sid=99 rpc_completed rows=25 dur=7034us
-- args: N'_Reference21_VT11036NG',N'_Reference21_VT11036'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.669000 sid=99 rpc_completed rows=16 dur=2779us
-- args: N'_Reference21._Reference21_1NG',N'_Reference21_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.678000 sid=99 rpc_completed rows=16 dur=1946us
-- args: N'_Reference21._Reference21_2NG',N'_Reference21_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.687000 sid=99 rpc_completed rows=16 dur=1754us
-- args: N'_Reference21._Reference21_3NG',N'_Reference21_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.699000 sid=99 rpc_completed rows=16 dur=6337us
-- args: N'_Reference21._Reference21_4NG',N'_Reference21_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.704000 sid=99 rpc_completed rows=16 dur=1347us
-- args: N'_Reference21._Reference21_5NG',N'_Reference21_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.713000 sid=99 rpc_completed rows=16 dur=4755us
-- args: N'_Reference21._Reference21_S_HPKNG',N'_Reference21_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.718000 sid=99 rpc_completed rows=16 dur=2228us
-- args: N'_Reference21_VT960._Reference21_VT960_SKNG',N'_Reference21_VT960_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.723000 sid=99 rpc_completed rows=16 dur=2724us
-- args: N'_Reference21_VT11036._Reference21_VT11036_SKNG',N'_Reference21_VT11036_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.731000 sid=99 rpc_completed rows=25 dur=6059us
-- args: N'_Document8409NG',N'_Document8409'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.736000 sid=99 rpc_completed rows=25 dur=3296us
-- args: N'_Document8409_VT8415NG',N'_Document8409_VT8415'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.747000 sid=99 rpc_completed rows=25 dur=8827us
-- args: N'_Document8409_VT11041NG',N'_Document8409_VT11041'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.755000 sid=99 rpc_completed rows=25 dur=6595us
-- args: N'_Document8409_VT11045NG',N'_Document8409_VT11045'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.761000 sid=99 rpc_completed rows=16 dur=2131us
-- args: N'_Document8409._Document8409_1NG',N'_Document8409_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.765000 sid=99 rpc_completed rows=16 dur=1833us
-- args: N'_Document8409._Document8409_2NG',N'_Document8409_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.768000 sid=99 rpc_completed rows=16 dur=1061us
-- args: N'_Document8409._Document8409_3NG',N'_Document8409_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.771000 sid=99 rpc_completed rows=16 dur=913us
-- args: N'_Document8409._Document8409_4NG',N'_Document8409_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.776000 sid=99 rpc_completed rows=16 dur=2264us
-- args: N'_Document8409._Document8409_S_HPKNG',N'_Document8409_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.779000 sid=99 rpc_completed rows=16 dur=1592us
-- args: N'_Document8409_VT8415._Document8409_VT8415_SKNG',N'_Document8409_VT8415_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.783000 sid=99 rpc_completed rows=16 dur=2007us
-- args: N'_Document8409_VT11041._Document8409_VT11041_SKNG',N'_Document8409_VT11041_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.789000 sid=99 rpc_completed rows=16 dur=4484us
-- args: N'_Document8409_VT11045._Document8409_VT11045_SKNG',N'_Document8409_VT11045_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.801000 sid=99 rpc_completed rows=25 dur=9961us
-- args: N'_DbCopiesUpdatesNG',N'_DbCopiesUpdates'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.809000 sid=99 rpc_completed rows=16 dur=5597us
-- args: N'_DbCopiesUpdates._DbCopiesUpdates_1NG',N'_DbCopiesUpdates_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.825000 sid=99 rpc_completed rows=25 dur=14506us
-- args: N'_DbCopiesNG',N'_DbCopies'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.829000 sid=99 rpc_completed rows=16 dur=1935us
-- args: N'_DbCopies._DbCopies_1NG',N'_DbCopies_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.847000 sid=99 rpc_completed rows=25 dur=15962us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.857000 sid=99 rpc_completed rows=25 dur=8225us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:03:02.860000 sid=99 rpc_completed rows=16 dur=1100us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.862000 sid=99 rpc_completed rows=16 dur=1045us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:02.867000 sid=99 rpc_completed rows=16 dur=2641us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:03:03.278000 sid=99 sql_batch_completed rows=0 dur=33184us
ALTER INDEX PK___Referen__AC8ED0C4077A8F3B ON _Reference10877X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.302000 sid=99 sql_batch_completed rows=0 dur=24482us
ALTER INDEX PK___Referen__AC8ED0C4DBBB6986 ON _Reference10878X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.324000 sid=99 sql_batch_completed rows=0 dur=21425us
ALTER INDEX PK___Referen__AC8ED0C4720747AF ON _Reference10825X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.349000 sid=99 sql_batch_completed rows=0 dur=24379us
ALTER INDEX PK___Enum108__AC8ED0C45E875D60 ON _Enum10826X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.366000 sid=99 sql_batch_completed rows=0 dur=17091us
ALTER INDEX PK___Enum110__AC8ED0C4A5E2EE79 ON _Enum11017X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.393000 sid=99 sql_batch_completed rows=0 dur=26060us
ALTER INDEX PK___Documen__AC8ED0C48EB57390 ON _Document10069X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.436000 sid=99 sql_batch_completed rows=0 dur=42635us
ALTER INDEX PK___Referen__AC8ED0C43F77AD82 ON _Reference2596X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.455000 sid=99 sql_batch_completed rows=0 dur=17121us
ALTER INDEX PK___Referen__AC8ED0C45FCE1C7A ON _Reference10523X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.459000 sid=99 sql_batch_completed rows=0 dur=3871us
ALTER INDEX PK___ConfigC__AC8ED0C4A9F51E3F ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:03:03.506000 sid=99 rpc_completed rows=1 dur=30039us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1957014 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 01:03:03.562000 sid=99 rpc_completed rows=1 dur=25699us
-- args: ,N'@P1 varbinary(max)',0x<1957014 hex>
UPDATE DBSchema SET SerializedData = @P1;

