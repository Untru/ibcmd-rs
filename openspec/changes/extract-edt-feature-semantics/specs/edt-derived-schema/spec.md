## ADDED Requirements

### Requirement: Feature semantics are evidence-aware

Система SHALL хранить model semantics и XML behaviour с явным статусом
доказательности и SHALL запрещать использование неполного `verified`-правила.

#### Scenario: XML behaviour has not been inspected

- **WHEN** Xcore подтверждает model type и cardinality, но writer ещё не исследован
- **THEN** model evidence имеет статус `verified`
- **AND** XML behaviour имеет статус `pending`

### Requirement: Unsupported Xcore syntax fails closed

Research importer SHALL отклонять неизвестные classifier, feature qualifiers и
multiplicity вместо создания предположительного model rule.

#### Scenario: Operation body contains feature-like Java syntax

- **WHEN** Xcore operation содержит generics, assignments и вложенные блоки
- **THEN** importer полностью пропускает operation body
- **AND** ни одна строка тела не попадает в список model features

### Requirement: Research import is deterministic and non-distributive

Research importer SHALL создавать одинаковый очищенный corpus для одинакового
EDT release и SHALL NOT копировать Xcore, JAR, class bytes или абсолютные пути.

#### Scenario: Import is repeated

- **WHEN** importer дважды запускается для одного inventory и scope
- **THEN** SHA-256 обоих JSON совпадает
- **AND** runtime build не требует EDT
