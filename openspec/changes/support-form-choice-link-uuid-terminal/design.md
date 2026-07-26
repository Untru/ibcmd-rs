# Дизайн: UUID-terminal ChoiceParameterLinks

## Граница слоёв

```text
raw slots 26/64
  -> ibcmd-schema strict mirrored decoder
  -> typed terminal (attribute | standard marker | metadata UUID)
  -> mssql physical adapter resolver
  -> canonical FormChoiceParameterLink
  -> ibcmd-xml writer
```

`ibcmd-schema` проверяет только физическую грамматику: marker, count, arity,
mode, numeric owner id, exact terminal shape, non-nil UUID, value-change и
duplicate tail. Он не разрешает metadata path.

Adapter разрешает:

- `mode=1` через form attribute id;
- `mode=2 + {-5|-8}` через существующую owner-scoped standard-attribute
  модель;
- `mode=2 + {0,uuid}` через
  `FormAttributeMetadataOwner + object_refs + form_metadata_data_path_route`.

Resolver обязан проверить владельца. Одного наличия UUID в `object_refs`
недостаточно.

## Совместимость API

Существующий `parse_form_choice_parameter_links` сохраняется как строгий
wrapper для прежнего standard-marker профиля. Новый typed-resolver entrypoint
используется production adapter и тестами UUID-terminal.

## Fail-closed

Следующие случаи не становятся `Empty` и не подавляются:

- неполная пара slots 26/64;
- malformed/non-canonical/nil UUID;
- разный terminal в 5006 и 5007;
- неизвестный либо foreign-owner UUID;
- неверный kind вместо `0`;
- лишние поля;
- непустой duplicate tail.

В этих случаях source-asset validation возвращает typed opaque diagnostic без
raw payload.
