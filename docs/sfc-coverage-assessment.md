# Оценка покрытия ibcmd-rs на базе D:\УХА\sfc

Дата оценки: 2026-06-27.

`D:\УХА\sfc` используется как крупный эталонный набор исходников 1С, а не как единственная целевая конфигурация. Цель оценки - понять, насколько `ibcmd-rs` сейчас способен полноценно выгружать конфигурацию из SQL в XML-исходники и загружать XML-исходники обратно в SQL для произвольных конфигураций такого класса сложности.

## Как читать эту оценку

Эта оценка не означает, что инструмент должен быть завязан на ERP/УХА или на конкретный путь `D:\УХА\sfc`. Эта папка нужна как мерная линейка: в ней есть почти все массовые типы метаданных, формы, макеты, роли, картинки, справка, командный интерфейс и большие бинарные assets. Если `ibcmd-rs` проходит такую конфигурацию, он ближе к универсальному инструменту для любых конфигураций 1С.

Ключевые режимы оценки:

| Направление | Команда/код | Текущий статус | Что еще надо доказать |
|---|---|---|---|
| Native SQL -> XML | `mssql-dump-config`, `src/mssql_dump.rs` | Частично работает: модули, многие `Ext`-тела, макеты, справка, роли, картинки; формы пока только каркас | Structural diff с эталонной выгрузкой по всей конфигурации или большому срезу |
| XML -> SQL staging | `mssql-stage-source-objects`, `src/mssql.rs` | Частично работает как обновление существующей базы через `ConfigSave`; не как полная загрузка в пустую базу | Массовый dry-run и реальный apply на большом срезе без неподдержанных body-файлов |
| XML -> пустая SQL-база | пока нет отдельного полноценного режима | Не доказано | Bootstrap всех `_Config`/metadata/body rows без зависимости от base blob-ов |
| Round-trip | `dump -> load/stage -> apply -> dump -> diff` | Доказан только на малых срезах; для `SpreadsheetDocument` есть сильный dry-run `XML -> blob -> XML -> blob -> XML` | Полный SQL end-to-end, особенно формы, роли, макеты и command interface |
| Wrapper над штатным `ibcmd` | `dump-sources` | Не считается native-возможностью `ibcmd-rs` | Использовать только как эталон/контроль, не как доказательство нашей реализации |

## Состав эталонной конфигурации

Конфигурация: `УправлениеХолдингомERP`, синоним `1С:ERP. Управление холдингом`, версия `3.3.3.3`, формат `2.21`, совместимость `Version8_3_27`.

Всего в дереве: 140 709 файлов, около 9 149 МБ. Внутри `Ext`: 83 950 файлов, около 8 746 МБ.

Всего в `Configuration.xml`: 25 977 объектов.

| Вид метаданных | Количество |
|---|---:|
| Языки | 2 |
| Подсистемы | 84 |
| Элементы стиля | 538 |
| Стили | 1 |
| Общие картинки | 3 248 |
| Параметры сеанса | 124 |
| Роли | 2 114 |
| Общие макеты | 335 |
| Критерии отбора | 16 |
| Общие модули | 4 704 |
| Общие реквизиты | 19 |
| Планы обмена | 19 |
| XDTO-пакеты | 429 |
| Веб-сервисы | 20 |
| HTTP-сервисы | 7 |
| WS-ссылки | 4 |
| Подписки на события | 663 |
| Регламентные задания | 300 |
| Хранилища настроек | 4 |
| Функциональные опции | 1 084 |
| Параметры ФО | 11 |
| Определяемые типы | 746 |
| Общие команды | 536 |
| Группы команд | 46 |
| Константы | 1 435 |
| Общие формы | 538 |
| Справочники | 1 466 |
| Документы | 891 |
| Нумераторы | 5 |
| Последовательности | 2 |
| Журналы документов | 61 |
| Перечисления | 2 063 |
| Отчеты | 1 329 |
| Обработки | 706 |
| Регистры сведений | 2 113 |
| Регистры накопления | 252 |
| ПВХ | 25 |
| Планы счетов | 3 |
| Регистры бухгалтерии | 5 |
| ПВР | 2 |
| Регистры расчета | 2 |
| Бизнес-процессы | 20 |
| Задачи | 2 |
| Сервисы интеграции | 2 |

## Файловые тела Ext

