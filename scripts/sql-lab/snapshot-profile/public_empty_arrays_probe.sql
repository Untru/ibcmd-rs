-- Source proposal only: one SELECT-only serializer probe, never SQL capture replay.
-- P1 nvarchar(128)=exact public database; P2 nvarchar(36)=exact database GUID;
-- P3 nvarchar(128)=exact SQL product version. Existing nonpooled operator context.
SET NOCOUNT ON;
SET XACT_ABORT ON;
SET LOCK_TIMEOUT 5000;
IF @@TRANCOUNT<>0 THROW 57800,'probe requires idle context',1;
IF CONVERT(varbinary(max),DB_NAME())<>CONVERT(varbinary(max),@P1)
 OR CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion'))<>@P3
 OR NOT EXISTS(SELECT 1 FROM sys.database_recovery_status
   WHERE database_id=DB_ID() AND CONVERT(varbinary(max),CONVERT(nvarchar(36),database_guid))=CONVERT(varbinary(max),@P2))
 THROW 57801,'probe context refused',1;
IF ISNULL(HAS_PERMS_BY_NAME(DB_NAME(),'DATABASE','VIEW DEFINITION'),0)<>1
 OR OBJECT_ID(N'dbo.Config',N'U') IS NULL OR OBJECT_ID(N'dbo.ConfigSave',N'U') IS NULL
 THROW 57802,'probe metadata visibility refused',1;
-- Public zero/one/two-row correlated projections with the producer's field order,
-- nullable LEFT JOIN and two outer JSON levels. No sys definitions are projected.
-- Candidate expressions are isolated here; frozen producer remains unchanged.
DECLARE @Public nvarchar(max)=(
 SELECT JSON_QUERY((SELECT p.ordinal,
   JSON_QUERY((SELECT ck.name,ck.definition,ck.is_disabled,ck.is_not_trusted,ck.is_not_for_replication
       FROM (VALUES(1,N'public-check-one',CAST(NULL AS nvarchar(32)),CAST(0 AS bit),CAST(0 AS bit),CAST(0 AS bit)),
                   (2,N'public-check-two',N'public-expression',CAST(1 AS bit),CAST(0 AS bit),CAST(1 AS bit)))
            ck(id,name,definition,is_disabled,is_not_trusted,is_not_for_replication)
       WHERE ck.id<=p.ordinal ORDER BY ck.name FOR JSON PATH,INCLUDE_NULL_VALUES)) AS original_checks,
   JSON_QUERY(COALESCE((SELECT ck.name,ck.definition,ck.is_disabled,ck.is_not_trusted,ck.is_not_for_replication
       FROM (VALUES(1,N'public-check-one',CAST(NULL AS nvarchar(32)),CAST(0 AS bit),CAST(0 AS bit),CAST(0 AS bit)),
                   (2,N'public-check-two',N'public-expression',CAST(1 AS bit),CAST(0 AS bit),CAST(1 AS bit)))
            ck(id,name,definition,is_disabled,is_not_trusted,is_not_for_replication)
       WHERE ck.id<=p.ordinal ORDER BY ck.name FOR JSON PATH,INCLUDE_NULL_VALUES),N'[]')) AS candidate_checks,
   JSON_QUERY((SELECT tr.name,tr.is_disabled,tr.is_instead_of_trigger,tr.is_not_for_replication,sm.definition
       FROM (VALUES(1,N'public-trigger-one',CAST(0 AS bit),CAST(0 AS bit),CAST(0 AS bit)),
                   (2,N'public-trigger-two',CAST(1 AS bit),CAST(0 AS bit),CAST(1 AS bit)))
            tr(id,name,is_disabled,is_instead_of_trigger,is_not_for_replication)
       LEFT JOIN (VALUES(2,N'public-module')) sm(id,definition) ON sm.id=tr.id
       WHERE tr.id<=p.ordinal ORDER BY tr.name FOR JSON PATH,INCLUDE_NULL_VALUES)) AS original_triggers,
   JSON_QUERY(COALESCE((SELECT tr.name,tr.is_disabled,tr.is_instead_of_trigger,tr.is_not_for_replication,sm.definition
       FROM (VALUES(1,N'public-trigger-one',CAST(0 AS bit),CAST(0 AS bit),CAST(0 AS bit)),
                   (2,N'public-trigger-two',CAST(1 AS bit),CAST(0 AS bit),CAST(1 AS bit)))
            tr(id,name,is_disabled,is_instead_of_trigger,is_not_for_replication)
       LEFT JOIN (VALUES(2,N'public-module')) sm(id,definition) ON sm.id=tr.id
       WHERE tr.id<=p.ordinal ORDER BY tr.name FOR JSON PATH,INCLUDE_NULL_VALUES),N'[]')) AS candidate_triggers
 FROM (VALUES(0),(1),(2)) p(ordinal) ORDER BY p.ordinal
 FOR JSON PATH,INCLUDE_NULL_VALUES)) AS tables
 FOR JSON PATH,WITHOUT_ARRAY_WRAPPER,INCLUDE_NULL_VALUES);
