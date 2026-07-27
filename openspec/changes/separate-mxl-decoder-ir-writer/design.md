# Дизайн: первый безопасный MXL IR boundary

```text
compressed MOXCEL / native body
          |
          v
decoder -> canonical spreadsheet fields + palette provenance + format map
          |
          v
write plan (output order and index map)
          |
          v
existing evidenced XML projection
```

## Граница

Decoder остаётся владельцем разбора контейнера, native braces, palette slots и
canonical/XML format mapping. Он строит `MxlSpreadsheetWritePlan`; identity
relation сохраняется как явное значение, а не как отсутствие map. XML writer
получает только canonical spreadsheet и этот plan. Он не просматривает raw
payload, не выбирает другую palette и не вызывает legacy эвристику определения
`formatIndex`.

Первый срез намеренно сохраняет существующую строковую XML projection. Её
QName, namespace, порядок и default-ветви не переносятся и не расширяются,
поскольку этот change не добавляет evidence для таких решений.

## Диагностика

`MxlDiagnostic` содержит стабильные `(stage, code)`:

- `decoder`: container/body/IR/map defects;
- `writer`: неполный или небиективный plan, непригодный для XML projection.

Legacy `Option` adapters сохраняются для старых callers, но production MXL
paths используют fallible typed boundary до преобразования в прежний контракт.

## Риски и последующие шаги

- Оставшиеся legacy helpers допустимы только для unit tests в этом срезе;
  следующим шагом следует удалить или изолировать их после migration callers.
- Palette provenance пока несёт slots для диагностики/следующего writer slice;
  она не является разрешением менять XML colours.
- Нужны evidence fixtures для каждой новой XML writer decision. Без них
  writer возвращает typed diagnostic или сохраняет текущую подтверждённую
  projection.
