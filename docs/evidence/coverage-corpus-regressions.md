# Corpus regression coverage: 2026-10-25

This extends [PR #535](https://github.com/Untru/ibcmd-rs/pull/535). It records observed failures of the pinned product; it does not fix the converter or establish release/SQL parity.

The full XML audit found 2042 failed rows in 104 diagnostic classes: 99 reproduced by native-built public CF sets, three by source XML that native normalizes away, and two by minimized legacy XML commands refused by the current native writer. Diagnostic classes are not asserted to be independent root causes.

All new source inputs, complete native oracle trees, CFs and proof manifests are committed. Two cross-version CFs remain separate because XML reconstruction would erase their stored layout. No application exports, paths, code or blobs are published.

The old 95 generated configuration sets remain byte-identical. Original baseline entries are retained. A typed per-entry CF refusal now allows complete byte-equal sibling objects to be checked without claiming global export success. Unknown failures, empty partial output and unattributed global refusals remain failures.

[Rebuild instructions](coverage-corpus-build.md); [special source/legacy probes](../../tests/fixtures/coverage-records/README.md); [pinned evidence matrix](coverage-corpus-matrix.json).

| Class | Diagnostic | Observed rows | Evidence scope | Representative set |
| --- | --- | --- | --- | --- |
| I001 | MXL: <verticalPageBreak> of a SpreadsheetDocument has no writer | 1286 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I002 | no code for UseInInterfaceCompatibilityMode "Version85" | 148 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I003 | no code for UseInInterfaceCompatibilityMode "Taxi" | 117 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I004 | form <UsualGroup>: ControlRepresentation ButtonInParentElement has no 8.3.27 spelling | 85 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I005 | the form's tail names something the writer cannot place | 48 | same_failure_cf | 8.5.1.1529/corpus-legacy-form-properties |
| I006 | MXL: colour pal:<name> names an unknown namespace | 29 | same_failure_cf | 8.5.1.1529/corpus-modern-spreadsheet |
| I007 | <LabelDecoration> names a font the writer cannot place | 25 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I008 | MXL: drawing line style Dashed has no stored code | 19 | same_failure_cf | 8.5.1.1529/corpus-modern-spreadsheet |
| I009 | MXL: built-in StyleItem font reference read as a file | 18 | same_failure_cf | 8.5.1.1529/corpus-modern-spreadsheet |
| I010 | <Table> excludes SelectAll, which has no measured uuid | 16 | same_failure_cf | 8.5.1.1529/corpus-navigation-commands |
| I011 | <LabelDecoration> names a colour the writer cannot place | 14 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I012 | an embedded mxl:SpreadsheetDocument with cells has no writer yet | 14 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I013 | a group names a background colour it cannot place | 12 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I014 | a <ChartField> names a property whose slot is not measured | 9 | same_failure_cf | 8.3.27.2214/corpus-form-facets |
| I015 | form <PictureDecoration>: PictureColor: the writer cannot place the colour pal:<name> | 8 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I016 | MXL: <printSettings><pageSize> has no writer | 6 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I017 | MXL: an AutoFont <font> spells members | 6 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I018 | form <InputField>: TextSize: the 2.21 value Some("Reduced") has no 8.5 code | 6 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I019 | form <LabelField>: UseCopy: the 2.21 value Some("false") has no 8.5 code | 6 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I020 | <Table> excludes DeselectAll, which has no measured uuid | 5 | same_failure_cf | 8.5.1.1150/corpus-form-facets |
| I021 | unsupported Form VerticalScroll: use | 5 | same_failure_cf | 8.3.27.2214/corpus-form-settings |
| I022 | unsupported colour "pal:<name>" | 5 | same_failure_cf | 8.5.1.1529/corpus-palette-help-and-interface |
| I023 | <LabelField> cannot encode a bound path | 4 | same_failure_cf | 8.3.27.2214/corpus-owned-catalog-forms |
| I024 | <LabelField> names a colour the writer cannot place | 4 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I025 | MXL: standard picture reference read as a file | 4 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I026 | a <GraphicalSchemaField> names a property whose slot is not measured | 4 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I027 | no base-free compiler for Interface yet | 4 | same_failure_cf | 8.3.27.2342/interfaces |
| I028 | no measured value for StdPicture.NewConversation | 4 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I029 | the form names an event the writer cannot place | 4 | same_failure_cf | 8.3.27.2214/corpus-task-form-events |
| I030 | unsupported AdditionalIndexes: AdditionalIndexes target suffix is not mapped for this source family | 4 | same_failure_cf | 8.3.27.2214/corpus-catalog-additional-index |
| I031 | <InputField> cannot encode a bound path | 3 | same_failure_cf | 8.3.27.2214/corpus-owned-catalog-forms |
| I032 | <LabelField> names a font the writer cannot place | 3 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I033 | form <InputField>: WidthInCard: the 2.21 value Some("Full") has no 8.5 code | 3 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I034 | form <InputField>: no measured value for StdPicture.CollaborationSystemExternalUser | 3 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I035 | form <RadioButtonField>: HorizontalStretch: the 2.21 value Some("false") has no 8.5 code | 3 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I036 | no measured value for StdPicture.Char | 3 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I037 | no measured value for StdPicture.Today | 3 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I038 | <ChartField> names an event the writer cannot place | 2 | same_failure_cf | 8.3.27.2214/corpus-form-facets |
| I039 | <CheckBoxField> cannot encode a bound path | 2 | same_failure_cf | 8.3.27.2214/corpus-form-deep-bindings |
| I040 | <PeriodField> has no writer yet | 2 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I041 | <PlannerField> names an event the writer cannot place | 2 | same_failure_cf | 8.3.27.2214/corpus-form-facets |
| I042 | <SpreadSheetDocumentField> excludes FormattedString, which has no measured uuid | 2 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I043 | <Table> excludes AddAutoSelectedField, which has no measured uuid | 2 | same_failure_cf | 8.3.27.2214/corpus-form-facets |
| I044 | ClientApplicationInterface: unsupported layout property | 2 | same_failure_cf | 8.3.27.2214/corpus-client-application-interface |
| I045 | InvalidEnvelope("mobile functionality block is not the complete dialect roster") | 2 | same_failure_cf | 8.3.27.2214/corpus-mobile-permission-messages |
| I046 | MXL: <chart:chartType> spells Column, which the chart writer has not measured | 2 | same_failure_cf | 8.5.1.1529/corpus-modern-spreadsheet |
| I047 | MXL: a <line> with a gap has no writer | 2 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I048 | MXL: a value of type v8:ValueListType has no writer | 2 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I049 | MXL: drawing line style DashDotted has no stored code | 2 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I050 | MXL: system font sys:OEMFixedFont has no stored code | 2 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I051 | MXL: web colour SandyBrown has no stored code | 2 | same_failure_cf | 8.3.27.2214/corpus-spreadsheet-facets |
| I052 | MXL: web colour SkyBlue has no stored code | 2 | same_failure_cf | 8.5.1.1529/corpus-modern-spreadsheet |
| I053 | a command names the shortcut Space, which is not measured | 2 | same_failure_cf | 8.3.27.2214/corpus-owned-catalog-forms |
| I054 | a dynamic list's <KeyType>RowNumber is not measured | 2 | same_failure_cf | 8.3.27.2214/corpus-form-settings |
| I055 | a progress bar's <Representation>BrokenTilt is not measured | 2 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I056 | a table's <RowPictureDataPath> names <path>, which the writer cannot place | 2 | same_failure_cf | 8.5.1.1529/corpus-navigation-commands |
| I057 | command: unknown standard picture StdPicture.QueryWizardTempTableDescription | 2 | same_failure_cf | 8.3.27.2214/corpus-reference-network |
| I058 | form <ColumnGroup>: ShowTitleInCard: the 2.21 value Some("true") has no 8.5 code | 2 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I059 | form <CommandBar>: AppearanceMode: the 2.21 value Some("UsualGroup") has no 8.5 code | 2 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I060 | form <LabelField>: CellHyperlinkRepresentation: the 2.21 value Some("DontShow") has no 8.5 code | 2 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I061 | no measured ordinal for AccountingRegister.StandardCommand.OpenList | 2 | same_failure_cf | 8.5.1.1529/corpus-navigation-commands |
| I062 | no measured value for StdPicture.AccountingRegister | 2 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I063 | no measured value for StdPicture.AppearanceStarHalfFilled | 2 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I064 | no measured value for StdPicture.FormattedString | 2 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I065 | no measured value for StdPicture.GotoURL | 2 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I066 | no measured value for StdPicture.QueryWizardTempTableDescription | 2 | same_failure_cf | 8.3.27.2214/corpus-managed-forms |
| I067 | no stored value for StdPicture.GotoURL | 2 | same_failure_cf | 8.3.27.2214/corpus-catalog-pictures |
| I068 | the exporter names no `ExchangePlan` standard command `Create` | 2 | same_failure_cf | 8.5.1.1529/corpus-palette-help-and-interface |
| I069 | unresolved reference -2 | 2 | same_failure_cf | 8.3.27.2214/corpus-orphan-characteristics |
| I070 | unsupported colour "web:MediumGreen" | 2 | same_failure_cf | 8.3.27.2214/corpus-style-colors |
| I071 | unsupported colour "web:Tomato" | 2 | same_failure_cf | 8.3.27.2214/corpus-style-colors |
| I072 | unsupported picture reference 0 | 2 | same_failure_cf | 8.3.27.2214/corpus-orphan-catalog-facets |
| I073 | <AutofillHint>PhoneNumber is not measured | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I074 | <Button> names a colour the writer cannot place | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I075 | <GeographicalSchemaField> has no writer yet | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I076 | <InputField> names an event the writer cannot place | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I077 | <LabelDecoration> names a background colour it cannot place | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I078 | <PDFDocumentField> excludes ConvertToGrayScale, which has no measured uuid | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I079 | <Settings> names <chart:selectionMode>, which the chart writer cannot place | 1 | same_failure_cf | 8.5.1.1529/corpus-chart-settings |
| I080 | <UsualGroup> names a font the writer cannot place | 1 | same_failure_cf | 8.5.1.1529/corpus-legacy-form-properties |
| I081 | <chart:chartType> spells Column, which the chart writer has not measured | 1 | same_failure_cf | 8.5.1.1529/corpus-chart-settings |
| I082 | <chart:chartType> spells Pie3D, which the chart writer has not measured | 1 | same_failure_cf | 8.5.1.1529/corpus-chart-settings |
| I083 | Configuration <InterfaceCompatibilityMode> "TaxiEnableVersion8_5" has no known V76 pair | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I084 | Help: stored-page read-back differs | 1 | same_failure_cf | 8.5.1.1529/corpus-palette-help-and-interface |
| I085 | MXL: <embeddedTable> of a SpreadsheetDocument has no writer | 1 | same_failure_xml_normalized | 8.5.1.1529/corpus-modern-spreadsheet |
| I086 | MXL: the exporter cannot read back the chart the writer built | 1 | same_failure_cf | 8.5.1.1529/corpus-modern-spreadsheet |
| I087 | a <ChoiceList> item names <variant> | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I088 | a table settles into <panel>, which the form does not declare | 1 | same_failure_xml_normalized | 8.5.1.1529/corpus-navigation-commands |
| I089 | a track bar's marking appearance is not measured | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I090 | form <InputField>: CellHyperlinkRepresentation: the 2.21 value Some("DontShow") has no 8.5 code | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I091 | no flowchart code for Shape "VerticalBrackets" | 1 | same_failure_cf | 8.3.27.2214/corpus-template-bodies |
| I092 | no measured ordinal for ChartOfAccounts.StandardCommand.OpenList | 1 | same_failure_cf | 8.5.1.1529/corpus-navigation-commands |
| I093 | no measured uuid for <ExcludedCommand>ChangeHistory</ExcludedCommand> on cfg:ExchangePlanObject | 1 | same_failure_cf | 8.5.1.1529/corpus-navigation-commands |
| I094 | no measured uuid for Form.Item.<name>.StandardCommand.Back on <PlannerField> | 1 | same_failure_cf | 8.5.1.1529/corpus-navigation-commands |
| I095 | no measured uuid for Form.StandardCommand.CreateByParameter on cfg:DynamicList | 1 | same_failure_legacy_xml | 8.5.1.1529/corpus-navigation-commands |
| I096 | no measured value for StdPicture.AppearanceUpArrowGray | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I097 | no measured value for StdPicture.DontNotify | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I098 | no measured value for StdPicture.GrayedAll | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I099 | no measured value for StdPicture.StartVideoconference | 1 | same_failure_cf | 8.5.1.1529/corpus-hybrid-interface |
| I100 | the chart declares 1 series and spells 0 | 1 | same_failure_xml_normalized | 8.5.1.1529/corpus-chart-settings |
| I101 | unsupported Form ExcludedCommand: EditList | 1 | same_failure_legacy_xml | 8.5.1.1529/corpus-navigation-commands |
| I102 | unsupported Form ExcludedCommand: Preview | 1 | same_failure_cf | 8.5.1.1529/corpus-navigation-commands |
| I103 | unsupported colour "web:Linen" | 1 | same_failure_cf | 8.3.27.2214/corpus-style-colors |
| I104 | unsupported enum value colour pal:<name> | 1 | same_failure_cf | 8.5.1.1529/corpus-palette-help-and-interface |

Product revision: `26f116648d70e6d2f0881963a7f957068a084540`. Binary SHA-256: `11175e72ff51cee29bf8d0e17f11b4f681f7d04d8219629fdef0eb8c431a47a2`.
