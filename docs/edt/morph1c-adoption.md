# morph1c adoption for milestone 0.7

Assessment made 2026-10-01 against the immutable upstream revision
[`962eedddd14493a914bae11f11667951de0f23a7`](https://github.com/Segate-ekb/morph1c/tree/962eedddd14493a914bae11f11667951de0f23a7).
Its workspace manifests declare `MIT OR Apache-2.0`; the copied snapshot chooses
Apache-2.0 and includes a license and attribution notice. Original and adapted
source inventories are in `crates/ibcmd-edt/vendor/morph1c/{UPSTREAM,LOCAL}-FILES.json`.

The source-derived OpenJDK 17 number formatter is a separate Rust library,
`crates/ibcmd-number-format`, outside this Apache-selected snapshot. It retains
the original copyright and `GPL-2.0-only WITH Classpath-exception-2.0` license,
including the exception for the modified library. Its notice records the
original source SHA and the translation date. The release archive includes the
complete modified library source and notices; its actual license is in the SBOM.

## Actual reuse boundary

| Upstream source | Host use | Boundary |
| --- | --- | --- |
| `crates/core/src/{engine,resolve,spec,ir,version}` and build inventories | Metadata field specs, version defaults and private codec intermediate | Never a second public canonical model; no public morph types |
| `crates/formats-xml` | Typed ordered XML/body codecs, forms, rights, XDTO, DCS and projections | Host completely validates UTF-8/XML and source-derived filename safety first |
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
The CLI uses the additive `DirectorySource`/`DirectoryConversion` path: owned
disk snapshots retain payloads, while inventories retain paths, identities,
lengths and hashes. Existing memory API contracts remain available.
The public metadata bridge uses the existing XML metadata-envelope source
decoder, preserving only its exact family-specific named Form/Template/Subsystem
and Recalculation references beside UUID-bearing children: ordered scalar/typed
properties, child identities/owners/references,
generated types and explicit opaque complex facets stay in the established
canonical contracts. Standalone Form/Template/Subsystem/Recalculation owners are
linked only through validated explicit ChildObjects references and exact descriptor
paths; neighbouring paths alone do not establish ownership.
Module/form/template/binary bodies are content-addressed
asset references, with bytes retained by the source tree or owned disk stage. Physical import
family-codec restrictions are not used as a second EDT format gate.

## Safety and reversibility

Every source file must have a matching regenerated descriptor/body, or be an
explicitly retained `ConfigDumpInfo.xml` in the original XML payload; otherwise
conversion errors with the path. Known project controls (`PROJECT.PMF`, Eclipse
project builders/natures, UTF-8 preferences) are parsed and consumed as converted
container metadata. Unknown PMF fields, project fields/settings and DT-INF files
are rejected. Native EDT to XML to EDT is not a byte-exact Eclipse-control-file
round trip. No control files are added to native XML inventories.

Both readers reject symlinks/reparse points and unsafe/colliding paths
before borrowed parsing. No editor/build folders are silently skipped. All
metadata/child/help names used as file components are checked before borrowed
body reads. Disk inventory and publication use chunked copy/hash and complete
streaming XML inspection; they impose no default file-count, file-size,
total-size or path-depth validity ceilings. Actual filesystem errors remain
errors. Destination publication uses the existing exclusive atomic rename
after verifying the sibling stage. Both CLI directory routes explicitly use
the Source operation policy for XML profile detection and canonical metadata,
values, members and asset references. Checked accounting replaces fixed size
quotas on that path; syntax, identity, ordering, ownership and provenance checks
remain in force. Ordinary bounded memory APIs retain their existing contract.
Actual IO and allocation failures remain errors. Full-corpus and installed-EDT
acceptance is recorded separately; a successful size control does not establish
that every configuration has been verified. No product conversion invokes Java,
EDT or a native platform.

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
Paired BSP 8.5 form witnesses additionally map localized `choiceButtonTitle` and
`dropListHint`, `widthInCard=Half`, and the platform-spelled
`onMainServerUnavalableBehavior=DontChangeBehavior`. Enum grammars are restricted
to these observed values; no absent-field default is invented. Existing picture
codecs already preserve `choiceButtonPicture`. Strict parser censuses cover every
form in the genuine BSP 8.3 and 8.5 Designer/installed-EDT corpora; the laboratory
JSON reports record exact inputs and outcomes. Parser coverage and paired field
roundtrips are separate evidence from the installed-tool acceptance gate.
The canonical bridge propagates the explicit operation policy while accounting
for objects, members, retained bytes and each owner's asset references before
cloning metadata. It checks the final graph including separately declared
ownership. CLI Source operations impose no fixed count or scalar-byte ceiling;
ordinary bounded memory operations still enforce their published budgets.
Structured body formatting equivalence is restricted to explicitly listed typed
body roots with codec-defined BOM/EOL/indent conventions, respecting mixed text
and inherited `xml:space`. Unknown XML bodies require exact bytes. Prolog/epilog
fragments, comments and changed fields cannot disappear through that comparison.

DCS template QName projection uses an iterative namespace-aware cursor rather
than fixed byte/node/depth ceilings. Arbitrary unselected CDATA, comments and PI
remain byte-exact. Only the qualified current-configuration `AnyIBRef`/`AnyRef`
`TypeSet` leaf has a format alias. Its semantic view is recomputed from the
current typed template body and resolved binding; entity or split CDATA framing
is lexical. Other bytes and ordered comments/PI remain significant. Ordinary IR
serialization and native body emission retain actual source bytes. Directory
tests prove both routes with stripped provenance and exact unchanged native
return with independently validated provenance; edited types and forged hashes
cannot restore an old source. DTD remains forbidden.

Metadata-command pictures use an additional versioned resource,
`ibcmd-metadata-picture-semantics.v1.json`, next to the owning metadata descriptor.
The closed resource binds the owner UUID/kind and registered command UUID/kind,
Picture slot and current reference. Reference, LoadTransparent and optional
pixel coordinates remain current typed properties in semantic fingerprints;
only selected resource identities are lexical transport metadata. Native XML
emits all current values, including an explicit empty-reference container when
it carries nondefault transparency. EDT descriptors carry the ordinary SDK
PictureRef; the adapter consumes and re-emits the separate resource exactly once.
The conversion report identifies `ibcmd-metadata-picture-semantics/1` with actual
resource and carried-reference counts. Unknown, duplicate, deleted or stale
bindings fail before output publication. Stripped-provenance tests prove exact
native descriptor regeneration and pixel/flag edits; forged provenance hashes
cannot hide a changed tuple. This resource preserves adapter semantics and does
not claim that the installed EDT model or its native exporter stores per-use
transparency. Installed-tool compatibility is a separate laboratory gate.
For reference-only EDT metadata commands, the whole-project reader derives
LoadTransparent from CURRENT CommonPicture nullable transparentPixel presence,
as the installed SDK does. It never reconstructs a per-use pixel. Validated
resource-owned tuples override this derived default, including explicit false.
The writer and extension ledger use the same current configuration context, so
a reference matching this default needs no extra resource. Editing shared
picture metadata therefore updates an unowned command default while preserving
independently carried per-use values; semantic fingerprints detect both edits.

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

## Root MdPicture semantics

The source codec handles Logo, Splash and MainSectionPicture as typed picture
presence plus current nullable transparentPixel and glyph Points, with image bytes retained
separately. Native ExtPicture wrappers and EDT descriptor Point fields map to the
same property value. Null and Point(-1,-1) remain distinct; native explicit sentinel
coordinates retain their source spelling only while the current Point is unchanged.
Point/image edits participate in semantic and inventory checks. A rehashed manifest
cannot restore contradictory retained XML. Both XML2.20 and2.21 are exercised.

The installed Configuration importer routes all three slots to MdPicture, which
inherits PictureDef. MetadataPictureXmlExporter passes the current picture to
MetadataPictureDefWriter, which reads getTransparentPixel; it does not force null.
Historical bound primary artifacts: F:/ibcmd/lab/07/root-picture-sdk-contract-r1/result.json
and F:/ibcmd/lab/07/root-mdpicture-model-proof-r9/result.json. The latter proves
original SDK model save/load, both Point coordinate shapes and the persisted glyph
field. Point exposes only signed EInt x/y; the complete signed32 domain is retained,
and out-of-type coordinates are rejected. No name field is discarded. These are primary model
proofs, not headless project acceptance. The original image exporter copies the
current binary content directly; it does not crop or modify it using glyph.

EDT persists glyph directly in each MdPicture descriptor. Native SDK picture
wrappers omit glyph, so the adapter emits a closed, versioned semantic resource
`Ext/ibcmd-root-picture-semantics.v1.json` only when glyph is present. Each record
binds the current Configuration UUID and one declared slot with a unique current
image. All coordinates remain fingerprint-visible typed properties; no source
bytes are replayed. Native input consumes and regenerates this resource exactly
once; EDT output uses its actual descriptor field without a redundant sidecar.
The extension ledger reports resource/slot counts. Native SDK projection alone
does not preserve glyph; exact adapter roundtrips include the explicit resource.
Unknown fields, duplicate slots, deleted images, owner mismatches and forged
provenance are rejected; edited or deleted current glyph changes the output.

## Form event ownership and independent field state

The installed SDK allows open nonempty symbolic event references in Form,
Table and FormField body containers. A nested extension excludes events owned
by its actual parent; the base container excludes only the events of its
actually attached extension. Unknown references in group, decoration and
addition extensions follow their different SDK parent policy. Event names are
scalar identities and receive no UUID or filename grammar. XML namespaces,
typed ancestry, duplicate identities and current handler validation remain
enforced.

Field type and extension presence are separate SDK properties. Registered SDK
model validation accepts type None with LabelField or CheckBox extensions,
while the actual native writer projects None as InputField. Explicit mismatched
types produce SDK diagnostic 200. Historical primary evidence is in
`F:/ibcmd/lab/07/field-extension-primary-root-independent-review-r1.json`;
these small model probes do not establish full headless project acceptance.

The adapter uses `ibcmd-form-event-semantics.v1.json` beside the owning native
form only for state that the native projection cannot retain. A record binds
the declared form UUID, exact typed control path and complete current event
identities to the independent type and actual extension state. Top-level
handler values come from the current native form. When its InputField projection
cannot carry an actual extension's auto-table or additions, the resource owns
their complete current typed subtrees once, including nested handlers.
Extension values that require transport participate in semantic fingerprints.
The extension ledger
reports actual resources and owner records; a bare SDK native projection alone
cannot preserve all this state. Complete project conversion and matched SDK
validation remain separate acceptance gates.

## CURRENT form paths and coupled native accounting

AbstractDataPath retains semantic segments and ordered extra paths. Legacy scalar
paths remain compatible. The EDT form extension carries only values that its
standard projection cannot represent. Native XML uses one closed typed comment
in ConfigDumpInfo.xml, with the CURRENT declared metadata roster and no invented
platform incremental checksums. Unknown transport versions, duplicate or deleted
owners, stale counterpart values and profile mismatches are errors.

The normal complete configuration reader restores the typed values and verifies
all global UUID/context bindings before publication. Coupled body accounting
requires equality of the complete CURRENT configuration and complete restored
FormBody at its exact registered UUID/path. Errors from structural comparison
remain errors; unrelated artifacts keep their existing complete comparison.
A plain source ConfigDumpInfo retains its original bytes through provenance,
separately from the regenerated CURRENT control. The tests exercise both memory
and directory conversions, removal of provenance, CURRENT edits and forged data.

Dynamic-list required fields use selected backing definitions independently of
query aliases. Result batches and UNION operators own separate source namespaces;
only actual standard definitions in CURRENT metadata receive language twins.
The SDK public Order property is available independently of query columns.
Table additions are three distinct singleton features; duplicate assignments
are errors. Their semantic serialization does not reorder real child lists.

Native chart numeric output follows the original SDK primitive rounding and
integral long conversion. The existing chart transport preserves genuinely
lossy CURRENT binary64 values with the full projected counterpart binding.
Lexical layout does not substitute saved numeric values after an edit.
These focused corrections do not establish complete installed-EDT, native import
or large-corpus acceptance; the milestone gates remain separate.
