# Diff Mining - 2026-07-01 Round 35

Input diff:

`E:\ibcmd_lab\full_diff_20260630_234359_round33_percent\diff_full_source_only.json`

This snapshot was generated before round 34 was merged, so round 34 fixes must
be treated as already handled when planning new work:

- Catalog `<Owners>`;
- DCS `calculatedField` `d4p1` current-config type context;
- Form wrapper `55` table `RowFilter xsi:nil`;
- ExchangePlan `Content.xml` metadata-tree ordering;
- CommonAttribute property-tail `FillValue`;
- `Ext/ParentConfigurations.bin` raw-deflated staging.

Sampling limits used for XML-path mining:

- `form`: first 900 different files;
- `metadata_xml`: first 900 different files;
- `template`: first 450 different files;
- `configuration_root`: 1 file.

The full all-file XML-path pass timed out after 120 seconds, so this report is a
bounded prioritization aid, not a replacement for a full post-round diff.

## High-Volume Signatures

### Template / MXL

The largest sampled deltas are MXL spreadsheet shape/format paths:

| Direction | Count | Path |
|---|---:|---|
| native missing in ours | 22533 | `document/format/width` |
| native missing in ours | 17958 | `document/format` |
| native missing in ours | 15471 | `document/format/font` |
| native missing in ours | 12211 | `document/format/verticalAlignment` |
| native missing in ours | 10789 | `document/format/horizontalAlignment` |
| native missing in ours | 10185 | `document/format/textPlacement` |
| value/attr diff | 323286 | `document/rowsItem/row/c/c/f` |
| value/attr diff | 16425 | `document/rowsItem/row/formatIndex` |
| value/attr diff | 16328 | `document/columns/columnsItem/column/formatIndex` |

Likely next work: MXL format table/index preservation or native default
suppression around row/cell format references.

### Catalog Metadata XML

The largest metadata XML missing-path cluster is Catalog child `Attribute`
property tails. In the sample, many properties have the same count (`7106`):

- `PasswordMode`;
- `Format`;
- `EditFormat`;
- `ToolTip`;
- `MarkNegatives`;
- `Mask`;
- `MultiLine`;
- `ExtendedEdit`;
- `MinValue`;
- `MaxValue`;
- `FillFromFillingValue`;
- `FillValue`;
- `FillChecking`;
- `ChoiceFoldersAndItems`;
- `ChoiceParameterLinks`;
- `ChoiceParameters`;
- `QuickChoice`;
- `CreateOnInput`;
- `ChoiceForm`;
- `LinkByType`;
- `ChoiceHistoryOnInput`;
- `Use`;
- `Indexing`;
- `FullTextSearch`;
- `DataHistory`.

Likely next work: reuse the generic child attribute property-tail formatter for
Catalog attributes and tabular-section attributes, preserving native order and
type decoding.

Other Catalog value/property clusters:

- standard attribute `FillValue` / `FillChecking`;
- owner-level `QuickChoice`;
- owner-level `UseStandardCommands` and `IncludeHelpInContents`;
- owner-level `DataLockControlMode`.

### Form XML

The sampled extra-path clusters show our Form exporter still emits defaults that
native omits in many files:

| Count | Extra path in ours |
|---:|---|
| 415 | `MetaDataObject/Form/Properties/UseInInterfaceCompatibilityMode` |
| 414 | `Form/ShowCommandBar` |
| 413 | `Form/Group` |
| 244 | `Form/WindowOpeningMode` |
| 203 | `Form/ChildItems/Table/RowSelectionMode` |
| 161 | `Form/ChildItems/Pages/ChildItems/Page/ScrollOnCompress` |
| 160 | `Form/ChildItems/Table/SkipOnInput` |
| 130 | `Form/AutoFillCheck` |
| 129 | `Form/ChildItems/Table/EnableDrag` |

Likely next work: native default suppression for one repeated root/child-item
property class, not more object-specific form handling.

### BusinessProcess Flowchart

The sampled value differences include Flowchart / GraphicalSchema properties:

- `GraphicalSchema/Items/ConnectionLine/Properties/ZOrder`;
- `GraphicalSchema/Items/ConnectionLine/Properties/Font`.

Likely next work: decode or normalize one generic GraphicalSchema connection
line property class.

## Planning Notes

- Do not use this stale diff to claim percentage improvement after round 34.
  Generate a fresh full diff before updating the overall readiness percentage.
- Prefer assigning agents by signature class, not by top-level object kind.
- Keep lab artifacts under `E:\ibcmd_lab`.
- Do not generate `ConfigDumpInfo.xml`.

