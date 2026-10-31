-- Operator-only synthetic lab metadata capture. No BinaryData is returned.
-- One parameterized request on an already authorized, disposable connection.
-- P1..P14 types/order match snapshot.rs; P1 may be NULL for measurement only.
SET NOCOUNT ON;
SET XACT_ABORT ON;
SET LOCK_TIMEOUT 5000;
IF @@TRANCOUNT <> 0 THROW 57700, 'capture requires an idle session', 1;
IF @P2 IS NULL OR @P2 NOT BETWEEN 1 AND 1024 OR @P3 IS NULL OR @P3 NOT BETWEEN 1 AND 1048576
 OR @P4 IS NULL OR @P4 NOT BETWEEN 1 AND 8388608 OR @P5 IS NULL OR @P5 NOT BETWEEN 1 AND 262144
 OR @P6 IS NULL OR @P6 NOT BETWEEN 1 AND 128 OR @P8 IS NULL OR @P8 NOT BETWEEN 1 AND 32768
 THROW 57701, 'capture limits are outside operator envelope', 1;
-- Exact intended database before resolving application storage definitions.
IF CONVERT(varbinary(max),DB_NAME())<>CONVERT(varbinary(max),@P11)
 THROW 57702, 'capture database context differs', 1;
BEGIN TRY
 BEGIN TRANSACTION;
 IF ISNULL(HAS_PERMS_BY_NAME(DB_NAME(),'DATABASE','VIEW DEFINITION'),0)<>1
   THROW 57626, 'snapshot requires visible storage definitions', 1;
 IF OBJECT_ID(N'dbo.Config',N'U') IS NULL OR OBJECT_ID(N'dbo.ConfigSave',N'U') IS NULL
   THROW 57627, 'snapshot storage tables are unsupported', 1;
 DECLARE @NC bigint, @NS bigint, @BC bigint, @BS bigint, @MC bigint, @MS bigint;
 -- Deterministic table lock order; TOP caps counting work for oversized targets.
 SELECT @NC=COUNT_BIG(*) FROM
   (SELECT TOP (@P2+1) FileName FROM dbo.Config WITH (TABLOCK,HOLDLOCK)) c;
 SELECT @NS=COUNT_BIG(*) FROM
   (SELECT TOP (@P2+1) FileName FROM dbo.ConfigSave WITH (TABLOCK,HOLDLOCK)) s;
 IF @NC+@NS>@P2 THROW 57621, 'snapshot part count exceeded', 1;
 DECLARE @Schema nvarchar(max)=(
   SELECT d.collation_name, d.compatibility_level, d.is_read_committed_snapshot_on,
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
 IF @P1 IS NOT NULL AND @SchemaHash<>@P1
   THROW 57703, 'capture schema changed from expected measurement', 1;
 IF @P9 IS NULL OR CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion'))<>@P9
   THROW 57704, 'capture SQL version differs', 1;
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

 IF EXISTS (SELECT 1 FROM dbo.Config WHERE
   DATALENGTH(CONVERT(varbinary(max),Creation)) NOT BETWEEN 1 AND 64 OR
   DATALENGTH(CONVERT(varbinary(max),Modified)) NOT BETWEEN 1 AND 64 OR
   DATALENGTH(CONVERT(varbinary(max),Attributes)) NOT BETWEEN 1 AND 64)
 OR EXISTS (SELECT 1 FROM dbo.ConfigSave WHERE
   DATALENGTH(CONVERT(varbinary(max),Creation)) NOT BETWEEN 1 AND 64 OR
   DATALENGTH(CONVERT(varbinary(max),Modified)) NOT BETWEEN 1 AND 64 OR
   DATALENGTH(CONVERT(varbinary(max),Attributes)) NOT BETWEEN 1 AND 64)
 THROW 57705, 'capture header shape is unsupported', 1;
 SELECT CONVERT(nvarchar(128),SERVERPROPERTY('ServerName')) AS server,
   d.name AS database_name,CONVERT(varchar(36),r.database_guid) AS database_guid,
   CONVERT(varchar(36),r.family_guid) AS family_guid,
   CONVERT(varchar(36),r.recovery_fork_guid) AS recovery_fork,
   CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion')) AS sql_version,
   @Schema AS schema_json,CONVERT(varchar(64),@SchemaHash,2) AS schema_sha256,
   CONVERT(bigint,DATALENGTH(@Schema)) AS schema_bytes,@P7 AS nonce,
   CONVERT(bigint,@@SPID) AS session_id,CONVERT(bigint,@@TRANCOUNT) AS transaction_count
 FROM sys.databases d JOIN sys.database_recovery_status r ON r.database_id=d.database_id
 WHERE d.database_id=DB_ID();
 SELECT FileName AS file_name,CONVERT(bigint,PartNo) AS part_no,
   CONVERT(bigint,DataSize) AS data_size,
   CONVERT(varchar(128),CONVERT(varbinary(max),Creation),2) AS creation_hex,
   CONVERT(varchar(128),CONVERT(varbinary(max),Modified),2) AS modified_hex,
   CONVERT(varchar(128),CONVERT(varbinary(max),Attributes),2) AS attributes_hex,
   CONVERT(bigint,DATALENGTH(BinaryData)) AS payload_bytes,
   CONVERT(varchar(64),HASHBYTES('SHA2_256',BinaryData),2) AS payload_sha256
 FROM dbo.Config ORDER BY FileName COLLATE Latin1_General_100_BIN2,PartNo;
 SELECT FileName AS file_name,CONVERT(bigint,PartNo) AS part_no,
   CONVERT(bigint,DataSize) AS data_size,
   CONVERT(varchar(128),CONVERT(varbinary(max),Creation),2) AS creation_hex,
   CONVERT(varchar(128),CONVERT(varbinary(max),Modified),2) AS modified_hex,
   CONVERT(varchar(128),CONVERT(varbinary(max),Attributes),2) AS attributes_hex,
   CONVERT(bigint,DATALENGTH(BinaryData)) AS payload_bytes,
   CONVERT(varchar(64),HASHBYTES('SHA2_256',BinaryData),2) AS payload_sha256
 FROM dbo.ConfigSave ORDER BY FileName COLLATE Latin1_General_100_BIN2,PartNo;
 ROLLBACK TRANSACTION;
 SELECT @P7 AS nonce,@NC AS config_parts,@NS AS configsave_parts,
   @BC AS config_bytes,@BS AS configsave_bytes,
   CONVERT(bigint,@@SPID) AS session_id,CONVERT(bigint,@@TRANCOUNT) AS transaction_count;
END TRY
BEGIN CATCH
 IF @@TRANCOUNT>0 ROLLBACK TRANSACTION;
 THROW;
END CATCH;
