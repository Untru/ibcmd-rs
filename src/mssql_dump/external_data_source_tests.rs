use super::*;
use crate::metadata_model::{
    DescriptorContext,
    brace::{Brace, parse_row, serialize},
    compile_descriptor,
    external_data_source::generated as fixture,
    index::ConfigIndex,
};

fn compiled(kind: &str, xml: &str, version: &str) -> Brace {
    let context = DescriptorContext {
        root: PathBuf::from("."),
        index: ConfigIndex::default(),
        source: crate::module_blob::MetadataSourceContext::new(PathBuf::from(".")),
        version: version.into(),
    };
    parse_row(
        &compile_descriptor(kind, Path::new("generated.xml"), xml.as_bytes(), &context).unwrap(),
    )
    .unwrap()
}

fn extract(
    text: &str,
    uuid: &str,
    declared: &str,
    version: InfobaseConfigSourceVersion,
) -> std::result::Result<ExtractedMetadataSourceXml, MetadataSourceExtractionDiagnostic> {
    extract_with_current_refs(text, uuid, declared, version, &BTreeMap::new())
}

fn extract_with_current_refs(
    text: &str,
    uuid: &str,
    declared: &str,
    version: InfobaseConfigSourceVersion,
    object_refs: &BTreeMap<String, String>,
) -> std::result::Result<ExtractedMetadataSourceXml, MetadataSourceExtractionDiagnostic> {
    let row = metadata_text_row_from_text(uuid, text.into()).unwrap();
    extract_metadata_source_xml_from_text_row_audited_with_object_ref_resolutions(
        &row,
        &BTreeMap::new(),
        &BTreeSet::new(),
        object_refs,
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::from([(uuid.into(), declared.into())]),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        &BTreeMap::new(),
        version,
        &MetadataTypeSetLeafIndex::new(),
    )
}

#[test]
fn physical_export_uses_complete_eds_codec_both_dialects() {
    for (dialect, version) in [
        ("2.20", InfobaseConfigSourceVersion::V2_20),
        ("2.21", InfobaseConfigSourceVersion::V2_21),
    ] {
        let text = serialize(&compiled(
            "ExternalDataSource",
            &fixture::external_data_source(dialect, "Automatic", [2, 0, 1]),
            dialect,
        ));
        let row = metadata_text_row_from_text(fixture::OBJECT_UUID, text.clone()).unwrap();
        assert_eq!(row.kind.as_deref(), Some("ExternalDataSource"));
        assert_eq!(row.folder, Some("ExternalDataSources"));
        let output = extract(
            &text,
            fixture::OBJECT_UUID,
            "ExternalDataSource.Source435",
            version,
        )
        .unwrap();
        assert_eq!(
            output.relative_path,
            Path::new("ExternalDataSources/Source435.xml")
        );
        let xml = std::str::from_utf8(&output.xml).unwrap();
        assert_eq!(xml.matches("<xr:GeneratedType ").count(), 3);
        assert!(xml.contains("category=\"CubesManager\""));
        assert!(xml.contains("<ChildObjects/>"));
        assert_eq!(
            compiled("ExternalDataSource", xml, dialect),
            compiled(
                "ExternalDataSource",
                &fixture::external_data_source(dialect, "Automatic", [0, 1, 2]),
                dialect
            )
        );
    }
}

#[test]
fn declared_eds_corruption_never_falls_through_to_another_family_or_header() {
    let valid = compiled(
        "ExternalDataSource",
        &fixture::external_data_source("2.20", "Automatic", [0, 1, 2]),
        "2.20",
    );
    for (path, replacement) in [
        (vec![0], Brace::num(0)),
        (vec![1, 0], Brace::num(19)),
        (vec![1, 1, 0], Brace::num(1)),
        (vec![1, 8], Brace::num(1)),
        (vec![1, 9], Brace::num(7)),
        (vec![3, 1], Brace::num(1)),
    ] {
        let mut row = valid.clone();
        let mut target = &mut row;
        for slot in &path {
            target = &mut target.as_list_mut().unwrap()[*slot];
        }
        *target = replacement;
        let diagnostic = match extract(
            &serialize(&row),
            fixture::OBJECT_UUID,
            "ExternalDataSource.Source435",
            InfobaseConfigSourceVersion::V2_20,
        ) {
            Ok(_) => panic!("damaged EDS exported for {path:?}"),
            Err(error) => error,
        };
        assert_eq!(diagnostic.family, "ExternalDataSource", "{path:?}");
        assert_eq!(diagnostic.parser_stage, "canonical_empty_family");
    }
    let diagnostic = match extract(
        &serialize(&valid),
        fixture::JOB_UUID,
        "ExternalDataSource.Source435",
        InfobaseConfigSourceVersion::V2_20,
    ) {
        Ok(_) => panic!("wrong own UUID exported"),
        Err(error) => error,
    };
    assert_eq!(diagnostic.family, "ExternalDataSource");
}

