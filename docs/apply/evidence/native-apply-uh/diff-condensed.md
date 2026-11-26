# Snapshot diff

- before: `F:\ibcmd\lab\04\trace\captures\20260929-171426-uh-case1-forms-exclusive\before` (ibcmd_rs_04_trace_uha_c1, 2026-09-29T17:24:39.8051016+03:00)
- after: `F:\ibcmd\lab\04\trace\captures\20260929-171426-uh-case1-forms-exclusive\after` (ibcmd_rs_04_trace_uha_c1, 2026-09-29T17:58:58.4582922+03:00)
- date columns are stored with a year offset of 2000: 4026-09-29 is 2026-09-29

## Schema

tables: 21187 -> 21187; created 0, dropped 0, altered 0

no schema changes

## Service tables

### Config: 118326 -> 118326 rows (unchanged 118303)

**updated 11**: `<guid>.<n>` x10, `versions` x1
- only the compression differs (same decoded bytes): 0
- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): 0
- content changed: 10
- content not kept, cannot tell: 1
  - unknown: `versions` x1; first: `versions`

Updated rows of Config (the module texts are in the capture, `diff\diff.md`; each differs by the appended comment line):

- `102f78a6-ed48-47f3-b6f2-b1aaaf213ab8.0` stored 511 -> 465 B
- `16d44ab1-4bce-4df8-a215-6ed5173c75a8.0` stored 7974 -> 7984 B
- `323997ca-488d-4d84-aa44-da60b6b4d414.0` stored 2090 -> 2098 B
- `782cb93d-36e6-4923-b2b6-75caa3459cd6.0` stored 1740 -> 1790 B
- `8778c153-7e92-4932-9e60-42d8b2577c04.0` stored 496 -> 521 B
- `8bd9f000-83c9-4a48-8166-2691b6f02183.2` stored 35901 -> 36277 B
- `b27aebc8-f190-4658-a81d-fd1406905f39.0` stored 643 -> 494 B
- `ddb9d033-8b6f-474b-b637-733b5fdcbced.0` stored 409 -> 458 B
- `e5dca369-dc9a-4332-a509-f734b8787e13.0` stored 15633 -> 15694 B
- `f7db3f70-efbb-457f-80d9-04cfd6b306f9.0` stored 500 -> 519 B

**same content, other metadata 12**: `<guid>` x10, `root` x1, `version` x1
- = `102f78a6-ed48-47f3-b6f2-b1aaaf213ab8` creation 4026-06-18T12:04:39 -> 4026-09-29T17:14:06, modified 4026-06-18T12:04:39 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `16d44ab1-4bce-4df8-a215-6ed5173c75a8` creation 4026-06-18T12:04:47 -> 4026-09-29T17:14:06, modified 4026-06-18T12:04:47 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `323997ca-488d-4d84-aa44-da60b6b4d414` creation 4026-06-18T12:05:05 -> 4026-09-29T17:14:06, modified 4026-06-18T12:05:05 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `782cb93d-36e6-4923-b2b6-75caa3459cd6` creation 4026-06-18T12:06:25 -> 4026-09-29T17:14:06, modified 4026-06-18T12:06:25 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `8778c153-7e92-4932-9e60-42d8b2577c04` creation 4026-06-18T12:06:34 -> 4026-09-29T17:14:06, modified 4026-06-18T12:06:34 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `8bd9f000-83c9-4a48-8166-2691b6f02183` creation 4026-06-18T12:06:37 -> 4026-09-29T17:14:06, modified 4026-06-18T12:06:37 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `b27aebc8-f190-4658-a81d-fd1406905f39` creation 4026-06-18T12:06:56 -> 4026-09-29T17:14:06, modified 4026-06-18T12:06:56 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `ddb9d033-8b6f-474b-b637-733b5fdcbced` creation 4026-06-18T12:07:18 -> 4026-09-29T17:14:06, modified 4026-06-18T12:07:18 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `e5dca369-dc9a-4332-a509-f734b8787e13` creation 4026-06-18T12:07:21 -> 4026-09-29T17:14:06, modified 4026-06-18T12:07:21 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `f7db3f70-efbb-457f-80d9-04cfd6b306f9` creation 4026-06-18T12:07:32 -> 4026-09-29T17:14:06, modified 4026-06-18T12:07:32 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `root` creation 4026-06-28T20:59:27 -> 4026-09-29T17:14:06, modified 4026-06-28T20:59:27 -> 4026-09-29T17:14:06, attributes 0 -> 0
- = `version` creation 4026-06-28T20:59:27 -> 4026-09-29T17:14:06, modified 4026-06-28T20:59:27 -> 4026-09-29T17:14:06, attributes 0 -> 0

