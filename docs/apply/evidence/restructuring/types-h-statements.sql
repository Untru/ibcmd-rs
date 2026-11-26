-- Case h (types): the new-generation tables of the catalogs that got attributes, and the data copy.
-- 07:40:26.389000 sid=139 sql_batch_completed rows=0 dur=6448us
create table dbo._Reference15_VT11039NG (
_Reference15_IDRRef binary(16) not null,
_Fld2683 numeric(7, 0) not null,
_KeyField binary(4) not null,
_LineNo11040 numeric(5, 0) not null,
_Fld11041 nvarchar(10) not null,
_Fld11042 nvarchar(max) not null
)
;alter table dbo._Reference15_VT11039NG SET (LOCK_ESCALATION = DISABLE);

-- 07:40:27.622000 sid=139 sql_batch_completed rows=0 dur=7947us
create table dbo._Reference27NG (
_IDRRef binary(16) not null,
_Version timestamp not null,
_Marked binary(1) not null,
_PredefinedID binary(16) not null,
_ParentIDRRef binary(16) not null,
_Folder binary(1) not null,
_Description nvarchar(150) not null,
_Fld8014 binary(1),
_Fld3624 binary(1),
_Fld5563 nvarchar(20),
_Fld7873 nvarchar(20),
_Fld4940 binary(1),
_Fld7935 nvarchar(150),
_Fld7348 nvarchar(150),
_Fld7347 nvarchar(150) not null,
_Fld5075 binary(1) not null,
_Fld7916 binary(1),
_Fld8013 nvarchar(150),
_Fld7058 binary(1),
_Fld218 binary(1),
_Fld3625 binary(1),
_Fld3626 binary(1),
_Fld4743 binary(1),
_Fld3630 binary(1),
_Fld216 numeric(5, 0),
_Fld3628 binary(1),
_Fld5027 binary(1),
_Fld215RRef binary(16),
_Fld219 binary(1),
_Fld217 binary(1),
_Fld5025 binary(1),
_Fld5387 binary(1),
_Fld9095 binary(1),
_Fld11044 nvarchar(30),
_Fld7857 nvarchar(150) not null,
_Fld7858 nvarchar(150) not null,
_Fld2683 numeric(7, 0) not null,
_Fld9894 nvarchar(max) not null
)
;alter table dbo._Reference27NG SET (LOCK_ESCALATION = DISABLE);

-- 07:40:28.043000 sid=139 sql_batch_completed rows=0 dur=9229us
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
_Fld11045 binary(1) not null,
_Fld11046 datetime2(0) not null,
_Fld11047 datetime2(0) not null,
_Fld11048 numeric(10, 0) not null,
_Fld11049 numeric(12, 3) not null,
_Fld11050 nchar(20) not null,
_Fld11051 nvarchar(max) not null,
_Fld11052RRef binary(16) not null,
_Fld11053RRef binary(16) not null,
_Fld11054_TYPE binary(1) not null,
_Fld11054_N numeric(10, 2) not null,
_Fld11054_S nvarchar(15) not null,
_Fld11054_RRRef binary(16) not null,
_Fld11055 varbinary(max) not null,
_Fld11056 binary(16) not null
)
;alter table dbo._Reference2598NG SET (LOCK_ESCALATION = DISABLE);

-- 07:40:28.123000 sid=139 rpc_completed rows=716 dur=18131us
-- args: ,N'@P1 datetime2(3),@P2 datetime2(3),@P3 numeric(10),@P4 numeric(10),@P5 nvarchar(4000),@P6 nvarchar(4000),@P7 numeric(10),@P8 nvarchar(4000),@P9 varbinary(8000),@P10 varbinary(8000)','2001-01-01 00:00:00.4074577920','2001-01-01 00:00:00.4074577920',0,0,N'                    ',N'',0,N'',0x01010800000000000000EFBBBF7B2255227D,0x00000000000000000000000000000000
INSERT INTO dbo._Reference2598NG WITH(TABLOCK) (_IDRRef, _Marked, _PredefinedID, _Description, _Fld4266, _Fld2644RRef, _Fld2645, _Fld2646, _Fld5283, _Fld5921, _Fld6495, _Fld11045, _Fld11046, _Fld11047, _Fld11048, _Fld11049, _Fld11050, _Fld11051, _Fld11052RRef, _Fld11053RRef, _Fld11054_TYPE, _Fld11054_N, _Fld11054_S, _Fld11054_RRRef, _Fld11055, _Fld11056) SELECT
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
@P1,
@P2,
CAST(@P3 AS NUMERIC(10, 0)),
CAST(@P4 AS NUMERIC(12, 3)),
CAST(@P5 AS NCHAR(20)),
@P6,
0x00000000000000000000000000000000,
0x00000000000000000000000000000000,
0x01,
@P7,
@P8,
0x00000000000000000000000000000000,
@P9,
@P10
FROM dbo._Reference2598 T1 WITH(NOLOCK);

-- 07:40:39.148000 sid=139 rpc_completed rows=25 dur=2641us
-- args: N'_Reference15_VT11039NG',N'_Reference15_VT11039'
exec @P1 = sp_rename @P2, @P3, 'object';

-- 07:40:39.417000 sid=139 rpc_completed rows=25 dur=3853us
-- args: N'_Reference2598NG',N'_Reference2598'
exec @P1 = sp_rename @P2, @P3, 'object';

