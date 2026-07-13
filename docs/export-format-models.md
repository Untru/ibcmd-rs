# Export Format Data Models

This file records cross-cutting export-format models that are required for
export parity work. It is separate from progress logs: entries here should say
which parts are proven by native export comparison and which parts are still a
working hypothesis.

Status terms:

- `confirmed by export`: observed in native export output and matched by the
  current parity work.
- `supported by tests`: covered by focused unit/model tests, but still worth
  rechecking on a wider native export set.
- `hypothesis`: inferred from raw shape or current implementation behavior and
  not yet proven on enough native exports.

## DCS template `settings` / `settingsVariant`

Status: settings binding, QName scope allocation, area side tables, and Color
reference resolution are `confirmed by export`.

Model:

- A `DataCompositionSchema` template body can contain more than one embedded XML
  document after inflation. The schema document is the `SchemaFile` wrapper that
  contains `dataCompositionSchema`; settings documents are separate `Settings`
  XML documents in the DCS settings namespace.
- Native source export is a single `DataCompositionSchema` XML document, not the
  raw `SchemaFile` container. The `SchemaFile` and inner
  `dataCompositionSchema` wrappers are structural storage wrappers and are not
  emitted.
- A template may contain multiple `SchemaFile` documents. Direct children of a
  structurally standalone additional `dataCompositionSchema` are inserted,
  in source order, immediately before the first direct `settingsVariant`.
  Self-closing additional schema roots are empty sentinels and are no-ops.
- Additional documents are admitted only when `SchemaFile` has no other element
  children and the complete document has no area-template namespace. Documents
  with storage side tables remain fail-closed until their references are
  resolved; no raw wrapper, side-table node, or unresolved index is emitted.
- Each embedded `Settings` document is normalized as a `dcsset:settings` block.
  External settings pair by ordinal only with direct `settingsVariant` children
  of the root schema. Nested variants are already inside the schema document and
  do not consume external settings. A count mismatch is rejected; settings are
  not appended as root siblings.
- Existing variant metadata remains inside the same `settingsVariant`. For
  example, variant `name` and `presentation` are normalized into the DCS settings
  namespace and the attached settings payload is added after them as
  `dcsset:settings`.
- DCS settings payload namespaces are canonicalized during insertion. The
  settings namespace uses `dcsset`, DCS core uses `dcscor`, DCS common uses
  `dcscom`, and data core uses `v8`. Unqualified `xsi:type` values from the
  settings namespace are emitted as `dcsset:*`; DCS core values are emitted as
  `dcscor:*`.
- DCS schema-side type references are normalized in the same pass: known
  metadata `TypeId` values use a typed resolution policy. `DefinedType` keeps
  `v8:TypeId`, ordinary generated types become current-config `v8:Type`, and
  `Characteristic` plus generic reference-family roots become
  current-config `v8:TypeSet`. Unknown TypeIds stay unchanged; AnyIBRef keeps
  its separate `v8:TypeSet` rule.
- Lexical QNames are resolved through the input namespace stack into
  `{namespace URI, local}` and then serialized for the output scope. Raw `dNp1`
  prefix text is never copied. Dynamic current-config and enterprise prefixes
  use their structural base plus `2 * nestedSchemaDepth`.
- Reparenting a standalone storage `Settings` document into a
  `settingsVariant` adds one output namespace layer. For an otherwise unknown
  generated namespace, an exact input prefix `dNp1` therefore becomes
  `d(N+2)p1`; Schema mode, vendor prefixes, and other suffix families such as
  `dNp2` do not shift. The writer records URI-to-output-prefix bindings in each
  emitted element scope and applies them consistently to element start/end
  names, qualified attribute names, and `xsi:type`. Prefix conflicts and
  unbound qualified names fail closed.
- A settings payload root under a direct `settingsVariant`, or schema settings
  under `nestedSchema`, always declares `style`, `sys`, `web`, and `win`, even
  when empty. An inner `dcsset:settings` under an item is not a contract root.

Confirmed by export:

- The source XML shape is `DataCompositionSchema` with `settingsVariant`, not a
  top-level `SchemaFile`.
- A raw embedded `Settings` document belongs inside a `settingsVariant` as
  `dcsset:settings`, not as a sibling of `settingsVariant`.
- The namespace/prefix rewrite above is required for byte-level parity on DCS
  template exports.

Supported by tests:

- Multiple embedded XML documents are split from the inflated template body.
- `Settings` documents are detected by the DCS settings namespace and inserted
  into variants in input order.
- Settings payload nodes such as selected items, filters, and output parameters
  are rewritten to canonical DCS prefixes.

Corpus evidence:

- 661 raw SchemaFile records contained 1,050 external Settings documents and
  exactly 1,050 direct root variants, with zero count mismatches; 152 schemas
  were multi-variant and seven also contained nested variants.
- Native binding matched variant counts, names, and settings in all 69 DCS
  templates; 23 were multi-variant.
- QName evidence covered 29 StandardPeriod text QNames, 25 enterprise
  LinkedValueChangeMode QNames, 145 settings contract roots, and 15 nested
  current-config prefixes. The accepted gate moved the full diff from
  `1642 files, +32149/-223052` to `1629 files, +32044/-222947`; all four scoped
  classes reached zero without changing settingsVariant/name/presentation.
- Typed TypeId evidence covered 19 DefinedType `Type -> TypeId` pairs, 14
  generic-root `TypeId -> TypeSet` pairs, and two Characteristic
  `Type -> TypeSet` pairs. The negative corpus contained 98 unchanged `Type`
  and eight unchanged AnyIBRef `TypeSet` rows. The accepted gate removed all 35
  pairs and moved the then-current full diff from
  `1616 files, +31526/-221470` to `1610 files, +31491/-221435`.
- The eight generic reference-family TypeIds are platform protocol ids. All
  eight occur in an independent `buh` raw Config sample with counts
  `4,2,14,2,2,2,2,39`; none is a Config FileName, and three already existed in
  the platform builtin type registry. They map only to family QNames, never to
  an object name.
- The BSP multi-document matrix has 21 remaining DCS rows: five empty second
  schemas, one standalone nonempty schema, and 15 documents using the
  area-template namespace. The standalone document reached byte parity with
  SHA-256
  `EB4B9AFBC2DEE1FCAA987995FE353A472AEE27A223AF290D772C00564F2DA7A5`.
  The accepted narrow gate moved the full diff from
  `1603 files, +31486/-204757` to `1602 files, +31486/-204748` and the 66-file
  DCS body slice from `21 files, +157/-10975` to
  `20 files, +157/-10966`. All 66 generated documents parsed successfully;
  root storage `appearance`, unresolved `appIndex`, and inner storage-wrapper
  counts were zero. TypeId, TypeSet, settingsVariant, settings name, and Form
  command guardrails did not change.
- The area-template storage model is implemented for structurally complete
  side-table documents. Each `dcsat:appIndex` value is a zero-based index into
  the ordered outer `SchemaFile/appearance` side table. Native replaces the
  index in place with
  `dcsat:appearance` containing a deep copy of the indexed wrapper's children;
  it drops the side-table wrapper attributes and emits no side-table nodes at
  the schema root. Repeated indexes duplicate the body. In the independent UT
  corpus, all 1,099 references across 49 side-table documents were in range and
  every table's referenced unique indexes covered `0..N-1`. The resolver
  requires that exact coverage, validates the table-cell/index shape, applies
  byte offsets in reverse order, and rejects unknown or partial envelopes.
- The BSP area gate covered 14 side-table documents with 165 entries and 458
  references. Twelve were admitted; two with 13 referenced unresolved color
  UUIDs stayed fail-closed. The full diff moved from
  `1602 files, +31486/-204748` to `1599 files, +31470/-201297`, and the DCS body
  slice moved from `20 files, +157/-10966` to
  `17 files, +141/-7515`. All 66 DCS documents parse; 150 inline area
  appearances were emitted, while root storage appearances, `appIndex`, and
  inner storage wrappers remained zero. Existing unresolved direct color UUID
  additions remained exactly 124, so the area merge introduced none.
- A serialized Color value shaped exactly as `0:<UUID>` is a reference to a
  `StyleItem` object in the current configuration, not a platform identifier.
  BSP contains 14 unique referenced StyleItems and independent UT contains nine;
  every observed UUID resolves through metadata and none is a platform constant.
  Resolution uses the metadata-derived `object_refs` index and accepts only a
  nonempty `StyleItem.*` reference. There is no UUID-to-name table. Direct schema
  and settings values become `style:<name>`. Values copied from an area side
  table are resolved by namespace URI and serialized with a local structural
  `d8p1` declaration; this also repairs the lost scope of already-qualified
  style/web values. Direct literals and already-qualified values remain
  unchanged. Unknown or non-StyleItem UUIDs do not use a fallback; dependent
  additional area documents stay fail-closed.
- The accepted Color gate moved the full diff from
  `1599 files, +31470/-201297` to `1592 files, +31369/-193848`, entirely inside
  the DCS slice. That slice moved from `17 files, +141/-7515` to
  `10 files, +40/-66`. All 66 documents parse; both large side-table documents
  have native-equal appearance and Color counts. Raw Color UUIDs, unscoped
  `d4p2` Color QNames, root storage appearances, `appIndex`, and inner storage
  wrappers are all zero. TypeId, TypeSet, settingsVariant, settings name, Form,
  ConfigDumpInfo, and configuration Help guardrails did not regress. Production
  additions contain no database UUID, object name, or local path literals.
- A self-contained additional area document is a separate envelope from the
  side-table form. It has one nonempty direct `dataCompositionSchema` child,
  no sibling elements, no root storage `appearance`, and no `appIndex`.
  Admission requires an actual expanded area-template element or `xsi:type`;
  an unused namespace declaration is insufficient. Serialized Color references
  remain subject to the same dynamic `StyleItem.*` rule. Parser, QName, and
  attribute errors propagate; only an explicit unsupported-envelope result is
  skipped. This class occurred once in BSP and three times in the independent
  UT corpus. The accepted gate made the BSP `Reports/Задания` template exact,
  moving the full diff from `1592 files, +31369/-193848` to
  `1591 files, +31369/-193822` and the DCS slice from
  `10 files, +40/-66` to `9 files, +40/-40`. All DCS and cross-group guards
  remained unchanged.
- A DCS core `value` whose expanded `xsi:type` is the data-UI `Picture` type
  canonicalizes its `ref` as `v8ui:<local>` only when the reference QName also
  resolves to the data-UI namespace. The input prefix text is irrelevant.
  Bare references, references in another namespace, other value types, and
  other elements remain unchanged. In the accepted gate, all 30 residual
  Picture pairs disappeared exactly: the full diff moved from
  `1569 files, +31338/-192968` to `1565 files, +31308/-192938`, and the 66-file
  DCS slice moved from `9 files, +40/-40` to `5 files, +10/-10`. All 66 DCS
  documents parse, no dynamic Picture prefix remains, and Form, ConfigDumpInfo,
  and Help guardrails stayed unchanged.
- The standalone Settings namespace-layer rule was confirmed by three
  `ChartSplineMode` values in one template. Their chart namespace changed from
  storage `d6p1` to native output `d8p1`; no chart/report/path condition exists
  in the implementation. The exact old/new tree delta was one path and
  `+3/-3`. The full diff moved from
  `1560 files, +31287/-190974` to
  `1559 files, +31284/-190971`, while the DCS residual moved from
  `5 files, +10/-10` to `4 files, +7/-7`. All 69 BSP DCS template bodies parse;
  the remaining seven pairs are the deliberately unresolved mixed-valueType
  order described below.

Rejected hypothesis:

- Omitting `style` / `sys` / `web` / `win` namespace declarations on empty
  inserted `dcsset:settings` roots was tested and rejected. It increased the
  full diff from `1917` to `1924` files while the target DCS file still had
  `58 insertions` / `58 deletions`. The residual also contains enterprise
  namespace prefix shifts and `v8:StandardPeriod` differences. The accepted
  expanded-QName/output-scope model above supersedes this rejected shortcut.
- Flattening every additional `SchemaFile` was rejected. The first gate
  regressed the full diff from 1,603 to 1,650 files because an empty
  `dataCompositionSchema` event and storage-side area-template appearances were
  serialized literally. Restricting output to the inner schema fixed the
  wrapper leak but was still incomplete because unresolved `appIndex` values
  require the side-table substitution model above. Both failures were reported
  and rolled into the final fail-closed admission rule.
- Reordering mixed `valueType` entries by QName, source order, or an inferred
  UUID threshold is not supported. In BSP, 19 of 726 schema `valueType` nodes
  mix symbolic `xs:string` with a metadata `TypeId`; seven have an order-only
  residual and 12 are native-exact negative controls. In `ut_ibcmd`, the same
  `CatalogRef.Пользователи` QName sorts on the opposite side of `xs:string`
  because its dynamic TypeId differs. Raw data stores primitives symbolically
  (`{"S"}`) and exposes no platform order key for `xs:string`; no builtin type
  registry was found in Config. A production change therefore requires an
  authoritative primitive order key/comparator. Until then, the seven residuals
  remain deliberately unchanged rather than introducing a corpus-derived
  threshold hardcode.

## Metadata generated type families

Status: ExchangePlan raw codes `36`/`37` and ChartOfCalculationTypes raw code
`35` are `confirmed by export`.

Model:

- Generated type ids are read from a metadata family's structural raw slots and
  paired with the parsed metadata header name. They are not database object
  UUID constants in production code.
- ExchangePlan raw codes `36` and `37` use the same five TypeId slots at
  indexes `1,3,5,7,9` of the generated-type payload. In order, they represent
  `ExchangePlanObject`, `ExchangePlanRef`, `ExchangePlanSelection`,
  `ExchangePlanList`, and `ExchangePlanManager`.
- A slot is admitted only when the enclosing metadata row/header is parsed and
  the slot contains a valid UUID. Unknown or malformed records remain
  fail-closed; there is no object-name fallback.
- ChartOfCalculationTypes code `35` has its metadata header at index `1` and
  requires all 22 UUID fields at indexes `2..23`. The TypeIds occupy the even
  indexes `2..22` and map, in order, to Object, Ref, Selection, List, Manager,
  DisplacingCalculationTypes and Row, BaseCalculationTypes and Row, and
  LeadingCalculationTypes and Row. Requiring the complete UUID range prevents
  partial records from matching this family.

Confirmed by export:

- A selected code-36 gate restored five previously absent Constant/DefinedType
  consumer XML files byte-for-byte. The full gate had a wider, valid downstream
  effect because the same type index feeds metadata properties, command
  parameters, subscriptions, and form types.
- An isolated old-versus-new full export proved the complete delta. The full
  diff moved from `1591 files, +31369/-193819` to
  `1575 files, +31350/-193392`. Exactly 25 paths changed: five direct metadata,
  nine CommonCommands, two EventSubscriptions, two Catalogs, four ExchangePlan
  forms, and three InformationRegisters. Sixteen became exact.
- Every one of the 427 new generated lines exists in native output; all 19
  removed generated lines were non-native empty or partial blocks. The restored
  typed values were 34 `ExchangePlanRef` and five `ExchangePlanObject` QNames.
  No path worsened, and production additions contained no database UUID, object
  name, or local path literal.
- The ChartOfCalculationTypes gate was also verified with an isolated pre-gate
  full export. The full diff moved from `1575 files, +31350/-193392` to
  `1569 files, +31338/-192968`. Only seven paths changed: two DefinedTypes and
  four EventSubscriptions became exact, and one form gained its native
  three-line main-attribute type block. All 424 inserted old-to-new lines are
  native; the 12 removals were non-native partial subscription shells. The
  newly resolved values were six `ChartOfCalculationTypesObject` and one
  `ChartOfCalculationTypesRef` QName.

## Form.xml `CommandName` / `CommandSource`

Status: draft model. Several common paths are `confirmed by export`; the full
item-type command-source matrix remains a `hypothesis`.

Model:

- `CommandName` names the command executed by a form item, usually a `Button`.
  It is resolved from the native command-reference tuple and supporting command
  tables, not from the visible item name alone.
- For every nonzero command-reference kind, local form commands are resolved
  first by the exact pair `(command.id, command.reference_uuid)` and emitted as
  `Form.Command.<name>`. This precedence also applies to ids `10` and `21`;
  table-standard resolution is only a fallback after the exact local lookup.
- Form standard command UUIDs are emitted as `Form.StandardCommand.*` only for
  mappings that are confirmed by export or explicitly covered by focused tests.
  Do not promote UUIDs from nearby excluded-command tables into
  `Button/CommandName` mappings without export evidence.
- Object or common command references are emitted as object/common command names
  when the referenced owner metadata is available. If the object reference is a
  standard object command, the emitted name includes the standard-command suffix.
- A table-standard Button reference has raw shape
  `{<table-item-id>,<command-uuid>}`. The first tuple field is resolved directly
  through the form table item index and emitted as
  `Form.Item.<table>.StandardCommand.<suffix>`. Button-name, ancestry, and
  single-table fallbacks are not part of the model.
- Unknown command-reference tuples should be omitted rather than guessed.

`CommandName` standard-command UUID model:

| Raw precondition | UUID | Emitted `CommandName` | Status | Evidence |
| --- | --- | --- | --- | --- |
| `Button/CommandName` standard form command reference | `679b62d9-ff72-4329-bf3a-c0c32b311dd2` | `Form.StandardCommand.Cancel` | confirmed by export | Narrow command-name subfix; final release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `f3613d5c-20c6-46e5-b4d5-7d712ece1296` | `Form.StandardCommand.OK` | confirmed by export | Narrow command-name subfix; final release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `342c531d-dc73-458a-8ac4-6a746916a33b` | `Form.StandardCommand.Copy` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `96e0bc70-f8ff-4732-8119-060923203629` | `Form.StandardCommand.CancelSearch` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `9758d344-4b1d-4dc9-80bd-81060bc18b2a` | `Form.StandardCommand.OutputList` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `3a17e914-ec6a-4280-b4df-78914f40522b` | `Form.StandardCommand.ShowInList` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `1f317795-c420-4a30-b594-c492abc55f7a` | `Form.StandardCommand.Reread` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `1c00edb8-a826-4855-9bde-94dbc5f620e5` | `Form.StandardCommand.ListSettings` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `87317f86-057f-477e-9045-2da4e4980199` | `Form.StandardCommand.PostAndClose` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `952c2984-9955-415a-8235-5c710aabe732` | `Form.StandardCommand.LoadDynamicListSettings` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `d5c3842d-7252-4370-9174-756a6cc553e5` | `Form.StandardCommand.SaveDynamicListSettings` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `d603a249-6eb3-4e38-bb2d-a8a86a8ab156` | `Form.StandardCommand.DynamicListStandardSettings` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `d8772fd1-a3bf-417d-8334-c49968dbb45e` | `Form.StandardCommand.CreateFolder` | confirmed by export | Raw evidence as `kind == 0`; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `6886601d-276c-4d3f-af0a-05c586025608` | `Form.StandardCommand.Change` | confirmed by export | Raw evidence 21 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `8e2b82cf-d1ea-46b2-afdf-a8d64e66ea2b` | `Form.StandardCommand.Choose` | confirmed by export | Raw evidence 14 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `bdefa701-6685-453e-a02a-3683d0cc16d3` | `Form.StandardCommand.Find` | confirmed by export | Raw evidence 12 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `3b8cedbc-8e74-4017-b901-d14b09f32f7a` | `Form.StandardCommand.Post` | confirmed by export | Raw evidence 11 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `2e86453d-8958-4c9a-a1b4-b15215eedc2e` | `Form.StandardCommand.SetDeletionMark` | confirmed by export | Raw evidence 6 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `827b541d-30c1-4f06-aecf-92aa496a0835` | `Form.StandardCommand.SetDeletionMark` | confirmed by export | Raw evidence 6 examples; release/export had `NO_ADDED_COMMANDNAME` |

