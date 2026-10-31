# Proposed compiler mutation authority (analysis only)

No shared effective-view extraction, mutation manifest, SQL transaction or RPC
route described here is implemented. The full verified compiler map is an
expected-state artifact, not authority to replace all of CONFIGSAVE. This
proposal requires a separate ownership/review decision before shared edits.

## Current reusable policy

`mssql_dump/file_infobase.rs::file_infobase_config` checks unfinished state and
folds existing storage generations through OfflineRows. `file_infobase_effective`
then checks complete staging, unsupported staged deletions and inventory coverage,
and merges the recognized stage. Its input happens to be FileDb ConfigRows;
SQL must not manufacture FileDb timestamps/headers to call it. Export's lenient
`publish` fallback is also not a safe editor-import admission policy.

`stored_objects::stored_rows_selected` already resolves metadata identities and
owned members through the shared inventory, using schema-owned root/nested
collections. Its current rows_under behavior includes id/suffix ranges; that is
useful read-side evidence, not exact file-role mutation authority. SQL must not
reuse a UUID wildcard as permission, invent a second list of object kinds or
infer owner UUIDs from arbitrary filenames. The binding index must detect
collisions rather than overwrite an earlier owner for the same source path/key.

## Proposed pure seam (not an existing API)

Inputs: complete assembled packed CONFIG and CONFIGSAVE maps, each obtained from
its separately validated physical parts, plus cumulative cancellation controls.
No filesystem path, connection, timestamp synthesis or write method is an input.

Output: the strict effective packed view used by the compiler; actual stored
names; validated stage status; and provenance mapping each effective name to its
actual table/stored name and any generation overlay. Driver-specific part/header
images remain in the original immutable SQL readset, outside the shared policy.

Before any extraction, add an explicit coverage invariant: staged versions
listing only A must not prune Config-only B and then validate the already-pruned
set. Existing historical generation projection is a separate policy with its own
bound proof; it does not authorize a new partial import to omit B. Validate the
pre/post exact key and binding coverage before any stage-based pruning. The
current helper cannot be transplanted unchanged as SQL write admission.

After those contracts are independently reviewed, a separately authorized
mechanical extraction can reuse the existing strict import policy. Retain the FileDb wrapper's physical assembly, API and behavior.
Do not copy the lenient export fallback. Any provenance needed beyond today's
OfflineRows result is an explicit separately reviewed extension; it cannot be
recovered by assuming every canonical name is its physical plain-name row.

## Selected owner-to-key manifest

A prospective manifest must bind all of:

- Actual target identity/schema/full physical readset digest; platform/profile;
  source census and immutable compiled expected tree; exact selection/connection
  epoch and the later one-use plan lifetime.
- Each selected file's canonical metadata owner, type, UUID and parent chain,
  resolved through the shared engine on the old effective configuration and the
  compiled expected configuration. The identity and ownership relation must agree.
- Exact selected source paths and the engine-derived allowed stored key sets.
  A file selection must not silently become recursive whole-configuration consent.
  Object-selection recursion must match the requested existing object semantics.
  New/deleted/renamed metadata identities and ownership ambiguity are refusals.
- Required global inventory keys separately from owned body keys. Inventory
  changes must be validated through the existing origin-aware versions parser,
  not accepted because a row is named `versions` or has a changed digest.

Constructors of such a manifest must be internal to the verified compiler path.
New compiled bindings may narrow or invalidate a prior authority, but may never
expand it. UUID/kind/owner/exact-key-set changes are refusals for this first
existing-only scope. No client parameter may supply owner/key lists, schema fingerprints, mutation
allowlists or a verification-disable option. Unresolved dependencies require a
new prepare after consent, never expansion of an already-authorized manifest.

## Old/new binding and minimal mutation proof

For every compiled body-key difference, require an admitted old/new owner witness
and explicit membership in the selected key authority. Missing old keys, deleted
keys, unknown ancillary rows or a new stored name are refused in the first scope.
A compiler re-encoding outside selected authority is a refusal, not an invitation
to restage an unselected row. Identical packed rows create no replacement entry.
The independently verified expected tree must still match selected source edits
and preserve every other exported file.

Global versions must retain the old effective key coverage and every unselected
entry's version, including entries from pre-existing valid staging. Only changes
justified by selected owned keys and measured required service semantics may be
emitted. A whole compiler-generated versions blob is not sufficient proof: CF
and SQL inventories have distinct count/service conventions in config_dump_info.
Other service rows (root/platform/deletion/commit/generation markers) are not a
blanket global allowlist. An unexplained service change is refused before SQL.

Apply an admitted replacement only to that exact physical stored name, replacing
all of its old parts and no other name. Preserve every unselected CONFIGSAVE
part's name, part number, total size, Creation/Modified/Attributes and bytes.
Preserve all CONFIG rows. A shared versions replacement is the explicit global
exception, with the stricter per-entry preservation proof above. No full-table
clear or replace_config_save route is allowed. Physical native part sizing and
headers for selected replacements require measured SQL policy, not FileDb values.

An existing active-generation mapping needs exact inverse provenance and actual
acceptance before any SQL write authority can include it. Until that is supplied,
refusal is preferable to inventing an alias or silently writing the plain row.
This does not add dynamic apply or change existing engine generation semantics.

## Required independent regressions

1. Module and nested form selections resolve stable existing owners on both old
   and new views; unrelated owners/rows have no mutation authority.
