//! ExternalDataSource child descriptors (`Cube`, `Table`, `DimensionTable`):
//! native row -> Designer XML.
//!
//! The native body layouts and generated-type slots are measured on the target
//! matrix configuration and on the stepwise `_ВнешнийИсточникДанных` corpus.
//! Property values that are not yet decoded are written as the platform's
//! defaults only after the stored row was checked against the measured shape;
//! an unknown shape is refused instead of silently rewritten.

use anyhow::{Result, anyhow, bail};
use ibcmd_schema::external_data_source_children as schema;

use super::attribute::{CHOICE_HISTORY_ON_INPUT, CREATE_ON_INPUT, FILL_CHECKING, QUICK_CHOICE};
use super::brace::{Brace, parse_row};
use super::export::values::{
    Header, Owner, attribute_properties, bool_text, code_text, header, header_elements,
    localized_element, reference_name, type_element, value_element,
};
use super::export::{
    Build, ExportContext, GeneratedTypeName, NameIndex, ObjectNames, atom, el, item, leaf, list,
    string, xml_text,
};
use super::xml::Element;

/// The owner payload of a child descriptor: `{<code>,<header>,<types...>,...}`.
struct Row<'a> {
    root: &'a [Brace],
    payload: &'a [Brace],
}

impl<'a> Row<'a> {
    fn parse(row: &'a Brace, kind: &str) -> Result<Self> {
        let root = list(row)?;
        if atom(item(root, 0)?)? != "1" {
            bail!("{kind}: not a descriptor row");
        }
        let payload = list(item(root, 1)?)?;
        if payload.len() != schema::body_arity(kind)
            || atom(item(payload, 0)?)? != schema::record_code(kind)
        {
            bail!("{kind}: unexpected body layout");
        }
        Ok(Self { root, payload })
    }

    fn header(&self, kind: &str) -> Result<Header> {
        let wrapper = list(item(self.payload, 1)?)?;
        let head = list(item(wrapper, 1)?)?;
        let _ = kind;
        header(&Brace::List(head.to_vec()))
    }

    fn optional_items(&self, collection: &str) -> Result<Option<&'a [Brace]>> {
        let count = usize::try_from(super::export::number(item(
            self.root,
            schema::ROW_COLLECTION_COUNT_SLOT,
        )?)?)
        .map_err(|_| anyhow!("collection count does not fit"))?;
        if self.root.len().checked_sub(schema::ROW_COLLECTIONS_START) != Some(count) {
            bail!("descriptor collection count does not match its members");
        }
        let mut found = None;
        let mut seen = std::collections::BTreeSet::new();
        for node in &self.root[schema::ROW_COLLECTIONS_START..] {
            let fields = list(node)?;
            let id = atom(item(fields, 0)?)?;
            if !seen.insert(id) {
                bail!("duplicate {id} collection");
            }
            let count = usize::try_from(super::export::number(item(
                fields,
                schema::COLLECTION_COUNT_SLOT,
            )?)?)
            .map_err(|_| anyhow!("item count does not fit"))?;
            let end = schema::COLLECTION_ITEMS_START
                .checked_add(count)
                .ok_or_else(|| anyhow!("{id} count overflows"))?;
            if fields.len() != end {
                bail!("{id} count does not match its members");
            }
            // A non-inline collection (forms, templates, commands) holds its
            // members as separate descriptor rows; being non-empty is the
            // normal case, not content this row must render. Only a collection
            // the schema does not know at all is refused.
            schema::collection_is_rendered(parsed_kind(self.payload)?, id)
                .ok_or_else(|| anyhow!("unknown {id} child collection"))?;
            if id == collection {
                found = Some(&fields[schema::COLLECTION_ITEMS_START..]);
            }
        }
        Ok(found)
    }

    fn items(&self, collection: &str) -> Result<&'a [Brace]> {
        self.optional_items(collection)?
            .ok_or_else(|| anyhow!("no {collection} collection"))
    }
}

/// The path segments of an EDS child: root, cube and (for a dimension table)
/// the dimension-table name.
struct Path {
    root: String,
    cube: Option<String>,
    table: Option<String>,
    dimension: Option<String>,
}

impl Path {
    fn from_full(kind: &str, full: &str) -> Result<Self> {
        let parts = full.split('.').collect::<Vec<_>>();
        let expected = match kind {
            schema::CUBE_KIND => 4,
            schema::TABLE_KIND => 4,
            schema::DIMENSION_TABLE_KIND => 6,
            other => bail!("unknown EDS child kind {other}"),
        };
        if parts.len() != expected || parts.first().copied() != Some("ExternalDataSource") {
            bail!("{kind}: unexpected full name {full}");
        }
        Ok(match kind {
            schema::CUBE_KIND => Self {
                root: parts[1].to_string(),
                cube: Some(parts[3].to_string()),
                table: None,
                dimension: None,
            },
            schema::TABLE_KIND => Self {
                root: parts[1].to_string(),
                cube: None,
                table: Some(parts[3].to_string()),
                dimension: None,
            },
            schema::DIMENSION_TABLE_KIND => {
                if parts[2] != schema::CUBE_KIND || parts[4] != schema::DIMENSION_TABLE_KIND {
                    bail!("{kind}: unexpected full name {full}");
                }
                Self {
                    root: parts[1].to_string(),
                    cube: Some(parts[3].to_string()),
                    table: None,
                    dimension: Some(parts[5].to_string()),
                }
            }
            _ => unreachable!(),
        })
    }

    fn generated_name(&self, kind: &str, stem: &str) -> Result<String> {
        Ok(match kind {
            schema::CUBE_KIND => format!(
                "{stem}.{}.{}",
                self.root,
                self.cube.as_deref().ok_or_else(|| anyhow!("cube name"))?
            ),
            schema::TABLE_KIND => format!(
                "{stem}.{}.{}",
                self.root,
                self.table.as_deref().ok_or_else(|| anyhow!("table name"))?
            ),
            schema::DIMENSION_TABLE_KIND => format!(
                "{stem}.{}.{}.{}",
                self.root,
                self.cube.as_deref().ok_or_else(|| anyhow!("cube name"))?,
                self.dimension
                    .as_deref()
                    .ok_or_else(|| anyhow!("dimension-table name"))?
            ),
            other => bail!("unknown EDS child kind {other}"),
        })
    }
}

fn full_name(names: &NameIndex, uuid: &str) -> Result<String> {
    names
        .name(uuid)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("no name for EDS child {uuid}"))
}

fn short_name(names: &NameIndex, uuid: &str) -> Result<String> {
    let full = full_name(names, uuid)?;
    full.rsplit('.')
        .next()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("empty name"))
}

fn generated_type(name: String, category: &str, type_id: &str, value_id: &str) -> Element {
    el("xr:GeneratedType")
        .attr("name", name)
        .attr("category", category)
        .child(leaf("xr:TypeId", type_id))
        .child(leaf("xr:ValueId", value_id))
}

