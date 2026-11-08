## ADDED Requirements

### Requirement: Current XML 2.21 empty pictures follow the measured native writer

The XML 2.21 codec SHALL convert an unreferenced empty stored Picture into
the empty readable Picture measured on native 8.5.1.1529, with the generated
namespace prefix numbered by value depth. Its inverse SHALL recover the
empty stored primitive without synthesizing Ref or LoadTransparent children.

#### Scenario: Empty and named pictures share a document

- **GIVEN** a document containing measured empty and named picture primitives
- **WHEN** it is exported as XML 2.21 and compiled back
- **THEN** the empty values match current native spelling and the original stored values are recovered
- **THEN** named-picture representation and context requirements remain unchanged

#### Scenario: An unsupported picture shape is encountered

- **GIVEN** an empty picture with an unknown attribute, malformed reference or neighboring inline XML
- **WHEN** the codec attempts to convert it
- **THEN** conversion fails before returning a rewritten document

### Requirement: Legacy evidence remains distinct from current native equality

The fix SHALL preserve the pinned 1150 oracle and SHALL NOT count its classic
empty spelling as byte-equal to the current 1529 readable spelling. Default
verified import SHALL retain its atomic refusal when the source cannot be
reproduced exactly.

#### Scenario: A current native source is imported

- **GIVEN** the measured Russian-script 1529 native source containing empty pictures
- **WHEN** it is imported with default verification and applied
- **THEN** native and candidate exports match all source files byte-for-byte

#### Scenario: A legacy empty spelling cannot be reproduced

- **GIVEN** a current source whose readable empty values alone are replaced by classic values
- **WHEN** default verified import targets profile 1529
- **THEN** import refuses and the database bytes remain unchanged
- **THEN** the legacy difference remains an explicit gap rather than a successful equality claim
