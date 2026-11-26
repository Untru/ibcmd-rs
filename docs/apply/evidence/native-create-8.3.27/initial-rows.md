# Rows written by `infobase create` (decoded)

Three runs on empty databases (see `../../native-infobase-create.md`). Values are the decoded text of the stored bytes;
`stored` is the size in the table. Dates are shown as stored (no year offset was applied by create).

## run 1, ru_RU

* `Params.locale.inf` stored 112 B, created 2026-09-29T10:25:03.0000000, kind v8text: `{"ru_RU",0,0,"",-1,"","","","",1,0,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000}`
* `Params.log.inf` stored 123 B, created 2026-09-29T10:25:04.0000000, kind v8text: `{2627d1f1-782e-454a-b639-510d82431d81,0,5,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,2}`
* `Params.evlogparams.inf` stored 6 B, created 2026-09-29T10:25:04.0000000, kind v8text: `{1}`
* `Params.ibparams.inf` stored 325 B, created 2026-09-29T10:25:04.0000000, kind v8text: `{20,0,0,1,"",1200,86400,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,-1,0,4,5,30,"",
{0,"",3,8,3,30,"","","",12333896249540139201,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600,0,2,"/AccessToken",""},0,0,0,0,0,432000,0,0,2,0,0,0,
{3,6,3,60,"","","",0,465,1,"","","","",600,0,2,"",""}
}`
* `Params.DBNamesVersion-DBNames` stored 43 B, created 2026-09-29T10:25:05.0000000, kind v8text: `{0,806c6618-91a5-4aeb-9860-c93aedbdb5cc}`
* `Files.dbcopiesparams` stored 8 B, created 2026-09-29T10:25:04.0000000, kind v8text: `{1,2}`

## run 2, ru_RU

* `Params.locale.inf` stored 112 B, created 2026-09-29T11:11:06.0000000, kind v8text: `{"ru_RU",0,0,"",-1,"","","","",1,0,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000}`
* `Params.log.inf` stored 123 B, created 2026-09-29T11:11:06.0000000, kind v8text: `{3b3fcda5-2789-45f1-b1ba-7b5bbc1097ee,0,5,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,2}`
* `Params.evlogparams.inf` stored 6 B, created 2026-09-29T11:11:06.0000000, kind v8text: `{1}`
* `Params.ibparams.inf` stored 324 B, created 2026-09-29T11:11:06.0000000, kind v8text: `{20,0,0,1,"",1200,86400,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,-1,0,4,5,30,"",
{0,"",3,8,3,30,"","","",5763501942151448049,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600,0,2,"/AccessToken",""},0,0,0,0,0,432000,0,0,2,0,0,0,
{3,6,3,60,"","","",0,465,1,"","","","",600,0,2,"",""}
}`
* `Params.DBNamesVersion-DBNames` stored 43 B, created 2026-09-29T11:11:08.0000000, kind v8text: `{0,74807a95-a45b-46c0-97e5-9622edad7d5f}`
* `Files.dbcopiesparams` stored 8 B, created 2026-09-29T11:11:06.0000000, kind v8text: `{1,2}`

## run 3, en_US

* `Params.locale.inf` stored 112 B, created 2026-09-29T11:11:28.0000000, kind v8text: `{"en_US",0,0,"",-1,"","","","",1,0,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000}`
* `Params.log.inf` stored 123 B, created 2026-09-29T11:11:29.0000000, kind v8text: `{f1f2390f-de55-4d9e-afb7-750c6bf4f7c5,0,5,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,2}`
* `Params.evlogparams.inf` stored 6 B, created 2026-09-29T11:11:29.0000000, kind v8text: `{1}`
* `Params.ibparams.inf` stored 325 B, created 2026-09-29T11:11:29.0000000, kind v8text: `{20,0,0,1,"",1200,86400,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,-1,0,4,5,30,"",
{0,"",3,8,3,30,"","","",12737347375708166306,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600,0,2,"/AccessToken",""},0,0,0,0,0,432000,0,0,2,0,0,0,
{3,6,3,60,"","","",0,465,1,"","","","",600,0,2,"",""}
}`
* `Params.DBNamesVersion-DBNames` stored 43 B, created 2026-09-29T11:11:31.0000000, kind v8text: `{0,032231e5-63ae-45ec-a51e-9eb49b9b92b3}`
* `Files.dbcopiesparams` stored 8 B, created 2026-09-29T11:11:29.0000000, kind v8text: `{1,2}`

## Non-file tables that hold rows after create (run 1)

* `IBVersion`: IBVersion=7; PlatformVersionReq=80313; 
* `_YearOffset`: Offset=2000; 
* `DBSchema`: SerializedData=len:22114 sha:14c15a71135f blob; 
* `SchemaStorage`: SchemaID=0; Status=100; CurrentSchema=len:22114 sha:14c15a71135f blob; NewGenCreated=EFBBBF7B302C0D0A7B307D0D0A7D; NewGenDropped=EFBBBF7B302C0D0A7B307D0D0A7D; 
