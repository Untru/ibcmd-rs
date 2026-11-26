//! The packers of a configuration's stored bodies are the
//! `ibcmd-metadata-blob` crate's. This module keeps the root's paths to them
//! (`crate::module_blob::…`, the public ones included) and the entry points
//! that read the rest of the root: the command-line wrappers, the form
//! compiler's 8.5 and adopted-form routes and the form readiness audit (the
//! exporter's 2.21 converters), the role-rights packer's read-back through
//! the exporter, the command-interface packer that patches a base blob, and
//! the help-page packer with its submodule. The moxel template packers live
//! beside their writer (`compiler::bodies::mxl_native`) and keep their paths
//! here.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
// `help_pages`' tests read it through `use super::*`.
#[cfg(test)]
use uuid::Uuid;

use crate::cli::{ModuleBlobPackArgs, VersionsBlobPatchArgs};

// The items that moved keep their paths here: the public ones as they were,
// and the rest for the root's own code (their crate makes them `pub` for it).
pub(crate) use ibcmd_metadata_blob::*;
pub use ibcmd_metadata_blob::{
    CommonModuleXmlProperties, LocalizedString, MetadataSourceContext, ModuleBlobPackReport,
    PackedCommonModuleMetadataBlob, PackedExtPictureBlob, PackedHelpBlob, PackedScheduleBlob,
    PackedSimpleMetadataBlob, PackedStyleBodyBlob, PredefinedPatch, ReturnValuesReuse,
    SimpleMetadataXmlProperties, SpreadsheetNumberFormatHint, VersionsBlobPatchReport,
    business_process_flowchart_base_free_blockers, common_module_metadata_base_free_blockers,
    metadata_xml_base_free_blockers, pack_additional_indexes_blob_from_xml,
    pack_base64_payload_blob_from_bytes, pack_business_process_flowchart_blob_from_xml,
    pack_common_module_metadata_blob_from_xml, pack_exchange_plan_content_blob_from_xml,
    pack_ext_picture_blob_from_bytes, pack_ext_picture_blob_from_bytes_with_base,
    pack_ext_picture_blob_from_xml_and_bytes, pack_form_body_blob_from_form_xml,
    pack_form_body_blob_from_form_xml_base_free, pack_form_body_blob_from_form_xml_with_source,
    pack_form_body_blob_from_form_xml_with_source_and_assets, pack_form_body_blob_from_module_text,
    pack_help_blob_from_parts, pack_predefined_data_blob_from_xml, pack_role_rights_blob_from_xml,
    pack_role_rights_blob_from_xml_with_source, pack_schedule_blob_from_xml,
    pack_simple_metadata_blob_from_xml, pack_simple_metadata_blob_from_xml_with_source,
    pack_style_body_blob_from_xml, pack_style_body_blob_from_xml_with_base,
    parse_common_module_xml_properties, parse_ext_picture_file_name_from_xml, parse_form_body_blob,
    parse_help_pages_from_xml, parse_simple_metadata_xml_properties, parse_template_type_from_xml,
    patch_predefined_data_blob_from_xml, predefined_data_base_free_blockers,
    raw_deflated_help_content_sha256, raw_deflated_looks_like_help_blob,
};

// The rest of the root reaches these by their `module_blob` paths.
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "the root's tests read it; outside them its one reader, the exporter, names the crate"
    )
)]
pub(crate) use ibcmd_1c_text::decode_base64_mime_lenient as decode_base64_mime;
pub(crate) use ibcmd_1c_text::encode_base64;
// The module blob codec, the raw deflate helpers and digests, the
// raw-deflated packer and the versions-blob patcher are the
// `ibcmd-module-blob` crate's; the root reaches them by these paths.
#[expect(
    unused_imports,
    reason = "the root keeps the path; its one reader, the exporter, names the crate"
)]
pub(crate) use ibcmd_module_blob::unpack_plain_text_module_body;
pub use ibcmd_module_blob::{
    PackedModuleBlob, PackedRawDeflatedBlob, PatchedVersionsBlob, VersionReplacement, hex_sha256,
    module_blob_text_sha256, pack_module_blob_bytes, pack_module_blob_bytes_base_free,
    pack_module_blob_container_bytes, pack_raw_deflated_blob_from_bytes, patch_versions_blob_bytes,
    patch_versions_blob_bytes_allowing_additions, raw_deflated_first_base64_payload_sha256,
    raw_deflated_plain_sha256, unpack_module_blob_text, versions_base_free_blockers,
};
pub(crate) use ibcmd_module_blob::{deflate_raw, inflate_raw};

// A form body record and its container revision are the `ibcmd-form-body`
// crate's (the form reader there reads them); this module unpacks them and
// keeps their public paths.
pub use ibcmd_form_body::ParsedFormBodyBlob;

// The form body record (with the root-`42` upgrade it reads first), the
// physical layout of its Form Attributes tail and the braced-record text
// primitives both read moved to that crate too; the packers here patch
// the ranges they locate.
pub(crate) use ibcmd_form_body::parse_form_body_plain;

pub(crate) use ibcmd_form_body::record_text::scan_braced_fields;

pub use ibcmd_form_body::FormBodyRevision;

// The form compiler's dispatch among the 8.3.27 writer, its 2.21 route and
// the adopted form, and the form readiness audit, are the
// `ibcmd-form-writer` crate's; the public wrappers below coerce into them.
pub(crate) use ibcmd_form_writer::{
    compile_native_form_body_through, form_body_base_free_blockers_with_resolver,
    pack_native_form_body_blob_through,
};

mod help_pages;

pub use help_pages::{HtmlPageOwner, html_page_storage_bytes};

// The base-free writers of the command-interface family are
// `ibcmd-metadata-blob`'s; the tests that own their source trees through the
// root's test scratch stay here.
pub use ibcmd_metadata_blob::{
    InterfaceAssetSource, command_interface_base_free_blockers,
    command_interface_xml_can_pack_without_base, interface_asset_plaintext,
    pack_command_interface_blob_from_xml_base_free, pack_interface_asset_blob,
};

#[cfg(test)]
mod interface_assets_root_tests;

pub fn pack_module_blob(args: &ModuleBlobPackArgs) -> Result<ModuleBlobPackReport> {
    let text = fs::read(&args.text)
        .with_context(|| format!("failed to read BSL text {}", args.text.display()))?;
    let base_blob = match &args.base_blob {
        Some(path) => Some(
            fs::read(path)
                .with_context(|| format!("failed to read base blob {}", path.display()))?,
        ),
        None => None,
    };
    let info = match &args.info_file {
        Some(path) => Some(
            fs::read(path).with_context(|| format!("failed to read info {}", path.display()))?,
        ),
        None => None,
    };
    let packed = pack_module_blob_bytes(&text, base_blob.as_deref(), info.as_deref())?;

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&args.output, &packed.blob)
        .with_context(|| format!("failed to write {}", args.output.display()))?;

    Ok(ModuleBlobPackReport {
        text: args.text.clone(),
        output: args.output.clone(),
        base_blob: args.base_blob.clone(),
        info_bytes: packed.info_bytes,
        text_bytes: packed.text_bytes,
        inner_bytes: packed.inner_bytes,
        output_bytes: packed.blob.len(),
        output_sha256: packed.output_sha256,
    })
}

// The stored MOXCEL blob of a `Template.xml`: its writer's wrappers, beside
// the writer in `compiler::bodies::mxl_native`.
pub use crate::compiler::bodies::mxl_native::{
    pack_moxel_spreadsheet_blob_from_xml, pack_moxel_spreadsheet_blob_from_xml_with_source,
    pack_moxel_spreadsheet_blob_from_xml_with_source_and_hint,
};

pub fn compile_native_form_body(
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<String> {
    compile_native_form_body_through(
        form_xml,
        module_text,
        source.map(|source| source as &dyn MetadataSource),
        items_root,
    )
}

/// The stored blob of one Form.xml through the native writer: the body text
/// with the BOM the platform writes, raw-deflated. The loader's first choice;
/// a form the writer refuses falls back to the older paths.
pub fn pack_native_form_body_blob(
    form_xml: &[u8],
    module_text: Option<&[u8]>,
    source: Option<&MetadataSourceContext>,
    items_root: Option<&Path>,
) -> Result<PackedRawDeflatedBlob> {
    pack_native_form_body_blob_through(
        form_xml,
        module_text,
        source.map(|source| source as &dyn MetadataSource),
        items_root,
    )
}

pub fn form_body_base_free_blockers(
    form_xml: &[u8],
    has_module_text: bool,
    form_item_asset_files: usize,
) -> Result<Vec<String>> {
    form_body_base_free_blockers_with_resolver(
        form_xml,
        has_module_text,
        form_item_asset_files,
        true,
    )
}

pub(crate) fn form_body_base_free_compilation_blockers(
    form_xml: &[u8],
    has_module_text: bool,
    form_item_asset_files: usize,
    has_source_resolver: bool,
) -> Result<Vec<String>> {
    form_body_base_free_blockers_with_resolver(
        form_xml,
        has_module_text,
        form_item_asset_files,
        has_source_resolver,
    )
}

/// Compiles a role's `Rights.xml` into its stored rights row without a base
/// blob (see `compiler::bodies::role_rights_writer`), names looked up in the
/// source tree.
///
/// Fail-closed twice over: the writer refuses what it cannot place, and a
/// row it did write is read back through the exporter and must give the same
/// document again -- the objects in any order, everything inside them as the
/// XML has it. A row that would export differently is refused, never staged.
pub fn pack_role_rights_blob_base_free(
    xml: &[u8],
    source: &MetadataSourceContext,
) -> Result<PackedRawDeflatedBlob> {
    use crate::compiler::bodies::role_rights_writer::{parse_rights_xml, write_role_rights};

    let written = write_role_rights(xml, source.role_rights_source())
        .map_err(|refusal| anyhow!("Role rights writer refused: {refusal}"))?;
    let blob = deflate_raw(&written.plain)?;
    let exported = crate::mssql_dump::role_rights_xml_from_blob(
        &blob,
        &written.object_refs,
        &written.field_refs,
    )
    .ok_or_else(|| anyhow!("Role rights writer: the written row does not read back"))?;
    let mut original = parse_rights_xml(xml)
        .map_err(|refusal| anyhow!("Role rights writer refused: {refusal}"))?;
    let mut reread = parse_rights_xml(exported.as_bytes()).map_err(|refusal| {
        anyhow!("Role rights writer: the read-back XML is unreadable: {refusal}")
    })?;
    original
        .objects
        .sort_by(|left, right| left.name.cmp(&right.name));
    reread
        .objects
        .sort_by(|left, right| left.name.cmp(&right.name));
    if original != reread {
        return Err(anyhow!(
            "Role rights writer: the written row reads back into a different Rights.xml"
        ));
    }
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        plain_bytes: written.plain.len(),
        blob,
        output_sha256,
    })
}

/// Why a role's `Rights.xml` still needs the active base row: nothing when
/// the base-free writer compiles it from `source`, else the writer's refusal
/// followed by what the base patcher preserves from the base row.
pub fn role_rights_base_free_blockers(
    xml: &[u8],
    source: Option<&MetadataSourceContext>,
) -> Result<Vec<String>> {
    let writer_refusal = match source {
        Some(source) => match pack_role_rights_blob_base_free(xml, source) {
            Ok(_) => return Ok(Vec::new()),
            Err(error) => Some(format!("{error:#}")),
        },
        None => None,
    };
    let rights = parse_role_rights_xml(xml)?;
    let conditional_rights = rights
        .objects
        .iter()
        .flat_map(|object| object.rights.iter())
        .filter(|right| right.restriction_by_condition.is_some())
        .count();
    let enabled_rights = rights
        .objects
        .iter()
        .flat_map(|object| object.rights.iter())
        .filter(|right| right.value)
        .count();
    let total_rights = rights
        .objects
        .iter()
        .map(|object| object.rights.len())
        .sum::<usize>();
    let mut blockers = Vec::new();
    if let Some(refusal) = writer_refusal {
        blockers.push(refusal);
    }
    blockers.push(format!(
        "source XML has {} object entries but does not carry the base Role object table order or per-object entry identifiers",
        rights.objects.len()
    ));
    blockers.push(format!(
        "source XML has {total_rights} explicit rights ({enabled_rights} enabled) by name, while the Config row stores platform right UUID slots whose order and complete disabled-right set are read from the base blob"
    ));
    if conditional_rights > 0 {
        blockers.push(format!(
            "source XML has {conditional_rights} conditional rights, but the base blob owns the restricted-right payload shape and condition slot mapping"
        ));
    }
    if !rights.restriction_templates.is_empty() {
        blockers.push(format!(
            "source XML has {} restriction templates, but staging currently patches the existing base template table instead of generating its full serialized shape",
            rights.restriction_templates.len()
        ));
    }
    if rights.set_for_attributes_by_default.is_some()
        || rights.independent_rights_of_child_objects.is_some()
    {
        blockers.push(
            "source XML may set optional role flags, but their presence and trailing field positions are preserved from the base blob".to_string(),
        );
    }
    Ok(blockers)
}

pub fn pack_command_interface_blob_from_xml(
    base_blob: &[u8],
    xml: &[u8],
) -> Result<PackedRawDeflatedBlob> {
    if base_blob.is_empty() {
        return pack_command_interface_blob_from_xml_base_free(xml);
    }
    let entries = parse_command_interface_xml(xml)?;
    let inflated =
        inflate_raw(base_blob).context("failed to inflate base CommandInterface blob")?;
    let mut plain =
        String::from_utf8(inflated).context("base CommandInterface blob is not valid UTF-8")?;
    let body_start = plain
        .find('{')
        .ok_or_else(|| anyhow!("base CommandInterface body has no braced payload"))?;
    let fields = scan_braced_fields(&plain, body_start)?;
    if fields.first().map(|range| plain[range.clone()].trim()) != Some("7") {
        return Err(anyhow!(
            "base CommandInterface body does not start with type marker 7"
        ));
    }
    let count_range = fields
        .get(2)
        .ok_or_else(|| anyhow!("base CommandInterface body has no command count"))?;
    let count = plain[count_range.clone()]
        .trim()
        .parse::<usize>()
        .context("invalid base CommandInterface command count")?;
    if count != entries.len() {
        return Err(anyhow!(
            "CommandInterface.xml command count {} does not match base blob command count {}",
            entries.len(),
            count
        ));
    }
    let required_fields = 3 + count * 2;
    if fields.len() < required_fields {
        return Err(anyhow!(
            "base CommandInterface body has {} fields, expected at least {}",
            fields.len(),
            required_fields
        ));
    }

    let mut replacements = Vec::with_capacity(count);
    for (index, entry) in entries.iter().enumerate() {
        let common_range = fields[3 + index * 2 + 1].clone();
        let common = if entry.common { "1" } else { "0" };
        replacements.push((common_range, format!("{{0,{{0,{{\"B\",{common}}},0}}}}")));
    }
    replacements.sort_by_key(|b| std::cmp::Reverse(b.0.start));
    for (range, replacement) in replacements {
        replace_1c_value_if_different(&mut plain, range, &replacement);
    }
    let blob = deflate_raw(plain.as_bytes())?;
    let output_sha256 = hex_sha256(&blob);
    Ok(PackedRawDeflatedBlob {
        blob,
        plain_bytes: plain.len(),
        output_sha256,
    })
}

pub fn patch_versions_blob(args: &VersionsBlobPatchArgs) -> Result<VersionsBlobPatchReport> {
    let input = fs::read(&args.input)
        .with_context(|| format!("failed to read versions blob {}", args.input.display()))?;
    let patched = patch_versions_blob_bytes(&input, &args.changes, !args.no_standard_entries)?;

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(&args.output, &patched.blob)
        .with_context(|| format!("failed to write {}", args.output.display()))?;

    Ok(VersionsBlobPatchReport {
        input: args.input.clone(),
        output: args.output.clone(),
        plain_bytes: patched.plain_bytes,
        output_bytes: patched.blob.len(),
        output_sha256: patched.output_sha256,
        replacements: patched.replacements,
    })
}

#[cfg(test)]
mod tests {
    use super::{MetadataSource, MetadataSourceContext, deflate_raw, inflate_raw};

