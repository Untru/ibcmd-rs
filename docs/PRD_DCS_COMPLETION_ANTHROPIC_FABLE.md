# PRD: завершение поддержки СКД в ibcmd-rs

**Документ для реализации с помощью Anthropic Fable / Claude Code**  
**Статус:** рабочая спецификация  
**Дата:** 13 августа 2026  
**Репозиторий:** `ibcmd-rs`  
**Целевой профиль:** платформа 1С 8.3.27, XML 2.20  
**Основной принцип:** evidence-driven, fail-closed, двунаправленная совместимость

---

## 1. Резюме

Цель проекта — завершить канонический контур работы со схемами компоновки данных (СКД), чтобы один и тот же смысл безопасно проходил полный цикл:

`XML-исходник ⇄ каноническая модель ⇄ бинарное тело CF/БД ⇄ платформа 1С ⇄ XML-исходник`.

«Безопасно» означает:

- один семантический объект не описывается несколькими несовместимыми моделями;
- QName, порядок XML-узлов, `xsi:type`, значения по умолчанию и физические UUID берутся только из проверенной evidence-policy;
- неподдержанная конструкция не превращается в «отсутствующую» и не теряется молча;
- запись выполняется атомарно: полная проверка до изменения байтов, файла или базы;
- каждое заявленное направление подтверждено тестом и, где требуется, циклом через настоящую платформу 1С;
- номер patch-релиза не становится диалектом без отдельного доказанного несовпадения.

На текущем этапе уже построен фундамент: канонические Settings, общий бинарный envelope, 1–2 варианта настроек, ограниченная внутренняя `DataCompositionSchema`, Query/Union/link, первый `AreaTemplate` и appearance side table. Оставшаяся работа — не «написать СКД с нуля», а последовательно расширять доказанную поверхность и удалить старые эвристические маршруты.

## 2. Уровни завершённости

| Уровень | Смысл | Критерий |
|---|---|---|
| L0 — безопасное ядро | Уже реализованные формы не теряют данные; неизвестное блокируется | Пройдено для текущих bounded-cohort |
| L1 — практическая полнота | Типовые отчёты и DynamicList проходят туда-обратно | Цель ближайшей программы |
| L2 — corpus parity | Покрыты все конструкции, встречающиеся в выбранном очищенном корпусе | После L1 |
| L3 — закрытие эпика | EDT roundtrip, IBCMD parity, нет второго сериализатора и silent fallback | Финальная цель |

Процент готовности нельзя корректно выводить из числа XML-элементов: один редкий `AreaTemplate` может требовать больше работы, чем десятки простых полей. Прогресс измеряется закрытыми координатами: **семантика × XML-контекст × физическое представление × направление**.

## 3. Текущий baseline

### 3.1. Уже реализовано и доказано

**Settings СКД**

- `selection`;
- bounded `filter`;
- `order`;
- bounded `conditionalAppearance`;
- `ListSettings` управляемой формы;
- Form-wide `Attributes/ConditionalAppearance`;
- source-owned `dataParameters` и `StructureItemGroup` для доказанных форм;
- один или два `settingsVariant`, связанные с внешними Settings позиционно;
- единый namespace-aware envelope и проверка ролей документов;
- обратный компилятор XML → бинарное тело для доказанной формы envelope.

**Внутренняя DataCompositionSchema**

- локальный источник данных;
- объектный набор данных;
- строковые и десятичные поля;
- вычисляемое поле;
- негруппированные итоги;
- строковый параметр;
- reference `TypeId` в ограниченном доказанном контексте;
- один Query/Union/link cohort;
- 1–2 оболочки вариантов настроек.

**AreaTemplate и appearance**

- один style-free `AreaTemplate`;
- одна строка, одна ячейка, Field/expression;
- первый appearance-параметр;
- side table и `appIndex=0`;
- оба физических направления и platform compiler acceptance для доказанного cohort.

### 3.2. Проверки baseline

На последнем подтверждённом этапе проходили:

- `ibcmd-core`: 193 теста;
- `ibcmd-schema`: 79 тестов;
- `ibcmd-xml`: 198 тестов;
- focused DCS compiler: 22 теста;
- clippy для общих crates.

