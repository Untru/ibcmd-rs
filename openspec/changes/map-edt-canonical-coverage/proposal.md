# Предложение: построить coverage map EDT → ibcmd-core

## Зачем

Полный model corpus сам по себе не показывает, какие features представлены
канонически, сохраняются opaque или теряются. Без машинной карты backlog снова
будет строиться по отдельным XML-файлам.

## Что меняется

- Добавляется versioned coverage corpus для каждого EDT model feature.
- Статусы: `typed`, `opaque-lossless`, `unsupported`, `platform-only`.
- Строгий gate запрещает feature без явного mapping.
- Машинные агрегаты показывают покрытие
  metadata/forms/DCS/MXL/common/other, включая честные нулевые семейства.
- Упорядоченный migration backlog группирует `unsupported/schema.unmapped`
  только по переиспользуемой семантике rule/package/classifier/feature kind,
  без имён прикладных объектов, файлов и UUID.

## Результат

Любой отсутствующий функционал выражается конкретным schema key и mapping
status, а не именем прикладного объекта или XML-файла.