fn internal_info(kind: &str, path: &Path, payload: &[Brace]) -> Result<Element> {
    let mut info = el("InternalInfo");
    for category in schema::categories(kind) {
        let type_id = atom(item(payload, category.body_slot)?)?;
        let value_id = atom(item(payload, category.body_slot + 1)?)?;
        info.children.push(generated_type(
            path.generated_name(kind, category.name_stem)?,
            category.category,
            type_id,
            value_id,
        ));
    }
    Ok(info)
}

fn bool_element(name: &str, node: &Brace) -> Result<Element> {
    Ok(leaf(name, bool_text(node)?))
}

/// Cube properties, in Designer order, from the slots measured on the matrix.
fn cube_properties(row: &Row<'_>, head: &Header, context: &ExportContext) -> Result<Element> {
    let p = row.payload;
    let [name, synonym, comment] = header_elements(head)?;
    check_row_shape(row, "Cube")?;
    Ok(el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(leaf(
            "NameInDataSource",
            xml_text(string(item(p, schema::CUBE_NAME_IN_DATA_SOURCE_SLOT)?)?),
        ))
        .child(characteristics(
            p,
            schema::CUBE_CHARACTERISTICS_SLOT,
            &context.names,
        )?)
        .child(bool_element(
            "UseStandardCommands",
            item(p, schema::CUBE_USE_STANDARD_COMMANDS_SLOT)?,
        )?)
        .child(form_reference(
            p,
            schema::CUBE_DEFAULT_RECORD_FORM_SLOT,
            "DefaultRecordForm",
            &context.names,
        )?)
        .child(form_reference(
            p,
            schema::CUBE_DEFAULT_LIST_FORM_SLOT,
            "DefaultListForm",
            &context.names,
        )?)
        .child(localized(
            p,
            schema::CUBE_RECORD_PRESENTATION_SLOT,
            "RecordPresentation",
        )?)
        .child(localized(
            p,
            schema::CUBE_EXTENDED_RECORD_PRESENTATION_SLOT,
            "ExtendedRecordPresentation",
        )?)
        .child(localized(
            p,
            schema::CUBE_LIST_PRESENTATION_SLOT,
            "ListPresentation",
        )?)
        .child(localized(
            p,
            schema::CUBE_EXTENDED_LIST_PRESENTATION_SLOT,
            "ExtendedListPresentation",
        )?)
        .child(localized(p, schema::CUBE_EXPLANATION_SLOT, "Explanation")?)
        .child(bool_element(
            "IncludeHelpInContents",
            item(p, schema::CUBE_INCLUDE_HELP_IN_CONTENTS_SLOT)?,
        )?))
}

/// Table properties, in Designer order, from the slots measured on the matrix.
fn table_properties(
    row: &Row<'_>,
    head: &Header,
    full: &str,
    context: &ExportContext,
) -> Result<Element> {
    let p = row.payload;
    let [name, synonym, comment] = header_elements(head)?;
    check_row_shape(row, "Table")?;
    let table_type = code_text(
        item(p, schema::TABLE_TYPE_SLOT)?,
        &[("Table", 0), ("Expression", 1)],
    )?;
    // The stored code is the *inverse* of the published name: `1` is the plain
    // table, `0` the object-data one (the matrix measures `1`; the stepwise
    // corpus and the branch row carry it the same way).
    let table_data_type = code_text(
        item(p, schema::TABLE_TABLE_DATA_TYPE_SLOT)?,
        &[("NonobjectData", 1), ("ObjectData", 0)],
    )?;
    let (key_fields, input_by_string) = table_ref_lists(row)?;
    let modes = list(item(p, schema::TABLE_SEARCH_MODES_SLOT)?)?;
    if modes.len() != 3 || atom(item(modes, 1)?)? != "2" {
        bail!("Table: invalid input-by-string mode bag");
    }
    let search_mode = code_text(item(modes, 0)?, &[("Begin", 1), ("AnyPart", 2)])?;
    let choice_data_mode = code_text(item(modes, 2)?, &[("Directly", 0), ("Background", 1)])?;
    Ok(el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(leaf("TableType", table_type))
        .child(leaf(
            "NameInDataSource",
            xml_text(string(item(p, schema::TABLE_NAME_IN_DATA_SOURCE_SLOT)?)?),
        ))
        .child(expression_in_data_source(
            p,
            schema::TABLE_EXPRESSION_IN_DATA_SOURCE_SLOT,
        )?)
        .child(leaf("TableDataType", table_data_type))
        .child(el("KeyFields").children(key_field_elements(full, &key_fields)))
        .child(leaf(
            "PresentationField",
            reference_name(
                atom(item(p, schema::TABLE_PRESENTATION_FIELD_SLOT)?)?,
                &context.names,
            )?,
        ))
        .child(leaf(
            "ParentField",
            reference_name(
                atom(item(p, schema::TABLE_PARENT_FIELD_SLOT)?)?,
                &context.names,
            )?,
        ))
        .child(unfilled_parent_value(item(
            p,
            schema::TABLE_UNFILLED_PARENT_VALUE_SLOT,
        )?)?)
        .child(characteristics(
            p,
            schema::TABLE_CHARACTERISTICS_SLOT,
            &context.names,
        )?)
        .child(bool_element(
            "UseStandardCommands",
            item(p, schema::TABLE_USE_STANDARD_COMMANDS_SLOT)?,
        )?)
        .child(bool_element(
            "QuickChoice",
            item(p, schema::TABLE_QUICK_CHOICE_SLOT)?,
        )?)
        .child(el("InputByString").children(key_field_elements(full, &input_by_string)))
        .child(leaf(
            "CreateOnInput",
            code_text(
                item(p, schema::TABLE_CREATE_ON_INPUT_SLOT)?,
                &[("Auto", 0), ("DontUse", 1), ("Use", 2)],
            )?,
        ))
        .child(leaf("SearchStringModeOnInputByString", search_mode))
        .child(leaf("ChoiceDataGetModeOnInputByString", choice_data_mode))
        .child(leaf(
            "ChoiceHistoryOnInput",
            code_text(
                item(p, schema::TABLE_CHOICE_HISTORY_ON_INPUT_SLOT)?,
                &[("Auto", 0), ("DontUse", 1)],
            )?,
        ))
        .child(form_reference(
            p,
            schema::TABLE_DEFAULT_OBJECT_FORM_SLOT,
            "DefaultObjectForm",
            &context.names,
        )?)
        .child(form_reference(
            p,
            schema::TABLE_DEFAULT_RECORD_FORM_SLOT,
            "DefaultRecordForm",
            &context.names,
        )?)
        .child(form_reference(
            p,
            schema::TABLE_DEFAULT_LIST_FORM_SLOT,
            "DefaultListForm",
            &context.names,
        )?)
        .child(form_reference(
            p,
            schema::TABLE_DEFAULT_CHOICE_FORM_SLOT,
            "DefaultChoiceForm",
            &context.names,
        )?)
        .child(localized(
            p,
            schema::TABLE_OBJECT_PRESENTATION_SLOT,
            "ObjectPresentation",
        )?)
        .child(localized(
            p,
            schema::TABLE_EXTENDED_OBJECT_PRESENTATION_SLOT,
            "ExtendedObjectPresentation",
        )?)
        .child(localized(
            p,
            schema::TABLE_RECORD_PRESENTATION_SLOT,
            "RecordPresentation",
        )?)
        .child(localized(
            p,
            schema::TABLE_EXTENDED_RECORD_PRESENTATION_SLOT,
            "ExtendedRecordPresentation",
        )?)
        .child(localized(
            p,
            schema::TABLE_LIST_PRESENTATION_SLOT,
            "ListPresentation",
        )?)
        .child(localized(
            p,
            schema::TABLE_EXTENDED_LIST_PRESENTATION_SLOT,
            "ExtendedListPresentation",
        )?)
        .child(localized(p, schema::TABLE_EXPLANATION_SLOT, "Explanation")?)
        .child(bool_element(
            "IncludeHelpInContents",
            item(p, schema::TABLE_INCLUDE_HELP_IN_CONTENTS_SLOT)?,
        )?)
        .child(bool_element(
            "ReadOnly",
            item(p, schema::TABLE_READ_ONLY_SLOT)?,
        )?)
        .child(leaf(
            "TransactionsIsolationLevel",
            code_text(
                item(p, schema::TABLE_TRANSACTIONS_ISOLATION_LEVEL_SLOT)?,
                &[
                    ("Auto", 0),
                    ("ReadUncommitted", 1),
                    ("ReadCommitted", 2),
                    ("RepeatableRead", 3),
                    ("Serializable", 4),
                ],
            )?,
        ))
        .child(leaf(
            "DataVersionField",
            reference_name(
                atom(item(p, schema::TABLE_DATA_VERSION_FIELD_SLOT)?)?,
                &context.names,
            )?,
        ))
        .child(leaf(
            "EditType",
            code_text(
                item(p, schema::TABLE_EDIT_TYPE_SLOT)?,
                &[("InList", 0), ("InDialog", 1), ("BothWays", 2)],
            )?,
        ))
        .child(based_on(p, schema::TABLE_BASED_ON_SLOT, &context.names)?)
        .child(el("DataLockFields").children(key_field_elements(
            full,
            &field_list_names(row, schema::TABLE_DATA_LOCK_FIELDS_SLOT)?,
        )))
        .child(leaf(
            "DataLockControlMode",
            code_text(
                item(p, schema::TABLE_DATA_LOCK_CONTROL_MODE_SLOT)?,
                &[("Automatic", 0), ("Managed", 1), ("AutomaticAndManaged", 2)],
            )?,
        )))
}

