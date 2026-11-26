-- 00:58:49.070000 sid=257 sql_batch_completed rows=0 dur=36605us
create table dbo._Reference19NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_Code nvarchar(9) not null,
_Description nvarchar(150) not null,
_Fld4821 nvarchar(9) not null,
_Fld4824RRef binary(16) not null,
_Fld4823RRef binary(16) not null,
_Fld4816RRef binary(16) not null,
_Fld4817 nvarchar(12) not null,
_Fld4825RRef binary(16) not null,
_Fld4822 nvarchar(20) not null,
_Fld4818 nvarchar(9) not null,
_Fld4814 nvarchar(250) not null,
_Fld4815 nvarchar(250) not null,
_Fld4819 nvarchar(15) not null,
_Fld130 nvarchar(2) not null,
_Fld4820 nvarchar(20) not null,
_Fld5847RRef binary(16) not null,
_Fld6482RRef binary(16) not null,
_Fld7056 nvarchar(250) not null,
_Fld7057RRef binary(16) not null,
_Fld7322RRef binary(16) not null,
_Fld7323RRef binary(16) not null,
_Fld7324RRef binary(16) not null,
_Fld8162RRef binary(16) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference19NG SET (LOCK_ESCALATION = DISABLE);

-- 00:58:49.101000 sid=257 sql_batch_completed rows=0 dur=28719us
create table dbo._Reference19_VT131NG (
_Reference19_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo132 numeric(5, 0) not null,
_Fld133RRef binary(16) not null,
_Fld134RRef binary(16) not null,
_Fld135 nvarchar(500) not null,
_Fld136 nvarchar(max) not null,
_Fld137 nvarchar(100) not null,
_Fld138 nvarchar(50) not null,
_Fld139 nvarchar(50) not null,
_Fld140 nvarchar(100) not null,
_Fld141 nvarchar(100) not null,
_Fld142 nvarchar(20) not null,
_Fld143 nvarchar(20) not null,
_Fld5348RRef binary(16) not null,
_Fld5442 datetime2(0) not null,
_Fld6752 nvarchar(max) not null,
_Fld11034 numeric(12, 3) not null,
_Fld11035 binary(1) not null
)
;alter table dbo._Reference19_VT131NG SET (LOCK_ESCALATION = DISABLE);

