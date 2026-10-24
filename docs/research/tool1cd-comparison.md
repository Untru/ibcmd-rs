# Сверка 1CD с tool1cd (#525)

## Вывод

tool1cd полезен как второй источник описаний физических структур 1CD.
Совпадают основные заголовки, blob-блоки, размеры полей и раскладка листа
индекса. Найдены различия writer, которые требуют отдельных native измерений:

- offset 12 заголовка объекта: третий компонент версии у tool1cd и количество
  страниц второго уровня при записи ibcmd-rs;
- left в листе: tool1cd сравнивает полный предыдущий ключ, ibcmd-rs ограничивает
  общий префикс его значимой частью;
- ширины битовых полей листа: вычисляемые у tool1cd, измеренный фиксированный
  вариант у writer ibcmd-rs;
- ConfigStorage имеет общий writefile, но запись через адаптер таблиц
  в этой ревизии возвращает false.

Эти различия не доказывают баг ibcmd-rs. Ограничения его writer не ослаблялись.
**Побайтовое равенство CONFIG-ключа с ICU 4.6 не установлено.**
Исходники выбора ICU прочитаны, но независимый runtime не запускался.

## Источники

Дата: 24.10.2026. Требования: [issue #525](https://github.com/Untru/ibcmd-rs/issues/525).

| Источник | Ревизия |
| --- | --- |
| ibcmd-rs | 6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4 |
| tool1cd | 7c4b91cb7a7bb48a32be00b9f32afe740dada6e1, коммит 07.04.2017 |
| Native evidence проекта | [1cd-format.md](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/openspec/changes/add-file-infobase-backend/evidence/1cd-format.md) |

В корне закреплённого tool1cd не найден LICENSE/COPYING/README.
В vendor ICU есть собственные license.html; это не установленная лицензия
всего проекта. Материалы tool1cd, MetaTree и его код в ibcmd-rs не переносились.
C++ читался в CP1251. Новые native измерения не выполнялись.

В таблице «совпадает» означает статическое совпадение указанного правила,
а не полную совместимость runtime. Все ссылки закреплены на SHA.

## Сопоставление

| Правило | ibcmd-rs | tool1cd | Вывод | Проверка (статическая) | Риск правки без нового замера |
| --- | --- | --- | --- | --- | --- |
| Заголовок файла и версия | [pager.rs:19–93](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/pager.rs#L19-L93) | [Class_1CD.h:36–46](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.h#L36-L46), [db_ver.h:5–13](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/db_ver.h#L5-L13) | Раскладка совпадает; ibcmd-rs поддерживает только 8.3.8, tool1cd перечисляет старые версии | Сверка полей заголовка | Неверное чтение файла/версии |
| Размер файла | [pager.rs:85–93](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/pager.rs#L85-L93) | [Class_1CD.cpp:7771–7775](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L7771-L7775) | ibcmd-rs проверяет вместимость заявленных страниц; tool1cd дополнительно отвергает некратную странице длину. Не доказательство бага | Сверка условий отказа | Принятие хвоста или лишний отказ |
| Объект FD1C, fat level, длина u64, список offset 24 | [object.rs:40–103](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/object.rs#L40-L103) | [Class_1CD.h:95–102](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.h#L95-L102), [cpp:537–585](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L537-L585) | Расположение и расчёт числа страниц по длине совпадают | Сверка offsets и расчёта need | Неверная адресация данных |
| Offset 12 объекта | [object.rs:220–259](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/object.rs#L220-L259) | [Class_1CD.h:78–83](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.h#L78-L83), [cpp:563–565](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L563-L565), [1733–1742](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L1733-L1742) | tool1cd читает version_3; обновление версии пишет только первые 8 байт. ibcmd-rs записывает table_count. Семантика offset 12 независимо не доказана | Сверка reader и writer | Потеря неизвестного header-поля |
| Переход FAT 0 → 1 | [object.rs:227–254](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/object.rs#L227-L254) | [cpp:1525–1573](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L1525-L1573) | Совпадает критерий вместимости прямого списка; вся стратегия resize не объявляется тождественной | Сверка порога и нового FAT | Порча FAT при resize |
| Blob: next u32, length u16, payload 250, блок 256 | [blob.rs:1–62](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/blob.rs#L1-L62) | [Class_1CD.h:454–459](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.h#L454-L459), [cpp:6624–6647](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L6624-L6647) | Раскладка и free-head блока 0 совпадают; повреждённые цепочки и старший бит length отдельно не дифференцировались | Сверка структуры и writer | Неверная цепочка или payload |
| Размеры NVC и N | [descr.rs:177–190](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/descr.rs#L177-L190), [config.rs:37–84](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/config.rs#L37-L84) | [cpp:3080–3096](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L3080-L3096) | NVC = 2+2×length, N = (digits+2)/2 согласованы; полное равенство кодеков не проверено | Сверка формул размеров | Смещение границ записи |
| Заголовок листа 30 байт, left/right и хвосты ключей | [index.rs:204–250](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/index.rs#L204-L250) | [Class_1CD.h:314–327](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.h#L314-L327), [cpp:2290–2323](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L2290-L2323) | Раскладка совпадает; next/prev в 8.3.8 задают номера страниц | Сверка unpack и offsets | Повреждение индекса |
| Граница left | [index.rs:294–305](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/index.rs#L294-L305) | [cpp:2376–2400](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L2376-L2400) | Различается: ibcmd-rs ограничивает left значением key_len−previous_right; сохранять измеренное ограничение | Сверка границ pack | Повреждение индекса при mutation |
| Битовые ширины и запас листа | [index.rs:38–42](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/index.rs#L38-L42), [287–317](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/index.rs#L287-L317) | [cpp:2421–2450](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L2421-L2450), [2813–2849](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L2813-L2849) | ibcmd-rs writer использует 32/16/16 и 8 байт с запасом; tool1cd вычисляет ширины и делит при переполнении. Не эквивалентные алгоритмы | Сверка pack/split | Переполнение при удалении/вставке |
| Составной CONFIG-ключ | [index.rs:120–142](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/index.rs#L120-L142) | [cpp:2469–2480](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L2469-L2480), [3965–3993](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L3965-L3993) | Специальный ключ 392 байта против общей сборки по полям с zero padding. Равенство конкретных ключей не измерено | Сверка цепочки сборки ключа | Коллизия или неверный порядок |
| ICU и CI/CS | [index.rs:44–117](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/index.rs#L44-L117) | [ICU.cpp:70–103](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/ICU.cpp#L70-L103), [115–138](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/ICU.cpp#L115-L138), [CI/CS](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L4674-L4675) | Для 8.3.8 выбран ICU 4.6, CI использует SECONDARY. Формулы ibcmd-rs для en_US_POSIX/ru_RU этим не доказаны равными ICU | Сверка выбора версии/strength | Расширение недоказанного домена записи |
| Общий интерфейс файлов | [ConfigRow](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/config.rs#L24-L35), [CF layer](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-cf/src/lib.rs#L1-L7) | [ConfigStorage.h:27–71](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/ConfigStorage.h#L27-L71), [104–153](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/ConfigStorage.h#L104-L153) | Полезный пример границы носителя; profile/provenance/locking ibcmd-rs требуют отдельного контракта | Сверка границ интерфейса | Потеря profile/provenance гарантий |
| Запись таблиц через ConfigStorage | [filedb writer](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/lib.rs#L6-L10) | [ConfigStorage.cpp:475–478](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/ConfigStorage.cpp#L475-L478) | В ibcmd-rs есть writer; ConfigStorageTable::writefile tool1cd возвращает false | Чтение тела writefile | Ложное обещание общей записи |
| Recovery и depot | Весь ibcmd-rs на наличие аналогов не инвентаризировался | [Class_1CD.h:873–939](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.h#L873-L939) | Есть точки входа для следующего исследования; отсутствие аналогов в ibcmd-rs не утверждается | Инвентарь точек входа | Необоснованное расширение возможностей |

## Исполненные проверки

Windows x64, Rust 1.95.0. tool1cd не компилировался. В закреплённом дереве
tool1cd не найдено DLL; одних ICU headers/export-файлов недостаточно для
исполнения ICU 4.6.

Существующий набор `cargo test --locked -p ibcmd-filedb`:
**29 unit + 11 integration, 0 failed, 0 ignored**, doc-tests: 0.
Покрыты, среди прочего:
- `index::tests::parts_of_one_row_share_only_the_significant_prefix`;
- `index::tests::russian_keys_match_native_ru_ru_keys`;
- `blob::tests::broken_chains_fail_closed`;
- `object::budget_tests::fat_vector_budget_is_enforced_on_every_platform`;
- `native_files_read_and_every_index_matches_its_records`;
- `objects_grow_to_second_level_tables_and_shrink_onto_free_chains`;
- `an_infobase_in_an_unmeasured_locale_is_read_but_not_written`;
- `a_corrupted_object_length_is_refused_without_allocating_it`.

Это текущие тесты и сохранённые фикстуры, не новый запуск платформы.

Первые прогоны не скрыты: системный TEMP дал `AlreadyExists` в fixture
directory (28/1), sandbox — `Access denied` на junction/custody fixtures
(25/4). Причина первого collision не установлена. Полный успешный прогон
использовал отдельные TEMP/TMP и разрешённое исполнение. Тесты не отключались
и не сериализовались; неравные условия не устанавливают дефект writer.

### Границы публичного API

[Исследовательский probe](../../tools/research/filedb-index-boundaries.rs)
вызывает публичный API без повторения формулы ключа.
**26 групп assertions, 0 failures**:
- обе локали, длины 0/1/43/44: отказ/успех/успех/отказ;
- все одиночные допущенные ASCII дают ключ 392 байта;
- пробел, кириллица, é, NUL отвергаются;
- части 0 и 1 различаются, максимальный u32 part принимается;
- ru_RU не различает DBNames/dbnames, en_US_POSIX различает;
- de_DE не выбирает writer collation.

Воспроизведение в PowerShell из корня checkout с Rust 1.95.0:

```powershell
cargo test --locked -p ibcmd-filedb
cargo build --locked -p ibcmd-filedb
rustc --edition=2024 tools/research/filedb-index-boundaries.rs --extern ibcmd_filedb=target/debug/libibcmd_filedb.rlib -L dependency=target/debug/deps -o target/issue525-index-boundaries.exe
./target/issue525-index-boundaries.exe
```

Команды предполагают стандартный target; при CARGO_TARGET_DIR заменить оба
target пути. Windows fixtures требуют доступного отдельного TEMP/TMP и
разрешения создать junction/custody fixtures.

### Матрица пяти классов входа

| Класс | Проверено сейчас | Чего не хватает для независимого доказательства |
| --- | --- | --- |
| Пустые/усечённые данные | Тест broken_chains_fail_closed и отказ на повреждённую длину объекта; пустое имя в probe | Одинаковые усечённые файлы в tool1cd/native и классификация каждого отказа |
| Границы страницы | rebuilt_trees_read_back_in_order, objects_grow_to_second_level_tables_and_shrink_onto_free_chains | Native resize/mutation на границах 4/8/64 KiB и FAT 0/1 |
| Blob-цепочки | values_round_trip_and_free_tail_is_chained и broken_chains_fail_closed | Независимое чтение пустых, циклических, усечённых цепочек и free-list на одинаковых файлах |
| Длины имён | Обе локали, 0/1/43/44, отказ недопущенных символов в probe | ICU 4.6, полные ключи на 1/42/43 и совпадение native порядка |
| Неизмеренная локаль | de_DE не выбирает writer; существующий тест read-but-not-write | Native/ICU evidence перед любым расширением списка разрешённых локалей |

Табличные риски — оценка последствий преждевременной правки, а не перечень
доказанных дефектов. Здесь нет заявленного runtime differential tool1cd/native.

## Как проверить равенство ICU

Ответ пока: **не установлено**. Нужен независимый ICU 4.6 с закреплёнными
build/data/version/hash, параметрами locale/SECONDARY/CI и обработкой
trailing spaces как у tool1cd. Сравнивать весь 392-байтовый CONFIG-ключ:
ICU sort key, PARTNO, padding.

Матрица: все разрешённые одиночные символы, пары, технические имена,
длины 1/42/43, case pairs, разделители, PARTNO 0/1/u32::MAX, обе локали
и отдельно неизмеренная. Отказы 0/44 фиксируются отдельно от возможностей ICU.
Затем native mutation на disposable копии с проверкой индекса и данных.
Совпадение коротких ASCII-имён не разрешает Unicode, длинные имена или новую
локаль; современный ICU не заменяет исторический ICU 4.6.

## Кандидаты в отдельные задачи

| Приоритет | Вопрос | Следующее доказательство | Риск преждевременной правки |
| --- | --- | --- | --- |
| P1 | Семантика offset 12 объекта | Exact-build native create/grow/shrink/rewrite, FAT 0/1, все три u32 до/после | Нарушение версии или неизвестного header-поля |
| P1 | Допустимые left и битовые ширины | Clean-room ключи на границе нулевого хвоста, native delete/apply | Повреждение индекса |
| P2 | Равенство CONFIG-ключа ICU 4.6 | Полные differential отчёты по допущенному домену | Коллизии/неверный порядок и запись |
| P2 | Trailing bytes и page count | Контролируемое усечение/добавление и native отказ/допуск | Принятие неполного состояния или лишний отказ |
| P3 | Общий readfile-адаптер CF/filedb | Инвентарь реального дублирования с profile/provenance/locking | Потеря гарантий записи ради формального интерфейса |

Это предложения; новые issues не создавались. Расхождение с tool1cd само
по себе не квалифицируется как подтверждённый баг ibcmd-rs.

## Остаток

- Независимый runtime tool1cd/ICU 4.6 и differential матрица.
- Новые native измерения header/index изменений.
- APIcfBase/V8-container и Parse_tree/NodeTypes/MetaTree определены как
  следующие области, но до байтового оракула не разобраны.
- Hosted merge gate, release build, native micro-corpus, SBOM и release audit
  не выполнялись. Отдельный `cargo xtask check` не запускался.

## Общие локальные проверки

`cargo xtask ci --keep-going` в исходном окружении: 15 passed / 4 failed.
Python EDT oracle воспроизвёл тот же отказ на неизменённой базе.
Контролируемая проверка показала необходимость UTF-8 для чтения JSON и
сгенерированного C++ include с кириллицей в пути. Настройки окружения
изменены локально, исходники и зависимости проекта не менялись.

Повтор с `PYTHONUTF8=1` и MSVC `/utf-8`: **18 passed / 1 failed**.
Единственное падение — компоновка root integration suites:
Windows `0xc000009a` и `os error 1450` (недостаточно ресурсов).

После ограничения `CARGO_BUILD_JOBS=1` выполнена точная команда упавшего шага:

```powershell
cargo test --locked -p ibcmd-rs --test '*' --no-default-features
```

Результат отдельного повтора: **PASS, exit 0**.
Таким образом проверены все 19 шагов с учётом повтора; единый успешный
запуск `cargo xtask ci` не заявляется. Уже успешные 18 шагов не повторялись
после изменения только параллелизма сборки. Для перевода будущего PR
из draft всё равно нужен полный gate на его окончательной ревизии.

Issue #525 остаётся открытым; будущий PR должен использовать `Refs #525`.