#[test]
fn shared_code_two_scheduled_job_still_uses_its_existing_route() {
    const MODULE_UUID: &str = "43500000-0000-4000-8000-00000000000a";
    for (dialect, version) in [
        ("2.20", InfobaseConfigSourceVersion::V2_20),
        ("2.21", InfobaseConfigSourceVersion::V2_21),
    ] {
        let module = fixture::document(
            dialect,
            &format!(
                r#"<CommonModule uuid="{MODULE_UUID}"><Properties><Name>Handler435</Name><Synonym/><Comment/><Global>false</Global><ClientManagedApplication>false</ClientManagedApplication><Server>true</Server><ExternalConnection>false</ExternalConnection><ClientOrdinaryApplication>false</ClientOrdinaryApplication><ServerCall>false</ServerCall><Privileged>false</Privileged><ReturnValuesReuse>DontUse</ReturnValuesReuse></Properties></CommonModule>"#,
            ),
        );
        let job = fixture::scheduled_job(dialect).replace(
            "<MethodName/>",
            "<MethodName>CommonModule.Handler435.Run</MethodName>",
        );
        let files = vec![
            (
                PathBuf::from("CommonModules/Handler435.xml"),
                std::sync::Arc::new(module.as_bytes().to_vec()),
            ),
            (
                PathBuf::from("ScheduledJobs/Job435.xml"),
                std::sync::Arc::new(job.as_bytes().to_vec()),
            ),
        ];
        let context = DescriptorContext::with_files(Path::new("."), dialect, &files).unwrap();
        let module_row = parse_row(
            &compile_descriptor("CommonModule", &files[0].0, module.as_bytes(), &context).unwrap(),
        )
        .unwrap();
        let current_module =
            crate::metadata_model::export::object_names("CommonModule", &module_row).unwrap();
        assert_eq!(current_module.uuid, MODULE_UUID);
        assert_eq!(current_module.full_name, "CommonModule.Handler435");
        let refs = BTreeMap::from([(current_module.uuid, current_module.full_name)]);
        let job_row = parse_row(
            &compile_descriptor("ScheduledJob", &files[1].0, job.as_bytes(), &context).unwrap(),
        )
        .unwrap();
        let text = serialize(&job_row);
        let row = metadata_text_row_from_text(fixture::JOB_UUID, text.clone()).unwrap();
        assert_eq!(row.kind.as_deref(), Some("ScheduledJob"));
        // Neither an absent current module nor an unbound empty handler is a
        // positive witness for this physical route.
        assert!(extract(&text, fixture::JOB_UUID, "ScheduledJob.Job435", version).is_err());
        let empty_handler = serialize(&compiled(
            "ScheduledJob",
            &fixture::scheduled_job(dialect),
            dialect,
        ));
        assert!(
            extract_with_current_refs(
                &empty_handler,
                fixture::JOB_UUID,
                "ScheduledJob.Job435",
                version,
                &refs
            )
            .is_err()
        );
        let output = extract_with_current_refs(
            &text,
            fixture::JOB_UUID,
            "ScheduledJob.Job435",
            version,
            &refs,
        )
        .unwrap();
        assert_eq!(output.relative_path, Path::new("ScheduledJobs/Job435.xml"));
        let xml = std::str::from_utf8(&output.xml).unwrap();
        assert!(!xml.contains("ExternalDataSource"));
        assert!(xml.contains("<MethodName>CommonModule.Handler435.Run</MethodName>"));
    }
}

#[test]
fn malformed_synonym_counts_do_not_panic_or_publish_physical_xml() {
    let valid = compiled(
        "ExternalDataSource",
        &fixture::external_data_source("2.20", "Automatic", [0, 1, 2]),
        "2.20",
    );
    for count in [
        "-1",
        "9223372036854775807",
        "9223372036854775808",
        "18446744073709551615",
    ] {
        let mut row = valid.clone();
        *row.as_list_mut().unwrap()[1].as_list_mut().unwrap()[1]
            .as_list_mut()
            .unwrap()[1]
            .as_list_mut()
            .unwrap()
            .get_mut(3)
            .unwrap() = Brace::List(vec![Brace::atom(count)]);
        let text = serialize(&row);
        let nominal = metadata_text_row_from_text(fixture::OBJECT_UUID, text.clone()).unwrap();
        assert!(
            nominal.kind.is_none(),
            "corrupt count {count} cannot promote EDS"
        );
        let error = match extract(
            &text,
            fixture::OBJECT_UUID,
            "ExternalDataSource.Source435",
            InfobaseConfigSourceVersion::V2_20,
        ) {
            Ok(_) => panic!("corrupt synonym count {count} published XML"),
            Err(error) => error,
        };
        assert_eq!(error.family, "ExternalDataSource");
        assert_eq!(error.parser_stage, "canonical_empty_family");
    }
}
