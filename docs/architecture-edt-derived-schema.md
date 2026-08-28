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
- очищенная семантика Xcore features: kind, model type, cardinality,
  qualifiers и явно заданный default;
- проверенные декларативные правила writer-классов;
- версия источника и способ получения;
- тесты полноты, уникальности и отсутствия абсолютных путей.

Не хранятся JAR, bytecode, native libraries и исходный код EDT.

Текущий снимок EDT 2025.2.3 содержит 67 EPackage, 1 845 classifiers,
12 224 feature IDs и 1 447 operation IDs. Это структурные идентификаторы
моделей.

Полный доступный Xcore-снимок содержит 63 packages, 1 820 classifiers и
4 966 локально объявленных features: 3 425 attributes, 214 references и
1 327 containments. В нём зафиксированы 585 явно заданных defaults. Модели,
для которых EDT inventory не содержит Xcore-ресурс, в этот corpus не
выдумываются. Неподтверждённые XML QName, order, default-emission, version
gate и delegate имеют статус `pending` и не могут использоваться как
production-правило.

Каждый из 4 966 feature имеет запись в canonical coverage map. Два поля
`DataCompositionSettings` представлены типизированной `DcsSettings`; остальные
4 964 ещё не подключённых features помечены как `unsupported` с диагностикой
`schema.unmapped`. Полнота карты не означает готовность реализации. Отдельный
metadata-order corpus содержит 60
подтверждённых записей: 40 property orders, 4 special cases `internalInfo` и
16 таблиц `producedTypes`. Связь constructor `InvokeDynamic` с конкретным
`get*`-методом provider доказывается через constant pool и `BootstrapMethods`;
неизвестная или неоднозначная форма bytecode по-прежнему отклоняется
fail-closed. Порядок не используется как доказательство QName, nil,
default-emission или иных XML-правил.

## Обновление corpus

Исследовательская машина с EDT сначала строит inventory. Затем:

```powershell
pwsh ./tools/import-edt-model-inventory.ps1 `
  -InputInventory "C:\path\to\inventory.json" `
  -OutputInventory "./crates/ibcmd-schema/data/edt-2025.2.3-model-inventory.json"

pwsh ./tools/import-edt-package-features.ps1 `
  -InputInventory "C:\path\to\inventory.json" `
  -OutputFeatures "./crates/ibcmd-schema/data/edt-2025.2.3-package-features.json"

pwsh ./tools/import-edt-xcore-semantics.ps1 `
  -InputInventory "C:\path\to\inventory.json" `
  -OutputSemantics "./crates/ibcmd-schema/data/edt-2025.2.3-feature-semantics.json"

pwsh ./tools/generate-canonical-coverage.ps1 `
  -InputFeatureSemantics "./crates/ibcmd-schema/data/edt-2025.2.3-feature-semantics.json" `
  -OutputCoverage "./crates/ibcmd-schema/data/edt-2025.2.3-canonical-coverage.json"

pwsh ./tools/import-edt-metadata-order.ps1 `
  -InputInventory "C:\path\to\inventory.json" `
  -OutputOrder "./crates/ibcmd-schema/data/edt-2025.2.3-metadata-order.json"
```

Сгенерированный JSON проходит тесты `ibcmd-schema` и проверяется как обычное
изменение исходников. Скрипты не участвуют в default build. Xcore, JAR,
class-файлы и абсолютные пути в репозиторий не копируются.

## Governance очищенного corpus

CI запускает `tools/validate-edt-corpus.ps1` в Linux и Windows matrix до
offline-сборки. Валидатор работает без EDT и сети: он проверяет каждый
отслеживаемый файл на запрещённые расширения и сигнатуры ZIP/JAR, Java class,
PE, ELF и Mach-O независимо от имени файла, а также отклоняет `.xcore` и
reparse-point. Затем отдельно проверяется JSON corpus в
`crates/ibcmd-schema/data`. Отклоняются drive/UNC/POSIX пути и `file:` URI, а
также corpus без версии источника.

Provenance `verified` факта — непустой массив portable строк `sources`; вложенный
объект или произвольное поле `provenance` не считаются доказательством. Узкое
исключение существует только для исторического `rules[].evidence` в writer-rule
schema: там проверяется точная тройка `status`, `kind`, `note`. После gate CI
запускает `cargo test --locked -p ibcmd-schema`, чтобы Rust schema валидировала
структуру и агрегаты committed corpus. Статус `pending` не является production
знанием и не может быть повышен одной догадкой или XML diff.

## Правило приёма новых знаний

Новое правило допустимо, если есть хотя бы одно из доказательств:

1. подтверждённая логика EDT importer/writer;
2. согласованный результат IBCMD и EDT roundtrip;
3. полный корпус положительных и отрицательных примеров двух баз.

Один XML diff не является достаточным доказательством: он показывает лишь
конкретный наблюдаемый результат и не отличает правило модели от побочного
эффекта версии, порядка записи или исходного состояния объекта. Для принятия
правила нужен воспроизводимый источник либо сопоставление нескольких
независимых примеров с явно сохранённой provenance.
