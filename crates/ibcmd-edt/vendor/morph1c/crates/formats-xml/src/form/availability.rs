//! Native SDK unavailable-path spelling for EDT XML 2.20/2.21 projections.
//! Query selection grammar is adapted from ibcmd's existing
//! src/mssql_dump/form_body.rs; query bytes and canonical paths are unchanged.
use super::FormError;
use morph1c_core::{
    ir::{Configuration, DynamicListAttrExt, FormBody, MetadataObject, PropertyValue},
    version::{FormatVersion, current_roundtrip_target},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::Arc;
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};
/// CURRENT configuration metadata used only to derive native path availability.
/// The namespace borrows fully claimed objects; no source paths or field values
/// are cached in the form or transported through provenance.
pub struct FormProjectionContext<'a> {
    pub metadata: &'a Configuration,
    objects: BTreeMap<String, &'a MetadataObject>,
    metadata_sha256: String,
    reference_graph: Arc<ReferenceGraph>,
}
impl<'a> FormProjectionContext<'a> {
    pub fn new(metadata: &'a Configuration) -> Result<Self, FormError> {
        let mut objects = BTreeMap::new();
        for object in &metadata.objects {
            if family(object.kind.as_str()).is_none() && object.kind.as_str() != "CommonAttribute" {
                continue;
            }
            let identity =
                super::java_case_fold::fold(&format!("{}.{}", object.kind.as_str(), object.name));
            if objects.insert(identity, object).is_some() {
                return Err(FormError::Frame(
                    "duplicate current metadata identity in form projection".into(),
                ));
            }
        }
        let mut hash = DependencyHash(Sha256::new());
        // The SDK calculated-field provider depends on CURRENT project
        // compatibility, independently of the requested native XML dialect.
        let compatibility = morph1c_core::spec::metadata::configuration::F_COMPATIBILITY_MODE;
        hash.part(
            &metadata
                .properties
                .iter()
                .find(|(id, _)| *id == compatibility),
        )?;
        for object in objects.values() {
            let mut pending = vec![*object];
            while let Some(object) = pending.pop() {
                hash.part(&(
                    object.kind.as_str(),
                    &object.name,
                    object.uuid,
                    &object.properties,
                ))?;
                pending.extend(object.children.iter().rev());
            }
        }
        let mut context = Self {
            metadata,
            objects,
            metadata_sha256: hash.finish(),
            reference_graph: Arc::new(ReferenceGraph::default()),
        };
        context.reference_graph = Arc::new(context.reference_graph());
        Ok(context)
    }
}
struct DependencyHash(Sha256);
impl Write for DependencyHash {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl DependencyHash {
    fn part(&mut self, value: &impl Serialize) -> Result<(), FormError> {
        serde_json::to_writer(&mut *self, value)
            .map_err(|e| FormError::Frame(format!("form availability dependency: {e}")))?;
        self.0.update([0]);
        Ok(())
    }
    fn finish(self) -> String {
        format!("{:x}", self.0.finalize())
    }
}
impl FormProjectionContext<'_> {
    /// Bind CURRENT dependencies only; no source values, modules or asset bytes.
    pub fn dependency_sha256(
        &self,
        body: &FormBody,
        profile: FormatVersion,
    ) -> Result<String, FormError> {
        dependency_sha256(body, profile, &self.metadata_sha256)
    }
}
pub(crate) fn source_form_dependency_sha256(
    body: &FormBody,
    profile: FormatVersion,
) -> Result<String, FormError> {
    dependency_sha256(body, profile, "standalone-form-v1")
}
fn dependency_sha256(
    body: &FormBody,
    profile: FormatVersion,
    metadata_sha256: &str,
) -> Result<String, FormError> {
    let mut hash = DependencyHash(Sha256::new());
    hash.part(&(metadata_sha256, profile.major, profile.minor))?;
    let mut attributes: Vec<_> = body.data_attributes.iter().rev().collect();
    while let Some(attribute) = attributes.pop() {
        hash.part(&(
            &attribute.name,
            attribute.id,
            &attribute.value_type,
            attribute.main,
            &attribute.settings_saved_data,
            &attribute.not_default_use_always,
        ))?;
        if let Some(list) = &attribute.dynamic_list {
            hash.part(&(
                &list.query_text,
                &list.main_table,
                &list.key_type,
                list.custom_query,
                list.dynamic_data_read,
                list.auto_fill_available_fields,
                list.auto_save_user_settings,
                list.get_invisible_field_presentations,
                &list.key_fields,
                &list.calculated_fields,
                &list.fields,
                &list.parameters,
            ))?;
        }
        attributes.extend(attribute.columns.iter().rev());
        for additional in attribute.additional_columns.iter().rev() {
            hash.part(&additional.table_path)?;
            attributes.extend(additional.columns.iter().rev());
        }
    }
    for command in body
        .form_ci_navigation_panel
        .iter()
        .chain(&body.form_ci_command_bar)
    {
        hash.part(&(&command.command, &command.command_parameter))?;
    }
    let mut items: Vec<_> = body.items.iter().rev().collect();
    if let Some(bar) = &body.auto_command_bar {
        items.extend(bar.items.iter().rev());
    }
    while let Some(item) = items.pop() {
        let row_path = if item.kind.as_str() == "Table" {
            item.get(morph1c_core::spec::forms::controls::table::F_ROW_PICTURE_DATA_PATH)
        } else {
            None
        };
        hash.part(&(&item.kind, &item.name, item.id, row_path))?;
        // CURRENT typed AbstractDataPath values participate in the binding too:
        // editing a control path must not replay its previous native marker.
        let mut projections: Vec<&[super::fields::FieldProj]> = Vec::new();
        if let Some(kind) = super::tables::field_kind(item.kind.as_str()) {
            projections.extend([super::tables::FORM_FIELD_COMMON, kind.ext]);
        } else if let Some(kind) = super::tables::group_kind(item.kind.as_str()) {
            projections.extend([super::tables::FORM_GROUP_BODY, kind.ext]);
        } else if let Some(kind) = super::tables::decoration_kind(item.kind.as_str()) {
            projections.extend([super::tables::DECORATION_BODY, kind.ext]);
        } else {
            match item.kind.as_str() {
                "Button" => projections.push(super::tables::BUTTON_BODY),
                "Table" => projections.push(super::tables::TABLE_BODY),
                _ => {}
            }
        }
        for (region, bag) in [
            (super::fields::Region::Body, &item.properties),
            (super::fields::Region::Ext, &item.ext_info),
        ] {
            let path_fields: BTreeSet<_> = projections
                .iter()
                .flat_map(|fields| fields.iter())
                .filter(|field| {
                    field.region == region
                        && matches!(
                            field.codec,
                            super::fields::Codec::DataPath
                                | super::fields::Codec::TypeLink
                                | super::fields::Codec::ChoiceParameterLinks
                        )
                })
                .map(|field| field.id)
                .collect();
            for (field, value) in bag {
                if path_fields.contains(field) {
                    hash.part(&(region == super::fields::Region::Ext, field, value))?;
                }
            }
        }
        items.extend(item.children.iter().rev());
        items.extend(item.additions.iter().rev());
        if let Some(table) = &item.auto_table {
            items.push(table);
        }
        if let Some(bar) = &item.auto_command_bar {
            items.extend(bar.items.iter().rev());
        }
        if let Some(menu) = &item.context_menu {
            if let morph1c_core::ir::DecoratorBody::ContextMenu(menu) = &menu.body {
                items.extend(menu.items.iter().rev());
            }
        }
    }
    Ok(hash.finish())
}
/// Called only after the complete native configuration has been read.
pub fn bind_native_availability_sources(metadata: &mut Configuration) -> Result<(), FormError> {
    let Some(profile) = metadata.source_version else {
        return Ok(());
    };
    let context = FormProjectionContext::new(metadata)?;
    let mut pending: Vec<_> = metadata
        .objects
        .iter()
        .enumerate()
        .rev()
        .map(|(i, o)| (vec![i], o))
        .collect();
    let mut plans = Vec::new();
    while let Some((path, object)) = pending.pop() {
        for (index, form) in object.form_bodies.iter().enumerate() {
            if form.ordinary_body.is_none() && form.body.designer_path_spelling {
                plans.push((
                    path.clone(),
                    index,
                    context.dependency_sha256(&form.body, profile)?,
                ));
            }
        }
        for (index, child) in object.children.iter().enumerate().rev() {
            let mut child_path = path.clone();
            child_path.push(index);
            pending.push((child_path, child));
        }
    }
    drop(context);
    for (path, index, digest) in plans {
        let mut object = &mut metadata.objects[path[0]];
        for child in &path[1..] {
            object = &mut object.children[*child];
        }
        object.form_bodies[index]
            .body
            .availability_source_dependency = Some(digest);
    }
    Ok(())
}
// The SDK alias-shadowing predicate uses String.toLowerCase, separately from
// identifier equalsIgnoreCase. Neither comparison changes authored spelling.
fn contains_alias(names: &BTreeSet<String>, name: &str) -> bool {
    names
        .iter()
        .any(|candidate| candidate.to_lowercase() == name.to_lowercase())
}
fn contains_name(names: &BTreeSet<String>, name: &str) -> bool {
    names
        .iter()
        .any(|candidate| super::java_case_fold::equal(candidate, name))
}
struct MetadataTable {
    fields: BTreeSet<String>,
    required: BTreeSet<String>,
    standards: Vec<(&'static str, &'static str)>,
}
impl MetadataTable {
    fn field_identity(&self, field: &str) -> String {
        for (ru, en) in &self.standards {
            if super::java_case_fold::equal(field, ru) || super::java_case_fold::equal(field, en) {
                return super::java_case_fold::fold(en);
            }
        }
        super::java_case_fold::fold(field)
    }
}
fn current_property<'a>(object: &'a MetadataObject, name: &str) -> Option<&'a PropertyValue> {
    let spec = morph1c_core::spec::registry::spec_for(object.kind.as_str())?;
    let field = spec.fields.iter().find(|f| f.name == name)?;
    object.get(field.id).or(field.default.as_ref())
}
fn current_bool(object: &MetadataObject, name: &str) -> bool {
    matches!(
        current_property(object, name),
        Some(PropertyValue::Bool(true))
    )
}
fn current_int(object: &MetadataObject, name: &str) -> i64 {
    match current_property(object, name) {
        Some(PropertyValue::Int(n)) => *n,
        _ => 0,
    }
}
fn current_token<'a>(object: &'a MetadataObject, name: &str) -> &'a str {
    match current_property(object, name) {
        Some(PropertyValue::Enum(n)) => n.as_str(),
        _ => "",
    }
}
#[derive(Clone, Copy)]
enum CommonAttributeFilter {
    DbView,
    Any,
    Nonseparators,
}