| Категория исходников | Количество в sfc | Текущее покрытие ibcmd-rs |
|---|---:|---|
| `*.bsl` | 29 818 | Загрузка и выгрузка модульных blob-ов реализованы. |
| `Ext/Form.xml` | 13 044 | Добавлен `audit-form-sources`: все 13 044 XML валидно парсятся как XML-структура; 12 216 форм имеют `Ext/Form/Module.bsl`, 12 218 имеют вложенные файлы в `Ext/Form`. Все 13 044 `Form.xml` / 590 173 726 XML bytes остаются неподдержанной полной структурой формы. Выгрузка сейчас формирует каркас формы, распознанные `Events`, `AutoCommandBar`, верхние свойства, базовые `Attributes` для `DynamicList`, базовые `Commands`, базовый `CommandInterface/NavigationPanel`, базовое дерево `ChildItems` для подтвержденных элементов layout и часть вложенных assets. Загрузка использует `Ext/Form.xml` для патча верхних `WindowOpeningMode`/`Group`, имени уже существующей `AutoCommandBar`, обработчиков уже существующих `Events`, существующих `Attributes`, существующих `Commands`, части существующего `CommandInterface/NavigationPanel` и части существующих `ChildItems` в base blob, дополнительно заменяет `Ext/Form/Module.bsl`, если он есть; полный compile структуры формы пока не реализован. |
| `Ext/Rights.xml` | 2 114 | Есть загрузка и выгрузка прав ролей, но нужна проверка на реальных RLS/шаблонах ограничений всей ERP. |
| `Ext/Schedule.xml` | 262 | Есть загрузка и выгрузка расписаний регламентных заданий. |
| `Ext/Template.xml` | 15 769 | Все найденные типы маршрутизируются. Для `SpreadsheetDocument` есть отдельный dry-run аудит: 14 046 из 14 046 файлов пакуются текущим кодом. |
| `Ext/Template.txt` | 1 022 | Текстовые макеты поддержаны как raw deflated body. |
| `Ext/Template.bin` | 831 | `AddIn` и `BinaryData` маршрутизируются как binary/base64 payload. |
| `Ext/Picture.xml` | 3 247 | Общие картинки поддержаны как `ExtPicture`; бинарные файлы считываются из вложенного каталога. |
| `*.zip` | 1 587 | Используются в общих картинках и других бинарных assets. |
| `*.svg` | 553 | Используются в общих картинках; упаковка идет как bytes payload. |
| `*.png` | 1 497 | Встречаются в assets форм/справки; выгрузка вложенных form item assets есть, загрузка таких вложений форм пока не полная. |
| `*.jpg` | 64 | Аналогично прочим бинарным assets. |
| `*.gif` | 21 | Аналогично прочим бинарным assets. |
| `Help.xml` | 6 333 | Загрузка и выгрузка справки реализованы через help blob. |
| `CommandInterface.xml` | 331 | Загрузка и выгрузка реализованы для распознанных command interface blob-ов. |
| `Predefined.xml` | 206 | Загрузка и выгрузка реализованы для поддержанных объектов с предопределенными данными. |
| `Content.xml` | 19 | Планы обмена поддержаны. |
| `Flowchart.xml` | 20 | Бизнес-процессы поддержаны. |

BSL по именам:

| Файл | Количество |
|---|---:|
| `Module.bsl` | 16 942 |
| `ManagerModule.bsl` | 6 082 |
| `ObjectModule.bsl` | 3 642 |
| `CommandModule.bsl` | 1 651 |
| `RecordSetModule.bsl` | 1 154 |
| `ValueManagerModule.bsl` | 343 |

Типы макетов в `sfc`:

| TemplateType | Количество | Текущее покрытие |
|---|---:|---|
| `SpreadsheetDocument` | 14 051 объектов, из них 14 046 с `Template.xml` | Pack-покрытие на `sfc` закрыто: `audit-spreadsheet-templates` упаковал 14 046 из 14 046. Semantic round-trip `XML -> blob -> XML -> blob -> XML` стабилен для 14 046 из 14 046 после исправления обратного преобразования format indexes, сохранения значимых пробелов в тексте, row `columnsID`, ложных пустых строк fallback-сканера и пустых листов. Для текущего dry-run аудита этот блок закрыт; остается подтвердить его в полном SQL `dump -> load -> dump` сценарии. |
| `DataCompositionSchema` | 1 541 | Поддержан как raw deflated XML body. Семантической проверки СКД нет. |
| `TextDocument` | 1 022 | Поддержан как raw deflated text body. |
| `BinaryData` | 723 объекта, из них 719 с `Template.bin` | Поддержан как binary/base64 payload, но нужна проверка round-trip. |
| `AddIn` | 112 | Поддержан как binary/base64 payload, но нужна проверка round-trip. |
| `HTMLDocument` | 113 | Поддержан через help-like blob. |
| `GraphicalSchema` | 64 | Поддержан как raw deflated XML body. |
| `DataCompositionAppearanceTemplate` | 5 | Поддержан как raw deflated XML body. |

Форматы общих картинок по `xr:Abs` в `Picture.xml`: `zip` 1 587, `png` 928, `svg` 550, `bmp` 151, `gif` 17, `jpg` 11, `ico` 3. Все ссылки из `Picture.xml` нашли физический файл в `Ext/Picture/...`. При выгрузке `ExtPicture` JPEG/BMP требуют отдельной проверки, так как текущий детектор формата явно покрывает не все расширения.

## Фактические проверки на `sfc`

