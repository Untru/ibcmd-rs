-- DDL of the traced command, verbatim, in order (temporary-table DDL is only counted).

-- seq=9 t=+00:00.014 spid=94 tx=0 dur=2.5 ms rows=0
IF OBJECT_ID('FORMAT_NUMBER', 'FN') IS NULL
BEGIN
EXEC('CREATE FUNCTION dbo.FORMAT_NUMBER(@numstr VARCHAR(40), @grplen TINYINT, @secgrplen TINYINT, @grpsep VARCHAR(1), @decsep VARCHAR(1))
    RETURNS varchar(80)
    AS BEGIN
       DECLARE @numstrlen TINYINT;
       SET @numstrlen = LEN(@numstr);
       IF @grplen > 0 AND @grplen < @numstrlen
       BEGIN
           DECLARE @dotpos TINYINT;
           SET @dotpos = CHARINDEX(''.'', @numstr, 2);
           DECLARE @outstr VARCHAR(80);
           SET @outstr = (CASE WHEN @dotpos = 0 THEN '''' ELSE @decsep + right(@numstr, @numstrlen - @dotpos) END);

           IF(@dotpos = 0)
               SET @dotpos = @numstrlen + 1;

           DECLARE @startpos TINYINT;
           SET @startpos = (CASE WHEN left(@numstr, 1) = '' - '' THEN 2 ELSE 1 END);
           DECLARE @curpos SMALLINT;
           SET @curpos = @dotpos;
           WHILE @curpos > 0
           BEGIN
               SET @outstr = SUBSTRING(@numstr, @curpos - @grplen, @grplen) + (CASE WHEN(@curpos < @dotpos AND @curpos > @startpos) THEN @grpsep ELSE '''' END) + @outstr;
               SET @curpos = @curpos - @grplen;
               IF @secgrplen > 0 AND @secgrplen < @numstrlen
                   SET @grplen = @secgrplen;
           END

           RETURN @outstr;
       END

       IF @decsep != ''.''
           RETURN REPLACE(@numstr, ''.'', @decsep);
       RETURN @numstr;
    END')
END
GO

-- seq=22 t=+00:01.799 spid=94 tx=0 dur=2.1 ms rows=0
CREATE TABLE dbo.IBVersion (IBVersion int NOT NULL, PlatformVersionReq int NOT NULL)
GO

-- seq=25 t=+00:01.805 spid=94 tx=0 dur=2.0 ms rows=0
create table dbo.Config (FileName nvarchar(128) not null, Creation datetime2(0) not null, Modified datetime2(0) not null, Attributes smallint not null, DataSize bigint not null, BinaryData varbinary(max) not null, PartNo int not null, CONSTRAINT ByNameNo_Config PRIMARY KEY (FileName, PartNo))
GO

-- seq=26 t=+00:01.807 spid=94 tx=0 dur=1.6 ms rows=0
create table dbo.ConfigSave (FileName nvarchar(128) not null, Creation datetime2(0) not null, Modified datetime2(0) not null, Attributes smallint not null, DataSize bigint not null, BinaryData varbinary(max) not null, PartNo int not null, CONSTRAINT ByNameNo_ConfigSave PRIMARY KEY (FileName, PartNo))
GO

-- seq=27 t=+00:01.809 spid=94 tx=0 dur=1.1 ms rows=0
create table dbo.Params (FileName nvarchar(128) not null, Creation datetime2(0) not null, Modified datetime2(0) not null, Attributes smallint not null, DataSize bigint not null, BinaryData varbinary(max) not null, PartNo int not null, CONSTRAINT ByNameNo_Params PRIMARY KEY (FileName, PartNo))
GO

-- seq=28 t=+00:01.810 spid=94 tx=0 dur=1.5 ms rows=0
create table dbo.Files (FileName nvarchar(128) not null, Creation datetime2(0) not null, Modified datetime2(0) not null, Attributes smallint not null, DataSize bigint not null, BinaryData varbinary(max) not null, PartNo int not null, CONSTRAINT ByNameNo_Files PRIMARY KEY (FileName, PartNo))
GO

-- seq=29 t=+00:01.813 spid=94 tx=0 dur=2.3 ms rows=0
create table dbo.DepotFiles (FileName nvarchar(128) not null, Creation datetime2(0) not null, Modified datetime2(0) not null, Attributes smallint not null, DataSize bigint not null, BinaryData varbinary(max) not null, PartNo int not null, CONSTRAINT ByNameNo_DepotFiles PRIMARY KEY (FileName, PartNo))
GO

-- seq=30 t=+00:01.815 spid=94 tx=0 dur=1.1 ms rows=0
create table dbo.ConfigCAS (FileName nvarchar(128) not null, Creation datetime2(0) not null, Modified datetime2(0) not null, Attributes smallint not null, DataSize bigint not null, BinaryData varbinary(max) not null, PartNo int not null, CONSTRAINT ByNameNo_ConfigCAS PRIMARY KEY (FileName, PartNo))
GO

-- seq=31 t=+00:01.817 spid=94 tx=0 dur=1.1 ms rows=0
create table dbo.ConfigCASSave (FileName nvarchar(128) not null, Creation datetime2(0) not null, Modified datetime2(0) not null, Attributes smallint not null, DataSize bigint not null, BinaryData varbinary(max) not null, PartNo int not null, CONSTRAINT ByNameNo_ConfigCASSave PRIMARY KEY (FileName, PartNo))
GO

-- seq=33 t=+00:01.818 spid=94 tx=0 dur=1.1 ms rows=0
create table dbo._YearOffset (Offset int not null)
GO

-- seq=37 t=+00:01.822 spid=94 tx=0 dur=1.0 ms rows=0
create table dbo.DBSchema(SerializedData varbinary(max) not null)
GO

-- seq=87 t=+00:01.937 spid=94 tx=0 dur=15.4 ms rows=0
create table dbo.V8CMSDPWDS (
PwdHash nvarchar(256) not null primary key
)
;alter table dbo.V8CMSDPWDS SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=107 t=+00:01.967 spid=94 tx=0 dur=13.7 ms rows=0
create table dbo.SchemaStorage (
SchemaID int not null primary key,
Status int not null,
CurrentSchema varbinary(max) not null,
NewGenCreated varbinary(max) not null,
NewGenDropped varbinary(max) not null
)
;alter table dbo.SchemaStorage SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=118 t=+00:02.009 spid=94 tx=0 dur=2.3 ms rows=0
create table dbo._DbSegments (
_SegmentId binary(16) not null,
_SegmentName nvarchar(256) not null,
_Path nvarchar(256) not null
)
;alter table dbo._DbSegments SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=120 t=+00:02.011 spid=94 tx=0 dur=1.9 ms rows=0
create table dbo._DbSegmentsItems (
_ItemId nvarchar(256) not null,
_SegmentId binary(16) not null,
_ForIndex binary(1) not null,
_Applied binary(1) not null
)
;alter table dbo._DbSegmentsItems SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=121 t=+00:02.014 spid=94 tx=0 dur=1.3 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbSegments_1 ON dbo._DbSegments (_SegmentName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=123 t=+00:02.106 spid=94 tx=0 dur=3.8 ms rows=0
CREATE UNIQUE INDEX _DbSegments_2 ON dbo._DbSegments (_SegmentId) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=124 t=+00:02.111 spid=94 tx=0 dur=4.5 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbSegmentsItems_1 ON dbo._DbSegmentsItems (_ItemId, _ForIndex, _Applied) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=126 t=+00:02.118 spid=94 tx=0 dur=3.2 ms rows=0
CREATE INDEX _DbSegmentsItems_2 ON dbo._DbSegmentsItems (_SegmentId, _ForIndex) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=132 t=+00:02.129 spid=94 tx=0 dur=2.4 ms rows=0
create table dbo._WebSocketClients (
_ID binary(16) not null,
_WSCKey nvarchar(100) not null,
_MetadataID binary(16) not null,
_ServerURL nvarchar(255) not null,
_Predefined binary(1) not null,
_ConnectionParameters varbinary(max) not null,
_IBUserName nvarchar(100),
_AutoConnect binary(1) not null
)
;alter table dbo._WebSocketClients SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=138 t=+00:02.138 spid=94 tx=0 dur=6.3 ms rows=0
create table dbo._ExtensionsRestruct (
_ExtDataID binary(16) not null,
_RestructData varbinary(max) not null,
_RestructDataInt numeric(9, 0) not null,
_RestructDataType numeric(9, 0) not null
)
;alter table dbo._ExtensionsRestruct SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=140 t=+00:02.146 spid=94 tx=0 dur=1.3 ms rows=0
CREATE INDEX _ExtensionsRestruct_1 ON dbo._ExtensionsRestruct (_ExtDataID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=142 t=+00:02.147 spid=94 tx=0 dur=1.0 ms rows=0
CREATE INDEX _ExtensionsRestruct_2 ON dbo._ExtensionsRestruct (_ExtDataID, _RestructDataType) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=148 t=+00:02.154 spid=94 tx=0 dur=3.2 ms rows=0
create table dbo._ExtensionsRestructNGS (
_ExtDataID binary(16) not null,
_RestructData varbinary(max) not null,
_RestructDataInt numeric(9, 0) not null,
_RestructDataType numeric(9, 0) not null
)
;alter table dbo._ExtensionsRestructNGS SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=150 t=+00:02.158 spid=94 tx=0 dur=1.2 ms rows=0
CREATE INDEX _ExtensionsRestructNGS_1 ON dbo._ExtensionsRestructNGS (_ExtDataID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=152 t=+00:02.160 spid=94 tx=0 dur=1.2 ms rows=0
CREATE INDEX _ExtensionsRestructNGS_2 ON dbo._ExtensionsRestructNGS (_ExtDataID, _RestructDataType) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=158 t=+00:02.165 spid=94 tx=0 dur=1.9 ms rows=0
create table dbo._ExtensionsInfo (
_IDRRef binary(16) not null primary key,
_ExtensionOrder numeric(9, 0) not null,
_ExtName nvarchar(255) not null,
_UpdateTime datetime2(0) not null,
_ExtensionUsePurpose numeric(2, 0) not null,
_ExtensionScope numeric(2, 0) not null,
_ExtensionZippedInfo varbinary(max) not null,
_MasterNode nvarchar(max) not null,
_UsedInDistributedInfoBase binary(1) not null,
_Version timestamp not null
)
;alter table dbo._ExtensionsInfo SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=164 t=+00:02.171 spid=94 tx=0 dur=2.6 ms rows=0
create table dbo._ExtensionsInfoNGS (
_IDRRef binary(16) not null primary key,
_ExtensionOrder numeric(9, 0) not null,
_ExtName nvarchar(255) not null,
_UpdateTime datetime2(0) not null,
_ExtensionUsePurpose numeric(2, 0) not null,
_ExtensionScope numeric(2, 0) not null,
_ExtensionZippedInfo varbinary(max) not null,
_MasterNode nvarchar(max) not null,
_UsedInDistributedInfoBase binary(1) not null,
_Version timestamp not null
)
;alter table dbo._ExtensionsInfoNGS SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=170 t=+00:02.179 spid=94 tx=0 dur=2.0 ms rows=0
create table dbo._SystemSettings (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._SystemSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=172 t=+00:02.181 spid=94 tx=0 dur=1.8 ms rows=0
create table dbo._CommonSettings (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._CommonSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=174 t=+00:02.183 spid=94 tx=0 dur=2.5 ms rows=0
create table dbo._RepSettings (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._RepSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=176 t=+00:02.187 spid=94 tx=0 dur=2.0 ms rows=0
create table dbo._RepVarSettings (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._RepVarSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=178 t=+00:02.189 spid=94 tx=0 dur=1.6 ms rows=0
create table dbo._FrmDtSettings (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._FrmDtSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=180 t=+00:02.192 spid=94 tx=0 dur=1.7 ms rows=0
create table dbo._DynListSettings (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._DynListSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=182 t=+00:02.194 spid=94 tx=0 dur=1.8 ms rows=0
create table dbo._ErrorProcessingSettings (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._ErrorProcessingSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=184 t=+00:02.197 spid=94 tx=0 dur=1.4 ms rows=0
create table dbo._URLExternalData (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._URLExternalData SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=186 t=+00:02.198 spid=94 tx=0 dur=1.6 ms rows=0
create table dbo._InternalSettings (
_UserId nvarchar(max) not null,
_ObjectKey nvarchar(256) not null,
_SettingsKey nvarchar(max) not null,
_Version binary(16) not null,
_SettingsPresentation nvarchar(256),
_SettingsData varbinary(max),
_ChangeDate datetime2(0),
_UserIdHash numeric(10, 0) not null,
_SettingsKeyHash numeric(10, 0) not null
)
;alter table dbo._InternalSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=188 t=+00:02.201 spid=94 tx=0 dur=3.9 ms rows=0
create table dbo._DefaultSystemSettings (
_ObjectKey nvarchar(256) not null,
_Version binary(16) not null,
_SettingsData varbinary(max),
_ChangeDate datetime2(0)
)
;alter table dbo._DefaultSystemSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=190 t=+00:02.206 spid=94 tx=0 dur=2.0 ms rows=0
create table dbo._DefaultInternalSettings (
_ObjectKey nvarchar(256) not null,
_Version binary(16) not null,
_SettingsData varbinary(max),
_ChangeDate datetime2(0)
)
;alter table dbo._DefaultInternalSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=191 t=+00:02.209 spid=94 tx=0 dur=1.3 ms rows=0
CREATE CLUSTERED INDEX _SystemSettings_1 ON dbo._SystemSettings (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=192 t=+00:02.209 spid=94 tx=0 dur=1.4 ms rows=0
CREATE CLUSTERED INDEX _CommonSettings_1 ON dbo._CommonSettings (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=193 t=+00:02.211 spid=94 tx=0 dur=1.4 ms rows=0
CREATE CLUSTERED INDEX _RepSettings_1 ON dbo._RepSettings (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=194 t=+00:02.214 spid=94 tx=0 dur=1.3 ms rows=0
CREATE CLUSTERED INDEX _RepVarSettings_1 ON dbo._RepVarSettings (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=195 t=+00:02.214 spid=94 tx=0 dur=1.5 ms rows=0
CREATE CLUSTERED INDEX _FrmDtSettings_1 ON dbo._FrmDtSettings (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=196 t=+00:02.217 spid=94 tx=0 dur=2.4 ms rows=0
CREATE CLUSTERED INDEX _DynListSettings_1 ON dbo._DynListSettings (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=197 t=+00:02.219 spid=94 tx=0 dur=4.6 ms rows=0
CREATE CLUSTERED INDEX _ErrorProcessingSettings_1 ON dbo._ErrorProcessingSettings (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=198 t=+00:02.225 spid=94 tx=0 dur=2.0 ms rows=0
CREATE CLUSTERED INDEX _URLExternalData_1 ON dbo._URLExternalData (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=199 t=+00:02.227 spid=94 tx=0 dur=1.9 ms rows=0
CREATE CLUSTERED INDEX _InternalSettings_1 ON dbo._InternalSettings (_UserIdHash, _ObjectKey, _SettingsKeyHash, _Version) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=200 t=+00:02.230 spid=94 tx=0 dur=3.2 ms rows=0
CREATE CLUSTERED INDEX _DefaultSystemSettings_1 ON dbo._DefaultSystemSettings (_ObjectKey) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=201 t=+00:02.233 spid=94 tx=0 dur=1.5 ms rows=0
CREATE CLUSTERED INDEX _DefaultInternalSettings_1 ON dbo._DefaultInternalSettings (_ObjectKey) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=207 t=+00:02.241 spid=94 tx=0 dur=1.9 ms rows=0
create table dbo._DbCopiesInfoBaseUse (
_Id binary(16) not null,
_Description nvarchar(256) not null
)
;alter table dbo._DbCopiesInfoBaseUse SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=209 t=+00:02.244 spid=94 tx=0 dur=2.2 ms rows=0
create table dbo._DbCopiesUpdateTableStat (
_CopyId binary(16) not null,
_TableName nvarchar(256) not null,
_UpdateTime datetime2(0) not null,
_TransferTime numeric(10, 0) not null,
_IsPortion binary(1) not null
)
;alter table dbo._DbCopiesUpdateTableStat SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=211 t=+00:02.246 spid=94 tx=0 dur=2.2 ms rows=0
create table dbo._DbCopiesUpdateStat (
_CopyId binary(16) not null,
_UpdateTime datetime2(0) not null,
_TranPerSec numeric(16, 4) not null
)
;alter table dbo._DbCopiesUpdateStat SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=213 t=+00:02.250 spid=94 tx=0 dur=2.3 ms rows=0
create table dbo._DbCopies (
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
;alter table dbo._DbCopies SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=215 t=+00:02.252 spid=94 tx=0 dur=1.4 ms rows=0
create table dbo._DbCopiesSettings (
_CopyId binary(16) not null,
_CopyContent varbinary(max) not null,
_CopySchema varbinary(max) not null,
_Version numeric(9, 0) not null
)
;alter table dbo._DbCopiesSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=217 t=+00:02.254 spid=94 tx=0 dur=1.4 ms rows=0
create table dbo._DbCopiesTrLogs (
_TrNum int identity(1,1) not null,
_TrTime datetime2(0) not null,
_TrId binary(16) not null,
_TrLog varbinary(max)
)
;alter table dbo._DbCopiesTrLogs SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=219 t=+00:02.256 spid=94 tx=0 dur=1.6 ms rows=0
create table dbo._DbCopiesTrTables (
_TrNum int,
_TrTime datetime2(0),
_TableName nvarchar(256) not null
)
;alter table dbo._DbCopiesTrTables SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=221 t=+00:02.259 spid=94 tx=0 dur=1.2 ms rows=0
create table dbo._DbCopiesUpdates (
_CopyId binary(16) not null,
_TrNum int,
_TrTime datetime2(0),
_UpdateId binary(16),
_LastUpdateResult numeric(2, 0),
_LastUpdateError varbinary(max)
)
;alter table dbo._DbCopiesUpdates SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=223 t=+00:02.260 spid=94 tx=0 dur=1.1 ms rows=0
create table dbo._DbCopiesTablesStates (
_CopyId binary(16) not null,
_TableName nvarchar(256) not null,
_TableState int not null,
_TrNum int
)
;alter table dbo._DbCopiesTablesStates SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=225 t=+00:02.262 spid=94 tx=0 dur=2.3 ms rows=0
create table dbo._DbCopiesInitialLast (
_CopyId binary(16) not null,
_TableName nvarchar(256) not null,
_BlockNum int identity(1,1) not null,
_FirstKey varbinary(max),
_LastKey varbinary(max),
_BlockState int not null
)
;alter table dbo._DbCopiesInitialLast SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=227 t=+00:02.265 spid=94 tx=0 dur=1.3 ms rows=0
create table dbo._DbCopiesTrChanges (
_CopyId binary(16) not null,
_TableName nvarchar(256) not null,
_TrNum int not null,
_ChId binary(16) not null
)
;alter table dbo._DbCopiesTrChanges SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=229 t=+00:02.267 spid=94 tx=0 dur=1.2 ms rows=0
create table dbo._DbCopiesTrChObj (
_ChId binary(16) not null,
_ChObj varbinary(max)
)
;alter table dbo._DbCopiesTrChObj SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=230 t=+00:02.269 spid=94 tx=0 dur=2.1 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbCopies_1 ON dbo._DbCopies (_CopyId, _CopyName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=231 t=+00:02.271 spid=94 tx=0 dur=1.8 ms rows=0
CREATE CLUSTERED INDEX _DbCopiesSettings_1 ON dbo._DbCopiesSettings (_CopyId) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=232 t=+00:02.273 spid=94 tx=0 dur=1.3 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbCopiesTrLogs_1 ON dbo._DbCopiesTrLogs (_TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=234 t=+00:02.275 spid=94 tx=0 dur=0.8 ms rows=0
CREATE UNIQUE INDEX _DbCopiesTrLogs_2 ON dbo._DbCopiesTrLogs (_TrId) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=235 t=+00:02.276 spid=94 tx=0 dur=2.5 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbCopiesTrTables_1 ON dbo._DbCopiesTrTables (_TableName, _TrNum, _TrTime) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=237 t=+00:02.280 spid=94 tx=0 dur=0.9 ms rows=0
CREATE INDEX _DbCopiesTrTables_2 ON dbo._DbCopiesTrTables (_TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=238 t=+00:02.281 spid=94 tx=0 dur=2.2 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbCopiesUpdates_1 ON dbo._DbCopiesUpdates (_CopyId, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=239 t=+00:02.283 spid=94 tx=0 dur=1.5 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbCopiesTablesStates_1 ON dbo._DbCopiesTablesStates (_CopyId, _TableName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=240 t=+00:02.285 spid=94 tx=0 dur=1.3 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbCopiesInitialLast_1 ON dbo._DbCopiesInitialLast (_CopyId, _TableName, _BlockNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=241 t=+00:02.286 spid=94 tx=0 dur=1.6 ms rows=0
CREATE CLUSTERED INDEX _DbCopiesTrChanges_1 ON dbo._DbCopiesTrChanges (_CopyId, _TableName, _TrNum) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=242 t=+00:02.288 spid=94 tx=0 dur=2.7 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DbCopiesTrChObj_1 ON dbo._DbCopiesTrChObj (_ChId) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=248 t=+00:02.297 spid=94 tx=0 dur=3.3 ms rows=0
create table dbo._MobileClientDataExchange (
_ID binary(16) not null,
_Version numeric(2, 0) not null,
_Type numeric(2, 0) not null,
_Data varbinary(max),
_Date datetime2(0) not null
)
;alter table dbo._MobileClientDataExchange SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=250 t=+00:02.301 spid=94 tx=0 dur=1.0 ms rows=0
CREATE INDEX _MobileClientDataExchange_1 ON dbo._MobileClientDataExchange (_ID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=256 t=+00:02.306 spid=94 tx=0 dur=3.1 ms rows=0
create table dbo._Bots (
_ID binary(16) not null,
_ClientID nvarchar(100) not null,
_ECSUserID nvarchar(100) not null,
_MDBotID binary(16) not null,
_IBUserName nvarchar(100),
_Param varbinary(max) not null,
_Predefined binary(1) not null,
_NeedsUpdate binary(1)
)
;alter table dbo._Bots SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=262 t=+00:02.314 spid=94 tx=0 dur=1.9 ms rows=0
create table dbo._DataHistoryQueue0 (
_MetadataId binary(16) not null,
_DataId binary(20) not null,
_Position numeric(9, 0) not null,
_Content varbinary(max) not null
)
;alter table dbo._DataHistoryQueue0 SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=264 t=+00:02.316 spid=94 tx=0 dur=1.7 ms rows=0
create table dbo._DataHistoryVersions (
_HistoryDataId binary(16) not null,
_VersionNumber numeric(9, 0) not null,
_MetadataVersionNumber numeric(9, 0) not null,
_Date datetime2(0) not null,
_ChangeType numeric(1, 0) not null,
_UserId binary(16) not null,
_UserName nvarchar(256) not null,
_UserFullName nvarchar(256) not null,
_Comment nvarchar(1024) not null,
_Transaction binary(16) not null,
_Node_TYPE binary(1) not null,
_Node_RTRef binary(4) not null,
_Node_RRRef binary(16) not null,
_Content varbinary(max) not null
)
;alter table dbo._DataHistoryVersions SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=266 t=+00:02.319 spid=94 tx=0 dur=1.9 ms rows=0
create table dbo._DataHistoryLatestVersions (
_MetadataId binary(16) not null,
_DataId binary(20) not null,
_HistoryDataId binary(16) not null,
_VersionNumber numeric(9, 0) not null,
_Content varbinary(max) not null
)
;alter table dbo._DataHistoryLatestVersions SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=268 t=+00:02.322 spid=94 tx=0 dur=2.3 ms rows=0
create table dbo._DataHistoryMetadata (
_MetadataId binary(16) not null,
_IsSettings binary(1) not null,
_IsActual binary(1) not null,
_MetadataVersionNumber numeric(9, 0) not null,
_Content varbinary(max) not null,
_IsExtensions binary(1) not null,
_ActionOnAccept numeric(1, 0) not null
)
;alter table dbo._DataHistoryMetadata SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=270 t=+00:02.325 spid=94 tx=0 dur=2.5 ms rows=0
create table dbo._DataHistorySettings (
_MetadataId binary(16) not null,
_Content varbinary(max) not null
)
;alter table dbo._DataHistorySettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=272 t=+00:02.329 spid=94 tx=0 dur=2.1 ms rows=0
create table dbo._DataHistoryAfterWriteQueue (
_MetadataId binary(16) not null,
_HistoryDataId binary(16) not null,
_VersionNumber numeric(9, 0) not null
)
;alter table dbo._DataHistoryAfterWriteQueue SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=273 t=+00:02.332 spid=94 tx=0 dur=2.0 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DataHistoryQueue0_1 ON dbo._DataHistoryQueue0 (_MetadataId, _DataId, _Position) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=274 t=+00:02.334 spid=94 tx=0 dur=1.9 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DataHistoryVersions_1 ON dbo._DataHistoryVersions (_HistoryDataId, _VersionNumber) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=275 t=+00:02.336 spid=94 tx=0 dur=2.9 ms rows=0
CREATE CLUSTERED INDEX _DataHistoryLatestVersions_1 ON dbo._DataHistoryLatestVersions (_MetadataId, _DataId) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=276 t=+00:02.339 spid=94 tx=0 dur=1.4 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DataHistoryMetadata_1 ON dbo._DataHistoryMetadata (_MetadataId, _IsSettings, _IsActual, _MetadataVersionNumber, _IsExtensions) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=278 t=+00:02.341 spid=94 tx=0 dur=0.6 ms rows=0
CREATE UNIQUE INDEX _DataHistoryMetadata_2 ON dbo._DataHistoryMetadata (_MetadataId, _MetadataVersionNumber) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=279 t=+00:02.343 spid=94 tx=0 dur=1.2 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DataHistorySettings_1 ON dbo._DataHistorySettings (_MetadataId) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=280 t=+00:02.344 spid=94 tx=0 dur=1.0 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _DataHistoryAfterWriteQueue_1 ON dbo._DataHistoryAfterWriteQueue (_MetadataId, _HistoryDataId, _VersionNumber) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=286 t=+00:02.349 spid=94 tx=0 dur=1.9 ms rows=0
create table dbo._RefOpt (
_MDID binary(16) not null,
_ExtID binary(16) not null,
_PDUpdMode numeric(1, 0) not null
)
;alter table dbo._RefOpt SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=288 t=+00:02.353 spid=94 tx=0 dur=1.4 ms rows=0
CREATE INDEX _RefOpt_1 ON dbo._RefOpt (_MDID, _ExtID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=294 t=+00:02.360 spid=94 tx=0 dur=2.0 ms rows=0
create table dbo._ChrcOpt (
_MDID binary(16) not null,
_ExtID binary(16) not null,
_PDUpdMode numeric(1, 0) not null
)
;alter table dbo._ChrcOpt SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=296 t=+00:02.363 spid=94 tx=0 dur=1.2 ms rows=0
CREATE INDEX _ChrcOpt_1 ON dbo._ChrcOpt (_MDID, _ExtID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=302 t=+00:02.367 spid=94 tx=0 dur=1.7 ms rows=0
create table dbo._AccOpt (
_MDID binary(16) not null,
_ExtID binary(16) not null,
_PDUpdMode numeric(1, 0) not null
)
;alter table dbo._AccOpt SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=304 t=+00:02.370 spid=94 tx=0 dur=1.2 ms rows=0
CREATE INDEX _AccOpt_1 ON dbo._AccOpt (_MDID, _ExtID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=310 t=+00:02.375 spid=94 tx=0 dur=1.7 ms rows=0
create table dbo._CKindsOpt (
_MDID binary(16) not null,
_ExtID binary(16) not null,
_PDUpdMode numeric(1, 0) not null
)
;alter table dbo._CKindsOpt SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=312 t=+00:02.378 spid=94 tx=0 dur=0.8 ms rows=0
CREATE INDEX _CKindsOpt_1 ON dbo._CKindsOpt (_MDID, _ExtID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=318 t=+00:02.382 spid=94 tx=0 dur=1.7 ms rows=0
create table dbo._UsersWorkHistory (
_ID binary(16) not null,
_UserID binary(16) not null,
_URL nvarchar(max) not null,
_Date datetime2(0) not null,
_URLHash numeric(10, 0) not null,
_ECSActivity binary(1)
)
;alter table dbo._UsersWorkHistory SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=320 t=+00:02.384 spid=94 tx=0 dur=0.7 ms rows=0
CREATE UNIQUE INDEX _UsersWorkHistory_1 ON dbo._UsersWorkHistory (_ID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=322 t=+00:02.386 spid=94 tx=0 dur=0.8 ms rows=0
CREATE INDEX _UsersWorkHistory_2 ON dbo._UsersWorkHistory (_UserID, _Date) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=323 t=+00:02.388 spid=94 tx=0 dur=2.4 ms rows=0
CREATE CLUSTERED INDEX _UsersWorkHistory_3 ON dbo._UsersWorkHistory (_UserID, _URLHash, _Date) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=329 t=+00:02.394 spid=94 tx=0 dur=1.6 ms rows=0
create table dbo._ODataSettings (
_MetadataObjectUUID binary(16) not null
)
;alter table dbo._ODataSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=335 t=+00:02.400 spid=94 tx=0 dur=1.8 ms rows=0
create table dbo._STTSettings (
_Token nvarchar(100) not null,
_Host nvarchar(100)
)
;alter table dbo._STTSettings SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=337 t=+00:02.402 spid=94 tx=0 dur=1.9 ms rows=0
create table dbo._STTGrammar (
_Grammar nvarchar(100) not null,
_Phrase nvarchar(100)
)
;alter table dbo._STTGrammar SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=339 t=+00:02.405 spid=94 tx=0 dur=1.5 ms rows=0
create table dbo._STTGrammarChecksum (
_Grammar nvarchar(100) not null,
_Checksum numeric(10, 0) not null
)
;alter table dbo._STTGrammarChecksum SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=341 t=+00:02.407 spid=94 tx=0 dur=1.3 ms rows=0
create table dbo._STTModels (
_IDRRef binary(16) not null primary key,
_ModelID binary(16) not null,
_ModelCompatibility numeric(5, 0) not null,
_Acoustic nvarchar(100) not null,
_AcousticRU nvarchar(100) not null,
_LanguageModel nvarchar(100) not null,
_LanguageModelRU nvarchar(100) not null,
_Version nvarchar(100) not null,
_Language nvarchar(2) not null,
_SampleRate numeric(5, 0) not null
)
;alter table dbo._STTModels SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=343 t=+00:02.408 spid=94 tx=0 dur=1.5 ms rows=0
create table dbo._STTModelsDesc (
_IDRRef binary(16) not null primary key,
_ModelRRef binary(16) not null
)
;alter table dbo._STTModelsDesc SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=345 t=+00:02.411 spid=94 tx=0 dur=1.1 ms rows=0
create table dbo._STTModelsDesc_Descr (
_STTModelsDesc_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_Language nchar(2) not null,
_Description nvarchar(max) not null
)
;alter table dbo._STTModelsDesc_Descr SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=347 t=+00:02.413 spid=94 tx=0 dur=1.2 ms rows=0
create table dbo._STTModelsDesc_Acoustic (
_STTModelsDesc_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_Language nchar(2) not null,
_Description nvarchar(max) not null
)
;alter table dbo._STTModelsDesc_Acoustic SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=349 t=+00:02.414 spid=94 tx=0 dur=1.1 ms rows=0
create table dbo._STTModelsDesc_LangModel (
_STTModelsDesc_IDRRef binary(16) not null,
_KeyField binary(4) not null,
_Language nchar(2) not null,
_Description nvarchar(max) not null
)
;alter table dbo._STTModelsDesc_LangModel SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=351 t=+00:02.416 spid=94 tx=0 dur=0.8 ms rows=0
CREATE INDEX _STTGrammar_1 ON dbo._STTGrammar (_Grammar) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=353 t=+00:02.417 spid=94 tx=0 dur=0.6 ms rows=0
CREATE UNIQUE INDEX _STTGrammarChecksum_1 ON dbo._STTGrammarChecksum (_Grammar) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=355 t=+00:02.418 spid=94 tx=0 dur=0.6 ms rows=0
CREATE UNIQUE INDEX _STTModels_1 ON dbo._STTModels (_ModelID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=357 t=+00:02.419 spid=94 tx=0 dur=0.8 ms rows=0
CREATE UNIQUE INDEX _STTModelsDesc_1 ON dbo._STTModelsDesc (_ModelRRef) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=358 t=+00:02.421 spid=94 tx=0 dur=1.0 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _STTModelsDesc_Descr_SK ON dbo._STTModelsDesc_Descr (_STTModelsDesc_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=359 t=+00:02.422 spid=94 tx=0 dur=0.9 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _STTModelsDesc_Acoustic_SK ON dbo._STTModelsDesc_Acoustic (_STTModelsDesc_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=360 t=+00:02.423 spid=94 tx=0 dur=1.1 ms rows=0
CREATE UNIQUE CLUSTERED INDEX _STTModelsDesc_LangModel_SK ON dbo._STTModelsDesc_LangModel (_STTModelsDesc_IDRRef, _KeyField) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON)
GO

-- seq=366 t=+00:02.428 spid=94 tx=0 dur=2.0 ms rows=0
create table dbo.v8users (
ID binary(16) not null primary key,
Name nvarchar(64) not null,
Descr nvarchar(128) not null,
OSName nvarchar(128),
Changed datetime2(0) not null,
RolesID numeric(10, 0) not null,
Show binary(1) not null,
[Data] varbinary(max) not null,
EAuth binary(1),
AdmRole binary(1),
UsSprH numeric(10, 0),
Email nvarchar(128)
)
;alter table dbo.v8users SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=368 t=+00:02.431 spid=94 tx=0 dur=1.0 ms rows=0
CREATE UNIQUE INDEX ByName ON dbo.v8users (Name) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=370 t=+00:02.432 spid=94 tx=0 dur=1.1 ms rows=0
CREATE INDEX ByDescr ON dbo.v8users (Descr) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=372 t=+00:02.434 spid=94 tx=0 dur=0.7 ms rows=0
CREATE INDEX ByOSName ON dbo.v8users (OSName) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=374 t=+00:02.435 spid=94 tx=0 dur=0.6 ms rows=0
CREATE INDEX ByRolesID ON dbo.v8users (RolesID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=376 t=+00:02.436 spid=94 tx=0 dur=0.6 ms rows=0
CREATE INDEX ByShow ON dbo.v8users (Show) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=378 t=+00:02.437 spid=94 tx=0 dur=0.7 ms rows=0
CREATE INDEX ByEAuth ON dbo.v8users (AdmRole, EAuth) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=380 t=+00:02.438 spid=94 tx=0 dur=0.7 ms rows=0
CREATE INDEX ByEmail_V8USERS ON dbo.v8users (Email) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=382 t=+00:02.440 spid=94 tx=0 dur=1.7 ms rows=0
create table dbo.V8USERSMATKEYS (
ID binary(16) not null,
PROVIDERSH nvarchar(128) not null,
MATKEYSH nvarchar(128) not null,
[DATA] varbinary(max) not null
)
;alter table dbo.V8USERSMATKEYS SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=384 t=+00:02.442 spid=94 tx=0 dur=0.6 ms rows=0
CREATE INDEX ByMatKey ON dbo.V8USERSMATKEYS (PROVIDERSH, MATKEYSH) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=386 t=+00:02.443 spid=94 tx=0 dur=0.7 ms rows=0
CREATE INDEX ByID ON dbo.V8USERSMATKEYS (ID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=388 t=+00:02.445 spid=94 tx=0 dur=1.5 ms rows=0
create table dbo.V8USERPWDPLCS (
Name nvarchar(64) not null,
[Data] varbinary(max) not null
)
;alter table dbo.V8USERPWDPLCS SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=390 t=+00:02.448 spid=94 tx=0 dur=0.8 ms rows=0
CREATE UNIQUE INDEX ByName_V8USERPWDPLCS ON dbo.V8USERPWDPLCS (Name) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=392 t=+00:02.449 spid=94 tx=0 dur=1.6 ms rows=0
create table dbo.BinaryData (
f_key binary(16) not null,
f_off numeric(18, 0) not null,
f_num numeric(18, 0) not null,
f_data varbinary(max) not null
)
;alter table dbo.BinaryData SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=395 t=+00:02.838 spid=94 tx=0 dur=2.4 ms rows=0
CREATE UNIQUE INDEX BinaryDataInd ON dbo.BinaryData (f_key, f_off) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=397 t=+00:02.841 spid=94 tx=0 dur=1.3 ms rows=0
CREATE INDEX BinaryDataInd2 ON dbo.BinaryData (f_num) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=399 t=+00:02.842 spid=94 tx=0 dur=2.7 ms rows=0
create table dbo.ExternalBinDataStrgsList (
StorageID binary(16) not null,
Name nchar(80) not null,
ConnectionSettings_URL nvarchar(max) not null,
ConnectionSettings_URLType numeric(1, 0) not null,
AccessId nvarchar(max) not null,
SecretKey nvarchar(max) not null,
Region nvarchar(max) not null,
MinWriteDataSize numeric(18, 0) not null,
EnableWrite binary(1) not null,
IsDeleted binary(1) not null
)
;alter table dbo.ExternalBinDataStrgsList SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=401 t=+00:02.846 spid=94 tx=0 dur=0.9 ms rows=0
CREATE UNIQUE INDEX ExternalBinDataStrgsListInd ON dbo.ExternalBinDataStrgsList (StorageID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=403 t=+00:02.848 spid=94 tx=0 dur=1.8 ms rows=0
create table dbo.ExternalBinDataStrgsBList (
StorageID binary(16) not null,
BlobID binary(16) not null,
[Timestamp] datetime2(0) not null,
BlobSize numeric(10, 0) not null,
IsDeleted binary(1) not null
)
;alter table dbo.ExternalBinDataStrgsBList SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=405 t=+00:02.850 spid=94 tx=0 dur=0.7 ms rows=0
CREATE UNIQUE INDEX ExternalBinDataStrgsBListInd ON dbo.ExternalBinDataStrgsBList (StorageID, BlobID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=407 t=+00:02.852 spid=94 tx=0 dur=4.3 ms rows=0
create table dbo.BinaryDataStorageContent (
f_key binary(16) not null,
f_type nchar(16) not null,
f_parent binary(16) not null,
f_id1 binary(16) not null,
f_id2 binary(16) not null,
f_id3 binary(16) not null,
f_id4 binary(16) not null,
f_id5 binary(16) not null,
f_id6 binary(16) not null,
f_str1 nchar(80) not null,
f_num1 numeric(18, 0) not null,
f_num2 numeric(18, 0) not null,
f_num3 numeric(18, 0) not null,
f_num4 numeric(18, 0) not null,
f_num5 numeric(18, 0) not null,
f_vstr1 nvarchar(max) not null,
f_vstr2 nvarchar(max) not null,
f_vstr3 nvarchar(max) not null,
f_vstr4 nvarchar(max) not null
)
;alter table dbo.BinaryDataStorageContent SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=409 t=+00:02.857 spid=94 tx=0 dur=1.0 ms rows=0
CREATE UNIQUE INDEX BinaryDataStorageContentInd ON dbo.BinaryDataStorageContent (f_key) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- seq=411 t=+00:02.858 spid=94 tx=0 dur=1.6 ms rows=0
create table dbo.BinaryDataStorageVersion (
StorageID binary(16) not null,
Version datetime2(0) not null
)
;alter table dbo.BinaryDataStorageVersion SET (LOCK_ESCALATION = DISABLE);
GO

-- seq=413 t=+00:02.861 spid=94 tx=0 dur=0.8 ms rows=0
CREATE UNIQUE INDEX BinaryDataStorageVersionInd ON dbo.BinaryDataStorageVersion (StorageID) WITH (SORT_IN_TEMPDB = ON, MAXDOP = 0, ALLOW_PAGE_LOCKS = OFF, ALLOW_ROW_LOCKS = ON) ON [PRIMARY]
GO

-- 141 DDL statements written, 0 temporary-table DDL statements not listed
