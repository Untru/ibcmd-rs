# Полный побайтовый аудит УТ: `33a9baf`, 24 июля 2026 года

Статус: **промежуточный, не PASS**. Это воспроизводимый полный прогон для УТ;
он не подтверждает побайтовое соответствие и не является основанием для
промышленной эксплуатации. Полный повторяемый прогон БСП на этом шаге ещё не
выполнялся.

## Контур и воспроизводимость

| Параметр | Значение |
|---|---|
| Run ID | `ut_full_33a9baf_20260724_a` |
| Каталог артефактов | `E:\ibcmd_lab\parity\ut_ibcmd_ut_full_33a9baf_20260724_a` |
| База | `ut_ibcmd` на `localhost` |
| Кандидат | `33a9baf89a58174a16b4ad2a2aad01c151edcb36` |
| Нативный `ibcmd` | `8.3.27.1989` |
| XML / source version | `2.20 / 2.20` |
| Состояние манифеста | `passed` |

Манифест фиксирует чистое рабочее дерево кандидата до запуска. Отпечаток
`Config` + `ConfigSave` до и после запуска одинаков: `40 625` строк,
SHA-256 `a70a53f2ceb6063a7a461fe5515144661b0e2958768919716de318eb442ce20f`.
Следовательно, измерение не опирается на изменение данных базы во время
выгрузки.

## Результат raw-diff

| Показатель | Файлов |
|---|---:|
| Всего | 49 623 |
| Побайтово совпало | 45 753 |
| Различается | 3 795 |
| Только в native | 75 |
| Только в candidate | 0 |
| Не совпало всего | 3 870 |
| Точное совпадение | 92,2012% |

По сравнению с каноническим полным снимком `87c8dc3`: совпавших файлов
`45 316 → 45 753` (`+437`), различающихся `3 974 → 3 795` (`−179`),
отсутствующих у кандидата `333 → 75` (`−258`).

| Класс текущих расхождений | Файлов |
|---|---:|
| `Form.xml` | 2 558 |
| `Template.xml` | 1 163 |
| Корневые XML метаданных: different | 70 |
| Корневые XML метаданных: только в native | 75 |
| Прочие файлы | 3 |
| `Configuration.xml` | 1 |

Отсутствующие корневые XML распределены так: `Catalogs` — 32, `Documents` —
24, `ChartsOfCharacteristicTypes` — 9, `BusinessProcesses` — 7,
`DataProcessors` — 1, `FilterCriteria` — 1, `Tasks` — 1.

## Ведущие обезличенные сигнатуры

Сигнатуры сформированы из `signatures.json`; XML, содержимое базы и UUID в
документ не переносятся.

| Kind | Путь | Событий | Файлов |
|---|---|---:|---:|
| template | `document/rowsItem/row/c/c/f` | 1 222 | 28 |
| template | `document/format/borderColor` | 767 | 29 |
| template | `document/columns/columnsItem/column/formatIndex` | 762 | 4 |
| template | `document/format/backColor` | 655 | 49 |
| template | `document/format/containsValue` | 293 | 50 |
| form | `…/InputField/ChoiceParameterLinks/Link` | 243 | 47 |

## Артефакты

| Файл | SHA-256 |
|---|---|
| `raw-diff.json` | `52f40da5bda152f5f57ac9254b343b7c9212e7bd08dcccb97c4a26c17e541031` |
| `signatures.json` | `0e6ef7c2a4bc868585244ab8d6f8420e84ec80e9882ad62652992eecf4432b9d` |
| `matrix.json` | `2155c2ab1d275346511f8cacc59fe2b39eaca4835701e4c54eead730ea2df5ee` |
| `matrix.md` | `b32e5828678e69dcec09a71a8fca707c0fe65ec6c9bd87393b9230060a7b72f4` |

Порядок выполнения, аргументы всех шагов и SHA-256 исполняемых инструментов
содержатся в `parity-manifest.json` в указанном каталоге прогона. Все шесть
шагов завершились со статусом `passed`: native export, candidate export,
сборка source-layout, raw-diff, сигнатуры и parity matrix.