fn common_attribute_used(
    attribute: &MetadataObject,
    target: &str,
    filter: CommonAttributeFilter,
) -> bool {
    // The SDK DbView provider excludes an independent separator even when
    // CURRENT content explicitly requests Use for this metadata object.
    let separated = current_token(attribute, "dataSeparation") == "Separate";
    if separated
        && match filter {
            CommonAttributeFilter::Any => false,
            CommonAttributeFilter::Nonseparators => true,
            CommonAttributeFilter::DbView => {
                current_token(attribute, "separatedDataUse") != "IndependentlyAndSimultaneously"
            }
        }
    {
        return false;
    }
    if let Some(PropertyValue::List(content)) = current_property(attribute, "content") {
        for row in content {
            let PropertyValue::List(parts) = row else {
                continue;
            };
            let [PropertyValue::Str(metadata), PropertyValue::Enum(usage)] = parts.as_slice()
            else {
                continue;
            };
            if !super::java_case_fold::equal(metadata, target) {
                continue;
            }
            match usage.as_str() {
                "Use" => return true,
                "DontUse" => return false,
                _ => {}
            }
        }
    }
    current_token(attribute, "autoUse") == "Use"
}

fn family(name: &str) -> Option<&'static str> {
    [
        ("Catalog", "Справочник"),
        ("Document", "Документ"),
        ("Enum", "Перечисление"),
        ("ChartOfCharacteristicTypes", "ПланВидовХарактеристик"),
        ("ChartOfAccounts", "ПланСчетов"),
        ("ChartOfCalculationTypes", "ПланВидовРасчета"),
        ("ExchangePlan", "ПланОбмена"),
        ("BusinessProcess", "БизнесПроцесс"),
        ("Task", "Задача"),
        ("InformationRegister", "РегистрСведений"),
        ("AccumulationRegister", "РегистрНакопления"),
        ("AccountingRegister", "РегистрБухгалтерии"),
        ("CalculationRegister", "РегистрРасчета"),
        ("DocumentJournal", "ЖурналДокументов"),
    ]
    .into_iter()
    .find(|(en, ru)| en.eq_ignore_ascii_case(name) || super::java_case_fold::equal(ru, name))
    .map(|(en, _)| en)
}
impl FormProjectionContext<'_> {
    fn calculated_fields_available(&self) -> bool {
        let id = morph1c_core::spec::metadata::configuration::F_COMPATIBILITY_MODE;
        let mode = self
            .metadata
            .properties
            .iter()
            .find(|(field, _)| *field == id)
            .and_then(|(_, value)| match value {
                PropertyValue::Enum(mode) => Some(mode.as_str()),
                _ => None,
            })
            .unwrap_or("");
        // Bound SDK EEnum factory/import default is 8.5.1. Its DontUse reader
        // sentinel maps to 8.3.8, not to the native XML target or latest mode.
        if mode.is_empty() {
            return true;
        }
        if mode == "DontUse" {
            return false;
        }
        let mut parts = mode.split('.').map(str::parse::<u32>);
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(Ok(major)), Some(Ok(minor)), Some(Ok(patch)), None) => {
                [major, minor, patch] > [8, 3, 18]
            }
            _ => false,
        }
    }

    fn table(&self, table: &str) -> Option<MetadataTable> {
        let mut parts = table.split('.');
        let kind = family(parts.next()?)?;
        let name = parts.next()?;
        let object = self
            .objects
            .get(&super::java_case_fold::fold(&format!("{kind}.{name}")))?;
        // Main tables and authored tabular sections are distinct CURRENT namespaces.
        // Virtual tables require their own compiler field rules, never a guessed
        // reuse of the main-table universe.
        if let Some(section) = parts.next() {
            if parts.next().is_some() {
                return None;
            }
            let child = object.children.iter().find(|c| {
                c.kind.as_str().ends_with(".TabularSection")
                    && super::java_case_fold::equal(&c.name, section)
            })?;
            let mut fields = BTreeSet::new();
            fields.extend(["Ref", "Ссылка", "LineNumber", "НомерСтроки"].map(String::from));
            for attribute in &child.children {
                if attribute.kind.as_str().ends_with(".Attribute") {
                    fields.insert(attribute.name.clone());
                }
            }
            return Some(MetadataTable {
                fields,
                required: BTreeSet::new(),
                standards: vec![("Ссылка", "Ref"), ("НомерСтроки", "LineNumber")],
            });
        }
        let mut fields = BTreeSet::new();
        let mut required = BTreeSet::new();
        let mut standards = Vec::new();
        let pairs = standard_pairs(kind)?;
        for (ru, en) in pairs {
            let present = match (kind, *en) {
                ("InformationRegister", "Period") => {
                    current_token(object, "informationRegisterPeriodicity") != "Nonperiodical"
                }
                ("InformationRegister", "Recorder" | "LineNumber" | "Active") => {
                    current_token(object, "writeMode") == "RecorderSubordinate"
                }
                ("Catalog", "IsFolder") => {
                    current_bool(object, "hierarchical")
                        && current_token(object, "hierarchyType") == "HierarchyFoldersAndItems"
                }
                ("Catalog" | "ChartOfCharacteristicTypes", "Parent" | "IsFolder") => {
                    current_bool(object, "hierarchical")
                }
                ("Catalog", "Owner") => {
                    matches!(current_property(object, "owners"), Some(PropertyValue::List(v)) if !v.is_empty())
                }
                (_, "Code") => current_int(object, "codeLength") > 0,
                (_, "Description") => current_int(object, "descriptionLength") > 0,
                (_, "Number") => current_int(object, "numberLength") > 0,
                ("AccumulationRegister", "RecordType") => {
                    current_token(object, "registerType") == "Balance"
                }
                _ => true,
            };
            if !present {
                continue;
            }
            standards.push((*ru, *en));
            fields.insert((*ru).into());
            fields.insert((*en).into());
            // DynamicListFieldService selects keys, deletion/posted, recorder,
            // hierarchy and date roles; it does not append every std attribute.
            let injected = matches!(
                *en,
                "Ref"
                    | "DeletionMark"
                    | "Posted"
                    | "Recorder"
                    | "Parent"
                    | "IsFolder"
                    | "Owner"
                    | "Date"
            ) || (kind == "InformationRegister" && *en == "Period")
                || (kind == "AccumulationRegister" && matches!(*en, "Period" | "LineNumber"));
            if injected {
                required.insert((*ru).into());
                required.insert((*en).into());
            }
        }
        for child in &object.children {
            let suffix = child
                .kind
                .as_str()
                .strip_prefix(kind)
                .and_then(|s| s.strip_prefix('.'));
            if matches!(
                suffix,
                Some(
                    "Attribute"
                        | "Dimension"
                        | "Resource"
                        | "AccountingFlag"
                        | "ExtDimensionAccountingFlag"
                )
            ) {
                fields.insert(child.name.clone());
                if suffix == Some("Dimension") && kind == "InformationRegister" {
                    required.insert(child.name.clone());
                }
            }
        }
        let target = format!("{kind}.{name}");
        let filter = match kind {
            "ExchangePlan" => CommonAttributeFilter::Any,
            "DocumentJournal" if !matches!(current_property(object, "registeredDocuments"), Some(PropertyValue::List(v)) if !v.is_empty()) => {
                CommonAttributeFilter::Nonseparators
            }
            _ => CommonAttributeFilter::DbView,
        };
        for common in self
            .objects
            .values()
            .filter(|object| kind != "Enum" && object.kind.as_str() == "CommonAttribute")
        {
            if common_attribute_used(common, &target, filter) {
                // A current CommonAttribute owns one name in both SDK maps;
                // it is not a standard definition with inferred language twins.
                fields.insert(common.name.clone());
            }
        }
        Some(MetadataTable {
            fields,
            required,
            standards,
        })
    }
    fn fields(&self, list: &DynamicListAttrExt) -> Result<Option<BTreeSet<String>>, FormError> {
        if !list.custom_query {
            let Some(mut fields) = list
                .main_table
                .as_deref()
                .and_then(|t| self.table(t))
                .map(|t| t.fields)
            else {
                return Ok(None);
            };
            if let Some(node) = list
                .main_table
                .as_deref()
                .and_then(|table| self.reference_graph.nodes.get(&canonical_table(table)))
            {
                fields.retain(|field| {
                    node.fields
                        .contains_key(&super::java_case_fold::fold(field))
                });
            }
            if self.calculated_fields_available() {
                fields.extend(list.calculated_fields.iter().map(|c| c.data_path.clone()));
            }
            return Ok(Some(fields));
        }
        if !list.auto_fill_available_fields {
            let mut available = list
                .fields
                .iter()
                .map(|field| field.data_path.clone())
                .collect::<BTreeSet<_>>();
            if self.calculated_fields_available() {
                available.extend(
                    list.calculated_fields
                        .iter()
                        .map(|field| field.data_path.clone()),
                );
            }
            return Ok(Some(available));
        }
        let Some(query) = &list.query_text else {
            return Ok(Some(BTreeSet::new()));
        };
        let Some(Selection {
            mut fields,
            aliases,
            sources,
            selected_backing,
            required_scope,
            ..
        }) = selection_current(query, Some(self), list.main_table.as_deref())?
        else {
            return Ok(None);
        };
        // Query aliases are independent names; language twins belong only to
        // CURRENT metadata fields referenced without an explicit alias.
        for table in sources {
            if let Some(schema) = self.table(&table.1) {
                // Only actual standard definitions in this CURRENT table
                // own language twins. A Catalog's custom Date field is not
                // the Document Date/Дата definition.
                for (ru, en) in &schema.standards {
                    let identity = (canonical_table(&table.1), schema.field_identity(en));
                    if !selected_backing.contains(&identity) {
                        continue;
                    }
                    if contains_name(&fields, ru) && !contains_alias(&aliases, ru) {
                        fields.insert((*en).into());
                    }
                    if contains_name(&fields, en) && !contains_alias(&aliases, en) {
                        fields.insert((*ru).into());
                    }
                }
            }
        }
        if let Some(scope) = required_scope {
            if !scope.has_star {
                if let Some(schema) = self.table(&scope.main_table) {
                    // The SDK chooses one matching QuerySchemaOperator, then
                    // subtracts that operator's selected field definitions.
                    for field in &schema.required {
                        let identity = (
                            canonical_table(&scope.main_table),
                            schema.field_identity(field),
                        );
                        if !scope.selected_backing.contains(&identity) {
                            fields.insert(field.clone());
                        }
                    }
                }
            }
        }
        if self.calculated_fields_available() {
            fields.extend(list.calculated_fields.iter().map(|c| c.data_path.clone()));
        }
        Ok(Some(fields))
    }
}
fn canonical_table(table: &str) -> String {
    let Some((kind, name)) = table.split_once('.') else {
        return super::java_case_fold::fold(table);
    };
    super::java_case_fold::fold(&format!("{}.{}", family(kind).unwrap_or(kind), name))
}
// Top-level source references only. Nested SELECTs own their source namespace;
// their output requires a result schema rather than treating inner fields as
// columns of the outer query.
fn query_tables(tokens: &[String]) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i].as_str() {
            "(" | "{" => {
                depth += 1;
                i += 1;
                continue;
            }
            ")" | "}" => {
                depth -= 1;
                i += 1;
                continue;
            }
            _ => {}
        }
        if depth == 0 && keyword(&tokens[i], &["ИЗ", "FROM", "СОЕДИНЕНИЕ", "JOIN"]) {
            i += 1;
            let start = i;
            if i < tokens.len() && ident(&tokens[i]) {
                i += 1;
                while i + 1 < tokens.len() && tokens[i] == "." && ident(&tokens[i + 1]) {
                    i += 2;
                }
                if i - start >= 3 {
                    let table = tokens[start..i].join("");
                    if tokens.get(i).is_some_and(|t| t == "(") {
                        let mut args = 1;
                        i += 1;
                        while i < tokens.len() && args != 0 {
                            match tokens[i].as_str() {
                                "(" => args += 1,
                                ")" => args -= 1,
                                _ => {}
                            }
                            i += 1;
                        }
                    }
                    let mut alias = tokens[i - 1].clone();
                    if tokens.get(i).is_some_and(|t| keyword(t, &["КАК", "AS"]))
                        && tokens.get(i + 1).is_some_and(|t| ident(t))
                    {
                        alias = tokens[i + 1].clone();
                        i += 2;
                    }
                    result.push((alias, table));
                }
            }
        } else {
            i += 1;
        }
    }
    result
}
struct Availability {
    default_picture_unavailable: bool,
    fields: Option<BTreeSet<String>>,
    reference_root: Option<String>,
    untyped_roots: BTreeSet<String>,
}
thread_local! {
    static REFERENCE_GRAPH: RefCell<Option<Arc<ReferenceGraph>>> = const { RefCell::new(None) };
    static CURRENT_DATA_TABLES: RefCell<BTreeMap<String, Option<String>>> = const { RefCell::new(BTreeMap::new()) };
}
thread_local! { static AVAILABILITY: RefCell<BTreeMap<String,Availability>> = const {RefCell::new(BTreeMap::new())}; }
thread_local! { static RETAIN_SOURCE: std::cell::Cell<bool> = const {std::cell::Cell::new(true)}; }
pub(crate) fn unavailable(path: &str) -> bool {
    resolve(path).unwrap_or(false)
}
fn resolve(path: &str) -> Option<bool> {
    AVAILABILITY.with(|map| {
        REFERENCE_GRAPH.with(|graph| {
            CURRENT_DATA_TABLES.with(|tables| {
                resolve_current_path(
                    &map.borrow(),
                    graph.borrow().as_deref(),
                    &tables.borrow(),
                    path,
                )
            })
        })
    })
}

