# Дизайн: canonical coverage map

Ключ mapping совпадает с schema identity:

```text
namespace URI / classifier / feature
```

Запись содержит canonical family/type/field, preservation status, evidence и
reason. `opaque-lossless` требует placement/provenance contract.
`unsupported` требует diagnostic code. `platform-only` запрещён в portable
writer path.

Coverage validator выполняет полный join с Xcore corpus:

- feature без mapping — ошибка;
- mapping без feature — stale entry;
- duplicate key — ошибка;
- неизвестный package/classifier-kind route — ошибка без fallback в `other`;
- агрегаты и migration backlog сохраняются в corpus, но валидатор независимо
  пересчитывает их из полного join;
- порядок семейств фиксирован:
  metadata/forms/DCS/MXL/common/other;
- backlog строится только из `unsupported/schema.unmapped` и группируется по
  rule/package/classifier-kind/feature-kind, не по именам объектов, features
  или файлов.

Публичный JSON parser выполняет bounded streaming preflight до создания
полного object graph. Preflight ограничивает размер документа, строк и всех
внешне управляемых массивов и отклоняет unknown/duplicate fields. Он не
сохраняет строки и элементы массивов после проверки, поэтому отклонение
oversized input не требует unbounded retention.

Все route/key lookup tables генератора используют
`StringComparer.Ordinal`. Package/classifier identity регистрозависима:
регистровая мутация не может совпасть с известным route.
