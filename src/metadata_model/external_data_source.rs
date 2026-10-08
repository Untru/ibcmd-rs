//! The empty ExternalDataSource native layout, both directions. The canonical
//! XML adapter admits namespaces/identities; the schema owns physical slots.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_schema::external_data_source as schema;
use ibcmd_xml::{XmlReader, decode_empty_external_data_source};

use super::brace::{Brace, NIL_UUID, parse_row};
use super::common::Header;
use super::export::values::{header, header_elements};
use super::export::{Build, GeneratedTypeName, ObjectNames, atom, el, leaf, list, write_document};
use super::xml::Element;
use super::{DescriptorContext, ObjectXml};
use crate::brace_list;

#[derive(Debug)]
struct Record {
    head: Header,
    generated: [(String, String); 3],
    lock_mode: i64,
}

/// Exact original source admission precedes the legacy local-name DOM.
pub(crate) fn validate_source(xml: &[u8], version: &str) -> Result<()> {
    let document = XmlReader::from_slice(xml)?;
    decode_empty_external_data_source(
        &document,
        ProfileId::parse(&format!("xml-{version}"))?,
        ObjectPath::root(),
    )?;
    Ok(())
}

fn uuid(value: &str) -> Result<String> {
    let parsed = ObjectUuid::parse(value)?;
    if parsed.as_bytes().iter().all(|byte| *byte == 0) {
        bail!("ExternalDataSource identity is nil");
    }
    Ok(parsed.to_string())
}

pub(crate) fn compile(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    // This path also serves decoded, host-owned DOMs in lossless audits.
    // compile_descriptor validates original bytes before local-name decoding.
    let bytes = write_document(object.element, &context.version);
    let document = XmlReader::from_slice(bytes.as_bytes())?;
    let canonical = decode_empty_external_data_source(
        &document,
        ProfileId::parse(&format!("xml-{}", context.version))?,
        ObjectPath::root(),
    )?;
    let head = Header::of(object)?;
    if head.uuid != canonical.root().identity().uuid().to_string() || head.name != object.name {
        bail!("ExternalDataSource descriptor identity differs");
    }
    let mut generated: [(String, String); 3] = Default::default();
    for (ordinal, category) in schema::GENERATED_CATEGORIES.iter().enumerate() {
        let item = canonical
            .root()
            .generated_types()
            .iter()
            .find(|item| item.kind().as_str() == category.category)
            .ok_or_else(|| anyhow!("ExternalDataSource missing generated category"))?;
        generated[ordinal] = (
            item.uuid().to_string(),
            item.value_id()
                .ok_or_else(|| anyhow!("ExternalDataSource missing generated ValueId"))?
                .to_string(),
        );
    }
    let properties = object.properties()?;
    let lock_mode = schema::data_lock_code(
        properties
            .child_text("DataLockControlMode")
            .unwrap_or_default(),
    )
    .ok_or_else(|| anyhow!("ExternalDataSource unknown DataLockControlMode"))?;
    Ok(Record {
        head,
        generated,
        lock_mode,
    }
    .to_brace())
}

impl Record {
    fn to_brace(&self) -> Brace {
        let mut body = vec![
            Brace::atom(schema::RECORD_CODE),
            brace_list![Brace::num(0), self.head.to_brace()],
        ];
        for (type_id, value_id) in &self.generated {
            body.push(Brace::uuid(type_id));
            body.push(Brace::uuid(value_id));
        }
        body.push(Brace::num(0));
        body.push(Brace::num(self.lock_mode));
        let mut outer = vec![Brace::num(1), Brace::List(body), Brace::num(3)];
        outer.extend(
            schema::EMPTY_COLLECTIONS
                .iter()
                .map(|id| brace_list![Brace::uuid(id), Brace::num(0)]),
        );
        Brace::List(outer)
    }

