# Предложение: поддержать TableCurrentData в ChoiceParameterLinks

## Зачем

Полный immutable-прогон УТ на commit `2c9d836` прошёл ранее исправленную форму,
но доказал следующий физический вариант `InputFieldExtInfo.choiceParameterLinks`:

```text
mode=2,
owner={positive-table-item-id,02023637-7868-4a5f-8576-835a76e0c9ba},
terminal={positive-column-id}
```

Текущий decoder требует одно поле в owner и поэтому корректно останавливает
экспорт как `PrimaryMalformed`. Native XML показывает пути вида
`Items.<Table>.CurrentData.<Column>`.

## Что меняется

- Physical decoder вводит отдельный typed-вариант `TableCurrentData`.
- Платформенный UUID типа элемента формы проверяется как точная константа
  физической схемы, а не разрешается как UUID объекта метаданных.
- Adapter разрешает table/column ids через существующие индексы элементов
  формы и колонок.
- Доказанный последующим run вариант terminal `{0,metadata-binding-uuid}`
  разрешается через отдельный однозначный индекс связи таблицы и её дочернего
  поля.
- Вариант `{positive-binding-id,metadata-binding-uuid}` использует тот же
  индекс и при наличии numeric route требует совпадения канонических путей.
- Зеркала `5006/5007` сравниваются до semantic resolution.
- Неизвестные table/column ids остаются opaque/fail-closed.

## Не входит

- Fallback по имени формы, таблицы или колонки.
- Принятие произвольного UUID во втором поле owner.
- Смешивание с профилем metadata UUID-terminal `{0,uuid}`.
- Ослабление count/arity/value-change/duplicate-tail проверок.

## Evidence

- run:
  `E:\ibcmd_lab\parity\ut_ibcmd_20260726_full_2c9d836_e_ut`;
- git SHA: `2c9d836ed4c6f6dcdf520ab33e30c63fca2b98ce`;
- native export: 49 623 файла;
- source form UUID: `09ae274e-3a9e-4f3f-9f1b-859e16706917`;
- две exact mirrored пары TableCurrentData и две пары уже поддержанного
  direct-attribute профиля;
- native XML однозначно связывает table id `1050` / column id `21` и
  table id `785` / column id `12` с каноническими путями.

Последующий run
`E:\ibcmd_lab\parity\ut_ibcmd_20260726_full_22d9fe6_f_ut` доказал второй
terminal того же TableCurrentData owner:

```text
terminal={0,461bb43b-8803-4f48-811f-6beef397ee4c}
```

Raw child binding той же формы связывает этот UUID с таблицей item `81` и
полем `Отправитель`; native XML даёт
`Items.ВыбранныеОтправители.CurrentData.Отправитель`.

Run `E:\ibcmd_lab\parity\ut_ibcmd_20260726_full_54ca4ee_g_ut` доказал три
варианта `{positive-binding-id,uuid}` для таблиц формы `НастройкиРМК`.
