# Form, shared-value and reference applicability

Refs #570. Format source: [tool1cd pinned revision](https://github.com/jjzhang166/tool1cd/tree/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1). The public Coverage inputs come from integration revision `718aef51`; decoded CF hashes are pinned in [form-native-inputs.json](form-native-inputs.json). No native payload, application source, copied implementation or private path is published here.

[form-map.json](form-map.json) accounts for **81 audit signatures** in this scope, including source-only legacy probes. The other 23 belong to the separately assessed template/MXL/help/flowchart scope. This is an applicability table, not a claim that every code candidate is a measured repair. Final Coverage/source-audit results decide closure.

| Disposition | Signature count |
| --- | --- |
| additional-index-owner-gap | 1 |
| already-fixed | 1 |
| candidate-needs-dual-slot-proof | 1 |
| color-context-needs-current-retest | 2 |
| command-domain-gap | 7 |
| configuration-profile-gap | 2 |
| control-layout-needs-native-proof | 1 |
| event-owner-gap | 4 |
| existing-code-needs-current-retest | 1 |
| generated-command-context-gap | 3 |
| implemented-native-extension | 3 |
| implemented-native-identities | 9 |
| implemented-tool1cd-and-native | 16 |
| legacy-generated-panel-gap | 1 |
| modern-font-name-gap | 3 |
| palette-version-gap | 3 |
| profile-context-gap | 1 |
| reference-context-gap | 5 |
| runtime-name-not-storage | 2 |
| value-profile-gap | 1 |
| version-profile-gap | 10 |
| widget-profile-gap | 4 |

## Exact old facts, independently checked against native Coverage

- [WebColor domain](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L1062) supplies Linen=82, MediumGreen=88 and Tomato=138. Native `corpus-style-colors` stores `{3,2,{n}}` on 8.3.27.2214 and `{4,2,{n},2}` on 8.5.1.1150. The existing dialect transformation is retained; only the independently confirmed values enter the shared encoder table. This also reaches label/group colors through their existing shared path.
- [Picture-reference grammar](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L53550) and the predefined identities at [L52890](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L52890), [L52965](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L52965), [L53005](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L53005) match `corpus-managed-forms` on both native platforms: QueryWizardTempTableDescription, GotoURL and AccountingRegister. One shared UUID table serves the readers and existing writer fallbacks. The older adapter UUID table is not duplicated or refreshed.
- [System picture GrayedAll=-12](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L42489) matches native 8.5.1.1529 `corpus-modern-details`: a one-member `{-12}` value in the existing picture tuple. The negative code retains priority over UUID lookup. A multi-member negative value is not admitted as this scalar profile.
- [Space key=32](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L10391) and [shortcut storage](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L62130) match both native SpaceForm controls: `{0,32,0}`. The existing modifier mask remains unchanged.
- [ProgressBarSmoothingMode BrokenTilt=2](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L40992) matches the native `corpus-hybrid-interface` progress-bar payload; the owning field keeps its existing layout.
- [Table SelectAll](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L43449) and [AddAutoSelectedField](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L43462) match their native Coverage contexts. SelectAll's existing plain-table mapping does not cover a dynamic-list table; the newly measured alias is keyed to that target. AddAutoSelectedField is admitted only when the owning attribute is declared `cfg:DynamicList` and the table binds exactly `<attribute>.SettingsComposer.Settings.Selection`. Missing paths, an extra path member and plain ValueTable owners are negative controls. The generic context-free command lookup refuses this command.

## Modern extensions: distinguish new measurements from old documentation

The [old form descriptor](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L59644) establishes the metadata header/help/type/purposes prefix. It does **not** describe the modern form record 14 append. Native 8.5.1.1529 `corpus-hybrid-interface` descriptors independently show the appended mode values Taxi=1 and Version85=2; Any=0 was already supported. Both directions use the same schema-owned enumeration. The old Configuration Taxi=3 and ClientApplicationInterfaceVariant Taxi=1 are different domains and were not reused as proof.

Native Coverage also confirms nine further standard-picture identities (NewConversation, CollaborationSystemExternalUser, Char, Today, AppearanceStarHalfFilled, FormattedString, AppearanceUpArrowGray, DontNotify, StartVideoconference), with repeated witnesses where the matrix supplies them. Their UUIDs are modern native facts; old tool1cd's picture grammar gives the surrounding representation, not the missing identities. DeselectAll on a ValueTable-bound Table is similarly a new native identity in the existing command-set grammar; it is absent from the old catalogue.

## Explicit remaining boundaries

The [managed-form grammar](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L62494) lists body versions 22–38 and predates current 8.5 profiles. A runtime property with the same name does not establish modern slot coordinates. Palette colors are not old web colors: [old ColorType](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L4224) has five categories and no current palette domain. TitleLevel1/TitleLevel4 likewise are not present in the old style-font identifiers. Independent public native 8.5.1.1529 records now confirm TitleLevel1=-48 and TitleLevel4=-51 in the existing kind-2 tuple. These identity facts explain I007/I032/I080, but are deliberately not admitted in this patch: the shared form font writer receives a version-7 intermediate after the 2.21 adapter down-converts the XML, with no source-profile context. No 8.3 native witness was found. A context-aware writer follow-up is required before enabling the identities, so an 8.5 measurement does not silently broaden 2.20 source admission. The tool1cd grammar remains useful; the remaining dependency is profile plumbing.

The old form-attribute/type-link grammar helps identify owner relationships, but it does not supply a universal rule for a generated type, a deep data path or sentinel `-2`. Standard-command names need their target namespace: the picture named Back is not the Planner command Back, and a static UUID is not an OpenList generated ordinal. Source-only legacy command/panel probes remain separate from native-built CF evidence.

`VerticalScroll=use` is a concrete follow-up candidate: tool1cd identifies Use=1, while the existing reader uses a qualifier/mode pair (2/2 for useIfNecessary; 0/3 for useWithoutStretch). This work does not guess 1/1 from a single enum name. Exact source, native profile and current rerun must decide that extension.

Tests added here use the `tool1cd_` prefix, compare complete picture/shortcut values and preserve unknown-name refusals. No test or baseline is weakened. Final measured improvements, full gates and the fresh-context review are recorded in the campaign evidence by the integrating author.

## Export clusters in this scope

The machine-readable map also records E03/E04/E05/E08/E09/E11/E12. In particular, old Resource versions 10/25/26/27 ([L59044](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L59044)) do not establish BinaryDataStorageLocationUse on current resources. Font versions 6/7 do not establish the cross-version TitleFont wrapper. The runtime TableBoxSelectedRows class is not the stored internal selected-rows group profile. SQL `_ExtensionZippedInfo`/`_ExtensionsInfo` framing was not found in the inspected MetaTree/MetaData files, and no SQL-envelope support is inferred from ordinary Configuration metadata.

E11 help canonicalization belongs to the template/help evidence. E12 retains the difference between per-file field observations, complete owner/type semantics and missing output after an earlier export abort. These boundaries are unresolved research items, not permanent product limitations.