    fn from_brace(row: &Brace) -> Result<Self> {
        let outer = list(row)?;
        if outer.len() != schema::OUTER_ARITY || atom(&outer[0])? != "1" || atom(&outer[2])? != "3"
        {
            bail!("ExternalDataSource invalid outer layout");
        }
        let body = list(&outer[1])?;
        if body.len() != schema::BODY_ARITY
            || atom(&body[0])? != schema::RECORD_CODE
            || atom(&body[8])? != "0"
        {
            bail!("ExternalDataSource invalid body layout");
        }
        let identity = list(&body[1])?;
        if identity.len() != 2 || atom(&identity[0])? != "0" {
            bail!("ExternalDataSource invalid identity wrapper");
        }
        let stored_head = list(&identity[1])?;
        let decoded_head = header(&identity[1])?;
        let own_identity = list(&stored_head[1])?;
        if own_identity.len() != 3
            || atom(&own_identity[0])? != "1"
            || atom(&own_identity[1])? != "0"
            || [5, 6, 8]
                .iter()
                .any(|slot| stored_head[*slot].as_atom() != Some("0"))
            || atom(&stored_head[7])? != NIL_UUID
        {
            bail!("ExternalDataSource invalid header identity/tail");
        }
        uuid(&decoded_head.uuid)?;
        if decoded_head.name.is_empty() {
            bail!("ExternalDataSource empty name");
        }
        let [name, synonym, comment] = header_elements(&decoded_head)?;
        let properties = el("Properties").child(name).child(synonym).child(comment);
        let head = Header::from_properties(&decoded_head.uuid, &properties);
        let mut languages = BTreeSet::new();
        if head
            .synonym
            .iter()
            .any(|(language, _)| language.is_empty() || !languages.insert(language))
        {
            bail!("ExternalDataSource invalid synonym languages");
        }
        let mut generated: [(String, String); 3] = Default::default();
        let mut seen = BTreeSet::from([head.uuid.clone()]);
        for (index, category) in schema::GENERATED_CATEGORIES.iter().enumerate() {
            let type_id = uuid(atom(&body[category.type_slot])?)?;
            let value_id = uuid(atom(&body[category.value_slot])?)?;
            if !seen.insert(type_id.clone()) || !seen.insert(value_id.clone()) {
                bail!("ExternalDataSource duplicate generated identity");
            }
            generated[index] = (type_id, value_id);
        }
        let mode = schema::data_lock_name(atom(&body[9])?)
            .ok_or_else(|| anyhow!("ExternalDataSource unknown lock mode"))?;
        let lock_mode = schema::data_lock_code(mode).expect("schema inverse");
        for (collection, expected) in outer[3..].iter().zip(schema::EMPTY_COLLECTIONS) {
            let fields = list(collection)?;
            if fields.len() != 2 || atom(&fields[0])? != expected || atom(&fields[1])? != "0" {
                bail!("ExternalDataSource requires the exact empty child collections");
            }
        }
        Ok(Self {
            head,
            generated,
            lock_mode,
        })
    }

    fn element(&self) -> Element {
        let mut internal = el("InternalInfo");
        for (category, (type_id, value_id)) in
            schema::GENERATED_CATEGORIES.iter().zip(&self.generated)
        {
            internal.children.push(
                el("xr:GeneratedType")
                    .attr("name", schema::generated_name(*category, &self.head.name))
                    .attr("category", category.category)
                    .child(leaf("xr:TypeId", type_id))
                    .child(leaf("xr:ValueId", value_id)),
            );
        }
        let header_tree = self.head.to_brace();
        let head = header(&header_tree).expect("validated header");
        let fields = header_elements(&head).expect("validated localized string");
        el(schema::KIND)
            .attr("uuid", &self.head.uuid)
            .child(internal)
            .child(el("Properties").children(fields).child(leaf(
                "DataLockControlMode",
                schema::data_lock_name(&self.lock_mode.to_string()).expect("validated mode"),
            )))
            .child(el("ChildObjects"))
    }
}

pub(crate) fn decode(row: &Brace) -> Result<Element> {
    Ok(Record::from_brace(row)?.element())
}

pub(crate) fn names(row: &Brace) -> Result<ObjectNames> {
    let record = Record::from_brace(row)?;
    Ok(ObjectNames {
        uuid: record.head.uuid.clone(),
        full_name: format!("{}.{}", schema::KIND, record.head.name),
        children: Vec::new(),
        types: schema::GENERATED_CATEGORIES
            .iter()
            .zip(&record.generated)
            .map(|(category, (type_id, value_id))| GeneratedTypeName {
                name: schema::generated_name(*category, &record.head.name),
                category: category.category.to_string(),
                type_id: type_id.clone(),
                value_id: value_id.clone(),
            })
            .collect(),
    })
}

