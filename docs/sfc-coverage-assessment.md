# Оценка покрытия ibcmd-rs на базе D:\УХА\sfc

Дата оценки: 2026-06-27.

`D:\УХА\sfc` используется как крупный эталонный набор исходников 1С, а не как единственная целевая конфигурация. Цель оценки - понять, насколько `ibcmd-rs` сейчас способен полноценно выгружать конфигурацию из SQL в XML-исходники и загружать XML-исходники обратно в SQL.

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
| `Ext/Form.xml` | 13 044 | Выгрузка сейчас формирует каркас формы и вложенные assets; загрузка использует только `Ext/Form/Module.bsl`. Полная структура формы не восстанавливается. |
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
| `SpreadsheetDocument` | 14 051 объектов, из них 14 046 с `Template.xml` | Pack-покрытие на `sfc` закрыто: `audit-spreadsheet-templates` упаковал 14 046 из 14 046. Semantic round-trip `XML -> blob -> XML -> blob -> XML` пока стабилен только для 638 из 14 046, поэтому этот блок еще нельзя считать полноценно совместимым. |
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
| `audit-spreadsheet-roundtrip D:\УХА\sfc` после добавления параллельного аудита | 14 046 packed, 13 105 extracted, 13 105 repacked, 638 matched, 12 334 different, 1 074 extract failures; 81.049 секунды release-прогона вместе с компиляцией | Паковать все SpreadsheetDocument уже можем, но полноценный стабильный цикл выгрузки/загрузки макета пока проходит только на небольшой доле. Основной фронт работ - устранить XML-нормализационные отличия `compare` и 941 отказ первичного `extract` плюс 133 отказа `extract-repacked`. |
| `cargo test` | 288 passed | Unit-покрытие текущего кода стабильно после изменений. |

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
| Формы | Существенно частично | `prepare_form_body_row` пакует только модуль формы из `Ext/Form/Module.bsl`, а не весь `Ext/Form.xml` |
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
| Forms | Существенно частично | `extract_form_body_xml` возвращает каркас `Form.xml`; часть вложенных item assets выгружается отдельно |

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
8. Корневые assets конфигурации. В `sfc` встречаются `AdditionalIndexes.xml`, `MobileClientSignature.bin`, `StandaloneConfigurationContent.bin`, `MainSectionCommandInterface.xml`, `ClientApplicationInterface.xml`, `HomePageWorkArea.xml`; текущий staging покрывает только часть конфигурационных assets (`Splash`, `ParentConfigurations`, `MainSectionPicture`).
9. Бинарные legacy bodies. В `sfc` встречаются `Form.bin`, `Module.bin`, `ObjectModule.bin`; текущий код ориентирован на XML/BSL-представление и не импортирует эти файлы как отдельный полноценный источник.

## Итоговая оценка

`ibcmd-rs` уже подходит как база для инструмента загрузки/выгрузки и покрывает много инфраструктурных форматов. Но на базе `D:\УХА\sfc` его нельзя считать полноценно готовым аналогом `ibcmd`.

Ориентировочная готовность:

| Направление | Оценка |
|---|---|
| Выгрузка XML-карточек объектов | Высокая, но требует mass diff |
| Загрузка XML-карточек объектов | Средняя/высокая, но зависит от base blob |
| Модули BSL | Высокая |
| Общие картинки | Высокая |
| Справка | Средняя/высокая |
| Роли | Средняя, нужен stress-test |
| Командный интерфейс | Средняя |
| Макеты не SpreadsheetDocument | Средняя/высокая |
| SpreadsheetDocument | Высокая для pack-аудита, низкая/средняя для полного semantic round-trip |
| Формы | Низкая для полного round-trip |
| Загрузка в новую пустую SQL-базу | Низкая/не доказана |

## Разбиение работ по агентам

| Направление | Ответственность | Ближайший критерий готовности |
|---|---|---|
| Агент загрузки XML -> SQL | `src/mssql.rs`, stage/load CLI, batch-и, dry-run prepare | Есть отчет по всем объектам `sfc`: сколько root XML выбрано, сколько body rows подготовлено, сколько файлов проигнорировано; batch row count покрыт тестом. |
| Агент выгрузки SQL -> XML | `src/mssql_dump.rs`, native `mssql-dump-config`, source layout writer | Есть structural diff между native dump и эталонным layout: missing/extra/different по типам файлов. |
| Агент макетов и assets | `src/module_blob.rs`, `src/source_audit.rs`, MOXCEL, CommonPictures, Template.bin | `audit-spreadsheet-templates` дает 14 046 / 14 046 packed; `audit-spreadsheet-roundtrip` должен вырасти с 638 / 14 046 matched до полного semantic round-trip. Следующий критерий - разбор `compare`-расхождений и stress-test `BinaryData`/`AddIn`. |
| Агент форм | compile/decompile `Ext/Form.xml`, form item assets | Вместо каркаса формы выгружается и загружается реальная структура хотя бы для малого набора форм, затем расширение на `sfc`. |
| Агент интеграции | test harness, SQL clone, сравнение с `ibcmd` | Есть сценарий `ibcmd load` vs `ibcmd-rs stage/load` на копии базы и post-compare по `_Config`/`ConfigSave`/source dump. |

Ближайший рациональный план работ:

1. Разобрать `audit-spreadsheet-roundtrip` для `SpreadsheetDocument`: сначала 941 первичный `extract` failure, затем 133 `extract-repacked`, затем массовые `compare`-расхождения XML.
2. Проверить и покрыть тестом batch accounting в `mssql-stage-source-objects`, особенно второй batch и stable rows.
3. Добавить dry-run prepare отчет для загрузки всех исходников `sfc` без записи в SQL.
4. Начать отдельную ветку по полноценному `Form.xml` round-trip: сначала анализ реальной структуры, затем выгрузка, затем упаковка обратно.
5. Добавить native structural diff `mssql-dump-config -> source layout -> compare with sfc`.
6. Добавить round-trip tests для `BinaryData`, `AddIn`, всех форматов `CommonPictures`, ролей и command interface.
7. Добавить интеграционный harness `dump -> load -> dump -> diff` на небольшой базе, затем на подмножестве `sfc`.
8. После этого замерять скорость против `ibcmd`: до закрытия форм и end-to-end load такой замер будет показывать скорость неполного сценария, а не честную замену.
