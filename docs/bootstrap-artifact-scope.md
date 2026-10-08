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
both XML2.20 and2.21, comparing explicit scope projection with the existing
Configuration collector and exact graph inventory.

This is the scoped A0 port from original commit06459188 and its2445 repaired
owner context, reconciled onto the current package classifier. It does not
merge their ConfigInfo/load/CLI changes or make those historical heads ancestors.
The original four artifact-scope controls are retained without assertion changes.
SOURCE readiness does not carry over old runtime results or complete issue438;
ROOT must run fresh tests on the exact new source revision. Existing inherited
storage/text bounds are a separately tracked later policy atom, not a new limit
introduced by this scope layer or a claim that all external paths have no caps.
