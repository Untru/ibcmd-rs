# Demonstrations behind the findings of section 8

## Patch-mode import drops an added attribute (`runs_p1/v14_attribute_added`, pristine БСП clone `p1`)

* stage: `infobase config import` (patch mode) of the tree with an attribute added to `Catalog._ДемоКассы`: 9517 rows
* ConfigSave check: `needs_restructuring=False`, 0 descriptors compared
* tree against database (`--tree`): `needs_restructuring=True`: Catalog._ДемоКассы ChildObjects/Attribute[РеквизитRcheck] added (a column is added or dropped)
* native apply: exit 0, changed in the database: ['Config', 'Params', 'sql-rowcounts'], objects named: []

## Patch-mode import on a dynamically updated database stages the plain rows (`runs_demo/v00_control`, database `a`)

* stage: patch mode, the UNCHANGED tree of the database: 9516 rows
* ConfigSave check: `needs_restructuring=True`, 40 reasons, 80 objects, 149 descriptors compared

  * structure CommonAttribute.НаименованиеЯзык1 Properties/ExtendedEdit: true -> false (a property no rule covers)
  * data ScheduledJob.ЗапросЛицензийУникальныхУслуг Properties/RestartIntervalOnFailure: 601 -> 10 (the platform keeps the scheduled jobs in a table of its own)
  * structure InformationRegister.КэшПереводов Properties/EnableTotalsSliceLast: true -> false (a property no rule covers)
  * structure ChartOfCharacteristicTypes.РазделыДатЗапретаИзменения Properties/Hierarchical: true -> false (a property no rule covers)
  * structure CommonAttribute.ОтредактированныеПредопределенныеРеквизиты Properties/MarkNegatives: true -> false (a property no rule covers)
  * structure Catalog.Валюты Properties/DataHistory: Use -> DontUse (a property no rule covers)
  * structure Catalog._ДемоКонтрагенты Properties/CodeLength: 12 -> 9 (a property no rule covers)
  * structure Catalog._ДемоКонтрагенты Properties/CodeType: Number -> String (a property no rule covers)
  * structure Catalog._ДемоКонтрагенты Properties/StandardAttributes/StandardAttribute[Code]/DataHistory: DontUse -> Use (a standard attribute's property no rule covers)
  * structure Document._ДемоСчетФактураПолученный Properties/Posting: Deny -> Allow (a property no rule covers)
  * structure Catalog.ОбработчикиОчередиЗаданий Properties/Hierarchical: true -> false (a property no rule covers)
  * structure CommonAttribute.НаименованиеЯзык2 Properties/PasswordMode: true -> false (a property no rule covers)

## The ConfigSave of a native import (`selftest/nat_a2_check3.json`, ddl case a2 restored as `nat_a2`)

* staged rows 9839, descriptors compared 517, notes 125, reasons 400 (unknown: 399)
* the change of the case: Catalog._ДемоПартнеры ChildObjects/Attribute[ДемоНовыйРеквизит] added (a column is added or dropped)
* the first reason: the descriptors could not all be read: the Configuration row: Configuration fields 26 and 43 hold 80324 and 80327; no corpus shows which one the platform prints

## The base-free stage of an unchanged 8.5 БСП (`runs_85/v04_synonym`, first apply of the clone)

* structure Configuration : the root row differs: another configuration
* unknown Task.ЗадачаИсполнителя : the Task row differs (32311 -> 32311 bytes) but both sides decode to the same XML
* unknown Configuration : the Configuration row differs (139204 -> 139197 bytes) but both sides decode to the same XML
* unknown Configuration row .81401d17-a303-42f0-9b12-003e1d64672c: content changed (99664295 -> 99663207 bytes)
* data BusinessProcess.Задание Flowchart: content changed (8805 -> 8805 bytes)
* native apply: exit 0, objects named: ['БизнесПроцесс.Задание']

