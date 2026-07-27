# Пакетный сбор отказов source assets — план реализации

> **Для субагента:** используй
> `/subagent-dev openspec/changes/collect-source-asset-rejections/tasks.md`.

**Цель:** получить за один full-scope проход полный структурированный backlog
диагностируемых source-asset rejections, не ослабляя strict release gate.

**Дизайн:** `openspec/changes/collect-source-asset-rejections/design.md`

**Базовый SHA:** `78c5be6519e25c34fe9b494bce45782f12308318`

---

## Задача 1: Добавить CLI policy collect-all

**Статус:** `[x]`

**Зависимости:** нет

**Файлы:**
- Изменить: `src/cli.rs`
- Изменить: `src/infobase.rs`
- Изменить: `src/mssql_dump/mod.rs`

**Шаги:**
1. Добавить `collect_all_source_asset_diagnostics` в аргументы dump-config.
2. Ограничить режим full scope, metadata extraction и no-binary rows.
3. Передать policy в production row context без изменения test-only
   `continue_on_row_error`.

**Проверка:**
- [x] Валидная комбинация разбирается CLI.
- [x] Scoped и неполные комбинации отклоняются.
- [x] Флаг совместим с `require_complete_source_assets`.

**Коммит:** `feat(diagnostics): add collect-all source asset mode`

## Задача 2: Продолжить после диагностируемого form rejection

**Статус:** `[x]`

**Зависимости:** задача 1

**Файлы:**
- Изменить: `src/mssql_dump/source_assets.rs`
- Изменить: `src/mssql_dump/mod.rs`

**Шаги:**
1. В collect-all режиме преобразовать `Rejected` с непустыми diagnostics в
   отдельный not-emitted result.
2. Записать diagnostics в completeness report.
3. Сохранить fail-fast для default и для rejection без diagnostics.

**Проверка:**
- [x] Следующая строка после diagnosed rejection обрабатывается.
- [x] Rejected `Form.xml` не создаётся.
- [x] Default и undiagnosed ошибки остаются fatal.

**Коммит:** `feat(diagnostics): collect typed form rejections`

## Задача 3: Построить bounded diagnostic clusters

**Статус:** `[x]`

**Зависимости:** задача 2

**Файлы:**
- Изменить: `src/mssql_dump/mod.rs`

**Шаги:**
1. Добавить stable family token и schema-v2 cluster types.
2. Кластеризовать entries по ключу из design.
3. Ограничить clusters/samples и записать overflow totals.

**Проверка:**
- [x] Merge order не меняет JSON.
- [x] Counts не теряются при truncation.
- [x] Raw sentinel отсутствует в сериализации.

**Коммит:** `feat(diagnostics): cluster source asset failures`

## Задача 4: Сохранить evidence в parity failure path

**Статус:** `[x]`

**Зависимости:** задача 3

**Файлы:**
- Изменить: `scripts/export-ibcmd-vs-ours.ps1`
- Изменить: `tests/scripts/parity-manifest.Tests.ps1`

**Шаги:**
1. Добавить full-scope switch и CLI argument.
2. При ожидаемом strict failure прочитать уже записанный candidate manifest.
3. Сохранить source-assets evidence, оставив step/run failed и
   release-ineligible.

**Проверка:**
- [x] Partial evidence доступен после non-zero candidate export.
- [x] Strict gate не помечается passed.
- [x] Existing successful path не меняется.

**Коммит:** `feat(parity): retain collected rejection evidence`

## Задача 5: Добавить интеграционные fixtures

**Статус:** `[x]`

**Зависимости:** задачи 1–4

**Файлы:**
- Изменить: `src/mssql_dump/tests.rs` или локальный test module рядом с типами

**Шаги:**
1. Проверить два rejected и один следующий emitted asset.
2. Проверить default fail-fast и undiagnosed fatal.
3. Проверить collect-all + strict: manifest существует, команда возвращает
   completeness error.

**Проверка:**
- [x] Targeted Rust tests проходят.
- [x] Targeted Pester tests проходят.

**Коммит:** `test(diagnostics): cover collect-all source failures`

## Задача 6: Финальная валидация

**Статус:** `[ ]`

**Зависимости:** все предыдущие задачи

**Шаги:**
1. Запустить format и targeted tests.
2. Запустить policy/self-test проекта.
3. На сохранённом реальном BCP собрать все rejection clusters.
4. Только после локального gate запустить один immutable full UT diagnostic.

**Проверка:**
- [ ] OpenSpec change проходит strict validation.
- [ ] В одном manifest присутствуют все найденные кластеры.
- [ ] Strict source-asset gate завершился ошибкой при partial report.
- [ ] Полный прогон не запускался до прохождения локальных gates.

**Коммит:** `test(parity): validate collected source diagnostics`