pub(crate) fn marked(path: &str, source_marker: bool) -> bool {
    RETAIN_SOURCE.with(|retain| {
        if retain.get() {
            source_marker || unavailable(path)
        } else {
            unavailable(path)
        }
    })
}
/// Render CURRENT AbstractDataPath values; native marker presence is restored
/// separately by the typed wire-order facet under the same dependency guard.
pub(crate) fn rendered_data_path(path: &str) -> String {
    if unavailable(path) && !path.starts_with('~') {
        format!("~{path}")
    } else {
        path.to_owned()
    }
}
pub(crate) fn retain_source_markers() -> bool {
    RETAIN_SOURCE.with(std::cell::Cell::get)
}
pub(crate) fn with_availability<T>(
    body: &FormBody,
    context: Option<&FormProjectionContext<'_>>,
    f: impl FnOnce() -> Result<T, FormError>,
) -> Result<T, FormError> {
    struct Restore(
        BTreeMap<String, Availability>,
        bool,
        Option<Arc<ReferenceGraph>>,
        BTreeMap<String, Option<String>>,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            AVAILABILITY.with(|m| *m.borrow_mut() = std::mem::take(&mut self.0));
            RETAIN_SOURCE.with(|retain| retain.set(self.1));
            REFERENCE_GRAPH.with(|graph| *graph.borrow_mut() = self.2.take());
            CURRENT_DATA_TABLES.with(|tables| *tables.borrow_mut() = std::mem::take(&mut self.3));
        }
    }
    let retain_source = match context {
        None => current_roundtrip_target()
            .map(|profile| source_form_dependency_sha256(body, profile))
            .transpose()?
            .is_some_and(|hash| body.availability_source_form_dependency.as_ref() == Some(&hash)),
        Some(context) => current_roundtrip_target()
            .map(|profile| context.dependency_sha256(body, profile))
            .transpose()?
            .is_some_and(|hash| body.availability_source_dependency.as_ref() == Some(&hash)),
    };
    let mut map = BTreeMap::new();
    if (!body.designer_path_spelling || !retain_source)
        && matches!(
            current_roundtrip_target(),
            Some(FormatVersion {
                major: 2,
                minor: 20 | 21
            })
        )
    {
        for attr in &body.data_attributes {
            if let Some(list) = &attr.dynamic_list {
                let no_picture = list.main_table.as_deref().is_none_or(|table| {
                    table
                        .split_once('.')
                        .is_some_and(|(family, _)| matches!(family, "Enum" | "FilterCriterion"))
                });
                let fields = match context {
                    Some(context) => context.fields(list)?,
                    None => fields(list)?,
                };
                if map
                    .insert(
                        super::java_case_fold::fold(&attr.name),
                        Availability {
                            default_picture_unavailable: no_picture,
                            fields,
                            reference_root: if list.custom_query
                                || !attr.columns.is_empty()
                                || !attr.additional_columns.is_empty()
                            {
                                None
                            } else {
                                list.main_table.as_deref().map(canonical_table)
                            },
                            untyped_roots: if context
                                .is_none_or(|context| context.calculated_fields_available())
                            {
                                list.calculated_fields
                                    .iter()
                                    .map(|field| field.data_path.clone())
                                    .collect()
                            } else {
                                BTreeSet::new()
                            },
                        },
                    )
                    .is_some()
                {
                    return Err(FormError::Frame(
                        "duplicate dynamic-list attribute identity".into(),
                    ));
                }
            }
        }
    }
    let tables = current_data_tables(body)?;
    let graph = context.map(|context| Arc::clone(&context.reference_graph));
    let _restore = Restore(
        AVAILABILITY.with(|m| m.replace(map)),
        RETAIN_SOURCE.with(|retain| retain.replace(retain_source)),
        REFERENCE_GRAPH.with(|current| current.replace(graph)),
        CURRENT_DATA_TABLES.with(|current| current.replace(tables)),
    );
    f()
}
fn fields(list: &DynamicListAttrExt) -> Result<Option<BTreeSet<String>>, FormError> {
    if !list.custom_query {
        return Ok(None);
    }
    if !list.auto_fill_available_fields {
        let mut fields: BTreeSet<String> = list
            .fields
            .iter()
            .map(|field| field.data_path.clone())
            .collect();
        fields.extend(
            list.calculated_fields
                .iter()
                .map(|field| field.data_path.clone()),
        );
        return Ok(Some(fields));
    }
    let Some(query) = &list.query_text else {
        return Ok(None);
    };
    let Some(Selection {
        mut fields,
        aliases,
        ..
    }) = selection(query)?
    else {
        return Ok(None);
    };
    // Standard platform columns also have localized field-map twins. Do not
    // interpret unrelated property bags as paths or inspect query string contents.
    for kind in [
        "Catalog",
        "Document",
        "Enum",
        "ChartOfCharacteristicTypes",
        "ChartOfAccounts",
        "ChartOfCalculationTypes",
        "ExchangePlan",
        "BusinessProcess",
        "Task",
        "InformationRegister",
        "AccumulationRegister",
        "AccountingRegister",
        "CalculationRegister",
        "DocumentJournal",
    ] {
        if let Some(pairs) = standard_pairs(kind) {
            for (ru, en) in pairs {
                if fields.contains(*ru) && !aliases.contains(*ru) {
                    fields.insert((*en).into());
                }
            }
        }
    }
    if let Some(table) = &list.main_table {
        let Some(pairs) = standard_pairs(table.split('.').next().unwrap_or_default()) else {
            return Ok(None);
        };
        for (ru, en) in pairs {
            // An explicit query alias is its own name, even when it spells a
            // localized platform field. The SDK does not add that selected
            // definition again under its other language spelling.
            if !fields.contains(*ru) && !fields.contains(*en) {
                fields.insert((*ru).into());
                fields.insert((*en).into());
            }
        }
    }
    for calculated in &list.calculated_fields {
        fields.insert(calculated.data_path.clone());
    }
    Ok(Some(fields))
}
fn ident(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && c.all(|c| c.is_alphanumeric() || c == '_')
}
fn keyword(s: &str, words: &[&str]) -> bool {
    words.iter().any(|w| s.to_uppercase() == *w)
}
fn tokens(query: &str) -> Result<Vec<String>, FormError> {
    let mut chars = Vec::new();
    chars
        .try_reserve_exact(query.chars().count())
        .map_err(|_| FormError::Frame("dynamic-list query allocation failed".into()))?;
    chars.extend(query.chars());
    let mut out = Vec::new();
    let mut i = 0;
    let mut stack = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        let start = i;
        if c == '"' {
            i += 1;
            let mut closed = false;
            while i < chars.len() {
                if chars[i] == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                return Err(FormError::Frame("unterminated query string".into()));
            }
        } else if c.is_alphanumeric() || c == '_' || c == '&' {
            i += 1;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
        } else {
            i += 1;
            if matches!(c, '(' | '{') {
                stack
                    .try_reserve(1)
                    .map_err(|_| FormError::Frame("query delimiter allocation failed".into()))?;
                stack.push(c);
            } else if matches!(c, ')' | '}')
                && stack.pop() != Some(if c == ')' { '(' } else { '{' })
            {
                return Err(FormError::Frame("unbalanced query delimiter".into()));
            }
        }
        out.try_reserve(1)
            .map_err(|_| FormError::Frame("query token allocation failed".into()))?;
        out.push(chars[start..i].iter().collect());
    }
    if !stack.is_empty() {
        return Err(FormError::Frame("unbalanced query delimiter".into()));
    }
    Ok(out)
}
fn top_keyword(tokens: &[String], words: &[&str]) -> bool {
    let mut depth = 0;
    tokens.iter().any(|t| {
        match t.as_str() {
            "(" | "{" => depth += 1,
            ")" | "}" => depth -= 1,
            _ => {}
        }
        depth == 0 && keyword(t, words)
    })
}
struct Selection {
    fields: BTreeSet<String>,
    aliases: BTreeSet<String>,
    sources: Vec<(String, String)>,
    selected_backing: BTreeSet<(String, String)>,
    has_star: bool,
    required_scope: Option<RequiredScope>,
}
struct RequiredScope {
    main_table: String,
    selected_backing: BTreeSet<(String, String)>,
    has_star: bool,
}
fn selected_field_identity(
    context: &FormProjectionContext<'_>,
    sources: &[(String, String)],
    expression: &[&str],
) -> Option<(String, String)> {
    let (qualifier, field) = match expression {
        [field] if ident(field) => (None, *field),
        [qualifier, ".", field] if ident(qualifier) && ident(field) => (Some(*qualifier), *field),
        _ => return None,
    };
    let mut definitions = sources.iter().filter_map(|(alias, table)| {
        if qualifier.is_some_and(|name| !super::java_case_fold::equal(name, alias)) {
            return None;
        }
        context
            .table(table)
            .filter(|schema| contains_name(&schema.fields, field))
            .map(|schema| (canonical_table(table), schema.field_identity(field)))
    });
    let first = definitions.next()?;
    // Ambiguous unqualified query names do not prove one backing definition.
    if definitions.next().is_some() {
        None
    } else {
        Some(first)
    }
}
fn selection(query: &str) -> Result<Option<Selection>, FormError> {
    selection_current(query, None, None)
}
fn selection_current(
    query: &str,
    context: Option<&FormProjectionContext<'_>>,
    main_table: Option<&str>,
) -> Result<Option<Selection>, FormError> {
    let tokens = tokens(query)?;
    let mut batches = Vec::new();
    let mut begin = 0;
    let mut depth = 0;
    for (i, t) in tokens.iter().enumerate() {
        match t.as_str() {
            "(" | "{" => depth += 1,
            ")" | "}" => depth -= 1,
            _ => {}
        }
        if depth == 0 && t == ";" {
            batches.push(&tokens[begin..i]);
            begin = i + 1;
        }
    }
    batches.push(&tokens[begin..]);
    let Some(batch) = batches.into_iter().rfind(|b| {
        top_keyword(b, &["ВЫБРАТЬ", "SELECT"]) && !top_keyword(b, &["ПОМЕСТИТЬ", "INTO"])
    }) else {
        return Ok(None);
    };
    // UNION operators have separate table-alias namespaces. The output
    // names belong to the first operator; required injection belongs to the
    // first operator which references the actual CURRENT main table.
    let mut operators = Vec::new();
    let mut start = 0;
    depth = 0;
    for (index, token) in batch.iter().enumerate() {
        match token.as_str() {
            "(" | "{" => depth += 1,
            ")" | "}" => depth -= 1,
            _ => {}
        }
        if depth == 0 && keyword(token, &["UNION", "ОБЪЕДИНИТЬ"]) {
            operators.push(&batch[start..index]);
            start = index + 1;
            if batch
                .get(start)
                .is_some_and(|next| keyword(next, &["ALL", "ВСЕ"]))
            {
                start += 1;
            }
        }
    }
    operators.push(&batch[start..]);
    let Some(mut selection) = selection_operator(operators[0], context)? else {
        return Ok(None);
    };
    if let Some(main) = main_table {
        for operator in operators {
            if query_tables(operator)
                .iter()
                .any(|(_, table)| canonical_table(table) == canonical_table(main))
            {
                let Some(scope) = selection_operator(operator, context)? else {
                    return Ok(None);
                };
                selection.required_scope = Some(RequiredScope {
                    main_table: main.into(),
                    selected_backing: scope.selected_backing,
                    has_star: scope.has_star,
                });
                break;
            }
        }
    }
    Ok(Some(selection))
}
fn selection_operator(
    batch: &[String],
    context: Option<&FormProjectionContext<'_>>,
) -> Result<Option<Selection>, FormError> {
    let Some(mut i) = batch
        .iter()
        .position(|t| keyword(t, &["ВЫБРАТЬ", "SELECT"]))
    else {
        return Ok(None);
    };
    i += 1;
    while i < batch.len()
        && (keyword(
            &batch[i],
            &[
                "РАЗРЕШЕННЫЕ",
                "ALLOWED",
                "РАЗЛИЧНЫЕ",
                "DISTINCT",
                "ПЕРВЫЕ",
                "TOP",
            ],
        ) || batch[i].chars().all(|c| c.is_ascii_digit()))
    {
        i += 1;
    }
    let mut terms = Vec::new();
    let mut term = Vec::new();
    let mut depth = 0;
    for t in &batch[i..] {
        if depth == 0
            && keyword(
                t,
                &[
                    "ИЗ",
                    "FROM",
                    "ГДЕ",
                    "WHERE",
                    "ПОМЕСТИТЬ",
                    "INTO",
                    "СГРУППИРОВАТЬ",
                    "GROUP",
                    "УПОРЯДОЧИТЬ",
                    "ORDER",
                    "ОБЪЕДИНИТЬ",
                    "UNION",
                    "ИТОГИ",
                    "TOTALS",
                ],
            )
        {
            break;
        }
        match t.as_str() {
            "(" | "{" => depth += 1,
            ")" | "}" => depth -= 1,
            _ => {}
        }
        if depth == 0 && t == "," {
            terms.push(std::mem::take(&mut term));
        } else {
            term.push(t.as_str());
        }
    }
    if !term.is_empty() {
        terms.push(term);
    }
    if terms.is_empty() {
        return Ok(None);
    }
    let mut fields = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    let sources = query_tables(batch);
    let mut selected_backing = BTreeSet::new();
    let mut has_star = false;
    for term in terms {
        let n = term.len();
        if n == 0 || term.contains(&"{") {
            return Ok(None);
        }
        if term.last() == Some(&"*") && (n == 1 || (n == 3 && ident(term[0]) && term[1] == ".")) {
            let Some(context) = context else {
                return Ok(None);
            };
            has_star = true;
            let selected: Vec<_> = sources
                .iter()
                .filter(|(alias, _)| n == 1 || super::java_case_fold::equal(alias, term[0]))
                .collect();
            if selected.is_empty() {
                return Ok(None);
            }
            for (_, table) in selected {
                let Some(schema) = context.table(table) else {
                    return Ok(None);
                };
                fields.extend(schema.fields);
            }
            continue;
        }
        let explicit_alias = n >= 2 && keyword(term[n - 2], &["КАК", "AS"]) && ident(term[n - 1]);
        let expression = if explicit_alias {
            &term[..n - 2]
        } else {
            &term[..]
        };
        if let Some(context) = context {
            if let Some(identity) = selected_field_identity(context, &sources, expression) {
                selected_backing.insert(identity);
            }
        }
        let name = if explicit_alias {
            aliases.insert(term[n - 1].to_owned());
            term[n - 1].to_owned()
        } else if n % 2 == 1
            && term
                .iter()
                .enumerate()
                .all(|(i, t)| if i % 2 == 0 { ident(t) } else { *t == "." })
        {
            let start = if n >= 5 && keyword(term[2], &["ССЫЛКА", "REF"]) {
                4
            } else if n > 1 {
                2
            } else {
                0
            };
            term[start..].iter().step_by(2).copied().collect()
        } else if n == 1 && term[0].starts_with('&') && ident(&term[0][1..]) {
            term[0][1..].to_owned()
        } else {
            // Expressions without an explicit alias do not prove a complete
            // result-field universe. In particular, the trailing member of
            // A.X + B.Y is not an implicit alias.
            return Ok(None);
        };
        fields.insert(name);
    }
    Ok(Some(Selection {
        fields,
        aliases,
        sources,
        selected_backing,
        has_star,
        required_scope: None,
    }))
}

