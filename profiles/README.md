# Version profiles

Version profiles describe independent coordinates used by the standalone
conversion core. A profile may name a platform build, XML dialect,
compatibility mode, logical storage profile, container revision, and DBMS.
None of these values is inferred from another value, and loading a registry
never selects a profile for an artifact.

`schema.json` is the public JSON Schema for declaration format version 1. The
Rust parser is also strict: unknown or duplicate fields, duplicate map keys,
invalid bounded identifiers, malformed version coordinates, and unsupported
schema versions are rejected. Identifier and capability namespaces are open so
future names do not require a code change. Dotted versions contain two to eight
canonical decimal `u32` components (`0` through `4294967295`) without leading
zeroes.

This directory intentionally contains no seed profile documents. Seed profiles
and their evidence are delivered separately.

## Inheritance and merge rules

`extends` names at most one parent. Resolution is recursive and parent-first.
Self-parenting, absent parents, cycles, and duplicate profile IDs are errors.
Every root must declare `status`; a bundled child may inherit it.

- A child scalar replaces the parent scalar independently of all other
  coordinates.
- `fingerprints`, `constants`, and `capabilities` merge by key. A child value
  replaces the value for the same key.
- Capability `unsupported` is an explicit value and therefore overrides
  inherited `supported` just like any other child declaration.
- Evidence strings are combined, sorted, and deduplicated. When a child repeats
  inherited evidence, the original declaring profile remains its source.

Every effective scalar and every effective map, capability, or evidence entry
includes `declared_by`. Effective profiles also retain parent-first inheritance
and named-source chains. This provenance makes overrides auditable without
guessing from the final values.

## Determinism and external profiles

The core accepts named bundled JSON inputs directly. The application adapter
can additionally load regular files whose extension is exactly `.json` from
one directory. It sorts UTF-8 filenames before parsing and records canonical
source names such as `external/example.json`; directory enumeration and caller
input order cannot affect the resolved registry.

External files are untrusted extensions. Each one must explicitly declare
`"status": "experimental"`; an inherited status is not sufficient. Duplicate
IDs across bundled and external sources are rejected. External profiles cannot
declare `verified`, and a bundled descendant cannot resolve to `verified` while
any external source remains in its ancestry. Such descendants must stay
explicitly `experimental`.

Default filesystem limits are 256 external files, 1 MiB per file, and 8 MiB in
total. Callers may choose smaller bounds through `ProfileRegistryLimits`.
Symlinks, subdirectories, and non-JSON files are ignored. Loading and resolution
perform no platform, JVM, process, network, capability-probing, or profile-
selection operations.
