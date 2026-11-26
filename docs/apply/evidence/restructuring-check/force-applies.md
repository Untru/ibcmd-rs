# Native `infobase config apply --dynamic=force` runs of the rcheck lab (database `a`)

One line per apply: the change staged, the exit code, the seconds, what changed in the database, the objects the
platform named. `sql-objids` alone is noise (`_ConfigChngR*` is rebuilt at every apply).

* `e02_module` (prefix): append a comment line to CommonModule._ДемоЗаметки
  * exit 0 in 48.3 s; changed: Config, Params
* `e05_synonym` (prefix): synonym of Catalog._ДемоКассы
  * exit 0 in 72.5 s; changed: Config, Params, sql-objids
* `e08_attr_add` (basefree): a new attribute (String 20) in Catalog._ДемоКассы
  * exit 0 in 123.5 s; changed: Config, Params, DBSchema, SchemaStorage, sql-columns, sql-objids
  * `Объект изменен`: Справочник._ДемоКассы
* `e05b_synonym` (prefix): synonym of Catalog._ДемоКассыККМ: Демо: Кассы ККМ
  * exit 0 in 18.8 s; changed: Config, Params, sql-objids
* `p1_catalog_props` (basefree): 18 catalogs, one property each: Autonumbering(БанковскиеСчета) Explanation(ВидыНоменклатуры) ObjectPresentation(ГруппыДоступаНоменклатуры) ListPresentation(ГруппыДоступаПартнеров) DescriptionLength(ДоговорыКонтрагентов) DefaultPresentation(КассыККМ) QuickChoice(КлючиАналитики) ChoiceMode(КонтактныеЛица) CodeLength(Контрагенты) IncludeHelpInContents(МестаХранения) CheckUnique(Номенклатура) DataHist
  * exit 0 in 53.0 s; changed: Config, Params, DBSchema, SchemaStorage, sql-columns, sql-objids
  * `Объект изменен`: Справочник._ДемоДоговорыКонтрагентов, Справочник._ДемоКонтрагенты, Справочник._ДемоОрганизации
* `p2_attribute_props` (basefree): 16 catalogs, one attribute property each: Synonym(БанковскиеСчета) ToolTip(ВидыНоменклатуры) FillChecking(ГруппыДоступаНоменклатуры) QuickChoice(ГруппыДоступаПартнеров) Indexing(ДоговорыКонтрагентов) FullTextSearch(КассыККМ) ChoiceHistoryOnInput(КлючиАналитики) CreateOnInput(КонтактныеЛица) MultiLine(Контрагенты) PasswordMode(Номенклатура) ExtendedEdit(Организации) MarkNegatives(Партнеры) DataHist
  * exit 0 in 49.4 s; changed: Config, Params, DBSchema, SchemaStorage, sql-columns, sql-indexes, sql-objids
  * `Объект изменен`: Справочник._ДемоДоговорыКонтрагентов, Справочник._ДемоФизическиеЛица
* `probe_auto_a` (basefree): 168 single-property probes (auto_a)
  * exit 1 in 17.5 s; changed: -
  * errors: [ERROR] ПланОбмена.ОбменСообщениями: Недопустимое значение поля ВключатьРасширенияКонфигурации | [ERROR] ОбщийРеквизит.КомментарийЯзык2: Не указан параметр сеанса, задающий значение разделителя | [ERROR] HTTPСервис.Биллинг: Корневой URL совпадает с корневым URL сервиса exchange_dsl_1_0_0_1
* `probe_auto_a` (basefree): 160 single-property probes (auto_a)
  * exit 102 in 127.7 s; changed: -
  * errors: [ERROR] Ошибка проверки модели XDTO: xdto-package-3.3	пакет: http://v8.1c.ru/edi/edi_stnd/_DemoEnterpriseDataExt/1.2 Импортируемый пакет типов 'http://v8.1c.ru/
