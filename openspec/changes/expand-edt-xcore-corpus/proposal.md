# Предложение: расширить Xcore corpus на все EDT model bundles

## Зачем

Первый vertical slice покрывает только `model/Form.xcore`. Для полного coverage
map и последующих writers нужны все декларативные model packages EDT, при этом
неизвестные конструкции нельзя интерпретировать предположительно.

## Что меняется

- Importer поддерживает подтверждённую грамматику всех Xcore resources release.
- Для неподдержанных конструкций создаётся детерминированный machine-readable
  reject report.
- В репозиторий включается очищенный all-model semantics corpus.
- Bundled API и tests проверяют totals, representative packages и детерминизм.

## Результат

Каждый model feature EDT 2025.2.3 либо присутствует в corpus, либо явно указан в
reject report с причиной; молчаливых пропусков нет.