/// `{0,source,presentation,{0,0,0}}` -> `<ExpressionInDataSource>`.
fn expression_in_data_source(p: &[Brace], slot: usize) -> Result<Element> {
    let bag = list(item(p, slot)?)?;
    if bag.len() != 4 || atom(item(bag, 0)?)? != "0" {
        bail!("Table: invalid ExpressionInDataSource bag");
    }
    Ok(leaf(
        "ExpressionInDataSource",
        xml_text(string(item(bag, 1)?)?),
    ))
}

/// `{0,N,{"#",<ref>,{1,<uuid>}}...}` -> `<BasedOn>` with object references.
fn based_on(p: &[Brace], slot: usize, names: &NameIndex) -> Result<Element> {
    let outer = list(item(p, slot)?)?;
    if atom(item(outer, 0)?)? != "0" {
        bail!("BasedOn: unexpected wrapper");
    }
    let count = usize::try_from(super::export::number(item(outer, 1)?)?)
        .map_err(|_| anyhow!("BasedOn: count does not fit"))?;
    if outer.len() != count + 2 {
        bail!("BasedOn: count does not match its members");
    }
    let mut element = el("BasedOn");
    for entry in &outer[2..] {
        let fields = list(entry)?;
        if string(item(fields, 0)?)? != "#" {
            bail!("BasedOn: not an object reference");
        }
        let typed = list(item(fields, 2)?)?;
        if typed.len() != 2 || atom(item(typed, 0)?)? != "1" {
            bail!("BasedOn: not a typed reference");
        }
        element.children.push(
            leaf("xr:Item", reference_name(atom(item(typed, 1)?)?, names)?)
                .attr("type", "xr:MDObjectRef"),
        );
    }
    Ok(element)
}

/// `{0,{1,{"#",<class>,{4,<types from>,<values from>,...}}}}` -> `<Characteristics>`.
///
/// The matrix measures the all-unset characteristic; its field order beyond the
/// two `from` references is only partly recovered, so any assigned field is
/// refused rather than written into the wrong element.
fn characteristics(p: &[Brace], slot: usize, names: &NameIndex) -> Result<Element> {
    let outer = list(item(p, slot)?)?;
    if atom(item(outer, 0)?)? != "0" {
        bail!("Characteristics: unexpected wrapper");
    }
    let items = list(item(outer, 1)?)?;
    let count = usize::try_from(super::export::number(item(items, 0)?)?)
        .map_err(|_| anyhow!("Characteristics: count does not fit"))?;
    if items.len() != count + 1 {
        bail!("Characteristics: count does not match its members");
    }
    let mut element = el("Characteristics");
    for entry in &items[1..] {
        let fields = list(entry)?;
        if string(item(fields, 0)?)? != "#" {
            bail!("Characteristics: not a characteristic record");
        }
        let record = list(item(fields, 2)?)?;
        if record.len() != 13 || atom(item(record, 0)?)? != "4" {
            bail!("Characteristics: unmeasured characteristic layout");
        }
        let reference = |index: usize| -> Result<String> {
            let wrapper = list(item(record, index)?)?;
            if wrapper.len() != 2 || atom(item(wrapper, 0)?)? != "1" {
                bail!("Characteristics: not a reference wrapper");
            }
            reference_name(atom(item(wrapper, 1)?)?, names)
        };
        let number = |index: usize| -> Result<i64> {
            let wrapper = list(item(record, index)?)?;
            if wrapper.len() != 3 || atom(item(wrapper, 0)?)? != "1" {
                bail!("Characteristics: not a field-reference wrapper");
            }
            super::export::number(item(list(item(wrapper, 1)?)?, 0)?)
        };
        let undefined = |index: usize| -> Result<bool> {
            let wrapper = list(item(record, index)?)?;
            Ok(wrapper.len() == 1
                && (wrapper[0].as_atom() == Some("U") || wrapper[0].as_str() == Some("U")))
        };
        // Field references the matrix leaves unset. A set one would need the
        // stored order of these slots, which no corpus has measured yet.
        let key_field = number(3)?;
        let types_filter_field = number(4)?;
        let data_path_field = number(5)?;
        let multiple_values_use_field = number(6)?;
        let types_filter_value_unset = undefined(7)?;
        let object_field = number(8)?;
        let type_field = number(9)?;
        let value_field = number(10)?;
        let multiple_values_key_field = number(11)?;
        let multiple_values_order_field = number(12)?;
        for assigned in [
            key_field,
            types_filter_field,
            data_path_field,
            multiple_values_use_field,
            object_field,
            type_field,
            value_field,
            multiple_values_key_field,
            multiple_values_order_field,
        ] {
            if assigned != -1 {
                bail!("Characteristics: assigned field reference {assigned} is not measured");
            }
        }
        if !types_filter_value_unset {
            bail!("Characteristics: assigned types filter value is not measured");
        }
        element.children.push(
            el("xr:Characteristic")
                .child(
                    el("xr:CharacteristicTypes")
                        .attr("from", reference(1)?)
                        .child(leaf("xr:KeyField", key_field.to_string()))
                        .child(leaf("xr:TypesFilterField", types_filter_field.to_string()))
                        .child(el("xr:TypesFilterValue").attr("nil", "true"))
                        .child(leaf("xr:DataPathField", data_path_field.to_string()))
                        .child(leaf(
                            "xr:MultipleValuesUseField",
                            multiple_values_use_field.to_string(),
                        )),
                )
                .child(
                    el("xr:CharacteristicValues")
                        .attr("from", reference(2)?)
                        .child(leaf("xr:ObjectField", object_field.to_string()))
                        .child(leaf("xr:TypeField", type_field.to_string()))
                        .child(leaf("xr:ValueField", value_field.to_string()))
                        .child(leaf(
                            "xr:MultipleValuesKeyField",
                            multiple_values_key_field.to_string(),
                        ))
                        .child(leaf(
                            "xr:MultipleValuesOrderField",
                            multiple_values_order_field.to_string(),
                        )),
                ),
        );
    }
    Ok(element)
}