Исторический root baseline до последних срезов: 1946 тестов, 1861 passed, 85 известных failures. Эти 85 нельзя считать автоматически устранёнными: нужен свежий последовательный прогон и сравнение списка имён.

### 3.3. Что baseline не доказывает

- полную модель СКД;
- произвольное число вариантов;
- полный Type/TypeSet/TypeId;
- все виды DataSet, links и параметров;
- все Settings и вложенные контексты;
- произвольный `AreaTemplate`, appearance и `appIndex`;
- сохранение произвольных неизвестных XML-расширений;
- полный EDT roundtrip;
- полную совместимость с IBCMD/Unica;
- отсутствие legacy-веток во всех физических адаптерах.

## 4. Источники истины

| Источник | Для чего разрешён | Для чего запрещён |
|---|---|---|
| Fresh clean-room 1С 8.3.27.2214, два раунда | XML spelling, бинарная топология, omission/default, платформенная приёмка | Обобщение на непроверенные формы |
| Retained sanitized corpus | Частотность, поиск координат, cross-evidence | Публикация приватных данных, byte oracle без provenance |
| EDT model/bytecode | Containment, cardinality, отрицательное поведение reader/writer | Утверждение о бинарном формате CF/БД |
| Unica main `a527d409…` | Seed, гипотеза, список поддерживаемых вариантов | Единственный XML/binary oracle |
| XSD/Xcore | Словарь и модельные связи | Автоматическое право на emission |
| Synthetic unit tests | Регрессия уже принятого контракта | Новая platform-policy |

Любая новая возможность проходит цепочку: **гипотеза → clean-room fixture → manifest → schema-policy → core IR → XML codec → physical adapters → roundtrip gates**.

## 5. Объём оставшейся работы

### WS1. Расширение внутренней DataCompositionSchema

Цель: покрыть практически используемые структуры схемы без возврата к generic XML writer.

Оставшиеся координаты:

- расширенные Query и Union;
- несколько наборов данных и links;
- дополнительные свойства link;
- calculated fields: title, restrictions, дополнительные типы;
- grouped totals и повторные группы;
- rich parameters: boolean, decimal, StandardPeriod, available values, expressions;
- `outputParameters`, `userFields`, `additionalProperties`;
- `defaultSettings`;
- вложенные schema/settings contexts;
- ограничения порядка, cardinality и defaults для каждого нового узла.

Критерий завершения: доказанные schema-cohort не используют `DataCompositionDocumentMode::Schema`, provider-owned QName mapping или fallback к строковой нормализации.

### WS2. TypeId, current-config и разрешение ссылок

Цель: заменить строковые и контекстные эвристики типизированным разрешением ссылок.

Работы:

- инвентаризация всех видов `Type`, `TypeSet`, `TypeId`;
- отдельные evidence-cohort для прямого поля, параметра и вложенного контекста;
- canonical reference identity, независимая от XML-префикса;
- adapter-supplied resolver без возврата готового XML;
- строгие ошибки для неизвестного/неразрешённого UUID;
- проверка current-config prefix/namespace;
- оба направления через платформу.

Запрещено: сохранять неизвестный `TypeId` как строку и продолжать каноническую запись.

### WS3. Полнота Settings

Цель: довести Settings до уровня типовых отчётов и форм.

Работы:

- дополнительные comparison operators и right-value types;
- группы фильтров и списки значений;
- presentation и user-setting metadata;
- несколько filter/order/appearance items;
- `outputParameters`, `dataParameters`, `userFields` как typed либо evidence-backed source-owned;
- вложенные Settings в structure items, charts, tables/groups;
- context-specific defaults и cardinality;
- доказанная семантика пустых контейнеров.

Для каждого контекста нужен отдельный gate: одинаковое локальное имя не означает одинаковый контракт.

### WS4. AreaTemplate, appearance и appIndex

Цель: перейти от одного доказанного примера к полезной модели макетов СКД.

Последовательность:

1. web color;
2. StyleItem UUID reference;
3. несколько appearance items;
4. несколько ячеек и строк;
5. повторное использование одного `appIndex`;
6. несколько индексов и строгий порядок side table;
7. Picture, Font, Border, merge и formatting;
8. field/group/header/footer template bindings;
9. отрицательные случаи: missing/duplicate/out-of-range index.

