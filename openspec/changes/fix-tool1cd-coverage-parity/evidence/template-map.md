# Template and spreadsheet evidence map

Issue #570. Source pin: [tool1cd 7c4b91cb7a7bb48a32be00b9f32afe740dada6e1](https://github.com/jjzhang166/tool1cd/tree/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1).

## Source applicability

The `SpreadsheetDocument` entry at [MetaTree 44666](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L44666) describes the runtime object and its properties. It does **not** describe the MOXCEL binary root, format masks, counted page-break runs, charts or embedded tables. A matching property name in this entry is not a storage coordinate. Searches for `verticalPageBreak`, `VerticalPageBreak` and `MOXEL` find no entries; the two `SpreadsheetInsertPageBreak` / `SpreadsheetDeletePageBreak` entries at 53053/53060 are command identities, not document-body storage.

Tool1cd does independently give concrete enum values and an HTML storage grammar:

- [Web colours 1122/1142](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L1122): SkyBlue=129 and SandyBrown=124. Their separate runtime property declarations at 1445 etc. carry no new storage semantics.
- [Windows fonts 1780](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L1780): OEMFixedFont=1.
- [SpreadsheetDocumentDrawingLineType 45772–45781](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L45772): Dashed=2, DashDotted=4. The **ChartLineType** enum immediately above it has Dashed=3; copying the similarly named chart enumeration into spreadsheet drawing lines would be wrong.
- [GraphicalSchemaShapes 47746](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L47746): VerticalBrackets=9.
- [HTML storage 53128–53147](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L53128): version and page count, a count-prefixed sequence of page/language content, and related files for version newer than 4. The minimized legacy `{5,0,0}` therefore has a meaningful empty document shape. It is admitted only in that complete exact empty shape; missing or extra trailing data remains refused.

No implementation or MetaTree data is copied or loaded at runtime. New declarative values live in `ibcmd-schema::tool1cd_templates`; actual modern bytes are checked against public Coverage.

## Measured rules beyond the old source

The page-break writer is based on native Coverage plus the existing independent reader, not on an invented tool1cd grammar. Extracting the `CovSheetVerticalPageBreak` body from the public 8.3.27.2214 CF shows the row/column grouping run followed directly by vertical count=1, position=1, horizontal count=0 and the merge list. The previous writer's final `0,0` in its grouping method were empty break counts. They are replaced with counted runs; no extra framing zeros are added. Horizontal positions carry `-1` companions, as already measured by the reader. Break entries reject duplicate/missing positions, unknown attributes/children and fractional positions.

Native `CovSheetPageSizeA4` stores print key 0 as text `A4`, whereas numeric key 0 is the older `<paper>` member. Both variants retain their type. Native `CovSheetAutoFontHeight` stores the sparse AutoFont height as revision 7, kind 3, mask 2, height in tenths, marker 1 and scale 100. These two rules are native Coverage measurements; tool1cd's runtime PageSize/AutoFont names alone do not establish these physical layouts.

## Audit-class disposition

The implementation column describes changes awaiting the final central checks; it is not a claim that an entire set or SQL route is now equal.