| Проверка | Результат | Вывод |
|---|---:|---|
| `scan D:\УХА\sfc` | дерево сканируется; manifest около 57 МБ, около 32 секунд | Сканер распознает большую структуру исходников и подходит как вход для дальнейших dry-run проверок. |
| `audit-spreadsheet-templates D:\УХА\sfc` до поддержки `v8ui:Print` | 14 034 / 14 046 `SpreadsheetDocument` packed | Ошибки были связаны не с узлами MOXCEL, а со стандартными картинками платформы. |
| `audit-spreadsheet-templates D:\УХА\sfc` после поддержки `v8ui:Print` | 14 042 / 14 046 packed, 487.422 секунды | Осталось 4 ошибки: 2 `InputFieldCalculator`, 1 `Information`, 1 `SaveFile`. |
| `audit-spreadsheet-templates D:\УХА\sfc` после поддержки всех найденных стандартных картинок | 14 046 / 14 046 packed, 488.805 секунды | Pack-аудит всех табличных макетов `sfc` проходит без отказов. |
| `audit-spreadsheet-roundtrip D:\УХА\sfc` после добавления параллельного аудита | 14 046 packed, 13 105 extracted, 13 105 repacked, 638 matched, 12 334 different, 1 074 extract failures; 81.049 секунды release-прогона вместе с компиляцией | Базовая точка: паковать все SpreadsheetDocument уже можем, но стабильный цикл выгрузки/загрузки макета проходил только на небольшой доле. |
| `audit-spreadsheet-roundtrip D:\УХА\sfc` после исправления format index round-trip | 14 046 packed, 13 105 extracted, 13 105 repacked, 4 515 matched, 8 457 different, 1 074 extract failures; 76.909 секунды release-прогона вместе с компиляцией | Массовый сдвиг format indexes закрыт. Основной фронт работ теперь - XML/text-нормализация в `compare` и 941 отказ первичного `extract` плюс 133 отказа `extract-repacked`. |
| `audit-spreadsheet-roundtrip D:\УХА\sfc` после сохранения пробелов вокруг XML entities в тексте | 14 046 packed, 13 105 extracted, 13 105 repacked, 5 903 matched, 7 069 different, 1 074 extract failures; 80.179 секунды release-прогона вместе с компиляцией | Закрыта потеря значимых пробелов в HTML/text content. Первые оставшиеся `compare` теперь связаны с потерей `columnsID` у строк. |
| `audit-spreadsheet-roundtrip D:\УХА\sfc` после сохранения row `columnsID` pair-mapping с нулевой строкой | 14 046 packed, 13 105 extracted, 13 105 repacked, 12 734 matched, 238 different, 1 074 extract/extract-repacked failures; 80.578 секунды release-прогона вместе с компиляцией | Закрыт массовый класс потери `columnsID`. Первые оставшиеся `compare` связаны с появлением/потерей пустых строк перед `templateMode`; отдельно остаются 941 первичный отказ `extract` и 133 отказа `extract-repacked`. |
| `audit-spreadsheet-roundtrip D:\УХА\sfc` после ограничения fallback-сканера пустых строк | 14 046 packed, 13 105 extracted, 13 105 repacked, 12 972 matched, 0 different, 1 074 extract/extract-repacked failures; 77.736 секунды release-прогона вместе с компиляцией | Закрыты все оставшиеся compare-расхождения SpreadsheetDocument. Следующий фронт - 941 первичный отказ `extract` и 133 отказа `extract-repacked`. |
| `audit-spreadsheet-roundtrip D:\УХА\sfc` после поддержки пустых листов в fallback-сканере | 14 046 packed, 14 046 extracted, 14 046 repacked, 14 046 matched, 0 different, 0 failed; 83.434 секунды release-прогона вместе с компиляцией | Текущий semantic round-trip всех табличных макетов `sfc` проходит полностью. Следующий фронт - SQL end-to-end проверка макетов и остальные крупные блоки, прежде всего формы. |
| `audit-form-sources D:\УХА\sfc` | 13 044 `Form.xml`, 13 044 parsed, 0 failed, 590 173 726 XML bytes, max 1 375 225 bytes; 12 216 forms with module, 859 045 118 module bytes; 12 218 forms with nested `Ext/Form` files, 12 511 files, 860 770 681 bytes; current loader stageable 12 216, without stageable body 828; ignored non-module `Ext/Form` files 295 / 1 725 563 bytes; 38.1 секунды release-прогона вместе с компиляцией | Получен измеримый baseline для следующего компилятора/декомпилятора форм. Самые массовые верхние секции: `Attributes`, `AutoCommandBar`, `Group`, `WindowOpeningMode` во всех формах; `ChildItems` в 12 941; `Events` в 11 918; `Commands` в 8 931. Проверка маленькой реальной формы `DataProcessors/ЭлектронноеВзаимодействие/Forms/ПустаяФорма` через `form-info`: `Group=Vertical`, `WindowOpeningMode=DontBlock`, событие `OnOpen -> ПриОткрытии`, `AutoCommandBar`. |
| `audit-source-load-coverage D:\УХА\sfc` | 140 709 файлов / 9 593 572 857 bytes; stage entry files 55 789, из них metadata XML 51 085 и common module XML 4 704; potentially stageable body files 59 970; module files 29 818, supported module files 29 814; supported Ext body files 30 156; known uncovered 13 342 files / 592 676 841 bytes | Общий source-side baseline для любой конфигурации: показывает, какие исходники текущий loader вообще рассматривает как вход, и какие известные блоки остаются вне загрузки до SQL. Основной uncovered блок - 13 044 полных `Ext/Form.xml` / 590 173 726 bytes; 12 216 из них имеют stageable module-only путь, 828 не имеют даже `Ext/Form/Module.bsl`; дополнительно 295 non-module files внутри `Ext/Form` / 1 725 563 bytes; из корневых configuration assets закрыты `MobileClientSignature.bin` и `MainSectionCommandInterface.xml`. |
| `mssql-audit-source-parity --database ibcmd_rs_sfc_20260626_v85 --source-root D:\УХА\sfc --path-prefix Catalogs/Валюты` | 5 metadata XML выбрано, 5 metadata objects подготовлено, 11 body rows, 16 total Config rows; prepare failures 0; `version_patch_error`: empty; 17 `version_replacements`, включая новую entry `7aadbb67-f93e-43bb-9f53-f14d2c2a347a.5`; 1 batch, expected_total_rows 19 | SQL-backed dry-run на реальной копии базы `sfc`: `versions` patch проходит для большого flat-list shape, а отсутствующий baseline help blob `.5` теперь готовится как новая `ConfigSave` body row с добавлением entry в `versions`. |
| `mssql-stage-source-objects --database ibcmd_rs_sfc_stage_valyuty_20260627 --source-root D:\УХА\sfc --path-prefix Catalogs/Валюты --batch-size 10 --replace-config-save --allow-non-lab` | Реальный staging на одноразовой SQL-копии: 5 metadata objects, 11 body rows, 1 script; `ConfigSave` после записи: 19 rows / 4 106 522 binary bytes; новая строка `7aadbb67-f93e-43bb-9f53-f14d2c2a347a.5` записана с `DataSize=2685`, `versions` записан с `DataSize=4055698` | Первый подтвержденный SQL write-path для среза `sfc`: scoped source staging записывает полный staged set в `ConfigSave`, включая help body row, которой не было в active `Config`, и обновленный `versions`. |
| `ibcmd infobase config check/apply --dbms=MSSQLServer --db-server=localhost --db-name=ibcmd_rs_sfc_stage_valyuty_20260627` после staging `Catalogs/Валюты` | `config check` успешно прошел за 22.9 секунды; `config apply --dynamic=disable --session-terminate=force` успешно прошел за 45.6 секунды и создал поколение `8257a3ea35791b46b68297a2ebda2b6600000000`; после apply `ConfigSave=0 rows`, `Config=118077 rows / 1 637 310 030 binary bytes`; активные строки `7aadbb67-f93e-43bb-9f53-f14d2c2a347a`, `.0`, `.5` и `versions` присутствуют в `Config`; штатный `ibcmd export objects --recursive Catalog.Валюты` после apply успешно выгрузил 23/23 файлов, без missing/extra; 11 файлов совпали с `D:\УХА\sfc` по SHA-256, включая все BSL-модули и HTML справки, 12 XML-файлов отличаются байтово | Первый end-to-end SQL -> `ConfigSave` -> штатный `ibcmd apply` -> штатный export для реального среза `sfc`. Это доказывает применимость текущего write-path на ограниченном объекте с metadata, модулем, справкой и `versions`, но не доказывает полную загрузку всей конфигурации или байтовую реконструкцию всех XML, особенно `Ext/Form.xml`. |
| `mssql-dump-config --database ibcmd_rs_sfc_stage_valyuty_20260627 --file-name 7aadbb67-f93e-43bb-9f53-f14d2c2a347a --file-name 5f91b00f-d8fc-4d63-8486-66339357ab22 --extract-metadata-xml` после staged apply | Native dump applied-копии больше не падает на duplicate `Catalogs/Валюты/Ext/Help.xml`: при наличии legacy help `.1` и нового help `.5` выгрузчик выбирает `.5`, `Catalogs/Валюты/Ext/Help/ru.html` совпадает с исходником по SHA-256 `47FBC12B7702CD43828A54988143C670F6DB4046C79877432978FDEBC5557DE1`; unit-тесты добавлены для выбора `.5` и для извлечения UUID-based событий формы + `AutoCommandBar` | Закрыт реальный dump-дефект, проявившийся после нашего staging/apply. Выгрузка `Ext/Form.xml` стала ближе к оригиналу по событиям и `AutoCommandBar`, но при неполном selected-наборе form path resolver всё еще может ошибочно класть форму в `IntegrationServices/...`; полная реконструкция `WindowOpeningMode`, `Group`, `ChildItems`, `Attributes` остается открытой. |
| Повторный `mssql-dump-config --database ibcmd_rs_sfc_stage_valyuty_20260627 --file-name 7aadbb67-f93e-43bb-9f53-f14d2c2a347a --file-name 5f91b00f-d8fc-4d63-8486-66339357ab22 --extract-metadata-xml` после исправления form metadata shape `14` | Dump selected-набора проходит: 7 rows, 25 489 binary bytes, 3 module text rows, 2 metadata XML rows, 2 source asset rows. `5f91b00f-d8fc-4d63-8486-66339357ab22` теперь выгружается в `Catalogs/Валюты/Forms/ФормаСписка.xml`, body `.0` - в `Catalogs/Валюты/Forms/ФормаСписка/Ext/Form.xml`, module - в `Catalogs/Валюты/Forms/ФормаСписка/Ext/Form/Module.bsl`; ошибочные пути `IntegrationServices/ФормаСписка.xml` и `CommonForms/Валюты/Ext/Help.xml` отсутствуют | Закрыт path resolver дефект для реальной SFC формы с wrapped form metadata code `14`. Это улучшает точечную native SQL -> XML выгрузку до layout, ожидаемого `ibcmd export`; содержимое `Ext/Form.xml` всё еще частичное. |
| Повторный `mssql-dump-config --database ibcmd_rs_sfc_stage_valyuty_20260627 --file-name 7aadbb67-f93e-43bb-9f53-f14d2c2a347a --file-name 5f91b00f-d8fc-4d63-8486-66339357ab22 --extract-metadata-xml` после извлечения верхних свойств формы | `Catalogs/Валюты/Forms/ФормаСписка/Ext/Form.xml` теперь содержит `WindowOpeningMode=DontBlock`, `Group=Vertical`, `AutoCommandBar`, события `ChoiceProcessing`, `NotificationProcessing`, `OnCreateAtServer`; тесты добавлены для `WindowOpeningMode` codes `0/1/2` и подтвержденных `Group` mappings `Vertical`/`Horizontal`/`AlwaysHorizontal` | Еще один шаг к native decompile формы. `Attributes`, `ChildItems`, команды и полное дерево элементов пока не реконструируются, поэтому это не полная совместимость с `ibcmd export`. |
| Повторный `mssql-dump-config --database ibcmd_rs_sfc_stage_valyuty_20260627 --file-name 7aadbb67-f93e-43bb-9f53-f14d2c2a347a --file-name 5f91b00f-d8fc-4d63-8486-66339357ab22 --extract-metadata-xml` после разбора tail-секций формы | `Catalogs/Валюты/Forms/ФормаСписка/Ext/Form.xml` теперь дополнительно содержит `Attributes` с `Список: DynamicList`, `UseAlways` по `Список.Наименование`/`Список.Ссылка`, `QueryText`, `MainTable=Catalog.Валюты`, а также команды `ПодборИзКлассификатора` и `ЗагрузитьКурсыВалют` с `Title`, `ToolTip`, `Action`, `CurrentRowUse`. `form-info` успешно распознает выгруженный файл как форму `ФормаСписка` с DynamicList и двумя командами | Закрыт первый реальный слой tail body формы: базовые `Attributes` и `Commands`. Это все еще не полная форма: нет `ChildItems`, `CommandInterface`, `ListSettings`, всех типов реквизитов, параметров и полного round-trip compile обратно в blob. |
| Повторный `mssql-dump-config --database ibcmd_rs_sfc_stage_valyuty_20260627 --file-name 7aadbb67-f93e-43bb-9f53-f14d2c2a347a --file-name 5f91b00f-d8fc-4d63-8486-66339357ab22 --file-name 097b7cbf-cd63-4213-96ae-fc88d1be24d6 --file-name b34b580f-305b-4e22-b601-cfcf4dabaae0 --extract-metadata-xml` после разбора `CommandInterface` формы | Selected dump с владельцами ссылочных команд выгрузил валидный `Catalogs/Валюты/Forms/ФормаСписка/Ext/Form.xml` с `<CommandInterface><NavigationPanel>`: `DataProcessor.ЗагрузкаКурсовВалютЕЦБ.Command.ЗагрузитьКурсыВалютЕЦБ`, `InformationRegister.ОтносительныеКурсыВалют.Command.ЗагрузитьКурсыИзТаблицы`, `CommandGroup=FormNavigationPanelGoTo`, `Index=1` у первой команды, `DefaultVisible=false`, `Visible/xr:Common=false`. Ключевые поля совпадают с исходным `D:\УХА\sfc` фрагментом | Закрыт базовый decompile navigation panel внутри формы. Ограничение: для корректных имен команд нужен индекс UUID по владельцам команд; selected dump только формы без владельцев не может восстановить эти ссылки. Остались `ChildItems`, `ListSettings`, параметры, все типы реквизитов и полный compile формы обратно в blob. |
| Повторный `mssql-dump-config --database ibcmd_rs_sfc_stage_valyuty_20260627 --file-name 7aadbb67-f93e-43bb-9f53-f14d2c2a347a --file-name 5f91b00f-d8fc-4d63-8486-66339357ab22 --file-name 097b7cbf-cd63-4213-96ae-fc88d1be24d6 --file-name b34b580f-305b-4e22-b601-cfcf4dabaae0 --file-name 9e3fce60-09bb-4d21-af76-3ac4e1a12396 --extract-metadata-xml` после разбора `ChildItems` формы | Selected dump с владельцами всех внешних команд выгрузил валидный `Form.xml`; `form-info` видит дерево элементов формы: `ГруппаПользовательскихНастроек`, `КоманднаяПанель`, popup `Создать`, кнопки локальных/стандартных/внешних команд, popup `ГруппаЗагрузитьКурсыВалют`, таблицу `Валюты -> Список`, колонки `НаименованиеПолное`, `Код`, `Наименование`, `ОтносительныйКурс`, `Ссылка`. Восстановлены `CommandName` для локальных команд, стандартных `Create/Help` и внешних команд при наличии owner metadata; восстановлены `DataPath` таблицы и колонок | Закрыт базовый decompile layout tree для подтвержденных элементов (`UsualGroup`, `CommandBar`, `Popup`, `ButtonGroup`, `Button`, `Table`, `InputField`, `LabelField`, search additions). Это все еще не полный `Form.xml`: не покрыты все типы элементов, параметры, расширенные свойства элементов, события элементов вроде `OnGetDataAtServer`, `ListSettings` и compile обратно в blob. |
| `mssql-audit-source-parity --database ibcmd_rs_sfc_stage_valyuty_20260627 --source-root D:\УХА\sfc --path-prefix Catalogs/Валюты/Forms/ФормаСписка --batch-size 10` после scoped source scan | Команда завершилась за 5 секунд вместо прежнего timeout: scoped coverage содержит 3 файла / 34 102 bytes, 1 metadata XML, 1 module, 1 `Ext/Form.xml`; prepared metadata objects 1, body rows 1, total Config rows 2; prepare failures 0; batch expected_total_rows 5 | Scoped parity-аудит теперь пригоден для быстрой проверки отдельных форм и не сканирует весь `sfc` при заданном `path-prefix`. Это напрямую ускоряет итерации по загрузке `Form.xml -> blob`. |
| `mssql-audit-source-parity --database ibcmd_rs_sfc_stage_valyuty_20260627 --source-root D:\УХА\sfc --path-prefix Catalogs/Валюты --batch-size 10` после scoped source scan | Команда завершилась за несколько секунд: scoped coverage содержит 24 файла / 261 076 bytes, 5 stage metadata XML, 11 potentially stageable body files; prepared metadata objects 5, body rows 11, total Config rows 16; prepare failures 0; batch expected_total_rows 19 | Срез справочника `Валюты` снова стал быстрым regression-check для загрузочного пути. Ранее этот аудит таймаутился из-за полного scan/coverage по 140 709 файлам `sfc` даже при узком prefix. |
| `mssql-audit-source-parity --database ibcmd_rs_sfc_20260626_v85 --source-root D:\УХА\sfc --path-prefix CommonModules/CRMЛокализация --path-prefix Roles/АдминистраторПроцесса --path-prefix Enums/ВариантыЗагрузкиРабочихЦентров --path-prefix CommonPictures/AppStore` | 3 metadata XML и 1 common module XML выбрано; 3 metadata objects и 1 common module подготовлены; 2 body rows, 7 total Config rows; prepare failures 0; `version_patch_error`: empty; 8 `version_replacements`; 1 batch, expected_total_rows 10 | Смешанный smoke-аудит по разным семействам показал, что текущий loader может подготовить простой срез metadata/common module/body rows против SQL baseline и собрать staged `versions` blob. |
| `cargo test` | 323 passed | Unit-покрытие текущего кода стабильно после изменений. |

