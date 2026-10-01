# `_generated/` — ЧЕРНОВЫЕ Designer-ПРОЕКЦИИ из метамодели (Phase 2; §1.0: НЕ активны)

**Черновики НЕ коммитятся** (устаревают по мере роста субстрата) — регенерируй по требованию
перед fan-out (`python tools/gen_projection.py --only <Вид>`). Генератор пишет
**авто-сгенерированные** черновики `formats/designer/src/metadata/<вид>.rs` — по одному на вид:
карта `LocusMap` (`FieldId → (XmlLocus, Codec)`) для Designer-диалекта.

## Откуда

`tools/gen_projection.py` парсит УЖЕ-сгенерированный канонический черновик спека
(`crates/core/src/spec/metadata/_generated/<вид>.rs`) — берёт РАЗРЕШЁННЫЙ `ValueKind` каждого
поля и применяет таблицу **value_kind → (имя-тега, кодек)**, ВЫВЕДЕННУЮ из hand-written проекций
уже-done видов (SessionParameter/DefinedType/Constant/CommonModule):

| ValueKind | Designer тег       | Designer кодек              |
|-----------|--------------------|-----------------------------|
| Localized | `<Name>` UpperCamel | `LocalizedV8`              |
| Str       | `<Name>`            | `PlainText`                 |
| Bool      | `<Name>`            | `BoolText`                  |
| Enum      | `<Name>`            | `EnumText`                  |
| Int       | `<Name>`            | `IntText`                   |
| Type      | `<Name>`            | `Type(TypeDialect::Designer)`|
| Value     | `<Name>`            | `Value(ValueDialect::Designer)`|

Имя тега Designer = UpperCamelCase (первая буква вверх), путь = `["<Вид>", "Properties", "<Tag>"]`,
`ns=""` (property-теги — дефолтный MDClasses-ns; внутренности v8/xsi разбирают кодеки).

## Почему НЕ компилируются (намеренно, §1.0)

`crates/formats/designer/build.rs` сканирует `src/metadata` **нерекурсивно** и регистрирует
только `*.rs` В КОРНЕ каталога. Эта подпапка в граф модулей НЕ попадает → черновики НЕ
компилируются и НЕ могут быть случайно приняты как готовые.

## Как ими пользоваться (контракт fan-out агента)

1. Взять черновик `_generated/<вид>.rs` как отправную точку Designer-проекции.
2. Скопировать в `src/metadata/<вид>.rs` (в граф модулей → компиляция + `HARNESS_ENTRY`).
3. Снять/проверить каждый `// TODO(draft)` (List/Ref/Blob кодеки, x_ignored, DENSE-порядок).
4. Прогнать Tier 1 (`MORPH1C_ONLY_KINDS=<Вид>`) byte-exact R/X-гейт; поправить нерегулярное;
   довести до бинарной приёмки → только тогда `done`.

**Гейт-факт (`gen_projection.py --gate`):** на 4 done-видах × 2 диалекта генератор воспроизводит
ВСЕ mapped-поля hand-written проекций byte-identically (см. `../../../edt/src/metadata/_generated/README.md`).

## Регенерация

```
python tools/gen_projection.py            # все виды из _generated core-спеков
python tools/gen_projection.py --gate     # сверить с hand-written done-видами
```
