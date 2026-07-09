# Raw form format: accounting register list form

Form:
`AccountingRegisters/_ДемоЖурналПроводокБухгалтерскогоУчета/Forms/ФормаСписка/Ext/Form.xml`

Full inflated raw sources:

- Metadata header: `E:\ibcmd_lab\bsp_metadata_only_probe\Config_inflated\60566ede-81fd-4469-b81e-4cc957b4540d__part0.txt`
- Form body: `E:\ibcmd_lab\bsp_metadata_only_probe\Config_inflated\60566ede-81fd-4469-b81e-4cc957b4540d.0__part0.txt`

The form is not stored as one small `{1,...}` record. There are two relevant records:

- `{1,...}`: metadata object header for the form itself.
- `{4,...}`: form body container: layout tree, module text, attributes, parameters, commands, and other trailing sections.

## Full metadata header

```text
{1,
{13,
{3,
{1,0,60566ede-81fd-4469-b81e-4cc957b4540d},"ФормаСписка",
{1,"ru","Форма списка"},"",0,0,00000000-0000-0000-0000-000000000000,0},0,1,
{2,
{"#",1708fdaa-cbce-4289-b373-07a5a74bee91,1},
{"#",1708fdaa-cbce-4289-b373-07a5a74bee91,2}
}
},0}
```

Mapping:

| Raw fragment | Meaning | XML/output use |
| --- | --- | --- |
| `{1,...}` | Metadata source record | Container for the form metadata object. |
| `{13,...}` | Managed form metadata payload | Identifies this object as a form metadata object. |
| `{3,{1,0,<uuid>},"ФормаСписка",...}` | Standard metadata header | Form UUID, name, synonym/comment flags. |
| `60566ede-81fd-4469-b81e-4cc957b4540d` | Form UUID | Used for object identity and file ownership, not printed as `id` in `Form.xml`. |
| `"ФормаСписка"` | Form metadata name | Path/name of the form object. |
| `{1,"ru","Форма списка"}` | Localized synonym | Human presentation of the form. |

## Form body container

The 83 KB body file has this top-level shape:

```text
{4,
  <layout: {50,...}>,
  <module text: "...">,
  <trailing field 1>,
  <trailing field 2>,
  ...
}
```

Code path:

- `parse_form_body_blob` splits this into `ParsedFormBodyBlob { layout, module_text, trailing }`.
- `extract_form_body_properties` reads the `{50,...}` layout root into top-level `<Form>` properties and events.
- `extract_form_child_items` walks nested item records and formats `<ChildItems>`.
- `extract_form_body_attributes`, `extract_form_body_parameters`, and `extract_form_body_commands` read trailing fields.
- `format_form_body_xml` assembles final `Form.xml`.

## Child item wrappers used in this form

| Wrapper | Key field | Meaning | XML element |
| --- | --- | --- | --- |
| `{22,...}` | field 5 | Group-like item discriminator | `CommandBar`, `UsualGroup`, `ButtonGroup`, `ContextMenu`, `AutoCommandBar`, etc. |
| `{55,...}` | wrapper id | Form table | `<Table>` |
| `{37,...}` / `{48,...}` | type-specific | Form field item | `LabelField`, `InputField`, etc. |
| `{31,...}` / `{34,...}` | wrapper id | Button | `<Button>` |
| `{12,...}` | wrapper id plus fields | Tooltip/decoration | `<ExtendedTooltip>`, `LabelDecoration`, `PictureDecoration` |

For `{22,...}`, `field[5]` is decoded by `form_child_item_tag`:

| field[5] | XML tag |
| --- | --- |
| `0` | `CommandBar` |
| `1` | `Popup` |
| `2` | `ColumnGroup` |
| `3` | `Pages` |
| `4` | `Page` |
| `5` | `UsualGroup` |
| `6` | `ButtonGroup` |
| `8` | `ContextMenu` |
| `9` | `AutoCommandBar` |

## Problem case: ButtonGroup command source

Raw fragment from this form:

