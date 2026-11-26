//! The tests of the command-interface family's base-free writers (in
//! `ibcmd-metadata-blob`'s `interface_assets.rs`) that own their source
//! trees through the root's test scratch.

use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use ibcmd_form_native::ConfigurationObject;
    use std::sync::Arc;

    // The writer's own nil uuid is private to it.
    const NIL_UUID: &str = "00000000-0000-0000-0000-000000000000";
    const CATALOG: &str = "11111111-1111-4111-8111-111111111111";
    const COMMAND: &str = "22222222-2222-4222-8222-222222222222";
    const NESTED: &str = "33333333-3333-4333-8333-333333333333";
    const GROUP: &str = "44444444-4444-4444-8444-444444444444";
    const SUBSYSTEM: &str = "55555555-5555-4555-8555-555555555555";
    const ROLE: &str = "66666666-6666-4666-8666-666666666666";
    const CONSTANT: &str = "77777777-7777-4777-8777-777777777777";

    fn metadata(root: &Path, relative: &str, body: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!(
                "<MetaDataObject xmlns=\"http://v8.1c.ru/8.3/MDClasses\">{body}</MetaDataObject>"
            ),
        )
        .unwrap();
    }

    /// Every kind of name a command interface spells, resolved against a
    /// source tree and written in the platform's layout.
    #[test]
    fn writes_every_section_with_names_resolved_against_the_source() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-interface-writer-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        metadata(
            &root,
            "Catalogs/Товары.xml",
            &format!(
                "<Catalog uuid=\"{CATALOG}\"><Properties><Name>Товары</Name></Properties><ChildObjects><Command uuid=\"{NESTED}\"><Properties><Name>Печать</Name></Properties></Command></ChildObjects></Catalog>"
            ),
        );
        metadata(
            &root,
            "CommonCommands/Открыть.xml",
            &format!("<CommonCommand uuid=\"{COMMAND}\"/>"),
        );
        metadata(
            &root,
            "CommandGroups/Группа.xml",
            &format!("<CommandGroup uuid=\"{GROUP}\"/>"),
        );
        metadata(
            &root,
            "Subsystems/А/Subsystems/Б.xml",
            &format!("<Subsystem uuid=\"{SUBSYSTEM}\"/>"),
        );
        metadata(&root, "Roles/Роль.xml", &format!("<Role uuid=\"{ROLE}\"/>"));
        metadata(
            &root,
            "Constants/Флаг.xml",
            &format!("<Constant uuid=\"{CONSTANT}\"/>"),
        );
        let xml = r#"<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="Catalog.Товары.StandardCommand.CreateFolder">
			<Visibility>
				<xr:Common>false</xr:Common>
				<xr:Value name="Role.Роль">true</xr:Value>
			</Visibility>
		</Command>
		<Command name="0">
			<Visibility>
				<xr:Common>true</xr:Common>
			</Visibility>
		</Command>
	</CommandsVisibility>
	<CommandsPlacement>
		<Command name="CommonCommand.Открыть">
			<CommandGroup>CommandGroup.Группа</CommandGroup>
			<Placement>Manual</Placement>
		</Command>
	</CommandsPlacement>
	<CommandsOrder>
		<Command name="Catalog.Товары.Command.Печать">
			<CommandGroup>NavigationPanelOrdinary</CommandGroup>
		</Command>
		<Command name="Constant.Флаг.StandardCommand.Open">
			<CommandGroup>NavigationPanelOrdinary</CommandGroup>
		</Command>
	</CommandsOrder>
	<SubsystemsVisibility>
		<Subsystem name="Subsystem.А.Subsystem.Б">
			<Visibility>
				<xr:Common>false</xr:Common>
			</Visibility>
		</Subsystem>
	</SubsystemsVisibility>
	<SubsystemsOrder>
		<Subsystem>Subsystem.А.Subsystem.Б</Subsystem>
		<Subsystem/>
	</SubsystemsOrder>
	<GroupsOrder>
		<Group>NavigationPanelOrdinary</Group>
	</GroupsOrder>
