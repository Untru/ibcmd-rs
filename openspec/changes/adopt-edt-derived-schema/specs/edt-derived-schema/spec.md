# EDT-derived schema

## ADDED Requirements

### Requirement: Автономный corpus моделей

Система MUST включать версионированный индекс EDT-derived model types, importers
и exporters и MUST загружать его без установленной EDT.

#### Scenario: Portable build

- **WHEN** выполняется default build на машине без EDT
- **THEN** model inventory и writer rules доступны из committed corpus
- **AND** Java, OSGi и native EDT libraries не запускаются

### Requirement: Проверяемое происхождение правил

Каждое writer rule MUST содержать исходный класс, feature, поведение и
provenance. Непроверенная гипотеза MUST NOT считаться production rule.

#### Scenario: Загрузка bundled rules

- **WHEN** registry загружает встроенный writer corpus
- **THEN** все идентификаторы уникальны
- **AND** каждое правило содержит EDT release и evidence kind

### Requirement: Разделение decoder и serializer

Физический decoder MUST NOT определять порядок XML, а XML serializer MUST NOT
интерпретировать числовые raw slots.

#### Scenario: Новое правило XML

- **WHEN** добавляется правило порядка, default или version gate
- **THEN** оно размещается в schema/writer corpus
- **AND** может быть проверено независимо от MSSQL
