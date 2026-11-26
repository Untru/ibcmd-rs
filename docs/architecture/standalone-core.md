# Standalone conversion contract

This document defines the architectural contract for the standalone XML/CF
conversion path. It is normative for new core, XML, container, CF, profile, and
migration code. Existing MSSQL research and staging commands remain a legacy
integration layer while they are migrated behind these boundaries.

## Standalone runtime boundary

Supported production conversion paths MUST be implemented from project code,
profiles, and input artifacts alone. Released binaries and libraries MUST NOT
have a production or runtime dependency on, link to, discover, invoke, or
delegate conversion to any of the following:

- `ibcmd`;
- `1cv8` or Designer;
- EDT;
- a JVM, Java/OSGi sidecar, or EDT-derived runtime component.

Those tools may be used outside the product runtime to create legally usable
research evidence or fixtures. They are not a fallback backend, an optional
provider, or a condition for a supported capability. Standard CI MUST exercise
the standalone path without them available.

## Independent version axes

A conversion profile is a coordinated tuple of independent axes. None of these
values is an alias for another, and changing one MUST NOT implicitly change or
infer the others:

- `PlatformBuild` identifies a concrete platform build or supported build
  range whose behavior supplied compatibility evidence.
- `XmlDialect` identifies the XML/XCF schema and serialization dialect, such as
  2.17, 2.20, or 2.21.
- `CompatibilityMode` is an open value describing configuration behavior; it
  is not the platform build or XML dialect.
- `StorageProfile` identifies the logical native-storage layout, including the
  relevant `Config`/`ConfigSave` and DBMS representation.
- `ContainerRevision` identifies the physical CF/container format and its
  parameters, including Format15 and Format16 families.

Profiles MUST declare their supported axes and evidence explicitly. Detection
MUST report `exact`, `ambiguous`, or `unknown` together with its evidence. A
reader may inspect an ambiguous or unknown artifact, but every write requires
one unambiguous target profile. Rewriting only an XML root `version` is never a
migration.

## Lossless and opaque data

The physical/logical `StorageImage` preserves ordered entries, multipart
identity, raw headers and payloads, compression information, storage
attributes, source profile, and provenance. The semantic
`CanonicalConfiguration` holds typed data plus opaque facets attached to stable
anchors.

For a same-profile no-op or repack, unknown data MUST use opaque passthrough.
An unchanged opaque payload, its identity, ordering constraints, and required
storage metadata MUST be retained; the payload bytes MUST remain identical.
Known entries may be encoded into deterministic project-owned output, so this
contract does not require byte-for-byte equality with a vendor compressor.
Same-profile round trips MUST nevertheless preserve the semantic digest.

Opaque data MUST NOT be copied automatically across profiles. A cross-profile
operation without a typed codec and an explicit migration rule for every
affected opaque facet MUST fail closed before output is written. Unknown
container layouts, unresolved profile detection, and missing migration edges
follow the same rule.

## Capability levels

Capabilities are declared and verified independently. A lower-level reader or
writer does not imply a higher-level capability.

- `Inspect` structurally reads, identifies, and validates an artifact. It makes
  no promise that semantic export or writing is available.
- `Repack` reads a CF into a `StorageImage` and writes it back for the same
  profile. It preserves opaque entries and is not a semantic migration.
- `Export` decodes a supported CF/storage artifact through canonical IR and
  emits XML for its declared profile and dialect. It does not promise a reverse
  writer.
- `Overlay` applies supported XML changes to a compatible base CF and emits a
  CF. Any family marked `NeedsBase` remains base-dependent; overlay is not
  bootstrap.
- `Bootstrap` builds a complete `StorageImage` and new CF from XML without a
  base artifact. It is available only when the readiness manifest has no
  `NeedsBase` or `Unsupported` blocker, including special entries, generated
  types, references, metadata bodies, and assets.
- `Convert` decodes to canonical IR, follows an explicit migration graph, and
  encodes a different target profile as CF or XML. It is the only capability
  that promises cross-profile transformation.

Capability claims MUST identify artifact kind, source and target profiles,
family coverage, and reproducible evidence. In particular, availability of
`Inspect`, `Repack`, `Export`, or `Overlay` MUST NOT be presented as evidence
that `Bootstrap` or `Convert` is supported.

## Loss policy

Every analysis, migration, and write path MUST inventory potential losses
during preflight and produce a machine-readable loss report. Silent dropping
or coercion is forbidden.

The default loss policy is `error`: any reported loss aborts the operation
before creating or replacing its output. Alternative policies require an
explicit caller choice:

- `warn` permits writing only when the affected information remains reversibly
  preserved, for example as a supported opaque facet; the report remains
  mandatory.
- `drop` permits an explicit destructive conversion and records every dropped
  item and the responsible rule in the report.

Downgrades use the same default and are fail-closed. A successful preflight,
complete capability coverage, and an unambiguous target profile are required
before any writer creates its temporary output; validation must succeed before
that output atomically replaces the destination.
