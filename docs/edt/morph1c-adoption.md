# morph1c adoption for milestone 0.7

Assessment made 2026-10-01 against the immutable upstream revision
[`962eedddd14493a914bae11f11667951de0f23a7`](https://github.com/Segate-ekb/morph1c/tree/962eedddd14493a914bae11f11667951de0f23a7).
Its workspace manifests declare `MIT OR Apache-2.0`; the copied snapshot chooses
Apache-2.0 and includes a license and attribution notice. Original and adapted
source inventories are in `crates/ibcmd-edt/vendor/morph1c/{UPSTREAM,LOCAL}-FILES.json`.

## Actual reuse boundary

| Upstream source | Host use | Boundary |
| --- | --- | --- |
| `crates/core/src/{engine,resolve,spec,ir,version}` and build inventories | Metadata field specs, version defaults and private codec intermediate | Never a second public canonical model; no public morph types |
| `crates/formats-xml` | Typed ordered XML/body codecs, forms, rights, XDTO, DCS and projections | Host validates UTF-8/XML depth, event/attribute budgets and source-derived filenames first |
| `crates/formats/{edt,designer}` | Descriptor read/write registry and typed source conversion | Namespace/version/totality guards retained; BOM/EOL spelling is host provenance |
| `crates/pipeline` source-family modules | Modules, forms, templates, pictures, help, schedules, command interfaces, children and languages | CF/detection/survey/CLI entrypoints removed; full bodies always read; error-collection skip mode removed |

The private packages are pinned using root Cargo patches into the local snapshot
and excluded as a nested workspace. The host workspace contains `ibcmd-edt`,
not the upstream packages. This avoids pulling upstream research tests and lint
policy into host gates while compiling the exact attributed dependency source.
Versioned build tables are retained because codec defaults depend on them; their
source data does not constitute installed-platform acceptance.

`ibcmd-edt` exposes `Project`, bounded `SourceTree` inventory, `ConversionOptions`,
file accounting and the established `ibcmd-core::CanonicalConfiguration`.
The public metadata bridge uses the existing XML metadata-envelope source
decoder, preserving only its exact family-specific named Form/Template/Subsystem
and Recalculation references beside UUID-bearing children: ordered scalar/typed
properties, child identities/owners/references,
generated types and explicit opaque complex facets stay in the established
canonical contracts. Standalone Form/Template/Subsystem/Recalculation owners are
linked only through validated explicit ChildObjects references and exact descriptor
paths; neighbouring paths alone do not establish ownership.
Module/form/template/binary bodies are content-addressed
asset references, with bytes retained by the source tree. Physical import
family-codec restrictions are not used as a second EDT format gate.

## Safety and reversibility

Every source file must have a matching regenerated descriptor/body, or be an
explicitly retained `ConfigDumpInfo.xml` in the original XML payload; otherwise
conversion errors with the path. Known project controls (`PROJECT.PMF`, Eclipse
project builders/natures, UTF-8 preferences) are parsed and consumed as converted
container metadata. Unknown PMF fields, project fields/settings and DT-INF files
are rejected. Native EDT to XML to EDT is not a byte-exact Eclipse-control-file
round trip. No control files are added to native XML inventories.

Both readers reject symlinks/reparse points, unsafe/colliding paths and budgets
before borrowed parsing. No editor/build folders are silently skipped. All
metadata/child/help names used as file components are checked before borrowed
body reads. Source snapshots are private and use the same explicit large source
budgets as publication; destination publication remains the host's no-clobber
staged writer. No product conversion invokes Java, EDT or a native platform.

The supported project model is explicit EDT 2025.2.3 with XML 2.20 or 2.21.
XML-to-EDT requires an explicit runtime version agreeing with the XML profile;
EDT-to-XML checks it against `DT-INF/PROJECT.PMF`. Generated projects include
authentic Xtext/V8 builder/nature scaffolding (the native import base set and
its witnessed translation-builder variant), UTF-8 settings and a stable Eclipse
project name derived from the existing configuration UUID.

XML-to-EDT stores original lexical XML/body bytes under `.ibcmd-provenance`.
The manifest fingerprints **all** generated files, including PMF, project controls
and every source asset. Exact unchanged return requires the same explicit
profile, complete inventories/hashes and matching typed configuration semantics.
The original XML is independently decoded and checked against current EDT;
updating a payload hash cannot legitimize contradictory metadata. Edits/additions/
deletions fail as stale provenance. Removing that directory explicitly selects
typed conversion and preserves edited metadata/modules through codecs.

The private property bag uses stable FieldId lookup; Designer and EDT projection
enumeration orders differ. Only that bag is sorted as borrowed references for streamed semantic fingerprinting;
children, modules, forms, templates, values and all other ordered arrays remain
ordered. Public canonical properties and retained original XML remain ordered.
Local source-only additions cover accumulation-register aggregates, calculation
register recalculations (including generated types and dimensions), and chart of
calculation types predefined items with their dependency lists. Their closed
grammars map EDT inline descriptors to native XML sidecars; all typed fields are
included in the same semantic fingerprint. Unknown cells, namespaces and enum
values fail explicitly. Configuration help, localized short caption and the
witnessed 8.5 interface migration mode use existing typed property/body contracts.
These additions are local adaptations, recorded separately from pinned upstream
file hashes. Local PaletteColor metadata and EnumValue color projections reuse
the existing form RGB/reference converters with exact QName namespace bindings;
PaletteColor requires an explicit XML 2.21 profile. Color extras, duplicate or
out-of-range RGB components and unknown references fail explicitly.
Client application interface regions are optional typed groups: an absent top
region stays absent, while panel membership and unset/definition tables remain
validated. No group UUID is synthesized for an absent region.
Form command-interface parameters reuse the existing DataPath grammar and map
EDT `commandParameter` to XML `Attribute`; both optional presence and content are
part of typed form semantics. Native role visibility witnesses prove that an
absent EDT CMI `for/value` means false, while XML retains an explicit false.
Inventory equivalence permits only that exact EDT form/CMI scalar spelling;
true, unknown attributes/namespaces, mixed content and `xml:space` stay distinct.
The canonical bridge checks core object/member/retained-byte budgets before
cloning metadata or retaining asset references, checks each owner's asset count,
and checks the final graph including separately declared ownership.
Structured body formatting equivalence is restricted to explicitly listed typed
body roots with codec-defined BOM/EOL/indent conventions, respecting mixed text
and inherited `xml:space`. Unknown XML bodies require exact bytes. Prolog/epilog
fragments, comments and changed fields cannot disappear through that comparison.

## Tests and acceptance scope

The small offline fixture is derived from upstream
`.fixtures/probes/subsystem_ci/src`. Its originally mixed 2.20/2.21 envelopes
were made consistently 2.21 and its XML line endings consistently CRLF for the
host unit fixture. This is an explicitly synthetic codec fixture, not native
BSP/UH acceptance. Tests cover exact XML return, typed provenance-stripped return,
public properties/assets, changed modules, added/deleted files, altered DT-INF,
tampered retained XML with updated hashes, unsafe names, parser depth, unsupported
source/control files, meaningful whitespace and prolog preservation.
Typed-extra tests also change aggregate payloads while recomputing manifest
hashes, and prove an edited aggregate survives conversion after explicit removal
of provenance. An opt-in, bounded read-only corpus witness compares aggregate,
recalculation and calculation-predefined semantics between independently
prepared native XML and installed-EDT project files. It invokes no native tool and
does not substitute for whole-project import/export acceptance.

Installed EDT/native acceptance is recorded separately by the oracle harness.
Upstream's acceptance claims are references for investigation, not acceptance
evidence for ibcmd-rs. In particular, the source-only writer does not manufacture
native `ConfigDumpInfo.xml` configVersion storage values from an EDT project;
exact native return is proven by retained XML only for an unchanged generated
project. Native import/export and inventory comparison must adjudicate that
boundary independently.

## Applicability outside 0.7

| Area | Useful concrete source | Reuse decision and limit |
| --- | --- | --- |
| 0.6 — whole CF/CFE compilation | [`formats/cf`](https://github.com/Segate-ekb/morph1c/tree/962eedddd14493a914bae11f11667951de0f23a7/crates/formats/cf), especially assembly/body modules | Strong research/oracle reference for independent format gaps; excluded from this adapter to preserve the existing ibcmd-cf/storage architecture |
| 0.8 — version migrations and corpus coverage | [`core/version`](https://github.com/Segate-ekb/morph1c/tree/962eedddd14493a914bae11f11667951de0f23a7/crates/core/src/version), inventory/model witnesses | Useful default/availability witnesses and negative cases; not a substitute for host explicit-profile migration graph and platform proof |
| 0.6/0.8 — forms/DCS and XDTO coverage | [`formats-xml/src`](https://github.com/Segate-ekb/morph1c/tree/962eedddd14493a914bae11f11667951de0f23a7/crates/formats-xml/src) | Typed source transcoders reused here; physical storage compilation still belongs to existing host codecs and native evidence |
| Semantic comparison/testing | [`testkit`](https://github.com/Segate-ekb/morph1c/tree/962eedddd14493a914bae11f11667951de0f23a7/crates/testkit) | Useful test design and adverse cases; not a production dependency and not authority to ignore host inventory or unknown fragments |
| 0.5 — high-load generation/import | [`pipeline/cf_stream.rs`](https://github.com/Segate-ekb/morph1c/blob/962eedddd14493a914bae11f11667951de0f23a7/crates/pipeline/src/cf_stream.rs) | Descriptor-first/body-streaming design may reduce memory; no SQL generation switching or database transaction guarantees supplied, so no automatic completion of other milestones |

Milestone labels follow the repository's maintained [roadmap](../../README.md).
No reusable SQL Server generation-switching, PostgreSQL backend (0.9), or file
database storage/transactions (0.10) is supplied by this source-codec closure.
Those tasks need their own implementation and evidence. No other milestone is
implemented or closed merely by including these sources.