IF @Public IS NULL OR DATALENGTH(@Public)>16384 THROW 57803,'probe public budget refused',1;
DECLARE @checks_one nvarchar(max)=N'[{"name":"public-check-one","definition":null,"is_disabled":false,"is_not_trusted":false,"is_not_for_replication":false}]';
DECLARE @checks_two nvarchar(max)=N'[{"name":"public-check-one","definition":null,"is_disabled":false,"is_not_trusted":false,"is_not_for_replication":false},{"name":"public-check-two","definition":"public-expression","is_disabled":true,"is_not_trusted":false,"is_not_for_replication":true}]';
DECLARE @triggers_one nvarchar(max)=N'[{"name":"public-trigger-one","is_disabled":false,"is_instead_of_trigger":false,"is_not_for_replication":false,"definition":null}]';
DECLARE @triggers_two nvarchar(max)=N'[{"name":"public-trigger-one","is_disabled":false,"is_instead_of_trigger":false,"is_not_for_replication":false,"definition":null},{"name":"public-trigger-two","is_disabled":true,"is_instead_of_trigger":false,"is_not_for_replication":true,"definition":"public-module"}]';
-- Public observation controls: literal null and arrays admit; all other shapes
-- (including a quoted array, missing or duplicate key) must refuse the oracle.
IF EXISTS(SELECT 1
 FROM (VALUES(0,N'{"v":null}',1,0),(1,N'{"v":[]}',1,4),
             (2,N'{"v":"[]"}',0,-1),(3,N'{}',0,-1),(4,N'{"v":{}}',0,-1),
             (5,N'{"v":0}',0,-1),(6,N'{"v":false}',0,-1),
             (7,N'{"v":null,"v":[]}',0,-1)) c(ordinal,json_text,expected_admit,expected_type)
 OUTER APPLY(SELECT COUNT_BIG(*) AS property_count,MAX(p.[type]) AS property_type
   FROM OPENJSON(c.json_text) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'v') t
 WHERE CASE WHEN property_count=1 AND property_type IN (0,4) THEN 1 ELSE 0 END<>expected_admit
 OR (expected_admit=1 AND property_type<>expected_type))
 THROW 57806,'probe public observation controls refused',1;
-- Default OPENJSON key/type preserves literal null vs missing/scalar/string.
-- AS JSON/lax must not reduce these different observations to SQL NULL.
IF (SELECT COUNT_BIG(*) FROM OPENJSON(@Public,'$.tables'))<>3
 THROW 57804,'probe row envelope refused',1;
IF EXISTS(SELECT 1 FROM (
 SELECT CONVERT(bigint,r.[key]) AS ordinal,r.[type] AS row_type,original_checks_count,original_checks_type,original_checks,candidate_checks_count,candidate_checks_type,candidate_checks,original_triggers_count,original_triggers_type,original_triggers,candidate_triggers_count,candidate_triggers_type,candidate_triggers
 FROM OPENJSON(@Public,'$.tables') r
 OUTER APPLY(SELECT COUNT_BIG(*) AS original_checks_count,MAX(p.[type]) AS original_checks_type,MAX(p.[value]) AS original_checks
   FROM OPENJSON(r.[value]) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'original_checks') p0
 OUTER APPLY(SELECT COUNT_BIG(*) AS candidate_checks_count,MAX(p.[type]) AS candidate_checks_type,MAX(p.[value]) AS candidate_checks
   FROM OPENJSON(r.[value]) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'candidate_checks') p1
 OUTER APPLY(SELECT COUNT_BIG(*) AS original_triggers_count,MAX(p.[type]) AS original_triggers_type,MAX(p.[value]) AS original_triggers
   FROM OPENJSON(r.[value]) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'original_triggers') p2
 OUTER APPLY(SELECT COUNT_BIG(*) AS candidate_triggers_count,MAX(p.[type]) AS candidate_triggers_type,MAX(p.[value]) AS candidate_triggers
   FROM OPENJSON(r.[value]) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'candidate_triggers') p3
) o WHERE row_type<>5 OR ordinal NOT BETWEEN 0 AND 2
 OR original_checks_count<>1 OR original_checks_type NOT IN (0,4)
 OR candidate_checks_count<>1 OR candidate_checks_type NOT IN (0,4)
 OR original_triggers_count<>1 OR original_triggers_type NOT IN (0,4)
 OR candidate_triggers_count<>1 OR candidate_triggers_type NOT IN (0,4))
 THROW 57805,'probe property presence or type refused',1;
