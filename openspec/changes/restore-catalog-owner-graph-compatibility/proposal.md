# Предложение: восстановить совместимость compact Catalog owner graph

## Зачем

Полный unit baseline `e63d244` выявил общий кластер: 114 сохранённых Catalog
fixtures используют доказанный compact root `{1,{owner fields}}`, тогда как
новый owner-graph decoder принимал только root с пятью дочерними коллекциями.
Из-за этого extraction завершался до проверки семантики полей.

## Что меняется

- Schema layer классифицирует только точные Catalog layouts 56/57 и их
  физические sentinel-профили.
- Physical adapter принимает compact root только для Catalog и только для
  60–62 owner fields.
- Отсутствующие в compact root коллекции материализуются как типизированные
  пустые коллекции с обычным provenance.
- Input/history tail декодируется по schema-owned layout, без UUID или имён
  прикладных объектов.
- Все другие family/root/count варианты по-прежнему fail closed.

## Evidence

- baseline: `1810 passed / 100 failed`;
- после среза: `1814 passed / 97 failed`;
- `catalog_input_history_tail`: 7/7;
- physical-adapter policy gate: passed.