    /// One `Form.xml` carrying every part of an attribute this change reads:
    /// a value table with three columns, a value list with an element type,
    /// and a date parameter. The strings are the shapes ERP УХ bodies store.
    #[test]
    fn writes_the_columns_and_the_value_list_element_type() -> anyhow::Result<()> {
        let xml = concat!(
            r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" "#,
            r#"xmlns:v8="http://v8.1c.ru/8.1/data/core" "#,
            r#"xmlns:xs="http://www.w3.org/2001/XMLSchema" "#,
            r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">"#,
            r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#,
            r#"<Attributes><Attribute name="Таблица" id="1">"#,
            r#"<Title><item><lang>ru</lang><content>Таблица</content></item></Title>"#,
            r#"<Type><v8:Type>v8:ValueTable</v8:Type></Type><Columns>"#,
            r#"<Column name="Дата" id="2">"#,
            r#"<Title><item><lang>ru</lang><content>Дата</content></item></Title>"#,
            r#"<Type><v8:Type>xs:dateTime</v8:Type>"#,
            r#"<v8:DateQualifiers><v8:DateFractions>Date</v8:DateFractions>"#,
            r#"</v8:DateQualifiers></Type></Column>"#,
            r#"<Column name="Пусто" id="3"><Type/></Column>"#,
            r#"<Column name="Строка" id="4"><Type><v8:Type>xs:string</v8:Type>"#,
            r#"<v8:StringQualifiers><v8:Length>50</v8:Length>"#,
            r#"<v8:AllowedLength>Variable</v8:AllowedLength></v8:StringQualifiers></Type>"#,
            r#"<FillCheck>ShowError</FillCheck></Column>"#,
            r#"</Columns></Attribute>"#,
            r#"<Attribute name="Список" id="5">"#,
            r#"<Type><v8:Type>v8:ValueListType</v8:Type></Type>"#,
            r#"<Settings xsi:type="v8:TypeDescription"><v8:Type>xs:string</v8:Type>"#,
            r#"<v8:StringQualifiers><v8:Length>50</v8:Length>"#,
            r#"<v8:AllowedLength>Fixed</v8:AllowedLength></v8:StringQualifiers></Settings>"#,
            r#"</Attribute></Attributes>"#,
            r#"<Parameters><Parameter name="Период"><Type><v8:Type>xs:dateTime</v8:Type>"#,
            r#"<v8:DateQualifiers><v8:DateFractions>Time</v8:DateFractions>"#,
            r#"</v8:DateQualifiers></Type></Parameter></Parameters>"#,
            "</Form>",
        );
        let body = super::compile_native_form_body(xml.as_bytes(), None, None, None)?;

        // Member 13 counts the columns and members 14.. are their records;
        // an empty `<Type/>` is `{"Pattern"}`, `DateFractions` is the second
        // member of the `{"D"}` element, `ShowError` is the only spelling
        // that makes a column's last member 1, and a `Variable` length is a
        // 1 in the string element -- the element type below is `Fixed` and
        // writes the 0.
        assert!(
            body.contains(concat!(
                r#"{9,{1},0,"Таблица",{1,1,{"ru","Таблица"}},"#,
                r##"{"Pattern",{"#",acf6192e-81ca-46ef-93a6-5a6968b78663}},"##,
                r#"{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,3,"#,
                r#"{5,2,0,"Дата",{1,1,{"ru","Дата"}},{"Pattern",{"D","D"}},"#,
                r#"{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},0},"#,
                r#"{5,3,0,"Пусто",{1,0},{"Pattern"},"#,
                r#"{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},0},"#,
                r#"{5,4,0,"Строка",{1,0},{"Pattern",{"S",50,1}},"#,
                r#"{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},1},"#,
                r#"{0,0},{0,0}}"#,
            )),
            "the value table's record is not the one the platform stores: {body}"
        );

        // A value list's `<Settings>` is its element type, not a dynamic
        // list's settings, and it is written into the first trailing block.
        assert!(
            body.contains(concat!(
                r#"{9,{5},0,"Список",{1,0},"#,
                r##"{"Pattern",{"#",4772b3b4-f4a3-49c0-a1a5-8cb5961511a3}},"##,
                r#"{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,"#,
                r##"{0,1,"ElementType",{"#",f5c65050-3bbb-11d5-b988-0050bae0a95d,"##,
                r#"{"Pattern",{"S",50,0}}}},{0,0}}"#,
            )),
            "the value list's element type is not stored: {body}"
        );

        assert!(
            body.contains(r#"{0,"Период",{"Pattern",{"D","T"}},0}"#),
            "the parameter's date fractions are not stored: {body}"
        );
        Ok(())
    }

    /// The head of a `Form.xml` the tests below build on.
    const FORM_XML_HEAD: &str = concat!(
        r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" "#,
        r#"xmlns:v8="http://v8.1c.ru/8.1/data/core" "#,
        r#"xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" "#,
        r#"xmlns:xs="http://www.w3.org/2001/XMLSchema" "#,
        r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">"#,
    );

    #[test]
    fn ordinary_table_additions_without_source_serve_the_enclosing_table() -> anyhow::Result<()> {
        use crate::metadata_model::brace::{self, Brace};
        let mut xml = format!(
            "{FORM_XML_HEAD}<AutoCommandBar name=\"FormBar\" id=\"-1\"/><ChildItems><Table name=\"Rows\" id=\"25\"><AutoCommandBar name=\"Bar\" id=\"26\"/>"
        );
        for (index, tag) in [
            "SearchStringAddition",
            "ViewStatusAddition",
            "SearchControlAddition",
        ]
        .iter()
        .enumerate()
        {
            let id = index * 3 + 30;
            xml.push_str(&format!("<{tag} name=\"Addition{index}\" id=\"{id}\"><ContextMenu name=\"Menu{index}\" id=\"{}\"/><ExtendedTooltip name=\"Tip{index}\" id=\"{}\"/></{tag}>", id + 1, id + 2));
        }
        xml.push_str("<ContextMenu name=\"TableMenu\" id=\"50\"/><ExtendedTooltip name=\"TableTip\" id=\"51\"/></Table></ChildItems></Form>");
        let body = super::compile_native_form_body(xml.as_bytes(), None, None, None)?;
        let tree = brace::parse_row(body.as_bytes())?;
        fn collect(node: &Brace, sources: &mut Vec<Brace>) {
            if let Some(items) = node.as_list() {
                if items
                    .get(6)
                    .and_then(Brace::as_str)
                    .is_some_and(|name| name.starts_with("Addition"))
                {
                    sources.push(items[19].clone());
                }
                for child in items {
                    collect(child, sources);
                }
            }
        }
        let mut sources = Vec::new();
        collect(&tree, &mut sources);
        assert_eq!(
            sources,
            vec![
                Brace::list(vec![Brace::num(25), Brace::num(0)]),
                Brace::list(vec![Brace::num(25), Brace::num(1)]),
                Brace::list(vec![Brace::num(25), Brace::num(2)]),
            ]
        );
        Ok(())
    }

    /// `<MobileDeviceCommandBarContent>` reaches root trailer slot 22, and
    /// what it stores there is the id of the form **item** its `<xr:Value>`
    /// names.
    ///
    /// Pinned to `BusinessProcesses/_ДемоЗаданиеСРолевойАдресацией/Forms/
    /// ФормаСписка` of the BSP tree, whose body holds `{50,1,"",{"N",46}}`
    /// for the `<CommandBar name="КоманднаяПанель" id="46">` it names. The
    /// second form here adds a `<Command>` of that same name: in all ten
    /// forms of the two corpora where the two collide the item's id is what
    /// the body holds, never the command's.
    #[test]
    fn writes_the_mobile_device_command_bar_content_from_the_item_it_names() -> anyhow::Result<()> {
        let form = |extra_command: &str, content: &str| {
            format!(
                concat!(
                    "{head}",
                    "{content}",
                    r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#,
                    "{extra_command}",
                    r#"<ChildItems><CommandBar name="КоманднаяПанель" id="46"/>"#,
                    r#"</ChildItems></Form>"#,
                ),
                head = FORM_XML_HEAD,
                content = content,
                extra_command = extra_command,
            )
        };
        let named = form(
            "",
            concat!(
                "<MobileDeviceCommandBarContent><xr:Item>",
                "<xr:Presentation/><xr:CheckState>0</xr:CheckState>",
                r#"<xr:Value xsi:type="xs:string">КоманднаяПанель</xr:Value>"#,
                "</xr:Item></MobileDeviceCommandBarContent>",
            ),
        );
        let body = super::compile_native_form_body(named.as_bytes(), None, None, None)?;
        assert!(
            body.contains(r#"{50,1,"",{"N",46}}"#),
            "the mobile command bar does not name the item's id: {body}"
        );

        // The item wins over a `<Command>` of the same name.
        let clashing = form(
            r#"<Commands><Command name="КоманднаяПанель" id="7"/></Commands>"#,
            concat!(
                "<MobileDeviceCommandBarContent><xr:Item>",
                "<xr:Presentation/><xr:CheckState>0</xr:CheckState>",
                r#"<xr:Value xsi:type="xs:string">КоманднаяПанель</xr:Value>"#,
                "</xr:Item></MobileDeviceCommandBarContent>",
            ),
        );
        let body = super::compile_native_form_body(clashing.as_bytes(), None, None, None)?;
        assert!(
            body.contains(r#"{50,1,"",{"N",46}}"#),
            "the command's id was written instead of the item's: {body}"
        );

        // An empty `<xr:Value/>` is `{"N",0}` -- 6 items of the two corpora,
        // among them `SettingsStorages/ХранилищеВариантовОтчетов/Forms/
        // СохранениеВариантаОтчета`, which stores three of them in a row.
        let empty = form(
            "",
            concat!(
                "<MobileDeviceCommandBarContent><xr:Item>",
                "<xr:Presentation/><xr:CheckState>0</xr:CheckState>",
                r#"<xr:Value xsi:type="xs:string"/>"#,
                "</xr:Item></MobileDeviceCommandBarContent>",
            ),
        );
        let body = super::compile_native_form_body(empty.as_bytes(), None, None, None)?;
        assert!(
            body.contains(r#"{50,1,"",{"N",0}}"#),
            "an empty value is not zero: {body}"
        );

        // Absent is the literal the other 13 443 forms store.
        let absent = form("", "");
        let body = super::compile_native_form_body(absent.as_bytes(), None, None, None)?;
        assert!(
            body.contains("{50,0}"),
            "a form with no mobile command bar lost its literal: {body}"
        );

        // A name no item of the form answers refuses rather than writing 0.
        let unknown = form(
            "",
            concat!(
                "<MobileDeviceCommandBarContent><xr:Item>",
                "<xr:Presentation/><xr:CheckState>0</xr:CheckState>",
                r#"<xr:Value xsi:type="xs:string">НетТакого</xr:Value>"#,
                "</xr:Item></MobileDeviceCommandBarContent>",
            ),
        );
        let error = super::compile_native_form_body(unknown.as_bytes(), None, None, None)
            .expect_err("a name no item answers must refuse the form")
            .to_string();
        assert!(
            error.contains("which is not a form item"),
            "the unknown name was written instead of refused: {error}"
        );
        Ok(())
    }

    /// A radio button's `<ChoiceList>` is slot 1 of its `{8,…}` payload, and
    /// its `<Font>` is slot 4 -- two slots and two conditions, where the
    /// writer used to refuse on either.
    ///
    /// Pinned to `Catalogs/_ДемоБанковскиеСчета/Forms/ФормаЭлемента` of the
    /// BSP tree, whose body holds this payload whole for `МестоОткрытия`.
    #[test]
    fn writes_a_radio_buttons_choice_list_of_string_literals() -> anyhow::Result<()> {
        let xml = format!(
            concat!(
                "{head}",
                r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#,
                r#"<Attributes><Attribute name="МестоОткрытия" id="1">"#,
                r#"<Type><v8:Type>xs:string</v8:Type></Type></Attribute></Attributes>"#,
                r#"<ChildItems><RadioButtonField name="МестоОткрытия" id="167">"#,
                "<DataPath>МестоОткрытия</DataPath>",
                "<RadioButtonType>Auto</RadioButtonType><ChoiceList>",
                "<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState>",
                r#"<xr:Value xsi:type="FormChoiceListDesTimeValue"><Presentation>"#,
                "<v8:item><v8:lang>ru</v8:lang>",
                "<v8:content>в Российской Федерации</v8:content></v8:item></Presentation>",
                r#"<Value xsi:type="xs:string">РФ</Value></xr:Value></xr:Item>"#,
                "<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState>",
                r#"<xr:Value xsi:type="FormChoiceListDesTimeValue"><Presentation>"#,
                "<v8:item><v8:lang>ru</v8:lang>",
                "<v8:content>за рубежом</v8:content></v8:item></Presentation>",
                r#"<Value xsi:type="xs:string">ЗаРубежом</Value></xr:Value></xr:Item>"#,
                "</ChoiceList>",
                r#"<ContextMenu name="МестоОткрытияКонтекстноеМеню" id="168"/>"#,
                r#"<ExtendedTooltip name="МестоОткрытияРасширеннаяПодсказка" id="169"/>"#,
                "</RadioButtonField></ChildItems></Form>",
            ),
            head = FORM_XML_HEAD,
        );
        let body = super::compile_native_form_body(xml.as_bytes(), None, None, None)?;
        assert!(
            body.contains(concat!(
                r##"{8,{3,2,"",{"#",0e704aa2-07bd-48b9-8223-a0212c4d5fc2,"##,
                r##"{0,1,{"S","РФ"},00000000-0000-0000-0000-000000000000,"##,
                r##"00000000-0000-0000-0000-000000000000,"##,
                r##"{1,1,{"ru","в Российской Федерации"}}}},"##,
                r##""",{"#",0e704aa2-07bd-48b9-8223-a0212c4d5fc2,"##,
                r##"{0,1,{"S","ЗаРубежом"},00000000-0000-0000-0000-000000000000,"##,
                r##"00000000-0000-0000-0000-000000000000,{1,1,{"ru","за рубежом"}}}},"##,
                r##"{0,{4,0,{0},"",-1,-1,1,0,""}},{0,{4,0,{0},"",-1,-1,1,0,""}}},"##,
                r##"0,{3,4,{0}},{7,3,0,1,100},{3,4,{0}},0,0,{3,4,{0}},0,0,2}"##,
            )),
            "the radio button's payload is not the one the platform stores: {body}"
        );

        // A platform style font carries a negative code of its own, from a
        // table that is not the colours'; `LargeTextFont` is -32 in 177 of
        // the corpus's elements. An `Absolute` font is still refused: its
        // nineteen-member shape is not decided by the source.
        let with_font = xml.replace(
            "<RadioButtonType>Auto</RadioButtonType>",
            r#"<RadioButtonType>Auto</RadioButtonType><Font ref="style:LargeTextFont" kind="StyleItem"/>"#,
        );
        let body = super::compile_native_form_body(with_font.as_bytes(), None, None, None)?;
        assert!(
            body.contains("{7,2,0,{-32},1,100}"),
            "the platform style font is not the one the platform stores: {body}"
        );
        let absolute = xml.replace(
            "<RadioButtonType>Auto</RadioButtonType>",
            r#"<RadioButtonType>Auto</RadioButtonType><Font faceName="Arial" height="8" kind="Absolute"/>"#,
        );
        let error = super::compile_native_form_body(absolute.as_bytes(), None, None, None)
            .expect_err("an Absolute font must refuse the form")
            .to_string();
        assert!(
            error.contains("names a font the writer cannot place"),
            "the Absolute font was written instead of refused: {error}"
        );
        Ok(())
    }

    /// A `<ChoiceList>` item whose value is a design-time reference stores
    /// `{"U"}` with the owner's Ref `<xr:TypeId>` and the referenced value's
    /// own id beside it.
    ///
    /// Pinned to `Catalogs/Валюты/Forms/ФормаЭлемента` of the BSP tree, whose
    /// body holds this value for `СпособУстановкиКурса`; the two uuids are
    /// `Enums/СпособыУстановкиКурсаВалюты.xml`'s `EnumRef` type id and the
    /// `<EnumValue uuid>` of `РучнойВвод`.
    #[test]
    fn writes_a_choice_list_reference_from_the_enum_it_names() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-choice-list-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Enums"))?;
        std::fs::write(
            root.join("Enums/СпособыУстановкиКурсаВалюты.xml"),
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" "#,
                r#"xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">"#,
                r#"<Enum uuid="ec11342f-594f-416f-a10f-bd1bfa695c08"><InternalInfo>"#,
                r#"<xr:GeneratedType name="EnumRef.СпособыУстановкиКурсаВалюты" category="Ref">"#,
                "<xr:TypeId>a01d58fd-ca5c-43b1-ad9c-8e39ce91beba</xr:TypeId>",
                "</xr:GeneratedType></InternalInfo><Properties>",
                "<Name>СпособыУстановкиКурсаВалюты</Name></Properties><ChildObjects>",
                r#"<EnumValue uuid="74f03c35-5da0-406e-a938-e8e5d49975ef"><Properties>"#,
                "<Name>РучнойВвод</Name></Properties></EnumValue>",
                "</ChildObjects></Enum></MetaDataObject>",
            )
            .as_bytes(),
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let xml = format!(
            concat!(
                "{head}",
                r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#,
                r#"<Attributes><Attribute name="Способ" id="1">"#,
                r#"<Type><v8:Type>xs:string</v8:Type></Type></Attribute></Attributes>"#,
                r#"<ChildItems><RadioButtonField name="Способ" id="2">"#,
                "<DataPath>Способ</DataPath><ChoiceList>",
                "<xr:Item><xr:Presentation/><xr:CheckState>0</xr:CheckState>",
                r#"<xr:Value xsi:type="FormChoiceListDesTimeValue"><Presentation>"#,
                "<v8:item><v8:lang>ru</v8:lang>",
                "<v8:content>вводится вручную</v8:content></v8:item></Presentation>",
                r#"<Value xsi:type="xr:DesignTimeRef">"#,
                "Enum.СпособыУстановкиКурсаВалюты.EnumValue.РучнойВвод",
                "</Value></xr:Value></xr:Item></ChoiceList>",
                r#"<ContextMenu name="СпособКонтекстноеМеню" id="3"/>"#,
                r#"<ExtendedTooltip name="СпособРасширеннаяПодсказка" id="4"/>"#,
                "</RadioButtonField></ChildItems></Form>",
            ),
            head = FORM_XML_HEAD,
        );
        let body = super::compile_native_form_body(xml.as_bytes(), None, Some(&source), None)?;
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            body.contains(concat!(
                r##"{"#",0e704aa2-07bd-48b9-8223-a0212c4d5fc2,{0,0,{"U"},"##,
                "a01d58fd-ca5c-43b1-ad9c-8e39ce91beba,",
                r#"74f03c35-5da0-406e-a938-e8e5d49975ef,{1,1,{"ru","вводится вручную"}}}}"#,
            )),
            "the reference's two uuids are not the ones the platform stores: {body}"
        );
        Ok(())
    }

    /// The payload at member 18 of a decoration record, end to end from the
    /// XML: nine members beginning with `5` for a label and thirteen beginning
    /// with `4` for a picture.
    ///
    /// Both payloads are pinned to bodies of the ERP УХ tree:
    /// `Catalogs/ПоказателиМонитораКлючевыхПоказателей/Forms/ФормаЭлемента`
    /// item 246 for the `#C0DCC0` background and the `Single` border,
    /// `BusinessProcesses/ТиповаяПродажа/Forms/Взаимодействия` item 183 for the
    /// hyperlink and the `Click`, `Catalogs/НастройкиОнлайнОплат/Forms/
    /// ФормаЭлемента` item 280 for `<VerticalAlign>` and `<TitleHeight>`, and
    /// `Catalogs/БланкиОтчетов/Forms/МастерСозданияНовыхСтрок` item 85 for the
    /// common picture, `<PictureSize>Stretch</>` and `<FileDragMode>AsFile</>`.
    ///
    /// The writer used to give every decoration one constant, which is the
    /// right answer for neither of these.
    #[test]
    fn writes_the_decoration_payload_the_platform_stores() -> anyhow::Result<()> {
        const HEAD: &str = concat!(
            r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" "#,
            r#"xmlns:v8="http://v8.1c.ru/8.1/data/core" "#,
            r#"xmlns:v8ui="http://v8.1c.ru/8.1/data/ui" "#,
            r#"xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" "#,
            r#"xmlns:xs="http://www.w3.org/2001/XMLSchema" "#,
            r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">"#,
        );
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-decoration-payload-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("CommonPictures"))?;
        std::fs::write(
            root.join("CommonPictures").join("СтрелкаВниз.xml"),
            r#"<MetaDataObject><CommonPicture uuid="ce463861-92df-47f2-bd0f-4a2dbf72aacc">
<Properties><Name>СтрелкаВниз</Name></Properties></CommonPicture></MetaDataObject>"#
                .as_bytes(),
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let form = |picture_ref: &str| {
            format!(
                concat!(
                    "{head}",
                    r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/><ChildItems>"#,
                    r#"<LabelDecoration name="Надпись" id="2">"#,
                    "<BackColor>#C0DCC0</BackColor>",
                    r#"<Border width="1"><v8ui:style xsi:type="v8ui:ControlBorderType">"#,
                    "Single</v8ui:style></Border>",
                    "<Hyperlink>true</Hyperlink><HorizontalAlign>Right</HorizontalAlign>",
                    "<VerticalAlign>Center</VerticalAlign><TitleHeight>1</TitleHeight>",
                    r#"<ContextMenu name="НадписьКонтекстноеМеню" id="3"/>"#,
                    r#"<ExtendedTooltip name="НадписьРасширеннаяПодсказка" id="4"/>"#,
                    r#"<Events><Event name="Click">ДекорацияНажатие</Event></Events>"#,
                    "</LabelDecoration>",
                    r#"<PictureDecoration name="Картинка" id="5">"#,
                    "<Picture><xr:Ref>{picture_ref}</xr:Ref>",
                    "<xr:LoadTransparent>false</xr:LoadTransparent></Picture>",
                    "<PictureSize>Stretch</PictureSize><FileDragMode>AsFile</FileDragMode>",
                    r#"<ContextMenu name="КартинкаКонтекстноеМеню" id="6"/>"#,
                    r#"<ExtendedTooltip name="КартинкаРасширеннаяПодсказка" id="7"/>"#,
                    "</PictureDecoration></ChildItems></Form>",
                ),
                head = HEAD,
                picture_ref = picture_ref,
            )
        };

        let body = super::compile_native_form_body(
            form("CommonPicture.СтрелкаВниз").as_bytes(),
            None,
            Some(&source),
            None,
        )?;
        assert!(
            body.contains(concat!(
                "{5,1,2,1,1,{1,11707a99-4eb9-4373-bc8c-84891483a034,\"ДекорацияНажатие\",1,0,",
                "11707a99-4eb9-4373-bc8c-84891483a034,0,1},{3,0,{12639424}},{3,4,{0}},",
                "{3,0,{0},1,1,0,48312c09-257f-4b29-b280-284dd89efc1e}}",
            )),
            "the label decoration payload is not the one the platform stores: {body}"
        );
        assert!(
            body.contains(concat!(
                "{4,{4,1,{0,ce463861-92df-47f2-bd0f-4a2dbf72aacc},\"\",-1,-1,0,0,\"\"},0,1,0,",
                "{1,0},{3,4,{0}},{3,0,{0},0,1,0,48312c09-257f-4b29-b280-284dd89efc1e},0,0,",
                "{0,1,0},0,100}",
            )),
            "the picture decoration payload is not the one the platform stores: {body}"
        );

        // A `StdPicture` names the platform's own picture, which no file of
        // the source declares. Its value comes from a table measured over both
        // corpora; member 6 is `<xr:LoadTransparent>`, which this helper spells
        // `false` -- 1 is what the usual `true` stores.
        let body = super::compile_native_form_body(
            form("StdPicture.Information").as_bytes(),
            None,
            Some(&source),
            None,
        )?;
        assert!(
            body.contains("{4,1,{0,4b54770b-d069-4c0e-9b17-5cc2a01134d9},\"\",-1,-1,0,0,\"\"}"),
            "the StdPicture reference is not the one the platform stores: {body}"
        );

        // A name the table does not know is still refused.
        let error = super::compile_native_form_body(
            form("StdPicture.NoSuchPictureInAnyCorpus").as_bytes(),
            None,
            Some(&source),
            None,
        )
        .expect_err("an unmeasured StdPicture name must refuse the form");
        let error = format!("{error:#}");
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            error.contains("no measured value for StdPicture."),
            "an unmeasured StdPicture name was written instead of refused: {error}"
        );
        Ok(())
    }

    /// An attribute's `<Save>` is member 9 of its `{9,…}` record, and the
    /// paths it lists are **sorted**, not in XML order.
    ///
    /// Pinned to `DataProcessors/ГрупповоеИзменениеРеквизитов/Forms/
    /// РедактированиеФормулы` of the BSP tree. Its `<Save>` spells
    /// `ИзменятьВТранзакции` first and `ПрерыватьПриОшибке` second, and the
    /// body stores their uuids the other way round -- pairing the nth
    /// `<Field>` with the nth entry agrees on the set and swaps the uuids.
    #[test]
    fn writes_an_attributes_save_with_its_paths_sorted() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-attribute-save-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("DataProcessors"))?;
        std::fs::write(
            root.join("DataProcessors/ГрупповоеИзменениеРеквизитов.xml"),
            concat!(
                r#"<?xml version="1.0" encoding="UTF-8"?>"#,
                r#"<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" "#,
                r#"xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">"#,
                r#"<DataProcessor uuid="e1b94dcf-ec37-4b2d-a7af-120eeee40944"><InternalInfo>"#,
                r#"<xr:GeneratedType name="DataProcessorObject.ГрупповоеИзменениеРеквизитов" "#,
                r#"category="Object">"#,
                "<xr:TypeId>f06540d8-044b-4f8c-99c2-22070273fd43</xr:TypeId>",
                "</xr:GeneratedType></InternalInfo><Properties>",
                "<Name>ГрупповоеИзменениеРеквизитов</Name></Properties><ChildObjects>",
                r#"<Attribute uuid="7d2a37b1-a1a0-435a-846a-95e255655044"><Properties>"#,
                "<Name>ИзменятьВТранзакции</Name></Properties></Attribute>",
                r#"<Attribute uuid="109db64c-00dc-490e-99a5-4430d9344e99"><Properties>"#,
                "<Name>ПрерыватьПриОшибке</Name></Properties></Attribute>",
                "</ChildObjects></DataProcessor></MetaDataObject>",
            )
            .as_bytes(),
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let xml = format!(
            concat!(
                "{head}",
                r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#,
                r#"<Attributes><Attribute name="Объект" id="18"><Type><v8:Type>"#,
                "cfg:DataProcessorObject.ГрупповоеИзменениеРеквизитов</v8:Type></Type>",
                "<MainAttribute>true</MainAttribute><Save>",
                "<Field>Объект.ИзменятьВТранзакции</Field>",
                "<Field>Объект.ПрерыватьПриОшибке</Field>",
                "</Save></Attribute></Attributes></Form>",
            ),
            head = FORM_XML_HEAD,
        );
        let body = super::compile_native_form_body(xml.as_bytes(), None, Some(&source), None)?;
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            body.contains(concat!(
                "{0,2,{1,{0,109db64c-00dc-490e-99a5-4430d9344e99}},",
                "{1,{0,7d2a37b1-a1a0-435a-846a-95e255655044}}}",
            )),
            "the <Save> paths are not sorted the way the body stores them: {body}"
        );

