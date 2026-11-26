//! The staging compiler (`stage_source_objects`) run against a container's
//! own entries instead of a database: an exported source tree, or the objects
//! of it under some path prefixes, compiled into the rows the container
//! stores. `cf load` uses it for the edits a module overlay cannot carry
//! (`crate::load`).
//!
//! The rows of a `.cf` are the rows of the `Config` table: an entry's payload
//! is the raw-deflated `BinaryData`. The compiler patches each object's base
//! row, so the base container must hold every object it compiles; an object
//! it has not got is compiled against no base row and fails, naming it.

use super::*;

/// The name the prefetched rows are filed under; no database is reached.
const OFFLINE_DATABASE: &str = "offline-container";

/// The metadata kind of a subsystem, as its XML names it.
const SUBSYSTEM_KIND: &str = "Subsystem";

/// The rows an offline compile gives.
#[derive(Debug, Default)]
pub struct CompiledContainerRows {
    /// Entry name -> packed payload, as the container stores it.
    pub rows: Vec<(String, Vec<u8>)>,
    /// The container's `versions` entry with the compiled rows' generations
    /// moved on (`None` for a container that keeps none, an extension).
    pub versions: Option<Vec<u8>>,
}

/// Compiles the objects of `source_root` under `prefixes` (every object when
/// empty) against `base`, a container's entries (name -> packed payload).
/// `fresh` (tree-relative metadata XML paths) are compiled base-free even when
/// the base has them: a parent whose child objects came or went (the patch of
/// a base row keeps the base's child lists). Once per process: the compiler
/// reads its base rows from a process-wide table.
pub fn compile_source_rows_offline(
    source_root: &Path,
    base: std::collections::HashMap<String, Vec<u8>>,
    prefixes: &[String],
    fresh: &[String],
) -> Result<CompiledContainerRows> {
    let has_versions = base.contains_key("versions");
    // The object rows' texts, to see whether a changed object's children
    // are still the base's.
    let base_rows_for_structure = base
        .iter()
        .filter(|(key, _)| !key.contains('.'))
        .filter_map(|(key, packed)| {
            let plain = crate::module_blob::inflate_raw(packed).ok()?;
            Some((key.clone(), String::from_utf8(plain).ok()?))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let base = base
        .into_iter()
        .map(|(name, packed)| (name, std::sync::Arc::new(packed)))
        .collect();
    if PREFETCHED_BASE_ROWS
        .set((OFFLINE_DATABASE.to_string(), base))
        .is_err()
    {
        bail!("an offline compile has already run in this process");
    }
    OFFLINE_STAGE.store(true, std::sync::atomic::Ordering::Relaxed);
    CF_LOAD_COMPILE.store(true, std::sync::atomic::Ordering::Relaxed);

    let manifest = scan_sources_with_prefixes(source_root, prefixes)?;
    let metadata_xmls = filter_source_paths_by_prefix(
        source_metadata_xmls(&manifest, source_root),
        source_root,
        prefixes,
    );
    let common_module_xmls = filter_source_paths_by_prefix(
        source_common_module_xmls(&manifest, source_root),
        source_root,
        prefixes,
    );
    // An object the base has not got is compiled base-free (after the
    // others: the base-free context stops every base-row read).
    let relative = |xml: &Path| {
        xml.strip_prefix(source_root)
            .unwrap_or(xml)
            .to_string_lossy()
            .replace('\\', "/")
    };
    let fresh = fresh
        .iter()
        .map(|path| path.replace('\\', "/"))
        .collect::<std::collections::HashSet<_>>();
    let in_base = |xml: &PathBuf| -> Result<bool> {
        if fresh.contains(&relative(xml)) {
            return Ok(false);
        }
        let bytes =
            fs::read(xml).with_context(|| format!("failed to read XML {}", xml.display()))?;
        let properties = parse_simple_metadata_xml_properties(&bytes)?;
        let metadata = crate::metadata_model::xml::MetadataXml::parse(&bytes)?;
        let adopted = metadata
            .object()?
            .child("Properties")
            .and_then(|properties| properties.child_text("ObjectBelonging"))
            .map(str::trim)
            == Some("Adopted");
        // A subsystem is compiled whole from its XML: the patch of its base
        // row keeps the base's content list and does not reach a nested
        // subsystem (#417: an edited content, top-level or nested, did not
        // export back), and the base-free compiler writes the БСП's 9 155
        // files back byte for byte.
        // An adopted subsystem's XML cannot express whether its always
        // printed Content is controlled. Keep its original adoption header
        // by patching the base row instead of inferring that state from XML.
        if properties.kind == SUBSYSTEM_KIND && !adopted {
            return Ok(false);
        }
        let Some(row) = base_rows_for_structure.get(&properties.uuid) else {
            return Ok(false);
        };
        // The root's child lists are the caller's (`cf load` rewrites its
        // families); its row is always patched.
        if properties.kind == "Configuration" || same_children(&bytes, row) {
            return Ok(true);
        }
        // A patch of the base row keeps its children (attributes, tabular
        // sections, ...): an object whose children came or went is compiled
        // base-free -- which an adopted object's XML, holding only what the
        // extension controls, cannot be.
        if adopted && properties.kind != "Form" {
            bail!(
                "{}: children added to or removed from an adopted object are not loaded yet",
                relative(xml)
            );
        }
        Ok(false)
    };
    let mut new_xmls = Vec::new();
    let mut metadata_xmls_in_base = Vec::new();
    for xml in metadata_xmls {
        if in_base(&xml)? {
            metadata_xmls_in_base.push(xml);
        } else {
            new_xmls.push(xml);
        }
    }
    let mut common_module_xmls_in_base = Vec::new();
    for xml in common_module_xmls {
        if in_base(&xml)? {
            common_module_xmls_in_base.push(xml);
        } else {
            new_xmls.push(xml);
        }
    }
    let (metadata_xmls, common_module_xmls) = (metadata_xmls_in_base, common_module_xmls_in_base);
    let source = MetadataSourceContext::new(source_root.to_path_buf());
    // Every row comes from PREFETCHED_BASE_ROWS; a request that reaches
    // the database is a bug and fails.
    let sql = SqlExec::detached("an offline compile reads no database");
    let sql = &sql;
    install_always_used_constants_source(sql, OFFLINE_DATABASE, Some(source_root));
    let metadata_objects = parallel::install(|| {
        metadata_xmls
            .par_iter()
            .map(|xml| {
                prepare_metadata_object_stage(sql, OFFLINE_DATABASE, xml.clone(), Some(&source))
            })
            .collect::<Result<Vec<_>>>()
    })??;
    let common_modules = parallel::install(|| {
        common_module_xmls
            .par_iter()
            .map(|xml| prepare_common_module_object_stage(sql, OFFLINE_DATABASE, xml.clone(), None))
            .collect::<Result<Vec<_>>>()
    })??;
    ensure_unique_source_stage_ids(&metadata_objects, &common_modules)?;

    let child_xmls = metadata_xmls
        .iter()
        .map(|path| {
            let xml = crate::metadata_model::xml::MetadataXml::parse(&fs::read(path)?)?;
            let id = xml
                .object()?
                .attr("uuid")
                .unwrap_or_default()
                .to_ascii_lowercase();
            Ok((id, xml))
        })
        .collect::<Result<std::collections::HashMap<_, _>>>()?;
    let mut rows = Vec::new();
    for object in &metadata_objects {
        let descriptor =
            patch_child_headers(child_xmls.get(&object.object_id), &object.metadata_blob)?;
        rows.push((object.object_id.clone(), descriptor));
        for body in &object.body_rows {
            rows.push((body.body_id.clone(), body.blob.clone()));
        }
    }
    for module in &common_modules {
        rows.push((module.module_id.clone(), module.metadata_blob.clone()));
        if module.has_module_body {
            rows.push((module.module_body_id.clone(), module.module_blob.clone()));
        }
    }
    if !new_xmls.is_empty() {
        new_xmls.sort();
        new_xmls.dedup();
        for object in super::empty_stage::prepare_empty_objects(source_root, &new_xmls)? {
            if let Some(failure) = object.failures.first() {
                bail!(
                    "the new object {} cannot be compiled ({} {}): {}",
                    object.relative,
                    failure.family,
                    failure.file_name.as_deref().unwrap_or(""),
                    failure.error
                );
            }
            rows.extend(object.rows.into_iter().map(|row| (row.file_name, row.blob)));
        }
    }
    let versions = if has_versions {
        // Both patched and base-free rows changed, including subsystem
        // descriptors compiled from new_xmls above.
        let changes = rows.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>();
        let blob = fetch_classified_versions_blob(
            sql,
            OFFLINE_DATABASE,
            &legacy_non_xml_compile_axes(),
            changes.len(),
        )?;
        Some(patch_versions_blob_bytes_allowing_additions(&blob, &changes, true)?.blob)
    } else {
        None
    };
    Ok(CompiledContainerRows { rows, versions })
}

/// The object XML's own and child uuids (`uuid="..."`) are the headers
/// (`{1,0,<uuid>}`) of its base row, no more and no fewer.
fn same_children(xml: &[u8], row: &str) -> bool {
    let xml = String::from_utf8_lossy(xml);
    let in_xml = xml
        .match_indices("uuid=\"")
        .filter_map(|(at, marker)| xml.get(at + marker.len()..at + marker.len() + 36))
        .map(str::to_ascii_lowercase)
        .collect::<std::collections::BTreeSet<_>>();
    let in_row = row
        .match_indices("{1,0,")
        .filter_map(|(at, marker)| row.get(at + marker.len()..at + marker.len() + 36))
        .filter(|uuid| uuid::Uuid::parse_str(uuid).is_ok())
        .map(str::to_ascii_lowercase)
        .collect::<std::collections::BTreeSet<_>>();
    in_xml == in_row
}

/// The staging patch preserves child collections, including their headers.
/// Apply edits to child names, synonyms and comments without rebuilding the
/// collection's unexported fields.
fn patch_child_headers(
    xml: Option<&crate::metadata_model::xml::MetadataXml>,
    packed: &[u8],
) -> Result<Vec<u8>> {
    use crate::metadata_model::{
        brace::{self, Brace},
        xml::Element,
    };
    type HeaderPatch = (Brace, [bool; 3]);
    fn collect(node: &Element, headers: &mut std::collections::HashMap<String, HeaderPatch>) {
        if let (Some(uuid), Some(properties)) = (node.attr("uuid"), node.child("Properties")) {
            headers.insert(
                uuid.to_ascii_lowercase(),
                (
                    crate::metadata_model::md_base(uuid, properties),
                    ["Name", "Synonym", "Comment"].map(|name| properties.child(name).is_some()),
                ),
            );
        }
        for child in &node.children {
            collect(child, headers);
        }
    }
    fn patch(node: &mut Brace, headers: &std::collections::HashMap<String, HeaderPatch>) {
        let Some(items) = node.as_list_mut() else {
            return;
        };
        if items.first().and_then(Brace::as_atom) == Some("3") {
            let uuid = items
                .get(1)
                .and_then(Brace::as_list)
                .and_then(|v| v.get(2))
                .and_then(Brace::as_atom);
            if let Some((header, present)) = uuid.and_then(|uuid| headers.get(uuid))
                && let Some(source) = header.as_list()
                && items.len() >= 5
            {
                for (index, present) in present.iter().enumerate() {
                    if *present {
                        items[index + 2] = source[index + 2].clone();
                    }
                }
            }
        }
        for item in items {
            patch(item, headers);
        }
    }
    if let Some(xml) = xml {
        let object = xml.object()?;
        let Some(children) = object.child("ChildObjects") else {
            return Ok(packed.to_vec());
        };
        let mut headers = std::collections::HashMap::new();
        collect(children, &mut headers);
        if headers.is_empty() {
            return Ok(packed.to_vec());
        }
        let mut row = brace::parse_row(&crate::module_blob::inflate_raw(packed)?)?;
        patch(&mut row, &headers);
        return crate::module_blob::deflate_raw(&brace::serialize_row(&row));
    }
    Ok(packed.to_vec())
}

#[cfg(test)]
mod child_header_tests {
    use super::*;

    #[test]
    fn edited_child_comment_preserves_unexported_header_fields() {
        let id = "aaaaaaaa-0000-0000-0000-000000000001";
        let xml = crate::metadata_model::xml::MetadataXml::parse(format!(
            "<MetaDataObject><DataProcessor><ChildObjects><Attribute uuid=\"{id}\"><Properties><Name>Field</Name><Synonym/><Comment>edited</Comment></Properties></Attribute></ChildObjects></DataProcessor></MetaDataObject>"
        ).as_bytes()).unwrap();
        let row = format!("{{1,{{3,{{1,0,{id}}},\"Field\",{{0}},\"old\",7,8,9,10}}}}");
        let packed = crate::module_blob::deflate_raw(row.as_bytes()).unwrap();
        let result = patch_child_headers(Some(&xml), &packed).unwrap();
        let result = crate::metadata_model::brace::parse_row(
            &crate::module_blob::inflate_raw(&result).unwrap(),
        )
        .unwrap();
        let expected = row.replace("\"old\"", "\"edited\"");
        assert_eq!(
            result,
            crate::metadata_model::brace::parse_row(expected.as_bytes()).unwrap()
        );
    }

    #[test]
    fn omitted_child_name_and_synonym_preserve_stored_values() {
        let id = "aaaaaaaa-0000-0000-0000-000000000001";
        let xml = crate::metadata_model::xml::MetadataXml::parse(format!(
            "<MetaDataObject><DataProcessor><ChildObjects><Attribute uuid=\"{id}\"><Properties><Comment>edited</Comment></Properties></Attribute></ChildObjects></DataProcessor></MetaDataObject>"
        ).as_bytes()).unwrap();
        let row = format!(
            "{{1,{{3,{{1,0,{id}}},\"StoredName\",{{1,\"ru\",\"Stored synonym\"}},\"old\",7,8,9,10}}}}"
        );
        let packed = crate::module_blob::deflate_raw(row.as_bytes()).unwrap();
        let result = patch_child_headers(Some(&xml), &packed).unwrap();
        assert_eq!(
            crate::metadata_model::brace::parse_row(
                &crate::module_blob::inflate_raw(&result).unwrap()
            )
            .unwrap(),
            crate::metadata_model::brace::parse_row(
                row.replace("\"old\"", "\"edited\"").as_bytes()
            )
            .unwrap()
        );
    }

    #[test]
    fn explicit_empty_child_fields_clear_stored_values() {
        let id = "aaaaaaaa-0000-0000-0000-000000000001";
        let xml = crate::metadata_model::xml::MetadataXml::parse(format!(
            "<MetaDataObject><DataProcessor><ChildObjects><Attribute uuid=\"{id}\"><Properties><Synonym/><Comment/></Properties></Attribute></ChildObjects></DataProcessor></MetaDataObject>"
        ).as_bytes()).unwrap();
        let row = format!(
            "{{1,{{3,{{1,0,{id}}},\"StoredName\",{{1,\"ru\",\"Stored synonym\"}},\"old\",7,8,9,10}}}}"
        );
        let packed = crate::module_blob::deflate_raw(row.as_bytes()).unwrap();
        let result = patch_child_headers(Some(&xml), &packed).unwrap();
        let expected = format!("{{1,{{3,{{1,0,{id}}},\"StoredName\",{{0}},\"\",7,8,9,10}}}}");
        assert_eq!(
            crate::metadata_model::brace::parse_row(
                &crate::module_blob::inflate_raw(&result).unwrap()
            )
            .unwrap(),
            crate::metadata_model::brace::parse_row(expected.as_bytes()).unwrap()
        );
    }
}

/// Bodies an offline compile could not write (kept from the base unless an
/// edit needed them), for the caller's error when one did.
static SKIPPED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub(super) fn note_skipped(what: String) {
    if let Ok(mut skipped) = SKIPPED.lock() {
        skipped.push(what);
    }
}

/// The bodies the offline compile of this process skipped.
pub fn skipped_bodies() -> Vec<String> {
    SKIPPED
        .lock()
        .map(|skipped| skipped.clone())
        .unwrap_or_default()
}

/// A module body entry (`<owner>.<n>`) packed from its text, as a stage
/// packs it (`pack_module_body_source`).
pub fn pack_module_text(key: &str, text: &[u8]) -> Result<Vec<u8>> {
    let classification = compile_mssql_source(
        &legacy_non_xml_compile_axes(),
        key,
        key,
        SourcePayload::ModuleText { text, info: None },
    )?;
    match classification.outcome() {
        StoragePatchOutcome::Compiled(payload) => Ok(payload.bytes().to_vec()),
        StoragePatchOutcome::NeedsBase { required, reason } => {
            bail!("module body {key} requires base row {required}: {reason}")
        }
        StoragePatchOutcome::Unsupported { reason } => {
            bail!("unsupported module body {key}: {reason}")
        }
    }
}

/// Whether an offline form must use the adoption adapter. XML spelling does
/// not decide this: quotes, attribute order and whitespace around '=' are all
/// interpreted by the strict XML reader.
pub(super) fn requires_adoption_adapter(form_xml: &[u8], metadata_xml: &[u8]) -> Result<bool> {
    if form_xml.is_empty() {
        return Ok(false);
    }
    let document = ibcmd_xml::XmlReader::from_slice(form_xml).context("Form.xml is malformed")?;
    let mut pending = vec![document.root()];
    let mut has_call_type = false;
    while let Some(node) = pending.pop() {
        if node.name().local() == "BaseForm" {
            return Ok(true);
        }
        has_call_type |= node.attributes().iter().any(|attribute| {
            matches!(attribute.kind(),
            ibcmd_xml::AttributeKind::Ordinary(name) if name.local() == "callType")
        });
        pending.extend(node.children().iter().filter_map(|child| match child {
            ibcmd_xml::XmlNode::Element(element) => Some(element),
            _ => None,
        }));
    }
    if !has_call_type {
        return Ok(false);
    }
    let metadata =
        ibcmd_xml::XmlReader::from_slice(metadata_xml).context("form metadata XML is malformed")?;
    let mut pending = vec![metadata.root()];
    while let Some(node) = pending.pop() {
        if node.name().local() == "ObjectBelonging" && node.children().iter().any(|child|
            matches!(child, ibcmd_xml::XmlNode::Text(text) if text.value().trim() == "Adopted")) {
            return Ok(true);
        }
        pending.extend(node.children().iter().filter_map(|child| match child {
            ibcmd_xml::XmlNode::Element(element) => Some(element),
            _ => None,
        }));
    }
    Ok(false)
}
// The adopted-form compile is the form writer's (`ibcmd-form-writer`).
pub(super) use adopted_form::adopted_form_body;
#[cfg(test)]
use adopted_form::{STAND_IN, split_form_base_xml, with_call_type_blocks, without_adoption};
use ibcmd_form_writer::adopted_form;

#[cfg(test)]
mod tests {
    #[test]
    fn noncanonical_events_containers_cannot_silently_compile_before() {
        let metadata =
            b"<MetaDataObject><ObjectBelonging>Adopted</ObjectBelonging></MetaDataObject>";
        for xml in [
            "<Form><Events ><Event callType='After' name='OnOpen'>Handler</Event></Events></Form>",
            "<Form xmlns:f='http://v8.1c.ru/8.3/xcf/logform'><f:Events><f:Event callType='After' name='OnOpen'>Handler</f:Event></f:Events></Form>",
        ] {
            assert!(super::requires_adoption_adapter(xml.as_bytes(), metadata).unwrap());
            assert!(
                super::without_adoption(xml)
                    .unwrap_err()
                    .to_string()
                    .contains("uncompiled interceptor")
            );
        }
        for xml in [
            "<Form xmlns:lf='http://v8.1c.ru/8.3/xcf/logform'><Commands><Command><lf:Action callType='Override'>Run</lf:Action></Command></Commands></Form>",
            "<Form xmlns:lf='http://v8.1c.ru/8.3/xcf/logform'><Events><Event lf:callType='After' name='OnOpen'>Handler</Event></Events></Form>",
        ] {
            assert!(super::requires_adoption_adapter(xml.as_bytes(), metadata).unwrap());
            assert!(super::without_adoption(xml).is_err());
        }
    }
    #[test]
    fn interceptor_xml_spelling_does_not_change_dispatch_or_codes() {
        let metadata = b"<MetaDataObject><CommonForm><Properties><ObjectBelonging>Adopted</ObjectBelonging></Properties></CommonForm></MetaDataObject>";
        for event in [
            "<Event name=\"OnOpen\" callType='After'>Handler</Event>",
            "<Event callType=\"After\" name=\"OnOpen\">Handler</Event>",
            "<Event name = 'OnOpen' callType = 'After'>Handler</Event>",
            "<Event callType = 'Override' name = 'OnOpen'>Handler</Event>",
        ] {
            let xml = format!("<Form><Events>{event}</Events></Form>");
            assert!(super::requires_adoption_adapter(xml.as_bytes(), metadata).unwrap());
            let (own, stand_ins) = super::without_adoption(&xml).unwrap();
            assert!(own.contains("ИбкмдПерехватчик0"));
            assert_eq!(stand_ins["ИбкмдПерехватчик0"][0].0, "Handler");
            assert_eq!(
                stand_ins["ИбкмдПерехватчик0"][0].1,
                if event.contains("Override") {
                    "Override"
                } else {
                    "After"
                }
            );
        }
        let xml = b"<Form><Events><Event callType = 'Future' name = 'OnOpen'>Handler</Event></Events></Form>";
        assert!(super::requires_adoption_adapter(xml, metadata).unwrap());
        assert!(super::without_adoption(std::str::from_utf8(xml).unwrap()).is_err());
    }

    #[test]
    fn event_spelling_in_a_comment_is_not_an_interceptor() {
        let xml = "<Form><Events><!-- <Event name='OnOpen' callType='Before'>Bogus</Event> --><Event callType='After' name='OnOpen'>Handler</Event></Events></Form>";
        let (_, bindings) = super::without_adoption(xml).unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(
            bindings["ИбкмдПерехватчик0"],
            vec![("Handler".to_owned(), "After".to_owned())]
        );
    }

    #[test]
    fn a_no_base_adopted_form_compiles_native_interceptor_shapes() {
        let base =
            crate::module_blob::pack_native_form_body_blob(FORM.as_bytes(), None, None, None)
                .unwrap();
        let base_parsed = crate::module_blob::parse_form_body_blob(&base.blob).unwrap();
        for (call_type, expected) in [
            (
                "Before",
                include_str!(
                    "../../tests/fixtures/native-evidence/extension-form-interceptors/before.block.txt"
                ),
            ),
            (
                "After",
                include_str!(
                    "../../tests/fixtures/native-evidence/extension-form-interceptors/after.block.txt"
                ),
            ),
            (
                "Override",
                include_str!(
                    "../../tests/fixtures/native-evidence/extension-form-interceptors/override.block.txt"
                ),
            ),
        ] {
            let event = format!(
                "\t<Events>\r\n\t\t<Event name=\"OnCreateAtServer\" callType=\"{call_type}\">ПриСозданииНаСервере</Event>\r\n\t</Events>\r\n"
            );
            let xml = FORM.replace("\t<Commands>", &format!("{event}\t<Commands>"));
            let packed =
                super::adopted_form_body(xml.as_bytes(), None, None, None, &base.blob).unwrap();
            let parsed = crate::module_blob::parse_form_body_blob(&packed).unwrap();
            assert!(
                parsed.layout.contains(expected.trim()),
                "{call_type}: {}",
                parsed.layout
            );
            assert_eq!(parsed.trailing[5].trim(), "0");
            assert_eq!(parsed.trailing[6].trim(), "0");
            assert_eq!(
                parsed.trailing, base_parsed.trailing,
                "commands and trailing form sections changed"
            );
        }
    }

    #[test]
    fn unknown_xml_call_types_and_unmeasured_command_types_are_refused() {
        for call_type in ["Future", "", "before"] {
            let xml = format!(
                "<Form><Events><Event name=\"OnOpen\" callType=\"{call_type}\">ПриСозданииНаСервере</Event></Events></Form>"
            );
            assert!(super::without_adoption(&xml).is_err());
        }
        assert!(
            super::without_adoption(
                "<Form><Events><Event name=\"OnOpen\" callType=\"After\">Handler</Events></Form>"
            )
            .is_err()
        );
        assert!(super::without_adoption("<Form><Commands><Command><Action callType=\"After\">Run</Action></Command></Commands></Form>").is_err());
    }

    const FORM: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\t<AutoCommandBar name=\"ФормаКоманднаяПанель\" id=\"-1\"/>\r\n\t<Commands>\r\n\t\t<Command name=\"Включить\" id=\"1\">\r\n\t\t\t<Title>\r\n\t\t\t\t<v8:item>\r\n\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n\t\t\t\t\t<v8:content>Включить все</v8:content>\r\n\t\t\t\t</v8:item>\r\n\t\t\t</Title>\r\n\t\t\t<ToolTip>\r\n\t\t\t\t<v8:item>\r\n\t\t\t\t\t<v8:lang>ru</v8:lang>\r\n\t\t\t\t\t<v8:content>Включить все</v8:content>\r\n\t\t\t\t</v8:item>\r\n\t\t\t</ToolTip>\r\n\t\t\t<Picture>\r\n\t\t\t\t<xr:Ref>StdPicture.CheckAll</xr:Ref>\r\n\t\t\t\t<xr:LoadTransparent>true</xr:LoadTransparent>\r\n\t\t\t</Picture>\r\n\t\t\t<Action>Включить</Action>\r\n\t\t\t<Representation>TextPicture</Representation>\r\n\t\t\t<CurrentRowUse>DontUse</CurrentRowUse>\r\n\t\t</Command>\r\n\t</Commands>\r\n</Form>";

    /// Diagnostics, not a check: every Form.xml under IBCMD_RS_FORMS_TREE
    /// through the native writer, the refusals counted by reason.
    #[test]
    #[ignore = "diagnostics over a local tree (IBCMD_RS_FORMS_TREE)"]
    fn native_writer_refusals_over_a_tree() {
        let Some(root) = std::env::var_os("IBCMD_RS_FORMS_TREE") else {
            return;
        };
        let root = std::path::PathBuf::from(root);
        let source = super::MetadataSourceContext::new(root.clone());
        let mut reasons = std::collections::BTreeMap::<String, usize>::new();
        let (mut total, mut refused) = (0, 0);
        for entry in walkdir::WalkDir::new(&root) {
            let entry = entry.unwrap();
            if entry.file_name() != "Form.xml" {
                continue;
            }
            total += 1;
            let xml = std::fs::read(entry.path()).unwrap();
            if xml.windows(10).any(|window| window == b"<BaseForm ") {
                continue;
            }
            let items = entry.path().with_file_name("Form").join("Items");
            if let Err(error) = crate::module_blob::pack_native_form_body_blob(
                &xml,
                None,
                Some(&source),
                Some(items.as_path()),
            ) {
                refused += 1;
                let reason = format!("{error:#}");
                let reason = reason.chars().take(90).collect::<String>();
                *reasons.entry(reason).or_default() += 1;
            }
        }
        let mut sorted = reasons.into_iter().collect::<Vec<_>>();
        sorted.sort_by(|left, right| right.1.cmp(&left.1));
        eprintln!("forms {total}, refused {refused}");
        for (reason, count) in sorted.iter().take(25) {
            eprintln!("{count:5}  {reason}");
        }
    }

    /// A table whose XML carries a dynamic list's own properties is one,
    /// though the form declares no attribute to say so (an adopted form:
    /// its list attribute is the base configuration's).
    #[test]
    fn a_table_with_dynamic_list_properties_is_written_as_one() {
        let table = "\t<ChildItems>\r\n\t\t<Table name=\"Список\" id=\"1\">\r\n\t\t\t<AutoRefresh>false</AutoRefresh>\r\n\t\t\t<AutoRefreshPeriod>60</AutoRefreshPeriod>\r\n\t\t\t<Period>\r\n\t\t\t\t<v8:variant xsi:type=\"v8:StandardPeriodVariant\">Custom</v8:variant>\r\n\t\t\t\t<v8:startDate>0001-01-01T00:00:00</v8:startDate>\r\n\t\t\t\t<v8:endDate>0001-01-01T00:00:00</v8:endDate>\r\n\t\t\t</Period>\r\n\t\t\t<ChoiceFoldersAndItems>Items</ChoiceFoldersAndItems>\r\n\t\t\t<RestoreCurrentRow>false</RestoreCurrentRow>\r\n\t\t\t<TopLevelParent xsi:nil=\"true\"/>\r\n\t\t\t<ShowRoot>true</ShowRoot>\r\n\t\t\t<AllowRootChoice>false</AllowRootChoice>\r\n\t\t\t<UpdateOnDataChange>Auto</UpdateOnDataChange>\r\n\t\t\t<ContextMenu name=\"СписокКонтекстноеМеню\" id=\"2\"/>\r\n\t\t\t<AutoCommandBar name=\"СписокКоманднаяПанель\" id=\"3\"/>\r\n\t\t\t<ExtendedTooltip name=\"СписокРасширеннаяПодсказка\" id=\"4\"/>\r\n\t\t\t<SearchStringAddition name=\"СписокСтрокаПоиска\" id=\"5\">\r\n\t\t\t\t<AdditionSource>\r\n\t\t\t\t\t<Item>Список</Item>\r\n\t\t\t\t\t<Type>SearchStringRepresentation</Type>\r\n\t\t\t\t</AdditionSource>\r\n\t\t\t\t<ContextMenu name=\"СписокСтрокаПоискаКонтекстноеМеню\" id=\"6\"/>\r\n\t\t\t\t<ExtendedTooltip name=\"СписокСтрокаПоискаРасширеннаяПодсказка\" id=\"7\"/>\r\n\t\t\t</SearchStringAddition>\r\n\t\t\t<ViewStatusAddition name=\"СписокСостояниеПросмотра\" id=\"8\">\r\n\t\t\t\t<AdditionSource>\r\n\t\t\t\t\t<Item>Список</Item>\r\n\t\t\t\t\t<Type>ViewStatusRepresentation</Type>\r\n\t\t\t\t</AdditionSource>\r\n\t\t\t\t<ContextMenu name=\"СписокСостояниеПросмотраКонтекстноеМеню\" id=\"9\"/>\r\n\t\t\t\t<ExtendedTooltip name=\"СписокСостояниеПросмотраРасширеннаяПодсказка\" id=\"10\"/>\r\n\t\t\t</ViewStatusAddition>\r\n\t\t\t<SearchControlAddition name=\"СписокУправлениеПоиском\" id=\"11\">\r\n\t\t\t\t<AdditionSource>\r\n\t\t\t\t\t<Item>Список</Item>\r\n\t\t\t\t\t<Type>SearchControl</Type>\r\n\t\t\t\t</AdditionSource>\r\n\t\t\t\t<ContextMenu name=\"СписокУправлениеПоискомКонтекстноеМеню\" id=\"12\"/>\r\n\t\t\t\t<ExtendedTooltip name=\"СписокУправлениеПоискомРасширеннаяПодсказка\" id=\"13\"/>\r\n\t\t\t</SearchControlAddition>\r\n\t\t</Table>\r\n\t</ChildItems>\r\n";
        let head = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\t<AutoCommandBar name=\"ФормаКоманднаяПанель\" id=\"-1\"/>\r\n";
        let without = format!("{head}{table}\t<Attributes/>\r\n</Form>");
        let layout = |xml: &str| {
            let packed =
                crate::module_blob::pack_native_form_body_blob(xml.as_bytes(), None, None, None)
                    .unwrap();
            let plain =
                String::from_utf8(crate::module_blob::inflate_raw(&packed.blob).unwrap()).unwrap();
            crate::module_blob::parse_form_body_plain(&plain)
                .unwrap()
                .layout
        };
        let bare = layout(&without);
        assert!(bare.contains("{\"N\",60}"), "{bare}");
    }

    /// The form head every round trip below opens with, and the extraction
    /// the exporter reads a written body back with.
    const FORM_HEAD: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"2.20\">\r\n\t<AutoCommandBar name=\"ФормаКоманднаяПанель\" id=\"-1\"/>\r\n";

    fn written_and_read_back(items: &str) -> String {
        let xml = format!(
            "{FORM_HEAD}\t<ChildItems>\r\n{items}\t</ChildItems>\r\n\t<Attributes/>\r\n</Form>"
        );
        let packed =
            crate::module_blob::pack_native_form_body_blob(xml.as_bytes(), None, None, None)
                .unwrap_or_else(|error| panic!("{error:#}"));
        crate::mssql_dump::extract_form_body_xml(&packed.blob, &std::collections::BTreeMap::new())
            .expect("the written body reads back")
    }

    /// Монитор `Catalogs/Запросы/Forms/ФормаАнализа`: a tooltip with nothing
    /// but its `DisplayImportance` keeps it through the writer and the
    /// exporter's reading of the record.
    #[test]
    fn an_empty_tooltip_keeps_its_display_importance_through_the_round_trip() {
        let items = "\t\t<LabelField name=\"Надпись\" id=\"1\">\r\n\t\t\t<ContextMenu name=\"НадписьКонтекстноеМеню\" id=\"2\"/>\r\n\t\t\t<ExtendedTooltip name=\"НадписьРасширеннаяПодсказка\" id=\"3\" DisplayImportance=\"VeryHigh\"/>\r\n\t\t</LabelField>\r\n";
        let read_back = written_and_read_back(items);
        assert!(
            read_back.contains("<ExtendedTooltip name=\"НадписьРасширеннаяПодсказка\" id=\"3\" DisplayImportance=\"VeryHigh\"/>"),
            "{read_back}"
        );
    }

    /// Документооборот's scheme fields exclude the scheme's own commands and
    /// turn `<AutoMaxWidth>` off; both reach the record and come back.
    #[test]
    fn a_graphical_schema_field_keeps_its_commands_and_auto_max_width() {
        let items = "\t\t<GraphicalSchemaField name=\"Схема\" id=\"1\">\r\n\t\t\t<TitleLocation>None</TitleLocation>\r\n\t\t\t<CommandSet>\r\n\t\t\t\t<ExcludedCommand>AlignLeft</ExcludedCommand>\r\n\t\t\t\t<ExcludedCommand>InsertItemStart</ExcludedCommand>\r\n\t\t\t\t<ExcludedCommand>Print</ExcludedCommand>\r\n\t\t\t</CommandSet>\r\n\t\t\t<AutoMaxWidth>false</AutoMaxWidth>\r\n\t\t\t<ContextMenu name=\"СхемаКонтекстноеМеню\" id=\"2\"/>\r\n\t\t\t<ExtendedTooltip name=\"СхемаРасширеннаяПодсказка\" id=\"3\"/>\r\n\t\t</GraphicalSchemaField>\r\n";
        let read_back = written_and_read_back(items);
        assert!(
            read_back.contains("<CommandSet>\r\n\t\t\t\t<ExcludedCommand>AlignLeft</ExcludedCommand>\r\n\t\t\t\t<ExcludedCommand>InsertItemStart</ExcludedCommand>\r\n\t\t\t\t<ExcludedCommand>Print</ExcludedCommand>\r\n\t\t\t</CommandSet>"),
            "{read_back}"
        );
        assert!(
            read_back.contains("<AutoMaxWidth>false</AutoMaxWidth>"),
            "{read_back}"
        );
    }

    /// Документооборот 3.0 `Catalogs/ПроектныеЗадачи/Forms/ФормаПланаПроекта`:
    /// the table a Gantt chart field nests keeps `id="0"`, and its three
    /// additions spell no `<AdditionSource>`; they serve that table, and
    /// come back the same way.
    #[test]
    fn gantt_table_additions_without_a_source_serve_their_table() {
        let items = "\t\t<GanttChartField name=\"Диаграмма\" id=\"1\">\r\n\t\t\t<ContextMenu name=\"ДиаграммаКонтекстноеМеню\" id=\"2\"/>\r\n\t\t\t<ExtendedTooltip name=\"ДиаграммаРасширеннаяПодсказка\" id=\"3\"/>\r\n\t\t\t<Table name=\"Table\" id=\"0\">\r\n\t\t\t\t<ContextMenu name=\"TableКонтекстноеМеню\" id=\"4\"/>\r\n\t\t\t\t<AutoCommandBar name=\"TableКоманднаяПанель\" id=\"5\"/>\r\n\t\t\t\t<ExtendedTooltip name=\"TableРасширеннаяПодсказка\" id=\"6\"/>\r\n\t\t\t\t<SearchStringAddition name=\"TableСтрокаПоиска\" id=\"7\">\r\n\t\t\t\t\t<ContextMenu name=\"TableСтрокаПоискаКонтекстноеМеню\" id=\"8\"/>\r\n\t\t\t\t\t<ExtendedTooltip name=\"TableСтрокаПоискаРасширеннаяПодсказка\" id=\"9\"/>\r\n\t\t\t\t</SearchStringAddition>\r\n\t\t\t\t<ViewStatusAddition name=\"TableСостояниеПросмотра\" id=\"10\">\r\n\t\t\t\t\t<ContextMenu name=\"TableСостояниеПросмотраКонтекстноеМеню\" id=\"11\"/>\r\n\t\t\t\t\t<ExtendedTooltip name=\"TableСостояниеПросмотраРасширеннаяПодсказка\" id=\"12\"/>\r\n\t\t\t\t</ViewStatusAddition>\r\n\t\t\t\t<SearchControlAddition name=\"TableУправлениеПоиском\" id=\"13\">\r\n\t\t\t\t\t<ContextMenu name=\"TableУправлениеПоискомКонтекстноеМеню\" id=\"14\"/>\r\n\t\t\t\t\t<ExtendedTooltip name=\"TableУправлениеПоискомРасширеннаяПодсказка\" id=\"15\"/>\r\n\t\t\t\t</SearchControlAddition>\r\n\t\t\t</Table>\r\n\t\t</GanttChartField>\r\n";
        let read_back = written_and_read_back(items);
        assert!(read_back.contains(items), "{read_back}");
        assert!(!read_back.contains("<AdditionSource>"), "{read_back}");
    }

    /// An interceptor with no handler compiles under a stand-in and is stored
    /// as the platform stores it: an empty first handler, then a second empty
    /// one (a real extension: `9cc34712…,0,2,"",0`).
    #[test]
    fn an_empty_interceptor_is_stored_with_two_empty_handlers() {
        let xml = "\t<Events>\r\n\t\t<Event name=\"OnOpen\" callType=\"Before\"></Event>\r\n\t</Events>\r\n";
        let (own, call_types) = super::without_adoption(xml).unwrap();
        let stand_in = format!("{}0", super::STAND_IN);
        assert!(
            own.contains(&format!("<Event name=\"OnOpen\">{stand_in}</Event>")),
            "{own}"
        );
        let block = format!(
            "{{1,3ccc650e-f631-4cae-8e33-3eaac610b5f9,\"{stand_in}\",1,0,3ccc650e-f631-4cae-8e33-3eaac610b5f9,0,1}}"
        );
        assert_eq!(
            super::with_call_type_blocks(&block, &call_types),
            "{1,3ccc650e-f631-4cae-8e33-3eaac610b5f9,\"\",1,0,3ccc650e-f631-4cae-8e33-3eaac610b5f9,0,2,\"\",0}"
        );
    }

    /// Two lines of one event (a handler before, then an empty one) are one
    /// stored event with two handlers; an `After` line an empty first
    /// handler and the handler second -- as a real extension stores them.
    #[test]
    fn an_event_s_lines_fold_into_one_stored_event() {
        let xml = "\t<Events>\r\n\t\t<Event name=\"OnCreateAtServer\" callType=\"Before\">Перед</Event>\r\n\t\t<Event name=\"OnCreateAtServer\" callType=\"Before\"></Event>\r\n\t\t<Event name=\"BeforeWriteAtServer\" callType=\"After\">После</Event>\r\n\t</Events>\r\n";
        let (own, stand_ins) = super::without_adoption(xml).unwrap();
        let (first, second) = (
            format!("{}0", super::STAND_IN),
            format!("{}1", super::STAND_IN),
        );
        assert_eq!(
            own,
            format!(
                "\t<Events>\r\n\t\t<Event name=\"OnCreateAtServer\">{first}</Event>\r\n\t\t<Event name=\"BeforeWriteAtServer\">{second}</Event>\r\n\t</Events>\r\n"
            )
        );
        let block = format!("{{2,a,\"{first}\",b,\"{second}\",1,0,a,0,1,b,0,1}}");
        assert_eq!(
            super::with_call_type_blocks(&block, &stand_ins),
            "{2,a,\"Перед\",b,\"\",1,0,a,0,2,\"\",0,b,0,2,\"После\",1}"
        );
    }

    /// `<UseForFoldersAndItems>` is a catalog's (or a characteristic type
    /// chart's) form property: written though the form declares no main
    /// attribute (an adopted form: its main attribute is the base form's).
    #[test]
    fn use_for_folders_and_items_is_written_without_a_main_attribute() {
        let form = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"2.20\">\r\n\t<UseForFoldersAndItems>Items</UseForFoldersAndItems>\r\n\t<AutoCommandBar name=\"ФормаКоманднаяПанель\" id=\"-1\"/>\r\n\t<Attributes/>\r\n</Form>";
        let packed =
            crate::module_blob::pack_native_form_body_blob(form.as_bytes(), None, None, None)
                .unwrap();
        let plain =
            String::from_utf8(crate::module_blob::inflate_raw(&packed.blob).unwrap()).unwrap();
        assert!(
            plain.contains("59ef2b80-c86b-11d5-a3c1-0050bae0a776"),
            "{plain}"
        );
    }

    /// A type the export could not name is written as its raw
    /// `<v8:TypeId>`; the writer stores that id as the reference type.
    #[test]
    fn a_raw_type_id_attribute_keeps_its_type() {
        let form = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"2.20\">\r\n\t<AutoCommandBar name=\"ФормаКоманднаяПанель\" id=\"-1\"/>\r\n\t<Attributes>\r\n\t\t<Attribute name=\"Значение\" id=\"1\">\r\n\t\t\t<Type>\r\n\t\t\t\t<v8:TypeId>53fde52a-3d5f-4030-a5b4-365e4b1eee72</v8:TypeId>\r\n\t\t\t</Type>\r\n\t\t</Attribute>\r\n\t</Attributes>\r\n</Form>";
        let packed =
            crate::module_blob::pack_native_form_body_blob(form.as_bytes(), None, None, None)
                .unwrap();
        let plain =
            String::from_utf8(crate::module_blob::inflate_raw(&packed.blob).unwrap()).unwrap();
        assert!(
            plain.contains("53fde52a-3d5f-4030-a5b4-365e4b1eee72"),
            "{plain}"
        );
    }

    /// The export spells the any-reference type `cfg:AnyRef` under older
    /// compatibility modes (`respell_any_ib_ref_by_compatibility`); the
    /// writer reads it back as `cfg:AnyIBRef`.
    #[test]
    fn an_any_ref_attribute_compiles_natively() {
        let form = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:cfg=\"http://v8.1c.ru/8.1/data/enterprise/current-config\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"2.20\">\r\n\t<AutoCommandBar name=\"ФормаКоманднаяПанель\" id=\"-1\"/>\r\n\t<Attributes>\r\n\t\t<Attribute name=\"Ссылка\" id=\"1\">\r\n\t\t\t<Type>\r\n\t\t\t\t<v8:Type>cfg:AnyRef</v8:Type>\r\n\t\t\t</Type>\r\n\t\t</Attribute>\r\n\t</Attributes>\r\n</Form>";
        let any_ib = form.replace("cfg:AnyRef", "cfg:AnyIBRef");
        let body = |xml: &str| {
            let packed =
                crate::module_blob::pack_native_form_body_blob(xml.as_bytes(), None, None, None)
                    .unwrap();
            crate::module_blob::inflate_raw(&packed.blob).unwrap()
        };
        assert_eq!(body(form), body(&any_ib));
    }

    #[test]
    fn a_command_representation_reaches_the_native_writer() {
        let packed =
            crate::module_blob::pack_native_form_body_blob(FORM.as_bytes(), None, None, None)
                .unwrap();
        let plain =
            String::from_utf8(crate::module_blob::inflate_raw(&packed.blob).unwrap()).unwrap();
        let at = plain
            .find("\"Включить\",2,")
            .or_else(|| plain.find("\"Включить\",3,"));
        eprintln!(
            "{}",
            &plain[at.unwrap_or(0).saturating_sub(10)..(at.unwrap_or(0) + 40).min(plain.len())]
        );
        assert!(plain.contains("\"Включить\",2,"), "{plain}");
    }
}

#[cfg(test)]
mod adopted_base_free_tests {
    use super::*;

    #[test]
    fn source_base_form_keeps_its_controls_and_module_separate() -> Result<()> {
        for dialect in ["2.20", "2.21"] {
            // One line deliberately: BaseForm is structural, not a tab/line
            // spelling. Reused ids in base and own must stay in separate roots.
            let xml = format!(
                r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="{dialect}"><AutoCommandBar name="OwnBar" id="-1"/><Events><Event name="OnOpen" callType="After">OwnHandler</Event></Events><ChildItems><LabelField name="OwnLabel" id="1"><ContextMenu name="OwnMenu" id="2"/><ExtendedTooltip name="OwnTip" id="3"/></LabelField></ChildItems><BaseForm version="{dialect}"><AutoCommandBar name="BaseBar" id="-1"/><Events><Event name="OnOpen">BaseHandler</Event></Events><ChildItems><LabelField name="BaseLabel" id="1"><ContextMenu name="BaseMenu" id="2"/><ExtendedTooltip name="BaseTip" id="3"/></LabelField></ChildItems></BaseForm></Form>"#
            );
            let body = crate::module_blob::compile_native_form_body(
                xml.as_bytes(),
                Some(b"Own module"),
                None,
                None,
            )?;
            let own = crate::module_blob::parse_form_body_plain(&body)?;
            assert!(own.layout.contains("OwnLabel"));
            assert!(!own.layout.contains("BaseLabel"));
            assert_eq!(own.module_text, "Own module");
            assert_eq!(own.trailing.get(5).map(|s| s.trim()), Some("1"));
            let base = crate::module_blob::parse_form_body_plain(
                own.trailing.get(6).context("missing base body")?,
            )?;
            assert!(base.layout.contains("BaseLabel"));
            assert!(!base.layout.contains("OwnLabel"));
            assert!(base.module_text.is_empty());
            assert!(body.contains("OwnHandler"));
            assert!(!body.contains(STAND_IN));
        }
        Ok(())
    }

    #[test]
    fn source_base_form_rejects_ambiguous_or_mixed_dialect_bases() {
        for xml in [
            r#"<Form version="2.20"><BaseForm version="2.20"/><BaseForm version="2.20"/></Form>"#,
            r#"<Form version="2.20"><BaseForm version="2.21"/></Form>"#,
            r#"<Form><BaseForm><BaseForm/></BaseForm></Form>"#,
        ] {
            assert!(split_form_base_xml(xml).is_err());
        }
        let xml = "<Form><!-- BaseForm --></Form>";
        assert!(split_form_base_xml(xml).unwrap().is_none());
    }
    #[test]
    fn source_base_form_native_bom_crlf_and_empty_nodes_keep_byte_spans() -> Result<()> {
        let xml = concat!(
            "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
            "<Form xmlns=\"http://v8.1c.ru/8.3/xcf/logform\" xmlns:v8=\"http://v8.1c.ru/8.1/data/core\" version=\"2.21\">\r\n",
            "\t<Title><v8:item><v8:lang>ru</v8:lang><v8:content>Своя &amp; форма</v8:content></v8:item></Title>\r\n",
            "\t<AutoCommandBar name=\"СвояПанель\" id=\"-1\"/>\r\n",
            "\t<ChildItems><LabelField name=\"СвояНадпись\" id=\"1\"><ContextMenu name=\"СвоеМеню\" id=\"2\"/><ExtendedTooltip name=\"СвояПодсказка\" id=\"3\"/></LabelField></ChildItems>\r\n",
            "\t<BaseForm version=\"2.21\">\r\n",
            "\t\t<AutoCommandBar name=\"БазоваяПанель\" id=\"-1\"/>\r\n",
            "\t\t<ChildItems><LabelField name=\"БазоваяНадпись\" id=\"1\"><ContextMenu name=\"БазовоеМеню\" id=\"2\"/><ExtendedTooltip name=\"БазоваяПодсказка\" id=\"3\"/></LabelField></ChildItems>\r\n",
            "\t</BaseForm>\r\n</Form>",
        );
        let (own, base) = split_form_base_xml(xml)?.context("native-shaped base is missing")?;
        assert!(own.starts_with('\u{feff}'));
        assert!(own.contains("СвояНадпись"));
        assert!(!own.contains("БазоваяНадпись"));
        assert!(!own.contains("<BaseForm"));
        assert!(base.starts_with("<Form "));
        assert!(base.contains("БазоваяНадпись"));
        assert!(!base.contains("СвояНадпись"));
        ibcmd_xml::XmlReader::from_slice(own.as_bytes())?;
        ibcmd_xml::XmlReader::from_slice(base.as_bytes())?;
        let body = crate::module_blob::compile_native_form_body(xml.as_bytes(), None, None, None)?;
        let own = crate::module_blob::parse_form_body_plain(&body)?;
        assert!(!own.layout.contains("БазоваяНадпись"));
        let base = crate::module_blob::parse_form_body_plain(
            own.trailing.get(6).context("base record is missing")?,
        )?;
        assert!(base.layout.contains("БазоваяНадпись"));
        Ok(())
    }

    #[test]
    #[ignore = "local native source XML via IBCMD_RS_NATIVE_ADOPTED_FORM"]
    fn local_native_adopted_form_split_preserves_scopes() -> Result<()> {
        let path = std::path::PathBuf::from(
            std::env::var_os("IBCMD_RS_NATIVE_ADOPTED_FORM")
                .context("native XML path is missing")?,
        );
        let xml = std::fs::read_to_string(path)?;
        let (own, base) = split_form_base_xml(&xml)?.context("native BaseForm is missing")?;
        let original = ibcmd_xml::XmlReader::from_slice(xml.as_bytes())?;
        let own = ibcmd_xml::XmlReader::from_slice(own.as_bytes())?;
        let base = ibcmd_xml::XmlReader::from_slice(base.as_bytes())?;
        fn child_elements(element: &ibcmd_xml::XmlElement) -> Vec<&ibcmd_xml::XmlElement> {
            element
                .children()
                .iter()
                .filter_map(|n| match n {
                    ibcmd_xml::XmlNode::Element(e) => Some(e),
                    _ => None,
                })
                .collect::<Vec<_>>()
        }
        let original_base = child_elements(original.root())
            .into_iter()
            .find(|n| n.name().local() == "BaseForm")
            .context("original BaseForm is missing")?;
        let own_names = child_elements(own.root())
            .iter()
            .map(|n| n.name().local().to_owned())
            .collect::<Vec<_>>();
        let original_names = child_elements(original.root())
            .iter()
            .filter(|n| n.name().local() != "BaseForm")
            .map(|n| n.name().local().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(own_names, original_names);
        let base_names = child_elements(base.root())
            .iter()
            .map(|n| n.name().local().to_owned())
            .collect::<Vec<_>>();
        let expected_base_names = child_elements(original_base)
            .iter()
            .map(|n| n.name().local().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(base_names, expected_base_names);
        println!(
            "native scopes preserved: own {} sections, base {} sections",
            own_names.len(),
            base_names.len()
        );
        Ok(())
    }
}