The next confirmed form-level batch has the same raw precondition:
`Button.fields[8] == {0,<uuid>}`. A global scan of 987 raw form bodies found
exactly 43 occurrences of these UUIDs; all 43 matched native `CommandName`,
with no omitted or different native values.

| UUID | Standard-command suffix |
| --- | --- |
| `fd8f031f-c168-4e1b-8b0c-15eb3057e688` | `Refresh` |
| `c32d43de-b820-49d0-bf7a-d70829f48f40` | `Delete` |
| `3dd3bd8a-ac1e-44d6-ac83-e7802642a5e2` | `Delete` |
| `1cc781aa-f32b-4dc7-996a-6c38c3deda5c` | `Delete` |
| `8d7bcd38-1bbb-4dc1-a9ad-cc9d5966ca8e` | `Start` |
| `e6a9041f-4d43-4f06-8e17-e95753531565` | `StartAndClose` |
| `389ef1f1-97ce-4326-adf5-886b2dead75c` | `UndoPosting` |
| `b520ca45-d8db-4982-b128-bb42a6afd911` | `FindByCurrentValue` |
| `c9abb6b0-eafd-4505-8312-9a7b6888cbf3` | `ChangeHistory` |
| `a2b927a1-35af-43e3-af73-4af22ac2c0fa` | `List` |
| `ffc5e8d5-40a7-4893-a590-49bd588f9466` | `HierarchicalList` |
| `0b83270d-7f95-4cdd-93c3-342d7991fed5` | `Tree` |
| `39c6a2fb-45cc-41b1-853f-967fb68aa1df` | `MoveItem` |
| `eb880cb2-a91f-4ad6-afb7-f0e6d7a1b111` | `SetDateInterval` |
| `62778a6d-6114-471c-93f7-e1ccd54bd266` | `CreateInitialImage` |
| `b08b7a35-583a-4756-b814-0436ff9139c0` | `LoadVariant` |
| `0fb774df-ec1c-4e23-9ed1-e089974f74bf` | `ReportSettings` |
| `5d41082e-9619-42ec-b96f-98b082b3a2f0` | `Yes` |
| `06ee6a21-061e-47f8-81c5-92ae8b8f3b5d` | `No` |
| `68baa1bc-edd1-4d9b-ad80-1d53fb8a7988` | `Copy` |

Confirmed table-standard UUID additions under the strict table-item-id owner
model are:

| UUID | Table standard-command suffix |
| --- | --- |
| `0ae4bea5-23be-42a7-b69e-97b11b29c453` | `Copy` |
| `825c1c15-ef8f-47ab-b002-e6b84b3e5b10` | `OutputList` |
| `88078230-1f6b-415f-99e4-ad2ff73810cf` | `CopyToClipboard` |
| `8969c93a-23e5-4bef-941d-aaef315858d2` | `Choose` |
| `a2f737a8-0114-4e86-a214-45e5c213fa65` | `SetDeletionMark` |
| `b0016a68-ec64-4e6d-b905-c71fd62efc4c` | `Add` |
| `b41f5bbc-ba5d-4888-8cd1-db246a371418` | `Change` |
| `e7216412-03ac-4a81-99c2-1d7c28e88e31` | `ShowMultipleSelection` |

The second table batch was correlated across 379 forms and 647 raw Button
rows. The owner was accepted only when the tuple kind resolved to a raw wrapper
`55` Table, and every mapping had one native suffix with zero omitted,
different, or missing native Button cases.

| UUID | Table standard-command suffix |
| --- | --- |
| `01833a5a-6553-4c49-b445-095018107bb5` | `HierarchicalList` |
| `05468165-f954-45a5-84f2-6641c51f9f23` | `Tree` |
| `0d0249a4-2b2f-4fc0-a66f-b36f9494b3cc` | `List` |
| `0e9b637d-cf6e-4330-8a8f-cd44842e34bb` | `LevelUp` |
| `0f8d6d98-2f8b-405a-b8b3-0538e9d95da5` | `Create` |
| `14559f7c-853c-42a4-9ea1-01546107747b` | `ListSettings` |
| `18248aa8-e621-4e19-a611-54fb8923644c` | `CheckAll` |
| `182a793b-22a5-4625-b316-6a5be7f88078` | `LoadDynamicListSettings` |
| `1f1e900a-8488-4159-81be-9704eb96906d` | `UserSettingItemProperties` |
| `27bd521a-51c6-4fe7-846d-a98f988774b5` | `MoveItem` |
| `33b7b9cd-6979-4435-8c58-d9bc8250edec` | `DynamicListStandardSettings` |
| `403bc6e6-b98e-4181-9f43-9c75cbbf82cf` | `Refresh` |
| `4a817da0-5797-4e16-906f-02fb869e1873` | `GroupFilterItems` |
| `51c99108-107c-43e1-8918-e48835bf2495` | `SelectAll` |
| `714d44cc-63da-4431-b33a-428e398d2a08` | `FindByCurrentValue` |
| `7b683784-b474-441a-ba63-3d757bd0ffd4` | `SearchEverywhere` |
| `82b88a24-2856-484a-afd9-55a15bdf9785` | `Ungroup` |
| `95b4bc12-2ece-4d7a-b3e2-6f9293620a06` | `SaveDynamicListSettings` |
| `9ef79140-3de6-436a-8dda-610bb963f5db` | `EndEdit` |
| `a5fdef31-bbf0-4a9d-98aa-fd5fd8f1344a` | `AddFilterItemGroup` |
| `d7e55d2e-bfea-4d80-b4ad-a1bb31ec2147` | `UseFieldAsValue` |
| `d82ca05c-2966-4d77-9a39-a1eea087bfa7` | `CreateFolder` |
| `daa306cd-a78a-4e74-a14c-739daba624cb` | `SetDateInterval` |
| `dc118d99-b351-4e30-9310-e864f2e53ec0` | `LevelDown` |
| `fca750bc-4fb6-40e2-ae0f-e818939a32e7` | `AddFilterItem` |

Rejected model:

- Excluded-command tables are not equivalent to `Button/CommandName` standard
  mappings. A wider reuse of excluded-command UUID tables was tried earlier and
  rejected because it produced `CommandName +114`.
- Form standard command candidates remain unconfirmed unless listed in the
  table above or proven by a separate narrow export. Do not infer additional
  UUIDs from excluded-command tables.

Shifted extended-Button layout:

- Six raw Button records in the current 987-form corpus contain one service
  head slot. Their usual name/title/command and extended-property slots are all
  shifted by `+1`.
- The offset is derived structurally: a quoted non-empty value in slot `5`
  means offset `0`; otherwise the Button uses offset `1`. The same offset must
  be applied consistently to every Button field, not only `CommandName`.
- Native contained all six shifted Buttons and the pre-fix export contained
  none. The accepted export restored all six without adding target tags or
  increasing full-diff insertions.

Additional confirmed `CommandName` records:

- A one-field Button command record `{0}` emits literal `<CommandName>0</CommandName>`.
- After exact local-command lookup, typed kind `100` resolves only a known
  `CommonForm.*` reference and emits its standard `Open` command. Typed kind
  `4` resolves only a known `Catalog.*` reference and emits
  `StandardCommand.OpenByValue`. If the object reference is absent or has a
  different metadata kind, resolution continues to the table fallback instead
  of returning early.
- AutoCommandBar child Buttons need the same table-name index as ordinary form
  children. The index must be collected before AutoCommandBar parsing; empty
  maps silently suppress otherwise known table-standard commands.
- Owner indexes are purpose-specific. `table_name_by_id` is used for Table
  data paths; standard `CommandName` resolution uses Tables plus only proven
  command-capable field families; `CommandSource` uses its own source-owner
  index. A generic id-to-any-item-name map is not a valid substitute.
- A wrapper `37` / `48` record whose layout discriminator is `6` is a
  SpreadsheetDocument field owner. It may own standard commands even when that
  field subtree is not yet emitted as a form item. The following owner-scoped
  platform command UUIDs are proven for reachable Buttons:

| UUID | Standard-command suffix |
| --- | --- |
| `d8e20c4d-3519-49aa-80e5-d6d66fee741a` | `Save` |
| `d673d512-f71a-48a6-ae5d-527a64ffd813` | `Print` |
| `5aa38159-2001-42ae-8451-f8cabe0762c3` | `Preview` |
| `12acffde-8389-4e5e-bd86-ff248262d84a` | `ExpandAllGroups` |
| `ff5c34f8-b172-4ef2-91d3-48283a66a725` | `CollapseAllGroups` |
| `1ba33890-92e9-42a3-95bd-a5c783f46d55` | `CopyToClipboard` |
| `edf14e37-e755-4d1c-970c-48ed776e3a0e` | `PasteFromClipboard` |
| `ff533ae0-46a9-4e1d-aa3a-6dffa27e076b` | `SearchEverywhere` |
| `7eae9c22-db31-4f27-a56a-b4dd62d21a2c` | `ClearContent` |
| `59e67a77-8141-42cf-b062-7cb92e210b6d` | `ClearAll` |
| `ed6630f2-c296-43dd-b408-d370513fcebc` | `InsertComment` |
| `be8800c3-8ccf-444a-bbf0-8f3078ff0ded` | `Properties` |

- A wrapper `37` record with discriminator `14` is a GraphicalSchema owner.
  Its command IDs are distinct from SpreadsheetDocument and Table commands:

| UUID | GraphicalSchema suffix |
| --- | --- |
| `e2d6f793-b786-4640-a91b-8d77f73860f1` | `Print` |
| `1d13f9a3-402a-46cb-9c68-1709356840f2` | `Preview` |
| `01db2225-b62d-4112-a4b6-d39d627bf79f` | `PageSetup` |

- A wrapper `37` / `48` record with discriminator `17` is a distinct
  FormattedDocument owner. Standard-command UUID dispatch is owner-typed; these
  UUIDs must not leak into SpreadsheetDocument or Table resolution:

| UUID | FormattedDocument suffix |
| --- | --- |
| `39f6b9f1-7aa1-4a03-a01b-e127d51bc228` | `DecreaseIndent` |
| `4ca32834-6f9f-4dfb-89ce-6db36931c89b` | `Preview` |
| `56ae90b6-588f-406e-919c-cc5cc7f86297` | `AlignJustify` |
| `5a331cec-bf93-4af5-8f51-80fd7118db47` | `SaveAs` |
| `7a294bdc-b86b-4b73-abc4-df9c811f61ef` | `CopyToClipboard` |
| `87ecfbdd-8e2b-4ba2-a315-0897020f382f` | `AlignLeft` |
| `9d8a3915-de52-4227-91cd-2fce22e09972` | `Picture` |
| `a8483976-8b13-416a-9680-133b306dc6b0` | `Print` |
| `ab0ebc39-68ee-4034-b2f4-43eee55bd651` | `AlignCenter` |
| `d0a4d953-115b-4059-a6cb-6e67f903a4f3` | `IncreaseIndent` |
| `e428af27-c4f7-4577-b80e-95a79f94322d` | `AlignRight` |
| `b67f202a-dcf8-41f3-bda8-1ff9bed5f2ef` | `SelectAll` |

The owner-scoped gate changed exactly five Form.xml files by adding 23 native
`CommandName` lines and no other content. Eleven were in object-owned forms and
12 in two CommonForms that the former `**/Forms/**` guard path did not count.
The full diff improved from `1560 files, +31287/-190997` to
`1560 files, +31287/-190974`; that delta remains accepted.

A later independent guard over both object Forms and CommonForms disproved the
earlier closure claim and found `CommandName +0/-80` plus
`CommandSource +0/-1`. The accepted follow-up resolves all 81 rows through
typed owners: 69 SpreadsheetDocument commands, eight form-standard commands,
three `Table.StandardCommand.Pickup` rows, and one Popup item source. The broad
SpreadsheetDocument-to-Table fallback was removed; each owner family now has
an explicit platform-command allowlist.

The final gate changed exactly seven CommonForms by adding 81 native lines and
no other content. All 1,108 exported `Form.xml` files parse, and the complete
native/generated guard is now `CommandName extra/missing 0/0` and
`CommandSource extra/missing 0/0`. The full diff moved from
`1559 files, +31284/-187408` to `1559 files, +31284/-187327`. The seven shared
files are SHA-256 identical to an independent private export. A full
`ut_ibcmd` matrix matched 181/181 raw Button tuples and 5/5 Popup sources; none
of the added command/type UUIDs is a metadata-object UUID in BSP or UT.

`CommandSource` model:

- `CommandSource` describes which command set a command container reads from; it
  is not the same value as `CommandName`.
- Once a typed source id has been accepted, source id `0` maps to `Form`, source
  id `-1` maps to `FormCommandPanelGlobalCommands`, and other source ids map
  through the form item id table to `Item.<item-name>`.
- For `ButtonGroup`, the source tuple must carry both the source item id and the
  managed-form item type UUID `02023637-7868-4a5f-8576-835a76e0c9ba`.
  The outer source record must contain exactly four fields and its nested typed
  reference exactly two; trailing fields are not forward-compatible extensions
  of this proven shape.
  The proven shape is:

```text
{2,{<source-id>,02023637-7868-4a5f-8576-835a76e0c9ba},2,0}
  -> <CommandSource>...</CommandSource>
```

- A bare form marker is not enough for `ButtonGroup`:

```text
{2,{0},2,0}
  -> no <CommandSource>
```

- A `ButtonGroup` global-command source uses a distinct protocol type marker,
  not a metadata-object UUID. Only the full exact shape is accepted:

```text
{2,{0,2ef6d6fa-847a-485e-8684-d37a3ab5efb8},2,0}
  -> <CommandSource>FormCommandPanelGlobalCommands</CommandSource>
```

An independent raw dump of the `buh` infobase found this same marker in 35
different form blobs among the first 4,096 inflated Config rows. The same
independent slice also contained the standard-command UUIDs currently mapped
to `Save`, `Print`, and `Preview`. This rules out those values being metadata
object UUIDs specific to the BSP infobase; native XML parity on a second full
tree is still the stronger remaining portability check.

- For `CommandBar`, raw `fields[20]` is the source record. The safe export rule
  accepts only this shape:

```text
fields[20] = {1,<ignored>,{<source-id>,02023637-7868-4a5f-8576-835a76e0c9ba}}
  -> <CommandSource>...</CommandSource>
```

- The CommandBar source must have exactly three fields, `source[0] == 1`, and
  `source[2]` must be the two-field typed managed-form reference. `source[1]`
  and `fields[21]` are not source ids or a whitelist of current-corpus modes.
- Bare `CommandBar` records must not emit `CommandSource`:

```text
{1,0,{0}}
  -> no <CommandSource>

{1,2,{0}}
  -> no <CommandSource>
```

- Popup command source is a separate nine-field discriminator-7 record.
  Structural sentinels are `source[3] == 2`, `source[5] == 0`, and
  `source[6] == 0`; varying mode fields do not affect the source meaning. A
  typed `{id,02023637-7868-4a5f-8576-835a76e0c9ba}` reference resolves through
  the form item-id index (`0 -> Form`, otherwise `Item.<name>`). The distinct
  typed `{0,2ef6d6fa-847a-485e-8684-d37a3ab5efb8}` reference maps only to
  `FormCommandPanelGlobalCommands`. Unknown type, owner, length, or sentinel
  combinations fail closed.
- Native schema order is item-specific: CommandBar writes `CommandSource`
  after optional `ToolTip`; Popup writes `Picture`, `CommandSource`, then
  `Representation`. This order is required to avoid semantic matches appearing
  as added/removed target tags.
- Source owners are collected before AutoCommandBar parsing. The source-only
  index contains emitted form items and structurally named wrapper `37` / `48`
  field owners; adding those owners does not make unsupported fields emitted
  items and does not widen Table/DataPath resolution.

Confirmed by export:

- After the second CommandSource correction, the full export shortstat was
  `1929 files changed, 33829 insertions(+), 247785 deletions(-)`.
- The Form.xml metrics for that export were `CommandName +0/-1017` and
  `CommandSource +0/-181`; no added `CommandSource` entries remained.
- After the narrow `Cancel` / `OK` standard-command subfix, the final
  release/export shortstat was
  `1917 files changed, 33829 insertions(+), 247663 deletions(-)`.
- The Form.xml metrics for that final export were `CommandName +0/-925`,
  `CommandSource +0/-181`, with `NO_ADDED_COMMANDNAME`.
- After the six additional raw-confirmed form standard mappings, the
  release/export shortstat was
  `1917 files changed, 33829 insertions(+), 247593 deletions(-)`.
- The Form.xml metrics for that export were `CommandName +0/-855`,
  `CommandSource +0/-181`, with `NO_ADDED_COMMANDNAME` and
  `NO_ADDED_COMMANDSOURCE`.
- After the next form-level standard-command batch, the release/export shortstat
  was `1917 files changed, 33829 insertions(+), 247495 deletions(-)`.
- The Form.xml metrics for that export were `CommandName +0/-757`,
  `CommandSource +0/-181`, with `NO_ADDED_COMMANDNAME` and
  `NO_ADDED_COMMANDSOURCE`.
- After enabling exact local-command precedence for all nonzero ids, the
  release/export shortstat was
  `1917 files changed, 33829 insertions(+), 247373 deletions(-)`.
  `CommandName` improved from `+0/-757` to `+0/-659`; 98 local-command
  residuals disappeared without additions. Two remaining id `10` / `21`
  references live inside an entirely missing popup/context-menu subtree and
  therefore do not reach the resolver.
- After the 20-UUID form-level batch above, the release/export shortstat was
  `1917 files changed, 33829 insertions(+), 247327 deletions(-)`.
  `CommandName` improved from `+0/-659` to `+0/-616`, exactly matching the 43
  globally correlated raw references, without additions.
- After the strict table-item-id owner model and eight table UUID additions,
  the release/export shortstat was
  `1917 files changed, 33829 insertions(+), 246898 deletions(-)`.
  `CommandName` improved from `+0/-616` to `+0/-239`; `CommandSource` remained
  `+0/-181`, and neither target tag gained added lines.
- After the second 25-UUID table batch, the release/export shortstat was
  `1917 files changed, 33829 insertions(+), 246770 deletions(-)`.
  `CommandName` improved from `+0/-239` to `+0/-118`; the item residual fell
  from 191 to 70, with no added target tags.
- A provisional gate that admitted CommandBar mode `3` together with the exact
  ButtonGroup global marker produced
  `1917 files changed, 33829 insertions(+), 246731 deletions(-)`.
  `CommandSource` improved from `+0/-181` to `+0/-142`, exactly matching the
  predicted 22 mode-3 and 17 global-marker rows; `CommandSource +` stayed zero.
  The final parser below supersedes that observation and does not whitelist a
  mode value.
- After the shifted extended-Button offset model, the release/export shortstat
  was `1917 files changed, 33829 insertions(+), 246600 deletions(-)`.
  `CommandName` improved from `+0/-118` to `+0/-112`; all six shifted Buttons
  were restored, and the full insertion count remained unchanged.
