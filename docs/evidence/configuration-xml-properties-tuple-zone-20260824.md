# Configuration.xml: недостающий блок скалярных свойств — координаты для другой сессии

Статус на `b4048a3` (после `789b1ae`). Эта зона теперь ведёт отдельная
пользовательская сессия — координационное сообщение назвало её «кортеж
свойств Configuration.xml формы `{67,}`/`{76,}`». Записываю, что уже
известно, и пропускаю: `Configuration.xml` входит в остаток обоих корпусов
БСП.

## Где наблюдается

* `ssl` (БСП демо 3.1.12.297): `Configuration.xml` — 75 строк diff.
* `sslbase` (БСП базовая 3.1.12.297): `Configuration.xml` — 75 строк diff,
  байт-в-байт тот же список тегов, что и в `ssl` (оба Configuration.xml
  свойства идентичны между демо и базовой редакцией).

Больше нигде в остатке этих двух корпусов (не Configuration.xml -- ни разу
не всплывает в остальных 57+40 расходящихся файлах).

## Что именно недостаёт

Один смежный блок скалярных/пустых тегов внутри `<Properties>` целиком
отсутствует в нашем выводе (не переставлен, не испорчен — **не выдан**).
Три под-группы, в порядке появления у платформы:

Группа 1 (сразу после `<Comment/>`, перед `<UsePurposes>`):
```
<NamePrefix/>
<ConfigurationExtensionCompatibilityMode>Version8_3_27</ConfigurationExtensionCompatibilityMode>
<DefaultRunMode>ManagedApplication</DefaultRunMode>
```

Группа 2 (сразу после `<UsePurposes>`, перед `<DefaultRoles>`):
```
<ScriptVariant>Russian</ScriptVariant>
```

Группа 3 (сразу после `</DefaultRoles>`, перед `<UsedMobileApplicationFunctionalities>`)
— самая длинная, 26 тегов:
```
<Vendor>Фирма "1С"</Vendor>
<Version>3.1.12.297</Version>
<UpdateCatalogAddress>https://downloads.v8.1c.ru/tmplts/</UpdateCatalogAddress>
<IncludeHelpInContents>true</IncludeHelpInContents>
<UseManagedFormInOrdinaryApplication>true</UseManagedFormInOrdinaryApplication>
<UseOrdinaryFormInManagedApplication>false</UseOrdinaryFormInManagedApplication>
<AdditionalFullTextSearchDictionaries/>
<CommonSettingsStorage/>
<ReportsUserSettingsStorage/>
<ReportsVariantsStorage/>
<FormDataSettingsStorage/>
<DynamicListsUserSettingsStorage/>
<URLExternalDataStorage/>
<Content/>
<DefaultReportForm>CommonForm.ФормаОтчета</DefaultReportForm>
<DefaultReportVariantForm>CommonForm.ФормаВариантаОтчета</DefaultReportVariantForm>
<DefaultReportSettingsForm>CommonForm.ФормаНастроекОтчета</DefaultReportSettingsForm>
<DefaultReportAppearanceTemplate/>
<DefaultDynamicListSettingsForm/>
<DefaultSearchForm/>
<DefaultDataHistoryChangeHistoryForm/>
<DefaultDataHistoryVersionDataForm/>
<DefaultDataHistoryVersionDifferencesForm/>
<DefaultCollaborationSystemUsersChoiceForm/>
<RequiredMobileApplicationPermissions/>
```

Group 4 (сразу после `</UsedMobileApplicationFunctionalities>`, перед
`<BriefInformation>`) — 7 тегов:
```
<StandaloneConfigurationRestrictionRoles/>
<MobileApplicationURLs/>
<AllowedIncomingShareRequestTypes/>
<MainClientApplicationWindowMode>Normal</MainClientApplicationWindowMode>
<DefaultInterface/>
<DefaultStyle/>
<DefaultLanguage>Language.Русский</DefaultLanguage>
```

Group 5 (сразу после `</ConfigurationInformationAddress>`, перед
`</Properties>`) — 8 тегов:
```
<DataLockControlMode>Managed</DataLockControlMode>
<ObjectAutonumerationMode>NotAutoFree</ObjectAutonumerationMode>
<ModalityUseMode>UseWithWarnings</ModalityUseMode>
<SynchronousPlatformExtensionAndAddInCallUseMode>Use</SynchronousPlatformExtensionAndAddInCallUseMode>
<InterfaceCompatibilityMode>TaxiEnableVersion8_2</InterfaceCompatibilityMode>
<DatabaseTablespacesUseMode>DontUse</DatabaseTablespacesUseMode>
<CompatibilityMode>Version8_3_24</CompatibilityMode>
<DefaultConstantsForm/>
```

Полный diff (нативное дерево против нашего вывода) воспроизводится:

```
diff -u $S/cap/ssl-r1/src/Configuration.xml <вывод>/Configuration.xml
diff -u $S/cap/sslbase/src/Configuration.xml <вывод>/Configuration.xml
```

## Что НЕ было сделано

Эта сессия не трогала код Configuration-объекта: ни ридер, ни писатель
`Configuration.xml`. Дальше по остатку работали только над отдельными
конструкциями форм (`GraphicalSchemaField.Edit`, `LabelDecoration.Shortcut`
— см. коммит `b4048a3`), которые с этим блоком не пересекаются.

## Оснастка на момент записи

`$S/private/tmp/.../scratchpad` был очищен перезапуском процесса-владельца
в середине этой сессии (см. `docs/evidence/scratchpad-wipe-recovery-20260824.md`).
Нативные деревья `cap/ssl-r1/src` и `cap/sslbase/src` пересозданы локальным
`ibcmd infobase create --data=<dir> --load=<cf>` +
`ibcmd config export --data=<dir> <outdir>` и подтверждены точным числом
файлов (12 701 / 9 617). `base789/*.parity.json` восстановлен из текста этой
же сессии (см. тот же документ) — координаты выше сверены против него.
