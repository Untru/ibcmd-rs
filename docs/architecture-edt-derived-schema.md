# Архитектура EDT-derived schema

`ibcmd-rs` использует EDT только как исследовательский эталон. Установленная EDT
не требуется для сборки, запуска или тестирования проекта.

## Границы слоёв

- `ibcmd-core` — каноническая семантическая модель и provenance.
- `ibcmd-schema` — встроенные версионированные знания о model types и поведении
  XML writers.
- `ibcmd-xml` — чтение и запись XML по правилам `ibcmd-schema`.
- `ibcmd-v8`, `ibcmd-cf`, MSSQL adapters — декодирование физических форматов в
  каноническую модель.

Числовой slot физического blob не может напрямую определять порядок XML.
XML writer не должен угадывать семантику slot.

## Что хранится в репозитории

- очищенный список bundle/model/import/export class names;
- идентификаторы EPackage classifiers, features и operations;
- проверенные декларативные правила writer-классов;
- версия источника и способ получения;
- тесты полноты, уникальности и отсутствия абсолютных путей.

Не хранятся JAR, bytecode, native libraries и исходный код EDT.

Текущий снимок EDT 2025.2.3 содержит 67 EPackage, 1 845 classifiers,
12 224 feature IDs и 1 447 operation IDs. Это структурные идентификаторы
моделей; XML QName, default и version behaviour пополняются отдельными
проверенными writer rules.

## Обновление corpus

Исследовательская машина с EDT сначала строит inventory. Затем:

```powershell
pwsh ./tools/import-edt-model-inventory.ps1 `
  -InputInventory "C:\path\to\inventory.json" `
  -OutputInventory "./crates/ibcmd-schema/data/edt-2025.2.3-model-inventory.json"

pwsh ./tools/import-edt-package-features.ps1 `
  -InputInventory "C:\path\to\inventory.json" `
  -OutputFeatures "./crates/ibcmd-schema/data/edt-2025.2.3-package-features.json"
```

Сгенерированный JSON проходит тесты `ibcmd-schema` и проверяется как обычное
изменение исходников. Скрипт не участвует в default build.

## Правило приёма новых знаний

Новое правило допустимо, если есть хотя бы одно из доказательств:

1. подтверждённая логика EDT importer/writer;
2. согласованный результат IBCMD и EDT roundtrip;
3. полный корпус положительных и отрицательных примеров двух баз.

Один XML diff не является достаточным доказательством.
