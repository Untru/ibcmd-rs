# ChoiceParameterLinks evidence — план реализации

**Цель:** получить автономный fail-closed evidence slice без подключения к
production emission.

## Задача 1: Зафиксировать research-only контракт

**Статус:** `[x]`

**Файлы:**
- `openspec/changes/extract-form-choice-parameter-links-evidence/`

**Проверка:**
- [x] Перечислены exact keys и fail-closed условия.
- [x] Production writer, coverage и baseline явно вне scope.

## Задача 2: Реализовать extractor

**Статус:** `[x]`

**Файлы:**
- `tools/report-edt-form-choice-parameter-links-evidence.ps1`

**Проверка:**
- [x] Exact EDT release и bundle versions проверяются до анализа.
- [x] Inventory принимается только как top-level JSON array.
- [x] Каждый анализируемый method block имеет exact JVM descriptor.
- [x] Owner wrapper QName/prefix/item QName/empty/null/version/order
  извлекаются из provider и writer bytecode.
- [x] QName provider chain включает exact superclass, absence of relevant
  override, base constructor virtual fill calls, полный feature-map envelope
  и base fallback calls.
- [x] Проверен порядок `name → datapath → changeMode`.
- [x] Name, datapath, changeMode и extension behaviour имеют точный evidence.
- [x] Любая неоднозначность mandatory evidence завершает extractor ошибкой.

## Задача 3: Добавить synthetic javap selftests

**Статус:** `[x]`

**Файлы:**
- `tools/report-edt-form-choice-parameter-links-evidence.ps1`

**Проверка:**
- [x] Positive fixtures вызывают owner/item/order/extension fact extractors.
- [x] Wrong release/bundle/descriptor, missing/ambiguous method/instruction,
  wrong order, control-flow, QName, default, delegate и extension disagreement
  fixtures fail closed.
- [x] Name/changeMode default negatives вызывают общий helper, используемый
  real `Get-ModelFacts`, а не отдельную opcode assertion.

## Задача 4: Сформировать очищенный corpus

**Статус:** `[x]`

**Файлы:**
- `crates/ibcmd-schema/data/edt-2025.2.3-form-choice-parameter-links-writer-evidence.json`

**Проверка:**
- [x] JSON не содержит локальных путей, JAR/class payload или timestamps.
- [x] Два независимых extraction processes дают byte-identical output.
- [x] Недоказанные свойства представлены explicit `not-proven`.

## Задача 5: Выполнить focused validation

**Статус:** `[x]`

**Проверка:**
- [x] Extractor selftests проходят.
- [x] Real EDT extraction совпадает с repository research JSON.
- [x] Corpus governance и strict OpenSpec validation проходят.
- [x] Production form writer, coverage и baseline не изменены.
