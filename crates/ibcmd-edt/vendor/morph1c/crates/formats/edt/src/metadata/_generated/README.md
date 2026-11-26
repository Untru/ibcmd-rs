# `_generated/` — ЧЕРНОВЫЕ EDT-ПРОЕКЦИИ из метамодели (Phase 2; §1.0: НЕ активны)

**Черновики НЕ коммитятся** (устаревают по мере роста субстрата) — их РЕГЕНЕРИРУЮТ по требованию
перед fan-out (`python tools/gen_projection.py --only <Вид>`), чтобы черновик отражал ТЕКУЩИЙ
субстрат/метамодель, а не устаревший снимок. Генератор пишет **авто-сгенерированные** черновики
`formats/edt/src/metadata/<вид>.rs` — по одному на вид: карта `LocusMap` (`FieldId → (XmlLocus,
Codec)`) для EDT-диалекта.

## Откуда

`tools/gen_projection.py` парсит УЖЕ-сгенерированный канонический черновик спека
(`crates/core/src/spec/metadata/_generated/<вид>.rs`) — берёт РАЗРЕШЁННЫЙ `ValueKind` каждого
поля (Bool/Str/Enum/Int/Localized/Type/Value) и применяет таблицу
**value_kind → (имя-тега, кодек)**, ВЫВЕДЕННУЮ из hand-written проекций уже-done видов
(SessionParameter/DefinedType/Constant/CommonModule):

| ValueKind | EDT тег          | EDT кодек               |
|-----------|------------------|-------------------------|
| Localized | `<name>` verbatim | `LocalizedKeyVal`      |
| Str       | `<name>`          | `PlainText`             |
| Bool      | `<name>`          | `BoolPresence`          |
| Enum      | `<name>`          | `EnumText`              |
| Int       | `<name>`          | `IntText`               |
| Type      | `<name>`          | `Type(TypeDialect::Edt)`|
| Value     | `<name>`          | `Value(ValueDialect::Edt)`|

Имя тега EDT = имя поля ВЕРБАТИМ (lowerCamelCase), путь = `[name]`, `ns=""`.

## Почему НЕ компилируются (намеренно, §1.0)

`crates/formats/edt/build.rs` сканирует `src/metadata` **нерекурсивно** (`read_dir` + фильтр
`extension()==rs`) и регистрирует только `*.rs` В КОРНЕ каталога. Эта подпапка в граф модулей
(`metadata/mod.rs` / `FORMAT_KINDS`) НЕ попадает → черновики НЕ компилируются и НЕ могут быть
случайно приняты как готовые.

## Как ими пользоваться (контракт fan-out агента)

1. Взять черновик `_generated/<вид>.rs` как отправную точку EDT-проекции.
2. Скопировать в `src/metadata/<вид>.rs` (в граф модулей → компиляция + `HARNESS_ENTRY`).
3. Снять/проверить каждый `// TODO(draft)` (List/Ref/Blob кодеки, root-ns, x_ignored) — их
   ~10-20% на структурный вид; leaf-виды (SessionParameter/DefinedType) — 0 TODO.
4. Прогнать Tier 1 (`MORPH1C_ONLY_KINDS=<Вид>`) byte-exact R/X-гейт; поправить 2-3 нерегулярных
   поля; довести до бинарной приёмки → только тогда `done`.

**Гейт-факт (`gen_projection.py --gate`):** на 4 done-видах × 2 диалекта генератор воспроизводит
ВСЕ mapped-поля hand-written проекций byte-identically (SessionParameter 3/3, DefinedType 3/3,
Constant 29/29, CommonModule 10/10). Единственная разница — metamodel-superset поля
`objectBelonging`/`extendedConfigurationObject` (T1-хвост, ещё не адоптированы каноническим спеком).

## Регенерация

```
python tools/gen_projection.py            # все виды из _generated core-спеков
python tools/gen_projection.py --gate     # сверить с hand-written done-видами
```