## Загрузка XML -> SQL

Текущий путь загрузки находится в `src/mssql.rs`:

| Область | Статус | Код |
|---|---|---|
| XML-карточки метаданных | Частично/широко | `prepare_metadata_object_stage`, `pack_simple_metadata_blob_from_xml_with_source` |
| Тела объектов | Частично | `prepare_metadata_body_rows` |
| Стили | Есть | `prepare_style_body_row`, `pack_style_body_blob_from_xml` |
| Регламентные задания | Есть | `prepare_scheduled_job_body_row`, `pack_schedule_blob_from_xml` |
| XDTO/WS/raw XML bodies | Есть | `prepare_raw_deflated_body_row` |
| Макеты | Частично | `prepare_template_body_row` |
| Табличные макеты | Частично | `prepare_spreadsheet_template_body_row`, `pack_moxel_spreadsheet_blob_from_xml` |
| HTML-макеты | Есть | `prepare_html_template_body_row` |
| Binary/AddIn | Есть | `prepare_binary_template_body_row` |
| Общие картинки | Есть | `prepare_common_picture_body_row`, `pack_ext_picture_blob_from_bytes` |
| Предопределенные данные | Есть для поддержанных типов | `prepare_predefined_data_body_row` |
| Планы обмена | Есть | `prepare_exchange_plan_content_body_row` |
| Бизнес-процессы | Есть | `prepare_business_process_flowchart_body_row` |
| Формы | Существенно частично | `prepare_form_body_row` читает `Ext/Form.xml` и патчит верхние `WindowOpeningMode`/`Group`, имя уже существующей `AutoCommandBar`, обработчики уже существующих `Events`, существующие `Attributes` (`name`, `MainAttribute`, часть `DynamicList` settings), существующие `Commands`, существующий `CommandInterface/NavigationPanel` (`Command`, `CommandGroup`, `Index`, `DefaultVisible`, `Visible/xr:Common`) и существующие `ChildItems` (`name`, `Title`, часть `Button/CommandName`, включая внешние `Kind.Object.Command.Name` при наличии `source_root`, `Button/DataPath` для `Items.<Table>.CurrentData.<Field>` по `Ссылка` и найденным колонкам таблицы) в base blob; если есть `Ext/Form/Module.bsl`, дополнительно заменяет текст модуля. Полная компиляция `Ext/Form.xml` пока не реализована. |
| Роли | Частично | `prepare_role_rights_body_row`, `pack_role_rights_blob_from_xml`; количество объектов/прав должно совпадать с base blob |
| Справка | Есть | `prepare_object_help_body_row`, `pack_help_blob_from_parts` |
| Модули | Есть | `prepare_object_module_body_rows`, `pack_module_blob_bytes` |
| Интерфейс команд | Есть | `prepare_command_interface_body_row`, `pack_command_interface_blob_from_xml` |