Каждая ступень обязана проверять native document topology, а не только экспортированный `Template.xml`.

### WS5. Form-контексты СКД

Цель: один semantic codec для Settings/appearance внутри форм, без строкового редактирования.

Работы:

- завершить DynamicList `ListSettings` для расширенных Settings;
- доказать `UniversalListServerOnlyState` и его chunk/envelope;
- разделять metadata-only auto-save shell и реальный storage payload;
- единый проход по Form XML вместо параллельных collectors;
- XML → raw Form body и raw body → XML;
- exact opaque retention только для доказанного storage property;
- atomic update нескольких DCS-секций одной формы.

Формы вне DCS не расширяются в этой программе, кроме необходимой транспортной оболочки.

### WS6. Неизвестные и source-owned расширения

Цель: устранить silent loss, не обещая невозможное generic passthrough.

Классы:

- `Typed` — полностью поддержанная семантика;
- `SourceOwned` — профиль распознаёт форму, но canonical IR её не редактирует;
- `Unsupported` — well-formed форма вне доказанного cohort;
- `Malformed` — нарушена распознанная структура;
- `UnknownQName` — профиль не знает элемент/атрибут.

Правила:

- `Absent` только при физическом отсутствии;
- `Unsupported` никогда не преобразуется в `Absent`;
- truly unknown fail-closed, если нет положительной политики размещения;
- unchanged source envelope можно вернуть byte-exact только в том же profile/context;
- изменение typed-соседей при source-owned content блокируется, пока не доказан merge;
- EDT negative `throwWrongElement` остаётся отрицательной policy, а не «opaque support».

### WS7. Консолидация production-маршрута

Цель: удалить второй сериализатор и аварийные fallback.

Работы:

- убрать `.or_else(legacy_normalizer)` на доказанных маршрутах;
- исключить `filter_map`, `continue` и `Option`, если они могут скрыть потерю документа;
- убрать local-name-only scanners, prefix stripping и XML substring parsing;
- физические адаптеры оставляют только framing, compression, base64, record location и reference lookup;
- весь QName/order/type/default contract находится в schema+xml;
- preflight всего объекта до первой записи;
- отдельные стабильные diagnostics для evidence pending, unsupported и malformed.

### WS8. EDT и IBCMD parity

Цель: закрыть внешнюю совместимость, а не только приёмку платформой.

Работы:

- actual EDT 2025.2.3 import/export на bounded fixtures;
- сравнение model semantics, canonical XML и loss boundaries;
- IBCMD/Unica info/validate/compile cross-check;
- фиксация известных расхождений как nonclaim либо upstream issue;
- запрет заявлять EDT parity по одному platform roundtrip.

### WS9. Governance, evidence и обслуживание

Работы:

- immutable fixture manifest с provenance, hashes, round labels и privacy boundary;
- schema policy `deny_unknown_fields` и drift/self tests;
- coverage обновляется только по фактически typed features;
- физический adapter guard не расширяется новыми QName/UUID literals;
- OpenSpec, PR summary и issue memory синхронизируются после milestone;
- список 85 известных root failures хранится и сравнивается по именам;
- временные build/lab артефакты удаляются после каждого прогона.

## 6. Обязательная архитектура

### 6.1. Слои владения

**ibcmd-core** владеет:

- семантическим IR;
- инвариантами, bounds и serde;
- provenance/profile identity;
- без XML QName, prefix, raw XML и физических UUID.

**ibcmd-schema** владеет:

- evidence manifests и policy;
- expanded QName;
- XML child order, cardinality, defaults, lexical tokens;
- context/profile constraints;
- физическими UUID только как доказанным transport fact.

**ibcmd-xml** владеет:

- namespace-aware parse/emit;
- mapping XML ⇄ core IR;
- атомарным rewrite;
- source envelope и строгой taxonomy;
- canonical prefixes как formatting choice, не semantic identity.

**compiler / mssql_dump / module_blob** владеют:

- binary framing, deflate, base64, record lookup;
- DB/CF transport;
- reference resolution data;
- вызовом common codec;
- не владеют QName, child order, `xsi:type` или XML merge.

### 6.2. Обязательная taxonomy

