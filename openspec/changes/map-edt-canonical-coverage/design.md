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
- агрегаты вычисляются, а не задаются вручную.
