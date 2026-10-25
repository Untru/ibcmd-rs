# Native заголовки объектов 1CD (#525)

24.10.2026, Windows x64, **ibcmd 8.5.1.1529**, формат 1CD **8.3.8**,
страница **8192 байта**. Это продолжение [сверки с tool1cd](tool1cd-comparison.md),
а не изменение writer или доказательство полной совместимости.

**Offset 12 не является числом FAT-страниц в измеренных native состояниях.**
При двух страницах он равен 1; при четырёх встречаются 2 и 3.
Гипотеза «число страниц минус один» также опровергнута: одна страница даёт 1,
шесть — 3. Назначение поля не установлено. Его поведение совместимо с
отдельным компонентом версии/счётчиком изменений, но причинность не доказана.
Текущий Rust writer записывает число FAT-страниц — это подтверждённое
расхождение заголовков, без доказанного пользовательского сбоя.

## Входы и происхождение

Исходники продукта: `6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4`.
Исследовательская ветка до запуска: `add84be801ed7a19e062efe9606f5b084e702856`.
Rust **1.95.0**; вызовы продукта на запись не выполнялись.

Использована существующая clean-room фикстура
[`tests/fixtures/file-infobase/tiny-source`](../../tests/fixtures/file-infobase/tiny-source),
XML 2.21, четыре файла и один общий модуль. Оригинал не менялся.
Вариант `same_length` заменяет `Возврат 42;` на `Возврат 43;`:
оба модуля имеют длину **166 байт**. Большие варианты добавляют только
комментарии: SHAKE256 от ASCII seed
`ibcmd-rs issue525 native-header-corpus v1`, base64, строки по 120 символов
с `// ` и CRLF. Они не добавляют исполняемую логику.

| Вариант | Байты SHAKE256 | Длина Module.bsl, байт |
| --- | ---: | ---: |
| small | — | 166 |
| same_length | — | 166 |
| boundary | 15 843 794 | 22 005 443 |
| large | 24 MiB | 34 952 705 |
| medium_growth | 36 MiB | 52 428 971 |
| extra_growth | 48 MiB | 69 905 237 |

Генератор: [`prepare-filedb-header-corpus.py`](../../tools/research/prepare-filedb-header-corpus.py).
Порог подбирался по фактически наблюдавшейся длине blob-объекта, а не только
по длине BSL или оценке deflate. Признак FAT1 подтверждён после native записи.
Старые версии генератора/драйвера и все входы сохранены локально; совпадающие
варианты трёх поколений входов проверены по каждому файлу и tree hash.

Полный обезличенный набор —
[`filedb-native-headers-20261024.json`](filedb-native-headers-20261024.json).
Он содержит хеши входных деревьев/модулей, native executable, probe и static
library, измерительных скриптов, журналов команд и исходных/повторных
инвентарей. Заголовки всех 19 объектов каждого состояния сохранены как
массивы с порядком полей `header_columns`; два blob-объекта даны подробно.
Пути рабочих баз, базы и native деревья в репозиторий не включены.

Ключевые SHA-256:

```text
native exe       4dde71c1d061cc14c619255ec524b4ca8aca1f9bc38a92d1d766eb7d57721d20
fixture tree     5e6eb21fb8d5658b8479c8f79453c2710dde1843bac118ff445024cc65f52250
probe source     a3c23490f35a91bfa7c9aa73de73a18da3837aa885e4317d060ed5d89db88542
probe exe        5c358aa9cadc3987b4f05d24b5e494299b916639bb599925301774f7a826a60a
filedb rlib      a19a5d36f53eaabf857a4a4dfab922932ba075e110fa54738a7bad9808264815
```

Tree hash: относительные POSIX-пути сортируются по кодовым точкам, затем
SHA256 получает для каждого файла `UTF8(path) + NUL + ASCII(SHA256(file)) + LF`.
Хеши executable/библиотеки зависят от сборки; повтор должен фиксировать свои.

## Протокол и охват

Каждая операция получила собственные `--data` и `--temp`, процесс
`LANG=ru_RU.UTF-8`. Native create отдельно задаёт локаль базы.
Исходная native база создаётся с нуля. Следующая операция получает новую
копию только `1Cv8.1CD` предыдущего состояния. Копирование выполнялось с
read handle, запрещающим запись; хеши источника и копии совпали.
После команды база читалась с таким же handle и больше не использовалась
для записи. В конце все исходные хеши проверены заново.

Измерены **24 состояния, 456 заголовков**:

- `matrix/ru_RU`: create → import small → apply → import same_length → apply
  → import large → apply → import small → apply (9 состояний);
- `matrix/en_US_POSIX`: те же первые семь операций, затем import medium_growth
  → apply → import extra_growth → apply → import small → apply (13);
- `boundary`: копия `matrix/ru_RU/02-apply-small`, import boundary → apply (2).

