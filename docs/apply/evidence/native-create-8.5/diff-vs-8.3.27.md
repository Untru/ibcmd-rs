# Snapshot diff

- before: `F:/ibcmd/lab/04/trace/captures/20260929-102501-c3-native-create/after` (ibcmd_rs_04_trace_c3_new, 2026-09-29T10:25:08.9842290+03:00)
- after: `F:/ibcmd/lab/04/trace/captures/20260929-143847-c85-native-create/after` (ibcmd_rs_04_trace_c85_new, 2026-09-29T14:49:19.8943380+03:00)
- date columns are stored with a year offset of 2000: 4026-09-29 is 2026-09-29

## Schema

tables: 72 -> 72; created 0, dropped 0, altered 7

- ALTERED `dbo.SchemaStorage`
    - index added: PK__SchemaSt__95006FDA632C4C95 CLUSTERED (SchemaID) UNIQUE
    - index dropped: PK__SchemaSt__95006FDA086F71E6 CLUSTERED (SchemaID)
- ALTERED `dbo.V8CMSDPWDS`
    - index added: PK__V8CMSDPW__4575608D6532B40F CLUSTERED (PwdHash) UNIQUE
    - index dropped: PK__V8CMSDPW__4575608D2E68EB71 CLUSTERED (PwdHash)
- ALTERED `dbo._ExtensionsInfo`
    - index added: PK___Extensi__AC8ED0C48245EF2A CLUSTERED (_IDRRef) UNIQUE
    - index dropped: PK___Extensi__AC8ED0C4AD5C4774 CLUSTERED (_IDRRef)
- ALTERED `dbo._ExtensionsInfoNGS`
    - index added: PK___Extensi__AC8ED0C42DBA00D4 CLUSTERED (_IDRRef) UNIQUE
    - index dropped: PK___Extensi__AC8ED0C44D998923 CLUSTERED (_IDRRef)
- ALTERED `dbo._STTModels`
    - index added: PK___STTMode__AC8ED0C4A1533C8A CLUSTERED (_IDRRef) UNIQUE
    - index dropped: PK___STTMode__AC8ED0C49900E617 CLUSTERED (_IDRRef)
- ALTERED `dbo._STTModelsDesc`
    - index added: PK___STTMode__AC8ED0C4AF843C12 CLUSTERED (_IDRRef) UNIQUE
    - index dropped: PK___STTMode__AC8ED0C4ACDC1D22 CLUSTERED (_IDRRef)
- ALTERED `dbo.v8users`
    - index added: PK__v8users__3214EC276B2E8829 CLUSTERED (ID) UNIQUE
    - index dropped: PK__v8users__3214EC27771B62CA CLUSTERED (ID)
- setting `option:service_broker_guid`: a1505699-517a-48f7-b03b-1124811cc857 -> 1888fcb4-5ff4-4bb6-9d60-16eb728ff139

## Service tables

### Files: 1 -> 1 rows (unchanged 0)

**same content, other metadata 1**: `dbcopiesparams` x1
- = `dbcopiesparams` creation 2026-09-29T10:25:04 -> 2026-09-29T14:48:31, modified 2026-09-29T10:25:04 -> 2026-09-29T14:48:31, attributes 0 -> 0

### Params: 6 -> 6 rows (unchanged 0)

**updated 3**: `DBNamesVersion-DBNames` x1, `ibparams.inf` x1, `log.inf` x1
- only the compression differs (same decoded bytes): 0
- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): 0
- content changed: 3
- content not kept, cannot tell: 0
- ~ `DBNamesVersion-DBNames` size 43 -> 43 sha db959819278e -> 5915be669579; modified 2026-09-29T10:25:05 -> 2026-09-29T14:48:37
    value: {0,806c6618-91a5-4aeb-9860-c93aedbdb5cc}  ->  {0,59b5abe5-d7bd-479f-a823-86bab70f61f1}
- ~ `ibparams.inf` size 325 -> 293 sha 7dff4122f892 -> 805b2a995c03; modified 2026-09-29T10:25:04 -> 2026-09-29T14:48:31
    value: {20,0,0,1,"",1200,86400,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,-1,0,4,5,30,"", {0,"",3,8,3,30,"","","",12333896249540139201,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600,0,2,"/AccessToken",""},0,0,0,0,0,432000,0,0,2,0,0,0, {3,6,3,60,"","","",0,465,1,"","","","",600,0,2,"",""} }  ->  {20,0,0,1,"",1200,86400,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,-1,0,4,5,30,"", {0,"",3,8,3,30,"","","",13583566173859163247,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600},0,0,0,0,0,432000,0,0,2,0,0,0, {3,6,3,60,"","","",0,465,1,"","","","",600},0}
- ~ `log.inf` size 123 -> 123 sha 1b42a6cd1350 -> f94ef4c2670f; modified 2026-09-29T10:25:04 -> 2026-09-29T14:48:31
    value: {2627d1f1-782e-454a-b639-510d82431d81,0,5,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,2}  ->  {897d6dec-38c8-4cd8-8aff-9a50dca0cf12,0,5,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,2}

**same content, other metadata 3**: `DBNames` x1, `evlogparams.inf` x1, `locale.inf` x1
- = `DBNames` creation 2026-09-29T10:25:05 -> 2026-09-29T14:48:37, modified 2026-09-29T10:25:05 -> 2026-09-29T14:48:37, attributes 0 -> 0
- = `evlogparams.inf` creation 2026-09-29T10:25:04 -> 2026-09-29T14:48:31, modified 2026-09-29T10:25:04 -> 2026-09-29T14:48:31, attributes 0 -> 0
- = `locale.inf` creation 2026-09-29T10:25:03 -> 2026-09-29T14:48:30, modified 2026-09-29T10:25:04 -> 2026-09-29T14:48:30, attributes 0 -> 0

## Data tables and counts

unchanged tables: 70; changed: 2; created: 0; dropped: 0

| table | class | rows before | rows after | checksum |
|---|---|---|---|---|
| dbo.Files | service | 1 | 1 | changed |
| dbo.Params | service | 6 | 6 | changed |