```text
Absent
Typed(value)
SourceOwned { feature, exact_source, provenance }
Unsupported { feature, reason, location }
Malformed { feature, reason, location }
LimitExceeded { resource, actual, maximum }
```

Ошибки не должны схлопываться в `None`. Внешние API могут сохранять compatibility-wrapper, но production обязан видеть точный outcome.

### 6.3. Атомарность

До изменения output необходимо:

1. разобрать все связанные документы;
2. проверить evidence-policy;
3. разрешить все ссылки;
4. проверить bounds;
5. построить все замены в памяти;
6. только после этого применить изменения.

Ошибка в одном Settings/Area/Form блокирует весь объект. Частичная нормализация запрещена.

## 7. План поставки

Каждая capability выпускается вертикальным срезом:

1. read-only inventory;
2. evidence design;
3. clean-room experiment, если committed evidence недостаточно;
4. immutable fixture + manifest;
5. schema policy + drift tests;
6. core IR + invariants;
7. namespace-aware XML codec;
8. оба physical adapters;
9. unit/integration/platform tests;
10. удаление заменённого legacy path;
11. documentation/coverage update;
12. cleanup временных данных.

Нельзя делить production-срез так, чтобы XML → binary и binary → XML использовали разные semantic models.

## 8. Распределение моделей

### 8.1. Рекомендуемая матрица

| Работа | Модель | Почему |
|---|---|---|
| Архитектурное решение, новый форматный контракт, противоречивое evidence | Fable 5 | Нужны long-horizon reasoning и ответственность за границы |
| Core IR, taxonomy, unknown/source-owned policy, удаление fallback | Fable 5, затем независимый review Fable/Sonnet | Высокий риск потери данных |
| Bounded Rust implementation по утверждённой policy | Sonnet 5 | Основной coding workhorse |
| Namespace parser/emitter, wiring adapters, тесты и refactor | Sonnet 5 | Требуется хорошее локальное рассуждение, но контракт уже задан |
| Поиск, inventories, hash/size/b64, заполнение таблиц | Haiku 4.5 | Механическая проверяемая работа |
| Запуск тестов, сбор логов, cleanup explicit paths | Haiku 4.5 | Низкий риск при фиксированных командах |
| Финальная cross-layer проверка milestone | Fable 5 | Проверяет отсутствие второго маршрута и overclaims |

### 8.2. Где допустима слабая модель

Haiku 4.5 можно использовать только при трёх условиях:

1. входные файлы и команды перечислены явно;
2. результат проверяется автоматически или diff review;
3. модель не принимает семантических решений.

Разрешённые задачи:

- `rg`-inventory usages и literals;
- декодирование `.b64`, SHA-256 и размеры;
- сравнение двух round inventories;
- генерация manifest из уже утверждённых значений;
- добавление однотипных mutation/drift tests;
- обновление таблиц документации по готовым числам;
- запуск точного test matrix и сводка;
- удаление только явно созданной временной директории;
- механическое переименование API после утверждённого design.

Запрещённые задачи для слабой модели:

- выбирать QName, порядок, default или `xsi:type`;
- проектировать core IR;
- решать, что считать malformed/unsupported/source-owned;
- интерпретировать противоречащие fixtures;
- удалять production fallback;
- проектировать binary framing и Area side table;
- расширять emission cohort;
- создавать platform seed без review;
- самостоятельно коммитить/мержить high-risk изменение.

### 8.3. Правила эскалации

Haiku → Sonnet, если:

- требуется изменить Rust behavior;
- затронуто более трёх production-файлов;
- тесты показывают неизвестную форму;
- обнаружено расхождение manifest и артефакта;
- нужен новый public API.

Sonnet → Fable, если:

- вводится новый semantic type;
- evidence противоречиво;
- меняется unknown/source-owned policy;
- удаляется fallback или silent compatibility;
- возможна потеря пользовательских данных;
- требуется обобщить один cohort на несколько контекстов.

Ориентир затрат на одну координату:

- Fable: 10–20% — design, evidence adjudication, review;
- Sonnet: 60–75% — реализация и тесты;
- Haiku: 10–25% — inventory, артефакты, прогоны и docs.

## 9. Шаблон work package

Каждая задача для агента должна содержать:

