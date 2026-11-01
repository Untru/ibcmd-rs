## ADDED Requirements

### Requirement: Coverage sets are generated deterministically

The coverage generator SHALL build every variant of a set from its committed
inputs alone, and the same inputs SHALL produce a byte-identical XML tree with
the same object and generated-type identifiers.

#### Scenario: A set is regenerated

- **GIVEN** the committed `input/` of a coverage set
- **WHEN** `tools/coverage/generate.py` runs twice
- **THEN** both runs write byte-identical trees

#### Scenario: A variant changes one property

- **GIVEN** a template object and a value of one of its properties
- **WHEN** the variant for that value is generated
- **THEN** it differs from the template only in that property, its name and its identifiers

### Requirement: The native platform is the oracle of every variant

A variant SHALL count as covered only after the pinned native platform
imported it, exported it twice with identical trees, and the manifest recorded
the platform build, the `ibcmd` digest and the digest of every native file.

#### Scenario: The platform refuses a variant

- **WHEN** native import rejects a variant
- **THEN** the manifest records it as `platform_refused` with the platform's message, and the rest of the set is imported without it

#### Scenario: Native rounds differ

- **WHEN** the two native exports of a set differ outside `ConfigDumpInfo.xml`
- **THEN** the set is not committed

### Requirement: Coverage sets carry no application data

A coverage set SHALL contain only objects the generator created. A value space
MAY name the corpus a value was observed in, but SHALL NOT carry objects,
modules or other content of that corpus.

#### Scenario: A value comes from a typical configuration

- **GIVEN** a property value observed in a native export of a typical configuration
- **WHEN** it enters `values.json`
- **THEN** only the value and the corpus name are committed

### Requirement: The offline check holds every covered variant

The offline coverage test SHALL export every committed set without the 1C
platform, compare each variant's files with its native files, and fail when a
variant listed in `baseline.json` is no longer equal or when a committed digest
does not match.

#### Scenario: A decoder change breaks a covered value

- **GIVEN** a variant listed in `baseline.json`
- **WHEN** ibcmd-rs writes a file of that variant that differs from the native one
- **THEN** the test fails and names the variant, its property and value, and the first differing file

#### Scenario: A change fixes a variant

- **WHEN** a variant outside `baseline.json` becomes equal
- **THEN** the test passes and reports the variant so that it can be added to the baseline

### Requirement: Coverage is reported per set

The offline coverage test SHALL report, per set, the number of variants
generated, proven by the platform, refused by the platform, equal, different
and refused by ibcmd-rs.

#### Scenario: The report is read in CI

- **WHEN** the coverage test runs
- **THEN** its output carries one count line per set

### Requirement: Coverage spans configurations, extensions and external files

Coverage sets SHALL exist for each source kind ibcmd-rs exports: the
configuration, the configuration extension, the external data processor and
the external report. Each kind SHALL be proven by the native route that
produces it, and checked offline through the `cf export` input of the same kind.

#### Scenario: An extension variant is proven

- **GIVEN** a base configuration and a generated extension variant
- **WHEN** the platform creates the extension, imports it, saves it as `.cfe` and exports it twice with identical trees
- **THEN** the `.cfe` and the native tree are committed, and the offline check exports the `.cfe` and compares it with that tree

#### Scenario: An external data processor variant is proven

- **GIVEN** a generated external data processor variant
- **WHEN** the platform builds it into an `.epf` from its root XML and exports it twice with identical trees
- **THEN** the `.epf` and the native tree are committed, and the offline check exports the `.epf` and compares it with that tree

### Requirement: A combined configuration holds every proven variant of a platform

For each platform with configuration sets, a combined configuration SHALL
contain every platform-proven variant of those sets, be proven by the same
native route, and be checked offline like a set. The offline check SHALL fail
when a source set's manifest changed after the combined configuration was
built.

#### Scenario: A source set changes

- **GIVEN** a combined configuration built from the class sets of a platform
- **WHEN** one of those sets is proven again with new variants
- **THEN** the offline check fails and names `tools/coverage/combine.py` until the combined configuration is rebuilt

### Requirement: The writers' round trip holds the combined sets

An offline test SHALL write the native XML of every all-in-one set (each
platform's combined configuration, the combined extension and the combined
external files) with ibcmd-rs through `cf bootstrap` and, for configurations,
through an import into an empty platform-made file infobase, export it back
and compare each variant's files with the native ones. When `cf bootstrap`
refuses a combined configuration as a whole, the test SHALL run `cf bootstrap`
on the class sets the configuration was built from, so that output is
compared wherever the writers can write it. A refused set SHALL be held to
the exact files it refuses and the writer's reason for each, and every
variant that does not come back SHALL be listed, with how it fails and its
issue, in
`tests/fixtures/coverage/round-trip.json`.

#### Scenario: A writer change breaks a variant that round-trips

- **GIVEN** a variant of a combined set that is not listed as a gap
- **WHEN** ibcmd-rs writes it so that its export differs from the native file
- **THEN** the test fails and names the route, the set, the variant and the first differing file

#### Scenario: A writer refuses one more file

- **GIVEN** a combined set the route refuses with a listed set of files
- **WHEN** the writer refuses a file outside that list, or a listed file for another reason
- **THEN** the test fails and names the file and the writer's reason

#### Scenario: A gap is fixed

- **WHEN** a listed gap or refused file comes back equal
- **THEN** the test passes and reports the entry so that it can leave the list

### Requirement: A mismatch on a typical configuration becomes a coverage variant first

A difference between native and ibcmd-rs output found on a typical
configuration SHALL be reproduced as a platform-proven coverage variant that
the offline check reports as different or refused, before the decoder or
writer is changed for it.

#### Scenario: A property value differs on a typical configuration

- **GIVEN** a file of a typical configuration that differs from the native one in one property value
- **WHEN** an agent fixes it
- **THEN** the PR contains a coverage variant with that value which was red before the fix and is in the baseline after it
