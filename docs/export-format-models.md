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

Status: settings binding and QName scope allocation are `confirmed by export`.

Model:

- A `DataCompositionSchema` template body can contain more than one embedded XML
  document after inflation. The schema document is the `SchemaFile` wrapper that
  contains `dataCompositionSchema`; settings documents are separate `Settings`
  XML documents in the DCS settings namespace.
- Native source export is a single `DataCompositionSchema` XML document, not the
  raw `SchemaFile` container. The `SchemaFile` and inner
  `dataCompositionSchema` wrappers are structural storage wrappers and are not
  emitted.
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
  metadata `TypeId` values become current-config `v8:Type` references, and the
  known AnyIBRef type id becomes a `v8:TypeSet` current-config reference.
- Lexical QNames are resolved through the input namespace stack into
  `{namespace URI, local}` and then serialized for the output scope. Raw `dNp1`
  prefix text is never copied. Dynamic current-config and enterprise prefixes
  use their structural base plus `2 * nestedSchemaDepth`.
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

Rejected hypothesis:

- Omitting `style` / `sys` / `web` / `win` namespace declarations on empty
  inserted `dcsset:settings` roots was tested and rejected. It increased the
  full diff from `1917` to `1924` files while the target DCS file still had
  `58 insertions` / `58 deletions`. The residual also contains enterprise
  namespace prefix shifts and `v8:StandardPeriod` differences. The accepted
  expanded-QName/output-scope model above supersedes this rejected shortcut.

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
  field subtree is not yet emitted as a form item. Five correlated platform
  command UUIDs are currently proven for reachable Buttons:

| UUID | Standard-command suffix |
| --- | --- |
| `d8e20c4d-3519-49aa-80e5-d6d66fee741a` | `Save` |
| `d673d512-f71a-48a6-ae5d-527a64ffd813` | `Print` |
| `5aa38159-2001-42ae-8451-f8cabe0762c3` | `Preview` |
| `12acffde-8389-4e5e-bd86-ff248262d84a` | `ExpandAllGroups` |
| `ff5c34f8-b172-4ef2-91d3-48283a66a725` | `CollapseAllGroups` |

- A wrapper `37` / `48` record with discriminator `17` is a distinct
  FormattedDocument owner. Standard-command UUID dispatch is owner-typed; these
  UUIDs must not leak into SpreadsheetDocument or Table resolution:

| UUID | FormattedDocument suffix |
| --- | --- |
| `39f6b9f1-7aa1-4a03-a01b-e127d51bc228` | `DecreaseIndent` |
| `56ae90b6-588f-406e-919c-cc5cc7f86297` | `AlignJustify` |
| `87ecfbdd-8e2b-4ba2-a315-0897020f382f` | `AlignLeft` |
| `9d8a3915-de52-4227-91cd-2fce22e09972` | `Picture` |
| `a8483976-8b13-416a-9680-133b306dc6b0` | `Print` |
| `ab0ebc39-68ee-4034-b2f4-43eee55bd651` | `AlignCenter` |
| `d0a4d953-115b-4059-a6cb-6e67f903a4f3` | `IncreaseIndent` |
| `e428af27-c4f7-4577-b80e-95a79f94322d` | `AlignRight` |

`CommandSource` model:

- `CommandSource` describes which command set a command container reads from; it
  is not the same value as `CommandName`.
- Once a typed source id has been accepted, source id `0` maps to `Form`, source
  id `-1` maps to `FormCommandPanelGlobalCommands`, and other source ids map
  through the form item id table to `Item.<item-name>`.
- For `ButtonGroup`, the source tuple must carry both the source item id and the
  managed-form item type UUID `02023637-7868-4a5f-8576-835a76e0c9ba`.
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

- Popup global-command source is a separate nine-field discriminator-7 record.
  Its typed source is `{0,2ef6d6fa-847a-485e-8684-d37a3ab5efb8}` and structural
  sentinels are `source[3] == 2`, `source[5] == 0`, `source[6] == 0`; varying
  mode fields do not affect the source meaning.
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

## ConfigDumpInfo aggregate

Status: corpus model confirmed; implementation remains open.

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
