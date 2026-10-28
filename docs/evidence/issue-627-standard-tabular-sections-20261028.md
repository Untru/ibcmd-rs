# Отсутствующие стандартные табличные части планов (#627)

## Правило и входы

Общий экспорт метаданных отличает отсутствующую коллекцию {0} от
присутствующей {1,body}. Отсутствующая коллекция не пишет XML-свойство
StandardTabularSections. Заполненные секции сохраняются; некорректный
внешний контейнер отклоняется. Новый формат или маршрут не вводится.

Основа: 0c988ca869c6a718f1ab7111e29e370f6fe76464.
Исправление: 67224e3da6c13317e152c8733ddf2a85929a3eb7.
Закреплённый публичный 1c-fixtures: 8d138b896f462c3053d16076cbabf0ffe5030c1c.
Взяты существующие сгенерированные наборы chartsofaccounts,
chartsofcalculationtypes, calculationregisters для 8.3.27.2342 (XML 2.20)
и 8.5.1.1150 (XML 2.21). Входы корпуса и combined не менялись.

## Failing first

    cargo test --locked -p ibcmd-rs --test integration coverage_corpus::native_chart_trees_pass_import_self_check --no-default-features

До исправления: FAIL, 8.3.27.2342/chartsofaccounts, objects 14, identical 0,
different 14, failed 0; первое лишнее свойство — StandardTabularSections.
Тест компилирует нативный XML в записи, затем сравнивает полный XML планов,
выгруженный общим экспортёром, с нативным эталоном.

## Свежая проверка на macOS

macOS 26.7.1 arm64; нативная 1С **8.3.27.2214**, XML 2.20.
SHA-256 нативного ibcmd:
1a35bba7883a7fdf9a8678392e4b1520df452995ce005ff225f94594e70cfe25.
Обе стороны с LANG=ru_RU.UTF-8; каждый нативный вызов со своим --data.
Все базы одноразовые, созданы из публичных синтетических CF.
Для каждого набора декодирован coverage/8.3.27.2342/SET/configuration.cf.b64:

    ibcmd infobase create --db-path=DB --create-database --locale=ru_RU --load=INPUT.cf --apply --force --data=UNIQUE
    ibcmd infobase config export --db-path=DB NATIVE --data=UNIQUE
    ibcmd infobase config save --db-path=DB NATIVE.cf --data=UNIQUE
    ibcmd infobase create --db-path=EMPTY --create-database --locale=ru_RU --data=UNIQUE
    ibcmd-rs infobase config export --db-path=DB --platform=8.3.27 OURS
    # Скопировать EMPTY в отдельную IMPORT_DB перед каждым импортом.
    ibcmd-rs infobase config import --db-path=IMPORT_DB --platform=8.3.27 NATIVE

До исправления: debug-бинарник Rust 1.95.0 из основы с новым тестом,
SHA-256 2e79dfc61c121defb8eabd02f76610a8a1d4eae8e8b7eebaae3adc2ee90978c9.
Экспорты: код 0; импорты: код 255 с диагностикой
«в собранной конфигурации есть …/Properties/StandardTabularSections = «»,
в дереве его нет». Хеши всей 1Cv8.1CD после отказов не изменились.

После исправления пробный debug-прогон:

| Набор | Файлов | Отличий до → после | Импорт до → после |
| --- | ---: | ---: | --- |
| chartsofaccounts | 17 | 14 → 0 | 255 → 0 |
| chartsofcalculationtypes | 14 | 11 → 1 | 255 → 0 |
| calculationregisters | 9 | 1 → 0 | 255 → 0 |

Сравнение полное, побайтовое, без нормализации, включая ConfigDumpInfo.xml.
После всех экспортов SHA-256 всей 1Cv8.1CD совпадает с хешем до чтения.
Успешные импорты изменили свои отдельные копии пустых баз.

Оставшееся расхождение ChartsOfCalculationTypes/CovUseStandardCommands_false.xml:
нативные UseStandardCommands=false, EditType=InDialog выгружаются как
true, InList. Те же два неверных значения есть в выгрузке до исправления.
Это открытая [#630](https://github.com/Untru/ibcmd-rs/issues/630), связанная
с #545/#574. Её исправление в этот PR не входит. Требование полного
совпадения всех трёх наборов поэтому ещё не выполнено.

Хеш дерева: сортировка относительных путей как строк; для каждого файла в
SHA-256 подаётся UTF-8: path + NUL + lowercase_file_sha256 + NUL +
decimal_size + LF.

| Набор | SHA-256 полного нативного дерева |
| --- | --- |
| chartsofaccounts | 243409be252288ef24cb716bf30fc250841c674d0419a56b0221f7aaa8d5f26c |
| chartsofcalculationtypes | f2b50ea11d8997716051a6627257ef8b1b51a5ab50744e689e57e44f5f592544 |
| calculationregisters | 9d8a5ca591104848a903685b5f4c9f17e9573d8a7f182ef0e37ff8133208262d |

Свежие 8.5 и MSSQL здесь не запускались. Проверка XML 2.21 использует
закреплённые нативные фикстуры; она не заменяет живой прогон 8.5 или SQL.
Нативные деревья, базы и CF в репозиторий не включаются.

## Проверки проекта

Rust 1.95.0, PowerShell 7.6.8, Python 3.14.7:

- Граничный тест standard_tabular_section_tests: 1 PASS, 0 FAIL.
- cargo xtask check: 4 PASS, включая coverage_corpus.
- cargo xtask ci: 29 PASS, без SKIP; 2031 тест компонентов и 551
  интеграционный тест прошли без ошибок. Прежние ignored сохранены.
- Проверки закреплённых входов и актуальности combined прошли;
  baseline, корпус и gitlink не менялись.
- Проверка отсутствия оставшихся временных каталогов: PASS.

Релизная сборка, native micro-corpus, SBOM и release audit — отдельные
этапы GitHub CI, локально не запускались.
После коммита исправления обновлён только поясняющий комментарий
в slot_evidence_tests.rs; поведение и утверждения этого теста не менялись.

## Повтор product-сборкой

cargo build --locked --profile iter: PASS, 12m32s.
ibcmd-rs 0.4.1, Rust 1.95.0; дерево 4a0ac555
(production Rust не отличается от коммита исправления 67224e3d).
SHA-256 бинарника:
f0c5be75099aa267cb7c47cde5208b9767e2752467638cf33455bc142753197b.

Перед запуском проверены версия, хеш бинарника и отсутствие изменений
Rust-исходников. На тех же трёх нативных базах бинарник iter повторил
результаты таблицы: 39/40 файлов равны, оставшийся файл — #630;
во всех трёх наборах StandardTabularSections отсутствует как штатно.
Три импорта в новые копии пустой базы: код 0, без --no-verify.
Все три исходные базы после чтения сохранили SHA-256 всей 1Cv8.1CD.

Дополнительно штатный ibcmd 8.3.27.2214 успешно выгрузил конфигурации
из трёх баз после импорта. Сравнение с входом вне ConfigDumpInfo.xml:
0 / 1 / 0 различий, в оставшемся файле те же два флага #630.
ConfigDumpInfo.xml здесь исключён, поскольку компиляция создаёт новые
версии конфигурации; исходное сравнение база → XML выше включает его.

Полный внутренний набор CI: 3552 PASS, 0 FAIL, 15 прежних ignored.