fn standard_pairs(kind: &str) -> Option<&'static [(&'static str, &'static str)]> {
    const CATALOG: [(&str, &str); 10] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("Родитель", "Parent"),
        ("ЭтоГруппа", "IsFolder"),
        ("Владелец", "Owner"),
    ];
    const DOCUMENT: [(&str, &str); 7] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Дата", "Date"),
        ("Номер", "Number"),
        ("Проведен", "Posted"),
        ("ВерсияДанных", "DataVersion"),
        ("МоментВремени", "PointInTime"),
    ];
    const ENUM: [(&str, &str); 2] = [("Ссылка", "Ref"), ("Порядок", "Order")];
    const CHART_OF_CHARACTERISTIC_TYPES: [(&str, &str); 10] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("ТипЗначения", "ValueType"),
        ("Родитель", "Parent"),
        ("ЭтоГруппа", "IsFolder"),
    ];
    const CHART_OF_ACCOUNTS: [(&str, &str); 11] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("Родитель", "Parent"),
        ("Порядок", "Order"),
        ("Забалансовый", "OffBalance"),
        ("Вид", "Type"),
    ];
    const CHART_OF_CALCULATION_TYPES: [(&str, &str); 8] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("ПериодДействияБазовый", "ActionPeriodIsBasic"),
    ];
    const EXCHANGE_PLAN: [(&str, &str); 10] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Предопределенный", "Predefined"),
        ("ИмяПредопределенныхДанных", "PredefinedDataName"),
        ("ВерсияДанных", "DataVersion"),
        ("Код", "Code"),
        ("Наименование", "Description"),
        ("НомерОтправленного", "SentNo"),
        ("НомерПринятого", "ReceivedNo"),
        ("ЭтотУзел", "ThisNode"),
    ];
    const BUSINESS_PROCESS: [(&str, &str); 8] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("ВерсияДанных", "DataVersion"),
        ("Дата", "Date"),
        ("Номер", "Number"),
        ("Завершен", "Completed"),
        ("Стартован", "Started"),
        ("ВедущаяЗадача", "HeadTask"),
    ];
    const TASK: [(&str, &str); 9] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("ВерсияДанных", "DataVersion"),
        ("Дата", "Date"),
        ("Номер", "Number"),
        ("Наименование", "Description"),
        ("БизнесПроцесс", "BusinessProcess"),
        ("ТочкаМаршрута", "RoutePoint"),
        ("Выполнена", "Executed"),
    ];
    const INFORMATION_REGISTER: [(&str, &str); 4] = [
        ("Период", "Period"),
        ("Регистратор", "Recorder"),
        ("НомерСтроки", "LineNumber"),
        ("Активность", "Active"),
    ];
    const ACCUMULATION_REGISTER: [(&str, &str); 5] = [
        ("Период", "Period"),
        ("Регистратор", "Recorder"),
        ("НомерСтроки", "LineNumber"),
        ("Активность", "Active"),
        ("ВидДвижения", "RecordType"),
    ];
    const CALCULATION_REGISTER: [(&str, &str); 12] = [
        ("Период", "Period"),
        ("Регистратор", "Recorder"),
        ("НомерСтроки", "LineNumber"),
        ("Активность", "Active"),
        ("ПериодРегистрации", "RegistrationPeriod"),
        ("ПериодДействия", "ActionPeriod"),
        ("ПериодДействияНачало", "BegOfActionPeriod"),
        ("ПериодДействияКонец", "EndOfActionPeriod"),
        ("БазовыйПериодНачало", "BegOfBasePeriod"),
        ("БазовыйПериодКонец", "EndOfBasePeriod"),
        ("ВидРасчета", "CalculationType"),
        ("Сторно", "ReversingEntry"),
    ];
    const DOCUMENT_JOURNAL: [(&str, &str); 6] = [
        ("Ссылка", "Ref"),
        ("ПометкаУдаления", "DeletionMark"),
        ("Дата", "Date"),
        ("Номер", "Number"),
        ("Проведен", "Posted"),
        ("Тип", "Type"),
    ];
    match kind {
        "Catalog" => Some(&CATALOG),
        "Document" => Some(&DOCUMENT),
        "Enum" => Some(&ENUM),
        "ChartOfCharacteristicTypes" => Some(&CHART_OF_CHARACTERISTIC_TYPES),
        "ChartOfAccounts" => Some(&CHART_OF_ACCOUNTS),
        "ChartOfCalculationTypes" => Some(&CHART_OF_CALCULATION_TYPES),
        "ExchangePlan" => Some(&EXCHANGE_PLAN),
        "BusinessProcess" => Some(&BUSINESS_PROCESS),
        "Task" => Some(&TASK),
        "InformationRegister" => Some(&INFORMATION_REGISTER),
        "AccumulationRegister" => Some(&ACCUMULATION_REGISTER),
        "AccountingRegister" => Some(&INFORMATION_REGISTER),
        "CalculationRegister" => Some(&CALCULATION_REGISTER),
        "DocumentJournal" => Some(&DOCUMENT_JOURNAL),
        _ => None,
    }
}

