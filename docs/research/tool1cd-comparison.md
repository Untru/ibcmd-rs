# Сверка 1CD с tool1cd (#525)

## Вывод

tool1cd полезен как второй источник описаний физических структур 1CD.
Совпадают основные заголовки, blob-блоки, размеры полей и раскладка листа
индекса. Найдены различия writer; часть проверена отдельными native измерениями:

- offset 12 заголовка объекта: третий компонент версии у tool1cd и количество
  страниц второго уровня при записи ibcmd-rs; native 8.5.1.1529 записал другие
  значения, назначение поля пока не установлено;
- left в листе: tool1cd сравнивает полный предыдущий ключ, ibcmd-rs ограничивает
  общий префикс его значимой частью;
- ширины битовых полей листа: вычисляемые у tool1cd, измеренный фиксированный
  вариант у writer ibcmd-rs;
- ConfigStorage имеет общий writefile, но запись через адаптер таблиц
  в этой ревизии возвращает false.

Эти различия не доказывают баг ibcmd-rs. Ограничения его writer не ослаблялись.
**30 810 полных CONFIG-ключей совпали с независимым ICU 4.6** на описанной
ниже матрице: 5 135 имён × две локали × три PARTNO. Это конечная выборка,
не доказательство для всех возможных имён. tool1cd как приложение не запускался.
[Отдельная native матрица заголовков](filedb-native-headers-20261024.md)
содержит 24 состояния и 456 заголовков; Rust mutation в ней не выполнялась.
Домен записи не расширялся.

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
C++ читался в CP1251. Первоначальная фаза была статической;
последующее native измерение заголовков описано отдельно по ссылке выше.

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
| Составной CONFIG-ключ | [index.rs:120–142](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/index.rs#L120-L142) | [cpp:2469–2480](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L2469-L2480), [3965–3993](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L3965-L3993) | Специальный ключ 392 байта против общей сборки по полям с zero padding. 30 810 ключей совпали с ICU 4.6 + N(10) + padding; приложение tool1cd не исполнялось | Сверка цепочки сборки ключа | Коллизия или неверный порядок |
| ICU и CI/CS | [index.rs:44–117](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/index.rs#L44-L117) | [ICU.cpp:70–103](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/ICU.cpp#L70-L103), [115–138](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/ICU.cpp#L115-L138), [CI/CS](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L4674-L4675) | Для 8.3.8 выбран ICU 4.6, CI использует SECONDARY. Независимый ICU 4.6 подтвердил матрицу двух локалей ниже, не весь допустимый домен | Сверка выбора версии/strength | Расширение недоказанного домена записи |
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
| Длины имён | Обе локали, 0/1/43/44, отказ недопущенных символов в probe | ICU 4.6: полные ключи на 1/42/43 совпали; новый native порядок не измерен |
| Неизмеренная локаль | de_DE не выбирает writer; существующий тест read-but-not-write | Native/ICU evidence перед любым расширением списка разрешённых локалей |

Табличные риски — оценка последствий преждевременной правки, а не перечень
доказанных дефектов. Runtime differential выполнен с ICU 4.6; tool1cd как
приложение не запускался. Native заголовки измерены отдельно; native индексный
порядок и tool1cd differential этой матрицей не подтверждены.

## Независимое сравнение с ICU 4.6

Исполнен официальный **ICU 4.6.0.0, Windows x64, MSVC10**:
[исторический выпуск Unicode](https://icu.unicode.org/download/46),
[архив Win64](https://sourceforge.net/projects/icu/files/ICU4C/4.6/icu4c-4_6-Win64-msvc10.zip/download).
SHA-256 скачанного архива:
`c64bd3b9fade2c7cefbb1c8c16f765febda8b0f4de771d5f50d88a150d1cf41b`.
Это локально вычисленный идентификатор, не проверенная цифровая подпись.
Архив содержит license.html с ICU permission notice (IBM, 1995–2010).
Библиотеки, данные ICU и материалы tool1cd в репозиторий не добавлены.

[Результат](filedb-icu46-result.json) сохраняет хеши icuin46/icuuc46/icudt46,
версии runtime/UCA/collation, фактические локали, восемь атрибутов collator,
хеш входного потока и хеш ответа Rust. Python 3.12.14 x64 вызывает
`ucol_open_46`, `ucol_setStrength_46(SECONDARY)` и `ucol_getSortKey_46`
через ctypes; ICU 4.6 сообщает UCA 6.0.0.0. Для ru_RU получен допустимый
`U_USING_FALLBACK_WARNING` (-128), actual locale — ru, valid — ru_RU;
для en_US_POSIX status 0 и actual/valid en_US_POSIX. Этот fallback сохранён,
а не представлен как точная локаль без предупреждений.

Матрица генерируется clean-room скриптом:

- все 66 разрешённых одиночных символов и все 4 356 упорядоченных пар;
- каждый символ, повторённый 42/43 раза; последовательности длин 1…43;
- технические имена, UUID, case pairs, разделители и 512 воспроизводимых
  последовательностей из SHA-512; после удаления повторов — 5 135 имён;
- en_US_POSIX и ru_RU, PARTNO 0/1/4294967295: **30 810 сравнений всех
  392 байт, 0 расхождений**;
- отдельно **17 отказов**: пустое имя, длина 44, пробелы, кириллица, é,
  NUL и de_DE. ICU способен вычислить ключ de_DE, но Rust writer его отвергает.

Rust bridge вызывает публичный `config_key`; формулы весов не повторяются
в Python. Ожидаемый ключ — реальный ICU sort key с завершающим NUL,
положительное N(10) для PARTNO и нулевой хвост до 392 байт. Числовая упаковка
в helper проверена тремя явными byte anchors. tool1cd для положительного
numeric копирует байты поля ([Class_1CD.cpp:3826–3899](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Class_1CD.cpp#L3826-L3899)).
Это сравнение с ICU плюс независимая сборка составного ключа; весь tool1cd
writer не исполнялся. TrimSpacesRight не проверен на равенство: допустимые
имена не содержат пробелов, а Rust их отвергает.

Воспроизведение в PowerShell из checkout; `$icuRoot` — распакованный каталог
icu с bin64, `$python` — Python x64 с доступным MSVC10 runtime:

```powershell
cargo build --locked -p ibcmd-filedb
rustc --edition=2024 tools/research/filedb-config-key-probe.rs --extern ibcmd_filedb=target/debug/libibcmd_filedb.rlib -L dependency=target/debug/deps -o target/issue525-config-key-probe.exe
& $python tools/research/compare-filedb-icu46.py --icu-root $icuRoot --probe target/issue525-config-key-probe.exe --output target/issue525-icu46.json
```

Helper отказывается запускаться с другим хешем DLL, версией ICU, архитектурой
или заданным ICU_DATA. Несовпадение ключа/отказа даёт ненулевой exit code.
Локальные negative controls обнаружили намеренную порчу одного ключа
(ровно одно расхождение) и отклонили неверный хеш данных ICU.

Вывод ограничен этой матрицей. Равенство выборки не разрешает Unicode,
длину >43 или новую локаль и не заменяет native delete/apply с проверкой
индекса. Порядок ключей и мутации в новом запуске платформы не измерялись.

## Дополнительные локали: граница подстановки профиля

[Измеритель](../../tools/research/inspect-filedb-icu46-locales.py) использует
тот же закреплённый ICU 4.6, 5 135 имён и PARTNO 0/1/4294967295.
Сравниваются все 392 байта. Существующие Rust-профили рассматриваются только
как **кандидатные подстановки**, не как реализация новой локали.
[Результат](filedb-icu46-locales-result.json) сохраняет метаданные collator,
хеши DLL/helpers/probe, входов/выходов и потоков ожидаемых полных ключей.

| Запрошенная ICU-локаль | Valid / actual | Совпало с ru_RU | Отличается от ru_RU | Совпало с en_US_POSIX |
| --- | --- | ---: | ---: | ---: |
| en_US | en_US / root | 15 405 | 0 | 0 |
| de_DE | de_DE / de | 15 405 | 0 | 0 |
| tr_TR | tr_TR / tr | 14 517 | 888 | 0 |
| sv_SE | sv_SE / sv | 15 405 | 0 | 0 |

Всего **123 240 диагностических сравнений**: четыре локали × два профиля ×
15 405 ключей. Для POSIX каждая строка означает 15 405 отличий, а не отсутствие
проверки. При SECONDARY ключ `I` у tr_TR — `3651010500`, ключ `i` —
`37010500`; у ru_RU оба равны `37010500`. Даже внутри разрешённого ASCII-домена
нельзя универсально подставлять ru_RU вместо неизвестной локали. Совпадения
en_US/de_DE/sv_SE ограничены выборкой: native индексы, все Unicode-имена,
другие длины и разрешение записи ими не подтверждены.

Все 61 620 запросов четырёх неподдерживаемых локалей к Rust и девять запросов
`zz_ZZ` завершились `REFUSED`: **61 629 отказов**. Контроль zz_ZZ у ICU
вернул warning -128, valid ru_RU / actual ru на этом компьютере; fallback
к локали окружения не считается пятой проверенной локалью. У en_US status -127,
у остальных трёх -128; предупреждения сохранены. Default fallback и его
метаданные могут зависеть от компьютера.

Исходная supported-матрица 30 810/17 повторена без расхождений. Второй ответ
probe также сверяется с ICU на всех 30 810 кандидатных ключах: проверки
первого процесса и длины второго ответа недостаточно. RED→GREEN контроль
намеренной порчи второго ответа подтвердил исправление этого analysis helper.
[Пять live-контролей](../../tools/research/test-filedb-icu46-locales.py)
отвергают испорченный ключ, снятый отказ, усечённый ответ, подменённую valid
locale и неверный хеш данных ICU. Production-код и разрешённые локали не менялись.

После сборки прежнего Rust probe воспроизведение из checkout:

```powershell
& $python -B tools/research/inspect-filedb-icu46-locales.py --icu-root $icuRoot --probe target/issue525-config-key-probe.exe --output target/issue525-icu46-locales.json
& $python -B tools/research/test-filedb-icu46-locales.py --icu-root $icuRoot --probe target/issue525-config-key-probe.exe
```

Это ICU-only эксперимент; tool1cd executable и native инфобаза здесь не
запускались. Самостоятельное построение составного ключа не заменяет проверку
порядка/мутаций индекса на точной сборке платформы.

Для этого дополнения отдельно выполнены `cargo xtask check` — **3 passed**
и полный `cargo xtask ci --keep-going` — **19 passed, exit 0, без SKIP**
в одном запуске. Независимый ревьюер повторил матрицу и пять live-контролей;
его JSON совпал побайтово. Hosted/release-проверки этим не подтверждаются.

## Контейнер и текстовое дерево: статическая сверка

Здесь сравниваются конкретные участки, без запуска C++Builder, переносов
MetaTree и заявления полной эквивалентности парсеров.

| Правило | ibcmd-rs (та же закреплённая ревизия) | tool1cd (та же закреплённая ревизия) | Вывод / метод | Риск преждевременного переноса |
| --- | --- | --- | --- | --- |
| Контейнер с 32-битными адресами | [format15.rs:15–37](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-v8/src/format15.rs#L15-L37), [block.rs:9–10](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-v8/src/block.rs#L9-L10) | [APIcfBase.h:34–54](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/APIcfBase.h#L34-L54), [cpp:56–93](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/APIcfBase.cpp#L56-L93) | Совпадают 16-байтовый заголовок каталога, 12-байтовый FAT item, 31-байтовый текстовый заголовок блока и sentinel 0x7fffffff; статическая сверка размеров | Смешение ревизий контейнера |
| Продолжение цепочки | [block.rs:240–328](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-v8/src/block.rs#L240-L328) | [APIcfBase.cpp:56–93](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/APIcfBase.cpp#L56-L93) | В выбранном read_block tool1cd нет аналога seen/range/size проверок Rust; неверный start возвращает пустой stream. Сверка ветвей, не общий аудит безопасности tool1cd | Ослабление fail-closed поведения |
| 64-битный контейнер | [format16.rs:12–37](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-v8/src/format16.rs#L12-L37) | [APIcfBase.h:42–54](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/APIcfBase.h#L42-L54) | У Rust отдельно u64 addresses, header 20/FAT item 24; выбранные структуры tool1cd с int этого не подтверждают. Статическая граница применимости | Ошибочная экстраполяция старого API на Format16 |
| Сжатие payload отдельно от блока | [payload.rs:1–24](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-cf/src/payload.rs#L1-L24) | [APIcfBase.cpp:1124–1143](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/APIcfBase.cpp#L1124-L1143) | Оба разделяют цепочку блока и декодирование, но Rust явно различает Stored/RawDeflate и задаёт budgets; zipped bool tool1cd не доказывает равенство политик. Чтение API | Потеря лимитов/профиля при общем адаптере |
| Типы дерева | [metadata_model/brace.rs:10–18](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/src/metadata_model/brace.rs#L10-L18) | [NodeTypes.h:8–20](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/NodeTypes.h#L8-L20), [Parse_tree.cpp:14–20](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Parse_tree.cpp#L14-L20), [235–246](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Parse_tree.cpp#L235-L246) | Выбранное Rust дерево хранит List/Str/Atom; tool1cd классифицирует number/exp/GUID/#base64/bare binary/link/#data через regex. Это дополнительная классификация, не доказанная семантика всех Atom | Ошибочная типизация непрозрачных токенов |
| Кавычки и вывод списков | [brace.rs:105–132](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/src/metadata_model/brace.rs#L105-L132) | [Parse_tree.cpp:176–217](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/Parse_tree.cpp#L176-L217) | В обоих writer кавычка удваивается; правило CRLF совпадает для непустых списков. Пустой nd_list tool1cd выводит через value, Rust пишет {}. Сверка ветвей writer, не round-trip differential | Непроверенное равенство BOM/пустого дерева/ошибок |

Практический результат: сохранить границы block/payload/profile в Rust;
не заменять их общим интерфейсом только из-за похожего API. Для текстового
дерева полезна отдельная clean-room матрица пустых элементов, escaped quotes,
экспонент, binary/link и отказов. Копировать MetaTree для этой матрицы
не требуется. Этот обзор не инвентаризирует все парсеры ibcmd-rs.

## Кандидаты в отдельные задачи

| Приоритет | Вопрос | Следующее доказательство | Риск преждевременной правки |
| --- | --- | --- | --- |
| P1 | Семантика offset 12 объекта | Native create/grow/logical shrink/rewrite измерены: 24 состояния FAT0/FAT1, три u32. Значение не равно числу FAT-страниц или числу минус один; нужна проверка Rust mutation/native-read и правила сохранения поля | Нарушение версии или неизвестного header-поля |
| P1 | Допустимые left и битовые ширины | Clean-room ключи на границе нулевого хвоста, native delete/apply | Повреждение индекса |
| P2 | Расширение матрицы CONFIG-ключа | Текущая выборка совпала с ICU 4.6; для расширения нужны новые полные ключи и native mutation | Коллизии/неверный порядок и запись |
| P2 | Trailing bytes и page count | Контролируемое усечение/добавление и native отказ/допуск | Принятие неполного состояния или лишний отказ |
| P3 | Общий readfile-адаптер CF/filedb | Инвентарь реального дублирования с profile/provenance/locking | Потеря гарантий записи ради формального интерфейса |

Это предложения; новые issues не создавались. Расхождение с tool1cd само
по себе не квалифицируется как подтверждённый баг ibcmd-rs.

## Остаток

- Исполнение самого tool1cd и differential за пределами матрицы ICU 4.6.
- Семантика неизвестного header-поля и native чтение после Rust mutation;
  native измерения left/битовых ширин индекса.
- APIcfBase и Parse_tree/NodeTypes сверены статически; общий byte-level
  round-trip oracle и семантика MetaTree не проверены.
- Hosted merge gate, release build, native micro-corpus, SBOM и release audit
  не выполнялись.

## Общие локальные проверки

Дополнение по [native заголовкам](filedb-native-headers-20261024.md):
24 состояния / 456 заголовков, семь контролей PASS, отдельный
`cargo xtask check` — 3 PASS и новый полный `cargo xtask ci --keep-going` —
19 PASS, exit 0, без SKIP. Fresh-context ревью всего вклада — APPROVE.
Это конечная матрица native-only изменений, без Rust-write/native-read.

После добавления ICU helper: `cargo xtask check` — **3 passed** (format,
clippy workspace crates, root compile check). Отдельный запуск
`cargo test --locked -p ibcmd-filedb -p ibcmd-v8` — **79 passed, 0 failed,
0 ignored** (40 filedb + 39 v8). Standalone API probe — прежние 26 групп PASS;
оба standalone Rust helper прошли rustfmt. Это проверки Rust, не runtime
differential контейнера или текстового дерева.

Итоговый запуск после добавления ICU helper:
`cargo xtask ci --keep-going` — **19 passed, exit 0, без SKIP**.
Это единый полный локальный запуск с PYTHONUTF8=1, MSVC /utf-8 и
CARGO_BUILD_JOBS=1. Hosted/release проверки из раздела «Остаток» он не заменяет.

История предыдущей статической фазы:
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
В предыдущей фазе все 19 шагов были покрыты с учётом отдельного повтора,
но единого успешного запуска тогда не было. Новый полный запуск на этапе
ICU выше прошёл все 19 шагов. После публикации будущего PR остаётся
обязательным hosted CI на его окончательной ревизии.

Issue #525 остаётся открытым; будущий PR должен использовать `Refs #525`.