### ConfigSave: 23 -> 0 rows (unchanged 0)

**deleted 23**: `<guid>` x10, `<guid>.<n>` x10, `root` x1, `version` x1, `versions` x1
- - `102f78a6-ed48-47f3-b6f2-b1aaaf213ab8` size 236 sha 380bbdb4831a deflate v8text  `{1, {12, {3, {1,0,102f78a6-ed48-47f3-b6f2-b1aaaf213ab8},"ЭлектроннаяПодписьВМоделиСервисаПереопре...`
- - `102f78a6-ed48-47f3-b6f2-b1aaaf213ab8.0` size 465 sha 4bbecc489db4 deflate container
- - `16d44ab1-4bce-4df8-a215-6ed5173c75a8` size 189 sha dabbc0ca9136 deflate v8text  `{1, {0, {13, {3, {1,0,16d44ab1-4bce-4df8-a215-6ed5173c75a8},"ФормаЭлемента", {2,"ru","Форма элеме...`
- - `16d44ab1-4bce-4df8-a215-6ed5173c75a8.0` size 7984 sha fbb29e79efd6 deflate v8text
- - `323997ca-488d-4d84-aa44-da60b6b4d414` size 255 sha d198648a5587 deflate v8text  `{1, {4, {13, {3, {1,0,323997ca-488d-4d84-aa44-da60b6b4d414},"ВопросОбУстановкеВнешнейКомпоненты",...`
- - `323997ca-488d-4d84-aa44-da60b6b4d414.0` size 2098 sha f5180fa61465 deflate v8text  `{4, {50,0,1,40,0,1,0,0,00000000-0000-0000-0000-000000000000,0, {1,2, {"ru","Установка внешней ком...`
- - `782cb93d-36e6-4923-b2b6-75caa3459cd6` size 5961 sha a8df8b01a230 deflate v8text
- - `782cb93d-36e6-4923-b2b6-75caa3459cd6.0` size 1790 sha 5d1a7a22c3ab deflate container
- - `8778c153-7e92-4932-9e60-42d8b2577c04` size 215 sha c24250ef8684 deflate v8text  `{1, {12, {3, {1,0,8778c153-7e92-4932-9e60-42d8b2577c04},"ОповещениеПользователейБТСПереопределяем...`
- - `8778c153-7e92-4932-9e60-42d8b2577c04.0` size 521 sha 1fa3e6147396 deflate container
- - `8bd9f000-83c9-4a48-8166-2691b6f02183` size 24557 sha 945445bd3a91 deflate v8text
- - `8bd9f000-83c9-4a48-8166-2691b6f02183.2` size 36277 sha 6e2b702d9ddb deflate container
- - `b27aebc8-f190-4658-a81d-fd1406905f39` size 213 sha d43aa09c6079 deflate v8text  `{1, {12, {3, {1,0,b27aebc8-f190-4658-a81d-fd1406905f39},"ОплатаСервисаКлиентПереопределяемый", {2...`
- - `b27aebc8-f190-4658-a81d-fd1406905f39.0` size 494 sha ea2815edbc4a deflate container
- - `ddb9d033-8b6f-474b-b637-733b5fdcbced` size 132 sha aebe36696c7b deflate v8text  `{1, {12, {3, {1,0,ddb9d033-8b6f-474b-b637-733b5fdcbced},"ТарификацияВызовСервера", {0},"",0,0,000...`
- - `ddb9d033-8b6f-474b-b637-733b5fdcbced.0` size 458 sha 2cf1724d09f6 deflate container
- - `e5dca369-dc9a-4332-a509-f734b8787e13` size 200 sha ff5a8a043a00 deflate v8text  `{1, {12, {3, {1,0,e5dca369-dc9a-4332-a509-f734b8787e13},"СтроковыеФункцииКлиентСервер", {2,"ru","...`
- - `e5dca369-dc9a-4332-a509-f734b8787e13.0` size 15694 sha 46c06dceb423 deflate container
- - `f7db3f70-efbb-457f-80d9-04cfd6b306f9` size 203 sha 8ec077615bcb deflate v8text  `{1, {12, {3, {1,0,f7db3f70-efbb-457f-80d9-04cfd6b306f9},"СообщенияВМоделиСервисаПовтИсп", {2,"ru"...`
- - `f7db3f70-efbb-457f-80d9-04cfd6b306f9.0` size 519 sha 3be05335a739 deflate container
- - `root` size 46 sha 6191b978f069 deflate v8text  `{2,498ce97d-6689-44e1-a350-7d98de25218c,}`
- - `version` size 28 sha 919d95528de7 deflate v8text  `{ {216,0, {80327,0} } }`
- - `versions` size 4016543 sha b16c3110d8d5 deflate v8text