```text
ID:
Цель:
Текущий HEAD:
Разрешённые файлы:
Evidence authority:
Exact admitted cohort:
Explicit nonclaims:
Обязательные invariants:
Ожидаемые API:
Оба production directions:
Fail-closed cases:
Тесты и команды:
Cleanup paths:
Что нельзя менять:
Формат отчёта:
```

### Промпт для Fable

```text
Спроектируй bounded vertical slice <ID>. Не пиши код до завершения
evidence audit. Раздели platform facts, cross-evidence и гипотезы.
Определи admitted cohort, nonclaims, core IR, schema policy, XML
taxonomy, оба physical directions, atomicity и deletion boundary legacy
пути. Любое неподтвержденное emission rule оставь unsupported.
```

### Промпт для Sonnet

```text
Реализуй утверждённый контракт <ID> без расширения cohort. Следуй
готовым QName/order/type/default policy. Подключи XML→binary и
binary→XML в одном срезе, добавь fail-closed tests, удали только
перечисленный legacy path. Не создавай fallback. После тестов очисти
только explicit temp paths и покажи git diff --check.
```

### Промпт для Haiku

```text
Выполни механическую задачу <ID>. Не меняй semantics или public API.
Работай только с перечисленными файлами/командами. Сообщи exact
hashes/counts/test results. При любом расхождении остановись и передай
его старшей модели. Удали только созданную тобой temp directory.
```

## 10. Acceptance gates

### 10.1. Для каждой координаты

- fixture parse → IR → emit;
- source и native representation дают одинаковый IR;
- XML → binary → XML;
- binary → XML → binary;
- два свежих platform rounds, если вводится новый format fact;
- третий compiler acceptance, если меняется reverse writer;
- alias prefixes, ancestor namespaces, shadowing и unbound QName;
- duplicate, order, missing required, unsupported type/value;
- malformed/unsupported/absent различаются;
- unknown sibling не исчезает;
- failure не оставляет частичный output;
- resource bounds и retained byte budget;
- privacy scan и exact hashes.

### 10.2. Репозиторные gates

```bash
cargo fmt --all -- --check
cargo clippy --locked --workspace --exclude ibcmd-rs --all-targets -- -D warnings
cargo test --locked --workspace --exclude ibcmd-rs
cargo check --locked -p ibcmd-rs --all-targets --no-default-features
git diff --check
```

На Windows/Parallels:

```powershell
pwsh -NoProfile -File tools/validate-physical-adapter-policy.ps1 -RepositoryRoot .
pwsh -NoProfile -File tools/validate-physical-adapter-policy.ps1 -RepositoryRoot . -SelfTest
pwsh -NoProfile -File tools/verify-native-evidence.ps1 -RepositoryRoot . -BinaryPath target/release/ibcmd-rs.exe
```

Полный root baseline выполняется последовательно. Сравниваются не только totals, но и имена известных failures.

## 11. Evidence и privacy

Каждый manifest обязан содержать:

- product/release/build и XML profile;
- SHA и размер `ibcmd`;
- identity/path/hash extractor;
- seed provenance и его статус: hypothesis-only;
- два round labels и способ создания баз;
- locale;
- hashes raw/packed/unpacked/native/fragments;
- owner/fixture identity без приватных путей;
- exact claims и explicit nonclaims;
- privacy statement;
- cleanup statement.

Нельзя публиковать приватные CF/1CD, полные native trees, raw `.0`, object/query/module/value/GUID material. Допустимы clean-room CF, минимальные sanitized fragments, hashes и абстрактные semantic shapes.

## 12. Наблюдаемость прогресса

Для каждой координаты вести статус:

```text
hypothesis
evidence-captured
policy-bound
core-modeled
xml-bidirectional
physical-bidirectional
platform-accepted
legacy-removed
documented
```

Milestone считается закрытым только когда все обязательные состояния достигнуты. «Тест написан» или «XML экспортируется» отдельно не означает готовность.

## 13. Риски