| Audit class | Applicable evidence / disposition |
| --- | --- |
| I001 verticalPageBreak | Existing reader + native public CF counted runs; writer implemented, whole native XML comparison added on both original platforms. No MOXCEL storage grammar in tool1cd. |
| I006 palette colour namespace | Tool1cd predates the measured 8.5 palette profile. Runtime colour names do not establish its storage; remains a distinct profile investigation. |
| I008 drawing Dashed | Tool1cd drawing enum=2 + 8.5.1.1529 native Coverage; both directions implemented. |
| I009 built-in StyleItem font read as a file | Runtime style identities alone do not prove the configuration resolver path; not counted as fixed by the enum additions. |
| I012 embeddedSpreadsheetDocument | [MetaTree SpreadsheetDocument 44666](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L44666) gives runtime properties, not the embedded MOXCEL document storage grammar. No guessed nested body layout; remains unresolved. |
| I016 pageSize | Native text-vs-number print-key witness; both directions implemented and whole native XML comparison added. |
| I017 AutoFont height | Native sparse mask=2 witness; both directions implemented; other unmeasured AutoFont members remain explicit refusals. |
| I025 standard picture reference read as a file | Tool1cd has picture identities, but a separate MXL/reference routing rule is needed; not counted as fixed here. |
| I046 chart Column | Tool1cd runtime chart enum is not the chart's MOXCEL binary profile; no guessed body layout. |
| I047 line gap | Native Coverage identifies last line-descriptor member 7 as gap (member 5 is not gap). Implemented as a retained boolean in the line projection, strict 0/1 reader, source writer and XML projection; positive native Coverage and malformed-input checks remain subject to the final run. |
| I048 ValueListType | Runtime value-type declaration does not supply the complete stored SpreadsheetDocument cell-value grammar. |
| I049 drawing DashDotted | Tool1cd drawing enum=4 + native Coverage; both directions implemented. |
| I050 OEMFixedFont | Tool1cd WindowsFont enum=1 + native Coverage; both directions implemented. |
| I051 SandyBrown | Tool1cd web enum=124 + native Coverage; both directions implemented. |
| I052 SkyBlue | Tool1cd web enum=129 + native Coverage; both directions implemented, with separate modern namespace projection scrutiny. |
| I079 chart selectionMode | Runtime chart property does not identify its binary coordinate. |
| I081 chart Column | Same binary-profile gap as I046. |
| I082 chart Pie3D | Runtime chart enum alone does not establish the chart-body layout. |
| I084 Help stored-page readback | Tool1cd HTML page grammar does not define our canonical/relative help-link rewrite policy. Needs an independent link-context rule. |
| I085 embeddedTable | `EmbeddedTables` at MetaTree 44674 is a runtime property; no applicable MOXCEL embedded-table storage grammar. Source normalization probe is retained. |
| I086 chart readback failure | Need the concrete chart subprofile; a matching runtime chart type cannot justify bypassing the reader comparison. |
| I091 VerticalBrackets | Tool1cd shape enum=9 + native public graphical-schema witness; source writer and reader enum handling implemented. |
| I100 inconsistent chart series count | [MetaTree SeriesCount runtime declaration](https://github.com/jjzhang166/tool1cd/blob/7c4b91cb7a7bb48a32be00b9f32afe740dada6e1/MetaTree.txt#L6584) do not establish MOXCEL chart-series framing. This is currently an XML normalization probe; native normalization is not proof of a stored binary grammar. Remains unresolved. |
| E02 GIF graphical-schema picture | Tool1cd references binary picture content but does not define image magic sniffing. Native public GIF/PNG witnesses establish a separate reader admission rule; GIF87a/GIF89a extension detection is implemented while other GIF version signatures remain refused. |
| E06 empty HTML record | Concrete tool1cd HTML grammar plus minimized legacy public record. Exact zero-page/zero-file shape implemented with refusal tests; not mislabelled a freshly native-built nonempty source case. |
| E07 eleven flowchart print parameters | Native public `CovPrintedFlowchart` adds Paper, PageOrientation, Collate, Copies and PaperSource to the older six-field set. Tool1cd only contributes runtime semantics; the native keyed storage witness is the coordinate proof. The reader/model/source writer now preserve the five optional numeric properties and unknown keys/orientations remain refused. |
| E10 English composition names | Runtime language metadata does not establish the DCS alias/name writer policy. Independent DCS scope, not fixed by template enum additions. |

The targeted tests use public native Coverage files, not application exports. The empty HTML regression is upgraded from a conditional known-refusal check to mandatory successful export, preserved HTMLDocument descriptor and the complete expected zero-page Help XML body. Final pass/fail, whole-set/combined counts and any remaining modern projection mismatches must come from the central measured run.

## Additional native profile measurements

The line's gap flag is the **last** member of its eight-member descriptor (member 7), not member 5. `CovSheetGapLine` stores `{4,0,{0},0,1,0,<cell line type>,1}` and native emits `<line width="1" gap="true">`. Existing application-derived observations already show member 5 can be 3 while native gap is false, so interpreting that member as a boolean would corrupt previously supported lines. The new line model retains gap independently; existing synthesized default lines remain false and direct source-line copies retain their own value.

The graphical-schema header has 11 keyed parameters for `CovPrintedFlowchart`: Paper=9, PageOrientation=Portrait, Collate=1, Copies=1, PaperSource=7 plus the six previous margins/BlackAndWhite/FitPageMode fields. Native publishes Paper, Collate and PaperSource with `xsi:type="xs:decimal"`. Both the source writer and reader preserve this set, sorted by stored key when compiled; the source writer rejects unknown or duplicate print properties rather than ignoring them. The optional fields are not emitted for the old six-key profile.

## Counted group/break boundary

The measured native `8,1,12` profile now walks the already split root from its row count through rows, column sets, row-to-column mappings and drawings before reading the two group runs and page breaks. It does not scan unrelated zeros to guess a boundary. The earlier reader comment incorrectly described two additional reserved zeros: the native witness shows those zeros were the empty group counts. A matched malformed prefix is refused and never falls back to the historical scanner; that scanner remains only for profiles outside this measured header.

A regression combines the independently native page-break witness with the supported row-group and column-group source shapes. It checks both counted boundaries and malformed-prefix refusal. This is a clean-room combined case, not a claim that the SDK built this exact combined source. Whole-native page-break equality on both platforms remains a separate assertion.

## Whole-reader malformed-record checks

The gap regression mutates the native public descriptor's final member from 1 to 2, packs a real raw-deflate MOXCEL container and calls the same source-asset extraction entry point used by export. The initial whole-reader run failed: the local table parser refused the value but legacy reconstruction still published a spreadsheet after losing the flag. Native revision-4/marker-0/{0} descriptor framing is now recognized before that fallback, independently of the kind UUID; a failure to decode such a table refuses the whole source asset. Historical alternative descriptor profiles keep their existing reconstruction route. The successful-table reference-coverage policy is unchanged by this fix.

The equivalent whole-source-asset tests also mutate the measured native page-break witness's group count and signed position. Both already refused the complete source asset, confirming that the exact-prefix parser failure propagates beyond the helper rather than publishing empty groups or breaks. Positive whole-native comparisons remain mandatory.

The final review additionally replaced **all** line-kind UUIDs in each native public witness with one unknown valid UUID. The red whole-reader test showed that tying profile recognition to known kind values could still enter reconstruction. Recognition now depends only on the measured descriptor framing; the decoder itself rejects unknown kinds. Both raw and source-asset reader results are asserted on both original platforms.

Six older focused format/column-mapping tests had abbreviated synthetic roots declaring `8,1,12` while omitting native header/footer, group/break and merge framing. They now retain the exact original row/column/format/string test data inside a complete native-shaped frame measured on public Coverage. Their existing semantic assertions are unchanged; malformed native framing is not accepted to keep these synthetic tests green.

The restored format-index fixture initially published the target as `<f>1</f>`: complete native framing activates the existing native first-use pool order, and both its column and cell named the same source format first. The diagnostic XML confirmed correct target width/Wrap/orientation retention. The focused fixture now has its column claim the font-only source format 2 before the cell claims the unchanged target source format 1. Thus the intended target really occupies output slot 2; the original `<f>2</f>` and target-format assertions remain unchanged. This is a fixture correction, not a production normalization change.