2. Config-only B or a pre-existing staged B omitted by incoming versions must
   refuse before pruning. A positive historical-dynamic exclusion control does
   not permit that same omission in a new partial stage.
3. Missing/ambiguous/new/renamed owners, unlisted keys, new stored names or an
   unselected compiler byte change refuse before any SQL mutation dispatch.
4. Missing selected output or an omitted required inventory entry refuses, even
   if a renderer could export the remaining tree successfully.
5. Seed an unselected multipart stage with distinct per-part headers. Selection
   shrinks another staged name to one part; neighbor headers/parts and CONFIG
   remain exactly unchanged. Preserve neighbor inventory versions as well.
6. Corrupt an unselected versions entry or insert an unexplained service-row
   change: refuse, rather than accepting full-tree verification alone.
7. Feed active-generation aliases: either prove exact old/new physical mapping
   under its measured policy or refuse. No fallback to physical plain names.
8. Change any selected/unrelated CONFIG row, stage header/part, identity, schema,
   source witness or connection epoch after prepare. Full readset CAS/guard must
   refuse before DML. Two plans against one baseline cannot both overwrite it.
9. Cancellation/limit failures invalidate the complete manifest. Late commit
   acknowledgement loss has an unknown outcome and cannot replay automatically.

## Publication seam remains separate

A pure manifest does not establish atomic SQL CAS. Publication requires retained
same-session table/range locks across exact readset validation, final source /
policy/cancellation checks, selected DML, verification and commit. Existing
separate SqlClient calls cannot supply that lifetime. A dedicated scoped session
API, if needed for client-side checks while locks are held, requires its own
review; a result-row callback is not acknowledgement that pauses the server
before later statements in the same batch. An omitted or unauthorized compiled key is an error, not silently filtered
output. Validate the final merged export as well as the per-key authority. No
publisher is enabled by this proposal or by the read-only snapshot adapter.

## Explicit proposed records (not implemented Rust API)

- `ProjectedBaseline`: packed body/service rows before CONFIGSAVE inventory is
  considered; canonical body coverage; chosen physical CONFIG origin for each
  logical key; excluded physical history plus the existing engine rule/witness
  justifying each exclusion. Bind the whole record to the physical readset.
- `OriginalStageWitness`: assembled original stage body coverage and parsed
  inventory evidence captured before incoming compiler pruning; retain references
  to every original physical header/part. Inventory-entry IDs and stored-body
  keys are distinct fields, joined only by existing parser/metadata evidence.
- `BindingWitness`: exact selected source path and role, canonical metadata UUID,
  kind and complete owner chain, and exact stored keys. Preserve old/new records;
  reject path/key collisions or a changed record even if payload bytes are equal.
- `ManifestInputs`: immutable snapshot/source/selection bindings, those independent
  baseline/stage witnesses, old/new binding witnesses and the complete compiled
  packed map. Parsed global inventory witnesses include entry-to-body relations
  and origin/count semantics; they are not caller-supplied allowlist strings.
- `ProposedDelta`: changed selected body keys with old/new byte hashes, separately
  justified global changes, exact retained original-stage physical references and
  final merged export evidence. It is not a publishable plan or permission token.

A future engine-owned constructor must establish those witnesses. Set equality
on forged/already-pruned input is not an authority check. The before/after owner
relation and exact key set cannot be inferred from a payload digest alone.

## Owned structural coverage checkpoint

`server/sql_import/coverage.rs::require_preserved` currently checks only bounded
set algebra over already-supplied canonical BODY witnesses: independently
projected pre-stage keys union original-stage keys must equal effective keys;
effective keys must equal complete compiled keys. Limits count all supplied
witness entries/bytes, with cancellation during traversal and comparison.

Four clean-room negative/positive contracts cover omitted B, omitted original
stage C, unexpected compiled D, separate preprojected historical coverage,
cardinality/byte limits and typed cancellation. Independent scoped static review
found no defect. The combined integration suite passed all 27 tests and the
subsequent library check exited 0, with exact code hashes retained in the external
coverage verification receipt. This helper is not wired to the existing effective-view engine or
SQL route. It does not resolve ownership, interpret versions/service records,
prove historical exclusions, authorize a replacement set or fix the existing
shared FileDb helper. Those remain explicit separate review/integration gates.

## Existing exact codec-output evidence (read-only finding)

`load::export_base` already returns StorageImageSourceExportReport. Its raw
`storage.entries` retain each codec's logical key/name, disposition and output
paths. The neutral planner currently discards its old export report. A future
additive SQL witness path can retain those raw edges, then obtain the equivalent
new export evidence under mandatory verification without changing FileDb's API
or introducing another owner resolver.

Do not use `load::keys_by_output` as permission: its BTreeMap insertion at
src/load.rs silently overwrites a path when more than one entry claims it.
Construct a bounded collision-detecting multimap from raw supported export
entries. Check exact old/new path-to-key relations and canonical metadata
UUID/kind/owner relationships, and refuse unsupported/ambiguous claims. A codec
output edge alone does not establish metadata ownership; it must join the
existing engine's canonical owner evidence. No UUID-prefix expansion is allowed.
The legacy files_stage trim function has body-path provenance, but executing
legacy staging to obtain that information is not a safe substitute.

Capturing new evidence, extending Prepared, or changing export/verification
internals is a separate review/ownership decision. No such implementation or
shared loader/report change is part of this analysis checkpoint.
