# The refusal matrix of S1 (#404)

Every case of the probe runs of `restructuring-check.md` (6.1, 6.2) through the check and `apply_check::s1::classify`. Written by `IBCMD_RS_S1_MATRIX_OUT=<file> cargo test -p ibcmd-rs --lib --no-default-features apply_check::s1_matrix_tests::the_matrix_can_be_written_out`.

| case | platform acted | S1 | reasons (rule) |
|---|---|---|---|
| p1 Catalog Autonumbering | no | harmless | - |
| p1 Catalog Explanation | no | harmless | - |
| p1 Catalog ObjectPresentation | no | harmless | - |
| p1 Catalog ListPresentation | no | harmless | - |
| p1 Catalog DescriptionLength | yes | refused property-outside-s1 | property-not-covered |
| p1 Catalog DefaultPresentation | no | harmless | - |
| p1 Catalog QuickChoice | no | harmless | - |
| p1 Catalog ChoiceMode | no | harmless | - |
| p1 Catalog CodeLength | yes | refused property-outside-s1 | property-not-covered |
| p1 Catalog IncludeHelpInContents | no | harmless | - |
| p1 Catalog CheckUnique | no | harmless | - |
| p1 Catalog DataHistory | yes | refused property-outside-s1 | property-not-covered |
| p1 Catalog PredefinedDataUpdate | no | harmless | - |
| p1 Catalog FullTextSearch | no | harmless | - |
| p1 Catalog DataLockControlMode | no | harmless | - |
| p1 Catalog UseStandardCommands | no | harmless | - |
| p1 Catalog ChoiceHistoryOnInput | no | harmless | - |
| p1 Catalog EditType | no | harmless | - |
| p2 attribute Synonym | no | harmless | - |
| p2 attribute ToolTip | no | harmless | - |
| p2 attribute FillChecking | no | harmless | - |
| p2 attribute QuickChoice | no | harmless | - |
| p2 attribute FullTextSearch | no | harmless | - |
| p2 attribute ChoiceHistoryOnInput | no | harmless | - |
| p2 attribute CreateOnInput | no | harmless | - |
| p2 attribute MultiLine | no | harmless | - |
| p2 attribute PasswordMode | no | harmless | - |
| p2 attribute ExtendedEdit | no | harmless | - |
| p2 attribute MarkNegatives | no | harmless | - |
| p2 attribute FillFromFillingValue | no | harmless | - |
| p2 attribute Name | no | harmless | - |
| p2 attribute DataHistory | no | refused attribute-property-outside-s1 | attribute-property-not-covered |
| p2 attribute Indexing | yes | operation switch-index | attribute-property-not-covered |
| p2 attribute StringLength | yes | operation widen-string | attribute-property-not-covered |
| p4 attribute ChoiceFoldersAndItems | no | harmless | - |
| p4 attribute Use | yes | refused attribute-property-outside-s1 | attribute-property-not-covered |
| p4 attribute Indexing off | yes | operation switch-index | attribute-property-not-covered |
| p4 standard attribute FillChecking | no | harmless | - |
| p4 standard attribute MultiLine | no | harmless | - |
| p4 standard attribute FullTextSearch | no | harmless | - |
| p4 standard attribute ExtendedEdit | no | harmless | - |
| p4 standard attribute DataHistory | no | refused property-outside-s1 | standard-attribute-property-not-covered |
| p4 enumeration value added | yes | refused data-change | enum-value-added-or-dropped |
| p4 enumeration value renamed | no | harmless | - |
| p5 attribute added to a Report | no | harmless | - |
| p5 attribute added to a DataProcessor | no | harmless | - |
| p5 tabular section FillChecking | no | harmless | - |
| p5 tabular section attribute MultiLine | no | harmless | - |
| p5 attribute added to a tabular section | yes | operation add-section-attribute | tabular-section-column-added-dropped-moved |
| p5 tabular section added | yes | operation add-tabular-section | tabular-section-added-dropped-moved |
| p5 dimension DenyIncompleteValues | no | harmless | - |
| p5 dimension added to an accumulation register | yes | refused kind-outside-s1 | column-added-or-dropped |
| p5 last attribute of a catalog dropped | yes | operation delete-attribute | column-added-or-dropped |
| s1 attribute added to a Catalog | yes | operation add-attribute | column-added-or-dropped |
| s1 attribute added to a Document | yes | operation add-attribute | column-added-or-dropped |
| p6 Configuration Version | no | harmless | - |
| p6 Configuration UpdateCatalogAddress | no | harmless | - |
| p6 Configuration Copyright | no | harmless | - |
| p6 Configuration BriefInformation | no | harmless | - |
| p6 Configuration Vendor | no | harmless | - |
| p6 Configuration DetailedInformation | no | harmless | - |
| p6 Configuration VendorInformationAddress | no | harmless | - |
| p6 Configuration ConfigurationInformationAddress | no | harmless | - |
| p6 ScheduledJob MethodName | yes | refused data-change | scheduled-job-stored |
| p6 ScheduledJob RestartCountOnFailure | yes | refused data-change | scheduled-job-stored |
| p6 ScheduledJob RestartIntervalOnFailure | yes | refused data-change | scheduled-job-stored |
| p6 EventSubscription Event | no | harmless | - |
| p6 ScheduledJob Comment | no | harmless | - |
| p6 EventSubscription Comment | no | harmless | - |
| p6 WebService Comment | no | harmless | - |
| p6 HTTPService Comment | no | harmless | - |
| p6 XDTOPackage Comment | no | harmless | - |
| p6 SettingsStorage Comment | no | harmless | - |
| p6 Sequence Comment | no | harmless | - |
| p7a CommonModule | no | harmless | - |
| p7a Role | no | harmless | - |
| p7a Catalog | yes | operation add-object | object-with-storage-added-or-dropped, object-with-storage-added-or-dropped |
| p7b CommonModule | no | harmless | - |
| p7b Role | no | harmless | - |
| p7b Catalog | yes | refused object-removed | object-with-storage-added-or-dropped, object-with-storage-added-or-dropped |
| p8 Catalog ExtendedObjectPresentation | no | harmless | - |
| p8 Catalog ExtendedListPresentation | no | harmless | - |
| p8 Catalog FullTextSearchOnInputByString | no | harmless | - |
| p8 Catalog AuxiliaryObjectForm | no | harmless | - |
| p8 Catalog AuxiliaryListForm | no | harmless | - |
| p8 Catalog AuxiliaryChoiceForm | no | harmless | - |
| p8 Catalog AuxiliaryFolderForm | no | harmless | - |
| p8 Catalog AuxiliaryFolderChoiceForm | no | harmless | - |
| p8 Catalog InputByString | no | harmless | - |
| auto AccountingRegister IncludeHelpInContents | no | harmless | - |
| auto AccountingRegister UseStandardCommands | no | harmless | - |
| auto AccumulationRegister RegisterType | yes | refused kind-outside-s1 | property-not-covered |
| auto AccumulationRegister UseStandardCommands | no | harmless | - |
| auto BusinessProcess DefaultObjectForm | no | harmless | - |
| auto BusinessProcess UseStandardCommands | no | harmless | - |
| auto CalculationRegister UseStandardCommands | no | harmless | - |
| auto Catalog Autonumbering | no | harmless | - |
| auto Catalog CheckUnique | no | harmless | - |
| auto Catalog ChoiceHistoryOnInput | no | harmless | - |
| auto Catalog ChoiceMode | no | harmless | - |
| auto Catalog CodeAllowedLength | yes | refused property-outside-s1 | property-not-covered |
| auto Catalog CodeSeries | yes | refused property-outside-s1 | property-not-covered |
| auto Catalog CodeType | yes | refused property-outside-s1 | property-not-covered |
| auto Catalog Comment | no | harmless | - |
| auto Catalog CreateOnInput | no | harmless | - |
| auto Catalog DataHistory | yes | refused property-outside-s1 | property-not-covered |
| auto Catalog DataLockControlMode | no | harmless | - |
| auto Catalog DefaultChoiceForm | no | harmless | - |
| auto Catalog DefaultFolderChoiceForm | no | harmless | - |
| auto Catalog DefaultFolderForm | no | harmless | - |
| auto Catalog DefaultListForm | no | harmless | - |
| auto Catalog DefaultObjectForm | no | harmless | - |
| auto Catalog DefaultPresentation | no | harmless | - |
| auto Catalog EditType | no | harmless | - |
| auto Catalog ExecuteAfterWriteDataHistoryVersionProcessing | no | harmless | - |
| auto Catalog FoldersOnTop | yes | refused property-outside-s1 | property-not-covered |
| auto Catalog FullTextSearch | no | harmless | - |
| auto Catalog Hierarchical | yes | refused property-outside-s1 | property-not-covered |
| auto Catalog HierarchyType | no | refused property-outside-s1 | property-not-covered |
| auto Catalog IncludeHelpInContents | no | harmless | - |
| auto Catalog LimitLevelCount | no | refused property-outside-s1 | property-not-covered |
| auto Catalog PredefinedDataUpdate | no | harmless | - |
| auto Catalog QuickChoice | no | harmless | - |
| auto Catalog SearchStringModeOnInputByString | no | harmless | - |
| auto Catalog UpdateDataHistoryImmediatelyAfterWrite | no | harmless | - |
| auto Catalog UseStandardCommands | no | harmless | - |
| auto ChartOfAccounts UseStandardCommands | no | harmless | - |
| auto ChartOfCalculationTypes UseStandardCommands | no | harmless | - |
| auto ChartOfCharacteristicTypes CodeAllowedLength | yes | refused kind-outside-s1 | property-not-covered |
| auto ChartOfCharacteristicTypes FoldersOnTop | yes | refused kind-outside-s1 | property-not-covered |
| auto ChartOfCharacteristicTypes Hierarchical | yes | refused kind-outside-s1 | property-not-covered |
| auto ChartOfCharacteristicTypes IncludeHelpInContents | no | harmless | - |
| auto ChartOfCharacteristicTypes UseStandardCommands | no | harmless | - |
| auto CommandGroup Category | no | harmless | - |
| auto CommandGroup Representation | no | harmless | - |
| auto CommonAttribute ExtendedEdit | no | refused kind-outside-s1 | property-not-covered |
| auto CommonAttribute FillFromFillingValue | no | refused kind-outside-s1 | property-not-covered |
| auto CommonAttribute MarkNegatives | no | refused kind-outside-s1 | property-not-covered |
| auto CommonAttribute MultiLine | no | refused kind-outside-s1 | property-not-covered |
| auto CommonAttribute PasswordMode | no | refused kind-outside-s1 | property-not-covered |
| auto CommonCommand Comment | no | harmless | - |
| auto CommonCommand Group | no | harmless | - |
| auto CommonCommand IncludeHelpInContents | no | harmless | - |
| auto CommonCommand ModifiesData | no | harmless | - |
| auto CommonCommand ParameterUseMode | no | harmless | - |
| auto CommonCommand Representation | no | harmless | - |
| auto CommonForm Comment | no | harmless | - |
| auto CommonForm IncludeHelpInContents | no | harmless | - |
| auto CommonForm UseStandardCommands | no | harmless | - |
| auto CommonModule ClientManagedApplication | no | harmless | - |
| auto CommonModule ClientOrdinaryApplication | no | harmless | - |
| auto CommonModule Comment | no | harmless | - |
| auto CommonModule ExternalConnection | no | harmless | - |
| auto CommonModule Global | no | harmless | - |
| auto CommonModule Privileged | no | harmless | - |
| auto CommonModule ReturnValuesReuse | no | harmless | - |
| auto CommonModule Server | no | harmless | - |
| auto CommonModule ServerCall | no | harmless | - |
| auto CommonPicture AvailabilityForAppearance | no | harmless | - |
| auto CommonPicture AvailabilityForChoice | no | harmless | - |
| auto CommonPicture Comment | no | harmless | - |
| auto CommonTemplate Comment | no | harmless | - |
| auto CommonTemplate TemplateType | no | harmless | - |
| auto Constant Comment | no | harmless | - |
| auto Constant DataLockControlMode | no | harmless | - |
| auto Constant DefaultForm | no | harmless | - |
| auto Constant ExecuteAfterWriteDataHistoryVersionProcessing | no | harmless | - |
| auto Constant ExtendedEdit | no | harmless | - |
| auto Constant FillChecking | no | harmless | - |
| auto Constant MarkNegatives | no | harmless | - |
| auto Constant MultiLine | no | harmless | - |
| auto Constant PasswordMode | no | harmless | - |
| auto Constant UpdateDataHistoryImmediatelyAfterWrite | no | harmless | - |
| auto Constant UseStandardCommands | no | harmless | - |
| auto DataProcessor Comment | no | harmless | - |
| auto DataProcessor DefaultForm | no | harmless | - |
| auto DataProcessor IncludeHelpInContents | no | harmless | - |
| auto DataProcessor UseStandardCommands | no | harmless | - |
| auto Document Autonumbering | no | harmless | - |
| auto Document CheckUnique | yes | refused property-outside-s1 | property-not-covered |
| auto Document DataLockControlMode | no | harmless | - |
| auto Document DefaultChoiceForm | no | harmless | - |
| auto Document DefaultListForm | no | harmless | - |
| auto Document DefaultObjectForm | no | harmless | - |
| auto Document ExecuteAfterWriteDataHistoryVersionProcessing | no | harmless | - |
| auto Document IncludeHelpInContents | no | harmless | - |
| auto Document NumberAllowedLength | yes | refused property-outside-s1 | property-not-covered |
| auto Document NumberPeriodicity | yes | refused property-outside-s1 | property-not-covered |
| auto Document PostInPrivilegedMode | no | harmless | - |
| auto Document Posting | yes | refused property-outside-s1 | property-not-covered |
| auto Document RealTimePosting | no | harmless | - |
| auto Document RegisterRecordsDeletion | no | harmless | - |
| auto Document RegisterRecordsWritingOnPost | no | harmless | - |
| auto Document SequenceFilling | no | harmless | - |
| auto Document UnpostInPrivilegedMode | no | harmless | - |
| auto Document UpdateDataHistoryImmediatelyAfterWrite | no | harmless | - |
| auto Document UseStandardCommands | no | harmless | - |
| auto DocumentJournal DefaultForm | no | harmless | - |
| auto DocumentJournal IncludeHelpInContents | no | harmless | - |
| auto DocumentJournal UseStandardCommands | yes | refused kind-outside-s1 | property-not-covered |
| auto Enum ChoiceHistoryOnInput | no | harmless | - |
| auto Enum ChoiceMode | no | harmless | - |
| auto Enum Comment | no | harmless | - |
| auto Enum QuickChoice | no | harmless | - |
| auto Enum UseStandardCommands | no | harmless | - |
| auto EventSubscription Comment | no | harmless | - |
| auto ExchangePlan DefaultObjectForm | no | harmless | - |
| auto ExchangePlan DistributedInfoBase | no | refused kind-outside-s1 | property-not-covered |
| auto ExchangePlan IncludeHelpInContents | no | harmless | - |
| auto ExchangePlan QuickChoice | no | harmless | - |
| auto ExchangePlan UseStandardCommands | no | harmless | - |
| auto FilterCriterion UseStandardCommands | no | harmless | - |
| auto FunctionalOption Comment | no | harmless | - |
| auto FunctionalOption Location | no | harmless | - |
| auto FunctionalOption PrivilegedGetMode | no | harmless | - |
| auto InformationRegister Comment | no | harmless | - |
| auto InformationRegister DataLockControlMode | no | harmless | - |
| auto InformationRegister DefaultListForm | no | harmless | - |
| auto InformationRegister DefaultRecordForm | no | harmless | - |
| auto InformationRegister EditType | no | harmless | - |
| auto InformationRegister EnableTotalsSliceFirst | no | refused kind-outside-s1 | property-not-covered |
| auto InformationRegister EnableTotalsSliceLast | no | refused kind-outside-s1 | property-not-covered |
| auto InformationRegister ExecuteAfterWriteDataHistoryVersionProcessing | no | harmless | - |
| auto InformationRegister FullTextSearch | no | harmless | - |
| auto InformationRegister IncludeHelpInContents | no | harmless | - |
| auto InformationRegister InformationRegisterPeriodicity | yes | refused kind-outside-s1 | property-not-covered |
| auto InformationRegister MainFilterOnPeriod | no | refused kind-outside-s1 | property-not-covered |
| auto InformationRegister UpdateDataHistoryImmediatelyAfterWrite | no | harmless | - |
| auto InformationRegister UseStandardCommands | no | harmless | - |
| auto Report Comment | no | harmless | - |
| auto Report DefaultForm | no | harmless | - |
| auto Report DefaultSettingsForm | no | harmless | - |
| auto Report IncludeHelpInContents | no | harmless | - |
| auto Report MainDataCompositionSchema | no | harmless | - |
| auto Report UseStandardCommands | no | harmless | - |
| auto Role Comment | no | harmless | - |
| auto ScheduledJob Comment | no | harmless | - |
| auto ScheduledJob Description | yes | refused data-change | scheduled-job-stored |
| auto ScheduledJob Key | yes | refused data-change | scheduled-job-stored |
| auto ScheduledJob Predefined | yes | refused data-change | scheduled-job-stored |
| auto ScheduledJob Use | yes | refused data-change | scheduled-job-stored |
| auto SessionParameter Comment | no | harmless | - |
| auto StyleItem Comment | no | harmless | - |
| auto Subsystem Comment | no | harmless | - |
| auto Subsystem IncludeHelpInContents | no | harmless | - |
| auto Subsystem IncludeInCommandInterface | no | harmless | - |
| auto Subsystem UseOneCommand | no | harmless | - |
| auto Task UseStandardCommands | no | harmless | - |
