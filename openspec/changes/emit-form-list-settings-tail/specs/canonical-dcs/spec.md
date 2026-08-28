## ADDED Requirements

### Requirement: Verified Form ListSettings tail is schema-driven

The Form `ListSettings` writer SHALL obtain the namespace, final child order,
default omission rules, writer operations, delegation rule, and null omission
rule from the bounded committed EDT writer-evidence schema.

#### Scenario: Both non-default tail values are present

- **WHEN** `itemsViewMode` and `itemsUserSettingID` have non-default values
- **THEN** XML emits them in that exact order
- **AND** text values are XML-escaped

#### Scenario: Tail values are absent or default

- **WHEN** view mode is absent or `QuickAccess`, and user-setting ID is absent
  or empty
- **THEN** the tail emitter emits no child XML

#### Scenario: Model and writer defaults disagree

- **WHEN** the writer constant is not exactly `QUICK_ACCESS` or the verified
  Xcore lexical default is not exactly `QuickAccess`
- **THEN** policy construction fails closed
- **AND** no naming convention is used to convert between them

#### Scenario: Tail input is not legal XML

- **WHEN** a tail value contains an XML 1.0 forbidden character
- **THEN** emission fails before returning bytes

#### Scenario: Caller prefix is invalid or reserved

- **WHEN** the caller prefix is not a bounded NCName or is `xml` or `xmlns`
  case-insensitively
- **THEN** emission fails before returning bytes

### Requirement: Tail delegation does not authorize full DCS emission

The tail emitter SHALL produce child XML only. The Form adapter SHALL retain
ownership of the wrapper, prefix, indentation, and all complex sections, while
the general DCS serializer remains blocked by the four unverified wrapper,
type, and opaque-placement facts.

#### Scenario: Form contains complex ListSettings sections

- **WHEN** filter, order, or conditional appearance precedes typed tail values
- **THEN** their existing bytes remain unchanged
- **AND** only the two final manual branches are replaced by delegation

#### Scenario: Full DCS preflight is requested

- **WHEN** a caller requests full standalone or Form settings serialization
- **THEN** preflight remains fail-closed
- **AND** its diagnostic reports only the wrapper fact for that envelope, the
  common type fact, and opaque placement only when opaque input is present
- **AND** the aggregate evidence matrix retains all four missing facts
