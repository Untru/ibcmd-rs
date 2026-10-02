//! The platform's standard pictures (`StdPicture.<Name>`): one table of their
//! identifiers and the names the platform writes for them, for every reader
//! and writer of the crate.
//!
//! A standard-picture identifier is a platform constant, not a
//! per-configuration calibration: the same uuid names the same picture in
//! every configuration and every version measured. The table serves both
//! directions -- the exporters name a uuid they find in a stored record
//! (forms, common commands, the commands of objects and registers,
//! subsystem and command-group pictures, help pages) and the writers store a
//! name back as its uuid (help pages, pictures of forms and commands, the
//! base-free descriptor compiler).
//!
//! The table used to exist twice -- here under `mssql_dump` and a copy under
//! `metadata_model::objects::export` -- and the copies drifted: the object
//! and register exporters failed with "no name for picture" on the twelve
//! identities only the form exporter knew (Untru/ibcmd-rs#413). Every row
//! carries the evidence it was read off, with the observation count where
//! one was taken.

use std::collections::HashMap;
use std::sync::LazyLock;

#[rustfmt::skip]
pub(crate) const STANDARD_PICTURES: &[(&str, &str)] = &[
    ("4b54770b-d069-4c0e-9b17-5cc2a01134d9", "StdPicture.Information"),
    ("818ab7d0-4654-4542-bd5e-fd9d1352b5a1", "StdPicture.SaveFile"),
    ("6ff3ddbd-56e3-4ddf-a5bf-048c1e2dfb2f", "StdPicture.User"),
    // Монитор `Catalogs/Триггеры` (list form) and
    // `DataProcessors/График/Forms/Форма`.
    (
        "cf10e497-9779-44a7-82d8-811b88193215",
        "StdPicture.AppearanceCircleBlack",
    ),
    (
        "bbd37b6b-2742-48f3-9efa-d7c245f125b0",
        "StdPicture.AppearanceFlagGreen",
    ),
    // Монитор `Catalogs/Запросы/Forms/ФормаАнализа`, three commands.
    (
        "7a2a6f5c-7677-45e1-a2d3-ce2444d99a63",
        "StdPicture.AppearanceDownTriangleRed",
    ),
    (
        "8da615a0-65a5-4511-a8d4-00dc156478fb",
        "StdPicture.AppearanceUpTriangleGreen",
    ),
    (
        "879ca050-8650-4135-b799-f3ef9e00a65a",
        "StdPicture.AppearanceBoxesEmpty",
    ),
    // Монитор `Catalogs/Алгоритмы/Forms/ФормаЭлемента`, two buttons.
    (
        "f16ab927-3ac6-4cdd-bcf1-d8acf005a255",
        "StdPicture.UserWithoutNecessaryProperties",
    ),
    ("283ecabd-aaed-41d1-ad46-6cca91c29120", "StdPicture.LoadReportSettings"),
    ("942e0303-a3ec-4fe8-887c-5aea8516d424", "StdPicture.ReportSettings"),
    // Platform 8.5.1.1150 BSP: two forms, two references each.
    ("6a248caf-0a7b-46ad-a595-74890ea202f7", "StdPicture.Conversations"),
    ("5b87ad1b-d8cc-43c1-b5c4-dc43613c518c", "StdPicture.InformationRegister"),
    ("a064544f-6037-48ca-b19f-8ad63e43af23", "StdPicture.ShowData"),
    ("f04794cb-c198-4172-86c3-649386013c85", "StdPicture.CustomizeList"),
    ("97b2cc97-d5c6-45fb-9824-9d6d73db21fe", "StdPicture.Change"),
    ("37cf7cc0-abad-4385-b597-6fd2d8dc085a", "StdPicture.Task"),
    ("2f130057-bb2a-4e22-bba5-e108fac26940", "StdPicture.ChooseValue"),
    ("47f01799-7968-4f44-9acc-fe1bdde8beb2", "StdPicture.ActiveUsers"),
    ("e8a49985-fef7-45a9-b6bb-ddd2b9028172", "StdPicture.DataHistory"),
    ("a24cff7f-a1a5-4403-af82-a7b31852cde9", "StdPicture.BusinessProcessObject"),
    ("f6532868-30b9-44ab-803c-78f0f0b06b02", "StdPicture.CloneObject"),
    ("448d6f55-d885-496c-870d-d1bd78374745", "StdPicture.CloneListItem"),
    ("977e831a-0e73-4d60-af51-091a6fa8612e", "StdPicture.CreateListItem"),
    ("0e2da390-5c04-46a2-a74b-1b7e13a40f2b", "StdPicture.HidePassword"),
    ("97f87955-b88a-4225-a0d8-03af981ecd86", "StdPicture.ShowPassword"),
    ("fc6a06a8-1308-4385-b1b2-9d302d2054ed", "StdPicture.SearchControl"),
    ("723765ab-0b92-4745-a621-1ba0f77c92c9", "StdPicture.EventLog"),
    ("4fddea39-5129-4b4c-83fe-4e443cd61940", "StdPicture.EventLogByUser"),
    ("ffab30f1-da11-44b5-b34c-24da22badcf4", "StdPicture.Find"),
    ("785362cb-3756-48ed-87d2-292ded17054a", "StdPicture.OpenFile"),
    ("4d2570b5-205f-413c-b4cc-b2097f61684f", "StdPicture.CreateInitialImage"),
    ("0ce78048-0196-4f80-a781-9829cdb7f43e", "StdPicture.GenerateReport"),
    // Read straight off the platform: the subsystem picture slot carries
    // this identifier and the platform names it `StdPicture.Dendrogram` for
    // the same object. A standard-picture identifier is a platform
    // constant, not a per-configuration calibration.
    ("4bf9fbb5-53c5-4b09-bab2-d69bbfab945b", "StdPicture.Dendrogram"),
    ("18492a87-2fe4-44af-b218-304897fed020", "StdPicture.MarkToDelete"),
    ("20ebc47b-f4d9-439c-acd3-fdc624fbac2a", "StdPicture.Post"),
    ("23f940bf-7381-4c2b-85a1-e541ed428042", "StdPicture.SaveValues"),
    ("a7707ed1-39b0-418f-974d-4d500d27a9c6", "StdPicture.RestoreValues"),
    ("8f29e0e2-d5e6-41e8-a34d-9a0288156322", "StdPicture.Reread"),
    ("db817ee1-fd28-4e7f-bb4a-53686b2b153c", "StdPicture.Report"),
    ("1970a480-9b38-405e-9d9e-8209f3fad5f1", "StdPicture.ScheduledJob"),
    ("58174855-39be-462e-8723-cb2d95182146", "StdPicture.SetDateInterval"),
    ("2ef82795-06fe-4365-bd0c-44b486264620", "StdPicture.FilterCriterion"),
    ("b1406535-6cc2-4410-95ea-753556e8460f", "StdPicture.FilterByCurrentValue"),
    ("479470e0-ea0f-4266-8549-e2b1e8c06534", "StdPicture.ClearFilter"),
    ("fb7e9fb5-110b-41cb-adc6-753969ae1c81", "StdPicture.ExpandAll"),
    ("27ee3053-952c-49e5-8261-9215098e0e9c", "StdPicture.CollapseAll"),
    ("5289d9a4-b012-4d54-9bce-50473fe29b57", "StdPicture.DialogExclamation"),
    ("55ef0776-5ee4-4daf-9a9b-70d63643ab8d", "StdPicture.SetTime"),
    ("fc4f29e0-d168-4fe0-8e64-e982fabf2595", "StdPicture.Refresh"),
    ("91022b99-b610-48ad-954e-a297848081ce", "StdPicture.SortListAsc"),
    ("1fa32fdb-a180-418f-a6eb-db7516b7a30b", "StdPicture.SortListDesc"),
    ("894afc03-9904-465d-b671-f555ffb9b21c", "StdPicture.Document"),
    ("1cd7b762-ec6a-4e92-ac9a-1832be228ec3", "StdPicture.Stop"),
    ("8ca4ea33-603d-4992-8a41-c7924b5bd40b", "StdPicture.UndoPosting"),
    ("894cf65b-4109-4533-a1d7-c87b1fcc80a3", "StdPicture.Write"),
    ("e6fc55a0-3d58-4b15-bdd3-717453929598", "StdPicture.WriteAndClose"),
    ("08a45a70-c221-4339-b3b1-9f11cb22147d", "StdPicture.Delete"),
    ("6e3687cf-a8d1-446a-833a-bfaf38516353", "StdPicture.SwitchActivity"),
    ("7a9cd2fd-6372-4342-9a9e-3ebbd754fd83", "StdPicture.AppearanceCheckBox"),
    ("0c1f7756-6143-4903-a94c-8f22c85e44de", "StdPicture.Attribute"),
    ("3c904ff7-1195-4a7c-9a38-7b1f6ca49cce", "StdPicture.Back"),
    ("509c4a7f-6406-4388-bb8c-bc81fb5131aa", "StdPicture.BusinessProcess"),
    ("97c5a6d5-47ed-43f9-8c8c-10e9903c23d2", "StdPicture.Calendar"),
    ("4ab0e87f-7d9b-4aa8-ac4b-680a78522da8", "StdPicture.CreateFolder"),
    ("ee7c4a5b-2d9b-4087-ae3e-947792085f09", "StdPicture.DataCompositionOutputParameters"),
    ("544fdbe8-5956-4512-bc62-93b4c022d291", "StdPicture.ExchangePlan"),
    ("003024ed-fa25-42ac-9f53-f5014e383801", "StdPicture.ExecuteTask"),
    ("1a4342a5-fa06-4556-8a85-e8738fc25821", "StdPicture.GetURL"),
    ("3d4ad3b1-17de-4cf1-a2e4-0c2c83a5b5c2", "StdPicture.ListViewModeHierarchicalList"),
    ("64837726-d2a2-4682-a788-737423e80013", "StdPicture.Picture"),
    ("b4c7ab2c-bcda-4468-a28f-5fee93838c4e", "StdPicture.Properties"),
    ("b5a0aaba-3a83-4a71-b6f9-24aae1574681", "StdPicture.SaveReportSettings"),
    ("be23a908-fe1b-44df-be94-d0f6e8353abe", "StdPicture.SendMessage"),
    ("03665ff1-3a05-41d1-96d3-04bda2d8ede3", "StdPicture.SpreadsheetInsertComment"),
    ("aa96f4bb-cf28-4dad-bc42-5ed53de95c0c", "StdPicture.SpreadsheetDeletePageBreak"),
    ("2846af8d-af84-47e3-82b9-01b01f960426", "StdPicture.SpreadsheetReadOnly"),
    ("3bdc16c8-6a96-4467-9442-a8e4804b3fa2", "StdPicture.SyncContents"),
    ("8bdf1079-8fad-4d21-ad7f-4b2e4ecdce3d", "StdPicture.DialogInformation"),
    ("60643198-e4b2-4c39-9de1-53cca3fff382", "StdPicture.DeleteDirectly"),
    ("9fecbaff-2a05-4da6-9ef1-807e754b928d", "StdPicture.BusinessProcessStart"),
    ("83db1f8a-41bd-4016-bdb2-a28e3a8d6dcc", "StdPicture.DialogStop"),
    ("01ec9d9a-7497-4d88-b93f-066c633a4866", "StdPicture.InputOnBasis"),
    ("c7cdd3c0-3879-436a-b145-5e2615e9b3e1", "StdPicture.FindInList"),
    ("984b0a3e-daa2-4a9e-b75c-1f230a6e592a", "StdPicture.DataCompositionConditionalAppearance"),
    ("ef27ae9e-7040-4374-b93c-0d276de2ea23", "StdPicture.DialogQuestion"),
    ("9c96aa25-d656-4b3d-ab3e-81d9718da238", "StdPicture.ShowInList"),
    ("549a2c45-4fce-493f-94ee-9a3a4f426551", "StdPicture.ListViewMode"),
    ("b39aa431-a32f-4447-984a-45606474c82d", "StdPicture.AppearanceExclamationMarkIcon"),
    ("c757209d-a87f-4410-b1a3-76000178f1f0", "StdPicture.ListViewModeList"),
    ("501b8c1d-8062-408e-bde8-b6549324713e", "StdPicture.AppearanceExclamationMark"),
    ("ed0bec43-4633-416c-8c08-0384ca444e32", "StdPicture.EndEdit"),
    ("0abdab67-5c90-4296-8168-239d22024d11", "StdPicture.PrintImmediately"),
    ("31b93f03-0ba2-4631-a171-0d3a3d2ecc48", "StdPicture.ListSettings"),
    ("37e91e77-93ce-4c3b-8d30-a9d8cfd3d3b0", "StdPicture.MoveItem"),
    ("5182f57f-e834-4d11-9c9f-4aedc002b6e9", "StdPicture.FixTable"),
    ("affb1617-24bc-4170-9c84-0902cc3ef206", "StdPicture.DataCompositionSettingsWizard"),
    ("ad8cb448-a6bb-43b4-886a-7d6a8367eef2", "StdPicture.DataCompositionGroupFields"),
    // Read off native ERP УХ 3.3.3.3 form command pictures, where the
    // platform writes the name for the same descriptor uuid this export
    // wrote through as a bare `0:<uuid>` reference.
    ("021c20a0-071b-4a60-8e44-12487adde0c8", "StdPicture.EditInDialog"),
    ("64ca52ee-f1a3-468f-8055-311935077515", "StdPicture.QueryWizardTempTable"),
    ("a9481ba4-dc85-4112-9c50-f9f340a61298", "StdPicture.QueryWizardReplaceTable"),
    // Same reading, from the attribute picture of
    // `Catalogs/ПоляНаборовДанныхМСФО`, the one place ERP УХ names it.
    ("f695666a-bad9-49f6-ab7c-5198d7ea4739", "StdPicture.CustomExpression"),
    ("7df3febb-2640-41b7-ad8b-7a23b7ad4aec", "StdPicture.QueryWizardCreateTempTableDropQuery"),
    ("8ac19694-383a-457a-b050-0a3ee937f5f3", "StdPicture.DataCompositionNewNestedScheme"),
    ("6cb69e7f-fe19-4f64-bfb5-1a4fad6c2ef9", "StdPicture.Replace"),
    ("6c2759b1-2b63-4fa5-8d13-75786c7e1e89", "StdPicture.DataCompositionNewTable"),
    ("caf2e58b-ca3d-4b63-82c9-f21f1c9bc9eb", "StdPicture.Setting"),
    ("069b8324-7c51-4c73-a6a8-c06d4fc383b5", "StdPicture.Catalog"),
    ("31bf709f-3b50-4137-9b51-ebc7fb802a7c", "StdPicture.GotoExternalURL"),
    ("1377931c-5744-4948-bade-cb35117b5f63", "StdPicture.Close"),
    ("d90a7482-9a1d-4d3d-ae96-6db440214d96", "StdPicture.DataCompositionFilter"),
    ("2a0c2238-cb59-4473-ada6-352b60f3c0a9", "StdPicture.AddListItem"),
    ("b7c81c62-d6ad-4eae-9cea-0e203182db67", "StdPicture.FormHelp"),
    ("c2e2d966-5b7f-4699-903b-28a6f50d5471", "StdPicture.OutputList"),
    ("9e808d29-787b-4825-863a-13c6844ce91d", "StdPicture.CancelSearch"),
    ("eb47324b-85f9-4172-9315-bba8015d9970", "StdPicture.NewWindow"),
    ("dcd23a32-5c7c-43f2-9021-80d98128556f", "StdPicture.CheckSyntax"),
    ("e93f538e-dfaf-4a91-a9b6-c053555bcf60", "StdPicture.Constant"),
    ("a075c3ef-bc4e-4c96-bdad-2245ec09c28e", "StdPicture.DataCompositionDataParameters"),
    ("b0dd988f-2d9f-4364-b1f4-a4d5f45ffb78", "StdPicture.AppearanceFlagRed"),
    ("b68eb29c-2372-46e1-b84e-13843899ccf6", "StdPicture.DataCompositionUserFields"),
    ("9ef73565-2250-4a35-9fb3-470bd19ca9ca", "StdPicture.AppearanceCrossIcon"),
    ("c78c9f3d-e92c-4f38-bb72-d8bd7fa5dbe3", "StdPicture.Dimension"),
    ("7c75b1df-1fdc-471f-aae6-6b7870318cd4", "StdPicture.SpreadsheetDeleteComment"),
    ("892196a9-c94f-4e50-8224-3c0cea4ea6b8", "StdPicture.GoBack"),
    ("1001ae3e-9289-4303-9699-3c0c17e20e61", "StdPicture.AddToFavorites"),
    ("14b24498-e49c-4713-be64-75101d0abfb9", "StdPicture.GoToEnd"),
    ("167a160b-fa48-4337-87ab-7e0fe95c4b5a", "StdPicture.DeleteListItem"),
    // Read straight off the platform: ERP УХ 3.2.12.6
    // `CommandGroups/ПерейтиУХ` carries the picture descriptor
    // `{4,1,{0,dfcd2d21-…},"",-1,-1,1,0,""}` and the platform names it
    // `StdPicture.FunctionMenuCommand`. Unnamed, the descriptor collapsed
    // to `<Picture/>` and the group lost its picture block whole.
    ("dfcd2d21-24ea-4b27-ab9a-6bf754577536", "StdPicture.FunctionMenuCommand"),
    // Read straight off the platform: ERP УХ 3.2.12.6
    // `Catalogs/ХранимыеФайлыОрганизаций/Ext/Help/ru.html` embeds this
    // identifier in an `<img src="../../mdpicture/id…">` and the platform's
    // own page names it `StdPicture.DeleteListItemDirectly` in that very
    // spot. A standard-picture identifier is a platform constant, not a
    // per-configuration calibration.
    ("6cbf8f9a-3d2f-427b-bfce-5e2bc7a8589d", "StdPicture.DeleteListItemDirectly"),
    ("1f046bc2-d6c5-46a3-a459-b2c0508f86fb", "StdPicture.QueryWizard"),
    ("2c732bfa-f734-48bc-a18b-7554db8a3888", "StdPicture.DataCompositionSelection"),
    ("38bbcebe-e456-461b-8457-07c9a72344a3", "StdPicture.FindInTree"),
    ("4bf588d5-b8d8-47fd-8f41-eb9b668981ce", "StdPicture.SetListItemDeletionMark"),
    ("5b612c21-e223-4997-9e61-86f7a67ec945", "StdPicture.ExternalDataSourceTable"),
    ("6206a729-16e5-4e32-b53c-122de4e30c8d", "StdPicture.ScheduledJobs"),
    ("6511326b-20c3-4bf8-8503-c2c2c9072c6c", "StdPicture.CustomizeForm"),
    ("6b909f65-95a4-4697-8ca0-c8f331227b9a", "StdPicture.SettingsStorage"),
    ("6ecee038-9722-4d80-bb91-7ee7046ec4c7", "StdPicture.Help"),
    ("7168f070-087c-448e-ae3a-6740f424076a", "StdPicture.ChooseFromList"),
    ("732f9dd3-5baf-47ff-af7b-edfe16dac2a1", "StdPicture.DataCompositionNewChart"),
    ("7562cef7-0e57-4f63-a754-b61128a4f3ae", "StdPicture.GoForward"),
    ("75a40cc4-c719-4c3f-91ea-fc5787bc34ca", "StdPicture.UserWithAuthentication"),
    ("77180b5e-8faa-4712-a788-e9f8903e3419", "StdPicture.DataCompositionNewGroup"),
    ("efda7350-6cd7-4416-b188-f5ca9baf66c2", "StdPicture.DataCompositionOrder"),
    ("83c8f18d-8701-41f3-bef4-53f88adbb868", "StdPicture.ReadChanges"),
    ("85cc7dd0-44fc-41aa-967f-f52f202ee2e6", "StdPicture.GoToBegin"),
    ("928075d1-b90b-416c-b0b2-c3104cf084aa", "StdPicture.Notifications"),
    ("fc34a694-e99b-4d1c-a526-63f5571bdb09", "StdPicture.Form"),
    // The 41 further platform picture identities managed-form controls name.
    // Each was traced from the reference tuple of a `<Picture>`-valued slot
    // to the `<xr:Ref>` the platform writes at that exact owner, over all
    // 5 200 forms of the reference tree: every one of them is a total
    // function of the uuid with a single native spelling and no
    // counter-example, and none of the 41 uuids nor any of their names
    // occurs anywhere else in this crate, so no sibling table disagreed.
    // Counts are the observations behind each row.
    ("85998f14-805b-4e2b-ba19-9d79b0464042", "StdPicture.AppearanceCheckIcon"), // 60
    ("c283cd1c-3187-451d-8ef2-7df55daeef06", "StdPicture.History"),             // 18
    ("c1a61df2-f280-49d0-a8b3-7e5fc6f56ff7", "StdPicture.AppearanceCircleRed"), // 17
    ("b2202798-23e0-4165-9982-24878f432488", "StdPicture.AppearanceCross"),     // 17
    ("71cbcb5c-f3f0-4ffd-a4d0-19b802b5ed6b", "StdPicture.AppearanceCircleGreen"), // 15
    ("e51185a4-d915-45b8-b201-1c46cc2d8104", "StdPicture.DocumentJournal"),     // 12
    ("a6cbfd77-fcf0-40f4-a8de-ee0d3e580fe6", "StdPicture.DataProcessor"),       // 11
    ("fada8a16-8b14-4151-87a9-775099f37832", "StdPicture.Calculator"),          // 10
    ("8d7e5026-9c1c-4542-bec0-2b729c84e139", "StdPicture.AppearanceCircleYellow"), // 9
    ("c7f70aa3-b944-4efe-97e3-0fa3bda3cd88", "StdPicture.RotateClockwise"),     // 8
    ("a43fcd1b-ad8d-4318-9d55-fd1ba086e65b", "StdPicture.RotateCounterclockwise"), // 8
    ("f874b0cc-db1d-4577-8c77-d4ba206eb05d", "StdPicture.Forward"),             // 6
    ("2721abfb-fbff-4a3a-98ac-b7c9eb29cd85", "StdPicture.AppearanceCircleEmpty"), // 6
    ("da9ac044-0ff7-4bcf-a441-3187bd1d951f", "StdPicture.ListViewModeTree"),    // 5
    ("ed067d76-b144-4d00-bb36-d1833dd1350c", "StdPicture.DebitCredit"),         // 5
    ("fad46a2b-2e56-47cc-b90c-3c2d4b061937", "StdPicture.ExternalDataSourceCube"), // 4
    ("05612131-3e11-49c0-9592-07e6d9318ef7", "StdPicture.FindNext"),            // 3
    ("87d032df-0956-47e9-bead-4e15330f1983", "StdPicture.AppearanceUpArrowGreen"), // 3
    ("78da2c47-172f-4d57-ab52-a06e40548136", "StdPicture.Message"),             // 2
    ("251aaa98-0127-44c3-a163-6f5ab4367ee2", "StdPicture.DataCompositionStandardSettings"), // 2
    ("70f51581-87b6-41cb-a21b-c9dcdcc7fa93", "StdPicture.AccumulationRegister"), // 2
    ("f6e88116-03d8-4400-9d88-791895d7031a", "StdPicture.Attach"),              // 2
    ("9cf611dc-2370-4357-910d-a2b49c7a1ec6", "StdPicture.Next"),                // 2
    ("55bc1099-a7df-4d0a-b332-a45f0473b368", "StdPicture.Credit"),              // 2
    ("196622f7-0941-435b-992b-722f3082adf4", "StdPicture.Debit"),               // 2
    ("2a7e58e2-a6c5-4387-a459-5249c441947a", "StdPicture.GroupConversation"),   // 2
    ("788667db-61c9-45f3-9c4f-5f660ecdf3e1", "StdPicture.AppearanceCircleFilled"), // 2
    ("e3b38083-0191-4a10-8f5b-51571f2419b4", "StdPicture.FindPrevious"),        // 1
    ("e3b29b1d-4694-4f56-8d55-922f83afed7a", "StdPicture.AppearanceDownArrowGray"), // 1
    ("fc058833-e57f-4f93-ba7a-803992a65c3e", "StdPicture.AppearanceCircleOneFourthFilled"), // 1
    ("a722bc14-4edb-4eed-84b9-5d9b2b443e04", "StdPicture.CollaborationSystemUser"), // 1
    ("2954e819-f3fc-40de-9769-292efce9a355", "StdPicture.ExternalDataSourceFunction"), // 1
    ("f62488ee-f90c-47f7-929d-f42ec11a1e63", "StdPicture.WriteChanges"), // 1
    ("cb34c423-3d6a-4202-a809-3b3f45fb14ab", "StdPicture.LevelUp"),      // 1
    ("c38cc4cf-111d-4bc8-8dcb-4464e2ddfb25", "StdPicture.Favorites"),    // 1
    ("35bc8caa-f7ce-4158-87da-d9bf785afa39", "StdPicture.DataSearch"),   // 1
    ("d35bd799-1cc3-44d1-8ae3-09755a09d44b", "StdPicture.DataCompositionFilterDisabled"), // 1
    ("a9152be7-62cf-4523-be34-a23f018f497e", "StdPicture.GeographicalSchema"), // 1
    ("fe740df0-d828-4241-a12f-7414e12302e8", "StdPicture.QueryWizardTableParameters"), // 1
    ("01743054-d102-4e7c-bf15-5ed7fd84441b", "StdPicture.LevelDown"), // 1
    ("0bac63da-5b4e-48af-b593-7c5d29663e83", "StdPicture.FilterByType"), // 1
    // The last nineteen platform picture uuids the reference trees name and
    // this table did not carry.  Each was read off the ERP УХ 3.2.12.6
    // native bodies directly: at the very position where our export writes
    // the dangling `0:<uuid>` spelling the platform writes the name below,
    // over 58 aligned occurrences in 41 form bodies, and no uuid is ever
    // seen against two different names.
    ("73af51dd-6cda-48be-a093-5a7161c60c77", "StdPicture.FilterAndSort"), // 19
    ("ccb3d8f7-6da2-4c65-aba6-17b2ffbba78c", "StdPicture.ChartOfAccounts"), // 9
    ("835db646-1531-494b-b7c1-3239b0080bcb", "StdPicture.Parameters"),    // 8
    ("46598f81-5f95-4485-9b33-bfe4fd1276d0", "StdPicture.SpreadsheetShowHeaders"), // 3
    ("52b637e5-f95f-4c70-9a72-2a4b5a9df449", "StdPicture.NestedTable"),   // 2
    ("584b470d-ba34-4b25-9620-8de4066ffeaa", "StdPicture.Previous"),      // 2
    ("fa67cb81-8d56-4534-90bd-b62fb0dbf5f0", "StdPicture.GanttChart"),    // 2
    ("f3b8f300-5a54-4eea-8136-5798413a479c", "StdPicture.CalculationRegister"), // 2
    ("92e24ce1-3917-4ee4-bbde-adce48b6c96b", "StdPicture.AppearanceUpInclineArrowGray"), // 1
    ("20b82e97-5fcc-4c68-8e0d-d01060847520", "StdPicture.AppearanceRightArrowGray"), // 1
    ("a30ab2ef-6076-457d-9293-44edc7c6767e", "StdPicture.AppearanceDownInclineArrowGray"), // 1
    ("ba592483-bc90-4e26-ba4d-2126359c6529", "StdPicture.AppearanceBoxesFilled"), // 1
    ("3689585c-a3e2-45d0-a302-caeb31b78835", "StdPicture.AppearanceStarFilled"),  // 1
    ("d66b6f73-53b8-49b9-8efc-33c54aa06e3f", "StdPicture.Notify"),                // 1
    ("702a9e16-0bb6-4efb-af11-10faf1e6ee87", "StdPicture.SpreadsheetShowGroups"), // 1
    ("e96de06b-fa83-48cf-b033-190a249855c9", "StdPicture.GraphicalSchema"),       // 1
    ("a594c8a1-7218-420a-860f-7b493c5e65c4", "StdPicture.Sort"),                  // 1
    ("093dd4ed-e03c-4fc6-a95a-01f51379cccf", "StdPicture.ActivateTask"),          // 1
    ("26518e18-e364-475a-8026-e41134658b2a", "StdPicture.SpreadsheetInsertPageBreak"), // 1
    // These identities complete the same corpus-wide table for the two
    // picture consumers that previously kept separate registries. `Chart`
    // occurs four times in graphical schemes and once in a managed form;
    // `Resource` occurs on two managed-form owners. Every occurrence has
    // one native spelling and neither UUID is observed with another name.
    ("f3c1376a-d2ee-46c4-9e44-aa2f7dae31c4", "StdPicture.Chart"), // 5
    ("d6eefec0-792a-4720-8933-e2a57f9e312c", "StdPicture.Resource"), // 2
    ("da0c4924-973c-4ef0-9dcf-f1fc3307e5e2", "StdPicture.ChangeListItem"), // 19
    // ИТК query-wizard and editor forms (8.3.27.2214 dumps of three
    // extensions, counts over them); each identity has one native spelling.
    ("a119150f-6c0c-4a94-97b0-5f08d7ebd6f5", "StdPicture.HierarchicalView"), // 3
    ("18bca3d7-a7a5-41df-a180-4dff9c217f43", "StdPicture.QueryWizardCreateNestedQuery"), // 33
    ("7604cff7-5cc6-4f88-8d16-504f01b92a3c", "StdPicture.QueryWizardCreateTempTableDescription"), // 30
    ("270de5f0-f2df-4845-9fde-30b1ec486217", "StdPicture.QueryWizardShowChangesTables"), // 30
    ("c8a269ff-5b6d-4f42-9fa6-369d7b492aa7", "StdPicture.Rename"), // 36
    ("fafe4c1f-c265-4220-a0e1-8f82af26b72e", "StdPicture.SortList"), // 33
];

