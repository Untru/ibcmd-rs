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

Status: mostly `confirmed by export`; overflow behavior is a `hypothesis`.

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
  The block is inserted into a `settingsVariant` element before that variant's
  closing tag. The pairing is positional: first settings document to first
  `settingsVariant`, second to second, and so on.
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

Hypothesis / needs validation:

- If there are more embedded `Settings` documents than `settingsVariant`
  elements, the remaining settings blocks should be appended before the closing
  `DataCompositionSchema` tag. This is a conservative fallback, but it needs a
  native export sample before being treated as part of the proven model.
- The positional pairing should be rechecked on a template with two or more
  variants and two or more settings payload documents.

## Form.xml `CommandName` / `CommandSource`

Status: draft model. Several common paths are `confirmed by export`; the full
item-type command-source matrix remains a `hypothesis`.

Model:

- `CommandName` names the command executed by a form item, usually a `Button`.
  It is resolved from the native command-reference tuple and supporting command
  tables, not from the visible item name alone.
- Local form commands are emitted as `Form.Command.<name>` when the item
  reference matches a command entry from the form command tail section.
- Form standard command UUIDs are emitted as `Form.StandardCommand.*` only for
  mappings that are confirmed by export or explicitly covered by focused tests.
  Do not promote UUIDs from nearby excluded-command tables into
  `Button/CommandName` mappings without export evidence.
- Object or common command references are emitted as object/common command names
  when the referenced owner metadata is available. If the object reference is a
  standard object command, the emitted name includes the standard-command suffix.
- Table standard commands are emitted as
  `Form.Item.<table>.StandardCommand.<suffix>` when the command UUID is a known
  table standard command and the table owner can be resolved.
- Unknown command-reference tuples should be omitted rather than guessed.

`CommandName` standard-command UUID model:

| Raw precondition | UUID | Emitted `CommandName` | Status | Evidence |
| --- | --- | --- | --- | --- |
| `Button/CommandName` standard form command reference | `679b62d9-ff72-4329-bf3a-c0c32b311dd2` | `Form.StandardCommand.Cancel` | confirmed by export | Narrow command-name subfix; final release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `f3613d5c-20c6-46e5-b4d5-7d712ece1296` | `Form.StandardCommand.OK` | confirmed by export | Narrow command-name subfix; final release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `6886601d-276c-4d3f-af0a-05c586025608` | `Form.StandardCommand.Change` | confirmed by export | Raw evidence 21 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `8e2b82cf-d1ea-46b2-afdf-a8d64e66ea2b` | `Form.StandardCommand.Choose` | confirmed by export | Raw evidence 14 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `bdefa701-6685-453e-a02a-3683d0cc16d3` | `Form.StandardCommand.Find` | confirmed by export | Raw evidence 12 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `3b8cedbc-8e74-4017-b901-d14b09f32f7a` | `Form.StandardCommand.Post` | confirmed by export | Raw evidence 11 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `2e86453d-8958-4c9a-a1b4-b15215eedc2e` | `Form.StandardCommand.SetDeletionMark` | confirmed by export | Raw evidence 6 examples; release/export had `NO_ADDED_COMMANDNAME` |
| `Button/CommandName` standard form command reference | `827b541d-30c1-4f06-aecf-92aa496a0835` | `Form.StandardCommand.SetDeletionMark` | confirmed by export | Raw evidence 6 examples; release/export had `NO_ADDED_COMMANDNAME` |

Rejected model:

- Excluded-command tables are not equivalent to `Button/CommandName` standard
  mappings. A wider reuse of excluded-command UUID tables was tried earlier and
  rejected because it produced `CommandName +114`.
- Form standard command candidates remain unconfirmed unless listed in the
  table above or proven by a separate narrow export. Do not infer additional
  UUIDs from excluded-command tables.

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

- For `CommandBar`, raw `fields[20]` is the source record. The safe export rule
  accepts only this shape:

```text
fields[20] = {1,<ignored>,{<source-id>,02023637-7868-4a5f-8576-835a76e0c9ba}}
fields[21] = 5
  -> <CommandSource>...</CommandSource>
```

- For `CommandBar`, `source[0]` must be `1`, `fields[21]` must be `5`, and
  `source[2]` must be the typed managed-form reference. `source[1]` is not the
  item source id.
- Bare `CommandBar` records must not emit `CommandSource`:

```text
{1,0,{0}}
  -> no <CommandSource>

{1,2,{0}}
  -> no <CommandSource>
```

- `Popup` `CommandSource` is not part of the current proven model. The safe
  parser must not reuse the `CommandBar` rule for popup items until raw samples
  and native export output prove a separate popup shape.

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
- `Button/CommandName` is restored for local form commands, confirmed or
  test-supported form standard commands, and external/object commands when
  owner metadata is available.
- `ButtonGroup` with bare `{2,{0},2,0}` must not emit `CommandSource`.
- `ButtonGroup` with the typed form source tuple emits
  `<CommandSource>Form</CommandSource>`.
- `CommandBar` must read the typed source id from `source[2]`, not from
  `source[1]`.
- `CommandBar` with bare `{1,0,{0}}` or `{1,2,{0}}` must not emit
  `CommandSource`.

Supported by tests:

- Focused tests cover existing standard-command and table-standard-command
  resolution paths, but export confirmation is per UUID/status entry above.
- Unknown command tuples are intentionally omitted.

Hypothesis / needs validation:

- Non-form and non-zero typed source ids in accepted `CommandBar` /
  `ButtonGroup` records need more native export samples.
- Form standard commands beyond the confirmed/test-supported UUID set remain
  hypotheses. Do not infer additional `Button/CommandName` mappings from
  excluded-command tables.
- The table-standard-command table-name disambiguation needs broader export
  coverage for forms with multiple tables and similarly named buttons.
- The current draft does not prove every form item type that can legally carry
  `CommandName` or `CommandSource`; it only fixes the observed button,
  command-bar, and button-group routes and deliberately excludes popup
  `CommandSource`.
