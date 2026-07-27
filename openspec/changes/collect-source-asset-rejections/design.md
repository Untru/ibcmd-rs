# Дизайн: collect-all диагностика source assets

## Поток

```text
Form decoder/writer
  -> Emitted ---------------------------> write Form.xml
  -> OpaqueNotEmitted ------------------> structured partial entry
  -> Rejected + structured diagnostics
       -> default ----------------------> fail fast
       -> collect-all ------------------> structured partial entry
  -> Rejected without diagnostics ------> always fatal

all rows
  -> RootMetadataInventoryReport + SourceAssetCompletenessReport
  -> deterministic diagnostic clusters
  -> manifest.json
  -> evaluate every requested require-complete gate
       all complete -> success
       any partial  -> aggregated error
```

## Решения

### Явный режим

CLI-флаг `--collect-all-source-asset-diagnostics` требует:

- `--extract-metadata-xml`;
- `--no-binary-rows`;
- полный Config export без `--file-name` и `--file-name-list`.

Он может использоваться вместе с `--require-complete-source-assets`.
Совместное использование означает «собрать полный отчёт и затем строго
отклонить partial result», а не успешный выпуск.

### Граница восстанавливаемой ошибки

Восстанавливается только `DetailedFormBodyExtraction::Rejected`, если decoder
уже вернул хотя бы один `FormSourceAssetDiagnostic`. Это доказывает, что отказ
принадлежит известному source-property профилю.

Пустая диагностика, malformed container, arbitrary decoder error, I/O, SQL/BCP
и invariant failure не переводятся в partial.

### Кластеризация

`SourceAssetCompletenessReport` schema v2 содержит
`diagnostic_clusters`. Ключ:

```text
family + code + classification + parse_error_class + property + property_profile
```

Кластеры и samples сортируются лексикографически. В manifest хранятся только
безопасные признаки: путь, row id, item tag, slot, raw length и SHA-256.
Raw payload и formatted error text запрещены.

Количество кластеров и samples ограничено константами. Полные totals
сохраняются, overflow указывается явно.

### Release gate

`SourceAssetCompletenessReport::ensure_complete(true)` остаётся единственным
финальным source-asset gate. Любой collected rejection увеличивает partial
counts, поэтому gate обязательно возвращает ошибку.

`RootMetadataInventoryReport::ensure_complete(true)` сохраняет прежнюю
строгость. В collect-all режиме проверка откладывается до записи manifest и
выполняется вместе с source-asset gate. Это позволяет сохранить evidence обоих
контрактов, но команда успешна только когда прошли оба затребованных gate.

Parity manifest при этом может ссылаться на уже записанный candidate manifest,
но статусы шага и запуска остаются `failed`, а `release_eligible=false`.

## Проверка

- CLI принимает только full-scope допустимую комбинацию.
- Два диагностируемых rejected assets собираются за один проход.
- Следующий корректный asset после rejection записывается.
- Default mode сохраняет fail-fast.
- Rejection без structured diagnostics остаётся fatal.
- Кластеры детерминированы при разном порядке merge и не содержат raw payload.
- Collect-all + strict записывает manifest, затем возвращает ошибку.
- Одновременный partial root-metadata и partial source-assets сохраняет оба
  отчёта в manifest и возвращает агрегированную strict-ошибку.
