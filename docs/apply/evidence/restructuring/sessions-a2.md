# Sessions on the twins of case a2 (checkpoint 2)

A restructured infobase is only right if a 1C session works with it, so every twin was opened by a real client.

## How

- **Stand-alone server** (`scripts/restructure-lab/srv.ps1`): `ibsrv` of 8.3.27.2214 under the agent's Windows account on one lab
  database (`--dbms=MSSQLServer --db-name=<db> --http-port=<port>`), and the thin client `1cv8c ENTERPRISE /WS http://localhost:<port>/`.
  The cluster service of this machine runs as LocalSystem and has no SQL login, so it could not be used until the
  coordinator registered lab databases in the local cluster (`F:\ibcmd\lab\04\tools\register-ib.ps1`).
- **Real cluster session** (from 15:33): the twin is registered with `register-ib.ps1 register -Database <db> -Platform 8.3.27 -Track ddl`,
  the client connects with `/IBConnectionString Srvr="localhost:2541";Ref="<db>";`, and `register-ib.ps1 unregister` removes the
  registration (the database stays). The cluster connects with the SQL login; scheduled jobs are denied.
- **The client is one generic processing** (`probe/ddl_probe.epf`, built with the Designer from `probe/src`): its form reads
  `<startup parameter>.bsl`, runs the text on the server with `Выполнить()` (the job sets the variable `Результат`) and writes
  `<startup parameter>.out`. `scripts/restructure-lab/session_job.ps1 -Job <file.bsl> [-Port n | -Database <db>]` starts the client,
  waits for the `.out` file and stops the client. Nothing but the jobs below was run.

## The final run: the twin ibcmd_rs_04_ddl_a2_fin (our apply, XDTO row written by the prototype), cluster session

Job `final_a2.bsl` (one session; the result below is its output, run twice: the first run wrote one item, the alias `В` of the query in
step 3 is a reserved word and stopped the first run, so the job was corrected and run again; the item of the first run is counted in "rows=15"):

```
Реквизит = Метаданные.Справочники._ДемоПартнеры.Реквизиты.Найти("ДемоНовыйРеквизит");
1. read before any write: the new attribute of every item (a query over Справочник._ДемоПартнеры)
2. XDTO: СериализаторXDTO.ЗаписатьXML(ЗаписьXML, <an existing item's object>), look for the attribute in the XML text
3. write a new item with ДемоНовыйРеквизит = "новое значение"; query by the attribute (parameter); change an existing item and write it
4. XDTO of the written object: the value is in the XML text
```

```
metadata attribute: Да, type: Строка
1. rows=15
   items with the empty default: 13, NULL (folders): 1, other values: 1
2. xdto: length 1 195, contains the new attribute: Да
3. written: [новое значение] Тест ДДЛ final 155043
   query by the new attribute: Тест ДДЛ final 154950 = [новое значение]
   query by the new attribute: Тест ДДЛ final 155043 = [новое значение]
   changed existing: АКБ Инспецбанк -> [изменено]
4. xdto of the new item: contains the attribute value: Да
```

The 14 items that existed before have the empty string (13 items) and NULL (the one folder), as on the native twin; the new attribute is
written, found by a query with a parameter and changed on an existing item; the XDTO serialization knows the property, which needs the model
cache row `Params ea13a2c9-....si` to contain it -- the row the prototype wrote.

After the two sessions (snapshot fin_after -> fin_session, `out/diff_fin_session.txt`): `DBSchema` and `SchemaStorage` are unchanged, **no
`*.si` row and no `DBNames` row was rewritten by the platform** (it accepted the rows as they were); the only `Params` change is that
`ecsreg_FZK78m2MVVNEneTkF26Pk9zLToEg` (306 bytes, a session-state row of the source database) was removed by the first server start, as it was
on the native twin; the rest is the normal traffic of a session (settings, information registers).

## The native twin (stand-alone server on ibcmd_rs_04_ddl_a2_nat, 12:40)

Job `read_a2.bsl`: `ВЫБРАТЬ Наименование, ДемоНовыйРеквизит ИЗ Справочник._ДемоПартнеры`:

```
rows=14
ООО "Альфа"|[]     Закрытое акционерное общество "Дальстрой"|[]   Торговый дом "Комплексный"|[]   ... (14 rows, the attribute is empty in all)
```

Job `xml_a2.bsl` (metadata + XDTO of an item): `metadata attribute: Да`, `type: Строка`, `xml length: 1 195, contains the new attribute: Да`
(the item's XML has `<ДемоНовыйРеквизит/>` between `<ВидПартнера>` and `<ДополнительныеРеквизиты>`).

## The stale XDTO cache (twin ibcmd_rs_04_ddl_a2_own: the first prototype run, before the XDTO row was written; cluster session, 16:00)

Job `xml_a2.bsl` on a twin whose `Params ea13a2c9-....si` row (the XDTO model) was left as it was (`--skip-xdto` does the same):

```
SERVER ERROR: {<Неизвестный модуль>(12)}: Ошибка при вызове метода контекста (ЗаписатьXML): Ошибка преобразования данных XDTO:
Запись значения свойства:
	форма: Элемент
	имя: {http://v8.1c.ru/8.1/data/enterprise/current-config}CatalogObject._ДемоПартнеры: Неизвестное свойство:
Свойство 'ДемоНовыйРеквизит' не обнаружено
```

The metadata, queries and writes of that twin worked (an item written there with `ДемоНовыйРеквизит = "новое значение"` on a stand-alone server at
13:56 was read back by a query in the cluster session of 15:35: `Тест ДДЛ 135612|[новое значение]`, the other 14 items `[]`); only XDTO
serialization of the changed object failed. On stand-alone servers
the same job succeeded after the row had been **deleted** (the platform rebuilds the model in memory and does not write the row back), and a
database with **all** `*.si` rows deleted did not start (`ibsrv` exits at once, exit 1, no message).

## The ALTER TABLE experiment (twin ibcmd_rs_04_ddl_a2_own2, stand-alone server, 15:22)

Job `alter_check.bsl` after `mssql-restructure --alter-add` added a second attribute (`ДемоВторойРеквизит`, String(20)) with
`ALTER TABLE dbo._Reference20 ADD _Fld11035 nvarchar(20) NULL` (the column is last in the physical table, after `_Fld2683`):

```
metadata: second attribute Да
items: 14
written: [первый] [второй]
query by the new column: ДДЛ ALTER 152226 [первый] [второй]
changed existing: АКБ Инспецбанк -> [изменено]
xdto: contains the second attribute Да
```

(Ran after the model cache row had been deleted so that the platform rebuilt it; see `alter-experiment.md`.)