pub(crate) fn parse(text: &str) -> Result<Brace> {
    crate::compiler::families::native::validate_descriptor_layout(
        text.as_bytes(),
        schema::MAX_VALUE_DEPTH,
    )?;
    let row = parse_row(text.as_bytes())?;
    Record::from_brace(&row)?;
    Ok(row)
}

/// A complete, validated EDS never enters code2's SettingsStorage or
/// ScheduledJob classification. Unsupported recognized shapes return no kind.
pub(crate) fn source_family(
    text: &str,
    expected_uuid: &str,
) -> Option<(&'static str, &'static str)> {
    let row = parse(text).ok()?;
    let found = names(&row).ok()?;
    (found.uuid == expected_uuid).then_some((schema::KIND, schema::COLLECTION))
}

/// The exact object name and complete canonical XML, from one parsed row.
pub(crate) fn export_source(
    text: &str,
    expected_uuid: &str,
    version: &str,
) -> Result<(String, String)> {
    let row = parse(text)?;
    let record = Record::from_brace(&row)?;
    if record.head.uuid != expected_uuid {
        bail!("ExternalDataSource row identity differs");
    }
    let result = write_document(&record.element(), version);
    validate_source(result.as_bytes(), version)?;
    Ok((record.head.name, result))
}

#[cfg(test)]
pub(crate) mod generated {
    use crate as product;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/common/eds_generated.rs"
    ));
}

#[cfg(test)]
mod tests {
    use super::super::brace::{serialize, serialize_row};
    use super::super::{compile_descriptor, index::ConfigIndex};
    use super::generated as fixture;
    use super::*;
    use crate::module_blob::MetadataSourceContext;
    use std::path::{Path, PathBuf};

    fn context(version: &str) -> DescriptorContext {
        DescriptorContext {
            root: PathBuf::from("."),
            index: ConfigIndex::default(),
            source: MetadataSourceContext::new(PathBuf::from(".")),
            version: version.into(),
        }
    }
    fn compile(xml: &str, version: &str) -> Result<Brace> {
        parse_row(&compile_descriptor(
            schema::KIND,
            Path::new("ExternalDataSources/Source435.xml"),
            xml.as_bytes(),
            &context(version),
        )?)
    }
    fn set(row: &mut Brace, path: &[usize], value: Brace) {
        let mut node = row;
        for slot in path {
            node = &mut node.as_list_mut().unwrap()[*slot];
        }
        *node = value;
    }

    #[test]
    fn generated_empty_family_round_trips_every_pair_and_enum_both_profiles() {
        for version in ["2.20", "2.21"] {
            for (mode, code) in [
                ("Automatic", "0"),
                ("Managed", "1"),
                ("AutomaticAndManaged", "2"),
            ] {
                let source = fixture::external_data_source(version, mode, [2, 0, 1]);
                let row = compile(&source, version).unwrap();
                assert_eq!(row.as_list().unwrap().len(), 6);
                assert_eq!(row.at(&[1, 9]).unwrap().as_atom(), Some(code));
                for (ordinal, (type_id, value_id)) in fixture::PAIRS.iter().enumerate() {
                    assert_eq!(
                        row.at(&[1, 2 + 2 * ordinal]).unwrap().as_atom(),
                        Some(*type_id)
                    );
                    assert_eq!(
                        row.at(&[1, 3 + 2 * ordinal]).unwrap().as_atom(),
                        Some(*value_id)
                    );
                }
                let indexed = super::super::export::object_names(schema::KIND, &row).unwrap();
                assert_eq!(indexed.types.len(), 3);
                assert_eq!(indexed.full_name, "ExternalDataSource.Source435");
                let (name, xml) =
                    export_source(&serialize(&row), fixture::OBJECT_UUID, version).unwrap();
                assert_eq!(name, fixture::NAME);
                assert_eq!(xml.matches("<xr:GeneratedType ").count(), 3);
                assert!(xml.contains("category=\"CubesManager\""));
                assert!(xml.contains("<ChildObjects/>"));
                assert!(xml.contains("line1\nquoted \"text\""));
                assert!(xml.starts_with('\u{feff}'));
                assert_eq!(compile(&xml, version).unwrap(), row);
                assert_eq!(
                    source_family(&serialize(&row), fixture::OBJECT_UUID),
                    Some((schema::KIND, schema::COLLECTION))
                );
                assert!(export_source(&serialize(&row), fixture::JOB_UUID, version).is_err());
            }
        }
    }