</CommandInterface>"#;
        let source = MetadataSourceContext::new(root.clone());
        let plain = interface_asset_plaintext(
            InterfaceAssetSource::CommandInterface,
            xml.as_bytes(),
            Some(&source),
        )
        .unwrap();
        let ordinary = "77ea1b8f-dd79-4717-9dba-5628e7f348cf";
        assert_eq!(
            String::from_utf8(plain).unwrap(),
            format!(
                "\u{feff}{{7,1,2,\r\n{{2,{CATALOG}}},\r\n{{0,\r\n{{0,\r\n{{\"B\",0}},1,{ROLE},\r\n{{\"B\",1}}\r\n}}\r\n}},\r\n{{0}},\r\n{{0,\r\n{{0,\r\n{{\"B\",1}},0}}\r\n}},1,1,\r\n{{0,{COMMAND}}},{GROUP},1,1,2,{ordinary},\r\n{{0,{NESTED}}},{ordinary},\r\n{{100,{CONSTANT}}},1,2,{SUBSYSTEM},{NIL_UUID},1,1,{ordinary},1,1,{SUBSYSTEM},\r\n{{0,\r\n{{0,\r\n{{\"B\",0}},0}}\r\n}}\r\n}}"
            )
        );
        // Without the tree only raw spellings resolve.
        assert!(
            interface_asset_plaintext(InterfaceAssetSource::CommandInterface, xml.as_bytes(), None)
                .is_err()
        );
    }

    /// A source the interface writer reads from memory: the files by path,
    /// under a root that is not on disk. The writer reads the root, the
    /// files and the memo; any other member would be a read the seam does
    /// not expect.
    #[derive(Debug)]
    struct InMemorySource {
        root: std::path::PathBuf,
        files: BTreeMap<std::path::PathBuf, Vec<u8>>,
        memo: std::sync::Mutex<InterfaceResolutionMemo>,
    }

    impl MetadataSource for InMemorySource {
        fn source_root(&self) -> &Path {
            &self.root
        }
        fn read_source(&self, path: &Path) -> std::io::Result<SourceBytes> {
            self.files
                .get(path)
                .map(|bytes| SourceBytes::Owned(bytes.clone()))
                .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        }
        fn interface_memo(&self) -> &std::sync::Mutex<InterfaceResolutionMemo> {
            &self.memo
        }
        fn configuration_uuid(&self) -> Option<String> {
            unreachable!("the command interface reads no configuration uuid")
        }
        fn configuration_object(&self, _: &str) -> Option<Arc<ConfigurationObject>> {
            unreachable!("the command interface reads no data path")
        }
        fn resolve_metadata_type_id(&self, _: &str) -> Result<String> {
            unreachable!("the command interface resolves no type")
        }
        fn resolve_metadata_reference_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the command interface reads its own headers")
        }
        fn resolve_style_item_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the command interface names no style item")
        }
        fn resolve_common_picture_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the command interface names no picture")
        }
        fn resolve_command_group_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the command interface reads its own headers")
        }
        fn dcs_generated_type_id(&self, _: &str) -> Option<String> {
            unreachable!("the command interface has no data-composition schema")
        }
        fn dcs_style_items(&self) -> &BTreeMap<String, String> {
            unreachable!("the command interface has no data-composition schema")
        }
        fn moxel_object_refs(&self) -> Result<BTreeMap<String, String>> {
            unreachable!("the command interface has no chart")
        }
        fn help_reference(&self, _: &str) -> Result<HelpReference> {
            unreachable!("the command interface has no help page")
        }
        fn resolve_command_reference_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the command interface names no command")
        }
        fn resolve_form_uuid(&self, _: &str) -> Result<String> {
            unreachable!("the command interface names no form")
        }
        fn resolve_metadata_child_uuid(&self, _: &str, _: &str, _: &str) -> Result<String> {
            unreachable!("the command interface names no child object")
        }
        fn resolve_predefined_item_id(&self, _: &str, _: &str) -> Result<String> {
            unreachable!("the command interface names no predefined item")
        }
        fn information_register_master_dimensions(&self, _: &str) -> Result<Vec<String>> {
            unreachable!("the command interface reads no register")
        }
        fn resolve_simple_metadata_uuid(
            &self,
            _: &str,
            _: &str,
            _: &str,
            _: &str,
        ) -> Result<String> {
            unreachable!("the command interface names no object")
        }
    }

    /// The writer reads names only through `MetadataSource`: the same files
    /// in memory, under a root that does not exist, give the very plaintext
    /// the source context gives over them on disk, and the headers it
    /// memoises are the source's.
    #[test]
    fn an_in_memory_source_answers_the_writer_as_the_tree_on_disk_does() {
        let files = [
            (
                "Catalogs/Товары.xml",
                format!(
                    "<Catalog uuid=\"{CATALOG}\"><Properties><Name>Товары</Name></Properties><ChildObjects><Command uuid=\"{NESTED}\"><Properties><Name>Печать</Name></Properties></Command></ChildObjects></Catalog>"
                ),
            ),
            (
                "CommonCommands/Открыть.xml",
                format!("<CommonCommand uuid=\"{COMMAND}\"/>"),
            ),
            (
                "CommandGroups/Группа.xml",
                format!("<CommandGroup uuid=\"{GROUP}\"/>"),
            ),
            (
                "Subsystems/А/Subsystems/Б.xml",
                format!("<Subsystem uuid=\"{SUBSYSTEM}\"/>"),
            ),
            ("Roles/Роль.xml", format!("<Role uuid=\"{ROLE}\"/>")),
        ];
        let xml = r#"<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="Catalog.Товары.StandardCommand.CreateFolder">
			<Visibility>
				<xr:Common>false</xr:Common>
				<xr:Value name="Role.Роль">true</xr:Value>
			</Visibility>
		</Command>
	</CommandsVisibility>
	<CommandsPlacement>
		<Command name="CommonCommand.Открыть">
			<CommandGroup>CommandGroup.Группа</CommandGroup>
			<Placement>Manual</Placement>
		</Command>
	</CommandsPlacement>
	<CommandsOrder>
		<Command name="Catalog.Товары.Command.Печать">
			<CommandGroup>NavigationPanelOrdinary</CommandGroup>
		</Command>
	</CommandsOrder>
	<SubsystemsOrder>
		<Subsystem>Subsystem.А.Subsystem.Б</Subsystem>
	</SubsystemsOrder>
</CommandInterface>"#;

        let disk = std::env::temp_dir().join(format!(
            "ibcmd-rs-interface-seam-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&disk);
        for (relative, body) in &files {
            metadata(&disk, relative, body);
        }
        let context = MetadataSourceContext::new(disk.clone());
        let expected = interface_asset_plaintext(
            InterfaceAssetSource::CommandInterface,
            xml.as_bytes(),
            Some(&context),
        )
        .unwrap();

        let root = std::path::PathBuf::from("in-memory-tree-that-is-not-on-disk");
        assert!(!root.exists());
        let memory = InMemorySource {
            files: files
                .iter()
                // The very bytes the context reads from disk.
                .map(|(relative, _)| (root.join(relative), fs::read(disk.join(relative)).unwrap()))
                .collect(),
            root,
            memo: Default::default(),
        };
        let plain = plaintext_through(
            InterfaceAssetSource::CommandInterface,
            xml.as_bytes(),
            Some(&memory),
        )
        .unwrap();
        assert_eq!(plain, expected);
        {
            // Released before the writer runs again: it locks the memo.
            let memo = memory.memo.lock().unwrap();
            assert!(
                memo.headers
                    .contains_key(&memory.root.join("Subsystems/А/Subsystems/Б.xml")),
                "{memo:?}"
            );
            assert!(
                memo.commands
                    .contains_key(&memory.root.join("Catalogs/Товары.xml")),
                "{memo:?}"
            );
        }

        // A name the source does not have refuses alike through either.
        let missing = xml.replace("CommonCommand.Открыть", "CommonCommand.Нет");
        let through_memory = plaintext_through(
            InterfaceAssetSource::CommandInterface,
            missing.as_bytes(),
            Some(&memory),
        )
        .unwrap_err();
        let through_disk = interface_asset_plaintext(
            InterfaceAssetSource::CommandInterface,
            missing.as_bytes(),
            Some(&context),
        )
        .unwrap_err();
        assert_eq!(format!("{through_memory:#}"), format!("{through_disk:#}"));
        assert!(
            format!("{through_memory:#}")
                .contains("`CommonCommand.Нет` names no file of the source tree"),
            "{through_memory:#}"
        );
    }
}
