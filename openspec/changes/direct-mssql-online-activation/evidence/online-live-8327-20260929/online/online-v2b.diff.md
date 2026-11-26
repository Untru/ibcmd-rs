# Snapshot diff

- before: `F:\ibcmd\lab\05\online\runs\online\cap\20260929-203015-online-v2b\before` (ibcmd_rs_05_online_a1, 2026-09-29T20:32:01.0269055+03:00)
- after: `F:\ibcmd\lab\05\online\runs\online\cap\20260929-203015-online-v2b\after` (ibcmd_rs_05_online_a1, 2026-09-29T20:34:26.2957345+03:00)
- date columns are stored with a year offset of 2000: 4026-09-29 is 2026-09-29

## Schema

tables: 2234 -> 2234; created 0, dropped 0, altered 0

no schema changes

## Service tables

### Config: 9842 -> 9845 rows (unchanged 9839)

**inserted 3**: `<guid>_dynupdate_<guid>` x1, `<guid>_dynupdate_<guid>.<n>` x1, `versions_dynupdate_<guid>` x1
- + `16b3681c-426d-4d6f-9ffe-588a23974222_dynupdate_96eee589-ca6c-4269-8b14-1ca1ee9b69ed` size 174 (stored 174, 1 part) sha 2ca5a62b3681 deflate v8text  `{1, {12, {3, {1,0,16b3681c-426d-4d6f-9ffe-588a23974222},"РаботаСКлассификаторамиКлиентСервер", {1...`
- + `16b3681c-426d-4d6f-9ffe-588a23974222_dynupdate_96eee589-ca6c-4269-8b14-1ca1ee9b69ed.0` size 923 (stored 923, 1 part) sha 341adcc9499c deflate container
- + `versions_dynupdate_96eee589-ca6c-4269-8b14-1ca1ee9b69ed` size 344220 (stored 344220, 1 part) sha 70a2929b1de0 deflate v8text

**updated 1**: `DynamicallyUpdated` x1
- only the compression differs (same decoded bytes): 0
- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): 0
- content changed: 1
- content not kept, cannot tell: 0
- ~ `DynamicallyUpdated` size 45 -> 82 sha b2bf1fcdc585 -> 6cca7bb82241; modified 2026-09-29T17:08:36 -> 2026-09-29T17:32:07
    value: {1,1,4832dd20-0b9e-45e3-a3e7-095525bf9b3f}  ->  {1,2,4832dd20-0b9e-45e3-a3e7-095525bf9b3f,96eee589-ca6c-4269-8b14-1ca1ee9b69ed}

**same content, other metadata 2**: `root` x1, `version` x1
- = `root` creation 2026-09-29T17:08:33 -> 2026-09-29T17:29:52, modified 2026-09-29T17:08:33 -> 2026-09-29T17:29:52, attributes 0 -> 0
- = `version` creation 2026-09-29T17:08:33 -> 2026-09-29T17:29:52, modified 2026-09-29T17:08:33 -> 2026-09-29T17:29:52, attributes 0 -> 0

### ConfigSave: 5 -> 0 rows (unchanged 0)

**deleted 5**: `<guid>` x1, `<guid>.<n>` x1, `root` x1, `version` x1, `versions` x1
- - `16b3681c-426d-4d6f-9ffe-588a23974222` size 174 sha 2ca5a62b3681 deflate v8text  `{1, {12, {3, {1,0,16b3681c-426d-4d6f-9ffe-588a23974222},"РаботаСКлассификаторамиКлиентСервер", {1...`
- - `16b3681c-426d-4d6f-9ffe-588a23974222.0` size 923 sha 341adcc9499c deflate container
- - `root` size 45 sha adfe2e91d2ef deflate v8text  `{2,66193438-abc5-410b-a1f1-a204102d1a62,}`
- - `version` size 28 sha ec6d68d22cdf deflate v8text  `{ {216,0, {80324,0} } }`
- - `versions` size 344220 sha 70a2929b1de0 deflate v8text

### Params: 38 -> 38 rows (unchanged 37)

**updated 1**: `DynamicallyUpdated` x1
- only the compression differs (same decoded bytes): 0
- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): 0
- content changed: 1
- content not kept, cannot tell: 0
- ~ `DynamicallyUpdated` size 82 -> 119 sha f05f5efb4431 -> d929c5543c4d; modified 2026-09-29T17:08:36 -> 2026-09-29T17:32:07
    value: {0,2,848a0a59-8803-445f-b89d-3cfae4f98bbd,4832dd20-0b9e-45e3-a3e7-095525bf9b3f}  ->  {0,3,848a0a59-8803-445f-b89d-3cfae4f98bbd,4832dd20-0b9e-45e3-a3e7-095525bf9b3f,96eee589-ca6c-4269-8b14-1ca1ee9b69ed}

### _CommonSettings: 74 -> 74 rows (unchanged 73)

- + `row:314397a05d79d850` _UserId=Администратор; _ObjectKey=ТекущиеДела; _SettingsKey=НастройкиОтображения; _Version=93CACD6434F1D8AB4F77093C85427108; _SettingsPresentation=; _SettingsData=len:2607 sha:ceefa8b7f332 blob; _ChangeDate=4026-09-29T17:32:51.0000000; _...
- - `row:db1f5f86fc3b21dd` _UserId=Администратор; _ObjectKey=ТекущиеДела; _SettingsKey=НастройкиОтображения; _Version=BA48069A0C840F3E49B17E8F8BD1B75F; _SettingsPresentation=; _SettingsData=len:2607 sha:e5d1f79f27ea blob; _ChangeDate=4026-09-29T17:29:42.0000000; _...

## Data tables and counts

unchanged tables: 2230; changed: 4; created: 0; dropped: 0

| table | class | rows before | rows after | checksum |
|---|---|---|---|---|
| dbo.Config | service | 9845 | 9848 | same |
| dbo.ConfigSave | service | 5 | 0 | same |
| dbo._InfoRg6434 | data | 4564 | 4574 | same |
| dbo._InfoRg6498 | data | 37044 | 37046 | same |