У всех 24 native команд exit 0. Публичный reader FileDb прочитал шесть
CONFIG-таблиц во всех состояниях. Rust
[`filedb-header-probe.rs`](../../tools/research/filedb-header-probe.rs)
получает номера объектов и размеры сохранённых частей через публичный API.
[`inspect-filedb-headers.py`](../../tools/research/inspect-filedb-headers.py)
независимо читает raw заголовки ROOT и records/blob/index шести таблиц.
Число необходимых FAT-страниц выводится из длины и ёмкости страницы;
указатели обходятся и проверяются **без использования u32 offset 12**.

Все 24 снимка повторно прочитаны окончательным inspector: результаты равны
первоначальным во всех полях, кроме хеша самого скрипта. Единственная правка
inspector после измерения запрещает перезапись готового JSON (`open("x")`);
raw декодирование не менялось. Исходный скрипт восстановлен с точным LF и
его SHA совпал с каждым первоначальным инвентарём. Промежуточный ошибочный
CRLF-кандидат не прошёл эту проверку и не использовался как доказательство.

## Наблюдения

В таблицах `u4/u8/u12` — три независимых raw u32 little-endian по смещениям
4/8/12, без подстановки предполагаемых версий или счётчиков.

| ru_RU, операция | Объект | FAT | Длина объекта | FAT-страниц | u4/u8/u12 |
| --- | --- | ---: | ---: | ---: | --- |
| create | CONFIGSAVE/blob | 0 | 0 | 0 | 0/0/0 |
| import small | CONFIGSAVE/blob | 0 | 5 120 | 0 | 8/1/0 |
| apply small | CONFIG/blob | 0 | 5 120 | 0 | 9/1/0 |
| import same_length | CONFIGSAVE/blob | 0 | 5 120 | 0 | 17/1/0 |
| apply same_length | CONFIG/blob | 0 | 9 728 | 0 | 25/2/0 |
| import large | CONFIGSAVE/blob | 1 | 26 609 408 | 2 | 26/2/1 |
| apply large | CONFIG/blob | 1 | 26 614 016 | 2 | 41/3/1 |
| import small после large | CONFIGSAVE/blob | 1 | 26 609 408 | 2 | 35/2/1 |
| apply small после large | CONFIG/blob | 1 | 26 614 016 | 2 | 57/3/1 |

При повторе первых семи операций в en_US_POSIX эти длины и слова совпали.
Это два измеренных профиля одной сборки, не доказательство для других версий
платформы, размеров страниц или локалей.

| en_US_POSIX, операция | Объект | Длина объекта | FAT-страниц | u4/u8/u12 |
| --- | --- | ---: | ---: | --- |
| import medium_growth | CONFIGSAVE/blob | 39 911 680 | 3 | 35/4/2 |
| apply medium_growth | CONFIG/blob | 66 520 576 | 4 | 57/5/2 |
| import extra_growth | CONFIGSAVE/blob | 53 213 696 | 4 | 44/5/3 |
| apply extra_growth | CONFIG/blob | 93 124 864 | 6 | 73/6/3 |
| import small после extra | CONFIGSAVE/blob | 53 213 696 | 4 | 53/5/3 |
| apply small после extra | CONFIG/blob | 93 124 864 | 6 | 89/6/3 |

Граница FAT0/FAT1 дополнительно подтверждена в отдельной ветви ru_RU.
Прямой список в заголовке вмещает `(8192−24)/4 = 2042` страниц данных.
После import boundary CONFIGSAVE/blob имеет **2046 страниц данных,
одну FAT-страницу, длину 16 754 688, слова 17/2/1**.
После apply CONFIG/blob: **2046 страниц, одна FAT-страница,
длина 16 759 296, слова 25/2/1**.
Это исключает объяснение offset 12 как последнего нулевого индекса FAT.

Одинаковая длина исходного модуля не означает одинаковую длину физического
объекта: после same_length размер CONFIG вырос. Обратная загрузка small
является **логическим уменьшением содержимого**, а не physical truncate.
В конце ru_RU файл сохранил 56 770 560 байт; CONFIG содержит 3 623 байта
сохранённых частей вместо 25 984 412, но extent остался 26 614 016.
В en_US_POSIX файл сохранил 149 929 984 байта; CONFIG уменьшился с 51 965 174
до 3 620 байт частей, сохранив extent 93 124 864. CONFIGSAVE после apply
не содержит строк, но также сохраняет выделенный extent.
`used_data_pages` в JSON означает страницы всего extent, включая свободную
ёмкость blob, а не количество страниц живого payload.

## Сопоставление с writer и ограничения

