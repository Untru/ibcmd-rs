# Аудит порядка XML-свойств

Источник: `src\mssql_dump\form_body.rs`
Контракт: `docs\evidence\rea-xdto-20261022\attribute-order-8.5.1.json`

| Элемент | Свойств в эмиссии | Инверсий |
|---|---|---|
| UsualGroup | 11 | 16 |
| Table | 34 | 213 |
| Button | 34 | 132 |
| Field | 29 | 32 |
| Form | 10 | 18 |
| Pages | 2 | 0 |
| Page | 8 | 8 |
| CommandBar | 2 | 0 |
| ContextMenu | 1 | 0 |
| ColumnGroup | 6 | 10 |

## UsualGroup

- `Representation` стоит раньше `HorizontalAlign`, по контракту наоборот
- `Representation` стоит раньше `VerticalAlign`, по контракту наоборот
- `Representation` стоит раньше `Group`, по контракту наоборот
- `Representation` стоит раньше `Behavior`, по контракту наоборот
- `HorizontalAlign` стоит раньше `Group`, по контракту наоборот
- `VerticalAlign` стоит раньше `Group`, по контракту наоборот
- `BackColor` стоит раньше `Group`, по контракту наоборот
- `BackColor` стоит раньше `Behavior`, по контракту наоборот
- `BackColor` стоит раньше `ShowTitle`, по контракту наоборот
- `BackColor` стоит раньше `TitleDataPath`, по контракту наоборот
- `Hyperlink` стоит раньше `Group`, по контракту наоборот
- `Hyperlink` стоит раньше `Behavior`, по контракту наоборот
- `Hyperlink` стоит раньше `ShowTitle`, по контракту наоборот
- `Hyperlink` стоит раньше `TitleDataPath`, по контракту наоборот
- `Hyperlink` стоит раньше `AssociatedTableElementId`, по контракту наоборот
- `ScrollOnCompress` стоит раньше `AssociatedTableElementId`, по контракту наоборот

## Table

- `Title` стоит раньше `TitleLocation`, по контракту наоборот
- `Title` стоит раньше `DataPath`, по контракту наоборот
- `Title` стоит раньше `Visible`, по контракту наоборот
- `Title` стоит раньше `Enabled`, по контракту наоборот
- `Title` стоит раньше `ReadOnly`, по контракту наоборот
- `Title` стоит раньше `TitleHeight`, по контракту наоборот
- `Title` стоит раньше `Representation`, по контракту наоборот
- `Title` стоит раньше `Autofill`, по контракту наоборот
- `Title` стоит раньше `DefaultItem`, по контракту наоборот
- `Title` стоит раньше `SkipOnInput`, по контракту наоборот
- `Title` стоит раньше `Width`, по контракту наоборот
- `Title` стоит раньше `AutoMaxWidth`, по контракту наоборот
- `Title` стоит раньше `MaxWidth`, по контракту наоборот
- `Title` стоит раньше `Height`, по контракту наоборот
- `Title` стоит раньше `BackColor`, по контракту наоборот
- `Title` стоит раньше `EnableStartDrag`, по контракту наоборот
- `Title` стоит раньше `EnableDrag`, по контракту наоборот
- `Title` стоит раньше `FileDragMode`, по контракту наоборот
- `Title` стоит раньше `AutoMaxHeight`, по контракту наоборот
- `Title` стоит раньше `MaxHeight`, по контракту наоборот

## Button

- `Title` стоит раньше `DataPath`, по контракту наоборот
- `Title` стоит раньше `Visible`, по контракту наоборот
- `Title` стоит раньше `Enabled`, по контракту наоборот
- `Title` стоит раньше `Type`, по контракту наоборот
- `Title` стоит раньше `TitleHeight`, по контракту наоборот
- `Title` стоит раньше `Representation`, по контракту наоборот
- `Title` стоит раньше `CommandName`, по контракту наоборот
- `Title` стоит раньше `DefaultItem`, по контракту наоборот
- `Title` стоит раньше `SkipOnInput`, по контракту наоборот
- `Title` стоит раньше `DefaultButton`, по контракту наоборот
- `Title` стоит раньше `Width`, по контракту наоборот
- `Title` стоит раньше `Shortcut`, по контракту наоборот
- `Title` стоит раньше `GroupHorizontalAlign`, по контракту наоборот
- `Title` стоит раньше `GroupVerticalAlign`, по контракту наоборот
- `Title` стоит раньше `AutoMaxWidth`, по контракту наоборот
- `Title` стоит раньше `MaxWidth`, по контракту наоборот
- `Title` стоит раньше `Height`, по контракту наоборот
- `Title` стоит раньше `BackColor`, по контракту наоборот
- `Title` стоит раньше `Picture`, по контракту наоборот
- `Title` стоит раньше `AutoMaxHeight`, по контракту наоборот

