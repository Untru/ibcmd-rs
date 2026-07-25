# Предложение: автоматизировать governance EDT-derived corpus

## Зачем

Архитектурные правила clean-room и provenance сейчас описаны документацией, но
не являются обязательным CI gate. Ошибка может незаметно добавить binary,
absolute path или production rule без evidence.

## Что меняется

- Добавляется автономный validator committed corpus.
- CI запрещает proprietary binaries, source payloads, абсолютные пути и
  `verified` facts без provenance.
- Документируется evidence policy и процедура обновления.

## Результат

Нарушение границы EDT-derived данных блокирует pull request до merge.
