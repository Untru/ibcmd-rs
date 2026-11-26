-- 03:25:14.927000 sid=142 sql_batch_completed rows=0 dur=11526us
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

-- 03:25:14.933000 sid=142 sql_batch_completed rows=0 dur=3585us
create table dbo._Reference14_VT67NG (
_Reference14_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo68 numeric(5, 0) not null,
_Fld69RRef binary(16) not null,
_Fld70RRef binary(16) not null,
_Fld71 nvarchar(500) not null,
_Fld72 nvarchar(max) not null,
_Fld73 nvarchar(100) not null,
_Fld74 nvarchar(50) not null,
_Fld75 nvarchar(50) not null,
_Fld76 nvarchar(100) not null,
_Fld77 nvarchar(100) not null,
_Fld78 nvarchar(20) not null,
_Fld79 nvarchar(20) not null,
_Fld6750 nvarchar(max) not null
)
;alter table dbo._Reference14_VT67NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:14.938000 sid=142 sql_batch_completed rows=0 dur=2663us
create table dbo._Reference14_VT11034NG (
_Reference14_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11035 numeric(5, 0) not null,
_Fld11036 nvarchar(10) not null
)
;alter table dbo._Reference14_VT11034NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.024000 sid=142 sql_batch_completed rows=0 dur=2812us
CREATE INDEX _Reference14_1NG ON dbo._Reference14NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.026000 sid=142 sql_batch_completed rows=0 dur=1521us
CREATE UNIQUE INDEX _Reference14_2NG ON dbo._Reference14NG (_Fld2683, _OwnerIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.028000 sid=142 sql_batch_completed rows=0 dur=1119us
CREATE UNIQUE INDEX _Reference14_3NG ON dbo._Reference14NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.047000 sid=142 sql_batch_completed rows=0 dur=19261us
CREATE UNIQUE CLUSTERED INDEX _Reference14_S_HPKNG ON dbo._Reference14NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.061000 sid=142 sql_batch_completed rows=0 dur=13862us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT67_SKNG ON dbo._Reference14_VT67NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.066000 sid=142 sql_batch_completed rows=0 dur=843us
CREATE INDEX _Reference14_VT67_1NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld69RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.067000 sid=142 sql_batch_completed rows=0 dur=556us
CREATE INDEX _Reference14_VT67_2NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld70RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.069000 sid=142 sql_batch_completed rows=0 dur=1715us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT11034_SKNG ON dbo._Reference14_VT11034NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.073000 sid=142 rpc_completed rows=1 dur=2098us
-- args: ,N'@P1 varbinary(max)',0x<4110 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:25:15.081000 sid=142 sql_batch_completed rows=0 dur=1249us
drop index _Reference14_1NG on dbo._Reference14NG;

-- 03:25:15.084000 sid=142 sql_batch_completed rows=0 dur=2310us
drop index _Reference14_2NG on dbo._Reference14NG;

-- 03:25:15.092000 sid=142 sql_batch_completed rows=0 dur=6268us
drop index _Reference14_3NG on dbo._Reference14NG;

-- 03:25:15.104000 sid=142 sql_batch_completed rows=0 dur=9986us
drop index _Reference14_S_HPKNG on dbo._Reference14NG;

-- 03:25:15.118000 sid=142 sql_batch_completed rows=0 dur=6664us
drop index _Reference14_VT67_1NG on dbo._Reference14_VT67NG;

-- 03:25:15.136000 sid=142 sql_batch_completed rows=0 dur=9713us
drop index _Reference14_VT67_2NG on dbo._Reference14_VT67NG;

-- 03:25:15.142000 sid=142 sql_batch_completed rows=0 dur=5207us
drop index _Reference14_VT67_SKNG on dbo._Reference14_VT67NG;

-- 03:25:15.162000 sid=142 sql_batch_completed rows=0 dur=6653us
drop index _Reference14_VT11034_SKNG on dbo._Reference14_VT11034NG;

-- 03:25:15.173000 sid=142 sql_batch_completed rows=3 dur=9616us
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

-- 03:25:15.176000 sid=142 sql_batch_completed rows=4 dur=2987us
INSERT INTO dbo._Reference14_VT67NG WITH(TABLOCK) (_LineNo68, _Fld69RRef, _Fld70RRef, _Fld71, _Fld72, _Fld73, _Fld74, _Fld75, _Fld76, _Fld77, _Fld78, _Fld79, _Fld6750, _Fld2683, _Reference14_IDRRef, _KeyField) SELECT
T2._LineNo68,
T2._Fld69RRef,
T2._Fld70RRef,
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
T2._Fld2683,
T2._Reference14_IDRRef,
T2._KeyField
FROM dbo._Reference14_VT67 T2 WITH(NOLOCK);

-- 03:25:15.183000 sid=142 sql_batch_completed rows=3 dur=4861us
CREATE INDEX _Reference14_1NG ON dbo._Reference14NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.187000 sid=142 sql_batch_completed rows=3 dur=3183us
CREATE UNIQUE INDEX _Reference14_2NG ON dbo._Reference14NG (_Fld2683, _OwnerIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.190000 sid=142 sql_batch_completed rows=3 dur=2354us
CREATE UNIQUE INDEX _Reference14_3NG ON dbo._Reference14NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.201000 sid=142 sql_batch_completed rows=12 dur=11402us
CREATE UNIQUE CLUSTERED INDEX _Reference14_S_HPKNG ON dbo._Reference14NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.206000 sid=142 sql_batch_completed rows=4 dur=4712us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT67_SKNG ON dbo._Reference14_VT67NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.210000 sid=142 sql_batch_completed rows=4 dur=2736us
CREATE INDEX _Reference14_VT67_1NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld69RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.214000 sid=142 sql_batch_completed rows=4 dur=2351us
CREATE INDEX _Reference14_VT67_2NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld70RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.215000 sid=142 sql_batch_completed rows=0 dur=1392us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT11034_SKNG ON dbo._Reference14_VT11034NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.234000 sid=142 sql_batch_completed rows=0 dur=3979us
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

-- 03:25:15.238000 sid=142 sql_batch_completed rows=0 dur=2179us
create table dbo._Reference21_VT960NG (
_Reference21_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo961 numeric(5, 0) not null,
_Fld962RRef binary(16) not null
)
;alter table dbo._Reference21_VT960NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.244000 sid=142 sql_batch_completed rows=0 dur=4755us
create table dbo._Reference21_VT11037NG (
_Reference21_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11038 numeric(5, 0) not null,
_Fld11039 datetime2(0) not null,
_Fld11040 nvarchar(50) not null
)
;alter table dbo._Reference21_VT11037NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.247000 sid=142 sql_batch_completed rows=0 dur=1530us
CREATE INDEX _Reference21_1NG ON dbo._Reference21NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.250000 sid=142 sql_batch_completed rows=0 dur=1793us
CREATE UNIQUE INDEX _Reference21_2NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.252000 sid=142 sql_batch_completed rows=0 dur=1497us
CREATE UNIQUE INDEX _Reference21_3NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.253000 sid=142 sql_batch_completed rows=0 dur=856us
CREATE UNIQUE INDEX _Reference21_4NG ON dbo._Reference21NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.265000 sid=142 sql_batch_completed rows=0 dur=3515us
CREATE UNIQUE INDEX _Reference21_5NG ON dbo._Reference21NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.269000 sid=142 sql_batch_completed rows=0 dur=3258us
CREATE UNIQUE CLUSTERED INDEX _Reference21_S_HPKNG ON dbo._Reference21NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.271000 sid=142 sql_batch_completed rows=0 dur=1861us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT960_SKNG ON dbo._Reference21_VT960NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.274000 sid=142 sql_batch_completed rows=0 dur=2558us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT11037_SKNG ON dbo._Reference21_VT11037NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.276000 sid=142 sql_batch_completed rows=0 dur=1039us
CREATE INDEX _Reference21_VT11037_1NG ON dbo._Reference21_VT11037NG (_Fld2683, _Fld11040, _Reference21_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.276000 sid=142 rpc_completed rows=1 dur=253us
-- args: ,N'@P1 varbinary(max)',0x<6856 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:25:15.282000 sid=142 sql_batch_completed rows=0 dur=1078us
drop index _Reference21_1NG on dbo._Reference21NG;

-- 03:25:15.284000 sid=142 sql_batch_completed rows=0 dur=853us
drop index _Reference21_2NG on dbo._Reference21NG;

-- 03:25:15.285000 sid=142 sql_batch_completed rows=0 dur=1021us
drop index _Reference21_3NG on dbo._Reference21NG;

-- 03:25:15.286000 sid=142 sql_batch_completed rows=0 dur=756us
drop index _Reference21_4NG on dbo._Reference21NG;

-- 03:25:15.288000 sid=142 sql_batch_completed rows=0 dur=802us
drop index _Reference21_5NG on dbo._Reference21NG;

-- 03:25:15.290000 sid=142 sql_batch_completed rows=0 dur=1202us
drop index _Reference21_S_HPKNG on dbo._Reference21NG;

-- 03:25:15.292000 sid=142 sql_batch_completed rows=0 dur=1365us
drop index _Reference21_VT960_SKNG on dbo._Reference21_VT960NG;

-- 03:25:15.295000 sid=142 sql_batch_completed rows=0 dur=1173us
drop index _Reference21_VT11037_1NG on dbo._Reference21_VT11037NG;

-- 03:25:15.298000 sid=142 sql_batch_completed rows=0 dur=2372us
drop index _Reference21_VT11037_SKNG on dbo._Reference21_VT11037NG;

-- 03:25:15.306000 sid=142 sql_batch_completed rows=4 dur=7037us
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

-- 03:25:15.310000 sid=142 sql_batch_completed rows=4 dur=3635us
INSERT INTO dbo._Reference21_VT960NG WITH(TABLOCK) (_LineNo961, _Fld962RRef, _Fld2683, _Reference21_IDRRef, _KeyField) SELECT
T2._LineNo961,
T2._Fld962RRef,
T2._Fld2683,
T2._Reference21_IDRRef,
T2._KeyField
FROM dbo._Reference21_VT960 T2 WITH(NOLOCK);

-- 03:25:15.314000 sid=142 sql_batch_completed rows=4 dur=2661us
CREATE INDEX _Reference21_1NG ON dbo._Reference21NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.316000 sid=142 sql_batch_completed rows=4 dur=1979us
CREATE UNIQUE INDEX _Reference21_2NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.320000 sid=142 sql_batch_completed rows=4 dur=3080us
CREATE UNIQUE INDEX _Reference21_3NG ON dbo._Reference21NG (_Fld2683, _ParentIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.323000 sid=142 sql_batch_completed rows=4 dur=2284us
CREATE UNIQUE INDEX _Reference21_4NG ON dbo._Reference21NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.325000 sid=142 sql_batch_completed rows=4 dur=1684us
CREATE UNIQUE INDEX _Reference21_5NG ON dbo._Reference21NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.335000 sid=142 sql_batch_completed rows=24 dur=9371us
CREATE UNIQUE CLUSTERED INDEX _Reference21_S_HPKNG ON dbo._Reference21NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.338000 sid=142 sql_batch_completed rows=4 dur=2689us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT960_SKNG ON dbo._Reference21_VT960NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.340000 sid=142 sql_batch_completed rows=0 dur=1774us
CREATE UNIQUE CLUSTERED INDEX _Reference21_VT11037_SKNG ON dbo._Reference21_VT11037NG (_Fld2683, _Reference21_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.341000 sid=142 sql_batch_completed rows=0 dur=681us
CREATE INDEX _Reference21_VT11037_1NG ON dbo._Reference21_VT11037NG (_Fld2683, _Fld11040, _Reference21_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.351000 sid=142 sql_batch_completed rows=0 dur=2488us
create table dbo._Reference23NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_Code nvarchar(9) not null,
_Description nvarchar(50) not null,
_Fld181 numeric(5, 2) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference23NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.355000 sid=142 sql_batch_completed rows=0 dur=2266us
create table dbo._Reference23_VT11041NG (
_Reference23_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11042 numeric(5, 0) not null,
_Fld11043 nvarchar(20) not null,
_Fld11044 numeric(10, 2) not null,
_Fld11045 binary(1) not null,
_Fld11046 nvarchar(max) not null
)
;alter table dbo._Reference23_VT11041NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.359000 sid=142 sql_batch_completed rows=0 dur=2756us
CREATE INDEX _Reference23_1NG ON dbo._Reference23NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.360000 sid=142 sql_batch_completed rows=0 dur=905us
CREATE UNIQUE INDEX _Reference23_2NG ON dbo._Reference23NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.361000 sid=142 sql_batch_completed rows=0 dur=711us
CREATE UNIQUE INDEX _Reference23_3NG ON dbo._Reference23NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.364000 sid=142 sql_batch_completed rows=0 dur=2412us
CREATE UNIQUE CLUSTERED INDEX _Reference23_S_HPKNG ON dbo._Reference23NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.367000 sid=142 sql_batch_completed rows=0 dur=3022us
CREATE UNIQUE CLUSTERED INDEX _Reference23_VT11041_SKNG ON dbo._Reference23_VT11041NG (_Fld2683, _Reference23_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.369000 sid=142 sql_batch_completed rows=0 dur=841us
CREATE INDEX _Reference23_VT11041_1NG ON dbo._Reference23_VT11041NG (_Fld2683, _Fld11044, _Reference23_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.370000 sid=142 rpc_completed rows=1 dur=873us
-- args: ,N'@P1 varbinary(max)',0x<9038 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:25:15.375000 sid=142 sql_batch_completed rows=0 dur=1357us
drop index _Reference23_1NG on dbo._Reference23NG;

-- 03:25:15.377000 sid=142 sql_batch_completed rows=0 dur=1098us
drop index _Reference23_2NG on dbo._Reference23NG;

-- 03:25:15.378000 sid=142 sql_batch_completed rows=0 dur=895us
drop index _Reference23_3NG on dbo._Reference23NG;

-- 03:25:15.379000 sid=142 sql_batch_completed rows=0 dur=1022us
drop index _Reference23_S_HPKNG on dbo._Reference23NG;

-- 03:25:15.382000 sid=142 sql_batch_completed rows=0 dur=1184us
drop index _Reference23_VT11041_1NG on dbo._Reference23_VT11041NG;

-- 03:25:15.384000 sid=142 sql_batch_completed rows=0 dur=881us
drop index _Reference23_VT11041_SKNG on dbo._Reference23_VT11041NG;

-- 03:25:15.387000 sid=142 sql_batch_completed rows=2 dur=2951us
INSERT INTO dbo._Reference23NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _Code, _Description, _Fld181, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._Code,
T1._Description,
T1._Fld181,
T1._Fld2683
FROM dbo._Reference23 T1 WITH(NOLOCK);

-- 03:25:15.393000 sid=142 sql_batch_completed rows=2 dur=4534us
CREATE INDEX _Reference23_1NG ON dbo._Reference23NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.397000 sid=142 sql_batch_completed rows=2 dur=3460us
CREATE UNIQUE INDEX _Reference23_2NG ON dbo._Reference23NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.401000 sid=142 sql_batch_completed rows=2 dur=3046us
CREATE UNIQUE INDEX _Reference23_3NG ON dbo._Reference23NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.409000 sid=142 sql_batch_completed rows=8 dur=7886us
CREATE UNIQUE CLUSTERED INDEX _Reference23_S_HPKNG ON dbo._Reference23NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.412000 sid=142 sql_batch_completed rows=0 dur=1931us
CREATE UNIQUE CLUSTERED INDEX _Reference23_VT11041_SKNG ON dbo._Reference23_VT11041NG (_Fld2683, _Reference23_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.415000 sid=142 sql_batch_completed rows=0 dur=1501us
CREATE INDEX _Reference23_VT11041_1NG ON dbo._Reference23_VT11041NG (_Fld2683, _Fld11044, _Reference23_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.790000 sid=142 sql_batch_completed rows=0 dur=2448us
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

-- 03:25:15.794000 sid=142 sql_batch_completed rows=0 dur=1876us
create table dbo._Document578_VT668NG (
_Document578_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo669 numeric(5, 0) not null,
_Fld670RRef binary(16) not null,
_Fld673 numeric(15, 3) not null
)
;alter table dbo._Document578_VT668NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.798000 sid=142 sql_batch_completed rows=0 dur=2207us
create table dbo._Document578_VT11047NG (
_Document578_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11048 numeric(5, 0) not null,
_Fld11049 binary(1) not null
)
;alter table dbo._Document578_VT11047NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.801000 sid=142 sql_batch_completed rows=0 dur=1502us
CREATE UNIQUE INDEX _Document578_1NG ON dbo._Document578NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.802000 sid=142 sql_batch_completed rows=0 dur=804us
CREATE UNIQUE INDEX _Document578_2NG ON dbo._Document578NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.804000 sid=142 sql_batch_completed rows=0 dur=1559us
CREATE UNIQUE CLUSTERED INDEX _Document578_S_HPKNG ON dbo._Document578NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.806000 sid=142 sql_batch_completed rows=0 dur=2288us
CREATE UNIQUE CLUSTERED INDEX _Document578_VT668_SKNG ON dbo._Document578_VT668NG (_Fld2683, _Document578_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.808000 sid=142 sql_batch_completed rows=0 dur=1704us
CREATE UNIQUE CLUSTERED INDEX _Document578_VT11047_SKNG ON dbo._Document578_VT11047NG (_Fld2683, _Document578_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.809000 sid=142 rpc_completed rows=1 dur=289us
-- args: ,N'@P1 varbinary(max)',0x<11454 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:25:15.813000 sid=142 sql_batch_completed rows=0 dur=727us
drop index _Document578_1NG on dbo._Document578NG;

-- 03:25:15.814000 sid=142 sql_batch_completed rows=0 dur=599us
drop index _Document578_2NG on dbo._Document578NG;

-- 03:25:15.815000 sid=142 sql_batch_completed rows=0 dur=757us
drop index _Document578_S_HPKNG on dbo._Document578NG;

-- 03:25:15.817000 sid=142 sql_batch_completed rows=0 dur=1121us
drop index _Document578_VT668_SKNG on dbo._Document578_VT668NG;

-- 03:25:15.818000 sid=142 sql_batch_completed rows=0 dur=833us
drop index _Document578_VT11047_SKNG on dbo._Document578_VT11047NG;

-- 03:25:15.823000 sid=142 sql_batch_completed rows=10 dur=4103us
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

-- 03:25:15.825000 sid=142 sql_batch_completed rows=31 dur=1816us
INSERT INTO dbo._Document578_VT668NG WITH(TABLOCK) (_LineNo669, _Fld670RRef, _Fld673, _Fld2683, _Document578_IDRRef, _KeyField) SELECT
T2._LineNo669,
T2._Fld670RRef,
T2._Fld673,
T2._Fld2683,
T2._Document578_IDRRef,
T2._KeyField
FROM dbo._Document578_VT668 T2 WITH(NOLOCK);

-- 03:25:15.828000 sid=142 sql_batch_completed rows=10 dur=1790us
CREATE UNIQUE INDEX _Document578_1NG ON dbo._Document578NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.830000 sid=142 sql_batch_completed rows=10 dur=1745us
CREATE UNIQUE INDEX _Document578_2NG ON dbo._Document578NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.836000 sid=142 sql_batch_completed rows=30 dur=5840us
CREATE UNIQUE CLUSTERED INDEX _Document578_S_HPKNG ON dbo._Document578NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.838000 sid=142 sql_batch_completed rows=31 dur=1918us
CREATE UNIQUE CLUSTERED INDEX _Document578_VT668_SKNG ON dbo._Document578_VT668NG (_Fld2683, _Document578_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.839000 sid=142 sql_batch_completed rows=0 dur=920us
CREATE UNIQUE CLUSTERED INDEX _Document578_VT11047_SKNG ON dbo._Document578_VT11047NG (_Fld2683, _Document578_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.854000 sid=142 sql_batch_completed rows=0 dur=4273us
create table dbo._Document4883NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_Date_Time datetime2(0) not null,
_Number nvarchar(9) not null,
_Posted binary(1) not null,
_Fld4905RRef binary(16) not null,
_Fld4906RRef binary(16) not null,
_Fld4907RRef binary(16) not null,
_Fld4908 numeric(15, 2) not null,
_Fld5555 nvarchar(max) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Document4883NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.857000 sid=142 sql_batch_completed rows=0 dur=2020us
create table dbo._Document4883_VT11050NG (
_Document4883_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11051 numeric(5, 0) not null,
_Fld11052 nvarchar(15) not null,
_Fld11053 numeric(15, 2) not null
)
;alter table dbo._Document4883_VT11050NG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.859000 sid=142 sql_batch_completed rows=0 dur=1258us
CREATE UNIQUE INDEX _Document4883_1NG ON dbo._Document4883NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.860000 sid=142 sql_batch_completed rows=0 dur=735us
CREATE UNIQUE INDEX _Document4883_2NG ON dbo._Document4883NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.864000 sid=142 sql_batch_completed rows=0 dur=3549us
CREATE UNIQUE CLUSTERED INDEX _Document4883_S_HPKNG ON dbo._Document4883NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.866000 sid=142 sql_batch_completed rows=0 dur=1482us
CREATE UNIQUE CLUSTERED INDEX _Document4883_VT11050_SKNG ON dbo._Document4883_VT11050NG (_Fld2683, _Document4883_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.867000 sid=142 rpc_completed rows=1 dur=919us
-- args: ,N'@P1 varbinary(max)',0x<13638 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:25:15.871000 sid=142 sql_batch_completed rows=0 dur=688us
drop index _Document4883_1NG on dbo._Document4883NG;

-- 03:25:15.872000 sid=142 sql_batch_completed rows=0 dur=593us
drop index _Document4883_2NG on dbo._Document4883NG;

-- 03:25:15.873000 sid=142 sql_batch_completed rows=0 dur=947us
drop index _Document4883_S_HPKNG on dbo._Document4883NG;

-- 03:25:15.875000 sid=142 sql_batch_completed rows=0 dur=804us
drop index _Document4883_VT11050_SKNG on dbo._Document4883_VT11050NG;

-- 03:25:15.881000 sid=142 sql_batch_completed rows=7 dur=5622us
INSERT INTO dbo._Document4883NG WITH(TABLOCK) (_IDRRef, _Marked, _Date_Time, _Number, _Posted, _Fld4905RRef, _Fld4906RRef, _Fld4907RRef, _Fld4908, _Fld5555, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._Date_Time,
T1._Number,
T1._Posted,
T1._Fld4905RRef,
T1._Fld4906RRef,
T1._Fld4907RRef,
T1._Fld4908,
T1._Fld5555,
T1._Fld2683
FROM dbo._Document4883 T1 WITH(NOLOCK);

-- 03:25:15.884000 sid=142 sql_batch_completed rows=7 dur=2266us
CREATE UNIQUE INDEX _Document4883_1NG ON dbo._Document4883NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.886000 sid=142 sql_batch_completed rows=7 dur=1571us
CREATE UNIQUE INDEX _Document4883_2NG ON dbo._Document4883NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:15.892000 sid=142 sql_batch_completed rows=21 dur=5781us
CREATE UNIQUE CLUSTERED INDEX _Document4883_S_HPKNG ON dbo._Document4883NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.894000 sid=142 sql_batch_completed rows=0 dur=1362us
CREATE UNIQUE CLUSTERED INDEX _Document4883_VT11050_SKNG ON dbo._Document4883_VT11050NG (_Fld2683, _Document4883_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.917000 sid=142 sql_batch_completed rows=0 dur=3213us
create table dbo._DbCopiesUpdatesNG (
_CopyId binary(16) not null,
_TrNum int,
_TrTime datetime2(0),
_UpdateId binary(16),
_LastUpdateResult numeric(2, 0),
_LastUpdateError varbinary(max)
)
;alter table dbo._DbCopiesUpdatesNG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:15.920000 sid=142 sql_batch_completed rows=0 dur=2319us
CREATE UNIQUE CLUSTERED INDEX _DbCopiesUpdates_1NG ON dbo._DbCopiesUpdatesNG (_CopyId, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.921000 sid=142 rpc_completed rows=1 dur=1043us
-- args: ,N'@P1 varbinary(max)',0x<14526 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:25:15.926000 sid=142 sql_batch_completed rows=0 dur=3174us
INSERT INTO dbo._DbCopiesUpdatesNG WITH(TABLOCK) (_CopyId, _TrNum, _TrTime) SELECT
T1._CopyId,
(T1._TrNum + 0.0),
T1._TrTime
FROM dbo._DbCopiesUpdates T1 WITH(NOLOCK);

-- 03:25:15.931000 sid=142 sql_batch_completed rows=0 dur=2315us
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

-- 03:25:15.932000 sid=142 sql_batch_completed rows=0 dur=1510us
CREATE UNIQUE CLUSTERED INDEX _DbCopies_1NG ON dbo._DbCopiesNG (_CopyId, _CopyName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:15.934000 sid=142 rpc_completed rows=1 dur=1399us
-- args: ,N'@P1 varbinary(max)',0x<16054 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:25:15.937000 sid=142 rpc_completed rows=0 dur=1388us
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

-- 03:25:16.151000 sid=142 sql_batch_completed rows=0 dur=3760us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:16.154000 sid=142 sql_batch_completed rows=0 dur=2161us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 03:25:16.155000 sid=142 sql_batch_completed rows=0 dur=980us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:16.157000 sid=142 sql_batch_completed rows=0 dur=738us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:16.159000 sid=142 sql_batch_completed rows=0 dur=2101us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:16.160000 sid=142 rpc_completed rows=1 dur=487us
-- args: ,N'@P1 varbinary(max)',0x<17118 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 03:25:16.164000 sid=142 sql_batch_completed rows=0 dur=982us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 03:25:16.165000 sid=142 sql_batch_completed rows=0 dur=764us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 03:25:16.167000 sid=142 sql_batch_completed rows=0 dur=758us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 03:25:16.302000 sid=142 sql_batch_completed rows=10000 dur=26554us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 03:25:16.316000 sid=142 sql_batch_completed rows=9996 dur=13542us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=9996);

-- 03:25:16.446000 sid=142 sql_batch_completed rows=10000 dur=21806us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 03:25:16.461000 sid=142 sql_batch_completed rows=10000 dur=14688us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 03:25:16.465000 sid=142 sql_batch_completed rows=658 dur=3284us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 03:25:16.532000 sid=142 sql_batch_completed rows=685 dur=3392us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=685);

-- 03:25:16.537000 sid=142 sql_batch_completed rows=703 dur=3808us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=703);

-- 03:25:16.557000 sid=142 sql_batch_completed rows=20685 dur=18638us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:16.574000 sid=142 sql_batch_completed rows=20685 dur=15921us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 03:25:16.595000 sid=142 sql_batch_completed rows=21357 dur=20990us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:17.231000 sid=142 rpc_completed rows=1 dur=1224us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 03:25:17.269000 sid=142 rpc_completed rows=1 dur=912us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 03:25:17.889000 sid=142 rpc_completed rows=1 dur=285us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<888 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 03:25:17.917000 sid=142 rpc_completed rows=1 dur=184us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF00EFAB36899F1118F4F00E04C680093,0x<7728 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 03:25:42.349000 sid=142 sql_batch_completed rows=1 dur=997us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 03:25:42.359000 sid=142 sql_batch_completed rows=0 dur=6814us
drop table dbo._Reference14;

-- 03:25:42.362000 sid=142 sql_batch_completed rows=0 dur=2348us
drop table dbo._Reference14_VT67;

-- 03:25:42.367000 sid=142 sql_batch_completed rows=0 dur=4049us
drop table dbo._Reference21;

-- 03:25:42.369000 sid=142 sql_batch_completed rows=0 dur=1610us
drop table dbo._Reference21_VT960;

-- 03:25:42.373000 sid=142 sql_batch_completed rows=0 dur=2592us
drop table dbo._Reference23;

-- 03:25:42.375000 sid=142 sql_batch_completed rows=0 dur=1773us
drop table dbo._Document578;

-- 03:25:42.378000 sid=142 sql_batch_completed rows=0 dur=1210us
drop table dbo._Document578_VT668;

-- 03:25:42.380000 sid=142 sql_batch_completed rows=0 dur=1405us
drop table dbo._Document4883;

-- 03:25:42.382000 sid=142 sql_batch_completed rows=0 dur=1552us
drop table dbo._DbCopiesUpdates;

-- 03:25:42.385000 sid=142 sql_batch_completed rows=0 dur=1606us
drop table dbo._DbCopies;

-- 03:25:42.388000 sid=142 sql_batch_completed rows=0 dur=3338us
drop table dbo._ConfigChngR;

-- 03:25:42.391000 sid=142 sql_batch_completed rows=0 dur=2178us
drop table dbo._ConfigChngR_ExtProps;

-- 03:25:42.392000 sid=142 sql_batch_completed rows=1 dur=355us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 03:25:42.683000 sid=142 rpc_completed rows=30 dur=289146us
-- args: N'_Reference14NG',N'_Reference14'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.686000 sid=142 rpc_completed rows=25 dur=1982us
-- args: N'_Reference14_VT67NG',N'_Reference14_VT67'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.689000 sid=142 rpc_completed rows=25 dur=2246us
-- args: N'_Reference14_VT11034NG',N'_Reference14_VT11034'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.692000 sid=142 rpc_completed rows=16 dur=990us
-- args: N'_Reference14._Reference14_1NG',N'_Reference14_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.693000 sid=142 rpc_completed rows=16 dur=591us
-- args: N'_Reference14._Reference14_2NG',N'_Reference14_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.695000 sid=142 rpc_completed rows=16 dur=472us
-- args: N'_Reference14._Reference14_3NG',N'_Reference14_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.696000 sid=142 rpc_completed rows=16 dur=470us
-- args: N'_Reference14._Reference14_S_HPKNG',N'_Reference14_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.697000 sid=142 rpc_completed rows=16 dur=480us
-- args: N'_Reference14_VT67._Reference14_VT67_SKNG',N'_Reference14_VT67_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.698000 sid=142 rpc_completed rows=16 dur=552us
-- args: N'_Reference14_VT67._Reference14_VT67_1NG',N'_Reference14_VT67_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.699000 sid=142 rpc_completed rows=16 dur=479us
-- args: N'_Reference14_VT67._Reference14_VT67_2NG',N'_Reference14_VT67_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.701000 sid=142 rpc_completed rows=16 dur=441us
-- args: N'_Reference14_VT11034._Reference14_VT11034_SKNG',N'_Reference14_VT11034_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.703000 sid=142 rpc_completed rows=25 dur=1605us
-- args: N'_Reference21NG',N'_Reference21'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.706000 sid=142 rpc_completed rows=25 dur=2099us
-- args: N'_Reference21_VT960NG',N'_Reference21_VT960'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.709000 sid=142 rpc_completed rows=25 dur=1604us
-- args: N'_Reference21_VT11037NG',N'_Reference21_VT11037'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.710000 sid=142 rpc_completed rows=16 dur=583us
-- args: N'_Reference21._Reference21_1NG',N'_Reference21_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.712000 sid=142 rpc_completed rows=16 dur=609us
-- args: N'_Reference21._Reference21_2NG',N'_Reference21_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.713000 sid=142 rpc_completed rows=16 dur=482us
-- args: N'_Reference21._Reference21_3NG',N'_Reference21_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.714000 sid=142 rpc_completed rows=16 dur=462us
-- args: N'_Reference21._Reference21_4NG',N'_Reference21_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.715000 sid=142 rpc_completed rows=16 dur=416us
-- args: N'_Reference21._Reference21_5NG',N'_Reference21_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.716000 sid=142 rpc_completed rows=16 dur=398us
-- args: N'_Reference21._Reference21_S_HPKNG',N'_Reference21_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.717000 sid=142 rpc_completed rows=16 dur=394us
-- args: N'_Reference21_VT960._Reference21_VT960_SKNG',N'_Reference21_VT960_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.718000 sid=142 rpc_completed rows=16 dur=440us
-- args: N'_Reference21_VT11037._Reference21_VT11037_SKNG',N'_Reference21_VT11037_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.720000 sid=142 rpc_completed rows=16 dur=639us
-- args: N'_Reference21_VT11037._Reference21_VT11037_1NG',N'_Reference21_VT11037_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.722000 sid=142 rpc_completed rows=25 dur=1852us
-- args: N'_Reference23NG',N'_Reference23'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.725000 sid=142 rpc_completed rows=25 dur=1445us
-- args: N'_Reference23_VT11041NG',N'_Reference23_VT11041'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.726000 sid=142 rpc_completed rows=16 dur=556us
-- args: N'_Reference23._Reference23_1NG',N'_Reference23_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.728000 sid=142 rpc_completed rows=16 dur=485us
-- args: N'_Reference23._Reference23_2NG',N'_Reference23_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.729000 sid=142 rpc_completed rows=16 dur=495us
-- args: N'_Reference23._Reference23_3NG',N'_Reference23_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.731000 sid=142 rpc_completed rows=16 dur=478us
-- args: N'_Reference23._Reference23_S_HPKNG',N'_Reference23_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.732000 sid=142 rpc_completed rows=16 dur=468us
-- args: N'_Reference23_VT11041._Reference23_VT11041_SKNG',N'_Reference23_VT11041_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.733000 sid=142 rpc_completed rows=16 dur=450us
-- args: N'_Reference23_VT11041._Reference23_VT11041_1NG',N'_Reference23_VT11041_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.736000 sid=142 rpc_completed rows=25 dur=1334us
-- args: N'_Document578NG',N'_Document578'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.739000 sid=142 rpc_completed rows=25 dur=2157us
-- args: N'_Document578_VT668NG',N'_Document578_VT668'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.741000 sid=142 rpc_completed rows=25 dur=1620us
-- args: N'_Document578_VT11047NG',N'_Document578_VT11047'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.743000 sid=142 rpc_completed rows=16 dur=606us
-- args: N'_Document578._Document578_1NG',N'_Document578_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.744000 sid=142 rpc_completed rows=16 dur=490us
-- args: N'_Document578._Document578_2NG',N'_Document578_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.746000 sid=142 rpc_completed rows=16 dur=525us
-- args: N'_Document578._Document578_S_HPKNG',N'_Document578_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.747000 sid=142 rpc_completed rows=16 dur=481us
-- args: N'_Document578_VT668._Document578_VT668_SKNG',N'_Document578_VT668_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.748000 sid=142 rpc_completed rows=16 dur=459us
-- args: N'_Document578_VT11047._Document578_VT11047_SKNG',N'_Document578_VT11047_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.750000 sid=142 rpc_completed rows=25 dur=899us
-- args: N'_Document4883NG',N'_Document4883'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.752000 sid=142 rpc_completed rows=25 dur=824us
-- args: N'_Document4883_VT11050NG',N'_Document4883_VT11050'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.753000 sid=142 rpc_completed rows=16 dur=624us
-- args: N'_Document4883._Document4883_1NG',N'_Document4883_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.755000 sid=142 rpc_completed rows=16 dur=508us
-- args: N'_Document4883._Document4883_2NG',N'_Document4883_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.756000 sid=142 rpc_completed rows=16 dur=457us
-- args: N'_Document4883._Document4883_S_HPKNG',N'_Document4883_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.758000 sid=142 rpc_completed rows=16 dur=463us
-- args: N'_Document4883_VT11050._Document4883_VT11050_SKNG',N'_Document4883_VT11050_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.759000 sid=142 rpc_completed rows=25 dur=849us
-- args: N'_DbCopiesUpdatesNG',N'_DbCopiesUpdates'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.760000 sid=142 rpc_completed rows=16 dur=530us
-- args: N'_DbCopiesUpdates._DbCopiesUpdates_1NG',N'_DbCopiesUpdates_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.762000 sid=142 rpc_completed rows=25 dur=992us
-- args: N'_DbCopiesNG',N'_DbCopies'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.764000 sid=142 rpc_completed rows=16 dur=548us
-- args: N'_DbCopies._DbCopies_1NG',N'_DbCopies_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.765000 sid=142 rpc_completed rows=25 dur=871us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.767000 sid=142 rpc_completed rows=25 dur=941us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 03:25:42.769000 sid=142 rpc_completed rows=16 dur=575us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.770000 sid=142 rpc_completed rows=16 dur=596us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:42.772000 sid=142 rpc_completed rows=16 dur=569us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 03:25:48.140000 sid=142 sql_batch_completed rows=0 dur=806us
ALTER INDEX PK___ConfigC__AC8ED0C43EF0DA0B ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.152000 sid=142 sql_batch_completed rows=0 dur=11187us
ALTER INDEX PK___Referen__AC8ED0C4077A8F3B ON _Reference10877X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.154000 sid=142 sql_batch_completed rows=0 dur=2167us
ALTER INDEX PK___Referen__AC8ED0C4DBBB6986 ON _Reference10878X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.156000 sid=142 sql_batch_completed rows=0 dur=1322us
ALTER INDEX PK___Referen__AC8ED0C4720747AF ON _Reference10825X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.157000 sid=142 sql_batch_completed rows=0 dur=955us
ALTER INDEX PK___Enum108__AC8ED0C45E875D60 ON _Enum10826X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.158000 sid=142 sql_batch_completed rows=0 dur=1439us
ALTER INDEX PK___Enum110__AC8ED0C4A5E2EE79 ON _Enum11017X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.162000 sid=142 sql_batch_completed rows=0 dur=3948us
ALTER INDEX PK___Documen__AC8ED0C48EB57390 ON _Document10069X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.167000 sid=142 sql_batch_completed rows=0 dur=4309us
ALTER INDEX PK___Referen__AC8ED0C43F77AD82 ON _Reference2596X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.168000 sid=142 sql_batch_completed rows=0 dur=953us
ALTER INDEX PK___Referen__AC8ED0C45FCE1C7A ON _Reference10523X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 03:25:48.202000 sid=142 rpc_completed rows=1 dur=16072us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1957980 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 03:25:48.351000 sid=142 rpc_completed rows=1 dur=127015us
-- args: ,N'@P1 varbinary(max)',0x<1957980 hex>
UPDATE DBSchema SET SerializedData = @P1;