// Original DCS eFormField reference expansion: current main DbView definitions,
// actual reference targets and parent availability, never arbitrary dotted names.
#[derive(Clone, Default)]
struct ReferenceGraph {
    nodes: BTreeMap<String, ReferenceNode>,
}
#[derive(Clone)]
struct ReferenceNode {
    fields: BTreeMap<String, ReferenceField>,
    complete: bool,
}
#[derive(Clone, Default)]
struct ReferenceField {
    targets: BTreeSet<String>,
    type_ids: BTreeSet<String>,
    unknown_type: bool,
    compound_type: bool,
    table: bool,
    record_reference: bool,
}
impl ReferenceField {
    fn merge(&mut self, other: &Self) {
        self.targets.extend(other.targets.iter().cloned());
        self.type_ids.extend(other.type_ids.iter().cloned());
        self.unknown_type |= other.unknown_type;
        self.compound_type |= other.compound_type || self.type_ids.len() > 1;
    }
}

fn reference_type(type_id: &str) -> Option<String> {
    let (kind, name) = type_id.split_once('.')?;
    let kind = kind.strip_suffix("Ref")?;
    // A reference identity comes from TypeSpec, not from a guessed query alias.
    let kind = family(kind)?;
    Some(super::java_case_fold::fold(&format!("{kind}.{name}")))
}
fn field_type(value: Option<&PropertyValue>) -> ReferenceField {
    let Some(PropertyValue::Type(value)) = value else {
        return ReferenceField {
            unknown_type: true,
            ..ReferenceField::default()
        };
    };
    let mut field = ReferenceField {
        compound_type: value.parts.len() > 1,
        ..ReferenceField::default()
    };
    for part in &value.parts {
        field.type_ids.insert(part.id.clone());
        if let Some(target) = reference_type(&part.id) {
            field.targets.insert(target);
        } else if !part.has_complete_dbview_leaf_roster() {
            // Date expressions, Number resources, TypeSet/DefinedType and other
            // provider expansion need their actual registry and restrictions.
            // Unknown is not a negative lookup or an invented all-fields node.
            field.unknown_type = true;
        }
    }
    field
}

