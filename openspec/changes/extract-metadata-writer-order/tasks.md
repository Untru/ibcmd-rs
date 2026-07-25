# Metadata writer order — план реализации

**Цель:** перенести provider order в автономный schema corpus и подключить
первые metadata families.

## Задача 1: Добавить order corpus schema

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/src/lib.rs`

**Проверка:**
- [x] Типизированы section/version/fallback/evidence.
- [x] Duplicate classifier+section+version отклоняется.

## Задача 2: Реализовать research extractor

**Статус:** `[x]`

**Файлы:** `tools/import-edt-metadata-order.ps1`

**Проверка:**
- [x] Извлекаются class-to-method, cursor/next и produced-types tables.
- [x] Unknown bytecode pattern fail-closed.
- [x] Повторная генерация детерминирована в Windows PowerShell 5.1 и
  PowerShell 7.

Verbose `javap` связывает constant-pool `InvokeDynamic` с точным
`BootstrapMethods` method handle; порядковое сопоставление не используется.

## Задача 3: Добавить verified order corpus

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-schema/data/edt-2025.2.3-metadata-order.json`

**Проверка:**
- [x] Есть Configuration, Catalog, Document.
- [x] InnerInfo special cases и producedTypes представлены явно.

## Задача 4: Подключить schema order API к metadata writer

**Статус:** `[x]`

**Файлы:** `crates/ibcmd-xml/src/metadata/`

**Проверка:**
- [x] Writer не задаёт порядок выбранных families локальным массивом.
- [x] Default/nil/QName не угадываются из order corpus.

Первый безопасный срез подключён для реально используемых
Configuration `InternalInfo` и Catalog/Document `producedTypes`: категории
сопоставляются с EReference-токенами и упорядочиваются через bundled schema API.
Неизвестные classifier/category отклоняются. Configuration properties не входят
в этот срез: XML dialect 2.20/2.21 не является доказательством platform
predicate `V8_3_14`, а полный исходный порядок EClass, необходимый для честного
применения `cursor/next`, ещё не перенесён в автономный corpus.

## Задача 5: Проверить УТ и БСП representative cohort

**Статус:** `[ ]`

**Проверка:**
- [ ] Catalog, Document, Configuration не имеют scoped regression.
- [ ] GitHub #282 содержит evidence.