/// A localized string property (`{N,"lang","text",...}`).
fn localized(p: &[Brace], slot: usize, name: &str) -> Result<Element> {
    localized_element(name, item(p, slot)?)
}

/// A form reference property: nil uuid becomes an empty element.
fn form_reference(p: &[Brace], slot: usize, name: &str, names: &NameIndex) -> Result<Element> {
    Ok(leaf(name, reference_name(atom(item(p, slot)?)?, names)?))
}

/// `{"U"}` -> `xsi:nil`; `{"S","A"}` -> `xsi:type="xs:string"` with the text.
fn unfilled_parent_value(node: &Brace) -> Result<Element> {
    let wrapper = list(node)?;
    match (wrapper.first().and_then(Brace::as_str), wrapper.get(1)) {
        (Some("U"), None) => Ok(el("UnfilledParentValue").attr("nil", "true")),
        (Some("S"), Some(text)) => {
            Ok(leaf("UnfilledParentValue", xml_text(string(text)?)).attr("type", "xs:string"))
        }
        _ => bail!("unfilled parent value: unmeasured layout"),
    }
}

fn key_field_elements(full: &str, fields: &[String]) -> Vec<Element> {
    fields
        .iter()
        .map(|name| leaf("xr:Field", format!("{full}.Field.{name}")))
        .collect()
}

/// `KeyFields` and `InputByString` as field names, in stored order.
fn table_ref_lists(row: &Row<'_>) -> Result<(Vec<String>, Vec<String>)> {
    let names = field_names(row)?;
    let resolve = |ids: Vec<String>| -> Result<Vec<String>> {
        ids.into_iter()
            .map(|id| {
                names
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| anyhow!("Table: typed list names unknown field {id}"))
            })
            .collect()
    };
    let key_fields = resolve(typed_ref_list(
        item(row.payload, schema::TABLE_KEY_FIELDS_SLOT)?,
        schema::KEY_FIELD_REF_TYPE,
        "1",
    )?)?;
    let input = list(item(row.payload, schema::TABLE_INPUT_BY_STRING_SLOT)?)?;
    if input.len() != 2 || atom(item(input, 0)?)? != "1" {
        bail!("Table: invalid InputByString wrapper");
    }
    let input_by_string = resolve(typed_ref_list(
        item(input, schema::INPUT_BY_STRING_ITEMS_SLOT)?,
        schema::INPUT_BY_STRING_REF_TYPE,
        "0",
    )?)?;
    Ok((key_fields, input_by_string))
}

/// A `{1,{0,N,{"#",<ref>,{0,<field>}}...}}` field list as field names.
fn field_list_names(row: &Row<'_>, slot: usize) -> Result<Vec<String>> {
    let names = field_names(row)?;
    let wrapper = list(item(row.payload, slot)?)?;
    if wrapper.len() != 2 || atom(item(wrapper, 0)?)? != "1" {
        bail!("Table: invalid field-list wrapper at slot {slot}");
    }
    typed_ref_list(
        item(wrapper, schema::INPUT_BY_STRING_ITEMS_SLOT)?,
        schema::INPUT_BY_STRING_REF_TYPE,
        "0",
    )?
    .into_iter()
    .map(|id| {
        names
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow!("Table: field list names unknown field {id}"))
    })
    .collect()
}

fn field_names(row: &Row<'_>) -> Result<std::collections::BTreeMap<String, String>> {
    let mut names = std::collections::BTreeMap::new();
    for entry in row
        .optional_items(schema::FIELDS_COLLECTION)?
        .unwrap_or_default()
    {
        let wrapper = list(entry)?;
        let record = list(item(wrapper, 0)?)?;
        let body = item(record, 1)?;
        let typed = list(item(list(body)?, 1)?)?;
        let head = header(item(typed, 1)?)?;
        names.insert(head.uuid, head.name);
    }
    Ok(names)
}

/// `{0,<count>,{"#",<type>,{<kind>,<uuid>}}…}` -> field UUIDs of `type`.
fn typed_ref_list(node: &Brace, type_id: &str, reference_kind: &str) -> Result<Vec<String>> {
    let fields = list(node)?;
    if atom(item(fields, 0)?)? != "0" {
        bail!("invalid typed-reference list marker");
    }
    let count = usize::try_from(super::export::number(item(
        fields,
        schema::COLLECTION_COUNT_SLOT,
    )?)?)
    .map_err(|_| anyhow!("typed-reference count does not fit"))?;
    let end = schema::COLLECTION_ITEMS_START
        .checked_add(count)
        .ok_or_else(|| anyhow!("typed-reference count overflows"))?;
    if fields.len() != end {
        bail!("typed-reference count does not match members");
    }
    fields[schema::COLLECTION_ITEMS_START..]
        .iter()
        .map(|entry| {
            let typed = list(entry)?;
            if typed.len() != 3
                || string(item(typed, 0)?)? != "#"
                || atom(item(typed, 1)?)? != type_id
            {
                bail!("unexpected typed-reference member");
            }
            let inner = list(item(typed, 2)?)?;
            if inner.len() != 2 || atom(item(inner, 0)?)? != reference_kind {
                bail!("unexpected typed-reference identity");
            }
            Ok(atom(item(inner, 1)?)?.to_owned())
        })
        .collect()
}