## Field

- `Title` стоит раньше `DataPath`, по контракту наоборот
- `Title` стоит раньше `Visible`, по контракту наоборот
- `Title` стоит раньше `Enabled`, по контракту наоборот
- `Title` стоит раньше `ReadOnly`, по контракту наоборот
- `Title` стоит раньше `DefaultItem`, по контракту наоборот
- `Title` стоит раньше `SkipOnInput`, по контракту наоборот
- `TitleLocation` стоит раньше `DataPath`, по контракту наоборот
- `TitleLocation` стоит раньше `Visible`, по контракту наоборот
- `TitleLocation` стоит раньше `Enabled`, по контракту наоборот
- `TitleLocation` стоит раньше `ReadOnly`, по контракту наоборот
- `TitleLocation` стоит раньше `DefaultItem`, по контракту наоборот
- `TitleLocation` стоит раньше `SkipOnInput`, по контракту наоборот
- `TitleLocation` стоит раньше `TitleBackColor`, по контракту наоборот
- `Enabled` стоит раньше `DefaultItem`, по контракту наоборот
- `ReadOnly` стоит раньше `DefaultItem`, по контракту наоборот
- `ToolTipRepresentation` стоит раньше `TitleHeight`, по контракту наоборот
- `ToolTipRepresentation` стоит раньше `DefaultItem`, по контракту наоборот
- `ToolTipRepresentation` стоит раньше `SkipOnInput`, по контракту наоборот
- `ToolTipRepresentation` стоит раньше `TitleBackColor`, по контракту наоборот
- `ToolTipRepresentation` стоит раньше `ToolTip`, по контракту наоборот

## Form

- `Enabled` стоит раньше `HorizontalAlign`, по контракту наоборот
- `Enabled` стоит раньше `Width`, по контракту наоборот
- `Enabled` стоит раньше `VerticalAlign`, по контракту наоборот
- `Enabled` стоит раньше `Height`, по контракту наоборот
- `Enabled` стоит раньше `Group`, по контракту наоборот
- `ReadOnly` стоит раньше `HorizontalAlign`, по контракту наоборот
- `ReadOnly` стоит раньше `Width`, по контракту наоборот
- `ReadOnly` стоит раньше `VerticalAlign`, по контракту наоборот
- `ReadOnly` стоит раньше `Height`, по контракту наоборот
- `ReadOnly` стоит раньше `Group`, по контракту наоборот
- `HorizontalAlign` стоит раньше `Width`, по контракту наоборот
- `HorizontalAlign` стоит раньше `Height`, по контракту наоборот
- `HorizontalAlign` стоит раньше `Group`, по контракту наоборот
- `CommandSet` стоит раньше `VerticalAlign`, по контракту наоборот
- `CommandSet` стоит раньше `Height`, по контракту наоборот
- `CommandSet` стоит раньше `Group`, по контракту наоборот
- `VerticalAlign` стоит раньше `Height`, по контракту наоборот
- `VerticalAlign` стоит раньше `Group`, по контракту наоборот

## Page

- `HorizontalAlign` стоит раньше `Picture`, по контракту наоборот
- `HorizontalAlign` стоит раньше `Group`, по контракту наоборот
- `VerticalAlign` стоит раньше `Picture`, по контракту наоборот
- `VerticalAlign` стоит раньше `Group`, по контракту наоборот
- `BackColor` стоит раньше `Picture`, по контракту наоборот
- `BackColor` стоит раньше `Group`, по контракту наоборот
- `BackColor` стоит раньше `ShowTitle`, по контракту наоборот
- `BackColor` стоит раньше `TitleDataPath`, по контракту наоборот

## ColumnGroup

- `TitleBackColor` стоит раньше `Group`, по контракту наоборот
- `TitleBackColor` стоит раньше `ShowTitle`, по контракту наоборот
- `FixingInTable` стоит раньше `ShowInHeader`, по контракту наоборот
- `FixingInTable` стоит раньше `HeaderHorizontalAlign`, по контракту наоборот
- `FixingInTable` стоит раньше `Group`, по контракту наоборот
- `FixingInTable` стоит раньше `ShowTitle`, по контракту наоборот
- `ShowInHeader` стоит раньше `Group`, по контракту наоборот
- `ShowInHeader` стоит раньше `ShowTitle`, по контракту наоборот
- `HeaderHorizontalAlign` стоит раньше `Group`, по контракту наоборот
- `HeaderHorizontalAlign` стоит раньше `ShowTitle`, по контракту наоборот
