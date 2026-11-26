# Managed-form round trips on 8.5.1.1529

Refs #680. Fixtures come from 1c-fixtures at 8d138b896f462c3053d16076cbabf0ffe5030c1c. For each Modern, Hybrid and Legacy case the native ibcmd 8.5.1.1529 imported the selected forms into a fresh file infobase, saved a CF and exported XML. Control configurations omitted the property under test. Native storage comparisons used the inflated UUID .0 entry.

| Property | Control → explicit |
| --- | --- |
| ProgressBarField Representation BrokenTilt | 1 → 2 |
| ColumnGroup ShowTitleInCard true | 2 → 1 |
| CommandBar AppearanceMode UsualGroup | 0 → 2 |
| InputField AutofillHint PhoneNumber | 0 → 17 |
| InputField TextSize Reduced | 1 → 2 |
| RadioButtonField HorizontalStretch false | 2 → 0 |
| UsualGroup ControlRepresentation ButtonInParentElement | 0 → 2 |
| TrackBarField MarkingAppearance BothSides | 2 → 3 |
| Form ChildrenAlign TitlesLeftDataAuto | 0 → 6 |

The native inputs also pin GeographicalSchemaField kind 16 and options revision 2, the InputField MultipleValueURLProcessing event UUID, the RowPictureDataPath Active marker -5 on AccumulationRegister record sets, and the command exclusions Preview, ChangeHistory, ConvertToGrayScale and FormattedString. FormattedString stores two exclusion UUIDs.

The converter checks used cf bootstrap --base-free --platform 8.5.1.1529 followed by cf export --source-version 2.21, and infobase config import --no-verify into a fresh empty file infobase followed by config export. For Hybrid and navigation checks the converter used a Taxi configuration shell because the original TaxiEnableVersion8_5 root setting is handled by PR #739. Exact target Form.xml entries were kept. Navigation checks retained the original Report, ExchangePlan and AccumulationRegister objects. Both routes returned the target XML byte-for-byte for the verified fields, root properties, exclusions and row binding. The two ColumnGroup forms matched in XML on both routes and differed in inflated CF payload outside the tested property.

The full FormattedString fixtures still reach an embedded spreadsheet with cells, which the current writer refuses. The embedded-cell writer is tracked in issue #613 and PR #698. Complete acceptance requires rerunning both routes against those exact fixtures with embedded cell support present.

The Modern and Hybrid ChoicePeriod forms also matched byte-for-byte through both routes and in the inflated CF form body. Their stored period is Custom (0) with both date members 00010101000000.