Важное ограничение: текущий staging не является полной загрузкой в абсолютно пустую SQL-базу. Многие упаковщики берут существующий blob из базы как основу через `fetch_config_blob`, а затем патчат его. Значит сейчас практический сценарий - обновление/перезапись существующей базы-шаблона, а не самостоятельное создание всей конфигурации с нуля.

Дополнительный риск для большой загрузки: `mssql-stage-source-objects` режет исходники на batch-и. Нужно отдельно проверить расчет ожидаемого числа строк для второго и последующих batch-ей, потому что на `sfc` десятки тысяч XML-объектов и ошибка в batch accounting сломает длинную загрузку раньше функциональных packer-ов.

## Выгрузка SQL -> XML

Текущий путь выгрузки находится в `src/mssql_dump.rs`:

| Область | Статус | Код |
|---|---|---|
| XML-карточки метаданных | Частично/широко | `extract_metadata_source_xml_with_refs` |
| CommonPicture | Есть | `SourceAssetKind::ExtPicture` |
| ScheduledJob | Есть | `SourceAssetKind::Schedule` |
| XDTOPackage/WSReference/raw bodies | Есть | `SourceAssetKind::InflatedBinary` |
| Style body | Есть | `SourceAssetKind::StyleBody` |
| Role rights | Есть, требует stress-test | `SourceAssetKind::RoleRights` |
| CommandInterface | Есть | `SourceAssetKind::CommandInterface` |
| Help | Есть | `SourceAssetKind::Help` |
| PredefinedData | Есть | `SourceAssetKind::PredefinedData` |
| ExchangePlan content | Есть | `SourceAssetKind::ExchangePlanContent` |
| BusinessProcess flowchart | Есть | `SourceAssetKind::BusinessProcessFlowchart` |
| Template bodies | Частично/широко | `template_body_source_asset` |
| SpreadsheetDocument | Частично | `extract_moxel_spreadsheet_xml` |
| Forms | Существенно частично | `extract_form_body_xml` возвращает каркас `Form.xml`, распознает `WindowOpeningMode`, подтвержденные варианты верхнего `Group`, часть `Events` включая UUID-based события `OnOpen`/`ChoiceProcessing`/`NotificationProcessing`/`OnCreateAtServer`, извлекает `AutoCommandBar`, базовые tail-секции `Attributes` для `DynamicList`, `Commands`, `CommandInterface/NavigationPanel` и базовое дерево `ChildItems`; resolver форм поддерживает wrapped form metadata code `13` и `14`; часть вложенных item assets выгружается отдельно. Разбор `{4, layout, module, ...}` вынесен в общий `parse_form_body_blob`, который также используется при загрузочной замене модуля формы. |