        // A `<Field>` that is the attribute's own name is `{0}`, which is
        // every one of the 303 BSP entries but eight forms' worth.
        let own = format!(
            concat!(
                "{head}",
                r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#,
                r#"<Attributes><Attribute name="СтрокаПоиска" id="1">"#,
                "<Type><v8:Type>xs:string</v8:Type></Type>",
                "<Save><Field>СтрокаПоиска</Field></Save>",
                "</Attribute></Attributes></Form>",
            ),
            head = FORM_XML_HEAD,
        );
        let body = super::compile_native_form_body(own.as_bytes(), None, None, None)?;
        assert!(
            body.contains("{0,1,{0}}"),
            "a <Save> of the attribute's own name is not {{0}}: {body}"
        );

        // An attribute that names no `<Save>` keeps the `{0,0}` that 138 968
        // of the 141 723 attributes of both corpora store.
        let absent = own.replace("<Save><Field>СтрокаПоиска</Field></Save>", "");
        let body = super::compile_native_form_body(absent.as_bytes(), None, None, None)?;
        assert!(
            body.contains(r#"{9,{1},0,"СтрокаПоиска",{1,0},{"Pattern",{"S"}},{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,{0,0},{0,0}}"#),
            "an attribute with no <Save> lost its defaults: {body}"
        );
        Ok(())
    }

    /// Fail-closed: a column's `<View>` and `<Edit>` land in members the
    /// measurement does not give, so naming one refuses the form rather than
    /// writing the default the other 142 594 store. `<FunctionalOptions>` has
    /// left this list -- member 8 is measured now -- and refuses for the one
    /// reason left, which the test below pins.
    #[test]
    fn writes_a_column_view_and_edit_as_the_rights_tuple() {
        // `<View>` and `<Edit>` are members 6 and 7 of the column record, the
        // tuple an attribute's own rights take (rt-embedded.md §4.5, 19 of 19).
        for (part, body) in [
            ("View", "<View><Common>false</Common></View>"),
            ("Edit", "<Edit><Common>false</Common></Edit>"),
        ] {
            let xml = format!(
                concat!(
                    r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" "#,
                    r#"xmlns:v8="http://v8.1c.ru/8.1/data/core" "#,
                    r#"xmlns:xs="http://www.w3.org/2001/XMLSchema" "#,
                    r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">"#,
                    r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#,
                    r#"<Attributes><Attribute name="Таблица" id="1">"#,
                    r#"<Type><v8:Type>v8:ValueTable</v8:Type></Type><Columns>"#,
                    r#"<Column name="Графа" id="2"><Type/>{part_body}</Column>"#,
                    r#"</Columns></Attribute></Attributes></Form>"#,
                ),
                part_body = body,
            );
            let body = super::compile_native_form_body(xml.as_bytes(), None, None, None)
                .unwrap_or_else(|error| {
                    panic!("a column that names {part} must be written: {error}")
                });
            let tuple = if part == "View" {
                "{0,{0,{\"B\",0},0}},{0,{0,{\"B\",1},0}}"
            } else {
                "{0,{0,{\"B\",1},0}},{0,{0,{\"B\",0},0}}"
            };
            assert!(
                body.contains(tuple),
                "{part} was not written as its rights: {body}"
            );
        }
    }

    /// `<FunctionalOptions>` is one `{0,N,<option uuid>×N}` collection, and
    /// the uuid is only in the source tree. Without one the holder refuses
    /// rather than write the `{0,0}` an absent element takes -- which is what
    /// 132 013 attributes, 59 320 commands and 142 994 columns store, and no
    /// record that spells the element ever does.
    #[test]
    fn refuses_functional_options_it_cannot_resolve() {
        for holder in [
            concat!(
                r#"<Attributes><Attribute name="Реквизит" id="1"><Type/>"#,
                r#"<FunctionalOptions><Item>FunctionalOption.Опция</Item></FunctionalOptions>"#,
                r#"</Attribute></Attributes>"#,
            ),
            concat!(
                r#"<Attributes><Attribute name="Таблица" id="1">"#,
                r#"<Type><v8:Type>v8:ValueTable</v8:Type></Type><Columns>"#,
                r#"<Column name="Графа" id="2"><Type/>"#,
                r#"<FunctionalOptions><Item>FunctionalOption.Опция</Item></FunctionalOptions>"#,
                r#"</Column></Columns></Attribute></Attributes>"#,
            ),
            concat!(
                r#"<Commands><Command name="Команда" id="1">"#,
                r#"<FunctionalOptions><Item>FunctionalOption.Опция</Item></FunctionalOptions>"#,
                r#"</Command></Commands>"#,
            ),
        ] {
            let xml = format!(
                concat!(
                    r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" "#,
                    r#"xmlns:v8="http://v8.1c.ru/8.1/data/core" "#,
                    r#"xmlns:xs="http://www.w3.org/2001/XMLSchema" "#,
                    r#"xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">"#,
                    r#"<AutoCommandBar name="ФормаКоманднаяПанель" id="-1"/>"#,
                    "{holder}</Form>",
                ),
                holder = holder,
            );
            let error = super::compile_native_form_body(xml.as_bytes(), None, None, None)
                .expect_err("functional options with no source must refuse the form")
                .to_string();
            assert!(
                error.contains("no source resolver"),
                "the holder was written instead of refused: {error}"
            );
        }
    }

    fn decode_base64_for_test(input: &str) -> anyhow::Result<Vec<u8>> {
        fn value(byte: u8) -> anyhow::Result<u8> {
            match byte {
                b'A'..=b'Z' => Ok(byte - b'A'),
                b'a'..=b'z' => Ok(byte - b'a' + 26),
                b'0'..=b'9' => Ok(byte - b'0' + 52),
                b'+' => Ok(62),
                b'/' => Ok(63),
                _ => anyhow::bail!("invalid base64 byte {byte}"),
            }
        }

        let bytes: Vec<u8> = input
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
        for chunk in bytes.chunks(4) {
            if chunk.len() != 4 {
                anyhow::bail!("invalid base64 length");
            }
            let first = value(chunk[0])?;
            let second = value(chunk[1])?;
            let third = if chunk[2] == b'=' {
                0
            } else {
                value(chunk[2])?
            };
            let fourth = if chunk[3] == b'=' {
                0
            } else {
                value(chunk[3])?
            };
            output.push((first << 2) | (second >> 4));
            if chunk[2] != b'=' {
                output.push((second << 4) | (third >> 2));
            }
            if chunk[3] != b'=' {
                output.push((third << 6) | fourth);
            }
        }
        Ok(output)
    }

    fn form_setting_base64_xml_for_test(text: &str, key: &str) -> anyhow::Result<String> {
        let key_start = text
            .find(&format!("\"{key}\""))
            .ok_or_else(|| anyhow::anyhow!("setting {key} not found"))?;
        let payload_prefix = "{#base64:";
        let payload_start = text[key_start..]
            .find(payload_prefix)
            .map(|relative| key_start + relative + payload_prefix.len())
            .ok_or_else(|| anyhow::anyhow!("setting {key} base64 payload not found"))?;
        let payload_end = text[payload_start..]
            .find('}')
            .map(|relative| payload_start + relative)
            .ok_or_else(|| anyhow::anyhow!("setting {key} base64 payload is not closed"))?;
        let bytes = decode_base64_for_test(&text[payload_start..payload_end])?;
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        String::from_utf8(bytes.to_vec()).map_err(Into::into)
    }

    #[test]
    fn configuration_version_load_changes_the_stored_slot_and_preserves_adoption() {
        use crate::metadata_model::brace::{self, Brace};
        for (dialect, marker) in [("2.20", 68), ("2.21", 76)] {
            let header = brace::parse_row(b"{3,{1,0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},\"Extension\",{0},\"\",1,0,00000000-0000-0000-0000-000000000000,0}").unwrap();
            let mut tuple = vec![Brace::num(0); marker + 1];
            tuple[0] = Brace::num(marker as i64);
            tuple[1] = Brace::list(vec![Brace::num(0), header]);
            tuple[15] = Brace::str("1.0.1.1");
            tuple[42] = Brace::str("ext_");
            tuple[44] = Brace::num(2);
            let row = Brace::list(vec![
                Brace::num(2),
                Brace::list(vec![Brace::uuid("aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa")]),
                Brace::num(1),
                Brace::list(vec![
                    Brace::uuid("9cd510cd-abfc-11d4-9434-004095e12fc7"),
                    Brace::list(vec![Brace::num(1), Brace::list(tuple)]),
                ]),
            ]);
            let base = deflate_raw(&brace::serialize_row(&row)).unwrap();
            for version in [Some("1.0.1.2"), Some(""), None] {
                let version_xml = version
                    .map(|v| format!("<Version>{v}</Version>"))
                    .unwrap_or_default();
                let xml = format!(
                    "<MetaDataObject version=\"{dialect}\"><Configuration uuid=\"aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa\"><Properties><ObjectBelonging>Adopted</ObjectBelonging><Name>Extension</Name><Synonym/><Comment/>{version_xml}</Properties></Configuration></MetaDataObject>"
                );
                let packed =
                    super::pack_simple_metadata_blob_from_xml(&base, xml.as_bytes()).unwrap();
                let back = brace::parse_row(&inflate_raw(&packed.blob).unwrap()).unwrap();
                let mut expected = row.clone();
                if let Some(version) = version {
                    let fields = expected.as_list_mut().unwrap()[3].as_list_mut().unwrap()[1]
                        .as_list_mut()
                        .unwrap()[1]
                        .as_list_mut()
                        .unwrap();
                    fields[15] = Brace::str(version);
                }
                assert_eq!(back, expected, "dialect={dialect} version={version:?}");
            }
        }
    }

    #[cfg(feature = "research-corpus-tests")]
    #[test]
    #[ignore = "requires external research corpus"]
    fn round_trips_real_new_sfc_simple_metadata_families() -> anyhow::Result<()> {
        let sfc_root = std::path::PathBuf::from(r"D:\УХА\sfc");
        if !sfc_root.is_dir() {
            return Ok(());
        }

        for (kind, file) in [
            ("Bot", "Bots/ОповещенияПользователейОСобытиях.xml"),
            (
                "DocumentNumerator",
                "DocumentNumerators/ДенежныеДокументы.xml",
            ),
            (
                "IntegrationService",
                "IntegrationServices/ОбменСообщениями.xml",
            ),
            ("Sequence", "Sequences/ДокументыОрганизаций.xml"),
            ("Style", "Styles/Основной.xml"),
            ("WSReference", "WSReferences/UpdateFilesApiImplService.xml"),
        ] {
            let xml = std::fs::read(sfc_root.join(file))?;
            let parsed = super::parse_simple_metadata_xml_properties(&xml)?;
            assert_eq!(parsed.kind, kind);

            let mut active = b"\xEF\xBB\xBF".to_vec();
            active.extend_from_slice(
                format!(
                    "{{1,\n{{3,\n{{1,0,{uuid}}},\"OldName\",\n{{1,\"ru\",\"Old synonym\"}},\"Old comment\",0,0,00000000-0000-0000-0000-000000000000,0}},0}}",
                    uuid = parsed.uuid,
                )
                .as_bytes(),
            );
            let base_blob = deflate_raw(&active)?;

            let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, &xml)?;
            let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;

            assert_eq!(packed.properties.kind, kind);
            assert_eq!(packed.properties.uuid, parsed.uuid);
            assert!(
                inflated.contains(&format!("\"{}\"", parsed.name)),
                "{inflated}"
            );
            if !parsed.comment.is_empty() {
                assert!(
                    inflated.contains(&format!("\"{}\"", parsed.comment)),
                    "{inflated}"
                );
            }
            assert!(inflated.contains("00000000-0000-0000-0000-000000000000"));
        }

        Ok(())
    }

    #[cfg(feature = "research-corpus-tests")]
    #[test]
    #[ignore = "requires external research corpus"]
    fn round_trips_common_form_and_template_from_lab_sources() -> anyhow::Result<()> {
        let lab_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");

        for (kind, file, uuid, expected_name) in [
            (
                "CommonForm",
                "CommonForms/ФормаОтчета.xml",
                "9d6d77a9-1f55-4162-93a5-14bb3f3febaf",
                "ФормаОтчета",
            ),
            (
                "CommonTemplate",
                "CommonTemplates/ВидыДокументовУдостоверяющихЛичность.xml",
                "1682d528-87bf-48c5-acf9-57ab654a615a",
                "ВидыДокументовУдостоверяющихЛичность",
            ),
        ] {
            let xml = std::fs::read(lab_root.join(file))?;
            let mut active = b"\xEF\xBB\xBF".to_vec();
            active.extend_from_slice(
                format!(
                    "{{1,\n{{3,\n{{1,0,{uuid}}},\"OldName\",\n{{1,\"ru\",\"Old synonym\"}},\"Old comment\",0,0,00000000-0000-0000-0000-000000000000,0}},0}}"
                )
                .as_bytes(),
            );
            let base_blob = deflate_raw(&active)?;

            let packed = super::pack_simple_metadata_blob_from_xml(&base_blob, &xml)?;
            let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;

            assert_eq!(packed.properties.kind, kind);
            assert_eq!(packed.properties.uuid, uuid);
            assert!(
                inflated.contains(&format!("\"{expected_name}\"")),
                "{inflated}"
            );
            assert!(inflated.as_bytes().starts_with(b"\xEF\xBB\xBF"));
        }

        Ok(())
    }

    #[test]
    fn patches_defined_type_cfg_reference_type_pattern() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Catalogs"))?;
        std::fs::write(
            root.join("Catalogs").join("TestUsers.xml"),
            br#"
<MetaDataObject>
  <Catalog uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <InternalInfo>
      <xr:GeneratedType xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" name="CatalogRef.TestUsers" category="Ref">
        <xr:TypeId>bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb</xr:TypeId>
      </xr:GeneratedType>
    </InternalInfo>
    <Properties>
      <Name>TestUsers</Name>
      <Synonym/>
      <Comment/>
    </Properties>
  </Catalog>
</MetaDataObject>
"#,
        )?;

        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{0,11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldType",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},
{"Pattern",{"S",10,1}}
},0}"#,
        );
        let base_blob = deflate_raw(&active)?;
        let xml = br#"
<MetaDataObject xmlns:v8="urn:v8">
  <DefinedType uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewType</Name>
      <Synonym/>
      <Comment/>
      <Type>
        <v8:Type>cfg:CatalogRef.TestUsers</v8:Type>
      </Type>
    </Properties>
  </DefinedType>
</MetaDataObject>
"#;
        let source = super::MetadataSourceContext::new(root.clone());

        let packed =
            super::pack_simple_metadata_blob_from_xml_with_source(&base_blob, xml, Some(&source))?;
        let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(packed.properties.kind, "DefinedType");
        assert!(inflated.contains("{\"Pattern\",{\"#\",bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb}}"));

        Ok(())
    }

    #[cfg(feature = "research-corpus-tests")]
    #[test]
    #[ignore = "requires external research corpus"]
    fn patches_command_group_metadata_blob_from_lab_xml() -> anyhow::Result<()> {
        let source_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let source = super::MetadataSourceContext::new(source_root);

        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{3,
{4,0,{0},"",-1,-1,1,0,""},4,3,
{0},
{0},
{3,
{1,0,c59e11f3-6bcb-404a-9d76-1416c12be354},"OldGroup",
{1,"ru","Old synonym"},"Old comment",0,0,00000000-0000-0000-0000-000000000000,0}
},0}"#,
        );
        let base_blob = deflate_raw(&active)?;
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommandGroup uuid="c59e11f3-6bcb-404a-9d76-1416c12be354">
    <Properties>
      <Name>Органайзер</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Органайзер</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture>
        <xr:Ref>CommonPicture.Органайзер</xr:Ref>
        <xr:LoadTransparent>false</xr:LoadTransparent>
      </Picture>
      <Category>FormCommandBar</Category>
    </Properties>
  </CommandGroup>
