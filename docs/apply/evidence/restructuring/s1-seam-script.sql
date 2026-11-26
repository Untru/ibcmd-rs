-- The transaction of `ibcmd-rs mssql-restructure --through-apply` on ibcmd_rs_04_ddl_s1_t1_seam (the types case, S1 step 1),
-- as `--script-output` wrote it: the apply's script with the structure phase between `DECLARE @now` and the fold of the
-- dynamic generations. Binary literals are shortened to their first 12 bytes and the length. Full size 9303611 bytes.
SET NOCOUNT ON;
SET XACT_ABORT ON;
SET LOCK_TIMEOUT 30000;
USE [ibcmd_rs_04_ddl_s1_t1_seam];
SET TRANSACTION ISOLATION LEVEL SERIALIZABLE;
BEGIN TRY
BEGIN TRANSACTION;
DECLARE @r int, @touch nvarchar(256), @n bigint, @b bigint, @h1 bigint, @h2 bigint, @h3 bigint;
EXEC @r = sys.sp_getapplock @Resource = N'ibcmd-rs:config-apply', @LockMode = 'Exclusive', @LockOwner = 'Transaction', @LockTimeout = 0;
IF @r < 0 THROW 57300, N'the config apply application lock is busy', 1;
SELECT TOP (1) @touch = FileName FROM dbo.Config WITH (TABLOCKX, HOLDLOCK) ORDER BY FileName;
SELECT TOP (1) @touch = FileName FROM dbo.ConfigSave WITH (TABLOCKX, HOLDLOCK) ORDER BY FileName;
SELECT TOP (1) @touch = FileName FROM dbo.Params WITH (TABLOCKX, HOLDLOCK) ORDER BY FileName;
SELECT TOP (1) @touch = FileName FROM dbo.Files WITH (TABLOCKX, HOLDLOCK) ORDER BY FileName;
SELECT TOP (1) @touch = NULL FROM dbo._ConfigChngR WITH (TABLOCKX, HOLDLOCK);
IF HAS_PERMS_BY_NAME(NULL, NULL, N'VIEW SERVER STATE') <> 1 THROW 57301, N'exclusive access cannot be proven without VIEW SERVER STATE', 1;
IF EXISTS (SELECT 1 FROM sys.dm_exec_sessions WHERE is_user_process = 1 AND session_id <> @@SPID AND database_id = DB_ID() AND ISNULL(host_process_id, -1) <> 68356) THROW 57302, N'another session is connected to the database; the apply needs exclusive access', 1;
IF EXISTS (SELECT 1 FROM dbo.Config WHERE FileName IN (N'commit', N'dynamicCommit', N'dbStruFinal', N'convertPhase', N'erase_save', N'deleted') OR FileName LIKE N'%.new') OR EXISTS (SELECT 1 FROM dbo.ConfigSave WHERE FileName IN (N'commit', N'dynamicCommit', N'dbStruFinal', N'convertPhase', N'erase_save', N'deleted') OR FileName LIKE N'%.new') THROW 57307, N'an unfinished operation is recorded in Config or ConfigSave; run the native config repair first', 1;
IF EXISTS (SELECT 1 FROM dbo.SchemaStorage WHERE Status <> 100) THROW 57316, N'SchemaStorage is not in the settled state (Status 100): an interrupted restructuring; run the native config repair first', 1;
SELECT @n = q.n, @b = q.bytes, @h1 = q.h1, @h2 = q.h2, @h3 = q.h3 FROM (SELECT COUNT_BIG(*) AS n, ISNULL(SUM(CONVERT(bigint, t.DataSize)), 0) AS bytes, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 1, 4) AS int))), 0) AS h1, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 5, 4) AS int))), 0) AS h2, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 9, 4) AS int))), 0) AS h3 FROM (SELECT s.DataSize AS DataSize, HASHBYTES('SHA2_256', CONCAT(s.FileName, N'|', s.PartNo, N'|', s.DataSize, N'|', CONVERT(nvarchar(64), HASHBYTES('SHA2_256', s.BinaryData), 2))) AS d FROM dbo.ConfigSave s) t) q;
IF @n <> 9 OR @b <> 361423 OR @h1 <> 3238647714 OR @h2 <> -1772240543 OR @h3 <> -2601945399 THROW 57303, N'ConfigSave changed since the plan was made (ConfigSave)', 1;
SELECT @n = q.n, @b = q.bytes, @h1 = q.h1, @h2 = q.h2, @h3 = q.h3 FROM (SELECT COUNT_BIG(*) AS n, ISNULL(SUM(CONVERT(bigint, t.DataSize)), 0) AS bytes, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 1, 4) AS int))), 0) AS h1, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 5, 4) AS int))), 0) AS h2, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 9, 4) AS int))), 0) AS h3 FROM (SELECT s.DataSize AS DataSize, HASHBYTES('SHA2_256', CONCAT(s.FileName, N'|', s.PartNo, N'|', s.DataSize, N'|', CONVERT(nvarchar(64), HASHBYTES('SHA2_256', s.BinaryData), 2))) AS d FROM dbo.Config s WHERE EXISTS (SELECT 1 FROM dbo.ConfigSave x WHERE x.FileName = s.FileName)) t) q;
IF @n <> 9 OR @b <> 359656 OR @h1 <> 2104126205 OR @h2 <> -878293654 OR @h3 <> -385302856 THROW 57304, N'the Config rows to replace changed since the plan was made (Config)', 1;
SELECT @n = q.n, @b = q.bytes, @h1 = q.h1, @h2 = q.h2, @h3 = q.h3 FROM (SELECT COUNT_BIG(*) AS n, ISNULL(SUM(CONVERT(bigint, t.DataSize)), 0) AS bytes, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 1, 4) AS int))), 0) AS h1, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 5, 4) AS int))), 0) AS h2, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 9, 4) AS int))), 0) AS h3 FROM (SELECT s.DataSize AS DataSize, HASHBYTES('SHA2_256', CONCAT(s.FileName, N'|', s.PartNo, N'|', s.DataSize, N'|', CONVERT(nvarchar(64), HASHBYTES('SHA2_256', s.BinaryData), 2))) AS d FROM dbo.Config s WHERE s.FileName = N'DynamicallyUpdated' OR s.FileName LIKE N'%!_dynupdate!_%' ESCAPE N'!') t) q;
IF @n <> 6 OR @b <> 344741 OR @h1 <> 2370674287 OR @h2 <> 2083817027 OR @h3 <> 323847018 THROW 57305, N'the dynamic-update rows of Config changed since the plan was made (Config markers)', 1;
SELECT @n = q.n, @b = q.bytes, @h1 = q.h1, @h2 = q.h2, @h3 = q.h3 FROM (SELECT COUNT_BIG(*) AS n, ISNULL(SUM(CONVERT(bigint, t.DataSize)), 0) AS bytes, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 1, 4) AS int))), 0) AS h1, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 5, 4) AS int))), 0) AS h2, ISNULL(SUM(CONVERT(bigint, CAST(SUBSTRING(t.d, 9, 4) AS int))), 0) AS h3 FROM (SELECT s.DataSize AS DataSize, HASHBYTES('SHA2_256', CONCAT(s.FileName, N'|', s.PartNo, N'|', s.DataSize, N'|', CONVERT(nvarchar(64), HASHBYTES('SHA2_256', s.BinaryData), 2))) AS d FROM dbo.Params s WHERE s.FileName = N'DynamicallyUpdated') t) q;
IF @n <> 1 OR @b <> 82 OR @h1 <> -556815054 OR @h2 <> -960225838 OR @h3 <> -1403494357 THROW 57305, N'Params.DynamicallyUpdated changed since the plan was made (Params marker)', 1;
DECLARE @offset int = ISNULL((SELECT TOP (1) Offset FROM dbo._YearOffset), 0);
DECLARE @now datetime2(6) = DATEADD(year, @offset, CONVERT(datetime2(6), SYSDATETIME()));
-- structure phase
-- restructure: guards
IF NOT EXISTS (SELECT 1 FROM dbo.SchemaStorage WHERE SchemaID = 0 AND Status = 100 AND DATALENGTH(NewGenCreated) = 14 AND DATALENGTH(NewGenDropped) = 14 AND CONVERT(varchar(64), HASHBYTES('SHA2_256', CurrentSchema), 2) = 'BA56D560791813EE2063F55501D866DD05EFDA500D6EEA9A0AD97F1DB094AAEF') THROW 57400, N'SchemaStorage is not idle or its schema is not the one the restructure was planned from', 1;
IF (SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName = N'DBNames' AND PartNo = 0 AND DATALENGTH(BinaryData) = 143619 AND CONVERT(varchar(64), HASHBYTES('SHA2_256', BinaryData), 2) = 'E1D4860828F09A2F30D333E8B97F29180A5C7573F226BFE60DCCEFD7BB2233F8') <> 1 THROW 57402, N'Params DBNames changed since the restructure was planned', 1;
IF (SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName = N'DBNamesVersion-DBNames' AND PartNo = 0) <> 1 THROW 57402, N'Params DBNamesVersion-DBNames is missing', 1;
IF OBJECT_ID(N'dbo._Reference569', N'U') IS NULL OR OBJECT_ID(N'dbo._Reference569NG', N'U') IS NOT NULL THROW 57401, N'the table _Reference569 is missing or _Reference569NG is left over', 1;
IF OBJECT_ID(N'dbo._Reference16', N'U') IS NULL OR OBJECT_ID(N'dbo._Reference16NG', N'U') IS NOT NULL THROW 57401, N'the table _Reference16 is missing or _Reference16NG is left over', 1;
IF OBJECT_ID(N'dbo._Reference20', N'U') IS NULL OR OBJECT_ID(N'dbo._Reference20NG', N'U') IS NOT NULL THROW 57401, N'the table _Reference20 is missing or _Reference20NG is left over', 1;
IF OBJECT_ID(N'dbo._Reference20_VT155', N'U') IS NULL OR OBJECT_ID(N'dbo._Reference20_VT155NG', N'U') IS NOT NULL THROW 57401, N'the table _Reference20_VT155 is missing or _Reference20_VT155NG is left over', 1;
IF OBJECT_ID(N'dbo._Reference20_VT159', N'U') IS NULL OR OBJECT_ID(N'dbo._Reference20_VT159NG', N'U') IS NOT NULL THROW 57401, N'the table _Reference20_VT159 is missing or _Reference20_VT159NG is left over', 1;
IF OBJECT_ID(N'dbo._Reference2598', N'U') IS NULL OR OBJECT_ID(N'dbo._Reference2598NG', N'U') IS NOT NULL THROW 57401, N'the table _Reference2598 is missing or _Reference2598NG is left over', 1;
IF OBJECT_ID(N'dbo._Reference9367', N'U') IS NULL OR OBJECT_ID(N'dbo._Reference9367NG', N'U') IS NOT NULL THROW 57401, N'the table _Reference9367 is missing or _Reference9367NG is left over', 1;
IF OBJECT_ID(N'dbo._Document39', N'U') IS NULL OR OBJECT_ID(N'dbo._Document39NG', N'U') IS NOT NULL THROW 57401, N'the table _Document39 is missing or _Document39NG is left over', 1;
IF OBJECT_ID(N'dbo._Document39_VT970', N'U') IS NULL OR OBJECT_ID(N'dbo._Document39_VT970NG', N'U') IS NOT NULL THROW 57401, N'the table _Document39_VT970 is missing or _Document39_VT970NG is left over', 1;
IF OBJECT_ID(N'dbo._Document39_VT2148', N'U') IS NULL OR OBJECT_ID(N'dbo._Document39_VT2148NG', N'U') IS NOT NULL THROW 57401, N'the table _Document39_VT2148 is missing or _Document39_VT2148NG is left over', 1;
IF OBJECT_ID(N'dbo._Document39_VT3663', N'U') IS NULL OR OBJECT_ID(N'dbo._Document39_VT3663NG', N'U') IS NOT NULL THROW 57401, N'the table _Document39_VT3663 is missing or _Document39_VT3663NG is left over', 1;
-- restructure: the new generation
create table dbo._Reference569NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_ParentIDRRef binary(16) not null,
_Folder binary(1) not null,
_Description nvarchar(150) not null,
_Fld4383RRef binary(16),
_Fld11034 binary(1),
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference569NG SET (LOCK_ESCALATION = DISABLE);
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
_Fld11035 datetime2(0),
_Fld11036 numeric(10, 2) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference16NG SET (LOCK_ESCALATION = DISABLE);
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
_Fld11037 nvarchar(30),
_Fld11038 datetime2(0) not null,
_Fld11039 numeric(5, 0),
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Reference20NG SET (LOCK_ESCALATION = DISABLE);
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
create table dbo._Reference2598NG (
_IDRRef binary(16) not null primary key,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_Description nvarchar(150) not null,
_Fld4266 nvarchar(1000) not null,
_Fld2644RRef binary(16) not null,
_Fld2645 numeric(15, 0) not null,
_Fld2646 numeric(15, 2) not null,
_Fld5283 nvarchar(40) not null,
_Fld5921 binary(1) not null,
_Fld6495 binary(1) not null,
_Fld11040 binary(1) not null,
_Fld11041 nvarchar(10) not null,
_Fld11042 nchar(20) not null,
_Fld11043 nvarchar(max) not null,
_Fld11044 numeric(10, 0) not null,
_Fld11045 numeric(12, 3) not null,
_Fld11046 numeric(5, 0) not null,
_Fld11047 datetime2(0) not null,
_Fld11048 datetime2(0) not null
)
;alter table dbo._Reference2598NG SET (LOCK_ESCALATION = DISABLE);
create table dbo._Reference9367NG (
_IDRRef binary(16) not null primary key,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_Code nvarchar(9) not null,
_Description nvarchar(25) not null,
_Fld11049 nvarchar(15) not null
)
;alter table dbo._Reference9367NG SET (LOCK_ESCALATION = DISABLE);
create table dbo._Document39NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_Date_Time datetime2(0) not null,
_Number nvarchar(11) not null,
_Posted binary(1) not null,
_Fld3822 nvarchar(max) not null,
_Fld5547 nvarchar(200) not null,
_Fld2575RRef binary(16) not null,
_Fld3825 nvarchar(50) not null,
_Fld3827 datetime2(0) not null,
_Fld11050 numeric(3, 0) not null,
_Fld2909RRef binary(16) not null,
_Fld3828 nvarchar(100) not null,
_Fld1516 nvarchar(max) not null,
_Fld6427RRef binary(16) not null,
_Fld2908RRef binary(16) not null,
_Fld968RRef binary(16) not null,
_Fld4336RRef binary(16) not null,
_Fld969RRef binary(16) not null,
_Fld3824 nvarchar(50) not null,
_Fld3821RRef binary(16) not null,
_Fld3823RRef binary(16) not null,
_Fld2574 numeric(15, 2) not null,
_Fld6309 binary(1) not null,
_Fld2147 binary(1) not null,
_Fld6310RRef binary(16) not null,
_Fld3826 nvarchar(max) not null,
_Fld5548 nvarchar(200) not null,
_Fld11051 binary(1) not null,
_Fld11052 nvarchar(40) not null,
_Fld11053 numeric(15, 3) not null,
_Fld11054 datetime2(0) not null,
_Fld11055 datetime2(0) not null,
_Fld11056 nvarchar(max) not null,
_Fld2683 numeric(7, 0) not null
)
;alter table dbo._Document39NG SET (LOCK_ESCALATION = DISABLE);
create table dbo._Document39_VT970NG (
_Document39_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo971 numeric(5, 0) not null,
_Fld972RRef binary(16) not null
)
;alter table dbo._Document39_VT970NG SET (LOCK_ESCALATION = DISABLE);
create table dbo._Document39_VT2148NG (
_Document39_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo2149 numeric(5, 0) not null,
_Fld2150RRef binary(16) not null,
_Fld2151RRef binary(16) not null,
_Fld2152 nvarchar(max) not null,
_Fld3662 numeric(7, 0) not null
)
;alter table dbo._Document39_VT2148NG SET (LOCK_ESCALATION = DISABLE);
create table dbo._Document39_VT3663NG (
_Document39_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo3664 numeric(5, 0) not null,
_Fld3665RRef binary(16) not null,
_Fld3666RRef binary(16) not null,
_Fld3667 nvarchar(500) not null,
_Fld3668 nvarchar(max) not null,
_Fld3669 nvarchar(100) not null,
_Fld3670 nvarchar(50) not null,
_Fld3671 nvarchar(50) not null,
_Fld3672 nvarchar(100) not null,
_Fld3673 nvarchar(100) not null,
_Fld3674 nvarchar(20) not null,
_Fld3675 nvarchar(20) not null,
_Fld3676 numeric(7, 0) not null,
_Fld6749 nvarchar(max) not null
)
;alter table dbo._Document39_VT3663NG SET (LOCK_ESCALATION = DISABLE);
INSERT INTO dbo._Reference569NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Description, _Fld4383RRef, _Fld11034, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._ParentIDRRef,
T1._Folder,
T1._Description,
T1._Fld4383RRef,
CAST(CASE WHEN T1._Folder = 0x01 THEN 0x00 END AS BINARY(1)),
T1._Fld2683
FROM dbo._Reference569 T1 WITH(NOLOCK);
INSERT INTO dbo._Reference16NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Code, _Description, _Fld121RRef, _Fld11035, _Fld11036, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._ParentIDRRef,
T1._Folder,
T1._Code,
T1._Description,
T1._Fld121RRef,
CAST(CASE WHEN T1._Folder = 0x01 THEN CAST('2001-01-01T00:00:00' AS DATETIME2(0)) END AS DATETIME2(0)),
CAST(0 AS NUMERIC(10, 2)),
T1._Fld2683
FROM dbo._Reference16 T1 WITH(NOLOCK);
INSERT INTO dbo._Reference20NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _ParentIDRRef, _Folder, _Code, _Description, _Fld151, _Fld152, _Fld153, _Fld154, _Fld949RRef, _Fld6357RRef, _Fld11037, _Fld11038, _Fld11039, _Fld2683) SELECT
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
CAST(CASE WHEN T1._Folder = 0x01 THEN N'' END AS NVARCHAR(30)),
CAST('2001-01-01T00:00:00' AS DATETIME2(0)),
CAST(CASE WHEN T1._Folder = 0x00 THEN CAST(0 AS NUMERIC(5, 0)) END AS NUMERIC(5, 0)),
T1._Fld2683
FROM dbo._Reference20 T1 WITH(NOLOCK);
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
INSERT INTO dbo._Reference2598NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _Description, _Fld4266, _Fld2644RRef, _Fld2645, _Fld2646, _Fld5283, _Fld5921, _Fld6495, _Fld11040, _Fld11041, _Fld11042, _Fld11043, _Fld11044, _Fld11045, _Fld11046, _Fld11047, _Fld11048) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._Description,
T1._Fld4266,
T1._Fld2644RRef,
T1._Fld2645,
T1._Fld2646,
T1._Fld5283,
T1._Fld5921,
T1._Fld6495,
0x00,
N'',
CAST(N'                    ' AS NCHAR(20)),
N'',
CAST(0 AS NUMERIC(10, 0)),
CAST(0 AS NUMERIC(12, 3)),
CAST(0 AS NUMERIC(5, 0)),
CAST('2001-01-01T00:00:00' AS DATETIME2(0)),
CAST('2001-01-01T00:00:00' AS DATETIME2(0))
FROM dbo._Reference2598 T1 WITH(NOLOCK);
INSERT INTO dbo._Reference9367NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _Code, _Description, _Fld11049) SELECT
T1._IDRRef,
T1._Marked,
T1._PredefinedID,
T1._Code,
T1._Description,
N''
FROM dbo._Reference9367 T1 WITH(NOLOCK);
INSERT INTO dbo._Document39NG WITH(TABLOCK) (_IDRRef, _Marked, _Date_Time, _Number, _Posted, _Fld3822, _Fld5547, _Fld2575RRef, _Fld3825, _Fld3827, _Fld11050, _Fld2909RRef, _Fld3828, _Fld1516, _Fld6427RRef, _Fld2908RRef, _Fld968RRef, _Fld4336RRef, _Fld969RRef, _Fld3824, _Fld3821RRef, _Fld3823RRef, _Fld2574, _Fld6309, _Fld2147, _Fld6310RRef, _Fld3826, _Fld5548, _Fld11051, _Fld11052, _Fld11053, _Fld11054, _Fld11055, _Fld11056, _Fld2683) SELECT
T1._IDRRef,
T1._Marked,
T1._Date_Time,
T1._Number,
T1._Posted,
T1._Fld3822,
T1._Fld5547,
T1._Fld2575RRef,
T1._Fld3825,
T1._Fld3827,
CAST(0 AS NUMERIC(3, 0)),
T1._Fld2909RRef,
T1._Fld3828,
T1._Fld1516,
T1._Fld6427RRef,
T1._Fld2908RRef,
T1._Fld968RRef,
T1._Fld4336RRef,
T1._Fld969RRef,
T1._Fld3824,
T1._Fld3821RRef,
T1._Fld3823RRef,
T1._Fld2574,
T1._Fld6309,
T1._Fld2147,
T1._Fld6310RRef,
T1._Fld3826,
T1._Fld5548,
0x00,
N'',
CAST(0 AS NUMERIC(15, 3)),
CAST('2001-01-01T00:00:00' AS DATETIME2(0)),
CAST('2001-01-01T00:00:00' AS DATETIME2(0)),
N'',
T1._Fld2683
FROM dbo._Document39 T1 WITH(NOLOCK);
INSERT INTO dbo._Document39_VT970NG WITH(TABLOCK) (_LineNo971, _Fld972RRef, _Fld2683, _Document39_IDRRef, _KeyField) SELECT
T2._LineNo971,
T2._Fld972RRef,
T2._Fld2683,
T2._Document39_IDRRef,
T2._KeyField
FROM dbo._Document39_VT970 T2 WITH(NOLOCK);
INSERT INTO dbo._Document39_VT2148NG WITH(TABLOCK) (_LineNo2149, _Fld2150RRef, _Fld2151RRef, _Fld2152, _Fld3662, _Fld2683, _Document39_IDRRef, _KeyField) SELECT
T3._LineNo2149,
T3._Fld2150RRef,
T3._Fld2151RRef,
T3._Fld2152,
T3._Fld3662,
T3._Fld2683,
T3._Document39_IDRRef,
T3._KeyField
FROM dbo._Document39_VT2148 T3 WITH(NOLOCK);
INSERT INTO dbo._Document39_VT3663NG WITH(TABLOCK) (_LineNo3664, _Fld3665RRef, _Fld3666RRef, _Fld3667, _Fld3668, _Fld3669, _Fld3670, _Fld3671, _Fld3672, _Fld3673, _Fld3674, _Fld3675, _Fld3676, _Fld6749, _Fld2683, _Document39_IDRRef, _KeyField) SELECT
T4._LineNo3664,
T4._Fld3665RRef,
T4._Fld3666RRef,
T4._Fld3667,
T4._Fld3668,
T4._Fld3669,
T4._Fld3670,
T4._Fld3671,
T4._Fld3672,
T4._Fld3673,
T4._Fld3674,
T4._Fld3675,
T4._Fld3676,
T4._Fld6749,
T4._Fld2683,
T4._Document39_IDRRef,
T4._KeyField
FROM dbo._Document39_VT3663 T4 WITH(NOLOCK);
IF (SELECT COUNT_BIG(*) FROM dbo._Reference569) <> (SELECT COUNT_BIG(*) FROM dbo._Reference569NG) THROW 57403, N'the copy of _Reference569 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Reference16) <> (SELECT COUNT_BIG(*) FROM dbo._Reference16NG) THROW 57403, N'the copy of _Reference16 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Reference20) <> (SELECT COUNT_BIG(*) FROM dbo._Reference20NG) THROW 57403, N'the copy of _Reference20 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Reference20_VT155) <> (SELECT COUNT_BIG(*) FROM dbo._Reference20_VT155NG) THROW 57403, N'the copy of _Reference20_VT155 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Reference20_VT159) <> (SELECT COUNT_BIG(*) FROM dbo._Reference20_VT159NG) THROW 57403, N'the copy of _Reference20_VT159 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Reference2598) <> (SELECT COUNT_BIG(*) FROM dbo._Reference2598NG) THROW 57403, N'the copy of _Reference2598 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Reference9367) <> (SELECT COUNT_BIG(*) FROM dbo._Reference9367NG) THROW 57403, N'the copy of _Reference9367 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Document39) <> (SELECT COUNT_BIG(*) FROM dbo._Document39NG) THROW 57403, N'the copy of _Document39 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Document39_VT970) <> (SELECT COUNT_BIG(*) FROM dbo._Document39_VT970NG) THROW 57403, N'the copy of _Document39_VT970 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Document39_VT2148) <> (SELECT COUNT_BIG(*) FROM dbo._Document39_VT2148NG) THROW 57403, N'the copy of _Document39_VT2148 has another number of rows', 1;
IF (SELECT COUNT_BIG(*) FROM dbo._Document39_VT3663) <> (SELECT COUNT_BIG(*) FROM dbo._Document39_VT3663NG) THROW 57403, N'the copy of _Document39_VT3663 has another number of rows', 1;
CREATE INDEX _Reference569_1NG ON dbo._Reference569NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference569_2NG ON dbo._Reference569NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference569_3NG ON dbo._Reference569NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE CLUSTERED INDEX _Reference569_S_HPKNG ON dbo._Reference569NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE INDEX _Reference16_1NG ON dbo._Reference16NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference16_2NG ON dbo._Reference16NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference16_3NG ON dbo._Reference16NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference16_4NG ON dbo._Reference16NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference16_5NG ON dbo._Reference16NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE CLUSTERED INDEX _Reference16_S_HPKNG ON dbo._Reference16NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE INDEX _Reference20_1NG ON dbo._Reference20NG (_Fld2683, _PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference20_2NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference20_3NG ON dbo._Reference20NG (_Fld2683, _ParentIDRRef, _Folder, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference20_4NG ON dbo._Reference20NG (_Fld2683, _Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference20_5NG ON dbo._Reference20NG (_Fld2683, _Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE CLUSTERED INDEX _Reference20_S_HPKNG ON dbo._Reference20NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT155_SKNG ON dbo._Reference20_VT155NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE UNIQUE CLUSTERED INDEX _Reference20_VT159_SKNG ON dbo._Reference20_VT159NG (_Fld2683, _Reference20_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE INDEX _Reference20_VT159_1NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld161RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE INDEX _Reference20_VT159_2NG ON dbo._Reference20_VT159NG (_Fld2683, _Fld162RRef, _Reference20_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE INDEX _Reference2598_1NG ON dbo._Reference2598NG (_PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference2598_2NG ON dbo._Reference2598NG (_Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference2598_3NG ON dbo._Reference2598NG (_Fld2645, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference2598_4NG ON dbo._Reference2598NG (_Fld5283, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference2598_5NG ON dbo._Reference2598NG (_Fld5921, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference2598_6NG ON dbo._Reference2598NG (_Fld6495, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE INDEX _Reference9367_1NG ON dbo._Reference9367NG (_PredefinedID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference9367_2NG ON dbo._Reference9367NG (_Code, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Reference9367_3NG ON dbo._Reference9367NG (_Description, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Document39_1NG ON dbo._Document39NG (_Fld2683, _Number, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Document39_2NG ON dbo._Document39NG (_Fld2683, _Date_Time, _IDRRef, _Marked) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Document39_3NG ON dbo._Document39NG (_Fld2683, _Fld2909RRef, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE INDEX _Document39_4NG ON dbo._Document39NG (_Fld2683, _Fld3821RRef, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE CLUSTERED INDEX _Document39_S_HPKNG ON dbo._Document39NG (_Fld2683, _IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE UNIQUE CLUSTERED INDEX _Document39_VT970_SKNG ON dbo._Document39_VT970NG (_Fld2683, _Document39_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE INDEX _Document39_VT970_1NG ON dbo._Document39_VT970NG (_Fld2683, _Fld972RRef, _Document39_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE UNIQUE CLUSTERED INDEX _Document39_VT2148_SKNG ON dbo._Document39_VT2148NG (_Fld2683, _Document39_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE UNIQUE CLUSTERED INDEX _Document39_VT3663_SKNG ON dbo._Document39_VT3663NG (_Fld2683, _Document39_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON);
CREATE INDEX _Document39_VT3663_1NG ON dbo._Document39_VT3663NG (_Fld2683, _Fld3665RRef, _Document39_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
CREATE INDEX _Document39_VT3663_2NG ON dbo._Document39_VT3663NG (_Fld2683, _Fld3666RRef, _Document39_IDRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY];
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference569NG')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_ParentIDRRef|binary|16|0|0|0;_Folder|binary|1|0|0|0;_Description|nvarchar|300|0|0|0;_Fld4383RRef|binary|16|0|0|1;_Fld11034|binary|1|0|0|1;_Fld2683|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Reference569NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference569NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference569_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_PredefinedID') THROW 57404, N'dbo._Reference569NG lacks the index _Reference569_1NG (_Fld2683,_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference569NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference569_2NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef') THROW 57404, N'dbo._Reference569NG lacks the index _Reference569_2NG (_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference569NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference569_3NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Description,_IDRRef') THROW 57404, N'dbo._Reference569NG lacks the index _Reference569_3NG (_Fld2683,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference569NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference569_S_HPKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_IDRRef') THROW 57404, N'dbo._Reference569NG lacks the index _Reference569_S_HPKNG (_Fld2683,_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference569NG') AND type IN (1, 2)) <> 4 THROW 57404, N'dbo._Reference569NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference16NG')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_ParentIDRRef|binary|16|0|0|0;_Folder|binary|1|0|0|0;_Code|nvarchar|18|0|0|0;_Description|nvarchar|100|0|0|0;_Fld121RRef|binary|16|0|0|1;_Fld11035|datetime2|0|0|0|1;_Fld11036|numeric|0|10|2|0;_Fld2683|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Reference16NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_PredefinedID') THROW 57404, N'dbo._Reference16NG lacks the index _Reference16_1NG (_Fld2683,_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_2NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Code,_IDRRef') THROW 57404, N'dbo._Reference16NG lacks the index _Reference16_2NG (_Fld2683,_ParentIDRRef,_Folder,_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_3NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef') THROW 57404, N'dbo._Reference16NG lacks the index _Reference16_3NG (_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_4NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Code,_IDRRef') THROW 57404, N'dbo._Reference16NG lacks the index _Reference16_4NG (_Fld2683,_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_5NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Description,_IDRRef') THROW 57404, N'dbo._Reference16NG lacks the index _Reference16_5NG (_Fld2683,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_S_HPKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_IDRRef') THROW 57404, N'dbo._Reference16NG lacks the index _Reference16_S_HPKNG (_Fld2683,_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference16NG') AND type IN (1, 2)) <> 6 THROW 57404, N'dbo._Reference16NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference20NG')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_ParentIDRRef|binary|16|0|0|0;_Folder|binary|1|0|0|0;_Code|nvarchar|18|0|0|0;_Description|nvarchar|300|0|0|0;_Fld151|binary|1|0|0|1;_Fld152|binary|1|0|0|1;_Fld153|binary|1|0|0|1;_Fld154|binary|1|0|0|1;_Fld949RRef|binary|16|0|0|1;_Fld6357RRef|binary|16|0|0|1;_Fld11037|nvarchar|60|0|0|1;_Fld11038|datetime2|0|0|0|0;_Fld11039|numeric|0|5|0|1;_Fld2683|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Reference20NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_PredefinedID') THROW 57404, N'dbo._Reference20NG lacks the index _Reference20_1NG (_Fld2683,_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_2NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Code,_IDRRef') THROW 57404, N'dbo._Reference20NG lacks the index _Reference20_2NG (_Fld2683,_ParentIDRRef,_Folder,_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_3NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef') THROW 57404, N'dbo._Reference20NG lacks the index _Reference20_3NG (_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_4NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Code,_IDRRef') THROW 57404, N'dbo._Reference20NG lacks the index _Reference20_4NG (_Fld2683,_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_5NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Description,_IDRRef') THROW 57404, N'dbo._Reference20NG lacks the index _Reference20_5NG (_Fld2683,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_S_HPKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_IDRRef') THROW 57404, N'dbo._Reference20NG lacks the index _Reference20_S_HPKNG (_Fld2683,_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference20NG') AND type IN (1, 2)) <> 6 THROW 57404, N'dbo._Reference20NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference20_VT155NG')), N'') <> N'_Reference20_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo156|numeric|0|5|0|0;_Fld157RRef|binary|16|0|0|0;_Fld158_TYPE|binary|1|0|0|0;_Fld158_L|binary|1|0|0|0;_Fld158_N|numeric|0|17|5|0;_Fld158_T|datetime2|0|0|0|0;_Fld158_S|nvarchar|2048|0|0|0;_Fld158_RTRef|binary|4|0|0|0;_Fld158_RRRef|binary|16|0|0|0;_Fld1473|nvarchar|-1|0|0|0' THROW 57404, N'the columns of dbo._Reference20_VT155NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20_VT155NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_VT155_SKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Reference20_IDRRef,_KeyField') THROW 57404, N'dbo._Reference20_VT155NG lacks the index _Reference20_VT155_SKNG (_Fld2683,_Reference20_IDRRef,_KeyField)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference20_VT155NG') AND type IN (1, 2)) <> 1 THROW 57404, N'dbo._Reference20_VT155NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference20_VT159NG')), N'') <> N'_Reference20_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo160|numeric|0|5|0|0;_Fld161RRef|binary|16|0|0|0;_Fld162RRef|binary|16|0|0|0;_Fld163|nvarchar|1000|0|0|0;_Fld164|nvarchar|-1|0|0|0;_Fld165|nvarchar|200|0|0|0;_Fld166|nvarchar|100|0|0|0;_Fld167|nvarchar|100|0|0|0;_Fld168|nvarchar|200|0|0|0;_Fld169|nvarchar|200|0|0|0;_Fld170|nvarchar|40|0|0|0;_Fld171|nvarchar|40|0|0|0;_Fld5024RRef|binary|16|0|0|0;_Fld6753|nvarchar|-1|0|0|0' THROW 57404, N'the columns of dbo._Reference20_VT159NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20_VT159NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_VT159_SKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Reference20_IDRRef,_KeyField') THROW 57404, N'dbo._Reference20_VT159NG lacks the index _Reference20_VT159_SKNG (_Fld2683,_Reference20_IDRRef,_KeyField)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20_VT159NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_VT159_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld161RRef,_Reference20_IDRRef') THROW 57404, N'dbo._Reference20_VT159NG lacks the index _Reference20_VT159_1NG (_Fld2683,_Fld161RRef,_Reference20_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20_VT159NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_VT159_2NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld162RRef,_Reference20_IDRRef') THROW 57404, N'dbo._Reference20_VT159NG lacks the index _Reference20_VT159_2NG (_Fld2683,_Fld162RRef,_Reference20_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference20_VT159NG') AND type IN (1, 2)) <> 3 THROW 57404, N'dbo._Reference20_VT159NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference2598NG')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_Description|nvarchar|300|0|0|0;_Fld4266|nvarchar|2000|0|0|0;_Fld2644RRef|binary|16|0|0|0;_Fld2645|numeric|0|15|0|0;_Fld2646|numeric|0|15|2|0;_Fld5283|nvarchar|80|0|0|0;_Fld5921|binary|1|0|0|0;_Fld6495|binary|1|0|0|0;_Fld11040|binary|1|0|0|0;_Fld11041|nvarchar|20|0|0|0;_Fld11042|nchar|40|0|0|0;_Fld11043|nvarchar|-1|0|0|0;_Fld11044|numeric|0|10|0|0;_Fld11045|numeric|0|12|3|0;_Fld11046|numeric|0|5|0|0;_Fld11047|datetime2|0|0|0|0;_Fld11048|datetime2|0|0|0|0' THROW 57404, N'the columns of dbo._Reference2598NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_PredefinedID') THROW 57404, N'dbo._Reference2598NG lacks the index _Reference2598_1NG (_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_2NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Description,_IDRRef') THROW 57404, N'dbo._Reference2598NG lacks the index _Reference2598_2NG (_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_3NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2645,_IDRRef') THROW 57404, N'dbo._Reference2598NG lacks the index _Reference2598_3NG (_Fld2645,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_4NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld5283,_IDRRef') THROW 57404, N'dbo._Reference2598NG lacks the index _Reference2598_4NG (_Fld5283,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_5NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld5921,_IDRRef') THROW 57404, N'dbo._Reference2598NG lacks the index _Reference2598_5NG (_Fld5921,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_6NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld6495,_IDRRef') THROW 57404, N'dbo._Reference2598NG lacks the index _Reference2598_6NG (_Fld6495,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_IDRRef') THROW 57404, N'dbo._Reference2598NG lacks the index  (_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference2598NG') AND type IN (1, 2)) <> 7 THROW 57404, N'dbo._Reference2598NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference9367NG')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_Code|nvarchar|18|0|0|0;_Description|nvarchar|50|0|0|0;_Fld11049|nvarchar|30|0|0|0' THROW 57404, N'the columns of dbo._Reference9367NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference9367NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference9367_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_PredefinedID') THROW 57404, N'dbo._Reference9367NG lacks the index _Reference9367_1NG (_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference9367NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference9367_2NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Code,_IDRRef') THROW 57404, N'dbo._Reference9367NG lacks the index _Reference9367_2NG (_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference9367NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference9367_3NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Description,_IDRRef') THROW 57404, N'dbo._Reference9367NG lacks the index _Reference9367_3NG (_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference9367NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_IDRRef') THROW 57404, N'dbo._Reference9367NG lacks the index  (_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference9367NG') AND type IN (1, 2)) <> 4 THROW 57404, N'dbo._Reference9367NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Document39NG')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_Date_Time|datetime2|0|0|0|0;_Number|nvarchar|22|0|0|0;_Posted|binary|1|0|0|0;_Fld3822|nvarchar|-1|0|0|0;_Fld5547|nvarchar|400|0|0|0;_Fld2575RRef|binary|16|0|0|0;_Fld3825|nvarchar|100|0|0|0;_Fld3827|datetime2|0|0|0|0;_Fld11050|numeric|0|3|0|0;_Fld2909RRef|binary|16|0|0|0;_Fld3828|nvarchar|200|0|0|0;_Fld1516|nvarchar|-1|0|0|0;_Fld6427RRef|binary|16|0|0|0;_Fld2908RRef|binary|16|0|0|0;_Fld968RRef|binary|16|0|0|0;_Fld4336RRef|binary|16|0|0|0;_Fld969RRef|binary|16|0|0|0;_Fld3824|nvarchar|100|0|0|0;_Fld3821RRef|binary|16|0|0|0;_Fld3823RRef|binary|16|0|0|0;_Fld2574|numeric|0|15|2|0;_Fld6309|binary|1|0|0|0;_Fld2147|binary|1|0|0|0;_Fld6310RRef|binary|16|0|0|0;_Fld3826|nvarchar|-1|0|0|0;_Fld5548|nvarchar|400|0|0|0;_Fld11051|binary|1|0|0|0;_Fld11052|nvarchar|80|0|0|0;_Fld11053|numeric|0|15|3|0;_Fld11054|datetime2|0|0|0|0;_Fld11055|datetime2|0|0|0|0;_Fld11056|nvarchar|-1|0|0|0;_Fld2683|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Document39NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Number,_IDRRef') THROW 57404, N'dbo._Document39NG lacks the index _Document39_1NG (_Fld2683,_Number,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_2NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Date_Time,_IDRRef,_Marked') THROW 57404, N'dbo._Document39NG lacks the index _Document39_2NG (_Fld2683,_Date_Time,_IDRRef,_Marked)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_3NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld2909RRef,_IDRRef') THROW 57404, N'dbo._Document39NG lacks the index _Document39_3NG (_Fld2683,_Fld2909RRef,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39NG') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_4NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld3821RRef,_IDRRef') THROW 57404, N'dbo._Document39NG lacks the index _Document39_4NG (_Fld2683,_Fld3821RRef,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_S_HPKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_IDRRef') THROW 57404, N'dbo._Document39NG lacks the index _Document39_S_HPKNG (_Fld2683,_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Document39NG') AND type IN (1, 2)) <> 5 THROW 57404, N'dbo._Document39NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Document39_VT970NG')), N'') <> N'_Document39_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo971|numeric|0|5|0|0;_Fld972RRef|binary|16|0|0|0' THROW 57404, N'the columns of dbo._Document39_VT970NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT970NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT970_SKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Document39_IDRRef,_KeyField') THROW 57404, N'dbo._Document39_VT970NG lacks the index _Document39_VT970_SKNG (_Fld2683,_Document39_IDRRef,_KeyField)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT970NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT970_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld972RRef,_Document39_IDRRef') THROW 57404, N'dbo._Document39_VT970NG lacks the index _Document39_VT970_1NG (_Fld2683,_Fld972RRef,_Document39_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Document39_VT970NG') AND type IN (1, 2)) <> 2 THROW 57404, N'dbo._Document39_VT970NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Document39_VT2148NG')), N'') <> N'_Document39_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo2149|numeric|0|5|0|0;_Fld2150RRef|binary|16|0|0|0;_Fld2151RRef|binary|16|0|0|0;_Fld2152|nvarchar|-1|0|0|0;_Fld3662|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Document39_VT2148NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT2148NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT2148_SKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Document39_IDRRef,_KeyField') THROW 57404, N'dbo._Document39_VT2148NG lacks the index _Document39_VT2148_SKNG (_Fld2683,_Document39_IDRRef,_KeyField)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Document39_VT2148NG') AND type IN (1, 2)) <> 1 THROW 57404, N'dbo._Document39_VT2148NG has indexes the model does not', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Document39_VT3663NG')), N'') <> N'_Document39_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo3664|numeric|0|5|0|0;_Fld3665RRef|binary|16|0|0|0;_Fld3666RRef|binary|16|0|0|0;_Fld3667|nvarchar|1000|0|0|0;_Fld3668|nvarchar|-1|0|0|0;_Fld3669|nvarchar|200|0|0|0;_Fld3670|nvarchar|100|0|0|0;_Fld3671|nvarchar|100|0|0|0;_Fld3672|nvarchar|200|0|0|0;_Fld3673|nvarchar|200|0|0|0;_Fld3674|nvarchar|40|0|0|0;_Fld3675|nvarchar|40|0|0|0;_Fld3676|numeric|0|7|0|0;_Fld6749|nvarchar|-1|0|0|0' THROW 57404, N'the columns of dbo._Document39_VT3663NG are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT3663NG') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT3663_SKNG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Document39_IDRRef,_KeyField') THROW 57404, N'dbo._Document39_VT3663NG lacks the index _Document39_VT3663_SKNG (_Fld2683,_Document39_IDRRef,_KeyField)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT3663NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT3663_1NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld3665RRef,_Document39_IDRRef') THROW 57404, N'dbo._Document39_VT3663NG lacks the index _Document39_VT3663_1NG (_Fld2683,_Fld3665RRef,_Document39_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT3663NG') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT3663_2NG' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld3666RRef,_Document39_IDRRef') THROW 57404, N'dbo._Document39_VT3663NG lacks the index _Document39_VT3663_2NG (_Fld2683,_Fld3666RRef,_Document39_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Document39_VT3663NG') AND type IN (1, 2)) <> 3 THROW 57404, N'dbo._Document39_VT3663NG has indexes the model does not', 1;
drop table dbo._Reference569;
drop table dbo._Reference16;
drop table dbo._Reference20;
drop table dbo._Reference20_VT155;
drop table dbo._Reference20_VT159;
drop table dbo._Reference2598;
drop table dbo._Reference9367;
drop table dbo._Document39;
drop table dbo._Document39_VT970;
drop table dbo._Document39_VT2148;
drop table dbo._Document39_VT3663;
EXEC sp_rename N'_Reference569NG', N'_Reference569', 'OBJECT';
EXEC sp_rename N'_Reference16NG', N'_Reference16', 'OBJECT';
EXEC sp_rename N'_Reference20NG', N'_Reference20', 'OBJECT';
EXEC sp_rename N'_Reference20_VT155NG', N'_Reference20_VT155', 'OBJECT';
EXEC sp_rename N'_Reference20_VT159NG', N'_Reference20_VT159', 'OBJECT';
EXEC sp_rename N'_Reference2598NG', N'_Reference2598', 'OBJECT';
EXEC sp_rename N'_Reference9367NG', N'_Reference9367', 'OBJECT';
EXEC sp_rename N'_Document39NG', N'_Document39', 'OBJECT';
EXEC sp_rename N'_Document39_VT970NG', N'_Document39_VT970', 'OBJECT';
EXEC sp_rename N'_Document39_VT2148NG', N'_Document39_VT2148', 'OBJECT';
EXEC sp_rename N'_Document39_VT3663NG', N'_Document39_VT3663', 'OBJECT';
EXEC sp_rename N'_Reference569._Reference569_1NG', N'_Reference569_1', 'INDEX';
EXEC sp_rename N'_Reference569._Reference569_2NG', N'_Reference569_2', 'INDEX';
EXEC sp_rename N'_Reference569._Reference569_3NG', N'_Reference569_3', 'INDEX';
EXEC sp_rename N'_Reference569._Reference569_S_HPKNG', N'_Reference569_S_HPK', 'INDEX';
EXEC sp_rename N'_Reference16._Reference16_1NG', N'_Reference16_1', 'INDEX';
EXEC sp_rename N'_Reference16._Reference16_2NG', N'_Reference16_2', 'INDEX';
EXEC sp_rename N'_Reference16._Reference16_3NG', N'_Reference16_3', 'INDEX';
EXEC sp_rename N'_Reference16._Reference16_4NG', N'_Reference16_4', 'INDEX';
EXEC sp_rename N'_Reference16._Reference16_5NG', N'_Reference16_5', 'INDEX';
EXEC sp_rename N'_Reference16._Reference16_S_HPKNG', N'_Reference16_S_HPK', 'INDEX';
EXEC sp_rename N'_Reference20._Reference20_1NG', N'_Reference20_1', 'INDEX';
EXEC sp_rename N'_Reference20._Reference20_2NG', N'_Reference20_2', 'INDEX';
EXEC sp_rename N'_Reference20._Reference20_3NG', N'_Reference20_3', 'INDEX';
EXEC sp_rename N'_Reference20._Reference20_4NG', N'_Reference20_4', 'INDEX';
EXEC sp_rename N'_Reference20._Reference20_5NG', N'_Reference20_5', 'INDEX';
EXEC sp_rename N'_Reference20._Reference20_S_HPKNG', N'_Reference20_S_HPK', 'INDEX';
EXEC sp_rename N'_Reference20_VT155._Reference20_VT155_SKNG', N'_Reference20_VT155_SK', 'INDEX';
EXEC sp_rename N'_Reference20_VT159._Reference20_VT159_SKNG', N'_Reference20_VT159_SK', 'INDEX';
EXEC sp_rename N'_Reference20_VT159._Reference20_VT159_1NG', N'_Reference20_VT159_1', 'INDEX';
EXEC sp_rename N'_Reference20_VT159._Reference20_VT159_2NG', N'_Reference20_VT159_2', 'INDEX';
EXEC sp_rename N'_Reference2598._Reference2598_1NG', N'_Reference2598_1', 'INDEX';
EXEC sp_rename N'_Reference2598._Reference2598_2NG', N'_Reference2598_2', 'INDEX';
EXEC sp_rename N'_Reference2598._Reference2598_3NG', N'_Reference2598_3', 'INDEX';
EXEC sp_rename N'_Reference2598._Reference2598_4NG', N'_Reference2598_4', 'INDEX';
EXEC sp_rename N'_Reference2598._Reference2598_5NG', N'_Reference2598_5', 'INDEX';
EXEC sp_rename N'_Reference2598._Reference2598_6NG', N'_Reference2598_6', 'INDEX';
EXEC sp_rename N'_Reference9367._Reference9367_1NG', N'_Reference9367_1', 'INDEX';
EXEC sp_rename N'_Reference9367._Reference9367_2NG', N'_Reference9367_2', 'INDEX';
EXEC sp_rename N'_Reference9367._Reference9367_3NG', N'_Reference9367_3', 'INDEX';
EXEC sp_rename N'_Document39._Document39_1NG', N'_Document39_1', 'INDEX';
EXEC sp_rename N'_Document39._Document39_2NG', N'_Document39_2', 'INDEX';
EXEC sp_rename N'_Document39._Document39_3NG', N'_Document39_3', 'INDEX';
EXEC sp_rename N'_Document39._Document39_4NG', N'_Document39_4', 'INDEX';
EXEC sp_rename N'_Document39._Document39_S_HPKNG', N'_Document39_S_HPK', 'INDEX';
EXEC sp_rename N'_Document39_VT970._Document39_VT970_SKNG', N'_Document39_VT970_SK', 'INDEX';
EXEC sp_rename N'_Document39_VT970._Document39_VT970_1NG', N'_Document39_VT970_1', 'INDEX';
EXEC sp_rename N'_Document39_VT2148._Document39_VT2148_SKNG', N'_Document39_VT2148_SK', 'INDEX';
EXEC sp_rename N'_Document39_VT3663._Document39_VT3663_SKNG', N'_Document39_VT3663_SK', 'INDEX';
EXEC sp_rename N'_Document39_VT3663._Document39_VT3663_1NG', N'_Document39_VT3663_1', 'INDEX';
EXEC sp_rename N'_Document39_VT3663._Document39_VT3663_2NG', N'_Document39_VT3663_2', 'INDEX';
IF OBJECT_ID(N'dbo._Reference569NG', N'U') IS NOT NULL THROW 57401, N'_Reference569NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference569')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_ParentIDRRef|binary|16|0|0|0;_Folder|binary|1|0|0|0;_Description|nvarchar|300|0|0|0;_Fld4383RRef|binary|16|0|0|1;_Fld11034|binary|1|0|0|1;_Fld2683|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Reference569 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference569') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference569_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_PredefinedID') THROW 57404, N'dbo._Reference569 lacks the index _Reference569_1 (_Fld2683,_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference569') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference569_2' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef') THROW 57404, N'dbo._Reference569 lacks the index _Reference569_2 (_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference569') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference569_3' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Description,_IDRRef') THROW 57404, N'dbo._Reference569 lacks the index _Reference569_3 (_Fld2683,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference569') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference569_S_HPK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_IDRRef') THROW 57404, N'dbo._Reference569 lacks the index _Reference569_S_HPK (_Fld2683,_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference569') AND type IN (1, 2)) <> 4 THROW 57404, N'dbo._Reference569 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Reference16NG', N'U') IS NOT NULL THROW 57401, N'_Reference16NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference16')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_ParentIDRRef|binary|16|0|0|0;_Folder|binary|1|0|0|0;_Code|nvarchar|18|0|0|0;_Description|nvarchar|100|0|0|0;_Fld121RRef|binary|16|0|0|1;_Fld11035|datetime2|0|0|0|1;_Fld11036|numeric|0|10|2|0;_Fld2683|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Reference16 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_PredefinedID') THROW 57404, N'dbo._Reference16 lacks the index _Reference16_1 (_Fld2683,_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_2' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Code,_IDRRef') THROW 57404, N'dbo._Reference16 lacks the index _Reference16_2 (_Fld2683,_ParentIDRRef,_Folder,_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_3' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef') THROW 57404, N'dbo._Reference16 lacks the index _Reference16_3 (_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_4' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Code,_IDRRef') THROW 57404, N'dbo._Reference16 lacks the index _Reference16_4 (_Fld2683,_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_5' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Description,_IDRRef') THROW 57404, N'dbo._Reference16 lacks the index _Reference16_5 (_Fld2683,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference16') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference16_S_HPK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_IDRRef') THROW 57404, N'dbo._Reference16 lacks the index _Reference16_S_HPK (_Fld2683,_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference16') AND type IN (1, 2)) <> 6 THROW 57404, N'dbo._Reference16 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Reference20NG', N'U') IS NOT NULL THROW 57401, N'_Reference20NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference20')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_ParentIDRRef|binary|16|0|0|0;_Folder|binary|1|0|0|0;_Code|nvarchar|18|0|0|0;_Description|nvarchar|300|0|0|0;_Fld151|binary|1|0|0|1;_Fld152|binary|1|0|0|1;_Fld153|binary|1|0|0|1;_Fld154|binary|1|0|0|1;_Fld949RRef|binary|16|0|0|1;_Fld6357RRef|binary|16|0|0|1;_Fld11037|nvarchar|60|0|0|1;_Fld11038|datetime2|0|0|0|0;_Fld11039|numeric|0|5|0|1;_Fld2683|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Reference20 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_PredefinedID') THROW 57404, N'dbo._Reference20 lacks the index _Reference20_1 (_Fld2683,_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_2' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Code,_IDRRef') THROW 57404, N'dbo._Reference20 lacks the index _Reference20_2 (_Fld2683,_ParentIDRRef,_Folder,_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_3' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef') THROW 57404, N'dbo._Reference20 lacks the index _Reference20_3 (_Fld2683,_ParentIDRRef,_Folder,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_4' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Code,_IDRRef') THROW 57404, N'dbo._Reference20 lacks the index _Reference20_4 (_Fld2683,_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_5' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Description,_IDRRef') THROW 57404, N'dbo._Reference20 lacks the index _Reference20_5 (_Fld2683,_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_S_HPK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_IDRRef') THROW 57404, N'dbo._Reference20 lacks the index _Reference20_S_HPK (_Fld2683,_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference20') AND type IN (1, 2)) <> 6 THROW 57404, N'dbo._Reference20 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Reference20_VT155NG', N'U') IS NOT NULL THROW 57401, N'_Reference20_VT155NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference20_VT155')), N'') <> N'_Reference20_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo156|numeric|0|5|0|0;_Fld157RRef|binary|16|0|0|0;_Fld158_TYPE|binary|1|0|0|0;_Fld158_L|binary|1|0|0|0;_Fld158_N|numeric|0|17|5|0;_Fld158_T|datetime2|0|0|0|0;_Fld158_S|nvarchar|2048|0|0|0;_Fld158_RTRef|binary|4|0|0|0;_Fld158_RRRef|binary|16|0|0|0;_Fld1473|nvarchar|-1|0|0|0' THROW 57404, N'the columns of dbo._Reference20_VT155 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20_VT155') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_VT155_SK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Reference20_IDRRef,_KeyField') THROW 57404, N'dbo._Reference20_VT155 lacks the index _Reference20_VT155_SK (_Fld2683,_Reference20_IDRRef,_KeyField)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference20_VT155') AND type IN (1, 2)) <> 1 THROW 57404, N'dbo._Reference20_VT155 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Reference20_VT159NG', N'U') IS NOT NULL THROW 57401, N'_Reference20_VT159NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference20_VT159')), N'') <> N'_Reference20_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo160|numeric|0|5|0|0;_Fld161RRef|binary|16|0|0|0;_Fld162RRef|binary|16|0|0|0;_Fld163|nvarchar|1000|0|0|0;_Fld164|nvarchar|-1|0|0|0;_Fld165|nvarchar|200|0|0|0;_Fld166|nvarchar|100|0|0|0;_Fld167|nvarchar|100|0|0|0;_Fld168|nvarchar|200|0|0|0;_Fld169|nvarchar|200|0|0|0;_Fld170|nvarchar|40|0|0|0;_Fld171|nvarchar|40|0|0|0;_Fld5024RRef|binary|16|0|0|0;_Fld6753|nvarchar|-1|0|0|0' THROW 57404, N'the columns of dbo._Reference20_VT159 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20_VT159') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_VT159_SK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Reference20_IDRRef,_KeyField') THROW 57404, N'dbo._Reference20_VT159 lacks the index _Reference20_VT159_SK (_Fld2683,_Reference20_IDRRef,_KeyField)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20_VT159') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_VT159_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld161RRef,_Reference20_IDRRef') THROW 57404, N'dbo._Reference20_VT159 lacks the index _Reference20_VT159_1 (_Fld2683,_Fld161RRef,_Reference20_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference20_VT159') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference20_VT159_2' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld162RRef,_Reference20_IDRRef') THROW 57404, N'dbo._Reference20_VT159 lacks the index _Reference20_VT159_2 (_Fld2683,_Fld162RRef,_Reference20_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference20_VT159') AND type IN (1, 2)) <> 3 THROW 57404, N'dbo._Reference20_VT159 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Reference2598NG', N'U') IS NOT NULL THROW 57401, N'_Reference2598NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference2598')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_Description|nvarchar|300|0|0|0;_Fld4266|nvarchar|2000|0|0|0;_Fld2644RRef|binary|16|0|0|0;_Fld2645|numeric|0|15|0|0;_Fld2646|numeric|0|15|2|0;_Fld5283|nvarchar|80|0|0|0;_Fld5921|binary|1|0|0|0;_Fld6495|binary|1|0|0|0;_Fld11040|binary|1|0|0|0;_Fld11041|nvarchar|20|0|0|0;_Fld11042|nchar|40|0|0|0;_Fld11043|nvarchar|-1|0|0|0;_Fld11044|numeric|0|10|0|0;_Fld11045|numeric|0|12|3|0;_Fld11046|numeric|0|5|0|0;_Fld11047|datetime2|0|0|0|0;_Fld11048|datetime2|0|0|0|0' THROW 57404, N'the columns of dbo._Reference2598 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_PredefinedID') THROW 57404, N'dbo._Reference2598 lacks the index _Reference2598_1 (_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_2' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Description,_IDRRef') THROW 57404, N'dbo._Reference2598 lacks the index _Reference2598_2 (_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_3' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2645,_IDRRef') THROW 57404, N'dbo._Reference2598 lacks the index _Reference2598_3 (_Fld2645,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_4' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld5283,_IDRRef') THROW 57404, N'dbo._Reference2598 lacks the index _Reference2598_4 (_Fld5283,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_5' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld5921,_IDRRef') THROW 57404, N'dbo._Reference2598 lacks the index _Reference2598_5 (_Fld5921,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference2598_6' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld6495,_IDRRef') THROW 57404, N'dbo._Reference2598 lacks the index _Reference2598_6 (_Fld6495,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference2598') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_IDRRef') THROW 57404, N'dbo._Reference2598 lacks the index  (_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference2598') AND type IN (1, 2)) <> 7 THROW 57404, N'dbo._Reference2598 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Reference9367NG', N'U') IS NOT NULL THROW 57401, N'_Reference9367NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Reference9367')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_PredefinedID|binary|16|0|0|0;_Code|nvarchar|18|0|0|0;_Description|nvarchar|50|0|0|0;_Fld11049|nvarchar|30|0|0|0' THROW 57404, N'the columns of dbo._Reference9367 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference9367') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference9367_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_PredefinedID') THROW 57404, N'dbo._Reference9367 lacks the index _Reference9367_1 (_PredefinedID)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference9367') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference9367_2' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Code,_IDRRef') THROW 57404, N'dbo._Reference9367 lacks the index _Reference9367_2 (_Code,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference9367') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Reference9367_3' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Description,_IDRRef') THROW 57404, N'dbo._Reference9367 lacks the index _Reference9367_3 (_Description,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Reference9367') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_IDRRef') THROW 57404, N'dbo._Reference9367 lacks the index  (_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Reference9367') AND type IN (1, 2)) <> 4 THROW 57404, N'dbo._Reference9367 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Document39NG', N'U') IS NOT NULL THROW 57401, N'_Document39NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Document39')), N'') <> N'_IDRRef|binary|16|0|0|0;_Version|timestamp|0|0|0|0;_Marked|binary|1|0|0|0;_Date_Time|datetime2|0|0|0|0;_Number|nvarchar|22|0|0|0;_Posted|binary|1|0|0|0;_Fld3822|nvarchar|-1|0|0|0;_Fld5547|nvarchar|400|0|0|0;_Fld2575RRef|binary|16|0|0|0;_Fld3825|nvarchar|100|0|0|0;_Fld3827|datetime2|0|0|0|0;_Fld11050|numeric|0|3|0|0;_Fld2909RRef|binary|16|0|0|0;_Fld3828|nvarchar|200|0|0|0;_Fld1516|nvarchar|-1|0|0|0;_Fld6427RRef|binary|16|0|0|0;_Fld2908RRef|binary|16|0|0|0;_Fld968RRef|binary|16|0|0|0;_Fld4336RRef|binary|16|0|0|0;_Fld969RRef|binary|16|0|0|0;_Fld3824|nvarchar|100|0|0|0;_Fld3821RRef|binary|16|0|0|0;_Fld3823RRef|binary|16|0|0|0;_Fld2574|numeric|0|15|2|0;_Fld6309|binary|1|0|0|0;_Fld2147|binary|1|0|0|0;_Fld6310RRef|binary|16|0|0|0;_Fld3826|nvarchar|-1|0|0|0;_Fld5548|nvarchar|400|0|0|0;_Fld11051|binary|1|0|0|0;_Fld11052|nvarchar|80|0|0|0;_Fld11053|numeric|0|15|3|0;_Fld11054|datetime2|0|0|0|0;_Fld11055|datetime2|0|0|0|0;_Fld11056|nvarchar|-1|0|0|0;_Fld2683|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Document39 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Number,_IDRRef') THROW 57404, N'dbo._Document39 lacks the index _Document39_1 (_Fld2683,_Number,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_2' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Date_Time,_IDRRef,_Marked') THROW 57404, N'dbo._Document39 lacks the index _Document39_2 (_Fld2683,_Date_Time,_IDRRef,_Marked)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_3' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld2909RRef,_IDRRef') THROW 57404, N'dbo._Document39 lacks the index _Document39_3 (_Fld2683,_Fld2909RRef,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39') AND i.type = 2 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_4' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld3821RRef,_IDRRef') THROW 57404, N'dbo._Document39 lacks the index _Document39_4 (_Fld2683,_Fld3821RRef,_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_S_HPK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_IDRRef') THROW 57404, N'dbo._Document39 lacks the index _Document39_S_HPK (_Fld2683,_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Document39') AND type IN (1, 2)) <> 5 THROW 57404, N'dbo._Document39 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Document39_VT970NG', N'U') IS NOT NULL THROW 57401, N'_Document39_VT970NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Document39_VT970')), N'') <> N'_Document39_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo971|numeric|0|5|0|0;_Fld972RRef|binary|16|0|0|0' THROW 57404, N'the columns of dbo._Document39_VT970 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT970') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT970_SK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Document39_IDRRef,_KeyField') THROW 57404, N'dbo._Document39_VT970 lacks the index _Document39_VT970_SK (_Fld2683,_Document39_IDRRef,_KeyField)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT970') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT970_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld972RRef,_Document39_IDRRef') THROW 57404, N'dbo._Document39_VT970 lacks the index _Document39_VT970_1 (_Fld2683,_Fld972RRef,_Document39_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Document39_VT970') AND type IN (1, 2)) <> 2 THROW 57404, N'dbo._Document39_VT970 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Document39_VT2148NG', N'U') IS NOT NULL THROW 57401, N'_Document39_VT2148NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Document39_VT2148')), N'') <> N'_Document39_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo2149|numeric|0|5|0|0;_Fld2150RRef|binary|16|0|0|0;_Fld2151RRef|binary|16|0|0|0;_Fld2152|nvarchar|-1|0|0|0;_Fld3662|numeric|0|7|0|0' THROW 57404, N'the columns of dbo._Document39_VT2148 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT2148') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT2148_SK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Document39_IDRRef,_KeyField') THROW 57404, N'dbo._Document39_VT2148 lacks the index _Document39_VT2148_SK (_Fld2683,_Document39_IDRRef,_KeyField)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Document39_VT2148') AND type IN (1, 2)) <> 1 THROW 57404, N'dbo._Document39_VT2148 has indexes the model does not', 1;
IF OBJECT_ID(N'dbo._Document39_VT3663NG', N'U') IS NOT NULL THROW 57401, N'_Document39_VT3663NG is left behind', 1;
IF ISNULL((SELECT STRING_AGG(CONVERT(nvarchar(max), CONCAT(c.name, N'|', t.name, N'|', CASE WHEN t.name IN (N'binary', N'varbinary', N'nvarchar', N'nchar') THEN c.max_length ELSE 0 END, N'|', CASE WHEN t.name = N'numeric' THEN c.precision ELSE 0 END, N'|', CASE WHEN t.name IN (N'numeric', N'datetime2') THEN c.scale ELSE 0 END, N'|', c.is_nullable)), N';') WITHIN GROUP (ORDER BY c.column_id) FROM sys.columns c JOIN sys.types t ON t.user_type_id = c.user_type_id WHERE c.object_id = OBJECT_ID(N'dbo._Document39_VT3663')), N'') <> N'_Document39_IDRRef|binary|16|0|0|0;_Fld2683|numeric|0|7|0|0;_KeyField|binary|4|0|0|0;_LineNo3664|numeric|0|5|0|0;_Fld3665RRef|binary|16|0|0|0;_Fld3666RRef|binary|16|0|0|0;_Fld3667|nvarchar|1000|0|0|0;_Fld3668|nvarchar|-1|0|0|0;_Fld3669|nvarchar|200|0|0|0;_Fld3670|nvarchar|100|0|0|0;_Fld3671|nvarchar|100|0|0|0;_Fld3672|nvarchar|200|0|0|0;_Fld3673|nvarchar|200|0|0|0;_Fld3674|nvarchar|40|0|0|0;_Fld3675|nvarchar|40|0|0|0;_Fld3676|numeric|0|7|0|0;_Fld6749|nvarchar|-1|0|0|0' THROW 57404, N'the columns of dbo._Document39_VT3663 are not the model''s', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT3663') AND i.type = 1 AND i.is_unique = 1 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT3663_SK' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Document39_IDRRef,_KeyField') THROW 57404, N'dbo._Document39_VT3663 lacks the index _Document39_VT3663_SK (_Fld2683,_Document39_IDRRef,_KeyField)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT3663') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT3663_1' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld3665RRef,_Document39_IDRRef') THROW 57404, N'dbo._Document39_VT3663 lacks the index _Document39_VT3663_1 (_Fld2683,_Fld3665RRef,_Document39_IDRRef)', 1;
IF NOT EXISTS (SELECT 1 FROM sys.indexes i WHERE i.object_id = OBJECT_ID(N'dbo._Document39_VT3663') AND i.type = 2 AND i.is_unique = 0 AND (CASE WHEN i.is_primary_key = 1 THEN N'' ELSE i.name END) = N'_Document39_VT3663_2' AND (SELECT STRING_AGG(c.name, N',') WITHIN GROUP (ORDER BY ic.key_ordinal) FROM sys.index_columns ic JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id WHERE ic.object_id = i.object_id AND ic.index_id = i.index_id AND ic.key_ordinal > 0) = N'_Fld2683,_Fld3666RRef,_Document39_IDRRef') THROW 57404, N'dbo._Document39_VT3663 lacks the index _Document39_VT3663_2 (_Fld2683,_Fld3666RRef,_Document39_IDRRef)', 1;
IF (SELECT COUNT(*) FROM sys.indexes WHERE object_id = OBJECT_ID(N'dbo._Document39_VT3663') AND type IN (1, 2)) <> 3 THROW 57404, N'dbo._Document39_VT3663 has indexes the model does not', 1;
-- restructure: publication
DECLARE @ddl_schema varbinary(max) = 0xEFBBBF7B302C0D0A7B313736...<978617 bytes>;
UPDATE dbo.SchemaStorage SET Status = 100, CurrentSchema = @ddl_schema, NewGenCreated = 0xEFBBBF7B302C0D0A7B307D0D0A7D, NewGenDropped = 0xEFBBBF7B302C0D0A7B307D0D0A7D WHERE SchemaID = 0;
IF @@ROWCOUNT <> 1 THROW 57405, N'SchemaStorage was not rewritten', 1;
UPDATE dbo.DBSchema SET SerializedData = @ddl_schema;
IF @@ROWCOUNT <> 1 THROW 57405, N'DBSchema was not rewritten', 1;
UPDATE dbo.Params SET BinaryData = 0x9CFDCDF22D4B72DD89CD65A6...<145512 bytes>, DataSize = 145512, Modified = @now WHERE FileName = N'DBNames' AND PartNo = 0;
IF @@ROWCOUNT <> 1 THROW 57405, N'Params DBNames was not rewritten', 1;
UPDATE dbo.Params SET BinaryData = 0xEFBBBF7B302C356533343264...<43 bytes>, DataSize = 43, Modified = @now WHERE FileName = N'DBNamesVersion-DBNames' AND PartNo = 0;
IF @@ROWCOUNT <> 1 THROW 57405, N'Params DBNamesVersion-DBNames was not rewritten', 1;
IF (SELECT DATALENGTH(SerializedData) FROM dbo.DBSchema) <> DATALENGTH(@ddl_schema) THROW 57405, N'DBSchema and SchemaStorage.CurrentSchema differ', 1;
DELETE FROM dbo.Config WHERE FileName IN (SELECT LEFT(FileName, CHARINDEX(N'_dynupdate_', FileName) - 1) + SUBSTRING(FileName, CHARINDEX(N'_dynupdate_', FileName) + 47, 4000) FROM dbo.Config WHERE FileName LIKE N'%!_dynupdate!_06cb0442-0c47-4fad-986a-f08f28287c1b%' ESCAPE N'!' AND FileName NOT LIKE N'versions!_dynupdate!_%' ESCAPE N'!' AND FileName NOT LIKE N'deleted!_dynupdate!_%' ESCAPE N'!');
UPDATE dbo.Config SET FileName = LEFT(FileName, CHARINDEX(N'_dynupdate_', FileName) - 1) + SUBSTRING(FileName, CHARINDEX(N'_dynupdate_', FileName) + 47, 4000) WHERE FileName LIKE N'%!_dynupdate!_06cb0442-0c47-4fad-986a-f08f28287c1b%' ESCAPE N'!' AND FileName NOT LIKE N'versions!_dynupdate!_%' ESCAPE N'!' AND FileName NOT LIKE N'deleted!_dynupdate!_%' ESCAPE N'!';
DELETE FROM dbo.Config WHERE FileName = N'versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b';
IF EXISTS (SELECT 1 FROM dbo.Config WHERE FileName LIKE N'%!_dynupdate!_%' ESCAPE N'!' AND (FileName LIKE N'%!_dynupdate!_06cb0442-0c47-4fad-986a-f08f28287c1b%' ESCAPE N'!')) THROW 57312, N'a dynamic alias row is left after the fold', 1;
DELETE FROM dbo.Config WHERE FileName = N'DynamicallyUpdated';
DELETE FROM dbo.Params WHERE FileName = N'DynamicallyUpdated';
IF EXISTS (SELECT 1 FROM dbo.Config WHERE FileName = N'DynamicallyUpdated') OR EXISTS (SELECT 1 FROM dbo.Params WHERE FileName = N'DynamicallyUpdated') THROW 57311, N'DynamicallyUpdated is left after the cleanup', 1;
DELETE FROM dbo.Config WHERE FileName IN (SELECT FileName FROM dbo.ConfigSave);
INSERT dbo.Config (FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo) SELECT FileName, Creation, Modified, Attributes, DataSize, BinaryData, PartNo FROM dbo.ConfigSave;
IF @@ROWCOUNT <> 9 THROW 57308, N'the number of rows moved into Config differs from the staged count', 1;
UPDATE r SET _MessageNo = NULL FROM dbo._ConfigChngR r WHERE r._MessageNo IS NOT NULL AND (r._MDObjID IN (SELECT CAST(TRY_CAST(LEFT(s.FileName, 36) AS uniqueidentifier) AS binary(16)) FROM dbo.ConfigSave s WHERE s.PartNo = 0 AND TRY_CAST(LEFT(s.FileName, 36) AS uniqueidentifier) IS NOT NULL AND (LEN(s.FileName) = 36 OR SUBSTRING(s.FileName, 37, 1) = N'.'))          OR EXISTS (SELECT 1 FROM dbo._ConfigChngR_ExtProps e JOIN dbo.ConfigSave s ON s.FileName = e._FileName WHERE e._ConfigChngR_IDRRef = r._IDRRef));
IF EXISTS (SELECT 1 FROM dbo._ConfigChngR r WHERE r._MessageNo IS NOT NULL AND (r._MDObjID IN (SELECT CAST(TRY_CAST(LEFT(s.FileName, 36) AS uniqueidentifier) AS binary(16)) FROM dbo.ConfigSave s WHERE s.PartNo = 0 AND TRY_CAST(LEFT(s.FileName, 36) AS uniqueidentifier) IS NOT NULL AND (LEN(s.FileName) = 36 OR SUBSTRING(s.FileName, 37, 1) = N'.'))          OR EXISTS (SELECT 1 FROM dbo._ConfigChngR_ExtProps e JOIN dbo.ConfigSave s ON s.FileName = e._FileName WHERE e._ConfigChngR_IDRRef = r._IDRRef))) THROW 57313, N'a change registration of a staged object was not reset', 1;
IF (SELECT COUNT_BIG(*) FROM dbo.Files WHERE FileName = N'MobileVersions.dat' AND PartNo = 0 AND CONVERT(bigint, DataSize) = 37009 AND HASHBYTES('SHA2_256', BinaryData) = 0xAC60A3B8740844FED89AFCB3...<32 bytes>) <> 1 THROW 57306, N'Files.MobileVersions.dat changed since the plan was made', 1;
DELETE FROM dbo.Files WHERE FileName = N'MobileVersions.dat' AND PartNo <> 0;
UPDATE dbo.Files SET Creation = @now, Modified = @now, DataSize = 37009, BinaryData = 0xEFBBBF7B313030302C313531...<37009 bytes> WHERE FileName = N'MobileVersions.dat' AND PartNo = 0;
IF @@ROWCOUNT <> 1 THROW 57314, N'Files.MobileVersions.dat was not rewritten', 1;
IF (SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName = N'ea13a2c9-0c2f-40fa-b855-710387e3271d.si' AND PartNo = 0 AND CONVERT(bigint, DataSize) = 2996649 AND HASHBYTES('SHA2_256', BinaryData) = 0xB1697B6045650FBD33AE7858...<32 bytes>) <> 1 THROW 57306, N'Params.ea13a2c9-0c2f-40fa-b855-710387e3271d.si changed since the plan was made', 1;
DELETE FROM dbo.Params WHERE FileName = N'ea13a2c9-0c2f-40fa-b855-710387e3271d.si' AND PartNo <> 0;
UPDATE dbo.Params SET Creation = @now, Modified = @now, DataSize = 2977771, BinaryData = 0xECFDD9525B4BD72D8ADEAF88...<2977771 bytes> WHERE FileName = N'ea13a2c9-0c2f-40fa-b855-710387e3271d.si' AND PartNo = 0;
IF @@ROWCOUNT <> 1 THROW 57315, N'Params.ea13a2c9-0c2f-40fa-b855-710387e3271d.si was not rewritten', 1;
IF (SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName = N'1a621f0f-5568-4183-bd9f-f6ef670e7090.si' AND PartNo = 0 AND CONVERT(bigint, DataSize) = 447544 AND HASHBYTES('SHA2_256', BinaryData) = 0xF362A522B8A2EC563B88D2A6...<32 bytes>) <> 1 THROW 57306, N'Params.1a621f0f-5568-4183-bd9f-f6ef670e7090.si changed since the plan was made', 1;
DELETE FROM dbo.Params WHERE FileName = N'1a621f0f-5568-4183-bd9f-f6ef670e7090.si' AND PartNo <> 0;
UPDATE dbo.Params SET Creation = @now, Modified = @now, DataSize = 450423, BinaryData = 0xD4FD5BB325C7951E08BEB759...<450423 bytes> WHERE FileName = N'1a621f0f-5568-4183-bd9f-f6ef670e7090.si' AND PartNo = 0;
IF @@ROWCOUNT <> 1 THROW 57315, N'Params.1a621f0f-5568-4183-bd9f-f6ef670e7090.si was not rewritten', 1;
IF (SELECT COUNT_BIG(*) FROM dbo.Params WHERE FileName = N'siVersions' AND PartNo = 0 AND CONVERT(bigint, DataSize) = 1273 AND HASHBYTES('SHA2_256', BinaryData) = 0xA2DF3595E9F41169533F03BE...<32 bytes>) <> 1 THROW 57306, N'Params.siVersions changed since the plan was made', 1;
DELETE FROM dbo.Params WHERE FileName = N'siVersions' AND PartNo <> 0;
UPDATE dbo.Params SET Modified = @now, DataSize = 1273, BinaryData = 0xEFBBBF7B302C31362C226334...<1273 bytes> WHERE FileName = N'siVersions' AND PartNo = 0;
IF @@ROWCOUNT <> 1 THROW 57315, N'Params.siVersions was not rewritten', 1;
IF EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE NOT EXISTS (SELECT 1 FROM dbo.Config c WHERE c.FileName = s.FileName AND c.PartNo = s.PartNo AND c.DataSize = s.DataSize AND c.Creation = s.Creation AND c.Modified = s.Modified AND c.Attributes = s.Attributes AND c.BinaryData = s.BinaryData)) THROW 57309, N'a staged row is missing or differs in Config after the move', 1;
IF EXISTS (SELECT 1 FROM dbo.Config c WHERE EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE s.FileName = c.FileName) AND NOT EXISTS (SELECT 1 FROM dbo.ConfigSave s WHERE s.FileName = c.FileName AND s.PartNo = c.PartNo)) THROW 57309, N'Config keeps a part the staged row does not have', 1;
DELETE FROM dbo.ConfigSave;
IF @@ROWCOUNT <> 9 THROW 57310, N'ConfigSave cleanup removed an unexpected number of rows', 1;
IF EXISTS (SELECT 1 FROM dbo.ConfigSave) THROW 57310, N'ConfigSave is not empty after the cleanup', 1;
COMMIT TRANSACTION;
END TRY
BEGIN CATCH
IF XACT_STATE() <> 0 ROLLBACK TRANSACTION;
THROW;
END CATCH;
