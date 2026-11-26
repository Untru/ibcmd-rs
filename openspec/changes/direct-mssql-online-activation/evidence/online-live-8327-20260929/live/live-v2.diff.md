# Snapshot diff

- before: `F:\ibcmd\lab\05\online\runs\live\cap\20260929-210714-live-v2\before` (ibcmd_rs_05_online_b1, 2026-09-29T21:08:10.5030223+03:00)
- after: `F:\ibcmd\lab\05\online\runs\live\cap\20260929-210714-live-v2\after` (ibcmd_rs_05_online_b1, 2026-09-29T21:09:36.5153997+03:00)
- date columns are stored with a year offset of 2000: 4026-09-29 is 2026-09-29

## Schema

tables: 2234 -> 2234; created 0, dropped 0, altered 0

no schema changes

## Service tables

### Config: 9843 -> 9843 rows (unchanged 9838)

**updated 2**: `<guid>.<n>` x1, `versions` x1
- only the compression differs (same decoded bytes): 0
- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): 0
- content changed: 1
- content not kept, cannot tell: 1
  - unknown: `<guid>.<n>` x1; first: `313d9858-3995-4a4c-b2b0-15d2350417b4.0`
- ~ `versions` size 344211 -> 344213 sha 2e6cc0b877a4 -> c281e72b5767; modified 2026-09-29T17:52:20 -> 2026-09-29T18:08:17
    header 1 9839 "" 9ede6f06-d28e-4bee-ba93-a0c54a4873f3 -> 1 9839 "" d2c91459-9438-4113-979b-f13b7d9228d7
    entries: 9838 -> 9838; changed 5, added 0, removed 0
        ~ 313d9858-3995-4a4c-b2b0-15d2350417b4: f2805d10-7452-46da-92f8-2a1c92fa629c -> 2ad37bd8-98de-4c21-9051-11c56f17d989
        ~ 313d9858-3995-4a4c-b2b0-15d2350417b4.0: 11b2583e-904c-4830-a01d-b715e0245965 -> b7f9674a-53c9-48dc-9287-70a5b0537a67
        ~ root: e2ee2b19-c62d-472c-a023-a7d83447defa -> d85ebc70-ef48-49b4-9395-a4f1473ec5bc
        ~ version: 709b0408-c23d-4021-a645-dcaba04a3bb4 -> 323a2b6e-324d-4c76-830d-21346d402342
        ~ versions: 1294913a-4c6d-46d1-9e96-d940e90b5552 -> 662a48e2-e306-4003-8e74-e01115e6e5e5

**same content, other metadata 3**: `<guid>` x1, `root` x1, `version` x1
- = `313d9858-3995-4a4c-b2b0-15d2350417b4` creation 2026-09-29T17:52:20 -> 2026-09-29T18:08:17, modified 2026-09-29T17:52:20 -> 2026-09-29T18:08:17, attributes 0 -> 0
- = `root` creation 2026-09-29T17:52:20 -> 2026-09-29T18:08:17, modified 2026-09-29T17:52:20 -> 2026-09-29T18:08:17, attributes 0 -> 0
- = `version` creation 2026-09-29T17:52:20 -> 2026-09-29T18:08:17, modified 2026-09-29T17:52:20 -> 2026-09-29T18:08:17, attributes 0 -> 0

### _CommonSettings: 74 -> 74 rows (unchanged 73)

- + `row:81ad6a47bbed2ff8` _UserId=Администратор; _ObjectKey=ТекущиеДела; _SettingsKey=НастройкиОтображения; _Version=B63C51B3CBD930B549D223487672A3FD; _SettingsPresentation=; _SettingsData=len:2607 sha:37d839cedb22 blob; _ChangeDate=4026-09-29T18:08:38.0000000; _...
- - `row:0d8bcc08a89bb9b6` _UserId=Администратор; _ObjectKey=ТекущиеДела; _SettingsKey=НастройкиОтображения; _Version=B3B21E9D644C3D5B46352CB0035B3DDE; _SettingsPresentation=; _SettingsData=len:2607 sha:0bb79d2d7f47 blob; _ChangeDate=4026-09-29T18:06:29.0000000; _...

## Data tables and counts

unchanged tables: 2233; changed: 1; created: 0; dropped: 0

| table | class | rows before | rows after | checksum |
|---|---|---|---|---|
| dbo._InfoRg6498 | data | 36957 | 36959 | same |