    #[test]
    fn original_xml_namespace_identity_presence_and_unknown_fields_are_not_lost() {
        let source = fixture::external_data_source("2.20", "Automatic", [0, 1, 2]);
        let variants = [
            source.replace("http://v8.1c.ru/8.3/MDClasses", "urn:foreign"),
            source.replace("http://v8.1c.ru/8.3/xcf/readable", "urn:foreign"),
            source.replace("http://v8.1c.ru/8.1/data/core", "urn:foreign"),
            source.replace("category=\"TablesManager\"", "category=\"Manager\""),
            source.replace(
                "ExternalDataSourceTablesManager.Source435",
                "ExternalDataSourceTablesManager.Other",
            ),
            source.replace(fixture::PAIRS[2].0, fixture::PAIRS[1].0),
            source.replace(fixture::PAIRS[2].1, fixture::PAIRS[1].1),
            source.replace(fixture::OBJECT_UUID, NIL_UUID),
            source.replace(fixture::PAIRS[0].0, NIL_UUID),
            source.replace(fixture::PAIRS[0].1, NIL_UUID),
            source.replace("<Comment>", "<Comment extra=\"lost\">"),
            source.replace(
                "<ChildObjects/>",
                "<ChildObjects><Table>Unknown</Table></ChildObjects>",
            ),
            source.replace("<ChildObjects/>", ""),
            source.replace("<DataLockControlMode>Automatic</DataLockControlMode>", ""),
            source.replace(
                "<DataLockControlMode>Automatic</DataLockControlMode>",
                "<DataLockControlMode>Foreign</DataLockControlMode>",
            ),
            source.replace("</Properties>", "<Future>lost</Future></Properties>"),
            source.replace("</Properties>", "<Name>Duplicate</Name></Properties>"),
            source.replace("<xr:TypeId>", "<xr:TypeId future=\"lost\">"),
        ];
        for (ordinal, xml) in variants.iter().enumerate() {
            assert!(
                compile(xml, "2.20").is_err(),
                "negative source case {ordinal}"
            );
        }
        assert!(
            compile(&source, "2.21").is_err(),
            "source dialect must match the requested profile"
        );
    }

    #[test]
    fn malformed_native_layout_never_promotes_or_exports_a_header_projection() {
        let valid = compile(
            &fixture::external_data_source("2.20", "Automatic", [0, 1, 2]),
            "2.20",
        )
        .unwrap();
        for (path, value) in [
            (vec![0], Brace::num(0)),
            (vec![2], Brace::num(2)),
            (vec![1, 0], Brace::num(3)),
            (vec![1, 8], Brace::num(1)),
            (vec![1, 9], Brace::num(3)),
            (vec![1, 1, 0], Brace::num(1)),
            (vec![1, 1, 1, 1, 0], Brace::num(0)),
            (vec![1, 1, 1, 5], Brace::num(1)),
            (
                vec![1, 1, 1, 3],
                brace_list![
                    Brace::num(2),
                    Brace::str("en"),
                    Brace::str("one"),
                    Brace::str("en"),
                    Brace::str("two")
                ],
            ),
            (vec![1, 2], Brace::str(fixture::PAIRS[0].0)),
            (vec![1, 7], Brace::uuid(fixture::PAIRS[0].1)),
            (vec![3, 0], Brace::uuid(fixture::OBJECT_UUID)),
            (vec![3, 1], Brace::num(1)),
        ] {
            let mut row = valid.clone();
            set(&mut row, &path, value);
            assert!(decode(&row).is_err(), "bad slot {path:?}");
            let text = serialize(&row);
            assert_eq!(source_family(&text, fixture::OBJECT_UUID), None);
            assert!(export_source(&text, fixture::OBJECT_UUID, "2.20").is_err());
        }
        let mut row = valid.clone();
        row.as_list_mut().unwrap().push(Brace::num(0));
        assert!(decode(&row).is_err());
        let mut trailing = serialize_row(&valid);
        trailing.extend_from_slice(b"{0}");
        assert!(parse(std::str::from_utf8(&trailing).unwrap()).is_err());
        let deep = format!("{}0{}", "{".repeat(4096), "}".repeat(4096));
        assert!(parse(&deep).is_err()); // canonical scanner refuses before recursive Brace allocation.
    }

