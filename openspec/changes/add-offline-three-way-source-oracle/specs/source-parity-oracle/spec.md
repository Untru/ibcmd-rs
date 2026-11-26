## ADDED Requirements

### Requirement: Offline three-way source evidence remains bounded and reproducible

The system SHALL compare three explicitly supplied, pre-existing source trees
without launching EDT, Java/JVM, `ibcmd`, or database tooling. It SHALL require
explicit source and tool-version strings, preserve per-path SHA-256 values and
sizes, and produce deterministic tree hashes and JSON/Markdown artifacts under
explicit resource limits.

#### Scenario: Valid pre-existing trees are supplied

- **WHEN** native, EDT import/export, and ibcmd-rs trees plus exact provenance
  strings are supplied within configured limits
- **THEN** the system emits new stable JSON and Markdown evidence outside the
  input trees
- **AND** it does not start EDT/JVM or any export tool

#### Scenario: An input exceeds a resource bound

- **WHEN** a tree exceeds the configured file-count, total-byte, or per-file
  byte limit
- **THEN** the system fails before producing a report
- **AND** it does not modify any input tree

#### Scenario: An input changes while it is hashed

- **WHEN** an opened input grows, is replaced, changes metadata identity, or
  produces different hashes across bounded passes
- **THEN** the system fails without publishing evidence
- **AND** aggregate accounting uses the size obtained from the opened handle

#### Scenario: A Windows input is replaced with matching superficial metadata

- **WHEN** a same-size replacement copies timestamps and attributes from the
  originally accounted Windows file
- **THEN** the open-handle volume serial and file index comparison detects the
  different file identity
- **AND** the system fails without publishing evidence

#### Scenario: Unix source names are not valid UTF-8

- **WHEN** distinct Unix path components contain invalid bytes such as `0xFE`
  and `0xFF`
- **THEN** the system rejects them with a clear non-UTF-8 path error
- **AND** it does not lossy-normalize them into the same report key

#### Scenario: Paired artifact publication fails

- **WHEN** both synced temporary artifacts are complete but either no-overwrite
  final publication fails
- **THEN** every final published by that attempt is rolled back
- **AND** no temporary artifact from the attempt remains

#### Scenario: The output parent changes identity

- **WHEN** the canonical same-parent destination becomes an alias, symlink,
  reparse point, or different directory before publication
- **THEN** the system fails before publishing either final artifact

#### Scenario: Provenance or paths contain Markdown syntax

- **WHEN** a caller-supplied value contains pipes, backslashes, backticks,
  markup characters, CR, or LF
- **THEN** Markdown renders it as deterministic escaped plain text
- **AND** it cannot add a row, cell, or code span

### Requirement: Three-way agreement diagnoses are candidates, not facts

The system SHALL classify every path as exactly one of all equal,
native=EDT!=ours, native=ours!=EDT, EDT=ours!=native, or all different. For
non-equal states it SHALL describe a candidate investigation direction without
claiming causal layer diagnosis from hashes alone.

#### Scenario: Native and EDT agree while ibcmd-rs differs

- **WHEN** native and EDT hashes match at a path and ibcmd-rs differs
- **THEN** the row is `native_edt_not_ours`
- **AND** its interpretation names decoder/model/schema/writer only as a
  candidate rather than a proven fact

#### Scenario: All three hashes differ

- **WHEN** native, EDT, and ibcmd-rs hashes are pairwise unequal at a path
- **THEN** the row is `all_different`
- **AND** the interpretation is explicitly unclassified
