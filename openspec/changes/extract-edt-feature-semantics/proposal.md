# Предложение: извлечь семантику features из EDT Xcore

## Зачем

Текущий автономный corpus фиксирует идентификаторы EPackage, classifiers,
features и operations, но не содержит типов, cardinality, default values и
статуса сведений об XML-представлении. Без этого XML writers продолжают
восстанавливать часть схемы локальными условиями.

EDT 2025.2.3 model bundles содержат декларативные `*.xcore`. Они позволяют
извлекать структуру EMF-моделей напрямую, без угадывания `PackageImpl` bytecode.

## Что меняется

- Добавляется версионированный feature-semantics corpus.
- Research importer читает Xcore только из локальной EDT-лаборатории.
- Runtime использует исключительно committed JSON.
- Каждое значение получает происхождение и состояние `verified` или `pending`.
- Неизвестные QName/order/version/delegate сохраняются как `pending`, а не
  превращаются в production-правило.

## Не входит в первый инкремент

- Копирование Xcore, JAR или class-файлов EDT в репозиторий.
- Автоматическое объявление XML QName по имени model feature.
- Перевод всех XML writers на новый corpus.

## Результат

Для первого представительного набора моделей форм создаётся воспроизводимый
вертикальный срез: Xcore → очищенный JSON → строгая Rust-валидация → bundled API.
Следующие задачи расширяют покрытие и дополняют XML behaviour из readers/writers.