</MetaDataObject>
"#
        .as_bytes();

        let packed =
            super::pack_simple_metadata_blob_from_xml_with_source(&base_blob, xml, Some(&source))?;
        let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;

        assert_eq!(packed.properties.kind, "CommandGroup");
        assert!(inflated.contains("\"Органайзер\""), "{inflated}");
        assert!(inflated.contains("{1,\"ru\",\"Органайзер\"}"));
        assert!(
            inflated.contains("{4,1,{0,dce82d28-9a7b-4d4c-af13-90f459cf4af2},\"\",-1,-1,0,0,\"\"}")
        );

        Ok(())
    }

    #[test]
    fn patches_common_command_picture_and_parameter_type() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("CommonPictures"))?;
        std::fs::create_dir_all(root.join("DefinedTypes"))?;
        std::fs::create_dir_all(root.join("CommandGroups"))?;
        std::fs::write(
            root.join("CommonPictures").join("TestPicture.xml"),
            br#"
<MetaDataObject>
  <CommonPicture uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
    <Properties>
      <Name>TestPicture</Name>
      <Synonym/>
      <Comment/>
    </Properties>
  </CommonPicture>
</MetaDataObject>
"#,
        )?;
        std::fs::write(
            root.join("DefinedTypes").join("TestOwner.xml"),
            br#"
<MetaDataObject>
  <DefinedType uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
    <InternalInfo>
      <xr:GeneratedType xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" name="DefinedType.TestOwner" category="DefinedType">
        <xr:TypeId>cccccccc-cccc-4ccc-cccc-cccccccccccc</xr:TypeId>
      </xr:GeneratedType>
    </InternalInfo>
    <Properties>
      <Name>TestOwner</Name>
      <Synonym/>
      <Comment/>
    </Properties>
  </DefinedType>
</MetaDataObject>
"#,
        )?;
        std::fs::write(
            root.join("CommandGroups").join("Органайзер.xml"),
            r#"
<MetaDataObject>
  <CommandGroup uuid="c59e11f3-6bcb-404a-9d76-1416c12be354">
    <Properties>
      <Name>Органайзер</Name>
      <Synonym/>
      <Comment/>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture/>
      <Category>FormCommandBar</Category>
    </Properties>
  </CommandGroup>
</MetaDataObject>
"#,
        )?;

        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,dddddddd-dddd-4ddd-dddd-dddddddddddd,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,{0},"",-1,-1,1,0,""},3,
{0},1,
{0,0,0},0,
{1,77ea1b8f-dd79-4717-9dba-5628e7f348cf},
{"Pattern"},
{3,
{1,0,dddddddd-dddd-4ddd-dddd-dddddddddddd},"OldCommand",
{1,"ru","Old synonym"},"",0,0,00000000-0000-0000-0000-000000000000,0},0,0,0}
}
},0}"#,
        );
        let base_blob = deflate_raw(&active)?;
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommonCommand uuid="dddddddd-dddd-4ddd-dddd-dddddddddddd">
    <Properties>
      <Name>NewCommand</Name>
      <Synonym/>
      <Comment/>
      <Group>CommandGroup.Органайзер</Group>
      <Representation>Picture</Representation>
      <ToolTip/>
      <Picture>
        <xr:Ref>CommonPicture.TestPicture</xr:Ref>
        <xr:LoadTransparent>false</xr:LoadTransparent>
      </Picture>
      <Shortcut/>
      <IncludeHelpInContents>false</IncludeHelpInContents>
      <CommandParameterType>
        <v8:TypeSet>cfg:DefinedType.TestOwner</v8:TypeSet>
      </CommandParameterType>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#
        .as_bytes();
        let source = super::MetadataSourceContext::new(root.clone());

        let packed =
            super::pack_simple_metadata_blob_from_xml_with_source(&base_blob, xml, Some(&source))?;
        let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(packed.properties.kind, "CommonCommand");
        assert!(
            inflated
                .contains(r#"{4,1,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},"",-1,-1,0,0,""},1,"#)
        );
        assert!(inflated.contains("{1,c59e11f3-6bcb-404a-9d76-1416c12be354}"));
        assert!(inflated.contains("{\"Pattern\",{\"#\",cccccccc-cccc-4ccc-cccc-cccccccccccc}}"));

        Ok(())
    }

    #[test]
    fn packs_style_body_xml_with_standard_and_style_item_refs() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-style-body-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("StyleItems"))?;
        std::fs::write(
            root.join("StyleItems/ErrorBackColor.xml"),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
	<StyleItem uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa">
		<Properties>
			<Name>ErrorBackColor</Name>
		</Properties>
	</StyleItem>
</MetaDataObject>
"#,
        )?;
        std::fs::write(
            root.join("StyleItems/StrikeFont.xml"),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
	<StyleItem uuid="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb">
		<Properties>
			<Name>StrikeFont</Name>
		</Properties>
	</StyleItem>
</MetaDataObject>
"#,
        )?;
        let source = MetadataSourceContext::new(root.clone());
        let xml = br##"<?xml version="1.0" encoding="UTF-8"?>
<Style xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:style="http://v8.1c.ru/8.1/data/ui/style" xmlns:web="http://v8.1c.ru/8.1/data/ui/colors/web" version="2.21">
	<Item name="FormBackColor">
		<Color>web:Cream</Color>
	</Item>
	<Item name="ControlBorder">
		<Border ref="style:ControlBorder"/>
	</Item>
	<Item name="TextFont">
		<Font ref="style:TextFont" kind="StyleItem"/>
	</Item>
	<Item name="StyleItem.ErrorBackColor">
		<Color>#FFC8C8</Color>
	</Item>
	<Item name="StyleItem.StrikeFont">
		<Font ref="style:TextFont" kind="StyleItem" bold="false" italic="false" underline="false" strikeout="true"/>
	</Item>
