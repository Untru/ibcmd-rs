-- The statements of ibcmd-rs mssql-restructure for case a2 (a new String(50) attribute of the catalog _ДемоПартнеры, БСП 8.3.27)
-- as the tool dumped them (--dump-plan) before the real apply on the twin ibcmd_rs_04_ddl_a2_fin. They run in ONE transaction
-- (SET XACT_ABORT ON; SET LOCK_TIMEOUT 60000; BEGIN TRANSACTION ... COMMIT, see prototype-apply-report-a2.json for the times).
-- @P1..@P3 are bound parameters: the binary DBSchema text, the empty generation marker (14 bytes), the DBNames deflate, the
-- DBNamesVersion row, the XDTO model row (a raw deflate of about 2.9 MB) -- the values are in the lab, not here.
-- Compare with a2-structure-statements.sql, the statements of the NATIVE apply of the same case.
-- Guard: the stored schema is the one the plan was made from
IF NOT EXISTS (SELECT 1 FROM dbo.SchemaStorage WHERE SchemaID = 0 AND Status = 100 AND DATALENGTH(NewGenCreated) = 14 AND DATALENGTH(NewGenDropped) = 14 AND CONVERT(varchar(64), HASHBYTES('SHA2_256', CurrentSchema), 2) = 'BA56D560791813EE2063F55501D866DD05EFDA500D6EEA9A0AD97F1DB094AAEF') THROW 51000, 'SchemaStorage is not idle or its schema is not the planned one', 1;

-- Create: create _Reference20NG
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

-- Create: create _Reference20_VT155NG
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

-- Create: create _Reference20_VT159NG
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

-- Load: copy _Reference20 into _Reference20NG
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
CAST(CASE WHEN T1._Folder = 0x01 THEN N'' END AS NVARCHAR(50)),
T1._Fld2683
FROM dbo._Reference20 T1 WITH(NOLOCK);

-- Load: copy _Reference20_VT155 into _Reference20_VT155NG
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

-- Load: copy _Reference20_VT159 into _Reference20_VT159NG
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

-- Indexes: index _Reference20_1NG
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- Indexes: index _Reference20_2NG
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- Indexes: index _Reference20_3NG
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- Indexes: index _Reference20_4NG
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- Indexes: index _Reference20_5NG
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- Indexes: index _Reference20_S_HPKNG
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- Indexes: index _Reference20_VT155_SKNG
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- Indexes: index _Reference20_VT159_SKNG
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);

-- Indexes: index _Reference20_VT159_1NG
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- Indexes: index _Reference20_VT159_2NG
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];

-- DropOld: drop _Reference20
drop table dbo._Reference20;

-- DropOld: drop _Reference20_VT155
drop table dbo._Reference20_VT155;

-- DropOld: drop _Reference20_VT159
drop table dbo._Reference20_VT159;

-- Rename: rename _Reference20NG
EXEC sp_rename N'_Reference20NG', N'_Reference20', 'OBJECT';

-- Rename: rename _Reference20_VT155NG
EXEC sp_rename N'_Reference20_VT155NG', N'_Reference20_VT155', 'OBJECT';

-- Rename: rename _Reference20_VT159NG
EXEC sp_rename N'_Reference20_VT159NG', N'_Reference20_VT159', 'OBJECT';

-- Rename: rename index _Reference20_1NG
EXEC sp_rename N'_Reference20._Reference20_1NG', N'_Reference20_1', 'INDEX';

-- Rename: rename index _Reference20_2NG
EXEC sp_rename N'_Reference20._Reference20_2NG', N'_Reference20_2', 'INDEX';

-- Rename: rename index _Reference20_3NG
EXEC sp_rename N'_Reference20._Reference20_3NG', N'_Reference20_3', 'INDEX';

-- Rename: rename index _Reference20_4NG
EXEC sp_rename N'_Reference20._Reference20_4NG', N'_Reference20_4', 'INDEX';

-- Rename: rename index _Reference20_5NG
EXEC sp_rename N'_Reference20._Reference20_5NG', N'_Reference20_5', 'INDEX';

-- Rename: rename index _Reference20_S_HPKNG
EXEC sp_rename N'_Reference20._Reference20_S_HPKNG', N'_Reference20_S_HPK', 'INDEX';

-- Rename: rename index _Reference20_VT155_SKNG
EXEC sp_rename N'_Reference20_VT155._Reference20_VT155_SKNG', N'_Reference20_VT155_SK', 'INDEX';

-- Rename: rename index _Reference20_VT159_SKNG
EXEC sp_rename N'_Reference20_VT159._Reference20_VT159_SKNG', N'_Reference20_VT159_SK', 'INDEX';

-- Rename: rename index _Reference20_VT159_1NG
EXEC sp_rename N'_Reference20_VT159._Reference20_VT159_1NG', N'_Reference20_VT159_1', 'INDEX';

-- Rename: rename index _Reference20_VT159_2NG
EXEC sp_rename N'_Reference20_VT159._Reference20_VT159_2NG', N'_Reference20_VT159_2', 'INDEX';

-- Publish: SchemaStorage: idle, the new schema, empty generations
UPDATE dbo.SchemaStorage SET Status = 100, CurrentSchema = @P1, NewGenCreated = @P2, NewGenDropped = @P3 WHERE SchemaID = 0;

-- Publish: DBSchema
UPDATE dbo.DBSchema SET SerializedData = @P1;

-- Publish: Params DBNames
UPDATE dbo.Params SET BinaryData = @P1, DataSize = @P2, Modified = DATEADD(YEAR, 2000, SYSDATETIME()) WHERE FileName = N'DBNames' AND PartNo = 0;

-- Publish: Params DBNamesVersion-DBNames
UPDATE dbo.Params SET BinaryData = @P1, DataSize = @P2, Modified = DATEADD(YEAR, 2000, SYSDATETIME()) WHERE FileName = N'DBNamesVersion-DBNames' AND PartNo = 0;

-- Publish: Params ea13a2c9-0c2f-40fa-b855-710387e3271d.si: the XDTO model with the new properties
UPDATE dbo.Params SET BinaryData = @P1, DataSize = @P2, Modified = DATEADD(YEAR, 2000, SYSDATETIME()) WHERE FileName = N'ea13a2c9-0c2f-40fa-b855-710387e3271d.si' AND PartNo = 0;

-- Publish: Config: the changed staged rows replace the stored ones
DELETE c FROM dbo.Config c WHERE c.FileName IN (SELECT FileName FROM dbo.ConfigSave WHERE FileName <> N'deleted') AND NOT EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE s.FileName = c.FileName AND s.PartNo = c.PartNo AND s.Creation = c.Creation AND s.Modified = c.Modified AND s.Attributes = c.Attributes AND s.DataSize = c.DataSize AND s.BinaryData = c.BinaryData);
INSERT INTO dbo.Config (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) SELECT s.FileName, s.Creation, s.Modified, s.Attributes, s.DataSize, s.BinaryData, s.PartNo FROM dbo.ConfigSave s WHERE s.FileName <> N'deleted' AND NOT EXISTS (SELECT 1 FROM dbo.Config c WHERE c.FileName = s.FileName AND c.PartNo = s.PartNo);

-- Publish: ConfigSave: emptied
DELETE FROM dbo.ConfigSave;

