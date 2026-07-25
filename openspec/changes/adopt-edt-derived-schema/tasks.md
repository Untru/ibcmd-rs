# EDT-derived schema-first архитектура — план реализации

**Цель:** создать автономный model/writer corpus и сделать его новым источником
правил XML-сериализации.

**Дизайн:** `openspec/changes/adopt-edt-derived-schema/design.md`

## Задача 1: Создать `ibcmd-schema`

- [x] Добавить crate в workspace.
- [x] Определить типы inventory и writer rules.
- [x] Добавить строгую проверку corpus.

## Задача 2: Перенести очищенный EDT inventory

- [x] Добавить детерминированный research importer.
- [x] Удалить абсолютные пути и бинарные материалы.
- [x] Зафиксировать контрольные количества corpus.

## Задача 3: Перенести первые проверенные writer rules

- [x] ChoiceList.
- [x] ListSettings/DCS delegation.
- [x] SpreadsheetContent structural copy.

## Задача 4: Подключить XML-слой

- [x] Добавить зависимость `ibcmd-xml -> ibcmd-schema`.
- [x] Экспортировать API bundled schema registry.
- [x] Доказать portable build без EDT.

## Задача 5: Перестроить документацию и GitHub backlog

- [x] Описать новую архитектуру.
- [x] Закрыть старые symptom-oriented issues/project.
- [x] Создать новый project и layer-oriented issues.

## Задача 6: Проверить и опубликовать

- [x] Запустить тесты `ibcmd-schema` и `ibcmd-xml`.
- [x] Проверить отсутствие абсолютных EDT-путей в corpus.
- [x] Закоммитить и отправить ветку.