* `probe_auto_a` (basefree): 159 single-property probes (auto_a)
  * exit 0 in 152.7 s; changed: Config, Params, DBSchema, SchemaStorage, sql-columns, sql-indexes, sql-objids, sql-tables, sql-rowcounts
  * `Объект изменен`: Справочник._ДемоКонтрагенты, Справочник.ОбработчикиОчередиЗаданий, Справочник.ТелефонныйЗвонокПрисоединенныеФайлы, Справочник.ПрограммыЭлектроннойПодписиИШифрования, Справочник._ДемоМестаХранения, Справочник.Валюты, Документ.СогласиеНаОбработкуПерсональныхДанных, Документ.ЭлектронноеПисьмоИсходящее, Документ.СообщениеSMS, Документ._ДемоСчетФактураПолученный, ЖурналДокументов._ДемоЖурналВсехДокументов, РегламентноеЗадание.ОчисткаУстаревшихВерсийОбъектов, РегламентноеЗадание.ОбработкаОповещенийПользователей, РегламентноеЗадание.СборИОтправкаОтчетовОбОшибках, РегламентноеЗадание.УдалитьОбработкаОчередиЗаданийБТС, РегистрСведений.КонтурСинхронизации, РегистрНакопления._ДемоОборотыПоСчетамНаОплату, ПланВидовХарактеристик._ДемоВидыСубконто, ПланВидовХарактеристик.ВопросыДляАнкетирования, ПланВидовХарактеристик.РазделыДатЗапретаИзменения
  * errors: [INFO] Объект изменен: РегламентноеЗадание.СборИОтправкаОтчетовОбОшибках
* `p4_children` (basefree): p4: Attribute ГруппаДоступа ChoiceFoldersAndItems: Items -> FoldersAndItems; Attribute ГруппаДоступа Use: ForItem -> ForFolderAndItem; Attribute Банк Indexing: Index -> DontIndex; StandardAttribute Description FillChecking: ShowError -> DontCheck; StandardAttribute Code MultiLine: false -> true; StandardAttribute Description FullTextSearch: Use -> DontUse; StandardAttribute Code DataHistory: Use -
  * exit 0 in 128.2 s; changed: Config, Params, DBSchema, SchemaStorage, sql-columns, sql-indexes, sql-objids, sql-rowcounts
  * `Объект изменен`: Справочник._ДемоБанковскиеСчета, Перечисление._ДемоПолФизическогоЛица, Справочник._ДемоГруппыДоступаПартнеров
* `p5_children` (basefree): p5: DataProcessor _ДемоДлительнаяОперация: attribute added; Report _ДемоФайлы: attribute added; Document _ДемоЗаказПокупателя: TabularSection СчетаНаОплату FillChecking: DontCheck -> ShowError; Document _ДемоПеремещениеТоваров: TabularSection Товары attribute MultiLine: false -> true; Document _ДемоПоступлениеТоваров: TabularSection Товары: attribute РеквизитRcheck added; Documents _ДемоОприходова
  * exit 0 in 81.8 s; changed: Config, Params, DBSchema, SchemaStorage, sql-columns, sql-indexes, sql-objids, sql-tables, sql-rowcounts
  * `Объект изменен`: Справочник._ДемоКассы, РегистрНакопления._ДемоОстаткиТоваровВМестахХранения, Документ._ДемоПоступлениеТоваров, Документ._ДемоОприходованиеТоваров
* `p6_config` (basefree): Configuration: Version, UpdateCatalogAddress, Copyright, BriefInformation
  * exit 0 in 189.6 s; changed: Config, Params