Закреплённый
[`object.rs:220–259`](https://github.com/Untru/ibcmd-rs/blob/6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4/crates/ibcmd-filedb/src/object.rs#L220-L259)
записывает 0 в FAT0 и `table_count` в FAT1. Измеренный native при двух
FAT-страницах записал 1, при шести — 3. Reader выводит число страниц из
длины и не использует offset 12; все измеренные native объекты им прочитаны.

Это основание отдельно исследовать сохранение неизвестных header-полей и
проверить результат Rust mutation нативной платформой. Продуктовый writer
здесь не меняется: значения 1/2/3 нельзя механически заменить ни числом
страниц, ни числом минус один, ни произвольным счётчиком.
Измерения не доказывают, что native отвергает файлы текущего writer.
Native export исходников, Rust-write/native-read, forced physical shrink,
индексные left/битовые ширины и отказ на испорченные native файлы здесь не
измерялись. tool1cd executable не запускался; его `version_3` остаётся
статическим названием поля, а не независимо доказанной семантикой.

## Воспроизведение

Из корня checkout, Rust 1.95.0, Python 3.12 и доступный native 8.5.1.1529:

```powershell
cargo build --locked -p ibcmd-filedb
rustc --edition=2024 tools/research/filedb-header-probe.rs --extern ibcmd_filedb=target/debug/libibcmd_filedb.rlib -L dependency=target/debug/deps -o target/issue525-headers.exe
python -B tools/research/test-filedb-headers.py
python -B tools/research/prepare-filedb-header-corpus.py --fixture tests/fixtures/file-infobase/tiny-source --output <fresh-input-dir> --boundary-bytes 15843794
```

Для каждого состояния создать новые `<db>`, `<data>`, `<temp>`; при продолжении
скопировать только закрытый `1Cv8.1CD` предыдущего состояния и сверить SHA.
Исполнить соответствующую строку, задав `$native` и `LANG=ru_RU.UTF-8`:

```powershell
& $native infobase create --database-path=<db> --data=<data> --temp=<temp> --create-database --locale=ru_RU
& $native config import --database-path=<db> --data=<data> --temp=<temp> <input-variant>
& $native config apply --database-path=<db> --data=<data> --temp=<temp> --force
python -B tools/research/inspect-filedb-headers.py --database <closed-snapshot> --probe target/issue525-headers.exe --output <new-json>
```

Повторить последовательности выше; для POSIX create использовать
`--locale=en_US_POSIX`. У каждого процесса также должны быть собственные
TEMP/TMP. Сохранить аргументы, exit, native/probe/input/output SHA, затем
повторно проверить неизменность всех снимков. Имена `<...>` — места для
подстановки, не буквальные PowerShell аргументы. Сравнивать измеренные raw
слова/топологию, не требовать одинакового SHA базы: native содержит изменяемые
метаданные. Этот протокол использует только созданные для него базы.

## Проверки исследовательских инструментов

[`test-filedb-headers.py`](../../tools/research/test-filedb-headers.py):
**7 PASS** — независимое сохранение raw u32 при FAT0/FAT1, усечение,
неверный FAT pointer, невозможная длина, повтор страницы и запрет перезаписи
JSON. Первые шесть контролей упали на незавершённом raw decoder, затем прошли.
Контроль перезаписи отдельно показал RED (`FileExistsError not raised`),
после `open("x")` весь набор GREEN. Это тесты synthetic байтов и инструмента,
не замена 24 native измерениям. Первые sandbox отказы доступа к TEMP не
считались ожидаемым RED; подтверждённый RED получен с доступным локальным TEMP.

`cargo test --locked -p ibcmd-filedb`: **29 unit + 11 integration PASS,
0 failed/ignored**. Physical adapter policy и её self-test прошли.
Окончательный `cargo xtask check`: **3 PASS**.
Новый полный `cargo xtask ci --keep-going`: **19 PASS, exit 0, без SKIP**
в одном запуске (workspace crates 283 с, root integration 273 с).
Исходники инструментов и staged JSON на протяжении запуска не менялись.
После запуска в отчёт добавлены только эти результаты и итог ревью;
окончательный staged diff повторно проверен corpus governance и diff check.

Независимое fresh-context ревью всего вклада: **APPROVE**, без замечаний.
Ревьюер без использования raw inspector автора проверил все 24 снимка /
456 заголовков, FAT и принадлежность страниц, исходные/повторные инвентари,
связь состояний и публичный probe. Независимо воспроизведены шесть module
hashes и подтверждена provenance инструментов. Семь контролей прошли;
старый inspector повторно показал RED на перезаписи JSON. Повторены прежние
ICU и locale матрицы с побайтово идентичными результатами, пять live controls
и 26 API assertions. Проверена 61 закреплённая ссылка на исходники.
Ревьюер не запускал Cargo/native writes и не подтверждал тогда ещё текущий
CI; приведённый выше окончательный exit проверен отдельно после его завершения.

Hosted/release gates этим локальным запуском не заменяются.

Полная приёмка #525 остаётся открытой; это конечная native матрица заголовков.