/// DimensionTable properties, in Designer order, from the slots measured on
/// the matrix.
fn dimension_properties(row: &Row<'_>, head: &Header, context: &ExportContext) -> Result<Element> {
    let p = row.payload;
    let [name, synonym, comment] = header_elements(head)?;
    check_row_shape(row, "DimensionTable")?;
    // The stored text is what the platform publishes: `-1` is kept as the
    // unsigned maximum rather than rescaled in the XML.
    let level_text = atom(item(p, schema::DIMENSION_LEVEL_NUMBER_SLOT)?)?;
    if level_text.parse::<u64>().is_err() {
        bail!("DimensionTable: unmeasured LevelNumber {level_text}");
    }
    Ok(el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(leaf(
            "NameInDataSource",
            xml_text(string(item(
                p,
                schema::DIMENSION_NAME_IN_DATA_SOURCE_SLOT,
            )?)?),
        ))
        .child(leaf(
            "PresentationField",
            reference_name(
                atom(item(p, schema::DIMENSION_PRESENTATION_FIELD_SLOT)?)?,
                &context.names,
            )?,
        ))
        .child(leaf(
            "HierarchyNameInDataSource",
            xml_text(string(item(
                p,
                schema::DIMENSION_HIERARCHY_NAME_IN_DATA_SOURCE_SLOT,
            )?)?),
        ))
        .child(leaf("LevelNumber", level_text.to_string()))
        .child(bool_element(
            "Hierarchical",
            item(p, schema::DIMENSION_HIERARCHICAL_SLOT)?,
        )?)
        .child(unfilled_parent_value(item(
            p,
            schema::DIMENSION_UNFILLED_PARENT_VALUE_SLOT,
        )?)?)
        .child(bool_element(
            "UseStandardCommands",
            item(p, schema::DIMENSION_USE_STANDARD_COMMANDS_SLOT)?,
        )?)
        .child(bool_element(
            "QuickChoice",
            item(p, schema::DIMENSION_QUICK_CHOICE_SLOT)?,
        )?)
        .child(form_reference(
            p,
            schema::DIMENSION_DEFAULT_OBJECT_FORM_SLOT,
            "DefaultObjectForm",
            &context.names,
        )?)
        .child(form_reference(
            p,
            schema::DIMENSION_DEFAULT_LIST_FORM_SLOT,
            "DefaultListForm",
            &context.names,
        )?)
        .child(form_reference(
            p,
            schema::DIMENSION_DEFAULT_CHOICE_FORM_SLOT,
            "DefaultChoiceForm",
            &context.names,
        )?)
        .child(localized(
            p,
            schema::DIMENSION_OBJECT_PRESENTATION_SLOT,
            "ObjectPresentation",
        )?)
        .child(localized(
            p,
            schema::DIMENSION_EXTENDED_OBJECT_PRESENTATION_SLOT,
            "ExtendedObjectPresentation",
        )?)
        .child(localized(
            p,
            schema::DIMENSION_LIST_PRESENTATION_SLOT,
            "ListPresentation",
        )?)
        .child(localized(
            p,
            schema::DIMENSION_EXTENDED_LIST_PRESENTATION_SLOT,
            "ExtendedListPresentation",
        )?)
        .child(localized(
            p,
            schema::DIMENSION_EXPLANATION_SLOT,
            "Explanation",
        )?)
        .child(bool_element(
            "IncludeHelpInContents",
            item(p, schema::DIMENSION_INCLUDE_HELP_IN_CONTENTS_SLOT)?,
        )?))
}

/// Refuse a row whose un-decoded property slots are not the measured defaults.
/// This keeps the exporter from silently publishing a default where the source
/// held something else until the slot is evidenced. A kind with every property
/// decoded (the cube) has no defaults to guard.
fn check_row_shape(row: &Row<'_>, kind: &str) -> Result<()> {
    for expected in schema::unmodeled_defaults(kind) {
        let wrapped = parse_row(expected.wrapped_value.as_bytes())?;
        let expected_value = item(list(&wrapped)?, 0)?;
        if item(row.payload, expected.body_slot)? != expected_value {
            bail!(
                "{kind}: unmodeled property at slot {} changed",
                expected.body_slot
            );
        }
    }
    // A malformed optional collection must not become an absent collection.
    let _ = row.optional_items(schema::FIELDS_COLLECTION)?;
    Ok(())
}

/// `{27,{2,<header>,<pattern>},...}` -> `<Properties>` for an inline Dimension.
fn inline_dimension_properties(
    record: &[Brace],
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let body = item(record, 1)?;
    attribute_properties(body, true, Vec::new(), owner, context)
}

/// Cube `resources` collection item: `{0,{27,...},"<NameInDataSource>"}`,
/// written as the platform's inline `<Resource>`.
fn inline_resource_properties(
    record: &[Brace],
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let body = item(record, 1)?;
    let slots = list(body)?;
    if atom(item(slots, 0)?)? != "27" || slots.len() != 23 {
        bail!("Resource: not an attribute body");
    }
    let typed = list(item(slots, 1)?)?;
    let head = header(item(typed, 1)?)?;
    let [name, synonym, comment] = header_elements(&head)?;
    let names = &context.names;
    Ok(el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(type_element("Type", item(typed, 2)?, names)?)
        .child(bool_element("PasswordMode", item(slots, 2)?)?)
        .child(localized_element("Format", item(slots, 3)?)?)
        .child(localized_element("EditFormat", item(slots, 18)?)?)
        .child(localized_element("ToolTip", item(slots, 4)?)?)
        .child(bool_element("MarkNegatives", item(slots, 5)?)?)
        .child(leaf("Mask", xml_text(string(item(slots, 6)?)?)))
        .child(bool_element("MultiLine", item(slots, 7)?)?)
        .child(super::export::values::choice_parameter_links_element(
            "ChoiceParameterLinks",
            item(slots, 14)?,
            owner,
            names,
        )?)
        .child(super::export::values::choice_parameters_element(
            "ChoiceParameters",
            item(slots, 16)?,
            names,
        )?)
        .child(leaf(
            "QuickChoice",
            code_text(item(slots, 12)?, QUICK_CHOICE)?,
        ))
        .child(leaf(
            "ChoiceForm",
            reference_name(atom(item(slots, 11)?)?, names)?,
        ))
        .child(bool_element("ExtendedEdit", item(slots, 17)?)?)
        .child(leaf(
            "NameInDataSource",
            xml_text(string(item(record, 2)?)?),
        )))
}