static STANDARD_PICTURE_NAMES: LazyLock<HashMap<&'static str, &'static str>> =
    LazyLock::new(|| STANDARD_PICTURES.iter().copied().collect());

/// The `StdPicture.<Name>` the platform writes for a picture identifier,
/// `None` for an identifier that is no standard picture (a configuration's
/// own common picture, or one the table has not measured).
pub(crate) fn standard_picture_name(uuid: &str) -> Option<&'static str> {
    STANDARD_PICTURE_NAMES
        .get(uuid.to_ascii_lowercase().as_str())
        .copied()
}

/// The identifier a `StdPicture.<Name>` is stored by, read off the same
/// table.
pub(crate) fn standard_picture_uuid(reference: &str) -> Option<&'static str> {
    STANDARD_PICTURES
        .iter()
        .find_map(|(uuid, name)| (*name == reference).then_some(*uuid))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The twelve identities the form exporter named and the copy under
    /// `metadata_model` did not: the six ИТК query-wizard pictures the issue
    /// counts and the six Монитор ones added since.
    const FORMERLY_FORM_ONLY: &[(&str, &str)] = &[
        (
            "a119150f-6c0c-4a94-97b0-5f08d7ebd6f5",
            "StdPicture.HierarchicalView",
        ),
        (
            "18bca3d7-a7a5-41df-a180-4dff9c217f43",
            "StdPicture.QueryWizardCreateNestedQuery",
        ),
        (
            "7604cff7-5cc6-4f88-8d16-504f01b92a3c",
            "StdPicture.QueryWizardCreateTempTableDescription",
        ),
        (
            "270de5f0-f2df-4845-9fde-30b1ec486217",
            "StdPicture.QueryWizardShowChangesTables",
        ),
        ("c8a269ff-5b6d-4f42-9fa6-369d7b492aa7", "StdPicture.Rename"),
        (
            "fafe4c1f-c265-4220-a0e1-8f82af26b72e",
            "StdPicture.SortList",
        ),
        (
            "cf10e497-9779-44a7-82d8-811b88193215",
            "StdPicture.AppearanceCircleBlack",
        ),
        (
            "bbd37b6b-2742-48f3-9efa-d7c245f125b0",
            "StdPicture.AppearanceFlagGreen",
        ),
        (
            "7a2a6f5c-7677-45e1-a2d3-ce2444d99a63",
            "StdPicture.AppearanceDownTriangleRed",
        ),
        (
            "8da615a0-65a5-4511-a8d4-00dc156478fb",
            "StdPicture.AppearanceUpTriangleGreen",
        ),
        (
            "879ca050-8650-4135-b799-f3ef9e00a65a",
            "StdPicture.AppearanceBoxesEmpty",
        ),
        (
            "f16ab927-3ac6-4cdd-bcf1-d8acf005a255",
            "StdPicture.UserWithoutNecessaryProperties",
        ),
    ];

    /// The table is the union of the two copies it replaces: every pair of
    /// the `mssql_dump` copy (218 rows at the time of the merge, the 206 of
    /// the `metadata_model` copy among them), and nothing else.
    #[test]
    fn the_table_is_the_snapshot_of_the_two_copies_it_replaces() {
        let mut rows = STANDARD_PICTURES
            .iter()
            .map(|(uuid, name)| format!("{uuid} {name}"))
            .collect::<Vec<_>>();
        rows.sort_unstable();
        let mut snapshot = SNAPSHOT
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();
        snapshot.sort_unstable();
        assert_eq!(rows.len(), 218);
        assert_eq!(rows, snapshot);
    }

    #[test]
    fn identifiers_and_names_are_unique_and_well_formed() {
        let mut uuids = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        for (uuid, name) in STANDARD_PICTURES {
            assert!(uuids.insert(*uuid), "uuid {uuid} is listed twice");
            assert!(names.insert(*name), "name {name} is listed twice");
            assert!(
                uuid.len() == 36
                    && uuid.bytes().enumerate().all(|(index, byte)| match index {
                        8 | 13 | 18 | 23 => byte == b'-',
                        _ => byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte),
                    }),
                "{uuid} is not a lower-case uuid"
            );
            let bare = name
                .strip_prefix("StdPicture.")
                .unwrap_or_else(|| panic!("{name} is not a StdPicture name"));
            assert!(
                !bare.is_empty() && bare.bytes().all(|byte| byte.is_ascii_alphanumeric()),
                "{name} is not a platform picture name"
            );
        }
    }

    /// Both directions answer for every row, whatever the case of the uuid
    /// the caller found, and the identities that only the form exporter used
    /// to name are named for the commands of objects and registers too.
    #[test]
    fn both_directions_answer_for_every_row() {
        for (uuid, name) in STANDARD_PICTURES {
            assert_eq!(standard_picture_name(uuid), Some(*name));
            assert_eq!(
                standard_picture_name(&uuid.to_ascii_uppercase()),
                Some(*name)
            );
            assert_eq!(standard_picture_uuid(name), Some(*uuid));
        }
        for (uuid, name) in FORMERLY_FORM_ONLY {
            assert_eq!(standard_picture_name(uuid), Some(*name), "{name}");
            assert_eq!(standard_picture_uuid(name), Some(*uuid), "{name}");
        }
        assert_eq!(
            standard_picture_name("00000000-0000-0000-0000-000000000000"),
            None
        );
        assert_eq!(standard_picture_uuid("StdPicture.NoSuchPicture"), None);
        assert_eq!(standard_picture_uuid("Information"), None);
    }

    const SNAPSHOT: &str = "
        003024ed-fa25-42ac-9f53-f5014e383801 StdPicture.ExecuteTask
        01743054-d102-4e7c-bf15-5ed7fd84441b StdPicture.LevelDown
        01ec9d9a-7497-4d88-b93f-066c633a4866 StdPicture.InputOnBasis
        021c20a0-071b-4a60-8e44-12487adde0c8 StdPicture.EditInDialog
        03665ff1-3a05-41d1-96d3-04bda2d8ede3 StdPicture.SpreadsheetInsertComment
        05612131-3e11-49c0-9592-07e6d9318ef7 StdPicture.FindNext
        069b8324-7c51-4c73-a6a8-c06d4fc383b5 StdPicture.Catalog
        08a45a70-c221-4339-b3b1-9f11cb22147d StdPicture.Delete
        093dd4ed-e03c-4fc6-a95a-01f51379cccf StdPicture.ActivateTask
        0abdab67-5c90-4296-8168-239d22024d11 StdPicture.PrintImmediately
        0bac63da-5b4e-48af-b593-7c5d29663e83 StdPicture.FilterByType
        0c1f7756-6143-4903-a94c-8f22c85e44de StdPicture.Attribute
        0ce78048-0196-4f80-a781-9829cdb7f43e StdPicture.GenerateReport
        0e2da390-5c04-46a2-a74b-1b7e13a40f2b StdPicture.HidePassword
        1001ae3e-9289-4303-9699-3c0c17e20e61 StdPicture.AddToFavorites
        1377931c-5744-4948-bade-cb35117b5f63 StdPicture.Close
        14b24498-e49c-4713-be64-75101d0abfb9 StdPicture.GoToEnd
        167a160b-fa48-4337-87ab-7e0fe95c4b5a StdPicture.DeleteListItem
        18492a87-2fe4-44af-b218-304897fed020 StdPicture.MarkToDelete
        18bca3d7-a7a5-41df-a180-4dff9c217f43 StdPicture.QueryWizardCreateNestedQuery
        196622f7-0941-435b-992b-722f3082adf4 StdPicture.Debit
        1970a480-9b38-405e-9d9e-8209f3fad5f1 StdPicture.ScheduledJob
        1a4342a5-fa06-4556-8a85-e8738fc25821 StdPicture.GetURL
        1cd7b762-ec6a-4e92-ac9a-1832be228ec3 StdPicture.Stop
        1f046bc2-d6c5-46a3-a459-b2c0508f86fb StdPicture.QueryWizard
        1fa32fdb-a180-418f-a6eb-db7516b7a30b StdPicture.SortListDesc
        20b82e97-5fcc-4c68-8e0d-d01060847520 StdPicture.AppearanceRightArrowGray
        20ebc47b-f4d9-439c-acd3-fdc624fbac2a StdPicture.Post
        23f940bf-7381-4c2b-85a1-e541ed428042 StdPicture.SaveValues
        251aaa98-0127-44c3-a163-6f5ab4367ee2 StdPicture.DataCompositionStandardSettings
        26518e18-e364-475a-8026-e41134658b2a StdPicture.SpreadsheetInsertPageBreak
        270de5f0-f2df-4845-9fde-30b1ec486217 StdPicture.QueryWizardShowChangesTables
        2721abfb-fbff-4a3a-98ac-b7c9eb29cd85 StdPicture.AppearanceCircleEmpty
        27ee3053-952c-49e5-8261-9215098e0e9c StdPicture.CollapseAll
        283ecabd-aaed-41d1-ad46-6cca91c29120 StdPicture.LoadReportSettings
        2846af8d-af84-47e3-82b9-01b01f960426 StdPicture.SpreadsheetReadOnly
        2954e819-f3fc-40de-9769-292efce9a355 StdPicture.ExternalDataSourceFunction
        2a0c2238-cb59-4473-ada6-352b60f3c0a9 StdPicture.AddListItem
        2a7e58e2-a6c5-4387-a459-5249c441947a StdPicture.GroupConversation
        2c732bfa-f734-48bc-a18b-7554db8a3888 StdPicture.DataCompositionSelection
        2ef82795-06fe-4365-bd0c-44b486264620 StdPicture.FilterCriterion
        2f130057-bb2a-4e22-bba5-e108fac26940 StdPicture.ChooseValue
        31b93f03-0ba2-4631-a171-0d3a3d2ecc48 StdPicture.ListSettings
        31bf709f-3b50-4137-9b51-ebc7fb802a7c StdPicture.GotoExternalURL
        35bc8caa-f7ce-4158-87da-d9bf785afa39 StdPicture.DataSearch
        3689585c-a3e2-45d0-a302-caeb31b78835 StdPicture.AppearanceStarFilled
        37cf7cc0-abad-4385-b597-6fd2d8dc085a StdPicture.Task
        37e91e77-93ce-4c3b-8d30-a9d8cfd3d3b0 StdPicture.MoveItem
        38bbcebe-e456-461b-8457-07c9a72344a3 StdPicture.FindInTree
        3bdc16c8-6a96-4467-9442-a8e4804b3fa2 StdPicture.SyncContents
        3c904ff7-1195-4a7c-9a38-7b1f6ca49cce StdPicture.Back
        3d4ad3b1-17de-4cf1-a2e4-0c2c83a5b5c2 StdPicture.ListViewModeHierarchicalList
        448d6f55-d885-496c-870d-d1bd78374745 StdPicture.CloneListItem
        46598f81-5f95-4485-9b33-bfe4fd1276d0 StdPicture.SpreadsheetShowHeaders
        479470e0-ea0f-4266-8549-e2b1e8c06534 StdPicture.ClearFilter
        47f01799-7968-4f44-9acc-fe1bdde8beb2 StdPicture.ActiveUsers
        4ab0e87f-7d9b-4aa8-ac4b-680a78522da8 StdPicture.CreateFolder
        4b54770b-d069-4c0e-9b17-5cc2a01134d9 StdPicture.Information
        4bf588d5-b8d8-47fd-8f41-eb9b668981ce StdPicture.SetListItemDeletionMark
        4bf9fbb5-53c5-4b09-bab2-d69bbfab945b StdPicture.Dendrogram
        4d2570b5-205f-413c-b4cc-b2097f61684f StdPicture.CreateInitialImage
        4fddea39-5129-4b4c-83fe-4e443cd61940 StdPicture.EventLogByUser
        501b8c1d-8062-408e-bde8-b6549324713e StdPicture.AppearanceExclamationMark
        509c4a7f-6406-4388-bb8c-bc81fb5131aa StdPicture.BusinessProcess
        5182f57f-e834-4d11-9c9f-4aedc002b6e9 StdPicture.FixTable
        5289d9a4-b012-4d54-9bce-50473fe29b57 StdPicture.DialogExclamation
        52b637e5-f95f-4c70-9a72-2a4b5a9df449 StdPicture.NestedTable
        544fdbe8-5956-4512-bc62-93b4c022d291 StdPicture.ExchangePlan
        549a2c45-4fce-493f-94ee-9a3a4f426551 StdPicture.ListViewMode
        55bc1099-a7df-4d0a-b332-a45f0473b368 StdPicture.Credit
        55ef0776-5ee4-4daf-9a9b-70d63643ab8d StdPicture.SetTime
        58174855-39be-462e-8723-cb2d95182146 StdPicture.SetDateInterval
        584b470d-ba34-4b25-9620-8de4066ffeaa StdPicture.Previous
        5b612c21-e223-4997-9e61-86f7a67ec945 StdPicture.ExternalDataSourceTable
        5b87ad1b-d8cc-43c1-b5c4-dc43613c518c StdPicture.InformationRegister
        60643198-e4b2-4c39-9de1-53cca3fff382 StdPicture.DeleteDirectly
        6206a729-16e5-4e32-b53c-122de4e30c8d StdPicture.ScheduledJobs
        64837726-d2a2-4682-a788-737423e80013 StdPicture.Picture
        64ca52ee-f1a3-468f-8055-311935077515 StdPicture.QueryWizardTempTable
        6511326b-20c3-4bf8-8503-c2c2c9072c6c StdPicture.CustomizeForm
        6a248caf-0a7b-46ad-a595-74890ea202f7 StdPicture.Conversations
        6b909f65-95a4-4697-8ca0-c8f331227b9a StdPicture.SettingsStorage
        6c2759b1-2b63-4fa5-8d13-75786c7e1e89 StdPicture.DataCompositionNewTable
        6cb69e7f-fe19-4f64-bfb5-1a4fad6c2ef9 StdPicture.Replace
        6cbf8f9a-3d2f-427b-bfce-5e2bc7a8589d StdPicture.DeleteListItemDirectly
        6e3687cf-a8d1-446a-833a-bfaf38516353 StdPicture.SwitchActivity
        6ecee038-9722-4d80-bb91-7ee7046ec4c7 StdPicture.Help
        6ff3ddbd-56e3-4ddf-a5bf-048c1e2dfb2f StdPicture.User
        702a9e16-0bb6-4efb-af11-10faf1e6ee87 StdPicture.SpreadsheetShowGroups
        70f51581-87b6-41cb-a21b-c9dcdcc7fa93 StdPicture.AccumulationRegister
        7168f070-087c-448e-ae3a-6740f424076a StdPicture.ChooseFromList
        71cbcb5c-f3f0-4ffd-a4d0-19b802b5ed6b StdPicture.AppearanceCircleGreen
        723765ab-0b92-4745-a621-1ba0f77c92c9 StdPicture.EventLog
        732f9dd3-5baf-47ff-af7b-edfe16dac2a1 StdPicture.DataCompositionNewChart
        73af51dd-6cda-48be-a093-5a7161c60c77 StdPicture.FilterAndSort
        7562cef7-0e57-4f63-a754-b61128a4f3ae StdPicture.GoForward
        75a40cc4-c719-4c3f-91ea-fc5787bc34ca StdPicture.UserWithAuthentication
        7604cff7-5cc6-4f88-8d16-504f01b92a3c StdPicture.QueryWizardCreateTempTableDescription
        77180b5e-8faa-4712-a788-e9f8903e3419 StdPicture.DataCompositionNewGroup
        785362cb-3756-48ed-87d2-292ded17054a StdPicture.OpenFile
        788667db-61c9-45f3-9c4f-5f660ecdf3e1 StdPicture.AppearanceCircleFilled
        78da2c47-172f-4d57-ab52-a06e40548136 StdPicture.Message
        7a2a6f5c-7677-45e1-a2d3-ce2444d99a63 StdPicture.AppearanceDownTriangleRed
        7a9cd2fd-6372-4342-9a9e-3ebbd754fd83 StdPicture.AppearanceCheckBox
        7c75b1df-1fdc-471f-aae6-6b7870318cd4 StdPicture.SpreadsheetDeleteComment
        7df3febb-2640-41b7-ad8b-7a23b7ad4aec StdPicture.QueryWizardCreateTempTableDropQuery
        818ab7d0-4654-4542-bd5e-fd9d1352b5a1 StdPicture.SaveFile
        835db646-1531-494b-b7c1-3239b0080bcb StdPicture.Parameters
        83c8f18d-8701-41f3-bef4-53f88adbb868 StdPicture.ReadChanges
        83db1f8a-41bd-4016-bdb2-a28e3a8d6dcc StdPicture.DialogStop
        85998f14-805b-4e2b-ba19-9d79b0464042 StdPicture.AppearanceCheckIcon
        85cc7dd0-44fc-41aa-967f-f52f202ee2e6 StdPicture.GoToBegin
        879ca050-8650-4135-b799-f3ef9e00a65a StdPicture.AppearanceBoxesEmpty
        87d032df-0956-47e9-bead-4e15330f1983 StdPicture.AppearanceUpArrowGreen
        892196a9-c94f-4e50-8224-3c0cea4ea6b8 StdPicture.GoBack
        894afc03-9904-465d-b671-f555ffb9b21c StdPicture.Document
        894cf65b-4109-4533-a1d7-c87b1fcc80a3 StdPicture.Write
        8ac19694-383a-457a-b050-0a3ee937f5f3 StdPicture.DataCompositionNewNestedScheme
        8bdf1079-8fad-4d21-ad7f-4b2e4ecdce3d StdPicture.DialogInformation
        8ca4ea33-603d-4992-8a41-c7924b5bd40b StdPicture.UndoPosting
        8d7e5026-9c1c-4542-bec0-2b729c84e139 StdPicture.AppearanceCircleYellow
        8da615a0-65a5-4511-a8d4-00dc156478fb StdPicture.AppearanceUpTriangleGreen
        8f29e0e2-d5e6-41e8-a34d-9a0288156322 StdPicture.Reread
        91022b99-b610-48ad-954e-a297848081ce StdPicture.SortListAsc
        928075d1-b90b-416c-b0b2-c3104cf084aa StdPicture.Notifications
        92e24ce1-3917-4ee4-bbde-adce48b6c96b StdPicture.AppearanceUpInclineArrowGray
        942e0303-a3ec-4fe8-887c-5aea8516d424 StdPicture.ReportSettings
        977e831a-0e73-4d60-af51-091a6fa8612e StdPicture.CreateListItem
        97b2cc97-d5c6-45fb-9824-9d6d73db21fe StdPicture.Change
        97c5a6d5-47ed-43f9-8c8c-10e9903c23d2 StdPicture.Calendar
        97f87955-b88a-4225-a0d8-03af981ecd86 StdPicture.ShowPassword
        984b0a3e-daa2-4a9e-b75c-1f230a6e592a StdPicture.DataCompositionConditionalAppearance
        9c96aa25-d656-4b3d-ab3e-81d9718da238 StdPicture.ShowInList
        9cf611dc-2370-4357-910d-a2b49c7a1ec6 StdPicture.Next
        9e808d29-787b-4825-863a-13c6844ce91d StdPicture.CancelSearch
        9ef73565-2250-4a35-9fb3-470bd19ca9ca StdPicture.AppearanceCrossIcon
        9fecbaff-2a05-4da6-9ef1-807e754b928d StdPicture.BusinessProcessStart
        a064544f-6037-48ca-b19f-8ad63e43af23 StdPicture.ShowData
        a075c3ef-bc4e-4c96-bdad-2245ec09c28e StdPicture.DataCompositionDataParameters
        a119150f-6c0c-4a94-97b0-5f08d7ebd6f5 StdPicture.HierarchicalView
        a24cff7f-a1a5-4403-af82-a7b31852cde9 StdPicture.BusinessProcessObject
        a30ab2ef-6076-457d-9293-44edc7c6767e StdPicture.AppearanceDownInclineArrowGray
        a43fcd1b-ad8d-4318-9d55-fd1ba086e65b StdPicture.RotateCounterclockwise
        a594c8a1-7218-420a-860f-7b493c5e65c4 StdPicture.Sort
        a6cbfd77-fcf0-40f4-a8de-ee0d3e580fe6 StdPicture.DataProcessor
        a722bc14-4edb-4eed-84b9-5d9b2b443e04 StdPicture.CollaborationSystemUser
        a7707ed1-39b0-418f-974d-4d500d27a9c6 StdPicture.RestoreValues
        a9152be7-62cf-4523-be34-a23f018f497e StdPicture.GeographicalSchema
        a9481ba4-dc85-4112-9c50-f9f340a61298 StdPicture.QueryWizardReplaceTable
        aa96f4bb-cf28-4dad-bc42-5ed53de95c0c StdPicture.SpreadsheetDeletePageBreak
        ad8cb448-a6bb-43b4-886a-7d6a8367eef2 StdPicture.DataCompositionGroupFields
        affb1617-24bc-4170-9c84-0902cc3ef206 StdPicture.DataCompositionSettingsWizard
        b0dd988f-2d9f-4364-b1f4-a4d5f45ffb78 StdPicture.AppearanceFlagRed
        b1406535-6cc2-4410-95ea-753556e8460f StdPicture.FilterByCurrentValue
        b2202798-23e0-4165-9982-24878f432488 StdPicture.AppearanceCross
        b39aa431-a32f-4447-984a-45606474c82d StdPicture.AppearanceExclamationMarkIcon
        b4c7ab2c-bcda-4468-a28f-5fee93838c4e StdPicture.Properties
        b5a0aaba-3a83-4a71-b6f9-24aae1574681 StdPicture.SaveReportSettings
        b68eb29c-2372-46e1-b84e-13843899ccf6 StdPicture.DataCompositionUserFields
        b7c81c62-d6ad-4eae-9cea-0e203182db67 StdPicture.FormHelp
        ba592483-bc90-4e26-ba4d-2126359c6529 StdPicture.AppearanceBoxesFilled
        bbd37b6b-2742-48f3-9efa-d7c245f125b0 StdPicture.AppearanceFlagGreen
        be23a908-fe1b-44df-be94-d0f6e8353abe StdPicture.SendMessage
        c1a61df2-f280-49d0-a8b3-7e5fc6f56ff7 StdPicture.AppearanceCircleRed
        c283cd1c-3187-451d-8ef2-7df55daeef06 StdPicture.History
        c2e2d966-5b7f-4699-903b-28a6f50d5471 StdPicture.OutputList
        c38cc4cf-111d-4bc8-8dcb-4464e2ddfb25 StdPicture.Favorites
        c757209d-a87f-4410-b1a3-76000178f1f0 StdPicture.ListViewModeList
        c78c9f3d-e92c-4f38-bb72-d8bd7fa5dbe3 StdPicture.Dimension
        c7cdd3c0-3879-436a-b145-5e2615e9b3e1 StdPicture.FindInList
        c7f70aa3-b944-4efe-97e3-0fa3bda3cd88 StdPicture.RotateClockwise
        c8a269ff-5b6d-4f42-9fa6-369d7b492aa7 StdPicture.Rename
        caf2e58b-ca3d-4b63-82c9-f21f1c9bc9eb StdPicture.Setting
        cb34c423-3d6a-4202-a809-3b3f45fb14ab StdPicture.LevelUp
        ccb3d8f7-6da2-4c65-aba6-17b2ffbba78c StdPicture.ChartOfAccounts
        cf10e497-9779-44a7-82d8-811b88193215 StdPicture.AppearanceCircleBlack
        d35bd799-1cc3-44d1-8ae3-09755a09d44b StdPicture.DataCompositionFilterDisabled
        d66b6f73-53b8-49b9-8efc-33c54aa06e3f StdPicture.Notify
        d6eefec0-792a-4720-8933-e2a57f9e312c StdPicture.Resource
        d90a7482-9a1d-4d3d-ae96-6db440214d96 StdPicture.DataCompositionFilter
        da0c4924-973c-4ef0-9dcf-f1fc3307e5e2 StdPicture.ChangeListItem
        da9ac044-0ff7-4bcf-a441-3187bd1d951f StdPicture.ListViewModeTree
        db817ee1-fd28-4e7f-bb4a-53686b2b153c StdPicture.Report
        dcd23a32-5c7c-43f2-9021-80d98128556f StdPicture.CheckSyntax
        dfcd2d21-24ea-4b27-ab9a-6bf754577536 StdPicture.FunctionMenuCommand
        e3b29b1d-4694-4f56-8d55-922f83afed7a StdPicture.AppearanceDownArrowGray
        e3b38083-0191-4a10-8f5b-51571f2419b4 StdPicture.FindPrevious
        e51185a4-d915-45b8-b201-1c46cc2d8104 StdPicture.DocumentJournal
        e6fc55a0-3d58-4b15-bdd3-717453929598 StdPicture.WriteAndClose
        e8a49985-fef7-45a9-b6bb-ddd2b9028172 StdPicture.DataHistory
        e93f538e-dfaf-4a91-a9b6-c053555bcf60 StdPicture.Constant
        e96de06b-fa83-48cf-b033-190a249855c9 StdPicture.GraphicalSchema
        eb47324b-85f9-4172-9315-bba8015d9970 StdPicture.NewWindow
        ed067d76-b144-4d00-bb36-d1833dd1350c StdPicture.DebitCredit
        ed0bec43-4633-416c-8c08-0384ca444e32 StdPicture.EndEdit
        ee7c4a5b-2d9b-4087-ae3e-947792085f09 StdPicture.DataCompositionOutputParameters
        ef27ae9e-7040-4374-b93c-0d276de2ea23 StdPicture.DialogQuestion
        efda7350-6cd7-4416-b188-f5ca9baf66c2 StdPicture.DataCompositionOrder
        f04794cb-c198-4172-86c3-649386013c85 StdPicture.CustomizeList
        f16ab927-3ac6-4cdd-bcf1-d8acf005a255 StdPicture.UserWithoutNecessaryProperties
        f3b8f300-5a54-4eea-8136-5798413a479c StdPicture.CalculationRegister
        f3c1376a-d2ee-46c4-9e44-aa2f7dae31c4 StdPicture.Chart
        f62488ee-f90c-47f7-929d-f42ec11a1e63 StdPicture.WriteChanges
        f6532868-30b9-44ab-803c-78f0f0b06b02 StdPicture.CloneObject
        f695666a-bad9-49f6-ab7c-5198d7ea4739 StdPicture.CustomExpression
        f6e88116-03d8-4400-9d88-791895d7031a StdPicture.Attach
        f874b0cc-db1d-4577-8c77-d4ba206eb05d StdPicture.Forward
        fa67cb81-8d56-4534-90bd-b62fb0dbf5f0 StdPicture.GanttChart
        fad46a2b-2e56-47cc-b90c-3c2d4b061937 StdPicture.ExternalDataSourceCube
        fada8a16-8b14-4151-87a9-775099f37832 StdPicture.Calculator
        fafe4c1f-c265-4220-a0e1-8f82af26b72e StdPicture.SortList
        fb7e9fb5-110b-41cb-adc6-753969ae1c81 StdPicture.ExpandAll
        fc058833-e57f-4f93-ba7a-803992a65c3e StdPicture.AppearanceCircleOneFourthFilled
        fc34a694-e99b-4d1c-a526-63f5571bdb09 StdPicture.Form
        fc4f29e0-d168-4fe0-8e64-e982fabf2595 StdPicture.Refresh
        fc6a06a8-1308-4385-b1b2-9d302d2054ed StdPicture.SearchControl
        fe740df0-d828-4241-a12f-7414e12302e8 StdPicture.QueryWizardTableParameters
        ffab30f1-da11-44b5-b34c-24da22badcf4 StdPicture.Find
    ";
}