impl FormProjectionContext<'_> {
    fn remove_password_fields(&self) -> bool {
        let id = morph1c_core::spec::metadata::configuration::F_COMPATIBILITY_MODE;
        let mode = self
            .metadata
            .properties
            .iter()
            .find(|(field, _)| *field == id)
            .and_then(|(_, value)| match value {
                PropertyValue::Enum(mode) => Some(mode.as_str()),
                _ => None,
            })
            .unwrap_or("");
        if mode.is_empty() {
            return false;
        }
        if mode == "DontUse" {
            return true;
        }
        let mut parts = mode.split('.').map(str::parse::<u32>);
        matches!((parts.next(), parts.next(), parts.next(), parts.next()),
            (Some(Ok(major)), Some(Ok(minor)), Some(Ok(patch)), None)
                if [major, minor, patch] <= morph1c_core::spec::metadata::configuration::PASSWORD_FIELDS_COMPATIBILITY_BOUNDARY)
    }

    fn reference_graph(&self) -> ReferenceGraph {
        let mut graph = ReferenceGraph::default();
        for (identity, object) in &self.objects {
            let Some(schema) = self.table(&format!("{}.{}", object.kind.as_str(), object.name))
            else {
                continue;
            };
            let mut fields: BTreeMap<_, _> = schema
                .fields
                .iter()
                .map(|name| {
                    (
                        super::java_case_fold::fold(name),
                        ReferenceField {
                            unknown_type: true,
                            ..ReferenceField::default()
                        },
                    )
                })
                .collect();
            // Ref belongs to the root result but LocalBase excludes the actual
            // recordRefField when expanding a reference. Presentation is a
            // distinct virtual field and is not in this main-field registry.
            for (ru, en) in &schema.standards {
                if *en == "Ref" {
                    let field = ReferenceField {
                        targets: BTreeSet::from([identity.clone()]),
                        record_reference: true,
                        ..ReferenceField::default()
                    };
                    for name in [*ru, *en] {
                        fields.insert(super::java_case_fold::fold(name), field.clone());
                    }
                }
            }
            for child in &object.children {
                let suffix = child.kind.as_str().rsplit('.').next();
                if matches!(
                    suffix,
                    Some(
                        "Attribute"
                            | "Dimension"
                            | "Resource"
                            | "AccountingFlag"
                            | "ExtDimensionAccountingFlag"
                    )
                ) {
                    let key = super::java_case_fold::fold(&child.name);
                    if self.remove_password_fields() && current_bool(child, "passwordMode") {
                        fields.remove(&key);
                    } else if fields.contains_key(&key) {
                        fields.insert(key, field_type(current_property(child, "type")));
                    }
                } else if suffix == Some("TabularSection") {
                    // Single-reference expansion includes actual authored table
                    // definitions; mixed references exclude them in DcsUtil.
                    let target = format!("{identity}.{}", super::java_case_fold::fold(&child.name));
                    let mut table_fields = BTreeMap::new();
                    for attribute in &child.children {
                        if attribute.kind.as_str().ends_with(".Attribute")
                            && !(self.remove_password_fields()
                                && current_bool(attribute, "passwordMode"))
                        {
                            table_fields.insert(
                                super::java_case_fold::fold(&attribute.name),
                                field_type(current_property(attribute, "type")),
                            );
                        }
                    }
                    // Existing table registry owns the proved standard aliases.
                    if let Some(table) = self.table(&format!(
                        "{}.{}.{}",
                        object.kind.as_str(),
                        object.name,
                        child.name
                    )) {
                        for (ru, en) in table.standards {
                            for name in [ru, en] {
                                table_fields
                                    .entry(super::java_case_fold::fold(name))
                                    .or_insert(ReferenceField {
                                        unknown_type: true,
                                        ..ReferenceField::default()
                                    });
                            }
                        }
                    }
                    graph.nodes.insert(
                        target.clone(),
                        ReferenceNode {
                            fields: table_fields,
                            complete: true,
                        },
                    );
                    fields.insert(
                        super::java_case_fold::fold(&child.name),
                        ReferenceField {
                            targets: BTreeSet::from([target]),
                            table: true,
                            ..ReferenceField::default()
                        },
                    );
                }
            }
            for common in self
                .objects
                .values()
                .filter(|o| o.kind.as_str() == "CommonAttribute")
            {
                let key = super::java_case_fold::fold(&common.name);
                if !fields.contains_key(&key) {
                    continue;
                }
                if self.remove_password_fields() && current_bool(common, "passwordMode") {
                    fields.remove(&key);
                } else {
                    fields.insert(key, field_type(current_property(common, "type")));
                }
            }
            // Other families have provider-derived views/standard definitions
            // beyond the existing main registry. Their known authored fields
            // remain positive; absence must not become a guessed negative.
            let complete = morph1c_core::spec::ir_child::ReferenceChildRosterFamily::for_kind(
                object.kind.as_str(),
            )
            .is_some();
            graph
                .nodes
                .insert(identity.clone(), ReferenceNode { fields, complete });
        }
        graph
    }
}

impl ReferenceGraph {
    // Some(true)=proved missing, Some(false)=proved present, None=unknown.
    // Each segment consumes one CURRENT typed edge; cycles need no depth limit.
    fn resolve(&self, root: &str, path: &str) -> Option<bool> {
        let mut targets = BTreeSet::from([root.to_owned()]);
        let mut unknown = false;
        let mut reference_child = false;
        let mut current_compound = false;
        let mut inherited_multi_ref = false;
        let mut in_nested_table = false;
        let mut segments = path.split('.').peekable();
        while let Some(segment) = segments.next() {
            let key = super::java_case_fold::fold(cut_index(segment));
            let mut found: Option<ReferenceField> = None;
            for target in &targets {
                let Some(node) = self.nodes.get(target) else {
                    unknown = true;
                    continue;
                };
                if let Some(field) = node.fields.get(&key) {
                    if reference_child
                        && (field.record_reference
                            || ((current_compound || inherited_multi_ref || in_nested_table)
                                && field.table))
                    {
                        continue;
                    }
                    if let Some(found) = &mut found {
                        found.merge(field);
                    } else {
                        found = Some(field.clone());
                    }
                } else if !node.complete {
                    unknown = true;
                }
            }
            let Some(field) = found else {
                return (!unknown).then_some(true);
            };
            if segments.peek().is_none() {
                return Some(false);
            }
            targets = field.targets;
            unknown |= field.unknown_type;
            // DcsUtil's current TypeDescription compound predicate is local to
            // this expansion. LocalBase's actual-reference multiplicity and
            // nested-table flags are inherited independently.
            current_compound = field.compound_type;
            inherited_multi_ref |= targets.len() > 1;
            in_nested_table |= field.table;
            reference_child = true;
        }
        None
    }
}

fn current_data_tables(body: &FormBody) -> Result<BTreeMap<String, Option<String>>, FormError> {
    let mut tables = BTreeMap::new();
    let mut pending: Vec<_> = body.items.iter().rev().collect();
    if let Some(bar) = &body.auto_command_bar {
        pending.extend(bar.items.iter().rev());
    }
    while let Some(item) = pending.pop() {
        if item.kind.as_str() == "Table" {
            let source = match item.get(morph1c_core::spec::forms::controls::table::F_DATA_PATH) {
                Some(PropertyValue::DataPath(path)) if path.extra_paths.is_empty() => {
                    Some(path.primary())
                }
                Some(PropertyValue::DataPath(_)) => None,
                Some(PropertyValue::Ref(path)) => Some(path.as_str().to_owned()),
                None => None,
                _ => {
                    return Err(FormError::Frame(
                        "current Table dataPath type mismatch".into(),
                    ));
                }
            };
            if tables
                .insert(super::java_case_fold::fold(&item.name), source)
                .is_some()
            {
                return Err(FormError::Frame("duplicate CURRENT Table identity".into()));
            }
        }
        pending.extend(item.children.iter().rev());
        pending.extend(item.additions.iter().rev());
        if let Some(bar) = &item.auto_command_bar {
            pending.extend(bar.items.iter().rev());
        }
        // AutoTable paths require the separately proved parent-path derivation.
        // Do not manufacture a binding from its name or dotted path spelling.
    }
    Ok(tables)
}

fn resolve_current_path(
    map: &BTreeMap<String, Availability>,
    graph: Option<&ReferenceGraph>,
    tables: &BTreeMap<String, Option<String>>,
    path: &str,
) -> Option<bool> {
    if let Some((table, current, field)) =
        morph1c_core::spec::forms::controls::table::table_element_member_path(path)
    {
        if !morph1c_core::spec::forms::controls::table::CURRENT_DATA_MEMBER_NAMES
            .iter()
            .any(|name| super::java_case_fold::equal(cut_index(current), name))
        {
            return None;
        }
        let source = tables.get(&super::java_case_fold::fold(cut_index(table)))?;
        // EMPTY_CURRENT_DATA has no fields, but with no CURRENT list source
        // it is not related to a DynamicList for DataPathWriter's marker.
        let source = source.as_deref()?;
        // Only actual DynamicList sources are in this map. A regular form
        // attribute/table needs its own existing PropertyInfo provider.
        let (owner, _) = source.split_once('.').unwrap_or((source, ""));
        if !map.contains_key(&super::java_case_fold::fold(cut_index(owner))) {
            return None;
        }
        return resolve_list_path(map, graph, &format!("{source}.{field}"));
    }
    resolve_list_path(map, graph, path)
}

