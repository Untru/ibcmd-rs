-- 03:26:44.291000 sid=145 sql_batch_completed rows=0 dur=29698us
create table dbo._Reference14NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_OwnerIDRRef binary(16) not null,
_Description nvarchar(100) not null,
_Fld61 nvarchar(10) not null,
_Fld62RRef binary(16) not null,
_Fld63 datetime2(0) not null,
_Fld64 datetime2(0) not null,
_Fld65RRef binary(16) not null,
_Fld66 nvarchar(max) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference14NG SET (LOCK_ESCALATION = DISABLE);

-- 03:26:44.297000 sid=145 sql_batch_completed rows=0 dur=3464us
create table dbo._Reference14_VT67NG (
_Reference14_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo68 numeric(5, 0) not null,
_Fld11034 numeric(12, 3) not null,
_Fld69RRef binary(16) not null,
_Fld70RRef binary(16) not null,
_Fld11035 nvarchar(15) not null,
_Fld71 nvarchar(500) not null,
_Fld72 nvarchar(max) not null,
_Fld73 nvarchar(100) not null,
_Fld74 nvarchar(50) not null,
_Fld75 nvarchar(50) not null,
_Fld76 nvarchar(100) not null,
_Fld77 nvarchar(100) not null,
_Fld78 nvarchar(20) not null,
_Fld79 nvarchar(20) not null,
_Fld6750 nvarchar(max) not null,
_Fld11036 binary(1) not null
)
;alter table dbo._Reference14_VT67NG SET (LOCK_ESCALATION = DISABLE);