- After the refined literal-zero and typed object-reference resolver, the
  release/export shortstat was
  `1917 files changed, 33829 insertions(+), 246579 deletions(-)`.
  `CommandName` improved from `+0/-112` to `+0/-92`; an earlier version that
  returned before table fallback was rejected after creating 13 new item
  misses.
- After replacing the rejected CommandBar mode whitelist with the structural
  typed-record model and item-specific XML order, the release/export shortstat
  was `1914 files changed, 33829 insertions(+), 246448 deletions(-)`.
  `CommandSource` improved from `+0/-142` to `+0/-17`, with no target additions.
- After passing the previously collected table index into AutoCommandBar, the
  release/export shortstat was
  `1914 files changed, 33829 insertions(+), 246351 deletions(-)`.
  `CommandName` improved from `+0/-92` to `+0/-55`, with no new mappings or
  target additions.
- After adding the dedicated source-owner index, the release/export shortstat
  was `1902 files changed, 32596 insertions(+), 230365 deletions(-)`.
  `CommandSource` improved from `+0/-17` to `+0/-0`; `CommandName` stayed
  `+0/-55` and both target addition counts stayed zero.
- The later exact-length ButtonGroup guard was an intentional no-delta gate.
  The full export stayed at `1591 files, +31369/-193822`, with
  `CommandName +0/-26` and `CommandSource +0/-0`. Before/after binary-patch and
  numstat fingerprints were identical, proving that the stricter parser did
  not exchange one current-corpus difference for another.
- After the narrow SpreadsheetDocument field-owner model and the five UUIDs
  above, the release/export shortstat was
  `1902 files changed, 32596 insertions(+), 230340 deletions(-)`.
  `CommandName` improved from `+0/-55` to `+0/-47`; the worker predicted seven
  reachable rows on its older branch, while the newer integration baseline
  restored eight. `CommandName +` and `CommandSource +` both remained zero.
- After typed FormattedDocument owner resolution and the eight UUIDs above, the
  release/export shortstat was
  `1686 files changed, 32376 insertions(+), 224137 deletions(-)`.
  `CommandName` improved from `+0/-47` to `+0/-39`; `CommandSource` remained
  `+0/-0` and no owner type shared another family's UUID dispatch.
- Adding the three independently confirmed FormattedDocument commands later
  moved the full diff from `1591 files, +31369/-193822` to
  `1591 files, +31369/-193819`. `CommandName` improved from `+0/-26` to
  `+0/-23`; `CommandSource` and every non-Form guard remained unchanged.
- Broadly admitting the full FormattedDocumentField subtree was rejected. It
  improved `CommandName` to `+0/-26` but increased full insertions from 32044
  to 32051. Exact rollback comparison isolated one false localized `Title`
  block and an order-only `VerticalScroll` line (seven added lines total); the
  accepted baseline `1629 files, +32044/-222947`, `CommandName +0/-39` was
  restored before further work. The follow-up native audit proved that
  `FormattedDocumentField` Title is slot 9 only, while slot 10 is ToolTip and
  is not InputHint. It also proved the root order `VerticalScroll`,
  `CommandSet`, `UseForFoldersAndItems` with matrices 20/20, 6/6, and 20/20
  respectively and zero reverse-order cases. The accepted structural gate
  moved the then-current full diff from `1616 files, +31553/-221996` to
  `1616 files, +31526/-221470`; `CommandName` improved from `+0/-39` to
  `+0/-26` and `CommandSource` remained `+0/-0`.
- `679b62d9-ff72-4329-bf3a-c0c32b311dd2` maps to
  `Form.StandardCommand.Cancel`.
- `f3613d5c-20c6-46e5-b4d5-7d712ece1296` maps to
  `Form.StandardCommand.OK`.
- `6886601d-276c-4d3f-af0a-05c586025608` maps to
  `Form.StandardCommand.Change`.
- `8e2b82cf-d1ea-46b2-afdf-a8d64e66ea2b` maps to
  `Form.StandardCommand.Choose`.
- `bdefa701-6685-453e-a02a-3683d0cc16d3` maps to
  `Form.StandardCommand.Find`.
- `3b8cedbc-8e74-4017-b901-d14b09f32f7a` maps to
  `Form.StandardCommand.Post`.
- `2e86453d-8958-4c9a-a1b4-b15215eedc2e` maps to
  `Form.StandardCommand.SetDeletionMark`.
- `827b541d-30c1-4f06-aecf-92aa496a0835` maps to
  `Form.StandardCommand.SetDeletionMark`.
- The second form-level batch maps:
  `Copy`, `CancelSearch`, `OutputList`, `ShowInList`, `Reread`,
  `ListSettings`, `PostAndClose`, `LoadDynamicListSettings`,
  `SaveDynamicListSettings`, `DynamicListStandardSettings`, and `CreateFolder`
  by the UUIDs listed in the table above.
- `Button/CommandName` is restored for exact local form commands, confirmed or
  test-supported form standard commands, external/object commands when owner
  metadata is available, and table commands whose raw tuple identifies a
  known table item id.
- `ButtonGroup` with bare `{2,{0},2,0}` must not emit `CommandSource`.
- `ButtonGroup` with the typed form source tuple emits
  `<CommandSource>Form</CommandSource>`.
- `CommandBar` must read the typed source id from `source[2]`, not from
  `source[1]`.
- `CommandBar` with bare `{1,0,{0}}` or `{1,2,{0}}` must not emit
  `CommandSource`.
- CommandBar typed-source recognition is structural and independent of
  `fields[21]`; no observed-mode whitelist is part of the model.
- Popup global source uses its own discriminator-7 typed record and item-specific
  XML order; it does not reuse the CommandBar parser.

Document-field admission is also structural. Wrapper `37` accepts field
discriminators `6`, `8`, `14`, and `15` as `SpreadSheetDocumentField`,
`CalendarField`, `GraphicalSchemaField`, and `HTMLDocumentField` respectively;
wrapper `48` is not admitted for these four types. The resulting population is
exact over the current native tree: 45, 4, 1, and 24 fields.

Calendar typed options use discriminator `6`. Defaults are omitted: Width 16,
Height 9, ShowMonthsPanel false, ShowCurrentDate true, WidthInMonths 1, and
HeightInMonths 1. Calendar field slot 10 is ToolTip, not InputHint; field 50
value 7 is `ToolTipRepresentation=ShowBottom`. An extended-tooltip wrapper 12
of length 34 uses slot 25 for `AutoMaxWidth`, where false is emitted and true
is the default. HTML typed height 10 is also a default and is omitted. HTML
option event `Click` maps structurally to `OnClick`.

Two UUID-valued Calendar option events were observed only in the BSP corpus.
The independent `buh` slice reproduced the global-command control marker but
contained no Calendar records, so those UUID meanings were not promoted.
Calendar typed events remain deliberately absent until portable evidence is
available.

The accepted gate changed only 63 Form.xml files. Full diff improved from
`1564 files, +31308/-192861` to `1560 files, +31287/-190997`;
`CommandName` improved from `+0/-23` to `+0/-11`. Every one of the 74 generated
field nodes is an order-preserving recursive subset of native XML, and no file
had a positive insertion delta.

### Form item picture assets

External item picture files are classified by the enclosing raw item record and
the exact property slot that owns the typed picture value. Item names, nearby
strings, serialized occurrence, payload hash, and configuration UUIDs are not
part of the model.

| Raw owner/property | External property |
| --- | --- |
| wrapper `12` `PictureDecoration`, typed value at options field `18/1` | `Picture` |
| wrapper `31` or `34` `Button`, typed value at `25 + top-level offset` | `Picture` |
| wrapper `55` `Table`, typed value at field `44` | `RowsPicture` |
| wrapper `37` `PictureField`, typed value at `29 + input offset` | `HeaderPicture` |
| the same PictureField, discriminator-`10` options field `5` | `ValuesPicture` |

The property value must have the typed `{4,3,...}` shape, contain exactly one
field-7 base64 child, and pass the existing image-signature check. Wrapper `73`,
nearby slots, non-picture reference kinds, wrong option discriminators, and
ambiguous payload lists fail closed. Export and inverse pack call the same
resolver.

An initial candidate additionally required an optional wrapper-55 tail shape
and admitted only 55 of 61 BSP assets. It was rejected before integration. The
accepted model covers 61 records in 17 forms: `Picture` 48, `RowsPicture` 7,
`ValuesPicture` 5, and `HeaderPicture` 1. Generated and native paths and SHA-256
hashes match 61/61; the serialized 12,198-file before/after tree is identical.

Supported by tests:

- Focused tests cover existing standard-command and table-standard-command
  resolution paths, but export confirmation is per UUID/status entry above.
- Unknown command tuples are intentionally omitted.

Hypothesis / needs validation:

- `CommandSource` has no remaining target diff (`+0/-0`). Additional item
  families in other configurations still require the same exact typed-record
  and owner-index evidence before widening source resolution.
- Form standard commands beyond the confirmed/test-supported UUID set remain
  hypotheses. Do not infer additional `Button/CommandName` mappings from
  excluded-command tables.
- Remaining table-standard UUIDs still require raw/native correlation, but the
  table owner rule itself is confirmed for multi-table forms: the tuple kind is
  the table item id, not a command category or button-name hint.
- The current draft does not prove every field owner type that can carry table
  standard commands. Remaining `CommandName` rows are concentrated in field
  owners and unsupported parent subtrees rather than ordinary tables.

## InformationRegister child objects

Status: XML reconstruction and reference-path classification confirmed by raw
corpus and serialized exports.

Raw child discriminator model across all 259 InformationRegisters:

- `27 -> 9` encloses a `Dimension`: 515 of 515 native dimensions, zero
  anomalies.
- `27 -> 7` encloses a `Resource`: 726 of 726 native resources, zero anomalies.
- A real metadata `Command` uses code `9` without enclosing code `27`: 20 of 20
  true commands.
- Generic command scanning must exclude markers already recognized as register
  child objects, including command-module path discovery.

Native child group order is stable:

```text
Resource -> Attribute -> Dimension -> Command
```

The order had zero violations across the 259 native files. All 578 nonempty
within-tag UUID sequences already matched native, so sorting is stable by group
and must preserve raw UUID order inside each group.

Confirmed by export:

- The first shape-only patch removed false Commands and restored Resources but
  was rejected because inter-group order created 258 paired tag additions and
  deletions.
- After adding the proven group order, the release/export shortstat was
  `1914 files changed, 32603 insertions(+), 230412 deletions(-)`.
- InformationRegister `Dimension`, `Resource`, `Attribute`, and `Command`
  opening-tag metrics were all `+0/-0`.
- The same owner-aware classifier is applied before generic code-9 handling in
  both reference indexes and standalone child-reference paths. Its serialized
  gate improved the full diff from
  `1914 files, +32603/-230412` to `1902 files, +32596/-230391`, with zero added
  Attribute, Dimension, Resource, or Command reference lines.

The accepted payload gate completes the properties of all 1,595 data children
(515 Dimensions, 726 Resources, and 354 Attributes) plus 20 Commands. Child
wrappers, collection counts, and typed envelopes are exact and fail closed.
`DesignTimeRef` and `FixedArray` use platform TypeIds; references must resolve
through the current metadata/predefined indexes and remain under the same
logical owner. `Type` and `TypeSet` retain native stable partition order.
Command pictures distinguish empty descriptors, current CommonPictures,
IR-scoped standard pictures, and the platform Print descriptor.

Direct code-14 forms are admitted only by their exact form record shape and a
single metadata owner. The complete form index is shared by streamed,
non-streamed, and module-path routing; FilterCriterion-shaped code-14 rows are
negative controls. Selected predefined dependencies are index-only and do not
leak extra output files.

The integrated gate changed exactly 259 root InformationRegister XML files.
All 1,615 target child/command subtrees are native-exact for count, identity,
order, Properties, and complete XML. The full diff moved from
`1559/+31284/-187135` to `1559/+31268/-142685`; the 12,198-file manifest had no
additions, removals, or changes outside InformationRegisters. A second SFC
gate covered 309 registers and 3,566 payloads with zero canonical mismatch.
Seven added production UUID literals had zero intersection with 10,080 BSP
metadata identity UUIDs; no database object names or absolute paths were
added. `Configuration.xml`, all Form/DCS files, and `ConfigDumpInfo.xml`
retained their accepted SHA-256 content.

## InformationRegister root properties

Status: the physical owner protocol and formatter are implemented and covered
by focused tests. A native-tree exactness gate remains pending because the
current run explicitly excludes ConfigDump and full configuration export.

The root envelope is exact: 9 fields with the owner at index 1. A true
InformationRegister owner has code 33, exactly 39 fields, and the metadata
header wrapper at owner index 15 as `{0, header}`. The 25 logical property
slots are the wrapper code/header followed by owner fields 16 through 38. Code
33 alone is not sufficient: Task uses the same code with a different shape and
is a required negative control.

The scalar tail uses strict platform domains for the four form references,
periodicity, write mode, edit type, standard commands, help/filter flags, data
lock mode, full-text search, five localized presentations, totals, and data
history. Localized wrappers must be counted, fully consumed, and preserve
language order. Nonzero form UUIDs resolve only to a unique form owned by the
same InformationRegister. Any malformed scalar, localized wrapper, or form
reference suppresses the complete owner-property result; the generic legacy
fallback is not mixed into a partially decoded InformationRegister.

Slot 12 is a strict union:

- `{0}` omits `StandardAttributes`;
- legacy bags use `{13,24}` and contain the canonical property-key set without
  `TypeReductionMode`;
- modern bags use `{14,25}` and contain all 25 canonical property keys.

Both populated variants use the exact four-marker payload for Active,
LineNumber, Recorder, and Period. Property identity is resolved by named
platform property UUID, not by object name or corpus position. Wrapper types,
counts, marker order, section UUID, keys, duplicates, enum values, fill-value
types, and full consumption are validated fail closed. The legacy shape derives
the structural default `TypeReductionMode=TransformValues`.

The formatter emits the decoded block once in native direct-child order and
does not emit a generic InformationRegister property subset when exact parsing
fails. Independent captured legacy/modern bags, reordered and duplicate keys,
wrong wrapper types, malformed counts, foreign forms, alternate owner shapes,
and nonmodal property values are covered by focused tests.

Integrated commit `7ce449f` passes 30 InformationRegister tests and 12 shared
register-standard-attribute tests. Exact full-suite failure-name comparison
against its parent found no new failure and fixed the prior
`extracts_information_register_data_lock_control_mode_from_extended_owner_fields`
failure. Production contains only platform serialization identifiers; audits
found no database/object identities, corpus names, absolute evidence paths, or
corpus/raw-shape version branches. The only source-version branch is the
independently proven V2.21 `pal` namespace insertion; V2.20 omits it. No
ConfigDump, configuration export, DB access, or `ConfigDumpInfo.xml` change was
performed for this gate.

## ExchangePlan root properties

Status: the exact saved BSP/UT owner protocol and formatter are implemented
and covered by independent captured-tail and mutation tests. The saved export
tree was not refreshed; native exactness, outside-scope preservation, and an
ExchangePlan-specific V2.21/SFC gate remain pending.

The root is exactly `{1, owner, 5, C0, C1, C2, C3, C4}`. The five ordered
counted collections have platform TypeIds for Attributes, Templates,
TabularSections, Forms, and Commands. Declared counts, item counts, TypeId
order, and complete consumption are strict. Direct Template and Form items use
strict nonzero UUIDs; Attribute, TabularSection, and Command collection items
are required only to be nonempty fully braced payload shells at this layer.
BSP owners use code 36 with 50 fields; UT owners use code 37 with 51 fields and
the final strict sentinel `1`. Both layouts have the direct metadata header at
owner index 12. Code 37 is observed with XML 2.20 and is not a source-version
discriminator.

Owner fields 13 through 49 encode the complete root property tail:

- standard commands, code and description lengths, presentation/edit/choice
  enums, and input-by-string behavior;
- six form properties: three default and three auxiliary forms;
- BasedOn, Characteristics, distributed-mode, help, extension, lock, search,
  creation, choice-history, and data-history properties;
- five localized presentation/explanation wrappers;
- StandardAttributes and DataLockFields reference collections.

Every enum, boolean, decimal, wrapper, and ordered field-reference sequence is
validated against the saved 6 BSP + 22 UT contract. BasedOn uses a resolved
metadata design reference. DataLockFields accepts only Code or an Attribute of
the current owner. All 31 observed nonzero form UUIDs resolve uniquely under
the same ExchangePlan; filename/name fallback is forbidden. The same strict
resolver covers the three auxiliary slots, whose saved corpus values are all
zero and whose nonzero tests are ownership-protocol extensions rather than
corpus claims.

StandardAttributes is either exact `{0}` or an eight-marker payload in the
fixed order ExchangeDate, ThisNode, ReceivedNo, SentNo, Ref, DeletionMark,
Description, Code. The legacy shape has 24 canonical keyed properties and the
modern shape adds TypeReductionMode as the 25th protocol property. Shape,
never owner code or XML version, selects the variant. Shared named-key and
wrapper parsing is reused from InformationRegister, but dynamic policy remains
family- and marker-specific. In particular, ExchangePlan string FillValue is
allowed only for Description and Code; InformationRegister Boolean, decimal,
and date FillValue domains do not leak into this family.

The complete owner-property result is atomic for the root collection
envelopes/counts/TypeIds/item shells and for every owner scalar, localized
wrapper, form/reference, and StandardAttributes bag. Malformed nested
Attribute, TabularSection, or Command payloads remain independently omittable
by their existing child parsers and do not suppress a valid root property tail.
Generated types, existing Attribute children, and generic Form/Template child
references remain independently preserved and are emitted once in their
existing order. The dedicated formatter inserts the complete tail once in
native direct-child order.

The independent captured owner-tail fixtures use an explicit leaf-only
sanitization: whitespace outside quoted strings is compacted, every nonempty
Russian localized content value is replaced with `Text`, and the two nonzero
UT form UUIDs are replaced with neutral `90000000-0000-4000-8000-000000000001`
and `90000000-0000-4000-8000-000000000002`. Braces, field order, scalar values,
wrapper shapes, protocol UUIDs, and all other leaf values remain unchanged.

Integrated commit `060ea1b` passes 23 ExchangePlan tests, 24 shared
StandardAttributes tests, and all 30 InformationRegister regressions. Two
independent reviews approved frozen diff SHA-256
`A7860C04D02719EAA2D38C422CEE2F88448C5444698AC961F3B311DF25700370`.
Exact full-suite comparison against parent produced the same 74 failure names
with five additional passing tests. The new production UUID literals are
exactly six named platform protocol TypeIds; no database object/form/header
UUID, object name, evidence path, or filename fallback is present.

No DB access, ConfigDump, configuration export, or `ConfigDumpInfo.xml` change
was performed. The V2.21 `pal` namespace test reuses the generic metadata
envelope rule and is not presented as ExchangePlan-native proof.

## Catalog attribute payloads

Status: confirmed by complete BSP and independent UT corpora, serialized
exports, and an integrated native-tree gate.

