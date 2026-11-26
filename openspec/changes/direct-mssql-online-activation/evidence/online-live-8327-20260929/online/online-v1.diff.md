# Snapshot diff

- before: `F:\ibcmd\lab\05\online\runs\online\cap\20260929-200430-online-v1\before` (ibcmd_rs_05_online_a1, 2026-09-29T20:08:13.3704517+03:00)
- after: `F:\ibcmd\lab\05\online\runs\online\cap\20260929-200430-online-v1\after` (ibcmd_rs_05_online_a1, 2026-09-29T20:10:47.1455509+03:00)
- date columns are stored with a year offset of 2000: 4026-09-29 is 2026-09-29

## Schema

tables: 2234 -> 2234; created 0, dropped 0, altered 0

no schema changes

## Service tables

### Config: 9838 -> 9842 rows (unchanged 9836)

**inserted 4**: `<guid>_dynupdate_<guid>` x1, `<guid>_dynupdate_<guid>.<n>` x1, `DynamicallyUpdated` x1, `versions_dynupdate_<guid>` x1
- + `313d9858-3995-4a4c-b2b0-15d2350417b4_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f` size 165 (stored 165, 1 part) sha 68173761147f deflate v8text  `{1, {12, {3, {1,0,313d9858-3995-4a4c-b2b0-15d2350417b4},"ОбсужденияСлужебныйКлиентСервер", {1,"ru...`
- + `313d9858-3995-4a4c-b2b0-15d2350417b4_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f.0` size 704 (stored 704, 1 part) sha edba3df25528 deflate container
- + `DynamicallyUpdated` size 45 (stored 45, 1 part) sha b2bf1fcdc585 raw v8text  `{1,1,4832dd20-0b9e-45e3-a3e7-095525bf9b3f}`
- + `versions_dynupdate_4832dd20-0b9e-45e3-a3e7-095525bf9b3f` size 344212 (stored 344212, 1 part) sha 46cf96caf891 deflate v8text

**same content, other metadata 2**: `root` x1, `version` x1
- = `root` creation 4026-09-19T23:32:53 -> 2026-09-29T17:08:33, modified 4026-09-19T23:32:53 -> 2026-09-29T17:08:33, attributes 0 -> 0
- = `version` creation 4026-09-19T23:32:53 -> 2026-09-29T17:08:33, modified 4026-09-19T23:32:53 -> 2026-09-29T17:08:33, attributes 0 -> 0

### Params: 37 -> 38 rows (unchanged 37)

**inserted 1**: `DynamicallyUpdated` x1
- + `DynamicallyUpdated` size 82 (stored 82, 1 part) sha f05f5efb4431 raw v8text  `{0,2,848a0a59-8803-445f-b89d-3cfae4f98bbd,4832dd20-0b9e-45e3-a3e7-095525bf9b3f}`

### _CommonSettings: 74 -> 74 rows (unchanged 73)

- + `row:d3e98ea9752d0b83` _UserId=Администратор; _ObjectKey=ТекущиеДела; _SettingsKey=НастройкиОтображения; _Version=92C7F80E6913790547A726B9FDE63A59; _SettingsPresentation=; _SettingsData=len:2607 sha:32c41d2b1f5e blob; _ChangeDate=4026-09-29T17:09:15.0000000; _...
- - `row:d288ed40209c76bc` _UserId=Администратор; _ObjectKey=ТекущиеДела; _SettingsKey=НастройкиОтображения; _Version=8CCD368A01AE27234ED08AEC149C33F8; _SettingsPresentation=; _SettingsData=len:2607 sha:9a434943e053 blob; _ChangeDate=4026-09-29T17:01:55.0000000; _...

### _UsersWorkHistory: 714 -> 715 rows (unchanged 714)

- + `row:81935341bc4a87a4` _ID=81F354D7D198270A4132D2A3CF8836D9; _UserID=8465AB078A9F6F2F42FDF680AD1DB191; _URL=e1cib/app/Обработка.ИнформацияПриЗапуске; _Date=4026-09-29T20:00:49.0000000; _URLHash=3541841427; _ECSActivity=00; _DataSeparationUse3609=01; _DataSepar...

## Data tables and counts

unchanged tables: 2231; changed: 3; created: 0; dropped: 0

| table | class | rows before | rows after | checksum |
|---|---|---|---|---|
| dbo.Config | service | 9841 | 9845 | same |
| dbo.Params | service | 37 | 38 | same |
| dbo._ConstChngR9527 | data | 0 | 3 | same |

