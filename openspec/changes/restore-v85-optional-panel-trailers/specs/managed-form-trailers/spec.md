## ADDED Requirements

### Requirement: Generated optional panels do not change logical trailer positions

At a source-publication boundary, a declared revision-59 managed form SHALL
be read through its schema-owned
trailer profile. The profile SHALL permit twelve logical appended members
and at most one validated generated empty panel after each of the three
known presence flags. Physical payload records SHALL NOT shift another
logical property or prevent reading a table's HierarchyPanelLocation.

#### Scenario: Optional generated panels are present

- **GIVEN** a public native-proven revision-59 form with known generated root panels and HierarchyPanelLocation=None
- **WHEN** the form is exported to XML 2.21
- **THEN** the hierarchy property matches native and every retained logical trailer member keeps its original position

#### Scenario: No panel is present

- **GIVEN** a known revision-59 form with all three presence flags clear
- **WHEN** the form is exported
- **THEN** its existing output remains unchanged

### Requirement: Unknown trailer layouts refuse before publication

The reader SHALL require exactly one complete trailer boundary. A declared
revision-59 form with an invalid flag, a missing or unsupported panel payload,
an unexplained extra member or an ambiguous boundary SHALL refuse before
publishing its source file. It SHALL NOT fall through to the generic legacy
reader or synthesize a property value.

#### Scenario: A panel flag names an unsupported record

- **GIVEN** a declared revision-59 form whose set presence flag is followed by an unknown record
- **WHEN** source export is attempted
- **THEN** export reports the unsupported trailer and does not publish that form file

#### Scenario: Trailer boundaries are ambiguous

- **GIVEN** a declared revision-59 form with more than one candidate that otherwise passes the bounded grammar
- **WHEN** source export is attempted
- **THEN** export refuses the ambiguity before publishing the form file

### Requirement: Compatibility fragments do not admit physical profiles

The low-level compatibility reader MAY retain the pre-existing legacy
root-59 fragment semantics. It SHALL use modern conversion only for a fully
validated bounded trailer. Source publishers and physical-stage matching
SHALL use the strict declared-profile reader instead. Compatibility success
SHALL NOT establish native physical-profile admission.

#### Scenario: A short legacy fragment uses root number 59

- **GIVEN** a legacy fragment with no modern trailer
- **WHEN** the compatibility helper reads it
- **THEN** its existing semantics remain available and the strict source helper refuses the same fragment
