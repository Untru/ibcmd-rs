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

The repository embeds six minimal experimental seed profiles: three XML
dialects (`2.17`, `2.20`, and `2.21`) and three exact platform builds
(`8.3.24.1819`, `8.3.27.1989`, and `8.5.1.1150`). Each seed declares exactly
one version axis. There is no inheritance or mapping between a platform build
and an XML dialect, and no compatibility, storage, container, or DBMS value is
inferred. XML fingerprints record only an observed `xcf.version`; platform
seeds deliberately contain no invented fingerprints or capabilities. Empty
evidence on the 8.3.24 and 8.5.1 seeds is honest: the coordinate is requested
for future research, not verified support.

`profile_registry::BUNDLED_PROFILES` embeds these files at compile time and
`load_bundled_profile_registry` resolves them without filesystem or platform
access.

## Exact detection

The core detector accepts bounded, independent observations for an exact
platform build, exact XML dialect, and open fingerprints. It matches a profile
only when every supplied observation is explicitly present and equal. Empty,
contradictory, or unmatched input is `Unknown`; multiple exact matches are
`Ambiguous`. It never chooses a nearest version or maps one version axis to
another. Writers must call `require_exact_target` before selecting an encode
profile.

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