### Files: 14 -> 14 rows (unchanged 13)

**updated 1**: `MobileVersions.dat` x1
- only the compression differs (same decoded bytes): 0
- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): 0
- content changed: 1
- content not kept, cannot tell: 0
- ~ `MobileVersions.dat` size 43 -> 80 sha 53f2e8740aeb -> fe03a3b59b70; modified 4026-06-28T21:02:43 -> 4026-09-29T17:29:22
    value: {1,09190309-7274-4bb6-b547-f6936a8aa00c}  ->  {2,f6d0a593-47fa-4657-8fa0-a947fe7ae14c,09190309-7274-4bb6-b547-f6936a8aa00c}

### Params: 23 -> 25 rows (unchanged 4)

**inserted 2**: `<guid>.ui` x2
- + `0c796029-75db-4e63-a8c4-948043f8bb0c.ui` size 94 (stored 94, 1 part) sha 17a1366335ce raw text  `WitSPDfx0q5WOMf5DAELWgn3HMrgGP7FNRQmkrl/gTLM4JGWQI6dBeR+izwuKFrV 2iqiSFtZYHHw8fCJAgQKXw==`
- + `41d98f83-fc5d-4f0b-9709-a32839d47a93.ui` size 24659 (stored 24659, 1 part) sha b0fb1f5f49b4 raw text

**updated 3**: `ibparams.inf` x1, `locale.inf` x1, `siVersions` x1
- only the compression differs (same decoded bytes): 0
- only formatting differs (same tokens / container elements; line breaks, base64 wrapping): 0
- content changed: 3
- content not kept, cannot tell: 0
- ~ `ibparams.inf` size 292 -> 324 sha aa8225168b85 -> b6409e2a6bfa; modified 4026-06-28T20:47:54 -> 4026-09-29T17:33:13
    value: {20,0,0,1,"",1200,86400,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000,-1,0,4,5,30,"", {0,"",3,8,3,30,"","","",1354290245127518426,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600},0,0,0,0,0,432000,0,0,2,0,0,0, {3,6,3,60,"","","",0,465,1,"","","","",600},0}  ->  {20,0,0,1,"",1200,86400,150607a8-e263-6d7b-9c1c-cc510adb2b81,58a6e802-6be5-6c13-a3d1-fbf970f7ea1e,-1,0,4,5,30,"", {0,"",3,8,3,30,"","","",1354290245127518426,465,1,"","","","",160,0,0,0,432000,1,0,0,"",1,1,600,0,2,"/AccessToken",""},0,0,0,0,0,432000,0,0,2,0,0,0, {3,6,3,60,"","","",0,465,1,"","","","",600,0,2,"",""} }