Важное ограничение выгрузки: многие metadata XML пока минимальные (`Name`, `Synonym`, `Comment`). Расширенные свойства есть для части типов, но это еще не полная реконструкция исходников `ibcmd`.

Отдельно: команда `dump-sources` не доказывает native SQL -> XML выгрузку `ibcmd-rs`, потому что она является wrapper-ом над внешним `ibcmd infobase config export`. Native путь выгрузки сейчас - `mssql-dump-config` и код в `src/mssql_dump.rs`.

## Главные разрывы до полной совместимости с ibcmd

1. Полные управляемые формы. Для `sfc` это 13 044 `Ext/Form.xml`, самый крупный обязательный блок после модулей и табличных макетов. Нужны полноценные decompile/compile form body, а не только модуль формы.
2. Табличные макеты MOXCEL. Для `sfc` это 14 051 объект `SpreadsheetDocument`, 14 046 файлов `Template.xml`; суммарно макеты занимают около 5 958 МБ. Pack-аудит уже проходит 14 046 из 14 046 файлов. Теперь нужно доказывать не только упаковку, а полный `dump -> load -> dump -> diff`.
3. Загрузка в новую пустую базу. Сейчас код зависит от base blob-ов существующей базы; для полного аналога `ibcmd` нужен режим создания/вставки всех строк `_Config`/metadata bodies без опоры на старые blob-ы или надежный bootstrap минимальной базы.
4. Полный round-trip тест на большой конфигурации. Нужны тесты вида `dump -> load -> dump -> diff` на копии базы, а не только unit tests упаковщиков.
5. Проверка всех RLS/прав ролей на ERP. Парсер ролей есть, но 2 114 файлов `Rights.xml` требуют массового теста на реальных данных.
6. Проверка HTML/help assets. Количество `Help.xml` и HTML-файлов большое; нужна массовая сверка после round-trip.
7. Командный интерфейс и ссылки между объектами. Есть resolver-ы для части ссылок, но нужна полная проверка на 331 `CommandInterface.xml` и все ссылки метаданных.
8. Корневые assets конфигурации. В `sfc` встречаются `AdditionalIndexes.xml`, `StandaloneConfigurationContent.bin`, `ClientApplicationInterface.xml`, `HomePageWorkArea.xml`; текущий staging покрывает часть конфигурационных assets (`Splash`, `ParentConfigurations`, `MainSectionPicture`, `MobileClientSignature`, `MainSectionCommandInterface`).
9. Бинарные legacy bodies. В `sfc` встречаются `Form.bin`, `Module.bin`, `ObjectModule.bin`; текущий код ориентирован на XML/BSL-представление и не импортирует эти файлы как отдельный полноценный источник.