;WITH Observed AS (
 SELECT CONVERT(bigint,r.[key]) AS ordinal,r.[type] AS row_type,original_checks_count,original_checks_type,original_checks,candidate_checks_count,candidate_checks_type,candidate_checks,original_triggers_count,original_triggers_type,original_triggers,candidate_triggers_count,candidate_triggers_type,candidate_triggers
 FROM OPENJSON(@Public,'$.tables') r
 OUTER APPLY(SELECT COUNT_BIG(*) AS original_checks_count,MAX(p.[type]) AS original_checks_type,MAX(p.[value]) AS original_checks
   FROM OPENJSON(r.[value]) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'original_checks') p0
 OUTER APPLY(SELECT COUNT_BIG(*) AS candidate_checks_count,MAX(p.[type]) AS candidate_checks_type,MAX(p.[value]) AS candidate_checks
   FROM OPENJSON(r.[value]) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'candidate_checks') p1
 OUTER APPLY(SELECT COUNT_BIG(*) AS original_triggers_count,MAX(p.[type]) AS original_triggers_type,MAX(p.[value]) AS original_triggers
   FROM OPENJSON(r.[value]) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'original_triggers') p2
 OUTER APPLY(SELECT COUNT_BIG(*) AS candidate_triggers_count,MAX(p.[type]) AS candidate_triggers_type,MAX(p.[value]) AS candidate_triggers
   FROM OPENJSON(r.[value]) p WHERE p.[key] COLLATE Latin1_General_100_BIN2=N'candidate_triggers') p3
)
SELECT ordinal,
 CASE WHEN original_checks_type=0 THEN N'NULL' WHEN original_checks=N'[]' THEN N'ARRAY_EMPTY'
      ELSE N'ARRAY_NONEMPTY' END AS original_checks_shape,
 CONVERT(bigint,CASE WHEN original_checks_type=4 AND CONVERT(varbinary(max),original_checks)=CONVERT(varbinary(max),CASE ordinal WHEN 0 THEN N'[]' WHEN 1 THEN @checks_one ELSE @checks_two END) THEN 1 ELSE 0 END) AS original_checks_exact,
 CASE WHEN candidate_checks_type=0 THEN N'NULL' WHEN candidate_checks=N'[]' THEN N'ARRAY_EMPTY'
      ELSE N'ARRAY_NONEMPTY' END AS candidate_checks_shape,
 CONVERT(bigint,CASE WHEN candidate_checks_type=4 AND CONVERT(varbinary(max),candidate_checks)=CONVERT(varbinary(max),CASE ordinal WHEN 0 THEN N'[]' WHEN 1 THEN @checks_one ELSE @checks_two END) THEN 1 ELSE 0 END) AS candidate_checks_exact,
 CASE WHEN original_triggers_type=0 THEN N'NULL' WHEN original_triggers=N'[]' THEN N'ARRAY_EMPTY'
      ELSE N'ARRAY_NONEMPTY' END AS original_triggers_shape,
 CONVERT(bigint,CASE WHEN original_triggers_type=4 AND CONVERT(varbinary(max),original_triggers)=CONVERT(varbinary(max),CASE ordinal WHEN 0 THEN N'[]' WHEN 1 THEN @triggers_one ELSE @triggers_two END) THEN 1 ELSE 0 END) AS original_triggers_exact,
 CASE WHEN candidate_triggers_type=0 THEN N'NULL' WHEN candidate_triggers=N'[]' THEN N'ARRAY_EMPTY'
      ELSE N'ARRAY_NONEMPTY' END AS candidate_triggers_shape,
 CONVERT(bigint,CASE WHEN candidate_triggers_type=4 AND CONVERT(varbinary(max),candidate_triggers)=CONVERT(varbinary(max),CASE ordinal WHEN 0 THEN N'[]' WHEN 1 THEN @triggers_one ELSE @triggers_two END) THEN 1 ELSE 0 END) AS candidate_triggers_exact
FROM Observed ORDER BY ordinal;
-- Only existence-of-rows flags for two fixed public tables; no names/definitions.
SELECT CONVERT(bigint,p.ordinal) AS table_ordinal,
 CONVERT(bigint,CASE WHEN EXISTS(SELECT 1 FROM sys.check_constraints ck WHERE ck.parent_object_id=OBJECT_ID(p.fixed_name,N'U')) THEN 0 ELSE 1 END) AS check_rows_zero,
 CONVERT(bigint,CASE WHEN EXISTS(SELECT 1 FROM sys.triggers tr WHERE tr.parent_id=OBJECT_ID(p.fixed_name,N'U')) THEN 0 ELSE 1 END) AS trigger_rows_zero
FROM (VALUES(0,N'dbo.Config'),(1,N'dbo.ConfigSave')) p(ordinal,fixed_name) ORDER BY p.ordinal;