/// Table/DimensionTable `fields` collection item:
/// `{1,{27,...},"<NameInDataSource>",<ReadOnly>,<AllowNull>}`.
fn field_properties(
    record: &[Brace],
    owner: Owner<'_>,
    context: &ExportContext,
) -> Result<Element> {
    let body = item(record, 1)?;
    let slots = list(body)?;
    if atom(item(slots, 0)?)? != "27" || slots.len() != 23 {
        bail!("Field: not an attribute body");
    }
    let typed = list(item(slots, 1)?)?;
    let head = header(item(typed, 1)?)?;
    let [name, synonym, comment] = header_elements(&head)?;
    let names = &context.names;
    Ok(el("Properties")
        .child(name)
        .child(synonym)
        .child(comment)
        .child(type_element("Type", item(typed, 2)?, names)?)
        .child(bool_element("PasswordMode", item(slots, 2)?)?)
        .child(localized_element("Format", item(slots, 3)?)?)
        .child(localized_element("EditFormat", item(slots, 18)?)?)
        .child(localized_element("ToolTip", item(slots, 4)?)?)
        .child(bool_element("MarkNegatives", item(slots, 5)?)?)
        .child(leaf("Mask", xml_text(string(item(slots, 6)?)?)))
        .child(bool_element("MultiLine", item(slots, 7)?)?)
        .child(bool_element("ExtendedEdit", item(slots, 17)?)?)
        .child(value_element("MinValue", item(slots, 8)?, names)?)
        .child(value_element("MaxValue", item(slots, 9)?, names)?)
        .child(bool_element("FillFromFillingValue", item(slots, 20)?)?)
        .child(value_element("FillValue", item(slots, 19)?, names)?)
        .child(leaf(
            "FillChecking",
            code_text(item(slots, 13)?, FILL_CHECKING)?,
        ))
        .child(super::export::values::choice_parameter_links_element(
            "ChoiceParameterLinks",
            item(slots, 14)?,
            owner,
            names,
        )?)
        .child(super::export::values::choice_parameters_element(
            "ChoiceParameters",
            item(slots, 16)?,
            names,
        )?)
        .child(leaf(
            "QuickChoice",
            code_text(item(slots, 12)?, QUICK_CHOICE)?,
        ))
        .child(leaf(
            "CreateOnInput",
            code_text(item(slots, 21)?, CREATE_ON_INPUT)?,
        ))
        .child(leaf(
            "ChoiceHistoryOnInput",
            code_text(item(slots, 22)?, CHOICE_HISTORY_ON_INPUT)?,
        ))
        .child(leaf(
            "ChoiceForm",
            reference_name(atom(item(slots, 11)?)?, names)?,
        ))
        .child(leaf(
            "NameInDataSource",
            xml_text(string(item(record, 2)?)?),
        ))
        .child(bool_element("ReadOnly", item(record, 3)?)?)
        .child(bool_element("AllowNull", item(record, 4)?)?))
}

/// Decode one child row into its XML element.
#[doc(hidden)]
pub fn decode_row(kind: &str, row: &Brace, context: &ExportContext) -> Result<Element> {
    let parsed = Row::parse(row, kind)?;
    let head = parsed.header(kind)?;
    let full = full_name(&context.names, &head.uuid)?;
    let path = Path::from_full(kind, &full)?;
    let internal = internal_info(kind, &path, parsed.payload)?;
    let properties = match kind {
        schema::CUBE_KIND => cube_properties(&parsed, &head, context)?,
        schema::TABLE_KIND => table_properties(&parsed, &head, &full, context)?,
        schema::DIMENSION_TABLE_KIND => dimension_properties(&parsed, &head, context)?,
        other => bail!("unknown EDS child kind {other}"),
    };
    let children = child_objects(kind, &parsed, &head, &full, context)?;
    Ok(el(kind)
        .attr("uuid", &head.uuid)
        .child(internal)
        .child(properties)
        .child(children))
}

fn child_objects(
    kind: &str,
    row: &Row<'_>,
    head: &Header,
    full: &str,
    context: &ExportContext,
) -> Result<Element> {
    let mut objects = el("ChildObjects");
    let owner = Owner {
        kind,
        full_name: full,
    };
    match kind {
        schema::CUBE_KIND => {
            if let Some(entries) = row.optional_items(schema::CUBE_DIMENSION_TABLES_COLLECTION)? {
                for entry in entries {
                    objects.children.push(leaf(
                        "DimensionTable",
                        short_name(&context.names, atom(entry)?)?,
                    ));
                }
            }
            if let Some(entries) = row.optional_items(schema::CUBE_INLINE_DIMENSIONS_COLLECTION)? {
                for entry in entries {
                    let wrapper = list(entry)?;
                    let record = list(item(wrapper, 0)?)?;
                    let body = item(record, 1)?;
                    let typed = list(item(list(body)?, 1)?)?;
                    let head = header(item(typed, 1)?)?;
                    objects.children.push(
                        el("Dimension")
                            .attr("uuid", &head.uuid)
                            .child(inline_dimension_properties(record, owner, context)?),
                    );
                }
            }
            if let Some(entries) = row.optional_items(schema::CUBE_INLINE_RESOURCES_COLLECTION)? {
                for entry in entries {
                    let wrapper = list(entry)?;
                    let record = list(item(wrapper, 0)?)?;
                    let body = item(record, 1)?;
                    let typed = list(item(list(body)?, 1)?)?;
                    let head = header(item(typed, 1)?)?;
                    objects.children.push(
                        el("Resource")
                            .attr("uuid", &head.uuid)
                            .child(inline_resource_properties(record, owner, context)?),
                    );
                }
            }
        }
        schema::TABLE_KIND | schema::DIMENSION_TABLE_KIND => {
            if let Some(entries) = row.optional_items(schema::FIELDS_COLLECTION)? {
                for entry in entries {
                    let wrapper = list(entry)?;
                    let record = list(item(wrapper, 0)?)?;
                    let body = item(record, 1)?;
                    let typed = list(item(list(body)?, 1)?)?;
                    let head = header(item(typed, 1)?)?;
                    objects.children.push(
                        el("Field")
                            .attr("uuid", &head.uuid)
                            .child(field_properties(record, owner, context)?),
                    );
                }
            }
        }
        other => bail!("unknown EDS child kind {other}"),
    }
    // Forms and templates are separate descriptor rows named by full name;
    // commands are inline records of this row. Both follow the fields.
    for member in row
        .optional_items(schema::forms_collection(kind))?
        .unwrap_or_default()
    {
        objects
            .children
            .push(leaf("Form", short_name(&context.names, atom(member)?)?));
    }
    for member in row
        .optional_items(schema::commands_collection(kind))?
        .unwrap_or_default()
    {
        let wrapper = list(member)?;
        let record = list(item(wrapper, 0)?)?;
        let command = item(list(item(record, 1)?)?, 3)?;
        objects
            .children
            .push(super::objects::export::command(command, context)?);
    }
    for member in row
        .optional_items(schema::TEMPLATES_COLLECTION)?
        .unwrap_or_default()
    {
        objects
            .children
            .push(leaf("Template", short_name(&context.names, atom(member)?)?));
    }
    let _ = head;
    Ok(objects)
}

/// The separate-file child rows of one EDS child descriptor, as
/// `(XML tag, uuid)`: its forms, templates and commands. Each of those is its
/// own row, so the owner needs only their names to spell references.
#[doc(hidden)]
pub fn owned_source_members(kind: &str, text: &str) -> Result<Vec<(&'static str, String)>> {
    let row = parse_row(text.as_bytes())?;
    let parsed = Row::parse(&row, kind)?;
    let collections: &[(&str, &str)] = match kind {
        schema::CUBE_KIND => &[
            (schema::CUBE_FORMS_COLLECTION, "Form"),
            (schema::TEMPLATES_COLLECTION, "Template"),
        ],
        schema::TABLE_KIND => &[
            (schema::TABLE_FORMS_COLLECTION, "Form"),
            (schema::TEMPLATES_COLLECTION, "Template"),
        ],
        schema::DIMENSION_TABLE_KIND => &[
            (schema::DIMENSION_TABLE_FORMS_COLLECTION, "Form"),
            (schema::TEMPLATES_COLLECTION, "Template"),
        ],
        other => bail!("unknown EDS child kind {other}"),
    };
    let mut members = Vec::new();
    for (collection, tag) in collections {
        for member in parsed.optional_items(collection)?.unwrap_or_default() {
            members.push((*tag, atom(member)?.to_string()));
        }
    }
    Ok(members)
}

