# Дизайн: TableCurrentData ChoiceParameterLinks

## Граница слоёв

```text
raw 5006/5007
  -> ibcmd-schema exact physical decoder
  -> typed TableCurrentData(table_id, column_id)
  -> mssql form-index resolver
  -> canonical FormChoiceParameterLink
  -> ibcmd-xml writer
```

Schema-layer принимает новый профиль только при точном совпадении:

- mode равен `2`;
- owner содержит canonical positive table id и exact
  `02023637-7868-4a5f-8576-835a76e0c9ba`;
- terminal содержит ровно один canonical positive column/binding id;
- duplicate имеет два пустых хвостовых значения;
- полностью разобранные primary и duplicate равны.

Adapter сначала использует доказанный
`type_link_data_path_by_table_column`, затем общий
`resolve_form_item_current_data_path`. Оба маршрута основаны на индексах,
построенных из той же формы. Отсутствие table/column id не даёт частичный XML.

## Разделение UUID-смыслов

UUID второго поля owner — идентификатор платформенного типа элемента формы.
Он не передаётся в `object_refs`, не считается metadata UUID и не участвует в
owner-scoped metadata resolution.

Профиль metadata UUID-terminal остаётся отдельным:

```text
owner={form-attribute-id}, terminal={0,metadata-uuid}
```

## Совместимость

Прежний standard-marker API сохраняется. Production typed-resolver получает
достаточно данных для выбора одного из путей: direct attribute, standard
terminal, metadata UUID либо TableCurrentData.