| Риск | Последствие | Контроль |
|---|---|---|
| Overclaim из одного fixture | Фабрикация неподдержанного XML | Narrow cohort + nonclaims + Fable review |
| Silent `Unsupported → None` | Потеря данных | Typed outcome, negative tests |
| Два сериализатора | Drift направлений | Один common codec, search/guard gates |
| Local-name parsing | Namespace collision | Expanded QName + shadowing tests |
| Неатомарная запись | Повреждённый объект | Full preflight + staged replacements |
| Приватные данные в evidence | Утечка | Clean-room fixtures + privacy scan |
| Слабая модель принимает policy | Неверная архитектура | Task allowlist + mandatory escalation |
| Disk exhaustion | Ложные сбои и остановка VM | Cleanup policy и контроль свободного места |

## 14. Политика очистки

После каждого прогона агент обязан:

- удалить созданную им `.codex-tmp/<task-id>`;
- удалить новый guest lab root только если fixture уже перенесён и путь явно подтверждён;
- удалить generated build artifacts текущего milestone, если они больше не нужны;
- остановить VM, если агент её запускал и нет явной просьбы оставить running;
- проверить свободное место;
- не трогать retained fixtures, пользовательские caches и чужие temp directories;
- не выполнять широкие `rm -rf` по переменным, home или workspace root.

Cleanup является acceptance gate, а не факультативной операцией.

## 15. Definition of Done для СКД

СКД считается завершённой, когда:

1. типовые bounded-schema/settings/area/form конструкции проходят оба направления;
2. binary ↔ XML использует один canonical semantic route;
3. legacy generic writer и string-edit fallback недоступны для поддержанных профилей;
4. неизвестные конструкции либо доказанно сохраняются, либо стабильно блокируются;
5. нет `Unsupported → Absent`, partial mutation и unresolved `continue`;
6. TypeId/current-config и Area side tables имеют evidence-policy;
7. Form ListSettings/Attributes/ServerState используют common body codecs;
8. platform 1C принимает reverse output;
9. actual EDT roundtrip и IBCMD parity зафиксированы;
10. root baseline не имеет новых failures, исторический cohort объяснён;
11. OpenSpec, coverage, PR и issue memory соответствуют коду;
12. временные лаборатории и build-мусор очищены.

## 16. Ближайший backlog

1. **DCS-AREA-COLOR-01** — один WebColor в Area appearance: Fable design/evidence review → Sonnet implementation.
2. **DCS-AREA-STYLE-REF-01** — один StyleItem UUID reference.
3. **DCS-AREA-INDEX-02** — несколько ячеек, повторный и второй `appIndex`.
4. **DCS-PARAM-TYPES-01** — boolean/decimal/StandardPeriod parameters.
5. **DCS-OUTPUT-PARAMS-01** — outputParameters end-to-end.
6. **DCS-LINK-OPTIONAL-01** — optional link properties.
7. **DCS-FORM-SERVERSTATE-01** — clean-room `UniversalListServerOnlyState`.
8. **DCS-UNKNOWN-INVENTORY-01** — Haiku read-only inventory, Fable classification.
9. **DCS-LEGACY-REMOVAL-01** — удаление remaining schema/settings fallbacks.
10. **DCS-EDT-CLOSURE-01** — actual EDT import/export и parity report.

Порядок зависит от evidence. Если ближайшая координата требует неподтверждённого format fact, сначала выполняется отдельный evidence-only milestone.

## 17. Рекомендация по моделям Anthropic

Для программы рекомендуется трёхуровневая схема:

- **Claude Fable 5** — технический руководитель: архитектура, evidence adjudication, data-loss boundaries, финальный review;
- **Claude Sonnet 5** — основной исполнитель bounded implementation;
- **Claude Haiku 4.5** — слабая/дешёвая модель для строго механических, автоматически проверяемых задач.

Если Fable недоступна, её задачи можно временно отдать наиболее сильной доступной Opus-модели, но не Haiku. Слабая модель экономит бюджет только тогда, когда контракт уже закрыт и результат можно проверить без интерпретации.

Официальные страницы моделей:

- [Claude Fable 5](https://www.anthropic.com/claude/fable)
- [Claude Sonnet 5](https://www.anthropic.com/news/claude-sonnet-5)
- [Claude Haiku 4.5](https://www.anthropic.com/claude/haiku)

---

**Главное правило для передаваемой реализации:** никакая модель, включая Fable, не получает право расширить emission surface на основании правдоподобия. Право на запись появляется только после evidence, policy, обоих направлений и fail-closed тестов.