- ~ `locale.inf` size 112 -> 112 sha 5efc85f31f80 -> 9ef881afd44e; modified 4026-06-28T20:47:54 -> 4026-09-29T17:33:13
    value: {"ru_RU",0,0,"",-1,"","","","",1,0,00000000-0000-0000-0000-000000000000,00000000-0000-0000-0000-000000000000}  ->  {"ru_RU",0,0,"",-1,"","","","",1,0,150607a8-e263-6d7b-9c1c-cc510adb2b81,58a6e802-6be5-6c13-a3d1-fbf970f7ea1e}
- ~ `siVersions` size 1273 -> 1273 sha acadc3c3f2d7 -> 2d3de4ea573d; modified 4026-06-28T21:13:22 -> 4026-09-29T17:33:10
    @@ -3,34 +3,34 @@
       16
       "0b698dcd-501d-42d9-892d-5a9157bc996a.si"
    -  6b9da1d6-53e5-4a3b-b1d7-f07d7207da63
    +  aec47b98-48cc-4c9a-a4d1-d639014c5974
       "c4629235-4823-4320-b8b5-1d08f4c6d612.si"
    -  21e49f7d-91ae-4341-98ff-912bcab3825a
    +  7170df4a-306c-45dd-8e4b-8be2f7084a56
       "1a621f0f-5568-4183-bd9f-f6ef670e7090.si"
    -  d15321da-5289-45da-b633-bbf9f27ad49c
    +  c4dcb920-9566-4377-829d-e3147765f15d
       "215d232c-9c9e-4f7c-8a87-142cd3797264.si"
    -  d7930ab7-dfcb-469d-a2d0-56f50ef5f233
    +  06dd5bad-ca04-423b-83aa-d7f5b62f6429
       "2203278d-ef4f-4f68-98f1-feb257d53ecc.si"
    -  52a27e2c-bc7a-4764-b515-05299b8b6b5b
    +  6e8f18eb-8024-4cf0-be5d-ba35201e2d44
       "c40aafd6-c889-4229-807a-851d0bc5bc97.si"
    -  4aea77e2-95cd-44a5-bda3-b84ee3d78863
    +  fbe9632f-0c50-4c58-b30b-fded796f4ab9
       "42ed49cc-765d-4314-bc2d-af425af7bf13.si"
    -  4167d778-ef4c-4e4a-9b73-5e9ce406646d
    +  1a3214e1-b6a5-420a-9cbe-9fd6fbb814d1
       "59274b8d-4447-4bf4-9d29-bfa099a1de37.si"
    -  94efe78e-da29-4621-b584-37c5384eac76
    +  fd5d0e51-5c9b-45ac-8dcf-cf02fbd55bbe
       "a07b62f0-1f01-484a-93d9-d42764cedac0.si"
    -  f730fc19-212a-494b-beed-ff5178002f0c
    +  305ba55a-d431-462a-a630-ae9fb234aaff
       "c77bc206-5935-48ea-b32e-508a572d94f4.si"
    ... (21 more diff lines)

