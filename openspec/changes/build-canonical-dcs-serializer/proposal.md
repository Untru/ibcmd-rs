# Предложение: каноническая DCS-модель и serializer

## Зачем

DCS schema/settings/template и Form ListSettings сейчас проходят через разные
ручные XML-преобразования. Это дублирует QName/order/type решения и создаёт
несогласованные исправления.

## Что меняется

- Добавляется единая каноническая DCS-модель.
- Schema-derived rules владеют QName, TypeId, collection order и делегированием.
- Неизвестные расширения сохраняются opaque-lossless с placement/provenance.
- Form и standalone DCS используют один serializer.

## Результат

Одинаковая DCS-семантика даёт одинаковый XML независимо от физического
источника и не редактируется строковыми нормализаторами.
