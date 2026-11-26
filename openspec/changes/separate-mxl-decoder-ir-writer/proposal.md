# Предложение: отделить MXL decoder от canonical IR и XML writer

## Зачем

`mssql_dump::moxel` одновременно распознаёт двоичный контейнер MOXCEL,
интерпретирует палитры и source-format references, а затем выбирает индексы
для XML. Это делает расхождение неразличимым: нельзя стабильно сказать,
потерял ли decoder данные или writer неверно их проецировал.

## Что меняется

- Вводится ограниченный typed hand-off: decoded spreadsheet IR вместе с
  provenance палитры и явным identity/non-one-based отображением форматов.
- XML projection получает готовый write plan и не читает raw MOXCEL slots и не
  вычисляет palette/`formatIndex` заново.
- Добавляются стабильные typed diagnostics с владельцем `decoder` или `writer`.
- Первый перенос ограничен MXL template source assets; DCS, forms и metadata
  не меняются.

## Не входит в изменение

- Новые QName, порядок XML-элементов и правила default values.
- Поддержка неизвестных вариантов MOXCEL и корректировка полного parity.
- Перевод MXL на EDT-derived schema rules без отдельного evidence corpus.
