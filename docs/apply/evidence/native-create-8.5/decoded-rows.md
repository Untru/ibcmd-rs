# `infobase create` on platform 8.5.1.1150 (run 4), decoded

Command: `ibcmd.exe infobase create --dbms=MSSQLServer --db-server=localhost --db-name=<db> --data=<dir> --locale=ru_RU`
on an empty database (collation Cyrillic_General_CI_AS), traced with the kit (capture `c85-native-create`).

* DDL statements: 8.3.27 141, 8.5 141; identical statement for statement and in the same order after
  the automatic constraint names (`PK__...`) are masked: True.
* `DBSchema` (22 114 B, BOM), `DBNames`, `locale.inf`, `evlogparams.inf`, `Files.dbcopiesparams`, `IBVersion`, `_YearOffset`: identical.
* Differs: the three random values and `ibparams.inf` (293 B against 325 B, below).

## 8.3.27.2214 (run 1)

* `Params.ibparams.inf` stored 325 B: `{20,0,0,1,"",1200,86400,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,-1,0,4,5,30,"",
{0,"",3,8,3,30,"","","",12333896249540139201,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600,0,2,"/AccessToken",""},0,0,0,0,0,432000,0,0,2,0,0,0,
{3,6,3,60,"","","",0,465,1,"","","","",600,0,2,"",""}
}`
* `Params.log.inf` stored 123 B: `{2627d1f1-782e-454a-b639-510d82431d81,0,5,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,2}`
* `Params.DBNamesVersion-DBNames` stored 43 B: `{0,806c6618-91a5-4aeb-9860-c93aedbdb5cc}`
* `Params.locale.inf` stored 112 B: `{"ru_RU",0,0,"",-1,"","","","",1,0,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000}`
* `IBVersion`: IBVersion=7; PlatformVersionReq=80313; 
* `_YearOffset`: Offset=2000; 

## 8.5.1.1150 (run 4)

* `Params.ibparams.inf` stored 293 B: `{20,0,0,1,"",1200,86400,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,-1,0,4,5,30,"",
{0,"",3,8,3,30,"","","",13583566173859163247,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600},0,0,0,0,0,432000,0,0,2,0,0,0,
{3,6,3,60,"","","",0,465,1,"","","","",600},0}`
* `Params.log.inf` stored 123 B: `{897d6dec-38c8-4cd8-8aff-9a50dca0cf12,0,5,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,1,2}`
* `Params.DBNamesVersion-DBNames` stored 43 B: `{0,59b5abe5-d7bd-479f-a823-86bab70f61f1}`
* `Params.locale.inf` stored 112 B: `{"ru_RU",0,0,"",-1,"","","","",1,0,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000}`
* `IBVersion`: IBVersion=7; PlatformVersionReq=80313; 
* `_YearOffset`: Offset=2000; 

