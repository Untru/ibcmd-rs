//! The empty ExternalDataSource native layout, both directions. The canonical
//! XML adapter admits namespaces/identities; the schema owns physical slots.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow, bail};
use ibcmd_core::artifact::ProfileId;
use ibcmd_core::diagnostic::ObjectPath;
use ibcmd_core::identity::ObjectUuid;
use ibcmd_schema::external_data_source as schema;
use ibcmd_xml::{XmlReader, decode_empty_external_data_source, metadata::MetadataEnvelope};

use super::brace::{Brace, NIL_UUID, parse_row};
use super::common::Header;
use super::export::values::{attribute_header, header, header_elements, type_element};
use super::export::{
    Build, GeneratedTypeName, NameIndex, ObjectNames, atom, el, item, leaf, list, write_document,
};
use super::xml::Element;
use super::{DescriptorContext, ObjectXml};
use ibcmd_restructure::brace_list;
use ibcmd_schema::external_data_source_children as child_schema;

#[derive(Debug)]
struct Record {
    head: Header,
    generated: [(String, String); 3],
    lock_mode: i64,
    cubes: Vec<String>,
    tables: Vec<String>,
    functions: Vec<Brace>,
}

/// Exact original source admission precedes the legacy local-name DOM.
pub(crate) fn validate_source(xml: &[u8], version: &str) -> Result<MetadataEnvelope> {
    let document = XmlReader::from_slice(xml)?;
    Ok(decode_empty_external_data_source(
        &document,
        ProfileId::parse(&format!("xml-{version}"))?,
        ObjectPath::root(),
    )?)
}

fn uuid(value: &str) -> Result<String> {
    let parsed = ObjectUuid::parse(value)?;
    if parsed.as_bytes().iter().all(|byte| *byte == 0) {
        bail!("ExternalDataSource identity is nil");
    }
    Ok(parsed.to_string())
}

pub(crate) fn compile(object: &ObjectXml<'_>, context: &DescriptorContext) -> Result<Brace> {
    // Decoded host-owned DOMs have the established writer's own bindings.
    // Original source compilation instead retains its admitted envelope and
    // calls compile_admitted directly, without losing inherited namespaces.
    let bytes = write_document(object.element, &context.version);
    let document = XmlReader::from_slice(bytes.as_bytes())?;
    let canonical = decode_empty_external_data_source(
        &document,
        ProfileId::parse(&format!("xml-{}", context.version))?,
        ObjectPath::root(),
    )?;
    compile_admitted(object, &canonical)
}

/// Encode the same canonical envelope admitted from the original source.
/// Prefix bindings belong to that envelope, not the local-name object subtree.
pub(crate) fn compile_admitted(
    object: &ObjectXml<'_>,
    canonical: &MetadataEnvelope,
) -> Result<Brace> {
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
    if object
        .child_objects()
        .is_some_and(|children| !children.children.is_empty())
    {
        bail!("ExternalDataSource children are exported but not compiled yet");
    }
    Ok(Record {
        head,
        generated,
        lock_mode,
        cubes: Vec::new(),
        tables: Vec::new(),
        functions: Vec::new(),
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
        let mut cubes = Vec::new();
        let mut tables = Vec::new();
        let mut functions = Vec::new();
        let mut seen_collections = BTreeSet::new();
        for collection in &outer[3..] {
            let fields = list(collection)?;
            if fields.len() < 2 {
                bail!("ExternalDataSource short child collection");
            }
            if !seen_collections.insert(atom(&fields[0])?) {
                bail!("ExternalDataSource duplicate child collection");
            }
            let count = atom(&fields[1])?
                .parse::<usize>()
                .map_err(|_| anyhow!("ExternalDataSource child count"))?;
            let end = 2usize
                .checked_add(count)
                .ok_or_else(|| anyhow!("ExternalDataSource child count overflows"))?;
            if fields.len() != end {
                bail!("ExternalDataSource child count does not match members");
            }
            let members = &fields[2..];
            match atom(&fields[0])? {
                id if id == child_schema::ROOT_CUBES_COLLECTION => {
                    cubes = uuid_members(members)?;
                }
                id if id == child_schema::ROOT_TABLES_COLLECTION => {
                    tables = uuid_members(members)?;
                }
                id if id == child_schema::ROOT_FORMULAS_COLLECTION => {
                    functions = members.to_vec();
                }
                other => bail!("ExternalDataSource unknown child collection {other}"),
            }
        }
        Ok(Self {
            head,
            generated,
            lock_mode,
            cubes,
            tables,
            functions,
        })
    }

    fn element(&self, names: &NameIndex) -> Result<Element> {
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
        Ok(el(schema::KIND)
            .attr("uuid", &self.head.uuid)
            .child(internal)
            .child(el("Properties").children(fields).child(leaf(
                "DataLockControlMode",
                schema::data_lock_name(&self.lock_mode.to_string()).expect("validated mode"),
            )))
            .child(self.child_objects(names)?))
    }

    fn child_objects(&self, names: &NameIndex) -> Result<Element> {
        let mut objects = el("ChildObjects");
        for uuid in &self.tables {
            objects
                .children
                .push(leaf("Table", short_name(names, uuid)?));
        }
        for uuid in &self.cubes {
            objects
                .children
                .push(leaf("Cube", short_name(names, uuid)?));
        }
        for function in &self.functions {
            objects.children.push(function_element(function, names)?);
        }
        Ok(objects)
    }
}

fn uuid_members(members: &[Brace]) -> Result<Vec<String>> {
    members
        .iter()
        .map(|member| Ok(atom(member)?.to_string()))
        .collect()
}

fn short_name(names: &NameIndex, uuid: &str) -> Result<String> {
    let full = names
        .name(uuid)
        .ok_or_else(|| anyhow!("ExternalDataSource has no name for child {uuid}"))?;
    full.rsplit('.')
        .next()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("empty child name"))
}