/// The name index contribution of an owned child row.
#[doc(hidden)]
pub fn owned_names(kind: &str, row: &Brace, owner: &str) -> Result<ObjectNames> {
    let parsed = Row::parse(row, kind)?;
    let head = parsed.header(kind)?;
    if owner.is_empty() {
        bail!("{kind}: owner is required for names");
    }
    let full_name = format!("{owner}.{kind}.{}", head.name);
    let path = Path::from_full(kind, &full_name)?;
    let mut types = Vec::new();
    for category in schema::categories(kind) {
        let type_id = atom(item(parsed.payload, category.body_slot)?)?.to_string();
        let value_id = atom(item(parsed.payload, category.body_slot + 1)?)?.to_string();
        types.push(GeneratedTypeName {
            name: path.generated_name(kind, category.name_stem)?,
            category: category.category.to_string(),
            type_id,
            value_id,
        });
    }
    let children = inline_children(kind, &parsed, &full_name)?;
    Ok(ObjectNames {
        uuid: head.uuid,
        full_name,
        children,
        types,
    })
}

fn inline_children(kind: &str, row: &Row<'_>, full_name: &str) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut add = |tag: &str, entry: &Brace| -> Result<()> {
        let wrapper = list(entry)?;
        let record = list(item(wrapper, 0)?)?;
        let body = item(record, 1)?;
        let typed = list(item(list(body)?, 1)?)?;
        let head = header(item(typed, 1)?)?;
        out.push((format!("{full_name}.{tag}.{}", head.name), head.uuid));
        Ok(())
    };
    match kind {
        schema::CUBE_KIND => {
            if let Some(entries) = row.optional_items(schema::CUBE_INLINE_DIMENSIONS_COLLECTION)? {
                for entry in entries {
                    add("Dimension", entry)?;
                }
            }
            if let Some(entries) = row.optional_items(schema::CUBE_INLINE_RESOURCES_COLLECTION)? {
                for entry in entries {
                    add("Resource", entry)?;
                }
            }
        }
        schema::TABLE_KIND | schema::DIMENSION_TABLE_KIND => {
            for entry in row
                .optional_items(schema::FIELDS_COLLECTION)?
                .unwrap_or_default()
            {
                add("Field", entry)?;
            }
        }
        _ => {}
    }
    Ok(out)
}

/// The cube and table UUIDs a root ExternalDataSource row lists.
#[doc(hidden)]
pub fn root_children(text: &str) -> Result<(Vec<String>, Vec<String>)> {
    let row = parse_row(text.as_bytes())?;
    let root = list(&row)?;
    let count = usize::try_from(super::export::number(item(root, 2)?)?)
        .map_err(|_| anyhow!("root collection count"))?;
    let mut cubes = Vec::new();
    let mut tables = Vec::new();
    for collection in root.iter().skip(3).take(count) {
        let fields = list(collection)?;
        let id = atom(item(fields, 0)?)?;
        let count = atom(item(fields, 1)?)?
            .parse::<usize>()
            .map_err(|_| anyhow!("root child count"))?;
        let members = fields
            .get(2..2 + count)
            .ok_or_else(|| anyhow!("root child collection is short"))?;
        let target = match id {
            id if id == schema::ROOT_CUBES_COLLECTION => &mut cubes,
            id if id == schema::ROOT_TABLES_COLLECTION => &mut tables,
            _ => continue,
        };
        for member in members {
            target.push(atom(member)?.to_string());
        }
    }
    Ok((cubes, tables))
}

/// The dimension-table UUIDs a Cube row lists.
#[doc(hidden)]
pub fn dimension_table_refs(text: &str) -> Result<Vec<String>> {
    let row = parse_row(text.as_bytes())?;
    let parsed = Row::parse(&row, schema::CUBE_KIND)?;
    parsed
        .items(schema::CUBE_DIMENSION_TABLES_COLLECTION)?
        .iter()
        .map(|member| Ok(atom(member)?.to_string()))
        .collect()
}

/// Recognise a child descriptor by its measured body layout. `None` is not an
/// error: the caller keeps its previous fallbacks.
#[doc(hidden)]
pub fn family(text: &str, uuid: &str) -> Option<(&'static str, &'static str)> {
    let row = parse_row(text.as_bytes()).ok()?;
    let parsed = Row::parse(&row, schema::CUBE_KIND)
        .ok()
        .or_else(|| Row::parse(&row, schema::DIMENSION_TABLE_KIND).ok())
        .or_else(|| Row::parse(&row, schema::TABLE_KIND).ok())?;
    let kind = parsed_kind(parsed.payload).ok()?;
    if parsed.header(kind).ok()?.uuid != uuid {
        return None;
    }
    Some((kind, schema::collection(kind)))
}

fn parsed_kind(payload: &[Brace]) -> Result<&'static str> {
    match (atom(item(payload, 0)?)?, payload.len()) {
        ("0", schema::CUBE_BODY_ARITY) => Ok(schema::CUBE_KIND),
        ("0", schema::DIMENSION_TABLE_BODY_ARITY) => Ok(schema::DIMENSION_TABLE_KIND),
        ("11", schema::TABLE_BODY_ARITY) => Ok(schema::TABLE_KIND),
        (code, len) => bail!("not an EDS child: code {code}, arity {len}"),
    }
}