-- 03:26:44.342000 sid=145 sql_batch_completed rows=0 dur=2973us
CREATE INDEX _Reference14_1NG ON dbo._Reference14NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.344000 sid=145 sql_batch_completed rows=0 dur=1745us
CREATE UNIQUE INDEX _Reference14_2NG ON dbo._Reference14NG (_Fld2683, _OwnerIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.347000 sid=145 sql_batch_completed rows=0 dur=1547us
CREATE UNIQUE INDEX _Reference14_3NG ON dbo._Reference14NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.352000 sid=145 sql_batch_completed rows=0 dur=5019us
CREATE UNIQUE CLUSTERED INDEX _Reference14_S_HPKNG ON dbo._Reference14NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:44.355000 sid=145 sql_batch_completed rows=0 dur=2484us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT67_SKNG ON dbo._Reference14_VT67NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:44.357000 sid=145 sql_batch_completed rows=0 dur=929us
CREATE INDEX _Reference14_VT67_1NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld11034, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.358000 sid=145 sql_batch_completed rows=0 dur=885us
CREATE INDEX _Reference14_VT67_2NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld69RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.360000 sid=145 sql_batch_completed rows=0 dur=974us
CREATE INDEX _Reference14_VT67_3NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld70RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.362000 sid=145 sql_batch_completed rows=0 dur=1312us
CREATE INDEX _Reference14_VT67_4NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld11035, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.367000 sid=145 rpc_completed rows=1 dur=1858us
-- args: ,N'@P1 varbinary(max)',0x<4290 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:26:44.375000 sid=145 sql_batch_completed rows=0 dur=1829us
drop index _Reference14_1NG on dbo._Reference14NG;

-- 03:26:44.377000 sid=145 sql_batch_completed rows=0 dur=1245us
drop index _Reference14_2NG on dbo._Reference14NG;

-- 03:26:44.378000 sid=145 sql_batch_completed rows=0 dur=1002us
drop index _Reference14_3NG on dbo._Reference14NG;

-- 03:26:44.380000 sid=145 sql_batch_completed rows=0 dur=1157us
drop index _Reference14_S_HPKNG on dbo._Reference14NG;

-- 03:26:44.384000 sid=145 sql_batch_completed rows=0 dur=1724us
drop index _Reference14_VT67_1NG on dbo._Reference14_VT67NG;

-- 03:26:44.386000 sid=145 sql_batch_completed rows=0 dur=962us
drop index _Reference14_VT67_2NG on dbo._Reference14_VT67NG;

-- 03:26:44.387000 sid=145 sql_batch_completed rows=0 dur=1022us
drop index _Reference14_VT67_3NG on dbo._Reference14_VT67NG;

-- 03:26:44.389000 sid=145 sql_batch_completed rows=0 dur=688us
drop index _Reference14_VT67_4NG on dbo._Reference14_VT67NG;

-- 03:26:44.390000 sid=145 sql_batch_completed rows=0 dur=992us
drop index _Reference14_VT67_SKNG on dbo._Reference14_VT67NG;

-- 03:26:44.397000 sid=145 sql_batch_completed rows=3 dur=5858us
INSERT INTO dbo._Reference14NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _OwnerIDRRef, _Description, _Fld61, _Fld62RRef, _Fld63, _Fld64, _Fld65RRef, _Fld66, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._OwnerIDRRef,
T1._Description,
T1._Fld61,
T1._Fld62RRef,
T1._Fld63,
T1._Fld64,
T1._Fld65RRef,
T1._Fld66,
T1._Fld2683
FROM dbo._Reference14 T1 WITH(NOLOCK);

-- 03:26:44.400000 sid=145 rpc_completed rows=4 dur=2830us
-- args: ,N'@P1 numeric(10),@P2 nvarchar(4000)',0,N''
INSERT INTO dbo._Reference14_VT67NG WITH(TABLOCK) (_LineNo68, _Fld11034, _Fld69RRef, _Fld70RRef, _Fld11035, _Fld71, _Fld72, _Fld73, _Fld74, _Fld75, _Fld76, _Fld77, _Fld78, _Fld79, _Fld6750, _Fld11036, _Fld2683, _Reference14_IDRRef, _KeyField) SELECT
T2._LineNo68,
CAST(@P1 AS NUMERIC(12, 3)),
T2._Fld69RRef,
T2._Fld70RRef,
CAST(@P2 AS NVARCHAR(15)),
T2._Fld71,
T2._Fld72,
T2._Fld73,
T2._Fld74,
T2._Fld75,
T2._Fld76,
T2._Fld77,
T2._Fld78,
T2._Fld79,
T2._Fld6750,
0x00,
T2._Fld2683,
T2._Reference14_IDRRef,
T2._KeyField
FROM dbo._Reference14_VT67 T2 WITH(NOLOCK);

-- 03:26:44.404000 sid=145 sql_batch_completed rows=3 dur=2899us
CREATE INDEX _Reference14_1NG ON dbo._Reference14NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.407000 sid=145 sql_batch_completed rows=3 dur=1984us
CREATE UNIQUE INDEX _Reference14_2NG ON dbo._Reference14NG (_Fld2683, _OwnerIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.410000 sid=145 sql_batch_completed rows=3 dur=1861us
CREATE UNIQUE INDEX _Reference14_3NG ON dbo._Reference14NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.419000 sid=145 sql_batch_completed rows=12 dur=8517us
CREATE UNIQUE CLUSTERED INDEX _Reference14_S_HPKNG ON dbo._Reference14NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:44.423000 sid=145 sql_batch_completed rows=4 dur=4390us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT67_SKNG ON dbo._Reference14_VT67NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:44.426000 sid=145 sql_batch_completed rows=4 dur=2035us
CREATE INDEX _Reference14_VT67_1NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld11034, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.428000 sid=145 sql_batch_completed rows=4 dur=1658us
CREATE INDEX _Reference14_VT67_2NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld69RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.431000 sid=145 sql_batch_completed rows=4 dur=1691us
CREATE INDEX _Reference14_VT67_3NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld70RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.433000 sid=145 sql_batch_completed rows=4 dur=1514us
CREATE INDEX _Reference14_VT67_4NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld11035, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.447000 sid=145 sql_batch_completed rows=0 dur=2971us
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

-- 03:26:44.451000 sid=145 sql_batch_completed rows=0 dur=2104us
create table dbo._Reference21_VT960NG (
_Reference21_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo961 numeric(5, 0) not null,
_Fld962RRef binary(16) not null,
_Fld11037 nvarchar(30) not null
)
;alter table dbo._Reference21_VT960NG SET (LOCK_ESCALATION = DISABLE);

-- 03:26:44.454000 sid=145 sql_batch_completed rows=0 dur=2020us
CREATE INDEX _Reference21_1NG ON dbo._Reference21NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.456000 sid=145 sql_batch_completed rows=0 dur=1050us
CREATE UNIQUE INDEX _Reference21_2NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.458000 sid=145 sql_batch_completed rows=0 dur=1327us
CREATE UNIQUE INDEX _Reference21_3NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.460000 sid=145 sql_batch_completed rows=0 dur=849us
CREATE UNIQUE INDEX _Reference21_4NG ON dbo._Reference21NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.461000 sid=145 sql_batch_completed rows=0 dur=726us
CREATE UNIQUE INDEX _Reference21_5NG ON dbo._Reference21NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.464000 sid=145 sql_batch_completed rows=0 dur=2941us
CREATE UNIQUE CLUSTERED INDEX _Reference21_S_HPKNG ON dbo._Reference21NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:44.466000 sid=145 sql_batch_completed rows=0 dur=2014us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT960_SKNG ON dbo._Reference21_VT960NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:44.467000 sid=145 rpc_completed rows=1 dur=249us
-- args: ,N'@P1 varbinary(max)',0x<6556 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:26:44.473000 sid=145 sql_batch_completed rows=0 dur=1461us
drop index _Reference21_1NG on dbo._Reference21NG;

-- 03:26:44.475000 sid=145 sql_batch_completed rows=0 dur=805us
drop index _Reference21_2NG on dbo._Reference21NG;

-- 03:26:44.476000 sid=145 sql_batch_completed rows=0 dur=844us
drop index _Reference21_3NG on dbo._Reference21NG;

-- 03:26:44.477000 sid=145 sql_batch_completed rows=0 dur=880us
drop index _Reference21_4NG on dbo._Reference21NG;

-- 03:26:44.478000 sid=145 sql_batch_completed rows=0 dur=753us
drop index _Reference21_5NG on dbo._Reference21NG;

-- 03:26:44.480000 sid=145 sql_batch_completed rows=0 dur=870us
drop index _Reference21_S_HPKNG on dbo._Reference21NG;

-- 03:26:44.482000 sid=145 sql_batch_completed rows=0 dur=1211us
drop index _Reference21_VT960_SKNG on dbo._Reference21_VT960NG;

-- 03:26:44.489000 sid=145 sql_batch_completed rows=4 dur=6950us
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

-- 03:26:44.492000 sid=145 rpc_completed rows=4 dur=2538us
-- args: ,N'@P1 nvarchar(4000)',N''
INSERT INTO dbo._Reference21_VT960NG WITH(TABLOCK) (_LineNo961, _Fld962RRef, _Fld11037, _Fld2683, _Reference21_IDRRef, _KeyField) SELECT
T2._LineNo961,
T2._Fld962RRef,
CAST(@P1 AS NVARCHAR(30)),
T2._Fld2683,
T2._Reference21_IDRRef,
T2._KeyField
FROM dbo._Reference21_VT960 T2 WITH(NOLOCK);

-- 03:26:44.496000 sid=145 sql_batch_completed rows=4 dur=2457us
CREATE INDEX _Reference21_1NG ON dbo._Reference21NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.498000 sid=145 sql_batch_completed rows=4 dur=1861us
CREATE UNIQUE INDEX _Reference21_2NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.504000 sid=145 sql_batch_completed rows=4 dur=4601us
CREATE UNIQUE INDEX _Reference21_3NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.508000 sid=145 sql_batch_completed rows=4 dur=2986us
CREATE UNIQUE INDEX _Reference21_4NG ON dbo._Reference21NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.511000 sid=145 sql_batch_completed rows=4 dur=2662us
CREATE UNIQUE INDEX _Reference21_5NG ON dbo._Reference21NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:44.521000 sid=145 sql_batch_completed rows=24 dur=10084us
CREATE UNIQUE CLUSTERED INDEX _Reference21_S_HPKNG ON dbo._Reference21NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:44.525000 sid=145 sql_batch_completed rows=4 dur=3387us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT960_SKNG ON dbo._Reference21_VT960NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:45.035000 sid=145 sql_batch_completed rows=0 dur=9172us
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

-- 03:26:45.039000 sid=145 sql_batch_completed rows=0 dur=2555us
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

-- 03:26:45.042000 sid=145 sql_batch_completed rows=0 dur=1569us
CREATE UNIQUE INDEX _Document578_1NG ON dbo._Document578NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.044000 sid=145 sql_batch_completed rows=0 dur=1592us
CREATE UNIQUE INDEX _Document578_2NG ON dbo._Document578NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.046000 sid=145 sql_batch_completed rows=0 dur=1708us
CREATE UNIQUE CLUSTERED INDEX _Document578_S_HPKNG ON dbo._Document578NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:45.050000 sid=145 sql_batch_completed rows=0 dur=3267us
CREATE UNIQUE CLUSTERED INDEX _Document578_VT668_SKNG ON dbo._Document578_VT668NG (_Fld2683, _Document578_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:45.052000 sid=145 sql_batch_completed rows=0 dur=1199us
CREATE INDEX _Document578_VT668_1NG ON dbo._Document578_VT668NG (_Fld2683, _Fld11039, _Document578_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.055000 sid=145 rpc_completed rows=1 dur=423us
-- args: ,N'@P1 varbinary(max)',0x<8956 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:26:45.059000 sid=145 sql_batch_completed rows=0 dur=873us
drop index _Document578_1NG on dbo._Document578NG;

-- 03:26:45.060000 sid=145 sql_batch_completed rows=0 dur=717us
drop index _Document578_2NG on dbo._Document578NG;

-- 03:26:45.062000 sid=145 sql_batch_completed rows=0 dur=802us
drop index _Document578_S_HPKNG on dbo._Document578NG;

-- 03:26:45.065000 sid=145 sql_batch_completed rows=0 dur=1037us
drop index _Document578_VT668_1NG on dbo._Document578_VT668NG;

-- 03:26:45.067000 sid=145 sql_batch_completed rows=0 dur=2443us
drop index _Document578_VT668_SKNG on dbo._Document578_VT668NG;

-- 03:26:45.072000 sid=145 sql_batch_completed rows=10 dur=3980us
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

-- 03:26:45.075000 sid=145 rpc_completed rows=31 dur=2126us
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

-- 03:26:45.079000 sid=145 sql_batch_completed rows=10 dur=3441us
CREATE UNIQUE INDEX _Document578_1NG ON dbo._Document578NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.084000 sid=145 sql_batch_completed rows=10 dur=3643us
CREATE UNIQUE INDEX _Document578_2NG ON dbo._Document578NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.091000 sid=145 sql_batch_completed rows=30 dur=7217us
CREATE UNIQUE CLUSTERED INDEX _Document578_S_HPKNG ON dbo._Document578NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:45.098000 sid=145 sql_batch_completed rows=31 dur=6419us
CREATE UNIQUE CLUSTERED INDEX _Document578_VT668_SKNG ON dbo._Document578_VT668NG (_Fld2683, _Document578_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:45.107000 sid=145 sql_batch_completed rows=31 dur=5885us
CREATE INDEX _Document578_VT668_1NG ON dbo._Document578_VT668NG (_Fld2683, _Fld11039, _Document578_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.140000 sid=145 sql_batch_completed rows=0 dur=3575us
create table dbo._DbCopiesUpdatesNG (
_CopyId binary(16) not null,
_TrNum int,
_TrTime datetime2(0),
_UpdateId binary(16),
_LastUpdateResult numeric(2, 0),
_LastUpdateError varbinary(max)
)
;alter table dbo._DbCopiesUpdatesNG SET (LOCK_ESCALATION = DISABLE);

-- 03:26:45.144000 sid=145 sql_batch_completed rows=0 dur=3693us
CREATE UNIQUE CLUSTERED INDEX _DbCopiesUpdates_1NG ON dbo._DbCopiesUpdatesNG (_CopyId, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:45.145000 sid=145 rpc_completed rows=1 dur=314us
-- args: ,N'@P1 varbinary(max)',0x<9844 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:26:45.149000 sid=145 sql_batch_completed rows=0 dur=2491us
INSERT INTO dbo._DbCopiesUpdatesNG WITH(TABLOCK) (_CopyId, _TrNum, _TrTime) SELECT
T1._CopyId,
(T1._TrNum + 0.0),
T1._TrTime
FROM dbo._DbCopiesUpdates T1 WITH(NOLOCK);

-- 03:26:45.154000 sid=145 sql_batch_completed rows=0 dur=2440us
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

-- 03:26:45.156000 sid=145 sql_batch_completed rows=0 dur=2360us
CREATE UNIQUE CLUSTERED INDEX _DbCopies_1NG ON dbo._DbCopiesNG (_CopyId, _CopyName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:45.157000 sid=145 rpc_completed rows=1 dur=336us
-- args: ,N'@P1 varbinary(max)',0x<11372 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:26:45.161000 sid=145 rpc_completed rows=0 dur=1513us
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

-- 03:26:45.431000 sid=145 sql_batch_completed rows=0 dur=5237us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 03:26:45.435000 sid=145 sql_batch_completed rows=0 dur=3326us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 03:26:45.439000 sid=145 sql_batch_completed rows=0 dur=1134us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.440000 sid=145 sql_batch_completed rows=0 dur=912us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.444000 sid=145 sql_batch_completed rows=0 dur=2890us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:45.445000 sid=145 rpc_completed rows=1 dur=876us
-- args: ,N'@P1 varbinary(max)',0x<12436 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:26:45.456000 sid=145 sql_batch_completed rows=0 dur=3115us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 03:26:45.460000 sid=145 sql_batch_completed rows=0 dur=2108us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 03:26:45.464000 sid=145 sql_batch_completed rows=0 dur=2803us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 03:26:45.621000 sid=145 sql_batch_completed rows=10000 dur=45466us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 03:26:45.641000 sid=145 sql_batch_completed rows=9996 dur=19183us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=9996);

-- 03:26:45.788000 sid=145 sql_batch_completed rows=10000 dur=32402us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 03:26:45.834000 sid=145 sql_batch_completed rows=10000 dur=44156us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 03:26:45.838000 sid=145 sql_batch_completed rows=658 dur=3702us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 03:26:45.905000 sid=145 sql_batch_completed rows=685 dur=7432us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=685);

-- 03:26:45.913000 sid=145 sql_batch_completed rows=703 dur=6230us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=703);

-- 03:26:45.938000 sid=145 sql_batch_completed rows=20685 dur=24419us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.965000 sid=145 sql_batch_completed rows=20685 dur=26318us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:26:45.993000 sid=145 sql_batch_completed rows=21357 dur=27631us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:26:46.784000 sid=145 rpc_completed rows=1 dur=1500us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 03:26:46.821000 sid=145 rpc_completed rows=1 dur=901us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 03:26:47.539000 sid=145 rpc_completed rows=1 dur=130us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<888 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 03:26:47.568000 sid=145 rpc_completed rows=1 dur=185us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF00EFAB36899F1118F4F00E04C680093,0x<7728 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 03:27:27.779000 sid=145 sql_batch_completed rows=1 dur=140775us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 03:27:28.880000 sid=145 sql_batch_completed rows=0 dur=1024973us
drop table dbo._Reference14;

-- 03:27:29.122000 sid=145 sql_batch_completed rows=0 dur=238576us
drop table dbo._Reference14_VT67;

-- 03:27:29.942000 sid=145 sql_batch_completed rows=0 dur=751194us
drop table dbo._Reference21;

-- 03:27:30.144000 sid=145 sql_batch_completed rows=0 dur=199189us
drop table dbo._Reference21_VT960;

-- 03:27:31.853000 sid=145 sql_batch_completed rows=0 dur=1700568us
drop table dbo._Document578;

-- 03:27:32.103000 sid=145 sql_batch_completed rows=0 dur=114238us
drop table dbo._Document578_VT668;

-- 03:27:32.430000 sid=145 sql_batch_completed rows=0 dur=322105us
drop table dbo._DbCopiesUpdates;

-- 03:27:32.743000 sid=145 sql_batch_completed rows=0 dur=258474us
drop table dbo._DbCopies;

-- 03:27:33.524000 sid=145 sql_batch_completed rows=0 dur=774856us
drop table dbo._ConfigChngR;

-- 03:27:33.715000 sid=145 sql_batch_completed rows=0 dur=190435us
drop table dbo._ConfigChngR_ExtProps;

-- 03:27:33.756000 sid=145 sql_batch_completed rows=1 dur=40097us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 03:27:35.223000 sid=145 rpc_completed rows=34 dur=1461535us
-- args: N'_Reference14NG',N'_Reference14'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:35.314000 sid=145 rpc_completed rows=25 dur=77680us
-- args: N'_Reference14_VT67NG',N'_Reference14_VT67'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:35.376000 sid=145 rpc_completed rows=16 dur=23137us
-- args: N'_Reference14._Reference14_1NG',N'_Reference14_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.450000 sid=145 rpc_completed rows=16 dur=15530us
-- args: N'_Reference14._Reference14_2NG',N'_Reference14_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.473000 sid=145 rpc_completed rows=16 dur=12033us
-- args: N'_Reference14._Reference14_3NG',N'_Reference14_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.499000 sid=145 rpc_completed rows=16 dur=7341us
-- args: N'_Reference14._Reference14_S_HPKNG',N'_Reference14_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.547000 sid=145 rpc_completed rows=16 dur=7777us
-- args: N'_Reference14_VT67._Reference14_VT67_SKNG',N'_Reference14_VT67_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.596000 sid=145 rpc_completed rows=16 dur=5518us
-- args: N'_Reference14_VT67._Reference14_VT67_1NG',N'_Reference14_VT67_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.628000 sid=145 rpc_completed rows=16 dur=8626us
-- args: N'_Reference14_VT67._Reference14_VT67_2NG',N'_Reference14_VT67_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.682000 sid=145 rpc_completed rows=16 dur=9788us
-- args: N'_Reference14_VT67._Reference14_VT67_3NG',N'_Reference14_VT67_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.726000 sid=145 rpc_completed rows=16 dur=6005us
-- args: N'_Reference14_VT67._Reference14_VT67_4NG',N'_Reference14_VT67_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.813000 sid=145 rpc_completed rows=25 dur=31237us
-- args: N'_Reference21NG',N'_Reference21'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:35.860000 sid=145 rpc_completed rows=25 dur=18920us
-- args: N'_Reference21_VT960NG',N'_Reference21_VT960'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:35.911000 sid=145 rpc_completed rows=16 dur=12463us
-- args: N'_Reference21._Reference21_1NG',N'_Reference21_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.946000 sid=145 rpc_completed rows=16 dur=7793us
-- args: N'_Reference21._Reference21_2NG',N'_Reference21_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.988000 sid=145 rpc_completed rows=16 dur=14195us
-- args: N'_Reference21._Reference21_3NG',N'_Reference21_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:35.994000 sid=145 rpc_completed rows=16 dur=1946us
-- args: N'_Reference21._Reference21_4NG',N'_Reference21_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.018000 sid=145 rpc_completed rows=16 dur=6711us
-- args: N'_Reference21._Reference21_5NG',N'_Reference21_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.066000 sid=145 rpc_completed rows=16 dur=1696us
-- args: N'_Reference21._Reference21_S_HPKNG',N'_Reference21_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.113000 sid=145 rpc_completed rows=16 dur=5293us
-- args: N'_Reference21_VT960._Reference21_VT960_SKNG',N'_Reference21_VT960_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.117000 sid=145 rpc_completed rows=25 dur=2848us
-- args: N'_Document578NG',N'_Document578'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:36.146000 sid=145 rpc_completed rows=25 dur=11175us
-- args: N'_Document578_VT668NG',N'_Document578_VT668'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:36.174000 sid=145 rpc_completed rows=16 dur=8132us
-- args: N'_Document578._Document578_1NG',N'_Document578_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.205000 sid=145 rpc_completed rows=16 dur=9015us
-- args: N'_Document578._Document578_2NG',N'_Document578_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.218000 sid=145 rpc_completed rows=16 dur=1813us
-- args: N'_Document578._Document578_S_HPKNG',N'_Document578_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.224000 sid=145 rpc_completed rows=16 dur=1989us
-- args: N'_Document578_VT668._Document578_VT668_SKNG',N'_Document578_VT668_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.232000 sid=145 rpc_completed rows=16 dur=2105us
-- args: N'_Document578_VT668._Document578_VT668_1NG',N'_Document578_VT668_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.243000 sid=145 rpc_completed rows=25 dur=4705us
-- args: N'_DbCopiesUpdatesNG',N'_DbCopiesUpdates'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:36.251000 sid=145 rpc_completed rows=16 dur=4691us
-- args: N'_DbCopiesUpdates._DbCopiesUpdates_1NG',N'_DbCopiesUpdates_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.260000 sid=145 rpc_completed rows=25 dur=5162us
-- args: N'_DbCopiesNG',N'_DbCopies'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:36.264000 sid=145 rpc_completed rows=16 dur=1216us
-- args: N'_DbCopies._DbCopies_1NG',N'_DbCopies_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.272000 sid=145 rpc_completed rows=25 dur=5689us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:36.280000 sid=145 rpc_completed rows=25 dur=3629us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:27:36.284000 sid=145 rpc_completed rows=16 dur=979us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.286000 sid=145 rpc_completed rows=16 dur=1352us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.293000 sid=145 rpc_completed rows=16 dur=1253us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:27:36.717000 sid=145 sql_batch_completed rows=0 dur=12775us
ALTER INDEX PK___Referen__AC8ED0C4077A8F3B ON _Reference10877X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.729000 sid=145 sql_batch_completed rows=0 dur=10965us
ALTER INDEX PK___Referen__AC8ED0C4DBBB6986 ON _Reference10878X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.733000 sid=145 sql_batch_completed rows=0 dur=3416us
ALTER INDEX PK___Referen__AC8ED0C4720747AF ON _Reference10825X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.742000 sid=145 sql_batch_completed rows=0 dur=9130us
ALTER INDEX PK___Enum108__AC8ED0C45E875D60 ON _Enum10826X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.746000 sid=145 sql_batch_completed rows=0 dur=2586us
ALTER INDEX PK___Enum110__AC8ED0C4A5E2EE79 ON _Enum11017X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.758000 sid=145 sql_batch_completed rows=0 dur=5779us
ALTER INDEX PK___Documen__AC8ED0C48EB57390 ON _Document10069X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.778000 sid=145 sql_batch_completed rows=0 dur=19783us
ALTER INDEX PK___Referen__AC8ED0C43F77AD82 ON _Reference2596X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.794000 sid=145 sql_batch_completed rows=0 dur=7953us
ALTER INDEX PK___Referen__AC8ED0C45FCE1C7A ON _Reference10523X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.795000 sid=145 sql_batch_completed rows=0 dur=473us
ALTER INDEX PK___ConfigC__AC8ED0C4A8EEF6E2 ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:27:36.881000 sid=145 rpc_completed rows=1 dur=36137us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1956388 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 03:27:37.089000 sid=145 rpc_completed rows=1 dur=170708us
-- args: ,N'@P1 varbinary(max)',0x<1956388 hex>
UPDATE DBSchema SET SerializedData = @P1;

