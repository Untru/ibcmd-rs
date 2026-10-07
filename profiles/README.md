# Version profiles

Version profiles describe the coordinates used by the standalone conversion
core. A profile may name a platform build, XML dialect, compatibility mode,
logical storage profile, container revision, and DBMS. The scalar coordinates
stay independent -- none is inferred from another, and loading a registry
never selects a profile for an artifact -- with one explicit exception: a
platform profile declares which XML format its build reads and writes (see
"Platform registry" below).

`schema.json` is the public JSON Schema for declaration format version 1. The
Rust parser is also strict: unknown or duplicate fields, duplicate map keys,
invalid bounded identifiers, malformed version coordinates, and unsupported
schema versions are rejected. Identifier and capability namespaces are open so
future names do not require a code change. Dotted versions contain two to eight
canonical decimal `u32` components (`0` through `4294967295`) without leading
zeroes.

The repository embeds seven minimal experimental seed profiles: three XML
dialects (`2.17`, `2.20`, and `2.21`) and four exact platform builds
(`8.3.24.1819`, `8.3.27.1989`, `8.3.27.2214`, and `8.5.1.1150`). The XML
profiles form a parent-first `2.17` → `2.20` → `2.21` baseline/delta chain.
Platform profiles stay independent with one evidenced exception: `8.3.27.2214`
extends `8.3.27.1989` because a full native export comparison re-confirmed the
same storage constants on that build. No compatibility, storage, container, or
DBMS value is inferred. XML fingerprints record observed XCF evidence and the
2.21 profile declares only confirmed feature deltas; platform seeds
deliberately contain no invented fingerprints or capabilities.

Native MSSQL write policy is declared by these profiles, not by code.
`8.3.27.2214` declares `mssql.main.write` and `mssql.extension.write` as
supported, because activation, live, worker and load parity were measured on it.
`8.3.27.1989` keeps its read evidence and declares no write capability, so
writes selecting it fail closed. The 8.5.1 profile records the exact live SQL
fingerprint and extension-registry read evidence; main and extension writes
remain explicitly unsupported because the native generation-selection protocol
is not yet reproduced.

The own exclusive `config apply` (`mssql-config-apply`, `docs/apply/own-apply.md`)
has a capability of its own, `mssql.config.apply`: it moves a stage that needs no
restructuring from `ConfigSave` into `Config` in one transaction and does not select
a generation. It is declared supported only for the builds on which its end state was
compared with the native apply on twins: `8.3.27.2214` and `8.5.1.1150`. It is
independent of `mssql.main.write`, so 8.5.1 can admit it while main writes stay
unsupported; `8.3.27.1989` declares neither and fails closed.

The drop-in `ibcmd infobase config apply --dynamic=force` (`docs/apply/dropin-dynamic.md`) has one of its
own, `mssql.config.apply.dynamic`: a small delta stage published as a dynamic (online) generation, with the
change registrations and `MobileVersions.dat` the platform's own `force` writes. It is declared supported
for `8.3.27.2214` only, the build the online transition and the twin were measured on; `8.5.1.1150` declares
it unsupported (its main writes are), `8.3.27.1989` not at all and fails closed.

`profile_registry::BUNDLED_PROFILES` embeds these files at compile time and
`load_bundled_profile_registry` resolves them without filesystem or platform
access.

## Platform registry

A platform maps to its XML format explicitly: 8.3.x reads and writes 2.20,
8.5.x reads and writes 2.21, and only for the builds the registry knows. Each
supported platform profile declares, as constants,

- `platform.xml_format`: the XML format of the build (`2.20`, `2.21`);
- `platform.form_layout`: the stored layout of its managed forms, colours and
  fonts, named by the version that introduced it (`8.3`, `8.5.1`);
- `platform.feature.<name>`: a feature a configuration saved by the build may
  list in its `version` row, with its fixed uuid (`palette-colors` on 8.5.1).

`src/platform/` reads these declarations (it does not repeat them) and answers
`--platform 8.3.27 | 8.5.1 | 8.3.27.2214 | 8.5.1.1150`: an exact build, or a
release standing for every known build of it while they agree. A build whose
profile declares no XML format (`8.3.24.1819`) is known but refused, and so is
every version no profile names; nothing maps to the nearest version. The
`--source-version` aliases `8.3`/`8.3.27` and `8.5`/`8.5.1` follow the same
mapping. A new build that changes nothing for ibcmd-rs is one more profile; a
changed layout or XML format also needs a delta module named after the full
version (`layout_8_5_4.rs`, `xml_2_22_*`), see `src/platform/mod.rs`. The
registry also reports live-activation support: an exact build whose profile
declares `mssql.main.write` supported, as above.

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