fn resolve_list_path(
    map: &BTreeMap<String, Availability>,
    graph: Option<&ReferenceGraph>,
    path: &str,
) -> Option<bool> {
    let (owner, field) = path.split_once('.')?;
    let a = map.get(&super::java_case_fold::fold(cut_index(owner)))?;
    if super::java_case_fold::equal(field, "Order")
        || super::java_case_fold::equal(field, "Порядок")
    {
        return Some(false);
    }
    if field.eq_ignore_ascii_case("DefaultPicture") {
        return Some(a.default_picture_unavailable);
    }
    // Preserve the existing custom-query/standalone dotted-path boundary. Only
    // a CURRENT default-list root supplies the newly proved reference graph.
    if field.contains('.') && a.reference_root.is_none() {
        return None;
    }
    let fields = a.fields.as_ref()?;
    let first = cut_index(field.split('.').next()?);
    if !contains_name(fields, first) {
        return Some(true);
    }
    if !field.contains('.') {
        return Some(false);
    }
    if contains_name(&a.untyped_roots, first) {
        return None;
    }
    let graph = graph?;
    let root = a.reference_root.as_deref()?;
    // Calculated/schema-only fields belong to their actual expression provider,
    // not to a manufactured metadata edge (even if their name is declared).
    if !graph
        .nodes
        .get(root)?
        .fields
        .contains_key(&super::java_case_fold::fold(first))
    {
        return None;
    }
    graph.resolve(root, field)
}

fn cut_index(segment: &str) -> &str {
    segment.split_once('[').map_or(segment, |(name, _)| name)
}

#[cfg(test)]
mod reference_child_tests {
    use super::*;
    use morph1c_core::ir::{FormControlKind, FormItem, ObjectKind, Token, TypeRef, TypeSpec, Uuid};
    use morph1c_core::version::with_roundtrip_target;