/// Whether the row has a measured EDS-child body shape but failed `family`.
/// The caller refuses it instead of letting it fall into `IntegrationService`.
#[doc(hidden)]
pub fn looks_like_eds_child(text: &str) -> bool {
    let Ok(row) = parse_row(text.as_bytes()) else {
        return false;
    };
    let Ok(root) = list(&row) else {
        return false;
    };
    let Ok(payload) = item(root, 1) else {
        return false;
    };
    let Some(payload) = payload.as_list() else {
        return false;
    };
    matches!(
        (payload.first().and_then(Brace::as_atom), payload.len()),
        (Some("0"), schema::CUBE_BODY_ARITY)
            | (Some("0"), schema::DIMENSION_TABLE_BODY_ARITY)
            | (Some("11"), schema::TABLE_BODY_ARITY)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // ServiceDesk 3.10.28 / 8.5.1.1529 row, independently documented by
    // `feat/rea-form-property-inventory-20261022`
    // (`docs/evidence/rea-external-data-source-20261022/README.md`). It has
    // non-empty KeyFields/InputByString and both N and S field types.
    const ROW: &str =
        include_str!("../../../../tests/fixtures/eds-branch/external_data_source_table_row.txt");

    #[test]
    fn branch_table_row_names_key_fields_and_input_by_string() {
        let row = parse_row(ROW.as_bytes()).unwrap();
        let parsed = Row::parse(&row, schema::TABLE_KIND).unwrap();
        let (key_fields, input_by_string) = table_ref_lists(&parsed).unwrap();
        assert_eq!(key_fields, vec!["product_id".to_string()]);
        assert_eq!(input_by_string, vec!["product_id".to_string()]);

        let names = field_names(&parsed).unwrap();
        assert_eq!(
            names
                .get("a9a45781-332e-4c72-ac04-579e8e11e6c5")
                .map(String::as_str),
            Some("product_id")
        );
        assert_eq!(
            names
                .get("b35dfac3-6d64-4c5f-8feb-2758776bec94")
                .map(String::as_str),
            Some("_c_id")
        );
    }

    #[test]
    fn branch_table_row_field_types_are_n_and_s() {
        let row = parse_row(ROW.as_bytes()).unwrap();
        let parsed = Row::parse(&row, schema::TABLE_KIND).unwrap();
        let mut patterns = Vec::new();
        for entry in parsed.items(schema::FIELDS_COLLECTION).unwrap() {
            let wrapper = list(entry).unwrap();
            let record = list(item(wrapper, 0).unwrap()).unwrap();
            let body = item(record, 1).unwrap();
            let typed = list(item(list(body).unwrap(), 1).unwrap()).unwrap();
            let pattern = list(item(typed, 2).unwrap()).unwrap();
            patterns.push(
                pattern[1].as_list().unwrap()[0]
                    .as_str()
                    .unwrap()
                    .to_string(),
            );
        }
        assert_eq!(patterns, ["N", "S"]);
    }
}

#[cfg(test)]
mod eds_guard_regressions {
    use super::*;
    const TABLE: &[u8] =
        include_bytes!("../../../../tests/fixtures/eds-branch/external_data_source_table_row.txt");

    fn export_context() -> ExportContext {
        ExportContext {
            names: NameIndex::default(),
            version: "2.20".to_string(),
            compat: super::super::objects::parts::Compat(8, 3, 27),
        }
    }

    #[test]
    fn eds_guard_refuses_malformed_presentation() {
        let mut row = parse_row(TABLE).unwrap();
        row.as_list_mut().unwrap()[1].as_list_mut().unwrap()
            [schema::TABLE_OBJECT_PRESENTATION_SLOT] =
            parse_row(br#"{2,"ru","only one pair"}"#).unwrap();
        let parsed = Row::parse(&row, schema::TABLE_KIND).unwrap();
        let head = parsed.header(schema::TABLE_KIND).unwrap();
        assert!(
            table_properties(
                &parsed,
                &head,
                "ExternalDataSource.Review.Table.T",
                &export_context()
            )
            .is_err()
        );
    }

    #[test]
    fn eds_guard_refuses_changed_unmodeled_scalar() {
        let mut row = parse_row(TABLE).unwrap();
        row.as_list_mut().unwrap()[1].as_list_mut().unwrap()[38] =
            parse_row(br#"{11111111-1111-4111-8111-111111111111}"#).unwrap();
        let parsed = Row::parse(&row, schema::TABLE_KIND).unwrap();
        let head = parsed.header(schema::TABLE_KIND).unwrap();
        assert!(
            table_properties(
                &parsed,
                &head,
                "ExternalDataSource.Review.Table.T",
                &export_context()
            )
            .is_err()
        );
    }

    #[test]
    fn eds_guard_refuses_malformed_child_collections_at_name_boundary() {
        for bad_count in ["invalid", "999", "18446744073709551615"] {
            let mut row = parse_row(TABLE).unwrap();
            let outer = row.as_list_mut().unwrap();
            let fields = outer
                .iter_mut()
                .skip(3)
                .find_map(|node| {
                    let fields = node.as_list_mut()?;
                    (fields.first()?.as_atom()? == schema::FIELDS_COLLECTION).then_some(fields)
                })
                .unwrap();
            fields[1] = Brace::atom(bad_count);
            assert!(
                owned_names(schema::TABLE_KIND, &row, "ExternalDataSource.Review").is_err(),
                "{bad_count}"
            );
        }
    }

    #[test]
    fn eds_guard_refuses_collection_members_after_declared_count() {
        let mut row = parse_row(TABLE).unwrap();
        let outer = row.as_list_mut().unwrap();
        let fields = outer
            .iter_mut()
            .skip(3)
            .find_map(|node| {
                let fields = node.as_list_mut()?;
                (fields.first()?.as_atom()? == schema::FIELDS_COLLECTION).then_some(fields)
            })
            .unwrap();
        fields[1] = Brace::num(0);
        assert!(owned_names(schema::TABLE_KIND, &row, "ExternalDataSource.Review").is_err());
    }
}

#[cfg(test)]
mod eds_guard_additional {
    use super::*;
    #[test]
    fn eds_guard_checks_every_declared_default_structurally() {
        for (kind, bytes) in [
            (
                schema::CUBE_KIND,
                include_bytes!("../../../../tests/fixtures/eds-matrix/cube.txt").as_slice(),
            ),
            (
                schema::TABLE_KIND,
                include_bytes!("../../../../tests/fixtures/eds-matrix/table.txt").as_slice(),
            ),
            (
                schema::DIMENSION_TABLE_KIND,
                include_bytes!("../../../../tests/fixtures/eds-matrix/dimension.txt").as_slice(),
            ),
        ] {
            let valid = parse_row(bytes).unwrap();
            check_row_shape(&Row::parse(&valid, kind).unwrap(), kind).unwrap();
            for expected in schema::unmodeled_defaults(kind) {
                let mut changed = valid.clone();
                changed.as_list_mut().unwrap()[1].as_list_mut().unwrap()[expected.body_slot] =
                    Brace::str("changed");
                assert!(
                    check_row_shape(&Row::parse(&changed, kind).unwrap(), kind).is_err(),
                    "{kind} slot {}",
                    expected.body_slot
                );
            }
        }
    }
    #[test]
    fn eds_guard_rejects_malformed_typed_reference_counts() {
        let mut row = parse_row(include_bytes!(
            "../../../../tests/fixtures/eds-branch/external_data_source_table_row.txt"
        ))
        .unwrap();
        row.as_list_mut().unwrap()[1].as_list_mut().unwrap()[schema::TABLE_KEY_FIELDS_SLOT]
            .as_list_mut()
            .unwrap()[schema::COLLECTION_COUNT_SLOT] = Brace::num(0);
        assert!(table_ref_lists(&Row::parse(&row, schema::TABLE_KIND).unwrap()).is_err());
    }
    #[test]
    fn eds_guard_distinguishes_absent_from_malformed_collections() {
        let mut row = parse_row(include_bytes!(
            "../../../../tests/fixtures/eds-matrix/table.txt"
        ))
        .unwrap();
        let outer = row.as_list_mut().unwrap();
        let at = outer
            .iter()
            .position(|n| {
                n.as_list().and_then(|v| v.first()).and_then(Brace::as_atom)
                    == Some(schema::FIELDS_COLLECTION)
            })
            .unwrap();
        outer.remove(at);
        outer[schema::ROW_COLLECTION_COUNT_SLOT] =
            Brace::num((outer.len() - schema::ROW_COLLECTIONS_START) as i64);
        assert!(
            Row::parse(&row, schema::TABLE_KIND)
                .unwrap()
                .optional_items(schema::FIELDS_COLLECTION)
                .unwrap()
                .is_none()
        );
    }
}
