## ADDED Requirements

### Requirement: Client application panels preserve the evidenced height coordinate

The client interface codec SHALL retain an absent panel height as native tail `2,0` and explicit XML height `1` as native tail `2,1`. The exporter SHALL emit `height` after `uuid` only for the witnessed non-default coordinate. Other interface properties and native default fixtures SHALL remain unchanged.

#### Scenario: Witnessed non-default height

- **GIVEN** a supported client application interface panel with `height` equal to `1`
- **WHEN** the interface is compiled and exported
- **THEN** its native record SHALL retain terminal coordinate `2,1` and its XML SHALL retain the height and panel identity

#### Scenario: Default height remains absent

- **GIVEN** a supported panel without XML height and a native record ending in `2,0`
- **WHEN** the interface is compiled or exported
- **THEN** its existing native spelling SHALL remain unchanged and the XML SHALL omit height

#### Scenario: Unmeasured or malformed input

- **GIVEN** an explicit XML height other than `1`, duplicate/extra panel properties, or an unknown/incomplete/extra native tail
- **WHEN** the codec reads the input
- **THEN** it SHALL refuse the input before configuration publication rather than discard or invent a height