Catalog Attributes use an exact `27/23` payload under a typed header and
Pattern. BSP direct Attributes use wrapper code 5/length 6 under Catalog root
layout 56; UT direct Attributes use code 6/length 8 under root layout 57, with
the exact reserved `0,{1,zero UUID}` tail. Nested Attributes use code 8/length
5 in both corpora. Direct and nested collection markers are platform
structural discriminators, declared counts and every item close exactly, and a
collection cannot mix wrapper cohorts.

All scalar enums are Catalog-specific. Structured ChoiceParameterLinks,
LinkByType, ChoiceParameters, FillValue, and typed arrays use checked counts
and current metadata/predefined indexes. Nonempty ChoiceForm references are
valid only when their form owner is the unique CatalogRef in that Attribute's
Pattern; all 10 independent UT cases are cross-Catalog references. Nested
Attributes omit Use, FillFromFillingValue, and FillValue only after proving the
raw omitted values are their exact defaults. Unsupported or ambiguous values
suppress the complete property tail.

The accepted BSP gate reconstructed all 1,560 tails (1,108 direct and 452
nested) across exactly 109 changed Catalog root XML files, adding 44,663 native
lines and no non-native lines. The UT gate matched all 8,607 tails (7,106
direct and 1,501 nested), including ChoiceForm 10/10. A full metadata-only
manifest checked 9,212 files with zero changes outside the 109 roots and did
not generate ConfigDumpInfo.

The first integrated export was held because adding large exact tails exposed
an older group-order debt in 57 mixed Catalogs. Raw traversal put
TabularSection before Attribute, while native requires stable
`Attribute -> TabularSection` groups. A metadata-kind-only stable sort preserved
UUID sets and within-group order and reduced the Catalog root diff from
`114/+34013/-35695` to `114/+1718/-3400`. The final full diff moved from
`1559/+31268/-142685` to `1559/+20562/-87316`; manifest outside/add/missing
counts were all zero. Remaining Catalog differences are pre-existing root,
TabularSection, and Type nodes, not Attribute tails or child order.

## Document attribute payloads

Status: confirmed by complete BSP, UT, and SFC corpora, serialized
metadata-only exports, and an integrated native-tree gate.

Document Attributes use the same exact `27/23` common payload shape under a
typed header and Pattern, but their wrapper and omission rules are
Document-specific. BSP direct Attributes use wrapper code 5/length 5; UT and
SFC direct Attributes use code 6/length 7 with the reserved
`0,{1,zero UUID}` tail. Nested Attributes use code 8/length 5. Direct and
nested collection markers are platform storage discriminators. The parser
validates declared counts, wrapper cohorts, item closure, child identity, and
the parent TabularSection before emitting any property tail.

Direct Attributes emit 24 tail properties. Nested Attributes emit 22 and omit
`FillFromFillingValue` and `FillValue` only after validating their exact raw
defaults. `ChoiceForm` is accepted only when its owner is the unique matching
CatalogRef or DocumentRef in the Attribute Pattern. Choice parameters use a
recursive typed FixedArray representation because arrays can mix
DesignTimeRef, Nil, Decimal, String, and nested arrays. Predefined values are
indexed by canonical `owner-value:{owner}:{uuid}` keys; ambiguous bare UUIDs
are removed, qualified conflicts fail, and qualified keys are never emitted.

Document data paths require current-owner structural proof, exact global
reference agreement, and the correct direct or TabularSection/child role.
Only ChoiceParameterLinks may preserve native raw paths, and only for exact
`{-8}`, `{0}`, `{0}/{0}`, or single/double UUID shapes whose IDs are not
ambiguous and do not resolve to the current Document. LinkByType never uses
this fallback. A dangling Enum FillValue is preserved as raw owner/value UUIDs
only for an exact nonzero DesignTimeRef envelope, a sole matching
`cfg:EnumRef.*` Attribute type, and a value absent from every bare, qualified,
type, and form index.

The accepted gates covered 25 BSP Documents and 441 Attributes, 285 UT
Documents and 13,479 Attributes, and 891 SFC Documents and 48,662 Attributes.
All tail, identity, and group-order mismatches were zero. SFC additionally
proved 6,556 ChoiceParameterLinks (6,508 semantic plus 48 controlled raw), 384
nonempty LinkByType values, 61 ChoiceForms, 5,136 ChoiceParameters, and one
guarded dangling Enum FillValue. Eager and streamed full-run dependency
prefetch both include Document owners; selected runs preserve their exact
requested set.

Shared integration changed exactly 25 root `Documents/*.xml` files and nothing
else. The full diff moved from `1559/+20562/-87316` to
`1559/+15818/-69978`, removing 22,082 differing lines. `ConfigDumpInfo.xml`
was not generated or modified; its SHA-256 remained
`F187FA4F131F9C5DCBD2E41FE630585B1D6C74FB2809D62F4B3B3F0563425A2F`.
Production adds only the two platform collection marker UUIDs and contains no
database object names, absolute paths, or source-version data-shape branches.

## WebService properties, Operations, and Parameters

Status: confirmed by complete BSP, UT, and SFC corpora, serialized selected
exports, two independent reviews, and an integrated native-tree gate.

WebService roots use the exact shape
`{1,{4,Namespace,Header,MetadataPackages,DescriptorFileName,NamespacePackages,ReuseSessions,SessionMaxAge},1,Operations}`.
Metadata package references use the platform metadata-object-reference TypeId;
literal namespace packages follow them in native order. Operations use the
platform operation collection marker and exact code-1/length-7 payload;
Parameters use the platform parameter collection marker and exact
code-0/length-5 payload. Every wrapper, declared count, item tail, quoted or
localized value, and UUID is consumed completely. Owner, package, operation,
and parameter UUIDs form one case-normalized uniqueness set. Child names are
nonempty, dot-free, and case-insensitively unique in their native scope.

Raw `ReuseSessions` values map as `0=DontUse`, `1=Use`, and `2=AutoUse`;
unknown values fail. Operation lock mode maps `0=Automatic`, `1=Managed`;
parameter direction maps `0=In`, `1=Out`, `2=InOut`. QName local names follow
the XML 1.0 Fifth Edition NCName grammar. XML Schema and 1C core namespaces use
the existing `xs` and `v8` prefixes; custom operation and parameter namespaces
use inline `d6p1` and `d8p1` declarations. XML/XMLNS reserved namespace URIs,
invalid XML 1.0 characters, malformed UUIDs, unresolved XDTOPackages, and
ambiguous identity paths fail closed. Source 2.21 adds `xmlns:pal` locally for
WebService roots between `lf` and `style`; source 2.20 does not.

The selected gates matched native XML byte-for-byte for all 13 BSP, 18 UT, and
20 SFC services: 559 Operations, 1,476 Parameters, and 52 XDTO package items in
total. Output allowlists contained only the requested root XML files and each
manifest. Paired `.0` rows were read for reference preparation but emitted no
module files; native module SHA-256 manifests had zero delta. No gate generated
or referenced `ConfigDumpInfo.xml`, and the existing native files retained
their SHA-256 content.

Shared integration changed exactly 13 `WebServices/*.xml` roots in the
12,198-file BSP tree. The full diff moved from `1559/+15818/-69978` to
`1546/+15779/-61205`, removing 8,812 differing lines; the WebServices subtree
now has zero diff. Added production literals are limited to three independently
proven platform protocol UUIDs, canonical XML/XDTO namespace URIs, and the zero
UUID. There are no database or configuration object names, absolute paths,
corpus identities, raw fallbacks, or source-data-specific branches.

## Subsystem properties and content

Status: confirmed across all 244 raw Subsystem records and by serialized export.

Every record has a four-field root whose metadata-object record at the known
position has exactly nine fields. The fixed object slots are:

| Slot | Export property | Corpus evidence |
| --- | --- | --- |
| `2` | `IncludeHelpInContents` | 110 true / 134 false |
| `4` | `IncludeInCommandInterface` | 108 true / 136 false |
| `5` | `Picture` | 231 empty / 12 CommonPicture / 1 StdPicture |
| `6` | localized `Explanation` | 229 empty / 15 localized |
| `7` | ordered `Content` references | 4,027 items, zero count/order violations |
| `8` | `UseOneCommand` | 244 false |

The root slot `3` is the ordered ChildObjects list: 206 are empty and 38 are
nonempty, containing 225 child subsystem links with zero shape/order
violations. Empty `Explanation`, `Picture`, `Content`, and `ChildObjects`
elements are schema-significant and must still be emitted.

The accepted gate improved the full diff from
`1902 files, +32596/-230340` to `1690 files, +32376/-224580`. `Explanation`,
`Picture`, `Content`, `ChildObjects`, `IncludeHelpInContents`,
`IncludeInCommandInterface`, and `UseOneCommand` all reached `+0/-0`; Form
`CommandName` and `CommandSource` guardrails stayed unchanged.

## CommandInterface section grammar

Status: confirmed across 78 subsystem blobs and by export; scoped issue closed.

The raw root discriminator is `7`, followed by five ordered sections:
visibility, placement, command order, subsystem order, and group order. Each
section starts with a `0/1` presence marker; a present section adds a count and
exactly that many records. The document ends with one trailing `0` and no extra
fields. Command-order records store `(group UUID, command reference)` in that
order. Subsystem UUIDs resolve through the qualified subsystem index; all 60
observations were nested qualified names, with zero leaf fallbacks.

Of 820 command references, three are exact bare `{0}` records: two in
Visibility and one in CommandsOrder. Native emits literal name `0`. Placement
has no bare records and remains typed-only; other bare codes, invalid UUIDs,
wrong arity, or trailing data are rejected.

The first strict parser was rejected because it dropped the 1,074-line
Administration CommandInterface on the valid bare order record. After rollback
and the exact `{0}` refinement, the accepted gate moved the full diff from
`1686 files, +32376/-224137` to `1642 files, +32149/-223052`; all 44 scoped
CommandInterface files reached byte parity and the source-asset count stayed
unchanged.

## Source module routing

Status: the Chart module-routing submodel is confirmed by raw/native bytes and
export.

Module routing keys on `(metadata family, source suffix)`, never suffix alone:

| Family | Suffix | Canonical module |
| --- | --- | --- |
| `ChartOfAccounts` | `14` | `ObjectModule.bsl` |
| `ChartOfAccounts` | `15` | `ManagerModule.bsl` |
| `ChartOfCalculationTypes` | `0` | `ObjectModule.bsl` |
| `ChartOfCalculationTypes` | `3` | `ManagerModule.bsl` |
| `ChartOfCharacteristicTypes` | `15` | `ObjectModule.bsl` |
| `ChartOfCharacteristicTypes` | `16` | `ManagerModule.bsl` |

The matrix was checked over 1,791 module entries. Help and Predefined suffixes
are negative controls; suffix `15` itself demonstrates why a global mapping is
invalid. The accepted gate removed four fallback Config_module_text files and
restored four canonical modules, improving tracked diff from 1690 to 1686 files
and deletions from 224580 to 224145.

## Chart predefined data

Status: the `ChartOfAccounts` and `ChartOfCalculationTypes` models are confirmed
by raw structure and full export.

Predefined-data routing and parsing are keyed by metadata family and exact
source layout. `ChartOfAccounts` uses suffix `9`, root discriminator `2`, and a
nested rowset; `ChartOfCalculationTypes` uses suffix `2`, root discriminator
`9`, and a root rowset. Row schemas provide column ids and value offsets.
Account flags are the non-fixed boolean schema columns, and references are
resolved through metadata-derived object and predefined-item indexes.

The reference index is built from all parsed Predefined bodies and contained
314 unique items in the BSP corpus. Missing or ambiguous references fail the
export; UUID passthrough and owner/file exceptions are not used. Catalog and
ChartOfCharacteristicTypes bodies retain their independent generic parser.

The accepted gate restored the complete 442-line ChartOfAccounts payload and
18-line ChartOfCalculationTypes payload. Both target files reached zero content
diff, while the full diff moved from `1629 files, +32044/-222947` to
`1627 files, +32044/-222487`. Added production lines contained no UUID literals,
Cyrillic object names, or object/path special cases.

The remaining generic Predefined diff was lexical and closed exactly over all
314 items: 276 empty `Code` values require `<Code/>`, 207 empty `Description`
values require `<Description/>`, and eight descriptions retain literal quotes
in XML element text. The counts sum to all 491 remaining line pairs. After the
canonical text-element gate, all 19 Predefined files had zero content diff and
the full diff moved to `1616 files, +31553/-221996`.

## Calculation-register Recalculations

Status: current-metadata routing and the complete selected payload are
confirmed by a serialized full export.

A Recalculation source row shares raw object code `4` with unrelated metadata
rows. Code `4` alone is therefore not a route. A row is treated as a
Recalculation only when its UUID occurs in the Recalculation child list of a
current `CalculationRegister`. Ambiguous owners and sanitized-path collisions
are removed from the index. Undeclared code-4 rows continue through the common
metadata serializers; a declared but malformed Recalculation fails closed.

The child body has an exact four-field root. Its object record supplies six
nonzero IDs for the `RecalculationRecord`, `RecalculationManager`, and
`RecalculationRecordSet` generated types. Root lock mode `0` maps to
`Automatic`, and `1` maps to `Managed`. The dimension list is identified by
platform serialization marker `3c456b74-4ea5-4b22-a957-e9fad9133b54`; this is
not a metadata-object UUID. Each dimension must resolve its register-dimension
UUID through current `object_refs` to
`CalculationRegister.<owner>.Dimension.<name>`.

The accepted gate added exactly one exported path and no other content change.
The generated Recalculation is 3,812 bytes and byte-identical to the native
checkout. Full diff moved from `1565 files, +31308/-192925` to
`1564 files, +31308/-192861`; Form command guards remained unchanged.

## Configuration root indexes

Status: `InternalInfo` and `ChildObjects` are `confirmed by export` for the
2.20/BSP code-67 root layout. Root `Properties` remain separately incomplete.

The admitted root envelope is structurally:

```text
{2,{<configuration UUID>},7,<contained 0>...<contained 6>,{{0,"",""}}}
```

Each contained entry has a nonzero class UUID and one payload. The payload has
exactly one `{1,0,<ObjectId>}` marker and an ordered sequence of family lists.
The seven `(ClassId,ObjectId)` pairs serialize unchanged and in raw order as
`InternalInfo/xr:ContainedObject`.

Each family list contains a family UUID, an exact child count, and that many
metadata UUIDs. Empty families are skipped. Every nonempty family must resolve
all UUIDs to one supported metadata kind; unresolved, nested, mixed-kind,
duplicate-kind, duplicate-UUID, unknown-kind, and malformed count/footer cases
fail closed. Order inside a family is raw order. Family groups use the proven
platform metadata-kind order, not metadata object names or UUID sorting.

The ordinary object reference index deliberately excludes `DefinedType` for
other serializers. Configuration therefore receives a separate reference view
that adds only structurally recognized DefinedType root rows. This avoids a
broad reference-index change: the rejected broad variant also changed 32
Subsystem XML files, while the scoped variant changed only `Configuration.xml`.

The accepted gate preserved all 12,198 exported paths and 4,929 metadata XML
rows. Native/generated `InternalInfo` matched `7/7` in exact sequence and
`ChildObjects` matched `3518/3518` in exact kind/name sequence. A whole-tree
SHA-256 manifest found exactly one changed path. The full diff moved from
`1559 files, +31284/-190971` to `1559 files, +31284/-187421`; the Configuration
residual moved from `+0/-3786` to `+0/-236`, with no added lines. Code-67,
code-68, and code-76 root property layouts have different field counts and are
not treated as interchangeable merely because each is a configuration root.

The next accepted Configuration-only cohort reconstructs
`UsedMobileApplicationFunctionalities` from raw slot 53. Root admission is the
structural `root[3] -> contained payload -> property payload` path with exact
67/60, 68/61, or 76/77 layouts and matching internal header identity. Layout
does not imply source version. The accepted raw count pairs are 2.17/37,
2.20/37, and 2.21/38; platform ids, order, boolean values, trailing field, and
all lengths are validated as one fail-closed unit. The gate changed only
`Configuration.xml` by +154/-0, moved the full diff from
`1559/+31284/-187327` to `1559/+31284/-187173`, and reduced the Configuration
residual to `+0/-82`. `ConfigDumpInfo.xml` was explicitly excluded and its
SHA-256 content did not change.

Three further Configuration-only cohorts use the same admitted root property
payload and fail closed independently. `UsePurposes` accepts the exact
application-purpose envelope and emits `PlatformApplication`. `DefaultRoles`
accepts the exact Role reference envelope, but resolves every target UUID
through the current `object_refs` index; database role names and object UUIDs
are not literals in production. The five localized fields at raw slots 4..8
are parsed atomically with exact declared counts, quoted strings, and unique
language keys. A malformed member suppresses the entire five-field cohort
rather than mixing strict and legacy output.

The serialized gates changed only `Configuration.xml`: `UsePurposes` added 3
native lines, `DefaultRoles` added 5, and the localized cohort added 30. The
full diff moved successively to `1559/+31284/-187170`,
`1559/+31284/-187165`, and `1559/+31284/-187135`; the remaining Configuration
residual is `+0/-44`. The two new UUID literals are platform TypeId markers,
observed with the same meaning in the BSP layout 67 and independent UHA
layouts 68 and 76. Neither occurs anywhere in the 12,198-file exported BSP
tree, which includes the metadata inventory. `ConfigDumpInfo.xml` remained
outside scope and retained its SHA-256 content after every gate.

## AccountingRegister RecordType presence

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

AccountingRegister standard-attribute definitions use an exact triplet
collection. Each item contains a marker, the platform standard-attribute
section UUID, and a legacy `13/24` or modern `14/25` property bag. Marker
`-9` denotes `RecordType`. It is present for a register without
correspondence and absent from the paired register with correspondence, but
the marker itself is the serialization source of truth. Object names,
database UUIDs, and the `Correspondence` property are not used as fallbacks.

The collection parser is atomic. It validates the complete envelope, declared
count and cardinality, scalar and indexed marker shapes, the existing platform
section UUID, every property bag, and one uniform legacy/modern bag shape.
Tooltip and synonym overrides are decoded during the validation phase. Only
after all triplets are valid are presence and overrides retained. A malformed
member therefore leaves the existing 11 AccountingRegister standard
attributes unchanged and cannot emit a partial `RecordType`.

When marker `-9` is present, the shared standard-attribute formatter emits the
native default payload after `Account` and before `Active`. Captured positive
and negative raw records also confirm the existing indexed ExtDimension marker
families and an unknown negative singleton marker, so validation is structural
rather than restricted to the current named set.

Integrated commit `449b771` passes 3 direct RecordType tests, 6
AccountingRegister tests, and 13 shared register-standard-attribute tests. The
full suite changed from `1260 passed / 74 failed / 6 ignored` to
`1264 passed / 74 failed / 6 ignored`; the exact 74 failure names are
unchanged. Two independent reviews approved frozen diff SHA-256
`583B30BBD0612EAD3D53CF6697C595689730A8C67863558B96DFC08540FE5CFB`.
The patch adds no production UUID literal and contains no database object name,
UUID, path, or corpus branch.

The saved normalized residual is one root XML at `+0/-27`; the paired
`Correspondence=true` root is already exact. The expected residual after a
future permitted native gate is `+0/-0`, but this has not been claimed without
that gate. No database access, ConfigDump, configuration export, or
`ConfigDumpInfo.xml` change was performed.

## Sequence generated types in EventSubscription patterns

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

