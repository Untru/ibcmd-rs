# Дизайн: автономное ядро, версии и CF

## 1. Контекст

Сейчас ключевые операции связаны напрямую:

```text
MSSQL rows -> blob decode -> XML formatting -> filesystem
XML files  -> patch existing blob -> ConfigSave SQL staging
```

Версия XML передаётся как закрытый enum и местами используется как версия платформы. Импорт часто изменяет существующий blob вместо построения нового. `src/v8_container.rs` знает только ограниченный вариант контейнера и отклоняет chained pages.

Этот дизайн вводит новые границы рядом с рабочим legacy-кодом. Массового rewrite нет: каждое семейство переносится отдельным PR и может сравниваться со старым путём.

## 2. Архитектура

```text
                          +----------------------+
XML tree ---- ibcmd-xml --|                      |-- ibcmd-xml ---- XML tree
                          |                      |
CF ----------- ibcmd-cf --|     ibcmd-core       |-- ibcmd-cf ----- CF
                          |                      |
MSSQL ---- legacy/native -| IR + profiles +      |-- storage image - MSSQL
                          | migrations + losses  |
                          +----------------------+
```

### 2.1 Crate boundaries

- `ibcmd-core`: версии, профили, IR, diagnostics, migration graph и adapter contracts. Не зависит от CLI, XML, SQL, process execution или CF.
- `ibcmd-xml`: lossless XML AST, source-tree layout и XCF dialect adapters.
- `ibcmd-v8`: бинарные контейнеры Format15/Format16, block chains, streaming reader/writer и resource limits.
- `ibcmd-cf`: CF preamble, archive model, compression, `StorageImage` mapping, repack/overlay/bootstrap.
- root `ibcmd-rs`: CLI, orchestration и временный legacy MSSQL adapter.

Dependency direction проверяется отдельным архитектурным тестом. `ibcmd-core` не может зависеть от остальных crates.

## 3. Две модели данных

### 3.1 `StorageImage`

Lossless физически-логический снимок конфигурационного хранилища:

- ordered entries;
- logical key/name и multipart identity;
- raw element header;
- packed и unpacked payload;
- compression kind;
- storage attributes;
- source profile и provenance.

Он используется для CF/MSSQL passthrough, overlay и сравнения storage-level данных.

### 3.2 `CanonicalConfiguration`

Семантическая модель:

- UUID и логическая identity объекта;
- kind, ownership и ссылки;
- упорядоченные typed properties;
- assets по content hash;
- generated types и служебные связи;
- anchored opaque facets для пока неизвестных полей;
- provenance исходного профиля.

Typed islands добавляются постепенно. Неизвестное семейство может быть прочитано и сохранено в том же профиле, но cross-profile encode блокируется до появления migration/codec rule.

## 4. Версии и профили

Независимые значения:

```text
PlatformBuild       8.3.24.1819, 8.3.27.1989, 8.5.1.1150
XmlDialect          2.17, 2.20, 2.21
CompatibilityMode   отдельное открытое значение
StorageProfile      layout Config/ConfigSave и DBMS
ContainerRevision   Format15, Format16 и их параметры
ArtifactKind        Configuration; позднее Extension/EPF/ERF
```

`VersionProfile` загружается из schema-validated JSON/TOML. Профиль может наследовать один базовый профиль и задавать delta. Встроенные профили поставляются с binary, внешний каталог может добавлять новые experimental-профили. Override существующего verified-профиля без явного флага запрещён.

Профиль содержит:

- точные/range version coordinates;
- fingerprints артефакта;
- availability/default/order правил;
- platform constants и known UUIDs;
- capabilities по направлению и семейству;
- status `experimental | verified`;
- ссылки на fixture/test evidence.

Автодетекция возвращает evidence и состояние `exact | ambiguous | unknown`. Запись допускается только с однозначным target profile.

## 5. CF codec

Реализуются два контейнерных layout:

- Format15: существующая 32-bit family с versioned storage headers;
- Format16: 64-bit offsets/block headers, собственный preamble и base offset.

CF reader:

1. определяет layout;
2. проверяет headers и address tables;
3. лениво читает chained pages через `Read + Seek`;
4. ограничивает depth, element count, expanded bytes и compression ratio;
5. различает stored/raw-DEFLATE payload;
6. отображает root entries в `StorageImage`.

CF writer:

1. выполняет полный preflight до создания результата;
2. планирует offsets/page chains детерминированно;
3. пишет через временный sibling-file;
4. повторно открывает и структурно проверяет результат;
5. атомарно заменяет output;
6. сохраняет неизвестные same-profile entries и исходный порядок.