</Style>
"##;

        let packed = super::pack_style_body_blob_from_xml(xml, Some(&source))?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;
        assert!(text.starts_with("\u{feff}{1,5,\r\n{\r\n{-1},0,\r\n{3,2,\r\n{20}\r\n}\r\n}"));
        let text = text.replace("\r\n", "");

        assert!(text.contains("{{-1},0,{3,2,{20}}}"));
        assert!(text.contains("{{-18},2,{3,1,{-18},0,0,0}}"));
        assert!(text.contains("{{-20},1,{7,2,0,{-20},1,100}}"));
        assert!(text.contains("{{0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},0,{3,0,{13158655}}}"));
        assert!(text.contains(
            "{{0,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb},1,{7,2,60,{-20},400,0,0,1,1,100}}"
        ));
        assert!(text.ends_with("1,100}}}"));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn style_brand_colour_roundtrips_and_can_be_updated_or_removed() -> anyhow::Result<()> {
        use crate::cli::InfobaseConfigSourceVersion;
        let xml = br##"<Style>
          <Item name="ImportantColor"><Color>#123456</Color></Item>
          <Item name="TitleLevel3"><Font ref="style:TitleLevel3" kind="StyleItem" height="10" bold="true" italic="false"/></Item>
          <Item name="FirstBrand"><Color>#112233</Color></Item>
        </Style>"##;
        let packed = super::pack_style_body_blob_from_xml(xml, None)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?.replace("\r\n", "");
        assert!(
            text.starts_with("\u{feff}{2,2,"),
            "brand is not part of the item count: {text}"
        );
        assert!(
            text.contains("{8,2,14,{-50},100,700,0,1,100}"),
            "font mask: {text}"
        );
        let read = |blob: &[u8]| {
            crate::mssql_dump::extract_style_body_xml(
                blob,
                &std::collections::BTreeMap::new(),
                &std::collections::BTreeMap::new(),
                InfobaseConfigSourceVersion::V2_21,
            )
        };
        let native_xml = read(&packed.blob)?;
        assert!(
            native_xml.contains(r#"<Item name="FirstBrand">"#),
            "{native_xml}"
        );
        assert!(
            native_xml.contains("<Color>#112233</Color>"),
            "{native_xml}"
        );
        assert!(
            native_xml.contains(r#"height="10" bold="true" italic="false""#),
            "{native_xml}"
        );
        let edited = String::from_utf8(xml.to_vec())?.replace("#112233", "#445566");
        let changed =
            super::pack_style_body_blob_from_xml_with_base(&packed.blob, edited.as_bytes(), None)?;
        assert!(read(&changed.blob)?.contains("<Color>#445566</Color>"));
        let no_brand = br##"<Style><Item name="ImportantColor"><Color>#123456</Color></Item>
          <Item name="TitleLevel3"><Font ref="style:TitleLevel3" kind="StyleItem" height="10" bold="true" italic="false"/></Item></Style>"##;
        let removed =
            super::pack_style_body_blob_from_xml_with_base(&changed.blob, no_brand, None)?;
        assert!(!read(&removed.blob)?.contains("FirstBrand"));
        assert!(super::pack_style_body_blob_from_xml(br#"<Style><Item name="FirstBrand"><Font ref="style:TextFont" kind="StyleItem"/></Item></Style>"#, None).is_err());
        Ok(())
    }

    fn form_item_asset_test_record(fields: Vec<String>) -> String {
        format!("{{{}}}", fields.join(","))
    }

    fn form_item_asset_test_fields(wrapper: &str, id: usize, len: usize) -> Vec<String> {
        let mut fields = vec!["0".to_string(); len];
        fields[0] = wrapper.to_string();
        fields[1] = format!("{{{id},0}}");
        fields
    }

    fn form_item_asset_test_picture_value(payload: &str) -> String {
        format!(r#"{{4,3,{{0}},"",-1,-1,0,{{{{#base64:{payload}}}}},0,""}}"#)
    }

    fn form_item_asset_test_body(items: &[String]) -> String {
        format!(r#"{{4,{{0}},"",{{2,{}}}}}"#, items.join(","))
    }

    fn form_item_asset_test_table(wrapper: &str, id: usize, name: &str, payload: &str) -> String {
        let mut fields = form_item_asset_test_fields(wrapper, id, 45);
        fields[5] = format!(r#""{name}""#);
        fields[43] = "{0}".to_string();
        fields[44] = form_item_asset_test_picture_value(payload);
        form_item_asset_test_record(fields)
    }

    #[test]
    fn packs_form_body_item_picture_assets_from_source_layout() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-item-assets-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        let assets_root = root.join("Items");
        std::fs::create_dir_all(assets_root.join("RenamedRows"))?;
        let new_png = b"\x89PNG\r\n\x1a\nnew";
        std::fs::write(
            assets_root.join("RenamedRows").join("RowsPicture.png"),
            new_png,
        )?;
        let base_text = form_item_asset_test_body(&[form_item_asset_test_table(
            "55",
            59,
            "RenamedRows",
            "iVBORw0KGgo=",
        )]);
        let base = super::deflate_raw(base_text.as_bytes())?;

        let packed = super::pack_form_body_blob_from_form_xml_with_source_and_assets(
            &base,
            b"",
            None,
            None,
            Some(&assets_root),
        )?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(&format!("{{#base64:{}}}", super::encode_base64(new_png))));
        assert!(!text.contains("{#base64:iVBORw0KGgo=}"));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_form_body_item_picture_assets_preserving_equal_wrapped_payload() -> anyhow::Result<()>
    {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-item-assets-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        let assets_root = root.join("Items");
        std::fs::create_dir_all(assets_root.join("RenamedRows"))?;
        let png = b"\x89PNG\r\n\x1a\n";
        std::fs::write(assets_root.join("RenamedRows").join("RowsPicture.png"), png)?;
        let base_text = form_item_asset_test_body(&[form_item_asset_test_table(
            "55",
            59,
            "RenamedRows",
            "iVBOR\r\nw0KGgo=",
        )]);
        let base = super::deflate_raw(base_text.as_bytes())?;

        let packed = super::pack_form_body_blob_from_form_xml_with_source_and_assets(
            &base,
            b"",
            None,
            None,
            Some(&assets_root),
        )?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("{#base64:iVBOR\r\nw0KGgo=}"));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_attributes() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-main-table-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Catalogs"))?;
        std::fs::write(
            root.join("Catalogs/Products.xml"),
            br#"<MetaDataObject><Catalog uuid="99999999-9999-4999-8999-999999999999"><Properties><Name>Products</Name></Properties></Catalog></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            br##"{4,{7,{"layout"}},"Old module",{4,1,{9,{1},0,"OldList",{1,0},{"Pattern",{"#",65abad24-838b-4987-8b35-ed9e2bd4d9c8}},{0,{0,{"B",1},0}},{0,{0,{"B",1},0}},{0,0},{0,0},0,0,0,0,{0,9,"QueryText",{"S","Old query"},"MainTable",{"#",88888888-8888-4888-8888-888888888888},"DynamicalDataSelection",{"B",1},"ManualQuery",{"B",0},"Filter",{"#",21743ff3-2db3-4cfc-9404-90ed8209437f,{#base64:77u/PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4NCjxGaWx0ZXIgeG1sbnM9Imh0dHA6Ly92OC4xYy5ydS84LjEvZGF0YS1jb21wb3NpdGlvbi1zeXN0ZW0vc2V0dGluZ3MiIHhtbG5zOnhzPSJodHRwOi8vd3d3LnczLm9yZy8yMDAxL1hNTFNjaGVtYSIgeG1sbnM6eHNpPSJodHRwOi8vd3d3LnczLm9yZy8yMDAxL1hNTFNjaGVtYS1pbnN0YW5jZSI+DQoJPHZpZXdNb2RlPk5vcm1hbDwvdmlld01vZGU+DQoJPHVzZXJTZXR0aW5nSUQ+ZGZjZWNlOWQtNTA3Ny00NDBiLWI2YjMtNDVhNWNiNDUzOGViPC91c2VyU2V0dGluZ0lEPg0KPC9GaWx0ZXI+}},"Order",{"#",11743ff3-2db3-4cfc-9404-90ed8209437f,{#base64:77u/PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4NCjxPcmRlciB4bWxucz0iaHR0cDovL3Y4LjFjLnJ1LzguMS9kYXRhLWNvbXBvc2l0aW9uLXN5c3RlbS9zZXR0aW5ncyIgeG1sbnM6eHM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDEvWE1MU2NoZW1hIiB4bWxuczp4c2k9Imh0dHA6Ly93d3cudzMub3JnLzIwMDEvWE1MU2NoZW1hLWluc3RhbmNlIj4NCgk8aXRlbSB4c2k6dHlwZT0iT3JkZXJJdGVtRmllbGQiPg0KCQk8ZmllbGQ+0J3QsNC40LzQtdC90L7QstCw0L3QuNC10J/QvtC70L3QvtC1PC9maWVsZD4NCgkJPG9yZGVyVHlwZT5Bc2M8L29yZGVyVHlwZT4NCgk8L2l0ZW0+DQoJPHZpZXdNb2RlPk5vcm1hbDwvdmlld01vZGU+DQoJPHVzZXJTZXR0aW5nSUQ+ODg2MTk3NjUtY2NiMy00NmM2LWFjNTItMzhlOWM5OTJlYmQ0PC91c2VyU2V0dGluZ0lEPg0KPC9PcmRlcj4=}},"ConditionalAppearance",{"#",31743ff3-2db3-4cfc-9404-90ed8209437f,{#base64:77u/PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiPz4NCjxDb25kaXRpb25hbEFwcGVhcmFuY2UgeG1sbnM9Imh0dHA6Ly92OC4xYy5ydS84LjEvZGF0YS1jb21wb3NpdGlvbi1zeXN0ZW0vc2V0dGluZ3MiIHhtbG5zOnhzPSJodHRwOi8vd3d3LnczLm9yZy8yMDAxL1hNTFNjaGVtYSIgeG1sbnM6eHNpPSJodHRwOi8vd3d3LnczLm9yZy8yMDAxL1hNTFNjaGVtYS1pbnN0YW5jZSI+DQoJPHZpZXdNb2RlPk5vcm1hbDwvdmlld01vZGU+DQoJPHVzZXJTZXR0aW5nSUQ+Yjc1ZmVjY2UtOTQyYi00YWVkLWFiYzktZTZhMDJlNDYwZmIzPC91c2VyU2V0dGluZ0lEPg0KPC9Db25kaXRpb25hbEFwcGVhcmFuY2U+}},"ItemsViewMode",{"S","Normal"},"ItemsUserSettingID",{"S","911b6018-f537-43e8-a417-da56b22f9aec"}},{0,0}}},{0,0},{0,0},{0}}"##,
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:dcscor="http://v8.1c.ru/8.1/data-composition-system/core" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<Attributes>
		<Attribute name="Список" id="1">
			<Type>
				<v8:Type>cfg:DynamicList</v8:Type>
			</Type>
			<MainAttribute>true</MainAttribute>
			<Settings xsi:type="DynamicList">
				<ManualQuery>true</ManualQuery>
				<DynamicDataRead>true</DynamicDataRead>
				<QueryText>ВЫБРАТЬ Ссылка ИЗ Справочник.Товары</QueryText>
				<MainTable>Catalog.Products</MainTable>
				<ListSettings>
					<dcsset:filter>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</dcsset:userSettingID>
					</dcsset:filter>
					<dcsset:order>
						<dcsset:item xsi:type="dcsset:OrderItemField">
							<dcsset:field>Код</dcsset:field>
							<dcsset:orderType>Asc</dcsset:orderType>
						</dcsset:item>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>88619765-ccb3-46c6-ac52-38e9c992ebd4</dcsset:userSettingID>
					</dcsset:order>
					<dcsset:conditionalAppearance>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>b75fecce-942b-4aed-abc9-e6a02e460fb3</dcsset:userSettingID>
					</dcsset:conditionalAppearance>
					<dcsset:itemsViewMode>Compact</dcsset:itemsViewMode>
					<dcsset:itemsUserSettingID>cccccccc-cccc-4ccc-cccc-cccccccccccc</dcsset:itemsUserSettingID>
				</ListSettings>
			</Settings>
		</Attribute>
	</Attributes>
</Form>
"#
        .as_bytes();

        let parsed_xml = super::parse_form_xml_body_properties(xml)?;
        let parsed_order = parsed_xml.attributes[0]
            .settings
            .as_ref()
            .and_then(|settings| settings.list_settings.order.as_ref())
            .ok_or_else(|| anyhow::anyhow!("order was not parsed from form XML"))?;
        assert!(matches!(
            parsed_order.canonical.items(),
            [ibcmd_core::dcs::DcsOrderItem::Field(field)] if field.field().as_str() == "Код"
        ));

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.layout, r#"{7,{"layout"}}"#);
        assert_eq!(parsed.module_text, "Old module");
        assert!(parsed.trailing[0].contains(r#""Список""#));
        assert!(
            parsed.trailing[0]
                .contains(r#""QueryText",{"S","ВЫБРАТЬ Ссылка ИЗ Справочник.Товары"}"#)
        );
        assert!(parsed.trailing[0].contains(r#""DynamicalDataSelection",{"B",0}"#));
        assert!(parsed.trailing[0].contains(r#""ManualQuery",{"B",1}"#));
        assert!(parsed.trailing[0].contains("\"MainTable\",{\"#\","));
        assert!(parsed.trailing[0].contains("99999999-9999-4999-8999-999999999999"));
        assert!(!parsed.trailing[0].contains("\"Filter\""));
        assert!(
            parsed.trailing[0]
                .contains("\"Order\",{\"#\",11743ff3-2db3-4cfc-9404-90ed8209437f,{#base64:")
        );
        let order_xml = form_setting_base64_xml_for_test(&parsed.trailing[0], "Order")?;
        assert!(order_xml.contains("<field>Код</field>"), "{order_xml}");
        assert!(
            order_xml.contains("<orderType>Asc</orderType>"),
            "{order_xml}"
        );
        assert!(!parsed.trailing[0].contains("\"Appearance\""));
        assert!(parsed.trailing[0].contains(r#""ItemsViewMode",{"S","Compact"}"#));
        assert!(
            parsed.trailing[0]
                .contains(r#""ItemsUserSettingID",{"S","cccccccc-cccc-4ccc-cccc-cccccccccccc"}"#)
        );
        assert!(!parsed.trailing[0].contains("88888888-8888-4888-8888-888888888888"));
        assert!(!parsed.trailing[0].contains("OldList"));
        assert!(!parsed.trailing[0].contains("Old query"));
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");
        assert_eq!(
            packed.plain_bytes,
            String::from_utf8(super::inflate_raw(&packed.blob)?)?.len()
        );

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_dynamic_list_attribute() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-new-dynamic-list-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Catalogs"))?;
        std::fs::write(
            root.join("Catalogs/Products.xml"),
            br#"<MetaDataObject><Catalog uuid="99999999-9999-4999-8999-999999999999"><Properties><Name>Products</Name></Properties></Catalog></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(br#"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:dcsset="http://v8.1c.ru/8.1/data-composition-system/settings" xmlns:v8="http://v8.1c.ru/8.1/data/core" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" version="2.20">
	<Attributes>
		<Attribute name="List" id="1">
			<Type>
				<v8:Type>cfg:DynamicList</v8:Type>
			</Type>
			<MainAttribute>true</MainAttribute>
			<Settings xsi:type="DynamicList">
				<ManualQuery>true</ManualQuery>
				<DynamicDataRead>true</DynamicDataRead>
				<QueryText>SELECT Ref FROM Catalog.Products</QueryText>
				<MainTable>Catalog.Products</MainTable>
				<ListSettings>
					<dcsset:filter>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>dfcece9d-5077-440b-b6b3-45a5cb4538eb</dcsset:userSettingID>
					</dcsset:filter>
					<dcsset:order>
						<dcsset:item xsi:type="dcsset:OrderItemField">
							<dcsset:field>Code</dcsset:field>
							<dcsset:orderType>Asc</dcsset:orderType>
						</dcsset:item>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb</dcsset:userSettingID>
					</dcsset:order>
					<dcsset:conditionalAppearance>
						<dcsset:viewMode>Normal</dcsset:viewMode>
						<dcsset:userSettingID>b75fecce-942b-4aed-abc9-e6a02e460fb3</dcsset:userSettingID>
					</dcsset:conditionalAppearance>
					<dcsset:itemsViewMode>Compact</dcsset:itemsViewMode>
					<dcsset:itemsUserSettingID>cccccccc-cccc-4ccc-cccc-cccccccccccc</dcsset:itemsUserSettingID>
				</ListSettings>
			</Settings>
		</Attribute>
	</Attributes>
</Form>
"#
        .as_bytes();

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let attributes_fields = super::scan_braced_fields(&parsed.trailing[0], 0)?;
        let attribute_fields =
            super::scan_braced_fields(&parsed.trailing[0], attributes_fields[2].start)?;

        assert_eq!(parsed.layout, r#"{7,{"layout"}}"#);
        assert_eq!(&parsed.trailing[0][attributes_fields[0].clone()], "4");
        assert_eq!(&parsed.trailing[0][attributes_fields[1].clone()], "1");
        assert_eq!(&parsed.trailing[0][attribute_fields[0].clone()], "9");
        assert_eq!(&parsed.trailing[0][attribute_fields[1].clone()], "{1}");
        assert_eq!(
            &parsed.trailing[0][attribute_fields[3].clone()],
            r#""List""#
        );
        assert_eq!(&parsed.trailing[0][attribute_fields[10].clone()], "1");
        assert!(parsed.trailing[0].contains(super::FORM_DYNAMIC_LIST_TYPE_UUID));
        assert!(
            parsed.trailing[0].contains(r#""QueryText",{"S","SELECT Ref FROM Catalog.Products"}"#)
        );
        assert!(parsed.trailing[0].contains(r#""DynamicalDataSelection",{"B",0}"#));
        assert!(parsed.trailing[0].contains(r#""ManualQuery",{"B",1}"#));
        assert!(parsed.trailing[0].contains("99999999-9999-4999-8999-999999999999"));
        assert!(!parsed.trailing[0].contains("\"Filter\""));
        assert!(
            parsed.trailing[0]
                .contains("\"Order\",{\"#\",11743ff3-2db3-4cfc-9404-90ed8209437f,{#base64:")
        );
        let order_xml = form_setting_base64_xml_for_test(&parsed.trailing[0], "Order")?;
        assert!(order_xml.contains("<field>Code</field>"), "{order_xml}");
        assert!(
            order_xml.contains("<orderType>Asc</orderType>"),
            "{order_xml}"
        );
        assert!(order_xml.contains("<viewMode>Normal</viewMode>"));
        assert!(
            order_xml
                .contains("<userSettingID>bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb</userSettingID>")
        );
        let conditional_appearance_policy =
            ibcmd_schema::bundled_dcs_conditional_appearance_policy()?;
        assert!(
            !parsed.trailing[0].contains(conditional_appearance_policy.storage_property_name()),
            "the platform-authenticated metadata-only shell has no physical storage record"
        );
        assert!(parsed.trailing[0].contains(r#""ItemsViewMode",{"S","Compact"}"#));
        assert!(
            parsed.trailing[0]
                .contains(r#""ItemsUserSettingID",{"S","cccccccc-cccc-4ccc-cccc-cccccccccccc"}"#)
        );
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_parameters() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-parameter-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("ChartsOfAccounts"))?;
        std::fs::write(
            root.join("ChartsOfAccounts").join("Хозрасчетный.xml"),
            r#"<MetaDataObject><ChartOfAccounts><InternalInfo><GeneratedType name="ChartOfAccountsRef.Хозрасчетный"><TypeId>99999999-9999-4999-8999-999999999999</TypeId></GeneratedType></InternalInfo><Properties><Name>Хозрасчетный</Name></Properties></ChartOfAccounts></MetaDataObject>"#
                .as_bytes(),
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            r##"{4,{7,{"layout"}},"Old module",{0},{0,1,{0,"Счет",{"Pattern",{"#",88888888-8888-4888-8888-888888888888}},0}},{0,0},{0}}"##
                .as_bytes(),
        )?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Parameters>
		<Parameter name="Счет">
			<Type>
				<v8:Type>cfg:ChartOfAccountsRef.Хозрасчетный</v8:Type>
			</Type>
			<KeyParameter>true</KeyParameter>
		</Parameter>
	</Parameters>
</Form>
"#
        .as_bytes();

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.layout, r#"{7,{"layout"}}"#);
        assert_eq!(parsed.module_text, "Old module");
        assert!(
            parsed.trailing[1]
                .contains(r##""Счет",{"Pattern",{"#",99999999-9999-4999-8999-999999999999}},1"##)
        );
        assert!(!parsed.trailing[1].contains("88888888-8888-4888-8888-888888888888"));
        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_parameter() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-new-parameter-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Catalogs"))?;
        std::fs::write(
            root.join("Catalogs").join("Products.xml"),
            r#"<MetaDataObject><Catalog><InternalInfo><GeneratedType name="CatalogRef.Products"><TypeId>99999999-9999-4999-8999-999999999999</TypeId></GeneratedType></InternalInfo><Properties><Name>Products</Name></Properties></Catalog></MetaDataObject>"#
                .as_bytes(),
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(br#"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0}}"#)?;
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Parameters>
		<Parameter name="Product">
			<Type>
				<v8:Type>cfg:CatalogRef.Products</v8:Type>
			</Type>
			<KeyParameter>true</KeyParameter>
		</Parameter>
	</Parameters>
</Form>
"#
        .as_bytes();

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let parameters_fields = super::scan_braced_fields(&parsed.trailing[1], 0)?;
        let parameter_fields =
            super::scan_braced_fields(&parsed.trailing[1], parameters_fields[2].start)?;

        assert_eq!(parsed.layout, r#"{7,{"layout"}}"#);
        assert_eq!(&parsed.trailing[1][parameters_fields[0].clone()], "0");
        assert_eq!(&parsed.trailing[1][parameters_fields[1].clone()], "1");
        assert_eq!(&parsed.trailing[1][parameter_fields[0].clone()], "0");
        assert_eq!(
            &parsed.trailing[1][parameter_fields[1].clone()],
            r#""Product""#
        );
        assert!(
            parsed.trailing[1].contains(
                r##""Product",{"Pattern",{"#",99999999-9999-4999-8999-999999999999}},1"##
            ),
            "{}",
            parsed.trailing[1]
        );
        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[2], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_existing_commands() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-command-options-{}",
            std::process::id()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("FunctionalOptions"))?;
        std::fs::write(
            root.join("FunctionalOptions").join("UseFeature.xml"),
            br#"<MetaDataObject><FunctionalOption uuid="99999999-9999-4999-8999-999999999999"><Properties><Name>UseFeature</Name></Properties></FunctionalOption></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            b"{4,{7,{\"layout\"}},\"Old module\",{0},{0,0},{0,1,{11,{2,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},\"Do\",{1,\"ru\",\"Old title\"},{1,\"ru\",\"Old tip\"},0,0,0,\"OldAction\",0,0,0,{0,1,88888888-8888-4888-8888-888888888888}}},{0}}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:v8="http://v8.1c.ru/8.1/data/core" version="2.20">
	<Commands>
		<Command name="Do" id="2">
			<Title>
				<v8:item>
					<v8:lang>ru</v8:lang>
					<v8:content>New title</v8:content>
				</v8:item>
			</Title>
			<ToolTip>
				<v8:item>
					<v8:lang>ru</v8:lang>
					<v8:content>New tip</v8:content>
				</v8:item>
			</ToolTip>
			<Action>NewAction</Action>
			<ModifiesSavedData>true</ModifiesSavedData>
			<CurrentRowUse>DontUse</CurrentRowUse>
			<FunctionalOptions>
				<Item>FunctionalOption.UseFeature</Item>
			</FunctionalOptions>
		</Command>
	</Commands>
</Form>
"#;

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert_eq!(parsed.layout, r#"{7,{"layout"}}"#);
        assert_eq!(parsed.module_text, "Old module");
        assert!(parsed.trailing[2].contains(r#""Do",{1,"ru","New title"}"#));
        assert!(parsed.trailing[2].contains(r#"{1,"ru","New tip"}"#));
        assert!(
            parsed.trailing[2]
                .contains(r#""NewAction",3,1,0,{0,1,99999999-9999-4999-8999-999999999999}"#)
        );
        assert!(!parsed.trailing[2].contains("Old title"));
        assert!(!parsed.trailing[2].contains("Old tip"));
        assert!(!parsed.trailing[2].contains("OldAction"));
        assert!(!parsed.trailing[2].contains("88888888-8888-4888-8888-888888888888"));
        assert_eq!(parsed.trailing[0], "{0}");
        assert_eq!(parsed.trailing[1], "{0,0}");
        assert_eq!(parsed.trailing[3], "{0}");
        assert_eq!(
            packed.plain_bytes,
            String::from_utf8(super::inflate_raw(&packed.blob)?)?.len()
        );

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_form_body_xml_new_command_interface_item_from_source() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-new-interface-command-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("DataProcessors"))?;
        std::fs::write(
            root.join("DataProcessors/Loader.xml"),
            br#"<MetaDataObject><DataProcessor uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"><Properties><Name>Loader</Name></Properties><ChildObjects><Command uuid="99999999-9999-4999-8999-999999999999"><Properties><Name>Load</Name></Properties></Command></ChildObjects></DataProcessor></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(br##"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0}}"##)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandInterface>
		<NavigationPanel>
			<Item>
				<Command>DataProcessor.Loader.Command.Load</Command>
				<Type>Added</Type>
				<CommandGroup>FormNavigationPanelImportant</CommandGroup>
				<Index>3</Index>
				<DefaultVisible>false</DefaultVisible>
				<Visible>
					<xr:Common>false</xr:Common>
				</Visible>
			</Item>
		</NavigationPanel>
	</CommandInterface>
</Form>
"#;

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let fields = super::scan_braced_fields(&parsed.trailing[3], 0)?;

        assert_eq!(&parsed.trailing[3][fields[0].clone()], "0");
        assert_eq!(&parsed.trailing[3][fields[1].clone()], "1");
        assert!(parsed.trailing[3].contains("99999999-9999-4999-8999-999999999999"));
        assert!(parsed.trailing[3].contains("dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2"));
        assert!(parsed.trailing[3].contains("},3,0,{0,{0,{\"B\",0},0}}"));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_popup_usual_group_title_styles_from_current_source() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-popup-style-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("StyleItems"))?;
        for (name, uuid) in [
            ("AccentOne", "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
            ("AccentTwo", "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"),
        ] {
            std::fs::write(
                root.join("StyleItems").join(format!("{name}.xml")),
                format!(
                    r#"<MetaDataObject><StyleItem uuid="{uuid}"><Properties><Name>{name}</Name></Properties></StyleItem></MetaDataObject>"#
                ),
            )?;
        }
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<UsualGroup name="PopupA" id="22">
			<TitleTextColor>style:AccentOne</TitleTextColor>
			<TitleFont ref="style:NormalTextFont" kind="StyleItem"/>
			<Behavior>PopUp</Behavior>
		</UsualGroup>
		<UsualGroup name="PopupB" id="23">
			<TitleTextColor>style:AccentTwo</TitleTextColor>
			<Behavior>PopUp</Behavior>
		</UsualGroup>
	</ChildItems>
</Form>
"#;

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let _ = std::fs::remove_dir_all(&root);

        assert!(
            parsed
                .layout
                .contains("{4,3,{0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},3}")
        );
        assert!(
            parsed
                .layout
                .contains("{4,3,{0,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb},3}")
        );
        assert!(parsed.layout.contains("{8,2,0,{-31},1,100}"));
        Ok(())
    }

    #[test]
    fn patches_popup_title_style_preserving_form_tuple_flavor() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-popup-style-flavor-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("StyleItems"))?;
        std::fs::write(
            root.join("StyleItems/AccentLegacy.xml"),
            br#"<MetaDataObject><StyleItem uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"><Properties><Name>AccentLegacy</Name></Properties></StyleItem></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            r#"{4,{59,1,11111111-1111-4111-8111-111111111111,{22,{22,22222222-2222-4222-8222-222222222222},0,0,0,5,"MainGroup",{1,0},{1,0},0,1,0,0,0,2,2,{3,4,{0}},{7,3,0,1,100},{0,0,0},1,{38,0,0,0,0,{0},{1,0},{"Pattern"},"",{4,4,{0},4},0,0,0,1,{1,0},0,0,3,3,2,0,1,0,{4,4,{0},4},0,2,0,0,0,0,0,0,{4,0,{0},"",-1,-1,1,0,""},{0,1,0},0,0,0,0,2,0,0,0},0,1,0,0,0,3,3,0}},"Old module",{0}}"#
                .as_bytes(),
        )?;
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><UsualGroup name="MainGroup" id="22"><TitleTextColor>style:AccentLegacy</TitleTextColor><TitleFont ref="style:NormalTextFont" kind="StyleItem"/><Behavior>PopUp</Behavior></UsualGroup></ChildItems></Form>"#;

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let _ = std::fs::remove_dir_all(&root);

        assert!(
            parsed
                .layout
                .contains("{3,3,{0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa}}")
        );
        assert!(parsed.layout.contains("{7,2,0,{-31},1,100}"));
        assert!(
            !parsed
                .layout
                .contains("{4,3,{0,aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa},3}")
        );
        assert!(!parsed.layout.contains("{8,2,0,{-31},1,100}"));
        Ok(())
    }

    #[test]
    fn popup_usual_group_style_requires_declared_source_metadata() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform">
	<ChildItems>
		<UsualGroup name="Popup" id="22">
			<TitleTextColor>style:MissingAccent</TitleTextColor>
			<Behavior>PopUp</Behavior>
		</UsualGroup>
	</ChildItems>
</Form>
"#;

        let source_less = super::pack_form_body_blob_from_form_xml(&base, xml, None)
            .expect_err("custom StyleItem must not resolve without a source root");
        assert!(format!("{source_less:#}").contains("source root is required"));

        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-popup-missing-style-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("StyleItems"))?;
        let source = super::MetadataSourceContext::new(root.clone());
        let missing_style =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))
                .expect_err("undeclared StyleItem must not produce a serialized UUID");
        let _ = std::fs::remove_dir_all(&root);

        assert!(format!("{missing_style:#}").contains("failed to read StyleItem XML"));
        Ok(())
    }

    #[test]
    fn packs_nested_new_text_document_field_with_context_menu_slots() -> anyhow::Result<()> {
        let base = super::deflate_raw(br#"{4,{59,0},"Old module",{0}}"#)?;
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform"><ChildItems><Pages name="Tabs" id="1"><ChildItems><Page name="General" id="2"><ChildItems><TextDocumentField name="Notes" id="3"/></ChildItems></Page></ChildItems></Pages></ChildItems></Form>"#;

        let packed = super::pack_form_body_blob_from_form_xml(&base, xml, None)?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;
        let layout_fields = super::scan_braced_fields(&parsed.layout, 0)?;
        let pages_fields = super::scan_braced_fields(&parsed.layout, layout_fields[3].start)?;
        let page_fields = super::scan_braced_fields(&parsed.layout, pages_fields[12].start)?;
        let text_fields = super::scan_braced_fields(&parsed.layout, page_fields[12].start)?;

        assert_eq!(&parsed.layout[text_fields[0].clone()], "48");
        assert_eq!(&parsed.layout[text_fields[5].clone()], "7");
        assert_eq!(&parsed.layout[text_fields[6].clone()], r#""Notes""#);
        assert_eq!(text_fields.len(), 43);
        assert_eq!(&parsed.layout[text_fields[41].clone()], "0");
        assert_eq!(&parsed.layout[text_fields[42].clone()], "0");
        let extracted = crate::mssql_dump::extract_form_body_xml(
            &packed.blob,
            &std::collections::BTreeMap::new(),
        )
        .expect("nested TextDocumentField must remain exportable");
        assert!(extracted.contains("<TextDocumentField name=\"Notes\" id=\"3\""));

        Ok(())
    }

    #[test]
    fn packs_form_body_xml_external_button_command_from_source() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-command-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("DataProcessors"))?;
        std::fs::write(
            root.join("DataProcessors/Loader.xml"),
            br#"<MetaDataObject><DataProcessor uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"><Properties><Name>Loader</Name></Properties><ChildObjects><Command uuid="99999999-9999-4999-8999-999999999999"><Properties><Name>Load</Name></Properties></Command></ChildObjects></DataProcessor></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            br##"{4,{59,{34,{44,dddddddd-dddd-4ddd-dddd-dddddddddddd},0,0,0,"OldButton",{1,0},1,{0,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee},{0}}},"Old module",{0}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<ChildItems>
		<Button name="OldButton" id="44">
			<CommandName>DataProcessor.Loader.Command.Load</CommandName>
		</Button>
	</ChildItems>
</Form>
"#;

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(
            parsed
                .layout
                .contains("{0,99999999-9999-4999-8999-999999999999}")
        );
        assert!(
            !parsed
                .layout
                .contains("{0,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee}")
        );
        assert_eq!(parsed.module_text, "Old module");
        assert_eq!(parsed.trailing, vec!["{0}"]);

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_form_command_interface_external_command_from_source() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-form-interface-command-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("DataProcessors"))?;
        std::fs::write(
            root.join("DataProcessors/Loader.xml"),
            br#"<MetaDataObject><DataProcessor uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"><Properties><Name>Loader</Name></Properties><ChildObjects><Command uuid="99999999-9999-4999-8999-999999999999"><Properties><Name>Load</Name></Properties></Command></ChildObjects></DataProcessor></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            br##"{4,{7,{"layout"}},"Old module",{0},{0,0},{0,0},{0,1,{3,0,{0,eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee},{0},1,{0,eacad741-96b9-4b3a-bf79-dde9ecead1a1},0,1,{0,{0,{"B",1},0}}}}}"##,
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandInterface>
		<NavigationPanel>
			<Item>
				<Command>DataProcessor.Loader.Command.Load</Command>
				<Type>Added</Type>
				<CommandGroup>FormNavigationPanelGoTo</CommandGroup>
				<DefaultVisible>false</DefaultVisible>
				<Visible>
					<xr:Common>false</xr:Common>
				</Visible>
			</Item>
		</NavigationPanel>
	</CommandInterface>
</Form>
"#;

        let packed =
            super::pack_form_body_blob_from_form_xml_with_source(&base, xml, None, Some(&source))?;
        let parsed = super::parse_form_body_blob(&packed.blob)?;

        assert!(parsed.trailing[3].contains("99999999-9999-4999-8999-999999999999"));
        assert!(!parsed.trailing[3].contains("eeeeeeee-eeee-4eee-eeee-eeeeeeeeeeee"));
        assert!(parsed.trailing[3].contains("},0,0,{0,{0,{\"B\",0},0}}"));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn audits_module_only_form_body_as_missing_layout_source() -> anyhow::Result<()> {
        let blockers = super::form_body_base_free_blockers(&[], true, 0)?;

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("Form body source is absent"));
        assert!(blockers[0].contains("marker-50 source model"));
        Ok(())
    }

    #[test]
    fn audits_sectionless_form_body_as_base_free() -> anyhow::Result<()> {
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20"/>"#;
        let blockers = super::form_body_base_free_blockers(xml, false, 0)?;

        assert!(blockers.is_empty(), "{blockers:?}");
        Ok(())
    }

    #[test]
    fn audits_supported_form_sections_and_embedded_assets_separately() -> anyhow::Result<()> {
        let xml = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20">
	<Width>480</Width>
	<Attributes>
		<Attribute name="Object" id="1">
			<Type>
				<Type>cfg:CatalogObject.Products</Type>
			</Type>
		</Attribute>
	</Attributes>
	<Parameters>
		<Parameter name="Key" id="2"/>
	</Parameters>
	<Commands>
		<Command name="Do" id="3"/>
	</Commands>
	<CommandInterface/>
	<ChildItems>
		<Button name="Run" id="4"/>
	</ChildItems>
</Form>"#;

        let blockers = super::form_body_base_free_blockers(xml, true, 2)?;

        assert_eq!(blockers.len(), 1, "{blockers:?}");
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("embedded Form item assets")
                    && blocker.contains("2 source asset file"))
        );
        Ok(())
    }

    #[test]
    fn packs_role_rights_xml_matches_source_objects_by_ref_key() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-role-rights-source-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Catalogs"))?;
        std::fs::create_dir_all(root.join("DataProcessors"))?;
        std::fs::create_dir_all(root.join("InformationRegisters"))?;
        std::fs::write(
            root.join("Catalogs/Products.xml"),
            br#"<MetaDataObject><Catalog uuid="aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa"><Properties><Name>Products</Name></Properties><ChildObjects><Form uuid="ffffffff-ffff-4fff-8fff-ffffffffffff"><Properties><Name>ItemForm</Name></Properties></Form></ChildObjects></Catalog></MetaDataObject>"#,
        )?;
        std::fs::write(
            root.join("DataProcessors/Loader.xml"),
            br#"<MetaDataObject><DataProcessor uuid="cccccccc-cccc-4ccc-8ccc-cccccccccccc"><Properties><Name>Loader</Name></Properties><ChildObjects><Command uuid="dddddddd-dddd-4ddd-8ddd-dddddddddddd"><Properties><Name>Run</Name></Properties></Command></ChildObjects></DataProcessor></MetaDataObject>"#,
        )?;
        std::fs::write(
            root.join("InformationRegisters/Prices.xml"),
            br#"<MetaDataObject><InformationRegister uuid="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"><Properties><Name>Prices</Name></Properties><ChildObjects><Dimension uuid="eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee"><Properties><Name>Contract</Name></Properties></Dimension></ChildObjects></InformationRegister></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            b"{10,{6,{{1,dddddddd-dddd-4ddd-8ddd-dddddddddddd,0,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,-1}},{{1,eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee,0,0},{0,b7bab52d-c1b1-4bd8-8276-02db08d42352,-1}},{{1,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,1,1},{0,b7bab52d-c1b1-4bd8-8276-02db08d42352,-1}},{{1,ffffffff-ffff-4fff-8fff-ffffffffffff,0,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,-1}},{{1,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,0,0},{0,1c87578f-9e09-4ec0-a991-5629c87b1588,-1}},{{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,0,0},{0,287b74b8-3a66-4a76-ba27-4f1f6a93770e,-1}}},{0},0,1,0,4294967295}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>false</setForNewObjects>
	<setForAttributesByDefault>true</setForAttributesByDefault>
	<independentRightsOfChildObjects>false</independentRightsOfChildObjects>
	<object>
		<name>Catalog.Products</name>
		<right><name>Update</name><value>true</value></right>
	</object>
	<object>
		<name>InformationRegister.Prices.StandardAttribute.Period</name>
		<right><name>Edit</name><value>true</value></right>
	</object>
	<object>
		<name>DataProcessor.Loader.Command.Run</name>
		<right><name>View</name><value>true</value></right>
	</object>
	<object>
		<name>Catalog.Products.Form.ItemForm</name>
		<right><name>View</name><value>true</value></right>
	</object>
	<object>
		<name>InformationRegister.Prices.Dimension.Contract</name>
		<right><name>Edit</name><value>true</value></right>
	</object>
	<object>
		<name>InformationRegister.Prices</name>
		<right><name>Read</name><value>true</value></right>
	</object>
</Rights>
"#;

        let packed = super::pack_role_rights_blob_from_xml_with_source(&base, xml, Some(&source))?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(
            "{1,dddddddd-dddd-4ddd-8ddd-dddddddddddd,0,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1}"
        ));
        assert!(text.contains(
            "{1,eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee,0,0},{0,b7bab52d-c1b1-4bd8-8276-02db08d42352,1}"
        ));
        assert!(text.contains(
            "{1,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,1,1},{0,b7bab52d-c1b1-4bd8-8276-02db08d42352,1}"
        ));
        assert!(text.contains(
            "{1,ffffffff-ffff-4fff-8fff-ffffffffffff,0,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1}"
        ));
        assert!(text.contains(
            "{1,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,0,0},{0,1c87578f-9e09-4ec0-a991-5629c87b1588,1}"
        ));
        assert!(text.contains(
            "{1,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa,0,0},{0,287b74b8-3a66-4a76-ba27-4f1f6a93770e,1}"
        ));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_role_rights_task_addressing_attribute_by_source_ref() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-role-rights-task-addressing-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Tasks"))?;
        std::fs::write(
            root.join("Tasks/ExecutorTask.xml"),
            br#"<MetaDataObject><Task uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"><Properties><Name>ExecutorTask</Name></Properties><ChildObjects><AddressingAttribute uuid="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"><Properties><Name>Assignee</Name></Properties></AddressingAttribute></ChildObjects></Task></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            b"{10,{1,{{1,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,0,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,-1}}},{0},0,1,0,4294967295}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>false</setForNewObjects>
	<setForAttributesByDefault>true</setForAttributesByDefault>
	<independentRightsOfChildObjects>false</independentRightsOfChildObjects>
	<object>
		<name>Task.ExecutorTask.AddressingAttribute.Assignee</name>
		<right><name>View</name><value>true</value></right>
	</object>
</Rights>
"#;

        let packed = super::pack_role_rights_blob_from_xml_with_source(&base, xml, Some(&source))?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(
            "{1,bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb,0,0},{0,aa6448f2-be0f-42ea-ba26-1af7f52b5b65,1}"
        ));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn packs_role_rights_http_service_method_by_source_ref() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-role-rights-http-method-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("HTTPServices"))?;
        std::fs::write(
            root.join("HTTPServices/Api.xml"),
            br#"<MetaDataObject><HTTPService uuid="aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"><Properties><Name>Api</Name></Properties><ChildObjects><URLTemplate uuid="bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"><Properties><Name>Any</Name><Template>/*</Template></Properties><ChildObjects><Method uuid="cccccccc-cccc-4ccc-8ccc-cccccccccccc"><Properties><Name>POST</Name><HTTPMethod>POST</HTTPMethod><Handler>HandlePost</Handler></Properties></Method></ChildObjects></URLTemplate></ChildObjects></HTTPService></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(
            b"{10,{1,{{1,cccccccc-cccc-4ccc-8ccc-cccccccccccc,0,0},{0,c6de80da-a4f7-4ce9-bbeb-0b00ea564ec1,-1}}},{0},0,1,0,4294967295}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>false</setForNewObjects>
	<setForAttributesByDefault>true</setForAttributesByDefault>
	<independentRightsOfChildObjects>false</independentRightsOfChildObjects>
	<object>
		<name>HTTPService.Api.URLTemplate.Any.Method.POST</name>
		<right><name>Use</name><value>true</value></right>
	</object>
</Rights>
"#;

        let packed = super::pack_role_rights_blob_from_xml_with_source(&base, xml, Some(&source))?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains(
            "{1,cccccccc-cccc-4ccc-8ccc-cccccccccccc,0,0},{0,c6de80da-a4f7-4ce9-bbeb-0b00ea564ec1,1}"
        ));

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[test]
    fn audits_role_rights_as_base_dependent_with_precise_blockers() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20">
	<setForNewObjects>true</setForNewObjects>
	<setForAttributesByDefault>false</setForAttributesByDefault>
	<independentRightsOfChildObjects>true</independentRightsOfChildObjects>
	<object>
		<name>Catalog.Products</name>
		<right>
			<name>View</name>
			<value>true</value>
		</right>
		<right>
			<name>Read</name>
			<value>true</value>
			<restrictionByCondition><condition>WHERE TRUE</condition></restrictionByCondition>
		</right>
	</object>
	<restrictionTemplate>
		<name>OwnerOnly</name>
		<condition>WHERE Owner = &amp;CurrentUser</condition>
	</restrictionTemplate>
</Rights>
"#;

        let blockers = super::role_rights_base_free_blockers(xml, None)?;

        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("base Role object table order"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("platform right UUID slots"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("conditional rights"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("restriction templates"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("trailing field positions"))
        );

        Ok(())
    }

    #[test]
    fn packs_command_interface_xml_preserving_command_refs() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{7,1,2,{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{0,{0,{\"B\",0},0}},{100,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},{0,{0,{\"B\",1},0}},0,0,0,0,0}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="Catalog.Products.StandardCommand.OpenList">
			<Visibility><xr:Common>true</xr:Common></Visibility>
		</Command>
		<Command name="100:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
			<Visibility><xr:Common>false</xr:Common></Visibility>
		</Command>
	</CommandsVisibility>
</CommandInterface>
"#;

        let packed = super::pack_command_interface_blob_from_xml(&base, xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},{0,{0,{\"B\",1},0}}"));
        assert!(text.contains("{100,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},{0,{0,{\"B\",0},0}}"));
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn packs_command_interface_xml_preserving_equivalent_visibility_format() -> anyhow::Result<()> {
        let base = super::deflate_raw(
            b"{7,1,1,\n{0,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},\n{0,\n{0,\n{\"B\",0},0}\n},0,0,0,0,0}",
        )?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="Catalog.Products.StandardCommand.OpenList">
			<Visibility><xr:Common>false</xr:Common></Visibility>
		</Command>
	</CommandsVisibility>
</CommandInterface>
"#;

        let packed = super::pack_command_interface_blob_from_xml(&base, xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("{0,\n{0,\n{\"B\",0},0}\n}"), "{text:?}");

        Ok(())
    }

    #[test]
    fn packs_command_interface_xml_without_base_for_raw_command_refs() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="100:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
			<Visibility><xr:Common>true</xr:Common></Visibility>
		</Command>
		<Command name="0:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
			<Visibility><xr:Common>false</xr:Common></Visibility>
		</Command>
	</CommandsVisibility>
</CommandInterface>
"#;

        assert!(super::command_interface_xml_can_pack_without_base(xml)?);
        let packed = super::pack_command_interface_blob_from_xml(&[], xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(
            text,
            "\u{feff}{7,1,2,\r\n{100,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa},\r\n{0,\r\n{0,\r\n{\"B\",1},0}\r\n},\r\n{0,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb},\r\n{0,\r\n{0,\r\n{\"B\",0},0}\r\n},0,0,0,0,0}"
        );
        assert_eq!(packed.plain_bytes, text.len());

        Ok(())
    }

    #[test]
    fn base_free_command_interface_wrapper_packs_raw_refs() -> anyhow::Result<()> {
        let xml = br#"<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
  <CommandsVisibility>
    <Command name="100:aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa">
      <Visibility><xr:Common>true</xr:Common></Visibility>
    </Command>
  </CommandsVisibility>
</CommandInterface>"#;

        let packed = super::pack_command_interface_blob_from_xml_base_free(xml)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert!(text.contains("{100,aaaaaaaa-aaaa-4aaa-aaaa-aaaaaaaaaaaa}"));
        assert_eq!(packed.plain_bytes, text.len());
        Ok(())
    }

    #[test]
    fn command_interface_xml_without_base_rejects_readable_command_refs() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="Catalog.Products.StandardCommand.OpenList">
			<Visibility><xr:Common>true</xr:Common></Visibility>
		</Command>
	</CommandsVisibility>
</CommandInterface>
"#;

        assert!(!super::command_interface_xml_can_pack_without_base(xml).unwrap());
        let error = super::pack_command_interface_blob_from_xml(&[], xml).unwrap_err();
        assert!(
            error.to_string().contains("requires a base entry")
                && error
                    .to_string()
                    .contains("Catalog.Products.StandardCommand.OpenList"),
            "{error}"
        );
    }

    /// The defect the writer replaced: a file holding only readable
    /// subsystem names was accepted without a base and compiled to the empty
    /// body `{7,0,0,0,0,0,0}`, dropping the section.
    #[test]
    fn subsystems_order_alone_is_never_packed_as_an_empty_body() {
        let xml = br#"<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<SubsystemsOrder>
		<Subsystem>Subsystem.A.Subsystem.B</Subsystem>
	</SubsystemsOrder>
</CommandInterface>"#;

        assert!(!super::command_interface_xml_can_pack_without_base(xml).unwrap());
        assert!(super::pack_command_interface_blob_from_xml_base_free(xml).is_err());
    }

    #[test]
    fn base_free_command_interface_wrapper_rejects_readable_refs() {
        let xml = br#"<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
  <CommandsVisibility>
    <Command name="Catalog.Products.StandardCommand.OpenList">
      <Visibility><xr:Common>true</xr:Common></Visibility>
    </Command>
  </CommandsVisibility>
</CommandInterface>"#;

        let error = super::pack_command_interface_blob_from_xml_base_free(xml).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("requires a base entry"), "{message}");
        assert!(
            message.contains("Catalog.Products.StandardCommand.OpenList"),
            "{message}"
        );
    }

    #[test]
    fn base_free_command_interface_wrapper_rejects_malformed_xml() {
        for xml in [
            b"not XML".as_slice(),
            b"<WrongRoot/>".as_slice(),
            b"<CommandInterface>".as_slice(),
        ] {
            let error = super::pack_command_interface_blob_from_xml_base_free(xml).unwrap_err();
            assert!(
                error.to_string().contains("CommandInterface XML"),
                "{error}"
            );
        }
    }

    #[test]
    fn reports_command_interface_readable_ref_base_free_blockers() -> anyhow::Result<()> {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<CommandInterface xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" xmlns:xr="http://v8.1c.ru/8.3/xcf/readable" version="2.20">
	<CommandsVisibility>
		<Command name="Catalog.Products.StandardCommand.OpenList">
			<Visibility><xr:Common>true</xr:Common></Visibility>
		</Command>
		<Command name="100:bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb">
			<Visibility><xr:Common>false</xr:Common></Visibility>
		</Command>
	</CommandsVisibility>
</CommandInterface>
"#;

        let blockers = super::command_interface_base_free_blockers(xml)?;

        assert_eq!(blockers.len(), 1);
        assert!(blockers[0].contains("without a source tree"));
        assert!(blockers[0].contains("Catalog.Products.StandardCommand.OpenList"));

        Ok(())
    }

    #[test]
    fn packs_exchange_plan_content_xml_with_metadata_refs() -> anyhow::Result<()> {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-module-blob-test-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Catalogs"))?;
        std::fs::create_dir_all(root.join("InformationRegisters"))?;
        std::fs::write(
            root.join("Catalogs/Customers.xml"),
            br#"<MetaDataObject><Catalog uuid="bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb"><Properties><Name>Customers</Name></Properties></Catalog></MetaDataObject>"#,
        )?;
        std::fs::write(
            root.join("InformationRegisters/Prices.xml"),
            br#"<MetaDataObject><InformationRegister uuid="cccccccc-cccc-4ccc-cccc-cccccccccccc"><Properties><Name>Prices</Name></Properties></InformationRegister></MetaDataObject>"#,
        )?;
        let source = super::MetadataSourceContext::new(root.clone());
        let base = super::deflate_raw(b"{2,0}")?;
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<ExchangePlanContent xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" version="2.20">
	<Item>
		<Metadata>Catalog.Customers</Metadata>
		<AutoRecord>Deny</AutoRecord>
	</Item>
	<Item>
		<Metadata>InformationRegister.Prices</Metadata>
		<AutoRecord>Auto</AutoRecord>
	</Item>
</ExchangePlanContent>
"#;

        let packed = super::pack_exchange_plan_content_blob_from_xml(&base, xml, &source)?;
        let text = String::from_utf8(super::inflate_raw(&packed.blob)?)?;

        assert_eq!(
            text,
            "\u{feff}{2,2,bbbbbbbb-bbbb-4bbb-bbbb-bbbbbbbbbbbb,0,cccccccc-cccc-4ccc-cccc-cccccccccccc,2,0}"
        );
        assert_eq!(packed.plain_bytes, text.len());

        let _ = std::fs::remove_dir_all(root);
        Ok(())
    }

    #[cfg(feature = "research-corpus-tests")]
    #[test]
    #[ignore = "requires external research corpus"]
    fn patches_common_command_metadata_blob_from_lab_xml() -> anyhow::Result<()> {
        let source_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let source = super::MetadataSourceContext::new(source_root);

        let mut active = b"\xEF\xBB\xBF".to_vec();
        active.extend_from_slice(
            br#"{1,
{2,
{1,
{2,becf53b6-3fbc-4c70-822f-4a70b0434353,078a6af8-d22c-4248-9c33-7e90075a3d2c},
{9,
{4,0,{0},"",-1,-1,1,0,""},3,
{0},1,
{0,0,0},0,
{1,cb50f5c0-8013-4262-93a2-f0db379d6b6b},
{"Pattern"},
{3,
{1,0,becf53b6-3fbc-4c70-822f-4a70b0434353},"OldCommand",
{1,"ru","Old synonym"},"Old comment",{1,cb50f5c0-8013-4262-93a2-f0db379d6b6b},3,{0,0,0},0,0},0,0,0}
}
},0}"#,
        );
        let base_blob = deflate_raw(&active)?;
        let xml = r#"
<MetaDataObject xmlns:v8="urn:v8" xmlns:xr="urn:xr">
  <CommonCommand uuid="becf53b6-3fbc-4c70-822f-4a70b0434353">
    <Properties>
      <Name>ДополнительныеСведенияКоманднаяПанель</Name>
      <Synonym>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Дополнительные сведения</v8:content>
        </v8:item>
      </Synonym>
      <Comment/>
      <Group>FormCommandBarImportant</Group>
      <Representation>Picture</Representation>
      <ToolTip>
        <v8:item>
          <v8:lang>ru</v8:lang>
          <v8:content>Дополнительные сведения</v8:content>
        </v8:item>
      </ToolTip>
      <Picture>
        <xr:Ref>CommonPicture.ДополнительныеСведения</xr:Ref>
        <xr:LoadTransparent>false</xr:LoadTransparent>
      </Picture>
      <Shortcut/>
      <IncludeHelpInContents>false</IncludeHelpInContents>
      <CommandParameterType>
        <v8:TypeSet>cfg:DefinedType.ВладелецДополнительныхСведений</v8:TypeSet>
      </CommandParameterType>
      <ParameterUseMode>Single</ParameterUseMode>
      <ModifiesData>false</ModifiesData>
      <OnMainServerUnavalableBehavior>Auto</OnMainServerUnavalableBehavior>
    </Properties>
  </CommonCommand>
</MetaDataObject>
"#
        .as_bytes();

        let packed =
            super::pack_simple_metadata_blob_from_xml_with_source(&base_blob, xml, Some(&source))?;
        let inflated = String::from_utf8(inflate_raw(&packed.blob)?)?;

        assert_eq!(packed.properties.kind, "CommonCommand");
        assert!(inflated.contains("\"ДополнительныеСведенияКоманднаяПанель\""));
        assert!(inflated.contains("{1,\"ru\",\"Дополнительные сведения\"}"));
        assert!(
            inflated.contains("{4,1,{0,a755cb43-492d-4069-9b6a-29b92ebb5b0e},\"\",-1,-1,0,0,\"\"}")
        );
        assert!(inflated.contains("{1,cb50f5c0-8013-4262-93a2-f0db379d6b6b}"));
        assert!(inflated.contains("{\"Pattern\",{\"#\",2da879f6-1141-480b-b647-fdf6698f8aba}}"));

        Ok(())
    }

    /// The writer's own value table spells the same identifier for every
    /// name as the crate's one standard picture table
    /// (`metadata_model::standard_pictures`), so the two cannot drift apart
    /// the way the exporters' two copies did (Untru/ibcmd-rs#413).
    #[test]
    fn std_picture_values_agree_with_the_one_standard_picture_table() {
        for (name, value) in super::STD_PICTURE_VALUES {
            // The seventeen pictures stored by a bare code, not a uuid, are
            // the descriptor compiler's `STANDARD_PICTURE_CODES`.
            if let Some(code) = value
                .strip_prefix("{-")
                .and_then(|rest| rest.strip_suffix('}'))
            {
                let expected = crate::metadata_model::registers::parts::STANDARD_PICTURE_CODES
                    .iter()
                    .find_map(|(candidate, code)| (*candidate == *name).then_some(*code))
                    .unwrap_or_else(|| panic!("StdPicture.{name} has no measured code"));
                assert_eq!(expected, -code.parse::<i64>().unwrap(), "StdPicture.{name}");
                continue;
            }
            let uuid = crate::metadata_model::standard_pictures::standard_picture_uuid(&format!(
                "StdPicture.{name}"
            ))
            .unwrap_or_else(|| panic!("StdPicture.{name} is not in the standard picture table"));
            assert_eq!(*value, format!("{{0,{uuid}}}"), "StdPicture.{name}");
        }
    }

    #[cfg(feature = "research-corpus-tests")]
    #[test]
    #[ignore = "requires external research corpus"]
    fn resolves_metadata_references_from_lab_sources() {
        let source_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("lab")
            .join("ssl_3_1_11_461")
            .join("src")
            .join("ssl");
        let source = MetadataSourceContext::new(source_root);

        for (reference, expected_uuid) in [
            (
                "Role.АдминистраторСистемы",
                "76702e9e-fa7a-4b98-befa-f9b37db2dae0",
            ),
            ("Language.Русский", "db4a9ccb-9ef5-4b3c-8577-b6fe5db1b62e"),
            (
                "CommonAttribute.ОтредактированныеПредопределенныеРеквизиты",
                "141c7b66-d689-4c8e-ace8-b8e1d8c7fbaa",
            ),
            (
                "Constant.АвтоматическиНастраиватьРазрешенияВПрофиляхБезопасности",
                "9893e2d6-f3f8-4d73-bb06-19bf26d216ab",
            ),
            (
                "CommonForm.ФормаОтчета",
                "9d6d77a9-1f55-4162-93a5-14bb3f3febaf",
            ),
            (
                "CommonForm.ФормаНастроекОтчета",
                "6106e958-354c-4211-992c-3c3819e8828e",
            ),
            (
                "CommonForm.ФормаВариантаОтчета",
                "1f55f330-7c0a-4a29-905f-51d664515bc5",
            ),
            (
                "CommonForm.АвтономнаяРабота",
                "1f3057c2-135f-44b2-9f86-34481fbc5596",
            ),
            (
                "ChartOfCharacteristicTypes.ДополнительныеРеквизитыИСведения",
                "1055d15b-8cb5-4ff0-a526-7fd20a08a96c",
            ),
            (
                "ChartOfCharacteristicTypes.ОбъектыАдресацииЗадач",
                "ad083c26-7461-4e94-b524-0174242fbd91",
            ),
            (
                "CommonTemplate.ВидыДокументовУдостоверяющихЛичность",
                "1682d528-87bf-48c5-acf9-57ab654a615a",
            ),
            (
                "CommonTemplate.СтруктураПодчиненности",
                "7a62d031-c340-4b1e-90af-fff697a2e979",
            ),
            (
                "ScheduledJob.ЗагрузкаКурсовВалют",
                "c7ffd8ab-15e9-4cf1-a7fd-d05534dff000",
            ),
            (
                "FunctionalOption.ВыполнятьЗамерыПроизводительности",
                "7f06703e-24cd-4db7-be88-3fbd65e5c252",
            ),
            (
                "FunctionalOptionsParameter.ОбщиеНастройкиУзлов",
                "f9479915-cdee-40d5-ba53-101132aac672",
            ),
            (
                "EventSubscription.ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных",
                "a64b15fa-fc34-43fe-a366-d27c0f1c3df2",
            ),
            (
                "FilterCriterion.СвязанныеДокументы",
                "18bf6916-83cc-41e5-a35b-1489450ae632",
            ),
            (
                "SettingsStorage.ХранилищеВариантовОтчетов",
                "14512818-58b0-44cc-b00d-d37913c57aad",
            ),
            (
                "Enum.ВариантыВажностиЗадачи",
                "c39750ca-e33f-40c2-b830-119423d9a2ae",
            ),
            (
                "Enum.ВариантыОтображенияМеток",
                "fd9177e4-1277-4f77-be3e-07e169fad918",
            ),
            (
                "SessionParameter.АвторизованныйПользователь",
                "5efc4bc4-b711-4620-8d2e-9d947c6cc141",
            ),
            (
                "DefinedType.Пользователь",
                "a72517c3-8c91-4e40-81ac-83c762789e87",
            ),
            (
                "Catalog.РолиИсполнителей",
                "45c0003f-0ed7-4582-b84e-217fdc4ddeaf",
            ),
            (
                "BusinessProcess.Задание",
                "dad11c2e-08fc-4a6b-8829-8be6c64c15fc",
            ),
            (
                "Document.ЭлектронноеПисьмоИсходящее",
                "8f2b8a8e-4cd3-45e3-89ae-1cc4bd0ff30a",
            ),
            (
                "InformationRegister.ДополнительныеСведения",
                "3ad5d8a7-3071-46aa-aebf-306bdb67983b",
            ),
            (
                "Report.БизнесПроцессы",
                "c5f91669-13d8-4f0a-a054-2701078da38a",
            ),
            (
                "Task.ЗадачаИсполнителя",
                "3ad08f4a-6202-4099-b6cc-bc116e6731a0",
            ),
            (
                "StyleItem.ВажнаяНадписьШрифт",
                "fa2a9ef2-00a1-44f4-a82c-6c7288dd62dc",
            ),
            (
                "HTTPService.exchange_dsl_1_0_0_1",
                "c09df096-f9cc-4b2f-a44e-69147339dc8c",
            ),
            (
                "WebService.EnterpriseDataUpload_1_0_1_1",
                "9ad3b432-5b49-44ee-9d8d-83c36458d927",
            ),
            (
                "WebService.RemoteControl",
                "03d08c14-f814-4e12-8f96-020c36cca2bf",
            ),
            (
                "XDTOPackage.АдминистрированиеОбменаДанными_2_4_5_1",
                "ac7ea771-4b10-4d43-9c0a-9cd36e4c49a4",
            ),
            (
                "DocumentJournal.Взаимодействия",
                "7da57c89-af2c-445a-96f7-39250f70306f",
            ),
            (
                "Subsystem.СтандартныеПодсистемы",
                "0421b67e-ed26-491d-ab98-ec59002ed4ce",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
            );
        }

        for (reference, expected_type_id) in [
            (
                "BusinessProcessObject.Задание",
                "4a670c5f-960b-4b36-b587-59bcea4d8449",
            ),
            (
                "BusinessProcessRef.Задание",
                "07d25a98-bdd8-4f7b-b87b-172294158755",
            ),
            (
                "BusinessProcessSelection.Задание",
                "d0447d5c-7808-4532-8a98-0cb3974a90bf",
            ),
            (
                "BusinessProcessList.Задание",
                "9c74798b-2430-4cda-97f2-44472b8d59ac",
            ),
            (
                "BusinessProcessManager.Задание",
                "9f615ee8-8711-4ca9-98d0-f0a258dcdfd2",
            ),
            (
                "BusinessProcessRoutePointRef.Задание",
                "35f39a4f-8a59-4b48-aa38-ef5f2640d375",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "CatalogObject.РолиИсполнителей",
                "ef072c81-fde0-4cf3-a449-5572c679b351",
            ),
            (
                "CatalogRef.РолиИсполнителей",
                "44422b6d-5eb8-49c6-856b-dd9009611933",
            ),
            (
                "CatalogSelection.РолиИсполнителей",
                "7a78b42f-4b88-4938-a59a-f1227ae3e4da",
            ),
            (
                "CatalogList.РолиИсполнителей",
                "26cbb04c-a1eb-45fc-8e62-cc3f010f34cd",
            ),
            (
                "CatalogManager.РолиИсполнителей",
                "28f405d9-0472-418a-a888-838dd917ced7",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "DataProcessorObject.АвтоматическоеИзвлечениеТекстов",
                "5db20d4f-615f-4911-9cd4-45ff4f623dd2",
            ),
            (
                "DataProcessorManager.АвтоматическоеИзвлечениеТекстов",
                "76b2ec7a-4ddb-4d50-aec2-4a1b6bb1b3b9",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "ReportObject.БизнесПроцессы",
                "1d3afecc-d10d-4795-a819-cadc3d5ecd95",
            ),
            (
                "ReportManager.БизнесПроцессы",
                "5a14c1b5-a349-4c68-aee7-d7b6b35e78a4",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "DocumentObject.АктОбУничтоженииПерсональныхДанных",
                "12576083-65c9-4698-a669-bd9dec07cc88",
            ),
            (
                "DocumentRef.АктОбУничтоженииПерсональныхДанных",
                "6851400e-2dbc-4f37-868b-a4683b097408",
            ),
            (
                "DocumentSelection.АктОбУничтоженииПерсональныхДанных",
                "54a7694c-e379-4c71-8e87-e8cd69ac617a",
            ),
            (
                "DocumentList.АктОбУничтоженииПерсональныхДанных",
                "300148b3-5f55-43a5-a9a6-0840f74a0c3e",
            ),
            (
                "DocumentManager.АктОбУничтоженииПерсональныхДанных",
                "d67de8ab-dd38-424a-aaea-753b75c3b7e8",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "ExchangePlanObject.ОбновлениеИнформационнойБазы",
                "0bcfe249-60b0-40fe-bf8e-531749953e91",
            ),
            (
                "ExchangePlanRef.ОбновлениеИнформационнойБазы",
                "4676cf0b-d6fd-4c39-a5f0-43da2d37c210",
            ),
            (
                "ExchangePlanSelection.ОбновлениеИнформационнойБазы",
                "0a64a383-8e2c-435e-a2fa-f8fc69fad418",
            ),
            (
                "ExchangePlanList.ОбновлениеИнформационнойБазы",
                "7559cd4f-0728-442e-9593-02e845cea7fd",
            ),
            (
                "ExchangePlanManager.ОбновлениеИнформационнойБазы",
                "80a1960a-e4be-445e-8446-cfb59885e83e",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "TaskObject.ЗадачаИсполнителя",
                "edccd440-4634-484c-b31d-443ba8674912",
            ),
            (
                "TaskRef.ЗадачаИсполнителя",
                "526f0ebe-d70d-4909-8ae9-86bbabfa55da",
            ),
            (
                "TaskSelection.ЗадачаИсполнителя",
                "a29971a7-e94c-4876-b4de-5ba996cfef0d",
            ),
            (
                "TaskList.ЗадачаИсполнителя",
                "a3849f5f-312a-4950-a52e-ee3f915b5490",
            ),
            (
                "TaskManager.ЗадачаИсполнителя",
                "cb17d3c0-8e58-4bed-aea8-a6cc40c5bd74",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "ChartOfCharacteristicTypesObject.ОбъектыАдресацииЗадач",
                "20630724-4f14-4d31-b479-6b01d7d318e0",
            ),
            (
                "ChartOfCharacteristicTypesRef.ОбъектыАдресацииЗадач",
                "48723115-46af-4d1f-8070-bc9ce5745356",
            ),
            (
                "ChartOfCharacteristicTypesSelection.ОбъектыАдресацииЗадач",
                "e676427d-6d38-4ea2-b363-157ba41d7156",
            ),
            (
                "ChartOfCharacteristicTypesList.ОбъектыАдресацииЗадач",
                "4f89d4d5-72f5-4640-9c43-e6c104c86198",
            ),
            (
                "Characteristic.ОбъектыАдресацииЗадач",
                "6357c29c-abbc-467d-961b-8ccb5be8c151",
            ),
            (
                "ChartOfCharacteristicTypesManager.ОбъектыАдресацииЗадач",
                "5b41e3e3-086e-45b2-b05c-aeae98d85834",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "EnumRef.ВажностьПроблемыУчета",
                "c8f0f421-adcf-417b-8509-93d4569c4435",
            ),
            (
                "EnumManager.ВажностьПроблемыУчета",
                "7b08935e-284b-4995-ae95-f93cc6666d02",
            ),
            (
                "EnumList.ВажностьПроблемыУчета",
                "001333d9-a79f-4306-900e-a56c9e37802f",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [(
            "SettingsStorageManager.ХранилищеВариантовОтчетов",
            "c78f03a8-fbb0-4d73-b78e-23dc1810a05c",
        )] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "InformationRegisterRecord.АдминистративнаяИерархия",
                "b849da51-b14a-4348-87e7-9ba778ef267d",
            ),
            (
                "InformationRegisterManager.АдминистративнаяИерархия",
                "f91e151b-de23-41be-a03d-15c69393c1c3",
            ),
            (
                "InformationRegisterSelection.АдминистративнаяИерархия",
                "2f2b5932-32e0-411b-accc-79d663c5308c",
            ),
            (
                "InformationRegisterList.АдминистративнаяИерархия",
                "b957dd6a-b02c-4096-8a02-4ca35d78a3b3",
            ),
            (
                "InformationRegisterRecordSet.АдминистративнаяИерархия",
                "ef23c5b7-2a2b-4573-a996-b7e9b0c719c9",
            ),
            (
                "InformationRegisterRecordKey.АдминистративнаяИерархия",
                "139107d1-4583-43ef-8b20-283a3074458a",
            ),
            (
                "InformationRegisterRecordManager.АдминистративнаяИерархия",
                "f9f726d6-bd9a-4cf8-bb57-ee742ca0fad4",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_type_id) in [
            (
                "InformationRegisterRecord.АдресныеОбъекты",
                "9ea90ec5-ab70-486e-8cc5-1707d8e5998e",
            ),
            (
                "InformationRegisterManager.АдресныеОбъекты",
                "28648720-16ff-487f-8f79-68a1480055bd",
            ),
            (
                "InformationRegisterSelection.АдресныеОбъекты",
                "f8538bed-cc51-4c92-a18e-7b5a933a1025",
            ),
            (
                "InformationRegisterList.АдресныеОбъекты",
                "aba286a3-0324-4840-92f2-7edc3980054e",
            ),
            (
                "InformationRegisterRecordSet.АдресныеОбъекты",
                "d7d6d986-6bec-45a0-b3ec-e29f286da38c",
            ),
            (
                "InformationRegisterRecordKey.АдресныеОбъекты",
                "273accea-5bfd-4163-9a96-86f4995ef650",
            ),
            (
                "InformationRegisterRecordManager.АдресныеОбъекты",
                "f9342f93-5225-4459-b1fe-d2553b85a5af",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }

        for (reference, expected_uuid) in [
            (
                "CommonPicture.Предупреждение",
                "ac2e5217-aaeb-4b6f-b063-538c84f2da06",
            ),
            (
                "CommonPicture.Взаимодействия",
                "44cf6d0a-0a5b-4ca1-b91e-af61f40fb825",
            ),
        ] {
            assert_eq!(
                source.resolve_common_picture_uuid(reference).unwrap(),
                expected_uuid
            );
        }

        for (reference, expected_uuid) in [
            (
                "CommonCommand.АвтономнаяРабота",
                "75ffd0b9-79be-4600-a310-591fddb6d63e",
            ),
            (
                "CommandGroup.Органайзер",
                "c59e11f3-6bcb-404a-9d76-1416c12be354",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
            );
        }

        for (reference, expected_uuid) in [
            (
                "CommandGroup.Взаимодействия",
                "e4842271-4fc0-4e15-afef-876f05af78c0",
            ),
            (
                "CommandGroup.Информация",
                "31ee6430-b65d-42fa-859b-c4f1c40686ae",
            ),
            (
                "CommandGroup.Органайзер",
                "c59e11f3-6bcb-404a-9d76-1416c12be354",
            ),
        ] {
            assert_eq!(
                source.resolve_command_group_uuid(reference).unwrap(),
                expected_uuid
            );
        }
    }

    #[test]
    fn resolves_additional_metadata_type_ids_from_synthetic_sources() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-module-types-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("Catalogs")).unwrap();
        std::fs::create_dir_all(root.join("AccumulationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("AccountingRegisters")).unwrap();
        std::fs::create_dir_all(root.join("CalculationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfAccounts")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCharacteristicTypes")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationTypes")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("BusinessProcesses")).unwrap();
        std::fs::create_dir_all(root.join("DataProcessors")).unwrap();
        std::fs::create_dir_all(root.join("Documents")).unwrap();
        std::fs::create_dir_all(root.join("ExchangePlans")).unwrap();
        std::fs::create_dir_all(root.join("Enums")).unwrap();
        std::fs::create_dir_all(root.join("InformationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("Reports")).unwrap();

        std::fs::write(
            root.join("Catalogs/РолиИсполнителей.Товары.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Catalog uuid="20202020-2020-4202-8202-202020202020">
    <Properties>
      <Name>РолиИсполнителей</Name>
      <GeneratedTypes>
        <GeneratedType name="CatalogTabularSection.РолиИсполнителей.Товары">
          <TypeId>21212121-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Catalog>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Catalogs/РолиИсполнителей.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Catalog uuid="20202020-2020-4202-8202-202020202020">
    <Properties>
      <Name>РолиИсполнителей</Name>
      <GeneratedTypes>
        <GeneratedType name="CatalogRef.РолиИсполнителей">
          <TypeId>22222222-2222-4222-8222-222222222222</TypeId>
        </GeneratedType>
        <GeneratedType name="CatalogSelection.РолиИсполнителей">
          <TypeId>23232323-2323-4232-8232-232323232323</TypeId>
        </GeneratedType>
        <GeneratedType name="CatalogList.РолиИсполнителей">
          <TypeId>24242424-2424-4242-8242-242424242424</TypeId>
        </GeneratedType>
        <GeneratedType name="CatalogManager.РолиИсполнителей">
          <TypeId>25252525-2525-4252-8252-252525252525</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Catalog>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccumulationRegisters/Продажи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccumulationRegister uuid="11111111-1111-4111-8111-111111111111">
    <Properties>
      <Name>Продажи</Name>
      <GeneratedTypes>
        <GeneratedType name="AccumulationRegisterObject.Продажи">
          <TypeId>aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </AccumulationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccountingRegisters/Хозрасчеты.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccountingRegister uuid="22222222-2222-4222-8222-222222222222">
    <Properties>
      <Name>Хозрасчеты</Name>
      <GeneratedTypes>
        <GeneratedType name="AccountingRegisterManager.Хозрасчеты">
          <TypeId>bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </AccountingRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CalculationRegisters/Премии.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <CalculationRegister uuid="33333333-3333-4333-8333-333333333333">
    <Properties>
      <Name>Премии</Name>
      <GeneratedTypes>
        <GeneratedType name="CalculationRegisterList.Премии">
          <TypeId>cccccccc-cccc-4ccc-8ccc-cccccccccccc</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </CalculationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfAccounts uuid="44444444-4444-4444-8444-444444444444">
    <Properties>
      <Name>ПланСчетов</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfAccountsObject.ПланСчетов">
          <TypeId>dddddddd-dddd-4ddd-8ddd-dddddddddddd</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfAccounts>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCharacteristicTypes/ОбъектыАдресацииЗадач.Товары.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCharacteristicTypes uuid="1b1b1b1b-1b1b-41b1-81b1-1b1b1b1b1b1b">
    <Properties>
      <Name>ОбъектыАдресацииЗадач</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfCharacteristicTypesTabularSection.ОбъектыАдресацииЗадач.Товары">
          <TypeId>1c1c1c1c-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
        <GeneratedType name="ChartOfCharacteristicTypesTabularSectionRow.ОбъектыАдресацииЗадач.Товары">
          <TypeId>1d1d1d1d-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfCharacteristicTypes>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationTypes/ВидыРасчета.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationTypes uuid="55555555-5555-4555-8555-555555555555">
    <Properties>
      <Name>ВидыРасчета</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfCalculationTypesSelection.ВидыРасчета">
          <TypeId>eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee</TypeId>
        </GeneratedType>
        <GeneratedType name="ChartOfCalculationTypesObject.ВидыРасчета">
          <TypeId>eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeea</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfCalculationTypes>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationRegisters/Начисления.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationRegisters uuid="66666666-6666-4666-8666-666666666666">
    <Properties>
      <Name>Начисления</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfCalculationRegistersManager.Начисления">
          <TypeId>ffffffff-ffff-4fff-8fff-ffffffffffff</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfCalculationRegisters>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("BusinessProcesses/Задание.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <BusinessProcess uuid="15151515-1515-4151-8151-151515151515">
    <Properties>
      <Name>Задание</Name>
      <GeneratedTypes>
        <GeneratedType name="BusinessProcessRef.Задание">
          <TypeId>11111111-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </BusinessProcess>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("DataProcessors/АвтоматическоеИзвлечениеТекстов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <DataProcessor uuid="18181818-1818-4181-8181-181818181818">
    <Properties>
      <Name>АвтоматическоеИзвлечениеТекстов</Name>
      <GeneratedTypes>
        <GeneratedType name="DataProcessorManager.АвтоматическоеИзвлечениеТекстов">
          <TypeId>22222222-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </DataProcessor>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Documents/АктОбУничтоженииПерсональныхДанных.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Document uuid="14141414-1414-4141-8141-141414141414">
    <Properties>
      <Name>АктОбУничтоженииПерсональныхДанных</Name>
      <GeneratedTypes>
        <GeneratedType name="DocumentRef.АктОбУничтоженииПерсональныхДанных">
          <TypeId>26262626-2626-4262-8262-262626262626</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentSelection.АктОбУничтоженииПерсональныхДанных">
          <TypeId>27272727-2727-4272-8272-272727272727</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentList.АктОбУничтоженииПерсональныхДанных">
          <TypeId>28282828-2828-4282-8282-282828282828</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentManager.АктОбУничтоженииПерсональныхДанных">
          <TypeId>29292929-2929-4292-8292-292929292929</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentTabularSection.АктОбУничтоженииПерсональныхДанных.Товары">
          <TypeId>1e1e1e1e-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentObject.АктОбУничтоженииПерсональныхДанных">
          <TypeId>33333333-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Document>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Documents/АктОбУничтоженииПерсональныхДанных.Товары.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Document uuid="14141414-1414-4141-8141-141414141414">
    <Properties>
      <Name>АктОбУничтоженииПерсональныхДанных</Name>
      <GeneratedTypes>
        <GeneratedType name="DocumentTabularSection.АктОбУничтоженииПерсональныхДанных.Товары">
          <TypeId>1e1e1e1e-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
        <GeneratedType name="DocumentObject.АктОбУничтоженииПерсональныхДанных">
          <TypeId>33333333-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Document>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Enums/ВариантыВажностиЗадачи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Enum uuid="19191919-1919-4191-8191-191919191919">
    <Properties>
      <Name>ВариантыВажностиЗадачи</Name>
      <GeneratedTypes>
        <GeneratedType name="EnumRef.ВариантыВажностиЗадачи">
          <TypeId>30303030-3030-4303-8303-303030303030</TypeId>
        </GeneratedType>
        <GeneratedType name="EnumList.ВариантыВажностиЗадачи">
          <TypeId>31313131-3131-4313-8313-313131313131</TypeId>
        </GeneratedType>
        <GeneratedType name="EnumManager.ВариантыВажностиЗадачи">
          <TypeId>32323232-3232-4323-8323-323232323232</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Enum>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("InformationRegisters/АдминистративнаяИерархия.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <InformationRegister uuid="33333333-3333-4333-8333-333333333334">
    <Properties>
      <Name>АдминистративнаяИерархия</Name>
      <GeneratedTypes>
        <GeneratedType name="InformationRegisterRecord.АдминистративнаяИерархия">
          <TypeId>33333333-3333-4333-8333-333333333331</TypeId>
        </GeneratedType>
        <GeneratedType name="InformationRegisterRecordSet.АдминистративнаяИерархия">
          <TypeId>33333333-3333-4333-8333-333333333332</TypeId>
        </GeneratedType>
        <GeneratedType name="InformationRegisterRecordKey.АдминистративнаяИерархия">
          <TypeId>33333333-3333-4333-8333-333333333333</TypeId>
        </GeneratedType>
        <GeneratedType name="InformationRegisterRecordManager.АдминистративнаяИерархия">
          <TypeId>33333333-3333-4333-8333-333333333335</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </InformationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ExchangePlans/ОбновлениеИнформационнойБазы.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ExchangePlan uuid="16161616-1616-4161-8161-161616161616">
    <Properties>
      <Name>ОбновлениеИнформационнойБазы</Name>
      <GeneratedTypes>
        <GeneratedType name="ExchangePlanObject.ОбновлениеИнформационнойБазы">
          <TypeId>44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ExchangePlan>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Reports/БизнесПроцессы.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Report uuid="17171717-1717-4171-8171-171717171717">
    <Properties>
      <Name>БизнесПроцессы</Name>
      <GeneratedTypes>
        <GeneratedType name="ReportObject.БизнесПроцессы">
          <TypeId>55555555-aaaa-4aaa-8aaa-aaaaaaaaaaaa</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </Report>
</MetaDataObject>
"#,
        )
        .unwrap();

        let source = MetadataSourceContext::new(root);
        for (reference, expected_type_id) in [
            (
                "AccumulationRegisterObject.Продажи",
                "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "AccountingRegisterManager.Хозрасчеты",
                "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            ),
            (
                "CalculationRegisterList.Премии",
                "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
            ),
            (
                "ChartOfAccountsObject.ПланСчетов",
                "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            ),
            (
                "ChartOfCalculationTypesSelection.ВидыРасчета",
                "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
            ),
            (
                "ChartOfCalculationTypesObject.ВидыРасчета",
                "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeea",
            ),
            (
                "ChartOfCalculationRegistersManager.Начисления",
                "ffffffff-ffff-4fff-8fff-ffffffffffff",
            ),
            (
                "CatalogTabularSection.РолиИсполнителей.Товары",
                "21212121-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CatalogRef.РолиИсполнителей",
                "22222222-2222-4222-8222-222222222222",
            ),
            (
                "CatalogSelection.РолиИсполнителей",
                "23232323-2323-4232-8232-232323232323",
            ),
            (
                "CatalogList.РолиИсполнителей",
                "24242424-2424-4242-8242-242424242424",
            ),
            (
                "CatalogManager.РолиИсполнителей",
                "25252525-2525-4252-8252-252525252525",
            ),
            (
                "ChartOfCharacteristicTypesTabularSection.ОбъектыАдресацииЗадач.Товары",
                "1c1c1c1c-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ChartOfCharacteristicTypesTabularSectionRow.ОбъектыАдресацииЗадач.Товары",
                "1d1d1d1d-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "BusinessProcessRef.Задание",
                "11111111-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DataProcessorManager.АвтоматическоеИзвлечениеТекстов",
                "22222222-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DocumentObject.АктОбУничтоженииПерсональныхДанных",
                "33333333-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DocumentTabularSection.АктОбУничтоженииПерсональныхДанных.Товары",
                "1e1e1e1e-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DocumentRef.АктОбУничтоженииПерсональныхДанных",
                "26262626-2626-4262-8262-262626262626",
            ),
            (
                "DocumentSelection.АктОбУничтоженииПерсональныхДанных",
                "27272727-2727-4272-8272-272727272727",
            ),
            (
                "DocumentList.АктОбУничтоженииПерсональныхДанных",
                "28282828-2828-4282-8282-282828282828",
            ),
            (
                "DocumentManager.АктОбУничтоженииПерсональныхДанных",
                "29292929-2929-4292-8292-292929292929",
            ),
            (
                "ExchangePlanObject.ОбновлениеИнформационнойБазы",
                "44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "EnumRef.ВариантыВажностиЗадачи",
                "30303030-3030-4303-8303-303030303030",
            ),
            (
                "EnumList.ВариантыВажностиЗадачи",
                "31313131-3131-4313-8313-313131313131",
            ),
            (
                "EnumManager.ВариантыВажностиЗадачи",
                "32323232-3232-4323-8323-323232323232",
            ),
            (
                "InformationRegisterRecord.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333331",
            ),
            (
                "InformationRegisterRecordSet.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333332",
            ),
            (
                "InformationRegisterRecordKey.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333333",
            ),
            (
                "InformationRegisterRecordManager.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333335",
            ),
            (
                "ReportObject.БизнесПроцессы",
                "55555555-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_type_id(reference).unwrap(),
                expected_type_id
            );
        }
    }

    #[test]
    fn resolves_additional_metadata_references_from_synthetic_sources() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-module-refs-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);
        std::fs::create_dir_all(root.join("AccumulationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("AccountingRegisters")).unwrap();
        std::fs::create_dir_all(root.join("CalculationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("Catalogs")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfAccounts")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationTypes")).unwrap();
        std::fs::create_dir_all(root.join("ChartsOfCalculationRegisters")).unwrap();
        std::fs::create_dir_all(root.join("Documents")).unwrap();
        std::fs::create_dir_all(root.join("Enums")).unwrap();
        std::fs::create_dir_all(root.join("InformationRegisters")).unwrap();

        std::fs::write(
            root.join("AccumulationRegisters/Продажи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccumulationRegister uuid="11111111-1111-4111-8111-111111111111">
    <Properties>
      <Name>Продажи</Name>
    </Properties>
  </AccumulationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("AccountingRegisters/Хозрасчеты.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <AccountingRegister uuid="22222222-2222-4222-8222-222222222222">
    <Properties>
      <Name>Хозрасчеты</Name>
    </Properties>
  </AccountingRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Catalogs/РолиИсполнителей.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Catalog uuid="20202020-2020-4202-8202-202020202020">
    <Properties>
      <Name>РолиИсполнителей</Name>
    </Properties>
  </Catalog>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("CalculationRegisters/Премии.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <CalculationRegister uuid="33333333-3333-4333-8333-333333333333">
    <Properties>
      <Name>Премии</Name>
    </Properties>
  </CalculationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfAccounts/ПланСчетов.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfAccounts uuid="44444444-4444-4444-8444-444444444444">
    <Properties>
      <Name>ПланСчетов</Name>
    </Properties>
  </ChartOfAccounts>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationTypes/ВидыРасчета.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationTypes uuid="55555555-5555-4555-8555-555555555555">
    <Properties>
      <Name>ВидыРасчета</Name>
    </Properties>
  </ChartOfCalculationTypes>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("ChartsOfCalculationRegisters/Начисления.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <ChartOfCalculationRegisters uuid="66666666-6666-4666-8666-666666666666">
    <Properties>
      <Name>Начисления</Name>
      <GeneratedTypes>
        <GeneratedType name="ChartOfCalculationRegistersManager.Начисления">
          <TypeId>ffffffff-ffff-4fff-8fff-ffffffffffff</TypeId>
        </GeneratedType>
      </GeneratedTypes>
    </Properties>
  </ChartOfCalculationRegisters>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Documents/АктОбУничтоженииПерсональныхДанных.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Document uuid="14141414-1414-4141-8141-141414141414">
    <Properties>
      <Name>АктОбУничтоженииПерсональныхДанных</Name>
    </Properties>
  </Document>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("Enums/ВариантыВажностиЗадачи.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <Enum uuid="19191919-1919-4191-8191-191919191919">
    <Properties>
      <Name>ВариантыВажностиЗадачи</Name>
    </Properties>
  </Enum>
</MetaDataObject>
"#,
        )
        .unwrap();
        std::fs::write(
            root.join("InformationRegisters/АдминистративнаяИерархия.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <InformationRegister uuid="33333333-3333-4333-8333-333333333334">
    <Properties>
      <Name>АдминистративнаяИерархия</Name>
    </Properties>
  </InformationRegister>
</MetaDataObject>
"#,
        )
        .unwrap();

        let source = MetadataSourceContext::new(root);
        for (reference, expected_uuid) in [
            (
                "AccumulationRegister.Продажи",
                "11111111-1111-4111-8111-111111111111",
            ),
            (
                "AccountingRegister.Хозрасчеты",
                "22222222-2222-4222-8222-222222222222",
            ),
            (
                "CalculationRegister.Премии",
                "33333333-3333-4333-8333-333333333333",
            ),
            (
                "Catalog.РолиИсполнителей",
                "20202020-2020-4202-8202-202020202020",
            ),
            (
                "ChartOfAccounts.ПланСчетов",
                "44444444-4444-4444-8444-444444444444",
            ),
            (
                "ChartOfCalculationTypes.ВидыРасчета",
                "55555555-5555-4555-8555-555555555555",
            ),
            (
                "ChartOfCalculationRegisters.Начисления",
                "66666666-6666-4666-8666-666666666666",
            ),
            (
                "Document.АктОбУничтоженииПерсональныхДанных",
                "14141414-1414-4141-8141-141414141414",
            ),
            (
                "Enum.ВариантыВажностиЗадачи",
                "19191919-1919-4191-8191-191919191919",
            ),
            (
                "InformationRegister.АдминистративнаяИерархия",
                "33333333-3333-4333-8333-333333333334",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
            );
        }
    }

    #[test]
    fn resolves_simple_metadata_references_from_synthetic_sources() {
        let root = std::env::temp_dir().join(format!(
            "ibcmd-rs-simple-refs-{}",
            uuid::Uuid::new_v4().hyphenated()
        ));
        let _scratch = crate::test_scratch::TestScratch::adopt(&root);

        let cases = [
            (
                "CommonAttributes",
                "ОтредактированныеПредопределенныеРеквизиты",
                "CommonAttribute",
                "41414141-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CommonForms",
                "ФормаОтчета",
                "CommonForm",
                "42424242-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CommonTemplates",
                "СтруктураПодчиненности",
                "CommonTemplate",
                "43434343-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "EventSubscriptions",
                "ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных",
                "EventSubscription",
                "44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "FunctionalOptions",
                "ВыполнятьЗамерыПроизводительности",
                "FunctionalOption",
                "45454545-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "FunctionalOptionsParameters",
                "ОбщиеНастройкиУзлов",
                "FunctionalOptionsParameter",
                "46464646-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "HTTPServices",
                "exchange_dsl_1_0_0_1",
                "HTTPService",
                "47474747-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Languages",
                "Русский",
                "Language",
                "48484848-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Roles",
                "АдминистраторСистемы",
                "Role",
                "49494949-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ScheduledJobs",
                "ЗагрузкаКурсовВалют",
                "ScheduledJob",
                "4a4a4a4a-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "SettingsStorages",
                "ХранилищеВариантовОтчетов",
                "SettingsStorage",
                "4b4b4b4b-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "StyleItems",
                "ВажнаяНадписьШрифт",
                "StyleItem",
                "4c4c4c4c-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Subsystems",
                "СтандартныеПодсистемы",
                "Subsystem",
                "4d4d4d4d-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Tasks",
                "ЗадачаИсполнителя",
                "Task",
                "4e4e4e4e-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "WebServices",
                "RemoteControl",
                "WebService",
                "4f4f4f4f-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "XDTOPackages",
                "АдминистрированиеОбменаДанными_2_4_5_1",
                "XDTOPackage",
                "50505050-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "BusinessProcesses",
                "Задание",
                "BusinessProcess",
                "51515151-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DataProcessors",
                "АвтоматическоеИзвлечениеТекстов",
                "DataProcessor",
                "52525252-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ExchangePlans",
                "ОбновлениеИнформационнойБазы",
                "ExchangePlan",
                "53535353-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Reports",
                "БизнесПроцессы",
                "Report",
                "54545454-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
        ];

        for (folder, name, kind, uuid) in cases {
            std::fs::create_dir_all(root.join(folder)).unwrap();
            std::fs::write(
                root.join(folder).join(format!("{name}.xml")),
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20">
  <{kind} uuid="{uuid}">
    <Properties>
      <Name>{name}</Name>
    </Properties>
  </{kind}>
</MetaDataObject>
"#
                ),
            )
            .unwrap();
        }

        let source = MetadataSourceContext::new(root);
        for (reference, expected_uuid) in [
            (
                "CommonAttribute.ОтредактированныеПредопределенныеРеквизиты",
                "41414141-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CommonForm.ФормаОтчета",
                "42424242-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "CommonTemplate.СтруктураПодчиненности",
                "43434343-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "EventSubscription.ВариантыОтчетовПередУдалениемИдентификатораОбъектаМетаданных",
                "44444444-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "FunctionalOption.ВыполнятьЗамерыПроизводительности",
                "45454545-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "FunctionalOptionsParameter.ОбщиеНастройкиУзлов",
                "46464646-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "HTTPService.exchange_dsl_1_0_0_1",
                "47474747-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            ("Language.Русский", "48484848-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
            (
                "Role.АдминистраторСистемы",
                "49494949-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ScheduledJob.ЗагрузкаКурсовВалют",
                "4a4a4a4a-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "SettingsStorage.ХранилищеВариантовОтчетов",
                "4b4b4b4b-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "StyleItem.ВажнаяНадписьШрифт",
                "4c4c4c4c-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Subsystem.СтандартныеПодсистемы",
                "4d4d4d4d-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Task.ЗадачаИсполнителя",
                "4e4e4e4e-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "WebService.RemoteControl",
                "4f4f4f4f-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "XDTOPackage.АдминистрированиеОбменаДанными_2_4_5_1",
                "50505050-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "BusinessProcess.Задание",
                "51515151-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "DataProcessor.АвтоматическоеИзвлечениеТекстов",
                "52525252-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "ExchangePlan.ОбновлениеИнформационнойБазы",
                "53535353-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
            (
                "Report.БизнесПроцессы",
                "54545454-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ),
        ] {
            assert_eq!(
                source.resolve_metadata_reference_uuid(reference).unwrap(),
                expected_uuid
            );
        }
    }
}

#[cfg(test)]
mod root_read_only_tests {
    use super::*;

    #[test]
    fn root_read_only_round_trips_through_fresh_and_overlay_compilers() -> anyhow::Result<()> {
        for dialect in ["2.20", "2.21"] {
            for value in [false, true] {
                let xml = format!(
                    r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="{dialect}"><ReadOnly>{value}</ReadOnly><AutoCommandBar name="FormCommandBar" id="-1"/></Form>"#
                );
                let packed = pack_native_form_body_blob(xml.as_bytes(), None, None, None)?;
                let parsed = parse_form_body_blob(&packed.blob)?;
                let fields = scan_braced_fields(&parsed.layout, 0)?;
                assert_eq!(
                    &parsed.layout[fields[16].clone()],
                    if value { "1" } else { "0" }
                );
                let flipped_xml = xml.replace(&format!(">{value}<"), &format!(">{}<", !value));
                let patched =
                    pack_form_body_blob_from_form_xml(&packed.blob, flipped_xml.as_bytes(), None)?;
                let patched = parse_form_body_blob(&patched.blob)?;
                let patched_fields = scan_braced_fields(&patched.layout, 0)?;
                assert_eq!(
                    &patched.layout[patched_fields[16].clone()],
                    if value { "0" } else { "1" }
                );
                // A patch that does not name ReadOnly keeps the original flag.
                let absent = format!(
                    r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="{dialect}"><AutoCommandBar name="FormCommandBar" id="-1"/></Form>"#
                );
                let unchanged =
                    pack_form_body_blob_from_form_xml(&packed.blob, absent.as_bytes(), None)?;
                let unchanged = parse_form_body_blob(&unchanged.blob)?;
                let unchanged_fields = scan_braced_fields(&unchanged.layout, 0)?;
                assert_eq!(
                    &unchanged.layout[unchanged_fields[16].clone()],
                    if value { "1" } else { "0" }
                );
            }
        }
        let invalid = br#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="2.20"><ReadOnly>maybe</ReadOnly></Form>"#;
        assert!(pack_native_form_body_blob(invalid, None, None, None).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod chart_field_form_scope_tests {
    use super::*;

    #[test]
    fn form_scope_chart_field_compiles_independent_stretch_flags() -> anyhow::Result<()> {
        for dialect in ["2.20", "2.21"] {
            for (horizontal, vertical) in
                [(true, false), (false, true), (false, false), (true, true)]
            {
                let xml = format!(
                    r#"<Form xmlns="http://v8.1c.ru/8.3/xcf/logform" version="{dialect}"><AutoCommandBar name="FormCommandBar" id="-1"/><ChildItems><ChartField name="Chart" id="1"><Width>32</Width><Height>5</Height><HorizontalStretch>{horizontal}</HorizontalStretch><VerticalStretch>{vertical}</VerticalStretch><ContextMenu name="ChartContextMenu" id="2"/><ExtendedTooltip name="ChartTooltip" id="3"/></ChartField></ChildItems></Form>"#
                );
                let body = compile_native_form_body(xml.as_bytes(), None, None, None)?;
                let normalized = body.replace(['\r', '\n'], "");
                assert!(
                    normalized.contains(&format!(
                        "{{1,32,5,{},{},{{0,1,0}},1,0,0,1,0}}",
                        u8::from(horizontal),
                        u8::from(vertical)
                    )),
                    "{normalized}"
                );
            }
        }
        Ok(())
    }
}