/// Inline `{0,{27,...},<ReturnValue>,<tail>}` function of the root.
fn function_element(node: &Brace, names: &NameIndex) -> Result<Element> {
    let wrapper = list(node)?;
    if wrapper.len() != 2 || atom(item(wrapper, 1)?)? != "0" {
        bail!("ExternalDataSource invalid Function collection item");
    }
    let record = list(item(wrapper, 0)?)?;
    if record.len() != 4 || atom(item(record, 0)?)? != "0" {
        bail!("ExternalDataSource invalid Function wrapper");
    }
    let body = item(record, 1)?;
    let head = attribute_header(body)?;
    let typed = list(item(list(body)?, 1)?)?;
    let [name, synonym, comment] = header_elements(&head)?;
    let tail = list(item(record, 3)?)?;
    if tail.first().and_then(Brace::as_atom) != Some("0")
        || tail
            .get(1)
            .and_then(Brace::as_str)
            .is_some_and(|text| !text.is_empty())
        || tail
            .get(2)
            .and_then(Brace::as_str)
            .is_some_and(|text| !text.is_empty())
    {
        bail!("ExternalDataSource non-empty ExpressionInDataSource is not supported yet");
    }
    Ok(el("Function").attr("uuid", &head.uuid).child(
        el("Properties")
            .child(name)
            .child(synonym)
            .child(comment)
            .child(leaf(
                "ReturnValue",
                super::export::values::bool_text(item(record, 2)?)?,
            ))
            .child(type_element("Type", item(typed, 2)?, names)?)
            .child(el("ExpressionInDataSource")),
    ))
}

#[doc(hidden)]
pub fn decode(row: &Brace, names: &NameIndex) -> Result<Element> {
    Record::from_brace(row)?.element(names)
}

#[doc(hidden)]
pub fn names(row: &Brace) -> Result<ObjectNames> {
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

#[doc(hidden)]
pub fn parse(text: &str) -> Result<Brace> {
    ibcmd_compiler_families::compiler::families::native::validate_descriptor_layout(
        text.as_bytes(),
        schema::MAX_VALUE_DEPTH,
    )?;
    let row = parse_row(text.as_bytes())?;
    Record::from_brace(&row)?;
    Ok(row)
}

/// A complete, validated EDS never enters code2's SettingsStorage or
/// ScheduledJob classification. Unsupported recognized shapes return no kind.
#[doc(hidden)]
pub fn source_family(text: &str, expected_uuid: &str) -> Option<(&'static str, &'static str)> {
    // A non-empty root carries an inline function body deeper than the empty
    // family's `MAX_VALUE_DEPTH`; `from_brace` is the admission here.
    let row = parse_row(text.as_bytes()).ok()?;
    let found = names(&row).ok()?;
    (found.uuid == expected_uuid).then_some((schema::KIND, schema::COLLECTION))
}

/// The exact object name and complete canonical XML, from one parsed row.
#[doc(hidden)]
pub fn export_source(text: &str, expected_uuid: &str, version: &str) -> Result<(String, String)> {
    let row = parse(text)?;
    let record = Record::from_brace(&row)?;
    if record.head.uuid != expected_uuid {
        bail!("ExternalDataSource row identity differs");
    }
    if !record.cubes.is_empty() || !record.tables.is_empty() || !record.functions.is_empty() {
        bail!("ExternalDataSource children require the model exporter");
    }
    let result = write_document(&record.element(&NameIndex::default())?, version);
    validate_source(result.as_bytes(), version)?;
    Ok((record.head.name, result))
}