Уровни capability:

```text
Inspect   -> только structural read
Repack    -> CF -> StorageImage -> CF
Export    -> CF -> Canonical IR -> XML
Overlay   -> XML changes + base CF -> CF
Bootstrap -> XML -> complete StorageImage -> new CF
Convert   -> source IR -> migrations -> target CF/XML
```

Каждый уровень включается независимо; наличие reader не означает готовность bootstrap writer.

## 6. XCF adapter

XML сначала читается в ordered lossless AST с QName/namespace/attribute order. Затем dialect adapter преобразует известные узлы в typed IR, а неизвестные прикрепляет как opaque facets к стабильному anchor.

Диалекты реализуются как общий baseline плюс delta-descriptors. Изменение root `version` само по себе никогда не считается миграцией.

Source-tree reader защищает от path traversal, duplicate normalized paths и conflicting UUID. Writer выполняет preflight и использует temporary directory + atomic replace.

## 7. Family codecs и bootstrap

Единый `FamilyCodec` объявляет:

- поддержанные profile/dialect ranges;
- `decode_storage`, `decode_xcf`;
- `encode_storage`, `encode_xcf`;
- `inspect_capabilities`;
- `requires_base_blob`;
- возможные losses.

Существующие packer-ы сначала оборачиваются в `StoragePatch` и классифицируются:

- `Compiled` — запись построена без base;
- `NeedsBase` — возможен только overlay;
- `Unsupported` — операция запрещена.

Bootstrap readiness manifest связывает каждый source artifact с ожидаемыми CF entries и codec. Финальный assembler запускается только при полном покрытии root, `version`, `versions`, generated types, ссылок, metadata bodies и assets.

## 8. Миграции

`MigrationStep` отдельно реализует `analyze` и `apply`. Graph planner строит детерминированный путь между профилями. Каждый шаг перечисляет touched capabilities и возможные losses.

Политики:

- `error` — default, результат не записывается;
- `warn` — запись разрешена только для обратимо сохранённых данных;
- `drop` — явное удаление, каждый случай фиксируется в отчёте.

Opaque-фрагмент с другим source profile не переносится автоматически. Same-profile no-op обязан сохранять semantic digest, а passthrough неизменённого payload — его bytes.

## 9. Проверка и compatibility evidence

Обычный CI работает без компонентов 1С и включает Windows/Linux:

- format, clippy, all-targets build/test;
- fixture manifest/hash validation;
- XML/CF same-profile roundtrip;
- migration matrix;
- malformed corpus и property tests;
- clean-PATH smoke, подтверждающий отсутствие platform process execution.

Каждая fixture хранит artifact kind, source build, XML dialect, container/storage revision, SHA-256, provenance, features и expected losses. В репозиторий допускаются только специально созданные минимальные конфигурации без стороннего прикладного кода.

Compatibility report генерируется из `operation x artifact x source profile x target profile x family x evidence`. Статус `verified` невозможен без зелёного evidence.

## 10. Порядок поставки

1. Вернуть зелёный baseline и зафиксировать standalone contract.
2. Создать workspace boundaries, version/profile model и IR.
3. Реализовать CF read/inspect и CF -> XML.
4. Реализовать deterministic repack.
5. Выделить pure `StoragePatch` и добавить base-CF overlay.
6. Реализовать XCF adapters и base-free codecs по семействам.
7. Собрать special entries и полный XML -> CF bootstrap.
8. Добавить migration graph и первый 2.20 <-> 2.21 edge.
9. Включить evidence-based compatibility и release gates.

## 11. Отклонённые варианты

- Обязательный `ibcmd`/Designer backend: нарушает standalone constraint.
- EDT/JVM как core model: добавляет тяжёлый runtime и не решает CF/native storage.
- Один универсальный `version` enum: снова связывает независимые форматы.
- Продолжение version checks внутри `mssql_dump`: не масштабируется и не даёт testable boundaries.
- Сразу переписать все семейства: слишком высокий regression risk; выбран strangler migration.
- Считать overlay полноценным XML -> CF: скрывает зависимость от base blob и создаёт недостоверную capability.

## 12. Основные риски

- Реальные CF layout могут иметь дополнительные варианты; unknown layout всегда fail-closed.
- Невозможно честно оценить bootstrap completeness без versioned fixture corpus.
- Generated type IDs, `root`, `version` и `versions` — самостоятельные компиляторы.
- Формы, MXL/DCS, права, predefined и support/signature data требуют отдельных codecs.
- Downgrade в общем случае lossy и не может быть обещан для произвольной конфигурации.