-- 00:58:49.245000 sid=257 sql_batch_completed rows=0 dur=6700us
CREATE INDEX _Reference19_1NG ON dbo._Reference19NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.250000 sid=257 sql_batch_completed rows=0 dur=2783us
CREATE UNIQUE INDEX _Reference19_2NG ON dbo._Reference19NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.252000 sid=257 sql_batch_completed rows=0 dur=1792us
CREATE UNIQUE INDEX _Reference19_3NG ON dbo._Reference19NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.258000 sid=257 sql_batch_completed rows=0 dur=3958us
CREATE UNIQUE INDEX _Reference19_4NG ON dbo._Reference19NG (_Fld2683, _Fld4817, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.261000 sid=257 sql_batch_completed rows=0 dur=1672us
CREATE UNIQUE INDEX _Reference19_5NG ON dbo._Reference19NG (_Fld2683, _Fld4822, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.271000 sid=257 sql_batch_completed rows=0 dur=9764us
CREATE UNIQUE CLUSTERED INDEX _Reference19_S_HPKNG ON dbo._Reference19NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.275000 sid=257 sql_batch_completed rows=0 dur=3667us
CREATE UNIQUE CLUSTERED INDEX _Reference19_VT131_SKNG ON dbo._Reference19_VT131NG (_Fld2683, _Reference19_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.278000 sid=257 sql_batch_completed rows=0 dur=2467us
CREATE INDEX _Reference19_VT131_1NG ON dbo._Reference19_VT131NG (_Fld2683, _Fld133RRef, _Reference19_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.283000 sid=257 sql_batch_completed rows=0 dur=4032us
CREATE INDEX _Reference19_VT131_2NG ON dbo._Reference19_VT131NG (_Fld2683, _Fld134RRef, _Reference19_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.290000 sid=257 rpc_completed rows=1 dur=4607us
-- args: ,N'@P1 varbinary(max)',0x<6118 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:58:49.302000 sid=257 sql_batch_completed rows=0 dur=5954us
drop index _Reference19_1NG on dbo._Reference19NG;

-- 00:58:49.304000 sid=257 sql_batch_completed rows=0 dur=1640us
drop index _Reference19_2NG on dbo._Reference19NG;

-- 00:58:49.306000 sid=257 sql_batch_completed rows=0 dur=1523us
drop index _Reference19_3NG on dbo._Reference19NG;

-- 00:58:49.308000 sid=257 sql_batch_completed rows=0 dur=1088us
drop index _Reference19_4NG on dbo._Reference19NG;

-- 00:58:49.313000 sid=257 sql_batch_completed rows=0 dur=4804us
drop index _Reference19_5NG on dbo._Reference19NG;

-- 00:58:49.316000 sid=257 sql_batch_completed rows=0 dur=1812us
drop index _Reference19_S_HPKNG on dbo._Reference19NG;

-- 00:58:49.320000 sid=257 sql_batch_completed rows=0 dur=1905us
drop index _Reference19_VT131_1NG on dbo._Reference19_VT131NG;

-- 00:58:49.326000 sid=257 sql_batch_completed rows=0 dur=5471us
drop index _Reference19_VT131_2NG on dbo._Reference19_VT131NG;

-- 00:58:49.331000 sid=257 sql_batch_completed rows=0 dur=4037us
drop index _Reference19_VT131_SKNG on dbo._Reference19_VT131NG;

-- 00:58:49.357000 sid=257 sql_batch_completed rows=4 dur=24318us
INSERT INTO dbo._Reference19NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _Code, _Description, _Fld4821, _Fld4824RRef, _Fld4823RRef, _Fld4816RRef, _Fld4817, _Fld4825RRef, _Fld4822, _Fld4818, _Fld4814, _Fld4815, _Fld4819, _Fld130, _Fld4820, _Fld5847RRef, _Fld6482RRef, _Fld7056, _Fld7057RRef, _Fld7322RRef, _Fld7323RRef, _Fld7324RRef, _Fld8162RRef, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._Code,
T1._Description,
T1._Fld4821,
T1._Fld4824RRef,
T1._Fld4823RRef,
T1._Fld4816RRef,
T1._Fld4817,
T1._Fld4825RRef,
T1._Fld4822,
T1._Fld4818,
T1._Fld4814,
T1._Fld4815,
T1._Fld4819,
T1._Fld130,
T1._Fld4820,
T1._Fld5847RRef,
T1._Fld6482RRef,
T1._Fld7056,
T1._Fld7057RRef,
T1._Fld7322RRef,
T1._Fld7323RRef,
T1._Fld7324RRef,
T1._Fld8162RRef,
T1._Fld2683
FROM dbo._Reference19 T1 WITH(NOLOCK);

-- 00:58:49.369000 sid=257 rpc_completed rows=19 dur=11684us
-- args: ,N'@P1 numeric(10)',0
INSERT INTO dbo._Reference19_VT131NG WITH(TABLOCK) (_LineNo132, _Fld133RRef, _Fld134RRef, _Fld135, _Fld136, _Fld137, _Fld138, _Fld139, _Fld140, _Fld141, _Fld142, _Fld143, _Fld5348RRef, _Fld5442, _Fld6752, _Fld11034, _Fld11035, _Fld2683, _Reference19_IDRRef, _KeyField) SELECT
T2._LineNo132,
T2._Fld133RRef,
T2._Fld134RRef,
T2._Fld135,
T2._Fld136,
T2._Fld137,
T2._Fld138,
T2._Fld139,
T2._Fld140,
T2._Fld141,
T2._Fld142,
T2._Fld143,
T2._Fld5348RRef,
T2._Fld5442,
T2._Fld6752,
CAST(@P1 AS NUMERIC(12, 3)),
0x00,
T2._Fld2683,
T2._Reference19_IDRRef,
T2._KeyField
FROM dbo._Reference19_VT131 T2 WITH(NOLOCK);

-- 00:58:49.383000 sid=257 sql_batch_completed rows=4 dur=9992us
CREATE INDEX _Reference19_1NG ON dbo._Reference19NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.392000 sid=257 sql_batch_completed rows=4 dur=8377us
CREATE UNIQUE INDEX _Reference19_2NG ON dbo._Reference19NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.399000 sid=257 sql_batch_completed rows=4 dur=6020us
CREATE UNIQUE INDEX _Reference19_3NG ON dbo._Reference19NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.402000 sid=257 sql_batch_completed rows=4 dur=2907us
CREATE UNIQUE INDEX _Reference19_4NG ON dbo._Reference19NG (_Fld2683, _Fld4817, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.414000 sid=257 sql_batch_completed rows=4 dur=11220us
CREATE UNIQUE INDEX _Reference19_5NG ON dbo._Reference19NG (_Fld2683, _Fld4822, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.444000 sid=257 sql_batch_completed rows=24 dur=28955us
CREATE UNIQUE CLUSTERED INDEX _Reference19_S_HPKNG ON dbo._Reference19NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.454000 sid=257 sql_batch_completed rows=19 dur=9453us
CREATE UNIQUE CLUSTERED INDEX _Reference19_VT131_SKNG ON dbo._Reference19_VT131NG (_Fld2683, _Reference19_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.462000 sid=257 sql_batch_completed rows=19 dur=7343us
CREATE INDEX _Reference19_VT131_1NG ON dbo._Reference19_VT131NG (_Fld2683, _Fld133RRef, _Reference19_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.469000 sid=257 sql_batch_completed rows=19 dur=6259us
CREATE INDEX _Reference19_VT131_2NG ON dbo._Reference19_VT131NG (_Fld2683, _Fld134RRef, _Reference19_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.490000 sid=257 sql_batch_completed rows=0 dur=12980us
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
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference20NG SET (LOCK_ESCALATION = DISABLE);

-- 00:58:49.502000 sid=257 sql_batch_completed rows=0 dur=8116us
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

-- 00:58:49.511000 sid=257 sql_batch_completed rows=0 dur=6638us
create table dbo._Reference20_VT159NG (
_Reference20_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo160 numeric(5, 0) not null,
_Fld161RRef binary(16) not null,
_Fld162RRef binary(16) not null,
_Fld11036 nvarchar(15) not null,
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

-- 00:58:49.514000 sid=257 sql_batch_completed rows=0 dur=2017us
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.516000 sid=257 sql_batch_completed rows=0 dur=956us
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.518000 sid=257 sql_batch_completed rows=0 dur=918us
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.523000 sid=257 sql_batch_completed rows=0 dur=4786us
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.526000 sid=257 sql_batch_completed rows=0 dur=2734us
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.530000 sid=257 sql_batch_completed rows=0 dur=3833us
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.535000 sid=257 sql_batch_completed rows=0 dur=3990us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.541000 sid=257 sql_batch_completed rows=0 dur=6492us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.546000 sid=257 sql_batch_completed rows=0 dur=3623us
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.548000 sid=257 sql_batch_completed rows=0 dur=1470us
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.552000 sid=257 rpc_completed rows=1 dur=3272us
-- args: ,N'@P1 varbinary(max)',0x<11260 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:58:49.560000 sid=257 sql_batch_completed rows=0 dur=2219us
drop index _Reference20_1NG on dbo._Reference20NG;

-- 00:58:49.563000 sid=257 sql_batch_completed rows=0 dur=2555us
drop index _Reference20_2NG on dbo._Reference20NG;

-- 00:58:49.565000 sid=257 sql_batch_completed rows=0 dur=1140us
drop index _Reference20_3NG on dbo._Reference20NG;

-- 00:58:49.567000 sid=257 sql_batch_completed rows=0 dur=968us
drop index _Reference20_4NG on dbo._Reference20NG;

-- 00:58:49.569000 sid=257 sql_batch_completed rows=0 dur=1511us
drop index _Reference20_5NG on dbo._Reference20NG;

-- 00:58:49.575000 sid=257 sql_batch_completed rows=0 dur=4965us
drop index _Reference20_S_HPKNG on dbo._Reference20NG;

-- 00:58:49.579000 sid=257 sql_batch_completed rows=0 dur=2287us
drop index _Reference20_VT155_SKNG on dbo._Reference20_VT155NG;

-- 00:58:49.582000 sid=257 sql_batch_completed rows=0 dur=1167us
drop index _Reference20_VT159_1NG on dbo._Reference20_VT159NG;

-- 00:58:49.593000 sid=257 sql_batch_completed rows=0 dur=10108us
drop index _Reference20_VT159_2NG on dbo._Reference20_VT159NG;

-- 00:58:49.596000 sid=257 sql_batch_completed rows=0 dur=2284us
drop index _Reference20_VT159_SKNG on dbo._Reference20_VT159NG;

-- 00:58:49.610000 sid=257 sql_batch_completed rows=14 dur=13385us
INSERT INTO dbo._Reference20NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Code, _Description, _Fld151, _Fld152, _Fld153, _Fld154, _Fld949RRef, _Fld6357RRef, _Fld2683) SELECT
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
T1._Fld2683
FROM dbo._Reference20 T1 WITH(NOLOCK);

-- 00:58:49.621000 sid=257 sql_batch_completed rows=29 dur=10092us
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

-- 00:58:49.629000 sid=257 rpc_completed rows=18 dur=6924us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Reference20_VT159NG WITH(TABLOCK) (_LineNo160, _Fld161RRef, _Fld162RRef, _Fld11036, _Fld163, _Fld164, _Fld165, _Fld166, _Fld167, _Fld168, _Fld169, _Fld170, _Fld171, _Fld5024RRef, _Fld6753, _Fld2683, _Reference20_IDRRef, _KeyField) SELECT
T3._LineNo160,
T3._Fld161RRef,
T3._Fld162RRef,
CAST(@P1 AS NVARCHAR(15)),
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

-- 00:58:49.640000 sid=257 sql_batch_completed rows=14 dur=7398us
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.653000 sid=257 sql_batch_completed rows=14 dur=10899us
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.663000 sid=257 sql_batch_completed rows=14 dur=7961us
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.682000 sid=257 sql_batch_completed rows=14 dur=16291us
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.694000 sid=257 sql_batch_completed rows=14 dur=10157us
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.736000 sid=257 sql_batch_completed rows=84 dur=40496us
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.745000 sid=257 sql_batch_completed rows=29 dur=8311us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.758000 sid=257 sql_batch_completed rows=18 dur=13311us
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.764000 sid=257 sql_batch_completed rows=18 dur=4361us
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.770000 sid=257 sql_batch_completed rows=18 dur=4571us
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.789000 sid=257 sql_batch_completed rows=0 dur=11518us
create table dbo._Reference21NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_ParentIDRRef binary(16) not null,
_Code nvarchar(9) not null,
_Description nvarchar(150) not null,
_Fld959RRef binary(16) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference21NG SET (LOCK_ESCALATION = DISABLE);

-- 00:58:49.795000 sid=257 sql_batch_completed rows=0 dur=4385us
create table dbo._Reference21_VT960NG (
_Reference21_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo961 numeric(5, 0) not null,
_Fld962RRef binary(16) not null,
_Fld11037 nvarchar(30) not null
)
;alter table dbo._Reference21_VT960NG SET (LOCK_ESCALATION = DISABLE);

-- 00:58:49.804000 sid=257 sql_batch_completed rows=0 dur=7811us
CREATE INDEX _Reference21_1NG ON dbo._Reference21NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.810000 sid=257 sql_batch_completed rows=0 dur=3652us
CREATE UNIQUE INDEX _Reference21_2NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.813000 sid=257 sql_batch_completed rows=0 dur=2448us
CREATE UNIQUE INDEX _Reference21_3NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.815000 sid=257 sql_batch_completed rows=0 dur=1254us
CREATE UNIQUE INDEX _Reference21_4NG ON dbo._Reference21NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.817000 sid=257 sql_batch_completed rows=0 dur=1174us
CREATE UNIQUE INDEX _Reference21_5NG ON dbo._Reference21NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.820000 sid=257 sql_batch_completed rows=0 dur=2706us
CREATE UNIQUE CLUSTERED INDEX _Reference21_S_HPKNG ON dbo._Reference21NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.823000 sid=257 sql_batch_completed rows=0 dur=2634us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT960_SKNG ON dbo._Reference21_VT960NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:49.824000 sid=257 rpc_completed rows=1 dur=782us
-- args: ,N'@P1 varbinary(max)',0x<13526 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:58:49.830000 sid=257 sql_batch_completed rows=0 dur=1307us
drop index _Reference21_1NG on dbo._Reference21NG;

-- 00:58:49.838000 sid=257 sql_batch_completed rows=0 dur=7336us
drop index _Reference21_2NG on dbo._Reference21NG;

-- 00:58:49.841000 sid=257 sql_batch_completed rows=0 dur=2743us
drop index _Reference21_3NG on dbo._Reference21NG;

-- 00:58:49.845000 sid=257 sql_batch_completed rows=0 dur=2603us
drop index _Reference21_4NG on dbo._Reference21NG;

-- 00:58:49.849000 sid=257 sql_batch_completed rows=0 dur=3580us
drop index _Reference21_5NG on dbo._Reference21NG;

-- 00:58:49.857000 sid=257 sql_batch_completed rows=0 dur=7130us
drop index _Reference21_S_HPKNG on dbo._Reference21NG;

-- 00:58:49.864000 sid=257 sql_batch_completed rows=0 dur=5286us
drop index _Reference21_VT960_SKNG on dbo._Reference21_VT960NG;

-- 00:58:49.900000 sid=257 sql_batch_completed rows=4 dur=35591us
INSERT INTO dbo._Reference21NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Code, _Description, _Fld959RRef, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._ParentIDRRef,
T1._Code,
T1._Description,
T1._Fld959RRef,
T1._Fld2683
FROM dbo._Reference21 T1 WITH(NOLOCK);

-- 00:58:49.927000 sid=257 rpc_completed rows=4 dur=25659us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Reference21_VT960NG WITH(TABLOCK) (_LineNo961, _Fld962RRef, _Fld11037, _Fld2683, _Reference21_IDRRef, _KeyField) SELECT
T2._LineNo961,
T2._Fld962RRef,
CAST(@P1 AS NVARCHAR(30)),
T2._Fld2683,
T2._Reference21_IDRRef,
T2._KeyField
FROM dbo._Reference21_VT960 T2 WITH(NOLOCK);

-- 00:58:49.942000 sid=257 sql_batch_completed rows=4 dur=12025us
CREATE INDEX _Reference21_1NG ON dbo._Reference21NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.959000 sid=257 sql_batch_completed rows=4 dur=15542us
CREATE UNIQUE INDEX _Reference21_2NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.970000 sid=257 sql_batch_completed rows=4 dur=10293us
CREATE UNIQUE INDEX _Reference21_3NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.988000 sid=257 sql_batch_completed rows=4 dur=11597us
CREATE UNIQUE INDEX _Reference21_4NG ON dbo._Reference21NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:49.999000 sid=257 sql_batch_completed rows=4 dur=10446us
CREATE UNIQUE INDEX _Reference21_5NG ON dbo._Reference21NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:55.772000 sid=257 sql_batch_completed rows=24 dur=5773941us
CREATE UNIQUE CLUSTERED INDEX _Reference21_S_HPKNG ON dbo._Reference21NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:55.788000 sid=257 sql_batch_completed rows=4 dur=13224us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT960_SKNG ON dbo._Reference21_VT960NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:56.804000 sid=257 sql_batch_completed rows=0 dur=84495us
create table dbo._Document578NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_Date_Time datetime2(0) not null,
_Number nvarchar(11) not null,
_Posted binary(1) not null,
_Fld1518 nvarchar(max) not null,
_Fld666RRef binary(16) not null,
_Fld665RRef binary(16) not null,
_Fld667RRef binary(16) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Document578NG SET (LOCK_ESCALATION = DISABLE);

-- 00:58:56.852000 sid=257 sql_batch_completed rows=0 dur=44687us
create table dbo._Document578_VT668NG (
_Document578_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo669 numeric(5, 0) not null,
_Fld11038 numeric(15, 3) not null,
_Fld670RRef binary(16) not null,
_Fld673 numeric(15, 3) not null,
_Fld11039 nvarchar(10) not null
)
;alter table dbo._Document578_VT668NG SET (LOCK_ESCALATION = DISABLE);

-- 00:58:56.945000 sid=257 sql_batch_completed rows=0 dur=85306us
CREATE UNIQUE INDEX _Document578_1NG ON dbo._Document578NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:56.958000 sid=257 sql_batch_completed rows=0 dur=11055us
CREATE UNIQUE INDEX _Document578_2NG ON dbo._Document578NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:56.976000 sid=257 sql_batch_completed rows=0 dur=17292us
CREATE UNIQUE CLUSTERED INDEX _Document578_S_HPKNG ON dbo._Document578NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:56.995000 sid=257 sql_batch_completed rows=0 dur=18319us
CREATE UNIQUE CLUSTERED INDEX _Document578_VT668_SKNG ON dbo._Document578_VT668NG (_Fld2683, _Document578_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:57.008000 sid=257 sql_batch_completed rows=0 dur=11695us
CREATE INDEX _Document578_VT668_1NG ON dbo._Document578_VT668NG (_Fld2683, _Fld11039, _Document578_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:58:57.019000 sid=257 rpc_completed rows=1 dur=8263us
-- args: ,N'@P1 varbinary(max)',0x<15926 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:58:57.036000 sid=257 sql_batch_completed rows=0 dur=12338us
drop index _Document578_1NG on dbo._Document578NG;

-- 00:58:57.052000 sid=257 sql_batch_completed rows=0 dur=12414us
drop index _Document578_2NG on dbo._Document578NG;

-- 00:58:57.063000 sid=257 sql_batch_completed rows=0 dur=9979us
drop index _Document578_S_HPKNG on dbo._Document578NG;

-- 00:58:57.075000 sid=257 sql_batch_completed rows=0 dur=8511us
drop index _Document578_VT668_1NG on dbo._Document578_VT668NG;

-- 00:58:57.090000 sid=257 sql_batch_completed rows=0 dur=12367us
drop index _Document578_VT668_SKNG on dbo._Document578_VT668NG;

-- 00:58:57.222000 sid=257 sql_batch_completed rows=10 dur=131696us
INSERT INTO dbo._Document578NG WITH(TABLOCK) (_IDRRef, _Marked, _Date_Time, _Number, _Posted, _Fld1518, _Fld666RRef, _Fld665RRef, _Fld667RRef, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._Date_Time,
T1._Number,
T1._Posted,
T1._Fld1518,
T1._Fld666RRef,
T1._Fld665RRef,
T1._Fld667RRef,
T1._Fld2683
FROM dbo._Document578 T1 WITH(NOLOCK);

-- 00:58:57.342000 sid=257 rpc_completed rows=31 dur=117522us
-- args: ,N'@P1 numeric(10),@P2 nvarchar(4000)',0,N''
INSERT INTO dbo._Document578_VT668NG WITH(TABLOCK) (_LineNo669, _Fld11038, _Fld670RRef, _Fld673, _Fld11039, _Fld2683, _Document578_IDRRef, _KeyField) SELECT
T2._LineNo669,
CAST(@P1 AS NUMERIC(15, 3)),
T2._Fld670RRef,
T2._Fld673,
CAST(@P2 AS NVARCHAR(10)),
T2._Fld2683,
T2._Document578_IDRRef,
T2._KeyField
FROM dbo._Document578_VT668 T2 WITH(NOLOCK);

-- 00:59:03.275000 sid=257 sql_batch_completed rows=10 dur=5928773us
CREATE UNIQUE INDEX _Document578_1NG ON dbo._Document578NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:59:03.294000 sid=257 sql_batch_completed rows=10 dur=15523us
CREATE UNIQUE INDEX _Document578_2NG ON dbo._Document578NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:59:03.311000 sid=257 sql_batch_completed rows=30 dur=17047us
CREATE UNIQUE CLUSTERED INDEX _Document578_S_HPKNG ON dbo._Document578NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:59:03.319000 sid=257 sql_batch_completed rows=31 dur=6724us
CREATE UNIQUE CLUSTERED INDEX _Document578_VT668_SKNG ON dbo._Document578_VT668NG (_Fld2683, _Document578_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:59:03.327000 sid=257 sql_batch_completed rows=31 dur=6821us
CREATE INDEX _Document578_VT668_1NG ON dbo._Document578_VT668NG (_Fld2683, _Fld11039, _Document578_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:59:03.417000 sid=257 sql_batch_completed rows=0 dur=18491us
create table dbo._DbCopiesUpdatesNG (
_CopyId binary(16) not null,
_TrNum int,
_TrTime datetime2(0),
_UpdateId binary(16),
_LastUpdateResult numeric(2, 0),
_LastUpdateError varbinary(max)
)
;alter table dbo._DbCopiesUpdatesNG SET (LOCK_ESCALATION = DISABLE);

-- 00:59:03.437000 sid=257 sql_batch_completed rows=0 dur=16281us
CREATE UNIQUE CLUSTERED INDEX _DbCopiesUpdates_1NG ON dbo._DbCopiesUpdatesNG (_CopyId, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:59:03.445000 sid=257 rpc_completed rows=1 dur=7072us
-- args: ,N'@P1 varbinary(max)',0x<16814 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:59:03.489000 sid=257 sql_batch_completed rows=0 dur=35320us
INSERT INTO dbo._DbCopiesUpdatesNG WITH(TABLOCK) (_CopyId, _TrNum, _TrTime) SELECT
T1._CopyId,
(T1._TrNum + 0.0),
T1._TrTime
FROM dbo._DbCopiesUpdates T1 WITH(NOLOCK);

-- 00:59:03.519000 sid=257 sql_batch_completed rows=0 dur=23749us
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

-- 00:59:03.541000 sid=257 sql_batch_completed rows=0 dur=21381us
CREATE UNIQUE CLUSTERED INDEX _DbCopies_1NG ON dbo._DbCopiesNG (_CopyId, _CopyName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:59:03.546000 sid=257 rpc_completed rows=1 dur=4237us
-- args: ,N'@P1 varbinary(max)',0x<18342 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:59:03.550000 sid=257 rpc_completed rows=0 dur=1877us
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

-- 00:59:04.099000 sid=257 sql_batch_completed rows=0 dur=25624us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 00:59:04.116000 sid=257 sql_batch_completed rows=0 dur=14540us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 00:59:04.123000 sid=257 sql_batch_completed rows=0 dur=6342us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:59:04.129000 sid=257 sql_batch_completed rows=0 dur=4185us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:59:04.134000 sid=257 sql_batch_completed rows=0 dur=3866us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:59:04.142000 sid=257 rpc_completed rows=1 dur=2311us
-- args: ,N'@P1 varbinary(max)',0x<19406 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:59:04.154000 sid=257 sql_batch_completed rows=0 dur=6672us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 00:59:04.164000 sid=257 sql_batch_completed rows=0 dur=4287us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 00:59:04.174000 sid=257 sql_batch_completed rows=0 dur=5608us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 00:59:04.472000 sid=257 sql_batch_completed rows=10000 dur=41060us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 00:59:04.512000 sid=257 sql_batch_completed rows=9996 dur=36739us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=9996);

-- 00:59:05.051000 sid=257 sql_batch_completed rows=10000 dur=71020us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 00:59:05.083000 sid=257 sql_batch_completed rows=10000 dur=28675us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 00:59:05.097000 sid=257 sql_batch_completed rows=658 dur=13519us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 00:59:05.294000 sid=257 sql_batch_completed rows=685 dur=16451us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=685);

-- 00:59:05.324000 sid=257 sql_batch_completed rows=703 dur=16795us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=703);

-- 00:59:05.408000 sid=257 sql_batch_completed rows=20685 dur=60833us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:59:05.446000 sid=257 sql_batch_completed rows=20685 dur=33097us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:59:05.494000 sid=257 sql_batch_completed rows=21357 dur=47052us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:59:06.769000 sid=257 rpc_completed rows=1 dur=13996us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 00:59:06.818000 sid=257 rpc_completed rows=1 dur=1222us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 00:59:08.020000 sid=257 rpc_completed rows=1 dur=218us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<888 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 00:59:08.058000 sid=257 rpc_completed rows=1 dur=302us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF00EFAB36899F1118F4F00E04C680093,0x<7728 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 01:00:35.037000 sid=257 sql_batch_completed rows=1 dur=29349us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 01:00:35.199000 sid=257 sql_batch_completed rows=0 dur=119087us
drop table dbo._Reference19;

-- 01:00:35.311000 sid=257 sql_batch_completed rows=0 dur=65131us
drop table dbo._Reference19_VT131;

-- 01:00:35.455000 sid=257 sql_batch_completed rows=0 dur=113494us
drop table dbo._Reference20;

-- 01:00:35.541000 sid=257 sql_batch_completed rows=0 dur=43226us
drop table dbo._Reference20_VT155;

-- 01:00:35.625000 sid=257 sql_batch_completed rows=0 dur=54968us
drop table dbo._Reference20_VT159;

-- 01:00:35.770000 sid=257 sql_batch_completed rows=0 dur=74968us
drop table dbo._Reference21;

-- 01:00:35.821000 sid=257 sql_batch_completed rows=0 dur=25737us
drop table dbo._Reference21_VT960;

-- 01:00:35.991000 sid=257 sql_batch_completed rows=0 dur=101033us
drop table dbo._Document578;

-- 01:00:36.062000 sid=257 sql_batch_completed rows=0 dur=28620us
drop table dbo._Document578_VT668;

-- 01:00:36.180000 sid=257 sql_batch_completed rows=0 dur=64550us
drop table dbo._DbCopiesUpdates;

-- 01:00:36.237000 sid=257 sql_batch_completed rows=0 dur=26102us
drop table dbo._DbCopies;

-- 01:00:36.463000 sid=257 sql_batch_completed rows=0 dur=161216us
drop table dbo._ConfigChngR;

-- 01:00:36.534000 sid=257 sql_batch_completed rows=0 dur=45364us
drop table dbo._ConfigChngR_ExtProps;

-- 01:00:36.548000 sid=257 sql_batch_completed rows=1 dur=13007us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 01:00:37.468000 sid=257 rpc_completed rows=30 dur=912754us
-- args: N'_Reference19NG',N'_Reference19'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.496000 sid=257 rpc_completed rows=25 dur=26181us
-- args: N'_Reference19_VT131NG',N'_Reference19_VT131'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.506000 sid=257 rpc_completed rows=16 dur=8811us
-- args: N'_Reference19._Reference19_1NG',N'_Reference19_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.515000 sid=257 rpc_completed rows=16 dur=7310us
-- args: N'_Reference19._Reference19_2NG',N'_Reference19_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.524000 sid=257 rpc_completed rows=16 dur=7778us
-- args: N'_Reference19._Reference19_3NG',N'_Reference19_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.528000 sid=257 rpc_completed rows=16 dur=2859us
-- args: N'_Reference19._Reference19_4NG',N'_Reference19_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.542000 sid=257 rpc_completed rows=16 dur=11794us
-- args: N'_Reference19._Reference19_5NG',N'_Reference19_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.551000 sid=257 rpc_completed rows=16 dur=7593us
-- args: N'_Reference19._Reference19_S_HPKNG',N'_Reference19_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.562000 sid=257 rpc_completed rows=16 dur=9443us
-- args: N'_Reference19_VT131._Reference19_VT131_SKNG',N'_Reference19_VT131_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.572000 sid=257 rpc_completed rows=16 dur=7788us
-- args: N'_Reference19_VT131._Reference19_VT131_1NG',N'_Reference19_VT131_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.575000 sid=257 rpc_completed rows=16 dur=2643us
-- args: N'_Reference19_VT131._Reference19_VT131_2NG',N'_Reference19_VT131_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.603000 sid=257 rpc_completed rows=25 dur=26557us
-- args: N'_Reference20NG',N'_Reference20'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.616000 sid=257 rpc_completed rows=25 dur=11967us
-- args: N'_Reference20_VT155NG',N'_Reference20_VT155'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.643000 sid=257 rpc_completed rows=25 dur=25182us
-- args: N'_Reference20_VT159NG',N'_Reference20_VT159'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.656000 sid=257 rpc_completed rows=16 dur=11347us
-- args: N'_Reference20._Reference20_1NG',N'_Reference20_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.665000 sid=257 rpc_completed rows=16 dur=5615us
-- args: N'_Reference20._Reference20_2NG',N'_Reference20_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.680000 sid=257 rpc_completed rows=16 dur=14425us
-- args: N'_Reference20._Reference20_3NG',N'_Reference20_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.688000 sid=257 rpc_completed rows=16 dur=6551us
-- args: N'_Reference20._Reference20_4NG',N'_Reference20_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.702000 sid=257 rpc_completed rows=16 dur=12854us
-- args: N'_Reference20._Reference20_5NG',N'_Reference20_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.709000 sid=257 rpc_completed rows=16 dur=5260us
-- args: N'_Reference20._Reference20_S_HPKNG',N'_Reference20_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.729000 sid=257 rpc_completed rows=16 dur=18594us
-- args: N'_Reference20_VT155._Reference20_VT155_SKNG',N'_Reference20_VT155_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.736000 sid=257 rpc_completed rows=16 dur=5337us
-- args: N'_Reference20_VT159._Reference20_VT159_SKNG',N'_Reference20_VT159_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.749000 sid=257 rpc_completed rows=16 dur=12125us
-- args: N'_Reference20_VT159._Reference20_VT159_1NG',N'_Reference20_VT159_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.763000 sid=257 rpc_completed rows=16 dur=12707us
-- args: N'_Reference20_VT159._Reference20_VT159_2NG',N'_Reference20_VT159_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.766000 sid=257 rpc_completed rows=25 dur=2391us
-- args: N'_Reference21NG',N'_Reference21'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.780000 sid=257 rpc_completed rows=25 dur=12442us
-- args: N'_Reference21_VT960NG',N'_Reference21_VT960'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.783000 sid=257 rpc_completed rows=16 dur=2243us
-- args: N'_Reference21._Reference21_1NG',N'_Reference21_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.795000 sid=257 rpc_completed rows=16 dur=10381us
-- args: N'_Reference21._Reference21_2NG',N'_Reference21_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.799000 sid=257 rpc_completed rows=16 dur=3466us
-- args: N'_Reference21._Reference21_3NG',N'_Reference21_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.811000 sid=257 rpc_completed rows=16 dur=11510us
-- args: N'_Reference21._Reference21_4NG',N'_Reference21_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.815000 sid=257 rpc_completed rows=16 dur=2632us
-- args: N'_Reference21._Reference21_5NG',N'_Reference21_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.819000 sid=257 rpc_completed rows=16 dur=3178us
-- args: N'_Reference21._Reference21_S_HPKNG',N'_Reference21_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.826000 sid=257 rpc_completed rows=16 dur=5478us
-- args: N'_Reference21_VT960._Reference21_VT960_SKNG',N'_Reference21_VT960_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.833000 sid=257 rpc_completed rows=25 dur=5942us
-- args: N'_Document578NG',N'_Document578'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.838000 sid=257 rpc_completed rows=25 dur=4162us
-- args: N'_Document578_VT668NG',N'_Document578_VT668'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.842000 sid=257 rpc_completed rows=16 dur=3063us
-- args: N'_Document578._Document578_1NG',N'_Document578_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.848000 sid=257 rpc_completed rows=16 dur=4412us
-- args: N'_Document578._Document578_2NG',N'_Document578_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.855000 sid=257 rpc_completed rows=16 dur=6632us
-- args: N'_Document578._Document578_S_HPKNG',N'_Document578_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.866000 sid=257 rpc_completed rows=16 dur=9585us
-- args: N'_Document578_VT668._Document578_VT668_SKNG',N'_Document578_VT668_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.874000 sid=257 rpc_completed rows=16 dur=7129us
-- args: N'_Document578_VT668._Document578_VT668_1NG',N'_Document578_VT668_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.898000 sid=257 rpc_completed rows=25 dur=22543us
-- args: N'_DbCopiesUpdatesNG',N'_DbCopiesUpdates'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.912000 sid=257 rpc_completed rows=16 dur=12937us
-- args: N'_DbCopiesUpdates._DbCopiesUpdates_1NG',N'_DbCopiesUpdates_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.933000 sid=257 rpc_completed rows=25 dur=20122us
-- args: N'_DbCopiesNG',N'_DbCopies'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.944000 sid=257 rpc_completed rows=16 dur=9936us
-- args: N'_DbCopies._DbCopies_1NG',N'_DbCopies_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:37.965000 sid=257 rpc_completed rows=25 dur=19752us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:37.990000 sid=257 rpc_completed rows=25 dur=24651us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 01:00:38.002000 sid=257 rpc_completed rows=16 dur=10839us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:38.014000 sid=257 rpc_completed rows=16 dur=10530us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:38.025000 sid=257 rpc_completed rows=16 dur=7223us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 01:00:38.177000 sid=257 sql_batch_completed rows=0 dur=5684us
ALTER INDEX PK___ConfigC__AC8ED0C47E5E5969 ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.219000 sid=257 sql_batch_completed rows=0 dur=42035us
ALTER INDEX PK___Referen__AC8ED0C4077A8F3B ON _Reference10877X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.243000 sid=257 sql_batch_completed rows=0 dur=23246us
ALTER INDEX PK___Referen__AC8ED0C4DBBB6986 ON _Reference10878X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.266000 sid=257 sql_batch_completed rows=0 dur=23333us
ALTER INDEX PK___Referen__AC8ED0C4720747AF ON _Reference10825X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.306000 sid=257 sql_batch_completed rows=0 dur=39769us
ALTER INDEX PK___Enum108__AC8ED0C45E875D60 ON _Enum10826X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.342000 sid=257 sql_batch_completed rows=0 dur=34946us
ALTER INDEX PK___Enum110__AC8ED0C4A5E2EE79 ON _Enum11017X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.394000 sid=257 sql_batch_completed rows=0 dur=52406us
ALTER INDEX PK___Documen__AC8ED0C48EB57390 ON _Document10069X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.444000 sid=257 sql_batch_completed rows=0 dur=49502us
ALTER INDEX PK___Referen__AC8ED0C43F77AD82 ON _Reference2596X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.463000 sid=257 sql_batch_completed rows=0 dur=18887us
ALTER INDEX PK___Referen__AC8ED0C45FCE1C7A ON _Reference10523X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 01:00:38.512000 sid=257 rpc_completed rows=1 dur=37032us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1956140 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 01:00:38.598000 sid=257 rpc_completed rows=1 dur=44898us
-- args: ,N'@P1 varbinary(max)',0x<1956140 hex>
UPDATE DBSchema SET SerializedData = @P1;

