# Предложение: построить coverage map EDT → ibcmd-core

## Зачем

Полный model corpus сам по себе не показывает, какие features представлены
канонически, сохраняются opaque или теряются. Без машинной карты backlog снова
будет строиться по отдельным XML-файлам.

## Что меняется

- Добавляется versioned coverage corpus для каждого EDT model feature.
- Статусы: `typed`, `opaque-lossless`, `unsupported`, `platform-only`.
- Строгий gate запрещает feature без явного mapping.
- Агрегация показывает покрытие metadata/forms/DCS/MXL.

## Результат

Любой отсутствующий функционал выражается конкретным schema key и mapping
status, а не именем прикладного объекта или XML-файла.
