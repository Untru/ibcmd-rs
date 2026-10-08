# Artifact scopes for future CFE/EPF/ERF bootstrap

These source APIs implement identity and inventory selection. They do not yet
compile or publish extension/external packages. The public bootstrap guard
continues to refuse those packages until the complete builders are ready.

`inspect_package_identity` uses the existing XML expanded-name resolver. An
external root retains its main UUID and exactly one direct readable-namespace
ContainedObject class/ObjectId. `ArtifactScope::from_source_identity` checks the
actual class using the existing external family registry. Configuration and
Extension retain their canonical Configuration UUID.

`collect_artifact_identities` consumes the same `ValidatedConfiguration` proof
as ordinary bootstrap. External metadata remains DataProcessor/Report in this
canonical graph, with ContainedObject ObjectId as its semantic and module owner.
Every external package object must belong transitively to that root; context
objects cannot become package leaves. The main UUID may alias only that root's
primary storage row. A collision with any other object or generated identity
refuses. Its modules/help remain ObjectId suffixes.

| Scope | Reserved service inventory |
| --- | --- |
| Configuration | root, version, versions |
| Extension | configinfo |
| ExternalDataProcessor / ExternalReport | root, version, versions, copyinfo |

The shared graph retains exact routes, ownership and inventory validation.
`BeforeServices` admits only actual compiled object/assets. Existing CF wrappers
and special-entry compilers require Configuration scope, preventing an external
or extension graph from accidentally producing a synthetic ordinary CF root.
No new graph IR, body constructor, source projection, copyinfo builder, native
acceptance, or reference-context support is claimed here. Existing neutral
model/storage contracts remain; no new count/size/depth quota is introduced.

Generated controls use actual namespace inspection, canonical validation and
graph APIs, including class/UUID/collision/foreign-root refusals and distinct
external primary/module identities. Own extension Version 1.0.0 and 1.0.1 are
separate cleanroom source inputs, not a built release or foreign fixture edit.

The identity inspector delegates package classification to the current
`inspect_package_intent` unchanged. Ordinary `NamePrefix` (empty or populated)
and `ConfigurationExtensionCompatibilityMode` do not select Extension;
actual extension purpose/adoption/mapping markers retain the existing scoped
namespace and duplicate checks. Identity selection does not validate or rewrite
those property values. The ordinary/nondefault-compatibility control covers
both XML 2.20 and 2.21, comparing explicit scope projection with the existing
Configuration collector and exact graph inventory.

This is the scoped A0 port from original commit `06459188` and repaired commit `2445af7f`
owner context, reconciled onto the current package classifier. It does not
merge their ConfigInfo/load/CLI changes or make those historical heads ancestors.
The original four artifact-scope controls are retained without assertion changes.
SOURCE readiness does not carry over old runtime results or complete issue #438;
ROOT must run fresh tests on the exact new source revision. Existing inherited
storage/text bounds are a separately tracked later policy atom, not a new limit
introduced by this scope layer or a claim that all external paths have no caps.


## External canonical root component (A1, partial)

The library exposes `decode_external_root` / `encode_external_root` and explicit
ExternalDataProcessor/ExternalReport metadata codec registrations. These map the
source root to the existing canonical DataProcessor/Report kind under the actual
ContainedObject ObjectId; the private validated source binding preserves physical
main UUID/class separately. Existing Configuration wrappers and CLI external
bootstrap refusals remain unchanged. This API does not create an EPF or ERF.

The component accepts XML 2.20 and 2.21 independently, exact declared Object
TypeId/ValueId, known root properties and owned named Form/Template references.
Known CURRENT text/name/synonym/default references edit through the existing XML
writer; a consistent name edit must also update its ObjectTypeName and all owned
references. Root/child/type identity edits, source property-slot changes,
synonym-item topology changes and cross-edition facet migration currently return
an explicit error. They never silently publish old or partial values. Unknown
attributes/properties/sections, duplicate identities and foreign/undeclared
references refuse admission. AuxiliaryVariantForm is an explicit external Report
2.21 root value; XML support proves no fresh nonempty native header mapping.

This first component does not admit embedded attributes/tabular sections or read
files/directories, resolve configuration-context storage references, compile
forms/assets, construct external main/services, or publish storage. Complete A1
owner intake and A2–7 acceptance remain pending. Inherited common metadata/core
bounds are unchanged and remain the A6 SourceOperationPolicy dependency; no new
quota is introduced and unrestricted configuration support is not claimed.

Source readiness alone does not establish native/public acceptance. The new own
cleanroom controls need fresh ROOT execution on the reviewed exact commit, along
with unchanged A0/package/classifier/metadata controls. Native/SDK expected files
remain independent, read-only evidence and are not copied into repository tests.


### Strict external root and ObjectModule inventory (partial A1)

The non-publishing `compiler::external_intake::ExternalIntake` API inventories
one direct root file's parent or a directory. Both modes select the root from
its complete parsed document in the SAME retained SourceTree. Root filename
never selects DataProcessor versus Report. Every inventoried entry is either
that root or exactly the existing internal-family ObjectModule route; module
claims bind the contained ObjectId, source path, suffix and exact content hash.
A0 identity/graph checks remain shared. A claim is source ownership, not native
body compilation or permission to publish an EPF/ERF.

The new explicit `read_source_tree_strict` uses caller-selected existing
ReaderLimits, refuses symlinks/reparse/nonregular files and operational paths,
and refuses empty orphan directories. The old Configuration reader retains
its existing behavior. Current edits prepare another immutable tree from the
same root envelope and exact already-owned module replacements; unknown or
duplicate replacements fail before a result. Root edits use the existing
canonical editor and complete decoded forward equality. Retained intake never
rereads later disk changes while preparing current edits. A memory `from_tree`
constructor proves its supplied inventory only, not the absence of omitted
files on disk. A read snapshot is retained authored bytes, not an OS-wide
immutable source/directory or custody receipt.

This slice honestly refuses declared Form/Template inventories, embedded
metadata, body/help/Manager/other files instead of silently consuming them.
Full typed child/body/assets intake is remaining A1 work. Existing core/XML/
source policy budgets are inherited pending A6; no new count/size/depth quota
is introduced. Configuration context, fresh external main/header, services,
complete assets/publication and native platform load remain A2–7. No CLI or
new working EPF/ERF builder is advertised. Tests and review on the exact current
component remain required; previous root-component PASS is not its acceptance.