**same content, other metadata 16**: `<guid>.si` x16
- = `0b698dcd-501d-42d9-892d-5a9157bc996a.si` creation 4026-06-28T21:11:00 -> 4026-09-29T17:32:56, modified 4026-06-28T21:11:00 -> 4026-09-29T17:32:57, attributes 0 -> 0
- = `1a621f0f-5568-4183-bd9f-f6ef670e7090.si` creation 4026-06-28T21:11:04 -> 4026-09-29T17:33:04, modified 4026-06-28T21:11:05 -> 4026-09-29T17:33:06, attributes 0 -> 0
- = `215d232c-9c9e-4f7c-8a87-142cd3797264.si` creation 4026-06-28T21:11:00 -> 4026-09-29T17:32:55, modified 4026-06-28T21:11:00 -> 4026-09-29T17:32:55, attributes 0 -> 0
- = `2203278d-ef4f-4f68-98f1-feb257d53ecc.si` creation 4026-06-28T21:11:00 -> 4026-09-29T17:32:55, modified 4026-06-28T21:11:00 -> 4026-09-29T17:32:56, attributes 0 -> 0
- = `42ed49cc-765d-4314-bc2d-af425af7bf13.si` creation 4026-06-28T21:11:00 -> 4026-09-29T17:32:55, modified 4026-06-28T21:11:00 -> 4026-09-29T17:32:55, attributes 0 -> 0
- = `59274b8d-4447-4bf4-9d29-bfa099a1de37.si` creation 4026-06-28T21:11:00 -> 4026-09-29T17:32:55, modified 4026-06-28T21:11:00 -> 4026-09-29T17:32:55, attributes 0 -> 0
- = `a07b62f0-1f01-484a-93d9-d42764cedac0.si` creation 4026-06-28T21:11:00 -> 4026-09-29T17:32:57, modified 4026-06-28T21:11:01 -> 4026-09-29T17:32:58, attributes 0 -> 0
- = `c40aafd6-c889-4229-807a-851d0bc5bc97.si` creation 4026-06-28T21:11:05 -> 4026-09-29T17:33:06, modified 4026-06-28T21:11:06 -> 4026-09-29T17:33:06, attributes 0 -> 0
- = `c4629235-4823-4320-b8b5-1d08f4c6d612.si` creation 4026-06-28T21:11:00 -> 4026-09-29T17:32:57, modified 4026-06-28T21:11:00 -> 4026-09-29T17:32:57, attributes 0 -> 0
- = `c77bc206-5935-48ea-b32e-508a572d94f4.si` creation 4026-06-28T21:11:04 -> 4026-09-29T17:33:04, modified 4026-06-28T21:11:04 -> 4026-09-29T17:33:04, attributes 0 -> 0
- = `cf8b5e0f-5e46-4cf4-bc6f-204eae2c4e8a.si` creation 4026-06-28T21:11:04 -> 4026-09-29T17:33:04, modified 4026-06-28T21:11:04 -> 4026-09-29T17:33:04, attributes 0 -> 0
- = `e05c0074-0404-4b7a-835e-9cacd405960e.si` creation 4026-06-28T21:11:06 -> 4026-09-29T17:33:06, modified 4026-06-28T21:11:06 -> 4026-09-29T17:33:06, attributes 0 -> 0
- = `ea13a2c9-0c2f-40fa-b855-710387e3271d.si` creation 4026-06-28T21:11:01 -> 4026-09-29T17:32:59, modified 4026-06-28T21:11:04 -> 4026-09-29T17:33:04, attributes 0 -> 0
- = `facbfffe-feb2-4d30-8930-a557b185e5c4.si` creation 4026-06-28T21:11:06 -> 4026-09-29T17:33:06, modified 4026-06-28T21:11:06 -> 4026-09-29T17:33:06, attributes 0 -> 0
- = `fd1b2a86-b7df-4f32-84e2-befd4f3a2331.si` creation 4026-06-28T21:11:04 -> 4026-09-29T17:33:04, modified 4026-06-28T21:11:04 -> 4026-09-29T17:33:04, attributes 0 -> 0
- = `fe8acd6a-22c9-4b5a-aeae-232a1c8324cb.si` creation 4026-06-28T21:11:01 -> 4026-09-29T17:32:58, modified 4026-06-28T21:11:01 -> 4026-09-29T17:32:59, attributes 0 -> 0

### _ConfigChngR: 0 -> 0 rows (compared by node and object, not by the row id)

- rows only after: 0; rows only before: 0; `_MessageNo` changed: 0; row id `_IDRRef` differs in 0 of 0 common rows

### _DbCopiesInfoBaseUse: 1 -> 1 rows (unchanged 0)

- + `row:2fb9f65212f72867` _Id=A5CD3EA844A6F9AA42B37B47446D9425; _Description=DESKTOP-SMI5N4O : DefAlias;
- - `row:a90b2c7a3f234d81` _Id=81BF554AD509A7494A0CFC42214B3353; _Description=localhost:3541 : uha;

## Data tables and counts

unchanged tables: 21185; changed: 2; created: 0; dropped: 0

| table | class | rows before | rows after | checksum |
|---|---|---|---|---|
| dbo.ConfigSave | service | 23 | 0 | same |
| dbo.Params | service | 23 | 25 | same |