## Итоговая оценка универсальной готовности

`ibcmd-rs` уже подходит как база для инструмента загрузки/выгрузки и покрывает много инфраструктурных форматов. Но на основании проверки через `D:\УХА\sfc` его нельзя считать полноценно готовым аналогом `ibcmd` для произвольных конфигураций.

Числа ниже не являются целью "подогнать инструмент под `sfc`". Они показывают масштаб классов объектов, которые должны стабильно проходить в любой большой конфигурации: если класс закрыт на `sfc`, это сильный аргумент в пользу универсальности; если не закрыт, это риск для любых похожих ERP/КА/УХА-конфигураций.

Короткий вывод на 2026-06-27: текущий `ibcmd-rs` уже может выгружать и загружать значимые срезы конфигурации, но пока не может гарантированно полностью выгрузить произвольную SQL-базу в исходники и затем загрузить эти исходники в новую базу без штатного `ibcmd`.

Ориентировочная готовность:

| Область | Объем в `sfc` | Native SQL -> XML | XML -> SQL | Главный остаток |
|---|---:|---|---|---|
| XML-карточки объектов | 25 977 объектов в `Configuration.xml`; 51 085 metadata XML в source coverage | Средняя/высокая: многие типы распознаются, но часть XML минимальная | Средняя/высокая: stage metadata работает, но зависит от base blob | Mass diff и расширение свойств metadata XML до уровня `ibcmd export` |
| Модули BSL | 29 818 `.bsl` | Высокая | Высокая для поддержанного whitelist; 29 814 файлов отмечены поддержанными source-side | Добить редкие неподдержанные `.bsl` и проверить большой apply |
| Управляемые формы | 13 044 `Ext/Form.xml`, 590 МБ XML; 12 216 form modules | Низкая/средняя для частичной выгрузки: есть `WindowOpeningMode`, `Group`, `AutoCommandBar`, часть `Events`, базовые `Attributes`/`Commands`/`CommandInterface`/`ChildItems`, module/assets | Низкая/частичная: `Ext/Form.xml` патчит top-level `WindowOpeningMode`/`Group`, имя уже существующей `AutoCommandBar`, обработчики уже существующих `Events`, существующие `Attributes`, существующие `Commands`, часть существующего `CommandInterface`, часть существующих `ChildItems`, внешние command refs для подтвержденных путей при наличии `source_root`, `Ext/Form/Module.bsl` заменяет модуль; полный `Ext/Form.xml` не компилируется | Полный compiler/decompiler всех `ChildItems`, `Parameters`, все типы реквизитов, `ListSettings`, добавление/удаление attributes/events/commands/items, расширить резолюцию всех ссылок и вложенные form assets |
| Табличные макеты `SpreadsheetDocument` | 14 046 `Template.xml` | Высокая в dry-run: semantic round-trip 14 046/14 046 | Высокая для pack-аудита, но SQL e2e не доказан | SQL `dump -> load -> dump -> diff` на реальной базе |
| Прочие макеты | 1 541 DCS, 1 022 Text, 831 bin, 113 HTML, 64 GraphicalSchema, 5 appearance | Средняя/высокая | Средняя/высокая по маршрутам packer-ов | Round-trip/stress-test по каждому TemplateType |
| Общие картинки | 3 248 объектов, 3 247 `Picture.xml` | Высокая, но JPEG/BMP требуют отдельной проверки детектора | Высокая для `CommonPictures` и части configuration pictures | Проверка всех форматов `zip/png/svg/bmp/gif/jpg/ico` в SQL e2e |
| Справка | 6 333 `Help.xml` | Средняя/высокая | Средняя/высокая | Массовая сверка HTML/help assets после round-trip |
| Роли | 2 114 `Rights.xml`, 496 МБ | Средняя | Средняя | Stress-test всех прав, RLS и шаблонов ограничений |
| CommandInterface | 331 `CommandInterface.xml` | Средняя | Средняя | Полная сверка ссылок и порядка команд |
| Прочие Ext bodies | `Schedule`, `Predefined`, `Content`, `Flowchart`, `Style`, raw XML bodies | Средняя/высокая | Средняя/высокая | Массовая проверка всех типов с diff |
| Корневые assets конфигурации | несколько известных файлов `Ext/*` | Частично | Частично | Закрыть `AdditionalIndexes.xml`, `StandaloneConfigurationContent.bin`, `ClientApplicationInterface.xml`, `HomePageWorkArea.xml`; `MobileClientSignature.bin` уже маршрутизируется как deflated raw body, `MainSectionCommandInterface.xml` - как CommandInterface body |
| Загрузка в новую пустую SQL-базу | вся конфигурация | Не применимо | Низкая/не доказана | Bootstrap без существующих base blob-ов |

