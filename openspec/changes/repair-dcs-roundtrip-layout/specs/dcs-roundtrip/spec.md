## ADDED Requirements

### Requirement: DCS round-trip verification preserves lexical values while accepting structural indentation

The DCS schema round-trip gate SHALL accept documents differing only in literal XML whitespace between children of element-only containers. It SHALL additionally accept removal, addition or changed binding of a prefixed namespace declaration only when a conservative document-wide proof establishes that the prefix is unused. It SHALL preserve default namespaces, all used or uncertain prefixed namespace declarations, the exact remaining element markup and all other lexical content, honor inherited `xml:space`, reject invalid XML and leave compiler input unchanged.

#### Scenario: Supported compact or alternate-line-ending layout

- **GIVEN** a supported DCS schema whose type descriptions or selection items use compact layout, LF or CR indentation
- **WHEN** compilation exports the same values with canonical indentation
- **THEN** round-trip verification SHALL accept the schema while preserving query text, bindings and typed values

#### Scenario: Changed scalar or non-structural content

- **GIVEN** a compiled schema and a source with changed scalar text, leaf whitespace, query line endings, character references, mixed text, comments or CDATA
- **WHEN** round-trip verification compares them
- **THEN** it SHALL reject the difference before configuration rows are published

#### Scenario: Preserved whitespace or invalid XML

- **GIVEN** source whitespace under an inherited `xml:space="preserve"` declaration, or malformed XML
- **WHEN** comparison would otherwise discard indentation
- **THEN** preserved whitespace SHALL remain exact and invalid XML SHALL be rejected

#### Scenario: Proven unused prefixed declarations

- **GIVEN** a supported schema whose writer replaces unused prefixed namespace declarations
- **WHEN** neither element/attribute names nor conservatively decoded content anywhere in either document can use those prefixes
- **THEN** the DCS gate SHALL accept only those declaration differences and structural indentation
- **AND** both existing general XML comparison APIs SHALL continue to compare namespace declarations exactly

#### Scenario: Namespace usage or uncertain proof

- **GIVEN** a default namespace, a prefix used in a name or encoded QName value, or uncertain opaque content
- **WHEN** the corresponding declaration differs
- **THEN** comparison SHALL retain the declaration and reject the difference
- **AND** QName text split across character references, CDATA, comments or child elements SHALL not establish non-usage