Sequence metadata uses object code `6` with its header at field 7. Fields 1/2,
3/4, and 5/6 are the dynamic TypeId/ValueId pairs for SequenceRecord,
SequenceManager, and SequenceRecordSet. The generated-type index retains only
TypeIds at fields 1, 3, and 5 and derives each qualified `cfg:` name from the
parsed Sequence header. Generated IDs, Sequence names, and object UUIDs are not
production literals.

Object code `6` is shared with Role, whose header is at field 1. The Sequence
schema therefore requires both `HeaderIndex(7)` and a valid UUID range covering
all six generated-type fields. The header guard anchors the Sequence owner
shape; the UUID-range guard validates the complete three-pair envelope rather
than only the selected TypeId slots. Saved BSP evidence admits the Sequence and
rejects all 160 Roles. UHA and SFC saved selections contain no additional code-6
positive, so they provide absence rather than independent positive coverage.

The existing strict EventSubscription parser resolves specific pattern TypeIds
through this index. Qualified Sequence types are emitted as `v8:Type` in their
input order; the unqualified `cfg:SequenceRecordSet` family remains a
`v8:TypeSet`. No EventSubscription-specific name or TypeId fallback was added.

Integrated commit `e278bc2` passes the guarded Sequence index, Role collision,
malformed UUID-range/header, Event Source order, and generic TypeSet tests. The
full suite changed from `1264 passed / 74 failed / 6 ignored` to
`1268 passed / 74 failed / 6 ignored`; the exact 74 failure names are unchanged.
Two independent reviews approved frozen diff SHA-256
`CDB40AAA3B674E8DA54D12D3F1368D9DB2877A7012B3454D6DC3BD993573E336`.
The production change adds no UUID literal, object name, path, or corpus branch.

The saved normalized residual is two EventSubscription roots totaling
`+8/-150`. The expected future permitted native gate is `+0/-0` for those two
roots, but exactness is not claimed without that gate. The full Sequence root
and Dimension serializer remains outside this scope. No database access,
ConfigDump, configuration export, or `ConfigDumpInfo.xml` change was performed.

## SettingsStorage default forms

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The SettingsStorage root has five fields: tag `1`, an eight-field code-`2`
owner, declared child-collection count `2`, an empty template collection, and
the owner form collection. The owner fields at slots 4 and 5 are respectively
`DefaultLoadForm` and `DefaultSaveForm`; slots 6 and 7 are zero in the available
evidence and are therefore rejected when nonzero rather than interpreted as
unproven auxiliary-form references.

The parser consumes the complete root value and validates the exact owner and
header wrappers. Both child collections require their platform collection
UUID, a digits-only count, exact cardinality, and strict UUID syntax. Available
raw evidence supports only an empty template collection. Form UUIDs must be
unique case-insensitively. A nonzero default must occur in that owner collection
and resolve uniquely through the current form index to
`SettingsStorages/<owner>/Forms/<name>.xml`; missing, ambiguous, CommonForm, and
cross-owner matches fail atomically. Object, form, and database UUIDs are not
production literals.

The formatter always emits `DefaultSaveForm`, `DefaultLoadForm`,
`AuxiliarySaveForm`, and `AuxiliaryLoadForm` immediately after `Comment`, in
that native order. Version 2.20 retains the shared namespace set. Version 2.21
adds exactly one palette namespace immediately before the style namespace, as
observed in the native SettingsStorage schema.

Integrated commit `5501111` passes all 9 focused SettingsStorage tests. The
full suite changed from `1268 passed / 74 failed / 6 ignored` to
`1273 passed / 74 failed / 6 ignored`; the exact 74 failure names are unchanged.
Independent live review approved frozen diff SHA-256
`EA2723C7303B3C1BC171CA90693B48C431D9FD08CDBE8AB49FDF67A93F9BB28D`.
The only new production UUID literals are the two platform collection IDs,
both taken from the saved raw envelope; no corpus object name, UUID, or path is
embedded.

The saved normalized residual is the single SettingsStorage root at `+0/-4`.
The expected future permitted native gate is `+0/-0`, but exactness is not
claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## Report child template collections

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

Code-19 Report roots contain exactly five counted child collections. The root
has eight fields: tag `1`, the Report owner, declared collection count `5`, and
the five collections. The first collection uses the shared platform template
collection UUID. Its members are strict, unique nonzero template UUIDs in
native `ChildObjects/Template` order. The remaining four collections retain
opaque members, but their outer UUID/count/cardinality envelopes are validated
and all five collection UUIDs must be unique.

The saved corpus contains 52 code-19 Reports with this envelope. Template
collection counts range from 1 to 7; every inventory resolves to same-owner
template paths in native order. Five otherwise-exact roots expose the isolated
residual with counts 4, 3, 2, 2, and 2. One of them places its main data
composition schema second in the raw collection, proving that neither
`MainDataCompositionSchema`, alphabetical sorting, nor a name scan defines the
child order. Code-20 Reports remain on their established legacy path.

Every template member must resolve uniquely, case-insensitively by UUID,
through the current template index. The resolved entry must be a `Template` at
`Reports/<owner>/Templates/<name>.xml`. Distinct UUIDs that resolve to the same
Unicode-lowercased name are rejected. Any malformed envelope, UUID,
resolution, kind, owner, path, or name collision empties the complete Report
child-template cohort without suppressing independently parsed Report
properties, forms, or commands. Selected Report exports continue to request
the broad metadata index needed to resolve child templates.

Integrated commit `e014863` passes 3 atomic parser/resolution tests, the
selected-index routing test, and the existing Report XML end-to-end test. The
full suite changed from `1273 passed / 74 failed / 6 ignored` to
`1277 passed / 74 failed / 6 ignored`; the exact 74 failure names are unchanged.
Independent frozen review approved diff SHA-256
`8A5025004224F1C82571A26792F8579EEAE9ADBB3F044176BB3D0C8218DDC589`.
Production reuses the existing platform collection UUID and adds no Report,
template, database, or path identity.

The saved normalized isolated residual is five Report roots totaling `+0/-8`.
The expected future permitted native gate is `+0/-0` for those roots, but
exactness is not claimed without that gate. No database access, ConfigDump,
configuration export, or `ConfigDumpInfo.xml` change was performed.

## FilterCriterion strict V2.20 metadata

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

Code-14 FilterCriterion metadata is admitted in three fully consumed forms.
Two root-length-3 legacy envelopes retain the existing minimal XML behavior.
The strict V2.20 form has root length 5 and an owner with 13 fields. It emits
the four generated forms from owner slots 1 through 4, a nonempty `Pattern`
type from slot 5, a counted `Content` reference list from slot 6, and
`UseStandardCommands` from the boolean in slot 7. Slots 8 and 9 must be zero,
slots 10 through 12 must be exact empty envelopes, and the two root-tail
collections must be distinct strict nonzero empty envelopes. Any other or
partially malformed code-14 shape is suppressed rather than falling back to a
legacy partial document.

Generated type names are resolved through the shared type index. Pattern type
qualified names must have the generic `cfg:<family ending Ref>.<name>` shape.
Content UUIDs resolve uniquely and case-insensitively through the base object
reference index to either an owner attribute or a tabular-section attribute.
Resolved paths and names must be nonempty and exact; Unicode-lowercase
reference collisions reject the complete object. The saved positive row has
`UseStandardCommands=false`. Its observed tail collection UUIDs, metadata UUIDs,
object names, and paths are deliberately absent from production literals.

Only the full V2.20 namespace/version envelope is enabled. A strict full V2.21
form remains on hold because no native FilterCriterion sample proves its
namespace shape. The two exact legacy envelopes remain version-neutral for
compatibility.

Integrated commit `a572554` passes 11 focused FilterCriterion tests. The full
suite changed from `1277 passed / 74 failed / 6 ignored` to
`1284 passed / 74 failed / 6 ignored`; the exact 74 failure names are unchanged.
Independent frozen review approved diff SHA-256
`5A0B1AEED2C26632B2F002CCC19ACEE0644317A052A7BD78D9BAAB2ADCD158E4`.

The saved normalized residual is the single FilterCriterion root at `+1/-30`.
The expected future permitted native gate is `+0/-0`, but exactness is not
claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## Metadata child-command shortcut modifiers

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The saved native tree has exactly four nonempty shortcuts in direct metadata
root command objects. Their native/raw pairs are `F3` / `{0,114,0}`, `F2` /
`{0,113,0}`, `Ctrl+Alt+F` / `{0,70,24}`, and `Ctrl+Shift+S` / `{0,83,12}`
across Catalog, DataProcessor, and DocumentJournal owners. Empty shortcuts use
`{0,0,0}`.

The shared child-command decoder now requires an exact three-field tag-zero
tuple. It accepts only uppercase ASCII key codes 65 through 90 and F1 through
F12 key codes 112 through 123. The only modifier bits are `Shift=4`, `Ctrl=8`,
and `Alt=16`; output order is Ctrl, Alt, Shift, then the key. Zero keys,
unsupported key codes, unknown bits, wrong tags, missing fields, and trailing
fields are rejected. The separately proven strict InformationRegister command
parser is unchanged.

Integrated commit `467fae0` passes the strict decoder matrix, the legacy
F-key/empty control, and the DataProcessor raw-to-XML end-to-end test. The full
suite changed from `1284 passed / 74 failed / 6 ignored` to
`1285 passed / 74 failed / 6 ignored`; the exact 74 failure names are unchanged.
Independent frozen review approved the 4,296-byte diff with SHA-256
`64657058A390BCF936DCC67D792FDC7673E4AF727229A30776D3F05522DBC197`.
Production adds no UUID, metadata name, database identity, or path literal.

The saved normalized isolated residual is one DataProcessor root at `+1/-1`.
The expected future permitted native gate is `+0/-0`, but exactness is not
claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## DataProcessor SettingsComposer attribute types

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

Two independent DataProcessor attributes have the same platform type ID and
native qualified type `dcsset:SettingsComposer`. The code-27 Attribute cohort
in their owner raws contains 27 type Patterns: two exact single-member
positives and 25 local negatives. Across the complete raws there are 28
Pattern occurrences; the extra owner-level empty Pattern is outside code 27.

The platform type is admitted only after DataProcessor routing, code-27
Attribute recognition, an exact detail header match, an exact two-field
`Pattern`, and an exact two-field nonzero UUID member. A malformed, duplicate,
or mixed candidate, a duplicate resolved attribute, or any case-insensitive
collision with the dynamic type index suppresses the whole DataProcessor
source instead of degrading the candidate to `<Type/>`. The same UUID outside
the Attribute type Pattern has no special meaning. Both direct and
tabular-section attribute routes use this contract.

The namespace declaration is local to the emitted type element:

```xml
<v8:Type xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings">dcsset:SettingsComposer</v8:Type>
```

Integrated commit `58c8c78` passes six focused positive, routing, malformed,
collision, unrelated-context, and 25-negative-cohort tests. The full suite
changed from `1285 passed / 74 failed / 6 ignored` to
`1291 passed / 74 failed / 6 ignored`; the exact 74 failure names are unchanged.
Independent frozen review approved the 18,736-byte diff with SHA-256
`932040B4942A888FF3BE3272DE0EF04A780092D260D88284DCBB114DC64DC989`.
Production adds only the platform type ID, qualified name, and namespace; no
metadata owner/attribute identity, database name, or path is embedded.

The saved normalized residual is two DataProcessor roots totaling `+2/-6`.
The expected future permitted native gate is `+0/-0`, but exactness is not
claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## DocumentJournal generated type InternalInfo

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

All four saved DocumentJournal owners have an exact seven-field root with tag
`1`, declared collection count `4`, and a 17-field code-26 owner. The exact
metadata header is wrapped at owner slot 3. Generated type/value UUID pairs are
stored at slots 10/11 for Selection, 1/2 for List, and 8/9 for Manager. Native
XML order is Selection, List, Manager, independent of slot order. Type names
derive only from the parsed owner name.

The parser requires the exact root, owner, header wrapper, and complete header
match. All six IDs must be strict nonzero UUIDs and unique case-insensitively.
Any wrong tag, count, length, header component, malformed or zero UUID,
duplicate or case-duplicate UUID, or trailing field suppresses the whole source
instead of producing a partial InternalInfo cohort. No other DocumentJournal
property is inferred or emitted.

The dedicated formatter only adds the three GeneratedType blocks to the
minimal metadata root. Existing shared post-format routing still inserts owned
forms, templates, and commands once and in their established order. Tests use
four distinct six-ID cohorts with exact block assertions, a renamed header,
the complete malformed/duplicate matrix, absence of unrelated properties, and
nonempty Form/Template/Command preservation.

Integrated commit `92a1139` passes the five exact parser/routing gates. A broad
name filter also selects one already-known external-path source test; the full
suite proves it is unchanged. The suite moved from
`1291 passed / 74 failed / 6 ignored` to
`1294 passed / 74 failed / 6 ignored`, with exact failure-name delta zero.
Independent frozen review approved the 18,483-byte diff with SHA-256
`A733A7B2E07746C03193955B4D4639419F80E619FB777626B491BA63264ADDB9`.
Production adds no UUID, owner name, database identity, or path literal.

The saved normalized isolated residual is four DocumentJournal roots totaling
`+0/-56`: 14 lines per root, comprising the two InternalInfo wrapper lines and
three four-line GeneratedType blocks. An earlier `-68` estimate was corrected
before the implementation freeze. The expected future permitted native gate
is `+0/-0`, but exactness is not claimed without that gate. No database access,
ConfigDump, configuration export, or `ConfigDumpInfo.xml` change was performed.

## CommonAttribute additional-order exact tail

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

Two saved CommonAttribute roots have an exact three-field `[1, owner, 0]`
envelope, a 15-field code-5 owner, and owner fields 3 through 14 equal to:

```text
2,1,1,1,{1,zero},{1,zero},{1,zero},0,0,0,0,1
```

For only this complete structural candidate, native places an adjacent
11-element block immediately after `AutoUse`: DataSeparation `DontUse`,
SeparatedDataUse `Independently`, three empty separation references, Users,
Authentication, and ConfigurationExtensions separation `DontUse`, Indexing
`IndexWithAdditionalOrder`, FullTextSearch `Use`, and DataHistory `Use`.

Field 3 value `2` is candidate intent. The candidate parser requires the exact
root, owner, typed payload, full metadata header, scalar vector, and three exact
zero-reference envelopes. Any mismatch suppresses the whole source rather than
silently omitting the block. Field 3 values other than `2` continue through the
unchanged legacy separation parser. The broad CommonAttribute indexing and
separation mappings are deliberately unchanged.

Evidence includes two BSP positives, five same-code BSP controls with field 3
equal to `0`, and an independent saved UT raw pair with the same positive tail.
Three controls have the otherwise default zero-reference tail and native
`DontIndex`; two nonzero-reference controls prove that broader separation enum
semantics are not safe to infer. Across ten paired BSP/UT raws, all four field-3
value-2 occurrences match the exact candidate and no alternative tail occurs.
An SFC native-only positive lacks paired raw and does not broaden the model.

Integrated commit `4c553da` passes six candidate, control, header, mutation,
atomicity, adjacency, and production-literal tests. The full suite changed from
`1294 passed / 74 failed / 6 ignored` to
`1300 passed / 74 failed / 6 ignored`; the exact failure names are unchanged.
Independent frozen review approved the 18,090-byte diff with SHA-256
`1D2FBEDD94F138CF968437B4654C3C6D83493C903D59497E361919AB7328167B`.
Production adds no UUID, CommonAttribute name, database identity, or path.

The saved normalized isolated residual is two CommonAttribute roots totaling
`+0/-22`. The expected future permitted native gate is `+0/-0`, but exactness
is not claimed without that gate. No database access, ConfigDump,
configuration export, or `ConfigDumpInfo.xml` change was performed.

## DataProcessor strict EmptyRef FillValue

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The saved DataProcessor residual is one missing DesignTimeRef FillValue in a
tabular-section Attribute. The exact raw descriptor is an owner/zero reference
whose nonzero owner TypeId equals the Attribute's sole Pattern TypeId. A unique
type-index result of generic shape `cfg:<Kind>Ref.<name>` derives the native
value `<Kind>.<name>.EmptyRef`; no metadata-object identity is involved.

The admitted route is narrower than a lexical code-11 child. Native stores the
tabular Attribute list after the closed code-11 TabularSection payload. The
existing structural router must associate a code-27 Attribute with a parent
TabularSection, and the candidate must be inside the exact tabular attribute
list. Its inner `{0,{27,...},0}` and outer `{{0,{27,...},0},0}` wrappers,
declared count, cardinality, payload, full header, Pattern, and flattened
FillValue slot are all exact. Direct modern and legacy DataProcessor Attribute
layouts remain noncandidates; legacy FillValue omission is unchanged.

A descriptor-shaped DesignTimeRef envelope anywhere in the code-27 fields is
candidate intent. Once intent exists, the envelope must occur exactly in
flattened slot 20 with owner-nonzero/value-zero shape and the same singleton
Pattern TypeId. This catches shifted and malformed descriptors instead of
silently treating them as noncandidates. Mere UUID text in another property or
comment has no special meaning.

`MetadataTypeIndexes` now keeps a collision sidecar while preserving the
existing last-writer reference map and DCS inserts byte-for-byte. Any repeated
normalized TypeId, including an equal-qname duplicate, is marked collided.
Only this strict candidate consults the sidecar and a case-insensitive unique
map scan. The saved native tree has 4,623 generated TypeIds, all unique, and
their intersection with 10,818 XML object UUIDs is zero. A TypeId/object-index
collision is therefore rejected. Generic DesignTimeRef resolvers are unchanged.

The pre-scan/apply contract is atomic. Pattern/descriptor mismatches, missing
or malformed type references, collisions, bad qname grammar, duplicate
candidate identities, malformed list counts/wrappers, nonzero value IDs, or a
target whose FillValue emission is disabled suppress the whole source. A
matched target must be exactly one Attribute with one matching Reference type
and no pre-resolved FillValue.

Integrated commit `ec1f003` passes 11 EmptyRef protocol/routing tests, the
type-index collision invariant, and the selected DataProcessor broad-fallback
test. The full suite changed from `1300 passed / 74 failed / 6 ignored` to
`1313 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 41,480-byte diff with SHA-256
`BC1451E1A298A9D41EA7B77E3E618A764672CF73B5E71CE9ACF379BF0D731F59`.
Production contains no positive owner, tabular section, Attribute, TypeId,
qualified name, database identity, or path literal. The existing platform
DesignTimeRef TypeId remains a serialization discriminator.

The saved normalized isolated residual is one DataProcessor root at `+0/-1`.
The expected future permitted native gate is `+0/-0`, but exactness is not
claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## Catalog bounded default presentation

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The admitted model is local to the exact BSP Catalog source layout with raw
object code `56` and 61 fields. Among 114 such owners, all six records whose
strict unsigned-integer slot 19 is zero have native `DefaultPresentation` equal
to `AsCode`. Of the 108 records with a positive slot 19, 107 have native
`AsDescription`; the remaining `AsCode` record has value 150 and is an
unexplained HOLD. It is not encoded as an owner-name or UUID exception.

The full metadata-object header must occur exactly once at field 9. Wrong,
missing, duplicate, or malformed headers and malformed slot-19 values fail
closed. Exact code-56/length-61 records map zero to `AsCode` and a positive
value to `AsDescription`. Other shapes retain the previous
`AsDescription` behavior. In particular, all 489 exact code-57/length-61
Catalog controls remain unchanged, including 26 whose slot 19 is zero. The
mapping is therefore not a platform-global Catalog semantic and is not shared
with the independently encoded UT and SFC layouts.

Integrated commit `727d180` passes 12 positive, control, malformed-header,
wrong-family, and native-exception tests. The full suite changed from
`1313 passed / 74 failed / 6 ignored` to
`1325 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 12,637-byte diff with SHA-256
`2FA6986B9632F1AA28A299FF264F38740F4AD63545B0D1672ABCCB49E6423002`.
Production contains no Catalog name, object UUID, database identity, path,
field-43 branch, or exception table.

