# External named source ownership (N1)

`compiler::external_owned::ExternalOwnedSource` consumes one strict retained
`SourceTree` snapshot, using the same physical census and expanded-name package
admission as the existing root-only `ExternalIntake`. The old API still refuses
nonempty named declarations. Neither API creates an EPF/ERF archive.

Only actual root declarations authorize named Form/Template metadata. Their
exact names, UUIDs, owner ObjectId, source profile and full typed properties live
in the existing canonical objects/configuration. The package main UUID remains
separate. A0 validation and the owner graph precede native services. No synthetic
Configuration, Manager, foreign content, native profile downgrade or second IR
is introduced.

Form source properties include typed localized ExtendedPresentation and optional
2.21 UseInInterfaceCompatibilityMode. Absence is not a default. The schema reuses
the existing FormType, UsePurpose, interface-mode and TemplateType enumeration
domains. Ordinary six-property managed Form native compilation is unchanged.
XML edition selects source properties only, never native record13/14 or body50/59.

The exact metadata source filename stem binds each owner directory. Form body
`Ext/Form.xml` is mandatory; `Ext/Form/Module.bsl` is optional. Their independent
AssetReferences share one `.0` bundle route. Template bodies use the existing
TemplateKind source-file dispatch. XML envelopes and namespace/QName bindings
are checked without claiming semantic body compilation. HTML bundles, unsupported
ActiveDocument/GeographicalSchema body dispatch and other unconsumed files refuse
with their source path; no wildcard subtree is consumed.

`with_current` takes the current root envelope, complete canonical configuration
and replacements for already claimed asset paths. It preserves UUID/ownership,
metadata paths, declaration names, property presence/order and fixed asset role
paths. Property and asset edits are prepared in memory and re-admitted, proving
whole configuration, binding, ownership and asset equality before return. Desired
AssetReferences must describe the actual replacement bytes. Root Name edits need
explicitly coherent ObjectTypeName and qualified child references; inconsistent
edits refuse. Physical file/declaration add/delete/rename and type changes needing
a new body path remain N5 topology work. No public disk writer is invoked here.

N2 embedded children, N3 closed reference context, N4 external Form native writer,
N5 full body compilation/topology, N6 main builder, N7 services, N8 public artifact
publication, N9 end-to-end unrestricted resource policy and N10 genuine platform
acceptance remain open. The inherited ReaderLimits/SourceTree/root compatibility
quotas have not been removed in N1. No new arbitrary quotas are introduced; the
new canonical named-object projection uses existing Source accounting policy.

The new `bootstrap_external_named_owners` target is independently authored MIT
cleanroom source. Foreign native captures are read-only hash-bound evidence on F,
not copied product fixtures. Test methods are prepared source, not runtime proof.