* `p6b_kinds` (basefree): p6b: Configuration: Vendor DetailedInformation VendorInformationAddress ConfigurationInformationAddress; ScheduledJob ЗагрузкаКурсовВалют: MethodName: CommonModule.РаботаСКурсамиВалютЛокализация.ПриЗагрузкеАктуальныхКурсов -> CommonModule.УправлениеДоступомСлужебный.ЗаполнениеДанныхДляОграниченияДоступаОбработчикЗадания; ScheduledJob ЗаполнениеПараметровРаботыРасширений: RestartCountOnFailure: 3 -
  * exit 0 in 112.1 s; changed: Config, Params, DBSchema, SchemaStorage, sql-objids
  * `Объект изменен`: РегламентноеЗадание.ЗагрузкаКурсовВалют, РегламентноеЗадание.ЗаполнениеПараметровРаботыРасширений, РегламентноеЗадание.ЗапросЛицензийУникальныхУслуг
* `p7a_objects_add` (basefree): p7a: CommonModule МодульRcheck added; Catalog СправочникRcheck added; Role РольRcheck added (a copy of _ДемоБазовыеПраваБСП)
  * exit 0 in 117.3 s; changed: Config, Params, DBSchema, SchemaStorage, sql-columns, sql-indexes, sql-objids, sql-tables, sql-rowcounts
* `p7b_objects_remove` (basefree): p7b: CommonModule МодульRcheck removed; Catalog СправочникRcheck removed; Role РольRcheck removed
  * exit 0 in 128.9 s; changed: Config, Params, DBSchema, SchemaStorage, sql-columns, sql-indexes, sql-objids, sql-tables, sql-rowcounts
  * `Объект изменен`: Справочник.ВидыПроверок, Справочник.ИдентификаторыОбъектовРасширений, РегистрСведений.БезопасноеХранилищеДанных, Справочник.НастройкиТранспортаСообщенийОбмена, РегистрСведений.ВерсииОбъектов, РегистрСведений.ДанныеОбработанныеВЦентральномУзлеРИБ, Справочник.ИдентификаторыОбъектовМетаданных, РегистрСведений.БезопасноеХранилищеДанныхОбластейДанных, РегистрСведений.ДанныеОбъектовДляРегистрацииВОбменах, РегистрСведений.КлючиДоступаКРегиструРезультатыПроверкиУчета, РегистрСведений.ЗависимостиПравДоступа, РегистрСведений.НаборыЗначенийДоступа, РегистрСведений.НастройкиОчисткиФайлов, РегистрСведений.ДополнительныеСведения, РегистрСведений.ОбъектыНезарегистрированныеПриЗацикливании, РегистрСведений.НеудаленныеОбъекты, РегистрСведений.ПубличныеИдентификаторыСинхронизируемыхОбъектов, РегистрСведений.РезультатыОбменаДанными, РегистрСведений.РезультатыПроверкиУчета, РегистрСведений.СервисМобильнойПодписиСтатусы, РегистрСведений.СоответствияОбъектовИнформационныхБаз, РегистрСведений.СтатусыСинхронизацииФайловСОблачнымСервисом, РегистрСведений.НастройкиСинхронизацииФайлов, РегистрСведений.ТаблицыГруппДоступа, РегистрСведений.УдаляемыеОбъекты, РегистрСведений.УдалитьБезопасноеХранилищеДанныхОбластейДанных, Задача.ЗадачаИсполнителя, ПланВидовХарактеристик.ДополнительныеРеквизитыИСведения, РегистрСведений.УдалитьРезультатыОбменаДанными
  * log: Объект удален: Справочник.СправочникRcheck
* `p8_catalog_presentation` (basefree): p8: ExtendedObjectPresentation(ВидыНоменклатуры), ExtendedListPresentation(ГруппыДоступаПартнеров), FullTextSearchOnInputByString(КлючиАналитики), InputByString of _ДемоМестаХранения: + Code, AuxiliaryObjectForm(ГруппыДоступаНоменклатуры), AuxiliaryListForm(Организации), AuxiliaryChoiceForm(Партнеры), AuxiliaryFolderForm(Контрагенты), AuxiliaryFolderChoiceForm(Номенклатура)
  * exit 0 in 824.4 s; changed: Config, Params, sql-rowcounts