The saved normalized isolated residual is six Catalog roots totaling `+6/-6`.
The expected future permitted native gate is `+0/-0`, but exactness is not
claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## Catalog bounded input and history tail

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

This model reuses the exact code-56/61-field Catalog boundary and full owner
header validation used by the bounded default-presentation model. All 114 BSP
owners have exactly one matching fully parsed owner header at field 9. Within
that cohort, field 53 maps `1` to `CreateOnInput=DontUse` for 21 owners and `2`
to `CreateOnInput=Use` for 93. Field 57 maps `0` to
`ChoiceHistoryOnInput=Auto` for 110 owners and `1` to
`ChoiceHistoryOnInput=DontUse` for four. There are no exceptions.

The six observed `(field53, field57, legacy field51)` cells are
`(1,0,0)=1`, `(1,0,1)=18`, `(1,1,1)=2`, `(2,0,0)=11`, `(2,0,1)=80`, and
`(2,1,1)=2`. They prove 94 CreateOnInput changes and four
ChoiceHistoryOnInput changes with two overlapping owners: 96 roots and 98 line
replacements. The previous field 51 is not a valid discriminator: its zero
partition contains 11 native Use and one native DontUse value, while its one
partition contains 82 Use and 20 DontUse values.

Exact candidates accept only field-53 values `1|2` and field-57 values `0|1`.
Missing, empty, textual, negative, overflow, wrapped, or out-of-domain values
reject the whole source atomically. A malformed, moved, duplicate, or
wrong-owner header is likewise fatal. Other codes and arities retain the prior
field-51/field-52 expressions and defaults byte-for-byte. In particular, 489
exact code-57/61-field UT controls keep their legacy semantics even though
their field-53 distribution is 225/264 and their field-57 distribution is
452/37. Their complex field-52 envelopes remain fallback controls.

Integrated commit `6279413` passes seven joint-matrix, semantic-legacy,
nonexact-arity, malformed-slot, property-only, ordering, and literal tests.
The full suite changed from `1325 passed / 74 failed / 6 ignored` to
`1332 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 14,939-byte diff with SHA-256
`20058BEBE493303741DD78A94032800D32EDAF99CF257F937DE3F295E5F9EA0B`.
Production contains no Catalog name, object UUID, database identity, path, or
corpus-specific branch.

`CreateOnInput=Auto` for exact code 56, `DataHistory`, the two data-history
processing flags, fields 54, 56, and 58 through 60, and every code-57/global
remap remain HOLD. The saved normalized isolated residual is 96 Catalog roots
totaling `+98/-98`. The expected future permitted native gate is `+0/-0`, but
exactness is not claimed without that gate. No database access, ConfigDump,
configuration export, or `ConfigDumpInfo.xml` change was performed.

## AccumulationRegister bounded totals splitting

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The saved BSP corpus contains exactly two AccumulationRegister root streams
with object code `28`, 26 fields, and one owner-header marker at field 13. The
field-13 wrapper and header must parse completely and match the routed owner
UUID, name, synonyms, and comment. Field 20 is `1` for the native
`EnableTotalsSplitting=true` root and `0` for the native false root. Both
generated roots previously omitted the property.

The parser uses a three-state contract. An exact code-28/26-field candidate
with a malformed, moved, duplicate, partial, or wrong-owner header, or a field
20 value outside the strict `0|1` domain, rejects the whole source. A nonexact
AccumulationRegister layout returns a legacy omission, while a valid exact
candidate returns the parsed boolean. A plain UUID outside an owner-header
marker is not treated as another header. Code-28 arities 21, 25, and 27 remain
explicit omission controls.

Formatting is family-specific. AccumulationRegister emits the property after
`FullTextSearch`, so its order is `DataLockControlMode`, `FullTextSearch`, then
`EnableTotalsSplitting`. The two BSP AccountingRegister controls use code 21,
30 fields, and header 15; their header+7 field is zero while header+8 is one.
They retain their existing true value and the separate order
`DataLockControlMode`, `EnableTotalsSplitting`, `FullTextSearch` byte-for-byte.
A CalculationRegister code-21/33-field control has a UUID at header+7 and
continues to omit the property.

Native-only SFC evidence contains 252 AccumulationRegister roots: 209 true and
43 false, all with the same XML order and none without the property. Because
paired SFC raw data is unavailable, this breadth does not widen the admitted
raw layout beyond the exact BSP shape.

Integrated commit `a926e19` passes ten exact, malformed, marker, nonexact,
family-control, property-only, ordering, and literal tests plus the existing
code-28/21-field control. The full suite changed from
`1332 passed / 74 failed / 6 ignored` to
`1342 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 15,354-byte diff with SHA-256
`D590346CBEB45A669A576C98A7AEC5FD7CED64DE85ECA89E6333212DD83C83DA`.
Production contains no register name, owner UUID, database identity, path, or
corpus-specific branch.

Adjacent empty presentation and explanation properties, other code-28 raw
layouts, global register offsets, RegisterType, and child-object residuals
remain HOLD. The saved normalized isolated residual is two roots totaling
`+0/-2`. The expected future permitted native gate is `+0/-0`, but exactness is
not claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## AccumulationRegister bounded presentation tail

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

This model shares the exact AccumulationRegister code-28/26-field validator
with totals splitting. The routed owner marker must be unique at field 13, and
the wrapped header must parse completely and match the routed UUID, name,
synonyms, and comment. Both BSP roots store strict localized values in fields
23, 24, and 25; all three are `{0}`. Native emits mandatory self-closing
`ListPresentation`, `ExtendedListPresentation`, and `Explanation` nodes, while
the previous generated roots omitted them.

A saved UT/direct corpus supplies 102 exact raw/native pairs. All have valid
unique owner boundaries and strict localized tails. Fields 23 and 24 are empty
in all 102. Field 25 is empty in 98 and contains a Russian Explanation in four;
raw/native empty shape matches in every pair. Three nonempty texts match
byte-for-byte and one matches after CRLF/LF normalization. Native includes
additional language items supplied by broader language/version data, so the
comparison claims only raw-present language pairs.

Native-only SFC breadth contains 252 AccumulationRegister roots. Every root has
all three nodes in the order `EnableTotalsSplitting`, `ListPresentation`,
`ExtendedListPresentation`, `Explanation`. List and Extended List are empty in
251 roots and localized in one; Explanation is empty in 245 and localized in
seven. There is no paired raw nonempty field-23 or field-24 sample, so their
nonempty corpus semantics remain a structural inference rather than a direct
evidence claim.

The presentation parser explicitly calls the same full exact-layout validator
as totals splitting; its safety does not depend on call order. Exact fields
23, 24, and 25 use the strict counted localized-value parser. A malformed
count, pair, atom, duplicate language, trailing field, invalid XML character,
or invalid owner boundary rejects the whole source atomically. Nonexact
code-28 arities 21, 25, and 27 retain omission and do not consume slot-like
values. A separate emission flag preserves the three mandatory nodes when the
parsed vectors are empty.

Formatting adds the trio only in the AccumulationRegister branch, immediately
after totals splitting. AccountingRegister retains its distinct fields
26/27/28 and order after FullTextSearch. CalculationRegister code-21/33-field
roots use another tail layout and remain an explicit omission control.

Integrated commit `915a427` passes eight strict-tail, boundary, mandatory-empty,
paired-style, nonexact, property-only, and literal tests plus all ten totals
splitting regressions. The full suite changed from
`1342 passed / 74 failed / 6 ignored` to
`1350 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 19,726-byte diff with SHA-256
`5DD4E4ECD7112FEAA7766D2E187BF67E8F145A352A2B59C5C1595C8CE0FF030D`.
Production contains no register name, owner UUID, database identity, path, or
corpus-specific branch.

CalculationRegister presentations, alternate code-28 layouts, global register
offsets, and a direct corpus claim for nonempty fields 23 and 24 remain HOLD.
The saved normalized isolated residual is two roots totaling `+0/-6`. The
expected future permitted native gate is `+0/-0`, but exactness is not claimed
without that gate. No database access, ConfigDump, configuration export, or
`ConfigDumpInfo.xml` change was performed.

## CalculationRegister bounded empty presentation tail

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The complete BSP raw corpus contains one CalculationRegister root with object
code `21`, 33 fields, and a unique fully parsed routed owner header at field 15.
Its final fields 30, 31, and 32 are exactly `{0}`. Native emits mandatory
self-closing `ListPresentation`, `ExtendedListPresentation`, and `Explanation`
nodes, while the previous generated root omitted all three. Their order follows
`DataLockControlMode` and `FullTextSearch`; CalculationRegister does not emit
`EnableTotalsSplitting`.

The saved UT/direct corpus contains 20,067 primary roots and no top-level
code-21 owner root, so it supplies no paired CalculationRegister breadth.
Native-only SFC has two CalculationRegister roots; both contain all three empty
nodes in the same order. This confirms native presence and order but does not
prove nonempty raw semantics.

The admitted model is therefore deliberately empty-only. A shared
parameterized wrapped-owner validator checks code `21`, arity 33, unique routed
marker at field 15, and complete UUID/name/synonym/comment identity. Exact
fields 30 through 32 must each equal `{0}` after outer whitespace trimming.
Malformed values and valid nonempty localized values reject the whole exact
source atomically. Arity-32 and arity-34 controls preserve legacy omission and
do not consume tail-shaped values.

The same raw code is not a global discriminator. AccountingRegister uses
code 21 with 30 fields, presentation fields 26 through 28, and a distinct order
that includes totals splitting before full-text search. AccumulationRegister
uses code 28 with 26 fields and presentation fields 23 through 25. Both
families retain their existing parsing and formatting.

Integrated commit `4ac0aed` passes six empty-tail, boundary, nonempty/malformed,
nonexact, property-only, and literal tests, plus all eight AccumulationRegister
presentation and ten totals-splitting regressions. The full suite changed from
`1350 passed / 74 failed / 6 ignored` to
`1356 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Independent frozen review approved the 16,169-byte diff with SHA-256
`3860BA29A0EF0A9AC3E19E5B33576DD3D95687CA55030B75C3F9141A868C8AB7`.
Production contains no register name, owner UUID, database identity, path, or
corpus-specific branch.

Nonempty CalculationRegister presentation semantics, alternate arities and
layouts, and global register offsets remain HOLD. The saved normalized isolated
residual is one root totaling `+0/-3`. The expected future permitted native gate
is `+0/-0`, but exactness is not claimed without that gate. No database access,
ConfigDump, configuration export, or `ConfigDumpInfo.xml` change was performed.

## CalculationRegister bounded period vector

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The sole paired BSP CalculationRegister root uses the exact code-21/33-field
owner layout and stores fields 16, 17, and 18 as `2/1/1`. Native emits
`Periodicity=Month`, `ActionPeriod=true`, and `BasePeriod=true`; the previous
generated root omitted the trio. Schema order places it after default and
auxiliary list forms and before Schedule. On the currently supported surface it
appears after `UseStandardCommands` and before the bounded presentation tail.

Native-only SFC contains two CalculationRegister roots. Both use Month and
`BasePeriod=true`, while one has `ActionPeriod=true` and the other false. The
saved UT/direct corpus has no top-level code-21 root, so the false raw encoding
is not paired. The admitted parser therefore recognizes only the exact fixed
vector `2/1/1`. Every other structurally valid scalar vector, including
`2/0/1`, remains an accepted legacy omission rather than a source error.

The parser independently calls the shared full code-21/33-field/header-15
validator and returns an optional fixed state. A malformed owner boundary still
rejects the exact source atomically. Alternative scalar values and nonexact
arities return no period state and are not consumed. Field 16 remains visible
to the pre-existing generic `UseStandardCommands` parser; tests distinguish
that neighboring behavior from the new fixed-vector state.

Formatting emits the three fixed values only in the CalculationRegister branch.
AccountingRegister roots with code 21 and 30 fields, and AccumulationRegister
roots with code 28 and 26 fields, retain their independent behavior and order.

Integrated commit `e975719` passes six fixed-vector, alternative-omission,
boundary, nonexact, property-only, and literal tests, plus the six Calculation
presentation, eight Accumulation presentation, and ten totals-splitting
regressions. The full suite changed from
`1356 passed / 74 failed / 6 ignored` to
`1362 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Independent frozen review approved the 12,732-byte diff with SHA-256
`4CF630ECEEAA2C727B11F7CB809165871F5F12781B88FA7342C4AA0A241AB367`.
Production contains no register name, owner UUID, database identity, path, or
corpus-specific branch.

The raw mapping for `ActionPeriod=false`, other Periodicity and BasePeriod
values, default and auxiliary forms, Schedule, Calculation DataLockControlMode
and FullTextSearch, alternate layouts, and global offsets remain HOLD. The saved
normalized isolated residual is one root totaling `+0/-3`. The expected future
permitted native gate is `+0/-0`, but exactness is not claimed without that gate.
No database access, ConfigDump, configuration export, or `ConfigDumpInfo.xml`
change was performed.

## CalculationRegister bounded include-help flag

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

In the exact code-21/33-field CalculationRegister owner layout, absolute field
25 is zero. Native emits `IncludeHelpInContents=false`; the previous generated
root omitted the property. Native-only SFC has two CalculationRegister roots
and both emit false. The saved UT/direct corpus has no code-21 owner root and
therefore supplies no paired true alternative.

The parser independently invokes the shared full header-15 owner validator.
Only field 25 equal to `0` after outer whitespace trimming returns an explicit
false value. Field 25 equal to `1`, other numeric or textual atoms, braced
values, UUIDs, and every other alternative remain accepted legacy omissions;
the implementation neither infers true nor rejects the source. A malformed
exact owner boundary remains atomic failure, while nonexact arities do not
consume field 25.

Fields 26 and 27 are intentionally outside this model. Exhaustive mutation
tests keep their XML byte-identical, preserving the unresolved Calculation
DataLockControlMode and FullTextSearch polarity. Formatting reuses the existing
optional include-help position after the fixed period vector and before the
bounded presentation tail.

The same raw code is not a global discriminator. AccountingRegister uses a
30-field layout whose include-help field is 17 and whose paired value is true.
AccumulationRegister uses code 28 and retains its separately proven false
value. Both family controls remain unchanged.

Integrated commit `71438cf` passes eight exact, alternative-omission, neighbor,
boundary, nonexact, family-control, property-only, and literal tests, plus all
30 Calculation and Accumulation regressions from issues 77 through 80. The full
suite changed from `1362 passed / 74 failed / 6 ignored` to
`1370 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 10,867-byte diff with SHA-256
`257FB8AF33681BCE7B99733D296DA4DB15E61CBC64AFDE6D0BC4DA466C7633DB`.
Production contains no register name, owner UUID, database identity, path, or
corpus-specific branch.

The raw mapping for Calculation include-help true, fields 26 and 27, forms,
Schedule and Chart references, StandardAttributes, alternate layouts, and
global offsets remain HOLD. The saved normalized isolated residual is one root
totaling `+0/-1`. The expected future permitted native gate is `+0/-0`, but
exactness is not claimed without that gate. No database access, ConfigDump,
configuration export, or `ConfigDumpInfo.xml` change was performed.

## CalculationRegister bounded owner form pair

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The exact code-21/33-field CalculationRegister root stores a nonzero form UUID
in field 23 and the zero UUID in field 29. The nonzero UUID resolves to its
actual same-owner Form source. Native emits the qualified `DefaultListForm`
followed by mandatory empty `AuxiliaryListForm`; the previous generated root
omitted both. Native-only SFC has two CalculationRegister roots whose two form
nodes are mandatory and empty, but paired SFC raw is unavailable and therefore
does not widen the admitted nonzero form case.

The dedicated resolver deliberately bypasses the existing preferred-name
fallback and unchecked generic form resolver. It strictly parses field 23 as a
nonzero UUID, requires exactly one case-insensitive key match, verifies kind
`Form`, lowercase `.xml`, exact path
`CalculationRegisters/<owner>/Forms/<name>.xml`, and a single-segment qualified
name `CalculationRegister.<owner>.Form.<name>`. Field 29 must be the zero UUID.

Zero, unknown, cross-owner, CommonForm, wrong-kind, invalid-extension, nested
path, multi-segment name, case-collision, and nonzero or malformed auxiliary
alternatives all remain accepted omissions. A malformed exact owner boundary
still rejects atomically through the shared validator. An explicit form-pair
state distinguishes mandatory empty Auxiliary from unsupported omission.

The `form_refs` map can detect case-variant key collisions but cannot recover
exact duplicate UUID rows already collapsed during index construction. The
saved BSP candidate has one matching source row; no global collision-freedom
claim is made. Formatting emits the pair only in CalculationRegister, after
`UseStandardCommands` and before the fixed period vector. Accounting and
Accumulation form routing remain unchanged.

Integrated commit `0613cf9` passes eight exact, resolver-alternative, auxiliary,
boundary, nonexact, family-control, property-only, and literal tests, plus 39
issue-77-through-81 and existing form regressions. The full suite changed from
`1370 passed / 74 failed / 6 ignored` to
`1378 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 23,461-byte diff with SHA-256
`1BB52202826B88D87B3EBBF05E509A140DE316059DE7B49402B7127FCB60C02E`.
Production contains no register name, form UUID, database identity, path, or
corpus-specific branch.

Field-23 zero/default-empty semantics, exact duplicate UUID rows collapsed by
the current index, Schedule and Chart references, StandardAttributes,
DataLockControlMode and FullTextSearch, alternate layouts, and global offsets
remain HOLD. The saved normalized isolated residual is one root totaling
`+0/-2`. The expected future permitted native gate is `+0/-0`, but exactness is
not claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## CalculationRegister bounded schedule tuple

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

In the exact code-21/33-field CalculationRegister owner layout, fields 19
through 22 contain four nonzero UUIDs. They resolve respectively to an
InformationRegister root, a Resource and Dimension of that same register, and
a ChartOfCalculationTypes root. Native emits the qualified `Schedule`,
`ScheduleValue`, `ScheduleDate`, and `ChartOfCalculationTypes` properties as
one ordered tuple; the previous generated root omitted all four.

The parser independently invokes the shared full header-15 owner validator and
then resolves the tuple atomically. Every UUID must be strict and nonzero,
every case-insensitive reference match must be unique, the resource and
dimension must have the same InformationRegister owner, and all qualified names
must have the exact expected family and segment count. Zero, malformed,
partial, unknown, cross-owner, wrong-family, wrong-role, and case-collision
alternatives remain accepted omissions of the entire tuple. Only a malformed
exact owner boundary remains fatal.

Formatting emits the tuple only in CalculationRegister, after the fixed period
properties and before `IncludeHelpInContents`. AccountingRegister code 21 with
30 fields and AccumulationRegister code 28 retain their independent behavior.
Selected CalculationRegister extraction now requests the object-reference
index needed by this resolver.

Integrated commit `a958f08` passes eight exact, resolver-alternative, partial,
boundary, nonexact, family-control, property-only, and literal tests plus the
selected-index regression. The full suite changed from
`1378 passed / 74 failed / 6 ignored` to
`1387 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 27,297-byte diff with SHA-256
`2FBAF59D8A866B4579E5B9E757D76BD52D0327772616F75BB4953F750069E469`.
Production contains no register name, UUID, database identity, path, or
corpus-specific branch.

