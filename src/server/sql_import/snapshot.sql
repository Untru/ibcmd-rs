-- Read-only acquisition, one sp_executesql request. No SQL write authority.
USE __DATABASE__;
SET NOCOUNT ON;
SET XACT_ABORT ON;
SET LOCK_TIMEOUT 5000;
IF @@TRANCOUNT <> 0 THROW 57620, 'snapshot requires an idle session', 1;
-- Initial operation admission is restricted to the CURRENT effective sysadmin
-- context. No grant/elevation occurs; restricted principals remain unsupported.
DECLARE @MetadataSysadmin int=IS_SRVROLEMEMBER(N'sysadmin');
IF @MetadataSysadmin IS NULL OR @MetadataSysadmin<>1
 THROW 57635, 'snapshot metadata context is unsupported', 1;
DECLARE @ExecutionSid varbinary(max)=SUSER_SID();
IF @ExecutionSid IS NULL OR DATALENGTH(@ExecutionSid)=0 OR DATALENGTH(@ExecutionSid)>85
 THROW 57636, 'snapshot execution identity is unavailable', 1;
DECLARE @ExecutionSidHash varbinary(32)=HASHBYTES('SHA2_256',@ExecutionSid);
BEGIN TRY
 BEGIN TRANSACTION;
 IF ISNULL(HAS_PERMS_BY_NAME(DB_NAME(),'DATABASE','VIEW DEFINITION'),0)<>1
   THROW 57626, 'snapshot requires visible storage definitions', 1;
 IF OBJECT_ID(N'dbo.Config',N'U') IS NULL OR OBJECT_ID(N'dbo.ConfigSave',N'U') IS NULL
   OR OBJECT_ID(N'dbo.Params',N'U') IS NULL
   THROW 57627, 'snapshot storage tables are unsupported', 1;
 DECLARE @NC bigint, @NS bigint, @BC bigint, @BS bigint, @MC bigint, @MS bigint, @NP bigint;
 -- Deterministic table lock order; TOP caps counting work for oversized targets.
 SELECT @NC=COUNT_BIG(*) FROM
   (SELECT TOP (@P2+1) FileName FROM dbo.Config WITH (TABLOCK,HOLDLOCK)) c;
 SELECT @NS=COUNT_BIG(*) FROM
   (SELECT TOP (@P2+1) FileName FROM dbo.ConfigSave WITH (TABLOCK,HOLDLOCK)) s;
 IF @NC+@NS>@P2 THROW 57621, 'snapshot part count exceeded', 1;
 -- Params is not a payload source. Its retained lock protects the absence of
 -- the paired dynamic-generation marker through the same publication boundary.
 SELECT @NP=COUNT_BIG(*) FROM
   (SELECT TOP (@P2+1) FileName FROM dbo.Params WITH (TABLOCK,HOLDLOCK)) p;
 IF @NP>@P2 THROW 57637, 'snapshot Params marker inventory is oversized', 1;
 -- Closed staging catalog cohort. Resolve object identities only AFTER all
 -- three table locks; retain the objects against DDL through the completion seal.
 DECLARE @ConfigId int=OBJECT_ID(N'dbo.Config',N'U'),
   @SaveId int=OBJECT_ID(N'dbo.ConfigSave',N'U'),
   @ParamsId int=OBJECT_ID(N'dbo.Params',N'U');
 DECLARE @UnsafeStorageFeatures bigint=
   (SELECT COUNT_BIG(*) FROM sys.foreign_keys
     WHERE parent_object_id IN (@ConfigId,@SaveId,@ParamsId)
        OR referenced_object_id IN (@ConfigId,@SaveId,@ParamsId))+
   (SELECT COUNT_BIG(*) FROM sys.security_predicates
     WHERE target_object_id IN (@ConfigId,@SaveId,@ParamsId))+
   (SELECT COUNT_BIG(*) FROM sys.triggers
     WHERE parent_id IN (@ConfigId,@SaveId,@ParamsId))+
   (SELECT COUNT_BIG(*) FROM sys.check_constraints
     WHERE parent_object_id IN (@ConfigId,@SaveId,@ParamsId))+
   (SELECT COUNT_BIG(*) FROM sys.change_tracking_tables
     WHERE object_id IN (@ConfigId,@SaveId,@ParamsId))+
   (SELECT COUNT_BIG(*) FROM sys.tables WHERE object_id IN (@ConfigId,@SaveId,@ParamsId)
     AND (is_memory_optimized=1 OR temporal_type<>0 OR is_filetable=1
       OR is_node=1 OR is_edge=1 OR ledger_type<>0 OR is_external=1
       OR is_remote_data_archive_enabled=1 OR is_tracked_by_cdc=1
       OR is_replicated=1 OR has_replication_filter=1 OR is_merge_published=1
       OR is_sync_tran_subscribed=1))+
   (SELECT COUNT_BIG(*) FROM sys.columns c WHERE c.object_id IN (@ConfigId,@SaveId,@ParamsId)
     AND (c.is_hidden=1 OR c.is_sparse=1 OR c.is_column_set=1 OR c.is_filestream=1
       OR c.rule_object_id<>0 OR (c.default_object_id<>0 AND NOT EXISTS
         (SELECT 1 FROM sys.default_constraints dc
           WHERE dc.object_id=c.default_object_id AND dc.parent_object_id=c.object_id))))+
   (SELECT COUNT_BIG(*) FROM sys.indexes WHERE object_id IN (@ConfigId,@SaveId,@ParamsId)
     AND (ignore_dup_key=1 OR is_hypothetical=1))+
   (SELECT COUNT_BIG(*) FROM sys.sql_expression_dependencies d
     JOIN sys.views v ON v.object_id=d.referencing_id
     WHERE d.referenced_id IN (@ConfigId,@SaveId,@ParamsId) AND d.is_schema_bound_reference=1
       AND EXISTS(SELECT 1 FROM sys.indexes i
         WHERE i.object_id=v.object_id AND i.index_id>0));
 IF @UnsafeStorageFeatures<>0
   THROW 57629, 'snapshot storage has unsupported DML features', 1;
 IF NOT EXISTS(SELECT 1 FROM sys.columns c WHERE c.object_id=@ParamsId
   AND c.column_id=1 AND CONVERT(varbinary(max),c.name)=CONVERT(varbinary(max),N'FileName')
   AND c.system_type_id=231 AND c.user_type_id=231 AND c.max_length=256
   AND c.is_nullable=0 AND c.is_identity=0 AND c.is_computed=0
   AND c.generated_always_type=0 AND c.encryption_type IS NULL)
   THROW 57638, 'snapshot Params marker key layout is unsupported', 1;
 IF EXISTS(SELECT 1 FROM dbo.Params
   WHERE FileName COLLATE Latin1_General_100_CI_AS=N'DynamicallyUpdated')
   THROW 57639, 'snapshot dynamic generation is unsupported', 1;
 DECLARE @ParamsMarkerAbsent bit=1;
 DECLARE @Schema nvarchar(max)=(
   SELECT 1 AS staging_safety_version, @UnsafeStorageFeatures AS unsafe_storage_features,
     CONVERT(bit,@MetadataSysadmin) AS metadata_sysadmin,
     @ParamsMarkerAbsent AS params_marker_absent,
     CONVERT(varchar(64),@ExecutionSidHash,2) AS principal_sid_sha256,
     d.collation_name, d.compatibility_level, d.is_read_committed_snapshot_on,
     JSON_QUERY((SELECT t.name,t.is_memory_optimized,t.temporal_type,
       JSON_QUERY((SELECT c.column_id,c.name,c.system_type_id,c.user_type_id,
         c.max_length,c.precision,c.scale,c.collation_name,c.is_nullable,
         c.is_identity,c.is_computed,c.generated_always_type,c.encryption_type,
         cc.definition AS computed_definition,cc.is_persisted,dc.definition AS default_definition
         FROM sys.columns c
         LEFT JOIN sys.computed_columns cc ON cc.object_id=c.object_id AND cc.column_id=c.column_id
         LEFT JOIN sys.default_constraints dc ON dc.object_id=c.default_object_id
         WHERE c.object_id=t.object_id
         ORDER BY c.column_id FOR JSON PATH,INCLUDE_NULL_VALUES)) AS columns,
       JSON_QUERY((SELECT i.name,i.type,i.is_unique,i.is_primary_key,
         i.is_unique_constraint,i.is_disabled,i.has_filter,i.filter_definition,
         JSON_QUERY((SELECT ic.index_column_id,ic.column_id,ic.key_ordinal,
           ic.is_descending_key,ic.is_included_column FROM sys.index_columns ic
           WHERE ic.object_id=i.object_id AND ic.index_id=i.index_id
           ORDER BY ic.index_column_id FOR JSON PATH,INCLUDE_NULL_VALUES)) AS columns
         FROM sys.indexes i WHERE i.object_id=t.object_id
         ORDER BY i.index_id FOR JSON PATH,INCLUDE_NULL_VALUES)) AS indexes,
       JSON_QUERY(COALESCE((SELECT ck.name,ck.definition,ck.is_disabled,ck.is_not_trusted,
         ck.is_not_for_replication FROM sys.check_constraints ck
         WHERE ck.parent_object_id=t.object_id ORDER BY ck.name
         FOR JSON PATH,INCLUDE_NULL_VALUES),N'[]')) AS checks,
       JSON_QUERY(COALESCE((SELECT tr.name,tr.is_disabled,tr.is_instead_of_trigger,
         tr.is_not_for_replication,sm.definition FROM sys.triggers tr
         LEFT JOIN sys.sql_modules sm ON sm.object_id=tr.object_id
         WHERE tr.parent_id=t.object_id ORDER BY tr.name
         FOR JSON PATH,INCLUDE_NULL_VALUES),N'[]')) AS triggers
       FROM sys.tables t JOIN sys.schemas s ON s.schema_id=t.schema_id
       WHERE s.name=N'dbo' AND t.name IN (N'Config',N'ConfigSave')
       ORDER BY t.name COLLATE Latin1_General_100_BIN2
       FOR JSON PATH,INCLUDE_NULL_VALUES)) AS tables
   FROM sys.databases d WHERE d.database_id=DB_ID()
   FOR JSON PATH,WITHOUT_ARRAY_WRAPPER,INCLUDE_NULL_VALUES);
 IF DATALENGTH(@Schema)>@P8 OR @Schema IS NULL
   THROW 57622, 'snapshot schema budget exceeded', 1;
 DECLARE @SchemaHash varbinary(32)=HASHBYTES('SHA2_256',CONVERT(varbinary(max),@Schema));
 IF @SchemaHash<>@P1 OR CONVERT(varbinary(max),CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion')))<>CONVERT(varbinary(max),@P9)
   THROW 57623, 'snapshot profile is not measured', 1;
 -- Verify exact expected identity before any payload can be returned.
 IF NOT EXISTS (SELECT 1 FROM sys.databases d
   JOIN sys.database_recovery_status r ON r.database_id=d.database_id
   WHERE d.database_id=DB_ID() AND d.state_desc=N'ONLINE' AND d.user_access_desc=N'MULTI_USER'
   AND CONVERT(varbinary(max),CONVERT(nvarchar(128),SERVERPROPERTY('ServerName')))=CONVERT(varbinary(max),@P10)
   AND CONVERT(varbinary(max),d.name)=CONVERT(varbinary(max),@P11)
   AND CONVERT(varbinary(max),CONVERT(nvarchar(36),r.database_guid))=CONVERT(varbinary(max),@P12)
   AND CONVERT(varbinary(max),CONVERT(nvarchar(36),r.family_guid))=CONVERT(varbinary(max),@P13)
   AND CONVERT(varbinary(max),CONVERT(nvarchar(36),r.recovery_fork_guid))=CONVERT(varbinary(max),@P14))
   THROW 57628, 'snapshot target identity changed', 1;
 IF EXISTS (SELECT 1 FROM dbo.Config WHERE FileName IS NULL OR PartNo IS NULL
   OR PartNo<0 OR DataSize IS NULL OR DataSize<0 OR BinaryData IS NULL
   OR Creation IS NULL OR Modified IS NULL OR Attributes IS NULL
   OR DATALENGTH(FileName)>@P6*2 OR DATALENGTH(BinaryData)>@P3)
   OR EXISTS (SELECT 1 FROM dbo.ConfigSave WHERE FileName IS NULL OR PartNo IS NULL
   OR PartNo<0 OR DataSize IS NULL OR DataSize<0 OR BinaryData IS NULL
   OR Creation IS NULL OR Modified IS NULL OR Attributes IS NULL
   OR DATALENGTH(FileName)>@P6*2 OR DATALENGTH(BinaryData)>@P3)
   THROW 57624, 'snapshot physical row is invalid or oversized', 1;
 SELECT @BC=COALESCE(SUM(CONVERT(bigint,DATALENGTH(BinaryData))),0),
   @MC=COALESCE(SUM(CONVERT(bigint,DATALENGTH(FileName))*2+
   DATALENGTH(CONVERT(varbinary(max),Creation))+
   DATALENGTH(CONVERT(varbinary(max),Modified))+
   DATALENGTH(CONVERT(varbinary(max),Attributes))+24),0) FROM dbo.Config;
 SELECT @BS=COALESCE(SUM(CONVERT(bigint,DATALENGTH(BinaryData))),0),
   @MS=COALESCE(SUM(CONVERT(bigint,DATALENGTH(FileName))*2+
   DATALENGTH(CONVERT(varbinary(max),Creation))+
   DATALENGTH(CONVERT(varbinary(max),Modified))+
   DATALENGTH(CONVERT(varbinary(max),Attributes))+24),0) FROM dbo.ConfigSave;
 IF @BC+@BS>@P4 OR @MC+@MS>@P5
   THROW 57625, 'snapshot aggregate budget exceeded', 1;
 -- Identity and schema precede all payload resultsets, under both table locks.
 SELECT CONVERT(nvarchar(128),SERVERPROPERTY('ServerName')),d.name,
   CONVERT(varchar(36),r.database_guid),CONVERT(varchar(36),r.family_guid),
   CONVERT(varchar(36),r.recovery_fork_guid),
   CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion')),@SchemaHash
 FROM sys.databases d JOIN sys.database_recovery_status r ON r.database_id=d.database_id
 WHERE d.database_id=DB_ID() AND d.state_desc=N'ONLINE' AND d.user_access_desc=N'MULTI_USER';
 SELECT FileName,PartNo,DataSize,CONVERT(varbinary(max),Creation),
   CONVERT(varbinary(max),Modified),CONVERT(varbinary(max),Attributes),BinaryData
 FROM dbo.Config ORDER BY FileName COLLATE Latin1_General_100_BIN2,PartNo;
 SELECT FileName,PartNo,DataSize,CONVERT(varbinary(max),Creation),
   CONVERT(varbinary(max),Modified),CONVERT(varbinary(max),Attributes),BinaryData
 FROM dbo.ConfigSave ORDER BY FileName COLLATE Latin1_General_100_BIN2,PartNo;
 ROLLBACK TRANSACTION;
 SELECT @P7,@NC,@NS,@BC,@BS;
END TRY
BEGIN CATCH
 IF @@TRANCOUNT>0 ROLLBACK TRANSACTION;
 THROW;
END CATCH;
