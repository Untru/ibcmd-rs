# Design: file infobase extension export

## Storage boundaries

The filedb crate owns the measured registry table layout and returns typed
rows, not SQL text or XML. Its reader validates named fields, lengths,
record boundaries and blob chains before exposing a row. The root adapter
reuses the existing extension registry payload decoder and CAS resolver;
storage hashes are not logical object names and independent extensions must
never be combined by enumerating all CAS payloads as source assets.

One FileDb instance keeps the read lock from the registry read through the
last CAS read and publication. The implementation is read-only. Unknown
registry profiles and unmeasured prepared states fail closed rather than
silently choosing a different generation.

The measured file registry has the a2 envelope; SQL retains its existing a1
decoder. CONFIGCASSAVE keys use the physical registry identity's namespace.
A selected prepared configinfo takes priority over the active registry root;
its content digests can resolve from either supplied rows or active CAS.
Every supplied logical name must match that manifest. A live
_EXTENSIONSINFONGS registry remains unsupported. The base metadata provider
reads CONFIG through the same descriptor, so adopted references need no SQL
connection or second file open.

## Publication

Reuse the extension exporter and its staging/publication boundary. A missing
name, duplicate name, malformed registry, absent CAS digest or unsupported
option must fail before a result is published. No native subprocess runs in
the production route.

## Alternatives

Calling the platform would hide the missing implementation and require a
platform installation. Treating CONFIGCAS as CONFIG would mix extensions
and discard the CAS manifest contract. Reimplementing the form writer would
duplicate #414 and change an unrelated layer. None is acceptable.

## Risks and integration

Native file registry facts must be measured independently of SQL
_ExtensionsInfo. Active and prepared images require explicit evidence.
PR #481 changes filedb reading/locking, #485 changes export queries and
#494 changes infobase routing: keep this change narrow and reconcile common
files when integrating. Existing code owns bounds and typed CAS identities;
new parsing must not guess unknown values or panic on external data.