Empty or chart-only schedule semantics, exact duplicate UUID rows collapsed by
the current index, StandardAttributes, DataLockControlMode and FullTextSearch,
alternate layouts, and global offsets remain HOLD. The saved normalized
isolated residual is one root totaling `+0/-4`. The expected future permitted
native gate is `+0/-0`, but exactness is not claimed without that gate. No
database access, ConfigDump, configuration export, or `ConfigDumpInfo.xml`
change was performed.

## CalculationRegister bounded full-text-search flag

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

In the exact code-21/33-field CalculationRegister owner layout, absolute field
27 is zero and native emits `FullTextSearch=DontUse`. The previous generated
root omitted the property. Both native-only SFC CalculationRegister roots also
emit `DontUse`, while their DataLockControlMode differs from the paired BSP
root, confirming that the neighboring properties must remain independent.

The parser independently invokes the shared full header-15 owner validator.
Only field 27 equal to `0` after outer whitespace trimming returns the explicit
`DontUse` value. Field 27 equal to `1`, other numeric or textual atoms, braced
values, UUIDs, and every other alternative remain accepted omissions; the
implementation neither infers `Use` nor rejects the source. Nonexact arities do
not consume field 27, while a malformed exact owner boundary remains fatal.

Fields 26 and 28 are deliberately not inspected. Calculation
DataLockControlMode and the complex 11-item StandardAttributes collection remain
HOLD. AccountingRegister code 21 with 30 fields and AccumulationRegister code
28 retain their independent parsers. Formatting reuses the established
FullTextSearch position before the presentation tail.

Integrated commit `923a2e9` passes eight exact, alternative-omission, neighbor,
boundary, nonexact, family-control, property-only, and literal tests. The full
suite changed from `1387 passed / 74 failed / 6 ignored` to
`1395 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 10,913-byte diff with SHA-256
`DB4F71C16E66373842A0F5E44EEF3EC7108EA15534117306AE95375090EED868`.
Production contains no register name, UUID, database identity, path, or
corpus-specific branch.

Calculation FullTextSearch values other than the paired zero,
DataLockControlMode, StandardAttributes, alternate layouts, and global offsets
remain HOLD. The saved normalized isolated residual is one root totaling
`+0/-1`. The expected future permitted native gate is `+0/-0`, but exactness is
not claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## CalculationRegister exact generated-type vector

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The exact code-21/33-field CalculationRegister owner layout has its wrapped
header at field 15 and fourteen UUIDs in fields 1 through 14. Native consumes
them as seven ordered TypeId/ValueId pairs: Record, Manager, Selection, List,
RecordSet, RecordKey, and RecalculationsManager. The previous generated XML
used `CalculationRegisterObject` with category `Object` for the first pair and
omitted the final `RecalculationsManager` entry. Its raw type index repeated the
same wrong first qualified name and omitted the seventh TypeId.

One structural schema now feeds both XML entries and index entries. Exact
admission requires the shared full header-15 validator plus fourteen strict,
nonzero, case-insensitively unique UUIDs. Outer whitespace and UUID case are
accepted. Any malformed, zero, duplicate, or case-duplicate slot in an exact
layout rejects source extraction atomically and contributes no indexed types
for that row. ValueIds participate in validation but are not index keys; all
seven TypeIds retain the `Type` DCS policy.

Nonexact Calculation layouts deliberately retain the preexisting permissive
six-role `Object` through `RecordKey` XML and index behavior. Accounting code
21 and code 22, and Accumulation code 28, retain their own role order and index
semantics. Cross-row TypeId collisions continue through the established
collision policy and do not affect owner validity. Selected Calculation
extraction already requested the type index and now exposes the corrected
Record and RecalculationsManager references.

Integrated commit `47763b0` passes nine exact XML/index, case/whitespace,
invalid-vector, duplicate/header, legacy-layout, family-control, collision,
selected-index, target, and literal tests. The full suite changed from
`1395 passed / 74 failed / 6 ignored` to
`1404 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Two independent frozen reviews approved the 20,312-byte diff with SHA-256
`1CB8E03334C8165E504C0ACB206CF2854FAD8780B607F0230F7F3E1F656C0321`.
Production contains no generated UUID, owner name, database identity, path, or
corpus-specific branch; UUIDs are read only from the raw vector.

Seven-role inference for nonexact layouts, alternate role orders and offsets,
global collision-policy changes, other Calculation properties, and the native
re-export gate remain HOLD. The saved normalized isolated residual is one root
totaling `+1/-5`. The expected future permitted native gate is `+0/-0`, but
exactness is not claimed without that gate. No database access, ConfigDump,
configuration export, or `ConfigDumpInfo.xml` change was performed.

## CalculationRegister exact recalculation child references

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

The exact code-21/33-field CalculationRegister owner layout declares
Recalculation children in a top-level protocol collection after the owner
object. The paired BSP owner declares one UUID whose separate code-4 row is a
structurally valid Recalculation already routed to that same owner. Native emits
its leaf name as one `Recalculation` child after register data children and
before Forms; the previous generated root omitted the line. Both native-only
SFC Calculation roots declare no Recalculation children.

The root collection parser is separate from the existing provisional child
routing index. It first requires the shared exact owner validator, then requires
one top-level collection with an unsigned exact count, strict nonzero and
case-insensitively unique UUID members, and no trailing values. Missing,
multiple, malformed, nested, quoted, zero, or duplicate alternatives remain
accepted whole-cohort omissions. Nonexact owner layouts do not consume the
collection.

A dedicated validated sidecar resolves every declared member atomically. Each
UUID must have exactly one matching metadata row with code 4, a matching header,
and a structurally valid Recalculation body. Multi-owner declarations, duplicate
owner rows, unresolved or CommonPicture-shaped code-4 rows, and sanitized path
collisions omit the entire owner cohort. Path collision keys use Unicode-aware
lowercasing. The provisional index remains unchanged so standalone child
routing keeps its previous behavior.

Formatting preserves raw declaration order, escapes only validated leaf names,
and emits no partial list. Selected owner extraction supplies the declared child
row through its existing direct-reference path; an owner without that evidence
omits the root line. Accounting and Accumulation families remain unchanged.
The collection marker UUID is a platform serialization discriminator already
present in the format model, not an object UUID from the paired database.

Integrated commit `78d4a26` passes seven strict collection, structural child,
collision, exact/nonexact, selected, family-control, target, and literal tests,
plus all ten related recalculation tests. The full suite changed from
`1404 passed / 74 failed / 6 ignored` to
`1411 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Independent frozen review approved the 39,550-byte diff with SHA-256
`5E1A95D218B2E3BAFD63E5F31080AEF4ECEA5AA60C70E3AB396B048A157946FE`.
Production contains no owner or child UUID, metadata name, database identity,
path, or corpus-specific branch.

Resource/Dimension tag correction and child properties, Calculation F26/F28,
nonexact layouts, alternate collection formats, and the native re-export gate
remain HOLD. The saved normalized isolated residual is one root totaling
`+0/-1`. The expected future permitted native gate is `+0/-0`, but exactness is
not claimed without that gate. No database access, ConfigDump, configuration
export, or `ConfigDumpInfo.xml` change was performed.

## Form exact empty nested AutoCommandBar lexical form

Status: implemented from saved raw/native evidence; native re-export remains
paused by user instruction.

Across 1,108 saved BSP Form.xml files, native has 224 self-closing and 527
open nested AutoCommandBar nodes. Generated output represented 223 of those
self-closing nodes as empty open/close pairs; the remaining native positive is
an independently missing upstream item and is outside this change. Top-level
AutoCommandBar output was already exact at 306 self-closing and 802 open nodes.

A dedicated FormChildItem flag is derived only from the exact raw grammar:
wrapper 22, item type 9, non-top-level id, exactly 29 fields, and field 20
structurally equal to exactly `{0,0,1}`. The field parser must consume the
entire trimmed value, so appended braced values or text cannot make a malformed
marker pass. Wrong wrapper/type/id/arity, all six observed horizontal-alignment
and autofill variants, and child-bearing command bars remain open. Formatting
does not infer lexical emptiness from the parsed property model and requires
both the AutoCommandBar tag and the raw-derived flag.

The nested formatter emits one escaped self-closing line for that exact flag.
ContextMenu, top-level bars, nonempty bars, packer row preservation, and service
child ordering remain unchanged:
`ContextMenu < AutoCommandBar < ExtendedTooltip < additions < Events < regular ChildItems`.

Integrated commit `642626d` adds seven tests and passes the exact nested filter
5/5. The full suite changed from `1411 passed / 74 failed / 6 ignored` to
`1418 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Independent frozen reviews approved the 15,912-byte canonical diff with SHA-256
`05673CB8C403BC3EB774108C3E6BE3078E8C1FF8A68F88528744C069102610F3`.
The implementation diff is three files totaling `+301/-0`, and production adds
no object UUID, metadata name, database identity, path, or corpus-specific
branch.

After explicit user authorization, a fresh release export produced 12,197
files and the normalized full diff `1529 files, +14239/-85386`. This improves
the overwritten pre-gate tree by 30 files and 8,253 changed lines. The current
AutoCommandBar line residual is three files totaling `+1/-4`; one missing
upstream item, HorizontalAlign, surrounding Form omissions, ExcludedCommand,
FileDragMode, and other AutoCommandBar layouts/properties remain HOLD.
ConfigDumpInfo remains byte-exact with SHA-256
`F187FA4F131F9C5DCBD2E41FE630585B1D6C74FB2809D62F4B3B3F0563425A2F`.

## Form exact field ToolTipRepresentation enum

Status: implemented from immutable raw/native evidence and checked by a fresh
full export after explicit user authorization.

Across 1,108 native Form.xml files, 7,193 wrapper-37 field items map to raw
records without an unmatched item. Field 50 is zero and the property is absent
for 6,825/6,825 controls. All 368 nonzero values match the native enum exactly:
None 16, Balloon 4, Button 205, ShowAuto 2, ShowTop 2, ShowBottom 133, and
ShowRight 6. An independent 446-form raw subset confirms 2,813 zero controls
and 135 nonzero positives with no contradiction.

The exact parser requires wrapper 37, exactly 59 top-level fields, a direct
field-5 kind/tag pair for LabelField, InputField, CheckBoxField, PictureField,
RadioButtonField, or CalendarField, and one scalar field-50 value. Zero omits
the property; values 1, 2, 3, 4, 5, 7, and 8 map to the seven native enum names.
Unknown, quoted, braced, prefixed, suffixed, shifted-discriminator, wrapper-48,
58-field, and 60-field alternatives remain accepted omissions.

The previous InputField heuristic inferred Button from InputHint or
ListChoiceMode, creating 34 false additions. A separate Calendar branch also
depended on nonempty parsed tooltip text. Both are removed. Formatting emits
the raw-derived property for only the six admitted tags, after the existing
localized ToolTip position and before later field properties. Localized ToolTip
serialization remains limited to its previous InputField, PictureField, and
CalendarField scope.

Integrated commit `202a5c5` passes eight exact enum, tag, ordering, zero,
inference, empty-tooltip, malformed-layout, target, and literal tests plus the
Calendar and document-field regressions. The full suite changed from
`1418 passed / 74 failed / 6 ignored` to
`1426 passed / 74 failed / 6 ignored`; exact failure-name delta is zero.
Independent immutable review approved the 13,466-byte canonical diff with
SHA-256
`9740DDDEC14FEDB389B5B42B240B37EB88089C3417157D5CE2ACABDB1C27F614`.
The implementation changes two files totaling `+316/-18`; production adds no
object UUID, metadata name, database identity, path, or corpus-specific branch.

The fresh release export produced 12,197 files and the normalized full diff
`1529 files, +14239/-85386`. ToolTipRepresentation still has a line residual
of 131 files totaling `+60/-444`: Balloon `+1/-7`, Button `+36/-142`, None
`+2/-46`, ShowBottom `+21/-212`, ShowLeft `+0/-2`, ShowRight `+0/-31`, and
ShowTop `+0/-4`. Wrapper-48/len-60 semantics, property ordering, localized
ToolTip expansion, and other form layouts therefore remain HOLD; the issue
stays open. ConfigDumpInfo remains byte-exact. No test command was run for this
gate, following the direct user instruction.

## Form UsualGroup default Representation

Status: integrated from a comprehensive native/generated schema matrix; a new
export was not run.

`Representation=WeakSeparation` is a parsed `UsualGroup` value but is also the
native XML default for that owner kind. The parser must preserve the enum value
for the in-memory form model, while the XML formatter must omit the property.
This default does not depend on `Group`, `Behavior`, `ShowTitle`, an object or
form name, a UUID, or the metadata family that owns the form.

The comprehensive path model is every managed-form body matching
`bsp/**/Ext/Form.xml`, including object Forms and CommonForms. The current tree
contains 1,008 changed Form.xml files totaling `+9001/-29143`. It has exactly
274 emitted `UsualGroup/Representation=WeakSeparation` nodes in 127 files;
all 274 match native items where Representation is absent. The native baseline
contains zero positive nodes across all 1,108 Form.xml files. The earlier
object-Forms-only slice was 246 nodes in 113 files; CommonForms contribute the
remaining 28 nodes in 14 files.

`form_schema.rs` now owns typed child-item kind and representation default
semantics. `form_body.rs` keeps parsing unchanged and asks that schema predicate
at the existing formatter position. The rule is fail-closed: only the pair
`UsualGroup + WeakSeparation` is omitted; every other owner/value pair is still
emitted, so neighboring XML order is unchanged.

Integrated commit `b681153` changes two files totaling `+45/-4`.
`cargo fmt --check` and `cargo check --all-targets` pass with two pre-existing
warnings. Tests, export, database access, ConfigDump, and ConfigDumpInfo were
not run or changed. Independent frozen review returned GO and found no object
name, path, DB UUID, arity, or corpus branch. A later exact shadow gate corrected
the file-count forecast: three Form.xml files contain no other difference.
The exact #89 prediction is Form `1005 files, +8727/-29143` and full
`1526 files, +13965/-85386`; the last actual full diff remains
`1529 files, +14239/-85386` until another export is explicitly authorized.

## Form exact Button ToolTipRepresentation layout

Status: integrated from full and immutable-subset raw/native matrices; a new
export was not run.

ToolTipRepresentation is one property family with layout-specific raw schemas
and XML positions. The shared scalar decoder remains the accepted #88 enum:
zero omits the property, while 1, 2, 3, 4, 5, 7, and 8 map to None, Balloon,
Button, ShowAuto, ShowTop, ShowBottom, and ShowRight. Code 6 remains HOLD and
malformed, quoted, braced, suffixed, or unknown values omit the property.

The exact Button layout is wrapper 31 with exactly 52 top-level fields and the
enum at slot 30. Across all 1,108 forms, every one of 5,636 such records agrees
with native XML: 5,467 zero/absent controls and 169 positives consisting of
None 7, Balloon 4, Button 26, ShowBottom 121, and ShowRight 11. An immutable
446-form slice independently reproduces 1,990 zero controls and 36 positives
with counts 3, 2, 8, 22, and 1. Wrapper-31 length-53 is a decisive negative
control: all 11 native records omit the property even though seven have raw
slot-30 value 2. Wrapper 34, length 53, other owners, and other layouts remain
HOLD.

Native order is exact for 169/169 nodes:
`CommandName < optional Title < ToolTipRepresentation < optional
ShapeRepresentation/RepresentationInContextMenu < ExtendedTooltip`.
`form_schema.rs` now owns the typed layout slot, shared enum decoder, and XML
order class. The parser asks the schema for a slot; the formatter emits the
property at the schema-selected field or post-Title position. The previously
accepted wrapper-37/length-59 six-field schema remains semantically unchanged.

Integrated commit `37eca68` changes two files totaling `+169/-43`.
`cargo fmt --check` and `cargo check --all-targets` pass with the same two
pre-existing warnings. Independent frozen review returned GO. Tests, export,
database access, ConfigDump, and ConfigDumpInfo were not run or changed; no
object name, path, UUID, or corpus branch was added. The exact static #90
target is 41 Form.xml files totaling `+0/-169`; combined with #89, predicted
Form diff is `1005 files, +8727/-28974` and predicted full diff is
`1526 files, +13965/-85217`. The last actual full diff remains
`1529 files, +14239/-85386` pending explicit authorization for another export.

## Form UsualGroup header property order

Status: integrated from an owner-wide property-order matrix and exact raw
layouts; a new export was not run.

Native contains 3,878 UsualGroup nodes across 1,108 forms. Current output has
3,848 nodes, all matched to native by form path and item id; 30 native-only
groups remain an upstream HOLD. The audit covers 38 direct property tags and
all 703 co-occurring unordered pairs. No pair has mixed or reverse native
ordering, which proves one owner-level order instead of per-form branches.

The implemented boundary is the header prefix:

```text
Title < TitleTextColor < TitleFont < ToolTip < ToolTipRepresentation < Width
```

`FormUsualGroupHeaderXmlProperty` and
`FORM_USUAL_GROUP_HEADER_XML_ORDER` own the first five properties. The formatter
emits that prefix once immediately before the existing generic Width position.
The old styled and unstyled Title branches are removed, and UsualGroup is
excluded from the late ToolTip position. Width, Height, stretch, alignment,
Group, Behavior, Representation, visual tails, ExtendedTooltip, and ChildItems
remain in their existing serializers or explicit HOLD cohorts.

The current wrong-order union contains 280 nodes in 86 files. Native has
`Title < Width` in 171/171 applicable groups while current reverses all 171.
All 116 parsed ToolTip blocks are value-exact but late. The two order cohorts
overlap in seven nodes. Reordering existing blocks alone improves the saved
native diff by 571 additions and 571 deletions.

ToolTipRepresentation uses only four explicit wrapper-22, discriminator-5
layouts: arity 30 slot 23, arity 32 slot 25, arity 34 slot 27, and arity 36
slot 29. The full matrix contains 3,425 zero/absent controls and 68 values
admitted by the unchanged shared decoder. A single raw code-6/native ShowLeft
positive remains HOLD, as do arities 38 and above; no `len - 7` formula is
used. The prior #88 field and #90 Button layouts and the #89 default filter are
unchanged.

Integrated commit `e954e72` changes two files totaling `+79/-33`.
`cargo fmt --check`, `cargo check --all-targets`, and `git diff --check` pass;
the same two pre-existing warnings remain. Independent frozen review returned
GO. Tests, export, database access, ConfigDump, and ConfigDumpInfo were not run
or changed. The frozen 8,473-byte diff has SHA-256
`ED670891B89A3AFD1738EB4629B534FE8CD74981AC2E3610461B74912586088B` and adds
no UUID, object name, path, corpus branch, or code-6 mapping.

