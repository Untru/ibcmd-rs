-- 00:56:54.164000 sid=257 sql_batch_completed rows=0 dur=54354us
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

-- 00:56:54.197000 sid=257 sql_batch_completed rows=0 dur=14782us
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

-- 00:56:54.212000 sid=257 sql_batch_completed rows=0 dur=10335us
create table dbo._Reference14_VT11034NG (
_Reference14_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11035 numeric(5, 0) not null,
_Fld11036 nvarchar(10) not null
)
;alter table dbo._Reference14_VT11034NG SET (LOCK_ESCALATION = DISABLE);

-- 00:56:54.330000 sid=257 sql_batch_completed rows=0 dur=14666us
CREATE INDEX _Reference14_1NG ON dbo._Reference14NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:54.335000 sid=257 sql_batch_completed rows=0 dur=3341us
CREATE UNIQUE INDEX _Reference14_2NG ON dbo._Reference14NG (_Fld2683, _OwnerIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:54.348000 sid=257 sql_batch_completed rows=0 dur=1893us
CREATE UNIQUE INDEX _Reference14_3NG ON dbo._Reference14NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:54.367000 sid=257 sql_batch_completed rows=0 dur=17435us
CREATE UNIQUE CLUSTERED INDEX _Reference14_S_HPKNG ON dbo._Reference14NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:54.383000 sid=257 sql_batch_completed rows=0 dur=14462us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT67_SKNG ON dbo._Reference14_VT67NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:54.390000 sid=257 sql_batch_completed rows=0 dur=5613us
CREATE INDEX _Reference14_VT67_1NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld69RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:54.398000 sid=257 sql_batch_completed rows=0 dur=1859us
CREATE INDEX _Reference14_VT67_2NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld70RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:54.407000 sid=257 sql_batch_completed rows=0 dur=8574us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT11034_SKNG ON dbo._Reference14_VT11034NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:54.419000 sid=257 rpc_completed rows=1 dur=3471us
-- args: ,N'@P1 varbinary(max)',0x<4110 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:56:54.446000 sid=257 sql_batch_completed rows=0 dur=6771us
drop index _Reference14_1NG on dbo._Reference14NG;

-- 00:56:54.455000 sid=257 sql_batch_completed rows=0 dur=1441us
drop index _Reference14_2NG on dbo._Reference14NG;

-- 00:56:54.460000 sid=257 sql_batch_completed rows=0 dur=4451us
drop index _Reference14_3NG on dbo._Reference14NG;

-- 00:56:54.469000 sid=257 sql_batch_completed rows=0 dur=2505us
drop index _Reference14_S_HPKNG on dbo._Reference14NG;

-- 00:56:54.480000 sid=257 sql_batch_completed rows=0 dur=7459us
drop index _Reference14_VT67_1NG on dbo._Reference14_VT67NG;

-- 00:56:54.525000 sid=257 sql_batch_completed rows=0 dur=4232us
drop index _Reference14_VT67_2NG on dbo._Reference14_VT67NG;

-- 00:56:54.534000 sid=257 sql_batch_completed rows=0 dur=4186us
drop index _Reference14_VT67_SKNG on dbo._Reference14_VT67NG;

-- 00:56:54.559000 sid=257 sql_batch_completed rows=0 dur=15944us
drop index _Reference14_VT11034_SKNG on dbo._Reference14_VT11034NG;

-- 00:56:54.597000 sid=257 sql_batch_completed rows=3 dur=36549us
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

-- 00:56:54.607000 sid=257 sql_batch_completed rows=4 dur=9901us
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

-- 00:56:55.279000 sid=257 sql_batch_completed rows=3 dur=668777us
CREATE INDEX _Reference14_1NG ON dbo._Reference14NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.317000 sid=257 sql_batch_completed rows=3 dur=18638us
CREATE UNIQUE INDEX _Reference14_2NG ON dbo._Reference14NG (_Fld2683, _OwnerIDRRef, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.358000 sid=257 sql_batch_completed rows=3 dur=17185us
CREATE UNIQUE INDEX _Reference14_3NG ON dbo._Reference14NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.498000 sid=257 sql_batch_completed rows=12 dur=128962us
CREATE UNIQUE CLUSTERED INDEX _Reference14_S_HPKNG ON dbo._Reference14NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:55.535000 sid=257 sql_batch_completed rows=4 dur=29278us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT67_SKNG ON dbo._Reference14_VT67NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:55.563000 sid=257 sql_batch_completed rows=4 dur=16166us
CREATE INDEX _Reference14_VT67_1NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld69RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.612000 sid=257 sql_batch_completed rows=4 dur=41030us
CREATE INDEX _Reference14_VT67_2NG ON dbo._Reference14_VT67NG (_Fld2683, _Fld70RRef, _Reference14_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.625000 sid=257 sql_batch_completed rows=0 dur=8111us
CREATE UNIQUE CLUSTERED INDEX _Reference14_VT11034_SKNG ON dbo._Reference14_VT11034NG (_Fld2683, _Reference14_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:55.676000 sid=257 sql_batch_completed rows=0 dur=10470us
create table dbo._Reference16NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_ParentIDRRef binary(16) not null,
_Folder binary(1) not null,
_Code nvarchar(9) not null,
_Description nvarchar(50) not null,
_Fld121RRef binary(16),
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference16NG SET (LOCK_ESCALATION = DISABLE);

-- 00:56:55.703000 sid=257 sql_batch_completed rows=0 dur=13023us
create table dbo._Reference16_VT11037NG (
_Reference16_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11038 numeric(5, 0) not null,
_Fld11039 datetime2(0) not null,
_Fld11040 nvarchar(50) not null
)
;alter table dbo._Reference16_VT11037NG SET (LOCK_ESCALATION = DISABLE);

-- 00:56:55.717000 sid=257 sql_batch_completed rows=0 dur=4279us
CREATE INDEX _Reference16_1NG ON dbo._Reference16NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.728000 sid=257 sql_batch_completed rows=0 dur=3114us
CREATE UNIQUE INDEX _Reference16_2NG ON dbo._Reference16NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.739000 sid=257 sql_batch_completed rows=0 dur=5259us
CREATE UNIQUE INDEX _Reference16_3NG ON dbo._Reference16NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.750000 sid=257 sql_batch_completed rows=0 dur=8292us
CREATE UNIQUE INDEX _Reference16_4NG ON dbo._Reference16NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.757000 sid=257 sql_batch_completed rows=0 dur=5479us
CREATE UNIQUE INDEX _Reference16_5NG ON dbo._Reference16NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.772000 sid=257 sql_batch_completed rows=0 dur=11664us
CREATE UNIQUE CLUSTERED INDEX _Reference16_S_HPKNG ON dbo._Reference16NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:55.800000 sid=257 sql_batch_completed rows=0 dur=26764us
CREATE UNIQUE CLUSTERED INDEX _Reference16_VT11037_SKNG ON dbo._Reference16_VT11037NG (_Fld2683, _Reference16_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:55.813000 sid=257 sql_batch_completed rows=0 dur=5385us
CREATE INDEX _Reference16_VT11037_1NG ON dbo._Reference16_VT11037NG (_Fld2683, _Fld11040, _Reference16_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:55.822000 sid=257 rpc_completed rows=1 dur=6495us
-- args: ,N'@P1 varbinary(max)',0x<6622 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:56:55.856000 sid=257 sql_batch_completed rows=0 dur=6991us
drop index _Reference16_1NG on dbo._Reference16NG;

-- 00:56:55.888000 sid=257 sql_batch_completed rows=0 dur=23259us
drop index _Reference16_2NG on dbo._Reference16NG;

-- 00:56:55.899000 sid=257 sql_batch_completed rows=0 dur=7201us
drop index _Reference16_3NG on dbo._Reference16NG;

-- 00:56:55.912000 sid=257 sql_batch_completed rows=0 dur=6945us
drop index _Reference16_4NG on dbo._Reference16NG;

-- 00:56:55.923000 sid=257 sql_batch_completed rows=0 dur=8152us
drop index _Reference16_5NG on dbo._Reference16NG;

-- 00:56:55.961000 sid=257 sql_batch_completed rows=0 dur=2752us
drop index _Reference16_S_HPKNG on dbo._Reference16NG;

-- 00:56:55.973000 sid=257 sql_batch_completed rows=0 dur=3778us
drop index _Reference16_VT11037_1NG on dbo._Reference16_VT11037NG;

-- 00:56:55.992000 sid=257 sql_batch_completed rows=0 dur=17608us
drop index _Reference16_VT11037_SKNG on dbo._Reference16_VT11037NG;

-- 00:56:56.016000 sid=257 sql_batch_completed rows=5 dur=20681us
INSERT INTO dbo._Reference16NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Code, _Description, _Fld121RRef, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._ParentIDRRef,
T1._Folder,
T1._Code,
T1._Description,
T1._Fld121RRef,
T1._Fld2683
FROM dbo._Reference16 T1 WITH(NOLOCK);

-- 00:56:56.046000 sid=257 sql_batch_completed rows=5 dur=22190us
CREATE INDEX _Reference16_1NG ON dbo._Reference16NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.076000 sid=257 sql_batch_completed rows=5 dur=23506us
CREATE UNIQUE INDEX _Reference16_2NG ON dbo._Reference16NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.094000 sid=257 sql_batch_completed rows=5 dur=7598us
CREATE UNIQUE INDEX _Reference16_3NG ON dbo._Reference16NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.103000 sid=257 sql_batch_completed rows=5 dur=6643us
CREATE UNIQUE INDEX _Reference16_4NG ON dbo._Reference16NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.138000 sid=257 sql_batch_completed rows=5 dur=16677us
CREATE UNIQUE INDEX _Reference16_5NG ON dbo._Reference16NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.186000 sid=257 sql_batch_completed rows=30 dur=46681us
CREATE UNIQUE CLUSTERED INDEX _Reference16_S_HPKNG ON dbo._Reference16NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:56.192000 sid=257 sql_batch_completed rows=0 dur=5283us
CREATE UNIQUE CLUSTERED INDEX _Reference16_VT11037_SKNG ON dbo._Reference16_VT11037NG (_Fld2683, _Reference16_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:56.205000 sid=257 sql_batch_completed rows=0 dur=6892us
CREATE INDEX _Reference16_VT11037_1NG ON dbo._Reference16_VT11037NG (_Fld2683, _Fld11040, _Reference16_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.326000 sid=257 sql_batch_completed rows=0 dur=15643us
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

-- 00:56:56.373000 sid=257 sql_batch_completed rows=0 dur=44286us
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

-- 00:56:56.386000 sid=257 sql_batch_completed rows=0 dur=6457us
CREATE INDEX _Reference23_1NG ON dbo._Reference23NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.392000 sid=257 sql_batch_completed rows=0 dur=5070us
CREATE UNIQUE INDEX _Reference23_2NG ON dbo._Reference23NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.404000 sid=257 sql_batch_completed rows=0 dur=9228us
CREATE UNIQUE INDEX _Reference23_3NG ON dbo._Reference23NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.420000 sid=257 sql_batch_completed rows=0 dur=15230us
CREATE UNIQUE CLUSTERED INDEX _Reference23_S_HPKNG ON dbo._Reference23NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:56.446000 sid=257 sql_batch_completed rows=0 dur=18537us
CREATE UNIQUE CLUSTERED INDEX _Reference23_VT11041_SKNG ON dbo._Reference23_VT11041NG (_Fld2683, _Reference23_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:56.456000 sid=257 sql_batch_completed rows=0 dur=2205us
CREATE INDEX _Reference23_VT11041_1NG ON dbo._Reference23_VT11041NG (_Fld2683, _Fld11044, _Reference23_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.461000 sid=257 rpc_completed rows=1 dur=3926us
-- args: ,N'@P1 varbinary(max)',0x<8804 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:56:56.497000 sid=257 sql_batch_completed rows=0 dur=3358us
drop index _Reference23_1NG on dbo._Reference23NG;

-- 00:56:56.500000 sid=257 sql_batch_completed rows=0 dur=2629us
drop index _Reference23_2NG on dbo._Reference23NG;

-- 00:56:56.504000 sid=257 sql_batch_completed rows=0 dur=2695us
drop index _Reference23_3NG on dbo._Reference23NG;

-- 00:56:56.512000 sid=257 sql_batch_completed rows=0 dur=7096us
drop index _Reference23_S_HPKNG on dbo._Reference23NG;

-- 00:56:56.558000 sid=257 sql_batch_completed rows=0 dur=24773us
drop index _Reference23_VT11041_1NG on dbo._Reference23_VT11041NG;

-- 00:56:56.572000 sid=257 sql_batch_completed rows=0 dur=4335us
drop index _Reference23_VT11041_SKNG on dbo._Reference23_VT11041NG;

-- 00:56:56.596000 sid=257 sql_batch_completed rows=2 dur=20265us
INSERT INTO dbo._Reference23NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _Code, _Description, _Fld181, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._Code,
T1._Description,
T1._Fld181,
T1._Fld2683
FROM dbo._Reference23 T1 WITH(NOLOCK);

-- 00:56:56.637000 sid=257 sql_batch_completed rows=2 dur=12975us
CREATE INDEX _Reference23_1NG ON dbo._Reference23NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.650000 sid=257 sql_batch_completed rows=2 dur=9183us
CREATE UNIQUE INDEX _Reference23_2NG ON dbo._Reference23NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.668000 sid=257 sql_batch_completed rows=2 dur=9645us
CREATE UNIQUE INDEX _Reference23_3NG ON dbo._Reference23NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:56:56.695000 sid=257 sql_batch_completed rows=8 dur=21436us
CREATE UNIQUE CLUSTERED INDEX _Reference23_S_HPKNG ON dbo._Reference23NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:56.708000 sid=257 sql_batch_completed rows=0 dur=4778us
CREATE UNIQUE CLUSTERED INDEX _Reference23_VT11041_SKNG ON dbo._Reference23_VT11041NG (_Fld2683, _Reference23_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:56:56.726000 sid=257 sql_batch_completed rows=0 dur=8151us
CREATE INDEX _Reference23_VT11041_1NG ON dbo._Reference23_VT11041NG (_Fld2683, _Fld11044, _Reference23_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:00.642000 sid=257 sql_batch_completed rows=0 dur=7735us
create table dbo._Document581NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_Date_Time datetime2(0) not null,
_Number nvarchar(11) not null,
_Posted binary(1) not null,
_Fld1520 nvarchar(max) not null,
_Fld802RRef binary(16) not null,
_Fld801RRef binary(16) not null,
_Fld803RRef binary(16) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Document581NG SET (LOCK_ESCALATION = DISABLE);

-- 00:57:00.653000 sid=257 sql_batch_completed rows=0 dur=7096us
create table dbo._Document581_VT804NG (
_Document581_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo805 numeric(5, 0) not null,
_Fld806RRef binary(16) not null,
_Fld809 numeric(15, 3) not null
)
;alter table dbo._Document581_VT804NG SET (LOCK_ESCALATION = DISABLE);

-- 00:57:00.686000 sid=257 sql_batch_completed rows=0 dur=9204us
create table dbo._Document581_VT11047NG (
_Document581_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11048 numeric(5, 0) not null,
_Fld11049 binary(1) not null
)
;alter table dbo._Document581_VT11047NG SET (LOCK_ESCALATION = DISABLE);

-- 00:57:00.694000 sid=257 sql_batch_completed rows=0 dur=4518us
CREATE UNIQUE INDEX _Document581_1NG ON dbo._Document581NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:00.699000 sid=257 sql_batch_completed rows=0 dur=3548us
CREATE UNIQUE INDEX _Document581_2NG ON dbo._Document581NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:00.703000 sid=257 sql_batch_completed rows=0 dur=3888us
CREATE UNIQUE CLUSTERED INDEX _Document581_S_HPKNG ON dbo._Document581NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:00.707000 sid=257 sql_batch_completed rows=0 dur=2819us
CREATE UNIQUE CLUSTERED INDEX _Document581_VT804_SKNG ON dbo._Document581_VT804NG (_Fld2683, _Document581_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:00.712000 sid=257 sql_batch_completed rows=0 dur=5047us
CREATE UNIQUE CLUSTERED INDEX _Document581_VT11047_SKNG ON dbo._Document581_VT11047NG (_Fld2683, _Document581_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:00.718000 sid=257 rpc_completed rows=1 dur=1796us
-- args: ,N'@P1 varbinary(max)',0x<11220 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:57:00.730000 sid=257 sql_batch_completed rows=0 dur=1648us
drop index _Document581_1NG on dbo._Document581NG;

-- 00:57:00.733000 sid=257 sql_batch_completed rows=0 dur=1968us
drop index _Document581_2NG on dbo._Document581NG;

-- 00:57:00.738000 sid=257 sql_batch_completed rows=0 dur=3873us
drop index _Document581_S_HPKNG on dbo._Document581NG;

-- 00:57:00.751000 sid=257 sql_batch_completed rows=0 dur=9095us
drop index _Document581_VT804_SKNG on dbo._Document581_VT804NG;

-- 00:57:00.763000 sid=257 sql_batch_completed rows=0 dur=9091us
drop index _Document581_VT11047_SKNG on dbo._Document581_VT11047NG;

-- 00:57:00.810000 sid=257 sql_batch_completed rows=6 dur=45537us
INSERT INTO dbo._Document581NG WITH(TABLOCK) (_IDRRef, _Marked, _Date_Time, _Number, _Posted, _Fld1520, _Fld802RRef, _Fld801RRef, _Fld803RRef, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._Date_Time,
T1._Number,
T1._Posted,
T1._Fld1520,
T1._Fld802RRef,
T1._Fld801RRef,
T1._Fld803RRef,
T1._Fld2683
FROM dbo._Document581 T1 WITH(NOLOCK);

-- 00:57:00.836000 sid=257 sql_batch_completed rows=8 dur=20390us
INSERT INTO dbo._Document581_VT804NG WITH(TABLOCK) (_LineNo805, _Fld806RRef, _Fld809, _Fld2683, _Document581_IDRRef, _KeyField) SELECT
T2._LineNo805,
T2._Fld806RRef,
T2._Fld809,
T2._Fld2683,
T2._Document581_IDRRef,
T2._KeyField
FROM dbo._Document581_VT804 T2 WITH(NOLOCK);

-- 00:57:00.869000 sid=257 sql_batch_completed rows=6 dur=8890us
CREATE UNIQUE INDEX _Document581_1NG ON dbo._Document581NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:00.904000 sid=257 sql_batch_completed rows=6 dur=18202us
CREATE UNIQUE INDEX _Document581_2NG ON dbo._Document581NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:00.926000 sid=257 sql_batch_completed rows=18 dur=19100us
CREATE UNIQUE CLUSTERED INDEX _Document581_S_HPKNG ON dbo._Document581NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:00.936000 sid=257 sql_batch_completed rows=8 dur=9293us
CREATE UNIQUE CLUSTERED INDEX _Document581_VT804_SKNG ON dbo._Document581_VT804NG (_Fld2683, _Document581_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:00.941000 sid=257 sql_batch_completed rows=0 dur=3795us
CREATE UNIQUE CLUSTERED INDEX _Document581_VT11047_SKNG ON dbo._Document581_VT11047NG (_Fld2683, _Document581_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:00.968000 sid=257 sql_batch_completed rows=0 dur=13904us
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

-- 00:57:00.987000 sid=257 sql_batch_completed rows=0 dur=12738us
create table dbo._Document4883_VT11050NG (
_Document4883_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11051 numeric(5, 0) not null,
_Fld11052 nvarchar(15) not null,
_Fld11053 numeric(15, 2) not null
)
;alter table dbo._Document4883_VT11050NG SET (LOCK_ESCALATION = DISABLE);

-- 00:57:00.995000 sid=257 sql_batch_completed rows=0 dur=5074us
CREATE UNIQUE INDEX _Document4883_1NG ON dbo._Document4883NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:00.999000 sid=257 sql_batch_completed rows=0 dur=3216us
CREATE UNIQUE INDEX _Document4883_2NG ON dbo._Document4883NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:01.005000 sid=257 sql_batch_completed rows=0 dur=5439us
CREATE UNIQUE CLUSTERED INDEX _Document4883_S_HPKNG ON dbo._Document4883NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:01.023000 sid=257 sql_batch_completed rows=0 dur=17331us
CREATE UNIQUE CLUSTERED INDEX _Document4883_VT11050_SKNG ON dbo._Document4883_VT11050NG (_Fld2683, _Document4883_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:01.026000 sid=257 rpc_completed rows=1 dur=2142us
-- args: ,N'@P1 varbinary(max)',0x<13404 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:57:01.042000 sid=257 sql_batch_completed rows=0 dur=5728us
drop index _Document4883_1NG on dbo._Document4883NG;

-- 00:57:01.048000 sid=257 sql_batch_completed rows=0 dur=3966us
drop index _Document4883_2NG on dbo._Document4883NG;

-- 00:57:01.057000 sid=257 sql_batch_completed rows=0 dur=6563us
drop index _Document4883_S_HPKNG on dbo._Document4883NG;

-- 00:57:01.066000 sid=257 sql_batch_completed rows=0 dur=4656us
drop index _Document4883_VT11050_SKNG on dbo._Document4883_VT11050NG;

-- 00:57:01.105000 sid=257 sql_batch_completed rows=7 dur=35477us
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

-- 00:57:01.130000 sid=257 sql_batch_completed rows=7 dur=19883us
CREATE UNIQUE INDEX _Document4883_1NG ON dbo._Document4883NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:01.141000 sid=257 sql_batch_completed rows=7 dur=7911us
CREATE UNIQUE INDEX _Document4883_2NG ON dbo._Document4883NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:01.160000 sid=257 sql_batch_completed rows=21 dur=18381us
CREATE UNIQUE CLUSTERED INDEX _Document4883_S_HPKNG ON dbo._Document4883NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:01.163000 sid=257 sql_batch_completed rows=0 dur=2530us
CREATE UNIQUE CLUSTERED INDEX _Document4883_VT11050_SKNG ON dbo._Document4883_VT11050NG (_Fld2683, _Document4883_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:01.298000 sid=257 sql_batch_completed rows=0 dur=19847us
create table dbo._DbCopiesUpdatesNG (
_CopyId binary(16) not null,
_TrNum int,
_TrTime datetime2(0),
_UpdateId binary(16),
_LastUpdateResult numeric(2, 0),
_LastUpdateError varbinary(max)
)
;alter table dbo._DbCopiesUpdatesNG SET (LOCK_ESCALATION = DISABLE);

-- 00:57:01.312000 sid=257 sql_batch_completed rows=0 dur=12780us
CREATE UNIQUE CLUSTERED INDEX _DbCopiesUpdates_1NG ON dbo._DbCopiesUpdatesNG (_CopyId, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:01.314000 sid=257 rpc_completed rows=1 dur=1333us
-- args: ,N'@P1 varbinary(max)',0x<14292 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:57:01.329000 sid=257 sql_batch_completed rows=0 dur=10951us
INSERT INTO dbo._DbCopiesUpdatesNG WITH(TABLOCK) (_CopyId, _TrNum, _TrTime) SELECT
T1._CopyId,
(T1._TrNum + 0.0),
T1._TrTime
FROM dbo._DbCopiesUpdates T1 WITH(NOLOCK);

-- 00:57:01.343000 sid=257 sql_batch_completed rows=0 dur=9489us
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

-- 00:57:01.354000 sid=257 sql_batch_completed rows=0 dur=10512us
CREATE UNIQUE CLUSTERED INDEX _DbCopies_1NG ON dbo._DbCopiesNG (_CopyId, _CopyName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:01.362000 sid=257 rpc_completed rows=1 dur=3690us
-- args: ,N'@P1 varbinary(max)',0x<15820 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:57:01.372000 sid=257 rpc_completed rows=0 dur=3088us
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

-- 00:57:02.760000 sid=257 sql_batch_completed rows=0 dur=24573us
create table dbo._ConfigChngRNG (
_NodeTRef binary(4) not null,
_NodeRRef binary(16) not null,
_MessageNo numeric(10, 0),
_MDObjID binary(16) not null,
_IDRRef binary(16) not null primary key
)
;alter table dbo._ConfigChngRNG SET (LOCK_ESCALATION = DISABLE);

-- 00:57:02.784000 sid=257 sql_batch_completed rows=0 dur=16880us
create table dbo._ConfigChngR_ExtPropsNG (
_ConfigChngR_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_FileName nvarchar(128) not null
)
;alter table dbo._ConfigChngR_ExtPropsNG SET (LOCK_ESCALATION = DISABLE);

-- 00:57:02.797000 sid=257 sql_batch_completed rows=0 dur=5667us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:02.817000 sid=257 sql_batch_completed rows=0 dur=10703us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:02.836000 sid=257 sql_batch_completed rows=0 dur=15256us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:02.841000 sid=257 rpc_completed rows=1 dur=3182us
-- args: ,N'@P1 varbinary(max)',0x<16884 hex>
UPDATE SchemaStorage SET NewGenCreated = @P1, Status = 200 WHERE SchemaID = 0;

-- 00:57:02.862000 sid=257 sql_batch_completed rows=0 dur=4927us
drop index _ConfigChngR_1NG on dbo._ConfigChngRNG;

-- 00:57:02.891000 sid=257 sql_batch_completed rows=0 dur=17839us
drop index _ConfigChngR_2NG on dbo._ConfigChngRNG;

-- 00:57:02.920000 sid=257 sql_batch_completed rows=0 dur=6478us
drop index _ConfigChngR_ExtProps_SKNG on dbo._ConfigChngR_ExtPropsNG;

-- 00:57:03.759000 sid=257 sql_batch_completed rows=10000 dur=77986us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 00:57:03.823000 sid=257 sql_batch_completed rows=9996 dur=54945us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=9996);

-- 00:57:04.610000 sid=257 sql_batch_completed rows=10000 dur=60512us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=10000);

-- 00:57:04.680000 sid=257 sql_batch_completed rows=10000 dur=59013us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 00:57:04.689000 sid=257 sql_batch_completed rows=658 dur=8487us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=10000);

-- 00:57:04.983000 sid=257 sql_batch_completed rows=685 dur=9531us
insert bulk dbo._ConfigChngRNG([_NodeTRef] binary(4),[_NodeRRef] binary(16),[_MessageNo] numeric(10,0),[_MDObjID] binary(16),[_IDRRef] binary(16))with(TABLOCK, ROWS_PER_BATCH=685);

-- 00:57:05.002000 sid=257 sql_batch_completed rows=703 dur=15314us
insert bulk dbo._ConfigChngR_ExtPropsNG([_ConfigChngR_IDRRef] binary(16),[_KeyField] binary(4),[_FileName] nvarchar(128) collate Cyrillic_General_CI_AS)with(TABLOCK, ROWS_PER_BATCH=703);

-- 00:57:05.059000 sid=257 sql_batch_completed rows=20685 dur=45843us
CREATE UNIQUE INDEX _ConfigChngR_1NG ON dbo._ConfigChngRNG (_NodeTRef, _NodeRRef, _MessageNo, _MDObjID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:05.101000 sid=257 sql_batch_completed rows=20685 dur=39049us
CREATE UNIQUE INDEX _ConfigChngR_2NG ON dbo._ConfigChngRNG (_MDObjID, _NodeTRef, _NodeRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- 00:57:05.150000 sid=257 sql_batch_completed rows=21357 dur=47874us
CREATE UNIQUE CLUSTERED INDEX _ConfigChngR_ExtProps_SKNG ON dbo._ConfigChngR_ExtPropsNG (_ConfigChngR_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:57:06.801000 sid=257 rpc_completed rows=1 dur=3102us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x,-1,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 00:57:06.861000 sid=257 rpc_completed rows=1 dur=5852us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0x02EA7E347307E8118780107B44A2858A,0x<27780 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 00:57:08.348000 sid=257 rpc_completed rows=1 dur=540us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xBA6F15B59163934A88887C063A551319,0x<888 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 00:57:08.395000 sid=257 rpc_completed rows=1 dur=242us
-- args: ,N'@P1 varbinary(16),@P2 varbinary(max),@P3 numeric(9),@P4 numeric(9),@P5 varbinary(1),@P6 numeric(7)',0xF00EFAB36899F1118F4F00E04C680093,0x<7728 hex>,0,0,0x00,0
INSERT INTO dbo._ExtensionsRestructNGS WITH(TABLOCK) (_ExtDataID,_RestructData,_RestructDataInt,_RestructDataType,_DataSeparationUse2858,_Fld2683) VALUES(@P1,@P2,@P3,@P4,@P5,@P6);

-- 00:58:21.306000 sid=257 sql_batch_completed rows=1 dur=6150us
UPDATE SchemaStorage SET Status = 400 WHERE SchemaID = 0;

-- 00:58:21.345000 sid=257 sql_batch_completed rows=0 dur=30412us
drop table dbo._Reference14;

-- 00:58:21.361000 sid=257 sql_batch_completed rows=0 dur=13631us
drop table dbo._Reference14_VT67;

-- 00:58:21.379000 sid=257 sql_batch_completed rows=0 dur=16082us
drop table dbo._Reference16;

-- 00:58:21.422000 sid=257 sql_batch_completed rows=0 dur=31097us
drop table dbo._Reference23;

-- 00:58:21.456000 sid=257 sql_batch_completed rows=0 dur=28614us
drop table dbo._Document581;

-- 00:58:21.466000 sid=257 sql_batch_completed rows=0 dur=5997us
drop table dbo._Document581_VT804;

-- 00:58:21.489000 sid=257 sql_batch_completed rows=0 dur=22839us
drop table dbo._Document4883;

-- 00:58:21.499000 sid=257 sql_batch_completed rows=0 dur=8567us
drop table dbo._DbCopiesUpdates;

-- 00:58:21.504000 sid=257 sql_batch_completed rows=0 dur=3931us
drop table dbo._DbCopies;

-- 00:58:21.525000 sid=257 sql_batch_completed rows=0 dur=20031us
drop table dbo._ConfigChngR;

-- 00:58:21.534000 sid=257 sql_batch_completed rows=0 dur=5398us
drop table dbo._ConfigChngR_ExtProps;

-- 00:58:21.537000 sid=257 sql_batch_completed rows=1 dur=1971us
UPDATE SchemaStorage SET Status = 500 WHERE SchemaID = 0;

-- 00:58:22.256000 sid=257 rpc_completed rows=34 dur=713058us
-- args: N'_Reference14NG',N'_Reference14'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.271000 sid=257 rpc_completed rows=25 dur=10406us
-- args: N'_Reference14_VT67NG',N'_Reference14_VT67'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.277000 sid=257 rpc_completed rows=25 dur=4117us
-- args: N'_Reference14_VT11034NG',N'_Reference14_VT11034'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.281000 sid=257 rpc_completed rows=16 dur=1929us
-- args: N'_Reference14._Reference14_1NG',N'_Reference14_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.284000 sid=257 rpc_completed rows=16 dur=2183us
-- args: N'_Reference14._Reference14_2NG',N'_Reference14_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.288000 sid=257 rpc_completed rows=16 dur=1623us
-- args: N'_Reference14._Reference14_3NG',N'_Reference14_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.293000 sid=257 rpc_completed rows=16 dur=2121us
-- args: N'_Reference14._Reference14_S_HPKNG',N'_Reference14_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.297000 sid=257 rpc_completed rows=16 dur=2706us
-- args: N'_Reference14_VT67._Reference14_VT67_SKNG',N'_Reference14_VT67_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.304000 sid=257 rpc_completed rows=16 dur=5235us
-- args: N'_Reference14_VT67._Reference14_VT67_1NG',N'_Reference14_VT67_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.308000 sid=257 rpc_completed rows=16 dur=1419us
-- args: N'_Reference14_VT67._Reference14_VT67_2NG',N'_Reference14_VT67_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.315000 sid=257 rpc_completed rows=16 dur=5261us
-- args: N'_Reference14_VT11034._Reference14_VT11034_SKNG',N'_Reference14_VT11034_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.321000 sid=257 rpc_completed rows=25 dur=3972us
-- args: N'_Reference16NG',N'_Reference16'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.325000 sid=257 rpc_completed rows=25 dur=2651us
-- args: N'_Reference16_VT11037NG',N'_Reference16_VT11037'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.329000 sid=257 rpc_completed rows=16 dur=1914us
-- args: N'_Reference16._Reference16_1NG',N'_Reference16_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.331000 sid=257 rpc_completed rows=16 dur=1058us
-- args: N'_Reference16._Reference16_2NG',N'_Reference16_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.334000 sid=257 rpc_completed rows=16 dur=1653us
-- args: N'_Reference16._Reference16_3NG',N'_Reference16_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.343000 sid=257 rpc_completed rows=16 dur=5624us
-- args: N'_Reference16._Reference16_4NG',N'_Reference16_4'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.352000 sid=257 rpc_completed rows=16 dur=6385us
-- args: N'_Reference16._Reference16_5NG',N'_Reference16_5'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.358000 sid=257 rpc_completed rows=16 dur=3337us
-- args: N'_Reference16._Reference16_S_HPKNG',N'_Reference16_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.362000 sid=257 rpc_completed rows=16 dur=1635us
-- args: N'_Reference16_VT11037._Reference16_VT11037_SKNG',N'_Reference16_VT11037_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.366000 sid=257 rpc_completed rows=16 dur=2410us
-- args: N'_Reference16_VT11037._Reference16_VT11037_1NG',N'_Reference16_VT11037_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.374000 sid=257 rpc_completed rows=25 dur=4993us
-- args: N'_Reference23NG',N'_Reference23'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.387000 sid=257 rpc_completed rows=25 dur=10605us
-- args: N'_Reference23_VT11041NG',N'_Reference23_VT11041'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.391000 sid=257 rpc_completed rows=16 dur=1724us
-- args: N'_Reference23._Reference23_1NG',N'_Reference23_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.395000 sid=257 rpc_completed rows=16 dur=2409us
-- args: N'_Reference23._Reference23_2NG',N'_Reference23_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.401000 sid=257 rpc_completed rows=16 dur=3126us
-- args: N'_Reference23._Reference23_3NG',N'_Reference23_3'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.404000 sid=257 rpc_completed rows=16 dur=1231us
-- args: N'_Reference23._Reference23_S_HPKNG',N'_Reference23_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.407000 sid=257 rpc_completed rows=16 dur=1353us
-- args: N'_Reference23_VT11041._Reference23_VT11041_SKNG',N'_Reference23_VT11041_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.409000 sid=257 rpc_completed rows=16 dur=881us
-- args: N'_Reference23_VT11041._Reference23_VT11041_1NG',N'_Reference23_VT11041_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.416000 sid=257 rpc_completed rows=25 dur=6054us
-- args: N'_Document581NG',N'_Document581'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.422000 sid=257 rpc_completed rows=25 dur=4251us
-- args: N'_Document581_VT804NG',N'_Document581_VT804'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.427000 sid=257 rpc_completed rows=25 dur=3851us
-- args: N'_Document581_VT11047NG',N'_Document581_VT11047'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.433000 sid=257 rpc_completed rows=16 dur=3136us
-- args: N'_Document581._Document581_1NG',N'_Document581_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.440000 sid=257 rpc_completed rows=16 dur=5132us
-- args: N'_Document581._Document581_2NG',N'_Document581_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.443000 sid=257 rpc_completed rows=16 dur=1129us
-- args: N'_Document581._Document581_S_HPKNG',N'_Document581_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.449000 sid=257 rpc_completed rows=16 dur=3629us
-- args: N'_Document581_VT804._Document581_VT804_SKNG',N'_Document581_VT804_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.456000 sid=257 rpc_completed rows=16 dur=3609us
-- args: N'_Document581_VT11047._Document581_VT11047_SKNG',N'_Document581_VT11047_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.471000 sid=257 rpc_completed rows=25 dur=12342us
-- args: N'_Document4883NG',N'_Document4883'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.480000 sid=257 rpc_completed rows=25 dur=6935us
-- args: N'_Document4883_VT11050NG',N'_Document4883_VT11050'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.490000 sid=257 rpc_completed rows=16 dur=3683us
-- args: N'_Document4883._Document4883_1NG',N'_Document4883_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.509000 sid=257 rpc_completed rows=16 dur=5334us
-- args: N'_Document4883._Document4883_2NG',N'_Document4883_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.519000 sid=257 rpc_completed rows=16 dur=1570us
-- args: N'_Document4883._Document4883_S_HPKNG',N'_Document4883_S_HPK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.530000 sid=257 rpc_completed rows=16 dur=6841us
-- args: N'_Document4883_VT11050._Document4883_VT11050_SKNG',N'_Document4883_VT11050_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.539000 sid=257 rpc_completed rows=25 dur=5465us
-- args: N'_DbCopiesUpdatesNG',N'_DbCopiesUpdates'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.546000 sid=257 rpc_completed rows=16 dur=4288us
-- args: N'_DbCopiesUpdates._DbCopiesUpdates_1NG',N'_DbCopiesUpdates_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.572000 sid=257 rpc_completed rows=25 dur=24375us
-- args: N'_DbCopiesNG',N'_DbCopies'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.578000 sid=257 rpc_completed rows=16 dur=4681us
-- args: N'_DbCopies._DbCopies_1NG',N'_DbCopies_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.597000 sid=257 rpc_completed rows=25 dur=17139us
-- args: N'_ConfigChngRNG',N'_ConfigChngR'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.606000 sid=257 rpc_completed rows=25 dur=3984us
-- args: N'_ConfigChngR_ExtPropsNG',N'_ConfigChngR_ExtProps'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 00:58:22.612000 sid=257 rpc_completed rows=16 dur=3511us
-- args: N'_ConfigChngR._ConfigChngR_1NG',N'_ConfigChngR_1'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.616000 sid=257 rpc_completed rows=16 dur=2233us
-- args: N'_ConfigChngR._ConfigChngR_2NG',N'_ConfigChngR_2'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.624000 sid=257 rpc_completed rows=16 dur=4120us
-- args: N'_ConfigChngR_ExtProps._ConfigChngR_ExtProps_SKNG',N'_ConfigChngR_ExtProps_SK'
exec @P1 = sp_rename @P2, @P3, 'index';

-- 00:58:22.761000 sid=257 sql_batch_completed rows=0 dur=1013us
ALTER INDEX PK___ConfigC__AC8ED0C41B0F2873 ON _ConfigChngR SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.774000 sid=257 sql_batch_completed rows=0 dur=12875us
ALTER INDEX PK___Referen__AC8ED0C4077A8F3B ON _Reference10877X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.789000 sid=257 sql_batch_completed rows=0 dur=14726us
ALTER INDEX PK___Referen__AC8ED0C4DBBB6986 ON _Reference10878X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.800000 sid=257 sql_batch_completed rows=0 dur=10102us
ALTER INDEX PK___Referen__AC8ED0C4720747AF ON _Reference10825X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.813000 sid=257 sql_batch_completed rows=0 dur=12597us
ALTER INDEX PK___Enum108__AC8ED0C45E875D60 ON _Enum10826X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.824000 sid=257 sql_batch_completed rows=0 dur=11413us
ALTER INDEX PK___Enum110__AC8ED0C4A5E2EE79 ON _Enum11017X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.851000 sid=257 sql_batch_completed rows=0 dur=25959us
ALTER INDEX PK___Documen__AC8ED0C48EB57390 ON _Document10069X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.880000 sid=257 sql_batch_completed rows=0 dur=29167us
ALTER INDEX PK___Referen__AC8ED0C43F77AD82 ON _Reference2596X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.888000 sid=257 sql_batch_completed rows=0 dur=7180us
ALTER INDEX PK___Referen__AC8ED0C45FCE1C7A ON _Reference10523X1 SET(ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- 00:58:22.972000 sid=257 rpc_completed rows=1 dur=54271us
-- args: ,N'@P1 varbinary(max),@P2 varbinary(max),@P3 varbinary(max)',0x<1957984 hex>,0xEFBBBF7B302C0D0A7B307D0D0A7D,0xEFBBBF7B302C0D0A7B307D0D0A7D
UPDATE SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- 00:58:23.050000 sid=257 rpc_completed rows=1 dur=36789us
-- args: ,N'@P1 varbinary(max)',0x<1957984 hex>
UPDATE DBSchema SET SerializedData = @P1;

