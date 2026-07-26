# TableCurrentData ChoiceParameterLinks — план реализации

## Задача 1: Schema decoder

- [x] Добавить typed `TableCurrentData(table_id, terminal)`.
- [x] Разделить terminal на `BindingId` и `MetadataUuid`.
- [x] Добавить `BindingUuid { binding_id, uuid }`.
- [x] Проверить exact platform form-item UUID и canonical positive ids.
- [x] Сравнивать полностью разобранные 5006/5007 до resolver.
- [x] Сохранить прежние public API и профили.

## Задача 2: Production adapter

- [x] Передать в resolver существующие form table/column indexes.
- [x] Разрешать путь через общие table-current-data маршруты.
- [x] Отклонять missing table/column без fallback по имени.
- [x] Построить однозначный `(table item, binding UUID)` route из form
      bindings.
- [x] Отклонять конфликтующие UUID routes.

## Задача 3: Тесты

- [x] Sanitized live-shaped пары `1050/21` и `785/12` проходят.
- [x] Wrong UUID/id/arity/tail и mirror mismatch fail closed.
- [x] Adapter выдаёт exact `Items.<Table>.CurrentData.<Column>`.
- [x] Missing table/column остаётся opaque.
- [x] Exact live hybrid pair direct + table UUID terminal проходит.
- [x] Wrong UUID kind/nil/case/arity и ambiguous route fail closed.
- [x] BindingUuid route согласуется с numeric route либо fail closed.
- [x] Старые direct, standard и metadata UUID профили не регрессируют.

## Задача 4: Production proof

- [x] Focused schema/form tests проходят.
- [x] Новый candidate export проходит source form `09ae274e-...`.
- [x] Новый candidate export проходит UUID-binding source form `0a35aae5-...`.
- [x] Следующий blocker либо полный diff зафиксирован новым immutable run.
- [ ] OpenSpec strict validation проходит.
