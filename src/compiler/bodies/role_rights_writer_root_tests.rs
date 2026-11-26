//! The tests of `role_rights_writer` (in `ibcmd-metadata-blob`) that read
//! the rights back through the root's exporter, or pack them through
//! `module_blob`'s base-free packer in a test scratch directory.

#[cfg(test)]
mod tests {
    use crate::compiler::bodies::role_rights_writer::*;
    use ibcmd_schema::configuration_rights::CONFIGURATION_MODE_RIGHT_NAMES;

    struct Fixed;

    impl RoleRightsSource for Fixed {
        fn metadata_object_uuid(&self, reference: &str) -> Result<String, String> {
            match reference {
                "Catalog.A" => Ok("aaaaaaaa-0000-4000-8000-000000000001".to_string()),
                "Catalog.A.Attribute.B" => Ok("bbbbbbbb-0000-4000-8000-000000000002".to_string()),
                "Configuration.C" => Ok("cccccccc-0000-4000-8000-000000000003".to_string()),
                _ => Err(reference.to_string()),
            }
        }

        fn declared_field_uuid(&self, _: &str, _: &str) -> Result<Option<String>, String> {
            Ok(None)
        }
    }

    #[test]
    fn adopted_standard_attribute_rights_keep_grants_against_attribute_default() {
        use crate::module_blob::{
            MetadataSourceContext, inflate_raw, pack_role_rights_blob_base_free,
        };
        let temp =
            std::env::temp_dir().join(format!("ibcmd-adopted-rights-{}", uuid::Uuid::new_v4()));
        let _scratch = crate::test_scratch::TestScratch::adopt(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        std::fs::create_dir(temp.as_path().join("Catalogs")).unwrap();
        std::fs::write(
            temp.as_path().join("Catalogs/A.xml"),
            br#"<MetaDataObject>
          <Catalog uuid="aaaaaaaa-0000-4000-8000-000000000001"><Properties>
            <ObjectBelonging>Adopted</ObjectBelonging><Name>A</Name></Properties>
            <ChildObjects><Attribute uuid="bbbbbbbb-0000-4000-8000-000000000002"><Properties>
              <Name>B</Name></Properties></Attribute></ChildObjects>
          </Catalog></MetaDataObject>"#,
        )
        .unwrap();
        let xml = br#"<Rights><setForNewObjects>false</setForNewObjects>
          <setForAttributesByDefault>true</setForAttributesByDefault>
          <independentRightsOfChildObjects>false</independentRightsOfChildObjects>
          <object><name>Catalog.A.StandardAttribute.Code</name>
            <right><name>View</name><value>true</value></right>
            <right><name>Edit</name><value>true</value></right></object>
          <object><name>Catalog.A.Attribute.B</name>
            <right><name>View</name><value>false</value></right></object></Rights>"#;
        let source = SourceTreeRoleRightsSource::new(temp.as_path());
        assert!(source.is_adopted_object("Catalog.A").unwrap());
        assert!(
            source
                .is_adopted_object("Catalog.A.StandardAttribute.Code")
                .unwrap()
        );
        assert!(!source.is_adopted_object("Catalog.A.Attribute.B").unwrap());
        let context = MetadataSourceContext::new(temp.as_path().to_path_buf());
        // The packer itself reads the written row back and compares the full
        // rights document. Losing the adopted marker used to drop both grants.
        let packed = pack_role_rights_blob_base_free(xml, &context).unwrap();
        let text = String::from_utf8(inflate_raw(&packed.blob).unwrap()).unwrap();
        assert!(text.contains(",3}"), "{text}");
        assert!(
            text.contains("{1,bbbbbbbb-0000-4000-8000-000000000002,0,1}"),
            "{text}"
        );
        assert!(temp.starts_with(std::env::temp_dir()));
    }

    #[test]
    fn writes_termination_right_and_preserves_configuration_defaults() {
        use ibcmd_schema::configuration_rights::{
            EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START,
            EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START_UUID,
        };
        use std::io::Write;

        for flag in [false, true] {
            for explicit_modes in [false, true] {
                for termination_value in [false, true] {
                    let mut rights = vec![RightsRight {
                        name: "CollaborationSystemInfoBaseRegistration".into(),
                        value: !flag,
                        restrictions: vec![],
                    }];
                    if explicit_modes {
                        rights.extend(CONFIGURATION_MODE_RIGHT_NAMES.map(|name| RightsRight {
                            name: name.into(),
                            value: !flag,
                            restrictions: vec![],
                        }));
                    }
                    rights.extend([
                        RightsRight {
                            name: EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START.into(),
                            value: termination_value,
                            restrictions: vec![],
                        },
                        RightsRight {
                            name: "SaveUserData".into(),
                            value: !flag,
                            restrictions: vec![],
                        },
                    ]);
                    let mut expected = RightsDocument {
                        set_for_new_objects: flag,
                        set_for_attributes_by_default: true,
                        objects: vec![RightsObject {
                            name: "Configuration.C".into(),
                            rights,
                        }],
                        ..Default::default()
                    };
                    let written = write_role_rights_document(&expected, &Fixed).unwrap();
                    assert!(
                        String::from_utf8_lossy(&written.plain)
                            .contains(EXCLUSIVE_MODE_TERMINATION_AT_SESSION_START_UUID)
                    );
                    let mut encoder = flate2::write::DeflateEncoder::new(
                        Vec::new(),
                        flate2::Compression::default(),
                    );
                    encoder.write_all(&written.plain).unwrap();
                    let blob = encoder.finish().unwrap();
                    let xml = crate::mssql_dump::role_rights_xml_from_blob(
                        &blob,
                        &written.object_refs,
                        &written.field_refs,
                    )
                    .unwrap();
                    // Explicit default rights are intentionally omitted by
                    // the native Configuration-root presentation convention.
                    expected.objects[0]
                        .rights
                        .retain(|right| right.value != flag);
                    assert_eq!(
                        parse_rights_xml(xml.as_bytes()).unwrap(),
                        expected,
                        "flag={flag}, modes={explicit_modes}, termination={termination_value}"
                    );
                }
            }
        }
    }
}
