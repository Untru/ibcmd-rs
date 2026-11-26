# Snapshot diff

- before: `F:\ibcmd\lab\05\online\runs\live\cap\20260929-205255-live-v1\before` (ibcmd_rs_05_online_b1, 2026-09-29T20:55:24.6741203+03:00)
- after: `F:\ibcmd\lab\05\online\runs\live\cap\20260929-205255-live-v1\after` (ibcmd_rs_05_online_b1, 2026-09-29T20:58:56.8871211+03:00)
- date columns are stored with a year offset of 2000: 4026-09-29 is 2026-09-29

## Schema

tables: 2234 -> 2234; created 0, dropped 0, altered 0

no schema changes

## Service tables

### Config: 9844 -> 9843 rows (unchanged 9838)

**deleted 1**: `DynamicallyUpdated` x1
- - `DynamicallyUpdated` size 45 sha 350612c7a9a9 raw v8text  `{1,1,06cb0442-0c47-4fad-986a-f08f28287c1b}`

**updated 3**: `<guid>` x1, `<guid>.<n>` x1, `versions` x1
- only the compression differs (same decoded bytes): 1
- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): 0
- content changed: 2
- content not kept, cannot tell: 0
  - recompressed: `<guid>` x1; first: `313d9858-3995-4a4c-b2b0-15d2350417b4`
- ~ `313d9858-3995-4a4c-b2b0-15d2350417b4.0` size 680 -> 701 sha acc5da75d0c8 -> 4c0e9b9da0a9; modified 4026-08-22T09:21:11 -> 2026-09-29T17:52:20
    container element `text` changed: 1339 -> 1344 bytes
        @@ -12,5 +12,5 @@
         
         	ТипыИнтеграций = Новый Структура;
        -	ТипыИнтеграций.Вставить("Telegram", "Telegram");
        +	ТипыИнтеграций.Вставить("Telegram", "ibcmd-live-v1");
         	ТипыИнтеграций.Вставить("ВКонтакте", "VK");
         	ТипыИнтеграций.Вставить("WhatsApp", "WhatsApp Devino");
- ~ `versions` size 340602 -> 344211 sha 1e15f7bfa97d -> 2e6cc0b877a4; modified 4026-09-19T23:00:46 -> 2026-09-29T17:52:20
    header 1 9839 "" 848a0a59-8803-445f-b89d-3cfae4f98bbd -> 1 9839 "" 9ede6f06-d28e-4bee-ba93-a0c54a4873f3
    entries: 9838 -> 9838; changed 5, added 0, removed 0
        ~ 313d9858-3995-4a4c-b2b0-15d2350417b4: 8157c25b-98e9-40f4-9b7e-c69909ac86c3 -> f2805d10-7452-46da-92f8-2a1c92fa629c
        ~ 313d9858-3995-4a4c-b2b0-15d2350417b4.0: 151cc9e7-c526-4df2-ba1c-996280ae6451 -> 11b2583e-904c-4830-a01d-b715e0245965
        ~ root: 9a3159b0-2b3f-436f-bdc0-b6e01272b560 -> e2ee2b19-c62d-472c-a023-a7d83447defa
        ~ version: 30424fa4-6704-4ce4-807c-b62386988f97 -> 709b0408-c23d-4021-a645-dcaba04a3bb4
        ~ versions: ee353752-4462-450c-9b1c-fef4a89ea3f4 -> 1294913a-4c6d-46d1-9e96-d940e90b5552

**same content, other metadata 2**: `root` x1, `version` x1
- = `root` creation 4026-09-19T23:32:53 -> 2026-09-29T17:52:20, modified 4026-09-19T23:32:53 -> 2026-09-29T17:52:20, attributes 0 -> 0
- = `version` creation 4026-09-19T23:32:53 -> 2026-09-29T17:52:20, modified 4026-09-19T23:32:53 -> 2026-09-29T17:52:20, attributes 0 -> 0

### ConfigSave: 5 -> 0 rows (unchanged 0)

**deleted 5**: `<guid>` x1, `<guid>.<n>` x1, `root` x1, `version` x1, `versions` x1
- - `313d9858-3995-4a4c-b2b0-15d2350417b4` size 165 sha 68173761147f deflate v8text  `{1, {12, {3, {1,0,313d9858-3995-4a4c-b2b0-15d2350417b4},"ОбсужденияСлужебныйКлиентСервер", {1,"ru...`
- - `313d9858-3995-4a4c-b2b0-15d2350417b4.0` size 701 sha 4c0e9b9da0a9 deflate container
- - `root` size 45 sha adfe2e91d2ef deflate v8text  `{2,66193438-abc5-410b-a1f1-a204102d1a62,}`
- - `version` size 28 sha ec6d68d22cdf deflate v8text  `{ {216,0, {80324,0} } }`
- - `versions` size 344211 sha 2e6cc0b877a4 deflate v8text

### Params: 38 -> 37 rows (unchanged 37)

**deleted 1**: `DynamicallyUpdated` x1
- - `DynamicallyUpdated` size 82 sha 2d0954f535c6 raw v8text  `{0,2,848a0a59-8803-445f-b89d-3cfae4f98bbd,06cb0442-0c47-4fad-986a-f08f28287c1b}`

### _CommonSettings: 74 -> 74 rows (unchanged 73)

- + `row:9410018f4f17d16b` _UserId=Администратор; _ObjectKey=ТекущиеДела; _SettingsKey=НастройкиОтображения; _Version=8774EF4AF9308433414778CC6A45D31F; _SettingsPresentation=; _SettingsData=len:2607 sha:4e9f722afa57 blob; _ChangeDate=4026-09-29T17:56:52.0000000; _...
- - `row:bb9ce73e23b6461f` _UserId=Администратор; _ObjectKey=ТекущиеДела; _SettingsKey=НастройкиОтображения; _Version=AAFE191780A127CA4A361269120355FE; _SettingsPresentation=; _SettingsData=len:2607 sha:25ebd52bc74a blob; _ChangeDate=4026-09-29T17:44:41.0000000; _...

## Data tables and counts

unchanged tables: 2230; changed: 4; created: 0; dropped: 0

| table | class | rows before | rows after | checksum |
|---|---|---|---|---|
| dbo.Config | service | 9847 | 9846 | same |
| dbo.ConfigSave | service | 5 | 0 | same |
| dbo.Params | service | 38 | 37 | same |
| dbo._UsersWorkHistory | service | 714 | 717 | same |