## Разбиение работ по агентам

| Направление | Ответственность | Ближайший критерий готовности |
|---|---|---|
| Агент загрузки XML -> SQL | `src/mssql.rs`, stage/load CLI, batch-и, dry-run prepare | Есть отчет по всем объектам `sfc`: сколько root XML выбрано, сколько body rows подготовлено, сколько файлов проигнорировано; batch row count покрыт тестом. |
| Агент выгрузки SQL -> XML | `src/mssql_dump.rs`, native `mssql-dump-config`, source layout writer | Есть structural diff между native dump и эталонным layout: missing/extra/different по типам файлов. |
| Агент макетов и assets | `src/module_blob.rs`, `src/source_audit.rs`, MOXCEL, CommonPictures, Template.bin | `audit-spreadsheet-templates` дает 14 046 / 14 046 packed; `audit-spreadsheet-roundtrip` дает 14 046 / 14 046 matched без отказов и compare-расхождений. Следующий критерий - SQL end-to-end проверка SpreadsheetDocument и stress-test `BinaryData`/`AddIn`. |
| Агент форм | compile/decompile `Ext/Form.xml`, form item assets | Есть `audit-form-sources` baseline: 13 044 форм, 590 МБ XML, 12 216 stageable-by-module, 828 без stageable body, 295 ignored non-module `Ext/Form` files. Обратный compile уже патчит верхние свойства, существующую `AutoCommandBar`, существующие `Events`, существующие `Attributes`, существующие `Commands`, часть существующего `CommandInterface/NavigationPanel` и часть существующих `ChildItems`, включая локальные/стандартные/внешние `Button/CommandName`, внешний `CommandInterface/NavigationPanel/Item/Command` при наличии `source_root` и явный `Button/DataPath` по `Ссылка`/колонкам таблицы. Следующий критерий - расширить резолюцию остальных ссылок и затем создание/удаление элементов хотя бы на малой форме, затем расширять до `sfc`. |
| Агент интеграции | test harness, SQL clone, сравнение с `ibcmd` | Есть сценарий `ibcmd load` vs `ibcmd-rs stage/load` на копии базы и post-compare по `_Config`/`ConfigSave`/source dump. |
| Агент parity-аудита | source coverage, SQL dry-run, batch accounting, dump/load diff | Есть `audit-source-load-coverage` source-side baseline без SQL: 55 789 stage entry files, 59 970 potentially stageable body files, 13 342 known uncovered. SQL-backed `mssql-audit-source-parity` уже прогнан на копии `sfc` по справочнику `Валюты`, форме `Валюты.ФормаСписка` и смешанному smoke-срезу; scoped scan по `path-prefix` больше не обходит весь `sfc`. Actual `mssql-stage-source-objects --path-prefix Catalogs/Валюты` записал 19-row staged set в `ConfigSave`, штатный `ibcmd config check/apply` применил этот staged set, а post-export подтвердил отсутствие missing/extra файлов на срезе. Следующий критерий - расширить проверку на несколько семейств объектов и добавить native `ibcmd-rs dump -> compare` после apply. |

Ближайший рациональный план работ:

1. Довести `SpreadsheetDocument` от закрытого dry-run round-trip 14 046/14 046 до SQL end-to-end: `native dump -> stage/load -> apply -> native dump -> diff`.
2. Проверить и покрыть тестом batch accounting в `mssql-stage-source-objects`, особенно второй batch и stable rows.
3. Расширить `mssql-audit-source-parity` на копии SQL-базы `sfc` по нескольким семействам объектов и разложить failures по типам: missing base blob, unsupported packer, broken source reference, batch/accounting issue.
4. Начать отдельную ветку по полноценному `Form.xml` round-trip: сначала анализ реальной структуры, затем выгрузка, затем упаковка обратно.
5. Добавить native structural diff `mssql-dump-config -> source layout -> compare with sfc`.
6. Добавить round-trip tests для `BinaryData`, `AddIn`, всех форматов `CommonPictures`, ролей и command interface.
7. Добавить интеграционный harness `dump -> load -> dump -> diff` на небольшой базе, затем на подмножестве `sfc`.
8. После этого замерять скорость против `ibcmd`: до закрытия форм и end-to-end load такой замер будет показывать скорость неполного сценария, а не честную замену.