An exact flattened shadow gate reproduces the last actual Form diff
`1008 files, +9001/-29143`. It predicts #89+#90 as
`1005 files, +8727/-28974` and #91 as `1005 files, +8156/-28335`.
The #91 pre/post delta is 94 files totaling `+639/-571`: 571 reordered lines
plus 68 admitted TTR lines. The corrected combined full prediction is
`1526 files, +13394/-84578`. The last actual full diff remains
`1529 files, +14239/-85386` pending a separately authorized export.

## Form PictureDecoration tooltip header

Status: integrated from complete full/subset raw and native matrices; a new
export was not run.

All 769 native PictureDecoration records have one exact raw layout: wrapper 12,
direct discriminator 1, and exactly 36 top-level fields. One typed
`FormDecorationHeaderSchema` owns both localized ToolTip slot 8 and
ToolTipRepresentation slot 24. This prevents either property from being parsed
through a wrapper-only rule.

ToolTip is nonempty in 34 records and exactly `{1,0}` in 735; native has the
same 34 localized values and 735 absences, with semantic equality 769/769 and
no malformed values. The immutable 446-form subset independently matches all
203 records: four nonempty and 199 empty. Discriminator-0/length-36 provides a
decisive negative control with 63 nonempty slot-8 values that must not be
treated as PictureDecoration ToolTip.

ToolTipRepresentation has 748 zero/absent controls and 21 exact positives:
ShowRight 19, None 1, and ShowBottom 1. The immutable subset reproduces 197
zeros and six ShowRight positives. Discriminator-0/length-36 contains 86
code-like slot-24 values plus one code 6, so the discriminator is mandatory.
The exact target has no alternate arity and no code-6 value; ShowLeft and all
other layouts remain HOLD.

Across 769 nodes and all 378 co-occurring direct-property pairs, native has no
mixed or reverse order. The typed decoration header is:

```text
Title < ToolTip < ToolTipRepresentation < Picture
```

`FORM_DECORATION_HEADER_XML_ORDER` owns Title, ToolTip, and TTR. A
PictureDecoration-only helper replaces the prior Title call at the existing
pre-Picture position, before PictureSize, Picture, and FileDragMode. The generic
late ToolTip path excludes PictureDecoration. The existing AfterTitle position
is deliberately not reused because it follows the Picture block. LabelDecoration,
one missing PictureDecoration item, 39 late inline-file Picture nodes, and all
other style/default/property gaps remain separate HOLD cohorts.

Integrated commit `4613681` changes two files totaling `+115/-7`.
`cargo fmt --check`, `cargo check --all-targets`, and `git diff --check` pass;
the same two pre-existing warnings remain. Independent frozen review returned
GO. Tests, export, database access, ConfigDump, and ConfigDumpInfo were not run
or changed. The frozen 8,182-byte diff has SHA-256
`601363033B40550512F8F1EC2535C5D59BD05AB08936BCC1BB29580A12B80F1A` and adds
no UUID, object name, path, corpus branch, or code-6 mapping. Existing #88-#91
schemas remain semantically unchanged.

The exact combined shadow after #89-#91 changes 22 files by `+229/-0`: 208
localized ToolTip lines and 21 TTR lines. It predicts Form
`1005 files, +8156/-28106` and full `1526 files, +13394/-84349`. No path becomes
exact or newly different. The last actual full diff remains
`1529 files, +14239/-85386` pending a separately authorized export.

## Form LabelDecoration tooltip header

Status: integrated for the already recognized, title-bearing owner cohort; a
new export was not run.

The exact LabelDecoration raw owner layout is wrapper 12, direct discriminator
0, and exactly 36 top-level fields. The full corpus contains 1,866 such owners:
1,513 have a nonempty localized slot 7 and 353 do not. The immutable subset
independently contains 596 owners split 492/104. Current traversal reaches and
emits 1,512 title-bearing owners; one further owner is below an unsupported
parent layout and has no ToolTip or TTR, so #93 does not widen item admission.

For the bounded title-bearing cohort, `FormDecorationHeaderSchema` owns ToolTip
slot 8 and ToolTipRepresentation slot 24. ToolTip is present in 63 owners and
absent in 1,450; the immutable subset reproduces 13 positives and 479 controls.
All 353 title-empty owners have an empty ToolTip slot. TTR has 1,431 zero
controls and 82 nonzero values: None 4, Button 67, ShowBottom 10, and one
ShowLeft. The shared decoder admits the first 81 values. Raw code 6 and its
single ShowLeft observation remain HOLD and are not added to production.

Native direct-property order has no reverse pair or duplicate across all 1,866
owners. The typed header prefix is:

```text
Title < ToolTip < ToolTipRepresentation < GroupHorizontalAlign
```

The existing 21 GroupHorizontalAlign properties in 16 files previously appeared
too early. `FORM_DECORATION_HEADER_XML_ORDER` now owns the four-property order
for both LabelDecoration and PictureDecoration; PictureDecoration remains
semantically unchanged because that parser never assigns GroupHorizontalAlign.
LabelDecoration is excluded from the generic late ToolTip path, and its old
early GroupHorizontalAlign branch is removed, so each property is emitted once.

The broader structural cleanups remain explicitly rejected. Admitting all 353
title-empty LabelDecoration owners would expose many unrelated unsupported
properties and is a separate HOLD. Eleven of them have a native empty
`<Title/>`, which is not represented by the nonempty slot-7 predicate. Exact
wrapper-12/discriminator-0/length-34 is also not a sufficient ExtendedTooltip
classifier: the full corpus has 26,226 records but the existing suffix oracle
matches 26,225, while the immutable subset is 9,710 versus 9,709. The same one
non-oracle contradiction occurs in both corpora. Therefore
`is_form_extended_tooltip_name`, both name checks, and child-item classification
are deliberately unchanged by #93.

Integrated commit `0d36bff` changes two files totaling `+23/-7`. The frozen
4,818-byte diff has SHA-256
`7DFD208BD4D3B6D86863C1EDCD06CC10983F16BE605EB7623C7B8920683D4A2A`.
Independent frozen review returned GO. `cargo fmt --check`,
`cargo check --all-targets`, and `git diff --check` pass with the same two
pre-existing warnings. The change adds no object/form/item name, path, database
UUID, corpus branch, or code-6 mapping. Tests, export, database access,
ConfigDump, and ConfigDumpInfo were not run or changed.

The exact header-only shadow removes 21 insertion lines and 508 deletion lines:
21 moved GroupHorizontalAlign lines, 406 physical ToolTip lines, and 81 TTR
lines. It predicts Form `1005 files, +8135/-27598` and full
`1526 files, +13373/-83841`; file counts do not change. The last actual full
diff remains `1529 files, +14239/-85386` pending a separately authorized export.

## Form LabelDecoration alignment tail

Status: integrated for the same 1,512 reachable, title-bearing owners as the
LabelDecoration tooltip header; a new export was not run.

The alignment schema is guarded by the complete raw owner shape: wrapper 12,
direct discriminator 0, exactly 36 top-level fields, and a nine-field options
tuple at slot 18 whose kind is 5. It owns GroupHorizontalAlign slot 32,
GroupVerticalAlign slot 33, HorizontalAlign options slot 2, and VerticalAlign
options slot 3. No item name, path, UUID, database identity, or corpus branch is
part of this classifier.

The full/immutable matrices have zero raw-only, native-only, or value
contradictions. Existing GroupHorizontalAlign has Left 5/1, Center 8/2, Right
8/3, and omitted default 1,491/486. GroupVerticalAlign has Center 64/19,
Bottom 3/1, and omitted default 1,445/472. HorizontalAlign has Center 9/3,
Right 8/4, Auto 4/1, and omitted default 1,491/484. VerticalAlign has Top
78/26, Center 169/57, Bottom 27/8, and omitted default 1,238/401. The first
number in each pair is the complete 1,512-owner cohort and the second is the
immutable 492-owner subset.

Across the native owner-wide property matrix, all applicable pair reversals and
duplicates are zero. The typed boundary is:

```text
Title < ToolTip < ToolTipRepresentation < GroupHorizontalAlign
      < GroupVerticalAlign < Hyperlink < HorizontalAlign < VerticalAlign
```

The shared decoration header owns GroupVerticalAlign immediately after the
existing GroupHorizontalAlign position. A LabelDecoration-only typed tail owns
HorizontalAlign followed by VerticalAlign after the existing Hyperlink
position. LabelDecoration is excluded from the earlier generic HorizontalAlign
serializer, preventing both reordering and duplicate output. PictureDecoration
cannot receive the new alignment model, so its #92 behavior remains unchanged;
the #93 tooltip, TTR, and GroupHorizontalAlign semantics are also unchanged.

Integrated commit `02a4df8` changes three files totaling `+228/-16`. Eleven
insertions and three deletions in the existing test module are only the three
mechanical type adaptations required by `cargo check --all-targets`; they add no
test, assertion, or fixture. The frozen 14,854-byte diff has SHA-256
`0C30488CE96E1BC81DE7D65AAFF838CC6EDE1E6F54B1C45628F3300491F01CFB`.
Independent frozen review returned GO. `cargo fmt --check`,
`cargo check --all-targets`, and `git diff --check` pass with the same two
pre-existing warnings. Tests, export, database access, ConfigDump, and
ConfigDumpInfo were not run or changed.

The exact shadow adds 362 native scalar lines in 153 already changed Form.xml
files: 67 GroupVerticalAlign, 21 HorizontalAlign, and 274 VerticalAlign lines.
It predicts Form `1005 files, +8135/-27236` and full
`1526 files, +13373/-83479`; file counts and additions do not change. The last
actual full diff remains `1529 files, +14239/-85386` pending a separately
authorized export.

Title-empty and unreachable owners, unobserved GroupVerticalAlign code 0,
ToolTipRepresentation code 6, ExtendedTooltip, geometry, stretch, style,
visibility, enabled state, shortcuts, input skipping, and the outstanding
TextColor and AutoMaxWidth mismatches remain separate HOLD cohorts.

## Form LabelDecoration geometry block

Status: integrated for the same 1,512 reachable, title-bearing owners as the
LabelDecoration header and alignment gates; a new export was not run.

`FormLabelDecorationSchema` now owns both alignment and geometry under one
exact raw guard: wrapper 12, direct discriminator 0, 36 top-level fields, and a
nine-field options tuple of kind 5 at slot 18. No second layout predicate was
introduced. The complete 1,512-owner corpus and independent 492-owner subset
have zero raw-only, native-only, or value contradictions for every accepted
geometry property.

Width slot 10 is a nonzero unsigned integer and was already exact in 101/33
owners. Height slot 11 is present in 165/41 owners. HorizontalStretch slot 12
contains false 29/11, true 146/39, and omitted default 1,337/442;
VerticalStretch slot 13 contains false 17/7, true 68/28, and omitted default
1,427/457. AutoMaxWidth slot 27 contains false 985/325 and omitted default
527/167. MaxWidth slot 28 is present in 112/30 owners. AutoMaxHeight slot 30
contains false 35/9 and omitted default 1,477/483. MaxHeight slot 31 is present
in eight/two owners. The first number in each pair is the complete cohort and
the second is the immutable subset.

The previous decoration AutoMaxWidth heuristic was not an owner schema: for
LabelDecoration it emitted 14 non-native lines and missed 34 native lines; the
subset independently reproduced three extras and eight misses. Exact slot 27
removes every contradiction. The old helper remains unchanged for
PictureDecoration and nested ExtendedTooltip only. As negative controls, slot
23 is empty in all 1,512 owners despite 112 MaxWidth positives, slot 25 is one
in all owners despite the AutoMaxHeight split, and MinWidth/MinHeight are absent
from the native owner property universe.

All applicable native adjacent pairs have zero reverse occurrences and both
native and current output have zero duplicates. The complete boundary is:

```text
Width < AutoMaxWidth < MaxWidth < Height < AutoMaxHeight < MaxHeight
      < HorizontalStretch < VerticalStretch < SkipOnInput < TextColor
      < Font < Title < ToolTip < ToolTipRepresentation
      < GroupHorizontalAlign < GroupVerticalAlign < Hyperlink
      < HorizontalAlign < VerticalAlign
```

`FORM_LABEL_DECORATION_GEOMETRY_XML_ORDER` owns the first eight properties. A
LabelDecoration-only helper emits them once at the existing Width position;
the eight scattered generic serializers exclude this owner. Consequently the
101 exact Width lines and 951 already exact AutoMaxWidth lines do not move,
while SkipOnInput, TextColor, Font, and the #93/#94 post-title header remain at
their established positions. PictureDecoration behavior is unchanged.

Integrated commit `9044cc0` changes two files totaling `+248/-25`. The frozen
18,994-byte diff has SHA-256
`9A38DC8CE5DB5A9D84EA6F299ED20628296AEA41849D81856A4D911C12706CF9`.
Independent frozen review returned GO. `cargo fmt --check`,
`cargo check --all-targets`, and `git diff --check` pass with the same two
pre-existing warnings. The diff adds no object/form/item name, path, UUID,
database identity, or corpus branch. Tests, export, database access, ConfigDump,
and ConfigDumpInfo were not run or changed.

The exact shadow covers 436 nodes in 166 already changed Form.xml files. It
adds 614 native lines, removes 14 non-native AutoMaxWidth lines, and makes four
Form.xml paths exact. It predicts Form `1001 files, +8121/-26622` and full
`1522 files, +13359/-82865`. The last actual full diff remains
`1529 files, +14239/-85386` pending a separately authorized export.

TitleHeight and the style tail remain post-VerticalAlign HOLD cohorts. TextColor,
BackColor, Border, visibility, enabled state, shortcuts, input skipping,
title-empty and unreachable owners, ToolTipRepresentation code 6, and
ExtendedTooltip also remain outside this gate.

## Form LabelDecoration TitleHeight tail

Status: integrated for the same 1,512 reachable, title-bearing owners as the
other LabelDecoration gates; a new export was not run.

The existing `FormLabelDecorationSchema` retains its exact wrapper-12,
discriminator-0, length-36, options-kind-5/length-9 guard. It now owns only one
additional visual-tail value: options slot 4 is TitleHeight. Zero and invalid
values are omitted; a valid nonzero unsigned integer is emitted with its raw
decimal representation. The full cohort has 34 positives and 1,478 omissions:
value 1 occurs 10 times, 2 occurs 17 times, 3 occurs three times, 4 occurs
twice, and 6 occurs twice. The immutable 492-owner subset independently has 13
positives and 479 omissions. Both matrices have zero raw-only, native-only, or
value contradictions.

The native ordering evidence is exact and one-directional:

```text
VerticalAlign < TitleHeight < ContextMenu
```

VerticalAlign precedes TitleHeight in all 10 co-occurring owners with zero
reverse pairs. TitleHeight precedes ContextMenu in all 34 positive owners, also
with zero reverse pairs. Native and current duplicate counts are zero.
`FormLabelDecorationVisualTail` and its typed XML order are separate from the
alignment and geometry models. The formatter emits the tail once immediately
after the existing HorizontalAlign/VerticalAlign helper; ContextMenu remains in
the later child-item section.

The apparent neighboring style properties are not admitted. Direct field 16
cannot classify BackColor: the same raw control covers 1,495 absences and 16
native positives. Direct field 17 is constant while only one owner has Border.
BackColor, Border, and TitleHeight never co-occur, so their internal ordering is
also unproven. Current TextColor has one missing value and one value mismatch
among 236 native positives. Current Font has three missing values and 12 value
mismatches among 79 native positives; the immutable subset reproduces five
mismatches among 27 positives. Those reference, absolute-font, and compound
layouts remain separate schema work rather than fallbacks in this scalar gate.

Integrated commit `a58eabc` changes three files totaling `+72/-6`. The test
module contains exactly three mechanical `label_decoration_visual_tail: None`
additions in existing struct literals; no test, assertion, function, or fixture
was added. The frozen 8,310-byte diff has SHA-256
`EE9D4EB8F952EADDFC8E66067ACEB8DB178BD764B7927E763FCF0036845A7F54`.
Independent frozen review returned GO. `cargo fmt --check`,
`cargo check --all-targets`, and `git diff --check` pass with the same two
pre-existing warnings. The diff adds no object/form/item name, path, UUID,
database identity, or corpus branch. Tests, export, database access, ConfigDump,
and ConfigDumpInfo were not run or changed.

The exact shadow adds 34 native one-line properties in 24 reachable Form.xml
files and makes one Form.xml path exact. It predicts Form
`1000 files, +8121/-26588` and full `1521 files, +13359/-82831`. The last actual
full diff remains `1529 files, +14239/-85386` pending a separately authorized
export.

BackColor, Border, TextColor, Font, Title values, ExtendedTooltip, visibility,
enabled state, shortcuts, input skipping, title-empty and unreachable owners,
and ToolTipRepresentation code 6 remain HOLD.

## ConfigDumpInfo aggregate

Status: implemented and confirmed by raw corpus, independent-config checks, and
byte-exact full export.

`ConfigDumpInfo.xml` is not a source asset row or a filesystem-only manifest.
Native synthesizes it by joining the complete metadata inventory with the
`Config` row named `versions`. On the BSP corpus, 9,839 version pairs consist of
one generation entry, 9,835 exported entries, and service entries `root`,
`version`, and `versions`. The XML has exactly the 9,835 non-service top
entries; ID-set, version-formula, and ordering mismatches were all zero.

For every entry:

```text
configVersion = lowercase_hex(Uuid::to_bytes_le(generation_uuid)) + "00000000"
```

Canonical names/hierarchy come from parsed metadata and row-role indexes, not
from physical paths or suffix guesses. Top entries sort ordinally by name;
nested entries sort ordinally by id. Generation is valid only for a complete
Config source-layout export after all batches succeed. Selected, ConfigSave,
and row-local helpers must continue not to emit this global aggregate; unknown,
duplicate, or unmatched inventory entries must fail rather than be skipped.

The complete nested-role matrix contains 5,878 entries. The structural
classifiers resolve 5,851 directly and the established `DocumentJournal.Column`
fallback resolves the remaining 27; there are zero missing or incorrectly
typed roles. Covered families include WebService operation parameters,
HTTPService URL-template methods, register and recalculation dimensions,
Sequence dimensions, root attributes, and tabular-section attributes. List
UUIDs used by these classifiers are platform serialization discriminators, not
metadata object UUIDs: none matches any of the 15,713 native aggregate IDs.
The added production rules contain no application object names or local paths.

Runtime-emitted source assets participate in aggregate routing through their
actual manifest paths. This is required for dynamic assets such as role rights;
duplicate file-name routes with different paths are rejected. Configuration
source suffix `3` is the typed Help asset route and emits both `Ext/Help.xml`
and its localized HTML payload.

When `--include-config-save` is enabled, the streamed Config pass is explicitly
not allowed to create `ConfigDumpInfo.xml`: ConfigSave subsequently replaces
the exported assets, so an aggregate derived from the earlier Config inventory
would be inconsistent.

Accepted BSP gate:

- full diff improved from `1610 files, +31491/-221435` to
  `1603 files, +31486/-204757`;
- native and generated `ConfigDumpInfo.xml` are both 3,110,746 bytes and have
  SHA-256
  `F187FA4F131F9C5DCBD2E41FE630585B1D6C74FB2809D62F4B3B3F0563425A2F`;
- both contain 9,835 top entries and 5,878 nested entries;
- configuration `Ext/Help.xml` and `Ext/Help/ru.html` are byte-exact;
- HTTPService files have zero content diff, while Form and DCS guard metrics
  remain unchanged.
