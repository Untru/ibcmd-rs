# Fast parity evidence — план реализации

**Цель:** получить воспроизводимый секундный цикл platform-authenticated
raw-to-XML проверки на 8.3.27.2214 / XML 2.20.

**Дизайн:** `openspec/changes/add-fast-parity-evidence/design.md`

## Задача 1: Получить нативный micro-corpus

**Статус:** `[x]`

- [x] Создан чистый Task seed через публичный Unica workflow.
- [x] Выполнены два изолированных native import/export round-trip.
- [x] Stable source trees совпадают при исключении `ConfigDumpInfo.xml`.

## Задача 2: Зафиксировать provenance и bytes

**Статус:** `[x]`

- [x] Добавлены CF, raw Task entry, native XML/BSL и manifest.
- [x] Зафиксированы platform, Unica lineage, команды, размеры и SHA-256.

## Задача 3: Перенести доказанные правила в schema layer

**Статус:** `[x]`

- [x] Task scalar mappings извлекаются fail-closed через `ibcmd-schema`.
- [x] Adapter больше не содержит ошибочные inline mappings для этих slots.
- [x] Полный emitted Task XML байтово совпадает с native output.

## Задача 4: Добавить быстрый runner

**Статус:** `[x]`

- [x] Runner проверяет manifest hashes и декодирует committed CF.
- [x] Offline export не имеет failed entries.
- [x] Selected Task XML/BSL совпадают с native outputs.

## Задача 5: Финальная проверка на Windows VM

**Статус:** `[x]`

- [x] `cargo fmt`, focused tests и clippy проходят на Windows.
- [x] OpenSpec strict и corpus runner проходят.
- [x] CI запускает strict OpenSpec и parity runner на Linux и Windows.
- [x] Evidence опубликовано в #318 и канонической памяти #276.