    fn attribute(kind: &str, name: &str, types: &[&str]) -> MetadataObject {
        let mut object = MetadataObject::new(ObjectKind::new(kind), name, Uuid([2; 16]));
        object.properties.push((
            morph1c_core::spec::ir_child::F_TYPE,
            PropertyValue::Type(TypeSpec {
                parts: types
                    .iter()
                    .map(|name| TypeRef {
                        id: (*name).to_owned(),
                        qualifier: None,
                    })
                    .collect(),
            }),
        ));
        object
    }
    fn metadata() -> Configuration {
        let mut metadata = Configuration::new();
        let mut source = MetadataObject::new(
            ObjectKind::new("AccumulationRegister"),
            "Source",
            Uuid([1; 16]),
        );
        source.children.push(attribute(
            "AccumulationRegister.Attribute",
            "Reference",
            &["CatalogRef.Target"],
        ));
        let mut target = MetadataObject::new(ObjectKind::new("Catalog"), "Target", Uuid([3; 16]));
        target
            .children
            .push(attribute("Catalog.Attribute", "KnownChild", &["String"]));
        target
            .children
            .push(attribute("Catalog.Attribute", "Next", &["CatalogRef.Deep"]));
        target
            .children
            .push(attribute("Catalog.Attribute", "Password", &["String"]));
        target.children[2].properties.push((
            morph1c_core::spec::ir_child::F_PASSWORD_MODE,
            PropertyValue::Bool(true),
        ));
        let mut deep = MetadataObject::new(ObjectKind::new("Catalog"), "Deep", Uuid([4; 16]));
        deep.children
            .push(attribute("Catalog.Attribute", "Leaf", &["Boolean"]));
        let mut other = MetadataObject::new(ObjectKind::new("Catalog"), "Other", Uuid([5; 16]));
        other
            .children
            .push(attribute("Catalog.Attribute", "OtherChild", &["Number"]));
        metadata.objects.extend([source, target, deep, other]);
        metadata
    }
    fn body() -> FormBody {
        let xml = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<form:Form xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:core=\"http://g5.1c.ru/v8/dt/mcore\" xmlns:form=\"http://g5.1c.ru/v8/dt/form\">\r\n<attributes><name>List</name><valueType><types>DynamicList</types></valueType><view><common>true</common></view><edit><common>true</common></edit><extInfo xsi:type=\"form:DynamicListExtInfo\"><mainTable>AccumulationRegister.Source</mainTable></extInfo></attributes>\r\n</form:Form>\r\n";
        super::super::read_form(super::super::FormDialect::Edt, xml).unwrap()
    }
    fn lookup(metadata: &Configuration, body: &FormBody, minor: u16, path: &str) -> Option<bool> {
        let context = FormProjectionContext::new(metadata).unwrap();
        with_roundtrip_target(FormatVersion::new(2, minor), || {
            with_availability(body, Some(&context), || Ok(resolve(path)))
        })
        .unwrap()
    }
    #[test]
    fn provider_expanded_primitive_children_retain_unknown_across_current_owners() {
        let mut metadata = metadata();
        metadata.objects[0].children.extend([
            attribute("AccumulationRegister.Attribute", "When", &["Date"]),
            attribute("AccumulationRegister.Resource", "Amount", &["Number"]),
            attribute(
                "AccumulationRegister.Attribute",
                "MixedDate",
                &["Date", "CatalogRef.Target"],
            ),
            attribute(
                "AccumulationRegister.Attribute",
                "MixedNumber",
                &["Number", "CatalogRef.Target"],
            ),
        ]);
        metadata.objects[1].children.extend([
            attribute("Catalog.Attribute", "When", &["Date"]),
            attribute("Catalog.Attribute", "Amount", &["Number"]),
        ]);
        let mut body = body();
        let mut table = FormItem::new(FormControlKind::new("Table"), "Rows", 1);
        table.properties.push((
            morph1c_core::spec::forms::controls::table::F_DATA_PATH,
            PropertyValue::Ref("List".into()),
        ));
        body.items.push(table);
        for minor in [20, 21] {
            for path in [
                "List.When",
                "List.Amount",
                "List.Reference.When",
                "List.Reference.Amount",
                "List.MixedDate.KnownChild",
                "List.MixedNumber.KnownChild",
                "Items.Rows.CurrentData.When",
            ] {
                assert_eq!(
                    lookup(&metadata, &body, minor, path),
                    Some(false),
                    "actual current leaf/member: {path}"
                );
            }
            for path in [
                "List.When.DateParts.Year",
                "List.Amount.PercentOverall",
                "List.Reference.When.DateParts.Year",
                "List.Reference.Amount.PercentOverall",
                "Items.Rows.CurrentData.When.DateParts.Year",
                "Items.Rows.CurrentData.Amount.PercentOverall",
                "List.MixedDate.DateParts.Year",
                "List.MixedNumber.PercentOverall",
                "Items.Rows.CurrentData.MixedDate.DateParts.Year",
            ] {
                assert_eq!(
                    lookup(&metadata, &body, minor, path),
                    None,
                    "unproved expression/resource provider: {path}"
                );
            }
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.KnownChild.Missing"),
                Some(true),
                "proved empty String roster remains negative"
            );
            // CURRENT replacement with a proved leaf invalidates the previous
            // provider possibility; retaining unknown must not be a stale cache.
            metadata.objects[0].children[1] =
                attribute("AccumulationRegister.Attribute", "When", &["Boolean"]);
            assert_eq!(
                lookup(&metadata, &body, minor, "List.When.DateParts.Year"),
                Some(true)
            );
            metadata.objects[0].children[1] =
                attribute("AccumulationRegister.Attribute", "When", &["Date"]);
        }
    }

    #[test]
    fn current_default_list_reference_child_graph_positive_negative_and_deep() {
        let mut metadata = metadata();
        let body = body();
        for minor in [20, 21] {
            for path in [
                "List.Reference.KnownChild",
                "List.Reference.Next.Leaf",
                "list.reference.knownchild",
                "List.Reference[0].KnownChild",
            ] {
                assert_eq!(lookup(&metadata, &body, minor, path), Some(false), "{path}");
            }
            for path in [
                "List.Reference.MissingChild",
                "List.Reference.Missing.Leaf",
                "List.Reference.Next.Missing",
                "List.Reference.Ref",
                "List.Reference.Presentation",
                "List.Reference.KnownChild.Anything",
            ] {
                assert_eq!(lookup(&metadata, &body, minor, path), Some(true), "{path}");
            }
            // A missing intermediate cannot use a later name from another branch.
            metadata.objects[1].children[1].name = "RenamedNext".into();
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.Next.Leaf"),
                Some(true)
            );
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.RenamedNext.Leaf"),
                Some(false)
            );
            metadata.objects[1].children[1].name = "Next".into();
            metadata.objects[1].children[0].name = "CurrentChild".into();
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.KnownChild"),
                Some(true)
            );
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.CurrentChild"),
                Some(false)
            );
            metadata.objects[1].children[0].name = "KnownChild".into();
        }
    }
    #[test]
    fn current_reference_union_retarget_unknown_provider_and_password_compatibility() {
        let mut metadata = metadata();
        let body = body();
        for minor in [20, 21] {
            metadata.objects[0].children[0] = attribute(
                "AccumulationRegister.Attribute",
                "Reference",
                &["CatalogRef.Target", "CatalogRef.Other"],
            );
            for path in [
                "List.Reference.KnownChild",
                "List.Reference.OtherChild",
                "List.Reference.Next.Leaf",
            ] {
                assert_eq!(lookup(&metadata, &body, minor, path), Some(false));
            }
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.Missing"),
                Some(true)
            );
            metadata.objects[0].children[0] = attribute(
                "AccumulationRegister.Attribute",
                "Reference",
                &["CatalogRef.Other"],
            );
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.KnownChild"),
                Some(true)
            );
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.OtherChild"),
                Some(false)
            );
            metadata.objects[0].children[0] =
                attribute("AccumulationRegister.Attribute", "Reference", &["AnyRef"]);
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.Missing"),
                None
            );
            metadata.objects[0].children[0] = attribute(
                "AccumulationRegister.Attribute",
                "Reference",
                &["CatalogRef.Target"],
            );
            let compatibility = morph1c_core::spec::metadata::configuration::F_COMPATIBILITY_MODE;
            metadata.properties = vec![(compatibility, PropertyValue::Enum(Token::new("8.3.11")))];
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.Password"),
                Some(true)
            );
            metadata.properties = vec![(compatibility, PropertyValue::Enum(Token::new("8.3.12")))];
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.Password"),
                Some(false)
            );
            metadata.properties.clear();
        }
    }
    #[test]
    fn table_current_data_uses_only_its_current_dynamic_list_binding() {
        let metadata = metadata();
        let mut body = body();
        let mut table = FormItem::new(FormControlKind::new("Table"), "ActualTable", 1);
        let path = morph1c_core::spec::forms::controls::table::F_DATA_PATH;
        table
            .properties
            .push((path, PropertyValue::DataPath("List".into())));
        body.items.push(table);
        let mut second = body.data_attributes[0].clone();
        second.name = "OtherList".into();
        second.dynamic_list.as_mut().unwrap().main_table = Some("Catalog.Other".into());
        body.data_attributes.push(second);
        for minor in [20, 21] {
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "Items.ActualTable.CurrentData.Reference.KnownChild"
                ),
                Some(false)
            );
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "Items.ActualTable.CurrentData.Reference.Missing"
                ),
                Some(true)
            );
            body.items[0].properties[0].1 = PropertyValue::DataPath("OtherList".into());
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "Items.ActualTable.CurrentData.OtherChild"
                ),
                Some(false)
            );
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "Items.ActualTable.CurrentData.Reference.KnownChild"
                ),
                Some(true)
            );
            body.items[0].properties.clear();
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "Items.ActualTable.CurrentData.OtherChild"
                ),
                None
            );
            body.items[0]
                .properties
                .push((path, PropertyValue::DataPath("List".into())));
            let mut second_table = body.items[0].clone();
            second_table.name = "SecondTable".into();
            second_table.id = 2;
            second_table.properties[0].1 = PropertyValue::DataPath("OtherList".into());
            body.items.push(second_table);
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "Items.SecondTable.CurrentData.OtherChild"
                ),
                Some(false)
            );
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "Items.ActualTable.CurrentData.OtherChild"
                ),
                Some(true)
            );
            body.items.pop();
        }
    }
    #[test]
    fn default_reference_graph_does_not_change_custom_query_alias_resolution() {
        let metadata = metadata();
        let mut body = body();
        let list = body.data_attributes[0].dynamic_list.as_mut().unwrap();
        list.custom_query = true;
        list.auto_fill_available_fields = true;
        list.query_text =
            Some("SELECT R.Reference AS CurrentAlias FROM AccumulationRegister.Source AS R".into());
        for minor in [20, 21] {
            assert_eq!(
                lookup(&metadata, &body, minor, "List.CurrentAlias"),
                Some(false)
            );
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference"),
                Some(true)
            );
            // Alias type inference is not replaced by a guessed main-table edge.
            assert_eq!(
                lookup(&metadata, &body, minor, "List.CurrentAlias.KnownChild"),
                None
            );
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.KnownChild"),
                None
            );
        }
    }
    #[test]
    fn default_reference_graph_drives_production_native_markers_from_current_values() {
        let mut metadata = metadata();
        let mut body = body();
        body.data_attributes[0].not_default_use_always = [
            "List.Reference.KnownChild",
            "List.Reference.MissingChild",
            "List.Reference.Next.Leaf",
            "List.Reference.Missing.Leaf",
        ]
        .map(morph1c_core::ir::form::DataPathSpec::from_form_text)
        .to_vec();
        let emitted = |metadata: &Configuration, body: &FormBody, minor| {
            let context = FormProjectionContext::new(metadata).unwrap();
            let bytes = with_roundtrip_target(FormatVersion::new(2, minor), || {
                super::super::write_form_with_context(
                    super::super::FormDialect::Designer,
                    body,
                    &context,
                )
            })
            .unwrap();
            crate::parse(&bytes)
                .unwrap()
                .root
                .child("Attributes")
                .unwrap()
                .child("Attribute")
                .unwrap()
                .child("UseAlways")
                .unwrap()
                .children
                .iter()
                .map(|field| field.text.clone())
                .collect::<Vec<_>>()
        };
        for minor in [20, 21] {
            assert_eq!(
                emitted(&metadata, &body, minor),
                [
                    "List.Reference.KnownChild",
                    "~List.Reference.MissingChild",
                    "List.Reference.Next.Leaf",
                    "~List.Reference.Missing.Leaf",
                ]
            );
            // Rename CURRENT target data; the old name cannot prove presence.
            metadata.objects[1].children[0].name = "MissingChild".into();
            assert_eq!(
                emitted(&metadata, &body, minor),
                [
                    "~List.Reference.KnownChild",
                    "List.Reference.MissingChild",
                    "List.Reference.Next.Leaf",
                    "~List.Reference.Missing.Leaf",
                ]
            );
            metadata.objects[1].children[0].name = "KnownChild".into();
        }
    }
    #[test]
    fn compound_type_and_inherited_reference_table_policies_remain_distinct() {
        let mut metadata = metadata();
        let body = body();
        let mut first = MetadataObject::new(
            ObjectKind::new("Catalog.TabularSection"),
            "FirstTable",
            Uuid([6; 16]),
        );
        first.children.push(attribute(
            "Catalog.TabularSection.Attribute",
            "Link",
            &["CatalogRef.Deep"],
        ));
        metadata.objects[1].children.push(first);
        let mut second = MetadataObject::new(
            ObjectKind::new("Catalog.TabularSection"),
            "SecondTable",
            Uuid([7; 16]),
        );
        second.children.push(attribute(
            "Catalog.TabularSection.Attribute",
            "TableLeaf",
            &["String"],
        ));
        metadata.objects[2].children.push(second);
        metadata.objects[3].children.push(attribute(
            "Catalog.Attribute",
            "Next",
            &["CatalogRef.Deep"],
        ));
        for minor in [20, 21] {
            // A single reference permits its current declared table definition.
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "List.Reference.FirstTable.Link.Leaf"
                ),
                Some(false)
            );
            // LocalBase's inNestedTable flag is inherited through scalar refs.
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "List.Reference.FirstTable.Link.SecondTable.TableLeaf"
                ),
                Some(true)
            );
            // CURRENT compound type excludes its immediate table definitions,
            // but String contributes no DbObjectDef / inherited inMultiRef.
            metadata.objects[0].children[0] = attribute(
                "AccumulationRegister.Attribute",
                "Reference",
                &["CatalogRef.Target", "String"],
            );
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.FirstTable"),
                Some(true)
            );
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "List.Reference.Next.SecondTable.TableLeaf"
                ),
                Some(false)
            );
            // A genuine two-reference parent sets LocalBase.inMultiRef; shared
            // deduplicated child targets do not clear the inherited flag.
            metadata.objects[0].children[0] = attribute(
                "AccumulationRegister.Attribute",
                "Reference",
                &["CatalogRef.Target", "CatalogRef.Other"],
            );
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.Next.Leaf"),
                Some(false)
            );
            assert_eq!(
                lookup(
                    &metadata,
                    &body,
                    minor,
                    "List.Reference.Next.SecondTable.TableLeaf"
                ),
                Some(true)
            );
            metadata.objects[0].children[0] = attribute(
                "AccumulationRegister.Attribute",
                "Reference",
                &["CatalogRef.Target"],
            );
        }
    }
    #[test]
    fn calculated_field_children_remain_with_their_current_expression_provider() {
        let metadata = metadata();
        let mut body = body();
        body.data_attributes[0]
            .dynamic_list
            .as_mut()
            .unwrap()
            .calculated_fields
            .push(
                serde_json::from_value::<morph1c_core::ir::form::DcsCalculatedField>(
                    serde_json::json!({
                        "data_path": "Computed", "expression": "1"
                    }),
                )
                .unwrap(),
            );
        for minor in [20, 21] {
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Computed"),
                Some(false)
            );
            assert_eq!(lookup(&metadata, &body, minor, "List.Computed.Child"), None);
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.Missing"),
                Some(true)
            );
            let context = FormProjectionContext::new(&metadata).unwrap();
            let profile = FormatVersion::new(2, minor);
            let before = context.dependency_sha256(&body, profile).unwrap();
            body.data_attributes[0]
                .dynamic_list
                .as_mut()
                .unwrap()
                .calculated_fields[0]
                .expression = "2".into();
            assert_ne!(context.dependency_sha256(&body, profile).unwrap(), before);
            assert_eq!(lookup(&metadata, &body, minor, "List.Computed.Child"), None);
            body.data_attributes[0]
                .dynamic_list
                .as_mut()
                .unwrap()
                .calculated_fields[0]
                .data_path = "Renamed".into();
            assert_ne!(context.dependency_sha256(&body, profile).unwrap(), before);
            assert_eq!(lookup(&metadata, &body, minor, "List.Computed"), Some(true));
            assert_eq!(lookup(&metadata, &body, minor, "List.Renamed"), Some(false));
            assert_eq!(lookup(&metadata, &body, minor, "List.Renamed.Child"), None);
            // A same-name calculated definition cannot borrow a metadata type.
            body.data_attributes[0]
                .dynamic_list
                .as_mut()
                .unwrap()
                .calculated_fields[0]
                .data_path = "Reference".into();
            assert_eq!(
                lookup(&metadata, &body, minor, "List.Reference.KnownChild"),
                None
            );
            body.data_attributes[0]
                .dynamic_list
                .as_mut()
                .unwrap()
                .calculated_fields[0]
                .data_path = "Computed".into();
            body.data_attributes[0]
                .dynamic_list
                .as_mut()
                .unwrap()
                .calculated_fields[0]
                .expression = "1".into();
        }
    }
}