```text
{22,
{84,02023637-7868-4a5f-8576-835a76e0c9ba},0,0,0,6,"ФормаГруппаАктивностьПроводок",
{1,1,{"ru","Активность проводок"}},
{1,0},0,1,0,0,0,2,2,
{3,4,{0}},
{7,3,0,1,100},
{0,0,0},1,
{2,{0},2,0},1,a9f3b1ac-f51b-431e-b102-55a69acdecad,
...
}
```

Mapping:

| Raw field | Value | Meaning | XML result |
| --- | --- | --- | --- |
| wrapper | `22` | Group-like form item | Candidate for group/command bar tags. |
| identity | `{84,02023637-...}` | Element id and form-item type UUID | `id="84"` |
| field[5] | `6` | `ButtonGroup` discriminator | `<ButtonGroup ...>` |
| field[6] | `"ФормаГруппаАктивностьПроводок"` | Element name | `name="ФормаГруппаАктивностьПроводок"` |
| field[7] | `{1,1,{"ru","Активность проводок"}}` | Localized title | `<Title>...Активность проводок...</Title>` |
| field[20] | `{2,{0},2,0}` | Bare form source marker | No `<CommandSource>` in ibcmd XML. |

Correct `CommandSource` rule for `ButtonGroup`:

```text
{2,{0,02023637-7868-4a5f-8576-835a76e0c9ba},2,0}
  -> <CommandSource>Form</CommandSource>

{2,{0},2,0}
  -> no <CommandSource>
```

This is why `parse_form_button_group_command_source` must require the second item inside `source[1]` to be `02023637-7868-4a5f-8576-835a76e0c9ba`, not just accept bare `{0}`.

## Table block highlights

The main list is a `{55,...}` item:

```text
{55,{1,02023637-7868-4a5f-8576-835a76e0c9ba},..., "Список", ..., 11,
  5,{"B",0},
  6,{"N",60},
  7,{"#",2fdc88ec-7c9b-43cd-8ba5-873f043bdd88,{0,00010101000000,00010101000000}},
  8,{"#",59ef2b80-c86b-11d5-a3c1-0050bae0a776,0},
  9,{"B",0},
  10,{"U"},
  11,{"B",1},
  12,{"B",0},
  14,{"#",eac7bfa0-10b4-4369-996c-d258871ad519,0},
  16,{"N",123},
  20,{"B",1},
  ...
}
```

Selected table mapping:

| Raw source | XML |
| --- | --- |
| wrapper `55` | `<Table>` |
| identity `{1,02023637-...}` | `id="1"` |
| name `"Список"` | `name="Список"` |
| table slots / defaults | `Representation`, `CommandBarLocation`, `DefaultItem`, `InitialTreeView`, drag settings |
| property-bag key `5` | `<AutoRefresh>false</AutoRefresh>` |
| property-bag key `6` | `<AutoRefreshPeriod>60</AutoRefreshPeriod>` |
| property-bag key `7` | `<Period>...</Period>` |
| property-bag key `8` | `<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>` |
| property-bag key `9` | `UseAlternationRowColor`; false is ignored here and the final true value is recovered from table slots |
| property-bag key `10` | row filter undefined marker; not printed by current formatter for this case |
| property-bag key `11` | `<DefaultItem>true</DefaultItem>` |
| property-bag key `12` | `<RestoreCurrentRow>false</RestoreCurrentRow>` |
| property-bag key `14` | `<UpdateOnDataChange>Auto</UpdateOnDataChange>` |
| property-bag key `16` | `<UserSettingsGroup>...</UserSettingsGroup>` via attribute id/name resolution |
| property-bag key `20` | `<AllowGettingCurrentRowURL>true</AllowGettingCurrentRowURL>` |
| table slot `36` | `<ShowRoot>true</ShowRoot>` |
| table slot `37` | `<AllowRootChoice>false</AllowRootChoice>` |

For this table ibcmd prints default root properties even when key `15` is absent:

```text
wrapper 55 + ShowRoot=true + AllowRootChoice=false + no explicit key 15
  -> <InitialTreeView>ExpandTopLevel</InitialTreeView>
  -> <TopLevelParent xsi:nil="true"/>
```