    #[test]
    fn scheduled_job_shared_code_remains_a_different_layout() {
        let row = parse_row(
            &compile_descriptor(
                "ScheduledJob",
                Path::new("ScheduledJobs/Job435.xml"),
                fixture::scheduled_job("2.20").as_bytes(),
                &context("2.20"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(row.as_list().unwrap().len(), 3);
        assert_eq!(row.at(&[1, 0]).unwrap().as_atom(), Some("2"));
        assert_eq!(row.at(&[1]).unwrap().as_list().unwrap().len(), 10);
        assert!(decode(&row).is_err());
        assert_eq!(source_family(&serialize(&row), fixture::JOB_UUID), None);
    }

    #[test]
    fn corrupt_localized_counts_return_errors_without_panicking() {
        let valid = compile(
            &fixture::external_data_source("2.20", "Automatic", [0, 1, 2]),
            "2.20",
        )
        .unwrap();
        for count in [
            "-1",
            "9223372036854775807",
            "9223372036854775808",
            "18446744073709551615",
        ] {
            let mut row = valid.clone();
            set(&mut row, &[1, 1, 1, 3], brace_list![Brace::atom(count)]);
            assert!(decode(&row).is_err(), "bad localized count {count}");
            assert!(names(&row).is_err(), "bad localized count {count}");
            let text = serialize(&row);
            assert_eq!(
                source_family(&text, fixture::OBJECT_UUID),
                None,
                "count {count}"
            );
            assert!(
                export_source(&text, fixture::OBJECT_UUID, "2.20").is_err(),
                "count {count}"
            );
        }
    }

    #[test]
    fn generated_configuration_retains_the_declared_eds_and_job_binary_families() {
        for version in ["2.20", "2.21"] {
            let root = Path::new(".");
            let config = fixture::configuration(version);
            let files = vec![
                (
                    root.join("Configuration.xml"),
                    std::sync::Arc::new(config.as_bytes().to_vec()),
                ),
                (
                    root.join("ExternalDataSources/Source435.xml"),
                    std::sync::Arc::new(
                        fixture::external_data_source(version, "Automatic", [0, 1, 2]).into_bytes(),
                    ),
                ),
                (
                    root.join("ScheduledJobs/Job435.xml"),
                    std::sync::Arc::new(fixture::scheduled_job(version).into_bytes()),
                ),
            ];
            let context = DescriptorContext::with_files(root, version, &files).unwrap();
            // Preserve the original incomplete fixture as a negative witness:
            // child names alone do not authorize seven contained root IDs.
            let start = config.find("<InternalInfo>").unwrap();
            let end =
                config[start..].find("</InternalInfo>").unwrap() + start + "</InternalInfo>".len();
            let incomplete = format!("{}{}", &config[..start], &config[end..]);
            assert!(
                compile_descriptor(
                    "Configuration",
                    &root.join("Configuration.xml"),
                    incomplete.as_bytes(),
                    &context,
                )
                .is_err()
            );
            let row = parse_row(
                &compile_descriptor(
                    "Configuration",
                    &root.join("Configuration.xml"),
                    config.as_bytes(),
                    &context,
                )
                .unwrap(),
            )
            .unwrap();
            let objects = super::super::export::configuration_objects(&row).unwrap();
            assert!(objects.contains(&("ExternalDataSource".into(), fixture::OBJECT_UUID.into())));
            assert!(objects.contains(&("ScheduledJob".into(), fixture::JOB_UUID.into())));
            assert_eq!(objects.len(), 2);
            assert_eq!(
                context.index.configuration_uuid.as_deref(),
                Some(fixture::CONFIGURATION_UUID)
            );
        }
    }

    #[test]
    fn large_comment_has_no_new_source_or_row_byte_ceiling() {
        let comment = "x".repeat(8 * 1_048_576 + 1);
        let source = fixture::external_data_source("2.20", "Automatic", [0, 1, 2])
            .replace("line1&#10;quoted \"text\"", &comment);
        let row = compile(&source, "2.20").unwrap();
        assert_eq!(
            row.at(&[1, 1, 1, 4]).unwrap().as_str().unwrap().len(),
            comment.len()
        );
        let (_, output) = export_source(&serialize(&row), fixture::OBJECT_UUID, "2.20").unwrap();
        assert_eq!(compile(&output, "2.20").unwrap(), row);
    }
}
