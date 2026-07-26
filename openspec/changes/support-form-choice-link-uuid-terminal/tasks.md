# UUID-terminal ChoiceParameterLinks — план реализации

## Задача 1: Расширить schema-owned physical decoder

- [x] Ввести typed terminal variant для absent, standard marker и UUID.
- [x] Строго разобрать `{0,non-nil-uuid}` без расширения допустимой arity.
- [x] Сравнивать полностью разобранные 5006/5007 до semantic resolution.
- [x] Сохранить прежний public standard-marker entrypoint.

## Задача 2: Подключить owner-scoped resolver

- [x] Передать в canonical adapter metadata owner bindings и object refs.
- [x] Разрешать UUID через существующий metadata data-path route.
- [x] Отклонять unknown, ambiguous и foreign-owner references.
- [x] Не добавлять special cases по имени/UUID объекта.

## Задача 3: Добавить доказательные тесты

- [x] Sanitized live-shaped 2-link и 1-link пары дают canonical links.
- [x] Existing `{-5|-8}` profile не регрессирует.
- [x] Mirror mismatch, nil/non-UUID, wrong kind/arity и duplicate tail fail
      closed.
- [x] Missing/foreign owner resolution остаётся opaque.
- [x] Native XML order `Name → DataPath → ValueChange` побайтово совпадает.

## Задача 4: Улучшить диагностику

- [x] Source-asset diagnostic содержит parse-error class без raw value.
- [x] Opaque payload не попадает в manifest/journal.

## Задача 5: Проверить production blocker

- [x] Focused schema/form tests проходят.
- [x] Полный candidate export УТ проходит этот source asset без special case.
- [x] Следующий blocker либо полный diff зафиксирован новым immutable run.
- [ ] OpenSpec strict validation проходит.
