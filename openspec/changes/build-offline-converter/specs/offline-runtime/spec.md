# Offline Runtime

## ADDED Requirements

### Requirement: Product operations are platform-independent

The default and release builds MUST complete supported XML, CF and storage conversions without locating, loading or executing components of 1C:Enterprise or 1C:EDT.

#### Scenario: Convert with an empty executable search path

- **GIVEN** a supported fixture and a process environment without `ibcmd`, `1cv8`, Designer, EDT or Java
- **WHEN** the user runs a supported inspect, repack, export or convert operation
- **THEN** the operation completes using only bundled Rust code and profile data

#### Scenario: Unsupported operation is requested

- **GIVEN** an artifact containing a feature without a native codec
- **WHEN** an encode operation is requested
- **THEN** the tool fails before writing output and reports the missing capability

### Requirement: Platform-oracle code is not part of the product path

Any research-only oracle integration MUST be compile-time isolated from default/release binaries and MUST NOT be required by standard CI.

#### Scenario: Inspect release dependencies

- **WHEN** a release artifact is built
- **THEN** it contains no platform executable, EDT JAR, JNI/OSGi bridge or code path that probes an installed 1C distribution

