# Дизайн: TableCurrentData ChoiceParameterLinks

## Граница слоёв

```text
raw 5006/5007
  -> ibcmd-schema exact physical decoder
  -> typed TableCurrentData(table_id, BindingId | MetadataUuid)
  -> mssql form-index resolver
  -> canonical FormChoiceParameterLink
  -> ibcmd-xml writer
```

Schema-layer принимает новый профиль только при точном совпадении:

- mode равен `2`;
- owner содержит canonical positive table id и exact
  `02023637-7868-4a5f-8576-835a76e0c9ba`;
- terminal содержит либо один canonical positive column id, либо точную пару
  `{0,canonical-lowercase-non-nil-uuid}`;
- duplicate имеет два пустых хвостовых значения;
- полностью разобранные primary и duplicate равны.

Для numeric BindingId adapter сначала использует доказанный
`type_link_data_path_by_table_column`, затем общий
`resolve_form_item_current_data_path`.

Для MetadataUuid adapter строит однозначный маршрут из двух raw-связей той же
формы:

1. table item id связан со своим table binding key;
2. дочернее поле таблицы связано с тем же table binding key и UUID колонки.

Индекс имеет ключ `(table_item_id, metadata_binding_uuid)`. Разные имена для
одного ключа делают маршрут ambiguous и удаляют его из production lookup.

## Разделение UUID-смыслов

UUID второго поля owner — идентификатор платформенного типа элемента формы.
Он не передаётся в `object_refs`, не считается metadata UUID и не участвует в
owner-scoped metadata resolution.

Профиль form-attribute metadata UUID-terminal остаётся отдельным:

```text
owner={form-attribute-id}, terminal={0,metadata-uuid}
```

В TableCurrentData UUID означает binding дочернего поля формы и не проходит
через `object_refs`.

## Совместимость

Прежний standard-marker API сохраняется. Production typed-resolver получает
достаточно данных для выбора одного из путей: direct attribute, standard
terminal, metadata UUID либо TableCurrentData.
