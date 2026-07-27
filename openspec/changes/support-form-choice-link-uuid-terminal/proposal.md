# Предложение: поддержать UUID-terminal в ChoiceParameterLinks

## Зачем

Полный immutable-прогон УТ на commit `0fbca11` доказал новый физический
вариант `InputFieldExtInfo.choiceParameterLinks`:

```text
mode=2, owner={numeric-form-attribute-id}, terminal={0,non-nil-uuid}
```

Текущий strict decoder принимает для `mode=2` только terminal `{-5}` или
`{-8}` и поэтому классифицирует три зеркальные пары `5006/5007` как
`PrimaryMalformed`. Экспорт останавливается fail-closed до writer.

Следующий immutable run `20260727_full_78c5be6_j_ut` доказал ещё один exact
standard-marker: `{-3}` в ссылке `Дата`, которой native XML сопоставляет
`Объект.Date`. Это расширение остаётся перечислимым typed-профилем, а не
fallback для произвольных отрицательных значений.

## Что меняется

- Physical decoder различает standard-marker terminal и UUID-terminal.
- Standard markers перечислены явно: `-3 → Date`, `-5 → Owner`, `-8 → Ref`.
- Зеркала `5006/5007` по-прежнему разбираются независимо и сравниваются до
  semantic resolution.
- UUID-terminal разрешается через существующие schema-owned metadata owner
  bindings и `object_refs`, без имён/UUID прикладных объектов в production.
- Absent, ambiguous, foreign-owner и malformed UUID остаются typed error и
  opaque/fail-closed.
- Writer получает только канонический `FormChoiceParameterLink` и не знает raw
  slot indices.
- Диагностика публикует класс parse error без raw payload.

## Не входит

- Ослабление mirror/arity/count/duplicate-tail проверок.
- Fallback на строковое редактирование XML.
- Правила по имени формы, документа или конкретному UUID.
- Изменение EDT research corpus.

## Evidence

- run:
  `E:\ibcmd_lab\parity\ut_ibcmd_20260726_full_0fbca11_d_ut`;
- git SHA: `0fbca1118166e4a3decc20ab78efd23120018dfe`;
- native export: 49 623 файла;
- три непустых пары `5006/5007`, 44 пустые пары в том же exact physical
  profile;
- native XML и EDT-derived writer evidence совпадают по wrapper/item/order.
