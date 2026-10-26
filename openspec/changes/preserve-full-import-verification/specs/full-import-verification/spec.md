## ADDED Requirements

### Requirement: Final XML comparison preserves semantic changes

The import guard SHALL compare equivalent empty-leaf spelling, admitted unique metadata Type member sets and uniquely named Rights objects without hiding missing values, changed types, qualifiers, rights, conditions or attributes. XML references SHALL contribute decoded content; unknown references SHALL fail parsing. Foreign or ambiguous XML SHALL retain strict comparison.

#### Scenario: Equivalent serialization

- **GIVEN** equal supported XML differing only by paired/self-closing empty leaves, admitted Type member order or Rights object order
- **WHEN** final import verification compares the documents
- **THEN** it SHALL accept the equivalent serialization

#### Scenario: Changed semantics or ambiguous namespaces

- **GIVEN** a changed type, qualifier, encoded RLS condition, revoked right, missing empty leaf, duplicate member, or foreign/rebound namespace
- **WHEN** the documents are compared
- **THEN** the semantic change SHALL remain visible and foreign or ambiguous structures SHALL NOT gain unordered comparison

#### Scenario: Native Rights root type

- **GIVEN** supported Rights XML whose root type attribute resolves to XML Schema instance type Rights
- **WHEN** only uniquely named objects are reordered
- **THEN** the guard SHALL admit that root attribute and retain every namespace declaration, type value, right, restriction and other indexed coordinate in comparison

#### Scenario: Ambiguous Rights root coordinates

- **GIVEN** Rights root attributes that collide in the diagnostic index through namespace aliases, ordinary attributes or namespace declarations
- **WHEN** comparison reaches byte identity, hash identity, object rekeying or generic leaf fallback
- **THEN** it SHALL refuse the ambiguity rather than collapse distinct coordinates, while retaining conservative fallback for unrelated unknown RLS structures

#### Scenario: DCS prefix declarations

- **GIVEN** two valid DataCompositionSchema documents in the schema namespace
- **WHEN** final verification compares them
- **THEN** it SHALL use the existing conservative DCS unused-prefix proof and SHALL retain every used or uncertain namespace and changed leaf

### Requirement: Evidenced codecs retain source values

The typed codecs SHALL preserve the measured MediumGreen color, search-addition width, CatalogRef.Parent and scoped UseAlways path, and nondefault Document tabular-section fill values. Other families and unknown coordinates SHALL retain existing behavior or refuse input.

#### Scenario: Supported nondefault values

- **GIVEN** supported input containing one of the measured form coordinates or an explicit nil or nondefault Document tabular-section fill pair
- **WHEN** the input is compiled and exported
- **THEN** the value SHALL remain represented and SHALL NOT be silently replaced by an existing target value or default

#### Scenario: Unsupported property

- **GIVEN** an unknown native profile or unsupported source property
- **WHEN** compilation or final verification encounters it
- **THEN** the operation SHALL refuse before publication rather than invent a slot or omit the difference

### Requirement: External TableDataType edits preserve the verified base

The import path SHALL admit an existing external-source TableDataType edit only for a validated native profile whose complete candidate XML matches the source. The patch SHALL change only the schema-owned data-type coordinate.

#### Scenario: Bounded semantic edit

- **GIVEN** a supported existing Table with a valid generated owner/name and source differing only in TableDataType
- **WHEN** the row is prepared
- **THEN** the candidate SHALL retain all other native coordinates and its complete XML SHALL match the source

#### Scenario: Additional or unsupported edit

- **GIVEN** a changed unsupported property, field, identifier or unknown native profile
- **WHEN** the Table patch is attempted
- **THEN** it SHALL refuse before returning a candidate for publication

### Requirement: Managed Form comparison uses bounded native defaults

The final guard SHALL use an authoritative qualified-tree comparison for managed Form XML 2.20. It SHALL accept only the witnessed defaults at typed owner paths and SHALL retain namespaces, unknown coordinates, meaningful content and full order of semantic collections. Uncertain namespace proofs SHALL preserve declarations, and invalid original namespace bindings SHALL refuse.

#### Scenario: Witnessed form defaults

- **GIVEN** a supported checkbox Auto, usual-group ShowTitle true, nil RowFilter directly bound to a singleton ValueTable attribute, root Document defaults proved by a unique main DocumentObject attribute, or ordinary root Button UsualButton
- **WHEN** one source document omits only that witnessed default
- **THEN** the Form comparison SHALL accept equivalent values without extending the allowance to other owners, types or values

#### Scenario: Strong comparison refuses

- **GIVEN** changed heterogeneous control order, unknown values or attributes, unproven binding, foreign namespace, invalid namespace declaration, or changed meaningful text
- **WHEN** the Form comparator refuses equivalence
- **THEN** final verification SHALL retain that refusal even if its diagnostic local-name index happens to match

#### Scenario: Independent native properties

- **GIVEN** a supported modern UsualGroup with source ShowTitle and EnableContentChange values
- **WHEN** its existing row is patched
- **THEN** the values SHALL occupy their independent typed coordinates and unadmitted modern layouts SHALL refuse requested ShowTitle edits

#### Scenario: Existing Document fill edits

- **GIVEN** a supported existing Document with a UUID-matched tabular-section attribute and supported explicit fill pair
- **WHEN** staging patches the descriptor
- **THEN** only the two fill coordinates SHALL change and unsupported values or ambiguous/native-profile matches SHALL refuse

#### Scenario: Modern usual-group arrangement

- **GIVEN** the evidenced modern UsualGroup layout and a supported explicit Group value
- **WHEN** its existing row is patched
- **THEN** only the three schema-owned arrangement coordinates SHALL change, the tooltip header SHALL remain unchanged, and unsupported or ambiguous profiles SHALL refuse

#### Scenario: Unsupported arrangement cannot verify itself

- **GIVEN** a managed Form XML 2.20 containing UsualGroup Group HorizontalIfPossible, which the native platform does not retain
- **WHEN** final comparison sees even identical source and candidate XML
- **THEN** it SHALL refuse that unsupported property without extending the refusal to root or Page grouping

#### Scenario: Literal indentation of an empty root UI command bar

- **GIVEN** a Form XML 2.20 in the managed-form namespace with a direct root `ChildItems/CommandBar` whose only attributes are ordinary `name` and `id`
- **WHEN** its body is empty or contains only literal XML indentation, including LF or CRLF spelling
- **THEN** final comparison SHALL treat that container indentation and empty/self-closing spelling as equivalent while retaining the command bar identity
- **AND** it SHALL preserve differences in scalar text, character references, CDATA, comments, processing instructions, extra attributes, foreign or nested owners, and inherited `xml:space="preserve"`

The bounded rule is evidenced by an offline comparison of actual staged XML: replacing only CRLF with LF inside one empty root command bar changes the final comparison from different to equal. It does not add or remove UI children, reorder events or normalize tooltip defaults.
