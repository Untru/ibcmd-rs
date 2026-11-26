# Selected CF extract — план реализации

## Задача 1: CLI и bounded extraction

**Статус:** `[x]`

- [x] Добавить exact-element CLI с явным compression profile.
- [x] Читать и декодировать только выбранный payload.

## Задача 2: Fail-closed publication

**Статус:** `[x]`

- [x] Публиковать packed/unpacked bytes только в новый каталог.
- [x] Проверить отказ без изменения существующего каталога.

## Задача 3: Native #317 evidence

**Статус:** `[x]`

- [x] Извлечь точный Task UUID из CF, сохранённого 8.3.27.2214.
- [x] Зафиксировать hashes и parity с native XML 2.20.
